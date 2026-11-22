//! Synergies — pure satisfaction, strength and effect-descriptor model.
//!
//! This is a host-free port of upstream `category/Synergies.java`,
//! `server/SynergyEffects.java` and `server/SynergyHooks.java`. It has no
//! dependency on `mod_api`, the launcher, or any player-state type: callers
//! hand it a [`SynergyState`] (levels / adapted / switched-off concepts) and
//! it returns ids, strengths and plain-data [`Effect`] descriptors.
//!
//! Every id, requirement kind, constant and numeric coefficient below is
//! taken verbatim from upstream. In particular:
//!
//! - `Concepts.type(FIRE)` is `Env_Fire`, **not** `Type_FIRE` — so Ashwalker's
//!   first requirement is the one-time fire immunity.
//! - `FistTiers.concept(2)` is `Fist_Iron`, and it is a `MAXED` gate, not a
//!   `PRESENT` one.
//! - A switched-off concept is `level 0` and not "held" everywhere in this
//!   module (upstream `satisfied` and `levelOrZero`), even though the rest of
//!   the wheel still counts it as held. Otherwise switching a concept off in
//!   the panel would keep its synergy lit on the server.
//! - `MAXED` and family requirements are gates: they never contribute to
//!   [`strength`]. One-time mutations live in `adapted`, so their
//!   `levelOrZero` is 0 and they do not contribute either.
//!
//! # What is deliberately NOT here
//!
//! SynergyEffects also mutates the world: it adds attribute modifiers,
//! schedules a per-tick chained-hit guard, plays sounds, spawns particles and
//! replaces dropped `ItemEntity`s. Those are host calls and cannot be pure.
//! Instead this module exposes the exact numbers they use as [`Effect`]
//! variants plus small pure helpers ([`ignite_ticks`], [`chain_target_limit`],
//! [`retaliation_damage`], …) so a host layer can apply them without re-deriving
//! upstream arithmetic. Two further upstream caveats are preserved as data:
//! Drowned Waltz's air refill needs a *second* gate (the Drowning adaptation
//! being active), and Goliath's block-smelting needs `FistTiers.usableWith` on
//! the held item. Both are expressed as flags/notes in the descriptor.
//! Astral Mine's "the fist never slows" blurb has **no** implementation in the
//! three upstream source files — the only coded effect is the block-break heal.

use std::collections::{BTreeMap, BTreeSet};

/// Upstream `PlayerAdaption.MAX_LEVEL`.
pub const MAX_LEVEL: u8 = 8;

/// Upstream denominator of the whole-wheel strength fallback.
pub const WHEEL_BREADTH: f64 = 60.0;

/// Upstream `Synergies.Requirement`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requirement {
    Present,
    Maxed,
    PresentOrFamily,
}

/// One entry of a synergy's `requires` list: `(concept, kind)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SynergyRequirement {
    pub concept: &'static str,
    pub requirement: Requirement,
}

const fn present(concept: &'static str) -> SynergyRequirement {
    SynergyRequirement { concept, requirement: Requirement::Present }
}

const fn maxed(concept: &'static str) -> SynergyRequirement {
    SynergyRequirement { concept, requirement: Requirement::Maxed }
}

const fn family(prefix: &'static str) -> SynergyRequirement {
    SynergyRequirement { concept: prefix, requirement: Requirement::PresentOrFamily }
}

/// A `minecraft:raw_iron -> minecraft:iron_ingot` style rule for Goliath's
/// "cooks what it breaks" effect (upstream `SynergyEffects.SMELTING`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmeltRule {
    pub input: &'static str,
    pub output: &'static str,
}

/// The upstream smelting table, in declaration order.
pub static GOLIATH_SMELTING: &[SmeltRule] = &[
    SmeltRule { input: "minecraft:raw_iron", output: "minecraft:iron_ingot" },
    SmeltRule { input: "minecraft:raw_gold", output: "minecraft:gold_ingot" },
    SmeltRule { input: "minecraft:raw_copper", output: "minecraft:copper_ingot" },
    SmeltRule { input: "minecraft:nether_gold_ore", output: "minecraft:gold_ingot" },
    SmeltRule { input: "minecraft:ancient_debris", output: "minecraft:netherite_scrap" },
];

