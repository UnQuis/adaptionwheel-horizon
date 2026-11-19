//! AdaptionWheel native mod — migrated to the new Horizon mod API (`mod_api`, 8bc19c5f97).
//!
//! Server-authoritative adaptation core. The `#[mod_api::sunrise_mod]` attribute
//! derives the subscription list and exports `pub fn hooks()` for the launcher.
use mod_api::{
    self, Flow, Hooks, HostSide, HitHost, LivingDrownEvent, LivingFallEvent,
    LivingIncomingDamageEvent, LoopContext, PlayerCloneEvent, PlayerInventoryChangedEvent,
    PlayerLoggedInEvent, PlayerTickPostEvent, PlayerTickPreEvent, RegisterCommandsEvent,
    Registrar, RegistrationContext, RunModCommandEvent, ServerHost,
};

mod abilities;
mod commands_impl;
mod concepts_registry;
mod damage_categories;
mod engine_impl;
mod hooks_impl;
mod state_v2;
mod task_system;
mod state_impl;

#[derive(Default)]
struct Mod;

#[mod_api::sunrise_mod]
impl Hooks for Mod {
    fn register(&self, r: &mut dyn Registrar, cx: &RegistrationContext) {
        state_impl::register_attachments(r, cx);
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
        level: &mut dyn HitHost,
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
