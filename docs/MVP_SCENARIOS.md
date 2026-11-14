# MVP Scenarios Spec — adaptionwheel port to Horizon (PR-1, Agent #1)

Документ описывает 5 целевых MVP-сценариев порта на основе исходников `adaptionwheel` (NeoForge 1.21.1, `ru.adaptionwheel`, mod 0.1.6).  
Фокус: минимальный end-to-end поток **триггер → анализ/прогресс → завершение/уровень → эффект → sync/persist**, с сохранением ключевых инвариантов оригинала.

> Примечание: в Horizon API названия хуков/событий и точки интеграции могут отличаться; ниже зафиксированы **поведенческие контракты** и исходные референсы.

---

## Общие правила (применимо ко всем сценариям)

### Базовые компоненты (референс)
- **Данные игрока/колеса**: `data/PlayerAdaption.java`, `data/WheelData.java`, `data/AdaptionTask.java`, `data/Extras.java`, `data/AttachmentTypes.java`, `data/ModDataComponents.java`.
- **Реестр/концепты**: `adapt/AdaptationRegistry.java`, `adapt/AdaptationDefinition.java`, `adapt/AdaptationDomain.java`, `category/Concepts.java`, `category/AdaptionCategory.java`.
- **Серверная логика**: `server/AdaptionEvents.java`, `server/MovementTriggers.java`, `server/DomainTriggers.java`, `server/AdaptationProgression.java` (и связанные хелперы).
- **Конфиг/тайминги**: `config/AdaptionConfig.java`, `AdaptionTimings.java`.
- **Сеть/клиент**: `network/AdaptionNetworking.java`, `network/AdaptionSyncPayload.java`, `network/TaskProgressPayload.java`, `client/ClientAdaption.java`, `client/AdaptionHud.java`.

### Ключевые инварианты (обязательно сохранить в MVP-ядре)
1. **Server authority**: мутации состояния (start/accelerate/complete/grant/level-up/save/sync) выполняются на сервере; клиент получает зеркальное состояние и только отображает/шлёт intent там, где это предусмотрено.
2. **`active` vs `adapted/level`**: отключённая адаптация не должна применяться как активный эффект/не должна продолжать анализ, но её прогресс/факт получения не теряются. (Для MVP можно не делать UI toggle, но модель данных/гейты стоит заложить корректно.)
3. **Merge, не replace**: загрузка данных колеса/игрока при экипировке/входе должна объединять состояние (max levels, union sets, без дублей задач), а не затирать прогресс.
4. **Дедупликация задач**: для одного концепта не должно быть нескольких одновременных `AdaptionTask`.
5. **Персистентность после релога**: состояние (минимум: adapted/levels/tasks/relevant counters) восстанавливается и корректно синхронизируется клиенту.

### Общий паттерн анализа/завершения (из `AdaptionEvents`)
- `startTask(player, data, concept, timer)`: создаёт задачу, если нет активной задачи/концепт не завершён/не на max, соблюдён лимит одновременных анализов, не в adversity, концепт включён; добавляет штраф времени от текущего уровня (кроме offense-формулы для offense-концептов).
- `startOrAccelerate(..., baseTicks, accelerate[, accelerationTicks])`: создаёт задачу или ускоряет существующую (уменьшает `task.timer`), с учётом лимитов/факторов (в т.ч. shedding reattach factor в оригинале).
- `completeTask -> grantConceptLevel/applyGrant`:
  - **one-time** концепт: добавляется в `adapted`, лечение, history, поворот колеса, событие/синхронизация.
  - **leveled** концепт: увеличивается уровень (или вычисляется по порогам для специальных случаев), сообщение/лечение/событие/синхронизация; при max — финальный статус.
- Сервер периодически тикает задачи, завершает по таймеру, применяет статы/эффекты, сохраняет и синхронизирует (`sync/save/applyStats/applyEnvEffects`).

