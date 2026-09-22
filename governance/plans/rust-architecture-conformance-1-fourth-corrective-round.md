---
title: Rust architecture conformance 1 — четвёртый корректирующий раунд
document_type: plan
id: rust-architecture-conformance-1-fourth-corrective-round
status: changes-requested
scope: workspace
owner: workspace-owner
created: 2026-09-22
updated: 2026-09-22
---

# Задание исполнителю: `rust-architecture-conformance-1`, четвёртый корректирующий раунд

## 1. Статус и цель

Текущий вердикт архитектора: `CHANGES_REQUESTED` только для
`rust-architecture-conformance-1`.

Цель раунда — полностью устранить оставшиеся архитектурные и доказательные
дефекты пилота `controlled-rule-intake`. Частичное соответствие не принимается:
передача возвращается в `READY_FOR_ARCHITECT_REVIEW` только после выполнения
всех требований этого документа и самостоятельного архитектурного self-check.

Применяются правила наивысшего приоритета:

1. соблюдать выбранную Rust-архитектуру;
2. если Rust позволяет выразить функциональность лучше или надёжнее Node.js,
   использовать Rust-native решение;
3. сохранять бизнес-ценность и заявленный внешний контракт, а не дефекты или
   внутреннюю структуру Node.js;
4. не подменять архитектурную приёмку зелёным conformance-прогоном.

Нормативные источники:

- `standards/workspace/rust-migration-quality.md`;
- `governance/specifications/meridian-rust-target-architecture.md`;
- `governance/audits/meridian-rust-codebase-architecture-audit.md`;
- `COMPATIBILITY.md`;
- `governance/plans/meridian-rust-migration-program-plan.md` §5.14.

## 2. Граница роли и Git

Исполнитель пишет и проверяет код. Архитектор production-код не пишет.

В этом раунде запрещены любые Git-записи и изменение истории:

- `branch` / `switch`;
- `add`;
- `commit`;
- `merge` / `rebase`;
- `reset` / `stash`;
- `tag` / `push`.

Сохранить все чужие незакоммиченные изменения. Не выполнять широкую очистку
рабочего дерева.

## 3. Обязательные исправления

### 3.1. Восстановить правильную семантику `Prefetched` resolution

Текущая реализация изменила контракт композиции
`existing-project-compatibility-mode`:

- Node `buildSourceResolver` сначала разрешает discovered source по
  `source_ref.id`;
- затем `controlled-rule-intake` независимо сравнивает разрешённый
  `reference` с `payload.source_ref.reference`;
- Rust строит `Prefetched`-таблицу по `dto.reference` и выполняет lookup по
  `pinned.reference()`, поэтому правильный source id с неправильным pinned
  reference превращается в `NotFound` вместо точной reference-mismatch
  диагностики.

Требования:

1. `Prefetched` lookup должен использовать типизированный source id — тот же
   идентификатор, по которому выполняется композиция Node и прежняя Rust
   реализация этого call site.
2. Сравнение `reference` должно остаться в чистом
   `meridian-core::controlled_rule_intake::checks::check_source_ref`.
3. Не объявлять это новым intentional difference: исходное изменение не было
   принято как Rust-native улучшение.
4. Не возвращать resolver callback, closure-адаптер или production
   `impl SourceResolver` в `meridian-app`.
5. `SourceResolver` остаётся app-owned trait; production-реализация порта
   остаётся только в `meridian-cli`.

### 3.2. Не терять malformed prefetched source

Текущий `build_resolved_sources` использует
`serde_json::from_value(...).ok()` внутри `filter_map`. Существующий, но
malformed source молча исчезает, после чего `Prefetched` сообщает `NotFound`.
Это стирает различие между отсутствием и ошибкой преобразования.

Требования:

1. Представить результат построения таблицы типизированно и без silent drop.
   Допустимые направления:
   - значение таблицы содержит `Result<ResolvedInstructionSourceDto,
     SourceResolverError>`;
   - builder возвращает типизированный результат с накопленными diagnostics;
   - другое решение, сохраняющее те же различимые состояния и слои.
2. Три состояния обязаны оставаться различимыми:
   - source отсутствует;
   - source существует, но его transport/shape некорректен;
   - source успешно преобразован и передан в последующую предметную проверку.
3. Не использовать `.ok()`, `unwrap_or_default` или иной механизм, который
   превращает ошибку в отсутствие записи.
4. Не дублировать бизнес-проверки `controlled-rule-intake` в
   `existing_project_compatibility_mode`.

### 3.3. Сделать library-level Node/Rust proof действительно парным

Текущий Node-тест использует реальную valid fixture и в одном кандидате
создаёт два независимых дефекта:

1. удаляет обязательный `payload.classification_basis` — schema defect;
2. изменяет `origin.source_ref` — composite origin/source defect.

Текущий Rust-тест использует другой вход: пустой `rule_candidates` и
отсутствующий `registry_id`. В нём нет кандидата и нет composite-дефекта,
который мог бы быть остановлен schema short-circuit. Эти тесты не являются
matched pair.

Требования:

1. Node и Rust должны использовать одну и ту же реальную fixture либо
   доказуемо идентичную копию одного fixture-документа.
2. На обеих сторонах выполнить эквивалентные две мутации:
   - удалить `classification_basis`;
   - испортить `origin.source_ref`.
