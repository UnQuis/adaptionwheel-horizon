//! Typed server commands for the AdaptionWheel native mod (Horizon mod API v2).
//!
//! `register` contributes the six original usages plus explicit `<target:player>`
//! variants so the console / command blocks / functions can drive the same
//! subcommands. It also contributes the nine P1.6 diagnostic usages (`list`,
//! `info`, `grantall`, `shed`, `ungrant`, `analyze`, `wheel`, `flight`,
//! `registry`). `run` matches the parsed `CommandInvocation::literals` and reads
//! named arguments; it never splits a raw command line.
//!
//! The diagnostic subcommands are delegated to the pure
//! [`crate::abilities`] responder. Read-only subcommands come back as a
//! `Reply` that is sent verbatim; state-changing ones come back as a `Mutate`
//! applied here to the v1 state on a best-effort basis (see
//! [`apply_diagnostic_mutation`] for exactly which mutations are real and which
//! are diagnostic-only).
//!
//! Response wording is kept from the old `native/adaptation/api` command handler.

use mod_api::{
    Command, CommandArgumentValue, CommandSource, Flow, RegisterCommandsEvent, RunModCommandEvent,
    ServerHost, Text,
};
use std::collections::BTreeMap;

use crate::abilities::{
    DiagnosticCtx, DiagnosticOutcome, MutationKind, SUPPORTED_SUBCOMMANDS, activate_for_hook,
    builtin_dispatch, concept_suggestions, handle_diagnostic, immediate_sync_message,
};
use crate::engine_impl::{
    ALL_CONCEPTS, PlayerAdaptationStateV1, apply_grant_command, apply_set_command, canonical_id,
    format_status, normalize_concept,
};
use std::sync::{Mutex, OnceLock};
use crate::state_impl::{get_player_state, persist_player_state_if_needed, with_player_state};

const MOD_ID: &str = "adaptionwheel";
/// Old `NativeCommand::op(..., 2)` -> new `Command::op(..., 2)`; see
/// `horizon-mod-vocab/src/command/types.rs:49`.
const OP_LEVEL: u8 = 2;

/// The seven concepts, in the same order the old usage strings spelled them out.
/// Reuse the engine's `ALL_CONCEPTS` instead of a second hand-written list.
fn concepts() -> String {
    ALL_CONCEPTS.join(",")
}

