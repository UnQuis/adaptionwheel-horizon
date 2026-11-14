# Horizon Notes (Draft)

## Target
- **Horizon repo (local):** `../Horizon`
- **Target revision/commit:** TBD (pin before native implementation/builds)
- **Hooks API level:** TBD (confirm via `native-hooks.json` / target build)

## Confirmed from initial research
- Horizon supports **content mods (`.hmod`)** and **native mods (`.hnmod`, horizon-mixins).
- `cargo-horizon-mod` templates exist for content and native (trait mod) scaffolds.
- `native-hooks.json` is the catalog of named hook points/helpers; hook contracts and availability must be validated against chosen Horizon revision.
- Mod API / native hooks work may be tracked in Horizon mod API channels/fork per ecosystem guidance (validate before filing issues).

## MVP-relevant capabilities to validate
### Persistence / state
- Per-player persistent custom data storage and lifecycle (join/leave/death/respawn/save/load).
- Item stack custom data persistence (for wheel instance state), if needed.
- Serialization format constraints and migration hooks.

### Events / gameplay hooks
- Incoming/outgoing damage hooks (source/type/entity context).
- Environment/tick hooks for fire/lava/drowning/fall/exposure scenarios.
- Player lifecycle/tick hooks suitable for analysis progression.

### Networking / sync
- Custom server<->client message channel for mod state and intents.
- Reliability/ordering/versioning expectations.

### UI / input
- HUD overlay/text rendering hooks.
- Screen/menu APIs for future wheel/cache/trade UI.
- Keybind/input action registration (future abilities/wheel actions).

## Known likely gaps (from team reports)
- Curios-like accessory slots.
- Rich custom UI/rendering (complex meshes/OBJ/shaders/cinematics).
- High-level persistence and input APIs may be limited or require specific hook patterns.
- NeoForge-style attachments/capabilities/mixins semantics are not directly portable.

## Validation log
| Date | Item | Method | Result | Notes |
|------|------|--------|--------|-------|
| TBD  | Target Horizon revision selected | docs/build metadata | pending | |
| TBD  | Player persistence hooks/helpers | source/docs inspection | pending | |
| TBD  | Damage/env hooks for MVP scenarios | source/docs inspection | pending | |
| TBD  | Custom mod messages/sync | source/docs inspection | pending | |

## Issue candidates (draft, validate before filing)
- Per-player mod data persistence helper/API ergonomics (if needed).
- Accessory slot abstraction / Curios replacement path.
- Keybind/input action registration for native mods.
- Advanced UI/render hooks for wheel/radial and VFX parity (later).
