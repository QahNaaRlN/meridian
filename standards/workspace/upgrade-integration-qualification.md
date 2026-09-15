---
title: Объединённая квалификация интеграции обновления операционной модели
document_type: standard
status: draft
scope: workspace
owner: workspace-owner
created: 2026-09-15
updated: 2026-09-15
topic: unclassified
unclassified_reason: >-
  пул тем выведен из корпуса продуктовых норм и предмета «объединённая
  квалификация интеграции обновления операционной модели» не содержит; тема
  присваивается при ревью границ пула
profile: universal
delivery: kernel-doc
activation: task-class
related_documents:
  - ./operating-foundation.yaml
  - ./task-pattern-registry.md
  - ./task-specification.md
  - ./execution-state-model.md
  - ./role-and-human-control.md
  - ./bounded-context-manifest.md
  - ./evidence-and-handoff-contract.md
  - ./meridian-field-evaluation.md
  - ./upgrade-integration-qualification-operator-guide.md
  - ./operating-glossary.md
  - ./operating-principles.md
  - ../../registries/operating-model/upgrade-integration-qualification.schema.json
  - ../../registries/operating-model/scoped-record.schema.json
---

# Объединённая квалификация интеграции обновления операционной модели

Идентификатор контракта: `upgrade-integration-qualification`.

Контракт вычисляет один закрытый итоговый вердикт о том, что пакеты 1–8
программы `meridian-operating-upgrade` — `meridian-operating-foundation`
([`operating-foundation.yaml`](operating-foundation.yaml),
[`operating-glossary.md`](operating-glossary.md),
[`operating-principles.md`](operating-principles.md)),
[`task-pattern-registry`](task-pattern-registry.md),
[`task-specification-contract`](task-specification.md),
[`execution-state-model`](execution-state-model.md),
[`role-and-human-control`](role-and-human-control.md),
[`bounded-context-manifest`](bounded-context-manifest.md),
[`evidence-and-handoff-contract`](evidence-and-handoff-contract.md) и
[`meridian-field-evaluation`](meridian-field-evaluation.md) — работают вместе
на одном задачном пути одного рабочего пространства, **композируя** уже
существующие проверки этих восьми пакетов **без дублирования** ни одной из
них. Это девятый, предпоследний пакет программы (перед выпуском
`meridian-operating-upgrade-release`, пакет 10): он не классифицирует задачу,
не ведёт запуск, не назначает роль, не собирает контекст, не записывает
доказательство и не наблюдает показатель сам — он только **квалифицирует**
уже составленные записи восьми пакетов, на которые ссылается, точно тем же
приёмом, что [`workspace-compatibility-qualification.md`](workspace-compatibility-qualification.md)
уже применяет к пяти пакетам программы `meridian-workspace-compatibility`.

Схема — [`upgrade-integration-qualification.schema.json`](../../registries/operating-model/upgrade-integration-qualification.schema.json).
Определения и нормативный смысл — здесь. Композитная проверка живёт в одной
функции
[`scripts/lib/upgrade-integration-qualification.mjs`](../../scripts/lib/upgrade-integration-qualification.mjs),
которую вызывают и `scripts/kernel-validate.mjs` (участок
`upgrade-integration-qualification`), и самостоятельный набор
`test/upgrade-integration-qualification.test.mjs`.

Контракт — **обязательная часть текущего Ядра**. Отсутствие схемы или
продуктово-нейтральных фикстур рядом с ней — ошибка проверки Ядра (`FAIL`), а
не информационный пропуск.

## 1. Один терминальный composed-путь для пакетов 2–7 (свойство 1)

Разрешённая запись `evidence-and-handoff` (`evidence-and-handoff-contract.md`
§4, §12) уже композирует `execution-state-model`, `role-and-human-control`,
`bounded-context-manifest` и, транзитивно через собственный
`task_specification_ref`, `task-specification-contract` и
`task-pattern-registry`. Эта запись — единственная терминальная точка
композиции для пакетов 2–7: `payload.task_journey_ref` ссылается ровно на
**одну** такую запись, разрешаемую через внешнюю границу и проверяемую **той
же** реальной `evaluateEvidenceAndHandoff` против **той же**
`evidence-and-handoff.schema.json` — этот пакет не разрешает ни один из
четырёх пакетов, которые она уже композирует, повторно и напрямую.

