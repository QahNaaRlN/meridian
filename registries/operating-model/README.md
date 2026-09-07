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
