//! Wiring layer between the pure P1/P2 systems and the live Horizon hooks.
//!
//! The four pure modules (`synergies`, `fist_ladders`, `existence_mut`,
//! `damage_categories`) expose no state of their own. This module gives the
//! lead a single, testable seam: build a plain snapshot from the player's
//! state, call one `*_from_*` function to get numbers/data, then optionally
//! hand those numbers to a thin `apply_*` helper that owns the only game
//! calls. Nothing here reads globals or caches; the hook owns persistence.
//!
//! # The two level contracts (do not mix them)
//! - Synergy satisfaction and strength use **`levelOrZero`**: a concept the
//!   player switched off is level 0 (see [`crate::synergies::level_or_zero`]).
//! - Fist tiers use **raw** `levels` (see the header of
//!   [`crate::fist_ladders`]): a switched-off but maxed stage is still maxed and
//!   must hand progress to the next stage.
//! - The Fist *mutation* gate is the exception in the other direction:
//!   [`crate::existence_mut::fist_mutation_ready`] requires
//!   `levelOrZero(Mine_Labor)`. `FistProgressSnapshot` therefore carries both
//!   the raw mining levels and an explicitly effective `Mine_Labor` value.
//!
//! Only the `apply_*` helpers at the bottom depend on `mod_api`; every
//! snapshot, resolver and outcome above them is plain data and is unit-tested
//! without a host (tests at the bottom of this file).

use std::collections::{BTreeMap, BTreeSet};

use mod_api::{ActiveEffect, PlayerRef, ServerHost, Text};

use crate::damage_categories::{
    DamageCategory, categorize_with_tags, category_capstone_applies, contact_immunity_applies,
    full_immunity_applies, maxed_immunity_applies, mitigation_level_reduction, type_concept,
};
use crate::existence_mut::{
    ExistenceReward, accumulate_boss_combat, bank_boss_hit, existence_reward, fist_mutation_ready,
    hit_reduction, note_boss_encounter, visible_boss_progress,
};
use crate::fist_ladders::{
    TIER_COUNT, TierProgress, can_harvest, combat_current_tier, combat_training_tier,
    mining_reach_tier, progress_after_event,
};
use crate::synergies::{
    Effect as SynergyEffect, SmeltRule, SynergyState, active_effects, chain_damage, chain_power,
    chain_target_limit, ignite_ticks, level_or_zero, retaliation_damage, slow_amplifier, swim_bonus,
};

// ── 1. Synergy effects ──────────────────────────────────────────────────────

/// Plain, host-free view of the parts of a player's wheel the synergies read.
#[derive(Debug, Default, Clone)]
pub struct SynergySnapshot {
    /// Raw `levels` map. `SynergyState` turns each value into `levelOrZero`.
    pub levels: BTreeMap<String, u8>,
    /// One-time adaptations, including those also present in `levels`.
    pub adapted: BTreeSet<String>,
    /// Concepts switched off in the panel; they neither satisfy nor strengthen.
    pub disabled: BTreeSet<String>,
    /// Upstream `configuredAquaticBonus`, supplied by the host config.
    pub configured_aquatic_bonus: f64,
    /// Damage the wearer just took, for GRAVEBLOOM's retaliation.
    pub damage_taken: f64,
    /// First target's health, for STORMCALL's chained damage.
    pub first_target_health: f32,
}

impl SynergySnapshot {
    /// Build the exact input type the pure synergy module expects.
    pub fn synergy_state(&self) -> SynergyState {
        SynergyState {
            levels: self.levels.clone(),
            adapted: self.adapted.clone(),
            disabled: self.disabled.clone(),
        }
    }

    /// `levelOrZero` for one concept: 0 when the player switched it off.
    pub fn effective_level(&self, concept: &str) -> u8 {
        level_or_zero(&self.synergy_state(), concept)
    }
}

/// A synergy effect with every upstream coefficient already resolved to a
/// number the host can apply without re-deriving strength arithmetic.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolvedEffect {
    Ignite {
        id: &'static str,
        ticks: i32,
        cooldown_ticks: i64,
        strength: f64,
    },
    Slow {
        id: &'static str,
        ticks: i32,
        amplifier: i32,
        strength: f64,
    },
    Swim {
        id: &'static str,
        bonus: f64,
        /// DROWNED_WALTZ needs the Drowning adaptation on top of satisfaction.
        needs_drown_adaptation: bool,
        strength: f64,
    },
    ImpactRadius {
        id: &'static str,
        strength: f64,
    },
    VoidHeal {
        id: &'static str,
        heal: f64,
        strength: f64,
    },
    TargetRefusal {
        id: &'static str,
        distance_sqr: f64,
        strength: f64,
    },
    Retaliation {
        id: &'static str,
        damage: f64,
        strength: f64,
    },
    Goliath {
        id: &'static str,
        ticks: i32,
        smelting: &'static [SmeltRule],
        strength: f64,
    },
    Chain {
        id: &'static str,
        power: f64,
        targets: i32,
        damage: f32,
        strength: f64,
    },
    BlockBreakHeal {
        id: &'static str,
        heal: f64,
        strength: f64,
    },
}

