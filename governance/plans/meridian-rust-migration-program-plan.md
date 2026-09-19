---
title: Миграция Meridian на Rust — программа и дорожная карта
document_type: plan
status: active
scope: workspace
owner: workspace-owner
created: 2026-09-14
updated: 2026-09-20
related_documents:
  - $MERIDIAN_KERNEL/governance/meridian-owner-intent-contract.md
  - $MERIDIAN_KERNEL/governance/decisions/meridian-rust-sqlite-architecture.md
  - $MERIDIAN_KERNEL/governance/specifications/meridian-rust-target-architecture.md
  - $MERIDIAN_KERNEL/governance/rfcs/meridian-cli-rfc.md
  - $MERIDIAN_KERNEL/standards/workspace/release-versioning.md
  - $MERIDIAN_KERNEL/standards/workspace/version-control-flow.md
  - $MERIDIAN_KERNEL/standards/workspace/instance-data-migration.md
  - $MERIDIAN_KERNEL/AGENTS.md
---

# Миграция Meridian на Rust — программа и дорожная карта

> Канонизировано в Kernel 2026-09-19 (`$MERIDIAN_KERNEL/AGENTS.md` §1–§2,
> §10) вместе с четырьмя другими документами программы переноса на Rust;
> ранее велось в отдельном репозитории Instance, теперь замороженном для
> чтения как источник миграции. При переносе: (1) упоминания конкретного
> продуктового литерала в историческом разделе §5.1 (пакет 3) обобщены до
> «продуктовый литерал текущего Instance», без изменения зафиксированных
> доказательств; (2) зафиксирована приёмка пакета 5 и введён обязательный
> корректирующий рубеж `instance-repository-retirement-baseline` перед
> пакетом 6 — это содержательное изменение состояния программы, а не
> редакционная правка, и описано ниже как часть настоящей ревизии.
> Идентификатор Concord и стабильные идентификаторы
> `concord-meridian-onboarding`/`concord-meridian-field-evaluation` сохранены
> как в источнике: условие паузы касается конкретно Concord, а не запрещает
> любое полевое применение вообще. Всюду ниже голая ссылка на
> `meridian-operating-upgrade-plan.md` или
> `meridian-workspace-compatibility-program-plan.md` (без пути
> `$MERIDIAN_KERNEL/…`) указывает на замороженный исторический источник
> происхождения — план уже завершённой (`meridian-operating-upgrade-plan.md`)
> либо отдельно управляемой (`meridian-workspace-compatibility-program-plan.md`)
> программы, а не на действующую зависимость этого плана; чтение и
> продолжение программы `meridian-rust-migration` не требуют
> `MERIDIAN_INSTANCE`.

Канонический идентификатор программы: `meridian-rust-migration`.

Статус программы: **`active`**. Активационный рубеж §1 выполнен: Kernel `0.6.0`
принят и интегрирован (`meridian-operating-upgrade-plan.md` §12). Пакет 1
(`rust-workspace-foundation`) принят и интегрирован в интеграционную линию
Kernel (§4, §5.1). Пакет 2 (`rust-conformance-harness`) принят и
интегрирован в интеграционную линию Kernel (§4, §5.1). Пакет 3
(`rust-domain-core`) принят и интегрирован в интеграционную линию Kernel
(§4, §5.1). Пакет 4 (`rust-source-format-adapters`) принят и интегрирован
в интеграционную линию Kernel (§4, §5.1). Пакет 5 (`rust-rule-resolution`)
принят и интегрирован в интеграционную линию Kernel (§4, §5.1).
Корректирующий рубеж `instance-repository-retirement-baseline` (§5.2) —
вывод отдельного репозитория Instance из роли активного центра управления
разработкой Meridian и перенос канонических документов программы в Kernel —
**принят и интегрирован** в интеграционную линию Kernel (§4, §5.2). Пакет 6
(`sqlite-storage-adapter`) готов к началу (`current_package_status: ready`,
§10), но этой документальной синхронизацией **не начинается** — начало
пакета остаётся отдельным исполнением.

## 1. Активационный рубеж

Эта программа не открывается ни одним своим пакетом до тех пор, пока не
выполнено оба условия:

1. программа `meridian-operating-upgrade` принята и интегрирована — то есть
   пакеты 1–9 её дорожной карты приняты, `meridian-operating-upgrade-release`
   (пакет 10) выпущен, и выполнены все пункты выпускного рубежа
   (`meridian-operating-upgrade-plan.md` §12);
2. Concord остаётся на паузе (`meridian-owner-intent-contract.md` §20, §24)
   — активация этой программы Rust-миграции сама по себе не является
   возобновлением Concord и не требует его возобновления.

Условие 1 выполнено: Kernel `0.6.0` принят и интегрирован
(`meridian-operating-upgrade-plan.md` §12, §15–16). Условие 2 сохраняется:
Concord остаётся на паузе. Активационный рубеж пройден, и пакет 1
(`rust-workspace-foundation`) уже принят и интегрирован под этой активацией
(§5.1) — активация программы этой ревизией не переоткрывается и не
переисполняется задним числом.

## 2. Зачем нужна отдельная программа

Перенос исполняющей механики Kernel на другой язык — это не рефакторинг одного
модуля и не отдельный пакет существующей операционной программы: он затрагивает
каждый принятый контракт операционной модели (`meridian-operating-upgrade-plan.md`)
и модели рабочих пространств (`meridian-workspace-compatibility-program-plan.md`),
вводит первое постоянное хранилище данных и меняет инструмент, которым
проверяется каждая другая программа Meridian. Он классифицируется как
`initiative` (`meridian-owner-intent-contract.md` §12,
`meridian-operating-upgrade-plan.md` §5) и декомпозируется на пакеты ниже, а не
исполняется как единый неделимый `REFACTOR`.

## 3. Что уже решено и не проектируется заново здесь

- Язык (Rust), монолитная форма, синхронное предметное ядро, порты и адаптеры,
  запрет `unsafe`, отказ от Big Bang Rewrite, отказ от постоянной двойной
  реализации, отказ от микросервисов, SQLite как первое хранилище, отложенный
  PostgreSQL — `meridian-rust-sqlite-architecture.md` (ADR, `accepted`).
- Границы крейтов Cargo workspace, схема SQLite, состав типов предметной
  области, разделение уровней проверки, поверхность CLI —
  `meridian-rust-target-architecture.md`.
- Состав команд CLI, паритетный харнесс, маршрут замещения Node-реализации —
  `meridian-cli-rfc.md` (`in-review`).

Эта программа их не переоткрывает: она определяет **порядок**, в котором эти
уже принятые решения превращаются в код, и **ворота**, которые обязаны пройти
на каждом шаге.

## 4. Пакеты

Пакеты выполняются в строгом порядке; пакет N не начинается, пока пакет N−1 не
принят и не интегрирован, за исключением явно отмеченных параллельных долей.

