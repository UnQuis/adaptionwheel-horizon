//! Pure damage-category matching and mitigation rules for the AdaptionWheel native port.
//!
//! This module is a **pure layer**: it depends on neither `mod_api`, the Horizon `Host`,
//! nor any player state type. Callers supply string damage-type ids (and, optionally,
//! vanilla-style damage-type tags) and closures for "is this one-time concept active?"
//! and "what level is this concept?". That keeps the upstream `AdaptionCategory.match`
//! and `AdaptionEvents` immunity/capstone logic directly unit-testable without a game
//! runtime.
//!
//! # Upstream contract mirrored here
//!
//! - `AdaptionCategory.match` (`category/AdaptionCategory.java:61-94`) checks vanilla
//!   damage-type tags first, then falls back to exact registered-name path checks.
//! - `Concepts.type` (`category/Concepts.java:110-115`) returns `Type_<CATEGORY>`
//!   except for `FIRE`, which is the one-time `Env_Fire`; `RETIRED_TYPES` removes
//!   the level ladders for FALL/DROWN/STARVE/SUFFOCATE/VOID/CONTACT.
//! - `AdaptionEvents.envImmunityConcept` (`:183-199`) maps the one-time `Env_*` immunity
//!   concepts; `hasFullImmunity` (`:218-224`) gives FIRE a source-specific `Env_Lava`
//!   override; `isMaxedImmunity` (`:255-273`) handles leveled `Type_*` ladders;
//!   `contactImmunityApplies` (`:287-303`) handles `Contact_<mob>` ladders;
//!   `TOTAL_IMMUNITY_CATEGORIES` (`:816-817`) is **only** `PROJECTILE` and `OTHER`.
//!
//! # Mapping table (data-driven, in priority order)
//!
//! Exact path tokens are checked before substring tokens. Ids are ResourceLocation
//! paths; any namespace prefix (`minecraft:`, `somemod:`) is stripped first.
//!
//! | Category | Exact id tokens | Substring tokens | Vanilla tag |
//! |---|---|---|---|
//! | Fall | `fall` | `fall`, `falling` | `is_fall` |
//! | Wither | `wither`, `wither_skull` | `wither` | `is_wither` |
//! | Fire | `fire`, `hot_floor`, `in_fire`, `on_fire`, `campfire`, `fireball`, `lava` | `lava`, `fireball`, `in_fire`, `on_fire`, `campfire`, `fire` | `is_fire` |
//! | Drown | `drown`, `drowning` | `drown` | `is_drowning` |
//! | Freeze | `freeze`, `freezing`, `powder_snow` | `powder_snow`, `freez`, `frost` | `is_freezing` |
//! | Lightning | `lightning_bolt`, `lightning` | `lightning` | — |
//! | Explosion | `explosion`, `blast`, `fireworks`, `firework` | `fireworks`, `firework`, `explosion`, `blast` | `is_explosion` |
//! | Projectile | `arrow`, `projectile`, `trident` | `projectile`, `arrow`, `trident` | `is_projectile` |
//! | Magic | `magic`, `indirect_magic`, `sonic_boom` | `magic`, `sonic_boom` | — |
//! | Mob | `mob_attack`, `mob_attack_no_aggro`, `sting`, `mob` | `mob_attack`, `mob`, `sting` | — |
//! | Player | `player_attack`, `mace_smash`, `player` | `player_attack`, `player` | — |
//! | Contact | `cacti`, `cactus`, `sweet_berry_bush`, `stalagmite`, `thorns` | `cactus`, `cacti`, `sweet_berry`, `berry_bush`, `stalagmite`, `thorns` | — |
//! | Starve | `starve`, `starvation` | `starve`, `starvation` | — |
//! | Suffocate | `suffocation`, `suffocate`, `in_wall`, `fly_into_wall` | `suffocat`, `in_wall` | — |
//! | Void | `out_of_world`, `void` | `out_of_world`, `void` | — |
//! | Other | — (fallback) | — (fallback) | — |
//!
//! Ordering matters inside the substring table: `fireworks`/`firework` precede `fire`
//! so `minecraft:fireworks` is an Explosion, while `fireball` precedes `fire` so a
//! fireball stays Fire. Likewise `lightning` precedes `bolt`, and `player_attack`
//! precedes `player`.
//!
//! # Deliberate divergences from upstream, documented
//!
//! 1. Upstream `match` uses vanilla damage-type **tag queries** on a `DamageSource`.
//!    This pure layer has no such query; tags are passed in by the caller through
//!    [`categorize_with_tags`]. A caller with tag access should forward them.
//! 2. This layer accepts broader **substring** matches (modded ids) in addition to
//!    upstream's exact-path matches; upstream would call those `OTHER`.
//! 3. Upstream matches `thorns` as `MAGIC` (its exact-id arm), but its immunity is
//!    `Env_Thorns`, which belongs to the `CONTACT` family. Per the port's public
//!    contract `thorns` is classified as `Contact` here.
//! 4. Upstream checks tags in wither-first order; [`categorize_with_tags`] follows the
//!    contract's listed order (fall, fire, drowning, freezing, explosion, projectile,
//!    wither). In practice a damage type has at most one of these tags.

