//! Generalised concept model for the native port.
//!
//! This module is a pure-data mirror of the upstream Java
//! `AdaptationDefinition` / `AdaptationDomain` / `AdaptationRegistry` /
//! `Concepts` contract. It deliberately has **no** dependency on
//! `mod_api` or the host: it is compiled and tested as plain Rust, and the
//! rest of the native crate can layer engine behaviour on top of it.
//!
//! Every id here is taken verbatim from upstream (`Concepts`,
//! `AdaptationRegistry`, `FistTiers`, `CombatFistTiers`). In particular the
//! upstream spelling quirks are preserved: the adversity concept is
//! `"ADBERSITY"` (not `"ADVERSITY"`) and the fall environment concept is
//! `"Env_FallDamage"` (not `"Env_Fall"`).
//!
//! # Retired damage concepts
//!
//! Upstream keeps `Type_FALL`, `Type_DROWN`, `Type_STARVE`, `Type_SUFFOCATE`,
//! `Type_VOID` and `Type_CONTACT` in `Concepts.RETIRED_TYPES`. They were
//! replaced by one-time `Env_*` immunities. This table retains them (as
//! leveled, max-8 entries) so old wheels/attachments can still be looked up
//! and enumerated, but the `is_level_based` / `is_one_time` predicates gate
//! them **before** consulting the table — exactly like upstream
//! `Concepts.isLevelBased`, which checks `isRetired` first.

/// Upstream `PlayerAdaption.MAX_LEVEL` / `AdaptationDefinition.DEFAULT_MAX_LEVEL`.
pub const MAX_LEVEL: u8 = 8;

/// Upstream `AdaptationDomain` — all 11 domains, in the same declaration
/// order (which upstream `allDefinitions()` sorts by).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Domain {
    Damage,
    Effect,
    Movement,
    Physics,
    Mining,
    Environment,
    Combat,
    Perception,
    Entity,
    Existence,
    Special,
}

impl Domain {
    /// The lowercase key used by upstream for translations and commands
    /// (`adaptionwheel.domain.<key>`).
    pub const fn key(self) -> &'static str {
        match self {
            Domain::Damage => "damage",
            Domain::Effect => "effect",
            Domain::Movement => "movement",
            Domain::Physics => "physics",
            Domain::Mining => "mining",
            Domain::Environment => "environment",
            Domain::Combat => "combat",
            Domain::Perception => "perception",
            Domain::Entity => "entity",
            Domain::Existence => "existence",
            Domain::Special => "special",
        }
    }
}

/// Upstream `AdaptationDefinition` record, plus a `display_id` translation
/// key for the UI (upstream derives this via `Concepts.displayName`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdaptationDefinition {
    pub concept: &'static str,
    pub domain: Domain,
    pub leveled: bool,
    pub max_level: u8,
    /// Translation key of the form `adaptionwheel.concept.<family>.<name>`.
    pub display_id: &'static str,
}

impl AdaptationDefinition {
    /// Constant constructor for a level ladder (`max_level == 8`).
    pub const fn new_leveled(
        concept: &'static str,
        domain: Domain,
        display_id: &'static str,
    ) -> Self {
        Self {
            concept,
            domain,
            leveled: true,
            max_level: MAX_LEVEL,
            display_id,
        }
    }

    /// Constant constructor for a one-time adaptation (`max_level == 1`).
    pub const fn new_one_time(
        concept: &'static str,
        domain: Domain,
        display_id: &'static str,
    ) -> Self {
        Self {
            concept,
            domain,
            leveled: false,
            max_level: 1,
            display_id,
        }
    }

    /// Complement of [`Self::leveled`]: a definition is one-time when its
    /// level cap is at most 1 or it is not flagged as a ladder at all.
    pub fn one_time(&self) -> bool {
        self.max_level <= 1 || !self.leveled
    }

