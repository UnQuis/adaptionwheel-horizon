# Command System Specification — `/adaptionwheel` (Agent #4)

Owner: Agent #4 (integration/verification, command-system spec + evidence). Native implementation owner: Agent #3 (`native/adaptation/api/src/lib.rs`). This document is **read-only w.r.t. native code**; it specifies expected behavior and records verified evidence/gaps.

Scope: the debug/operator command family registered by the native mod `adaptionwheel` on the target Horizon build. Authoritative implementation at the time of this spec: PR #12 (`feat/native-combined-8-11`, head `2f91bdca72eb88164a7fc011a6cc829cd4a03986`, base PR #5 `fc581fff`), file `native/adaptation/api/src/lib.rs`.

## 1. Registration & dispatch

- Commands are registered in `Hooks::register_commands` via `RegisterCommandsEvent::commands.push(NativeCommand::op("<signature>", 2))`.
- **Permission level: 2** (operator) for all six commands. There is no per-command override; no player-target selector argument is registered.
- Registered signatures (exact strings):

| # | Registered signature | Permission |
|---|---|---|
| 1 | `adaptionwheel status` | 2 |
| 2 | `adaptionwheel activate` | 2 |
| 3 | `adaptionwheel reset` | 2 |
| 4 | `adaptionwheel help` | 2 |
| 5 | `adaptionwheel set <concept:word> <field:word> <value:number>` | 2 |
| 6 | `adaptionwheel grant <concept:word> <level:integer>` | 2 |

- Dispatch (player context, `handle_adaptionwheel_command`): tokens = `command.split_whitespace()`; first token must equal `adaptionwheel` case-insensitively, otherwise `Flow::Continue` (not handled). Subcommand = token 2, lowercased; if absent, defaults to `status`. Recognized: `status`, `activate`, `reset`, `grant`, `set`, `help`; anything else → "unknown subcommand" reply.
- On a handled command: state mutations set `dirty_persist`; `perform_command` then calls `persist_player_state_if_needed(host, uuid, force=true)` (best-effort attachment flush), sends the response via `host.feedback(Text::literal(message))`, and returns `Flow::Cancel`. Unhandled → `Flow::Continue`.
- All replies are prefixed `[adaptionwheel]`.

## 2. Concept model & thresholds

Concepts (canonical IDs): `env_fall`, `type_explosion`, `env_fire`, `env_lava`, `type_general`, `env_drowning`, `type_contact`.

Aliases accepted by `normalize_concept` (case-insensitive):

| Canonical | Aliases |
|---|---|
| `env_fall` | `fall` |
| `type_explosion` | `explosion` |
| `env_fire` | `fire` |
| `env_lava` | `lava` |
| `type_general` | `general`, `generic`, `damage` |
| `env_drowning` | `drowning`, `drown` |
| `type_contact` | `contact`, `cactus` |

Max levels (code constants): `env_fall` = 1 (special, binary); `type_explosion`/`env_fire`/`env_lava`/`type_general`/`env_drowning`/`type_contact` = 3.

Observed progress thresholds in `status` output (`PROG = 0.0 / threshold` for a level-0 entry): `env_fall` 40, `type_explosion` 30, `env_fire` 20, `env_lava` 24, `type_general` 30, `env_drowning` 10, `type_contact` 12. (These are display thresholds from `leveled_threshold(c, level)`; they are not hard-coded acceptance criteria for the command system.)

State schema version: 1. State fields shown by `status`: `wheel_active`, `source` (`None`/`Inventory`/`MainHand`/`OffHand`/`Armor`/`Hotbar`), `schema`, per-concept `lvl`, `prog`, `done`.

## 3. Behavior matrix (player context)

