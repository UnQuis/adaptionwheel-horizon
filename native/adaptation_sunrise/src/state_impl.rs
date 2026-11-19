//! Player adaptation state cache, attachment-backed persistence and entity-id mapping.
//!
//! Ported from the frozen v1 `native/adaptation/api/src/lib.rs` and extended for
//! the schema-v2 model in [`crate::state_v2`]:
//!
//! * The **authoritative persisted value** is now [`PlayerAdaptionV2`], stored in
//!   an attachment with the JSON envelope `{"v":2,"json":<serde V2>}`. The codec
//!   still accepts a legacy `v:1` envelope and migrates its inner JSON through
//!   [`migrate_v1_json`], so pre-existing saves keep working.
//! * The **legacy v1 in-memory cache** (`states()` / `with_player_state` / ...) is
//!   kept with its original signatures because the whole v1 engine (`engine_impl`,
//!   `commands_impl`, `hooks_impl`) reads and writes it. It is seeded from V2 on
//!   load and merged back into V2 on save.
//! * A parallel **V2 cache** (`states_v2()` / `with_v2_state` / `get_v2_state`)
//!   holds the P1 fields (Extras / history / tasks / adversity timers) that the
//!   v1 model cannot represent.

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, Compound, Holder, Registrar,
    RegistrationContext, ServerHost, Tag,
};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::engine_impl::{ActivationScan, PlayerAdaptationStateV1, WheelSource};
use crate::state_v2::{
    PlayerAdaptionV2, SCHEMA_VERSION_V2, decode_v2, encode_v2, migrate_v1_json, p1_concept_id,
};

// --- In-memory state cache (runtime source of truth for the v1 engine) ---

/// Legacy v1 cache. Kept with its original signature: the v1 engine reads/writes
/// this directly. It is reconstructed from V2 on load and merged back into V2
/// on save (see [`v1_from_v2`] / [`merge_v1_into_v2`]).
pub fn states() -> &'static Mutex<HashMap<u128, PlayerAdaptationStateV1>> {
    static STATES: OnceLock<Mutex<HashMap<u128, PlayerAdaptationStateV1>>> = OnceLock::new();
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn with_player_state<R>(player: u128, f: impl FnOnce(&mut PlayerAdaptationStateV1) -> R) -> R {
    let mut map = states().lock().unwrap_or_else(|e| e.into_inner());
    let state = map
        .entry(player)
        .or_insert_with(PlayerAdaptationStateV1::default);
    state.normalize();
    f(state)
}

pub fn get_player_state(player: u128) -> PlayerAdaptationStateV1 {
    let map = states().lock().unwrap_or_else(|e| e.into_inner());
    map.get(&player).cloned().unwrap_or_default()
}

pub fn set_player_activation(player: u128, scan: ActivationScan) -> bool {
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

// --- Extended v2 cache (P1 fields the v1 model cannot represent) ---

/// Authoritative schema-v2 cache. Holds Extras / history / tasks / adversity
/// timers plus the mirrored concept levels/adapted set.
pub fn states_v2() -> &'static Mutex<HashMap<u128, PlayerAdaptionV2>> {
    static STATES_V2: OnceLock<Mutex<HashMap<u128, PlayerAdaptionV2>>> = OnceLock::new();
    STATES_V2.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn with_v2_state<R>(player: u128, f: impl FnOnce(&mut PlayerAdaptionV2) -> R) -> R {
    let mut map = states_v2().lock().unwrap_or_else(|e| e.into_inner());
    let state = map.entry(player).or_insert_with(PlayerAdaptionV2::default);
    state.normalize();
    f(state)
}

pub fn get_v2_state(player: u128) -> PlayerAdaptionV2 {
    let map = states_v2().lock().unwrap_or_else(|e| e.into_inner());
    map.get(&player).cloned().unwrap_or_default()
}

/// Seed the V2 cache from the current legacy-v1 cache entry.
///
/// The v1 state is serialized to its JSON form and passed through
/// [`migrate_v1_json`], so the exact same v1 -> v2 mapping is used here as in
/// the attachment codec. This overwrites any existing V2 entry for `player`;
/// call it only when no authoritative V2 state is available (e.g. a player with
/// no attachment, or a legacy save that has not been loaded yet).
pub fn seed_v2_from_v1(player: u128) {
    let v1 = get_player_state(player);
    let Ok(json) = serde_json::to_string(&v1) else {
        return;
    };
    let v2 = migrate_v1_json(&json);
    with_v2_state(player, |state| {
        *state = v2;
        state.dirty = false;
    });
}

// --- V2 <-> V1 bridges ---

/// Reverse of [`p1_concept_id`]: upstream-style concept id -> frozen v1 key.
/// Only the seven P1 concepts are representable in the v1 model; everything
/// else has no legacy slot and is dropped from the legacy cache.
fn reverse_concept_id(concept: &str) -> Option<&'static str> {
    match concept {
        "Env_FallDamage" | "env_fall" | "fall" => Some("env_fall"),
        "Env_Fire" | "env_fire" | "fire" => Some("env_fire"),
        "Env_Lava" | "env_lava" | "lava" => Some("env_lava"),
        "Env_Drowning" | "env_drowning" | "drowning" | "drown" => Some("env_drowning"),
        "Type_EXPLOSION" | "type_explosion" | "explosion" => Some("type_explosion"),
        "Contact_CACTUS" | "type_contact" | "contact" | "cactus" => Some("type_contact"),
        "Type_OTHER" | "type_general" | "general" | "generic" | "damage" => Some("type_general"),
        _ => None,
    }
}

