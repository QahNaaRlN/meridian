---
title: Environments
document_type: reference
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-08-18
updated: 2026-08-19
---

# Environments

This area stores source-backed environment topology, access references, action permissions, and test-data references. It must never contain credentials, tokens, private keys, personal identifiers, or copied secret values.

## Evidence states

- `declared` endpoint means that a repository source declares the URL. It does not prove reachability.
- `unknown` means that no authoritative value was found in the approved sources.
- `verification_state` is observational and may change only from current execution evidence.
- `required_config` records configuration relevance without resolving or copying values.
- unresolved but irrelevant configuration does not block a run; the selected workflow computes the required union through `requires_config`.

Repository templates prove variable names and declared topology only. Reachability, resolved credentials, capabilities, commit provenance, and observed provider metadata are Execution Context facts and require current evidence.

## Provenance resolver

Each environment declares one provenance resolver requirement. A Test Contract selects the requirement before observation; the factual `provenance_result` belongs in Execution Context and cannot retroactively weaken the contract.

- Local resolution records all participating Git revisions and dirty states plus the identities and origins of the running processes or containers.
- Deployed resolution must use an authoritative deployment metadata source. DEV and QA remain blocked until that source is named.
- Provider metadata is P2 corroborating evidence. It may supplement, but never replace, the selected provenance resolver.

## Access

`access.yaml` preserves the Agent Smoke Workflow terms `credentials_by_role` and `default_role`. Values are external references only. An empty map or `null` default is an explicit unresolved state, not permission to guess or reuse a credential.

Action profiles default to unresolved. No environment action is authorized by merely listing it. In particular, test-data creation, database changes, queue changes, and every instance-defined `x-*` action require explicit approval. Instance-defined actions are product vocabulary: their meaning and approval rules live in the Instance, never in this registry's schema.

## Capabilities and verdicts

Capabilities are not declared here as permanent facts. Their lifecycle remains `unknown -> probe-on-use -> true/false -> TTL expiry -> unknown`, with evidence captured in Execution Context. This registry does not assign smoke verdicts; Agent Smoke Workflow §11 remains the Single Verdict Authority.

## Test data

`test-data.yaml` contains only external references and always forbids sensitive data. Until the baseline task and target environment are chosen, pilot test data remains an explicit blocker.
