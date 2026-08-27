---
title: Идентичность репозитория, display alias и reference-запись
document_type: standard
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-08-27
updated: 2026-08-27
topic: unclassified
unclassified_reason: >-
  предмет — разведение канонической идентичности репозитория, adapter display
  alias и типизированной reference-записи, разрешающей одно в другое, — не
  сводится целиком ни к одной теме действующего пула: `repository-context`
  отвечает за контекст в `AGENTS.md`, а не за разрешение имён; `version-control-flow`
  — за поток изменений. Тема присваивается при ревью границ пула, не остаточно
  (`instruction-topics.md` §1)
profile: universal
delivery: kernel-doc
activation: always
related_documents:
  - ./kernel-boundary.md
  - ../../registries/inventory/README.md
  - ../../registries/inventory/repository-references.schema.json
---

# Идентичность репозитория, display alias и reference-запись

Стандарт отвечает на один вопрос: **как имя корня, показанное пользователю в
IDE или другом adapter, превращается в каноническую идентичность репозитория —
и почему это превращение не выполняется по догадке.** Он строго разделяет три
разные вещи, которые легко спутать, потому что в конкретной среде они часто
совпадают текстуально: каноническую идентичность, display alias и
reference-запись.

Kernel хранит схему и алгоритм; конкретные aliases и их провенанс — Instance
(`kernel-boundary.md`, правило классификации новых файлов). Cursor workspace
и любой другой adapter остаются доставкой, а не источником истины
(`meridian-owner-intent-contract.md` §5–6).

## 1. Каноническая идентичность

**Единственная каноническая идентичность продуктового репозитория —
`repositories[].id`** из `inventory/repositories.yaml` (реестр инвентаризации
Instance).

- `id` остаётся строчным `kebab-case` по действующей
  `registries/inventory/repositories.schema.json` (`^[a-z0-9][a-z0-9-]*$`);
  этот стандарт форму `id` не меняет и второго поля идентичности не вводит.
- **Не** являются канонической идентичностью и не заменяют `id`: `path`,
  remote URL, имя каталога на диске, `semantic_areas`, `role`, а также
  display name репозитория в любом adapter.
- Совпадение или сходство имени каталога с чем бы то ни было — не
  доказательство соответствия. Идентичность подтверждается только явной
  записью реестра, а не тем, что две строки похожи.

## 2. Display alias

**Display alias** — человекочитаемая подпись корня в UI или adapter: например
`folders[].name` в файле Cursor workspace.

- Alias создаётся для удобства чтения, а не для идентификации. Он **не**
  заменяет `id`, **не** создаёт repository scope и **не** разрешает
  изменение, затрагивающее другой репозиторий.
- Один и тот же alias может встречаться в нескольких adapter-файлах и может
  отличаться от `id` и от имени каталога. Это ожидаемо и само по себе
  дефектом не является.
- Одинаковое или похожее имя каталога — в том числе вложенный каталог, чьё
  имя буквально совпадает с именем каталога верхнего уровня другого
  репозитория, — **не** evidence соответствия и не основание разрешить alias
  без записи.

## 3. Reference-запись

Instance хранит product-specific reference records в собственном реестре
(`inventory/repository-references.yaml`), подчинённом
`registries/inventory/repository-references.schema.json`. Kernel хранит только
схему.

Каждая запись содержит ровно четыре поля:

| Поле | Значение |
|---|---|
| `kind` | вид reference. Для MVP пул закрыт единственным значением `cursor-workspace-folder` — folder display name в файле Cursor workspace. |
| `alias` | непустая строка: display name ровно в том виде, в каком его пишет adapter. |
| `repository_id` | канонический `id`, в который alias разрешается. Обязан существовать как `repositories[].id` в `inventory/repositories.yaml`. |
| `sources` | непустой список мест, где этот display alias фактически записан. |

Каждый элемент `sources` — это `artifact` + `pointer`:

