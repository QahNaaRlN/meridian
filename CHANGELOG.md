---
title: Changelog
document_type: changelog
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-08-18
updated: 2026-08-19
---

# Changelog

Формат — [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
версионирование — [SemVer](https://semver.org/spec/v2.0.0.html).
Линия версий Kernel независима от Instance и от продуктовых репозиториев.

## [Unreleased]

### Added

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
