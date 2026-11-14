# Architecture (Draft)

## Goals
- Port AdaptionWheel to Horizon with a pragmatic split between declarative content and native runtime logic.
- Keep server-authoritative gameplay logic for adaptation progression/effects.
- Isolate Horizon-specific integration points behind clear modules so API gaps can be addressed incrementally.

## Source and target
- **Source:** `../adaptionwheel` (NeoForge 1.21.1, Java) — complex adaptation systems, custom data/network/UI/render/mixins.
- **Target:** Horizon (Rust Minecraft reimplementation) using:
  - **Content mod (`.hmod`)** for declarative assets/data/registrations supported by Horizon content API.
  - **Native mod (`.hnmod`)** for gameplay logic and hook-based integrations not expressible in content-only form.

## Proposed repository layout
- `content/adaptation/` — `.hmod` source (manifest, `assets/`, `data/`)
- `native/adaptation/` — `.hnmod` scaffold (native hooks/trait mod crate)
- `docs/` — architecture, parity matrix, MVP scope, Horizon notes, decisions
- `.github/` — PR/issue templates and workflow conventions

## High-level modules (planned)
1. **adapt_defs**: adaptation definitions/domains/concepts, tiers, static tables.
2. **adapt_state**: runtime player/wheel state (levels, adapted, tasks/progress, counters, flags, disabled/cache).
3. **adapt_engine**: analysis/progression rules and effect application (damage/env/movement/combat/etc.).
4. **adapt_abilities**: active abilities, cooldowns, server validation.
5. **adapt_persist**: save/load, schema versioning, migrations (player/item/world as supported).
6. **adapt_net**: custom messages/sync/intents, protocol versioning.
7. **adapt_ui**: HUD/wheel/trade/cache UI integration (as Horizon APIs allow).
8. **adapt_fx**: rendering/VFX integration (degraded parity acceptable when necessary).
9. **adapt_config**: server/client config, defaults, migration handling.
10. **adapt_content_bindings**: glue between declared content and runtime behavior.

## Runtime principles
- **Server authoritative:** adaptation grants, progression, prices/cost validation, and state mutation are decided server-side.
- **Client mirror:** client receives synchronized state for display/input affordances; avoid trusting client for progression.
- **Deterministic state transitions:** prefer explicit state machines for analysis/tasks/cooldowns.
- **Versioned data/protocol:** schema and network message versions from first implementation.

## Known integration challenges (to validate)
- Curios-like accessory slot replacement.
- Player/item persistent custom data APIs and lifecycle.
- Custom networking/message channels and sync patterns.
- Input/keybind handling for abilities/wheel actions.
- Complex UI and advanced rendering (custom meshes/shaders/cinematics).
- Movement/physics and specialized mixin-equivalent hook coverage.

## Incremental delivery strategy
- Start with scaffold + parity matrix + MVP vertical slice.
- Implement smallest end-to-end loop (activation, state persistence, a few adaptation scenarios, basic sync/UI feedback).
- Expand by prioritized domains; create evidence-backed Horizon API issues for confirmed gaps.
