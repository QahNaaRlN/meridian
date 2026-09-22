---
title: Meridian compatibility contract
document_type: contract
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-08-18
updated: 2026-09-21
---

# Meridian compatibility contract

Совместимость **объявляется**, а не выводится из близости номеров версий.

При миграции Node.js → Rust совместимость означает сохранение объявленной
бизнес-ценности, форматов обмена и публичных гарантий. Она не требует
воспроизводить внутреннюю структуру, динамические слабости или известные
дефекты Node.js. Намеренное Rust-native усиление допустимо и обязательно,
когда делает реализацию лучше или надёжнее; такое отличие документируется,
обосновывается и покрывается тестом по
`standards/workspace/rust-migration-quality.md`.

## Релизные единицы

Meridian состоит из независимо версионируемых единиц. Общей линии версий у них нет.

| Единица | Где | Линия версий | Примечание |
|---|---|---|---|
| Kernel | этот репозиторий | SemVer, `VERSION` | методология, валидатор, схемы, writers |
| Instance | отдельный репозиторий на продукт, вне Kernel | Git revision; SemVer только если распространяется | размещается там, где данные продукта; никогда не поставляется вместе с Kernel |
| Завендоренные skills | `skills/<name>/` | неизменяемый SHA-256 в `PIN.yaml` | пин, а не номер версии |
| Smoke-пакет | Instance | собственный SemVer | product-specific по природе |

## Kernel ↔ Instance