/// Parse the serialized `WheelSource` provenance string back into the enum.
fn parse_wheel_source(source: &str) -> WheelSource {
    match source {
        "inventory" => WheelSource::Inventory,
        "main_hand" => WheelSource::MainHand,
        "off_hand" => WheelSource::OffHand,
        "armor" => WheelSource::Armor,
        "hotbar" => WheelSource::Hotbar,
        _ => WheelSource::None,
    }
}

/// Mirror of `state_v2::legacy_max_level`, used when deciding whether a v1
/// entry counts as fully adapted. Fall is one-time (max 1); the rest cap at 3.
fn legacy_max_level(key: &str) -> u8 {
    if key == "env_fall" { 1 } else { 3 }
}

/// Reconstruct a legacy v1 state from an authoritative V2 state.
///
/// `levels` and `adapted` are mapped back through [`reverse_concept_id`] so the
/// old engine functions (`concept_level`, `concept_completed`, mitigation, ...)
/// see the same progress as V2. V2-only fields (Extras/history/tasks/adversity
/// timers) have no v1 slot and intentionally stay in the V2 cache.
fn v1_from_v2(v2: &PlayerAdaptionV2) -> PlayerAdaptationStateV1 {
    let mut v1 = PlayerAdaptationStateV1::default(); // seeds all seven concepts
    v1.wheel_active = v2.legacy_wheel_active;
    v1.active_source = parse_wheel_source(&v2.legacy_active_source);

    for (id, &level) in &v2.levels {
        if let Some(key) = reverse_concept_id(id) {
            let entry = v1.entry_mut(key);
            if level > entry.level {
                entry.level = level;
            }
        }
    }
    for id in &v2.adapted {
        if let Some(key) = reverse_concept_id(id) {
            let entry = v1.entry_mut(key);
            entry.completed = true;
            // An acquired concept implies full v1 progress so mitigation works.
            let max = legacy_max_level(key);
            if entry.level < max {
                entry.level = max;
            }
        }
    }

    v1.normalize();
    v1.dirty_persist = false;
    v1
}

/// Merge legacy v1 progress back into a V2 state before it is persisted.
///
/// The engine/command layer writes levels/completed into the v1 cache; this is
/// the only path by which that progress reaches the V2 attachment. Existing V2
/// levels are never lowered (`max` wins), and V2-only fields are left untouched
/// so Extras/history/tasks/adversity timers survive the round trip.
fn merge_v1_into_v2(v1: &PlayerAdaptationStateV1, v2: &mut PlayerAdaptionV2) {
    for (key, entry) in &v1.entries {
        let id = p1_concept_id(key);
        if entry.completed || entry.level >= legacy_max_level(key) {
            v2.adapted.insert(id.to_string());
        } else if entry.level > 0 {
            let slot = v2.levels.entry(id.to_string()).or_insert(0);
            if entry.level > *slot {
                *slot = entry.level;
            }
        }
    }
}

// --- Attachment-backed persistence (best-effort, caches remain runtime source of truth) ---

/// Encode a V2 state into the persisted NBT envelope `{"v":2,"json":<serde V2>}`.
fn encode_player_state_nbt(state: &PlayerAdaptionV2) -> Result<Compound, AttachmentError> {
    let json = encode_v2(state);
    let mut c = Compound::new();
    c.insert("v".into(), Tag::Int(SCHEMA_VERSION_V2 as i32));
    c.insert("json".into(), Tag::String(json.as_str().into()));
    Ok(c)
}

