---
title: Document quality standard
document_type: standard
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-08-18
updated: 2026-08-19
---

# Document quality standard

File names and `document_type` are governed by [document identity](document-identity.md); this standard does not restate them. Write concise, factual documents with explicit scope, owner, status, and links to authoritative sources where useful.

Нормативные объяснения и определения — на русском языке; канонический английский термин — в скобках при первом определении (см. Documentation Standard §9). Машинные идентификаторы (`area_type`, `document_type`, пути репозиториев) не переводить. Не смешивать оси: семантическое владение ≠ тип документа ≠ привязка реализации ≠ отношения.

README files use `../templates/readme-template.md` when that template is applicable. Do not invent commands, environment variables, architecture, or operational guarantees. Keep vendor adapters short and link to the canonical standard instead of copying it.

Before completion, validate the `document_type`, status, required Front Matter fields and physical phase against [the status model](document-status-model.md). Never use an archive folder as a substitute for a meaningful terminal status.

For publication to the canonical wiki, follow the [template contract](../templates/template-contract.md) and the [profile](../templates/profiles/README.md) of the platform in that role: Page Title is the only H1, the body begins with the shared bilingual metadata table, and visible status values include the stable English identifier in parentheses. Local Markdown files keep machine-readable English Front Matter and may retain their own H1.
