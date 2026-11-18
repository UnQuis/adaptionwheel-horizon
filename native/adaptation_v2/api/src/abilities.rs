//! P1.5 (active-ability dispatch) + P1.6 (diagnostic command responder) — pure Rust.
//!
//! This module deliberately depends on **nothing** outside `std`: no `mod_api`, no
//! `Host`, no state globals, no `serde`. The integration lead owns all host wiring:
//!
//! * `AbilityDispatch` mirrors Java `server/AbilityDispatch.java`. The single
//!   persisted slot, the server-side unlock re-check, the "first unlocked fallback"
//!   and the immediate sync are all here. The only host-facing primitive is the
//!   `sync: &mut dyn FnMut(String)` callback: on success the **raw ability id** is
//!   emitted, which the lead maps to `syncAdaption` / host feedback. The `Ok(String)`
//!   return value carries the same id for callers that want it directly.
//! * `handle_diagnostic` mirrors the diagnostic half of Java
//!   `server/AdaptionCommand.java`. It parses a raw command line and returns a
//!   `DiagnosticOutcome`. Pure read-only subcommands return `Reply(text)` (already
//!   prefixed with `[adaptionwheel]`, matching `commands_impl.rs`); state-changing
//!   subcommands return `Mutate { kind, concept }` so the lead performs the real
//!   mutation against player state and sends its own confirmation.
//!
//! ## Design choices (differences from Java, documented for the lead)
//!
//! 1. `register(id, unlock, activate)` takes the **unlock predicate before** the
//!    activate function. Java's `AbilityDispatch.register(id, activate, unlock)`
//!    uses the opposite order; this module follows the required native signature.
//!    Both are plain `fn` pointers so registration stays `'static`, copyable and
//!    trivial to unit-test. A new ability still needs one registration call.
//! 2. `activate: fn()` takes no player argument because the pure layer has no
//!    player type. The stored function is still invoked on a successful activate
//!    (a no-op for the built-ins; a global side effect is possible via `static`s),
//!    and the returned id is what the lead uses to drive the real host effect.
//!    There is **no** universal activation key in the pure layer — the lead maps
//!    one key/bind to `AbilityDispatch::activate`.
//! 3. `is_active: &dyn Fn(&str) -> bool` is the read-only view of
//!    `PlayerAdaption::active(concept)` (adapted **and** enabled). Unlock
//!    predicates never see the player, only this closure.
//! 4. Immediate sync is represented by the callback; there is no 1 Hz sync timer
//!    here. `select` and `activate` both fire it on success, matching the Java
//!    comment that waiting for the next sync tick leaves the HUD stale.
//! 5. Built-in ability ids: upstream `ActiveAbilities` ships **no** built-ins; the
//!    only real registration is `DiveAbility.ID = "dive"` (unlocked by
//!    `Env_FallDamage` or `Mutation_Flight`). `flight` and `mutation` are no-op
//!    placeholders so selection/fallback can be exercised and so future
//!    mutations have a stable id — register the real behaviour before exposing
//!    them to players. `with_builtins()` mirrors Java's `commonSetup` registration;
//!    `new()`/`Default` start empty (deterministic tests).
//! 6. **There is no universal activation key in this port.** Horizon `8bc19c5f97`
//!    exposes no shared keybind / input payload that reaches the server for
//!    "activate the selected ability" — adaptation state lives in `PlayerAdaption`
//!    (concepts/levels) and the choice in `Extras.selectedAbility`, but there is
//!    no activation packet. Activation is therefore available **ONLY** through
//!    the command path `/adaptionwheel wheel` (plus the optional raw-line
//!    `ability <id>` / `activate` diagnostic subcommands added below). See
//!    [`activation_key_help`] for the exact wording; hooks should reach
//!    [`activate_for_hook`] only as the server-side consequence of such a
//!    command, never from a key event.

use std::collections::{BTreeMap, BTreeSet};

// ===========================================================================
// P1.5 — Active ability dispatch
// ===========================================================================

/// A non-capturing unlock gate. Receives `is_active`, the read-only "is this
/// concept adapted AND enabled for the current player?" closure supplied by the
/// lead. Mirrors Java `Predicate<PlayerAdaption>` narrowed to concept queries.
pub type UnlockPredicate = fn(&dyn Fn(&str) -> bool) -> bool;

/// A non-capturing activation side effect. See module docs for why this is
/// parameterless in the pure layer.
pub type ActivateFn = fn();

/// Upstream `DiveAbility.ID` — the only ability actually registered in Java.
pub const ABILITY_DIVE: &str = "dive";
/// Placeholder for a future flight active ability (gate: `Mutation_Flight`).
pub const ABILITY_FLIGHT: &str = "flight";
/// Placeholder for a future mutation active ability (gate: `Mutation_Fist`).
pub const ABILITY_MUTATION: &str = "mutation";

/// The built-in id set, in registration order (dive first, as in `commonSetup`).
pub const BUILTIN_ABILITY_IDS: [&str; 3] = [ABILITY_DIVE, ABILITY_FLIGHT, ABILITY_MUTATION];

// Concept ids used by built-in unlock gates (exact strings from `Concepts.java`).
const CONCEPT_ENV_FALL: &str = "Env_FallDamage";
const CONCEPT_MUTATION_FLIGHT: &str = "Mutation_Flight";
const CONCEPT_MUTATION_FIST: &str = "Mutation_Fist";
const CONCEPT_MUTATION_IMPACT: &str = "Mutation_Impact";

#[derive(Clone)]
struct AbilityHandler {
    id: String,
    unlock: UnlockPredicate,
    activate: ActivateFn,
}

/// The single active-ability slot — one key, one chosen ability, server-authoritative.
///
/// Construct with `new()` (empty) or `with_builtins()` (dive + two placeholders).
/// Registration order is stable and is the fallback order used by
/// [`AbilityDispatch::resolve_selected`], matching iteration over Java's
/// `ActiveAbilities.all()`.
#[derive(Clone)]
pub struct AbilityDispatch {
    handlers: Vec<AbilityHandler>,
    /// The persisted `Extras.selectedAbility` value (may be stale/locked).
    selected: Option<String>,
}

impl Default for AbilityDispatch {
    fn default() -> Self {
        Self::new()
    }
}

