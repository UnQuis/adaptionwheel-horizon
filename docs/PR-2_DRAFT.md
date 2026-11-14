# PR-2 Draft: Initial MVP slice (scaffold/build, state, activation, scenarios, sync/feedback, debug)

Target Horizon baseline: **139e83fff8** (2026-11-08). See `docs/HORIZON_NOTES.md`, `docs/BUILDING.md`, `docs/MVP_IMPL_CHECKLIST.md`.

## Summary
Initial vertical MVP slice for `adaptionwheel-horizon` with server-authoritative design intent:
- Pin native/content manifests and crate dependencies to Horizon `139e83fff8` (baseline).
- Add content placeholder wheel item (`adaptionwheel:adaptation_wheel`) for activation wiring.
- Implement native MVP scaffold and core logic modules:
  - state schema v1 / attachment integration intent (`PlayerAdaptationStateV1`, codec helpers)
  - temporary activation provider skeleton (offhand/mainhand/hotbar model)
  - adaptation engine + scenario rules (`env_fall`, fire/explosion-oriented MVP logic scaffolds)
  - mod message protocol/sync scaffolding (`state_sync` envelope/full/delta/event)
  - debug command execution helpers
  - hook wiring points (`player_logged_in`, `player_clone`, `living_fall`, `living_incoming_damage`, `net_from_client`)
- Prepare/refresh API gap issue drafts as "ready to file if blocker confirmed" with pinned target and prototype-confirmation criteria.
- Document build/validation steps and MVP implementation checklist/evidence requirements.

## Implemented in this PR branch (`pr2-mvp-slice`)
- `native/adaptation/horizon-mixin.toml`: pinned `horizon_revision = "139e83fff8"`
- `native/adaptation/api/Cargo.toml`: pinned `horizon-client` rev `139e83fff8`, crate metadata for scaffold
- `native/adaptation/api/src/*`: MVP code skeleton/modules and logic for fall/explosion(fire-class) progress & mitigation (server-authoritative intent), state serialization, sync/command helpers
- `content/adaptation/*`: version bump + placeholder wheel item declarations/assets/lang
- `docs/BUILDING.md`, `docs/MVP_IMPL_CHECKLIST.md`: reproducible build/validation instructions and evidence checklist
- `docs/issues/*`: updated drafts (target pinned, HORIZON_NOTES refs, prototype blocker confirmation criteria, filing guidance)

## Build/check/test evidence (current)
- Environment: `cargo 1.99.1`, `rustc 1.99.1`
- Attempted: `cd native/adaptation/api && cargo test` (fetch/build against pinned `horizon-client` rev may be long in this environment; include captured command output/log in PR comments if/when completed)
- Pure logic covered by in-crate unit tests: state defaults, fall progress/activation gating/mitigation, explosion progress/level-up/classification helpers (see `native/adaptation/api/src/lib.rs` and modules)
- Runtime/integration evidence pending (requires Horizon launcher/native-build instance on pinned-compatible setup): attachment lifecycle persistence, inventory activation detection, hook payload mapping for damage sources, mod-net roundtrip, in-game mitigation/progress verification

## Known gaps / blockers (prototype-dependent)
- Exact Host/player handle mapping and attachment set/get ergonomics on target build
- Inventory/item ID inspection APIs for wheel activation
- Damage source/type payload details for explosion/fire/lava coverage in `living_incoming_damage`
- Mod message registration/send/receive wiring details and client HUD hook availability
- `cargo-horizon-mod` / `horizon-launcher native-build` end-to-end validation in this environment

## How to validate (next)
1. Content: `cd content/adaptation && cargo horizon-mod build && cargo horizon-mod check target/horizon-mod/adaptionwheel.hmod --json` (if tool available)
2. Native crate: `cd native/adaptation/api && cargo check` and `cargo test`
3. Runtime prototype: `horizon-launcher native-build native/adaptation --instance <instance> --trust-native-code` against Horizon `139e83fff8`-compatible environment; verify login/init, activation, fall/explosion scenarios, state persistence, sync/debug feedback
4. File/confirm Horizon API issues only after prototype reproduces concrete blocker (see `docs/issues/*`)
