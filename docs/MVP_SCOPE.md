# MVP Scope (Draft)

## MVP objective
Deliver a minimal, verifiable end-to-end slice of AdaptionWheel on Horizon:
**equip/activate wheel -> track adaptation progress -> gain adaptation -> apply effect -> sync to client -> persist across relog**.

## In scope (MVP)
### 1. Project/scaffold foundations
- Loadable content/native scaffold structure in repo.
- Pinned/identified target Horizon version and notes (or explicit TODO/blockers).
- Feature parity matrix and documented exclusions.

### 2. Wheel activation (temporary model)
- Temporary replacement for Curios slot (explicit server-side rule), e.g. active while held/selected or in a defined inventory condition.
- Single authoritative "wheel active" check used by engine.

### 3. Core state and persistence (minimal)
- Player adaptation state subset: adapted set/levels for selected concepts, basic progress/tasks/timers as needed, schema version.
- Save/load across relog/session in supported Horizon persistence mechanism.
- Safe initialization/migration hooks scaffold (full legacy migration out of MVP).

### 4. Adaptation engine (representative subset)
- Small, representative concept set covering 3-5 scenarios, for example:
  - 1-2 damage-type adaptations (incoming damage mitigation/progression)
  - 1-2 environmental adaptations (fire/lava/fall/drowning subset)
  - (optional) 1 simple movement/environment case if cheap
- Server-side analysis/progression and effect application for selected subset only.

### 5. Basic client feedback and sync
- Minimal synchronization of relevant state/progress.
- Basic HUD/chat/tooltip feedback indicating wheel state and adaptation progress/completion (full wheel UI deferred).

### 6. Debug/verification aids (if available)
- Minimal inspect/grant/reset mechanism for QA (command or equivalent), subject to Horizon command API availability.

## Out of scope (deferred: P1/P2)
- Full Curios-equivalent accessory UX.
- Full adaptation registry parity (all domains, mutations, synergies, existence/bosses, adversity, shedding/transfer breadth, trade depth).
- Sword of Extermination and its advanced projectiles/FX.
- Disciple/entity AI/render parity and custom projectiles.
- Ritual block full behavior parity (brazier/totem/altar trading/auras depth).
- Full wheel/cache/trade GUIs and polished HUD/overlays.
- Advanced rendering/VFX (custom OBJ mesh parity, shaders/cinematics/fog/lightmap hacks).
- External mod integrations (Draconic Evolution etc.).
- Legacy config/data migration from original Java mod worlds.

## Acceptance criteria
- Content/native scaffold loads in target Horizon environment (or documented blocker with evidence).
- Wheel can be activated via temporary model and recognized server-side.
- At least **3 adaptation scenarios** from different representative categories can be triggered and completed.
- Completed adaptations affect gameplay as specified for MVP subset.
- State persists across relog/rejoin (within Horizon-supported persistence scope).
- Client receives enough feedback to observe progress/completion.
- Known gaps/degradations are documented in `docs/HORIZON_NOTES.md` and parity matrix.

## Key risks / blockers affecting MVP
- Availability of hooks/APIs for damage/environment events, player tick, persistence, and custom messages.
- Persistence format/lifecycle for per-player mod data.
- Input/UI hooks for eventual active abilities (not required for MVP core loop, but impacts next steps).
- Native mod build/pin workflow complexity for selected Horizon revision.
