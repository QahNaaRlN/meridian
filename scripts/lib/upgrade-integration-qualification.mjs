/**
 * upgrade-integration-qualification: the one checkable implementation.
 *
 * registries/operating-model/upgrade-integration-qualification.schema.json is
 * the specialised payload schema for the final, product-neutral qualification
 * (record_type: upgrade-integration-qualification) of one workspace's run
 * through packages 1-8 of the meridian-operating-upgrade program:
 * meridian-operating-foundation, task-pattern-registry,
 * task-specification-contract, execution-state-model, role-and-human-control,
 * bounded-context-manifest, evidence-and-handoff-contract and
 * meridian-field-evaluation. This module composes those EIGHT
 * already-checkable contracts into one closed verdict; it duplicates none of
 * their logic — the same discipline
 * scripts/lib/workspace-compatibility-qualification.mjs already applies to
 * the five contracts of meridian-workspace-compatibility.
 *
 * Bounded, not embedded (property: no persisted composed body). A
 * qualification record never carries a composed evidence-and-handoff,
 * field-evaluation-report, task-specification or execution-run record INLINE
 * — only closed, pinned references:
 *
 *   payload.task_journey_ref           — { record_type, id, reference, sha256 }
 *   payload.field_evaluation_report_ref — { record_type, id, reference, sha256 } | null
 *   payload.scenario_classifications   — exactly 3 { scenario_id, task_specification_ref, execution_state_ref }
 *
 * One terminal reference covers packages 2-7 (property: no re-resolution of
 * an already-composed chain). A resolved evidence-and-handoff record already
 * composes execution-state-model, role-and-human-control and
 * bounded-context-manifest, and — transitively, through its own
 * task_specification_ref — task-specification-contract and
 * task-pattern-registry (evidence-and-handoff-contract.md §4, §12). This
 * module therefore resolves and composes exactly ONE evidence-and-handoff
 * record (payload.task_journey_ref) as the terminal point for packages 2-7,
 * via the REAL evaluateEvidenceAndHandoff, and never resolves any of those
 * four packages a second time on its own account.
 *
 * A field-evaluation-report is composed separately, never folded into the
 * task journey's verdict (property: no collapse of two different closed
 * shapes into one). meridian-field-evaluation.md §3 requires eight
 * characteristics reported SEPARATELY, with no collapse into one score — a
 * field-evaluation-report therefore carries no pass/fail axis this module
 * could read as a verdict. payload.field_evaluation_report_ref is composed,
 * via the REAL evaluateFieldEvaluation, only for its own structural
 * correctness; its presence (not its content) is what the decision matrix
 * reads, because "the owner receives an intelligible report" (the operating
 * upgrade programme's release bar, §12.11) is a presence requirement, not a
 * score requirement.
 *
 * Exactly three neutral scenarios, every one checkable, none executed
 * (property: closed scenario coverage). payload.scenario_classifications is
 * closed to exactly three entries — one per required scenario_id
 * (single-module-refactor, multi-repository-decomposition,
 * language-change-limit-case; upgrade-integration-qualification.md §3) — no
 * gap, no duplicate, no extra. Each entry's task_specification_ref and
 * execution_state_ref are resolved through an external boundary and composed
 * against the REAL evaluateTaskSpecification / evaluateExecutionState, then
 * checked against this scenario's own closed expectation:
 *
 *   - single-module-refactor: the resolved task-specification's task_pattern
 *     is "refactor-preserving-behavior" (task-pattern-registry.md §2, a
 *     `change` pattern that carries no decomposition_required).
 *   - multi-repository-decomposition: the resolved task-specification's
 *     task_pattern is "decompose-initiative" AND the resolved execution-run's
 *     lifecycle_stage is strictly past "classification" — decomposition is
 *     actually proceeding, not merely named.
 *   - language-change-limit-case: the resolved task-specification's
 *     task_pattern is ALSO "decompose-initiative" (an implementation-language
 *     change is definitionally an initiative requiring decomposition,
 *     task-pattern-registry.md §2.1) but the resolved execution-run's
 *     lifecycle_stage is AT OR BEFORE "classification" — proving
 *     classification happened and NOTHING past it did, the checkable form of
 *     "only classification, no product rewrite is executed"
 *     (upgrade-integration-qualification.md §3, the operating upgrade plan
 *     §12.5).
 *
 * A product-neutral Kernel/Instance transition template, not filled Instance
 * evidence (property: honest, bounded scope). payload.workspace_transition_compatibility
 * is required and closed to status "kernel-template-only" in this Kernel
 * slice: it records that a compatibility requirement/template for the
 * Kernel/Instance transition to the new operating-model workspace model
 * exists, never actual evidence for any specific product's transition — that
 * is the subject of a separate, later Instance package
 * (upgrade-integration-qualification.md §4) and is never composed here.
 *
 * Decision matrix, not "the three scenarios" (property: separated concerns,
 * same split workspace-compatibility-qualification.md §4.1/§4.2 already
 * draws). computeIntegrationQualificationState is a closed, pure PRIORITY
 * FUNCTION — an internal implementation detail of how one qualification_state
 * is derived from the task journey's own outcome, whether every required
 * scenario resolves and classifies as expected, and whether a
 * field-evaluation-report has been composed. It is not the three neutral
 * scenarios themselves — those are a separate, checkable set, proven by
 * dedicated fixtures/tests (test/upgrade-integration-qualification.test.mjs).
 *
 * Closed result axes (property: three, not two — the same relation
 * workspace-compatibility-qualification.md §4.1 already states).
 * qualification_state, blockers and open_questions are checked as ONE closed
 * relation: blockers is non-empty EXACTLY when qualification_state is
 * BLOCKED; open_questions is non-empty EXACTLY when qualification_state is
 * UNVERIFIED.
 */
