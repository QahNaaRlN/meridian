---
title: Статусная модель инженерных документов
document_type: standard
status: maintained
scope: workspace
owner: не определён
created: 2026-07-27
updated: 2026-08-18
related_documents:
  - ../README.md
  - ./agent-workspace.md
- ./document-lifecycle.md
- ./document-quality.md
- ../../../engineering-workspace/skills/versioning-standard-docs/SKILL.md
---

# Статусная модель инженерных документов

## Назначение

Настоящий стандарт определяет допустимые статусы инженерных документов,
разрешённые переходы между ними, обязательные метаданные, правила физического
размещения и порядок публикации в Confluence.

Стандарт применяется к следующим типам документов:

- `analysis`;
- `technical-specification`;
- `plan`;
- `report`;
- `problem`;
- `rfc`;
- `adr`;
- `concept`;
- `readme`;
- `standard`.

Статус является частью содержательной модели документа, а не декоративной
меткой.

### Граница с Confluence Versioning Standard

Этот файл остаётся источником истины для lifecycle локальных Markdown-файлов,
их Front Matter и физического размещения в `docs/`, `.agent/` и
`publication-drafts/`.

Для документа, создаваемого или редактируемого непосредственно в Confluence,
авторитетен принятый workspace skill
`C:\Work\engineering-workspace\skills\versioning-standard-docs\SKILL.md`:

```text
Draft → Proposed → Accepted → Deprecated / Superseded / Archived
```

Он же определяет, нужна ли Confluence-странице собственная версия, правила
SemVer, V12-интерпретацию, историю изменений и обязательную остановку перед
сменой `Status` или версии.

Локальный Front Matter status и Confluence Status — разные поля разных
контекстов. Между ними нет автоматического преобразования: при публикации
Confluence Status выбирается явно по принятому skill. При конфликте по
Confluence status/version приоритет имеет принятый skill; этот файл продолжает
управлять локальным lifecycle и размещением.

В видимой таблице метаданных Confluence название и контролируемое значение
статуса записываются двуязычно: `Статус (Status)` и, например,
`Черновик (draft)`, `На ревью (in-review)`, `Принят (accepted)`,
`Заменено (superseded)`. Это правило отображения не изменяет английские
машинные значения YAML, labels, валидаторов и интеграций. Полный контракт — в
[`templates/CONFLUENCE.md`](../templates/CONFLUENCE.md).

## 1. Основные принципы

### 1.1. Front Matter — единственный источник истины

Точный статус хранится в YAML Front Matter:

```yaml
---
document_type: plan
status: active
---
```

Каталог отражает только крупную фазу хранения. Например, `plans/active/` может
содержать документы со статусами `draft`, `in-review`, `approved`, `active` и
`blocked`.

### 1.2. Статусы машиночитаемы

Используй только значения из этого стандарта:

```yaml
status: in-review
```

Не используй произвольные варианты:

```yaml
status: На ревью
status: reviewing
status: почти готово
status: done
```

Значения статусов пишутся на английском языке, в нижнем регистре и
`kebab-case`. Основной текст документа пишется на русском языке.

### 1.3. Статус зависит от типа документа

Не существует одного универсального жизненного цикла. RFC проходит обсуждение и
публикацию, ADR фиксирует архитектурное решение, Plan описывает выполнение,
Report фиксирует результат, README сопровождается постоянно.

Это правило относится к локальному lifecycle. Для Confluence используется
единая цепочка из принятого `versioning-standard-docs` skill.

### 1.4. Статус меняется только по факту события

Основанием для перехода являются реальные события:

- документ передан на ревью;
- решение принято или отклонено;
- реализация началась;
- работа заблокирована;
- проверка завершена;
- документ опубликован;
- документ заменён другим;
- работа прекращена.

### 1.5. Терминальные документы не переписываются задним числом

К терминальным или историческим состояниям относятся:

