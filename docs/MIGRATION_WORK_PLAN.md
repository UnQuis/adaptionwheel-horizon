# Migration work plan — AdaptionWheel native port (Sunrise primary)

_Docs-only companion to `docs/reference/ADAPTATION_SYSTEM_MAP.md` §§4–5 (Ensemble task S5, 2026-11-18; reconciled 2026-11-19 to the user decision: **Sunrise primary**, Horizon archive/alternative). Owner: Agent #1 (docs/readiness); execution owners are the role IDs below._

**Target decision (2026-11-19):** Sunrise is primary — official signed SDK (`sdk-ed3ac19558d8dc1092c431312848cbb58fb01e41`), two mechanical API edits from the migrated core, PR #19 `cargo test` **81/81**. Horizon SDK issue #18184 closed as our own setup error. See `docs/SUNRISE_TARGET_STATUS.md`.

## Roles
- **#1 Docs/Readiness:** docs, evidence register, gap register, gate packets, tracker hygiene.
- **#2 Content/Assets:** content/data/assets, schema/build, portability.
- **#3 Native Core:** hooks/logic/commands/persistence/sync, unit evidence, API validation.
- **#4 Integration/Verification:** stack composition, reproducible build/test, runtime/CI evidence, merge gate.

## P0 — 100% merge-readiness gate

| ID | Task | Dependencies | Owner | Acceptance | Evidence gate |
|---|---|---|---|---|---|
| **P0.0** | **Sunrise native-build + runtime gate (primary)**: PR #19 head, signed SDK, installed launcher 1.1.1 (`/mnt/Barracuda/tmp/opencode/sunrise-launcher/sunrise-launcher`), profile `sunrise`, nightly `nightly-2026-11-19-ed3ac19` | #19 | #3,#4,#2 | Build succeeds and in-game matrix passes: commands, activation, persistence, target selectors | exact command/flags/build id/logs/env + `sunrise-ctl` artifacts; content rebuild for #18 |
| P0.1 | Compose parallel native #8/#11 via #12 on final #5 base; decide #8/#11 supersession (archive Horizon path only) | final heads | #3,#4 | Clean composed state, both features preserved | SHAs/refs; `cargo check -q && cargo test -q` PASS; review notes |
| P0.2 | Reproduce native tests on canonical heads and composed state | P0.1 | #4,#3 | PASS with recorded toolchain | commands + cargo/rustc versions + SHA + test counts |
| P0.3 | Reproduce content build/check on final artifact | P0.1, #7 schema fixes | #2,#4 | `cargo-horizon-mod build` + `check --json` ok | tool version, artifact path, sha256/size, SHA (confirm/supersede `5c86ca5f…` / `3cee70ad…`) |
| P0.4 | Complete runtime matrix on final native+content stack | P0.2,P0.3 | #4,#2,#3 | Each case pass/fail: six commands (incl. console), 41-slot activation, fall/explosion/fire/lava/drown/contact/general progress+mitigation **outside creative mode**, relog/clone/respawn persistence, content render/formats | steps/commands + logs/screenshots/video + SHAs/env per case (builds on issue #10 partial evidence) |
| P0.5 | Minimal CI: native tests + content build/check | P0.2,P0.3 stable | #1,#4,#2,#3 | PR checks run on relevant SHAs | workflow/check URLs + green runs + local parity commands |
| P0.6 | Stack/docs/gate reconciliation and merge decision | P0.1–P0.5 | #1,#4 | Exact evidence in docs/PRs; merge order explicit; only fully evidenced scopes merge | updated map/register/evidence docs; blockers/rebase notes |

Proposed order: **primary Sunrise path — #17 → #19 → runtime validation (#18 content as ready)**; archived Horizon path (only if Sunrise fails) — #4 → #5 → #7 → #12 (preferred over #8/#11) → #9 → #6, with docs #13/#14 reconciled; revalidate on final heads.

## P1 — core MVP parity and validated gaps

