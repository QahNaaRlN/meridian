---
title: Каталог типов задач
document_type: standard
status: draft
scope: workspace
owner: workspace-owner
created: 2026-09-08
updated: 2026-09-08
topic: task-classification
profile: universal
delivery: kernel-doc
activation: task-class
related_documents:
  - ./task-pattern-registry.yaml
  - ./operating-glossary.md
  - ./operating-principles.md
  - ./workspace-scope-model.md
  - ./rule-resolution.md
  - ../../workflows/task-lifecycle.md
  - ../../registries/operating-model/task-pattern-registry.schema.json
  - ../../registries/operating-model/scoped-record.schema.json
  - ../../verification/functional-parity/refactor-protocol.md
  - ../../verification/functional-parity/functional-parity-evidence-contract.md
---

# Каталог типов задач

Идентификатор реестра: `task-pattern-registry`.

Каталог вводит переносимый, машинно-проверяемый набор универсальных **шаблонов
типов задач** (`task-pattern` — универсальный контракт входов, инвариантов,
остановок и доказательств для повторяемого типа работы,
[`operating-glossary.md`](operating-glossary.md)). Шаблон описывает, *какого
рода* работа и что она обязана соблюдать, но не выбирает конкретную программу,
продукт, репозиторий или технологию. Продуктовые постановки задач, назначения
участников и состояния запусков в каталог не входят — они принадлежат
Экземпляру и модели состояния запуска, которые вводятся отдельными пакетами.

Каталог — **обязательная часть текущего Ядра**. Отсутствие
[`task-pattern-registry.yaml`](task-pattern-registry.yaml), его схемы или
фикстур — ошибка проверки Ядра, а не информационный пропуск. Обязательность не
переключается через `VERSION`.

Машинные данные — [`task-pattern-registry.yaml`](task-pattern-registry.yaml).
Определения и нормативный смысл — здесь. Проверки согласованности выполняет одна
общая функция [`scripts/lib/task-pattern-registry.mjs`](../../scripts/lib/task-pattern-registry.mjs),
которую вызывают и `scripts/kernel-validate.mjs` (участок `task-pattern-registry`),
и самостоятельный набор `test/task-pattern-registry.test.mjs` — две реализации
не расходятся, потому что реализация одна.

## 1. Что каталог не вводит заново

Каталог **не создаёт второй словарь**. Виды работы `work_kind`
(`assessment | operation | initiative | change`) и классы изменения
`change_class` (`BUGFIX | FEATURE | BEHAVIOR_CHANGE | REFACTOR`) — это
действующие закрытые пулы [`rule-resolution.md`](rule-resolution.md) §2–§3.
Каталог перечисляет их в схеме как закрытые перечисления только для машинной
проверки и не меняет их семантику. Проверка сверяет пул схемы с этими четырьмя
значениями каждого рода и падает при расхождении.

Каталог также не дублирует протоколы, контракты доказательств и универсальный
конверт записи. Он **ссылается** на существующие артефакты и композирует
проверки (§4).

## 2. Обязательная модель шаблона

Ожидается ровно **семь** шаблонов — по одному на каждую действующую
классификационную пару. Общего шаблона `change`, конкурирующего с четырьмя
классами изменения, нет.

| `id` | Название | `work_kind` | `change_class` |
|---|---|---|---|
| `assess-existing-state` | Оценка существующего состояния | `assessment` | — |
| `operate-environment-action` | Эксплуатационное действие в среде | `operation` | — |
| `decompose-initiative` | Декомпозиция инициативы | `initiative` | — |
| `fix-defect` | Исправление дефекта | `change` | `BUGFIX` |
| `add-capability` | Добавление новой возможности | `change` | `FEATURE` |
| `change-behavior` | Намеренное изменение поведения | `change` | `BEHAVIOR_CHANGE` |
| `refactor-preserving-behavior` | Рефакторинг с сохранением поведения | `change` | `REFACTOR` |

Каждый шаблон несёт как минимум:

- устойчивый смысловой `id` и понятное русское `title` (принцип
  `human-agent-readable`, [`operating-principles.md`](operating-principles.md));
- `work_kind`; `change_class` — **только** при `work_kind: change` и
  **обязательно** при нём;
- `invariants` — что обязано оставаться истинным во время выполнения;
- `required_inputs` — входы, без которых работа не начинается;
- `required_evidence` — проверяемые результаты, подтверждающие результат;
- `stop_conditions` — условия, при которых выполнение останавливается;
- `applicable_protocols` — явные ссылки на применимые существующие **протоколы**
  (обязательный порядок действий и инварианты);
