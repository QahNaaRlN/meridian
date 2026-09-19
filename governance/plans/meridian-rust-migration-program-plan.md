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
До пакета 7 введён корректирующий пакет `knowledge-agent-foundation`: он
сохраняет проверенный результат пакета 6 и обязан закрепить роли баз,
эволюцию схемы и событийную границу до реализации командного интерфейса
(§4, §5.4). Его исполнение **начато**: реализация подготовлена в рабочем
дереве и ожидает независимой проверки и Git-интеграции (§5.4) — она не
считается принятой этой документальной записью. Ни эксперимент
исследовательского реестра этой синхронизацией не начинается.

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
| — | Корректирующий пакет: основание знаний и агентной среды (`knowledge-agent-foundation`) | Роли баз `tool`/`workspace`, привязка рабочей базы к редакции Kernel, последовательные миграции схемы, маршрутизация хранилищ, версионируемый конверт наблюдаемого события и отключаемый приёмник событий; спецификации `init`/`doctor`/`export` и импорта согласованы с этими границами | 6, `research-governance-foundation` | active — реализация подготовлена, ожидает независимой проверки и интеграции (§5.4, §10) |
| 7 | Основа CLI (`meridian-cli-foundation`) | `meridian-cli`: `init`, `doctor`, `validate`, `resolve`, `export`, `--format human|json`, стабильные коды завершения, разделение stdout/stderr; `init` и `doctor` соблюдают принятые роли баз, а наблюдаемые события не меняют предметный результат команды | 5–6, `knowledge-agent-foundation` | planned |
| 8 | Миграционный CLI (`meridian-cli-migration`) | `import`, `migration plan|apply|verify|rollback` — реализация контракта `instance-data-migration.md` поверх `meridian-storage-sqlite`; импорт направляет продуктовые записи только в базу рабочей среды и не делает базу инструмента вторым продуктовым каноном; `plan` не изменяет состояние; `apply` поддерживает `--dry-run` и явное подтверждение. **Обязан доказать** (§6.5a): полный импорт всех записей замороженного источника миграции без потерь; эквивалентность применимых норм между Node-эталоном и импортированным состоянием; идемпотентность повторного импорта; обратимость (`apply → rollback`); отсутствие эксплуатационного чтения через `$MERIDIAN_INSTANCE` в штатной работе выпускаемого бинарника | 6–7, `knowledge-agent-foundation` | planned |
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
last_completed_package: sqlite-storage-adapter
current_package: knowledge-agent-foundation
current_package_status: active
next_package: meridian-cli-foundation
concord_status: paused_pending_meridian_rust_release
release_version: unassigned
release_gate: closed
owner_decision_date: 2026-09-20
```

Настоящая ревизия фиксирует принятие и интеграцию корректирующего пакета
`research-governance-foundation` (пакетный коммит
`3e7881d84ed74eb70c08725e684754a78d079002`, коммит слияния
`b9d16ee07bf7b5bd268e8b46fd35cd1451e26662`) и начало исполнения пакета
`knowledge-agent-foundation` — реализация подготовлена в рабочем дереве
(§5.4, «Переданная реализация»), но не принята и не Git-интегрирована этой
записью: `current_package_status: active` фиксирует, что пакет начат, а не
что он завершён. Ни Metis, ни Concord, ни один эксперимент этой
синхронизацией не начинаются; Concord остаётся на паузе.