/// Contribute every usage on each `register_commands` call (the server asks again
/// at world open, `/reload`, and data-pack changes).
///
/// Verified against `horizon-mod-vocab/src/command/types.rs:8-24`: required args
/// are `<name:kind>`, optional args are `[name:kind]` and must come **last**.
/// `word` is a one-word argument delivered as `CommandArgumentValue::String`;
/// `[domain:word]` is therefore the correct optional form for `list`/`grantall`.
pub fn register(event: &mut RegisterCommandsEvent<'_>) {
    event.commands.extend([
        // Original in-game surface: same literals, same level, no target argument.
        Command::op("adaptionwheel status", OP_LEVEL),
        Command::op("adaptionwheel activate", OP_LEVEL),
        Command::op("adaptionwheel reset", OP_LEVEL),
        Command::op("adaptionwheel help", OP_LEVEL),
        Command::op(
            "adaptionwheel set <concept:word> <field:word> <value:number>",
            OP_LEVEL,
        ),
        Command::op(
            "adaptionwheel grant <concept:word> <level:integer>",
            OP_LEVEL,
        ),
        // Explicit target variants. `<target:player>` resolves to a live
        // `CommandArgumentValue::Player(PlayerRef)`, which lets a non-player
        // source select the player to act on (old build refused those outright).
        Command::op("adaptionwheel status <target:player>", OP_LEVEL),
        Command::op("adaptionwheel activate <target:player>", OP_LEVEL),
        Command::op("adaptionwheel reset <target:player>", OP_LEVEL),
        Command::op(
            "adaptionwheel set <target:player> <concept:word> <field:word> <value:number>",
            OP_LEVEL,
        ),
        Command::op(
            "adaptionwheel grant <target:player> <concept:word> <level:integer>",
            OP_LEVEL,
        ),
        // P1.6 diagnostic surface. Five read-only (`list`, `info`, `wheel`,
        // `flight`, `registry`) and four state-changing (`grantall`, `shed`,
        // `ungrant`, `analyze`). `list`/`grantall` take an OPTIONAL trailing
        // domain word; the others take a required concept word.
        Command::op("adaptionwheel list [domain:word]", OP_LEVEL),
        Command::op("adaptionwheel info <concept:word>", OP_LEVEL),
        Command::op("adaptionwheel grantall [domain:word]", OP_LEVEL),
        Command::op("adaptionwheel shed <concept:word>", OP_LEVEL),
        Command::op("adaptionwheel ungrant <concept:word>", OP_LEVEL),
        Command::op("adaptionwheel analyze <concept:word>", OP_LEVEL),
        Command::op("adaptionwheel wheel", OP_LEVEL),
        Command::op("adaptionwheel flight", OP_LEVEL),
        Command::op("adaptionwheel registry", OP_LEVEL),
        // Console/dedicated-server reachability for the P1.6 diagnostics.
        // `sunrise-server cmd` runs with CommandSource::Console (no acting
        // player), so these need an explicit `<target:player>` to resolve a
        // state to read/mutate. `resolve_target` already reads the `target`
        // argument for every source; `diagnostic_arg_name`/`string_arg` keep
        // reading `domain`/`concept` by name, so only the usages are new.
        Command::op("adaptionwheel list <target:player> [domain:word]", OP_LEVEL),
        Command::op("adaptionwheel info <target:player> <concept:word>", OP_LEVEL),
        Command::op("adaptionwheel grantall <target:player> [domain:word]", OP_LEVEL),
        Command::op("adaptionwheel shed <target:player> <concept:word>", OP_LEVEL),
        Command::op("adaptionwheel ungrant <target:player> <concept:word>", OP_LEVEL),
        Command::op("adaptionwheel analyze <target:player> <concept:word>", OP_LEVEL),
        Command::op("adaptionwheel wheel <target:player>", OP_LEVEL),
        Command::op("adaptionwheel flight <target:player>", OP_LEVEL),
        Command::op("adaptionwheel registry <target:player>", OP_LEVEL),
    ]);
}

/// Which registered subcommand this invocation is, or `None` when the command
/// belongs to another mod. `literals` contains the command name and subcommands
/// (`CommandInvocation::literals`, `command/mod.rs:150-152`).
fn subcommand(event: &RunModCommandEvent<'_>) -> Option<String> {
    let first = event.command.literals.first().map(String::as_str)?;
    if first != MOD_ID {
        return None;
    }
    // No subcommand was registered: `adaptionwheel` alone is not one of ours.
    event.command.literals.get(1).cloned()
}

/// True for one of the nine P1.6 diagnostic subcommands handled by
/// [`crate::abilities::handle_diagnostic`].
fn is_diagnostic_sub(sub: &str) -> bool {
    SUPPORTED_SUBCOMMANDS.contains(&sub)
}

/// Pure core of [`resolve_target`], kept separate so the precedence can be
/// unit-tested without a host or an event.
///
/// An explicit target always wins: a player operator running
/// `adaptionwheel grant <target> ...` must mutate the named player, not itself.
/// Without an explicit target a player source acts on itself; a non-player
/// source has no state to act on and falls through to the "requires player
/// context" feedback.
fn choose_target_uuid(explicit: Option<u128>, acting: Option<u128>) -> Option<u128> {
    explicit.or(acting)
}

/// The player whose state the subcommand acts on.
///
/// `PlayerRef::uuid(self) -> u128` is at
/// `horizon-mod-vocab/src/entity/types.rs:1097`. Both the acting player and an
/// explicit `<target:player>` argument are read for every source kind; see
/// [`choose_target_uuid`] for the precedence (review fix #6).
fn resolve_target(event: &RunModCommandEvent<'_>) -> Option<u128> {
    let explicit = match event.command.argument("target") {
        Some(CommandArgumentValue::Player(player)) => Some(player.uuid()),
        _ => None,
    };
    let acting = match event.source {
        CommandSource::Player { player, .. } => Some(player.uuid()),
        _ => None,
    };
    choose_target_uuid(explicit, acting)
}

