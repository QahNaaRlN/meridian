// ---------------------------------------------------------------------------
// execution state model: the one checkable implementation
// ---------------------------------------------------------------------------
// registries/operating-model/execution-state.schema.json is the COMPLETE schema
// for the state of ONE execution run (record_type: execution-run): a record
// declares it in its own $schema, so a consumer that follows the declaration
// validates the whole record — the reused scoped-record envelope AND the
// specialised body — in one pass. Run-state DATA is Instance, like the task
// specification and the instruction source registry — the Kernel ships the
// schema, the product-neutral fixtures and this module. scripts/kernel-validate.mjs
// (the `execution-state-model` section) and test/execution-state.test.mjs both
// call the functions here so the gate and the standalone set cannot drift.
//
// Nothing in this module reads process state, touches the filesystem or exits.
// It takes a parsed record and the two schemas, and returns a flat list of
// problem strings — empty means valid.
//
// The managed entity is the RUN, not the conversational "task". One task
// specification may drive several runs; each run carries its own stable id and
// its own human-readable Russian title, and references EXACTLY ONE task
// specification by a portable reference — it never absorbs or duplicates the
// specification body inside its payload.
//
// Boundaries this module enforces that the JSON Schema subset cannot:
//   - the record's $schema declaration is a portable relative reference that
//     RESOLVES, within the Meridian namespace, to execution-state.schema.json —
//     not merely a matching basename, and not the bare record envelope. The
//     resolution and the rooted-path rules are REUSED from
//     scripts/lib/task-specification.mjs (resolveSchemaRef, nonPortableReason):
//     both record bases sit at namespace depth two, so a portable
//     "../../<dir>/<file>" reference resolves to the same logical address, and
//     this package adds no divergent copy of that address math or of the
//     absolute-path rules;
//   - the canonical scoped-record envelope is validated SEPARATELY and is never
//     dropped, even though the specialised schema re-states its shape;
//   - a run record lives only in scope.type run-state; scope.id identifies the
//     run and scope.workspace_id is mandatory. project-workspace,
//     repository-scope, built-in-methodology, user-profile and
//     organization-profile are rejected with the rationale;
//   - the independent axes are all present and mutually consistent: a non-empty
//     blockers list agrees with work_status blocked and vice versa; waiting_human
//     names a concrete next action; a completed or cancelled run carries no
//     executable next step; next_action and next_gate are stated explicitly
//     (an action, or an explicit null where there is no continuation), never
//     left to a default;
//   - the transition history is ordered by an explicit, strictly increasing
//     sequence; its first record states the initial state explicitly; each
//     later record continues the previous state with no break; the last record
//     matches the current lifecycle_stage, work_status and scope_revision; the
//     ordinal and the scope_revision never repeat or decrease; a backward move
//     or a skipped stage is never accepted silently, and a transition after a
//     completed or cancelled state needs an explicit reopen basis; every
//     transition states a reason;
//   - run state does not name a role, a supervision mode, a communication mode,
//     reviewer independence, a full context manifest or a full evidence /
//     handoff contract — those belong to later packages; current_actor stays a
//     portable opaque participant reference and grants no role or authority;
//   - the reference strings (task_specification_ref, current_actor,
//     resolved_norms, completed_checks, the envelope refs, the transition
//     reasons and rationales) are portable: no rooted machine path, no "~/"
//     reference, no drive-letter path, no backslash path, no file:// URL;
//   - the human-readable name is stated in Russian for the reader.
// ---------------------------------------------------------------------------
import { validate } from './json-schema.mjs';
import { nonPortableReason, resolveSchemaRef } from './task-specification.mjs';

// Reused unchanged from the task specification contract: the same Meridian-
// namespace address math and the same rooted-path detection, so this package
// carries no divergent second copy of either rule.
export { nonPortableReason, resolveSchemaRef };

export const RECORD_TYPE = 'execution-run';

// The closed, ordered lifecycle. The order is the sequence the plan declares
// (meridian-operating-upgrade-plan.md §7.1):
//   приём постановки → классификация → определение норм → планирование →
//   исполнение → проверка → приёмка → интеграция → развёртывание →
//   наблюдение → завершение
export const LIFECYCLE_STAGES = [
  'intake', 'classification', 'norm_resolution', 'planning', 'execution',
  'verification', 'acceptance', 'integration', 'deployment', 'observation',
  'completion',
];

