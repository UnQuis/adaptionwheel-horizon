//! Player adaptation state (Attachment schema v1).
//! Server is the source of truth; client keeps a mirrored copy for UI.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA_VERSION_V1: u32 = 1;

/// Stable scenario/concept identifiers for MVP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConceptId {
    EnvFall,
    EnvFire,
    // Reserved for later MVP expansion (kept in schema/protocol for forward planning):
    EnvLava,
    TypeGenericHostile,
    EnvDrowning,
}

impl ConceptId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EnvFall => "env_fall",
            Self::EnvFire => "env_fire",
            Self::EnvLava => "env_lava",
            Self::TypeGenericHostile => "type_generic_hostile",
            Self::EnvDrowning => "env_drowning",
        }
    }

    pub fn mvp_active_scenarios() -> [ConceptId; 2] {
        [ConceptId::EnvFall, ConceptId::EnvFire]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[repr(u8)]
pub enum WheelSourceTag {
    #[default]
    None = 0,
    MainHand = 1,
    OffHand = 2,
    Hotbar = 3,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct AdaptationEntryV1 {
    pub level: u8,
    pub progress: f32,
    pub completed: bool,
    pub cooldown_until_tick: u64,
    pub flags: u16,
}

impl AdaptationEntryV1 {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerAdaptationStateV1 {
    pub schema_version: u32,
    pub wheel_active: bool,
    pub active_source: WheelSourceTag,
    pub entries: BTreeMap<ConceptId, AdaptationEntryV1>,
    /// Runtime-only dirty marker (may be skipped by serializer in real impl).
    #[serde(skip)]
    pub dirty: bool,
    /// Runtime-only sync bookkeeping.
    #[serde(skip)]
    pub last_sync_tick: u64,
}

impl Default for PlayerAdaptationStateV1 {
    fn default() -> Self {
        let mut entries = BTreeMap::new();
        for c in ConceptId::mvp_active_scenarios() {
            entries.insert(c, AdaptationEntryV1::default());
        }
        Self {
            schema_version: SCHEMA_VERSION_V1,
            wheel_active: false,
            active_source: WheelSourceTag::None,
            entries,
            dirty: false,
            last_sync_tick: 0,
        }
    }
}

impl PlayerAdaptationStateV1 {
    pub fn entry(&self, concept: ConceptId) -> &AdaptationEntryV1 {
        self.entries.get(&concept).unwrap_or_else(|| {
            // Should not happen for MVP fixed set; callers can use ensure_entry_mut for safety.
            static EMPTY: std::sync::OnceLock<AdaptationEntryV1> = std::sync::OnceLock::new();
            EMPTY.get_or_init(AdaptationEntryV1::default)
        })
    }

    pub fn ensure_entry_mut(&mut self, concept: ConceptId) -> &mut AdaptationEntryV1 {
        self.entries.entry(concept).or_default()
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn clear_dirty(&mut self, tick: u64) {
        self.dirty = false;
        self.last_sync_tick = tick;
    }
}

/// Load/migrate persisted bytes into current schema.
/// `TBD(Horizon API)`: replace with actual attachment codec (NBT/bytes/serde integration).
pub fn migrate_state(raw: Option<&[u8]>) -> PlayerAdaptationStateV1 {
    let Some(raw) = raw else {
        return PlayerAdaptationStateV1::default();
    };
    match serde_json::from_slice::<PlayerAdaptationStateV1>(raw) {
        Ok(mut state) => {
            if state.schema_version == SCHEMA_VERSION_V1 {
                // Ensure MVP entries exist.
                for c in ConceptId::mvp_active_scenarios() {
                    state.entries.entry(c).or_default();
                }
                state
            } else if state.schema_version < SCHEMA_VERSION_V1 {
                // Future: stepwise migrations. For now reset safely.
                PlayerAdaptationStateV1::default()
            } else {
                // Newer schema than known: best-effort fallback.
                PlayerAdaptationStateV1::default()
            }
        }
        Err(_) => PlayerAdaptationStateV1::default(),
    }
}

pub fn encode_state(state: &PlayerAdaptationStateV1) -> Vec<u8> {
    serde_json::to_vec(state).unwrap_or_default()
}

/// Registration point: bind attachment type `adaptionwheel:player_state`.
/// `TBD(Horizon API @139e83fff8)`: call host/registry attachment registration with
/// serializer, default factory, and copy_on_death policy (desired: true for MVP).
pub fn register_attachment(_host: &mut dyn horizon_client::mod_api::Host) {
    // TODO: host.register_attachment(AttachmentSpec { id: "adaptionwheel:player_state", ... })
}
