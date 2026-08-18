# Commands

This area records how an agent finds and selects repository-owned commands. It does not replace package or build manifests.

Every populated command cites its source file and pointer, states applicability and prerequisites, and exposes side-effect and execution-approval classification. Commands run from the corresponding repository root.

Rules:

- `exact` means the command is copied from a manifest script or explicit local rule.
- `template` requires replacing angle-bracket placeholders before execution.
- `external` or `unknown` side effects always require separate execution approval.
- install, service, migration, maintenance, and release commands always require separate execution approval.
- absence from this registry does not prove a command is forbidden, but it does mean the Workspace cannot recommend it without returning to the repository source.
- registry presence is not execution evidence.

The Core `AGENTS.md` currently mentions `composer run dev`, but `composer.json` has no `dev` script. That command is intentionally excluded until the source conflict is resolved. Install commands are also excluded: lockfile presence identifies the package ecosystem but does not by itself establish the approved install operation for this Workspace.
