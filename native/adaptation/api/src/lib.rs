//! AdaptionWheel Horizon native mod - inventory activation update (Horizon 26.3-era APIs).
//!
//! Key update: wheel activation works when the wheel item is present anywhere in the player's
//! inventory (hotbar/main inventory/offhand/armor slots) using server-side inventory hooks,
//! preferring `player_inventory_changed` (41-slot snapshot: 0..36 main incl. hotbar, 36..40 armor,
//! 40 offhand) when available on the target Horizon build.
//!
//! Server-authoritative MVP logic retained:
//! - per-player adaptation state (schema v1, in-memory cache; attachment persistence TODO/wiring)
//! - fall progress/mitigation (`env_fall`)
//! - explosion damage progress/mitigation (`type_explosion`, classified by damage type)

use horizon_client::mod_api::{
    Flow, Hooks, Host, LivingFallEvent, LivingIncomingDamageEvent, NativeCommand,
    NativeServerPlayer, PerformCommandEvent, PlayerCloneEvent, PlayerInventoryChangedEvent,
    PlayerLoggedInEvent, RegisterCommandsEvent, Text,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, OnceLock};

const MOD_ID: &str = "adaptionwheel";
const STATE_SCHEMA_VERSION: u32 = 1;

const CONCEPT_FALL: &str = "env_fall";
const CONCEPT_EXPLOSION: &str = "type_explosion";

const FALL_THRESHOLD: f32 = 40.0;
const FALL_MAX_GAIN_PER_EVENT: f32 = 20.0;
const EXPLOSION_MAX_LEVEL_MVP: u8 = 3;

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
}

impl Default for PlayerAdaptationStateV1 {
    fn default() -> Self {
        let mut entries = BTreeMap::new();
        entries.insert(CONCEPT_FALL.to_string(), AdaptationEntry::default());
        entries.insert(CONCEPT_EXPLOSION.to_string(), AdaptationEntry::default());
        Self {
            schema_version: STATE_SCHEMA_VERSION,
            wheel_active: false,
            active_source: WheelSource::None,
            entries,
        }
    }
}

impl PlayerAdaptationStateV1 {
    fn normalize(&mut self) {
        self.schema_version = STATE_SCHEMA_VERSION;
        self.entries.entry(CONCEPT_FALL.to_string()).or_default();
        self.entries
            .entry(CONCEPT_EXPLOSION.to_string())
            .or_default();
    }

    fn entry_mut(&mut self, concept: &str) -> &mut AdaptationEntry {
        self.entries.entry(concept.to_string()).or_default()
    }

    fn summary(&self) -> String {
        let fall = self.entries.get(CONCEPT_FALL).cloned().unwrap_or_default();
        let expl = self
            .entries
            .get(CONCEPT_EXPLOSION)
            .cloned()
            .unwrap_or_default();
        format!(
            "wheel={} ({:?}) | fall lvl={} prog={:.1}/{} done={} | explosion lvl={}/{} prog={:.1} done={}",
            self.wheel_active,
            self.active_source,
            fall.level,
            fall.progress,
            FALL_THRESHOLD,
            fall.completed,
            expl.level,
            EXPLOSION_MAX_LEVEL_MVP,
            expl.progress,
            expl.completed
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

// Horizon ItemStack in mod_api typically exposes `item` (identifier/name) and `count`.
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
        changed
    })
}

fn format_status(state: &PlayerAdaptationStateV1) -> String {
    let fall = state.entries.get(CONCEPT_FALL).cloned().unwrap_or_default();
    let expl = state
        .entries
        .get(CONCEPT_EXPLOSION)
        .cloned()
        .unwrap_or_default();
    format!(
        "wheel_active={} source={:?} schema={} | env_fall: level={} progress={:.2}/{} completed={} | type_explosion: level={}/{} progress={:.2} completed={}",
        state.wheel_active,
        state.active_source,
        state.schema_version,
        fall.level,
        fall.progress,
        FALL_THRESHOLD,
        fall.completed,
        expl.level,
        EXPLOSION_MAX_LEVEL_MVP,
        expl.progress,
        expl.completed
    )
}

