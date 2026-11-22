//! P1.3 — analysis task system and surface-exposure mapping.
//!
//! Pure Rust: no `mod_api`, no `Host`, no I/O. The lead calls these helpers from
//! `player_tick_pre`/`player_tick_post`, populating [`SurfaceExposure`] from
//! Horizon block/effect queries and feeding the returned concept ids into
//! [`start_task`] / [`start_or_accelerate`].
//!
//! Upstream references (read-only, Java mod `adaptionwheel`):
//! - `data/AdaptionTask.java` — `{concept, timer, maxTimer}` with
//!   `progress = 1 - timer/maxTimer` (upstream uses integer ticks; here we use
//!   `f32` seconds so the native tick hooks can pass a float `dt`).
//! - `server/AdaptionEvents.java` `startTask`/`startOrAccelerate`/`completeTask`
//!   (~1882–1937): a task is added only once per concept; further exposure
//!   shortens the running timer instead of restarting it.
//! - `server/AdaptionEvents.java` `onPlayerTick` (~1221–1246): lava, liquid,
//!   drowning and (every 4 ticks) darkness, ice, slime, cobweb.
//! - `server/MovementTriggers.java` `onPlayerTick` (~39–56): soul sand, honey,
//!   powder snow, sweet berry bush, bubble column, every 4 ticks.
//! - `config/AdaptionConfig.java`: `ENV_ANALYSIS_SECONDS` default **3.0 s**
//!   (`envAnalysisTime`, 0.1–300.0), converted upstream by `* 20` into ticks.
//! - `category/Concepts.java` (~58–76): the exact concept id strings below.

/// Upstream `AdaptionConfig.ENV_ANALYSIS_SECONDS` default (3.0 s). Every
/// surface-exposure task in `AdaptionEvents`/`MovementTriggers` is started with
/// `ENV_ANALYSIS_SECONDS * 20` ticks, so all eleven concepts share this base.
pub const TASK_ENV_ANALYSIS_SECONDS: f32 = 3.0;

/// `Env_Lava` — `AdaptionEvents.onPlayerTick` when `player.isInLava()`.
pub const TASK_LAVA_SECONDS: f32 = TASK_ENV_ANALYSIS_SECONDS;
/// `Env_Liquid` — `AdaptionEvents.onPlayerTick` when `player.isInWater()`.
pub const TASK_LIQUID_SECONDS: f32 = TASK_ENV_ANALYSIS_SECONDS;
/// `Env_Drowning` — `AdaptionEvents.onPlayerTick` when `player.getAirSupply() <= 0`.
pub const TASK_DROWN_SECONDS: f32 = TASK_ENV_ANALYSIS_SECONDS;
/// `Env_Darkness` — `AdaptionEvents.onPlayerTick` via `isInDarkness(player)`.
pub const TASK_DARKNESS_SECONDS: f32 = TASK_ENV_ANALYSIS_SECONDS;
/// `Env_Ice` — `AdaptionEvents.onPlayerTick` via `isOnIce(player)`.
pub const TASK_ICE_SECONDS: f32 = TASK_ENV_ANALYSIS_SECONDS;
/// `Env_Slime` — `AdaptionEvents.onPlayerTick` via `isStandingOnSlime(player)`.
pub const TASK_SLIME_SECONDS: f32 = TASK_ENV_ANALYSIS_SECONDS;
/// `Env_Cobweb` — `AdaptionEvents.onPlayerTick` via `isInCobweb(player)`.
pub const TASK_COBWEB_SECONDS: f32 = TASK_ENV_ANALYSIS_SECONDS;
/// `Move_BerryBush` — `MovementTriggers.onPlayerTick` via `isInBerryBush(player)`.
pub const TASK_BERRY_SECONDS: f32 = TASK_ENV_ANALYSIS_SECONDS;
/// `Move_BubbleColumn` — `MovementTriggers.onPlayerTick` via `isInBubbleColumn(player)`.
pub const TASK_BUBBLE_SECONDS: f32 = TASK_ENV_ANALYSIS_SECONDS;
/// `Move_PowderSnow` — `MovementTriggers.onPlayerTick` when `player.isInPowderSnow`.
pub const TASK_POWDER_SNOW_SECONDS: f32 = TASK_ENV_ANALYSIS_SECONDS;
/// `Move_SoulSand` — `MovementTriggers.onPlayerTick` via `BlockTags.SOUL_SPEED_BLOCKS`.
pub const TASK_SOUL_SAND_SECONDS: f32 = TASK_ENV_ANALYSIS_SECONDS;