- `artifact` — **Instance-relative** путь к adapter-файлу, несущему alias
  (например `adapters/cursor/<workspace>.code-workspace`). Абсолютный
  Unix-путь, путь с буквой диска, любой обратный слэш и **любой сегмент пути
  `..`** запрещены настолько, насколько это выражает один pattern схемы.
  `artifact` не может содержать сегмент `..` ни в какой позиции; разрешение
  `source`-ссылки не вправе выходить за корень Instance. Путь, который
  выводил бы за пределы корня Instance, считается недействительным —
  результат `UNVERIFIED` / STOP, а не разрешение доверять сохранённому
  display имени.
- `pointer` — точная ссылка на место alias внутри `artifact`, однозначно
  называющая folders-запись по display name, например
  `folders[name=<alias>].name`.

**Текст workspace в реестр не копируется.** Reference-запись хранит ссылку и
провенанс, а не вторую копию adapter-файла — тот же инвариант, что для
`derived_from`/`digest` в `agent-instruction-identity.md` §6 и
`instruction-intake.md` §7: Instance хранит квалифицированную ссылку, не копию.

## 4. Алгоритм разрешения

Вход — одна строка-кандидат (`id` или alias) и, для alias, реестр
reference-записей. Алгоритм детерминированный и fail-closed:

1. **Точное совпадение с каноническим `id`.** Если строка буквально равна
   какому-то `repositories[].id` — это и есть результат; reference-записи не
   требуются.
2. **Иначе — точное, case-sensitive совпадение с объявленным `alias`.**
   Сверяется буквально: без приведения регистра, без транслитерации, без
   подбора по похожести, без угадывания по `basename`, `path`, remote URL.
3. `alias` обязан разрешиться **ровно в один** `repository_id`.
4. **Ноль совпадений** → `unresolved_repository_reference` / STOP. Отсутствие
   записи — это остановка, а не разрешение доверять сохранённому display
   имени.
5. **Несколько записей с разными `repository_id`** для одного `alias` →
   `conflict` / STOP.
6. Успешный результат **всегда** возвращает канонический `repository_id` и
   ссылку на разрешившую reference-запись как провенанс. `alias` выходной
   идентичностью не становится никогда.
7. `repository_id` разрешившей записи обязан существовать в
   `inventory/repositories.yaml`; если его там нет — STOP, запись
   недействительна.
8. Если `source`-ссылка отсутствует или устарела (adapter-файла нет по
   указанному `artifact`, либо alias по `pointer` больше не найден) —
   `UNVERIFIED` / STOP, а не разрешение доверять сохранённому display имени.
9. `artifact` обязан быть Instance-relative и не содержать сегмента `..` ни
   в какой позиции. Абсолютный путь, путь с буквой диска, путь с обратным
   слэшем или с сегментом `..` недействителен: разрешение `source`-ссылки не
   вправе выходить за корень Instance — результат `UNVERIFIED` / STOP.

## 5. Границы

- **Kernel хранит схему и алгоритм.** Универсальный список конкретных
  product-имён в Kernel не заводится.
- **Instance хранит конкретные aliases и references** в
  `inventory/repository-references.yaml`.
- **Adapter остаётся доставкой.** Наличие display alias в Cursor workspace
  или ином adapter не делает adapter источником идентичности.
- **Абсолютный machine-local путь в Kernel не переносится**
  (`kernel-boundary.md`). Reference-записи оперируют Instance-relative
  путями.
- Механизм **не** добавляет репозиторий в `inventory/repositories.yaml`, не
  заменяет intake (`instruction-intake.md`), resolver PHASE C или
  repository provenance. Он лишь разрешает уже объявленное имя в уже
  объявленную идентичность.

## 6. Что этот стандарт не вводит

- **универсальный список product-имён в Kernel** — конкретные значения живут
  в Instance;
- **alias как второе поле идентичности** — идентичность одна, `id`;
- **новые виды `reference` «на будущее»** — пул `kind` закрыт единственным
  `cursor-workspace-folder` до появления реальной второй потребности;
- **remote-, path-, basename-разрешение** и **fuzzy matching** — только
  точное совпадение;
- **alias для корней, которых нет в каноническом product inventory** —
  управляющие корни (Instance, Kernel, произвольный control-plane root)
  разрешаются через `MERIDIAN_KERNEL`/`MERIDIAN_INSTANCE`, а не через этот
  реестр; корень вне canonical inventory остаётся неразрешённым до
  отдельного решения владельца, а не изобретается здесь.
