//! Debug/admin commands (server-side). Registration via Horizon commands hooks.

use crate::engine::AdaptationEngine;
use crate::net;
use crate::state::{ConceptId, PlayerAdaptationStateV1};

/// Logical command actions; map to Horizon command builder/dispatcher at wiring time.
#[derive(Debug, Clone)]
pub enum AdaptionCommand {
    Inspect,
    Grant { concept: ConceptId, level: u8 },
    Reset,
}

#[derive(Debug, Clone)]
pub enum CommandOutcome {
    Message(String),
    StateMutated,
}

/// Execute a command against a player's adaptation state (server-authoritative).
/// Caller is responsible for permission checks, attachment lookup, and sync.
pub fn execute(
    _engine: &AdaptationEngine,
    state: &mut PlayerAdaptationStateV1,
    cmd: AdaptionCommand,
) -> CommandOutcome {
    match cmd {
        AdaptionCommand::Inspect => {
            let mut lines = vec![format!(
                "wheel_active={} source={:?} schema={}",
                state.wheel_active, state.active_source, state.schema_version
            )];
            for (id, e) in &state.entries {
                lines.push(format!(
                    "- {}: level={} progress={:.2} completed={}",
                    id.as_str(), e.level, e.progress, e.completed
                ));
            }
            CommandOutcome::Message(lines.join("\n"))
        }
        AdaptionCommand::Grant { concept, level } => {
            let e = state.ensure_entry_mut(concept);
            e.level = level.max(1);
            e.completed = true;
            e.progress = e.progress.max(0.0);
            state.mark_dirty();
            CommandOutcome::StateMutated
        }
        AdaptionCommand::Reset => {
            *state = PlayerAdaptationStateV1::default();
            state.mark_dirty();
            CommandOutcome::StateMutated
        }
    }
}

/// Registration point: `/adaptionwheel inspect|grant|reset`.
/// `TBD(Horizon API @139e83fff8)`: register command tree and permission requirements.
pub fn register_commands(_host: &mut dyn horizon_client::mod_api::Host) {
    // TODO: host.commands().literal("adaptionwheel")
    //   .then(literal("inspect").executes(...))
    //   .then(literal("grant").arg("concept").arg_opt("level").requires(op).executes(...))
    //   .then(literal("reset").requires(op).executes(...))
}

/// Helper for command handlers after mutation.
pub fn sync_after_mutation(_host: &mut dyn horizon_client::mod_api::Host, _player: (), state: &PlayerAdaptationStateV1) {
    let _ = net::encode_full(state);
    // TODO: net::send_full_sync_to_player(host, player, state)
}
