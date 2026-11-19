//! Horizon hook handlers for the AdaptionWheel v2 native mod.
//!
//! These are free functions; the crate-root `Hooks` implementation delegates
//! to them (see `lib.rs`). Semantics are a faithful port of the validated v1
//! `Hooks` impl in `native/adaptation/api/src/lib.rs`, plus the P1 surface-exposure
//! and damage-category integrations.
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
//!
//! P1 integration (all queries verified at Horizon `8bc19c5f97`):
//! - Surface exposure uses `EntityRef::{is_in_lava, is_in_water, eye_fluid,
//!   air_supply, level, block_position, on_block, bounding_box}`,
//!   `LevelRef::{brightness, block}`, `BlockKey::id` and `TagsHost::block_in_tag`.
//!   Darkness is a LIGHT LEVEL read (`LevelRef::brightness(LightLayer::Block, pos)`),
//!   not a potion check. See the report for exact signatures + file:line.
//! - Damage categorization uses `HurtSource::damage_type` with
//!   `damage_categories::categorize_with_tags`. Horizon 8bc19c5f97 exposes no
//!   damage-type tag query on `HurtSource`/`DamageContainer`/`Host`, so tags are
//!   empty and the id-based `categorize` fallback applies.

use mod_api::{
    Flow, HostSide, HitHost, LightLayer, LivingDrownEvent, LivingFallEvent,
    LivingIncomingDamageEvent, LoopContext, PlayerCloneEvent, PlayerInventoryChangedEvent,
    PlayerLoggedInEvent, PlayerRef, PlayerTickPostEvent, PlayerTickPreEvent, ServerHost, Text,
};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::damage_categories::{
    DamageCategory, categorize_with_tags, env_immunity_concept, full_immunity_applies,
    maxed_immunity_applies,
};
use crate::engine_impl::{
    CONCEPT_DROWNING, DROWNING_MAX_LEVEL, PlayerAdaptationStateV1, apply_damage_adaptation,
    apply_fall_progress, apply_leveled_progress, fall_mitigation_factor,
    mitigation_factor_for_concept,
};
use crate::task_system::{
    AdaptionTask, SurfaceExposure, exposure_concepts, is_surface_exposure_concept,
    start_or_accelerate, tick_tasks,
};

const MOD_ID: &str = "adaptionwheel";

/// One server tick in seconds (20 TPS). P1 task timers are expressed in seconds.
const TICK_SECONDS: f32 = 0.05;

/// Block light at or below this level counts as darkness for `Env_Darkness`.
const DARKNESS_LIGHT_LEVEL: i32 = 7;

/// Max level of the type ladders used by the maxed-immunity check (v1 engine caps at 3).
const TYPE_IMMUNITY_MAX_LEVEL: u8 = 3;

/// Module-local per-player analysis-task cache. The lead may later move this into
/// `state_v2` persistence; it is deliberately private to this hook module.
fn player_tasks() -> &'static Mutex<HashMap<u128, Vec<AdaptionTask>>> {
    static TASKS: OnceLock<Mutex<HashMap<u128, Vec<AdaptionTask>>>> = OnceLock::new();
    TASKS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Compare a canonical block id (`minecraft:ice`) to a bare path (`ice`).
fn block_id_is(id: &str, path: &str) -> bool {
    id.rsplit(':').next() == Some(path)
}

/// A P1 `Env_*`/`Type_*` concept is complete in either spelling: capitalised
/// (granted by the task system / `damage_categories`) or lowercase (v1 engine).
fn concept_is_complete(state: &PlayerAdaptationStateV1, concept: &str) -> bool {
    state.concept_completed(concept) || state.concept_completed(&concept.to_ascii_lowercase())
}

/// Level of a concept in either spelling (see [`concept_is_complete`]).
fn concept_level_of(state: &PlayerAdaptationStateV1, concept: &str) -> u8 {
    state
        .concept_level(concept)
        .max(state.concept_level(&concept.to_ascii_lowercase()))
}

