---
title: Квалификация бизнес-контракта Rust Meridian
document_type: report
status: current
scope: workspace
owner: workspace-owner
created: 2026-09-25
updated: 2026-09-25
related_documents:
  - $MERIDIAN_KERNEL/governance/plans/meridian-rust-migration-program-plan.md
  - $MERIDIAN_KERNEL/standards/workspace/rust-migration-quality.md
  - $MERIDIAN_KERNEL/governance/decisions/meridian-rust-sqlite-architecture.md
  - $MERIDIAN_KERNEL/governance/specifications/meridian-rust-target-architecture.md
  - $MERIDIAN_KERNEL/governance/rfcs/meridian-cli-rfc.md
  - $MERIDIAN_KERNEL/standards/workspace/instance-data-migration.md
  - $MERIDIAN_KERNEL/COMPATIBILITY.md
---

# Квалификация бизнес-контракта Rust Meridian

Пакет `rust-business-contract-qualification` (пакет 9,
`governance/plans/meridian-rust-migration-program-plan.md` §5.23). Отчёт
исполнителя; итоговый вердикт выносит независимый архитектор после чтения
фактического diff и исполняемых доказательств.

## 1. Итог

**Итоговый вердикт архитектора: `QUALIFIED`; пакет `ACCEPTED`.**

Первая передача нашла пятнадцать пробелов. Восемь закрыты в ней (§9.1).
Корректирующий раунд по вердикту архитектора `CHANGES_REQUESTED` закрыл
ещё шесть (§9.2): GAP-10…GAP-13 классифицированы архитектором как
`ACCEPTED_RUST_NATIVE` и получили строки `COMPATIBILITY.md` с точными
Node/Rust-доказательствами, GAP-14 исправлен (`doctor` проверяет Git), а
GAP-15 снят: переходное Node-tooling не входит в квалифицируемую
Rust-поверхность (§8.5). GAP-02 доисправлен для относительного `--kernel`,
а закреплённый счётчик `agent_instruction_identity_undeclared_other`
больше не требует правки после индексации отчёта (§9.2).

Открытым остаётся GAP-09. Mapping каждой Instance-проверки Node (§9.3)
показывает непустую третью категорию — действительно утраченные
бизнес-проверки продуктового состояния, которые не доказаны ни импортом, ни
SQLite: продуктовые литералы `kernel-purity`, форма payload импортированных
продуктовых записей, контекст Instance для скиллов, внешние зависимости,
сверка инвентаря с репозиториями, объявления stack-profile, ссылочная
целостность и полнота приёма инструкций, журнал метрик прогона. Поэтому по
§5.23.3 вердикт `NOT_QUALIFIED`; недостающий контракт и production-владелец
названы в §9.3.

Корректирующий пакет `rust-workspace-state-validation` (§9.5) реализует
все category-3 проверки mapping §9.3 и три проверки решения D3 штатным
маршрутом `meridian validate --workspace-db`; решения D1/D2 зафиксированы.
Повторный полный рубеж §5.23.7 пройден на неизменённом fingerprint
`0a54175ff8b02a094afcd2bf5e9c24c79eedb896a84862aa193f67e452646671`:
1162 workspace Rust tests, 2/2 ignored real-bundle tests, 152/152 cases
conformance harness и 293/293 `kernel-validate.test.mjs`; остальные ворота
также зелёные. GAP-09 закрыт независимой приёмкой.

Итог закрытой матрицы (§4–§8 и построчный аудит `COMPATIBILITY.md` §10),
136 строк:

| Статус | Строк |
|---|---:|
| `CONFORMANT` | 53 |
| `ACCEPTED_RUST_NATIVE` | 41 |
| `RUST_ONLY_CONTRACT` | 42 |
| `GAP` | 0 |
| **Итого** | **136** |

Три прежние строки `GAP` — VAL-30, NODE-02 и NODE-03 — переведены в
`ACCEPTED_RUST_NATIVE`; GAP-09 закрыт. По сравнению с первой передачей
(131 строка):
четыре новые строки реестра COMPAT-29…32; строка NODE-07 удалена из матрицы
— переходное Node-tooling зафиксировано отдельной границей §8.5 без
статуса матрицы. Корректирующий пакет `rust-workspace-state-validation`
(§9.5) добавил две строки реестра COMPAT-33/34 (решение D2 и fail-closed
порты `instruction-intake`); его новые cases исполнены на финальном рубеже.

## 2. База и метод

- Рабочее дерево: отдельный рабочий каталог Kernel на отсоединённом `HEAD`,
  равном `dev`. `HEAD` до работы и после:
  `f00b60d15f75cc88d88b6bd2239e62a1bcb8e893` (merge спецификации пакета 9);
  достижимы package commit пакета 8
  `6a68620ff7af4efde492dab897d566e7763f410b`, его merge
  `e85a73fec4bb7eb6b0ac0497bc043a9d2c445c03` и commit спецификации
  `f5eaeebbe9b303d088290c0a6097587dc053f74c`. Дерево до работы чистое,
  индекс пуст.
- Инвентаризация построена от production entry points, а не от списка
  тестов: `meridian-cli/src/main.rs` → `meridian_cli::run`
  (`meridian-cli/src/lib.rs`) → семь команд (`init`, `doctor`, `validate`,
  `resolve`, `export`, `import`, `migration plan|apply|verify|rollback`);
  `commands::validate::collect_with_git_result` → 28 семейств и фиксированные
  Instance-советы; публичные операции `meridian-app` и порты; схема и порты
  `meridian-storage-sqlite`; каждая строка реестра Rust-native усилений
  `COMPATIBILITY.md`; каждый Node entry point `scripts/*.mjs`, `hooks/`,
  `.github/workflows/gate.yml`.
- Каждый контракт затем сверен с исполняемыми доказательствами; где их не
  было, построено минимальное воспроизведение на реальных Node и Rust
  (копия дерева без `.git`/`target`, прямой запуск `scripts/kernel-validate.mjs`
  и собранного `meridian`) и либо добавлен real-path case, либо зафиксирован
  пробел.
- Путь к замороженному источнику — только локальная проводка окружения; в
  файлах репозитория его нет.
- Корректирующий раунд выполнен в том же рабочем каталоге, `HEAD` до и после
  — тот же `f00b60d15f75cc88d88b6bd2239e62a1bcb8e893`; индекс пуст.

### 2.1. Обозначения ссылок на тесты

Все ссылки — точные имена исполняемых тестов. Сокращения путей:

| Метка | Файл |
|---|---|
| `[H]` | `test/conformance-harness.test.mjs` (массив мутаций и `id`, либо заголовок `check`) |
| `[HF]` | `verification/conformance-harness/fixtures/conformance-harness.fixtures.json` (имя case; исполняется `[H]`) |
| `[BR]` | `meridian-cli/tests/binary_runs.rs` |
| `[BRM]` | `meridian-cli/tests/binary_runs/migration.rs` (тест `migration::<имя>`) |
| `[CLI]` | `meridian-cli/src/lib.rs`, модуль `tests` |
| `[SM]` | `meridian-storage-sqlite/tests/schema_migration.rs` |
| `[SA]` | `meridian-storage-sqlite/tests/sqlite_storage_adapter.rs` |
| `[MR]` | `meridian-storage-sqlite/tests/migration_runs.rs` |
| `[RA]` | `meridian-core/tests/resolver_acceptance.rs` |
| `[RR]` | `meridian-core/tests/resolver_route_reference.rs` |
| `[JC]` | `meridian-core/tests/json_canonical_reference.rs` |
| `[FP]` | `meridian-core/tests/migration_fingerprint_reference.rs` |
| `app:` | `meridian-app/src/<модуль>` — путь теста модуля `meridian_app` |
| `core:` | `meridian-core/src/<модуль>` — путь теста модуля `meridian_core` |
| `cli:` | `meridian-cli/src/<модуль>` — путь теста модуля `meridian_cli` |

Мутации `[H]` исполняются на полной копии дерева; Node-сторона — реальный
`scripts/kernel-validate.mjs`, Rust-сторона — реальный собранный `meridian`
через `meridian-cli/examples/validate_cli_producer.rs`; сравниваются код
завершения и полные мультимножества `FAIL`/`WARN`.

## 3. Статусы

Ровно один статус на строку (§5.23.3): `CONFORMANT`, `ACCEPTED_RUST_NATIVE`,
`RUST_ONLY_CONTRACT`, `GAP`. Отношение Node/Rust: **равно** — одинаковые
код и мультимножество диагностик либо одинаковый нормализованный результат;
**вердикт равен** — одинаковый исход при принятой различающейся детали;
**нет аналога** — у Node нет соответствующей поверхности.

## 4. Адаптеры исходных форматов

| ID | Канон | Node-эталон | Rust-маршрут (владелец) | Positive | Negative | Отношение | Факт | Статус |
|---|---|---|---|---|---|---|---|---|
| SF-01 | Подмножество YAML: принимаемые конструкции и значения (RFC D-A) | `scripts/lib/yaml.mjs` `yamlParse` | `meridian_app::source_format::yaml::parse` (app) | `[HF] real-node-rust-source-format-adapters` (4 принимаемых случая корпуса); `app: source_format::yaml::tests::parses_block_and_flow_subset`, `…::implicit_extensions_remain_strings` | `[HF] real-node-rust-source-format-adapters` (5 `reject-*`); `app: source_format::yaml::tests::rejects_extensions` | равно | равно | `CONFORMANT` |
| SF-02 | Текст отказа строгого слоя YAML (strict-lint до библиотеки, D-A) | тот же | тот же; исправлен порядок (GAP-03) | `app: source_format::yaml::tests::parses_block_and_flow_subset` | `[H] VALIDATE_MUTATION_FAMILIES` `registry-yaml-strict-lint-rejections` (5 `reject-*` корпуса + 2 flow-случая); `app: source_format::yaml::tests::the_strict_line_lint_names_the_rejection_before_the_library_does` | равно | равно после исправления | `CONFORMANT` |
| SF-03 | Текст, который подмножество Node принимает, хотя это не YAML | тот же (молча отбрасывает часть содержимого) | тот же: вторая проверка библиотекой | `app: source_format::yaml::tests::parses_block_and_flow_subset`; контрольный документ в `[H]` «accepted-rust-native: GAP-12 lossy YAML is rejected fail-closed» | `app: source_format::yaml::tests::text_that_is_not_yaml_is_still_rejected_by_the_library`; `[H]` «accepted-rust-native: GAP-12 lossy YAML is rejected fail-closed» | вердикт различается (принято, COMPAT-31) | Rust fail-closed, Node теряет данные | `ACCEPTED_RUST_NATIVE` |
| SF-04 | Строгость JSON Schema: allowlist ключей и форматов, запрет внешних `$ref`, обход непосещаемых веток | `scripts/lib/json-schema.mjs` `assertSupportedDeep` | `meridian_app::source_format::json_schema::{assert_supported_deep, validate}` (app); место — `meridian-cli` `commands::validate::registry_schema` (GAP-02) | `[HF] real-node-rust-source-format-adapters` (8 принимаемых schema-случаев); `app: source_format::json_schema::tests::validates_supported_schema_with_reference_diagnostics` | `[HF]` тот же (`reject-unknown-keyword-in-optional-branch`, `reject-unknown-format`, `reject-external-reference`); `[H] VALIDATE_MUTATION_FAMILIES` `registry-schema-unsupported-keyword-and-format`; `app: source_format::json_schema::tests::rejects_unsupported_keywords_in_unvisited_branches`, `…::rejects_unknown_formats_and_external_references`; `cli: commands::validate::registry_schema::tests::an_unsupported_schema_construct_is_located_under_the_schema_file` | равно | равно после исправления | `CONFORMANT` |
| SF-05 | Диагностики валидации JSON Schema (набор и текст ошибок) | `json-schema.mjs` `validate` | `json_schema::validate` (app) | `[HF] real-node-rust-source-format-adapters` (списки ошибок сравниваются) | `[H] VALIDATE_MUTATION_FAMILIES` `registry-schema`; `[BR] validate_detects_a_registry_document_that_violates_its_declared_schema` | равно | равно | `CONFORMANT` |
| SF-06 | Размеченные области и fenced-блоки | `scripts/lib/regions.mjs` | `meridian_app::source_format::regions` (app) | `[HF] real-node-rust-regions-adapters` (11 случаев); `app: source_format::regions::tests::marked_region_reads_exactly_the_body_between_its_markers` | `app: source_format::regions::tests::marked_region_rejects_a_missing_marker`, `…::marked_region_rejects_a_duplicated_begin_marker`; `[H] VALIDATE_MUTATION_FAMILIES_7A_ADVERSARIAL` `stack-profiles-duplicated-pool-region`, `operating-foundation-missing-pool-region` | равно | равно | `CONFORMANT` |
| SF-07 | Блок Front Matter (`text.slice(4, indexOf('\n---', 3))`) | `kernel-validate.mjs` (document-identity, agent-instruction-identity) | `meridian_app::source_format::front_matter_block` (app; новый единственный владелец, GAP-01) | `app: source_format::markdown_identity::tests::the_front_matter_block_is_the_text_between_the_two_markers` | `app: source_format::markdown_identity::tests::an_empty_block_is_empty_not_a_reversed_range`, `…::a_multi_byte_character_after_the_opening_marker_is_not_split`, `…::a_missing_opening_or_closing_marker_yields_an_empty_block`; `[H] VALIDATE_MUTATION_FAMILIES` `document-identity-empty-front-matter` | равно | равно после исправления | `CONFORMANT` |
| SF-08 | Каноническая сериализация JSON (порядок ключей, числа, суррогаты) | `JSON.stringify` + `canonicalize` | `meridian_core::json`, `meridian_core::canonical` (core) | `[JC]` 23 `accepts_*` и 5 `key_order_*`; `meridian-core/tests/json_number_corpus_reference.rs::every_corpus_entry_is_accepted_as_canonical_json_through_the_public_api` | `[JC]` 13 `rejects_*` (например `rejects_a_duplicate_key_matching_the_node_reference`) | равно (ожидания получены реальным Node) | равно | `CONFORMANT` |
| SF-09 | Хвост сообщения о невалидном JSON схемы/фикстур | `e.message` V8 (49 мест `kernel-validate.mjs`) | текст `serde_json`; маршруты — COMPAT-29 | `[HF] real-node-rust-cli-validate-clean-kernel` | `[H]` «accepted-rust-native: GAP-10 invalid-JSON tail: combined routes» и пять изолированных «accepted-rust-native: GAP-10 invalid-JSON tail: <фикстуры> fixtures» (10 пар `FAIL`) | вердикт и авторский префикс равны (принято, COMPAT-29) | различается только хвост | `ACCEPTED_RUST_NATIVE` |

## 5. `meridian validate`

Общий маршрут: `meridian_cli::commands::validate::run` →
`collect_with_git_result` → адаптеры `meridian-cli` (`FsWorkspaceReader`,
`RealGitInspector`/`CachedGitInspector`, `FsLinkTarget`) → операции
`meridian_app` → проверки `meridian_core` → единственная точка представления
(`with_family_prefix`, `split_diagnostics`). Общий positive для всех строк —
`[HF] real-node-rust-cli-validate-clean-kernel` (Node и Rust: код 0, равные
`FAIL`/`WARN`) и `[BR] validate_reports_ok_on_this_kernel_with_zero_real_failures`.
В столбце Positive ниже — дополнительный тест семейства.