- `rejected`;
- `withdrawn`;
- `cancelled`;
- `completed`;
- `fulfilled`;
- `final`;
- `published`;
- `referred`;
- `tolerated`;
- `superseded`;
- `deprecated`;
- `invalidated`;
- `archived`.

После перехода в такое состояние допускаются только исправления опечаток,
ссылок и метаданных. Нельзя переписывать принятое или отклонённое решение так,
будто предыдущего состояния не существовало.

## 2. Базовые метаданные

```yaml
---
title: <название>
document_type: <тип документа>
status: <допустимый статус>
scope: <workspace | repository>
repository: <обязательно для scope: repository>
owner: <ответственный>
created: YYYY-MM-DD
updated: YYYY-MM-DD
related_tickets: []
related_documents: []
canonical_url:
supersedes:
superseded_by:
---
```

| Поле                | Требование                                                        |
| ------------------- | ----------------------------------------------------------------- |
| `title`             | обязательно                                                       |
| `document_type`     | обязательно                                                       |
| `status`            | обязательно                                                       |
| `scope`             | обязательно                                                       |
| `repository`        | обязательно при `scope: repository`                               |
| `owner`             | обязательно                                                       |
| `created`           | обязательно                                                       |
| `updated`           | обязательно                                                       |
| `related_tickets`   | рекомендуется                                                     |
| `related_documents` | рекомендуется                                                     |
| `canonical_url`     | обязательно для `published`; для `problem` — начиная с `recorded` |
| `supersedes`        | при замене предыдущего документа                                  |
| `superseded_by`     | обязательно для `superseded`                                      |

## 3. Статусная модель Analysis

`document_type: analysis`

| Статус         | Значение                            |
| -------------- | ----------------------------------- |
| `draft`        | Формируется область исследования    |
| `active`       | Исследование выполняется            |
| `concluded`    | Получен достаточный вывод           |
| `inconclusive` | Данных недостаточно                 |
| `invalidated`  | Выводы потеряли актуальность        |
| `cancelled`    | Исследование прекращено             |
| `archived`     | Документ выведен из активной работы |

Разрешённые переходы:

```text
draft → active
draft → cancelled
active → concluded
active → inconclusive
active → cancelled
inconclusive → active
concluded → invalidated
concluded → archived
inconclusive → archived
invalidated → archived
cancelled → archived
```

Дополнительные поля:

```yaml
# Для inconclusive
inconclusive_reason:

# Для invalidated
invalidated_at: YYYY-MM-DD
invalidated_reason:
```

Размещение:

```text
analysis/active/   ← draft, active
analysis/archive/  ← concluded, inconclusive, invalidated, cancelled, archived
```

`concluded` допустимо временно оставить в `active/`, пока его выводы напрямую
используются текущей задачей.

## 4. Статусная модель Technical Specification

`document_type: technical-specification`

| Статус           | Значение                           |
| ---------------- | ---------------------------------- |
| `draft`          | Требования формируются             |
| `in-review`      | Требования согласовываются         |
| `approved`       | Требования приняты                 |
| `implementation` | По ТЗ ведётся реализация           |
| `fulfilled`      | Требования реализованы и проверены |
| `rejected`       | ТЗ отклонено                       |
| `cancelled`      | Работа прекращена                  |
| `superseded`     | ТЗ заменено другим                 |
| `archived`       | Документ сохранён для истории      |

```text
draft → in-review
draft → cancelled
in-review → draft
in-review → approved
in-review → rejected
in-review → cancelled
approved → implementation
approved → superseded
approved → cancelled
implementation → fulfilled
implementation → superseded
implementation → cancelled
fulfilled → superseded
fulfilled → archived
rejected → archived
cancelled → archived
superseded → archived
```

Обязательные дополнительные поля:

```yaml
# approved
approved_at: YYYY-MM-DD
approved_by: []

# implementation
implementation_started_at: YYYY-MM-DD
implementation_plan:

# fulfilled
fulfilled_at: YYYY-MM-DD
verification_status: passed
verification_report:

# superseded
superseded_by:
```

Размещение:

```text
technical-specifications/active/  ← draft, in-review, approved, implementation
technical-specifications/done/    ← fulfilled, rejected, cancelled, superseded, archived
```

## 5. Статусная модель Implementation Plan

`document_type: plan`

| Статус       | Значение                                     |
| ------------ | -------------------------------------------- |
| `draft`      | План формируется                             |
| `in-review`  | План проверяется                             |
| `approved`   | План согласован                              |
| `active`     | План выполняется                             |
| `blocked`    | Выполнение временно остановлено              |
| `completed`  | Реализация и обязательные проверки завершены |
| `cancelled`  | Выполнение прекращено                        |
| `superseded` | План заменён другим                          |
| `archived`   | План сохранён для истории                    |

```text
draft → in-review
draft → cancelled
in-review → draft
in-review → approved
in-review → cancelled
approved → active
approved → superseded
approved → cancelled
active → blocked
active → completed
active → superseded
active → cancelled
blocked → active
blocked → superseded
blocked → cancelled
completed → archived
completed → superseded
cancelled → archived
superseded → archived
```

Дополнительные поля:

```yaml
# approved
approved_at: YYYY-MM-DD
approved_by: []

# active
started_at: YYYY-MM-DD

# blocked
blocked_since: YYYY-MM-DD
blocked_reason:
blocked_by:

# completed
completed_at: YYYY-MM-DD
implementation_status: completed
verification_status: passed
verification_report:
```

Нельзя устанавливать `status: completed`, пока обязательная проверка не
завершена. Если код написан, но smoke/e2e/ручная проверка ещё не выполнена:

```yaml
status: active
implementation_status: completed
verification_status: pending
```

Размещение:

```text
plans/active/  ← draft, in-review, approved, active, blocked
plans/done/    ← completed, cancelled, superseded, archived
```

## 6. Статусная модель Report

`document_type: report`

| Статус       | Значение                                   |
| ------------ | ------------------------------------------ |
| `draft`      | Отчёт формируется                          |
| `final`      | Итог зафиксирован                          |
| `superseded` | Отчёт заменён более актуальным             |
| `archived`   | Отчёт больше не отражает текущее состояние |

```text
draft → final
draft → archived
final → superseded
final → archived
superseded → archived
```

Для `final`:

```yaml
finalized_at: YYYY-MM-DD
verification_status: <passed | failed | partial | not-applicable>
```

Для `superseded`:

```yaml
superseded_by:
```

Размещение:

```text
reports/current/  ← draft, final
reports/archive/  ← superseded, archived
```

`current` является именем каталога, а не статусом.

## 7. Статусная модель Problem Record

`document_type: problem`

Problem Record фиксирует расхождение между зафиксированными требованиями,
реализацией и тестами. Запись не обязана предлагать решение: она отвечает на
вопрос «что сломано и почему это не чинится как обычный баг». Это
pre-decision-артефакт: он существует до решения и может завершиться, так и не
породив его. Основной семантический владелец (primary semantic owner) —
семантический владелец затронутого утверждения (`area_type` + `area`);
тип `problem` не является domain-only и может жить у Common / Domain /
System / Platform (при `unclassified` — только временно, до классификации).

### 7.1. Публикация до содержательного ревью

Запись публикуется в Confluence сразу после написания, без ревью и согласования.
Непубликованное предупреждение бесполезно — до появления страницы и баннеров
следующий читатель реализует устаревшее требование повторно.

Границы исключения, за которые оно не распространяется:

- исключение применяется **только** к `document_type: problem` и не создаёт
  прецедента для `rfc`, `adr`, `concept` и любых других типов;
- `recorded` означает факт фиксации и публикации **наблюдаемого расхождения**;
- `recorded` **не** означает согласия команды с причиной расхождения,
  с интерпретацией требований или со способом исправления;
- запись в `recorded` не должна содержать нормативно принятого решения; всё,
  что выглядит как решение, оформляется отдельным RFC или ADR;
