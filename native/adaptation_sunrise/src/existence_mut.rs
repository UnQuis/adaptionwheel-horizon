//! Pure port of the upstream Existence/boss and Mutation systems.
//!
//! No `mod_api`, no host, no RNG, no side effects beyond the maps/sets passed
//! in: the integration layer owns game calls and persistence. Every id and
//! number below is taken verbatim from
//! `ru.adaptionwheel.{server.AdaptionEvents, data.PlayerAdaption,
//! category.Concepts, config.AdaptionConfig, server.FistMastery}`.
//!
//! Upstream randomness is represented as a caller-supplied `roll`, so a whole
//! fight can be replayed and tested. The one upstream float quirk that matters
//! (`noteBossHit` narrows `scale` to `f32` before multiplying) is preserved.

use std::collections::{BTreeMap, BTreeSet};

// ── Upstream ids and constants ───────────────────────────────────────────────

/// `PlayerAdaption.MAX_LEVEL`.
pub const MAX_LEVEL: u8 = 8;

/// `Concepts.*_PREFIX`.
pub const EXISTENCE_PREFIX: &str = "Existence_";
pub const MUTATION_PREFIX: &str = "Mutation_";
pub const CONTACT_PREFIX: &str = "Contact_";
pub const OFFENSE_PREFIX: &str = "Offense_NPC_";
pub const DROP_PREFIX: &str = "Drop_NPC_";
pub const TYPE_PREFIX: &str = "Type_";
pub const DEBUFF_PREFIX: &str = "Debuff_";

/// `Concepts.MUTATION_*` and `DIMENSION_DESTROY`, verbatim.
pub const MUTATION_THERMAL: &str = "Mutation_Thermal";
pub const MUTATION_AQUATIC: &str = "Mutation_Aquatic";
pub const MUTATION_IMPACT: &str = "Mutation_Impact";
pub const MUTATION_SEA_EYE: &str = "Mutation_SeaEye";
pub const MUTATION_FLIGHT: &str = "Mutation_Flight";
pub const MUTATION_FIST: &str = "Mutation_Fist";
pub const DIMENSION_DESTROY: &str = "Dimension_Destroy";

/// `Concepts.ENV_*` ids used by the combo gates. Note the upstream spellings:
/// `Env_FallDamage` and `Env_Drowning`, not `Env_Fall` / `Env_Drown`.
pub const ENV_FIRE: &str = "Env_Fire";
pub const ENV_LAVA: &str = "Env_Lava";
pub const ENV_LIQUID: &str = "Env_Liquid";
pub const ENV_DROWN: &str = "Env_Drowning";
pub const ENV_FALL: &str = "Env_FallDamage";
pub const ENV_KNOCKBACK: &str = "Env_Knockback";

pub const MINE_LABOR: &str = "Mine_Labor";
/// `DraconicCompat.GUARDIAN_ID`.
pub const GUARDIAN_ID: &str = "draconicevolution:draconic_guardian";
/// `AdaptionEvents.FLIGHT_MOB` / `FLIGHT_LEVITATION`.
pub const FLIGHT_MOB: &str = "minecraft:phantom";
pub const FLIGHT_LEVITATION: &str = "minecraft:levitation";

/// `AdaptionConfig` defaults and the two hard-coded upstream denominators.
pub const EXISTENCE_REQUIRED_SECONDS: f64 = 240.0;
pub const TICKS_PER_SECOND: u32 = 20;
pub const EXISTENCE_REQUIRED_TICKS: u32 = 4_800;
/// Original's `requiredFrames / 108000f` hit-reduction scale.
pub const EXISTENCE_HIT_FRAME_BASE: f64 = 108_000.0;
/// `6 + random.nextInt(55)` in `ReduceCombatTime`.
pub const EXISTENCE_HIT_BANK_BASE: u32 = 6;
pub const EXISTENCE_HIT_ROLL_SPAN: u32 = 55;
/// `AdaptionConfig.DROP_RATE_KILLS`.
pub const DROP_KILL_THRESHOLDS: [u32; MAX_LEVEL as usize] = [1, 5, 10, 30, 100, 300, 500, 1_000];
/// `AdaptionConfig.FLIGHT_ALTITUDE`, `DIMENSION_DESTROY_PERCENT`.
pub const FLIGHT_ALTITUDE: f64 = 310.0;
pub const DIMENSION_DESTROY_PERCENT: u32 = 80;
/// `FistTiers.tierOf(state) != 1` in `FistMastery.tryUnlock` (stone tier).
pub const FIST_STONE_TIER: u8 = 1;
/// `data.targetRotation += (float)(Math.PI / 2)` on an existence grant.
pub const TARGET_ROTATION_STEP_RAD: f32 = std::f32::consts::FRAC_PI_2;