import { createHash } from 'node:crypto';
import { validate } from './json-schema.mjs';
import { evaluateEvidenceAndHandoff } from './evidence-and-handoff.mjs';
import { evaluateFieldEvaluation } from './field-evaluation.mjs';
import { evaluateTaskSpecification } from './task-specification.mjs';
import { evaluateExecutionState } from './execution-state.mjs';

const isObject = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);

export const RECORD_TYPE = 'upgrade-integration-qualification';
export const ALLOWED_SCOPE_TYPE = 'project-workspace';
export const REQUIRED_ORIGIN_KIND = 'derived';
export const REQUIRED_AUTHORITY_KIND = 'delegated-run';
export const QUALIFICATION_STATES = ['QUALIFIED', 'BLOCKED', 'UNVERIFIED'];
export const PINNED_REF_KEYS = ['record_type', 'id', 'reference', 'sha256'];
export const SHA256_HEX = /^[0-9a-f]{64}$/;

export const SCENARIO_IDS = [
  'single-module-refactor',
  'multi-repository-decomposition',
  'language-change-limit-case',
];

// Closed lifecycle stage order (execution-state-model.md §7) — read only to
// compare two stages' relative position, never to invent a ninth or tenth
// value this module does not otherwise recognise.
export const LIFECYCLE_STAGE_ORDER = [
  'intake', 'classification', 'norm_resolution', 'planning', 'execution',
  'verification', 'acceptance', 'integration', 'deployment', 'observation', 'completion',
];

export const EXPECTED_PATTERN_BY_SCENARIO = {
  'single-module-refactor': 'refactor-preserving-behavior',
  'multi-repository-decomposition': 'decompose-initiative',
  'language-change-limit-case': 'decompose-initiative',
};

/**
 * The workspace a scope-bearing record belongs to (§6.1): for
 * "project-workspace" scope, the workspace IS scope.id; for every other
 * scope type that carries one (run-state, repository-scope), the workspace
 * is the separate scope.workspace_id field — task-specification legally
 * uses either scope shape (task-specification.md §2), so this reads
 * whichever field actually names the workspace instead of assuming one
 * fixed key.
 */
function workspaceIdOfScope(scope) {
  if (!isObject(scope)) return undefined;
  return scope.type === 'project-workspace' ? scope.id : scope.workspace_id;
}

function stageIndex(stage) {
  const i = LIFECYCLE_STAGE_ORDER.indexOf(stage);
  return i; // -1 for an unrecognised stage — every comparison below treats -1 as "not past classification".
}

/**
 * Recursive, key-order-insensitive canonicalisation — the same shape
 * computeConnectionDigest (workspace-compatibility-qualification.mjs)
 * already applies, reimplemented here for the same reason: this module
 * composes a package's PUBLIC contract, never its internal helpers.
 */
