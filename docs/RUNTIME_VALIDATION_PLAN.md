# Post-Migration Runtime Validation Plan — AdaptionWheel Horizon (S1)

Owner: qa-s1-runtime (Team `aw-qa-evidence-migration`, Agent #4 lane).
Status: DRAFT (scratch only — do NOT place in repo). Created 2026-11-18.

This plan defines the **runtime** (live `horizon-ctl`) validation to perform AFTER the
AdaptionWheel content/native migration is merged, so the merge gate has reproducible,
bounded, evidence-bearing artifacts. It complements the command-system spec already
produced by Agent #4 (read-only refs listed in §10). No repo files are edited here.

---

## 1. Scope & shared-instance safety

- Validate: native mod loads, `/adaptionwheel` player-context command family registers and
  responds, wheel activation, and attachment-backed state persistence across quit→relaunch.
- The live Horizon instance (`adaptionwheel-test`) is **shared/cycled by other agents**.
  **Do NOT `launch`, `quit`, `save`, `respawn` or otherwise mutate the shared instance
  without explicit lead approval.** The default posture for this deliverable is:
  1. write/bound scripts (done here), 2. document exact commands + expected results (this doc),
  3. run a short **read-only** probe only if the control socket is free and the lead has cleared it.
- Read-only horizon-ctl verbs: `hello`, `status`, `messages`, `chat-log`, `inventory`, `state`,
  `logs`, `events`, `worlds`. Mutating verbs: `command`, `chat`, `give`-via-`command`, `save`,
  `quit`, `launch`, `open`, `tp` — require approval.
- At drafting time the socket `/run/user/1000/horizon-control.sock` was **absent** (no game
  running); `hello` returned a `connection` error. No live commands were run against the
  shared instance. Scripts are provided to be run by the lead under controlled conditions.

## 2. Environment & guardrails (MANDATORY)

Every script sets these before any command. Getting HOME/build wrong is the #1 failure mode.

```
export TMPDIR=/mnt/Barracuda/tmp TEMP=/mnt/Barracuda/tmp TMP=/mnt/Barracuda/tmp
export HORIZON_HOME=/home/alexquasar/.local/share/horizon/instances/adaptionwheel-test   # CRITICAL
export CARGO_TARGET_DIR=/mnt/Barracuda/tmp/cargo-target-qa-s1                            # unique; only if cargo invoked
SOCK=/run/user/1000/horizon-control.sock
GAME=/home/alexquasar/.local/share/horizon-launcher/versions/native-9f42064-67f92751b91f/horizon-client
```

- **CRITICAL `HORIZON_HOME`**: must be the `adaptionwheel-test` instance. Without it,
  `horizon-ctl` uses default `~/.local/share/horizon`, where `adaptionwheel` is **not enabled**
  and the commands are **not registered** (see §7).
- Native build: `native-9f42064-67f92751b91f` (`horizon-client`). This is a current native
  build, not a fresh rebuild of PR #12 head (`2f91bdca`); PR #12-head equivalence is by code
  inspection of `native/adaptation/api/src/lib.rs` only. Record this distinction in evidence.
- Content hmod (PR #7): sha256 `5c86ca5fe759f4706aacae4a22bab9b2e628e4b492aca84b19cccf8e94a2dc11`.
- World: `New World`, mode: creative. Expected `held.item = adaptionwheel:adaptation_wheel`,
  wheel active from Hotbar.
- All output bounded: every horizon-ctl call wrapped in `timeout`, piped through `head -c` /
  `tail -c`, logs kept under `/mnt/Barracuda/tmp/opencode/qa-s1-runtime/`. Gametests/logs can
  reach 100–200 MB — never leave unbounded output.
- No git worktrees; no commits; no edits under `native/**` or `content/**`.

## 3. Exact launch command

`horizon-ctl launch` injects the control socket automatically and the native build renders
headless; the observed launch record already showed `socket=/run/user/1000/horizon-control.sock`,
`creative=true`, `ok=true`. Pass the control/headless flags explicitly only as trailing game
args if the lead requires an exact-flag reproduction; do not double-inject `--control`.

```sh
export HORIZON_HOME=/home/alexquasar/.local/share/horizon/instances/adaptionwheel-test
export TMPDIR=/mnt/Barracuda/tmp TEMP=/mnt/Barracuda/tmp TMP=/mnt/Barracuda/tmp
GAME=/home/alexquasar/.local/share/horizon-launcher/versions/native-9f42064-67f92751b91f/horizon-client
timeout 150 horizon-ctl launch --game "$GAME" --world "New World" --creative --wait \
  --headless --control-socket /run/user/1000/horizon-control.sock
```

- `--wait` waits until player spawned + chunks loaded; use `horizon-ctl wait-for loaded
  --timeout 120` afterwards if launching without `--wait`.
- Record the launch JSON (pid, socket, home, game, log) as artifact `launch.json`.
- Reference observed log: `/mnt/Barracuda/tmp/opencode/launch-native.out`.

## 4. Readiness probes (read-only)

| Command | Expected | Artifact |
|---|---|---|
| `horizon-ctl hello` | JSON `features[...]`, `protocol 0.1.0`, `release 0.1.0` | `hello.json` |
| `horizon-ctl status` | `creative:true`, dimension overworld, `held.item=adaptionwheel:adaptation_wheel`, socket/home as above | `status.json` |
| `horizon-ctl inventory` | wheel present in hotbar (slot 0-8); ids `adaptionwheel:*` | `inventory.json` |

If any probe fails with a `connection` error → game not running / socket absent → see §7.

## 5. Command battery

All `horizon-ctl command ...` calls run in **player context** (the host player). Use `--socket` + `--timeout` and bound output. Commands are registered at permission level 2 (operator); the control player is operator.

| # | horizon-ctl invocation | Expected reply (prefix `[adaptionwheel]`, `"ok":true`) |
|---|---|---|
| 1 | `command adaptionwheel help` | `commands: /adaptionwheel status \| activate \| set ... \| grant ... \| reset (concepts: env_fall,type_explosion,env_fire,env_lava,type_general,env_drowning,type_contact)` |
| 2 | `command adaptionwheel status` | `status: wheel_active=true source=Hotbar schema=1 \| env_fall: lvl=0/1 prog=0.00/40 done=false \| type_explosion: lvl=0/3 prog=0.00/30.00 done=false \| env_fire: lvl=0/3 prog=0.00/20.00 done=false \| env_lava: lvl=0/3 prog=0.00/24.00 done=false \| type_general: lvl=0/3 prog=0.00/30.00 done=false \| env_drowning: lvl=0/3 prog=0.00/10.00 done=false \| type_contact: lvl=0/3 prog=0.00/12.00 done=false` |
| 3 | `command adaptionwheel activate` | `activate: wheel_active=true source=Hotbar (cached from inventory hooks; move/change inventory to re-evaluate automatically)` |
| 4 | `command adaptionwheel grant env_lava 3` | `granted env_lava level 3. ... env_lava: lvl=3/3 done=true` |
| 5 | `command adaptionwheel status` | reflects `env_lava: lvl=3/3 done=true` |
| 6 | `command adaptionwheel set env_fall progress 10` | `env_fall progress set to 10.00. ... env_fall: lvl=0/1 prog=10.00/40 done=false` |
| 7 | `command adaptionwheel status` | reflects `env_fall prog=10.00/40`, lava still 3/3 |
| 8 | `command adaptionwheel reset` | `reset done. wheel_active=false source=None schema=1 \| ... lvl=0 ... done=false` (all concepts zeroed) |
| 9 | `command adaptionwheel status` | matches reset state |
| 10 | `messages 20` / `chat-log 20` | shows `[adaptionwheel] ready. activation: ...` login line + summary line + command replies |
| 11 | `command /give @s minecraft:compass 1` (the "give" step) | `ok:true`; `inventory` shows a wheel-fallback item (`minecraft:compass` is a runtime wheel id) |
| 12 | `inventory` | wheel/compass present; confirms activation source (Hotbar/Inventory) |

Notes:
- `status/activate/reset` ignore extra tokens; no subcommand defaults to `status` (no bare-root help).
- `set`/`grant` echo canonical concept; aliases accepted (`fall`, `lava`, `drown`, etc.).
- Edge cases known from code but NOT yet runtime-captured (spec §3/§7): unknown subcommand,
  `grant badconcept 2`, `set env_fall progress abc`, missing args. Include them in a follow-up
  edge-case script if the socket is stable; do not conflate with the core battery above.
- Dedicated-console behavior (`perform_console_command`) is a separate unmet item; horizon-ctl
  `command` is player context, so it does NOT exercise console. Do not claim console coverage here.

## 6. Quit / relaunch persistence checks

Attachment `adaptionwheel:player_state` (`copy_on_death`, JSON-in-NBT) must survive quit→relaunch.
Two sequences; Sequence A is authoritative (matches lead evidence). **Requires lead approval**
because it mutates the shared instance (quit + relaunch).

Sequence A (authoritative, with intervening status):
1. `command adaptionwheel reset` → all lvl=0.
2. `command adaptionwheel grant env_lava 3` → `env_lava: lvl=3/3 done=true`.
3. `command adaptionwheel status` → confirm `lvl=3/3`.
4. `command adaptionwheel quit` → `{"ok":true}`.
5. `sleep 3`, then relaunch per §3.
6. `command adaptionwheel status` → **expect `env_lava: lvl=3/3 done=true`**.
7. `chat-log 20` → expect login line + summary containing `lava lvl=3`. ✅

Sequence B (tight, immediate quit after grant) — diagnostic only:
- `reset` → `grant env_fire 2` → immediate `quit` → relaunch → status.
- If `env_fire 2` is lost in the CORRECT `HORIZON_HOME`, that is a genuine flush-timing issue
  to file against native owner (#3). Do NOT report the historical false alarm: the earlier
  "persistence gap" was caused by launching without `HORIZON_HOME` into the default instance.
- Reproduced lead evidence: `/mnt/Barracuda/tmp/opencode/persist-test.{sh,out}`.

## 7. Env guardrails & failure modes

| Symptom | Cause | Action |
|---|---|---|
| `command adaptionwheel ...` → `{"chat":["Unknown or incomplete command…"]}` | Wrong `HORIZON_HOME` (default instance); content is `mod.netherstarrecipe`, mod not enabled | Set `HORIZON_HOME=.../adaptionwheel-test` and relaunch. Environment/selection issue, NOT a code defect. |
| `hello`/any call → `{"error":"cannot connect ... (os error 2)","kind":"connection"}` | No game running / socket absent | Launch per §3 (needs approval) or wait for another agent. |
| Commands return `ok:false` or `[adaptionwheel] unknown subcommand` | Typo / not-registered build | Verify build id + `hello`; confirm wheel mod enabled in instance. |
| Console says `requires player context` | Expected from dedicated console by design | Use in-game player context (`horizon-ctl command`); do not treat as bug. |
| State reset after relaunch | Missing/incorrect `HORIZON_HOME` (default instance) | Same as row 1; do NOT file native bug. |

## 8. Artifact capture

Store all under `/mnt/Barracuda/tmp/opencode/qa-s1-runtime/` (bounded):
- `launch.json` (§3), `hello.json`, `status.json`, `inventory.json`.
- `battery.out` (§5), `persistence-A.out`, `persistence-B.out` (§6), `chatlog.out`.
- `screenshot.png` via `horizon-ctl screenshot` (optional, if lead wants visual wheel evidence).
- Record in the evidence report: timestamp (`date -Is`), HORIZON_HOME, build id, world, mode,
  hmod sha256, each command + verbatim reply, and artifact absolute paths.

## 9. Script inventory (in `scripts/`)

| Script | Purpose | Mutates shared instance? |
|---|---|---|
| `env.sh` | Shared env/temp/target/socket/build constants (sourced) | No |
| `launch.sh` | Exact native headless launch (§3) — LEAD APPROVAL REQUIRED | Yes |
| `command-battery.sh` | Readiness probes + command battery (§4–§5) | Only via `command`/`give` (op-gated) |
| `persistence-check.sh` | quit→relaunch persistence (§6) — LEAD APPROVAL REQUIRED | Yes |
| `cleanup.sh` | Bounded log/scratch cleanup (no game mutation) | No |

All scripts: set TMPDIR/TEMP/TMP + unique CARGO_TARGET_DIR, `timeout` every horizon-ctl call,
bound output with `head -c`/`tail -c`, and reference the shared socket path.

## 10. Read-only references used

- `/mnt/Barracuda/tmp/aw-pr12/native/adaptation/api/src/lib.rs` (PR #12 head native impl)
- `/mnt/Barracuda/tmp/opencode/aw-cmdspec/docs/COMMAND_SYSTEM_SPEC.md` (§3, §6–§8)
- `/mnt/Barracuda/tmp/opencode/aw-cmdspec/docs/INTEGRATION_STATUS.md` (evidence table, blockers)
- `/mnt/Barracuda/tmp/opencode/cmd-validate.{sh,out}`, `persist-test.{sh,out}`, `launch-native.out`
- live `horizon-ctl --help` (verb list, launch/command/messages/inventory/chat-log syntax)
