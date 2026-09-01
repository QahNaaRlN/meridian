---
title: Meridian Kernel — agent bootstrap
document_type: protocol
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-08-27
updated: 2026-09-01
topic: agent-conduct
profile: universal
delivery: agents-md-section
activation: always
---

# Meridian Kernel — agent bootstrap

This file is a thin, portable entry point for any agent about to work on
Meridian itself (Kernel, Instance, or delivery adapters). It carries no
product facts, no permanent role assignment, and no plan content — those
live in Instance documents this file only points to. Follow the steps below
in order before making any change.

<!-- meridian:begin instruction-section id=preflight owner=workspace-owner generated=no -->
## 1. Preflight before touching Meridian

Before editing anything in this repository, run the Kernel preflight check:

```bash
node scripts/preflight.mjs
```

Do not proceed on a session wired to stale roots or missing environment
variables — preflight exists precisely to fail loudly instead of letting the
agent read the wrong Kernel/Instance pair silently.

If preflight reports that `MERIDIAN_INSTANCE` is missing, apply this recovery
rule before asking the owner to change their shell:

1. If the current task or trusted machine-local configuration identifies one
   unambiguous Instance path, rerun preflight yourself with
   `MERIDIAN_KERNEL` and `MERIDIAN_INSTANCE` set inline for that command. Use
   the same inline environment for later Meridian commands in the session.
2. Do not require the owner to export the variable and return with a manual
   confirmation when the path is already known.
3. If no path is known, or several candidates are plausible, remain
   fail-closed and ask the owner which Instance is authoritative.

An absolute product path is machine-local adapter configuration. Never add it
to this portable Kernel document.
<!-- meridian:end instruction-section id=preflight -->

<!-- meridian:begin instruction-section id=instance-resolution owner=workspace-owner generated=no -->
## 2. Resolve `MERIDIAN_INSTANCE`

Resolve the `MERIDIAN_INSTANCE` environment variable to the root of the
current product's Instance repository. Kernel does not store this path
itself; it only knows the variable name (`standards/workspace/kernel-boundary.md`).
Without it, product-specific facts and governance documents referenced below
are not reachable, and the checks in `scripts/kernel-validate.mjs` report
UNVERIFIED rather than a false pass.
<!-- meridian:end instruction-section id=instance-resolution -->

<!-- meridian:begin instruction-section id=owner-intent owner=workspace-owner generated=no -->
## 3. Owner intent contract, if it exists

If `$MERIDIAN_INSTANCE/governance/meridian-owner-intent-contract.md` exists,
read it in full before doing anything else. It is the canonical record of
the owner's vision for Meridian — what it is, its base operating model, and
what would break that vision. It is not a restatement of the current
program plan, and it does not go stale when a plan finishes.
<!-- meridian:end instruction-section id=owner-intent -->

<!-- meridian:begin instruction-section id=collaboration-protocol owner=workspace-owner generated=no -->
## 4. Collaboration protocol and task-local role, if it exists

If `$MERIDIAN_INSTANCE/governance/meridian-self-development-collaboration-protocol.md`
exists, read it in full and determine your current task-local role from it.
This protocol governs only work on Meridian Kernel/Instance/adapters (and
product instructions when they are changed as part of that work) — it does
not apply automatically to ordinary product development.
<!-- meridian:end instruction-section id=collaboration-protocol -->

<!-- meridian:begin instruction-section id=active-plan owner=workspace-owner generated=no -->
## 5. Active plan

Read the active program plan referenced by the collaboration protocol (or,
absent a collaboration protocol, whichever plan document in
`$MERIDIAN_INSTANCE/.agent/plans/active/` the current task names) before
proposing or starting work.
<!-- meridian:end instruction-section id=active-plan -->

<!-- meridian:begin instruction-section id=role-behavior owner=workspace-owner generated=no -->
## 6. Behavior by role

**If your current role is reviewer/Git integrator with a separately assigned
external executor:**

- do not implement the package yourself;
- automatically produce a paste-ready instruction for the executor;
- automatically produce a correction instruction after review findings, without
  waiting for the owner to ask again;
- independently perform review, gates, and Git integration. With an external
  executor assigned, the Git integrator owns branch creation and switching,
  staging, commit, tag, and post-merge verification; the executor performs none
  of these Git operations. Advancing the integration line by merging the
  accepted feature branch is the Git integrator's step **unless** an accepted
  tracked protocol assigns that merge to the owner (owner-managed Merge Request,
  below). To run gates that must see files not yet tracked, the Git
  integrator may temporarily stage a name-limited candidate package, reviewing
  the staged name list and staged diff first; temporary staging is not
  acceptance — a `CHANGES_REQUESTED` verdict unstages it with a path-limited
  `git restore --staged -- <candidate-path>...` (never a whole-index reset,
  which would drop unrelated staged state) without touching the working tree,
  and only the finally approved package is staged after `ACCEPTED`. The branch
  and versioning rules themselves are
  `standards/workspace/version-control-flow.md` and
  `standards/workspace/release-versioning.md`.
- **owner-managed Merge Request (MR).** When an accepted tracked protocol
  declares it, the final merge of an accepted feature branch into the declared
  integration line is performed by the **owner** through the platform's web
  interface, not by the Git integrator locally. In that mode the Git integrator
  still creates the branch, makes the single package commit, and prepares the
  branch for publication, and after the owner's merge it verifies the actual
  history and re-runs the gates; it does not run a local merge into the
  integration line. Publishing the branch and opening the MR are external
  actions, done on the owner's explicit instruction or by the tracked protocol
  that assigns this order; if the Git integrator cannot publish the branch or
  open the MR, it hands the owner the exact command and the source/target, it
  does not work around the limit. The external executor receives no Git rights
  at any point. "MR" is the platform-neutral term (equivalent to a pull
  request); the platform may be GitLab, GitHub, or another. This is an
  owner-chosen reinforcement for work on Meridian itself — it is not a universal
  Meridian requirement, and it changes neither the promotion mode, the stable
  line, the release flow, nor the non-fast-forward advancement-commit rules
  (`standards/workspace/version-control-flow.md` §5.3).
  - **The MR is merged as an ordinary merge commit that keeps the accepted
    package commit in history.** Squash is forbidden, rebase is forbidden, and a
    fast-forward with no merge commit is forbidden: the review verified that
    exact commit, so the integration line must attach it, not replace it with a
    squashed or rewritten commit. After the merge the accepted package commit
    must stay reachable from the integration line and a distinct merge commit
    must appear in its history. If the platform offers no such merge method, the
    owner does not press merge and reports the blocker. The Git integrator's
    post-merge verification then confirms: (1) the package commit is reachable
    from the integration line; (2) a distinct merge commit was created;
    (3) the accepted package diff was not rewritten; (4) the post-merge gates
    pass.

**If no separate reviewer is assigned:**

- do not invent one;
- work under the base model — owner plus one executor — as described in the
  owner intent contract.
<!-- meridian:end instruction-section id=role-behavior -->

<!-- meridian:begin instruction-section id=source-of-truth owner=workspace-owner generated=no -->
## 7. Source of truth

Do not treat chat history as the canonical record of an agreement when a
corresponding tracked document exists. A tracked document in Instance
(owner intent contract, collaboration protocol, active plan) always
supersedes what a prior conversation implied.
<!-- meridian:end instruction-section id=source-of-truth -->
