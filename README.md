# AdaptionWheel (Horizon Port)

Port of the **AdaptionWheel / Adaptation** mod to [Horizon](https://github.com/horizon-rust/horizon) (Minecraft: Java Edition reimplemented in Rust).

Original mod (NeoForge 1.21.1, Java): `../adaptionwheel` (source/assets owned by project author).

## Status

Early bootstrap / planning phase.

This repository is being developed by a coordinated multi-agent team:
- repository and GitHub workflow coordination
- source/mechanics analysis of the original mod
- Horizon modding API research and feasibility mapping
- architecture and phased migration plan

## Goals

- Port core AdaptionWheel gameplay systems to Horizon's modding model (`.hmod` content + native `.hnmod`/hook-based implementation as needed).
- Preserve behavior and assets where feasible.
- Track Horizon API gaps and file actionable, well-founded issues in the appropriate Horizon repositories when required.

## Development workflow

- Work happens through pull requests.
- Multiple PRs will be used for incremental milestones.
- PRs are merged only when we are confident they work.

## References

- Horizon: https://github.com/horizon-rust/horizon
- Horizon mods/docs: https://github.com/horizon-rust/horizon-mods
- Native mod template: https://github.com/horizon-rust/horizon-native-mod-template