/// The 16 damage categories mirrored from upstream `AdaptionCategory`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageCategory {
    Fall,
    Wither,
    Fire,
    Drown,
    Freeze,
    Lightning,
    Explosion,
    Projectile,
    Magic,
    Mob,
    Player,
    Contact,
    Starve,
    Suffocate,
    Void,
    Other,
}

impl DamageCategory {
    /// Every category, for enumeration in tests and registry/UI code.
    pub const ALL: [DamageCategory; 16] = [
        DamageCategory::Fall,
        DamageCategory::Wither,
        DamageCategory::Fire,
        DamageCategory::Drown,
        DamageCategory::Freeze,
        DamageCategory::Lightning,
        DamageCategory::Explosion,
        DamageCategory::Projectile,
        DamageCategory::Magic,
        DamageCategory::Mob,
        DamageCategory::Player,
        DamageCategory::Contact,
        DamageCategory::Starve,
        DamageCategory::Suffocate,
        DamageCategory::Void,
        DamageCategory::Other,
    ];
}

/// Strips an optional `namespace:` prefix, matching upstream's `id` after the colon.
fn damage_path(id: &str) -> &str {
    match id.rsplit_once(':') {
        Some((_, path)) => path,
        None => id,
    }
}

/// Accepts both bare (`is_fall`) and namespaced (`minecraft:is_fall`) tag spellings.
fn tag_path(tag: &str) -> &str {
    tag.rsplit(':').next().unwrap_or(tag)
}

