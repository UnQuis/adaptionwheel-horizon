//! Player adaptation state cache, attachment-backed persistence and entity-id mapping.
//!
//! Ported as-is from the frozen v1 `native/adaptation/api/src/lib.rs`
//! (sections `states`/`with_player_state`/`get_player_state`/`set_player_activation`,
//! the JSON-in-NBT attachment codec and persistence helpers, and the
//! entity-id -> player-uuid map). The persistence implementation worked on the
//! old build; this is a straight API migration, not a redesign.

use mod_api::{
    AttachmentCodec, AttachmentError, AttachmentType, Compound, Holder, Registrar,
    RegistrationContext, ServerHost, Tag,
};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::engine_impl::{ActivationScan, PlayerAdaptationStateV1, STATE_SCHEMA_VERSION};

// --- In-memory state cache (runtime source of truth) ---

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

/// `AttachmentType<T>` has no separate registration step in the new API: it is a
/// self-describing static key declared once and used through `get`/`set`/`has`/
/// `get_existing` with a host and a `Holder`. See
/// `crates/horizon-mod-runtime/src/attachments.rs:364-372` (fields) and
/// `docs/modding/src/concepts/attachments.md:7-26` (declaration + usage).
pub static PLAYER_STATE_ATTACHMENT: AttachmentType<PlayerAdaptationStateV1> = AttachmentType {
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

pub fn load_player_state_from_attachment(
    host: &mut dyn ServerHost,
    player: u128,
) -> Option<PlayerAdaptationStateV1> {
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

pub fn save_player_state_to_attachment(
    host: &mut dyn ServerHost,
    player: u128,
    state: &PlayerAdaptationStateV1,
) {
    let mut to_save = state.clone();
    to_save.normalize();
    to_save.dirty_persist = false;
    let _ = PLAYER_STATE_ATTACHMENT.set(host, Holder::Player(player), to_save);
}

pub fn sync_cache_from_attachment_if_present(host: &mut dyn ServerHost, player: u128) {
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

pub fn persist_player_state_if_needed(host: &mut dyn ServerHost, player: u128, force: bool) {
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
    let mut map = entity_player_map().lock().unwrap_or_else(|e| e.into_inner());
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
pub fn register_attachments(_r: &mut dyn Registrar, _cx: &RegistrationContext) {}
