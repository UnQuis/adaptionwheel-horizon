# Content / Data Parity Assessment (Horizon port)

Snapshot: 2026-11-14  
Source reference: `../adaptionwheel/src/main/resources`  
Port target: `content/adaptation` (Horizon `.hmod`, API 0.2)  
Scope note: assessment focuses on content/data declarations and datapack-style files. Runtime behavior depends on Horizon content/runtime support and native mod logic; this is not in-game verification.

## Summary

| Area | Source (adaptionwheel) | Ported / present | Status | Notes |
|---|---:|---:|---|---|
| Assets (`assets/adaptionwheel/**`) | 64 files | 64/64 (+ Horizon-specific additions like `assets/.../items/*.json`) | **ported (files)** | Prior audit confirmed full coverage of source asset path set; runtime support for `.obj`, vanilla core/post shaders in `.hmod`, fonts, custom UI/entity rendering is target-dependent/unverified. |
| Horizon block declarations | n/a (Forge code regs) | 3 (`adaptation_brazier`, `resonance_altar`, `wheel_totem`) | **ported (core)** | Under `data/adaptionwheel/horizon/block/*.json`. PR #7 addresses schema/build correctness (removing duplicate block-item item defs, fixing item schema). |
| Horizon item declarations | n/a | 5 standalone in HEAD state (`adaptation_wheel`, `all_adaption`, `mahoraga_wheel`, `mahoraga_wheel_wood`, `sword_of_extermination`) | **ported (core)** | Block items handled via block declarations/schema per PR #7 approach; final schema alignment tracked in PR #7. |
| Client item model refs (`assets/.../items/*.json`) | n/a | 8 | **ported** | Covers all 8 core items/blocks. |
| Creative tab | n/a | 1 (`main.json`) | **ported** | Single tab with 8 core entries; deduplicated. |
| Lang | en/ru present | present (content varies by key set) | **partial** | Core names/tab keys present; full gameplay/UI string parity with source ru/en not audited exhaustively in this pass. |
| Sounds | `sounds.json` + 6 ogg | ported as assets | **ported (files)** | Runtime playback/wiring unverified. |
| Block loot tables | 3 | 3 (`loot_table/blocks/*`) | **ported** | Vanilla datapack format (`dfd10f9`). |
| Recipes | 5 | 5 (`recipe/*`) | **ported** | Vanilla datapack format (`dfd10f9`). |
| Mineable tags | mod pickaxe tag present | ported (mod + minecraft addition) | **ported** | `934e36c`; important for block mining behavior. |
| Other mod block tags (`fist_*`) | 6 | 0 | **not ported** | Gameplay-mechanic tags for unported/partial systems; low value without corresponding logic; can be added later if needed. |
| Biome tags | 2 (`dark`, `has_structure/adaptation_temple`) | 0 | **not ported** | Relevant only with worldgen/structure parity; vanilla-format but depends on unported generation features. |
| Advancements | 28 | 0 | **not ported (high risk)** | Likely depend on custom criteria/triggers, stats, entities, and gameplay systems not present in Horizon port; blind port would be non-functional/misleading. |
| Worldgen/structures | structure NBTs, template pool/set/processor, structure/biome tags | 0 | **not ported** | Requires Horizon worldgen/data compatibility validation and broader feature parity; not included in current MVP/content slice. |
| Curios data (`curios/slots`, `entities`) | 2 | 0 | **not portable as-is** | Curios-specific; accessory-slot behavior is a Horizon API/UX gap (see issue drafts). |
| NeoForge biome modifier | 1 | 0 | **not portable as-is** | NeoForge-specific format/system. |
| Domain altar data (`domain_altar/*.json`) | 63+ | 0 | **not ported** | Custom mod data format/mechanics; requires ported gameplay logic and/or Horizon-compatible data handling. |
| Entity loot tables | 1 (`entities/disciple.json`) | 0 | **not ported** | Depends on entity/content parity (disciple/entity systems not ported in this scope). |

## Evidence / actions taken in this integration pass

- Verified core Horizon declarations/assets refs: 3 block + 5 standalone item declarations (current HEAD state), 8 `assets/adaptionwheel/items/*.json`, single creative tab, sounds/lang/core refs.
- Added portable datapack data on PR #5 branch: block loot tables (3) and core recipes (5) — commit `dfd10f9` (pushed). Added mineable/pickaxe tags (mod + minecraft) — commit `934e36c` (pushed).
- Documented schema/build fix context: PR #7 (`fix/content-schema-build-pr5`) provides scoped Horizon schema/build corrections with reported `cargo-horizon-mod build` + `check --json ok:true` evidence (reproduced by Agent #2/coordinator; not reproduced by Agent #4 in this environment).

## Recommendations

1. **Prioritize runtime-validated correctness** of ported core content (blocks/items/tab/models/textures/sounds/loot/recipes/tags) in target Horizon before expanding data parity.
2. **Do not blindly port** advancements, worldgen, curios, NeoForge biome modifiers, or custom domain_altar data without corresponding Horizon-compatible systems/logic and evidence.
3. **Port incrementally** only data that is (a) vanilla-format compatible with Horizon `.hmod` data handling and (b) backed by implemented mechanics/content references.
4. **Track gaps** (Curios/accessory slot, keybind/wheel UX, persistence/sync ergonomics, rendering/UI/runtime format support) as evidence-backed Horizon issues once prototype/runtime blockers are confirmed.