| Kernel | Ожидаемый Instance | Ломающие изменения |
|---|---|---|
| `0.1.x` | `schema_version: 1` в `product.yaml` | не объявлены; `0.x` не даёт гарантий стабильности |
| `0.2.x` | `schema_version: 1` в `product.yaml`; action-профили `environments/access.yaml` используют generic-enum или `x-*`-префикс для продуктовых действий | относительно `0.1.x`: enum действий сужен, продуктовые значения без префикса `x-*` не проходят schema-gate — Instance требует миграции (см. `CHANGELOG.md` 0.2.0 → Breaking) |
| `0.4.x` | то же, что `0.2.x`; плюс: рабочие заметки Instance, ссылающиеся на слой шаблонов, указывают на `standards/templates/template-contract.md` и `standards/templates/profiles/<платформа>/<тип>-body.md`; плюс: каждая запись `inventory/repositories.yaml` несёт `profile`, значение которого названо из закрытого пула Kernel stack profiles и подтверждается манифестом соответствующего репозитория | относительно `0.3.x`: слой шаблонов разделён на контракт и профиль, после чего всё дерево приведено к единому правилу имён (`standards/workspace/document-identity.md`). Переименовано шестнадцать файлов, в том числе `CONFLUENCE.md` → `profiles/confluence/confluence-profile.md`, восемь `*.confluence-template.md` → `profiles/confluence/*-body.md`, `PROTOCOL.md` → `smoke-protocol.md`, `ACCEPTANCE-GATE.template.md` → `acceptance-gate-template.md`, `BOOTSTRAP.md` → `instance-bootstrap.md`. Ломает текстовые ссылки Instance, и гейт этого не видит — см. следующий раздел. Плюс относительно `0.3.x`: `registries/inventory/repositories.schema.json` сделал `repositories[].profile` обязательным; отсутствие `profile`, имя вне закрытого пула Kernel stack profiles или объявление, противоречащее манифесту репозитория, даёт FAIL валидатора и требует миграции Instance |
| `0.5.x` | сохраняет все требования `0.4.x`; плюс, при переходе на Kernel `0.5.x`: временные ветви Instance соответствуют закрытому шаблону имён временных ветвей (`standards/workspace/version-control-flow.md` §3, §3.1); сообщения новых коммитов Instance используют форму сообщений коммитов Meridian — типизированный заголовок и четыре раздела тела (§12); принятие этих общих норм конкретным Instance оформляется отдельным репозиторий-локальным пакетом Instance. Instance **не обязан** переименовывать свои стабильную и интеграционную линии в `main` / `dev` и **сохраняет имена своих линий из собственного tracked-источника** (§1.2): постоянные линии `main` / `dev`, их защита и операционное переименование `master` → `main` / `develop` → `dev` относятся только к репозиторию Kernel | относительно `0.4.x` (пакет `git-governance-migration`, см. `CHANGELOG.md` 0.5.0 → Breaking): временная ветвь вне разрешённого закрытого набора норме больше не соответствует; неанглийское имя, транслитерация, верхний регистр и неправильный kebab-case норме не соответствуют; новые коммиты обязаны использовать установленный заголовок и четыре раздела тела; переход конкретного Instance требует отдельного репозиторий-локального пакета, но **не** переписывания и не переименования его существующей истории и ветвей |
| `0.6.x` | сохраняет все требования `0.5.x`; описывает три аспекта совместимости операционной модели `0.6.x`. Эти требования применяются **при фактическом использовании** соответствующей новой модели, а не ретроактивно ко всякому старому Instance просто потому, что Kernel продвинулся до `0.6.x`. **Контракты Ядра**: модель областей рабочих данных (`standards/workspace/workspace-scope-model.md`) и объединённая квалификация совместимости рабочих пространств (`standards/workspace/workspace-compatibility-qualification.md`, закрытый вердикт `QUALIFIED`/`BLOCKED`/`UNVERIFIED`), которая композирует пять уже обязательных для Ядра участков — реестр источников инструкций (`instruction-source-registry`), приём внешних правил (`controlled-rule-intake`), режим подключения существующего проекта (`existing-project-compatibility-mode`) и контракт миграции данных Экземпляра с его каноническим экспортом (`instance-data-migration`, `instance-canonical-export-contract`); плюс отдельная линия операционной модели, также обязательная часть Ядра: каталог типов задач (`task-pattern-registry.md`), контракт постановки задачи (`task-specification.md`), многоосевая модель состояния выполнения (`execution-state-model.md`), роли и управление человеком (`role-and-human-control.md`), ограниченный манифест контекста (`bounded-context-manifest.md`), доказательства и передача результата (`evidence-and-handoff-contract.md`), полевая оценка без единого итогового балла — восемь раздельных характеристик без свёртки в одну цифру (`meridian-field-evaluation.md`) и объединённая квалификация обновления интеграции с закреплёнными (`sha256`-pinned) ссылками на составленные записи, а не встроенными копиями (`upgrade-integration-qualification.md`). Её поле `workspace_transition_compatibility` — обязательный, но продуктово-нейтральный шаблон: Kernel предоставляет только его, закрытым в этом срезе к единственному значению `kernel-template-only`. **Доказательства Экземпляра**: заполненное доказательство перехода конкретного продукта на новую модель рабочих пространств принадлежит отдельному, более позднему пакету Экземпляра — не этому Kernel-срезу. При подключении через модель рабочих пространств Instance предъявляет проверяемые записи, а не заявления — зарегистрированный источник инструкций, кандидата правила с явным решением владельца (обнаружение или чтение источника или кандидата правила само по себе полномочия нормы не даёт), план обнаружения существующего проекта, который не вносит и не изменяет ни одной записи в файлах подключаемого проекта автоматически, и, если заявлена миграция, план миграции с каноническим экспортом; композитная квалификация остаётся `UNVERIFIED`, пока хотя бы один разрешённый кандидат правила не получил решения владельца. **Переходная совместимость**: `standards/workspace/kernel-boundary.md` по-прежнему признаёт отдельный Экземпляр и `MERIDIAN_INSTANCE` действующим и **поддерживаемым** переходным адаптером и больше не объявляет разделение на два Git-репозитория целевой архитектурой или требованием для нового проекта — Instance, уже использующий такое разделение, продолжает соответствовать норме без обязательной консолидации. Ни объединённая квалификация, ни контракт миграции данных не переносят и не запускают перенос продуктовых данных сами и не выбирают постоянное хранилище этим выпуском — они определяют и проверяют независимый от хранилища контракт; положительный вердикт требует предъявленных и разрешённых записей, а не отсутствия найденных проблем. Пакет `meridian-operating-foundation` и последующие пакеты этой линии не начинают, не расширяют и не изменяют PHASE G или Concord. Принятие этой строки конкретным выпускаемым Instance подтверждается не фактом выпуска Kernel `0.6.x`, а отдельной repository-local `revision-promotion`-процедурой самого этого Instance (`version-control-flow.md` §1.3, §2.3) | относительно `0.5.x`: явных Breaking-изменений не заявлено. Исполнитель и Git-интегратор перестают быть привязаны к конкретной модели или приложению (`version-control-flow.md` §5) — их наличие и распределение для конкретного Instance устанавливает принятый им repository- или task-local протокол, а не универсальная норма Ядра. Механизм проверки имени ветви (§13.2) сужен до проверки исходной ветви merge request на уже обязательном участке — закрытый шаблон имён (§3, §3.1) и топология ветвей не изменены |

