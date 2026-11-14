# Decisions (ADR-lite)

## D-001: Split implementation into content (`.hmod`) and native (`.hnmod`)
- **Status:** Accepted (draft)
- **Context:** AdaptionWheel includes both declarative content/assets and complex runtime behavior not representable as content-only mod data.
- **Decision:** Use `.hmod` for portable declarative content/data and `.hnmod` for adaptation engine, persistence, sync, and hook-driven behavior.
- **Consequences:** Requires native build/pin workflow and Horizon hook validation; enables incremental port where content-only is insufficient.

## D-002: Temporary wheel activation model (Curios replacement)
- **Status:** Proposed
- **Context:** Original mod depends on Curios wheel slot. Horizon may not provide equivalent accessory API.
- **Decision:** Implement temporary server-authoritative activation rule for MVP (e.g. held/selected inventory condition), to be replaced by better slot integration if/when supported.
- **Consequences:** Behavior parity gap vs original UX; must be documented and isolated behind a single "wheel active" check.

## D-003: Server-authoritative adaptation state
- **Status:** Accepted (draft)
- **Context:** Adaptation grants/progression/pricing/effects are security- and consistency-sensitive.
- **Decision:** All state mutations and progression decisions occur server-side; client receives mirrored state for UI/feedback.
- **Consequences:** Requires reliable sync/messages; simplifies correctness and cheat resistance.

## D-004: MVP-first vertical slice
- **Status:** Accepted (draft)
- **Context:** Full parity is large (mechanics, UI, rendering, entities, integrations).
- **Decision:** Deliver MVP loop first (activation, subset adaptations, persistence, basic sync/feedback), then expand by priority.
- **Consequences:** Early validation of Horizon capabilities; some features deferred with explicit parity gaps.

## D-005: Evidence-backed Horizon API issues
- **Status:** Accepted (draft)
- **Context:** Some required capabilities may be missing or impractical with current hooks.
- **Decision:** Validate gaps against Horizon docs/source/hooks first; file focused, well-founded issues with use-case, existing-hook analysis, proposal, and acceptance criteria.
- **Consequences:** Slower than ad-hoc hacks but improves upstream alignment and long-term maintainability.
