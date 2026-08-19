---
title: Smoke protocol (portable)
document_type: readme
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-08-18
updated: 2026-08-19
---

# Smoke protocol (portable)

The product-agnostic core of runtime/browser smoke verification: what a smoke
run must fix before acting (Test Contract), what it records (Execution
Context), how a verdict is computed (Single Verdict Authority), and what a
new product needs to instantiate its own smoke unit. This closes the gap
where the whole smoke methodology lived only inside one product's release
unit and could not travel with the Kernel.

## Contents

- [`smoke-protocol.md`](smoke-protocol.md) — the portable protocol: contract, execution
  context, verdict matrix, side-effect tiers, test-data rules, evidence and
  stop conditions.
- [`acceptance-gate-template.md`](acceptance-gate-template.md) — the
  acceptance-experiment pattern (AE-1…AE-5) a product smoke unit must pass
  before its verdicts are trusted.

## Division of labour

The protocol is Kernel; everything a run needs to know about a *product* is
Instance data inside that product's smoke unit: environments, applications,
flows, access references, capabilities, test-data references, and the unit's
own `VERSION`/`CHANGELOG`. A product smoke unit is a separate release unit
that *conforms to* this protocol; this package never absorbs its files, and
an existing product unit is never split retroactively.

To bootstrap a smoke unit for a new product: create
`$MERIDIAN_INSTANCE/verification/smoke-testing/` with its own `VERSION`,
`CHANGELOG.md`, an `ACCEPTANCE-GATE.md` instantiated from the template here,
the product's `environments.yaml`, `access.yaml`, `capabilities.yaml`,
`test-data.yaml`, per-application flow definitions, and a workflow document
that binds this protocol to the product's applications. Unresolved values are
`REPLACE_ME` blockers, never guesses.

## Provenance

Derived on 2026-08-18 from the current Instance's smoke unit at its version
`0.3.0` — the methodology was extracted; the product unit itself was left
untouched. Upstream file digests at extraction time:

```text
9255c3a23a19e6b375908ee2cc8c1e2226ac2921342721956907dab19143fedd  workflows/smoke-test.md
40cc7b05b745dbd90aaf898ef5989ffdab054575aaf3ebea92d1798015d22797  ACCEPTANCE-GATE.md
38c8a8d70898757e8f07aa4cc35d7860ba1d995befcff5ca4f44c9e869a1aace  policies/side-effects.md
dfc6faeddc43744785740d155be9c3664117781bacfd445caf213346cc8813e8  policies/test-data.md
```

Transformations: product and workspace references removed; tool-specific
phrasing generalized (a named CI host became "the CI/hosting pipeline");
domain-shaped side-effect examples generalized; the two policies folded into
the protocol as sections rather than separate release files. The derived text
is not presented as a verbatim copy.

This package versions with the Kernel release unit; it has no separate
`VERSION`.