| ID | Семейство | Node-эталон | Rust (владелец алгоритма) | Positive | Negative | Отношение | Факт | Статус |
|---|---|---|---|---|---|---|---|---|
| VAL-00 | Конверт `validate`: вердикт, `FAIL`/`WARN`, код 0/1 | `scripts/kernel-validate.mjs` | `commands::validate::run` (cli, презентация) | `[CLI] tests::swapping_the_event_sink_does_not_change_stdout_stderr_or_exit_code` | `[H]` все мутации ниже (код 1 на обеих сторонах) | равно | равно | `CONFORMANT` |
| VAL-01 | `kernel-purity` (личные пути) | тот же | `commands::validate::kernel_purity` (cli) | `[H]` «реальный Node/Rust validate на копии текущего дерева без .git совпадает (без мутации)» | `[H] VALIDATE_MUTATION_FAMILIES` `kernel-purity`; `[BR] validate_detects_a_personal_path_leak` | равно | равно | `CONFORMANT` |
| VAL-02 | `document-identity` | тот же | `commands::validate::document_identity` (cli) + `front_matter_block` (app) | `[BR] validate_reports_ok_on_this_kernel_with_zero_real_failures` | `[H] VALIDATE_MUTATION_FAMILIES` `document-identity`, `document-identity-empty-front-matter`; `[BR] validate_detects_a_non_kebab_case_file_name`, `validate_reports_an_empty_front_matter_block_instead_of_panicking` | равно | равно после исправления GAP-01 | `CONFORMANT` |
| VAL-03 | `duplicate-fm` | тот же | `commands::validate::duplicate_fm` (cli) | как VAL-00 | `[H] VALIDATE_MUTATION_FAMILIES` `duplicate-fm`; `[BR] validate_detects_an_orphaned_trailing_front_matter_block` | равно | равно | `CONFORMANT` |
| VAL-04 | `link` | тот же | `commands::validate::link_check` (cli) | как VAL-00 | `[H] VALIDATE_MUTATION_FAMILIES` `link-check`; `[BR] validate_detects_a_dangling_markdown_link` | равно | равно | `CONFORMANT` |
| VAL-05 | `schema` — общий проход `$schema` | тот же | `commands::validate::registry_schema` (cli) + `json_schema` (app) | `cli: commands::validate::registry_schema::tests::a_relative_schema_reference_is_resolved_like_the_reference`, `…::normalizing_a_relative_path_keeps_its_leading_parent_segments`; `[BR] validate_reports_a_relative_kernel_schema_under_its_absolute_path` (`--kernel k` и `../k`) | `[H] VALIDATE_MUTATION_FAMILIES` `registry-schema`, `registry-schema-unsupported-keyword-and-format`, `registry-yaml-strict-lint-rejections` | равно | равно после GAP-02/GAP-03 | `CONFORMANT` |
| VAL-05a | `schema`: текст «ссылка ни на что не указывает» | тот же (`…in the Instance or the Kernel`) | тот же (`…in the Kernel`) | `[BR] validate_reports_ok_on_this_kernel_with_zero_real_failures` (14/14 ссылок разрешены) | `[BR] validate_reports_a_missing_schema_as_absent_from_the_kernel_only`; `[H]` «accepted-rust-native: GAP-11 missing schema names the Kernel only» | вердикт равен (принято, COMPAT-30) | различается только место поиска в тексте | `ACCEPTED_RUST_NATIVE` |
| VAL-06 | `rule-resolution` — PHASE B схемы против фикстур | тот же | `commands::validate::rule_resolution_fixtures` (cli) + `json_schema` (app) | `[BR] validate_reports_ok_on_this_kernel_with_zero_real_failures` (20/27) | `[H] VALIDATE_MUTATION_FAMILIES_7A` `rule-resolution-fixtures` (новый, GAP-04) | равно | равно | `CONFORMANT` |
| VAL-07 | `functional-parity` | тот же + `scripts/lib` нет (inline) | `meridian_app::operating_model::functional_parity` → `meridian_core::functional_parity` | `app: operating_model::functional_parity::tests::the_real_kernel_schema_and_fixtures_agree` | `[H] VALIDATE_MUTATION_FAMILIES_7B` `functional-parity`; `app: operating_model::functional_parity::tests::evaluate_case_returns_domain_rejected_for_a_schema_clean_but_domain_invalid_document` | равно (кроме COMPAT-10/11) | равно | `CONFORMANT` |
| VAL-08 | `git-provenance` (в режиме Kernel — совет) | тот же | `commands::validate::git_provenance` (cli) | как VAL-00 | `[H]` «копия без .git» (ветвь «Git недоступен»); `cli: commands::validate::rust_architecture_conformance_3_single_git_snapshot::unavailable_snapshot_yields_exactly_one_git_enumeration_warning` | равно | равно | `CONFORMANT` |
| VAL-09 | `instance-context` (в режиме Kernel — совет) | тот же | `commands::validate::instance_context` (cli) | как VAL-00 | отрицательная ветвь существует только в режиме Instance — mapping §9.3, M-06 | равно | равно | `CONFORMANT` |
| VAL-10 | `sha-provenance` | тот же | `meridian_app::validation::mechanical_integrity::sha_provenance` → `meridian_core::mechanical_integrity::sha_provenance` | `app: validation::mechanical_integrity::sha_provenance::tests::the_real_kernel_vendored_skills_verify_cleanly` | `[H] VALIDATE_MUTATION_FAMILIES_7A` `sha-provenance`; `[H] VALIDATE_MUTATION_FAMILIES_7A_ADVERSARIAL` `sha-provenance-symlinked-skill-is-not-followed` | равно (кроме COMPAT-06/07) | равно | `CONFORMANT` |
| VAL-11 | `instruction-topics` | тот же | `…::mechanical_integrity::instruction_topics` (app/core) | `app: validation::mechanical_integrity::instruction_topics::tests::the_real_kernel_pool_agrees_with_itself` | `[H] VALIDATE_MUTATION_FAMILIES_7A` `instruction-topics`; `[H] VALIDATE_MUTATION_FAMILIES_7A_ADVERSARIAL` `instruction-topics-several-elements-out-of-order` | равно | равно | `CONFORMANT` |
| VAL-11a | `instruction-topics`: `topics` неверного типа | тот же (`FAIL` с текстом исключения движка) | тот же (авторский `FAIL`) | `app: validation::mechanical_integrity::instruction_topics::tests::the_real_kernel_pool_agrees_with_itself` | `app: validation::mechanical_integrity::instruction_topics::tests::a_wrong_type_topics_key_is_a_failure`; `[H]` «accepted-rust-native: GAP-13 instruction-topics of the wrong type» (точная форма, без `exactTextNotRequired`) | вердикт равен (принято, COMPAT-32) | Node — текст исключения движка, Rust — авторский `FAIL` | `ACCEPTED_RUST_NATIVE` |
| VAL-12 | `operating-foundation` | тот же | `…::mechanical_integrity::operating_foundation` (app/core) | `app: validation::mechanical_integrity::operating_foundation::tests::the_real_kernel_pool_agrees_with_itself` | `[H] VALIDATE_MUTATION_FAMILIES_7A` `operating-foundation`; `[H] VALIDATE_MUTATION_FAMILIES_7A_ADVERSARIAL` `operating-foundation-missing-pool-region` | равно (кроме COMPAT-08) | равно | `CONFORMANT` |
| VAL-13 | `stack-profiles` | тот же | `…::mechanical_integrity::stack_profiles` (app/core) | `app: validation::mechanical_integrity::stack_profiles::tests::agreeing_halves_produce_no_diagnostics` | `[H] VALIDATE_MUTATION_FAMILIES_7A` `stack-profiles`; `[H] VALIDATE_MUTATION_FAMILIES_7A_ADVERSARIAL` `stack-profiles-duplicated-pool-region` | равно (кроме COMPAT-28) | равно | `CONFORMANT` |
| VAL-14 | `agent-instruction-identity` | тот же | `…::mechanical_integrity::agent_instruction_identity` (app/core) | `app: validation::mechanical_integrity::agent_instruction_identity::tests::a_fully_declared_norm_is_clean` | `[H] VALIDATE_MUTATION_FAMILIES_7A` `agent-instruction-identity`; `app: …::agent_instruction_identity::tests::a_half_declared_norm_is_a_failure` | равно | равно | `CONFORMANT` |
| VAL-15 | `task-pattern-registry` | тот же + `scripts/lib/task-pattern-registry.mjs` | `meridian_app::operating_model::task_pattern_registry` → `meridian_core::task_contracts::catalog` | `app: operating_model::task_pattern_registry::tests::the_real_kernel_catalog_agrees_with_its_own_schema_and_fixtures` | `[H] VALIDATE_MUTATION_FAMILIES_7B` `task-pattern-registry`; `app: …::task_pattern_registry::tests::duplicate_pattern_id_is_reported_through_the_full_pipeline` | равно (кроме COMPAT-09) | равно | `CONFORMANT` |
| VAL-16 | `instruction-source-registry` | `scripts/lib/instruction-source-registry.mjs` | `meridian_app::operating_model::instruction_source_registry` → `meridian_core::instruction_source` | `app: operating_model::instruction_source_registry::tests::the_real_kernel_schema_and_fixtures_agree` | `[H] VALIDATE_MUTATION_FAMILIES_7B` `instruction-source-registry` | равно (кроме COMPAT-03) | равно | `CONFORMANT` |
| VAL-17 | `task-specification-contract` | `scripts/lib/task-specification.mjs` | `meridian_app::operating_model::task_specification` → `meridian_core::task_contracts::specification` | `app: operating_model::task_specification::tests::the_real_kernel_schema_and_fixtures_agree` | `[H] VALIDATE_MUTATION_FAMILIES_7B` `task-specification-contract`; `app: …::task_specification::tests::a_rooted_path_in_the_goal_is_rejected` | равно (кроме COMPAT-09) | равно | `CONFORMANT` |
| VAL-18 | `execution-state-model` | `scripts/lib/execution-state.mjs` | `meridian_app::operating_model::execution_state` → `meridian_core::run_contracts::execution_state` | `app: operating_model::execution_state::tests::the_real_kernel_contract_is_consistent_through_the_production_route` | `[H] VALIDATE_MUTATION_FAMILIES_7B` `execution-state-model` | равно (кроме COMPAT-12) | равно | `CONFORMANT` |
| VAL-19 | `role-and-human-control` | `scripts/lib/role-and-human-control.mjs` | `meridian_app::operating_model::role_and_human_control` → `meridian_core::run_contracts::{roles, human_control}` | `app: operating_model::role_and_human_control::tests::the_real_kernel_contract_is_consistent_through_the_production_route` | `[H] VALIDATE_MUTATION_FAMILIES_7B` `role-and-human-control` | равно (кроме COMPAT-12) | равно | `CONFORMANT` |
| VAL-20 | `bounded-context-manifest` | `scripts/lib/context-manifest.mjs` | `meridian_app::operating_model::bounded_context_manifest` → `meridian_core::run_contracts::context_manifest` | `app: operating_model::bounded_context_manifest::tests::the_real_kernel_contract_is_consistent_through_the_production_route` | `[H] VALIDATE_MUTATION_FAMILIES_7B` `bounded-context-manifest` | равно (кроме COMPAT-12/13) | равно | `CONFORMANT` |
| VAL-21 | `evidence-and-handoff-contract` | `scripts/lib/evidence-and-handoff.mjs` | `meridian_app::operating_model::evidence_and_handoff` → `meridian_core::evidence::handoff` | `app: operating_model::evidence_and_handoff::tests::evidence_and_handoff_the_real_kernel_contract_is_consistent_through_the_production_route` | `[H] VALIDATE_MUTATION_FAMILIES_7C` `evidence-and-handoff-contract` | равно (кроме COMPAT-13/14/15) | равно | `CONFORMANT` |
| VAL-22 | `meridian-field-evaluation` | `scripts/lib/field-evaluation.mjs` | `meridian_app::operating_model::field_evaluation` → `meridian_core::field_evaluation` | `app: operating_model::field_evaluation::tests::field_evaluation_the_real_kernel_contract_is_consistent_through_the_production_route` | `[H] VALIDATE_MUTATION_FAMILIES_7C` `meridian-field-evaluation` | равно (кроме COMPAT-13/14/15/16) | равно | `CONFORMANT` |
| VAL-23 | `controlled-rule-intake` | `scripts/lib/controlled-rule-intake.mjs` | `meridian_app::operating_model::controlled_rule_intake` → `meridian_core::controlled_rule_intake` | `cli: commands::validate::controlled_rule_intake::tests::the_real_kernel_schema_and_fixtures_agree` | `[H] VALIDATE_MUTATION_FAMILIES_7C` `controlled-rule-intake` | равно (кроме COMPAT-01/02) | равно | `CONFORMANT` |
| VAL-24 | `existing-project-compatibility-mode` | `scripts/lib/existing-project-compatibility-mode.mjs` | `meridian_app::operating_model::existing_project_compatibility_mode` → `meridian_core::existing_project_compatibility_mode` | `app: operating_model::existing_project_compatibility_mode::tests::the_real_kernel_schema_and_fixtures_agree` | `[H] VALIDATE_MUTATION_FAMILIES_7C` `existing-project-compatibility-mode` | равно (кроме COMPAT-04/05) | равно | `CONFORMANT` |
| VAL-25 | `instance-data-migration` | `scripts/lib/instance-data-migration.mjs` | `meridian_app::operating_model::instance_data_migration` → `meridian_core::migration::plan` | `app: operating_model::instance_data_migration::tests::instance_data_migration_the_real_kernel_contract_is_clean_through_the_real_adapter` | `[H] VALIDATE_MUTATION_FAMILIES_7D` `instance-data-migration` | равно (кроме COMPAT-17/18/19/24) | равно | `CONFORMANT` |
| VAL-26 | `instance-canonical-export` | тот же | `meridian_app::operating_model::instance_canonical_export` → `meridian_core::migration::export` | `app: operating_model::instance_canonical_export::tests::instance_canonical_export_the_real_kernel_contract_is_clean_through_the_real_adapter` | `[H] VALIDATE_MUTATION_FAMILIES_7D` `instance-canonical-export`; `[H] ADVERSARIAL_7D` `scope-mismatch` | равно (кроме COMPAT-17/18/24) | равно | `CONFORMANT` |
| VAL-27 | `workspace-compatibility-qualification` | `scripts/lib/workspace-compatibility-qualification.mjs` | `meridian_app::operating_model::workspace_compatibility_qualification` → `meridian_core::qualification::workspace` | `app: operating_model::workspace_compatibility_qualification::tests::workspace_compatibility_qualification_the_real_kernel_contract_is_clean` | `[H] VALIDATE_MUTATION_FAMILIES_7D` `workspace-compatibility-qualification`; `[H] ADVERSARIAL_7D` `stale-pin` | равно (кроме COMPAT-17/18/21/22) | равно | `CONFORMANT` |
| VAL-28 | `upgrade-integration-qualification` | `scripts/lib/upgrade-integration-qualification.mjs` | `meridian_app::operating_model::upgrade_integration_qualification` → `meridian_core::qualification::upgrade` | `app: operating_model::upgrade_integration_qualification::tests::upgrade_integration_qualification_the_real_kernel_contract_is_clean` | `[H] VALIDATE_MUTATION_FAMILIES_7D` `upgrade-integration-qualification`; `[H] ADVERSARIAL_7D` `wrong-kind`, `incomplete-scenario-set` | равно (кроме COMPAT-17/18/21/23) | равно | `CONFORMANT` |
| VAL-29 | Советы режима без Instance (`front-matter/path-placement`, `ext-dependencies`, `inventory-git`, `stack-profile`, `instruction-intake`, `kernel-purity` UNVERIFIED) | тот же без `MERIDIAN_INSTANCE` | `commands::validate::collect_with_git_result` (cli, фиксированные строки) | `[HF] real-node-rust-cli-validate-clean-kernel` (равные `WARN`) | нет отдельной ветви в режиме Kernel | равно | равно | `CONFORMANT` |
| VAL-30 | Проверка Instance: продуктовые литералы, рабочая память, `instruction-intake`, `inventory-git`, `ext-dependencies`, объявления stack-profile, Git Instance | `kernel-validate.mjs` с `MERIDIAN_INSTANCE` | `meridian validate --workspace-db` (§9.5) | см. §9.5 | mapping §9.3 (M-01…M-18); реализация §9.5 | категория 3 и D3 реализованы; D1/D2 решены архитектором; новые cases исполнены на финальном рубеже §5.23.7 | Node: 10 `FAIL` на замороженном источнике; Rust: `validate --workspace-db` (§9.5), real-bundle и COMPAT-33 cases совпадают по вердикту, различия классифицированы | `ACCEPTED_RUST_NATIVE` |
| VAL-31 | Структурная граница семейств (порты, один `WorkspaceReader`, тонкие CLI-модули) | нет аналога | `commands::validate::mechanical_integrity_boundary`, `rust_architecture_conformance_{3,5,6,7}` (cli, структурные тесты) | `cli: commands::validate::mechanical_integrity_boundary::structural_tests::production_has_exactly_one_concrete_workspace_reader_implementation` | `cli: commands::validate::mechanical_integrity_boundary::structural_tests::only_an_explicit_test_only_module_file_is_exempt_from_the_production_scan` | нет аналога | исполнено | `RUST_ONLY_CONTRACT` |

