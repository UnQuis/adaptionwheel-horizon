# Readiness evidence register — PRs #4–#14

_Docs-only companion to `docs/reference/ADAPTATION_SYSTEM_MAP.md`. Compiled 2026-11-18 from Ensemble task S2 (read-only inspection of local docs plus GitHub PR/issue pages). No builds/tests/runtime were re-run for this register; every claim is attributed to its source. Owner: Agent #1 (docs/readiness)._

## Stack order and native composition

Heads at audit: #4 `cf987c0`→`main`; #5 `fc581fff`→#4 (older snapshots `70f844d`); #6 `3a715bb`→`main`; #7 `3f044c5`→#5; #8 `4a0d66b`→#5; #9 `9ce3432`→#7; #11 `cac5cf86`→#5; #12 `2f91bdc`→#5; #13 `c0b0569`→#5; #14 `f2db6f3`→`main`.

- **#8/#11 conflict:** both modify `native/adaptation/api/src/lib.rs` on #5. Merge #8 clean; #11 on top → conflict (still against `#5@fc581fff`), recorded in issue #10 comments `6875182718` / `6875209318`.
- **Resolution:** PR #12 (`2f91bdc`, base `#5@fc581fff`) combines #8+#11 (issue #10 `6875615437`). Proposed merge order (issue #10 `6875628303`): **#4 → #5 → #7 → #12 (instead of #8/#11) → #9 → #6**, then revalidate on final heads.

## Per-PR evidence

| PR | Scope | Evidence CONFIRMED (command / env / SHA / result, with source) | Evidence only CLAIMED | Remaining for 100% readiness | Notes |
|---|---|---|---|---|---|
| #4 `cf987c0` | Inventory-anywhere activation (`player_inventory_changed`, API 579, 41-slot), pin `f4d8d62c31` | PR body: `cd native/adaptation/api && cargo test` → **6 passed / 0 failed** against `horizon-client f4d8d62c31` | Not independently rerun; no CI/runtime artifact | Revalidate on integrated head; build/install; runtime E2E; CI | Runtime/CI gap |
| #5 `fc581fff` (older `70f844d`) | Scenarios/engine/commands, creative tab/items/assets/lang, loot/recipes/tags | PR body **10/0**; `docs/INTEGRATION_STATUS.md:35` Agent #4 reproduced **10/0** (Linux, cargo 1.99.1 / rustc 1.94.1); issue #10 `6875078734` orchestrator reproduced | Head variance; no CI | Revalidate on composed native (#12)+content; runtime; CI | Composition via #12 |
| #6 `3a715bb` | Docs-only readiness + conditional API gap drafts | Docs-only PR; local `docs/issues/api-*.md` present | `docs/AGENT1_API_GAP_AND_READINESS.md` not in the shared checkout (lives on this docs branch); head drift | Refresh to final heads/#12/#14/issue #10 evidence | Confirm upstream filing channel with maintainers before filing |
| #7 `3f044c5` | Content schema/build fixes; lang parity `051ff6c` | `cargo-horizon-mod build` + `check --json` **ok:true**, hmod sha256 `5c86ca5fe759f4706aacae4a22bab9b2e628e4b492aca84b19cccf8e94a2dc11`, size 975453 (PR; `HORIZON_NOTES.md:112`; `INTEGRATION_STATUS.md:22,39`); issue #10 `6875637425` reconfirms | Not independently rerun; artifact is head-specific. Later runtime artifacts reference hmod `3cee70ad…` (issue #10 `6971836995/6971913940`) | Rebuild/check final head; runtime content/formats; CI | Runtime/CI pending |
| #8 `4a0d66b` | Attachment-backed persistence fallback | PR body: `cargo check -q` PASS + `cargo test -q` **10/0** (`native/adaptation/api`, `f4d8d62c31`) | No runtime semantics; not independently rerun; superseded by #12 | Runtime persistence semantics or confirm via #12 final head | Conflict with #11 → #12 |
| #9 `9ce3432` | Docs-only runtime blocker / evidence requirements | Docs-only PR, tracked in issue #10 | `docs/CONTENT_RUNTIME_BLOCKER.md` absent from shared checkout | Align with newer partial runtime evidence | Stacked on #7 |
| #10 (issue, not PR) | Merge-readiness gate tracker | Issue #10 records partial `horizon-ctl` runtime on `native-9f42064-67f92751b91f` from #12 `2f91bdc`: six commands respond, inventory activation, persistence across relaunch, lava progression/level-up, death/clone preserves levels (caveats: creative limits mitigation; console/sync/full matrix/final hmod revalidation pending) | — | Complete full runtime matrix on final artifacts | Primary gate tracker |
| #11 `cac5cf86` | Console `perform_console_command` (API 99) | PR body: `cargo test` **10/0** (clean worktree, Linux cargo 1.99.1/rustc 1.94.1); `INTEGRATION_STATUS.md`; issue #10 `6875170693` rerun with `CARGO_TARGET_DIR=/tmp/opencode/cargo-target-pr11` **10/0** (earlier `6875136944` failed due to full `/tmp`) | Runtime console not verified; superseded by #12 | Runtime console check or confirm via #12 | Conflict with #8 → #12 |
| #12 `2f91bdc` | Combined native #8+#11 | `cargo check -q` PASS + `cargo test -q` **10/0** (Linux 1.99.1, `CARGO_TARGET_DIR=/mnt/Barracuda/tmp/cargo-target-combo`, `/tmp/opencode/combine-811`; issue #10 `6875615437`); PR #13 cites independent rerun. Runtime: build `native-9f42064-67f92751b91f` via `horizon-launcher native-build … --instance adaptionwheel-test --ignore-pins --trust-native-code` (~46 s, `/mnt/Barracuda/tmp/native-build-pr12.log`, `HORIZON_HOME` set), headless `horizon-ctl` checks above | Not rerun by this register; matrix incomplete | Full runtime matrix on final artifacts; CI; decide #8/#11 supersession | **Partial runtime confirmed**, not 100% |
| #13 `c0b0569` | Docs-only gate update for #12 | Docs-only PR/commits | Branch-only summaries | Reconcile with final heads/SHAs/issue #10 | Drift risk |
| #14 `f2db6f3` | Docs-only adaptation system map | Docs-only PR (this integration adds §7/§8 and companion docs) | File absent in the shared checkout because it lives on `docs/adaptation-system-map` | Review/merge; refresh after final runtime/merge decisions | Open |

## Cross-cutting gaps

- Partial runtime exists (issue #10, PR #12 build), but the full scenario/mitigation/console/sync/content-runtime matrix and CI are still needed for the 100% gate.
- Heads/SHAs evolve; #8/#11 vs #12 disposition and the final hmod artifact require final-head evidence.
- No confirmed Horizon API blocker (see `docs/API_GAP_BLOCKER_CHECK.md`); all drafts remain conditional. Canonical tracker: `horizon-rust/horizon/issues` — confirm channel with maintainers before filing.
- Upstream Horizon `main` mod API restructuring is a future migration caveat (see `docs/HORIZON_NATIVE_FORMAT_GUIDE.md`).