// Concept ids mirrored verbatim from `category/Concepts.java`.
pub const CONCEPT_LAVA: &str = "Env_Lava";
pub const CONCEPT_LIQUID: &str = "Env_Liquid";
pub const CONCEPT_DROWN: &str = "Env_Drowning";
pub const CONCEPT_DARKNESS: &str = "Env_Darkness";
pub const CONCEPT_ICE: &str = "Env_Ice";
pub const CONCEPT_SLIME: &str = "Env_Slime";
pub const CONCEPT_COBWEB: &str = "Env_Cobweb";
pub const CONCEPT_BERRY: &str = "Move_BerryBush";
pub const CONCEPT_BUBBLE: &str = "Move_BubbleColumn";
pub const CONCEPT_POWDER_SNOW: &str = "Move_PowderSnow";
pub const CONCEPT_SOUL_SAND: &str = "Move_SoulSand";

/// A single persisted analysis task, mirroring upstream `AdaptionTask`.
///
/// `timer` counts down from `max_timer` to zero; `progress()` is the elapsed
/// fraction, clamped to `0.0..=1.0`.
#[derive(Debug, Clone, PartialEq)]
pub struct AdaptionTask {
    pub concept: String,
    pub timer: f32,
    pub max_timer: f32,
}

impl AdaptionTask {
    /// Start a fresh task: full timer, zero progress.
    pub fn new(concept: impl Into<String>, max_timer: f32) -> Self {
        Self {
            concept: concept.into(),
            timer: max_timer,
            max_timer,
        }
    }

    /// Elapsed fraction `1 - timer/max_timer`, clamped to `0.0..=1.0`.
    /// A non-positive `max_timer` is treated as already finished when the
    /// timer is non-positive, and as untouched otherwise (no NaN leaking out).
    pub fn progress(&self) -> f32 {
        if self.max_timer <= 0.0 {
            return if self.timer <= 0.0 { 1.0 } else { 0.0 };
        }
        (1.0 - self.timer / self.max_timer).clamp(0.0, 1.0)
    }

    /// Advance the countdown by `dt` seconds; the timer never goes below zero.
    pub fn tick(&mut self, dt: f32) {
        self.timer = (self.timer - dt).max(0.0);
    }

    /// The countdown has reached zero (upstream: task leaves the list and its
    /// concept is granted).
    pub fn is_done(&self) -> bool {
        self.timer <= 0.0
    }
}

/// Add a task for `concept` if one is not already running.
/// Returns `true` only when a task was actually added (upstream `startTask`
/// de-duplicates by concept).
pub fn start_task(tasks: &mut Vec<AdaptionTask>, concept: &str, max_timer: f32) -> bool {
    if tasks.iter().any(|t| t.concept == concept) {
        return false;
    }
    tasks.push(AdaptionTask::new(concept, max_timer));
    true
}

/// Start a task if absent; otherwise shorten the running timer by
/// `accelerate` seconds (min 0), mirroring upstream `startOrAccelerate`.
/// Returns `true` when progress changed — i.e. a task was added or its
/// timer actually moved.
pub fn start_or_accelerate(
    tasks: &mut Vec<AdaptionTask>,
    concept: &str,
    max_timer: f32,
    accelerate: f32,
) -> bool {
    if let Some(task) = tasks.iter_mut().find(|t| t.concept == concept) {
        let before = task.timer;
        task.timer = (task.timer - accelerate).max(0.0);
        task.timer != before
    } else {
        start_task(tasks, concept, max_timer)
    }
}