## 6. `meridian resolve`

| ID | Канон | Node-эталон / основание | Rust-маршрут (владелец) | Positive | Negative | Отношение | Факт | Статус |
|---|---|---|---|---|---|---|---|---|
| RES-01 | Resolver: `work item + repository + paths + profiles + applicability → нормы, протоколы, verification, blockers, reresolution` (контракт видения §8–§9) | `scripts/rule-resolver.mjs` `resolveRules` | `meridian_app::rule_resolution::resolve` → `meridian_core::resolver` | `[HF] real-node-rust-rule-resolution` и `real-node-rust-cli-resolve` (6 случаев); `[RA] universal_scope_always_applies`, `task_class_activation_matches_work_kind_and_change_class`, `changed_paths_outside_candidate_paths_forces_reresolution` | `[RA] profile_scope_no_match_for_a_different_profile`, `explicit_activation_never_activates_and_is_not_unresolved`, `two_incompatible_mandatory_routes_produce_a_conflict_not_a_silent_pick` | равно | равно | `CONFORMANT` |
| RES-02 | Fail-closed ветви и неразрешённое состояние (контракт видения §10) | `resolveRules` (`ResolverError`) | тот же | `[HF] real-node-rust-rule-resolution-fail-closed` (новый; 3 принимаемых: `valid-supersedes-picks-the-declared-head`, `undetermined-activation-is-reported-unresolved`, `unresolved-status-carries-its-resume-condition`) | тот же case: 7 отказов (unknown repository, stale/missing norm text, same-date без supersedes, cycle, self, dangling); `[RA] unknown_repository_id_fails_closed_with_no_fallback`, `stale_digest_fails_closed`, `a_supersedes_cycle_fails_closed` | равно (включая тексты причин) | равно | `CONFORMANT` |
| RES-03 | Детерминированный выбор маршрута протокола | `resolveRules` | тот же | `[RR] route_tiebreak_picks_the_same_winner_as_the_node_reference` | `[RR] tiebreak_key_cannot_be_mismatched_to_a_different_routes_fields`, `mandatory_conflict_count_and_exclusion_match_the_node_reference` | равно | равно | `CONFORMANT` |
| RES-04 | Транспорт `meridian resolve`: один самодостаточный JSON-запрос, закрытый DTO, схема applicability, схема результата | Node CLI читает инвентарь, реестры и тексты норм через `$MERIDIAN_INSTANCE` — запрещено §6.5a.5; основание: техническая спецификация §7.1 | `commands::resolve` (cli) → `rule_resolution::resolve` (app) | `[BR] resolve_reads_a_request_file_and_prints_json_to_stdout_only`, `resolve_reads_from_stdin_when_no_request_flag_is_given`; `app: rule_resolution::tests::composes_transport_sources_and_returns_core_output` | `[BR] resolve_rejects_unknown_transport_fields`, `resolve_rejects_malformed_json_with_environment_exit_code_and_empty_stdout`; `app: rule_resolution::tests::malformed_applicability_fails_before_core_resolution` | нет аналога | исполнено | `RUST_ONLY_CONTRACT` |
| RES-05 | Коды `resolve`: 0 (включая неразрешённое), 2 usage, 3 отклонённый запрос | основание: спецификация §7.1 | `commands::resolve` (cli) | `[BR] resolve_reads_a_request_file_and_prints_json_to_stdout_only` | `[BR] resolve_distinguishes_a_usage_error_from_a_rejected_request_by_exit_code`; `meridian-cli/tests/resolve_cli_producer_runs.rs::the_rejected_branch_prints_the_real_exit_code_and_a_non_empty_stderr_detail` | нет аналога | исполнено | `RUST_ONLY_CONTRACT` |

## 7. `init`, `doctor`, `export`, `import`, `migration`

| ID | Канон | Node-эталон / основание | Rust-маршрут (владелец) | Positive | Negative | Отношение | Факт | Статус |
|---|---|---|---|---|---|---|---|---|
| INIT-01 | `init`: копия `instance-template/` файл-в-файл, две базы с явной ролью и редакцией | основание: спецификация §7, §7.1; RFC «Критерии успеха» | `commands::init` + `kernel::copy_instance_template` (cli) → `SqliteStorage` (storage) | `[BR] init_on_an_empty_directory_copies_the_template_file_for_file_and_creates_two_databases` | `[BR] repeat_init_is_idempotent_and_never_overwrites_an_existing_file`, `init_rolls_back_completely_when_the_template_copy_fails_partway_through`, `init_rolls_back_a_partial_file_left_by_a_real_write_failure_mid_copy`, `init_rolls_back_completely_when_the_tool_database_cannot_be_created`, `init_rolls_back_completely_when_the_workspace_database_fails_after_a_successful_template_copy`, `init_fails_without_partial_state_when_the_workspace_database_cannot_be_created` | нет аналога | исполнено | `RUST_ONLY_CONTRACT` |
| DOC-01 | `doctor`: Kernel, рабочий каталог, роль/редакция/схема двух баз, только чтение | основание: спецификация §7, §7.1 | `commands::doctor` (cli) → `kernel::{read_kernel_edition, check_kernel_under_git}` (cli), `SqliteStorage::open_existing` (storage) | `[BR] doctor_reports_healthy_after_a_successful_init` | `[BR] doctor_rejects_swapped_database_roles_instead_of_guessing_from_the_path`, `doctor_reports_a_corrupt_database_file_explicitly`, `doctor_reports_a_missing_workspace_directory`, `doctor_reports_unhealthy_when_schema_version_cannot_be_read` | нет аналога | исполнено | `RUST_ONLY_CONTRACT` |
| DOC-02 | `doctor` как эквивалент `preflight`: Kernel под Git (критерий `preflight` — запись `.git` существует); `validator present` не переносится — артефакт Node | `scripts/preflight.mjs` «kernel is under Git» | `commands::doctor` → `kernel::check_kernel_under_git` (cli, только чтение, без процесса `git`); поле `kernel_git`, строка `Kernel Git:` | `[BR] doctor_confirms_the_kernel_is_under_git`; тот же тест `doctor_reports_a_kernel_that_is_not_under_git` после записи `gitdir:`-файла | `[BR] doctor_reports_a_kernel_that_is_not_under_git` (код 1, один JSON-документ, пустой stderr, ничего не создано) | равно (критерий Git) | равно после исправления GAP-14 | `CONFORMANT` |
| EXP-01 | `export`: канонические записи обеих баз по ролям, без артефактов SQLite, детерминированно | основание: спецификация §4.2, §7; `instance-data-migration.md` §10 | `commands::export` (cli) → `SqliteStorage::canonical_export` (storage) | `[BR] export_produces_an_empty_deterministic_array_right_after_init`; `[SA] canonical_export_is_byte_identical_across_repeated_calls_on_unchanged_state` | `[BR] export_reports_a_missing_database_as_an_environment_error`; `cli: commands::export::tests::merge_orders_deterministically_regardless_of_input_order`; `[BRM] migration::migration_databases_fail_closed_on_missing_corrupt_wrong_role_or_edition` | нет аналога | исполнено | `RUST_ONLY_CONTRACT` |
| IMP-01 | Принятие замороженного bundle принятыми операциями 7d | `evaluateInstanceDataMigration` на том же bundle | `meridian_app::migration::bundle` → `operating_model::{instance_data_migration, instance_canonical_export}` | `[H]` «meridian-cli-migration: Node-эталон принимает реальный bundle …, Rust `migration plan` принимает тот же bundle …»; `[BRM] migration::migration_plan_accepts_the_bundle_through_the_accepted_operations` | `[BRM] migration::migration_plan_fails_closed_on_every_foreign_source_state` | равно | равно | `CONFORMANT` |
| IMP-02 | `import --kind frozen-instance` = `plan → apply → verify`; 336 записей только в `workspace`, 10 удержаны | основание: план §5.22.2–§5.22.4; `instance-data-migration.md` | `commands::import` (cli, `GitFrozenSource`) → `meridian_app::migration::operations::import_frozen` → `MigrationRepository` (storage) | `[BRM] migration::migration_import_frozen_is_plan_apply_verify_in_one_route`; `[BRM] migration::migration_real_bundle_imports_336_records_verifies_61_norms_and_rolls_back` (ignored, исполнен отдельно) | `[BRM] migration::migration_import_frozen_reports_a_wrong_confirmation_before_touching_any_database`, `migration_dry_run_and_wrong_or_missing_confirmation_change_nothing` | нет аналога | исполнено | `RUST_ONLY_CONTRACT` |
| IMP-03 | `import --kind canonical-records`: только полный конверт `export`, маршрут по области, компенсация двух баз | основание: план §5.22.2, §5.22.4 п. 11 | `commands::import` (cli) → `operations::import_canonical` (app) | `[BRM] migration::migration_round_trip_import_export_import_export_is_byte_identical`; `app: migration::operations::tests::migration_canonical_import_routes_each_record_by_its_scope` | `[BRM] migration::migration_canonical_import_accepts_only_a_complete_export_envelope`; `app: migration::operations::tests::migration_canonical_import_restores_the_tool_database_when_the_workspace_refuses`, `…::migration_canonical_import_a_tool_refusal_never_touches_the_workspace` | нет аналога | исполнено | `RUST_ONLY_CONTRACT` |
| IMP-04 | Каждая импортированная запись после чтения из SQLite равна принятому Node-экспорту по девяти полям | принятый `canonical-export.json` bundle | `export` после `import` | `[H]` «meridian-cli-migration: каждая запись, прочитанная из SQLite через Rust `export`, равна записи принятого Node-экспорта …»; `[BRM] migration::migration_real_bundle_every_read_back_record_equals_its_accepted_target` (ignored, исполнен) | `[BRM] migration::migration_plan_fails_closed_on_every_foreign_source_state` (вариант `content-mismatch`) | равно | равно (336/336) | `CONFORMANT` |
| IMP-05 | Восстановление содержимого единиц | `verify.mjs` (продуктовая таблица) | `meridian_app::migration::fragment` | см. COMPAT-27 | см. COMPAT-27 | вердикт равен | равно (336 байт-в-байт) | `ACCEPTED_RUST_NATIVE` |
| IMP-06 | Эквивалентность 61 нормы применимости (§6.5a.2) | `verify.mjs` | `meridian_core::migration::applicability` | см. COMPAT-26 | см. COMPAT-26 | вердикт равен | lost/added/changed/duplicate = 0 | `ACCEPTED_RUST_NATIVE` |
| MIG-01 | `migration plan`: чистая функция, пересчёт fingerprint, не пишет | основание: спецификация §7; план §5.22.3 | `commands::migration` (cli) → `operations::plan` (app) → `meridian_core::migration::plan` | `[BRM] migration::migration_plan_accepts_the_bundle_through_the_accepted_operations`; `[FP] plan_fingerprint_matches_the_node_reference_for_a_migrated_mapping` | `[BRM] migration::migration_plan_fails_closed_on_every_foreign_source_state` | нет аналога (fingerprint — равно Node) | исполнено | `RUST_ONLY_CONTRACT` |
| MIG-02 | `migration apply`: dry-run по умолчанию, точное подтверждение, одна транзакция, `AlreadyApplied` | основание: план §5.22.3 п. 3, §5.22.4 п. 9–10 | `operations::apply` (app) → `SqliteStorage` `MigrationRepository` (storage) | `[BRM] migration::migration_apply_verify_and_every_repeat_are_idempotent`; `[MR] migration_apply_writes_every_record_and_the_journal_row_together` | `[BRM] migration::migration_dry_run_and_wrong_or_missing_confirmation_change_nothing`; `[MR] migration_a_failure_in_the_middle_of_the_batch_leaves_the_exact_pre_state`, `migration_a_repeated_apply_of_the_same_key_writes_nothing` | нет аналога | исполнено | `RUST_ONLY_CONTRACT` |
| MIG-03 | `migration verify`: missing/extra/changed/duplicate + применимость | основание: план §5.22.3 | `operations::verify` (app) → `meridian_core::migration::write_set::verify_read_back` | `[BRM] migration::migration_apply_verify_and_every_repeat_are_idempotent` | `app: migration::operations::tests::migration_verify_empty_applicability_sides_never_verify`; `core: migration::applicability::tests::migration_applicability_detects_lost_added_changed_and_duplicate` | нет аналога | исполнено | `RUST_ONLY_CONTRACT` |
| MIG-04 | `migration rollback`: только самый новый applied run, проверенный checkpoint, append-only журнал | основание: план §5.22.5 | `operations::rollback` (app) → `SqliteStorage` (storage) | `[BRM] migration::migration_apply_then_rollback_restores_the_exact_pre_state`; `[MR] migration_rollback_restores_the_exact_pre_state_and_keeps_both_journal_facts` | `[BRM] migration::migration_rollback_over_a_newer_revision_or_a_damaged_checkpoint_changes_nothing`; `[MR] migration_a_second_rollback_is_refused_without_change`, `migration_rollback_of_an_older_run_after_a_newer_one_is_refused`, `migration_rollback_of_an_older_run_after_a_rolled_back_newer_one_is_refused`, `migration_rollback_refuses_unknown_runs_and_foreign_plans`, `migration_rollback_refuses_a_damaged_or_missing_checkpoint_without_change` | нет аналога | исполнено | `RUST_ONLY_CONTRACT` |
| MIG-05 | Грамматика пяти команд, закрытые `--kind`/`--dry-run` | основание: план §5.22.3 | `commands::{import, migration}` (cli) | `[BRM] migration::migration_help_lists_the_five_new_commands` | `[BRM] migration::migration_usage_errors_are_closed_and_write_no_stdout` | нет аналога | исполнено | `RUST_ONLY_CONTRACT` |
| MIG-06 | Нет эксплуатационного чтения `MERIDIAN_INSTANCE` (§6.5a.5) | основание: план §6.5a.5 | весь production | `[BRM] migration::migration_production_code_respects_crate_boundaries_and_never_reads_meridian_instance` | `[BRM] migration::migration_all_five_commands_run_without_network` и `[BR] init_doctor_validate_resolve_and_export_run_without_network` (без `MERIDIAN_INSTANCE`) | нет аналога | исполнено | `RUST_ONLY_CONTRACT` |

## 8. SQLite, CLI и ворота §6

### 8.1. SQLite

