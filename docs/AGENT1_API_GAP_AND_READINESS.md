# Agent #1: API gaps, coordination and PR readiness snapshot

_Zone: specs/prioritization, Horizon API gap tracking & issue proposals, integration coordination, PR merge-readiness criteria._
_Last updated: 2026-11-14 (session agent #1, goal `edc26e81-3c9c-48a9-a0ff-922bdda9c8bf`)._

## 1) Baseline (repo / PR / agents)

- **Port repo:** https://github.com/UnQuis/adaptionwheel-horizon (`origin` configured locally), current branch during audit: `feat/creative-tab-adaptation-logic`.
- **Open PRs (stack/evidence snapshot; last checked 2026-11-14 via `gh pr view`):**  
  - **#4** `feat: wheel activation from any inventory slot (Horizon 26.3 inventory hook)` → `main` (head `cf987c0`, MERGEABLE) — pins native target to Horizon `f4d8d62c31`, uses `player_inventory_changed` (API 579, 41-slot snapshot) for server-side wheel activation anywhere in inventory; PR reports `cargo test` 6 passed.  
  - **#5** `feat: expand adaptation scenarios, commands, and creative tab data` → `#4` branch (head `70f844d`, MERGEABLE) — expands native scenarios/commands/state handling and aligns creative tab/content/lang data; coordinator reproduced `cargo test` 10/10 on an earlier head (evidence must be tied to exact tested SHA/current head).  
  - **#7** `fix(content): Horizon schema/build fixes for PR #5 stack` → `#5` branch (head `3f044c5`, rebased onto PR #5 head `70f844d`, MERGEABLE) — scoped content/schema/build fixes. Reproduced evidence (coordinator, 2026-11-14): `cargo-horizon-mod build/check` → `ok:true` (sha256 `5c86ca5fe759f4706aacae4a22bab9b2e628e4b492aca84b19cccf8e94a2dc11`, size `975453`), https://github.com/UnQuis/adaptionwheel-horizon/pull/7#issuecomment-6874867629. Runtime/instance/CI/integration evidence and validation on final merge heads still required.
  - **#6** (this docs PR, head `00801c4` at time of prior sync; maintained in this branch) — coordination/readiness snapshot and Horizon API gap draft updates (docs-only).
- **Cross-agent notes (local, untracked in port worktree):** `docs/HORIZON_API_GAPS_NATIVE_CORE.md` (Agent #3) reports attachments/mod_net primitives present in Horizon target but persistence wiring/mod message API surface and runtime validation remain prototype-dependent — consistent with conditional gaps tracked here.
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

## 4) PR readiness criteria and evidence gates (Agent #1 lens)

Minimum evidence expected before recommending merge (policy: 100% confidence, reproducible evidence per PR, stack-aware):

1. **Scope & behavior:** PR objective is explicit; implemented behavior matches agreed MVP/MVP+ slice and does not silently regress existing behavior.
2. **Build/test evidence:** reproducible commands, environment/target revision, and results recorded in PR (e.g. `cargo test` for native crate, `cargo-horizon-mod build/check` for content) on the declared Horizon target.
3. **Target/API alignment:** pinned Horizon commit/API versions documented; hook/helper/data schema usage matches target contracts.
4. **Runtime/prototype validation (where required):** scenarios affected by hooks, player state, inventory, damage, sync, UI, commands, or persistence have launcher/instance-level verification or clearly documented, bounded limitations accepted for the slice.
5. **State/safety semantics:** activation, persistence/clone/respawn/login, server-authoritative decisions, and failure modes reviewed for the slice.
6. **Docs/coordination:** relevant docs (`HORIZON_NOTES`, MVP docs, issue drafts/register) updated; known gaps/risks and follow-ups explicitly listed.
7. **No hidden blockers:** any confirmed Horizon API blocker has a prepared/linked issue proposal with evidence; unresolved blockers block merge recommendation.
8. **Stack integrity:** for stacked PRs, verify base/head SHAs, rebase/merge state, and that evidence applies to the exact reviewed commit(s) in merge order.

### Current open PR stack (2026-11-14 snapshot)
- **#4** `feat/inventory-activation-26.3` → `main` (MERGEABLE, no CI): inventory activation via `player_inventory_changed` (API 579), target `f4d8d62c31`.
- **#5** `feat/creative-tab-adaptation-logic` → `feat/inventory-activation-26.3` (MERGEABLE, no CI): expanded scenarios/commands + creative tab/data/lang, stacked on #4.
- **#7** `fix/content-schema-build-pr5` → `feat/creative-tab-adaptation-logic` (MERGEABLE, no CI): scoped content schema/build fixes, stacked on #5.
- **#6** `docs/agent1-api-gaps-readiness` → `main` (MERGEABLE, no CI): docs-only readiness/gap register and issue draft updates.

### Evidence/status by PR (not a merge decision)

| PR | Reproduced/recorded evidence | Missing for 100% confidence (known) | Merge gate notes |
|---|---|---|---|
| #4 | PR reports `cargo test` 6 passed on `f4d8d62c31` (inventory activation slice). | Independent reproduction details if needed (env/cmd/head SHA), runtime/instance validation of activation anywhere in inventory (incl. login/clone/inventory mutation paths) on target Horizon, CI if available. | Base for #5/#7 stack; must be validated in merge order and not invalidated by stack fixes. |
| #5 | Coordinator reproduced locally (2026-11-14, on earlier head of `feat/creative-tab-adaptation-logic`): `cd native/adaptation/api && cargo test` → **10 passed, 0 failed**. Current head: `70f844d`; native/content checks and runtime evidence must be tied to this (or later) exact SHA. | Reproducible content build/check evidence tied to current head, runtime validation for expanded scenarios/commands and damage classification/player correlation, inventory activation end-to-end behavior, persistence/state semantics for intended slice, sync/UI as applicable, CI. | Stacked on #4; PR #7 rebased on current head `70f844d` (build/check evidence recorded) but full stack integration/runtime validation still required. |
| #7 | Rebased onto PR #5 head `70f844d`, current head `3f044c5` (`adb7e87`, `3f044c5`). Coordinator reproduced (2026-11-14): `cargo-horizon-mod build/check` → `ok:true` (sha256 `5c86ca5fe759f4706aacae4a22bab9b2e628e4b492aca84b19cccf8e94a2dc11`, size `975453`; https://github.com/UnQuis/adaptionwheel-horizon/pull/7#issuecomment-6874867629). | Runtime/launcher/in-game validation of packaged content, integration with native behavior in stack (#4/#5/#7), CI/content-build automation, and confirmation on final merged heads. | Stacked on current #5 head; prior drift resolved by rebase, but merge-readiness still requires runtime/integration/CI evidence. |
| #6 | Docs-only diff; contains readiness/gap register and updated conditional issue drafts with filing/process guardrails. | Review for accuracy against latest evidence/PR states (ongoing maintenance). No runtime/build claim. | Can merge independently if reviewed accurate; does not substitute feature/runtime evidence for #4/#5/#7. |

### Cross-cutting readiness gaps / risks
- **No CI status checks** on open PRs; all evidence currently manual/local and must be reproducible (command, cwd, tool version/path, target Horizon rev, commit SHA).
- **Runtime/instance validation missing** across stack: hook dispatch, player/inventory/damage mapping in real gameplay context, commands/content behavior in Horizon launcher instance, persistence/clone/login/respawn semantics where applicable.
- **Horizon API uncertainty:** damage payload/context & player correlation, mod_net/UI feedback contract sufficiency, and persistence ergonomics remain hypothesis/conditional until prototype evidence confirms or rules out blockers (drafts prepared in `docs/issues/*`).
- **Stack ordering:** intended stack #4 → #5 → #7 requires careful merge/rebase sequencing. PR #7 is now rebased on PR #5 head `70f844d` (head `3f044c5`) with reproduced content build/check evidence; prior drift resolved, but all stack evidence must be revalidated on final merge heads and supplemented with runtime/integration/CI.

## 5) Runtime/integration validation checklist (required for 100% merge confidence)

_No reproducible runtime/instance evidence recorded yet for stack #4→#5→#7 (as of last check). Each item must be tied to exact PR heads/SHAs, Horizon target rev/build, commands, environment, and captured artifacts (logs/screenshots/video/save data as applicable)._

### A. Environment & setup (reproducibility)
- [ ] Horizon build/revision and launcher/client binary used (path/version/commit), OS/Java/toolchain, prepared game resources/instance.
- [ ] Mod artifacts installed: `adaptionwheel.hmod` (sha256/size/source commit), native mod build/artifacts if applicable, install path/config.
- [ ] Exact launch commands and clean/used world/profile state.

### B. Content/package loading (PR #7 + stack)
- [ ] `cargo-horizon-mod build` and `check --json` on final reviewed head(s) (ok, sha256/size) — recorded for PR #7 on `3f044c5`, must remain valid on final merge head.
- [ ] In-game/runtime: mod/content loads without errors; creative tab(s), items, blocks appear with expected IDs/names/icons/models where supported.
- [ ] Known unsupported/runtime-unverified assets/formats explicitly checked and documented (no crashes/missing critical content for scoped slice).

### C. Inventory activation & core state (PR #4/#5)
- [ ] Wheel activation detected when adaptation wheel is in hotbar, main inventory, offhand, and armor slots (as applicable to scoped behavior) using server-authoritative logic (`player_inventory_changed` path).
- [ ] Activation updates on inventory changes, login, and clone/respawn flows as designed.
- [ ] Commands `/adaptionwheel status|activate|set|grant|reset|help` behave correctly in runtime context (permissions/output/state effects).

### D. Adaptation scenarios (PR #5 slice, as applicable)
- [ ] Repro steps and observed outcomes for each implemented scenario/concept (e.g. env_fall, type_explosion, env_fire, env_lava, type_general, env_drowning, type_contact): trigger condition, progress/level changes, mitigation/effects, classification correctness.
- [ ] Damage/event-to-player correlation correct in runtime (no misattribution/missed events for covered cases).
- [ ] Edge cases/bounded limitations documented with evidence.

### E. Persistence & session lifecycle (if in scope of tested slice)
- [ ] State behavior across relog, respawn/clone, dimension change (if relevant), and server/world restart/save-load.
- [ ] If persistence is in-memory by design for slice, explicitly document limitation and acceptance; if attachment-backed, capture evidence of save/load correctness.

### F. Sync/UI/feedback (if in scope)
- [ ] Client-visible feedback/state sync (HUD/messages/screens/mod_net) matches server-authoritative state for tested flows.
- [ ] No desync/regression observed in covered scenarios.

### G. Stack/integration & CI
- [ ] Evidence collected on actual stack order #4→#5→#7 (or final merged result) with exact SHAs.
- [ ] No regressions introduced by rebases/stacked fixes; native `cargo test` and content build/check revalidated on final heads as needed.
- [ ] CI/checks added or, if unavailable, manual evidence is complete and reproducible.

## 6) Immediate support priorities for Agent #4 / coordinator
- Finalize and maintain this gap/readiness register with evidence from ongoing PR work.
- Validate/update `docs/issues/*` drafts against confirmed blockers and correct upstream filing target/process.
- Provide/maintain acceptance checklists per PR/scenario (especially #4/#5/#7) and call out missing evidence blocking 100% confidence.
- Support adaptation logic/data/command integration questions with parity/spec references (`FEATURE_PARITY_MATRIX.md`, `MVP_*`, `HORIZON_NOTES.md`).
