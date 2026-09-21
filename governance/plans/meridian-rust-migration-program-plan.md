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
  - $MERIDIAN_KERNEL/governance/plans/meridian-improvement-research-plan.md
  - $MERIDIAN_KERNEL/governance/research/hypothesis-registry.yaml
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
(`sqlite-storage-adapter`) принят и локально интегрирован (§4, §5.3).
Пакет `research-governance-foundation` — основание Исследовательского
отдела Meridian (`governance/research/`) — **принят и интегрирован**:
пакетный коммит `3e7881d84ed74eb70c08725e684754a78d079002`, коммит слияния
`b9d16ee07bf7b5bd268e8b46fd35cd1451e26662`.
Корректирующий пакет `knowledge-agent-foundation` — роли баз `tool`/
`workspace`, эволюция схемы и событийная граница до реализации командного
интерфейса (§4, §5.4) — **принят и интегрирован**: пакетный коммит
`e790a3af880cfab83894cb332e03d48b4ff6fc88`, коммит слияния
`97108dfa00e8b7474ec32332ecacf8494df9460c` (§5.4). Пакет 7
(`meridian-cli-foundation`) начат: реализация подготовлена в рабочем дереве
и ожидает независимой проверки и Git-интеграции (§5.4) — она не считается
принятой этой документальной записью. Ни эксперимент исследовательского
реестра этой синхронизацией не начинается.

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
| 6 | Хранилище SQLite (`sqlite-storage-adapter`) | `meridian-storage-sqlite`: схема §4 технической спецификации, транзакционная запись, включённые foreign keys, неизменяемые редакции и доказательства, идемпотентный импорт, резервная копия, канонический экспорт | 3, 5, `instance-repository-retirement-baseline` | accepted — принят и локально интегрирован (§5.3) |
| — | Корректирующий пакет: основание знаний и агентной среды (`knowledge-agent-foundation`) | Роли баз `tool`/`workspace`, привязка рабочей базы к редакции Kernel, последовательные миграции схемы, маршрутизация хранилищ, версионируемый конверт наблюдаемого события и отключаемый приёмник событий; спецификации `init`/`doctor`/`export` и импорта согласованы с этими границами | 6, `research-governance-foundation` | accepted — принят и интегрирован (§5.4) |
| 7 | Основа CLI (`meridian-cli-foundation`) | `meridian-cli`: `init`, `doctor`, `validate`, `resolve`, `export`, `--format human|json`, стабильные коды завершения, разделение stdout/stderr; `init` и `doctor` соблюдают принятые роли баз, а наблюдаемые события не меняют предметный результат команды. С 2026-09-21 (§5.5b) пакет 7 — агрегатор подпакетов: 7a `validate-mechanical-integrity` (сокращение `BLOCKED_CHECKS`, реальное CLI-сравнение, fail-clean `init`), затем 7b `validate-operating-contracts`, 7c `validate-evidence-and-intake`, 7d `validate-migration-qualification` — оставшиеся 15 заблокированных семейств `validate`, разбитые на три подпакета в этом порядке решением владельца (§5.5c) | 5–6, `knowledge-agent-foundation` | active — агрегатор; 7a active (реализация подготовлена, ожидает независимой проверки и интеграции, §5.5b), 7b–7d planned, в порядке 7b → 7c → 7d (§5.5c) |
| 8 | Миграционный CLI (`meridian-cli-migration`) | `import`, `migration plan|apply|verify|rollback` — реализация контракта `instance-data-migration.md` поверх `meridian-storage-sqlite`; импорт направляет продуктовые записи только в базу рабочей среды и не делает базу инструмента вторым продуктовым каноном; `plan` не изменяет состояние; `apply` поддерживает `--dry-run` и явное подтверждение. **Обязан доказать** (§6.5a): полный импорт всех записей замороженного источника миграции без потерь; эквивалентность применимых норм между Node-эталоном и импортированным состоянием; идемпотентность повторного импорта; обратимость (`apply → rollback`); отсутствие эксплуатационного чтения через `$MERIDIAN_INSTANCE` в штатной работе выпускаемого бинарника | 6–7, `knowledge-agent-foundation` | planned — заблокирован приёмкой и интеграцией всех подпакетов 7a–7d (§5.5b) |
| 9 | Квалификация равенства (`rust-parity-qualification`) | Полный паритетный прогон (полный набор ворот §6.1–§6.6) на всех классах тестовых деревьев; 0 расхождений вердиктов | 2, 4–8 | planned |
| 10 | Выпуск Rust Meridian (`meridian-rust-release`) | Один устанавливаемый бинарник, выпускная ветка, версия, журнал изменений, возврат в интеграционную линию — выпускной рубеж §7 ниже | 9 | planned |

### После выпуска (вне этой программы, но зависимые от неё)

| Название и идентификатор | Результат | Зависимость |
|---|---|---|
| Повторное подключение Concord (`concord-meridian-onboarding`) | Переклассификация Concord по выпущенной Rust-модели, без автоматического продолжения со старой точки G0 | 10 |
| Полевая оценка Concord (`concord-meridian-field-evaluation`) | Первое реальное полевое применение Rust Meridian | `concord-meridian-onboarding` |
| Подготовка экспериментов агентного контекста (`agent-context-experiment-readiness`) | Доказательство условий допуска ровно одной гипотезы; не начинает эксперимент автоматически | 10 и условия соответствующей записи `governance/research/hypothesis-registry.yaml` |

Эти три пункта не являются пакетами настоящей программы. Первые два наступают
после её выпускного рубежа и управляются условием возобновления Concord
(`meridian-owner-intent-contract.md` §24, §20;
`meridian-operating-upgrade-plan.md` §13, распространённое этим документом на
`meridian-rust-release`). Третий управляется отдельной исследовательской
программой и не открывает эксперимент одним фактом выпуска.

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

<a id="rust-workspace-foundation"></a>

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

### 5.3. Принятый пакет 6: хранилище SQLite (`sqlite-storage-adapter`)

**Результат.** Добавлены порты `RecordRepository` и
`EvidenceRepository`, строгая нейтральная к адаптеру модель хранения и
реализация `meridian-storage-sqlite`: восемь таблиц первой версии схемы,
транзакционные пакетные записи, включённые внешние ключи, неизменяемые
редакции и доказательства, идемпотентность, резервное копирование и
канонический экспорт.

**Зависимости.** Пакет 3 (`rust-domain-core`, §5.1, `accepted`) —
предметные типы и структуры доказательств, на которых строятся
`RecordRepository`/`EvidenceRepository`; пакет 5 (`rust-rule-resolution`,
§5.1, `accepted`); корректирующий рубеж
`instance-repository-retirement-baseline` (§5.2, `accepted`) — как
зафиксировано в строке пакета 6 таблицы §4. Внешние решения, принятые как
данность и не пересматриваемые этим пакетом: `meridian-rust-sqlite-architecture.md`
(ADR, `accepted` — выбор SQLite, портов и адаптеров) и §4 технической
спецификации `meridian-rust-target-architecture.md` (схема SQLite).

**Критерии приёмки.** Общие ворота §6.1 для пакета с Rust-кодом (`cargo fmt
--check`; `cargo clippy --workspace --all-targets --all-features -- -D
warnings`; `cargo test --workspace`; `cargo doc --workspace --no-deps`) и
специфичные ворота §6.4, обязательные с этого пакета: сбой внутри транзакции
SQLite не оставляет частично применённое состояние; открытие повреждённого
файла SQLite даёт явную ошибку, а не тихий неверный результат; открытие базы
с версией `schema_migrations`, не поддерживаемой текущим бинарником, даёт
явную остановку. Достигается на объёме результата, описанного выше (порты
`RecordRepository`/`EvidenceRepository`, восемь таблиц схемы, транзакционные
пакетные записи, внешние ключи, неизменяемые редакции и доказательства,
идемпотентность, резервная копия, канонический экспорт — §4 таблицы
пакетов).

**Запрещённое расширение.** Пакет не начинает интерфейс командной строки
(пакет 7); не закрепляет роли баз `tool`/`workspace`, эволюцию схемы поверх
версии 1 или событийную границу — они остаются отдельным следующим
корректирующим пакетом (`knowledge-agent-foundation`, §5.4); не вводит
PostgreSQL, сетевой API или многопользовательский доступ (§8;
`meridian-rust-sqlite-architecture.md` §6, §14); не меняет логическую модель
шести областей `workspace-scope-model.md` ради удобства схемы; не расширяет
семантику уже принятых контрактов (`instance-data-migration.md`,
`controlled-rule-intake.md`, `existing-project-compatibility-mode.md`) сверх
того, что определяет §4 технической спецификации; не начинает Metis или
Concord.

**Проверочные доказательства.** Пакетный коммит
`a7c32d70f2d1d3f6c9b250c564289dd9b650599a` принят отдельным локальным
коммитом слияния в `dev`
`426dd8bc69fbeb66b2858879b27201eadfffe0e6`. Деревья пакетного коммита и
коммита слияния совпадают; слияние имеет два родителя. Переданные владельцем
проверки: форматирование; сборка рабочего пространства со всеми целями и
возможностями; полный набор тестов; статический анализ с запретом
предупреждений; документация с запретом предупреждений; `git diff --check`
— успешно. В `Cargo.lock` относительно принятого основания добавлены только
три ребра зависимостей `meridian-storage-sqlite`, без изменения версий
пакетов.

Интеграция локальная: коммит слияния находится в `dev`, но не отправлен в
`origin/dev`. Это не выдаётся за удалённую публикацию.

**Условие перехода дальше.** Пакет 6 завершён. Следующим становится не пакет
7, а отдельно начинаемый корректирующий пакет §5.4.

<a id="knowledge-agent-foundation"></a>

### 5.4. Корректирующий пакет: основание знаний и агентной среды (`knowledge-agent-foundation`)

**Причина.** Гипотезы структурного получения контекста, детерминированной
навигации и независимой трассы остаются экспериментами, но их устойчивые
границы пересекают ближайшие `init`, `doctor`, `export` и `import`.
Откладывание ролей баз, эволюции схемы и событийной границы до времени после
пакета 8 создало бы переделку уже выпущенной поверхности командного интерфейса.

**Результат:**

1. роль SQLite-базы является явным метаданным `tool` или `workspace`, а не
   выводится из пути файла;
2. база инструмента привязана к одной редакции Kernel, рабочая база хранит
   совместимую ссылку на редакцию и не копирует продуктовые данные в базу
   инструмента;