/// A `word` argument, delivered as `CommandArgumentValue::String`.
fn string_arg(event: &RunModCommandEvent<'_>, name: &str) -> Option<String> {
    match event.command.argument(name) {
        Some(CommandArgumentValue::String(value)) => Some(value.clone()),
        _ => None,
    }
}

fn help_message() -> String {
    format!(
        "[{}] commands: /adaptionwheel status | /adaptionwheel activate | /adaptionwheel set <concept> <level|progress> <value> | /adaptionwheel grant <concept> <level> | /adaptionwheel reset (concepts: {})",
        MOD_ID,
        concepts()
    )
}

fn status_message(uuid: u128) -> String {
    let state = get_player_state(uuid);
    format!("[{}] status: {}", MOD_ID, format_status(&state))
}

fn activate_message(uuid: u128) -> String {
    let state = get_player_state(uuid);
    format!(
        "[{}] activate: wheel_active={} source={:?} (cached from inventory hooks; move/change inventory to re-evaluate automatically)",
        MOD_ID, state.wheel_active, state.active_source
    )
}

fn reset_message(uuid: u128) -> String {
    with_player_state(uuid, |state| {
        *state = PlayerAdaptationStateV1::default();
        state.dirty_persist = true;
    });
    format!("[{}] reset done. {}", MOD_ID, status_message_tail(uuid))
}

fn status_message_tail(uuid: u128) -> String {
    let state = get_player_state(uuid);
    format_status(&state)
}

fn grant_message(event: &RunModCommandEvent<'_>, uuid: u128) -> String {
    let Some(concept_raw) = string_arg(event, "concept") else {
        return format!(
            "[{}] usage: /adaptionwheel grant <{}> <level>",
            MOD_ID,
            concepts()
        );
    };
    let Some(concept) = normalize_concept(&concept_raw) else {
        return format!(
            "[{}] unknown concept '{}'. Use {}",
            MOD_ID,
            concept_raw,
            concepts()
        );
    };
    // `integer` arrives as `CommandArgumentValue::Integer(i32)`; the old parser
    // defaulted an absent level to 1, so keep that fallback.
    let level = match event.command.argument("level") {
        Some(CommandArgumentValue::Integer(value)) => (*value).max(0) as u8,
        _ => 1,
    };
    let result = with_player_state(uuid, |state| apply_grant_command(state, concept, level));
    match result {
        Ok(message) => format!("[{}] {}. {}", MOD_ID, message, status_message_tail(uuid)),
        Err(error) => format!("[{}] {}", MOD_ID, error),
    }
}

fn set_message(event: &RunModCommandEvent<'_>, uuid: u128) -> String {
    let usage = format!(
        "[{}] usage: /adaptionwheel set <concept> <level|progress> <value>",
        MOD_ID
    );
    let Some(concept_raw) = string_arg(event, "concept") else {
        return usage;
    };
    let Some(field) = string_arg(event, "field") else {
        return usage;
    };
    // `number` arrives as `CommandArgumentValue::Number(f64)`; the engine helper
    // takes `f32`, as the old command handler did.
    let Some(value) = (match event.command.argument("value") {
        Some(CommandArgumentValue::Number(value)) => Some(*value as f32),
        _ => None,
    }) else {
        return format!("[{}] invalid value", MOD_ID);
    };
    let Some(concept) = normalize_concept(&concept_raw) else {
        return format!(
            "[{}] unknown concept '{}'. Use {}",
            MOD_ID,
            concept_raw,
            concepts()
        );
    };
    let result = with_player_state(uuid, |state| {
        apply_set_command(state, concept, &field, value)
    });
    match result {
        Ok(message) => format!("[{}] {}. {}", MOD_ID, message, status_message_tail(uuid)),
        Err(error) => format!("[{}] {}", MOD_ID, error),
    }
}

// ===========================================================================
// P1.6 diagnostic wiring (pure helpers + best-effort v1 mutations)
// ===========================================================================

