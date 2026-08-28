---
title: Meridian
document_type: readme
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-08-18
updated: 2026-08-28
---

# Meridian

Переносимое ядро для дисциплинированной работы AI-агентов над инженерными
задачами: жизненный цикл задачи, жизненный цикл и статусная модель документов,
маршрутизация верификации, дисциплина доказательств и governance версионирования —
как самостоятельная версионируемая единица поставки.

**Status:** `draft` · **Version:** [`VERSION`](VERSION) · **Visibility:** private

Впервые здесь — [`start-here.md`](start-here.md): что это и зачем, на простом
языке. Дальше [`MANUAL.md`](MANUAL.md) — как делать конкретные вещи.

---

## 1. Что это

Meridian — **Kernel**: методология, стандарты, writers, verification router,
правила governance и механический валидатор. Он не знает, какой продукт
обслуживает и на какой машине запущен. Одну зависимость он
пока знает: стандарты сформулированы в терминах Confluence (шаблоны,
publication flow, статусная модель страниц). Привязка к конкретному типу
wiki остаётся first-class и зафиксирована как открытое решение D-3 — она
не выдаётся за независимость.

Всё, что специфично для продукта — реестр репозиториев, окружения, команды,
smoke-флоу, рабочая память задач — живёт в отдельном репозитории **Instance** и
никогда сюда не попадает.

Instance размещается там, где находятся данные продукта: репозиторий с фактами
продукта живёт на инфраструктуре этого продукта, а не рядом с инструментом. Meridian это не фиксирует и не может зафиксировать — он нигде не
хранит ни адреса, ни remote, ни submodule Instance, только имя переменной
окружения `MERIDIAN_INSTANCE`. Поэтому смена места хранения Instance не является
изменением Kernel.

Граница нормативна, а не стилистична:
[`standards/workspace/kernel-boundary.md`](standards/workspace/kernel-boundary.md).
Проверяется механически: [`scripts/kernel-validate.mjs`](scripts/kernel-validate.mjs).

## 2. Зачем разделение

Одно техническое состояние обслуживает три сценария, которые иначе были бы тремя
разными переделками:

1. применение инструмента к другому продукту;
2. публикация как открытого репозитория;
3. упаковка и передача третьей стороне.

Все три требуют, чтобы Kernel не содержал имени ни одного продукта. Это не
самоочевидное свойство, а проверяемое утверждение с известными пределами:
gate ищет литералы и паттерны, объявленные текущим Instance, и ловит ровно
то, что объявлено, — одна утечка (имя компонента текущего продукта) была
найдена внешним ревью уже после первого «чистого» прогона и устранена вместе
с расширением списка. Сценарий 2 дополнительно ограничен привязкой к
Confluence (D-3), сценарии 2/3 — решениями D-1/D-2.

## 3. Структура

```
meridian/
├── VERSION  CHANGELOG.md  COMPATIBILITY.md  LICENSE
├── standards/          методология, lifecycle, статусная модель, writers, шаблоны
│   └── workspace/kernel-boundary.md    ← нормативная граница
├── workflows/          жизненный цикл задачи
├── verification/       verification router, regression-правила, smoke-protocol, functional-parity
├── skills/             завендоренные skill-пакеты с SHA-пинами
├── registries/         правила и JSON Schema реестров (не данные)
│   └── rule-resolution/ схемы применимости норм и вывода резолвера (PHASE B)
├── instance-template/  bootstrap-каркас нового Instance (первый день на продукте)
├── scripts/            kernel-validate.mjs, preflight.mjs, rule-resolver.mjs
│   └── lib/            общие чистые helpers (YAML-subset, JSON-Schema-subset, marked-region reader)
├── test/               instance-fixture/ и регрессионные наборы валидатора и резолвера
├── hooks/              pre-push: тесты валидатора + прогон против fixture
└── .github/workflows/gate.yml
```

## 4. Запуск валидатора

```bash
node scripts/preflight.mjs                            # сессия подключена к правильным корням?
MERIDIAN_INSTANCE=/path/to/meridian-instance-<product> node scripts/kernel-validate.mjs
node scripts/kernel-validate.mjs                      # без Instance: явный FAIL, не молчание
MERIDIAN_INSTANCE=$PWD/test/instance-fixture node scripts/kernel-validate.mjs   # как в CI
node test/kernel-validate.test.mjs                    # правила валидатора действительно срабатывают
```

Preflight — первый шаг любой агентной сессии: он громко падает, если сессия
открыта против устаревших корней или без `MERIDIAN_INSTANCE`, вместо того
чтобы молча читать не те правила. Bootstrap нового Instance начинается с
[`instance-template/instance-bootstrap.md`](instance-template/instance-bootstrap.md).

## 4a. Резолвер норм

`scripts/rule-resolver.mjs` (PHASE C программы `MERIDIAN-RULE-RESOLUTION-001`) —
read-only механика ядра рядом с валидатором. Она отвечает на вопрос «какие
агентные нормы, протоколы и верификация применимы к этой единице работы и
почему» как детерминированная функция от явного входа: один и тот же вход на
одной ревизии источников даёт байт-идентичный, одинаково упорядоченный вывод
по контракту
[`registries/rule-resolution/resolver-output.schema.json`](registries/rule-resolution/resolver-output.schema.json).
Это не расширение `kernel-validate.mjs`; гейтом остаётся валидатор. Общий
YAML-reader, JSON-Schema-движок и reader маркированных участков вынесены в
`scripts/lib/`, чтобы оба скрипта использовали одну реализацию, а не копию;
участок норм-контейнера пересчитывается тем же reader'ом — маркеры ищутся в
буфере с погашёнными fenced-блоками (пример не становится объявлением), но в
дайджест идёт дословный source-срез участка между маркерами, со всеми
символами, включая fenced-код; объявленный, но отсутствующий/дублированный/
незакрытый участок — fail-closed, без отката к чтению всего файла.

