---
title: Changelog
document_type: changelog
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-08-18
updated: 2026-09-08
---

# Changelog

Формат — [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
версионирование — [SemVer](https://semver.org/spec/v2.0.0.html).
Линия версий Kernel независима от Instance и от продуктовых репозиториев.

## [Unreleased]

### Added

- **Ограниченный манифест контекста (`bounded-context-manifest`) — обязательный
  участок проверки.** Продуктово-независимый контракт переносимого, ограниченного
  и машинно-проверяемого перечня авторитетных входов и состояния, достаточного,
  чтобы продолжить именно этот запуск после паузы, смены сеанса, участника или
  роли **без восстановления авторитетного состояния из истории чата**. Новые
  артефакты Ядра: `standards/workspace/bounded-context-manifest.md`,
  `registries/operating-model/context-manifest.schema.json`,
  `registries/operating-model/fixtures/context-manifest.fixtures.json`,
  `scripts/lib/context-manifest.mjs`, `test/context-manifest.test.mjs`.
  - **Одна объявленная схема.** Запись объявляет в `$schema` **полную**
    `context-manifest.schema.json` переносимой относительной ссылкой;
    специализированная схема композирует общий конверт `scoped-record` с телом, а
    `scripts/lib/context-manifest.mjs` дополнительно прогоняет запись против
    канонической `scoped-record.schema.json`. Значение `$schema` **разрешается** в
    логическом пространстве имён Meridian: `resolveSchemaRef` и
    `nonPortableReason` **переиспользуются** из
    `scripts/lib/task-specification.mjs`, а закрытые пулы этапов и рабочих
    состояний — из `scripts/lib/execution-state.mjs`; расходящихся копий пакет не
    вводит.
  - **Область — только `run-state`** (`scope.id` идентифицирует запуск,
    `scope.workspace_id` обязателен, `origin.kind: built-in` отклоняется).
  - **Детерминированная связь с одним запуском.** `execution_run_ref`,
    `task_specification_ref` и `human_control_ref` — **закрытые
    структурированные закреплённые ссылки** `pinned_ref`
    (`{ record_type, id, reference, run_id?, revision?/sha256? }`); строка вместо
    объекта и лишнее поле (тело записи) отклоняются. `scope.id` **точно равен**
    `execution_run_ref.id` (не префикс, не середина, не подстрока —
    `example-run` и `example-run-other` разные запуски);
    `human_control_ref.run_id` равен `execution_run_ref.id`;
    `task_specification_ref` `run_id` не несёт; `record_type` каждой ссылки
    обязан соответствовать слоту.
  - **Закрытое правило точной ревизии.** Точной ревизией машинно признаётся
    только полный Git SHA (40 или 64 hex — не сокращение), строгий тег
    `v?X.Y.Z` или `sha256`-дайджест. Ветка или канал (строка со слэшем —
    `feature/…`, `release/…`, `refs/heads/…`, одно слово без цифр или подвижный
    токен `latest`/`HEAD`/`main`/`develop`/`trunk`/…) — плавающее обозначение
    независимо от цифр; сокращённый Git SHA и любой иной провайдер-зависимый
    `revision` без доказуемой неизменяемости требуют `sha256`. Одно правило
    применяется к `pinned_ref`, `applicable_norms` и изменяемым
    `authoritative_sources`; расходящейся второй реализации нет.
  - **Авторитетные источники.** `authoritative_sources` — непустой список;
    каждый **изменяемый** источник (`mutable: true`) закреплён по правилу выше;
    **неизменяемый** закрепления не требует, но веточная ревизия в нём
    противоречива и отклоняется. Перечисляются только источники, нужные для
    продолжения (`purpose`); повтор `id` или `reference` отклоняется.
  - **Применимые нормы.** Для каждой нормы — переносимая ссылка, происхождение,
    закрепление по тому же правилу и **объяснение применимости**; норма без
    объяснения, без закрепления или с повтором `id`/`reference` отклоняется.
    Множество `reference` всех применимых норм **точно равно**
    `run_state_checkpoint.resolved_norms`: манифест добавляет объяснение, но не
    может добавить норму сверх разрешённого набора запуска или убрать из него.
  - **Раздельные оси.** `decisions` и `open_questions`, `completed_actions` и
    `completed_checks`, `known_gaps` и `blockers` — шесть раздельных списков без
    повторов; каждое препятствие несёт **проверяемое** условие возобновления.
  - **Закреплённый снимок состояния запуска, разрешаемый извне.**
    `run_state_checkpoint` несёт **собственные** `pinned_ref` на запуск,
    постановку и запись управления человеком, а также `scope_revision`,
    `lifecycle_stage`, `work_status`, `next_action`, `next_gate`, `blocker_ids`,
    `resolved_norms` и `completed_checks`. Снимок **не доказывается** второй
    копией тех же полей внутри манифеста: проверка получает **внешний**
    резолвер `(pinned_ref) → фактически разрешённая запись` (для фикстур —
    ключ `resolution` файла фикстур, `makeRecordResolver`) и разрешает
    **отдельно каждую** из шести ссылок — трёх манифеста и трёх снимка. Ответ
    преобразователя — **закрытый контракт**: обязательны `record_type`, непустой
    `id` и подтверждение закрепления (`revision` и/или `content_digest` — по
    исходным байтам либо явному каноническому представлению); заявленный
    `reference` обязан совпадать с разрешаемой ссылкой; для `execution-run`
    обязателен `resolved_state` со всеми осями (`task_specification_ref`,
    `scope_revision`, `lifecycle_stage`, `work_status`, `next_action`,
    `next_gate`, `blocker_ids`, `resolved_norms`, `completed_checks`); для
    `run-human-control` обязателен `linked_run_ref`; отсутствие любого поля —
    точная ошибка, а не пропуск сравнения. Оси `run_state_checkpoint` сверяются
    с записью `execution-run`, разрешённой **из
    `run_state_checkpoint.execution_run_ref`**, и подтверждается, что это **та
    же редакция**, что у `execution_run_ref` верхнего уровня; ссылка снимка на
    другой существующий запуск, постановку или запись управления отклоняется — и
    как несовпадение полей с манифестом, и как расхождение разрешённой
    идентичности. Дополнительно проверяются `resolved_state.task_specification_ref`
    против `manifest.task_specification_ref.reference` и `linked_run_ref` против
    закреплённого запуска. Вымышленный `sha256`, согласованная подмена обеих
    копий и неполный ответ преобразователя отклоняются; при отсутствии самого
    резолвера проверка завершается **закрыто** с точной причиной. С манифестом снимок
    по-прежнему обязан совпадать по редакции области, следующему шагу, рубежу,
    множеству препятствий и множеству завершённых проверок; отсутствие
    закреплённого `execution-run` или `run-human-control` делает манифест
    непостроимым; сам снимок подчиняется осевым правилам модели состояния
    выполнения (`blocked` ⇔ непустой набор препятствий; `waiting_human` называет
    конкретное действие; `completed`/`cancelled` не несут исполнимого шага).
  - **Ограниченность.** Необъявленный избыток контекста (полные тексты файлов,
    журнал команд, `transcript`/`messages`/`chat_history`, выгрузка каталога) и
    поля полного контракта доказательств и передачи (`evidence`, `handoff`,
    состояние рабочего дерева, итоговый вердикт, связь утверждений с
    доказательствами) или показателей полевой оценки отклоняются по построению —
    это предмет пакета `evidence-and-handoff-contract` и следующих.
  - Схема и фикстуры — Ядро; конкретные манифесты запусков — Экземпляр (позже —
    область рабочих данных); в этот пакет реальная запись в Экземпляр не
    добавляется. Продуктово-нейтральный сопутствующий набор разрешённых записей
    (`resolution` в файле фикстур) лежит **вне** каждого `manifest.spec`. Рубеж
    подключён к `scripts/kernel-validate.mjs`, `hooks/pre-push`,
    `.github/workflows/gate.yml` и общей проверке изоляции Git
    (`test/pre-push-git-isolation.test.mjs`). Отсутствие схемы, фикстур или
    набора `resolution` — `FAIL`, не пропуск. `VERSION` не меняется. Пакеты доказательств и передачи,
    полевой оценки и хранилища этим пакетом не начинаются.

- **Роли и управление человеком (`role-and-human-control`) — обязательный участок
  проверки.** Продуктово-независимый контракт универсальных ролей, полномочий
  человека, режимов надзора, назначений участников, независимости проверки,
  переключений и способа связи. Новые артефакты Ядра:
  `standards/workspace/role-and-human-control.md`,
  `standards/workspace/role-registry.yaml`,
  `registries/operating-model/role-registry.schema.json`,
  `registries/operating-model/human-control.schema.json`,
  `registries/operating-model/fixtures/role-and-human-control.fixtures.json`,
  `scripts/lib/role-and-human-control.mjs`,
  `test/role-and-human-control.test.mjs`.
  - **Встроенный каталог ролей.** Одна запись области `built-in-methodology`
    (`record_type: role-registry`) с закрытым пулом семи универсальных ролей:
    `owner`, `operator`, `executor`, `reviewer`, `verifier`, `git_integrator`,
    `deployer` — каждая ровно один раз, с русским названием и списком
    ответственности. Каталог композирует общий конверт `scoped-record`;
    `scripts/lib/role-and-human-control.mjs` дополнительно прогоняет его против
    `scoped-record.schema.json`. Конкретная программа, модель ИИ, поставщик или
    имя человека в универсальное определение роли не входят и отклоняются как
    лишнее поле. `git_integrator` определён как **подготовка и проверка
    интеграции** и выполнение только тех Git-операций, которые назначены
    действующим потоком; окончательное слияние принятый порядок может передать
    владельцу. Каталог — `standards/workspace/role-registry.yaml`.
  - **Запись управления одним запуском.** Область только `run-state`
    (`scope.workspace_id` обязателен, `origin.kind: built-in` отклоняется),
    переносимая ссылка `execution_run_ref` ровно на один запуск; постановка и
    тело `execution-run` (этап, рабочее состояние, препятствия, история
    переходов, следующий шаг) не встраиваются — второй источник состояния
    запуска не вводится. `$schema` объявляет полную `human-control.schema.json` и
    **разрешается** в пространстве имён Meridian —
    `resolveSchemaRef`/`nonPortableReason` переиспользуются из
    `scripts/lib/task-specification.mjs`.
  - **Независимые оси.** `human_authority` — постоянная константа
    `posture: human-in-command` (не член пула режимов, не отключается; полное
    исключение власти человека не представимо) и `holder` — участник,
    сохраняющий HIC, **обязанный** быть назначенным с ролью `owner` (отсутствие
    носителя или ссылка на неназначенного либо не-владельца отклоняются; конкретная
    программа/модель/поставщик в ось не вводятся). `supervision_mode` — закрытый
    переключаемый пул `human-in-the-loop` | `human-on-the-loop`, **взаимоисключающие**:
    `human-in-the-loop` требует блок `hitl` (конкретное действие человека и
    объявленный рубеж) и **запрещает** `hotl`; `human-on-the-loop` требует блок
    `hotl` (непустые границы автономии и возможность вмешательства) и
    **запрещает** `hitl`. `acting_actor` — действующий участник, обязанный быть в
    `role_assignments`. `role_assignments` — непустой список `{ actor, roles }`
    из закрытого пула; один участник может совмещать роли; пустое или
    неоднозначное назначение, повторный участник и повторная пара (участник,
    роль) отклоняются. `review_independence` необязателен: базовая модель
    «владелец и один исполнитель» принимается без проверяющего; при
    `required: true` нужен отдельный подходящий участник, не совпадающий с
    проверяемым; при `required: false` блок не несёт ссылок `reviewer_actor`/
    `reviewed_actor`; любая присутствующая ссылка проверяется на переносимость
    независимо от `required`. `communication_mode` — закрытый пул `owner_relayed`
    | `direct`, моделируется отдельно от роли и режима.
  - **История переключений.** Обязательна и упорядочена строго возрастающим
    `sequence`. Каждая запись несёт пару `from_`/`to_` для **каждой** изменяемой
    оси управления: режима надзора, способа связи, действующего участника и
    полного набора `role_assignments`. Первая запись задаёт исходное назначение
    (все четыре `from_*` равны `null`); каждая следующая продолжает предыдущую
    **без разрыва на каждой оси**; `to_role_assignments` каждой записи — полный
    валидный набор, а `to_acting_actor` — его участник; каждая несёт основание
    (`reason`) и сохранённую контрольную точку (`checkpoint_ref`); последняя
    запись совпадает со **всем** текущим состоянием управления — режимом надзора,
    способом связи, действующим участником **и** назначениями ролей. Отдельное
    изменение одной оси — обычная запись, где остальные оси повторяют предыдущее
    `to_`-значение. Переключение любой оси не открывает новый запуск и не
    сбрасывает состояние исполнения.
  - Схемы, каталог и фикстуры — Ядро; конкретные назначения и состояние
    управления — Экземпляр (позже — область рабочих данных); в этот пакет
    реальная запись в Экземпляр не добавляется. Рубеж подключён к
    `scripts/kernel-validate.mjs`, `hooks/pre-push`,
    `.github/workflows/gate.yml` и общей проверке изоляции Git
    (`test/pre-push-git-isolation.test.mjs`). Отсутствие схемы, каталога или
    фикстур — `FAIL`, не пропуск. `VERSION` не меняется. Пакеты ограниченного
    контекста, доказательств и передачи, полевой оценки и хранилища этим пакетом
    не начинаются.

- **Модель состояния выполнения (`execution-state-model`) — обязательный участок
  проверки.** Переносимый, независимый от способа хранения снимок состояния
  одного **запуска** (`execution-run`). Новые артефакты Ядра:
  `standards/workspace/execution-state-model.md`,
  `registries/operating-model/execution-state.schema.json`,
  `registries/operating-model/fixtures/execution-state.fixtures.json`,
  `scripts/lib/execution-state.mjs`, `test/execution-state.test.mjs`.
  - **Управляемая сущность — запуск, а не разговорная «задача».** Одна
    постановка может иметь несколько запусков; каждый несёт свой `id`, русское
    `title` и **ровно одну** переносимую ссылку `task_specification_ref` —
    постановка не встраивается и не дублируется в `payload`.
  - **Одна объявленная схема.** Запись объявляет в `$schema` **полную**
    `execution-state.schema.json` переносимой относительной ссылкой;
    специализированная схема композирует общий конверт `scoped-record` с телом,
    а `scripts/lib/execution-state.mjs` дополнительно прогоняет запись против
    канонической `scoped-record.schema.json`. Значение `$schema` **разрешается**
    в логическом пространстве имён Meridian: функции разрешения ссылки и правил
    корневых путей (`resolveSchemaRef`, `nonPortableReason`) **переиспользуются**
    из `scripts/lib/task-specification.mjs` — расходящейся копии этих правил
    пакет не вводит. Разрешение не обращается к файловой системе и не зависит от
    `process.cwd()`; проверочный адаптер отображает логический адрес на файл
    схемы Ядра.
  - **Область — только `run-state`** (`scope.id` идентифицирует запуск,
    `scope.workspace_id` обязателен). `project-workspace`, `repository-scope`,
    `built-in-methodology`, `user-profile`, `organization-profile` и
    `origin.kind: built-in` отклоняются.
  - **Независимые оси.** `payload` закрыт и требует
    `lifecycle_stage` (закрытый упорядоченный пул `intake` → `classification` →
    `norm_resolution` → `planning` → `execution` → `verification` → `acceptance`
    → `integration` → `deployment` → `observation` → `completion`),
    `work_status` (закрытый пул `planned`, `ready`, `active`, `waiting_human`,
    `blocked`, `failed`, `completed`, `cancelled`), `scope_revision`,
    `current_actor` (непрозрачная переносимая ссылка без роли и полномочия),
    `resolved_norms`, `completed_checks` (уникальные переносимые ссылки),
    `blockers`, `next_action`, `next_gate` (исполнимое значение либо явный
    `null`) и `transition_history`. Одно универсальное поле `status` и
    преждевременные поля роли, режима надзора (`HIC`/`HITL`/`HOTL`), способа
    связи, независимости проверяющего, манифеста контекста, доказательств,
    передачи и полевой оценки отклоняются по построению.
  - **Согласованность осей.** Непустой `blockers` ⇔ `work_status: blocked`;
    `blocked` без препятствия отклоняется; `waiting_human` обязан называть
    конкретное действие в `next_action`; `completed`/`cancelled` обязаны нести
    `next_action` и `next_gate` равными `null`; препятствие несёт устойчивый
    `id`, `description` и проверяемое `resumption_condition`.
  - **История переходов.** Обязательна и упорядочена строго возрастающим
    `sequence` (не порядком файлов и не временем изменения); первая запись
    задаёт исходное состояние (`from_stage`/`from_status` равны `null`); каждая
    следующая продолжает предыдущую без разрыва; последняя совпадает с текущими
    `lifecycle_stage`, `work_status` и `scope_revision`; `scope_revision` в
    записях не убывает; переход назад требует `backward_rationale`, пропуск
    этапа — перечисления промежуточных этапов в `skipped_stages` с обоснованием,
    переход после `completed`/`cancelled` — `reopen_rationale`; `reason`
    обязателен; журнал команд и переписка в записи перехода отклоняются.
  - Схема — Ядро, записи запусков — Экземпляр; в этот пакет реальная запись в
    Экземпляр не добавляется. Рубеж подключён к `scripts/kernel-validate.mjs`,
    `hooks/pre-push`, `.github/workflows/gate.yml` и общей проверке изоляции Git
    (`test/pre-push-git-isolation.test.mjs`). Отсутствие схемы или фикстур —
    `FAIL`, не пропуск. `VERSION` не меняется. Пакеты ролей и управления
    человеком, ограниченного контекста, доказательств и передачи, полевой
    оценки и хранилища этим пакетом не начинаются.

- **Контракт постановки задачи (`task-specification`) — обязательный участок
  проверки.** Переносимая человеко- и машиночитаемая форма конкретной единицы
  работы. Новые артефакты Ядра: `standards/workspace/task-specification.md`,
  `registries/operating-model/task-specification.schema.json`,
  `registries/operating-model/fixtures/task-specification.fixtures.json`,
  `scripts/lib/task-specification.mjs`, `test/task-specification.test.mjs`.
  - **Одна объявленная схема.** Запись объявляет в `$schema` **полную**
    `task-specification.schema.json` (переносимой относительной ссылкой), а не
    схему конверта: специализированная схема композирует общий конверт
    `scoped-record` (идентичность и русское название — `id`/`title`;
    `origin`/`authority`) с телом, поэтому потребитель, идущий за `$schema`, за
    один проход проверяет и конверт, и тело и не принимает запись без
    `goal`/`initial_state`/`target_model`. Канонический конверт при этом
    переиспользуется и проверяется отдельно (`scripts/lib/task-specification.mjs`
    прогоняет запись и против `scoped-record.schema.json`). Значение `$schema`
    проверка **разрешает** внутри пространства имён Meridian, а не сверяет по
    базовому имени: каноническая база (`CANONICAL_RECORD_BASE`, логический
    адрес `records/task-specification`) — **логическое пространство имён, а не
    каталог файловой системы, репозиторий или способ хранения**; адаптер
    хранения отображает логический адрес на фактический ресурс (встроенная
    схема Ядра, локальная база, удалённая служба либо переходный Экземпляр), и
    перемещение записи между способами хранения не меняет её `id`, `scope`,
    `origin`, `authority` или смысл `$schema`. От этой базы переносимая ссылка
    обязана разрешаться ровно в логический адрес `task-specification.schema.json`
    Ядра. Отклоняются ссылки, разрешающиеся не туда: отсутствующая;
    непереносимая (абсолютный путь); указывающая на конверт; без относительного
    пути — одно базовое имя; в отсутствующий сегмент пространства имён; с
    правильным базовым именем в другом сегменте; выходящая за корень
    пространства имён. Пакет определяет только логический контракт разрешения и
    не реализует постоянное хранилище, миграцию Экземпляра или новый адаптер.
    Семантические инварианты разрешения: оно не обращается к файловой системе,
    не зависит от текущего рабочего каталога процесса, `resolveSchemaRef`
    возвращает логический адрес, а отдельный проверочный адаптер отображает его
    на файл схемы Ядра. То, что логический адрес не определяет физическое
    расположение, не запрещает будущему адаптеру хранения использовать
    совпадающий физический путь.
  - **Область.** `scope` ограничен `project-workspace` и `repository-scope`;
    `built-in-methodology`, `user-profile`, `organization-profile` и
    `run-state` отклоняются (профили хранят правила и настройки; `run-state` —
    эпизодическое состояние запуска, а постановка существует до запуска и может
    иметь несколько запусков).
  - **Тело.** Цель, явное исходное состояние, явная целевая модель, ссылка
    ровно на один существующий шаблон типа задачи из `task-pattern-registry`,
    непустой набор ограничений, непустой набор структурированных критериев
    приёмки (`id` + `statement` + `verification{method, expected_result}` —
    свободная фраза критерием не является). Постановка не хранит состояние
    запуска (этап, рабочее состояние, участник, история переходов, следующий
    шаг) — это область следующего пакета `execution-state-model`.
  - **Переносимость строк.** Отклоняется **любой** корневой POSIX-путь —
    независимо от имени первого сегмента, от обычного текстового разделителя
    перед ним (`path=/…`, `a,/…`, `x:/…` отклоняются так же, как `/etc/hosts`;
    слово, оканчивающееся на `:/`, — это корневой POSIX-путь, а не путь с
    буквой диска) и **без перечня допустимых символов первого сегмента**
    (`/@scope/file`, `/$private/file`, `/~service/file` — это корневой путь, а
    не ссылка `~/…`, — `/💾`, `/Проект/файл`, `/数据/文件` отклоняются так же,
    как `/etc/hosts`), — ссылка `~/…`, путь Windows с буквой диска, разделитель
    `\` и `file://`; обычный относительный путь не отклоняется, включая
    scoped-сегменты и префикс `$` (`packages/@scope/module`,
    `relative/$private/file`, `документы/описание`), как и обычный веб-URL и
    обиходные `n/a`, `24/7`, `owner-decision:2026-09-09:example`. Проверка
    распространяется и на ссылочные строки
    конверта — `origin.source_ref`, `authority.authority_ref` и
    `authority.decision_ref` (если присутствует): постановка переносима
    целиком, поэтому машинный путь или `file://` в происхождении или полномочии
    — такой же дефект. В производственном модуле нет собственного списка
    продуктовых маркеров — продуктовую нейтральность доказывает действующий
    полный участок `kernel-purity` после индексации кандидата.
  - Схема — Ядро, конкретные постановки — Экземпляр; в этот пакет реальная
    постановка в Экземпляр не добавляется. Рубеж подключён к
    `scripts/kernel-validate.mjs`, `hooks/pre-push`, `.github/workflows/gate.yml`
    и общей проверке изоляции Git (`test/pre-push-git-isolation.test.mjs`).
    Отсутствие схемы, каталога типов задач или фикстур — `FAIL`, не пропуск.
    `VERSION` не меняется.

### Changed

- **`standards/workspace/rule-resolution.md` §7 — ограниченное согласование по
  решению владельца.** Один маршрутный пункт больше не называет
  `BUGFIX → bugfix-protocol` маршрутом к протоколу Ядра: `bugfix-protocol`
  (`skills/bugfix-protocol/SKILL.md`) — это `skill` в терминах глоссария, и
  отдельного документа-протокола класса `protocol` для `BUGFIX` в Ядре нет. Для
  `REFACTOR` маршрут к настоящему протоколу `refactor-protocol` сохранён. Запись
  `applicable_protocols` резолвера с `routed_from: BUGFIX` не трогается, новый
  протокол `BUGFIX` не вводится, прочие правила разрешения норм не изменяются.
  Текстовая защита в участке `task-pattern-registry` валидатора не даёт этой
  формулировке вернуться.
- **`standards/workspace/version-control-flow.md` §13.2 — the enforcement
  mechanism of the branch-name norm, not the norm's content.** The public
  personal GitHub repository has no available metadata rule that forbids
  *creating* a branch by a regular expression, so §13.2 no longer claims a
  repository-wide platform restriction on branch creation ("набор Б —
  ограничение имён ветвей"). It is reworked into an honest contract: a
  machine check of the **merge request's source branch name** only.
  - The **closed name template (§3, §3.1) and the branch topology are
    unchanged**, and the canonical full expression
    `^(?:main|dev|(?:(?:feature|bugfix|hotfix|promotion|chore|docs|refactor|test|ci|build)/[a-z0-9]+(?:-[a-z0-9]+)*|release/[0-9]+\.[0-9]+\.[0-9]+))$`
    is unchanged. §3 and §3.1 stay the normative source of the template.
  - **The global branch-creation restriction is replaced by a reachable
    merge-request check.** Creating or pushing a branch with a disallowed
    name stays technically possible; such a branch fails the mandatory
    merge-request check and therefore cannot be merged into the protected
    `main` or `dev`. That is the whole mechanical guarantee.
  - **Link to set A.** The check is a step of the already-mandatory
    `Kernel validate (synthetic instance)` job (§13.1), so a red result
    blocks the merge into `main` / `dev`. No new required check and no new
    job name are introduced; the exact existing name is kept.
  - **Substantive review check kept.** English meaning, no transliteration
    of Russian, a meaningful (non-generic) slug and a branch type that
    matches the actual work stay a review responsibility (§3, §3.1, §2.7) —
    a regular expression cannot prove them.
  - §3.1, §10, §11 references that assumed a platform-level restriction on
    creating all branches are corrected to point at the source-branch check.
  - The historical `[0.5.0]` release entry, which describes set B as a
    platform branch-name restriction, is left unchanged — it records what
    that package declared.

### Added

- **Реестр источников инструкций (`instruction-source-registry`) — обязательный
  переносимый контракт.** Добавлены нормативный документ
  `standards/workspace/instruction-source-registry.md` и специализированная
  схема снимка `registries/operating-model/instruction-source-registry.schema.json`.
  Контракт — **обязательная часть Ядра**: отсутствие схемы или фикстур рядом с
  ней — ошибка проверки Ядра (`FAIL`), а не информационный пропуск. Контракт
  описывает **источник инструкций** (`instruction-source`) — зарегистрированный
  носитель агентских инструкций: устойчивые `id`/`title` в форме «Название
  [идентификатор]»; область из принятой модели шести логических областей, не
  выводимую из физического каталога или репозитория; **взаимоисключающие формы**
  `location` — `medium: file` требует относительный нормализованный `path` без
  абсолютов, `..` и обратной косой черты **и** непрозрачный `container_ref`
  (идентичность носителя, не машинный путь) и **запрещает**
  `service_ref`/`resource_ref`; `external-service` требует
  `service_ref` + `resource_ref` без файлового пути и **запрещает**
  `path`/`container_ref`; единственное допустимое поведение при неразрешимости
  `missing_behavior: fail-closed` (без скрытого запасного разрешения);
  зафиксированное состояние с **явной редакцией и SHA-256-дайджестом** и полем
  `currency` — `current` (снимок сверен, статус не `source-missing`/`source-unreadable`),
  `stale` (снимок был сверен, но более поздняя проверка нашла источник
  **пропавшим или недоступным** — не даётся для `changed`, где свежее
  наблюдение перезаписывается, и не подменяется `unverified`), `unverified`
  (снимок ни разу не сверялся); **раздельно** смоделированные формат содержимого
  и `read_channel`, который описывает **только наблюдение самого источника**
  (`agent-native` — агент читает файл сам; `meridian-observed` — Meridian сам
  читает источник для снятия снимка; `manual` — сообщает человек) и **не**
  является каналом доставки нормы — ни регистрация, ни чтение источника не дают
  его тексту полномочий и не обходят `controlled-rule-intake`; отдельное
  отображение `agent-native` с явной границей видимости Meridian
  (`meridian_visibility ≠ full`). Временна́я семантика однозначна:
  `recorded_state` — снимок, который держат сейчас; `divergence.previous_state`
  — снимок до последней проверки (историческое, может отличаться);
  `divergence.current_state` — свежее наблюдение, которое описывает то же
  текущее состояние, что и `recorded_state`, и не вправе противоречить ему по
  `revision`, дайджесту **или признаку `verified`** (в обе стороны — одно
  наблюдение не бывает одновременно проверенным и непроверенным);
  `source-missing`/`source-unreadable` текущего состояния не несут, а их
  `previous_state` (последнее известное состояние) обязан совпадать с
  сохранённым `recorded_state` по `revision` и дайджесту. Расхождение
  детерминировано: два проверенных состояния совпадают (`unchanged`) **только**
  при совпадении и `revision`, и SHA-256-дайджеста — различие любого из них даёт
  `changed`; при двух полных проверенных состояниях объявленный статус обязан
  совпадать с вычисленным, а `unknown` допустим только при действительно
  недостаточных доказательствах. Изменённый, отсутствующий, недоступный или
  непроверенный источник совпадающим не считается. **Запись источника не даёт
  его тексту полномочий нормы** (`normative_status: not-a-norm`,
  `record_type: instruction-source`), не принимает правила, не задаёт приоритет
  и не разрешает конфликты; сканирование файлов проекта, разбор текста на
  правила, решение о принятии, генерация адаптеров и миграция данных в контракт
  не входят. Термин «Источник инструкций [`instruction-source`]» добавлен
  одновременно в `standards/workspace/operating-glossary.md` и
  `standards/workspace/operating-foundation.yaml`.
- **Проверка реестра источников инструкций.** Общая проверяемая функция
  `scripts/lib/instruction-source-registry.mjs` композирует проверки — общий
  конверт каждой записи против существующей `scoped-record.schema.json`, тело
  снимка против новой схемы, а правила, которые подмножество JSON Schema
  выразить не может (непрозрачность ссылок и относительный нормализованный путь
  — взаимоисключающие формы `location` проверяются схемой; связь `currency` с
  `revision_verified` и с `divergence.status`, где `stale` — только при
  `source-missing`/`source-unreadable`; временна́я согласованность
  `recorded_state` и `divergence.current_state` по `revision`, дайджесту и
  признаку `verified` в обе стороны, а для `source-missing`/`source-unreadable`
  — совпадение `previous_state` с сохранённым `recorded_state`; вычисление
  расхождения по совпадению и `revision`, и дайджеста; запрет `unknown` при
  двух полных проверенных состояниях; запрет `current_state` при
  `source-missing`/`source-unreadable`; граница канала `agent-native`), — кодом
  функции; её вызывают и участок `instruction-source-registry` в
  `scripts/kernel-validate.mjs` (обязательный: отсутствие схемы или фикстур —
  `FAIL`), и самостоятельный набор `test/instruction-source-registry.test.mjs`,
  поэтому две реализации не расходятся. Синтетическое Ядро набора
  `kernel-validate` несёт этот обязательный контракт (как и каталог типов
  задач). Проверка детерминирована и закрыта при неоднозначности;
  продуктово-нейтральные фикстуры
  `registries/operating-model/fixtures/instruction-source-registry.fixtures.json`
  покрывают обычный обнаруживаемый источник, автоматически читаемый агентом
  источник, внешнюю службу, свежую регистрацию, недоступный источник и
  непроверенное наблюдение, а отрицательные — неизвестную область, неустойчивую
  идентичность, абсолютный путь и выход из области, файловый источник без
  `container_ref`, смешанные формы `location` (`file` с
  `service_ref`/`resource_ref`, `external-service` с `path`/`container_ref`),
  неверный дайджест, отсутствие редакции, неизвестный формат или канал, различие
  только `revision` или только дайджеста при объявленном `unchanged`, `unknown`
  при двух полных проверенных состояниях, `recorded_state` в противоречии с
  `divergence.current_state` (в т. ч. по признаку `verified`, в обе стороны),
  `source-missing` с `previous_state`, расходящимся с `recorded_state`, ложное
  совпадение без доказательства, ложный `currency: current` и `currency: stale`
  без доказательства, попытку объявить источник нормой и необъявленное запасное
  поведение. Набор подключён к локальному рубежу перед отправкой
  (`hooks/pre-push`) и общей непрерывной проверке (`.github/workflows/gate.yml`).
- **Каталог типов задач (`task-pattern-registry`) — обязательная часть Ядра.**
  Добавлены нормативный документ `standards/workspace/task-pattern-registry.md`,
  машинный каталог `standards/workspace/task-pattern-registry.yaml` и
  специализированная схема содержимого
  `registries/operating-model/task-pattern-registry.schema.json`. Каталог
  вводит семь встроенных универсальных шаблонов типов задач — по одному на
  каждую действующую пару `work_kind` / `change_class`: оценку, эксплуатацию,
  декомпозицию инициативы и четыре класса изменения (`BUGFIX`, `FEATURE`,
  `BEHAVIOR_CHANGE`, `REFACTOR`). Каждый шаблон несёт `invariants`,
  `required_inputs`, `required_evidence`, `stop_conditions` и **три раздельные
  оси ссылок** — `applicable_protocols`, `applicable_skills` и
  `applicable_evidence_contracts` (протокол, способ выполнения и контракт
  доказательств — разные сущности глоссария). `BUGFIX` связан со способом
  выполнения `skills/bugfix-protocol/SKILL.md` через `applicable_skills`, а не
  через протоколы; `REFACTOR` ссылается на настоящий протокол
  `verification/functional-parity/refactor-protocol.md`. Отсутствие
  канонического источника представлено машинно различимым `status: absent` с
  причиной, а не подставленной ссылкой. `change_class` обязателен и допустим
  только при `work_kind: change`; `initiative` требует декомпозиции и класса
  изменения не получает. Действующие пулы `work_kind` и `change_class`
  (`rule-resolution.md` §2–§3) не переопределяются; сущности не
  переименовываются. Единственное согласование вне каталога — ограниченная
  правка `rule-resolution.md` §7 (см. `### Changed`); новый протокол `BUGFIX`
  не вводится.
- **Проверка каталога типов задач.** Общая проверяемая функция
  `scripts/lib/task-pattern-registry.mjs` композирует проверки — общий конверт
  каждой записи против существующей `scoped-record.schema.json`, тело шаблона
  против новой схемы, межзаписные правила и продуктово-нейтральные фикстуры
  (`registries/operating-model/fixtures/task-pattern-registry.fixtures.json`)
  кодом функции — и её вызывают и участок `task-pattern-registry` в
  `scripts/kernel-validate.mjs`, и самостоятельный набор
  `test/task-pattern-registry.test.mjs`, поэтому две реализации не расходятся.
  Отсутствие каталога, его схемы или фикстур — ошибка проверки Ядра, а не
  информационный пропуск; обязательность не зависит от `VERSION`. Для каждой
  ссылки `status: present` проверяется принадлежность цели Ядру: путь
  относительный и нормализованный, без абсолютных путей, сегментов `.`/`..` и
  обратной косой черты, лексически и после разрешения симлинков внутри
  `KERNEL_ROOT`, обычный файл, входящий в отслеживаемый набор. Отдельная
  текстовая защита не даёт `rule-resolution.md` снова назвать `bugfix-protocol`
  протоколом Ядра. Самостоятельный набор подтверждает семь шаблонов, правила
  `change_class`, разделение трёх осей, маршруты `REFACTOR` и `BUGFIX`,
  представление отсутствия, продуктово-нейтральные сценарии выхода за пределы
  Ядра, защиту согласования с `rule-resolution.md` и обязательные отрицательные
  отклонения; он подключён к локальному рубежу перед отправкой и общей
  непрерывной проверке. Синтетическое Ядро набора `kernel-validate` несёт
  минимальный корректный каталог и компактный набор фикстур; полный набор из 34
  отрицательных документов используется только сценариями, действительно
  проверяющими каталог, — время набора не привязано к нестабильному лимиту, а
  снижено устранением повторного копирования большого файла.
- **Модель областей рабочих данных (`workspace-scope-model`).** Добавлены шесть
  логических областей: встроенная методология, профиль пользователя, профиль
  организации, рабочее пространство проекта, локальная область репозитория и
  состояние запуска. Область не зависит от каталога, репозитория Git, базы
  данных или удалённого хранилища.
- **Проверяемый конверт записи.** Новая схема требует отдельные идентичность,
  область, происхождение и полномочие; область репозитория и состояние запуска
  явно ссылаются на рабочее пространство. Более локальная область не получает
  больший приоритет автоматически, а путь хранения запрещён как поле конверта.
- **Переходная совместимость Экземпляра.** `kernel-boundary.md` сохраняет
  отдельный Экземпляр и `MERIDIAN_INSTANCE` как действующий переходный адаптер,
  но больше не объявляет два Git-репозитория целевой архитектурой или
  требованием для нового проекта.
- **Проверка модели областей.** Отдельный набор из четырнадцати проверок подтверждает
  точный пул областей, поддерживаемость схем, допустимый конверт и отклонение
  записей с потерянным контекстом, происхождением либо с физическим путём в
  идентичности. Он подключён к локальному рубежу перед отправкой и общей
  непрерывной проверке.

- **Фундамент операционной модели (`meridian-operating-foundation`).** Добавлены
  канонический глоссарий, реестр универсальных принципов и их общая машинная
  половина `standards/workspace/operating-foundation.yaml` со схемой
  `registries/operating-model/foundation.schema.json`. Термины различают
  задачу, единицу работы, постановку, запуск, норму, протокол, рабочий процесс,
  способ выполнения, средство, доказательство, роль, надзор и контекст.
- **Проверка согласованности основания.** `kernel-validate.mjs` сверяет
  идентификаторы и двуязычные названия YAML с отмеченными таблицами
  `operating-glossary.md` и `operating-principles.md`, падает при отсутствующей
  половине, рассинхронизации, дублировании, пустом определении или обязательном
  следствии, неоднозначном машинном имени либо повреждённой разметке; набор
  регрессии покрывает эти пути.
- **Правило идентичности управляющих сущностей.** Каждая такая сущность имеет
  стабильный смысловой `id` и понятное человеку `title`, которые показываются
  вместе; номер дорожной карты, статус, версия, дата, участник, средство и
  модель ИИ идентичностью не являются.

- **`scripts/validate-branch-name.mjs` — a portable, dependency-free Node.js
  branch-name syntax check.** A pure exported `isValidBranchName(name)` over
  the canonical §13 expression plus a CLI that takes one branch name
  explicitly: exit 0 for an allowed name, exit 1 for a disallowed name,
  exit 2 for a missing argument. It makes no claim about English meaning or
  transliteration.
- **`test/branch-name-validation.test.mjs` — its regression suite.** Covers
  the permanent lines `main` / `dev`, every allowed temporary-branch type,
  `release/MAJOR.MINOR.PATCH`, uppercase, underscores, double and edge
  dashes, a missing or unknown type, an extra `/` segment, Cyrillic and
  spaces, a malformed version form, and the missing-CLI-argument path. It
  also makes the mechanism boundary explicit with a live assertion: a
  syntactically valid transliteration or generic slug passes the regex and
  must still be rejected at review.
- **`.github/workflows/gate.yml` — a source-branch step in the existing
  `Kernel validate (synthetic instance)` job.** The job name is unchanged.
  A separate step runs only for the `pull_request` event, checks
  `github.head_ref`, passes the value through an environment variable and
  quotes it in the command; for `push` and `workflow_dispatch` the step is
  deliberately skipped so a missing `head_ref` never breaks those runs. The
  new regression suite is wired into the same job.

### Notes

- **Граница программы.** Пакет реализует только
  `meridian-operating-foundation`. Схемы типов задач, постановки, состояния,
  управления человеком, контекста и передачи остаются следующими отдельными
  пакетами. `VERSION` остаётся `0.5.0`; выпуск не начинается, Concord не
  возобновляется.

- **Version boundary.** This package stays in `[Unreleased]`; `VERSION`
  stays `0.5.0`. It adds a check and changes the meaning of a norm's
  enforcement mechanism, so a future release under `release-versioning.md`
  §7.1 must be MINOR — but this package does not start or prepare a release.
- **Repository-local Kernel amendment.** It does not start, extend or change
  PHASE G or Concord.

## [0.5.0] — 2026-09-07 (`draft`)

### Added

- **`standards/workspace/version-control-flow.md` — the `git-governance-migration`
  package: permanent line names for Kernel, a shared Meridian temporary-branch
  template and commit-message form, repository-local platform rules for Kernel's
  `main` / `dev`, and an executable operational rename.** One cohesive governance
  package. It does not touch a machine contract or a schema, but it **does change
  a shared Meridian norm** (branch-name template, commit-message form) — see the
  Instance-impact note below; it is not "Kernel-facing only".
  - **Permanent line names — Kernel only (§1.1, §1.2, §1.3).** Kernel declares
    its permanent stable line `main` and permanent integration line `dev`. The
    universal default integration-line name stays `develop`; a repository
    declares its own. Instance and delivery adapters keep the line names their
    own tracked source declares — this package renames none of their branches
    and does not require any `develop` → `dev` move. Until the migration
    completes Kernel's lines are physically `master` / `develop`.
  - **Shared temporary-branch template (§3, §3.1; Meridian-wide).** Ordinary
    branches from the integration line and back — `feature/<slug>`,
    `bugfix/<slug>`, `chore/<slug>`, `docs/<slug>`, `refactor/<slug>`,
    `test/<slug>`, `ci/<slug>`, `build/<slug>`; special-lifecycle branches —
    `release/<semver>`, `hotfix/<slug>`, `promotion/<slug>` (the last only in
    `revision-promotion`). `<slug>` is English, lowercase kebab-case, names the
    result, no transliteration, no empty/generic/process names (`test`, `temp`,
    `changes`, `new-branch`, `my-feature`). A whole-name check expression is
    given; it checks **syntax only** and cannot prove the slug is English words
    and not transliteration — that stays a substantive review check. Meridian
    extensions are separated from classic Git Flow explicitly (§2.5–§2.7):
    `feature` / `release` / `hotfix` keep their original topology; `bugfix` is
    an ordinary fix of not-yet-released state (integration line and back);
    `chore` / `docs` / `refactor` / `test` / `ci` / `build` are ordinary package
    types; `hotfix` is only an urgent fix of already-released state; a branch
    type does not replace work classification or applicable-norm resolution.
  - **Commit-message form (§12; Meridian-wide, not Kernel-local).** First line by
    commit type: `<type>(<scope>): <краткое действие на русском>` for a package
    commit (`<type>` from `feat`, `fix`, `docs`, `chore`, `refactor`, `test`,
    `ci`, `build`, `perf`, `revert`; `<type>`/`<scope>` English lowercase, the
    meaning in Russian, naming the package result, infinitive preferred; one
    commit, one package); `merge(<target>): принять <source> — …` for the
    acceptance merge commit; `release(kernel): выпустить <semver>` for the
    release advancement commit; `promotion(<target>): принять <source> — …` for
    the promotion advancement commit (for a `revision-promotion` repository);
    `hotfix(<target>): исправить <source> — …` for the hotfix advancement commit
    (this form was previously missing). **Advancement commit vs back-merge:** a
    `release` / `promotion` / `hotfix` first line marks **only** the merge of
    that branch into the stable line (release / promotion / hotfix advancement
    commit); the **second MR of the same branch into the integration line is
    not an advancement commit** and takes `merge(<integration-target>): принять
    <source> — …`, like an ordinary `feature` acceptance. The SemVer tag is put
    **only** on the stable-line advancement merge commit; the back-merge commit
    into `dev` gets no tag. The mandatory body — `Что изменено` / `Зачем` /
    `Проверки` / `Связано` (the last `нет` when empty) — applies to **all** of
    these commit types, **both** merge-commit kinds included, not only the
    package commit. A platform-generated English merge message is not sufficient:
    the title and body are brought to this form before the merge. §7 and §11 now
    point to §12.
  - **Repository-local platform rules for Kernel (§13), two independent sets.**
    Set A — protection of `main` and `dev`: mandatory merge request as the only
    change path; empty bypass list; direct and force pushes forbidden for
    everyone including the administrator; deletion forbidden; a distinct merge
    commit mandatory (squash / rebase / fast-forward-without-merge-commit
    forbidden); linear history off (incompatible with the mandatory merge
    commits); required status check named exactly `Kernel validate (synthetic
    instance)` (the existing `.github/workflows/gate.yml` job). Set B — a
    branch-name restriction that takes effect **after the migration completes**,
    applies to all branches, and admits `main`, `dev` and the permitted
    temporary branches; full expression
    `^(?:main|dev|(?:(?:feature|bugfix|hotfix|promotion|chore|docs|refactor|test|ci|build)/[a-z0-9]+(?:-[a-z0-9]+)*|release/[0-9]+\.[0-9]+\.[0-9]+))$`.
    The platform configuration is execution of the tracked norm, not its
    replacement.
  - **Merge flow aligned with the protected lines (§5.5), Kernel-local.** Once
    protection is active every change to `main` or `dev` is a merge request — no
    path needs a direct push. Kernel is `semver-release`, so the Kernel-local
    path table has **no `promotion/<slug>` row** (`promotion` is a
    `revision-promotion` path, §2.3): `feature` / `bugfix` / `chore` / `docs` /
    `refactor` / `test` / `ci` / `build` → `dev`; `release/<semver>` → `main`
    (release advancement commit), then a separate MR → `dev` (back-merge);
    `hotfix/<slug>` → `main` (hotfix advancement commit), then a separate MR →
    `dev` (back-merge). For each MR the owner performs the web merge; a distinct
    merge commit is used; the reviewed source commit stays reachable; squash,
    rebase and fast-forward-without-merge-commit are forbidden; the Git
    integrator runs the post-merge verification; the mandatory body is carried
    by both the advancement commit and the back-merge; the external executor
    gets no Git write on any path. The annotated tag `vX.Y.Z` is created only
    after the confirmed `release/<semver>` → `main` merge, is put **only** on
    the release advancement merge commit in `main`, and never on the back-merge
    into `dev`. §5.3, §9 updated to match; owner-managed MR now covers all these
    paths, not only `feature` → integration.
  - **Executable operational rename (§10, §10.1).** Named
    "create → protect → verify → delete old name", not a platform in-place
    rename (a GitHub in-place rename does not keep the old name available for
    later verification). Order: (1) the package is accepted into the current
    `develop` without a `VERSION` change; (2) **Kernel 0.5.0** is prepared and
    released under the existing `semver-release` model (`release/0.5.0`, MINOR —
    a new norm); (3) the Git integrator records the exact SHAs of the released
    `master` and `develop`; (4) new refs `main` (verified `master` SHA) and
    `dev` (verified `develop` SHA) are created, old refs kept for now; (5)
    active protection rules are created for `main` and `dev`; (6) **a distinct
    step sets `main` as the repository default branch and points the remote
    `HEAD` at `main`**; (7) a following step runs the final checks — SHA
    equality, release reachability, the required check present, protection
    applicability, default branch `main`, remote `HEAD` → `main`; (8) only after
    the final checks pass are `master` and `develop` deleted; (9) local refs and
    worktree tracking move to `main` / `dev`; (10) history and commits are not
    rewritten. The new requirements take effect only after the migration
    completes; the existing published history is not declared a violation; the
    `feature/git-governance-migration` branch is valid under the branch-name
    norm in force when it was created.
  - **Instance impact.** `main` / `dev` and their protection are Kernel's own.
    The shared temporary-branch template (§3.1) and commit-message form (§12)
    change the **shared Meridian norm**. An existing Instance is **not** renamed
    or reconfigured automatically; for a specific Instance to adopt the new norm
    a separate repository-local package is needed there. The compatibility
    statement for this is prepared for the **Kernel 0.5.0** release: the final
    `0.5.x` row of `COMPATIBILITY.md` is written in `release/0.5.0`, and
    `COMPATIBILITY.md` here records that expectation rather than claiming no
    impact.
  - **Current (not historical) references updated.**
    `standards/workspace/release-versioning.md` §6, `README.md` §6 and
    `COMPATIBILITY.md` now name Kernel's permanent lines `main` / `dev` with the
    physical `master` / `develop` transitional note; historical entries in this
    `CHANGELOG.md` that describe past events with `master` / `develop` are left
    unchanged.
  - No new schema, scenario, Git handler or CI file is added. `VERSION`
    unchanged in this package: it is accepted into the integration line, and the
    norm reaches the stable line only with the `release/0.5.0` release.

- **`registries/inventory/repositories.schema.json` — an optional
  `repositories[].ownership` field, `own | foreign` (MERIDIAN-RULE-RESOLUTION —
  authoritative provenance).** `own` — the workspace owner owns this
  repository's norms; `foreign` — write access to the repository does not mean
  ownership of its norms, and its text may not be adopted into the Kernel as the
  owner's own norm. The field is **optional**: an absent value is read as legacy
  `own`, because the inventory was written before the distinction existed and
  every entry then was the owner's — a missing field is not a silent `foreign`
  claim. `schema_version` stays `1` (backward-compatible optional field, no data
  migration), `additionalProperties` stays `false`, `resolver-output.schema.json`
  and `applicability.schema.json` are untouched. `VERSION` unchanged: not a
  release.
  - `test/kernel-validate.test.mjs` — an isolated regression (`t159a`–`t159e`,
    159 → 164) against the real schema and the shared validation engine:
    `ownership: own` and `ownership: foreign` accepted, any other value rejected,
    an entry with no `ownership` still valid, `schema_version` still `const 1`.

- **`registries/rule-resolution/applicability.schema.json` — an optional
  `supersedes` pointer that gives two same-day applicability records of one
  norm identity a truthful, explicit precedence (MERIDIAN-RULE-RESOLUTION —
  same-day precedence).** Background: one textual norm can go through
  instruction-intake twice on one date with different verdicts (e.g. `deferred`
  and `adopt-edition`); §9 makes each a distinct applicability record, both may
  be established or re-synced on the same day, and the resolver — which orders
  a norm identity only by `recorded_at` — then fails closed on the tie. Rather
  than an implicit tie-breaker (a synthetic date, YAML order, a hard-coded
  `adopt-edition > deferred`), a record may now carry `supersedes`: an explicit
  statement that it takes precedence over one earlier applicability record **of
  the same norm identity and the same applicability `recorded_at`**.
  - The pointer reuses the exact append-only record identity `instruction-intake.md`
    defines and `rule-resolution.md` §9 names — `register` + artifact (`path`) +
    region (`region`) + `recorded_at` + `verdict` (+ `revision` when the register
    is revision-identified) — it invents no parallel identity and no free-text
    ordering rule. `path`/`region` must equal the carrying record's own
    `norm.path`/`norm.region`. Optional; `additionalProperties: false`.
    **Forbidden when `source` is `kernel`** (no intake identity to point at, no
    verdict multiplicity to disambiguate — the same reason `intake_record` is
    forbidden there). `schema_version` stays `1`: this is a backward-compatible
    optional field, every existing document remains valid and no Instance data
    is migrated.
  - Every declared `supersedes` relationship is data that is validated
    fail-closed — a `supersedes` is never harmless stray data, including on a
    record that is alone on its applicability date (it has no same-date target).
    The greatest applicability `recorded_at` then decides which cohort is
    authoritative.
  - `resolver-output.schema.json` unchanged — the output shape does not change:
    a resolved tie yields an ordinary `applicable_norms` / `unresolved_applicability`
    entry for the head, an unresolvable tie still fails closed with exit 2.
  - `VERSION` unchanged: this package is not a release.

- **`verification/functional-parity/refactor-protocol.md` — the canonical
  REFACTOR execution protocol (PHASE E of `MERIDIAN-RULE-RESOLUTION-001`).**
  A `document_type: protocol` Kernel document in the functional-parity
  verification unit, beside the PHASE D evidence contract it executes against.
  It designs the **order** a `REFACTOR` is carried out in —
  `CLASSIFY → DEFINE PARITY → CAPTURE BASELINE → IMPLEMENT → CAPTURE POST-CHANGE
  → COMPARE → REPORT` — and nothing else: it references the PHASE D contract as
  already-defined at the capture and compare steps and introduces no second
  evidence contract, no new evidence kind, and no new record field. It resolves
  open decision §8.3 of the normative model (phase names and structure) in
  favour of a structure oriented on the evidence contract rather than a
  re-use of the bugfix phases.
  - All four preserved-contract facets (`public_api`, `observable_io`,
    `side_effects_and_interactions`, `user_visible_behavior`) are addressed at
    DEFINE PARITY; per-assertion evidence selection from the contract's §5 set,
    with no kind — snapshot included — mandatory, primary or default; honest
    per-assertion `UNVERIFIED` recorded at COMPARE, with a record-scoped gap or
    an unestablished baseline forcing every assertion and the overall
    `UNVERIFIED`.
  - The safe invariant for a defect found inside a `REFACTOR` (normative model
    §1.4.1) is step COMPARE: the difference is not normalised into the baseline
    or a snapshot, not fixed inside the `REFACTOR` work item; a separate
    `BUGFIX` work item is created or the owner decides; the original `REFACTOR`
    keeps its class and its protocol route and is paused only if the defect
    blocks the parity proof. `git worktree` is named as one possible practice,
    not a step. A file set that widens beyond the resolved `candidate_paths`
    re-resolves before VERIFY.
  - Linked from `verification/README.md` (strategy router and evidence
    boundary), the VERIFY stage and authority invariants of
    `workflows/task-lifecycle.md`, `verification/functional-parity/README.md`,
    `standards/workspace/rule-resolution.md` (§3.1, §3.2, §7, §10) and
    `functional-parity-evidence-contract.md` (intro and §12).
  - `VERSION` unchanged: this package is not a release.

- **`AGENTS.md` §6 and `standards/workspace/version-control-flow.md` §5 / §9 —
  support for an owner-managed Merge Request (MR) as the final step of feature
  integration, when an accepted tracked protocol declares it.** The Git
  integrator still runs independent review, prepares the accepted feature
  package and its single package commit, prepares the branch for publication,
  and — after the merge — verifies the resulting history and re-runs the gates;
  but in this mode it does **not** advance the integration line by a local
  `git merge`. The **owner** performs the merge in the platform's web interface
  (GitLab, GitHub or another); publishing the branch and opening the MR are
  external actions done on the owner's instruction or by the declaring
  protocol, and if the integrator cannot do them it hands the owner the exact
  command and source/target rather than working around the limit. The external
  executor still performs no Git write of any kind.
  - New `version-control-flow.md` §5.3 states the rule; §5 and §9 are made
    consistent with it (the feature-branch merge is the integrator's step
    *unless* a tracked protocol assigns it to the owner; publishing a branch
    and opening an MR are external actions, not triggered by this standard).
  - The MR is merged as an **ordinary merge commit that keeps the accepted
    package commit** — squash, rebase and a fast-forward with no merge commit
    are forbidden; the package commit must stay reachable from the integration
    line and a distinct merge commit must appear. If the platform offers no
    such method the owner reports a blocker instead of merging. Post-merge
    verification checks: package commit reachable, distinct merge commit
    present, package diff not rewritten, gates pass.
  - Scoped to the declaring protocol only: Meridian's base
    owner-plus-one-executor model and every other repository are unchanged, an
    MR is **not** a universal Meridian requirement, and the promotion mode, the
    stable line, the release flow and the non-fast-forward advancement-commit
    rules are untouched — the MR concerns only the merge of a feature branch
    into the integration line, and pushing a feature branch is not a push of
    the integration line, the stable line, a release or a tag. No new schema or
    machine field: a text rule suffices.
  - `VERSION` unchanged: this package is not a release.

- **`standards/workspace/version-control-flow.md` §5.4 and `AGENTS.md` §6 — the
  lifecycle of a temporary Git worktree used for one task.** New
  `version-control-flow.md` §5.4 makes the Git integrator the owner of a
  temporary worktree's creation, accounting and cleanup — for the executor
  `git worktree add` / `remove` are Git writes it does not perform — and states
  a closed set of rules for it:
  - **Creation is allowed only** for one of four reasons: parallel work,
    someone else's unfinished working-directory state, an independent source
    revision, or a tool's technical requirement. Creating a worktree merely for
    a checkpoint, a result handoff, documentation, or a short isolated step is
    forbidden.
  - A `CHANGES_REQUESTED` cycle **reuses the same worktree**; a new verdict does
    not spawn a new one.
  - After `ACCEPTED` the result is **first fixed by a branch on the accepted
    commit**, and only then is the worktree removed. Removal requires a clean
    (empty) `git status --porcelain` and runs the fixed sequence
    `git worktree remove` → `git worktree prune` → `git worktree list`.
    `--force`, manual directory deletion, and removing an unknown / foreign /
    unfinished worktree are forbidden. Removing a worktree does **not** delete
    its branch.
  - A task closes in only one of three worktree states — `not_created`,
    `removed`, or `retained`; `retained` requires a reason, a responsible
    owner, and a verifiable future-cleanup condition. At task close the Git
    integrator reviews the repository's worktree register (`git worktree list`)
    and removes the clean temporary worktrees of finished tasks not moved to
    `retained`.
  - The norm does **not** auto-remove an existing permanent worktree created as
    part of a separate migration or data-provenance mechanism; such a tree may
    remain only as `retained` with a reason, a responsible owner, and a cleanup
    condition. No new schema, machine field, task-type model, or handoff schema
    is introduced.
  - `AGENTS.md` §6 adds only a short pointer to §5.4 from the Git integrator's
    role behavior; the normative text is not duplicated.
  - `VERSION` unchanged: this package is not a release.

### Changed

- **`scripts/rule-resolver.mjs` — current-source provenance is verified against
  the AUTHORITATIVE applicability record only; a superseded historical record's
  now-stale digest no longer blocks a newer authoritative record
  (MERIDIAN-RULE-RESOLUTION — authoritative provenance).** The applicability
  register is append-only, so after a norm's text legitimately changes an older
  record's `digest` necessarily differs from the single current text the
  resolver is handed. `resolveNorms()` previously recomputed that current text
  against **every** matching record's digest, so the older record failed closed
  on "stale digest" and the newer authoritative record could never take effect.
  What is unchanged and still fail-closed:
  - schema-shape validation of every applicability record;
  - `pickAuthoritative()` and the full `supersedes`-graph validation of **every**
    cohort — current and historical alike (a dangling / other-date / ambiguous /
    cross-identity / self pointer, a cycle, a multi-head cohort);
  - the exact `intake_record` pointer of **every** matching non-kernel record
    (historical included) resolving to exactly one append-only intake record —
    a missing or ambiguous pointer is still fail-closed, and a newer
    authoritative record does not excuse it;
  - `verifyProvenance()` on the authoritative record: its current text must be
    supplied and hash to **its** digest, a stale authoritative digest is still
    an error, and its `delivery` comes only from its own resolved intake pointer
    (`kernel-doc` for `source: kernel`) — never a historical record's delivery
    or digest.
  What changes: the current-text SHA-256 comparison runs for the authoritative
  matching record only. A newer authoritative `resolved` legitimately supersedes
  an older `unresolved` carrying an old digest; a newer authoritative
  `unresolved` likewise suppresses an older `resolved`. Resolver output schema
  and result shape are unchanged; resolution stays byte-identical under
  input-record reordering. `VERSION` unchanged: not a release.
  - `standards/workspace/rule-resolution.md` §9 — a new paragraph states the
    model normatively: (1) schema + `supersedes` graph of every cohort stay
    fail-closed and pick the authoritative cohort; (2) the `intake_record`
    pointer of every matching non-kernel record still resolves fail-closed;
    (3) the single current text is hashed only against the authoritative
    record's `digest` — a historical digest records a since-superseded state and
    is not recomputed against today's text, a stale authoritative digest stays
    an error; (4) no fallback to a historical record's delivery / digest.
  - `test/rule-resolver.test.mjs` — six regression tests (89 → 95): a newer
    `resolved` supersedes an older `unresolved` whose digest is now stale (and
    the reverse direction); a stale **authoritative** digest still fails closed;
    a missing intake pointer on a matching **historical** record still fails
    closed despite a newer authoritative record; the changed-text resolution is
    byte-identical under reversed input order; and a dangling `supersedes` in a
    historical cohort still fails closed under the authoritative-only text check.
    The suite header comment (which claimed current-digest verification for
    *every* matching record) is corrected.

- **`scripts/rule-resolver.mjs` — `pickAuthoritative()` validates every declared
  `supersedes` relationship, then selects by date (MERIDIAN-RULE-RESOLUTION —
  same-day precedence).** Two steps, in order: **(1) validate.** The norm group
  is split into applicability `recorded_at` cohorts and the `supersedes` graph
  of **every** cohort — the current greatest-date one and every historical one —
  is validated fail-closed; a `supersedes` on a record that is alone on its date
  is validated too and fails closed, because it has no same-date target (it is
  never treated as harmless stray data). **(2) select.** The greatest
  applicability `recorded_at` picks the current cohort; `recorded_at` keeps its
  meaning and is never a synthetic sequence. A unique record in that cohort is
  authoritative. A tied current cohort is authoritative only through a valid
  one-head `supersedes` graph; a tied cohort with no relationship still fails
  closed. Fail-closed, each with its own diagnostic: a `supersedes` target that
  resolves to no record, to a record on another applicability date, to more than
  one record, to another norm identity, to the record itself, a cohort whose
  edges form a cycle, or a cohort that declares ordering yet leaves more than
  one un-superseded head. New helpers `resolveSupersedesEdge()`,
  `validateCohortSupersedes()`, `matchesIntakePointer()`,
  `assertNoSupersedesCycle()` (three-colour DFS per cohort). Existing provenance,
  digest, `unresolved` status, scope, activation and **different-date**
  precedence (with no `supersedes`) are unchanged; the intake verdict is never a
  precedence key. Resolution stays byte-identical under input-record reordering.
  - `standards/workspace/rule-resolution.md` §4 and §9 — the rule is stated
    normatively. §4: every declared `supersedes` is validated before the
    date-based selection, in the current and in every historical cohort; a
    shared greatest `recorded_at` is `unresolved`/fail-closed unless an explicit
    `supersedes` relationship among the same-date records resolves it; a
    `supersedes` on a record alone on its date fails closed; `recorded_at` is
    not a synthetic sequence and the intake verdict is not a precedence key. §9:
    the `supersedes` field reuses the §9 record identity, orders only records of
    one norm identity **and one applicability date**, and the resolver validates
    the graph of every cohort fail-closed (no/other-date/ambiguous/cross-identity
    /self target, cycle, multi-head cohort).
  - `registries/rule-resolution/fixtures/rule-resolution.fixtures.json` — one
    `valid` case (a same-day pair ordered by `supersedes`) and three `invalid`
    cases (a pointer missing its `verdict`, a `supersedes` on a `kernel`-source
    record, an unknown property in the pointer).
  - `test/rule-resolver.test.mjs` — 19 same-day precedence regression tests
    (70 → 89). The former "a unique greatest `recorded_at` … ignores a stray
    `supersedes`" test is **replaced** with fail-closed coverage: a
    dangling / cross-identity / self / other-date pointer on the unique latest
    record; an invalid (cyclic, multi-head) relationship in a historical cohort
    while a newer unique record exists; a valid historical same-day cohort
    followed by a newer unique date (validates, then the newer date wins);
    different-date records with no relationship keep the existing precedence; and
    byte-identical output under reversed input for every successful case — plus
    the retained valid-head, no-relationship, verdict-never-orders,
    missing/ambiguous target, multiple-heads, and malformed / `kernel`-source
    boundary cases.
  - `registries/rule-resolution/resolver-output.schema.json` unchanged — the
    output shape does not change. `VERSION` unchanged: not a release.
- **`scripts/rule-resolver.mjs` — `REFACTOR` now routes its own protocol with
  provenance instead of returning `unresolved_applicability`.** The PHASE D
  evidence contract and the PHASE E execution protocol exist, so a `REFACTOR`
  work item is routed through `routeProtocols()` like every other class and its
  `applicable_protocols` entry carries `source`, `scope` and `digest`/`revision`
  (`rule-resolution.md` §7). The stale `subject: "REFACTOR"` /
  "does not exist yet (PHASE D) … not designed yet (PHASE E)" entry is removed
  from the resolver and its tests. The defect-in-`REFACTOR` invariant is
  unchanged: with a `prior_state.refactor_findings` entry the resolver still
  emits `refactor-in-progress-finding` in `unresolved_applicability` — now
  **alongside** the resolved protocol route, not instead of it — and the
  `REFACTOR` class is never reclassified.
  - `test/fixtures/rule-resolver.fixtures.json` — a Kernel-universal
    `REFACTOR → refactor-protocol` route added to `protocol_routes`.
  - `test/rule-resolver.test.mjs` — acceptance case 5 renamed and rewritten to
    assert the provenance-carrying route and the absence of the `REFACTOR`
    unresolved entry; case 6 rewritten to assert the finding entry sits beside
    the parent's `refactor-protocol` route.
  - `resolver-output.schema.json` unchanged — `routed_from` already admitted
    `REFACTOR`.
- **`standards/workspace/rule-resolution.md` §3.1, §3.2, §7, §10 — the "PHASE
  D/E do not exist yet" language is removed.** §3.1 and §10 now state that both
  artefacts exist and are authoritative and that the resolver routes `REFACTOR`
  to the protocol with provenance; §3.2 points at the protocol's COMPARE step
  for the safe invariant and records that the resolver returns the route and the
  finding side by side; §7 lists `REFACTOR → refactor-protocol` as a
  Kernel-universal route example. No new evidence contract is introduced and the
  PHASE D contract is not weakened.
- **`skills/bugfix-protocol/SKILL.md` step 0 — `REFACTOR` added as a fourth,
  BUGFIX-incompatible change class.** The classifier now names all four classes
  and routes `REFACTOR` out to
  `$MERIDIAN_KERNEL/verification/functional-parity/refactor-protocol.md`, with a
  note that a defect found during a `REFACTOR` is not fixed under this skill.
  `skills/bugfix-protocol/PIN.yaml` — `sha256` re-pinned
  (`269930b0…` → `67f55d7d…`), `pinned_at` set to `2026-08-29`, and a
  `transformations` entry recording the Kernel-side step-0 extension;
  `upstream.sha256` is untouched.
  contract for `REFACTOR` (PHASE D of `MERIDIAN-RULE-RESOLUTION-001`).** A new
  verification unit beside `regression-testing/` and `smoke-protocol/` in the
  verification router, answering one question: how the observable behaviour of
  the system before an internal change is fixed and compared with the behaviour
  after it.
  - `functional-parity-evidence-contract.md` (`document_type: standard`) — the
    normative contract. The preserved observable contract across four facets
    (`public_api`, `observable_io`, `side_effects_and_interactions`,
    `user_visible_behavior`), with assertion ids unique across the whole record
    and each under one owning facet; the baseline before the change (identifiable
    source state, identified inputs and observation conditions, provenance, the
    result actually observed); the post-change evidence (state after the change;
    conditions that are either `same` — referencing every baseline condition id
    and no others — or `explicitly-comparable` with a justification; the observed
    result; contract links that resolve to a declared assertion's exact
    `{facet, id}` pair); six neutral evidence kinds with the limitations of each;
    the public-contract snapshot as **one** kind among them — permitted only with
    an explicit applicability justification, never the primary or default proof,
    its limits named; explicit gaps and `UNVERIFIED` — an incomplete baseline,
    non-comparable conditions, a missing facet, incomplete post-change evidence
    or an unjustified kind leave the affected assertion, or the whole claim,
    `UNVERIFIED`, a narrow kind is not widened into a broader claim, a
    per-assertion `VERIFIED` needs both a covering evidence entry and a resolving
    link, and an unestablished baseline forces every assertion and the overall
    `UNVERIFIED`; the product/repository boundary — concrete test runners,
    snapshot frameworks, commands, configuration and test paths stay
    Instance/repository data; and the per-assertion and overall verdict rules.
    The contract defines requirements and form; it does not design the order in
    which a `REFACTOR` is executed (that is PHASE E).
  - `functional-parity-evidence.schema.json` — JSON Schema for one evidence
    record, using only keyword and format subsets `scripts/kernel-validate.mjs`
    implements. `records` is non-empty (`minItems: 1`). Every facet must be
    addressed (an assertion or a stated not-applicable justification, never a
    blank); each baseline condition carries a stable `id`; `relationship: same`
    carries `baseline_condition_ids` and neither restated `items` nor a
    justification, `explicitly-comparable` carries `items` and a comparability
    justification and no id references; `contract_links` is always present and
    may be empty; every evidence entry names at least one limitation; a
    `public-contract-snapshot` entry requires an applicability justification; an
    `UNVERIFIED` per-assertion state or overall verdict requires a reason;
    `additionalProperties: false` throughout, so an unrecognised form extension
    fails closed. No `kind` value and no required field names a product tool.
  - `fixtures/functional-parity-evidence.fixtures.json` — product-neutral valid
    and invalid fixtures (no real repository, path, URL, runner, framework or
    command): 8 valid, 29 invalid. Valid records show a snapshot is not required
    where other kinds cover the claim, that different single kinds each form a
    valid record, that `explicitly-comparable` conditions with a justification
    are accepted, a mixed record with one assertion `VERIFIED`-with-link and one
    `UNVERIFIED`-without-link, that an incomplete or unestablished baseline
    yields `UNVERIFIED` for every assertion, and an honest all-`UNVERIFIED`
    record with an empty `contract_links` array. Invalid records cover an empty
    record set, an absent baseline, absent or non-comparable post-change
    evidence, a `same` relation that carries `items` / references a non-baseline
    condition / reproduces only some conditions, a duplicate baseline condition
    id, a blank facet, a snapshot with no applicability justification, an empty
    limitations list, an unknown property, an empty observation, a missing
    verdict reason, and — schema-clean but rejected by the inference rules — a
    record-scoped gap under a `VERIFIED` overall, a record-scoped gap that leaves
    an assertion `VERIFIED`, an `UNVERIFIED` assertion under a `VERIFIED`
    overall, evidence covering an undeclared assertion, a declared assertion with
    no verdict, a `VERIFIED` assertion no evidence covers, a `VERIFIED` assertion
    with no resolving link, a contract link under the wrong facet, a duplicate
    assertion id across facets, an unestablished baseline that still carries a
    `VERIFIED` assertion, and identical baseline / post-change source states.
  - `scripts/kernel-validate.mjs` — new check `functional-parity` (`6c` in the
    header). The schema is parsed and walked for unsupported keywords; the
    fixtures bundle is classified fail-closed on its own shape (a non-empty
    array, exactly one group for the schema, non-empty `valid` and `invalid`),
    exactly as the `rule-resolution` block is. Beyond the schema,
    `functionalParityConsistency()` enforces the record-level inference rules
    the draft-07 subset cannot state: the document carries at least one record;
    assertion ids unique record-wide, each under one owning facet; every
    per-assertion verdict names a declared assertion and every declared assertion
    carries exactly one; every `covers` id resolves, and every `contract_links`
    entry resolves to the exact `{facet, assertion_id}` pair; a `VERIFIED`
    assertion has both a covering evidence entry and a resolving link (an empty
    `contract_links` array is legal only when nothing is `VERIFIED`); any
    `UNVERIFIED` per-assertion state forces an `UNVERIFIED` overall; a
    record-scoped gap forces every declared per-assertion verdict and the overall
    `UNVERIFIED`, an assertion-scoped gap only the assertions it names;
    `relationship: same` references every baseline condition id and no others
    with no duplicate baseline condition id; an unestablished baseline forces
    every declared per-assertion verdict and the overall `UNVERIFIED`; the
    baseline and post-change source states are a distinguishable
    `{identifier_kind, identifier}` pair. `kernel-validate` remains the gate;
    this is not a new script.
  - `test/kernel-validate.test.mjs` — new adversarial cases (t126–t158) proving
    the check goes red on a malformed schema, missing fixtures, a misclassified
    fixture, an unsupported schema keyword, each fail-closed bundle-shape
    violation, and each negative case of the contract (an empty record set, a
    missing baseline, missing or non-comparable post-change evidence, a
    mechanically-broken `same` relation, a duplicate baseline condition id, an
    unclosed gap left `VERIFIED`, a record-scoped gap that still leaves an
    assertion `VERIFIED`, identical before/after source states, a snapshot with
    no applicability justification, a `VERIFIED` assertion with no resolving
    link, a contract link under the wrong facet, a duplicate assertion id, an
    unestablished baseline still carrying a `VERIFIED` assertion, an unknown form
    extension), plus green cases proving a single non-snapshot kind, an
    `explicitly-comparable`-with-justification record, the mixed
    `VERIFIED`/`UNVERIFIED` record, the honest all-`UNVERIFIED` unestablished
    baseline, an honest all-`UNVERIFIED` record with an empty `contract_links`
    array, and the same identifier string under a different `identifier_kind` are
    accepted. Existing PHASE B / PHASE C checks are unchanged and still pass.
  - Linked from `verification/README.md` (strategy router and evidence
    boundary), the VERIFY stage of `workflows/task-lifecycle.md`, the Kernel
    file list in `standards/workspace/kernel-boundary.md`, and the structure
    tree in `README.md`.
  - `VERSION` unchanged: this package is not a release.

### Fixed

- **`hooks/pre-push` now binds its gates to the Kernel being pushed instead of
  inheriting the caller's Git environment.** Two problems in the pre-push hook,
  both of which made the gates read the wrong tree:
  - Git runs the hook with `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE` and the
    rest of `git rev-parse --local-env-vars` exported. The regression suites
    build throwaway synthetic repositories and run `git init/add/commit` inside
    them; those child `git` invocations were inheriting the caller's variables
    and operating on the repository being pushed — a synthetic test had
    committed straight onto a Kernel branch that way, and an ordinary push was
    blocked as a side effect. The hook now computes `ROOT` while the Git
    environment is still intact and then sources the new
    `hooks/lib/git-env-isolate.sh`, which unsets every repository-local Git
    variable (the list comes from `git rev-parse --local-env-vars`, not a
    hard-coded copy) before any gate runs. The helper is fail-closed: it checks
    the exit status of `git rev-parse --local-env-vars` explicitly instead of
    swallowing a failure into a successful empty list, so on any non-zero exit
    it prints a diagnostic and returns non-zero and the hook — under `set -e` —
    exits before a single gate is started. The helper does not touch
    `MERIDIAN_*` or `PATH`.
  - `scripts/kernel-validate.mjs` and `scripts/rule-resolver.mjs` prefer
    `$MERIDIAN_KERNEL` over their own location, so an ambient `MERIDIAN_KERNEL`
    left in the operator's shell (a different, possibly dirty checkout) made the
    gates validate that other tree and falsely block the push. After the scrub
    the hook now pins `export MERIDIAN_KERNEL="$ROOT"` — the worktree Git is
    pushing from — so every gate sees the pushed Kernel. `MERIDIAN_INSTANCE` is
    left as the owner set it: the fixture gate still overrides it inline to
    `$ROOT/test/instance-fixture`, and the optional logged real-Instance run
    still uses the value passed in. No `--no-verify` is needed.

  New regression test `test/pre-push-git-isolation.test.mjs` — wired into
  `hooks/pre-push` and `.github/workflows/gate.yml` — runs the verbatim
  production hook and helper against a synthetic Kernel repo with gate shims
  that record what each gate received, and drives a normal `git push` to a
  local bare remote with no `--no-verify`. It proves the Git environment is
  scrubbed before the first gate and that every gate sees `MERIDIAN_KERNEL`
  equal to the pushed root even when a stale ambient value is set; deleting the
  isolation sourcing line or the `MERIDIAN_KERNEL` pin makes the push fail
  closed and the test red; a failing `git rev-parse --local-env-vars` fails the
  hook with no gate started; and the caller and real Kernel HEADs are unchanged
  throughout. Nothing touches the network or any real repository.

### Added

- **Deterministic rule resolver (PHASE C of `MERIDIAN-RULE-RESOLUTION-001`).**
  `scripts/rule-resolver.mjs` — a read-only Kernel mechanic beside
  `kernel-validate.mjs`, not an extension of it; the validator stays the gate.
  A pure core `resolveRules(workItem, sources)` plus a CLI wrapper: the work
  item is exactly the §3.1 shape of the normative model, `sources` are
  explicitly injected environmental data (repository inventory, PHASE B
  applicability records, instruction-intake registers, and the not-yet-
  standardised protocol/verification route records), and the same input on the
  same source revision returns a byte-identical, identically ordered object
  that satisfies `registries/rule-resolution/resolver-output.schema.json`.
  Axes are checked separately and conjunctively; a technology profile never
  yields an architecture profile; `architecture_profile` absence is not by
  itself unresolved; a `deferred` intake verdict does not disable a norm.
  Path-glob: every separate `**` is zero or more whole path segments, several
  and adjacent `**` are supported (`**/**/x`, `a/**/**/b`) and collapse to one,
  and any mask outside the declared grammar is refused, never approximated.
  Protocol routes are filtered against the work context — the injected route
  shape carries its own exact `repository` / `product_domain` selectors — before
  any conflict is computed, so a repository route for one repository never
  reaches another and a local/universal disagreement that survives scope
  filtering is a `conflicts` entry, not a silent override. Provenance is
  verified for every applicability record that matches the work item —
  `status: resolved`, `status: unresolved` and `activation: undetermined`
  alike: each must have a resolvable exact intake pointer and a supplied norm
  text/region whose SHA-256 equals the recorded digest; records irrelevant on
  scope/path/task-class need no text. Append-only precedence: the authoritative
  latest applicability record decides one norm identity — a newer `unresolved`
  suppresses an older `resolved` and a newer `resolved` suppresses an older
  `unresolved`; records that cannot be ordered within the available
  identity/date are fail-closed, never ordered by JSON lexical order. A
  container norm's region is recomputed through the shared marked-region reader:
  markers are located in the fenced-blanked buffer (a marker quoted in a fenced
  example is not a declaration), but the text handed to the digest is the
  region's verbatim source slice — `instructionRegions().regions[].sourceText`,
  every character of the region between its markers intact, fenced code
  included — never the space-blanked parser view; a named region that is
  missing, duplicated or unclosed is fail-closed, never a fallback to the whole
  file. The CLI's `--applicability` input is the whole PHASE B register
  envelope: it is validated against
  `registries/rule-resolution/applicability.schema.json` with the shared engine
  before records are touched, so a bare `{}`, a missing `records`, a wrong
  `schema_version`, an additional property, a non-array `records` or a
  top-level raw array is fail-closed with exit 2 — `records` is never coerced
  to `[]`. The pure core still takes an injected `applicability_records` array.
  The work item is validated strictly against §3.1:
  `candidate_paths` and `changed_paths` are mandatory string arrays and a
  missing or non-array value is not coerced to `[]`, an unknown work-item field
  and a malformed `declared_profiles` are rejected. Repository ids, intake
  pointers and route keys match exactly, with no fuzzy/path/basename fallback;
  `changed_paths` that widen the prior `candidate_paths` set
  `requires_reresolution`; `REFACTOR` gets `unresolved_applicability`, never
  another class's protocol, until PHASE D/E; a defect found inside a `REFACTOR`
  needs a separate `BUGFIX` child or an owner decision and does not reclassify
  the original work; an undecomposed `initiative` returns
  `decomposition_required`, the declared decomposition protocol or `null`,
  pre-decomposition norms and separate `unresolved_items`; a reviewer
  assignment is not an input and does not change the result.

- **Shared pure helpers under `scripts/lib/`.** The YAML subset reader
  (`scripts/lib/yaml.mjs`), the JSON Schema subset engine
  (`scripts/lib/json-schema.mjs`) and the marked-region reader
  (`scripts/lib/regions.mjs` — `blankFencedBlocks`, `markedRegion`,
  `instructionRegions`) moved verbatim out of `kernel-validate.mjs`, which now
  imports them, so the validator and the resolver read YAML, validate against
  JSON Schema and parse marked regions through one implementation each rather
  than a second copy. The documented subsets, the supported keyword set, the
  format checks, the region parsing rules and every throw are unchanged; the
  validator's regression suite is unaffected. `instructionRegions()` additionally
  exposes, per region, a `sourceText` (the verbatim slice of the original input
  between the markers) beside the existing `text` (the fenced-blanked parser
  view) — an additive field; the validator keeps reading `text`.

- **Regression coverage for the resolver.** `test/rule-resolver.test.mjs` and
  the product-neutral `test/fixtures/rule-resolver.fixtures.json`: one named
  test per acceptance case of §4/§9 of the normative model (cases 9 and 10 are
  separate tests), plus a genuine `status: unresolved` case surfaced under its
  own resume condition; append-only precedence in both directions and its
  unorderable-records fail-closed; cross-repository and product-domain protocol
  route isolation; missing/duplicate/unclosed region, a fenced-example marker,
  and a region whose `sourceText` keeps fenced code verbatim and hashes to the
  same SHA-256 as the raw slice between its markers; a valid `--applicability`
  envelope resolving via the CLI and malformed envelopes (no `records`, bare
  `{}`, wrong `schema_version`, non-array `records`, an extra property, a raw
  array) each fail-closed with exit 2;
  missing/non-array/non-string `candidate_paths` and `changed_paths`;
  negative tests for a stale digest, a missing text and an unresolvable intake
  pointer on `unresolved` and `undetermined` records as well as `resolved`
  ones; the new several/adjacent `**` glob cases; and the pre-existing
  fail-closed paths (unknown repository id, `work_kind`/`change_class` pairing,
  malformed applicability record, unsupported glob, incompatible mandatory
  routes, missing mandatory source), stable ordering and output-schema
  conformance. Wired into `.github/workflows/gate.yml` and `hooks/pre-push`;
  `README.md` documents the CLI and the test command.
  `instance-template/hooks/pre-push` is left untouched — it checks published
  Instance state, not Kernel unit tests. `VERSION` unchanged; no release, no
  tag.

### Breaking

Относительно `0.4.x` (пакет `git-governance-migration`,
`standards/workspace/version-control-flow.md`):

- **Для репозиториев Meridian введён закрытый шаблон имён временных ветвей**
  (§3, §3.1). Временная ветвь вне разрешённого закрытого набора —
  `feature` / `bugfix` / `chore` / `docs` / `refactor` / `test` / `ci` /
  `build` (обычные) и `release` / `hotfix` / `promotion` (специальные) — норме
  больше не соответствует.
- **Имя временной ветви обязано быть английским, строчным и в формате
  kebab-case** (§3.1). Неанглийское имя, транслитерация русских слов, верхний
  регистр и неправильный kebab-case норме не соответствуют.
- **Введена обязательная форма заголовка и тела сообщений коммитов Meridian**
  (§12): типизированный заголовок и четыре обязательных раздела тела
  `Что изменено` / `Зачем` / `Проверки` / `Связано`.
- **Instance, переходящий на Kernel `0.5.x`, принимает эти общие нормы отдельным
  репозиторий-локальным пакетом** (`COMPATIBILITY.md`, строка `0.5.x`).
- **Существующие ветви и опубликованная история Instance автоматически не
  переименовываются и не переписываются**: переход конкретного Instance
  затрагивает только его новые ветви и коммиты.
- **Постоянные линии `main` / `dev`, их платформенная защита и операционное
  переименование относятся только к репозиторию Kernel** (§1.1, §1.2, §13,
  §10.1); на Instance и delivery adapters они не распространяются.
- **Физическое переименование `master` → `main` и `develop` → `dev` выполняется
  после выпуска `0.5.0` отдельным операционным этапом** (§10.1) и **в
  release-коммит `0.5.0` не входит**.

## [0.4.1] — 2026-09-01 (`draft`)

### Fixed

- **Восстановление стабильной линии после ошибочного слияния GitHub PR #1 в
  `master`.** PR #1 (`feature/owner-merge-request-integration`) должен был идти
  в интеграционную линию `develop`, но был направлен и слит в стабильную линию
  `master` — merge-коммит `8c48d77`. Этим слиянием в стабильную ветку попали
  **30 ещё не выпущенных путей из `develop`** (13 новых файлов и 17 изменённых,
  включая resolver, его библиотеки и тесты, functional-parity контракт,
  pre-push isolation и правку правила owner-managed MR) — состояние, которое
  тег `v0.4.0` не покрывает.
- **Что делает выпуск 0.4.1.** Все файлы, **кроме `VERSION` и `CHANGELOG.md`**,
  возвращены **byte-for-byte** к выпущенному состоянию `v0.4.0` (коммит
  `c875262`): 15 файлов восстановлены из `c875262`, `test/kernel-validate.test.mjs`
  восстановлен из `c875262`, и 13 файлов, которых в `c875262` не было, удалены
  из рабочего дерева. `VERSION` поднят `0.4.0` → `0.4.1`; настоящая запись —
  единственное содержательное изменение `CHANGELOG.md`.
- **Опубликованная история не переписывается.** Merge-коммит `8c48d77` и всё,
  что до него, остаются в истории как есть. Восстановление оформляется новым
  package-коммитом и отдельным release advancement merge-коммитом поверх
  существующей истории; `reset`, `rebase` и force-push не используются
  (`standards/workspace/version-control-flow.md` §8, §10).
- **Совместимость Kernel ↔ Instance не меняется.** 0.4.1 не трогает ни один
  контракт, на который вправе рассчитывать Instance; `COMPATIBILITY.md`
  правки не требует и не изменялся.
- **Принятое владельцем исключение: ветка восстановления не вливается целиком
  обратно в `develop`.** Полное слияние `hotfix/stable-line-recovery` → `develop`
  удалило бы из `develop` правильную незавершённую работу (те самые 30 путей,
  которые в `develop` легитимны). Поэтому обратного слияния всей ветки нет.
- **Синхронизация с `develop` — отдельными ограниченными пакетами.** Сведения о
  выпуске 0.4.1 (`VERSION`, эта запись `CHANGELOG.md`) будут перенесены в
  `develop` отдельным ограниченным пакетом. Правило **owner-managed MR**
  (`AGENTS.md` §6, `version-control-flow.md` §5.3, коллаборационный протокол
  Instance) будет доставлено в `develop` отдельно — правильным PR
  `feature → develop`, а не через эту hotfix-ветку.

## [0.4.0] — 2026-08-27 (`draft`)

### Added

- **Repository reference mechanism — каноническая идентичность и display alias
  разведены.** Единственная каноническая идентичность продуктового репозитория
  — `inventory/repositories.yaml` → `repositories[].id`; display alias adapter
  (`folders[].name` в файле Cursor workspace) идентичностью не является и
  repository scope не создаёт. Новый стандарт
  `standards/workspace/repository-references.md` и схема
  `registries/inventory/repository-references.schema.json` вводят
  типизированную reference-запись (`kind` / `alias` / `repository_id` /
  `sources`) и fail-closed exact resolution: сначала точное совпадение с
  каноническим `id`, иначе точное case-sensitive совпадение с объявленным
  alias — ровно в один `repository_id`; ноль совпадений —
  `unresolved_repository_reference` / STOP, несколько разных — `conflict` /
  STOP; alias выходной идентичностью не становится. Без fuzzy-, remote-,
  path- и basename-подбора. Instance хранит конкретные aliases в
  `inventory/repository-references.yaml`; `instance-template/` несёт пустой
  реестр. `VERSION` не менялся (`0.3.0`), релиз не выполнялся.

- **Режим продвижения — свойство репозитория, а не каждой вложенной release
  unit.** `version-control-flow.md` объявлял себя применимым к Kernel, Instance
  и delivery adapters, но требовал для стабильной линии `release/<semver>`,
  `VERSION`, тег `vX.Y.Z` и SemVer-hotfix — а `release-versioning.md` §8
  одновременно фиксирует Instance как Git-revision без SemVer. Обе нормы
  Instance исполнить не мог. Correction разделяет две сущности: режим
  продвижения описывает **верхнеуровневую (repository-level) release identity
  репозитория**. Закрытый набор: `semver-release` (repository-level `VERSION` и
  repository-level тег `vX.Y.Z`; так объявлен Kernel — `master` / `develop`) и
  `revision-promotion` (repository-level состояние идентифицируется Git SHA
  итогового non-fast-forward advancement commit; repository-level `VERSION` и
  тег не создаются; так объявлен тип репозитория Instance до отдельного решения
  владельца о SemVer — конкретные имена линий и само объявление остаются в
  Instance).

  Вложенная независимо версионируемая единица (`stack-profiles/` в Kernel,
  smoke-пакет в Instance) разведена по **двум независимым осям**. Ось A
  (версионирование): режим репозитория не переопределяет её `VERSION`,
  `CHANGELOG`, SemVer и tag convention; изменение вложенного `VERSION` — не
  повышение repository-level `VERSION` и режим репозитория не выбирает. Ось B
  (Git-flow файлов): изменение вложенной единицы — обычный package change,
  идёт через `feature/<slug>` в интеграционную линию, а в стабильную линию его
  файлы попадают только следующим repository advancement своего режима
  (`release/<semver>` или `promotion/<slug>`); независимость номера единицы не
  разрешает прямую запись в стабильную линию и не создаёт третий способ
  продвижения репозитория; такое advancement не обязано повышать
  repository-level `VERSION` только из-за вложенного. Отдельного flow и нового
  tag namespace для вложенных единиц стандарт не проектирует.

  Общая часть модели сохранена: `feature/<slug>` от интеграционной линии и
  обратно, запрет прямой записи в стабильную линию, first-parent чтение,
  отдельный non-fast-forward advancement commit на каждое продвижение,
  неизменность опубликованной истории, `push` не подразумевается. Режим,
  стабильная и интеграционная линии объявляются в tracked репозиторий-локальном
  источнике governance/конфигурации (для Kernel — сам стандарт). Общее
  ожидаемое правило для типа репозитория (`revision-promotion` для Instance)
  **не заменяет** локальное объявление конкретного репозитория: каждый
  конкретный Instance обязан объявить режим и линии в своём tracked-источнике,
  до этого Git-интегратор fail-closed и не начинает release/promotion
  integration, а противоречащее общему правилу локальное объявление требует
  явного решения владельца. Fail-closed также при нескольких конфликтующих
  объявлениях и при конфликте объявления с repository-level evidence; имя
  `main`/`master`, исторические теги и `VERSION`/тег вложенной единицы
  repository-level evidence не являются.

  `VERSION` не менялся (`0.3.0`), релиз не создавался; оба стандарта остаются
  `maintained`.

- **Git-flow и release versioning приняты как два раздельных нормативных
  стандарта** (`status: maintained`) после независимого review Codex
  2026-08-27. `standards/workspace/version-control-flow.md` вводит роль
  стабильной линии (её имя объявляет репозиторий; для Kernel это `master`, для
  репозитория с веткой по умолчанию `main` — `main`) и интеграционную линию
  `develop`, ветки `feature/<slug>`, `release/<semver>` и `hotfix/<slug>` с
  объявленными точками ветвления и возврата, запрет прямых package-коммитов в
  стабильную линию, раздельные коммиты Kernel и Instance, запрет на
  переписывание опубликованной истории без решения владельца и на неявный
  `push`. Применение перспективное:
  отдельный раздел adoption/migration фиксирует, что история до точки принятия
  нарушением не объявляется, что Kernel сейчас в переходном состоянии (`master`
  с коммитами после `v0.3.0`, `develop` с дополнительными принятыми пакетами) и
  что переход к модели выполняет первый будущий `release`, а миграцию —
  Git-интегратор. `standards/workspace/release-versioning.md` описывает Kernel
  как самостоятельную SemVer release unit с источником версии `VERSION`,
  ведение `CHANGELOG.md` по Keep a Changelog, фиксацию незавершённой работы в
  `Unreleased`, повышение `VERSION` только в `release/<semver>` и переход к
  `1.0.0` только по решению владельца. Стабильная линия читается по first-parent
  history: каждый её шаг — принятый release или hotfix, слитый отдельным
  merge-коммитом (release advancement commit) без fast-forward; аннотированный
  тег `vX.Y.Z` несёт этот merge-коммит и обозначает целостный snapshot, а
  package-коммиты, вошедшие в выпуск, индивидуально не версионируются и своих
  тегов не получают. Правила выбора MAJOR/MINOR/PATCH однозначны. Hotfix
  допустим только для изменения PATCH-класса и повышает patch; изменение,
  требующее MINOR/MAJOR, готовится как `release` соответствующего номера, а не
  как hotfix. `stack-profiles/` и Instance остаются самостоятельными линиями,
  vendored-зависимости — привязанными к SHA.
  Обе темы (`version-control-flow`, `release-versioning`) уже были в пуле
  `instruction-topics`; теперь у них есть принятый нормативный текст.

- **README §6 и `COMPATIBILITY.md` больше не утверждают, что принятого профиля
  версионирования Git-артефактов нет** — оба ссылаются на два принятых
  `maintained`-стандарта; compatibility matrix не тронута. `AGENTS.md` §6
  фиксирует, что при назначенном внешнем исполнителе создание и переключение
  веток, staging, commit, merge и tag принадлежат Git-интегратору; он вправе
  временно
  проиндексировать поимённо ограниченный candidate package, чтобы прогнать
  гейты по новым файлам, а снимает его path-limited операцией
  (`git restore --staged -- <path>...`), не очищая остальной индекс. Такая
  индексация приёмкой не является; исполнитель Git-записей не выполняет.
  `VERSION` не изменён: этот пакет релизом не является.

- **Пул профилей стека — релизная единица `stack-profiles/`.**
  Собственные `VERSION` (0.1.0) и `CHANGELOG.md`, пул в двух половинах
  (`stack-profiles.yaml` — имена и предикаты по манифесту, `stack-profiles.md` —
  сигнатуры между маркерами), схема данных. Три профиля: `vue-spa`,
  `laravel-app`, `node-ts-service`.

  Состав подтверждён манифестами и корпусом норм четырёх репозиториев первой
  волны, а не перечнем найденных технологий. Кандидат `vue-monorepo` отложен до появления монорепозитория в корпусе:
  ни один манифест не объявляет `workspaces`. Решения владельца от 2026-08-21:
  три профиля, ровно один на репозиторий, единица живёт в дереве ядра до
  появления профиля чужого владельца, извод без нейтрального родителя не
  заводится.

- **Сторож неотслеживаемых норм не видел исключённых.** `--exclude-standard`
  отворачивается от всего, что покрывает `.gitignore`, поэтому норма внутри
  исключённого каталога была невидима сильнее, чем просто неотслеживаемая, — и
  проверка, написанная против невидимости, проходила мимо неё молча. Нашлось на
  первом же репозитории: в рабочем месте `.cursor` исключён целиком. Вопрос
  задан в обратную сторону — для артефактов, которые реестр уже называет, — и
  это один дешёвый вызов вместо обхода дерева. Граница объявлена: обнаружить
  исключённую норму, которую никто не записал, эта проверка не может. Кейс t87.

- **Норма вне контроля версий была невидима для проверки полноты.** Список
  артефактов строится из `git ls-files`, поэтому файл, который инструмент
  грузит, но который никто не добавил в индекс, в подсчёт не попадал — и дерево
  объявлялось описанным полностью, пока действующие правила лежали вне этого
  утверждения. В корпусе это не гипотеза: в краевом агенте печати не отслежен
  ни один из девяти файлов инструкций, в службе печати — три из пяти. Проверка
  теперь называет такие файлы отдельной строкой и не засчитывает дерево как
  подтверждённое. Тот же сторож, что ядро получило в R-1, применён там, где
  живёт корпус. Кейс t86.

- **Запись о контейнере без объявленных границ была невыразима.** §3.1 требует
  для такого файла ровно одну запись с вердиктом `deferred`, а схема требовала
  `region` от всякой записи с доставкой `agents-md-section`. Участка у такого
  файла нет, поэтому предписанная протоколом запись не проходила схему, и
  написать её можно было только соврав о доставке — что и делал регрессионный
  кейс t56, единственный, кто этот путь проверял. Исключение внесено в схему
  ровно в одну точку и описано в §3.1; t56 переписан на настоящую доставку,
  добавлен t85 на границу исключения.

- **Четыре документа пакета приняты владельцем** и переведены из `in-review` в
  `maintained`: протокол приёмки, стандарт идентичности агентной нормы, реестр
  тем и реестр профилей стека.

- **Читатель YAML молча терял поля после свёрнутого скаляра с обрезкой.**
  Заголовок `>-` не совпадал с точным сравнением на `>`, поэтому значение
  читалось как обычная строка, а каждая строка под ним — как соседний ключ. В
  первой же настоящей записи приёмки так исчезли пять обязательных полей.
  Указатель обрезки `-` теперь распознаётся, `+` — отвергается: читатель
  не моделирует хвостовые переводы строк и обязан сказать об этом, а не читать
  «примерно так же». Кейсы t83, t84.

- **Корневой набор имён принадлежит релизной единице, а не верхнему уровню.**
  `document-identity` считал прописные имена допустимыми только в корне
  репозитория; стандарт говорит «корень релизной единицы». Расхождение вскрылось
  на первой же вложенной единице: `stack-profiles/CHANGELOG.md` шёл красным.
  Единица опознаётся по собственному `VERSION` — свойство проверяемое, а не
  список, который пришлось бы вести руками. Кейсы t81, t82.

- **Проверка профиля в гейте: объявление и доказательство.**
  `stack-profiles` сверяет две половины пула и падает на расхождении, отвергает
  `universal` внутри пула; `stack-profile` требует объявления от каждой записи
  инвентаризации, отвергает имя вне пула и объявление, которого не подтверждает
  манифест репозитория. Недостижимый манифест даёт UNVERIFIED, не зелёный
  результат. Ни одна половина не работает без второй: выведение профиля из
  манифеста было бы догадкой, объявление без сверки — фактом, свободным молча
  разойтись с реальностью. Отрицательные и положительный кейсы: t71–t80,
  набор 73 → 84.

  Поле `profile` стало обязательным в `registries/inventory/repositories.schema.json`;
  §4.2 `agent-instruction-identity.md` называет место объявления, §8 — оставшуюся
  границу проверки.

- **Стандарт идентичности агентной нормы.**
  `standards/workspace/agent-instruction-identity.md`: класс «агентная норма»
  (правило инструмента, навык, участок файла инструкций, протокол ядра) и
  четыре независимых ответа о нём — тема, жанр, профиль, доставка с активацией.
  Плюс `derived_from` со списком сужений для всякой редакции.

  Понадобился потому, что `document_type` описывает документ, а у предписания,
  которое инструмент грузит агенту автоматически, есть свойства, которых у
  документа нет: оно существует в нескольких редакциях под разные стеки, и
  редакции расходятся молча.

- **Протокол приёмки агентной нормы.**
  `standards/workspace/instruction-intake.md`: две фазы с необратимым порядком,
  полнота описи из перечисления отслеживаемых файлов, закрытый пул из семи
  вердиктов с сигнатурами, реестр append-only, четыре остановки. Вывод нормы из
  обращения — вердикт с обязательным обоснованием, а не удаление файла.

- **Реестр приёмки: схема и проверка гейта.**
  `registries/instruction-intake/` — схема записи (Kernel) под данные в
  `$MERIDIAN_INSTANCE/instruction-intake/<репозиторий>.yaml`. Состав полей
  зависит от вердикта: `adopt-edition` без родителя, его дайджеста и непустого
  списка сужений схему не проходит; `retire` без преемника и без причины — тоже.

  Форма проверяется общим проходом по `$schema` и собственного кода не требует.
  Код понадобился для утверждений, которых схема выразить не может. Три из них
  доказываются в чужом дереве и деградируют в UNVERIFIED, а не в проход:
  полнота (всякая норма из дерева репозитория имеет запись), происхождение
  (редакция произведена от названного текста), дисциплина упаковки (каталог
  пакета назван именем навыка; `SKILL.md` не несёт полей области действия
  правил инструмента). Два доказываются внутри ядра: сохранность (ни одна
  запись не исчезла относительно предыдущей ревизии) и существование темы.
  Кейсы t22–t25, t27.

  Существование темы проверяется, верность — нет: норма с неверно присвоенной
  темой механически неотличима от верно присвоенной. §8 стандарта приведён к
  тому, что реализовано, и разделён на «проверяется всегда» и «проверяется при
  достижимости». Оттуда же убрано утверждение «нормы вне ядра гейту не
  подчиняются»: оно верно для правки и неверно для чтения, а проверка читает их
  через реестр и инвентарь. Сверка профиля с манифестом объявлена отложенной до
  появления релизной единицы профилей — сказана, а не умолчана.

  Отсутствие реестра — INFO «приёмка не начиналась», а не дефект: приёмка это
  процесс, а не предусловие.

- **Реестр тем агентных норм.** `standards/workspace/instruction-topics.md`:
  двадцать одна нейтральная тема с трёхчастными сигнатурами. Темы, названные по
  библиотеке, в ядро не входят: их срок жизни короче срока жизни методологии.

  Пул лежит в двух файлах: имена — в `instruction-topics.yaml`, где гейт
  способен их прочитать, сигнатуры — в `.md`, где читателю нужны три ответа
  рядом. Проверка `instruction-topics` сверяет обе половины и падает на
  расхождении: пул, разложенный по двум местам без такой сверки, это два пула.
  Кейс t26.

  Пул выведен из описи корпуса, а не придуман: каждая тема подтверждена хотя бы
  одной существующей нормой. Одна тема при этом из пула удалена собственным
  правилом — предмет «параметры среды и секреты» нашёлся только фрагментом
  внутри нормы, которая сама распадается, и записан как предсказанная тема с
  условием заведения.

- **Входная страница на простом языке.** `start-here.md`: какую задачу система
  решает, из чего состоит, как выглядит обычный день, чего она стоит. Без
  терминов, пятнадцать минут чтения.

  Понадобилась потому, что входа не было вовсе: `README.md` — техническое
  описание репозитория, а `MANUAL.md` начинался с границы, переменных окружения
  и словаря. Классификация по стандарту идентичности это уже показала —
  `MANUAL.md` был единственным `unclassified` документом ядра, потому что
  совмещал три жанра сразу.

  Теперь разделено: `start-here.md` — `tutorial`, `MANUAL.md` — `how-to` и
  начинается с того, что делать, а не с того, как всё устроено. `unclassified`
  в ядре не осталось ни одного.

- **Предпосылки о форме продукта объявлены и измерены.**
  `standards/workspace/product-assumptions.md`. «Ядро не знает, какой продукт
  обслуживает» — не то же самое, что «работает с любым продуктом»: оно
  предполагает продукт определённой формы, и до сих пор эти предположения жили
  в тексте двадцати трёх нормативных документов как само собой разумеющееся.

  Шесть предпосылок, у каждой сказано, что происходит при её невыполнении:
  контроль версий, несколько репозиториев, каноническая площадка документации,
  автоматические тесты, таск-трекер, несовпадение продукта с инструментом.

  Написаны не из рассуждений, а из прогона против Instance, описывающего сам
  Meridian как продукт — один репозиторий, без wiki, без трекера. Что дал
  прогон: одно-репозиторная форма применима без правок (`inventory-git: 1/1
  confirmed` — впервые эта проверка вообще что-то подтвердила); отсутствие
  канонической площадки ломает жизненный цикл документа так, что гейт этого
  **не видит**, потому что предположение записано в прозе, а не в коде; а
  совпадение продукта с инструментом делает проверку чистоты неудовлетворимой
  по построению.

- **Стандарт идентичности документа и механическая проверка под него.**
  `standards/workspace/document-identity.md`: одно правило имени (регистр по
  уровню, самодостаточное имя, запрет на дату/версию/`final`/`new`/`copy` в
  имени) и один закрытый пул `document_type` вместо двух несовпадающих наборов,
  выросших порознь — локального в статусной модели и публикационного в шаблонах.

  У каждого типа сигнатура из трёх пунктов: на какой вопрос отвечает, что обязан
  утверждать, чего утверждать не вправе. Тип присваивается только при совпадении
  всех трёх. Остаточное присвоение запрещено — «остальные не подошли, значит
  этот» даёт тип, который ничего не означает. Это правило уже действовало для
  `area_type` и перенесено без изменения смысла. `unclassified` — состояние
  классификации: легально, дефектом является только незаписанная причина.

  Проверка `document-identity` в валидаторе устроена как правило, а не как
  список файлов: новый документ не требует правки валидатора. Исключения из
  обязательного Front Matter заданы шаблонами путей — заготовки, завендоренные
  артефакты, данные фикстур. Пять новых кейсов в регрессионном наборе (t16–t20),
  включая тот, что доказывает: обоснованный `unclassified` проходит.

  Запрещена транслитерация: имя пишется по-английски, а не русским словом в
  латинской записи. `naming-standard` — норма, `standardizaciya-imen` — дефект;
  текст внутри документа при этом остаётся русским.

  Граница проверки объявлена в самом стандарте: гейт видит, что тип есть в
  пуле, но не видит, что документ ему соответствует, и не отличает английское
  слово от русского, записанного латиницей.

- Раздел 12.1 статусной модели: `contract`, `protocol`, `skill`, `template`,
  `reference` и `changelog` используют жизненный цикл `standard`. Шесть почти
  одинаковых таблиц были бы шестью местами для расхождения.

- **Слой профилей шаблонов — платформа стала профилем, а не предположением.**
  `standards/templates/CONTRACT.md` — контракт формы публикуемого документа, не
  называющий ни одного инструмента: блок метаданных, порядок полей,
  двуязычность меток, удаление неприменимых строк, отсутствие самоссылки,
  история изменений и перечень восьми типов, которые профиль обязан покрыть.
  `standards/templates/profiles/README.md` — что профиль обязан содержать, чего
  он не вправе переопределять и как добавить новый.
  `standards/templates/profiles/confluence/PROFILE.md` — механика одной
  конкретной платформы, сведённая к таблице «требование контракта → чем
  обеспечено».

  Kernel не выбирает профиль сам: при отсутствии объявления в
  `$MERIDIAN_INSTANCE/product.yaml` состояние — `unresolved`, а не разрешение
  взять единственный существующий профиль потому, что он единственный.

### Changed

- **Тип `skill` выведен из пула жанров** (`document-identity.md` §3.1).
  Первые два столбца его сигнатуры описывали тот же предмет, что и у
  `protocol` — порядок шагов и условие завершения каждого, — другими словами;
  по §2.2 тип присваивается по предмету, а не по формулировке. Третий столбец
  различался, но различие оказалось не жанровым: `skill` был не вправе зависеть
  от фактов одного продукта без объявленного внешнего контекста, а это
  требование к переносимости упаковки. Два типа, совпадающие по предмету и
  различающиеся упаковкой, делают присвоение неоднозначным по построению:
  артефакт совпадает с обеими сигнатурами, то есть по правилу полного
  совпадения не совпадает ни с одной.

  Ограничение переносимости, которое несла третья графа выведенного типа, не
  утрачено вместе с ним: оно сохранено отдельным утверждением —
  `agent-instruction-identity.md` §5.5.

  Следствия: §2.4 получил запрет вводить тип ради упаковки, инструмента или
  места хранения; завендоренные артефакты перечислены в §4 поимённо с типом
  `protocol` вместо строки по каталогу; `skills/README.md` объявляет каталог
  способом поставки, а не жанром; раздел 12.1 статусной модели перечисляет пять
  типов вместо шести; `skill` убран из `DOCUMENT_TYPES` валидатора; кейс t21
  проверяет, что выведенный тип отвергается, — вывод типа доказан механически,
  а не только записан.

- **Всё дерево приведено к правилу имён: шестнадцать переименований.**
  `CONTRACT.md` → `template-contract.md`, `{ADR,RFC,README}.template.md` →
  `{adr,rfc,readme}-template.md`, `PROFILE.md` → `confluence-profile.md`, восемь
  `*.body.md` → `*-body.md`, `PROTOCOL.md` → `smoke-protocol.md`,
  `ACCEPTANCE-GATE.template.md` → `acceptance-gate-template.md`,
  `BOOTSTRAP.md` → `instance-bootstrap.md`. Front Matter добавлен тридцати
  документам.

  Три типа встали против имени файла — ради чего тип и объявляют, а не читают с
  пути: `verification/README.md` и `regression-testing/README.md` оказались
  `standard` (они предписывают, а не навигируют), `registries/environments/README.md`
  — `reference`. `MANUAL.md` получил `unclassified` с записанной причиной:
  он несёт сигнатуры tutorial, how-to и explanation одновременно и не совпадает
  целиком ни с одной.

  Видимый блок статуса убран у четырёх документов: это конвенция опубликованной
  страницы, и рядом с Front Matter он был бы вторым источником истины.

- **Граница описана как «инструмент и продукт», а не через людей вокруг неё.**
  Формулировки про то, что уезжает с автором и остаётся у заказчика, были
  иллюстрацией одной расстановки и затвердели в определение. Правило не
  изменилось — изменилось то, на чём оно основано: `kernel-boundary.md`
  открывается парой инструмент/продукт, правило классификации спрашивает «нужно
  ли править этот файл, чтобы применить инструмент к другому продукту», README и
  `COMPATIBILITY.md` больше не называют работодателя и владельца данных.

- **Девять файлов слоя шаблонов переименованы и перемещены.**
  `standards/templates/CONFLUENCE.md` → `profiles/confluence/PROFILE.md`;
  восемь `*.confluence-template.md` → `profiles/confluence/*.body.md`
  (Tutorial, How-to, Reference, Explanation, RFC, ADR, Incident,
  Problem Record). Тела не изменены — переехали дословно.
  Ссылки обновлены в `standards/README.md`, `document-quality.md`,
  `document-status-model.md`, `kernel-boundary.md` и обоих writers; writers
  больше не называют платформу даже как «текущую».
  Заодно в `document-quality.md` устранена ссылка на путь через junction
  (`docs/agent-standards/…`) — Kernel-документ не должен адресовать сам себя
  через имя, которого нет в чистом клоне.
- **`COMPATIBILITY.md` объявляет `0.4.x` и фиксирует асимметрию проверки
  ссылок.** Текстовая ссылка из Instance на путь внутри Kernel не проверяется
  ничем; поэтому перемещения в нормативных каталогах объявляются строкой, а не
  выводятся из зелёного прогона.
- **The documentation platform is named by role, not by vendor.**
  `standards/workspace/tooling-axes.md` now defines *canonical wiki* as a role
  resolved through `$MERIDIAN_INSTANCE/product.yaml` (`canonical_wiki`), and
  states the only three places a vendor name stays legitimate: a platform
  profile under `standards/templates/`, this changelog, and Instance data.
  Every terminological mention across the standards was renamed to the role:
  `document-status-model.md` (23 occurrences), `document-lifecycle.md`,
  `agent-memory.md`, `agent-workspace.md`, `document-quality.md`, both writers,
  `standards/README.md`, and the RFC / ADR / README templates. First step of
  closing D-3; the template profile layer, the two vendored skills and
  README section 1 follow as separate changes.
- `status: published` is redefined as "the canonical version is published on
  the platform holding the canonical-wiki role", and the definition is
  explicitly **forward-only**: `canonical_url` and `published_at` on documents
  already published are not rewritten retroactively — they record a fact that
  did happen, on the platform that was canonical at the time.
- The self-hosted example in `tooling-axes.md` no longer carries a vendor's
  default cloud address. An example naming a real vendor is the same coupling
  as a rule naming one — which is now what that document says.

No change to validator behaviour: regression suite 15/15, fixture
`0 failing, 0 warnings`, real Instance unchanged at `0 failing, 2 warnings`.

### Fixed

Результат ревью пакета агентных норм: вердикт `changes_requested`, восемь
замечаний, четыре блокирующих. Общая форма всех четырёх одна — **нормативная
гарантия и фактическая проверка разошлись**, то есть ровно тот класс дефекта,
ради которого система и строится, найденный в первом же пакете, написанном по её
правилам. Регрессионный набор вырос с 27 кейсов до 55.

- **Зелёный прогон не охватывал шесть новых файлов** (P1-1). Набор файлов ядра
  строится из `git ls-files`, а новые файлы оставались untracked: код проверялся
  кейсами t21–t27 на синтетическом ядре, документы — ничем. Файлы внесены в
  индекс; в гейт добавлен сторож — untracked-файлы дерева ядра перечисляются
  отдельным предупреждением, а строка «N файлов чисты» больше не читается как
  «проверено всё». Кейсы t28 и t29: невидимый файл назван; он же после внесения
  в индекс даёт красный прогон.

- **Сохранность реестра держалась на привычке, а не на устройстве** (P1-2).
  Сравнение шло с `HEAD` и по множеству путей. Оба выбора не ловили того, ради
  чего проверка написана: после закоммиченного удаления `HEAD` и есть усечённый
  файл, а артефакт, потерявший две записи из трёх, по путям выглядит целым.
  Теперь единица сохранности — запись (артефакт, дата, вердикт), а сверка идёт
  на двух уровнях: по всей истории файла реестра на каждом прогоне и по
  опубликованной ветке при отправке (`MERIDIAN_CHECK_PUBLISHED`, хук
  `instance-template/hooks/pre-push`). Недостижимость удалённой ветки —
  предупреждение «не сверено», не проход. Кейсы t36, t37, t38, t39.

- **Полнота считалась по файлам, а протокол объявляет единицей участок** (P1-3).
  Единица приёмки для `AGENTS.md` и `CLAUDE.md` — объявленный участок. Механика
  объявления описана в протоколе §3.1: парные маркеры-комментарии с `id`,
  `owner` и `generated`. Гейт разбирает их и требует запись на каждый участок
  владельца, отказывает записи на сгенерированный участок, называет строки вне
  всех участков и объявляет полноту по участкам, а не по файлу. Кейсы t49–t54.

- **Стандарт требовал полей, которых не нёс сам** (P1-4). §7 дополнен правилом
  самообъявления: документ ядра объявляет себя агентной нормой полем `delivery`
  и тогда обязан нести все четыре поля. Четыре документа объявлены
  (`agent-instruction-identity.md`, `instruction-intake.md`,
  `workflows/task-lifecycle.md`, `verification/smoke-protocol/smoke-protocol.md`);
  гейт проверяет комплектность и принадлежность значений пулам. Размер
  неохваченного печатается на каждом прогоне: 19 предписывающих документов ядра
  нормой себя не объявили, и §7.2 называет это объявленным долгом, а не
  результатом. Кейсы t45–t48.

- **`derived_from` не был ссылкой** (P2-5). Голый путь разрешается в разный
  текст в разном чекауте и в разный момент. Теперь это тройка «репозиторий,
  путь, ревизия», а дайджест сверяется с содержимым на названной ревизии.
  Вместе с этим исправлен смысл проверки: законное изменение родителя после
  того, как редакция взята, — не дефект записи, а **отставшая редакция**, то
  есть предупреждение. Красный прогон остался за неразрешимой ссылкой и за
  ссылкой на другой текст. Прежний вердикт учил переставлять дату записи —
  единственный ремонт, который протокол запрещает. Кейсы t24, t40–t44.

- **Обоснование вывода типа `skill` содержало ложное фактическое утверждение**
  (P2-6). «Дословно совпадала» — неправда: совпадал предмет первых двух граф
  сигнатуры, третья различалась. Формулировка исправлена в трёх местах
  (`document-identity.md` §3.1, `agent-instruction-identity.md` §5.4 и запись
  выше), а ограничение переносимости, которое несла третья графа, сохранено
  отдельным утверждением — `agent-instruction-identity.md` §5.5.

- **Пул тем отключался fail-open, а его парсер был шире объявленного** (P2-7).
  Отсутствие пула теперь красный прогон: без него нельзя судить ни одну тему, а
  предупреждение пропустило бы реестр с выдуманными темами. Сигнатуры читаются
  только между маркерами `topic-pool`; любая будущая таблица с идентификатором в
  первой ячейке больше не попадает в множество документированных тем. Проверка
  вскрыла это на себе в первый же прогон: маркер, процитированный в тексте
  целиком, — тоже маркер. Кейсы t30–t32.

- **Правильность 21 темы оставалась неподтверждённой** (P2-8). Отображение
  корпуса на темы внесено в Instance
  (`.agent/reports/current/instruction-topic-coverage-report.md`); в ядре оно
  жить не может, поскольку описывает корпус конкретного продукта. Раньше оно
  существовало вне репозиториев, то есть утверждение «каждая тема подтверждена
  нормой» не опиралось ни на что проверяемое.

  Ревью границ проведено по четырём спорным парам. Ни одна тема не слита и не
  заведена — пул остаётся из 21 темы; изменена одна сигнатура: третий столбец
  `application-bootstrap` теперь явно запрещает утверждать, где живёт состояние
  и кто им владеет, — граница со `state-management` стала проверяемой при
  присвоении. Решения по остальным трём парам и их основания записаны в самом
  отображении.

  Внесение отображения в репозиторий немедленно опровергло утверждение, ради
  которого оно вносилось: **подтверждены записями девятнадцать тем из 21**, а
  `repository-context` и `task-classification` опознаны только при чтении
  файлов-контейнеров, границы участков в которых не объявлены. §6 реестра тем
  исправлен: их статус — UNVERIFIED до разложения контейнеров на участки, и
  если предмета там не окажется, обе выводятся из пула по §4.

- **Второй проход: четырнадцать дефектов в самих исправлениях.** Правки выше
  прошли независимую проверку до записи в историю, и она нашла в них тот же
  класс дефекта, ради которого весь блок и делался. Существенное:

  - `^поле:\s*\S` считало поле заполненным, если значение пустое: `\s` включает
    перевод строки, и условие удовлетворял первый символ **следующей** строки.
    Пять из шести обязательных полей блока метаданных могли быть пустыми при
    зелёном прогоне; та же ошибка в новой проверке §7 отвергала правильную
    составную ссылку `derived_from` и принимала неправильную. Кейсы t59, t60;
  - реестр без строки `$schema` не проверялся на форму записи ничем: общий
    проход добровольный, а код приёмки полагался на него. Теперь объявление
    схемы обязательно. Кейс t58;
  - идентификатор участка не входил в ключ записи, и решение об одном участке
    контейнера можно было стереть, перенацелив запись на соседний участок;
  - переименование файла реестра в том же коммите, где удалена запись, снимало
    основание для сверки: `--follow` плюс имя файла **на той ревизии**. Кейс t61;
  - контейнер без объявленных границ покрывался записью любого вердикта, хотя
    протокол называет единственный — `deferred`. Кейс t56;
  - запись, называющая несуществующий участок, игнорировалась молча: опечатка в
    `region` роняла полноту по настоящему участку, а сама оставалась незамеченной.
    Кейс t57;
  - маркер участка, процитированный в блоке кода, разбирался как настоящий —
    контейнер, описывающий эту же разметку, не мог пройти гейт. Кейс t55;
  - отсутствие ревизии в мелком клоне (shallow clone) давало красный прогон
    вместо UNVERIFIED — ровно та ошибка, которую уточнение P2-5 велело убрать;
  - строка «дерево подтверждено полным» печаталась рядом с предупреждением о
    строках вне всех участков. Теперь такое дерево полным не считается;
  - §8 стандарта объявлял проверяемым наличие списка сужений у всякой нормы с
    `derived_from`; в коде этой проверки не было. Добавлена;
  - сторож untracked-файлов не распространялся на Instance — и первым же делом
    это подтвердилось: отчёт о покрытии тем, внесённый ради замечания P2-8, сам
    остался вне системы контроля версий. Сторож распространён на Instance.

  **Третий проход** — проверка исправлений второго прохода — нашёл ещё пять
  дефектов, четыре из них внесённые самими исправлениями:

  - гашение ограждённых блоков кода (написанное ради маркера в примере)
    при **незакрытом** ограждении гасило остаток контейнера, и текст вне всех
    участков переставал считаться. Незакрытое ограждение теперь ошибка разбора;
    закрывающее ограждение обязано быть не короче открывающего (кейсы t62, t63);
  - правило разбора маркеров стало разным для пула тем и для контейнеров.
    Сведено к одной реализации: маркер внутри ограждённого блока — пример, вне
    его — объявление, и это записано в обоих стандартах (кейс t66);
  - «текущая запись» контейнера бралась по порядку строк в файле, а не по дате:
    устаревший `deferred`, сдвинутый вниз, выдавал себя за действующее решение
    (кейс t64);
  - обратная сверка участка не доставала до контейнера, который участков не
    объявляет: запись с несуществующим `region` там игнорировалась (кейс t65);
  - внесение `region` в ключ записи сделало дописывание забытого поля
    нарушением append-only. Поведение верное, цена — нет: §6 протокола теперь
    называет её прямо, вместе с ремонтом (новая полная запись, неполная остаётся
    в истории) и с тем, почему неполная не останется незамеченной.

  **Четвёртый проход** нашёл ещё три fail-open случая в идентичности реестра:
  опечатка в `repository` выдавалась за недоступность среды; удаление целого
  файла удаляло и цикл его проверки; переименование вместе с переписанной
  локальной историей скрывало опубликованный файл за старым путём; два текущих
  файла могли объявить один `repository`. Реестр теперь
  опознаётся по стабильному inventory-id, а неизвестный id является
  неразрешимой ссылкой, а его identity обязана быть уникальной (кейсы t67–t70).
  Оба `pre-push` hook записаны исполняемыми (`100755`), иначе Unix Git
  проигнорировал бы именно механизм опубликованного уровня.

  Регрессионный набор: 55 → 73 кейса.

- **Статус `README` реестра приёмки не принадлежал его собственной статусной
  модели.** `draft` в §11 `document-status-model.md` нет: README живёт в
  `maintained | stale | deprecated | archived`. Значение исправлено. Само
  расхождение гейт не видит — он проверяет наличие поля `status`, а не
  принадлежность значения модели типа; чтобы проверять, статусные модели должны
  существовать в машиночитаемом виде, иначе проверка станет вторым источником
  истины о них. Записано как находка для следующего ревью, а не залатано кодом.

- **Условная обязательность полей проверялась только положительными случаями**
  (дополнительно к восьми). Кейсы t33–t35: `retire` без преемника и без причины
  отвергается, `retire` с одной причиной проходит, `deferred` без условия
  возврата отвергается. Без положительного случая проверка, отвергающая всё,
  выглядела бы так же.

## [0.3.0] — 2026-08-19 (`draft`)

Feedback and metrics collection for field testing, requested the same day
0.2.0 was pinned as the field-testing baseline. Purely additive: no schema
or behavior change to anything 0.2.0 already validated.

### Added

- **`standards/workspace/feedback-and-metrics.md`** — methodology for
  collecting field-testing feedback without turning it into ceremony: a
  qualitative friction log (`$MERIDIAN_INSTANCE/.agent/feedback/friction-log.md`,
  one entry per REPORT only when something actually cost time, closed
  vocabulary of friction categories) and a quantitative gate-run log
  (`$MERIDIAN_INSTANCE/.agent/metrics/validate-log.jsonl`).
- **`scripts/validate-and-log.mjs`** — transparent wrapper around
  `kernel-validate.mjs`: identical output and exit code, plus one JSON
  record appended per run against a real Instance. Runs against no Instance
  or the in-repository fixture are deliberately not logged, so the trend is
  never diluted by synthetic data. Covered by
  `test/validate-and-log.test.mjs` (5 cases: exit-code parity, one
  well-formed record per real run, append-only across runs, fixture run not
  logged, no-Instance run not logged) and wired into CI.
- `hooks/pre-push` now also runs the logging wrapper against a configured
  real Instance (non-blocking — this adds a data point, not a new gate) and
  `workflows/task-lifecycle.md`'s REPORT stage points to the friction log.
- `instance-template/` ships `.agent/feedback/` and `.agent/metrics/`
  pre-seeded, so a new product gets both streams from day one.

### Fixed

- `workflows/task-lifecycle.md` still referenced the pre-extraction "Agent
  Smoke Workflow" path; updated to point at `verification/smoke-protocol/PROTOCOL.md`
  as configured by the product smoke unit, consistent with the 0.2.0 extraction.

## [0.2.0] — 2026-08-19 (`draft`)

Правки по результатам внешнего ревью 0.1.0 (verdict: changes_requested), затем
закрытие структурных пробелов, которые ревью зафиксировало как открытые.
Помечена как baseline для полевого тестирования: с этого снимка начинается
сбор обратной связи на боевых задачах (`git tag v0.2.0`).

### Added

- **`test/kernel-validate.test.mjs`** — регрессионный набор валидатора:
  синтетические Kernel/Instance во временных каталогах, adversarial-кейс на
  каждое правило (утечка литерала и паттерна, personal path, побег ссылки,
  symlink-побег, format-нарушение, unsupported keyword в необращённой ветке,
  sha-mismatch, отсутствующий instance-context, duplicate Front Matter,
  прогон без Instance, fixture-исключение). Подключён шагом в CI gate и в
  `hooks/pre-push`; невозможный кейс печатается как SKIP, не пропускается.
- **`verification/smoke-protocol/`** — переносимое ядро smoke-методологии
  (Test Contract, Execution Context, §11 Single Verdict Authority,
  side-effect tiers, test-data правила, шаблон acceptance gate AE-1…AE-5).
  Производный текст: провенанс и SHA исходных файлов продуктового unit 0.3.0
  записаны в README пакета; сам продуктовый unit не разрезан и не изменён.
  Закрывает D-6.
- **`instance-template/`** — bootstrap-каркас нового Instance: `product.yaml`
  и реестры с `REPLACE_ME`-блокерами, скелет `.agent`, `BOOTSTRAP.md` с
  чеклистом первого дня. Шаблонные реестры валидируются в gate теми же
  схемами, что и боевые данные.
- **`scripts/preflight.mjs`** — громкая проверка подключения сессии к
  правильным Kernel/Instance до начала работы; закрывает сценарий «сессия
  молча работает со старыми корнями».

### Fixed

- **Утечка имени компонента текущего продукта** в `registries/commands/README.md`
  найдена ревью и устранена: правило переформулировано без продуктовых имён,
  конкретное исключение перенесено в данные Instance-реестра команд.
- **Product-shaped имена действий удалены из Kernel-схемы** action-профилей
  (`registries/environments/access.schema.json`): generic-действия остаются
  enum'ом, продуктовые объявляются Instance'ом по паттерну `x-*`. Их
  прозаические упоминания в `registries/environments/README.md` и
  `workflows/task-lifecycle.md` также обезличены.
- **Устаревшие абсолютные пути** (`standards/workspace/document-status-model.md`
  ссылался на до-миграционный путь скилла; `workflows/task-lifecycle.md` — на
  абсолютный путь workspace-корня) заменены Kernel-относительной ссылкой и
  нейтральной формулировкой.

### Changed

- **Валидатор: `format` проверяется, а не молча принимается.** Реализованы
  `date`, `date-time`, `uri`; любой другой format валит прогон как
  unsupported.
- **Валидатор: unsupported-keyword-проверка схем идёт по всему дереву схемы
  до валидации**, включая ветки, которые конкретный документ не посещает.
- **Валидатор: link-confinement сравнивает realpath**, а не текстовый префикс
  пути — symlink/junction внутри Kernel больше не маскирует выход за границу.
- **Список Kernel-файлов перестал быть ручным зеркалом**: он перечисляется из
  `git ls-files` и покрывает все tracked-файлы, включая `VERSION`, `LICENSE`,
  `.gitignore`, `.github/`, `hooks/` и `test/`. Бинарные артефакты не
  сканируются текстом и учитываются отдельно как покрытые sha-provenance.
- **`forbidden_patterns` в `product.yaml`**: case-sensitive regex для терминов,
  слишком общих для case-insensitive literal-поиска (например, имя компонента,
  совпадающее с обиходным словом).
- **`requires_instance_context` в `PIN.yaml`**: производный скилл декларирует
  обязательный Instance-контекст, и валидатор проверяет его наличие —
  отсутствие контекста было блокером «на словах», теперь оно механическое.
- `test/instance-fixture` поставляет `skills/bugfix-protocol/context.md`
  с вымышленными конвенциями, чтобы производный протокол был применим и в CI.
- Формулировки `README.md` приведены к собранным доказательствам: привязка к
  Confluence названа явно (D-3); «Kernel не выучил имя продукта» заменено на
  проверяемое утверждение gate с его известными пределами; «проходящий
  валидатор» ограничено объёмом проверок валидатора на момент снимка.
- **Шум предсказуемых warning'ов убран**: недостижимость продуктового
  репозитория из среды запуска — per-repo INFO, единственным WARN остаётся
  агрегат «0/N confirmed». Warning, срабатывающий на каждом зелёном прогоне,
  обучает игнорировать warning'и и разъедает правило «зелёное значит
  проверено».
- `verification/README.md`: маршрут smoke-строки указывает на Kernel-протокол
  (`verification/smoke-protocol/PROTOCOL.md`), конфигурируемый продуктовым
  smoke-unit из Instance.

### Breaking

- `registries/environments/access.schema.json`: enum действий action-профилей
  сужен до generic-набора; продуктовые действия обязаны иметь префикс `x-*`.
  Instance с action-профилями без префикса не пройдёт schema-проверку gate
  без миграции данных (см. `COMPATIBILITY.md`).

### Known state, not claimed otherwise

- Фаза 6 (приёмочный прогон полного жизненного цикла на пустом Instance)
  выполнена и задокументирована в Instance
  (`.agent/reports/current/kernel-portability-phase6-report.md`): доказана
  структурная независимость и применимость на минимальном
  одно-репозиторном продукте. Пригодность методологии для продукта с
  принципиально другой архитектурой — не проверена.
- AE-1…AE-5 продуктового smoke-unit по-прежнему `not-run`; smoke-protocol
  (Kernel) готов и покрыт документацией, но ни один продуктовый smoke-unit
  ещё не прошёл acceptance gate.
- `MANUAL.md` добавлен как единая точка входа для нового оператора; сама
  система впервые выходит за пределы одной агентной сессии, которая её
  строила.

## [0.1.0] — 2026-08-18 (`draft`)

Первый снимок Kernel как самостоятельной релизной единицы под контролем версий.
Версия не является ретроспективной меткой прошлой работы: она обозначает первое
состояние, которое одновременно лежит в Git и проходит валидатор.

### Added

- Физическое разделение Kernel и Instance на два репозитория. До этого граница
  существовала только как документ.
- `skills/bugfix-protocol/` — завендорен внутрь Kernel. Раньше методология
  жила по пути на машине одного оператора и была невоспроизводима где-либо ещё.
  Артефакт производный: удалены имя продукта и раздел продуктовых конвенций,
  оба дайджеста записаны раздельно в `PIN.yaml`.
- `skills/versioning-standard-docs/source/` — исходный архив внутри Kernel, так
  что цепочка «архив → запись → установленный файл» проверяется из чистого
  checkout, а не со слов.
- `test/instance-fixture/` — синтетический Instance для CI и приёмочного прогона.
- `.github/workflows/gate.yml` — валидатор на каждый push и PR, с явной печатью
  того, что CI проверить не может.
- `VERSION`, `CHANGELOG.md`, `COMPATIBILITY.md`, `LICENSE`, `.gitignore`.

### Changed

- Валидатор переведён с трёх жёстко заданных корней на `MERIDIAN_KERNEL` и
  `MERIDIAN_INSTANCE`. Отсутствие Instance теперь явно объявляется как
  «product-литералы не проверялись», а не молча пропускается.
- Проверка провенанса переписана с одного файла на обход всех `skills/*/PIN.yaml`
  с пересчётом дайджестов, включая исходные архивы.
- JSON Schema реестров резолвятся из Kernel по имени каталога реестра.
  Сопоставление по одному имени файла проверило бы `commands/repositories.yaml`
  схемой из `inventory/` — оба реестра объявляют `repositories.schema.json`.
- Добавлено правило: относительная ссылка Kernel-документа, уходящая за пределы
  Kernel, — дефект, даже если на текущей машине она открывается. Восемь таких
  ссылок нашлись сразу и переписаны в форму `$MERIDIAN_INSTANCE/...`.
- Область проверки чистоты расширена на JSON Schema реестров: пример значения
  внутри схемы — такая же утечка, как фраза в тексте.
- Пути перестроены под Meridian: `docs/agent-standards` → `standards/`,
  `engineering-workspace/{commands,environments,inventory,adapters}` →
  `registries/`, `governance/kernel-validate.mjs` → `scripts/`,
  `.agent/README.md` → `standards/workspace/agent-memory.md`.

### Known state, not claimed otherwise

- Приёмочный прогон полного жизненного цикла задачи на `test/instance-fixture`
  ещё не выполнялся. Структурная независимость проверена механически;
  практическая пригодность на чужой архитектуре — нет.
- Записи инвентаря сообщают `UNVERIFIED`, когда продуктовые репозитории
  недостижимы из текущего окружения. Это работа проверки по назначению, а не сбой.