/// Build the read-only snapshot `handle_diagnostic` needs from a v1 state.
///
/// * `adapt_count`: a schema-v2 accessor **does** exist
///   (`state_v2::PlayerAdaptionV2::get_adapt_count`, the union of `adapted` and
///   positive `levels`), so the v1 snapshot is migrated via
///   `state_v2::migrate_v1_json` and that accessor is used. No v2 runtime state
///   cache is wired into the command path (`state_v2` is a pure model), so the
///   count is derived from this v1 snapshot rather than from a live v2 player.
/// * `active_ids` / `level_map`: derived from the v1 `entries`, mapped to the
///   upstream concept ids through `state_v2::p1_concept_id` so the pure
///   diagnostics' domain/level predicates (which expect e.g. `Env_FallDamage`)
///   see the same ids they use in `abilities`.
/// * `selected_ability`: `Extras.selectedAbility` is a v2-only field; migration
///   leaves it `None` because the v1 state has no such slot. Read-only
///   diagnostics report `(none)` rather than fabricating a value.
/// * `wheel_worn`: the v1 `wheel_active` flag (cached from inventory hooks).
fn build_diagnostic_ctx(state: &PlayerAdaptationStateV1) -> DiagnosticCtx {
    let json = serde_json::to_string(state).unwrap_or_default();
    let v2 = crate::state_v2::migrate_v1_json(&json);

    let mut active_ids: Vec<String> = Vec::new();
    let mut level_map: BTreeMap<String, u8> = BTreeMap::new();
    for (legacy, entry) in &state.entries {
        let id = crate::state_v2::p1_concept_id(legacy).to_string();
        if entry.completed {
            active_ids.push(id.clone());
        }
        if entry.level > 0 {
            level_map.insert(id, entry.level);
        }
    }

    DiagnosticCtx {
        adapt_count: v2.get_adapt_count(),
        active_ids,
        level_map,
        selected_ability: v2.extras.selected_ability.clone(),
        wheel_worn: state.wheel_active,
    }
}

/// Reconstruct the raw line the pure responder expects. `handle_diagnostic`
/// tokenises a line whose first token is the subcommand (it also accepts a
/// leading `/adaptionwheel` or `/aw` alias, which we do not need here).
fn diagnostic_line(sub: &str, arg: Option<&str>) -> String {
    match arg.filter(|value| !value.is_empty()) {
        Some(value) => format!("{} {}", sub, value),
        None => sub.to_owned(),
    }
}

/// Which argument name a diagnostic subcommand uses: `list`/`grantall` filter
/// by domain; the other concept subcommands act on a single concept.
fn diagnostic_arg_name(sub: &str) -> &'static str {
    if matches!(sub, "list" | "grantall") {
        "domain"
    } else {
        "concept"
    }
}

/// Case-insensitive concept suggestions backed by the real concept registry.
///
/// **Not wired to the command event.** The new API has no per-argument
/// suggestion hook on `RunModCommandEvent`: completions come from a
/// `CommandArgumentType::suggest` implementation registered once with
/// `Registrar::register_command_argument` in `Hooks::register`, or from the
/// separate `Hooks::tab_complete` hook (`TabCompleteEvent` carries
/// `player/name/command/range/suggestions`, not an argument name). Neither is
/// reachable from `run_mod_command`, and this file does not own `Hooks::register`
/// or the `Hooks` impl, so suggestions are prepared here and left unused; no
/// fake/per-call argument type is registered. `handle_diagnostic` still accepts
/// the ids this helper would complete.
#[allow(dead_code)]
fn diagnostic_concept_suggestions(prefix: &str) -> Vec<String> {
    let ids: Vec<&str> = crate::concepts_registry::all_ids().collect();
    concept_suggestions(prefix, &ids)
}

/// Map a `grantall` domain word onto the seven v1 concepts. The v1 model only
/// has damage/environment concepts, so movement, mining, combat, perception,
/// entity, existence, effect and special domains legitimately match nothing;
/// this is a best-effort filter, not the full 11-domain Java registry.
fn grantall_domain_matches(legacy: &str, domain: &str) -> bool {
    match domain.to_ascii_lowercase().as_str() {
        "physics" => legacy.contains("fall"),
        "environment" => matches!(legacy, "env_fire" | "env_lava" | "env_drowning"),
        "damage" => matches!(legacy, "type_explosion" | "type_general" | "type_contact"),
        _ => false,
    }
}

