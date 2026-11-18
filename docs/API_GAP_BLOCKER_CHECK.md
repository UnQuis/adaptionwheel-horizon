# Horizon API blocker check — status: NO CONFIRMED BLOCKER

_Docs-only companion to `docs/reference/ADAPTATION_SYSTEM_MAP.md`. Result of Ensemble task S3 (read-only investigation, 2026-11-18). No issue was filed externally. Owner: Agent #1 (docs/readiness)._

## Conclusion

**No Horizon API blocker is confirmed with reproducible evidence.** All identified gaps remain hypotheses/conditional, pending prototype or runtime evidence. What *is* confirmed is an **environment/runtime verification blocker** — no fully equipped launcher/instance/GUI/CI in the agents' environments — which has since been partially mitigated for the native slice (issue #10, PR #12 runtime evidence), but that is not a Horizon API defect.

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

## Filing decision and required proposal contents

- **No external issue filed.** File only when a blocker is confirmed with reproducible evidence, target/rev, repro, acceptance criteria, and after confirming the correct channel with Horizon maintainers (canonical reference: `horizon-rust/horizon/issues`).
- When confirmed, a filing-ready proposal must contain: title; affected Horizon target/rev/build; component/API area; observed vs expected behavior; minimal repro with prototype branch/commit; concrete artifacts (commands, output, logs, screenshots); why existing hooks/helpers are insufficient; proposed minimal API/contract change; acceptance criteria; channel-confirmation note.