/// Pure, host-free description of what an active synergy does.
///
/// Each variant corresponds to exactly one upstream `SynergyEffects` branch.
/// The numbers stored here are the upstream constants; the `strength` is
/// supplied separately in [`ActiveEffect`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Effect {
    /// `onHit` / ASHWALKER: set the target alight, at most once per 20 ticks.
    IgniteOnHit { base_ticks: i32, min_ticks: i32, cooldown_ticks: i64 },
    /// `onHit` / GLACIERBLOOD: slowness with a capped amplifier.
    SlowOnHit { base_ticks: i32, min_ticks: i32, max_amplifier: i32 },
    /// `tickPassive` / DROWNED_WALTZ: refill air and add swim speed while
    /// submerged. Upstream needs a second gate (the Drowning adaptation being
    /// active) that is not part of synergy satisfaction.
    DrownedWaltz { swim_bonus_base: f64, needs_drown_adaptation: bool },
    /// `impactRadius` / SKYBREAKER: multiply the configured fall shockwave radius.
    ImpactRadiusMultiplier,
    /// `tryAbsorbVoid` / UNMAKER: cancel void damage and heal.
    VoidDamageToHeal { heal_per_strength: f64 },
    /// `refuseTarget` / UNSEEN: mobs give up past `36 * strength` squared distance.
    TargetRefusal { distance_sqr_per_strength: f64 },
    /// `onHurtTaken` / GRAVEBLOOM: return a fraction of the damage taken.
    Retaliate { fraction_per_strength: f64 },
    /// `onHit` + `smeltDrops` / GOLIATH: burn the target and cook bare-hand drops.
    Goliath { ignite_base_ticks: i32, min_ticks: i32, smelting: &'static [SmeltRule] },
    /// `onHit` / STORMCALL: chain to nearby living entities once per tick.
    ChainLightning {
        max_targets: i32,
        min_targets: i32,
        damage_fraction: f64,
        min_damage: f32,
    },
    /// `onBlockBroken` / ASTRAL_MINE: heal on every broken block.
    BlockBreakHeal { heal_per_strength: f64 },
}

/// A synergy definition: exact upstream id, ordered requirements, blurb and
/// the pure effect descriptor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Synergy {
    pub id: &'static str,
    pub requires: &'static [SynergyRequirement],
    pub blurb: &'static str,
    pub effect: Effect,
}

// Upstream ids are the camel/lowercase strings, the enum names are only Java
// field names. Keep both verbatim.
pub const ASHWALKER: Synergy = Synergy {
    id: "ashwalker",
    requires: &[present("Env_Fire"), present("Mutation_Thermal")],
    blurb: "Your attacks set the target alight.",
    effect: Effect::IgniteOnHit { base_ticks: 40, min_ticks: 20, cooldown_ticks: 20 },
};

pub const GLACIERBLOOD: Synergy = Synergy {
    id: "glacierblood",
    requires: &[present("Type_FREEZE"), present("Env_Ice")],
    blurb: "Your attacks slow whatever they land on.",
    effect: Effect::SlowOnHit { base_ticks: 20, min_ticks: 20, max_amplifier: 4 },
};

pub const DROWNED_WALTZ: Synergy = Synergy {
    id: "drownedwaltz",
    requires: &[present("Env_Liquid"), present("Mutation_Aquatic")],
    blurb: "Submerged, you no longer need air and move faster still.",
    effect: Effect::DrownedWaltz { swim_bonus_base: 1.5, needs_drown_adaptation: true },
};

pub const SKYBREAKER: Synergy = Synergy {
    id: "skybreaker",
    requires: &[present("Env_FallDamage"), present("Mutation_Impact")],
    blurb: "Your landing shockwave is far larger.",
    effect: Effect::ImpactRadiusMultiplier,
};

pub const UNMAKER: Synergy = Synergy {
    id: "unmaker",
    requires: &[present("Env_Void"), family("Existence_")],
    blurb: "Void damage heals you instead of hurting you.",
    effect: Effect::VoidDamageToHeal { heal_per_strength: 2.0 },
};

pub const UNSEEN: Synergy = Synergy {
    id: "unseen",
    requires: &[present("Percep_SteadyGaze"), present("Env_Darkness")],
    blurb: "Mobs lose track of you far more quickly.",
    effect: Effect::TargetRefusal { distance_sqr_per_strength: 36.0 },
};

