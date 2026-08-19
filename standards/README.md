# AI documentation standards

Start here when creating or reorganizing documentation and agent working artifacts.

**Размещение / таксономия** (Common, Domain, System, Platform; `unclassified` как состояние, не обязательная папка; привязка реализации к repository) задаётся только в каноническом wiki текущего продукта — точная ссылка на действующий Documentation Standard указана в `$MERIDIAN_INSTANCE/product.yaml` (`canonical_wiki.documentation_standard_url`). Локальные файлы ниже site map не дублируют; исторический снимок предыдущей версии стандарта не является действующим, если `product.yaml` не указывает иное.

Нормативные определения — на русском; канонический английский термин — в скобках при первом вхождении; машинные идентификаторы (`area_type`, `document_type`, пути репозиториев) не переводить. Подробности — Documentation Standard §9.

## Read first

1. [Kernel / Instance boundary](workspace/kernel-boundary.md) — what belongs to the portable methodology vs. the current product's data.
2. [Tooling axes](workspace/tooling-axes.md) — documentation, task tracker, and repository hosting are Instance-configured; never assume vendor, URL, or access protocol.
3. [Workspace and instruction precedence](workspace/agent-workspace.md) — choose the authoritative instruction source and task scope; repository `AGENTS.md` lists semantic areas.
4. [Document lifecycle](workspace/document-lifecycle.md) — choose a permanent-document or `.agent` location.
5. [Document status model](workspace/document-status-model.md) — choose and change a machine-readable status.
6. [Document quality](workspace/document-quality.md) — apply shared naming and evidence rules.

## Use when writing

- [Plan writer](writers/plan-writer.md) — task plans.
- [RFC writer](writers/rfc-writer.md) — architecture proposals and decisions.
- [ADR writer](writers/adr-writer.md) — concise, recorded architecture decisions.
- [README writer](writers/readme-writer.md) — repository and component entry points.
- Problem Record — status model §7 and Documentation Standard §8; draft body from `$MERIDIAN_INSTANCE/.agent/publication-drafts/template-problem-record.md` (the canonical wiki's own page template is canonical for new pages).

## Operational workflows

- `$MERIDIAN_INSTANCE/verification/smoke-testing/README.md` — operational browser smoke-тестирование, Test Contract, evidence и единая verdict matrix; release unit размещён в Engineering Workspace.

## Templates

Use a template when creating the corresponding document, but adapt only factual content: [README](templates/README.template.md), [RFC](templates/RFC.template.md), [ADR](templates/ADR.template.md).

For publication to the canonical wiki, first apply the [unified template contract](templates/CONFLUENCE.md), then use the type-specific body from the platform profile: [Tutorial](templates/Tutorial.confluence-template.md), [How-to](templates/How-to.confluence-template.md), [Reference](templates/Reference.confluence-template.md), [Explanation](templates/Explanation.confluence-template.md), [RFC](templates/RFC.confluence-template.md), [ADR](templates/ADR.confluence-template.md), [Incident](templates/Incident.confluence-template.md), or [Problem Record](templates/Problem-Record.confluence-template.md). The file `.agent/publication-drafts/template-problem-record.md` remains an operational note and points to the same canonical template.

Cursor automatically routes RFC and README paths to their writer standards. All other standards are selected when their document type or task requires them. Publication drafts are working artifacts in `.agent/publication-drafts/` (`problems|rfc|adr|concepts`), not a second canonical product-documentation base. Ссылки на опубликованные канонические версии: `$MERIDIAN_INSTANCE/.agent/publication-drafts/PUBLISHED-INDEX.md`.
