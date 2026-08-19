---
title: Feedback and metrics during field use
document_type: standard
status: draft
scope: workspace
owner: workspace-owner
created: 2026-08-19
updated: 2026-08-19
related_documents:
  - ./document-quality.md
  - ../../workflows/task-lifecycle.md
  - ../../verification/README.md
canonical_url:
supersedes:
superseded_by:
---

# Feedback and metrics during field use

This is methodology, not data: it defines what to record and why, and stays
in the Kernel so it travels to the next product unchanged. What gets
recorded — the actual friction entries and gate-run history — is Instance
data, product-specific and never published with the Kernel.

The rule this document exists to prevent: turning feedback collection into
its own ceremony that competes with the work it is supposed to observe. Two
streams, both cheap, both optional to skip when there is nothing to report,
neither a dashboard.

## Stream 1 — friction log (qualitative)

A single append-only file: `$MERIDIAN_INSTANCE/.agent/feedback/friction-log.md`.

Append one entry during REPORT whenever a Kernel rule, document, protocol,
or gate check cost more than it should have — fought the task instead of
serving it. Do not append when nothing fought back; a quiet day produces no
entry, not a "no friction" line. One entry:

```text
### YYYY-MM-DD — <one-line summary>
task: <bugfix | feature | audit | ...>
where: <file or rule that caused friction>
category: <false-positive gate | missing rule | unclear wording |
           working-as-intended-but-costly | other>
what happened: <one to three sentences, facts not feelings>
```

`category` is deliberately closed-vocabulary: it forces a friction entry to
say *what kind* of problem this is, not just that something felt slow. A
recurring `false-positive gate` or `missing rule` entry is a CHANGELOG
candidate; a recurring `working-as-intended-but-costly` entry is a
prioritization signal, not automatically a bug.

Review the log periodically (weekly is a reasonable default, not a rule) and
turn recurring entries into Kernel changes through the ordinary path: a
defect found, a fix made, `CHANGELOG.md` updated. This is the same loop the
system already runs on itself; the friction log just makes the "find" step
mechanical instead of dependent on memory.

## Stream 2 — gate-run log (quantitative)

`node scripts/validate-and-log.mjs` is a transparent wrapper around
`kernel-validate.mjs`: same output, same exit code, plus one JSON line
appended to `$MERIDIAN_INSTANCE/.agent/metrics/validate-log.jsonl` per run
— but only when run against a real Instance. A run with no Instance or
against the in-repository CI fixture is not logged: it carries no
operational signal and would only dilute the trend.

Record fields: `ts`, `kernel_version`, `kernel_revision`,
`instance_revision`, `exit_code`, `failing`, `warnings`, `info_ok`,
`fail_messages`, `warn_messages` (each truncated to 20 entries — a log
built to answer "is this trending better" does not need to be a full
transcript).

Use `hooks/pre-push` (already wired to this wrapper) as the primary
collection point: every push produces one data point with zero extra
operator action. Reading the trend is a separate, occasional act — grep or
a small ad-hoc script over the JSONL, not a maintained dashboard. What to
look for: `failing` should be zero on every logged run (a red run should
never reach a push in the first place); a `warnings` count that climbs
instead of holding flat is worth a friction-log entry naming the recurring
`warn_messages` value.

## What this deliberately does not do

No sentiment score, no "how did this feel" survey, no automated rollup
report, no CI upload. The data stays local to the Instance, exactly like
every other product fact. If a future need justifies a rollup script, it is
built when that need is concrete — not pre-built for data that does not
exist yet.
