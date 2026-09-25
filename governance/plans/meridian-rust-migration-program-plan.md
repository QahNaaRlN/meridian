---
title: Миграция Meridian на Rust — программа и дорожная карта
document_type: plan
status: active
scope: workspace
owner: workspace-owner
created: 2026-09-14
updated: 2026-09-25
related_documents:
  - $MERIDIAN_KERNEL/governance/meridian-owner-intent-contract.md
  - $MERIDIAN_KERNEL/governance/plans/meridian-improvement-research-plan.md
  - $MERIDIAN_KERNEL/governance/research/hypothesis-registry.yaml
  - $MERIDIAN_KERNEL/governance/decisions/meridian-rust-sqlite-architecture.md
  - $MERIDIAN_KERNEL/governance/decisions/metis-federated-knowledge-architecture.md
  - $MERIDIAN_KERNEL/governance/specifications/meridian-rust-target-architecture.md
  - $MERIDIAN_KERNEL/governance/rfcs/meridian-cli-rfc.md
  - $MERIDIAN_KERNEL/governance/audits/meridian-rust-codebase-architecture-audit.md
  - $MERIDIAN_KERNEL/standards/workspace/rust-migration-quality.md
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

> **Наивысший приоритет миграции:** соблюдать выбранную архитектуру Rust;
> если Rust позволяет реализовать функциональность лучше или надёжнее, это
> преимущество обязательно используется. Сохраняются бизнес-ценность и
> публичные контракты, а не устройство, ограничения или дефекты Node.js.
> Полные правила: `standards/workspace/rust-migration-quality.md`.

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
`97108dfa00e8b7474ec32332ecacf8494df9460c` (§5.4). Обязательное
архитектурное исправление пакета 7 (`meridian-cli-foundation`) по §5.13
завершено: пакеты 1–7 `rust-architecture-conformance` и исправление 7a
приняты и локально интегрированы, все семейства 7a–7d закрыты. Следующим
остаётся пакет 8 (`meridian-cli-migration`); он специфицирован и открыт к
исполнению в §5.22, но ещё не реализован. Ни эксперимент исследовательского
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
- Состав команд CLI, харнесс соответствия, маршрут замещения Node-реализации —
  `meridian-cli-rfc.md` (`in-review`).
- Приоритет архитектуры Rust над механическим копированием и обязательное
  использование более надёжных возможностей Rust —
  `standards/workspace/rust-migration-quality.md`.

Эта программа их не переоткрывает: она определяет **порядок**, в котором эти
уже принятые решения превращаются в код, и **ворота**, которые обязаны пройти
на каждом шаге.

## 4. Пакеты

Пакеты выполняются в строгом порядке; пакет N не начинается, пока пакет N−1 не
принят и не интегрирован, за исключением явно отмеченных параллельных долей.

| № | Название и идентификатор | Результат | Зависимость | Состояние |
|---:|---|---|---|---|
| 1 | Основание рабочего пространства Rust (`rust-workspace-foundation`) | Cargo workspace в репозитории Kernel: четыре крейта, `#![forbid(unsafe_code)]`, выбор конкретных крейтов (сериализация, JSON Schema, SQLite-драйвер), CI-сборка workspace | §1 (активационный рубеж) | accepted — принят и интегрирован (§5.1) |
| 2 | Набор проверки соответствия (`rust-conformance-harness`) | Независимый от Rust-реализации механизм запуска, нормализации и сравнения выбранных наблюдаемых контрактов; он обнаруживает как совпадения, так и расхождения, но не определяет архитектуру и не требует копировать дефекты Node.js (§6.2) | 1 | accepted — принят и интегрирован (§5.1) |
| 3 | Предметное ядро (`rust-domain-core`) | `meridian-core`: типы (§5 технической спецификации), resolver, конфликты, планы миграции, доказательства, вердикты и диагностика — без файлов, Git, БД, сети, env, вывода | 1–2 | accepted — принят и интегрирован (§5.1) |
| 4 | Адаптеры исходных форматов (`rust-source-format-adapters`) | Строгий слой поверх YAML/JSON Schema библиотек (allowlist полей и форматов, strict-lint YAML), сохраняющий принятые входные и диагностические контракты без копирования внутреннего устройства Node.js | 3 | accepted — принят и интегрирован (§5.1) |
| 5 | Разрешение норм (`rust-rule-resolution`) | Порт `scripts/rule-resolver.mjs` и композитных проверок контрактов операционной модели (`scripts/lib/*.mjs`) в `meridian-app` поверх портов `meridian-core` | 3–4 | accepted — принят и интегрирован (§5.1) |
| — | Корректирующий рубеж: вывод Instance-репозитория из эксплуатации (`instance-repository-retirement-baseline`) | Устранение отдельного репозитория Instance как активного центра управления разработкой Meridian; перенос пяти канонических документов в `governance/` Kernel; самодостаточный по умолчанию `preflight`/`kernel-validate`; не начинает `sqlite-storage-adapter` | 5 | accepted — принят и интегрирован (§5.2) |
| 6 | Хранилище SQLite (`sqlite-storage-adapter`) | `meridian-storage-sqlite`: схема §4 технической спецификации, транзакционная запись, включённые foreign keys, неизменяемые редакции и доказательства, идемпотентный импорт, резервная копия, канонический экспорт | 3, 5, `instance-repository-retirement-baseline` | accepted — принят и локально интегрирован (§5.3) |
| — | Корректирующий пакет: основание знаний и агентной среды (`knowledge-agent-foundation`) | Роли баз `tool`/`workspace`, привязка рабочей базы к редакции Kernel, последовательные миграции схемы, маршрутизация хранилищ, версионируемый конверт наблюдаемого события и отключаемый приёмник событий; спецификации `init`/`doctor`/`export` и импорта согласованы с этими границами | 6, `research-governance-foundation` | accepted — принят и интегрирован (§5.4) |
| 7 | Основа CLI (`meridian-cli-foundation`) | `meridian-cli`: `init`, `doctor`, `validate`, `resolve`, `export`, `--format human|json`, стабильные коды завершения, разделение stdout/stderr; подпакеты 7a–7d остаются исторической декомпозицией объёма | 5–6, `knowledge-agent-foundation`, `rust-architecture-conformance` | accepted — архитектурное исправление 7a–7d принято и локально интегрировано (§5.13, §5.21.9) |
| — | Корректирующий пакет архитектуры (`rust-architecture-conformance`) | Устранить нарушения аудита: типизированные границы DTO → domain, обязательные порты, typed diagnostics, перенос предметной логики из CLI, декомпозиция Value-центричных модулей; доказать сохранение бизнес-ценности и обоснованные Rust-native улучшения | 1–7d, аудит §5.13 | accepted — пакеты 1–7 и исправление 7a приняты и локально интегрированы (§5.21.9) |
| — | Архитектурное исправление исторического 7a (`meridian-cli-foundation-architecture-remediation`) | Перенести пять семейств 7a из CLI в typed core/app pipeline и ввести реально используемый app-owned `WorkspaceReader`; CLI оставить адаптером и presentation-слоем | `rust-architecture-conformance-1` и `-2`, §5.13 | accepted — принято и локально интегрировано (§5.16.8) |
| — | Основание типизированных task-контрактов (`rust-architecture-conformance-3`) | Перевести `task-pattern-registry` и `task-specification-contract` на общий typed core/app pipeline; переиспользовать канонические `WorkKind`/`ChangeClass`, ввести реально используемый `GitInspector`, передавать принятую каталогизацию паттернов в спецификацию без повторного разбора | `meridian-cli-foundation-architecture-remediation`, §5.13 | accepted — принято и локально интегрировано (§5.17.8) |
| — | Типизированное доказательство функционального паритета (`rust-architecture-conformance-4`) | Перевести `functional-parity` с `Value`-центричной CLI/app реализации на `WorkspaceReader -> private DTO -> typed core evidence/checks -> Diagnostic -> CLI presentation`, сохранив evidence-контракт и сделав I/O-сбои fail-closed | `rust-architecture-conformance-3`, §5.13 | accepted — принято и локально интегрировано (§5.18.8) |
| — | Типизированные контракты исполнения и управления запуском (`rust-architecture-conformance-5`) | Перевести связные `execution-state-model`, `role-and-human-control` и `bounded-context-manifest` на общие typed core contracts; app оставить transport/schema/resolution orchestration, CLI — composition/presentation; сохранить временный тонкий фасад только для двух ещё не перенесённых семейств 7c | `rust-architecture-conformance-4`, §5.13 | accepted — принято и локально интегрировано (§5.19.8) |
| — | Типизированные доказательства и полевая оценка (`rust-architecture-conformance-6`) | Перевести оставшиеся семейства 7c `evidence-and-handoff-contract` и `meridian-field-evaluation` на общий typed core/app pipeline, удалить временные resolver/portability фасады и оставить CLI слоем composition/presentation | `rust-architecture-conformance-5`, §5.13 | accepted — принято и локально интегрировано (§5.20.9) |
| — | Типизированная миграционная и upgrade-квалификация (`rust-architecture-conformance-7`) | Перевести весь связный 7d: `instance-data-migration`, `instance-canonical-export`, `workspace-compatibility-qualification`, `upgrade-integration-qualification`; расширить существующий migration owner и композиционно переиспользовать принятые операции 7b/7c | `rust-architecture-conformance-6`, §5.13 | accepted — принято и локально интегрировано (§5.21.9) |
| 8 | Миграционный CLI (`meridian-cli-migration`) | `import`, `migration plan|apply|verify|rollback` — реализация контракта `instance-data-migration.md` поверх `meridian-storage-sqlite`; импорт направляет продуктовые записи только в базу рабочей среды и не делает базу инструмента вторым продуктовым каноном; `plan` не изменяет состояние; `apply` поддерживает `--dry-run` и явное подтверждение. **Обязан доказать** (§6.5a): полный импорт без потерь бизнес-данных; сохранение применимых бизнес-норм либо явно принятое Rust-native улучшение; идемпотентность; обратимость; отсутствие эксплуатационного чтения через `$MERIDIAN_INSTANCE` | 6–7, `knowledge-agent-foundation`, `rust-architecture-conformance` | accepted — принят и локально интегрирован (§5.22.11) |
| 9 | Квалификация бизнес-контракта (`rust-business-contract-qualification`) | Полный прогон ворот §6.1–§6.6: сохранённые контракты совпадают, каждое намеренное Rust-native улучшение явно классифицировано, обосновано и протестировано; необъяснённых расхождений нет | 2, 4–8, `rust-architecture-conformance` | accepted — post-merge failure исправлен, tracked-рубеж пройден, `QUALIFIED`, принят и локально интегрирован (§5.23.11) |
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
  функциональность вне бизнес-цели пакета; Rust-native улучшения надёжности
  не являются запрещённым расширением, если сохраняют бизнес-ценность и
  проходят архитектурные ворота (§6, §8);
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

## 5.7. Подпакет 7b `validate-operating-contracts` (исполнитель, 2026-09-21)

Владелец подтвердил приёмку и локальную интеграцию 7a: пакетный коммит
`63c7c65551f6575f02b1a2f3ccd8a6edec390921`, коммит слияния
`758fdfff55a44e79e16a895c1752d78878a1f43c` — дерево пакетного коммита равно
дереву коммита слияния, пакетный коммит достижим из `dev`, `dev` (HEAD на
начало этого раунда) указывает на этот коммит слияния. 7a переходит в
`accepted`/`integrated`; 7b становится `active`. 7c и 7d остаются `planned`,
пакет 8 остаётся заблокирован приёмкой и интеграцией всех подпакетов 7a–7d
(§5.5c). Работа этого раунда выполнена в рабочем дереве без Git-записей (без
`branch`/`switch`, `add`, `commit`, `merge`, `rebase`, `reset`, `stash`,
`tag`, `push`) — эта запись не является приёмкой 7b, и он не переводится в
`accepted`/`ready` этой записью.

**Перенесённые семь семейств.** Каждое — специализированная JSON Schema плюс
собственный bespoke composite-consistency алгоритм из `scripts/lib/*.mjs`
(или, для `functional-parity`, инлайн-функция `kernel-validate.mjs`),
перенесённый в `meridian_app::operating_model::*` — чистую функцию над уже
разобранными значениями, без файлового, Git-, env- или process-ввода/вывода.
Файловое чтение (схема, данные, fixtures) остаётся в собственном модуле
`meridian-cli/src/commands/validate/*.rs` каждого семейства; ровно две
внешние границы, которые composite-алгоритм не может обойти без файлового
или резолверного ввода/вывода, выражены callback'ом, а не прямым `fs`/
резолвером внутри `meridian-app`: `task-pattern-registry`'s
`check_kernel_link` (существование, каноникализация и членство в отслеживаемом
множестве целевого файла канонической ссылки — с 2026-09-21, §5.8 пункт 3)
и `bounded-context-manifest`'s внешняя граница разрешения закреплённых
ссылок. Это трёхуровневое разделение `meridian-core`/`meridian-app`/
`meridian-cli`, которое пакет явно требовал, применено единообразно ко всем
семи семействам — в отличие от плоского размещения всей логики в
`meridian-cli`, принятого 7a:

| Семейство | `meridian-app` модуль | `meridian-cli` модуль | Реальные fixtures (valid/invalid) |
|---|---|---|---|
| `functional-parity` | `operating_model::functional_parity` | `commands::validate::functional_parity` | 8 / 29 |
| `task-pattern-registry` | `operating_model::task_pattern_registry` — с 2026-09-21 (§5.8 пункт 3): исправлено после корректирующего раунда, устранив собственное противоречие этой таблицы, ранее заявлявшей composite только в `meridian-cli` | `commands::validate::task_pattern_registry` (только файловая цель канонической ссылки — `check_kernel_link` передаётся как callback, см. §5.8 пункт 3) | 1 / 34 |
| `instruction-source-registry` | `operating_model::instruction_source_registry` | `commands::validate::instruction_source_registry` | 7 / 36 |
| `task-specification-contract` | `operating_model::task_specification` | `commands::validate::task_specification` | 11 / 48 |
| `execution-state-model` | `operating_model::execution_state` | `commands::validate::execution_state` | 11 / 47 |
| `role-and-human-control` | `operating_model::role_and_human_control` | `commands::validate::role_and_human_control` | реестр ролей 1 / 12, human-control 10 / 49 |
| `bounded-context-manifest` | `operating_model::bounded_context_manifest` | `commands::validate::bounded_context_manifest` | 10 / 65 |

`execution-state-model`, `role-and-human-control` и
`bounded-context-manifest` переиспользуют `resolve_schema_ref`/
`non_portable_reason` из `operating_model::task_specification` и
`LIFECYCLE_STAGES`/`WORK_STATUSES`/`TERMINAL_STATUSES` из
`operating_model::execution_state` НЕИЗМЕНЁННЫМИ — тот же приём, что и у
Node-эталона (`execution-state.mjs`/`role-and-human-control.mjs`/
`context-manifest.mjs` реэкспортируют, а не копируют, эти функции и
константы из `task-specification.mjs`/`execution-state.mjs`), а не
расходящаяся вторая копия адресной математики или закрытых пулов.

Все семь удалены из `BLOCKED_CHECKS`
(`meridian-cli/src/commands/validate/mod.rs`) и подключены в `collect()`.
**Точный список восьми оставшихся записей** (все — 7c/7d, ни одна не
принадлежит 7b): `evidence-and-handoff-contract`, `meridian-field-evaluation`,
`controlled-rule-intake`, `existing-project-compatibility-mode`,
`instance-data-migration`, `instance-canonical-export`,
`workspace-compatibility-qualification`, `upgrade-integration-qualification`
— сверено построчно с §5.5c и не расходится с ним ни одним именем.
`ok = failures.is_empty() && BLOCKED_CHECKS.is_empty()` не ослаблено: код `0`
по-прежнему невозможен, пока хоть одно из оставшихся восьми семейств
блокировано.

**Доказательство переноса — реальное CLI-сравнение для каждого семейства
(исправлено корректирующими раундами §5.8 пункт 1 и §5.9 пункт 1 — эта
запись сама не редактировалась, чтобы сохранить читаемой историю раунда;
действующее описание проверки — только в §5.9).** Первая редакция этой
записи мутировала, для пяти из семи семейств, только форму собственного
fixtures-бандла (усечение `invalid` до `[]`) — это доказывало только, что
связующий код в `meridian-cli` сообщает о пустом массиве одинаково, а не
что сам bespoke composite-алгоритм (`evaluate*`/`check*` в
`meridian_app::operating_model::*`) вообще запускался и совпадает. §5.8
пункт 1 заменил все пять на schema-valid мутацию одного значения внутри
уже существующего `valid`-fixture, которую JSON Schema не отвергает и
которую отклоняет только сам composite-алгоритм, и проверял её через
пересечение множеств новых FAIL — устарело: §5.9 пункт 1 заменил эту
проверку на точное равенство multiset-дельты (с учётом кратности повторов)
между Node и Rust относительно baseline, посчитанного на той же копии
непосредственно перед мутацией. Сами семь мутаций (значения, которые
меняются) — не устарели, только способ их проверки; за точными мутациями и
найденной ими ошибкой упорядочивания см. §5.8, за действующим способом
проверки — §5.9. Два семейства с настоящими данными Kernel
(`task-pattern-registry`, `role-and-human-control`) по-прежнему мутируют
`standards/workspace/{task-pattern-registry,role-registry}.yaml` напрямую —
они уже с первой редакции этой записи упражняли собственный
composite-алгоритм, не только связующий код.

**Именованная граница, найденная и исправленная этим раундом —
детерминированный порядок множественных одновременных диагностик.**
Первая написанная версия `task_pattern_registry.rs` использовала `HashMap`
там, где Node-эталон использует `Map` (`pairCount`), и `functional_parity.rs`
использовал `HashMap`/`HashSet` там, где Node-эталон использует `Map`/`Set`
(`catalog`/`declared`, `baseCondIds`) — оба итерируются в порядке первой
вставки, тогда как `HashMap`/`HashSet` в Rust итерируются в произвольном,
зависящем от хешера порядке. Это было обнаружено НЕ модульными тестами
против реальных fixtures (они проверяют только `problems.is_empty()`/
`!problems.is_empty()`, не точный порядок или текст), а именно требуемой
кросс-языковой мутацией: `task-pattern-registry`'s собственная мутация
(дублирование всего списка `task_patterns`) вызвала одновременно несколько
диагностик о повторяющейся классификационной паре, и Rust называл другую
пару первой, чем Node — `divergent`, не `conformant`. Исправлено: обе точки
теперь ведут отдельный `Vec` порядка первой вставки рядом с `HashMap`/
`HashSet` для членства, и все места, где Node-эталон обходит `Map`/`Set` для
построения диагностик, обходят этот `Vec`. Сама мутация также сужена: полное
дублирование `task_patterns` сталкивало ВСЕ семь реальных id, включая те, на
которые `task-specification-contract` и — что важнее —
ещё не перенесённый `upgrade-integration-qualification` (7d, `BLOCKED_CHECKS`)
ссылаются по имени в собственных fixtures, из-за чего Node сообщал о четырёх
диагностиках `upgrade-integration-qualification`, которые Rust не может
сообщить никогда, пока 7d не перенесён, — расхождение, не имеющее отношения
к `task-pattern-registry` как таковому. Новая мутация добавляет один
новый шаблон под id `mutation-probe-pattern`, на который никакой другой
fixture не ссылается, — единственное семейство, которое она может задеть,
это само `task-pattern-registry`.

**Именованные, принятые границы, оставленные как есть (не новые для этого
раунда) — устарело: на момент этой записи граница была подтверждена только
именованием, без исполняемой проверки; действующее доказательство — §5.8
пункт 2 (Rust unit-тест) и §5.9 пункт 2 (real-process случай с точным
полным набором дельты); текст ниже сохранён как есть ради читаемой истории
раунда, а не как действующее описание проверки.** Две точки в
`bounded_context_manifest.rs` (`check_resolved_state`'s и
`resolve_pinned_reference`'s собственные проверки "неизвестное поле")
обходят `serde_json::Map` (в этом workspace — `BTreeMap`, без
`preserve_order`) там, где Node-эталон обходит `Object.keys()` в порядке
исходного текста; при одновременно нескольких неизвестных полях в одной
записи, возвращённой резолвером, языки могут назвать разные поля первыми —
та же граница, что уже документирована на `resolve_cli_producer.rs`'s
`result`, а не новая. На момент ЭТОЙ записи ни один из имеющихся реальных
fixtures или мутаций её не задевал (резолвер — тестовые данные, а не
содержимое Kernel), так что исправление оставалось документированным
именованием, а не кодом, вместо непропорционального усложнения ради
ненаблюдаемого тогда случая — следующий корректирующий раунд (§5.8 пункт 2)
добавил именно такую исполняемую проверку, узко для этой цели, не
меняя это решение задним числом.

**Проверки, выполненные исполнителем (2026-09-21):** `cargo fmt --all --
--check` — чисто; `cargo build --workspace --all-targets --all-features
--locked` — чисто; `cargo test --workspace --all-targets --all-features
--locked` — **539 passed, 0 failed** (было 481 после третьего/четвёртого
корректирующего раунда 7a; +58 новых модульных тестов по семи семействам
7b, распределённых между `meridian-app::operating_model::*` и
`meridian-cli::commands::validate::*`, включая по одному
`the_real_kernel_*_schema_and_fixtures_agree` тесту на каждое семейство,
прогоняющему ВСЕ перечисленные выше реальные valid/invalid fixtures);
`cargo clippy --workspace --all-targets --all-features --locked -- -D
warnings` — чисто; `RUSTDOCFLAGS="-D warnings" cargo doc --workspace
--no-deps --locked` — чисто; `node --test test/conformance-harness.test.mjs`
— **69 passed, 0 failed** (было 62 после четвёртого корректирующего раунда
7a; +7 — по одной сквозной мутационной проверке на каждое из семи семейств
7b, `VALIDATE_MUTATION_FAMILIES_7B`); `node test/kernel-validate.test.mjs` —
293 passed, 0 failed, 0 skipped (не изменился этим раундом — ни один
Node-файл-эталон не тронут); полный `node scripts/preflight.mjs` —
самодостаточен; `git diff --check` — без ошибок; изолированная `git add -A
&& git diff --cached --check` (в отдельной полной копии рабочего дерева вне
этого репозитория, никогда не в самом репозитории) — сообщает те же две
строки о завершающих пробелах в `standards/templates/readme-template.md`,
что и предыдущий (четвёртый) раунд 7a, файле, не тронутом ни этим, ни любым
предыдущим раундом (`git diff --stat`/`git status --short` для этого пути —
пусто) — то же пред-существующее, ранее задокументированное явление, не
регрессия этого раунда. HEAD (`758fdfff55a44e79e16a895c1752d78878a1f43c`) и
ветка (`dev`) не изменились этим раундом и совпадают с состоянием на начало
раунда; рабочее дерево содержит только незакоммиченные изменения этого
раунда — никаких `branch`/`switch`, `add`, `commit`, `merge`, `rebase`,
`reset`, `stash`, `tag` или `push` не выполнялось.

**Непроверенное и намеренные расхождения.** За пределами семи мутаций
`VALIDATE_MUTATION_FAMILIES_7B` (по одной на семейство, минимум, требуемый
заданием) не проводилось отдельного состязательного раунда по образцу
второго `CHANGES_REQUESTED` над 7a (§5.5a пункт 2 и далее) — сложные
многошаговые состязательные формы (несколько одновременных
missing/wrong-type/duplicate/cross-reference случаев сверх того, что уже
покрывают сами реальные invalid-fixtures) не проверялись отдельно кросс-
языково, только через уже встроенные в реальные fixtures adversarial-случаи
(суммарно 29+34+36+48+47+12+49+65 = 320 реальных invalid-fixture-кейсов —
`functional-parity` (29) была по ошибке пропущена из этой суммы в первой
редакции этой записи, исправлено на месте, не пометкой как устаревшей, —
между семью семействами, каждый прогнанный напрямую через обе реализации).
Помимо
двух именованных границ выше (детерминированный порядок — исправлено;
BTreeMap-порядок неизвестных полей резолвера — документировано, не
исправлено кодом), намеренных расхождений с Node-эталоном в этом раунде не
вводилось.

**Точный список путей, изменённых или добавленных этим раундом:**

*Добавлены:* `meridian-app/src/operating_model/mod.rs`,
`meridian-app/src/operating_model/functional_parity.rs`,
`meridian-app/src/operating_model/instruction_source_registry.rs`,
`meridian-app/src/operating_model/task_specification.rs`,
`meridian-app/src/operating_model/execution_state.rs`,
`meridian-app/src/operating_model/role_and_human_control.rs`,
`meridian-app/src/operating_model/bounded_context_manifest.rs`,
`meridian-cli/src/commands/validate/functional_parity.rs`,
`meridian-cli/src/commands/validate/task_pattern_registry.rs`,
`meridian-cli/src/commands/validate/instruction_source_registry.rs`,
`meridian-cli/src/commands/validate/task_specification.rs`,
`meridian-cli/src/commands/validate/execution_state.rs`,
`meridian-cli/src/commands/validate/role_and_human_control.rs`,
`meridian-cli/src/commands/validate/bounded_context_manifest.rs`.

*Изменены:* `meridian-app/src/lib.rs` (реэкспорт `operating_model`),
`meridian-cli/src/commands/validate/mod.rs` (семь новых модулей подключены
в `collect()`, `BLOCKED_CHECKS` сокращён с 15 до 8 записей, обновлена
документация модуля), `meridian-cli/tests/binary_runs.rs` (ground-truth
числа `document_identity_checked` 302→333 и `blocked.len()` 15→8 — первое
исправляет уже устаревшее до начала этого раунда число, ставшее неверным
после интеграции 7a в `dev`, не изменение этого раунда по существу; второе —
прямое следствие удаления семи записей 7b), `test/conformance-harness.test.mjs`
(`VALIDATE_MUTATION_FAMILIES_7B`, семь мутаций и вспомогательные функции),
`governance/plans/meridian-rust-migration-program-plan.md` (эта запись).

*Не изменены этим раундом:* ничего из 7c/7d или пакета 8; ни один файл 7a
(`sha_provenance.rs`, `instruction_topics.rs`, `operating_foundation.rs`,
`stack_profiles.rs`, `agent_instruction_identity.rs`, `regions.rs`) не
тронут.

**Условие перехода к 7c/7d/пакету 8:** подпакет 7b отдельно исполнен,
независимо принят и интегрирован; статус передачи ниже —
`READY_FOR_ARCHITECT_REVIEW` только для 7b. Подпакет 7a остаётся
`accepted`/`integrated` (подтверждено выше); 7c и 7d остаются `planned` до
своей собственной реализации, в зафиксированном порядке 7b → 7c → 7d
(§5.5c); пакет 8 остаётся заблокирован приёмкой и интеграцией всех
подпакетов 7a–7d, не только 7a–7b.

## 5.8. Первый корректирующий раунд подпакета 7b (исполнитель, 2026-09-21)

Владелец передал пять пунктов правки после первого раунда
`CHANGES_REQUESTED` на подпакет 7b, узко ограниченного самим 7b: 7c, 7d и
пакет 8 не начаты и не затронуты этой записью. Работа снова выполнена в
рабочем дереве без Git-записей (без `branch`/`switch`, `add`, `commit`,
`merge`, `rebase`, `reset`, `stash`, `tag`, `push`) — эта запись не является
приёмкой подпакета, и он не переводится в `accepted`/`ready` этой записью.
7b остаётся `active`, статус передачи ниже — по-прежнему
`READY_FOR_ARCHITECT_REVIEW` только для 7b; 7a остаётся
`accepted`/`integrated`, 7c–7d остаются `planned`, порядок 7b → 7c → 7d
(§5.5c) и блокировка пакета 8 приёмкой и интеграцией всех 7a–7d — без
изменений.

**Пункт 1 — все семь семейств доказаны schema-valid мутацией, запускающей
именно bespoke composite-алгоритм.** §5.7's `VALIDATE_MUTATION_FAMILIES_7B`
мутировала, для пяти из семи семейств без реальных данных в Kernel
(`functional-parity`, `instruction-source-registry`,
`task-specification-contract`, `execution-state-model`,
`bounded-context-manifest`), только форму собственного fixtures-бандла
(усечение `invalid` до `[]`) — это доказывало лишь, что связующий код в
`meridian-cli` одинаково сообщает о пустом массиве, никогда не запуская сам
`evaluate*`/`check*` алгоритм в `meridian_app::operating_model::*` на
содержательном входе. Заменено на мутацию одного значения внутри уже
существующего `valid`-fixture, которую JSON Schema не отвергает (ни разу не
`uniqueItems`/тип/enum — иначе было бы неоднозначно, какой слой отловил
мутацию) и которую отклоняет только сам composite-алгоритм:

- `functional-parity` — из первого VERIFIED-fixture удалена ровно одна
  запись `post_change_evidence.contract_links` (`io.mapping`), оставляя
  `evidence.covers` нетронутым: правило 6 ("VERIFIED нужны ОБА — покрывающее
  evidence И post-change contract link");
- `instruction-source-registry` — у verified, source-missing источника
  (`recorded_state.currency`) переведён с `"stale"` на `"current"`:
  источник, известный как пропавший, не может иметь текущий снимок;
- `task-specification-contract` — к первому fixture добавлен второй
  acceptance-criterion под новым id, с тем же `statement`/`verification`,
  что и у первого (`acceptance_criteria` в схеме не несёт `uniqueItems`, в
  отличие от `constraints`/`resolved_norms` — они намеренно не тронуты по
  этой причине);
- `execution-state-model` — `payload.current_actor` первого fixture заменён
  на `/etc/passwd`: поле — обычная непустая строка без ограничения формы в
  схеме, отклоняет только `nonPortableReason`;
- `bounded-context-manifest` — `purpose` первого authoritative source
  первого fixture заменён на `/etc/passwd`: то же рассуждение, поле не
  взаимодействует с checkpoint/резолвером.

`task-pattern-registry` и `role-and-human-control` сохранили свои прежние
мутации реальных данных Kernel
(`standards/workspace/{task-pattern-registry,role-registry}.yaml`) без
изменений — они уже в §5.7 упражняли собственный composite-алгоритм, не
только связующий код.

Проверка также переписана: раньше — через общий корпус
`conformant`/`divergent` харнесса (`mutatedKernelValidateCase`/`runCase`),
чья полная сверка множества диагностик хрупка к любому постороннему
изменению в дереве (см. предыдущий, четвёртый, корректирующий раунд 7a, где
именно эта хрупкость впервые проявилась). Новая проверка
(`computeFailLines`/`assertMutationIntroducesAMatchingFail`,
`test/conformance-harness.test.mjs`) сравнивает мутированный прогон с
BASELINE, посчитанным на ТОЙ ЖЕ немутированной копии непосредственно перед
мутацией — **устарело: способ сравнения описан здесь так, как он был в
ЭТОМ раунде (пересечение множеств новых FAIL, без учёта кратности); §5.9
пункт 1 заменил его на точное равенство multiset-дельты; см. §5.9 за
действующим описанием**: утверждает, что на КАЖДОЙ стороне появляется хотя
бы одна НОВАЯ строка FAIL с префиксом `expectedFailPrefix` семейства,
отсутствующая в baseline, и хотя бы одна из новых строк побайтово совпадает
между Node и Rust. Все семь случаев проверены напрямую против реальных
бинарников; для каждого из пяти новых — ровно одна новая строка с каждой
стороны, и она совпадает побайтово.

**Пункт 2 — детерминированный BTreeMap-порядок сохранён как Rust-native
улучшение, с честно закреплённой границей.** `check_resolved_state` и
`resolve_pinned_reference` (`meridian-app/src/operating_model/bounded_context_manifest.rs`)
продолжают обходить `serde_json::Map` (в этом workspace — `BTreeMap`, без
`preserve_order`) в алфавитном порядке при перечислении неизвестных полей
резолвер-записи — не откачено к попытке воспроизвести порядок исходного
текста Node (что потребовало бы order-preserving JSON-парсера, которого этот
workspace сознательно не использует). Добавлено:

- Rust unit-тест
  `unknown_fields_on_a_resolved_entry_are_reported_in_stable_alphabetical_order_across_repeated_calls`
  — резолвер-запись с двумя неизвестными полями, чьи имена намеренно
  расставлены так, что порядок их появления в исходном тексте (`zzzz_extra_field`
  первым, `aaaa_extra_field` последним) противоположен алфавитному; пять
  независимых вызовов подряд подтверждают, что `aaaa_extra_field` всегда
  назван первым и что вывод побайтово идентичен между вызовами, не только
  одним и тем же множеством;
- отдельный real-process тест Node/Rust
  (`test/conformance-harness.test.mjs`, безымянный блок после
  `VALIDATE_MUTATION_FAMILIES_7B`) — та же намеренная расстановка имён полей
  в реальной fixtures-записи `context-manifest.fixtures.json`'s
  `resolution`, реальный Node-эталон и реальный собранный `meridian`
  запущены напрямую. Проверено явно, без обращения к общему
  `conformant`/`divergent` харнессу и без утверждения `conformant`: Node
  называет `zzzz_extra_field` первым (порядок исходного текста), Rust —
  `aaaa_extra_field` первым (алфавитный порядок) — расхождение полного
  вывода подтверждено прямым сравнением отсортированных множеств
  диагностик (не равны), и в этом же тесте закреплено, что ОБЕ стороны
  fail-closed: ненулевой код завершения и `result.ok: false`/наличие FAIL с
  обеих сторон, ни одна не считает мутированный резолвер-ответ чистым —
  **устарело: этот тест не вычислял явный baseline/дельту и не закреплял
  точный полный набор дельты, только первую строку каждой стороны; §5.9
  пункт 2 переписал его на явную baseline/дельту с точным полным набором;
  см. §5.9 за действующим описанием**.

**Пункт 3 — чистая часть `task-pattern-registry` вынесена в
`meridian_app::operating_model::task_pattern_registry`.** Composite-алгоритм
(`evaluate_task_pattern_registry` и все его внутренние проверки — id/пара
уникальность, классификация ссылок, REFACTOR/BUGFIX/initiative-правила,
`check_rule_resolution_bugfix_consistency`) перенесён в новый файл
`meridian-app/src/operating_model/task_pattern_registry.rs` как чистая
функция над уже разобранными значениями. Единственная неотделимая внешняя
граница — проверка файловой ЦЕЛИ канонической ссылки (существование,
каноникализация симлинков, членство в отслеживаемом множестве) — выражена
callback'ом `EvalContext::check_kernel_link: &dyn Fn(&str) -> Result<(), String>`,
а не вызовом `fs` изнутри `meridian-app`, — тот же приём, что уже
использует `bounded_context_manifest`'s внешняя граница разрешения. Пути
канонической ссылки, чья некорректность НЕ требует файловой системы (пустой
путь, обратный слэш, абсолютный путь или диск, необработанный `.`/`..`
сегмент), проверяются отдельной чистой функцией
`portable_relative_path_defect` ДО вызова callback'а — они никогда не
доходят до `meridian-cli`, и `meridian-cli`'s собственная
`check_kernel_link_target` больше не дублирует эти проверки, сокращённая до
ровно файловой части (`std::fs::canonicalize`/`std::fs::metadata` против
корня Kernel и отслеживаемого множества). `meridian-cli/src/commands/validate/task_pattern_registry.rs`
теперь строит этот callback в `run()`, замыкая `kernel_root` и
отслеживаемое множество, и передаёт его в `EvalContext` — файловый ввод/
вывод, разбор YAML/JSON и построение отслеживаемого множества остаются
целиком в `meridian-cli`, как и требовалось.

**Пункт 4 — governance синхронизирован.** Строка пакета 7 в §4 исправлена:
была устаревшей ("7a active … , 7b–7d planned"), теперь отражает текущее
состояние (7a `accepted`/`integrated`, 7b `active` после этого
корректирующего раунда, 7c–7d `planned`). Устранено собственное
противоречие §5.7's таблицы размещения: вводный абзац утверждал
единообразное трёхуровневое разделение для всех семи семейств, а строка
`task-pattern-registry` в той же таблице заявляла composite целиком в
`meridian-cli` — оба места исправлены на месте (не пометкой как
устаревшими: это было фактической ошибкой первой редакции, не сменой
решения), отражая пункт 3 выше. Арифметическая ошибка в сумме реальных
invalid-fixture-кейсов (§5.7, «Непроверенное и намеренные расхождения»)
исправлена: `functional-parity`'s 29 было пропущено из суммы
`34+36+48+47+12+49+65 = 291`; верная сумма —
`29+34+36+48+47+12+49+65 = 320`, тоже исправлено на месте.

**Проверки, выполненные исполнителем (2026-09-21) — устаревшие числа: см.
§5.9 ниже для действующих после второго корректирующего раунда:**
`cargo fmt --all --
--check` — чисто; `cargo build --workspace --all-targets --all-features
--locked` — чисто; `cargo test --workspace --all-targets --all-features
--locked` — **546 passed, 0 failed** (было 539 после первой редакции 7b;
+7 — Rust unit-тесты нового `meridian-app::operating_model::task_pattern_registry`
модуля и его `meridian-cli` обёртки сверх перенесённых один-в-один, плюс
новый детерминированного-порядка unit-тест в `bounded_context_manifest.rs`,
за вычетом двух удалённых из `meridian-cli`'s `task_pattern_registry.rs`
тестов пути канонической ссылки, чья проверка переехала в `meridian-app`);
`cargo clippy --workspace --all-targets --all-features --locked -- -D
warnings` — чисто; `RUSTDOCFLAGS="-D warnings" cargo doc --workspace
--no-deps --locked` — чисто; `node --test test/conformance-harness.test.mjs`
— **73 passed, 0 failed** (было 69 после первой редакции 7b; +4 — новый
безымянный блок пункта 2's четырёх real-process проверок; семь мутаций
`VALIDATE_MUTATION_FAMILIES_7B` заменены на schema-valid форму, не добавлены
и не удалены как проверки, отсюда не +7); `node test/kernel-validate.test.mjs`
— 293 passed, 0 failed, 0 skipped (не изменился этим раундом — ни один
Node-файл-эталон не тронут); полный `node scripts/preflight.mjs` —
самодостаточен; `git diff --check` — без ошибок; изолированная `git add -A
&& git diff --cached --check` (в отдельной полной копии рабочего дерева вне
этого репозитория, никогда не в самом репозитории) — сообщает те же две
строки о завершающих пробелах в `standards/templates/readme-template.md`,
файле, не тронутом ни этим, ни любым предыдущим раундом (`git diff --stat`/
`git status --short` для этого пути — пусто) — то же пред-существующее,
ранее задокументированное явление, не регрессия этого раунда. HEAD
(`758fdfff55a44e79e16a895c1752d78878a1f43c`) и ветка (`dev`) не изменились
этим раундом и совпадают с состоянием на начало раунда; рабочее дерево
содержит только незакоммиченные изменения — никаких `branch`/`switch`,
`add`, `commit`, `merge`, `rebase`, `reset`, `stash`, `tag` или `push` не
выполнялось.

**Точный список путей, изменённых или добавленных этим (корректирующим)
раундом:**

*Добавлены:* `meridian-app/src/operating_model/task_pattern_registry.rs`.

*Изменены:* `meridian-app/src/operating_model/mod.rs` (подключён новый
модуль), `meridian-app/src/operating_model/bounded_context_manifest.rs`
(пункт 2 — `RecordResolver` именованный тип вместо инлайн `dyn Fn`-типа для
`clippy::type_complexity`, новый детерминированного-порядка unit-тест, два
уточняющих doc-комментария у существующих `for k in obj.keys()`),
`meridian-cli/src/commands/validate/task_pattern_registry.rs` (пункт 3 —
переписан в тонкую файловую обёртку над перенесённым composite-алгоритмом),
`test/conformance-harness.test.mjs` (пункт 1 — семь мутаций
`VALIDATE_MUTATION_FAMILIES_7B` заменены на schema-valid форму и новую
baseline-diff проверку; пункт 2 — новый real-process блок из четырёх
проверок), `governance/plans/meridian-rust-migration-program-plan.md` (эта
запись; §4 строка пакета 7; §5.7 исправления пунктов 3–4).

*Не изменены этим раундом:* ничего из 7a, 7c, 7d или пакета 8; шесть из семи
`meridian_app::operating_model::*` модулей 7b
(`functional_parity`, `instruction_source_registry`, `task_specification`,
`execution_state`, `role_and_human_control`) и все шесть соответствующих
`meridian-cli` модулей, кроме `task_pattern_registry.rs`, не тронуты.

**Условие перехода к 7c/7d/пакету 8:** без изменений — подпакет 7b отдельно
исполнен, независимо принят и интегрирован; статус передачи —
`READY_FOR_ARCHITECT_REVIEW` только для 7b.

## 5.9. Второй корректирующий раунд подпакета 7b (исполнитель, 2026-09-21)

Владелец передал четыре пункта правки после второго раунда
`CHANGES_REQUESTED` на подпакет 7b, узко ограниченного самим 7b: 7c, 7d и
пакет 8 не начаты и не затронуты этой записью. Работа снова выполнена в
рабочем дереве без Git-записей (без `branch`/`switch`, `add`, `commit`,
`merge`, `rebase`, `reset`, `stash`, `tag`, `push`) — эта запись не является
приёмкой подпакета, и он не переводится в `accepted`/`ready` этой записью.
7b остаётся `active`, статус передачи ниже — по-прежнему
`READY_FOR_ARCHITECT_REVIEW` только для 7b; 7a остаётся
`accepted`/`integrated`, 7c–7d остаются `planned`, порядок 7b → 7c → 7d
(§5.5c) и блокировка пакета 8 приёмкой и интеграцией всех 7a–7d — без
изменений. Ни один Rust-файл этим раундом не тронут — оба пункта ниже
касаются только `test/conformance-harness.test.mjs`.

**Пункт 1 — пересечение множеств заменено на точное равенство
multiset-дельты.** §5.8's `assertMutationIntroducesAMatchingFail`
утверждала только, что хотя бы одна новая строка с нужным префиксом
пересекается между Node и Rust — это не отклоняло случай, где одна сторона
сообщает дополнительную, ничем не объяснённую диагностику сверх общей, или
где та же строка встречается разное число раз. Заменено на:
`multisetCounts`/`multisetsEqual`/`multisetDelta` — дельта считается как
мультимножество (`Map<строка, количество>`) МУТИРОВАННОГО прогона
относительно BASELINE, посчитанного на той же копии непосредственно перед
мутацией, с учётом кратности: строка, появившаяся в мутированном прогоне на
N раз больше, чем в baseline, входит в дельту ровно N раз.
`assertMutationIntroducesAMatchingFail` теперь утверждает: обе дельты (Node
и Rust) непустые, обе содержат хотя бы одну строку с `expectedFailPrefix`
семейства, и обе дельты равны друг другу КАК МУЛЬТИМНОЖЕСТВА — не только
пересекаются. Проверено против всех семи семейств: каждая из ранее
подтверждённых хирургических (§5.8) мутаций даёт дельту из ровно одной
строки на каждой стороне, и эта строка совпадает побайтово — точное
равенство мультимножеств проходит без изменения самих мутаций.

**Пункт 2 — BTreeMap real-process случай считает явные baseline/дельту и
закрепляет точный полный набор.** Прежняя версия (§5.8 пункт 2) не считала
baseline вовсе — она мутировала пустое дерево и читала `nodeUnknown[0]`/
`rustUnknown[0]` напрямую, не отделяя эффект мутации от того, что уже было
в дереве, и не утверждая, что никаких ДРУГИХ новых FAIL не появилось.
Переписано на тот же `computeFailLines`/`multisetDelta`, что и
`VALIDATE_MUTATION_FAMILIES_7B`: baseline считается на немутированной копии
ДО записи резолвер-ответа с двумя неизвестными полями, дельта — после.
Закреплено явно, шестью проверками:

1. обе дельты непустые и целиком состоят из строк `"unknown field"`;
2. вся Node-дельта называет `"zzzz_extra_field"` (порядок исходного текста
   JSON) и НИ ОДНА строка не называет `"aaaa_extra_field"` — точный полный
   набор, не только первая строка;
3. вся Rust-дельта называет `"aaaa_extra_field"` (алфавитный порядок
   `BTreeMap`) и ни одна строка не называет `"zzzz_extra_field"`;
4. Node-дельта после текстовой замены `"zzzz_extra_field"` →
   `"aaaa_extra_field"` совпадает с Rust-дельтой как мультимножество —
   доказывает, что единственное различие между сторонами это ИМЯ первого
   названного поля, а не какая-то ещё скрытая расходящаяся диагностика;
5. обе стороны действительно fail-closed (ненулевой код завершения,
   `result.ok: false` у Rust) — тем же прогоном бинарников, что и раньше;
6. полный вывод (`mutated.nodeFails`/`mutated.rustFails`, не только
   дельта) действительно не равен как мультимножество — случай честно не
   маркируется `conformant`.

Эмпирически подтверждено: мутация задевает ДВЕ реальные valid-fixture
записи `context-manifest.fixtures.json` (`reference manifest` и
`participant switch`), обе ссылающиеся на один и тот же резолвер-ключ
`records/execution-run/example-run-001` — обе дельты (Node и Rust) состоят
из двух строк, по одной на каждую задетую fixture, и проверка 4 выше
подтверждает их точное соответствие после замены имени поля, а не только
что «какая-то» строка совпадает.

**Пункт 3 — исторический абзац §5.7 синхронизирован с текущим состоянием.**
§5.7's абзац «Доказательство переноса» указывал только на §5.8 как на
действующее описание проверки; после этого раунда способ проверки в §5.8
сам устарел (пункт 1 выше). §5.7 обновлён: ссылается на §5.8 за точными
мутациями (не устарели) и на §5.9 (эту запись) за действующим способом
проверки (multiset-дельта, не пересечение). §5.8's собственный текст также
отмечен на месте: описание способа сравнения в пункте 1, описание
real-process случая в пункте 2 и итоговые числа проверок помечены
устаревшими со ссылкой сюда — не переписаны и не удалены, история раунда
остаётся читаемой.

**Проверки, выполненные исполнителем (2026-09-21):** `cargo fmt --all --
--check` — чисто; `cargo build --workspace --all-targets --all-features
--locked` — чисто; `cargo test --workspace --all-targets --all-features
--locked` — **546 passed, 0 failed** (без изменения по сравнению с первым
корректирующим раундом — этот раунд не тронул ни одного Rust-файла);
`cargo clippy --workspace --all-targets --all-features --locked -- -D
warnings` — чисто; `RUSTDOCFLAGS="-D warnings" cargo doc --workspace
--no-deps --locked` — чисто; `node --test test/conformance-harness.test.mjs`
— **75 passed, 0 failed** (было 73 после первого корректирующего раунда;
+2 — BTreeMap real-process случай пункта 2 разделён на шесть проверок
вместо четырёх, при этом ни одна из семи проверок `VALIDATE_MUTATION_FAMILIES_7B`
не добавлена и не удалена, только усилен способ проверки внутри каждой);
`node test/kernel-validate.test.mjs` — 293 passed, 0 failed, 0 skipped (не
изменился этим раундом — ни один Node-файл-эталон не тронут); полный `node
scripts/preflight.mjs` — самодостаточен; `git diff --check` — без ошибок;
изолированная `git add -A && git diff --cached --check` (в отдельной полной
копии рабочего дерева вне этого репозитория, никогда не в самом
репозитории) — сообщает те же две строки о завершающих пробелах в
`standards/templates/readme-template.md`, файле, не тронутом ни этим, ни
любым предыдущим раундом (`git diff --stat`/`git status --short` для этого
пути — пусто) — то же пред-существующее, ранее задокументированное явление,
не регрессия этого раунда. HEAD (`758fdfff55a44e79e16a895c1752d78878a1f43c`)
и ветка (`dev`) не изменились этим раундом и совпадают с состоянием на
начало раунда; рабочее дерево содержит только незакоммиченные изменения —
никаких `branch`/`switch`, `add`, `commit`, `merge`, `rebase`, `reset`,
`stash`, `tag` или `push` не выполнялось.

**Точный список путей, изменённых или добавленных этим (вторым
корректирующим) раундом:**

*Добавлены:* ничего.

*Изменены:* `test/conformance-harness.test.mjs` (пункт 1 —
`multisetCounts`/`multisetsEqual`/`multisetDelta` и переписанная
`assertMutationIntroducesAMatchingFail`; пункт 2 — BTreeMap real-process
блок переписан на явные baseline/дельту и шесть точных проверок),
`governance/plans/meridian-rust-migration-program-plan.md` (эта запись;
пункт 3 — синхронизация §5.7 и пометка устаревших мест §5.8).

*Не изменены этим раундом:* ничего из 7a, 7c, 7d или пакета 8; ни один
Rust-файл (`meridian-app/src/operating_model/*`,
`meridian-cli/src/commands/validate/*`) не тронут; сами семь мутаций
`VALIDATE_MUTATION_FAMILIES_7B` (какие значения меняются) и мутация
BTreeMap-случая (расстановка имён полей) не изменены — изменился только
способ их проверки.

**Условие перехода к 7c/7d/пакету 8:** без изменений — подпакет 7b отдельно
исполнен, независимо принят и интегрирован; статус передачи —
`READY_FOR_ARCHITECT_REVIEW` только для 7b.

## 5.10. Приёмка и интеграция подпакета 7b (2026-09-21)

Владелец подтвердил приёмку и локальную интеграцию подпакета 7b
(`validate-operating-contracts`) в `dev`. Пакетный коммит
`04757df1ba896e9a1a5ecff9a1de49d85ee77ac1` (`feat(cli): перенести
операционные проверки validate`) и коммит слияния
`07229f6a6bd0ebbaf5957e6549d990fedbb71fe2` (`merge(dev): принять
feature/validate-operating-contracts — перенести операционные проверки
validate`) — дерево пакетного коммита равно дереву коммита слияния
(`fc6d9b001dcc2cc2590884a3b82137f4275fa702` на обеих сторонах,
`git rev-parse 04757df1b^{tree} 07229f6a6^{tree}`), пакетный коммит
достижим из коммита слияния (`git merge-base --is-ancestor 04757df1b
07229f6a6`), а коммит слияния — обычный двухродительский коммит слияния,
первый родитель которого (`758fdff`) — предыдущий принятый коммит слияния
пакета 7a, второй — сам пакетный коммит 7b. Подпакет 7b переходит в
`accepted`/`integrated` (§4). Эта запись — приёмочное подтверждение, а не
пересказ реализации 7b — сама реализация, её доказательства и оба
корректирующих раунда остаются задокументированы в §5.7–§5.9 без изменений.

Подпакет 7a остаётся `accepted`/`integrated` (подтверждено ранее). Подпакет
7c становится `active` с этой же даты; его собственная передача — §5.11.
Подпакет 7d остаётся `planned`. Пакет 8 остаётся заблокирован приёмкой и
интеграцией всех подпакетов 7a–7d — 7a и 7b теперь приняты и
интегрированы, 7c и 7d — нет.

## 5.11. Подпакет 7c `validate-evidence-and-intake` (исполнитель, 2026-09-21)

Владелец назначил исполнителю реализацию подпакета 7c
(`validate-evidence-and-intake`, §5.5c): четыре из восьми оставшихся
заблокированных семейств `validate` — `evidence-and-handoff-contract`,
`meridian-field-evaluation`, `controlled-rule-intake`,
`existing-project-compatibility-mode` — узко в границах самого 7c: пакет 8
и подпакет 7d не начаты и не затронуты этой записью. Работа выполнена в
рабочем дереве без Git-записей (без `branch`/`switch`, `add`, `commit`,
`merge`, `rebase`, `reset`, `stash`, `tag`, `push`) — интегратор Git не
назначен исполнителю, и эта запись не является приёмкой подпакета: 7c не
переводится в `accepted`/`ready` этой записью, только в состояние передачи
`READY_FOR_ARCHITECT_REVIEW`, заявленное ниже.

**Четыре перенесённых семейства.** Каждое — специализированная JSON Schema
плюс собственный bespoke composite-consistency алгоритм из
`scripts/lib/*.mjs`, перенесённый в `meridian_app::operating_model::*` —
чистую функцию над уже разобранными значениями, без файлового, Git-, env-
или process-ввода/вывода. Файловое чтение (схема, fixtures, построение
resolver-границы из fixtures-данных) остаётся в собственном модуле
`meridian-cli/src/commands/validate/*.rs` каждого семейства — то же
трёхуровневое разделение `meridian-core`/`meridian-app`/`meridian-cli`,
которое 7a и 7b уже применили единообразно:

- `evidence_and_handoff.rs` (`meridian-app/src/operating_model/`, порт
  `scripts/lib/evidence-and-handoff.mjs`): портированы все 20 пронумерованных
  разделов Node-эталона — закрытая ссылка `$schema`, run-state scope с
  детерминированной связью `scope.id`/`execution_run_ref.id`, четыре
  закрытые структурированные pinned-ссылки (`execution_run_ref`,
  `task_specification_ref`, `human_control_ref`, `context_manifest_ref`) с
  закрытым правилом точной ревизии, отдельные id-связанные списки claimed
  results/verifiable assertions/evidence, established/not_established как
  вычисляемое (не заявляемое) состояние, три статуса обязательной проверки
  (`passed`/`failed`/`unable`) с проверкой по `check_ref`, закрытие
  acceptance-criteria и ПОЛНОГО набора `mandatory_checks` против
  разрешённой спецификации, source/result state по репозиториям, closed
  worktree-disposition (`version-control-flow.md` §5.4), `outcome.status`
  и, наконец, границу внешнего разрешения (§20) — четыре pinned-ссылки И
  каждая запись evidence, разрешённые через резолвер и проверенные против
  реального состояния execution-run. `meridian_core::evidence::aggregate`
  (`verify_assertion`/`claimed_result_status`, уже перенесённые ранее)
  проверены на применимость и остались НЕ переиспользованы напрямую:
  их API берёт уже типизированные `ClaimedResult`/`VerifiableAssertion` и
  предрешённую карту `ResolvedEvidenceResult`, тогда как composite-алгоритм
  работает над сырым `serde_json::Value` на всех 20 этапах (как и все
  остальные операционные семейства 7a–7c) и вычисляет established/verified
  по ТОЙ ЖЕ логике самостоятельно (§9, §11, §20б) — подгонка под чужой тип
  ради формального переиспользования была бы точно тем «несовпадающим
  контрактом», который задание прямо запрещает подгонять искусственно;
  переиспользованы вместо этого `classify_revision`,
  `REQUIRED_RESOLVED_STATE_FIELDS`, `RESOLVED_ENTRY_COMMON_KEYS` из
  `bounded_context_manifest` и `non_portable_reason`/`resolve_schema_ref` из
  `task_specification` — ровно тот же набор, что Node-эталон реально
  импортирует из `context-manifest.mjs`, не более и не менее;
- `field_evaluation.rs` (порт `scripts/lib/field-evaluation.mjs`):
  `evaluateFieldEvaluation`, ветвящийся по `record_type` на
  `field-evaluation-observation`/`field-evaluation-report` — восемь
  характеристик плана §10 с фиксированным per-metric видом измерения,
  закрытый пул исходов классификации per-metric, обязательное pinned
  evidence с закрытым `evidence-result` (`observed_result` И
  `metric_ref`), пара supersedes/correction_reason, для отчёта —
  сопоставимость по workspace/периоду, запрет двойного счёта включая пару
  superseded+superseding, полнота 8-из-8 `per_metric` и ТОЧНЫЙ пересчёт
  каждого агрегата из разрешённой, поимённой выборки
  (`compute_metric_aggregate`, порт `computeMetricAggregate`). Портирован
  только `evaluateFieldEvaluation` — единственная функция, которую вызывает
  гейт `scripts/kernel-validate.mjs`; `buildFieldEvaluationReport`
  (детерминированный строитель отчёта Node-эталона) не входит в контракт
  `validate` и не перенесён. Даты/время проверяются собственной
  календарной арифметикой (алгоритм Говарда Хиннанта `days_from_civil`,
  тот же proleptic-григорианский принцип, что уже использует
  `is_leap_year`), без стороннего date/time-крейта — новая зависимость не
  вводилась ради двух точек сравнения интервалов;
- `controlled_rule_intake.rs` (порт `scripts/lib/controlled-rule-intake.mjs`):
  одиннадцать свойств контракта — pinned `source_ref` с ОБЯЗАТЕЛЬНЫМИ
  ревизией И SHA-256 (не гибкий выбор «или-или» остальных пакетов),
  разрешённый через границу и проверенный против `recorded_state`
  ВКЛЮЧАЯ currency, минимальную проекцию `read_channel` (свойство 10,
  переиспользуя `instruction_source_registry::check_read_channel`,
  `READ_CHANNEL_KINDS`, `MERIDIAN_VISIBILITY`, `CURRENCY` — не
  расходящуюся копию), `origin.kind`/`origin.source_ref`, именующий ТОТ ЖЕ
  источник, что и `payload.source_ref.id`, явную кластеризацию по
  `semantic_key` с согласованными scope/decision/conflicts_with между
  origin-ами одного кластера, симметричный граф конфликтов, вычисляемый
  один раз над ВСЕМ набором кандидатов, и закрытое трёхзначное
  `applicability_state` с человеческим владельческим authority по scope
  (`delegated-run` никогда не решает применимость сам);
- `existing_project_compatibility_mode.rs` (порт
  `scripts/lib/existing-project-compatibility-mode.mjs`'s
  `evaluateExistingProjectCompatibilityMode`): композиция, а не
  конкурирующий формат — `discovered_sources` проверяются реальным
  `evaluate_instruction_source_registry`, `rule_candidates[].candidate` —
  реальным `evaluate_controlled_rule_intake` против резолвера,
  ПОСТРОЕННОГО этим же модулем (`build_source_resolver`) из
  `discovered_sources` того же скана, а не отдельного внешнего резолвера;
  ограниченный discovery-план, где каждый слот несёт РОВНО один исход
  (обнаружен/отсутствует/недоступен), полная непересекающаяся партиция;
  правило «изменение не остаётся без finding»; дискавери не выдаёт
  решения (`discovery_status: "new"` требует `applicability_state:
  "candidate"`); единственный закрытый приоритет `next_step`
  (`compute_next_step`); и обязательная проверка ЧЕТЫРЁХ реальных
  composed-схем (`registrySchema`, `envelopeSchema`,
  `sourceRegistrySchema`, `ruleIntakeSchema`) по их $id/registry_id/entries-
  форме/entry record_type — независимо от того, заполнены ли
  `discovered_sources`/`rule_candidates` в конкретном документе.
  `scanDiscoveryPlan` (файлово-зависимое, только `fs.lstatSync`/
  `realpathSync`/`statSync`/`readFileSync`, ни одной записи) НЕ перенесён:
  `scripts/kernel-validate.mjs`'s гейт вызывает только
  `evaluateExistingProjectCompatibilityMode`, и внесение файлового
  ввода/вывода в `meridian-app` не требовалось для реального прогона
  fixtures — граница callback/port, которую задание прямо разрешило не
  открывать, когда она не нужна.

**Границы задания подтверждены.** Composite-алгоритмы размещены в
`meridian-app::operating_model` без файлового/Git-/env-/process-
ввода/вывода (проверено чтением каждого модуля — единственный внешний вход
каждого это уже разобранный `serde_json::Value` и резолвер-функция);
файловое чтение схем/fixtures и построение resolver-границ из
fixtures-данных остаётся в `meridian-cli`; `scanDiscoveryPlan` не перенесён
в `meridian-app` (см. выше); `evidence-and-handoff` проверил применимость
`meridian_core::evidence::aggregate` и не стал её подгонять под
несовпадающий контракт (см. выше); `existing-project-compatibility-mode`
композиционно вызывает РЕАЛЬНЫЕ `evaluate_instruction_source_registry` и
`evaluate_controlled_rule_intake`, а не упрощённые локальные проверки —
резолвер для controlled-rule-intake строится из данных, которые сам скан
уже несёт (`discovered_sources`), в точности как Node-эталон.

**`BLOCKED_CHECKS` сокращён с 8 до 4 записей.** Удалены ровно четыре записи
7c (`evidence-and-handoff-contract`, `meridian-field-evaluation`,
`controlled-rule-intake`, `existing-project-compatibility-mode`).
Оставшиеся ровно четыре записи — все 7d, в неизменном порядке:
`instance-data-migration`, `instance-canonical-export`,
`workspace-compatibility-qualification`,
`upgrade-integration-qualification`. Итоговый предикат
`ok = failures.is_empty() && BLOCKED_CHECKS.is_empty()` (§5.5a пункт 1) не
изменён и не ослаблен: код `0`/`status: "ok"` по-прежнему недостижим, пока
`BLOCKED_CHECKS` непусто — на этом дереве это подтверждает
`real-node-rust-cli-validate-clean-kernel` (единственное расхождение с
Node-эталоном по-прежнему код завершения из-за непустого
`BLOCKED_CHECKS`, диагностики совпадают полностью — см. проверки ниже).

**Точные fixture counts** (реальные `registries/operating-model/fixtures/
*.fixtures.json`, ни одного синтетического примера):

| Семейство | valid | invalid |
|---|---|---|
| `evidence-and-handoff-contract` | 8 | 95 |
| `meridian-field-evaluation` | 24 | 36 |
| `controlled-rule-intake` | 7 | 49 |
| `existing-project-compatibility-mode` | 10 | 17 |
| **Итого** | **49** | **197** |

Все 49 valid и 197 invalid fixtures прогнаны через реальный
`meridian_app::operating_model::*` composite-алгоритм каждого семейства
(тест `the_real_kernel_*_schema_and_fixtures_agree` в собственном модуле
`meridian-cli/src/commands/validate/*.rs`) — ни одна не пропущена, ни одна
не заменена синтетической.

**Резолвер-ответы: несколько неизвестных полей, wrong-type, отсутствующая
запись, закрытый набор полей.** Для каждого из трёх семейств с собственной
resolver-границей (`evidence-and-handoff`, `meridian-field-evaluation`,
`controlled-rule-intake`) добавлен Rust-модульный тест, проверяющий ОДНИМ
резолвер-ответом сразу два одновременных неизвестных поля, названных так,
что порядок их исходного текста ПРОТИВОПОЛОЖЕН алфавитному — этот тест
пройден одинаково детерминированно на трёх повторных вызовах подряд, что
подтверждает аудит `Object.keys`/`Map`/`Set` против
`BTreeMap`/`HashMap`/`HashSet` пункта ниже. Отдельные тесты проверяют:
wrong-type поле резолвер-ответа (`revision_verified` строкой вместо
булева — `controlled-rule-intake`; `resolved_state: null` —
`evidence-and-handoff`) и одновременно отсутствующее обязательное поле
(`recorded_state.revision` отсутствует) дают закрытые, поимённые ошибки, а
не тихий пропуск; резолвер, ничего не находящий (`None`), даёт
непустой `problems` и не паникует
(`existing_project_compatibility_mode::build_source_resolver` — три
отдельных случая: неизвестный id, источник без `recorded_state`,
`Value::Null` на входе). `existing-project-compatibility-mode` не несёт
собственной resolver-границы для источников (резолвер строится из уже
разобранных `discovered_sources`, не из внешнего вызова) — её эквивалент
проверки закрытого набора полей это тест на отсутствующую/неприменимую
composed-схему (`evaluate_on_missing_schema_identity_fails_closed_without_panicking`),
дающий ровно ОДНУ диагностику, называющую все четыре недостающих ключа
`opts` разом, без паники на `Value::Null`-документе.

**Аудит `Object.keys`/`Map`/`Set` против
`BTreeMap`/`HashMap`/`HashSet`.** Ровно тот же именованный, принятый в 7b
рубеж (`bounded_context_manifest::check_resolved_state`/
`resolve_pinned_reference`, `meridian-rust-migration-program-plan.md` §5.7):
`serde_json::Value::Object` — это `BTreeMap` во всём рабочем пространстве
(флаг `preserve_order` нигде не включён), так что проверка неизвестных
полей резолвер-ответа идёт в стабильном алфавитном порядке, никогда не в
порядке исходного текста `Object.keys()` — когда неизвестных полей больше
одного одновременно, два языка могут назвать разное поле первым в тексте
диагностики, но каждый называет их ВСЕ. Это поведение СОХРАНЕНО, а не
исправлено под Node, как более надёжное детерминированное поведение
(`meridian-rust-migration-program-plan.md` §5.7), и закреплено
модульными тестами трёх семейств, названными выше
(`..._reports_every_unknown_field_deterministically_and_never_panics`).
Реальный сквозной случай этого расхождения — точно тот же
zzzz/aaaa-резолвер-ответ, что уже доказан для
`bounded-context-manifest` в 7b (§5.9 пункт 2) — не задет и не изменён
этим подпакетом: те шесть проверок продолжают проходить без изменений
(см. `node --test test/conformance-harness.test.mjs` ниже), а четыре новых
семейства 7c проверены реальными, а не синтетическими, невалидными
fixtures, которые эту же природу расхождения не пересекают (ни одна
реальная invalid-fixture четырёх семейств 7c не зависит от порядка
перечисления неизвестных полей — все реальные проверки полагаются на
МНОЖЕСТВО diagnostics, не на их порядок).

**Отсутствие panic/fail-open на malformed fixtures и resolver-ответах.**
Явно проверено модульными тестами каждого из четырёх семейств:
`evaluate_evidence_and_handoff`/`evaluate_field_evaluation`/
`evaluate_controlled_rule_intake`/`evaluate_existing_project_compatibility_mode`
на документе `Value::Null` не паникуют и возвращают непустой `problems`
(fail-closed, не «документ пуст — значит чист»); резолвер, отсутствующий
или ничего не находящий, даёт явную диагностику «cannot be verified: no
external record resolver was supplied» / «does not resolve to an actual …»
и никогда не трактует непроверенное как валидное. Ни один `.unwrap()` в
составе самого composite-алгоритма не введён на данных, приходящих из
документа или резолвер-ответа (единственные `.unwrap()` в этих четырёх
модулях — на статически известных regex-компиляциях в `LazyLock`, тот же
шаблон, что 7a/7b уже используют, и на CLI-обёрточном коде после
собственных `bundle_ok`/`ok`-гейтов, которые уже проверили нужную форму).

**Governance-запись переноса.** `governance/plans/
meridian-rust-migration-program-plan.md` — эта запись; §4 обновлён
(7a–7b accepted/integrated, 7c active/READY_FOR_ARCHITECT_REVIEW, 7d
planned); §5.10 записывает приёмку/интеграцию 7b отдельной записью с SHA
`04757df1ba896e9a1a5ecff9a1de49d85ee77ac1`/
`07229f6a6bd0ebbaf5957e6549d990fedbb71fe2`, как поручил владелец. Пакет 8
остаётся заблокирован приёмкой и интеграцией ВСЕХ подпакетов 7a–7d — 7a и
7b теперь приняты, 7c передан на независимую проверку, 7d не начат.

**Проверки, выполненные исполнителем (2026-09-21):** `cargo fmt --all --
--check` — чисто; `cargo build --workspace --all-targets --all-features
--locked` — чисто; `cargo test --workspace --all-targets --all-features
--locked` — **572 passed, 0 failed** (было 546 к концу 7b/§5.9; прирост —
19 новых модульных тестов в `meridian-app::operating_model::{evidence_and_handoff,
field_evaluation, controlled_rule_intake, existing_project_compatibility_mode}`
и 4 новых `the_real_kernel_*_schema_and_fixtures_agree` в
`meridian-cli::commands::validate::*`, прогоняющих ВСЕ 49 valid и 197
invalid реальных fixtures четырёх семейств); `cargo clippy --workspace
--all-targets --all-features --locked -- -D warnings` — чисто;
`RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked` —
чисто (после исправления двух `rustdoc::broken_intra_doc_links` на
`[`scan_discovery_plan`]`, символе, намеренно отсутствующем в этом модуле, —
заменено на обычный текст `scanDiscoveryPlan` без ссылки); `node --test
test/conformance-harness.test.mjs` — **79 passed, 0 failed** (было 75 к
концу 7b; +4 — по одной сквозной schema-valid мутационной проверке на
каждое из четырёх семейств 7c, `VALIDATE_MUTATION_FAMILIES_7C`, той же
`assertMutationDeltaMultisetsEqual` multiset-точности, что и
`VALIDATE_MUTATION_FAMILIES_7B`; все прежние 75 проверок, включая шесть
BTreeMap-проверок §5.9 пункта 2 и семь мутаций 7b, продолжают проходить
без изменений); `node test/kernel-validate.test.mjs` — 293 passed, 0
failed, 0 skipped (не изменился этим подпакетом — ни один Node-файл-эталон
не тронут); полный `node scripts/preflight.mjs` — самодостаточен; `git
diff --check` — без ошибок на отслеживаемых изменённых файлах; для восьми
новых неотслеживаемых файлов — `git diff --no-index --check /dev/null
<файл>` на каждый (индекс не трогался: `git add`/`add -N` не выполнялись)
— чисто на всех восьми; изолированная `rsync`-копия рабочего дерева (без
`.git`/`target`) в отдельный временный каталог вне этого репозитория,
`git init` и `git add -A && git diff --cached --check` ТАМ (никогда не в
самом репозитории) — сообщает те же две строки о завершающих пробелах в
`standards/templates/readme-template.md`, файле, не тронутом ни этим, ни
любым предыдущим подпакетом (`git status --short` для этого пути в
рабочем дереве — пусто) — то же пред-существующее, ранее
задокументированное явление (§5.7, §5.9), не регрессия этого подпакета.
Ветка `dev` и `HEAD` не изменялись этим подпакетом (переход §5.10 —
отдельная, предшествующая запись, отражающая уже состоявшееся решение
владельца, не действие исполнителя); рабочее дерево содержит только
незакоммиченные изменения этого подпакета — никаких `branch`/`switch`,
`add`, `commit`, `merge`, `rebase`, `reset`, `stash`, `tag` или `push` не
выполнялось.

**Ground-truth cross-check.** `document_identity_checked` в
`meridian-cli/tests/binary_runs.rs` исправлен с 333 на 348 и
`blocked.len()` — с 8 на 4. Первое — не изменение по существу этого
подпакета: реальное число (348) уже было верным на коммите слияния 7b
`07229f6` (`git ls-tree -r --name-only 07229f6 | wc -l` → 348), то есть
константа 333 устарела ДО начала работы над 7c — та же природа, что и
исправление 302→333 в самом 7b (§5.7), только на этот раз расхождение
осталось незамеченным при слиянии 7b. Восемь новых файлов 7c не
отслеживаются Git и не входят в этот подсчёт. Второе — прямое следствие
удаления четырёх записей 7c из `BLOCKED_CHECKS`. Оба числа сверены
напрямую с `node scripts/kernel-validate.mjs` на этом дереве: `OK
document-identity: 348 tracked names conform`, и Node-эталон сообщает
ровно `8`/`24`/`7`/`10` satisfied и `95`/`36`/`49`/`17` rejected для
четырёх семейств 7c — побайтово те же числа, что в таблице fixture counts
выше.

**Непроверенное и намеренные расхождения.** За пределами четырёх мутаций
`VALIDATE_MUTATION_FAMILIES_7C` (по одной на семейство, минимум, требуемый
заданием) не проводилось отдельного состязательного раунда по образцу
второго `CHANGES_REQUESTED` над 7a — сложные многошаговые состязательные
формы сверх того, что уже покрывают сами реальные invalid-fixtures, не
проверялись отдельно кросс-языково (суммарно 95+36+49+17 = 197 реальных
invalid-fixture-кейсов между четырьмя семействами, каждый прогнанный
напрямую через обе реализации). Новых намеренных поведенческих расхождений
с Node-эталоном в этом подпакете не вводилось: `js_number` (в
`field_evaluation.rs`) — механизм ТОЧНОГО соответствия, не расхождение —
JS не различает целое и дробное число одного значения при
`JSON.stringify` (`600` и `600.0` печатаются одинаково), тогда как
`serde_json::Number` различает свои целочисленный и дробный варианты;
`js_number` перекодирует вычисленное целое `f64` обратно в целочисленный
вариант перед сравнением с рукописным fixture-литералом — без этого шага
`compute_metric_aggregate` расходился бы с эталоном на первой же duration/
count-метрике (обнаружено и исправлено реальным прогоном fixtures, не
угадано заранее). Единственная реализационная (не поведенческая) заметка:
`field_evaluation.rs` сравнивает два ISO-подобных timestamp на порядок
через собственную календарную арифметику (`days_from_civil`, алгоритм
Говарда Хиннанта) вместо стороннего date/time-крейта — для уже
провалидированных (`is_valid_date_time`) строк это даёт побайтово те же
результаты сравнения, что `Date.parse` эталона, и не вводит новую
Cargo-зависимость ради двух точек сравнения интервалов. Ранее
задокументированное расхождение BTreeMap/`Object.keys()` (§5.7, §5.9)
сохранено без изменений, применяется теперь и к 7c (см. выше) и не
объявляется заново как новое.

**Точный список путей, изменённых или добавленных этим подпакетом:**

*Добавлены:* `meridian-app/src/operating_model/evidence_and_handoff.rs`,
`meridian-app/src/operating_model/field_evaluation.rs`,
`meridian-app/src/operating_model/controlled_rule_intake.rs`,
`meridian-app/src/operating_model/existing_project_compatibility_mode.rs`,
`meridian-cli/src/commands/validate/evidence_and_handoff.rs`,
`meridian-cli/src/commands/validate/field_evaluation.rs`,
`meridian-cli/src/commands/validate/controlled_rule_intake.rs`,
`meridian-cli/src/commands/validate/existing_project_compatibility_mode.rs`.

*Изменены:* `meridian-app/src/operating_model/mod.rs` (четыре новых модуля
подключены), `meridian-cli/src/commands/validate/mod.rs` (четыре новых
модуля подключены в `collect()`, `BLOCKED_CHECKS` сокращён с 8 до 4
записей, обновлена документация модуля), `meridian-cli/tests/binary_runs.rs`
(ground-truth числа `document_identity_checked` 333→348 и `blocked.len()`
8→4 — см. раздел выше), `test/conformance-harness.test.mjs`
(`VALIDATE_MUTATION_FAMILIES_7C`, четыре мутации и вспомогательные
функции), `governance/plans/meridian-rust-migration-program-plan.md` (§4,
§5.10, эта запись).

*Не изменены этим подпакетом:* ничего из 7d или пакета 8; ни один файл 7a
или 7b не тронут (`sha_provenance.rs`, `instruction_topics.rs`,
`operating_foundation.rs`, `stack_profiles.rs`,
`agent_instruction_identity.rs`, `regions.rs`, `functional_parity.rs`,
`task_pattern_registry.rs`, `instruction_source_registry.rs`,
`task_specification.rs`, `execution_state.rs`, `role_and_human_control.rs`,
`bounded_context_manifest.rs` и их CLI-обёртки не изменены — только
переиспользованы через `use super::...`).

**Условие перехода к 7d/пакету 8:** подпакет 7c реализован и передан на
независимую проверку; статус передачи ниже —
`READY_FOR_ARCHITECT_REVIEW` только для 7c. Подпакеты 7a и 7b — приняты и
интегрированы (§5.10). Подпакет 7d остаётся `planned` до своей собственной
реализации, в зафиксированном порядке 7b → 7c → 7d (§5.5c); пакет 8
остаётся заблокирован приёмкой и интеграцией всех подпакетов 7a–7d, не
только 7a–7c.

## 5.12. Первый корректирующий раунд подпакета 7c (исполнитель, 2026-09-21)

Владелец передал исполнителю один пункт правки на подпакет 7c, узко
ограниченный самим 7c: 7d и пакет 8 не начаты и не затронуты этой записью.
Работа снова выполнена в рабочем дереве без Git-записей — эта запись не
является приёмкой подпакета, и он не переводится в `accepted`/`ready` ею.

**Дефект.** `field_evaluation.rs`'s `parse_datetime_millis` (порт
`Date.parse` для уже провалидированных `start_ts`/`end_ts`) сопоставляло
дробные секунды регулярным выражением `(?:\.\d+)?` БЕЗ захвата группы —
совпавшие цифры считывались и тут же отбрасывались. Две метки времени,
различающиеся только долей секунды (например, `.100Z` и `.200Z`),
вычисляли ОДИНАКОВОЕ значение миллисекунд, так что реально более поздний
`end_ts` сравнивался как «не позже» `start_ts`, и подлинный,
schema-valid суб-секундный интервал отклонялся композитным алгоритмом —
ровно там, где Node-эталон (настоящий `Date.parse`) принимает его.

**Исправление.** `(?:\.\d+)?` заменено на захватывающую `(?:\.(\d+))?`;
новая функция `fractional_seconds_to_millis` читает ПЕРВЫЕ три цифры доли
(округление ВПРАВО нулями, если цифр меньше трёх; ОТСЕЧЕНИЕ, не
округление, если цифр больше трёх) — поведение сверено напрямую с
реальным `Date.parse` V8 на 1/2/3/4/6-значных долях
(`node -e 'console.log(Date.parse("...".1/.12/.123/.1234/.123456.../Z"))'`)
до написания фикса, не угадано заранее. Сдвиг индексов захватывающих
групп (offset-группы теперь 8/9/10, была 7/8/9) обновлён вместе с самим
регулярным выражением. Внешней зависимости на дату/время по-прежнему не
добавлено — `fractional_seconds_to_millis` работает над уже
провалидированной строкой, как и `days_from_civil` (§5.11).

**Rust-тесты.** `meridian-app/src/operating_model/field_evaluation.rs`,
модуль `tests`, четыре новых теста:
`parse_datetime_millis_reads_fractional_seconds_to_millisecond_precision`
(1/2/3/4/6-значные доли, включая явную проверку отсечения, не округления,
и разницу ровно в 100мс между `.100Z`/`.200Z`),
`duration_interval_differing_only_in_sub_second_fraction_is_accepted`
(`.100Z` → `.200Z` — обязан приниматься, требование задания пункта 2),
`duration_interval_with_equal_timestamps_is_rejected` (равные метки —
обязаны отклоняться) и
`duration_interval_with_offset_and_sub_second_difference_is_accepted`
(`+02:00`-смещение И доля секунды одновременно — offset-арифметика не
пострадала от сдвига индексов групп).

**Реальная Node/Rust conformance-проверка.**
`test/conformance-harness.test.mjs` получил новый блок (после
`VALIDATE_MUTATION_FAMILIES_7C`, перед корректирующим блоком 7b §5.9
пункта 2): `VALIDATE_MUTATION_FAMILIES_7C_EXTRA_WRITE_fieldEvaluationSubSecondInterval`
клонирует уже существующую РЕАЛЬНУЮ invalid-fixture «невозможный временной
интервал (конец раньше начала)» (переиспользуя её же `execution_run_ref`/
evidence-идентичность, уже разрешимую через `resolution`-карту бандла),
меняет `start_ts`/`end_ts` на `.100Z`/`.200Z` и добавляет результат как
НОВУЮ valid-fixture в копию дерева; проверка
(`cli-validate-mutation-7c-field-evaluation-subsecond-interval`) запускает
РЕАЛЬНЫЙ `scripts/kernel-validate.mjs` и РЕАЛЬНЫЙ скомпилированный
`meridian` над этой копией и утверждает `status === 'conformant'` — обе
стороны принимают интервал одинаково. Это проверка формы, ПРОТИВОПОЛОЖНОЙ
`VALIDATE_MUTATION_FAMILIES_*` (которые мутируют валидную fixture во
ЧТО-ТО, что обязано быть отклонено): здесь добавляется подлинно валидная
fixture, которую фикс обязан ПРИНЯТЬ.

Эмпирически подтверждено, что проверка действительно ловит именно этот
дефект, а не совпадение: `frac_millis` временно принудительно занулён
(с восстановленным регулярным выражением, без затрагивания индексов
групп) в рабочем дереве, `meridian`-бинарник пересобран
(`cargo build -p meridian-cli --bin meridian`), автономный прогон той же
логики, что и новая проверка, дал `status: "divergent"` с ОЖИДАЕМОЙ
причиной — Node принимает, Rust отклоняет с сообщением `measurement
end_ts is not after start_ts`; временное занижение убрано, бинарник
пересобран заново, тот же прогон дал `status: "conformant"`. По ходу
этой же эмпирической проверки обнаружена и исправлена ОТДЕЛЬНАЯ,
не относящаяся к самому фиксу ошибка методики: `validate_cli_producer`
(пример) спавнит РЕАЛЬНЫЙ бинарник `meridian` (`meridian-cli/examples/
validate_cli_producer.rs`) как отдельный подпроцесс — пересборка одного
только `--example validate_cli_producer` не пересобирает сам `target/
debug/meridian`, и первый прогон автономной проверки ложно показал
`divergent` из-за УСТАРЕВШЕГО бинарника, а не из-за исходного кода;
`cargo build -p meridian-cli --bin meridian` (или полный `cargo build
--workspace`) обязателен перед любым таким прогоном — сама официальная
`test/conformance-harness.test.mjs` уже собирает `--bins --examples`
вместе («реальные Rust-производители собраны перед сравнением», строка
268), так что штатный прогон полного файла этой ошибке не подвержен;
отмечено здесь как методическая заметка для будущих раундов, не как
дефект composite-алгоритма.

**Проверки, выполненные исполнителем (2026-09-21):** `cargo fmt --all --
--check` — чисто; `cargo build --workspace --all-targets --all-features
--locked` — чисто; `cargo test --workspace --all-targets --all-features
--locked` — **576 passed, 0 failed** (было 572 к концу основной передачи
7c/§5.11; +4 — четыре новых теста в
`meridian-app::operating_model::field_evaluation`, перечисленные выше);
`cargo clippy --workspace --all-targets --all-features --locked -- -D
warnings` — чисто; `RUSTDOCFLAGS="-D warnings" cargo doc --workspace
--no-deps --locked` — чисто; `node --test test/conformance-harness.test.mjs`
— **80 passed, 0 failed** (было 79 к концу §5.11; +1 — новая проверка
суб-секундного интервала); `node test/kernel-validate.test.mjs` — 293
passed, 0 failed, 0 skipped (не изменился этим раундом — ни один
Node-файл-эталон не тронут); полный `node scripts/preflight.mjs` —
самодостаточен; `git diff --check` — без ошибок на отслеживаемых
изменённых файлах; `git diff --no-index --check /dev/null <файл>` на
каждый из восьми неотслеживаемых файлов подпакета 7c — чисто на всех
восьми (индекс не трогался); изолированная `rsync`-копия рабочего дерева
вне этого репозитория, `git init` и `git add -A && git diff --cached
--check` ТАМ — сообщает те же две пред-существующие строки о завершающих
пробелах в `standards/templates/readme-template.md` (§5.7, §5.9, §5.11),
не регрессия этого раунда. Ветка `dev` и `HEAD` не изменялись; рабочее
дерево содержит только незакоммиченные изменения — никаких
`branch`/`switch`, `add`, `commit`, `merge`, `rebase`, `reset`, `stash`,
`tag` или `push` не выполнялось.

**Точный список путей, изменённых этим раундом:**

*Изменены:* `meridian-app/src/operating_model/field_evaluation.rs`
(`DATETIME_RE` теперь захватывает дробные секунды, новая
`fractional_seconds_to_millis`, `parse_datetime_millis` использует её и
сдвинутые индексы offset-групп, четыре новых теста),
`test/conformance-harness.test.mjs` (новый блок суб-секундной проверки),
`governance/plans/meridian-rust-migration-program-plan.md` (эта запись).

*Не изменены этим раундом:* ничего из 7a, 7b, 7d или пакета 8; ни один из
остальных трёх файлов 7c (`evidence_and_handoff.rs`,
`controlled_rule_intake.rs`, `existing_project_compatibility_mode.rs`) не
тронут; `VALIDATE_MUTATION_FAMILIES_7C` (четыре исходные мутации §5.11) не
изменены.

**Условие перехода к 7d/пакету 8:** без изменений — подпакет 7c
реализован и передан на независимую проверку; статус передачи —
`READY_FOR_ARCHITECT_REVIEW` только для 7c.

## 5.13. Решение владельца и аудит архитектуры всей Rust-кодовой базы (2026-09-21)

Владелец установил два правила наивысшего приоритета: соблюдать выбранную
архитектуру Rust; если Rust позволяет реализовать функциональность лучше или
надёжнее, обязательно использовать это преимущество. Целью остаётся сохранение
бизнес-ценности, а не технического устройства или дефектов Node.js. Нормативная
формулировка находится в `standards/workspace/rust-migration-quality.md`.

Проведён полный структурный аудит всех четырёх Rust-крейтов. Его доказательства
и границы зафиксированы в
`governance/audits/meridian-rust-codebase-architecture-audit.md`. Архитектура
workspace, направление зависимостей, изоляция SQLite и запрет `unsafe`
соответствуют целевой модели. Выпуск блокируют следующие нарушения:

1. модули `meridian-app/src/operating_model/` смешивают транспортные
   `serde_json::Value`, синтаксическую и предметную проверку, вычисление
   решения и форматирование строк;
2. значимая предметная логика `validate` размещена в `meridian-cli`, хотя CLI
   должен быть адаптером композиции и представления;
3. предусмотренные архитектурой порты `SourceResolver`, `GitInspector`,
   `WorkspaceReader` и `Clock` отсутствуют как рабочие границы;
4. диагностика широко представлена `Vec<String>` вместо типизированной
   модели;
5. успешный conformance-прогон доказывает наблюдаемое совпадение выбранного
   контракта, но не доказывает архитектурное качество.

Поэтому прежняя передача 7c `READY_FOR_ARCHITECT_REVIEW` отозвана; 7c нельзя
интегрировать в текущем виде. Исторические приёмки 7a и 7b не переписываются,
но их release-readiness отозвана до повторной проверки после корректирующего
пакета. 7d и пакеты 8–10 не начинаются.

Следующий обязательный пакет — `rust-architecture-conformance`. Он начинается
с пилота `controlled-rule-intake`, затем переводит
`existing-project-compatibility-mode` на ту же типизированную модель,
добавляет недостающие порты, типизированную диагностику и выносит предметные
правила из CLI. Пакет принимается только после архитектурных ворот §6.1 и
проверки бизнес-контракта §6.2; буквальное совпадение с Node.js не является
критерием, если расхождение представляет принятое и протестированное
Rust-native улучшение.

## 5.14. Пакет `rust-architecture-conformance-1`: пилот `controlled-rule-intake` (исполнитель, 2026-09-21)

Первый пакет `rust-architecture-conformance`, названный §5.13. Пилот —
`controlled-rule-intake` (рекомендация аудита, §5.13). Работа прошла три
раунда в рабочем дереве без Git-записей; эта запись консолидирует все три,
а не заводит по отдельному разделу на каждый, ради размера документа.

**Раунд 1 (первая передача).** Полностью переписан
`meridian-app/src/operating_model/controlled_rule_intake.rs` (плоский файл,
1030 строк, Value-centric) в модуль-директорию
`meridian-app/src/operating_model/controlled_rule_intake/{mod,domain,source_resolution,checks}.rs`:
закрытая граница JSON Schema как транспорт/синтаксис, доменное построение
из уже провалидированной `Value` в строгие типы (`PinnedSourceRef`,
`Boundary`, `Applicability`, `RuleCandidate` и др.), чистые предикаты,
типизированная `Diagnostic` вместо `Vec<String>`. Намеренное отличие:
полное тождество `Scope` в сравнении кластеров вместо `type::id`-ключа
Node-эталона.

**Раунд 2 (корректирующий).** Архитектор потребовал: закрытые transport
DTO с `deny_unknown_fields`; построение доменных объектов только после
успеха schema-проверки; три раздельных состояния (DTO / промежуточная
валидация / валидный тип); перенос предметных типов и чистых алгоритмов в
`meridian-core` (новый крейт-модуль `meridian-core/src/controlled_rule_intake/{types,resolved,checks}.rs`,
`meridian-core` не получил новых зависимостей); `RuleCandidate::try_new` —
единственный конструктор, сам проверяющий согласованность
origin/source/authority/decision; переиспользование `IsoDate`
(`meridian_core::migration::OwnerDecision`, уже несущий `decided_at: IsoDate`,
вместо второй копии); `SourceResolver` как типизированный app-порт с
конкретным адаптером в `meridian-cli`; полное устранение fail-fast —
независимые дефекты одного кандидата накапливаются до решения о валидном
типе; фиксация намеренного расхождения по `Scope` в `COMPATIBILITY.md` и
реальный Node/Rust тест на него; structural-тесты (closed DTO,
core без Value/serde, недостижимость невалидного домен-объекта, резолвер
вызывается один раз, resolver-failure отличается от not-found, CLI —
только presentation/I/O).

**Раунд 3 (второй корректирующий, доводит раунд 2 до конца).**
Архитектор указал на остаточный Value-centric узел: временный
`DiscoveredSourceResolver` в `meridian-app/src/operating_model/existing_project_compatibility_mode.rs`
оборачивал типизированный `PinnedSourceRef` в синтетический
`serde_json::json!({"id": ...})` только чтобы вызвать Value-based
`build_source_resolver`. Исправление — структурное, не косметическое:
`SourceResolver` в `meridian-app` перестал быть трейтом и стал типом-алиасом
замыкания (`dyn Fn(&PinnedSourceRef) -> Result<ResolvedInstructionSourceDto, SourceResolverError>`),
так что в `meridian-app` не осталось ни одного `impl SourceResolver for ...`
структурно — реализовывать нечего, подходит любое замыкание; `build_source_resolver`
теперь принимает id напрямую (`&str`), а не синтетический query-объект.
Отдельно: `read_channel` при валидации разрешённого ответа больше не
собирается обратно в `Value` для вызова Value-based `check_read_channel` —
выделен типизированный `meridian_core::instruction_source::{ReadChannel, ReadChannelKind, MeridianVisibility}`
и общая чистая проверка `check_read_channel_coherence`, вызываемая
напрямую на типе. `instruction_source_registry.rs`'s собственная
Value-based `check_read_channel` сознательно не тронута (вне области этого
пилота; риск регресса подпакета 7b, не мандат этого раунда) — новый
типизированный модуль зафиксирован как канонический вариант для её
будущей собственной конверсии, задокументировано в doc-комментарии
`meridian-core/src/instruction_source.rs`. Также добавлены: честный
structural-тест всех 13 object DTO (по одному явному случаю на каждый, не
4 из 13, как в раунде 2); проверка утверждения «Rust останавливается на
ошибке схемы, Node — нет» реальным исполняемым путём (раньше — только
doc-комментарий без проверки). Проверка показала: утверждение верно на
уровне библиотеки (Node вычисляет составной анализ даже после ошибки
схемы, Rust — нет), но НЕ наблюдаемо через единственный сегодняшний
исполняемый путь controlled-rule-intake — и `scripts/kernel-validate.mjs`
(строка ~1952), и Rust CLI-обёртка сообщают только ПЕРВУЮ диагностику
отклонённой fixture, а диагностика схемы всегда идёт первой на обеих
сторонах, так что полный wrapped-вывод на практике совпадает (conformant).
Первоначальная формулировка doc-комментария была избыточно широкой
(заявляла наблюдаемое расхождение) и исправлена на узкую, проверенную —
это и есть исполнение пункта 3 («документировать и протестировать
расхождение либо исправить ошибочное утверждение»): утверждение оказалось
частично ошибочным на уровне наблюдаемости и исправлено, а не подтверждено
задним числом.

**Итоговая архитектура.** `meridian-core::controlled_rule_intake` —
домен и чистые проверки (0 строк с `Value`/serde, доказано
structural-тестом `crate_manifest_declares_no_serde_or_json_dependency`);
`meridian-core::instruction_source` — разделяемый типизированный
`ReadChannel`; `meridian-app::operating_model::controlled_rule_intake` —
закрытые DTO, DTO→домен конвертация, оркестрация,
`SourceResolver`-как-тип-функции; `meridian-cli::commands::validate::controlled_rule_intake` —
только I/O и presentation, конкретный fixtures-резолвер.

**Реальная Node/Rust conformance.** `test/conformance-harness.test.mjs`
несёт для этого пилота: исходную мутацию `controlled-rule-intake` в
составе `VALIDATE_MUTATION_FAMILIES_7C` (не изменена); намеренную границу
«cluster scope divergence» (три проверки, Node принимает/Rust отклоняет
расхождение по `workspace_id` — подлинное CLI-наблюдаемое расхождение);
проверку «schema short-circuit» (две проверки, полный wrapped-вывод
совпадает на практике — библиотечное отличие подтверждено отдельно
Rust-юнит-тестом, не этой проверкой).

**Проверки, выполненные исполнителем после второго корректирующего
раунда.** `cargo fmt --check` — чисто; `cargo build --workspace` — чисто;
`cargo clippy --workspace --all-targets --all-features -- -D warnings` —
чисто; `cargo doc --workspace --no-deps` — чисто, 0 предупреждений;
`cargo test --workspace` — **599 passed, 0 failed** по всему workspace;
`node test/conformance-harness.test.mjs` — полный прогон, **85 passed, 0
failed**, включая обе намеренные границы. Git: `HEAD` не сдвигался, индекс
пуст, `add/commit/merge/push` не выполнялись.

**Раунд 4 (третий корректирующий).** Архитектор принял два пункта раунда 3
(честная проверка всех 13 DTO; устранение DTO→Value round-trip для
`read_channel`), но не принял пакет в целом и потребовал: восстановить
`SourceResolver` как app-owned trait в `meridian-app` (вместо type-alias
замыкания раунда 3); оставить производственные реализации порта только в
`meridian-cli`; убрать resolver-замыкание из production-кода `meridian-app`
целиком — `existing_project_compatibility_mode.rs`'s внутренняя композиция
со сканом теперь строится через типизированные данные
(`SourceResolution::Prefetched(&HashMap<String, ResolvedInstructionSourceDto>)`,
построенные ОДИН раз новой `build_resolved_sources`), не через скрытый
callback-адаптер; сделать `ReadChannel` невалидным непредставимым —
`ReadChannel::new` (инфаллибельный) удалён, единственный конструктор —
`ReadChannel::try_new`, сам проверяющий согласованность
`kind`/`meridian_visibility`/`agent_auto_read` (тот же паттерн, что
`RuleCandidate::try_new`); перевести
`instruction_source_registry::check_read_channel` на делегирование
единственной типизированной реализации
(`meridian_core::instruction_source::ReadChannel::try_new`) — этот модуль
впервые за весь пилот получил правку (одна функция, текст сообщений
побайтово сохранён, один юнит-тест дополнен обязательным по схеме полем
`meridian_visibility`, которого ему не хватало); добавить ПРЯМОЙ
библиотечный (не через self-test wrapper) Node/Rust тест на
schema-short-circuit — `test/conformance-harness.test.mjs` теперь
импортирует `evaluateControlledRuleIntake`/`makeRecordResolver` напрямую из
`scripts/lib/*.mjs`, паре с существующим Rust-юнит-тестом; исправить
итоговый статус программы (§10: `current_package_status` был устаревшим
`required_audit_complete_implementation_not_started`, исправлен на
`pilot_controlled_rule_intake_implemented_pending_architect_acceptance`).
Новый `EvalOpts.resolve_source: SourceResolution<'a>` (`None`/`Port(&dyn
SourceResolver)`/`Prefetched(&HashMap<...>)`) заменил
`Option<&SourceResolver<'a>>`.

**Проверки, выполненные исполнителем (2026-09-22, после третьего
корректирующего раунда).** `cargo fmt --check` — чисто; `cargo build
--workspace` — чисто; `cargo clippy --workspace --all-targets
--all-features -- -D warnings` — чисто; `cargo doc --workspace --no-deps`
— чисто, 0 предупреждений; `cargo test --workspace` — **600 passed, 0 failed** по всему
workspace (включая один исправленный тест `instruction_source_registry`,
дополненный обязательным полем схемы, не ослабленный); `node
test/conformance-harness.test.mjs` — **86 passed, 0 failed**, включая
прямой библиотечный Node/Rust тест item 6. Git: `HEAD` не сдвигался,
индекс пуст, `add/commit/merge/push` не выполнялись.

**Условие перехода (после третьего корректирующего раунда).** Только для
`rust-architecture-conformance-1`: статус передачи —
`READY_FOR_ARCHITECT_REVIEW`. `existing-project-compatibility-mode`
(следующий шаг корректирующей программы аудита, §5.13/§6 аудита) остаётся
НЕ начатым как самостоятельная конверсия: правки этого раунда в этом файле
— `build_resolved_sources` (типизированная композиция для ОДНОГО call
site) — механическая композиционная работа, не переход всего модуля на
типизированные предметные правила. `instruction_source_registry.rs`
получил ОДНУ целевую правку (делегирование `check_read_channel`), не
общую конверсию — остальные его функции остаются Value-based. Остальные
семейства `operating_model` (`evidence_and_handoff`, `field_evaluation`,
`role_and_human_control`, `bounded_context_manifest`) не затронуты вовсе.

**Раунд 5 (четвёртый корректирующий, `CHANGES_REQUESTED`).** Архитектор
принял два пункта раунда 3 (честная проверка 13 DTO; устранение DTO→Value
round-trip для `read_channel`), но не пакет в целом, и потребовал
устранить четыре оставшихся дефекта:

1. **Семантика `Prefetched`-резолюции исправлена.** Таблица
   (`existing_project_compatibility_mode::build_resolved_sources`) и lookup
   (`SourceResolution::Prefetched`) теперь ключуются по ID источника, а не
   по `reference` — тем же полем, по которому резолвит Node
   `buildSourceResolver` и Rust-`build_source_resolver`. Правильный id с
   неправильным pinned `reference` теперь резолвится и получает точную
   reference-mismatch диагностику из
   `meridian_core::controlled_rule_intake::checks::check_source_ref`
   (сравнение id и сравнение reference остались там, раздельно), а не
   `NotFound`. Не объявлено новым intentional difference — это исправление
   ошибочно изменённого контракта, не улучшение.
2. **Malformed prefetched source больше не теряется.** Таблица —
   `HashMap<String, Result<ResolvedInstructionSourceDto, SourceResolverError>>`;
   каждый id из `discovered_by_id` получает запись — `Ok` при успешной
   конверсии, `Err(Failed(..))` при её отсутствии (и при неполном
   payload/recorded_state, которое раньше тоже терялось через `raw(id) ->
   None`). Три состояния — отсутствие, malformed, успех — различимы вплоть
   до текста диагностики; `.ok()`/`unwrap_or_default` удалены.
3. **Library-level Node/Rust proof стал настоящей парой.** Оба теста
   грузят ОДИН И ТОТ ЖЕ `registries/operating-model/fixtures/controlled-rule-intake.fixtures.json`'s
   `valid[0].registry` и применяют ОДИНАКОВЫЕ две мутации (удаление
   `classification_basis`, порча `origin.source_ref`): Rust —
   `meridian-app/.../mod.rs::tests::schema_and_composite_defect_together_produce_exactly_one_schema_diagnostic_matching_the_node_reference`
   (новый тест; утверждает ровно 1 диагностику и явное отсутствие
   origin/source_ref-диагностики), Node — обновлённый блок в
   `test/conformance-harness.test.mjs` (утверждает ≥2, включая обе).
   Старый Rust-тест (`a_schema_invalid_document_stops_before_any_domain_construction`,
   синтетический вход) сохранён отдельно как дешёвая проверка общего
   механизма, но его doc-комментарий больше не заявляет, что он — половина
   парного доказательства.
4. **Публичный API `ReadChannel` закрыт.** Бывшая `pub fn
   check_read_channel_coherence` стала приватной `coherence_problems` —
   единственный вызывающий — сам `ReadChannel::try_new`; внешний
   вызывающий, получивший `ReadChannel` любым публичным путём, уже прошёл
   правило, так что публичная отдельная проверка была ложной поверхностью.
   Тест, ранее конструировавший невалидный `ReadChannel` напрямую через
   приватные поля, переписан на проверку отклонения через `try_new`.
   `instruction_source_registry::check_read_channel` продолжает
   делегировать — теперь `ReadChannel::try_new` (был и остаётся —
   `check_read_channel_coherence` этим модулем никогда не вызывалась
   напрямую).

Новые/изменённые тесты (item 4 задания): id-first resolution +
reference-mismatch; NotFound на отсутствующем id; failure-диагностика на
malformed source, не NotFound; детерминизм построения/lookup таблицы;
парные Node/Rust library-level тесты на одном fixture; несогласованный
`ReadChannel` недостижим публично; оба read-channel entrypoint используют
одну core-проверку; прежние 13 DTO-тестов и остальные доказательства
остаются зелёными.

**Проверки, выполненные исполнителем (2026-09-22, после четвёртого
корректирующего раунда).** `cargo fmt --all -- --check` — чисто; `cargo
build --workspace --all-targets --all-features` — чисто; `cargo clippy
--workspace --all-targets --all-features -- -D warnings` — чисто;
`RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` — чисто, 0
предупреждений; `cargo test --workspace --all-targets --all-features` —
**605 passed, 0 failed** по всему workspace; `node --test
test/conformance-harness.test.mjs` — **86 passed, 0 failed**; `node
test/kernel-validate.test.mjs` — **293 passed, 0 failed, 0 skipped** (без
регрессии — ни один Node-файл-эталон не тронут этим раундом, то же число,
что и в предыдущих раундах); `node
scripts/preflight.mjs` — самодостаточен; `git diff --check` — чисто на
отслеживаемых файлах; `git diff --no-index --check /dev/null <файл>` на
каждом untracked `.rs`-файле этого раунда — чисто на всех. Git: `HEAD` не
сдвигался, индекс пуст, `branch/switch/add/commit/merge/rebase/reset/stash/tag/push`
не выполнялись; чужие незакоммиченные изменения не тронуты.

**Раунд 6 (пятый корректирующий, `CHANGES_REQUESTED`).** Архитектор не принял
пакет в целом и потребовал устранить два оставшихся дефекта:

1. **У теста schema short-circuit появились «зубы».** Прежняя мутация
   (удаление `payload.classification_basis`) не доказывала работу именно
   schema-заслона: это поле одновременно обязательно по схеме И невалидируемо
   как non-optional `String` в закрытом `dto::PayloadDto`, так что парсинг
   транспортного DTO сам по себе уже давал одну диагностику, даже при
   полностью отключённом schema short-circuit — тест ничего конкретно не
   проверял. Мутация заменена на `registry_id`, изменённый на неверную
   непустую строку: схема (`registry_id: {const:
   "controlled-rule-intake"}`) это отклоняет, а `dto::RegistryDto.registry_id`
   — простой невалидируемый `String`, принимающий что угодно — эту мутацию
   может поймать ТОЛЬКО schema-заслон. Node и Rust используют один и тот же
   fixture и одинаковые обе мутации (`registry_id` + порча
   `origin.source_ref`). Rust-тест
   (`meridian-app/.../mod.rs::tests::schema_and_composite_defect_together_produce_exactly_one_schema_diagnostic_matching_the_node_reference`)
   утверждает: диагностика по `registry_id`/`const` присутствует, диагностик
   ровно 1, диагностики origin/source_ref нет. Node-тест
   (`test/conformance-harness.test.mjs`) утверждает: обе диагностики
   присутствуют, их ≥2. **Эмпирически подтверждено**: schema short-circuit во
   `evaluate_controlled_rule_intake` был временно отключён (накопление вместо
   `return` на строке проверки схемы), тест ПАДАЛ ровно из-за появления
   составной диагностики поверх схемной — затем производственный код в точности
   восстановлен, тест снова зелёный. `COMPATIBILITY.md` и doc-комментарии
   обновлены, ссылка на старое имя теста и старую мутацию убрана.
2. **Проверка настоящего `build_resolved_sources`, не ручной проекции.**
   Новый тест
   `evaluate_controlled_rule_intake_reports_resolution_failed_for_a_malformed_prefetched_source_built_by_production_code`
   строит реальный `HashMap<String, Value>` с malformed-источником
   (`payload` без `recorded_state`), вызывает настоящий
   `existing_project_compatibility_mode::build_resolved_sources`, утверждает,
   что результат содержит `Err(SourceResolverError::Failed(..))` для этого id
   (`Err` нигде не вставлен вручную), передаёт эту таблицу в
   `SourceResolution::Prefetched` и подтверждает, что
   `evaluate_controlled_rule_intake` сообщает «resolution failed», а не
   `NotFound`. Тест детерминизма
   (`build_resolved_sources_and_its_lookup_are_deterministic`) переписан:
   вызывает `build_resolved_sources` ДВАЖДЫ на одном `discovered_by_id`,
   прогоняет обе полученные таблицы через настоящий
   `evaluate_controlled_rule_intake` и сравнивает полные typed diagnostics —
   проверяется builder и lookup вместе, не повторный lookup вручную
   построенной таблицы. Прежний
   `prefetched_resolution_of_a_malformed_entry_is_a_failure_not_a_not_found`
   (ручная таблица) сохранён отдельно как decoupled orchestration-level
   тест — doc-комментарии обоих тестов перекрёстно ссылаются друг на друга.

Самопроверка перед прогоном (пункт 3 задания) подтверждена: `SourceResolver`
остаётся трейтом; единственная production-реализация — в `meridian-cli`;
`Prefetched` резолвит по id источника; malformed-вход не сливается с
отсутствием; парный Node/Rust-тест использует один fixture и одинаковые
мутации; Rust-тест short-circuit не может пройти из-за одного лишь провала
DTO-парсинга (эмпирически проверено); `ReadChannel` не строится невалидным
через публичный API; комментарии и запись передачи называют реально
использованные тесты и мутации.

При прогоне полного `test/conformance-harness.test.mjs` исполнитель нашёл и
исправил собственную ошибку в новом Node-блоке этого раунда: `assert` в этом
файле — локальная truthy-функция (`function assert(condition, message)`), а
не модуль `node:assert`, так что `assert.strictEqual(...)` падал с
`TypeError: assert.strictEqual is not a function`. Исправлено на
`assert(registry.registry_id === 'controlled-rule-intake', ...)` в стиле
остального файла; полный набор перепрогнан и подтверждён зелёным.

**Проверки, выполненные исполнителем (2026-09-22, после пятого
корректирующего раунда).** `cargo fmt --all -- --check` — чисто (после
однократного `cargo fmt --all`, применившего форматирование к новым тестам
этого раунда); `cargo build --workspace --all-targets --all-features` —
чисто; `cargo clippy --workspace --all-targets --all-features -- -D
warnings` — чисто; `RUSTDOCFLAGS="-D warnings" cargo doc --workspace
--no-deps` — чисто, 0 предупреждений; `cargo test --workspace --all-targets
--all-features` — **606 passed, 0 failed** по всему workspace (605 из
четвёртого корректирующего раунда + 2 новых теста этого раунда − 1 удалённый:
`prefetched_table_lookup_is_deterministic` заменён на
`build_resolved_sources_and_its_lookup_are_deterministic`); `node --test
test/conformance-harness.test.mjs`
— **86 passed, 0 failed** (после исправления `assert.strictEqual`-бага,
описанного выше; то же число проверок, что и в раунде 5 — обновлённый
матчед-пара тест заменяет прежний, новых блоков не добавлено); `node
test/kernel-validate.test.mjs` — **293 passed, 0 failed, 0 skipped** (без
регрессии); `node scripts/preflight.mjs` — самодостаточен; `git diff
--check` — чисто на отслеживаемых файлах; `git diff --no-index --check
/dev/null <файл>` на каждом untracked-файле репозитория (не только новых
файлах этого раунда) — чисто на всех. Git: `HEAD` не сдвигался (остаётся на
`07229f6`, ветка `dev`), индекс пуст,
`branch/switch/add/commit/merge/rebase/reset/stash/tag/push` не выполнялись;
незакоммиченные изменения других участников не тронуты; untracked-файл
`governance/plans/rust-architecture-conformance-1-fourth-corrective-round.md`
(не созданный исполнителем) не тронут.

**Раунд 7 (шестой корректирующий, промежуточный, `CHANGES_REQUESTED` на входе).**
Архитектор потребовал устранить два оставшихся дефекта только в пилоте
`controlled-rule-intake`, без начала последующих семейств и без
Git-операций записи:

1. **Порядок диагностик графа конфликтов сделан детерминированным.** В
   `meridian-core/src/controlled_rule_intake/checks.rs::check_conflict_graph`
   закрывающая соседей функция `conflicts_of` возвращала `HashSet<&SemanticKey>`
   и обходилась в этом порядке при формировании `Vec<Diagnostic>` — при
   нескольких одновременных конфликтах одного кластера порядок результата
   зависел от случайного hash seed процесса. Заменена на
   `BTreeSet<&SemanticKey>` (лексикографический порядок по `SemanticKey`,
   который уже реализует `Ord`) — единственное место во всём новом коде
   `controlled_rule_intake`, где обход неупорядоченной коллекции влиял на
   наблюдаемый результат (остальные `HashSet`/`HashMap` пилота — в
   `check_cluster`, в `mod.rs`'s `seen_ids` — используются исключительно для
   membership/подсчёта, не для порядка, и оставлены без изменений).
   Doc-комментарий `check_conflict_graph` переписан: теперь он точно
   описывает не только канонизацию имён внутри пары, но и детерминизм
   полного списка диагностик (внешний обход — `BTreeMap`, внутренний —
   теперь тоже `BTreeSet`). Новый core unit-тест
   `check_conflict_graph_reports_two_neighbour_diagnostics_in_a_fixed_order`
   строит один кластер (`"a-key"`), объявляющий `conflicts_with` на ДВУХ
   разных соседей одновременно (`"b-key"` — существующий кластер,
   объявляющий конфликт только в одну сторону; `"d-key"` — не
   `semantic_key` ни одного кандидата вовсе), сравнивает полный
   `Vec<Diagnostic>` с точным ожидаемым порядком через настоящий
   `check_conflict_graph` (не через вспомогательную проекцию), затем
   вызывает его повторно на том же входе и подтверждает побайтово
   одинаковый результат. **Эмпирически подтверждены «зубы»**: обход
   временно возвращён к `HashSet`, тест в восьми повторных свежих
   процессах падал в четырёх (порядок недетерминирован, ровно как
   ожидалось от случайного hash seed), после чего исходный файл
   восстановлен байт-в-байт (`diff` подтвердил идентичность) — тест снова
   зелёный во всех повторных прогонах.
2. **Конструирование `ResolvedInstructionSource` закрыто.**
   `meridian-core/src/controlled_rule_intake/resolved.rs`: поля
   `ResolvedInstructionSource` (`id`, `reference`, `recorded_state`) и
   `RecordedState` (`revision`, `digest`, `revision_verified`, `currency`)
   стали приватными; для обоих типов единственный публичный конструктор —
   `new(..)` (инфаллибельный: каждая комбинация уже валидных строгих
   компонентов сама по себе является допустимым предметным состоянием —
   зафиксировано в doc-комментарии для обоих типов, кросс-полевого
   инварианта нет, фиктивная валидация не добавлена); минимальные
   read-only getters (`id()`, `reference()`, `recorded_state()`,
   `revision()`, `digest()`, `revision_verified()`, `currency()`)
   предоставлены для `check_source_ref`. Doc-комментарий
   `ResolvedInstructionSource` переписан: прежняя формулировка заявляла,
   что значение «could only be built from one that passed validation»
   (провалидированного ответа резолвера), хотя публичные поля это
   опровергали; новая формулировка честно ограничивает гарантию
   типа (каждое поле — валидный строгий core-тип) и явно называет
   провенанс «прошло через `validate_resolved_response`» ответственностью
   вызывающего слоя `meridian-app`, а не инвариантом самого типа.
   Единственный производственный вызывающий —
   `meridian-app/src/operating_model/controlled_rule_intake/source_resolver.rs::validate_resolved_response` —
   переписан на `ResolvedInstructionSource::new(..)`/`RecordedState::new(..)`;
   `meridian-core/src/controlled_rule_intake/checks.rs::check_source_ref`
   переписан на getters без ослабления ни одной из семи проверок.
   Structural/API-тесты в `resolved.rs`: `new_and_its_getters_round_trip_every_field`
   (поведенческий — каждое поле проходит через `new` и обратно через
   getter, включая `check_source_ref`'s собственный путь чтения);
   `meridian_app_never_constructs_these_types_with_a_struct_literal`
   (структурный, в стиле уже существующего
   `crate_manifest_declares_no_serde_or_json_dependency` — обходит ВЕСЬ
   `meridian-app/src` рекурсивно через `env!("CARGO_MANIFEST_DIR")`,
   проверяет по имени отсутствие `ResolvedInstructionSource {`/
   `RecordedState {` в каждом `.rs`-файле, с защитным порогом
   количества просмотренных файлов). `RecordedState` проверен отдельно:
   у него тоже нет кросс-полевого инварианта, который поля позволяли бы
   обойти — это зафиксировано явно в doc-комментарии, а не восполнено
   фиктивной проверкой.

Самопроверка перед прогоном (пункт 3 задания) подтверждена: ни в одном
output-producing пути пилота не осталось обхода `HashMap`/`HashSet`, влияющего
на порядок; `ResolvedInstructionSource` невозможно собрать struct-literal'ом
из другого крейта (закрытые поля, подтверждено компилятором и структурным
тестом); `meridian-core` не получил `serde`/`serde_json::Value`/I/O в
production-коде (`std::fs`/`std::path` использованы только внутри
`#[cfg(test)]`, тем же способом, что уже использует существующий
`crate_manifest_declares_no_serde_or_json_dependency`); `SourceResolver`
остаётся app-owned трейтом; единственная production-реализация порта
остаётся в `meridian-cli` (не тронута этим раундом); schema short-circuit,
id-first prefetched resolution, различение `NotFound`/`Failed` и закрытый
`ReadChannel` не затронуты и не ослаблены; intentional Rust/Node differences
остаются документированы и протестированы без изменений.

**Проверки, выполненные исполнителем (2026-09-22, этот промежуточный
раунд — только затронутая область, полный набор не запускался).**
`cargo fmt --all -- --check` — чисто (после одного `cargo fmt --all`,
применившего форматирование к новому коду этого раунда); `cargo test -p
meridian-core controlled_rule_intake` — **9 passed, 0 failed** (было 7 до
этого раунда: +1 тест детерминизма графа конфликтов, +2 теста
`resolved.rs`); `cargo test -p meridian-app controlled_rule_intake` —
**23 passed, 0 failed** (без регрессии; CLI-адаптер этим раундом не
тронут, `cargo test -p meridian-cli controlled_rule_intake` не запускался);
`cargo clippy -p meridian-core -p meridian-app --all-targets --all-features
-- -D warnings` — чисто; `git diff --check` — чисто. Не запускались в этом
промежуточном раунде (как и предписано заданием): полный `node --test
test/conformance-harness.test.mjs`; `node test/kernel-validate.test.mjs`;
`cargo test --workspace`; `cargo doc --workspace`; прочие длительные
интеграционные проверки — полный обязательный набор будет выполнен один раз
после архитектурного одобрения, перед итоговым `ACCEPTED`. Git: `HEAD` не
сдвигался, индекс пуст, `branch/switch/add/commit/merge/rebase/reset/stash/tag/push`
не выполнялись; чужие незакоммиченные изменения не тронуты.

**Условие перехода.** Только для `rust-architecture-conformance-1`:
статус передачи — `READY_FOR_ARCHITECT_REVIEW`. Не `ACCEPTED` — решение
принимает архитектор после независимой проверки.

## 5.15. Пакет `rust-architecture-conformance-2`: `instruction-source-registry` и `existing-project-compatibility-mode` (исполнитель, 2026-09-22)

Второй пакет `rust-architecture-conformance`, названный §5.13 (§6 аудита):
переводит `instruction-source-registry` (7b) и `existing-project-compatibility-mode`
(7c) на типизированную модель пилота `controlled-rule-intake`
(`rust-architecture-conformance-1`, §5.14), выполняясь одним пакетом, так как
второй контракт композиционно использует первый.

**Каноническая модель источника (`meridian-core::instruction_source`).**
Однофайловый `instruction_source.rs` (уже нёсший `ReadChannel` из пилота)
преобразован в модуль-директорию:
`meridian-core/src/instruction_source/{mod,read_channel,medium,source_format,currency,location,recorded_state,resolved_instruction_source,divergence,source}.rs`.
Новые закрытые типы: `Medium`, `SourceFormat`, `RelativePath`/`OpaqueRef`
(валидирующие конструкторы вместо `checkLocationPath`/`opaqueRefProblem`),
`Location` (перечисление `File`/`ExternalService`, невозможная комбинация
медиума и полей непредставима), `ObservedState`/`DivergenceStatus`/`Divergence`
(`Divergence::try_new` переносит правило `checkDivergenceClaim` в
конструктор — несогласованный статус непредставим), `InstructionSourcePayload`/
`InstructionSource` (`InstructionSource::try_new` переносит `checkStateCoherence`
— temporal-несогласованность `recorded_state`/`divergence` непредставима).
`Currency`, `RecordedState`, `ResolvedInstructionSource` **перенесены** сюда
из `controlled_rule_intake::resolved` (удалён), которая теперь
`pub use crate::instruction_source::{Currency, RecordedState,
ResolvedInstructionSource};` — существующие пути импорта пилота не
изменились, `checks.rs` обновлён на прямой импорт из `instruction_source`.
Структурный тест `each_moved_canonical_type_is_defined_exactly_once_in_the_whole_crate`
защищает от повторного появления второй копии любого из четырёх типов.

**`instruction-source-registry` (`meridian-app/src/operating_model/instruction_source_registry/{mod,dto,convert}.rs`).**
Тот же маршрут `transport DTO -> syntax -> domain -> pure checks`, что и у
пилота: `dto::EntryDto` и вложенные DTO — закрытые
`#[serde(deny_unknown_fields)]`; `convert::build_source` строит
`InstructionSource`, накапливая независимые дефекты, и завершается
`InstructionSource::try_new`. `evaluate_instruction_source_registry`
возвращает `Vec<Diagnostic>` (было `Vec<String>`). Новая
`convert_registry` — композиционная точка входа: возвращает не только
диагностики, но и `sources: BTreeMap<String, InstructionSource>` +
`malformed_ids: BTreeSet<String>` (присутствовал, но не собрался) —
`existing-project-compatibility-mode` вызывает ЕЁ ЖЕ для своих
`discovered_sources`, не копию границы.

**Композиция без `Value`-круговорота (`existing-project-compatibility-mode`).**
`meridian-app/src/operating_model/existing_project_compatibility_mode/{mod,dto,domain,convert}.rs`.
Типизированные понятия задания (`domain.rs`): `ConnectionMode`, `ScanKind`,
`DiscoveryStatus`, `FindingKind`, `NextStep`, `DiscoveryPlanSlot` (переиспользует
`meridian_core::instruction_source::Location` напрямую), `PreviousKnowledge`
(enum-вариант вместо пары `previously_known: bool` + `Option<id>` — невалидная
комбинация непредставима), `MissingSource`/`UnreadableSource`/`Finding`/
`RepositoryRef`/`ManagedModeDecision`, чистые `compute_next_step`,
`check_discovery_outcomes` (partition-инвариант), `check_changes_have_findings`,
`check_repository_scope`, `check_managed_mode` — все над уже типизированными
значениями, ни одна не видит `Value`. **Архитектурное решение исполнителя**,
подлежащее проверке архитектора: эти типы и проверки остаются в
`meridian-app`, а не переносятся в `meridian-core` — в отличие от
`instruction_source`, ни один из них не переиспользуется вторым контрактом;
композиционная обязанность пакета — реальное использование ДВУХ существующих
типизированных контрактов (`instruction_source_registry`,
`controlled_rule_intake`), которое выполнено полностью (ниже).

Реальная композиция: `discovered_sources` оборачиваются в контейнер и идут в
`instruction_source_registry::convert_registry` (та же функция, тот же
schema+DTO+domain путь, что и для отдельного реестра) — результат даёт
типизированные `InstructionSource` для сравнения `Location` с
`discovery_plan`-слотом (`==`, вместо ручного покомпонентного сравнения) и
чтения `divergence().status()`. `rule_candidates[].candidate` идут в
`controlled_rule_intake::evaluate_controlled_rule_intake` с
`SourceResolution::Prefetched`, построенным `build_resolved_sources` —
которая берёт уже типизированный `InstructionSource` и строит
`ResolvedInstructionSourceDto` **без единого `serde_json::Value`**, через
новый инструментальный конструктор `ResolvedInstructionSourceDto::from_typed`
(добавлен в пилот, `controlled_rule_intake/source_resolver.rs`, единственная
правка принятого пилота этим пакетом, аддитивная — не меняет ни одного
существующего поведения или теста) — раньше единственным способом собрать
этот закрытый транспортный тип извне модуля был `serde_json::from_value`, так
как его подтипы приватны; теперь есть прямой typed-конструктор. Три состояния
источника кандидата различимы до диагностики: отсутствует (не в
`discovered_ids` → `NotFound` через `Prefetched`), присутствует, но невалиден
(`malformed_ids` → `Err(Failed(..))`, тест
`a_malformed_discovered_source_is_distinguished_from_one_never_discovered_at_all`),
валиден, но не совпадает с pin (`check_source_ref`'s существующая
reference/revision/digest-диагностика, непереписанная), валиден и совпадает.
Два теста подтверждают, что композиция настоящая, а не заглушка: изменение
`recorded_state.currency` внутри `discovered_sources` всплывает как диагностика
с префиксом `discovered source:` (`a_malformed_embedded_discovered_source_is_reported_through_the_real_instruction_source_registry_path`);
изменение `payload.classification_basis` кандидата всплывает с префиксом
`rule candidate:` через реальный принятый `controlled_rule_intake`
(`a_malformed_embedded_rule_candidate_is_reported_through_the_real_controlled_rule_intake_path`).

**Порты и CLI.** `SourceResolver` не тронут — остаётся app-owned trait
пилота, единственная production-реализация в `meridian-cli`.
`meridian-cli/src/commands/validate/{instruction_source_registry,existing_project_compatibility_mode}.rs`
не получили новой предметной логики — только правка presentation
(`problems[0]` → `problems[0].message()` под новый `Vec<Diagnostic>`).

**Намеренное отличие от Node (задокументировано в `COMPATIBILITY.md`,
покрыто прямым библиотечным Node/Rust тестом на ОДНОМ И ТОМ ЖЕ реальном
fixture-документе с ОДИНАКОВЫМИ мутациями на обеих сторонах, как у пилота).**
В отличие от `controlled-rule-intake`, ни один из двух контрактов не
останавливается на нарушении схемы документа/записи — это сохранено как у
Node, так и у прежней Value-реализации. Единственное место, где типизированная
реализация вычисляет МЕНЬШЕ диагностик, чем Node: если запись не проходит сам
`serde`-разбор закрытого DTO (`deny_unknown_fields` на лишнем поле или
несовпадение типа поля — строже, чем текущее подмножество JSON Schema),
доменное построение для ЭТОЙ ОДНОЙ записи недостижимо; обычный доступ к полям
Node не имеет понятия закрытой формы и продолжает вычислять свои проверки
над остальными полями записи. Тесты: Rust —
`instruction_source_registry/mod.rs::tests::a_transport_parse_failure_skips_business_checks_for_that_entry_only`,
`existing_project_compatibility_mode/mod.rs::tests::a_transport_parse_failure_skips_business_checks_for_that_entry_only`;
Node — `test/conformance-harness.test.mjs`, два новых блока с прямым импортом
`evaluateInstructionSourceRegistry`/`evaluateExistingProjectCompatibilityMode`.

**Служебный файл `rust-architecture-conformance-1-fourth-corrective-round.md`**
не входит в этот пакет и не тронут (§10 задания).

**Проверки, выполненные исполнителем (2026-09-22, первая передача — только
затронутая область, как предписано заданием §11).** `cargo fmt --all --
--check` — чисто; `cargo test -p meridian-core instruction_source` — 34
passed; `cargo test -p meridian-app instruction_source_registry` — 12
passed; `cargo test -p meridian-app existing_project_compatibility_mode` —
21 passed (включает `cargo test -p meridian-app controlled_rule_intake` —
24 passed, 0 failed, без регрессии пилота — сигнатура
`build_resolved_sources` сменилась на типизированную, два теста пилота
обновлены под новую сигнатуру, поведение тестов не ослаблено); `cargo test
-p meridian-cli instruction_source_registry` — 2 passed (включая реальные
schema+fixtures); `cargo test -p meridian-cli
existing_project_compatibility_mode` — 1 passed (реальные schema+fixtures);
`cargo clippy -p meridian-core -p meridian-app -p meridian-cli
--all-targets --all-features -- -D warnings` — чисто; `node --check
test/conformance-harness.test.mjs` — чисто (два новых блока синтаксически
проверены, НЕ исполнены полным прогоном — предписано заданием §11, полный
`node --test` остаётся для проверки после архитектурного одобрения); `git
diff --check` — чисто. Полный `cargo test --workspace`, `cargo doc
--workspace`, `node test/kernel-validate.test.mjs` не запускались — за
пределами предписанной для этой передачи области. Git: `HEAD` не
сдвигался, индекс пуст,
`branch/switch/add/commit/merge/rebase/reset/stash/tag/push` не
выполнялись; чужие незакоммиченные изменения не тронуты.

**Условие перехода.** Только для `rust-architecture-conformance-2`: статус
передачи — `READY_FOR_ARCHITECT_REVIEW`. Не `ACCEPTED`. Задание указывало
`rust-architecture-conformance-1` уже полностью принятым архитектором на
момент начала этого пакета; этот файл на момент начала пакета 2 ещё нёс
`READY_FOR_ARCHITECT_REVIEW` для пакета 1 (§5.14) — исполнитель не правит
чужую запись приёмки задним числом и оставляет её как есть; архитектор/
владелец при следующей правке этого документа синхронизируют статус пакета
1, если фактическое решение действительно уже принято.

**Раунд 2 (первый корректирующий, `CHANGES_REQUESTED`).** Архитектор не
принял пакет и потребовал устранить шесть пунктов:

1. **Предметная модель `existing-project-compatibility-mode` перенесена в
   `meridian-core`.** Первая передача оставила `ConnectionMode`, `ScanKind`,
   `DiscoveryStatus`, `FindingKind`, `NextStep`, `DiscoveryPlanSlot`,
   `PreviousKnowledge`, `MissingSource`, `UnreadableSource`, `Finding`,
   `RepositoryRef`, `ManagedModeDecision`, `build_connection_scope` и чистые
   проверки в `meridian-app`, обосновав это отсутствием второго потребителя
   — архитектор отклонил это обоснование: критерий размещения в
   `meridian-core` — предметный алгоритм и тип с непредставимым невалидным
   состоянием, а не переиспользование вторым контрактом. Новый
   `meridian-core/src/existing_project_compatibility_mode/{mod,types,checks}.rs`
   несёт все перечисленные типы и чистые проверки без `serde`/`Value` и без
   файлового/Git/DB/сетевого/env/process/output I/O (проверено новым
   структурным тестом `no_rs_file_performs_filesystem_process_or_env_io_in_production_code`
   в `meridian-core/src/lib.rs`, сканирующим весь крейт). `meridian-app`'s
   `domain.rs` стал чистым re-export shim'ом (`pub use meridian_core::existing_project_compatibility_mode::{...}`)
   — прежние пути импорта `domain::X` в `convert.rs`/`mod.rs` не изменились.
2. **Outcome partition больше не сворачивает `discovered_sources` в `Set`
   до подсчёта.** `check_discovery_outcomes` (теперь в
   `meridian-core::existing_project_compatibility_mode::checks`) принимает
   типизированные ID-проекции: `plan_ids: &BTreeSet<SemanticId>` (деduplication
   здесь корректна — повторное объявление слота уже даёт отдельную
   диагностику) и `discovered`/`missing`/`unreadable: &[SemanticId]` —
   МУЛЬТИМНОЖЕСТВА, без дедупликации: повтор одного plan id внутри
   `discovered_sources` — два отдельных occurrence, не одно членство.
3. **Аудит независимых проверок после частичного domain-construction
   failure.** Найдено и исправлено две протечки: (а) `discovery_plan` слот с
   валидным id, но невалидным `location`, раньше пропадал из
   outcome-partition целиком (проверялся только через `plan:
   Vec<DiscoveryPlanSlot>`, которая содержит ТОЛЬКО полностью валидные
   слоты) — теперь `plan_ids` строится из валидного id НЕЗАВИСИМО от
   исхода построения location; (б) `missing_sources`/`unreadable_sources`
   запись с валидным `plan_id`, но невалидной остальной частью (например
   несогласованность `previously_known`/`id`), аналогично пропадала —
   `missing_occurrences`/`unreadable_occurrences` теперь собираются из
   валидного `plan_id` независимо от успеха `convert::build_missing_source`/
   `build_unreadable_source`. Также исправлена смежная протечка: проверка
   «discovered source не соответствует ни одному объявленному слоту»
   раньше использовала `plan_by_id` (только полностью валидные слоты) —
   теперь использует `declared_plan_id_strings` (все объявленные id,
   независимо от валидности их location), так что discovered source,
   ссылающийся на слот с невалидным location, больше не получает ложный
   «does not correspond» вместо реальной привязки к уже отдельно
   продиагностированному слоту.
4. **Типизированная промежуточная проекция вместо сырого `Value` в core.**
   `check_discovery_outcomes` в `meridian-core` никогда не видит `Value` —
   вызывающая сторона (`meridian-app`) строит `SemanticId`-проекции сама, до
   вызова, сохраняя валидный id и multiplicity даже когда остальные поля
   записи не построились.
5. **COMPATIBILITY.md/§5.15 пересмотрены.** Обе записи о transport-parse-failure
   отличии (добавленные первой передачей) остались точными — исправления
   этого раунда не затронули тот механизм. Но сам аудит пункта 3 вскрыл
   НОВОЕ наблюдаемое отличие как побочный эффект: Node's `sameLocation`
   (`scripts/lib/existing-project-compatibility-mode.mjs`) сравнивает сырые
   строковые поля слота и discovered source БЕЗУСЛОВНО, даже когда сам слот
   уже невалиден по `checkPlanLocation`; Rust, начиная с этого раунда,
   пропускает сравнение location целиком, если сам слот не построился в
   типизированный `Location` (типизированному "location" из уже невалидных
   данных сравнивать не с чем). Итоговый вердикт записи не меняется ни в
   одном найденном случае (собственный дефект слота уже проваливает запись
   на обеих сторонах) — меняется только состав диагностик. **Это НЕ
   объявлено принятым намеренным отличием** — исполнитель не вправе решать
   это сам (прямое указание задания). Пара тестов зафиксирована как
   `flagged_*`: Rust —
   `meridian-app/src/operating_model/existing_project_compatibility_mode/mod.rs::tests::flagged_a_discovered_source_naming_an_invalid_plan_slot_gets_no_location_mismatch_diagnostic_on_the_rust_side`;
   Node — `test/conformance-harness.test.mjs`, блок сразу после
   transport-parse-failure пары, с тем же реальным fixture и теми же двумя
   мутациями. Решение — за архитектором: либо принять как обоснованное
   Rust-native сужение (тот же прецедент, что уже принят для
   `controlled-rule-intake`'s schema short-circuit — вердикт не меняется,
   меняется состав диагностик), либо потребовать сравнение по сырым полям
   DTO в `meridian-app` (не в `meridian-core`) для точного паритета.
6. **Структурные тесты добавлены.** `meridian-core::lib.rs::tests::no_rs_file_performs_filesystem_process_or_env_io_in_production_code`;
   `meridian-app`'s `existing_project_compatibility_mode::tests::app_domain_module_defines_no_type_or_check_of_its_own`
   (грепом подтверждает `domain.rs` не содержит `pub struct`/`pub enum`/`pub fn`,
   только `pub use meridian_core::existing_project_compatibility_mode`);
   `orchestration_calls_the_real_core_checks_by_name` (грепом подтверждает
   `mod.rs` вызывает `domain::check_discovery_outcomes`/`check_changes_have_findings`/
   `check_repository_scope`/`check_managed_mode`/`compute_next_step`/
   `build_connection_scope` по имени). Duplicate-outcome и
   partial-construction случаи покрыты тремя новыми тестами, идущими через
   настоящий `evaluate_existing_project_compatibility_mode` (не юнит-тестом
   `check_discovery_outcomes` в изоляции):
   `two_discovered_sources_naming_the_same_plan_id_get_duplicate_and_outcome_count_diagnostics`,
   `a_discovery_plan_slot_with_a_valid_id_but_invalid_location_gets_both_defects`,
   `a_malformed_missing_source_with_a_valid_plan_id_still_counts_as_its_slots_outcome`.

**Проверки, выполненные исполнителем (2026-09-22, второй корректирующий
раунд — только назначенные targeted gates, как предписано заданием §7).**
`cargo fmt --all -- --check` — чисто; `cargo clippy -p meridian-core -p
meridian-app -p meridian-cli --all-targets --all-features -- -D warnings`
— чисто; `cargo test -p meridian-core existing_project_compatibility_mode`
— 8 passed; `cargo test -p meridian-core instruction_source` — 34 passed;
`cargo test -p meridian-app instruction_source_registry` — 12 passed;
`cargo test -p meridian-app existing_project_compatibility_mode` — 21
passed (включает три новых регрессионных теста и два структурных); `cargo
test -p meridian-app controlled_rule_intake` — 24 passed, 0 failed, без
регрессии пилота; `cargo test -p meridian-cli instruction_source_registry`
— 2 passed; `cargo test -p meridian-cli existing_project_compatibility_mode`
— 1 passed; `node --check test/conformance-harness.test.mjs` — чисто (три
блока этого раунда синтаксически проверены, НЕ исполнены полным прогоном —
предписано заданием, полный `node --test` — после архитектурного
одобрения); `git diff --check` — чисто. Полный `cargo test --workspace`,
`node test/kernel-validate.test.mjs`, полный Node conformance/workspace
gate НЕ запускались — прямо запрещено заданием этого раунда. Git: `HEAD` не
сдвигался, индекс пуст,
`branch/switch/add/commit/merge/rebase/reset/stash/tag/push` не
выполнялись; чужие незакоммиченные изменения не тронуты.

**Условие перехода (после второго корректирующего раунда).** Статус
передачи остаётся `READY_FOR_ARCHITECT_REVIEW`. Пункт 5 (location-match
отличие) явно оставлен НЕ решённым исполнителем и вынесен архитектору —
это открытый вопрос передачи, не дефект реализации.

**Раунд 3 (финализационный).** Архитектор рассмотрел вынесенный пункт 5 и
принял его как обоснованное Rust-native сужение — тот же прецедент, что уже
принят для `controlled-rule-intake`'s schema short-circuit (§5.14): итоговый
вердикт записи не меняется ни в одном найденном случае, меняется только
состав вторичных диагностик, а собственный дефект слота и участие слота в
outcome partition (по независимо типизированному `SemanticId`) сохраняются
на обеих сторонах без изменений. Raw-field `sameLocation`-паритет НЕ
восстанавливается.

Выполнено по итогам решения:

1. **`COMPATIBILITY.md` получил отдельную принятую запись** для
   `existing-project-compatibility-mode` (`rust-architecture-conformance-2`,
   корректирующий раунд, item 5), пятая строка реестра намеренных
   Rust-native усилений — формат идентичен уже принятым записям
   `controlled-rule-intake`/`instruction-source-registry`/
   `existing-project-compatibility-mode` (transport-parse-failure) выше в
   том же реестре.
2. **Тестовая пара переименована** из `flagged_*`/"FLAGGED" в обычную
   именованную границу: Rust —
   `a_discovered_source_naming_an_invalid_plan_slot_gets_no_secondary_location_mismatch_diagnostic`
   (было `flagged_a_discovered_source_naming_an_invalid_plan_slot_gets_no_location_mismatch_diagnostic_on_the_rust_side`);
   Node-блок в `test/conformance-harness.test.mjs` — заголовок `check()`
   переписан без «не принято»/«ждёт решения архитектора». Формулировки
   "not accepted"/"awaiting architect decision" удалены из doc-комментариев
   обеих сторон и заменены точным описанием принятой границы. Мутации и
   assertions НЕ ослаблены — Rust-тест, наоборот, усилен третьей проверкой
   (`no outcome` для того же слота отсутствует, подтверждая, что слот
   остаётся учтён в partition), доказывающей соответствие claim'у
   `COMPATIBILITY.md` о сохранении outcome partition.
3. **Статус передачи.** Не `ACCEPTED` — полный набор ворот выполнен этим же
   раундом (ниже), но архитектурное решение о самом пакете остаётся за
   архитектором после независимой проверки. Промежуточный статус:
   `architecture_approved_pending_final_gate_review` — архитектурное
   решение по всем пунктам `CHANGES_REQUESTED` принято (пункты 1-6 второго
   раунда закрыты, пункт 5 разрешён этим раундом), полный технический gate
   зелёный (ниже), но итоговый вердикт `ACCEPTED`/`CHANGES_REQUESTED`
   пакета в целом — решение архитектора после этой передачи, не
   самопровозглашённое исполнителем.

**Проверки, выполненные исполнителем (2026-09-22, финализационный раунд —
полный назначенный набор, как предписано заданием, выполнен
последовательно).** `cargo fmt --all -- --check` — чисто; `cargo build
--workspace` — чисто; `cargo clippy --workspace --all-targets
--all-features -- -D warnings` — чисто; `RUSTDOCFLAGS="-D warnings" cargo
doc --workspace --no-deps` — чисто, 0 предупреждений (после исправления
пяти устаревших/повреждённых intra-doc-ссылок, обнаруженных ТОЛЬКО этим
прогоном — см. ниже); `cargo test --workspace` — **659 passed, 0 failed**
по всем бинарям тестов workspace; `node --test
test/conformance-harness.test.mjs` — **89 passed, 0 failed** (665.3s;
включает обе новые пары этого пакета — transport-parse-failure для
`instruction-source-registry`/`existing-project-compatibility-mode` и
принятую границу «invalid plan slot secondary location match»); `node
test/kernel-validate.test.mjs` — **293 passed, 0 failed, 0 skipped** (без
регрессии — то же число, что и во всех предыдущих раундах); `git diff
--check` — чисто; `git diff --cached --quiet` — чисто (индекс пуст); `git
status --short --branch` — `dev...origin/dev [ahead 17]`, без staged
записей; `git rev-parse HEAD` — `07229f6a6bd0ebbaf5957e6549d990fedbb71fe2`
(не сдвигался с начала пакета). Git:
`branch/switch/add/commit/merge/rebase/reset/stash/tag/push` не
выполнялись; чужие незакоммиченные изменения не тронуты.

**Побочная находка этого раунда: пять устаревших/повреждённых intra-doc-ссылок.**
`cargo doc --workspace --no-deps` — единственная команда во всей программе,
фактически проверяющая doc-ссылки — не входила в назначенный набор ни
одного из двух предыдущих раундов (задание §11 обоих ограничивало набор
целевой областью). Она вскрыла: (а) doc-комментарий
`controlled_rule_intake/mod.rs` всё ещё ссылался на
`super::instruction_source_registry::check_read_channel` — функцию,
удалённую ПЕРВЫМ раундом этого пакета при типизации
`instruction_source_registry` (реальная, ранее незамеченная регрессия
документации, не просто синтаксическая ошибка ссылки) — переписан на
точное описание фактического пути (`meridian_core::instruction_source::ReadChannel::try_new`,
общий для обоих модулей); (б) четыре ссылки на приватные элементы
(`validate_resolved_response`, `domain`, `evaluate_connection`,
`convert::build_source` дважды) в модулях, написанных этим пакетом —
заменены на обычный `code`-текст без док-ссылки. Ни одно исправление не
меняет поведение — только текст документации.

**Итоговый вердикт архитектора (2026-09-22): `ACCEPTED`.** Полный gate
финализационного раунда принят как достаточное доказательство пакета
`rust-architecture-conformance-2`; открытых архитектурных пунктов пакета не
осталось. Это принятие относится только к двум контрактам §5.15 и не означает,
что вся уже написанная Rust-часть приведена к целевой архитектуре. Пакеты
`rust-architecture-conformance-1` и `-2` локально интегрированы сопровождающим
эту запись package/merge flow; следующий пакет специфицирован, но не начат.

## 5.16. Пакет `meridian-cli-foundation-architecture-remediation`: исторический 7a и `WorkspaceReader` (задание, 2026-09-22)

### 5.16.1. Статус и условие старта

Статус пакета: `ACCEPTED_AND_LOCALLY_INTEGRATED`. Итоговый вердикт архитектора
и сопровождающий его локальный package/merge flow записаны в §5.16.8. Условие
старта ниже исторически зафиксировано как оно стояло в задании.

Пакет можно начать только после того, как принятые
`rust-architecture-conformance-1` и `rust-architecture-conformance-2` будут
зафиксированы обычной Git-интеграцией по §5.4 и новый рабочий каталог будет
создан от содержащей их интеграционной ревизии. Наличие их файлов только как
незакоммиченных изменений в прежнем рабочем дереве условие старта не выполняет.

### 5.16.2. Цель и содержательная граница

Пакет исправляет ровно исторический подпакет 7a
`validate-mechanical-integrity` как один связный вертикальный срез:

1. `sha-provenance`;
2. `instruction-topics`;
3. `operating-foundation`;
4. `stack-profiles`;
5. `agent-instruction-identity`.

Эти семейства уже образуют одну композицию: YAML/Markdown-пулы, декларации
документов и skill pins читаются из одного Kernel workspace, а
`agent-instruction-identity` потребляет принятый `instruction-topics` pool.
Пакет устраняет вывод A3 и часть A4 аудита §5.13 на реальном production path,
а не добавляет неиспользуемую абстракцию.

```text
WorkspaceReader (bytes/entries)
  -> закрытый transport DTO / source-format parser в meridian-app
  -> валидирующий domain constructor
  -> типы и чистые проверки в meridian-core
  -> Vec<Diagnostic>
  -> существующий CLI human/json presentation
```

### 5.16.3. Обязательная архитектура результата

1. **`meridian-core`.** Получает предметные типы и чистые проверки пяти
   семейств. После transport boundary нет `serde_json::Value`, строковых enum
   и сочетаний полей, допускающих противоречивое состояние. Core не читает
   файлы, Git, env, process, сеть или БД и ничего не выводит.
2. **`meridian-app`.** Определяет app-owned trait `WorkspaceReader` и
   оркестрирует пять проверок. Порт работает только с валидированными
   относительными workspace-путями, различает отсутствие и ошибку чтения и
   возвращает типизированные directory entries без следования по symlink.
   Минимальная реально используемая поверхность покрывает чтение text/bytes,
   перечисление файлов и непосредственных каталогов; методы «на будущее»
   запрещены.
3. **`meridian-cli`.** Содержит единственную production-реализацию
   `WorkspaceReader`, привязывающую один проверенный root к файловой системе,
   и presentation/composition. Прямой `std::fs` для этих пяти семейств
   допускается только внутри адаптера. Файлы
   `commands/validate/{sha_provenance,instruction_topics,
   operating_foundation,stack_profiles,agent_instruction_identity}.rs`
   удаляются либо становятся тонкими shims без предметных правил, regex,
   `serde_json::Value` и самостоятельного файлового обхода.
4. **Пути.** Путь порта — отдельный строгий тип либо осознанно
   переиспользованный общий относительный путь core: непустой,
   POSIX-relative, нормализованный, без `.`/`..`, обратной косой черты и
   absolute/drive-prefixed формы. Проверка выхода за root не может оставаться
   строковым соглашением адаптера.
5. **Ошибки и диагностика.** Ошибки доступа порта типизированы и не
   смешиваются с отрицательным предметным результатом. App/core возвращают
   `meridian_core::types::Diagnostic`, не `Vec<String>`. Наблюдаемые тексты,
   уровни, порядок и CLI exit-code сохраняются; presentation извлекает текст
   только на внешней границе.
6. **Уровни проверки.** Schema/transport validation, построение домена и
   чистые предметные проверки — отдельные функции/модули. Успешный schema
   check сам по себе не является построенным доменным объектом.
7. **Композиция.** `agent-instruction-identity` получает типизированный
   результат реального `instruction-topics` pipeline. Второй парсер или
   копия topic pool запрещены. Общие `parse_yaml`, marked regions/front
   matter переиспользуются, а не переписываются в core или CLI.
8. **SHA и symlink.** Digest считается над фактически прочитанными bytes;
   текстовый артефакт сохраняет текущую семантику кодировки. Symlink под
   `skills/` не становится каталогом и не обходится. Ошибка directory entry
   или неполный обход не превращаются в пустой успешный набор.
9. **Rust-native надёжность.** Невалидные состояния делаются
   непредставимыми. Принятые различия `COMPATIBILITY.md`, включая wrong-type
   `stack-profiles`, сохраняются. Любое НОВОЕ наблюдаемое отличие получает
   matched Node/Rust test и статус `BLOCKED_FOR_ARCHITECT_DECISION`;
   исполнитель не принимает его сам.
10. **Production panic audit.** Каждый `unwrap`/`expect` в изменённом
    production scope удалён либо обоснован доказуемым внутренним инвариантом.
    Workspace/YAML/Markdown/path/directory-entry data внутренним инвариантом
    не считаются.

Рекомендуемая физическая декомпозиция (названия подмодулей можно уточнить без
изменения владения):

```text
meridian-core/src/mechanical_integrity/...
meridian-app/src/workspace/{mod,reader}.rs
meridian-app/src/validation/mechanical_integrity/...
meridian-cli/src/adapters/workspace_reader.rs
meridian-cli/src/commands/validate/...  # composition/presentation only
```

### 5.16.4. Явно вне объёма

- `GitInspector` и замена `std::process::Command("git")`;
- `Clock`;
- остальные механические семейства `validate`;
- Value-centric семейства 7b/7c (`bounded-context-manifest`,
  `task-specification`, `execution-state`, `role-and-human-control`,
  `task-pattern-registry`, `functional-parity`, `evidence-and-handoff`,
  `field-evaluation`);
- четыре заблокированных семейства 7d, пакет 8, SQLite, import и migration;
- изменение Node-эталона ради подгонки, удаление Node-пути, изменение
  публичного CLI JSON/human формата или exit codes;
- перевод всех файловых операций CLI на новый порт одним пакетом.

`GitInspector` и `Clock` вынесены сознательно: они не нужны пяти операциям
этого пакета. Каждый следующий порт вводится только вместе с production-
операцией, которая действительно его использует.

### 5.16.5. Критерии приёмки

Пакет получает `ACCEPTED` только если одновременно выполнены все пункты:

1. Все пять CLI-проверок проходят через одну production-реализацию app-owned
   `WorkspaceReader`; обходного прямого чтения в их CLI-модулях нет.
2. Для каждого семейства предъявлен полный маршрут DTO -> constructor -> core
   check -> `Diagnostic`; core check тестируется без файловой системы.
3. `serde_json::Value` не пересекает transport boundary; `Vec<String>` не
   является результатом app/core операций пакета.
4. Типы предотвращают как минимум небезопасный workspace path, неизвестные
   закрытые enum-значения, половинчатую agent-instruction identity,
   противоречивый/неполный skill pin и `universal` как профиль.
5. Partial-construction audit тестами доказывает, что один нестроящийся объект
   не подавляет независимые диагностики остальных объектов/полей, кроме
   обоснованно недостижимой проверки над отсутствующим typed value.
6. Missing/unreadable/walk/symlink ошибки воспроизводятся fake-reader
   app-тестами; CLI adapter отдельно проверен real-filesystem fixtures.
7. Структурные gates доказывают: core без external I/O; app без concrete
   filesystem adapter; пять CLI-модулей без предметных функций,
   `serde_json::Value`, regex и прямого `std::fs` вне адаптера.
8. Сохраняются fixtures, порядок диагностик, human/json output и exit codes;
   conformance corpus запускает реальные Node и Rust CLI над одинаковыми
   мутациями пяти семейств.
9. Temp paths уникальны, cleanup выполняется RAII даже при panic; чужие
   файлы/изменения не удаляются.
10. `COMPATIBILITY.md` содержит все и только принятые различия scope; новых
    различий без решения архитектора нет.
11. Публичные типы/порты проходят rustdoc с warnings-as-errors; нет ссылок на
    удалённые CLI-функции.
12. Передача содержит карту `старый CLI symbol -> новый owner/symbol`, аудит
    production `unwrap`/`expect` и точные результаты ворот.

### 5.16.6. Ворота исполнения

Первая передача запускает последовательно только целевые ворота:

```bash
cargo fmt --all -- --check
cargo clippy -p meridian-core -p meridian-app -p meridian-cli --all-targets --all-features -- -D warnings
cargo test -p meridian-core mechanical_integrity
cargo test -p meridian-app mechanical_integrity
cargo test -p meridian-cli mechanical_integrity
node --check test/conformance-harness.test.mjs
git diff --check
```

После архитектурного одобрения архитектор назначает полный gate:

```bash
cargo fmt --all -- --check
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo test --workspace
node --test test/conformance-harness.test.mjs
node test/kernel-validate.test.mjs
git diff --check
```

**Правило корректирующих раундов.** Полный gate выше относится к одному
финальному кандидату после архитектурного одобрения и не повторяется после
каждого `CHANGES_REQUESTED`. В промежуточном корректирующем раунде исполнитель
запускает только целевые Rust-ворота, `node --check` для изменённого
`test/conformance-harness.test.mjs` и `git diff --check`. Если раунд меняет
наблюдаемое Node/Rust-поведение, запускается отдельный сфокусированный
исполняемый case, когда харнесс позволяет выбрать его безопасно; иначе тест
добавляется и синтаксически проверяется, а полный
`node --test test/conformance-harness.test.mjs` откладывается до финального
gate. `node test/kernel-validate.test.mjs` в корректирующих раундах не
повторяется без отдельного прямого назначения. После последней Rust-правки оба
полных Node-набора выполняются один раз на неизменяемом финальном кандидате.
Это правило является применением общего протокола `AGENTS.md` §9 и имеет
приоритет над механическим копированием списка финальных ворот в очередное
задание исполнителю.

Conformance не заменяет §5.16.3–§5.16.5. Полный Node gate исполнитель не
запускает до архитектурного одобрения первой передачи.

### 5.16.7. Готовое задание исполнителю

> Работай над пакетом
> `meridian-cli-foundation-architecture-remediation` строго по §5.16
> `governance/plans/meridian-rust-migration-program-plan.md`.
>
> До изменений выполни `node scripts/preflight.mjs`, проверь, что база
> содержит интегрированные `rust-architecture-conformance-1` и `-2`, а
> рабочее дерево не требует переноса чужих незакоммиченных изменений. Если
> пакеты есть только как dirty diff или интеграционной ревизии нет, остановись
> со статусом `BLOCKED_PENDING_PRIOR_PACKAGE_INTEGRATION`.
>
> Перенеси ровно пять семейств исторического 7a (`sha-provenance`,
> `instruction-topics`, `operating-foundation`, `stack-profiles`,
> `agent-instruction-identity`) на маршрут `WorkspaceReader -> app
> DTO/source format -> core domain/checks -> Vec<Diagnostic> -> CLI
> presentation`. Определи `WorkspaceReader` в `meridian-app`, реализуй его
> только в `meridian-cli`, используй строгий relative workspace path, typed
> I/O errors и non-following symlink policy. Не добавляй `GitInspector` или
> `Clock`, не трогай 7b–7d, migration, SQLite и публичный CLI contract.
>
> Сохрани real Node/Rust corpus. Новое наблюдаемое различие не принимай:
> добавь matched test, опиши бизнес-эффект и остановись с
> `BLOCKED_FOR_ARCHITECT_DECISION`. Не выполняй branch/switch/add/commit/
> merge/rebase/reset/stash/tag/push и не форматируй чужие файлы вне scope.
>
> Для первой передачи выполни только targeted gates §5.16.6. Передай
> `READY_FOR_ARCHITECT_REVIEW` (не `ACCEPTED`), карту перемещённых symbols,
> список production panic sites, различия и результаты каждой команды.

### 5.16.8. Реализация и статус передачи (исполнитель, 2026-09-22)

Пакет реализован исполнителем в рабочем дереве (без Git-записей) за первую
передачу и четыре последующих корректирующих раунда `CHANGES_REQUESTED`
архитектора. Статус передачи: `READY_FOR_ARCHITECT_REVIEW`. Не `ACCEPTED` —
итоговый вердикт принимает архитектор после независимой проверки; настоящая
запись не провозглашает приёмку самостоятельно.

**Итоговый вердикт архитектора (2026-09-22): `ACCEPTED`.** Все замечания пяти
раундов закрыты; полный финальный gate четвёртого корректирующего раунда и
целевой документационный gate финализационного раунда приняты как достаточное
доказательство. Пакет локально интегрирован сопровождающим эту запись
package/merge flow. Это принятие не начинает 7d или пакет 8 и не разрешает
публикацию `dev`.

**Содержание раундов.** Раунд 1 — маршрут `WorkspaceReader -> app DTO -> core
domain/checks -> Vec<Diagnostic>` для всех пяти семейств, единственная
production-реализация порта (`meridian_cli::adapters::workspace_reader::FsWorkspaceReader`),
CLI-модули сведены к тонким shims. Раунд 2 — типизированные directory entries
(`meridian_core::types::EntryName`), устранение fail-open путей парсера
(`meridian_app::validation::mechanical_integrity::{regex_is_match,
regex_capture1, regex_collect_captures}`), явный typed-result вместо
`filter_map(...ok())` в `instruction-topics`/`stack-profiles`, два намеренных
Rust-native отличия (`sha-provenance` неполный `source_archive`,
`operating-foundation` entry без `id`) документированы в `COMPATIBILITY.md` с
matched Node/Rust conformance-тестами. Раунд 3 — `agent-instruction-identity`
получил construction-report модель (`CompleteIdentity`/
`IdentityConstructionIssue`); `sha-provenance` получил валидирующие
`SkillPin::new`/`SourceArchive::new`. Раунд 4 (текущий кандидат) — `sha-provenance`
переведён на единственные production construction report builders
(`build_pin_report`/`build_archive_report` -> `PinConstructionReport`/
`ArchiveConstructionReport`), устраняющие независимый вызов low-level
валидаторов приложением; `operating-foundation` декомпозирован так, что
id/ru/en-сигнатура записи/строки остаётся доступной pool-agreement даже при
пустом `body`/`ru`/`en` (без ложного «halves disagree», без нового
`entry_missing_bilingual_name` — пустое bilingual-имя мапится на существующий
Node-совместимый mismatch); `IncoherentField` остаётся внутренним construction
issue агента-инструкции без отдельного user-visible diagnostic. Финализационный
раунд (эта передача) синхронизирует §5.16.1/§4/§10 этого документа,
исправляет две устаревшие ссылки в `COMPATIBILITY.md` и три устаревших
doc comment в `meridian-core`/`meridian-app`, не меняя код или наблюдаемое
поведение.

**Карта: старый CLI symbol -> новый owner/symbol (по пяти семействам).**

| Семейство | Старый CLI symbol (monolithic `meridian-cli/src/commands/validate/<family>.rs`) | Новый owner/symbol |
|---|---|---|
| sha-provenance | `fn run` (собственные `std::fs`, `serde_json::Value`, digest, path join); `fn short` | `meridian_app::validation::mechanical_integrity::sha_provenance::run` (оркестрация через порт); `meridian_core::mechanical_integrity::sha_provenance::{build_pin_report, build_archive_report, PinConstructionReport, ArchiveConstructionReport, SkillPin, SourceArchive, short}`; CLI — тонкий shim `meridian_cli::commands::validate::sha_provenance::run`, вызывающий app |
| instruction-topics | `fn run`, `fn dedupe_preserve_order`, `fn type_name`, `struct TestDir` | `meridian_app::validation::mechanical_integrity::instruction_topics::run`; `meridian_core::mechanical_integrity::{dedupe_preserve_order, instruction_topics::{TopicId, TopicPool, check_pool_agreement}}`; CLI — тонкий shim |
| operating-foundation | `fn run`, `fn dedupe_preserve_order`, `fn duplicates_after_first`, `struct Row` | `meridian_app::validation::mechanical_integrity::operating_foundation::{run, resolve_entries, extract_rows}`; `meridian_core::mechanical_integrity::operating_foundation::{FoundationEntry, FoundationRow, check_pool, empty_body_row_ids, entry_missing_id}`; CLI — тонкий shim |
| stack-profiles | `fn run`, `fn dedupe_preserve_order`, `fn type_name` | `meridian_app::validation::mechanical_integrity::stack_profiles::run`; `meridian_core::mechanical_integrity::stack_profiles::{StackProfileName, StackProfilePool, check_pool_agreement}`; CLI — тонкий shim |
| agent-instruction-identity | `fn run`, `fn capture`, `fn matches`, `fn relative_slash` (кандидат-отбор и regex внутри CLI) | `meridian_app::validation::mechanical_integrity::agent_instruction_identity::{run, select_candidates, RawFrontMatter}` (regex здесь; `meridian-core` без `fancy-regex`); `meridian_core::mechanical_integrity::agent_instruction_identity::{build_identity, evaluate_document, CompleteIdentity, IdentityConstructionIssue}`; CLI — тонкий shim |

Общее для всех пяти: единственная production-реализация порта —
`meridian_cli::adapters::workspace_reader::FsWorkspaceReader` (impl
`meridian_app::workspace::WorkspaceReader`); типизированный путь —
`meridian_core::types::WorkspaceRelativePath`; типизированное имя записи
каталога — `meridian_core::types::EntryName`.

**Production `unwrap`/`expect` audit (полный список, обоснование на месте).**
Ни один сайт не зависит от пользовательских/файловых данных — каждый
guarded литералом, кодовой константой или непосредственно предшествующей
проверкой:

- `meridian-core/src/mechanical_integrity/mod.rs:26,30` —
  `Diagnostic::new(level, message).expect("message is non-empty")`: `message`
  всегда построен через `format!` с непустым литеральным префиксом.
- `meridian-app/src/validation/mechanical_integrity/{operating_foundation,instruction_topics,stack_profiles}.rs`
  (`fn fail`) — тот же паттерн, тот же инвариант.
- `meridian-app/.../agent_instruction_identity.rs:85` —
  `NormField::parse(f).expect(...)`: `f` перебирает `NORM_FIELDS` — тот же
  модуль, что и `NormField::parse`, оба списка синхронизированы намеренно и
  проверены тестом.
- `meridian-app/.../agent_instruction_identity.rs:88-104` (9 сайтов) —
  `Regex::new(ЛИТЕРАЛ).expect("... pattern compiles")`: regex — исходный
  литерал, ошибка возможна только при опечатке в коде (compile-time
  инвариант), не во время выполнения.
- `meridian-app/.../{operating_foundation,instruction_topics,stack_profiles}.rs`
  — `WorkspaceRelativePath::new(YAML_PATH/MD_PATH/GLOSSARY_PATH/PRINCIPLES_PATH).expect("literal path is valid")`
  и `sha_provenance.rs` — `WorkspaceRelativePath::new("skills").expect(...)`:
  все аргументы — `const &str` литералы, синтаксически валидные workspace-пути
  по построению.
- `meridian-app/.../{operating_foundation,instruction_topics,stack_profiles}.rs`
  — `yaml_raw.unwrap()`/`glossary_raw.unwrap()`/`principles_raw.unwrap()`/
  `md_raw.unwrap()`: каждый непосредственно следует за `if x.is_none() || ...
  { return ...; }` — к моменту `.unwrap()` значение гарантированно `Some`.
- `meridian-app/.../{operating_foundation,instruction_topics,stack_profiles}.rs`
  — `Regex::new(ЛИТЕРАЛ).expect("row pattern compiles")`: тот же
  compile-time-инвариант, что и выше.

Новых `unwrap`/`expect`/`panic!` сайтов раунд 4 не добавил; список идентичен
раунду 2 (первому, где аудит был выполнен полностью).

**Точный список изменённых файлов (рабочее дерево, без Git-записей).**

Изменённые (`M`): `AGENTS.md`\*, `COMPATIBILITY.md`, `governance/plans/meridian-rust-migration-program-plan.md`\*,
`meridian-app/src/lib.rs`, `meridian-app/src/source_format/mod.rs`,
`meridian-cli/src/commands/validate/{agent_instruction_identity,document_identity,instruction_topics,mod,operating_foundation,sha_provenance,stack_profiles}.rs`,
`meridian-cli/src/lib.rs`, `meridian-cli/tests/binary_runs.rs`,
`meridian-core/src/lib.rs`, `meridian-core/src/types/mod.rs`,
`test/conformance-harness.test.mjs`.

Новые (`??`): `meridian-app/src/source_format/markdown_identity.rs`,
`meridian-app/src/validation/{mod.rs,mechanical_integrity/{mod,agent_instruction_identity,instruction_topics,operating_foundation,sha_provenance,stack_profiles}.rs}`,
`meridian-app/src/workspace/{mod,reader}.rs`,
`meridian-cli/src/adapters/{mod,workspace_reader}.rs`,
`meridian-cli/src/commands/validate/mechanical_integrity_boundary.rs`,
`meridian-core/src/mechanical_integrity/{mod,agent_instruction_identity,instruction_topics,operating_foundation,pool,sha_provenance,stack_profiles}.rs`,
`meridian-core/src/types/{entry_name,workspace_relative_path}.rs`.

\* `AGENTS.md` и этот план несут владельческую политику корректирующих раундов
(§9/выше в этом разделе), применённую до пятого раунда, и правку §5.16.1/§4/§10
этим же раундом — оба файла не относятся к production-коду пакета.

**Проверки, выполненные исполнителем.** Целевой набор §5.16.6 зелёный на
каждом из четырёх корректирующих раундов (см. соответствующие передачи в
чате с архитектором; краткая сводка последнего раунда — `cargo fmt --all --
check` чисто; `cargo build --workspace` чисто; `cargo clippy --workspace
--all-targets --all-features -- -D warnings` чисто; `RUSTDOCFLAGS="-D
warnings" cargo doc --workspace --no-deps` чисто; `cargo test -p
meridian-core mechanical_integrity` — 71 passed; `cargo test -p meridian-app
mechanical_integrity` — 54 passed; `cargo test -p meridian-cli
mechanical_integrity` — 9 passed; `cargo test -p meridian-cli` — 79 passed;
`cargo test --workspace` — 725 passed; `node --test
test/conformance-harness.test.mjs` — 99 passed, 0 failed; `node
test/kernel-validate.test.mjs` — 293 passed, 0 failed; `git diff --check`
чисто). Пятый, финализационный раунд документации Node-наборы не повторяет
(поведение и Node corpus не менялись) — запускает только корректирующие
ворота, результаты в этой же передаче исполнителя.

**Git.** `HEAD` не сдвигался на протяжении всех пяти раундов, индекс пуст,
`branch/switch/add/commit/merge/rebase/reset/stash/tag/push` не выполнялись
ни разу; чужие незакоммиченные изменения не тронуты.

## 5.17. Пакет `rust-architecture-conformance-3`: основание типизированных task-контрактов (задание, 2026-09-22)

### 5.17.1. Статус и условие старта

Статус пакета: `ACCEPTED_AND_LOCALLY_INTEGRATED` (§5.17.8). Историческое
условие старта сохраняется: начинать его было можно только от ревизии, в которой
`meridian-cli-foundation-architecture-remediation` принят и локально
интегрирован (§5.16.8). Условие было выполнено: стартовый `dev` содержал
отдельный package commit и отдельный `--no-ff` merge-коммит предшествующего
пакета. Исполнитель не выполнял Git-записи и передал результат со статусом
`READY_FOR_ARCHITECT_REVIEW`; окончательный `ACCEPTED` вынес архитектор после
независимого ревью и полного финального gate.

### 5.17.2. Почему следующий срез именно такой

Пакет исправляет ровно два оставшихся семейства исторического 7b:

1. `task-pattern-registry`;
2. `task-specification-contract`.

Это один связный вертикальный срез, а не объединение по удобству. Task
specification разрешает `task_pattern` против встроенного каталога, а текущий
CLI повторно разбирает сырой YAML каталога и собирает нетипизированный
`TaskPatternRef`. После исправления первая операция строит типизированный
`TaskPatternCatalog`, а вторая потребляет именно этот принятый результат.
Кроме того, `task_specification` сейчас является неправильным владельцем общих
`resolve_schema_ref`/`non_portable_reason`, которые импортируют следующие
семейства 7b/7c. Пакет переносит общую чистую семантику в нейтрального typed
owner, создавая основание для последующих срезов без преждевременного переноса
самих этих семейств.

Порядок после этого пакета остаётся явным: сначала остальные семейства 7b,
затем два оставшихся семейства 7c, затем четыре новых реализации 7d. Пакет 8
остаётся закрыт.

### 5.17.3. Обязательная архитектура результата

```text
WorkspaceReader + GitInspector
  -> private transport DTO / source-format boundary в meridian-app
  -> domain constructors в meridian-core
  -> TaskPatternCatalog
  -> TaskSpecification, разрешённая против этого каталога
  -> Vec<Diagnostic>
  -> существующий CLI human/json presentation
```

1. **Канонические типы core.** `meridian-core` владеет закрытыми domain-типами
   каталога и спецификации, их конструкторами и чистыми проверками. Уже
   существующие `meridian_core::resolver::{WorkKind, ChangeClass}`
   переиспользуются; второй строковый или enum-пул для тех же понятий запрещён.
   Идентификаторы, обязательные непустые тексты, допустимые сочетания
   `work_kind`/`change_class`, классификация ссылок и cardinality выражаются
   типами и конструкторами, а не соглашением над `Value`.
2. **Transport boundary.** JSON Schema и YAML/JSON decoding остаются в
   `meridian-app::source_format`/приватных transport DTO. `serde_json::Value`
   допустим только внутри этой границы и существующего общего schema engine;
   он не входит в публичную сигнатуру operations/domain constructors и не
   хранится в domain-типах пакета. Успешная schema-проверка не считается
   построенным доменным объектом.
3. **Две app-операции, одна композиция.** App-операция registry читает
   обязательные schema/envelope/YAML/fixtures и `rule-resolution.md`, строит
   `TaskPatternCatalog` и возвращает typed diagnostics. App-операция task
   specification читает свои schema/envelope/fixtures и получает уже
   построенный каталог как вход. Она не перечитывает и не разбирает
   `task-pattern-registry.yaml`, не восстанавливает ссылки из кортежей строк и
   не дублирует его валидацию.
4. **`WorkspaceReader`.** Оба production routes используют уже принятый
   app-owned `WorkspaceReader`; прямого `std::fs`, `Path::canonicalize` или
   `fs::metadata` в двух CLI command-модулях нет. Проверка canonical-link
   target выполняется через минимальную типизированную capability порта,
   которая различает missing, I/O, не-regular-file и symlink escape. Можно
   расширить `WorkspaceReader` либо выделить узкий app-owned companion port,
   но нельзя передавать callback `Fn(&str) -> Result<(), String>`.
5. **`GitInspector`.** Пакет вводит app-owned порт и CLI-адаптер для
   перечисления tracked workspace paths, потому что registry реально
   использует этот факт. Порт возвращает валидированные
   `WorkspaceRelativePath` и типизированную ошибку; запуск
   `std::process::Command("git")` остаётся только в CLI-адаптере. Поведение
   общего validate при недоступном Git — существующий явный filesystem
   fallback плюс warning — сохраняется и тестируется; operation не угадывает
   provenance по пустому списку.
6. **Общие reference/portability rules.** `resolve_schema_ref` и
   `non_portable_reason` получают нейтрального владельца в core/app, название
   которого не связывает их с task specification. Другие пока не исправленные
   модули могут быть механически переключены на этот owner без изменения их
   поведения. В `task_specification` допускается только временный публичный
   re-export без собственной логики, если он необходим для ограничения diff;
   новые потребители через него запрещены, а карта передачи обязана назвать
   оставшихся потребителей и план удаления facade.
7. **Typed diagnostics.** Core/app возвращают
   `meridian_core::types::Diagnostic`, а ошибки чтения/парсинга/построения
   представлены отдельными typed errors и не смешиваются с отрицательным
   предметным результатом. Тексты, уровни, порядок, ограничение отображения
   первых десяти registry problems, human/json output и exit codes сохраняются
   на presentation boundary, если архитектор отдельно не примет Rust-native
   отличие.
8. **Partial construction.** Ошибка одного элемента каталога не создаёт
   частично валидный `TaskPatternCatalog` и не разрешает спецификацию против
   отброшенных строк. При этом независимые ошибки элементов/полей собираются в
   прежнем детерминированном порядке настолько полно, насколько соответствующие
   typed values построимы; ранний `return` не должен скрывать независимые
   diagnostics без явного обоснования.
9. **Fixtures и production path.** Реальные fixtures проходят через те же
   transport/domain/operation entrypoints, что и реальные документы. Нельзя
   сохранять параллельный `evaluate_*(&Value) -> Vec<String>` только ради
   fixtures или conformance. Schema-pool consistency и проверка текста
   `rule-resolution.md` также входят в новую production-композицию, а не
   остаются предметной логикой CLI.
10. **Rust-native надёжность.** Неизвестные закрытые enum-значения,
    несовместимый `change_class`, пустые/дублирующиеся semantic ids,
    некорректные portable paths и неоднозначная ссылка на pattern не могут
    быть представлены как валидный domain state. Новое наблюдаемое отличие от
    Node получает matched Node/Rust test, запись в `COMPATIBILITY.md` и статус
    `BLOCKED_FOR_ARCHITECT_DECISION`; исполнитель не принимает его сам.
11. **Production panic audit.** Каждый `unwrap`/`expect` в изменённом
    production scope удалён либо обоснован доказуемым внутренним инвариантом.
    Workspace bytes, decoded DTO, schemas, fixtures, Git output, paths и
    catalogue data внутренним инвариантом не считаются.
12. **Физическая декомпозиция.** Плоские Value-centric файлы разрешено
    заменить модуль-директориями по образцу принятых conformance-пакетов.
    Имена файлов вторичны; обязательны владельцы и направление зависимостей:
    `core domain/checks <- app transport/orchestration/ports <- CLI adapters
    and presentation`.

### 5.17.4. Явно вне объёма

- `functional-parity`, `execution-state-model`, `role-and-human-control` и
  `bounded-context-manifest`, кроме механической смены импорта общих pure
  reference helpers без изменения поведения;
- оба оставшихся семейства 7c: `evidence-and-handoff-contract` и
  `meridian-field-evaluation`;
- четыре семейства 7d, пакет 8, import/migration, SQLite и release;
- `SourceResolver` и `Clock`: ни одна из двух операций пакета их не использует;
- перенос общего `kernel-purity`, document identity, link checker или всех
  Git-вызовов репозитория на новые порты одним пакетом;
- изменение Node-эталона ради подгонки, удаление Node-пути, изменение
  публичного CLI JSON/human формата или exit codes;
- косметическое переписывание всех соседних operating-model модулей.

### 5.17.5. Критерии приёмки

Пакет получает `ACCEPTED` только если одновременно выполнены все пункты:

1. Оба CLI command-модуля являются тонкой композицией/presentation и не
   содержат `std::fs`, `serde_json::Value`, regex, schema navigation,
   предметных constants/rules или собственных fixture loops.
2. Публичных production entrypoints вида `evaluate_*(&Value) -> Vec<String>`
   для двух семейств не осталось; `Value` не пересекает transport boundary.
3. Task specification получает принятый `TaskPatternCatalog`; тест доказывает,
   что она не может разрешиться против сырых, schema-only или частично
   построенных entries.
4. `WorkKind`/`ChangeClass` имеют одного владельца в core и используются
   registry, specification и resolver без преобразования enum -> string ->
   enum между слоями.
5. `WorkspaceReader` и `GitInspector` реально вызываются production route;
   fake ports покрывают missing/unreadable/malformed, Git unavailable,
   невалидный Git path, untracked target, directory target и symlink escape.
6. Git fallback сохраняет прежний warning/checked-files contract, а
   недоступность Git не маскируется пустым успешным tracked set.
7. Core проверки выполняются без filesystem/Git; app orchestration проверяется
   fake ports; CLI adapters отдельно проверяются real filesystem и локальным
   временным Git-репозиторием с RAII cleanup.
8. Все valid/invalid fixtures обоих семейств и реальные Kernel documents
   проходят новый production pipeline; diagnostic order и truncation
   проверены явно.
9. Structural gates запрещают concrete I/O в core/app, `std::process` вне
   CLI Git adapter, direct I/O/Value/domain rules в двух CLI commands и новые
   импорты общих helpers через task-specification facade.
10. `COMPATIBILITY.md` содержит все и только принятые различия scope; новых
    различий без решения архитектора нет.
11. Публичные domain types, ports и errors документированы; rustdoc проходит
    с warnings-as-errors и не ссылается на удалённые symbols.
12. Передача содержит карту `старый symbol -> новый owner/symbol`, список
    временных facade consumers, production panic audit, точный список файлов
    и результаты каждой назначенной проверки.

### 5.17.6. Ворота исполнения

Первая передача запускает последовательно только целевые ворота:

```bash
cargo fmt --all -- --check
cargo clippy -p meridian-core -p meridian-app -p meridian-cli --all-targets --all-features -- -D warnings
cargo test -p meridian-core task_pattern
cargo test -p meridian-core task_specification
cargo test -p meridian-app task_pattern
cargo test -p meridian-app task_specification
cargo test -p meridian-cli task_pattern
cargo test -p meridian-cli task_specification
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
node --check test/conformance-harness.test.mjs
git diff --check
```

Если выбранные test filters не захватывают тесты из-за окончательных имён
модулей, исполнитель перечисляет фактические test names и запускает
эквивалентные package-scoped filters; молчаливый прогон `0 tests` не считается
воротами.

После архитектурного одобрения архитектор назначает один полный gate на
неизменяемом кандидате:

```bash
cargo fmt --all -- --check
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo test --workspace
node --test test/conformance-harness.test.mjs
node test/kernel-validate.test.mjs
git diff --check
```

Для корректирующих раундов действует правило `AGENTS.md` §9: повторяются
только затронутые Rust gates, syntax check изменённого Node harness и
`git diff --check`; полный Node gate не гоняется после каждой правки и
выполняется один раз после последней Rust-правки.

### 5.17.7. Готовое задание исполнителю

> Работай над пакетом `rust-architecture-conformance-3` строго по §5.17
> `governance/plans/meridian-rust-migration-program-plan.md`.
>
> До изменений выполни `node scripts/preflight.mjs` и проверь, что база
> содержит локально интегрированный
> `meridian-cli-foundation-architecture-remediation`, а рабочее дерево не
> требует переноса чужих изменений. Если интеграционной ревизии нет или пакет
> доступен только как dirty diff, остановись со статусом
> `BLOCKED_PENDING_PRIOR_PACKAGE_INTEGRATION`.
>
> Перенеси ровно `task-pattern-registry` и
> `task-specification-contract` на маршрут `WorkspaceReader + GitInspector ->
> private transport DTO -> core constructors/checks -> TaskPatternCatalog ->
> TaskSpecification -> Vec<Diagnostic> -> CLI presentation`. Переиспользуй
> существующие core `WorkKind`/`ChangeClass`; не создавай второй пул. Task
> specification должна потреблять принятый каталог, а не повторно читать его
> YAML. Введи `GitInspector` с production CLI adapter и typed errors; direct
> filesystem/process code оставь только в CLI adapters.
>
> Перенеси общие schema-reference/portability helpers к нейтральному owner.
> Соседние модули меняй только механически ради импорта либо оставь явно
> учтённый временный facade. Не начинай остальные семейства 7b/7c, 7d или
> пакет 8. Новое наблюдаемое отличие не принимай: добавь matched test, опиши
> бизнес-эффект и остановись с `BLOCKED_FOR_ARCHITECT_DECISION`.
>
> Не выполняй branch/switch/add/commit/merge/rebase/reset/stash/tag/push и не
> форматируй посторонние файлы. Для первой передачи выполни только targeted
> gates §5.17.6. Передай `READY_FOR_ARCHITECT_REVIEW` (не `ACCEPTED`), карту
> symbols/owners, facade consumers, production panic audit, точные изменённые
> файлы и результаты каждой команды.

### 5.17.8. Реализация, приёмка и локальная интеграция (2026-09-23)

**Итоговый вердикт архитектора: `ACCEPTED`.** Пакет реализован исполнителем без
Git-записей, прошёл независимое архитектурное ревью и корректирующие раунды.
Финальный неизменяемый кандидат прошёл полный gate §5.17.6 и локально
интегрирован сопровождающим эту запись package/merge flow. Публикация ветви
или `dev` не выполнялась; остальные семейства 7b/7c, весь 7d и пакет 8 не
открыты.

**Итоговая архитектура.** `task-pattern-registry` строит закрытый
`TaskPatternCatalog`; любой schema-, DTO-, domain-, canonical-link-,
`rule-resolution.md`- или fixture-дефект оставляет каталог `None` целиком.
`task-specification-contract` получает только принятый каталог и строит
`TaskSpecification` единым публичным core-gate структуры, portability и
pattern resolution, не перечитывая `task-pattern-registry.yaml`. Общие
`resolve_schema_ref`/`non_portable_reason` принадлежат
`meridian_core::task_contracts::portability`; app `reference_portability` —
временный re-export facade для `execution_state`, `role_and_human_control`,
`bounded_context_manifest`, `evidence_and_handoff` и `field_evaluation`,
удаляемый пакетом последнего из этих потребителей.

`meridian validate` вызывает `RealGitInspector::tracked_files` один раз,
нормализует `Ok(empty)` в typed `Unavailable` до fan-out и передаёт один
результат file-universe и registry через `CachedGitInspector`. `Unavailable`
даёт ровно один прежний fallback warning; `InvalidPath` fail-closed. Реальный
Git process находится только в CLI adapter; canonical-link filesystem semantics
принадлежат app-owned `LinkTargetPort`.

Старые `evaluate_task_pattern_registry(&Value, ...)`, строковый
`TaskPatternRef` и `evaluate_task_specification(&Value, ...)` заменены
`TaskPatternEntry::try_new`, `TaskPatternCatalog::{build,resolve}` и
`check_task_specification`/`TaskSpecification`. Два CLI command-файла остались
тонкими composition/presentation shims; family prefix добавляется ровно один
раз на CLI boundary. Production panic audit оставил только доказанные инварианты
литеральных diagnostic messages, static regex и literal workspace paths;
workspace data, schemas, fixtures, Git output и catalog data ими не считаются.

Принято одно ограниченное Rust-native отличие из `COMPATIBILITY.md`: Node
независимо повторно разбирает сырой каталог, а Rust не разрешает спецификацию
против отвергнутого `TaskPatternCatalog` и добавляет одну зависимую диагностику
на уже failing Kernel. Matched conformance-case допускает ровно эту строку и
требует точного совпадения остального multiset.

**Принятый файловый периметр.** Изменены `COMPATIBILITY.md`, `Cargo.lock`,
`governance/plans/meridian-rust-migration-program-plan.md`,
`meridian-core/{Cargo.toml,src/lib.rs}`,
`meridian-app/src/operating_model/{mod,task_pattern_registry,task_specification}.rs`,
`meridian-app/src/workspace/mod.rs`, `meridian-cli/src/adapters/mod.rs`,
`meridian-cli/src/commands/validate/{mod,task_pattern_registry,task_specification}.rs`
и `test/conformance-harness.test.mjs`; добавлены
`meridian-core/src/task_contracts/{mod,catalog,portability,specification}.rs`,
`meridian-app/src/operating_model/reference_portability.rs`,
`meridian-app/src/workspace/{git_inspector,link_target}.rs` и
`meridian-cli/src/adapters/{git_inspector,link_target}.rs`.

**Финальный gate владельца.** Format, build, workspace clippy и rustdoc —
чисто; `cargo test --workspace` — 814 passed, 0 failed; conformance harness —
99 passed, 0 failed; `kernel-validate.test.mjs` — 293 passed, 0 failed, 0
skipped; `git diff --check` — чисто. Полный вывод принят как доказательство
согласно `AGENTS.md` §6.3 и §9.

## 5.18. Пакет `rust-architecture-conformance-4`: типизированное доказательство функционального паритета (задание, 2026-09-23)

### 5.18.1. Статус и условие старта

Стартовый статус пакета (историческая запись, не переписывается): при
специфицировании — `SPECIFIED_NOT_STARTED`. Начинать его можно было только от
ревизии, в которой `rust-architecture-conformance-3` принят и локально
интегрирован. Условие было выполнено: стартовый `dev` —
`6a3ff188a31b0e7e848b91a61ba3674dfbd28b0b`, отдельный package commit
`ddb29dc3319c35f4afcc3ef55a6aac412eac0443` достижим через отдельный
`--no-ff` merge-коммит; рабочее дерево перед спецификацией было чистым.

**Текущий статус (2026-09-23, после первой передачи и первого
корректирующего раунда): `READY_FOR_ARCHITECT_REVIEW`, не `ACCEPTED`.**
Исполнитель реализовал пакет в рабочем дереве (без Git-записей — ни одной
операции branch/switch/add/commit/merge/rebase/reset/stash/tag/push за весь
пакет) за первую передачу и один корректирующий раунд `CHANGES_REQUESTED`
(полная типизированная evidence-модель вместо проекции; замена
`NonEmptyString` на контрактный `EvidenceText` там, где схема задаёт только
`minLength: 1`; типизированное разделение fixture pipeline на schema
rejection / conversion drift / domain rejection / accepted evidence;
различение `NotFound`/`Io` при чтении fixtures; matched Node/Rust case для
принятого schema-I/O различия; точные multi-record diagnostic tests Node с
ordered-регрессией). Целевой набор ворот §5.18.6 зелёный на обоих раундах.
Итоговый вердикт пакета — решение архитектора после независимой проверки, не
самопровозглашённый исполнителем.

**Второй корректирующий раунд (2026-09-23), статус сохраняется:
`READY_FOR_ARCHITECT_REVIEW`, не `ACCEPTED`.** Исправлено без расширения
пакета за `functional-parity` и без Git-записей: (1) принятое evidence больше
не является проекцией — `RecordEvidence` владеет полным проверенным
`RecordInput` в приватной обёртке, единственный конструктор которой вызывает
`check_document`; core-регрессия подтверждает сохранение assertion statement,
baseline/provenance/observed result, post-change conditions/result/link,
evidence entry, gap и verdict с причинами; (2) `COMPATIBILITY.md` фиксирует
две отдельные I/O-границы — schema non-`NotFound` I/O (Node: тихий skip,
Rust: `FAIL`, вердикт меняется) и fixture non-`NotFound` I/O (Node: generic
«carries no fixtures», Rust: отдельный «… could not be read: …», уже-failing
вердикт не меняется, меняется текст); (3) добавлен matched Node/Rust
mutated-tree case для fixture-I/O в `test/conformance-harness.test.mjs`;
(4) rustdoc app-модуля перечисляет корректные пять исходов fixture case,
включая `DomainRejected`. Целевой набор ворот §5.18.6 зелёный; полные
Node-наборы и `cargo test --workspace` не запускались.

### 5.18.2. Почему следующий срез именно такой

Пакет исправляет ровно одно оставшееся семейство исторического 7b —
`functional-parity`. Оно является самостоятельным evidence-контрактом:
читает только собственные schema/fixtures и не потребляет результаты
`execution-state-model`, `role-and-human-control` или
`bounded-context-manifest`.

Три остальных семейства 7b образуют другой связный срез:
`bounded-context-manifest` уже переиспользует vocabulary execution state, а
его revision/resolver primitives потребляют оба семейства 7c. Их перенос
одновременно с функциональным паритетом смешал бы независимые домены и
создал бы пакет более чем из пяти тысяч строк текущей app-логики. Поэтому
после §5.18 остаётся один связный пакет для трёх последних семейств 7b;
только затем могут начаться два семейства 7c. 7d и пакет 8 закрыты.

### 5.18.3. Обязательная архитектура результата

```text
FsWorkspaceReader (CLI adapter)
  -> functional_parity app operation
  -> JSON Schema + private fixture/record DTO boundary
  -> typed functional-parity input в meridian-core
  -> cross-reference/inference checks
  -> Option<FunctionalParityEvidence> + Vec<Diagnostic>
  -> CLI family-prefix/presentation
```

1. **Владелец домена.** `meridian-core` получает отдельный модуль
   functional-parity evidence. Он владеет предметными типами и чистыми
   проверками; `meridian-app` владеет transport DTO и оркестрацией;
   `meridian-cli` — только concrete reader, composition и presentation.
2. **Строгие типы.** После DTO строковые закрытые множества заменяются enum:
   как минимум четыре facet, evidence kind, verification state, gap scope и
   condition relationship. Assertion/condition ids, source-state identity,
   preserved-contract catalog, verdicts, gaps, evidence coverage и contract
   links имеют отдельные типы. Нельзя ошибочно переиспользовать
   `SemanticId`, `Verdict` или `NonEmptyString`, если их допустимое множество
   не совпадает с JSON Schema этого контракта; пробелы, точка в id и отсутствие
   `BLOCKED` проверяются по фактическому контракту, а не по похожему имени.
3. **Четыре уровня.** Приватный serde DTO отражает wire shape и закрыт
   `deny_unknown_fields`; schema validation остаётся отдельным syntax gate.
   Schema-clean DTO преобразуется в typed core input, способный представить
   cross-reference inconsistency, после чего единый core constructor/check
   возвращает `(Option<FunctionalParityEvidence>, Vec<Diagnostic>)`.
   `Some` возможен только при нуле предметных проблем; принятый evidence type
   не представляет дублированные ids, висячие ссылки, ложный VERIFIED или
   неразличимые before/after states.
4. **Не проекция над `Value`.** Все поля, несущие бизнес-смысл evidence
   contract, представлены typed значениями; `serde_json::Value` не входит в
   публичные app/core сигнатуры и не хранится в domain types. Старый
   `functional_parity_consistency(&Value) -> Vec<String>` удаляется, а не
   сохраняется параллельно для fixtures.
5. **Полнота диагностик.** Core собирает независимые предметные дефекты одного
   документа в прежнем детерминированном порядке: порядок records, затем
   порядок facet/assertion declarations и полей контракта. Membership lookup
   может быть индексом, но диагностики не итерируют `HashMap`/`HashSet`.
   Недостижимая проверка над не построившимся typed значением документируется;
   один дефект ссылки не должен подавлять независимый дефект другой ссылки.
6. **Один production pipeline для fixtures.** App operation читает
   `verification/functional-parity/functional-parity-evidence.schema.json` и
   единственный fixture bundle через уже принятый `WorkspaceReader`. Valid
   fixture проходит schema -> DTO -> core и обязан дать `Some`; invalid
   fixture считается ожидаемо отклонённым schema gate либо, если schema чиста,
   core gate. Schema-clean DTO decode/conversion failure — drift слоёв и Fail,
   а не успешное отклонение invalid fixture.
7. **Typed I/O.** `ReadError::NotFound` для самой optional PHASE D schema
   сохраняет существующее `nothing to check`. Любой `ReadError::Io` для schema
   является Fail, а не молчаливым skip; missing/unreadable/malformed fixture
   различаются. Это заранее принятое fail-closed Rust-native усиление над
   Node `readIfExists`, который поглощает любую ошибку: граница, бизнес-эффект
   и тесты фиксируются в `COMPATIBILITY.md`. Если надёжный перенос этой
   разницы нельзя воспроизвести переносимым matched case, исполнитель
   останавливается с `BLOCKED_FOR_ARCHITECT_DECISION`, а не ослабляет I/O.
8. **Typed diagnostics.** Core/app возвращают
   `meridian_core::types::Diagnostic`; schema/fixture access errors принадлежат
   app, inference diagnostics — core. Источники возвращают prefix-free text.
   `functional-parity: ` добавляется ровно один раз через общий
   `with_family_prefix` на CLI boundary; human/json output и exit code не
   меняются.
9. **Переиспользование.** Используются существующие `WorkspaceReader`,
   `FsWorkspaceReader`, JSON Schema adapter, `Diagnostic` и CLI presentation.
   Новый port, второй filesystem adapter, собственный schema engine или копия
   family-prefix helper запрещены: у операции нет Git, Clock, DB или network
   зависимости.
10. **Проверяемость.** Core tests не читают filesystem. App fake-reader tests
    покрывают schema absent, schema I/O, malformed schema, missing/I/O/bad
    fixtures, bundle shape, valid Some, schema-invalid rejection,
    schema-clean domain-invalid None и deterministic multi-error order. CLI
    отдельно доказывает реальный adapter route и ровно один family prefix.
11. **Production panic audit.** Каждый `unwrap`/`expect` в изменённом
    production scope удалён либо обоснован compile-time literal/static
    invariant. Workspace bytes, JSON, schema, fixture shape, ids и ссылки
    внутренними инвариантами не считаются.
12. **Декомпозиция.** Монолит не переносится целиком в новый файл. Core
    разделяется как минимум на types/construction/checks (точные имена могут
    отличаться), app — на operation + private DTO/conversion; публичный API
    остаётся минимальным и документированным.

### 5.18.4. Явно вне объёма

- `execution-state-model`, `role-and-human-control` и
  `bounded-context-manifest`;
- оба оставшихся семейства 7c, включая перенос их импорта portability facade
  и bounded-context resolver primitives;
- четыре семейства 7d, пакет 8, SQLite, import/migration и release;
- изменение normative functional-parity schema/contract или Node reference
  ради подгонки реализации;
- проверка продуктового Instance evidence: текущий production family
  проверяет переносимый schema/fixture contract Kernel;
- изменение общего JSON Schema engine, публичного CLI JSON/human формата,
  exit codes или порядка других validate families;
- удаление `reference_portability` facade: `functional-parity` его не
  использует.

### 5.18.5. Критерии приёмки

Пакет получает `ACCEPTED` только если одновременно выполнены все пункты:

1. CLI command — тонкая composition/presentation обёртка без `std::fs`,
   `serde_json::Value`, schema navigation, fixture loop и domain rules.
2. В app/core production API нет `functional_parity_consistency(&Value)`,
   `Vec<String>` и публичного transport DTO; core не зависит от serde/JSON/I/O.
3. Schema-valid fixture преобразуется в typed input, а accepted evidence
   публикуется только при полном отсутствии предметных проблем.
4. Все одиннадцать существующих inference rules сохранены; их cross-reference
   инварианты защищены типами/constructor gate и прямыми core tests.
5. Детерминированный diagnostic order совпадает с Node для сохраняемого
   контракта и проверен multi-error тестом без зависимости от hash iteration.
6. Optional schema NotFound остаётся skip; schema I/O fail-closed отличие
   записано в `COMPATIBILITY.md` и покрыто положительным/отрицательным тестом.
7. Все реальные valid/invalid fixtures проходят тот же app production
   entrypoint; нет отдельного test-only `Value` алгоритма.
8. Fake reader покрывает ошибки доступа и parsing; реальный
   `FsWorkspaceReader` используется CLI production route и проверен отдельно.
9. Family prefix имеет одного владельца; exact-string regression доказывает
   ровно один `functional-parity: ` для app-generated failure.
10. Structural gates запрещают direct I/O/Value/domain logic в CLI, concrete
    adapter в app, serde/I/O в core и повторное появление старого symbol.
11. Новых расхождений кроме заранее принятого I/O fail-closed нет; если они
    обнаружены, пакет имеет статус `BLOCKED_FOR_ARCHITECT_DECISION`, пока
    архитектор не примет либо не отклонит каждое из них.
12. Rustdoc с warnings-as-errors зелёный; передача содержит карту старых
    symbols к новым owners, production panic audit, точный diff и результаты
    каждой назначенной проверки.

### 5.18.6. Ворота исполнения

Первая передача запускает последовательно только целевые ворота:

```bash
cargo fmt --all -- --check
cargo clippy -p meridian-core -p meridian-app -p meridian-cli --all-targets --all-features -- -D warnings
cargo test -p meridian-core functional_parity
cargo test -p meridian-app functional_parity
cargo test -p meridian-cli functional_parity
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
node --check test/conformance-harness.test.mjs
git diff --check
```

Нулевой test filter не считается воротами: исполнитель перечисляет
фактические test names и запускает ближайший package-scoped эквивалент.
Полные Node-наборы в первой передаче не запускаются. После архитектурного
одобрения архитектор назначает один полный gate неизменяемого кандидата:

```bash
cargo fmt --all -- --check
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo test --workspace
node --test test/conformance-harness.test.mjs
node test/kernel-validate.test.mjs
git diff --check
```

Для корректирующих раундов действует `AGENTS.md` §9: только затронутые Rust
checks, syntax check изменённого Node-файла и `git diff --check`; оба полных
Node-набора выполняются один раз на финальном кандидате.

### 5.18.7. Готовое задание исполнителю

> Работай над пакетом `rust-architecture-conformance-4` строго по §5.18
> `governance/plans/meridian-rust-migration-program-plan.md`.
>
> До изменений выполни `node scripts/preflight.mjs`; подтверди базовый `dev`
> с локально интегрированным `rust-architecture-conformance-3` и отсутствие
> чужого dirty diff. Если интеграционной ревизии нет, остановись с
> `BLOCKED_PENDING_PRIOR_PACKAGE_INTEGRATION`.
>
> Перенеси ровно `functional-parity` на маршрут `WorkspaceReader -> private
> DTO/schema boundary -> typed meridian-core evidence input -> constructor /
> inference checks -> Option<FunctionalParityEvidence> + Vec<Diagnostic> ->
> CLI presentation`. Удали старый публичный Value-алгоритм. Не создавай
> параллельный fixture-only путь, новый filesystem port или второй family
> prefix helper. Сохрани одиннадцать inference rules и deterministic order;
> не переиспользуй похожий общий тип, если его допустимое множество шире или
> уже фактической schema.
>
> Сохрани NotFound optional-schema как skip, но делай schema I/O fail-closed;
> зафиксируй это ограниченное Rust-native отличие в `COMPATIBILITY.md` и
> тестах. Любое другое новое наблюдаемое отличие не принимай самостоятельно:
> добавь минимальный reproduction и остановись с
> `BLOCKED_FOR_ARCHITECT_DECISION`.
>
> Не начинай остальные семейства 7b/7c, 7d или пакет 8. Не выполняй
> branch/switch/add/commit/merge/rebase/reset/stash/tag/push и не форматируй
> посторонние файлы. Для первой передачи выполни только targeted gates
> §5.18.6. Передай `READY_FOR_ARCHITECT_REVIEW` (не `ACCEPTED`), карту
> `old symbol -> new owner/symbol`, production panic audit, точный список
> файлов, результаты команд и явно перечисленные невыполненные полные gates.

### 5.18.8. Итоговый вердикт архитектора и интеграция

`ACCEPTED`, принято и локально интегрировано 2026-09-23. После
двух корректирующих раундов независимое ревью подтвердило полную typed
evidence-модель без проекции, закрытые DTO и enum-формы, единый
constructor/check gate, детерминированные Node-compatible диагностики,
однократное владение presentation prefix и две явно зафиксированные
Rust-native I/O-границы. Полный финальный gate на неизменённом
кандидате: workspace Rust tests — 852/852, conformance harness — 103/103,
`kernel-validate.test.mjs` — 293/293; fmt, build, clippy с `-D warnings`, rustdoc
с `-D warnings` и `git diff --check` также зелёные. Остальные три
семейства 7b, оба семейства 7c, весь 7d и пакет 8 этим решением не
открываются; следующий связный пакет ещё не специфицирован.

## 5.19. Пакет `rust-architecture-conformance-5`: типизированные контракты исполнения и управления запуском (задание, 2026-09-23)

### 5.19.1. Статус и условие старта

Статус: `SPECIFIED_NOT_STARTED`. Пакет можно начинать только от ревизии, в
которой `rust-architecture-conformance-4` принят и локально
интегрирован. Условие выполнено: стартовый `dev` —
`fb0a5032386155980bd75075a92e17647d287f3d`; package commit пакета 4
`879e542717985f113f3a60c491bf7bdb534872ef` достижим через отдельный
`--no-ff` merge-коммит; рабочее дерево перед спецификацией
чистое.

Целевой статус первой передачи — `READY_FOR_ARCHITECT_REVIEW`, не
`ACCEPTED`. Исполнитель не выполняет Git write-операций. Пакет не
открывает 7c, 7d или пакет 8.

**Текущий статус (2026-09-23, первая передача исполнителя):
`READY_FOR_ARCHITECT_REVIEW`, не `ACCEPTED`.** Исполнитель реализовал
пакет в рабочем дереве от `dev` `61d9a49cea7183ccb7b02c85b01a7620fd514227`:
декомпозированный `meridian_core::run_contracts` (vocabulary, identity,
envelope, execution_state, roles, human_control, revision, pinned_ref,
resolution, context_manifest), три app-операции над одним
`WorkspaceReader` с приватными закрытыми DTO и общим schema gate
(`meridian-app/src/operating_model/run_contract_boundary/`), типизированный
`ResolutionCatalogue` вместо `Value`-замыкания, тонкие CLI-модули и
временный фасад `bounded_context_manifest::compat_7c` ровно для четырёх
7c-потребителей. Единственное новое наблюдаемое отличие —
fail-closed различение `NotFound`/`Io` обязательных файлов — записано в
`COMPATIBILITY.md` и покрыто fake-reader, real-adapter и matched
Node/Rust case. Целевые ворота §5.19.6 выполнены; полные ворота не
запускались. Во время работы исполнитель ошибочно выполнил одну
Git-команду, изменившую индекс (`git mv` старого
`bounded_context_manifest.rs`), и сразу вернул обе затронутые записи
индекса к HEAD (`git restore --staged`); индекс чист, коммитов, веток и
иных Git-записей нет. Итоговый вердикт — решение архитектора.

### 5.19.2. Почему три семейства составляют один связный пакет

После §5.18 в историческом 7b остаются ровно
`execution-state-model`, `role-and-human-control` и
`bounded-context-manifest`. Это не три независимых порта:

- context manifest повторно описывает закрытые lifecycle/work-status
  vocabulary и проверенный checkpoint одного execution run;
- human control привязан к тому же run identity и сохраняет отдельные
  оси роли, надзора, связи и текущего участника;
- revision classification, pinned references и resolved-record shape из текущего
  `bounded_context_manifest.rs` уже потребляют оба ещё не перенесённых
  семейства 7c.

Их раздельный перенос создал бы промежуточные строковые DTO/domain
контракты или дубли. Одновременный перенос 7c был бы расширением
области. Поэтому пакет владеет общими typed primitives, а для 7c
оставляет только ограниченный совместимый фасад.

### 5.19.3. Обязательная архитектура результата

```text
FsWorkspaceReader (CLI adapter)
  -> three app operations over one WorkspaceReader boundary
  -> JSON Schema + private closed DTOs + fixture resolution transport
  -> typed execution / roles-control / context inputs in meridian-core
  -> pure constructors and checks
  -> Option<AcceptedDomainValue> + Vec<Diagnostic>
  -> one CLI prefix/presentation path per family
```

1. **Core — владелец домена.** `meridian-core` получает декомпозированное
   семейство run contracts, а не один перенесённый монолит. Оно владеет
   execution state, universal role catalogue, human-control state, context
   manifest, pinned-reference/revision classification и resolved checkpoint
   vocabulary. Core не зависит от serde, `Value`, JSON Schema, файлов,
   process/env или вывода.
2. **Закрытые множества — типы.** Как минимум lifecycle stage, work
   status, universal role, supervision mode, communication mode, pinned-record
   kind/slot и revision class становятся enum/newtype. Run identity, scope
   revision, actor/reference, transition sequence, role assignment, checkpoint
   и accepted manifest имеют собственные типы и валидируемые
   конструкторы. Нельзя переиспользовать существующие `Scope`,
   `Revision`, `SemanticId` или `NonEmptyString`, если их допустимое
   множество не совпадает с фактической schema.
3. **Валидные типы публикуются fail-closed.** Каждый публичный core
   constructor/check возвращает accepted type только при нуле проблем.
   Недопустимые current/history disagreement, silent stage skip,
   broken role/control history, unpinned mutable source, mismatched run ids,
   checkpoint/manifest disagreement и unresolved reference не могут попасть
   в accepted value.
4. **App — transport и orchestration.** `meridian-app` владеет приватными
   `#[serde(deny_unknown_fields)]` DTO, schema validation, logical `$schema`
   resolution, portability checks, conversion DTO -> core input, чтением
   реальных fixtures/catalogue через уже принятый `WorkspaceReader` и
   адаптацией fixture `resolution` map в typed resolution input. В
   публичном production API этих трёх семейств нет `Value` и
   `Vec<String>`.
5. **Четыре уровня не смешиваются.** Schema rejection, schema-clean DTO
   conversion drift, domain rejection и accepted domain value — разные
   типизированные исходы. Schema-clean DTO drift — всегда `Fail`, в том
   числе для fixture, помеченной invalid; test-only алгоритма нет.
6. **Resolver не остаётся `Value`-замыканием.** Fixture resolution map —
   transport data; app преобразует его в типизированный resolution
   catalogue/port input, а core проверяет typed resolved records. Проверки
   revision/pin и общего resolved shape имеют одного владельца.
7. **Временный фасад 7c.** Пока не перенесены
   `evidence-and-handoff-contract` и `field-evaluation`, старый
   `operating_model::bounded_context_manifest` может экспортировать
   только тонкие adapters/re-exports для текущих четырёх внешних
   потребителей: app `evidence_and_handoff.rs`, app
   `field_evaluation.rs`, CLI `evidence_and_handoff.rs`, CLI
   `field_evaluation.rs`. В фасаде нет второй предметной
   реализации; structural test фиксирует этот allowlist и запрещает
   пятого потребителя. Удаление фасада принадлежит пакету 7c.
8. **CLI — тонкая композиция.** `meridian-cli` только подставляет
   `FsWorkspaceReader`, компонует операции в прежнем порядке и
   добавляет family prefix через один presentation path. В CLI нет
   `std::fs`, schema/fixture parsing или domain checks этих семейств.
9. **Диагностика и детерминизм.** Core/app возвращают
   `Vec<Diagnostic>` без family prefix. Порядок диагностик сохраняется
   для наблюдаемого контракта и не зависит от hash iteration.
10. **I/O fail-closed.** Все schema, envelope, fixture bundle и канонический
    role registry этого пакета обязательны. `ReadError::NotFound` и
    `ReadError::Io` различаются и оба дают `Fail`; не-файл не
    превращается в «отсутствует». Если точный текст отличается от
    Node, новая граница записывается в `COMPATIBILITY.md` и покрывается
    fake-reader и portable matched Node/Rust cases. Исполнитель не
    принимает других новых расхождений самостоятельно.

### 5.19.4. Обязательный сохраняемый контракт

1. Сохраняются смысл, порядок и количество текущих valid/invalid
   fixtures и канонического role registry.
2. Сохраняются закрытые множества и все cross-field/history/
   resolution invariants из `scripts/lib/execution-state.mjs`,
   `scripts/lib/role-and-human-control.mjs` и
   `scripts/lib/context-manifest.mjs`.
3. Порядок семейств в `validate`, exact family prefixes, human/JSON
   presentation и exit semantics не меняются.
4. Общие execution vocabulary, revision classification и resolved-record rules
   имеют одного production owner; фасад не дублирует алгоритм.
5. Никакой accepted domain value не публикуется при schema, DTO,
   portability, domain или resolution problem.
6. Production panic audit не оставляет `unwrap`/`expect` на внешних
   данных, fixtures, filesystem и resolver output; допустимы только
   явно обоснованные внутренние инварианты.

### 5.19.5. Вне пакета

- перенос логики `evidence-and-handoff-contract` и `field-evaluation`;
- любое семейство 7d, package 8, storage, migration CLI, Metis и Concord;
- изменение Node reference implementation, schemas, fixtures и канонического
  role registry, кроме минимального conformance case для явно принятого
  Rust-native расхождения;
- Git write-операции исполнителя и форматирование несвязанных файлов.

### 5.19.6. Ворота исполнения

Первая передача запускает последовательно только целевые ворота:

```bash
cargo fmt --all -- --check
cargo clippy -p meridian-core -p meridian-app -p meridian-cli --all-targets --all-features -- -D warnings
cargo test -p meridian-core run_contracts
cargo test -p meridian-app execution_state
cargo test -p meridian-app role_and_human_control
cargo test -p meridian-app bounded_context_manifest
cargo test -p meridian-cli rust_architecture_conformance_5
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
node --check test/conformance-harness.test.mjs
git diff --check
```

Имя `run_contracts` не предписывает физическую композицию модуля, но
задаёт фильтр для нового domain-семейства. Нулевой test filter не
считается воротами: исполнитель называет фактические tests и
запускает ближайший package-scoped эквивалент. Полные Node-наборы в
первой передаче не запускаются.

После архитектурного одобрения архитектор назначает один полный
gate неизменяемого кандидата:

```bash
cargo fmt --all -- --check
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo test --workspace
node --test test/conformance-harness.test.mjs
node test/kernel-validate.test.mjs
git diff --check
```

Для корректирующих раундов действует `AGENTS.md` §9.

### 5.19.7. Готовое задание исполнителю

> Работай над пакетом `rust-architecture-conformance-5` строго по §5.19
> `governance/plans/meridian-rust-migration-program-plan.md`.
>
> До изменений выполни `node scripts/preflight.mjs`; подтверди базовый
> `dev` с локально интегрированным `rust-architecture-conformance-4` и
> отсутствие чужого dirty diff. Если ревизии нет, остановись с
> `BLOCKED_PENDING_PRIOR_PACKAGE_INTEGRATION`.
>
> Перенеси ровно `execution-state-model`, `role-and-human-control` и
> `bounded-context-manifest` на маршрут `WorkspaceReader -> private closed
> DTO/schema boundary -> typed meridian-core run contracts -> pure constructors/checks
> -> Option<AcceptedDomainValue> + Vec<Diagnostic> -> CLI presentation`. Раздели
> core по предметным ответственностям; не переноси три монолита почти
> дословно. Замени закрытые строковые множества enum/newtype; не
> переиспользуй похожий общий тип, если его множество значений не
> совпадает с schema.
>
> Убери `Value`/`Vec<String>` из production API этих трёх семейств и
> direct filesystem/schema/domain logic из CLI. Все реальные fixtures должны
> проходить тот же production entrypoint через раздельные schema / DTO /
> domain / accepted outcomes. Преобразуй fixture resolution map в typed
> resolution input; accepted value возвращай только при нуле проблем.
>
> Оставь тонкий временный фасад только для четырёх уже существующих
> 7c-потребителей и защити allowlist structural test. Не переноси сами
> `evidence-and-handoff-contract` и `field-evaluation`. Все mandatory read errors
> делай fail-closed с различением `NotFound`/`Io`; наблюдаемые
> Rust-native отличия фиксируй в `COMPATIBILITY.md` и matched tests. Любое
> иное новое расхождение не принимай: подготовь reproduction и остановись
> с `BLOCKED_FOR_ARCHITECT_DECISION`.
>
> Не начинай 7c, 7d или пакет 8. Не выполняй
> branch/switch/add/commit/merge/rebase/reset/stash/tag/push и не форматируй
> посторонние файлы. Для первой передачи выполни только targeted gates
> §5.19.6. Передай `READY_FOR_ARCHITECT_REVIEW` (не `ACCEPTED`), карту
> `old symbol -> new owner/symbol`, владельца каждого нового типа,
> production panic audit, точный список файлов, результаты каждой команды
> и явно названные невыполненные полные gates.

### 5.19.8. Итоговый вердикт архитектора и интеграция

`ACCEPTED`, принято и локально интегрировано 2026-09-23. Независимое ревью
подтвердило typed core без transport- и I/O-зависимостей, приватные закрытые DTO
и единый schema gate в app, типизированный `ResolutionCatalogue`, тонкую
CLI-presentation и ограниченный четырьмя потребителями временный `compat_7c`.
Намеренное Rust-native различение `NotFound`/`Io` для обязательных файлов
записано как fail-closed улучшение и покрыто matched Node/Rust тестом. Полный
финальный gate на неизменённом кандидате: workspace Rust tests — 910/910,
conformance harness — 105/105, `kernel-validate.test.mjs` — 293/293; fmt, build,
clippy с `-D warnings`, rustdoc с `-D warnings`, `node --check` и оба
`git diff --check` также зелёные. Семейства 7c, 7d и пакет 8 этим решением не
открываются; следующий пакет ещё не специфицирован.

## 5.20. Пакет `rust-architecture-conformance-6`: типизированные доказательства и полевая оценка (задание, 2026-09-23)

### 5.20.1. Статус и условие старта

Статус: `SPECIFIED_NOT_STARTED`. Пакет можно начинать только от ревизии, в
которой `rust-architecture-conformance-5` принят и локально интегрирован.
Условие выполнено: стартовый `dev` —
`b9eed66a2e50e6c7dc571a819e997a3b231eb2fd`; package commit пакета 5
`75a1244ac74dd92fccfd8c92cee95abeb7cd6b48` достижим через отдельный
`--no-ff` merge-коммит, а его дерево совпадает с деревом merge-коммита.
Рабочее дерево перед спецификацией чистое.

Целевой статус первой передачи — `READY_FOR_ARCHITECT_REVIEW`, не
`ACCEPTED`. Исполнитель не выполняет Git write-операций. Пакет не начинает
7d или пакет 8.

**Текущий статус (2026-09-23, первая передача исполнителя):
`READY_FOR_ARCHITECT_REVIEW`, не `ACCEPTED`.** Исполнитель реализовал
пакет в рабочем дереве от `dev` `481a9a29ee1fa8e3dcce27b98733eb7dd47a315e`:
единственный владелец доказательств `meridian_core::evidence` (словарь,
закреплённое evidence и его разрешение, вычисляемая связь claim/assertion/
evidence, двадцать разделов handoff в `evidence::handoff`), новый
`meridian_core::field_evaluation` (восемь закрытых метрик, валидируемые даты,
timestamps и окна, одно правило измерения, пересчёт агрегатов из
типизированных образцов, наблюдение и отчёт), расширенные общие примитивы
`run_contracts` (виды записей, форма ответа resolver, один закрытый контракт
ответа для трёх семейств), одна transport-to-resolution конверсия
`meridian-app/src/operating_model/record_resolution.rs`, две app-операции с
приватными закрытыми DTO и общим загрузчиком обязательных файлов, тонкие
CLI-модули. `compat_7c` и re-export `task_specification::{non_portable_reason,
resolve_schema_ref}` удалены; structural gates запрещают их возвращение. Все
163 реальных fixture-случая проходят единственный production-маршрут.
Наблюдаемые границы — заранее принятое различение `NotFound`/`Io` и
библиотечный schema short-circuit — записаны в `COMPATIBILITY.md` и покрыты
matched Node/Rust cases; одно библиотечное, не CLI-наблюдаемое сужение
(отвергнутое разрешённое измерение не агрегируется) было записано там же
для решения архитектора. Целевые ворота §5.20.7 выполнены; полные
ворота не запускались. Во время работы исполнитель ошибочно выполнил одну
Git-команду, изменившую индекс (`git mv` прежнего
`bounded_context_manifest/resolution.rs`), и сразу вернул обе затронутые
записи индекса к HEAD (`git restore --staged`); индекс чист, коммитов, веток и
иных Git-записей нет. Итоговый вердикт — решение архитектора.

**Корректирующий раунд 1 (2026-09-23): вердикт архитектора
`CHANGES_REQUESTED`; после исправления — снова `READY_FOR_ARCHITECT_REVIEW`,
не `ACCEPTED`.** Архитектор принял Rust-native границу: наблюдение,
отвергнутое закрытым контрактом measurement или обязательного окна, не
участвует в пересчёте aggregate/status; CLI-вердикт не меняется. Выявленный
дефект: `meridian_core::field_evaluation::resolved::contribution` относил
`observed`-наблюдение без measurement-объекта (отсутствует, `null`, не
объект) к «прочим», а отсутствующее или `null` обязательное окно — к «без
окна», и такие значения влияли на пересчёт. Исправлено: сначала
обрабатывается `status`; при `observed` любое measurement, кроме
`Present(Some(поля))`, прошедшего контракт, и для метрики с обязательным
окном любое `observation_period`, кроме `Present(Some(окно))` с корректными
датами и `coverage`, дают `NotComputable` — отвергнутые значения не
становятся образцом. Прямой core-тест
`field_evaluation_a_refused_resolved_measurement_or_window_is_never_aggregated`
через реальный `check_field_evaluation` доказывает отсутствие вторичных
диагностик статуса/агрегата для отсутствующего, `null` и не-объектного
measurement, невалидного `measurement.seconds`, отсутствующего, `null` и
не-объектного обязательного окна; проверен на зубы против прежней логики.
`COMPATIBILITY.md` фиксирует границу как принятую архитектором. Целевые
ворота раунда (`AGENTS.md` §9) успешны: `cargo fmt --all -- --check`,
`cargo clippy -p meridian-core -p meridian-app -p meridian-cli --all-targets
--all-features -- -D warnings`, `cargo test -p meridian-core field_evaluation`
(25), `cargo test -p meridian-app field_evaluation` (8), `cargo test -p
meridian-cli field_evaluation` (6), `RUSTDOCFLAGS="-D warnings" cargo doc
--workspace --no-deps`, `git diff --check`, `git diff --cached --check`.
Полный workspace/Node gate не запускался — он назначается после повторного
архитектурного одобрения. Git write-операций в раунде не было.

**Корректирующий раунд 2 (2026-09-23): вердикт архитектора
`CHANGES_REQUESTED` по результату первого полного gate; после исправления —
снова `READY_FOR_ARCHITECT_REVIEW`, не `ACCEPTED`.** Первый полный gate
неизменяемого кандидата упал: `cargo test --workspace` — FAILED,
`meridian-app` 265 passed / 1 failed
(`tests::no_rs_file_performs_concrete_filesystem_io_in_production_code`);
`node --test test/conformance-harness.test.mjs` — 109/0 и
`node test/kernel-validate.test.mjs` — 293/0 были зелёными, но пакет не
принят. Причина: purity-scanner `meridian-app/src/lib.rs` считал
production-частью всё до первого `#[cfg(test)]` в файле и поэтому принимал
отдельный тестовый module-файл `field_evaluation/tests.rs` (реальное чтение
fixtures через `std::fs`) за production-код. Исправлено без ослабления gate:
четыре отдельных тестовых module-файла пакета
(`meridian-app/src/operating_model/{evidence_and_handoff,field_evaluation}/tests.rs`,
`meridian-core/src/evidence/handoff/tests.rs`,
`meridian-core/src/field_evaluation/tests.rs`) явно маркированы внутренним
атрибутом `#![cfg(test)]`; scanner (`production_part` в
`meridian-app/src/lib.rs`, для обоих структурных тестов файла) считает
файл test-only только тогда, когда `#![cfg(test)]` — его первая строка
кода, иначе production-часть по-прежнему заканчивается на первом
`#[cfg(test)]`; исключения по имени файла нет. Прежнее исключение по имени
`tests.rs` в собственном structural gate пакета
(`meridian-cli/src/commands/validate/mod.rs::rust_architecture_conformance_6`)
заменено тем же правилом. Регрессия
`meridian-app/src/lib.rs::tests::only_an_explicit_test_only_module_file_is_exempt_from_the_purity_gate`
доказывает: явно test-only файл допускает `std::fs`; обычный файл, файл с
упоминанием `#![cfg(test)]` в doc-комментарии или не в голове файла
по-прежнему проверяется; реальные `tests.rs` двух семейств — test-only, их
`mod.rs` — production. Предметная реализация пакета не менялась. Целевые
проверки раунда успешны: `cargo fmt --all -- --check`; `cargo clippy -p
meridian-core -p meridian-app -p meridian-cli --all-targets --all-features --
-D warnings`; `cargo test -p meridian-app` — 267/0 (ранее упавший тест,
соседний `no_workspace_reader_implementation_exists_in_this_crates_production_code`
и новая регрессия зелёные); `cargo test -p meridian-cli
rust_architecture_conformance` — 20/0; `cargo test -p meridian-core evidence`
и `field_evaluation` — зелёные; `RUSTDOCFLAGS="-D warnings" cargo doc
--workspace --no-deps`; `git diff --check`; `git diff --cached --check`.
Повторный полный gate не запускался — он назначается после повторного
архитектурного ревью. Git write-операций в раунде не было.

### 5.20.2. Почему два семейства составляют один связный пакет

После §5.19 в историческом 7c остаются ровно два не переведённых семейства:

1. `evidence-and-handoff-contract`;
2. `meridian-field-evaluation`.

Это один связный вертикальный срез. Оба семейства разрешают pinned records и
evidence через один внешний record-resolution boundary, импортируют временный
`bounded_context_manifest::compat_7c` и используют общие revision,
portability, resolved-entry и evidence-result понятия. Field evaluation
дополнительно строит отчёт из разрешённых observations, а будущая
`upgrade-integration-qualification` должна композиционно вызывать настоящие
операции обоих семейств. Раздельный перенос оставил бы либо второй
Value-resolver, либо временную вторую доменную модель.

Фактический стартовый долг: app-монолиты
`evidence_and_handoff.rs` и `field_evaluation.rs` содержат соответственно
2909 и 2442 строки, смешивают `serde_json::Value`, transport, domain,
resolution, aggregation и строки диагностик; два CLI-модуля всё ещё владеют
`std::fs`, schema parsing, fixture loop и `unwrap`. Пакет устраняет этот
узел целиком и удаляет временные фасады, созданные специально до его начала.

### 5.20.3. Обязательная архитектура результата

```text
FsWorkspaceReader (единственный concrete adapter)
  -> две app operations
  -> общий schema gate + private closed DTO
  -> typed resolution catalogue / typed evidence projection
  -> meridian-core evidence-and-handoff + field-evaluation
  -> Option<AcceptedRecord> + Vec<Diagnostic>
  -> CLI family-prefix / human-json presentation
```

1. **Владельцы.** `meridian-core` владеет всеми предметными типами,
   конструкторами, вычисляемыми статусами и чистыми проверками двух семейств.
   `meridian-app` владеет schema/transport/resolution orchestration и
   `WorkspaceReader` operations. `meridian-cli` владеет только concrete
   adapter, порядком вызова и presentation.
2. **Расширение, не дублирование основания пакета 5.** Переиспользуются
   `run_contracts::RecordEnvelope`, scope/vocabulary, revision,
   `PinnedRef`, `ResolutionCatalogue` и typed resolver-response primitives.
   Если закрытым enum не хватает `context-manifest`,
   `field-evaluation-observation` или `evidence-result`, он расширяется
   явным вариантом и slot-specific rule; строковый escape hatch и параллельный
   `Value`-resolver запрещены. Поведение трёх принятых семейств 7b не
   ослабляется.
3. **Один evidence owner.** Уже существующий `meridian_core::evidence`
   расширяется до полного handoff-контракта либо становится тонким фасадом над
   новым декомпозированным owner. Вторые `ClaimedResult`,
   `VerifiableAssertion`, `EvidenceEntry`, `ObservedResult`,
   pin/digest или aggregate rules запрещены. Старые ограниченные types
   мигрируют без параллельного production API.
4. **Непредставимые невалидные состояния handoff.** Typed input различает
   claimed results, assertions, evidence, acceptance criteria, mandatory
   checks, source/result repository state, worktree disposition и outcome.
   `established`/verification, выполненные checks и итоговый outcome
   вычисляются, а не доверяются входу. Accepted handoff нельзя построить с
   duplicate id, dangling cross-reference, неподтверждённым evidence,
   несовпадающим run/pin или неполным closed set.
5. **Типизированная полевая оценка.** В core существуют закрытые
   `MetricId` для восьми характеристик, measurement kind, per-metric
   classification outcome, observation/report status, duration kind, coverage
   и aggregate shape. Measurement моделируется enum-вариантами так, чтобы
   classification/duration/count и metric-specific поля нельзя было смешать.
   Observation и report — разные типы; report принимает только разрешённые
   typed observations и пересчитывает aggregate сам.
6. **Время и числа.** Даты, timestamps, интервалы, non-negative counts и
   проценты получают валидируемые типы. Сохраняются календарная корректность,
   timezone offset и sub-second ordering. `NaN`/infinity, отрицательное
   значение, обратный или нулевой интервал и деление на ноль не прячутся в
   `f64`/sentinel; отсутствие вычислимого процента выражается типом.
7. **Четыре уровня границы.** Для каждого object DTO действует
   `#[serde(deny_unknown_fields)]`. Порядок:
   `Value -> envelope+family schema -> closed DTO -> typed core input ->
   constructor/check`. Schema-clean DTO decode/conversion failure — drift
   слоёв и Fail, а не ожидаемое отклонение invalid fixture. `Value` не входит
   в core и не хранится в accepted types.
8. **Typed resolution.** Bundle `resolution` разбирается один раз в app в
   typed catalogue. Отсутствующая запись, malformed response, wrong record
   kind, pin mismatch и evidence mismatch остаются различимыми. Unknown fields
   сообщаются детерминированно; output не зависит от обхода
   `HashMap`/`HashSet`. Handoff и field evaluation используют одну
   каноническую transport-to-resolution конверсию, а не по копии.
9. **Композиционные точки.** App предоставляет минимальные `pub(crate)`
   операции проверки одного уже разобранного record для будущего 7d и две
   публичные workspace operations для текущего validate route. Обе используют
   один и тот же schema/DTO/core pipeline; fixture-only или 7d-only алгоритм
   запрещён. Field report не повторно разбирает observation из JSON после
   построения typed результата.
10. **I/O и CLI.** Обязательные schema/envelope/fixture files читаются через
    существующий `WorkspaceReader`. `ReadError::NotFound` сохраняет прежний
    Node-текст; `ReadError::Io` сообщает отдельный fail-closed текст по
    образцу пакета 5. В двух CLI command-модулях нет `std::fs`,
    `serde_json::Value`, schema navigation, fixture loop, resolver
    construction или domain rules.
11. **Typed diagnostics.** Core/app возвращают
    `meridian_core::types::Diagnostic` с prefix-free сообщениями.
    `evidence-and-handoff-contract: ` и `meridian-field-evaluation: `
    добавляются ровно один раз общей CLI presentation boundary. Сохраняются
    порядок validate families, human/json output и exit codes.
12. **Удаление временных фасадов.** Удаляется
    `bounded_context_manifest::compat_7c`, его четыре consumer imports и
    allowlist-тест. Два оставшихся re-export
    `task_specification::{non_portable_reason, resolve_schema_ref}` также
    удаляются; потребители используют канонический
    `reference_portability`/core owner напрямую. Structural gate запрещает
    повторное появление обоих фасадов и старых Value entrypoints.
13. **Реальные fixtures.** Все 8 valid + 95 invalid handoff cases и
    24 valid + 36 invalid field-evaluation cases проходят через те же app
    production entrypoints. Valid case обязан вернуть `Some(AcceptedRecord)`
    без диагностик; invalid case отвергается schema или domain gate. Отдельной
    слабой проверки fixtures нет.
14. **Наблюдаемый контракт.** Существующие mutation cases и sub-second
    regression сохраняются. Schema short-circuit закрытого DTO проверяется
    прямым Node/Rust библиотечным matched case: если различается только полный
    список вторичных диагностик, граница и CLI-наблюдаемость документируются
    точно. Любое другое новое расхождение получает reproduction и
    `BLOCKED_FOR_ARCHITECT_DECISION`.
15. **Декомпозиция и panic audit.** Монолиты не переносятся целиком. Core
    разделяется минимум на handoff types/construction/checks и field
    observation/report/aggregation; app — на operation, private DTO/conversion
    и общую resolution boundary. Каждый production `unwrap`/`expect`
    удалён либо обоснован compile-time literal/static invariant; workspace
    bytes, JSON, fixture shape и resolver response такими инвариантами не
    считаются.

### 5.20.4. Обязательный сохраняемый контракт

- точные first-problem тексты существующих CLI fixture cases, если они не
  относятся к отдельно принятому Rust-native отличию;
- все 20 смысловых разделов handoff: run identity, четыре pinned refs,
  claim/assertion/evidence linkage, acceptance criteria, mandatory checks,
  repository states, worktree disposition, outcome и resolved-run coherence;
- восемь независимых field metrics без composite score, per-metric
  measurement/outcome rules, evidence binding, correction/supersedes,
  workspace/period comparability, double-count protection, 8-of-8 completeness
  и точный recomputed aggregate;
- closed resolution response, exact revision/digest pinning, portability и
  deterministic diagnostic order;
- существующие реальные schemas, fixtures, Node reference и порядок validate
  families.

Заранее принимается только уже установленное пакетами 4–5 различение
mandatory `NotFound` и `Io`, расширенное на два семейства этого пакета:
обе ветви Fail, но unreadable файл не называется missing. Оно фиксируется в
`COMPATIBILITY.md`, fake-reader tests, real-adapter test и matched
Node/Rust mutated-tree case. Иные наблюдаемые различия заранее не приняты.

### 5.20.5. Вне пакета

- четыре семейства 7d: `instance-data-migration`,
  `instance-canonical-export`, `workspace-compatibility-qualification`,
  `upgrade-integration-qualification`;
- пакет 8, import/apply/rollback, SQLite, release, Metis и Concord;
- Node `buildFieldEvaluationReport`: текущий validate gate вызывает только
  `evaluateFieldEvaluation`; builder не добавляется без отдельного
  consumer-driven пакета;
- изменение normative schemas, fixtures, Node reference или публичного CLI
  формата ради упрощения Rust-порта;
- новый filesystem/resolver port, второй concrete reader или общий рефакторинг
  уже принятых семейств вне необходимого расширения shared typed primitives;
- composite score, readiness verdict или смешение field report с handoff.

### 5.20.6. Критерии приёмки

Пакет получает `ACCEPTED` только если одновременно выполнены все пункты:

1. Оба CLI command-модуля — тонкие composition/presentation wrappers без
   прямого I/O, `Value`, schema и business logic.
2. Core не зависит от serde/JSON/I/O; app не содержит предметных алгоритмов
   над `Value` после schema boundary.
3. Accepted handoff и field records выдаются только при нуле диагностик и не
   позволяют представить перечисленные cross-field нарушения.
4. Старый `meridian_core::evidence` реально переиспользован/мигрирован;
   второго evidence domain и дублированных aggregate rules нет.
5. `ResolutionCatalogue` и run-contract primitives имеют одного владельца;
   `compat_7c` и task-specification portability facade удалены.
6. Все 20 handoff sections и восемь field metrics покрыты прямыми core tests;
   report aggregate пересчитывается из typed observations.
7. Все 163 реальные fixture cases проходят единственный production pipeline;
   schema-invalid и schema-clean domain-invalid случаи различены.
8. Missing/malformed/unreadable schema, envelope, fixture bundle и resolution
   map покрыты fake reader; реальный `FsWorkspaceReader` route проверен.
9. NotFound/Io отличие документировано и покрыто положительной/отрицательной
   границей для обоих семейств и matched Node/Rust case.
10. Multi-error order детерминирован и не зависит от hash iteration;
    independent defects не теряются из-за частично не построенного соседа.
11. Family prefix имеет одного владельца; exact-string regressions доказывают
    ровно один prefix.
12. Structural gates запрещают старые Value entrypoints, прямой I/O в CLI,
    concrete adapter в app, serde/I/O в core и повторное появление фасадов.
13. Новое observable divergence отсутствует либо отдельно воспроизведено,
    записано в `COMPATIBILITY.md` и принято архитектором.
14. Rustdoc с warnings-as-errors зелёный; передача содержит owner map,
    old-symbol map, panic audit, точный diff и результаты назначенных gates.

### 5.20.7. Ворота исполнения

Первая передача запускает последовательно только целевые ворота:

```bash
node scripts/preflight.mjs
cargo fmt --all -- --check
cargo clippy -p meridian-core -p meridian-app -p meridian-cli --all-targets --all-features -- -D warnings
cargo test -p meridian-core evidence
cargo test -p meridian-core field_evaluation
cargo test -p meridian-app evidence_and_handoff
cargo test -p meridian-app field_evaluation
cargo test -p meridian-cli evidence_and_handoff
cargo test -p meridian-cli field_evaluation
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
node --check test/conformance-harness.test.mjs
git diff --check
```

Нулевой test filter не считается воротами: исполнитель перечисляет
фактические test names и запускает ближайший package-scoped эквивалент.
Полные Node-наборы и `cargo test --workspace` в первой передаче не
запускаются. После архитектурного одобрения архитектор назначает один полный
gate неизменяемого кандидата:

```bash
cargo fmt --all -- --check
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo test --workspace
node --test test/conformance-harness.test.mjs
node test/kernel-validate.test.mjs
git diff --check
git diff --cached --check
```

Для корректирующих раундов действует `AGENTS.md` §9: только затронутые
Rust-проверки, syntax check изменённого Node-файла и оба diff-check; полный
набор выполняется один раз на финальном кандидате владельцем и передаётся
архитектору.

### 5.20.8. Готовое задание исполнителю

> Работай над пакетом `rust-architecture-conformance-6` строго по §5.20
> `governance/plans/meridian-rust-migration-program-plan.md`.
>
> До изменений выполни `node scripts/preflight.mjs`; подтверди базовый
> `dev` `b9eed66a2e50e6c7dc571a819e997a3b231eb2fd` с локально
> интегрированным package commit
> `75a1244ac74dd92fccfd8c92cee95abeb7cd6b48` и отсутствие чужого dirty
> diff. Если интеграционной ревизии нет, остановись с
> `BLOCKED_PENDING_PRIOR_PACKAGE_INTEGRATION`.
>
> Перенеси ровно `evidence-and-handoff-contract` и
> `meridian-field-evaluation` на общий маршрут `WorkspaceReader -> schema
> gate -> private closed DTO -> typed resolution -> meridian-core
> constructor/check/aggregation -> Option<AcceptedRecord> + Vec<Diagnostic>
> -> CLI presentation`. Расширяй существующие `run_contracts` и
> `meridian_core::evidence`, не создавай вторую модель. Удали
> `compat_7c` и task-specification portability re-export после перевода всех
> четырёх потребителей.
>
> Сохрани 20 разделов handoff, восемь независимых field metrics и все 163
> реальные fixture cases. Не добавляй `buildFieldEvaluationReport`,
> composite score или 7d-only путь. Сохрани deterministic diagnostics,
> sub-second ordering и вычисляемые, а не доверенные статусы.
>
> Различай mandatory `NotFound`/`Io`, зафиксируй границу в
> `COMPATIBILITY.md` и matched tests. Любое иное observable divergence не
> принимай самостоятельно: подготовь reproduction и остановись с
> `BLOCKED_FOR_ARCHITECT_DECISION`.
>
> Не начинай 7d или пакет 8. Не выполняй
> branch/switch/add/commit/merge/rebase/reset/stash/tag/push и не форматируй
> посторонние файлы. Для первой передачи выполни только targeted gates
> §5.20.7. Передай `READY_FOR_ARCHITECT_REVIEW` (не `ACCEPTED`), карту
> `old symbol -> new owner/symbol`, владельца каждого нового типа,
> production panic audit, точный список файлов, результаты каждой команды и
> явно названные невыполненные полные gates.

### 5.20.9. Итоговый вердикт архитектора и интеграция

**Итоговый вердикт архитектора: `ACCEPTED`.** Пакет реализован исполнителем
без Git-записей, прошёл независимое архитектурное ревью и два корректирующих
раунда. Финальный неизменяемый кандидат прошёл повторный полный gate §5.20.7
и локально интегрирован сопровождающим эту запись package/merge flow. Ни
пакетная ветвь, ни `dev` не публиковались; 7d и пакет 8 этим решением не
начаты.

`evidence-and-handoff-contract` и `meridian-field-evaluation` теперь идут по
единственному production-маршруту `WorkspaceReader → schema gate → private
closed DTO → typed resolution → meridian-core → Diagnostic → CLI
presentation`. `meridian_core::evidence` остаётся единственным evidence-owner,
а новый `meridian_core::field_evaluation` владеет восемью закрытыми метриками,
валидируемыми observation/report и пересчётом агрегатов из типизированных
образцов. Общий `ResolutionCatalogue` и transport-to-resolution conversion
переиспользуются тремя семействами; `compat_7c`, старые Value-entrypoints и
task-specification portability facade удалены. CLI-модули остались тонкими,
а structural gates защищают core/app/CLI границы и явно отличают test-only
module-файлы от production-кода без исключения по имени файла.

Приняты три точно ограниченные границы `COMPATIBILITY.md`: прежнее
fail-closed различение mandatory `NotFound`/`Io`; библиотечный schema
short-circuit закрытого DTO; Rust-native отказ вычислять aggregate/status из
resolved observation, отвергнутого закрытым measurement/window контрактом.
Последняя граница сохраняет внешний вердикт, не позволяет отвергнутым данным
влиять на принятый aggregate и покрыта прямым тестом полного report-check для
семи отказных форм.

Повторный полный gate на неизменённом кандидате зелёный: `cargo fmt`, workspace
build, workspace clippy с `-D warnings` и rustdoc с `-D warnings`; workspace
Rust tests — 965 passed, 0 failed; conformance harness — 109 passed, 0 failed;
`kernel-validate.test.mjs` — 293 passed, 0 failed, 0 skipped; оба
`git diff --check` — exit 0. Начальный и конечный HEAD совпали:
`481a9a29ee1fa8e3dcce27b98733eb7dd47a315e`; индекс до интеграции был чист.

## 5.21. Пакет `rust-architecture-conformance-7`: типизированная миграционная и upgrade-квалификация (задание, 2026-09-23)

### 5.21.1. Статус и условие старта

Статус: `SPECIFIED_NOT_STARTED`. Пакет можно начинать только от ревизии, в
которой `rust-architecture-conformance-6` принят и локально интегрирован.
Условие выполнено: стартовый `dev` —
`d9439f053921717e82e084aba7764cac34798f4e`; package commit пакета 6
`ed3f02ee780f8795da821c400869a90d5df9fb71` достижим через отдельный
`--no-ff` merge-коммит. Рабочее дерево перед спецификацией чистое.

Целевой статус первой передачи — `READY_FOR_ARCHITECT_REVIEW`, не
`ACCEPTED`. Исполнитель не выполняет Git write-операций. Пакет завершает
архитектурное исправление исторического 7d, но не начинает пакет 8.

**Текущий статус (2026-09-24, корректирующий раунд 1):
`READY_FOR_ARCHITECT_REVIEW`, не `ACCEPTED`.**

*Первая передача исполнителя (2026-09-24).* Исполнитель
реализовал пакет в рабочем дереве от `dev`
`7a7c7170` без Git write-операций. `meridian_core::migration` расширен до
полного владельца (`plan`: типизированный schema-shaped вход, девять свойств
как проверки, принятый `MigrationPlan` и `PinnedPlan`; `export`: вход,
`PlanBoundary::{Resolver, Pinned}`, принятый `CanonicalExport` и
`PinnedExport`; `content` — одно правило content-envelope; `projection` —
один владелец fingerprint/idempotency/export digest; `resolved` — типизированные
ответы и каталоги); прежние `types`/`checks`/`canonical` удалены. Новый
`meridian_core::qualification` (`pin`, `workspace`, `upgrade`) композирует
реальные проверки и читает решающие входы только из принятых записей;
`meridian_core::canonical::CanonicalJson` — один владелец канонического текста
открытого содержимого и ICU-порядка `localeCompare`. Исправлен латентный
дефект прежнего fingerprint (порядок ключей `unit-1`/`unit-10`), подтверждён
эталонным хешем Node. Четыре app-операции идут через общий
`migration_boundary`; добавлены минимальные composition points
(`existing_project_compatibility_mode::compose_connections`,
`task_specification::evaluate_document`, `execution_state::evaluate_case`,
`TaskSpecification::{task_pattern, scope}`). Четыре записи `BLOCKED_CHECKS` и
сам механизм (поле `result.blocked`, вердикт `BLOCKED`) удалены; чистый
Kernel: Rust `validate` — `ok`/`0`, FAIL/WARN совпадают с Node. Все 119
fixture-случаев проходят единственный production route. Три наблюдаемые
границы были вынесены на решение архитектора (`BLOCKED_FOR_ARCHITECT_DECISION`).
Целевые ворота §5.21.7 выполнены; полные ворота не запускались.

*Вердикт архитектора по первой передаче: `CHANGES_REQUESTED`.* Все три
границы приняты по существу: (1) квалификация читает решающие входы только из
записей, принятых их собственным production-check, — отвергнутый raw JSON не
участвует в scope-, workspace-, repository-set- и decision-matrix-проверках;
(2) schema-invalid план/экспорт не получает fingerprint/digest из raw JSON —
пиннинг начинается только после построения typed schema-shaped input; (3)
`upgrade-integration-qualification` использует тот же опубликованный
`TaskPatternCatalog`, что `task-specification-contract`, и при его отсутствии
fail-closed сообщает одну зависимую диагностику. Также приняты: container/
envelope schema short-circuit как тот же Rust-native класс, что принят для
пакетов 1 и 6; нейтральный Rust-хвост ошибки JSON-парсера вместо
V8-специфичного при том же вердикте и контексте; исправление UTF-8 panic
opaque-ref (`str::get(..7)`) как сближение с Node. Потребовано: перевести
строки `COMPATIBILITY.md` в «принято архитектором» с точным бизнес-эффектом,
убрать формулировки ожидания решения из harness и §5.21, добавить matched
Node/Rust доказательства на одном и том же реальном документе с одинаковой
мутацией и строку о сближении opaque-ref.

*Корректирующий раунд 1 (2026-09-24).* Без Git write-операций и без
изменения production-поведения:

- `COMPATIBILITY.md`: строки трёх границ, schema short-circuit и хвоста
  JSON-парсера — «принято архитектором» с точным бизнес-эффектом; добавлена
  строка сближения opaque-ref (прежний Rust panic на многобайтной границе
  седьмого байта устранён; Node и Rust возвращают обычный результат;
  `meridian-core/src/types/evidence_ref.rs::tests::a_multibyte_prefix_is_checked_without_panicking`).
- matched Node/Rust library cases (Rust — `meridian-app/src/operating_model/*/tests.rs`;
  Node — `test/conformance-harness.test.mjs`, блок
  `rust-architecture-conformance-7`, корректирующий раунд 1): (a) отклонённое
  собственным контрактом соединение (workspace) и field-evaluation report
  (upgrade) с raw scope/repository/next_step/workspace значениями — точные
  списки диагностик обеих сторон, первичная диагностика и точный набор
  различий; (b) schema-invalid composed plan и export — Rust сообщает
  собственную префиксированную schema-диагностику без sha256, Node —
  raw sha256-несовпадение; (c) container и envelope schema short-circuit
  четырёх семейств с независимыми schema-only и доменным дефектами; (d)
  невалидный JSON content — одинаковый отказ, отличие ограничено хвостом
  одной строки.
- Для доступа тестов к полному списку диагностик одного контейнера
  построение композиции маршрутов `workspace-compatibility-qualification` и
  `upgrade-integration-qualification` вынесено в приватную `with_route`,
  которую использует и `evaluate` (поведение не меняется).
- harness: формулировки ожидания решения заменены на принятые; сообщение о
  допустимых дополнительных Rust-строках мутации `task-pattern-registry`
  называет обе принятые зависимые строки.

Целевые ворота §5.21.7 повторены на окончательном кандидате раунда; полные
ворота не запускались (AGENTS.md §9).

### 5.21.2. Почему четыре семейства составляют один пакет

После §5.20 в `validate-migration-qualification` остаются ровно четыре
заблокированных семейства:

1. `instance-data-migration`;
2. `instance-canonical-export`;
3. `workspace-compatibility-qualification`;
4. `upgrade-integration-qualification`.

Это один обязательный композиционный срез. Canonical export доказывает
результат конкретного принятого migration plan. Workspace qualification
композирует реальные workspace-connection, migration и export records.
Upgrade qualification композирует реальные evidence-and-handoff,
field-evaluation, task-specification и execution-state records. Раздельный
перенос позволил бы qualification-модулям создать собственные упрощённые
`Value`-алгоритмы или независимо разрешить запись, уже закреплённую
родительским pin.

Фактический стартовый долг: три Node-библиотеки содержат около 2 900 строк
алгоритмов; четыре fixture bundle несут 119 случаев (23 valid, 96 invalid) и
несколько раздельных resolver maps. В Rust уже существует
`meridian_core::migration` с типами плана и чистыми проверками его девяти
контрактных свойств, но
нет полной принятой записи, canonical export и file-facing app operations.
CLI содержит четыре записи `BLOCKED_CHECKS`; после этого пакета
`validate` впервые не должен иметь архитектурно заблокированных семейств.

### 5.21.3. Обязательная архитектура результата

```text
FsWorkspaceReader
  -> четыре app operations / обязательные schemas + fixture bundles
  -> private closed DTO + typed resolution catalogues
  -> meridian-core migration/export/qualification checks
  -> композиция через реальные accepted records операций 7b/7c
  -> Option<AcceptedRecord> + Vec<Diagnostic>
  -> CLI family-prefix / human-json presentation
```

1. **Владельцы.** `meridian-core` владеет migration plan, canonical export,
   двумя qualification-моделями, digest/fingerprint, decision matrices и
   чистыми проверками. `meridian-app` владеет schema/transport/resolution
   orchestration и `WorkspaceReader` operations. `meridian-cli` владеет
   только порядком вызова и presentation.
2. **Расширение существующего migration owner.** Текущие
   `meridian_core::migration::{types,checks,resolved}` мигрируют в полный
   owner без параллельных `MigrationPlan`, `Mapping`, source/evidence/
   rollback resolution, canonicalization или fingerprint rules. Старый
   публичный API либо используется настоящим production route, либо
   заменяется с картой `old symbol -> new owner/symbol`.
3. **Непредставимые невалидные состояния.** Accepted migration plan содержит
   валидированные scope/source/units/mappings/rollback/verification,
   пересчитанные `plan_fingerprint` и `idempotency_key`. Accepted export
   содержит только envelope-valid records, подтверждённые исходным content и
   тем же принятым plan. Qualification state, blockers и open questions
   вычисляются закрытой матрицей, а не доверяются входу.
4. **Одна граница pin/resolution.** Переиспользуются
   `run_contracts::PinnedRef`, закрытые record kinds, revision/digest types
   и каноническая app-конверсия resolver response. Отсутствующая запись,
   malformed response, wrong kind, scope mismatch, stale digest и
   content/fingerprint mismatch остаются различимыми и fail-closed.
5. **Canonical export привязан к уже принятому plan.** Workspace
   qualification передаёт export-check только тот typed plan, который сама
   разрешила и проверила по `migration_plan_ref.sha256`. Независимый
   `resolveMigrationPlan` для этого шага отсутствует; подмена содержимого под
   тем же id конструктивно невозможна.
6. **Композиция, а не копирование.** Workspace qualification использует
   production-check одного workspace connection, migration plan и export.
   Upgrade qualification использует production-check одного
   evidence-and-handoff record, optional field report, task specification и
   execution run. Для уже принятых app-семейств добавляются только минимальные
   `pub(crate)` typed composition points; workspace fixture route и
   composition point сходятся в один schema/DTO/core pipeline.
7. **Accepted values идут дальше типизированно.** После успешного DTO/core
   check qualification не извлекает decision inputs повторно из исходного
   JSON. `serde_json::Value` не входит в core, accepted types и
   qualification algorithms.
8. **Четыре уровня границы.** Для каждого object DTO действует
   `#[serde(deny_unknown_fields)]`. Порядок:
   `Value -> envelope+family schema -> closed DTO -> typed core input ->
   constructor/check`. Schema-clean DTO/conversion failure — drift слоёв и
   Fail, а не допустимый способ отклонить invalid fixture.
9. **Typed resolver bundles.** Каждый именованный map fixture bundle
   разбирается ровно один раз в app в типизированный каталог своего slot.
   Unknown fields и duplicate keys не теряются; порядок диагностик не зависит
   от `HashMap`/`HashSet`.
10. **I/O и CLI.** Все обязательные schemas, envelope и fixture bundles
    читаются существующим `WorkspaceReader`. `NotFound` и `Io`
    различаются fail-closed. В четырёх CLI command-модулях нет `std::fs`,
    `serde_json::Value`, schema navigation, fixture loop, resolver
    construction или domain rules.
11. **Typed diagnostics.** Core/app возвращают prefix-free
    `Diagnostic`. Четыре family prefix добавляются ровно один раз общей CLI
    presentation boundary. Human/json output, порядок семейств и exit codes
    сохраняются.
12. **Закрытие blocker-механизма.** После подключения всех четырёх операций
    удаляются ровно четыре записи 7d из `BLOCKED_CHECKS`; если иных
    потребителей механизма нет, удаляется и сам пустой механизм. Нельзя
    скрывать незавершённую операцию пустым success или специальным fixture-only
    ответом.
13. **Реальные fixtures и route.** Все 119 случаев проходят те же public app
    operations, которые вызывает `validate`. Valid case даёт accepted
    records без Fail; invalid case отвергается schema/domain/resolution
    boundary. Отдельная слабая fixture-проверка запрещена.
14. **Conformance.** Реальный Node/Rust clean-kernel case переводится из
    заранее известного blocker divergence в совпадающий success. Сохраняются
    mutation cases для каждого семейства и отдельные adversarial cases
    stale pin, wrong kind, plan substitution, scope mismatch и неполного
    scenario set.
15. **Декомпозиция и panic audit.** Node-монолиты не копируются одним Rust-
    файлом. Core разделяется минимум на plan, export и qualification
    types/checks; app — на operation, private DTO/conversion и resolver
    catalogues. Каждый production `unwrap`/`expect` удалён либо обоснован
    compile-time literal/static invariant.

### 5.21.4. Обязательный сохраняемый контракт

- девять свойств migration contract: coverage, target grouping/authority,
  repeatability, verification, reproducibility, reversibility, idempotency и
  supersedes, включая точные scope/revision/evidence связи;
- canonical export: полное покрытие mint-targets, envelope-valid records,
  сохранение фактического source content, plan binding, digest и
  idempotency;
- workspace qualification: exact pinned connection set, отсутствие duplicate
  repository scans, полное совпадение scope, plan/export coherence и закрытая
  decision matrix;
- upgrade qualification: единая workspace identity, обязательный task
  journey, explicit nullable field report, ровно три neutral scenarios,
  настоящие task/run checks и закрытая decision matrix;
- существующие schemas, 119 fixtures, Node reference, first-problem CLI
  strings, deterministic order и validate family order.

Заранее принято только уже установленное различение mandatory
`NotFound`/`Io`: обе ветви Fail, но unreadable файл не называется
missing. Любое иное observable отличие требует reproduction,
`COMPATIBILITY.md` и решения архитектора; исполнитель не принимает его
самостоятельно.

### 5.21.5. Вне пакета

- пакет 8: `import`, `migration plan|apply|verify|rollback`, запись в
  SQLite и любой state-changing migration workflow;
- Node builder/API, которые не вызываются текущим `validate` route;
- изменение нормативных schemas, fixtures или Node reference ради упрощения
  порта;
- новый filesystem/Git/storage port, второй concrete reader или второй
  resolver protocol;
- рефакторинг принятых 7b/7c операций сверх минимальных composition points;
- публикация ветвей, release, Metis, Concord и удаление Node-реализации.

### 5.21.6. Критерии приёмки

Пакет получает `ACCEPTED` только если одновременно выполнены все пункты:

1. Четыре CLI-модуля — тонкие composition/presentation wrappers.
2. Core не зависит от serde/JSON/I/O; app не выполняет domain algorithms над
   `Value` после transport boundary.
3. Существующий migration owner расширен, а не продублирован; fingerprint,
   digest и resolver primitives имеют по одному владельцу.
4. Qualification-модули вызывают реальные production checks и потребляют
   accepted typed records; повторного JSON-разбора принятой записи нет.
5. Derived plan boundary canonical export доказана отрицательным тестом:
   независимо подложенный plan с тем же id не может повлиять на результат.
6. Все 119 fixture cases проходят единый production pipeline.
7. Missing/malformed/unreadable schemas, bundles и каждый resolver map
   покрыты fake-reader tests; реальный `FsWorkspaceReader` route проверен.
8. Все четыре `BLOCKED_CHECKS` удалены, clean-kernel Rust validate успешен,
   а Node/Rust clean-kernel case совпадает.
9. Multi-error order детерминирован; ошибки независимых composed records не
   исчезают из-за отказа соседнего record.
10. Prefix имеет одного владельца, observable strings/JSON/exit code
    сохранены либо расхождение принято отдельно.
11. Structural gates запрещают старые `Value` entrypoints, прямой I/O в
    CLI, concrete adapter в app, serde/I/O в core и пустой blocker bypass.
12. Rustdoc с warnings-as-errors зелёный; передача содержит owner map,
    old-symbol map, production panic audit, точный diff и результаты ворот.

### 5.21.7. Ворота исполнения

Первая передача запускает последовательно только целевые ворота:

```bash
node scripts/preflight.mjs
cargo fmt --all -- --check
cargo clippy -p meridian-core -p meridian-app -p meridian-cli --all-targets --all-features -- -D warnings
cargo test -p meridian-core migration
cargo test -p meridian-core qualification
cargo test -p meridian-app instance_data_migration
cargo test -p meridian-app instance_canonical_export
cargo test -p meridian-app workspace_compatibility_qualification
cargo test -p meridian-app upgrade_integration_qualification
cargo test -p meridian-cli migration
cargo test -p meridian-cli qualification
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
node --check test/conformance-harness.test.mjs
git diff --check
```

Нулевой test filter не считается воротами: исполнитель перечисляет реальные
test names и запускает ближайший package-scoped эквивалент. Полные Node-наборы
и `cargo test --workspace` в первой передаче не запускаются. После
архитектурного одобрения архитектор назначает один полный gate неизменяемого
кандидата:

```bash
cargo fmt --all -- --check
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo test --workspace
node --test test/conformance-harness.test.mjs
node test/kernel-validate.test.mjs
git diff --check
git diff --cached --check
```

Для корректирующих раундов действует `AGENTS.md` §9: только затронутые
Rust-проверки, syntax check изменённого Node-файла и оба diff-check; полный
набор выполняется один раз на финальном кандидате владельцем и передаётся
архитектору.

### 5.21.8. Готовое задание исполнителю

> Работай над пакетом `rust-architecture-conformance-7` строго по §5.21
> `governance/plans/meridian-rust-migration-program-plan.md`.
>
> До изменений выполни `node scripts/preflight.mjs`; подтверди базовый
> `dev` с локально интегрированным package commit пакета 6 и отсутствие
> чужого dirty diff. Если спецификация §5.21 ещё не интегрирована в `dev`,
> остановись с `BLOCKED_PENDING_SPEC_INTEGRATION`.
>
> Перенеси ровно четыре семейства 7d на маршрут `WorkspaceReader -> schemas
> -> private closed DTO -> typed resolution -> meridian-core ->
> Option<AcceptedRecord> + Vec<Diagnostic> -> CLI presentation`. Расширяй
> существующий `meridian_core::migration`, не создавай вторую модель.
> Qualification-операции обязаны композиционно использовать реальные
> accepted records принятых операций 7b/7c. Canonical export внутри workspace
> qualification получает только уже разрешённый и pin-проверенный plan.
>
> Сохрани 119 реальных fixture cases и добавь отрицательные доказательства
> stale pin, wrong kind, plan substitution, scope mismatch и scenario
> coverage. Удали четыре 7d blocker только после подключения production
> routes. Не начинай пакет 8 и не добавляй state-changing migration commands.
>
> Различай mandatory `NotFound`/`Io`. Любое иное observable divergence
> не принимай самостоятельно: подготовь reproduction и остановись с
> `BLOCKED_FOR_ARCHITECT_DECISION`.
>
> Не выполняй branch/switch/add/commit/merge/rebase/reset/stash/tag/push и
> не форматируй посторонние файлы. Для первой передачи выполни только
> targeted gates §5.21.7. Передай `READY_FOR_ARCHITECT_REVIEW` (не
> `ACCEPTED`), owner map, карту `old symbol -> new owner/symbol`,
> production panic audit, точный список файлов, результаты каждой команды и
> явно названные невыполненные полные gates.

### 5.21.9. Итоговый вердикт архитектора и интеграция

Итоговый вердикт: **`ACCEPTED`**.

После независимого ревью первой передачи и корректирующего раунда 1 приняты
все три вынесенные архитектурные границы: qualification использует только
accepted records собственных production-checks; fingerprint/digest строятся
только после typed schema boundary; upgrade qualification потребляет общий
опубликованный `TaskPatternCatalog` и fail-closed отклоняет его отсутствие.
Также приняты container/envelope schema short-circuit, нейтральный
Rust-специфичный хвост JSON parse error и исправление UTF-8 panic
opaque-ref. Для каждой границы добавлены matched Node/Rust доказательства,
а `COMPATIBILITY.md` фиксирует точный наблюдаемый бизнес-эффект.

Полный финальный gate выполнен владельцем на неизменённом кандидате:
`cargo fmt`, workspace build, workspace clippy с `-D warnings` и rustdoc
с `-D warnings` прошли; workspace Rust tests — 1006 passed, 0 failed;
conformance harness — 129 passed, 0 failed; `kernel-validate.test.mjs` —
293 passed, 0 failed, 0 skipped; оба `git diff --check` — exit 0.
Начальный и конечный HEAD совпали:
`7a7c717098d79aa88b670f066320fab48c60f903`; индекс до интеграции был
чист. Пакет локально интегрируется отдельным package commit и отдельным
`--no-ff` merge-коммитом без публикации. Архитектурное исправление 7a–7d
завершено; пакет 8 этим решением не начинается.

## 5.22. Пакет 8 `meridian-cli-migration` (задание, 2026-09-24)

### 5.22.1. Статус и условие старта

Исходный статус при спецификации (исторический, 2026-09-24):
**`SPECIFIED_NOT_STARTED`**. Он фиксирует момент открытия пакета и не
является текущим: текущий статус пакета ведётся в §5.22.10 и в состоянии
программы §10.

Пакет открыт только после независимой приёмки и локальной интеграции
`rust-architecture-conformance-7`. Условие выполнено: package commit
`fa9f1c02be5e129da861ba322c6d164066e23f41` достижим из `dev` через
отдельный `--no-ff` merge-коммит `a0d3cf42cebdf3e10752fa65cbabe49f18e6af61`.
Стартовый `dev` архитектурной спецификации —
`6f9b6661afb3c62304b921bcf427eb396d978d43`; рабочее дерево было чистым.

Переходный источник разрешён только для этого пакета и только для чтения.
Строгий preflight прошёл с явно заданными `MERIDIAN_KERNEL` и локальным
`MERIDIAN_INSTANCE`, содержащим принятый комплект
`migration/instance-data`. Абсолютный путь является локальной проводкой и
не записывается в код, фикстуры, документацию либо вывод CLI. Каноническая
идентичность источника берётся только из самого принятого bundle:

- `repository_ref` — точное значение принятого bundle, не встроенная
  константа Kernel;
- `revision: 59f2fa220d015cde8eed873629750aa74f72dbbe`;
- source digest и алгоритм — из `source-snapshot.json`;
- `plan_id` — точное значение принятого bundle, не встроенная константа
  Kernel;
- `plan_fingerprint: 6e426393b62c8d918def095ef34d06e871f44bfa3c6d524868103fa46e6def76`;
- 346 record units = 336 migrated + 10 retained-transitional + 0 merged.

Целевой статус первой передачи — `READY_FOR_ARCHITECT_REVIEW`, не
`ACCEPTED`. Исполнитель не выполняет Git write-операций.

### 5.22.2. Результат и граница пакета

Пакет добавляет пять production-команд: `import` и
`migration plan|apply|verify|rollback`.

`import` имеет ровно два закрытых вида входа:

- **`frozen-instance`** — принятый bundle
  `migration/instance-data` вместе с Git-источником, содержащим его
  закреплённую ревизию; этот путь доказывает фактический перенос прежнего
  Instance;
- **`canonical-records`** — полный закрытый JSON-конверт, который
  производит `meridian export --format json`, с
  `command: "export"`, `status: "ok"` и массивом канонических
  `scoped-record` в `result`; этот путь существует для доказуемого
  round trip `import → export → import`, а не как общий ingestion API.

Вид входа всегда задаётся явно. Автоматическое угадывание формата,
fallback между видами и принятие произвольного дерева документов запрещены.
`frozen-instance` направляет все 336 продуктовых записей только в базу
роли `workspace`; 10 `retained-transitional` не минтятся и остаются
явно отражены в результате. `canonical-records` маршрутизирует уже
канонические записи через `StorageRouter` по их типизированной области:
`built-in-methodology` — в `tool`, остальные области — в `workspace`.

`import frozen-instance` — один публичный orchestration route:
`plan → apply → verify`. Он не создаёт второй алгоритм рядом с
`migration plan|apply|verify`; обе поверхности обязаны вызывать те же
app operations и те же accepted domain values.

### 5.22.3. Точный контракт CLI

```text
meridian import --kernel <path> --kind frozen-instance --source <instance-repository-path> --tool-db <path> --workspace-db <path> --confirm <plan-fingerprint> [--format human|json]
meridian import --kernel <path> --kind canonical-records --input <json-path> --tool-db <path> --workspace-db <path> --confirm <sha256-of-input> [--format human|json]
meridian migration plan --kernel <path> --source <instance-repository-path> [--format human|json]
meridian migration apply --kernel <path> --source <instance-repository-path> --workspace-db <path> [--dry-run true|false] [--confirm <plan-fingerprint>] [--format human|json]
meridian migration verify --kernel <path> --source <instance-repository-path> --workspace-db <path> [--format human|json]
meridian migration rollback --kernel <path> --source <instance-repository-path> --workspace-db <path> --run <migration-run-id> --confirm <migration-run-id> [--format human|json]
```

Правила:

1. `--format` и коды завершения наследуют §7.1 технической спецификации:
   0 — положительный предметный результат; 1 — корректный отрицательный
   результат; 2 — usage; 3 — вход/окружение не позволили получить
   предметный результат.
2. JSON использует общий конверт
   `{"status","command","result"}`; `command` равен `import`,
   `migration plan`, `migration apply`, `migration verify` или
   `migration rollback`.
3. `migration apply` по умолчанию имеет `--dry-run true`. Только точная
   пара `--dry-run false --confirm <recomputed plan fingerprint>` может
   изменить состояние. Для dry-run присутствующий несовпадающий confirm
   также является отказом, а не игнорируется.
4. `import` изменяет состояние только после совпадения `--confirm`:
   fingerprint принятого плана для `frozen-instance` либо SHA-256 точных
   входных байтов для `canonical-records`.
5. `migration rollback` требует совпадения `--run` и `--confirm`;
   подтверждение плана вместо конкретного запуска недостаточно.
6. Значения `--kind` и `--dry-run` — закрытые перечисления. Неизвестное
   значение является usage error, не truthy-строкой и не fallback.
7. Ни одна команда не читает `MERIDIAN_INSTANCE`. Переходный источник
   передаётся только явным `--source`; обычные `validate`, `resolve`,
   `doctor`, `export` и `init` не получают скрытый fallback к нему.
8. Результат в stdout — ровно один документ. Ошибки и предупреждения —
   только stderr. Событийный sink не меняет stdout, stderr, exit code либо
   сохранённое состояние.

Минимальный JSON result:

- `plan`: source identity, plan id, recomputed fingerprint/idempotency key,
  346/336/10/0 counts, verification status и diagnostics;
- `apply`: dry-run, run id либо `null`, fingerprint, created, updated,
  already-applied, retained, checkpoint digest и status;
- `verify`: run/plan identity, expected/imported/missing/extra/changed/
  duplicate counts, applicability-equivalence verdict и overall status;
- `rollback`: run id, checkpoint digest, restored record-state digest и
  status;
- `import`: kind, входной digest, итог apply/import и verify без
  дублирования внутренних записей.

### 5.22.4. Обязательная архитектура

```text
explicit CLI paths and confirmation
  -> CLI source/checkpoint adapters
  -> closed app DTO + accepted plan/export/canonical records
  -> pure core migration/import/verification model
  -> app-owned MigrationRepository + existing RecordRepository ports
  -> meridian-storage-sqlite transaction/checkpoint adapter
  -> typed outcome
  -> one CLI presentation boundary
```

1. **Предметный владелец.** `meridian-core` владеет чистыми типами write set, expected
   record state, verification diff и migration outcome. Он переиспользует
   принятые `MigrationPlan`, `CanonicalExport`, fingerprints,
   idempotency и content envelope; параллельный migration model запрещён.
   Предметное ядро не знает SQLite, пути, Git, env, stdout или
   checkpoint-файлы.
2. **App.** `meridian-app` владеет orchestration, закрытыми transport DTO,
   преобразованием accepted export/canonical record в строгий
   `PutRecordRequest`, а также новым adapter-neutral
   `MigrationRepository` port. App не открывает файлы или базы и не знает
   конкретный SQLite adapter.
3. **Storage.** `meridian-storage-sqlite` реализует
   `MigrationRepository` и остаётся единственным владельцем SQL,
   транзакции, checkpoint/restore и `migration_runs`. Нужное расширение
   схемы выполняется новой последовательной атомарной миграцией; существующая
   версия схемы не переписывается задним числом.
4. **CLI.** `meridian-cli` владеет nested command grammar, явными путями,
   адаптерами source/Git/checkpoint, подтверждением и представлением. В CLI
   нет предметного сравнения записей, вычисления migration fingerprint,
   преобразования произвольного JSON в domain либо SQL.
5. **Четыре уровня.** Каждый вход проходит:
   `bytes → schema/closed DTO → domain validation → accepted typed value`.
   Schema-clean DTO/conversion failure — drift слоёв и fail-closed.
6. **Один source snapshot.** Bundle разбирается один раз за операцию.
   `registry.json`, `canonical-export.json`, source/rollback snapshots,
   reconstruction plan и evidence разрешаются из одного закреплённого
   source view. Нельзя смешать working-tree файл одного checkout с
   `git show` другой ревизии.
7. **Git boundary.** Проверка revision, tree digest и `git show` живёт в
   одном CLI adapter/port. Аргументы передаются в `Command` раздельно,
   shell-строка не строится. Отсутствующий Git, revision или object
   различаются типизированно.
8. **Workspace-only apply.** Frozen bundle должен содержать только записи,
   допустимые для `workspace`. Любая built-in запись блокирует всю
   операцию до записи первой строки; автоматическое перенаправление в
   `tool` запрещено.
9. **Одна транзакция.** Accepted frozen write set и строка
   `migration_runs` применяются одной SQLite-транзакцией. Ошибка любой
   записи откатывает весь batch и не оставляет applied run.
10. **Идемпотентность.** Повтор того же source revision/fingerprint
    возвращает typed `AlreadyApplied`, не создаёт revisions, второй
    checkpoint или второй migration run. Тот же idempotency key с иным
    содержимым — конфликт.
11. **Canonical import.** Конверт и весь массив `result` сначала полностью
    принимаются, затем записи дедуплицируются по `RecordKey` и только
    после этого разбиваются по роли.
    Mixed-role импорт заранее создаёт checkpoints обеих баз и использует
    компенсационный restore при ошибке второй базы: общей транзакции двух
    SQLite-файлов контракт не выдумывает.
12. **Проверка применимости.** Эквивалентность 61 контролируемой нормы
    вычисляется отдельно из закреплённого источника и из payload реально
    прочитанных обратно SQLite-записей. Повторное чтение источника вместо
    candidate projection не считается доказательством.
13. **События.** Input, decision, check и outcome используют существующий
    versioned `ObservedEvent`; события не заменяют `migration_runs`.

### 5.22.5. Rollback и checkpoint

Rollback не может означать удаление append-only `record_revisions` либо
перезапись опубликованной истории. Перед первым state-changing apply адаптер
создаёт согласованный checkpoint точного pre-state workspace DB и вычисляет
его digest. Checkpoint получает identity конкретного `migration_run_id`, а
run хранит plan id/fingerprint, idempotency key, pre-state digest, expected
post-state digest и checkpoint digest.

Требования:

- checkpoint создаётся SQLite-механизмом согласованного backup, а не
  копированием открытого файла обычным filesystem API;
- apply начинается только после успешного checkpoint; неудачный apply
  удаляет созданный им незапечатанный checkpoint;
- rollback разрешён только для applied run, который ещё не rolled back, и
  только если после него нет более нового migration run или иной revision
  затронутого `RecordKey`; иначе операция fail-closed и не меняет БД;
- checkpoint identity/digest и текущий post-state проверяются до restore;
- restore выполняется storage adapter, после чего база заново открывается,
  её роль/Kernel edition проверяются, а канонический record-state digest
  обязан совпасть с pre-state;
- факт rollback записывается после восстановления без изменения
  восстановленного набора текущих records. Равенство `apply → rollback`
  означает байт-идентичный канонический экспорт records, ту же
  `database_metadata`/schema version и отсутствие частичного эффекта;
  audit-строка о выполненном rollback может отличаться.

### 5.22.6. Критерии приёмки

Пакет получает `ACCEPTED` только если одновременно доказано:

1. Все пять production-команд существуют, проходят реальный binary route и
   соблюдают §5.22.3.
2. Frozen source pin и tree digest пересчитаны; bundle проходит уже
   принятые production operations пакета 7d, а не отдельный упрощённый
   validator.
3. Реальный импорт даёт ровно 336 SQLite records и 10 retained entries;
   все 346 record units учтены ровно один раз, missing/extra/duplicate = 0.
4. Каждая импортированная запись после чтения из SQLite равна принятому
   target по schema/id/title/type/scope/origin/authority/payload.
5. Applicability proof охватывает все 61 записи и сравнивает pinned source с
   импортированным candidate; lost/added/changed/duplicate = 0.
6. Второй frozen import и второй apply возвращают `AlreadyApplied`, не
   меняют export digest, revision counts, checkpoint count или migration run
   count.
7. `import canonical-records` принимает реальный вывод `meridian export`;
   следующий export байт-в-байт равен первому, а повторный импорт не создаёт
   эффект.
8. Dry-run не создаёт и не меняет database, migration run или checkpoint.
   Wrong/missing confirmation также не меняет состояние.
9. Контролируемый сбой на записи в середине 336-entry batch оставляет
   record-state, revisions и migration journal равными pre-state.
10. Apply реального frozen bundle, затем rollback того же run возвращают
    канонический record-state к pre-state; повторный rollback и rollback
    поверх более новой revision отклоняются без изменения.
11. Corrupt/missing/wrong-role/wrong-edition DB, damaged checkpoint,
    missing revision, wrong source digest, stale plan/export pin,
    source-content mismatch и plan substitution fail-closed.
12. Обычные команды не читают `MERIDIAN_INSTANCE`; offline-тест блокирует
    сеть и все пять новых команд успешно работают на локальных входах.
13. NoOp/Recording event sinks дают одинаковые stdout/stderr/exit code и
    одинаковое состояние для каждой новой команды.
14. Human/JSON вывод детерминирован; diagnostics и порядок records/runs не
    зависят от `HashMap`, SQLite row order или порядка файлов.
15. Production panic audit не оставляет `unwrap`/`expect` на внешнем
    вводе, I/O, Git, SQLite, JSON, checkpoint или confirmation boundary.

### 5.22.7. Явно вне объёма

- Metis source adapters, Confluence, произвольный Git/Markdown ingestion,
  code search, snapshots/index/graph/embeddings;
- изменение или импорт 10 `retained-transitional` записей без нового
  owner authority;
- PostgreSQL, сервер, сеть, daemon, GUI и многопользовательская блокировка;
- изменение нормативных migration schemas или принятого source bundle ради
  упрощения реализации;
- удаление Node.js, архивирование Instance и выпуск — решения пакетов 9–10
  и владельца после их ворот;
- общий backup/restore/GC CLI, произвольное удаление records или
  переписывание append-only history;
- рефакторинг принятых validate/qualification operations, кроме минимальных
  typed composition points, реально используемых новыми командами;
- запуск эксперимента Исследовательского отдела, Metis или Concord;
- Git branch/switch/add/commit/merge/rebase/reset/stash/tag/push исполнителем.

### 5.22.8. Ворота исполнения

Первая передача выполняет targeted gates:

```bash
cargo fmt --all -- --check
cargo clippy -p meridian-core -p meridian-app -p meridian-storage-sqlite -p meridian-cli --all-targets --all-features -- -D warnings
cargo test -p meridian-core migration
cargo test -p meridian-app migration
cargo test -p meridian-storage-sqlite migration
cargo test -p meridian-cli migration
cargo test -p meridian-cli --test binary_runs migration
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
MERIDIAN_KERNEL=/home/krmiftakhov/PersonalProjects/meridian MERIDIAN_INSTANCE=<local-frozen-source> node --test --test-name-pattern 'meridian-cli-migration' test/conformance-harness.test.mjs
MERIDIAN_KERNEL=/home/krmiftakhov/PersonalProjects/meridian MERIDIAN_INSTANCE=<local-frozen-source> node scripts/kernel-validate.mjs
git diff --check
git diff --cached --check
```

`<local-frozen-source>` — локальная проводка, не литерал для коммита.
Если Cargo или Node child process получает sandbox `EPERM`, повторяется
ровно та же команда через разрешённый путь; инфраструктурная блокировка не
становится кодовым вердиктом. Полные `cargo test --workspace`,
полный `kernel-validate.test.mjs` и повторный полный conformance назначает
архитектор только финальному кандидату по `AGENTS.md` §9.

Обязательные отдельные тестовые наборы:

- positive real bundle 346/336/10 и 61 applicability records;
- round trip canonical import/export;
- idempotent import/apply;
- mid-transaction failure;
- apply/rollback real bundle;
- stale/newer-state rollback refusal;
- damaged source/checkpoint/DB/pin/confirmation;
- CLI help/usage/exit codes/stdout-stderr/event-sink parity;
- structural crate-boundary and no-`MERIDIAN_INSTANCE` production scan.

### 5.22.9. Готовое задание исполнителю

> Работай только в
> `/home/krmiftakhov/PersonalProjects/meridian` от текущего `dev`.
> Прочитай `AGENTS.md`, `standards/workspace/rust-migration-quality.md`,
> §5.22/§6.5–§6.5a этого плана, Rust target architecture, CLI RFC и
> `migration/instance-data/README.md` замороженного источника. Перед
> изменениями выполни строгий preflight с явно заданными
> `MERIDIAN_KERNEL` и локальным `MERIDIAN_INSTANCE`; абсолютный путь
> источника не записывай в репозиторий.
>
> Реализуй ровно пакет `meridian-cli-migration`: два закрытых вида import,
> nested `migration plan|apply|verify|rollback`, app-owned
> `MigrationRepository`, SQLite transaction/checkpoint adapter и реальные
> доказательства 346/336/10, 61 applicability records, round trip,
> idempotency и rollback. Переиспользуй принятые typed
> `MigrationPlan`/`CanonicalExport` и production operations 7d; не
> создавай параллельную Value-реализацию.
>
> Сначала составь owner map и schema-migration/rollback design. Если
> append-only invariants, двухбазовая компенсация или точный CLI контракт
> требуют нарушения §5.22, остановись с
> `BLOCKED_FOR_ARCHITECT_DECISION`; не выбирай упрощение самостоятельно.
> Любое наблюдаемое отличие от принятого контракта воспроизведи, опиши в
> `COMPATIBILITY.md` и передай на решение архитектора.
>
> Не выполняй Git write-операций и не меняй посторонние файлы. Для первой
> передачи выполни targeted gates §5.22.8. Передай
> `READY_FOR_ARCHITECT_REVIEW` (не `ACCEPTED`), точный список файлов,
> owner map, schema migration, rollback/checkpoint model, production panic
> audit, результаты каждой команды и явно названные невыполненные полные
> gates.

### 5.22.10. Реализация и статус передачи (исполнитель, 2026-09-24)

Статус пакета: **`CHANGES_REQUESTED`** — вердикт архитектора по первой
передаче; выполнен корректирующий раунд 1, его передача —
**`READY_FOR_ARCHITECT_REVIEW`** (не `ACCEPTED`). Git write-операций
исполнитель не выполнял ни в одной передаче; полный отчёт с owner map,
списком файлов, результатами ворот и невыполненными полными воротами передан
архитектору.

- **Ядро** (`meridian-core/src/migration/{write_set,run,applicability}.rs`):
  `FrozenWriteSet` (только из принятого `MigrationPlan`, built-in цель —
  отказ до записи), `verify_read_back` (missing/extra/changed/duplicate),
  идентичность и решения запуска (`MigrationRunId`, `decide_apply`,
  `check_rollback_preconditions`), типизированная эквивалентность
  применимости над `ApplicabilityRecord`.
- **App** (`meridian-app/src/migration/`, `storage/migration_repository.rs`):
  порт `FrozenSource` (один source view), принятие bundle композицией
  production-операций 7d (`instance_data_migration::check_container`,
  `instance_canonical_export::check_pinned_container` против `PinnedPlan`),
  восстановление содержимого единиц из закреплённой ревизии без продуктовых
  констант, порт `MigrationRepository`, операции `plan`/`apply`/`verify`/
  `rollback`/`import_frozen`/`import_canonical`; база открывается только
  после совпадения подтверждения.
- **Storage** (`meridian-storage-sqlite`): схема версии 3 — новая атомарная
  ступень `2 → 3` заменяет пустой журнал версии 1 двумя отдельными
  append-only фактами: `migration_runs` (факт applied, без изменяемого
  столбца статуса) и `migration_rollbacks` (факт rolled-back, ссылка на
  run); статус запуска выводится из наличия факта отката. Checkpoint через
  SQLite online backup API, одна `IMMEDIATE`-транзакция на apply,
  проверенный restore с повторным открытием; read-only открытие для
  dry-run/verify.
- **CLI**: `import --kind frozen-instance|canonical-records`, вложенные
  `migration plan|apply|verify|rollback`, Git-адаптер `GitFrozenSource`,
  каталог checkpoint `<db>.checkpoints/` рядом с базой.
- **Доказательства на реальном bundle**: 346 = 336 + 10 + 0; 336 записей,
  равных принятому экспорту по девяти полям; 61 норма применимости,
  lost/added/changed/duplicate = 0; повтор — `already-applied`; round trip
  побайтно; `apply → rollback` возвращает канонический экспорт к pre-state,
  повторный rollback отклоняется.
- Два наблюдаемых решения внесены в `COMPATIBILITY.md` (типизированная
  проверка применимости; обобщённая адресация фрагментов вместо продуктовой
  таблицы контейнеров) и **приняты архитектором** в вердикте первой
  передачи; их наблюдаемые границы не менялись.

**Корректирующий раунд 1 (исполнитель, 2026-09-24).** Закрыты пункты
`CHANGES_REQUESTED`:

1. *Журнал rollback.* Restore checkpoint больше не заменяет историю:
   checkpoint предшествует запуску, поэтому rollback сначала копирует его
   (backup API) в staging-базу, доказывает, что её журнал равен живому
   журналу без собственного факта applied этого run (иначе
   `checkpoint-state-mismatch` до изменения живой базы), дописывает в неё
   дословно (включая `recorded_at`) исходную строку applied и отдельный
   факт `migration_rollbacks` одной транзакцией и только затем
   восстанавливает staging в живую базу. После повторного открытия БД
   проверяются количество и идентичность обеих записей; ошибка
   `RestoredButUnrecorded` стала непредставимой и удалена.
2. *Доказательство применимости fail-closed.* Frozen bundle без ровно одного
   принятого applicability register (ни одного либо несколько), с пустым
   register, без `$schema` либо без записи, минтящей target, отклоняется
   на `plan`, до `apply`/`verify`; пустые стороны сравнения никогда не
   эквивалентны (`ApplicabilityEquivalence::is_equivalent` требует
   `controlled > 0`), поэтому не дают `verified`.
3. *Порядок `import frozen-instance`.* Tool DB открывается и проверяется
   только после совпадения `--confirm`, так же как workspace DB: неверное
   подтверждение при отсутствующей или повреждённой tool DB — это
   `confirmation-mismatch`, код 1, без создания базы или checkpoint.

**Корректирующий раунд 2 (исполнитель, 2026-09-24).** Вердикт архитектора
по раунду 1 — `CHANGES_REQUESTED`; статус пакета остаётся
**`CHANGES_REQUESTED`**, передача раунда 2 — **`READY_FOR_ARCHITECT_REVIEW`**
(не `ACCEPTED`). Закрыты пункты:

1. *Самый новый run.* `rollback_migration` сверяет запрошенный run с самым
   новым записанным migration run независимо от наличия у него факта
   отката; helper «самого нового не откатанного run» удалён.
   `AlreadyRolledBack` самого запрошенного run по-прежнему решается первым.
   Сценарий `apply A → apply B → rollback B → rollback A` отклоняется как
   `NewerRun` (newer = B) без изменения record-state, канонического
   экспорта, обеих таблиц журнала и списка checkpoint.
2. *Внешние ключи staging.* Записывающее staging-соединение rollback явно
   включает и проверяет `PRAGMA foreign_keys = ON` до транзакции; после
   записи весь staging-файл проходит `PRAGMA foreign_key_check`, и любое
   нарушение отклоняет rollback (`checkpoint-state-mismatch`) до restore
   живой базы.

**Полный финальный рубеж и корректирующий раунд 3 (2026-09-24).**
Архитектурная проверка раунда 2 пройдена, кандидат был допущен к полному
финальному рубежу, но **рубеж не прошёл**. Архитектурный код rollback
принят; найдены три дефекта вне него:

1. *Test-boundary.* Файл тестового модуля
   `meridian-app/src/migration/operations/tests.rs`, подключённый через
   `#[cfg(test)] mod tests;`, не нёс собственного `#![cfg(test)]`, и оба
   структурных сканера границы `WorkspaceReader` (в `meridian-app` и
   `meridian-cli`) считали его fake production-реализацией.
2. *Baseline-snapshot.* `validate_reports_ok_on_this_kernel_with_zero_real_failures`
   ожидал `agent_instruction_identity_undeclared_other = 32`; чистый `HEAD`
   после интеграции документов Metis даёт 34 — дрейф существовал до
   реализации пакета 8.
3. *Harness-environment.* Legacy-сравнения Kernel validation передавали
   Node-стороне унаследованный `MERIDIAN_INSTANCE`, и продуктовые
   диагностики Экземпляра попадали только в одну сторону сравнения.

Начат корректирующий раунд 3: файл тестового модуля получил
`#![cfg(test)]` (сканеры не ослаблены); snapshot обновлён до 34 с
фиксацией происхождения дрейфа; харнесс получил единый helper
`kernelOnlyEnv` для Kernel-only дочерних процессов (копия окружения,
overrides, удалённый `MERIDIAN_INSTANCE`) для обеих сторон каждого
legacy-сравнения, а блок `meridian-cli-migration` по-прежнему читает frozen
source из `MERIDIAN_INSTANCE`, передаёт его бинарнику только через
`--source` и без него остаётся UNVERIFIED. Статус пакета остаётся
**`CHANGES_REQUESTED`**. Передача раунда 3 —
**`BLOCKED_FOR_ARCHITECT_DECISION`**: сканер
`meridian-cli` (`mechanical_integrity_boundary::production_text`)
распознаёт только внешний `#[cfg(test)]`, но не заголовочный
`#![cfg(test)]` (который уже распознаёт принятый сканер `meridian-app`),
поэтому назначенное исправление без изменения сканера не проходит его
структурный тест.

**Решение архитектора по блокировке раунда 3 (2026-09-24).**
`BLOCKED_FOR_ARCHITECT_DECISION` снят; разрешён вариант A — выравнивание
сканера `meridian-cli` с принятым правилом `meridian-app`. Раунд 3
продолжен: `production_text` в
`meridian-cli/src/commands/validate/mechanical_integrity_boundary.rs`
теперь опирается на один helper `production_part` — после пустых строк и
`//!` заголовочный `#![cfg(test)]` делает весь файл test-only, иначе
production заканчивается перед первой строкой-атрибутом внешнего
`#[cfg(test)]`; имя `tests.rs`, позднее либо строковое упоминание
атрибута и комментарий файл не освобождают (регрессионный тест
`only_an_explicit_test_only_module_file_is_exempt_from_the_production_scan`).
Статус пакета остаётся **`CHANGES_REQUESTED`**; передача продолженного
раунда 3 — **`READY_FOR_ARCHITECT_REVIEW`** (не `ACCEPTED`).

### 5.22.11. Итоговый вердикт архитектора и интеграция

Итоговый вердикт: **`ACCEPTED`**.

Независимое ревью первой передачи и трёх корректирующих раундов подтвердило
закрытые import kinds, чистые границы core/app/SQLite/CLI, mutation только
после явного подтверждения, append-only журнал applied/rollback, проверяемый
checkpoint restore, fail-closed applicability proof и отсутствие
эксплуатационного чтения через `MERIDIAN_INSTANCE`. Два наблюдаемых
Rust-native решения `COMPATIBILITY.md` приняты; иных необъяснённых
расхождений не осталось.

Полный финальный gate выполнен на неизменённом кандидате с fingerprint
`dc6f4e2b7281c34f5200a23f25575dc5158f109a3033e3a1c28975b5a2615934`:
workspace Rust tests — 1070 passed, 0 failed, 2 ignored; оба ignored-теста
реального frozen bundle запущены отдельно — 2 passed, 0 failed; conformance
harness — 137 passed, 0 failed; `kernel-validate.test.mjs` — 293 passed,
0 failed, 0 skipped. Format, workspace build, workspace clippy с
`-D warnings`, rustdoc с `-D warnings`, оба diff-check и строгий preflight
также прошли. Начальный и конечный HEAD совпали:
`1757ec29777f5454fe5f90c2a0e7618a62d45c79`; индекс до интеграции был чист.

Пакет локально интегрируется отдельным package commit и отдельным `--no-ff`
merge-коммитом без публикации. Пакет 9 этим решением не начинается: для
`rust-business-contract-qualification` ещё нет отдельной спецификации и
назначения исполнения.

## 5.23. Пакет 9 `rust-business-contract-qualification` (задание, 2026-09-24)

### 5.23.1. Статус и условие старта

Статус: **`SPECIFIED_NOT_STARTED`**. Документальная спецификация не является
началом исполнения пакета (§8). Целевой статус первой передачи исполнителя —
`READY_FOR_ARCHITECT_REVIEW`, не `ACCEPTED`; исполнитель не выполняет Git
write-операций.

Предшествующий пакет `meridian-cli-migration` принят и локально интегрирован:
package commit `6a68620ff7af4efde492dab897d566e7763f410b` достижим из `dev` через отдельный
`--no-ff` merge-коммит `e85a73fec4bb7eb6b0ac0497bc043a9d2c445c03`.
Пакет 9 вправе начаться отдельным исполнением только от этого или более нового
чистого `dev`, в котором эти коммиты остаются достижимы.

### 5.23.2. Результат и граница пакета

Пакет даёт один закрытый ответ на вопрос: вся ли перенесённая в Rust механика
Meridian сохраняет объявленные бизнес- и публичные контракты, а каждое
наблюдаемое отличие от Node.js является явно принятым Rust-native усилением с
исполняемым доказательством.

Обязательный результат:

1. новый отчёт
   `governance/audits/meridian-rust-business-contract-qualification.md` с
   итоговым вердиктом `QUALIFIED` либо `NOT_QUALIFIED`;
2. закрытая матрица всей реализованной поверхности: source formats, все
   семейства `validate`, `resolve`, `init`, `doctor`, `export`, `import` и
   `migration plan|apply|verify|rollback`;
3. для каждой строки матрицы — канонический контракт, Node-эталон либо
   основание отсутствия прямого Node-аналога, production-маршрут Rust,
   positive/negative executable evidence, ожидаемое отношение результатов и
   фактический результат;
4. полная сверка раздела «Намеренные Rust-native усиления» в
   `COMPATIBILITY.md`: каждая наблюдаемая дельта имеет ровно одну
   классификацию, бизнес-обоснование, границу наблюдаемости и положительный с
   отрицательным тесты; устаревших, дублирующих и недоказанных записей нет;
5. полный архитектурный аудит production Rust по
   `rust-migration-quality.md`: crate ownership, порты, четыре уровня
   проверки, строгие типы, отсутствие внешнего I/O в `meridian-core`,
   отсутствие concrete adapters в `meridian-app`, production panic audit;
6. одновременное исполнение полного набора §6.1–§6.5a, включая реальные
   frozen-bundle сценарии, offline-поведение, стабильные exit/stdout/stderr и
   NoOp/Recording event-sink эквивалентность.

Отчёт не вправе выводить полноту из зелёного агрегатного счётчика. Каждая
строка должна ссылаться на фактически исполняемый тест того же production
алгоритма; projection-only сравнение или повторно написанный алгоритм
доказательством не являются. Если существующее доказательство не покрывает
контракт, исполнитель добавляет минимальный real-path case. Если обнаружен
дефект реализации, разрешено минимальное исправление в правильном крейте и
его regression test; новая функциональность не добавляется.

### 5.23.3. Классификация результатов

Каждая строка закрытой матрицы получает ровно один статус:

- `CONFORMANT` — сохраняемый внешний или бизнес-контракт совпал;
- `ACCEPTED_RUST_NATIVE` — отличие уже отражено в `COMPATIBILITY.md`, не
  теряет бизнес-ценность и доказано matched positive/negative cases;
- `RUST_ONLY_CONTRACT` — у принятой Rust/SQLite/CLI поверхности нет прямого
  Node-аналога, но она полностью определяется действующим ADR, технической
  спецификацией или нормативным контрактом и доказана executable tests;
- `GAP` — контракт, доказательство либо классификация отсутствуют или
  противоречат друг другу.

Наличие хотя бы одного `GAP`, необъяснённой дельты, непрошедшего обязательного
ворота или непроверенного обязательного real-bundle сценария принудительно
даёт `NOT_QUALIFIED`. Исполнитель не вправе сам принять новую наблюдаемую
дельту: он передаёт минимальное воспроизведение как
`BLOCKED_FOR_ARCHITECT_DECISION`. Уже принятые записи `COMPATIBILITY.md` не
переоткрываются без нового факта, но их ссылки и доказательства проверяются.

### 5.23.4. Проверяемая полнота матрицы

Инвентаризация строится от production entry points и канонических контрактов,
а не только от уже существующих тестов. Отчёт обязан отдельно закрыть:

- все команды и коды завершения `meridian-cli`;
- все семейства, которые участвуют в production `validate`;
- полный resolver request/result contract;
- все роли, schema migrations, transaction/foreign-key и routing boundaries
  SQLite;
- canonical export, оба закрытых import kind и пять migration operations;
- каждую строку реестра Rust-native усилений `COMPATIBILITY.md`;
- все требования §6.3–§6.5a, включая те, у которых нет Node-аналога.

Для каждого пункта указываются точные пути и имена тестов. Обобщённая ссылка
«покрыто workspace tests» не принимается. Один тест может доказывать несколько
строк только когда отчёт явно называет проверяемое утверждение для каждой из
них. Счётчики полного прогона фиксируются отдельно и не заменяют матрицу.

### 5.23.5. Разрешённые изменения

Ожидаемый документальный минимум:

- `governance/audits/meridian-rust-business-contract-qualification.md`;
- `governance/plans/meridian-rust-migration-program-plan.md` — только запись
  фактического раунда и синхронизация статуса;
- `COMPATIBILITY.md` — только исправление неполной, устаревшей или неточной
  классификации, найденной квалификацией.

Тесты и production-файлы изменяются только при доказанном пробеле. Любое
такое изменение перечисляется отдельно как `gap -> owner -> fix -> regression
evidence`; оно делает ранее полученные полные gate-результаты устаревшими и
требует одного повторного полного прогона на финальном кандидате.

### 5.23.6. Явно вне объёма

- выпуск, версия, `CHANGELOG`, release/promotion ветви и пакет 10;
- удаление Node.js, архивирование или изменение замороженного Instance;
- Metis, Concord, network API, PostgreSQL, daemon, GUI и новые команды;
- изменение принятых бизнес-контрактов ради зелёного conformance;
- принятие новой Rust-native дельты исполнителем;
- рефакторинг без конкретного квалификационного пробела;
- Git branch/switch/add/commit/merge/rebase/reset/stash/tag/push исполнителем.

### 5.23.7. Ворота исполнения

Пакет 9 сам является полным финальным квалификационным рубежом, поэтому его
первая передача выполняет полный набор, а не сокращённый корректирующий gate:

```bash
node scripts/preflight.mjs
MERIDIAN_KERNEL=/home/krmiftakhov/PersonalProjects/meridian MERIDIAN_INSTANCE=<local-frozen-source> node scripts/preflight.mjs --require-instance
cargo fmt --all -- --check
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo test --workspace
MERIDIAN_KERNEL=/home/krmiftakhov/PersonalProjects/meridian MERIDIAN_INSTANCE=<local-frozen-source> cargo test -p meridian-cli --test binary_runs migration_real_bundle -- --ignored
MERIDIAN_KERNEL=/home/krmiftakhov/PersonalProjects/meridian MERIDIAN_INSTANCE=<local-frozen-source> node --test test/conformance-harness.test.mjs
node test/kernel-validate.test.mjs
env -u MERIDIAN_INSTANCE node scripts/kernel-validate.mjs
git diff --check
git diff --cached --check
git diff --cached --name-only
```

`<local-frozen-source>` — локальная проводка, не литерал для коммита. Полный
conformance запускается с тем же frozen source, но Rust binary получает путь
только через явный `--source`; его штатное окружение остаётся без
`MERIDIAN_INSTANCE`. Два ignored real-bundle tests обязаны фактически
выполниться и дать `2 passed`, а не остаться пропущенными внутри workspace
прогона.

Если Cargo/Node child process получает sandbox `EPERM`, повторяется ровно та
же команда через разрешённый путь. Инфраструктурный запрет не становится
кодовым вердиктом. После любого исправления полный набор повторяется один раз
на окончательном кандидате; в промежуточном корректирующем раунде действуют
суженные правила `AGENTS.md` §9.

### 5.23.8. Критерии передачи

Передача исполнителя содержит:

- статус `READY_FOR_ARCHITECT_REVIEW`, `BLOCKED_FOR_ARCHITECT_DECISION` либо
  `NOT_QUALIFIED`, но никогда самоназначенный `ACCEPTED`;
- точный HEAD до/после и fingerprint всех изменённых файлов кандидата;
- итог матрицы по каждому из четырёх статусов §5.23.3 и список всех `GAP`;
- точные новые/изменённые строки `COMPATIBILITY.md`, если они есть;
- production owner map и production panic audit;
- результаты каждой команды §5.23.7 с кодом и счётчиками;
- точный список изменённых файлов и отдельно невыполненные проверки;
- подтверждение отсутствия Git write-операций и пустого индекса.

Пакет может получить `ACCEPTED` только после независимого чтения отчёта,
фактического diff и executable evidence архитектором. Даже `QUALIFIED` в
отчёте исполнителя является заявленным результатом, а не итоговым вердиктом.

### 5.23.9. Готовое задание исполнителю

> Работай над пакетом `rust-business-contract-qualification` строго по §5.23
> `governance/plans/meridian-rust-migration-program-plan.md`.
>
> До изменений выполни оба preflight из §5.23.7; подтверди чистый базовый
> `dev`, достижимость package commit `6a68620ff7af4efde492dab897d566e7763f410b`
> через merge `e85a73fec4bb7eb6b0ac0497bc043a9d2c445c03` и отсутствие чужого dirty
> diff. Локальный путь frozen source используй только как environment wiring,
> никогда не записывай его в файлы.
>
> Построй закрытую квалификационную матрицу от production entry points и
> канонических контрактов по §5.23.2–§5.23.4. Создай
> `governance/audits/meridian-rust-business-contract-qualification.md`.
> Для каждой строки укажи канон, Node reference либо основание Rust-only,
> Rust production route, positive/negative executable evidence, ожидаемое
> отношение и фактический статус. Проверь каждую запись Rust-native усиления
> в `COMPATIBILITY.md`; зелёный общий счётчик не заменяет эту сверку.
>
> Не добавляй функциональность. Исправляй код или тесты только при конкретном
> квалификационном пробеле и фиксируй `gap -> owner -> fix -> evidence`. Новую
> наблюдаемую дельту не принимай: остановись с минимальным воспроизведением и
> `BLOCKED_FOR_ARCHITECT_DECISION`.
>
> Выполни полный набор §5.23.7. Не выполняй branch/switch/add/commit/merge/
> rebase/reset/stash/tag/push. Передай статус не выше
> `READY_FOR_ARCHITECT_REVIEW`, точные файлы, матрицу, panic/owner audits,
> коды и счётчики всех ворот, fingerprint кандидата, пустой индекс и все
> отклонения.

### 5.23.10. Передачи исполнителя (2026-09-25)

**Первая передача.**

Заявленный результат квалификации — **`NOT_QUALIFIED`**; статус передачи —
**`BLOCKED_FOR_ARCHITECT_DECISION`** (не `ACCEPTED`). Git write-операций
исполнитель не выполнял; индекс пуст. Отчёт —
`governance/audits/meridian-rust-business-contract-qualification.md`:
закрытая матрица из 131 строки (50 `CONFORMANT`, 28
`ACCEPTED_RUST_NATIVE`, 42 `RUST_ONLY_CONTRACT`, 11 `GAP`), карта
владельцев, четыре уровня, production panic audit и построчный аудит
`COMPATIBILITY.md`.

Закрыто минимальными исправлениями (`gap -> owner -> fix -> evidence`,
отчёт §9.1):

1. авария `meridian validate` (код 101) на Kernel-документе с пустым
   Front Matter — новый единственный владелец
   `meridian_app::source_format::front_matter_block`;
2. общий проход `$schema` терял путь схемы в тексте отказа строгого слоя и
   не нормализовал путь схемы, как эталон;
3. строгий слой YAML отдавал вход библиотеке раньше собственного lint и
   reader (RFC D-A), подменяя тексты отказа эталона;
4. отрицательное Node/Rust доказательство семейства `rule-resolution`;
5. межъязыковое доказательство fail-closed ветвей resolver (отдельный
   корпус `rule-resolution-fail-closed-corpus.json`);
6. доказательство работы без сети для `init`/`doctor`/`validate`/
   `resolve`/`export`;
7. в реестр `COMPATIBILITY.md` внесено уже принятое (§5.16.3 п. 9) отличие
   `stack-profiles` неверного типа;
8. шаблонные ссылки строк реестра заменены точными именами тестов.

Открыто и передано на решение архитектора (отчёт §9.2): поверхность
Instance у `kernel-validate.mjs`/`validate-and-log.mjs` без Rust-аналога и
классификации; класс хвоста сообщения о невалидном JSON; текст
`schema` для несуществующей схемы; вердикт по тексту, который подмножество
YAML эталона принимает, хотя это не YAML; текст `instruction-topics`
неверного типа; отсутствие проверки Git в `doctor` при заявленной
эквивалентности `preflight`; Node-поверхности `validate-branch-name`,
`hooks/pre-push` и CI без пакета.

Коды и счётчики полного набора §5.23.7 — в передаче исполнителя. (Условие
интеграции первой передачи — правка литерала
`agent_instruction_identity_undeclared_other` после индексации отчёта —
снято корректирующим раундом, см. ниже.)

**Корректирующий раунд (вердикт архитектора `CHANGES_REQUESTED`).**
Заявленный результат и статус передачи — **`NOT_QUALIFIED`** (не
`ACCEPTED`). Git write-операций исполнитель не выполнял; индекс пуст.
Матрица пересчитана: 134 строки (53 `CONFORMANT`, 36
`ACCEPTED_RUST_NATIVE`, 42 `RUST_ONLY_CONTRACT`, 3 `GAP`).

1. По решениям архитектора GAP-10 (хвост сообщения о невалидном JSON —
   класс), GAP-11 (текст отсутствующей схемы называет только Kernel),
   GAP-12 (fail-closed отказ от YAML, который Node принимает с потерей
   данных) и GAP-13 (`instruction-topics` неверного типа) классифицированы
   `ACCEPTED_RUST_NATIVE`: четыре строки `COMPATIBILITY.md` и раздел
   харнесса «accepted-rust-native» с точной формой каждого расхождения;
   `exactTextNotRequired` удалён.
2. GAP-14 исправлен: `meridian doctor` только чтением проверяет, что
   Kernel под Git (критерий `preflight`), с новым полем `kernel_git`;
   `validator present` не переносится.
3. GAP-15 снят: `validate-branch-name`, `hooks/pre-push` и CI — переходное
   governance/tooling вне квалифицируемой Rust-поверхности (отчёт §8.5,
   решение D-F); удаление Node — отдельное решение после выпуска.
4. GAP-02 доисправлен: путь схемы общего прохода `$schema` совпадает с
   `path.resolve` Node и для относительного `--kernel`; доказано
   production run.
5. Счётчик `agent_instruction_identity_undeclared_other` выводится тестом
   из отслеживаемого состояния, поэтому кандидат корректен и до, и после
   индексации отчёта; остальные snapshot-счётчики не ослаблены.
6. GAP-09 остаётся открытым: mapping каждой Instance-проверки Node (отчёт
   §9.3) даёт непустую категорию «действительно утраченная бизнес-проверка»
   — продуктовые литералы `kernel-purity`, форма payload импортированных
   продуктовых записей, контекст Instance для скиллов, внешние
   зависимости, сверка инвентаря, объявления stack-profile, ссылочная
   целостность и полнота приёма инструкций, журнал метрик. Недостающий
   контракт — проверка продуктового состояния рабочей базы только чтением
   (и запись метрик) без `MERIDIAN_INSTANCE`; владельцы — `meridian-app`
   над `RecordRepository` и `meridian-cli` `commands::validate`.

Раунд выполнил только целевой рубеж `AGENTS.md` §9; полные
`conformance-harness`, `kernel-validate` и остальной набор §5.23.7 —
один раз на финальном кандидате после архитектурного одобрения.

**Второй корректирующий раунд (вердикт архитектора `CHANGES_REQUESTED`,
узкий).** Вывод GAP-09 подтверждён архитектором; итог и статус —
**`NOT_QUALIFIED`**, GAP-09 открыт и в этом раунде не реализуется. Для
COMPAT-29 добавлено matched Node/Rust negative evidence каждого оставшегося
самостоятельного владельца парсера (`instruction_source_registry`,
`controlled_rule_intake`, `existing_project_compatibility_mode`,
`task_pattern_registry`, `task_specification`) изолированными копиями;
класс доказан для всех девяти владельцев, сужение не потребовалось. Точный
счёт GAP-10: 10 пар `FAIL` с расхождением только в хвосте парсера плюс две
объявленные Rust-only строки каскада принятой границы «каталог публикуется
только целиком»; прежнее «шесть» было ошибкой (фактически пять в
combined-case). Матрица не изменилась: 134 строки, 3 `GAP`.

### 5.23.11. Корректирующий пакет GAP-09 `rust-workspace-state-validation` (задание, 2026-09-25)

#### Статус и условие старта

Статус: **`SPECIFIED_NOT_STARTED`**. Это отдельное исполнение внутри пакета 9,
а не пакет 10 и не продолжение второго корректирующего раунда. Документальная
спецификация не начинает реализацию. Первая передача исполнителя имеет статус
не выше `READY_FOR_ARCHITECT_REVIEW`; исполнитель не выполняет Git
write-операций.

База исполнения — чистый `dev` с package commit
`964ade898e1c3ebb4826229ad2488287d0a9c75f` и отдельным merge-коммитом
`815ab80d94d4fcff156f6c149686d60c0272f560`. Пакет закрывает только GAP-09
квалификационного отчёта. Пакет 10 `meridian-rust-release` остаётся закрытым
до реализации этого пакета, повторной квалификации всей матрицы пакета 9,
вердикта `QUALIFIED`, независимой приёмки и локальной интеграции.

#### Результат и граница

Результат — штатный production-маршрут проверки продуктового состояния из
рабочей SQLite-базы без эксплуатационного чтения `MERIDIAN_INSTANCE`:

```text
explicit --workspace-db + Kernel/workspace/Git inputs
  -> meridian-cli adapters and composition
  -> meridian-app workspace-state validation operation
  -> RecordRepository + WorkspaceReader + GitInspector ports
  -> closed transport DTOs and schema gates
  -> strict meridian-core domain types and pure checks
  -> typed diagnostics and optional metrics record
  -> one meridian-cli presentation boundary
```

Пакет обязан закрыть все строки категории 3 mapping §9.3 отчёта:

1. M-02/M-03 — обязательная типизированная продуктовая запись и проверка
   product-specific литералов/паттернов `kernel-purity` по фактическому
   Kernel; некомпилируемый паттерн даёт явный `FAIL`, а не пропуск.
2. M-05b — закрытый registry `record_type -> payload contract`; полный
   transport/schema/domain проход применяется и к обоим import kind до
   записи, и к уже сохранённым current records при `validate`. Неизвестный
   product record type, для которого объявлен обязательный контракт, и
   schema-invalid payload fail-closed; `serde_json::Value` не проходит за
   transport boundary.
3. M-06 — требования `requires_instance_context` скиллов Kernel сверяются с
   типизированным контекстом рабочей базы.
4. M-07 — внешние зависимости разбираются в закрытый предметный тип;
   malformed запись даёт `FAIL`, непинованная допустимая зависимость сохраняет
   канонический уровень `WARN`.
5. M-08 — записи inventory сверяются с фактическими локальными репозиториями,
   revision/ref/dirty и TTL `last_verified`; недоступность остаётся явным
   `UNVERIFIED`, а не успехом. Один адаптер Git получает входы явно от CLI.
6. M-09 — профиль каждой repository identity принадлежит принятому пулу и
   подтверждается объявленным manifest через `WorkspaceReader`; `universal`
   не становится профилем.
7. M-12 — каждая instruction-intake record ссылается на существующую
   repository identity; ссылочная целостность проверяется чистой предметной
   операцией над принятыми типами.
8. M-15 — полнота приёма сверяется с фактическим деревом репозитория,
   tracked/untracked/ignored состоянием и размеченными regions через
   существующий `source_format::regions`; отсутствующее покрытие fail-closed,
   недоступный репозиторий остаётся `UNVERIFIED`.
9. M-18 — `--log-metrics` после сформированного результата добавляет одну
   `gate-run-observation` в рабочую базу. Запись является opt-in эффектом,
   никогда не меняет предметный verdict/stdout/stderr/exit code исходной
   проверки; ошибка записи сообщается отдельно и не маскируется как успешная
   запись.

M-01, M-04, M-05a, M-10/M-11, M-13/M-14 и M-16/M-17 не переносятся повторно:
их категория 1/2 и доказательство остаются границей §9.3. Исполнитель не
возвращает файловую идентичность старого Instance и не создаёт второй
универсальный validator.

#### Точный контракт CLI

Пакет расширяет существующую команду, не добавляя новую:

```text
meridian validate --kernel <path> [--workspace-db <path>] [--log-metrics] [--format human|json]
```

1. Без `--workspace-db` сохраняется принятый Kernel-only контракт, включая
   явные `WARN ... no Instance root ... not checked`; команда не ищет базу по
   cwd, окружению или соседнему файлу.
2. С `--workspace-db` база открывается как `DatabaseRole::Workspace` с
   редакцией из `<kernel>/VERSION`; missing/corrupt/wrong-role/wrong-edition
   даёт код 3, пустой stdout и стабильную ошибку stderr.
3. При успешно открытой базе Kernel-only заглушки для M-02…M-18 заменяются
   фактическими diagnostics production-операции; одни и те же проверки не
   исполняются и не выводятся дважды.
4. `--log-metrics` без `--workspace-db` — usage error, код 2. Без флага
   `validate` остаётся read-only, включая отрицательный предметный результат.
5. `--log-metrics` пишет observation после вычисления полного результата и
   фиксирует как минимум время через порт `Clock`, редакцию Kernel, проверяемую
   локальную revision при её наличии, exit code, счётчики FAIL/WARN/INFO и
   ограниченные стабильные списки сообщений. Idempotency key исключает вторую
   запись при повторе того же логического запуска.
6. Human/JSON envelope, разделение stdout/stderr и коды 0/1/2/3 сохраняются.
   Расширение JSON выполняется только добавлением явно названного объекта
   `workspace_state` и, при opt-in, `metrics`; существующие поля и их смысл не
   меняются.

#### Обязательная архитектура

- `meridian-core` владеет чистыми строгими типами продуктовой конфигурации,
  внешних зависимостей, repository inventory, profile declaration,
  instruction-intake links/completeness и наблюдения gate; core не знает
  JSON/YAML, SQLite, путей баз, Git, файлов, env, времени или CLI.
- `meridian-app` владеет закрытыми DTO, transport/schema/domain границей,
  оркестрацией одной операции над `RecordRepository`, `WorkspaceReader`,
  `GitInspector` и `Clock`, а также построением `PutRecordRequest` для
  observation. Concrete adapters и форматирование в app запрещены.
- Существующий `RecordRepository` переиспользуется. Новый query-порт допустим
  только если `export_all` объективно не выражает требуемый контракт; он не
  должен раскрывать SQL, таблицы или роль физической базы в core.
- `meridian-storage-sqlite` меняется только для adapter-neutral расширения
  принятого порта либо необходимого атомарного инварианта. SQL и транзакции не
  переходят в app/CLI.
- `meridian-cli` открывает рабочую базу, реализует реальные filesystem/Git/
  Clock adapters, передаёт их одной app-операции и форматирует typed result.
  Предметные проверки в CLI запрещены.
- Проверка import payload выполняется тем же production validator/registry,
  что чтение рабочей базы; параллельный упрощённый алгоритм для теста или
  импорта запрещён.
- Диагностики детерминированно сортируются по семантическому ключу, а не по
  hash iteration или порядку строк SQLite.

#### Критерии приёмки

1. Все десять category-3 пунктов §9.3 имеют positive и negative executable
   evidence того же production route; строки VAL-30/NODE-02/NODE-03 матрицы
   больше не `GAP`.
2. Реальный импортированный bundle проверяется через `--workspace-db` без
   `MERIDIAN_INSTANCE`; каждый category-3 verdict совпадает с замороженным
   эталоном. Общие счётчики вправе отличаться только на поимённо исключённые
   category-1/2 строки §9.3 либо отдельно классифицированное архитектором
   Rust-native отличие. Сравнение не переписывает алгоритм проекцией.
3. Каждый поддержанный payload принимается после transport/schema/domain
   прохода; мутация одного обязательного поля каждого семейства отвергается
   до записи обоими import kind и обнаруживается в уже сохранённой базе.
4. Missing/corrupt/wrong-role/wrong-edition workspace DB и port I/O failures
   fail-closed; отсутствие необязательного локального репозитория даёт ровно
   объявленный `UNVERIFIED`, не зелёный факт.
5. Kernel-only `validate` байт-в-байт сохраняет прежний stdout/stderr и код;
   обычный DB-backed `validate` не изменяет export digest, revisions,
   migration journal или observation count.
6. Один `--log-metrics` добавляет ровно одну observation после положительного
   и после отрицательного валидного результата; повтор с тем же idempotency
   key не дублирует запись. Ошибка записи не меняет validation verdict и явно
   отражается в `metrics` outcome.
7. `NoOpEventSink` и `RecordingEventSink` дают одинаковые stdout/stderr/exit
   code и одинаковое состояние базы для одинакового режима логирования.
8. Offline-тест блокирует сеть; все новые DB-backed маршруты успешно работают
   на локальных входах.
9. В core нет I/O/serde/SQLite/Git/process/env/clock; в app нет concrete
   adapters; production panic audit не находит нового достижимого panic.
10. Квалификационный отчёт и `COMPATIBILITY.md` обновляются только после
    реализации и executable evidence; сам пакет не объявляет `QUALIFIED` и не
    открывает release.

#### Явно вне объёма

- пакет 10, версия, `CHANGELOG`, release/promotion ветви, теги и публикация;
- удаление Node.js, изменение/архивирование замороженного Instance;
- Metis, Concord, network API, PostgreSQL, daemon, GUI;
- восстановление переходных category-1 проверок Git/раскладки Instance;
- чтение `MERIDIAN_INSTANCE` штатным Rust CLI;
- произвольный CRUD, универсальный schema registry для неизвестных будущих
  record type или новая команда;
- принятие новой наблюдаемой дельты исполнителем;
- Git branch/switch/add/commit/merge/rebase/reset/stash/tag/push исполнителем.

#### Ворота исполнения

Первый раунд и каждый `CHANGES_REQUESTED` выполняют только целевой набор:

```bash
node scripts/preflight.mjs
MERIDIAN_KERNEL=/home/krmiftakhov/PersonalProjects/meridian MERIDIAN_INSTANCE=<local-frozen-source> node scripts/preflight.mjs --require-instance
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p meridian-core workspace_state
cargo test -p meridian-app workspace_state
cargo test -p meridian-cli workspace_state
cargo test -p meridian-cli --test binary_runs workspace_state
node --check test/conformance-harness.test.mjs
git diff --check
git diff --cached --check
git diff --cached --name-only
```

Если фактические имена тестовых фильтров отличаются, исполнитель перечисляет
точные команды и доказывает, что они выбирают все новые positive/negative
cases, а не ноль тестов. Полные `cargo test --workspace`, conformance и
`kernel-validate.test.mjs` не повторяются в промежуточном раунде. После
архитектурного одобрения финального кандидата один раз выполняется весь набор
§5.23.7, включая real-bundle tests; дополнительно real Node/Rust DB-backed
validate case обязан доказать критерий 2 выше.

Sandbox `EPERM` повторяется той же командой через разрешённый путь и не
становится кодовым вердиктом.

#### Готовое задание исполнителю

> Реализуй корректирующий пакет `rust-workspace-state-validation` строго по
> §5.23.11 активного плана. Это отдельное исполнение внутри пакета 9, которое
> закрывает только GAP-09; release, Node removal и новые функции вне mapping
> §9.3 запрещены.
>
> До изменений выполни оба preflight, подтверди чистый `dev`, достижимость
> `964ade898e1c3ebb4826229ad2488287d0a9c75f` через merge
> `815ab80d94d4fcff156f6c149686d60c0272f560` и пустой индекс. Локальный frozen
> source используй только как проводку команд и real-bundle evidence; не
> записывай его путь и не добавляй эксплуатационное чтение
> `MERIDIAN_INSTANCE`.
>
> Построй один typed production route
> `CLI adapters -> app operation -> RecordRepository/WorkspaceReader/
> GitInspector/Clock -> closed DTO/schema/domain -> core checks -> typed
> diagnostics`. Закрой M-02/M-03, M-05b, M-06…M-09, M-12, M-15 и M-18; не
> возвращай category-1 механику старого Instance. Переиспользуй существующие
> порты и `source_format::regions`; не оставляй предметные алгоритмы в CLI и
> `Value` после границы.
>
> Сохрани Kernel-only поведение. Добавь явный `--workspace-db`; DB-backed
> validate по умолчанию read-only. `--log-metrics` требует базу и является
> единственным opt-in эффектом, не меняющим validation verdict. Import обоих
> kind и validate обязаны использовать один payload validator.
>
> Выполни целевые ворота этого раздела. Любую новую наблюдаемую дельту не
> принимай самостоятельно: передай минимальное воспроизведение как
> `BLOCKED_FOR_ARCHITECT_DECISION`. Не выполняй Git write-операций. Передай
> статус не выше `READY_FOR_ARCHITECT_REVIEW`, exact HEAD/fingerprint,
> изменённые файлы, owner map, mapping каждого M-пункта на production route и
> positive/negative test, коды/счётчики ворот, невыполненные проверки и
> подтверждение пустого индекса.

#### Первая передача исполнителя (2026-09-25)

Статус передачи — **`BLOCKED_FOR_ARCHITECT_DECISION`** (не `ACCEPTED`):
реализация и executable evidence готовы, но три наблюдаемых отличия от
эталона исполнитель не вправе принять сам. Git write-операций исполнитель
не выполнял; индекс пуст. База — `dev` `d00d7a52f035093934a92457f936706065bf5c0f`.

Реализовано по «Точному контракту CLI» и «Обязательной архитектуре»:
`meridian validate --kernel <path> [--workspace-db <path>] [--log-metrics]`;
`meridian-core::workspace_state` (строгие типы и чистые проверки M-02/M-03,
M-05b, M-07, M-08, M-09, M-12, M-15, M-18), `meridian-app::workspace_state`
(закрытые DTO, единый `PayloadRegistry`, одна операция `validate` над
`RecordRepository`/`WorkspaceReader`/`RepositoryAccess`/`Clock`,
`observation_request`), адаптеры `meridian-cli` `RealRepositoryAccess` и
`SystemClock`. Тот же `PayloadRegistry` проверяет оба import kind до записи.
Mapping каждого M-пункта на production route и positive/negative тесты —
отчёт квалификации §9.5.

На реальном bundle без `MERIDIAN_INSTANCE` все 336 записей проходят реестр
контрактов, и category-3 вердикты DB-backed `validate` совпадают с
Node-эталоном на Instance в закреплённой ревизии источника (харнесс,
«workspace-state: …»), кроме объявленного исключения D1.

Решения архитектора (отчёт §9.5): **D1** — строки удержанного
(`retained-transitional`) репозитория инвентаря есть только у эталона;
**D2** — тексты, где эталон называет Instance или файл реестра, и новое
семейство `payload-contract:`; **D3** — mapping §9.3 не назвал три
проверки `instruction-intake` эталона (пул тем, упаковка `skill-package`,
parentage `adopt-edition`). Строки VAL-30/NODE-02/NODE-03 остаются `GAP`
до этих решений и финального рубежа §5.23.7.

Синтетический fixture пакета 8 переведён с неконтрактного типа
`inventory-item` на `versioning-conformance-case` и перегенерирован штатным
`generate.mjs` (добавлен вариант `unknown-record-type`); ревизия источника
fixture не изменилась.

#### Корректирующий раунд исполнителя (2026-09-25)

Статус передачи — **`READY_FOR_ARCHITECT_REVIEW`** (не `ACCEPTED`, не
`QUALIFIED`). Исполнено по корректирующей инструкции архитектора без Git
write-операций; индекс пуст. База — тот же `dev`
`d00d7a52f035093934a92457f936706065bf5c0f`.

1. Fail-open `repository_tree` удалён: сбой `untracked_files` или
   `ignored_among` передаётся типизированным
   `meridian_core::workspace_state::intake::TreeFact::Unavailable` до
   `check_completeness`, даёт явный `WARN … UNVERIFIED` и никогда не
   увеличивает `intake_repositories_complete` (нечитаемый контейнер тоже).
2. Три проверки D3 (пул тем, упаковка `skill-package`, parentage
   `adopt-edition`) реализованы тем же production-маршрутом: чистые типы и
   решения — `meridian-core` (`workspace_state::{intake, packaging,
   parentage}`), оркестрация — `meridian-app` (`workspace_state::intake`)
   над расширенным портом `RepositoryAccess` (`has_revision`,
   `file_at(RevisionSelector)`), Git — только адаптер `meridian-cli`.
   D3 больше не открытое решение.
3. Критерий приёмки 3 закрыт таблицей
   `meridian-cli/tests/fixtures/payload-matrix/families.json` и отдельным
   синтетическим замороженным источником `frozen-instance/payload-matrix`
   (тот же `generate.mjs`): все 32 `record_type` приняты; мутация каждого из
   15 schema-backed семейств отвергнута до записи `import --kind
   frozen-instance` и `import --kind canonical-records` и обнаружена в уже
   сохранённой базе.
4. **D1** зафиксирован как `ACCEPTED_MIGRATION_BOUNDARY`: десять
   `retained-transitional` единиц пакета 8 не изменены; из сравнения
   исключаются ровно строки `inventory-git`/`stack-profile` удержанного
   репозитория инвентаря, и харнесс требует, чтобы Rust такой репозиторий не
   называл. **D2** зафиксирован в `COMPATIBILITY.md` с бизнес-эффектом и
   matched evidence; отличие п. 1 — отдельной строкой.

Mapping и payload matrix — отчёт квалификации §9.5. Строки
VAL-30/NODE-02/NODE-03 остаются `GAP` до исполнения новых cases на
финальном рубеже §5.23.7; итог квалификации — `NOT_QUALIFIED`.

#### Финальный рубеж §5.23.7 (2026-09-25)

Архитектурный кандидат одобрен (fingerprint рабочего дерева
`bd1857e2af32275902e9e22ebf566e9557c50c86afee6dd3b7204a8dfbe7aea4` над
`dev` `d00d7a52f035093934a92457f936706065bf5c0f`). Первый прогон полного
рубежа **не пройден**: `cargo test --workspace` — код 101, один отказ
`meridian-cli/tests/binary_runs.rs::validate_reports_ok_on_this_kernel_with_zero_real_failures`
(`schema_validated` = 14 при закреплённом 13). Причина — устаревший
закреплённый счётчик: второй синтетический реестр применимости
(источник `payload-matrix`) объявляет `$schema`; эталон Node даёт тот же
`schema: 14/14`. Остальные ворота прошли (оба ignored real-bundle теста —
2 passed; харнесс — 152 passed; `kernel-validate.test.mjs` — 293 passed).
Исправлен только счётчик 13/13 → 14/14 в тесте, `COMPATIBILITY.md` и
отчёте квалификации; полный рубеж повторяется на новом fingerprint. Статус
пакета — не принят; итог квалификации — `NOT_QUALIFIED` до прохождения
рубежа и независимой приёмки.

Повторный полный рубеж пройден на fingerprint
`0a54175ff8b02a094afcd2bf5e9c24c79eedb896a84862aa193f67e452646671`,
неизменном до и после прогона. Все 14 команд завершились кодом 0:
`cargo test --workspace` — 1162 passed, 0 failed, 2 ignored; отдельный
real-bundle запуск — 2/2; conformance harness — 152/152;
`kernel-validate.test.mjs` — 293/293; Kernel-only validation — 0 failures,
9 warnings, `schema: 14/14`; остальные ворота §5.23.7 также зелёные.
Архитектор перевёл VAL-30/NODE-02/NODE-03 в `ACCEPTED_RUST_NATIVE`, закрыл
GAP-09 и вынес вердикт `QUALIFIED` / `ACCEPTED`. Пакет локально
интегрируется отдельным package commit и отдельным `--no-ff` merge-коммитом
без публикации. Пакет 10 этим вердиктом не начинается.

#### Post-merge failure (2026-09-25)

После локальной интеграции (package commit `b5076d5`, merge `074fca1`) новые
файлы стали tracked, и Kernel-only `kernel-validate.mjs` дал 13 `FAIL
document-identity`: Markdown payload-fixtures источника `payload-matrix`
не несли Front Matter; `validate_reports_ok_on_this_kernel_with_zero_real_failures`
давал код 1 вместо 0. Кандидат прошёл рубеж только потому, что эти файлы
были неотслеживаемыми и не сканировались. Вердикт `QUALIFIED` / `ACCEPTED`
больше не действует: текущий статус — **`CHANGES_REQUESTED`**, квалификация —
**`NOT_QUALIFIED`**. Корректирующий раунд (отчёт квалификации §9.5): Front
Matter в таблице `families.json`, штатная перегенерация `payload-matrix`,
закреплённый счётчик `undeclared_prescriptive` 22 → 25 по независимо
проверенному tracked-состоянию; затем полный рубеж §5.23.7 на
tracked-кандидате. Пакет 10 не начинается.

Корректирующий tracked-кандидат с fingerprint
`26bb4f6a213a3fd52a2bed56ce2907a166d6f64bbe0d20ae1ffe61bc8458f151`
прошёл все 15 команд полного рубежа §5.23.7 с кодом 0: 1162 workspace Rust
tests, ранее падавший binary test, 2/2 real-bundle tests, 152/152 conformance
harness и 293/293 `kernel-validate.test.mjs`; Kernel-only validation —
0 failures, 9 warnings, `schema: 14/14`. Архитектор принял исправление,
подтвердил `QUALIFIED` / `ACCEPTED` и локально интегрирует его отдельным
bugfix package commit и отдельным `--no-ff` merge-коммитом без публикации.
Пакет 10 остаётся `planned_not_started`.

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

- применён `standards/workspace/rust-migration-quality.md`, а решение не
  копирует Node.js-структуру ради сходства;
- ответственность каждого изменения соответствует своему крейту; CLI и
  адаптеры не владеют предметными правилами;
- внешние эффекты проходят через предусмотренные порты;
- транспорт, синтаксис, предметная валидация и валидный доменный тип разделены;
- невозможные состояния исключаются типами, конструкторами и исчерпывающим
  сопоставлением, где Rust это позволяет;
- каждое намеренное расхождение с Node.js классифицировано как сохранение
  контракта либо Rust-native улучшение, имеет бизнес-обоснование и тест;
- только после этих архитектурных проверок применяются технические ворота:

- `cargo fmt --check`;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
- `cargo test --workspace`;
- `cargo doc --workspace --no-deps`;
- уже применимые на этом шаге перенесённые фикстуры и регрессионные тесты
  (`test/*.test.mjs`, адаптированные для новой реализации) — только за ту
  поверхность, которую этот пакет и ранее принятые пакеты этой программы уже
  реализуют, а не весь будущий объём заранее.

### 6.2. Харнесс соответствия — с пакета 2, по нарастающей поверхности

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

Харнесс не является архитектурным гейтом и не требует нулевого расхождения
ради самого равенства. Каждый случай относится к одной из трёх категорий:
сохранённый публичный или бизнес-контракт; принятое Rust-native улучшение с
отдельным ожидаемым результатом; дефект Node.js, который запрещено переносить.
Необъяснённые расхождения остаются блокирующей ошибкой.

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

Пакет 9 (`rust-business-contract-qualification`) и пакет 10 (`meridian-rust-release`)
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

- **Запрещён паритет ради паритета.** Node.js является источником
  бизнес-семантики, совместимых входов и проверочных примеров, но не целевой
  архитектурой. Нельзя переносить динамические границы, строковую типизацию,
  смешение ответственности или известный дефект только потому, что это даёт
  одинаковый результат. Сохраняются принятые публичные и бизнес-контракты;
  Rust-native улучшения обязательны по
  `standards/workspace/rust-migration-quality.md` и фиксируются как
  намеренные, обоснованные, протестированные различия.
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
- **Пакет 8 не является ingestion Metis.** `meridian import` переносит
  только управляемые записи замороженного Instance по
  `instance-data-migration.md`. Он не подключает Confluence, Git/Markdown
  knowledge source или кодовый репозиторий, не копирует продуктовую базу
  знаний и не создаёт snapshot/index/graph. Федеративная архитектура Metis и
  будущий `metis-contract-foundation` ведутся отдельной исследовательской
  программой по
  `governance/decisions/metis-federated-knowledge-architecture.md`.
- Не менять логическую модель областей (`workspace-scope-model.md`) ради
  удобства схемы SQLite — схема (§4 технической спецификации) обязана
  выражать существующие шесть областей, а не переопределять их.
- Не вводить PostgreSQL, сетевой API или многопользовательский доступ в
  рамках этой программы (`meridian-rust-sqlite-architecture.md` §6, §14).
- Не удалять Node-реализацию внутри этой программы: удаление — отдельное
  решение владельца **после** пакета 10 и доказанного сохранения
  бизнес-контракта
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
last_completed_package: rust-business-contract-qualification
current_package: rust-business-contract-qualification
current_package_status: accepted_and_locally_integrated
next_package: meridian-rust-release
next_package_status: planned_not_started
concord_status: paused_pending_meridian_rust_release
release_version: unassigned
release_gate: closed
owner_decision_date: 2026-09-24
```

Настоящая ревизия фиксирует решение владельца §5.13 и результат полного
архитектурного аудита, и синхронизирует `current_package_status` с §5.14:
`rust-architecture-conformance-1`'s pilot (`controlled-rule-intake`) has been
IMPLEMENTED across three rounds in the исполнитель's рабочее дерево (no Git
records) and is READY_FOR_ARCHITECT_REVIEW, but is not yet ACCEPTED — the
earlier `required_audit_complete_implementation_not_started` status is
stale the moment implementation begins and is corrected here rather than
left to silently drift from the actual record in §5.14. Исторические факты
интеграции не переписываются; архитектурная готовность 7a/7b к выпуску и
передача 7c остаются отозванными до исправления ВСЕХ семейств 7b/7c
(`existing-project-compatibility-mode` и далее) и повторной независимой
проверки — принятие одного пилота не восстанавливает их. Ни 7d, ни пакет
8, ни Metis, ни Concord, ни эксперимент этой синхронизацией не начинаются;
Concord остаётся на паузе.

**Обновление (2026-09-22, §5.15).** `current_package_status` синхронизирован
с §5.15: `rust-architecture-conformance-2` (`instruction-source-registry`,
`existing-project-compatibility-mode`) реализован исполнителем в рабочем
дереве (без Git-записей) и находится в статусе `READY_FOR_ARCHITECT_REVIEW`
— не `ACCEPTED`. Этот пакет не начинает и не завершает 7d/пакет 8; их запуск
по-прежнему заблокирован до независимой приёмки ОБОИХ пакетов
`rust-architecture-conformance` (1 и 2) архитектором.

**Обновление (2026-09-22, §5.15 раунд 3, финализационный).**
`current_package_status` синхронизирован повторно:
`conformance_2_architecture_approved_pending_final_gate_review` —
архитектор закрыл все шесть пунктов `CHANGES_REQUESTED` второго раунда
(включая пункт 5 — принят как намеренное Rust-native сужение,
`COMPATIBILITY.md`), полный назначенный набор ворот (§5.15, раунд 3) зелёный.
Статус НЕ `ACCEPTED`: итоговый вердикт пакета целиком — решение архитектора
после этой передачи, не самопровозглашённое исполнителем. 7d и пакет 8
по-прежнему не начинаются.

**Обновление (2026-09-22, §5.16, четвёртый корректирующий раунд,
финализационный).** `next_package_status` синхронизирован:
`ready_for_architect_review_not_accepted` —
`meridian-cli-foundation-architecture-remediation` реализован исполнителем в
рабочем дереве (без Git-записей) за первую передачу и четыре корректирующих
раунда `CHANGES_REQUESTED`; целевой набор ворот §5.16.6 зелёный на каждом
раунде, полный `cargo test --workspace` и оба Node-набора (conformance
harness — 99/99, `kernel-validate.test.mjs` — 293/293) зелёные на кандидате
четвёртого раунда (§5.16.8). Статус НЕ `ACCEPTED`: итоговый вердикт —
решение архитектора после независимой проверки, не самопровозглашённое
исполнителем. `current_package`/`current_package_status` не переписаны —
`rust-architecture-conformance` остаётся исторической записью предыдущего
пакета. Ни 7d, ни пакет 8 этим обновлением не открываются: их запуск
по-прежнему заблокирован до независимой архитектурной приёмки
`meridian-cli-foundation-architecture-remediation`.

**Обновление (2026-09-22, §5.16, итоговый вердикт архитектора).** Пакет
`meridian-cli-foundation-architecture-remediation` принят после независимой
проверки фактического production route, канонического реестра совместимости,
публичной документации и переданных результатов ворот; статус синхронизирован
как `accepted_and_locally_integrated`. Локальная интеграция выполнена отдельным
package commit и отдельным `--no-ff` merge-коммитом по действующему режиму
`AGENTS.md` §6.7, без публикации `dev` или исходной ветви. 7d и пакет 8 этим
решением не начинаются.

**Обновление (2026-09-22, §5.17, следующий пакет).** Канонический указатель
продвинут на `rust-architecture-conformance-3` со статусом
`specified_not_started`. Пакет ограничен связанным основанием task-контрактов:
`task-pattern-registry` строит типизированный каталог, а
`task-specification-contract` потребляет его; одновременно вводится реально
используемый `GitInspector` и переиспользуется принятый `WorkspaceReader`.
Остальные четыре семейства 7b, два семейства 7c, весь 7d и пакет 8 остаются
неоткрытыми.


**Обновление (2026-09-23, §5.17, итоговый вердикт архитектора).** Пакет
`rust-architecture-conformance-3` принят после независимого ревью и полного
финального gate: workspace Rust tests — 814/814, conformance harness — 99/99,
`kernel-validate.test.mjs` — 293/293. Он локально интегрирован отдельным
package commit и отдельным `--no-ff` merge-коммитом без публикации.
`current_package_status` синхронизирован как `accepted_and_locally_integrated`;
следующий пакет ещё не специфицирован. Сначала должны быть назначены оставшиеся
семейства 7b, затем 7c; 7d и пакет 8 остаются закрытыми.

**Обновление (2026-09-23, §5.18, следующий пакет).** После аудита четырёх
оставшихся семейств 7b канонический указатель продвинут на
`rust-architecture-conformance-4` со статусом `specified_not_started`. Пакет
ограничен самостоятельным `functional-parity`: typed evidence/checks в core,
transport/WorkspaceReader orchestration в app и composition/presentation в
CLI. `execution-state-model`, `role-and-human-control` и
`bounded-context-manifest` остаются неоткрытым следующим связным срезом; оба
семейства 7c, весь 7d и пакет 8 закрыты.

**Обновление (2026-09-23, §5.18, первая передача и первый корректирующий
раунд).** `current_package_status` синхронизирован:
`ready_for_architect_review_not_accepted` — `rust-architecture-conformance-4`
реализован исполнителем в рабочем дереве (без Git-записей) за первую
передачу и один корректирующий раунд `CHANGES_REQUESTED` (§5.18.1 несёт
полный список исправленных пунктов). Целевой набор ворот §5.18.6 зелёный на
обоих раундах; полные Node-наборы и `cargo test --workspace` не запускались
(назначаются владельцем/архитектором на финальном кандидате, `AGENTS.md`
§9). Статус НЕ `ACCEPTED`: итоговый вердикт — решение архитектора после
независимой проверки, не самопровозглашённое исполнителем. `execution-state-model`,
`role-and-human-control`, `bounded-context-manifest`, оба семейства 7c, весь
7d и пакет 8 этим обновлением не открываются.

**Обновление (2026-09-23, §5.18, второй корректирующий раунд).**
`current_package_status` остаётся `ready_for_architect_review_not_accepted`:
исполнитель выполнил второй корректирующий раунд (полное принятое evidence
вместо проекции, две раздельные I/O-границы в `COMPATIBILITY.md`, matched
Node/Rust case для fixture-I/O, синхронизация §4/rustdoc/§5.18.1) в рабочем
дереве без Git-записей; целевой набор ворот §5.18.6 зелёный. Статус НЕ
`ACCEPTED`; остальные семейства 7b, 7c, 7d и пакет 8 не открываются.

**Обновление (2026-09-23, §5.18, итоговый вердикт архитектора).**
Пакет `rust-architecture-conformance-4` принят и локально интегрирован: полный
финальный gate на неизменённом кандидате дал 852/852 workspace Rust tests,
103/103 conformance harness и 293/293 `kernel-validate.test.mjs`; остальные
финальные ворота также зелёные. `last_completed_package` и
`current_package_status` синхронизированы с §5.18.8. Следующий связный пакет
ещё не специфицирован; остальные семейства 7b/7c, 7d и пакет 8 не открываются.

**Обновление (2026-09-23, §5.19, следующий пакет).** Канонический
указатель продвинут на `rust-architecture-conformance-5` со статусом
`specified_not_started`. Пакет ограничен тремя связными оставшимися
семействами 7b: `execution-state-model`, `role-and-human-control` и
`bounded-context-manifest`. Два семейства 7c остаются неоткрытыми и
временно потребляют тонкий совместимый фасад; 7d и пакет 8 закрыты.

**Обновление (2026-09-23, §5.19, итоговый вердикт архитектора).** Пакет
`rust-architecture-conformance-5` принят после независимого ревью и полного
финального gate: 910/910 workspace Rust tests, 105/105 conformance harness и
293/293 `kernel-validate.test.mjs`; остальные финальные ворота также зелёные.
Он локально интегрирован отдельным package commit и отдельным `--no-ff`
merge-коммитом без публикации. `last_completed_package` и
`current_package_status` синхронизированы с §5.19.8. Следующий пакет ещё не
специфицирован; 7c, 7d и пакет 8 остаются закрытыми.

**Обновление (2026-09-23, §5.20, следующий пакет).** После аудита двух
оставшихся семейств 7c канонический указатель продвинут на
`rust-architecture-conformance-6` со статусом `specified_not_started`.
Пакет объединяет `evidence-and-handoff-contract` и
`meridian-field-evaluation` вокруг одного typed evidence/resolution
основания и удаляет временные `compat_7c`/portability фасады. Четыре
семейства 7d и пакет 8 остаются закрытыми до независимой приёмки и локальной
интеграции пакета 6.

**Обновление (2026-09-23, §5.20, итоговый вердикт архитектора).** Пакет
`rust-architecture-conformance-6` принят после независимого ревью, двух
корректирующих раундов и повторного полного gate: 965/965 workspace Rust
tests, 109/109 conformance harness и 293/293
`kernel-validate.test.mjs`; остальные финальные ворота также зелёные. Он
локально интегрирован отдельным package commit и отдельным `--no-ff`
merge-коммитом без публикации.

**Обновление (2026-09-23, §5.21, следующий пакет).** После аудита всего
оставшегося 7d канонический указатель продвинут на
`rust-architecture-conformance-7` со статусом
`specified_not_started`. Пакет объединяет migration plan, canonical export,
workspace compatibility qualification и upgrade integration qualification,
расширяет существующий typed migration owner и требует композиции через
принятые операции 7b/7c. Пакет 8 остаётся закрытым до независимой приёмки и
локальной интеграции пакета 7.

**Обновление (2026-09-24, §5.21, итоговый вердикт архитектора).** Пакет
`rust-architecture-conformance-7` принят после независимого ревью,
корректирующего раунда и полного финального gate: 1006/1006 workspace Rust
tests, 129/129 conformance harness и 293/293
`kernel-validate.test.mjs`; остальные финальные ворота также зелёные. Он
локально интегрирован отдельным package commit и отдельным `--no-ff`
merge-коммитом без публикации. Архитектурное исправление исторических
семейств 7a–7d завершено. Следующим остаётся
`meridian-cli-migration`, но пакет 8 ещё не специфицирован и не начат.

**Обновление (2026-09-24, §5.22, пакет 8).** После приёмки и локальной
интеграции `rust-architecture-conformance-7` канонический указатель
продвинут на `meridian-cli-migration` со статусом
`specified_not_started`. Пакет фиксирует два закрытых import-входа
(`frozen-instance` и `canonical-records`), точный CLI
`migration plan|apply|verify|rollback`, app-owned
`MigrationRepository`, транзакционный SQLite apply и проверяемый
checkpoint rollback. Обязательные доказательства охватывают реальный bundle
346/336/10, 61 applicability records, round trip, идемпотентность,
mid-transaction failure и возврат pre-state. Пакет 9 остаётся закрытым до
независимой приёмки и локальной интеграции пакета 8; Metis и Concord не
открываются.

**Обновление (2026-09-24, §5.22.10, вердикт `CHANGES_REQUESTED` и
корректирующий раунд 1).** `current_package_status` синхронизирован как
`changes_requested`: архитектор не принял первую передачу
`meridian-cli-migration` (журнал rollback заменял факт applied при
restore; доказательство применимости проходило при пустых сторонах;
`import frozen-instance` проверял tool DB до `--confirm`), одновременно
приняв два наблюдаемых решения `COMPATIBILITY.md`. Исполнитель выполнил
корректирующий раунд 1 в рабочем дереве без Git-записей и передал его как
`READY_FOR_ARCHITECT_REVIEW`; целевые ворота §5.22.8 зелёные, полные
`cargo test --workspace` и полные Node-наборы не запускались (финальный
рубеж, `AGENTS.md` §9). Статус НЕ `ACCEPTED`; пакет 9, Metis и Concord не
открываются.

**Обновление (2026-09-24, §5.22.10, корректирующий раунд 2).**
`current_package_status` остаётся `changes_requested`: архитектор вернул
раунд 1 (rollback сверялся только с не откатанными runs; staging-запись
не доказывала FK-инвариант). Исполнитель выполнил корректирующий раунд 2
в рабочем дереве без Git-записей и передал его как
`READY_FOR_ARCHITECT_REVIEW`; назначенные целевые ворота раунда зелёные,
полные workspace/Node-наборы остаются финальным рубежом. §5.22.1 теперь
явно называет `SPECIFIED_NOT_STARTED` историческим исходным статусом.
Статус НЕ `ACCEPTED`; пакет 9, Metis и Concord не открываются.

**Обновление (2026-09-24, §5.22.10, полный финальный рубеж и
корректирующий раунд 3).** `current_package_status` остаётся
`changes_requested`: после архитектурного одобрения раунда 2 полный
финальный рубеж не прошёл. Архитектурный код rollback принят; найдены
дефекты test-boundary (тестовый модуль без `#![cfg(test)]` для
структурных сканеров), baseline-snapshot (дрейф `undeclared_other` 32 → 34,
существовавший на чистом `HEAD` после интеграции документов Metis) и
harness-environment (унаследованный `MERIDIAN_INSTANCE` в Kernel-only
сравнениях). Начат корректирующий раунд 3 без Git-записей; повторный полный
финальный рубеж назначается после новой архитектурной проверки. Статус НЕ
`ACCEPTED`; пакет 9, Metis и Concord не открываются.

**Обновление (2026-09-24, §5.22.10, решение архитектора по раунду 3).**
`current_package_status` остаётся `changes_requested`. Состоявшаяся
блокировка раунда 3 (`BLOCKED_FOR_ARCHITECT_DECISION`: CLI-сканер не
распознавал заголовочный `#![cfg(test)]`) сохранена в §5.22.10 как история;
архитектор снял её, разрешив вариант A — выравнивание CLI-сканера с
принятым правилом `meridian-app`. Раунд 3 продолжен без Git-записей и
передаётся как `READY_FOR_ARCHITECT_REVIEW`; повторный финальный рубеж
определяет архитектор. Статус НЕ `ACCEPTED`; пакет 9, Metis и Concord не
открываются.

**Обновление (2026-09-24, §5.22.11, итоговый вердикт архитектора).** Пакет
`meridian-cli-migration` принят после независимого ревью, трёх корректирующих
раундов и полного финального gate: 1070/1070 workspace Rust tests, 137/137
conformance harness, 293/293 `kernel-validate.test.mjs` и 2/2 отдельных
real-bundle tests; остальные финальные ворота также зелёные на неизменённом
кандидате. Он локально интегрируется отдельным package commit и отдельным
`--no-ff` merge-коммитом без публикации. Следующий пакет
`rust-business-contract-qualification` ещё не специфицирован и этим решением
не начинается; Metis и Concord остаются закрыты до собственных рубежей.

**Обновление (2026-09-24, §5.23, спецификация пакета 9).** Следующий пакет
`rust-business-contract-qualification` специфицирован, но не начат:
`current_package_status: specified_not_started`. Он обязан квалифицировать
закрытой матрицей всю production-поверхность и одновременно пройти полный
набор §6.1–§6.6; агрегатный зелёный прогон не заменяет построчных executable
доказательств. `meridian-rust-release` заблокирован до независимой приёмки и
интеграции пакета 9; Metis и Concord остаются вне области.

**Обновление (2026-09-25, §5.23.10, первая передача пакета 9).**
`current_package_status` синхронизирован как `blocked_for_architect_decision`:
исполнитель построил закрытую квалификационную матрицу, закрыл восемь
пробелов минимальными исправлениями и доказательствами и передал семь
открытых наблюдаемых дельт и неклассифицированных Node-поверхностей на
решение архитектора; заявленный результат — `NOT_QUALIFIED`. Статус НЕ
`ACCEPTED`; `meridian-rust-release`, Metis и Concord не открываются.

**Обновление (2026-09-25, §5.23.10, корректирующий раунд пакета 9).**
`current_package_status` синхронизирован как `not_qualified_gap_09_open`:
GAP-10…GAP-15 и GAP-02 закрыты по решениям архитектора, но mapping GAP-09
нашёл утраченные бизнес-проверки продуктового состояния без
production-владельца в Rust. Заявленный результат — `NOT_QUALIFIED`; статус
НЕ `ACCEPTED`; `meridian-rust-release`, Metis и Concord не открываются.

**Обновление (2026-09-25, §5.23.11, следующий корректирующий пакет).**
GAP-09 выделен в отдельный пакет `rust-workspace-state-validation` со статусом
`specified_not_started`. Пакет вводит DB-backed `validate` над рабочей базой,
закрывает все category-3 пункты mapping и добавляет только opt-in
`--log-metrics`; штатный Rust CLI не читает `MERIDIAN_INSTANCE`. Пакет 9
остаётся `NOT_QUALIFIED`, а пакет 10, Metis и Concord остаются закрытыми до
реализации, повторной квалификации и независимой приёмки.

**Обновление (2026-09-25, §5.23.11, первая передача корректирующего пакета).**
`current_package_status` синхронизирован как
`corrective_package_blocked_for_architect_decision`: исполнитель реализовал
`rust-workspace-state-validation` с executable evidence каждого
category-3 пункта и передал три решения D1–D3 (отчёт квалификации §9.5).
Статус НЕ `ACCEPTED`; итог квалификации остаётся `NOT_QUALIFIED`;
`meridian-rust-release`, Metis и Concord не открываются.

**Обновление (2026-09-25, §5.23.11, корректирующий раунд корректирующего
пакета).** `current_package_status` синхронизирован как
`corrective_package_ready_for_architect_review_not_accepted`: fail-open
дерева репозитория удалён, три проверки D3 реализованы, payload matrix
закрывает критерий 3, D1 зафиксирован как `ACCEPTED_MIGRATION_BOUNDARY`,
D2 — в `COMPATIBILITY.md`. Статус НЕ `ACCEPTED`; итог квалификации остаётся
`NOT_QUALIFIED` до финального рубежа §5.23.7 и независимой приёмки;
`meridian-rust-release`, Metis и Concord не открываются.

**Обновление (2026-09-25, §5.23.11, финальный рубеж).**
`current_package_status` синхронизирован как
`corrective_package_final_gate_failed_not_accepted`: первый прогон полного
рубежа §5.23.7 одобренного кандидата не пройден из-за устаревшего
закреплённого счётчика `schema_validated` (13 вместо 14); счётчик исправлен,
рубеж повторяется. Статус НЕ `ACCEPTED`; итог квалификации —
`NOT_QUALIFIED`; пакет 10 не открывается.

**Обновление (2026-09-25, §5.23.11, итоговый вердикт архитектора).**
Повторный полный рубеж пройден на неизменённом fingerprint: 1162/1162
workspace Rust tests, 2/2 ignored real-bundle tests, 152/152 conformance
harness и 293/293 `kernel-validate.test.mjs`; остальные ворота также зелёные.
Строки VAL-30/NODE-02/NODE-03 переведены в `ACCEPTED_RUST_NATIVE`, GAP-09
закрыт, квалификация — `QUALIFIED`, пакет 9 — `ACCEPTED` и локально
интегрируется без публикации. Пакет 10 остаётся `planned_not_started`.

**Обновление (2026-09-25, §5.23.11, post-merge failure).**
`current_package_status` синхронизирован как
`post_merge_failure_changes_requested`: на интегрированном tracked-состоянии
Kernel-only validation дала 13 `FAIL document-identity` (payload-fixtures без
Front Matter). Вердикт `QUALIFIED` / `ACCEPTED` не действует; статус —
`CHANGES_REQUESTED`, квалификация — `NOT_QUALIFIED`;
`last_completed_package` возвращён к `meridian-cli-migration`, пакет 10 —
`blocked_pending_package_9_acceptance`.

**Обновление (2026-09-25, §5.23.11, приёмка корректирующего
tracked-раунда).** Front Matter 13 payload-fixtures исправлен в единственном
источнике `families.json`, производные bundle штатно перегенерированы,
закреплённые счётчики синхронизированы с tracked-состоянием. Все 15 команд
полного рубежа прошли на неизменённом fingerprint: 1162/1162 workspace Rust
tests, отдельный ранее падавший binary test, 2/2 real-bundle tests, 152/152
conformance harness и 293/293 `kernel-validate.test.mjs`; остальные ворота
также зелёные. Итог — `QUALIFIED`, пакет 9 — `ACCEPTED` и локально
интегрируется без публикации; пакет 10 остаётся `planned_not_started`.