/// Decode the persisted envelope into a V2 state.
///
/// A `v:1` envelope (or JSON that does not parse as V2) is migrated through
/// [`migrate_v1_json`]; a `v:2` envelope with valid JSON is decoded via
/// [`decode_v2`]. The serde-skipped `dirty` marker is cleared on load.
fn decode_player_state_nbt(compound: &Compound) -> Result<PlayerAdaptionV2, AttachmentError> {
    let version = match compound.get(&"v".into()) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    };
    let Some(Tag::String(json)) = compound.get(&"json".into()) else {
        return Err(AttachmentError::Decode);
    };
    let s = json.to_string().map_err(|_| AttachmentError::Decode)?;

    let mut state = if version == 1 || serde_json::from_str::<PlayerAdaptionV2>(&s).is_err() {
        migrate_v1_json(&s)
    } else {
        decode_v2(&s)
    };
    state.normalize();
    state.dirty = false;
    Ok(state)
}

/// `AttachmentType<T>` has no separate registration step in the new API: it is a
/// self-describing static key declared once and used through `get`/`set`/`has`/
/// `get_existing` with a host and a `Holder`. See
/// `crates/horizon-mod-runtime/src/attachments.rs:364-372` (fields) and
/// `docs/modding/src/concepts/attachments.md:7-26` (declaration + usage).
pub static PLAYER_STATE_ATTACHMENT: AttachmentType<PlayerAdaptionV2> = AttachmentType {
    namespace: "adaptionwheel",
    name: "player_state",
    default: PlayerAdaptionV2::default,
    codec: Some(AttachmentCodec {
        encode: encode_player_state_nbt,
        decode: decode_player_state_nbt,
    }),
    copy_on_death: true,
    sync: false,
};

/// Load the authoritative V2 state from the attachment (if any).
fn load_v2_from_attachment(host: &mut dyn ServerHost, player: u128) -> Option<PlayerAdaptionV2> {
    PLAYER_STATE_ATTACHMENT
        .get_existing(host, Holder::Player(player))
        .ok()
        .flatten()
}

/// Legacy signature kept intact: returns the v1 reconstruction of the loaded V2
/// attachment. Callers that need the full V2 value use
/// [`load_v2_from_attachment`] via [`sync_cache_from_attachment_if_present`].
#[allow(dead_code)]
pub fn load_player_state_from_attachment(
    host: &mut dyn ServerHost,
    player: u128,
) -> Option<PlayerAdaptationStateV1> {
    load_v2_from_attachment(host, player).map(|v2| v1_from_v2(&v2))
}

/// Persist a v1 snapshot, merging it into the current V2 state so old concept
/// progress survives. Start from the cached V2 value so V2-only fields survive;
/// best-effort: `AttachmentError` is ignored as before (runtime evidence proved
/// persistence works).
pub fn save_player_state_to_attachment(
    host: &mut dyn ServerHost,
    player: u128,
    state: &PlayerAdaptationStateV1,
) {
    let mut to_save = get_v2_state(player); // cached V2: preserves Extras/history/tasks/timers
    merge_v1_into_v2(state, &mut to_save);
    to_save.normalize();
    to_save.dirty = false;
    let _ = PLAYER_STATE_ATTACHMENT.set(host, Holder::Player(player), to_save);
}

/// True when either cache still holds progress that has not been flushed to the
/// attachment. Reads the maps directly (no `entry` insert / `normalize`) so a
/// mere check does not create default entries.
fn caches_have_unflushed_progress(player: u128) -> bool {
    let v1_dirty = {
        let map = states().lock().unwrap_or_else(|e| e.into_inner());
        map.get(&player).map(|s| s.dirty_persist).unwrap_or(false)
    };
    let v2_dirty = {
        let map = states_v2().lock().unwrap_or_else(|e| e.into_inner());
        map.get(&player).map(|s| s.dirty).unwrap_or(false)
    };
    v1_dirty || v2_dirty
}

