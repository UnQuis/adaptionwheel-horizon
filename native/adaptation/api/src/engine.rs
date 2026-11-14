//! Server-authoritative adaptation analysis engine (MVP).

use crate::config::MvpConfig;
use crate::scenarios::{ScenarioContext, ScenarioOutcome, ScenarioRegistry};
use crate::state::{PlayerAdaptationStateV1};

#[derive(Debug, Clone, Default)]
pub struct EngineTickResult {
    pub state_changed: bool,
    pub progress_changed: bool,
    pub completed_now: bool,
    pub sync_recommended: bool,
}

#[derive(Debug, Default)]
pub struct AdaptationEngine {
    pub registry: ScenarioRegistry,
}

impl AdaptationEngine {
    pub fn new_mvp(cfg: &MvpConfig) -> Self {
        Self {
            registry: ScenarioRegistry::new_mvp(cfg),
        }
    }

    /// Apply an incoming gameplay event/context to the player's state.
    /// Progress accumulation only occurs when wheel is active (server-authoritative).
    /// Mitigation decisions are derived from completed entries and may be queried separately.
    pub fn process_event(
        &self,
        cfg: &MvpConfig,
        state: &mut PlayerAdaptationStateV1,
        ctx: &ScenarioContext,
    ) -> EngineTickResult {
        let mut result = EngineTickResult::default();
        if !state.wheel_active {
            // Still allow mitigation checks elsewhere based on completed state; no progress gain.
            return result;
        }

        for scenario in self.registry.iter() {
            if let Some(outcome) = scenario.evaluate_and_apply(cfg, state, ctx) {
                match outcome {
                    ScenarioOutcome::NoChange => {}
                    ScenarioOutcome::Progress { .. } => {
                        result.state_changed = true;
                        result.progress_changed = true;
                        state.mark_dirty();
                    }
                    ScenarioOutcome::Completed { .. } => {
                        result.state_changed = true;
                        result.progress_changed = true;
                        result.completed_now = true;
                        result.sync_recommended = true;
                        state.mark_dirty();
                    }
                }
            }
        }

        if result.state_changed && !result.sync_recommended {
            // Progress updates are coalesced by caller using config.sync_progress_interval_ticks.
            result.sync_recommended = false;
        }
        result
    }

    /// Compute mitigation multiplier for a context based on completed adaptations.
    /// Returns factor in [0.0, 1.0+] where <1 reduces damage.
    pub fn mitigation_multiplier(
        &self,
        cfg: &MvpConfig,
        state: &PlayerAdaptationStateV1,
        ctx: &ScenarioContext,
    ) -> f32 {
        let mut factor = 1.0_f32;
        for scenario in self.registry.iter() {
            factor *= scenario.mitigation_factor(cfg, state, ctx);
        }
        factor.clamp(0.0, 1.0)
    }
}
