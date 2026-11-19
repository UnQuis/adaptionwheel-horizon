# AdaptionWheel native core — Sunrise API port

Feasibility port of the migrated Horizon `mod_api` core to the Sunrise mod API. The architecture is
the same; only two mechanical changes were required:

- `#[mod_api::horizon_mod]` → `#[mod_api::sunrise_mod]`
- `LevelRandomHost` → `HitHost` for `living_incoming_damage`

Verified with the signed Sunrise SDK `sdk-ed3ac19558d8dc1092c431312848cbb58fb01e41`:
`cargo check` clean, `cargo test` 81/81.

To build locally, fetch the SDK source into `.sunrise/sdk/api/`:

```sh
mkdir -p .sunrise/sdk/api
curl -sL https://github.com/sunrise-org/sunrise/releases/download/sdk-ed3ac19558d8dc1092c431312848cbb58fb01e41/api-source.tar.xz | tar -xJ -C .sunrise/sdk/api
cargo test
```

This tree is intentionally separate from `native/adaptation_v2` (Horizon); no behavior was changed.