### MVP-упрощения (допустимые)
- Ограничить реестр/триггеры только 3–5 сценариями (без полного покрытия всех категорий/доменных семейств).
- Упростить/зафиксировать тайминги и формулы ускорения (конфигом или константами), сохранив корректный lifecycle задачи.
- Не реализовывать в MVP: adversity, existence, synergies, mutations, shedding, resonance/transfer, altar trade, full UI wheel screen, сложные mixin-поведения.
- Клиентская индикация: простой HUD/чат/тост вместо полного экрана; достаточно отображать активные задачи и завершённые/уровни по MVP-концептам.
- Для `Type_EXPLOSION` можно оставить только базовую кривую снижения урона без спец-кейсов (Chaos Guardian direct path, dimension destroy и т.п.).

---

## Сценарий 1: `Env_Fire` (одноразовая адаптация к огню)

### Источники (adaptionwheel)
- Концепт/регистрация: `category/Concepts.java` (`ENV_FIRE = "Env_Fire"`), `adapt/AdaptationRegistry.java` (one-time, `ENVIRONMENT`), `category/AdaptionCategory.java` (категория `FIRE`, матч damage tags/id).
- Триггеры/обработка: `server/AdaptionEvents.java` (обработка входящего урона/огненного контекста, старт/завершение анализа, иммунитет/применение env-эффектов), `config/AdaptionConfig.java` (`ENV_ANALYSIS_SECONDS`, toggles).
- Связанные инварианты/миграции: `data/LegacyConcepts.java` (переход от старого `Type_FIRE`/ладдера к one-time `Env_Fire`), разделение fire vs lava в `AdaptionEvents`/config/docs.

### Условия старта
- Игрок носит/активировал колесо (серверно подтверждено).
- Модуль environment включён (`AdaptionConfig.ENABLE_ENVIRONMENT` в оригинале).
- Концепт `Env_Fire` ещё не получен (`!isAdapted`) и не отключён, нет дубликата задачи, есть слот под задачу.
- Игрок получает огненный урон/находится в огненном сценарии, относящемся к `AdaptionCategory.FIRE` **без** трактовки как lava-обучение (lava — отдельный концепт `Env_Lava`, не часть MVP).

### Прогресс и завершение
- Создаётся `AdaptionTask("Env_Fire", timer, timer)` через `startTask` с базой `ENV_ANALYSIS_SECONDS` (в оригинале ~3s по умолчанию) + level penalty логика (для one-time обычно старт один раз).
- Повторные огненные события не должны плодить дубликаты задачи; (если реализуется ускорение) ускорение идёт по тем же правилам server-side.
- По достижении `timer <= 0` задача завершается: one-time grant (`adapted` содержит `Env_Fire`), лечение, history/rotation/событие, save+sync.

### Ожидаемый эффект
- После завершения огненный урон категории FIRE блокируется как one-time env-иммунитет (в оригинале через `hasFullImmunity`/`envImmunityConcept` для `FIRE -> ENV_FIRE`, с отдельной веткой для lava).
- Клиент получает обновление и показывает `Env_Fire` как адаптированный (HUD/чат-индикация минимум).

### Важные инварианты и MVP-упрощения
- **Не путать с `Env_Lava`**: урон/контакт с лавой не должен обучать/выдавать `Env_Fire`; в MVP можно вообще не реализовывать `Env_Lava`, но важно не сломать семантику `Env_Fire`.
- **Server authority**: отмена урона/выдача иммунитета решается на сервере.
- MVP-упрощение: считать любой валидный fire-damage тик/событие достаточным триггером старта; не реализовывать тонкие исключения/совместимости.

---

## Сценарий 2: `Env_FallDamage` (одноразовая адаптация к урону от падения)

