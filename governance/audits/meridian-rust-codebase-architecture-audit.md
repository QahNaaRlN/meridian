---
title: Аудит архитектуры всей Rust-кодовой базы Meridian
document_type: report
status: current
scope: workspace
owner: workspace-owner
created: 2026-09-21
updated: 2026-09-21
related_documents:
  - $MERIDIAN_KERNEL/standards/workspace/rust-migration-quality.md
  - $MERIDIAN_KERNEL/governance/decisions/meridian-rust-sqlite-architecture.md
  - $MERIDIAN_KERNEL/governance/specifications/meridian-rust-target-architecture.md
  - $MERIDIAN_KERNEL/governance/plans/meridian-rust-migration-program-plan.md
---

# Аудит архитектуры всей Rust-кодовой базы Meridian

## 1. Вердикт

Текущее дерево **частично соответствует** выбранной архитектуре, но не готово
к продолжению функциональной миграции. Макрограницы workspace в основном
сохранены; внутри `validate` накоплен системный буквальный перенос Node.js,
который нарушает обязательные уровни `transport -> syntax -> domain -> valid
type`. До исправления выводов этого аудита подпакет 7c не интегрируется, 7d и
пакет 8 не начинаются.

Это архитектурный аудит, а не требование уменьшить число строк. Большой файл
сам по себе не дефект; дефект — смешение ответственностей, сырого транспорта и
предметной логики, которое размер делает видимым.

## 2. Область и метод

Проверены все четыре крейта Cargo workspace и весь текущий Rust-инвентарь:

| Крейt | `.rs` файлов | Строк всего | Production `src` |
|---|---:|---:|---:|
| `meridian-core` | 40 | 18 239 | 14 735 |
| `meridian-app` | 33 | 18 959 | 18 744 |
| `meridian-storage-sqlite` | 7 | 3 573 | 1 726 |
| `meridian-cli` | 40 | 9 627 | 8 002 |
| **Итого** | **120** | **50 398** | **43 207** |

Сверены: зависимости крейтов, наличие I/O и `unsafe`, расположение SQLite,
порты, транспортные типы, использование `serde_json::Value`, типы диагностик,
крупные модули, `unwrap`/`expect` в production-коде и утверждения о
`verbatim`/построчном паритете. Сгенерированный
`meridian-core/src/json_number_corpus_data.rs` (6216 строк, явно помечен
`GENERATED`) не считается архитектурным монолитом.

## 3. Что уже соответствует архитектуре

### A. Workspace и зависимости — соответствует

- Четыре принятых крейта существуют; циклических зависимостей нет.
- `meridian-core` не зависит от serde, SQLite, CLI или filesystem crates.
- `meridian-storage-sqlite` — единственный production-крейт с `rusqlite`.
- На workspace действует `unsafe_code = "forbid"`; собственного `unsafe` нет.
- Async/runtime и сетевые зависимости в предметное ядро не введены.

### B. Типизированные участки — соответствует или близко к цели

- `meridian-core::{types,resolver,migration,evidence}` использует закрытые
  типы, валидируемые конструкторы и типизированные ошибки.
- `meridian-app::rule_resolution` применяет закрытые serde DTO с
  `deny_unknown_fields`, преобразует transport в `meridian-core` и проверяет
  выход; это эталонный маршрут для остальных переносов.
- `meridian-app::storage` выражает роли, запросы, ревизии и маршрутизацию
  адаптер-нейтральными типами.
- SQLite schema/storage изолированы в собственном адаптере и используют
  транзакционные границы.
- `meridian-app::source_format` остаётся адаптером формата; присутствие там
  JSON/YAML-представлений допустимо по назначению слоя.

## 4. Блокирующие выводы

### A1 — CRITICAL: критерий приёмки фактически был сужен до паритета

На момент аудита план и исходный код многократно называли перенос `Verbatim
port` или построчным. Это позволило зелёному conformance заменить архитектурную
приёмку. Источник Node должен определять сохраняемую бизнес-семантику, но не
структуру Rust. Норма `rust-migration-quality.md` исправляет правило; каждый
следующий пакет и каждое повторное ревью обязаны применять её до conformance.

### A2 — HIGH: весь `meridian-app::operating_model` смешивает уровни

Одиннадцать production-модулей одновременно принимают сырой
`serde_json::Value`, применяют schema/shape-проверки, выполняют предметные
правила и возвращают строковые диагностики. В каталоге более 1000 упоминаний
`Value`, но почти нет публичных валидных предметных типов: опубликованные
структуры преимущественно являются контейнерами schema/options.

Наиболее заметные модули:

