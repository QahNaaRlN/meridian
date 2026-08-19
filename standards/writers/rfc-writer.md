# RFC writer standard

Use an RFC for an architecture proposal that needs engineering review. Start from `../templates/RFC.template.md`. State the problem, constraints, decision drivers, goals and non-goals, proposed design, alternatives, tradeoffs, migration, rollout, metrics, risks and open questions. Use facts; do not invent requirements or alternatives. Select transitions and required metadata through [the document status model](../workspace/document-status-model.md).

Keep an unpublished RFC in `.agent/publication-drafts/rfc/active/`. After a real publication to the canonical wiki, add `published`, `published_at`, and `canonical_url`, add the required snapshot notice, and move it to `archive/`. An RFC is not a task plan.

When publishing, apply the [template contract](../templates/CONTRACT.md) and take the RFC body from the profile of the platform in the canonical-wiki role — `../templates/profiles/<platform>/RFC.body.md`, where `<platform>` is the one declared in `$MERIDIAN_INSTANCE/product.yaml`. Do not copy the local H1, and render visible metadata through the shared bilingual table.
