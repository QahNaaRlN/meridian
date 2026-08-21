---
title: Skills
document_type: readme
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-08-18
updated: 2026-08-20
---

# Skills

Методологические зависимости, завендоренные внутрь Kernel. Каждая лежит здесь
целиком вместе с `PIN.yaml`, и её неизменность проверяется пересчётом SHA-256 на
каждом прогоне гейта: пин, который только записан, ничего не доказывает.

| Skill | Состояние | Что пиннится |
|---|---|---|
| [`versioning-standard-docs`](versioning-standard-docs/SKILL.md) | `vendored` | SHA артефакта, SHA исходного архива и записи в нём — цепочка проверяется из чистого клона |
| [`bugfix-protocol`](bugfix-protocol/SKILL.md) | `vendored-derived` | SHA производного артефакта и отдельно SHA исходного, со списком трансформаций |

`vendored-derived` означает, что здесь лежит не дословная копия: из артефакта
удалены имя продукта и раздел продуктовых конвенций. Оба дайджеста записаны
раздельно, и ни один не выдаётся за другой. Конвенции переехали в
`$MERIDIAN_INSTANCE/skills/bugfix-protocol/context.md`; их отсутствие — блокер,
объявленный в `PIN.yaml` полем `requires_instance_context` и проверяемый гейтом,
а не разрешение догадываться.

Файлы `SKILL.md` несут Front Matter внешнего формата (`name`, `description`) и
не получают полей Kernel: дописать туда своё поле значит изменить артефакт,
неизменность которого пин и доказывает. Их `document_type` объявлен поимённо в
[`standards/workspace/document-identity.md`](../standards/workspace/document-identity.md)
§4: обе завендоренные зависимости — `protocol` по сигнатуре содержания.

Каталог `skills/` называет **способ поставки**, а не жанр. Тип `skill` из пула
жанров выведен: он отличался от `protocol` только упаковкой. Формат поставки
объявляется полем `delivery` —
[`agent-instruction-identity.md`](../standards/workspace/agent-instruction-identity.md) §5.

Смена пина — изменение Kernel и требует записи в `CHANGELOG.md`; правила
совместимости — `COMPATIBILITY.md`.
