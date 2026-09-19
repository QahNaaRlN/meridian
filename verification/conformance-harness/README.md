---
title: Conformance harness
document_type: readme
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-09-17
updated: 2026-09-19
---

# Conformance harness

Package `rust-conformance-harness` (package 2 of the `meridian-rust-migration`
program). This is an **independent verdict-comparison mechanism**: it runs two
verdict producers, captures each one's exit code, stdout and stderr
separately, normalizes what each side observably emitted, and compares the
two normalized results to an explicit `conformant` / `divergent` /
`harness_error` conclusion.

## What this is not

- Not a command of the future Meridian CLI (`meridian-cli-rfc.md`).
- Not part of the Node.js reference implementation and not part of any future
  Rust implementation — it does not belong to either side it may one day
  compare.
- Does not define `Verdict`, `Diagnostic`, a resolver, or any other Meridian
  domain concept (`meridian-rust-target-architecture.md` §2, §5). It only
  understands "a process ran, here is what it printed and how it exited".
- Does not substitute synthetic self-checks for product comparison. Package 4
  adds a real Node.js/Rust comparison for the introduced YAML and JSON Schema
  surface while retaining the synthetic cases that prove the mechanism can
  detect divergence and harness failure.

## Why a separate artifact

If this mechanism lived inside the Node implementation or inside the future
Rust implementation, the implementation being judged would also be the judge.
An overly aggressive normalization step could hide a real divergence and no
side effect would reveal it, because the mechanism finding the defect and the
mechanism containing the defect would be the same code
(`meridian-cli-rfc.md`, decision D-F).

## Contents

