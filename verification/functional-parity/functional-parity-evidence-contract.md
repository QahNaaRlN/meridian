---
title: Functional-parity evidence contract
document_type: standard
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-08-28
updated: 2026-08-29
---

# Functional-parity evidence contract

A portable, product-neutral contract for the one thing a `REFACTOR` change must
prove: that the observable behaviour of the system is the same before and after
an internal change (`change_class: REFACTOR` signature —
`$MERIDIAN_INSTANCE/.agent/technical-specifications/active/rule-resolution-normative-model.md`
§1.4). It says **what** must be recorded and compared and **what makes a
comparison sufficient**; it does not design the order in which a `REFACTOR` is
carried out. That execution order is a separate concern — the
[REFACTOR execution protocol](refactor-protocol.md) — which references this
contract as already-defined and adds no second evidence contract.

This document is Kernel: it carries only universal evidence kinds, the record
form, the semantic requirements and the inference rules. Every product-specific
thing a real comparison uses — the test runner, the snapshot or
characterisation-test framework, the exact commands, the configuration, the
paths where tests live, the tooling — is Instance/repository data and never
appears here or in the fixtures (§8).

## §1. When this applies

Apply when a change is classified `REFACTOR` and its acceptance therefore rests
on functional parity: the claim that the system's observable behaviour is
unchanged. It does not apply to `BUGFIX` (behaviour was wrong and is being
corrected), `FEATURE` (behaviour is new) or `BEHAVIOR_CHANGE` (the contract is
being changed on purpose) — for those, the [verification router](../README.md)
selects other evidence.

A functional-parity claim is proved per **assertion** about the observable
contract, not in one undivided step. An assertion whose parity is not
established leaves that assertion — and, through §9, the claim it belongs to —
`UNVERIFIED`; it does not sink the assertions that were established.

## §2. The preserved observable contract

The observable contract asserted unchanged is recorded across four facets. All
four are addressed in every record: a facet that genuinely does not apply
carries a stated justification for that; it is never simply omitted, because a
missing facet is a gap (§7), not a pass.

| Facet | What it covers |
|---|---|
| `public_api` | the declared surface a caller binds to — exported names, their signatures, their declared contracts |
| `observable_io` | the mapping from inputs to returned or emitted values, for the observation conditions the record fixes |
| `side_effects_and_interactions` | outward effects and interactions — writes, messages, calls to collaborators — in the set and order a caller can observe |
| `user_visible_behavior` | what a person using the system perceives — rendered state, timing that is part of the contract, surfaced errors |

Each facet holds one or more **assertions**. An assertion is a single statement
of what stays observably identical, with an id that the evidence entries,
the post-change contract links and the per-assertion verdict all reference.
"The code is cleaner" is not an assertion; "for every recorded input the
returned value is byte-identical" is.

An assertion id is **unique across the whole record**, the four facets
included, and the facet it is declared under is its **one owning facet**: the
gate rejects a repeated id and resolves every reference (a contract link, a
gap, a verdict entry) to the exact `{facet, id}` pair. A right id under the
wrong facet is not a match.

## §3. Baseline — the observation before the change

The baseline records the behaviour actually observed **before** the change, not
the behaviour expected of it. It carries:

- **an identifiable source state** — an identifier and the kind of identifier it
  is (a revision, a tag, a build id, a described environment state). The concrete
  identification scheme is repository data; that a state is pinned and nameable
  is the Kernel requirement. It must be a **distinguishable pair** from the
  post-change source state (§4): the same `{identifier_kind, identifier}` for
  both is not a before/after comparison, and the gate rejects it. The same
  `identifier` string under a different `identifier_kind` is a different pair.
- **the inputs and observation conditions** — the inputs exercised and the
  conditions they were exercised under, each carrying a **stable id**, unique
  within the baseline, so §4 can reference it rather than re-describe it.
- **provenance** — how the baseline observation was obtained, and whether it was
  actually established. A baseline that could not be run is recorded with
  `established: false`; that is a gap (§7), never a silent pass, and it forces
  **every** declared per-assertion verdict, and the overall verdict,
  `UNVERIFIED` — a record-scoped gap does not rescue one of them (§9).