// The closed work-status pool.
export const WORK_STATUSES = [
  'planned', 'ready', 'active', 'waiting_human', 'blocked', 'failed',
  'completed', 'cancelled',
];

// completed and cancelled are terminal: a transition that follows one of them
// needs an explicitly allowed reopen basis.
export const TERMINAL_STATUSES = ['completed', 'cancelled'];

// The canonical LOGICAL resolution base for an execution-run record within the
// Meridian namespace. It is a logical address — not a filesystem directory, a
// repository or a storage mechanism. Resolution itself is delegated to
// task-specification.mjs's resolveSchemaRef (see the note above); this constant
// documents the conceptual base and is not re-implemented here.
export const CANONICAL_RECORD_BASE = 'records/execution-run';
export const SCHEMA_NAMESPACE_DIR = 'registries/operating-model';
export const EXPECTED_SCHEMA_BASENAME = 'execution-state.schema.json';
export const ENVELOPE_SCHEMA_BASENAME = 'scoped-record.schema.json';
export const EXPECTED_SCHEMA_REF = `${SCHEMA_NAMESPACE_DIR}/${EXPECTED_SCHEMA_BASENAME}`;
export const ENVELOPE_SCHEMA_REF = `${SCHEMA_NAMESPACE_DIR}/${ENVELOPE_SCHEMA_BASENAME}`;

// The one area a run record may occupy, and why each other area is excluded.
export const ALLOWED_SCOPE_TYPE = 'run-state';
const SCOPE_REJECTION_REASON = {
  'built-in-methodology': 'built-in-methodology is Kernel methodology, not a place for one run\'s episodic state',
  'user-profile': 'user-profile holds a user\'s rules and settings, not run state',
  'organization-profile': 'organization-profile holds an organisation\'s rules and settings, not run state',
  'project-workspace': 'project-workspace holds the project\'s goals and decisions; a run\'s episodic state is scoped to run-state',
  'repository-scope': 'repository-scope holds facts true for one repository; a run may span repositories and is scoped to run-state',
};

// The independent axes of the minimal current state. One universal "status"
// field standing in for several of them is forbidden.
export const REQUIRED_AXES = [
  'lifecycle_stage', 'work_status', 'scope_revision', 'current_actor',
  'resolved_norms', 'completed_checks', 'blockers', 'next_action', 'next_gate',
  'transition_history',
];

// Fields that belong to LATER packages, or that would collapse the independent
// axes into one. Named here so a leak produces a pointed message, not just
// "additional property not allowed".
export const FORBIDDEN_PAYLOAD_FIELDS = [
  // the single universal status the model forbids
  'status',
  // role-and-human-control (package 5)
  'role', 'roles', 'assigned_role', 'reviewer_independence',
  'human_authority', 'supervision_mode', 'hic', 'hitl', 'hotl',
  'communication_mode', 'owner_relayed',
  // bounded-context-manifest (package 6)
  'context_manifest', 'bounded_context',
  // evidence-and-handoff-contract (package 7)
  'evidence', 'evidence_contract', 'handoff', 'worktree_state',
  // meridian-field-evaluation (package 8)
  'field_metrics', 'evaluation_metrics',
  // the statement of the work is not absorbed into the run
  'goal', 'initial_state', 'target_model', 'task_pattern', 'constraints',
  'acceptance_criteria', 'task_specification',
  // a full command log or transcript is not run state
  'command_log', 'transcript', 'messages', 'chat_history',
];

const SEMANTIC_ID_RE = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;
const isObject = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);
const blank = (s) => typeof s !== 'string' || s.trim() === '';
const hasCyrillic = (s) => typeof s === 'string' && /[Ѐ-ӿ]/.test(s);
const isPositiveInt = (n) => typeof n === 'number' && Number.isInteger(n) && n > 0;

export function stageIndex(stage) { return LIFECYCLE_STAGES.indexOf(stage); }