/// Build the eleven P1 surface-exposure flags for one player from real Horizon
/// queries. Every flag below is backed by a `mod_api` call; see the report for
/// exact method signatures and generated-file line numbers.
fn surface_exposure_for_player(host: &dyn ServerHost, player: PlayerRef) -> SurfaceExposure {
    let mut exposure = SurfaceExposure::default();
    let entity = player.entity();

    // in_lava — `EntityRef::is_in_lava(host) -> Option<bool>` (server).
    exposure.in_lava = entity.is_in_lava(host).unwrap_or(false);

    // in_water_or_liquid — `EntityRef::is_in_water(host)` plus the fluid at the
    // eyes (`EntityRef::eye_fluid(host) -> Option<Option<String>>`, Some(None) in air).
    exposure.in_water_or_liquid =
        entity.is_in_water(host).unwrap_or(false) || entity.eye_fluid(host).flatten().is_some();

    // drowning — `EntityRef::air_supply(host) -> Option<i32>`.
    exposure.drowning = entity.air_supply(host).map_or(false, |air| air <= 0);

    let Some(level) = entity.level(host) else {
        return exposure;
    };

    // in_darkness — LIGHT LEVEL, not the potion effect:
    // `LevelRef::brightness(host, LightLayer::Block, block_position)`.
    if let Some(pos) = entity.block_position(host) {
        let block_light = level.brightness(host, LightLayer::Block, pos).unwrap_or(15);
        exposure.in_darkness = block_light <= DARKNESS_LIGHT_LEVEL;
    }

    // on_ice / on_slime / on_soul_sand — the block under the feet via
    // `EntityRef::on_block` (fallback: one below `block_position`),
    // `LevelRef::block`, `BlockKey::id` and `TagsHost::block_in_tag`.
    if let Some(pos) = entity
        .on_block(host)
        .or_else(|| entity.block_position(host).map(|[x, y, z]| [x, y - 1, z]))
    {
        if let Some(key) = level.block(host, pos) {
            let id = key.id(host);
            exposure.on_ice = host.block_in_tag(key, "minecraft:ice").unwrap_or(false)
                || block_id_is(&id, "ice")
                || block_id_is(&id, "packed_ice")
                || block_id_is(&id, "blue_ice");
            exposure.on_slime = block_id_is(&id, "slime_block");
            exposure.on_soul_sand = host
                .block_in_tag(key, "minecraft:soul_speed_blocks")
                .unwrap_or(false)
                || block_id_is(&id, "soul_sand")
                || block_id_is(&id, "soul_soil");
        }
    }

    // in_cobweb / in_sweet_berry / in_bubble_column / in_powder_snow — AABB
    // overlap via `EntityRef::bounding_box` + `LevelRef::block` (a player box
    // spans only a handful of blocks). There is no dedicated `isInPowderSnow`
    // etc. flag query in mod_api, so the block overlap is the real API path.
    if let Some([[min_x, min_y, min_z], [max_x, max_y, max_z]]) = entity.bounding_box(host) {
        for x in (min_x.floor() as i32)..=(max_x.floor() as i32) {
            for y in (min_y.floor() as i32)..=(max_y.floor() as i32) {
                for z in (min_z.floor() as i32)..=(max_z.floor() as i32) {
                    let Some(key) = level.block(host, [x, y, z]) else {
                        continue;
                    };
                    let id = key.id(host);
                    if block_id_is(&id, "cobweb") {
                        exposure.in_cobweb = true;
                    } else if block_id_is(&id, "sweet_berry_bush") {
                        exposure.in_sweet_berry = true;
                    } else if block_id_is(&id, "bubble_column") {
                        exposure.in_bubble_column = true;
                    } else if block_id_is(&id, "powder_snow") {
                        exposure.in_powder_snow = true;
                    }
                }
            }
        }
    }

    exposure
}

/// Feed one tick of exposure into the module-local cache and return the concept
/// ids whose countdown completed this tick.
fn advance_exposure_tasks(uuid: u128, exposure: &SurfaceExposure) -> Vec<String> {
    let mut cache = player_tasks().lock().unwrap_or_else(|e| e.into_inner());
    let tasks = cache.entry(uuid).or_default();
    for (concept, seconds) in exposure_concepts(exposure) {
        start_or_accelerate(tasks, concept, seconds, TICK_SECONDS);
    }
    tick_tasks(tasks, TICK_SECONDS)
}

/// Mark completed surface-exposure concepts on the player's v1 state so the
/// one-time immunity helpers can observe them, and flag the state for flushing.
fn grant_completed_concepts(uuid: u128, completed: Vec<String>) {
    let mut changed = false;
    crate::state_impl::with_player_state(uuid, |state| {
        for concept in completed {
            if !is_surface_exposure_concept(&concept) {
                continue;
            }
            let entry = state.entry_mut(&concept);
            if !entry.completed {
                entry.completed = true;
                entry.level = entry.level.max(1);
                changed = true;
            }
        }
        if changed {
            state.dirty_persist = true;
        }
    });
}

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
    // The v2 per-player hooks do not run a global map rebuild, so keep the
    // entity-id -> uuid map fresh for this player here. `ServerHost::player(uuid)`
    // returns an owned `Option<PlayerRef>`; `PlayerRef::entity()` and
    // `EntityRef::id()` are Copy value methods with no backend lookup.
    if let Some(player) = host.player(event.player) {
        let entity_id = player.entity().id();
        if entity_id > 0 {
            crate::state_impl::entity_player_map()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(entity_id, event.player);
        }
    }
    crate::state_impl::sync_cache_from_attachment_if_present(host, event.player);
    crate::state_impl::with_player_state(event.player, |state| {
        state.normalize();
        state.dirty_persist = false;
    });
    crate::state_impl::persist_player_state_if_needed(host, event.player, false);
    let _ = event.was_death;
}

