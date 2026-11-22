//! AdaptionWheel Horizon native mod (server-authoritative adaptation core, Horizon 26.3-era APIs).
//!
//! Features:
//! - Wheel activation when the wheel item is present anywhere in player inventory (inventory hook scan).
//! - Per-player adaptation state cache (in-memory; attachment persistence TODO/wiring).
//! - Adaptation concepts/scenarios with progress and levels:
//!   - env_fall, type_explosion (existing)
//!   - env_fire, env_lava, type_general, env_drowning, type_contact (added)
//! - Hooks integration: inventory activation, player tick mapping, incoming damage (v3), fall, drown.
//! - Debug commands: /adaptionwheel status|activate|set|grant|reset|help

mod concepts_registry;
mod damage_categories;
mod existence_mut;
mod fist_ladders;
mod state_v2;
mod synergies;
mod task_system;

use horizon_client::mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, Compound, Flow, Holder, Hooks, Host,
    LivingDrownEvent, LivingFallEvent, LivingIncomingDamageV3Event, NativeCommand,
    NativeServerPlayer, PerformCommandEvent, PerformConsoleCommandEvent, PlayerCloneEvent,
    PlayerInventoryChangedEvent, PlayerLoggedInEvent, PlayerTickEvent, RegisterCommandsEvent, Tag,
    Text,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, OnceLock};

const MOD_ID: &str = "adaptionwheel";
const STATE_SCHEMA_VERSION: u32 = 1;

const CONCEPT_FALL: &str = "env_fall";
const CONCEPT_EXPLOSION: &str = "type_explosion";
const CONCEPT_FIRE: &str = "env_fire";
const CONCEPT_LAVA: &str = "env_lava";
const CONCEPT_GENERAL: &str = "type_general";
const CONCEPT_DROWNING: &str = "env_drowning";
const CONCEPT_CONTACT: &str = "type_contact";

const ALL_CONCEPTS: [&str; 7] = [
    CONCEPT_FALL,
    CONCEPT_EXPLOSION,
    CONCEPT_FIRE,
    CONCEPT_LAVA,
    CONCEPT_GENERAL,
    CONCEPT_DROWNING,
    CONCEPT_CONTACT,
];

const FALL_THRESHOLD: f32 = 40.0;
const FALL_MAX_GAIN_PER_EVENT: f32 = 20.0;
const EXPLOSION_MAX_LEVEL_MVP: u8 = 3;

const FIRE_MAX_LEVEL: u8 = 3;
const LAVA_MAX_LEVEL: u8 = 3;
const GENERAL_MAX_LEVEL: u8 = 3;
const DROWNING_MAX_LEVEL: u8 = 3;
const CONTACT_MAX_LEVEL: u8 = 3;

fn wheel_item_ids() -> &'static [&'static str] {
    &[
        "adaptionwheel:adaptation_wheel",
        "adaptionwheel:mahoraga_wheel",
        "adaptionwheel:mahoraga_wheel_wood",
        // Runtime test fallback (vanilla item) when content mod item is unavailable.
        "minecraft:compass",
    ]
}