// The whole composition pipeline for one execution-run record: the canonical
// record envelope (reused, always run), the complete specialised schema (which
// a $schema-follower would also use), then the cross-cutting rules the JSON
// Schema subset cannot state.
export function evaluateExecutionState(doc, { recordSchema, envelopeSchema } = {}) {
  const problems = [];

  // 1. the canonical scoped-record envelope, validated separately and always.
  const envErrs = [];
  try { validate(doc, envelopeSchema, envelopeSchema, '', envErrs); }
  catch (e) { return [`record envelope schema could not be applied: ${e.message}`]; }
  problems.push(...envErrs.map((m) => `envelope ${m}`));

  // 2. the complete specialised schema — the same one a record declares.
  const bodyErrs = [];
  try { validate(doc, recordSchema, recordSchema, '', bodyErrs); }
  catch (e) { return [`execution-state schema could not be applied: ${e.message}`]; }
  problems.push(...bodyErrs);

  if (!isObject(doc)) {
    return problems.length ? problems : ['the execution run record is not an object'];
  }

  const id = typeof doc.id === 'string' && doc.id !== '' ? doc.id : '(no id)';

  // 3. the record's $schema declaration must be a portable relative reference
  //    that RESOLVES, within the Meridian namespace, to the specialised schema
  //    — not merely carry the right basename, and not the bare record envelope.
  const declared = doc.$schema;
  if (typeof declared !== 'string' || declared === '') {
    problems.push(`execution run "${id}" declares no $schema; a record names ${EXPECTED_SCHEMA_BASENAME} so a consumer validates the whole contract — id, scope, origin, authority and body — not only the envelope`);
  } else {
    const portability = nonPortableReason(declared);
    if (portability) {
      problems.push(`execution run "${id}" $schema "${declared}" is not portable (${portability}); the schema is named by a relative reference resolved inside the Meridian namespace (canonical logical base ${CANONICAL_RECORD_BASE})`);
    } else {
      const resolved = resolveSchemaRef(declared);
      if (resolved === ENVELOPE_SCHEMA_REF) {
        problems.push(`execution run "${id}" $schema "${declared}" resolves to the record envelope (${ENVELOPE_SCHEMA_BASENAME}); it must name ${EXPECTED_SCHEMA_BASENAME}, which composes the envelope with the body`);
      } else if (resolved !== EXPECTED_SCHEMA_REF) {
        problems.push(`execution run "${id}" $schema "${declared}" does not resolve to the logical address ${EXPECTED_SCHEMA_REF} within the Meridian namespace; the specialised schema is named by a portable relative reference (a bare basename, a missing namespace segment, a wrong segment and a reference climbing out of the namespace all resolve elsewhere)`);
      }
    }
  }

  // 4. identity and the human-readable Russian name come from the envelope.
  if (doc.record_type !== RECORD_TYPE) {
    problems.push(`execution run "${id}" declares record_type "${doc.record_type}", not "${RECORD_TYPE}"; the record type names the run and does not open a second envelope`);
  }
  if (typeof doc.id !== 'string' || !SEMANTIC_ID_RE.test(doc.id)) {
    problems.push(`execution run "${id}" has no stable semantic id on the record envelope`);
  }
  if (typeof doc.title !== 'string' || doc.title.trim() === '') {
    problems.push(`execution run "${id}" has no human-readable title`);
  } else if (!hasCyrillic(doc.title)) {
    problems.push(`execution run "${id}" title "${doc.title}" carries no Russian (Cyrillic) text; the run name is stated in Russian for the human reader`);
  }
  const titleReason = nonPortableReason(doc.title);
  if (titleReason) problems.push(`execution run "${id}" title contains ${titleReason}`);

  // 5. a run record lives only in run-state; scope.id identifies the run and
  //    scope.workspace_id is mandatory. The physical directory, the Instance
  //    repository and the storage mechanism are not the area.
  const scope = isObject(doc.scope) ? doc.scope : {};
  if (typeof scope.type === 'string' && scope.type !== ALLOWED_SCOPE_TYPE) {
    const why = SCOPE_REJECTION_REASON[scope.type] || 'it is not run-state';
    problems.push(`execution run "${id}" is scoped to "${scope.type}"; a run record lives in run-state only — ${why}`);
  }
  if (scope.type === ALLOWED_SCOPE_TYPE) {
    if (typeof scope.id !== 'string' || !SEMANTIC_ID_RE.test(scope.id)) {
      problems.push(`execution run "${id}" run-state scope carries no stable scope.id identifying the run`);
    }
    if (typeof scope.workspace_id !== 'string' || !SEMANTIC_ID_RE.test(scope.workspace_id)) {
      problems.push(`execution run "${id}" run-state scope carries no workspace_id; a run belongs to a project workspace (workspace-scope-model.md §1)`);
    }
  }

  const origin = isObject(doc.origin) ? doc.origin : {};
  if (origin.kind === 'built-in') {
    problems.push(`execution run "${id}" declares origin.kind "built-in"; a run is started in a workspace, not shipped with the methodology`);
  }
  const authority = isObject(doc.authority) ? doc.authority : {};

  // 5a. the envelope reference strings travel inside the portable record too.
  for (const [obj, field, label] of [
    [origin, 'source_ref', 'origin.source_ref'],
    [authority, 'authority_ref', 'authority.authority_ref'],
    [authority, 'decision_ref', 'authority.decision_ref'],
  ]) {
    const val = obj[field];
    if (typeof val === 'string' && val !== '') {
      const r = nonPortableReason(val);
      if (r) problems.push(`execution run "${id}" ${label} contains ${r}; a run record is portable and carries no rooted machine path`);
    }
  }

  const payload = isObject(doc.payload) ? doc.payload : {};

  // 6. every required axis is present. A missing axis is a stated gap, never a
  //    silent empty list, null or default.
  for (const axis of REQUIRED_AXES) {
    if (!(axis in payload)) {
      problems.push(`execution run "${id}" states no ${axis}; it is a required independent axis with no default — a missing axis is not an empty list, null or a default value`);
    }
  }

  // 6a. a leaked next-package field, or one universal status axis.
  for (const f of FORBIDDEN_PAYLOAD_FIELDS) {
    if (f in payload) {
      const why = f === 'status'
        ? 'the model uses independent axes (lifecycle_stage, work_status, scope_revision, …), not one universal "status"'
        : 'a role, supervision mode, communication mode, context manifest, full evidence/handoff contract or the statement of the work belongs to a later package, not to run state';
      problems.push(`execution run "${id}" payload carries "${f}"; ${why}`);
    }
  }
  if ('record_type' in payload) {
    problems.push(`execution run "${id}" repeats record_type inside the payload; the record type is declared once, on the envelope`);
  }

  // 7. the run references exactly one task specification, portably, and does
  //    not absorb or duplicate its body.
  const tsr = payload.task_specification_ref;
  if (!('task_specification_ref' in payload)) {
    problems.push(`execution run "${id}" names no task_specification_ref; a run references exactly one task specification`);
  } else if (isObject(tsr) || Array.isArray(tsr)) {
    problems.push(`execution run "${id}" task_specification_ref is not a plain reference; the run references the specification, it does not embed or duplicate it`);
  } else if (blank(tsr)) {
    problems.push(`execution run "${id}" task_specification_ref is empty or whitespace-only`);
  } else {
    const r = nonPortableReason(tsr);
    if (r) problems.push(`execution run "${id}" task_specification_ref contains ${r}; the reference is portable and is not an absolute machine path`);
  }

  const workStatus = payload.work_status;
  const lifecycleStage = payload.lifecycle_stage;

  // 8. axis consistency.
  const blockers = Array.isArray(payload.blockers) ? payload.blockers : null;
  if (blockers) {
    const seenBlockerId = new Set();
    blockers.forEach((b, i) => {
      const bo = isObject(b) ? b : {};
      const bid = typeof bo.id === 'string' ? bo.id : null;
      const at = bid || `#${i}`;
      if (!bid || !SEMANTIC_ID_RE.test(bid)) {
        problems.push(`execution run "${id}" blocker ${at} has no stable id`);
      } else {
        if (seenBlockerId.has(bid)) problems.push(`execution run "${id}" blocker id "${bid}" is used more than once`);
        seenBlockerId.add(bid);
      }
      if (blank(bo.description)) problems.push(`execution run "${id}" blocker ${at} has no description`);
      if (blank(bo.resumption_condition)) {
        problems.push(`execution run "${id}" blocker ${at} has no verifiable resumption condition`);
      }
      for (const s of [bo.description, bo.resumption_condition]) {
        const r = nonPortableReason(s);
        if (r) problems.push(`execution run "${id}" blocker ${at} contains ${r}`);
      }
    });
    if (blockers.length > 0 && workStatus !== 'blocked') {
      problems.push(`execution run "${id}" carries ${blockers.length} blocker(s) but work_status is "${workStatus}"; a non-empty blocker agrees with work_status "blocked" (waiting_human and blocked are not interchangeable)`);
    }
    if (blockers.length === 0 && workStatus === 'blocked') {
      problems.push(`execution run "${id}" work_status is "blocked" with no blocker; "blocked" means continuation is impossible until a stated external condition changes`);
    }
  }

  // next_action / next_gate: stated explicitly as an action or an explicit
  // null; a completed or cancelled run has neither; waiting_human names one.
  for (const f of ['next_action', 'next_gate']) {
    if (f in payload && payload[f] !== null && blank(payload[f])) {
      problems.push(`execution run "${id}" ${f} is present but empty; state an action, or null where there is no continuation`);
    }
  }
  if (TERMINAL_STATUSES.includes(workStatus)) {
    for (const f of ['next_action', 'next_gate']) {
      if (payload[f] !== null && payload[f] !== undefined) {
        problems.push(`execution run "${id}" work_status is "${workStatus}" but ${f} carries an executable value "${payload[f]}"; a completed or cancelled run does not silently carry a next executable step`);
      }
    }
  }
  if (workStatus === 'waiting_human' && blank(payload.next_action)) {
    problems.push(`execution run "${id}" work_status is "waiting_human" but next_action names no concrete human action; "waiting_human" means a specific required human action is known`);
  }

  // resolved_norms and completed_checks: unique portable references.
  for (const f of ['resolved_norms', 'completed_checks']) {
    const arr = Array.isArray(payload[f]) ? payload[f] : null;
    if (!arr) continue;
    const seen = new Set();
    arr.forEach((ref, i) => {
      if (blank(ref)) {
        problems.push(`execution run "${id}" ${f}[${i}] is empty or whitespace-only`);
        return;
      }
      const r = nonPortableReason(ref);
      if (r) problems.push(`execution run "${id}" ${f}[${i}] contains ${r}`);
      const key = ref.trim();
      if (seen.has(key)) problems.push(`execution run "${id}" ${f} repeats reference "${ref}"`);
      seen.add(key);
    });
  }

  // current_actor: a portable opaque participant reference.
  if (typeof payload.current_actor === 'string' && payload.current_actor !== '') {
    const r = nonPortableReason(payload.current_actor);
    if (r) problems.push(`execution run "${id}" current_actor contains ${r}; the actor is a portable opaque reference and grants no role or authority`);
  }

  // scope_revision on the payload.
  if ('scope_revision' in payload && !isPositiveInt(payload.scope_revision)) {
    problems.push(`execution run "${id}" scope_revision "${payload.scope_revision}" is not a positive integer revision of the run's resolved scope`);
  }

  // 9. the transition history.
  const history = Array.isArray(payload.transition_history) ? payload.transition_history : null;
  if (!history) {
    if ('transition_history' in payload) {
      problems.push(`execution run "${id}" transition_history is not an ordered list`);
    }
  } else if (history.length === 0) {
    problems.push(`execution run "${id}" transition_history is empty; the history is mandatory and its first record states the initial state`);
  } else {
    let prevSeq = null;
    let prevRevision = null;
    history.forEach((tr, i) => {
      const t = isObject(tr) ? tr : {};
      const at = `transition #${i}`;

      // reason and rationales are portable prose.
      for (const [val, label] of [
        [t.reason, 'reason'], [t.backward_rationale, 'backward_rationale'],
        [t.reopen_rationale, 'reopen_rationale'],
      ]) {
        if (val === undefined) continue;
        if (label === 'reason' && blank(val)) problems.push(`execution run "${id}" ${at} states no reason; every transition states why it happened`);
        const r = nonPortableReason(val);
        if (r) problems.push(`execution run "${id}" ${at} ${label} contains ${r}`);
      }
      if (!('reason' in t)) problems.push(`execution run "${id}" ${at} states no reason; every transition states why it happened`);

      // the ordinal: strictly increasing, never repeated, never decreasing.
      if (!isPositiveInt(t.sequence)) {
        problems.push(`execution run "${id}" ${at} has no positive integer sequence ordinal; the history is ordered by an explicit sequence, not by object or file order`);
      } else if (prevSeq !== null) {
        if (t.sequence === prevSeq) problems.push(`execution run "${id}" ${at} repeats sequence ordinal ${t.sequence}; ordinals do not repeat`);
        else if (t.sequence < prevSeq) problems.push(`execution run "${id}" ${at} sequence ordinal ${t.sequence} is below the previous ${prevSeq}; ordinals do not decrease`);
      }
      if (isPositiveInt(t.sequence)) prevSeq = t.sequence;

      // per-transition scope_revision: positive, non-decreasing.
      if (!isPositiveInt(t.scope_revision)) {
        problems.push(`execution run "${id}" ${at} has no positive integer scope_revision`);
      } else {
        if (prevRevision !== null && t.scope_revision < prevRevision) {
          problems.push(`execution run "${id}" ${at} scope_revision ${t.scope_revision} is below the previous ${prevRevision}; the scope revision does not decrease`);
        }
        prevRevision = t.scope_revision;
      }

      // the chain: first record states the initial state; each later record
      // continues the previous one with no break.
      if (i === 0) {
        if (t.from_stage != null || t.from_status != null) {
          problems.push(`execution run "${id}" ${at} is the first record but names a prior from_stage/from_status; the first record states the initial state (from_stage and from_status are null)`);
        }
      } else {
        const prev = isObject(history[i - 1]) ? history[i - 1] : {};
        if (t.from_stage !== prev.to_stage) {
          problems.push(`execution run "${id}" ${at} from_stage "${t.from_stage}" does not continue the previous to_stage "${prev.to_stage}"; the history has no break`);
        }
        if (t.from_status !== prev.to_status) {
          problems.push(`execution run "${id}" ${at} from_status "${t.from_status}" does not continue the previous to_status "${prev.to_status}"; the history has no break`);
        }
      }

      // a backward move or a skipped stage is never silent.
      const fromIdx = stageIndex(t.from_stage);
      const toIdx = stageIndex(t.to_stage);
      if (i > 0 && fromIdx >= 0 && toIdx >= 0) {
        if (toIdx < fromIdx && blank(t.backward_rationale)) {
          problems.push(`execution run "${id}" ${at} moves back from "${t.from_stage}" to "${t.to_stage}" with no backward_rationale; a backward lifecycle move is not accepted silently`);
        }
        if (toIdx > fromIdx + 1) {
          const covered = new Set(
            (Array.isArray(t.skipped_stages) ? t.skipped_stages : [])
              .filter((s) => isObject(s) && !blank(s.rationale))
              .map((s) => s.stage),
          );
          for (let idx = fromIdx + 1; idx < toIdx; idx++) {
            const skipped = LIFECYCLE_STAGES[idx];
            if (!covered.has(skipped)) {
              problems.push(`execution run "${id}" ${at} jumps over "${skipped}" with no explicit inapplicability basis; a skipped stage is stated, not silently treated as passed`);
            }
          }
        }
      }
      for (const s of (Array.isArray(t.skipped_stages) ? t.skipped_stages : [])) {
        if (!isObject(s)) continue;
        if (stageIndex(s.stage) < 0) problems.push(`execution run "${id}" ${at} skipped_stages names unknown stage "${s.stage}"`);
        if (blank(s.rationale)) problems.push(`execution run "${id}" ${at} skipped_stages entry for "${s.stage}" has no rationale`);
        const r = nonPortableReason(s.rationale);
        if (r) problems.push(`execution run "${id}" ${at} skipped_stages rationale contains ${r}`);
      }

      // a transition after completed or cancelled needs an explicit reopen
      // basis — checked against the IMMEDIATELY PREVIOUS history record, not a
      // cumulative "a terminal was ever seen" flag. reopen_rationale is owed
      // only by the transition that directly follows a record whose to_status
      // is completed or cancelled. Once that resumption transition has carried
      // its reopen_rationale, an ordinary transition out of the now
      // non-terminal state does not repeat it; but if the resumption itself
      // lands on a terminal state again, the transition that follows it owes a
      // fresh reopen_rationale.
      const prevTerminal = i > 0
        && TERMINAL_STATUSES.includes((isObject(history[i - 1]) ? history[i - 1] : {}).to_status);
      if (prevTerminal && blank(t.reopen_rationale)) {
        problems.push(`execution run "${id}" ${at} follows a completed or cancelled state with no reopen_rationale; a new transition after a terminal state is rejected without an explicitly allowed reopen basis`);
      }
    });

    // the last record matches the current axes.
    const last = isObject(history[history.length - 1]) ? history[history.length - 1] : {};
    if (last.to_stage !== lifecycleStage) {
      problems.push(`execution run "${id}" last transition to_stage "${last.to_stage}" does not match the current lifecycle_stage "${lifecycleStage}"`);
    }
    if (last.to_status !== workStatus) {
      problems.push(`execution run "${id}" last transition to_status "${last.to_status}" does not match the current work_status "${workStatus}"`);
    }
    if (isPositiveInt(last.scope_revision) && isPositiveInt(payload.scope_revision) && last.scope_revision !== payload.scope_revision) {
      problems.push(`execution run "${id}" last transition scope_revision ${last.scope_revision} does not match the current scope_revision ${payload.scope_revision}`);
    }
  }

  return problems;
}