- пока не достигнут `in-review`, страница обязана явно показывать, что
  расхождение зафиксировано, но содержательного ревью не проходило.

Статуса `published` у типа нет: публикация происходит на переходе
`draft → recorded`. Из этого следует правило для валидатора — canonical-инварианты
нельзя определять через `status: published`; для `problem` они привязаны к
`recorded` и всем последующим статусам.

### 7.2. Статусы

| Статус        | Значение                                                                              |
| ------------- | ------------------------------------------------------------------------------------- |
| `draft`       | Запись формируется, страница ещё не создана                                           |
| `recorded`    | Расхождение зафиксировано, страница опубликована, баннеры расставлены                 |
| `in-review`   | Расхождение вынесено на содержательное обсуждение                                     |
| `referred`    | Передана вовне: RFC, ADR, задача на исправление или владелец другого источника истины |
| `tolerated`   | Расхождение признано и сознательно оставлено без изменения                            |
| `invalidated` | Расхождения нет: исходная оценка ошибочна                                             |
| `superseded`  | Запись заменена другой записью                                                        |
| `archived`    | Запись выведена из активной работы                                                    |

```text
draft → recorded
draft → invalidated
recorded → in-review
recorded → referred
recorded → invalidated
recorded → superseded
in-review → recorded
in-review → referred
in-review → tolerated
in-review → invalidated
referred → archived
referred → superseded
tolerated → archived
tolerated → superseded
invalidated → archived
superseded → archived
```

### 7.3. Терминальные исходы

Статуса `resolved` не существует, и цепочка `problem → rfc → adr` не является
обязательной. Запись имеет три самостоятельных терминальных исхода.

| Исход              | Статус        | Когда                                                                                                                                                                                                                                        |
| ------------------ | ------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Передана вовне     | `referred`    | Появился артефакт, который ведёт работу дальше: RFC, ADR, задача на исправление (если решение очевидно и обсуждения не требует) либо запись у владельца другого источника истины — например, у продуктовой команды, если ошибочны требования |
| Оставлено как есть | `tolerated`   | Расхождение подтверждено, но менять поведение сознательно не будут; причина обязательна                                                                                                                                                      |
| Расхождения нет    | `invalidated` | Один из трёх слоёв прочитан неверно, расхождение мнимое                                                                                                                                                                                      |

`referred` не означает «создан RFC»: он означает «ответственность за дальнейшее
движение передана конкретному артефакту», и этот артефакт указывается в
`referred_to`. Исход «ошибочна реализация, чиним обычным порядком» — это
`referred` на задачу, а не `invalidated`: расхождение было реальным.

`tolerated` терминален. Возобновление работы по тому же расхождению требует новой
записи, связанной через `supersedes` и `superseded_by`, а не возврата статуса.

### 7.4. Дополнительные поля

```yaml
# всегда
discovered_at: YYYY-MM-DD
discovered_in: # ссылка на задачу, в ходе которой обнаружено
area_type: # common | domain | system | platform | unclassified — primary owner
area: # slug области; конкретные слаги продукта — в naming_conventions Instance
# domain:                 # устаревшее; не требовать. При миграции заменять на area_type/area
affected_repositories: []

# recorded — canonical-метаданные, обязательны начиная с этого статуса
recorded_at: YYYY-MM-DD
canonical_url:
affected_documents: [] # страницы, на которые повешены баннеры
blocked_task_decision: # blocked | partial-fix | closed-as-mismatch

# in-review
review_scheduled_at: YYYY-MM-DD

# referred
referred_at: YYYY-MM-DD
referred_to: # RFC, ADR, задача или внешний владелец
referred_kind: # rfc | adr | task | external-owner

# tolerated
tolerated_at: YYYY-MM-DD
tolerated_reason:

# invalidated
invalidated_at: YYYY-MM-DD
invalidated_reason:

# superseded
superseded_by:
```

Поле `published_at` для этого типа не используется: фактическую дату публикации
несёт `recorded_at`.