| ID | Канон (спецификация §4) | Rust (владелец) | Positive | Negative | Факт | Статус |
|---|---|---|---|---|---|---|
| SQL-01 | Схема 1 → 2 → 3 последовательными атомарными шагами; повторное открытие не повторяет шаг | storage `schema`, `migration` | `[SM] migrates_a_populated_v1_database_to_v2_without_losing_records`, `reopening_an_already_migrated_database_is_idempotent` | `[SM] a_failed_migration_leaves_the_database_at_its_original_version`, `migration_journal_step_refuses_legacy_rows_and_keeps_version_two`, `migration_read_only_open_never_upgrades_an_older_schema` | исполнено | `RUST_ONLY_CONTRACT` |
| SQL-02 | Неизвестная будущая версия и неизвестная роль — остановка | storage | `[SA] all_ten_tables_exist_and_are_queryable_and_schema_version_is_the_one_supported_version` | `[SM] rejects_a_schema_version_beyond_supported_with_a_typed_error`, `rejects_an_unknown_database_role_with_a_typed_error`; `[SA] rejects_an_unrecognised_schema_version_with_a_typed_error` | исполнено | `RUST_ONLY_CONTRACT` |
| SQL-03 | Роль и редакция — метаданные, сверяются полностью | app `storage::{DatabaseRole, DatabaseMetadata}`, storage | `[SM] database_metadata_reports_the_kernel_edition_it_was_created_with`, `accessor_returns_metadata_actually_read_from_the_database_not_a_copy_of_the_argument` | `[SM] rejects_a_role_mismatch_on_reopen`, `rejects_a_kernel_edition_mismatch_on_reopen`, `migrating_a_populated_product_v1_database_as_tool_is_rejected`, `migrating_a_populated_built_in_methodology_v1_database_as_workspace_is_rejected` | исполнено | `RUST_ONLY_CONTRACT` |
| SQL-04 | Разделение ролей на запись на границе адаптера | storage | `[SM] accepts_a_built_in_record_written_to_a_tool_database` | `[SM] rejects_a_product_record_written_to_a_tool_database`, `rejects_a_built_in_record_written_to_a_workspace_database` | исполнено | `RUST_ONLY_CONTRACT` |
| SQL-05 | Маршрутизация `StorageRouter` по области | app `storage::role` | `app: storage::role::tests::routes_a_built_in_methodology_write_to_the_tool_storage`, `…::routes_a_product_write_to_the_workspace_storage` | `app: storage::role::tests::put_batch_rejects_a_mix_of_built_in_and_product_requests`, `…::new_rejects_a_tool_slot_carrying_the_workspace_role` | исполнено | `RUST_ONLY_CONTRACT` |
| SQL-06 | Foreign keys на каждом соединении, включая staging | storage | `[SA] foreign_keys_pragma_is_on` | `[SA] evidence_for_a_nonexistent_record_is_rejected_and_nothing_is_written`; `meridian-storage-sqlite/src/migration.rs::tests::migration_staging_enforces_foreign_keys_and_refuses_a_violating_copy` | исполнено | `RUST_ONLY_CONTRACT` |
| SQL-07 | Транзакционная запись; сбой посреди транзакции (§6.4) | storage | `[SA] put_mints_a_new_record_at_revision_one` | `[SA] a_batch_that_fails_partway_leaves_no_trace_of_its_earlier_writes`; `[MR] migration_a_failure_in_the_middle_of_the_batch_leaves_the_exact_pre_state` | исполнено | `RUST_ONLY_CONTRACT` |
| SQL-08 | Append-only редакции, доказательства и журнал | storage | `[SA] a_second_put_appends_a_revision_without_changing_the_previous_one` | `[SA] direct_update_or_delete_of_record_revisions_is_refused_by_sqlite_itself`, `direct_update_or_delete_of_evidence_is_refused_by_sqlite_itself`; `[SM] migration_journal_rows_are_append_only_at_the_sqlite_level` | исполнено | `RUST_ONLY_CONTRACT` |
| SQL-09 | Идемпотентность по ключу и конфликт при ином содержимом | storage | `[SA] an_exact_idempotent_repeat_creates_no_second_revision` | `[SA] the_same_idempotency_key_with_different_content_is_a_conflict_not_a_repeat`, `a_second_put_for_the_same_evidence_ref_is_rejected_even_with_identical_content` | исполнено | `RUST_ONLY_CONTRACT` |
| SQL-10 | Повреждённая база — явная ошибка (§6.4) | storage | `[SA] creates_and_reopens_the_same_database_without_reapplying_the_migration` | `[SA] rejects_a_corrupt_file_with_a_typed_error_instead_of_treating_it_as_empty`, `rejects_a_missing_schema_version_with_a_typed_error`; `[BR] doctor_reports_a_corrupt_database_file_explicitly` | исполнено | `RUST_ONLY_CONTRACT` |
| SQL-11 | Идентичность области не зависит от пути файла | storage, app `storage::model` | `[SA] record_identity_is_independent_of_which_database_file_backs_it`, `round_trips_a_record_in_every_one_of_the_six_scope_variants` | `[SA] two_project_workspace_records_with_the_same_type_id_and_record_id_but_different_organization_profile_id_do_not_collide`; `app: storage::model::tests::storage_key_differs_across_scope_types_with_the_same_bare_id` | исполнено | `RUST_ONLY_CONTRACT` |
| SQL-12 | Резервная копия и checkpoint | storage | `[SA] backup_produces_an_independently_openable_copy_with_the_same_exported_state`; `[MR] migration_standalone_checkpoint_restores_a_compensated_database` | `[MR] migration_a_read_only_or_locationless_storage_takes_no_checkpoint` | исполнено | `RUST_ONLY_CONTRACT` |
| SQL-13 | Канонический конверт записи без артефактов SQLite, `$schema` сохраняется | storage `codec` | `[SA] schema_ref_is_preserved_exactly_across_write_reopen_export_and_backup`; `meridian-storage-sqlite/src/codec.rs::tests::scope_round_trips_for_every_variant` | `app: storage::model::tests::payload_rejects_non_object_json`, `…::put_request_rejects_built_in_scope_with_non_built_in_origin` | исполнено | `RUST_ONLY_CONTRACT` |

### 8.2. Сквозные свойства CLI

| ID | Канон | Основание | Rust (владелец) | Positive | Negative | Факт | Статус |
|---|---|---|---|---|---|---|---|
| CLI-01 | Коды завершения 0/1/2/3 и usage | спецификация §7.1; план §5.22.3 п. 1 | `meridian_cli::run`, `exit_code` (cli) | `[CLI] tests::help_prints_usage_and_exits_ok`; `[BR] help_prints_usage_to_stdout_and_exits_ok` | `[CLI] tests::no_command_is_a_usage_error`, `unknown_command_is_a_usage_error`, `unknown_flag_is_a_usage_error`, `missing_required_argument_is_a_usage_error`, `missing_flag_value_is_a_usage_error`, `repeated_flag_is_a_usage_error`, `invalid_format_value_is_a_usage_error`; `[BR] validate_exits_with_environment_error_when_a_subdirectory_is_unreadable` | исполнено | `RUST_ONLY_CONTRACT` |
| CLI-02 | stdout — ровно один документ; ошибки и советы — только stderr; коды 2/3 без stdout | спецификация §7.1; план §5.22.3 п. 8 | `write_json_result`, `report_*_error` (cli) | `[BR] resolve_reads_a_request_file_and_prints_json_to_stdout_only` | `[BR] no_command_is_a_usage_error_on_stderr_with_empty_stdout`, `resolve_rejects_malformed_json_with_environment_exit_code_and_empty_stdout`; `[BRM] migration::migration_usage_errors_are_closed_and_write_no_stdout` | исполнено | `RUST_ONLY_CONTRACT` |
| CLI-03 | Детерминированный машинный вывод при повторном запуске | план §7 п. 9; §5.22.6 п. 14 | все команды | `[BR] export_produces_an_empty_deterministic_array_right_after_init`; `[H]` «чёрный ящик: повторный запуск публичного CLI на одном и том же входе детерминирован» | `cli: commands::export::tests::merge_orders_deterministically_regardless_of_input_order`; `[RA] output_does_not_depend_on_applicability_record_input_order` | исполнено | `RUST_ONLY_CONTRACT` |
| CLI-04 | `NoOpEventSink` и `RecordingEventSink` дают те же stdout/stderr/код и то же состояние (§6.4a, §6.4b) | план §6.4a–§6.4b; спецификация §3 | `meridian_app::events`, `meridian_cli::events` | `[CLI] tests::swapping_the_event_sink_does_not_change_stdout_stderr_or_exit_code`, `event_sink_has_no_effect_on_doctor`, `event_sink_has_no_effect_on_resolve`, `event_sink_has_no_effect_on_export`, `event_sink_has_no_effect_on_init_result_tree_or_database_metadata`; `[BRM] migration::migration_noop_and_recording_event_sinks_give_identical_results_and_state`; `app: storage::role::tests::a_disabled_event_sink_does_not_change_the_domain_result_or_stored_state` | `cli: events::tests::recording_sink_actually_records` и утверждение непустой записи в каждом тесте Positive (сравнение не пустое) | исполнено | `RUST_ONLY_CONTRACT` |
| CLI-05 | Работа без сети (§6.5) — все десять команд | план §6.5 | все команды; в графе зависимостей нет сетевого клиента, в production нет `std::net` | `[BR] init_doctor_validate_resolve_and_export_run_without_network` (новый, GAP-06); `[BRM] migration::migration_all_five_commands_run_without_network` | те же тесты: прокси на закрытый порт, `GIT_ALLOW_PROTOCOL=none`, без `MERIDIAN_INSTANCE`; сетевое пространство имён, где хост его даёт | исполнено (на этом хосте `unshare -rn` запрещён — только блокировка через окружение, §11) | `RUST_ONLY_CONTRACT` |
| CLI-06 | Нет аварии на внешнем вводе | `rust-migration-quality.md` §3; план §5.22.6 п. 15 | весь production | §10 | `[BR] validate_reports_an_empty_front_matter_block_instead_of_panicking`; `[H] VALIDATE_MUTATION_FAMILIES` `document-identity-empty-front-matter` | исполнено после GAP-01 | `RUST_ONLY_CONTRACT` |

### 8.3. Требования §6.1–§6.5a плана

| ID | Требование | Строки-доказательства | Статус |
|---|---|---|---|
| GATE-6.1 | Архитектурные ворота каждого пакета: крейты, порты, четыре уровня, строгие типы, классификация расхождений; затем fmt/clippy/test/doc | §10–§12; §13 | `CONFORMANT` |
| GATE-6.2 | Харнесс: каждое расхождение — сохранённый контракт, принятое Rust-native улучшение или непереносимый дефект Node; необъяснённых нет | §4–§7, COMPAT-01…32; раздел `[H]` «accepted-rust-native» фиксирует точную форму каждого принятого расхождения `validate`. GAP-09 — не расхождение харнесса, а отсутствующая поверхность (VAL-30, NODE-02, NODE-03) | `CONFORMANT` |
| GATE-6.3 | Строгий слой YAML/JSON Schema и adversarial-кейсы | SF-02, SF-04, SF-05 | `CONFORMANT` |
| GATE-6.4 | Сбой посреди транзакции, повреждённая база, несовместимая версия | SQL-07, SQL-10, SQL-02 | `RUST_ONLY_CONTRACT` |
| GATE-6.4a | Миграция схемы и метаданные, роль из метаданных, отказ несовместимой роли, разделение ролей, NoOp на уровне app, неизвестная версия/роль | SQL-01, SQL-03, SQL-04, SQL-02, CLI-04 (`app: storage::role::tests::a_disabled_event_sink_does_not_change_the_domain_result_or_stored_state`) | `RUST_ONLY_CONTRACT` |
| GATE-6.4b | Отключённый приёмник не меняет stdout/stderr/код ни одной команды | CLI-04 | `RUST_ONLY_CONTRACT` |
| GATE-6.5 | `import → export → import`, `apply → rollback`, повторный apply, работа без сети | IMP-03, MIG-04, MIG-02, CLI-05 | `RUST_ONLY_CONTRACT` |
| GATE-6.5a.1 | Полный импорт 346 = 336 + 10 + 0 | IMP-02, IMP-04 | `RUST_ONLY_CONTRACT` |
| GATE-6.5a.2 | Эквивалентность применимых норм | IMP-06 (COMPAT-26) | `ACCEPTED_RUST_NATIVE` |
| GATE-6.5a.3 | Идемпотентность импорта | MIG-02, `[BRM] migration::migration_apply_verify_and_every_repeat_are_idempotent` | `RUST_ONLY_CONTRACT` |
| GATE-6.5a.4 | Обратимость на данных замороженного источника | MIG-04, `[BRM] migration::migration_real_bundle_imports_336_records_verifies_61_norms_and_rolls_back` | `RUST_ONLY_CONTRACT` |
| GATE-6.5a.5 | Нет эксплуатационного чтения `MERIDIAN_INSTANCE` | MIG-06 | `RUST_ONLY_CONTRACT` |

### 8.4. Node entry points

| ID | Node-поверхность | Rust | Строки | Статус |
|---|---|---|---|---|
| NODE-01 | `scripts/kernel-validate.mjs` без Instance | `meridian validate` | VAL-00…VAL-29 | `CONFORMANT` |
| NODE-02 | `scripts/kernel-validate.mjs` с `MERIDIAN_INSTANCE` | `meridian validate --workspace-db`; mapping §9.3 | VAL-30, COMPAT-33/34 | `ACCEPTED_RUST_NATIVE` |
| NODE-03 | `scripts/validate-and-log.mjs` (запись метрик в Instance) | `meridian validate --workspace-db … --log-metrics` | mapping §9.3, M-18 | `ACCEPTED_RUST_NATIVE` |
| NODE-04 | `scripts/rule-resolver.mjs`: ядро `resolveRules` | `meridian_core::resolver` | RES-01…RES-03 | `CONFORMANT` |
| NODE-05 | `scripts/rule-resolver.mjs`: CLI (читает Instance) | `meridian resolve` (§7.1) | RES-04, RES-05 | `RUST_ONLY_CONTRACT` |
| NODE-06 | `scripts/preflight.mjs` (`--require-instance` — разрешённый переходный путь §6.5a.5; `validator present` — артефакт Node) | `meridian doctor` | DOC-01, DOC-02 | `CONFORMANT` |
| NODE-08 | `scripts/lib/*.mjs` (16 модулей) | семейства `meridian_app::operating_model`, `source_format`, `migration` | SF-*, VAL-07…VAL-28, IMP-01 | `CONFORMANT` |

### 8.5. Граница вне матрицы: переходное Node-tooling (решение архитектора по GAP-15)

`scripts/validate-branch-name.mjs`, `hooks/pre-push` и
`.github/workflows/gate.yml` — переходное governance/tooling разработки
Kernel, а не runtime-зависимость выпущенного бинарника `meridian`. Решение
RFC D-F сохраняет shell-hook на переходный период; hook и CI уже вызывают
общий харнесс (`test/conformance-harness.test.mjs`). Удаление Node из этих
мест — отдельное решение владельца после выпуска (выпускной рубеж §7 п. 2
касается бинарника). Эта граница не входит в закрытую матрицу и не получает
ни одного из четырёх статусов §5.23.3.

## 9. Пробелы

### 9.1. Закрытые в первой передаче (`gap -> owner -> fix -> regression evidence`)

