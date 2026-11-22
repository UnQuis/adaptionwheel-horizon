//! Pure Rust port of the five-stage breaking-fist / combat-fist ladders and the
//! 400-cap passive stat bonuses from the Java mod.
//!
//! Like `state_v2.rs` and `engine_impl.rs`, this module has no `mod_api`, no
//! globals and no `Host`: every rule takes plain values so the HUD, progress
//! banking and integration layer can all agree on one implementation. Callers
//! read levels from `PlayerAdaptionV2` (via `level_or_zero`) and pass them in.

use std::collections::BTreeSet;

/// Upstream `FistTiers.TIER_COUNT` / `CombatFistTiers.TIER_COUNT`.
pub const TIER_COUNT: usize = 5;
/// Upstream `PlayerAdaption.MAX_LEVEL`: eight levels per material stage.
pub const MAX_LEVEL: u8 = 8;

/// Stone is the tier whose break unlocks the breaking fist (`tierOf(state) == 1`).
pub const MINING_UNLOCK_BLOCK_TIER: i32 = 1;
pub const MINE_LABOR: &str = "Mine_Labor";
pub const MUTATION_FIST: &str = "Mutation_Fist";

pub const MINING_CONCEPTS: [&str; TIER_COUNT] = [
    "Fist_Wood",
    "Fist_Stone",
    "Fist_Iron",
    "Fist_Diamond",
    "Fist_Netherite",
];
pub const COMBAT_CONCEPTS: [&str; TIER_COUNT] = [
    "Combat_FistWood",
    "Combat_FistStone",
    "Combat_FistIron",
    "Combat_FistDiamond",
    "Combat_FistNetherite",
];
pub const SUFFIXES: [&str; TIER_COUNT] = ["wood", "stone", "iron", "diamond", "netherite"];
pub const MATERIALS: [&str; TIER_COUNT] = ["wooden", "stone", "iron", "diamond", "netherite"];
/// Both ladders share the same palette.
pub const COLORS: [u32; TIER_COUNT] = [
    0xFFA9784A, 0xFF9A9A9A, 0xFFDCDCDC, 0xFF4FE3D8, 0xFF6B4A78,
];
/// `FistTiers.LUCK`, one entry per tier.
pub const LUCK: [u8; TIER_COUNT] = [1, 2, 3, 5, 10];

/// `AdaptionConfig.fistBlocksForNextLevel` defaults.
pub const MINING_FIRST_LEVEL_BLOCKS: f64 = 10.0;
pub const MINING_LEVEL_COST_GROWTH: f64 = 1.35;
pub const MINING_TIER_COST: [f64; TIER_COUNT] = [1.0, 1.5, 2.5, 4.0, 5.0];
/// `AdaptionConfig.fistKillsForNextLevel` defaults.
pub const COMBAT_FIRST_LEVEL_KILLS: f64 = 3.0;
pub const COMBAT_LEVEL_COST_GROWTH: f64 = 1.6;
pub const COMBAT_TIER_KILL_COST: [f64; TIER_COUNT] = [1.0, 2.0, 3.5, 6.0, 10.0];

/// `AdaptationProgression`: full character progression at 400 adaptations.
pub const DEVELOPMENT_CAP: u32 = 400;
/// Four +20% movement-speed modifiers at the cap.
pub const MAX_MOVEMENT_SPEED_BONUS: f64 = 0.80;
/// 0.42 + 0.49 = 0.91, roughly a five-block jump under vanilla gravity.
pub const MAX_JUMP_STRENGTH_BONUS: f64 = 0.49;
pub const BASE_JUMP_STRENGTH: f64 = 0.42;

/// `FistTiers.suffix` / `CombatFistTiers.suffix`. Panics for an out-of-range tier.
pub fn suffix(tier: usize) -> &'static str {
    SUFFIXES[tier]
}

/// Vanilla tool id of a tier's representative tool.
pub fn tool_id(tier: usize, tool_class: &str) -> String {
    format!("minecraft:{}_{}", MATERIALS[tier], tool_class)
}

pub fn mining_concept(tier: usize) -> &'static str {
    MINING_CONCEPTS[tier]
}

pub fn combat_concept(tier: usize) -> &'static str {
    COMBAT_CONCEPTS[tier]
}

pub fn color(tier: usize) -> u32 {
    COLORS[tier]
}

