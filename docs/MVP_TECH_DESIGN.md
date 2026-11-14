# MVP Technical Design (PR-1)

Основано на подтверждённых возможностях Horizon: **attachments**, **mod messages API 40**, **player lifecycle hooks**, `living_incoming_damage`, `living_fall`, **commands/UI hooks**. Документ фиксирует технический дизайн MVP для PR-1/PR-2; детали точных сигнатур API помечены как `TBD(Horizon API)` и должны быть сверены с целевым Horizon commit/hooks.

---

## 1. Цели MVP и границы

### В скоупе
- Сервер-авторитетное ядро адаптаций для **3–5 репрезентативных сценариев**.
- Персистентное состояние игрока через Horizon attachments (schema v1, версионирование).
- Временная модель активации колеса (без Curios).
- Синхронизация состояния/событий server↔client через mod messages (API 40).
- Минимальная клиентская индикация статуса/прогресса.

### Вне скоупа MVP
- Полный набор концептов/доменов, мутации, existence, synergies, fist/combat прогрессии, adversity, debuff-покрытие «всё».
- Меч/проектайлы/сложные FX, блоки-станции с логикой, полноценные GUI (wheel/cache/trade), Curios-эквивалент.
- Миграция данных Java-версии, внешние интеграции.

---

## 2. Модель состояния игрока (Attachment)

### 2.1 AttachmentType и размещение
- **Attachment:** `adaptionwheel:player_state` (имя уточнить по API), привязан к игроку.
- **Server source of truth:** все мутации состояния выполняются на сервере; клиент хранит только зеркальную копию для UI.
- **Регистрация:** в native-модуле на этапе инициализации (`TBD(Horizon API): register_attachment_type`).

### 2.2 Схема данных (v1)

```text
PlayerAdaptationStateV1 {
  schema_version: u32 = 1,

  // Активация (временная модель MVP)
  wheel_active: bool,                 // вычисляется/кэшируется сервером, но можно хранить последний результат
  active_source: WheelSourceTag,      // NONE | MAIN_HAND | OFF_HAND | HOTBAR (диагностика/отладка)

  // Прогресс и результаты по сценариям MVP
  entries: map<ConceptId, AdaptationEntryV1>,
  // ConceptId (string/enum-id): напр. "type_fall", "type_fire", "env_lava", "type_generic_hostile"

  // Служебное
  dirty: bool (runtime, не обяз. сериализовать),
  last_sync_tick: u64 (runtime)
}

AdaptationEntryV1 {
  level: u8,                          // 0..=MAX_LEVEL (для MVP можно 0..=1 или 0..=3 по сценарию)
  progress: f32,                      // 0.0..=threshold (накопление анализа)
  completed: bool,                    // достигнут целевой уровень/иммунитет сценария MVP
  cooldown_until_tick: u64,           // антиспам/повторный анализ при необходимости
  flags: u16                          // резерв (e.g. suppressed, migrated)
}

WheelSourceTag = enum(u8) { NONE=0, MAIN_HAND=1, OFF_HAND=2, HOTBAR=3 }
```

**Примечания:**
- Для MVP достаточно детерминированного набора ключей `ConceptId` (фиксированный список из 3–5 сценариев), чтобы избежать динамического роста схемы.
- `progress` хранить как накопленную «единицу анализа» (секунды/очки/нормализованное значение) — порог задаётся конфигом/константой сценария.
- Резервировать место под расширение (`flags`, версия схемы), не усложняя v1.

### 2.3 Версионирование и миграции
- `schema_version` — обязательное поле сериализованной формы.
- **Правила загрузки:**
  - `v == 1`: десериализовать как есть.
  - `v < 1` / неизвестная: инициализировать дефолтом v1 + лог warning (без краша).
  - `v > 1` (из будущего): best-effort чтение известных полей или reset до безопасного дефолта (TBD по политике), с логом.
- **API миграций:** `migrate_state(raw, from_version) -> PlayerAdaptationStateV1` (чистая функция/шаг при load).
- **Совместимость сети:** протокол sync включает `state_schema`/protocol version (см. §4), чтобы клиент мог игнорировать неподдерживаемые расширения.

