# [API][Conditional] Контекст/пейлоад damage hooks для надёжной классификации сценариев и привязки к игроку (adaptionwheel)

> Статус: **ready to file if blocker confirmed** (conditional gap; подавать только после подтверждения блокера прототипом на целевом Horizon).
> Target Horizon (baseline notes): `139e83fff8` (`docs/HORIZON_NOTES.md`); актуальные PR порта (#4/#5) таргетируют `f4d8d62c31` — evidence/blocker подтверждать на фактическом target build/commit используемого прототипа.
> Трек подачи: **требует подтверждения**. Канонический трекер — https://github.com/horizon-rust/horizon/issues, но перед внешней подачей необходимо сверить актуальный канал/ожидания мейнтейнеров (repo docs/Discord) и явно указать подтверждённый target/rev + blocker evidence.

## Контекст и ссылки
- Baseline/наблюдения: `docs/HORIZON_NOTES.md` (damage/env hooks: `living_incoming_damage(_v2)`, документированный `living_incoming_damage_v3`, `living_fall`, `living_drown`, block/fluid/tick hooks), `docs/AGENT1_API_GAP_AND_READINESS.md`.
- Реализация порта: PR #4/#5, `native/adaptation/api/src/lib.rs` (классификация damage types, in-memory state/progress, команды); PR #5 отмечает ограничения runtime entity/player mapping и payload classification в некоторых путях.
- Источники adaptionwheel: `../adaptionwheel` (сценарии получения урона/окружения и привязка прогресса к игроку).

## Use-case из adaptionwheel
Для адаптаций к урону/окружению серверная логика должна надёжно:
- определить, что пострадавшая сущность — целевой игрок (и его идентификатор/контекст в mod API);
- классифицировать источник/тип урона (explosion, fire/lava/heat, contact/cactus/magma-like, generic/environmental, drowning/fall и т.п.) с достаточной точностью для MVP/MVP+ сценариев;
- при необходимости различать прямой/косвенный урон или источник (attacker/projectile/block/environment), чтобы корректно начислять прогресс и применять mitigation;
- обрабатывать события в правильной фазе (до/после применения урона, mutable amount/cancel semantics) без гонок с другим модовым/ванильным состоянием.

## Что проверено/известно в Horizon (по доступным материалам)
- В `native-hooks.json`/docs присутствуют `living_incoming_damage`, `living_incoming_damage_v2` (additive), `living_fall`, `living_drown`; docs также описывают `living_incoming_damage_v3` (richer actor/source info, порядок относительно v1) — фактическая доступность/сигнатура зависят от target build (`139e83fff8` vs `f4d8d62c31`).
- Для падения/утопления есть прямые hooks; часть environmental/fire/lava/contact случаев может приходить через общий damage hook и требовать классификации по damage type/source.
- Для движения по поверхностям (powder snow/soul sand и т.п.) выделенных damage hooks может не быть; текущий подход допускает `player_tick` + проверки позиции/блока.

Перед подачей issue: подтвердить на целевом build имена hooks, сигнатуры/аргументы, момент вызова, mutability/cancel semantics и доступность victim/player UUID и source metadata.

## Почему текущих возможностей может быть недостаточно (условно)
- Недостаточно информации для однозначной привязки события к игроку в mod API (или неудобный/нестабильный способ получить player context/UUID в нужном hook).
- Damage type/source metadata неполная/неоднородная для сценариев adaptionwheel (невозможно надёжно отличить нужные классы урона без хрупких эвристик).
- Порядок hooks/фаз не позволяет безопасно реализовать server-authoritative mitigation/progress (например, изменение amount недоступно там, где требуется, или состояние уже финализировано).
- Покрытие важных environmental случаев отсутствует или требует дорогих обходов в `player_tick` с риском пропусков/ложных срабатываний.

Если прототип на target build показывает, что существующие hooks дают достаточный контекст для MVP/MVP+ сценариев без хрупких обходов, issue не подавать.

## Предлагаемая точка расширения (минимально необходимая, по результатам прототипа)
1. **Уточнение/расширение payload damage hooks:** стабильный доступ к victim entity/player UUID, damage type/resource id, amount (read/write где применимо), source entity/block/cause metadata, side/phase.
2. **Стабильный контракт фаз:** документированный порядок `*_v1/_v2/_v3` (или эквивалент) и гарантии по mutability/cancel.
3. **При необходимости:** специализированные hooks/helpers для недостающих environmental контактов, если общий damage hook не покрывает сценарий надёжно.

Предпочтителен вариант с минимальной поверхностью и совместимостью с текущими hook conventions.

## Критерии подтверждения блокера при прототипе (до подачи issue)
- [ ] На фактическом target Horizon build воспроизведён сценарий adaptionwheel, который нельзя надёжно реализовать с текущими hooks/helpers.
- [ ] Зафиксированы конкретные отсутствующие/неудобные поля/семантики (имена hooks, аргументы, что пробовали, результат).
- [ ] Показано, почему обход (другие hooks, `player_tick` polling, эвристики по типам) недостаточен (ложные срабатывания, пропуски, race, производительность, отсутствие данных).
- [ ] Приложены воспроизводимые артефакты: ветка/коммит/PR, команды/окружение, логи/трассировка событий, ожидаемое vs фактическое поведение.

## Критерии приёмки (для будущего API/уточнения контракта)
- [ ] Мод может надёжно определить целевого игрока и классифицировать релевантные типы урона/окружения для согласованных MVP/MVP+ сценариев.
- [ ] Контракт вызова/фаз/mutability/cancel задокументирован и достаточен для server-authoritative progress/mitigation.
- [ ] Не требуются хрупкие обходы с недетерминированным поведением для покрываемых сценариев.
- [ ] Есть минимальный пример использования в native mod API.

## Куда и как подавать
- **Куда (уточнить перед подачей):** канонический трекер — https://github.com/horizon-rust/horizon/issues, но фактический канал для mod API/hooks gaps необходимо подтвердить с мейнтейнерами (repo docs/Discord). Исторические ссылки на `arismoko/horizon` могут быть устаревшими.
- **Как:** описать конкретный use-case/scenario, подтвердить target/rev, перечислить использованные hooks/helpers и наблюдаемые ограничения, предложить минимальное расширение контракта, приложить repro/evidence.
- **Когда подавать:** только после выполнения раздела "Критерии подтверждения блокера при прототипе" и подтверждения корректного канала подачи.