    /// True for a genuine level ladder (leveled flag and a cap above one).
    pub fn leveled(&self) -> bool {
        self.leveled && self.max_level > 1
    }
}

/// Concepts that used to be leveled but were replaced by one-time `Env_*`
/// immunity. Verbatim from `Concepts.RETIRED_TYPES`.
const RETIRED_TYPES: &[&str] = &[
    "Type_FALL",
    "Type_DROWN",
    "Type_STARVE",
    "Type_SUFFOCATE",
    "Type_VOID",
    "Type_CONTACT",
];

/// The five `Env_*` ids upstream classifies as `PHYSICS` rather than
/// `ENVIRONMENT` (`AdaptationRegistry.PHYSICS_ENV_IDS`).
const PHYSICS_ENV_IDS: &[&str] = &[
    "Ice",
    "Slime",
    "Cobweb",
    "Knockback",
    "FallDamage",
];

// Display-key families, mirroring `Concepts.displayName`:
//   Type_*      -> adaptionwheel.concept.type.<lower-name>
//   Env_*       -> adaptionwheel.concept.env.<concept>
//   Move_*      -> adaptionwheel.concept.move.<concept>
//   Mine_*      -> adaptionwheel.concept.mine.<concept>
//   Fist_*      -> adaptionwheel.concept.fist.<concept>
//   Combat_*    -> adaptionwheel.concept.combat.<concept>
//   Percep_*    -> adaptionwheel.concept.percep.<concept>
//   Mutation_*  -> adaptionwheel.concept.mutation.<suffix>
//   others      -> adaptionwheel.concept.<special key>
static BUILTINS: &[AdaptationDefinition] = &[
    // ---- DAMAGE: 15 leveled categories (all AdaptionCategory except FIRE) ----
    // FIRE has no Type_* ladder: Concepts.type(FIRE) == Env_Fire, one-time,
    // registered with the environment family below.
    AdaptationDefinition::new_leveled("Type_FALL", Domain::Damage, "adaptionwheel.concept.type.fall"),
    AdaptationDefinition::new_leveled("Type_WITHER", Domain::Damage, "adaptionwheel.concept.type.wither"),
    AdaptationDefinition::new_leveled("Type_DROWN", Domain::Damage, "adaptionwheel.concept.type.drown"),
    AdaptationDefinition::new_leveled("Type_FREEZE", Domain::Damage, "adaptionwheel.concept.type.freeze"),
    AdaptationDefinition::new_leveled("Type_LIGHTNING", Domain::Damage, "adaptionwheel.concept.type.lightning"),
    AdaptationDefinition::new_leveled("Type_EXPLOSION", Domain::Damage, "adaptionwheel.concept.type.explosion"),
    AdaptationDefinition::new_leveled("Type_PROJECTILE", Domain::Damage, "adaptionwheel.concept.type.projectile"),
    AdaptationDefinition::new_leveled("Type_MAGIC", Domain::Damage, "adaptionwheel.concept.type.magic"),
    AdaptationDefinition::new_leveled("Type_MOB", Domain::Damage, "adaptionwheel.concept.type.mob"),
    AdaptationDefinition::new_leveled("Type_PLAYER", Domain::Damage, "adaptionwheel.concept.type.player"),
    AdaptationDefinition::new_leveled("Type_CONTACT", Domain::Damage, "adaptionwheel.concept.type.contact"),
    AdaptationDefinition::new_leveled("Type_STARVE", Domain::Damage, "adaptionwheel.concept.type.starve"),
    AdaptationDefinition::new_leveled("Type_SUFFOCATE", Domain::Damage, "adaptionwheel.concept.type.suffocate"),
    AdaptationDefinition::new_leveled("Type_VOID", Domain::Damage, "adaptionwheel.concept.type.void"),
    AdaptationDefinition::new_leveled("Type_OTHER", Domain::Damage, "adaptionwheel.concept.type.other"),

    // ---- ENVIRONMENT: 10 one-time immunities ----
    AdaptationDefinition::new_one_time("Env_Fire", Domain::Environment, "adaptionwheel.concept.env.Env_Fire"),
    AdaptationDefinition::new_one_time("Env_Lava", Domain::Environment, "adaptionwheel.concept.env.Env_Lava"),
    AdaptationDefinition::new_one_time("Env_Drowning", Domain::Environment, "adaptionwheel.concept.env.Env_Drowning"),
    AdaptationDefinition::new_one_time("Env_Thorns", Domain::Environment, "adaptionwheel.concept.env.Env_Thorns"),
    AdaptationDefinition::new_one_time("Env_Darkness", Domain::Environment, "adaptionwheel.concept.env.Env_Darkness"),
    AdaptationDefinition::new_one_time("Env_Suffocate", Domain::Environment, "adaptionwheel.concept.env.Env_Suffocate"),
    AdaptationDefinition::new_one_time("Env_Void", Domain::Environment, "adaptionwheel.concept.env.Env_Void"),
    AdaptationDefinition::new_one_time("Env_Starve", Domain::Environment, "adaptionwheel.concept.env.Env_Starve"),
    AdaptationDefinition::new_one_time("Env_Liquid", Domain::Environment, "adaptionwheel.concept.env.Env_Liquid"),
    AdaptationDefinition::new_one_time("Env_Inventory", Domain::Environment, "adaptionwheel.concept.env.Env_Inventory"),

    // ---- MOVEMENT: 5 one-time surface adaptations ----
    AdaptationDefinition::new_one_time("Move_SoulSand", Domain::Movement, "adaptionwheel.concept.move.Move_SoulSand"),
    AdaptationDefinition::new_one_time("Move_Honey", Domain::Movement, "adaptionwheel.concept.move.Move_Honey"),
    AdaptationDefinition::new_one_time("Move_PowderSnow", Domain::Movement, "adaptionwheel.concept.move.Move_PowderSnow"),
    AdaptationDefinition::new_one_time("Move_BerryBush", Domain::Movement, "adaptionwheel.concept.move.Move_BerryBush"),
    AdaptationDefinition::new_one_time("Move_BubbleColumn", Domain::Movement, "adaptionwheel.concept.move.Move_BubbleColumn"),

    // ---- PHYSICS: 5 one-time `Env_*` ids ----
    AdaptationDefinition::new_one_time("Env_Ice", Domain::Physics, "adaptionwheel.concept.env.Env_Ice"),
    AdaptationDefinition::new_one_time("Env_Slime", Domain::Physics, "adaptionwheel.concept.env.Env_Slime"),
    AdaptationDefinition::new_one_time("Env_Cobweb", Domain::Physics, "adaptionwheel.concept.env.Env_Cobweb"),
    AdaptationDefinition::new_one_time("Env_Knockback", Domain::Physics, "adaptionwheel.concept.env.Env_Knockback"),
    AdaptationDefinition::new_one_time("Env_FallDamage", Domain::Physics, "adaptionwheel.concept.env.Env_FallDamage"),

    // ---- MINING: one leveled concept + the 5-level breaking-fist ladder ----
    AdaptationDefinition::new_leveled("Mine_Labor", Domain::Mining, "adaptionwheel.concept.mine.Mine_Labor"),
    AdaptationDefinition::new_leveled("Fist_Wood", Domain::Mining, "adaptionwheel.concept.fist.Fist_Wood"),
    AdaptationDefinition::new_leveled("Fist_Stone", Domain::Mining, "adaptionwheel.concept.fist.Fist_Stone"),
    AdaptationDefinition::new_leveled("Fist_Iron", Domain::Mining, "adaptionwheel.concept.fist.Fist_Iron"),
    AdaptationDefinition::new_leveled("Fist_Diamond", Domain::Mining, "adaptionwheel.concept.fist.Fist_Diamond"),
    AdaptationDefinition::new_leveled("Fist_Netherite", Domain::Mining, "adaptionwheel.concept.fist.Fist_Netherite"),

    // ---- COMBAT: leveled cooldown + two one-time flags + combat-fist ladder ----
    AdaptationDefinition::new_leveled("Combat_Cooldown", Domain::Combat, "adaptionwheel.concept.combat.Combat_Cooldown"),
    AdaptationDefinition::new_one_time("Combat_ShieldLock", Domain::Combat, "adaptionwheel.concept.combat.Combat_ShieldLock"),
    AdaptationDefinition::new_one_time("Combat_SkillIssue", Domain::Combat, "adaptionwheel.concept.combat.Combat_SkillIssue"),
    AdaptationDefinition::new_leveled("Combat_FistWood", Domain::Combat, "adaptionwheel.concept.combat.Combat_FistWood"),
    AdaptationDefinition::new_leveled("Combat_FistStone", Domain::Combat, "adaptionwheel.concept.combat.Combat_FistStone"),
    AdaptationDefinition::new_leveled("Combat_FistIron", Domain::Combat, "adaptionwheel.concept.combat.Combat_FistIron"),
    AdaptationDefinition::new_leveled("Combat_FistDiamond", Domain::Combat, "adaptionwheel.concept.combat.Combat_FistDiamond"),
    AdaptationDefinition::new_leveled("Combat_FistNetherite", Domain::Combat, "adaptionwheel.concept.combat.Combat_FistNetherite"),

    // ---- PERCEPTION ----
    AdaptationDefinition::new_one_time("Percep_SteadyGaze", Domain::Perception, "adaptionwheel.concept.percep.Percep_SteadyGaze"),

    // ---- SPECIAL ----
    AdaptationDefinition::new_leveled("Self_Damage", Domain::Special, "adaptionwheel.concept.self_damage"),
    AdaptationDefinition::new_one_time("ADBERSITY", Domain::Special, "adaptionwheel.concept.adversity"),
    AdaptationDefinition::new_one_time("Mutation_Thermal", Domain::Special, "adaptionwheel.concept.mutation.Thermal"),
    AdaptationDefinition::new_one_time("Mutation_Aquatic", Domain::Special, "adaptionwheel.concept.mutation.Aquatic"),
    AdaptationDefinition::new_one_time("Mutation_Impact", Domain::Special, "adaptionwheel.concept.mutation.Impact"),
    AdaptationDefinition::new_one_time("Mutation_SeaEye", Domain::Special, "adaptionwheel.concept.mutation.SeaEye"),
    AdaptationDefinition::new_one_time("Mutation_Flight", Domain::Special, "adaptionwheel.concept.mutation.Flight"),
    AdaptationDefinition::new_one_time("Mutation_Fist", Domain::Special, "adaptionwheel.concept.mutation.Fist"),
    AdaptationDefinition::new_one_time("Dimension_Destroy", Domain::Special, "adaptionwheel.concept.special.Dimension_Destroy"),
];

