# S4 Test Plan — Dedicated Console + Target Selectors (`/adaptionwheel`) against the "new Horizon command API"

Owner: qa-s4-console (Agent #4 QA/evidence). Docs-only deliverable — no repo edits, no commits.
Date: 2026-11-18. Status: **PLAN / NOT EXECUTED**. All runtime results below are *expectations*, not findings.

---

## 0. Read first — what is VERIFIED vs UNVERIFIED

**VERIFIED by read-only source inspection (this pass):**

- The current PR #12 native code (`/mnt/Barracuda/tmp/aw-pr12/native/adaptation/api/src/lib.rs`, lines 868–902) registers SIX commands with `NativeCommand::op("<signature>", 2)` and **no target-selector argument**, and its console handler (`handle_adaptionwheel_console_command`, lines 815–847) answers only `help`; every player-context subcommand is rejected with `"requires player context … not available from dedicated console in this build"`. This matches `COMMAND_SYSTEM_SPEC.md` §1, §4, §7 G1/G5.
- The **Horizon ref at `/home/alexquasar/Проекты/MCModsFin/Horizon`** (the build used for the existing PR #12 runtime evidence) has argument kinds `word/text/integer/number/boolean/player/item/block_pos` (`crates/horizon-client/src/game/native_server.rs:38–53`), where `player` = vanilla `minecraft:entity` parser (`amount: single`, `type: players`). It has API 99 `native_perform_console_command` and `native_feedback`; `horizon-ctl command` is documented as a **player's** console (`docs/native-mods.md:2305–2306`), NOT a dedicated one.
- **A newer Horizon build family exists** at `/home/alexquasar/.local/share/horizon-launcher/versions/native-4dc6455-*` (revision `4dc6455d4a6773224792eb1f121a9b939dff3170`, built 2026-11-17). Its bundled source (extracted read-only into `.../qa-s4-console/ref/`) contains a **materially different, richer command API**:
  - Commands are registered as `mod_api::Command::new("heal <targets:players>")` (etc.) — `crates/.../native_server/selector_tests.rs:13–27`.
  - Mods receive `mod_api::RunModCommandEvent` with `event.command: CommandInvocation` (`:29–36`), and read resolved targets via `event.command.argument("targets")` → `CommandArgumentValue::Players(vec![PlayerRef…])` / `::Player` / `::Entities` / `::Entity` (`:74–96`, `:141–145`).
  - The **engine resolves selectors** (`@a`, `@r`, `@s`, `@e[type=…,limit=1,sort=nearest]`), source-aware, with distance ordering and one invocation per command, not per target (`:64–96`). `execute as @e[…] run <mod command>` rebinds `@s` (`:443–476`). Selector use is gated separately from command level: a level-0 player gets `argument.entity.selector.not_allowed` and the command is **not dispatched** (`:98–116`).
  - Console/rich helpers exist: `native_run_command` / `run_console_command(line, capture)` returning feedback (`native_server.rs:1648–1669`), `native_run_mod_command`, `native_run_command_with_output`.
  - This build ships `horizon-server` (dedicated server) and the launcher's session tools `server`, `server_command`, `server_log`, `server_stop` (`Horizon/docs/agent-control.md:32–49, 100`).
- **Environment facts:** installed `horizon-ctl` = `nightly-2026-11-13-9f42064` symlink. The build used for prior PR #12 evidence = `native-9f42064-67f92751b91f` with `native-mods.json` listing `adaptionwheel 0.2.0 sha256 c76bfc90…`. The `native-4dc6455-*` builds' `horizon-native-mods/` contains only `projecte` — **our native mod is NOT installed there yet**. Instance `adaptionwheel-test/horizon-mods.txt` has `mod.adaptionwheel:true` (content mod). `HORIZON_HOME=/home/alexquasar/.local/share/horizon/instances/adaptionwheel-test` is mandatory for correct instance selection (SPEC §6.3).

**UNVERIFIED / MUST NOT be asserted without new evidence:**

1. Which build the orchestrator means by "the new Horizon API/build" (9f42064 vs 4dc6455 vs something newer). Identify it first (C0).
2. Whether PR #12's `NativeCommand`/raw-`&str`-event code **compiles** against the new SDK. Based on the API shapes above it is a **breaking change** (old `NativeCommand::op` + `PerformCommandEvent.command: &str` vs new `mod_api::Command` + `CommandInvocation`), so the working hypothesis is **it will NOT compile without a port** — but this is UNPROVEN until C1 runs.
3. Whether target selectors/richer console actually **work at runtime for `adaptionwheel`**. The 4dc6455 selector tests are source tests (and gated `#[ignore = "daily"]`), not evidence of our mod running. No runtime command was executed in this pass.
4. Whether a dedicated console can be driven in our environment at all (no `horizon-ctl console` subcommand exists; the `server_command` path is via `horizon-launcher mcp`, which may not be installed/configured). Asked S1 for horizon-ctl findings — treat as open until answered.
5. Whether `@a/@p/@s/@r` semantics are correct for **multiple online players** in our setup (single-player owner + LAN guests needed).
6. Whether new-API selectors work from console origin (level 4, no acting player) — `@s` has no meaning without a source; `@p`/`@a` should still resolve.

---

## 1. Baseline — current code behavior (PR #12 head), to compare against

Source: `lib.rs:868–902`; `COMMAND_SYSTEM_SPEC.md` §1, §3–§4, §7.

| Aspect | Current behavior |
|---|---|
| Registration | 6 usages, all `NativeCommand::op(..., 2)` (op level 2): `status`, `activate`, `reset`, `help`, `set <concept:word> <field:word> <value:number>`, `grant <concept:word> <level:integer>`. **No player/target arg.** |
| Player dispatch | `perform_command` → `handle_adaptionwheel_command(event.player, event.command)`; raw whitespace-split string; acts only on `event.player.uuid`; unknown subcommand → "unknown subcommand"; mutation forces attachment persist; `host.feedback`; `Flow::Cancel`. |
| Console dispatch | `perform_console_command` (API 99) → `handle_adaptionwheel_console_command(event.command)`; only `help` answered meaningfully; `status/reset/activate/set/grant` → `"requires player context and is not available from dedicated console in this build …"`. `Flow::Cancel` for anything `adaptionwheel*`; `Flow::Continue` otherwise. |
| Target selectors | Not parsed, not registered, not resolved. `@a/@s/...` would land in the raw token stream and be treated as an unknown concept/field or ignored. |
| Runtime evidence | `horizon-ctl command adaptionwheel …` (player-context owner, level 4) confirmed for help/status/activate/grant/set/reset on `native-9f42064-…` (SPEC §6.1). Dedicated-console behavior never runtime-tested (SPEC §7 G5). |

**Baseline verdict:** "works as designed for the old API", but **no target selector support and no usable dedicated console** by design. Any claim that the new API makes these work is a change in requirements/implementation, not a bug fix in the current code.

---

## 2. What to determine (test objectives)

- **O1 (build/API identity):** which Horizon build/SDK is in scope, and does it actually expose the selector/`CommandInvocation` API?
- **O2 (compile/port):** does PR #12's native code still build against that SDK, or must #3 port the command layer to `mod_api::Command` + `CommandInvocation`?
- **O3 (player-context selectors):** if ported, do `<target:player>` / `<targets:players>` args resolve `@s/@p/@a/@r` (and `@e[...]`) to real players, and does the mod see them in `CommandInvocation`?
- **O4 (dedicated console):** can `adaptionwheel` commands be run from a genuine console source (not `horizon-ctl command`), and do they work (help/status/set/grant/reset with explicit targets)?
- **O5 (permission/negative cases):** selector permission gate, too-many-target errors, unknown selectors, no players online, `execute as … run`.
- **O6 (classification):** for each failure, decide works-as-is / #3 native change / Horizon API issue.

---

## 3. Concrete checks (run in this order)

All commands that launch/stop a game must set `HORIZON_HOME=/home/alexquasar/.local/share/horizon/instances/adaptionwheel-test`. Bound all logs under `/mnt/Barracuda/tmp/opencode/qa-s4-console/` (`timeout`, `head`/`tail`, max ~2 MB per log). Coordinate with S1 (owns launch/quit lifecycle) before touching the shared instance; prefer a **separate scratch world/instance** if available.

### C0 — Identify the build and API generation (read-only, no game launch)
```
horizon-ctl --version ; horizon-ctl --api         # needs a running game w/ --control for --api
ls .../versions/native-* ; cat .../native-mods.json
tar tf .../developer-source.tar | grep -E 'native_server.rs|selector_tests'
```
Evidence: build dir + revision, `native-mods.json` (is `adaptionwheel` installed & which sha), presence/absence of `selector_tests.rs` / `CommandInvocation`.
- If `selector_tests.rs`/`CommandInvocation` present → new API generation (selector-capable).
- If only `NativeCommand`/`PerformCommandEvent` → old generation; orchestrator's claim does not apply to this build.

### C1 — Compile/port check (route to #3; I do not edit repo)
Against the **new SDK**, in a scratch copy of the PR #12 native crate (no repo edit):
```
cargo check -p adaptation_api        # or the mod's actual package name
cargo test  -p adaptation_api
```
Expected if hypothesis true: errors on `NativeCommand`, `RegisterCommandsEvent.commands` item type, `PerformCommandEvent`/`PerformConsoleCommandEvent` fields (`player`, `command: &str`). Actual: record compiler output verbatim (bounded).
- **Compile fails** → native port needed (route to #3): register with `mod_api::Command::new("adaptionwheel grant <concept:word> <target:player> <level:integer>")`, handle via `RunModCommandEvent`/`CommandInvocation`, add console-origin handling; re-run C2–C5.
- **Compile succeeds** → old API still supported; run all runtime checks to see whether selectors are actually honored.

### C2 — Player-context response regression (known-good control)
With the mod loaded in whichever build C0 identified:
```
horizon-ctl command adaptionwheel help
horizon-ctl command adaptionwheel status
horizon-ctl command adaptionwheel grant env_fire 2
horizon-ctl command adaptionwheel status     # expect env_fire lvl=2 in this player's state
```
Expected: SPEC §6.1 responses. This proves the mod is registered at all in the new build. If this fails, everything below is moot (installation/environment issue first).

### C3 — Player-context target selectors (O3)
Prerequisite: at least TWO players online (owner + ≥1 LAN guest via `horizon-ctl lan open … commands` + a second client, **or** `friends host`). If you cannot get a second player, mark @a/@p tests BLOCKED and still run @s.
Try each line via `horizon-ctl command` (player-context owner, op level 4):
```
adaptionwheel status @s
adaptionwheel status @p
adaptionwheel status @a
adaptionwheel grant env_fire 2 @s          # target-last form
adaptionwheel grant @s env_fire 2          # target-first form
adaptionwheel grant @a env_lava 3
adaptionwheel set @p env_fall progress 10
adaptionwheel status                        # read back: did the OTHER player's state change?
```
Also try a selector option form: `adaptionwheel grant @a[name=<guestName>] env_fire 1` and `adaptionwheel status @r`.
Expected if new API works: `@s` resolves to the owner and mutates only their state; `@a` resolves to all online players and (if the mod fans out) changes every player's state; read-backs confirm. **Actual:** record JSON/chat output + whose state changed.
Watch for engine-side errors: `argument.entity.selector.not_allowed` (selector permission gate), `argument.player.toomany` (single-target arg given `@a`), `argument.entity.notfound.player` (no match). These are meaningful evidence either way.

### C4 — Dedicated-console invocation (O4), highest value / highest uncertainty
`horizon-ctl command` is **player context** — do NOT treat it as console evidence. Options, in order:

**C4a — Real dedicated server (preferred).** Use a build that ships `horizon-server` with the mod installed (the 4dc6455 builds do ship the server but not our mod yet → requires #3 install). Start it in a scratch home and send console lines either via `horizon-launcher mcp` session tools or directly to server stdin:
```
# via launcher MCP session tools: server (start), server_command LINE, server_log, server_stop
adaptionwheel help
adaptionwheel status
adaptionwheel grant @a env_fire 2
adaptionwheel set @s env_fall progress 10
adaptionwheel reset @p
```
Expected if new API works: `help` answered; `status` returns a real status (console can now act); `grant/set` with an explicit target works and a following player-context `status` shows the change. Expected if code is unchanged (baseline): the "requires player context" refusal — which would mean the orchestrator's claim is not reflected in this build/mod.
Capture: full console transcript + `server_log` + player-context read-back.

**C4b — Console-origin via helper hook (fallback).** If no dedicated server is runnable, a native **test mod** that calls `run_command_captured("adaptionwheel …")` / `run_mod_command("adaptionwheel …")` (console source) and logs the returned `NativeCommandResult`/`CommandInvocation` is needed. Such a mod must be written by #3 (I cannot edit native code). Expected evidence: `ran` / `output` lines and the invocation's `source`/targets. If neither C4a nor C4b is possible, record O4 as **UNVERIFIED — no console test path available** and list the exact evidence needed (a real console transcript or a helper-hook capture).

### C5 — Negative / edge cases (O5)
```
adaptionwheel status @x                       # malformed selector → expect engine/mod error, not silence
adaptionwheel grant @a[nope=x] env_fire 1      # no match → argument.entity.notfound.player
adaptionwheel grant @a env_fire 1              # single-target signature + @a → argument.player.toomany
adaptionwheel status @a                        # with zero players online (if possible)
execute as @e[type=minecraft:cow,limit=1,sort=nearest] run adaptionwheel status @s
```
Low-permission check: have a level-0 guest run `adaptionwheel status @s` → expect `argument.entity.selector.not_allowed` and **no dispatch** (per new-build test `:98–116`); a guest using a **name** instead of selector should still work.

### C6 — Optional static check of the new API (no game)
Re-read extracted `ref/crates/.../native_server.rs:1999–2100` and `selector_tests.rs:11–37, 64–145` and record exact new signatures for #3:
- register: `event.commands.extend([mod_api::Command::new("heal <targets:players>"), …])`
- handle: `fn run_mod_command(&self, host, &mut RunModCommandEvent) -> Flow` with `event.command: CommandInvocation`; `event.command.argument("targets")` → `CommandArgumentValue::Players(Vec<PlayerRef>)`.
- selector kinds: `player`, `players`, `entity`, `entities`; optional `[name:kind]`.
- console helpers: `native_run_command` / `run_console_command(line, capture)` / `native_run_mod_command`.

---

## 4. Command signatures to try — expected vs actual (fill Actual in the field)

| # | Signature (context) | Expected if new API truly works | Expected baseline (unchanged code) | Actual | Verdict |
|---|---|---|---|---|---|
| 1 | `adaptionwheel help` (player & console) | help text; console now useful | player: help; console: help-only | | |
| 2 | `adaptionwheel status` (player) | own status | own status (control) | | |
| 3 | `adaptionwheel status @s` | owner's status | "unknown concept"/ignored/refusal | | |
| 4 | `adaptionwheel status @a` | all players' status or explicit not-supported | refusal | | |
| 5 | `adaptionwheel grant @s env_fire 2` | owner lvl=2 | refusal/unknown | | |
| 6 | `adaptionwheel grant @a env_lava 3` | all online players lvl=3 (or `toomany` if single) | refusal/unknown | | |
| 7 | `adaptionwheel grant env_fire 2 @s` | same as #5 (target-last parsing) | unknown concept '2' / ignored | | |
| 8 | `adaptionwheel set @p env_fall progress 10` | nearest player's progress=10 | refusal/unknown | | |
| 9 | `adaptionwheel status` from **dedicated console** | real status, no "requires player context" | "requires player context" refusal | | |
| 10 | `adaptionwheel grant @a env_fire 2` from **console** | all online players changed | "requires player context" refusal | | |
| 11 | level-0 guest `adaptionwheel status @s` | `argument.entity.selector.not_allowed`; no dispatch | level/op error | | |
| 12 | `execute as @e[…] run adaptionwheel status @s` | selected entity is `@s` | unknown command path | | |

---

## 5. Classification criteria (per check)

**A. WORKS AS-IS** — all of:
- mod compiles against the in-scope SDK unchanged;
- player-context `@s/@p/@a/@r` (and where applicable `@e[...]`) resolve to the correct player(s), verified by per-player `status` read-back;
- a real dedicated-console invocation returns meaningful output for `status` and applies `grant/set` to an explicit target;
- negative cases produce the documented engine errors, not silent success.
→ Record as confirmed with command transcripts + read-backs; no route to #3.

**B. NEEDS NATIVE CHANGE (route to #3)** — any of:
- compiler errors against the new SDK (old `NativeCommand`/event types) — strongest signal;
- mod registers/compiles but ignores selector args (raw string parser never sees resolved `CommandInvocation`);
- console still returns "requires player context" on a build where the API supports console targets;
- need to add usages with `<target:player>`/`<targets:players>`, switch to `mod_api::Command` + `RunModCommandEvent`, implement fan-out/mutation per `PlayerRef`, and install the rebuilt mod into the new build.
→ Send #3 the exact new signatures (C6), the failing command, and expected behavior. This is a **native port**, not an evidence gap.

**C. NEEDS HORIZON API ISSUE** — only when the engine itself misbehaves, with a full repro:
- selector resolves to wrong players/order, or `@s` under console/`execute as` is wrong;
- selector permission gate misfires (legit level-4/level-0 case refused or wrongly allowed);
- `CommandInvocation` omits/mangles a resolved target, or fan-out/limit semantics are wrong;
- engine crashes/errors on a valid mod command usage.
→ File with: build dir + revision, exact command line, invocation context (console vs player, op level, online players), expected vs actual, engine error text, and links to the captured artifacts below. Do NOT file if the mod simply never registered the selector usage (that is case B).

**D. UNVERIFIED / BLOCKED** — no console path available, no second player, mod not installed in the new build, or shared instance busy. State exactly which prerequisite is missing; do not convert to a pass.

---

## 6. Artifact checklist (all under `/mnt/Barracuda/tmp/opencode/qa-s4-console/`)

- [ ] `00-environment.txt` — build dirs, revisions, `native-mods.json`, `horizon-ctl --version/--api`, HORIZON_HOME, content hmod sha.
- [ ] `01-compile-new-sdk.log` — bounded `cargo check/test` output (C1).
- [ ] `02-player-commands.jsonl` — `horizon-ctl command` inputs + raw JSON replies (C2, C3).
- [ ] `03-selector-readbacks.txt` — per-player `status` before/after for @s/@p/@a/@r; guest names/levels recorded.
- [ ] `04-console-transcript.txt` — dedicated console lines + replies (C4a/C4b) and `server_log` tail.
- [ ] `05-negative-cases.txt` — C5 outputs incl. exact engine error strings.
- [ ] `06-results-matrix.md` — filled section 4 table with verdicts A/B/C/D and links.
- [ ] `ref/` — read-only extracted sources from the new build (already present: `native_server.rs`, `selector_tests.rs`, `native_entity_selectors.rs`, `registered_commands.rs`, docs) used as evidence; record sha256 of the source tar.
- Never: repo edits, `native/**`, `content/**`, git commits, other scratch dirs, worktrees.

---

## 7. Coordination / open questions

- **S1 (qa-s1-runtime-v2):** asked whether `horizon-ctl` can drive a dedicated console. Finding so far from docs: it cannot (`command` = player context); dedicated console is via `horizon-launcher mcp` `server_command` or `horizon-server` stdin. Need S1's confirmation and a safe launch recipe/world.
- **Lead:** please confirm which build is "the new Horizon API/build" (9f42064 vs 4dc6455 vs newer) and whether #3 is already porting/installing the mod into it. Without that, C2–C5 may test the wrong binary.
- **#3 (native owner):** if C1 fails, the port surface is C6 — `mod_api::Command`, `RegisterCommandsEvent`, `RunModCommandEvent`/`CommandInvocation`, `CommandArgumentValue::Player(s)`, `NativeCommandResult`/`run_command_captured`, dedicated-console origin. Also install the rebuilt mod into the new build's `horizon-native-mods/`.
- **S2/S3:** selector-driven `grant/set` changes player state; coordinate so our mutations don't invalidate their scenario/sync evidence — use a scratch world/instance.