Пока Kernel в `0.x`, любой minor может сломать Instance. Instance пиннит точную
версию Kernel до выхода `1.0.0`.

## Instance продукта и прежний Instance разработки Meridian

Таблица выше — о произвольном продуктовом Instance любого проекта,
использующего Meridian: он остаётся поддерживаемым переходным адаптером под
`0.6.x`, как и было. Ничего в этом контракте не объявляет продуктовые Instance
немедленно запрещёнными или устаревшими.

Отдельно от этого: конкретный прежний Instance, ранее служивший центром
управления разработкой самого Meridian (а не продукта), с 2026-09-19 заморожен
для чтения (`AGENTS.md` §10; `governance/meridian-owner-intent-contract.md`
§25) — он больше не активный источник планов, назначений или Git-процесса этой
разработки. Это решение о разработке Meridian, а не о совместимости Kernel с
продуктовыми Instance в целом.

Окончательный рубеж, после которого чтение `MERIDIAN_INSTANCE` для работы над
Meridian перестаёт быть нужным вовсе (включая переходный
`scripts/preflight.mjs --require-instance`), — проверенный импорт пакета 8
(`meridian-cli-migration`) программы `meridian-rust-migration` и отключение
эксплуатационных чтений через `MERIDIAN_INSTANCE`
(`governance/plans/meridian-rust-migration-program-plan.md` §6.5a, §7). До
этого рубежа замороженный источник остаётся читаемым только явно и только для
миграционных целей.

## Ссылки Instance на пути внутри Kernel

Гейт Kernel проверяет две вещи и не проверяет третью:

- относительные Markdown-ссылки **внутри** Kernel — проверяются: несуществующая
  цель красит прогон;
- относительная ссылка из Kernel, уходящая **за** его границу, — дефект по
  правилу, даже если на текущей машине она открывается;
- **текстовая ссылка из Instance на путь внутри Kernel** (`$MERIDIAN_KERNEL/…`)
  — не проверяется ничем. Это не Markdown-ссылка, и разрешать её валидатору не
  с чем: Instance для него — данные, а не исходник, и целевого корня у такой
  ссылки в общем случае нет.

Следствие, а не недосмотр: переименование файла в Kernel может молча сломать
рабочую заметку Instance, а прогон останется зелёным. Поэтому перемещения в
нормативных каталогах Kernel объявляются здесь строкой в таблице выше, а не
выводятся из того, что гейт не покраснел. Владелец Instance правит свои ссылки
по объявленной строке.

Исторические записи Instance (отчёты, архив анализа) при таком перемещении **не
переписываются**: они фиксируют путь, который существовал на момент записи. По
той же причине, по которой не переписывается `CHANGELOG.md`.