impl AbilityDispatch {
    /// An empty registry. No ability is unlocked until something is registered.
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
            selected: None,
        }
    }

    /// Registers the built-in no-op handlers (see module docs). Mirrors the
    /// `event.enqueueWork` block in Java `AdaptionWheel.commonSetup`.
    pub fn with_builtins() -> Self {
        let mut dispatch = Self::new();
        dispatch.register_builtins();
        dispatch
    }

    /// Adds the built-in handlers. Re-registration replaces existing ids in place.
    pub fn register_builtins(&mut self) {
        self.register(ABILITY_DIVE, unlock_dive, activate_noop);
        self.register(ABILITY_FLIGHT, unlock_flight, activate_noop);
        self.register(ABILITY_MUTATION, unlock_mutation, activate_noop);
    }

    /// Registers (or replaces) an ability. Blank ids are ignored, as in Java.
    ///
    /// Argument order is `(id, unlock, activate)` — see module docs.
    pub fn register(
        &mut self,
        id: impl Into<String>,
        unlock: UnlockPredicate,
        activate: ActivateFn,
    ) {
        let id = id.into();
        if id.trim().is_empty() {
            return;
        }
        let handler = AbilityHandler {
            id: id.clone(),
            unlock,
            activate,
        };
        if let Some(existing) = self.handlers.iter_mut().find(|h| h.id == id) {
            *existing = handler;
        } else {
            self.handlers.push(handler);
        }
    }

    /// Registered ids in registration order (Java `ActiveAbilities.all()` order).
    pub fn ids(&self) -> Vec<String> {
        self.handlers.iter().map(|h| h.id.clone()).collect()
    }

    /// The raw persisted selection, before any unlock re-check.
    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    /// Whether a registered id passes its unlock gate right now. An unknown id
    /// is never unlocked.
    pub fn is_unlocked(&self, id: &str, is_active: &dyn Fn(&str) -> bool) -> bool {
        match self.handlers.iter().find(|h| h.id == id) {
            Some(handler) => (handler.unlock)(is_active),
            None => false,
        }
    }

    /// The persisted id if still unlocked, otherwise the first unlocked
    /// registered ability, otherwise `None`. Protects old saves with an empty
    /// selection and abilities that close their own gate (Java lines 52–72).
    pub fn resolve_selected(&self, is_active: &dyn Fn(&str) -> bool) -> Option<String> {
        if let Some(id) = &self.selected {
            if self.is_unlocked(id, is_active) {
                return Some(id.clone());
            }
        }
        self.handlers
            .iter()
            .find(|h| (h.unlock)(is_active))
            .map(|h| h.id.clone())
    }

    /// Server-side selection. Never trusts a client-sent id without re-checking
    /// the gate. On success the persisted slot is updated and `sync` is fired
    /// immediately with the raw id (no waiting for the next sync tick).
    ///
    /// Errors: `"ability '<id>' is locked"` (unknown or gate refused).
    pub fn select(
        &mut self,
        id: &str,
        is_active: &dyn Fn(&str) -> bool,
        sync: &mut dyn FnMut(String),
    ) -> Result<String, String> {
        if !self.is_unlocked(id, is_active) {
            return Err(format!("ability '{}' is locked", id));
        }
        self.selected = Some(id.to_string());
        sync(id.to_string());
        Ok(id.to_string())
    }

    /// Called by the universal activation key. Re-checks every condition: the
    /// wheel must be worn, a selection must resolve, and that selection must
    /// still be unlocked. On success the stored activate function runs, `sync`
    /// receives the raw id, and the id is returned.
    ///
    /// Errors: `"wheel not worn"`, `"no unlocked ability available"`.
    pub fn activate(
        &mut self,
        wheel_worn: bool,
        is_active: &dyn Fn(&str) -> bool,
        sync: &mut dyn FnMut(String),
    ) -> Result<String, String> {
        if !wheel_worn {
            return Err("wheel not worn".to_string());
        }
        let Some(id) = self.resolve_selected(is_active) else {
            return Err("no unlocked ability available".to_string());
        };
        // Re-check after re-resolving, exactly like Java's activate() → Entry lookup.
        if !self.is_unlocked(&id, is_active) {
            return Err(format!("ability '{}' is locked", id));
        }
        if let Some(handler) = self.handlers.iter().find(|h| h.id == id) {
            (handler.activate)();
        }
        sync(id.clone());
        Ok(id)
    }
}

fn unlock_dive(is_active: &dyn Fn(&str) -> bool) -> bool {
    is_active(CONCEPT_ENV_FALL) || is_active(CONCEPT_MUTATION_FLIGHT)
}

fn unlock_flight(is_active: &dyn Fn(&str) -> bool) -> bool {
    is_active(CONCEPT_MUTATION_FLIGHT)
}

fn unlock_mutation(is_active: &dyn Fn(&str) -> bool) -> bool {
    is_active(CONCEPT_MUTATION_FIST) || is_active(CONCEPT_MUTATION_IMPACT)
}

fn activate_noop() {}

// ===========================================================================
// P1.5 — Integration-ready helpers (pure; no mod_api)
// ===========================================================================

/// Builds the read-only `is_active` view consumed by every `AbilityDispatch`
/// method from a snapshot of the player's adaptation data.
///
/// A concept is reported active when it is in `adapted` **or** has a positive
/// stored level in `levels` — the same union `PlayerAdaptionV2::get_adapt_count`
/// uses, so one-time gates (`Env_FallDamage`, `Mutation_Flight`, ...) and any
/// future level-based gate both work. The caller must pass only **enabled**
/// concepts: upstream `PlayerAdaption::active` also requires `!disabled`, so
/// filter toggle-off ids out of `adapted` and/or use `level_or_zero` when
/// producing these two collections.
///
/// The returned closure borrows both collections, so it is valid only as long
/// as the snapshot; build it and use it within the same hook tick.
pub fn active_predicate_from_levels<'a>(
    levels: &'a BTreeMap<String, u8>,
    adapted: &'a BTreeSet<String>,
) -> impl Fn(&str) -> bool + 'a {
    move |concept| adapted.contains(concept) || levels.get(concept).copied().unwrap_or(0) > 0
}

/// Named thin wrapper around an `is_active` view: "is `concept` active for this
/// player right now?". Provided so integration code has a single documented
/// query point instead of invoking the borrowed closure directly.
pub fn ability_is_active(is_active: &dyn Fn(&str) -> bool, concept: &str) -> bool {
    is_active(concept)
}