- **the result actually observed** — what happened, in enough detail to compare
  against. An empty result is not an observation.

## §4. Post-change evidence — the observation after the change

The post-change evidence records the behaviour observed **after** the change,
against the same assertions. It carries:

- **an identifiable source state** after the change, in the same form as §3, and
  a **distinguishable pair** from the baseline's (§3).
- **the inputs and observation conditions**, related to the baseline's in one of
  exactly two ways, and the relation is **mechanically checked**, not asserted
  by a word in a description:
  - `same` — the post-change observation reproduced the baseline's conditions
    exactly. It carries `baseline_condition_ids` referencing them, and the gate
    requires that list to be **every** baseline condition id and **no others**
    (and the baseline to carry no duplicate id); it carries no restated `items`
    and no justification.
  - `explicitly-comparable` — the conditions differ. It restates them in full as
    `items` and carries a written `comparability_justification` for why the
    difference does not affect the assertion under test; it carries no
    `baseline_condition_ids`.
  There is no third option: conditions that are neither are a gap (§7), and the
  assertion they bear on is `UNVERIFIED`.
- **the result actually observed**, in the same form as §3.
- **contract links** — the field is always present; each entry names one
  preserved-contract assertion (§2) the observation bears on, by its exact
  owning facet and id (§2). The gate resolves every link to a declared
  `{facet, id}` pair; a right id under the wrong facet is rejected. A
  per-assertion `VERIFIED` requires at least one link here that resolves to the
  assertion — an evidence entry's `covers` list alone is not enough (§9). The
  array **may be empty only when no assertion is `VERIFIED`**: a record where
  every assertion is honestly `UNVERIFIED` owes no link. An observation may only
  claim the assertions it names: narrow evidence is not silently widened into a
  broader claim (§7).

## §5. Evidence kinds

Several neutral evidence kinds are available. None is mandatory, none is
primary, and none is the default: the set is unordered, and a record is built
from whichever kinds actually observe the assertions it makes. A single kind can
carry a whole record when it fully covers every assertion; different kinds can
each carry a record on their own.

| Kind | What it is | Standing limitations |
|---|---|---|
| `behavioral-assertion` | an automated assertion exercising the observable behaviour on the same inputs before and after | proves parity only for the inputs it exercises, not for the input domain; an assertion written to the new behaviour proves nothing about the old |
| `observation-log` | a recorded observation of the running unit under stated conditions | covers only the observed conditions; sensitive to what the observer thought to look at; weak on timing and transitions unless they are explicitly observed |
| `interface-enumeration` | an enumeration or diff of the declared public surface | compares declarations, not the behaviour reached through them; a surface can be identical while behaviour behind it differs |
| `interaction-trace` | a recorded trace of outward interactions and side effects | covers only the collaborators and effects actually traced; a effect not instrumented is invisible to it |
| `public-contract-snapshot` | a captured snapshot of the public contract compared before and after — see §6 | one kind among these, never the default; captures a serialised form only, says nothing about behaviour; a snapshot re-recorded during the change hides the difference it should show |
| `differential-execution` | the same inputs run through the old and the new implementation with the results compared | needs the old implementation to be runnable alongside the new; parity holds only across the inputs actually run |

Every evidence entry names its own limitations in addition to the standing ones
above; an unlisted limitation is treated as an unnamed one, and the entry does
not carry more weight than its stated scope.

## §6. The public-contract snapshot is one kind, not the answer

A snapshot of the public contract — a captured serialised form compared before
and after — is permitted **only as one of the evidence kinds in §5**, and only
when its use is explicitly justified for the case at hand. It is not a
universal or preferred proof of functional parity, and it is not selected in
advance as the way parity is shown.

- An evidence entry of kind `public-contract-snapshot` must carry an
  **applicability justification**: a statement of why, for this specific
  contract, a before/after snapshot of the captured form observes the whole of
  what the linked assertion says stays identical.
- Its limitations are named like any other kind's, and they are real: a snapshot
  captures a rendered form, not the behaviour reached through it; a snapshot
  regenerated, renamed or re-approved as part of the change stops being
  evidence of anything.