## Намеренные Rust-native усиления над Node.js-эталоном

Записи реестра намеренных наблюдаемых расхождений Rust-реализации с
Node.js-эталоном, каждое — принятое Rust-native усиление
(`standards/workspace/rust-migration-quality.md`), не дефект переноса.
Каждая запись обоснована, ограничена и покрыта положительным и
отрицательным тестом.

| Норма/пакет | Node.js-поведение | Rust-native усиление | Тест |
|---|---|---|---|
| `controlled-rule-intake` (`rust-architecture-conformance-1`) | `scripts/lib/controlled-rule-intake.mjs`'s `checkCluster` сравнивает членов одного `semantic_key`-кластера по `scopeKey = \`${type}::${id}\`` — расхождение по `workspace_id`/`organization_profile_id` молча не замечается | `meridian_core::controlled_rule_intake::checks::check_cluster` сравнивает по полному тождеству `Scope` (те же поля, что уже использует `same_scope`/`checkSupersedes` контракта миграции данных Экземпляра) — два члена кластера, совпадающие по `type`/`id`, но расходящиеся по `workspace_id`, теперь корректно помечаются несогласованными | Rust: `meridian-app/src/operating_model/controlled_rule_intake/mod.rs::tests::a_cluster_whose_members_disagree_only_on_workspace_id_is_flagged`; реальный Node/Rust: `test/conformance-harness.test.mjs`, намеренная граница «cluster scope divergence» — Node принимает мутированный кластер чисто, Rust отклоняет его с диагностикой `more than one scope` |
| `controlled-rule-intake` (`rust-architecture-conformance-1`, корректирующий раунд, item 2) — **библиотечное, не CLI-наблюдаемое** отличие; см. примечание ниже | `evaluateControlledRuleIntake` никогда не останавливается на нарушении схемы (только на исключении компиляции самой схемы) — продолжает вычислять полный составной анализ (id/origin/authority/source_ref/кластеры/граф конфликтов) даже над записью, уже нарушившей свою схему | `evaluate_controlled_rule_intake` останавливается сразу при любом нарушении схемы контейнера ИЛИ записи и не строит доменные объекты вовсе (явное решение владельца, корректирующий раунд `rust-architecture-conformance-1` item 2) — схема-невалидный документ даёт только диагностики схемы | Прямое библиотечное доказательство (третий корректирующий раунд item 6, четвёртый item 3.3), НА ОДНОМ И ТОМ ЖЕ реальном fixture-документе (`registries/operating-model/fixtures/controlled-rule-intake.fixtures.json`'s `valid[0].registry`) с ОДИНАКОВЫМИ двумя мутациями (`registry_id` заменён на неверную непустую строку — отклоняется schema `const`, но принимается closed transport DTO, поскольку `RegistryDto.registry_id` не валидируется; испорчен `origin.source_ref`) на обеих сторонах, не «аналогичным по форме» другим документом (пятый корректирующий раунд, item 1 — прежняя мутация, удаление `classification_basis`, не имела эмпирических зубов на Rust-стороне: это поле одновременно обязательно по схеме и невалидируемо как non-optional `String` в закрытом DTO, так что тест оставался зелёным и при отключённом schema short-circuit): Rust — `meridian-app/src/operating_model/controlled_rule_intake/mod.rs::tests::schema_and_composite_defect_together_produce_exactly_one_schema_diagnostic_matching_the_node_reference` (прямой вызов `evaluate_controlled_rule_intake`, утверждает РОВНО 1 диагностику и явное отсутствие диагностики origin/source_ref); Node — `test/conformance-harness.test.mjs`, блок с прямым импортом `evaluateControlledRuleIntake` из `scripts/lib/controlled-rule-intake.mjs` (минуя `kernel-validate.mjs` целиком), утверждает МИНИМУМ 2 диагностики (схема И origin/source_ref) на том же документе. Отдельно, на исполняемом пути через self-test wrapper: `test/conformance-harness.test.mjs`, «schema short-circuit» — на записи с ОДНОВРЕМЕННО нарушенной схемой И расходящимся `origin.source_ref` полный wrapped-вывод **совпадает** на практике (conformant), потому что и `kernel-validate.mjs` (строка ~1952, `p[0]`), и Rust CLI-обёртка сообщают только ПЕРВУЮ диагностику отклонённой fixture, а диагностика схемы у обеих сторон всегда идёт первой — расхождение в количестве вычисляемых диагностик реально на уровне библиотеки (доказано прямыми тестами выше), но не наблюдаемо на единственном сегодняшнем self-test исполняемом пути (отдельной CLI-команды, читающей произвольный документ controlled-rule-intake напрямую, не существует — реестр остаётся данными Экземпляра) |
| `instruction-source-registry` (`rust-architecture-conformance-2`) — **библиотечное** отличие, наблюдаемое напрямую (нет self-test `p[0]`-обёртки, скрывающей его, как у `controlled-rule-intake`) | `evaluateInstructionSourceRegistry`'s обычный доступ к полям объекта не имеет понятия «закрытая транспортная форма» — лишнее поле в `payload` не мешает библиотеке вычислить остальные бизнес-проверки (например read_channel coherence) над той же записью | Транспортный слой `meridian-app/src/operating_model/instruction_source_registry/dto.rs` — закрытые `#[serde(deny_unknown_fields)]` DTO; при провале `serde`-разбора записи (лишнее поле или несовпадение типа) `convert::build_source` для этой ОДНОЙ записи не вызывается вовсе — сообщается только диагностика разбора, доменные проверки для неё недостижимы, а не просто не вычислены в этом прогоне | Rust: `meridian-app/src/operating_model/instruction_source_registry/mod.rs::tests::a_transport_parse_failure_skips_business_checks_for_that_entry_only` (РОВНО диагностика разбора, явное отсутствие диагностики read_channel); Node: `test/conformance-harness.test.mjs`, прямой импорт `evaluateInstructionSourceRegistry` из `scripts/lib/instruction-source-registry.mjs`, ОДИН И ТОТ ЖЕ реальный fixture-документ (`instruction-source-registry.fixtures.json`'s `valid[1].registry`) с ОДИНАКОВЫМИ двумя мутациями (`payload.unexpected_field` и `payload.read_channel.agent_auto_read`), утверждает ОБЕ диагностики |
| `existing-project-compatibility-mode` (`rust-architecture-conformance-2`) — тот же класс отличия, что и `instruction-source-registry` выше, на уровне записи `workspace_connections[]` | `evaluateExistingProjectCompatibilityMode`'s обычный доступ к полям `payload` не имеет понятия «закрытая транспортная форма» — лишнее поле не мешает библиотеке вычислить остальные проверки (например `next_step`) над той же записью | Транспортный слой `meridian-app/src/operating_model/existing_project_compatibility_mode/dto.rs` — закрытые `#[serde(deny_unknown_fields)]` DTO; при провале `serde`-разбора записи `evaluate_connection` для этой ОДНОЙ записи не вызывается вовсе | Rust: `meridian-app/src/operating_model/existing_project_compatibility_mode/mod.rs::tests::a_transport_parse_failure_skips_business_checks_for_that_entry_only`; Node: `test/conformance-harness.test.mjs`, прямой импорт `evaluateExistingProjectCompatibilityMode`, ОДИН И ТОТ ЖЕ реальный fixture-документ (`existing-project-compatibility-mode.fixtures.json`'s `valid[1].registry`) с ОДИНАКОВЫМИ двумя мутациями (`payload.unexpected_field` и `payload.next_step`), утверждает ОБЕ диагностики |
| `existing-project-compatibility-mode` (`rust-architecture-conformance-2`, корректирующий раунд, item 5) — **принято архитектором** как обоснованное Rust-native сужение, тот же прецедент, что уже принят для `controlled-rule-intake`'s schema short-circuit выше: итоговый вердикт записи не меняется, меняется только состав вторичных диагностик | `sameLocation` (`scripts/lib/existing-project-compatibility-mode.mjs`) сравнивает СЫРЫЕ поля (`path`/`container_ref`/`service_ref`/`resource_ref`) discovery_plan-слота и discovered source БЕЗУСЛОВНО — даже когда сам слот уже признан невалидным собственной `checkPlanLocation` | Rust сравнивает `location` только между ДВУМЯ успешно построенными типизированными `meridian_core::instruction_source::Location` — сравнивать типизированное значение не с чем, если сама запись слота не построилась. Первичный дефект слота (его собственная location-диагностика) и итоговый FAIL записи сохраняются на обеих сторонах без изменений; outcome partition (`check_discovery_outcomes`) по-прежнему учитывает этот слот — он остаётся в `plan_ids` по независимо типизированному `SemanticId`, извлечённому до и без учёта исхода построения `location` (`rust-architecture-conformance-2`, корректирующий раунд, items 3-4). Отсутствует только ВТОРИЧНАЯ диагностика «location does not match» | Rust: `meridian-app/src/operating_model/existing_project_compatibility_mode/mod.rs::tests::a_discovered_source_naming_an_invalid_plan_slot_gets_no_secondary_location_mismatch_diagnostic` (собственный дефект слота присутствует, `no outcome` отсутствует — слот учтён в partition, — а `location does not match` отсутствует); Node: `test/conformance-harness.test.mjs`, прямой импорт `evaluateExistingProjectCompatibilityMode`, ОДИН И ТОТ ЖЕ реальный fixture-документ (`existing-project-compatibility-mode.fixtures.json`'s `valid[1].registry`) с ОДИНАКОВЫМИ двумя мутациями (`discovery_plan[0].path` испорчен на `"../escape.md"`; `discovered_sources[0].payload.location.path` изменён на другой, валидный, но отличающийся путь), утверждает И собственный дефект слота, И диагностику `location does not match` |
| `sha-provenance` (`meridian-cli-foundation-architecture-remediation`, §5.16) — путь скилла ограничен строгим типом, а не строковым соглашением адаптера | `path.join(dir, pin?.artifact ?? '')` и `path.join(dir, arch.path)` (`scripts/kernel-validate.mjs`) не проверяют, что результат остаётся внутри `dir`: пин, называющий `../../<файл>`, переходит границу каталога скилла и верифицируется по файлу СНАРУЖИ него, если такой файл существует и его дайджест совпадает | `meridian_core::types::WorkspaceRelativePath::join` отклоняет `..`, обратный слэш и абсолютный/drive-prefixed путь на построении значения — выходящее имя `artifact`/`source_archive.path` даёт ТОТ ЖЕ текст диагностики, что и обычный отсутствующий файл («which does not exist» / «source archive … is missing», `meridian_core::mechanical_integrity::sha_provenance::{artifact_missing,archive_missing}`), новый текст не появляется; меняется только то, что выходящее имя больше не может быть удовлетворено файлом снаружи каталога скилла | Rust (типовой уровень, без файловой системы): `meridian-core/src/types/workspace_relative_path.rs::tests::rejects_parent_component`; Rust (через порт, позитивный и отрицательный): `meridian-app/src/validation/mechanical_integrity/sha_provenance.rs::tests::{an_escaping_artifact_name_is_reported_as_missing_not_read_from_outside_the_skill, an_escaping_source_archive_path_is_reported_as_missing_not_read_from_outside_the_skill, a_clean_pin_with_a_nested_artifact_path_verifies_positively}`; реальный Node/Rust на ОДНОМ И ТОМ ЖЕ мутированном рабочем дереве (полная копия репозитория, `copyRepoWithoutGitOrTarget`, плюс один добавленный `skills/<name>/PIN.yaml`, для обоих полей `artifact` и `source_archive.path` отдельно): `test/conformance-harness.test.mjs`, блок «sha-provenance path confinement» — Node действительно читает и принимает файл за пределами каталога скилла (пин чист, дайджест совпадает), Rust CLI сообщает `FAIL … which does not exist` / `FAIL … source archive … is missing` для того же пина |
| `sha-provenance` (`meridian-cli-foundation-architecture-remediation`, первый корректирующий раунд, item 2; финализировано четвёртым корректирующим раундом, item 1) — неполный `source_archive` больше не проходит молча | `if (arch?.path && arch?.sha256) { ... }` (`scripts/kernel-validate.mjs`) — когда пин объявляет `source_archive` только с `path` ИЛИ только с `sha256`, весь блок пропускается целиком: ни OK, ни FAIL, ни WARN для source archive этого скилла вообще не выводится | `meridian_core::mechanical_integrity::sha_provenance::build_archive_report` — единственный production construction report builder, вызываемый приложением (`meridian-app/src/validation/mechanical_integrity/sha_provenance.rs`, НЕ вызывающим `validate_pinned_path`/`validate_pinned_digest` независимо): возвращает `ArchiveConstructionReport { path: Result<WorkspaceRelativePath, ArtifactPathError>, sha256: Result<ContentDigest, PinnedDigestError>, complete: Option<SourceArchive> }`, где `complete` строится через реальный `SourceArchive::new` (не тестовый-only API). Приложение считает `path`/`sha256` "missing" (`Err(…Error::Missing)`) — только тогда весь блок "incomplete"; `path`-или-`sha256`-присутствующий-но-невалидный НЕ считается incomplete (тот же truthy-only gate, что и у Node) и всё равно доходит до попытки чтения. Транспортный слой сообщает РОВНО ОДИН FAIL (`source_archive_incomplete`) при missing — половинчатая декларация проверяется против ничего и не верифицирует ничего | Rust: `meridian-core/src/mechanical_integrity/sha_provenance.rs::tests::{source_archive_cannot_be_constructed_with_a_missing_path, source_archive_cannot_be_constructed_with_a_missing_or_invalid_sha256, archive_report_complete_is_none_when_path_is_escaping_even_though_sha256_is_valid, archive_report_complete_is_some_only_when_both_fields_are_valid}`; `meridian-app/src/validation/mechanical_integrity/sha_provenance.rs::tests::{a_source_archive_missing_path_or_sha256_is_a_new_incomplete_diagnostic, a_source_archive_with_a_malformed_but_present_sha256_still_attempts_the_read}`; реальный Node/Rust на ОДНОМ И ТОМ ЖЕ мутированном рабочем дереве (полная копия репозитория, `copyRepoWithoutGitOrTarget`, плюс один добавленный `skills/<name>/PIN.yaml` с `source_archive.path` без `source_archive.sha256`): `test/conformance-harness.test.mjs`, блок «sha-provenance: неполный source_archive» — Node не выводит ни OK, ни FAIL про source archive этого скилла вообще, Rust CLI сообщает РОВНО ОДИН `FAIL … missing path, sha256, or both` |
| `operating-foundation` (`meridian-cli-foundation-architecture-remediation`, первый корректирующий раунд, item 2; финализировано вторым корректирующим раундом, item 4) — data-запись без `id` больше не проходит молча | `comparePool` (`scripts/kernel-validate.mjs`) строит `data`/`duplicateData`/`undocumented` через `String(entry?.id ?? '')` и truthy-охраняемые фильтры (`id &&`) — запись без `id` (или с пустым `id`) молча выпадает из КАЖДОЙ из этих проверок и не получает никакой собственной диагностики | `meridian_core::mechanical_integrity::operating_foundation::FoundationEntry::new` отклоняет пустой `id` на построении (валидный доменный тип с приватными полями; `ru`/`en` НЕ отклоняются — четвёртый корректирующий раунд item 2 сохраняет их доступными для сравнения bilingual-имён) — приложение (`meridian-app/src/validation/mechanical_integrity/operating_foundation.rs::resolve_entries`) резолвит каждую сырую запись независимо, считает конструктивные отказы и сообщает РОВНО ОДИН FAIL (`entry_missing_id`, с точным числом) для всей партии, прежде чем передать доменному `check_pool` только успешно построенные записи — отказавшая запись никогда не достигает `check_pool` под видом валидного значения | Rust: `meridian-core/src/mechanical_integrity/operating_foundation.rs::tests::foundation_entry_rejects_empty_id`, `::entry_missing_id_names_the_label_and_count`; `meridian-app/src/validation/mechanical_integrity/operating_foundation.rs::tests::an_entry_with_no_id_is_its_own_reported_defect_not_silently_dropped` (остальные, корректные записи по-прежнему проверяются как обычно); реальный Node/Rust на ОДНОМ И ТОМ ЖЕ мутированном рабочем дереве (полная копия репозитория, `copyRepoWithoutGitOrTarget`, плюс одна добавленная запись `terms[]` без `id` в `standards/workspace/operating-foundation.yaml`): `test/conformance-harness.test.mjs`, блок «operating-foundation: entry без id» — Node не выводит ни один FAIL про эту запись (и ни одна другая term/principle запись не расходится), Rust CLI сообщает РОВНО ОДИН `FAIL … 1 term entry(ies) ... have no id` |

