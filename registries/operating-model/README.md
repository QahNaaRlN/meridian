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
