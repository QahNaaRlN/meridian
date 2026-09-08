---
title: Реестры операционной модели
document_type: readme
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-09-08
updated: 2026-09-08
---

# Реестры операционной модели

Каталог содержит схемы машинных контрактов универсальной операционной модели
Meridian. Данные конкретного продукта здесь не хранятся.

`foundation.schema.json` проверяет машинную половину канонического глоссария
и реестра принципов —
`standards/workspace/operating-foundation.yaml`. Определения терминов находятся
в `standards/workspace/operating-glossary.md`, нормативные последствия
принципов — в `standards/workspace/operating-principles.md`. Валидатор Kernel
проверяет не только форму YAML, но и совпадение идентификаторов и двуязычных
названий с двумя человекочитаемыми документами.

Будущие схемы типов задач, состояния исполнения и управления человеком
добавляются отдельными пакетами программы `meridian-operating-upgrade`; эта
схема не предопределяет их поля.

`workspace-scope-model.schema.json` проверяет канонический пул логических
областей из `standards/workspace/workspace-scope-model.yaml`.
`scoped-record.schema.json` задаёт нейтральный к способу хранения конверт
записи с отдельными областью, происхождением и полномочием. Их нормативный
смысл и переход от отдельного Экземпляра описаны в
`standards/workspace/workspace-scope-model.md`.

`instruction-source-registry.schema.json` — специализированная **обязательная**
схема снимка **источника инструкций** (`instruction-source`): объявленный
носитель и **взаимоисключающие формы** `location` (`medium: file` требует
`path` + непрозрачный `container_ref` и запрещает `service_ref`/`resource_ref`;
`external-service` — наоборот); явная редакция с SHA-256-дайджестом и поле
`currency` (`current`/`stale`/`unverified`, где `stale` — только при
`source-missing`/`source-unreadable`); раздельно смоделированные формат и
`read_channel`, который описывает **только наблюдение самого источника** (`agent-native`
— агент читает файл сам; `meridian-observed` — Meridian сам читает источник для
снимка; `manual` — сообщает человек) и **не** является каналом доставки нормы —
ни регистрация, ни чтение не дают тексту полномочий и не обходят
`controlled-rule-intake`; отдельное отображение `agent-native` с явной границей
видимости Meridian (`meridian_visibility ≠ full`); состояние расхождения: два
проверенных состояния совпадают (`unchanged`) только при совпадении **и**
`revision`, **и** SHA-256-дайджеста, `unknown` недопустим при двух полных
проверенных состояниях, а `source-missing`/`source-unreadable` не несут
текущего состояния. Временна́я связь одного наблюдения проверяется кодом:
`recorded_state` ↔ `divergence.current_state` согласуются по `revision`,
дайджесту **и признаку `verified`** в обе стороны, а при
`source-missing`/`source-unreadable` — `previous_state` совпадает с сохранённым
`recorded_state`. Общий конверт каждой записи по-прежнему проверяется
`scoped-record.schema.json`, а не переопределяется здесь. Правила, которые
подмножество JSON Schema выразить не может, и продуктово-нейтральные фикстуры из
`fixtures/instruction-source-registry.fixtures.json` живут в одной функции
`scripts/lib/instruction-source-registry.mjs`, которую вызывают и участок
`instruction-source-registry` в `scripts/kernel-validate.mjs`, и набор
`test/instruction-source-registry.test.mjs`. Схема — Ядро, **данные реестра —
Экземпляр** (как для реестра приёмки). Контракт обязателен: **отсутствие схемы
или фикстур в Ядре — ошибка проверки Ядра**. Регистрация источника не делает
его текст принятой нормой. Нормативный смысл — в
`standards/workspace/instruction-source-registry.md`.