function canonicalize(value) {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (value && typeof value === 'object') {
    return Object.keys(value).sort().reduce((acc, k) => {
      acc[k] = canonicalize(value[k]);
      return acc;
    }, {});
  }
  return value;
}

/**
 * The deterministic content digest of one FULLY RESOLVED record this module
 * pins by reference (an evidence-and-handoff, a field-evaluation-report, a
 * task-specification or an execution-run) — SHA-256 over the canonicalised
 * (recursively sorted-key) JSON projection of its complete meaningful content
 * (id, title, record_type, scope, origin, authority, payload), insensitive to
 * declaration order. None of the four composed packages defines an analogous
 * fingerprint of its own to reuse, so this module defines one, exactly as
 * workspace-compatibility-qualification.mjs's computeConnectionDigest does
 * for a workspace-connection-scan.
 */
export function computeContentDigest(record) {
  const r = isObject(record) ? record : {};
  const projection = {
    id: r.id,
    title: r.title,
    record_type: r.record_type,
    scope: r.scope,
    origin: r.origin,
    authority: r.authority,
    payload: r.payload,
  };
  return createHash('sha256').update(JSON.stringify(canonicalize(projection))).digest('hex');
}

/** Checks the closed shape of one pinned reference — never a plain string, never an embedded body. */
function checkPinnedRefShape(problems, at, field, ref) {
  if (!isObject(ref)) {
    problems.push(`${at} ${field} is not an object`);
    return false;
  }
  let ok = true;
  for (const k of Object.keys(ref)) {
    if (!PINNED_REF_KEYS.includes(k)) {
      problems.push(`${at} ${field} has unknown field "${k}"; a pinned reference is closed to { ${PINNED_REF_KEYS.join(', ')} }`);
      ok = false;
    }
  }
  if (typeof ref.record_type !== 'string' || ref.record_type === '') { problems.push(`${at} ${field}.record_type is not a non-empty string`); ok = false; }
  if (typeof ref.id !== 'string' || ref.id === '') { problems.push(`${at} ${field}.id is not a non-empty string`); ok = false; }
  if (typeof ref.reference !== 'string' || ref.reference === '') { problems.push(`${at} ${field}.reference is not a non-empty string`); ok = false; }
  if (typeof ref.sha256 !== 'string' || !SHA256_HEX.test(ref.sha256)) {
    problems.push(`${at} ${field}.sha256 is required and must be 64 lowercase hex characters — the exact recomputed content pin, never a bare id`);
    ok = false;
  }
  return ok;
}

/** No two declared scenario entries may pin the same task-specification or execution-run reference. */
function checkNoDuplicateRefs(problems, at, field, refs) {
  const seen = new Map();
  refs.forEach((ref, i) => {
    if (!isObject(ref) || typeof ref.reference !== 'string') return;
    if (seen.has(ref.reference)) {
      problems.push(`${at} ${field}[${i}].reference "${ref.reference}" repeats ${field}[${seen.get(ref.reference)}].reference; each scenario names a distinct composed record`);
    } else {
      seen.set(ref.reference, i);
    }
  });
}

/**
 * Resolves one pinned reference through an external boundary, checks the
 * resolved response's record_type/id echo the pinned values, and verifies
 * the pinned sha256 equals the resolved record's OWN recomputed content
 * digest — a claimed value with no resolved, matching backing never confirms
 * identity, even when id and reference already match literally.
 */
function resolveNamedRecord(problems, at, field, ref, resolve, expectedRecordType) {
  const doResolve = typeof resolve === 'function' ? resolve : () => null;
  const resolved = doResolve(ref.reference);
  if (!isObject(resolved)) {
    problems.push(`${at} ${field} "${ref.reference}" does not resolve through the external boundary; an unresolved composed record is never accepted`);
    return null;
  }
  if (resolved.record_type !== expectedRecordType) {
    problems.push(`${at} ${field} "${ref.reference}": resolved record_type is "${resolved.record_type}", not "${expectedRecordType}"`);
  }
  if (resolved.id !== ref.id) {
    problems.push(`${at} ${field} "${ref.reference}": resolved id "${resolved.id}" does not echo the pinned id "${ref.id}"`);
  }
  const recomputed = computeContentDigest(resolved);
  if (ref.sha256 !== recomputed) {
    problems.push(`${at} ${field} "${ref.reference}": sha256 "${ref.sha256}" does not equal the resolved record's own recomputed content digest "${recomputed}"; a pinned reference names the exact composed version, never a bare id/reference an independent resolver could satisfy with different content under the same label`);
    return null;
  }
  return resolved;
}

