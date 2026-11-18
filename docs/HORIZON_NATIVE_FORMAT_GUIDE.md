# Horizon native mod format guide (AdaptionWheelRS)

_Docs-only. Prepared from Ensemble task S4: upstream `horizon-rust/horizon` **main** verified via webfetch 2026-11-18; old/pinned format kept for comparison. Unverified items are marked explicitly. **Status: migration to the new format is BLOCKED** — no working `native-build` path yet; see §5 and [horizon-rust/horizon#18184](https://github.com/horizon-rust/horizon/issues/18184)._

## 1. Upstream main: package and `[native]` in `horizon-mod.toml` (confirmed)

Sources: `docs/modding/src/reference/manifest.md`, `crates/cargo-horizon-mod/src/main.rs` + `template/native/horizon-mod.toml`, `docs/modding/src/how-to/native-mods.md`.

- A single `.hmod` (content / native / wasm / combined) uses **one `horizon-mod.toml` as the sole manifest and id/version source**.
- Native layout: root `Cargo.toml`, `src/lib.rs`, `horizon-mod.toml`, optional `assets/` + `data/`. Scaffold: `cargo horizon-mod new <path> --template native|wasm [--horizon <build>]`.
- **`[native]` section in `horizon-mod.toml`** (confirmed on main): `game` (e.g. `"26.3"`), `horizon` (40-hex commit), `mixins` (40-hex commit), optional `[[native.module]]` (manifest reference + template append).
- No hand-written `horizon-mixin.toml` / `horizon-native.json` / native|rust input folders in this model.
- Also available on main: `[[inject]]` (`id`, `at`, `kind`, `with`, `priority`, `hash`), native routes `[[native.mixin]]`/`[[native.loop]]`/`[[native.bridge]]`/`[[native.module]]`, top-level `internals`, `[dependencies]`/`[properties]`/`[[jar]]`, `[wasm]`.

## 2. Upstream main: macro, Registrar, SDK (confirmed)

Sources: `template/native/lib.rs`, `docs/modding/src/concepts/{parts,injections,dependencies}.md`, `cargo-horizon-mod` main, `crates/horizon-mod-{vocab,api-gen,runtime}/Cargo.toml`.

- **Entry macro:** `#[mod_api::horizon_mod]` on the root `impl Hooks` generates the factory — the new template has **no manual `hooks()`**.
- **Registrar / Parts:** `Hooks::register(&self, r: &mut dyn Registrar, _: &RegistrationContext)`; custom stats/parts registration; `#[mod_api::horizon_part]` for `Block`/`BlockEntity`/`Item`/`Menu`/`Fluid`/`Entity` parts.
- **SDK/build:** pinned to a game build; the launcher compiles the mod on the player's machine with the pinned toolchain against the prebuilt game pinned in `[native]` (trust-gated). SDK fetched to `.horizon/sdk/`, path dependency `horizon-mod-api`. Commands: `cargo horizon-mod new|update|build|run|check|hooks|sdk diff|fits|inject|internals|use-jar|wasm-std`.
- **Generation/runtime crates:** `horizon-mod-vocab` (vocabulary/host contracts/dispatch), `horizon-mod-api-gen` (build-time typed API generation), `horizon-mod-runtime` (storage/attachment indexes/registry slot tables) — consistent with the generated API model.
- **Unverified on main:** exact event/type rename details such as `LivingIncomingDamageEvent` without version suffix (reported in our `native/adaptation/README.md:12-16`, not directly confirmed in fetched snippets).

## 3. Old/pinned format (local checkout `139e83fff8`, repo pin `f4d8d62c31`)

- Bundle + `horizon-mixin.toml` (`[mod]`, optional `[compatibility].horizon_revision`, `[api].path`, `[[injection]]`/`[[crate]]`/`[[post_build]]`).
- `api/` crate with `horizon-client` git rev; `pub fn hooks() -> Box<dyn Hooks>` implementing `horizon_client::mod_api::Hooks`.
- Packaged `.hnmod`/`.hmod`: `horizon-native.json` + `mixins/<id>/` (+ optional content). **No `[native]`** in content `horizon-mod.toml`.
- Repo now: `native/adaptation/` old-style bundle pinned `f4d8d62c31` with `hooks()` in `api/src/lib.rs`; `content/adaptation/` separate content mod (`horizon-mod.toml`, API 0.2).

## 4. Old → new diff

| Aspect | Old / pinned | Upstream main |
|---|---|---|
| Manifest | `horizon-mixin.toml` + `horizon-native.json` | `[native]` in single `horizon-mod.toml` (`game`, `horizon`, `mixins`) |
| Layout | `[api]` subcrate + `mixins/<id>/` | root `Cargo.toml` + `src/lib.rs` unified package |
| Entry / SDK | `horizon-client` rev + manual `hooks()` trait | prebuilt SDK `horizon-mod-api` + `#[mod_api::horizon_mod]` + `Registrar`/Parts |
| API model | trait-hooks era | `horizon-mod-vocab`/`api-gen` generated API; attachments in `horizon-mod-runtime` |
| Authoring | mixin injections in bundle manifest | `[[inject]]`, native route tables, macros |

## 5. Build / launcher / runtime control

- Upstream main: `cargo horizon-mod new|update|build|run|check|…`; native build via launcher against pinned prebuilt game; `cargo-horizon-mod` suggests `horizon-launcher native-build . --trust-native-code` for code mods.
- **Validated for this repo on the old pin:** `horizon-launcher native-build ... --instance adaptionwheel-test --ignore-pins --trust-native-code` (`native/adaptation/README.md:10-11`). `--trust-native-code` is required; `--ignore-pins` bypasses pin constraints for the chosen instance/context — record revision/profile/build id/flags.
- `horizon-ctl`: control CLI/MCP bridge over a control socket (launch/state/commands/wait/input). Used for the PR #12 runtime evidence (`native-9f42064-67f92751b91f`, issue #10). **Set `HORIZON_HOME` to the instance game home first**, otherwise the default (unmodded) instance is targeted (`README.md:50-51`).
- **Confirmed blocker (orchestrator, 2026-11-18) — working path NOT found.** Launcher `0.2.78` (`/mnt/Barracuda/tmp/opencode/launcher-0.2.78/horizon-launcher`) recognizes the flat `horizon-mod.toml` with `[native]`, but there is **no signed SDK for fresh builds**: `sdk-<sha>/SHA256SUMS` returns **404**, and the launcher fails with **`E_SDK_SOURCE`**. Launcher `0.2.69` offline also fails. A vendored SDK lets `cargo check` succeed but does **not** enable `native-build`. Port-side PR #15 (git dependency on `horizon-mod-api`) likewise passes `cargo check` while `native-build` remains blocked. Filed upstream as [horizon-rust/horizon#18184](https://github.com/horizon-rust/horizon/issues/18184); awaiting upstream response. Until then this section is **not** a reproducible migration recipe — keep using the pinned old-format build path (`native/adaptation/`, `f4d8d62c31`) and record any future working recipe here with exact manifest snippet, dependency declaration, build command, instance/flags, SDK provenance and results.

## 6. AdaptionWheelRS migration mapping

- **Current:** `native/adaptation/` old bundle + pin `f4d8d62c31` + `hooks()`; `content/adaptation/` separate content hmod.
- **Target model:** single `horizon-mod.toml` with `[native]` (`game`/`horizon`/`mixins`), root `Cargo.toml`/`src/lib.rs` against SDK `horizon-mod-api`, `#[mod_api::horizon_mod]` + `Registrar`/Parts, updated hook/event types for the chosen build, aligned packaging (content+native in one `.hmod` if desired).
- **Verify before changing:** target Horizon commit/API era; event/type renames (suffix removal still unverified here); manifest/layout rules; capture `native-build` + `horizon-ctl` evidence (`HORIZON_HOME`, instance, flags, build id, artifacts).

## 7. Key evidence

- Upstream main: `docs/modding/src/{reference/manifest.md,how-to/native-mods.md,concepts/parts.md,concepts/injections.md,concepts/dependencies.md}`, `crates/cargo-horizon-mod/src/main.rs` + `template/native/{horizon-mod.toml,lib.rs}`, `crates/horizon-mod-{vocab,api-gen,runtime}/Cargo.toml`, `crates/horizon-mod-vocab/src/hooks/mod.rs`.
- Local: `../Horizon@139e83fff8` `docs/api/mod-files.md`, `docs/native-mods.md`, `template/native/*`, `launcher/src/native_mods.rs`; repo `native/adaptation/{README.md,horizon-mixin.toml,api/Cargo.toml,api/src/lib.rs}`, `content/adaptation/horizon-mod.toml`.
