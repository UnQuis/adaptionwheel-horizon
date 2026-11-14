# Native module skeleton (`.hnmod`, PR-2 MVP slice)

Target: **Horizon commit `139e83fff8`** (pinned in `horizon-mixin.toml` and `api/Cargo.toml`).  
Design source: `docs/MVP_TECH_DESIGN.md`.

## Structure
- `horizon-mixin.toml` — native mod manifest (id/version, pinned `horizon_revision`, api path/hooks).
- `api/Cargo.toml` — Rust crate for trait hooks (pinned `horizon-client` rev).
- `api/src/lib.rs` — entry point (`hooks()`), mod wiring, registration points.
- `api/src/state.rs` — attachment schema v1 (`PlayerAdaptationStateV1`, `ConceptId`, migrations, encode/decode helpers).
- `api/src/activation.rs` — temporary wheel activation provider (`InventorySlotProvider`, offhand/mainhand modes).
- `api/src/engine.rs` — server-authoritative analysis/mitigation pipeline.
- `api/src/scenarios/` — MVP scenario rules/stubs:
  - `fall.rs` — `env_fall` (2nd MVP scenario stub for this slice)
  - `fire.rs` — `env_fire` (1st MVP scenario stub for this slice)
  - `mod.rs` — `ScenarioContext`, registry, shared progress application.
- `api/src/net.rs` — mod messages protocol/API-40 envelope (full/delta/event sync), client mirror, channel constants.
- `api/src/commands.rs` — debug commands (`inspect|grant|reset`) execution and registration points.
- `api/src/config.rs` — MVP defaults/thresholds and activation tuning.
- `api/src/hooks.rs` — Horizon hook wiring points (login, damage/fall/HUD placeholders), server event handling helpers.

## Integration points (TBD against Horizon API @139e83fff8)
- **Attachment registration:** `adaptionwheel:player_state`, serializer, `copy_on_death` policy.
- **Hooks/events:** `living_incoming_damage`, `living_fall`, player lifecycle/join/respawn/tick, inventory/hand change (if available), HUD render, mod message receive.
- **Mod messages:** channel registration and send/receive for `adaptionwheel:state_sync` (and optional ack/client_event).
- **Commands:** command tree registration and permission checks.
- **Item identity:** resolve wheel item IDs from content (`adaptionwheel:mahoraga_wheel[_wood]`).

## Notes
- Server is authoritative for activation, progress, completion, mitigation, and sync emission.
- This is an implementation-ready skeleton with explicit `TODO(Horizon API)` wiring points; compile validation requires the Horizon 139e83fff8 toolchain/build environment (coordinate with Agent #2).