/// All statically registered concepts.
pub fn builtins() -> &'static [AdaptationDefinition] {
    BUILTINS
}

/// Look up a concept by its exact upstream id.
pub fn get(concept: &str) -> Option<&'static AdaptationDefinition> {
    BUILTINS.iter().find(|def| def.concept == concept)
}

/// True for an id in `Concepts.RETIRED_TYPES`.
pub fn is_retired(concept: &str) -> bool {
    RETIRED_TYPES.contains(&concept)
}

/// Classify a concept into its domain. Registry lookup first, then the
/// prefix fallback from `AdaptationRegistry.domainOf`.
pub fn domain_of(concept: &str) -> Domain {
    if let Some(def) = get(concept) {
        return def.domain;
    }
    if concept.starts_with("Type_") {
        Domain::Damage
    } else if concept.starts_with("Debuff_") {
        Domain::Effect
    } else if concept.starts_with("Move_") {
        Domain::Movement
    } else if concept.starts_with("Mine_") || concept.starts_with("Fist_") {
        Domain::Mining
    } else if concept.starts_with("Combat_") {
        Domain::Combat
    } else if concept.starts_with("Percep_") {
        Domain::Perception
    } else if concept.starts_with("Contact_")
        || concept.starts_with("Offense_NPC_")
        || concept.starts_with("Drop_NPC_")
    {
        Domain::Entity
    } else if concept.starts_with("Existence_") {
        Domain::Existence
    } else if let Some(suffix) = concept.strip_prefix("Env_") {
        if PHYSICS_ENV_IDS.contains(&suffix) {
            Domain::Physics
        } else {
            Domain::Environment
        }
    } else {
        // Mutation_/Dimension_/Self_Damage/ADBERSITY and anything unknown.
        Domain::Special
    }
}

