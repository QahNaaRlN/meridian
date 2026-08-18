# Standard task lifecycle

This is the workspace-level route for a task that may span repositories. It coordinates decisions; it does not replace the current user request, the nearest repository `AGENTS.md`, repository manifests, or an applicable verification protocol.

```text
UNDERSTAND → INVESTIGATE → PLAN → IMPLEMENT → VERIFY → REPORT
```

Do not enter a later stage while a required fact, decision, authorization, or earlier-stage gate remains unresolved.

## UNDERSTAND

1. State the requested outcome and classify the task: explanation/audit, bugfix, feature, intentional behavior change, or operational verification.
2. Identify the expected contract or acceptance criteria and distinguish them from current implementation behavior.
3. Bound the candidate repositories through the [inventory rules](../registries/inventory/README.md) and the Instance data they govern (`$MERIDIAN_INSTANCE/inventory/`). Do not add a repository merely because it is adjacent.
4. Read `C:\Work\AGENTS.md` and every applicable nearest repository `AGENTS.md` before relying on local rules.
5. Record what the request authorizes. Product edits, commits, service starts, deployments, test-data creation, physical printing, and other external effects require their own applicable authority.

For an explanation, audit, or status report, stop before implementation unless the user also requested a change.

## INVESTIGATE

1. Inspect the current state read-only before editing and preserve unrelated working-tree changes.
2. Trace the smallest source path that can establish the contract, ownership, root cause, and affected boundary.
3. Resolve commands from repository-owned sources or the [command registry rules](../registries/commands/README.md) and its Instance data (`$MERIDIAN_INSTANCE/commands/repositories.yaml`); do not invent or normalize them in this workflow.
4. Separate verified facts, inferences, and unresolved blockers. An unresolved registry entry is not runtime evidence.
5. For a bugfix, capture reproducible input, expected behavior, actual behavior, and the violated domain invariant before changing production code.

## PLAN

Define the smallest authorized change and its proof before implementation:

- repositories and files expected to change;
- applicable local rules and source-backed commands;
- acceptance assertions and the cheapest layer where each is observable;
- targeted checks, relevant suites, and any runtime/smoke evidence that is actually required;
- approvals or environment facts required before execution;
- explicit exclusions that prevent adjacent cleanup or broader repository exploration.

Choose the verification strategy through the [verification router](../verification/README.md). A broader or more expensive check is not automatically stronger evidence if it does not observe the required invariant.

For a bugfix, use the [regression route](../verification/regression-testing/README.md):

```text
Reproduce → RED → Fix → GREEN → relevant suite
```

Do not plan an implementation when the expected contract is materially ambiguous; return the decision required to proceed.

## IMPLEMENT

1. Change only the authorized files and repositories.
2. Follow each repository's local architecture, test, formatting, and command rules.
3. Keep the change minimal; do not combine a bugfix with refactoring, renaming, formatting, or unrelated cleanup.
4. Preserve user-owned changes and stage or commit only task-owned files when those actions are authorized.
5. Stop if implementation invalidates the contract, scope, test plan, or required authorization established earlier.

## VERIFY

Execute the previously selected evidence path in increasing scope:

1. the smallest assertion or static check that directly observes the changed contract;
2. the repository-relevant suite required by the nearest `AGENTS.md`;
3. cross-repository integration checks only for boundaries the change actually affects;
4. smoke verification only when observable runtime behavior is required and its environment, access, and side effects are authorized.

Do not silently expand to a full suite, another repository, a live environment, or a side-effecting action. If the selected proof cannot run, report the concrete blocker and the unverified claim; do not substitute unrelated passing checks.

Regression evidence and smoke evidence have different contracts and must be recorded separately as described by the [verification router](../verification/README.md). Only the current Agent Smoke Workflow §11 may assign a smoke verdict.

## REPORT

Report:

- outcome and remaining blockers;
- files and repositories changed;
- checks selected, commands resolved from their owning sources, and results;
- bugfix RED/GREEN evidence when applicable;
- smoke evidence and §11 verdict when applicable;
- required checks not run and why;
- confirmation that unrelated changes and excluded scope were not modified.

A passing targeted check proves only its stated invariant. Do not claim a broader repository, runtime, or release outcome from narrower evidence.

## Authority and scope invariants

- Current user request controls the authorized outcome and mutations.
- Nearest repository `AGENTS.md` controls repository-specific implementation and test rules.
- Repository manifests control executable command definitions.
- The [regression route](../verification/regression-testing/README.md) points to the existing bugfix methodology instead of reproducing it.
- The Agent Smoke Workflow (`$MERIDIAN_INSTANCE/verification/smoke-testing/README.md`) controls smoke execution and verdicts.
- This lifecycle never creates authority for a baseline task, product changes, live execution, acceptance experiments, or release/status transitions.
