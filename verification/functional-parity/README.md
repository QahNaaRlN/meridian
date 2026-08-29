---
title: Functional parity
document_type: readme
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-08-28
updated: 2026-08-29
---

# Functional parity

The portable verification unit for the evidence a `REFACTOR` change rests on:
how the observable behaviour of the system before an internal change is fixed,
and how it is compared with the behaviour after it. A `REFACTOR` is defined by
functional parity — observable behaviour unchanged, internal structure changed —
so its acceptance needs a contract for what that proof must contain and when it
is sufficient, and a protocol for the order that proof is produced in.

## Contents

- [`functional-parity-evidence-contract.md`](functional-parity-evidence-contract.md)
  — the normative contract: the preserved observable contract (four facets), the
  baseline before the change, the post-change evidence, the neutral evidence
  kinds and the limitations of each, the public-contract snapshot as one kind
  among them, gaps and `UNVERIFIED`, the product/repository boundary, and the
  verdict rules.
- [`refactor-protocol.md`](refactor-protocol.md) — the execution order for a
  `REFACTOR` work item (`CLASSIFY → DEFINE PARITY → CAPTURE BASELINE → IMPLEMENT
  → CAPTURE POST-CHANGE → COMPARE → REPORT`), referencing the contract above at
  the capture and compare steps. It designs the order only; it defines no second
  evidence contract.
- [`functional-parity-evidence.schema.json`](./functional-parity-evidence.schema.json)
  — JSON Schema for one functional-parity evidence record.
- [`fixtures/functional-parity-evidence.fixtures.json`](./fixtures/functional-parity-evidence.fixtures.json)
  — product-neutral valid and invalid fixtures, reached in-gate.

## Division of labour

The contract, the record form, the evidence kinds, the semantic requirements
and the inference rules are Kernel. Everything a real comparison binds to —
test runners, snapshot or characterisation-test frameworks, exact commands,
configuration, the paths where a repository keeps its tests and recorded
observations, product tooling — is Instance/repository data and never appears
here or in the fixtures. A product's own application of this contract (a product
recording its own functional-parity evidence for a consolidation initiative) is
Instance work, PHASE F of the rule-resolution program, not part of this unit.

## Mechanically checked

`scripts/kernel-validate.mjs`, check `functional-parity`: the schema is parsed
and keyword-checked, and every fixture is classified — each valid one must
satisfy the schema and the record-level inference rules the schema subset cannot
express, each invalid one must be rejected by one or the other. The inference
rules enforced beyond the schema: the document carries at least one record;
assertion ids are unique across the whole record and each maps to one owning
facet; every per-assertion verdict names a declared assertion and every declared
assertion carries exactly one; every `covers` id and every post-change contract
link resolves — a link to the exact `{facet, id}` pair; a `VERIFIED` assertion
has both a covering evidence entry and a resolving link, and `contract_links`
may be empty only when nothing is `VERIFIED`; `relationship: same` references
every baseline condition id and no others, with no duplicate baseline condition
id; a record-scoped gap forces every declared per-assertion verdict and the
overall `UNVERIFIED`, an assertion-scoped gap only the assertions it names; an
unestablished baseline forces every per-assertion verdict and the overall
`UNVERIFIED`; the baseline and post-change source states are a distinguishable
`{identifier_kind, identifier}` pair. The adversarial cases in
`test/kernel-validate.test.mjs` prove the check goes red on an empty record set,
a missing baseline, non-comparable conditions, a mechanically-broken `same`
relation, a duplicate baseline condition id, an unclosed gap, a record-scoped
gap that still leaves an assertion `VERIFIED`, identical before/after source
states, a snapshot with no applicability justification, a `VERIFIED` assertion
with no resolving link, a contract link under the wrong facet, a duplicate
assertion id, an unestablished baseline that still carries a `VERIFIED`
assertion, an unknown form extension, a malformed schema, missing fixtures and a
misclassified fixture; and green on an honest all-`UNVERIFIED` record with an
empty `contract_links` array.

## Where it sits

This unit is part of the [verification router](../README.md), beside the
[regression route](../regression-testing/README.md) and the
[smoke protocol](../smoke-protocol/README.md). It is reached from the VERIFY
stage of the [standard task lifecycle](../../workflows/task-lifecycle.md) when
the change under verification is a `REFACTOR`; the
[REFACTOR execution protocol](refactor-protocol.md) sets the order its steps run
in, and the evidence contract is the authority on their sufficiency. It versions
with the Kernel release unit; it has no separate `VERSION`.
