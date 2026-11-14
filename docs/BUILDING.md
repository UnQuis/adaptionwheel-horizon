# Building / Validation (PR-2 MVP slice)

## Baseline
- Horizon target (validation baseline): `139e83fff8` (`../Horizon`, 2026-11-08)
- Native mod pinned in: `native/adaptation/horizon-mixin.toml` (`[compatibility].horizon_revision`)
- Native crate dep pinned in: `native/adaptation/api/Cargo.toml` (`horizon-client` git `rev = "139e83fff8"`)

## Repository layout
- `content/adaptation/` — Horizon content mod scaffold (`.hmod` source: `horizon-mod.toml`, `assets/`, `data/`)
- `native/adaptation/` — native trait mod scaffold (`horizon-mixin.toml`, `api/` crate)
- `docs/` — architecture, parity, MVP scope/design/scenarios, Horizon notes, issues

## Prerequisites (expected)
- Rust toolchain (edition 2024 compatible; repo environment: `cargo 1.99.1`, `rustc 1.99.1`)
- Access to Horizon source/build tooling matching pinned revision for native builds
- `cargo-horizon-mod` / Horizon launcher for packaging/running `.hmod` and native bundle builds in an instance

## Content mod (`.hmod`) checks
From `content/adaptation/` (when `cargo-horizon-mod` is available):
- `cargo horizon-mod build` → produces `target/horizon-mod/adaptionwheel.hmod`
- `cargo horizon-mod check target/horizon-mod/adaptionwheel.hmod --json`

Manual structure currently includes:
- `content/adaptation/horizon-mod.toml`
- placeholder wheel item declaration/model/assets (`adaptionwheel:adaptation_wheel`)

## Native mod scaffold checks
From `native/adaptation/api/`:
- `cargo check` — validates crate structure/deps/API alignment against pinned `horizon-client` (may require fetching Horizon git dependency and compatible environment)
- `cargo test` — runs pure logic unit tests in `src/lib.rs` (state/progress/mitigation helpers)

> Note: Full runtime validation requires Horizon launcher/native-build flow with a compatible instance and trusted native code build:
> `horizon-launcher native-build native/adaptation --instance <instance> --trust-native-code`

## Current PR-2 evidence status
- Implemented: pinned manifests, MVP native state/logic scaffold (`player_logged_in`, `player_clone`, `living_fall`, `living_incoming_damage`, `net_from_client` hook wiring intent), content wheel item placeholder, unit-testable core logic.
- Pending runtime confirmation: attachment persistence behavior on target build, exact Host/player handle mapping in hooks, inventory-based wheel activation, mod message roundtrip, in-game mitigation/progress verification.
