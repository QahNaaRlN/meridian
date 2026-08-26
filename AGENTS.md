---
title: Meridian Kernel — agent bootstrap
document_type: protocol
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-08-27
updated: 2026-08-27
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
- independently perform review, gates, and Git integration.

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