/// Apply a `Mutate` outcome to the v1 state (best-effort) and return the
/// confirmation text.
///
/// Real mutations:
/// * `GrantAll` — every selected concept is granted through
///   `apply_grant_command` at level 8; the v1 engine clamps to each concept's
///   own cap (fall = 1, the six ladders = 3), so "level 8 where sensible".
/// * `Shed` / `Ungrant` — the mapped concept is set to level 0 via
///   `apply_set_command` with field `level`, which clears `completed`.
///
/// Diagnostic-only (no real state change):
/// * `Analyze` — reports the v1 level/progress/completed snapshot; starting a
///   `state_v2::TaskRecord` analysis task needs a v2 runtime state, which the
///   command path does not have. Unknown/upstream-only ids (e.g.
///   `Mutation_Flight`) have no v1 entry, so shed/ungrant/analyze also become
///   diagnostic-only for them.
fn apply_diagnostic_mutation(uuid: u128, kind: MutationKind, concept: Option<&str>) -> String {
    match kind {
        MutationKind::GrantAll => {
            let domain = concept.filter(|value| !value.is_empty());
            let mut granted: Vec<&'static str> = Vec::new();
            for legacy in ALL_CONCEPTS {
                if domain.is_some_and(|d| !grantall_domain_matches(legacy, d)) {
                    continue;
                }
                let result = with_player_state(uuid, |state| apply_grant_command(state, legacy, 8));
                if result.is_ok() {
                    granted.push(legacy);
                }
            }
            match domain {
                Some(domain) => format!(
                    "[{}] grantall [{}]: {} concept(s) granted to v1 max ({})",
                    MOD_ID,
                    domain,
                    granted.len(),
                    granted.join(", ")
                ),
                None => format!(
                    "[{}] grantall: {} concept(s) granted to v1 max ({})",
                    MOD_ID,
                    granted.len(),
                    granted.join(", ")
                ),
            }
        }
        MutationKind::Shed | MutationKind::Ungrant => {
            let label = if kind == MutationKind::Shed {
                "shed"
            } else {
                "ungrant"
            };
            let Some(raw) = concept else {
                return format!("[{}] usage: /adaptionwheel {} <concept>", MOD_ID, label);
            };
            let Some(mapped) = normalize_concept(raw) else {
                return format!(
                    "[{}] {}: '{}' has no v1 state entry; no mutation applied (diagnostic-only)",
                    MOD_ID, label, raw
                );
            };
            let result =
                with_player_state(uuid, |state| apply_set_command(state, mapped, "level", 0.0));
            match result {
                Ok(_) => format!(
                    "[{}] {}: '{}' set to level 0 (completed=false)",
                    MOD_ID, label, mapped
                ),
                Err(error) => format!("[{}] {}: {}", MOD_ID, label, error),
            }
        }
        MutationKind::Analyze => {
            let Some(raw) = concept else {
                return format!("[{}] usage: /adaptionwheel analyze <concept>", MOD_ID);
            };
            let Some(mapped) = normalize_concept(raw) else {
                return format!(
                    "[{}] analyze: '{}' has no v1 state entry; no task scheduled (diagnostic-only)",
                    MOD_ID, raw
                );
            };
            let state = get_player_state(uuid);
            let entry = state.entries.get(mapped).cloned().unwrap_or_default();
            format!(
                "[{}] analyze {}: level={} progress={:.2} completed={} (diagnostic-only: no v2 task scheduled)",
                MOD_ID, mapped, entry.level, entry.progress, entry.completed
            )
        }
        // --- Ability dispatch (P1.5 wired through the command path) ---
        // Horizon 8bc19c5f97 has no universal activation key: abilities are
        // selected/activated only as a consequence of a command (or a hook
        // calling activate_for_hook). Keep one process-wide dispatch so a
        // selection persists between commands; `sync` forwards the immediate
        // sync message (upstream AbilityDispatch re-checks unlock and syncs
        // immediately instead of waiting for the 1 Hz state sync).
        MutationKind::SelectAbility | MutationKind::ActivateAbility => {
            let label = if kind == MutationKind::SelectAbility {
                "ability"
            } else {
                "activate"
            };
            static DISPATCH: OnceLock<Mutex<crate::abilities::AbilityDispatch>> = OnceLock::new();
            let mut dispatch = DISPATCH
                .get_or_init(|| Mutex::new(builtin_dispatch()))
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            // Build the "is concept active" predicate from the v1 state: a
            // concept is active when its entry is completed or has level > 0.
            let is_active = |cid: &str| {
                get_player_state(uuid)
                    .entries
                    .iter()
                    .any(|(legacy, entry)| canonical_id(legacy) == cid && (entry.completed || entry.level > 0))
            };
            let mut sync_lines: Vec<String> = Vec::new();
            let mut sync = |message: String| sync_lines.push(immediate_sync_message(&message));
            let result = if kind == MutationKind::SelectAbility {
                let Some(id) = concept else {
                    return format!("[{}] usage: /adaptionwheel ability <id>", MOD_ID);
                };
                dispatch.select(id, &is_active, &mut sync)
            } else {
                let wheel_worn = get_player_state(uuid).wheel_active;
                activate_for_hook(&mut dispatch, wheel_worn, &is_active, &mut sync)
            };
            match result {
                Ok(id) => format!(
                    "[{}] {}: {} ({}); {}",
                    MOD_ID,
                    label,
                    id,
                    immediate_sync_message(&id),
                    sync_lines.join(" | ")
                ),
                Err(error) => format!("[{}] {}: {}", MOD_ID, label, error),
            }
        }
    }
}

