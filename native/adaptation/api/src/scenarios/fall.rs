//! MVP scenario: env_fall (fall damage analysis/mitigation).

use crate::config::MvpConfig;
use crate::scenarios::{apply_progress, AdaptationScenario, ScenarioContext, ScenarioOutcome};
use crate::state::{ConceptId, PlayerAdaptationStateV1};

pub struct FallScenario {
    concept: ConceptId,
}

impl FallScenario {
    pub fn new(_cfg: &MvpConfig) -> Self {
        Self { concept: ConceptId::EnvFall }
    }

    fn gain(&self, cfg: &MvpConfig, ctx: &ScenarioContext) -> f32 {
        // Prefer fall distance; fallback to damage amount if distance unavailable.
        let raw = if ctx.fall_distance > 0.0 {
            ctx.fall_distance
        } else {
            ctx.incoming_damage
        };
        raw.clamp(0.0, cfg.scenarios.fall.max_gain_per_event)
    }
}

impl AdaptationScenario for FallScenario {
    fn concept(&self) -> ConceptId {
        self.concept
    }

    fn matches(&self, ctx: &ScenarioContext) -> bool {
        ctx.is_fall || ctx.damage_kind.as_deref() == Some("fall")
    }

    fn evaluate_and_apply(
        &self,
        cfg: &MvpConfig,
        state: &mut PlayerAdaptationStateV1,
        ctx: &ScenarioContext,
    ) -> Option<ScenarioOutcome> {
        if !self.matches(ctx) {
            return None;
        }
        let gain = self.gain(cfg, ctx);
        let threshold = cfg.scenarios.fall.threshold;
        let entry = state.ensure_entry_mut(self.concept);
        apply_progress(entry, gain, threshold, 1, self.concept)
    }

    fn mitigation_factor(
        &self,
        cfg: &MvpConfig,
        state: &PlayerAdaptationStateV1,
        ctx: &ScenarioContext,
    ) -> f32 {
        if !self.matches(ctx) {
            return 1.0;
        }
        let entry = state.entry(self.concept);
        if entry.completed {
            cfg.scenarios.fall.mitigation_multiplier
        } else {
            1.0
        }
    }
}