// ── Concept ids ──────────────────────────────────────────────────────────────

pub fn existence_concept(boss: &str) -> String {
    format!("{EXISTENCE_PREFIX}{boss}")
}

pub fn contact_concept(mob: &str) -> String {
    format!("{CONTACT_PREFIX}{mob}")
}

pub fn offense_concept(mob: &str) -> String {
    format!("{OFFENSE_PREFIX}{mob}")
}

pub fn drop_concept(mob: &str) -> String {
    format!("{DROP_PREFIX}{mob}")
}

pub fn type_concept(category: &str) -> String {
    format!("{TYPE_PREFIX}{category}")
}

pub fn debuff_concept(effect_id: &str) -> String {
    format!("{DEBUFF_PREFIX}{effect_id}")
}

// ── Existence: boss combat tick accumulation ────────────────────────────────

/// Upstream `(int)(seconds * 20)`. A non-positive duration yields 0.
pub fn ticks_from_seconds(seconds: f64) -> u32 {
    if seconds <= 0.0 {
        0
    } else {
        (seconds * TICKS_PER_SECOND as f64) as u32
    }
}

/// Upstream `bossCombatTicks.putIfAbsent(bossPath, 0)`: an encounter must not
/// reset progress from an earlier encounter with the same boss.
pub fn note_boss_encounter(progress: &mut BTreeMap<String, u32>, boss: &str) {
    progress.entry(boss.to_string()).or_insert(0);
}

/// The hit bonus from `noteBossHit` / the original `ReduceCombatTime`.
///
/// `roll` stands in for `random.nextInt(55)` and is reduced mod 55 so any
/// caller-supplied value stays in the upstream range. The `f32` narrowing of
/// `scale` is deliberate: it is what upstream does, and it changes the result
/// at large required-tick values.
pub fn hit_reduction(required_ticks: u32, roll: u32) -> u32 {
    let scale = (required_ticks as f64 / EXISTENCE_HIT_FRAME_BASE) as f32;
    let raw = ((EXISTENCE_HIT_BANK_BASE + roll % EXISTENCE_HIT_ROLL_SPAN) as f32 * scale) as i64;
    raw.max(1) as u32
}

/// Bank a hit onto the boss's progress, clamped to `required_ticks`.
///
/// `can_bank` is upstream's `active(Existence_<boss>) && !existenceAdapted`
/// guard plus `required > 0`; the integration layer computes the `active` part
/// from its own state. Upstream `required <= 0` returns without banking.
pub fn bank_boss_hit(
    progress: &mut BTreeMap<String, u32>,
    boss: &str,
    required_ticks: u32,
    roll: u32,
    can_bank: bool,
) {
    if !can_bank || required_ticks == 0 {
        return;
    }
    let banked = progress
        .get(boss)
        .copied()
        .unwrap_or(0)
        .saturating_add(hit_reduction(required_ticks, roll))
        .min(required_ticks);
    progress.insert(boss.to_string(), banked);
}

