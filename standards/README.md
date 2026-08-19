---
title: AI documentation standards
document_type: readme
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-08-18
updated: 2026-08-19
---

# AI documentation standards

Start here when creating or reorganizing documentation and agent working artifacts.

**Размещение / таксономия** (Common, Domain, System, Platform; `unclassified` как состояние, не обязательная папка; привязка реализации к repository) задаётся только в каноническом wiki текущего продукта — точная ссылка на действующий Documentation Standard указана в `$MERIDIAN_INSTANCE/product.yaml` (`canonical_wiki.documentation_standard_url`). Локальные файлы ниже site map не дублируют; исторический снимок предыдущей версии стандарта не является действующим, если `product.yaml` не указывает иное.

Нормативные определения — на русском; канонический английский термин — в скобках при первом вхождении; машинные идентификаторы (`area_type`, `document_type`, пути репозиториев) не переводить. Подробности — Documentation Standard §9.

## Read first

1. [Kernel / Instance boundary](workspace/kernel-boundary.md) — what belongs to the portable methodology vs. the current product's data.
2. [Tooling axes](workspace/tooling-axes.md) — documentation, task tracker, and repository hosting are Instance-configured; never assume vendor, URL, or access protocol.
3. [Document identity](workspace/document-identity.md) — how a file is named and how its `document_type` is assigned: by full signature match, never as a residual.
4. [Workspace and instruction precedence](workspace/agent-workspace.md) — choose the authoritative instruction source and task scope; repository `AGENTS.md` lists semantic areas.
5. [Document lifecycle](workspace/document-lifecycle.md) — choose a permanent-document or `.agent` location.
6. [Document status model](workspace/document-status-model.md) — choose and change a machine-readable status.
7. [Document quality](workspace/document-quality.md) — apply shared evidence and writing rules.

## Use when writing

- [Plan writer](writers/plan-writer.md) — task plans.
- [RFC writer](writers/rfc-writer.md) — architecture proposals and decisions.
- [ADR writer](writers/adr-writer.md) — concise, recorded architecture decisions.
- [README writer](writers/readme-writer.md) — repository and component entry points.
- Problem Record — status model §7 and Documentation Standard §8; draft body from `$MERIDIAN_INSTANCE/.agent/publication-drafts/template-problem-record.md` (the canonical wiki's own page template is canonical for new pages).

## Operational workflows

- `$MERIDIAN_INSTANCE/verification/smoke-testing/README.md` — operational browser smoke-тестирование, Test Contract, evidence и единая verdict matrix; release unit размещён в Engineering Workspace.

## Templates

Use a template when creating the corresponding document, but adapt only factual content: [README](templates/readme-template.md), [RFC](templates/rfc-template.md), [ADR](templates/adr-template.md).

For publication to the canonical wiki, first apply the [template contract](templates/template-contract.md) — it names no platform — then take the type-specific body from the [profile](templates/profiles/README.md) of the platform currently holding that role. Which platform that is comes from `$MERIDIAN_INSTANCE/product.yaml`, not from the fact that one profile happens to be the only one present. The file `$MERIDIAN_INSTANCE/.agent/publication-drafts/template-problem-record.md` remains an operational note and points to the same canonical body.

Cursor automatically routes RFC and README paths to their writer standards. All other standards are selected when their document type or task requires them. Publication drafts are working artifacts in `.agent/publication-drafts/` (`problems|rfc|adr|concepts`), not a second canonical product-documentation base. Ссылки на опубликованные канонические версии: `$MERIDIAN_INSTANCE/.agent/publication-drafts/PUBLISHED-INDEX.md`.
