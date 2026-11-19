# AdaptionWheel system map: Java upstream → native port (Sunrise primary; Horizon archived)

_Zone: specs/prioritization (Agent #1). Docs-only reference; no code changes._
_Sources inspected 2026-11-18/19: `../adaptionwheel/src/main/java/ru/adaptionwheel/` (upstream Java, NeoForge 1.21.1), `native/adaptation/api/src/lib.rs` (pinned Horizon path, PR #12 @ `2f91bdc`), and PR #19 (Sunrise feasibility port, `native/adaptation_sunrise/`)._
_Method: source inspection (`rg`/`read`) plus PR/evidence reports from other agents. No new build/runtime was run for this document; all build/runtime claims below are attributed to their sources._
_**Primary target changed 2026-11-19 (user decision): Sunrise.** Reasons: officially signed SDK from `sunrise-org/sunrise` (`sdk-ed3ac19558d8dc1092c431312848cbb58fb01e41`, published 2026-11-19) and a two-line mechanical API port from the migrated core (#[mod_api::horizon_mod] → #[mod_api::sunrise_mod], `LevelRandomHost` → `HitHost`). Evidence: PR #19 `cargo check` clean + `cargo test` **81 passed / 0 failed** (rustc 1.99.1, `CARGO_TARGET_DIR=/mnt/Barracuda/tmp/cargo-target-sunrise-2`). Horizon PRs #15–#17 remain as archive/alternative; the Horizon SDK issue [#18184](https://github.com/horizon-rust/horizon/issues/18184) was closed as our own setup error and no longer blocks work. See `docs/SUNRISE_TARGET_STATUS.md`, §7 changelog and companion docs._

## 0) How to read this map

- **Upstream** = original Java mod in `../adaptionwheel`.
- **Port (primary since 2026-11-19)** = Sunrise native mod in `native/adaptation_sunrise/` (PR #19; ported from the migrated core in PR #17).
- **Port (archive/alternative)** = Horizon native mod in `native/adaptation/api/src/lib.rs` (PR #12 combined persistence #8 + console #11) and `native/adaptation_v2/` (PR #15–#17 Horizon new-API migration). Sections §§3–4 describe the shared implementation lineage; the mechanical differences for Sunrise are recorded in `docs/SUNRISE_TARGET_STATUS.md`.
- **Status** legend: `PORTED` (equivalent behavior in native), `PARTIAL` (subset/approximation), `STUB` (API/command exists but limited), `MISSING` (no code yet), `DEFERRED` (out of agreed MVP/MVP+ scope).
- Priorities: **P0** = blocks correct core behavior / merge-readiness; **P1** = next-iteration parity; **P2** = full parity/long tail.

---

## 1) Upstream data model and registries (what actually exists)

### 1.1 `data/PlayerAdaption.java` — the per-player state (250 lines)

Codec-persisted fields (`PlayerAdaption.CODEC`, lines 18–39):

| Field | Type | Meaning |
|---|---|---|
| `levels` | `Map<String,Integer>` | Level per concept (leveled adaptations); `MAX_LEVEL = 8` |
| `adapted` | `Set<String>` | One-time adaptations acquired |
| `tasks` | `List<AdaptionTask>` | In-flight analysis tasks (`concept`, `timer`, `maxTimer` → `progress()`) |
| `history` | `List<String>` | Recent concepts, capped `HISTORY_LIMIT = 30` |
| `existence` | `Set<String>` | Existence/boss adaptations |
| `killCounts` | `Map<String,Integer>` | Kills per `Drop_NPC_*` concept |
| `existenceProgress` | `Map<String,Integer>` | Boss combat ticks per boss path |
| `healingTimer`, `adversityCooldown`, `adversityTimer`, `adversityActive` | int/int/int/bool | Regen and Adversity state machine |
| `targetRotation`, `wheelRotation` | float | Wheel/spin state |
| `wasWearing`, `shedCount` | bool/int | Equip edge detection / shedding counter |
| `extras` (`Extras.CODEC`) | `disabled[]`, `cache[]`, `progress{}`, `instabreak`, `selectedAbility` | Player-owned choices/progress; deliberately persisted with the player, not the wheel item |

Key semantics (lines 143–248):
- `active(concept) = isEnabled(concept) && adapted.contains(concept)`; `levelOrZero()` returns 0 when toggled off.
- `getAdaptCount()` counts the **union** of `adapted` and positive `levels` (avoids double-counting keys present in both; comment at 175–188 records a 483-vs-503 HUD/menu mismatch this fixed). This count drives passive bonuses and several thresholds.
- `resetForUnequip()` preserves `disabled`, `cache`, `progress`, `instabreak`, `selectedAbility` — these belong to the player, not to the wheel stack (comment at 224–238: clearing them on unequip destroyed a 54-slot cache, all off-switches, fist counters and the stance).
- Transient (non-persisted) fields: `equippedStack`, `thermalHealingTimer`, `lastVoidRescueTick`, `recentlyShed`, `lastTierAnnounced`, `impactLastFallDistance`, `impactWasOnGround`.

### 1.2 `adapt/` — definition, domain, registry

- `AdaptationDefinition.java` (22 lines): record `(concept, domain, leveled, maxLevel)`; `oneTime()` → max 1, `leveled()` → `PlayerAdaption.MAX_LEVEL` (8).
- `AdaptationDomain.java` (48 lines): 11 domains — `DAMAGE, EFFECT, MOVEMENT, PHYSICS, MINING, ENVIRONMENT, COMBAT, PERCEPTION, ENTITY, EXISTENCE, SPECIAL` (each with key + UI color).
- `AdaptationRegistry.java` (141 lines): static `ConcurrentHashMap` of builtins. Groups:
  - MOVEMENT one-time: `Move_SoulSand`, `Move_Honey`, `Move_PowderSnow`, `Move_BerryBush`, `Move_BubbleColumn`.
  - MINING/COMBAT/PERCEPTION: `Mine_Labor` (leveled), `Combat_Cooldown` (leveled), `Combat_ShieldLock`, `Combat_SkillIssue`, `Percep_SteadyGaze`.
  - Fist ladders: 5 mining tiers (`FistTiers.TIER_COUNT=5`) + 5 combat-fist tiers (`CombatFistTiers.TIER_COUNT=5`), all leveled.
  - ENVIRONMENT one-time: `Env_Fire`, `Env_Lava`, `Env_Drowning`, `Env_Thorns`, `Env_Darkness`, `Env_Suffocate`, `Env_Void`, `Env_Starve`, `Env_Liquid`, `Env_Inventory`.
  - PHYSICS one-time: `Env_Ice`, `Env_Slime`, `Env_Cobweb`, `Env_Knockback`, `Env_FallDamage`.
  - DAMAGE leveled: one `Concepts.type(category)` per `AdaptionCategory`, excluding `FIRE` (retired `Type_FIRE` ladder; comment 57–64) and retired concepts.
  - SPECIAL: `Self_Damage` (leveled), `ADBERSITY`, `Mutation_Thermal`, `Mutation_Aquatic`, `Mutation_Impact`, `Mutation_SeaEye`, `Mutation_Flight`, `Mutation_Fist`, `Dimension_Destroy`.
  - `domainOf(String)` fallback classifies unknown concepts by prefix (`Type_`, `Debuff_`, `Move_`, `Mine_`, `Fist_`, `Combat_`, `Percep_`, `Contact_`/`Offense_NPC_`/`Drop_NPC_`, `Existence_`, `Env_` → PHYSICS for the 5 ids above).

### 1.3 `category/` — concept naming, damage categories, ladders, synergies

- `AdaptionCategory.java` (95 lines): 15 damage categories (`FALL, WITHER, FIRE, DROWN, FREEZE, LIGHTNING, EXPLOSION, PROJECTILE, MAGIC, MOB, PLAYER, CONTACT, STARVE, SUFFOCATE, VOID, OTHER` — 16 constants incl. OTHER; `COUNT = values().length`). `match(DamageSource)` uses vanilla damage-type tags (`is_fall`, `is_fire`, `is_drowning`, `is_freezing`, `is_explosion`, `is_projectile`, `is_wither`) + id fallbacks, cached in `MATCH_CACHE` (limit 512).
- `Concepts.java` (384 lines): prefixes (`Type_`, `Debuff_`, `Move_`, `Mine_`, `Fist_`, `Combat_`, `Percep_`, `Contact_`, `Offense_NPC_`, `Drop_NPC_`, `Existence_`, `Mutation_`, `Env_`, `Dimension_`), constants (e.g. `ENV_FALL = "Env_FallDamage"`, `ENV_FIRE`, `ENV_LAVA`, `ENV_DROWN`, `MUTATION_*`), `RETIRED_TYPES`, and helpers `type/contact/offense/drop/debuff/existence`, `isLevelBased`, `isOneTime`, `color`, `displayName`, `domain`.
- `FistTiers.java` (251 lines) / `CombatFistTiers.java` (65 lines): 5 material tiers (wood→netherite), per-tier block tags/tool classes, declared/vanilla mining speeds, luck multiplier, `canHarvest`, `reachTier`, `dealsExtraAttackDamage`.
- `Synergies.java` (184 lines): 10 combos with `Requirement.{PRESENT, MAXED, PRESENT_OR_FAMILY}`: `ashwalker, glacierblood, drownedwaltz, skybreaker, unmaker, unseen, gravebloom, goliath, stormcall, astralmine`. `satisfied(data, synergy)` is the single definition of "synergy active", shared by server effects and UI/advancements.

### 1.4 Passive progression and tasks

- `server/AdaptationProgression.java` (33 lines): `DEVELOPMENT_CAP = 400` adaptations; at cap `+0.80` movement speed (4× Speed-IV-equivalent modifiers) and `+0.49` jump strength (0.42 → 0.91, ~5-block jump). Linear `progress(count) = min(count,400)/400`.
- `data/AdaptionTask.java` (20 lines): persisted analysis task `{concept, timer, maxTimer}` with `progress = 1 - timer/maxTimer`.
- `AdaptionEvents.startTask/startOrAccelerate/completeTask/grantConceptLevel/grantConceptUpTo/grantToWheel` (lines 1882–1965) implement analysis timers, acceleration on further exposure, and level grants (`Concepts.isLevelBased` decides leveled vs one-time).

---

## 2) Upstream runtime flow — `server/AdaptionEvents.java` (2601 lines)

16 `@SubscribeEvent` handlers (line numbers from source):

| Handler | Event (NeoForge) | Behavior (abbreviated) |
|---|---|---|
| `onAttack` (437) | `LivingIncomingDamageEvent` | Pre-damage classification; boss/existence target detection (`adaptedExistenceTarget`), attack-side hooks |
| `onDamage` (489) | `LivingDamageEvent.Pre` | Main damage pipeline: category match → immunity/capstone/contact checks → leveled reduction (`data.levelOrZero`), defense heal ratio, i-frames, rapid-fall analysis, adversity survival (`tryAdversitySurvival`) |
| `onMobDeath` (903) | `LivingDeathEvent` | Kills/bookkeeping for `Drop_NPC_*`, award drop levels via `AdaptionConfig.dropLevelFromKills` |
| `onExperienceDrop` (924) | `LivingExperienceDropEvent` | XP bonus from drop level (`lootBonus`) |
| `onMobDrops` (955) | `LivingDropsEvent` | Extra loot rolls (fallback-preserving) |
| `onPlayerLogin` (1016) | `PlayerLoggedInEvent` | Load state, wheel-wear detection, apply passive modifiers |
| `onPlayerLogout` (1045) | `PlayerLoggedOutEvent` | Save state / cache cleanup |
| `onPlayerDeath` (1061) | `LivingDeathEvent` | Pending respawn health/armor, wheel-item wipe semantics |
| `onPlayerRespawn` (1094) | `PlayerRespawnEvent` | Restore pending health/armor |
| `onPlayerTick` (1103) | `PlayerTickEvent.Post` | Biggest driver: equip/unequip transitions, task ticking/completion, combo mutations, env exposure tasks (lava/liquid/drown/darkness/ice/slime/cobweb), boss combat tick accumulation & cinematics, debuff tasks, regen/Adversity timers, wheel rotation, passive stat upkeep, particle FX |
| `onServerStoppedResetAdaptableCache` (2215) / `onTagsUpdatedResetAdaptableCache` (2220) | lifecycle/tags | Invalidate `adaptableConcepts` cache |
| `onKnockback` (2315) | `LivingKnockBackEvent` | `Env_Knockback` behavior |
| `onBreakSpeed` (2334) | `PlayerEvent.BreakSpeed` | Fist-tier mining speed/harvest integration |
| `onEffectApplicable` (2380) | `MobEffectEvent.Applicable` | Debuff adaptation immunity (`Debuff_*`) |
| `onAnvilUpdate` (2496) | `AnvilUpdateEvent` | Wheel/item interaction (cost `WHEEL_GOLD_COST = 10`) |

Supporting helpers in the same file: env/contact/category immunity (`envImmunityConcept`, `hasFullImmunity`, `isMaxedImmunity`, `contactImmunityApplies`, `categoryCapstoneApplies`), void rescue (`rescueFromVoid`), save/load to wheel stack vs player attachment (`saveToStack`, `readFrom`, `loadFromItem`), existence accumulation (`accumulateBossCombat`, `noteBossHit`, `nearbyBossPaths`, `grantExistenceAdaptation`), mutations/FX (`grantComboMutation`, `tickFlight`, `tickThermalRegeneration`, `tickImpactStomp`, `spawn*Particles`), env effects (`applyEnvEffects`, `isOnIce`, `isStandingOnSlime`, `isInCobweb`), one-time grant/all-grant (`grantAllAdaptations`, `applyGrant`, `adaptableConcepts`, `reachesDimensionDestroy`), and `syncAdaption(player, data, wearing)` for client sync.

**Wheel equip model:** `wearingWheel()` (112–128) checks Curios (`CuriosApi.getCuriosInventory`) plus inventory/hand checks; `CombatFistTiers`/`FistTiers` convert adaptations into tool behavior via mixins (`FrictionMixin`, `PowderSnowBlockMixin`, `SlimeBlockMixin`, `WebBlockMixin`, `SpeedFactorMixin`, `AttackCooldownMixin`, etc. in the Java tree).

### 2.1 `server/AbilityDispatch.java` (104 lines) — single active-ability slot

- `HANDLERS: Map<String, Entry{activate, unlock}>`; registration open (`register(id, activate, unlock)`).
- `resolveSelected(data)` — use persisted `selectedAbility` if still unlocked, else first unlocked registered ability, else `null` (comment 52–72: protects old saves and switches that close their own gate).
- `select(player, id)` re-checks unlock **server-side** and immediately calls `syncAdaption` (1 Hz sync alone would leave the HUD stale for a second).
- `activate(player)` requires `isWearingWheel(player)`, re-resolves and re-checks before invoking handler.

### 2.2 `server/AdaptionCommand.java` (1192 lines)

Command root built by `onRegisterCommands` (572) via `buildRoot` (577). Subcommands observed in method signatures: `status, list [domain], info <concept>, grant <concept> [level], grantall [domain], shed <concept>, ungrant <concept>, analyze <concept>, reset, aggro, wheel, flight, altar <item>, registry`. Support: concept suggestions from `AdaptationRegistry.allDefinitions()` + dynamic families (`ConceptSuggestions`, `resolveConcept`, cap 80), entity/effect/domain suggestions, permission/self-or-target resolution (`self`, `selfOrNull`, `selfOrTarget`). Debug/diagnostic output includes attachment-vs-item divergence (`wheel`: `onlyOnItem`, `attachmentOnly`, lines 995–1048) and flight diagnostics (1049+).

### 2.3 Network / UI (context, not owned by this map)

`network/*Payload.java` includes `AdaptionSyncPayload`, `AbilityActivatePayload`, `AbilityCooldownPayload`, `CacheSyncPayload`, `FistProgressPayload`, `CombatFistProgressPayload`, `DiveImpactPayload`, `ExistenceCinematicPayload`, `FireSlashPayload`, `FistInstabreakPayload`, etc.; client side has `AdaptionHud`, `AbilityHud`, `AdaptationWheelScreen`, `AdversityOverlay`, `Cache*`, multiple custom renderers (`WheelRenderer`, `DharmaChakraModel`, OBJ meshes, shader FX). Rich UI/render parity is explicitly deferred in prior planning docs.

---

## 3) What the Horizon native port currently implements (lib.rs @ `2f91bdc`)

### 3.1 State

- `PlayerAdaptationStateV1` (schema v1): `wheel_active`, `active_source: WheelSource`, `entries: BTreeMap<String, AdaptationEntry{level, progress, completed}>`, runtime-only `dirty_persist`.
- 7 concepts only: `env_fall, type_explosion, env_fire, env_lava, type_general, env_drowning, type_contact` (`ALL_CONCEPTS`, 7 entries). Fall and explosion max levels 1 and 3; other five max 3.
- Thresholds/reductions are hard-coded per concept (`leveled_threshold`, `leveled_reduction`): e.g. explosion `30+l*10` with reduction `0/.10/.20/.35`; fire `20+l*10`, `0/.15/.30/.45`; drowning `10+l*6`, `0/.25/.50/.75`.
- Damage classification via lowercase string matching on `damage_type` (`is_explosion_damage_type`, `is_fire_damage_type`, `is_lava_damage_type`, `is_drowning_damage_type`, `is_contact_damage_type`, fallback `type_general`). Fall progress normally comes from `living_fall` distance/multiplier, threshold 40 with cap 20 per event.
- In-memory `Mutex<HashMap<u128, PlayerAdaptationStateV1>>` keyed by player UUID remains the runtime source of truth. PR #12 adds best-effort attachment persistence (`PLAYER_STATE_ATTACHMENT`, `adaptionwheel:player_state`, `Holder::Player`, `copy_on_death`, JSON-in-NBT `encode/decode_player_state_nbt`, load/save/sync/flush helpers) with the cache as fallback.

### 3.2 Hooks wired (10 in `impl Hooks for Mod`)

`register_commands`, `perform_command`, `perform_console_command` (API 99, PR #11/#12), `player_logged_in`, `player_clone`, `player_tick`, `player_inventory_changed` (41-slot scan → activation anywhere in inventory), `living_incoming_damage_v3` (victim UUID + mutable amount → classify/progress/mitigate), `living_fall`, `living_drown`. Entity-id→UUID map (`entity_player_map`, `refresh_entity_player_map_from_host`) supports hooks that only carry entity ids.

### 3.3 Commands (native)

Registered as OP level 2: `/adaptionwheel status|activate|reset|help`, `/adaptionwheel set <concept> <field:level|progress> <value>`, `/adaptionwheel grant <concept> <level>`. `normalize_concept` accepts snake_case aliases. Console variant (`perform_console_command`) supports `help`; player-context subcommands return explicit guidance. All mutating commands force a persistence flush.

### 3.4 Evidence attached by other agents (not re-run here)

- **Sunrise (primary, 2026-11-19) — PR #19:** `cargo check` clean, `cargo test` **81 passed / 0 failed** against the signed `sunrise-org/sunrise` SDK `sdk-ed3ac19558d8dc1092c431312848cbb58fb01e41` (rustc 1.99.1, `CARGO_TARGET_DIR=/mnt/Barracuda/tmp/cargo-target-sunrise-2`); only two mechanical source edits from PR #17. No runtime claimed. See `docs/SUNRISE_TARGET_STATUS.md`.
- Native unit tests (Horizon archive, PR #12 combined): `cargo check -q` PASS, `cargo test -q` **10 passed / 0 failed**, `CARGO_TARGET_DIR=/mnt/Barracuda/tmp/cargo-target-combo`, integration worktree (Linux, cargo/rustc 1.99.1). Tests: `state_defaults_and_summary`, `activation_detects_wheel_anywhere_in_inventory`, `fall_progress_and_mitigation`, `fall_requires_activation`, `explosion_progress_levels_up`, `explosion_damage_classifier`, `damage_type_classification_for_new_concepts`, `leveled_concepts_progress_with_activation`, `commands_set_and_grant_new_concepts`, `damage_adaptation_applies_mitigation_and_progress`. #11 (`cac5cf86`) and #8 (`4a0d66b`) each also report 10/10; PR #12 supersedes both (see evidence register).
- Content (PR #7): `cargo-horizon-mod build` + `check --json ok:true`, hmod sha256 `5c86ca5fe759f4706aacae4a22bab9b2e628e4b492aca84b19cccf8e94a2dc11`, size 975453; later runtime artifacts reference hmod sha `3cee70ad…` (issue #10) — revalidate the final artifact.
- **Runtime (partial, CONFIRMED per issue #10 + `native/adaptation/README.md`):** PR #12 native built/installed via `horizon-launcher native-build ... --instance adaptionwheel-test --ignore-pins --trust-native-code` as `native-9f42064-67f92751b91f` (~46 s, log `/mnt/Barracuda/tmp/native-build-pr12.log`, `HORIZON_HOME` set to instance home), driven headlessly by `horizon-ctl`: all six `/adaptionwheel` commands respond, inventory-anywhere activation works, state survives quit/relaunch, lava damage progresses `env_lava` to level 1, and death/clone preserves levels. Caveats: creative mode limits mitigation checks; console/sync paths, the full scenario matrix, and revalidation on the final hmod remain pending. Earlier partial check with old native (`199210fc`) showed content/item load only — superseded for native behavior by the PR #12 build.
- **Not yet confirmed:** full in-game matrix (fall/explosion/fire/drown/contact/general mitigation), mod-net sync/UI, console runtime beyond help, content render/formats on final artifact, CI checks.

---

## 4) Parity matrix — upstream → port

| Upstream subsystem | Status | Port location / concrete gap |
|---|---|---|
| Per-player state codec (levels, adapted, tasks, history, existence, kills, boss ticks, timers, Extras) | **PARTIAL** | Native state has 7 concepts × `{level,progress,completed}` + activation; no tasks/history/existence/killCounts/bossCombatTicks/Extras/timers |
| Concept registry (≈100+ ids across 11 domains, leveled/one-time metadata) | **STUB** | Native has 7 hard-coded ids and no domain metadata; no `AdaptationRegistry`/`AdaptationDefinition` equivalent |
| Damage category matching (16 categories, vanilla damage-type tags + cache) | **PARTIAL** | Native uses string substring matching; no tag-based `AdaptionCategory.match`, no cache, no 16-category coverage (wither/freeze/lightning/projectile/magic/mob/player/starve/suffocate/void/other missing) |
| Damage mitigation/reduction + leveled progression | **PARTIAL** | Native leveled reduction/progress exist for 7 concepts; upstream also has defense heal ratio, i-frames, capstones, contact immunity, adversity survival, reflect |
| Fall adaptation + mitigation | **PARTIAL** | `living_fall` ported (threshold 40, cap 20); upstream `Env_FallDamage` is one-time with rapid-fall analysis and armor handling |
| Movement/environment tasks (lava, liquid, drown, darkness, ice, slime, cobweb, berry bush, bubble column, powder snow, soul sand) | **MISSING** | No `startTask`/`startOrAccelerate` or per-surface detection in native; related Horizon API gap draft exists (docs/issues) |
| Existence/boss system (proximity, combat ticks, cinematics, grants) | **MISSING** | No boss caches, `grantExistenceAdaptation`, `Existence_*` concepts |
| Mutations (thermal/aquatic/impact/sea-eye/flight/fist) + combos | **MISSING** | No combo mutation logic; only damage concepts |
| Synergies (10 combos) | **MISSING** | `Synergies.java` has no native equivalent |
| Fist ladders (5 mining + 5 combat tiers, tags, harvest/speed) | **MISSING** | No tier tables, no integration with mining/attack hooks |
| Passive progression (400-cap speed + jump bonuses) | **MISSING** | `AdaptationProgression.java` not ported; no attribute modifiers |
| Adversity / shedding / void rescue / regen timers | **MISSING** | No state fields or handlers |
| Cache (54-slot), off-switches, instabreak stance, selected ability | **MISSING** | `Extras` semantics absent; note `resetForUnequip` preservation rule has no native analogue |
| Active ability dispatch (single persisted slot + unlock re-check + immediate sync) | **MISSING** | `/adaptionwheel activate` only reports cached activation; no `register/select/activate`, no active abilities |
| Commands | **PARTIAL** | 6 native OP commands + console help vs upstream ~14 subcommands with suggestions, targets, domain filters, diagnostics, altar/registry |
| Wheel equip model (Curios slot, wear detection, stack-vs-attachment state) | **PARTIAL** | Inventory-anywhere scan replaces Curios; no dedicated slot, no `saveToStack`/`readFrom` split |
| Persistence / lifecycle | **PARTIAL** | Attachment fallback + clone/login/tick flush; runtime semantics (save timing, respawn/dimension, payload/versioning) unverified; upstream has explicit save/load on login/logout/death/respawn |
| Client sync / UI / FX | **MISSING** | No `AdaptionSyncPayload`, cooldowns, HUD, wheel screen, overlays, custom models/shaders (deferred by design) |

---

## 5) Prioritized gaps for the next iteration

### P0 — blocks 100% merge-readiness (primary target: Sunrise)
0. **Sunrise is the primary target (user decision, 2026-11-19).** PR #19 proves the feasibility port with two mechanical API edits and signed-SDK evidence (`cargo check` clean, `cargo test` **81/81**). Next: `cargo sunrise-mod build` / Sunrise launcher `native-build` with the installed Sunrise launcher `1.1.1` (`/mnt/Barracuda/tmp/opencode/sunrise-launcher/sunrise-launcher`, profile `sunrise`, nightly `nightly-2026-11-19-ed3ac19` install+prepare done), then `sunrise-ctl` in-game validation (commands, activation, persistence, target selectors). No runtime claim for Sunrise until that evidence exists. Horizon PRs #15–#17 are archive/alternative only.
1. **Complete the runtime matrix on the final installed stack** (PR #12 native `2f91bdc`/`native-9f42064-67f92751b91f` + final content hmod): already confirmed — six commands, inventory activation, relaunch persistence, lava progression, death/clone level preservation. Still required — mitigation checks outside creative mode for fall/explosion/fire/drown/contact/general, console behavior beyond help, mod-net sync/UI feedback, content rendering/format checks on the final hmod, and reproducible commands/logs/SHAs per case.
2. **Attachment semantics closure** (save/flush timing, `copy_on_death`, player payload limits, schema v1 migration): relaunch and clone/death are now evidenced (issue #10), but survival-mode/cross-dimension/server-restart behavior and limits remain unverified; keep the in-memory fallback explicitly documented until closed.
3. **Integration revalidation of the composed stack on final heads** (proposed order #4→#5→#7→**#12 instead of #8/#11**→#9→#6→docs #13/#14 for the archived Horizon path; Sunrise path: PR #17 → #19 → runtime), clean rebuild + combined `cargo check/test` + content build/check + runtime re-run, plus **CI checks** (currently none). The former Horizon SDK blocker [#18184](https://github.com/horizon-rust/horizon/issues/18184) was **closed as our own setup error** and no longer blocks the work — see `docs/API_GAP_BLOCKER_CHECK.md`. Remaining adaptation-logic gaps still require individual evidence before filing.

### P1 — next implementation iteration (parity of core mechanics)
1. **Concept model generalization**: replace 7 hard-coded constants with a registry carrying domain, leveled/one-time, max level and display id; use upstream ids from `Concepts`/`AdaptationRegistry` as the contract (at minimum add the 15 damage categories + 10 environment + 5 movement + 5 physics ids).
2. **Damage category matching by Horizon damage-type tags/ids**, mirroring `AdaptionCategory.match` (fallback to id matching), and richer mitigation/capstone/contact logic (defense heal, i-frames, adversity survival) once payload access is confirmed.
3. **Task/analysis system**: port `AdaptionTask` + `startTask`/`startOrAccelerate`/`completeTask` and the surface-exposure detection in `onPlayerTick` (lava/liquid/drown/darkness/ice/slime/cobweb/berry/bubble/powder-snow/soul-sand) — validate whether Horizon `player_tick` + block queries suffice; related conditional API-gap drafts are in `docs/issues/`.
4. **State model completion**: add tasks/history/existence/killCounts/bossCombatTicks/timers and `Extras` (disabled/cache/progress/instabreak/selectedAbility), plus union-based adapt-count and `resetForUnequip` preservation semantics; bump schema version with migration.
5. **Active abilities**: port `AbilityDispatch` (register/select/activate, unlock re-check, immediate sync) behind a single universal activation key, before adding more abilities.
6. **Commands**: add `list/info/grantall/shed/ungrant/analyze/wheel/flight/registry` with concept suggestions and target resolution (at least the diagnostic set needed to validate state in a live instance).

### P2 — long-tail parity (scope explicitly deferred unless reprioritized)
- Existence/boss system + cinematics; mutations and 10 synergies; fist ladders + attribute modifiers (400-cap speed/jump); Curios-like accessory slot; cache/altar/menus; client HUD/wheel UI/overlays and custom render/FX (OBJ/shader pipeline unlikely to be portable as-is).

## 6) Verification checklist for this document

- [x] Every upstream claim in §§1–2 tied to an inspected file/class/method (line references where especially load-bearing).
- [x] Port claims in §3 tied to `native/adaptation/api/src/lib.rs` symbols; test names/evidence attributed to agent reports, not re-run here.
- [x] Parity matrix and gaps distinguish `PORTED/PARTIAL/STUB/MISSING/DEFERRED` and avoid claiming unverified runtime behavior.
- [x] (2026-11-18) Runtime facts refreshed from issue #10 + `native/adaptation/README.md`; partial validation is now distinguished from the still-missing full matrix.
- [ ] Re-check after final merge-order decisions and the complete runtime matrix/CI; update §3.4/§4/§5 then (owner: Agent #1).

## 7) Changelog

- **2026-11-19** — user decision: primary target **Sunrise** (officially signed SDK `sdk-ed3ac19558d8dc1092c431312848cbb58fb01e41`; only two mechanical API edits from the migrated core; PR #19 `cargo test` 81/81). Horizon path (#15–#17, plus old #4–#12 stack) archived as alternative; Horizon issue #18184 closed as our own error. Added `docs/SUNRISE_TARGET_STATUS.md`; this map, readiness register, blocker check and work plan updated accordingly. Sunrise runtime validation still pending.
- **2026-11-18** — reconciled to combined native PR #12 `2f91bdc` (base `#5@fc581fff`): `perform_console_command` API 99, 41-slot `player_inventory_changed`, attachment `adaptionwheel:player_state` (JSON-in-NBT, `copy_on_death`, load/sync/flush), `living_incoming_damage_v3`/`living_fall`/`living_drown`. Refreshed PR stack (#4 `cf987c0`, #5 `fc581fff`, #6 `3a715bb`, #7 `3f044c5`, #8 `4a0d66b`, #9 `9ce3432`, #11 `cac5cf86`, #12 `2f91bdc`, #13 `c0b0569`, #14 `f2db6f3`) and runtime status (partial confirmation per issue #10; full matrix/CI still open). Upstream Horizon `main` mod API is being restructured (`horizon-mod-vocab`/`horizon-mod-api-gen`, attachments in `horizon-mod-runtime`, `#[mod_api::horizon_mod]`/`Registrar`) — tracked as a separate migration; see `HORIZON_NATIVE_FORMAT_GUIDE.md`.

## 8) Companion docs (Ensemble docs/readiness integration)

- `docs/READINESS_EVIDENCE_REGISTER.md` — per-PR confirmed vs claimed evidence and remaining gates (S2).
- `docs/API_GAP_BLOCKER_CHECK.md` — Horizon SDK issue #18184 closed as our own setup error; other candidate gaps remain hypotheses.
- `docs/MIGRATION_WORK_PLAN.md` — prioritized P0/P1/P2 work plan with owners/evidence gates (S5), now Sunrise-first.
- `docs/SUNRISE_TARGET_STATUS.md` — **primary target** summary: reasons, PR #19, 81/81 evidence, two API edits, next runtime gates.
- `docs/HORIZON_NATIVE_FORMAT_GUIDE.md` — archived/alternative Horizon native packaging guide (S4).