impl ResolvedEffect {
    /// Synergy id from `synergies.rs` (`"ashwalker"`, `"stormcall"`, …).
    pub fn id(&self) -> &'static str {
        match self {
            Self::Ignite { id, .. }
            | Self::Slow { id, .. }
            | Self::Swim { id, .. }
            | Self::ImpactRadius { id, .. }
            | Self::VoidHeal { id, .. }
            | Self::TargetRefusal { id, .. }
            | Self::Retaliation { id, .. }
            | Self::Goliath { id, .. }
            | Self::Chain { id, .. }
            | Self::BlockBreakHeal { id, .. } => id,
        }
    }
}

/// Resolve every active synergy into host-ready numbers, in `ALL` order.
///
/// Satisfaction and strength come from `synergies::active_effects`, i.e. from
/// `levelOrZero`, so a switched-off concept cannot light a synergy.
pub fn synergy_effects_for(snapshot: &SynergySnapshot) -> Vec<ResolvedEffect> {
    active_effects(&snapshot.synergy_state())
        .into_iter()
        .map(|active| {
            let id = active.id;
            let strength = active.strength;
            match active.effect {
                SynergyEffect::IgniteOnHit { base_ticks, min_ticks, cooldown_ticks } => {
                    ResolvedEffect::Ignite {
                        id,
                        ticks: ignite_ticks(base_ticks, min_ticks, strength),
                        cooldown_ticks,
                        strength,
                    }
                }
                SynergyEffect::SlowOnHit { base_ticks, min_ticks, max_amplifier } => {
                    ResolvedEffect::Slow {
                        id,
                        ticks: ignite_ticks(base_ticks, min_ticks, strength),
                        amplifier: slow_amplifier(max_amplifier, strength),
                        strength,
                    }
                }
                SynergyEffect::DrownedWaltz { swim_bonus_base, needs_drown_adaptation } => {
                    ResolvedEffect::Swim {
                        id,
                        bonus: swim_bonus(
                            strength,
                            swim_bonus_base,
                            snapshot.configured_aquatic_bonus,
                        ),
                        needs_drown_adaptation,
                        strength,
                    }
                }
                SynergyEffect::ImpactRadiusMultiplier => {
                    ResolvedEffect::ImpactRadius { id, strength }
                }
                SynergyEffect::VoidDamageToHeal { heal_per_strength } => ResolvedEffect::VoidHeal {
                    id,
                    heal: heal_per_strength * strength,
                    strength,
                },
                SynergyEffect::TargetRefusal { distance_sqr_per_strength } => {
                    ResolvedEffect::TargetRefusal {
                        id,
                        distance_sqr: distance_sqr_per_strength * strength,
                        strength,
                    }
                }
                SynergyEffect::Retaliate { fraction_per_strength } => ResolvedEffect::Retaliation {
                    id,
                    damage: retaliation_damage(
                        snapshot.damage_taken,
                        fraction_per_strength,
                        strength,
                    ),
                    strength,
                },
                SynergyEffect::Goliath { ignite_base_ticks, min_ticks, smelting } => {
                    ResolvedEffect::Goliath {
                        id,
                        ticks: ignite_ticks(ignite_base_ticks, min_ticks, strength),
                        smelting,
                        strength,
                    }
                }
                SynergyEffect::ChainLightning {
                    max_targets,
                    min_targets,
                    damage_fraction,
                    min_damage,
                } => {
                    let power = chain_power(strength);
                    ResolvedEffect::Chain {
                        id,
                        power,
                        targets: chain_target_limit(max_targets, min_targets, power),
                        damage: chain_damage(
                            snapshot.first_target_health,
                            damage_fraction,
                            min_damage,
                            power,
                        ),
                        strength,
                    }
                }
                SynergyEffect::BlockBreakHeal { heal_per_strength } => {
                    ResolvedEffect::BlockBreakHeal {
                        id,
                        heal: heal_per_strength * strength,
                        strength,
                    }
                }
            }
        })
        .collect()
}

// ── 2. Fist tiers ───────────────────────────────────────────────────────────