/// Upstream `accumulateBossCombat`. Returns the boss paths that crossed the
/// threshold this tick; the caller grants the reward and removes them.
///
/// Counters for adapted bosses are dropped; counters for bosses that walked
/// away are deliberately kept (upstream does not discard analysis on exit).
pub fn accumulate_boss_combat(
    progress: &mut BTreeMap<String, u32>,
    adapted: &BTreeSet<String>,
    nearby: &BTreeSet<String>,
    required_ticks: u32,
) -> Vec<String> {
    let mut completed = Vec::new();
    let mut next = BTreeMap::new();
    for (boss, ticks) in std::mem::take(progress) {
        if adapted.contains(&boss) {
            continue;
        }
        if !nearby.contains(&boss) {
            next.insert(boss, ticks);
            continue;
        }
        let ticks = ticks + 1;
        if ticks >= required_ticks {
            completed.push(boss);
        } else {
            next.insert(boss, ticks);
        }
    }
    *progress = next;
    completed
}

/// Upstream `visibleBossProgress`: only in-progress, present bosses are shown.
pub fn visible_boss_progress(
    progress: &BTreeMap<String, u32>,
    adapted: &BTreeSet<String>,
    nearby: &BTreeSet<String>,
) -> BTreeMap<String, u32> {
    progress
        .iter()
        .filter(|&(boss, ticks)| {
            *ticks > 0 && !adapted.contains(boss.as_str()) && nearby.contains(boss.as_str())
        })
        .map(|(boss, &ticks)| (boss.clone(), ticks))
        .collect()
}

// ── Existence: unlock conditions ────────────────────────────────────────────

/// Upstream `adaptedExistenceTarget`: the boss (or Chaos-Guardian-linked path)
/// whose existence adaptation grants immunity to the current hit.
pub fn adapted_existence_target(
    existence_enabled: bool,
    chaos_guardian_enabled: bool,
    boss_path: Option<&str>,
    linked_path: Option<&str>,
    is_enabled: impl Fn(&str) -> bool,
    existence_adapted: &BTreeSet<String>,
) -> Option<String> {
    if !existence_enabled {
        return None;
    }
    let path = if let Some(boss) = boss_path {
        boss
    } else if chaos_guardian_enabled {
        linked_path?
    } else {
        return None;
    };
    (is_enabled(&existence_concept(path)) && existence_adapted.contains(path))
        .then(|| path.to_string())
}

/// Everything `grantExistenceAdaptation` writes, as pure data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistenceReward {
    pub boss_path: String,
    /// Concept added to `adapted` and pushed to history.
    pub existence: String,
    /// Concepts written to `levels` at [`MAX_LEVEL`].
    pub maxed_levels: Vec<String>,
    /// One-time concepts added to `adapted` (`Existence_*` plus, for the
    /// Chaos Guardian, `Env_Fire`).
    pub one_time_adapted: Vec<String>,
    /// Contact/offence tasks upstream removes on completion.
    pub removed_tasks: Vec<String>,
}

/// Upstream `grantExistenceAdaptation` without the game calls: the same
/// if/else-if chain on `contains`, so a modded path with "wither" in it wins
/// over the generic `Type_MOB` fallback exactly as upstream.
pub fn existence_reward(boss: &str) -> ExistenceReward {
    let existence = existence_concept(boss);
    let contact = contact_concept(boss);
    let offense = offense_concept(boss);
    let mut maxed_levels = vec![contact.clone(), offense.clone()];
    let mut one_time_adapted = vec![existence.clone()];

    if boss.contains("wither") {
        maxed_levels.push(type_concept("WITHER"));
    } else if boss.contains("ender_dragon") {
        maxed_levels.push(type_concept("MAGIC"));
        maxed_levels.push(type_concept("EXPLOSION"));
    } else if boss.contains("warden") {
        maxed_levels.push(type_concept("MAGIC"));
    } else if boss == GUARDIAN_ID {
        maxed_levels.push(type_concept("EXPLOSION"));
        maxed_levels.push(type_concept("PROJECTILE"));
        one_time_adapted.push(ENV_FIRE.to_string());
    } else {
        maxed_levels.push(type_concept("MOB"));
    }

    ExistenceReward {
        boss_path: boss.to_string(),
        existence,
        maxed_levels,
        one_time_adapted,
        removed_tasks: vec![contact, offense],
    }
}

/// Upstream `targetRotation += (float)(Math.PI / 2)`.
pub fn advance_target_rotation(current: f32) -> f32 {
    current + TARGET_ROTATION_STEP_RAD
}