| ID | Пробел | Владелец | Исправление | Regression evidence |
|---|---|---|---|---|
| GAP-01 | `meridian validate` аварийно завершался (код 101, пустой stdout) на Kernel-документе `---\n---`: `&text[4..end]` при `end = 3`; то же при многобайтном символе сразу после `---`. Node: `slice(4, 3)` = `''`, семь `FAIL` о полях. Нарушены стабильные коды §7.1 и §5.22.6 п. 15 | `meridian-app` `source_format::markdown_identity` (общий владелец двух проверок) | Новая тотальная `front_matter_block` — единственный владелец выделения блока; `document_identity` (cli) и `agent_instruction_identity` (app) вызывают её | `app: source_format::markdown_identity::tests::{an_empty_block_is_empty_not_a_reversed_range, a_multi_byte_character_after_the_opening_marker_is_not_split, a_missing_opening_or_closing_marker_yields_an_empty_block, the_front_matter_block_is_the_text_between_the_two_markers}`; `[BR] validate_reports_an_empty_front_matter_block_instead_of_panicking`; `[H] VALIDATE_MUTATION_FAMILIES` `document-identity-empty-front-matter` |
| GAP-02 | Общий проход `$schema` терял путь схемы в тексте отказа строгого слоя (`at //properties/m` вместо `at <путь схемы>/properties/m`) и не нормализовал `./`/`../` в пути схемы, как `path.resolve` Node | `meridian-cli` `commands::validate::registry_schema` (владелец прохода) | До `validate` вызывается `json_schema::assert_supported_deep(schema, <путь схемы>)`, как у Node; путь нормализуется лексически — для относительного `--kernel` это было неполно, доисправлено в §9.2 | `cli: commands::validate::registry_schema::tests::{an_unsupported_schema_construct_is_located_under_the_schema_file, a_relative_schema_reference_is_resolved_like_the_reference}`; `[H] VALIDATE_MUTATION_FAMILIES` `registry-schema-unsupported-keyword-and-format`; `[HF] real-node-rust-cli-validate-clean-kernel` |
| GAP-03 | Строгий слой YAML (RFC D-A) отдавал вход библиотеке раньше собственного reader: табуляция, второй документ и незакрытая flow-коллекция получали текст библиотеки (включая `use from_multiple or from_multiple_with_options`) вместо текста эталона | `meridian-app` `source_format::yaml` | Строковый lint и reader подмножества идут первыми, библиотека — последней | `app: source_format::yaml::tests::{the_strict_line_lint_names_the_rejection_before_the_library_does, text_that_is_not_yaml_is_still_rejected_by_the_library}`; `[H] VALIDATE_MUTATION_FAMILIES` `registry-yaml-strict-lint-rejections`; ad hoc 11 250 случайных входов через реальный бинарник — ни одной аварии (не коммитится) |
| GAP-04 | Семейство `validate` `rule-resolution` не имело отрицательного Node/Rust доказательства | `test/conformance-harness.test.mjs` | Мутация фикстур обеих групп PHASE B | `[H] VALIDATE_MUTATION_FAMILIES_7A` `rule-resolution-fixtures` |
| GAP-05 | Fail-closed ветви resolver не имели межъязыкового доказательства (общий корпус содержит только успешные разрешения и не может нести отказ из-за CLI-производителя) | `verification/conformance-harness` | Отдельный корпус `rule-resolution-fail-closed-corpus.json` (10 случаев); `meridian-app/examples/rule_resolution_producer.rs` принимает путь к корпусу | `[HF] real-node-rust-rule-resolution-fail-closed` |
| GAP-06 | §6.5 для `init`/`doctor`/`validate`/`resolve`/`export` не имел доказательства | `meridian-cli/tests/binary_runs.rs` | Тест по образцу пакета 8 | `[BR] init_doctor_validate_resolve_and_export_run_without_network` |
| GAP-07 | Архитектор признал отличие `stack-profiles` неверного типа принятым (план §5.16.3 п. 9, §5.5d п. 2), но в реестре `COMPATIBILITY.md` строки нет | `COMPATIBILITY.md` | Добавлена строка (COMPAT-28) | `app: validation::mechanical_integrity::stack_profiles::tests::a_wrong_type_profiles_key_reports_and_does_not_load`; `[H]` «намеренная граница: Node-эталон падает с необработанным TypeError на profiles неверного типа …» и «… реальный `meridian validate` на profiles неверного типа даёт явный authored FAIL …» |
| GAP-08 | Строки реестра 119, 120, 128, 129 ссылались на шаблоны имён (`…_missing_or_unreadable…`) или целые модули, а не на тесты | `COMPATIBILITY.md` | Шаблоны заменены точными именами тестов, смысл строк не изменён | §9.3 |

### 9.2. Закрытые в корректирующем раунде (решения архитектора)

| ID | Решение | Владелец | Исправление / классификация | Evidence |
|---|---|---|---|---|
| GAP-02 (доисправление) | Итоговый путь схемы должен совпадать с Node и для абсолютного, и для относительного `--kernel` | `meridian-cli` `commands::validate::registry_schema` | Прежняя лексическая нормализация сохраняла ведущие `..` (`../k/…`) вместо `path.resolve` относительно текущего каталога. Теперь основной путь схемы — `resolve_like_the_reference`: относительный результат разрешается от текущего каталога и нормализуется; запасной путь `./`-ссылки — `normalize`, как `path.join` эталона (не абсолютный). Нечитаемый текущий каталог — новая ошибка окружения `CollectError::CurrentDirectory` (код 3), не молчаливый относительный путь | `[BR] validate_reports_a_relative_kernel_schema_under_its_absolute_path` (production run, `--kernel k` из родителя и `--kernel ../k` из соседнего каталога; ожидаемые строки получены реальным Node на том же дереве и cwd); `cli: …::registry_schema::tests::{a_relative_schema_reference_is_resolved_like_the_reference, normalizing_a_relative_path_keeps_its_leading_parent_segments, an_unsupported_schema_construct_is_located_under_the_schema_file}` |
| GAP-10 | `ACCEPTED_RUST_NATIVE`: код, вердикт и стабильный авторский префикс `<семейство>: <файл> is not valid JSON: ` — контракт; хвост V8/`serde_json` — нет | маршруты перечислены в COMPAT-29 | Строка реестра COMPAT-29 (одна, не дублирует COMPAT-24 о `content` миграции) | Positive: `[HF] real-node-rust-cli-validate-clean-kernel`. Negative — все девять самостоятельных владельцев парсера: `[H]` «accepted-rust-native: GAP-10 invalid-JSON tail: combined routes» и пять изолированных «accepted-rust-native: GAP-10 invalid-JSON tail: <фикстуры> fixtures». Combined: пять пар (общий проход `schema`, фикстуры `rule-resolution` и `functional-parity`, схема `execution-state-model` и зависимая строка `upgrade-integration-qualification` через тот же `run_contract_boundary::parse_json`); изолированные: по одной паре для `instruction_source_registry`, `controlled_rule_intake`, `existing_project_compatibility_mode` (cli), `task_pattern_registry`, `task_specification` (app). Итого 10 пар `FAIL` с равным кодом 1, равными `WARN`, поимённо равными авторскими префиксами и различающимся только хвостом; плюс две объявленные точным текстом Rust-only строки каскада «no task-pattern catalog is available» (принятые границы COMPAT-09 и COMPAT-23), других `FAIL` нет. Первая версия отчёта ошибочно называла «шесть» строк — фактически combined-case давал пять |
| GAP-11 | `ACCEPTED_RUST_NATIVE`: штатная Rust-команда не читает Instance, поэтому текст называет только Kernel | `meridian-cli` `commands::validate::registry_schema` | Строка реестра COMPAT-30 | `[BR] validate_reports_a_missing_schema_as_absent_from_the_kernel_only` (реальный бинарник без `MERIDIAN_INSTANCE`, точный текст); `[H]` «accepted-rust-native: GAP-11 missing schema names the Kernel only» |
| GAP-12 | `ACCEPTED_RUST_NATIVE`: Rust обязан fail-closed отклонять все три входа, которые Node принимает с молчаливой потерей данных | `meridian-app` `source_format::yaml` | Строка реестра COMPAT-31; явный Node/Rust case харнесса для всех трёх входов | `[H]` «accepted-rust-native: GAP-12 lossy YAML is rejected fail-closed» (Node-разбор с потерей зафиксирован; у Rust ровно три `schema: cannot parse … malformed YAML:`; контрольный документ равен); `app: source_format::yaml::tests::text_that_is_not_yaml_is_still_rejected_by_the_library` |
| GAP-13 | `ACCEPTED_RUST_NATIVE`: авторский `FAIL` предпочтительнее текста исключения Node | `meridian-app` `validation::mechanical_integrity::instruction_topics` | Необоснованный `exactTextNotRequired` удалён вместе с ветвью исполнителя харнесса; семейство перенесено в раздел «accepted-rust-native» с точной проверкой формы; строка реестра COMPAT-32 | `[H]` «accepted-rust-native: GAP-13 instruction-topics of the wrong type» (ровно одна Node-строка формы `instruction-topics: … is not a function`, ровно одна Rust-строка с точным текстом, иных расхождений нет); `app: …::instruction_topics::tests::a_wrong_type_topics_key_is_a_failure` |
| GAP-14 | Исправить: `doctor` проверяет только чтением, что переданный Kernel под Git; `validator present` не переносится | `meridian-cli` `kernel::check_kernel_under_git`, `commands::doctor` | Критерий `preflight`: запись `.git` существует (каталог основного checkout или `gitdir:`-файл связанного worktree); иначе `kernel_git: {ok: false, error}` и `healthy: false`, код 1. Новое поле JSON `kernel_git` и строка `Kernel Git:` human-вывода; конверт, коды, пустой stderr сохранены | `[BR] doctor_confirms_the_kernel_is_under_git`, `doctor_reports_a_kernel_that_is_not_under_git`; прежние `doctor_*` и `init_doctor_validate_resolve_and_export_run_without_network` зелёные |
| GAP-15 | Снять: переходное governance/tooling, не runtime-зависимость бинарника | — | Граница §8.5 вне матрицы; строка NODE-07 удалена | — |
| Счётчик идентичности | Кандидат не должен требовать правки литерала после индексации | `meridian-cli/tests/binary_runs.rs` | `agent_instruction_identity_undeclared_other` больше не литерал 34: тест независимо выводит классифицируемую совокупность (отслеживаемые Git `.md`, несущие собственный Front Matter и открывающиеся `---`) и требует `совокупность − 29 − 22`; литералы 29 и 22 и прочие snapshot-счётчики не ослаблены | `[BR] validate_reports_ok_on_this_kernel_with_zero_real_failures` в рабочем дереве (85 → 34); тот же бинарник на отдельной scratch-копии, где отчёт уже в индексе (86 → 35; индекс кандидата не затрагивался) |

### 9.3. GAP-09 — mapping Instance-проверок Node (открыт)

Источник — фактические Instance-ветви `scripts/kernel-validate.mjs` и
`scripts/validate-and-log.mjs`; прогон на замороженном источнике: код 1,
10 `FAIL` (5 `kernel-purity` продуктового паттерна, 1 `stack-profile`,
4 `instruction-intake`), 11 `WARN`. Категории: **1** — переходная проверка
замороженного источника (файловая раскладка или Git Instance, исчезающие
после импорта); **2** — бизнес-инвариант уже доказан import/SQLite
executable evidence; **3** — действительно утраченная бизнес-проверка.
Чтение `MERIDIAN_INSTANCE` штатным Rust CLI не добавлялось.

| ID | Проверка Node (уровень) | Кат. | Основание / доказательство | Недостающий контракт и production-владелец (кат. 3) |
|---|---|---|---|---|
| M-01 | `kernel-purity`: исключение поддерева Instance из скана Kernel (`INFO`) | 1 | Нужна только пока Instance может лежать внутри Kernel; у Rust нет корня Instance | — |
| M-02 | `kernel-purity`: продуктовые литералы и паттерны из `product.yaml` (`FAIL`; сейчас 5) | 3 | `product.yaml` перенесён непрозрачной `workspace-file` байт-в-байт (IMP-04, COMPAT-27), но ни один Rust-маршрут не выводит из него литералы и не сканирует Kernel; режим Kernel даёт только `WARN` UNVERIFIED (VAL-29). Пять текущих совпадений в Kernel Rust-гейт не видит | Типизированная продуктовая запись с литералами/паттернами чистоты в рабочей базе и чтение её `validate` через порт app; владелец — `meridian-cli` `commands::validate::kernel_purity` (представление) + операция `meridian-app` над `RecordRepository` |
| M-03 | `product record`: ошибка разбора или отсутствие записи (`FAIL`) | 3 | Часть M-02 | Тот же контракт, что M-02 |
| M-04 | `.agent`: наличие Front Matter и `path-placement` статуса относительно каталогов `active`/`done`/`archive` (`WARN`) | 1 | Раскладка каталогов рабочей памяти замороженного источника; после импорта документ — типизированная запись (`plan`, `report`, `analysis`, …, IMP-02/IMP-04), каталога, с которым статус мог бы разойтись, нет | — |
| M-05a | Общий проход `$schema` по Instance: `rule-resolution/applicability.yaml` | 2 | План миграции проверяет реестр применимости схемой применимости (`typed_applicability_register`); 61 норма, lost/added/changed/duplicate = 0 — IMP-06, COMPAT-26, `[BRM] migration::migration_real_bundle_imports_336_records_verifies_61_norms_and_rolls_back` | — |
| M-05b | Общий проход `$schema` по прочим Instance-реестрам (инвентарь, команды, окружения, verification, приём инструкций, …) (`FAIL`) | 3 | Импорт проверяет только конверт `scoped-record.schema.json`, где `payload` — любой объект; форму payload по реестру Rust не проверяет ни при `migration apply`, ни при `import --kind canonical-records` | Схема payload по `record_type` для записей рабочей базы и её проверка на импорте и в `validate`; владелец — `meridian-app` `migration::operations` (импорт) и `validate` над рабочей базой |
| M-06 | `instance-context`: контекст Instance, которого требует скилл (`requires_instance_context`) (`FAIL`) | 3 | Rust всегда даёт `WARN` UNVERIFIED (VAL-09); наличие соответствующей записи в рабочей базе не проверяется | Проверка, что рабочая база несёт контекст, требуемый скиллом Kernel; владелец — `meridian-cli` `commands::validate::instance_context` + порт app к `RecordRepository` |
| M-07 | `ext-dependencies`: разбор и совет о непинованных зависимостях (`WARN`, `FAIL` при разборе) | 3 | `external-dependencies.yaml` перенесён непрозрачной `workspace-file`; Rust-читателя нет | Типизированная запись внешних зависимостей и её проверка; владелец — `validate` над рабочей базой (`meridian-app` + `meridian-cli`) |
| M-08 | `inventory-git`: записанные факты VCS против фактического состояния репозиториев, TTL `last_verified` (`FAIL`/`WARN`; сейчас 7 `WARN`) | 3 | Инвентарь перенесён записями `repository-identity`/`-reference`/`-relationship` (IMP-04), но ничто не сверяет их с репозиториями | Сверка записей инвентаря с репозиториями продукта; владелец — `meridian-cli` (адаптер Git) + операция `meridian-app` |
| M-09 | `stack-profile`: профиль каждого репозитория инвентаря из пула и подтверждение манифестом (`FAIL`; сейчас 1) | 3 | Пул профилей Kernel проверяется (VAL-13), объявления репозиториев — нет | Проверка объявлений записей `repository-identity` против пула `meridian_core::mechanical_integrity::stack_profiles`; владелец — `meridian-app` + `meridian-cli` |
| M-10 | `instruction-intake`: один текущий реестр на репозиторий; исчезновение целого реестра по истории Git Instance (`FAIL`) | 1 / 2 | Файловая идентичность реестра и история Git — механика замороженного источника (1). Бизнес-инвариант «принятое решение не исчезает» для импортированных 74 записей `instruction-intake-record` доказан append-only хранилищем: `[SA] direct_update_or_delete_of_record_revisions_is_refused_by_sqlite_itself`, IMP-02/IMP-04 (2) | — |
| M-11 | `instruction-intake`: реестр объявляет `$schema` (`FAIL`) | 1 | Предусловие opt-in прохода `$schema` по файлам; сама утраченная проверка формы — M-05b | — |
| M-12 | `instruction-intake`: реестр называет репозиторий из инвентаря (`FAIL`) | 3 | Ссылочная целостность между записями приёма и `repository-identity` в рабочей базе не проверяется | Проверка ссылок записей приёма на записи инвентаря; владелец — `meridian-app` над `RecordRepository` + `validate` |
| M-13 | `instruction-intake`: append-only против истории файла (уровень 1) (`FAIL`) | 1 / 2 | Как M-10: история Git — механика источника (1); append-only записей — SQLite (2), `[SA] a_second_put_appends_a_revision_without_changing_the_previous_one`, `direct_update_or_delete_of_record_revisions_is_refused_by_sqlite_itself` | — |
| M-14 | `instruction-intake`: сверка с опубликованной веткой (уровень 2, сеть, по запросу) | 1 | Удалённая ветка Git замороженного источника; у рабочей базы её нет, сеть запрещена §6.5 | — |
| M-15 | `instruction-intake`: полнота против дерева репозитория продукта, неотслеживаемые и игнорируемые нормы, покрытие и существование регионов (`FAIL`; сейчас 4) | 3 | Rust не сверяет записи приёма с текущими деревьями репозиториев продукта | Проверка полноты приёма против репозиториев продукта; владелец — `meridian-app` (регионы — `source_format::regions`) + адаптер Git/файлов `meridian-cli` |
| M-16 | `instance-provenance`: неотслеживаемые файлы Instance (`WARN`) | 1 | Git замороженного источника; в рабочей базе ревизии append-only и журнал миграции (SQL-08) | — |
| M-17 | `git-provenance`: Instance под Git (`OK`/`WARN`) | 1 | То же | — |
| M-18 | `validate-and-log.mjs`: запись метрик прогона для реального Instance | 3 | Две импортированные записи `gate-run-observation` есть, новые не добавляет ничто; RFC называет `meridian validate --log-metrics` | Запись `gate-run-observation` в рабочую базу по прогону `validate`; владелец — `meridian-cli` `commands::validate` + операция записи `meridian-app` |