| Command | Arguments | Effect | Reply (prefix `[adaptionwheel]`) |
|---|---|---|---|
| `status` | none | read-only | `status: wheel_active=<bool> source=<Source> schema=<u32> \| <concept>: lvl=<n>/<max> [prog=<f>/<t>] done=<bool> \| …` |
| `activate` | none | read-only | `activate: wheel_active=<bool> source=<Source> (cached from inventory hooks; move/change inventory to re-evaluate automatically)` |
| `reset` | none | resets state to `PlayerAdaptationStateV1::default()` for the invoking player; sets `dirty_persist` | `reset done. <status string>` |
| `grant <concept> <level>` | concept (canonical or alias), level parsed as `u8`; **missing/invalid level defaults to 1** | `env_fall`: level≥1 → lvl=1, completed=true, prog=40; level=0 → lvl=0/completed=false/prog=0. Others: `lvl = min(level, max)`, `completed = (lvl ≥ max)`, `prog = 0`. Sets `dirty_persist` on success. | success: `granted <concept> level <lvl>. <status string>`; unknown concept: `unknown concept '<raw>'. Use env_fall,type_explosion,…`; missing concept: `usage: /adaptionwheel grant <concept> <level>` |
| `set <concept> <level\|progress> <value>` | concept, field (`level`\|`progress`, case-insensitive), value parsed as `f32` (all three required) | `level`: env_fall ≥1 → lvl1/completed/prog40 else lvl0; others `lvl = min(max(0,value as u8), max)`, `completed=(lvl≥max)`. `progress`: env_fall clamped 0..40 and completed if ≥40 or lvl≥1; others clamped 0..`leveled_threshold`, completed=false unless already maxed. Sets `dirty_persist` on success. | success: `<concept> <field> set to <value>. <status string>`; unknown concept: `unknown concept '<raw>'…`; bad field: `field must be 'level' or 'progress'`; bad value: `invalid value '<raw>'`; missing args: `usage: /adaptionwheel set <concept> <level\|progress> <value>` |
| `help` | none | read-only | `commands: /adaptionwheel status \| … \| reset (concepts: env_fall,type_explosion,env_fire,env_lava,type_general,env_drowning,type_contact)` |
| *(unknown subcommand)* | — | none | `unknown subcommand '<sub>'. Try: status, activate, set, grant, reset` |

Notes:
- `set` and `grant` accept both canonical IDs and aliases; replies echo the canonical concept name.
- `set <field>` and subcommand tokens are case-insensitive; concept values are normalized case-insensitively.
- There is **no** `/adaptionwheel` bare-root help default in player context: no subcommand defaults to `status` (see dispatch above). `help` must be passed explicitly.
- `status`/`activate`/`reset` do not take arguments; extra tokens are ignored by the dispatcher.

## 4. Console behavior (dedicated console, API 99 `perform_console_command`)

`Hooks::perform_console_command` calls `handle_adaptionwheel_console_command(event.command)`. Input must equal `adaptionwheel` (case-insensitive) or start with `adaptionwheel `; otherwise `Flow::Continue`.

| Console input | Reply | Flow |
|---|---|---|
| `adaptionwheel` / `adaptionwheel help` | `console: help \| status/reset/activate/set/grant require player context (use in-game as player or target-specific command when available)` | `Cancel` |
| `adaptionwheel status\|reset\|activate\|set\|grant [args]` | `'<sub>' requires player context and is not available from dedicated console in this build; run it as an in-game player command.` | `Cancel` |
| any other `adaptionwheel <sub>` | `unknown subcommand '<sub>'. Console supports: help` | `Cancel` |
| non-adaptionwheel command | — | `Continue` (not handled) |

So: from a dedicated console only `help` produces a meaningful command answer; all player-context operations are explicitly rejected with guidance. This is **by design** for the current build (no target-selector argument is registered), not an unimplemented stub.

## 5. Persistence contract

- Command mutations (`grant`, `set`, `reset`) set `dirty_persist = true` on the in-memory player state.
- `perform_command` then calls `persist_player_state_if_needed(host, uuid, force=true)` → best-effort write of `adaptionwheel:player_state` attachment (`Holder::Player`, `copy_on_death`, JSON payload in NBT `Compound`).
- `player_logged_in` loads the attachment into cache via `sync_cache_from_attachment_if_present` (best-effort; seeds attachment with default if missing) and reports login status.
- `player_tick` performs a non-forced flush when dirty; command path forces the flush.

**Status: CONFIRMED (runtime evidence 2026-11-18).** See §6 for the reproduced grant → quit → relaunch sequence and the earlier false alarm that was retracted.

## 6. Runtime evidence (horizon-ctl)

