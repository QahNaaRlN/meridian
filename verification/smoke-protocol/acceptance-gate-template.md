# Acceptance gate — template

A product smoke unit's verdicts are not trusted until the unit has
demonstrated, on a pinned candidate snapshot, that every §11 outcome is
reachable and none can be faked. Instantiate this file inside the product
smoke unit and fill the Status/Evidence columns only from executed runs.

## Experiments

| ID | Scenario | Required outcome | Status | Evidence |
|---|---|---|---|---|
| AE-1 | The change under test is absent from the selected deployment; the target action cannot be validly verified. | `BLOCKED` | not-run | — |
| AE-2 | The target action is accepted by the system, but the predicate is not evaluable. | `INCONCLUSIVE` | not-run | — |
| AE-3 | The primary predicate is confirmed; an adjacent behavior is broken. | `PASS + REGRESSION` | not-run | — |
| AE-4 | The primary predicate is true and the required evidence is collected in full. | `PASS` | not-run | — |
| AE-5 | The UI looks correct, but the required evidence is unavailable. | `INCONCLUSIVE`, never `PASS` | not-run | — |

AE-4 is the mandatory positive control: a workflow that always returns
`BLOCKED` does not pass this gate.

## Release gate

- [ ] AE-1…AE-5 executed on a pinned candidate snapshot.
- [ ] Test Contract, Execution Context and raw evidence preserved per
      experiment.
- [ ] Each verdict independently reproducible from §11.
- [ ] The unit's `CHANGELOG.md` confirmed by a normative diff.
- [ ] Derived metadata matches the unit's `VERSION`.
- [ ] No unexplained changes and no unresolved required configuration.

Any `FAIL`, `not-run`, missing evidence or mismatch blocks release of the
smoke unit.