- `applicable_skills` — явные ссылки на применимые существующие **способы
  выполнения** (практические инструкции; артефакт вида
  `skills/<имя>/SKILL.md`);
- `applicable_evidence_contracts` — явные ссылки на применимые существующие
  **контракты доказательств**.

Списки `invariants`, `required_inputs`, `required_evidence` и
`stop_conditions` не бывают пустыми и не содержат пустых строк. Все три оси
ссылок обязательны и непусты; там, где канонического источника нет, ось несёт
одну запись `status: absent` с причиной (§3).

### 2.1. Правила отдельных классов

- **`change` + `change_class`.** `change_class` осмыслен только внутри
  `work_kind: change` и там обязателен. У `assessment`, `operation` и
  `initiative` его нет.
- **`initiative`.** Требует декомпозиции: несёт `decomposition_required: true`
  и **не получает** `change_class`. Инициатива не резолвится как единая
  неделимая единица и не маркируется одним классом изменения
  (`$MERIDIAN_INSTANCE/governance/meridian-owner-intent-contract.md` §12,
  [`rule-resolution.md`](rule-resolution.md) §6). Только `initiative` несёт
  это поле.
- **`BUGFIX`.** Способ выполнения исправлений — завендоренный
  `skills/bugfix-protocol/SKILL.md` — записан в `applicable_skills`, **не** в
  `applicable_protocols` (`skill` и `protocol` — разные сущности глоссария,
  решение владельца). Отдельного документа-протокола класса `protocol` для
  `BUGFIX` в Ядре нет, поэтому `applicable_protocols` шаблона `fix-defect`
  несёт честный `status: absent`. Действующий текст
  [`rule-resolution.md`](rule-resolution.md) §7 ограниченно приведён в
  соответствие: он больше не называет `BUGFIX → bugfix-protocol` маршрутом к
  протоколу Ядра. Нового протокола `BUGFIX` не вводится, и другие правила
  разрешения норм не затрагиваются. `BUGFIX` не смешивается с `FEATURE` и
  `BEHAVIOR_CHANGE`: проверка требует, чтобы способ выполнения `fix-defect` не
  встречался в ссылках `add-capability` и `change-behavior`.
- **`REFACTOR`.** Сохраняет наблюдаемое поведение и ссылается на настоящий
  протокол и контракт функционального паритета —
  [`refactor-protocol.md`](../../verification/functional-parity/refactor-protocol.md)
  в `applicable_protocols` и
  [`functional-parity-evidence-contract.md`](../../verification/functional-parity/functional-parity-evidence-contract.md)
  в `applicable_evidence_contracts`. Проверка требует обе ссылки как
  `status: present`.

### 2.2. Три оси ссылок — разные сущности, не взаимозаменяемы

`applicable_protocols`, `applicable_skills` и `applicable_evidence_contracts` —
раздельные оси. Проверка это доказывает механически:

- запись `applicable_skills` со `status: present` обязана указывать на артефакт
  вида `skills/<имя>/SKILL.md`;
- запись `applicable_protocols` или `applicable_evidence_contracts` со
  `status: present` **не** может указывать на путь под `skills/`.

Поэтому способ выполнения нельзя поместить в массив протоколов, а протокол —
в массив способов выполнения: любая такая попытка делает прогон красным.

Отдельная текстовая защита проверяет, что
[`rule-resolution.md`](rule-resolution.md) не вернулась к формулировке
`BUGFIX → bugfix-protocol` как маршруту к протоколу Ядра и не называет
`bugfix-protocol` «протоколом ядра» в одной строке. Это машинная защита именно
от повторного появления согласованного здесь расхождения; она не расширяет
правила разрешения норм.

## 3. Отсутствие канонической связи — явное, а не подставленное

Если для класса ещё нет канонического протокола или контракта доказательств,
это **не** заполняется правдоподобной ссылкой. Ссылка записывается как

```yaml
- status: absent
  absence_reason: <почему источника ещё нет и что действует до него>
```

`present`-ссылка обязана нести `id` и `path`; `absent`-ссылка обязана нести
`absence_reason` и **не** нести `id`/`path`. Скрытого запасного поведения нет
(принцип `no-hidden-fallback`): отсутствие представлено машинно различимым
`status: absent`, а каждая `present`-ссылка проходит проверку принадлежности
Ядру (§4.1) — не просто «внешний файл отсутствует».

