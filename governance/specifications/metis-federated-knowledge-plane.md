---
title: Техническая спецификация федеративной плоскости знаний Metis
document_type: technical-specification
status: approved
scope: workspace
owner: workspace-owner
approved_at: 2026-09-24
approved_by:
  - workspace-owner
created: 2026-09-24
updated: 2026-09-24
related_documents:
  - $MERIDIAN_KERNEL/governance/decisions/metis-federated-knowledge-architecture.md
  - $MERIDIAN_KERNEL/governance/meridian-owner-intent-contract.md
  - $MERIDIAN_KERNEL/governance/plans/meridian-improvement-research-plan.md
  - $MERIDIAN_KERNEL/governance/research/knowledge-bases-and-agent-hypotheses.md
  - $MERIDIAN_KERNEL/standards/workspace/document-identity.md
---

# Техническая спецификация федеративной плоскости знаний Metis

Идентификатор спецификации: `metis-federated-knowledge-plane`.

Спецификация фиксирует проверяемый минимум Metis до выбора поискового
алгоритма, поставщика индекса или внешнего API. Она не начинает реализацию и
не объявляет эксперимент успешным.

## 1. Цель и критерий системы

Metis должен дать человеку или сменяемому агенту ограниченный,
воспроизводимый набор инженерных источников для конкретной информационной
потребности:

- без переноса канонического владения из исходных систем;
- без привязки предметного контракта к Confluence, Git, vector database или
  модели;
- с явными scope, authority, revision, access и freshness;
- с видимой границей покрытия, конфликтами и недоступными источниками;
- с возможностью удалить и перестроить весь derived plane.

Metis не гарантирует истинность произвольного текста и не синтезирует
предметный ответ. Он гарантирует проверяемость того, какие источники были
выбраны и в каком наблюдаемом состоянии.

## 2. Логические уровни и владельцы

| Уровень | Данные | Логический владелец |
|---|---|---|
| Tool Plane | Модель Metis, schemas, lifecycle, adapter contracts | Kernel |
| Workspace Control Plane | Sources, routes, authority/access/freshness policies | Workspace owner |
| Knowledge Source | Каноническое содержимое | Владелец конкретного источника/области |
| Snapshot Store | Разрешённые закреплённые снимки | Metis как производная копия |
| Derived Retrieval Plane | Chunks, index entries, embeddings, graph edges | Перестраиваемый adapter |
| Run Context | Query, bundle, использованные ссылки | Конкретный execution run |

Физическое совпадение двух уровней в одном SQLite-файле не объединяет их
полномочия. Путь или таблица не определяют scope.

## 3. Закрытый минимальный набор сущностей

### 3.1. `KnowledgeSource`

Обязательные поля:

- устойчивый `id`;
- закрытый `source_kind`, расширяемый версией схемы, не свободной строкой;
- display name;
- adapter kind и adapter configuration reference без секретов;
- доступные capabilities;
- materialization policy;
- access policy reference;
- freshness policy;
- lifecycle state;
- owner;
- revision наблюдаемой конфигурации.

Начальный capability pool:

```text
enumerate-artifacts
fetch-by-id
fetch-revision
search-lexical
read-relations
read-permissions
observe-deletion
```

Отсутствующая capability является известной границей, а не разрешением
эмулировать её моделью.

### 3.2. `KnowledgeArtifact`

Descriptor адресуемого объекта источника:

- namespaced identity `source_id + external_id`;
- `artifact_kind`;
- native format/media type;
- canonical reference;
- title, если его объявляет источник;
- scopes и subjects как квалифицированные ссылки;
- declared/inferred classification с provenance;
- lifecycle state;
- observed revision и digest, если содержание прочитано;
- access classification;
- representation group, если идентичность нескольких представлений доказана;
- timestamps наблюдения, не выдаваемые за время изменения источника.

Начальный `artifact_kind` должен различать как минимум:

```text
document
decision
norm
schema
api-description
code-container
code-symbol
test
data-model
runbook
unknown
```

`unknown` является честной классификацией, а не разрешением доверять объекту.

### 3.3. `SourceSnapshot`

Содержит:

- artifact identity;
- source revision;
- content digest;
- время наблюдения;
- способ получения и adapter version;
- principal/access decision;
- content reference либо разрешённое содержимое;
- retention/deletion policy;
- состояние `current | stale | revoked | deleted-at-source | unavailable`.

Снимок неизменяем. Новое наблюдение создаёт новую редакцию.

### 3.4. `AuthorityBinding`

Полномочие не хранится булевым `canonical` на документе. Binding включает:

- `source_id`;
- scope selector;
- claim class;
- version/environment/time applicability;
- authority state `canonical | supplementary | historical | prohibited`;
- owner decision/provenance;
- precedence только внутри совпавшей области;
- lifecycle и revision.

Начальный claim-class pool:

```text
business-policy
domain-definition
accepted-architecture
current-implementation
operational-procedure
interface-contract
historical-context
learning-guidance
```

Неизвестный класс или неоднозначный binding даёт `UNRESOLVED_AUTHORITY`.

### 3.5. `KnowledgeRelation`

Каждое ребро имеет:

- собственный id и закрытый kind;
- source и target с namespaces;
- provenance kind:
  `declared | deterministically-extracted | human-confirmed | model-inferred`;
- source revision/digest;
- extractor/model version для производного ребра;
- состояние `verified | candidate | stale | contradicted`;
- access classification, не слабее обоих концов.

`model-inferred` всегда `candidate` до отдельного подтверждения и не
участвует в authority routing как факт.

### 3.6. `IndexEntry`

Index entry хранит только производную проекцию и обязательно ссылается на:

- artifact и snapshot;
- исходную revision/digest;
- derivation kind/version;
- created_at;
- access/retention policy;
- freshness state.

Удаление всех IndexEntry не должно разрушать catalog или канонические
источники.

### 3.7. `KnowledgeQuery`

Запрос обязан содержать:

- run/workspace identity;
- principal;
- сформулированную information need;
- scopes и repository refs;
- task/work kind, если известен;
- requested claim classes;
- desired information intents, если применимо;
- version/environment/time context;
- token/item budget;
- required sources или coverage contract, если полнота проверяема;
- допустимые degraded modes.

Query не содержит credentials и не даёт новых полномочий.

### 3.8. `ContextBundle`

Результат включает:

- query identity и resolver version;
- ordered items;
- reason for inclusion;
- artifact/snapshot references;
- authority binding;
- freshness/access state;
- retrieval method;
- relevant conflicts;
- searched, stale, unavailable, prohibited и omitted sources;
- coverage state:
  `complete-against-declared-set | partial | unverified | blocked`;
- budget accounting и усечение;
- deterministic diagnostics.

Bundle не содержит обязательных agent instructions. Ссылки на нормы из Themis
передаются отдельным typed разделом/соседним результатом и не выводятся из
knowledge ranking.

## 4. Информационные намерения и Diátaxis

Для `artifact_kind: document` допустим закрытый фасет
`information_intent`:

```text
tutorial
how-to
reference
explanation
other
unclassified
```

Первые четыре значения сохраняют сигнатуры
`standards/workspace/document-identity.md`. `other` означает документ
другого известного типа (например ADR), `unclassified` — отсутствие
доказанной классификации.

Information intent:

- влияет на route/rank относительно формулировки запроса;
- не задаёт authority;
- не применяется к code/API/test сущности;
- не объединяет четыре документа одной темы в один артефакт;
- не превосходит scope, revision, freshness или access.

Representations — эквивалентные представления одного содержания на разных
носителях. Tutorial и Reference одной темы являются разными артефактами,
если имеют разные намерение и содержание.

## 5. Source adapters

