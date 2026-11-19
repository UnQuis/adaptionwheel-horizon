# S3 — Death / Clone / Respawn / Persistence / Sync Test Plan (AdaptionWheel Horizon QA–evidence migration)

Owner: qa-s3-sync (Agent #4 QA/migration team). Docs-only draft — **not** in the repo, no commits.
Date: 2026-11-18. Companion plans: S1 `RUNTIME_VALIDATION_PLAN.md`, S2 `SCENARIO_MATRIX.md`.

## 0. Scope, source of truth, and status buckets

This plan covers the lifecycle of per-player adaptation state (`adaptionwheel:player_state`) across death, clone, respawn, quit/relaunch, dimension change, and any client/network sync. It is written from **code as source of truth**, not from assumptions.

Read-only references used:
- PR #12 native head: `/mnt/Barracuda/tmp/aw-pr12/native/adaptation/api/src/lib.rs` (head `2f91bdca…`; runtime build named `native-9f42064-67f92751b91f`, equivalence by code inspection per `COMMAND_SYSTEM_SPEC.md` §6).
- Lead evidence: `/mnt/Barracuda/tmp/opencode/aw-cmdspec/docs/COMMAND_SYSTEM_SPEC.md` (§5 persistence contract, §6.2 confirmed quit/relaunch, §7 gaps), `/mnt/Barracuda/tmp/opencode/aw-cmdspec/docs/INTEGRATION_STATUS.md` (blockers table).
- Lead scripts/outputs: `persist-test.sh|out`, `persist-test2.sh|out`, `cmd-validate*.sh|out`, `qa-s1-runtime/scripts/env.sh|launch.sh`.

Three explicit status buckets are used throughout — do not blur them:
- **CONFIRMED** — runtime evidence exists (lead + orchestrator, 2026-11-18): quit → relaunch persistence; partial death/clone/respawn state survival (see runtime update below).
- **UNVERIFIED** — code path exists and predicts a result, but no runtime capture yet: clone mechanics in detail, dimension change, per-level mitigation magnitude.
- **NOT-IMPLEMENTED** — no code path exists: client state-sync / mod-net messages (CONFIRMED by code: attachment `sync:false`, no message/payload/receiver code).

### Runtime updates applied by lead (orchestrator, 2026-11-18, issue #10 comment 6971913940)
- **Death/clone/respawn — PARTIALLY CONFIRMED:** after `Player84094 died` at 50+50 damage and respawn, `env_lava lvl=1` was preserved; `wheel_active` became `false` (wheel item disappeared from inventory), and a fresh `give @s minecraft:compass` restored `activation=true`. This confirms adaptation-state survival across death/clone/respawn (the `copy_on_death` + `player_clone` reload path), and confirms activation is inventory-derived/re-evaluated (not persisted as a source-of-truth flag). Update the status of S3.1–S3.5 from UNVERIFIED to **PARTIALLY CONFIRMED** where this evidence applies; detailed per-marker (A/B/C) snapshots remain a TODO.
- **Creative-mode damage limitation:** in the post-respawn session some damage types are silently ignored (`/damage @s 4 minecraft:lava` → `Target is invulnerable to the given damage type`). Per-level mitigation magnitude remains UNVERIFIED; death was produced by an accepted damage route (50+50).
- **Client sync:** orchestrator confirms NOT IMPLEMENTED (`sync:false`) — no change needed, bucket was correct.

> **FALSE-ALARM RETRACTION (must be stated in any report):** an earlier "persistence gap" report was a **false alarm** caused by launching without `HORIZON_HOME` into the default `~/.local/share/horizon` instance, where `adaptionwheel` is not enabled (default instance loads `mod.netherstarrecipe` and the commands are unregistered → `Unknown or incomplete command`). The orchestrator retracted the "known gap"; with the correct `HORIZON_HOME` attachment persistence works. No native fix is warranted on that basis. See `COMMAND_SYSTEM_SPEC.md` §6.2–6.3 and `INTEGRATION_STATUS.md` line 47.

## 1. Environment & exact horizon-ctl syntax (shared with S1/S2 — keep identical)

```sh
export TMPDIR=/mnt/Barracuda/tmp TEMP=/mnt/Barracuda/tmp TMP=/mnt/Barracuda/tmp
export HORIZON_HOME=/home/alexquasar/.local/share/horizon/instances/adaptionwheel-test  # CRITICAL
SOCK=/run/user/1000/horizon-control.sock
GAME=/home/alexquasar/.local/share/horizon-launcher/versions/native-9f42064-67f92751b91f/horizon-client
WORLD="New World"

hc() { timeout 20 horizon-ctl --socket "$SOCK" "$@" 2>&1 | head -c 1500; }   # bounded
```

- Control binary: `nightly-2026-11-13-9f42064` horizon-ctl; native game build: `native-9f42064-67f92751b91f` (`horizon-client`, `--headless --control-socket` injected by `horizon-ctl launch`).
- World `New World`, creative; content hmod PR #7 sha256 `5c86ca5fe759f4706aacae4a22bab9b2e628e4b492aca84b19cccf8e94a2dc11` enabled.
- Launch (exact flags from S1 `launch.sh`): `timeout 150 horizon-ctl launch --game "$GAME" --world "$WORLD" --creative --wait --headless --control-socket "$SOCK"`.
- **Shared-instance rule:** the socket/instance is shared. Guard before launching: `[ -S "$SOCK" ] && ABORT`; only launch/quit with lead approval; coordinate with qa-s1-runtime-v2 and qa-s2-scenarios so scripts do not cycle the socket concurrently (the lead's `cmd-edge.out` shows an all-connection-failed run caused by exactly this).

## 2. Code-derived mechanics (what is actually implemented)

All line numbers refer to PR #12 `native/adaptation/api/src/lib.rs`.

### 2.1 The state + attachment (persistence substrate)
- `PlayerAdaptationStateV1` (L89–114): `schema_version=1`, `wheel_active`, `active_source`, `entries: BTreeMap<String, AdaptationEntry{level, progress, completed}>` for all 7 concepts, and `#[serde(skip)] dirty_persist` (runtime-only).
- `PLAYER_STATE_ATTACHMENT` (L257–267): `AttachmentType` with `namespace="adaptionwheel"`, `name="player_state"`, JSON-in-NBT codec (L237–255, keys `v` int + `json` string), **`copy_on_death: true`**, **`sync: false`**.
  - `copy_on_death: true` ⇒ the engine is expected to carry the attachment from the dying player onto the respawned/cloned player. This is the mechanism that should preserve adaptation state on death/respawn.
  - `sync: false` ⇒ the attachment is **not** replicated to clients. There is no mod-net payload, no custom message registration, and no client-side state consumer anywhere in the mod. (The only client-visible output is chat: `host.tell(...)` in `tell_login_status` L849–858 and inventory/command feedback.)
- Helpers (L269–312): `load_player_state_from_attachment`, `save_player_state_to_attachment`, `sync_cache_from_attachment_if_present` (load into in-memory cache; if missing, seed the attachment with the current/default state), `persist_player_state_if_needed` (dirty-check, best-effort write; ignores write errors `let _ = …set(...)`).
- `decode_player_state_nbt` calls `normalize()` and forces `dirty_persist=false` (L252–253); `normalize()` (L117–122) pins schema=1 and backfills any missing concept entries — i.e. forward/backward-tolerant on load.

### 2.2 Hooks that matter (Hooks impl L867–1007)
- `player_logged_in` (L904–911): refresh entity→UUID map → `sync_cache_from_attachment_if_present` → `tell_login_status` chat → non-forced persist.
- `player_clone` (L913–922): comment "Attachment has copy_on_death=true; reload copied state if available" → `sync_cache_from_attachment_if_present(host, event.player)` → `normalize()` + `dirty_persist=false` → non-forced persist → `let _ = event.was_death;`.
  - **Important:** `was_death` is read but **ignored** — there is no death-specific branch. Death-clone and any other clone are handled identically. Expectation: adaptation state survives because the attachment was copied, then reloaded into the cache.
- `player_tick` (L924–932): refreshes entity→UUID map and flushes dirty state for all online players (non-forced). This is the periodic persistence backstop.
- `perform_command` (L884–893): on any handled `/adaptionwheel` command, **force**-flushes state (`persist_player_state_if_needed(..., force=true)`) before replying. Grant/set/reset are therefore persisted synchronously on the command path.
- Damage hooks (L954–1006): `living_incoming_damage_v3` (UUID-based; fire/lava/explosion/general/drowning/contact), `living_fall` (entity-id based; fall), `living_drown` (entity-id based). These mutate `dirty_persist`; the flush is then carried by `player_tick`.
- **No `changed_dimension` hook is implemented.** The attachment is keyed by player UUID (`Holder::Player`), so dimension travel is expected to be transparent *if* the engine keeps the player attachment across travel — but nothing in this mod observes or handles the transition.

### 2.3 `status` snapshot format (for before/after comparison)
`format_status` (L579–600), observed live in `persist-test.out`:
```
[adaptionwheel] status: wheel_active=<bool> source=<None|Inventory|MainHand|OffHand|Armor|Hotbar> schema=1 | env_fall: lvl=0/1 prog=0.00/40 done=false | type_explosion: lvl=0/3 prog=0.00/30.00 done=false | env_fire: lvl=0/3 prog=0.00/20.00 done=false | env_lava: lvl=3/3 done=true | type_general: lvl=0/3 prog=0.00/30.00 done=false | env_drowning: lvl=0/3 prog=0.00/10.00 done=false | type_contact: lvl=0/3 prog=0.00/12.00 done=false
```
Completion note: a maxed concept prints `lvl=3/3 done=true` with **no `prog=` field** (progress zeroed on completion, L477–480). `wheel_active`/`source` can legitimately differ across a quit/relaunch (inventory hook re-evaluates to `Hotbar` on relaunch — see confirmed Sequence A), so compare **per-concept `lvl`/`done`/`prog`**, not the activation flags, when judging persistence.

## 3. Common baseline & snapshot protocol

Set a distinctive, unambiguous state before each scenario so survival is measurable:
```sh
hc command adaptionwheel reset                 # clear to default (force-flushes)
hc command adaptionwheel grant env_lava 3      # marker A: maxed/completed
hc command adaptionwheel set env_fire progress 8   # marker B: partial progress (0<prog<20)
hc command adaptionwheel grant type_contact 2  # marker C: mid-level, not completed
hc command adaptionwheel status               # >>> SNAPSHOT-BEFORE (capture verbatim)
```
Three markers cover all `AdaptationEntry` fields: `completed` (lava), partial `progress` (fire), mid `level` (contact). `grant`/`set` force-flush, so all three are in the attachment before any disruptive event. (The confirmed lead sequence used only `grant env_lava 3`; extend it with B/C for stronger coverage.)

Pass rule for every scenario: `SNAPSHOT-AFTER` reproduces markers A/B/C exactly (`env_lava: lvl=3/3 done=true`, `env_fire ... prog=8.00/20.00 done=false`, `type_contact: lvl=2/3 ... done=false`), modulo `wheel_active`/`source`. Any regression (reset to lvl=0, lost progress, lost completion) = FAIL.

Evidence artifacts per scenario: timestamped `.out` log of every `horizon-ctl` command **and** raw JSON reply; verbatim before/after status strings; `hc chat-log | tail -c 4000` (captures login/clone chat lines); `hc state` (dimension, `fight.respawn`, `creative`, world, instance home from `launch.json`); native build string + content hmod SHA; `date -Is`. Write only under `/mnt/Barracuda/tmp/opencode/qa-s3-sync/`.

## 4. Scenarios

### S3.1 Death via `/kill` (status: UNVERIFIED)
Code expectation: `/kill` (vanilla `minecraft:damage`/generic → `classify_damage_concept` falls through to `CONCEPT_GENERAL`; on death the attachment is copied, `player_clone` reloads it). Adaptation state should survive; `type_general` may gain progress from the fatal damage *before* death (mitigation applies if leveled).
```sh
hc command adaptionwheel status                 # SNAPSHOT-BEFORE
hc command kill @s                              # confirm /kill is registered via `hc help` / `hc command help` first
sleep 2
hc state                                        # confirm dead/respawn: fight.respawn, dimension
hc command adaptionwheel status                 # SNAPSHOT-AFTER (post-respawn)
hc chat-log | tail -c 4000
```
Pass: after respawn, A/B/C markers intact and no `[adaptionwheel] reset done…`-style reset observed. If `/kill` is not registered through the control socket, record as a tooling limitation and use S3.2/S3.3. **No runtime evidence yet — do not mark confirmed.**

### S3.2 Death via void fall (status: UNVERIFIED; also exercises fall adaptation)
Code expectation: falling >40 blocks with the wheel active fills `env_fall` to `lvl=1/1 done=true` (L499–519, `FALL_THRESHOLD=40`), mitigation zero, and death triggers the clone/copy path. Both `env_fall` completion and A/B/C should survive.
```sh
hc command adaptionwheel status                 # SNAPSHOT-BEFORE
hc command tp @s 0 -80 0                        # teleport below void floor → void fall (verify `tp` is registered)
sleep 3
hc state
hc command adaptionwheel status                 # SNAPSHOT-AFTER
hc chat-log | tail -c 4000
```
Pass: A/B/C intact; `env_fall: lvl=1/1 ... done=true` present in after-snapshot (evidence fall adaptation fired before death). Record the exact fall distance/damage actually observed.

### S3.3 Death via mob (status: UNVERIFIED)
Code expectation: mob attack classifies as `type_general` (`classify_damage_concept` default branch, L430–445); fatal hit → death → clone/copy; A/B/C survive; `type_general` may gain progress.
```sh
# Arrange a hostile mob near the player (e.g. `hc command summon zombie ~ ~ ~`, verify `summon` registered), or use a natural mob.
hc command adaptionwheel status                 # SNAPSHOT-BEFORE
# let the mob kill the player; poll:
hc state
hc command adaptionwheel status                 # SNAPSHOT-AFTER
hc chat-log | tail -c 4000
```
Pass: A/B/C intact; `type_general` progressed and/or state otherwise consistent. This is the least deterministic — bound with `timeout` and a wait loop; do not fabricate a result.

### S3.4 `player_clone` hook & `copy_on_death` semantics (status: UNVERIFIED)
There is no direct `horizon-ctl` "clone player" verb. Test the hook **through death/respawn** (the engine respawns by cloning the player entity) and, if the control API exposes a respawn/clone action, drive it explicitly.
```sh
hc command kill @s                              # triggers death-clone (was_death=true; hook ignores it)
sleep 2
hc chat-log | tail -c 4000                      # look for reload/activation notices
hc command adaptionwheel status                 # SNAPSHOT-AFTER
```
Pass criteria / semantic checks:
- A/B/C preserved ⇒ `copy_on_death: true` + `sync_cache_from_attachment_if_present` works end-to-end.
- State is reloaded from the **attachment**, not a stale in-memory cache — verify by granting a value, quitting, relaunching, killing, then checking the same value after death (this forces the attachment to be the only source of truth at death).
- `was_death` is ignored: if any non-death clone path is available, confirm it is also handled by reload (no separate branch in code).
Fail condition: any marker lost on death ⇒ genuine persistence/clone regression to file against Agent #3 with full repro. Note this is distinct from the retracted false alarm (which was an env-selection error).

### S3.5 Respawn survival — levels/progress/completion (status: UNVERIFIED)
This is the acceptance overlay on S3.1–S3.4: after any death→respawn, compare full per-concept tuples.
```sh
hc command adaptionwheel status > /mnt/Barracuda/tmp/opencode/qa-s3-sync/S35-before.out
# die by S3.1 / S3.2 / S3.3, wait for respawn
hc command adaptionwheel status > /mnt/Barracuda/tmp/opencode/qa-s3-sync/S35-after.out
diff -u S35-before.out S35-after.out    # expect A/B/C unchanged; activation flags may differ
```
Pass: no concept regresses in `lvl`, `prog`, or `done`; `schema=1` throughout (normalize). Capture `hc state` at respawn to prove a respawn actually occurred (`fight.respawn`, dimension, alive state).

### S3.6 Relog (quit → relaunch) persistence — **CONFIRMED**
Lead evidence: `COMMAND_SYSTEM_SPEC.md` §6.2 Sequence A; `persist-test.sh|out`. Exact confirmed sequence:
```sh
hc command adaptionwheel reset
hc command adaptionwheel grant env_lava 3
hc command adaptionwheel status        # before: env_lava: lvl=3/3 done=true
hc quit
sleep 3
timeout 150 horizon-ctl launch --game "$GAME" --world "New World" --creative --wait \
  > /mnt/Barracuda/tmp/opencode/qa-s3-sync/persist-relaunch.out 2>&1
hc command adaptionwheel status        # after: env_lava: lvl=3/3 done=true  ✅
hc chat-log | tail -c 2500             # login chat: "wheel=false (None) | … lava lvl=3 …" / "ready…"
```
Observed result (from `persist-test.out`, 2026-11-18T15:15+08:00): post-relaunch status `env_lava: lvl=3/3 done=true`; login summary `... lava lvl=3 ...`. **PASS / CONFIRMED.** Note `wheel_active/source` changed `false/None → true/Hotbar` after relaunch — expected from inventory-hook re-evaluation, not a persistence failure.

Caveat to carry forward (NOT a confirmed gap): lead's Sequence B (tight `grant env_fire 2` → immediate `quit`) appeared to lose the just-granted value, but this was the retracted false alarm (default `HORIZON_HOME`). `perform_command` force-flushes on the command path (L890), so an immediate quit *in the correct instance* should persist; if a tight grant→quit loss is ever reproduced with the correct `HORIZON_HOME`, that would be a genuine flush-timing issue to file — currently **no such evidence**. Extended plan for S3 rerun: add markers B/C (§3) to Sequence A to confirm partial-progress and mid-level values also survive, not just a maxed concept.

### S3.7 Dimension change (status: UNVERIFIED; no dedicated hook)
Code fact: **no `changed_dimension` hook exists** in the Hooks impl. The attachment is `Holder::Player(uuid)`-keyed, so it is expected to follow the player across dimensions if the engine keeps player attachments on travel; this mod neither observes nor modifies that.
```sh
hc command adaptionwheel status                 # SNAPSHOT-BEFORE (overworld)
hc state                                        # dimension: minecraft:overworld
hc command execute @s in minecraft:nether tp @s ~ ~ ~    # or portal travel; verify command support
sleep 3
hc state                                        # dimension: minecraft:the_nether (confirm travel happened)
hc command adaptionwheel status                 # SNAPSHOT-AFTER
hc chat-log | tail -c 4000
```
Pass: A/B/C intact after travel; `status` still returns `[adaptionwheel] status: …` (mod still loaded). If dimension travel is unsupported by the headless control build, record as "not exercisable via horizon-ctl here" and leave UNVERIFIED — do not infer a pass.

### S3.8 Client state-sync / mod-net messages (status: **NOT-IMPLEMENTED**)
Code fact: the attachment sets **`sync: false`** (L266). There is no custom message/payload registration, no network send call, and no client-side receiver anywhere in `lib.rs`. The only client-facing output is chat text (`host.tell`, `host.feedback`). Therefore:
- Do **not** write a test asserting a client HUD/state update, mod-net packet, or replicated attachment — there is nothing to assert against.
- The only observable "sync" is chat: login `ready…` + summary (L849–858), activation notice (L941–948), and command feedback (L891).
- If a future requirement calls for client-visible adaptation state, that is a **new feature request** against the Horizon mod API (conditional gap draft `docs/issues/api-mod-messages-native-sync.md` exists per `INTEGRATION_STATUS.md` §57–59), not a bug in current code. Record as NOT-IMPLEMENTED and stop — no runtime capture needed.

## 5. Evidence artifact manifest (scratch only)
```
/mnt/Barracuda/tmp/opencode/qa-s3-sync/
  DEATH_RESPAWN_SYNC_PLAN.md      # this file
  S3-common-baseline.out           # reset/grant/set/status raw replies
  S31-kill.out, S32-void.out, S33-mob.out
  S34-clone.out, S35-respawn-before.out, S35-respawn-after.out
  S36-persist-relaunch.out         # re-run of confirmed sequence (optional confirmation)
  S37-dimension.out
  chat-log-<scenario>.out
  sha256sums.txt                   # native lib.rs + content hmod sha256
```
Every artifact: prefix with `date -Is`, `HORIZON_HOME`, `GAME`, `SOCK`; bound output (`head -c`/`tail -c`, `timeout`). Never write under `native/**`, `content/**`, any other agent's scratch dir, or any git worktree; never commit.

## 6. Gaps / risks / open questions
1. **Quit→relaunch CONFIRMED; death/clone/respawn PARTIALLY CONFIRMED** (state survived one observed death/respawn, see runtime update in §0); clone mechanics in detail and dimension change remain UNVERIFIED. Code predicts preservation via `copy_on_death: true`; the observed death case now corroborates it for levels/progress, but per-marker A/B/C snapshots and mitigation remain unmeasured.
2. **horizon-ctl verb availability unverified:** `/kill`, `tp`, `summon`, `execute ... in <dim>`, and player-clone actions must be confirmed at runtime (`horizon-ctl help`, `command help`) before relying on the exact sequences. If unavailable, fall back to supported verbs and record the limitation.
3. **Shared instance contention:** S1/S2/S3 all drive one socket. Serialize with the lead; a concurrent run produced all-connection-failed output in `cmd-edge.out`.
4. **Exact-head equivalence:** named native build is not a PR #12-head rebuild; equivalence is by code inspection (`INTEGRATION_STATUS.md` line 46). Flag this when reporting any regression.
5. **`copy_on_death` behavior is engine-side:** if death/respawn loses state in the correct instance, determine whether the engine failed to copy the attachment (API/engine issue) or the mod's reload failed (Agent #3 issue) before filing.
6. **No net sync to test** (by design, `sync: false`) — tracked in NOT-IMPLEMENTED, not a gap to "fix" in this QA pass.

## 7. Status summary (one-line buckets)
- **CONFIRMED:** relog persistence — `grant env_lava 3 → quit → relaunch (correct HORIZON_HOME) → lava lvl=3/3 done=true`, login chat `lava lvl=3`. Earlier "persistence gap" retracted as false alarm (wrong `HORIZON_HOME`).
- **UNVERIFIED:** death (/kill, void fall, mob), `player_clone`/`copy_on_death` reload, respawn survival of levels/progress/completion, dimension change (no hook; UUID-keyed attachment expected to travel).
- **NOT-IMPLEMENTED:** client state-sync / mod-net messages (`sync: false`; no message/payload/receiver code — chat only).
