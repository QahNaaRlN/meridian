> **Статус документа:** поддерживается  
> **Последняя проверка:** 2026-08-18  
> **Владелец:** workspace-owner

# Regression testing

This directory routes to the existing regression methodology and repository-owned tests. It does not contain a copied protocol or centralized test commands.

## Authorities

- Methodology: `bugfix-protocol` skill. Vendored into the Kernel at [`skills/bugfix-protocol/SKILL.md`](../../skills/bugfix-protocol/SKILL.md) and pinned by SHA-256 in `skills/bugfix-protocol/PIN.yaml`. Product-specific conventions the protocol needs are Instance data (`$MERIDIAN_INSTANCE/skills/bugfix-protocol/context.md`); their absence is a blocker, not a licence to guess.
- Repository-specific test rules: the nearest repository `AGENTS.md`.
- Executable commands: repository manifests and the source-backed [command registry rules](../../registries/commands/README.md).
- Test location: beside or inside the owning product repository according to its local rules; tests are not moved into Engineering Workspace.

If the linked methodology is unavailable, do not reconstruct it from this summary. Report the missing source as a blocker before changing production code for a bugfix.

## Bugfix route

```text
Reproduce → RED → Fix → GREEN → relevant suite
```

The linked methodology remains authoritative for classification, behavioral assertion selection, mock boundaries, RED validity, human stop conditions, scope limits, and report detail. The route above adds no permission to bypass those gates.

`relevant suite` means the smallest existing suite required by the nearest repository rules for the affected contract. It does not automatically mean every test in the repository. If local rules require approval before a full suite, obtain it instead of silently expanding verification.

## Required evidence record

Keep the regression record separate from smoke evidence and include:

1. change type and expected-contract source;
2. reproducible input, expected behavior, actual behavior, and violated domain invariant;
3. repository revision and working-tree state for the RED run;
4. the specific behavioral assertion and runner output showing an assertion failure on the defective production code, not an environment/setup failure;
5. the minimal fix boundary;
6. GREEN output for the same assertion;
7. result of the repository-relevant suite selected under local rules;
8. checks not run, blockers, and claims that remain unverified.

Runtime/browser observations belong in a separate smoke record under Agent Smoke Workflow (`$MERIDIAN_INSTANCE/verification/smoke-testing/README.md`). They cannot replace RED/GREEN evidence.