3. Прямой Rust library test обязан подтвердить одновременно:
   - присутствует schema diagnostic;
   - возвращена ровно одна диагностика;
   - отсутствует origin/source-reference diagnostic.
4. Прямой Node library test обязан подтвердить одновременно:
   - присутствует schema diagnostic;
   - присутствует origin/source-reference diagnostic;
   - возвращено не менее двух диагностик.
5. Self-test wrapper conformance оставить отдельным доказательством
   CLI-наблюдаемого равенства; не смешивать его с library-level divergence.
6. Комментарии и `COMPATIBILITY.md` должны ссылаться на фактически одинаковую
   мутацию, а не на «аналогичный по форме» другой документ.

### 3.4. Очистить публичный API `ReadChannel`

После `ReadChannel::try_new` любой публично создаваемый `ReadChannel` уже
согласован. Поэтому публичная функция
`check_read_channel_coherence(&ReadChannel)` для внешнего вызывающего всегда
возвращает пустой результат и создаёт ложную публичную поверхность.

Требования:

1. Сделать coherence helper приватной деталью fallible constructor; либо
   ввести честно названный unvalidated input / typed error API, если он
   действительно нужен нескольким границам.
2. Единственным публичным способом построения `ReadChannel` должен оставаться
   путь, сохраняющий его инвариант.
3. Production-код и тесты должны проверять отклонение через публичный
   `ReadChannel::try_new`, а не конструировать невалидный `ReadChannel`
   напрямую через приватные поля.
4. `instruction_source_registry::check_read_channel` должен продолжать
   делегировать единственной core-реализации правила; вторая копия сравнений
   запрещена.

## 4. Обязательные тесты

Добавить или скорректировать тесты, доказывающие минимум следующее:

1. правильный source id + неправильный pinned reference разрешается по id и
   даёт reference-mismatch diagnostic, а не `NotFound`;
2. отсутствующий source id даёт `NotFound`;
3. существующий malformed source даёт failure/shape diagnostic, а не
   `NotFound`;
4. построение и lookup prefetched-таблицы детерминированы;
5. Node и Rust library-level проверки используют один fixture и одинаковые
   две мутации;
6. incoherent `ReadChannel` невозможно получить через публичный API;
7. оба transport entrypoint для read-channel используют одну core-проверку;
8. ранее принятые 13 DTO-тестов и остальные доказательства пилота остаются
   зелёными и не ослабляются.

Отрицательный тест должен вызывать тот же production-алгоритм после
контролируемой мутации. Projection-only сравнение или проверка текста исходника
не считается доказательством поведения.

## 5. Архитектурный self-check перед техническим прогоном

До запуска conformance исполнитель обязан самостоятельно подтвердить:

- `SourceResolver` остаётся trait в `meridian-app`;
- production `impl SourceResolver` существует только в adapter/CLI-слое;
- `meridian-app` не содержит скрытого resolver-адаптера в форме closure;
- `Prefetched` — типизированные уже загруженные данные, а не обход порта для
  внешнего I/O;
- transport, syntax, domain validation, valid domain type и pure operation
  остаются раздельными;
- malformed input нигде не превращается в absence;
- невалидный `ReadChannel` непредставим публичным API;
- intentional differences перечислены полностью и доказаны корректными
  положительными/отрицательными тестами;
- ни один комментарий или transfer record не утверждает больше, чем доказано
  кодом и исполняемыми тестами.

Если хотя бы один пункт не выполнен, не передавать работу как
`READY_FOR_ARCHITECT_REVIEW`.

## 6. Обязательный технический прогон

После успешного архитектурного self-check выполнить:

```bash
cargo fmt --all -- --check
cargo build --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo test --workspace --all-targets --all-features
node --test test/conformance-harness.test.mjs
node test/kernel-validate.test.mjs
node scripts/preflight.mjs
git diff --check
```

Для каждого нового файла отдельно выполнить эквивалентную whitespace-проверку,
поскольку обычный `git diff --check` не проверяет untracked-файлы.

Если команда не прошла из-за ограничения среды (`EPERM`, Cargo lock,
запрещённый child process), повторить ту же команду через разрешённый механизм.
Не выдавать infrastructure-blocked проверку за verdict по коду.

## 7. Документация и передача

Обновить `governance/plans/meridian-rust-migration-program-plan.md` §5.14 и
итоговый статус программы фактическими результатами этого раунда. Не писать
`ACCEPTED`: решение принимает архитектор после независимой проверки.

Итоговая передача должна содержать:

1. статус `READY_FOR_ARCHITECT_REVIEW` только для
   `rust-architecture-conformance-1`;
2. точный список изменённых файлов;
3. описание выбранной модели prefetched resolution и трёх различимых
   состояний;
4. доказательство сохранения id-first resolution и отдельной reference-check;
5. описание одного и того же library-level mutation input для Node и Rust;
6. подтверждение закрытого публичного API `ReadChannel`;
7. полный результат каждой обязательной команды без пропусков;
8. текущий `HEAD`, состояние индекса и явное подтверждение отсутствия
   Git-записей.

Если полное исправление требует расширения области, остановиться и передать
конкретный блокер владельцу. Временная compatibility-обёртка, silent fallback
или частичное решение запрещены.