```bash
node test/rule-resolver.test.mjs                       # десять acceptance-кейсов + fail-closed пути

MERIDIAN_INSTANCE=/path/to/meridian-instance-<product> \
node scripts/rule-resolver.mjs \
  --request <request.json> \
  --applicability <applicability-register.yaml|json> \
  [--protocol-routes <routes.json|yaml>] \
  [--verification-routes <routes.json|yaml>] \
  [--prior-state <prior.json|yaml>]
```

- `--request` — JSON `{ "work_item": { … } }` в форме §3.1 нормативной модели
  (`repository_id`, `work_kind`, `change_class` при `work_kind: change`,
  `candidate_paths`, `changed_paths`, опционально `declared_profiles`).
- `--applicability` — весь envelope реестра применимости PHASE B
  (`{ "$schema", "schema_version", "records" }`). До извлечения записей документ
  целиком проверяется тем же JSON-Schema-движком против
  [`registries/rule-resolution/applicability.schema.json`](registries/rule-resolution/applicability.schema.json);
  пустой `{}`, отсутствие `records`, неверный `schema_version`, лишнее поле,
  `records` не-массивом или голый массив верхнего уровня — fail-closed с
  кодом возврата 2. Отсутствующий `records` не превращается в `[]`.
- `--protocol-routes` / `--verification-routes` / `--prior-state` — ещё не
  стандартизованные источники; используются, только если переданы, без
  скрытого пути по умолчанию.
- Instance читается **только** через `MERIDIAN_INSTANCE`: оттуда берутся
  `inventory/repositories.yaml` и те `instruction-intake/*.yaml`, на которые
  ссылаются записи применимости. Вывод — JSON в stdout. Отсутствие
  обязательного источника — fail-closed: ясная диагностика в stderr и
  ненулевой код возврата. Скрипт ничего не изменяет.

Валидатор устроен по правилу «проверка, которую нельзя выполнить, сообщает
UNVERIFIED, а не OK». Зелёный прогон означает «проверено», а не «не смотрели».
Запуск без `MERIDIAN_INSTANCE` этого правила не нарушает: он явно объявляет, что
product-специфичные литералы не проверялись.

## 5. Что проверяет CI и чего не проверяет

CI гоняет валидатор против `test/instance-fixture` — вымышленного продукта.
Это доказывает, что Kernel работает против *какого-то* Instance и что личных
путей в нём нет. Это **не** доказывает отсутствия литералов реального продукта:
такая проверка требует приватного Instance, которого у CI нет, и выполняется
локально перед коммитом. Ограничение напечатано в самом прогоне, а не спрятано.

## 6. Версионирование

Kernel — собственная релизная единица со своей линией SemVer, независимой от
продуктовых репозиториев и от Instance. Первая версия не назначается задним
числом: `0.1.0` — первый воспроизводимый снимок под Git, проходящий валидатор
в границах того, что валидатор на тот момент проверял; сам объём его проверок
расширяется и зафиксирован в `CHANGELOG.md`, а не подразумевается полным.
Переход к `1.0.0` требует явного решения о приёмке, а не истечения времени.

Поток веток и правило повышения версии описаны двумя стандартами:
[`standards/workspace/version-control-flow.md`](standards/workspace/version-control-flow.md)
— стабильная линия (для Kernel это `master`) и интеграционная `develop`, ветки
`feature`/`release`/`hotfix`, запрет прямых package-коммитов в стабильную линию,
перспективное применение; и
[`standards/workspace/release-versioning.md`](standards/workspace/release-versioning.md)
— источник версии `VERSION`, changelog по Keep a Changelog, повышение версии
только в `release/<semver>` с аннотированным тегом `vX.Y.Z`, hotfix только для
изменения PATCH-класса. Стандарты задают режим продвижения репозитория:
`semver-release` (Kernel — repository-level `VERSION` и тег `vX.Y.Z`) и
`revision-promotion` (репозиторий без решения о SemVer, например Instance —
верхнеуровневое состояние идентифицируется Git SHA advancement commit,
repository-level `VERSION`/тег не создаются). Вложенные независимо
версионируемые единицы (`stack-profiles/`, smoke-пакет) ведут свои номера
независимо от режима репозитория. Kernel в `0.x`: `0.1.0` и последующие
остаются draft-релизами в смысле `COMPATIBILITY.md`.

Совместимость Kernel и Instance объявляется в
[`COMPATIBILITY.md`](COMPATIBILITY.md), а не выводится из близости номеров.

## 7. Открытые решения

| # | Решение | Что блокирует |
|---|---|---|
| D-1 | Права на методологию, разработанную в период найма (юридическая проверка) | публикацию, коммерческую передачу |
| D-2 | Выбор лицензии | публикацию |
| D-3 | Остаётся ли привязка к конкретному wiki/агентному инструменту first-class или нужен слой абстракции | объём сценариев 2/3 |
| D-4 | Нужен ли bootstrap CLI/installer | коммерческую передачу |
| D-5 | Критерии перехода `0.x` → `1.0.0` | первый стабильный релиз |
| D-6 | ~~Переносимый шаблон smoke-пакета в Kernel~~ — закрыто: `verification/smoke-protocol/` (протокол + шаблон acceptance gate); приёмка на реальном втором продукте остаётся будущим фактом | — |

Пока D-1 не решён, репозиторий остаётся приватным.