/// Exact registered-name path → category. Checked before the substring table.
const EXACT_ID_MATCHES: &[(&str, DamageCategory)] = &[
    ("fall", DamageCategory::Fall),
    ("wither", DamageCategory::Wither),
    ("wither_skull", DamageCategory::Wither),
    ("fire", DamageCategory::Fire),
    ("hot_floor", DamageCategory::Fire),
    ("in_fire", DamageCategory::Fire),
    ("on_fire", DamageCategory::Fire),
    ("campfire", DamageCategory::Fire),
    ("fireball", DamageCategory::Fire),
    ("lava", DamageCategory::Fire),
    ("drown", DamageCategory::Drown),
    ("drowning", DamageCategory::Drown),
    ("freeze", DamageCategory::Freeze),
    ("freezing", DamageCategory::Freeze),
    ("powder_snow", DamageCategory::Freeze),
    ("lightning_bolt", DamageCategory::Lightning),
    ("lightning", DamageCategory::Lightning),
    ("explosion", DamageCategory::Explosion),
    ("blast", DamageCategory::Explosion),
    ("fireworks", DamageCategory::Explosion),
    ("firework", DamageCategory::Explosion),
    ("arrow", DamageCategory::Projectile),
    ("projectile", DamageCategory::Projectile),
    ("trident", DamageCategory::Projectile),
    ("magic", DamageCategory::Magic),
    ("indirect_magic", DamageCategory::Magic),
    ("sonic_boom", DamageCategory::Magic),
    ("mob", DamageCategory::Mob),
    ("mob_attack", DamageCategory::Mob),
    ("mob_attack_no_aggro", DamageCategory::Mob),
    ("sting", DamageCategory::Mob),
    ("player", DamageCategory::Player),
    ("player_attack", DamageCategory::Player),
    ("mace_smash", DamageCategory::Player),
    ("cactus", DamageCategory::Contact),
    ("cacti", DamageCategory::Contact),
    ("sweet_berry_bush", DamageCategory::Contact),
    ("stalagmite", DamageCategory::Contact),
    ("thorns", DamageCategory::Contact),
    ("starve", DamageCategory::Starve),
    ("starvation", DamageCategory::Starve),
    ("suffocation", DamageCategory::Suffocate),
    ("suffocate", DamageCategory::Suffocate),
    ("in_wall", DamageCategory::Suffocate),
    ("fly_into_wall", DamageCategory::Suffocate),
    ("out_of_world", DamageCategory::Void),
    ("void", DamageCategory::Void),
];

/// Broader substring → category for modded ids. ORDER IS SIGNIFICANT: more specific
/// tokens precede the generic `fire`/`player`/`mob` tokens they would otherwise shadow.
const SUBSTRING_ID_MATCHES: &[(&str, DamageCategory)] = &[
    ("fireworks", DamageCategory::Explosion),
    ("firework", DamageCategory::Explosion),
    ("explosion", DamageCategory::Explosion),
    ("blast", DamageCategory::Explosion),
    ("wither", DamageCategory::Wither),
    ("lightning", DamageCategory::Lightning),
    ("powder_snow", DamageCategory::Freeze),
    ("freez", DamageCategory::Freeze),
    ("frost", DamageCategory::Freeze),
    ("drown", DamageCategory::Drown),
    ("suffocat", DamageCategory::Suffocate),
    ("in_wall", DamageCategory::Suffocate),
    ("out_of_world", DamageCategory::Void),
    ("void", DamageCategory::Void),
    ("starvation", DamageCategory::Starve),
    ("starve", DamageCategory::Starve),
    ("sweet_berry", DamageCategory::Contact),
    ("berry_bush", DamageCategory::Contact),
    ("cactus", DamageCategory::Contact),
    ("cacti", DamageCategory::Contact),
    ("stalagmite", DamageCategory::Contact),
    ("thorns", DamageCategory::Contact),
    ("player_attack", DamageCategory::Player),
    ("player", DamageCategory::Player),
    ("mob_attack", DamageCategory::Mob),
    ("mob", DamageCategory::Mob),
    ("sting", DamageCategory::Mob),
    ("projectile", DamageCategory::Projectile),
    ("arrow", DamageCategory::Projectile),
    ("trident", DamageCategory::Projectile),
    ("magic", DamageCategory::Magic),
    ("sonic_boom", DamageCategory::Magic),
    ("lava", DamageCategory::Fire),
    ("fireball", DamageCategory::Fire),
    ("in_fire", DamageCategory::Fire),
    ("on_fire", DamageCategory::Fire),
    ("campfire", DamageCategory::Fire),
    ("fire", DamageCategory::Fire),
    ("falling", DamageCategory::Fall),
    ("fall", DamageCategory::Fall),
];

/// Vanilla-style damage-type tags, in the contract's priority order.
const TAG_MATCHES: &[(&str, DamageCategory)] = &[
    ("is_fall", DamageCategory::Fall),
    ("is_fire", DamageCategory::Fire),
    ("is_drowning", DamageCategory::Drown),
    ("is_freezing", DamageCategory::Freeze),
    ("is_explosion", DamageCategory::Explosion),
    ("is_projectile", DamageCategory::Projectile),
    ("is_wither", DamageCategory::Wither),
];

