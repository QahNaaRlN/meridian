---
title: Inventory
document_type: readme
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-08-18
updated: 2026-08-27
---

# Inventory

This area defines repository identities and source-backed relationships. Phase 2 contains the four approved pilot repositories and the relationships demonstrated by repository-owned sources.

## Canonical id vs. display alias vs. reference

Three separate things, easy to conflate because a given environment often spells them the same way:

- **Canonical identity** — `repositories[].id` in `repositories.yaml` (validated by `repositories.schema.json`). The only identity of a product repository. A `path`, a remote URL, a directory name, a `semantic_areas` entry, or a display name in any adapter is not identity.
- **Display alias** — a human-readable label an adapter shows for a root, e.g. `folders[].name` in a Cursor workspace. A label, never an identity; it creates no repository scope and authorizes no cross-repository change.
- **Reference record** — an explicit typed row that resolves a display alias to a canonical `repository_id` with provenance. Instance data, kept in `repository-references.yaml` against `repository-references.schema.json`; the resolution is exact-match and fail-closed.

The neutral algorithm and the boundary between what the Kernel holds (schema + algorithm) and what the Instance holds (the aliases themselves) are in `../../standards/workspace/repository-references.md`. The record shape is `repository-references.schema.json` in this directory.

Population rules:

- use exact filesystem paths;
- cite repository-owned `AGENTS.md`, manifests, and other evidence;
- record verification time;
- pin the observed Git revision, ref, dirty state, and whether evidence includes working-tree content;
- do not infer architecture from directory names;
- do not duplicate repository-local instructions;
- treat `last_verified` as having a freshness window, not a permanent fact: an entry older than 3 days is stale and must be revalidated (revision, ref, dirty state) before being cited as current evidence, not merely re-dated. `engineering-workspace/governance/kernel-validate.mjs` flags stale entries as a WARN; a WARN does not by itself confirm the entry is wrong, only that it is unconfirmed.