/// Raw progression counters for the two fist ladders.
#[derive(Debug, Default, Clone)]
pub struct FistProgressSnapshot {
    /// Raw `levels[Mining_*]` levels. Do NOT feed `levelOrZero` here.
    pub mining_levels: [u8; TIER_COUNT],
    /// Raw `levels[Combat_*]` levels. Do NOT feed `levelOrZero` here.
    pub combat_levels: [u8; TIER_COUNT],
    /// `levelOrZero(Mine_Labor)` — 0 when the player disabled it.
    pub mine_labor_level_or_zero: u8,
    /// Raw `levels[Mine_Labor]`, for display only.
    pub mine_labor_raw_level: u8,
    /// Tier of the block just broken (the Fist mutation needs stone, tier 1).
    pub broken_block_tier: i32,
    /// Tier of a block the caller wants to harvest.
    pub requested_block_tier: i32,
    pub mutation_fist_adapted: bool,
    pub blocks_done: u32,
    pub blocks_need: u32,
    pub kills_done: u32,
    pub kills_need: u32,
}

/// Pure result of one fist-progress event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FistTierSnapshot {
    pub mining_tier: usize,
    pub combat_current_tier: i32,
    pub combat_training_tier: i32,
    pub can_harvest_requested: bool,
    pub fist_mutation_unlocked: bool,
    pub block_progress: TierProgress,
    pub kill_progress: TierProgress,
}

/// Resolve both ladders and the Fist mutation gate from **raw** levels.
///
/// The mutation gate is the one `levelOrZero` exception: it consumes
/// `mine_labor_level_or_zero`, never `mine_labor_raw_level`.
pub fn fist_tier_from_progress(snapshot: &FistProgressSnapshot) -> FistTierSnapshot {
    let mining_tier = mining_reach_tier(|tier| snapshot.mining_levels[tier]);
    let combat_current = combat_current_tier(|tier| snapshot.combat_levels[tier]);
    let combat_training = combat_training_tier(|tier| snapshot.combat_levels[tier]);
    let fist_mutation_unlocked = fist_mutation_ready(
        snapshot.mine_labor_level_or_zero,
        snapshot.broken_block_tier,
        snapshot.mutation_fist_adapted,
    );
    FistTierSnapshot {
        mining_tier,
        combat_current_tier: combat_current,
        combat_training_tier: combat_training,
        can_harvest_requested: can_harvest(snapshot.requested_block_tier, mining_tier),
        fist_mutation_unlocked,
        block_progress: progress_after_event(snapshot.blocks_done, snapshot.blocks_need),
        kill_progress: progress_after_event(snapshot.kills_done, snapshot.kills_need),
    }
}

// ── 3. Existence boss encounter / hit banking ──────────────────────────────

/// Plain snapshot of an incoming hit against an Existence boss.
#[derive(Debug, Default, Clone)]
pub struct ExistenceSnapshot {
    pub boss_path: String,
    /// Progress from earlier encounters with the same boss.
    pub progress: BTreeMap<String, u32>,
    pub adapted_bosses: BTreeSet<String>,
    pub nearby_bosses: BTreeSet<String>,
    pub required_ticks: u32,
    /// Stands in for upstream `random.nextInt(55)`.
    pub hit_roll: u32,
    /// Host-computed `active(Existence_<boss>) && !existenceAdapted`.
    pub can_bank: bool,
    pub already_adapted: bool,
}

/// What a single hit tick produces; `progress` is the map to persist back.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ExistenceOutcome {
    pub progress: BTreeMap<String, u32>,
    pub banked_ticks: u32,
    pub hit_bonus_ticks: u32,
    pub rewards: Vec<ExistenceReward>,
    pub visible_progress: BTreeMap<String, u32>,
}

/// Note the encounter, bank the hit, advance nearby bosses and grant rewards
/// for every boss that crossed the threshold.
pub fn existence_outcome_from_damage(snapshot: &ExistenceSnapshot) -> ExistenceOutcome {
    let mut progress = snapshot.progress.clone();
    note_boss_encounter(&mut progress, &snapshot.boss_path);
    let hit_bonus_ticks = hit_reduction(snapshot.required_ticks, snapshot.hit_roll);
    bank_boss_hit(
        &mut progress,
        &snapshot.boss_path,
        snapshot.required_ticks,
        snapshot.hit_roll,
        snapshot.can_bank && !snapshot.already_adapted,
    );
    let banked_ticks = progress
        .get(&snapshot.boss_path)
        .copied()
        .unwrap_or(0);
    // Upstream's bank is skipped entirely for a non-positive requirement;
    // `accumulate_boss_combat` would otherwise complete the boss at 0 ticks.
    let completed = if snapshot.required_ticks == 0 {
        Vec::new()
    } else {
        accumulate_boss_combat(
            &mut progress,
            &snapshot.adapted_bosses,
            &snapshot.nearby_bosses,
            snapshot.required_ticks,
        )
    };
    let rewards = completed.iter().map(|boss| existence_reward(boss)).collect();
    let visible_progress = visible_boss_progress(
        &progress,
        &snapshot.adapted_bosses,
        &snapshot.nearby_bosses,
    );
    ExistenceOutcome {
        progress,
        banked_ticks,
        hit_bonus_ticks,
        rewards,
        visible_progress,
    }
}