/// Classifies a damage type by its (optionally namespaced) id.
///
/// This is the id-only fallback: exact registered-name tokens first, then broader
/// substring tokens, then `Other`. Prefer [`categorize_with_tags`] when the caller can
/// supply vanilla damage-type tags.
pub fn categorize(damage_type_id: &str) -> DamageCategory {
    let path = damage_path(damage_type_id);
    for (token, category) in EXACT_ID_MATCHES {
        if path == *token {
            return *category;
        }
    }
    for (token, category) in SUBSTRING_ID_MATCHES {
        if path.contains(token) {
            return *category;
        }
    }
    DamageCategory::Other
}

/// Classifies a damage type by caller-supplied vanilla-style tags, falling back to
/// [`categorize`] when no recognised tag is present.
///
/// `tags` entries may be bare (`is_fall`) or namespaced (`minecraft:is_fall`).
/// The first matching tag in [`TAG_MATCHES`] order wins.
pub fn categorize_with_tags(damage_type_id: &str, tags: &[&str]) -> DamageCategory {
    for (tag, category) in TAG_MATCHES {
        if tags.iter().any(|supplied| tag_path(supplied) == *tag) {
            return *category;
        }
    }
    categorize(damage_type_id)
}

/// The leveled `Type_<CATEGORY>` concept for a category, if one exists.
///
/// Returns `None` for:
/// - `FIRE` — the retired `Type_FIRE` ladder was replaced by the one-time `Env_Fire`;
/// - `FALL`, `STARVE`, `DROWN`, `SUFFOCATE`, `VOID`, `CONTACT` — retired ladders with
///   one-time `Env_*` immunity concepts (see [`env_immunity_concept`]).
pub fn type_concept(category: DamageCategory) -> Option<&'static str> {
    match category {
        DamageCategory::Wither => Some("Type_WITHER"),
        DamageCategory::Freeze => Some("Type_FREEZE"),
        DamageCategory::Lightning => Some("Type_LIGHTNING"),
        DamageCategory::Explosion => Some("Type_EXPLOSION"),
        DamageCategory::Projectile => Some("Type_PROJECTILE"),
        DamageCategory::Magic => Some("Type_MAGIC"),
        DamageCategory::Mob => Some("Type_MOB"),
        DamageCategory::Player => Some("Type_PLAYER"),
        DamageCategory::Other => Some("Type_OTHER"),
        DamageCategory::Fall
        | DamageCategory::Drown
        | DamageCategory::Starve
        | DamageCategory::Suffocate
        | DamageCategory::Void
        | DamageCategory::Contact
        | DamageCategory::Fire => None,
    }
}

/// The one-time `Env_*` concept that grants full immunity to a category, if any.
///
/// Mirrors `AdaptionEvents.envImmunityConcept`: FALL→`Env_FallDamage`,
/// STARVE→`Env_Starve`, DROWN→`Env_Drowning`, FIRE→`Env_Fire`,
/// SUFFOCATE→`Env_Suffocate`, VOID→`Env_Void`, CONTACT→`Env_Thorns`.
pub fn env_immunity_concept(category: DamageCategory) -> Option<&'static str> {
    match category {
        DamageCategory::Fall => Some("Env_FallDamage"),
        DamageCategory::Starve => Some("Env_Starve"),
        DamageCategory::Drown => Some("Env_Drowning"),
        DamageCategory::Fire => Some("Env_Fire"),
        DamageCategory::Suffocate => Some("Env_Suffocate"),
        DamageCategory::Void => Some("Env_Void"),
        DamageCategory::Contact => Some("Env_Thorns"),
        _ => None,
    }
}

/// True only for the vanilla lava damage type itself — never for fire ticks,
/// fireballs, campfires or any other fire-family source.
pub fn is_lava_damage(damage_type_id: &str) -> bool {
    damage_path(damage_type_id) == "lava"
}