/// Player-visible confirmation emitted through `host.commands().feedback` /
/// `player.tell` when a selection or activation is synced immediately.
///
/// The `sync: &mut dyn FnMut(String)` callback passed to [`AbilityDispatch::select`],
/// [`AbilityDispatch::activate`] and [`activate_for_hook`] carries the **raw
/// ability id** — that is the value the lead maps onto the host's `syncAdaption`.
/// This helper formats the human-readable companion line; it is not itself a
/// network payload.
pub fn immediate_sync_message(ability_id: &str) -> String {
    format!("[adaptionwheel] ability synced: {}", ability_id)
}

/// A ready-made registry with the built-ins already registered. Hooks can hold
/// one of these for the mod's lifetime (e.g. behind a `OnceLock<Mutex<_>>`)
/// instead of rebuilding it on every event.
pub fn builtin_dispatch() -> AbilityDispatch {
    AbilityDispatch::with_builtins()
}

/// Front door for hook code that activates the currently selected ability.
///
/// A thin wrapper over [`AbilityDispatch::activate`] with the same conditions
/// and errors (`"wheel not worn"`, `"no unlocked ability available"`,
/// `"ability '<id>' is locked"`). `sync` maps to **immediate host feedback /
/// sync**: pass a closure that forwards its `String` to
/// `host.commands().feedback(Text::literal(immediate_sync_message(id)))` and/or
/// `player.tell`, and performs the host-side `syncAdaption` with that same raw
/// id. No deferred 1 Hz sync tick exists in this layer.
///
/// Remember: there is no activation key event to call this from — invoke it only
/// as the server-side consequence of the command path described in
/// [`activation_key_help`].
pub fn activate_for_hook(
    dispatch: &mut AbilityDispatch,
    wheel_worn: bool,
    is_active: &dyn Fn(&str) -> bool,
    sync: &mut dyn FnMut(String),
) -> Result<String, String> {
    dispatch.activate(wheel_worn, is_active, sync)
}

/// Exact wording for `/adaptionwheel help` / diagnostics about how — and only
/// how — an active ability can be triggered in this port.
pub fn activation_key_help() -> &'static str {
    concat!(
        "activation: Horizon 8bc19c5f97 has no universal activation key or input ",
        "payload in this port — the mod API exposes no shared keybind/activation ",
        "packet reaching the server, and adaptation state only stores concepts, ",
        "levels and Extras.selectedAbility. Activation is available ONLY through ",
        "the command path `/adaptionwheel wheel` (wear/toggle the wheel to fire ",
        "the selected ability). Optionally the raw-line subcommands ",
        "`/adaptionwheel ability <id>` (select) and `/adaptionwheel activate` ",
        "(fire) are recognised by handle_diagnostic; route their Mutate outcomes ",
        "to activate_for_hook. There is no key hook to wire."
    )
}

// ===========================================================================
// P1.6 — Diagnostic command responder
// ===========================================================================

/// Domain keys in Java `AdaptationDomain` enum-ordinal order.
pub const DOMAIN_KEYS: [&str; 11] = [
    "damage",
    "effect",
    "movement",
    "physics",
    "mining",
    "environment",
    "combat",
    "perception",
    "entity",
    "existence",
    "special",
];

/// The read-only snapshot `handle_diagnostic` needs. The lead fills this from
/// player state; every field maps to a Java `PlayerAdaption` concept.
#[derive(Debug, Clone, Default)]
pub struct DiagnosticCtx {
    /// `PlayerAdaption.getAdaptCount()` (union of adapted + positive levels).
    pub adapt_count: usize,
    /// Adapted concepts (`data.adapted`), including one-time ones.
    pub active_ids: Vec<String>,
    /// Level per concept (`data.levels`).
    pub level_map: BTreeMap<String, u8>,
    /// Persisted `Extras.selectedAbility`.
    pub selected_ability: Option<String>,
    /// `AdaptionEvents.isWearingWheel(player)`.
    pub wheel_worn: bool,
}

/// State changes the pure layer must delegate to the lead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationKind {
    /// `grantall [domain]`; `concept` holds the optional domain key, or `None`.
    GrantAll,
    /// `shed <concept>`.
    Shed,
    /// `ungrant <concept>`.
    Ungrant,
    /// `analyze <concept>` (start an analysis task).
    Analyze,
    /// `ability <id>`; `concept` holds the ability id to select. Route this to
    /// [`AbilityDispatch::select`] on the lead's held dispatch.
    SelectAbility,
    /// `activate`; `concept` is `None`. Route this to [`activate_for_hook`].
    ActivateAbility,
}

/// Result of a handled diagnostic command.
///
/// * `Reply(text)` — pure read-only output; send `text` to the command source.
/// * `Mutate { kind, concept }` — perform the named state mutation in the host
///   layer, then send your own confirmation/denial. `concept` is the domain for
///   `GrantAll`, the concept id for `Shed`/`Ungrant`/`Analyze`, the ability id
///   for `SelectAbility`, and `None` for `ActivateAbility`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticOutcome {
    Reply(String),
    Mutate {
        kind: MutationKind,
        concept: Option<String>,
    },
}

/// Subcommands recognised by [`handle_diagnostic`].
///
/// The first nine are the original diagnostic surface (kept unchanged); the last
/// two are the optional command-only activation path documented in
/// [`activation_key_help`].
pub const SUPPORTED_SUBCOMMANDS: [&str; 11] = [
    "list", "info", "grantall", "shed", "ungrant", "analyze", "wheel", "flight", "registry",
    "ability", "activate",
];