/// Tick every task by `dt` seconds, remove the finished ones and return their
/// concept ids in their original order (upstream `completeTask` grants one
/// concept per completed task on removal).
pub fn tick_tasks(tasks: &mut Vec<AdaptionTask>, dt: f32) -> Vec<String> {
    for task in tasks.iter_mut() {
        task.tick(dt);
    }
    let completed: Vec<String> = tasks
        .iter()
        .filter(|t| t.is_done())
        .map(|t| t.concept.clone())
        .collect();
    tasks.retain(|t| !t.is_done());
    completed
}

/// Plain input struct: one boolean per upstream surface-exposure trigger.
/// The lead must populate it from Horizon player_tick queries (see report).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SurfaceExposure {
    pub in_lava: bool,
    pub in_water_or_liquid: bool,
    pub drowning: bool,
    pub in_darkness: bool,
    pub on_ice: bool,
    pub on_slime: bool,
    pub in_cobweb: bool,
    pub in_sweet_berry: bool,
    pub in_bubble_column: bool,
    pub in_powder_snow: bool,
    pub on_soul_sand: bool,
}

/// Map the true flags to `(upstream concept id, duration in seconds)`.
///
/// Source per flag (all durations = `AdaptionConfig.ENV_ANALYSIS_SECONDS` = 3.0 s):
/// - `in_lava`        -> `Env_Lava`        (`AdaptionEvents` ~1222)
/// - `in_water_or_liquid` -> `Env_Liquid`  (`AdaptionEvents` ~1225)
/// - `drowning`       -> `Env_Drowning`    (`AdaptionEvents` ~1228)
/// - `in_darkness`    -> `Env_Darkness`    (`AdaptionEvents` ~1233)
/// - `on_ice`         -> `Env_Ice`         (`AdaptionEvents` ~1236)
/// - `on_slime`       -> `Env_Slime`       (`AdaptionEvents` ~1239)
/// - `in_cobweb`      -> `Env_Cobweb`      (`AdaptionEvents` ~1242)
/// - `in_sweet_berry` -> `Move_BerryBush`  (`MovementTriggers` ~51)
/// - `in_bubble_column` -> `Move_BubbleColumn` (`MovementTriggers` ~54)
/// - `in_powder_snow` -> `Move_PowderSnow` (`MovementTriggers` ~48)
/// - `on_soul_sand`   -> `Move_SoulSand`   (`MovementTriggers` ~43)
///
/// Returns an empty vector when nothing is exposed. Order is fixed by the
/// field order above so the lead gets deterministic scheduling.
pub fn exposure_concepts(exposure: &SurfaceExposure) -> Vec<(&'static str, f32)> {
    let mut out = Vec::new();
    if exposure.in_lava {
        out.push((CONCEPT_LAVA, TASK_LAVA_SECONDS));
    }
    if exposure.in_water_or_liquid {
        out.push((CONCEPT_LIQUID, TASK_LIQUID_SECONDS));
    }
    if exposure.drowning {
        out.push((CONCEPT_DROWN, TASK_DROWN_SECONDS));
    }
    if exposure.in_darkness {
        out.push((CONCEPT_DARKNESS, TASK_DARKNESS_SECONDS));
    }
    if exposure.on_ice {
        out.push((CONCEPT_ICE, TASK_ICE_SECONDS));
    }
    if exposure.on_slime {
        out.push((CONCEPT_SLIME, TASK_SLIME_SECONDS));
    }
    if exposure.in_cobweb {
        out.push((CONCEPT_COBWEB, TASK_COBWEB_SECONDS));
    }
    if exposure.in_sweet_berry {
        out.push((CONCEPT_BERRY, TASK_BERRY_SECONDS));
    }
    if exposure.in_bubble_column {
        out.push((CONCEPT_BUBBLE, TASK_BUBBLE_SECONDS));
    }
    if exposure.in_powder_snow {
        out.push((CONCEPT_POWDER_SNOW, TASK_POWDER_SNOW_SECONDS));
    }
    if exposure.on_soul_sand {
        out.push((CONCEPT_SOUL_SAND, TASK_SOUL_SAND_SECONDS));
    }
    out
}

