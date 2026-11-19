# Sunrise Runtime Validation Plan (Agent #4) — Sunrise is primary target; Horizon is archive

Adapted from `docs/RUNTIME_VALIDATION_PLAN.md` + `docs/SCENARIO_MATRIX.md` (horizon-ctl) to **sunrise-ctl** after the user decision (2026-11-19) that the main target is Sunrise, not Horizon. All temp/artifacts under `/mnt/Barracuda/tmp/opencode/`. Scripts: `sunrise-battery/*.sh` (`sh -n` PASS).

## 0. Tooling (discovered, not assumed)
- `sunrise-ctl`: `/home/alexquasar/.local/share/sunrise/launcher/versions/nightly-2026-11-19-ed3ac19/sunrise-ctl` (also a copy in the `native-ed3ac19-c350cd517a71` version dir). Not on PATH — use the absolute path from `env.sh`.
- Launcher: `/mnt/Barracuda/tmp/opencode/sunrise-launcher/sunrise-launcher` (1.1.1).
- Native client / profile `sunrise`: version `native-ed3ac19-c350cd517a71`; `--game` path in `env.sh` (`…/native-ed3ac19-c350cd517a71/sunrise-client`).
- Control socket: `$SUNRISE_CONTROL`, default `/run/user/1000/sunrise-control.sock` (was `/run/user/1000/horizon-control.sock` under Horizon).
- Game home: `$SUNRISE_HOME=/home/alexquasar/.local/share/sunrise` (launcher data root; **orchestrator-confirmed working value** — NOT `.../game`). **CRITICAL:** without it, sunrise-ctl may target the wrong home and the mod is not enabled → commands unregistered ("Unknown or incomplete command"). Same class of failure as the Horizon `HORIZON_HOME` false alarm.
- Working game binary: `/home/alexquasar/sunrise-game-bin/sunrise-client` — a copy of the native build install (contains `libmod_adaptionwheel.so`), used because of the launcher's cache-lease check. Alternative: the version-dir client `/home/alexquasar/.local/share/sunrise/launcher/versions/native-ed3ac19-c350cd517a71/sunrise-client` when the lease is satisfied.
- Content artifact: `adaptionwheel.smod` (Sunrise extension, NOT `.hmod`) at `/home/alexquasar/.local/share/sunrise/sunrise-mods/`; `sunrise-mods.txt` contains `mod.adaptionwheel:true`.
- Command syntax: `sunrise-ctl command <TEXT>` is player-context (leading `/` optional), same as horizon-ctl. `sunrise-ctl help --game` lists the running game's own commands/test hooks (use to confirm `/damage`, selectors). `sunrise-ctl command` replies include `ok`, `chat`, and (hosted/alone) `success`/`result` — richer than horizon-ctl; useful as negative evidence when a command is refused.