3. версия 1 существующей схемы обновляется последовательными атомарными
   миграциями, а не только принимается или отвергается целиком;
4. маршрутизация двух ролей баз определена на уровне композиции, не меняя
   логические шесть областей записи;
5. принят версионируемый конверт наблюдаемого события и отключаемый приёмник
   событий; внутренняя цепочка рассуждений модели не является событием;
6. будущий разрешатель знаний объявлен отдельной границей от разрешателя норм,
   но не реализован;
7. целевая архитектура и RFC командного интерфейса обновлены до начала
   пакета 7.

**Критерии приёмки:** общие Rust-ворота §6.1; миграция реальной заполненной
базы версии 1 в новую версию без потери записей; повторное открытие уже
обновлённой базы идемпотентно и сверяет полные метаданные (роль и редакцию
Kernel), а не только роль; неизвестная будущая версия и неизвестная роль
по-прежнему останавливают работу; миграция отклоняет назначение роли,
несовместимой с уже существующими записями (проверка внутри транзакции шага
миграции, до записи `database_metadata`); `init`/`doctor`/`export` и будущий
импорт имеют однозначную роль базы в спецификациях; тест запрещает попадание
продуктовой записи в базу инструмента и built-in-записи в рабочую базу;
отключённый приёмник событий (`NoOpEventSink`) не меняет предметный результат
и сохранённое состояние реальной доменной операции — доказано тестом на
уровне `meridian-app`, без CLI. Наблюдаемость через stdout/stderr/код
завершения **не входит** в критерии этого пакета — у него нет команды CLI,
которая производила бы их; это отдельное обязательное ворота пакета 7
(§6.4a).

**Запрещённое расширение:** пакет не строит индекс или граф, не выбирает
поставщика, не реализует семантический либо структурный поиск, не добавляет
разрешатель знаний, не сохраняет внутреннюю цепочку рассуждений, не начинает
Metis или Concord и не открывает ни один эксперимент из исследовательского
реестра.

**Переданная реализация (исполнитель, 2026-09-20, после корректирующего
раунда по замечаниям независимой проверки).** Работа выполнена в рабочем
дереве репозитория без Git-записей (без `branch`/`switch`, `add`, `commit`,
`merge`, `rebase`, `reset`, `stash`, `tag`, `push`) — ожидает независимого
архитектурного ревью и последующей Git-интеграции; настоящая запись не
является приёмкой пакета.

1. `DatabaseRole` и `DatabaseMetadata` живут в `meridian-app::storage`, не в
   `meridian-core`: `meridian-core` не содержит понятий базы данных, SQLite,
   роли физической базы или порта хранения — он лишь предоставляет уже
   существующие строгие типы `ScopeType` и `Revision`, из которых эти два
   типа построены. `DatabaseRole` — закрытый `enum { Tool, Workspace }` с
   `accepts_scope_type`; `DatabaseMetadata` — пара `(DatabaseRole,
   Revision)`.
2. `meridian-storage-sqlite::schema::prepare` возвращает `DatabaseMetadata`,
   фактически прочитанные из базы после бутстрапа, миграции или сверки при
   повторном открытии — не копию аргумента вызывающей стороны;
   `SqliteStorage` хранит именно это возвращённое значение. Версия схемы
   повышена до 2 (таблица `database_metadata`, ровно одна строка); свежая
   база бутстрапится сразу на версии 2, существующая версии 1 мигрируется
   последовательной атомарной миграцией (каждый шаг — отдельная
   транзакция), а уже текущая версия сверяется по **обеим** составляющим
   метаданных — роли и редакции Kernel — целиком, не только по роли: до
   определения отдельного правила совместимости редакций требуется точное
   совпадение (`OpenError::DatabaseMetadataMismatch` при любом расхождении).
   Неизвестная будущая версия схемы и неизвестная строка роли дают
   типизированную остановку (`OpenError::UnsupportedSchemaVersion`,
   `OpenError::UnknownDatabaseRole`).
3. Миграция версии 1 → 2 проверяет, внутри транзакции своего шага и до
   записи `database_metadata`, что каждый уже существующий
   `records.scope_type` совместим с назначаемой ролью
   (`DatabaseRole::accepts_scope_type`); нераспознанный `scope_type` и
   несовместимый — типизированная остановка
   (`OpenError::UnrecognizedScopeTypeInExistingRecords`,
   `OpenError::MigrationRoleIncompatibleWithExistingRecords`), после которой
   версия остаётся 1, таблица `database_metadata` не создаётся и ни одна
   запись не меняется.
4. `SqliteStorage::open_path`/`open_in_memory` требуют `DatabaseMetadata`
   явным параметром; `apply_one` (запись записей) и `EvidenceRepository::put`
   отклоняют запись, чья область не разрешена ролью текущей базы
   (`PortError::ScopeNotAllowedForDatabaseRole`) — проверка на границе
   самого адаптера, а не только на уровне маршрутизации.
5. `meridian-app::storage::{RoledStorage, StorageRouter}` — нейтральная к
   SQLite композиционная граница: `StorageRouter::new` проверяет роль обеих
   переданных базовых хранилищ, `put`/`put_batch`/`get` маршрутизируют по
   типу области; смешанный по роли батч отклоняется
   (`PortError::MixedDatabaseRolesInBatch`), а не тихо разбивается на два.
6. `meridian-app::events` — `EventEnvelopeVersion`, закрытый `EventKind` (9
   видов), `ObservedEvent` ровно с тремя полями (версия конверта, вид,
   текстовый summary) — без произвольного детального payload: у
   `ObservedEvent::new` нет параметра, через который можно было бы
   присоединить структурированное произвольное содержимое, что доказано
   исчерпывающей деструктуризацией типа в тесте. Это не означает, что поле
   `summary` защищено от произвольного текста, включая текст рассуждения —
   оно остаётся обычной непустой строкой без ограничения формы; закрыт
   именно структурированный канал, а не любой текстовый ввод. Порт
   `EventSink` не гарантирует отсутствие эффекта для любой реализации — его
   сигнатура `fn record(&self, event: &ObservedEvent)` лишь не позволяет
   реализации вернуть ошибку в вызывающий поток управления; реализация
   по-прежнему может паниковать, блокироваться или менять разделяемое
   состояние. Гарантия отсутствия эффекта зафиксирована только для
   `NoOpEventSink` и доказана тестом на реальной доменной операции
   (`StorageRouter::put` с `FakeStorage`), сравнивающим результат и
   сохранённое состояние с вызовом `NoOpEventSink::record` вокруг операции и
   без него — не самоподтверждающимся тестом с постоянным возвращаемым
   значением. Наблюдаемость через stdout/stderr/код завершения не
   проверяется и не заявлена проверенной этим пакетом — у него нет команды
   CLI; это отдельное обязательное ворота пакета 7 (§6.4b).
7. `meridian-app::knowledge` — модуль только с документацией, резервирующий
   границу будущего Knowledge Resolver; не содержит резолвера, индекса,
   поиска или хранения.
8. `meridian-rust-target-architecture.md` (§3, §4.1, §4.2, §5, §7) и
   `meridian-cli-rfc.md` («Состав CLI») обновлены: роли баз и метаданные —
   как понятия `meridian-app::storage`, а не `meridian-core`; чтение роли и
   редакции из самой базы; полное (роль и редакция) сравнение при
   расхождении на повторном открытии; проверка совместимости существующих
   записей при миграции; маршрутизация и событийная граница — однозначны
   для будущих `init`/`doctor`/`export`/`import`.

**Учёт области (после корректирующего раунда).** 21 путь: 8 новых
(`meridian-app/src/{events/{mod,envelope,sink}.rs,knowledge.rs,storage/{database_metadata,database_role,role}.rs}`,
`meridian-storage-sqlite/tests/schema_migration.rs`) и 13 изменённых
(`CHANGELOG.md`; четыре документа `governance/`; шесть файлов
`meridian-app`/`meridian-storage-sqlite`; `meridian-storage-sqlite/tests/sqlite_storage_adapter.rs`).
`meridian-core/src/types/mod.rs` был изменён в предыдущем раунде и возвращён
к содержимому принятого основания при восстановлении границы (пункт 1) — его
итоговый diff относительно `HEAD` пуст, и он не входит в счёт как
изменённый файл.

**Проверки, выполненные исполнителем:** `cargo fmt --all -- --check`;
`cargo build --workspace --all-targets --all-features --locked`; `cargo test
--workspace --all-targets --all-features --locked` — **375 passed, 0
failed** по всем крейтам, включая 33 теста в файлах, целиком новых для этого
пакета: `meridian-app::storage::database_role` (5),
`meridian-app::storage::database_metadata` (1),
`meridian-app::storage::role` (8, включая
`a_disabled_event_sink_does_not_change_the_domain_result_or_stored_state`
против реальной доменной операции), `meridian-app::events::envelope` (4,
включая исчерпывающую деструктуризацию типа), `meridian-app::events::sink`
(1) и `meridian-storage-sqlite/tests/schema_migration.rs` (14, включая
симметричную проверку миграции продуктовой/built-in базы под несовместимой
ролью и тест несовпадения редакции Kernel); `cargo clippy --workspace
--all-targets --all-features --locked -- -D warnings`; `RUSTDOCFLAGS="-D
warnings" cargo doc --workspace --no-deps --locked`; `node
test/conformance-harness.test.mjs` — 40 passed, 0 failed (харнесс не
запускается против новой Rust-поверхности отдельно — пакет не входит в
паритетную поверхность §6.2, у него нет Node-эквивалента); `node
test/kernel-validate.test.mjs` — 293 passed, 0 failed, 0 skipped; `git diff
--check` — без ошибок.

**Условие перехода к пакету 7:** пакет отдельно исполнен, независимо принят и
интегрирован; документальная запись `ready`, равно как и переданная выше
запись `active`, не считается его началом или приёмкой.