/// Membership test for the eleven surface-exposure concept ids.
pub fn is_surface_exposure_concept(concept: &str) -> bool {
    matches!(
        concept,
        CONCEPT_LAVA
            | CONCEPT_LIQUID
            | CONCEPT_DROWN
            | CONCEPT_DARKNESS
            | CONCEPT_ICE
            | CONCEPT_SLIME
            | CONCEPT_COBWEB
            | CONCEPT_BERRY
            | CONCEPT_BUBBLE
            | CONCEPT_POWDER_SNOW
            | CONCEPT_SOUL_SAND
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_math_and_completion() {
        let mut t = AdaptionTask::new(CONCEPT_LAVA, 4.0);
        assert_eq!(t.concept, CONCEPT_LAVA);
        assert_eq!(t.timer, 4.0);
        assert_eq!(t.max_timer, 4.0);
        assert!((t.progress() - 0.0).abs() < f32::EPSILON);
        assert!(!t.is_done());

        t.tick(1.0);
        assert!((t.progress() - 0.25).abs() < f32::EPSILON);
        t.tick(2.0);
        assert!((t.progress() - 0.75).abs() < f32::EPSILON);
        assert!(!t.is_done());

        t.tick(1.0);
        assert!((t.progress() - 1.0).abs() < f32::EPSILON);
        assert!(t.is_done());

        // Timer never goes negative.
        t.tick(10.0);
        assert_eq!(t.timer, 0.0);
        assert!((t.progress() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn progress_is_clamped_and_handles_degenerate_max_timer() {
        let mut t = AdaptionTask::new("X", 2.0);
        t.tick(-5.0); // artificial timer growth — clamp to 0 progress
        assert_eq!(t.progress(), 0.0);
        assert!(!t.is_done());

        let zero = AdaptionTask::new("X", 0.0);
        assert_eq!(zero.progress(), 1.0); // no NaN
        assert!(zero.is_done());
    }

    #[test]
    fn start_task_is_idempotent() {
        let mut tasks: Vec<AdaptionTask> = Vec::new();
        assert!(start_task(&mut tasks, CONCEPT_LAVA, 3.0));
        assert!(!start_task(&mut tasks, CONCEPT_LAVA, 9.0));
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].max_timer, 3.0); // original timer preserved
        assert!(start_task(&mut tasks, CONCEPT_ICE, 3.0));
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn start_or_accelerate_reduces_timer_and_reports_change() {
        let mut tasks: Vec<AdaptionTask> = Vec::new();
        // Absent -> starts (progress changed).
        assert!(start_or_accelerate(&mut tasks, CONCEPT_LAVA, 10.0, 2.0));
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].timer, 10.0); // acceleration only applies to a running task

        // Present -> timer shrinks by the acceleration amount.
        assert!(start_or_accelerate(&mut tasks, CONCEPT_LAVA, 10.0, 2.5));
        assert_eq!(tasks[0].timer, 7.5);
        assert!((tasks[0].progress() - 0.25).abs() < f32::EPSILON);
        assert_eq!(tasks.len(), 1);

        // No-op acceleration / already finished -> no progress change.
        assert!(!start_or_accelerate(&mut tasks, CONCEPT_LAVA, 10.0, 0.0));
        assert!(start_or_accelerate(&mut tasks, CONCEPT_LAVA, 10.0, 100.0));
        assert_eq!(tasks[0].timer, 0.0);
        assert!(!start_or_accelerate(&mut tasks, CONCEPT_LAVA, 10.0, 1.0));
    }

    #[test]
    fn tick_tasks_completes_and_removes() {
        let mut tasks = vec![
            AdaptionTask::new(CONCEPT_LAVA, 1.0),
            AdaptionTask::new(CONCEPT_ICE, 3.0),
            AdaptionTask::new(CONCEPT_COBWEB, 2.0),
        ];
        let done = tick_tasks(&mut tasks, 1.5);
        assert_eq!(done, vec![CONCEPT_LAVA.to_string()]);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].concept, CONCEPT_ICE);
        assert_eq!(tasks[1].concept, CONCEPT_COBWEB);
        assert!((tasks[1].timer - 0.5).abs() < f32::EPSILON);

        let done = tick_tasks(&mut tasks, 0.5);
        assert_eq!(done, vec![CONCEPT_COBWEB.to_string()]);
        assert_eq!(tasks.len(), 1);

        let done = tick_tasks(&mut tasks, 10.0);
        assert_eq!(done, vec![CONCEPT_ICE.to_string()]);
        assert!(tasks.is_empty());
        assert!(tick_tasks(&mut tasks, 1.0).is_empty());
    }

    #[test]
    fn exposure_mapping_for_each_flag() {
        let cases: [(fn(&mut SurfaceExposure), &str, f32); 11] = [
            (|e| e.in_lava = true, CONCEPT_LAVA, TASK_LAVA_SECONDS),
            (
                |e| e.in_water_or_liquid = true,
                CONCEPT_LIQUID,
                TASK_LIQUID_SECONDS,
            ),
            (|e| e.drowning = true, CONCEPT_DROWN, TASK_DROWN_SECONDS),
            (
                |e| e.in_darkness = true,
                CONCEPT_DARKNESS,
                TASK_DARKNESS_SECONDS,
            ),
            (|e| e.on_ice = true, CONCEPT_ICE, TASK_ICE_SECONDS),
            (|e| e.on_slime = true, CONCEPT_SLIME, TASK_SLIME_SECONDS),
            (
                |e| e.in_cobweb = true,
                CONCEPT_COBWEB,
                TASK_COBWEB_SECONDS,
            ),
            (
                |e| e.in_sweet_berry = true,
                CONCEPT_BERRY,
                TASK_BERRY_SECONDS,
            ),
            (
                |e| e.in_bubble_column = true,
                CONCEPT_BUBBLE,
                TASK_BUBBLE_SECONDS,
            ),
            (
                |e| e.in_powder_snow = true,
                CONCEPT_POWDER_SNOW,
                TASK_POWDER_SNOW_SECONDS,
            ),
            (
                |e| e.on_soul_sand = true,
                CONCEPT_SOUL_SAND,
                TASK_SOUL_SAND_SECONDS,
            ),
        ];
        for (set, concept, duration) in cases {
            let mut exposure = SurfaceExposure::default();
            set(&mut exposure);
            assert_eq!(exposure_concepts(&exposure), vec![(concept, duration)]);
        }
    }

    #[test]
    fn exposure_all_flags_and_no_false_positives() {
        let default = SurfaceExposure::default();
        assert!(exposure_concepts(&default).is_empty());

        let all = SurfaceExposure {
            in_lava: true,
            in_water_or_liquid: true,
            drowning: true,
            in_darkness: true,
            on_ice: true,
            on_slime: true,
            in_cobweb: true,
            in_sweet_berry: true,
            in_bubble_column: true,
            in_powder_snow: true,
            on_soul_sand: true,
        };
        let mapped = exposure_concepts(&all);
        assert_eq!(mapped.len(), 11);
        assert_eq!(mapped[0], (CONCEPT_LAVA, TASK_LAVA_SECONDS));
        assert_eq!(mapped[1], (CONCEPT_LIQUID, TASK_LIQUID_SECONDS));
        assert_eq!(mapped[2], (CONCEPT_DROWN, TASK_DROWN_SECONDS));
        assert_eq!(mapped[10], (CONCEPT_SOUL_SAND, TASK_SOUL_SAND_SECONDS));
        for (id, seconds) in &mapped {
            assert!(is_surface_exposure_concept(id), "missing membership: {id}");
            assert!((*seconds - TASK_ENV_ANALYSIS_SECONDS).abs() < f32::EPSILON);
        }

        // Unrelated flags do not leak into the mapping.
        for id in [
            "Env_Fire",
            "Env_FallDamage",
            "Move_Honey",
            "Type_FIRE",
            "env_lava",
            "",
        ] {
            assert!(!is_surface_exposure_concept(id), "false positive: {id}");
        }
    }
}
