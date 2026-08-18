> **Статус документа:** поддерживается  
> **Последняя проверка:** 2026-08-18  
> **Владелец:** workspace-owner

# Verification

Select the smallest evidence path that directly proves the task's acceptance criteria. Repository-specific test rules stay in the nearest repository `AGENTS.md`; exact commands stay in repository manifests and the source-backed [command registry rules](../registries/commands/README.md).

## Strategy router

| Changed or claimed contract | Primary evidence | Add only when applicable |
|---|---|---|
| Documentation, schema, registry, link, or adapter structure | parse, schema, link, path, or static conformance check | a consumer check when the task changes a consumed contract |
| Existing product behavior is defective | [regression route](regression-testing/README.md) with behavioral RED and GREEN | repository-relevant suite; integration evidence for an affected boundary |
| New or intentionally changed product behavior | acceptance assertions selected under repository-local rules | relevant suite and affected boundary checks |
| Contract crosses repositories or processes | targeted checks on each changed side plus an integration/contract observation | runtime evidence only if static/automated proof cannot establish the claim |
| Observable behavior in a running application or browser | applicable automated evidence plus Agent Smoke Workflow (`$MERIDIAN_INSTANCE/verification/smoke-testing/README.md`) | only authorized environments, roles, data, and side effects |

Choose by the claim being proved, not by test prestige. Do not broaden to a full suite, E2E, browser, deployed environment, or physical side effect unless the contract, nearest rules, or user request requires it.

## Evidence boundary

Regression evidence proves that an automated behavioral assertion failed on the defective implementation and passed after the fix, followed by the locally required relevant suite. It records source revision/tree state, the specific assertion, runner result, and any unverified remainder.

Smoke evidence observes a configured runtime action and predicate. It requires the smoke package's Test Contract, Execution Context, provenance, evidence set, and §11 computed verdict. A manual observation or passing regression test is not a smoke verdict.

The evidence types complement but do not replace one another:

- smoke PASS does not prove that a regression assertion was RED before the fix;
- regression GREEN does not prove deployed or browser behavior;
- static checks do not prove runtime reachability;
- a blocked higher layer does not erase valid narrower evidence, but the broader claim remains unverified.

## Scope gate

Before executing a check, confirm that its repository, environment, prerequisites, data, and side effects are within the task. If not, request the missing authority or report a concrete blocker. Never use verification as a reason to modify unrelated code, generate unrequested fixtures, or enter another planned phase.