| Модуль | Строк | Упоминаний `Value` | Риск |
|---|---:|---:|---|
| `evidence_and_handoff.rs` | 2909 | 229 | транспорт, резолвинг, evidence, verdict и diagnostics смешаны |
| `field_evaluation.rs` | 2442 | 184 | transport, календарь, измерения и агрегация смешаны |
| `bounded_context_manifest.rs` | 2074 | 150 | transport, pinned resolution и доменные инварианты смешаны |
| `role_and_human_control.rs` | 1207 | 90 | два контракта и их предметные правила в одном модуле |
| `controlled_rule_intake.rs` | 1030 | 91 | кандидаты, authority, кластеры и граф конфликтов остаются JSON |
| `existing_project_compatibility_mode.rs` | 999 | 99 | композиция контрактов происходит над сырыми объектами |

Это нарушает §5–§6 целевой архитектуры: после валидации должен существовать
тип, в котором невалидное состояние непредставимо, а предметная операция
работает не с `Value`.

### A3 — HIGH: часть `validate` реализует предметную логику прямо в CLI

Модули 7a (`operating_foundation`, `instruction_topics`, `stack_profiles`,
`agent_instruction_identity`, `sha_provenance` и часть других механических
проверок) содержат значительную проверочную логику в
`meridian-cli::commands::validate`. CLI должен реализовывать чтение workspace
и presentation, а orchestration/предметные правила — находиться в app/core.
Историческое принятие плоской реализации 7a не отменяет целевую архитектуру.

### A4 — HIGH: четыре заявленных порта отсутствуют как traits

Спецификация требует `SourceResolver`, `GitInspector`, `WorkspaceReader` и
`Clock` в `meridian-app`, с реализациями в `meridian-cli`. В текущем
production-коде присутствуют `RecordRepository`, `EvidenceRepository`,
`RoledStorage` и `EventSink`, но четыре перечисленных порта не определены.
Callbacks отдельных проверок не заменяют именованную, повторно используемую
архитектурную границу.

### A5 — HIGH: интегрированный 7b также требует исправления

Проблема возникла до 7c: семь семейств 7b уже интегрированы в той же
Value-centric форме. Коммиты остаются историческим фактом и не
переписываются, но выпускная готовность этих модулей отзывается до
архитектурного corrective package. 7c не должен закреплять этот шаблон, а 7d
не должен его продолжать.

## 5. Существенные, но неблокирующие сами по себе выводы

- `Vec<String>` широко используется вместо существующего типизированного
  `meridian_core::types::Diagnostic`; это облегчает буквальный перенос и
  усложняет машинную обработку provenance/path/code/severity.
- `unwrap`/`expect` многочисленны, но большая часть находится в тестах или на
  доказанных внутренних инвариантах. Их нельзя запрещать счётчиком; нужен
  отдельный аудит каждого production call site на данные внешнего происхождения.
- Размер модулей — индикатор, не gate. Декомпозиция нужна по предметной
  ответственности и типам, а не по лимиту строк.
- Real Node/Rust conformance остаётся ценным регрессионным инструментом, но
  его зелёный результат теперь является последним, а не первым рубежом.

## 6. Обязательная программа исправления

До 7d создаётся и независимо принимается corrective package
`rust-architecture-conformance`:

1. определить отсутствующие app-порты и CLI-адаптеры;
2. выбрать один вертикальный pilot из `operating_model` и реализовать маршрут
   transport DTO -> syntax -> domain constructor -> core operation -> typed
   diagnostics -> presentation;
3. после подтверждения шаблона последовательно перевести все семейства 7a–7c;
4. сохранить бизнес-fixtures и conformance как regression corpus, но явно
   классифицировать допустимые Rust-native улучшения;
5. не возвращать удалённую по результатам аудита маркировку `Verbatim port`;
   заголовки должны описывать сохраняемую семантику и выбранную архитектуру;
6. провести production-only audit panic/unwrap/expect;
7. добавить автоматические архитектурные gates: запрещённые crate
   dependencies/I/O, обязательные boundary types и проверку отсутствия новых
   Value-centric operating-model entrypoints;
8. только после `ACCEPTED` и интеграции corrective package возобновить 7d.

Pilot не должен быть самым простым механическим модулем: он обязан включать
внешнее разрешение, предметные инварианты и диагностику. Рекомендуемый pilot —
`controlled-rule-intake`; после него `existing-project-compatibility-mode`
доказывает композицию уже типизированных контрактов.

## 7. Статус по группам

| Область | Статус | Следующее действие |
|---|---|---|
| `meridian-core` types/resolver/migration/evidence | provisionally conformant | production panic audit, расширение типов operating model |
| `meridian-app::rule_resolution` | conformant reference | использовать как образец границы |
| `meridian-app::storage` / events | provisionally conformant | дополнить отсутствующие порты |
| `meridian-app::source_format` | conformant boundary | не протаскивать `Value` дальше transport |
| `meridian-app::operating_model` | architecture-blocked | типизированный перенос всех семейств 7b–7c |
| `meridian-cli::commands::validate` | architecture-blocked | оставить I/O/presentation, вынести правила |
| `meridian-storage-sqlite` | provisionally conformant | production panic/transaction audit |
| 7c | acceptance withdrawn | не интегрировать до corrective package |
| 7d / пакет 8 | blocked | не начинать |