fn normalize_concept(input: &str) -> Option<&'static str> {
    match input.to_ascii_lowercase().as_str() {
        "env_fall" | "fall" => Some(CONCEPT_FALL),
        "type_explosion" | "explosion" => Some(CONCEPT_EXPLOSION),
        _ => None,
    }
}

fn clamp_fall_progress(v: f32) -> f32 {
    v.clamp(0.0, FALL_THRESHOLD)
}

fn apply_set_command(
    state: &mut PlayerAdaptationStateV1,
    concept: &str,
    field: &str,
    value: f32,
) -> Result<String, String> {
    let field = field.to_ascii_lowercase();
    match concept {
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
        CONCEPT_EXPLOSION => {
            let e = state.entry_mut(CONCEPT_EXPLOSION);
            match field.as_str() {
                "level" => {
                    let lvl = (value.max(0.0) as u8).min(EXPLOSION_MAX_LEVEL_MVP);
                    e.level = lvl;
                    e.completed = lvl >= EXPLOSION_MAX_LEVEL_MVP;
                    if e.completed {
                        e.progress = 0.0;
                    }
                    Ok(format!("type_explosion level set to {}", e.level))
                }
                "progress" => {
                    if e.level >= EXPLOSION_MAX_LEVEL_MVP {
                        e.completed = true;
                        e.progress = 0.0;
                        Ok("type_explosion already completed (level max)".to_string())
                    } else {
                        let threshold = explosion_threshold(e.level);
                        e.progress = value.clamp(0.0, threshold.max(0.0));
                        e.completed = false;
                        Ok(format!("type_explosion progress set to {:.2}", e.progress))
                    }
                }
                _ => Err("field must be 'level' or 'progress'".to_string()),
            }
        }
        _ => Err("unsupported concept (use env_fall or type_explosion)".to_string()),
    }
}

