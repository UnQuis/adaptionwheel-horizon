//! Typed server commands for the AdaptionWheel native mod (Horizon mod API v2).
//!
//! `register` contributes the six original usages plus explicit `<target:player>`
//! variants so the console / command blocks / functions can drive the same
//! subcommands. `run` matches the parsed `CommandInvocation::literals` and reads
//! named arguments; it never splits a raw command line.
//!
//! Response wording is kept from the old `native/adaptation/api` command handler.

use mod_api::{
    Command, CommandArgumentValue, CommandSource, Flow, RegisterCommandsEvent,
    RunModCommandEvent, ServerHost, Text,
};

use crate::engine_impl::{
    apply_grant_command, apply_set_command, format_status, normalize_concept,
    PlayerAdaptationStateV1, ALL_CONCEPTS,
};
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

/// The player whose state the subcommand acts on.
///
/// A player source keeps the old behaviour (it always acts on itself, even when
/// a target argument was supplied). Every other source uses the explicit
/// `<target:player>` argument. `PlayerRef::uuid(self) -> u128` is at
/// `horizon-mod-vocab/src/entity/types.rs:1097`.
fn resolve_target(event: &RunModCommandEvent<'_>) -> Option<u128> {
    if let CommandSource::Player { player, .. } = event.source {
        return Some(player.uuid());
    }
    match event.command.argument("target") {
        Some(CommandArgumentValue::Player(player)) => Some(player.uuid()),
        _ => None,
    }
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
        Ok(message) => format!(
            "[{}] {}. {}",
            MOD_ID,
            message,
            status_message_tail(uuid)
        ),
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
        Ok(message) => format!(
            "[{}] {}. {}",
            MOD_ID,
            message,
            status_message_tail(uuid)
        ),
        Err(error) => format!("[{}] {}", MOD_ID, error),
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
        other => format!(
            "[{}] unknown subcommand '{}'. Try: status, activate, set, grant, reset, help",
            MOD_ID, other
        ),
    };

    // Persist command-driven mutations (set/grant/reset) best-effort.
    persist_player_state_if_needed(host, uuid, true);
    // `Commands::feedback` routes to the originating source and honours silence;
    // it returns Result, so explicitly discard it (host/services.rs:130).
    let _ = host.commands().feedback(Text::literal(message));
    Flow::Cancel
}