## Завендоренные skills

| Skill | Состояние | Пин |
|---|---|---|
| `versioning-standard-docs` | `vendored` | SHA артефакта + SHA исходного архива и записи в нём |
| `bugfix-protocol` | `vendored-derived` | SHA производного артефакта + отдельно SHA исходного, со списком трансформаций |

`vendored-derived` означает, что в Kernel лежит не дословная копия: из артефакта
удалены продуктовые данные. Оба дайджеста записаны раздельно, и ни один не
выдаётся за другой. Смена пина — изменение Kernel и требует записи в CHANGELOG.

## Чего этот контракт намеренно не покрывает

- Продуктовые релизы и их CI — вне области Kernel.
- Область действия завендоренного стандарта версионирования документов
  (`skills/versioning-standard-docs/`) ограничена документами в каноническом
  wiki. Версионирование Git-артефактов Kernel — отдельный предмет; он описан
  двумя стандартами:
  [`standards/workspace/version-control-flow.md`](standards/workspace/version-control-flow.md)
  (поток веток: постоянные линии `main` / `dev` — до завершения миграции
  физически `master` / `develop`; обычные ветки `feature` / `bugfix` / `chore` /
  `docs` / `refactor` / `test` / `ci` / `build` и специальные `release` /
  `hotfix` / `promotion`) и
  [`standards/workspace/release-versioning.md`](standards/workspace/release-versioning.md)
  (SemVer-линия Kernel, `VERSION`, Keep a Changelog, тег `vX.Y.Z`). Стандарты
  задают режим продвижения **репозитория**: `semver-release` (Kernel —
  repository-level `VERSION`, тег `vX.Y.Z`) и `revision-promotion` (репозиторий
  без решения о SemVer, включая Instance — верхнеуровневое состояние
  идентифицируется Git SHA advancement commit, repository-level `VERSION`/тег не
  создаются). Вложенные независимо версионируемые единицы (`stack-profiles/`,
  smoke-пакет) режим репозитория не переопределяет — они ведут свои
  `VERSION`/`CHANGELOG`/теги сами. Правила выбора MAJOR/MINOR/PATCH обоих
  стандартов ссылаются на ломающие изменения в терминах этого контракта, не
  переопределяя его.
