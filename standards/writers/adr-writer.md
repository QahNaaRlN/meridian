---
title: ADR writer standard
document_type: standard
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-08-18
updated: 2026-08-19
---

# ADR writer standard

Use an ADR to record an architecture decision after it is made. Start from `../templates/adr-template.md`. Keep it concise: context, decision, status, consequences, alternatives considered and links to related RFCs or tickets. Use [the document status model](../workspace/document-status-model.md).

Keep an unpublished ADR in `.agent/publication-drafts/adr/active/`. After acceptance, do not rewrite its context, decision or alternatives as if the decision had always been different; create a replacement ADR and use `superseded` where appropriate. A published snapshot moves to `archive/` only after a verified `canonical_url` is recorded.

When publishing, apply the [template contract](../templates/template-contract.md) and take the ADR body from the profile of the platform in the canonical-wiki role — `../templates/profiles/<platform>/adr-body.md`, where `<platform>` is the one declared in `$MERIDIAN_INSTANCE/product.yaml`. Do not copy the local H1, and render visible metadata through the shared bilingual table.