/// Called on login / clone. Populate BOTH caches from the attachment:
/// the V2 cache from the decoded/migrated value, and the legacy v1 cache
/// reconstructed from V2 `levels`/`adapted` via [`reverse_concept_id`].
///
/// Regression guard: a damage/fall/drown hook can advance the cache and mark it
/// dirty earlier in the same tick, before an inventory/clone/login event fires
/// this sync. Reloading then would overwrite that newer progress with the older
/// attachment value. We therefore SKIP the reload entirely while either cache
/// is dirty; the following `persist_player_state_if_needed` flushes the newer
/// state to the attachment instead.
pub fn sync_cache_from_attachment_if_present(host: &mut dyn ServerHost, player: u128) {
    if caches_have_unflushed_progress(player) {
        return;
    }
    if let Some(v2) = load_v2_from_attachment(host, player) {
        with_v2_state(player, |state| {
            *state = v2.clone();
            state.dirty = false;
        });
        let v1 = v1_from_v2(&v2);
        with_player_state(player, |state| {
            *state = v1;
        });
    } else {
        // Ensure the attachment exists with the current/default state for future
        // persistence: seed V2 from the legacy cache, then save the merge.
        seed_v2_from_v1(player);
        let current = get_player_state(player);
        save_player_state_to_attachment(host, player, &current);
    }
}

/// Flush dirty caches to the attachment. Both the V2 `dirty` flag and the
/// legacy `dirty_persist` marker are consumed as flush triggers; either one (or
/// `force`) causes a save. Errors remain best-effort and are ignored.
pub fn persist_player_state_if_needed(host: &mut dyn ServerHost, player: u128, force: bool) {
    let mut should_save = force;
    let v1_snapshot = with_player_state(player, |state| {
        if state.dirty_persist {
            should_save = true;
            state.dirty_persist = false;
        }
        state.clone()
    });
    with_v2_state(player, |state| {
        if state.dirty {
            should_save = true;
            state.dirty = false;
        }
    });
    if should_save {
        save_player_state_to_attachment(host, player, &v1_snapshot);
    }
}

// --- Entity id -> player uuid mapping (for hooks that provide entity id but not uuid/host) ---

pub fn entity_player_map() -> &'static Mutex<HashMap<u32, u128>> {
    static MAP: OnceLock<Mutex<HashMap<u32, u128>>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Port note: the v1 `host.server_players()` accessor moved to the `Players`
/// host service — `ServerHost::players() -> &dyn Players`, then
/// `Players::server_players() -> Vec<PlayerRef>`
/// (`crates/horizon-mod-vocab/src/host/services.rs:26,214-225`).
/// `PlayerRef::{entity, uuid}` and `EntityRef::id` are plain value methods
/// (`crates/horizon-mod-vocab/src/entity/types.rs:1091-1100,145-160`), so no
/// host is needed for the read and the signature keeps `&dyn ServerHost`.
pub fn refresh_entity_player_map_from_host(host: &dyn ServerHost) {
    let mut map = entity_player_map()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    map.clear();
    for player in host.players().server_players() {
        let entity_id = player.entity().id();
        if entity_id > 0 {
            map.insert(entity_id, player.uuid());
        }
    }
}

pub fn player_uuid_for_entity(entity: u32) -> Option<u128> {
    entity_player_map()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&entity)
        .copied()
}

/// No-op: the new API needs no attachment-type registration. `AttachmentType<T>`
/// is a static key with `(namespace, name, default, codec, copy_on_death, sync)`;
/// it is used directly through its `get`/`set`/`has`/`get_existing` methods.
/// `Hooks::register(&self, r: &mut dyn Registrar, cx: &RegistrationContext)`
/// has no attachment entry point (`mod_api_generated.rs:1895`), and the
/// runtime's `AttachmentRegistration` (`attachments.rs:708-723`) is an internal
/// handle for indexed attachment maps, not a mod-facing registration step.
/// Kept so `lib.rs::Hooks::register` can call it uniformly with other subsystems.
pub fn register_attachments(r: &mut dyn Registrar, _cx: &RegistrationContext) {
    // Sunrise refuses undeclared attachment ids (AttachmentError::Undeclared);
    // Horizon accepted an implicit declaration, which is why the no-op worked
    // there but silently dropped every save here.
    let _ = r.register_attachment_type("adaptionwheel:player_state", false);
}