// ── Kill-count thresholds ───────────────────────────────────────────────────

/// Upstream `AdaptionConfig.dropLevelFromKills`: the highest level whose
/// `DROP_RATE_KILLS` threshold has been reached (0 when none has).
pub fn drop_level_from_kills(kills: u32) -> u8 {
    let mut level = 0;
    for (idx, threshold) in DROP_KILL_THRESHOLDS.iter().enumerate() {
        if kills >= *threshold {
            level = (idx + 1) as u8;
        }
    }
    level
}

/// Upstream `onLivingDeath`: `killCounts.merge(concept, 1, Integer::sum)`,
/// returning the new drop level.
pub fn record_kill(kill_counts: &mut BTreeMap<String, u32>, concept: &str) -> u8 {
    let kills = kill_counts.entry(concept.to_string()).or_insert(0);
    *kills += 1;
    drop_level_from_kills(*kills)
}

/// Upstream `WheelData.mergeFrom`: wheel item kill counts overwrite the
/// player's only when higher (`Math::max`).
pub fn merge_kill_counts(dst: &mut BTreeMap<String, u32>, src: &BTreeMap<String, u32>) {
    for (concept, &kills) in src {
        dst.entry(concept.clone())
            .and_modify(|v| *v = (*v).max(kills))
            .or_insert(kills);
    }
}

// ── Mutation unlock/outcome resolution ──────────────────────────────────────

/// Exact prerequisite ids for the four environment combos in `tickWearer`.
/// Other mutations have event/percentage gates and return an empty slice.
pub fn mutation_prerequisites(concept: &str) -> &'static [&'static str] {
    match concept {
        MUTATION_THERMAL => &[ENV_FIRE, ENV_LAVA],
        MUTATION_AQUATIC => &[ENV_LIQUID, ENV_DROWN],
        MUTATION_IMPACT => &[ENV_FALL, ENV_KNOCKBACK],
        MUTATION_SEA_EYE => &[ENV_LIQUID, ENV_DROWN, ENV_LAVA],
        _ => &[],
    }
}

/// Upstream `grantComboMutation` idempotence plus a combo's active-concept
/// gate: `!isAdapted(concept) && !active(concept) && prerequisites all active`.
pub fn combo_mutation_ready(
    concept: &str,
    active: &BTreeSet<String>,
    adapted: &BTreeSet<String>,
) -> bool {
    !adapted.contains(concept)
        && !active.contains(concept)
        && mutation_prerequisites(concept)
            .iter()
            .all(|p| active.contains(*p))
}

/// Upstream `tickFlight`: altitude >= 310, `Contact_minecraft:phantom` level 8
/// (raw `level`, not `levelOrZero`) and adapted-to-levitation. Switching the
/// mutation off does not revoke the earned adaptation upstream.
pub fn flight_mutation_ready(
    altitude: f64,
    phantom_level: u8,
    has_levitation_adaptation: bool,
    already_adapted: bool,
) -> bool {
    !already_adapted
        && altitude >= FLIGHT_ALTITUDE
        && phantom_level >= MAX_LEVEL
        && has_levitation_adaptation
}

/// Upstream `FistMastery.tryUnlock`: maxed `Mine_Labor` and mining a tier-1
/// (stone) block. Event-driven, so it is not part of the per-tick resolver.
pub fn fist_mutation_ready(mine_labor_level: u8, block_tier: u8, already_adapted: bool) -> bool {
    !already_adapted && mine_labor_level >= MAX_LEVEL && block_tier == FIST_STONE_TIER
}

/// Upstream `reachesDimensionDestroy`, using 64-bit math so large modpack
/// concept counts cannot overflow the percentage comparison.
pub fn reaches_dimension_destroy(adapt_count: u32, total_concepts: u32, percent: u32) -> bool {
    if total_concepts == 0 {
        return false;
    }
    adapt_count as u64 * 100 >= total_concepts as u64 * percent as u64
}