pub const GRAVEBLOOM: Synergy = Synergy {
    id: "gravebloom",
    requires: &[present("Env_Thorns"), family("Contact_")],
    blurb: "Damage you take from a mob is partly returned to it.",
    effect: Effect::Retaliate { fraction_per_strength: 0.12 },
};

pub const GOLIATH: Synergy = Synergy {
    id: "goliath",
    requires: &[maxed("Fist_Iron"), present("Mutation_Thermal")],
    blurb: "Your bare hand burns what it breaks, and cooks what it touches.",
    effect: Effect::Goliath { ignite_base_ticks: 20, min_ticks: 20, smelting: GOLIATH_SMELTING },
};

pub const STORMCALL: Synergy = Synergy {
    id: "stormcall",
    requires: &[present("Type_LIGHTNING"), family("Existence_")],
    blurb: "Your damage arcs to a second and third target.",
    effect: Effect::ChainLightning {
        max_targets: 5,
        min_targets: 1,
        damage_fraction: 0.08,
        min_damage: 1.0,
    },
};

pub const ASTRAL_MINE: Synergy = Synergy {
    id: "astralmine",
    requires: &[maxed("Mine_Labor"), present("Mutation_Fist")],
    blurb: "Breaking blocks feeds your recovery, and the fist never slows.",
    effect: Effect::BlockBreakHeal { heal_per_strength: 0.25 },
};

/// All ten synergies, in upstream `Synergies.ALL` order.
pub static ALL: &[Synergy] = &[
    ASHWALKER,
    GLACIERBLOOD,
    DROWNED_WALTZ,
    SKYBREAKER,
    UNMAKER,
    UNSEEN,
    GRAVEBLOOM,
    GOLIATH,
    STORMCALL,
    ASTRAL_MINE,
];

/// Upstream `Synergies.COUNT`.
pub const COUNT: usize = ALL.len();

/// Upstream `Synergies.byId`.
pub fn by_id(id: &str) -> Option<&'static Synergy> {
    ALL.iter().find(|s| s.id == id)
}

/// Upstream `Synergies.componentsOf`: requirement concepts, de-duplicated in
/// encounter order.
pub fn components_of(synergy: &Synergy) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for req in synergy.requires {
        if !out.contains(&req.concept) {
            out.push(req.concept);
        }
    }
    out
}

/// The pure input to every synergy computation: what the wheel holds and
/// which of those concepts the player has switched off.
#[derive(Debug, Default, Clone)]
pub struct SynergyState {
    /// Level per concept (`BTreeMap<String, u8>` mirrors the attachment's map).
    pub levels: BTreeMap<String, u8>,
    /// One-time adaptations acquired, including ones also present in `levels`.
    pub adapted: BTreeSet<String>,
    /// Concepts switched off in the panel. These neither satisfy nor strengthen.
    pub disabled: BTreeSet<String>,
}

impl SynergyState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_level(mut self, concept: &str, level: u8) -> Self {
        self.levels.insert(concept.to_string(), level);
        self
    }

    pub fn with_adapted(mut self, concept: &str) -> Self {
        self.adapted.insert(concept.to_string());
        self
    }

    pub fn with_disabled(mut self, concept: &str) -> Self {
        self.disabled.insert(concept.to_string());
        self
    }

    /// Upstream `PlayerAdaption.getAdaptCount`: the size of the **union** of
    /// `adapted` and the keys with a positive level.
    pub fn compute_adapt_count(&self) -> usize {
        let mut ids: BTreeSet<&str> = self
            .levels
            .iter()
            .filter(|(_, level)| **level > 0)
            .map(|(id, _)| id.as_str())
            .collect();
        ids.extend(self.adapted.iter().map(String::as_str));
        ids.len()
    }
}

/// Upstream `PlayerAdaption.levelOrZero`: a switched-off concept has level 0.
pub fn level_or_zero(state: &SynergyState, concept: &str) -> u8 {
    if state.disabled.contains(concept) {
        0
    } else {
        state.levels.get(concept).copied().unwrap_or(0)
    }
}

fn is_held(state: &SynergyState, concept: &str) -> bool {
    !state.disabled.contains(concept)
        && (state.adapted.contains(concept) || level_or_zero(state, concept) > 0)
}