`payload.task_journey_ref` обязателен всегда: квалификация без хотя бы одного
разрешимого задачного пути не существует.

## 2. Отчёт полевой оценки — отдельная терминальная ссылка (свойство 2)

[`meridian-field-evaluation.md`](meridian-field-evaluation.md) §3 требует,
чтобы восемь характеристик отчёта показывались **раздельно, без свёртки в
один балл** — у отчёта `field-evaluation-report` нет оси вердикта, которую
эта запись могла бы прочитать как «пройдено/не пройдено». Поэтому
`payload.field_evaluation_report_ref` — **отдельная**, самостоятельная
ссылка, а не поле, слитое с вердиктом `task_journey_ref`: она допускает явный
`null` (отчёт ещё не составлен — легальный, но не финальный исход, §5) и,
когда присутствует, разрешается и проверяется **той же** реальной
`evaluateFieldEvaluation` против **той же** `field-evaluation.schema.json`.
Матрица решений (§5) читает только **факт присутствия и чистоты** этой
ссылки, никогда её содержательные показатели — то, что владелец получает
понятный отчёт о возможностях и известных пределах (программный выпускной
рубеж, §12 плана программы), это требование присутствия, а не требование
конкретного значения показателя.

## 3. Ровно три нейтральных сценария, ни один не исполнен (свойство 3)

`payload.scenario_classifications` закрыт **ровно тремя** элементами — по
одному на каждый обязательный `scenario_id` — без пропуска, без дубля и без
лишнего:

| `scenario_id` | Что доказывает | Ожидаемый `task_pattern.id` | Ожидаемый `lifecycle_stage` разрешённого `execution-run` |
|---|---|---|---|
| `single-module-refactor` | Маленький внутренний рефакторинг одного модуля переносится через полную модель без декомпозиции. | `refactor-preserving-behavior` ([`task-pattern-registry.md`](task-pattern-registry.md) §2) | без отдельного требования — это единственный из трёх сценариев, различаемый по шаблону, а не по этапу |
| `multi-repository-decomposition` | Крупная многорепозиторная инициатива классифицируется как требующая декомпозиции и **фактически продвигается** дальше классификации. | `decompose-initiative` (`decomposition_required: true`) | строго **позже** `classification` — декомпозиция не только названа, но и продвигается |
| `language-change-limit-case` | Предельная постановка о смене языка реализации классифицируется как инициатива, требующая декомпозиции, и **ни одна** единица продуктового переписывания не исполняется. | `decompose-initiative` (`decomposition_required: true`) | `intake` или `classification` — **не позже**; попытка продвинуться дальше отклоняется |

Каждый элемент несёт `task_specification_ref` и `execution_state_ref` —
закрытые закреплённые ссылки, разрешаемые через внешнюю границу и проверяемые
**той же** реальной `evaluateTaskSpecification` / `evaluateExecutionState`
против **тех же** схем этих двух пакетов, а затем — против собственного
закрытого ожидания сценария из таблицы выше (`checkScenarioExpectation`).

Два инициативных сценария намеренно разрешают **один и тот же**
`task_pattern.id` (`decompose-initiative`): смена языка реализации
продуктового кода — предельный случай инициативы, требующей декомпозиции, а
не отдельный шаблон каталога типов задач ([`task-pattern-registry.md`](task-pattern-registry.md)
§2.1 не вводит для неё второго шаблона). Различие между «декомпозиция
продвигается» и «только классификация, без исполнения» — это различие
**этапа жизненного цикла** разрешённого запуска
([`execution-state-model.md`](execution-state-model.md) §7), а не различие
шаблона: ровно поэтому `execution_state_ref` каждого сценария разрешается и
проверяется отдельно от `task_specification_ref`, и матрица §5 никогда не
объявляет `language-change-limit-case` пройденным, если разрешённый запуск
продвинулся дальше `classification`.

### 3.1. Точное закрепление постановки исполняемым запуском