fn is_wheel_item_id(item_id: &str) -> bool {
    wheel_item_ids()
        .iter()
        .any(|id| item_id == *id || item_id.ends_with(":adaptation_wheel"))
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum WheelSource {
    #[default]
    None,
    Inventory,
    MainHand,
    OffHand,
    Armor,
    Hotbar,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AdaptationEntry {
    level: u8,
    progress: f32,
    completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlayerAdaptationStateV1 {
    schema_version: u32,
    wheel_active: bool,
    active_source: WheelSource,
    entries: BTreeMap<String, AdaptationEntry>,
    /// Runtime-only: mark cache as needing persistence flush (not serialized).
    #[serde(skip)]
    dirty_persist: bool,
}

impl Default for PlayerAdaptationStateV1 {
    fn default() -> Self {
        let mut entries = BTreeMap::new();
        for c in ALL_CONCEPTS {
            entries.insert(c.to_string(), AdaptationEntry::default());
        }
        Self {
            schema_version: STATE_SCHEMA_VERSION,
            wheel_active: false,
            active_source: WheelSource::None,
            entries,
            dirty_persist: false,
        }
    }
}

impl PlayerAdaptationStateV1 {
    fn normalize(&mut self) {
        self.schema_version = STATE_SCHEMA_VERSION;
        for c in ALL_CONCEPTS {
            self.entries.entry(c.to_string()).or_default();
        }
    }

    fn entry_mut(&mut self, concept: &str) -> &mut AdaptationEntry {
        self.entries.entry(concept.to_string()).or_default()
    }

    fn concept_level(&self, concept: &str) -> u8 {
        self.entries.get(concept).map(|e| e.level).unwrap_or(0)
    }

    #[allow(dead_code)]
    fn concept_completed(&self, concept: &str) -> bool {
        self.entries.get(concept).map(|e| e.completed).unwrap_or(false)
    }

    fn summary(&self) -> String {
        let fall = self.entries.get(CONCEPT_FALL).cloned().unwrap_or_default();
        let expl = self.entries.get(CONCEPT_EXPLOSION).cloned().unwrap_or_default();
        format!(
            "wheel={} ({:?}) | fall lvl={} prog={:.1}/{} done={} | explosion lvl={}/{} prog={:.1} done={} | fire lvl={} | lava lvl={} | general lvl={} | drowning lvl={} | contact lvl={}",
            self.wheel_active,
            self.active_source,
            fall.level,
            fall.progress,
            FALL_THRESHOLD,
            fall.completed,
            expl.level,
            EXPLOSION_MAX_LEVEL_MVP,
            expl.progress,
            expl.completed,
            self.concept_level(CONCEPT_FIRE),
            self.concept_level(CONCEPT_LAVA),
            self.concept_level(CONCEPT_GENERAL),
            self.concept_level(CONCEPT_DROWNING),
            self.concept_level(CONCEPT_CONTACT),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActivationScan {
    active: bool,
    source: WheelSource,
}

/// Scan 41-slot inventory snapshot from `player_inventory_changed`:
/// 0..36 main inventory (0..9 hotbar), 36..40 armor (feet, legs, chest, head), 40 offhand.
fn activation_from_inventory_stacks(stacks: &[Option<impl HasItemId>]) -> ActivationScan {
    for (idx, slot) in stacks.iter().enumerate() {
        let Some(stack) = slot else { continue };
        let item = stack.item_id();
        if !is_wheel_item_id(&item) {
            continue;
        }
        let source = if idx <= 8 {
            WheelSource::Hotbar
        } else if idx <= 35 {
            WheelSource::Inventory
        } else if idx <= 39 {
            WheelSource::Armor
        } else {
            WheelSource::OffHand
        };
        return ActivationScan {
            active: true,
            source,
        };
    }
    ActivationScan {
        active: false,
        source: WheelSource::None,
    }
}

trait HasItemId {
    fn item_id(&self) -> String;
}

impl HasItemId for horizon_client::mod_api::ItemStack {
    fn item_id(&self) -> String {
        self.item.to_string()
    }
}

fn states() -> &'static Mutex<HashMap<u128, PlayerAdaptationStateV1>> {
    static STATES: OnceLock<Mutex<HashMap<u128, PlayerAdaptationStateV1>>> = OnceLock::new();
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn with_player_state<R>(player: u128, f: impl FnOnce(&mut PlayerAdaptationStateV1) -> R) -> R {
    let mut map = states().lock().unwrap_or_else(|e| e.into_inner());
    let state = map.entry(player).or_insert_with(PlayerAdaptationStateV1::default);
    state.normalize();
    f(state)
}

fn get_player_state(player: u128) -> PlayerAdaptationStateV1 {
    let map = states().lock().unwrap_or_else(|e| e.into_inner());
    map.get(&player).cloned().unwrap_or_default()
}

fn set_player_activation(player: u128, scan: ActivationScan) -> bool {
    with_player_state(player, |state| {
        let changed = state.wheel_active != scan.active || state.active_source != scan.source;
        state.wheel_active = scan.active;
        state.active_source = scan.source;
        if changed {
            state.dirty_persist = true;
        }
        changed
    })
}

// --- Attachment-backed persistence (best-effort, in-memory cache remains runtime source of truth) ---

fn encode_player_state_nbt(state: &PlayerAdaptationStateV1) -> Result<Compound, AttachmentError> {
    let json = serde_json::to_string(state).map_err(|_| AttachmentError::Encode)?;
    let mut c = Compound::new();
    c.insert("v".into(), Tag::Int(STATE_SCHEMA_VERSION as i32));
    c.insert("json".into(), Tag::String(json.as_str().into()));
    Ok(c)
}

fn decode_player_state_nbt(compound: &Compound) -> Result<PlayerAdaptationStateV1, AttachmentError> {
    let Some(Tag::String(json)) = compound.get(&"json".into()) else {
        return Err(AttachmentError::Decode);
    };
    let s = json.to_string().map_err(|_| AttachmentError::Decode)?;
    let mut state: PlayerAdaptationStateV1 =
        serde_json::from_str(&s).map_err(|_| AttachmentError::Decode)?;
    state.normalize();
    state.dirty_persist = false;
    Ok(state)
}

static PLAYER_STATE_ATTACHMENT: AttachmentType<PlayerAdaptationStateV1> = AttachmentType {
    namespace: "adaptionwheel",
    name: "player_state",
    default: PlayerAdaptationStateV1::default,
    codec: Some(AttachmentCodec {
        encode: encode_player_state_nbt,
        decode: decode_player_state_nbt,
    }),
    copy_on_death: true,
    sync: false,
};

fn load_player_state_from_attachment(host: &mut dyn Host, player: u128) -> Option<PlayerAdaptationStateV1> {
    PLAYER_STATE_ATTACHMENT
        .get_existing(host, Holder::Player(player))
        .ok()
        .flatten()
        .map(|mut s| {
            s.normalize();
            s.dirty_persist = false;
            s
        })
}

fn save_player_state_to_attachment(host: &mut dyn Host, player: u128, state: &PlayerAdaptationStateV1) {
    let mut to_save = state.clone();
    to_save.normalize();
    to_save.dirty_persist = false;
    let _ = PLAYER_STATE_ATTACHMENT.set(host, Holder::Player(player), to_save);
}

fn sync_cache_from_attachment_if_present(host: &mut dyn Host, player: u128) {
    if let Some(loaded) = load_player_state_from_attachment(host, player) {
        with_player_state(player, |state| {
            *state = loaded;
        });
    } else {
        // Ensure attachment exists with current/default state for future persistence.
        let current = get_player_state(player);
        save_player_state_to_attachment(host, player, &current);
    }
}

fn persist_player_state_if_needed(host: &mut dyn Host, player: u128, force: bool) {
    let mut should_save = force;
    let state = with_player_state(player, |state| {
        if state.dirty_persist {
            should_save = true;
            state.dirty_persist = false;
        }
        state.clone()
    });
    if should_save {
        save_player_state_to_attachment(host, player, &state);
    }
}

// --- Entity id -> player uuid mapping (for hooks that provide entity id but not uuid/host) ---

fn entity_player_map() -> &'static Mutex<HashMap<u32, u128>> {
    static MAP: OnceLock<Mutex<HashMap<u32, u128>>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

fn refresh_entity_player_map_from_host(host: &dyn Host) {
    let players = host.server_players();
    let mut map = entity_player_map().lock().unwrap_or_else(|e| e.into_inner());
    map.clear();
    for p in players {
        if p.entity_id > 0 {
            map.insert(p.entity_id as u32, p.uuid);
        }
    }
}

fn player_uuid_for_entity(entity: u32) -> Option<u128> {
    entity_player_map()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&entity)
        .copied()
}

// --- Concepts, thresholds, mitigation ---

fn concept_max_level(concept: &str) -> u8 {
    match concept {
        CONCEPT_FALL => 1,
        CONCEPT_EXPLOSION => EXPLOSION_MAX_LEVEL_MVP,
        CONCEPT_FIRE => FIRE_MAX_LEVEL,
        CONCEPT_LAVA => LAVA_MAX_LEVEL,
        CONCEPT_GENERAL => GENERAL_MAX_LEVEL,
        CONCEPT_DROWNING => DROWNING_MAX_LEVEL,
        CONCEPT_CONTACT => CONTACT_MAX_LEVEL,
        _ => 1,
    }
}

fn leveled_threshold(concept: &str, level: u8) -> f32 {
    let l = level as f32;
    match concept {
        CONCEPT_EXPLOSION => 30.0 + l * 10.0,
        CONCEPT_FIRE => 20.0 + l * 10.0,
        CONCEPT_LAVA => 24.0 + l * 12.0,
        CONCEPT_GENERAL => 30.0 + l * 15.0,
        CONCEPT_DROWNING => 10.0 + l * 6.0,
        CONCEPT_CONTACT => 12.0 + l * 6.0,
        _ => 20.0 + l * 10.0,
    }
}

fn leveled_reduction(concept: &str, level: u8) -> f32 {
    match concept {
        CONCEPT_EXPLOSION => match level { 0 => 0.0, 1 => 0.10, 2 => 0.20, _ => 0.35 },
        CONCEPT_FIRE => match level { 0 => 0.0, 1 => 0.15, 2 => 0.30, _ => 0.45 },
        CONCEPT_LAVA => match level { 0 => 0.0, 1 => 0.20, 2 => 0.35, _ => 0.50 },
        CONCEPT_GENERAL => match level { 0 => 0.0, 1 => 0.10, 2 => 0.20, _ => 0.30 },
        CONCEPT_DROWNING => match level { 0 => 0.0, 1 => 0.25, 2 => 0.50, _ => 0.75 },
        CONCEPT_CONTACT => match level { 0 => 0.0, 1 => 0.20, 2 => 0.35, _ => 0.50 },
        _ => 0.0,
    }
}

#[allow(dead_code)]
fn explosion_threshold(level: u8) -> f32 { leveled_threshold(CONCEPT_EXPLOSION, level) }
#[allow(dead_code)]
fn explosion_reduction(level: u8) -> f32 { leveled_reduction(CONCEPT_EXPLOSION, level) }

fn is_explosion_damage_type(damage_type: &str) -> bool {
    let t = damage_type.to_ascii_lowercase();
    t.contains("explosion") || t.contains("blast") || t.contains("fireworks") || t.contains("bad_respawn_point")
}

fn is_fall_damage_type(damage_type: &str) -> bool {
    damage_type.to_ascii_lowercase().contains("fall")
}

fn is_lava_damage_type(damage_type: &str) -> bool {
    let t = damage_type.to_ascii_lowercase();
    t == "lava" || t.ends_with(":lava") || t.contains("lava")
}

fn is_fire_damage_type(damage_type: &str) -> bool {
    let t = damage_type.to_ascii_lowercase();
    // lava treated separately; "hot_floor" (magma) treated as fire-like
    matches!(t.as_str(), "fire" | "in_fire" | "on_fire" | "hot_floor" | "campfire" | "fireball" | "unattributed_fireball")
        || t.ends_with(":fire")
        || t.ends_with(":in_fire")
        || t.ends_with(":on_fire")
        || t.ends_with(":hot_floor")
        || t.ends_with(":campfire")
        || t.ends_with(":fireball")
        || t.ends_with(":unattributed_fireball")
        || (t.contains("fire") && !t.contains("firework"))
}

fn is_drowning_damage_type(damage_type: &str) -> bool {
    let t = damage_type.to_ascii_lowercase();
    t == "drown" || t.ends_with(":drown") || t.contains("drown")
}

fn is_contact_damage_type(damage_type: &str) -> bool {
    let t = damage_type.to_ascii_lowercase();
    matches!(t.as_str(), "cactus" | "sweet_berry_bush" | "stalagmite" | "contact")
        || t.ends_with(":cactus")
        || t.ends_with(":sweet_berry_bush")
        || t.ends_with(":stalagmite")
        || t.contains("cactus")
        || t.contains("sweet_berry_bush")
        || t.contains("stalagmite")
}

/// Primary concept for a damage event (used for progress/mitigation). Fall is handled by living_fall hook.
fn classify_damage_concept(damage_type: &str) -> Option<&'static str> {
    if is_explosion_damage_type(damage_type) {
        Some(CONCEPT_EXPLOSION)
    } else if is_fall_damage_type(damage_type) {
        Some(CONCEPT_FALL) // fallback if delivered via damage hook
    } else if is_lava_damage_type(damage_type) {
        Some(CONCEPT_LAVA)
    } else if is_fire_damage_type(damage_type) {
        Some(CONCEPT_FIRE)
    } else if is_drowning_damage_type(damage_type) {
        Some(CONCEPT_DROWNING)
    } else if is_contact_damage_type(damage_type) {
        Some(CONCEPT_CONTACT)
    } else {
        Some(CONCEPT_GENERAL)
    }
}

fn apply_leveled_progress(
    state: &mut PlayerAdaptationStateV1,
    concept: &str,
    amount: f32,
    max_level: u8,
    gain_cap: f32,
) -> bool {
    if !state.wheel_active {
        return false;
    }
    let entry = state.entry_mut(concept);
    if entry.completed || entry.level >= max_level {
        entry.completed = true;
        entry.progress = 0.0;
        return false;
    }
    let before = (entry.level, entry.progress, entry.completed);
    entry.progress += amount.max(0.5).min(gain_cap);
    let mut threshold = leveled_threshold(concept, entry.level);
    while entry.progress >= threshold && entry.level < max_level {
        entry.progress -= threshold;
        entry.level += 1;
        if entry.level >= max_level {
            entry.completed = true;
            entry.progress = 0.0;
            break;
        }
        threshold = leveled_threshold(concept, entry.level);
    }
    if entry.level >= max_level {
        entry.completed = true;
        entry.progress = 0.0;
    }
    let changed = before.0 != entry.level
        || (before.1 - entry.progress).abs() > f32::EPSILON
        || before.2 != entry.completed;
    if changed { state.dirty_persist = true; }
    changed
}

fn mitigation_factor_for_concept(state: &PlayerAdaptationStateV1, concept: &str) -> f32 {
    if concept == CONCEPT_FALL {
        return fall_mitigation_factor(state);
    }
    let level = state.concept_level(concept);
    let reduction = leveled_reduction(concept, level);
    (1.0 - reduction).clamp(0.0, 1.0)
}

// --- Existing/compat functions (used by tests and hooks) ---

fn apply_fall_progress(state: &mut PlayerAdaptationStateV1, distance_or_damage: f32) -> bool {
    if !state.wheel_active {
        return false;
    }
    let entry = state.entry_mut(CONCEPT_FALL);
    if entry.completed || entry.level >= 1 {
        entry.completed = true;
        return false;
    }
    let gain = distance_or_damage.clamp(1.0, FALL_MAX_GAIN_PER_EVENT);
    let before = entry.progress;
    entry.progress = (entry.progress + gain).min(FALL_THRESHOLD);
    if entry.progress >= FALL_THRESHOLD {
        entry.progress = FALL_THRESHOLD;
        entry.level = 1;
        entry.completed = true;
    }
    let changed = (entry.progress - before).abs() > f32::EPSILON || entry.completed;
    if changed { state.dirty_persist = true; }
    changed
}

fn fall_mitigation_factor(state: &PlayerAdaptationStateV1) -> f32 {
    if state
        .entries
        .get(CONCEPT_FALL)
        .map(|e| e.completed || e.level >= 1)
        .unwrap_or(false)
    {
        0.0
    } else {
        1.0
    }
}

fn apply_explosion_progress(state: &mut PlayerAdaptationStateV1, amount: f32) -> bool {
    apply_leveled_progress(state, CONCEPT_EXPLOSION, amount, EXPLOSION_MAX_LEVEL_MVP, 20.0)
}

fn apply_damage_adaptation_for_player(
    player_uuid: u128,
    damage_type: &str,
    amount: &mut f32,
) {
    let Some(concept) = classify_damage_concept(damage_type) else { return; };
    // Fall is primarily handled by living_fall; if it arrives as damage, still apply fall adaptation.
    with_player_state(player_uuid, |state| {
        let factor = mitigation_factor_for_concept(state, concept);
        if *amount > 0.0 {
            *amount *= factor;
        }
        match concept {
            CONCEPT_FALL => {
                let _ = apply_fall_progress(state, (*amount).max(1.0));
            }
            CONCEPT_EXPLOSION => {
                let _ = apply_explosion_progress(state, *amount);
            }
            CONCEPT_FIRE => {
                let _ = apply_leveled_progress(state, CONCEPT_FIRE, *amount, FIRE_MAX_LEVEL, 12.0);
            }
            CONCEPT_LAVA => {
                let _ = apply_leveled_progress(state, CONCEPT_LAVA, *amount, LAVA_MAX_LEVEL, 12.0);
            }
            CONCEPT_DROWNING => {
                let _ = apply_leveled_progress(state, CONCEPT_DROWNING, *amount, DROWNING_MAX_LEVEL, 8.0);
            }
            CONCEPT_CONTACT => {
                let _ = apply_leveled_progress(state, CONCEPT_CONTACT, *amount, CONTACT_MAX_LEVEL, 10.0);
            }
            CONCEPT_GENERAL => {
                let _ = apply_leveled_progress(state, CONCEPT_GENERAL, *amount, GENERAL_MAX_LEVEL, 15.0);
            }
            _ => {}
        }
    });
}

// --- Commands/UI helpers ---

fn format_status(state: &PlayerAdaptationStateV1) -> String {
    let mut parts = vec![format!(
        "wheel_active={} source={:?} schema={}",
        state.wheel_active, state.active_source, state.schema_version
    )];
    for c in ALL_CONCEPTS {
        let e = state.entries.get(c).cloned().unwrap_or_default();
        let max = concept_max_level(c);
        let extra = if c == CONCEPT_FALL {
            format!(" prog={:.2}/{}", e.progress, FALL_THRESHOLD)
        } else if e.level < max && !e.completed {
            format!(" prog={:.2}/{:.2}", e.progress, leveled_threshold(c, e.level))
        } else {
            String::new()
        };
        parts.push(format!(
            "{}: lvl={}/{}{} done={}",
            c, e.level, max, extra, e.completed
        ));
    }
    parts.join(" | ")
}

fn normalize_concept(input: &str) -> Option<&'static str> {
    match input.to_ascii_lowercase().as_str() {
        "env_fall" | "fall" => Some(CONCEPT_FALL),
        "type_explosion" | "explosion" => Some(CONCEPT_EXPLOSION),
        "env_fire" | "fire" => Some(CONCEPT_FIRE),
        "env_lava" | "lava" => Some(CONCEPT_LAVA),
        "type_general" | "general" | "generic" | "damage" => Some(CONCEPT_GENERAL),
        "env_drowning" | "drowning" | "drown" => Some(CONCEPT_DROWNING),
        "type_contact" | "contact" | "cactus" => Some(CONCEPT_CONTACT),
        _ => None,
    }
}

fn clamp_fall_progress(v: f32) -> f32 { v.clamp(0.0, FALL_THRESHOLD) }

fn apply_set_command(
    state: &mut PlayerAdaptationStateV1,
    concept: &str,
    field: &str,
    value: f32,
) -> Result<String, String> {
    let field = field.to_ascii_lowercase();
    let max_level = concept_max_level(concept);
    let res = match concept {
        CONCEPT_FALL => {
            let e = state.entry_mut(CONCEPT_FALL);
            match field.as_str() {
                "level" => {
                    let lvl = value.max(0.0) as u8;
                    if lvl >= 1 {
                        e.level = 1;
                        e.completed = true;
                        e.progress = FALL_THRESHOLD;
                    } else {
                        e.level = 0;
                        e.completed = false;
                        e.progress = e.progress.min(FALL_THRESHOLD - f32::EPSILON.max(0.0));
                    }
                    Ok(format!("env_fall level set to {}", e.level))
                }
                "progress" => {
                    e.progress = clamp_fall_progress(value);
                    e.completed = e.progress >= FALL_THRESHOLD || e.level >= 1;
                    if e.completed {
                        e.level = 1;
                        e.progress = FALL_THRESHOLD;
                    } else {
                        e.level = 0;
                    }
                    Ok(format!("env_fall progress set to {:.2}", e.progress))
                }
                _ => Err("field must be 'level' or 'progress'".to_string()),
            }
        }
        CONCEPT_EXPLOSION | CONCEPT_FIRE | CONCEPT_LAVA | CONCEPT_GENERAL | CONCEPT_DROWNING | CONCEPT_CONTACT => {
            let e = state.entry_mut(concept);
            match field.as_str() {
                "level" => {
                    let lvl = (value.max(0.0) as u8).min(max_level);
                    e.level = lvl;
                    e.completed = lvl >= max_level;
                    if e.completed {
                        e.progress = 0.0;
                    }
                    Ok(format!("{} level set to {}", concept, e.level))
                }
                "progress" => {
                    if e.level >= max_level {
                        e.completed = true;
                        e.progress = 0.0;
                        Ok(format!("{} already completed (level max)", concept))
                    } else {
                        let threshold = leveled_threshold(concept, e.level);
                        e.progress = value.clamp(0.0, threshold.max(0.0));
                        e.completed = false;
                        Ok(format!("{} progress set to {:.2}", concept, e.progress))
                    }
                }
                _ => Err("field must be 'level' or 'progress'".to_string()),
            }
        }
        _ => Err("unsupported concept".to_string()),
    };
    if res.is_ok() {
        state.dirty_persist = true;
    }
    res
}

fn apply_grant_command(
    state: &mut PlayerAdaptationStateV1,
    concept: &str,
    level: u8,
) -> Result<String, String> {
    let max_level = concept_max_level(concept);
    let res = match concept {
        CONCEPT_FALL => {
            let e = state.entry_mut(CONCEPT_FALL);
            if level >= 1 {
                e.level = 1;
                e.completed = true;
                e.progress = FALL_THRESHOLD;
                Ok("granted env_fall level 1 (completed)".to_string())
            } else {
                e.level = 0;
                e.completed = false;
                e.progress = 0.0;
                Ok("set env_fall to level 0".to_string())
            }
        }
        CONCEPT_EXPLOSION | CONCEPT_FIRE | CONCEPT_LAVA | CONCEPT_GENERAL | CONCEPT_DROWNING | CONCEPT_CONTACT => {
            let lvl = level.min(max_level);
            let e = state.entry_mut(concept);
            e.level = lvl;
            e.completed = lvl >= max_level;
            e.progress = 0.0;
            Ok(format!("granted {} level {}", concept, lvl))
        }
        _ => Err("unsupported concept".to_string()),
    };
    if res.is_ok() {
        state.dirty_persist = true;
    }
    res
}

fn handle_adaptionwheel_command(player: &NativeServerPlayer, command: &str) -> (bool, String) {
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() || !parts[0].eq_ignore_ascii_case("adaptionwheel") {
        return (false, String::new());
    }
    let sub = parts.get(1).copied().unwrap_or("status").to_ascii_lowercase();
    match sub.as_str() {
        "status" => {
            let state = get_player_state(player.uuid);
            (true, format!("[{MOD_ID}] status: {}", format_status(&state)))
        }
        "activate" => {
            let state = get_player_state(player.uuid);
            (
                true,
                format!(
                    "[{MOD_ID}] activate: wheel_active={} source={:?} (cached from inventory hooks; move/change inventory to re-evaluate automatically)",
                    state.wheel_active, state.active_source
                ),
            )
        }
        "reset" => {
            with_player_state(player.uuid, |state| {
                *state = PlayerAdaptationStateV1::default();
                state.dirty_persist = true;
            });
            let state = get_player_state(player.uuid);
            (true, format!("[{MOD_ID}] reset done. {}", format_status(&state)))
        }
        "grant" => {
            let Some(concept_raw) = parts.get(2) else {
                return (true, format!("[{MOD_ID}] usage: /adaptionwheel grant <env_fall|type_explosion|env_fire|env_lava|type_general|env_drowning|type_contact> <level>"));
            };
            let Some(concept) = normalize_concept(concept_raw) else {
                return (true, format!("[{MOD_ID}] unknown concept '{}'. Use env_fall,type_explosion,env_fire,env_lava,type_general,env_drowning,type_contact", concept_raw));
            };
            let level = parts.get(3).and_then(|v| v.parse::<u8>().ok()).unwrap_or(1);
            let msg = with_player_state(player.uuid, |state| apply_grant_command(state, concept, level));
            match msg {
                Ok(m) => {
                    let state = get_player_state(player.uuid);
                    (true, format!("[{MOD_ID}] {}. {}", m, format_status(&state)))
                }
                Err(e) => (true, format!("[{MOD_ID}] {}", e)),
            }
        }
        "set" => {
            let Some(concept_raw) = parts.get(2) else {
                return (true, format!("[{MOD_ID}] usage: /adaptionwheel set <concept> <level|progress> <value>"));
            };
            let Some(field) = parts.get(3) else {
                return (true, format!("[{MOD_ID}] usage: /adaptionwheel set <concept> <level|progress> <value>"));
            };
            let Some(value_raw) = parts.get(4) else {
                return (true, format!("[{MOD_ID}] usage: /adaptionwheel set <concept> <level|progress> <value>"));
            };
            let Some(concept) = normalize_concept(concept_raw) else {
                return (true, format!("[{MOD_ID}] unknown concept '{}'. Use env_fall,type_explosion,env_fire,env_lava,type_general,env_drowning,type_contact", concept_raw));
            };
            let Ok(value) = value_raw.parse::<f32>() else {
                return (true, format!("[{MOD_ID}] invalid value '{}'", value_raw));
            };
            let msg = with_player_state(player.uuid, |state| apply_set_command(state, concept, field, value));
            match msg {
                Ok(m) => {
                    let state = get_player_state(player.uuid);
                    (true, format!("[{MOD_ID}] {}. {}", m, format_status(&state)))
                }
                Err(e) => (true, format!("[{MOD_ID}] {}", e)),
            }
        }
        "help" => (
            true,
            format!(
                "[{MOD_ID}] commands: /adaptionwheel status | /adaptionwheel activate | /adaptionwheel set <concept> <level|progress> <value> | /adaptionwheel grant <concept> <level> | /adaptionwheel reset (concepts: env_fall,type_explosion,env_fire,env_lava,type_general,env_drowning,type_contact)"
            ),
        ),
        _ => (
            true,
            format!(
                "[{MOD_ID}] unknown subcommand '{}'. Try: status, activate, set, grant, reset",
                sub
            ),
        ),
    }
}

fn handle_adaptionwheel_console_command(command: &str) -> (bool, String) {
    let trimmed = command.trim();
    if !trimmed.eq_ignore_ascii_case("adaptionwheel") && !trimmed.to_ascii_lowercase().starts_with("adaptionwheel ") {
        return (false, String::new());
    }

    let mut parts = trimmed.split_whitespace();
    let _ = parts.next();
    let sub = parts.next().unwrap_or("help");

    match sub.to_ascii_lowercase().as_str() {
        "help" => (
            true,
            format!(
                "[{MOD_ID}] console: help | status/reset/activate/set/grant require player context (use in-game as player or target-specific command when available)"
            ),
        ),
        "status" | "reset" | "activate" | "set" | "grant" => (
            true,
            format!(
                "[{MOD_ID}] '{}' requires player context and is not available from dedicated console in this build; run it as an in-game player command.",
                sub.to_ascii_lowercase()
            ),
        ),
        _ => (
            true,
            format!(
                "[{MOD_ID}] unknown subcommand '{}'. Console supports: help",
                sub
            ),
        ),
    }
}

fn tell_login_status(host: &mut dyn Host, event: &PlayerLoggedInEvent<'_>, state: &PlayerAdaptationStateV1) {
    host.tell(
        event.player,
        Text::literal(format!(
            "[{MOD_ID}] ready. activation: {} ({:?}). Wheel works when present anywhere in inventory.",
            state.wheel_active, state.active_source
        )),
    );
    host.tell(event.player, Text::literal(format!("[{MOD_ID}] {}", state.summary())));
}

pub fn hooks() -> Box<dyn Hooks> {
    Box::new(Mod)
}

#[derive(Default)]
struct Mod;

impl Hooks for Mod {
    fn register_commands(&self, _host: &mut dyn Host, event: &mut RegisterCommandsEvent<'_>) {
        // Operator/debug commands (level 2).
        event.commands.push(NativeCommand::op("adaptionwheel status", 2));
        event.commands.push(NativeCommand::op("adaptionwheel activate", 2));
        event.commands.push(NativeCommand::op("adaptionwheel reset", 2));
        event.commands.push(NativeCommand::op("adaptionwheel help", 2));
        event.commands.push(NativeCommand::op(
            "adaptionwheel set <concept:word> <field:word> <value:number>",
            2,
        ));
        event.commands.push(NativeCommand::op(
            "adaptionwheel grant <concept:word> <level:integer>",
            2,
        ));
    }

    fn perform_command(&self, host: &mut dyn Host, event: &mut PerformCommandEvent<'_>) -> Flow {
        let (handled, message) = handle_adaptionwheel_command(event.player, event.command);
        if !handled {
            return Flow::Continue;
        }
        // Persist any command-driven mutations (set/grant/reset) best-effort.
        persist_player_state_if_needed(host, event.player.uuid, true);
        host.feedback(Text::literal(message));
        Flow::Cancel
    }

    fn perform_console_command(&self, host: &mut dyn Host, event: &mut PerformConsoleCommandEvent<'_>) -> Flow {
        let (handled, message) = handle_adaptionwheel_console_command(event.command);
        if !handled {
            return Flow::Continue;
        }
        host.feedback(Text::literal(message));
        Flow::Cancel
    }

    fn player_logged_in(&self, host: &mut dyn Host, event: &mut PlayerLoggedInEvent<'_>) {
        refresh_entity_player_map_from_host(host);
        // Best-effort: load persisted attachment state into runtime cache; if missing, seed attachment.
        sync_cache_from_attachment_if_present(host, event.player);
        let state = get_player_state(event.player);
        tell_login_status(host, event, &state);
        persist_player_state_if_needed(host, event.player, false);
    }

    fn player_clone(&self, host: &mut dyn Host, event: &mut PlayerCloneEvent<'_>) {
        // Attachment has copy_on_death=true; reload copied state if available.
        sync_cache_from_attachment_if_present(host, event.player);
        with_player_state(event.player, |s| {
            s.normalize();
            s.dirty_persist = false;
        });
        persist_player_state_if_needed(host, event.player, false);
        let _ = event.was_death;
    }

    fn player_tick(&self, host: &mut dyn Host, _event: &mut PlayerTickEvent) {
        // Keep entity-id -> player-uuid map fresh for hooks that only provide entity ids.
        refresh_entity_player_map_from_host(host);
        // Best-effort persistence flush for mutations made in hooks without host access.
        // (Limited to online players known to server; cheap check per tick.)
        for p in host.server_players() {
            persist_player_state_if_needed(host, p.uuid, false);
        }
    }

    /// Inventory scan enables wheel activation anywhere in inventory.
    fn player_inventory_changed(&self, host: &mut dyn Host, event: &mut PlayerInventoryChangedEvent<'_>) {
        // Ensure cache reflects persisted state before applying activation changes.
        sync_cache_from_attachment_if_present(host, event.player);
        let scan = activation_from_inventory_stacks(event.stacks);
        let changed = set_player_activation(event.player, scan);
        if changed {
            let state = get_player_state(event.player);
            host.tell(
                event.player,
                Text::literal(format!(
                    "[{MOD_ID}] wheel activation: {} (source: {:?}, anywhere-in-inventory scan).",
                    state.wheel_active, state.active_source
                )),
            );
        }
        persist_player_state_if_needed(host, event.player, false);
    }

    /// Primary damage adaptation hook (has victim UUID): fire/lava/explosion/general/drowning/contact fallbacks.
    fn living_incoming_damage_v3(&self, event: &mut LivingIncomingDamageV3Event<'_>) -> Flow {
        let Some(player_uuid) = event.victim_uuid else {
            return Flow::Continue;
        };
        if event.amount <= 0.0 {
            return Flow::Continue;
        }
        // Only adapt for players we track (state exists or will be created lazily).
        // Server-authoritative progress/mitigation.
        apply_damage_adaptation_for_player(player_uuid, event.damage_type, &mut event.amount);
        if event.amount <= 0.0 {
            event.amount = 0.0;
        }
        Flow::Continue
    }

    /// Fall adaptation/mitigation fallback via fall hook (entity-id based).
    fn living_fall(&self, event: &mut LivingFallEvent<'_>) -> Flow {
        let Some(player_uuid) = player_uuid_for_entity(event.entity) else {
            return Flow::Continue;
        };
        let gain_base = (event.distance as f32).max(event.multiplier * 2.0).max(0.0);
        with_player_state(player_uuid, |state| {
            let factor = fall_mitigation_factor(state);
            if factor < 1.0 {
                event.multiplier *= factor;
                event.distance *= factor as f64;
            }
            if gain_base > 0.0 {
                let _ = apply_fall_progress(state, gain_base);
            }
        });
        Flow::Continue
    }

    /// Drowning adaptation/mitigation fallback via drown hook (entity-id based).
    fn living_drown(&self, event: &mut LivingDrownEvent<'_>) -> Flow {
        let Some(player_uuid) = player_uuid_for_entity(event.entity) else {
            return Flow::Continue;
        };
        if !event.is_drowning && event.damage_amount <= 0.0 {
            return Flow::Continue;
        }
        with_player_state(player_uuid, |state| {
            let factor = mitigation_factor_for_concept(state, CONCEPT_DROWNING);
            if event.damage_amount > 0.0 {
                event.damage_amount *= factor;
            }
            let gain = if event.damage_amount > 0.0 { event.damage_amount } else { 1.0 };
            let _ = apply_leveled_progress(state, CONCEPT_DROWNING, gain, DROWNING_MAX_LEVEL, 8.0);
        });
        Flow::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct MockStack {
        item: String,
    }
    impl HasItemId for MockStack {
        fn item_id(&self) -> String {
            self.item.clone()
        }
    }

    #[test]
    fn state_defaults_and_summary() {
        let s = PlayerAdaptationStateV1::default();
        assert!(s.entries.contains_key(CONCEPT_FALL));
        assert!(s.entries.contains_key(CONCEPT_EXPLOSION));
        assert!(s.entries.contains_key(CONCEPT_FIRE));
        assert!(s.entries.contains_key(CONCEPT_LAVA));
        assert!(s.entries.contains_key(CONCEPT_GENERAL));
        assert!(s.entries.contains_key(CONCEPT_DROWNING));
        assert!(s.entries.contains_key(CONCEPT_CONTACT));
        assert!(s.summary().contains("wheel=false"));
    }

    #[test]
    fn activation_detects_wheel_anywhere_in_inventory() {
        let mut slots: Vec<Option<MockStack>> = vec![None; 41];
        slots[12] = Some(MockStack { item: "adaptionwheel:adaptation_wheel".into() });
        let scan = activation_from_inventory_stacks(&slots);
        assert!(scan.active);
        assert_eq!(scan.source, WheelSource::Inventory);

        slots[12] = None;
        slots[0] = Some(MockStack { item: "adaptionwheel:adaptation_wheel".into() });
        let scan = activation_from_inventory_stacks(&slots);
        assert!(scan.active);
        assert_eq!(scan.source, WheelSource::Hotbar);

        slots[0] = None;
        slots[40] = Some(MockStack { item: "adaptionwheel:adaptation_wheel".into() });
        let scan = activation_from_inventory_stacks(&slots);
        assert!(scan.active);
        assert_eq!(scan.source, WheelSource::OffHand);

        slots[40] = None;
        let scan = activation_from_inventory_stacks(&slots);
        assert!(!scan.active);
        assert_eq!(scan.source, WheelSource::None);
    }

    #[test]
    fn fall_progress_and_mitigation() {
        let mut s = PlayerAdaptationStateV1::default();
        s.wheel_active = true;
        assert!(apply_fall_progress(&mut s, 12.0));
        assert!(apply_fall_progress(&mut s, 15.0));
        assert!(apply_fall_progress(&mut s, 20.0));
        assert!(s.entries[CONCEPT_FALL].completed);
        assert_eq!(fall_mitigation_factor(&s), 0.0);
    }

    #[test]
    fn fall_requires_activation() {
        let mut s = PlayerAdaptationStateV1::default();
        assert!(!apply_fall_progress(&mut s, 30.0));
        assert_eq!(s.entries[CONCEPT_FALL].progress, 0.0);
    }

    #[test]
    fn explosion_progress_levels_up() {
        let mut s = PlayerAdaptationStateV1::default();
        s.wheel_active = true;
        let mut changed = false;
        for _ in 0..15 {
            changed |= apply_explosion_progress(&mut s, 20.0);
        }
        assert!(changed);
        assert!(s.entries[CONCEPT_EXPLOSION].level >= 1);
    }

    #[test]
    fn explosion_damage_classifier() {
        assert!(is_explosion_damage_type("minecraft:explosion"));
        assert!(is_explosion_damage_type("minecraft:player_explosion"));
        assert!(!is_explosion_damage_type("minecraft:fall"));
        assert!(is_fall_damage_type("minecraft:fall"));
    }

    #[test]
    fn damage_type_classification_for_new_concepts() {
        assert_eq!(classify_damage_concept("minecraft:in_fire"), Some(CONCEPT_FIRE));
        assert_eq!(classify_damage_concept("minecraft:lava"), Some(CONCEPT_LAVA));
        assert_eq!(classify_damage_concept("minecraft:drown"), Some(CONCEPT_DROWNING));
        assert_eq!(classify_damage_concept("minecraft:cactus"), Some(CONCEPT_CONTACT));
        assert_eq!(classify_damage_concept("minecraft:mob_attack"), Some(CONCEPT_GENERAL));
        assert_eq!(classify_damage_concept("minecraft:explosion"), Some(CONCEPT_EXPLOSION));
    }

    #[test]
    fn leveled_concepts_progress_with_activation() {
        let mut s = PlayerAdaptationStateV1::default();
        s.wheel_active = true;
        let mut changed = false;
        for _ in 0..20 {
            changed |= apply_leveled_progress(&mut s, CONCEPT_FIRE, 10.0, FIRE_MAX_LEVEL, 12.0);
            changed |= apply_leveled_progress(&mut s, CONCEPT_LAVA, 10.0, LAVA_MAX_LEVEL, 12.0);
            changed |= apply_leveled_progress(&mut s, CONCEPT_GENERAL, 10.0, GENERAL_MAX_LEVEL, 15.0);
            changed |= apply_leveled_progress(&mut s, CONCEPT_CONTACT, 8.0, CONTACT_MAX_LEVEL, 10.0);
            changed |= apply_leveled_progress(&mut s, CONCEPT_DROWNING, 6.0, DROWNING_MAX_LEVEL, 8.0);
        }
        assert!(changed);
        assert!(s.concept_level(CONCEPT_FIRE) >= 1);
        assert!(s.concept_level(CONCEPT_LAVA) >= 1);
        assert!(s.concept_level(CONCEPT_GENERAL) >= 1);
        assert!(s.concept_level(CONCEPT_CONTACT) >= 1);
        assert!(s.concept_level(CONCEPT_DROWNING) >= 1);
    }

    #[test]
    fn commands_set_and_grant_new_concepts() {
        let mut s = PlayerAdaptationStateV1::default();
        assert!(apply_set_command(&mut s, CONCEPT_FIRE, "progress", 15.0).is_ok());
        assert!(s.entries[CONCEPT_FIRE].progress > 0.0);
        assert!(apply_grant_command(&mut s, CONCEPT_LAVA, 2).is_ok());
        assert_eq!(s.concept_level(CONCEPT_LAVA), 2);
        assert!(apply_set_command(&mut s, CONCEPT_CONTACT, "level", 3.0).is_ok());
        assert!(s.concept_completed(CONCEPT_CONTACT));
        assert!(normalize_concept("drown").is_some());
        assert!(normalize_concept("general").is_some());
    }

    #[test]
    fn damage_adaptation_applies_mitigation_and_progress() {
        let player = 42u128;
        with_player_state(player, |s| {
            *s = PlayerAdaptationStateV1::default();
            s.wheel_active = true;
            let e = s.entry_mut(CONCEPT_FIRE);
            e.level = 2;
        });
        let mut amount = 10.0f32;
        apply_damage_adaptation_for_player(player, "minecraft:in_fire", &mut amount);
        assert!(amount < 10.0); // mitigation from level 2
        let s = get_player_state(player);
        assert!(s.entries[CONCEPT_FIRE].progress >= 0.0);
    }
}