- Where another permitted kind already covers an assertion fully, a snapshot
  adds nothing and is not required. A record built entirely from
  `behavioral-assertion`, `differential-execution`, `observation-log`,
  `interface-enumeration` or `interaction-trace`, with every assertion covered,
  is complete without one.

## §7. Gaps and `UNVERIFIED`

The record does not let incomplete evidence read as complete. An evidence
document carries **at least one record**; a document with no record proves
nothing and is rejected. Within a record, each of the following is a **gap**,
recorded explicitly, and each leaves the assertion it bears on — or, when it
bears on the whole record, the whole claim — `UNVERIFIED`:

- an incomplete baseline (§3), including `provenance.established: false`;
- post-change conditions that are neither `same` nor justified
  `explicitly-comparable` (§4);
- a preserved-contract facet with no assertion and no not-applicable
  justification (§2);
- post-change evidence that does not observe the result for an assertion it is
  meant to cover (§4);
- an evidence kind used without the justification it requires — a
  `public-contract-snapshot` with no applicability justification (§6).

Three rules keep a narrow record from standing for a wide claim:

1. **Evidence is not widened.** An evidence entry's `covers` list, and a
   post-change contract link, may reference only assertions the preserved
   contract actually declares, and a link must resolve to the assertion's exact
   `{facet, id}` pair (§2, §4).
2. **`VERIFIED` is earned twice.** A per-assertion `VERIFIED` needs **both** at
   least one evidence entry that covers the assertion **and** at least one
   post-change contract link that resolves to it. Coverage without a link, or a
   link without coverage, is `UNVERIFIED`.
3. **Gap reach is fixed by scope.** A **record-scoped** gap forces **every**
   declared per-assertion verdict `UNVERIFIED` and the overall `UNVERIFIED` — it
   leaves no assertion standing, and this is what §9's "record-scoped gap"
   clause enforces. An **assertion-scoped** gap forces only the assertions it
   names `UNVERIFIED`; an assertion the gap does not name is free to be
   `VERIFIED` on its own evidence. This resolves the earlier wording that read
   as if a record-scoped gap only touched the overall verdict.

## §8. Product and repository boundary

This contract is the tool side. It defines the universal evidence kinds (§5),
the record form (the [schema](./functional-parity-evidence.schema.json)), the
semantic requirements (§2–§7) and the inference rules (§9). It stays true when
the product changes.

Everything a real comparison binds to is the product side and lives in
Instance/repository data, never here and never in the fixtures:

- concrete test runners, snapshot or characterisation-test frameworks and their
  configuration;
- the exact commands that produce an observation;
- the paths where a repository keeps its tests and recorded observations;
- product tooling of any kind, and any real repository, path or URL.

A repository names these in its own rules and manifests; the resolver and the
command registry point to them. The `kind` values in §5 carry no product tool
name, and no field in the record form requires one. A future product
application of this contract — a product recording its own functional-parity
evidence for a consolidation initiative — is Instance work (PHASE F of the
rule-resolution program), not a change to this document.

## §9. Verdict semantics

A record carries a per-assertion verdict and one overall verdict, each
`VERIFIED` or `UNVERIFIED`.

- **Per assertion.** `VERIFIED` requires all of: the preserved-contract
  assertion is declared, with a record-unique id under one owning facet (§2);
  the baseline provenance is established (§3); a post-change observation was made
  on `same` or justified `explicitly-comparable` conditions (§4); at least one
  evidence entry covers it (§5); at least one post-change contract link resolves
  to its exact `{facet, id}` pair (§4); and no gap names it (§7). Otherwise it
  is `UNVERIFIED`, with a stated reason; an `UNVERIFIED` assertion needs its
  reason but no contract link, provided a gap or the reason records the missing
  observation.
- **Record-scoped gap.** A gap with `scope: record` forces **every** declared
  per-assertion verdict `UNVERIFIED` and the overall verdict `UNVERIFIED`. It
  does not leave any assertion `VERIFIED` — the earlier wording that read as if
  it only touched the overall verdict is corrected here and in §7.
- **Unestablished baseline.** If `baseline.provenance.established` is `false`,
  **every** declared per-assertion verdict is `UNVERIFIED` and the overall
  verdict is `UNVERIFIED` — whatever gaps are recorded.