/// Parses a raw command line and returns the diagnostic outcome, or `None` when
/// the line is empty or is not one of ours (so the lead can fall through).
///
/// Accepts an optional leading `/adaptionwheel` or `/aw` alias, quoted concept
/// arguments, and extra whitespace. Read-only subcommands are `list`, `info`,
/// `wheel`, `flight`, `registry`; `grantall`, `shed`, `ungrant`, `analyze`,
/// `ability` and `activate` delegate to the host via `Mutate`. The last two are
/// the command-only activation path — see [`activation_key_help`].
pub fn handle_diagnostic(command_line: &str, ctx: &DiagnosticCtx) -> Option<DiagnosticOutcome> {
    let tokens = tokenize(command_line);
    let sub = match tokens.first().map(String::as_str) {
        Some(sub) if SUPPORTED_SUBCOMMANDS.contains(&sub) => sub,
        _ => return None,
    };
    let args = &tokens[1..];

    let outcome = match sub {
        "list" => Reply(list_reply(ctx, args.first().map(String::as_str))),
        "info" => Reply(info_reply(ctx, args.first().map(String::as_str))),
        "wheel" => Reply(wheel_reply(ctx)),
        "flight" => Reply(flight_reply(ctx)),
        "registry" => Reply(registry_reply()),
        "grantall" => Mutate {
            kind: MutationKind::GrantAll,
            concept: args.first().cloned(),
        },
        "shed" => required_mutation(MutationKind::Shed, args, "shed <concept>"),
        "ungrant" => required_mutation(MutationKind::Ungrant, args, "ungrant <concept>"),
        "analyze" => required_mutation(MutationKind::Analyze, args, "analyze <concept>"),
        "ability" => required_mutation(MutationKind::SelectAbility, args, "ability <id>"),
        "activate" => Mutate {
            kind: MutationKind::ActivateAbility,
            concept: None,
        },
        _ => return None,
    };
    Some(outcome)
}

use DiagnosticOutcome::{Mutate, Reply};

fn required_mutation(kind: MutationKind, args: &[String], usage: &str) -> DiagnosticOutcome {
    match args.first() {
        Some(concept) if !concept.is_empty() => Mutate {
            kind,
            concept: Some(concept.clone()),
        },
        _ => Reply(format!("[adaptionwheel] usage: /adaptionwheel {}", usage)),
    }
}

/// Splits on whitespace, strips surrounding quotes (Brigadier quoted concepts)
/// and an optional leading command alias.
fn tokenize(command_line: &str) -> Vec<String> {
    let mut tokens: Vec<String> = command_line
        .split_whitespace()
        .map(|raw| raw.trim_matches('"').to_string())
        .filter(|t| !t.is_empty())
        .collect();
    if let Some(first) = tokens.first() {
        let name = first.trim_start_matches('/');
        if name == "adaptionwheel" || name == "aw" {
            tokens.remove(0);
        }
    }
    tokens
}

/// Java `AdaptationRegistry.domainOf` prefix rules, applied case-insensitively
/// so both upstream ids (`Env_FallDamage`) and native ids (`env_fall`) classify.
fn domain_of(concept: &str) -> &'static str {
    let c = concept.to_ascii_lowercase();
    if let Some(tail) = c.strip_prefix("env_") {
        return if matches!(
            tail,
            "ice" | "slime" | "cobweb" | "knockback" | "falldamage" | "fall" | "fall_damage"
        ) {
            "physics"
        } else {
            "environment"
        };
    }
    if c.starts_with("type_") {
        "damage"
    } else if c.starts_with("debuff_") || c.starts_with("effect_") {
        "effect"
    } else if c.starts_with("move_") {
        "movement"
    } else if c.starts_with("mine_") || c.starts_with("fist_") {
        "mining"
    } else if c.starts_with("combat_fist") || c.starts_with("combat_") {
        "combat"
    } else if c.starts_with("percep_") {
        "perception"
    } else if c.starts_with("contact_")
        || c.starts_with("offense_")
        || c.starts_with("drop_")
        || c.starts_with("existence_")
    {
        // Java: Contact_/Offense_NPC_/Drop_NPC_ -> ENTITY, Existence_ -> EXISTENCE.
        if c.starts_with("existence_") {
            "existence"
        } else {
            "entity"
        }
    } else {
        "special"
    }
}

/// Mirrors Java `Concepts.isLevelBased` / registry `leveled` flag closely enough
/// for diagnostics: leveled damage categories, Self_Damage, Mine_Labor,
/// Combat_Cooldown and the fist ladders; everything else is one-time.
fn is_leveled(concept: &str) -> bool {
    let c = concept.to_ascii_lowercase();
    c.starts_with("type_")
        || matches!(c.as_str(), "self_damage" | "mine_labor" | "combat_cooldown")
        || c.starts_with("fist_")
        || c.starts_with("combat_fist")
}

fn is_adapted(ctx: &DiagnosticCtx, concept: &str) -> bool {
    ctx.active_ids
        .iter()
        .any(|id| id.eq_ignore_ascii_case(concept))
}

fn domain_index(domain: &str) -> usize {
    DOMAIN_KEYS
        .iter()
        .position(|d| *d == domain)
        .unwrap_or(DOMAIN_KEYS.len())
}

fn list_reply(ctx: &DiagnosticCtx, filter: Option<&str>) -> String {
    let mut keys: Vec<&String> = Vec::new();
    for id in ctx.active_ids.iter().chain(ctx.level_map.keys()) {
        if !keys.iter().any(|k| *k == id) {
            keys.push(id);
        }
    }
    // Sort by explicit enum ordinal (not lexical key order) for exact parity with
    // Java's `Comparator.comparingInt(domain.ordinal()).thenComparing(concept)`.
    keys.sort_by(|a, b| {
        domain_index(domain_of(a))
            .cmp(&domain_index(domain_of(b)))
            .then_with(|| a.cmp(b))
    });

    let mut lines = vec![format!(
        "[adaptionwheel] list: {} concept(s){}",
        keys.len(),
        filter.map(|d| format!(" [{}]", d)).unwrap_or_default()
    )];
    let mut shown = 0usize;
    for key in keys {
        let domain = domain_of(key);
        if let Some(wanted) = filter {
            if !domain.eq_ignore_ascii_case(wanted) {
                continue;
            }
        }
        let state = if is_adapted(ctx, key) && !is_leveled(key) {
            "ADAPTED".to_string()
        } else {
            let level = ctx.level_map.get(key).copied().unwrap_or(0);
            if level >= 8 {
                "MAX".to_string()
            } else {
                format!("Lv.{}", level)
            }
        };
        lines.push(format!("  {} [{}] {}", key, domain, state));
        shown += 1;
    }
    if shown == 0 {
        lines.push("  (none)".to_string());
    }
    lines.join("\n")
}

