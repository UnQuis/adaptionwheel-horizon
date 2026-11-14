# Horizon Notes

## Target (pinned for PR-1 validation baseline)
- **Horizon repo (local):** `../Horizon`
- **Pinned revision:** `139e83fff8` (`launcher@0.2.52-76-g139e83fff8`, commit date 2026-11-08, "Opaque Ice: own flag; IceBlock keeps frosted ice for fluids")
- **Evidence:** `cd ../Horizon && git rev-parse --short HEAD && git describe --tags --always && git log -1`
- **Hooks catalog:** `../Horizon/native-hooks.json` (contains `helpers` and `hooks` entries with names/targets/side/cancellable/version metadata)

> Note: this is the currently checked-out local Horizon baseline used for source/docs validation. Native build pinning must be finalized against the exact Horizon build/commit used by the target launcher/instance.

## MVP-critical capability validation (source/docs)

### Player lifecycle / server hooks
- **Hooks present (native-hooks.json):** `player_logged_in`, `player_logged_out`, `player_respawned`, `player_clone` (server, NeoForge `PlayerEvent.Clone` semantics noted), `player_tick` (server), `server_tick` / `level_tick_pre` / `level_tick_post`, `entity_tick_pre/post`.
- **Implication for MVP:** can initialize/load state on login, track per-tick analysis, and handle respawn/clone flows (important for preserving adaptation state semantics).
- **Evidence:** `native-hooks.json` hook entries; `docs/native-mods.md` sections/APIs referencing `native_player_clone` (API 322), player login/logout hooks.

### Persistence / custom state
- **Helper:** `attachments` (`horizon_client::game::native_hooks::Game::native_attachments`) exposed in helpers catalog.
- **Attachment API (docs):** `AttachmentType<T>` supports `get/set/remove/has/get_existing`, codec-based encode/decode, `copy_on_death`, and `sync` (sends to Horizon clients when applicable). Server attachments stored in holder/save context (level attachments in SavedData; player/entity/chunk/block entity holders described in docs). `sync` requires codec and syncs on set/remove/default/get-created and on tracking changes.
- **Implication for MVP:** strong candidate for server-authoritative player adaptation state persistence + Horizon client mirror sync for basic UI feedback.
- **Risks/TBD:** validate exact holder support for player state in our target build, lifecycle across death/respawn/logout/save, payload limits, and ergonomics for complex nested state/migrations.
- **Evidence:** `docs/native-mods.md` "Server attachments" (~4950+) and API 317 notes; `native-hooks.json` helper `attachments`.

### Damage / environment / movement-related hooks
- **Hooks present:** `living_incoming_damage` (server, cancellable), `living_incoming_damage_v2` (server, cancellable; docs/API 384: host-aware, limited to player melee/sweep/spear hits on mobs incl. LAN guests, before v1), `living_fall` (both sides), `block_fall_on` (server).
- **Implication for MVP:** `living_incoming_damage` is the primary candidate for generic incoming damage/type-based adaptation scenarios (e.g. explosion/fire-tagged sources depending on event payload). `living_fall` helps fall-related progression/effects. Fire/environmental exposure may require additional hooks/events or tick-based checks depending on Horizon event coverage.
- **Risks/TBD:** confirm exact payloads (damage source/type/attacker/cause metadata) and coverage for fire/lava/drowning/environmental damage in `living_incoming_damage` on target build; may need alternative hooks or polling.
- **Evidence:** `native-hooks.json` hook entries; `docs/native-mods.md` damage sections (API 384 and v1/v2 notes).

### Networking / sync / intents
- **Mod messages (API 40):** channel `horizon:mod_net`, channel format `mod_id:name`, max 30 KB, 32 messages/tick/player each way. Server->client: `native_net_send_to_player`; client->server: `native_net_send_to_server`. Receive hooks: `native_net_from_client`, `native_net_from_server`.
- **Implication for MVP:** suitable for state sync, progress updates, and basic client intents/requests (server remains authoritative). Need protocol versioning and validation of client-sent data.
- **Evidence:** `docs/native-mods.md` "Mod messages" (~4296+); helpers/hooks in `native-hooks.json` (`net_send_to_player`, `net_send_to_server`, `net_from_client`, `net_from_server`).

### Commands / config / debug
- **Commands:** `register_commands`, `perform_command`, `run_command` helper; player command hooks also present.
- **Config:** `native_config_dir` helper and config APIs/hooks described in docs (e.g. API 397 mod config files load/reload).
- **Implication for MVP:** feasible to add debug/inspection commands and basic config toggles/parameters (subject to final API usage in target build).

### UI / input (initial)
- **Hooks/helpers present (catalog/docs):** HUD/screen hooks, mod screen helpers (`mod_screen*`), key/input/mouse hooks, `mod_key` / `set_mod_key` helpers.
- **Implication for MVP:** basic HUD/chat/toast-like feedback and simple screens/overlays appear feasible; full wheel UI parity is P1/P2.
- **TBD:** validate exact rendering/text APIs and best minimal feedback path for MVP in this Horizon revision.

## Known likely gaps / high-risk areas (still relevant)
- **Curios-equivalent accessory slot:** no direct equivalent identified; MVP should use temporary activation model (see `docs/DECISIONS.md` D-002).
- **Rich UI/render/VFX parity:** custom mesh/OBJ/shaders/cinematics likely expensive or partially unsupported.
- **Movement/physics parity:** behavior-level port may require specific hooks or redesign (powder snow/soul sand/web/slime/ice etc.).
- **Complete environmental damage coverage:** need to confirm whether all MVP scenarios (fire/lava/drowning/contact/fall) are cleanly observable via available hooks or require mixed strategies.

## Validation log
| Date | Item | Method | Result | Notes |
|------|------|--------|--------|-------|
| 2026-11-14 | Target Horizon revision selected | `git` in `../Horizon` | done | `139e83fff8` pinned as local baseline for PR-1 validation. |
| 2026-11-14 | Lifecycle/persistence/messaging hooks presence | `native-hooks.json` + `docs/native-mods.md` inspection | done (initial) | Attachments, mod messages, player lifecycle, damage hooks identified; payload/semantics details still need prototype validation. |
| 2026-11-14 | Damage/env hook coverage for MVP scenarios | `native-hooks.json` + docs inspection | partial | `living_incoming_damage`/`living_fall` confirmed; fire/lava/drowning/contact coverage needs deeper validation. |
| TBD | Prototype: attachment save/load + clone/respawn behavior | native mod spike/build | pending | Blocker-sensitive for MVP state persistence. |
| TBD | Prototype: mod message roundtrip (client<->server) | native mod spike/build | pending | Needed for sync/intents protocol. |
| TBD | Prototype: minimal HUD/input feedback path | native mod spike/build | pending | Needed for MVP user-visible progress/completion. |

## Issue candidates (draft, validate before filing)
- **Accessory slot / wheel activation UX integration** (Curios replacement) if no native equivalent exists.
- **Per-player mod state persistence ergonomics** only if Attachment lifecycle/serialization proves insufficient for adaptation-state needs (must include concrete failure/limitation evidence).
- **Input/keybind registration UX** for ability/wheel actions (if raw hooks insufficient for expected UX).
- **Advanced UI/render APIs** for future parity (radial wheel, complex VFX) — defer until MVP blockers are proven.
