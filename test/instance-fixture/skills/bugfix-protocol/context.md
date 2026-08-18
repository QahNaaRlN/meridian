# Product context for bugfix-protocol (fixture)

Synthetic Instance data for the fictional fixture product. Its purpose is to
prove that the derived Kernel protocol is applicable when an Instance supplies
the required context — not to describe any real product's conventions.

- `fetchState`, not `loadState` — fictional FetchKit v3
- `reportFault` — the only error channel
- `NotifyBus` — user-facing messages go through it only
- `makeTestHarness({ stubEffects: false })` — for tests touching stores
- regression tests sit next to the source file (`*.test.mjs`), not in a
  separate tree