| № | Название и идентификатор | Результат | Зависимость | Состояние |
|---:|---|---|---|---|
| 1 | Основание рабочего пространства Rust (`rust-workspace-foundation`) | Cargo workspace в репозитории Kernel: четыре крейта, `#![forbid(unsafe_code)]`, выбор конкретных крейтов (сериализация, JSON Schema, SQLite-драйвер), CI-сборка workspace | §1 (активационный рубеж) | accepted — принят и интегрирован (§5.1) |
| 2 | Набор проверки соответствия (`rust-conformance-harness`) | Паритетный харнесс — независимый от Rust-реализации механизм запуска, нормализации и сравнения вердиктов; на этом пакете проверяется на контролируемом корпусе заведомо совпадающих и намеренно расходящихся пар вердиктов (реальной Rust-поверхности ещё нет); реальное сравнение Node/Rust наращивается начиная с пакета 4 (§6.2) | 1 | accepted — принят и интегрирован (§5.1) |
| 3 | Предметное ядро (`rust-domain-core`) | `meridian-core`: типы (§5 технической спецификации), resolver, конфликты, планы миграции, доказательства, вердикты и диагностика — без файлов, Git, БД, сети, env, вывода | 1–2 | accepted — принят и интегрирован (§5.1) |
| 4 | Адаптеры исходных форматов (`rust-source-format-adapters`) | Строгий слой поверх YAML/JSON Schema библиотек (allowlist полей и форматов, strict-lint YAML) — построчный паритет с `scripts/lib/yaml.mjs` и `scripts/lib/json-schema.mjs` | 3 | accepted — принят и интегрирован (§5.1) |
| 5 | Разрешение норм (`rust-rule-resolution`) | Порт `scripts/rule-resolver.mjs` и композитных проверок контрактов операционной модели (`scripts/lib/*.mjs`) в `meridian-app` поверх портов `meridian-core` | 3–4 | accepted — принят и интегрирован (§5.1) |
| — | Корректирующий рубеж: вывод Instance-репозитория из эксплуатации (`instance-repository-retirement-baseline`) | Устранение отдельного репозитория Instance как активного центра управления разработкой Meridian; перенос пяти канонических документов в `governance/` Kernel; самодостаточный по умолчанию `preflight`/`kernel-validate`; не начинает `sqlite-storage-adapter` | 5 | accepted — принят и интегрирован (§5.2) |
| 6 | Хранилище SQLite (`sqlite-storage-adapter`) | `meridian-storage-sqlite`: схема §4 технической спецификации, транзакционная запись, включённые foreign keys, неизменяемые редакции и доказательства, идемпотентный импорт, резервная копия, канонический экспорт | 3, 5, `instance-repository-retirement-baseline` | ready — готов к началу, не начат (§10) |
| 7 | Основа CLI (`meridian-cli-foundation`) | `meridian-cli`: `init`, `doctor`, `validate`, `resolve`, `export`, `--format human|json`, стабильные коды завершения, разделение stdout/stderr | 5–6 | planned |
| 8 | Миграционный CLI (`meridian-cli-migration`) | `import`, `migration plan|apply|verify|rollback` — реализация контракта `instance-data-migration.md` поверх `meridian-storage-sqlite`; `plan` не изменяет состояние; `apply` поддерживает `--dry-run` и явное подтверждение. **Обязан доказать** (§6.5a): полный импорт всех записей замороженного источника миграции без потерь; эквивалентность применимых норм между Node-эталоном и импортированным состоянием; идемпотентность повторного импорта; обратимость (`apply → rollback`); отсутствие эксплуатационного чтения через `$MERIDIAN_INSTANCE` в штатной работе выпускаемого бинарника | 6–7 | planned |
| 9 | Квалификация равенства (`rust-parity-qualification`) | Полный паритетный прогон (полный набор ворот §6.1–§6.6) на всех классах тестовых деревьев; 0 расхождений вердиктов | 2, 4–8 | planned |
| 10 | Выпуск Rust Meridian (`meridian-rust-release`) | Один устанавливаемый бинарник, выпускная ветка, версия, журнал изменений, возврат в интеграционную линию — выпускной рубеж §7 ниже | 9 | planned |

### После выпуска (вне этой программы, но зависимые от неё)

| Название и идентификатор | Результат | Зависимость |
|---|---|---|
| Повторное подключение Concord (`concord-meridian-onboarding`) | Переклассификация Concord по выпущенной Rust-модели, без автоматического продолжения со старой точки G0 | 10 |
| Полевая оценка Concord (`concord-meridian-field-evaluation`) | Первое реальное полевое применение Rust Meridian | `concord-meridian-onboarding` |

Эти два пункта не являются пакетами настоящей программы: они наступают после
её выпускного рубежа и управляются условием возобновления Concord
(`meridian-owner-intent-contract.md` §24, §20;
`meridian-operating-upgrade-plan.md` §13, распространённое этим документом на
`meridian-rust-release`).

## 5. Для каждого пакета

Ниже — обязательный состав описания каждого пакета §4, единый шаблон,
которому обязана следовать фактическая запись о его приёмке при исполнении.
Настоящий документ не заполняет фактические значения — они появляются при
приёмке каждого пакета отдельной документальной синхронизацией, по образцу,
уже принятому в `meridian-operating-upgrade-plan.md` §11.1 и
`meridian-workspace-compatibility-program-plan.md` §3.1.

- **результат** — что именно принято (артефакт, схема, крейт, команда);
- **зависимости** — какие предыдущие пакеты этой программы и какие внешние
  решения (ADR, техническая спецификация, RFC) пакет использует как данность;
- **критерии приёмки** — проверяемые условия, при которых пакет считается
  завершённым, включая применимые на этом шаге пункты ворот (§6);
- **запрещённое расширение** — что пакет не делает, даже если по ходу работы
  выглядит удобным сделать заодно (в первую очередь: не добавляет новую
  функциональность сверх достижения паритета, §8);
- **проверочные доказательства** — фактический вывод харнесса, тестов,
  gate-прогонов; точный package commit и merge commit по применимому Git-flow
  (`version-control-flow.md`);
- **условие перехода дальше** — что именно должно быть истинно, чтобы
  следующий пакет мог начаться.

### 5.1. Принятые пакеты и доказательства интеграции

- `rust-workspace-foundation` (пакет 1): пакет Ядра
  `693f1d4ac77d4dc63be35d90d14cf009fa3e76e3` принят отдельным коммитом
  слияния `357ac0f98e44dc47734e8f7243e0e3f5d0f607f6` (GitHub PR #33: база
  `dev`, источник `feature/rust-workspace-foundation`, состояние `MERGED`;
  обычный merge commit с двумя родителями — squash, rebase и fast-forward без
  отдельного merge-коммита не применялись). Дерево коммита слияния совпадает
  с деревом пакетного коммита; пакетный коммит достижим из `origin/dev`.
  Результат: Cargo workspace в корне Kernel с ровно четырьмя крейтами
  (`meridian-core`, `meridian-app`, `meridian-storage-sqlite`,
  `meridian-cli`), допустимым направлением зависимостей, `#![forbid(unsafe_code)]`
  во всех четырёх крейтах и на уровне `[workspace.lints.rust]`, зафиксированными
  библиотеками (`serde`/`serde_json`, `jsonschema`, `serde-saphyr`,
  `rusqlite`) и отдельной задачей CI `Rust workspace validate (foundation)`
  (`.github/workflows/gate.yml`: `cargo fmt --check`; `clippy`/`test`/`doc`
  через `--locked`). Переданные проверочные доказательства (пакетный коммит и
  коммит слияния): `cargo fmt --all -- --check`; `cargo clippy --locked
  --workspace --all-targets --all-features -- -D warnings`; `cargo test
  --locked --workspace`; `cargo doc --locked --workspace --no-deps`; `node
  test/kernel-validate.test.mjs`: 292 passed, 0 failed; полный
  `kernel-validate`: новых ошибок пакета нет; pre-push: синтетический полный
  прогон — 0 failing, 0 warnings. Пакет не переносит resolver, валидатор,
  предметные типы, порты/адаптеры, схему SQLite или CLI-команды — это предмет
  последующих пакетов программы. Условие перехода дальше выполнено: пакет 2
  (`rust-conformance-harness`) переведён в состояние `ready` (§10) — этой
  записью пакет 2 не начинается.