### 2.4 `copy_on_death` и жизненный цикл
- **MVP поведение по умолчанию:** `copy_on_death = true` для прогресса/полученных адаптаций (состояние игрока, не предмета), если целевой Horizon API поддерживает семантику копирования attachment при респавне.
  - Обоснование: соответствует оригинальному `persistOnItem/clearOnDeath` по смыслу «прогресс не теряется неожиданно» в MVP; точное соответствие конфигам — P1.
- **Альтернатива при ограничениях API:** если copy-on-death недоступен/нестабилен, явно сбрасывать в дефолт при смерти и фиксировать как временный parity-gap в `docs/HORIZON_NOTES.md` + `DECISIONS.md`.
- **Lifecycle hooks (player):**
  - `on_player_join`: загрузить/инициализировать attachment, пересчитать `wheel_active`, отправить полный sync клиенту.
  - `on_player_respawn/clone` (если есть): убедиться, что attachment скопирован/восстановлен согласно policy; отправить sync.
  - `on_player_leave`: flush/гарантировать сохранение (обычно движком), снять runtime-кэши.
  - `on_player_tick` (сервер, с троттлингом): периодические проверки активации/экспозиции, если нужны (осторожно с производительностью).

### 2.5 Sync-модель attachment
- **Авторитет:** сервер.
- **Триггеры sync:** join/respawn, изменение `wheel_active`, изменение `progress` сверх дельта-порога или `completed/level` переход, явные debug-команды.
- **Формы:** полный снимок (join/resync) и дельта (изменившиеся entries + активность). См. §4.
- **Частота:** коалесинг/батчинг изменений в пределах тика или по таймеру (напр. не чаще 2–5 раз/сек на игрока для progress-updates), мгновенная отправка на completion/activation toggle.

---

## 3. Временная активация колеса (без Curios)

### 3.1 Правило активации (MVP)
Детерминированная серверная функция `compute_wheel_active(player) -> (bool, WheelSourceTag)`:

- **Активно**, если в инвентаре игрока в одном из разрешённых мест есть валидный предмет колеса MVP:
  1. **Offhand** (приоритет 1) — рекомендованный дефолт, минимально конфликтует с боем/строительством.
  2. **Main hand** (приоритет 2, опционально включаемый) — удобно для теста, но может мешать; можно выключить конфигом.
  3. (Опционально P1/MVP+): выделенный слот хотбара, если Horizon даёт простой хук проверки.
- **Неактивно** иначе.
- **Конфиг MVP:** `activation.mode = "offhand" | "offhand_or_mainhand"` (дефолт `offhand`).

**Почему так:** не требует нового UI/слота, реализуемо через проверку инвентаря в lifecycle/tick/инвентарных событиях (если доступны) и даёт чёткую серверную проверку.

### 3.2 Пересчёт активации
- Пересчитывать при: join/respawn, смене предметов в руках (если есть hook), изменении выбранного слота, периодическом server tick (редко, fallback, напр. раз в 10 тиков), и по команде отладки.
- При смене `wheel_active`: обновить attachment, запустить/остановить накопление анализа (движок игнорирует события при inactive, кроме безопасного чтения), отправить sync клиенту.

### 3.3 Будущая замена
- Интерфейс `WheelActivationProvider` в коде: MVP-реализация `InventorySlotProvider`; позже заменить на Curios-эквивалент/спец-слот без изменения движка и протокола.

---

## 4. Каналы и протокол mod messages (API 40, server↔client)

### 4.1 Каналы
- `adaptionwheel:state_sync` — **S→C**: полный/дельта sync состояния.
- `adaptionwheel:state_ack` — **C→S** (опционально): подтверждение/запрос ресинка при рассинхроне (MVP можно опустить, оставить TODO).
- `adaptionwheel:client_event` — **C→S** (опционально, MVP минимально): зарезервировано под будущие действия UI/клавиши; в MVP не требуется, кроме debug при необходимости.
- `adaptionwheel:debug_action` — **C→S** или command-only: действия отладки за серверной валидацией (предпочтительно через commands hooks, не через произвольный client message).

> Точные идентификаторы/регистрация каналов: `TBD(Horizon mod messages API 40)`.

### 4.2 Версионирование и envelope
Все payload'ы содержат общий конверт:

```text
Envelope {
  proto_version: u16 = 1,
  msg_type: u8,
  payload: bytes/structured
}
```

- Несовместимый `proto_version`: лог + игнор (клиент может запросить resync при join-механизме; сервер не доверяет клиенту).

### 4.3 Сообщения (MVP)