/// Inputs to the per-tick grant resolution. `active` and `adapted` are the
/// integration layer's already-computed concept sets; no game API is called.
#[derive(Debug, Default)]
pub struct MutationInputs {
    pub active: BTreeSet<String>,
    pub adapted: BTreeSet<String>,
    pub altitude: f64,
    pub phantom_contact_level: u8,
    pub has_levitation_adaptation: bool,
    pub adapt_count: u32,
    pub total_concepts: u32,
    pub dimension_destroy_percent: u32,
    pub enable_aquatic: bool,
    pub enable_impact: bool,
    pub enable_sea_eye: bool,
    pub enable_flight: bool,
    pub enable_dimension_destroy: bool,
}

/// The mutations freshly earned this tick, in the exact order `tickWearer`
/// checks them. Thermal has no enable flag upstream, so it is unconditional.
/// Fist is omitted because it is unlocked by a mining event, not a tick.
pub fn resolve_mutation_outcomes(input: &MutationInputs) -> Vec<&'static str> {
    let mut out = Vec::new();
    let mut grant = |id: &'static str, ready: bool| {
        if ready && !input.adapted.contains(id) && !input.active.contains(id) {
            out.push(id);
        }
    };

    grant(
        MUTATION_THERMAL,
        combo_mutation_ready(MUTATION_THERMAL, &input.active, &input.adapted),
    );
    grant(
        MUTATION_AQUATIC,
        input.enable_aquatic
            && combo_mutation_ready(MUTATION_AQUATIC, &input.active, &input.adapted),
    );
    grant(
        MUTATION_IMPACT,
        input.enable_impact && combo_mutation_ready(MUTATION_IMPACT, &input.active, &input.adapted),
    );
    grant(
        MUTATION_SEA_EYE,
        input.enable_sea_eye
            && combo_mutation_ready(MUTATION_SEA_EYE, &input.active, &input.adapted),
    );
    if input.enable_flight {
        grant(
            MUTATION_FLIGHT,
            flight_mutation_ready(
                input.altitude,
                input.phantom_contact_level,
                input.has_levitation_adaptation,
                input.adapted.contains(MUTATION_FLIGHT),
            ),
        );
    }
    if input.enable_dimension_destroy {
        grant(
            DIMENSION_DESTROY,
            reaches_dimension_destroy(
                input.adapt_count,
                input.total_concepts,
                input.dimension_destroy_percent,
            ),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn concept_ids_are_verbatim_upstream() {
        assert_eq!(
            existence_concept("minecraft:wither"),
            "Existence_minecraft:wither"
        );
        assert_eq!(
            contact_concept("minecraft:phantom"),
            "Contact_minecraft:phantom"
        );
        assert_eq!(
            offense_concept("minecraft:zombie"),
            "Offense_NPC_minecraft:zombie"
        );
        assert_eq!(
            drop_concept("minecraft:zombie"),
            "Drop_NPC_minecraft:zombie"
        );
        assert_eq!(type_concept("WITHER"), "Type_WITHER");
        assert_eq!(
            debuff_concept(FLIGHT_LEVITATION),
            "Debuff_minecraft:levitation"
        );
        assert_eq!(ENV_FALL, "Env_FallDamage");
        assert_eq!(ENV_DROWN, "Env_Drowning");
        assert_eq!(MUTATION_SEA_EYE, "Mutation_SeaEye");
    }

    #[test]
    fn default_existence_requirement_is_4800_ticks() {
        assert_eq!(
            ticks_from_seconds(EXISTENCE_REQUIRED_SECONDS),
            EXISTENCE_REQUIRED_TICKS
        );
        assert_eq!(ticks_from_seconds(0.0), 0);
        assert_eq!(ticks_from_seconds(-1.0), 0);
    }

    #[test]
    fn encounter_does_not_reset_existing_progress() {
        let mut progress = BTreeMap::from([("minecraft:wither".to_string(), 42)]);
        note_boss_encounter(&mut progress, "minecraft:wither");
        note_boss_encounter(&mut progress, "minecraft:warden");
        assert_eq!(progress["minecraft:wither"], 42);
        assert_eq!(progress["minecraft:warden"], 0);
    }

    #[test]
    fn hit_reduction_uses_upstream_range_and_f32_scale() {
        // required == 108000 makes scale exactly 1.0: the raw roll range is 6..60.
        assert_eq!(hit_reduction(108_000, 0), 6);
        assert_eq!(hit_reduction(108_000, 54), 60);
        assert_eq!(hit_reduction(108_000, 999), 6 + (999 % 55));
        // At the default 4800 the original's arithmetic floors most hits to 1.
        assert_eq!(hit_reduction(4_800, 0), 1);
        assert_eq!(hit_reduction(4_800, 54), 2);
    }

    #[test]
    fn banked_hits_are_gated_and_clamped() {
        let boss = "minecraft:wither";
        let mut progress = BTreeMap::from([(boss.to_string(), 4_799)]);
        bank_boss_hit(&mut progress, boss, 4_800, 999, false);
        assert_eq!(progress[boss], 4_799);
        bank_boss_hit(&mut progress, boss, 0, 0, true);
        assert_eq!(progress[boss], 4_799);
        // At the default 4800 the original's scale floors every hit to one tick.
        bank_boss_hit(&mut progress, boss, 4_800, 0, true);
        assert_eq!(progress[boss], 4_800);
        bank_boss_hit(&mut progress, boss, 4_800, 54, true);
        assert_eq!(progress[boss], 4_800);
    }

    #[test]
    fn accumulate_ticks_only_nearby_and_completes_at_threshold() {
        let adapted = set(&["minecraft:warden"]);
        let nearby = set(&["minecraft:wither", "minecraft:ender_dragon"]);
        let mut progress = BTreeMap::from([
            ("minecraft:wither".to_string(), 1),
            ("minecraft:ender_dragon".to_string(), 0),
            ("minecraft:warden".to_string(), 10),
            ("minecraft:old_boss".to_string(), 7),
        ]);
        assert!(accumulate_boss_combat(&mut progress, &adapted, &nearby, 3).is_empty());
        assert_eq!(progress["minecraft:wither"], 2);
        assert_eq!(progress["minecraft:ender_dragon"], 1);
        assert!(!progress.contains_key("minecraft:warden"));
        assert_eq!(progress["minecraft:old_boss"], 7);

        let done = accumulate_boss_combat(&mut progress, &adapted, &nearby, 3);
        assert_eq!(done, vec!["minecraft:wither"]);
        assert!(!progress.contains_key("minecraft:wither"));
        assert_eq!(progress["minecraft:ender_dragon"], 2);
    }

    #[test]
    fn visible_progress_filters_adapted_absent_and_zero() {
        let progress = BTreeMap::from([
            ("a".to_string(), 0),
            ("b".to_string(), 5),
            ("c".to_string(), 5),
            ("d".to_string(), 5),
        ]);
        let visible =
            visible_boss_progress(&progress, &set(&["c"]), &set(&["b", "c", "not_tracked"]));
        assert_eq!(visible, BTreeMap::from([("b".to_string(), 5)]));
    }

    #[test]
    fn existence_immunity_respects_off_switch_and_guardian_link() {
        let adapted = set(&["minecraft:wither", GUARDIAN_ID]);
        let enabled = |c: &str| !c.contains("disabled");
        assert_eq!(
            adapted_existence_target(
                true,
                false,
                Some("minecraft:wither"),
                None,
                &enabled,
                &adapted
            ),
            Some("minecraft:wither".to_string())
        );
        assert_eq!(
            adapted_existence_target(
                false,
                false,
                Some("minecraft:wither"),
                None,
                &enabled,
                &adapted
            ),
            None
        );
        assert_eq!(
            adapted_existence_target(
                true,
                false,
                Some("minecraft:warden"),
                None,
                &enabled,
                &adapted
            ),
            None
        );
        // Linked path is only consulted when there is no direct boss source.
        assert_eq!(
            adapted_existence_target(true, true, None, Some(GUARDIAN_ID), &enabled, &adapted),
            Some(GUARDIAN_ID.to_string())
        );
        assert_eq!(
            adapted_existence_target(true, false, None, Some(GUARDIAN_ID), &enabled, &adapted),
            None
        );
        let disabled = |_c: &str| false;
        assert_eq!(
            adapted_existence_target(true, true, None, Some(GUARDIAN_ID), disabled, &adapted),
            None
        );
    }

    #[test]
    fn existence_reward_matches_each_boss_branch() {
        let wither = existence_reward("minecraft:wither");
        assert_eq!(wither.existence, "Existence_minecraft:wither");
        assert_eq!(
            wither.maxed_levels,
            vec![
                "Contact_minecraft:wither",
                "Offense_NPC_minecraft:wither",
                "Type_WITHER"
            ]
        );
        assert_eq!(wither.one_time_adapted, vec!["Existence_minecraft:wither"]);
        assert_eq!(wither.removed_tasks, wither.maxed_levels[..2]);

        assert_eq!(
            existence_reward("minecraft:ender_dragon").maxed_levels[2..],
            ["Type_MAGIC", "Type_EXPLOSION"]
        );
        assert_eq!(
            existence_reward("minecraft:warden").maxed_levels[2..],
            ["Type_MAGIC"]
        );

        let guardian = existence_reward(GUARDIAN_ID);
        assert_eq!(
            guardian.maxed_levels[2..],
            ["Type_EXPLOSION", "Type_PROJECTILE"]
        );
        assert_eq!(guardian.one_time_adapted[1], ENV_FIRE);

        assert_eq!(
            existence_reward("mod:final_boss").maxed_levels[2..],
            ["Type_MOB"]
        );
    }

    #[test]
    fn existence_rotation_step_is_half_pi() {
        assert!((advance_target_rotation(0.0) - TARGET_ROTATION_STEP_RAD).abs() < f32::EPSILON);
    }

    #[test]
    fn drop_level_from_kills_uses_exact_thresholds() {
        for (kills, level) in [
            (0, 0),
            (1, 1),
            (4, 1),
            (5, 2),
            (9, 2),
            (10, 3),
            (29, 3),
            (30, 4),
            (99, 4),
            (100, 5),
            (299, 5),
            (300, 6),
            (499, 6),
            (500, 7),
            (999, 7),
            (1_000, 8),
            (10_000, 8),
        ] {
            assert_eq!(drop_level_from_kills(kills), level, "kills={kills}");
        }
    }

    #[test]
    fn kills_accumulate_and_wheel_merge_uses_max() {
        let mut counts = BTreeMap::new();
        assert_eq!(record_kill(&mut counts, "Drop_NPC_one"), 1);
        let concept = "Drop_NPC_minecraft:zombie";
        assert_eq!(record_kill(&mut counts, concept), 1);
        for _ in 0..9 {
            record_kill(&mut counts, concept);
        }
        assert_eq!(counts[concept], 10);
        assert_eq!(drop_level_from_kills(counts[concept]), 3);

        let wheel = BTreeMap::from([(concept.to_string(), 30), ("mod:other".to_string(), 2)]);
        merge_kill_counts(&mut counts, &wheel);
        assert_eq!(counts[concept], 30); // max, not sum
        assert_eq!(counts["mod:other"], 2);
    }

    #[test]
    fn combo_prerequisites_are_the_exact_upstream_sets() {
        assert_eq!(
            mutation_prerequisites(MUTATION_THERMAL),
            &[ENV_FIRE, ENV_LAVA]
        );
        assert_eq!(
            mutation_prerequisites(MUTATION_AQUATIC),
            &[ENV_LIQUID, ENV_DROWN]
        );
        assert_eq!(
            mutation_prerequisites(MUTATION_IMPACT),
            &[ENV_FALL, ENV_KNOCKBACK]
        );
        assert_eq!(
            mutation_prerequisites(MUTATION_SEA_EYE),
            &[ENV_LIQUID, ENV_DROWN, ENV_LAVA]
        );
        assert!(mutation_prerequisites(MUTATION_FLIGHT).is_empty());
    }

    #[test]
    fn combo_ready_is_idempotent_and_needs_all_prerequisites() {
        let partial = set(&[ENV_LAVA, ENV_LIQUID]);
        assert!(!combo_mutation_ready(
            MUTATION_THERMAL,
            &partial,
            &BTreeSet::new()
        ));
        assert!(!combo_mutation_ready(
            MUTATION_AQUATIC,
            &partial,
            &BTreeSet::new()
        ));
        let full = set(&[ENV_FIRE, ENV_LAVA, ENV_LIQUID, ENV_DROWN]);
        assert!(combo_mutation_ready(
            MUTATION_THERMAL,
            &full,
            &BTreeSet::new()
        ));
        assert!(combo_mutation_ready(
            MUTATION_SEA_EYE,
            &full,
            &BTreeSet::new()
        ));
        assert!(!combo_mutation_ready(
            MUTATION_SEA_EYE,
            &full,
            &set(&[MUTATION_SEA_EYE])
        ));
        assert!(!combo_mutation_ready(
            MUTATION_AQUATIC,
            &set(&[MUTATION_AQUATIC, ENV_LIQUID, ENV_DROWN]),
            &BTreeSet::new()
        ));
    }

    #[test]
    fn flight_and_fist_gates_match_upstream() {
        assert!(flight_mutation_ready(310.0, MAX_LEVEL, true, false));
        assert!(!flight_mutation_ready(309.9, MAX_LEVEL, true, false));
        assert!(!flight_mutation_ready(310.0, MAX_LEVEL - 1, true, false));
        assert!(!flight_mutation_ready(310.0, MAX_LEVEL, false, false));
        assert!(!flight_mutation_ready(310.0, MAX_LEVEL, true, true));

        assert!(fist_mutation_ready(MAX_LEVEL, FIST_STONE_TIER, false));
        assert!(!fist_mutation_ready(MAX_LEVEL - 1, FIST_STONE_TIER, false));
        assert!(!fist_mutation_ready(MAX_LEVEL, 0, false));
        assert!(!fist_mutation_ready(MAX_LEVEL, FIST_STONE_TIER, true));
    }

    #[test]
    fn dimension_destroy_uses_integer_percentage_math() {
        assert!(reaches_dimension_destroy(265, 331, 80));
        assert!(!reaches_dimension_destroy(264, 331, 80));
        assert!(!reaches_dimension_destroy(0, 0, 80));
        assert!(reaches_dimension_destroy(1, 1, 1));
        assert!(reaches_dimension_destroy(u32::MAX, u32::MAX, 100));
    }

    #[test]
    fn mutation_outcomes_follow_upstream_order_and_gates() {
        let mut input = MutationInputs {
            active: set(&[
                ENV_FIRE,
                ENV_LAVA,
                ENV_LIQUID,
                ENV_DROWN,
                ENV_FALL,
                ENV_KNOCKBACK,
            ]),
            altitude: 310.0,
            phantom_contact_level: MAX_LEVEL,
            has_levitation_adaptation: true,
            adapt_count: 265,
            total_concepts: 331,
            dimension_destroy_percent: DIMENSION_DESTROY_PERCENT,
            enable_aquatic: true,
            enable_impact: true,
            enable_sea_eye: true,
            enable_flight: true,
            enable_dimension_destroy: true,
            ..MutationInputs::default()
        };
        assert_eq!(
            resolve_mutation_outcomes(&input),
            [
                MUTATION_THERMAL,
                MUTATION_AQUATIC,
                MUTATION_IMPACT,
                MUTATION_SEA_EYE,
                MUTATION_FLIGHT,
                DIMENSION_DESTROY,
            ]
        );

        // A granted mutation is not granted again, and each config switch
        // disables only its own mutation. Thermal has no switch.
        input.adapted.insert(MUTATION_THERMAL.to_string());
        input.enable_aquatic = false;
        input.enable_flight = false;
        assert_eq!(
            resolve_mutation_outcomes(&input),
            [MUTATION_IMPACT, MUTATION_SEA_EYE, DIMENSION_DESTROY]
        );

        // 80% of 331 is not reached: no Dimension Destroy.
        input.adapt_count = 264;
        assert_eq!(
            resolve_mutation_outcomes(&input),
            [MUTATION_IMPACT, MUTATION_SEA_EYE]
        );
    }
}
