//! Horizon hook handlers for the AdaptionWheel v2 native mod.
//!
//! These are free functions; the crate-root `Hooks` implementation delegates
//! to them (see `lib.rs`). Semantics are a faithful port of the validated v1
//! `Hooks` impl in `native/adaptation/api/src/lib.rs`.
//!
//! New-API migration notes (verified against the pinned Horizon vocabulary):
//! - `PlayerLoggedInEvent { player: u128, name: &str }`,
//!   `PlayerCloneEvent { player: u128, name: &str, was_death: bool, original }`,
//!   `PlayerInventoryChangedEvent { player: u128, stacks: &[Option<ItemStack>] }`.
//! - `PlayerTickPreEvent`/`PlayerTickPostEvent { player: PlayerRef }` receive a
//!   `HostSide`; the per-tick map refresh + persistence flush is done on the
//!   `HostSide::Server` arm of **pre** (the direct analogue of v1 `player_tick`).
//! - `LivingIncomingDamageEvent` has no victim UUID and no `amount`: the
//!   victim is `entity: i32` (mapped with `player_uuid_for_entity`), the damage
//!   concept is `source.damage_type` (`kind` is the *entity* type), and the
//!   mutable amount is `container.new_damage()` / `set_new_damage()`.
//! - `LivingFallEvent { entity: u32, distance: f64, multiplier: f32, ... }` and
//!   `LivingDrownEvent { entity: u32, is_drowning: bool, damage_amount: f32,
//!   ... }` are given only a `&dyn LoopContext`, so all state work goes through
//!   `state_impl`'s global cache; attachment persistence is deferred to the
//!   host-owning hooks (the per-tick flush).

use mod_api::{
    Flow, HostSide, LevelRandomHost, LivingDrownEvent, LivingFallEvent, LivingIncomingDamageEvent,
    LoopContext, PlayerCloneEvent, PlayerInventoryChangedEvent, PlayerLoggedInEvent, PlayerRef,
    PlayerTickPostEvent, PlayerTickPreEvent, ServerHost, Text,
};

use crate::engine_impl::{
    CONCEPT_DROWNING, DROWNING_MAX_LEVEL, apply_damage_adaptation, apply_fall_progress,
    apply_leveled_progress, fall_mitigation_factor, mitigation_factor_for_concept,
};

const MOD_ID: &str = "adaptionwheel";

/// Send one system line to a player by UUID (v1 `host.tell(uuid, ...)`).
fn tell_player(host: &mut dyn ServerHost, uuid: u128, line: String) {
    if let Some(player) = host.player(uuid) {
        let _ = player.tell(host, Text::literal(line));
    }
}

/// v1 `player_logged_in`.
pub fn player_logged_in(host: &mut dyn ServerHost, event: &mut PlayerLoggedInEvent<'_>) {
    crate::state_impl::refresh_entity_player_map_from_host(host);
    // Best-effort: load persisted attachment state into runtime cache; if missing, seed attachment.
    crate::state_impl::sync_cache_from_attachment_if_present(host, event.player);
    let state = crate::state_impl::get_player_state(event.player);
    // v1 `tell_login_status`.
    tell_player(
        host,
        event.player,
        format!(
            "[{}] ready. activation: {} ({:?}). Wheel works when present anywhere in inventory.",
            MOD_ID, state.wheel_active, state.active_source
        ),
    );
    tell_player(
        host,
        event.player,
        format!("[{}] {}", MOD_ID, state.summary()),
    );
    crate::state_impl::persist_player_state_if_needed(host, event.player, false);
}

/// v1 `player_clone` (attachment `copy_on_death=true`: reload copied state if available).
pub fn player_clone(host: &mut dyn ServerHost, event: &mut PlayerCloneEvent<'_>) {
    crate::state_impl::sync_cache_from_attachment_if_present(host, event.player);
    crate::state_impl::with_player_state(event.player, |state| {
        state.normalize();
        state.dirty_persist = false;
    });
    crate::state_impl::persist_player_state_if_needed(host, event.player, false);
    let _ = event.was_death;
}

/// v1 `player_tick`: keep the entity-id -> player-uuid map fresh for hooks that
/// only provide entity ids, and flush dirty state for online players.
///
/// The v2 API splits `player_tick` into pre/post with a `HostSide`. Map refresh
/// and the persistence flush belong in **pre**, once per tick, before other
/// hooks can observe the entity ids; `player_tick_post` is intentionally a
/// no-op (see below). Only the server arm has attachment/host access.
pub fn player_tick_pre(host: HostSide, event: &mut PlayerTickPreEvent) {
    let _ = event;
    let HostSide::Server(host) = host else {
        return;
    };
    crate::state_impl::refresh_entity_player_map_from_host(host);
    // Collect the handles first so the `&dyn Players` borrow ends before the
    // mutable host borrow used by the persistence flush.
    let players: Vec<PlayerRef> = host.players().server_players();
    for player in players {
        if let Some(uuid) = player.profile(host).map(|profile| profile.uuid) {
            crate::state_impl::persist_player_state_if_needed(host, uuid, false);
        }
    }
}