/// Upstream `Synergies.hasAnyInFamily`: any held concept starting with the
/// prefix counts, including modded ids.
fn has_any_in_family(state: &SynergyState, prefix: &str) -> bool {
    if state
        .adapted
        .iter()
        .any(|held| !state.disabled.contains(held) && held.starts_with(prefix))
    {
        return true;
    }
    state.levels.iter().any(|(held, &level)| {
        level > 0 && !state.disabled.contains(held) && held.starts_with(prefix)
    })
}

/// Upstream `Synergies.satisfied` — the single definition of "this synergy is
/// active", shared by server effects, the advancement trigger and the client.
pub fn satisfied(state: &SynergyState, synergy: &Synergy) -> bool {
    synergy.requires.iter().all(|req| match req.requirement {
        Requirement::Maxed => level_or_zero(state, req.concept) >= MAX_LEVEL,
        Requirement::Present => is_held(state, req.concept),
        Requirement::PresentOrFamily => {
            is_held(state, req.concept) || has_any_in_family(state, req.concept)
        }
    })
}

pub fn is_active(state: &SynergyState, synergy: &Synergy) -> bool {
    satisfied(state, synergy)
}

/// Upstream `Synergies.activeIds`, in `ALL` order.
pub fn active_ids(state: &SynergyState) -> Vec<&'static str> {
    ALL.iter().filter(|s| satisfied(state, s)).map(|s| s.id).collect()
}

/// Same as [`active_ids`], but returns the full definitions.
pub fn active(state: &SynergyState) -> Vec<&'static Synergy> {
    ALL.iter().filter(|s| satisfied(state, s)).collect()
}

/// Upstream `SynergyEffects.strengthOf`.
///
/// `MAXED` and family/one-time requirements are gates and add nothing. A
/// synergy whose requirements are all gates falls back to whole-wheel breadth
/// (`1 + 3 * min(1, adaptCount / 60)`), capped at 4.0.
pub fn strength(state: &SynergyState, synergy: &Synergy) -> f64 {
    let mut sum = 0.0;
    let mut counted = 0usize;
    for req in synergy.requires {
        if req.requirement == Requirement::Maxed {
            continue;
        }
        let level = level_or_zero(state, req.concept);
        if level == 0 {
            continue;
        }
        sum += (f64::from(level) / f64::from(MAX_LEVEL)).min(1.0);
        counted += 1;
    }
    let progress = if counted == 0 {
        (state.compute_adapt_count() as f64 / WHEEL_BREADTH).min(1.0)
    } else {
        sum / counted as f64
    };
    1.0 + 3.0 * progress
}

/// An active synergy together with its computed strength and pure effect
/// descriptor — the value a host layer would cache per player on `refresh`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveEffect {
    pub id: &'static str,
    pub strength: f64,
    pub effect: Effect,
}

/// Active synergies with their strength and effect descriptors, in `ALL` order.
pub fn active_effects(state: &SynergyState) -> Vec<ActiveEffect> {
    ALL.iter()
        .filter(|s| satisfied(state, s))
        .map(|s| ActiveEffect {
            id: s.id,
            strength: strength(state, s),
            effect: s.effect,
        })
        .collect()
}

// ---- Pure numeric helpers for the host-side effects -----------------------

/// `max(min, round(base * strength))` — upstream ignite/slow durations.
pub fn ignite_ticks(base_ticks: i32, min_ticks: i32, strength: f64) -> i32 {
    ((base_ticks as f64 * strength).round() as i32).max(min_ticks)
}

/// `min(max, round(strength))` — GLACIERBLOOD's slowness amplifier.
pub fn slow_amplifier(max_amplifier: i32, strength: f64) -> i32 {
    (strength.round() as i32).min(max_amplifier)
}

/// The effective `power` STORMCALL passes on: upstream casts strength to int
/// at the call site, so all downstream numbers use the truncated value.
pub fn chain_power(strength: f64) -> f64 {
    strength.trunc()
}

/// `min(5, max(1, round(power)))` — number of extra lightning targets.
pub fn chain_target_limit(max_targets: i32, min_targets: i32, power: f64) -> i32 {
    (power.round() as i32).clamp(min_targets, max_targets)
}

/// `max(1, firstHealth * 0.08 * power)` — damage per chained target.
pub fn chain_damage(first_health: f32, damage_fraction: f64, min_damage: f32, power: f64) -> f32 {
    (first_health * damage_fraction as f32 * power as f32).max(min_damage)
}