/// v1 `player_tick`: keep this player's entity-id -> uuid mapping fresh, flush
/// its dirty state and run the P1 surface-exposure analysis.
///
/// The v2 API splits `player_tick` into pre/post with a `HostSide` and calls
/// **pre once per player per tick**. All work therefore uses `event.player`
/// only: a full `refresh_entity_player_map_from_host` plus a `server_players()`
/// loop here would rebuild the map N times and issue N^2 attachment flushes.
/// `PlayerRef::uuid()` / `PlayerRef::entity().id()` are Copy value methods
/// (no backend), unlike `PlayerRef::profile(host)` which can refuse stale or
/// unavailable players.
pub fn player_tick_pre(host: HostSide, event: &mut PlayerTickPreEvent) {
    let HostSide::Server(host) = host else {
        return;
    };

    let uuid = event.player.uuid();

    // P1 surface-exposure analysis for the player named by this tick event.
    let exposure = surface_exposure_for_player(&*host, event.player);
    let completed = advance_exposure_tasks(uuid, &exposure);
    grant_completed_concepts(uuid, completed);

    // Single-entry map upsert for this player only (no global rebuild).
    let entity_id = event.player.entity().id();
    if entity_id > 0 {
        crate::state_impl::entity_player_map()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(entity_id, uuid);
    }

    // Flush this player's dirty state only — no server_players() loop.
    crate::state_impl::persist_player_state_if_needed(host, uuid, false);
}

/// Companion to `player_tick_pre`; the whole analysis runs at tick start, so no
/// work is needed after the tick.
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
    level: &mut dyn HitHost,
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

    // P1: classify by the real damage type id. Horizon 8bc19c5f97 exposes no
    // damage-type tag query on HurtSource/DamageContainer/Host, so `tags` is
    // empty and `categorize_with_tags` falls back to its id-based `categorize`.
    let damage_type = event.source.damage_type;
    let category: DamageCategory = categorize_with_tags(damage_type, &[]);
    let env_concept = env_immunity_concept(category);

    // Compute the immunity decision first, but still run the v1 damage-adaptation
    // path below on the unmodified amount so legacy progress/tests are unchanged.
    let immune = crate::state_impl::with_player_state(player_uuid, |state| {
        let full = env_concept.map_or(false, |c| concept_is_complete(state, c))
            || full_immunity_applies(|c| concept_is_complete(state, c), category, damage_type);
        let maxed = maxed_immunity_applies(
            |c| concept_level_of(state, c),
            category,
            damage_type,
            TYPE_IMMUNITY_MAX_LEVEL,
            // HurtSource has no `bypasses_invulnerability` flag at 8bc19c5f97.
            false,
        );
        full || maxed
    });

    // Server-authoritative progress/mitigation (legacy v1 path, unchanged).
    crate::state_impl::with_player_state(player_uuid, |state| {
        apply_damage_adaptation(state, damage_type, &mut amount);
    });

    // One-time / maxed immunity zeroes the hit after progress has been applied.
    if immune {
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

    // P1: Fall -> Env_FallDamage one-time immunity. The `kind` field is the
    // entity type, so use the canonical damage-type id "fall" for categorization.
    const FALL_DAMAGE_TYPE: &str = "fall";
    let category = categorize_with_tags(FALL_DAMAGE_TYPE, &[]);
    let env_concept = env_immunity_concept(category);
    let immune = crate::state_impl::with_player_state(player_uuid, |state| {
        env_concept.map_or(false, |c| concept_is_complete(state, c))
            || full_immunity_applies(
                |c| concept_is_complete(state, c),
                category,
                FALL_DAMAGE_TYPE,
            )
    });

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

    // Keep the mitigation/progress above intact; full immunity removes the hit.
    if immune {
        event.multiplier = 0.0;
        event.distance = 0.0;
    }
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

    // P1: Drown -> Env_Drowning one-time immunity (canonical damage type "drown").
    const DROWN_DAMAGE_TYPE: &str = "drown";
    let category = categorize_with_tags(DROWN_DAMAGE_TYPE, &[]);
    let env_concept = env_immunity_concept(category);
    let immune = crate::state_impl::with_player_state(player_uuid, |state| {
        env_concept.map_or(false, |c| concept_is_complete(state, c))
            || full_immunity_applies(
                |c| concept_is_complete(state, c),
                category,
                DROWN_DAMAGE_TYPE,
            )
    });

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

    // Keep the mitigation/progress above intact; full immunity removes the hit.
    if immune {
        event.damage_amount = 0.0;
    }
    Flow::Continue
}