/**
 * Checks one resolved scenario_classifications[] entry against its own
 * scenario_id's closed expectation (task_pattern id, and — for the two
 * initiative-shaped scenarios — the resolved execution-run's lifecycle_stage
 * relative to "classification"). Reads only named fields; never reaches into
 * the composed records by array position.
 */
function checkScenarioExpectation(problems, at, scenarioId, resolvedTaskSpec, resolvedExecutionRun) {
  const expectedPattern = EXPECTED_PATTERN_BY_SCENARIO[scenarioId];
  const actualPattern = resolvedTaskSpec && isObject(resolvedTaskSpec.payload) && isObject(resolvedTaskSpec.payload.task_pattern)
    ? resolvedTaskSpec.payload.task_pattern.id : undefined;
  if (expectedPattern && actualPattern !== expectedPattern) {
    problems.push(`${at} scenario "${scenarioId}" resolves a task-specification classified as task_pattern "${actualPattern}", not the expected "${expectedPattern}"`);
  }

  const stage = resolvedExecutionRun && isObject(resolvedExecutionRun.payload) ? resolvedExecutionRun.payload.lifecycle_stage : undefined;
  const idx = stageIndex(stage);
  const classificationIdx = LIFECYCLE_STAGE_ORDER.indexOf('classification');

  if (scenarioId === 'language-change-limit-case') {
    if (idx < 0 || idx > classificationIdx) {
      problems.push(`${at} scenario "language-change-limit-case" resolves an execution-run at lifecycle_stage "${stage}", past "classification"; this scenario proves classification only — no execution of a product rewrite is ever expected to have proceeded`);
    }
  } else if (scenarioId === 'multi-repository-decomposition') {
    if (idx <= classificationIdx) {
      problems.push(`${at} scenario "multi-repository-decomposition" resolves an execution-run at lifecycle_stage "${stage}", not past "classification"; a decomposition initiative that never proceeds past classification does not demonstrate decomposition actually happening`);
    }
  }
}

/**
 * The closed, pure decision matrix (property: separated concerns — see the
 * module-level comment). Reads five signals off records the composed
 * evaluators/checks have ALREADY checked clean; never recomputes
 * outcome.status or a scenario's own classification itself.
 *
 *   1. the task journey does not resolve, or its composition is not clean → BLOCKED
 *   2. the resolved task journey's outcome.status is "blocked" → BLOCKED
 *   3. scenario coverage is not exactly the three required, distinct scenario_ids → BLOCKED
 *   4. any required scenario's task-specification/execution-run does not resolve/compose cleanly, or does not match its scenario's expectation → BLOCKED
 *   5. the resolved task journey's outcome.status is "handed_off_incomplete" → UNVERIFIED
 *   6. a declared field_evaluation_report_ref does not resolve/compose cleanly → BLOCKED
 *   7. no field_evaluation_report_ref is declared at all → UNVERIFIED (the owner has no intelligible report yet)
 *   8. otherwise → QUALIFIED
 */
export function computeIntegrationQualificationState({
  taskJourneyResolved, outcomeStatus, scenarioCoverageOk, allScenariosClean,
  fieldEvaluationRefPresent, fieldEvaluationClean,
} = {}) {
  if (!taskJourneyResolved) return 'BLOCKED';
  if (outcomeStatus === 'blocked') return 'BLOCKED';
  if (!scenarioCoverageOk) return 'BLOCKED';
  if (!allScenariosClean) return 'BLOCKED';
  if (outcomeStatus === 'handed_off_incomplete') return 'UNVERIFIED';
  if (fieldEvaluationRefPresent && !fieldEvaluationClean) return 'BLOCKED';
  if (!fieldEvaluationRefPresent) return 'UNVERIFIED';
  return 'QUALIFIED';
}

