# AdaptionWheel Horizon — Scenario Matrix (S2)

Status: **DRAFT — expected values derived from source; selected items runtime-confirmed by orchestrator (2026-11-18, issue #10 comment 6971913940).**
Most expected numbers below are derived from the PR #12 head implementation (source of truth). Items verified live are marked **[CONFIRMED-RUNTIME]**; anything still not derivable from code or not yet measured is tagged **[UNVERIFIED]**.

### Runtime confirmations applied by lead (orchestrator, 2026-11-18, issue #10 comment 6971913940)
- **`/damage` availability — [CONFIRMED-RUNTIME]:** in instance `adaptionwheel-test`, `/damage @s 1 minecraft:generic` → `Applied 1.0 damage`. The command EXISTS and WORKS. The former §5 item 2 tag "`/damage` availability [UNVERIFIED]" is hereby lifted; prefer `/damage` for exact-number evidence.
- **lava progression — [CONFIRMED-RUNTIME]:** `/damage @s 10 minecraft:lava` ×5 drove `env_lava` from `lvl=0 prog=0/24` to `lvl=1 prog=22/36`. This confirms: lava classifier, activation gate, per-event progress accrual, and the L0→L1 threshold crossing at 24 with carry-over 22 into L1 (threshold 36), exactly matching the code-derived model (§1 rule 4, §3 lava row). The former §4.4 "[UNVERIFIED: per-tick lava damage]" tag is lifted for the `/damage`-driven route.
- **Remaining limitation:** the instance is creative; some damage types are sometimes silently suppressed (`/damage @s 4 minecraft:lava` → `Target is invulnerable to the given damage type` in a post-respawn session). Organic per-level mitigation magnitude remains **[UNVERIFIED]** — mitigation must be re-tested in a session where the damage type is accepted (survival world or a working `/damage` type).

## 0. Provenance & environment

| Item | Value |
|---|---|
| Code under test | PR #12 `feat/native-combined-8-11`, head `2f91bdca72eb88164a7fc011a6cc829cd4a03986`, base PR #5 `fc581fff` |
| Authoritative file | `native/adaptation/api/src/lib.rs` |
| lib.rs SHA256 | `0e234551a124be5a401ce38ee0679767ed83c425d32e702587e4ddbe2f5d39ae` |
| Companion spec | `docs/COMMAND_SYSTEM_SPEC.md`, SHA256 `26ab9379342fbfa832e0e2e9683acd7e98e495f1db1e0d9ae87f6ac0b3207741` |
| Spec's runtime env (from §6) | Linux; `horizon-ctl` `nightly-2026-11-13-9f42064`; native build `native-9f42064-67f92751b91f` (`--headless --control-socket`); instance `adaptionwheel-test`, world `New World` (creative); content hmod sha256 `5c86ca5fe759f4706aacae4a22bab9b2e628e4b492aca84b19cccf8e94a2dc11` (PR #7) |
| **Required env for every command below** | `export HORIZON_HOME=/home/alexquasar/.local/share/horizon/instances/adaptionwheel-test` (verified path exists). Without it `horizon-ctl` targets the default home where `adaptionwheel` is NOT enabled/registered → "Unknown or incomplete command" (spec §6.3). |
| Temp root | `export TMPDIR=/mnt/Barracuda/tmp TEMP=/mnt/Barracuda/tmp TMP=/mnt/Barracuda/tmp` |
| Scratch (this deliverable) | `/mnt/Barracuda/tmp/opencode/qa-s2-scenarios/` |

### horizon-ctl syntax used (from `horizon-ctl --help` on this host)

- `horizon-ctl command <TEXT>` — run a vanilla/mod MC command in **player context** (leading `/` optional, `@s` selectors work). Example: `horizon-ctl command /give @s compass`.
- `horizon-ctl give ITEM [COUNT]`, `horizon-ctl setblock X Y Z BLOCK[props]`, `horizon-ctl summon KIND [X Y Z] [baby]`, `horizon-ctl effect ID SECONDS [AMPLIFIER]`, `horizon-ctl tp X Y Z`, `horizon-ctl wait TICKS` (20/s), `horizon-ctl key ...`, `horizon-ctl move ...`, `horizon-ctl jump`, `horizon-ctl release`.
- Observe: `horizon-ctl status`, `horizon-ctl events [--since SEQ] [--limit N]` (includes **hurt with cause**, deaths, respawns, chat), `horizon-ctl chat-log [N]`, `horizon-ctl messages [N]`, `horizon-ctl state`, `horizon-ctl inventory`, `horizon-ctl looking-at`, `horizon-ctl block X Y Z`, `horizon-ctl logs`.
- `horizon-ctl hello`, `horizon-ctl help --game` (lists the running game's own commands/test hooks — use to confirm the `/damage` command is present before relying on it, see §5).

## 1. Core rules derived from code (apply to ALL concepts)

1. **Activation gate:** every `apply_*` progress function returns `false` immediately when `state.wheel_active == false` (lib.rs:455, 500). No progress accrues without a wheel item present in inventory.
   - Wheel IDs recognized (lib.rs:54-68): `adaptionwheel:adaptation_wheel`, `adaptionwheel:mahoraga_wheel`, `adaptionwheel:mahoraga_wheel_wood`, `minecraft:compass` (runtime fallback), and any id ending `:adaptation_wheel`.
   - Activation is set by `player_inventory_changed` scanning 41 slots (hotbar 0-8 → `Hotbar`, inventory 9-35 → `Inventory`, armor 36-39 → `Armor`, offhand 40 → `OffHand`). Confirmed on chat: `[adaptionwheel] wheel activation: true (source: ..., anywhere-in-inventory scan).`
   - Practical activation: `horizon-ctl give compass` (vanilla fallback always obtainable). Confirm with `horizon-ctl command adaptionwheel status` → `wheel_active=true`.
2. **Mitigation is NOT gated by activation:** `apply_damage_adaptation_for_player` multiplies `amount` by the mitigation factor before the progress match, independent of `wheel_active` (lib.rs:545-549). A level set by `grant`/`set` while wheel is inactive still mitigates. Progress, however, stays 0 until activation. **[UNVERIFIED at runtime — derived from code.]**
3. **Progress increment rule (leveled concepts):** `progress += clamp(post_mitigation_damage, 0.5, gain_cap)` (lib.rs:465). Minimum gain 0.5/event; maximum = per-concept cap. Damage is mitigated *first*, so gains shrink as level rises.
4. **Level-up rule:** while `progress >= leveled_threshold(concept, level)` and `level < max_level`: `progress -= threshold`, `level += 1`. Reaching `max_level` sets `completed=true`, `progress=0` (lib.rs:467-480). Further events return early (no change).
5. **Thresholds by level** `leveled_threshold` (lib.rs:355-366), `l = level`:
   - explosion `30 + 10l` → L0 30, L1 40, L2 50
   - fire `20 + 10l` → L0 20, L1 30, L2 40
   - lava `24 + 12l` → L0 24, L1 36, L2 48
   - general `30 + 15l` → L0 30, L1 45, L2 60
   - drowning `10 + 6l` → L0 10, L1 16, L2 22
   - contact `12 + 6l` → L0 12, L1 18, L2 24
   - fall (default arm) `20 + 10l`, but fall uses its own fixed `FALL_THRESHOLD=40`.
6. **Mitigation factor** `leveled_reduction` (lib.rs:368-378) → factor `1 - reduction`, clamped:
   - explosion L0 0%, L1 10%, L2 20%, L3 35%
   - fire L0 0%, L1 15%, L2 30%, L3 45%
   - lava L0 0%, L1 20%, L2 35%, L3 50%
   - general L0 0%, L1 10%, L2 20%, L3 30%
   - drowning L0 0%, L1 25%, L2 50%, L3 75%
   - contact L0 0%, L1 20%, L2 35%, L3 50%
   - fall: `fall_mitigation_factor` = 0.0 once completed/level≥1 (full immunity), else 1.0.
7. **Status rendering** `format_status` (lib.rs:579-600), order = `env_fall, type_explosion, env_fire, env_lava, type_general, env_drowning, type_contact`:
   `wheel_active=<bool> source=<Source> schema=1 | <concept>: lvl=<n>/<max>[ prog=<f>/<t>] done=<bool> | ...`
   - `prog=` is printed **only** when `level < max && !completed`; fall shows `prog=<f>/40`; completed entries omit `prog=`.
   - Login chat uses `summary()` (lib.rs:137-158) which is a *different, shorter* format: `wheel=... (...) | fall lvl=.. prog=../40 done=.. | explosion lvl=../3 prog=.. done=.. | fire lvl=.. | lava lvl=.. | general lvl=.. | drowning lvl=.. | contact lvl=..`. Do not confuse the two.
8. **Command access:** `/adaptionwheel status|activate|set|grant|reset|help` are permission level 2, **player context only**. Dedicated console only answers `help` (spec §4). Use `horizon-ctl command adaptionwheel <sub>` (runs as player). All replies prefixed `[adaptionwheel]`.
9. Deterministic helpers for threshold/completion verification (these bypass damage classification, use as cross-check, not as proof of classifier): `horizon-ctl command adaptionwheel grant <concept> <level>`, `horizon-ctl command adaptionwheel set <concept> progress <value>`, `horizon-ctl command adaptionwheel reset`.

## 2. Master matrix (expected values from code)

Increments below assume the **max gain per event** (cap reached) and ignore mitigation growth; real organic runs need more events because mitigation shrinks each gain. See per-concept detail for mitigation-adjusted sequences.

| Concept | Canonical / aliases | Trigger & API hook | Per-event progress increment (cap) | Thresholds L0/L1/L2 | Max lvl | Completion state | Mitigation by lvl | Pass / Fail criteria |
|---|---|---|---|---|---|---|---|---|
| Fall | `env_fall` / `fall` | `LivingFallEvent` → `Hooks::living_fall` (primary); damage type containing `fall` → `Hooks::living_incoming_damage_v3` (fallback) | `gain = clamp(max(distance, multiplier*2), 1.0, 20.0)`; `progress += gain`, capped at 40. Two ≥20-block falls complete. | single threshold 40 (fixed) | **1** (binary) | `lvl=1/1 done=true`, `prog` frozen at 40 (hidden in status) | L0 factor 1.0 → L1 factor 0.0 (full immunity) | **Pass:** with wheel active, each fall adds clamped gain to `prog`; cumulative ≥40 flips to `lvl=1/1 done=true`; later falls add nothing and `hurt` damage becomes 0. **Fail:** no progress with wheel active; wrong threshold (≠40); progress without activation; lvl ever >1. |
| Explosion | `type_explosion` / `explosion` | damage type contains `explosion`/`blast`/`fireworks`/`bad_respawn_point` → `Hooks::living_incoming_damage_v3` | `clamp(post-mitigation dmg, 0.5, 20.0)` | 30 / 40 / 50 | 3 | `lvl=3/3 done=true`, `prog=0` | 0/10/20/35% | **Pass:** increments match clamp; levels advance at 30/40/50; completes at L3; explosion `hurt` amount reduced per L1/L2 reduction; no other concept moves. **Fail:** no progress; wrong thresholds; general concept also increments (misclassification); damage not reduced. |
| Fire | `env_fire` / `fire` | damage type `fire`/`in_fire`/`on_fire`/`hot_floor`/`campfire`/`fireball`/`unattributed_fireball` (or `:fire` etc.; contains `fire` & not `firework`) → `Hooks::living_incoming_damage_v3` | `clamp(post-mitigation dmg, 0.5, 12.0)` | 20 / 30 / 40 | 3 | `lvl=3/3 done=true`, `prog=0` | 0/15/30/45% | **Pass:** fire-source `hurt` events add ≤12 each; thresholds 20/30/40; completes L3; damage falls with level. **Fail:** lava or explosion events increment fire (classifier order broken); no progress; wrong thresholds. |
| Lava | `env_lava` / `lava` | damage type `lava`/`:lava`/contains `lava` → `Hooks::living_incoming_damage_v3` | `clamp(post-mitigation dmg, 0.5, 12.0)` | 24 / 36 / 48 | 3 | `lvl=3/3 done=true`, `prog=0` | 0/20/35/50% | **Pass:** lava contact adds ≤12/event; thresholds 24/36/48; completes L3; damage reduced. **Fail:** fire classifier captures lava (`is_lava` checked before `is_fire` in code — verify this order); no progress. |
| General | `type_general` / `general`, `generic`, `damage` | any damage type not matched by explosion/fall/lava/fire/drowning/contact (e.g. `mob_attack`, `arrow`) → `Hooks::living_incoming_damage_v3` | `clamp(post-mitigation dmg, 0.5, 15.0)` | 30 / 45 / 60 | 3 | `lvl=3/3 done=true`, `prog=0` | 0/10/20/30% | **Pass:** unclassified damage (mob attack) increments general only; thresholds 30/45/60; completes L3; damage reduced. **Fail:** classified damage increments general; no progress; wrong thresholds. |
| Drowning | `env_drowning` / `drowning`, `drown` | damage type `drown`/`:drown`/contains `drown` → `Hooks::living_incoming_damage_v3`; **plus** `LivingDrownEvent` → `Hooks::living_drown` (dual hook) | `clamp(post-mitigation dmg, 0.5, 8.0)` in either hook; drown hook uses `gain = damage_amount` if >0 else **1.0** when `is_drowning` | 10 / 16 / 22 | 3 | `lvl=3/3 done=true`, `prog=0` | 0/25/50/75% | **Pass:** drowning events add ≤8 (or baseline 1.0) per event; thresholds 10/16/22; completes L3; damage reduced. **Fail:** no progress; progress attributed to wrong concept; thresholds off. Note dual-hook double counting possible — see §5. |
| Contact | `type_contact` / `contact`, `cactus` | damage type `cactus`/`sweet_berry_bush`/`stalagmite`/`contact` (or namespaced) → `Hooks::living_incoming_damage_v3` | `clamp(post-mitigation dmg, 0.5, 10.0)` | 12 / 18 / 24 | 3 | `lvl=3/3 done=true`, `prog=0` | 0/20/35/50% | **Pass:** cactus/berry/stalagmite `hurt` events add ≤10; thresholds 12/18/24; completes L3; damage reduced. **Fail:** no progress; wrong thresholds; fire/general cross-increments. |

### Level-sequence examples (max-gain events, mitigation ignored)

| Concept | cap | Event-by-event progress → level transitions |
|---|---|---|
| env_fall | 20 | 20 → 40 ⇒ **complete** (2 events) |
| type_explosion | 20 | 20 → 40⇒L1/P10 → 30 → 50⇒L2/P10 → 30 → 50⇒**L3 complete** (6) |
| env_fire | 12 | 12 → 24⇒L1/P4 → 16 → 28 → 40⇒L2/P10 → 22 → 34 → 46⇒**L3** (8) |
| env_lava | 12 | 12 → 24⇒L1/P0 → 12 → 24 → 36⇒L2/P0 → 12 → 24 → 36 → 48⇒**L3** (9) |
| type_general | 15 | 15 → 30⇒L1/P0 → 15 → 30 → 45⇒L2/P0 → 15 → 30 → 45 → 60⇒**L3** (9) |
| env_drowning | 8 | 8 → 16⇒L1/P6 → 14 → 22⇒L2/P6 → 14 → 22⇒**L3** (6) |
| type_contact | 10 | 10 → 20⇒L1/P8 → 18⇒L2/P0 → 10 → 20 → 30⇒**L3** (6) |

Mitigation-adjusted counts are higher: e.g. explosion post-L1 gain ≈18 (factor .9), post-L2 ≈16 (factor .8); fire ≈10.2/.7×12; lava ≈9.6/.8×12; drowning post-L1 ≈6 (factor .75), post-L2 ≈4 (factor .5); contact post-L1 ≈8, post-L2 ≈6.5. **[Derived; not runtime-verified.]**

## 3. Common reproduction setup (run once per scenario)

```sh
export TMPDIR=/mnt/Barracuda/tmp TEMP=/mnt/Barracuda/tmp TMP=/mnt/Barracuda/tmp
export HORIZON_HOME=/home/alexquasar/.local/share/horizon/instances/adaptionwheel-test
# (server assumed already running/launched per S1 plan; attach with:)
horizon-ctl wait-ready
# 1) install wheel (vanilla fallback item activates the mod)
horizon-ctl give compass
horizon-ctl command adaptionwheel status      # expect wheel_active=true source=Inventory|Hotbar schema=1
# 2) clean baseline for the concept under test
horizon-ctl command adaptionwheel reset
horizon-ctl command adaptionwheel status      # expect all lvl=0/... done=false, prog=0
# 3) start an event tail (capture SEQ from reply)
horizon-ctl events --limit 5
```

Record `horizon-ctl hello`, `horizon-ctl status`, `horizon-ctl state` (gamemode/position/health), `horizon-ctl inventory` at baseline. **Note:** the spec's instance world is creative; creative mode may suppress incoming damage — see §5 item 1. If organic damage does not register, switch world to survival or use the `/damage` route (§5 item 2); mark whichever route was used.

## 4. Per-concept reproduction steps

### 4.1 env_fall (`fall`)
1. Setup §3, wheel active, reset.
2. `horizon-ctl state` → note current Y (ground).
3. Fall #1: `horizon-ctl command tp @s ~ <groundY+25> ~` then `horizon-ctl wait 80` (~4 s) for landing.
4. `horizon-ctl command adaptionwheel status` → expect `env_fall: lvl=0/1 prog=20.00/40 done=false` **[derived: 25-block fall clamped to 20]**. `horizon-ctl events --since <seq>` → expect a `hurt` with cause `fall` (creative may omit damage — still check status).
5. Fall #2: repeat teleport to `<groundY+25>` and wait.
6. `status` → expect `env_fall: lvl=1/1 done=true` (prog hidden). `events` shows cumulative `fall` hurts.
7. Mitigation check: fall #3 from same height → `events` hurt amount for fall should be 0 / full immunity; `status` unchanged. **[UNVERIFIED: creative may already zero damage; use survival or /damage for a clean before/after.]**
8. Cross-check: `horizon-ctl command adaptionwheel grant env_fall 1` on a fresh reset → expect `lvl=1/1 done=true` immediately; `grant env_fall 0` → `lvl=0/1 prog=0.00/40 done=false`.
9. Negative control: `reset`, remove wheel (`horizon-ctl command ...` drop/clear — or use a fresh no-wheel session), take a fall → `status` prog stays 0.

### 4.2 type_explosion (`explosion`)
1. Setup §3, wheel active, reset.
2. `horizon-ctl summon creeper` (spawns in front of player) and `horizon-ctl wait 100`; or deterministic `horizon-ctl command summon tnt ~ ~1 ~` + `wait 60`. Confirm `events` `hurt` cause contains `explosion`.
3. After each blast: `status` → expect `type_explosion: lvl=0/3 prog=20.00/30 done=false` (first hit; damage-dependent, may be <20 if farther away). **[UNVERIFIED: actual explosion damage amount; use `/damage @s 20 minecraft:explosion` if available for exact numbers.]**
4. Repeat blasts, checking `prog` increments ≤20 and thresholds 30/40/50; expect `lvl=1/3 prog≈10/40`, then `lvl=2/3 prog≈10/50`, then `lvl=3/3 done=true`.
5. Verify other concepts remain `lvl=0/... done=false` (classifier isolation).
6. Mitigation: compare `events` hurt amount for identical-size blasts before L1 and after L2 (expect ×0.9 then ×0.8). **[Creative caveat.]**
7. Cross-check: `grant type_explosion 3` → `lvl=3/3 done=true prog=0`; `set type_explosion progress 25` at L0 → `prog=25.00/30`.
8. Negative control with wheel inactive → no progress despite blasts.

### 4.3 env_fire (`fire`)
1. Setup §3, wheel active, reset.
2. Fire source: `horizon-ctl setblock <x> <y> <z> fire` under/around player and stand in it (`horizon-ctl command setblock ~ ~ ~ fire` is **invalid** — fire must be placed on a solid block; place `stone` then `fire` above, or stand on `netherrack` lit). Alternative: `horizon-ctl command setblock ~ ~-1 ~ magma_block` → `hot_floor` damage when standing. Or place `campfire`. Confirm `events` hurt cause = `in_fire` / `hot_floor` / `campfire`.
3. After each damage tick (`horizon-ctl wait 20` between samples), `status` → `env_fire: lvl=0/3 prog<=12.00/20 done=false`; increments ≤12. **[UNVERIFIED: exact per-tick fire damage.]**
4. Continue through thresholds 20/30/40 → `lvl=1/3 prog=4/30`, `lvl=2/3 prog=10/40`, `lvl=3/3 done=true`.
5. Verify explosion/lava concepts stay at 0 (fire classifier excludes `fireworks`; lava classified first).
6. Mitigation check L1/L2/L3: 15/30/45% damage reduction on identical fire exposure.
7. Cross-check: `grant env_fire 2`, `set env_fire progress 15`.
8. Negative control: wheel inactive → no progress.

### 4.4 env_lava (`lava`)
1. Setup §3, wheel active, reset.
2. `horizon-ctl command setblock ~ ~-1 ~ lava` (replace block under player) then step in / `horizon-ctl move forward 200`. Or place a lava pool 2 blocks ahead and `horizon-ctl goto` into it. Confirm `events` hurt cause = `lava`.
3. Sample `status` after each hurt tick (`wait 20`): `env_lava: lvl=0/3 prog<=12.00/24 done=false`, increments ≤12. **[UNVERIFIED: per-tick lava damage ~4 HP ⇒ gain ~4, so ~6 ticks to first level — measure.]**
4. Continue through 24/36/48 → `lvl=1/3 prog=0/36`, `lvl=2/3 prog=0/48`, `lvl=3/3 done=true`.
5. Verify fire concept stays 0 (lava checked before fire in `classify_damage_concept`).
6. Mitigation check 20/35/50% reduction.
7. Cross-check: `grant env_lava 3` → done (this is the scenario already exercised in spec §6.1); `set env_lava progress 24`.
8. Negative control: wheel inactive → no progress.

### 4.5 type_general (`general`, `generic`, `damage`)
1. Setup §3, wheel active, reset.
2. `horizon-ctl summon zombie` (in front of player); let it attack (`horizon-ctl wait 200`) or deliberately stand adjacent. Confirm `events` hurt cause = `mob_attack`. (Alternative unclassified source: `arrow`, `mob_projectile`.)
3. After each hit, `status` → `type_general: lvl=0/3 prog<=15.00/30 done=false`, increments ≤15. **[UNVERIFIED: zombie hit damage ~3 HP ⇒ ~3/tick — measure.]**
4. Continue through 30/45/60 → `lvl=1/3 prog=0/45`, `lvl=2/3 prog=0/60`, `lvl=3/3 done=true`.
5. Verify no other concept incremented (mob_attack must be general, not contact/fire).
6. Mitigation check 10/20/30% reduction.
7. Cross-check: `grant type_general 2`, `set type_general progress 30`.
8. Negative control: wheel inactive → no progress.

### 4.6 env_drowning (`drowning`, `drown`)
1. Setup §3, wheel active, reset.
2. Water: `horizon-ctl command setblock ~ ~ ~ water` around the player or `horizon-ctl tp <x> <underwaterY> <z>` into a pre-dug water column; stay submerged with `horizon-ctl wait 400` (~20 s air). Confirm `events` hurt cause = `drown` once air runs out; also expect `LivingDrownEvent` firing during submersion.
3. Sample `status` every ~20-40 ticks: expect `env_drowning: lvl=0/3 prog<=8.00/10 done=false`. Note the drown hook can add a baseline **1.0** per event even before `damage_amount>0` (when `is_drowning` true). **[UNVERIFIED: exact event cadence and whether both hooks fire for the same damage tick — see §5 item 3.]**
4. Continue through 10/16/22 → `lvl=1/3 prog=6/16`, `lvl=2/3 prog=6/22`, `lvl=3/3 done=true`.
5. Mitigation check 25/50/75% reduction on `drown` hurt amounts.
6. Cross-check: `grant env_drowning 3` → done; `set env_drowning progress 8`.
7. Negative control: wheel inactive → `living_drown`/damage hook still runs but `apply_leveled_progress` returns false → no progress **[verify]**.

### 4.7 type_contact (`contact`, `cactus`)
1. Setup §3, wheel active, reset.
2. `horizon-ctl command setblock ~ ~ ~ cactus` (on ground beside player) then `horizon-ctl move forward 200` into it; or `horizon-ctl setblock <x> <y> <z> cactus` + `goto`. Confirm `events` hurt cause = `cactus`. (Alternative: `sweet_berry_bush` at ground level.)
3. After each contact tick (`wait 20`), `status` → `type_contact: lvl=0/3 prog<=10.00/12 done=false`, increments ≤10. **[UNVERIFIED: cactus does ~1 HP/tick ⇒ gain ~1 — many ticks needed; measure.]**
4. Continue through 12/18/24 → `lvl=1/3 prog=8/18`, `lvl=2/3 prog=0/24`, `lvl=3/3 done=true`.
5. Verify general/fire concepts stay 0.
6. Mitigation check 20/35/50% reduction.
7. Cross-check: `grant type_contact 3` (unit test `commands_set_and_grant_new_concepts` covers level 3), `set type_contact progress 12`.
8. Negative control: wheel inactive → no progress.

## 5. Observation & evidence capture

### Observation channels
- **Status (authoritative state):** `horizon-ctl command adaptionwheel status` → JSON, e.g. `{"chat":["[adaptionwheel] status: wheel_active=true source=Inventory schema=1 | env_fall: lvl=0/1 prog=0.00/40 done=false | type_explosion: lvl=0/3 prog=0.00/30 done=false | ..."]}`. Use after *every* event; snapshot before/after.
- **Chat-log:** `horizon-ctl chat-log 20` / `horizon-ctl messages 20` → `[adaptionwheel] ready...`, `summary()` login line, wheel-activation notice, and the echoed `status`/`grant`/`set`/`reset` replies.
- **Events:** `horizon-ctl events --since <SEQ>` → `hurt` (with cause), `death`, `respawn`, chat; map `cause` to classifier (e.g. `lava`→env_lava, `in_fire`→env_fire, `cactus`→type_contact, `fall`→env_fall, `explosion`→type_explosion, `drown`→env_drowning, `mob_attack`→type_general).
- **State/inventory:** `horizon-ctl state` (gamemode, health, position, effects), `horizon-ctl inventory` (wheel present), `horizon-ctl looking-at`/`block` (confirm block placement).
- **Logs:** `horizon-ctl logs` (bounded tail) for hook exceptions/desyncs.

### Evidence-capture checklist (per concept, bounded logs under scratch)
- [ ] Exact command sequence with full JSON replies, `tee`'d to `/mnt/Barracuda/tmp/opencode/qa-s2-scenarios/<concept>.log` (cap size, e.g. `head -c 200000`).
- [ ] `HORIZON_HOME` value, `horizon-ctl hello` (game release/protocol), `horizon-ctl status` (native build id `native-9f42064-67f92751b91f`), world name & gamemode, content hmod sha256 `5c86ca5f...`, ISO-8601 timestamp.
- [ ] Source fingerprint: PR #12 head `2f91bdca...` and lib.rs SHA256 above; companion spec SHA256 above.
- [ ] Baseline + post-each-event + final `status` snapshots; `events --since SEQ` covering every damage event with cause; `chat-log` showing `[adaptionwheel]` lines.
- [ ] `state`/`inventory` proving wheel active and intended damage source; negative-control snapshot proving no progress without wheel.
- [ ] Mitigation evidence: identical-source hurt amounts before vs after level-up.
- [ ] Deterministic `grant`/`set` cross-check replies (thresholds/completion) in the same log.
- [ ] `logs` tail on any anomaly; screenshots optional.

## 6. Gaps / uncertainties (all need live confirmation)

1. **Creative-mode damage suppression [UNVERIFIED].** Spec §6 records the instance world as creative. Creative players normally take no damage and `hurt` events may not fire, so organic damage-driven progress/mitigation cannot be assumed. Confirm with a single lava/cactus exposure; if no `hurt`, switch to survival or use `/damage`. This affects all 6 damage-driven concepts and fall mitigation checks. Fall's `LivingFallEvent` may still fire on landing in creative even with no damage — verify.
2. **`/damage` command availability [UNVERIFIED].** Vanilla `/damage @s <amount> <type>` (e.g. `minecraft:lava`) would give deterministic damage amounts and exact expected increments. Check `horizon-ctl help --game` / try `horizon-ctl command damage @s 1 minecraft:fire` before relying on it. If present, prefer it for exact-number evidence over environmental damage whose amount varies with position/distance.
3. **Drowning dual-hook double counting [UNVERIFIED].** Both `living_incoming_damage_v3` and `living_drown` can call `apply_leveled_progress(CONCEPT_DROWNING, ...)`. Whether one damage tick produces two increments (and whether the baseline `gain=1.0` fires every tick while `is_drowning`) is not determinable from code alone. Must measure; expected thresholds 10/16/22 assume a single increment source.
4. **Exact organic damage amounts [UNVERIFIED].** Fire/lava/cactus/mob/explosion damage depends on ticks, distance, difficulty, and gear. The caps (12/12/10/15/20) and thresholds are exact from code; the number of events to complete is not. Use `/damage` (item 2) for exact increment math, environmental damage for realism.
5. **Classifier edge cases [derived, unverified].** Explosion is checked before fire, so fireworks (`fireworks`) → explosion; `firework` singular matches neither fire (excluded) nor explosion (plural only) → falls through to general. Lava checked before fire. Contact = cactus/sweet_berry_bush/stalagmite only. Verify with `events` causes.
6. **Activation timing [partly verified].** `give compass` should trigger `player_inventory_changed`; confirm `wheel_active=true` via status before testing. If the wheel item must be moved to re-evaluate (per `activate` reply), re-open/move the inventory slot.
7. **Fall hook entity mapping [UNVERIFIED].** `living_fall`/`living_drown` require `entity_id → uuid` mapping refreshed in `player_tick`/`player_logged_in`. If mapping is stale, fall/drown progress won't accrue despite damage. Correlate `events` hurt with status progress; if mismatch, note as entity-map gap for Agent #3.
8. **Runtime edge-case replies** (unknown concept/field/value) remain a TODO in spec §3/§7; not re-tested here.
9. **No live execution by this writer.** All numbers are code-derived; no horizon-ctl scenario was run for this draft. Runtime owner should execute §4 and replace **[UNVERIFIED]** tags with measured values.