### Источники (adaptionwheel)
- Концепт/регистрация: `category/Concepts.java` (`ENV_FALL = "Env_FallDamage"`), `adapt/AdaptationRegistry.java` (one-time, `PHYSICS` в реестре доменов/PHYSICS_ENV_IDS), `category/AdaptionCategory.java` (`FALL`).
- Триггеры/обработка: `server/AdaptionEvents.java` (в ветке env-триггеров на урон от падения стартует `startTask(..., Concepts.ENV_FALL, ...)`; выбор длительности зависит от `RAPID_FALL_ANALYSIS`/`FALL_ANALYSIS_SECONDS`/`ENV_ANALYSIS_SECONDS`), иммунитет при завершении.
- Конфиг: `config/AdaptionConfig.java` (`FALL_ANALYSIS_SECONDS`, `RAPID_FALL_ANALYSIS`, `ENABLE_ENVIRONMENT`).

### Условия старта
- Колесо активно у игрока, environment-модуль включён.
- Получен урон падения, распознанный как `AdaptionCategory.FALL`.
- `Env_FallDamage` ещё не адаптирован, не отключён, нет активной задачи по этому концепту, есть лимит слотов задач.

### Прогресс и завершение
- При первом валидном событии падения создаётся задача анализа `Env_FallDamage`.
- Длительность в оригинале: `FALL_ANALYSIS_SECONDS` при `RAPID_FALL_ANALYSIS=true`, иначе `ENV_ANALYSIS_SECONDS` (через умножение на 20 тиков).
- Завершение — one-time grant в `adapted` + стандартные побочные эффекты (heal/history/sync/save).

### Ожидаемый эффект
- Последующий урон от падения (`FALL`) игнорируется (env-иммунитет для завершённого концепта).
- Клиент видит завершённую адаптацию/индикацию.

### Важные инварианты и MVP-упрощения
- Категоризация урона должна соответствовать `FALL` (не подменять другими типами).
- MVP-упрощение: можно использовать фиксированную длительность анализа (например, всегда `FALL_ANALYSIS_SECONDS`) и не поддерживать rapid-режим/доп. поведение.
- Не тащить в MVP связанные мутации/синергии (`Mutation_Impact`, Skybreaker и т.д.).

---

## Сценарий 3: `Type_EXPLOSION` (leveled адаптация к взрывному урону)

### Источники (adaptionwheel)
- Концепт/регистрация: `category/AdaptionCategory.java` (`EXPLOSION`, match по explosion tag/id), `category/Concepts.java` (типовые ключи формата `Type_<CATEGORY>`), `adapt/AdaptationRegistry.java` (leveled `Type_*` для damage categories, кроме специальных исключений).
- Триггеры/обработка: `server/AdaptionEvents.java` (на входящем уроне формирует defense-концепты: для не-env категорий добавляет `Concepts.type(category)`; при `ENABLE_DEFENSE` вызывает `startOrAccelerate` с `DEFENSE_ANALYSIS_SECONDS` и ускорением; также есть специфические ветки с `Type_EXPLOSION` в спец-контекстах, которые в MVP не обязательны), `config/AdaptionConfig.java` (defense timings/reduction tables/level caps/full immunity rules).
- Данные/уровни: `data/PlayerAdaption.java` (`levels`, max level), `category/Concepts.java`/`AdaptationRegistry` (leveled semantics).

### Условия старта
- Колесо активно, defense-модуль включён (`ENABLE_DEFENSE`).
- Игрок получает урон, классифицированный как `AdaptionCategory.EXPLOSION`.
- Концепт `Type_EXPLOSION` (ключ `Concepts.type(EXPLOSION)`) не на максимальном уровне, не отключён, разрешён старт/ускорение задачи.

### Прогресс и завершение
- Если задачи нет: создаётся leveled-задача анализа/защиты через `startOrAccelerate` с базой `DEFENSE_ANALYSIS_SECONDS * 20` тиков (с учётом штрафа от уровня/факторов).
- Повторные попадания взрывным уроном ускоряют существующую задачу (defense acceleration), серверно.
- По завершении: `grantConceptLevel` повышает уровень `Type_EXPLOSION` на +1 (до `PlayerAdaption.MAX_LEVEL`, обычно 8), применяет статы/эффекты, сохраняет и синхронизирует. Для следующего уровня требуется новый цикл анализа.

