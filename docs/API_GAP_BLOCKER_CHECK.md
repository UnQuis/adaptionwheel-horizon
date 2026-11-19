# Blocker check — Horizon SDK issue CLOSED (our error); Sunrise primary

_Docs-only companion to `docs/reference/ADAPTATION_SYSTEM_MAP.md`. Initial read-only investigation: Ensemble task S3 (2026-11-18); confirmed/filed the same day; updated 2026-11-19 after the user decision to target Sunrise. Owner: Agent #1 (docs/readiness)._

## Conclusion (updated 2026-11-19)

**[horizon-rust/horizon#18184](https://github.com/horizon-rust/horizon/issues/18184) is CLOSED as our own setup error** — it was not an upstream Horizon defect. The reported symptoms (`sdk-<sha>/SHA256SUMS` 404, launcher `0.2.78` `E_SDK_SOURCE`, `0.2.69` offline, vendored SDK allowing only `cargo check`, PR #15 git-dependency path) were caused by our attempt to build against an unsigned/unavailable SDK, not by a missing Horizon API.

**Consequence:** the former Horizon SDK blocker no longer blocks the work. Per user decision, the primary target is now **Sunrise**, whose official signed SDK (`sdk-ed3ac19558d8dc1092c431312848cbb58fb01e41`, `sunrise-org/sunrise`) is available; PR #19 ports the migrated core with two mechanical API edits and passes `cargo test` **81/81**. See `docs/SUNRISE_TARGET_STATUS.md`.

S3's original finding still applies to the *adaptation logic* gaps below: none of them is a confirmed Horizon API blocker, and no additional upstream issues have been filed. Horizon PRs #15–#17 are archive/alternative.

## Evidence reviewed (paths / refs)

- `docs/HORIZON_NOTES.md:4-9` (pinned baseline `139e83fff8`), `:46-51` (likely gaps), `:66` (advanced UI/render deferred), `:76-88` (inventory activation on `f4d8d62c31`), `:96-115` (packaged-but-unverified formats/assets, runtime verification blocker).
- `docs/issues/api-accessory-wheel-slot.md`, `api-input-keybind-actions-wheel.md`, `api-mod-messages-native-sync.md`, `api-persistence-ergonomics-conditional.md` — all marked "ready to file if blocker confirmed" / conditional; prototype-confirmation criteria largely unchecked (one compile/test criterion now checked for persistence).
- `docs/HORIZON_API_GAPS_NATIVE_CORE.md:7-13` (confirmed available: inventory activation, lifecycle/hooks, commands, attachment primitives), `:18-25` (conditional gaps), `:34-36` (validate before filing).
- `docs/INTEGRATION_STATUS.md:22-43,52-59,69` (mostly unit/content-build evidence; no CI/runtime at time of writing; file upstream issues only with concrete evidence).
- `native/adaptation/README.md:7-17,38-48` (pin `f4d8d62c31`; PR #12 build/runtime evidence; upstream API restructuring noted separately).
- GitHub: issue #10 (gate tracker; partial runtime evidence), PR #8 (persistence fallback), PR #12 (combined native).

## Candidate gaps still at hypothesis stage, and evidence required

| Candidate gap | Current status | Evidence needed to confirm (before any upstream issue) |
|---|---|---|
| Accessory / wheel slot (Curios-equivalent) | hypothesis / probable | Prototype on target Horizon build attempting existing workarounds (held/inventory rule, custom menu/virtual slot); record hooks/helpers tried, exact UX/sync/persistence/conflict limitations, repro branch/commit, logs/screenshots/video |
| Input/keybind actions and wheel UX contract | conditional | Prototype with `mod_key`/`set_mod_key`/input/screen hooks on target build; show missing/insufficient settings discoverability, press/hold/release/context/consume/conflict handling or radial input capture; if stable UX possible, gap is not a blocker |
| Native mod-net API surface/contract for sync | conditional | Minimal native send/receive prototype on target build via `horizon:mod_net` helpers; document available Rust symbols/signatures, registration/lifecycle, player identity, payload/limits/errors, and the exact failure preventing reliable wiring |
| Persistence runtime semantics / ergonomics | conditional (compile/test done; relaunch + clone/death now evidenced) | Survival-mode/cross-dimension/server-restart/flush-timing and payload/error behavior on target launcher; data-loss/corruption repro if any; otherwise close as "current API sufficient" |
| HUD/UI feedback for MVP indication | to validate | Prototype/runtime test showing inability (or concrete limits) to provide minimal progress/completion feedback via available HUD/screen/mod-screen APIs, with screenshots/logs and exact API constraints |
| Content/runtime format support (OBJ models, shaders, fonts, client item defs) | to validate | In-game verification with final native+content stack on target instance; if unsupported and no viable alternative, scoped request with runtime logs/screenshots |

## Former issue (closed)

- **Issue:** [horizon-rust/horizon#18184](https://github.com/horizon-rust/horizon/issues/18184), filed by the orchestrator after reproducing the SDK failure.
- **Title/topic:** signed SDK reportedly missing for fresh builds (`sdk-<sha>/SHA256SUMS` 404; launcher `0.2.78` `E_SDK_SOURCE`; `0.2.69` offline; vendored SDK only covered `cargo check`).
- **Status (2026-11-19): CLOSED as our own setup error.** Do not cite it as an upstream defect or as a current blocker. The Horizon new-format path remains archive/alternative; Sunrise is primary.

## Filing policy for the remaining candidate gaps

- The adaptation-logic gaps in the table above are still **not** confirmed blockers — file additional upstream issues only with reproducible prototype/runtime evidence, target/rev, repro, acceptance criteria, and a confirmed channel with Horizon maintainers.
- A filing-ready proposal must contain: title; affected Horizon target/rev/build; component/API area; observed vs expected behavior; minimal repro with prototype branch/commit; concrete artifacts (commands, output, logs, screenshots); why existing hooks/helpers are insufficient; proposed minimal API/contract change; acceptance criteria; channel-confirmation note.