pub fn luck_multiplier(tier: usize) -> u8 {
    if tier < TIER_COUNT { LUCK[tier] } else { 1 }
}

/// `Extras.miningKey` / `Extras.punchingKey`: persisted progress counters.
pub fn mining_progress_key(tier: usize) -> String {
    format!("fist_mining_{tier}")
}

pub fn combat_progress_key(tier: usize) -> String {
    format!("fist_punching_{tier}")
}

/// Shared handover rule: the previous stage's colour shows on both sides of
/// the material boundary (breaking fist at level 0 once the previous stage is
/// maxed, punching fist at level 8 until the next stage has a level).
pub fn shows_handover(level: u8, previous_maxed: bool, next_untrained: bool) -> bool {
    (level == 0 && previous_maxed) || (level >= MAX_LEVEL && next_untrained)
}

pub fn is_maxed(level: u8) -> bool {
    level >= MAX_LEVEL
}

/// `FistTiers.canHarvest`: an untagged block needs no tier.
pub fn can_harvest(required_tier: i32, tier: usize) -> bool {
    required_tier < 0 || required_tier <= tier as i32
}

/// `FistTiers.reachTier` / mining ladder thresholds: walk consecutive maxed
/// stages from Wood, then fall back to the highest stage ever trained.
pub fn mining_reach_tier(mut level_of: impl FnMut(usize) -> u8) -> usize {
    let mut progression = 0;
    while progression < TIER_COUNT - 1 && is_maxed(level_of(progression)) {
        progression += 1;
    }
    let mut granted = 0;
    for tier in (0..TIER_COUNT).rev() {
        if level_of(tier) > 0 {
            granted = tier;
            break;
        }
    }
    progression.max(granted)
}

/// `MartialArts.currentTier`: -1 before the first bare-handed kill.
pub fn combat_current_tier(level_of: impl Fn(usize) -> u8) -> i32 {
    let mut granted = -1;
    for tier in 0..TIER_COUNT {
        if level_of(tier) > 0 {
            granted = tier as i32;
        }
    }
    granted
}

/// `MartialArts.trainingTier`: a maxed stage hands its kills to the next one;
/// a finished Netherite ladder cannot train further.
pub fn combat_training_tier(level_of: impl Fn(usize) -> u8) -> i32 {
    let reached = combat_current_tier(&level_of);
    if reached < 0 {
        return -1;
    }
    let tier = reached as usize;
    if is_maxed(level_of(tier)) && tier + 1 < TIER_COUNT {
        (tier + 1) as i32
    } else {
        reached
    }
}

/// `FistMastery.tryUnlock`: max `Mine_Labor` plus breaking a stone-tier block.
pub fn mining_unlock_allowed(mine_labor_level: u8, broken_block_tier: i32) -> bool {
    is_maxed(mine_labor_level) && broken_block_tier == MINING_UNLOCK_BLOCK_TIER
}

/// `MartialArts.trains`. `direct_attacker_is_player` is true when the damage
/// source has no direct entity (the caller maps null to the bare hand). The
/// Extermination Sword is the one held item allowed to add attack damage, and
/// only in its non-cursed mode.
pub fn combat_kill_trains(
    direct_attacker_is_player: bool,
    held_is_positive_sword: bool,
    held_adds_attack_damage: bool,
) -> bool {
    direct_attacker_is_player && (held_is_positive_sword || !held_adds_attack_damage)
}

fn cost_for_level(base: f64, growth: f64, current_level: u8, tier_multiplier: f64) -> u32 {
    // Java clamps the exponent at 0, so level 0 and level 1 cost the same.
    let exponent = current_level.saturating_sub(1) as u32;
    let cost = base * growth.powf(exponent as f64) * tier_multiplier;
    cost.round().max(1.0) as u32
}

/// `AdaptionConfig.fistBlocksForNextLevel` with default config values.
pub fn blocks_for_next_level(tier: usize, current_level: u8) -> u32 {
    cost_for_level(
        MINING_FIRST_LEVEL_BLOCKS,
        MINING_LEVEL_COST_GROWTH,
        current_level,
        MINING_TIER_COST[tier],
    )
}

/// `AdaptionConfig.fistKillsForNextLevel` with default config values.
pub fn kills_for_next_level(tier: usize, current_level: u8) -> u32 {
    cost_for_level(
        COMBAT_FIRST_LEVEL_KILLS,
        COMBAT_LEVEL_COST_GROWTH,
        current_level,
        COMBAT_TIER_KILL_COST[tier],
    )
}