**Приёмка (синхронизация владельца, 2026-09-20).** Пакет `knowledge-agent-foundation`
принят и интегрирован: пакетный коммит
`e790a3af880cfab83894cb332e03d48b4ff6fc88` (`feat(storage): заложить
основание знаний и агентной среды`) принят отдельным коммитом слияния в
`dev` — `97108dfa00e8b7474ec32332ecacf8494df9460c` (`merge(dev): принять
feature/knowledge-agent-foundation — заложить основание знаний и агентной
среды`). Обычный merge-коммит с двумя родителями: первый родитель
`b9d16ee07bf7b5bd268e8b46fd35cd1451e26662` (предыдущий принятый коммит
слияния — `research-governance-foundation`), второй родитель — сам пакетный
коммит. Пакетный коммит достижим из коммита слияния (`git merge-base
--is-ancestor e790a3af880cfab83894cb332e03d48b4ff6fc88
97108dfa00e8b7474ec32332ecacf8494df9460c` — подтверждено). Дерево пакетного
коммита совпадает с деревом коммита слияния (`git diff --exit-code
e790a3af880cfab83894cb332e03d48b4ff6fc88^{tree}
97108dfa00e8b7474ec32332ecacf8494df9460c^{tree}` — код завершения 0, без
вывода): diff принятого пакета не переписан при слиянии. Проверочные
доказательства — переданные исполнителем результаты выше (375 passed, 0
failed по всем крейтам; `node test/conformance-harness.test.mjs` 40 passed;
`node test/kernel-validate.test.mjs` 293 passed; `git diff --check` без
ошибок) — приняты как предъявленный владельцем результат прогона
(`AGENTS.md` §6.3, «Переданный владельцем полный вывод проверок считается
входом проверки результата»), не перезапущены заново этой синхронизацией.

**Начало пакета 7.** Настоящей записью пакет 7 (`meridian-cli-foundation`)
переводится в состояние `active`: реализация подготовлена в рабочем дереве
без Git-записей и ожидает независимой архитектурной проверки и
Git-интеграции; эта запись не является его приёмкой (`AGENTS.md` §6.3,
§8 запретов этого плана — «не считать документальную ревизию плана началом
пакета» не применяется здесь, поскольку начало пакета 7 сопровождается
самой реализацией, переданной в этом же раунде, а не отдельной
документальной ревизией).

### 5.5. Пакет 7: основа CLI (`meridian-cli-foundation`)

**Переданная реализация (исполнитель, 2026-09-20).** Работа выполнена в
рабочем дереве без Git-записей (без `branch`/`switch`, `add`, `commit`,
`merge`, `rebase`, `reset`, `stash`, `tag`, `push`) — ожидает независимого
архитектурного ревью и последующей Git-интеграции; настоящая запись не
является приёмкой пакета.

1. `meridian-cli` теперь несёт настоящий Rust CLI: `init`, `doctor`,
   `validate`, `resolve`, `export`, общий `--format human|json`,
   документированные стабильные коды завершения
   (`meridian-cli/src/exit_code.rs`: `0` успех, `1` отрицательный, но
   корректно сформированный результат, `2` ошибка использования
   командной строки, `3` ошибка входа/окружения) и строгое разделение
   потоков — результат только в stdout, диагностика только в stderr,
   независимо от `--format` (`meridian-cli/src/lib.rs`).
2. Композиция: `meridian-core` остаётся чистым синхронным ядром без
   изменений в этом пакете; `meridian-app` не получила нового файлового,
   Git-, БД- или env-ввода-вывода — `meridian-cli` читает файлы, `VERSION`,
   `instance-template/`, обе SQLite-базы через уже существующие порты
   `meridian_storage_sqlite::SqliteStorage` и вызывает
   `meridian_app::rule_resolution::resolve` и
   `meridian_app::source_format` как уже перенесённые механизмы, не
   дублируя их алгоритмы (`meridian-cli/src/commands/*.rs`,
   `meridian-cli/src/kernel.rs`).
3. `init` копирует `instance-template/` файл-в-файл из названного
   `--kernel` (никогда не встроенной копии), никогда не перезаписывает уже
   существующий файл назначения, создаёт обе базы с явными ролями
   (`DatabaseRole::Tool`/`Workspace`) и редакцией, прочитанной из
   канонического `VERSION`; при сбое открытия второй базы удаляет только
   базу, реально созданную этим запуском, не трогая уже существовавшую
   (`meridian-cli/src/commands/init.rs`).
4. `doctor` read-only: не создаёт отсутствующую базу
   (`meridian-cli/src/commands/mod.rs::open_existing_db` явно проверяет
   существование файла до открытия), показывает фактически прочитанные
   роль, редакцию и версию схемы каждой базы, отклоняет переставленные
   роли и несовпадающую редакцию через уже существующий типизированный
   `OpenError::DatabaseMetadataMismatch`, не угадывая роль по имени файла.
5. `validate` (`meridian-cli/src/commands/validate/mod.rs` and its
   submodules `document_identity.rs`, `duplicate_fm.rs`, `git_provenance.rs`,
   `instance_context.rs`, `kernel_purity.rs`, `link_check.rs`,
   `registry_schema.rs`, `rule_resolution_fixtures.rs` — not a single file
   any more) implements, **ported for real against real Kernel content**:
   `kernel-purity` (personal-path-leak half), `document-identity`,
   `duplicate-fm`, the Markdown link check, generic in-gate registry
   `$schema` validation, the `rule-resolution` PHASE B fixture check, and
   `git-provenance`. It never runs Node.js and never reaches the network.
   Every other check family `scripts/kernel-validate.mjs` runs — the 15
   operating-model composite contracts, plus `sha-provenance`,
   `instruction-topics`, `operating-foundation`, `stack-profiles` and
   `agent-instruction-identity` — is named individually, with the specific
   reason it is not yet ported, in the fixed `BLOCKED_CHECKS` list
   (`meridian-cli/src/commands/validate/mod.rs`) and echoed verbatim in
   every `validate` result's `blocked` array: never silently passed, never
   silently failed. **`meridian validate` never returns exit code `0` /
   `status: "ok"` while `BLOCKED_CHECKS` is non-empty** — its verdict is
   `ok` only when both `failures` is empty *and* `blocked` is empty
   (`meridian-cli/src/commands/validate/mod.rs::run`), so a Kernel whose
   only unchecked surface is the blocked families is reported exactly as
   what it is: not fully validated, not "OK". See "Second
   `CHANGES_REQUESTED` corrective round" below for why the full port of
   those 19 families is escalated as `BLOCKED_FOR_OWNER_DECISION` rather
   than attempted in this package's scope.
6. `resolve` — тонкий файловый/stdin-адаптер вокруг
   `meridian_app::rule_resolution::resolve`: обе схемы реестра читаются из
   названного `--kernel` на каждом вызове, запрос — из `--request` или
   stdin. Отклонённый запрос (нарушение схемы, неизвестное поле,
   отклонение ядра) даёт код `3`; корректно разрешённый результат,
   включая несущий `unresolved_applicability`/`unresolved_items`/
   `conflicts`, — код `0`, поскольку это законный детерминированный ответ
   резолвера, а не отказ CLI.
7. `export` открывает обе базы только для чтения с проверенными
   метаданными, объединяет их канонические JSON-экспорты в один массив,
   пересортированный по тому же порядку сегментов, что и
   `RecordKey::storage_key`, — так что порядок результата зависит только
   от содержимого, а не от того, какая база прочитана первой; ни одна
   запись не копируется между ролями.
8. `EventSink` подключён через композицию: каждая функция команды
   принимает `&dyn EventSink`; производственный бинарник всегда собирает
   `NoOpEventSink` (`meridian-cli/src/main.rs`); `meridian-cli/src/events.rs`
   добавляет `RecordingEventSink`, не выводимый ни в один поток
   производственно. Ворота §6.4b проверяются пятью тестами, не одним:
   `swapping_the_event_sink_does_not_change_stdout_stderr_or_exit_code`
   (`validate`, реально исполненная дважды — с `NoOpEventSink` и с
   `RecordingEventSink` — и побайтово сравненная по stdout/stderr/коду
   завершения) и по одному аналогичному тесту на `doctor`, `export`,
   `resolve` и `init` (`meridian-cli/src/lib.rs`, `event_sink_has_no_effect_on_*`)
   — сильнее, чем доказательство `knowledge-agent-foundation` на уровне
   `meridian-app`, поскольку проверяется на границе процесса, а не только
   в памяти.
9. `governance/rfcs/meridian-cli-rfc.md` и
   `governance/specifications/meridian-rust-target-architecture.md` не
   потребовали содержательного пересмотра для этого пакета: состав команд,
   расположение `instance-template`, роли баз и модель кодов завершения
   уже были зафиксированы предыдущими пакетами; этот пакет реализует их, не
   переопределяя.

**Проверки, выполненные исполнителем (первая передача, 2026-09-20).** См.
раздел ниже, «Корректирующая передача после второго `CHANGES_REQUESTED`»:
эта первая передача's exact numbers (405/40 tests) are superseded there and
not repeated here, precisely because item 4 of the second round required
removing stale test-count claims rather than leaving two contradictory
counts side by side.

## 5.5a. Корректирующая передача после второго `CHANGES_REQUESTED` (исполнитель, 2026-09-21)

Владелец передал пять пунктов правки после второго раунда `CHANGES_REQUESTED`
на пакет 7. Работа снова выполнена в рабочем дереве без Git-записей (без
`branch`/`switch`, `add`, `commit`, `merge`, `rebase`, `reset`, `stash`,
`tag`, `push`) — эта запись не является приёмкой пакета, и пакет не
переводится в `accepted`/`ready` этой записью.

**Пункт 1 — полный перенос `validate`: BLOCKED_FOR_OWNER_DECISION.**
`scripts/kernel-validate.mjs` (3805 строк) реализует 20 семейств проверок,
которые `meridian validate` пока не переносит; 15 из них — композитные
контракты операционной модели, каждый со своим алгоритмом в
`scripts/lib/*.mjs`:

| Модуль | Строк |
|---|---:|
| `instance-data-migration.mjs` | 1579 |
| `evidence-and-handoff.mjs` | 1605 |
| `field-evaluation.mjs` | 1467 |
| `context-manifest.mjs` (bounded-context-manifest) | 1124 |
| `workspace-compatibility-qualification.mjs` | 734 |
| `existing-project-compatibility-mode.mjs` | 662 |
| `upgrade-integration-qualification.mjs` | 612 |
| `role-and-human-control.mjs` | 590 |
| `controlled-rule-intake.mjs` | 525 |
| `execution-state.mjs` | 491 |
| `task-specification.mjs` | 369 |
| `instruction-source-registry.mjs` | 318 |
| `task-pattern-registry.mjs` | 239 |

