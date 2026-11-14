# [API][Conditional] Эргономика персистентности сложного состояния игрока/мода (adaptionwheel)

> Статус черновика: **conditional gap** — создавать/эскалировать только если прототип покажет, что текущих средств сохранения недостаточно по удобству, надёжности или производительности.
> Трек: mod API / native hooks (см. `CONTRIBUTING.md` horizon-rust/horizon: mod-related API/hooks — в fork `arismoko/horizon`).

## Use-case из adaptionwheel
Порт может требовать сохранения сложного состояния на игрока/мир, например:
- разблокированные/выбранные адаптации, прогресс, кулдауны/заряды;
- привязки колеса (слоты/раскладка), настройки поведения способностей;
- служебные данные миграции версий порта (schema/versioning).

Точный объём и форма состояния зависят от реализации PR-1/PR-2 и должны быть уточнены по фактическому прототипу.

## Что проверено в Horizon
По `native-hooks.json` и `docs/native-mods.md`:
- В каталоге есть server/player data helpers, включая `player_persistent_data` / `set_player_persistent_data` (и связанные player compound/persistent helpers), что указывает на встроенный механизм персистентного хранения данных игрока.
- Есть `native_config_dir`, `native_register_configs` и config lifecycle hooks/events (`native_config_loading`, `native_config_reloading` по docs), подходящие для конфигов мода (не per-player world state).
- Для мировых/блоковых данных есть NBT/BlockEntity/Level механизмы (в т.ч. mod block entities с сохранением), но это не то же самое, что эргономичное per-player state API.

(На целевом Horizon commit нужно подтвердить формат/лимиты/сериализацию/потокобезопасность/момент flush для `player_persistent_data` и доступность аналогичных API для других сущностей/глобального состояния.)

## Почему может быть недостаточно (условно)
Даже при наличии базового persistent data API возможны проблемы:
- неудобная работа со сложными структурами (нет типизированной схемы, версионирования, миграций, partial updates);
- риск гонок/перезаписи при частых обновлениях из разных хуков/тиков;
- неочевидная семантика сохранения (когда реально записывается, что происходит при disconnect/crash/server stop);
- отсутствие удобного namespace-слоя/вспомогательных helper'ов для сложных объектов (списки, вложенные записи, дефолты).

Если текущий API покрывает нужды прототипа (достаточны NBT/compound + дисциплина версионирования в моде), отдельный запрос на расширение не нужен.

## Предлагаемая точка расширения (если подтвердится необходимость)
1. **Эргономические helper'ы поверх существующего API** (предпочтительно):
   - namespaced get/set/merge для mod-owned поддерева данных;
   - утилиты сериализации/дефолтов/миграций версии схемы;
   - явный контракт по моменту сохранения и частоте записи.
2. **События жизненного цикла персистентного состояния:**
   - load/clone(respawn/dimension)/save hooks для player mod-data с гарантированным порядком.
3. **Документационные улучшения (минимум):**
   - чётко описать ограничения и best practices текущего `player_persistent_data` API (размер, частота, поток, формат).

## Критерии приёмки
- [ ] Мод может безопасно хранить и обновлять сложное состояние adaptionwheel (включая эволюцию схемы между версиями порта).
- [ ] Семантика загрузки/сохранения/миграции документирована и предсказуема (login, respawn, dimension change, disconnect, stop).
- [ ] Нет необходимости в хрупких обходах (дублирование в нескольких хранилищах, частые полные перезаписи без контракта).
- [ ] Производительность приемлема для ожидаемой частоты обновлений (без лишнего тикового I/O/аллокаций).
- [ ] Есть минимальный пример/рекомендованный паттерн для модов.

## Источники
- Horizon native hooks catalog: https://github.com/horizon-rust/horizon/blob/main/native-hooks.json
- Horizon native mod docs: https://github.com/horizon-rust/horizon/blob/main/docs/native-mods.md
- Horizon config dir / mod data docs context: https://github.com/horizon-rust/horizon/blob/main/docs/mods.md
- Horizon mod API fork: https://github.com/arismoko/horizon
- Портируемый мод: adaptionwheel (Java/NeoForge 1.21.1), `../adaptionwheel` (уточнить состав сохраняемого состояния после прототипа)
