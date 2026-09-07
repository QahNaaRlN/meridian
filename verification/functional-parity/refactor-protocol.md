---
title: REFACTOR execution protocol
document_type: protocol
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-08-29
updated: 2026-08-29
---

# REFACTOR execution protocol

The order in which a `change_class: REFACTOR` work item is carried out so that its
acceptance claim — observable behaviour unchanged — rests on evidence that
already existed before the change and is compared against evidence taken after
it.

This protocol is the **execution order only**. *What* the evidence must contain
and *when a comparison is sufficient* is the
[functional-parity evidence contract](functional-parity-evidence-contract.md)
(PHASE D of `MERIDIAN-RULE-RESOLUTION-001`). This protocol does not restate that
contract, extend it, or define a second evidence contract; it references it as
already-defined and reaches it at CAPTURE BASELINE, CAPTURE POST-CHANGE and
COMPARE. The `REFACTOR` class signature is
`standards/workspace/rule-resolution.md` §3.1; the safe invariant for a defect
found inside a `REFACTOR` is §3.2.

It adds no authority to change product code, enter another program phase, run a
side-effecting command, or make a release/status transition. It is reached from
the VERIFY stage of the [standard task lifecycle](../../workflows/task-lifecycle.md)
and named as the `REFACTOR` route by the [verification router](../README.md) when
a change is classified `REFACTOR` and the claim to prove is functional parity.

## Invariants

- **Observable behaviour is preserved.** A change that alters observable
  behaviour, even partly, is not a `REFACTOR` (`rule-resolution.md` §3.1); it is
  a different work item, or an `initiative` that decomposes into several
  (`rule-resolution.md` §6).
- **Evidence is selected per assertion.** The evidence kinds are chosen from the
  contract's §5 set by what actually observes each assertion of this case. No
  kind — `public-contract-snapshot` included — is mandatory, primary, or the
  default (contract §5, §6).
- **No new evidence contract.** This protocol uses the PHASE D contract as it
  stands. It does not weaken it, duplicate it, or introduce an evidence kind or a
  record field the contract does not define.
- **A defect is not folded in.** A behaviour change discovered inside a
  `REFACTOR` is never fixed silently (see COMPARE). The original `REFACTOR` keeps
  its class; a separate `BUGFIX` work item is created, or the owner decides which
  path to take — the agent does not choose heuristically. The `REFACTOR` is
  paused if the defect blocks the parity proof (`rule-resolution.md` §3.2).
- **Worktree isolation is not required.** A `git worktree`, or any other specific
  working-tree isolation tool, is one possible execution practice, not a step of
  this protocol.
- **A wider file set re-resolves.** If IMPLEMENT changes paths beyond those the
  work item's rule resolution ran on, the resolution is re-run — or its
  invariance explicitly confirmed — before VERIFY: `changed_paths` diverging from
  the `candidate_paths` a prior resolution used sets `requires_reresolution`
  (`rule-resolution.md` §3.1 input; resolver output).

## The protocol

```text
CLASSIFY → DEFINE PARITY → CAPTURE BASELINE → IMPLEMENT →
CAPTURE POST-CHANGE → COMPARE → REPORT
```

Do not enter a later step while an earlier step's gate is still open.

### CLASSIFY

Confirm the work item is a `REFACTOR` by the class signature
(`rule-resolution.md` §3.1): the external contract — public API, observable
input/output, side effects and interactions, user-visible behaviour — is
asserted identical before and after, and only internal structure changes. If
observable behaviour is meant to change, stop: the work item is `BUGFIX`,
`FEATURE`, `BEHAVIOR_CHANGE`, or an `initiative` to decompose. An ambiguous
classification is a stop — ask the owner, do not pick the convenient branch.

Record the classification and how rule resolution routed this work item to this
protocol: the route's `source`, `scope`, and its `digest` or `revision`.

### DEFINE PARITY

State the preserved observable contract as the evidence contract's four facets
(§2), before any code changes:

| Facet | For this work item |
|---|---|
| `public_api` | the exported surface callers bind to — names, signatures, declared contracts |
| `observable_io` | the input→output mapping, for the observation conditions this record fixes |
| `side_effects_and_interactions` | outward effects and collaborator interactions, in the set and order a caller can observe |
| `user_visible_behavior` | rendered state, timing that is part of the contract, surfaced errors |

Every facet is addressed. A facet that genuinely does not apply carries a stated
justification for that; it is never simply omitted, because a missing facet is a
gap, not a pass (§2, §7). Under each facet write one or more **assertions** —
single statements of what stays observably identical, each with a record-unique
id under one owning facet (§2). "The code is cleaner" is not an assertion; "for
every recorded input the returned value is byte-identical" is.

Choose, per assertion, which of the contract's §5 evidence kinds will observe it,
and why that kind observes the whole of what the assertion claims. A
`public-contract-snapshot` chosen here carries its applicability justification
(§6). The selection is the smallest set that actually observes each assertion —
not "snapshot everything".

### CAPTURE BASELINE

Observe the behaviour **as it is now**, before the change, under the contract's
§3 requirements:

- pin an identifiable source state — an identifier and the kind of identifier it
  is — that will be a distinguishable `{identifier_kind, identifier}` pair from
  the post-change state (§3, §4);