| ID | Task | Dependencies | Owner | Acceptance | Evidence gate |
|---|---|---|---|---|---|
| P1.1 | Attachment persistence semantics closure (save/flush, schema v1, survival/cross-dimension/restart) | P0.1,P0.4 | #3 | Deterministic semantics or explicit documented fallback | runtime relog/clone/respawn logs + unit tests where feasible |
| P1.2 | Confirm mod_net native surface; minimal versioned S→C sync if supported | P0.4 + prototype | #3,#4 | Working minimal sync or confirmed limitation + docs | prototype/runtime proof; issue only if blocker confirmed |
| P1.3 | Harden activation + MVP scenarios (server-authoritative, active-gated) | P0.4,P1.1 | #3 | Spec behavior; unclassified paths not claimed | per-scenario tests/traces + runtime evidence |
| P1.4 | Generalize concept model/registry (domains, leveled/one-time, upstream ids) | P0.6 | #3 | Registry-driven, not 7 hard-coded ids | unit tests + parity doc update |
| P1.5 | Core content/data parity in supported runtime | P0.3,P0.4 | #2 | portable subset loads/behaves; unsupported de-scoped | build/check + in-game evidence + `CONTENT_DATA_PARITY.md` |
| P1.6 | Debug/inspection command flows (status/grant/reset/set/activate/help, console) | P0.1,P0.4 | #3,#4 | reproducible player+console verification UX | transcripts + tests + runtime evidence |
| P1.7 | Refresh readiness/evidence/gap docs | P0.6 + S1–S4 | #1,#4 | confirmed/reported/unverified/blocked labeled with full refs | doc/PR diffs + evidence cross-links |

## P2 — extended parity (milestone-based, after P1 + per-feature Horizon validation)

| ID | Task | Dependencies | Owner | Acceptance | Evidence gate |
|---|---|---|---|---|---|
| P2.1 | Accessory/wheel slot + keybind/wheel UX (if supported) | P1.3, API validation | #3,#2,#1 | subset implemented or deferral with confirmed constraints | prototype/runtime evidence; issue only if confirmed blocker |
| P2.2 | HUD/UI/client feedback expansion | P1.2,P1.6, UI validation | #3,#2,#4 | supported indication or explicit degradation | screenshots/video + capability notes |
| P2.3 | Broader adaptation domain/scenario parity (env/movement/entity/existence/mutations) | P1.3,P1.4, hooks | #3,#2,#4 | defined behavior + verified hooks per addition | per-scenario tests + runtime evidence + parity updates |
| P2.4 | Synergies, fist ladders, passive 400-cap bonuses, cache/altar/menus, entities/FX | P1 foundations + API maturity | #3,#2,#4,#1 | scoped, evidenced milestones | design + prototype/runtime evidence + confirmed-gap issues |

## Dependencies

- Critical path: P0.1 → P0.2/P0.3 → P0.4 → P0.6 (P0.5 parallel after stabilization).
- Stabilize: P1 after relevant P0 evidence; P1.1/P1.2 unblock P1.3+.
- Expand: P2 only after P1 and per-feature Horizon validation.

## Global evidence gates

- Every claim includes: command(s), environment/tool versions, commit/PR SHA(s), and runtime artifacts where applicable.
- Label each claim **confirmed / reported / unverified / blocked**.
- File upstream Horizon issues only with concrete blocker evidence, target/rev, repro, acceptance criteria, and a confirmed channel (`horizon-rust/horizon/issues`; verify with maintainers).

## Risks / open items

- Horizon `main` new-format path (#15–#17) and the old pinned stack (#4–#12) are **archive/alternative** after the 2026-11-19 decision. Issue [#18184](https://github.com/horizon-rust/horizon/issues/18184) was closed as our own setup error — no upstream blocker. Do not gate the Sunrise work on Horizon PRs.
- Sunrise runtime evidence is still missing: PR #19 proves compile/test only (81/81), not in-game behavior. `native-build` status must be recorded before any merge-readiness claim.
- No CI/status checks on PRs; heads/SHAs drift; final hmod artifact (`5c86ca5f…` vs later `3cee70ad…`) must be reconciled.
- Do not claim unsupported content/data parity (`docs/CONTENT_DATA_PARITY.md`).