fn info_reply(ctx: &DiagnosticCtx, concept: Option<&str>) -> String {
    let Some(concept) = concept else {
        return "[adaptionwheel] usage: /adaptionwheel info <concept>".to_string();
    };
    let domain = domain_of(concept);
    let leveled = is_leveled(concept);
    let level = ctx.level_map.get(concept).copied().unwrap_or(0);
    let adapted = is_adapted(ctx, concept);
    let kind = if leveled { "leveled 1-8" } else { "one-time" };
    let state = if leveled {
        format!("{}/8{}", level, if level >= 8 { " (MAX)" } else { "" })
    } else if adapted {
        "ADAPTED".to_string()
    } else {
        "-".to_string()
    };
    format!(
        "[adaptionwheel] info: {} [{}] {}\n  level={} max=8 adapted={} state={}",
        concept, domain, kind, level, adapted, state
    )
}

fn wheel_reply(ctx: &DiagnosticCtx) -> String {
    let dive_active = is_adapted(ctx, CONCEPT_ENV_FALL) || is_adapted(ctx, CONCEPT_MUTATION_FLIGHT);
    let ability = if dive_active { ABILITY_DIVE } else { "(none)" };
    format!(
        "[adaptionwheel] wheel data:\n  worn={} adapt_count={} active_ids={} levels={}\n  selected_ability={}\n  ability list to client={}\n  (item-vs-attachment divergence and equippedStack are host-side; not available in the pure layer)",
        ctx.wheel_worn,
        ctx.adapt_count,
        ctx.active_ids.len(),
        ctx.level_map.len(),
        ctx.selected_ability.as_deref().unwrap_or("(none)"),
        ability
    )
}

fn flight_reply(ctx: &DiagnosticCtx) -> String {
    let flight = is_adapted(ctx, CONCEPT_MUTATION_FLIGHT);
    format!(
        "[adaptionwheel] flight diagnostics:\n  wheel worn: {}\n  Mutation_Flight active: {}\n  selected ability: {}\n  adapt_count: {}\n  (config flag, altitude, gamemode, mayfly/flying and wheel-item counts are host-side; not available in the pure layer)",
        ctx.wheel_worn,
        flight,
        ctx.selected_ability.as_deref().unwrap_or("(none)"),
        ctx.adapt_count
    )
}

/// A concept definition for the pure `registry` diagnostic.
pub struct ConceptDef {
    pub concept: &'static str,
    pub domain: &'static str,
    pub leveled: bool,
    pub max_level: u8,
}

/// Representative static snapshot of Java `AdaptationRegistry.registerBuiltins()`
/// (fist ladders and damage categories included; dynamic per-mob `Contact_*` /
/// `Offense_NPC_*` / `Drop_NPC_*` ids are minted at runtime and are not listed).
pub const BUILTIN_CONCEPTS: &[ConceptDef] = &[
    // MOVEMENT
    ConceptDef {
        concept: "Move_BerryBush",
        domain: "movement",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Move_BubbleColumn",
        domain: "movement",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Move_Honey",
        domain: "movement",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Move_PowderSnow",
        domain: "movement",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Move_SoulSand",
        domain: "movement",
        leveled: false,
        max_level: 1,
    },
    // PHYSICS
    ConceptDef {
        concept: "Env_Cobweb",
        domain: "physics",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Env_FallDamage",
        domain: "physics",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Env_Ice",
        domain: "physics",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Env_Knockback",
        domain: "physics",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Env_Slime",
        domain: "physics",
        leveled: false,
        max_level: 1,
    },
    // MINING
    ConceptDef {
        concept: "Fist_Diamond",
        domain: "mining",
        leveled: true,
        max_level: 8,
    },
    ConceptDef {
        concept: "Fist_Iron",
        domain: "mining",
        leveled: true,
        max_level: 8,
    },
    ConceptDef {
        concept: "Fist_Netherite",
        domain: "mining",
        leveled: true,
        max_level: 8,
    },
    ConceptDef {
        concept: "Fist_Stone",
        domain: "mining",
        leveled: true,
        max_level: 8,
    },
    ConceptDef {
        concept: "Fist_Wood",
        domain: "mining",
        leveled: true,
        max_level: 8,
    },
    ConceptDef {
        concept: "Mine_Labor",
        domain: "mining",
        leveled: true,
        max_level: 8,
    },
    // ENVIRONMENT
    ConceptDef {
        concept: "Env_Darkness",
        domain: "environment",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Env_Drowning",
        domain: "environment",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Env_Fire",
        domain: "environment",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Env_Inventory",
        domain: "environment",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Env_Lava",
        domain: "environment",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Env_Liquid",
        domain: "environment",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Env_Starve",
        domain: "environment",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Env_Suffocate",
        domain: "environment",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Env_Thorns",
        domain: "environment",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Env_Void",
        domain: "environment",
        leveled: false,
        max_level: 1,
    },
    // COMBAT
    ConceptDef {
        concept: "Combat_Cooldown",
        domain: "combat",
        leveled: true,
        max_level: 8,
    },
    ConceptDef {
        concept: "Combat_FistDiamond",
        domain: "combat",
        leveled: true,
        max_level: 8,
    },
    ConceptDef {
        concept: "Combat_FistIron",
        domain: "combat",
        leveled: true,
        max_level: 8,
    },
    ConceptDef {
        concept: "Combat_FistNetherite",
        domain: "combat",
        leveled: true,
        max_level: 8,
    },
    ConceptDef {
        concept: "Combat_FistStone",
        domain: "combat",
        leveled: true,
        max_level: 8,
    },
    ConceptDef {
        concept: "Combat_FistWood",
        domain: "combat",
        leveled: true,
        max_level: 8,
    },
    ConceptDef {
        concept: "Combat_ShieldLock",
        domain: "combat",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Combat_SkillIssue",
        domain: "combat",
        leveled: false,
        max_level: 1,
    },
    // PERCEPTION
    ConceptDef {
        concept: "Percep_SteadyGaze",
        domain: "perception",
        leveled: false,
        max_level: 1,
    },
    // DAMAGE (type categories; FIRE and retired types excluded per Java)
    ConceptDef {
        concept: "Type_EXPLOSION",
        domain: "damage",
        leveled: true,
        max_level: 8,
    },
    ConceptDef {
        concept: "Type_FREEZE",
        domain: "damage",
        leveled: true,
        max_level: 8,
    },
    ConceptDef {
        concept: "Type_LIGHTNING",
        domain: "damage",
        leveled: true,
        max_level: 8,
    },
    ConceptDef {
        concept: "Type_MAGIC",
        domain: "damage",
        leveled: true,
        max_level: 8,
    },
    ConceptDef {
        concept: "Type_MOB",
        domain: "damage",
        leveled: true,
        max_level: 8,
    },
    ConceptDef {
        concept: "Type_OTHER",
        domain: "damage",
        leveled: true,
        max_level: 8,
    },
    ConceptDef {
        concept: "Type_PLAYER",
        domain: "damage",
        leveled: true,
        max_level: 8,
    },
    ConceptDef {
        concept: "Type_PROJECTILE",
        domain: "damage",
        leveled: true,
        max_level: 8,
    },
    ConceptDef {
        concept: "Type_WITHER",
        domain: "damage",
        leveled: true,
        max_level: 8,
    },
    // SPECIAL
    ConceptDef {
        concept: "ADBERSITY",
        domain: "special",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Dimension_Destroy",
        domain: "special",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Mutation_Aquatic",
        domain: "special",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Mutation_Flight",
        domain: "special",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Mutation_Fist",
        domain: "special",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Mutation_Impact",
        domain: "special",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Mutation_SeaEye",
        domain: "special",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Mutation_Thermal",
        domain: "special",
        leveled: false,
        max_level: 1,
    },
    ConceptDef {
        concept: "Self_Damage",
        domain: "special",
        leveled: true,
        max_level: 8,
    },
];