## 1. Source-of-truth / port changes (from orchestrator)
Port required only two edits vs the Horizon code: `#[mod_api::horizon_mod]` → `#[mod_api::sunrise_mod]`, and `LevelRandomHost` → `HitHost`. `cargo test` **81/81 PASS** on the signed SDK `sunrise-org/sunrise sdk-ed3ac195…` (PR #19). Signed SDK resolves the earlier "no signed SDK" Horizon tooling blocker (horizon-rust/horizon#18184).

## 2. Battery (scripts in sunrise-battery/)
| Script | Purpose |
|---|---|
| `env.sh` | source-able env (SUNRISE_HOME/CONTROL/CTL/GAME, TMP*, unique CARGO_TARGET_DIR, bounded `sc()`/`sc_ready()`/`sc_env()` helpers) |
| `launch.sh` | headless launch `--game <native> --world "New World" --creative --wait`; guard refuses to quit/launch a shared instance unless `QA_SUNRISE_APPROVED_LAUNCH=1` |
| `command-battery.sh` | hello, give compass, help/status/activate/grant env_lava 3/set env_fall progress 10/reset, chat-log |
| `persistence-check.sh` | reset → grant env_lava 3 → status → quit → relaunch → status (guard `QA_SUNRISE_APPROVED_RELAUNCH=1`); expected `lvl=3/3` survives |
| `target-selectors.sh` | check `hello`/`help --game`, `@s` selector sanity, `damage @s 1 minecraft:generic`, `adaptionwheel grant @s env_lava 1`; note dedicated-console is a separate path (launcher `server` / ctl `mcp server_command`) — `command` is NOT console evidence |
| `damage-progression.sh` | all 7 concepts via `/damage` + environment (lava/fire/generic/cactus/explosion/drown/fall), status snapshots |
| `cleanup.sh` | guarded quit (`QA_SUNRISE_APPROVED_QUIT=1`) |

## 3. Scenario coverage (carried over from SCENARIO_MATRIX.md, recalc for Sunrise)
- Concepts: env_fall, type_explosion, env_fire, env_lava, type_general, env_drowning, type_contact — expected caps/thresholds from the (now Sunrise) native code; use `/damage` for exact increments (old Horizon instance confirmed `/damage @s 1 minecraft:generic` → Applied 1.0; lava ×5 → lvl=1 prog=22/36). Re-verify on Sunrise.
- Activation gate: wheel present anywhere in inventory (`minecraft:compass` fallback) required before progress accrues; `wheel_active`/`source` re-derived from inventory.
- Lifecycle: relog persistence CONFIRMED on Horizon (lava lvl=3 across quit→relaunch); death/respawn PARTIALLY CONFIRMED (lava lvl=1 survived); client sync NOT IMPLEMENTED (`sync:false`). Re-run each on Sunrise to confirm parity (see DEATH_RESPAWN_SYNC_PLAN.md, adapt names).
- Console/target selectors: NEW Sunrise API reportedly supports `<targets:players>`, `@a/@r/@s/@e`, real dedicated server — VERIFY, do not assume (S4 plan adapted for Sunrise).

## 4. Evidence checklist
For every run record: `date -Is`, `sc_env` output, exact command + raw JSON reply, `sc command adaptionwheel status` before/after, `sc chat-log`, `sc hello` (API line/features), native build string + `.smod` sha256, and `sunrise-mods.txt`. Bounded logs only (`head -c`/`tail -c`, `timeout`). Write to `/mnt/Barracuda/tmp/opencode/sunrise-battery/runs/`.

## 5. Status / next step
**Runtime CONFIRMED by orchestrator (2026-11-19):** native-build succeeded — `native-ed3ac19-342a01ab4f13` (PR #19 head `c9a3d53` + attachment fix `77928aa`). In-game `sunrise-ctl` run: all six `/adaptionwheel` commands respond (help/status/activate/grant env_fire 2/set env_fall progress 25/reset/status). **Attachment persistence CONFIRMED**: `grant env_lava 3` → quit → relaunch → `lvl=3/3`. Evidence: issue #10 comment `7003283822`.
- Earlier persistence failure was a REAL bug (not an env false alarm): `register_attachments` was a no-op; Sunrise requires `register_attachment_type` (the `Undeclared` error was silently ignored). Fixed in PR #19 `77928aa`; re-run passed.
- Exact env used: `SUNRISE_HOME=/home/alexquasar/.local/share/sunrise`, working binary `/home/alexquasar/sunrise-game-bin/sunrise-client` (copy of native build install due to launcher cache-lease check), socket `/run/user/1000/sunrise-control.sock`, content `adaptionwheel.smod`.
- **Additional CONFIRMED evidence (orchestrator, issue #10 comment `7003334059`):** P1.6 commands `list/info/grantall/registry` respond in-game; target selector `grant @s env_fire 1` works (new Sunrise API supports selectors); `analyze` not registered (expected). Damage progression in a clean world: `give @s minecraft:compass` → `wheel_active=true`; 5× `damage @s 10 minecraft:lava` → `env_lava lvl=1/3 prog=22/36`. `reset` clears state including after `grantall`. PR #20 content `.smod` id `adaptionwheel_content` `ok:true` sha256 `589b6fde…`.
- Battery scripts here (`sh -n` PASS) remain reusable for full re-runs (damage progression for all 7 concepts, target selectors, death/respawn) — the remaining scenarios (full 7-concept sweep, death/respawn, dedicated console) are not yet runtime-confirmed on Sunrise. Do not run guarded launch/quit scripts without orchestrator approval.
- Docs (`INTEGRATION_STATUS.md` + this plan) updated in clean worktree `/mnt/Barracuda/tmp/opencode/aw-sunrise-src`, **NOT committed** — awaiting orchestrator command. Sunrise CI workflow kept as prepared (81/81 + `cargo-sunrise-mod build/check` only; native-build not mandatory yet).
