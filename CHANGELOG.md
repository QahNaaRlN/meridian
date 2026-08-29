---
title: Changelog
document_type: changelog
status: maintained
scope: workspace
owner: workspace-owner
created: 2026-08-18
updated: 2026-08-29
---

# Changelog

Формат — [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
версионирование — [SemVer](https://semver.org/spec/v2.0.0.html).
Линия версий Kernel независима от Instance и от продуктовых репозиториев.

## [Unreleased]

### Added

- **`verification/functional-parity/refactor-protocol.md` — the canonical
  REFACTOR execution protocol (PHASE E of `MERIDIAN-RULE-RESOLUTION-001`).**
  A `document_type: protocol` Kernel document in the functional-parity
  verification unit, beside the PHASE D evidence contract it executes against.
  It designs the **order** a `REFACTOR` is carried out in —
  `CLASSIFY → DEFINE PARITY → CAPTURE BASELINE → IMPLEMENT → CAPTURE POST-CHANGE
  → COMPARE → REPORT` — and nothing else: it references the PHASE D contract as
  already-defined at the capture and compare steps and introduces no second
  evidence contract, no new evidence kind, and no new record field. It resolves
  open decision §8.3 of the normative model (phase names and structure) in
  favour of a structure oriented on the evidence contract rather than a
  re-use of the bugfix phases.
  - All four preserved-contract facets (`public_api`, `observable_io`,
    `side_effects_and_interactions`, `user_visible_behavior`) are addressed at
    DEFINE PARITY; per-assertion evidence selection from the contract's §5 set,
    with no kind — snapshot included — mandatory, primary or default; honest
    per-assertion `UNVERIFIED` recorded at COMPARE, with a record-scoped gap or
    an unestablished baseline forcing every assertion and the overall
    `UNVERIFIED`.
  - The safe invariant for a defect found inside a `REFACTOR` (normative model
    §1.4.1) is step COMPARE: the difference is not normalised into the baseline
    or a snapshot, not fixed inside the `REFACTOR` work item; a separate
    `BUGFIX` work item is created or the owner decides; the original `REFACTOR`
    keeps its class and its protocol route and is paused only if the defect
    blocks the parity proof. `git worktree` is named as one possible practice,
    not a step. A file set that widens beyond the resolved `candidate_paths`
    re-resolves before VERIFY.
  - Linked from `verification/README.md` (strategy router and evidence
    boundary), the VERIFY stage and authority invariants of
    `workflows/task-lifecycle.md`, `verification/functional-parity/README.md`,
    `standards/workspace/rule-resolution.md` (§3.1, §3.2, §7, §10) and
    `functional-parity-evidence-contract.md` (intro and §12).
  - `VERSION` unchanged: this package is not a release.

### Changed

- **`scripts/rule-resolver.mjs` — `REFACTOR` now routes its own protocol with
  provenance instead of returning `unresolved_applicability`.** The PHASE D
  evidence contract and the PHASE E execution protocol exist, so a `REFACTOR`
  work item is routed through `routeProtocols()` like every other class and its
  `applicable_protocols` entry carries `source`, `scope` and `digest`/`revision`
  (`rule-resolution.md` §7). The stale `subject: "REFACTOR"` /
  "does not exist yet (PHASE D) … not designed yet (PHASE E)" entry is removed
  from the resolver and its tests. The defect-in-`REFACTOR` invariant is
  unchanged: with a `prior_state.refactor_findings` entry the resolver still
  emits `refactor-in-progress-finding` in `unresolved_applicability` — now
  **alongside** the resolved protocol route, not instead of it — and the
  `REFACTOR` class is never reclassified.
  - `test/fixtures/rule-resolver.fixtures.json` — a Kernel-universal
    `REFACTOR → refactor-protocol` route added to `protocol_routes`.
  - `test/rule-resolver.test.mjs` — acceptance case 5 renamed and rewritten to
    assert the provenance-carrying route and the absence of the `REFACTOR`
    unresolved entry; case 6 rewritten to assert the finding entry sits beside
    the parent's `refactor-protocol` route.
  - `resolver-output.schema.json` unchanged — `routed_from` already admitted
    `REFACTOR`.
- **`standards/workspace/rule-resolution.md` §3.1, §3.2, §7, §10 — the "PHASE
  D/E do not exist yet" language is removed.** §3.1 and §10 now state that both
  artefacts exist and are authoritative and that the resolver routes `REFACTOR`
  to the protocol with provenance; §3.2 points at the protocol's COMPARE step
  for the safe invariant and records that the resolver returns the route and the
  finding side by side; §7 lists `REFACTOR → refactor-protocol` as a
  Kernel-universal route example. No new evidence contract is introduced and the
  PHASE D contract is not weakened.
- **`skills/bugfix-protocol/SKILL.md` step 0 — `REFACTOR` added as a fourth,
  BUGFIX-incompatible change class.** The classifier now names all four classes
  and routes `REFACTOR` out to
  `$MERIDIAN_KERNEL/verification/functional-parity/refactor-protocol.md`, with a
  note that a defect found during a `REFACTOR` is not fixed under this skill.
  `skills/bugfix-protocol/PIN.yaml` — `sha256` re-pinned
  (`269930b0…` → `67f55d7d…`), `pinned_at` set to `2026-08-29`, and a
  `transformations` entry recording the Kernel-side step-0 extension;
  `upstream.sha256` is untouched.
  contract for `REFACTOR` (PHASE D of `MERIDIAN-RULE-RESOLUTION-001`).** A new
  verification unit beside `regression-testing/` and `smoke-protocol/` in the
  verification router, answering one question: how the observable behaviour of
  the system before an internal change is fixed and compared with the behaviour
  after it.
  - `functional-parity-evidence-contract.md` (`document_type: standard`) — the
    normative contract. The preserved observable contract across four facets
    (`public_api`, `observable_io`, `side_effects_and_interactions`,
    `user_visible_behavior`), with assertion ids unique across the whole record
    and each under one owning facet; the baseline before the change (identifiable
    source state, identified inputs and observation conditions, provenance, the
    result actually observed); the post-change evidence (state after the change;
    conditions that are either `same` — referencing every baseline condition id
    and no others — or `explicitly-comparable` with a justification; the observed
    result; contract links that resolve to a declared assertion's exact
    `{facet, id}` pair); six neutral evidence kinds with the limitations of each;
    the public-contract snapshot as **one** kind among them — permitted only with
    an explicit applicability justification, never the primary or default proof,
    its limits named; explicit gaps and `UNVERIFIED` — an incomplete baseline,
    non-comparable conditions, a missing facet, incomplete post-change evidence
    or an unjustified kind leave the affected assertion, or the whole claim,
    `UNVERIFIED`, a narrow kind is not widened into a broader claim, a
    per-assertion `VERIFIED` needs both a covering evidence entry and a resolving
    link, and an unestablished baseline forces every assertion and the overall
    `UNVERIFIED`; the product/repository boundary — concrete test runners,
    snapshot frameworks, commands, configuration and test paths stay
    Instance/repository data; and the per-assertion and overall verdict rules.
    The contract defines requirements and form; it does not design the order in
    which a `REFACTOR` is executed (that is PHASE E).
  - `functional-parity-evidence.schema.json` — JSON Schema for one evidence
    record, using only keyword and format subsets `scripts/kernel-validate.mjs`
    implements. `records` is non-empty (`minItems: 1`). Every facet must be
    addressed (an assertion or a stated not-applicable justification, never a
    blank); each baseline condition carries a stable `id`; `relationship: same`
    carries `baseline_condition_ids` and neither restated `items` nor a
    justification, `explicitly-comparable` carries `items` and a comparability
    justification and no id references; `contract_links` is always present and
    may be empty; every evidence entry names at least one limitation; a
    `public-contract-snapshot` entry requires an applicability justification; an
    `UNVERIFIED` per-assertion state or overall verdict requires a reason;
    `additionalProperties: false` throughout, so an unrecognised form extension
    fails closed. No `kind` value and no required field names a product tool.
  - `fixtures/functional-parity-evidence.fixtures.json` — product-neutral valid
    and invalid fixtures (no real repository, path, URL, runner, framework or
    command): 8 valid, 29 invalid. Valid records show a snapshot is not required
    where other kinds cover the claim, that different single kinds each form a
    valid record, that `explicitly-comparable` conditions with a justification
    are accepted, a mixed record with one assertion `VERIFIED`-with-link and one
    `UNVERIFIED`-without-link, that an incomplete or unestablished baseline
    yields `UNVERIFIED` for every assertion, and an honest all-`UNVERIFIED`
    record with an empty `contract_links` array. Invalid records cover an empty
    record set, an absent baseline, absent or non-comparable post-change
    evidence, a `same` relation that carries `items` / references a non-baseline
    condition / reproduces only some conditions, a duplicate baseline condition
    id, a blank facet, a snapshot with no applicability justification, an empty
    limitations list, an unknown property, an empty observation, a missing
    verdict reason, and — schema-clean but rejected by the inference rules — a
    record-scoped gap under a `VERIFIED` overall, a record-scoped gap that leaves
    an assertion `VERIFIED`, an `UNVERIFIED` assertion under a `VERIFIED`
    overall, evidence covering an undeclared assertion, a declared assertion with
    no verdict, a `VERIFIED` assertion no evidence covers, a `VERIFIED` assertion
    with no resolving link, a contract link under the wrong facet, a duplicate
    assertion id across facets, an unestablished baseline that still carries a
    `VERIFIED` assertion, and identical baseline / post-change source states.
  - `scripts/kernel-validate.mjs` — new check `functional-parity` (`6c` in the
    header). The schema is parsed and walked for unsupported keywords; the
    fixtures bundle is classified fail-closed on its own shape (a non-empty
    array, exactly one group for the schema, non-empty `valid` and `invalid`),
    exactly as the `rule-resolution` block is. Beyond the schema,
    `functionalParityConsistency()` enforces the record-level inference rules
    the draft-07 subset cannot state: the document carries at least one record;
    assertion ids unique record-wide, each under one owning facet; every
    per-assertion verdict names a declared assertion and every declared assertion
    carries exactly one; every `covers` id resolves, and every `contract_links`
    entry resolves to the exact `{facet, assertion_id}` pair; a `VERIFIED`
    assertion has both a covering evidence entry and a resolving link (an empty
    `contract_links` array is legal only when nothing is `VERIFIED`); any
    `UNVERIFIED` per-assertion state forces an `UNVERIFIED` overall; a
    record-scoped gap forces every declared per-assertion verdict and the overall
    `UNVERIFIED`, an assertion-scoped gap only the assertions it names;
    `relationship: same` references every baseline condition id and no others
    with no duplicate baseline condition id; an unestablished baseline forces
    every declared per-assertion verdict and the overall `UNVERIFIED`; the
    baseline and post-change source states are a distinguishable
    `{identifier_kind, identifier}` pair. `kernel-validate` remains the gate;
    this is not a new script.
  - `test/kernel-validate.test.mjs` — new adversarial cases (t126–t158) proving
    the check goes red on a malformed schema, missing fixtures, a misclassified
    fixture, an unsupported schema keyword, each fail-closed bundle-shape
    violation, and each negative case of the contract (an empty record set, a
    missing baseline, missing or non-comparable post-change evidence, a
    mechanically-broken `same` relation, a duplicate baseline condition id, an
    unclosed gap left `VERIFIED`, a record-scoped gap that still leaves an
    assertion `VERIFIED`, identical before/after source states, a snapshot with
    no applicability justification, a `VERIFIED` assertion with no resolving
    link, a contract link under the wrong facet, a duplicate assertion id, an
    unestablished baseline still carrying a `VERIFIED` assertion, an unknown form
    extension), plus green cases proving a single non-snapshot kind, an
    `explicitly-comparable`-with-justification record, the mixed
    `VERIFIED`/`UNVERIFIED` record, the honest all-`UNVERIFIED` unestablished
    baseline, an honest all-`UNVERIFIED` record with an empty `contract_links`
    array, and the same identifier string under a different `identifier_kind` are
    accepted. Existing PHASE B / PHASE C checks are unchanged and still pass.
  - Linked from `verification/README.md` (strategy router and evidence
    boundary), the VERIFY stage of `workflows/task-lifecycle.md`, the Kernel
    file list in `standards/workspace/kernel-boundary.md`, and the structure
    tree in `README.md`.
  - `VERSION` unchanged: this package is not a release.

### Fixed

- **`hooks/pre-push` now binds its gates to the Kernel being pushed instead of
  inheriting the caller's Git environment.** Two problems in the pre-push hook,
  both of which made the gates read the wrong tree:
  - Git runs the hook with `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE` and the
    rest of `git rev-parse --local-env-vars` exported. The regression suites
    build throwaway synthetic repositories and run `git init/add/commit` inside
    them; those child `git` invocations were inheriting the caller's variables
    and operating on the repository being pushed — a synthetic test had
    committed straight onto a Kernel branch that way, and an ordinary push was
    blocked as a side effect. The hook now computes `ROOT` while the Git
    environment is still intact and then sources the new
    `hooks/lib/git-env-isolate.sh`, which unsets every repository-local Git
    variable (the list comes from `git rev-parse --local-env-vars`, not a
    hard-coded copy) before any gate runs. The helper is fail-closed: it checks
    the exit status of `git rev-parse --local-env-vars` explicitly instead of
    swallowing a failure into a successful empty list, so on any non-zero exit
    it prints a diagnostic and returns non-zero and the hook — under `set -e` —
    exits before a single gate is started. The helper does not touch
    `MERIDIAN_*` or `PATH`.
  - `scripts/kernel-validate.mjs` and `scripts/rule-resolver.mjs` prefer
    `$MERIDIAN_KERNEL` over their own location, so an ambient `MERIDIAN_KERNEL`
    left in the operator's shell (a different, possibly dirty checkout) made the
    gates validate that other tree and falsely block the push. After the scrub
    the hook now pins `export MERIDIAN_KERNEL="$ROOT"` — the worktree Git is
    pushing from — so every gate sees the pushed Kernel. `MERIDIAN_INSTANCE` is
    left as the owner set it: the fixture gate still overrides it inline to
    `$ROOT/test/instance-fixture`, and the optional logged real-Instance run
    still uses the value passed in. No `--no-verify` is needed.

  New regression test `test/pre-push-git-isolation.test.mjs` — wired into
  `hooks/pre-push` and `.github/workflows/gate.yml` — runs the verbatim
  production hook and helper against a synthetic Kernel repo with gate shims
  that record what each gate received, and drives a normal `git push` to a
  local bare remote with no `--no-verify`. It proves the Git environment is
  scrubbed before the first gate and that every gate sees `MERIDIAN_KERNEL`
  equal to the pushed root even when a stale ambient value is set; deleting the
  isolation sourcing line or the `MERIDIAN_KERNEL` pin makes the push fail
  closed and the test red; a failing `git rev-parse --local-env-vars` fails the
  hook with no gate started; and the caller and real Kernel HEADs are unchanged
  throughout. Nothing touches the network or any real repository.

### Added

- **Deterministic rule resolver (PHASE C of `MERIDIAN-RULE-RESOLUTION-001`).**
  `scripts/rule-resolver.mjs` — a read-only Kernel mechanic beside
  `kernel-validate.mjs`, not an extension of it; the validator stays the gate.
  A pure core `resolveRules(workItem, sources)` plus a CLI wrapper: the work
  item is exactly the §3.1 shape of the normative model, `sources` are
  explicitly injected environmental data (repository inventory, PHASE B
  applicability records, instruction-intake registers, and the not-yet-
  standardised protocol/verification route records), and the same input on the
  same source revision returns a byte-identical, identically ordered object
  that satisfies `registries/rule-resolution/resolver-output.schema.json`.
  Axes are checked separately and conjunctively; a technology profile never
  yields an architecture profile; `architecture_profile` absence is not by
  itself unresolved; a `deferred` intake verdict does not disable a norm.
  Path-glob: every separate `**` is zero or more whole path segments, several
  and adjacent `**` are supported (`**/**/x`, `a/**/**/b`) and collapse to one,
  and any mask outside the declared grammar is refused, never approximated.
  Protocol routes are filtered against the work context — the injected route
  shape carries its own exact `repository` / `product_domain` selectors — before
  any conflict is computed, so a repository route for one repository never
  reaches another and a local/universal disagreement that survives scope
  filtering is a `conflicts` entry, not a silent override. Provenance is
  verified for every applicability record that matches the work item —
  `status: resolved`, `status: unresolved` and `activation: undetermined`
  alike: each must have a resolvable exact intake pointer and a supplied norm
  text/region whose SHA-256 equals the recorded digest; records irrelevant on
  scope/path/task-class need no text. Append-only precedence: the authoritative
  latest applicability record decides one norm identity — a newer `unresolved`
  suppresses an older `resolved` and a newer `resolved` suppresses an older
  `unresolved`; records that cannot be ordered within the available
  identity/date are fail-closed, never ordered by JSON lexical order. A
  container norm's region is recomputed through the shared marked-region reader:
  markers are located in the fenced-blanked buffer (a marker quoted in a fenced
  example is not a declaration), but the text handed to the digest is the
  region's verbatim source slice — `instructionRegions().regions[].sourceText`,
  every character of the region between its markers intact, fenced code
  included — never the space-blanked parser view; a named region that is
  missing, duplicated or unclosed is fail-closed, never a fallback to the whole
  file. The CLI's `--applicability` input is the whole PHASE B register
  envelope: it is validated against
  `registries/rule-resolution/applicability.schema.json` with the shared engine
  before records are touched, so a bare `{}`, a missing `records`, a wrong
  `schema_version`, an additional property, a non-array `records` or a
  top-level raw array is fail-closed with exit 2 — `records` is never coerced
  to `[]`. The pure core still takes an injected `applicability_records` array.
  The work item is validated strictly against §3.1:
  `candidate_paths` and `changed_paths` are mandatory string arrays and a
  missing or non-array value is not coerced to `[]`, an unknown work-item field
  and a malformed `declared_profiles` are rejected. Repository ids, intake
  pointers and route keys match exactly, with no fuzzy/path/basename fallback;
  `changed_paths` that widen the prior `candidate_paths` set
  `requires_reresolution`; `REFACTOR` gets `unresolved_applicability`, never
  another class's protocol, until PHASE D/E; a defect found inside a `REFACTOR`
  needs a separate `BUGFIX` child or an owner decision and does not reclassify
  the original work; an undecomposed `initiative` returns
  `decomposition_required`, the declared decomposition protocol or `null`,
  pre-decomposition norms and separate `unresolved_items`; a reviewer
  assignment is not an input and does not change the result.

- **Shared pure helpers under `scripts/lib/`.** The YAML subset reader
  (`scripts/lib/yaml.mjs`), the JSON Schema subset engine
  (`scripts/lib/json-schema.mjs`) and the marked-region reader
  (`scripts/lib/regions.mjs` — `blankFencedBlocks`, `markedRegion`,
  `instructionRegions`) moved verbatim out of `kernel-validate.mjs`, which now
  imports them, so the validator and the resolver read YAML, validate against
  JSON Schema and parse marked regions through one implementation each rather
  than a second copy. The documented subsets, the supported keyword set, the
  format checks, the region parsing rules and every throw are unchanged; the
  validator's regression suite is unaffected. `instructionRegions()` additionally
  exposes, per region, a `sourceText` (the verbatim slice of the original input
  between the markers) beside the existing `text` (the fenced-blanked parser
  view) — an additive field; the validator keeps reading `text`.

- **Regression coverage for the resolver.** `test/rule-resolver.test.mjs` and
  the product-neutral `test/fixtures/rule-resolver.fixtures.json`: one named
  test per acceptance case of §4/§9 of the normative model (cases 9 and 10 are
  separate tests), plus a genuine `status: unresolved` case surfaced under its
  own resume condition; append-only precedence in both directions and its
  unorderable-records fail-closed; cross-repository and product-domain protocol
  route isolation; missing/duplicate/unclosed region, a fenced-example marker,
  and a region whose `sourceText` keeps fenced code verbatim and hashes to the
  same SHA-256 as the raw slice between its markers; a valid `--applicability`
  envelope resolving via the CLI and malformed envelopes (no `records`, bare
  `{}`, wrong `schema_version`, non-array `records`, an extra property, a raw
  array) each fail-closed with exit 2;
  missing/non-array/non-string `candidate_paths` and `changed_paths`;
  negative tests for a stale digest, a missing text and an unresolvable intake
  pointer on `unresolved` and `undetermined` records as well as `resolved`
  ones; the new several/adjacent `**` glob cases; and the pre-existing
  fail-closed paths (unknown repository id, `work_kind`/`change_class` pairing,
  malformed applicability record, unsupported glob, incompatible mandatory
  routes, missing mandatory source), stable ordering and output-schema
  conformance. Wired into `.github/workflows/gate.yml` and `hooks/pre-push`;
  `README.md` documents the CLI and the test command.
  `instance-template/hooks/pre-push` is left untouched — it checks published
  Instance state, not Kernel unit tests. `VERSION` unchanged; no release, no
  tag.

## [0.4.0] — 2026-08-27 (`draft`)

### Added

- **Repository reference mechanism — каноническая идентичность и display alias
  разведены.** Единственная каноническая идентичность продуктового репозитория
  — `inventory/repositories.yaml` → `repositories[].id`; display alias adapter
  (`folders[].name` в файле Cursor workspace) идентичностью не является и
  repository scope не создаёт. Новый стандарт
  `standards/workspace/repository-references.md` и схема
  `registries/inventory/repository-references.schema.json` вводят
  типизированную reference-запись (`kind` / `alias` / `repository_id` /
  `sources`) и fail-closed exact resolution: сначала точное совпадение с
  каноническим `id`, иначе точное case-sensitive совпадение с объявленным
  alias — ровно в один `repository_id`; ноль совпадений —
  `unresolved_repository_reference` / STOP, несколько разных — `conflict` /
  STOP; alias выходной идентичностью не становится. Без fuzzy-, remote-,
  path- и basename-подбора. Instance хранит конкретные aliases в
  `inventory/repository-references.yaml`; `instance-template/` несёт пустой
  реестр. `VERSION` не менялся (`0.3.0`), релиз не выполнялся.

- **Режим продвижения — свойство репозитория, а не каждой вложенной release
  unit.** `version-control-flow.md` объявлял себя применимым к Kernel, Instance
  и delivery adapters, но требовал для стабильной линии `release/<semver>`,
  `VERSION`, тег `vX.Y.Z` и SemVer-hotfix — а `release-versioning.md` §8
  одновременно фиксирует Instance как Git-revision без SemVer. Обе нормы
  Instance исполнить не мог. Correction разделяет две сущности: режим
  продвижения описывает **верхнеуровневую (repository-level) release identity
  репозитория**. Закрытый набор: `semver-release` (repository-level `VERSION` и
  repository-level тег `vX.Y.Z`; так объявлен Kernel — `master` / `develop`) и
  `revision-promotion` (repository-level состояние идентифицируется Git SHA
  итогового non-fast-forward advancement commit; repository-level `VERSION` и
  тег не создаются; так объявлен тип репозитория Instance до отдельного решения
  владельца о SemVer — конкретные имена линий и само объявление остаются в
  Instance).

  Вложенная независимо версионируемая единица (`stack-profiles/` в Kernel,
  smoke-пакет в Instance) разведена по **двум независимым осям**. Ось A
  (версионирование): режим репозитория не переопределяет её `VERSION`,
  `CHANGELOG`, SemVer и tag convention; изменение вложенного `VERSION` — не
  повышение repository-level `VERSION` и режим репозитория не выбирает. Ось B
  (Git-flow файлов): изменение вложенной единицы — обычный package change,
  идёт через `feature/<slug>` в интеграционную линию, а в стабильную линию его
  файлы попадают только следующим repository advancement своего режима
  (`release/<semver>` или `promotion/<slug>`); независимость номера единицы не
  разрешает прямую запись в стабильную линию и не создаёт третий способ
  продвижения репозитория; такое advancement не обязано повышать
  repository-level `VERSION` только из-за вложенного. Отдельного flow и нового
  tag namespace для вложенных единиц стандарт не проектирует.

  Общая часть модели сохранена: `feature/<slug>` от интеграционной линии и
  обратно, запрет прямой записи в стабильную линию, first-parent чтение,
  отдельный non-fast-forward advancement commit на каждое продвижение,
  неизменность опубликованной истории, `push` не подразумевается. Режим,
  стабильная и интеграционная линии объявляются в tracked репозиторий-локальном
  источнике governance/конфигурации (для Kernel — сам стандарт). Общее
  ожидаемое правило для типа репозитория (`revision-promotion` для Instance)
  **не заменяет** локальное объявление конкретного репозитория: каждый
  конкретный Instance обязан объявить режим и линии в своём tracked-источнике,
  до этого Git-интегратор fail-closed и не начинает release/promotion
  integration, а противоречащее общему правилу локальное объявление требует
  явного решения владельца. Fail-closed также при нескольких конфликтующих
  объявлениях и при конфликте объявления с repository-level evidence; имя
  `main`/`master`, исторические теги и `VERSION`/тег вложенной единицы
  repository-level evidence не являются.

  `VERSION` не менялся (`0.3.0`), релиз не создавался; оба стандарта остаются
  `maintained`.

- **Git-flow и release versioning приняты как два раздельных нормативных
  стандарта** (`status: maintained`) после независимого review Codex
  2026-08-27. `standards/workspace/version-control-flow.md` вводит роль
  стабильной линии (её имя объявляет репозиторий; для Kernel это `master`, для
  репозитория с веткой по умолчанию `main` — `main`) и интеграционную линию
  `develop`, ветки `feature/<slug>`, `release/<semver>` и `hotfix/<slug>` с
  объявленными точками ветвления и возврата, запрет прямых package-коммитов в
  стабильную линию, раздельные коммиты Kernel и Instance, запрет на
  переписывание опубликованной истории без решения владельца и на неявный
  `push`. Применение перспективное:
  отдельный раздел adoption/migration фиксирует, что история до точки принятия
  нарушением не объявляется, что Kernel сейчас в переходном состоянии (`master`
  с коммитами после `v0.3.0`, `develop` с дополнительными принятыми пакетами) и
  что переход к модели выполняет первый будущий `release`, а миграцию —
  Git-интегратор. `standards/workspace/release-versioning.md` описывает Kernel
  как самостоятельную SemVer release unit с источником версии `VERSION`,
  ведение `CHANGELOG.md` по Keep a Changelog, фиксацию незавершённой работы в
  `Unreleased`, повышение `VERSION` только в `release/<semver>` и переход к
  `1.0.0` только по решению владельца. Стабильная линия читается по first-parent
  history: каждый её шаг — принятый release или hotfix, слитый отдельным
  merge-коммитом (release advancement commit) без fast-forward; аннотированный
  тег `vX.Y.Z` несёт этот merge-коммит и обозначает целостный snapshot, а
  package-коммиты, вошедшие в выпуск, индивидуально не версионируются и своих
  тегов не получают. Правила выбора MAJOR/MINOR/PATCH однозначны. Hotfix
  допустим только для изменения PATCH-класса и повышает patch; изменение,
  требующее MINOR/MAJOR, готовится как `release` соответствующего номера, а не
  как hotfix. `stack-profiles/` и Instance остаются самостоятельными линиями,
  vendored-зависимости — привязанными к SHA.
  Обе темы (`version-control-flow`, `release-versioning`) уже были в пуле
  `instruction-topics`; теперь у них есть принятый нормативный текст.

- **README §6 и `COMPATIBILITY.md` больше не утверждают, что принятого профиля
  версионирования Git-артефактов нет** — оба ссылаются на два принятых
  `maintained`-стандарта; compatibility matrix не тронута. `AGENTS.md` §6
  фиксирует, что при назначенном внешнем исполнителе создание и переключение
  веток, staging, commit, merge и tag принадлежат Git-интегратору; он вправе
  временно
  проиндексировать поимённо ограниченный candidate package, чтобы прогнать
  гейты по новым файлам, а снимает его path-limited операцией
  (`git restore --staged -- <path>...`), не очищая остальной индекс. Такая
  индексация приёмкой не является; исполнитель Git-записей не выполняет.
  `VERSION` не изменён: этот пакет релизом не является.

- **Пул профилей стека — релизная единица `stack-profiles/`.**
  Собственные `VERSION` (0.1.0) и `CHANGELOG.md`, пул в двух половинах
  (`stack-profiles.yaml` — имена и предикаты по манифесту, `stack-profiles.md` —
  сигнатуры между маркерами), схема данных. Три профиля: `vue-spa`,
  `laravel-app`, `node-ts-service`.

  Состав подтверждён манифестами и корпусом норм четырёх репозиториев первой
  волны, а не перечнем найденных технологий. Кандидат `vue-monorepo` отложен до появления монорепозитория в корпусе:
  ни один манифест не объявляет `workspaces`. Решения владельца от 2026-08-21:
  три профиля, ровно один на репозиторий, единица живёт в дереве ядра до
  появления профиля чужого владельца, извод без нейтрального родителя не
  заводится.

- **Сторож неотслеживаемых норм не видел исключённых.** `--exclude-standard`
  отворачивается от всего, что покрывает `.gitignore`, поэтому норма внутри
  исключённого каталога была невидима сильнее, чем просто неотслеживаемая, — и
  проверка, написанная против невидимости, проходила мимо неё молча. Нашлось на
  первом же репозитории: в рабочем месте `.cursor` исключён целиком. Вопрос
  задан в обратную сторону — для артефактов, которые реестр уже называет, — и
  это один дешёвый вызов вместо обхода дерева. Граница объявлена: обнаружить
  исключённую норму, которую никто не записал, эта проверка не может. Кейс t87.

- **Норма вне контроля версий была невидима для проверки полноты.** Список
  артефактов строится из `git ls-files`, поэтому файл, который инструмент
  грузит, но который никто не добавил в индекс, в подсчёт не попадал — и дерево
  объявлялось описанным полностью, пока действующие правила лежали вне этого
  утверждения. В корпусе это не гипотеза: в краевом агенте печати не отслежен
  ни один из девяти файлов инструкций, в службе печати — три из пяти. Проверка
  теперь называет такие файлы отдельной строкой и не засчитывает дерево как
  подтверждённое. Тот же сторож, что ядро получило в R-1, применён там, где
  живёт корпус. Кейс t86.

- **Запись о контейнере без объявленных границ была невыразима.** §3.1 требует
  для такого файла ровно одну запись с вердиктом `deferred`, а схема требовала
  `region` от всякой записи с доставкой `agents-md-section`. Участка у такого
  файла нет, поэтому предписанная протоколом запись не проходила схему, и
  написать её можно было только соврав о доставке — что и делал регрессионный
  кейс t56, единственный, кто этот путь проверял. Исключение внесено в схему
  ровно в одну точку и описано в §3.1; t56 переписан на настоящую доставку,
  добавлен t85 на границу исключения.

- **Четыре документа пакета приняты владельцем** и переведены из `in-review` в
  `maintained`: протокол приёмки, стандарт идентичности агентной нормы, реестр
  тем и реестр профилей стека.

- **Читатель YAML молча терял поля после свёрнутого скаляра с обрезкой.**
  Заголовок `>-` не совпадал с точным сравнением на `>`, поэтому значение
  читалось как обычная строка, а каждая строка под ним — как соседний ключ. В
  первой же настоящей записи приёмки так исчезли пять обязательных полей.
  Указатель обрезки `-` теперь распознаётся, `+` — отвергается: читатель
  не моделирует хвостовые переводы строк и обязан сказать об этом, а не читать
  «примерно так же». Кейсы t83, t84.

- **Корневой набор имён принадлежит релизной единице, а не верхнему уровню.**
  `document-identity` считал прописные имена допустимыми только в корне
  репозитория; стандарт говорит «корень релизной единицы». Расхождение вскрылось
  на первой же вложенной единице: `stack-profiles/CHANGELOG.md` шёл красным.
  Единица опознаётся по собственному `VERSION` — свойство проверяемое, а не
  список, который пришлось бы вести руками. Кейсы t81, t82.

- **Проверка профиля в гейте: объявление и доказательство.**
  `stack-profiles` сверяет две половины пула и падает на расхождении, отвергает
  `universal` внутри пула; `stack-profile` требует объявления от каждой записи
  инвентаризации, отвергает имя вне пула и объявление, которого не подтверждает
  манифест репозитория. Недостижимый манифест даёт UNVERIFIED, не зелёный
  результат. Ни одна половина не работает без второй: выведение профиля из
  манифеста было бы догадкой, объявление без сверки — фактом, свободным молча
  разойтись с реальностью. Отрицательные и положительный кейсы: t71–t80,
  набор 73 → 84.

  Поле `profile` стало обязательным в `registries/inventory/repositories.schema.json`;
  §4.2 `agent-instruction-identity.md` называет место объявления, §8 — оставшуюся
  границу проверки.

- **Стандарт идентичности агентной нормы.**
  `standards/workspace/agent-instruction-identity.md`: класс «агентная норма»
  (правило инструмента, навык, участок файла инструкций, протокол ядра) и
  четыре независимых ответа о нём — тема, жанр, профиль, доставка с активацией.
  Плюс `derived_from` со списком сужений для всякой редакции.

  Понадобился потому, что `document_type` описывает документ, а у предписания,
  которое инструмент грузит агенту автоматически, есть свойства, которых у
  документа нет: оно существует в нескольких редакциях под разные стеки, и
  редакции расходятся молча.

- **Протокол приёмки агентной нормы.**
  `standards/workspace/instruction-intake.md`: две фазы с необратимым порядком,
  полнота описи из перечисления отслеживаемых файлов, закрытый пул из семи
  вердиктов с сигнатурами, реестр append-only, четыре остановки. Вывод нормы из
  обращения — вердикт с обязательным обоснованием, а не удаление файла.

- **Реестр приёмки: схема и проверка гейта.**
  `registries/instruction-intake/` — схема записи (Kernel) под данные в
  `$MERIDIAN_INSTANCE/instruction-intake/<репозиторий>.yaml`. Состав полей
  зависит от вердикта: `adopt-edition` без родителя, его дайджеста и непустого
  списка сужений схему не проходит; `retire` без преемника и без причины — тоже.

  Форма проверяется общим проходом по `$schema` и собственного кода не требует.
  Код понадобился для утверждений, которых схема выразить не может. Три из них
  доказываются в чужом дереве и деградируют в UNVERIFIED, а не в проход:
  полнота (всякая норма из дерева репозитория имеет запись), происхождение
  (редакция произведена от названного текста), дисциплина упаковки (каталог
  пакета назван именем навыка; `SKILL.md` не несёт полей области действия
  правил инструмента). Два доказываются внутри ядра: сохранность (ни одна
  запись не исчезла относительно предыдущей ревизии) и существование темы.
  Кейсы t22–t25, t27.

  Существование темы проверяется, верность — нет: норма с неверно присвоенной
  темой механически неотличима от верно присвоенной. §8 стандарта приведён к
  тому, что реализовано, и разделён на «проверяется всегда» и «проверяется при
  достижимости». Оттуда же убрано утверждение «нормы вне ядра гейту не
  подчиняются»: оно верно для правки и неверно для чтения, а проверка читает их
  через реестр и инвентарь. Сверка профиля с манифестом объявлена отложенной до
  появления релизной единицы профилей — сказана, а не умолчана.

  Отсутствие реестра — INFO «приёмка не начиналась», а не дефект: приёмка это
  процесс, а не предусловие.

- **Реестр тем агентных норм.** `standards/workspace/instruction-topics.md`:
  двадцать одна нейтральная тема с трёхчастными сигнатурами. Темы, названные по
  библиотеке, в ядро не входят: их срок жизни короче срока жизни методологии.

  Пул лежит в двух файлах: имена — в `instruction-topics.yaml`, где гейт
  способен их прочитать, сигнатуры — в `.md`, где читателю нужны три ответа
  рядом. Проверка `instruction-topics` сверяет обе половины и падает на
  расхождении: пул, разложенный по двум местам без такой сверки, это два пула.
  Кейс t26.

  Пул выведен из описи корпуса, а не придуман: каждая тема подтверждена хотя бы
  одной существующей нормой. Одна тема при этом из пула удалена собственным
  правилом — предмет «параметры среды и секреты» нашёлся только фрагментом
  внутри нормы, которая сама распадается, и записан как предсказанная тема с
  условием заведения.

- **Входная страница на простом языке.** `start-here.md`: какую задачу система
  решает, из чего состоит, как выглядит обычный день, чего она стоит. Без
  терминов, пятнадцать минут чтения.

  Понадобилась потому, что входа не было вовсе: `README.md` — техническое
  описание репозитория, а `MANUAL.md` начинался с границы, переменных окружения
  и словаря. Классификация по стандарту идентичности это уже показала —
  `MANUAL.md` был единственным `unclassified` документом ядра, потому что
  совмещал три жанра сразу.

  Теперь разделено: `start-here.md` — `tutorial`, `MANUAL.md` — `how-to` и
  начинается с того, что делать, а не с того, как всё устроено. `unclassified`
  в ядре не осталось ни одного.

- **Предпосылки о форме продукта объявлены и измерены.**
  `standards/workspace/product-assumptions.md`. «Ядро не знает, какой продукт
  обслуживает» — не то же самое, что «работает с любым продуктом»: оно
  предполагает продукт определённой формы, и до сих пор эти предположения жили
  в тексте двадцати трёх нормативных документов как само собой разумеющееся.

  Шесть предпосылок, у каждой сказано, что происходит при её невыполнении:
  контроль версий, несколько репозиториев, каноническая площадка документации,
  автоматические тесты, таск-трекер, несовпадение продукта с инструментом.

  Написаны не из рассуждений, а из прогона против Instance, описывающего сам
  Meridian как продукт — один репозиторий, без wiki, без трекера. Что дал
  прогон: одно-репозиторная форма применима без правок (`inventory-git: 1/1
  confirmed` — впервые эта проверка вообще что-то подтвердила); отсутствие
  канонической площадки ломает жизненный цикл документа так, что гейт этого
  **не видит**, потому что предположение записано в прозе, а не в коде; а
  совпадение продукта с инструментом делает проверку чистоты неудовлетворимой
  по построению.

- **Стандарт идентичности документа и механическая проверка под него.**
  `standards/workspace/document-identity.md`: одно правило имени (регистр по
  уровню, самодостаточное имя, запрет на дату/версию/`final`/`new`/`copy` в
  имени) и один закрытый пул `document_type` вместо двух несовпадающих наборов,
  выросших порознь — локального в статусной модели и публикационного в шаблонах.

  У каждого типа сигнатура из трёх пунктов: на какой вопрос отвечает, что обязан
  утверждать, чего утверждать не вправе. Тип присваивается только при совпадении
  всех трёх. Остаточное присвоение запрещено — «остальные не подошли, значит
  этот» даёт тип, который ничего не означает. Это правило уже действовало для
  `area_type` и перенесено без изменения смысла. `unclassified` — состояние
  классификации: легально, дефектом является только незаписанная причина.

  Проверка `document-identity` в валидаторе устроена как правило, а не как
  список файлов: новый документ не требует правки валидатора. Исключения из
  обязательного Front Matter заданы шаблонами путей — заготовки, завендоренные
  артефакты, данные фикстур. Пять новых кейсов в регрессионном наборе (t16–t20),
  включая тот, что доказывает: обоснованный `unclassified` проходит.

  Запрещена транслитерация: имя пишется по-английски, а не русским словом в
  латинской записи. `naming-standard` — норма, `standardizaciya-imen` — дефект;
  текст внутри документа при этом остаётся русским.

  Граница проверки объявлена в самом стандарте: гейт видит, что тип есть в
  пуле, но не видит, что документ ему соответствует, и не отличает английское
  слово от русского, записанного латиницей.

- Раздел 12.1 статусной модели: `contract`, `protocol`, `skill`, `template`,
  `reference` и `changelog` используют жизненный цикл `standard`. Шесть почти
  одинаковых таблиц были бы шестью местами для расхождения.

- **Слой профилей шаблонов — платформа стала профилем, а не предположением.**
  `standards/templates/CONTRACT.md` — контракт формы публикуемого документа, не
  называющий ни одного инструмента: блок метаданных, порядок полей,
  двуязычность меток, удаление неприменимых строк, отсутствие самоссылки,
  история изменений и перечень восьми типов, которые профиль обязан покрыть.
  `standards/templates/profiles/README.md` — что профиль обязан содержать, чего
  он не вправе переопределять и как добавить новый.
  `standards/templates/profiles/confluence/PROFILE.md` — механика одной
  конкретной платформы, сведённая к таблице «требование контракта → чем
  обеспечено».

  Kernel не выбирает профиль сам: при отсутствии объявления в
  `$MERIDIAN_INSTANCE/product.yaml` состояние — `unresolved`, а не разрешение
  взять единственный существующий профиль потому, что он единственный.

### Changed

- **Тип `skill` выведен из пула жанров** (`document-identity.md` §3.1).
  Первые два столбца его сигнатуры описывали тот же предмет, что и у
  `protocol` — порядок шагов и условие завершения каждого, — другими словами;
  по §2.2 тип присваивается по предмету, а не по формулировке. Третий столбец
  различался, но различие оказалось не жанровым: `skill` был не вправе зависеть
  от фактов одного продукта без объявленного внешнего контекста, а это
  требование к переносимости упаковки. Два типа, совпадающие по предмету и
  различающиеся упаковкой, делают присвоение неоднозначным по построению:
  артефакт совпадает с обеими сигнатурами, то есть по правилу полного
  совпадения не совпадает ни с одной.

  Ограничение переносимости, которое несла третья графа выведенного типа, не
  утрачено вместе с ним: оно сохранено отдельным утверждением —
  `agent-instruction-identity.md` §5.5.

  Следствия: §2.4 получил запрет вводить тип ради упаковки, инструмента или
  места хранения; завендоренные артефакты перечислены в §4 поимённо с типом
  `protocol` вместо строки по каталогу; `skills/README.md` объявляет каталог
  способом поставки, а не жанром; раздел 12.1 статусной модели перечисляет пять
  типов вместо шести; `skill` убран из `DOCUMENT_TYPES` валидатора; кейс t21
  проверяет, что выведенный тип отвергается, — вывод типа доказан механически,
  а не только записан.

- **Всё дерево приведено к правилу имён: шестнадцать переименований.**
  `CONTRACT.md` → `template-contract.md`, `{ADR,RFC,README}.template.md` →
  `{adr,rfc,readme}-template.md`, `PROFILE.md` → `confluence-profile.md`, восемь
  `*.body.md` → `*-body.md`, `PROTOCOL.md` → `smoke-protocol.md`,
  `ACCEPTANCE-GATE.template.md` → `acceptance-gate-template.md`,
  `BOOTSTRAP.md` → `instance-bootstrap.md`. Front Matter добавлен тридцати
  документам.

  Три типа встали против имени файла — ради чего тип и объявляют, а не читают с
  пути: `verification/README.md` и `regression-testing/README.md` оказались
  `standard` (они предписывают, а не навигируют), `registries/environments/README.md`
  — `reference`. `MANUAL.md` получил `unclassified` с записанной причиной:
  он несёт сигнатуры tutorial, how-to и explanation одновременно и не совпадает
  целиком ни с одной.

  Видимый блок статуса убран у четырёх документов: это конвенция опубликованной
  страницы, и рядом с Front Matter он был бы вторым источником истины.

- **Граница описана как «инструмент и продукт», а не через людей вокруг неё.**
  Формулировки про то, что уезжает с автором и остаётся у заказчика, были
  иллюстрацией одной расстановки и затвердели в определение. Правило не
  изменилось — изменилось то, на чём оно основано: `kernel-boundary.md`
  открывается парой инструмент/продукт, правило классификации спрашивает «нужно
  ли править этот файл, чтобы применить инструмент к другому продукту», README и
  `COMPATIBILITY.md` больше не называют работодателя и владельца данных.

- **Девять файлов слоя шаблонов переименованы и перемещены.**
  `standards/templates/CONFLUENCE.md` → `profiles/confluence/PROFILE.md`;
  восемь `*.confluence-template.md` → `profiles/confluence/*.body.md`
  (Tutorial, How-to, Reference, Explanation, RFC, ADR, Incident,
  Problem Record). Тела не изменены — переехали дословно.
  Ссылки обновлены в `standards/README.md`, `document-quality.md`,
  `document-status-model.md`, `kernel-boundary.md` и обоих writers; writers
  больше не называют платформу даже как «текущую».
  Заодно в `document-quality.md` устранена ссылка на путь через junction
  (`docs/agent-standards/…`) — Kernel-документ не должен адресовать сам себя
  через имя, которого нет в чистом клоне.
- **`COMPATIBILITY.md` объявляет `0.4.x` и фиксирует асимметрию проверки
  ссылок.** Текстовая ссылка из Instance на путь внутри Kernel не проверяется
  ничем; поэтому перемещения в нормативных каталогах объявляются строкой, а не
  выводятся из зелёного прогона.
- **The documentation platform is named by role, not by vendor.**
  `standards/workspace/tooling-axes.md` now defines *canonical wiki* as a role
  resolved through `$MERIDIAN_INSTANCE/product.yaml` (`canonical_wiki`), and
  states the only three places a vendor name stays legitimate: a platform
  profile under `standards/templates/`, this changelog, and Instance data.
  Every terminological mention across the standards was renamed to the role:
  `document-status-model.md` (23 occurrences), `document-lifecycle.md`,
  `agent-memory.md`, `agent-workspace.md`, `document-quality.md`, both writers,
  `standards/README.md`, and the RFC / ADR / README templates. First step of
  closing D-3; the template profile layer, the two vendored skills and
  README section 1 follow as separate changes.
- `status: published` is redefined as "the canonical version is published on
  the platform holding the canonical-wiki role", and the definition is
  explicitly **forward-only**: `canonical_url` and `published_at` on documents
  already published are not rewritten retroactively — they record a fact that
  did happen, on the platform that was canonical at the time.
- The self-hosted example in `tooling-axes.md` no longer carries a vendor's
  default cloud address. An example naming a real vendor is the same coupling
  as a rule naming one — which is now what that document says.

No change to validator behaviour: regression suite 15/15, fixture
`0 failing, 0 warnings`, real Instance unchanged at `0 failing, 2 warnings`.

### Fixed

Результат ревью пакета агентных норм: вердикт `changes_requested`, восемь
замечаний, четыре блокирующих. Общая форма всех четырёх одна — **нормативная
гарантия и фактическая проверка разошлись**, то есть ровно тот класс дефекта,
ради которого система и строится, найденный в первом же пакете, написанном по её
правилам. Регрессионный набор вырос с 27 кейсов до 55.

- **Зелёный прогон не охватывал шесть новых файлов** (P1-1). Набор файлов ядра
  строится из `git ls-files`, а новые файлы оставались untracked: код проверялся
  кейсами t21–t27 на синтетическом ядре, документы — ничем. Файлы внесены в
  индекс; в гейт добавлен сторож — untracked-файлы дерева ядра перечисляются
  отдельным предупреждением, а строка «N файлов чисты» больше не читается как
  «проверено всё». Кейсы t28 и t29: невидимый файл назван; он же после внесения
  в индекс даёт красный прогон.

- **Сохранность реестра держалась на привычке, а не на устройстве** (P1-2).
  Сравнение шло с `HEAD` и по множеству путей. Оба выбора не ловили того, ради
  чего проверка написана: после закоммиченного удаления `HEAD` и есть усечённый
  файл, а артефакт, потерявший две записи из трёх, по путям выглядит целым.
  Теперь единица сохранности — запись (артефакт, дата, вердикт), а сверка идёт
  на двух уровнях: по всей истории файла реестра на каждом прогоне и по
  опубликованной ветке при отправке (`MERIDIAN_CHECK_PUBLISHED`, хук
  `instance-template/hooks/pre-push`). Недостижимость удалённой ветки —
  предупреждение «не сверено», не проход. Кейсы t36, t37, t38, t39.

- **Полнота считалась по файлам, а протокол объявляет единицей участок** (P1-3).
  Единица приёмки для `AGENTS.md` и `CLAUDE.md` — объявленный участок. Механика
  объявления описана в протоколе §3.1: парные маркеры-комментарии с `id`,
  `owner` и `generated`. Гейт разбирает их и требует запись на каждый участок
  владельца, отказывает записи на сгенерированный участок, называет строки вне
  всех участков и объявляет полноту по участкам, а не по файлу. Кейсы t49–t54.

- **Стандарт требовал полей, которых не нёс сам** (P1-4). §7 дополнен правилом
  самообъявления: документ ядра объявляет себя агентной нормой полем `delivery`
  и тогда обязан нести все четыре поля. Четыре документа объявлены
  (`agent-instruction-identity.md`, `instruction-intake.md`,
  `workflows/task-lifecycle.md`, `verification/smoke-protocol/smoke-protocol.md`);
  гейт проверяет комплектность и принадлежность значений пулам. Размер
  неохваченного печатается на каждом прогоне: 19 предписывающих документов ядра
  нормой себя не объявили, и §7.2 называет это объявленным долгом, а не
  результатом. Кейсы t45–t48.

- **`derived_from` не был ссылкой** (P2-5). Голый путь разрешается в разный
  текст в разном чекауте и в разный момент. Теперь это тройка «репозиторий,
  путь, ревизия», а дайджест сверяется с содержимым на названной ревизии.
  Вместе с этим исправлен смысл проверки: законное изменение родителя после
  того, как редакция взята, — не дефект записи, а **отставшая редакция**, то
  есть предупреждение. Красный прогон остался за неразрешимой ссылкой и за
  ссылкой на другой текст. Прежний вердикт учил переставлять дату записи —
  единственный ремонт, который протокол запрещает. Кейсы t24, t40–t44.

- **Обоснование вывода типа `skill` содержало ложное фактическое утверждение**
  (P2-6). «Дословно совпадала» — неправда: совпадал предмет первых двух граф
  сигнатуры, третья различалась. Формулировка исправлена в трёх местах
  (`document-identity.md` §3.1, `agent-instruction-identity.md` §5.4 и запись
  выше), а ограничение переносимости, которое несла третья графа, сохранено
  отдельным утверждением — `agent-instruction-identity.md` §5.5.

- **Пул тем отключался fail-open, а его парсер был шире объявленного** (P2-7).
  Отсутствие пула теперь красный прогон: без него нельзя судить ни одну тему, а
  предупреждение пропустило бы реестр с выдуманными темами. Сигнатуры читаются
  только между маркерами `topic-pool`; любая будущая таблица с идентификатором в
  первой ячейке больше не попадает в множество документированных тем. Проверка
  вскрыла это на себе в первый же прогон: маркер, процитированный в тексте
  целиком, — тоже маркер. Кейсы t30–t32.

- **Правильность 21 темы оставалась неподтверждённой** (P2-8). Отображение
  корпуса на темы внесено в Instance
  (`.agent/reports/current/instruction-topic-coverage-report.md`); в ядре оно
  жить не может, поскольку описывает корпус конкретного продукта. Раньше оно
  существовало вне репозиториев, то есть утверждение «каждая тема подтверждена
  нормой» не опиралось ни на что проверяемое.

  Ревью границ проведено по четырём спорным парам. Ни одна тема не слита и не
  заведена — пул остаётся из 21 темы; изменена одна сигнатура: третий столбец
  `application-bootstrap` теперь явно запрещает утверждать, где живёт состояние
  и кто им владеет, — граница со `state-management` стала проверяемой при
  присвоении. Решения по остальным трём парам и их основания записаны в самом
  отображении.

  Внесение отображения в репозиторий немедленно опровергло утверждение, ради
  которого оно вносилось: **подтверждены записями девятнадцать тем из 21**, а
  `repository-context` и `task-classification` опознаны только при чтении
  файлов-контейнеров, границы участков в которых не объявлены. §6 реестра тем
  исправлен: их статус — UNVERIFIED до разложения контейнеров на участки, и
  если предмета там не окажется, обе выводятся из пула по §4.

- **Второй проход: четырнадцать дефектов в самих исправлениях.** Правки выше
  прошли независимую проверку до записи в историю, и она нашла в них тот же
  класс дефекта, ради которого весь блок и делался. Существенное:

  - `^поле:\s*\S` считало поле заполненным, если значение пустое: `\s` включает
    перевод строки, и условие удовлетворял первый символ **следующей** строки.
    Пять из шести обязательных полей блока метаданных могли быть пустыми при
    зелёном прогоне; та же ошибка в новой проверке §7 отвергала правильную
    составную ссылку `derived_from` и принимала неправильную. Кейсы t59, t60;
  - реестр без строки `$schema` не проверялся на форму записи ничем: общий
    проход добровольный, а код приёмки полагался на него. Теперь объявление
    схемы обязательно. Кейс t58;
  - идентификатор участка не входил в ключ записи, и решение об одном участке
    контейнера можно было стереть, перенацелив запись на соседний участок;
  - переименование файла реестра в том же коммите, где удалена запись, снимало
    основание для сверки: `--follow` плюс имя файла **на той ревизии**. Кейс t61;
  - контейнер без объявленных границ покрывался записью любого вердикта, хотя
    протокол называет единственный — `deferred`. Кейс t56;
  - запись, называющая несуществующий участок, игнорировалась молча: опечатка в
    `region` роняла полноту по настоящему участку, а сама оставалась незамеченной.
    Кейс t57;
  - маркер участка, процитированный в блоке кода, разбирался как настоящий —
    контейнер, описывающий эту же разметку, не мог пройти гейт. Кейс t55;
  - отсутствие ревизии в мелком клоне (shallow clone) давало красный прогон
    вместо UNVERIFIED — ровно та ошибка, которую уточнение P2-5 велело убрать;
  - строка «дерево подтверждено полным» печаталась рядом с предупреждением о
    строках вне всех участков. Теперь такое дерево полным не считается;
  - §8 стандарта объявлял проверяемым наличие списка сужений у всякой нормы с
    `derived_from`; в коде этой проверки не было. Добавлена;
  - сторож untracked-файлов не распространялся на Instance — и первым же делом
    это подтвердилось: отчёт о покрытии тем, внесённый ради замечания P2-8, сам
    остался вне системы контроля версий. Сторож распространён на Instance.

  **Третий проход** — проверка исправлений второго прохода — нашёл ещё пять
  дефектов, четыре из них внесённые самими исправлениями:

  - гашение ограждённых блоков кода (написанное ради маркера в примере)
    при **незакрытом** ограждении гасило остаток контейнера, и текст вне всех
    участков переставал считаться. Незакрытое ограждение теперь ошибка разбора;
    закрывающее ограждение обязано быть не короче открывающего (кейсы t62, t63);
  - правило разбора маркеров стало разным для пула тем и для контейнеров.
    Сведено к одной реализации: маркер внутри ограждённого блока — пример, вне
    его — объявление, и это записано в обоих стандартах (кейс t66);
  - «текущая запись» контейнера бралась по порядку строк в файле, а не по дате:
    устаревший `deferred`, сдвинутый вниз, выдавал себя за действующее решение
    (кейс t64);
  - обратная сверка участка не доставала до контейнера, который участков не
    объявляет: запись с несуществующим `region` там игнорировалась (кейс t65);
  - внесение `region` в ключ записи сделало дописывание забытого поля
    нарушением append-only. Поведение верное, цена — нет: §6 протокола теперь
    называет её прямо, вместе с ремонтом (новая полная запись, неполная остаётся
    в истории) и с тем, почему неполная не останется незамеченной.

  **Четвёртый проход** нашёл ещё три fail-open случая в идентичности реестра:
  опечатка в `repository` выдавалась за недоступность среды; удаление целого
  файла удаляло и цикл его проверки; переименование вместе с переписанной
  локальной историей скрывало опубликованный файл за старым путём; два текущих
  файла могли объявить один `repository`. Реестр теперь
  опознаётся по стабильному inventory-id, а неизвестный id является
  неразрешимой ссылкой, а его identity обязана быть уникальной (кейсы t67–t70).
  Оба `pre-push` hook записаны исполняемыми (`100755`), иначе Unix Git
  проигнорировал бы именно механизм опубликованного уровня.

  Регрессионный набор: 55 → 73 кейса.

- **Статус `README` реестра приёмки не принадлежал его собственной статусной
  модели.** `draft` в §11 `document-status-model.md` нет: README живёт в
  `maintained | stale | deprecated | archived`. Значение исправлено. Само
  расхождение гейт не видит — он проверяет наличие поля `status`, а не
  принадлежность значения модели типа; чтобы проверять, статусные модели должны
  существовать в машиночитаемом виде, иначе проверка станет вторым источником
  истины о них. Записано как находка для следующего ревью, а не залатано кодом.

- **Условная обязательность полей проверялась только положительными случаями**
  (дополнительно к восьми). Кейсы t33–t35: `retire` без преемника и без причины
  отвергается, `retire` с одной причиной проходит, `deferred` без условия
  возврата отвергается. Без положительного случая проверка, отвергающая всё,
  выглядела бы так же.

## [0.3.0] — 2026-08-19 (`draft`)

Feedback and metrics collection for field testing, requested the same day
0.2.0 was pinned as the field-testing baseline. Purely additive: no schema
or behavior change to anything 0.2.0 already validated.

### Added

- **`standards/workspace/feedback-and-metrics.md`** — methodology for
  collecting field-testing feedback without turning it into ceremony: a
  qualitative friction log (`$MERIDIAN_INSTANCE/.agent/feedback/friction-log.md`,
  one entry per REPORT only when something actually cost time, closed
  vocabulary of friction categories) and a quantitative gate-run log
  (`$MERIDIAN_INSTANCE/.agent/metrics/validate-log.jsonl`).
- **`scripts/validate-and-log.mjs`** — transparent wrapper around
  `kernel-validate.mjs`: identical output and exit code, plus one JSON
  record appended per run against a real Instance. Runs against no Instance
  or the in-repository fixture are deliberately not logged, so the trend is
  never diluted by synthetic data. Covered by
  `test/validate-and-log.test.mjs` (5 cases: exit-code parity, one
  well-formed record per real run, append-only across runs, fixture run not
  logged, no-Instance run not logged) and wired into CI.
- `hooks/pre-push` now also runs the logging wrapper against a configured
  real Instance (non-blocking — this adds a data point, not a new gate) and
  `workflows/task-lifecycle.md`'s REPORT stage points to the friction log.
- `instance-template/` ships `.agent/feedback/` and `.agent/metrics/`
  pre-seeded, so a new product gets both streams from day one.

### Fixed

- `workflows/task-lifecycle.md` still referenced the pre-extraction "Agent
  Smoke Workflow" path; updated to point at `verification/smoke-protocol/PROTOCOL.md`
  as configured by the product smoke unit, consistent with the 0.2.0 extraction.

## [0.2.0] — 2026-08-19 (`draft`)

Правки по результатам внешнего ревью 0.1.0 (verdict: changes_requested), затем
закрытие структурных пробелов, которые ревью зафиксировало как открытые.
Помечена как baseline для полевого тестирования: с этого снимка начинается
сбор обратной связи на боевых задачах (`git tag v0.2.0`).

### Added

- **`test/kernel-validate.test.mjs`** — регрессионный набор валидатора:
  синтетические Kernel/Instance во временных каталогах, adversarial-кейс на
  каждое правило (утечка литерала и паттерна, personal path, побег ссылки,
  symlink-побег, format-нарушение, unsupported keyword в необращённой ветке,
  sha-mismatch, отсутствующий instance-context, duplicate Front Matter,
  прогон без Instance, fixture-исключение). Подключён шагом в CI gate и в
  `hooks/pre-push`; невозможный кейс печатается как SKIP, не пропускается.
- **`verification/smoke-protocol/`** — переносимое ядро smoke-методологии
  (Test Contract, Execution Context, §11 Single Verdict Authority,
  side-effect tiers, test-data правила, шаблон acceptance gate AE-1…AE-5).
  Производный текст: провенанс и SHA исходных файлов продуктового unit 0.3.0
  записаны в README пакета; сам продуктовый unit не разрезан и не изменён.
  Закрывает D-6.
- **`instance-template/`** — bootstrap-каркас нового Instance: `product.yaml`
  и реестры с `REPLACE_ME`-блокерами, скелет `.agent`, `BOOTSTRAP.md` с
  чеклистом первого дня. Шаблонные реестры валидируются в gate теми же
  схемами, что и боевые данные.
- **`scripts/preflight.mjs`** — громкая проверка подключения сессии к
  правильным Kernel/Instance до начала работы; закрывает сценарий «сессия
  молча работает со старыми корнями».

### Fixed

- **Утечка имени компонента текущего продукта** в `registries/commands/README.md`
  найдена ревью и устранена: правило переформулировано без продуктовых имён,
  конкретное исключение перенесено в данные Instance-реестра команд.
- **Product-shaped имена действий удалены из Kernel-схемы** action-профилей
  (`registries/environments/access.schema.json`): generic-действия остаются
  enum'ом, продуктовые объявляются Instance'ом по паттерну `x-*`. Их
  прозаические упоминания в `registries/environments/README.md` и
  `workflows/task-lifecycle.md` также обезличены.
- **Устаревшие абсолютные пути** (`standards/workspace/document-status-model.md`
  ссылался на до-миграционный путь скилла; `workflows/task-lifecycle.md` — на
  абсолютный путь workspace-корня) заменены Kernel-относительной ссылкой и
  нейтральной формулировкой.

### Changed

- **Валидатор: `format` проверяется, а не молча принимается.** Реализованы
  `date`, `date-time`, `uri`; любой другой format валит прогон как
  unsupported.
- **Валидатор: unsupported-keyword-проверка схем идёт по всему дереву схемы
  до валидации**, включая ветки, которые конкретный документ не посещает.
- **Валидатор: link-confinement сравнивает realpath**, а не текстовый префикс
  пути — symlink/junction внутри Kernel больше не маскирует выход за границу.
- **Список Kernel-файлов перестал быть ручным зеркалом**: он перечисляется из
  `git ls-files` и покрывает все tracked-файлы, включая `VERSION`, `LICENSE`,
  `.gitignore`, `.github/`, `hooks/` и `test/`. Бинарные артефакты не
  сканируются текстом и учитываются отдельно как покрытые sha-provenance.
- **`forbidden_patterns` в `product.yaml`**: case-sensitive regex для терминов,
  слишком общих для case-insensitive literal-поиска (например, имя компонента,
  совпадающее с обиходным словом).
- **`requires_instance_context` в `PIN.yaml`**: производный скилл декларирует
  обязательный Instance-контекст, и валидатор проверяет его наличие —
  отсутствие контекста было блокером «на словах», теперь оно механическое.
- `test/instance-fixture` поставляет `skills/bugfix-protocol/context.md`
  с вымышленными конвенциями, чтобы производный протокол был применим и в CI.
- Формулировки `README.md` приведены к собранным доказательствам: привязка к
  Confluence названа явно (D-3); «Kernel не выучил имя продукта» заменено на
  проверяемое утверждение gate с его известными пределами; «проходящий
  валидатор» ограничено объёмом проверок валидатора на момент снимка.
- **Шум предсказуемых warning'ов убран**: недостижимость продуктового
  репозитория из среды запуска — per-repo INFO, единственным WARN остаётся
  агрегат «0/N confirmed». Warning, срабатывающий на каждом зелёном прогоне,
  обучает игнорировать warning'и и разъедает правило «зелёное значит
  проверено».
- `verification/README.md`: маршрут smoke-строки указывает на Kernel-протокол
  (`verification/smoke-protocol/PROTOCOL.md`), конфигурируемый продуктовым
  smoke-unit из Instance.

### Breaking

- `registries/environments/access.schema.json`: enum действий action-профилей
  сужен до generic-набора; продуктовые действия обязаны иметь префикс `x-*`.
  Instance с action-профилями без префикса не пройдёт schema-проверку gate
  без миграции данных (см. `COMPATIBILITY.md`).

### Known state, not claimed otherwise

- Фаза 6 (приёмочный прогон полного жизненного цикла на пустом Instance)
  выполнена и задокументирована в Instance
  (`.agent/reports/current/kernel-portability-phase6-report.md`): доказана
  структурная независимость и применимость на минимальном
  одно-репозиторном продукте. Пригодность методологии для продукта с
  принципиально другой архитектурой — не проверена.
- AE-1…AE-5 продуктового smoke-unit по-прежнему `not-run`; smoke-protocol
  (Kernel) готов и покрыт документацией, но ни один продуктовый smoke-unit
  ещё не прошёл acceptance gate.
- `MANUAL.md` добавлен как единая точка входа для нового оператора; сама
  система впервые выходит за пределы одной агентной сессии, которая её
  строила.

## [0.1.0] — 2026-08-18 (`draft`)

Первый снимок Kernel как самостоятельной релизной единицы под контролем версий.
Версия не является ретроспективной меткой прошлой работы: она обозначает первое
состояние, которое одновременно лежит в Git и проходит валидатор.

### Added

- Физическое разделение Kernel и Instance на два репозитория. До этого граница
  существовала только как документ.
- `skills/bugfix-protocol/` — завендорен внутрь Kernel. Раньше методология
  жила по пути на машине одного оператора и была невоспроизводима где-либо ещё.
  Артефакт производный: удалены имя продукта и раздел продуктовых конвенций,
  оба дайджеста записаны раздельно в `PIN.yaml`.
- `skills/versioning-standard-docs/source/` — исходный архив внутри Kernel, так
  что цепочка «архив → запись → установленный файл» проверяется из чистого
  checkout, а не со слов.
- `test/instance-fixture/` — синтетический Instance для CI и приёмочного прогона.
- `.github/workflows/gate.yml` — валидатор на каждый push и PR, с явной печатью
  того, что CI проверить не может.
- `VERSION`, `CHANGELOG.md`, `COMPATIBILITY.md`, `LICENSE`, `.gitignore`.

### Changed

- Валидатор переведён с трёх жёстко заданных корней на `MERIDIAN_KERNEL` и
  `MERIDIAN_INSTANCE`. Отсутствие Instance теперь явно объявляется как
  «product-литералы не проверялись», а не молча пропускается.
- Проверка провенанса переписана с одного файла на обход всех `skills/*/PIN.yaml`
  с пересчётом дайджестов, включая исходные архивы.
- JSON Schema реестров резолвятся из Kernel по имени каталога реестра.
  Сопоставление по одному имени файла проверило бы `commands/repositories.yaml`
  схемой из `inventory/` — оба реестра объявляют `repositories.schema.json`.
- Добавлено правило: относительная ссылка Kernel-документа, уходящая за пределы
  Kernel, — дефект, даже если на текущей машине она открывается. Восемь таких
  ссылок нашлись сразу и переписаны в форму `$MERIDIAN_INSTANCE/...`.
- Область проверки чистоты расширена на JSON Schema реестров: пример значения
  внутри схемы — такая же утечка, как фраза в тексте.
- Пути перестроены под Meridian: `docs/agent-standards` → `standards/`,
  `engineering-workspace/{commands,environments,inventory,adapters}` →
  `registries/`, `governance/kernel-validate.mjs` → `scripts/`,
  `.agent/README.md` → `standards/workspace/agent-memory.md`.

### Known state, not claimed otherwise

- Приёмочный прогон полного жизненного цикла задачи на `test/instance-fixture`
  ещё не выполнялся. Структурная независимость проверена механически;
  практическая пригодность на чужой архитектуре — нет.
- Записи инвентаря сообщают `UNVERIFIED`, когда продуктовые репозитории
  недостижимы из текущего окружения. Это работа проверки по назначению, а не сбой.