/// Whether a concept is a level ladder. Mirrors `Concepts.isLevelBased`:
/// the retired gate comes first, then the registry, then the prefix
/// fallback (`Type_`, `Contact_`, `Offense_NPC_`, `Drop_NPC_` and
/// `Self_Damage`).
pub fn is_level_based(concept: &str) -> bool {
    if is_retired(concept) {
        return false;
    }
    if let Some(def) = get(concept) {
        return def.leveled();
    }
    concept.starts_with("Type_")
        || concept.starts_with("Contact_")
        || concept.starts_with("Offense_NPC_")
        || concept.starts_with("Drop_NPC_")
        || concept == "Self_Damage"
}

/// Whether a concept is acquired in one shot. Retired ids are treated as
/// one-time here because they were replaced by one-time `Env_*` immunity
/// (upstream `Concepts.isOneTime` returns false for them; this is a
/// deliberate, documented simplification for the generalised model).
/// Otherwise: registry first, then the prefix fallback from upstream.
pub fn is_one_time(concept: &str) -> bool {
    if is_retired(concept) {
        return true;
    }
    if let Some(def) = get(concept) {
        return def.one_time();
    }
    concept.starts_with("Env_")
        || concept.starts_with("Debuff_")
        || concept.starts_with("Existence_")
        || concept.starts_with("Mutation_")
        || concept.starts_with("Move_")
        || concept.starts_with("Percep_")
        || concept == "ADBERSITY"
}

