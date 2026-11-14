# Porting Plan: AdaptionWheel -> Horizon

## Source mod (../adaptionwheel, NeoForge 1.21.1)
- Complex adaptation system: analysis/tasks/timers, leveled and one-time adaptations, domains/concepts, mutations, synergies, shedding, resonance, transfer, existence/boss logic, adversity.
- Content: wheel items, ritual blocks (brazier/totem/resonance altar), altar trade system, Sword of Extermination, Disciple entity, projectiles/effects, advancements, recipes/loot/tags/worldgen, sounds, models/textures/shaders/fonts.
- Runtime systems: player/item persistent data (attachments/data components), server-authoritative networking, commands, config, Curios integration, many mixins and custom client rendering/HUD/UI.

## Horizon modding model (research summary)
- **Content mods (`.hmod`)**: data/assets/manifest-driven content (blocks/items/tabs/effects/entity/menu types via templates and JSON, recipes/tags/loot/advancements/lang/models/textures). Limited/no arbitrary runtime code.
- **Native mods (`.hnmod`, horizon-mixins)**: Rust hooks into Horizon source/build; powerful but trusted, version/pin sensitive (`horizon_revision` / hooks API), rebuild flow via launcher.
- Key constraints: no Fabric/NeoForge runtime compatibility, no direct Curios equivalent, custom rendering/GUI/network/persistence/entity workflows depend on available hooks/APIs and may require gaps to be filled.

## Proposed architecture
- **`adaptation-content` (`.hmod`)**: declarative assets/data/content that fits Horizon content API (items/blocks/tags/recipes/loot/adv/lang/basic models/textures/sounds where possible).
- **`adaptation-core` (`.hnmod`)**: server-authoritative adaptation engine/state, progression rules, persistence, abilities/cooldowns, commands/config, sync/messages, content bindings.
- **Client integration** (separate module or same native mod): HUD/wheel UI/overlays/input handling and VFX where supported by Horizon hooks.

## Phased PR roadmap (draft)
1. **PR-0 Bootstrap**: repo scaffold, docs, build/tooling layout, feature parity matrix baseline.
2. **PR-1 Content foundation**: portable declarative content/assets/data registration.
3. **PR-2 Core data/state**: adaptation definitions/state model, persistence/migration scaffolding.
4. **PR-3 MVP engine**: minimal adaptation analysis/progression + basic effects for a subset of domains.
5. **PR-4 Sync/UI basics**: minimal client-visible state/HUD/menu flow via available APIs.
6. **PR-5 Abilities/items/blocks**: active abilities, wheel interactions, ritual block behaviors where feasible.
7. **PR-6 Advanced parity**: synergies/mutations/existence/trade depth/entities/projectiles as supported.
8. **PR-7 Polish**: VFX/UX parity improvements, balancing/config, docs.
9. **PR-8 Stabilization**: testing, bug fixes, packaging/release readiness.

> PRs are merged only when behavior is verified with confidence.

## Known risks / likely Horizon API gaps (to validate)
- Player/item persistent custom data and migration ergonomics.
- Accessory/Curios-like slot support.
- Custom keybind/input actions and complex interactive UI (wheel/radial).
- Rich entity rendering, custom meshes/OBJ/shaders/VFX parity.
- Networking/custom message patterns and authoritative state sync helpers.
- Entity AI/renderer workflows beyond current template/hook coverage.

When a missing capability is confirmed, create a focused, evidence-backed issue in the appropriate Horizon repo/channel (see agent research on mod API process and `native-hooks.json`/docs precedents).

## Immediate next steps
- Finalize feature parity matrix (MVP/P1/P2) from source analysis.
- Validate Horizon native hook/catalog capabilities for persistence, UI, input, networking, rendering.
- Create first implementation PR (scaffold + parity matrix + initial content/native skeleton).