- [`conformance-harness.mjs`](conformance-harness.mjs) — the one
  implementation: `runProducer` (spawn, capture exit code/stdout/stderr
  separately, no shell string ever assembled), `normalizeDiagnostics` (parse
  `FAIL`/`WARN` lines, unify CRLF/LF, keep duplicates, flag unparseable
  diagnostic-looking lines instead of dropping them), `compareVerdicts`
  (order-independent multiset diff of FAIL and WARN, plus exit-code
  equality), and `runCase` (one producer pair, one conclusion:
  `conformant`/`divergent`/`harness_error`). Two aggregation policies sit on
  top of that single algorithm, and read fixture cases the same way but
  decide success differently — see "Two aggregation policies" below:
  `runConformanceCheck` (public; what the CLI's exit code is derived from)
  and `runCorpus` (self-check; used only by the regression test).
- [`fixtures/conformance-harness.fixtures.json`](fixtures/conformance-harness.fixtures.json)
  — the controlled corpus: synthetic `node -e '...'` pairs prove the
  mechanism, `real-node-rust-source-format-adapters` runs the real producers
  over [`fixtures/source-format-corpus.json`](fixtures/source-format-corpus.json),
  and `real-node-rust-rule-resolution` runs the real resolvers over
  [`fixtures/rule-resolution-corpus.json`](fixtures/rule-resolution-corpus.json).
- [`source-format-node-producer.mjs`](source-format-node-producer.mjs) and the
  Rust example `meridian-app/examples/source_format_producer.rs`, plus
  [`rule-resolution-node-producer.mjs`](rule-resolution-node-producer.mjs) and
  `meridian-app/examples/rule_resolution_producer.rs`, expose only
  deterministic verification output; none is a future CLI command.

## Producer spec

```text
{ command: string, args: string[], cwd: string, env: Record<string, string> }
```

`command`/`args` are passed to `child_process.spawnSync` with `shell: false`
— never a shell string. `env` is used verbatim as the child's entire
environment; nothing here inherits the caller's ambient environment or needs
network access. A spawn error, a signal, a non-numeric exit status, or an
invalid spec is returned as an explicit `{ ok: false, reason, detail }`
refusal from `runProducer` — never mistaken for a producer's own run.

## Diagnostic line convention

A line of the form `FAIL  <message>` or `WARN  <message>` (level, exactly two
spaces, non-empty message) is a diagnostic — the same convention already used
by `scripts/kernel-validate.mjs`'s `fail`/`warn` helpers. Both stdout and
stderr are scanned the same way, on both sides: this package makes no claim
yet about which stream a future real producer will put diagnostics on, so
neither stream is preferred or ignored. A line that starts with `FAIL` or
`WARN` but does not match the strict two-space form is recorded as
`unparseable` and turns the whole case into an explicit `harness_error` —
never silently dropped, and never allowed to produce a false `conformant`.

## Normalization — what is allowed and what is not

Allowed, and only this:

- unifying CRLF/LF before splitting into lines;
- separating the stable `FAIL`/`WARN` level from the message text;
- comparing FAIL and comparing WARN as **order-independent multisets** —
  duplicates are counted, never deduplicated, so a diagnostic repeated on one
  side only is still detected.

Never done, by construction of `compareVerdicts`:

- FAIL and WARN are never merged into one bucket;
- message text is never discarded, only split from its level;
- a message present on one side and absent on the other always appears in
  `missing`/`added` — there is no code path that drops it;
- exit codes are compared for equality, never mapped onto one another;
- a diagnostic-looking line that fails to parse never falls through silently
  — see above;
- there is no fuzzy/edit-distance matching between different message texts.
  A same-text-different-level diagnostic (e.g. `FAIL` on one side, `WARN` on
  the other) is not collapsed into a third "changed" bucket: it shows up as a
  `missing` entry in one level's diff and an `added` entry in the other's.
  Reporting it this way, rather than through fuzzy same-text matching, is a
  deliberate choice — fuzzy matching between texts is exactly the kind of
  normalization that could hide a real difference in wording.

## Result shape

```text
runCase(caseDef) → {
  status: "conformant" | "divergent" | "harness_error",
  ...
}
```

`harness_error` is returned when the mechanism itself could not reach a
trustworthy verdict for at least one side (spawn failure, invalid producer
config, or an unparseable diagnostic-looking line) — it is never reported as
`conformant`. `divergent` carries the full `compareVerdicts` result: exit-code
match, and the missing/added FAIL and WARN entries with left/right
occurrence counts.

## Two aggregation policies on top of one `runCase`

A fixtures file's cases can be read two different ways, and the module keeps
them as two separate functions so neither can be mistaken for the other:

- **`runConformanceCheck(fixturePath)` — public, drives the CLI's exit
  code.** Runs every case through `runCase` and reports `conformant: true`
  only when **every** case's *actual* status is `conformant`. It never reads
  a case's `expected_status` field — a real `divergent` or `harness_error`
  cannot be turned into success by anything a fixture claims it expected.
  This is the function a future direct single-pair caller (package 4+) would
  use the same way.
- **`runCorpus(fixturePath)` — self-check only, used by
  `test/conformance-harness.test.mjs`.** Runs every case through the same
  `runCase` and reports whether the *actual* status matches that case's own
  declared `expected_status`. This is how the controlled corpus proves the
  mechanism actually reaches `divergent`/`harness_error` on its adversarial
  cases, and it is never what the CLI runs.

Both call the exact same `runProducer`/`normalizeDiagnostics`/
`compareVerdicts`/`runCase` — the difference is only which aggregation
question is asked of the same per-case result, never a second comparison
algorithm.

## Controlled corpus

`fixtures/conformance-harness.fixtures.json` covers, each case run through the
same `runCase` path:

| Case | Expected status | Proves |
|---|---|---|
| `identical-exit-and-diagnostics` | `conformant` | Same exit code, same FAIL/WARN — a match is possible at all. |
| `same-diagnostics-different-order` | `conformant` | Diagnostic order never creates a false divergence. |
| `different-exit-code` | `divergent` | A differing exit code alone is detected. |
| `missing-fail-on-right` | `divergent` | A dropped FAIL is detected. |
| `extra-warn-on-right` | `divergent` | An added WARN is detected. |
| `same-text-different-level` | `divergent` | A FAIL/WARN severity change on identical text is detected in both level buckets. |
| `diagnostic-text-differs` | `divergent` | A wording change inside a diagnostic of the same level is detected. |
| `duplicate-diagnostic-only-left` | `divergent` | A duplicate lost on one side is detected — multiset diff, not a `Set`. |
| `spawn-error-on-right` | `harness_error` | A producer that fails to spawn is an explicit refusal, never a coincidental match. |
| `intentional-divergence-proof` | `divergent` | An unmistakable mismatch is never reported as anything but divergent — the mechanism does not stay silent on any input. |
| `unparseable-diagnostic-like-line` | `harness_error` | A diagnostic-looking-but-unparseable line common to both sides is refused, not silently ignored into a false match. |
| `real-node-rust-source-format-adapters` | `conformant` | The real Node.js and Rust source-format adapters agree on the shared adversarial corpus. |
| `real-node-rust-rule-resolution` | `conformant` | The real Node.js resolver and Rust application adapter agree on the shared resolution corpus. |

Run the CLI directly against a fixtures file (one case or many — "every
requested comparison" below means every case the given file contains):

```bash
node verification/conformance-harness/conformance-harness.mjs \
  verification/conformance-harness/fixtures/conformance-harness.fixtures.json
```

**Exit-code contract of the CLI (`runConformanceCheck`, not `runCorpus`):**

- **`0`** — every requested comparison's *actual* status came back
  `conformant`, and the harness itself ran without error.
- **non-zero** — at least one requested comparison came back `divergent`.
- **non-zero** — at least one requested comparison came back
  `harness_error` (a producer failed to spawn, gave no numeric exit code, or
  emitted an unparseable diagnostic-looking line).

A fixture's own `expected_status` field is **never** read by the CLI — it
exists solely for the self-check regression test (`runCorpus`, see above).
Running the CLI against the full controlled corpus above will therefore
always exit non-zero: that corpus deliberately contains real `divergent` and
`harness_error` cases, on purpose, to prove the mechanism detects them — the
CLI reports them honestly rather than treating a corpus case's own
`expected_status: "divergent"` as if it made the process succeed. Run it
against a single-case fixtures file (one `left`/`right` pair) to see the
`0`/non-zero contract for one real comparison.

Output is deterministic across repeated runs on the same input:
`compareVerdicts` always sorts its diff entries, and the synthetic producers
carry no timestamp, randomness or environment dependency.

## One implementation, three call sites

`test/conformance-harness.test.mjs` exercises `runProducer`,
`normalizeDiagnostics`, `compareVerdicts`, `runCase`, `runConformanceCheck`
and `runCorpus` directly, and also spawns `conformance-harness.mjs` itself as
a real child process to assert its actual exit code (never merely an
in-memory `.match`/`.status` field). `hooks/pre-push` and
`.github/workflows/gate.yml` both run that same test file — there is no
second, independently written comparison or aggregation algorithm living in
any of the three.

## Real-producer reuse

Package 4 wires real Node.js and Rust commands into a producer specification
without changing `runProducer`, `normalizeDiagnostics`, `compareVerdicts`,
`runCase` or `runConformanceCheck`. Later packages can add fixture cases or an
equivalent caller without adding a second comparison algorithm. The
`"$NODE"`/relative-`cwd`
fixture convenience lives only in the shared fixture-loading helper used by
`runConformanceCheck` and `runCorpus`, not in the reusable core, precisely so
it never has to be un-taught later.