// No in-process `ServerHost` mock is needed: the persistence path is exercised
// directly through the codec functions (`encode_player_state_nbt` /
// `decode_player_state_nbt`) and the pure V2<->V1 bridge helpers, which are the
// same code paths the host-backed `set`/`get_existing` call into. Mocking the
// broad `ServerHost` trait would add a large fake that tests none of the codec
// or migration logic these tests target.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_v2::TaskRecord;

    #[test]
    fn codec_roundtrip_preserves_all_v2_fields() {
        let mut s = PlayerAdaptionV2::default();
        s.levels.insert("Env_Fire".to_string(), 2);
        s.levels.insert("Type_EXPLOSION".to_string(), 3);
        s.adapted.insert("Env_FallDamage".to_string());
        s.tasks.push(TaskRecord::new("Env_Lava", 3.0, 10.0));
        s.push_history("Env_Fire");
        s.push_history("Env_Lava");
        s.extras.instabreak = true;
        s.extras.selected_ability = Some("flight".to_string());
        s.extras.disabled.push("Env_Lava".to_string());
        s.extras.progress.insert("fist_mining_0".to_string(), 12.0);
        s.adversity_timer = 7;
        s.adversity_cooldown = 9;
        s.adversity_active = true;
        s.healing_timer = 5;
        s.dirty = true;

        let c = encode_player_state_nbt(&s).unwrap();
        assert_eq!(c.get(&"v".into()), Some(&Tag::Int(2)));

        let back = decode_player_state_nbt(&c).unwrap();
        assert_eq!(back.schema_version, SCHEMA_VERSION_V2);
        assert_eq!(back.levels, s.levels);
        assert_eq!(back.adapted, s.adapted);
        assert_eq!(back.tasks, s.tasks);
        assert_eq!(back.history, s.history);
        assert_eq!(back.extras, s.extras);
        assert_eq!(back.adversity_timer, 7);
        assert_eq!(back.adversity_cooldown, 9);
        assert!(back.adversity_active);
        assert_eq!(back.healing_timer, 5);
        // dirty is serde-skipped and cleared on decode.
        assert!(!back.dirty);
    }

    #[test]
    fn codec_v1_envelope_migrates_and_seeds_legacy_cache() {
        let v1_json = r#"{
            "schema_version": 1,
            "wheel_active": true,
            "active_source": "inventory",
            "entries": {
                "env_fall": {"level": 1, "progress": 40.0, "completed": true},
                "env_fire": {"level": 2, "progress": 5.0, "completed": false}
            }
        }"#;
        let mut c = Compound::new();
        c.insert("v".into(), Tag::Int(1));
        c.insert("json".into(), Tag::String(v1_json.into()));

        let v2 = decode_player_state_nbt(&c).unwrap();
        assert_eq!(v2.schema_version, SCHEMA_VERSION_V2);
        assert!(v2.adapted.contains("Env_FallDamage"));
        assert_eq!(v2.levels.get("Env_Fire"), Some(&2));
        assert!(v2.legacy_wheel_active);
        assert_eq!(v2.legacy_active_source, "inventory");

        // Legacy cache seeding from the migrated V2.
        let v1 = v1_from_v2(&v2);
        assert!(v1.wheel_active);
        assert_eq!(v1.active_source, WheelSource::Inventory);
        assert!(v1.entries.get("env_fall").unwrap().completed);
        assert_eq!(v1.entries.get("env_fall").unwrap().level, 1);
        assert_eq!(v1.entries.get("env_fire").unwrap().level, 2);
        assert!(!v1.dirty_persist);
    }

    #[test]
    fn codec_malformed_v2_json_falls_back_to_migration() {
        let mut c = Compound::new();
        c.insert("v".into(), Tag::Int(2));
        c.insert("json".into(), Tag::String("not json at all {".into()));
        let v2 = decode_player_state_nbt(&c).unwrap();
        assert_eq!(v2.schema_version, SCHEMA_VERSION_V2);
        assert!(v2.levels.is_empty());
        assert!(v2.adapted.is_empty());

        // A missing json tag is a hard decode error.
        let mut bad = Compound::new();
        bad.insert("v".into(), Tag::Int(2));
        assert!(matches!(
            decode_player_state_nbt(&bad),
            Err(AttachmentError::Decode)
        ));
    }

    #[test]
    fn merge_v1_into_v2_preserves_progress_and_v2_extras() {
        let mut v1 = PlayerAdaptationStateV1::default();
        v1.entry_mut("env_fire").level = 3;
        v1.entry_mut("env_fall").completed = true;
        v1.entry_mut("type_explosion").level = 1; // lower than V2 -> must not regress
        v1.entry_mut("env_lava").level = 2;

        let mut v2 = PlayerAdaptionV2::default();
        v2.levels.insert("Type_EXPLOSION".to_string(), 3);
        v2.extras.instabreak = true;
        v2.extras.selected_ability = Some("flight".to_string());
        v2.push_history("Env_Lava");
        v2.adversity_timer = 11;
        v2.tasks.push(TaskRecord::new("Env_Drowning", 1.0, 10.0));

        merge_v1_into_v2(&v1, &mut v2);

        // A v1 entry at its concept max counts as fully adapted (same rule as
        // migrate_v1_json), so it lands in `adapted`, not `levels`.
        assert!(v2.adapted.contains("Env_Fire"));
        assert!(v2.adapted.contains("Env_FallDamage"));
        assert_eq!(v2.levels.get("Env_Lava"), Some(&2));
        assert_eq!(v2.levels.get("Type_EXPLOSION"), Some(&3)); // V2 value kept
        // V2-only fields survive the merge.
        assert!(v2.extras.instabreak);
        assert_eq!(v2.extras.selected_ability.as_deref(), Some("flight"));
        assert_eq!(v2.adversity_timer, 11);
        assert_eq!(v2.history, vec!["Env_Lava"]);
        assert_eq!(v2.tasks.len(), 1);
    }

    #[test]
    fn seed_v2_from_v1_populates_the_v2_cache() {
        // Unique id so this test cannot collide with other cache-using tests.
        let player = 0x0100_0000_0000_0001u128;
        with_player_state(player, |s| {
            s.entry_mut("type_explosion").level = 2;
            s.entry_mut("env_lava").completed = true;
            s.wheel_active = true;
            s.active_source = WheelSource::Hotbar;
        });

        seed_v2_from_v1(player);
        let v2 = get_v2_state(player);
        assert_eq!(v2.levels.get("Type_EXPLOSION"), Some(&2));
        assert!(v2.adapted.contains("Env_Lava"));
        assert!(v2.legacy_wheel_active);
        assert_eq!(v2.legacy_active_source, "hotbar");
        assert!(!v2.dirty);
    }

    #[test]
    fn sync_preserves_dirty_progress_instead_of_reloading() {
        // A FakeHost with a real in-memory attachment store: pre-seed an OLDER
        // value so a reload would clobber same-tick progress.
        let mut host = mod_api::testing::FakeHost::new();
        let player = 0x0200_0000_0000_0002u128;
        let mut older = PlayerAdaptionV2::default();
        older.levels.insert("Env_Fire".to_string(), 1);
        let h: &mut dyn mod_api::ServerHost = &mut host;
        PLAYER_STATE_ATTACHMENT
            .set(h, Holder::Player(player), older)
            .unwrap();

        // Simulate a damage hook advancing progress earlier in the same tick,
        // marking BOTH caches dirty (the bug the reviewer found).
        with_player_state(player, |s| {
            s.entry_mut("env_fire").level = 3;
            s.wheel_active = true;
            s.dirty_persist = true;
        });
        with_v2_state(player, |s| {
            s.extras.instabreak = true;
            s.dirty = true;
        });

        // A later inventory/clone/login sync must NOT overwrite the dirty caches.
        sync_cache_from_attachment_if_present(h, player);

        let v1 = get_player_state(player);
        assert_eq!(v1.entries.get("env_fire").unwrap().level, 3);
        assert!(v1.wheel_active);
        assert!(v1.dirty_persist); // still pending flush, untouched

        let v2 = get_v2_state(player);
        assert!(v2.extras.instabreak);
        assert!(v2.dirty); // still pending flush, untouched
        // V2 was NOT reloaded from the older attachment value.
        assert!(!v2.levels.contains_key("Env_Fire"));
    }

    #[test]
    fn reverse_concept_id_covers_all_p1_concepts() {
        assert_eq!(reverse_concept_id("Env_FallDamage"), Some("env_fall"));
        assert_eq!(reverse_concept_id("Env_Fire"), Some("env_fire"));
        assert_eq!(reverse_concept_id("Env_Lava"), Some("env_lava"));
        assert_eq!(reverse_concept_id("Env_Drowning"), Some("env_drowning"));
        assert_eq!(reverse_concept_id("Type_EXPLOSION"), Some("type_explosion"));
        assert_eq!(reverse_concept_id("Contact_CACTUS"), Some("type_contact"));
        assert_eq!(reverse_concept_id("Type_OTHER"), Some("type_general"));
        assert_eq!(reverse_concept_id("Unknown_Concept"), None);
        // Forward and reverse agree for every P1 concept.
        for key in [
            "env_fall",
            "env_fire",
            "env_lava",
            "env_drowning",
            "type_explosion",
            "type_contact",
            "type_general",
        ] {
            assert_eq!(reverse_concept_id(p1_concept_id(key)), Some(key));
        }
    }
}
