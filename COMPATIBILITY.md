---
title: Meridian compatibility contract
document_type: contract
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-08-18
updated: 2026-09-07
---

# Meridian compatibility contract

Совместимость **объявляется**, а не выводится из близости номеров версий.

## Релизные единицы

Meridian состоит из независимо версионируемых единиц. Общей линии версий у них нет.

| Единица | Где | Линия версий | Примечание |
|---|---|---|---|
| Kernel | этот репозиторий | SemVer, `VERSION` | методология, валидатор, схемы, writers |
| Instance | отдельный репозиторий на продукт, вне Kernel | Git revision; SemVer только если распространяется | размещается там, где данные продукта; никогда не поставляется вместе с Kernel |
| Завендоренные skills | `skills/<name>/` | неизменяемый SHA-256 в `PIN.yaml` | пин, а не номер версии |
| Smoke-пакет | Instance | собственный SemVer | product-specific по природе |

## Kernel ↔ Instance

| Kernel | Ожидаемый Instance | Ломающие изменения |
|---|---|---|
| `0.1.x` | `schema_version: 1` в `product.yaml` | не объявлены; `0.x` не даёт гарантий стабильности |
| `0.2.x` | `schema_version: 1` в `product.yaml`; action-профили `environments/access.yaml` используют generic-enum или `x-*`-префикс для продуктовых действий | относительно `0.1.x`: enum действий сужен, продуктовые значения без префикса `x-*` не проходят schema-gate — Instance требует миграции (см. `CHANGELOG.md` 0.2.0 → Breaking) |
| `0.4.x` | то же, что `0.2.x`; плюс: рабочие заметки Instance, ссылающиеся на слой шаблонов, указывают на `standards/templates/template-contract.md` и `standards/templates/profiles/<платформа>/<тип>-body.md`; плюс: каждая запись `inventory/repositories.yaml` несёт `profile`, значение которого названо из закрытого пула Kernel stack profiles и подтверждается манифестом соответствующего репозитория | относительно `0.3.x`: слой шаблонов разделён на контракт и профиль, после чего всё дерево приведено к единому правилу имён (`standards/workspace/document-identity.md`). Переименовано шестнадцать файлов, в том числе `CONFLUENCE.md` → `profiles/confluence/confluence-profile.md`, восемь `*.confluence-template.md` → `profiles/confluence/*-body.md`, `PROTOCOL.md` → `smoke-protocol.md`, `ACCEPTANCE-GATE.template.md` → `acceptance-gate-template.md`, `BOOTSTRAP.md` → `instance-bootstrap.md`. Ломает текстовые ссылки Instance, и гейт этого не видит — см. следующий раздел. Плюс относительно `0.3.x`: `registries/inventory/repositories.schema.json` сделал `repositories[].profile` обязательным; отсутствие `profile`, имя вне закрытого пула Kernel stack profiles или объявление, противоречащее манифесту репозитория, даёт FAIL валидатора и требует миграции Instance |

Пока Kernel в `0.x`, любой minor может сломать Instance. Instance пиннит точную
версию Kernel до выхода `1.0.0`.

**Ожидаемое влияние `0.5.x` (пакет `git-governance-migration`).** Выпуск
`0.5.0` вводит общую для репозиториев Meridian норму: закрытый шаблон имён
временных ветвей и форму сообщений коммитов
(`standards/workspace/version-control-flow.md` §3.1, §12). Постоянные линии
`main` / `dev` и их платформенная защита — репозиторий-локальны для Kernel и на
Instance не распространяются; физические ветви Instance выпуском `0.5.0` не
переименовываются. Существующий Instance не реконфигурируется автоматически:
для принятия общей нормы конкретному Instance нужен отдельный
репозиторий-локальный пакет. Ожидаемый Instance для `0.5.x`: то же, что для
`0.4.x`, плюс — при переходе на `0.5.x` — имена его временных ветвей и сообщения
его коммитов приведены к общей форме. **Окончательная строка `0.5.x` этой
таблицы и раздел `CHANGELOG.md` `0.5.0 → Breaking` формируются в ветке
`release/0.5.0`**, а не в этом пакете: здесь зафиксировано только ожидание, а не
итоговое объявление совместимости.

## Ссылки Instance на пути внутри Kernel

Гейт Kernel проверяет две вещи и не проверяет третью:

- относительные Markdown-ссылки **внутри** Kernel — проверяются: несуществующая
  цель красит прогон;
- относительная ссылка из Kernel, уходящая **за** его границу, — дефект по
  правилу, даже если на текущей машине она открывается;
- **текстовая ссылка из Instance на путь внутри Kernel** (`$MERIDIAN_KERNEL/…`)
  — не проверяется ничем. Это не Markdown-ссылка, и разрешать её валидатору не
  с чем: Instance для него — данные, а не исходник, и целевого корня у такой
  ссылки в общем случае нет.

Следствие, а не недосмотр: переименование файла в Kernel может молча сломать
рабочую заметку Instance, а прогон останется зелёным. Поэтому перемещения в
нормативных каталогах Kernel объявляются здесь строкой в таблице выше, а не
выводятся из того, что гейт не покраснел. Владелец Instance правит свои ссылки
по объявленной строке.

Исторические записи Instance (отчёты, архив анализа) при таком перемещении **не
переписываются**: они фиксируют путь, который существовал на момент записи. По
той же причине, по которой не переписывается `CHANGELOG.md`.

## Завендоренные skills

| Skill | Состояние | Пин |
|---|---|---|
| `versioning-standard-docs` | `vendored` | SHA артефакта + SHA исходного архива и записи в нём |
| `bugfix-protocol` | `vendored-derived` | SHA производного артефакта + отдельно SHA исходного, со списком трансформаций |

`vendored-derived` означает, что в Kernel лежит не дословная копия: из артефакта
удалены продуктовые данные. Оба дайджеста записаны раздельно, и ни один не
выдаётся за другой. Смена пина — изменение Kernel и требует записи в CHANGELOG.

## Чего этот контракт намеренно не покрывает

- Продуктовые релизы и их CI — вне области Kernel.
- Область действия завендоренного стандарта версионирования документов
  (`skills/versioning-standard-docs/`) ограничена документами в каноническом
  wiki. Версионирование Git-артефактов Kernel — отдельный предмет; он описан
  двумя стандартами:
  [`standards/workspace/version-control-flow.md`](standards/workspace/version-control-flow.md)
  (поток веток: постоянные линии `main` / `dev` — до завершения миграции
  физически `master` / `develop`; обычные ветки `feature` / `bugfix` / `chore` /
  `docs` / `refactor` / `test` / `ci` / `build` и специальные `release` /
  `hotfix` / `promotion`) и
  [`standards/workspace/release-versioning.md`](standards/workspace/release-versioning.md)
  (SemVer-линия Kernel, `VERSION`, Keep a Changelog, тег `vX.Y.Z`). Стандарты
  задают режим продвижения **репозитория**: `semver-release` (Kernel —
  repository-level `VERSION`, тег `vX.Y.Z`) и `revision-promotion` (репозиторий
  без решения о SemVer, включая Instance — верхнеуровневое состояние
  идентифицируется Git SHA advancement commit, repository-level `VERSION`/тег не
  создаются). Вложенные независимо версионируемые единицы (`stack-profiles/`,
  smoke-пакет) режим репозитория не переопределяет — они ведут свои
  `VERSION`/`CHANGELOG`/теги сами. Правила выбора MAJOR/MINOR/PATCH обоих
  стандартов ссылаются на ломающие изменения в терминах этого контракта, не
  переопределяя его.