- fix the inputs and observation conditions, each with a stable id the
  post-change step can reference instead of re-describing (§3);
- run the selected evidence and record the result actually observed, in enough
  detail to compare against — an empty result is not an observation (§3);
- record provenance: how the observation was obtained, and whether it was
  established. A baseline that could not be run is recorded with
  `established: false` — a gap, never a silent pass, and it forces every declared
  per-assertion verdict and the overall verdict `UNVERIFIED` (§3, §9).

Gate: the baseline is recorded before IMPLEMENT touches the code. A baseline
reconstructed after the change is not a baseline.

### IMPLEMENT

Change internal structure only — file layout, decomposition, internal naming,
architectural boundaries. Keep the change minimal for the parity claim: no
behaviour change, no opportunistic fix, no bundled `BUGFIX`, no unrelated
formatting or renaming beyond what the refactor is.

If the file set grows beyond what rule resolution ran on, stop and re-resolve —
or confirm the resolution is unchanged — before continuing.

If a behaviour difference surfaces here — the code does not do what the baseline
recorded — do not fix it in this work item. Carry it to COMPARE and the defect
rule below.

### CAPTURE POST-CHANGE

Observe the behaviour **after the change**, against the same assertions, under
the contract's §4 requirements:

- pin the post-change source state, a distinguishable pair from the baseline's
  (§4);
- relate the observation conditions to the baseline's in exactly one of two
  mechanically-checked ways (§4): `same` — reproduced exactly, referencing every
  baseline condition id and no others, with no restated items and no
  justification; or `explicitly-comparable` — restated in full with a written
  comparability justification for why the difference does not affect the
  assertion under test. Conditions that are neither are a gap, and the assertion
  they bear on is `UNVERIFIED` (§4, §7);
- record the result actually observed, in the same form as the baseline (§4);
- for each assertion an observation bears on, add a **contract link** to its
  exact `{facet, id}` pair (§4). An observation claims only the assertions it
  names; narrow evidence is not widened into a broader claim (§7).

### COMPARE

Derive the verdict from the two observations under the contract's §7 and §9 —
per assertion, then once overall:

- a per-assertion `VERIFIED` needs **all** of: the assertion declared with a
  record-unique id under one owning facet (§2); established baseline provenance
  (§3); a post-change observation on `same` or justified `explicitly-comparable`
  conditions (§4); at least one evidence entry covering it (§5); at least one
  post-change contract link resolving to its exact `{facet, id}` pair (§4); and
  no gap naming it (§7). Otherwise it is `UNVERIFIED` with a stated reason — an
  honest `UNVERIFIED`, recorded, not an omission;
- an assertion left `UNVERIFIED` does not sink the assertions that were
  established (§1, §7);
- a record-scoped gap, and an unestablished baseline, each force **every**
  declared per-assertion verdict and the overall verdict `UNVERIFIED` (§9);
- the overall verdict is `VERIFIED` only when every declared assertion is
  `VERIFIED` and no record-scoped gap is present; any `UNVERIFIED` per-assertion
  state or any record-scoped gap makes it `UNVERIFIED`, with a stated reason
  (§9).

**A behaviour difference found here, or during IMPLEMENT, is a defect — not a
result to normalise:**

1. Do not rewrite the baseline, the assertion, or a snapshot to match the new
   behaviour. That hides the difference the record exists to show (contract §5,
   §6).
2. Do not fix the defect inside this `REFACTOR` work item. Create a separate
   `BUGFIX` work item — its own class, its own protocol route — or ask the owner
   which path to take. The agent does not decide this heuristically
   (`rule-resolution.md` §3.2).
3. The original `REFACTOR` keeps its `REFACTOR` classification. It is **not**
   reclassified.
4. Pause the `REFACTOR` if the defect blocks the parity proof. If it does not,
   the parity claim for the unaffected assertions stands on its own evidence and
   the defect is tracked in the separate work item.

### REPORT

Report, as the task lifecycle's REPORT stage requires:

- outcome: the overall verdict, and every per-assertion verdict with the reason
  for each `UNVERIFIED` one;
- the preserved observable contract — the four facets and their assertions — the
  baseline and post-change source states, the evidence kinds used with each
  kind's stated limitations, and the explicit gaps;
- files and repositories changed;
- how rule resolution routed this work item to this protocol (`source`, `scope`,
  `digest`/`revision`), and any re-resolution triggered by a widened file set;
- any defect found, and the separate `BUGFIX` work item or owner decision it
  produced — never a silent fix;
- required evidence that could not be produced, and the claim that therefore
  stays `UNVERIFIED`.

The functional-parity evidence record is kept separate from regression and smoke
evidence (verification router). A passing narrower check is not a parity verdict;
only the evidence contract's §9 rules assign one.

## Where it sits

Part of the [functional-parity verification unit](README.md), beside the
[evidence contract](functional-parity-evidence-contract.md) it executes against.
It versions with the Kernel release unit; it has no separate `VERSION`. It is one
of the three verification routes in the [verification router](../README.md),
beside the [regression route](../regression-testing/README.md) and the
[smoke protocol](../smoke-protocol/smoke-protocol.md), and — like them — carries
no authority beyond saying in what order the evidence for its claim is produced.
