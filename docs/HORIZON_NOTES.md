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
- **Hooks present (confirmed in `native-hooks.json`):** `living_incoming_damage` (server, cancellable), `living_incoming_damage_v2` (server, cancellable; docs/API 384: additive host-aware hook for player melee/sweep/spear hits on mobs incl. LAN guests, fires before v1), `living_incoming_damage_v3` (documented in `docs/native-mods.md`; additive hook with richer actor/source info, fires after v1), `living_fall` (both sides), `living_drown` (server, NeoForge `LivingDrownEvent`, cancellable/modifiable), `living_breathe` (API 144), `block_fall_on` (server), `block_step_on` / `block_entity_inside` (API 73, server), fire/fluid-adjacent hooks like `block_ignite`, `block_burn`, `fluid_level_change`, etc.
- **Implication for MVP:** `living_incoming_damage`/`_v3` are primary candidates for generic incoming damage/type-based adaptation scenarios (e.g. explosion and possibly fire/lava/contact damage depending on damage type/payload coverage). `living_fall` and `living_drown` provide direct fall/drowning signals. For movement-surface scenarios (powder snow, soul sand, slime/ice/web-like effects), no dedicated movement-on-block hook was identified by name in `native-hooks.json`; MVP may need `player_tick` (server) plus position/block-state queries or other derived checks (to be prototyped).
- **Risks/TBD:** confirm exact payloads for `living_incoming_damage(_v3)` (damage type/source/cause metadata) and whether fire/lava/hot-floor/cactus-like environmental cases are best handled via damage hooks vs periodic checks. Validate `player_tick` performance/ergonomics for analysis sampling and context detection.
- **Evidence:** `native-hooks.json` entries (e.g. `living_incoming_damage` ~1394, `living_drown` ~4676, `living_fall` ~4687, `living_incoming_damage_v2` ~4859, `player_tick` present, block/fire/fluid hooks); `docs/native-mods.md` sections around API 73, 144, 356, 384, `living_incoming_damage_v3` notes.

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
| 2026-11-14 | Damage/env hook coverage for MVP scenarios | `native-hooks.json` + docs inspection | partial | `living_incoming_damage`/`_v2`/`_v3` (docs), `living_fall`, `living_drown`, `living_breathe`, block/fire/fluid hooks confirmed; fire/lava/contact coverage via damage payloads and movement-surface detection (powder snow/soul sand) require prototype/derived checks (`player_tick` + position/block context) validation. |
| TBD | Prototype: attachment save/load + clone/respawn behavior | native mod spike/build | pending | Blocker-sensitive for MVP state persistence. |
| TBD | Prototype: mod message roundtrip (client<->server) | native mod spike/build | pending | Needed for sync/intents protocol. |
| TBD | Prototype: minimal HUD/input feedback path | native mod spike/build | pending | Needed for MVP user-visible progress/completion. |

## Issue candidates (draft, validate before filing)
- **Accessory slot / wheel activation UX integration** (Curios replacement) if no native equivalent exists.
- **Per-player mod state persistence ergonomics** only if Attachment lifecycle/serialization proves insufficient for adaptation-state needs (must include concrete failure/limitation evidence).
- **Input/keybind registration UX** for ability/wheel actions (if raw hooks insufficient for expected UX).
- **Advanced UI/render APIs** for future parity (radial wheel, complex VFX) — defer until MVP blockers are proven.

## Agent #2 deep-dive notes (PR-1 validation addendum)
- **Catalog vs docs:** `docs/native-mods.md` mentions APIs/hooks not present as identically named entries in `native-hooks.json` on this baseline (e.g. text references around `living_incoming_damage_v3`/other damage hooks). Treat `native-hooks.json` as source-of-truth for available hook names/targets/side/cancellable metadata and verify semantics in docs/source before implementation.
- **Fire/lava/contact coverage:** no dedicated entity-level fire/lava damage hooks identified; likely covered (to be confirmed) via `living_incoming_damage` damage type/source classification. Block/world hooks like `block_ignite`, `block_burn`, `fire_odds`, `fluid_level_change` are environment/block-level, not direct damage-receive hooks.
- **Drowning/fall:** `living_drown` and `living_fall` confirmed as direct hooks; `block_fall_on` exists for landing/block interaction context.
- **Movement/surface detection:** no direct powder snow/soul sand movement hooks found. `block_step_on`/`block_entity_inside` are documented as mod-block contacts (API 73) and may not fire for vanilla blocks; `movement_input` is client-side/local input oriented. MVP workaround: server-side polling/derived checks in `player_tick` using position/block queries, with server-authoritative state in attachments and sync to client.
- **Ticking primitives:** `player_tick` (server, per-player), `server_tick`, `level_tick_pre/post`, generic `tick`, and `entity_tick_*` available for periodic logic/throttling.
- **Experimental priorities (PR-2):** confirm payload/signature/context for `living_incoming_damage(_v2/_v3 where available)`, `living_drown`, `living_fall`, `player_tick`; verify fire/lava/cactus/magma/powder-snow-freeze damage naming/paths; verify vanilla block behavior for contact hooks; validate `player_clone`/respawn/login behavior for attachment persistence.

