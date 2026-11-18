# Building / Validation

## Baselines
- **Merged MVP baseline (main/PR #3):** Horizon `139e83fff8` — `cargo test` in `native/adaptation/api` passed (5/5) for core logic.
- **Current dev target (inventory activation update):** Horizon `f4d8d62c31` (`origin/main`, MC 26.3-era APIs) to support inventory hooks (e.g. `player_inventory_changed`, 41-slot inventory snapshots) so the wheel can activate when present anywhere in player inventory.

## Repository layout
- `content/adaptation/` — Horizon content mod scaffold (`.hmod` source)
- `native/adaptation/` — native trait mod scaffold (`horizon-mixin.toml`, `api/` crate)
- `docs/` — architecture, parity, MVP scope/design/scenarios, Horizon notes, issues

## Prerequisites
- Rust toolchain (edition 2024 compatible)
- Horizon source/build tooling for the pinned revision
- `cargo-horizon-mod` / Horizon launcher for `.hmod` packaging and native builds in an instance

## Content mod (`.hmod`)
From `content/adaptation/` (if `cargo-horizon-mod` available):
- `cargo horizon-mod build`
- `cargo horizon-mod check target/horizon-mod/adaptionwheel.hmod --json`

## Native mod
From `native/adaptation/api/`:
- `cargo check` / `cargo test` against pinned `horizon-client` revision
- Full runtime: `horizon-launcher native-build native/adaptation --instance <instance> --trust-native-code`

## Inventory activation (26.3-era Horizon APIs)
- Server-side activation should consider wheel presence in **any player inventory slot** (hotbar/main inventory/offhand/armor slots as applicable), not only held/offhand.
- Preferred hook: `player_inventory_changed` (API 579, 41 slots) to recompute activation on inventory updates; fallback/recompute on login/respawn/tick as needed.
- Implementation must remain server-authoritative and sync activation/state changes to clients when networking is wired.