### Ожидаемый эффект
- С ростом уровня уменьшается входящий взрывной урон согласно defense-таблицам/формулам (в оригинале `defenseReduction(level)` и доп. механики high-level: heal/iframes/max-immunity правила).
- Клиент отображает текущий уровень и/или прогресс анализа.

### Важные инварианты и MVP-упрощения
- **Leveled vs one-time**: `Type_EXPLOSION` хранится в `levels` (не только в `adapted`), гейты/клиент должны учитывать это различие.
- **Server authority**: расчёт снижения урона и level-up только на сервере.
- MVP-упрощение: реализовать базовое снижение урона по уровню (линейно/таблично) без high-level спец-правил (iframes/heal-on-hit/full immunity at max), если это снижает риск; max-level можно трактовать как сильное снижение, а не абсолютный иммунитет.
- Не переносить в MVP спец-кейсы из `handleDirectHealthReduction`/Chaos Guardian/guardian-laser path.

---

## Сценарий 4: `Move_PowderSnow` (предпочтительно) или `Move_SoulSand` (одноразовая movement-адаптация)

> Рекомендуемый выбор для MVP: **`Move_PowderSnow`** (явный триггер состояния `player.isInPowderSnow` в `MovementTriggers`). `Move_SoulSand` допустим как альтернатива (триггер по блоку под ногами/тегу soul speed blocks).

### Источники (adaptionwheel)
- Концепты/регистрация: `category/Concepts.java` (`MOVE_POWDER_SNOW`, `MOVE_SOUL_SAND`), `adapt/AdaptationRegistry.java` (one-time, `MOVEMENT`).
- Триггеры: `server/MovementTriggers.java` (server tick, при ношении колеса и включённом movement-модуле; периодическая проверка каждые 4 тика):
  - `Move_SoulSand`: блок под игроком в `BlockTags.SOUL_SPEED_BLOCKS` (`startTask(..., Concepts.MOVE_SOUL_SAND, envTicks)`).
  - `Move_PowderSnow`: `player.isInPowderSnow` (`startTask(..., Concepts.MOVE_POWDER_SNOW, envTicks)`).
  - База времени: `AdaptionConfig.ENV_ANALYSIS_SECONDS * 20`.
- Эффекты/поведение после адаптации: movement mixins и helpers (`mixin/PowderSnowBlockMixin.java`, `mixin/SpeedFactorMixin.java`/другие movement mixins, `SurfaceAdaptations.java`, `ClientChecks.java`) — в оригинале снимают/меняют штрафы и особые ограничения.
- Доп. упоминания: `server/DomainExchange.java` (alt recipes для обмена концептами), конфиг `ENABLE_MOVEMENT`.

### Условия старта
- Колесо активно, movement-модуль включён.
- Игрок находится в целевом состоянии:
  - для `Move_PowderSnow`: внутри powder snow;
  - для `Move_SoulSand`: на soul sand/soul soil (через `BlockTags.SOUL_SPEED_BLOCKS`).
- Концепт не адаптирован, не отключён, нет дубликата задачи, соблюдён лимит одновременных задач.

### Прогресс и завершение
- Периодический серверный триггер вызывает `startTask` (one-time задача) с длительностью `ENV_ANALYSIS_SECONDS`.
- Пока условие выполняется повторно, дубликат не создаётся (задача уже существует).
- По завершении — one-time grant (`adapted`), save+sync, клиентская индикация.

### Ожидаемый эффект
- `Move_PowderSnow`: игрок перестаёт испытывать стандартные ограничения/штрафы powder snow согласно реализации адаптации (в оригинале — связка сервер/клиент проверок и mixin-поведения).
- `Move_SoulSand`: снимается/уменьшается штраф движения по soul sand/soul soil (в рамках реализованной поддержки Horizon).