## 2026-11-14 — Inventory activation update (MC 26.3-era Horizon APIs)
- **Requested:** "в оригинале на ветке 26.3 есть хук, который позволяет колесу работать тогда, когда оно находится в инвентаре. Просмотри это и примени"
- **Investigation (Horizon `origin/main`, `f4d8d62c31`):**
  - `player_inventory_changed` hook (API 579, server): `native_player_inventory_changed(player, stacks)` fires when inventory changes (and on first check after join). `stacks` contains **41 slots** in Inventory numbering: `0..36` main inventory (`0..9` hotbar), `36..40` armor (feet, legs, chest, head), `40` offhand. Hook contract: listen.
  - `inventory_changed` hook (API 600, server): notification `(uuid, kind)` once per inventory kind whose slots differ (`Player`, `Ender`, `Cursor`, `Crafting`) — can be paired with inventory read helpers.
  - Client helper `own_inventory()` (API 635): returns player's own 36 slots on client (not primary for server-authoritative activation).
- **Applied in repo (`feat/inventory-activation-26.3`):**
  - Bumped native target pins to Horizon `f4d8d62c31` (`horizon-mixin.toml`, `native/adaptation/api/Cargo.toml`).
  - Implemented server-side activation scan over full 41-slot inventory snapshot (`player_inventory_changed`) so the wheel is active when present **anywhere in inventory** (hotbar/main/offhand/armor), not only held/offhand.
  - Added per-player in-memory activation/state update on inventory changes and login messages reflecting source.
  - Kept MVP fall/explosion core logic; damage-to-player state wiring remains prototype-dependent pending player identity/context mapping in damage hooks for the target build.
- **Evidence:** code updated in `native/adaptation/api/src/lib.rs`; build/test against `f4d8d62c31` attempted/recorded in branch workflow (see commit/PR notes).

## Content/assets integration (Agent #2, 2026-11-14)

### Verified (content packaging / schema)
- `cargo horizon-mod build` + `cargo horizon-mod check --json` pass for `content/adaptation` after schema fixes (`adaptionwheel.hmod`, api 0.2, capabilities `[registries]`, side both). Reproducible with Horizon tools from `../Horizon`:
  - `cd content/adaptation && cargo run -p cargo-horizon-mod --manifest-path /home/alexquasar/Проекты/MCModsFin/Horizon/Cargo.toml -- build`
  - `... -- check target/horizon-mod/adaptionwheel.hmod --json`
- Horizon content path validation rejects unsupported/non-plain paths in `assets/` or `data/` (e.g. `.blend` source files): `assets/adaptionwheel/models/entity/DharmaChakra*.blend` caused build failure ("not a plain assets/ or data/ path"). **Workaround:** keep source assets outside packaged `assets/` (e.g. `docs/reference/...`) and ship only runtime-ready formats.
- `data/adaptionwheel/horizon/item/*.json` must follow Horizon item schema (no `item` field); block items should be declared via `data/.../horizon/block/*.json` (block `item` behavior) rather than duplicate item registrations with invalid fields.
- Current content registrations/assets aligned for declared set: 5 standalone items + 3 blocks + 1 creative tab (`main.json`), matching `assets/.../items/*.json`, `models/item`, `blockstates`, block models, textures, sounds refs, and lang keys (en/ru base names synchronized).

### Packaged but runtime-unverified formats/assets
- **Entity models (`.obj`)**: `assets/adaptionwheel/models/entity/{dharma_chakra,soe}.obj` are packaged and pass check, but runtime loading/rendering support in Horizon content pipeline is **not verified** in a Horizon instance.
- **Shaders**: `assets/adaptionwheel/shaders/core/*.json|.fsh|.vsh` and `shaders/core/post/*` are packaged; Horizon has a separate shader mod mechanism (`.hshader` / shader APIs in native context per docs), so NeoForge-style core/post shader assets inside `.hmod` content may not be applied at runtime — **needs runtime verification**.
- **Fonts**: TTF fonts and font JSON definitions under `assets/adaptionwheel/font/` are packaged; runtime font registration/rendering behavior in Horizon content mods is **unverified**.
- **Client item definitions**: `assets/adaptionwheel/items/*.json` (model refs) are packaged; treated as Horizon/vanilla-compatible client item definitions, but **runtime behavior must be verified** for target Horizon build.
- **Custom UI/radial wheel, advanced VFX/entity rendering parity**: content-only assets are insufficient; requires native hooks/UI/render/input APIs and protocol design (tracked separately in MVP scope/Horizon notes/issue drafts).

### Content-side gaps / follow-ups
- Perform runtime verification in target Horizon launcher/instance for tab visibility, item/block rendering, models (including OBJ), sounds, fonts, and shader-related visuals.
- If OBJ/core shaders/fonts are unsupported in content mods for target Horizon, prepare scoped API/format support requests or porting alternatives (with evidence from runtime tests).
- Keep scoped PRs for content schema/build fixes separate from unrelated docs/issue edits; include only files required for Horizon content correctness.
