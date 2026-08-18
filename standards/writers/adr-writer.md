# ADR writer standard

Use an ADR to record an architecture decision after it is made. Start from `../templates/ADR.template.md`. Keep it concise: context, decision, status, consequences, alternatives considered and links to related RFCs or tickets. Use [the document status model](../workspace/document-status-model.md).

Keep an unpublished ADR in `.agent/publication-drafts/adr/active/`. After acceptance, do not rewrite its context, decision or alternatives as if the decision had always been different; create a replacement ADR and use `superseded` where appropriate. A Confluence snapshot moves to `archive/` only after a verified `canonical_url` is recorded.

When publishing to Confluence, use `../templates/ADR.confluence-template.md`: do not copy the local H1, and render visible metadata through the unified bilingual table.