fn registry_reply() -> String {
    let mut lines = vec![format!(
        "[adaptionwheel] registry: {} definitions",
        BUILTIN_CONCEPTS.len()
    )];
    for def in BUILTIN_CONCEPTS {
        let shape = if def.leveled {
            format!("Lv1-{}", def.max_level)
        } else {
            "one-time".to_string()
        };
        lines.push(format!("  - {} [{}] {}", def.concept, def.domain, shape));
    }
    lines.join("\n")
}

/// Case-insensitive concept suggestions, mirroring Java `conceptSuggestionIds`:
/// prefix matches first, then (for prefixes of length >= 2) substring matches,
/// de-duplicated in order and capped at 80. A leading quote or surrounding
/// whitespace is trimmed. The generated family+mob ids from Java's entity and
/// effect registries are not reproducible without the host; pass them in
/// `all_ids` if the lead wants them suggested.
pub fn concept_suggestions(prefix: &str, all_ids: &[&str]) -> Vec<String> {
    const CAP: usize = 80;
    let rem = prefix.trim().trim_start_matches('"').to_ascii_lowercase();
    let mut out: Vec<String> = Vec::new();

    for id in all_ids {
        if rem.is_empty() || id.to_ascii_lowercase().starts_with(&rem) {
            push_unique(&mut out, id, CAP);
        }
    }
    if rem.len() >= 2 {
        for id in all_ids {
            if id.to_ascii_lowercase().contains(&rem) {
                push_unique(&mut out, id, CAP);
            }
        }
    }
    out
}