/// Outcome of one earned block/kill. Upstream drops surplus progress when the
/// counter reaches the requirement and grants exactly one level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TierProgress {
    NeedMore(u32),
    LevelUp,
}

pub fn progress_after_event(done: u32, need: u32) -> TierProgress {
    let have = done + 1;
    if have < need {
        TierProgress::NeedMore(have)
    } else {
        TierProgress::LevelUp
    }
}

/// Upstream `PlayerAdaption.getAdaptCount`: the **union** of `adapted` and
/// positive `levels` keys. A few concepts live in both collections; summing
/// them double-counted and produced the 483-vs-503 menu/HUD mismatch.
pub fn adapt_count<'a>(
    adapted: impl IntoIterator<Item = &'a str>,
    levels: impl IntoIterator<Item = (&'a str, u8)>,
) -> usize {
    let mut concepts = BTreeSet::new();
    concepts.extend(adapted);
    concepts.extend(levels.into_iter().filter_map(|(id, level)| (level > 0).then_some(id)));
    concepts.len()
}

/// `AdaptationProgression.progress`, clamped on both ends.
pub fn progression(adaptation_count: u32) -> f64 {
    adaptation_count.min(DEVELOPMENT_CAP) as f64 / DEVELOPMENT_CAP as f64
}

pub fn movement_speed_bonus(adaptation_count: u32) -> f64 {
    MAX_MOVEMENT_SPEED_BONUS * progression(adaptation_count)
}