### 7.5. Соответствие content status страницы Confluence

| Content status        | Статус        |
| --------------------- | ------------- |
| `Зафиксирована`       | `recorded`    |
| `На обсуждении`       | `in-review`   |
| `Передана в решение`  | `referred`    |
| `Принято жить с этим` | `tolerated`   |
| `Расхождения нет`     | `invalidated` |

Начиная с `recorded` тело локальной копии не редактируется: обновляются только
статус, метаданные и ссылки. Содержательные изменения вносятся в каноническую
страницу.

### 7.6. Размещение

```text
publication-drafts/problems/active/   ← draft, recorded, in-review
publication-drafts/problems/archive/  ← referred, tolerated, invalidated, superseded, archived
```

Для этого типа каноничность и активность — независимые оси. Опубликованная
запись остаётся canonical в Confluence и одновременно активным рабочим
артефактом, пока расхождение не закрыто. Общее правило «опубликован → в архив»
(§14) к `problem` по аналогии не применяется: перемещение вызывает не публикация,
а достижение терминального исхода.

### 7.7. Чек-лист закрытия

Перед переводом в `referred`, `tolerated` или `invalidated`:

- [ ] Указан исход: `referred_to` и `referred_kind`, `tolerated_reason` или
      `invalidated_reason`.
- [ ] Сняты баннеры со всех страниц из `affected_documents`.
- [ ] Characterization-тест со ссылкой на запись заменён нормативным тестом,
      перенесён на порождённый артефакт или удалён.
- [ ] Исходная задача не осталась в неопределённом состоянии.

## 8. Статусная модель RFC

`document_type: rfc`

RFC является публикационным документом. До переноса в Confluence локальная
версия считается рабочим черновиком.

| Статус       | Значение                                      |
| ------------ | --------------------------------------------- |
| `draft`      | Предложение формируется                       |
| `in-review`  | RFC обсуждается                               |
| `accepted`   | Предложение принято                           |
| `rejected`   | Предложение отклонено                         |
| `withdrawn`  | Автор снял предложение                        |
| `published`  | Каноническая версия опубликована в Confluence |
| `superseded` | RFC заменён новым                             |
| `archived`   | Локальная копия сохранена для истории         |

```text
draft → in-review
draft → withdrawn
in-review → draft
in-review → accepted
in-review → rejected
in-review → withdrawn
accepted → published
accepted → superseded
accepted → withdrawn
published → superseded
published → archived
rejected → archived
withdrawn → archived
superseded → archived
```

Дополнительные поля:

```yaml
# in-review
reviewers: []
review_started_at: YYYY-MM-DD

# accepted
accepted_at: YYYY-MM-DD
accepted_by: []

# published
published_at: YYYY-MM-DD
canonical_url:

# rejected
rejected_at: YYYY-MM-DD
rejection_reason:

# withdrawn
withdrawn_at: YYYY-MM-DD
withdrawal_reason:

# superseded
superseded_by:
```

После публикации в начало тела добавляется:

```markdown
> [!IMPORTANT]
> Документ опубликован в Confluence.
> Каноническая версия: <ссылка>.
> Локальная копия сохранена как снимок на момент публикации и не должна
> обновляться независимо от канонической версии.
```

Размещение:

```text
publication-drafts/rfc/active/   ← draft, in-review, accepted
publication-drafts/rfc/archive/  ← published, rejected, withdrawn, superseded, archived
```

## 9. Статусная модель ADR

`document_type: adr`

| Статус       | Значение                                      |
| ------------ | --------------------------------------------- |
| `proposed`   | Архитектурное решение предложено              |
| `in-review`  | Решение рассматривается                       |
| `accepted`   | Решение принято                               |
| `rejected`   | Решение отклонено                             |
| `withdrawn`  | Предложение снято                             |
| `deprecated` | Решение больше не рекомендуется               |
| `published`  | Каноническая версия опубликована в Confluence |
| `superseded` | Решение заменено другим ADR                   |
| `archived`   | Документ сохранён для истории                 |