/**
 * The whole composition pipeline for one upgrade-integration-qualification
 * document: container + payload schema, per-entry envelope schema,
 * resolution of task_journey_ref/field_evaluation_report_ref/
 * scenario_classifications[] through the caller-supplied external
 * boundaries, composition of the RESOLVED records against the REAL
 * evaluators of the four contracts they name, then the cross-cutting rules
 * the JSON Schema subset cannot state (exact-version pinning, exact scenario
 * coverage, each scenario's own classification expectation, the recomputed
 * qualification_state, and the blockers/open_questions/qualification_state
 * closed relation).
 *
 * `taskJourneyOptions`/`fieldEvaluationOptions` carry evidence-and-handoff's
 * and field-evaluation's OWN schemas and OWN external boundary
 * (recordSchema, envelopeSchema, resolveRecords — never this module's own
 * top-level resolvers); `taskSpecificationOptions` carries
 * task-specification's recordSchema/envelopeSchema/taskPatterns (the static
 * catalogue, not a resolver); `executionStateOptions` carries
 * execution-state's recordSchema/envelopeSchema (it resolves nothing
 * external of its own).
 */
export function evaluateUpgradeIntegrationQualification(doc, {
  registrySchema, envelopeSchema,
  resolveTaskJourney, resolveFieldEvaluationReport, resolveTaskSpecification, resolveExecutionState,
  taskJourneyOptions = {}, fieldEvaluationOptions = {}, taskSpecificationOptions = {}, executionStateOptions = {},
} = {}) {
  if (!isObject(registrySchema)) return ['registrySchema is not an object; the upgrade-integration-qualification contract cannot be checked without its schema'];
  if (!isObject(envelopeSchema)) return ['envelopeSchema is not an object; a qualification record composes with the record envelope and cannot be checked without it'];

  const namespaceFailures = {
    'taskJourneyOptions.recordSchema': isObject(taskJourneyOptions.recordSchema),
    'taskJourneyOptions.envelopeSchema': isObject(taskJourneyOptions.envelopeSchema),
    'fieldEvaluationOptions.recordSchema': isObject(fieldEvaluationOptions.recordSchema),
    'fieldEvaluationOptions.envelopeSchema': isObject(fieldEvaluationOptions.envelopeSchema),
    'taskSpecificationOptions.recordSchema': isObject(taskSpecificationOptions.recordSchema),
    'taskSpecificationOptions.envelopeSchema': isObject(taskSpecificationOptions.envelopeSchema),
    'taskSpecificationOptions.taskPatterns': Array.isArray(taskSpecificationOptions.taskPatterns),
    'executionStateOptions.recordSchema': isObject(executionStateOptions.recordSchema),
    'executionStateOptions.envelopeSchema': isObject(executionStateOptions.envelopeSchema),
  };
  const missing = Object.keys(namespaceFailures).filter((k) => !namespaceFailures[k]);
  if (missing.length) {
    return [`upgrade-integration-qualification composition requires the real composed contract schemas regardless of this document's own content (payload.task_journey_ref and payload.scenario_classifications are always composed): missing or not an object/array — ${missing.join(', ')}`];
  }

  const problems = [];

  const containerErrs = [];
  try { validate(doc, registrySchema, registrySchema, '', containerErrs); }
  catch (e) { return [`container/payload schema could not be applied: ${e.message}`]; }
  problems.push(...containerErrs);

  const entries = Array.isArray(doc && doc.qualifications) ? doc.qualifications : [];

  for (let i = 0; i < entries.length; i += 1) {
    const envErrs = [];
    try { validate(entries[i], envelopeSchema, envelopeSchema, '', envErrs); }
    catch (e) { problems.push(`entry ${i} envelope could not be applied: ${e.message}`); continue; }
    problems.push(...envErrs.map((m) => `entry ${i} envelope ${m}`));
  }

  const seenIds = new Set();
  for (const e of entries) {
    if (!isObject(e)) continue;
    const id = typeof e.id === 'string' ? e.id : `#${entries.indexOf(e)}`;
    const at = `qualification "${id}"`;
    if (typeof e.id === 'string') {
      if (seenIds.has(e.id)) problems.push(`${at} is declared more than once`);
      seenIds.add(e.id);
    }
    if (e.record_type !== RECORD_TYPE) {
      problems.push(`${at} declares record_type "${e.record_type}", not "${RECORD_TYPE}"`);
    }
    const scope = isObject(e.scope) ? e.scope : {};
    if (scope.type !== ALLOWED_SCOPE_TYPE) {
      problems.push(`${at} scope.type is "${scope.type}"; an upgrade-integration-qualification is scoped to project-workspace only`);
    }
    // The single workspace identity every composed reference is checked
    // against (§6.1) — the qualification's own scope.id, since scope.type is
    // project-workspace. undefined (a malformed scope, already flagged above)
    // disables the cross-checks below rather than comparing against "undefined".
    const qualificationWorkspaceId = typeof scope.id === 'string' ? scope.id : undefined;
    const origin = isObject(e.origin) ? e.origin : {};
    if (origin.kind !== REQUIRED_ORIGIN_KIND) {
      problems.push(`${at} origin.kind is "${origin.kind}", not "${REQUIRED_ORIGIN_KIND}"; a qualification is computed from the composed records it pins, never declared on its own`);
    }
    const authority = isObject(e.authority) ? e.authority : {};
    if (authority.kind !== REQUIRED_AUTHORITY_KIND) {
      problems.push(`${at} authority.kind is "${authority.kind}", not "${REQUIRED_AUTHORITY_KIND}"; computing a qualification is carried out by a run and never itself mints an owner decision`);
    }

    const payload = isObject(e.payload) ? e.payload : {};

    // --- task_journey_ref: always required, sha256-pinned, composed against
    //     the REAL evaluateEvidenceAndHandoff. ---
    const journeyRef = payload.task_journey_ref;
    let resolvedJourney = null;
    if (!isObject(journeyRef)) {
      problems.push(`${at} payload.task_journey_ref is missing; a qualification without a pinned, resolvable evidence-and-handoff record does not exist`);
    } else if (checkPinnedRefShape(problems, at, 'payload.task_journey_ref', journeyRef)) {
      resolvedJourney = resolveNamedRecord(problems, at, 'payload.task_journey_ref', journeyRef, resolveTaskJourney, 'evidence-and-handoff');
    }
    let journeyClean = false;
    let outcomeStatus;
    if (resolvedJourney) {
      const journeyProblems = evaluateEvidenceAndHandoff(resolvedJourney, taskJourneyOptions);
      if (journeyProblems.length) {
        problems.push(...journeyProblems.map((m) => `${at} payload.task_journey_ref: ${m}`));
      } else {
        journeyClean = true;
      }
      outcomeStatus = isObject(resolvedJourney.payload) && isObject(resolvedJourney.payload.outcome)
        ? resolvedJourney.payload.outcome.status : undefined;

      // Unified workspace identity (§6.1): the resolved task journey's own
      // run-state scope.workspace_id must name this qualification's own
      // workspace — a structural check, separate from the decision matrix,
      // the same way workspace-compatibility-qualification.mjs checks scope
      // agreement independently of computeQualificationState.
      const journeyWorkspaceId = workspaceIdOfScope(resolvedJourney.scope);
      if (qualificationWorkspaceId !== undefined && journeyWorkspaceId !== qualificationWorkspaceId) {
        problems.push(`${at} payload.task_journey_ref resolves to scope.workspace_id "${journeyWorkspaceId}", not this qualification's own workspace "${qualificationWorkspaceId}"; the task journey, the field-evaluation report and every scenario must belong to the same workspace`);
      }
    }

    // --- field_evaluation_report_ref: null legal, otherwise sha256-pinned
    //     and composed against the REAL evaluateFieldEvaluation. ---
    const fieldRef = payload.field_evaluation_report_ref;
    const fieldPresent = fieldRef !== null && fieldRef !== undefined;
    let fieldClean = false;
    if (fieldPresent) {
      let resolvedField = null;
      if (checkPinnedRefShape(problems, at, 'payload.field_evaluation_report_ref', fieldRef)) {
        resolvedField = resolveNamedRecord(problems, at, 'payload.field_evaluation_report_ref', fieldRef, resolveFieldEvaluationReport, 'field-evaluation-report');
      }
      if (resolvedField) {
        const fieldProblems = evaluateFieldEvaluation(resolvedField, fieldEvaluationOptions);
        if (fieldProblems.length) {
          problems.push(...fieldProblems.map((m) => `${at} payload.field_evaluation_report_ref: ${m}`));
        } else {
          fieldClean = true;
        }

        // Unified workspace identity (§6.1) — the resolved report's own
        // payload.workspace_id must name this qualification's own workspace.
        const fieldWorkspaceId = isObject(resolvedField.payload) ? resolvedField.payload.workspace_id : undefined;
        if (qualificationWorkspaceId !== undefined && fieldWorkspaceId !== qualificationWorkspaceId) {
          problems.push(`${at} payload.field_evaluation_report_ref resolves to payload.workspace_id "${fieldWorkspaceId}", not this qualification's own workspace "${qualificationWorkspaceId}"; the task journey, the field-evaluation report and every scenario must belong to the same workspace`);
        }
      }
    } else if (fieldRef !== null) {
      problems.push(`${at} payload.field_evaluation_report_ref is missing; the field must be explicitly present with a pinned reference or explicit null`);
    }

    // --- scenario_classifications: exactly three, one per required
    //     scenario_id, each resolved and composed against the REAL
    //     evaluateTaskSpecification / evaluateExecutionState, then checked
    //     against that scenario's own closed expectation. ---
    const scenarios = Array.isArray(payload.scenario_classifications) ? payload.scenario_classifications : [];
    if (scenarios.length !== SCENARIO_IDS.length) {
      problems.push(`${at} payload.scenario_classifications carries ${scenarios.length} entries, not exactly ${SCENARIO_IDS.length}; every required neutral scenario is covered, with no gap and no extra`);
    }
    const declaredScenarioIds = scenarios.filter(isObject).map((s) => s.scenario_id);
    for (const want of SCENARIO_IDS) {
      const count = declaredScenarioIds.filter((s) => s === want).length;
      if (count !== 1) {
        problems.push(`${at} payload.scenario_classifications declares scenario_id "${want}" ${count} time(s), expected exactly 1`);
      }
    }
    for (const s of declaredScenarioIds) {
      if (s !== undefined && !SCENARIO_IDS.includes(s)) {
        problems.push(`${at} payload.scenario_classifications declares unknown scenario_id "${s}"`);
      }
    }
    const scenarioCoverageOk = SCENARIO_IDS.every((want) => declaredScenarioIds.filter((s) => s === want).length === 1)
      && declaredScenarioIds.every((s) => SCENARIO_IDS.includes(s));

    checkNoDuplicateRefs(problems, at, 'payload.scenario_classifications[].task_specification_ref',
      scenarios.filter(isObject).map((s) => s.task_specification_ref));
    checkNoDuplicateRefs(problems, at, 'payload.scenario_classifications[].execution_state_ref',
      scenarios.filter(isObject).map((s) => s.execution_state_ref));

    let allScenariosClean = scenarios.length > 0;
    scenarios.forEach((s, i) => {
      const sObj = isObject(s) ? s : {};
      const field = `payload.scenario_classifications[${i}]`;
      let specClean = false;
      let runClean = false;
      let resolvedSpec = null;
      let resolvedRun = null;

      if (!isObject(sObj.task_specification_ref)) {
        problems.push(`${at} ${field}.task_specification_ref is missing`);
        allScenariosClean = false;
      } else if (checkPinnedRefShape(problems, at, `${field}.task_specification_ref`, sObj.task_specification_ref)) {
        resolvedSpec = resolveNamedRecord(problems, at, `${field}.task_specification_ref`, sObj.task_specification_ref, resolveTaskSpecification, 'task-specification');
      }
      if (resolvedSpec) {
        const specProblems = evaluateTaskSpecification(resolvedSpec, taskSpecificationOptions);
        if (specProblems.length) problems.push(...specProblems.map((m) => `${at} ${field}.task_specification_ref: ${m}`));
        else specClean = true;

        // Unified workspace identity (§6.1) — the resolved specification's
        // own project-workspace scope.id must name this qualification's own
        // workspace.
        const specWorkspaceId = workspaceIdOfScope(resolvedSpec.scope);
        if (qualificationWorkspaceId !== undefined && specWorkspaceId !== qualificationWorkspaceId) {
          problems.push(`${at} ${field}.task_specification_ref resolves to workspace "${specWorkspaceId}", not this qualification's own workspace "${qualificationWorkspaceId}"; the task journey, the field-evaluation report and every scenario must belong to the same workspace`);
          allScenariosClean = false;
        }
      }

      if (!isObject(sObj.execution_state_ref)) {
        problems.push(`${at} ${field}.execution_state_ref is missing`);
        allScenariosClean = false;
      } else if (checkPinnedRefShape(problems, at, `${field}.execution_state_ref`, sObj.execution_state_ref)) {
        resolvedRun = resolveNamedRecord(problems, at, `${field}.execution_state_ref`, sObj.execution_state_ref, resolveExecutionState, 'execution-run');
      }
      if (resolvedRun) {
        const runProblems = evaluateExecutionState(resolvedRun, executionStateOptions);
        if (runProblems.length) problems.push(...runProblems.map((m) => `${at} ${field}.execution_state_ref: ${m}`));
        else runClean = true;

        // Unified workspace identity (§6.1) — the resolved run's own
        // run-state scope.workspace_id must name this qualification's own
        // workspace.
        const runWorkspaceId = workspaceIdOfScope(resolvedRun.scope);
        if (qualificationWorkspaceId !== undefined && runWorkspaceId !== qualificationWorkspaceId) {
          problems.push(`${at} ${field}.execution_state_ref resolves to scope.workspace_id "${runWorkspaceId}", not this qualification's own workspace "${qualificationWorkspaceId}"; the task journey, the field-evaluation report and every scenario must belong to the same workspace`);
          allScenariosClean = false;
        }

        // Exact specification/run linkage (§3.1) — the resolved run's own
        // payload.task_specification_ref must equal THIS scenario's own
        // declared task_specification_ref.reference: an execution-run
        // belonging to a different specification is not this scenario's own
        // run, even when both individually pass their own real evaluator.
        if (isObject(sObj.task_specification_ref) && typeof sObj.task_specification_ref.reference === 'string') {
          const runsSpecRef = isObject(resolvedRun.payload) ? resolvedRun.payload.task_specification_ref : undefined;
          if (runsSpecRef !== sObj.task_specification_ref.reference) {
            problems.push(`${at} ${field}: resolved execution-run's payload.task_specification_ref "${runsSpecRef}" does not equal this scenario's own task_specification_ref.reference "${sObj.task_specification_ref.reference}"; an execution-run belonging to a different specification is not this scenario's own run`);
            allScenariosClean = false;
          }
        }
      }

      if (!specClean || !runClean) { allScenariosClean = false; return; }

      const before = problems.length;
      if (SCENARIO_IDS.includes(sObj.scenario_id)) {
        checkScenarioExpectation(problems, at, sObj.scenario_id, resolvedSpec, resolvedRun);
      }
      if (problems.length > before) allScenariosClean = false;
    });

    // --- the recomputed qualification_state, and the closed relation with
    //     blockers/open_questions. ---
    const computed = computeIntegrationQualificationState({
      taskJourneyResolved: journeyClean,
      outcomeStatus,
      scenarioCoverageOk,
      allScenariosClean,
      fieldEvaluationRefPresent: fieldPresent,
      fieldEvaluationClean: fieldClean,
    });

    if (!QUALIFICATION_STATES.includes(payload.qualification_state)) {
      problems.push(`${at} payload.qualification_state is "${payload.qualification_state}", not one of { ${QUALIFICATION_STATES.join(', ')} }`);
    } else if (payload.qualification_state !== computed) {
      problems.push(`${at} payload.qualification_state is declared "${payload.qualification_state}" but recomputes to "${computed}"; a qualification_state is recomputed, never trusted on its own`);
    }

    const blockers = Array.isArray(payload.blockers) ? payload.blockers : [];
    const openQuestions = Array.isArray(payload.open_questions) ? payload.open_questions : [];
    if (computed === 'BLOCKED' && blockers.length === 0) {
      problems.push(`${at} qualification_state recomputes to BLOCKED but payload.blockers is empty`);
    }
    if (computed !== 'BLOCKED' && blockers.length > 0) {
      problems.push(`${at} qualification_state recomputes to "${computed}" but payload.blockers is non-empty; blockers is closed to BLOCKED`);
    }
    if (computed === 'UNVERIFIED' && openQuestions.length === 0) {
      problems.push(`${at} qualification_state recomputes to UNVERIFIED but payload.open_questions is empty`);
    }
    if (computed !== 'UNVERIFIED' && openQuestions.length > 0) {
      problems.push(`${at} qualification_state recomputes to "${computed}" but payload.open_questions is non-empty; open_questions is closed to UNVERIFIED`);
    }
  }

  return problems;
}