- **Distinguishable source states.** The baseline and post-change source states
  are a distinguishable `{identifier_kind, identifier}` pair (§3, §4); an
  identical pair is not a before/after comparison and no verdict rests on it.
- **Overall.** `VERIFIED` only when every declared assertion has exactly one
  per-assertion verdict, every one of them is `VERIFIED`, and no record-scoped
  gap is present. Any `UNVERIFIED` per-assertion state, or any record-scoped
  gap, makes the overall verdict `UNVERIFIED`, with a stated reason.

A record with no declared assertion at all is not a parity claim and cannot be
`VERIFIED`; an evidence document with no record at all proves nothing.

## §10. The evidence record

One record is the functional-parity evidence for one `REFACTOR` work item; an
evidence document carries **at least one** record. The form is
[`functional-parity-evidence.schema.json`](./functional-parity-evidence.schema.json),
exercised with product-neutral valid and invalid fixtures in
[`fixtures/functional-parity-evidence.fixtures.json`](./fixtures/functional-parity-evidence.fixtures.json).

## §11. Mechanical reach, and its limits

`scripts/kernel-validate.mjs`, check `functional-parity`. The schema and the
fixtures are reached in-gate: the schema is parsed and walked for unsupported
keywords, and every fixture is classified — each `valid` one must satisfy the
schema, each `invalid` one must be rejected by the schema, by the inference
rules, or by both. The inference rules of §2, §4, §7 and §9 that JSON Schema's
expressible subset cannot state, because they relate one part of a record to
another, are enforced against the same fixtures:

- the document carries at least one record;
- every preserved-contract assertion id is unique across the whole record, the
  four facets included, and maps to exactly one owning facet;
- every per-assertion verdict names a declared assertion, and every declared
  assertion carries exactly one per-assertion verdict;
- every evidence `covers` id resolves to a declared assertion;
- every post-change contract link resolves to the exact `{facet, assertion_id}`
  pair of a declared assertion — a right id under the wrong facet is rejected;
- a per-assertion `VERIFIED` has both a covering evidence entry and a resolving
  contract link — coverage alone is not enough, and an empty `contract_links`
  array is legal only when nothing is `VERIFIED`;
- any `UNVERIFIED` per-assertion state forces an `UNVERIFIED` overall;
- a record-scoped gap forces every declared per-assertion verdict, and the
  overall, `UNVERIFIED`; an assertion-scoped gap forces only the assertions it
  names;
- `relationship: same` references every baseline condition id and no others,
  with no duplicate baseline condition id;
- an unestablished baseline forces every declared per-assertion verdict, and the
  overall verdict, `UNVERIFIED`;
- the baseline and post-change source states are a distinguishable
  `{identifier_kind, identifier}` pair.

What the gate does not check, and this is a boundary, not a defect:

- **whether an assertion's statement is true of the code.** The gate sees that
  the record is internally consistent and that a verdict is backed by an
  evidence entry — not that the evidence entry's observation was actually made
  or actually showed parity. That is what review and the evidence artefacts are
  for.
- **whether the four facets are the right four for this change.** The gate
  enforces that all four are addressed; it cannot tell a real not-applicable
  from a facet that was waved away.
- **whether an `explicitly-comparable` justification is sound.** The gate
  requires the justification to be present; its adequacy is a review judgement.
- **Instance evidence records.** This check exercises the Kernel fixtures.
  Wiring a real Instance register of functional-parity evidence to this schema
  is PHASE F work.

## §12. Relation to the verification router and the task lifecycle

This contract sits in the [verification router](../README.md) beside the
[regression route](../regression-testing/README.md) and the
[smoke protocol](../smoke-protocol/smoke-protocol.md) as a portable verification
unit. It is reached from the VERIFY stage of the
[standard task lifecycle](../../workflows/task-lifecycle.md) when the change
under verification is a `REFACTOR` and the claim to prove is functional parity;
the [REFACTOR execution protocol](refactor-protocol.md) is what runs the steps
that produce this evidence in order. It adds no authority to enter another
program phase, to change product code, or to run anything with a side effect; it
only says what the evidence for a parity claim must contain and when that
evidence is sufficient.