```text
proposed → in-review
proposed → withdrawn
in-review → proposed
in-review → accepted
in-review → rejected
in-review → withdrawn
accepted → published
accepted → deprecated
accepted → superseded
published → deprecated
published → superseded
published → archived
deprecated → superseded
deprecated → archived
rejected → archived
withdrawn → archived
superseded → archived
```

Дополнительные поля:

```yaml
# accepted
accepted_at: YYYY-MM-DD
decision_owners: []

# published
published_at: YYYY-MM-DD
canonical_url:

# deprecated
deprecated_at: YYYY-MM-DD
deprecation_reason:

# superseded
superseded_by:
```

После `accepted` разделы «Контекст», «Решение» и «Альтернативы» не
переписываются задним числом. При изменении решения создаётся новый ADR, а
старый получает `status: superseded`.

Размещение:

```text
publication-drafts/adr/active/   ← proposed, in-review, accepted
publication-drafts/adr/archive/  ← published, rejected, withdrawn, deprecated, superseded, archived
```

## 10. Статусная модель Concept

`document_type: concept`

Concept — публикационный черновик долгосрочного описания домена, архитектуры,
интеграций или границ системы.

| Статус       | Значение                                      |
| ------------ | --------------------------------------------- |
| `draft`      | Концепция формируется                         |
| `in-review`  | Концепция проверяется                         |
| `approved`   | Содержание согласовано                        |
| `published`  | Каноническая версия опубликована в Confluence |
| `rejected`   | Концепция отклонена                           |
| `superseded` | Концепция заменена новой                      |
| `archived`   | Документ сохранён для истории                 |

```text
draft → in-review
draft → archived
in-review → draft
in-review → approved
in-review → rejected
approved → published
approved → superseded
published → superseded
published → archived
rejected → archived
superseded → archived
```

Для `published`:

```yaml
published_at: YYYY-MM-DD
canonical_url:
```

Для `superseded`:

```yaml
superseded_by:
```

Размещение:

```text
publication-drafts/concepts/active/   ← draft, in-review, approved
publication-drafts/concepts/archive/  ← published, rejected, superseded, archived
```

## 11. Статусная модель README

`document_type: readme`

README — живой сопровождаемый документ.

| Статус       | Значение                                      |
| ------------ | --------------------------------------------- |
| `maintained` | Документ актуален                             |
| `stale`      | Часть информации устарела                     |
| `deprecated` | Проект или workflow выводится из эксплуатации |
| `archived`   | Проект или раздел больше не используется      |

```text
maintained → stale
maintained → deprecated
stale → maintained
stale → deprecated
stale → archived
deprecated → archived
```

Front Matter для README необязателен. Используй видимый блок:

```markdown
> **Статус документа (Document status):** Поддерживается (maintained)
> **Последняя проверка (Last reviewed):** YYYY-MM-DD
> **Владелец (Owner):** <команда или роль>
```

Соответствие:

| Видимое значение            | Статус       |
| --------------------------- | ------------ |
| `Поддерживается (maintained)`            | `maintained` |
| `Требует актуализации (stale)`           | `stale`      |
| `Выводится из эксплуатации (deprecated)` | `deprecated` |
| `Архивный (archived)`                    | `archived`   |

## 12. Статусная модель Standard

`document_type: standard`

| Статус       | Значение                            |
| ------------ | ----------------------------------- |
| `draft`      | Стандарт разрабатывается            |
| `in-review`  | Стандарт проверяется                |
| `maintained` | Стандарт принят и поддерживается    |
| `deprecated` | Стандарт выводится из использования |
| `superseded` | Стандарт заменён другим             |
| `archived`   | Стандарт сохранён для истории       |

```text
draft → in-review
in-review → draft
in-review → maintained
maintained → deprecated
maintained → superseded
deprecated → superseded
deprecated → archived
superseded → archived
```

Для `superseded` обязательно:

```yaml
superseded_by:
```

## 13. Физическое перемещение документов