Итог mapping: категория 3 непуста (M-02, M-03, M-05b, M-06, M-07, M-08,
M-09, M-12, M-15, M-18). Общий недостающий контракт — проверка
продуктового состояния рабочей базы только чтением (и запись метрик по
M-18) без `MERIDIAN_INSTANCE`: вход — рабочая база, владелец оркестрации —
`meridian-app` над `RecordRepository`, представление — `meridian-cli`
`commands::validate`. Пока его нет, итог — `NOT_QUALIFIED`.

### 9.4. Изменения `COMPATIBILITY.md`

- **Строка 119** (`instance-data-migration` … non-`NotFound` I/O): шаблоны имён
  заменены точными тестами:
  `instance_data_migration_a_missing_or_unreadable_mandatory_file_fails_closed_and_is_distinguished`,
  `instance_canonical_export_missing_malformed_and_unreadable_files_fail_closed`,
  `workspace_compatibility_qualification_every_composed_schema_is_mandatory`,
  `upgrade_integration_qualification_every_composed_schema_is_mandatory`.
- **Строка 120** (schema short-circuit 7d): шаблон заменён четырьмя
  точными именами `…_a_container_or_envelope_schema_violation_short_circuits_before_the_domain`
  для каждого семейства.
- **Строка 128** (применимость): вместо модуля — четыре теста
  `meridian-core/src/migration/applicability.rs::tests::{migration_applicability_identical_sides_are_equivalent, migration_applicability_detects_lost_added_changed_and_duplicate, migration_applicability_empty_sides_are_never_equivalent, migration_applicability_a_re_resolution_on_another_date_is_its_own_identity}`.
- **Строка 129** (фрагменты): вместо файла — восемь тестов
  `meridian-app/src/migration/fragment/tests.rs`.
- **Новая строка** (COMPAT-28): `stack-profiles` неверного типа — принятое
  архитектором отличие (план §5.16.3 п. 9), до этого отсутствовавшее в
  реестре.
- **Новые строки корректирующего раунда** (COMPAT-29…32): класс хвоста
  невалидного JSON (GAP-10), текст отсутствующей схемы (GAP-11),
  fail-closed разбор YAML с потерей данных у Node (GAP-12),
  `instruction-topics` неверного типа (GAP-13) — все приняты архитектором.

### 9.5. Корректирующий пакет `rust-workspace-state-validation` — реализация (первая передача и корректирующий раунд, 2026-09-25)

План §5.23.11. Штатный маршрут `meridian validate --kernel <path>
--workspace-db <path> [--log-metrics]`: `meridian-cli`
(`commands::validate::workspace_state`, адаптеры `FsWorkspaceReader`,
`RealRepositoryAccess`, `SystemClock`) → одна операция
`meridian_app::workspace_state::validate` над портами `RecordRepository`,
`WorkspaceReader`, `RepositoryAccess`, `Clock` → закрытые DTO и схемы
(`meridian_app::workspace_state::{dto, payload}`) → строгие типы и чистые
проверки `meridian_core::workspace_state`. `MERIDIAN_INSTANCE` не читается;
без `--workspace-db` Kernel-only контракт не изменён. Новые cases
корректирующего раунда исполнены на финальном рубеже §5.23.7; строки
VAL-30, NODE-02 и NODE-03 приняты как `ACCEPTED_RUST_NATIVE`.

| M | Production-маршрут | Positive | Negative |
|---|---|---|---|
| M-02/M-03 | `core: workspace_state::product` (литералы с ASCII-границей `\b`, как у эталона; паттерны), `app: workspace_state::validate` (запись `workspace-file` `product.yaml` → `ProductDto` → `ProductPurity`), представление пути — `cli: commands::validate::workspace_state` | `app: workspace_state::tests::workspace_state_a_consistent_workspace_is_clean`; `[BR] workspace_state::workspace_state_a_consistent_database_is_clean_and_left_untouched` | `app: …::workspace_state_m02_product_literal_and_pattern_are_found_in_the_kernel`, `…::workspace_state_m03_missing_or_uncompilable_product_record_fails`; `core: workspace_state::product::tests::*` (5); `[BR] workspace_state::workspace_state_every_category_3_row_fails_through_the_binary` |
| M-05b | `core: workspace_state::record_types` (закрытый реестр 32 типов → контракт), `app: workspace_state::PayloadRegistry::check` — один валидатор для `migration::bundle` (frozen), `migration::operations::import_canonical` и `validate` | тот же positive; `[BRM] migration::migration_real_bundle_imports_336_records_verifies_61_norms_and_rolls_back` (все 336 реальных записей приняты) | `app: …::workspace_state_m05b_a_stored_payload_no_contract_accepts_fails`; `app: migration::operations::tests::workspace_state_canonical_import_refuses_a_payload_no_contract_accepts`; `[BRM] migration::workspace_state_frozen_import_refuses_a_target_no_payload_contract_accepts` (вариант fixture `unknown-record-type`, принятый Node); `[BR] workspace_state::workspace_state_payload_contracts_guard_import_and_stored_state` (обе стороны: отказ импорта без записи и обнаружение вброшенной в SQLite записи) |
| M-06 | `app: workspace_state::validate` (PIN скиллов Kernel через `WorkspaceReader`, путь против записей `workspace-file`) | те же positive | `app: …::workspace_state_m06_missing_instance_context_fails`; `[BR] …every_category_3_row_fails_through_the_binary` |
| M-07 | `app: dto::ExternalDependenciesDto` → `core: workspace_state::dependencies` | те же positive | `app: …::workspace_state_m07_malformed_or_unpinned_dependencies`; `core: workspace_state::dependencies::tests::*`; `[BR] …every_category_3_row_fails_through_the_binary` (WARN) |
| M-08 | `cli: adapters::git_inspector::RealRepositoryAccess` → `core: workspace_state::inventory::check_entry` (TTL через `Clock`) | `core: …inventory::tests::workspace_state_inventory_confirms_a_matching_fresh_entry`; те же positive | `app: …::workspace_state_m08_drift_fails_and_an_unreachable_repository_is_unverified`; `[BR] …every_category_3_row_fails_through_the_binary`, `…workspace_state_undeclared_profile_and_unreachable_repository` |
| M-09 | `core: workspace_state::profile` против пула `stack-profiles.yaml`; манифест через `RepositoryAccess::reader` | `core: …profile::tests::workspace_state_profile_confirms_a_supporting_manifest_and_names_every_mismatch` | `app: …::workspace_state_m09_undeclared_pool_profile_or_unreadable_manifest`; `[BR] …workspace_state_undeclared_profile_and_unreachable_repository` |
| M-12 | `core: workspace_state::intake::check_references` (репозиторий записи — её собственная `repository-scope`) | те же positive | `app: …::workspace_state_m12_intake_of_an_unknown_repository_fails`; `core: …intake::tests::workspace_state_intake_reference_to_an_unknown_repository_fails`; `[BR] …every_category_3_row_fails_through_the_binary` |
| M-15 | `core: workspace_state::intake::check_completeness`; регионы — существующий `app: source_format::regions::instruction_regions`; tracked/untracked/ignored — `RepositoryAccess` | `core: …intake::tests::workspace_state_intake_complete_tree_is_confirmed` | `core: …::workspace_state_intake_names_every_gap_of_an_incomplete_tree`, `…unreachable_or_unreadable_is_unverified`; `app: …::workspace_state_m15_an_uncovered_norm_fails_completeness`; `[BR] …every_category_3_row_fails_through_the_binary` |
| M-18 | `core: workspace_state::observation`, `app: workspace_state::observation_request` (ключ идемпотентности — digest канонического содержимого; запрос проверяется тем же `PayloadRegistry`), запись — `cli: …::workspace_state::record_metrics` после готового результата | `app: …::workspace_state_m18_observation_request_is_checked_and_idempotent` | `[BR] workspace_state::workspace_state_log_metrics_records_one_observation_without_changing_the_verdict` (ровно одна запись после положительного и отрицательного результата; `--log-metrics` без базы — код 2; невозможная запись → `metrics.outcome = not-recorded`, вердикт и код неизменны) |

Сквозные критерии: открытие базы — `[BR] workspace_state::workspace_state_database_that_cannot_be_opened_is_an_environment_error`
(missing/corrupt/wrong-role/wrong-edition → код 3, пустой stdout, файл не
создаётся); Kernel-only форма — `[BR] workspace_state::workspace_state_kernel_only_validate_keeps_its_advisories_and_shape`;
только чтение — байтовое равенство файла базы и экспорта в
`…a_consistent_database_is_clean_and_left_untouched`; NoOp/Recording —
`[CLI] tests::workspace_state_event_sink_has_no_effect_on_db_backed_validate`;
без сети — `[BR] workspace_state::workspace_state_db_backed_validate_runs_without_network`;
критерий 2 на реальных данных — `[H]` «workspace-state: DB-backed validate на
импортированном реальном bundle совпадает с Node-эталоном на каждом
category-3 вердикте» (Node на Instance в закреплённой ревизии источника
bundle; исключение D1 объявлено поимённо).

Решения архитектора (корректирующая инструкция §5.23.11, 2026-09-25):

- **D1 — `ACCEPTED_MIGRATION_BOUNDARY`.** 10 единиц bundle остаются
  `retained-transitional` по принятому решению пакета 8 и не изменены
  (`[BRM] migration::migration_real_bundle_imports_336_records_verifies_61_norms_and_rolls_back`:
  `retained = 10`). Одна из них — запись `inventory/repositories.yaml`;
  её репозитория нет в рабочей базе. Граница точная: из сравнения
  исключаются только строки эталона с префиксом `inventory-git: <id> ` или
  `stack-profile: <id> ` удержанного репозитория (на реальных данных — один
  `FAIL` расхождения ревизии и один `WARN` TTL); харнесс требует, чтобы
  такие строки у эталона были, и чтобы Rust не назвал удержанный репозиторий
  ни в одной category-3 строке. Идентификатор репозитория в Kernel не
  записывается.
- **D2 — принято.** Тексты, где эталон называет Instance или файл реестра,
  и семейство `payload-contract:` зафиксированы в `COMPATIBILITY.md`
  (строка «`meridian validate --workspace-db` … решение D2», COMPAT-33) с
  бизнес-эффектом и matched evidence; архитектор включил в D2 и два текста
  проверок D3 того же класса (темы — одна строка на репозиторий,
  «workspace inventory» у родителя издания). Отрицательные ветви доказаны
  matched case `[H]` «accepted-rust-native: COMPAT-33 отрицательные ветви D2 — равные вердикты, отличие только в локусе текста»: код, уровень и
  множество дефектов равны, различаются только локус текста и порядок
  тем.
- **D3 — реализовано, больше не открытое решение.** Три проверки
  `instruction-intake` эталона — категория 3 — перенесены тем же
  production-маршрутом (таблица ниже).

Корректирующий раунд (2026-09-25):

1. **Fail-open `repository_tree` удалён.** Сбой `untracked_files` или
   `ignored_among` больше не становится пустым набором: `meridian-app`
   (`workspace_state::intake::repository_tree`) передаёт
   `meridian_core::workspace_state::intake::TreeFact::Unavailable` в
   `check_completeness`, которая даёт явный `WARN … UNVERIFIED …` и никогда
   не засчитывает дерево в `intake_repositories_complete`; нечитаемый
   контейнер тоже больше не засчитывается. Отличие от эталона — строка
   `COMPATIBILITY.md` «… сбой порта репозитория не становится пустым
   ответом».
2. **Три контракта D3** — чистые решения и типы в `meridian-core`,
   оркестрация и чтение пулов/репозиториев в `meridian-app`
   (`workspace_state::intake`), конкретные Git-операции только в адаптере
   `meridian-cli` (`RealRepositoryAccess::{has_revision, file_at}`); порт
   `RepositoryAccess` расширен `has_revision` и `file_at(RevisionSelector)`,
   `KernelSide` — пулом тем и корнем Kernel как репозитория `"kernel"`.

| D3 | Production-маршрут | Positive | Negative |
|---|---|---|---|
| Тема записи — из пула тем Kernel | `core: workspace_state::intake::{IntakeTopic, check_topics}` над `TopicPool` принятого `instruction-topics`; `app: workspace_state::intake::check` (пул передаётся `KernelSide::topic_pool`; не загрузился — проверка не выполняется, как у эталона) | `app: workspace_state::tests::workspace_state_a_consistent_workspace_is_clean`; `core: …intake::tests::workspace_state_intake_topics_outside_the_pool_fail_per_repository` (часть без строк); `[BR] workspace_state::workspace_state_d3_topic_packaging_and_parentage_are_confirmed_through_the_binary` | `app: …::workspace_state_d3_a_topic_outside_the_kernel_pool_fails`; `core: …::workspace_state_intake_topics_outside_the_pool_fail_per_repository`; `[BR] workspace_state::workspace_state_d3_every_broken_intake_contract_fails_through_the_binary` |
| Упаковка `skill-package`: имя `SKILL.md` = имя каталога; нет словаря правил (`alwaysApply`, `globs`) во Front Matter | `core: workspace_state::packaging::{is_packaged_skill, check_packaging}` над `SkillPackageText`; `app: workspace_state::intake::skill_package_text` (чтение текста через `RepositoryAccess::reader`) | те же positive; `core: …packaging::tests::workspace_state_packaging_a_package_named_by_its_directory_is_clean`; `app: workspace_state::intake::tests::workspace_state_skill_package_text_reads_name_and_rule_vocabulary` | `app: …::workspace_state_d3_a_misnamed_or_doubly_activated_skill_package_fails` (и нечитаемый пакет → `INFO … UNVERIFIED`); `core: …packaging::tests::{workspace_state_packaging_a_misnamed_or_doubly_activated_package_fails, workspace_state_packaging_an_unreadable_package_is_unverified}`; `[BR] …workspace_state_d3_every_broken_intake_contract_fails_through_the_binary`, `…workspace_state_d3_an_unreachable_parent_or_package_is_unverified` |
| `adopt-edition`: квалифицированный `derived_from`, известный репозиторий, доступность revision/path, совпадение `derived_from_digest` | `core: workspace_state::parentage::{ParentReference, CommitRevision, ParentRepository, judge}` (издание без квалифицированного родителя непредставимо: `IntakeVerdict::AdoptEdition(ParentReference)`; неквалифицированный отвергается схемой — `payload-contract`); `app: workspace_state::intake::observe_parent` над `RepositoryAccess::{vcs_state, has_revision, file_at}`; `cli: adapters::git_inspector::RealRepositoryAccess` (`rev-parse --verify --quiet <rev>^{commit}`: код 0 — есть, 1 — штатно нет, иной код или сигнал — `RepositoryUnavailable`; `ls-tree`, `cat-file blob`); real-adapter тесты `cli: adapters::git_inspector::tests::{workspace_state_has_revision_finds_a_present_commit, workspace_state_has_revision_answers_false_only_for_an_absent_commit, workspace_state_has_revision_reports_a_git_failure_as_unavailable}` | те же positive (родитель в репозитории инвентаря и в Kernel); `core: …parentage::tests::workspace_state_parentage_a_matching_current_parent_is_confirmed` | `app: …::workspace_state_d3_an_unresolvable_or_different_parent_fails` (другой digest, нет файла на ревизии, неизвестный репозиторий, голый путь), `…::workspace_state_d3_an_unavailable_parent_is_unverified_and_a_moved_one_warns`; `core: …parentage::tests::*` (4); `[BR] …workspace_state_d3_every_broken_intake_contract_fails_through_the_binary` (нет ревизии → `INFO … UNVERIFIED`), `…workspace_state_d3_an_unreachable_parent_or_package_is_unverified` |