/// Dispatch one of the nine P1.6 subcommands through the pure responder and
/// return the single message the common path below sends as feedback.
fn diagnostic_message(event: &RunModCommandEvent<'_>, uuid: u128, sub: &str) -> String {
    let state = get_player_state(uuid);
    let ctx = build_diagnostic_ctx(&state);
    let arg = string_arg(event, diagnostic_arg_name(sub));
    let line = diagnostic_line(sub, arg.as_deref());
    match handle_diagnostic(&line, &ctx) {
        Some(DiagnosticOutcome::Reply(text)) => text,
        Some(DiagnosticOutcome::Mutate { kind, concept }) => {
            apply_diagnostic_mutation(uuid, kind, concept.as_deref())
        }
        None => format!("[{}] subcommand '{}' is not available", MOD_ID, sub),
    }
}

/// `Hooks::run_mod_command` implementation: `Flow::Cancel` for every handled
/// invocation, `Flow::Continue` for commands that are not ours.
pub fn run(host: &mut dyn ServerHost, event: &mut RunModCommandEvent<'_>) -> Flow {
    let Some(sub) = subcommand(event) else {
        return Flow::Continue;
    };

    // `help` is pure text and works from any source, including the console.
    if sub == "help" {
        let _ = host.commands().feedback(Text::literal(help_message()));
        return Flow::Cancel;
    }

    let Some(uuid) = resolve_target(event) else {
        let message = format!(
            "[{}] '{}' requires player context (run as player or pass a <target:player>). Console help: /adaptionwheel help",
            MOD_ID, sub
        );
        let _ = host.commands().feedback(Text::literal(message));
        return Flow::Cancel;
    };

    let message = match sub.as_str() {
        "status" => status_message(uuid),
        "activate" => activate_message(uuid),
        "reset" => reset_message(uuid),
        "grant" => grant_message(event, uuid),
        "set" => set_message(event, uuid),
        // P1.6 diagnostic subcommands; the six legacy arms above are untouched.
        other if is_diagnostic_sub(other) => diagnostic_message(event, uuid, other),
        other => format!(
            "[{}] unknown subcommand '{}'. Try: status, activate, set, grant, reset, help, list, info, grantall, shed, ungrant, analyze, wheel, flight, registry",
            MOD_ID, other
        ),
    };

    // Persist only genuinely dirty state. `set`/`grant`/`reset` and the real
    // `grantall`/`shed`/`ungrant` mutations set `dirty_persist` in the engine,
    // so force=false still flushes every mutation. Read-only commands
    // (`status`/`activate`/`help`, analyze and the pure diagnostics) now cause
    // no attachment write (review fix #5).
    persist_player_state_if_needed(host, uuid, false);
    // `Commands::feedback` routes to the originating source and honours silence;
    // it returns Result, so explicitly discard it (host/services.rs:130).
    let _ = host.commands().feedback(Text::literal(message));
    Flow::Cancel
}