- `rust-conformance-harness` (пакет 2): **результат** — независимый от
  будущей Rust-реализации набор проверки соответствия: механизм запуска,
  нормализации и сравнения вердиктов; контролируемый корпус заведомо
  совпадающих и намеренно расходящихся пар вердиктов; чёрноящичные тесты
  харнесса; обновление регрессии `test/pre-push-git-isolation.test.mjs`,
  чтобы синтетический Kernel включал новый тест харнесса при проверке
  изоляции `hooks/pre-push` от внешнего Git-окружения; включение проверок в
  локальный (`hooks/pre-push`) и удалённый (`.github/workflows/gate.yml`)
  рубежи. **Зависимости** — пакет 1 (`rust-workspace-foundation`, §5.1,
  `accepted`). Пакетный коммит Ядра —
  `e3d9d57e85ad4dda5dc8c5eec28dfbcfb36d233f` (`feat(conformance): добавить
  независимый харнесс вердиктов`, ветка `feature/rust-conformance-harness`),
  принят отдельным коммитом слияния в `origin/dev` —
  `0281dadfa45fbe1fcc9998be80abad696002bb67`, обычный merge commit с двумя
  родителями: первый родитель `357ac0f98e44dc47734e8f7243e0e3f5d0f607f6`,
  второй родитель `e3d9d57e85ad4dda5dc8c5eec28dfbcfb36d233f`. Пакетный коммит
  достижим из `origin/dev` (`git merge-base --is-ancestor` — подтверждено).
  Дерево пакетного коммита совпадает с деревом коммита слияния (`git diff
  --exit-code <package-commit>^{tree} <merge-commit>^{tree}` — код завершения
  0, без вывода): diff принятого пакета не переписан при слиянии. Состав
  принятого результата — ровно восемь файлов: `.github/workflows/gate.yml`,
  `CHANGELOG.md`, `hooks/pre-push`, `test/conformance-harness.test.mjs`,
  `test/pre-push-git-isolation.test.mjs`,
  `verification/conformance-harness/README.md`,
  `verification/conformance-harness/conformance-harness.mjs`,
  `verification/conformance-harness/fixtures/conformance-harness.fixtures.json`.
  Переданные проверочные доказательства: `git diff --check` — код завершения
  0; `sh -n hooks/pre-push` — синтаксически корректен; `node
  test/conformance-harness.test.mjs` — 37 passed, 0 failed, код завершения 0;
  `node test/pre-push-git-isolation.test.mjs` — 8 passed, 0 failed, код
  завершения 0; успешный запуск с отдельным пустым `TMPDIR` не оставил
  временных файлов; форсированно падающий сценарий также не оставил временных
  файлов; набор проверки соответствия обнаруживает намеренные расхождения и
  принимает заведомо совпадающие пары. **Запрещённое расширение, которого
  пакет не выполнял**: реальная поверхность сравнения Node/Rust (она
  начинается с пакета 4, §6.2); предметный Rust-код; SQLite; интерфейс
  командной строки; перенос разрешения норм. **Условие перехода дальше**:
  пакет 3 (`rust-domain-core`) переведён в состояние `ready` (§10) — этой
  записью пакет 3 не начинается.

