---
title: Исследовательская программа улучшения Meridian
document_type: plan
status: active
scope: workspace
owner: workspace-owner
created: 2026-09-20
updated: 2026-09-24
related_documents:
  - $MERIDIAN_KERNEL/governance/research/README.md
  - $MERIDIAN_KERNEL/governance/research/hypothesis-registry.yaml
  - $MERIDIAN_KERNEL/governance/research/knowledge-bases-and-agent-hypotheses.md
  - $MERIDIAN_KERNEL/governance/plans/meridian-rust-migration-program-plan.md
  - $MERIDIAN_KERNEL/standards/workspace/rust-migration-quality.md
  - $MERIDIAN_KERNEL/governance/meridian-owner-intent-contract.md
  - $MERIDIAN_KERNEL/governance/decisions/metis-federated-knowledge-architecture.md
  - $MERIDIAN_KERNEL/governance/specifications/metis-federated-knowledge-plane.md
---

# Исследовательская программа улучшения Meridian

Идентификатор программы: `meridian-self-improvement-research`.

Эта исследовательская программа не может ослаблять правила Rust-миграции.
Для любого пересечения с ней наивысший приоритет имеют соблюдение выбранной
Rust-архитектуры и обязательное использование более надёжного Rust-native
варианта при сохранении бизнес-ценности
(`standards/workspace/rust-migration-quality.md`).

Программа превращает наблюдения об улучшении самого Meridian в проверяемые
гипотезы, отделяет уже обоснованные архитектурные основания от экспериментов и
маршрутизирует подтверждённый результат в обычные решения и планы.

## 1. Область программы

Входит:

- приём и классификация гипотез;
- проверка согласованности с контрактом владельца;
- выделение решений, необходимых независимо от результата эксперимента;
- определение контрольного варианта, метрик, порогов и условий допуска;
- проведение отдельно разрешённых экспериментов;
- принятие, отклонение или замещение гипотез с сохранением доказательств.

Не входит:

- продуктовый код конкретного пользователя Meridian;
- автоматическое начало эксперимента;
- выбор поставщика до сравнительного испытания;
- использование эксперимента как обхода границ активного пакета;
- хранение внутренней цепочки рассуждений модели.

## 2. Пакеты

<a id="package-r0"></a>

### R0. Основание исследовательского отдела (`research-governance-foundation`)

**Результат:** каталог `governance/research/`, сопровождающий документ,
машиночитаемый реестр гипотез, актуализированная исходная концепция и настоящий
план.

**Критерии приёмки:** документы имеют допустимые типы и статусы; каждая
гипотеза содержит проверяемое утверждение, опровержение, архитектурное решение
и условия допуска; реестр не объявляет гипотезу нормой.

**Запрещённое расширение:** пакет не меняет Rust-код, не начинает эксперимент
и не создаёт канонический источник знаний Metis.

<a id="package-r1"></a>

### R1. Основание знаний и агентной среды (`knowledge-agent-foundation`)

**Результат:** отдельный корректирующий пакет программы
`meridian-rust-migration` перед пакетом 7. Он принимает только основания,
необходимые, чтобы ближайший командный интерфейс не закрепил неверную
архитектуру: роли баз, маршрутизацию, эволюцию схемы, нейтральную событийную
границу и отдельность будущего разрешателя знаний.

**Зависимость:** пакет 6 `sqlite-storage-adapter` принят и интегрирован; R0
принят; владелец отдельно начинает пакет.

**Запрещённое расширение:** никакого структурного поиска, графа кода,
векторного хранилища, поставщика индекса или автоматического построения знания.

<a id="package-k0"></a>

### K0. Федеративная архитектура знаний (`metis-federated-knowledge-architecture`)

**Результат:** принятое ADR и approved technical specification отделяют
Workspace Knowledge Control Plane, внешние Knowledge Sources, snapshots и
Derived Retrieval Plane; определяют authority по scope/claim class/version,
adapter boundary, materialization modes, access/retention/deletion, layered
retrieval, coverage и границу Themis/Metis.

**Статус:** завершён решением владельца 2026-09-24. Это архитектурный пакет,
не реализация поиска и не начало эксперимента.

**Запрещённое расширение:** пакет не создаёт schemas/Rust types, provider
adapter, индекс, embeddings, глобальный граф, сервер или копию продуктовой
базы знаний.

<a id="package-k1"></a>

### K1. Минимальные контракты Metis (`metis-contract-foundation`)

**Результат:** закрытые schemas и typed contracts
`KnowledgeSource`, `KnowledgeArtifact`, `SourceSnapshot`,
`AuthorityBinding`, `KnowledgeRelation`, `IndexEntry`,
`KnowledgeQuery`, `ContextBundle`; нейтральные source/index ports;
production pipeline на двух fixture adapters (Git/Markdown-like и
Confluence-like) с exact/lexical retrieval, authority, freshness, access,
conflict и coverage.

**Зависимость:** Rust Meridian выпущен и закрыт пакет 10; K0 принят; владелец
отдельно начинает пакет. До этого статус — `blocked`.

**Критерии приёмки:** все требования
`metis-federated-knowledge-plane.md` §12 и §16; derived plane полностью
удаляем; fixture adapters не содержат provider-specific domain contract;
Themis/Metis и untrusted-content boundary доказаны adversarial tests.

