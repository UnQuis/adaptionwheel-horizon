# MVP Implementation Checklist (PR-2 slice)

Target Horizon baseline: `139e83fff8` (local `../Horizon`, 2026-11-08). See `docs/HORIZON_NOTES.md`, `docs/MVP_SCENARIOS.md`, `docs/MVP_TECH_DESIGN.md`.

## Selected initial scenarios (slice 1)
1. **Env_FallDamage** (`type_fall` / `env_fall`)
   - Hook: `living_fall` (server-authoritative). Event provides entity/kind/distance/multiplier/cancel.
   - Progress: accumulate fall distance (or computed damage proxy) while wheel active and entity is the adapted player; cap per event to avoid instant completion.
   - Threshold (MVP): configurable constant (e.g. total 40 blocks equivalent distance) -> `completed=true`, `level=1`.
   - Effect: on completed, reduce fall multiplier/distance or cancel fall handling per API semantics (prefer multiplier->0 / minimal damage path supported by hook).
   - Invariants: server-authoritative mutation, no progress when wheel inactive, no-op for non-player entities.

2. **Type_EXPLOSION** (`type_explosion`)
   - Hook: `living_incoming_damage` / highest available compatible variant for target build (validate signature; start with generic incoming damage hook and classify by source/cause metadata if exposed).
   - Progress: accumulate explosion-tagged incoming damage/events while wheel active.
   - Threshold (MVP): e.g. 30 damage points total from explosion-classified sources.
   - Effect: reduce/negate explosion damage when completed (within hook mutability constraints).
   - Fallback: if source classification unavailable in this build, implement behind feature flag/TODO and keep scenario as "pending classification" in evidence (do not claim parity).

## State model (attachment v1, server source of truth)
- Attachment key: `adaptionwheel:player_state` (player holder), `schema_version=1`, `copy_on_death=true` (if supported), `sync=true` for Horizon clients when supported.
- Fields: `wheel_active`, `active_source` (`NONE|MAIN_HAND|OFF_HAND|HOTBAR`), `entries{concept_id -> {level, progress, completed, cooldown_until_tick, flags}}`, runtime dirty markers.
- Lifecycle: initialize/load on `player_logged_in`; preserve/migrate on `player_clone`/respawn as API allows; resync on join/respawn/changes.

## Wheel activation (temporary, no Curios)
- Active when wheel item present in offhand (default) and/or main hand (configurable). Fallback scan hotbar only if cheap API exists; otherwise offhand+mainhand.
- Recompute on login, respawn/clone, relevant inventory/hand hooks if available, and throttled `player_tick` fallback.

## Networking / feedback
- Channels (API 40): `adaptionwheel:state_sync` (S->C full/delta), optional `adaptionwheel:event`.
- Envelope includes protocol version; server authoritative, client mirror read-only.
- Minimal feedback: chat/debug messages and state dump command; HUD hook integration if signature available in target build.

## Debug/inspection
- `/adaptionwheel status` (or equivalent registration) shows active state and entries.
- `/adaptionwheel grant <concept>` / `/adaptionwheel reset` for dev verification (permission-gated where possible).

## Evidence required for MVP slice PR
- [ ] Pinned Horizon target in native manifests/Cargo metadata (`rev = "139e83fff8"` or confirmed build pin) and documented.
- [ ] Code compiles/checks in available environment (`cargo check`/build logs) or documented blocker with command output.
- [ ] Implemented handlers for at least 1 fully wired scenario (fall) with server-side state mutation + activation gating.
- [ ] Second scenario implemented or clearly stubbed with classification blocker evidence (explosion/fire).
- [ ] Attachment/state serialization + lifecycle hooks wired in code paths (login/clone/respawn as supported).
- [ ] Basic sync/debug feedback path implemented or explicitly documented if API unavailable.
- [ ] Repro steps and observed output recorded in PR/docs (`docs/HORIZON_NOTES.md` validation addendum or PR body).

## Known risks / likely blockers (issue-ready if confirmed)
- Exact `Host`/attachment registration/access API and player holder ergonomics in 139e83fff8.
- Mod messages registration/send/receive trait signatures and channel naming rules.
- Damage source classification for explosion/fire/lava in `living_incoming_damage`.
- HUD/text rendering API for minimal indicator.
- Build toolchain/integration with `horizon-launcher native-build` / cargo-horizon-mod workflow.