// ── 4. Damage category and mitigation ──────────────────────────────────────

/// Plain snapshot of one incoming damage event.
#[derive(Debug, Default, Clone)]
pub struct DamageSnapshot {
    pub damage_type_id: String,
    /// Vanilla-style tags (`minecraft:is_fire`, …); empty for this SDK.
    pub tags: Vec<String>,
    pub active: BTreeSet<String>,
    pub levels: BTreeMap<String, u8>,
    /// Direct living attacker's entity path; `None` for environmental hits.
    pub direct_attacker_path: Option<String>,
    pub max_level: u8,
    pub contact_cutoff: u8,
    pub bypasses_invulnerability: bool,
    pub incoming_damage: f32,
}

/// Pure outcome of the category matching + immunity + mitigation chain.
#[derive(Debug, Clone, PartialEq)]
pub struct DamageOutcome {
    pub category: DamageCategory,
    pub one_time_immunity: bool,
    pub maxed_immunity: bool,
    pub contact_immunity: bool,
    pub capstone: bool,
    pub immune: bool,
    pub reduction: f32,
    pub outgoing_damage: f32,
}

/// Delegate category matching and every immunity/mitigation rule to
/// `damage_categories.rs`; returns the damage to write into the event.
pub fn damage_category_and_mitigation(snapshot: &DamageSnapshot) -> DamageOutcome {
    let tag_refs: Vec<&str> = snapshot.tags.iter().map(String::as_str).collect();
    let category = categorize_with_tags(&snapshot.damage_type_id, &tag_refs);
    let is_active = |concept: &str| snapshot.active.contains(concept);
    let level = |concept: &str| snapshot.levels.get(concept).copied().unwrap_or(0);

    let one_time_immunity = full_immunity_applies(&is_active, category, &snapshot.damage_type_id);
    let maxed_immunity = maxed_immunity_applies(
        &level,
        category,
        &snapshot.damage_type_id,
        snapshot.max_level,
        snapshot.bypasses_invulnerability,
    );
    let contact_immunity = contact_immunity_applies(
        &level,
        snapshot.direct_attacker_path.as_deref(),
        snapshot.max_level,
        snapshot.contact_cutoff,
    );
    let capstone = category_capstone_applies(
        &level,
        category,
        snapshot.max_level,
        snapshot.bypasses_invulnerability,
    );
    let immune = !snapshot.bypasses_invulnerability
        && (one_time_immunity || maxed_immunity || contact_immunity || capstone);

    let ladder_level = type_concept(category).map_or(0, &level);
    let reduction = mitigation_level_reduction(category, ladder_level, snapshot.max_level);
    let outgoing_damage = if immune {
        0.0
    } else {
        (snapshot.incoming_damage * (1.0 - reduction)).max(0.0)
    };
    DamageOutcome {
        category,
        one_time_immunity,
        maxed_immunity,
        contact_immunity,
        capstone,
        immune,
        reduction,
        outgoing_damage,
    }
}

// ── Host-facing helpers (the only `mod_api` calls in this file) ────────────

/// What the host side actually changed, so the caller can log/skip a flush.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AppliedSynergyEffects {
    pub ignited: usize,
    pub slowed: usize,
    pub healed: usize,
}

/// Apply the parts of a resolved effect that a `PlayerRef` can express with
/// pinned SDK calls: fire ticks, slowness and healing.
///
/// Chain targets, retaliation and mob-targeting need target refs/state the
/// hook owns; they stay as data in [`ResolvedEffect`] for the lead to apply.
pub fn apply_synergy_effects(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    effects: &[ResolvedEffect],
) -> AppliedSynergyEffects {
    let mut applied = AppliedSynergyEffects::default();
    let living = host.living(player.entity());
    for effect in effects {
        match effect {
            ResolvedEffect::Ignite { ticks, .. } | ResolvedEffect::Goliath { ticks, .. } => {
                if player.entity().set_remaining_fire_ticks(host, *ticks) {
                    applied.ignited += 1;
                }
            }
            ResolvedEffect::Slow { ticks, amplifier, .. } => {
                if let (Some(living), Some(key)) =
                    (living, host.effect_key("minecraft:slowness"))
                {
                    let instance = ActiveEffect::new(key, *ticks, *amplifier, false, true, true);
                    if living.add_effect(host, &instance) {
                        applied.slowed += 1;
                    }
                }
            }
            ResolvedEffect::VoidHeal { heal, .. }
            | ResolvedEffect::BlockBreakHeal { heal, .. } => {
                if let Some(living) = living {
                    if living.heal(host, *heal as f32) {
                        applied.healed += 1;
                    }
                }
            }
            _ => {}
        }
    }
    applied
}

