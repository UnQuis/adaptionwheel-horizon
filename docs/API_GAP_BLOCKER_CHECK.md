# Horizon API blocker check — status: CONFIRMED, filed upstream

_Docs-only companion to `docs/reference/ADAPTATION_SYSTEM_MAP.md`. Initial read-only investigation: Ensemble task S3 (2026-11-18); updated the same day after the orchestrator reproduced and filed a confirmed blocker. Owner: Agent #1 (docs/readiness)._

## Conclusion

**A Horizon API blocker is now CONFIRMED and filed:** [horizon-rust/horizon#18184](https://github.com/horizon-rust/horizon/issues/18184) — no signed SDK for fresh builds.

S3's earlier finding (no confirmed API blocker) applied to the *adaptation logic* gaps listed below; those remain hypotheses/conditional. The confirmed blocker is in the **native mod build/SDK supply chain** for the upstream-new format:

- `sdk-<sha>/SHA256SUMS` returns **404** for fresh/recent Horizon commits, so the signed SDK required by the new native mod format is unavailable.
- Launcher `0.2.78` (binary `/mnt/Barracuda/tmp/opencode/launcher-0.2.78/horizon-launcher`) recognizes the flat `horizon-mod.toml` with `[native]`, but fails with **`E_SDK_SOURCE`** when it cannot obtain the SDK.
- Launcher `0.2.69` offline path also fails.
- A vendored SDK allows `cargo check` but **not `native-build`** — the blocker is specific to the signed/build path.
- Port-side PR #15 (git dependency on `horizon-mod-api`) passes `cargo check` but does not unblock `native-build`.

Consequence: the migration path from the pinned `f4d8d62c31` trait-hooks format to upstream main's `[native]`/macro format is **blocked** pending a response on issue #18184. This does not invalidate the already validated old-format runtime slice (issue #10).

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

## Filed issue

- **Issue:** https://github.com/horizon-rust/horizon/issues/18184 (canonical tracker `horizon-rust/horizon/issues`). Filed by the orchestrator after reproducing the SDK failure.
- **Title/topic:** signed SDK missing for fresh builds (`sdk-<sha>/SHA256SUMS` 404; launcher `0.2.78` `E_SDK_SOURCE`; `0.2.69` offline; vendored SDK only covers `cargo check`).
- **Status:** awaiting upstream response. While open, do **not** attempt to claim the new native format is buildable; keep the pinned old-format build as the working path.

## Filing policy for the remaining candidate gaps

- The adaptation-logic gaps in the table above are still **not** confirmed blockers — file additional upstream issues only with reproducible prototype/runtime evidence, target/rev, repro, acceptance criteria, and a confirmed channel with Horizon maintainers.
- A filing-ready proposal must contain: title; affected Horizon target/rev/build; component/API area; observed vs expected behavior; minimal repro with prototype branch/commit; concrete artifacts (commands, output, logs, screenshots); why existing hooks/helpers are insufficient; proposed minimal API/contract change; acceptance criteria; channel-confirmation note.