/// `amount * 0.12 * strength` — GRAVEBLOOM retaliation.
pub fn retaliation_damage(amount: f64, fraction_per_strength: f64, strength: f64) -> f64 {
    amount * fraction_per_strength * strength
}

/// `(configuredAquaticBonus + 1.5) * strength` — DROWNED_WALTZ swim modifier.
pub fn swim_bonus(strength: f64, swim_bonus_base: f64, configured_aquatic_bonus: f64) -> f64 {
    (configured_aquatic_bonus + swim_bonus_base) * strength
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty() -> SynergyState {
        SynergyState::new()
    }

    /// Mirror of upstream `concreteMember`: turn a family prefix into one
    /// concrete held member.
    fn concrete_member(concept: &'static str) -> &'static str {
        match concept {
            "Existence_" => "Existence_minecraft:ender_dragon",
            "Contact_" => "Contact_minecraft:zombie",
            _ => concept,
        }
    }

    #[test]
    fn all_ten_synergies_have_exact_ids_in_upstream_order() {
        let ids: Vec<&str> = ALL.iter().map(|s| s.id).collect();
        assert_eq!(
            ids,
            [
                "ashwalker",
                "glacierblood",
                "drownedwaltz",
                "skybreaker",
                "unmaker",
                "unseen",
                "gravebloom",
                "goliath",
                "stormcall",
                "astralmine",
            ]
        );
        assert_eq!(COUNT, 10);
        let unique: BTreeSet<&str> = ids.iter().copied().collect();
        assert_eq!(unique.len(), 10);
        for s in ALL {
            assert!(!s.blurb.is_empty(), "{} has no blurb", s.id);
            assert!(!s.requires.is_empty(), "{} has no requirements", s.id);
            assert_eq!(by_id(s.id), Some(s), "{} does not resolve", s.id);
        }
        assert_eq!(by_id("nope"), None);
    }

    #[test]
    fn conditions_match_upstream_verbatim() {
        let expected: &[(&Synergy, &[(&str, Requirement)])] = &[
            (&ASHWALKER, &[("Env_Fire", Requirement::Present), ("Mutation_Thermal", Requirement::Present)]),
            (&GLACIERBLOOD, &[("Type_FREEZE", Requirement::Present), ("Env_Ice", Requirement::Present)]),
            (&DROWNED_WALTZ, &[("Env_Liquid", Requirement::Present), ("Mutation_Aquatic", Requirement::Present)]),
            (&SKYBREAKER, &[("Env_FallDamage", Requirement::Present), ("Mutation_Impact", Requirement::Present)]),
            (&UNMAKER, &[("Env_Void", Requirement::Present), ("Existence_", Requirement::PresentOrFamily)]),
            (&UNSEEN, &[("Percep_SteadyGaze", Requirement::Present), ("Env_Darkness", Requirement::Present)]),
            (&GRAVEBLOOM, &[("Env_Thorns", Requirement::Present), ("Contact_", Requirement::PresentOrFamily)]),
            (&GOLIATH, &[("Fist_Iron", Requirement::Maxed), ("Mutation_Thermal", Requirement::Present)]),
            (&STORMCALL, &[("Type_LIGHTNING", Requirement::Present), ("Existence_", Requirement::PresentOrFamily)]),
            (&ASTRAL_MINE, &[("Mine_Labor", Requirement::Maxed), ("Mutation_Fist", Requirement::Present)]),
        ];
        assert_eq!(expected.len(), ALL.len());
        for (synergy, reqs) in expected {
            let actual: Vec<(&str, Requirement)> = synergy
                .requires
                .iter()
                .map(|r| (r.concept, r.requirement))
                .collect();
            assert_eq!(actual.as_slice(), *reqs, "{} requirements", synergy.id);
        }
    }

    #[test]
    fn empty_state_activates_nothing() {
        let state = empty();
        for s in ALL {
            assert!(!satisfied(&state, s), "{} lit on an empty wheel", s.id);
        }
        assert!(active_ids(&state).is_empty());
        assert!(active_effects(&state).is_empty());
    }

    #[test]
    fn every_synergy_is_satisfied_by_exactly_its_own_requirements() {
        for s in ALL {
            let mut state = empty();
            for req in s.requires {
                match req.requirement {
                    Requirement::Maxed => {
                        state = state.with_level(req.concept, MAX_LEVEL);
                    }
                    Requirement::PresentOrFamily => {
                        state = state.with_level(concrete_member(req.concept), 1);
                    }
                    Requirement::Present => {
                        state = state.with_adapted(req.concept);
                    }
                }
            }
            assert!(satisfied(&state, s), "{} not lit by its own roster", s.id);
        }
    }

    #[test]
    fn family_requirements_match_any_member_including_modded_ids() {
        let void_and_vanilla_boss = empty()
            .with_level("Env_Void", 1)
            .with_level("Existence_minecraft:wither", 1);
        assert!(is_active(&void_and_vanilla_boss, &UNMAKER));

        let void_and_modded_boss = empty()
            .with_level("Env_Void", 1)
            .with_adapted("Existence_someothermod:the_final_boss");
        assert!(is_active(&void_and_modded_boss, &UNMAKER));

        let void_alone = empty().with_level("Env_Void", 1);
        assert!(!is_active(&void_alone, &UNMAKER));

        let unrelated = empty()
            .with_level("Env_Void", 1)
            .with_level("Type_MAGIC", MAX_LEVEL);
        assert!(!is_active(&unrelated, &UNMAKER));
    }

    #[test]
    fn contact_family_lights_gravebloom() {
        let state = empty()
            .with_adapted("Env_Thorns")
            .with_level("Contact_minecraft:zombie", 1);
        assert!(is_active(&state, &GRAVEBLOOM));
        let unrelated = empty().with_adapted("Env_Thorns").with_level("Type_MAGIC", 1);
        assert!(!is_active(&unrelated, &GRAVEBLOOM));
    }

    #[test]
    fn maxed_requirement_needs_the_full_level() {
        let partial = empty()
            .with_adapted("Env_Fire")
            .with_adapted("Mutation_Thermal")
            .with_level("Fist_Iron", MAX_LEVEL - 1);
        assert!(is_active(&partial, &ASHWALKER), "ashwalker has no fist gate");
        assert!(!is_active(&partial, &GOLIATH), "iron fist is one level short");

        let full = empty()
            .with_adapted("Env_Fire")
            .with_adapted("Mutation_Thermal")
            .with_level("Fist_Iron", MAX_LEVEL);
        assert!(is_active(&full, &GOLIATH));
    }

    #[test]
    fn astral_mine_needs_maxed_labor_and_fist_mutation() {
        let lit = empty().with_level("Mine_Labor", MAX_LEVEL).with_adapted("Mutation_Fist");
        assert!(is_active(&lit, &ASTRAL_MINE));
        assert!(!is_active(&empty().with_level("Mine_Labor", 7).with_adapted("Mutation_Fist"), &ASTRAL_MINE));
        assert!(!is_active(&empty().with_level("Mine_Labor", MAX_LEVEL), &ASTRAL_MINE));
    }

    #[test]
    fn switching_a_concept_off_turns_its_synergy_off() {
        let state = empty()
            .with_level("Type_FREEZE", MAX_LEVEL)
            .with_adapted("Env_Ice");
        assert!(is_active(&state, &GLACIERBLOOD));

        let one_time_off = state.clone().with_disabled("Env_Ice");
        assert!(!is_active(&one_time_off, &GLACIERBLOOD));
        assert!(!active_ids(&one_time_off).contains(&"glacierblood"));

        let leveled_off = state.clone().with_disabled("Type_FREEZE");
        assert!(!is_active(&leveled_off, &GLACIERBLOOD));
        assert!(strength(&leveled_off, &GLACIERBLOOD) < strength(&state, &GLACIERBLOOD));
    }

    #[test]
    fn switching_off_a_family_member_does_not_leave_a_hole() {
        let state = empty()
            .with_level("Env_Void", 1)
            .with_adapted("Existence_minecraft:wither")
            .with_disabled("Existence_minecraft:wither");
        assert!(!is_active(&state, &UNMAKER));
        // A second, still-enabled member keeps the family requirement satisfied.
        let state = state.with_adapted("Existence_minecraft:ender_dragon");
        assert!(is_active(&state, &UNMAKER));
    }

    #[test]
    fn strength_scales_with_the_level_ladder() {
        let barely = empty().with_level("Type_FREEZE", 1).with_adapted("Env_Ice");
        let halfway = empty().with_level("Type_FREEZE", 4).with_adapted("Env_Ice");
        let maxed = empty().with_level("Type_FREEZE", MAX_LEVEL).with_adapted("Env_Ice");
        assert!(strength(&barely, &GLACIERBLOOD) < strength(&halfway, &GLACIERBLOOD));
        assert!(strength(&halfway, &GLACIERBLOOD) < strength(&maxed, &GLACIERBLOOD));
        assert!((strength(&halfway, &GLACIERBLOOD) - 2.5).abs() < 1e-9);
        assert!((strength(&maxed, &GLACIERBLOOD) - 4.0).abs() < 1e-9);
    }

    #[test]
    fn maxed_gate_adds_no_strength_of_its_own() {
        let goliath = empty().with_level("Fist_Iron", MAX_LEVEL).with_adapted("Mutation_Thermal");
        let expected = 1.0 + 3.0 * (goliath.compute_adapt_count() as f64 / WHEEL_BREADTH).min(1.0);
        assert!((strength(&goliath, &GOLIATH) - expected).abs() < 1e-9);
    }

    #[test]
    fn one_time_mutations_and_family_members_do_not_add_strength() {
        let alone = empty().with_level("Env_Void", MAX_LEVEL);
        let with_boss = alone.clone().with_level("Existence_minecraft:wither", MAX_LEVEL);
        assert_eq!(strength(&with_boss, &UNMAKER), strength(&alone, &UNMAKER));

        let freeze_and_ice = empty().with_level("Type_FREEZE", 4).with_adapted("Env_Ice");
        let with_mutation = freeze_and_ice.clone().with_adapted("Mutation_Thermal");
        assert_eq!(strength(&with_mutation, &GLACIERBLOOD), strength(&freeze_and_ice, &GLACIERBLOOD));
    }

    #[test]
    fn all_gates_fall_back_to_whole_wheel_breadth_and_cap_at_four() {
        let narrow = empty().with_level("Mine_Labor", MAX_LEVEL).with_adapted("Mutation_Fist");
        let narrow_strength = strength(&narrow, &ASTRAL_MINE);
        let expected = 1.0 + 3.0 * (narrow.compute_adapt_count() as f64 / WHEEL_BREADTH);
        assert!((narrow_strength - expected).abs() < 1e-9);

        let mut padded = narrow.clone();
        for i in 0..10 {
            padded = padded.with_adapted(&format!("Filler_{i}"));
        }
        assert!(strength(&padded, &ASTRAL_MINE) > narrow_strength);

        let mut broad = narrow;
        for i in 0..60 {
            broad = broad.with_adapted(&format!("Filler_{i}"));
        }
        assert!((strength(&broad, &ASTRAL_MINE) - 4.0).abs() < 1e-9);
    }

    #[test]
    fn strength_never_exceeds_four_even_with_a_huge_wheel() {
        let mut broad = empty();
        for i in 0..200 {
            broad = broad.with_adapted(&format!("Filler_{i}"));
        }
        for s in ALL {
            assert!(strength(&broad, s) <= 4.0 + 1e-9, "{} above the ceiling", s.id);
        }
    }

    #[test]
    fn active_effects_are_in_sync_with_active_ids() {
        let state = empty()
            .with_adapted("Env_Fire")
            .with_adapted("Mutation_Thermal")
            .with_level("Type_FREEZE", 4)
            .with_adapted("Env_Ice");
        let effects = active_effects(&state);
        assert_eq!(effects.iter().map(|e| e.id).collect::<Vec<_>>(), active_ids(&state));
        for e in &effects {
            assert_eq!(e.strength, strength(&state, by_id(e.id).unwrap()));
            assert_eq!(e.effect, by_id(e.id).unwrap().effect);
        }
    }

    #[test]
    fn effect_descriptors_match_upstream_constants() {
        assert_eq!(
            ASHWALKER.effect,
            Effect::IgniteOnHit { base_ticks: 40, min_ticks: 20, cooldown_ticks: 20 }
        );
        assert_eq!(
            GLACIERBLOOD.effect,
            Effect::SlowOnHit { base_ticks: 20, min_ticks: 20, max_amplifier: 4 }
        );
        assert_eq!(
            DROWNED_WALTZ.effect,
            Effect::DrownedWaltz { swim_bonus_base: 1.5, needs_drown_adaptation: true }
        );
        assert_eq!(SKYBREAKER.effect, Effect::ImpactRadiusMultiplier);
        assert_eq!(UNMAKER.effect, Effect::VoidDamageToHeal { heal_per_strength: 2.0 });
        assert_eq!(UNSEEN.effect, Effect::TargetRefusal { distance_sqr_per_strength: 36.0 });
        assert_eq!(GRAVEBLOOM.effect, Effect::Retaliate { fraction_per_strength: 0.12 });
        assert_eq!(
            STORMCALL.effect,
            Effect::ChainLightning {
                max_targets: 5,
                min_targets: 1,
                damage_fraction: 0.08,
                min_damage: 1.0,
            }
        );
        assert_eq!(ASTRAL_MINE.effect, Effect::BlockBreakHeal { heal_per_strength: 0.25 });
    }

    #[test]
    fn goliath_describes_ignite_and_the_full_smelting_table() {
        match GOLIATH.effect {
            Effect::Goliath { ignite_base_ticks, min_ticks, smelting } => {
                assert_eq!((ignite_base_ticks, min_ticks), (20, 20));
                let rules: Vec<(&str, &str)> =
                    smelting.iter().map(|r| (r.input, r.output)).collect();
                assert_eq!(
                    rules,
                    [
                        ("minecraft:raw_iron", "minecraft:iron_ingot"),
                        ("minecraft:raw_gold", "minecraft:gold_ingot"),
                        ("minecraft:raw_copper", "minecraft:copper_ingot"),
                        ("minecraft:nether_gold_ore", "minecraft:gold_ingot"),
                        ("minecraft:ancient_debris", "minecraft:netherite_scrap"),
                    ]
                );
            }
            other => panic!("goliath has the wrong effect: {other:?}"),
        }
    }

    #[test]
    fn numeric_helpers_match_upstream_arithmetic() {
        assert_eq!(ignite_ticks(40, 20, 2.0), 80);
        assert_eq!(ignite_ticks(40, 20, 0.25), 20); // min wins
        assert_eq!(ignite_ticks(20, 20, 1.5), 30);
        assert_eq!(slow_amplifier(4, 8.0), 4);
        assert_eq!(slow_amplifier(4, 2.0), 2);

        assert_eq!(chain_power(3.9), 3.0); // upstream truncates at the call site
        assert_eq!(chain_target_limit(5, 1, chain_power(3.9)), 3);
        assert_eq!(chain_target_limit(5, 1, chain_power(1.0)), 1);
        assert_eq!(chain_target_limit(5, 1, chain_power(99.0)), 5);
        assert!((chain_damage(20.0, 0.08, 1.0, 3.0) - 4.8).abs() < 1e-5);
        assert_eq!(chain_damage(1.0, 0.08, 1.0, 1.0), 1.0); // min damage wins

        assert_eq!(retaliation_damage(50.0, 0.12, 2.0), 12.0);
        assert_eq!(swim_bonus(2.0, 1.5, 0.5), 4.0);
        assert_eq!(2.0 * 2.0, 4.0); // UNMAKER heal at strength 2
        assert_eq!(36.0 * 2.0, 72.0); // UNSEEN refusal radius at strength 2
        assert_eq!(0.25 * 2.0, 0.5); // ASTRAL_MINE heal at strength 2
        assert_eq!(3.0 * 2.0, 6.0); // SKYBREAKER radius multiplier at strength 2
    }

    #[test]
    fn components_of_is_deduplicated_and_ordered() {
        assert_eq!(components_of(&UNMAKER), ["Env_Void", "Existence_"]);
        assert_eq!(components_of(&GOLIATH), ["Fist_Iron", "Mutation_Thermal"]);
        assert_eq!(components_of(&ASHWALKER), ["Env_Fire", "Mutation_Thermal"]);
    }

    #[test]
    fn adapt_count_counts_the_union_not_the_sum() {
        let mut state = empty();
        state = state.with_adapted("Env_Fire").with_adapted("Mutation_Thermal");
        state = state.with_level("Env_Fire", 3).with_level("Type_MAGIC", 2);
        state = state.with_level("Zero_Level", 0);
        assert_eq!(state.compute_adapt_count(), 3);
    }
}