— 10 315 строк bespoke JS composite-consistency algorithms across these 13
modules alone (two more named families, `functional-parity` and
`instance-canonical-export`, live inline in `kernel-validate.mjs` itself,
not in a separate `scripts/lib` file). Two of the fifteen already have a
pure Rust port of their core algorithm in `meridian-core`
(`evidence::aggregate` for `evidence-and-handoff-contract`,
`migration::checks` for `instance-data-migration`) — но, как уже
задокументировано в `meridian-cli/src/commands/validate/mod.rs`'s own
module doc, только сам алгоритм: файловый дискавери, разбор специфического
формата каждого контракта, кросс-ссылки и текст сообщений об ошибках,
побитово совпадающий с Node, для каждого из этих двух ещё предстоит
построить — сопоставимая по объёму работа с уже перенесёнными
`rule-resolver.mjs` и `yaml.mjs`/`json-schema.mjs` пакетами каждая, не
«последний шаг» уже почти готового переноса. Остальные 5 семейств
(`sha-provenance`, `instruction-topics`, `operating-foundation`,
`stack-profiles`, `agent-instruction-identity`) не требуют нового
доменного алгоритма, но каждое требует: обнаружение файлов по
специфическим путям Kernel, разбор строгого YAML-поднабора уже перенесённым
`meridian_app::source_format`, извлечение «региона» из парного `.md`-файла
(порт `scripts/lib/regions.mjs`, 150 строк, пока не перенесён), сравнение
множеств и побитово совпадающие сообщения — не единственный день работы, но
и не архитектурное решение, обсуждаемое отдельно ниже.

Полный перенос всех 20 семейств — с реальными фикстурами, отрицательными
доказательствами и conformance-сравнением с Node на каждое — это, по
собственной оценке этого пакета, программа работы, сравнимая по объёму с
пакетами 2–7 вместе взятыми, а не corrective-раунд одного пакета. Попытка
приблизить её в этом раунде означала бы либо сфабрикованный вердикт, либо
молчаливое сужение принятого контракта — оба прямо запрещены переданной
инструкцией. **Это остановлено с `BLOCKED_FOR_OWNER_DECISION`**: решение о
том, разбивать ли перенос на отдельные пакеты (по семейству или группами),
в каком порядке относительно пакета 8 (`meridian-cli-migration`), и с каким
приоритетом — принадлежит владельцу, не исполнителю.

Что этот раунд всё же сделал в границах пакета 7, без нового
архитектурного решения:

- **Исправлен реальный дефект fail-open.** `meridian validate` раньше
  возвращал код `0`/`status: "ok"`, если единственными реальными
  диагностиками были `warnings`, даже когда `BLOCKED_CHECKS` был непустым —
  непройденный обязательный гейт репортился как пройденный. Теперь
  `ok = failures.is_empty() && BLOCKED_CHECKS.is_empty()`
  (`meridian-cli/src/commands/validate/mod.rs::run`): код `0` невозможен,
  пока хоть одно семейство остаётся заблокированным, независимо от того,
  насколько чист остальной результат. Человекочитаемый формат получил
  третий, отдельный вердикт — `BLOCKED` (не `OK`, не `FAIL`) — когда
  реальных ошибок нет, но обязательный гейт не исполнялся целиком, вместо
  того чтобы называть это состояние либо ложным «OK», либо вводящим в
  заблуждение «FAIL» (в котором на самом деле 0 диагностик).
- `BLOCKED_CHECKS` (20 семейств, каждое — с точной причиной непереноса)
  остаётся неизменным списком: ни одно семейство из него не удалено этим
  раундом — удаление элемента списка без реальной реализации и
  отрицательного доказательства прямо запрещено переданной инструкцией.

**Пункт 2 — conformance стал реальным сравнением бинарников.**
`resolve_cli_producer.rs` (`meridian-cli/examples/`) и новый
`validate_cli_producer.rs` больше не вызывают
`meridian_cli::commands::resolve::run`/`collect_diagnostics` библиотечно —
каждый порождает настоящий скомпилированный `target/.../meridian` как
отдельный процесс (`std::process::Command`, запрос через stdin для
`resolve`, `--format json` для `validate`, реформатируемый только после
получения реального stdout этого процесса) и берёт настоящий код завершения
этого процесса. `kernel_validate_producer.rs` (звавший
`collect_diagnostics` напрямую) удалён — фикстура
`real-node-rust-cli-validate-clean-kernel` больше не проходит через него.
`meridian-cli/examples/resolve_cli_producer.rs` находит путь к реальному
бинарнику через `std::env::current_exe()` (сосед по каталогу профиля —
`CARGO_BIN_EXE_meridian` недоступен примерам Cargo, только интеграционным
тестам).

Реальное сравнение обнажило структурный вопрос: `meridian validate`
корректно (после исправления выше) никогда не возвращает `exit 0` на этом
Kernel, пока `BLOCKED_CHECKS` непусто, тогда как полный Node-эталон на том
же чистом дереве возвращает `exit 0`. Это не регрессия — это ожидаемое,
единственное расхождение, вызванное намеренной неполнотой Rust-стороны, а
не ошибкой одной из сторон. Ослаблять `compareVerdicts`
(`verification/conformance-harness/conformance-harness.mjs`) для этого
случая запрещено README самого харнесса («exit codes are compared for
equality, never mapped onto one another»), поэтому:
`real-node-rust-cli-validate-clean-kernel`'s `expected_status` изменён на
`divergent` (было ошибочно `conformant`), и `test/conformance-harness.test.mjs`
добавляет отдельную, точную проверку формы этого расхождения — `exit_code.match
=== false`, но `fail`/`warn` `missing`/`added` пусты с обеих сторон — так
что регрессия в самих диагностиках (а не только в коде завершения) всё
равно была бы поймана.

Добавлено пять новых сквозных (Node ↔ реальный `meridian`) негативных
мутационных сравнений — по одному на каждое реально перенесённое семейство
`validate`, не только позитивный чистый прогон:
`test/conformance-harness.test.mjs`'s `VALIDATE_MUTATION_FAMILIES` —
`kernel-purity` (личный путь), `document-identity` (не-kebab-case имя
файла), `duplicate-fm` (осиротевший второй front-matter блок), `link-check`
(битая Markdown-ссылка), `registry-schema` (документ, нарушающий
объявленную JSON Schema) — каждое проверяет, что Node и реальный `meridian`
сообщают один и тот же новый `FAIL` на одной и той же мутированной полной
копии этого репозитория (без `.git`/`target`). `rule-resolution` и
`git-provenance` не повторены здесь: первое уже целиком покрыто выделенными
корпусами (`real-node-rust-rule-resolution`, `real-node-rust-cli-resolve`),
второе не имеет содержательной файловой мутации в этом объёме работы
(его единственная реализованная проверка — консультативное «Instance root
not supplied»).

**Пункт 3 — `init`'s file copy исправлен на настоящий fail-clean.**
`meridian-cli/src/kernel.rs::copy_instance_template` больше не делает
`destination.exists() → fs::copy`: это гонка (проверка и запись —
раздельные системные вызовы) и не даёт защиты от symlink (a dangling or
live symlink at the destination would pass `.exists()`'s check in the
old-но false branch or get written through by `fs::copy` in the new-file
branch). Теперь каждый файл создаётся `fs::OpenOptions::create_new` —
атомарно, `O_CREAT|O_EXCL`: уже существующий файл, каталог ИЛИ symlink
(висящий или нет) по этому пути делает создание неуспешным с
`AlreadyExists`, никогда не открывая и не переписывая то, что там есть.
Каждая цель регистрируется в `TemplateCopyResult` как `copied: true` в
момент успешного `create_new`, до копирования единого байта содержимого —
раньше эта запись добавлялась только после успешного `fs::copy`, так что
файл, чья запись оборвалась на середине, никогда не попадал в список
известных этому вызову путей и не мог быть откачен ни этой функцией, ни
вызывающим её `init`. При ошибке записи частичный файл удаляется здесь же,
второй, независимой линией защиты.

`rollback_template_copy` и `init`'s собственный `rollback()` больше не
проглатывают ошибки очистки (`let _ = fs::remove_file/remove_dir`) —
каждая настоящая ошибка (не `NotFound`, не «каталог не пуст» — оба ожидаемы
и не репортируются) собирается в `Vec<String>` и печатается как отдельные
строки `warning:` на stderr до основной ошибки
(`meridian-cli/src/commands/init.rs::report_rollback_diagnostics`).

Добавлен настоящий, а не смоделированный, fault-injection тест:
`meridian-cli/tests/binary_runs.rs::init_rolls_back_a_partial_file_left_by_a_real_write_failure_mid_copy`
запускает реальный `meridian` под `sh -c "trap '' XFSZ; ulimit -f 2; exec
\"$0\" \"$@\""` — ограничивает размер файла процесса на уровне ОС и
игнорирует `SIGXFSZ` средствами самой оболочки (никогда не `unsafe`
Rust-код — `Cargo.toml`'s `[workspace.lints.rust] unsafe_code = "forbid"`
запрещает его во всём workspace), так что запись, превышающая лимит, даёт
настоящую `EFBIG` `io::Error`, а не убивает процесс сигналом. Тест строит
синтетический Kernel с одним маленьким файлом (успешно копируется целиком
раньше лимита) и одним большим (падает на середине записи), затем
побайтово сравнивает снимок рабочего каталога до и после — включая
предсуществующий, не относящийся к `init`, пользовательский файл.

**Пункт 4 — эта самая синхронизация.** Устаревшие утверждения (405 тестов,
40 conformance-случаев, единственный EventSink-тест, путь
`meridian-cli/src/commands/validate.rs` как одного файла) удалены из §5.5
выше и не повторяются здесь двумя разными числами. Пакет остаётся `active`,
не `accepted`/`ready`: `BLOCKED_CHECKS` по-прежнему непусто (пункт 1 выше).

**Пункт 5 — проверки, выполненные исполнителем (2026-09-21):**
`cargo fmt --all -- --check` — чисто; `cargo build --workspace --all-targets
--all-features --locked` — чисто; `cargo test --workspace --all-targets
--all-features --locked` — **419 passed, 0 failed** по всем крейтам
(`meridian-cli` — 15 unit-тестов в `src/lib.rs`/`src/commands/*.rs` + 30 в
`tests/binary_runs.rs`, включая новый fault-injection тест пункта 3;
остальные крейты не изменены этим раундом); `cargo clippy --workspace
--all-targets --all-features --locked -- -D warnings` — чисто;
`RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked` —
чисто; `node --test test/conformance-harness.test.mjs` — **49 passed, 0
failed** (было 44 до этого раунда: +5 мутационных семейств пункта 2, и
+1 точная проверка формы расхождения `real-node-rust-cli-validate-clean-kernel`,
−1 старая проверка «conformant», замененная на «divergent» с точной формой);
`node test/kernel-validate.test.mjs` — 293 passed, 0 failed, 0 skipped
(не изменился этим раундом — ни один Node-файл не тронут); полный `node
scripts/preflight.mjs` — самодостаточен, `MERIDIAN_INSTANCE` не затронут;
`git diff --check` — без ошибок; изолированная `git add -A && git diff
--cached --check` (в отдельном `git clone` рабочего дерева, никогда не в
самом репозитории — переданная инструкция запрещает `add` в реальном
репозитории) — без ошибок.

