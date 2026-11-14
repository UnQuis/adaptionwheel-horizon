//! MVP adaptation scenarios.

pub mod fall;
pub mod fire;

use crate::config::MvpConfig;
use crate::state::{ConceptId, PlayerAdaptationStateV1};

/// Normalized event context passed into the engine.
/// `TBD(Horizon API)`: populate from `living_incoming_damage` / `living_fall` events.
#[derive(Debug, Clone, Default)]
pub struct ScenarioContext {
    pub is_fall: bool,
    pub fall_distance: f32,
    pub incoming_damage: f32,
    /// e.g. "fire", "lava", "in_fire", "on_fire", "drown", "mob", etc. (normalized by hook layer)
    pub damage_kind: Option<String>,
    /// True when damage source is a living attacker (for future generic-hostile scenario).
    pub has_living_attacker: bool,
    /// Server tick for cooldown bookkeeping.
    pub server_tick: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ScenarioOutcome {
    NoChange,
    Progress { concept: ConceptId, progress: f32 },
    Completed { concept: ConceptId, level: u8 },
}

pub trait AdaptationScenario: Send + Sync {
    fn concept(&self) -> ConceptId;
    fn matches(&self, ctx: &ScenarioContext) -> bool;

    /// Apply progress/ completion for a matching event. Must not run if wheel inactive (engine enforces).
    fn evaluate_and_apply(
        &self,
        cfg: &MvpConfig,
        state: &mut PlayerAdaptationStateV1,
        ctx: &ScenarioContext,
    ) -> Option<ScenarioOutcome>;

    /// Return mitigation factor for this context if adaptation is completed; 1.0 otherwise.
    fn mitigation_factor(
        &self,
        cfg: &MvpConfig,
        state: &PlayerAdaptationStateV1,
        ctx: &ScenarioContext,
    ) -> f32;
}

pub struct ScenarioRegistry {
    scenarios: Vec<Box<dyn AdaptationScenario>>,
}

impl Default for ScenarioRegistry {
    fn default() -> Self {
        Self { scenarios: Vec::new() }
    }
}

impl ScenarioRegistry {
    pub fn new_mvp(cfg: &MvpConfig) -> Self {
        let mut reg = Self::default();
        reg.register(Box::new(fall::FallScenario::new(cfg)));
        reg.register(Box::new(fire::FireScenario::new(cfg)));
        reg
    }

    pub fn register(&mut self, scenario: Box<dyn AdaptationScenario>) {
        self.scenarios.push(scenario);
    }

    pub fn iter(&self) -> impl Iterator<Item = &Box<dyn AdaptationScenario>> {
        self.scenarios.iter()
    }
}

pub(crate) fn apply_progress(
    entry: &mut crate::state::AdaptationEntryV1,
    gain: f32,
    threshold: f32,
    target_level: u8,
    concept: ConceptId,
) -> Option<ScenarioOutcome> {
    if entry.completed || gain <= 0.0 || threshold <= 0.0 {
        return None;
    }
    let before = entry.progress;
    entry.progress = (entry.progress + gain).min(threshold);
    if entry.progress >= threshold {
        entry.completed = true;
        entry.level = target_level.max(1);
        entry.progress = threshold;
        Some(ScenarioOutcome::Completed { concept, level: entry.level })
    } else if (entry.progress - before).abs() > f32::EPSILON {
        Some(ScenarioOutcome::Progress { concept, progress: entry.progress })
    } else {
        None
    }
}
