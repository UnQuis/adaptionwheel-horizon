# Sunrise — primary migration target (user decision 2026-11-19)

_Docs-only status page. Authoritative source: PR [#19](https://github.com/UnQuis/adaptionwheel-horizon/pull/19) and the orchestrator's decision message. Horizon is now an archive/alternative path._

## Decision

The main target for the AdaptionWheel port is **Sunrise** (`sunrise-org/sunrise`), not Horizon.

### Why
1. **Officially signed SDK** is published and fetchable: `sdk-ed3ac19558d8dc1092c431312848cbb58fb01e41` (2026-11-19 release). This removes the SDK supply-chain blocker that affected the new Horizon native format.
2. **The port is mechanically trivial**: on top of the migrated core (PR #17 `afcc334`), only two source edits were required — `#[mod_api::horizon_mod]` → `#[mod_api::sunrise_mod]` and `LevelRandomHost` → `HitHost` (for `living_incoming_damage`). No behavior changes.

## Evidence (confirmed, PR #19)

- New tree `native/adaptation_sunrise/`, deliberately separate from `native/adaptation_v2/` (Horizon new-API path).
- SDK is gitignored and fetched from the signed Sunrise release; no fork launcher or custom key needed.
- Environment: Linux, rustc 1.99.1, `TMPDIR=TEMP=TMP=/mnt/Barracuda/tmp`, `CARGO_TARGET_DIR=/mnt/Barracuda/tmp/cargo-target-sunrise-2`.
- `cargo check` — clean (pre-existing dead-code warnings only).
- `cargo test` — **81 passed / 0 failed**.
- Runtime validation is **not** claimed in PR #19.

## Tooling / runtime status (orchestrator, 2026-11-19)

- Sunrise launcher **1.1.1** installed at `/mnt/Barracuda/tmp/opencode/sunrise-launcher/sunrise-launcher`.
- `nightly-2026-11-19-ed3ac19` install + prepare completed; profile `sunrise`.
- `native-build` was running in the background at the time of this update — no result recorded here yet.

## Next gates (P0)

1. Finish native-build with the installed Sunrise launcher; record exact command, flags, build id, logs, environment.
2. `cargo sunrise-mod build` (or launcher `native-build`) on the final PR #19 head.
3. In-game `sunrise-ctl` validation: commands, inventory activation, persistence across relog/death, target selectors; capture logs/artifacts.
4. Content parity/rebuild for the Sunrise target (PR #18 wheel model texture + metal shader is separate work).
5. CI checks for native tests + content build/check; then final merge decisions.

## Horizon status (archive / alternative)

- Horizon SDK issue [horizon-rust/horizon#18184](https://github.com/horizon-rust/horizon/issues/18184) is **closed as our own setup error**; it is not an upstream defect and no longer blocks the port.
- PRs **#15–#17** (Horizon new-API migration, P1 core mechanics, integration) and the old pinned stack **#4–#12** remain as archive/alternative. Do not spend further gating effort on them unless the Sunrise path fails.
- Historical Horizon evidence (unit tests, partial runtime via `horizon-ctl` on `native-9f42064-67f92751b91f`) remains valid as lineage evidence for the migrated core, not as Sunrise runtime evidence.

## Cross-references

- `docs/reference/ADAPTATION_SYSTEM_MAP.md` — upstream Java → native system map (lineage shared by both targets).
- `docs/API_GAP_BLOCKER_CHECK.md` — Horizon blocker closure and remaining candidate gaps.
- `docs/READINESS_EVIDENCE_REGISTER.md` — per-PR evidence, including #15–#19.
- `docs/MIGRATION_WORK_PLAN.md` — Sunrise-first P0/P1/P2 plan.
- `docs/HORIZON_NATIVE_FORMAT_GUIDE.md` — archived Horizon format reference.
