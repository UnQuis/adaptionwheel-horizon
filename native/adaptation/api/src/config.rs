//! MVP configuration and constants.
//! Server-authoritative defaults; values may later be sourced from Horizon config APIs.

use crate::state::ConceptId;

#[derive(Debug, Clone)]
pub struct MvpConfig {
    pub activation_mode: ActivationMode,
    /// Coalesce progress sync updates (ticks); completion/activation sync immediately.
    pub sync_progress_interval_ticks: u64,
    /// Fallback activation recheck interval (ticks) if no inventory-change hook exists.
    pub activation_recheck_interval_ticks: u64,
    pub scenarios: ScenarioTuning,
}

impl Default for MvpConfig {
    fn default() -> Self {
        Self {
            activation_mode: ActivationMode::Offhand,
            sync_progress_interval_ticks: 20,
            activation_recheck_interval_ticks: 10,
            scenarios: ScenarioTuning::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationMode {
    Offhand,
    OffhandOrMainHand,
}

#[derive(Debug, Clone)]
pub struct ScenarioTuning {
    pub fall: FallTuning,
    pub fire: FireTuning,
}

impl Default for ScenarioTuning {
    fn default() -> Self {
        Self {
            fall: FallTuning::default(),
            fire: FireTuning::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FallTuning {
    pub concept: ConceptId,
    /// Accumulated fall "analysis points" required (e.g. sum of effective fall distance/damage).
    pub threshold: f32,
    /// Max gain per event to avoid instant completion from extreme values.
    pub max_gain_per_event: f32,
    /// Damage multiplier when completed (1.0 = unchanged, 0.1 = strong mitigation).
    pub mitigation_multiplier: f32,
}

impl Default for FallTuning {
    fn default() -> Self {
        Self {
            concept: ConceptId::EnvFall,
            threshold: 20.0,
            max_gain_per_event: 8.0,
            mitigation_multiplier: 0.1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FireTuning {
    pub concept: ConceptId,
    /// Accumulated fire exposure points required (damage/events normalized).
    pub threshold: f32,
    pub max_gain_per_event: f32,
    /// Damage multiplier when completed for fire-burning damage types.
    pub mitigation_multiplier: f32,
}

impl Default for FireTuning {
    fn default() -> Self {
        Self {
            concept: ConceptId::EnvFire,
            threshold: 12.0,
            max_gain_per_event: 4.0,
            mitigation_multiplier: 0.2,
        }
    }
}

/// Global MVP constants.
pub const STATE_SCHEMA_VERSION: u32 = 1;
pub const PROTOCOL_VERSION: u16 = 1;
pub const WHEEL_ITEM_ID: &str = "adaptionwheel:mahoraga_wheel"; // content id TBD by hmod registration
pub const WHEEL_ITEM_ID_WOOD: &str = "adaptionwheel:mahoraga_wheel_wood";
