---
title: Inventory
document_type: readme
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-08-18
updated: 2026-08-19
---

# Inventory

This area defines repository identities and source-backed relationships. Phase 2 contains the four approved pilot repositories and the relationships demonstrated by repository-owned sources.

Population rules:

- use exact filesystem paths;
- cite repository-owned `AGENTS.md`, manifests, and other evidence;
- record verification time;
- pin the observed Git revision, ref, dirty state, and whether evidence includes working-tree content;
- do not infer architecture from directory names;
- do not duplicate repository-local instructions;
- treat `last_verified` as having a freshness window, not a permanent fact: an entry older than 3 days is stale and must be revalidated (revision, ref, dirty state) before being cited as current evidence, not merely re-dated. `engineering-workspace/governance/kernel-validate.mjs` flags stale entries as a WARN; a WARN does not by itself confirm the entry is wrong, only that it is unconfirmed.