/// One-time (`Env_*`) full immunity.
///
/// For Fire+Lava the source-specific `Env_Lava` is checked instead of `Env_Fire`,
/// preserving the upstream fire/lava split (a fire arrow cannot grant lava immunity).
pub fn full_immunity_applies(
    is_active: impl Fn(&str) -> bool,
    category: DamageCategory,
    damage_type_id: &str,
) -> bool {
    if category == DamageCategory::Fire && is_lava_damage(damage_type_id) {
        return is_active("Env_Lava");
    }
    match env_immunity_concept(category) {
        Some(concept) => is_active(concept),
        None => false,
    }
}

/// Maxed-level (`Type_*`) immunity for the categories that still have a level ladder.
///
/// - `bypasses_invulnerability` (vanilla `/kill`) is never nullified.
/// - The one-time `Env_*` families return `false` here; they are handled by
///   [`full_immunity_applies`] instead.
/// - `damage_type_id` is accepted for call-site symmetry with upstream
///   `isMaxedImmunity(data, category, source)`; lava is deliberately not re-checked
///   because Fire is already excluded as a one-time family.
/// - Direct living-attacker `Contact_<mob>` immunity is a separate check:
///   [`contact_immunity_applies`].
pub fn maxed_immunity_applies(
    level: impl Fn(&str) -> u8,
    category: DamageCategory,
    damage_type_id: &str,
    max_level: u8,
    bypasses_invulnerability: bool,
) -> bool {
    let _ = damage_type_id;
    if bypasses_invulnerability {
        return false;
    }
    match category {
        DamageCategory::Fall
        | DamageCategory::Starve
        | DamageCategory::Drown
        | DamageCategory::Fire
        | DamageCategory::Suffocate
        | DamageCategory::Void
        | DamageCategory::Contact => false,
        _ => match type_concept(category) {
            Some(concept) => level(concept) >= max_level,
            None => false,
        },
    }
}

/// Completed (or early-cutoff) `Contact_<mob>` immunity.
///
/// `direct_attacker_path` must be the path of a **direct living attacker**
/// (e.g. `Some("zombie")`); a projectile or environmental source passes `None` and
/// never qualifies. The completed ladder (`level >= max_level`) is always total;
/// `cutoff > 0` additionally grants immunity at an earlier configured level.
/// Callers must enforce `BYPASSES_INVULNERABILITY` themselves — this pure signature
/// intentionally carries no damage-source flag.
pub fn contact_immunity_applies(
    level: impl Fn(&str) -> u8,
    direct_attacker_path: Option<&str>,
    max_level: u8,
    cutoff: u8,
) -> bool {
    let Some(path) = direct_attacker_path else {
        return false;
    };
    if path.is_empty() {
        return false;
    }
    let concept = format!("Contact_{}", path);
    let contact_level = level(&concept);
    contact_level >= max_level || (cutoff > 0 && contact_level >= cutoff)
}

/// Category-wide capstone, mirroring upstream `TOTAL_IMMUNITY_CATEGORIES`
/// (`AdaptionEvents.java:816-817`), which contains only `PROJECTILE` and `OTHER`.
///
/// These two cannot stall a training ladder: a projectile has no direct living
/// attacker and `OTHER` is the residue no category claims. Every other category —
/// including `WITHER`, `EXPLOSION`, `MAGIC`, `LIGHTNING` and `FREEZE` — stays on the
/// reduction curve even at max level.
pub fn category_capstone_applies(
    level: impl Fn(&str) -> u8,
    category: DamageCategory,
    max_level: u8,
    bypasses_invulnerability: bool,
) -> bool {
    if bypasses_invulnerability {
        return false;
    }
    if !matches!(category, DamageCategory::Projectile | DamageCategory::Other) {
        return false;
    }
    match type_concept(category) {
        Some(concept) => level(concept) >= max_level,
        None => false,
    }
}