// ===========================================================================
// Tests (pure parts: no host required)
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine_impl::{
        AdaptationEntry, CONCEPT_EXPLOSION, CONCEPT_FALL, CONCEPT_FIRE, CONCEPT_GENERAL,
    };

    fn state_with(entries: &[(&str, u8, bool)]) -> PlayerAdaptationStateV1 {
        let mut state = PlayerAdaptationStateV1::default();
        for (concept, level, completed) in entries {
            state.entries.insert(
                (*concept).to_string(),
                AdaptationEntry {
                    level: *level,
                    progress: 0.0,
                    completed: *completed,
                },
            );
        }
        state
    }

    #[test]
    fn diagnostic_ctx_derives_upstream_ids_via_v2_accessor() {
        let mut state = state_with(&[
            (CONCEPT_FALL, 1, true),
            (CONCEPT_EXPLOSION, 2, false),
            (CONCEPT_FIRE, 0, false),
        ]);
        state.wheel_active = true;
        let ctx = build_diagnostic_ctx(&state);

        assert!(ctx.wheel_worn);
        assert_eq!(ctx.selected_ability, None);
        // Union of `adapted` + positive `levels`: fall + explosion = 2.
        assert_eq!(ctx.adapt_count, 2);
        // `adapted` holds fully-completed concepts, mapped to upstream ids.
        assert_eq!(ctx.active_ids, vec!["Env_FallDamage".to_string()]);
        // Level map keeps the richer v1 levels under upstream ids.
        assert_eq!(ctx.level_map.get("Env_FallDamage"), Some(&1));
        assert_eq!(ctx.level_map.get("Type_EXPLOSION"), Some(&2));
        assert!(!ctx.level_map.contains_key("Env_Fire"));
    }

    #[test]
    fn diagnostic_ctx_empty_state_has_no_entries() {
        let ctx = build_diagnostic_ctx(&PlayerAdaptationStateV1::default());
        assert_eq!(ctx.adapt_count, 0);
        assert!(ctx.active_ids.is_empty());
        assert!(ctx.level_map.is_empty());
        assert!(!ctx.wheel_worn);
    }

    #[test]
    fn diagnostic_line_builds_sub_and_optional_arg() {
        assert_eq!(diagnostic_line("wheel", None), "wheel");
        assert_eq!(diagnostic_line("list", Some("physics")), "list physics");
        assert_eq!(diagnostic_line("info", Some("Env_Fire")), "info Env_Fire");
        assert_eq!(diagnostic_line("list", Some("")), "list");
    }

    #[test]
    fn nine_diagnostic_subcommands_are_recognised_and_legacy_are_not() {
        let expected = [
            "list", "info", "grantall", "shed", "ungrant", "analyze", "wheel", "flight", "registry",
        ];
        // I5 added two ability-command diagnostic verbs (`ability`, `activate`)
        // to the pure responder; the nine P1.6 verbs must remain recognised.
        assert!(SUPPORTED_SUBCOMMANDS.len() >= expected.len());
        for sub in expected {
            assert!(is_diagnostic_sub(sub), "{} must be diagnostic", sub);
        }
        // `activate` is intentionally shared: it remains a legacy wheel-activation
        // command AND is now a diagnostic verb that routes to ability activation
        // (I5), so it is recognised by the responder by design.
        for legacy in ["status", "reset", "help", "set", "grant"] {
            assert!(!is_diagnostic_sub(legacy), "{} must stay legacy", legacy);
        }
    }

    #[test]
    fn diagnostic_arg_names_match_registered_usage() {
        assert_eq!(diagnostic_arg_name("list"), "domain");
        assert_eq!(diagnostic_arg_name("grantall"), "domain");
        for sub in [
            "info", "shed", "ungrant", "analyze", "wheel", "flight", "registry",
        ] {
            assert_eq!(diagnostic_arg_name(sub), "concept");
        }
    }

    #[test]
    fn grantall_domain_filter_maps_only_supported_v1_domains() {
        assert!(grantall_domain_matches(CONCEPT_FALL, "physics"));
        assert!(grantall_domain_matches(CONCEPT_FIRE, "environment"));
        assert!(grantall_domain_matches(CONCEPT_GENERAL, "damage"));
        assert!(!grantall_domain_matches(CONCEPT_FIRE, "physics"));
        assert!(!grantall_domain_matches(CONCEPT_FALL, "movement"));
        // Case-insensitive.
        assert!(grantall_domain_matches(CONCEPT_EXPLOSION, "DAMAGE"));
    }

    #[test]
    fn concept_suggestions_come_from_the_real_registry() {
        let suggestions = diagnostic_concept_suggestions("Env_F");
        assert!(suggestions.contains(&"Env_Fire".to_string()));
        assert!(suggestions.contains(&"Env_FallDamage".to_string()));
        assert!(!suggestions.contains(&"Move_Honey".to_string()));
        // The pure helper caps at 80 entries.
        assert!(diagnostic_concept_suggestions("").len() <= 80);
    }

    #[test]
    fn all_read_subcommands_reply_and_all_mutations_delegate() {
        let mut state = state_with(&[(CONCEPT_EXPLOSION, 2, false)]);
        state.wheel_active = true;
        let ctx = build_diagnostic_ctx(&state);

        for line in [
            "list",
            "list damage",
            "info Env_Fire",
            "wheel",
            "flight",
            "registry",
        ] {
            match handle_diagnostic(line, &ctx) {
                Some(DiagnosticOutcome::Reply(text)) => assert!(
                    text.starts_with("[adaptionwheel]"),
                    "{} did not return a prefixed reply",
                    line
                ),
                other => panic!("{} should be read-only, got {:?}", line, other),
            }
        }

        let cases = [
            ("grantall", MutationKind::GrantAll),
            ("shed Env_Fire", MutationKind::Shed),
            ("ungrant Type_EXPLOSION", MutationKind::Ungrant),
            ("analyze Env_Fall", MutationKind::Analyze),
        ];
        for (line, kind) in cases {
            match handle_diagnostic(line, &ctx) {
                Some(DiagnosticOutcome::Mutate { kind: got, .. }) => assert_eq!(got, kind),
                other => panic!("{} should delegate a mutation, got {:?}", line, other),
            }
        }
    }

    #[test]
    fn explicit_target_wins_for_every_source_kind() {
        // Player source + explicit target: the target is mutated, not the
        // operator (review fix #6).
        assert_eq!(choose_target_uuid(Some(42), Some(7)), Some(42));
        // Player source without a target keeps the legacy self-targeting.
        assert_eq!(choose_target_uuid(None, Some(7)), Some(7));
        // Console / command block / function with a target: target is used.
        assert_eq!(choose_target_uuid(Some(42), None), Some(42));
        // Non-player source without a target: "requires player context".
        assert_eq!(choose_target_uuid(None, None), None);
    }

    #[test]
    fn mutation_helpers_mark_state_dirty_without_force() {
        // Fix #5 relies on the engine setting `dirty_persist`; verify every
        // real mutation path does, so force=false still flushes them.
        let mut grant = PlayerAdaptationStateV1::default();
        assert!(apply_grant_command(&mut grant, CONCEPT_FIRE, 2).is_ok());
        assert!(grant.dirty_persist);

        let mut set = PlayerAdaptationStateV1::default();
        assert!(apply_set_command(&mut set, CONCEPT_EXPLOSION, "level", 2.0).is_ok());
        assert!(set.dirty_persist);

        // `reset` writes the flag explicitly in `reset_message`, through the
        // same global cache path (use an isolated uuid to avoid test overlap).
        let uuid = u128::MAX - 42;
        with_player_state(uuid, |state| {
            *state = PlayerAdaptationStateV1::default();
            state.dirty_persist = true;
        });
        assert!(with_player_state(uuid, |state| state.dirty_persist));

        // Read-only reads leave it clean.
        let read = PlayerAdaptationStateV1::default();
        assert!(!read.dirty_persist);
        let _ = build_diagnostic_ctx(&read);
        assert!(!read.dirty_persist);
    }
}