3. **Payload matrix** (критерий приёмки 3). Одна таблица
   `meridian-cli/tests/fixtures/payload-matrix/families.json`: валидный
   payload каждого из 32 поддержанных `record_type` (все пять форм
   контракта: 12 элементов реестров, `gate-run-observation`, JSON-объект,
   Markdown, файл рабочего пространства, включая два типизированных) и
   обязательная мутация каждого из 15 schema-backed семейств (удалённое
   обязательное поле; у `product.yaml`, где обязательных полей нет, —
   неверный тип поля). Из неё `generate.mjs` строит отдельный синтетический
   замороженный источник `frozen-instance/payload-matrix` с двумя bundle,
   которые принимает каждая проверка эталона.

| Маршрут | Positive | Negative |
|---|---|---|
| Реестр контрактов (полнота таблицы) | `app: workspace_state::tests::workspace_state_payload_matrix_covers_every_family` (каждый `ProductRecordType` есть в таблице; мутация — ровно у schema-backed) | — |
| Один валидатор `PayloadRegistry::check` | `app: …::workspace_state_payload_matrix_every_family_accepts_and_refuses` (все валидные приняты) | тот же тест (все 15 мутаций отвергнуты) |
| `import --kind frozen-instance` | `[BR] workspace_state::workspace_state_payload_matrix_through_the_frozen_import` (bundle `accepted`: 35 записей записаны, `validate` — `rejected_payloads = 0`) | тот же тест (bundle `mutated`: код 1, по строке `payload-contract: target "<id>"` на каждое из 15 семейств, экспорт не изменён — отказ до записи) |
| `import --kind canonical-records` | `[BR] workspace_state::workspace_state_payload_matrix_through_the_canonical_import_and_the_stored_state` (все валидные записи импортированы, `rejected_payloads = 0`) | тот же тест (каждая из 15 мутаций: код 3, `payload-contract:` в stderr, экспорт не изменён) |
| Уже сохранённая база | `app: …::workspace_state_payload_matrix_is_judged_in_the_stored_state` (`rejected_payloads = 0`) | тот же app-тест; `[BR] …through_the_canonical_import_and_the_stored_state` (15 мутаций вставлены в SQLite в обход импорта: `rejected_payloads = 15`, каждая названа) |

Сфокусированные Node/Rust cases: `[H]` «accepted-rust-native: COMPAT-33 отрицательные ветви D2 — равные вердикты, отличие только в локусе текста» (отрицательные ветви COMPAT-33) и `[H]` «workspace-state: DB-backed validate
на импортированном реальном bundle совпадает с Node-эталоном на каждом
category-3 вердикте» — после реализации D3 и уточнения границы D1 (см.
передачу исполнителя, план §5.23.11).

Наблюдения без решения: DB-backed прогон на реальном bundle даёт код 1 —
пять совпадений объявленного продуктового паттерна в текстах Kernel
(`COMPATIBILITY.md`, план, три файла `meridian-app`/`meridian-cli`), те же,
что у эталона; исправление Kernel вне объёма пакета. Синтетический
fixture пакета 8 переведён с неконтрактного типа `inventory-item` на тип
`versioning-conformance-case` (контракт «JSON-объект») и перегенерирован
штатным `generate.mjs`; ревизия источника fixture не изменилась.

## 10. Аудит `COMPATIBILITY.md` — построчно

Каждая строка реестра «Намеренные Rust-native усиления» проверена: указанные
тесты существуют под этими именами; Node-блоки харнесса существуют. Номер —
строка файла до изменений пакета; COMPAT-28 — новая строка первой передачи,
COMPAT-29…32 — новые строки корректирующего раунда, COMPAT-33/34 — новые
строки корректирующего пакета `rust-workspace-state-validation` (§9.5;
последние две строки таблицы).

| ID | Строка | Норма | Классификация | Rust-доказательство (positive / negative) | Node-доказательство | Статус |
|---|---:|---|---|---|---|---|
| COMPAT-01 | 103 | `controlled-rule-intake`: полное тождество `Scope` в кластере | наблюдаемое | `app: operating_model::controlled_rule_intake::tests::a_well_formed_candidate_resolves_and_checks_clean` / `…::a_cluster_whose_members_disagree_only_on_workspace_id_is_flagged` | `[H]` «намеренная граница «cluster scope divergence» …» | `ACCEPTED_RUST_NATIVE` |
| COMPAT-02 | 104 | `controlled-rule-intake`: schema short-circuit | библиотечное | `…::controlled_rule_intake::tests::a_well_formed_candidate_resolves_and_checks_clean` / `…::schema_and_composite_defect_together_produce_exactly_one_schema_diagnostic_matching_the_node_reference` | `[H]` блок прямого импорта `evaluateControlledRuleIntake`; «schema short-circuit» | `ACCEPTED_RUST_NATIVE` |
| COMPAT-03 | 105 | `instruction-source-registry`: закрытый транспорт | библиотечное | `…::instruction_source_registry::tests::every_real_valid_fixture_converts_with_no_malformed_entries` / `…::a_transport_parse_failure_skips_business_checks_for_that_entry_only` | `[H]` прямой импорт `evaluateInstructionSourceRegistry` | `ACCEPTED_RUST_NATIVE` |
| COMPAT-04 | 106 | `existing-project-compatibility-mode`: закрытый транспорт | библиотечное | `…::existing_project_compatibility_mode::tests::the_real_kernel_schema_and_fixtures_agree` / `…::a_transport_parse_failure_skips_business_checks_for_that_entry_only` | `[H]` прямой импорт `evaluateExistingProjectCompatibilityMode` | `ACCEPTED_RUST_NATIVE` |
| COMPAT-05 | 107 | `existing-project-compatibility-mode`: нет вторичного location-mismatch | библиотечное | `…::a_discovery_plan_slot_with_a_valid_id_but_invalid_location_gets_both_defects` / `…::a_discovered_source_naming_an_invalid_plan_slot_gets_no_secondary_location_mismatch_diagnostic` | `[H]` прямой импорт, та же мутация | `ACCEPTED_RUST_NATIVE` |
| COMPAT-06 | 108 | `sha-provenance`: путь скилла не выходит за каталог | наблюдаемое | `app: validation::mechanical_integrity::sha_provenance::tests::a_clean_pin_with_a_nested_artifact_path_verifies_positively` / `…::an_escaping_artifact_name_is_reported_as_missing_not_read_from_outside_the_skill`, `…::an_escaping_source_archive_path_is_reported_as_missing_not_read_from_outside_the_skill`; `core: types::workspace_relative_path::tests::rejects_parent_component` | `[H]` «sha-provenance path confinement» (оба поля, позитивный контроль) | `ACCEPTED_RUST_NATIVE` |
| COMPAT-07 | 109 | `sha-provenance`: неполный `source_archive` | наблюдаемое | `core: mechanical_integrity::sha_provenance::tests::archive_report_complete_is_some_only_when_both_fields_are_valid` / `app: …::sha_provenance::tests::a_source_archive_missing_path_or_sha256_is_a_new_incomplete_diagnostic` | `[H]` «sha-provenance: неполный source_archive» | `ACCEPTED_RUST_NATIVE` |
| COMPAT-08 | 110 | `operating-foundation`: запись без `id` | наблюдаемое | `app: validation::mechanical_integrity::operating_foundation::tests::agreeing_pools_produce_no_diagnostics` / `…::an_entry_with_no_id_is_its_own_reported_defect_not_silently_dropped`; `core: mechanical_integrity::operating_foundation::tests::foundation_entry_rejects_empty_id` | `[H]` «operating-foundation: entry без id» | `ACCEPTED_RUST_NATIVE` |
| COMPAT-09 | 111 | `task-pattern-registry`/`task-specification-contract`: каталог публикуется только целиком | наблюдаемое (на FAIL-ящем Kernel) | `app: operating_model::task_pattern_registry::tests::the_real_kernel_catalog_agrees_with_its_own_schema_and_fixtures` / `…::duplicate_pattern_id_is_reported_through_the_full_pipeline`, `…::a_domain_construction_failure_leaves_the_catalog_none`; `cli: commands::validate::task_specification::tests::no_catalog_fails_closed_without_touching_the_workspace` | `[H] assertTaskPatternRegistryMutationAcceptedDivergence` | `ACCEPTED_RUST_NATIVE` |
| COMPAT-10 | 112 | `functional-parity`: I/O схемы ≠ `NotFound` | наблюдаемое (вердикт меняется) | `app: operating_model::functional_parity::tests::a_missing_schema_produces_no_diagnostics` / `…::a_schema_io_error_other_than_not_found_fails_closed_instead_of_skipping` | `[H]` блок `rust-architecture-conformance-4` schema-I/O | `ACCEPTED_RUST_NATIVE` |
| COMPAT-11 | 113 | `functional-parity`: I/O фикстур ≠ `NotFound` | наблюдаемое (текст) | `…::functional_parity::tests::a_missing_fixtures_file_fails_closed_with_the_not_found_text` / `…::an_unreadable_fixtures_file_fails_closed_with_distinct_text_from_missing` | `[H]` блок `rust-architecture-conformance-4` fixture-I/O | `ACCEPTED_RUST_NATIVE` |
| COMPAT-12 | 114 | run-contract семейства: I/O ≠ `NotFound` | наблюдаемое (текст) | `app: operating_model::execution_state::tests::{a_missing_schema_keeps_the_node_text_and_an_unreadable_one_is_distinct, a_missing_or_unreadable_envelope_fails_closed_before_any_fixture, missing_and_unreadable_fixtures_are_distinct_failures}`, `role_and_human_control::tests::missing_and_unreadable_mandatory_files_are_distinct_failures`, `bounded_context_manifest::tests::missing_and_unreadable_mandatory_files_are_distinct_failures`; `cli: commands::validate::rust_architecture_conformance_5::{a_directory_in_place_of_a_mandatory_file_is_unreadable_not_missing, an_app_generated_failure_carries_exactly_one_family_prefix}` | `[H]` блок `rust-architecture-conformance-5` mandatory-read I/O | `ACCEPTED_RUST_NATIVE` |
| COMPAT-13 | 115 | порядок неизвестных полей ответа резолвера | наблюдаемое (текст) | `app: operating_model::bounded_context_manifest::tests::unknown_resolver_fields_are_reported_in_stable_sorted_order`, `operating_model::record_resolution::tests::known_fields_are_typed_and_foreign_kinds_are_kept_for_the_diagnostic`, `operating_model::evidence_and_handoff::tests::evidence_and_handoff_unknown_resolver_fields_are_reported_in_stable_sorted_order` | `[H]` блок «unknown-field-order» | `ACCEPTED_RUST_NATIVE` |
| COMPAT-14 | 116 | `evidence-and-handoff`/`field-evaluation`: I/O ≠ `NotFound` | наблюдаемое (текст) | `app: operating_model::evidence_and_handoff::tests::evidence_and_handoff_missing_and_unreadable_mandatory_files_are_distinct_failures`, `field_evaluation::tests::field_evaluation_missing_and_unreadable_mandatory_files_are_distinct_failures`; `cli: commands::validate::rust_architecture_conformance_6::{evidence_and_handoff_and_field_evaluation_unreadable_is_not_missing, evidence_and_handoff_and_field_evaluation_failures_carry_exactly_one_prefix}` | `[H]` блок `rust-architecture-conformance-6` mandatory-read I/O | `ACCEPTED_RUST_NATIVE` |
| COMPAT-15 | 117 | те же: schema short-circuit | библиотечное | `…::evidence_and_handoff_every_real_fixture_reaches_its_declared_typed_outcome` / `…::evidence_and_handoff_a_schema_violation_short_circuits_before_the_domain`, `…::field_evaluation_a_schema_violation_short_circuits_before_the_domain` | `[H]` блок `rust-architecture-conformance-6` schema short-circuit | `ACCEPTED_RUST_NATIVE` |
| COMPAT-16 | 118 | `field-evaluation`: отвергнутое наблюдение не агрегируется | библиотечное | `core: field_evaluation::tests::field_evaluation_a_report_is_accepted_with_eight_recomputed_aggregates` / `…::field_evaluation_a_refused_resolved_measurement_or_window_is_never_aggregated`, `…::field_evaluation_a_stated_aggregate_or_status_that_diverges_is_rejected` | Node-вывод описан строкой; отдельного Node-блока нет, вердикт равен | `ACCEPTED_RUST_NATIVE` |
| COMPAT-17 | 119 | 7d: I/O ≠ `NotFound` | наблюдаемое (текст) | точные тесты §9.3; `cli: commands::validate::rust_architecture_conformance_7::migration_and_qualification_unreadable_is_not_missing` | Node-вывод — прежний текст (у строки нет отдельного Node-блока, как у принятых классов 4–6) | `ACCEPTED_RUST_NATIVE` |
| COMPAT-18 | 120 | 7d: schema short-circuit | библиотечное | четыре `…_a_container_or_envelope_schema_violation_short_circuits_before_the_domain` (§9.3) | `[H]` блок `rust-architecture-conformance-7`, корректирующий раунд 1, schema short-circuit | `ACCEPTED_RUST_NATIVE` |
| COMPAT-19 | 121 | fingerprint плана: сближение | сближение | `[FP] plan_fingerprint_sorts_mapping_keys_in_the_reference_locale_order`; `core: canonical::tests::locale_order_matches_the_node_reference_for_the_semantic_id_alphabet` | эталонный хеш получен реальным Node | `CONFORMANT` |
| COMPAT-20 | 122 | `opaque_ref_fault`: сближение (устранена авария) | сближение | `core: types::evidence_ref::tests::a_multibyte_prefix_is_checked_without_panicking` | `[H]` «сближение opaque-ref …» | `CONFORMANT` |
| COMPAT-21 | 123 | квалификации читают только принятые композируемые записи | библиотечное | `app: operating_model::workspace_compatibility_qualification::tests::workspace_compatibility_qualification_a_rejected_connection_contributes_no_raw_decision_input`, `upgrade_integration_qualification::tests::upgrade_integration_qualification_a_rejected_report_contributes_no_raw_workspace`; `core: qualification::tests::{qualification_workspace_a_rejected_plan_does_not_hide_its_neighbours, qualification_workspace_matrix_is_order_independent_and_fails_closed}` | `[H]` блок `rust-architecture-conformance-7`, корректирующий раунд 1 (оба случая) | `ACCEPTED_RUST_NATIVE` |
| COMPAT-22 | 124 | schema-invalid композируемый план/экспорт не пинуется по сырому JSON | библиотечное | `app: operating_model::workspace_compatibility_qualification::tests::{workspace_compatibility_qualification_a_schema_invalid_composed_plan_is_never_pinned_by_raw_json, workspace_compatibility_qualification_a_schema_invalid_composed_export_is_never_pinned_by_raw_json}`; `…::instance_data_migration::tests::instance_data_migration_the_composition_point_types_one_resolved_plan` | `[H]` «schema-invalid composed …» | `ACCEPTED_RUST_NATIVE` |
| COMPAT-23 | 125 | `upgrade-integration-qualification` без каталога | наблюдаемое (на FAIL-ящем Kernel) | `cli: commands::validate::rust_architecture_conformance_7::upgrade_integration_qualification_without_a_catalog_fails_closed` | `[H] assertTaskPatternRegistryMutationAcceptedDivergence` | `ACCEPTED_RUST_NATIVE` |
| COMPAT-24 | 126 | нейтральный хвост невалидного JSON `content` | наблюдаемое (текст) | `app: operating_model::instance_canonical_export::tests::instance_canonical_export_invalid_json_content_is_rejected_like_the_reference`, `instance_data_migration::tests::instance_data_migration_invalid_json_content_is_rejected_like_the_reference`; `core: migration::content::tests::json_content_is_parsed_then_checked_for_canonical_form` | `[H]` «invalid JSON content» (оба семейства) | `ACCEPTED_RUST_NATIVE` |
| COMPAT-25 | 127 | удаление пустого blocker-механизма | совпадение с Node | `[BR] validate_reports_ok_on_this_kernel_with_zero_real_failures` | `[HF] real-node-rust-cli-validate-clean-kernel` | `CONFORMANT` |
| COMPAT-26 | 128 | типизированная эквивалентность применимости | наблюдаемое (сравнение шире) | четыре теста `core: migration::applicability::tests` (§9.3) | `[H]` «meridian-cli-migration: эквивалентность применимости …» | `ACCEPTED_RUST_NATIVE` |
| COMPAT-27 | 129 | адресация фрагментов без продуктовых путей | наблюдаемое | восемь тестов `app: migration::fragment::tests` (§9.3); `[BRM] migration::migration_plan_fails_closed_on_every_foreign_source_state` | `[H]` блок «meridian-cli-migration»; ignored real-bundle | `ACCEPTED_RUST_NATIVE` |
| COMPAT-28 | новая | `stack-profiles` неверного типа: авторский `FAIL` вместо аварии Node | наблюдаемое | `app: validation::mechanical_integrity::stack_profiles::tests::agreeing_halves_produce_no_diagnostics` / `…::a_wrong_type_profiles_key_reports_and_does_not_load` | `[H]` две проверки «намеренная граница … profiles неверного типа …» | `ACCEPTED_RUST_NATIVE` |
| COMPAT-29 | новая | Хвост сообщения о невалидном JSON — класс (GAP-10) | наблюдаемое (текст хвоста) | `[HF] real-node-rust-cli-validate-clean-kernel` / `app: operating_model::instance_data_migration::tests::instance_data_migration_a_malformed_schema_or_bundle_fails_closed` (префикс) | `[H]` «accepted-rust-native: GAP-10 invalid-JSON tail: combined routes» и пять изолированных «accepted-rust-native: GAP-10 invalid-JSON tail: <фикстуры> fixtures»: 10 пар `FAIL`, все девять владельцев парсера | `ACCEPTED_RUST_NATIVE` |
| COMPAT-30 | новая | `schema`: отсутствующая схема называет только Kernel (GAP-11) | наблюдаемое (текст) | `[BR] validate_reports_ok_on_this_kernel_with_zero_real_failures` / `[BR] validate_reports_a_missing_schema_as_absent_from_the_kernel_only` | `[H]` «accepted-rust-native: GAP-11 missing schema names the Kernel only» | `ACCEPTED_RUST_NATIVE` |
| COMPAT-31 | новая | YAML с молчаливой потерей данных у Node отклоняется fail-closed (GAP-12) | наблюдаемое (вердикт) | `app: source_format::yaml::tests::parses_block_and_flow_subset` / `…::text_that_is_not_yaml_is_still_rejected_by_the_library` | `[H]` «accepted-rust-native: GAP-12 lossy YAML is rejected fail-closed» | `ACCEPTED_RUST_NATIVE` |
| COMPAT-32 | новая | `instruction-topics` неверного типа: авторский `FAIL` (GAP-13) | наблюдаемое (текст) | `app: validation::mechanical_integrity::instruction_topics::tests::the_real_kernel_pool_agrees_with_itself` / `…::a_wrong_type_topics_key_is_a_failure` | `[H]` «accepted-rust-native: GAP-13 instruction-topics of the wrong type» | `ACCEPTED_RUST_NATIVE` |
| COMPAT-33 | новая | `validate --workspace-db`: тексты, где эталон называет Instance или файл реестра; семейство `payload-contract:` (решение D2) | наблюдаемое (текст/группировка) | `app: workspace_state::tests::workspace_state_a_consistent_workspace_is_clean` / `…::{workspace_state_m03_missing_or_uncompilable_product_record_fails, workspace_state_m06_missing_instance_context_fails, workspace_state_m12_intake_of_an_unknown_repository_fails, workspace_state_m05b_a_stored_payload_no_contract_accepts_fails, workspace_state_d3_a_topic_outside_the_kernel_pool_fails, workspace_state_d3_an_unresolvable_or_different_parent_fails}`; `[BR] workspace_state::{workspace_state_every_category_3_row_fails_through_the_binary, workspace_state_payload_contracts_guard_import_and_stored_state, workspace_state_d3_every_broken_intake_contract_fails_through_the_binary}` | `[H]` «accepted-rust-native: COMPAT-33 отрицательные ветви D2 — равные вердикты, отличие только в локусе текста» (отрицательные ветви: код 1, шесть равных `FAIL`-дефектов, отличие только в локусе и порядке тем); `[H]` «workspace-state: DB-backed validate … совпадает с Node-эталоном на каждом category-3 вердикте» (реальные данные) | `ACCEPTED_RUST_NATIVE` (решение архитектора D2) |
| COMPAT-34 | новая | `validate --workspace-db`, `instruction-intake`: сбой порта репозитория — явный `UNVERIFIED`, дерево не засчитывается (п. 1 инструкции архитектора) | наблюдаемое (только при сбое порта) | `core: workspace_state::intake::tests::workspace_state_intake_complete_tree_is_confirmed`, `app: workspace_state::tests::workspace_state_m15_a_failing_ignored_port_is_unverified_never_complete` (контроль) / `core: …::{workspace_state_intake_an_unavailable_tree_fact_is_unverified_never_complete, workspace_state_intake_an_unreadable_container_is_never_counted_complete}`, `app: …::{workspace_state_m15_a_failing_untracked_port_is_unverified_never_complete, workspace_state_m15_a_failing_ignored_port_is_unverified_never_complete, workspace_state_m15_an_unreadable_container_is_never_counted_complete, workspace_state_d3_an_unavailable_parent_is_unverified_and_a_moved_one_warns}` | исходный текст эталона: `catch { return []; }` в `untrackedNorms`/`ignoredNorms`, `gitShowAt` → `null` (`scripts/kernel-validate.mjs`); исполняемого Node-case сбоя порта нет | `ACCEPTED_RUST_NATIVE` (по указанию архитектора; подтверждение формы строк — архитектору) |