Environment: Linux; `horizon-ctl` (build `nightly-2026-11-13-9f42064` control binary) driving the **native** Horizon build `native-9f42064-67f92751b91f` (`horizon-client`, `--headless --control-socket`), instance `adaptionwheel-test`, world `New World` (creative), content hmod sha256 `5c86ca5fe759f4706aacae4a22bab9b2e628e4b492aca84b19cccf8e94a2dc11` (PR #7) enabled. **Critical:** must set `HORIZON_HOME=/home/alexquasar/.local/share/horizon/instances/adaptionwheel-test`; without it `horizon-ctl` uses the default `~/.local/share/horizon` instance where `adaptionwheel` is not enabled and the commands are not registered.

All logs/scripts: `/mnt/Barracuda/tmp/opencode/cmd-validate.*`, `persist-test.*`, `persist2*`, `cmd-edge.*` (bounded output).

### 6.1 Command registration/response — all verified working

`horizon-ctl command adaptionwheel <sub>` responses (excerpts, 2026-11-18T15:14:54+08:00):

- `help` → `[adaptionwheel] commands: /adaptionwheel status | … | reset (concepts: env_fall,type_explosion,…)` ✅
- `status` → `[adaptionwheel] status: wheel_active=true source=Hotbar schema=1 | env_fall: lvl=0/1 prog=0.00/40 … env_lava: lvl=3/3 done=true …` ✅
- `activate` → `[adaptionwheel] activate: wheel_active=true source=Hotbar (cached from inventory hooks…)` ✅
- `grant env_lava 3` → `[adaptionwheel] granted env_lava level 3. … env_lava: lvl=3/3 done=true …` ✅
- `set env_fall progress 10` → `[adaptionwheel] env_fall progress set to 10.00. … env_fall: lvl=0/1 prog=10.00/40 …` ✅
- `reset` → `[adaptionwheel] reset done. wheel_active=false source=None schema=1 | … lvl=0 …` ✅
- final `status` matched the reset state ✅

Login-time system chat also verified: `[adaptionwheel] ready. activation: false (None). Wheel works when present anywhere in inventory.` and the summary line with per-concept levels.

### 6.2 Persistence — CONFIRMED (two independent sequences)

Sequence A (with intervening status):
1. `reset` → all lvl=0; 2. `grant env_lava 3` → `lvl=3/3 done=true`; 3. `status` → `lvl=3/3`; 4. `quit`; 5. relaunch (`--game …/native-9f42064-67f92751b91f/horizon-client --world "New World" --creative --wait`, HORIZON_HOME=adaptionwheel-test); 6. `status` → `env_lava: lvl=3/3 done=true` and login chat `lava lvl=3`. ✅

Sequence B (tight — immediate quit, no intervening status): `reset` → `grant env_fire 2` → immediate `quit` → relaunch. Result: the previously-persisted `env_lava lvl=3` survived; the just-granted `env_fire` value did **not** appear in the post-relaunch status. **Interpretation (retracted as a false alarm by orchestrator):** this was caused by launching without `HORIZON_HOME` into the default instance in the original report; with the correct `HORIZON_HOME`, Sequence A confirms attachment persistence works. The orchestrator retracted the earlier "known gap" (msg 2026-11-18) and asked that persistence be recorded as CONFIRMED; no native fix is warranted on that basis. (Follow-up: if a grant immediately followed by quit is ever shown to be lost **in the correct instance**, that would be a genuine flush-timing issue to file against #3 — not currently evidenced.)

### 6.3 Negative control (environment sensitivity)

Launching/attaching without `HORIZON_HOME=…/adaptionwheel-test` (default home) → `command adaptionwheel …` returns `{"chat":["Unknown or incomplete command…"]}` and content is `mod.netherstarrecipe`, not `adaptionwheel`. This is an environment/selection issue, not a code defect.

## 7. Gaps / risks relative to this spec

1. **Console scope (by design):** dedicated console only answers `help`; `status/activate/set/grant/reset` are refused with "requires player context". No target-selector command is registered, so an operator cannot act on another player. If targeted console control is required by the design, that is a future API/UX request (Horizon command API), not a current bug.
2. **Permission level 2 only:** all commands require operator. No runtime test with a non-operator player was performed in this pass; assumed enforced by Horizon's command layer.
3. **Edge-case replies not runtime-captured this pass:** unknown subcommand, missing/invalid arguments, bad concept/field/value (e.g. `grant badconcept 2`, `set env_fall progress abc`) are specified in §3 from code but the live instance was being cycled by other agents when the edge-case script ran (socket unavailable). Unit tests (`commands_set_and_grant_new_concepts`, etc.) cover parsing; runtime edge-case capture remains a TODO.
4. **`set <field>` order:** concept is normalized before value parsing; error precedence is unknown-concept → invalid-value → bad-field. Specified from code, not runtime-verified.
5. **No dedicated-console runtime test:** console guidance strings in §4 are from code (`handle_adaptionwheel_console_command`) and PR #11/#12 unit tests; no live dedicated-console execution was performed here (horizon-ctl `command` runs in player context).
6. **Environment binding:** command availability depends on correct `HORIZON_HOME` instance selection (§6.3). Gate evidence must always record the instance/home.

## 8. Merge-readiness note

Command system behavior (registration, player-context responses, help, state mutation, and attachment persistence) is **functionally confirmed in runtime** on the PR #12 native head with the PR #7 content hmod, which closes the previous "commands/console/persistence not verified" blocker for this scope. Remaining items above (target selector, non-op test, runtime edge cases, dedicated console) are documented gaps/risks, not blockers for the command system itself. Full-stack merge-readiness still depends on the broader runtime/CI evidence tracked in `docs/INTEGRATION_STATUS.md`.