#### `S→C FullStateSync` (msg_type=1)
Отправляется на join/respawn/forced resync.
```text
FullStateSync {
  state_schema: u32,
  wheel_active: bool,
  active_source: u8,
  entries: array<{ concept_id: string_id, level: u8, progress: f32, completed: bool }>
}
```

#### `S→C DeltaStateSync` (msg_type=2)
Коалесированные изменения.
```text
DeltaStateSync {
  wheel_active: optional<bool>,
  active_source: optional<u8>,
  updated_entries: array<{ concept_id: string_id, level: u8, progress: f32, completed: bool }>,
  removed_entries: array<string_id> (MVP обычно пусто)
}
```

#### `S→C AdaptationEvent` (msg_type=3, опционально)
Для немедленной клиентской индикации (toast/звук-хук позже).
```text
AdaptationEvent {
  kind: u8,                  // PROGRESS_MILESTONE=1, COMPLETED=2, ACTIVATION_CHANGED=3
  concept_id: optional<string_id>,
  level: optional<u8>,
  progress: optional<f32>
}
```

### 4.4 Надёжность и порядок
- Полагаться на гарантии транспорта Horizon mod messages (TBD). MVP-дизайн устойчив к потере дельты: клиент может хранить `last_full_sync_tick`, а сервер периодически/по событию может слать FullStateSync при подозрении на рассинхрон (или по reconnect/join).
- Сервер сериализует отправку per-player в порядке применения изменений состояния.

### 4.5 Безопасность
- Клиент **не** авторизован менять адаптации/прогресс; входящие `client_event` (если появятся) — только как запросы, валидируемые сервером.
- Debug-действия — только при разрешении (dev/permission/command context).

---

## 5. Серверный движок анализа (MVP, 3–5 сценариев)

### 5.1 Архитектура
`adapt_engine` (сервер): обработчики событий + чистые правила начисления.

- Вход: хуки Horizon + текущее `PlayerAdaptationStateV1` + конфиг.
- Выход: мутации `entries`, флаги `dirty/completed`, события для sync/UI.
- Ограничение: **только если `wheel_active == true`** (кроме чтения состояния).

### 5.2 Общая модель начисления
Для каждого сценария:
- `threshold` — сколько «очков анализа» нужно (секунды/урон-события/нормализованные единицы).
- `progress += gain(event, context)` (с капом/кулдауном при необходимости).
- При `progress >= threshold` → `completed=true`, `level = target_level` (MVP: обычно 1, «адаптирован»), сгенерировать `AdaptationEvent(COMPLETED)`.
- `reduction/mitigation` применяется при наличии завершённой адаптации (где применимо к сценарию).

Константы/пороги — в `adapt_defs`/конфиге (MVP: хардкод допустим с TODO вынести в config).

### 5.3 Набор сценариев MVP (предложение: 5)
Выбраны так, чтобы покрыть разные типы хуков и дать наглядный результат:

| ID | Сценарий | Источник события/хука | Условие анализа (пример) | Эффект MVP при completion |
|---|---|---|---|---|
| `env_fall` | Падение | `living_fall` (distance/damage) | накопить N "очков падения" (напр. сумма дистанции/урона до порога) при получении урона от падения | сильное снижение/иммунитет урона от падения (напр. ×0.1 или 0) |
| `env_fire` | Огонь/горение | `living_incoming_damage` (damage type/tag fire/on fire context `TBD`) + опц. tick в огне | накопить N очков экспозиции/урона огненного типа | снижение урона от огня/горения (напр. ×0.2) |
| `env_lava` | Лава | `living_incoming_damage` (lava damage type) | накопить N очков урона/событий от лавы | снижение урона от лавы (MVP, без изменения физики плавания) |
| `type_generic_hostile` | Общий входящий урон от моба/атаки | `living_incoming_damage` (attacker/entity damage category) | накопить N очков полученного урона (cap per hit/tick) | плоское/процентное снижение соответствующей категории урона (напр. ×0.7) |
| `env_drowning` (опц. 5-й) | Утопление | `living_incoming_damage` (drown type) или player tick в воде без воздуха (`TBD hook`) | накопить N событий/очков | снижение/иммунитет урону утопления |

