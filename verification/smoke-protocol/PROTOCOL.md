# Smoke protocol

Portable rules for observing runtime/browser behavior and turning the
observation into a verdict that cannot overstate itself. Product bindings —
environments, applications, flows, access, capabilities, test data — are
Instance data in the product's smoke unit; this document never names them.

## §1. Activation

Apply on an explicit request for a smoke test, a runtime/browser check, or a
verification of a fix's observable behavior. Code analysis and unit tests do
not substitute for requested observable runtime evidence.

## §2. Inputs

Fix the target behavior, success predicate, required evidence, application
and environment before acting. If the environment cannot be derived
unambiguously from the request or an applicable policy, ask; never default.

## §3. Computed light path

A reduced-ceremony path is computed, not chosen:

```text
change_under_test == null
AND max(selected_flows.side_effect_tier) == 1
```

The light path keeps the immutable Target, Predicate, Evidence and the full
§11 matrix. It relaxes no auth, config or evidence requirement.

## §4. Required configuration

```text
required_config = union(selected_flow.requires_config) ∪ workflow_level_requires_config
```

A `REPLACE_ME` inside the required set is a blocker. A `REPLACE_ME` outside
it does not block and is recorded as configuration debt.

## §5. Immutable Test Contract

Before the target action, fix and never weaken:

```yaml
target_behavior: ...
success_predicate: ...
required_evidence: [...]
environment: ...
environment_basis: ...
flows: [...]
adjacent_checks: [...]
out_of_scope: [...]
side_effect_tier: ...
budget: ...
provenance_requirement: ...
change_under_test: ...
```

A wrong contract does not earn the current run a `PASS`; a corrected
contract starts a new run.

## §6. Provenance

Use only the provenance resolver declared for the selected environment. The
requirement lives in the Test Contract; the factual result lives in the
Execution Context and cannot retroactively weaken the contract.

- Local: confirm the change is present in the running process via applicable
  reload/restart evidence.
- Deployed: confirm `change_under_test ∈ deployed build` via the declared
  authoritative deployment-metadata source. The CI/hosting pipeline status is
  not a universal fallback. A shell login to a host MAY serve as a negative
  pre-check only; it does not by itself confirm a deployment.

## §7. Access and credentials

Resolve the role via the smoke unit's access references; `null` means the
declared default role. Retrieve external credential references only. Secrets
are never written into the repository, the Test Contract, the Execution
Context, or the report.

## §8. Test data

1. Prefer existing, explicitly permitted test entities.
2. Never use production personal/sensitive data for smoke.
3. Create data through UI/API only if the selected flow requires it and the
   side-effect gate has passed.
4. Never guess IDs, states or business prerequisites.
5. Record source, created IDs, mutations and cleanup status in the Execution
   Context; cleanup is not assumed successful without evidence.
6. If required test data is unavailable before the target action, that fact
   flows into §11 as a blocker.

## §9. Side-effect gate

| Tier | Meaning | Confirmation |
|---|---|---|
| 1 | Navigation / read-only UI state; no persistent mutation. | None, when the task explicitly requests a smoke run. |
| 2 | Creation or reversible mutation of test data. | Explicit basis in the request or the confirmed Test Contract. |
| 3 | Hardware-actuating action, external notification, payment, irreversible or costly action, production mutation. | Explicit user confirmation for the specific action. |

Compute the maximum tier of the selected flows. Confirmation of one action
never extends to other side effects. If a required confirmation is absent,
the target action is not executed and that fact flows into §11.

## §10. Execution Context

After execution, record facts only:

```yaml
environment_result: ...
provenance_result: ...
credential_role_result: ...
test_data_result: ...
capabilities_result: ...
target_action_executed: true | false
predicate_evaluable: true | false
predicate_result: true | false | null
evidence_result: [...]
```

`target_action_executed = true` means the system accepted the action — not
merely that the agent clicked or sent a request.

## §11. Single Verdict Authority

Only this section assigns the verdict. Policies, flows and resolvers produce
facts.

| Facts | Verdict |
|---|---|
| Target action not executed | `BLOCKED` |
| Target action executed, predicate not evaluable | `INCONCLUSIVE` |
| Predicate evaluated false | `FAIL` |
| Predicate true, required evidence incomplete or unavailable | `INCONCLUSIVE` |
| Predicate true, required evidence complete | `PASS` |

## §12. Adjacent checks

If the primary contract is confirmed and an independent adjacent check is
broken, the report carries `PASS + REGRESSION`. If the adjacent failure makes
the primary predicate false or the evidence untrustworthy, the ordinary §11
matrix applies.

## §13. Capabilities

`unknown → probe-on-use → true/false → TTL expiry → unknown`. Record the
capability id, result, timestamp, executor/environment, evidence and expiry.
`unknown` is probed first; it never silently becomes a blocker or a
permission.

## §14. Evidence

Collect minimally sufficient evidence: screenshot, observable UI state, URL,
entity ID, console/network error, generated identifier. `PASS` without the
complete `required_evidence` set is forbidden.

## §15. Report

The report MUST include: environment/application, Test Contract, Execution
Context, the §11 verdict, evidence, adjacent regressions, blockers,
configuration debt and out-of-scope items.

## §16. Budget and stop conditions

Budget exhausted before the target action → `BLOCKED`; after the target
action with an unevaluable predicate → `INCONCLUSIVE`. Never continue
uncontrolled exploration beyond the Test Contract.

## §17. Invariants

- No fake PASS.
- No implicit environment.
- No secret storage.
- Minimal sufficient scope.
- Observable behavior over code appearance.
- Repeat after any fix made from smoke evidence.
- Do not invent business flows or unresolved configuration.