Не перемещай файл при каждом изменении статуса.

Перемещение не требуется:

```text
draft → in-review
approved → active
active → blocked
blocked → active
```

Для `document_type: problem` публикация перемещения не вызывает: запись
переезжает в `archive/` при достижении терминального исхода, а не при появлении
канонической страницы (§7.6).

Перемещение требуется при смене крупной фазы хранения:

```text
active → completed
accepted → published
final → archived
accepted → superseded
```

Порядок:

1. обновить `status`;
2. обновить `updated`;
3. заполнить обязательные поля нового статуса;
4. проверить ссылки;
5. переместить файл.

## 14. Публикация в Confluence

Confluence является каноническим источником для:

- Problem Records;
- RFC;
- ADR;
- Concepts;
- долгосрочной архитектурной документации;
- продуктовой и доменной документации.

До публикации документы хранятся в `.agent/publication-drafts/`.

Перед публикацией документ должен:

- пройти ревью;
- иметь согласованный статус;
- не содержать служебных подсказок шаблона;
- пройти проверку по `document-quality.md`;
- содержать актуальные ссылки.

После публикации обязательно:

```yaml
status: published
published_at: YYYY-MM-DD
canonical_url:
```

Локальная копия переносится в `archive/`, становится снимком и не редактируется
независимо от Confluence.

### Исключение для Problem Record

Problem Record публикуется до ревью и согласования, сразу после написания.
Требования «пройти ревью» и «иметь согласованный статус» к нему не применяются;
остальные требования (отсутствие служебных подсказок шаблона, проверка качества,
актуальные ссылки) применяются полностью.

Исключение узкое и не расширяется по аналогии: оно действует только для
`document_type: problem`, только потому что предметом публикации является факт
наблюдаемого расхождения, а не предложение или решение. Границы исключения и
запрет на нормативные решения внутри записи — в §7.1. Правило «опубликован → в
архив» к этому типу также не применяется (§7.6).

Одновременно с публикацией обязательны два действия, без которых запись не
считается зафиксированной:

1. На каждую страницу, описывающую поведение, объявленное неверным, повешен
   баннер со ссылкой на запись; список этих страниц указан в
   `affected_documents`.
2. Судьба исходной задачи решена явно и записана в `blocked_task_decision`.

## 15. Замена документа

`superseded` означает, что документ заменён другим.

Старый документ:

```yaml
status: superseded
superseded_by:
```

Новый документ:

```yaml
supersedes:
```

Запрещено использовать `superseded` без указания замены.

## 16. Различие терминальных статусов

| Статус        | Использование                               |
| ------------- | ------------------------------------------- |
| `rejected`    | Документ рассмотрели и отклонили            |
| `withdrawn`   | Автор снял предложение                      |
| `cancelled`   | Работу прекратили                           |
| `completed`   | План выполнен и проверен                    |
| `fulfilled`   | Требования ТЗ выполнены                     |
| `final`       | Отчёт завершён                              |
| `published`   | Документ перенесён в Confluence             |
| `referred`    | Проблема передана в решение                 |
| `tolerated`   | С расхождением решено жить                  |
| `superseded`  | Документ заменён другим                     |
| `deprecated`  | Решение больше не рекомендуется             |
| `invalidated` | Выводы анализа потеряли корректность        |
| `archived`    | Документ выведен из активного использования |

Не используй `archived` вместо содержательного статуса. Сначала укажи причину:

```text
accepted → superseded → archived
active → cancelled → archived
final → archived
```

## 17. Запрещённые переходы

Запрещены переходы, скрывающие историю:

```text
rejected → accepted
cancelled → active
superseded → active
published → draft
archived → active
referred → recorded
tolerated → recorded
```

При возобновлении работы создаётся новый документ или новая версия с новой
идентичностью. Старый связывается через `supersedes` и `superseded_by`.

## 18. Автоматическая проверка

Рекомендуется создать:

```text
scripts/validate-agent-documents.mjs
```