**Точный список путей, изменённых или добавленных этим раундом** (сверх
уже перечисленных в первой передаче §5.5 выше):

*Изменены:* `meridian-cli/src/commands/validate/mod.rs` (fail-closed `ok`,
трёхзначный человекочитаемый вердикт), `meridian-cli/src/kernel.rs`
(`create_new_file`, fail-clean `copy_instance_template`,
`rollback_template_copy` с диагностикой), `meridian-cli/src/commands/init.rs`
(`rollback()` с диагностикой, `report_rollback_diagnostics`),
`meridian-cli/src/lib.rs` (обновлённый `swapping_the_event_sink_...` тест),
`meridian-cli/tests/binary_runs.rs` (переименован и переписан
`validate_passes_on_this_kernel_in_human_and_json` →
`validate_reports_blocked_not_ok_on_this_kernel_with_zero_real_failures`;
добавлен `init_rolls_back_a_partial_file_left_by_a_real_write_failure_mid_copy`),
`meridian-cli/examples/resolve_cli_producer.rs` (спавнит реальный бинарник),
`verification/conformance-harness/fixtures/conformance-harness.fixtures.json`
(`real-node-rust-cli-resolve`/`real-node-rust-cli-validate-clean-kernel`
description и `expected_status`), `test/conformance-harness.test.mjs`
(`VALIDATE_MUTATION_FAMILIES`, точная проверка формы расхождения,
`--bins` добавлен к сборке производителей).

*Добавлены:* `meridian-cli/examples/validate_cli_producer.rs`.

*Удалены:* `meridian-cli/examples/kernel_validate_producer.rs`.

**Условие перехода к пакету 8:** пакет отдельно исполнен, независимо принят
и интегрирован; переданная выше запись `active` не считается его началом
или приёмкой. Пункт 1 (полный перенос `validate`) остаётся
`BLOCKED_FOR_OWNER_DECISION` и не является условием, которое исполнитель
может закрыть без отдельного решения владельца о разбиении и
приоритизации переноса 20 заблокированных семейств.

## 5.5b. Подпакет 7a `validate-mechanical-integrity` (исполнитель, 2026-09-21)

Владелец разрешил `BLOCKED_FOR_OWNER_DECISION` §5.5a пункта 1: пакет 7
(`meridian-cli-foundation`) становится **агрегатором** подпакетов, а не
одним неделимым результатом. Подпакет 7a (`validate-mechanical-integrity`)
переносит ровно пять из 20 семейств `BLOCKED_CHECKS`, названных в §5.5a как
«механические» — не требующие нового доменного алгоритма:
`sha-provenance`, `instruction-topics`, `operating-foundation`,
`stack-profiles`, `agent-instruction-identity`. Подпакеты 7b–7d (оставшиеся
15 заблокированных композитных контрактов операционной модели, их
разбиение и порядок) остаются `planned` — это по-прежнему решение
владельца, не закрытое этой записью. Пакет 8 (`meridian-cli-migration`)
остаётся `planned`, заблокированным приёмкой и интеграцией всех подпакетов
7a–7d. Эта запись фиксирует, что 7a **начат**, реализация подготовлена в
рабочем дереве и ожидает независимой проверки и Git-интеграции (§5.4) — она
не считается принятой этой документальной записью, и пакет 7 в целом
остаётся `active`, не `accepted`/`ready`.

**Реализация пяти семейств.** Каждое читает Kernel-специфичные пути через
`meridian-cli/src/kernel.rs`'s уже перенесённое перечисление файлов
(`list_git_tracked_files`/`walk_all_files`), разбирает данные строгими
уже перенесёнными адаптерами (`meridian_app::source_format::yaml`,
`::json_schema`), извлекает «регион» пула из парного `.md`-файла через
новый файлово-независимый адаптер `meridian_app::source_format::regions`
(порт `scripts/lib/regions.mjs`: `blankFencedBlocks`, `markedRegion`,
`instructionRegions` — 454 строки, 10 модульных тестов — **устарело: см.
§5.5d ниже для действующих чисел после третьего `CHANGES_REQUESTED`
раунда**), сравнивает
множества и воспроизводит точный текст диагностик Node-эталона
(`scripts/kernel-validate.mjs`, соответствующие секции для каждого
семейства) — с точностью до именованных намеренных расхождений §5.5d
(например, авария Node-эталона на `stack-profiles.profiles` неверного типа,
где само это утверждение о точном тексте неприменимо, потому что Node не
производит вовсе никакой структурированной диагностики для сравнения).
Ни один из пяти модулей или адаптер `regions` не выполняет
файловый, Git-, env- или process-ввод/вывод внутри `meridian-core`;
файловое чтение остаётся в `meridian-cli`, как и у всех ранее перенесённых
проверок:

**Приведённые ниже per-module числа строк/тестов — исторические (на момент
этой записи, до второго и третьего корректирующих раундов, изменивших
`instruction_topics.rs`/`stack_profiles.rs`/`operating_foundation.rs` пунктом
3 §5.5d и `regions.rs` пунктом 3 §5.5e); действующие числа — в §5.5e ниже.**

- `meridian-cli/src/commands/validate/sha_provenance.rs` (224 строки, 6
  модульных тестов) — сверка SHA-256 установленных skill-артефактов с их
  `PIN.yaml`;
- `meridian-cli/src/commands/validate/instruction_topics.rs` (245 строк, 6
  тестов) — согласие двух половин пула тем (`instruction-topics.yaml` и
  регион пула в `instruction-topics.md`);
- `meridian-cli/src/commands/validate/operating_foundation.rs` (427 строк,
  6 тестов) — согласие машинных идентичностей и человекочитаемых подписей
  терминов и принципов операционного основания (`operating-foundation.yaml`
  + регионы в связанном `.md`);
- `meridian-cli/src/commands/validate/stack_profiles.rs` (245 строк, 5
  тестов) — согласие пула стек-профилей (`stack-profiles/stack-profiles.yaml`
  + регион в `stack-profiles/stack-profiles.md`), включая отдельную проверку,
  что `universal` не значится профилем;
- `meridian-cli/src/commands/validate/agent_instruction_identity.rs` (349
  строк, 9 тестов) — четыре обязательных поля §7 (delivery/activation/topic/
  derived_from) на документах, объявляющих себя agent instruction нормой, с
  учётом пула `instruction-topics` для допустимых значений topic.

Все пять удалены из `BLOCKED_CHECKS`
(`meridian-cli/src/commands/validate/mod.rs`) и подключены в `collect()`
наравне с ранее перенесёнными проверками; `BLOCKED_CHECKS` теперь содержит
ровно **15** записей (было 20 в §5.5a) — список сверен построчно с §5.5a
пункта 1 и не расходится с ним ни одним именем. `ok = failures.is_empty()
&& BLOCKED_CHECKS.is_empty()` (§5.5a пункт 1) не изменено: код `0`
по-прежнему невозможен, пока хоть одно из оставшихся 15 семейств блокировано,
и эта запись не заявляет паритет с полным контрактом §7.1 — расхождение
`real-node-rust-cli-validate-clean-kernel` (exit code, `BLOCKED_CHECKS`)
остаётся `expected_status: "divergent"`, не `conformant`, до тех пор, пока
`BLOCKED_CHECKS` не опустеет.

**Доказательство переноса — реальное CLI-сравнение, не приближение.** Для
каждого из пяти семейств `test/conformance-harness.test.mjs`'s
`VALIDATE_MUTATION_FAMILIES_7A` строит отдельную полную копию этого
репозитория (без `.git`/`target`), вносит одну целевую негативную мутацию
(поддельный SHA-провенанс skill, лишняя запись в одной половине пула тем,
лишний термин в данных operating-foundation без соответствующей подписи,
лишний стек-профиль в данных без подписи, документ без обязательных полей
agent-instruction-identity) и сравнивает реальный Node-эталон
(`scripts/kernel-validate.mjs`) с реальным собранным `meridian validate`
(процесс, не библиотечный вызов — `meridian-cli/examples/validate_cli_producer.rs`
не изменился в этой части: уже спавнит реальный бинарник с §5.5a пункта 2).
Каждый случай ожидает и получает `conformant` — совпадающий новый `FAIL` на
обеих сторонах. Позитивные фикстуры для каждого семейства — само дерево
этого Kernel-репозитория, уже используемое чистым (немутированным) прогоном
`real-node-rust-cli-validate-clean-kernel`.

**Пункт 5 — `resolve` conformance: exit code 2 отличён от exit code 3.**
`meridian-cli/examples/resolve_cli_producer.rs` больше не схлопывает любой
ненулевой код завершения в одну строку `"rejected"`: наблюдение теперь несёт
точный код завершения процесса (`output.status.code()`) и текст stderr
(`rejected:exit={code}:{stderr}`), а не только факт отказа. Новый
модульный/чёрноящичный тест
`meridian-cli/tests/binary_runs.rs::resolve_distinguishes_a_usage_error_from_a_rejected_request_by_exit_code`
и соседний
`resolve_rejects_malformed_json_with_environment_exit_code_and_empty_stdout`
подтверждают, что отсутствующий обязательный флаг возвращает
`exit_code::USAGE` (2), а принятая, но не разбираемая или не удовлетворяющая
транспортному контракту заявка — `exit_code::INPUT_OR_ENVIRONMENT` (3): два
разных кода для двух разных причин отказа, не одна общая «rejected»
категория ни в CLI, ни в верификационном производителе, который его
наблюдает.

