# [API][Conditional] Эргономика персистентности сложного состояния игрока/мода (adaptionwheel)

> Статус: **ready to file if blocker confirmed** (conditional gap — подавать/эскалировать только если прототип покажет, что текущих средств сохранения недостаточно по удобству, надёжности или производительности).
> Target Horizon (baseline notes): `139e83fff8` (`docs/HORIZON_NOTES.md`); актуальные PR порта (#4/#5) таргетируют `f4d8d62c31` — evidence/blocker подтверждать на фактическом target build/commit используемого прототипа.
> Трек подачи: **требует подтверждения**. Черновик исторически ссылался на `arismoko/horizon` (mod API/hooks), однако текущий `CONTRIBUTING.md` в локальном checkout `horizon-rust/horizon` явного трека не задаёт; перед внешней подачей согласовать корректный репозиторий/процесс с мейнтейнерами Horizon/координатором.

## Контекст и ссылки
- Baseline и валидация возможностей Horizon: `docs/HORIZON_NOTES.md` (attachments `AttachmentType`/helpers, `player_persistent_data`, config APIs, риски lifecycle/migrations).
- Связанные артефакты порта: `docs/MVP_SCOPE.md`, `docs/MVP_TECH_DESIGN.md`, PR-0 scaffold.

## Use-case из adaptionwheel
Порт может требовать сохранения сложного состояния на игрока/мир, например:
- разблокированные/выбранные адаптации, прогресс, кулдауны/заряды;
- привязки колеса (слоты/раскладка), настройки поведения способностей;
- служебные данные миграции версий порта (schema/versioning).

Точный объём и форма состояния зависят от реализации PR-1/PR-2 и должны быть уточнены по фактическому прототипу.

## Что проверено в Horizon
По `native-hooks.json` и `docs/native-mods.md`, а также `docs/HORIZON_NOTES.md` (baseline `139e83fff8`):
- В каталоге есть server/player data helpers, включая `player_persistent_data` / `set_player_persistent_data`; в docs/HORIZON_NOTES отмечены attachments API (`AttachmentType`, codec, copy_on_death, sync) как сильный кандидат для server-authoritative состояния.
- Есть `native_config_dir`, `native_register_configs` и config lifecycle hooks/events (`native_config_loading`, `native_config_reloading`), подходящие для конфигов мода (не per-player world state).
- Для мировых/блоковых данных есть NBT/BlockEntity/Level механизмы (в т.ч. mod block entities с сохранением), но это не то же самое, что эргономичное per-player state API.

Перед подачей issue: подтвердить на target `139e83fff8` реальную семантику (holder support, lifecycle, flush/save timing, лимиты, sync, clone/respawn) выбранного механизма.

## Почему может быть недостаточно (условно)
Даже при наличии базового persistent data/attachments API возможны проблемы:
- неудобная работа со сложными структурами (нет типизированной схемы, версионирования, миграций, partial updates);
- риск гонок/перезаписи при частых обновлениях из разных хуков/тиков;
- неочевидная семантика сохранения (когда реально записывается, что происходит при disconnect/crash/server stop);
- отсутствие удобного namespace-слоя/вспомогательных helper'ов для сложных объектов (списки, вложенные записи, дефолты).

Если текущий API покрывает нужды прототипа (достаточны attachments/NBT/compound + дисциплина версионирования в моде), отдельный запрос на расширение не нужен.

## Предлагаемая точка расширения (если подтвердится необходимость)
1. **Эргономические helper'ы поверх существующего API** (предпочтительно):
   - namespaced get/set/merge для mod-owned поддерева данных;
   - утилиты сериализации/дефолтов/миграций версии схемы;
   - явный контракт по моменту сохранения и частоте записи.
2. **События жизненного цикла персистентного состояния:**
   - load/clone(respawn/dimension)/save hooks для player mod-data с гарантированным порядком.
3. **Документационные улучшения (минимум):**
   - чётко описать ограничения и best practices текущего `player_persistent_data`/attachments API (размер, частота, поток, формат).

## Критерии подтверждения блокера при прототипе (до подачи issue)
- [ ] На target Horizon `139e83fff8` реализован прототип persistence (attachments и/или `player_persistent_data`) для MVP-состояния PR-2.
- [ ] Проверены сценарии: login/logout, relog, respawn/death (`player_clone`/`copy_on_death`), смена измерения (если применимо), server stop/restart.
- [ ] Зафиксированы конкретные ограничения/дефекты: потеря/перезапись данных, сложность миграций, отсутствие гарантий flush, ограничения payload/сериализации, неудобный контракт.
- [ ] Есть воспроизводимые артефакты (ветка/коммит, тест-кейсы/логи, шаги воспроизведения) и ссылка на `docs/HORIZON_NOTES.md` / PR-2 evidence.

## Критерии приёмки (для будущего API/улучшения)
- [ ] Мод может безопасно хранить и обновлять сложное состояние adaptionwheel (включая эволюцию схемы между версиями порта).
- [ ] Семантика загрузки/сохранения/миграции документирована и предсказуема (login, respawn, dimension change, disconnect, stop).
- [ ] Нет необходимости в хрупких обходах (дублирование в нескольких хранилищах, частые полные перезаписи без контракта).
- [ ] Производительность приемлема для ожидаемой частоты обновлений (без лишнего тикового I/O/аллокаций).
- [ ] Есть минимальный пример/рекомендованный паттерн для модов.

## Куда и как подавать
- **Куда (уточнить перед подачей):** канонический трекер — https://github.com/horizon-rust/horizon/issues (по README/CONTRIBUTING Horizon), но для mod API/hooks/ergonomics gaps необходимо сверить актуальный канал/ожидания мейнтейнеров (repo docs/Discord). Историческая ссылка на `arismoko/horizon` может быть устаревшей; не подавать до подтверждения.
- **Как:** приложить конкретный прототипный кейс, описание текущего контракта/ограничений, подтверждённый target/rev Horizon, предлагаемое улучшение (helper/event/docs), метрики/логи и ссылки на `docs/HORIZON_NOTES.md`, PR/ветку.
- **Когда подавать:** только после выполнения раздела "Критерии подтверждения блокера при прототипе" и подтверждения корректного канала подачи.

## Источники
- Horizon native hooks catalog: https://github.com/horizon-rust/horizon/blob/main/native-hooks.json
- Horizon native mod docs: https://github.com/horizon-rust/horizon/blob/main/docs/native-mods.md
- Horizon config dir / mod data docs context: https://github.com/horizon-rust/horizon/blob/main/docs/mods.md
- Horizon mod API fork: https://github.com/arismoko/horizon
- Портируемый мод: adaptionwheel (Java/NeoForge 1.21.1), `../adaptionwheel` (уточнить состав сохраняемого состояния после прототипа)
- Локальные заметки: `docs/HORIZON_NOTES.md`