**Запрещённое расширение:** реальные credentials/Confluence, code graph,
embeddings, vector database, agent-as-connector, server/multi-user mode и
изменение канонических источников.

<a id="package-r2"></a>

### R2. Подготовка экспериментов (`agent-context-experiment-readiness`)

**Результат:** для выбранной гипотезы выполнены все условия допуска из
реестра; зафиксированы corpus, control, authority/coverage gold set, метрики,
пороги, безопасность, ACL/retention/deletion, откат, бюджет и роли. Реальные
экспериментальные provider adapters подключаются только в изолированном
контуре с одинаковыми полномочиями чтения.

**Зависимость:** K1 принят; выпускной рубеж Rust Meridian закрыт, если
конкретная гипотеза не содержит более строгого условия; R2 не стартует
автоматически после выпуска.

**Критерий завершения:** гипотеза может перейти из `experiment-gated` в
`experiment-ready` только отдельным решением владельца со ссылками на
доказательства каждого условия.

<a id="package-r3"></a>

### R3. Контролируемый эксперимент (`agent-context-experiment`)

**Результат:** один заранее описанный эксперимент, неизменённый корпус,
сырые измерения, нормализованный отчёт и воспроизводимая команда прогона.

**Зависимость:** соответствующая гипотеза имеет статус `experiment-ready`.

**Запрещённое расширение:** эксперимент не меняет канонические источники,
не применяется по умолчанию и не становится частью выпуска до решения R4.

<a id="package-r4"></a>

### R4. Решение по результату (`hypothesis-disposition`)

**Результат:** одно из решений `accepted`, `rejected` или `superseded` с
доказательствами. Для `accepted` указан отдельный контракт, решение или план,
который получит нормативное изменение. Сам реестр таким контрактом не
становится.

## 3. Очерёдность первых гипотез

1. Архитектурный пакет R1 — выполнен до пакета 7 Rust-миграции.
2. K0 — принятое федеративное решение; K1 — первый пакет после выпуска Rust
   Meridian и отдельного решения владельца.
3. Гипотеза машинно-проверяемой архитектуры — измеряется в будущем полевом
   применении, а не на искусственном переписывании текущего Kernel.
4. Структурное получение и детерминированная навигация — только после K1.
5. Трасса наблюдаемых событий — граница принята в R1, сравнительный
   эксперимент выполняется после появления стабильных точек наблюдения CLI.

## 4. Проверка и откат

- YAML реестра обязан разбираться стандартным YAML-парсером.
- Ссылки, типы документов и Front Matter проходят `kernel-validate`.
- Изменение планов не считается началом программного пакета.
- Откат документального решения выполняется новой записью решения и статусом
  `superseded` или `rejected`, а не удалением истории гипотезы.

## 5. Риски

- **Подмена нормы гипотезой:** предотвращается разными полями
  `architecture_decision` и `experiment`, а также явным решением владельца.
- **Подгонка метрик после результата:** пороги фиксируются до R3.
- **Смешение Metis и индекса:** канонический источник и производное
  представление записываются раздельно.
- **Authority целого документа:** запрещено; binding ограничен scope, claim
  class и применимой версией/временем.
- **Ложная полнота retrieval:** Context Bundle всегда несёт coverage и
  недоступные/устаревшие sources.
- **Утечка через кэш или embeddings:** derived data наследует ACL, retention
  и deletion исходника.
- **Prompt injection:** найденное содержимое всегда untrusted knowledge и не
  получает instruction authority.
- **Глобальная онтология/граф:** namespaced identity и необязательный derived
  graph предотвращают объединение по совпадению терминов.
- **Оптимизация неверной метрики:** recall, authority precision, conflicts и
  correctness оцениваются раньше токенов.
- **Расширение Rust-миграции:** R1 имеет закрытое запрещённое расширение;
  алгоритмы поиска остаются после выпуска.
- **Утечка продуктовых данных:** реестр Kernel хранит универсальную гипотезу,
  а продуктовый корпус и сырые результаты остаются в разрешённой области.

## 6. Фактическое состояние

```yaml
program_id: meridian-self-improvement-research
program_status: active
last_completed_package: metis-federated-knowledge-architecture
current_package: none
current_package_status: blocked
next_package: metis-contract-foundation
next_package_status: blocked_pending_rust_release_and_owner_start
experiments_open_for_implementation: []
owner_decision_date: 2026-09-24
```

R0 (`research-governance-foundation`) принят и интегрирован: пакетный коммит
`3e7881d84ed74eb70c08725e684754a78d079002`, коммит слияния
`b9d16ee07bf7b5bd268e8b46fd35cd1451e26662`
(`meridian-rust-migration-program-plan.md` §5.4). R1
(`knowledge-agent-foundation`) принят и интегрирован: пакетный коммит
`e790a3af880cfab83894cb332e03d48b4ff6fc88`, коммит слияния
`97108dfa00e8b7474ec32332ecacf8494df9460c`
(`meridian-rust-migration-program-plan.md` §5.4). K0 принят решением
владельца 2026-09-24 и зафиксирован ADR/спецификацией без программной
реализации. K1 (`metis-contract-foundation`) остаётся заблокирован до
выпускного рубежа Rust Meridian и отдельного решения владельца. R2
(`agent-context-experiment-readiness`) зависит от принятого K1 и также не
открывается автоматически. Ни один experiment и ни один provider adapter
этой синхронизацией не открывается.