**Проверки, выполненные исполнителем (2026-09-21):** `cargo fmt --all --
--check` — чисто; `cargo build --workspace --all-targets --all-features
--locked` — чисто; `cargo test --workspace --all-targets --all-features
--locked` — **462 passed, 0 failed** (было 419 в §5.5a; +43 — 32 новых
модульных теста по пяти семействам (6+6+6+5+9), 10 в
`meridian-app/src/source_format/regions.rs`, 1 новый в
`meridian-cli/tests/binary_runs.rs` для пункта 5) — **это число устарело
после второго и третьего раундов `CHANGES_REQUESTED`; действующее число —
в §5.5d ниже**; `cargo clippy --workspace --all-targets --all-features
--locked -- -D warnings` — чисто; `RUSTDOCFLAGS="-D warnings" cargo doc
--workspace --no-deps --locked` — чисто; `node --test
test/conformance-harness.test.mjs` — **54 passed, 0 failed** (было 49 в
§5.5a; +5 — по одной сквозной мутационной проверке на каждое из пяти
семейств 7a, `VALIDATE_MUTATION_FAMILIES_7A`) — **это число тоже устарело;
действующее число — в §5.5d ниже**; `node
test/kernel-validate.test.mjs` — 293 passed, 0 failed, 0 skipped (не
изменился этим раундом — ни один Node-файл-эталон не тронут); полный `node
scripts/preflight.mjs` — самодостаточен; `git diff --check` — без ошибок;
изолированная `git add -A && git diff --cached --check` (во временной копии
рабочего дерева вне этого репозитория, никогда не в самом репозитории) —
без ошибок.

**Точный список путей, изменённых или добавленных этим раундом:**

*Добавлены:* `meridian-app/src/source_format/regions.rs`,
`meridian-cli/src/commands/validate/sha_provenance.rs`,
`meridian-cli/src/commands/validate/instruction_topics.rs`,
`meridian-cli/src/commands/validate/operating_foundation.rs`,
`meridian-cli/src/commands/validate/stack_profiles.rs`,
`meridian-cli/src/commands/validate/agent_instruction_identity.rs`.

*Изменены:* `meridian-app/src/source_format/mod.rs` (реэкспорт `regions`),
`meridian-cli/src/commands/validate/mod.rs` (пять новых модулей подключены
в `collect()`, `BLOCKED_CHECKS` сокращён с 20 до 15 записей, обновлена
документация модуля), `meridian-cli/examples/resolve_cli_producer.rs`
(пункт 5 — точный код завершения и stderr вместо общего `"rejected"`),
`meridian-cli/tests/binary_runs.rs` (новый тест на различение exit 2/3
`resolve`), `test/conformance-harness.test.mjs`
(`VALIDATE_MUTATION_FAMILIES_7A`), `governance/plans/meridian-rust-migration-program-plan.md`
(эта запись и обновление §4 таблицы пакетов 7/8).

**Условие перехода к 7b–7d/пакету 8:** подпакет 7a отдельно исполнен,
независимо принят и интегрирован; статус передачи ниже —
`READY_FOR_ARCHITECT_REVIEW` только для подпакета 7a, не для всего пакета
7. Разбиение оставшихся 15 семейств на 7b–7d и их порядок более не
`BLOCKED_FOR_OWNER_DECISION` — см. §5.5c ниже, где владелец принял это
решение. Подпакет 7a остаётся `active` до отдельной независимой приёмки;
7b–7d остаются `planned` до своей собственной реализации.

## 5.5c. Решение владельца: разбиение и порядок подпакетов 7b–7d (2026-09-21)

Владелец закрыл вопрос §5.5a пункта 1 и §5.5b, ранее остановленный
`BLOCKED_FOR_OWNER_DECISION`, — не «оставить как единый неделимый перенос»
и не «разбить по одному семейству на подпакет», а сгруппировать оставшиеся
15 заблокированных семейств `BLOCKED_CHECKS`
(`meridian-cli/src/commands/validate/mod.rs`) в три подпакета, в
зафиксированном порядке реализации и приёмки:

- **7b `validate-operating-contracts`** — контракты операционной модели,
  проверяемые без Instance-фикстур эволюционного/полевого рода:
  `functional-parity`, `task-pattern-registry`,
  `instruction-source-registry`, `task-specification-contract`,
  `execution-state-model`, `role-and-human-control`,
  `bounded-context-manifest`;
- **7c `validate-evidence-and-intake`** — доказательная база и приёмка
  правил: `evidence-and-handoff-contract`, `meridian-field-evaluation`,
  `controlled-rule-intake`, `existing-project-compatibility-mode`;
- **7d `validate-migration-qualification`** — квалификация миграции и
  совместимости, естественно примыкающая к пакету 8
  (`meridian-cli-migration`): `instance-data-migration`,
  `instance-canonical-export`, `workspace-compatibility-qualification`,
  `upgrade-integration-qualification`.

**Порядок:** 7b → 7c → 7d — реализуются и принимаются последовательно в
этом порядке, не параллельно и не в другом порядке, если владелец не примет
отдельное решение об изменении.

**Пакет 8 (`meridian-cli-migration`) остаётся заблокирован** приёмкой и
интеграцией **всех** подпакетов 7a–7d — не только 7a, и не частичным
подмножеством 7b–7d. Таблица пакетов §4 уже отражает это условие для
пакета 8; эта запись не ослабляет и не сужает его.

Эта запись — единственное действующее слово по разбиению и порядку 7b–7d:
любое более раннее утверждение в этом документе (§5.5a пункт 1, §5.5b) о
том, что это решение ещё не принято владельцем, устарело этой записью и не
описывает текущее состояние. Начало фактической реализации 7b, её
исполнитель и её собственные критерии приёмки остаются отдельными,
последующими решениями — эта запись фиксирует только разбиение и порядок,
не назначение исполнителя или дату начала.

## 5.5d. Третья корректирующая передача подпакета 7a (исполнитель, 2026-09-21)

Владелец передал пять пунктов правки после третьего раунда `CHANGES_REQUESTED`
на подпакет 7a. Работа снова выполнена в рабочем дереве без Git-записей (без
`branch`/`switch`, `add`, `commit`, `merge`, `rebase`, `reset`, `stash`,
`tag`, `push`) — эта запись не является приёмкой подпакета, и он не
переводится в `accepted`/`ready` этой записью. 7a остаётся `active`,
статус передачи ниже — по-прежнему `READY_FOR_ARCHITECT_REVIEW` только для
7a; 7b–7d остаются `planned`, их разбиение и порядок (7b → 7c → 7d, §5.5c)
и блокировка пакета 8 приёмкой и интеграцией всех 7a–7d — без изменений.

**Пункт 1 — общий positive/adversarial corpus для `regions.mjs`/`regions.rs`,
реальные вызовы с обеих сторон.** Новая тройка файлов:

- `verification/conformance-harness/fixtures/regions-corpus.json` — 35
  случаев (было 34 до четвёртого корректирующего раунда, §5.5e): 11 на
  `blank_fenced_blocks`/`blankFencedBlocks` (простой fence, unclosed fence,
  короткий вложенный fence НЕ закрывает более длинный внешний, `~~~`-fence,
  Unicode/CJK внутри fence до последующего региона, CRLF, закрывающий fence
  короче/длиннее открывающего, пустой вход, вход без fence, несколько
  последовательных пробелов до и после fence сохраняются побайтово вне
  забеленных строк), 10 на
  `marked_region`/`markedRegion` (ровно одна пара, отсутствующий маркер,
  дублированный `begin`, `end` перед `begin`, маркер внутри цитируемого
  fence, unclosed fence где-то в документе, CRLF, кириллица до и внутри
  региона, атрибуты обрезаны и возвращены, маркер другого имени не
  совпадает), 14 на `instruction_regions`/`instructionRegions`
  (side-by-side регионы с заголовками снаружи, вложенность отклонена,
  дублированный id региона отклонён, `end` без открытого региона, открытый
  и никогда не закрытый регион, русский Front Matter блокируется по свою
  закрывающую строку, Front Matter с многобайтовым CJK-содержимым сохраняет
  байтовые смещения, непокрытые нестрочные-заголовком строки посчитаны,
  CRLF целиком, unclosed fence где-то в документе, закрывающий маркер с
  другим id, недопустимое значение `generated`, регион без id, документ
  вовсе без Front Matter и регионов).
- `verification/conformance-harness/regions-node-producer.mjs` — вызывает
  РЕАЛЬНЫЕ экспортированные `blankFencedBlocks`/`markedRegion`/
  `instructionRegions` из `scripts/lib/regions.mjs` напрямую (не копию
  логики).
- `meridian-app/examples/regions_producer.rs` — вызывает РЕАЛЬНЫЕ
  скомпилированные `blank_fenced_blocks`/`marked_region`/
  `instruction_regions` из `meridian_app::source_format::regions` напрямую.

Новая фикстура `real-node-rust-regions-adapters`
(`verification/conformance-harness/fixtures/conformance-harness.fixtures.json`,
`expected_status: "conformant"`) подключена в тот же контролируемый корпус,
что и `real-node-rust-source-format-adapters`/`real-node-rust-rule-resolution`
— подхватывается существующим в `test/conformance-harness.test.mjs` циклом
по `corpusResults` автоматически, без нового кода теста. Сравниваются
ошибки, `text`/`attrs`, `sourceText` и весь исходный текст, побитово — это
утверждение верно начиная с четвёртого корректирующего раунда (§5.5e), где
нормализация ширины забеленного пробега ограничена именно теми строками,
которые `blank_fenced_blocks`/`blankFencedBlocks` фактически забелили; до
этого раунда оба производителя схлопывали пробег из более чем одного пробела
на ЛЮБОЙ строке, включая нетронутый текст вне fence, и побитовое сравнение
вне забеленных строк не было доказано.

Одна названная, намеренная граница: `blank_fenced_blocks` в Rust заменяет
символ на столько же пробелов, сколько у него байт UTF-8 (обязательное
условие — иначе байтовые смещения для последующего среза `raw` съезжают),
тогда как Node-эталон заменяет по одному пробелу на каждую единицу UTF-16, —
поэтому ШИРИНА забеленного пробельного пробега для многобайтового символа
внутри fence легитимно расходится между языками, и ни один потребитель ни в
Kernel, ни в этом порту эту ширину никогда не читает (только факт
забеливания строки и нетронутые смещения вокруг него). Оба производителя
схлопывают пробег пробелов в один фиксированный плейсхолдер (`·BLANKED·`)
перед сравнением, но, с четвёртого корректирующего раунда (§5.5e), только на
строке, которая действительно отличается от соответствующей строки `raw` —
то есть строке, которую `blank_fenced_blocks`/`blankFencedBlocks`
действительно забелили; строка, которую забеливание не тронуло, сравнивается
как есть, включая любой собственный пробег пробелов. Это подтверждает всё,
что действительно входит в контракт, не заявляя побайтовое совпадение
ширины забеленного пробега, которого ни одна сторона не обещает, и при этом
больше не прячет за тем же плейсхолдером совпадение или расхождение вне
забеленных строк.