/// Tell the player which Existence reward is being granted. The lead owns the
/// actual `adapted`/`levels` writes; this helper only reports them.
pub fn apply_existence_reward(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    reward: &ExistenceReward,
) -> bool {
    player
        .tell(
            host,
            Text::literal(format!("[adaptionwheel] existence gained: {}", reward.existence)),
        )
        .unwrap_or(false)
}

/// Show the resolved fist tiers in the action bar. Progress/level writes stay
/// with the hook that owns the player's attachments.
pub fn apply_fist_tier(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    tiers: &FistTierSnapshot,
) -> bool {
    player
        .action_bar(
            host,
            Text::literal(format!(
                "[adaptionwheel] fist: mine {} / combat {} (train {})",
                tiers.mining_tier, tiers.combat_current_tier, tiers.combat_training_tier
            )),
        )
        .unwrap_or(false)
}

/// Show incoming damage after mitigation in the action bar. The hook writes
/// [`DamageOutcome::outgoing_damage`] into its mutable damage container.
pub fn apply_damage_outcome(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    outcome: &DamageOutcome,
) -> bool {
    player
        .action_bar(
            host,
            Text::literal(format!(
                "[adaptionwheel] {:?}: {:.2} dmg ({} reduction)",
                outcome.category, outcome.outgoing_damage, outcome.reduction
            )),
        )
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::existence_mut::MAX_LEVEL as EXISTENCE_MAX_LEVEL;
    use crate::fist_ladders::MAX_LEVEL as FIST_MAX_LEVEL;

    fn approx(a: f64, b: f64) -> bool {
        // f32 -> f64 widening is not exact; 1e-4 still separates all
        // distinct upstream coefficients used here.
        (a - b).abs() < 1e-4
    }

    fn synergy_snapshot() -> SynergySnapshot {
        SynergySnapshot {
            configured_aquatic_bonus: 0.5,
            damage_taken: 100.0,
            first_target_health: 20.0,
            ..SynergySnapshot::default()
        }
    }

    #[test]
    fn synergy_empty_state_has_no_effects() {
        assert!(synergy_effects_for(&synergy_snapshot()).is_empty());
    }

    #[test]
    fn synergy_ashwalker_ignite_uses_level_or_zero() {
        let mut s = synergy_snapshot();
        s.levels.insert("Env_Fire".into(), 1);
        s.adapted.insert("Mutation_Thermal".into());
        s.levels.insert("Mutation_Thermal".into(), FIST_MAX_LEVEL);
        let effects = synergy_effects_for(&s);
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].id(), "ashwalker");
        // strength = 1 + 3*((1/8 + 8/8)/2) = 2.6875 -> 40*2.6875 -> round 108.
        match &effects[0] {
            ResolvedEffect::Ignite { ticks, cooldown_ticks, .. } => {
                assert_eq!(*ticks, 108);
                assert_eq!(*cooldown_ticks, 20);
            }
            other => panic!("expected ignite, got {other:?}"),
        }
    }

    #[test]
    fn synergy_disabled_required_concept_extinguishes_effect() {
        let mut s = synergy_snapshot();
        s.levels.insert("Env_Fire".into(), 1);
        s.levels.insert("Mutation_Thermal".into(), FIST_MAX_LEVEL);
        s.disabled.insert("Mutation_Thermal".into());
        assert_eq!(s.effective_level("Mutation_Thermal"), 0);
        assert!(synergy_effects_for(&s).iter().all(|e| e.id() != "ashwalker"));
    }

    #[test]
    fn synergy_glacierblood_slow_amplifier_is_capped() {
        let mut s = synergy_snapshot();
        s.levels.insert("Type_FREEZE".into(), FIST_MAX_LEVEL);
        s.levels.insert("Env_Ice".into(), FIST_MAX_LEVEL);
        let effects = synergy_effects_for(&s);
        let slow = effects
            .iter()
            .find(|e| e.id() == "glacierblood")
            .expect("glacierblood active");
        match slow {
            ResolvedEffect::Slow { ticks, amplifier, .. } => {
                assert_eq!(*ticks, 80); // both requirements level 8 -> strength 4
                assert_eq!(*amplifier, 4);
            }
            other => panic!("expected slow, got {other:?}"),
        }
    }

    #[test]
    fn synergy_drowned_waltz_swim_bonus_keeps_second_gate() {
        let mut s = synergy_snapshot();
        s.levels.insert("Env_Liquid".into(), FIST_MAX_LEVEL);
        s.levels.insert("Mutation_Aquatic".into(), FIST_MAX_LEVEL);
        let swim = synergy_effects_for(&s)
            .into_iter()
            .find(|e| e.id() == "drownedwaltz")
            .expect("drowned waltz active");
        match swim {
            ResolvedEffect::Swim { bonus, needs_drown_adaptation, .. } => {
                assert!(approx(bonus, 2.0 * 4.0)); // (0.5 + 1.5) * strength 4
                assert!(needs_drown_adaptation);
            }
            other => panic!("expected swim, got {other:?}"),
        }
    }

    #[test]
    fn synergy_stormcall_chain_numbers_use_truncated_power() {
        let mut s = synergy_snapshot();
        s.levels.insert("Type_LIGHTNING".into(), 5);
        s.levels.insert("Existence_test_boss".into(), 5);
        let chain = synergy_effects_for(&s)
            .into_iter()
            .find(|e| e.id() == "stormcall")
            .expect("stormcall active");
        match chain {
            ResolvedEffect::Chain { power, targets, damage, .. } => {
                assert_eq!(power, 2.0); // strength 2.75 truncated
                assert_eq!(targets, 2);
                assert!(approx(f64::from(damage), 20.0 * 0.08 * 2.0));
            }
            other => panic!("expected chain, got {other:?}"),
        }
    }

    #[test]
    fn synergy_gravebloom_retaliation_uses_damage_taken() {
        let mut s = synergy_snapshot();
        s.damage_taken = 50.0;
        s.levels.insert("Env_Thorns".into(), 1);
        s.levels.insert("Contact_minecraft:zombie".into(), FIST_MAX_LEVEL);
        let retaliate = synergy_effects_for(&s)
            .into_iter()
            .find(|e| e.id() == "gravebloom")
            .expect("gravebloom active");
        match retaliate {
            ResolvedEffect::Retaliation { damage, .. } => {
                // The family gate contributes no strength: only Env_Thorns at
                // level 1 counts -> strength 1 + 3*(1/8) = 1.375.
                assert!(approx(damage, 50.0 * 0.12 * 1.375));
            }
            other => panic!("expected retaliation, got {other:?}"),
        }
    }

    #[test]
    fn synergy_astral_mine_is_a_maxed_gate() {
        let mut s = synergy_snapshot();
        s.levels.insert("Mine_Labor".into(), 7);
        s.adapted.insert("Mutation_Fist".into());
        assert!(synergy_effects_for(&s).is_empty());
        s.levels.insert("Mine_Labor".into(), FIST_MAX_LEVEL);
        let heal = synergy_effects_for(&s)
            .into_iter()
            .find(|e| e.id() == "astralmine")
            .expect("astral mine active");
        match heal {
            ResolvedEffect::BlockBreakHeal { heal, .. } => {
                // Both requirements are gates/zero level: wheel breadth = 2/60.
                assert!(approx(heal, 0.25 * (1.0 + 3.0 * 2.0 / 60.0)));
            }
            other => panic!("expected heal, got {other:?}"),
        }
    }

    #[test]
    fn synergy_goliath_uses_maxed_fist_iron_and_smelting_table() {
        let mut s = synergy_snapshot();
        s.levels.insert("Fist_Iron".into(), FIST_MAX_LEVEL);
        s.levels.insert("Mutation_Thermal".into(), FIST_MAX_LEVEL);
        let goliath = synergy_effects_for(&s)
            .into_iter()
            .find(|e| e.id() == "goliath")
            .expect("goliath active");
        match goliath {
            ResolvedEffect::Goliath { ticks, smelting, .. } => {
                assert_eq!(ticks, 80); // maxed gate + level-8 thermal -> strength 4
                assert_eq!(smelting[0].input, "minecraft:raw_iron");
            }
            other => panic!("expected goliath, got {other:?}"),
        }
    }

    #[test]
    fn synergy_unmaker_heals_via_existence_family() {
        let mut s = synergy_snapshot();
        s.levels.insert("Env_Void".into(), FIST_MAX_LEVEL);
        s.levels.insert("Existence_minecraft:ender_dragon".into(), 1);
        let heal = synergy_effects_for(&s)
            .into_iter()
            .find(|e| e.id() == "unmaker")
            .expect("unmaker active");
        match heal {
            ResolvedEffect::VoidHeal { heal, .. } => {
                // Existence family is a gate: only Env_Void at level 8 counts.
                assert!(approx(heal, 2.0 * 4.0));
            }
            other => panic!("expected void heal, got {other:?}"),
        }
    }

    fn fist_snapshot() -> FistProgressSnapshot {
        FistProgressSnapshot {
            blocks_need: 10,
            kills_need: 3,
            requested_block_tier: 1,
            ..FistProgressSnapshot::default()
        }
    }

    #[test]
    fn fist_mining_tier_walks_consecutive_maxed_stages() {
        let mut s = fist_snapshot();
        s.mining_levels = [FIST_MAX_LEVEL; TIER_COUNT];
        s.requested_block_tier = 4;
        assert_eq!(fist_tier_from_progress(&s).mining_tier, 4);
    }

    #[test]
    fn fist_raw_off_stage_keeps_its_tier() {
        let mut s = fist_snapshot();
        s.mining_levels = [FIST_MAX_LEVEL, FIST_MAX_LEVEL, 3, 0, 0];
        let tiers = fist_tier_from_progress(&s);
        assert_eq!(tiers.mining_tier, 2);
        assert_eq!(tiers.mining_tier, crate::fist_ladders::mining_reach_tier(|t| s.mining_levels[t]));
        assert!(tiers.can_harvest_requested); // tier 2 can harvest a tier-1 block
    }

    #[test]
    fn fist_combat_tier_and_training_handover_use_raw_levels() {
        let mut s = fist_snapshot();
        s.combat_levels = [FIST_MAX_LEVEL, 0, 0, 0, 0];
        let tiers = fist_tier_from_progress(&s);
        assert_eq!(tiers.combat_current_tier, 0);
        assert_eq!(tiers.combat_training_tier, 1);
    }

    #[test]
    fn fist_mutation_gate_uses_level_or_zero_not_raw_level() {
        let mut s = fist_snapshot();
        s.mine_labor_raw_level = FIST_MAX_LEVEL;
        s.mine_labor_level_or_zero = 0;
        s.broken_block_tier = 1;
        assert!(!fist_tier_from_progress(&s).fist_mutation_unlocked);
        s.mine_labor_level_or_zero = FIST_MAX_LEVEL;
        assert!(fist_tier_from_progress(&s).fist_mutation_unlocked);
    }

    #[test]
    fn fist_progress_after_event_grants_one_level() {
        let mut s = fist_snapshot();
        s.blocks_done = 9;
        s.kills_done = 2;
        assert_eq!(fist_tier_from_progress(&s).block_progress, TierProgress::LevelUp);
        assert_eq!(fist_tier_from_progress(&s).kill_progress, TierProgress::LevelUp);
        s.blocks_done = 1;
        assert_eq!(
            fist_tier_from_progress(&s).block_progress,
            TierProgress::NeedMore(2)
        );
    }

    fn existence_snapshot() -> ExistenceSnapshot {
        ExistenceSnapshot {
            boss_path: "minecraft:ender_dragon".into(),
            required_ticks: 4_800,
            hit_roll: 0,
            can_bank: true,
            ..ExistenceSnapshot::default()
        }
    }

    #[test]
    fn existence_hit_banks_then_completes_and_rewards() {
        let mut s = existence_snapshot();
        s.nearby_bosses.insert(s.boss_path.clone());
        s.progress.insert(s.boss_path.clone(), 4_799);
        let outcome = existence_outcome_from_damage(&s);
        assert_eq!(outcome.banked_ticks, 4_800);
        assert_eq!(outcome.rewards.len(), 1);
        assert_eq!(outcome.rewards[0].existence, "Existence_minecraft:ender_dragon");
        assert!(outcome.rewards[0].maxed_levels.contains(&"Type_EXPLOSION".to_string()));
        assert!(!outcome.progress.contains_key(&s.boss_path));
    }

    #[test]
    fn existence_cannot_bank_when_gate_is_false() {
        let mut s = existence_snapshot();
        s.can_bank = false;
        let outcome = existence_outcome_from_damage(&s);
        assert_eq!(outcome.banked_ticks, 0);
        assert!(outcome.rewards.is_empty());
        assert_eq!(outcome.progress.get(&s.boss_path), Some(&0));
    }

    #[test]
    fn existence_hit_reduction_is_always_at_least_one() {
        let s = existence_snapshot();
        let outcome = existence_outcome_from_damage(&s);
        assert!(outcome.hit_bonus_ticks >= 1);
        assert!(outcome.hit_bonus_ticks <= 6 + 54);
    }

    #[test]
    fn existence_adapted_boss_progress_is_dropped() {
        let mut s = existence_snapshot();
        s.progress.insert("minecraft:wither".into(), 42);
        s.adapted_bosses.insert("minecraft:wither".into());
        let outcome = existence_outcome_from_damage(&s);
        assert!(!outcome.progress.contains_key("minecraft:wither"));
    }

    #[test]
    fn existence_visible_progress_needs_presence() {
        let mut s = existence_snapshot();
        s.progress.insert("draconicevolution:draconic_guardian".into(), 10);
        let outcome = existence_outcome_from_damage(&s);
        assert!(!outcome.visible_progress.contains_key("draconicevolution:draconic_guardian"));
        s.nearby_bosses.insert("draconicevolution:draconic_guardian".into());
        let outcome = existence_outcome_from_damage(&s);
        // accumulate_boss_combat advances a nearby boss by one tick this turn.
        assert_eq!(
            outcome.visible_progress.get("draconicevolution:draconic_guardian"),
            Some(&11)
        );
    }

    #[test]
    fn existence_zero_requirement_never_completes() {
        let mut s = existence_snapshot();
        s.required_ticks = 0;
        s.nearby_bosses.insert(s.boss_path.clone());
        assert!(existence_outcome_from_damage(&s).rewards.is_empty());
    }

    fn damage_snapshot() -> DamageSnapshot {
        DamageSnapshot {
            damage_type_id: "minecraft:arrow".into(),
            incoming_damage: 10.0,
            max_level: EXISTENCE_MAX_LEVEL,
            ..DamageSnapshot::default()
        }
    }

    #[test]
    fn damage_tags_outrank_id_classification() {
        let mut s = damage_snapshot();
        s.tags.push("minecraft:is_fall".into());
        assert_eq!(
            damage_category_and_mitigation(&s).category,
            DamageCategory::Fall
        );
        s.tags.clear();
        assert_eq!(
            damage_category_and_mitigation(&s).category,
            DamageCategory::Projectile
        );
    }

    #[test]
    fn damage_lava_uses_lava_immunity_not_fire() {
        let mut s = damage_snapshot();
        s.damage_type_id = "minecraft:lava".into();
        s.active.insert("Env_Fire".into());
        assert!(!damage_category_and_mitigation(&s).one_time_immunity);
        s.active.insert("Env_Lava".into());
        assert!(damage_category_and_mitigation(&s).immune);
        assert_eq!(damage_category_and_mitigation(&s).outgoing_damage, 0.0);
    }

    #[test]
    fn damage_bypass_never_nullified() {
        let mut s = damage_snapshot();
        s.active.insert("Env_Fire".into());
        s.damage_type_id = "minecraft:in_fire".into();
        s.bypasses_invulnerability = true;
        let outcome = damage_category_and_mitigation(&s);
        assert!(!outcome.immune);
        assert!(!outcome.maxed_immunity);
        assert!(!outcome.capstone);
    }

    #[test]
    fn damage_contact_immunity_needs_direct_attacker() {
        let mut s = damage_snapshot();
        s.levels.insert("Contact_minecraft:zombie".into(), EXISTENCE_MAX_LEVEL);
        assert!(!damage_category_and_mitigation(&s).contact_immunity);
        s.direct_attacker_path = Some("minecraft:zombie".into());
        assert!(damage_category_and_mitigation(&s).contact_immunity);
    }

    #[test]
    fn damage_capstone_only_for_projectile_and_other() {
        let mut s = damage_snapshot();
        s.levels.insert("Type_PROJECTILE".into(), EXISTENCE_MAX_LEVEL);
        assert!(damage_category_and_mitigation(&s).capstone);
        s.damage_type_id = "minecraft:mob_attack".into();
        s.levels.insert("Type_MOB".into(), EXISTENCE_MAX_LEVEL);
        assert!(!damage_category_and_mitigation(&s).capstone);
    }

    #[test]
    fn damage_mitigation_scales_outgoing_damage() {
        let mut s = damage_snapshot();
        s.levels.insert("Type_PROJECTILE".into(), 2);
        let outcome = damage_category_and_mitigation(&s);
        assert!(approx(f64::from(outcome.reduction), 0.2));
        assert!(approx(f64::from(outcome.outgoing_damage), 8.0));
        assert!(!outcome.immune);
    }

    #[test]
    fn resolved_effect_id_is_stable_for_every_variant() {
        let effects = vec![
            ResolvedEffect::Ignite { id: "a", ticks: 1, cooldown_ticks: 1, strength: 1.0 },
            ResolvedEffect::Slow { id: "b", ticks: 1, amplifier: 1, strength: 1.0 },
            ResolvedEffect::Swim { id: "c", bonus: 1.0, needs_drown_adaptation: false, strength: 1.0 },
            ResolvedEffect::ImpactRadius { id: "d", strength: 1.0 },
            ResolvedEffect::VoidHeal { id: "e", heal: 1.0, strength: 1.0 },
            ResolvedEffect::TargetRefusal { id: "f", distance_sqr: 1.0, strength: 1.0 },
            ResolvedEffect::Retaliation { id: "g", damage: 1.0, strength: 1.0 },
            ResolvedEffect::Goliath { id: "h", ticks: 1, smelting: &[], strength: 1.0 },
            ResolvedEffect::Chain { id: "i", power: 1.0, targets: 1, damage: 1.0, strength: 1.0 },
            ResolvedEffect::BlockBreakHeal { id: "j", heal: 1.0, strength: 1.0 },
        ];
        let ids: Vec<_> = effects.iter().map(ResolvedEffect::id).collect();
        assert_eq!(ids, ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"]);
    }
}