/// Every registered id, for command suggestions.
pub fn all_ids() -> impl Iterator<Item = &'static str> {
    BUILTINS.iter().map(|def| def.concept)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_comprehensive() {
        assert!(
            builtins().len() >= 40,
            "expected >= 40 definitions, got {}",
            builtins().len()
        );
        // No duplicate ids.
        let mut ids: Vec<&str> = all_ids().collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "duplicate concept ids in builtins()");
    }

    #[test]
    fn key_ids_resolve_with_expected_metadata() {
        let fall = get("Env_FallDamage").expect("Env_FallDamage");
        assert_eq!(fall.domain, Domain::Physics);
        assert!(fall.one_time());
        assert_eq!(fall.max_level, 1);

        let fire = get("Env_Fire").expect("Env_Fire");
        assert_eq!(fire.domain, Domain::Environment);
        assert!(fire.one_time());
        assert_eq!(fire.display_id, "adaptionwheel.concept.env.Env_Fire");

        let mob = get("Type_MOB").expect("Type_MOB");
        assert_eq!(mob.domain, Domain::Damage);
        assert!(mob.leveled());
        assert_eq!(mob.max_level, MAX_LEVEL);

        let honey = get("Move_Honey").expect("Move_Honey");
        assert_eq!(honey.domain, Domain::Movement);
        assert!(honey.one_time());

        let self_dmg = get("Self_Damage").expect("Self_Damage");
        assert_eq!(self_dmg.domain, Domain::Special);
        assert!(self_dmg.leveled());

        // Upstream misspelling is part of the id contract.
        let adversity = get("ADBERSITY").expect("ADBERSITY");
        assert_eq!(adversity.domain, Domain::Special);
        assert!(adversity.one_time());
        assert!(get("ADVERSITY").is_none());
    }

    #[test]
    fn retired_types_are_not_level_based() {
        for id in RETIRED_TYPES {
            assert!(is_retired(id), "{id} must be flagged retired");
            assert!(!is_level_based(id), "{id} must not be level-based");
            assert!(is_one_time(id), "{id} became a one-time Env_* immunity");
        }
        // Present in the table for lookup, but excluded from level semantics.
        assert!(get("Type_FALL").is_some());
    }

    #[test]
    fn domain_of_registry_and_prefix_fallback() {
        // Registry hits.
        assert_eq!(domain_of("Env_FallDamage"), Domain::Physics);
        assert_eq!(domain_of("Env_Fire"), Domain::Environment);
        assert_eq!(domain_of("Type_MOB"), Domain::Damage);
        assert_eq!(domain_of("ADBERSITY"), Domain::Special);

        // Prefix fallbacks for unknown ids.
        assert_eq!(domain_of("Type_FUTURE"), Domain::Damage);
        assert_eq!(domain_of("Debuff_minecraft:poison"), Domain::Effect);
        assert_eq!(domain_of("Move_Future"), Domain::Movement);
        assert_eq!(domain_of("Mine_Future"), Domain::Mining);
        assert_eq!(domain_of("Fist_Future"), Domain::Mining);
        assert_eq!(domain_of("Combat_Future"), Domain::Combat);
        assert_eq!(domain_of("Percep_Future"), Domain::Perception);
        assert_eq!(domain_of("Contact_minecraft:pig"), Domain::Entity);
        assert_eq!(domain_of("Offense_NPC_minecraft:pig"), Domain::Entity);
        assert_eq!(domain_of("Drop_NPC_minecraft:pig"), Domain::Entity);
        assert_eq!(domain_of("Existence_minecraft:ender_dragon"), Domain::Existence);
        assert_eq!(domain_of("Env_Future"), Domain::Environment);
        assert_eq!(domain_of("Env_Ice"), Domain::Physics);
        assert_eq!(domain_of("Env_Slime"), Domain::Physics);
        assert_eq!(domain_of("Env_Cobweb"), Domain::Physics);
        assert_eq!(domain_of("Env_Knockback"), Domain::Physics);
        assert_eq!(domain_of("Mutation_Future"), Domain::Special);
        assert_eq!(domain_of("Dimension_Future"), Domain::Special);
        assert_eq!(domain_of("Self_Damage"), Domain::Special);
        assert_eq!(domain_of("something_unknown"), Domain::Special);
    }

    #[test]
    fn level_semantics_follow_max_level() {
        for def in builtins() {
            if def.leveled() {
                assert_eq!(def.max_level, MAX_LEVEL, "{} leveled cap", def.concept);
                assert!(!def.one_time(), "{} is leveled", def.concept);
                // Retired ladders are retained in the table for lookup but are
                // excluded from level semantics by the gate in is_level_based.
                if !is_retired(def.concept) {
                    assert!(is_level_based(def.concept), "{} predicate", def.concept);
                }
            } else {
                assert_eq!(def.max_level, 1, "{} one-time cap", def.concept);
                assert!(def.one_time(), "{} is one-time", def.concept);
                assert!(!is_level_based(def.concept), "{} predicate", def.concept);
            }
        }
        // Prefix fallback for unregistered ids.
        assert!(is_level_based("Contact_minecraft:pig"));
        assert!(is_level_based("Drop_NPC_minecraft:pig"));
        assert!(is_one_time("Env_Future"));
        assert!(is_one_time("Debuff_minecraft:poison"));
        assert!(is_one_time("Mutation_Future"));
    }
}