Разрешённый `execution_state_ref` не проверяется только сам по себе — его
собственное поле `payload.task_specification_ref` (переносимая строка,
`execution-state-model.md` §4) обязано **буквально совпадать** со строкой
`task_specification_ref.reference` **этого же** элемента
`scenario_classifications`. Запуск, честно проходящий свою собственную
проверку (`evaluateExecutionState`), и постановка, честно проходящая свою
(`evaluateTaskSpecification`), всё равно отклоняются вместе, если запуск
**принадлежит другой постановке**: заимствованный, но валидный сам по себе
запуск не превращается в доказательство именно этого сценария. Проверено
самостоятельным негативным тестом — запуск сценария `single-module-refactor`
подставлен в сценарий `multi-repository-decomposition` при неизменном
`task_specification_ref` — и отклонён.

## 4. Продуктово-нейтральный шаблон переходной совместимости (свойство 4)

`payload.workspace_transition_compatibility` — обязательное, закрытое поле
`{ status, notes }`. В этом Kernel-срезе `status` закрыт единственным legal
значением `kernel-template-only`: запись фиксирует, что требование/шаблон
записи совместимости переходной модели Kernel/Instance с новой моделью
рабочих пространств **существует** в Ядре как продуктово-нейтральный шаблон
— она **никогда** не несёт заполненное доказательство перехода конкретного
продукта. Это отдельный, более поздний пакет Экземпляра: он применит уже
готовый Kernel-механизм к настоящей паре Kernel/Instance конкретного продукта
и предъявит собственное заполненное доказательство (по той же дисциплине,
что `workspace-compatibility-qualification.md` §4.3 уже применяет к
`migration-applicability-preservation`). Это поле **не входит** в матрицу
решений §5 — оно проверяется исключительно на закрытую форму схемой; будущий,
более широкий пул legal значений `status` (включая заполненное состояние
Экземпляра) — предмет отдельного изменения контракта (§8), а не этого среза.

## 5. Закрытая матрица решений

`payload.qualification_state` — **закрытый, пересчитываемый** вердикт:
`scripts/lib/upgrade-integration-qualification.mjs` вычисляет ожидаемое
состояние одной чистой функцией (`computeIntegrationQualificationState`) и
отклоняет запись, где заявленное значение расходится с вычисленным.
Восемь строк ниже — исчерпывающее покрытие этой функции:

| № | Условие | `qualification_state` |
|---|---|---|
| 1 | `task_journey_ref` не разрешился, или его композиция не чиста | `BLOCKED` |
| 2 | Разрешённый задачный путь несёт `outcome.status: blocked` | `BLOCKED` |
| 3 | Покрытие `scenario_classifications` не равно ровно трём различным обязательным `scenario_id` | `BLOCKED` |
| 4 | Любой сценарий не разрешился, не прошёл композицию, либо не соответствует собственному ожиданию (§3) | `BLOCKED` |
| 5 | Разрешённый задачный путь несёт `outcome.status: handed_off_incomplete` | `UNVERIFIED` |
| 6 | Заявленный `field_evaluation_report_ref` не разрешился или не прошёл композицию чисто | `BLOCKED` |
| 7 | `field_evaluation_report_ref` явно равен `null` (отчёт ещё не составлен) | `UNVERIFIED` |
| 8 | Иначе — задачный путь завершён, все три сценария классифицированы корректно, отчёт полевой оценки составлен и прошёл композицию | `QUALIFIED` |

Приоритет закрыт и вычисляется в этом порядке, независимо от порядка
объявления полей записи или элементов `scenario_classifications`.

**Три закрытые оси результата, не одна (та же дисциплина, что
`workspace-compatibility-qualification.md` §4.1 уже применяет).**
`qualification_state`, `blockers` и `open_questions` образуют одно закрытое
отношение: `blockers` непуст **ровно** при `BLOCKED`; `open_questions` непуст
**ровно** при `UNVERIFIED`.

## 6. Композиция без копирования, с точным закреплением версии

Каждая из четырёх ссылок (`task_journey_ref`, `field_evaluation_report_ref` —
когда не `null` — и каждый `task_specification_ref`/`execution_state_ref`
элемента `scenario_classifications`) обязана нести `sha256` — точный
пересчитываемый дайджест содержимого разрешённой записи
(`computeContentDigest`, над полным содержимым `id`, `title`, `record_type`,
`scope`, `origin`, `authority`, `payload`, той же формы, что
`computeConnectionDigest` уже применяет в
[`workspace-compatibility-qualification.mjs`](../../scripts/lib/workspace-compatibility-qualification.mjs)).
Заявленный `sha256`, расходящийся с фактически пересчитанным, — отказ **даже
когда `id` и `reference` совпадают буквально**.