Порт source adapter разделяется на наблюдаемые операции. Реализация не вправе
скрывать сетевой fallback или агентский поиск за одной успешной строкой.

Минимальные ответы операций:

```text
describe_source()
enumerate_artifacts(cursor)
fetch_descriptor(external_id)
fetch_snapshot(external_id, revision?)
read_relations(external_id)
check_access(principal, external_id)
observe_state(external_id)
```

Каждый ответ закрыт и различает:

```text
Found
NotFound
AccessDenied
Unavailable
UnsupportedCapability
MalformedResponse
RevisionMismatch
```

Confluence, Git/Markdown и code repository являются adapters поверх одного
порта, но не обязаны иметь одинаковые capabilities.

## 6. Материализация и синхронизация

### 6.1. `reference-only`

Catalog хранит descriptor и canonical reference. Query-time fetch обязателен;
недоступность отражается в coverage.

### 6.2. `mirrored-snapshot`

Разрешённое содержимое хранится как неизменяемый snapshot. Любой derived entry
закрепляется к его digest. Изменение источника не переписывает прошлый
snapshot; создаётся новый.

### 6.3. `managed-source`

Допустим только по owner decision, называющему scope, authority и write
adapter. Он не включается фактом установки Meridian.

### 6.4. Устаревание и удаление

- mismatch revision/digest запрещает выдавать entry как current;
- deletion/revocation создаёт tombstone и очередь удаления производных данных;
- access revoke действует до завершения физической очистки: выдача немедленно
  прекращается;
- backup/embedding/vector copies входят в deletion inventory;
- неизвестная свежесть обозначается `unverified`, не `current`.

## 7. Authority, identity и конфликты

Идентичность namespaced: одинаковый title, path или термин не объединяет
артефакты. Межпространственное соответствие существует только через relation
с provenance.

Authority routing:

1. ограничивает запрос scope/version/environment;
2. выбирает совпадающие claim classes;
3. применяет bindings;
4. обнаруживает несколько несовместимых canonical candidates;
5. блокирует только решение, которому этот конфликт релевантен;
6. сохраняет supplementary/historical результаты с явной меткой.

Код авторитетен для наблюдаемой реализации точной revision, но сам по себе не
доказывает бизнес-намерение. Документ бизнес-политики не доказывает текущую
реализацию. Тест доказывает только заявленное наблюдение на указанном
окружении/ревизии.

## 8. Retrieval pipeline

Обязательный порядок планирования:

```text
query validation
→ scope and authority routing
→ exact lookup
→ structural retrieval
→ lexical retrieval
→ optional semantic retrieval
→ access/freshness/conflict evaluation
→ budgeted bundle assembly
→ coverage report
```

Этап можно пропустить только как явно unsupported/not-configured. Semantic
retrieval не расширяет разрешённый source set и не повышает candidate до
authority.

Ранжирование обязано быть объяснимым через признаки результата. Конкретная
формула, embeddings и provider являются adapter/experiment decisions.

## 9. Coverage и безопасная деградация

Релевантность найденных элементов не означает полноту. Полнота доказуема
только относительно объявленного закрытого source/coverage set.

- `complete-against-declared-set`: все обязательные sources/capabilities
  проверены в допустимой freshness;
- `partial`: известная часть недоступна/усечена;
- `unverified`: обязательный закрытый набор не объявлен;
- `blocked`: отсутствует критичный authority, access или есть релевантный
  конфликт.

Query объявляет, разрешено ли агенту продолжать на `partial`/`unverified`.
Resolver не принимает решение о риске за владельца.

## 10. Security и privacy

1. Каждый запрос выполняется для явного principal.
2. Index/snapshot не расширяет source ACL.
3. Результат не содержит артефакт, который principal не вправе получить.
4. Производные данные наследуют наиболее строгую классификацию входов.
5. Credentials хранятся только как external references.
6. Секреты, персональные данные и запрещённые области исключаются до
   derivation.