**Примечания реализации:**
- Если точная классификация damage type в Horizon ограничена, `type_generic_hostile` можно определить как «любой урон от живого атакующего» с консервативным фильтром.
- `env_drowning` включать только если доступен надёжный сигнал (иначе оставить 4 сценария в MVP).
- Нормализация: `gain = clamp(raw, min, max_per_event)`, чтобы один экстремальный удар не завершал анализ мгновенно (конфигурируемо).

### 5.4 Применение эффектов
- В `living_incoming_damage`/ `living_fall`: если сценарий `completed`, модифицировать результат (уменьшить damage/отменить) в соответствии с поддерживаемым способом хука (`TBD: mutable event / return modifier`).
- Порядок: сначала определение сценария по контексту, затем применение mitigation, затем (если урон всё ещё прошёл и колесо активно) начисление progress.

### 5.5 Команды отладки (через commands hooks)
- `/adaptionwheel inspect` — показать active, entries/progress.
- `/adaptionwheel grant <concept> [level]` — выдать адаптацию (server-only permission).
- `/adaptionwheel reset` — сбросить состояние attachment.
- После команд: пометить dirty и выполнить sync.

---

## 6. Минимальная клиентская индикация (UI hooks)

### 6.1 Цели UX MVP
- Понять: колесо активно/неактивно.
- Видеть: прогресс по сценариям и какие адаптации получены.
- Получать мгновенный сигнал о завершении адаптации.

### 6.2 Варианты реализации (минимум → лучше)
1. **HUD overlay/text (MVP baseline):** через UI hooks отрисовать компактный блок в углу экрана:
   - строка статуса: `Wheel: ON/OFF (offhand)`
   - до N строк: `env_fall: 45% | env_fire: DONE | ...`
   - обновление из локального зеркала состояния после `state_sync`.
2. **Chat/action feedback (fallback):** при `AdaptationEvent(COMPLETED/ACTIVATION_CHANGED)` выводить локальное сообщение (если HUD overlay сложен на этапе PR-1).
3. **Toast/звук (P1):** не в MVP, зарезервировать `AdaptationEvent`.

**Рекомендация:** реализовать (1) как основной путь, (2) как простой fallback/дополнение; оба питаются от одного client-state store.

### 6.3 Клиентское состояние
- `ClientAdaptationState` (зеркало): `wheel_active`, `active_source`, `entries`.
- Обновляется только из `S→C` сообщений; не модифицируется локальной игровой логикой.
- При потере соединения/ресинке — заменяется FullStateSync.

---

## 7. Интеграционные точки Horizon (TBD при реализации)

- **Attachments:** API регистрации, тип ключа/сериализатор, семантика `copy_on_death`, события clone/respawn.
- **Mod messages API 40:** регистрация каналов, codec/буфер, гарантии доставки/порядка, направление S→C/C→S.
- **Hooks:** сигнатуры `living_incoming_damage` (доступ к источнику/типу/атакующему/мутабельному урону), `living_fall` (дистанция/итоговый урон), player lifecycle/join/respawn/tick, inventory/hand change (если есть).
- **Commands/UI hooks:** регистрация команд и контекст пермишенов; API HUD-отрисовки/экранного текста и поток рендера.
- **Item identification:** как надёжно определить предмет колеса (id/registry key/tags) в `content/adaptation`.

---

## 8. План внедрения (PR-1 → PR-2)

- **PR-1 (design + scaffold hooks wiring points):** зафиксировать IDs/константы сценариев, схемы state/message, интерфейсы `ActivationProvider`, `AdaptationRule`, заглушки обработчиков и docs.
- **PR-2 (implementation MVP):** attachment v1 + lifecycle, activation provider, 3–5 правил в `living_incoming_damage`/`living_fall`, sync full/delta, минимальный HUD/feedback, debug-команды, базовые проверки.
- **Критерии приёмки PR-2:** колесо активируется по временной модели; сценарии накапливают прогресс и завершаются; mitigation применяется; состояние переживает релог (и респавн — если подтверждено API); клиент видит актуальный статус.

---

## 9. Риски и смягчение
- **Нет copy_on_death/clone hook:** документировать gap, fallback-политика (reset on death) за флагом.
- **Ограниченная классификация damage types:** начать с грубых категорий (fall/fire/lava/generic hostile) и уточнить маппинг после проверки API.
- **Нет удобного HUD hook:** fallback на chat/action сообщения в MVP.
- **Производительность tick-проверок:** пересчёт активации/экспозиции по событиям, tick — редкий fallback с троттлингом.