`task-pattern-registry.schema.json` проверяет **обязательный** каталог
универсальных шаблонов типов задач из
`standards/workspace/task-pattern-registry.yaml`: контейнер и тело шаблона в
`payload` каждой записи (виды работы, класс изменения только при
`work_kind: change`, инварианты, обязательные входы, требуемые доказательства,
условия остановки и три раздельные оси ссылок — `applicable_protocols`,
`applicable_skills`, `applicable_evidence_contracts`). Это специализированная
схема **содержимого**: общий конверт каждой записи по-прежнему проверяется
`scoped-record.schema.json`, а не переопределяется здесь. Межзаписные правила
(ровно семь классификационных пар, разделение осей — способ выполнения не
кладётся в массив протоколов и наоборот, принадлежность каждой `present`-ссылки
отслеживаемому набору файлов Ядра после разрешения симлинков, маршруты
`REFACTOR` и `BUGFIX`, а также текстовая защита от возврата формулировки
`BUGFIX → bugfix-protocol` в `standards/workspace/rule-resolution.md` §7) и
продуктово-нейтральные фикстуры из
`fixtures/task-pattern-registry.fixtures.json` живут в одной функции
`scripts/lib/task-pattern-registry.mjs`, которую вызывают и участок
`task-pattern-registry` в `scripts/kernel-validate.mjs`, и набор
`test/task-pattern-registry.test.mjs`. Отсутствие каталога, схемы или фикстур —
ошибка проверки Ядра. Нормативный смысл каталога — в
`standards/workspace/task-pattern-registry.md`.

`task-specification.schema.json` — **полная обязательная** схема одной
**постановки задачи** (`task-specification`): переносимая человеко- и
машиночитаемая форма конкретной единицы работы. Запись объявляет **эту** схему
в своём `$schema` (переносимой относительной ссылкой), а не схему конверта:
специализированная схема **композирует** общий конверт `scoped-record`
(`id`/`title` — идентичность и русское название; `origin`/`authority`) с телом,
так что потребитель, идущий за `$schema`, за один проход проверяет и конверт, и
тело и не принимает запись без `goal`/`initial_state`/`target_model`. Конверт
при этом **переиспользуется и проверяется отдельно**: `scripts/lib/task-specification.mjs`
дополнительно прогоняет каждую запись против канонической
`scoped-record.schema.json`. Область постановки — поле `scope` конверта,
суженное до двух значений: `project-workspace` и `repository-scope`;
`built-in-methodology`, `user-profile`, `organization-profile` и `run-state`
отклоняются (профили хранят правила и настройки; `run-state` — эпизодическое
состояние запуска, а постановка существует до запуска и может иметь несколько
запусков). Тело в `payload`: `goal`, явное `initial_state`, явная
`target_model`, ссылка `task_pattern` ровно на один существующий шаблон из
каталога типов задач, непустой массив `constraints` и непустой массив
`acceptance_criteria`, где каждый критерий несёт устойчивый семантический `id`,
`statement` и структурированную `verification` (`method` + `expected_result`) —
свободная фраза без проверяемого условия критерием не является. Постановка
описывает **требуемое содержание** работы и **не** несёт состояния запуска
(этап, рабочее состояние, участник, история переходов, следующий шаг) — это
область пакета `execution-state-model`. Правила, которые подмножество JSON
Schema выразить не может (**разрешение** объявленного `$schema` внутри
пространства имён Meridian от канонической **логической** базы
(`CANONICAL_RECORD_BASE`, логический адрес — не каталог файловой системы, не
репозиторий и не способ хранения) до логического адреса
`task-specification.schema.json` Ядра, а не сверка по базовому имени;
разрешение ссылки на шаблон по действующему каталогу с ошибкой при неизвестном
или неоднозначном идентификаторе; согласование `work_kind`/`change_class`;
допустимые области; переносимость строк тела **и ссылочных строк конверта**
`origin.source_ref`/`authority.authority_ref`/`authority.decision_ref` —
отклоняется любой корневой POSIX-путь независимо от разделителя перед ним и
**без перечня допустимых символов первого сегмента** (`/@scope`, `/$private`,
`/~service`, `/💾`, `/Проект`, `/数据` — корневые; `packages/@scope/module` и
`relative/$private/file` — относительные, не отклоняются), `~/`, путь Windows,
`\` и `file://`; непустота и отсутствие повторов критериев; отклонение данных
запуска; русское название), и
продуктово-нейтральные фикстуры из `fixtures/task-specification.fixtures.json`
живут в одной функции `scripts/lib/task-specification.mjs`, которую вызывают и
участок `task-specification-contract` в `scripts/kernel-validate.mjs`, и набор
`test/task-specification.test.mjs`. Схема — Ядро, **конкретные постановки —
Экземпляр**. Отсутствие схемы, каталога типов задач или фикстур — ошибка
проверки Ядра. Нормативный смысл — в
`standards/workspace/task-specification.md`.