Дублей и устаревших строк нет: строки 112–113, 114, 116, 119 — один класс
(`ReadError::Io` против `NotFound`) на разных семействах, каждая со своими
тестами; 104, 117, 120 — один принятый класс schema short-circuit на разных
семействах; COMPAT-29 (хвост парсера JSON файлов Kernel во всех маршрутах
`validate`) не пересекается с COMPAT-24 (нейтральный хвост невалидного JSON
`content` записи миграции).

## 11. Карта production-владельцев

| Крейт | Владеет | Порты / адаптеры |
|---|---|---|
| `meridian-core` | Предметные типы (`types`), resolver, конфликты, `migration` (plan, export, write set, run, applicability, content, fingerprint), `qualification`, `run_contracts`, `evidence`, `field_evaluation`, `functional_parity`, `controlled_rule_intake`, `existing_project_compatibility_mode`, `instruction_source`, `task_contracts`, `mechanical_integrity`, канонический JSON | Портов не знает. Зависимости: `fancy-regex`, `sha2`, `ryu` |
| `meridian-app` | Транспортные DTO, schema gate, конверсия в доменные типы, оркестрация `validate`-семейств, `rule_resolution`, `migration::{bundle, fragment, records, operations}`, `storage::{DatabaseRole, DatabaseMetadata, StorageRouter}`, `source_format` (YAML, JSON Schema, regions, Front Matter), `events` | Определяет порты `RecordRepository`, `EvidenceRepository`, `MigrationRepository`, `RoledStorage`, `WorkspaceReader`, `GitInspector`, `LinkTargetPort`, `FrozenSource`, `SourceResolver`, `EventSink`; реализует только `NoOpEventSink` |
| `meridian-storage-sqlite` | Схема и миграции, транзакции, FK, append-only, checkpoint/backup/restore, `migration_runs`/`migration_rollbacks`, канонический экспорт | Реализует `RecordRepository`, `EvidenceRepository`, `MigrationRepository`, `RoledStorage` |
| `meridian-cli` | Корень композиции: грамматика, коды, представление human/json, адаптеры файловой системы/Git/источника/checkpoint-каталога; проверка «Kernel под Git» для `doctor` (`kernel::check_kernel_under_git`, только чтение) | Реализует `WorkspaceReader` (`FsWorkspaceReader`), `GitInspector` (`RealGitInspector`, `CachedGitInspector`), `LinkTargetPort` (`FsLinkTarget`), `FrozenSource` (`GitFrozenSource`), `SourceResolver` (`FixtureSourceResolver`), `EventSink` (`RecordingEventSink`, не компонуется `main`) |

Порт `Clock` спецификации §3 не определён: ни `meridian-core`, ни
`meridian-app` не читают время; метки времени журнала — значения по
умолчанию схемы SQLite (`strftime('now')`) внутри адаптера хранения, не
предметные данные. Отклонения от архитектуры это не создаёт.

Известное состояние, не изменённое пакетом (исторически принято в 7a–7d):
модули `meridian-cli/src/commands/validate/{kernel_purity, document_identity, duplicate_fm, link_check, registry_schema, rule_resolution_fixtures, git_provenance, instance_context}`
выполняют файловый ввод-вывод и проверку сами, без порта `WorkspaceReader`;
`controlled_rule_intake`, `instruction_source_registry` и
`existing_project_compatibility_mode` читают файлы в CLI и передают
`serde_json::Value` в app. Исправления GAP-01…03 минимальны и эту структуру
не расширяют; `front_matter_block` вынесен в app как единственный владелец.

## 12. Четыре уровня, `meridian-core` и `meridian-app`

**Четыре уровня** (`transport DTO → syntax/schema → domain validation → valid
domain type`) для каждого operating-model семейства и миграции: schema gate
(`json_schema::validate`) → закрытые `#[serde(deny_unknown_fields)]` DTO
(`operating_model/*/dto.rs`) → конверсия с накоплением дефектов
(`convert.rs`, `record_resolution.rs`, `migration_boundary`) → доменные типы
`meridian-core` с приватными полями и валидирующими конструкторами. Отказ
на любом уровне останавливает запись до следующего; schema-clean провал DTO
— `ConversionDrift`, fail-closed. Доказательства:
`app: operating_model::functional_parity::tests::{evaluate_case_returns_schema_rejected_for_a_schema_invalid_document, evaluate_case_returns_conversion_drift_when_the_closed_dto_cannot_parse_a_schema_clean_document, evaluate_case_returns_domain_rejected_for_a_schema_clean_but_domain_invalid_document, evaluate_case_returns_accepted_for_a_real_valid_document}`;
`app: operating_model::execution_state::tests::schema_clean_conversion_drift_is_a_failure_even_for_an_invalid_fixture`;
классы schema short-circuit COMPAT-02/15/18. Исключения — адаптеры исходных
форматов (`source_format`) и перечисленные в §11 исторические CLI-модули.

**Нет внешнего I/O в `meridian-core`:** в production-части нет `std::fs`,
`std::env`, `std::process`, `std::net`, `std::io::{stdin,stdout,stderr}`,
`print!`/`eprintln!`, SQLite, `Command::new`, `SystemTime`; зависимости —
`fancy-regex`, `sha2`, `ryu`. Исполняемые проверки:
`cli: commands::validate::rust_architecture_conformance_6::evidence_and_field_evaluation_core_is_pure_and_the_app_is_adapter_free`,
`cli: commands::validate::rust_architecture_conformance_7::migration_and_qualification_core_is_pure`,
`[BRM] migration::migration_production_code_respects_crate_boundaries_and_never_reads_meridian_instance`.

**Нет конкретных адаптеров в `meridian-app`:** в production-части нет файлов,
окружения, процессов, сети, SQLite и зависимостей от `meridian-cli` или
`meridian-storage-sqlite`; единственная реализация порта — `NoOpEventSink`
(разрешена спецификацией §3). Исполняемые проверки:
`cli: commands::validate::rust_architecture_conformance_7::migration_and_qualification_app_routes_are_adapter_free`,
`cli: commands::validate::mechanical_integrity_boundary::structural_tests::production_has_exactly_one_concrete_workspace_reader_implementation`.

**Сеть:** в полном графе зависимостей workspace нет сетевого клиента
(`hyper`, `reqwest`, `ureq`, `tokio`, `mio`, `socket2` отсутствуют;
`jsonschema` собран с `default-features = false`).

## 13. Production panic audit

Сканер: production-часть каждого файла `src/` четырёх крейтов (до первого
`#[cfg(test)]`; файлы с заголовочным `#![cfg(test)]` и подключаемые только
`#[cfg(test)] mod tests;` исключены), шаблоны `unwrap()`, `expect(`,
`panic!`, `unreachable!`, `todo!`, `unimplemented!`, а также срезы строк по
байтовому диапазону.

До исправлений: **две аварии на внешнем вводе** (GAP-01,
`meridian-cli/src/commands/validate/document_identity.rs` и
`meridian-app/src/validation/mechanical_integrity/agent_instruction_identity.rs`,
`&text[4..end]`). После: **144** места, ни одно не принимает внешний ввод,
I/O, Git, SQLite, JSON, checkpoint или подтверждение:

| Класс | Мест | Основание |
|---|---:|---|
| `Diagnostic::new(<непустой литерал…>)` | 25 | шаблон сообщения начинается с фиксированного непустого текста |
| `Regex::new(<литерал>)` / `LazyLock` | 33 | константный шаблон; компиляция проверяется любым прогоном |
| `WorkspaceRelativePath::new(<литерал>)` | 12 | константный относительный путь |
| `NonEmptyString::new("<команда>: …")` в сводках событий CLI | 11 | фиксированный непустой префикс |
| Разворот после явной проверки формы/наличия | 30 | например `bundle["valid"].as_array().unwrap()` после `bundle_ok`, `yaml_raw.unwrap()` после проверки `is_none()` |
| Инварианты алгоритма ядра | 24 | resolver (элемент стека, непустая группа, голова при объявленном порядке, `unreachable!` для уже обработанного `NoMatch`, порядок полей `ProtocolRoute`), `json` (суррогатная пара, `peek` перед `next`, экспонента `ryu`), `divergence` (`verified` ⇒ состояние присутствует), `NORM_FIELDS` |
| Сериализация `serde_json::Value`/объекта в строку | 4 | не может завершиться ошибкой |
| Отравленный mutex `RecordingEventSink` | 2 | только после паники другого потока; `main` компонует `NoOpEventSink` |
| Прочие инварианты | 3 | повторная обёртка уже валидного `NonEmptyString`; `strip_prefix` пути, построенного под корнем шаблона; многострочный константный `Regex` |

Дополнительно: 11 250 случайных YAML-подобных входов через реальный бинарник
после GAP-03 — только коды 0/1/3, ни одной аварии.

Корректирующий раунд production-код `unwrap`/`expect`/`panic!`/срезов не
добавил: `registry_schema::{resolve_like_the_reference, normalize}`,
`kernel::check_kernel_under_git` и новое поле `doctor` возвращают ошибки
типами (`CollectError::CurrentDirectory`, `KernelGitError`); счёт 144 не
изменился.

## 14. Ворота

После архитектурного одобрения первый полный прогон §5.23.7 обнаружил один
устаревший закреплённый счётчик `schema_validated`/`schema_attempted` 13
вместо 14. После его единственного исправления полный рубеж повторён на
fingerprint
`0a54175ff8b02a094afcd2bf5e9c24c79eedb896a84862aa193f67e452646671`,
одинаковом до и после прогона. Все 14 команд завершились кодом 0:
`cargo test --workspace` — 1162 passed, 0 failed, 2 ignored; отдельный
ignored real-bundle запуск — 2 passed; conformance harness — 152 passed;
`kernel-validate.test.mjs` — 293 passed; Kernel-only validation — 0 failures,
9 warnings, `schema: 14/14`. Форматирование, сборка, clippy без предупреждений,
rustdoc, оба preflight и проверки diff/индекса также зелёные. Индекс при
передаче был пуст.