### Важные инварианты и MVP-упрощения
- Триггер должен быть серверным и не зависеть от клиентских побочных эффектов.
- MVP-упрощение (рекомендуется): реализовать только **факт адаптации + один заметный эффект** (например, отмена замедления/проваливания по доступным в Horizon хукам) без полного набора mixin-паритетов со всеми edge-cases.
- Если в Horizon нет прямого эквивалента mixin/физики блоков, допустимо временно ограничиться семантикой "адаптирован" + частичным эффектом и зафиксировать ограничение в notes к MVP.

---

## Сценарий 5 (опционально): `Contact_minecraft:zombie` (leveled контактная защита от конкретного моба)

### Источники (adaptionwheel)
- Концепт/формат: `category/Concepts.java` (`CONTACT_PREFIX = "Contact_"`, `Concepts.contact(...)`), `adapt/AdaptationRegistry.java`/`domainOf` (динамическое семейство `Contact_*` относится к `ENTITY`), `category/AdaptionCategory.java` (классификация урона).
- Триггеры/обработка: `server/AdaptionEvents.java` (в damage pipeline при ударе живой сущности добавляет `Concepts.contact(mobPath)` в обрабатываемые концепты; считает reduction с использованием `AdaptionConfig.contactProtection(level)`; при `ENABLE_DEFENSE` вызывает `startOrAccelerate` для contact-концепта с defense-таймингами/ускорением).
- Команды/селекторы: `server/AdaptionCommand.java` (поддержка `contact <mob>`, корректная работа с namespaced id через selectors/quoting).
- Конфиг: `config/AdaptionConfig.java` (defense timings, contact protection levels/thresholds, related immunity rules).

### Условия старта
- Колесо активно, defense-модуль включён.
- Игрок получает урон от конкретного моба (напр. зомби), и источник удара разрешается в living attacker (direct/contact context в оригинальной логике).
- Формируется концепт `Contact_minecraft:zombie` (или эквивалентный id моба в Horizon namespace-схеме).
- Концепт не на max, не отключён, соблюдены лимиты/дедуп задач.

### Прогресс и завершение
- Первая валидная атака создаёт задачу анализа контакта; последующие атаки того же типа моба ускоряют прогресс (server-side).
- Завершение повышает уровень contact-концепта (`levels`) на +1, сохраняет/синхронизирует состояние.
- Повторные циклы (получение урона снова) дают дальнейшие уровни до max.

### Ожидаемый эффект
- Увеличение защиты именно против контакта/атак данного моба (в оригинале — отдельная ветка reduction через `contactProtection(level)`, не идентичная обычному `Type_*`).
- В HUD/индикации отображается концепт/уровень для конкретного моба.

### Важные инварианты и MVP-упрощения
- **Идентичность моба должна быть стабильной** (namespaced id/path), иначе прогресс размажется по разным ключам.
- **Не смешивать с offense/loot/existence**: в MVP это только защита от контакта (`Contact_*`), без `Offense_NPC_*`, `Drop_NPC_*`, `Existence_*`.
- MVP-упрощение: поддержать 1 тестового моба (zombie) и прямой melee/contact damage path; не реализовывать сложные projectile-owner/indirect attribution кейсы, если Horizon API не даёт их просто.

---

## Рекомендации по приёмке MVP (по сценариям)
- **Старт анализа**: событие/условие создаёт ровно одну задачу (без дублей) и отображается в клиентской индикации.
- **Прогресс/завершение**: таймер доходит до завершения, состояние меняется server-side, клиент получает sync.
- **Эффект**: наблюдаемое изменение поведения/урона после завершения (для leveled — хотя бы уровень 1 с видимым эффектом).
- **Персистентность**: после релога (и/или повторного входа в мир) состояние и уровень/завершённость сохраняются.
- **Инварианты**: без client-authoritative мутаций, без потери данных при equip/relogin (в рамках реализованного MVP-потока).
