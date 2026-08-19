---
title: Plan writer standard
document_type: standard
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-08-18
updated: 2026-08-19
---

# Plan writer standard

Use `<subject>-plan.md` (or `<ticket>-<subject>-plan.md`) for task plans. A plan states scope, current facts, intended changes, affected files, validation, risks, rollback, and open decisions. It must distinguish facts from assumptions and must not invent commands, architectural facts, or approvals. Use only statuses and transitions allowed by [the document status model](../workspace/document-status-model.md); a completed plan requires recorded successful verification.

For cross-repository changes, list each owning repository and keep files in that repository unless the artifact itself spans repositories. Plans are working artifacts, not global instructions.
