# Instance bootstrap

First-day path for wiring the Kernel to a new product. Copy this directory,
fill in facts, and the same methodology that served the previous product runs
against the new one — with zero edits to Kernel files. If a Kernel edit turns
out to be required, that is a Kernel defect to report, not a local workaround
to make.

## Steps

1. Copy `instance-template/` to a new private repository OUTSIDE the Kernel
   (the Instance lives where its data belongs — usually the customer's
   infrastructure) and run `git init` there.
2. Fill `product.yaml`. Every `REPLACE_ME` is a deliberate blocker: the
   validator and the workflows treat an unresolved required value as a stop,
   never as something to guess. Populate `forbidden_literals` (and
   `forbidden_patterns` for names too common to match loosely) generously —
   the purity gate can only catch what the Instance declares.
3. Fill `skills/bugfix-protocol/context.md` with the product's conventions.
   The derived protocol declares a missing context file a blocker.
4. Set `MERIDIAN_INSTANCE` to the new Instance root and confirm the wiring:

   ```bash
   node scripts/preflight.mjs
   node scripts/kernel-validate.mjs
   ```

   The validator must be green before the first task, with one expected
   nuance: registries are empty, so nothing is confirmed yet — that is
   reported honestly, not hidden.
5. Populate registries as facts become known, never ahead of evidence:
   `inventory/repositories.yaml` (repository facts with Git evidence),
   `commands/repositories.yaml` (source-cited commands),
   `environments/*.yaml` (topology, access references, test data — external
   references only, never secrets).
6. Working memory lives under `.agent/` per the Kernel's
   `standards/workspace/agent-memory.md`. `.agent/feedback/friction-log.md`
   and `.agent/metrics/` are pre-seeded — see
   `standards/workspace/feedback-and-metrics.md` in the Kernel for what goes
   in them and when.

## What this template is not

It is not a product example: every value is a placeholder, and the fictional
CI fixture (`test/instance-fixture/`) exists separately so that this template
never accumulates product-shaped content.
