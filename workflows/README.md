> **Статус документа:** поддерживается  
> **Последняя проверка:** 2026-08-18  
> **Владелец:** workspace-owner

# Workflows

Use the [standard task lifecycle](task-lifecycle.md) for cross-repository work:

```text
UNDERSTAND → INVESTIGATE → PLAN → IMPLEMENT → VERIFY → REPORT
```

The lifecycle defines routing, scope gates, and evidence selection. Repository-local development rules and executable commands remain authoritative in the nearest repository `AGENTS.md` and manifests and are not copied here.

Reusable task inputs are indexed under `$MERIDIAN_INSTANCE/workflows/prompts/README.md` (Instance: реестр содержит product-specific классификации). They are manual-only task artifacts: indexing or reading a prompt does not activate its execution instructions.