Минимальные проверки:

1. `document_type` допустим.
2. `status` допустим для типа.
3. Обязательные поля заполнены.
4. Для `published` есть `canonical_url` и `published_at`.
5. Для `superseded` есть `superseded_by`.
6. Для `blocked` есть `blocked_reason` и `blocked_since`.
7. Для `completed` `verification_status` равен `passed`.
8. Для `fulfilled` указан отчёт проверки.
9. Расположение соответствует крупной фазе.
10. Архивная копия опубликованного документа содержит предупреждение о Confluence.
11. Для `problem` со статусом `recorded` и далее есть `canonical_url`,
    `recorded_at` и непустой `affected_documents`.
12. Для `referred` есть `referred_to` и `referred_kind`.
13. Для `tolerated` есть `tolerated_reason`.
14. Для `problem` не используются статус `published` и поле `published_at`.
15. Canonical-инварианты проверяются по признаку «тип публикационный и статус
    достиг публикации», а не по `status == published`.

## 19. Сводная матрица

| Тип                     | Активные статусы                                      | Терминальные и исторические                                                  |
| ----------------------- | ----------------------------------------------------- | ---------------------------------------------------------------------------- |
| Analysis                | `draft`, `active`                                     | `concluded`, `inconclusive`, `invalidated`, `cancelled`, `archived`          |
| Technical Specification | `draft`, `in-review`, `approved`, `implementation`    | `fulfilled`, `rejected`, `cancelled`, `superseded`, `archived`               |
| Plan                    | `draft`, `in-review`, `approved`, `active`, `blocked` | `completed`, `cancelled`, `superseded`, `archived`                           |
| Report                  | `draft`, `final`                                      | `superseded`, `archived`                                                     |
| Problem Record          | `draft`, `recorded`, `in-review`                      | `referred`, `tolerated`, `invalidated`, `superseded`, `archived`             |
| RFC                     | `draft`, `in-review`, `accepted`                      | `rejected`, `withdrawn`, `published`, `superseded`, `archived`               |
| ADR                     | `proposed`, `in-review`, `accepted`                   | `rejected`, `withdrawn`, `deprecated`, `published`, `superseded`, `archived` |
| Concept                 | `draft`, `in-review`, `approved`                      | `published`, `rejected`, `superseded`, `archived`                            |
| README                  | `maintained`, `stale`                                 | `deprecated`, `archived`                                                     |
| Standard                | `draft`, `in-review`, `maintained`                    | `deprecated`, `superseded`, `archived`                                       |

## 20. Чек-лист смены статуса

- [ ] Новый статус допустим для `document_type`.
- [ ] Переход разрешён моделью.
- [ ] Соответствующее событие реально произошло.
- [ ] Заполнены обязательные поля нового статуса.
- [ ] Обновлено поле `updated`.
- [ ] Ссылки актуальны.
- [ ] Для `superseded` указана замена.
- [ ] Для `published` указана каноническая ссылка.
- [ ] Терминальный документ больше не выдаётся за активный.
- [ ] При необходимости файл перемещён в `done/` или `archive/`.
- [ ] Изменение не скрывает историческое решение.

## 21. Критерии внедрения

- [ ] Все writer-стандарты ссылаются на этот документ.
- [ ] Все шаблоны используют допустимые статусы.
- [ ] `.agent/README.md` объясняет активные и архивные каталоги.
- [ ] Problem Records, RFC, ADR и Concepts хранятся в `publication-drafts`.
- [ ] Опубликованные документы содержат `canonical_url`.
- [ ] Произвольные старые статусы нормализованы.
- [ ] В `AGENTS.md` добавлена ссылка на стандарт.
- [ ] Определён владелец статусной модели.
- [ ] Настроена или запланирована автоматическая проверка.

---

_Перед добавлением нового статуса оцени влияние на шаблоны, writer-стандарты,
существующие документы и валидатор. Не добавляй отдельный статус ради единичного
исключения, если существующая модель уже выражает нужное состояние._