**Пункт 2 — названная граница для `stack-profiles.profiles` неверного
типа, не заявленная `conformant`.** Новый блок в
`test/conformance-harness.test.mjs` (вне общего цикла
`VALIDATE_MUTATION_FAMILIES_7A_ADVERSARIAL`, поскольку Node здесь не
производит вовсе никакой структурированной диагностики, которую можно было
бы сравнить) строит полную копию репозитория с
`stack-profiles/stack-profiles.yaml`, где `profiles: true`, и напрямую
запускает оба настоящих бинарника:

- `scripts/kernel-validate.mjs` — завершается ненулевым кодом с
  **необработанным** `TypeError: entries.map is not a function`
  (`comparePool`, вызов для `stack-profiles`) на stderr; stdout **пуст**,
  потому что `ok()`/`fail()`/`warn()` печатают накопленное только при
  штатном завершении, а авария происходит до него — каждая диагностика уже
  пройденных проверок (`kernel-purity`, `document-identity`, … до
  `stack-profiles`) теряется безвозвратно;
- реальный `meridian validate --format json` — код завершения `1`
  (`exit_code::DOMAIN_NEGATIVE`), пустой stderr, один валидный JSON-документ
  на stdout со `status: "fail"`, `result.ok: false` и authored `FAIL`
  `stack-profiles: "profiles" must be a list, found a boolean`
  (`meridian-cli/src/commands/validate/stack_profiles.rs`, второй раунд
  `CHANGES_REQUESTED`) — выполнение продолжается до полного результата.

Обе стороны корректно отказываются от нулевого/успешного кода (fail-open
отсутствует с обеих сторон), но по двум принципиально разным механизмам —
непрозрачная авария, теряющая всю информацию, против легального полного
негативного результата. Это зафиксировано двумя прямыми проверками против
обоих настоящих бинарников, не через общий `conformant`/`divergent` харнесс
(там нечего сравнивать на стороне Node) и не как `conformant`. Node-эталон
не исправлялся — только наблюдался, как и предписано. С четвёртого
корректирующего раунда (§5.5e) проверка Node-стороны закреплена полностью,
не только ненулевым кодом: `nodeRun.status === 1` (точный код необработанного
исключения Node, а не любой ненулевой), `nodeRun.signal === null`
(завершение самим процессом, не убито сигналом) и `nodeRun.error ===
undefined` (`spawnSync` сам не сообщил об ошибке запуска) — вместе с уже
проверявшимся пустым stdout и `TypeError` на stderr.

**Пункт 3 — RAII для новых файловых unit-тестов.** `temp()` в
`instruction_topics.rs`, `stack_profiles.rs` и `operating_foundation.rs`
теперь возвращает RAII-guard `TestDir` (`Drop` удаляет ровно свой каталог,
включая раскрутку паники), а не голый `PathBuf`; каждый вызывающий тест
больше не содержит собственный `let _ = fs::remove_dir_all(&dir);` в конце.
Имя каталога — PID **и** наносекундная метка времени, не только PID:
голый PID — общий, предсказуемый каталог, на который может столкнуться
повторный запуск или параллельный поток `cargo test`.

**Пункт 4 — эта самая синхронизация.** Устаревшие числа §5.5b (`454 строки,
10 тестов` для `regions.rs`; `462 passed`; `54 passed`) отмечены
устаревшими на месте, со ссылкой сюда, а не удалены молча — история
раунда остаётся читаемой. Действующие числа — в пункте 5 ниже.

**Пункт 5 — проверки, выполненные исполнителем (2026-09-21):** `cargo fmt
--all -- --check` — чисто; `cargo build --workspace --all-targets
--all-features --locked` — чисто; `cargo test --workspace --all-targets
--all-features --locked` — **481 passed, 0 failed** (без изменения по
сравнению со вторым корректирующим раундом — этот раунд не добавил ни
одного нового `#[test]`: пункты 1–2 живут в Node-стороне харнесса и в новом
верификационном примере `regions_producer.rs`, пункт 3 — чистый рефакторинг
существующих тестов без изменения их числа); `cargo clippy --workspace
--all-targets --all-features --locked -- -D warnings` — чисто;
`RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked` —
чисто; `node --test test/conformance-harness.test.mjs` — **62 passed, 0
failed** (было 59 после второго раунда: +1 `real-node-rust-regions-adapters`,
+2 обе прямые проверки намеренной границы `stack-profiles` неверного
типа); `node test/kernel-validate.test.mjs` — 293 passed, 0 failed, 0
skipped (не изменился этим раундом — ни один Node-файл-эталон не тронут,
только НАБЛЮДЁН новым тестом в пункте 2); полный `node scripts/preflight.mjs`
— самодостаточен; `git diff --check` — без ошибок; изолированная `git add -A
&& git diff --cached --check` (в отдельной полной копии рабочего дерева
вне этого репозитория, никогда не в самом репозитории) — без ошибок.

**Точный список путей, изменённых или добавленных этим (третьим)
раундом:**

*Добавлены:* `verification/conformance-harness/fixtures/regions-corpus.json`,
`verification/conformance-harness/regions-node-producer.mjs`,
`meridian-app/examples/regions_producer.rs`.

*Изменены:* `verification/conformance-harness/fixtures/conformance-harness.fixtures.json`
(новая фикстура `real-node-rust-regions-adapters`),
`test/conformance-harness.test.mjs` (намеренная граница `stack-profiles`
неверного типа, вне общего мутационного цикла),
`meridian-cli/src/commands/validate/instruction_topics.rs`,
`meridian-cli/src/commands/validate/stack_profiles.rs`,
`meridian-cli/src/commands/validate/operating_foundation.rs` (RAII
`TestDir` вместо голого `PathBuf` и ручной очистки),
`governance/plans/meridian-rust-migration-program-plan.md` (эта запись,
пометка устаревших чисел §5.5b).

## 5.5e. Четвёртая корректирующая передача подпакета 7a (исполнитель, 2026-09-21)

Владелец передал три пункта правки после четвёртого раунда `CHANGES_REQUESTED`
на подпакет 7a, узко ограниченного самим 7a: 7b–7d и пакет 8 не начаты и не
затронуты этой записью. Работа снова выполнена в рабочем дереве без
Git-записей (без `branch`/`switch`, `add`, `commit`, `merge`, `rebase`,
`reset`, `stash`, `tag`, `push`) — эта запись не является приёмкой подпакета,
и он не переводится в `accepted`/`ready` этой записью. 7a остаётся `active`,
статус передачи ниже — по-прежнему `READY_FOR_ARCHITECT_REVIEW` только для
7a; 7b–7d остаются `planned`, их разбиение и порядок (7b → 7c → 7d, §5.5c) и
блокировка пакета 8 приёмкой и интеграцией всех 7a–7d — без изменений.

**Пункт 1 — нормализация ширины забеленного пробега ограничена
фактически забеленными строками.** До этого раунда
`normalizeBlankedRuns`/`normalize_blanked_runs`
(`verification/conformance-harness/regions-node-producer.mjs`,
`meridian-app/examples/regions_producer.rs`) схлопывали `/ +/g` — КАЖДЫЙ
пробег из более чем одного пробела — на любой строке результата
`blank_fenced_blocks`/`blankFencedBlocks`, включая текст, которого забеливание
никогда не касалось. Это скрывало бы за одним и тем же плейсхолдером
(`·BLANKED·`) реальное расхождение в нетронутом тексте между двумя языками,
не только в ширине забеленного fence-содержимого, которую граница и должна
нормализовать. Обе функции теперь принимают также `raw` и построчно сравнивают
результат с исходным текстом: строка нормализуется, только если она
отличается от соответствующей строки `raw` (то есть была фактически забелена);
нетронутая строка сравнивается как есть, пробел за пробелом. Новый
corpus-case `multiple-spaces-around-fence-are-preserved-verbatim-outside-it`
(`verification/conformance-harness/fixtures/regions-corpus.json`) — несколько
последовательных пробелов и до, и после fence — делает точное побайтовое
сохранение этого текста частью сравнения, а не только предполагаемым
свойством.

**Пункт 2 — Node-завершение в проверке `stack-profiles` неверного типа
закреплено полностью.** Прежняя проверка утверждала только
`nodeRun.status !== 0` — любой ненулевой код прошёл бы, включая код от
сигнала или от ошибки самого `spawnSync`, ни один из которых не был бы тем
самым «необработанным исключением», который пункт и должен пригвоздить.
Проверка (`test/conformance-harness.test.mjs`) теперь также утверждает
`nodeRun.status === 1` (точный код Node для необработанного исключения),
`nodeRun.signal === null` (процесс завершился сам, не был убит сигналом) и
`nodeRun.error === undefined` (`spawnSync` не сообщил о собственной ошибке
запуска) — вместе с уже существовавшими проверками пустого stdout и
`TypeError: entries.map is not a function` на stderr. Действительный запуск
подтверждает `status: 1, signal: null`, без `error`, на этом же входе.