pub fn jump_strength_bonus(adaptation_count: u32) -> f64 {
    MAX_JUMP_STRENGTH_BONUS * progression(adaptation_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    #[test]
    fn tier_tables_match_upstream() {
        assert_eq!(TIER_COUNT, 5);
        assert_eq!(MAX_LEVEL, 8);
        assert_eq!(suffix(0), "wood");
        assert_eq!(suffix(4), "netherite");
        assert_eq!(tool_id(2, "pickaxe"), "minecraft:iron_pickaxe");
        assert_eq!(mining_concept(4), "Fist_Netherite");
        assert_eq!(combat_concept(0), "Combat_FistWood");
        assert_eq!(color(3), 0xFF4FE3D8);
        // Both ladders use the same palette, so one table serves both.
        assert_eq!(COLORS.len(), TIER_COUNT);
        assert_eq!(luck_multiplier(3), 5);
        assert_eq!(luck_multiplier(99), 1);
        assert_eq!(mining_progress_key(0), "fist_mining_0");
        assert_eq!(combat_progress_key(4), "fist_punching_4");
    }

    #[test]
    fn handover_only_at_material_boundaries() {
        assert!(shows_handover(0, true, false));
        assert!(shows_handover(MAX_LEVEL, false, true));
        assert!(!shows_handover(0, false, true));
        assert!(!shows_handover(1, true, true));
        assert!(!shows_handover(MAX_LEVEL - 1, true, false));
        assert!(!shows_handover(MAX_LEVEL, false, false));
    }

    #[test]
    fn mining_reach_tier_walks_maxed_stages() {
        let levels = [8u8, 8, 5, 0, 0];
        assert_eq!(mining_reach_tier(|t| levels[t]), 2);
        // Every stage maxed: the loop stops at Netherite, which is still granted.
        assert_eq!(mining_reach_tier(|_| MAX_LEVEL), TIER_COUNT - 1);
        // Consecutive maxed stages outrank a single high trained stage only
        // when they actually reach it.
        assert_eq!(mining_reach_tier(|t| if t == 1 { 3 } else { 0 }), 1);
        assert_eq!(mining_reach_tier(|_| 0), 0);
    }

    #[test]
    fn mining_reach_tier_grants_a_skipped_higher_stage() {
        // progression stops at Wood (level 7), but Iron was trained directly.
        let levels = [7u8, 0, 2, 0, 0];
        assert_eq!(mining_reach_tier(|t| levels[t]), 2);
    }

    #[test]
    fn combat_tier_thresholds_and_training_handover() {
        assert_eq!(combat_current_tier(|_| 0), -1);
        assert_eq!(combat_training_tier(|_| 0), -1);
        let levels = [8u8, 8, 2, 0, 0];
        assert_eq!(combat_current_tier(|t| levels[t]), 2);
        assert_eq!(combat_training_tier(|t| levels[t]), 2);
        let maxed_wood = [8u8, 0, 0, 0, 0];
        assert_eq!(combat_training_tier(|t| maxed_wood[t]), 1);
        // Netherite is the last stage: maxing it does not invent a sixth.
        assert_eq!(combat_training_tier(|_| MAX_LEVEL), (TIER_COUNT - 1) as i32);
        assert!(is_maxed(MAX_LEVEL));
        assert!(!is_maxed(MAX_LEVEL - 1));
    }

    #[test]
    fn harvest_gate_uses_required_tier() {
        assert!(can_harvest(-1, 0));
        assert!(can_harvest(1, 1));
        assert!(!can_harvest(4, 3));
    }

    #[test]
    fn mining_unlock_needs_max_mine_labor_and_stone() {
        assert!(mining_unlock_allowed(MAX_LEVEL, 1));
        assert!(!mining_unlock_allowed(MAX_LEVEL - 1, 1));
        assert!(!mining_unlock_allowed(MAX_LEVEL, 0));
        assert!(!mining_unlock_allowed(MAX_LEVEL, 2));
    }

    #[test]
    fn combat_kill_gate_rejects_ranged_and_weapons() {
        assert!(combat_kill_trains(true, false, false));
        assert!(combat_kill_trains(true, true, true));
        assert!(!combat_kill_trains(true, false, true));
        assert!(!combat_kill_trains(false, true, true));
        assert!(!combat_kill_trains(false, false, false));
    }

    #[test]
    fn mining_cost_curve_matches_java_rounding() {
        assert_eq!(blocks_for_next_level(0, 0), 10);
        assert_eq!(blocks_for_next_level(0, 1), 10); // exponent clamped at 0
        assert_eq!(blocks_for_next_level(0, 2), 14);
        assert_eq!(blocks_for_next_level(0, 7), 61);
        assert_eq!(blocks_for_next_level(4, 3), 91);
        assert_eq!(blocks_for_next_level(4, 4), 123);
        assert_eq!(blocks_for_next_level(4, 7), 303);
    }

    #[test]
    fn kill_cost_curve_matches_java_rounding() {
        assert_eq!(kills_for_next_level(0, 0), 3);
        assert_eq!(kills_for_next_level(0, 1), 3);
        assert_eq!(kills_for_next_level(0, 2), 5);
        assert_eq!(kills_for_next_level(0, 7), 50);
        assert_eq!(kills_for_next_level(4, 0), 30);
        assert_eq!(kills_for_next_level(4, 7), 503);
    }

    #[test]
    fn progress_banks_partial_and_levels_on_reaching_cost() {
        assert_eq!(progress_after_event(1, 3), TierProgress::NeedMore(2));
        assert_eq!(progress_after_event(2, 3), TierProgress::LevelUp);
        assert_eq!(progress_after_event(0, 1), TierProgress::LevelUp);
    }

    #[test]
    fn adapt_count_counts_union_not_storage_locations() {
        let adapted = ["Drop_A", "Drop_B"];
        let levels = [("Drop_A", 2u8), ("Drop_B", 1), ("Drop_C", 0), ("Drop_D", 3)];
        assert_eq!(adapt_count(adapted, levels), 3);
        assert_eq!(adapt_count([], []), 0);
        assert_eq!(adapt_count(["Drop_A"], [("Drop_A", 8)]), 1);
    }

    #[test]
    fn passive_bonuses_are_linear_and_capped_at_400() {
        assert_eq!(progression(0), 0.0);
        assert!(approx(progression(200), 0.5));
        assert_eq!(progression(DEVELOPMENT_CAP), 1.0);
        assert_eq!(progression(DEVELOPMENT_CAP + 100), 1.0);
        assert!(approx(movement_speed_bonus(0), 0.0));
        assert!(approx(movement_speed_bonus(200), 0.40));
        assert!(approx(movement_speed_bonus(500), 0.80));
        assert!(approx(jump_strength_bonus(200), 0.245));
        assert!(approx(jump_strength_bonus(400), 0.49));
        assert!(approx(jump_strength_bonus(999), 0.49));
        assert!(approx(BASE_JUMP_STRENGTH + jump_strength_bonus(400), 0.91));
    }
}
