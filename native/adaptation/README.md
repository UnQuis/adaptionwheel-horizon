# Native scaffold (`.hnmod`)

This directory contains the native mod scaffold for the Horizon port of AdaptionWheel.

## Status (PR-0)
- Minimal trait-mod skeleton only (`horizon-mixin.toml`, `api/` crate, placeholder hook).
- No adaptation gameplay logic implemented yet.
- Horizon target commit/hooks API must be pinned before real builds.

## Notes
- Native mods are trusted code built by Horizon launcher/build tooling against a specific Horizon revision.
- Build/validation steps depend on the selected Horizon version and environment; document confirmed steps in `docs/HORIZON_NOTES.md`.