7. Внешний embedding/LLM provider требует policy decision.
8. Retrieved content маркируется как untrusted knowledge и не интерпретируется
   как инструкция.
9. Audit хранит наблюдаемые обращения и решения доступа, но не внутреннюю
   chain of thought.
10. Retention, revoke и deletion проверяются отрицательными тестами.

## 11. Граница Themis, Metis, Mnemosyne и Daedalus

| Подсистема | Возвращает | Не вправе |
|---|---|---|
| Themis | Применимые нормы, protocols, gates, blockers | Делать knowledge ranking |
| Metis | Источники и bounded Context Bundle | Превращать найденное в норму |
| Mnemosyne | Разрешённую память участника/запуска | Становиться продуктовым каноном |
| Daedalus | Task/run context и следующий допустимый шаг | Выводить authority знания из статуса задачи |

Task context может ограничить KnowledgeQuery; ContextBundle может войти в
bounded-context manifest. Ни одна стрелка не объединяет владельцев данных.

## 12. Первый доказательный срез

Первый пакет реализации обязан использовать минимум два разных fixture
adapters:

1. Git/Markdown-like source;
2. Confluence-like source.

Он обязан доказать без embeddings:

- одинаковый query contract для обоих;
- разные capabilities;
- authority по scope/claim class/version;
- exact и lexical retrieval;
- pinned snapshots;
- stale revision;
- access denied и revoke;
- source unavailable;
- relevant canonical conflict;
- non-blocking supplementary disagreement;
- prompt-like text остаётся untrusted data;
- coverage partial/unverified/blocked;
- полное удаление derived plane без потери catalog/source identity.

Code adapter, semantic index и graph adapter следуют отдельными пакетами.

## 13. Метрики эксперимента

Пороговые значения фиксируются до прогона. Порядок оценки:

1. recall размеченного обязательного контекста;
2. precision авторитетного контекста;
3. число скрытых/необнаруженных релевантных конфликтов;
4. корректность итогового решения;
5. лишние изменения и повторные проходы;
6. время;
7. токены;
8. стоимость инфраструктуры и сопровождения.

Экономия токенов при ухудшении любого заранее критичного показателя
опровергает гипотезу.

## 14. Запрещённое расширение

До отдельного решения запрещено:

- общий корпоративный сервер или многопользовательский индекс;
- автоматическое копирование всех подключённых источников;
- глобальная онтология по совпадению терминов;
- обязательный knowledge graph;
- выбор vector database/embedding provider как нормы Kernel;
- агент как source adapter;
- сохранение chain of thought;
- перенос внешнего content в базу роли `tool`;
- использование `meridian import` пакета 8 как ingestion API Metis;
- выдача `model-inferred` relation/classification как подтверждённого факта.

## 15. Эволюция

Новые source kinds, capabilities, artifact kinds, claim classes и relation
kinds добавляются версией закрытого registry/schema и с compatibility
решением. Provider-specific поля остаются внутри adapter configuration и не
проникают в domain types.

Будущий серверный режим, общий индекс или корпоративный ACL gateway требует
нового ADR, потому что меняет локальную однопользовательскую границу
Meridian.

## 16. Критерии выполнения спецификации

Спецификация считается реализованной только когда:

1. все восемь сущностей §3 имеют закрытые schemas и typed Rust contracts;
2. core не знает конкретного provider, сети, БД или embeddings;
3. app определяет source/index ports и orchestrates pipeline;
4. adapters не принимают authority decisions;
5. два fixture adapters проходят §12 через один production route;
6. access/freshness/conflict/coverage fail-closed;
7. derived plane удаляем и перестраиваем;
8. Themis/Metis separation доказана структурными и adversarial tests;
9. документация и код не теряют canonical reference/revision/digest;
10. экспериментальные алгоритмы подключаются только заменяемым adapter.