fn push_unique(out: &mut Vec<String>, value: &str, cap: usize) {
    if out.len() < cap && !out.iter().any(|existing| existing == value) {
        out.push(value.to_string());
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn unlock_always(_: &dyn Fn(&str) -> bool) -> bool {
        true
    }
    fn unlock_never(_: &dyn Fn(&str) -> bool) -> bool {
        false
    }
    fn no_active(_: &str) -> bool {
        false
    }
    fn all_active(_: &str) -> bool {
        true
    }
    fn gate_fall(is_active: &dyn Fn(&str) -> bool) -> bool {
        is_active("Env_FallDamage")
    }
    fn gate_flight(is_active: &dyn Fn(&str) -> bool) -> bool {
        is_active("Mutation_Flight")
    }
    fn active<'a>(ids: &'a [&'a str]) -> impl Fn(&str) -> bool + 'a {
        move |concept| ids.contains(&concept)
    }

    fn two_abilities() -> AbilityDispatch {
        let mut d = AbilityDispatch::new();
        d.register("dive", gate_fall, activate_noop);
        d.register("flight", gate_flight, activate_noop);
        d
    }

    #[test]
    fn resolve_selected_prefers_persisted_when_unlocked() {
        let mut d = two_abilities();
        let is_active = active(&["Env_FallDamage", "Mutation_Flight"]);
        d.select("flight", &is_active, &mut |_| {}).unwrap();
        assert_eq!(d.selected(), Some("flight"));
        assert_eq!(d.resolve_selected(&is_active).as_deref(), Some("flight"));
    }

    #[test]
    fn resolve_selected_falls_back_when_persisted_locked() {
        let mut d = two_abilities();
        let both = active(&["Env_FallDamage", "Mutation_Flight"]);
        d.select("flight", &both, &mut |_| {}).unwrap();
        // The flight gate closed; dive is still unlocked and must take over.
        let only_fall = active(&["Env_FallDamage"]);
        assert_eq!(d.resolve_selected(&only_fall).as_deref(), Some("dive"));
    }

    #[test]
    fn resolve_selected_none_when_all_locked_and_empty_selection() {
        let d = two_abilities();
        assert_eq!(d.resolve_selected(&no_active), None);
        let mut d = d;
        d.select("dive", &active(&["Env_FallDamage"]), &mut |_| {})
            .unwrap();
        assert_eq!(d.resolve_selected(&no_active), None);
    }

    #[test]
    fn select_rejects_locked_id_without_syncing_or_persisting() {
        let mut d = two_abilities();
        let mut sync = Vec::new();
        let err = d.select("flight", &active(&["Env_FallDamage"]), &mut |s| {
            sync.push(s)
        });
        assert_eq!(err.unwrap_err(), "ability 'flight' is locked");
        assert!(sync.is_empty());
        assert_eq!(d.selected(), None);
        let err = d.select("unknown", &all_active, &mut |s| sync.push(s));
        assert!(err.is_err());
    }

    #[test]
    fn select_syncs_immediately_on_success() {
        let mut d = two_abilities();
        let mut sync = Vec::new();
        assert_eq!(
            d.select("dive", &active(&["Env_FallDamage"]), &mut |s| sync.push(s))
                .unwrap(),
            "dive"
        );
        assert_eq!(sync, vec!["dive".to_string()]);
    }

    static ACTIVATIONS: AtomicUsize = AtomicUsize::new(0);
    fn counted_activate() {
        ACTIVATIONS.fetch_add(1, Ordering::SeqCst);
    }

    #[test]
    fn activate_requires_wheel_and_resolves_unlocked() {
        ACTIVATIONS.store(0, Ordering::SeqCst);
        let mut d = AbilityDispatch::new();
        d.register("dive", gate_fall, counted_activate);
        let is_active = active(&["Env_FallDamage"]);
        let mut sync = Vec::new();

        assert_eq!(
            d.activate(false, &is_active, &mut |s| sync.push(s))
                .unwrap_err(),
            "wheel not worn"
        );
        assert_eq!(ACTIVATIONS.load(Ordering::SeqCst), 0);

        assert_eq!(
            d.activate(true, &is_active, &mut |s| sync.push(s)).unwrap(),
            "dive"
        );
        assert_eq!(ACTIVATIONS.load(Ordering::SeqCst), 1);
        assert_eq!(sync, vec!["dive".to_string()]);

        // Gate closes -> no fallback registered -> refuses.
        assert_eq!(
            d.activate(true, &no_active, &mut |s| sync.push(s))
                .unwrap_err(),
            "no unlocked ability available"
        );
        assert_eq!(ACTIVATIONS.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn builtins_match_upstream_dive_gate() {
        let d = AbilityDispatch::with_builtins();
        assert_eq!(
            d.ids(),
            BUILTIN_ABILITY_IDS
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
        let fall = active(&["Env_FallDamage"]);
        let flight = active(&["Mutation_Flight"]);
        let fist = active(&["Mutation_Fist"]);
        assert_eq!(d.resolve_selected(&fall).as_deref(), Some(ABILITY_DIVE));
        assert_eq!(d.resolve_selected(&flight).as_deref(), Some(ABILITY_DIVE));
        assert_eq!(d.resolve_selected(&fist).as_deref(), Some(ABILITY_MUTATION));
        assert_eq!(d.resolve_selected(&no_active), None);
    }

    #[test]
    fn register_replaces_duplicate_and_ignores_blank() {
        let mut d = AbilityDispatch::new();
        d.register("dive", unlock_never, activate_noop);
        d.register("flight", unlock_always, activate_noop);
        d.register("dive", unlock_always, activate_noop);
        d.register("   ", unlock_always, activate_noop);
        assert_eq!(d.ids(), vec!["dive".to_string(), "flight".to_string()]);
        assert_eq!(d.resolve_selected(&no_active).as_deref(), Some("dive"));
    }

    fn sample_ctx() -> DiagnosticCtx {
        let mut ctx = DiagnosticCtx {
            adapt_count: 3,
            active_ids: vec![
                "Env_FallDamage".to_string(),
                "Mutation_Flight".to_string(),
                "Env_Fire".to_string(),
            ],
            level_map: BTreeMap::from([
                ("Env_FallDamage".to_string(), 1),
                ("Mutation_Flight".to_string(), 8),
            ]),
            selected_ability: Some(ABILITY_DIVE.to_string()),
            wheel_worn: true,
        };
        // Keep a genuine leveled entry in the map for list/info output.
        ctx.level_map.insert("Type_EXPLOSION".to_string(), 2);
        ctx.level_map.insert("Self_Damage".to_string(), 8);
        ctx.adapt_count = 5;
        ctx
    }

    fn reply(line: &str, ctx: &DiagnosticCtx) -> String {
        match handle_diagnostic(line, ctx) {
            Some(Reply(text)) => text,
            other => panic!("expected Reply for {:?}, got {:?}", line, other),
        }
    }

    #[test]
    fn diagnostic_pure_subcommands_reply() {
        let ctx = sample_ctx();
        let list = reply("list", &ctx);
        assert!(list.starts_with("[adaptionwheel] list:"));
        assert!(list.contains("Env_Fire [environment] ADAPTED"));
        assert!(list.contains("Type_EXPLOSION [damage] Lv.2"));
        assert!(list.contains("Mutation_Flight [special] ADAPTED"));
        assert!(list.contains("Self_Damage [special] MAX"));

        let filtered = reply("/aw list physics", &ctx);
        assert!(filtered.contains("Env_FallDamage [physics]"));
        assert!(!filtered.contains("Env_Fire"));

        let info = reply("info Mutation_Flight", &ctx);
        assert!(info.contains("Mutation_Flight [special] one-time"));
        assert!(info.contains("adapted=true"));
        let missing = reply("info", &ctx);
        assert!(missing.contains("usage:"));

        assert!(reply("wheel", &ctx).contains("worn=true"));
        assert!(reply("wheel", &ctx).contains("ability list to client=dive"));
        let flight = reply("flight", &ctx);
        assert!(flight.contains("Mutation_Flight active: true"));
        assert!(flight.contains("host-side"));

        let registry = reply("registry", &ctx);
        assert!(registry.starts_with("[adaptionwheel] registry:"));
        assert!(registry.contains("Env_FallDamage [physics] one-time"));
        assert!(registry.contains("Self_Damage [special] Lv1-8"));
    }

    #[test]
    fn diagnostic_mutations_delegate_to_host() {
        let ctx = sample_ctx();
        assert_eq!(
            handle_diagnostic("grantall environment", &ctx),
            Some(Mutate {
                kind: MutationKind::GrantAll,
                concept: Some("environment".to_string())
            })
        );
        assert_eq!(
            handle_diagnostic("shed Env_Fire", &ctx),
            Some(Mutate {
                kind: MutationKind::Shed,
                concept: Some("Env_Fire".to_string())
            })
        );
        assert_eq!(
            handle_diagnostic("ungrant \"Type_EXPLOSION\"", &ctx),
            Some(Mutate {
                kind: MutationKind::Ungrant,
                concept: Some("Type_EXPLOSION".to_string())
            })
        );
        assert_eq!(
            handle_diagnostic("analyze Mutation_Flight", &ctx),
            Some(Mutate {
                kind: MutationKind::Analyze,
                concept: Some("Mutation_Flight".to_string())
            })
        );
        assert_eq!(
            handle_diagnostic("shed", &ctx),
            Some(Reply(
                "[adaptionwheel] usage: /adaptionwheel shed <concept>".to_string()
            ))
        );
        assert_eq!(handle_diagnostic("somethingelse", &ctx), None);
        assert_eq!(handle_diagnostic("  ", &ctx), None);
    }

    #[test]
    fn empty_context_does_not_panic() {
        let ctx = DiagnosticCtx::default();
        assert!(reply("list", &ctx).contains("(none)"));
        assert!(reply("info Env_Fire", &ctx).contains("adapted=false"));
        assert!(reply("wheel", &ctx).contains("worn=false"));
        assert!(reply("flight", &ctx).contains("Mutation_Flight active: false"));
    }

    #[test]
    fn suggestions_prefix_substring_trim_and_cap() {
        let ids = [
            "Env_FallDamage",
            "Env_Fire",
            "Env_Frost",
            "Mutation_Flight",
            "Move_SoulSand",
        ];
        assert_eq!(
            concept_suggestions("ENV_", &ids),
            vec!["Env_FallDamage", "Env_Fire", "Env_Frost"]
        );
        assert_eq!(
            concept_suggestions("\"flight", &ids),
            vec!["Mutation_Flight"]
        );
        // Single-character prefix does not run the substring pass.
        assert_eq!(concept_suggestions("e", &ids).len(), 3);
        assert!(concept_suggestions("", &ids).len() == ids.len());

        let many: Vec<String> = (0..200).map(|i| format!("Env_{}", i)).collect();
        let refs: Vec<&str> = many.iter().map(String::as_str).collect();
        let capped = concept_suggestions("env", &refs);
        assert_eq!(capped.len(), 80);
        assert_eq!(capped[0], "Env_0");
        assert_eq!(capped[79], "Env_79");
    }

    #[test]
    fn domain_classification_matches_registry_prefix_rules() {
        assert_eq!(domain_of("Env_FallDamage"), "physics");
        assert_eq!(domain_of("env_fall"), "physics");
        assert_eq!(domain_of("Env_Fire"), "environment");
        assert_eq!(domain_of("Type_EXPLOSION"), "damage");
        assert_eq!(domain_of("Fist_Wood"), "mining");
        assert_eq!(domain_of("Offense_NPC_minecraft:zombie"), "entity");
        assert_eq!(domain_of("Existence_Wither"), "existence");
        assert_eq!(domain_of("Mutation_Flight"), "special");
        assert_eq!(DOMAIN_KEYS.len(), 11);
    }

    #[test]
    fn active_predicate_from_levels_union_view() {
        let levels = BTreeMap::from([
            ("Type_EXPLOSION".to_string(), 2u8),
            ("Zero_Level".to_string(), 0u8),
        ]);
        let adapted = BTreeSet::from(["Env_FallDamage".to_string(), "Mutation_Flight".to_string()]);
        let is_active = active_predicate_from_levels(&levels, &adapted);
        assert!(ability_is_active(&is_active, "Env_FallDamage"));
        assert!(ability_is_active(&is_active, "Mutation_Flight"));
        assert!(ability_is_active(&is_active, "Type_EXPLOSION"));
        assert!(!ability_is_active(&is_active, "Zero_Level"));
        assert!(!ability_is_active(&is_active, "Unknown_Concept"));
    }

    #[test]
    fn builtin_dispatch_selection_and_immediate_sync_message() {
        let mut dispatch = builtin_dispatch();
        assert_eq!(
            dispatch.ids(),
            BUILTIN_ABILITY_IDS
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
        let levels = BTreeMap::new();
        let adapted = BTreeSet::from(["Env_FallDamage".to_string()]);
        let is_active = active_predicate_from_levels(&levels, &adapted);
        let mut synced = Vec::new();
        assert_eq!(
            dispatch
                .select(ABILITY_DIVE, &is_active, &mut |id| synced.push(id))
                .unwrap(),
            ABILITY_DIVE
        );
        assert_eq!(synced, vec![ABILITY_DIVE.to_string()]);
        assert_eq!(
            immediate_sync_message(&synced[0]),
            "[adaptionwheel] ability synced: dive"
        );
        assert_eq!(
            dispatch.resolve_selected(&is_active).as_deref(),
            Some(ABILITY_DIVE)
        );
    }

    #[test]
    fn activate_for_hook_matches_activate_semantics() {
        let mut dispatch = builtin_dispatch();
        let levels = BTreeMap::new();
        let adapted = BTreeSet::from(["Mutation_Flight".to_string()]);
        let is_active = active_predicate_from_levels(&levels, &adapted);
        let mut synced = Vec::new();

        assert_eq!(
            activate_for_hook(&mut dispatch, false, &is_active, &mut |id| synced.push(id))
                .unwrap_err(),
            "wheel not worn"
        );
        assert!(synced.is_empty());

        assert_eq!(
            activate_for_hook(&mut dispatch, true, &is_active, &mut |id| synced.push(id)).unwrap(),
            ABILITY_DIVE
        );
        assert_eq!(synced, vec![ABILITY_DIVE.to_string()]);

        let empty_levels = BTreeMap::new();
        let empty_adapted = BTreeSet::new();
        let none_active = active_predicate_from_levels(&empty_levels, &empty_adapted);
        assert!(activate_for_hook(&mut dispatch, true, &none_active, &mut |_| {}).is_err());
    }

    #[test]
    fn activation_key_help_documents_command_only_limitation() {
        let help = activation_key_help();
        let lower = help.to_ascii_lowercase();
        assert!(help.contains("8bc19c5f97"));
        assert!(lower.contains("no universal activation key"));
        assert!(help.contains("ONLY"));
        assert!(help.contains("/adaptionwheel wheel"));
        assert!(help.contains("ability <id>"));
        assert!(lower.contains("no key hook"));
    }

    #[test]
    fn diagnostic_ability_and_activate_subcommands() {
        let ctx = sample_ctx();
        assert_eq!(
            handle_diagnostic("ability dive", &ctx),
            Some(Mutate {
                kind: MutationKind::SelectAbility,
                concept: Some("dive".to_string())
            })
        );
        assert_eq!(
            handle_diagnostic("/aw activate", &ctx),
            Some(Mutate {
                kind: MutationKind::ActivateAbility,
                concept: None
            })
        );
        assert_eq!(
            handle_diagnostic("ability", &ctx),
            Some(Reply(
                "[adaptionwheel] usage: /adaptionwheel ability <id>".to_string()
            ))
        );
        // The original nine are preserved; the two activation ones are additive.
        assert_eq!(SUPPORTED_SUBCOMMANDS.len(), 11);
        for sub in [
            "list", "info", "grantall", "shed", "ungrant", "analyze", "wheel", "flight", "registry",
        ] {
            assert!(SUPPORTED_SUBCOMMANDS.contains(&sub));
        }
    }
}