- `rust-domain-core` (пакет 3): **результат** — `meridian-core`: строгие
  предметные типы (`SemanticId`, `WorkspaceId`, `RepositoryId`, `Revision`,
  `ContentDigest`, `EvidenceRef`, `Scope`, `Origin`, `Authority`, `Verdict`,
  `Diagnostic` — §5 технической спецификации); чистый resolver (перенос
  `resolveRules` из `scripts/rule-resolver.mjs` с сохранением потока
  управления и семантики каждого правила, включая атомарный конструктор
  `ProtocolRoute` с вычисляемым, а не произвольно задаваемым ключом
  разрешения равенства маршрутов); структуры и проверки плана миграции
  Экземпляра (девять свойств `instance-data-migration.md`, включая
  побайтовую совместимость `plan_fingerprint`/`idempotency_key` и полную
  семантическую проверку `ContentEnvelope`, включая каноничность
  JSON-содержимого); структуры доказательств и правила связывания
  утверждения с проверяемым результатом (`evidence-and-handoff-contract.md`
  §5, предметное подмножество без полного контракта передачи); `crate::json`
  — приватный, не экспортируемый из крейта канонический JSON writer/reader,
  используемый только для двух узких внутренних целей (сериализация плана
  миграции в канонический байтовый вид и проверка `ContentEnvelope`), а не
  общий транспортный JSON-парсер. `#![forbid(unsafe_code)]` сохранён;
  `meridian-core` не читает файлы, Git, БД, сеть, переменные окружения и не
  производит вывод; `cargo tree -p meridian-core` — только `sha2`, `ryu` и
  их транзитивные зависимости, никакого нового прямого крейта сверх этих
  двух. Состав принятого результата — 43 файла: `CHANGELOG.md`, `Cargo.lock`,
  `meridian-core/Cargo.toml`, `meridian-core/src/lib.rs` и 39 новых файлов
  под `meridian-core/src/{evidence,migration,resolver,types}/`,
  `meridian-core/src/json.rs`, `meridian-core/src/json_number_corpus_data.rs`
  и семь файлов `meridian-core/tests/*.rs` (относительно предыдущего
  принятого состояния `dev` — коммита слияния пакета 2,
  `0281dadfa45fbe1fcc9998be80abad696002bb67`).

  **Зависимости** — пакет 1 (`rust-workspace-foundation`, §5.1, `accepted`) и
  пакет 2 (`rust-conformance-harness`, §5.1, `accepted`); внешние решения,
  принятые как данность и не пересматриваемые этим пакетом:
  `meridian-rust-sqlite-architecture.md` (ADR, `accepted`) и §5
  `meridian-rust-target-architecture.md` (состав предметных типов).

  **Критерии приёмки** — общие ворота §6.1 для пакета с Rust-кодом
  (`cargo fmt --check`; `cargo clippy --workspace --all-targets
  --all-features -- -D warnings`; `cargo test --workspace`; `cargo doc
  --workspace --no-deps`) плюс уже применимые на этом шаге перенесённые
  регрессионные тесты пакета 2 (`test/conformance-harness.test.mjs`,
  `test/pre-push-git-isolation.test.mjs`) без изменения их собственного
  поведения; поведенческий паритет с замороженным Node-эталоном для
  переносимой на этом шаге поверхности (§8) — не только внутренняя
  согласованность Rust-реализации, а byte-for-byte/значение-в-значение
  сравнение с фактическим выводом настоящих `scripts/rule-resolver.mjs` и
  `scripts/lib/instance-data-migration.mjs`, полученным независимыми
  эталонными тестами внутри `meridian-core/tests/`, вызывающими реальные
  Node-функции — это ОТДЕЛЬНО от формального паритетного харнесса пакета 2
  (`verification/conformance-harness/`), который на реальной поверхности
  Node/Rust применяется только начиная с пакета 4 (§6.2): харнесс сравнивает
  вывод ДВУХ исполняемых поверхностей, а Rust-поверхности с интерфейсом
  командной строки на этом шаге ещё не существует.

  **Запрещённое расширение, которого пакет не выполнял** — адаптеры исходных
  форматов (YAML/JSON Schema — они начинаются с пакета 4, §6.3); хранилище
  SQLite; интерфейс командной строки; файловый, Git-, сетевой ввод-вывод и
  чтение переменных окружения (`meridian-core` их не выполняет, §5); общий
  транспортный JSON-парсер (`crate::json` остаётся приватным и используется
  только для двух узких внутренних целей — канонической сериализации плана
  миграции и проверки `ContentEnvelope`, а не как общесистемный парсер); и
  полный контракт передачи `evidence-and-handoff-contract.md` (перенесено
  только предметное подмножество структур доказательств — правила связывания
  утверждения с проверяемым результатом, без остальных разделов контракта).

  **Проверочные доказательства** — пакетный коммит Ядра
  `4654c058570134c672366f52f7ae451d97f71047` (`feat(core): добавить
  предметное ядро Rust`, ветка `feature/rust-domain-core`), принят отдельным
  коммитом слияния в `origin/dev` — `99ca17d37f3d55e862f59afa6bd6a2a12aeaf223`
  (GitHub PR #35: база `dev`, источник `feature/rust-domain-core`, состояние
  `MERGED`). Обычный merge commit с двумя родителями: первый родитель
  `0281dadfa45fbe1fcc9998be80abad696002bb67` (коммит слияния пакета 2),
  второй родитель `4654c058570134c672366f52f7ae451d97f71047` (пакетный
  коммит). Пакетный коммит достижим из коммита слияния (`git merge-base
  --is-ancestor 4654c058570134c672366f52f7ae451d97f71047
  99ca17d37f3d55e862f59afa6bd6a2a12aeaf223` — подтверждено). Дерево
  принятого пакета совпадает с деревом коммита слияния (`git diff
  --exit-code 4654c058570134c672366f52f7ae451d97f71047^{tree}
  99ca17d37f3d55e862f59afa6bd6a2a12aeaf223^{tree}` — код завершения 0, без
  вывода): diff принятого пакета не переписан при слиянии. Все четыре
  удалённые проверки PR #35 завершились успешно (`Kernel validate (synthetic
  instance)` ×2, `Rust workspace validate (foundation)` ×2 — все `pass`).
  Локальные и послесливные проверки (независимо перепроверены настоящей
  синхронизацией на пакетном коммите): `cargo fmt --all -- --check`; `cargo
  clippy --workspace --all-targets --all-features -- -D warnings`; `cargo
  test --workspace --all-targets --all-features`; `cargo test --workspace
  --doc`; `cargo doc --workspace --no-deps`; `git diff --check` — все без
  ошибок и предупреждений. `meridian-core`: 160 модульных тестов (включая
  детерминированный дифференциальный корпус из 6201 значения для
  `js_number_to_string`, сверенный с фактическим `JSON.stringify`) и 121
  интеграционный тест в `meridian-core/tests/` (26 `resolver_acceptance.rs`,
  35 `migration_acceptance.rs`, 10 `evidence_acceptance.rs`, 5
  `migration_fingerprint_reference.rs`, 3 `resolver_route_reference.rs`, 41
  `json_canonical_reference.rs`, 1 `json_number_corpus_reference.rs`) — все
  пройдены, 0 отказов. `node test/conformance-harness.test.mjs`: 37 passed, 0
  failed. `node test/pre-push-git-isolation.test.mjs`: 8 passed, 0 failed.

  **Исправляющий пакет kernel-purity, зафиксированный отдельно и правдиво** —
  после интеграции коммита слияния `99ca17d37f3d55e862f59afa6bd6a2a12aeaf223`
  фактический запуск `kernel-validate` против канонического продуктового
  Instance (`$MERIDIAN_INSTANCE`) обнаружил 18 новых отказов kernel-purity: в
  девяти файлах `meridian-core` использовался продуктовый литерал текущего
  Instance (`meridian-core/src/migration/canonical.rs`,
  `meridian-core/src/migration/types.rs`,
  `meridian-core/src/resolver/applicability.rs`,
  `meridian-core/src/resolver/work_item.rs`,
  `meridian-core/src/types/repository_id.rs`,
  `meridian-core/tests/migration_acceptance.rs`,
  `meridian-core/tests/migration_fingerprint_reference.rs`,
  `meridian-core/tests/resolver_acceptance.rs`,
  `meridian-core/tests/resolver_route_reference.rs`). Исправление: заменило
  продуктовые идентификаторы нейтральными тестовыми значениями; заново
  получило четыре эталонных отпечатка настоящим Node-алгоритмом; не
  ослабляло kernel-purity; не добавляло исключений; не меняло
  производственный алгоритм предметного ядра. Пакетный коммит исправления
  Ядра — `55010dd0f7b703a885519187ac22e5cef4772bf4` (`fix(core): убрать
  продуктовые литералы из предметного ядра`, ветка
  `bugfix/rust-domain-core-kernel-purity`), принят отдельным коммитом слияния
  в `origin/dev` — `a4bfa023f50452a4c44493b01af293832166253f` (GitHub PR #37:
  база `dev`, источник `bugfix/rust-domain-core-kernel-purity`, состояние
  `MERGED`). Обычный merge commit с двумя родителями: первый родитель
  `99ca17d37f3d55e862f59afa6bd6a2a12aeaf223` (коммит слияния основного
  пакета, PR #35), второй родитель `55010dd0f7b703a885519187ac22e5cef4772bf4`
  (исправляющий пакетный коммит). Исправляющий пакетный коммит достижим из
  коммита слияния (`git merge-base --is-ancestor
  55010dd0f7b703a885519187ac22e5cef4772bf4
  a4bfa023f50452a4c44493b01af293832166253f` — подтверждено). Дерево
  исправляющего пакетного коммита совпадает с деревом коммита слияния (`git
  diff --exit-code 55010dd0f7b703a885519187ac22e5cef4772bf4^{tree}
  a4bfa023f50452a4c44493b01af293832166253f^{tree}` — код завершения 0, без
  вывода). Коммит слияния несёт нормативный заголовок `merge(dev): принять
  bugfix/rust-domain-core-kernel-purity — восстановить чистоту предметного
  ядра` и тело из обязательных четырёх разделов («Что изменено», «Зачем»,
  «Проверки», «Связано» — `version-control-flow.md` §12.1–§12.2). Все
  удалённые проверки PR #37 завершились успешно (`Kernel validate (synthetic
  instance)` ×2, `Rust workspace validate (foundation)` ×2 — все `pass`).

  Итоговые проверочные доказательства исправления (независимо перепроверены
  настоящей синхронизацией на исправляющем пакетном коммите): поиск
  продуктового литерала по всему `meridian-core` — код завершения 1, без
  вывода; `cargo fmt --all -- --check`; `cargo clippy --workspace
  --all-targets --all-features -- -D warnings`; `cargo test --workspace
  --all-targets --all-features`: 160 модульных и 121 интеграционный тест
  `meridian-core` — все пройдены, 0 отказов; `cargo test --workspace --doc`;
  `cargo doc --workspace --no-deps`; `node test/conformance-harness.test.mjs`:
  37 passed, 0 failed; `node test/pre-push-git-isolation.test.mjs`: 8 passed,
  0 failed; `node test/kernel-validate.test.mjs`: 292 passed, 0 failed, 0
  skipped; фактический `kernel-validate` после слияния (против
  `$MERIDIAN_INSTANCE`): 5 failing, 9 warnings, 53 informational/ok — все
  пять отказов относятся к ранее зафиксированным темам Instance, не
  связанным с этим пакетом (стек-профиль конкретного продуктового
  репозитория вне пула и провенанс регионов его `AGENTS.md`), а не к
  `meridian-core`; kernel-purity: 263 отслеживаемых текстовых файла чисты.
  `git diff --check` на исправляющем пакетном коммите — без ошибок.

  **Исправленная процедурная попытка (PR #36), зафиксированная отдельно**:
  до PR #37 исправляющий пакетный коммит
  `55010dd0f7b703a885519187ac22e5cef4772bf4` был впервые опубликован через PR
  #36 (источник — ветка с запрещённым префиксом
  `fix/rust-domain-core-kernel-purity`, а не обязательным `bugfix/` —
  `version-control-flow.md`). Проверка имени ветки корректно отказала; PR #36
  закрыт без слияния (`state: CLOSED`, без `mergeCommit`); неправильная
  ветка `fix/rust-domain-core-kernel-purity` удалена из `origin` (проверено:
  `git ls-remote` не возвращает эту ссылку). Содержимое коммита не
  изменялось: тот же коммит `55010dd0f7b703a885519187ac22e5cef4772bf4`
  опубликован повторно через допустимую ветку
  `bugfix/rust-domain-core-kernel-purity` и принят посредством PR #37. PR #36
  не представляется частью принятой истории — это закрытая, неинтегрированная
  попытка; она не входит в цепочку принятия пакета 3 (настоящий §5.1) и не
  изменяет проверочные доказательства выше.

  **Процедурное отклонение управления (PR #35), зафиксированное отдельно и
  правдиво**: опубликованный коммит слияния
  `99ca17d37f3d55e862f59afa6bd6a2a12aeaf223` несёт сообщение
  `feat(core): добавить предметное ядро Rust` — заголовок пакетного коммита
  БЕЗ ЕДИНОГО символа тела — вместо обязательной формы `merge(dev): принять
  feature/rust-domain-core — <результат по-русски>` (`version-control-flow.md`
  §12.1) и без обязательных четырёх разделов тела «Что изменено», «Зачем»,
  «Проверки», «Связано» (`version-control-flow.md` §12.2); сам пакетный
  коммит `4654c058…` эти четыре раздела корректно несёт — отклонение
  затрагивает только отдельный, самостоятельный коммит слияния, а не
  пакетный коммит. Опубликованная история не переписывается. Это отдельное,
  самостоятельное отклонение PR #35; оно не заменяет, не отменяет и не
  смешивается с ранее записанными отклонениями других пакетов и программ
  (`workspace-scope-model`, MR !15–!29, PR #21 и другие, зафиксированные в
  `meridian-operating-upgrade-plan.md`). Техническая целостность пакета —
  дерево, достижимость, набор проверок — этим отклонением не затронута и
  независимо подтверждена выше. Правильный текст коммита слияния
  (`merge(dev): ...` с четырьмя разделами) обязан быть подготовлен до
  следующего слияния в `dev` в рамках этой программы; состояния пакетов
  программы `meridian-rust-migration` этим отклонением не меняются.
  Предупреждающее действие для следующего слияния уже выполнено: следующее
  слияние в `dev` в рамках этой программы — исправляющий PR #37 (коммит
  слияния `a4bfa023f50452a4c44493b01af293832166253f`) — опубликовано с
  корректным нормативным сообщением (`merge(dev): принять
  bugfix/rust-domain-core-kernel-purity — восстановить чистоту предметного
  ядра` и все четыре обязательных раздела тела).

  **Итоговый статус пакета 3** — пакет `rust-domain-core` считается принятым
  только как совокупность основного пакета (пакетный коммит
  `4654c058570134c672366f52f7ae451d97f71047`, коммит слияния
  `99ca17d37f3d55e862f59afa6bd6a2a12aeaf223`, PR #35) и последующего
  исправления kernel-purity (пакетный коммит
  `55010dd0f7b703a885519187ac22e5cef4772bf4`, коммит слияния
  `a4bfa023f50452a4c44493b01af293832166253f`, PR #37); ни один из двух
  коммитов слияния сам по себе не составляет полной приёмки пакета.

  **Условие перехода к пакету 4** — пакет 4 (`rust-source-format-adapters`)
  переведён в состояние `ready` (§10) только после интеграции обоих коммитов
  слияния — этой записью пакет 4 не начинается.

- `rust-source-format-adapters` (пакет 4): пакетный коммит Ядра
  `1601b75895df0e83caf4e0c30dacf32cfe7cf852` (ветка
  `feature/rust-source-format-adapters`) принят отдельным коммитом слияния в
  `origin/dev` — `7fe59830edebdb500a1b6fece175ec6d50314430` (GitHub PR #38:
  база `dev`, источник `feature/rust-source-format-adapters`). Обычный merge
  commit с двумя родителями: первый родитель
  `a4bfa023f50452a4c44493b01af293832166253f` (предыдущий принятый коммит
  слияния — исправление kernel-purity пакета 3, PR #37), второй родитель
  `1601b75895df0e83caf4e0c30dacf32cfe7cf852` (пакетный коммит). Пакетный
  коммит достижим из линии `dev` (`git merge-base --is-ancestor
  1601b75895df0e83caf4e0c30dacf32cfe7cf852
  7fe59830edebdb500a1b6fece175ec6d50314430` — подтверждено; коммит слияния,
  в свою очередь, лежит на `dev`). Дерево коммита слияния совпадает с
  деревом пакетного коммита (`git diff --exit-code
  1601b75895df0e83caf4e0c30dacf32cfe7cf852^{tree}
  7fe59830edebdb500a1b6fece175ec6d50314430^{tree}` — код завершения 0, без
  вывода): diff принятого пакета не переписан при слиянии.

  **Результат** — строгие адаптеры YAML и JSON Schema Draft 7 в
  `meridian-app::source_format`: граница поддерживаемого поднабора (allowlist
  полей и форматов, strict-lint YAML) и реальное сравнение Node.js/Rust через
  общий состязательный корпус. Файловый, Git, сетевой, env-, процессный и
  CLI-ввод-вывод в производственном слое отсутствуют.

  **Проверочные доказательства**, зафиксированные из пакетного коммита:
  `cargo fmt --check`; `cargo clippy --workspace --all-targets
  --all-features -- -D warnings`; `cargo test --workspace --all-targets
  --all-features`; `cargo test --doc`; `cargo doc --workspace --no-deps`;
  conformance harness — 39/39; pre-push Git isolation — 8/8; kernel-validate
  regression — 292 passed, 0 failed, 0 skipped; полный pre-push-гейт —
  зелёный.

  **Условие перехода к пакету 5** — пакет 5 (`rust-rule-resolution`)
  переведён в состояние `ready` (§10) только после интеграции этого коммита
  слияния — этой записью пакет 5 не начинается.

- `rust-rule-resolution` (пакет 5): пакетный коммит Ядра
  `d921944c385336436b9b423198ffafa081fbd91e` (`feat(app): добавить
  прикладное разрешение норм Rust`, ветка `feature/rust-rule-resolution`)
  принят отдельным коммитом слияния в `origin/dev` —
  `6f0fb6428a458d2aae56be7d7b7f9f6c27443574` (GitHub PR #39: база `dev`,
  источник `feature/rust-rule-resolution`, согласно решению владельца о
  приёмке пакета). Обычный merge commit с двумя родителями: первый родитель
  `7fe59830edebdb500a1b6fece175ec6d50314430` (предыдущий принятый коммит
  слияния — пакет 4, PR #38), второй родитель
  `d921944c385336436b9b423198ffafa081fbd91e` (пакетный коммит). Пакетный
  коммит достижим из коммита слияния (`git merge-base --is-ancestor
  d921944c385336436b9b423198ffafa081fbd91e
  6f0fb6428a458d2aae56be7d7b7f9f6c27443574` — подтверждено). Дерево
  пакетного коммита совпадает с деревом коммита слияния (`git diff
  --exit-code d921944c385336436b9b423198ffafa081fbd91e^{tree}
  6f0fb6428a458d2aae56be7d7b7f9f6c27443574^{tree}` — код завершения 0, без
  вывода): diff принятого пакета не переписан при слиянии.

  **Результат** — строгая граница `meridian-app::rule_resolution` поверх
  чистого `meridian-core`: перенос `scripts/rule-resolver.mjs` и применимых
  композитных проверок контрактов операционной модели, более сильные
  Rust-инварианты, нормализация `mandatory` и независимость от порядка
  ключей JSON, реальное сравнение Node.js/Rust на общем корпусе через
  паритетный харнесс, правило совместимости и граница локальных тестов
  закреплены в `AGENTS.md`. Состав изменённых файлов (пакетный коммит):
  `AGENTS.md`, `CHANGELOG.md`,
  `meridian-app/examples/rule_resolution_producer.rs`,
  `meridian-app/src/lib.rs`, `meridian-app/src/rule_resolution.rs`,
  `meridian-core/src/resolver/mod.rs`,
  `standards/workspace/kernel-boundary.md`,
  `test/conformance-harness.test.mjs`,
  `verification/conformance-harness/README.md` и фикстуры/producer харнесса.

  **Проверочные доказательства**, зафиксированные из пакетного коммита:
  `cargo fmt --check`; `cargo clippy -p meridian-app --all-targets
  --all-features -- -D warnings`; `cargo test -p meridian-app --all-targets
  --all-features` — 20 passed, 0 failed; `node
  test/conformance-harness.test.mjs` — 40 passed, 0 failed; `git diff
  --cached --check` — успешно. Согласно сообщению пакетного коммита, эти
  проверки выполнены владельцем; тестовые наборы не запускались настоящей
  синхронизацией повторно — исторические SHA, родители, достижимость и
  равенство деревьев проверены read-only Git-командами независимо, как
  указано выше.

  **Процедурное отклонение оформления коммита слияния, зафиксированное
  отдельно и правдиво**: коммит слияния
  `6f0fb6428a458d2aae56be7d7b7f9f6c27443574` несёт заголовок `feat(app):
  добавить прикладное разрешение норм Rust` — заголовок пакетного коммита —
  вместо обязательной формы `merge(dev): принять feature/rust-rule-resolution
  — <результат по-русски>` (`version-control-flow.md` §12.1). В отличие от
  отклонения PR #35 (пакет 3), тело коммита слияния здесь **несёт** все
  четыре обязательных раздела («Что изменено», «Зачем», «Проверки»,
  «Связано» — `version-control-flow.md` §12.2); отклонение затрагивает
  только заголовок, не структуру тела. Опубликованная история не
  переписывается. Техническая целостность пакета — дерево, достижимость,
  набор проверок — этим отклонением не затронута и независимо подтверждена
  выше. Правильный текст коммита слияния (`merge(dev): ...`) обязан быть
  подготовлен до следующего слияния в `dev` в рамках этой программы.

  **Условие перехода дальше** — корректирующий рубеж
  `instance-repository-retirement-baseline` (§5.2) переведён в состояние
  `active` (§10) только после интеграции этого коммита слияния — этой
  записью корректирующий рубеж не завершается, и пакет 6
  (`sqlite-storage-adapter`) этой записью не начинается.

### 5.2. Корректирующий рубеж: вывод Instance-репозитория из эксплуатации (`instance-repository-retirement-baseline`)

**Причина рубежа.** Отдельный репозиторий Instance до этой записи оставался
активным источником планов, назначений ролей и Git-процесса разработки
самого Meridian, хотя владелец уже 2026-09-19 решил обратное
(`meridian-owner-intent-contract.md` §25): код и встроенная методология
Meridian, а также контур самоуправления его разработкой, принадлежат Kernel;
рабочие данные в целевой модели хранятся в SQLite, а не в отдельном
Git-репозитории; прежний Instance используется только для чтения — как
замороженный источник миграции — до пакета 8.

**Результат.** Пять канонических документов программы переноса на Rust и
видения владельца перенесены в отслеживаемую область Kernel
(`governance/meridian-owner-intent-contract.md`,
`governance/plans/meridian-rust-migration-program-plan.md` — этот документ,
`governance/decisions/meridian-rust-sqlite-architecture.md`,
`governance/specifications/meridian-rust-target-architecture.md`,
`governance/rfcs/meridian-cli-rfc.md`), очищенные от продуктовых данных
конкретного Instance при переносе (см. примечание в начале документа).
`AGENTS.md` больше не требует разрешения `MERIDIAN_INSTANCE` для начала
работы над Meridian и называет новые документы каноническими.
`scripts/preflight.mjs` по умолчанию проверяет только самодостаточность
Kernel и принимает явный переходный флаг `--require-instance` для случаев,
которым по-прежнему нужен замороженный источник. `scripts/kernel-validate.mjs`
больше не красит гейт исключительно из-за отсутствия `MERIDIAN_INSTANCE`:
продуктово-зависимая часть kernel-purity в этом режиме честно сообщает
`UNVERIFIED` (warn), а не `OK`; строгая проверка при явно переданном
`MERIDIAN_INSTANCE` не ослаблена.

**Зависимости** — пакет 5 (`rust-rule-resolution`, §5.1, `accepted`).
Внешнее решение, принятое как данность и не пересматриваемое этим рубежом:
`meridian-owner-intent-contract.md` §25.

**Критерии приёмки:**

1. `AGENTS.md` называет пять документов `governance/` каноническими и не
   называет разрешение `MERIDIAN_INSTANCE` обязательным условием начала
   работы над Meridian;
2. `node scripts/preflight.mjs` (без аргументов) проходит без
   `MERIDIAN_INSTANCE`, проверяя только Kernel;
3. `node scripts/preflight.mjs --require-instance` без `MERIDIAN_INSTANCE`
   отклоняется; с корректным замороженным источником сохраняет прежнюю
   строгую проверку; с некорректным — отклоняется; неизвестный аргумент
   отклоняется в обоих режимах;
4. отсутствие `MERIDIAN_INSTANCE` не красит `kernel-validate` само по себе;
   продуктово-зависимая часть kernel-purity в этом режиме сообщает `WARN`
   (`UNVERIFIED`), не `OK`; строгая проверка при явно переданном
   `MERIDIAN_INSTANCE` не ослаблена;
5. ни один из пяти перенесённых документов не содержит продуктовых имён,
   URL, локальных путей конкретного Instance или иных закрытых данных;
6. `standards/workspace/kernel-boundary.md`,
   `standards/workspace/workspace-scope-model.md`, `COMPATIBILITY.md`,
   `README.md`, `CHANGELOG.md`, `.gitignore` обновлены согласованно с этим
   рубежом.

**Запрещённое расширение, которого рубеж не выполняет:**

- не начинает пакет 6 (`sqlite-storage-adapter`);
- не удаляет и не архивирует старый репозиторий Instance — он остаётся
  замороженным источником до пакета 8 (`meridian-owner-intent-contract.md`
  §25.5);
- не изменяет ни одно рабочее дерево старого Instance;
- не выполняет Git-записи (ветвление, коммиты, слияния, публикацию) —
  границы исполнителя и интегратора Git разделены (`AGENTS.md` §6.2, §6.4);
- не запускает cargo- или Node.js-тестовые наборы как доказательство —
  переданный владельцем полный вывод проверок считается входом проверки
  результата (`AGENTS.md` §9).

**Проверочные доказательства и приёмка.** Пакетный коммит Ядра
`e47e9464313b2844c47ce7c53c12fb9d96952707` (`feat(governance): вывести
Instance из центра разработки Meridian`, ветка
`feature/instance-repository-retirement-baseline`) принят отдельным коммитом
слияния в локальную линию `dev` —
`ee5c4e97f2d3b9b9301d5461bcbddf0c9ef754ee`. Обычный merge commit с двумя
родителями: первый родитель `9c6722713b2c47ab1511d36332dd8845e46e6818`
(предыдущий принятый коммит слияния — закрепление ролей и локальной
интеграции Meridian), второй родитель
`e47e9464313b2844c47ce7c53c12fb9d96952707` (пакетный коммит). Пакетный
коммит достижим из коммита слияния (`git merge-base --is-ancestor
e47e9464313b2844c47ce7c53c12fb9d96952707
ee5c4e97f2d3b9b9301d5461bcbddf0c9ef754ee` — подтверждено). Дерево пакетного
коммита совпадает с деревом коммита слияния (`git diff --exit-code
e47e9464313b2844c47ce7c53c12fb9d96952707^{tree}
ee5c4e97f2d3b9b9301d5461bcbddf0c9ef754ee^{tree}` — код завершения 0, без
вывода): diff принятого пакета не переписан при слиянии. Интеграция
выполнена только локально: исходная ветвь, `dev` и коммит слияния в GitHub
не отправлялись, запрос на слияние не создавался
(`AGENTS.md` §6.7, запись 2026-09-19).

Переданные владельцем проверочные доказательства: `node
test/preflight.test.mjs` — 10 passed, 0 failed; `node
test/kernel-validate.test.mjs` — 293 passed, 0 failed, 0 skipped; `node
test/pre-push-git-isolation.test.mjs` — 8 passed, 0 failed; `git diff
--cached --check` — успешно; `sh -n hooks/pre-push` — успешно;
синтаксический разбор `.github/workflows/gate.yml` — успешно. Cargo-тесты и
остальные Node.js-регрессионные наборы этой документальной синхронизацией
не запускались и для неё не требуются (`AGENTS.md` §9).

**Результат рубежа, зафиксированный этой приёмкой:** каноническое
управление разработкой Meridian находится в Kernel (`governance/`); прежний
репозиторий Instance является только замороженным для чтения источником
миграции; обычная предварительная проверка (`node scripts/preflight.mjs`)
работает без Instance; строгий переходный режим доступен явно через
`--require-instance`; регрессионная проверка preflight
(`test/preflight.test.mjs`) включена в локальные (`hooks/pre-push`) и
удалённые (`.github/workflows/gate.yml`) ворота.

**Условие перехода к пакету 6 — выполнено.** Корректирующий рубеж
`instance-repository-retirement-baseline` независимо проверен и
интегрирован (выше); пакет 6 (`sqlite-storage-adapter`) переведён в
состояние `ready` (§4, §10). Этой записью пакет 6 не начинается: она не
создаёт `meridian-storage-sqlite`, не переносит его схему и не начинает
CLI, Metis или Concord.

## 6. Ворота Rust

Ворота вводятся постепенно, по мере появления соответствующей возможности —
не единым списком, обязательным уже с пакета 1. Требовать проверку сбоя
транзакции SQLite до того, как появится сама SQLite (пакет 6), или проверку
`import → export → import` до появления самого `import` (пакет 8), физически
невозможно; предыдущая ревизия этого документа объявляла такой единый список
обязательным для «каждого пакета с Rust-кодом» и это было её ошибкой,
исправляемой настоящей ревизией.

### 6.1. Общие ворота каждого пакета с Rust-кодом

Каждый из пакетов 1, 3–10 (все, кроме пакета 2, не вносящего собственного
предметного Rust-кода, — его отдельная область §6.2) обязан пройти:

- `cargo fmt --check`;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
- `cargo test --workspace`;
- `cargo doc --workspace --no-deps`;
- уже применимые на этом шаге перенесённые фикстуры и регрессионные тесты
  (`test/*.test.mjs`, адаптированные для новой реализации) — только за ту
  поверхность, которую этот пакет и ранее принятые пакеты этой программы уже
  реализуют, а не весь будущий объём заранее.

### 6.2. Паритетный харнесс — с пакета 2, по нарастающей поверхности

Харнесс (пакет `rust-conformance-harness`, пакет 2) применяется начиная с
этого пакета, но только к той поверхности, которая на данном шаге уже
реализована:

- на самом пакете 2 (до появления какой-либо перенесённой на Rust
  возможности) харнесс проверяется на контролируемом корпусе — заведомо
  совпадающих парах вердиктов и намеренно расходящихся парах, подобранных
  так, чтобы показать, что харнесс действительно обнаруживает расхождение, а
  не молчит на любом входе;
- начиная с пакета 4 (`rust-source-format-adapters` — первая перенесённая
  поверхность) и на каждом следующем пакете 5–8 сравнение реальных
  Node/Rust-вердиктов наращивается ровно на ту поверхность, которую вводит
  соответствующий пакет, а не на весь объём сразу и не заранее.

### 6.3. Обязательно после пакета 4 (`rust-source-format-adapters`)

- строгий слой поверх YAML/JSON Schema библиотек (allowlist полей и
  форматов, strict-lint YAML — решение D-A `meridian-cli-rfc.md`) и
  перенесённые для него фикстуры/adversarial-кейсы становятся обязательным
  содержимым ворот с этого пакета, а не с пакета 2.

### 6.4. Обязательно после пакета 6 (`sqlite-storage-adapter`)

- сбой внутри транзакции — прерывание операции посередине транзакции SQLite
  не оставляет частично применённое состояние;
- повреждённая БД — открытие повреждённого файла SQLite даёт явную ошибку,
  а не тихий неверный результат;
- несовместимая версия схемы — открытие базы с версией `schema_migrations`,
  не поддерживаемой текущим бинарником, даёт явную остановку, а не попытку
  работать с несовместимой схемой.

### 6.5. Обязательно после пакетов 7–8 (`meridian-cli-foundation`, `meridian-cli-migration`)

- сценарий `import → export → import` — повторный импорт экспортированного
  состояния не создаёт расхождения;
- сценарий `apply → rollback` — применение плана миграции и последующий
  откат возвращают проверяемо то же состояние;
- повторное применение (idempotent apply) — повторный `migration apply` того
  же плана не создаёт вторую копию и не дублирует эффект;
- работа без сети — ни одна из перечисленных в §4 технической спецификации
  команд не требует сетевого доступа для успешного выполнения.

### 6.5a. Обязательно для пакета 8 (`meridian-cli-migration`) — доказательства полноты миграции

Пакет 8 не считается принятым по одному лишь успешному выполнению команд —
он обязан представить проверяемые доказательства, что переходный адаптер
можно закрыть:

1. **полный импорт** — каждая запись замороженного источника миграции
   (`meridian-owner-intent-contract.md` §25) присутствует в SQLite после
   `import`; ни одна запись не потеряна и не задвоена;
2. **эквивалентность применимых норм** — результат `resolve` на
   импортированном состоянии совпадает с результатом того же контекста на
   замороженном Node-эталоне, читающем исходный `$MERIDIAN_INSTANCE`;
3. **идемпотентность** — повторный `import` того же источника и той же
   редакции не создаёт вторую копию (§6.5, «повторное применение»);
4. **обратимость** — сценарий `apply → rollback` на импортированных из
   замороженного источника данных, а не только на синтетических фикстурах,
   проверяемо возвращает исходное состояние;
5. **отсутствие эксплуатационных чтений через `MERIDIAN_INSTANCE`** — после
   пакета 8 ни валидатор, ни resolver, ни любая другая штатная операция
   выпускаемого бинарника не выполняет операционное чтение через
   `$MERIDIAN_INSTANCE`; оставшееся использование переменной, если оно есть,
   ограничено явно переходным путём (`preflight --require-instance`) и не
   является частью обычной работы CLI.

### 6.6. Полный набор — одновременно обязателен для пакетов 9 и 10

Пакет 9 (`rust-parity-qualification`) и пакет 10 (`meridian-rust-release`)
обязаны пройти полный набор §6.1–§6.5a одновременно, на полном объёме
перенесённой механики, — а не по частям, раскиданным между более ранними
пакетами.

## 7. Выпускной рубеж перед Concord

Пакет 10 (`meridian-rust-release`) и, следовательно, вся программа считаются
выпущенными только когда одновременно подтверждены:

1. один устанавливаемый бинарник собирается для целевых платформ;
2. Node.js не требуется для работы выпущенного бинарника;
3. PostgreSQL не требуется — SQLite остаётся единственным хранилищем первого
   цикла;
4. SQLite создаётся и открывается автоматически командой `init` без ручной
   подготовки файла или схемы;
5. применимый замороженный источник миграции успешно импортируется через
   `meridian import` (§6.5a.1);
6. вердикты `validate`/`resolve` на импортированном состоянии совпадают с
   замороженным Node-эталоном (§6.5a.2; `meridian-cli-rfc.md`, решение D-H);
7. миграция и возврат доказаны — сценарии `apply → rollback` и повторное
   применение (§6.5, §6.5a.3–4) пройдены на импортированных данных, а не
   только на синтетических фикстурах;
8. переходный `MERIDIAN_INSTANCE`-адаптер отключён — ни одна штатная
   операция выпущенного бинарника не выполняет эксплуатационное чтение через
   `$MERIDIAN_INSTANCE` (§6.5a.5); окончательное отключение переменной и
   архивирование старого репозитория Instance выполняется как часть этого
   пункта, а не откладывается на факультативное будущее решение
   (`meridian-owner-intent-contract.md` §25.5);
9. машинный вывод стабилен — коды завершения и структурированный
   (`--format json`) вывод не меняются между повторными запусками на одном и
   том же состоянии;
10. полный набор ворот (§6.1–§6.6) зелёный на выпускной ревизии;
11. выпускная ветка слита в стабильную линию отдельным merge-коммитом и
    возвращена в интеграционную линию по режиму продвижения, действующему для
    репозитория Kernel на момент выпуска (`version-control-flow.md` §2.2,
    §5.5; `release-versioning.md` §6).

Прохождение отдельных пакетов 1–9 не равно прохождению всего выпускного
рубежа — так же, как для программы `meridian-operating-upgrade`
(`meridian-operating-upgrade-plan.md` §12).

## 8. Запреты программы

- **Строгий поведенческий паритет обязателен для переносимых возможностей
  Node.** Пакеты 3–5 (`rust-domain-core`, `rust-source-format-adapters`,
  `rust-rule-resolution`) достигают паритета с замороженным Node-эталоном
  прежде всего для `validate`/`resolve` и уже существующих композитных
  проверок контрактов операционной модели (`scripts/lib/*.mjs`): они не
  меняют смысл ни одной уже действующей проверки и не добавляют проверок, не
  существовавших в эталоне на момент заморозки (`meridian-cli-rfc.md`,
  решение D-H, не-цели).
- **Это не запрещает пакеты 6–8.** Хранилище SQLite и поверхности
  `init`/`import`/`export`/`migration plan|apply|verify|rollback` (пакеты
  `sqlite-storage-adapter`, `meridian-cli-foundation`,
  `meridian-cli-migration`) не имеют прямого эквивалента в сегодняшней
  Node-реализации, но не являются новой функциональностью в запрещённом
  смысле: они реализуют контракты, уже принятые до этой программы
  (`instance-data-migration.md`, `controlled-rule-intake.md`,
  `existing-project-compatibility-mode.md`, `workspace-scope-model.md`) и
  детализированные `meridian-rust-sqlite-architecture.md`/
  `meridian-rust-target-architecture.md`, а не изобретают новое поведение.
  Они обязаны точно реализовать уже описанные контракты, не менять семантику
  существующих норм и не добавлять постороннюю функциональность сверх того,
  что эти контракты и техническая спецификация уже определяют.
- Не менять логическую модель областей (`workspace-scope-model.md`) ради
  удобства схемы SQLite — схема (§4 технической спецификации) обязана
  выражать существующие шесть областей, а не переопределять их.
- Не вводить PostgreSQL, сетевой API или многопользовательский доступ в
  рамках этой программы (`meridian-rust-sqlite-architecture.md` §6, §14).
- Не удалять Node-реализацию внутри этой программы: удаление — отдельное
  решение владельца **после** пакета 10 и доказанного паритета
  (`meridian-owner-intent-contract.md` §24, `meridian-cli-rfc.md`, «Миграция и
  rollout», шаг с удалением Node-реализации).
- Не считать документальную ревизию плана началом пакета: ни один пакет или
  корректирующий рубеж (включая `instance-repository-retirement-baseline`,
  §5.2) не начинается документальной синхронизацией — только отдельным
  исполнением.
- Не начинать пакет 6 (`sqlite-storage-adapter`) до независимой проверки и
  интеграции корректирующего рубежа `instance-repository-retirement-baseline`
  (§5.2).
- Не удалять и не архивировать старый репозиторий Instance, не изменять его
  рабочие деревья и не переносить в Kernel его продуктовые данные — рубеж
  §5.2 выводит его из роли активного центра, а не уничтожает как источник
  миграции (`meridian-owner-intent-contract.md` §25.4–25.5).
- Не возобновлять исполнение Concord как часть этой программы или её
  выпускного рубежа — возобновление наступает отдельно, после §7 (см. таблицу
  «После выпуска» в §4).

## 9. Что не входит

- реализация любого пакета 1–10 вне его объявленного результата;
- изменение продуктового кода какого-либо конкретного продукта, использующего
  Meridian;
- выбор PostgreSQL или сетевого API;
- назначение точного номера версии Kernel для выпуска Rust Meridian —
  вычисляется из фактической истории при подготовке `release/<semver>`
  (`release-versioning.md` §7), не этим планом;
- переиздание или пересмотр `meridian-rust-sqlite-architecture.md`,
  `meridian-rust-target-architecture.md` или `meridian-cli-rfc.md` — эта
  программа их исполняет, не пересматривает;
- возобновление исполнения Concord.

## 10. Состояние программы

```yaml
program_id: meridian-rust-migration
program_status: active
activation_gate: meridian-operating-upgrade-release
activation_gate_status: passed
last_completed_package: instance-repository-retirement-baseline
current_package: sqlite-storage-adapter
current_package_status: ready
next_package: meridian-cli-foundation
concord_status: paused_pending_meridian_rust_release
release_version: unassigned
release_gate: closed
owner_decision_date: 2026-09-19
```

Настоящая ревизия фиксирует приёмку и интеграцию корректирующего рубежа
`instance-repository-retirement-baseline` (§5.2) и переводит пакет 6
(`sqlite-storage-adapter`) в состояние `ready`. Она **не начинает** пакет 6:
не создаёт `meridian-storage-sqlite`, не добавляет его схему или код
хранения и не начинает интерфейс командной строки, Metis или Concord — ни
один из них этой синхронизацией не начинается и не возобновляется; Concord
остаётся на паузе. Начало пакета 6 остаётся отдельным, самостоятельным
исполнением после этой синхронизации.