/// Fraction of incoming damage removed at a given level.
///
/// A simple pure default: 0.10 per level, capped at 0.75. Upstream drives this from
/// `AdaptionConfig.DEFENSE_REDUCTION_LEVELS` (default percentages
/// `{5,10,15,20,25,30,45,60}`) and the native engine uses per-concept tables; this
/// helper deliberately has no config dependency so callers can layer their own curve
/// on top. `level` is clamped to `max_level`; level 0 always yields 0.0.
const REDUCTION_PER_LEVEL: f32 = 0.10;
const MAX_REDUCTION: f32 = 0.75;

pub fn mitigation_level_reduction(category: DamageCategory, level: u8, max_level: u8) -> f32 {
    let _ = category; // the curve is category-independent in this pure default
    let effective = level.min(max_level);
    (effective as f32 * REDUCTION_PER_LEVEL).min(MAX_REDUCTION)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lvl<'a>(map: &'a [(&'a str, u8)]) -> impl Fn(&str) -> u8 + 'a {
        move |concept| {
            map.iter()
                .find(|(key, _)| *key == concept)
                .map(|(_, value)| *value)
                .unwrap_or(0)
        }
    }

    #[test]
    fn categorize_covers_all_sixteen_categories() {
        let cases: &[(&str, DamageCategory)] = &[
            ("minecraft:fall", DamageCategory::Fall),
            ("minecraft:wither_skull", DamageCategory::Wither),
            ("minecraft:in_fire", DamageCategory::Fire),
            ("minecraft:drown", DamageCategory::Drown),
            ("minecraft:powder_snow", DamageCategory::Freeze),
            ("minecraft:lightning_bolt", DamageCategory::Lightning),
            ("minecraft:fireworks", DamageCategory::Explosion),
            ("minecraft:arrow", DamageCategory::Projectile),
            ("minecraft:indirect_magic", DamageCategory::Magic),
            ("minecraft:mob_attack", DamageCategory::Mob),
            ("minecraft:player_attack", DamageCategory::Player),
            ("minecraft:cactus", DamageCategory::Contact),
            ("minecraft:starve", DamageCategory::Starve),
            ("minecraft:suffocation", DamageCategory::Suffocate),
            ("minecraft:out_of_world", DamageCategory::Void),
            ("minecraft:generic", DamageCategory::Other),
        ];
        for (id, expected) in cases {
            assert_eq!(categorize(id), *expected, "id {}", id);
        }
        assert_eq!(DamageCategory::ALL.len(), 16);
    }

    #[test]
    fn categorize_substring_and_ordering_rules() {
        assert_eq!(categorize("somemod:acid_burn_fire"), DamageCategory::Fire);
        assert_eq!(categorize("somemod:mod_explosion"), DamageCategory::Explosion);
        assert_eq!(categorize("somemod:giant_blast"), DamageCategory::Explosion);
        assert_eq!(categorize("minecraft:fireball"), DamageCategory::Fire);
        assert_eq!(categorize("minecraft:fireworks"), DamageCategory::Explosion);
        assert_eq!(categorize("minecraft:thorns"), DamageCategory::Contact);
        assert_eq!(categorize("minecraft:sweet_berry_bush"), DamageCategory::Contact);
        assert_eq!(categorize("minecraft:mace_smash"), DamageCategory::Player);
        assert_eq!(categorize("minecraft:fly_into_wall"), DamageCategory::Suffocate);
    }

    #[test]
    fn lava_is_distinct_from_fireball_and_fire_ticks() {
        assert!(is_lava_damage("lava"));
        assert!(is_lava_damage("minecraft:lava"));
        assert!(!is_lava_damage("minecraft:fireball"));
        assert!(!is_lava_damage("minecraft:in_fire"));
        assert!(!is_lava_damage("minecraft:hot_floor"));
        assert_eq!(categorize("minecraft:lava"), DamageCategory::Fire);
        assert_eq!(categorize("minecraft:fireball"), DamageCategory::Fire);
    }

    #[test]
    fn tags_override_id_and_fall_back() {
        assert_eq!(
            categorize_with_tags("minecraft:generic", &["is_fall"]),
            DamageCategory::Fall
        );
        assert_eq!(
            categorize_with_tags("minecraft:generic", &["minecraft:is_fire"]),
            DamageCategory::Fire
        );
        assert_eq!(
            categorize_with_tags("minecraft:generic", &["is_drowning"]),
            DamageCategory::Drown
        );
        assert_eq!(
            categorize_with_tags("minecraft:generic", &["is_freezing"]),
            DamageCategory::Freeze
        );
        assert_eq!(
            categorize_with_tags("minecraft:generic", &["is_explosion"]),
            DamageCategory::Explosion
        );
        assert_eq!(
            categorize_with_tags("minecraft:generic", &["is_projectile"]),
            DamageCategory::Projectile
        );
        assert_eq!(
            categorize_with_tags("minecraft:generic", &["is_wither"]),
            DamageCategory::Wither
        );
        assert_eq!(
            categorize_with_tags("minecraft:arrow", &["unrelated_tag"]),
            DamageCategory::Projectile
        );
        assert_eq!(categorize_with_tags("minecraft:arrow", &[]), DamageCategory::Projectile);
    }

    #[test]
    fn type_concept_excludes_fire_and_one_time_categories() {
        for category in [
            DamageCategory::Fire,
            DamageCategory::Fall,
            DamageCategory::Starve,
            DamageCategory::Drown,
            DamageCategory::Suffocate,
            DamageCategory::Void,
            DamageCategory::Contact,
        ] {
            assert_eq!(type_concept(category), None, "{:?}", category);
        }
        let expected: &[(DamageCategory, &str)] = &[
            (DamageCategory::Wither, "Type_WITHER"),
            (DamageCategory::Freeze, "Type_FREEZE"),
            (DamageCategory::Lightning, "Type_LIGHTNING"),
            (DamageCategory::Explosion, "Type_EXPLOSION"),
            (DamageCategory::Projectile, "Type_PROJECTILE"),
            (DamageCategory::Magic, "Type_MAGIC"),
            (DamageCategory::Mob, "Type_MOB"),
            (DamageCategory::Player, "Type_PLAYER"),
            (DamageCategory::Other, "Type_OTHER"),
        ];
        for (category, concept) in expected {
            assert_eq!(type_concept(*category), Some(*concept), "{:?}", category);
        }
    }

    #[test]
    fn env_immunity_mapping() {
        let expected: &[(DamageCategory, &str)] = &[
            (DamageCategory::Fall, "Env_FallDamage"),
            (DamageCategory::Starve, "Env_Starve"),
            (DamageCategory::Drown, "Env_Drowning"),
            (DamageCategory::Fire, "Env_Fire"),
            (DamageCategory::Suffocate, "Env_Suffocate"),
            (DamageCategory::Void, "Env_Void"),
            (DamageCategory::Contact, "Env_Thorns"),
        ];
        for (category, concept) in expected {
            assert_eq!(env_immunity_concept(*category), Some(*concept), "{:?}", category);
        }
        for category in [
            DamageCategory::Wither,
            DamageCategory::Freeze,
            DamageCategory::Lightning,
            DamageCategory::Explosion,
            DamageCategory::Projectile,
            DamageCategory::Magic,
            DamageCategory::Mob,
            DamageCategory::Player,
            DamageCategory::Other,
        ] {
            assert_eq!(env_immunity_concept(category), None, "{:?}", category);
        }
    }

    #[test]
    fn full_immunity_uses_lava_override_and_env_concepts() {
        let active = |concept: &str| concept == "Env_Lava" || concept == "Env_FallDamage";
        assert!(full_immunity_applies(
            &active,
            DamageCategory::Fire,
            "minecraft:lava"
        ));
        assert!(!full_immunity_applies(
            &active,
            DamageCategory::Fire,
            "minecraft:in_fire"
        ));
        assert!(full_immunity_applies(
            &active,
            DamageCategory::Fall,
            "minecraft:fall"
        ));
        assert!(!full_immunity_applies(
            &active,
            DamageCategory::Mob,
            "minecraft:mob_attack"
        ));
        let none = |_concept: &str| false;
        assert!(!full_immunity_applies(&none, DamageCategory::Fire, "minecraft:lava"));
    }

    #[test]
    fn maxed_immunity_excludes_one_time_categories_and_respects_bypass() {
        let maxed = lvl(&[("Type_MAGIC", 8), ("Type_PROJECTILE", 8), ("Env_Fire", 8)]);
        assert!(maxed_immunity_applies(
            &maxed,
            DamageCategory::Magic,
            "minecraft:magic",
            8,
            false
        ));
        assert!(!maxed_immunity_applies(
            &maxed,
            DamageCategory::Magic,
            "minecraft:magic",
            9,
            false
        ));
        assert!(!maxed_immunity_applies(
            &maxed,
            DamageCategory::Magic,
            "minecraft:magic",
            8,
            true
        ));
        for category in [
            DamageCategory::Fall,
            DamageCategory::Starve,
            DamageCategory::Drown,
            DamageCategory::Fire,
            DamageCategory::Suffocate,
            DamageCategory::Void,
            DamageCategory::Contact,
        ] {
            assert!(
                !maxed_immunity_applies(&maxed, category, "minecraft:generic", 8, false),
                "{:?}",
                category
            );
        }
    }

    #[test]
    fn contact_immunity_requires_attacker_and_honours_cutoff() {
        let levels = lvl(&[("Contact_zombie", 5), ("Contact_creeper", 8)]);
        assert!(contact_immunity_applies(&levels, Some("creeper"), 8, 0));
        assert!(contact_immunity_applies(&levels, Some("zombie"), 8, 5));
        assert!(!contact_immunity_applies(&levels, Some("zombie"), 8, 0));
        assert!(!contact_immunity_applies(&levels, Some("zombie"), 8, 6));
        assert!(!contact_immunity_applies(&levels, None, 8, 5));
        assert!(!contact_immunity_applies(&levels, Some(""), 8, 5));
        assert!(!contact_immunity_applies(&levels, Some("skeleton"), 8, 5));
    }

    #[test]
    fn capstone_only_covers_projectile_and_other() {
        let maxed = lvl(&[
            ("Type_PROJECTILE", 8),
            ("Type_OTHER", 8),
            ("Type_MAGIC", 8),
            ("Type_WITHER", 8),
            ("Type_EXPLOSION", 8),
            ("Type_LIGHTNING", 8),
            ("Type_FREEZE", 8),
        ]);
        assert!(category_capstone_applies(&maxed, DamageCategory::Projectile, 8, false));
        assert!(category_capstone_applies(&maxed, DamageCategory::Other, 8, false));
        for category in [
            DamageCategory::Magic,
            DamageCategory::Wither,
            DamageCategory::Explosion,
            DamageCategory::Lightning,
            DamageCategory::Freeze,
            DamageCategory::Mob,
        ] {
            assert!(
                !category_capstone_applies(&maxed, category, 8, false),
                "{:?}",
                category
            );
        }
        assert!(!category_capstone_applies(
            &maxed,
            DamageCategory::Projectile,
            8,
            true
        ));
    }

    #[test]
    fn mitigation_curve_is_clamped() {
        assert_eq!(mitigation_level_reduction(DamageCategory::Mob, 0, 8), 0.0);
        assert_eq!(mitigation_level_reduction(DamageCategory::Mob, 1, 8), 0.10);
        assert_eq!(mitigation_level_reduction(DamageCategory::Mob, 7, 8), 0.70);
        assert_eq!(mitigation_level_reduction(DamageCategory::Mob, 8, 8), 0.75);
        assert_eq!(mitigation_level_reduction(DamageCategory::Mob, 20, 8), 0.75);
    }
}