**Пункт 3 — формулировки синхронизированы.** (a) §5.5b теперь
квалифицирует заявление о «точном тексте диагностик Node-эталона» ссылкой
на именованные намеренные расхождения §5.5d (авария `stack-profiles`
неверного типа, где у Node вовсе нет структурированной диагностики для
сравнения). (b) corpus-case, ранее названный
`shorter-nested-fence-closes-the-outer-one`
(`verification/conformance-harness/fixtures/regions-corpus.json`) и
одноимённый Rust unit-тест
`blank_fenced_blocks_a_shorter_nested_fence_closes_the_outer_one`
(`meridian-app/src/source_format/regions.rs`) заявляли обратное тому, что
`raw = "````\n```\n````\nafter\n"` действительно проверяет: трёхбэктиковая
строка внутри четырёхбэктикового fence КОРОЧЕ открывающего маркера и потому
НЕ закрывает его (`marker.chars().count() >= fence_len` в
`blank_fenced_blocks` требует длину не меньше); закрывает fence только третья
строка, той же длины, что и открывающая. Оба переименованы в
`shorter-nested-fence-does-not-close-the-outer-one` (corpus) и
`blank_fenced_blocks_a_shorter_nested_fence_does_not_close_the_outer_one`
(Rust-тест) — сама логика и утверждения теста не менялись, только имя,
ранее заявлявшее не то, что тест проверяет. (c) Та же ложная формулировка в
описании корпуса §5.5d («короткий вложенный fence закрывает внешний»)
исправлена на «короткий вложенный fence НЕ закрывает более длинный
внешний», а счёт case'ов `blank_fenced_blocks` обновлён с 10 до 11 (34 → 35
всего) после добавления corpus-case пункта 1. (d) Описание фикстуры
`real-node-rust-regions-adapters` (в §5.5d и в
`verification/conformance-harness/fixtures/conformance-harness.fixtures.json`)
заявляет побитовое сравнение всего прочего, кроме ширины забеленного пробега
внутри fence, — это утверждение верно только начиная с этого, четвёртого,
раунда (пункт 1 выше); текст §5.5d обновлён явной оговоркой об этом. (e)
Устаревшие per-module числа строк/тестов в §5.5b (`sha_provenance.rs` 224/6,
`instruction_topics.rs` 245/6, `operating_foundation.rs` 427/6,
`stack_profiles.rs` 245/5, `agent_instruction_identity.rs` 349/9) явно
помечены историческими на месте, со ссылкой сюда; действующие числа на конец
этого раунда:

| Модуль | Строк | Тестов |
|---|---|---|
| `sha_provenance.rs` | 301 | 7 |
| `instruction_topics.rs` | 407 | 9 |
| `operating_foundation.rs` | 596 | 9 |
| `stack_profiles.rs` | 402 | 8 |
| `agent_instruction_identity.rs` | 349 | 9 |
| `regions.rs` | 557 | 18 |

Рост этих чисел относительно §5.5b — накопленный эффект второго и третьего
корректирующих раундов (RAII `TestDir`, добавленные adversarial-тесты и
исправления по их замечаниям), а не этого, четвёртого, раунда, который сам
по себе не добавил и не удалил ни одного `#[test]` ни в одном из этих шести
файлов (переименование Rust-теста в пункте 3(b) не меняет их число).

**Проверки, выполненные исполнителем (2026-09-21):** `cargo fmt --all --
--check` — чисто; `cargo build --workspace --all-targets --all-features
--locked` — чисто; `cargo test --workspace --all-targets --all-features
--locked` — **481 passed, 0 failed** (без изменения по сравнению с третьим
корректирующим раундом — этот раунд не добавил и не убрал ни одного
`#[test]`, только переименовал один в `regions.rs`); `cargo clippy
--workspace --all-targets --all-features --locked -- -D warnings` — чисто;
`RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked` —
чисто; `node --test test/conformance-harness.test.mjs` — **62 passed, 0
failed** (без изменения по сравнению с третьим раундом — этот раунд не
добавил и не убрал ни одного `test()`/`check()`, только уточнил
существующие); `node test/kernel-validate.test.mjs` — 293 passed, 0 failed,
0 skipped (не изменился этим раундом — ни один Node-файл-эталон не тронут);
полный `node scripts/preflight.mjs` — самодостаточен; `git diff --check` —
без ошибок; изолированная `git add -A && git diff --cached --check` (в
отдельной полной копии рабочего дерева вне этого репозитория, никогда не в
самом репозитории) — сообщает две строки о завершающих пробелах в
`standards/templates/readme-template.md`, файле, не тронутом ни этим, ни
любым из предыдущих раундов 7a (`git diff --stat` для этого пути — пусто);
это преднамеренный синтаксис Markdown-переноса строки (завершающие два
пробела), пред-существующий в дереве до этого раунда, а не регрессия от него —
изолированная проверка находит его только потому, что копия репозитория
заново инициализируется как Git и `git add -A` индексирует файл целиком, а не
как диапазон изменённых строк, которые видит обычный `git diff --check` на
самом репозитории (там же — «без ошибок» на реальном дереве). HEAD
(`97108dfa00e8b7474ec32332ecacf8494df9460c`) и ветка (`dev`) не изменились
этим раундом — рабочее дерево содержит только несколько незакоммиченных
изменений, `git status` тот же, что и до начала раунда, минус правки этого
раунда.

**Точный список путей, изменённых или добавленных этим (четвёртым)
раундом:**

*Изменены:* `verification/conformance-harness/regions-node-producer.mjs`
(пункт 1), `meridian-app/examples/regions_producer.rs` (пункт 1),
`verification/conformance-harness/fixtures/regions-corpus.json` (пункт 1 —
новый corpus-case; пункт 3(b) — переименование существующего case'а),
`test/conformance-harness.test.mjs` (пункт 2),
`meridian-app/src/source_format/regions.rs` (пункт 3(b) — переименование
Rust unit-теста, без изменения его тела), `governance/plans/meridian-rust-migration-program-plan.md`
(эта запись, пункт 3(a)(c)(d)(e)).

*Не изменены этим раундом:* ничего из 7b–7d или пакета 8; ни один из пяти
семейств-модулей `validate` (кроме их отражения в таблице выше — сами файлы
`sha_provenance.rs`, `instruction_topics.rs`, `operating_foundation.rs`,
`stack_profiles.rs`, `agent_instruction_identity.rs` не тронуты этим
раундом, только `regions.rs`).

## 6. Ворота Rust

Ворота вводятся постепенно, по мере появления соответствующей возможности —
не единым списком, обязательным уже с пакета 1. Требовать проверку сбоя
транзакции SQLite до того, как появится сама SQLite (пакет 6), или проверку
`import → export → import` до появления самого `import` (пакет 8), физически
невозможно; предыдущая ревизия этого документа объявляла такой единый список
обязательным для «каждого пакета с Rust-кодом» и это было её ошибкой,
исправляемой настоящей ревизией.

### 6.1. Общие ворота каждого пакета с Rust-кодом

Каждый из пакетов 1, 3–10 и корректирующий пакет
`knowledge-agent-foundation` (все, кроме пакета 2, не вносящего собственного
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

### 6.4a. Обязательно после `knowledge-agent-foundation`

- база версии 1 обновляется последовательной атомарной миграцией без потери
  записей, а повторное открытие не повторяет применённый шаг и сверяет
  полные метаданные (роль **и** редакцию Kernel), а не только роль;
- роль базы `tool` или `workspace` и совместимая редакция Kernel читаются
  из метаданных, а не угадываются из пути;
- миграция отклоняет назначение роли, несовместимой с уже существующими
  записями базы (проверка внутри транзакции шага миграции, до записи
  `database_metadata`), и явно останавливается на нераспознанном
  `scope_type`;
- продуктовая запись отклоняется при попытке направить её в базу
  инструмента, built-in-запись — при попытке направить её в рабочую базу;
- отключённый приёмник наблюдаемых событий (`NoOpEventSink`) не меняет
  предметный результат и сохранённое состояние доменной операции — до
  появления CLI (пакет 7) это доказывается на уровне `meridian-app` против
  реальной доменной операции, а не самоподтверждающимся тестом; stdout,
  stderr и код завершения здесь не проверяются, потому что у этого пакета
  нет команды, которая бы их производила (см. §6.4b);
- неизвестная будущая версия схемы и неизвестная роль базы дают явную
  остановку.

### 6.4b. Дополнительно обязательно, начиная с пакета 7 (`meridian-cli-foundation`)

- отключённый приёмник наблюдаемых событий не меняет stdout, stderr или код
  завершения ни одной команды CLI. Это отдельное, более сильное требование
  по сравнению с §6.4a: §6.4a доказывает отсутствие влияния только на
  предметный результат и состояние на уровне `meridian-app`, где
  stdout/stderr/код завершения ещё не существуют; настоящий пункт становится
  проверяемым и обязательным только с появлением первой команды CLI и не
  считается выполненным более ранним пакетом.

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
обязаны пройти полный набор §6.1–§6.5a, включая §6.4a и §6.4b, одновременно, на
полном объёме перенесённой механики, — а не по частям, раскиданным между более
ранними пакетами.

## 7. Выпускной рубеж перед Concord

Пакет 10 (`meridian-rust-release`) и, следовательно, вся программа считаются
выпущенными только когда одновременно подтверждены:

1. один устанавливаемый бинарник собирается для целевых платформ;
2. Node.js не требуется для работы выпущенного бинарника;
3. PostgreSQL не требуется — SQLite остаётся единственным хранилищем первого
   цикла;
4. необходимые SQLite-базы создаются и открываются автоматически командой
   `init` с явными ролями и совместимой редакцией Kernel, без ручной
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
- **Корректирующий пакет `knowledge-agent-foundation` не является началом
  экспериментов.** Он реализует только принятые владельцем архитектурные
  основания §5.4. Алгоритмы поиска, графы, производные индексы, выбор
  поставщика и проверка преимуществ остаются за пределами этой программы и
  закрыты условиями исследовательского реестра.
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
- Не начинать `knowledge-agent-foundation` до независимой проверки и
  интеграции `research-governance-foundation`; не начинать пакет 7 до
  независимой приёмки и интеграции `knowledge-agent-foundation`.
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
  `meridian-rust-target-architecture.md` или `meridian-cli-rfc.md`, кроме
  явно ограниченной синхронизации границ в `knowledge-agent-foundation`
  (§5.4); остальные пакеты исполняют эти документы, не пересматривая их;
- возобновление исполнения Concord.

## 10. Состояние программы

```yaml
program_id: meridian-rust-migration
program_status: active
activation_gate: meridian-operating-upgrade-release
activation_gate_status: passed
last_completed_package: knowledge-agent-foundation
current_package: meridian-cli-foundation
current_package_status: active
next_package: meridian-cli-migration
concord_status: paused_pending_meridian_rust_release
release_version: unassigned
release_gate: closed
owner_decision_date: 2026-09-20
```

Настоящая ревизия фиксирует принятие и интеграцию корректирующего пакета
`knowledge-agent-foundation` (пакетный коммит
`e790a3af880cfab83894cb332e03d48b4ff6fc88`, коммит слияния
`97108dfa00e8b7474ec32332ecacf8494df9460c`, §5.4) и начало исполнения
пакета 7 (`meridian-cli-foundation`) — реализация подготовлена в рабочем
дереве (§5.4, «Переданная реализация»), но не принята и не
Git-интегрирована этой записью: `current_package_status: active` фиксирует,
что пакет начат, а не что он завершён. Ни Metis, ни Concord, ни один
эксперимент этой синхронизацией не начинаются; Concord остаётся на паузе.