Каждая ссылка резолвится через внешнюю границу, которую эта запись не
контролирует, и composed-запись проверяется **той же** реальной функцией
пакета, который она называет — эта запись не вводит вторую, конкурирующую
форму ни для одной из четырёх.

Запись квалификации никогда не является актом объявления или решением
владельца: `origin.kind` закреплён значением `derived`, а `authority.kind` —
значением `delegated-run`. Область (`scope.type`) закрыта значением
`project-workspace` — квалификация относится к одному рабочему пространству
целиком, а не к отдельному репозиторию.

### 6.1. Единая идентичность рабочего пространства

Композиция без копирования (§6) закрепляет **версию** каждой ссылки, но не
её принадлежность одному рабочему пространству — это отдельное свойство.
Разрешённый задачный путь, разрешённый отчёт полевой оценки (когда не
`null`) и **каждая** разрешённая постановка/запуск любого из трёх сценариев
обязаны называть **то же самое** рабочее пространство, что несёт сама запись
квалификации (`scope.id`, поскольку `scope.type` закрыт значением
`project-workspace`, §6):

| Разрешённая запись | Поле, несущее рабочее пространство |
|---|---|
| `task_journey_ref` (`evidence-and-handoff`) | `scope.workspace_id` (область `run-state`) |
| `field_evaluation_report_ref` (`field-evaluation-report`) | `payload.workspace_id` |
| `scenario_classifications[].task_specification_ref` (`task-specification`) | `scope.id`, когда `scope.type: project-workspace`; иначе `scope.workspace_id` (постановка легально несёт любую из двух форм области, `task-specification.md` §2) |
| `scenario_classifications[].execution_state_ref` (`execution-run`) | `scope.workspace_id` (область `run-state`) |

Расхождение любой из них с рабочим пространством самой записи — отказ, даже
когда каждая ссылка по отдельности сходится по `sha256` и проходит
композицию против своего реального evaluator'а: задачный путь одного
рабочего пространства, отчёт другого и сценарии третьего не образуют одну
осмысленную квалификацию. Проверено самостоятельным негативным тестом.

## 7. Что этот контракт не делает

- не классифицирует задачу и не назначает шаблон типа задачи — это предмет
  `task-pattern-registry.md`/`task-specification.md`, переиспользуемых здесь
  как есть;
- не ведёт запуск и не переходит между этапами жизненного цикла — это
  предмет `execution-state-model.md`;
- не назначает роль и не переключает режим надзора — это предмет
  `role-and-human-control.md`;
- не собирает и не проверяет ограниченный контекст — это предмет
  `bounded-context-manifest.md`;
- не записывает доказательство и не формирует передачу результата — это
  предмет `evidence-and-handoff-contract.md`, переиспользуемый здесь как
  единственная терминальная точка композиции пакетов 2–7 (§1);
- не наблюдает показатель и не формирует отчёт полевой оценки — это предмет
  `meridian-field-evaluation.md`;
- не исполняет ни одну единицу продуктового переписывания ни для одного из
  трёх нейтральных сценариев (§3), в том числе для предельного случая смены
  языка реализации;
- не пишет заполненное доказательство перехода конкретного продукта на
  новую модель рабочих пространств — только продуктово-нейтральный
  Kernel-шаблон (§4); заполненное доказательство — предмет отдельного,
  более позднего пакета Экземпляра;
- не выбирает постоянное хранилище и не несёт продуктовых литералов,
  абсолютных машинных путей или зависимости от конкретного Экземпляра;
- не начинает выпуск обновлённого Meridian
  (`meridian-operating-upgrade-release`, пакет 10), Concord, SQLite, Rust или
  Git-операцию любого рода;
- не выполняет Git-операций и не изменяет `VERSION`.

## 8. Изменение контракта

Добавление, удаление или изменение смысла поля — изменение универсальной
нормы Ядра: оно требует записи в журнале изменений и учитывается при выборе
следующей версии. Расширение легального пула
`payload.workspace_transition_compatibility.status` за пределы
`kernel-template-only` (§4) — такое изменение, а не самостоятельная
корректировка библиотеки.
