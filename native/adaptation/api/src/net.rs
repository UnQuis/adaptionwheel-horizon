//! Mod messages protocol (Horizon mod messages API 40), server-authoritative sync.

use crate::config::PROTOCOL_VERSION;
use crate::state::{ConceptId, PlayerAdaptationStateV1, WheelSourceTag, SCHEMA_VERSION_V1};
use serde::{Deserialize, Serialize};

pub const CHANNEL_STATE_SYNC: &str = "adaptionwheel:state_sync";
pub const CHANNEL_STATE_ACK: &str = "adaptionwheel:state_ack"; // optional
pub const CHANNEL_CLIENT_EVENT: &str = "adaptionwheel:client_event"; // optional/reserved

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum MsgType {
    FullStateSync = 1,
    DeltaStateSync = 2,
    AdaptationEvent = 3,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Envelope<T> {
    pub proto_version: u16,
    pub msg_type: u8,
    pub payload: T,
}

impl<T> Envelope<T> {
    pub fn new(msg_type: MsgType, payload: T) -> Self {
        Self { proto_version: PROTOCOL_VERSION, msg_type: msg_type as u8, payload }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntrySnapshot {
    pub concept_id: ConceptId,
    pub level: u8,
    pub progress: f32,
    pub completed: bool,
}

impl EntrySnapshot {
    pub fn from_state(concept_id: ConceptId, state: &PlayerAdaptationStateV1) -> Self {
        let e = state.entry(concept_id);
        Self { concept_id, level: e.level, progress: e.progress, completed: e.completed }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FullStateSync {
    pub state_schema: u32,
    pub wheel_active: bool,
    pub active_source: WheelSourceTag,
    pub entries: Vec<EntrySnapshot>,
}

impl FullStateSync {
    pub fn from_state(state: &PlayerAdaptationStateV1) -> Self {
        let entries = state
            .entries
            .iter()
            .map(|(id, e)| EntrySnapshot {
                concept_id: *id,
                level: e.level,
                progress: e.progress,
                completed: e.completed,
            })
            .collect();
        Self {
            state_schema: state.schema_version.max(SCHEMA_VERSION_V1),
            wheel_active: state.wheel_active,
            active_source: state.active_source,
            entries,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DeltaStateSync {
    pub wheel_active: Option<bool>,
    pub active_source: Option<WheelSourceTag>,
    pub updated_entries: Vec<EntrySnapshot>,
    pub removed_entries: Vec<ConceptId>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum AdaptationEventKind {
    ProgressMilestone = 1,
    Completed = 2,
    ActivationChanged = 3,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdaptationEventMsg {
    pub kind: AdaptationEventKind,
    pub concept_id: Option<ConceptId>,
    pub level: Option<u8>,
    pub progress: Option<f32>,
}

/// Client-side mirror used by minimal UI indication.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClientAdaptationState {
    pub state_schema: u32,
    pub wheel_active: bool,
    pub active_source: WheelSourceTag,
    pub entries: Vec<EntrySnapshot>,
}

impl ClientAdaptationState {
    pub fn apply_full(&mut self, sync: FullStateSync) {
        self.state_schema = sync.state_schema;
        self.wheel_active = sync.wheel_active;
        self.active_source = sync.active_source;
        self.entries = sync.entries;
    }

    pub fn apply_delta(&mut self, sync: DeltaStateSync) {
        if let Some(v) = sync.wheel_active { self.wheel_active = v; }
        if let Some(v) = sync.active_source { self.active_source = v; }
        for upd in sync.updated_entries {
            if let Some(e) = self.entries.iter_mut().find(|e| e.concept_id == upd.concept_id) {
                *e = upd;
            } else {
                self.entries.push(upd);
            }
        }
        if !sync.removed_entries.is_empty() {
            self.entries.retain(|e| !sync.removed_entries.contains(&e.concept_id));
        }
    }
}

pub fn encode_full(state: &PlayerAdaptationStateV1) -> Vec<u8> {
    serde_json::to_vec(&Envelope::new(MsgType::FullStateSync, FullStateSync::from_state(state))).unwrap_or_default()
}

pub fn encode_delta(delta: DeltaStateSync) -> Vec<u8> {
    serde_json::to_vec(&Envelope::new(MsgType::DeltaStateSync, delta)).unwrap_or_default()
}

pub fn encode_event(event: AdaptationEventMsg) -> Vec<u8> {
    serde_json::to_vec(&Envelope::new(MsgType::AdaptationEvent, event)).unwrap_or_default()
}

/// Registration point: mod messages channels for API 40.
/// `TBD(Horizon API @139e83fff8)`: register codecs/handlers for CHANNEL_*.
pub fn register_channels(_host: &mut dyn horizon_client::mod_api::Host) {
    // TODO: host.register_mod_message_channel(CHANNEL_STATE_SYNC, Direction::ServerToClient, ...)
}

/// Server helper (TBD player handle type): send full sync to player.
pub fn send_full_sync_to_player(_host: &mut dyn horizon_client::mod_api::Host, _player: /* TBD */ (), _state: &PlayerAdaptationStateV1) {
    // TODO: host.send_mod_message(player, CHANNEL_STATE_SYNC, encode_full(state))
}

/// Server helper: send delta sync to player.
pub fn send_delta_sync_to_player(_host: &mut dyn horizon_client::mod_api::Host, _player: /* TBD */ (), _delta: DeltaStateSync) {
    // TODO: host.send_mod_message(player, CHANNEL_STATE_SYNC, encode_delta(delta))
}

/// Client handler stub: decode inbound state sync and update UI mirror.
pub fn on_client_message(_host: &mut dyn horizon_client::mod_api::Host, _raw: &[u8], client_state: &mut ClientAdaptationState) {
    // Minimal tolerant decode skeleton.
    #[derive(Deserialize)]
    struct AnyEnvelope { proto_version: u16, msg_type: u8, payload: serde_json::Value }
    let Ok(env) = serde_json::from_slice::<AnyEnvelope>(_raw) else { return; };
    if env.proto_version != PROTOCOL_VERSION { return; }
    match env.msg_type {
        x if x == MsgType::FullStateSync as u8 => {
            if let Ok(p) = serde_json::from_value::<FullStateSync>(env.payload) { client_state.apply_full(p); }
        }
        x if x == MsgType::DeltaStateSync as u8 => {
            if let Ok(p) = serde_json::from_value::<DeltaStateSync>(env.payload) { client_state.apply_delta(p); }
        }
        _ => {}
    }
}