fn apply_grant_command(
    state: &mut PlayerAdaptationStateV1,
    concept: &str,
    level: u8,
) -> Result<String, String> {
    match concept {
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
        CONCEPT_EXPLOSION => {
            let lvl = level.min(EXPLOSION_MAX_LEVEL_MVP);
            let e = state.entry_mut(CONCEPT_EXPLOSION);
            e.level = lvl;
            e.completed = lvl >= EXPLOSION_MAX_LEVEL_MVP;
            e.progress = 0.0;
            Ok(format!("granted type_explosion level {}", lvl))
        }
        _ => Err("unsupported concept (use env_fall or type_explosion)".to_string()),
    }
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
            // Re-evaluation from inventory requires inventory snapshot/API not available in this hook.
            // Report current cached activation (updated by inventory hooks) and allow manual refresh hint.
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
            });
            let state = get_player_state(player.uuid);
            (true, format!("[{MOD_ID}] reset done. {}", format_status(&state)))
        }
        "grant" => {
            let Some(concept_raw) = parts.get(2) else {
                return (true, format!("[{MOD_ID}] usage: /adaptionwheel grant <env_fall|type_explosion> <level>"));
            };
            let Some(concept) = normalize_concept(concept_raw) else {
                return (true, format!("[{MOD_ID}] unknown concept '{}'. Use env_fall or type_explosion.", concept_raw));
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
            // /adaptionwheel set <concept> <level|progress> <value>
            let Some(concept_raw) = parts.get(2) else {
                return (true, format!("[{MOD_ID}] usage: /adaptionwheel set <env_fall|type_explosion> <level|progress> <value>"));
            };
            let Some(field) = parts.get(3) else {
                return (true, format!("[{MOD_ID}] usage: /adaptionwheel set <env_fall|type_explosion> <level|progress> <value>"));
            };
            let Some(value_raw) = parts.get(4) else {
                return (true, format!("[{MOD_ID}] usage: /adaptionwheel set <env_fall|type_explosion> <level|progress> <value>"));
            };
            let Some(concept) = normalize_concept(concept_raw) else {
                return (true, format!("[{MOD_ID}] unknown concept '{}'. Use env_fall or type_explosion.", concept_raw));
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
                "[{MOD_ID}] commands: /adaptionwheel status | /adaptionwheel activate | /adaptionwheel set <env_fall|type_explosion> <level|progress> <value> | /adaptionwheel grant <env_fall|type_explosion> <level> | /adaptionwheel reset"
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

fn explosion_threshold(level: u8) -> f32 {
    30.0 + (level as f32) * 10.0
}

fn explosion_reduction(level: u8) -> f32 {
    match level {
        0 => 0.0,
        1 => 0.10,
        2 => 0.20,
        _ => 0.35,
    }
}

fn is_explosion_damage_type(damage_type: &str) -> bool {
    let t = damage_type.to_ascii_lowercase();
    t.contains("explosion") || t.contains("blast") || t.contains("fireworks") || t.contains("bad_respawn_point")
}

fn is_fall_damage_type(damage_type: &str) -> bool {
    damage_type.to_ascii_lowercase().contains("fall")
}

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
    (entry.progress - before).abs() > f32::EPSILON || entry.completed
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
    if !state.wheel_active {
        return false;
    }
    let entry = state.entry_mut(CONCEPT_EXPLOSION);
    if entry.completed || entry.level >= EXPLOSION_MAX_LEVEL_MVP {
        entry.completed = true;
        entry.progress = 0.0;
        return false;
    }
    let before = (entry.level, entry.progress, entry.completed);
    entry.progress += amount.max(1.0).min(20.0);
    let mut threshold = explosion_threshold(entry.level);
    while entry.progress >= threshold && entry.level < EXPLOSION_MAX_LEVEL_MVP {
        entry.progress -= threshold;
        entry.level += 1;
        if entry.level >= EXPLOSION_MAX_LEVEL_MVP {
            entry.completed = true;
            entry.progress = 0.0;
            break;
        }
        threshold = explosion_threshold(entry.level);
    }
    if entry.level >= EXPLOSION_MAX_LEVEL_MVP {
        entry.completed = true;
        entry.progress = 0.0;
    }
    before.0 != entry.level
        || (before.1 - entry.progress).abs() > f32::EPSILON
        || before.2 != entry.completed
}

fn tell_login_status(host: &mut dyn Host, event: &PlayerLoggedInEvent<'_>, state: &PlayerAdaptationStateV1) {
    // Try to message the joining player (API details may vary by target build).
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
        // Operator/debug commands (level 2). Usage strings follow Horizon NativeCommand argument syntax.
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
        host.feedback(Text::literal(message));
        Flow::Cancel
    }

    fn player_logged_in(&self, host: &mut dyn Host, event: &mut PlayerLoggedInEvent<'_>) {
        let state = get_player_state(event.player);
        tell_login_status(host, event, &state);
    }

    fn player_clone(&self, _host: &mut dyn Host, event: &mut PlayerCloneEvent<'_>) {
        // Preserve in-memory state across respawn/clone (best-effort). Attachment persistence TODO.
        let mut state = get_player_state(event.player);
        state.normalize();
        with_player_state(event.player, |s| *s = state.clone());
        let _ = event.was_death;
    }

    /// Horizon 26.3-era server hook: fires when a player's inventory changes and provides 41 slots
    /// (0..36 main incl. hotbar, 36..40 armor feet->head, 40 offhand). This enables wheel activation
    /// when the wheel item is anywhere in inventory.
    fn player_inventory_changed(&self, host: &mut dyn Host, event: &mut PlayerInventoryChangedEvent<'_>) {
        let scan = activation_from_inventory_stacks(event.stacks);
        let changed = set_player_activation(event.player, scan.clone());
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
    }

    fn living_incoming_damage(&self, _event: &mut LivingIncomingDamageEvent<'_>) -> Flow {
        // Server-authoritative mitigation/progress wiring requires mapping damage event entity -> player UUID/state.
        // With player_inventory_changed-based activation now available, progress/mitigation application can be
        // connected here once player identity/context helpers are finalized for target Horizon API.
        Flow::Continue
    }

    fn living_fall(&self, event: &mut LivingFallEvent<'_>) -> Flow {
        // Fall mitigation/progress requires player identity mapping; activation state can now come from
        // anywhere-in-inventory detection via inventory hooks.
        let _ = event.distance;
        let _ = event.multiplier;
        let _ = event.kind;
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
}
