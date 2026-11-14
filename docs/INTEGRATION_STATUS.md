# Integration & Verification Status (Agent #4)

Last updated: 2026-11-14 (local audit)  
Branch/worktree: `feat/creative-tab-adaptation-logic` @ `d7cbf17`  
Port repo: <https://github.com/UnQuis/adaptionwheel-horizon>  
Upstream Horizon: <https://github.com/horizon-rust/horizon> (local checkout referenced in prior work: `../Horizon`, baseline notes in docs)  
Source mod assets/code reference: `../adaptionwheel`

## Scope (Agent #4)

Integration and coherence of `content/` + `native/` in this repo: Horizon content declarations/assets/data parity where supported, command/hook wiring on available Horizon API, build/test verification where possible, documented limitations/evidence, Horizon API gap tracking/issues with evidence, and merge-readiness gating (merge only with 100% confirmed working evidence for the claimed scope).

## Current PRs (as audited)

| PR | Branch | Base | State | Checks | Notes |
|---|---|---|---|---|---|
| #4 | `feat/inventory-activation-26.3` | `main` | OPEN, MERGEABLE | none | Wheel activation from any inventory slot (player_inventory_changed, 41-slot snapshot), Horizon target pin `f4d8d62c31`, asset/data port + commands in scope per PR. Claimed `cargo test` 6/6 (not separately re-run on PR #4 head in this audit). |
| #5 | `feat/creative-tab-adaptation-logic` | `feat/inventory-activation-26.3` (stacked) | OPEN, MERGEABLE | none | Adds missing `assets/adaptionwheel/items/*.json` refs, creative tab data/lang, expanded adaptation scenarios/commands/engine logic. Claimed `cargo test` 10/10. Remote head audited earlier: `2887d23`; local includes pushed fix `d7cbf17`. |

**Stack:** PR #5 is stacked on PR #4; merge-readiness must consider base correctness and merge order/composed result.

## Reproduced verification (this audit)

Environment: Linux, `git 2.56.1`, `cargo 1.99.1 (rustc 1.94.1 host)`, `gh 2.104.0`. `cargo-horizon-mod` **not installed**. No Horizon launcher/instance available for native-build or runtime.

| Check | Command / context | Result | Evidence |
|---|---|---|---|
| Native unit tests (stack head / PR #5 scope) | `cd native/adaptation/api && cargo test` | **PASS** (10/10) | `activation_detects_wheel_anywhere_in_inventory`, `commands_set_and_grant_new_concepts`, `explosion_damage_classifier`, `damage_type_classification_for_new_concepts`, `damage_adaptation_applies_mitigation_and_progress`, `fall_progress_and_mitigation`, `explosion_progress_levels_up`, `fall_requires_activation`, `state_defaults_and_summary`, `leveled_concepts_progress_with_activation`; doc-tests 0. Warnings: dead_code (unused helpers/consts). |
| Native unit tests on PR #4 head only | not run separately in this audit | **not reproduced** | PR #4 claimed 6/6; current stack (includes PR #4 changes) is green on 10 tests. |
| Content build (`.hmod`) | `cargo horizon-mod build` (content/adaptation) | **not run** | tool unavailable (`cargo-horizon-mod` missing). |
| Native build / dry-run | `horizon-launcher native-build ...` or equivalent | **not run** | launcher/instance/tooling unavailable. |
| Runtime / in-game behavior | Horizon client/instance | **not run** | no target runtime environment. |
| CI status checks | GitHub PR checks | **none** | PR #4/#5 report no status check rollup. |

## Integration findings & fixes

- **Creative tab duplication (fixed, pushed to PR #5 branch):** remote `2887d23` added `data/adaptionwheel/horizon/creative_tab/adaptionwheel.json` while `main.json` also existed, creating duplicate tab definitions with overlapping items. Local fix commit `d7cbf17` (`fix(content): deduplicate creative tab and clean lang keys`) removes `adaptionwheel.json` and unused `itemGroup.adaptionwheel.adaptionwheel` lang keys, keeping a single tab `main.json` (`itemGroup.adaptionwheel.main`) listing all 8 core ported items/blocks. Pushed to `origin/feat/creative-tab-adaptation-logic`.
- **Content declarations (current head):** 3 Horizon block declarations + 8 Horizon item declarations under `data/adaptionwheel/horizon/{block,item}`; 8 `assets/adaptionwheel/items/*.json` model refs present; `assets/adaptionwheel` contains 73 files, `content/adaptation/data` contains 13 files (including 1 creative tab). Prior asset-port audit (earlier session) confirmed 64/64 source files from `../adaptionwheel/src/main/resources/assets/adaptionwheel` present in `content/adaptation/assets/adaptionwheel/**` with Horizon-specific additions.
- **Worktree notes:** unstaged deletions of `content/adaptation/assets/adaptionwheel/models/entity/DharmaChakra.blend` and `DharmaChakraClean.blend` observed (OBJ counterparts remain; `.blend` sources present in `../adaptionwheel` and copied to untracked `docs/reference/models_entity/` locally). Untracked `docs/AGENT1_API_GAP_AND_READINESS.md` present (not authored by this audit). Treat uncommitted/untracked items separately from PR evidence.

## Merge-readiness blockers / gaps (for 100% confirmed working)

1. **Reproducible content build:** run and pass `cargo horizon-mod build` for `content/adaptation` with toolchain evidence.
2. **Reproducible native build/load:** native-build/dry-run (or equivalent) against compatible Horizon/instance; confirm API/hook compatibility (native pins target `f4d8d62c31` in PR #4 scope).
3. **Runtime validation (target Horizon):** mod loads; blocks/items/creative tab (single) and assets resolve correctly; wheel activation via inventory snapshot behaves as intended; `/adaptionwheel` commands (`status/activate/reset/grant/set/help` as implemented) respond correctly to player (and console where supported); implemented adaptation scenarios behave as expected.
4. **CI/automated checks:** add checks for at least `cargo test` (+ content build where possible) to prevent regressions.
5. **Stack/PR hygiene:** ensure PR #5 contains dedup fix (done on remote branch now), verify composed result with PR #4 base, confirm no duplicate/conflicting content registrations.
6. **Functional parity limits:** large portions of original mod data/gameplay remain unported or Horizon-limited (e.g. extensive data parity: recipes/loot/tags/advancements/worldgen/structures beyond current, Curios accessory slot, Forge-specific biome modifiers/custom data formats, full adaptation concept coverage, advanced UI/VFX/entity rendering behaviors). Console command handler (`perform_console_command`, API 99) and persistence/attachment-backed state sync require confirmation/implementation per target API.

## Horizon API gaps / issue tracking

- Existing draft issue docs in repo: `docs/issues/api-accessory-wheel-slot.md`, `docs/issues/api-input-keybind-actions-wheel.md`, `docs/issues/api-persistence-ergonomics-conditional.md` (prepared in prior PR-1/PR-2 work; validate against current prototype evidence before filing).
- Upstream mod/API repos referenced historically: `horizon-rust/horizon`, `horizon-rust/horizon-mods`, and tracker discussions (e.g. `arismoko/horizon`) — confirm correct filing target per gap before submission.

## Next actions (Agent #4)

- Continue content/native registration audits and fix concrete mismatches with evidence.
- Run/add reproducible checks when tooling becomes available; capture logs/results.
- Coordinate with Agent #1/#2/#3 on parity criteria, content correctness, and native/runtime evidence for PR #4/#5.
- Prepare/file justified Horizon API issues only when blockers are evidenced.
