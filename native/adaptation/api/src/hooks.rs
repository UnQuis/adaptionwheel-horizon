//! Horizon hook wiring and integration points.
//! Keep handlers thin: extract context -> call activation/engine/net/state services -> emit sync.

use crate::activation::{refresh_activation, InventorySlotProvider, PlayerInventorySnapshot};
use crate::config::MvpConfig;
use crate::engine::AdaptationEngine;
use crate::net::{self, AdaptationEventKind, AdaptationEventMsg, DeltaStateSync, EntrySnapshot};
use crate::scenarios::ScenarioContext;
use crate::state::PlayerAdaptationStateV1;
use horizon_client::mod_api::{Host, PlayerLoggedInEvent, Text};

/// Global-ish service bundle placeholder.
/// `TBD(Horizon API)`: store in mod state/host extension/world singleton as appropriate.
#[derive(Default)]
pub struct RuntimeServices {
    pub cfg: MvpConfig,
    pub engine: Option<AdaptationEngine>,
    pub activation: InventorySlotProvider,
    /// Client-only mirror for HUD rendering.
    pub client_state: net::ClientAdaptationState,
}

impl RuntimeServices {
    pub fn ensure_engine(&mut self) {
        if self.engine.is_none() {
            self.engine = Some(AdaptationEngine::new_mvp(&self.cfg));
        }
    }
}

pub fn on_registered(_host: &mut dyn Host) {
    // TODO: initialize runtime services, register attachment/messages/commands if host supports here.
}

pub fn on_player_logged_in(
    _mod: &super::AdaptionWheelMod,
    host: &mut dyn Host,
    event: &mut PlayerLoggedInEvent<'_>,
) {
    // PR-2 skeleton behavior: keep visible smoke message and mark integration points.
    let line = format!(
        "[AdaptionWheel MVP skeleton] hello, {}. server-authoritative core hooks pending Horizon API wiring (139e83fff8).",
        event.name
    );
    host.tell(event.player, Text::literal(line));

    // TODO(Horizon API):
    // - load/init attachment state for event.player
    // - compute activation from inventory snapshot
    // - net::send_full_sync_to_player(...)
}

/// Common server-side damage processing path for `living_incoming_damage` and `living_fall`.
/// Returns effective damage multiplier (apply via mutable event field or return value per Horizon API).
pub fn handle_damage_like_event(
    services: &mut RuntimeServices,
    state: &mut PlayerAdaptationStateV1,
    inv: &PlayerInventorySnapshot,
    mut ctx: ScenarioContext,
) -> f32 {
    services.ensure_engine();
    let activation_changed = refresh_activation(&services.activation, inv, &services.cfg, state);
    if activation_changed {
        // Caller should enqueue ActivationChanged event/sync.
    }

    let Some(engine) = services.engine.as_ref() else { return 1.0; };
    // 1) Mitigation from already-completed adaptations.
    let mitigation = engine.mitigation_multiplier(&services.cfg, state, &ctx);
    // 2) Progress accumulation uses post-mitigation (or raw, per design choice) context; keep simple for MVP.
    ctx.incoming_damage *= mitigation;
    let _result = engine.process_event(&services.cfg, state, &ctx);
    mitigation
}

/// Build a delta sync payload for changed activation and/or specific entries.
pub fn build_delta_for_entries(
    state: &PlayerAdaptationStateV1,
    activation_changed: bool,
    concepts: &[crate::state::ConceptId],
) -> DeltaStateSync {
    DeltaStateSync {
        wheel_active: activation_changed.then_some(state.wheel_active),
        active_source: activation_changed.then_some(state.active_source),
        updated_entries: concepts.iter().map(|c| EntrySnapshot::from_state(*c, state)).collect(),
        removed_entries: vec![],
    }
}

pub fn build_activation_event(active: bool) -> AdaptationEventMsg {
    AdaptationEventMsg {
        kind: AdaptationEventKind::ActivationChanged,
        concept_id: None,
        level: None,
        progress: Some(if active { 1.0 } else { 0.0 }),
    }
}

// --- Horizon hook stubs (to be connected once exact event types are confirmed) ---
// pub fn on_living_incoming_damage(host: &mut dyn Host, event: &mut horizon_client::mod_api::LivingIncomingDamageEvent<'_>) {
//     // 1) extract player/inventory + damage context
//     // 2) handle_damage_like_event(...)
//     // 3) event.damage *= mitigation (or equivalent API)
//     // 4) send delta/full sync if needed
// }
// pub fn on_living_fall(host: &mut dyn Host, event: &mut horizon_client::mod_api::LivingFallEvent<'_>) {
//     // similar to incoming damage with ScenarioContext { is_fall: true, fall_distance, ... }
// }
// pub fn on_render_hud(host: &mut dyn Host, event: &mut horizon_client::mod_api::RenderHudEvent<'_>) {
//     // draw minimal overlay from services.client_state
// }
