//! MVP scenario: env_fire (fire/burning damage analysis/mitigation).

use crate::config::MvpConfig;
use crate::scenarios::{apply_progress, AdaptationScenario, ScenarioContext, ScenarioOutcome};
use crate::state::{ConceptId, PlayerAdaptationStateV1};

pub struct FireScenario {
    concept: ConceptId,
}

impl FireScenario {
    pub fn new(_cfg: &MvpConfig) -> Self {
        Self { concept: ConceptId::EnvFire }
    }

    fn is_fire_kind(kind: Option<&str>) -> bool {
        matches!(
            kind,
            Some("fire")
                | Some("in_fire")
                | Some("on_fire")
                | Some("hot_floor")
                | Some("campfire")
                | Some("lava" /* lava handled as fire-like in some impls; keep configurable/TBD */)
        )
    }

    fn gain(&self, cfg: &MvpConfig, ctx: &ScenarioContext) -> f32 {
        // Use incoming damage as exposure proxy for MVP; hook layer can refine.
        let raw = if ctx.incoming_damage > 0.0 { ctx.incoming_damage } else { 1.0 };
        raw.clamp(0.0, cfg.scenarios.fire.max_gain_per_event)
    }
}

impl AdaptationScenario for FireScenario {
    fn concept(&self) -> ConceptId {
        self.concept
    }

    fn matches(&self, ctx: &ScenarioContext) -> bool {
        Self::is_fire_kind(ctx.damage_kind.as_deref())
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
        let threshold = cfg.scenarios.fire.threshold;
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
            cfg.scenarios.fire.mitigation_multiplier
        } else {
            1.0
        }
    }
}