На текущей линии Ядра канонические источники существуют для `BUGFIX` (способ
выполнения `skills/bugfix-protocol/SKILL.md` и запись регрессионного
доказательства `verification/regression-testing/README.md`) и для `REFACTOR`
(протокол и контракт функционального паритета). Отдельного документа-протокола
класса `protocol` для `BUGFIX` в Ядре нет — это записано `absent`, а не
подменено ссылкой на способ выполнения или на маршрут. Для `assessment`,
`operation`, `initiative`, `FEATURE` и `BEHAVIOR_CHANGE` отдельные способ
выполнения и контракт доказательств в Ядре пока не определены и записаны как
`absent`; каждый такой шаблон при этом ссылается на применимый общий протокол —
жизненный цикл задачи.

## 4. Композиция проверок, а не конкурирующая схема

Каждый шаблон является записью встроенной методологии и подчиняется модели
областей и универсальному конверту записи
([`workspace-scope-model.md`](workspace-scope-model.md)):

```yaml
scope:   { type: built-in-methodology, id: built-in-methodology }
origin:  { kind: built-in }
authority: { kind: methodology-owner, authority_ref: methodology-owner }
record_type: task-pattern
```

Конверт **не переопределяется** новой схемой. Проверка композитна и живёт в
одной функции
[`scripts/lib/task-pattern-registry.mjs`](../../scripts/lib/task-pattern-registry.mjs):

1. общий конверт каждой записи проверяется существующей
   [`scoped-record.schema.json`](../../registries/operating-model/scoped-record.schema.json);
2. содержимое `payload` шаблона проверяется новой специализированной
   [`task-pattern-registry.schema.json`](../../registries/operating-model/task-pattern-registry.schema.json);
3. межзаписные правила, которые подмножество JSON Schema выразить не может
   (ровно семь классификационных пар по одному разу, отсутствие повторов `id`,
   разделение трёх осей ссылок, маршруты `REFACTOR` и `BUGFIX`, принадлежность
   каждой `present`-ссылки Ядру — §4.1), проверяются кодом этой функции.

Обе схемы используют поддерживаемое текущим движком подмножество JSON Schema
(`scripts/lib/json-schema.mjs`); неподдерживаемое ключевое слово в любой ветви
делает прогон красным.

### 4.1. Принадлежность цели ссылки Ядру

Для каждой ссылки со `status: present` проверяется, что путь:

- относительный и уже нормализованный — абсолютный путь, сегменты `.` и `..`,
  а также обратная косая черта как разделитель запрещены;
- лексически лежит внутри `KERNEL_ROOT`;
- после разрешения символических ссылок физически остаётся внутри
  `KERNEL_ROOT`;
- указывает на **обычный файл**, а не на каталог;
- принадлежит **отслеживаемому** набору файлов Ядра.

Проверка доказывает принадлежность Ядру, а не только отсутствие конкретного
внешнего файла. Продуктово-нейтральные отрицательные сценарии (абсолютный путь,
`../outside.md` при существующем внешнем файле, символическая ссылка наружу,
каталог вместо файла, существующий неотслеживаемый файл, путь с обратной косой
чертой) закреплены в `test/task-pattern-registry.test.mjs` и в бандле фикстур.

## 5. Разные сущности остаются разными

Шаблон типа задачи (`task-pattern`), постановка задачи (`task-specification`),
запуск (`execution-run`), норма (`norm`), протокол (`protocol`), рабочий
процесс (`workflow`), способ выполнения (`skill`), средство (`tool`),
проверочный рубеж (`gate`) и доказательство (`evidence`) — разные сущности
([`operating-glossary.md`](operating-glossary.md)). Каталог содержит только
шаблоны и раздельные ссылки на протоколы, способы выполнения и контракты
доказательств; он не вводит постановку задачи и не описывает состояние запуска —
это отдельные пакеты программы `meridian-operating-upgrade`. Существующие
сущности каталог не переименовывает и опирается на действующие пулы
`work_kind`/`change_class` и на разграничение `protocol` / `skill` / `evidence`,
уже заданное глоссарием. Единственное правило вне каталога, приведённое в
соответствие, — §7 [`rule-resolution.md`](rule-resolution.md): по решению
владельца оно ограниченно перестало называть `bugfix-protocol` протоколом
Ядра. Новый протокол `BUGFIX` не вводится; прочие правила разрешения норм не
меняются.

## 6. Изменение каталога

Добавление, удаление или изменение смысла шаблона — изменение универсальной
нормы Ядра: оно требует записи в журнале изменений и учитывается при выборе
следующей версии. Набор классификационных пар закрыт действующими пулами
`work_kind`/`change_class`; расширение каталога следует за расширением этих
пулов в [`rule-resolution.md`](rule-resolution.md), а не опережает его.
