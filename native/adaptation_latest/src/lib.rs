//! AdaptionWheel native mod — migrated to the new Horizon mod API (`mod_api`, 8bc19c5f97).
//!
//! Server-authoritative adaptation core. The `#[mod_api::horizon_mod]` attribute
//! derives the subscription list and exports `pub fn hooks()` for the launcher.
use mod_api::{
    self, ClientHost, Flow, GuiLayerDrawEvent, Hooks, HostSide, LevelRandomHost, LivingDrownEvent, LivingFallEvent,
    LivingIncomingDamageEvent, LoopContext, PlayerCloneEvent, PlayerInventoryChangedEvent,
    PlayerLoggedInEvent, PlayerTickPostEvent, PlayerTickPreEvent, RegisterCommandsEvent,
    RegisterGuiLayersEvent, Registrar, RegistrationContext, RunModCommandEvent, ServerHost,
};

mod abilities;
mod gameplay_wiring;
mod existence_mut;
mod fist_ladders;
mod commands_impl;
mod concepts_registry;
mod damage_categories;
mod engine_impl;
mod hooks_impl;
mod state_v2;
mod task_system;
mod state_impl;
mod synergies;
mod ui;


fn hud_snapshot() -> ui::HudSnapshot<'static> {
    let map = state_impl::states();
    let guard = map.lock().unwrap_or_else(|e| e.into_inner());
    let state = guard.values().next().cloned().unwrap_or_default();
    let mut concepts = [ui::ConceptSnapshot {
        label: "",
        level: 0,
        max_level: 1,
        progress: 0.0,
    }; 7];
    for (slot, concept) in engine_impl::ALL_CONCEPTS.iter().enumerate() {
        let entry = state.entries.get(*concept);
        concepts[slot] = ui::ConceptSnapshot {
            label: concept,
            level: entry.map(|e| e.level).unwrap_or(0),
            max_level: engine_impl::concept_max_level(concept),
            progress: entry
                .map(|e| {
                    let threshold = engine_impl::leveled_threshold(concept, e.level);
                    if threshold > 0.0 { e.progress / threshold } else { 0.0 }
                })
                .unwrap_or(0.0),
        };
    }
    ui::HudSnapshot {
        wheel_active: state.wheel_active,
        concepts,
        adversity: ui::AdversitySnapshot { active: false, label: "adversity", ticks: 0 },
    }
}

fn synergy_snapshot(player: u128) -> gameplay_wiring::SynergySnapshot {
    let state = state_impl::get_player_state(player);
    let v2 = state_impl::get_v2_state(player);
    let mut levels = std::collections::BTreeMap::new();
    let mut adapted = std::collections::BTreeSet::new();
    let mut disabled = std::collections::BTreeSet::new();
    for (id, entry) in &state.entries {
        levels.insert(id.clone(), entry.level);
        if entry.completed { adapted.insert(id.clone()); }
    }
    for id in &v2.extras.disabled { disabled.insert(id.clone()); }
    gameplay_wiring::SynergySnapshot {
        levels,
        adapted,
        disabled,
        configured_aquatic_bonus: 0.5,
        damage_taken: 0.0,
    }
}

#[derive(Default)]
struct Mod;

#[mod_api::horizon_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, cx: &RegistrationContext) {
        state_impl::register_attachments(r, cx);
    }

    fn register_gui_layers(&self, _context: &RegistrationContext, event: &mut RegisterGuiLayersEvent<'_>) {
        ui::register(event);
    }

    fn gui_layer_draw(&self, _host: &dyn ClientHost, event: &mut GuiLayerDrawEvent<'_>) {
        ui::on_gui_layer_draw(event, &hud_snapshot());
    }

    fn register_commands(&self, host: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        let _ = host;
        commands_impl::register(event);
    }

    fn run_mod_command(&self, host: &mut dyn ServerHost, event: &mut RunModCommandEvent<'_>) -> Flow {
        commands_impl::run(host, event)
    }

    fn player_logged_in(&self, host: &mut dyn ServerHost, event: &mut PlayerLoggedInEvent<'_>) {
        hooks_impl::player_logged_in(host, event);
    }

    fn player_clone(&self, host: &mut dyn ServerHost, event: &mut PlayerCloneEvent<'_>) {
        hooks_impl::player_clone(host, event);
    }

    fn player_tick_pre(&self, host: HostSide, event: &mut PlayerTickPreEvent) {
        hooks_impl::player_tick_pre(host, event);
    }

    fn player_tick_post(&self, host: HostSide, event: &mut PlayerTickPostEvent) {
        hooks_impl::player_tick_post(host, event);
        if let HostSide::Server(server) = host {
            let snap = synergy_snapshot(event.player.uuid());
            let effects = gameplay_wiring::synergy_effects_for(&snap);
            if !effects.is_empty() {
                let _ = gameplay_wiring::apply_synergy_effects(server, event.player, &effects);
            }
        }
    }

    fn player_inventory_changed(
        &self,
        host: &mut dyn ServerHost,
        event: &mut PlayerInventoryChangedEvent<'_>,
    ) {
        hooks_impl::player_inventory_changed(host, event);
    }

    fn living_incoming_damage(
        &self,
        level: &mut dyn LevelRandomHost,
        event: &mut LivingIncomingDamageEvent<'_>,
    ) -> Flow {
        hooks_impl::living_incoming_damage(level, event)
    }

    fn living_fall(&self, context: &dyn LoopContext, event: &mut LivingFallEvent<'_>) -> Flow {
        hooks_impl::living_fall(context, event)
    }

    fn living_drown(&self, context: &dyn LoopContext, event: &mut LivingDrownEvent<'_>) -> Flow {
        hooks_impl::living_drown(context, event)
    }
}
