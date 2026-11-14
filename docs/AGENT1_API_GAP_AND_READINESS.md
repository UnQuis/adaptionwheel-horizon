# Agent #1: API gaps, coordination and PR readiness snapshot

_Zone: specs/prioritization, Horizon API gap tracking & issue proposals, integration coordination, PR merge-readiness criteria._
_Last updated: 2026-11-14 (session agent #1, goal `edc26e81-3c9c-48a9-a0ff-922bdda9c8bf`)._

## 1) Baseline (repo / PR / agents)

- **Port repo:** https://github.com/UnQuis/adaptionwheel-horizon (`origin` configured locally), current branch during audit: `feat/creative-tab-adaptation-logic`.
- **Open PRs (stack/evidence snapshot):**  
  - **#4** `feat: wheel activation from any inventory slot (Horizon 26.3 inventory hook)` — pins native target to Horizon `f4d8d62c31`, uses `player_inventory_changed` (API 579, 41-slot snapshot) for server-side wheel activation anywhere in inventory; reports `cargo test` 6 passed.  
  - **#5** `feat: expand adaptation scenarios, commands, and creative tab data` (builds on #4) — expands native scenarios/commands/state handling and aligns creative tab/content/lang data; reports `cargo test` 10 passed; notes in-memory state and runtime/prototype validation gaps.  
  - **#7** `fix(content): Horizon schema/build fixes for PR #5 stack` (base `feat/creative-tab-adaptation-logic`, stacked on #5) — scoped content/schema/build fixes. Reproduced evidence (coordinator, 2026-11-14): base branch head `934e36c`, from `content/adaptation` with `/home/alexquasar/Проекты/MCModsFin/Horizon/target/debug/cargo-horizon-mod`: `build` produced `adaptionwheel.hmod`, `check --json` => `ok:true` (sha256 `dc3ce7c76b5da9d6b34afae48575fe917cb17de4a809b9e8964dda2c89ca4470`, size `974887`). Runtime/instance/CI evidence still required. 
  - **#6** (this docs PR) — coordination/readiness snapshot and Horizon API gap draft updates (docs-only).
- **Merged PRs:** #1 PR-0 scaffold/parity/planning, #2 PR-1 Horizon validation/MVP design/API gap drafts, #3 PR-2 initial MVP slice.
- **Key docs/artifacts:** `docs/FEATURE_PARITY_MATRIX.md`, `docs/MVP_SCOPE.md`, `docs/MVP_SCENARIOS.md`, `docs/MVP_TECH_DESIGN.md`, `docs/MVP_IMPL_CHECKLIST.md`, `docs/HORIZON_NOTES.md`, `docs/BUILDING.md`, `docs/DECISIONS.md`, `docs/ARCHITECTURE.md`, `docs/issues/*`.
- **Coordination:** coordinator `ses_e26d44459ffelqD8Ni1RyYl0qQ` (goal `54d64c97-c12d-4a2f-9802-0caf8c2ec724`), Agent #4 focus/integration-verification goal `10068506-8b9d-4675-89aa-f20777268d90`. Merge policy: several PRs, merge only with 100% confirmed working evidence.

## 2) Horizon target / API issue process (current understanding)

- **Local Horizon baseline in docs:** pinned `139e83fff8` for PR-1 validation; PR #4/#5 work targets `f4d8d62c31` / 26.3-era APIs (per PRs/notes). Evidence must be interpreted against the actual target build/commit used.
- **Repos/process (evidence + coordinator guidance, 2026-11-14):**  
  - https://github.com/horizon-rust/horizon/issues — канонический трекер issues Horizon (README/CONTRIBUTING в `../Horizon`). Для недостающего mod/API в рамках порта ориентироваться на него как основной адрес, **но** перед подачей сверить актуальный канал/ожидания мейнтейнеров (repo docs/Discord), т.к. CONTRIBUTING позиционирует issues для багов и не описывает отдельный mod-API gap track.
  - https://github.com/arismoko/horizon — fork; ссылки на него в существующих drafts могут быть устаревшими и не подтверждены текущим CONTRIBUTING.
- **Local Horizon checkout:** `../Horizon` (`origin`/`upstream` -> `horizon-rust/horizon`), `.github/ISSUE_TEMPLATE` отсутствует.
- **Правило перед внешней подачей:** подтверждённый target/rev Horizon, конкретное blocker evidence из порта (что пробовали, где/почему не хватает API, repro/контекст), и явно указанный подтверждённый канал подачи в тексте issue. Drafts в `docs/issues/*` обновлять под актуальный процесс/evidence; не подавать до подтверждения.

## 3) API gap register (initial)

| Gap / topic | Current status | Evidence (so far) | Readiness to file upstream issue | Notes / what is needed |
|---|---|---|---|---|
| Accessory/wheel slot (Curios-like equip/integration) | **Hypothesis / probable gap** | `docs/issues/api-accessory-wheel-slot.md`, `docs/HORIZON_NOTES.md` (no clear dedicated accessory-slot API found in surveyed hooks/docs); PR #4 implements inventory-anywhere activation as workaround path | Not ready (needs blocker confirmation on target build + prototype evidence) | Confirm whether inventory/menu/virtual-slot approach is sufficient; document UX/sync/persistence limitations if any. |
| Persistence ergonomics for complex player/mod state (conditional) | **Hypothesis / conditional** | `docs/issues/api-persistence-ergonomics-conditional.md`, Horizon attachments/player_persistent_data/config APIs noted in docs; PR #5 states in-memory state and persistence follow-up still needed | Not ready (depends on prototype) | Validate attachments/clone/login/respawn/save semantics and migration ergonomics on target; file only if concrete limitation evidenced. |
| Input/keybind actions for wheel & active abilities (conditional) | **Hypothesis / conditional** | `docs/issues/api-input-keybind-actions-wheel.md`, `mod_key`/input/screen hooks exist but high-level action registration/UX contract unclear from surveyed materials | Not ready (depends on prototype/UX validation) | Confirm if existing hooks suffice for bind discovery, contexts, press/hold/release and conflict handling; file if not. |
| Damage/environment hook payload/context sufficiency for adaptation classification & player correlation | **Hypothesis / to validate** | `docs/issues/api-damage-hook-context-conditional.md`, `docs/HORIZON_NOTES.md`, PR #5 notes runtime entity/player mapping and payload classification constraints in some paths; hooks include `living_incoming_damage(_v2/v3 docs)`, `living_fall`, `living_drown`, block/fluid/tick hooks | Not ready (needs prototype/runtime evidence) | Collect concrete cases where current payloads/helpers cannot reliably identify affected player or classify required damage/environment scenarios; file only with confirmed blocker evidence/repro. |
| Client sync/UI feedback path for adaptation progress/state (mod_net + UI contract) | **Hypothesis / to validate** | `docs/issues/api-modnet-sync-ui-conditional.md`, `docs/HORIZON_NOTES.md`, Horizon mod_net/UI hooks present but native mod-API surface/sufficiency for MVP feedback needs prototype validation | Not ready (needs prototype/runtime evidence) | Validate send/receive bindings, limits/semantics, and minimal HUD/screen feedback path; escalate only on confirmed gap with working/attempted prototype evidence. |
| Server-authoritative persistence lifecycle for adaptation state (attachments/player data) | **Hypothesis / conditional** | PR #5: in-memory state in current slice; drafts/Horizon notes reference attachments and `player_persistent_data` but lifecycle/ergonomics need prototype validation (`player_clone`, login/logout, respawn, save flush) | Not ready (needs prototype/runtime evidence) | Confirm whether existing APIs are sufficient for required semantics/migrations; use existing persistence-ergonomics draft if concrete blocker found. |

## 4) PR readiness criteria (Agent #1 lens)

Minimum evidence expected before recommending merge (per repo policy: 100% confidence):

1. **Scope & behavior:** PR objective is explicit; implemented behavior matches agreed MVP/MVP+ slice and does not silently regress existing behavior.
2. **Build/test evidence:** reproducible commands and results recorded in PR (e.g. `cargo test` for native crate, content validation/build steps as applicable) on the declared target Horizon revision.
3. **Target/API alignment:** pinned Horizon commit/API versions documented; hook/helper usage matches target contracts.
4. **Runtime/prototype validation (where required):** scenarios affected by hooks, player state, inventory, damage, sync, UI, or persistence have launcher/instance-level verification or clearly documented, bounded limitations accepted for the slice.
5. **State/safety semantics:** activation, persistence/clone/respawn/login, server-authoritative decisions, and failure modes reviewed for the slice.
6. **Docs/coordination:** relevant docs (`HORIZON_NOTES`, MVP docs, issue drafts/register) updated; known gaps/risks and follow-ups explicitly listed.
7. **No hidden blockers:** any confirmed Horizon API blocker has a prepared/linked issue proposal with evidence; unresolved blockers block merge recommendation.

### Quick assessment of current open PRs (evidence snapshot, not a merge decision)
- **PR #4:** PR description reports `cargo test` 6 passed on target `f4d8d62c31` for inventory-activation slice; readiness for merge still depends on required runtime/instance confirmation of activation behavior on target Horizon and acceptance of documented follow-ups (stacked base for #5 must be considered).
- **PR #5:** coordinator reproduced locally on `feat/creative-tab-adaptation-logic` (PR #5 head, `/home/alexquasar/Проекты/MCModsFin/AdaptationModRS`, 2026-11-14): `cd native/adaptation/api && cargo test` => **10 passed, 0 failed** (`activation_detects_wheel_anywhere_in_inventory`, `commands_set_and_grant_new_concepts`, `explosion_damage_classifier`, `damage_type_classification_for_new_concepts`, `damage_adaptation_applies_mitigation_and_progress`, `fall_progress_and_mitigation`, `explosion_progress_levels_up`, `fall_requires_activation`, `state_defaults_and_summary`, `leveled_concepts_progress_with_activation`). This strengthens native unit-test evidence for PR #5 head, but merge-readiness for #4/#5 still requires reproducible content/build/runtime/integration evidence, stacked-order validation (#5 over #4), and explicit closure/acceptance of remaining gaps (runtime damage/player mapping, persistence semantics as applicable, sync/UI).

## 5) Immediate support priorities for Agent #4 / coordinator
- Finalize and maintain this gap/readiness register with evidence from ongoing PR work.
- Validate/update `docs/issues/*` drafts against confirmed blockers and correct upstream filing target/process.
- Provide acceptance checklists per PR/scenario (especially PR #4/#5) and call out missing evidence blocking 100% confidence.
- Support adaptation logic/data/command integration questions with parity/spec references (`FEATURE_PARITY_MATRIX.md`, `MVP_*`, `HORIZON_NOTES.md`).