/// Companion to `player_tick_pre`; no work is needed after the tick because
/// v1 performed the whole refresh + flush once, at tick start.
pub fn player_tick_post(host: HostSide, event: &mut PlayerTickPostEvent) {
    let _ = (host, event);
}

/// v1 `player_inventory_changed`: wheel activation anywhere in inventory.
pub fn player_inventory_changed(
    host: &mut dyn ServerHost,
    event: &mut PlayerInventoryChangedEvent<'_>,
) {
    // Ensure cache reflects persisted state before applying activation changes.
    crate::state_impl::sync_cache_from_attachment_if_present(host, event.player);
    // `ItemStack::item()` is an opaque `ItemKey`; `ItemKey::id(host)` is the
    // canonical item id string (`minecraft:compass`, ...).
    let ids: Vec<Option<String>> = event
        .stacks
        .iter()
        .map(|slot| slot.as_ref().map(|stack| stack.item().id(host)))
        .collect();
    let scan = crate::engine_impl::activation_from_item_ids(&ids);
    let changed = crate::state_impl::set_player_activation(event.player, scan);
    if changed {
        let state = crate::state_impl::get_player_state(event.player);
        tell_player(
            host,
            event.player,
            format!(
                "[{}] wheel activation: {} (source: {:?}, anywhere-in-inventory scan).",
                MOD_ID, state.wheel_active, state.active_source
            ),
        );
    }
    crate::state_impl::persist_player_state_if_needed(host, event.player, false);
}

/// v1 `living_incoming_damage_v3`, adapted to the v2 event:
/// - victim comes from `event.entity` via the entity/uuid map (no `victim_uuid`);
/// - damage type comes from `event.source.damage_type` (`event.kind` is the
///   entity type, not the damage type);
/// - the mutable damage is `event.container.new_damage()` / `set_new_damage()`.
pub fn living_incoming_damage(
    level: &mut dyn LevelRandomHost,
    event: &mut LivingIncomingDamageEvent<'_>,
) -> Flow {
    let _ = level;
    let Some(player_uuid) = u32::try_from(event.entity)
        .ok()
        .and_then(crate::state_impl::player_uuid_for_entity)
    else {
        return Flow::Continue;
    };
    let mut amount = event.container.new_damage();
    if amount <= 0.0 {
        return Flow::Continue;
    }
    // Server-authoritative progress/mitigation.
    crate::state_impl::with_player_state(player_uuid, |state| {
        apply_damage_adaptation(state, event.source.damage_type, &mut amount);
    });
    if amount <= 0.0 {
        amount = 0.0;
    }
    event.container.set_new_damage(amount);
    Flow::Continue
}

/// v1 `living_fall`: fall mitigation/progress (entity-id based, no Host).
pub fn living_fall(context: &dyn LoopContext, event: &mut LivingFallEvent<'_>) -> Flow {
    let _ = context;
    let Some(player_uuid) = crate::state_impl::player_uuid_for_entity(event.entity) else {
        return Flow::Continue;
    };
    let gain_base = (event.distance as f32).max(event.multiplier * 2.0).max(0.0);
    crate::state_impl::with_player_state(player_uuid, |state| {
        let factor = fall_mitigation_factor(state);
        if factor < 1.0 {
            event.multiplier *= factor;
            event.distance *= factor as f64;
        }
        if gain_base > 0.0 {
            let _ = apply_fall_progress(state, gain_base);
        }
    });
    Flow::Continue
}

/// v1 `living_drown`: drowning mitigation/progress (entity-id based, no Host).
pub fn living_drown(context: &dyn LoopContext, event: &mut LivingDrownEvent<'_>) -> Flow {
    let _ = context;
    let Some(player_uuid) = crate::state_impl::player_uuid_for_entity(event.entity) else {
        return Flow::Continue;
    };
    if !event.is_drowning && event.damage_amount <= 0.0 {
        return Flow::Continue;
    }
    crate::state_impl::with_player_state(player_uuid, |state| {
        let factor = mitigation_factor_for_concept(state, CONCEPT_DROWNING);
        if event.damage_amount > 0.0 {
            event.damage_amount *= factor;
        }
        let gain = if event.damage_amount > 0.0 {
            event.damage_amount
        } else {
            1.0
        };
        let _ = apply_leveled_progress(state, CONCEPT_DROWNING, gain, DROWNING_MAX_LEVEL, 8.0);
    });
    Flow::Continue
}
