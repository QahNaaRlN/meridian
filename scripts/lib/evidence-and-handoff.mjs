// ---------------------------------------------------------------------------
// evidence and handoff: the one checkable implementation
// ---------------------------------------------------------------------------
// registries/operating-model/evidence-and-handoff.schema.json is the COMPLETE
// schema for the portable handoff of the STATE and RESULT of ONE execution run
// (record_type: evidence-and-handoff): a record declares it in its own $schema,
// so a consumer that follows the declaration validates the whole record — the
// reused scoped-record envelope AND the specialised body — in one pass.
// Handoff DATA is Instance data (later the working-data area) — the Kernel ships
// the schema, the product-neutral fixtures and this module.
// scripts/kernel-validate.mjs (the `evidence-and-handoff-contract` section) and
// test/evidence-and-handoff.test.mjs both call the functions here so the gate
// and the standalone set cannot drift.
//
// Nothing in this module reads process state, touches the filesystem or exits;
// the only Node core it uses is a SHA-256 hash over caller-supplied bytes. It
// takes a parsed record, the two schemas and an external record resolver, and
// returns a flat list of problem strings — empty means valid.
//
// The handoff relates to EXACTLY ONE run, and the link is DETERMINISTIC, not a
// substring heuristic:
//   - execution_run_ref, task_specification_ref, human_control_ref and
//     context_manifest_ref are CLOSED structured pinned references —
//     { record_type, id, reference, run_id?, revision?, sha256? } — never a
//     plain string, never an embedded body;
//   - scope.id equals execution_run_ref.id by exact string comparison;
//   - human_control_ref.run_id and context_manifest_ref.run_id equal
//     execution_run_ref.id; task_specification_ref carries no run_id;
//   - the four pinned references are RESOLVED through a boundary the handoff
//     does not control (a resolver the caller supplies), and the handoff's own
//     axes — the task specification it names, the next step and gate, the
//     blocker id set, the passed-check set — are checked against the state the
//     RESOLVED execution-run record carries. With no resolver the handoff fails
//     closed.
//
// The CLOSED, fail-closed exact-revision rule (classifyRevision) is REUSED from
// scripts/lib/context-manifest.mjs, so this package carries no divergent second
// copy of it. The same rule is applied to every pinned reference and to the
// per-repository source_state / result_state revisions.
//
// Boundaries this module enforces that the JSON Schema subset cannot:
//   - the record's $schema declaration is a portable relative reference that
//     RESOLVES, within the Meridian namespace, to evidence-and-handoff.schema.json
//     — not merely a matching basename, and not the bare record envelope
//     (resolveSchemaRef / nonPortableReason reused from
//     scripts/lib/task-specification.mjs via scripts/lib/context-manifest.mjs);
//   - the canonical scoped-record envelope is validated SEPARATELY and is never
//     dropped;
//   - the handoff lives only in scope.type run-state; scope.id identifies the
//     run and scope.workspace_id is mandatory; every other area, and
//     origin.kind built-in, are rejected;
//   - claimed results, verifiable assertions and evidence are three SEPARATE
//     id-linked lists: an assertion names its one claimed_result_id, an
//     evidence entry names the assertion ids it covers. Every evidence entry is
//     itself PINNED (an exact revision or a SHA-256 digest, the reused closed
//     rule) and RESOLVED through the boundary to a closed 'evidence-result'
//     transformer response carrying observed_result AND the SUBJECT it is about
//     — covers (the assertion ids this evidence bears on) and/or check_ref (the
//     mandatory check it is the recorded result of). An assertion is verified
//     ONLY when a covering entry resolves cleanly with observed_result
//     "confirmed" AND the transformer confirms it bears on that assertion: a
//     covers link is never transferred to a foreign assertion by the handoff's
//     own say-so. A specialised-evidence-record entry's specialised_contract
//     and recorded_verdict are confirmed by the resolved evidence-result by
//     EXACT match — the handoff's own words are not proof — and no full
//     specialised body is embedded;
//   - a claimed result's status is derived on BOTH sides: established iff it
//     carries at least one assertion and every linked assertion is verified;
//     not_established otherwise — a not_established status over a fully verified
//     assertion set is rejected;
//   - passed / failed / unable are three distinct check statuses; passed and
//     failed each name a result_evidence_id whose resolved evidence-result has
//     observed_result "confirmed" / "contradicted" AND records THIS check's
//     check_ref (a result is not moved between checks); unable forbids result
//     evidence and needs a verifiable reason. completed_checks of the resolved
//     run confirms the fact a check RAN, not the verdict: a passed OR a failed
//     check's check_ref is there, only an unable one is absent;
//   - every acceptance criterion the resolved task specification declares is
//     addressed by a verifiable assertion or explicitly uncovered with a
//     reason; an unknown, duplicate or missing criterion id is rejected, the
//     resolved specification's acceptance_criteria set is required NON-EMPTY,
//     and the FULL mandatory_checks set is closed against the resolved
//     specification's own minimal machine list — dropping any one is rejected;
//   - source_state and result_state are pinned per repository by the closed
//     exact-revision rule and pin the SAME set of repositories; every
//     repository with a changed path appears in both with a pin that differs;
//   - changed paths, external effects, deviations, open gaps, required owner
//     decisions and blockers are separate lists with no duplicate id;
//   - the worktree disposition uses the closed set not_created / removed /
//     retained (retained requires a reason, a responsible party and a
//     verifiable cleanup condition), aligned with version-control-flow.md §5.4;
//   - outcome.status "complete" is the WHOLE run finished: the resolved run is
//     work_status "completed", next_step.action and next_step.gate are null,
//     open_gaps is empty, and every acceptance criterion is covered by a
//     verified assertion;
//   - an unbounded material dump, a full specialised-evidence body or the body
//     of a referenced record (task specification, run, human control, context
//     manifest) or a field-evaluation field is rejected;
//   - every reference and prose string is portable;
//   - the human-readable name is stated in Russian for the reader.
// ---------------------------------------------------------------------------
import { createHash } from 'node:crypto';
import { validate } from './json-schema.mjs';
import {
  classifyRevision,
  isFloatingRevision,
  nonPortableReason,
  resolveSchemaRef,
  LIFECYCLE_STAGES,
  WORK_STATUSES,
  TERMINAL_STATUSES,
  REQUIRED_RESOLVED_STATE_FIELDS,
  RESOLVED_ENTRY_COMMON_KEYS,
  makeRecordResolver,
} from './context-manifest.mjs';

// Reused unchanged so this package carries no divergent second copy: the
// Meridian-namespace address math, the rooted-path detection, the closed
// exact-revision rule, the closed lifecycle and work-status pools, and the
// closed transformer-response key sets.
export {
  classifyRevision, isFloatingRevision, nonPortableReason, resolveSchemaRef,
  LIFECYCLE_STAGES, WORK_STATUSES, TERMINAL_STATUSES,
  REQUIRED_RESOLVED_STATE_FIELDS, RESOLVED_ENTRY_COMMON_KEYS, makeRecordResolver,
};

export const RECORD_TYPE = 'evidence-and-handoff';

// The canonical LOGICAL resolution base for an evidence-and-handoff record
// within the Meridian namespace. It is a logical address — not a filesystem
// directory, a repository or a storage mechanism. Resolution itself is
// delegated to resolveSchemaRef (see the note above); this constant documents
// the conceptual base and is not re-implemented here.
export const CANONICAL_RECORD_BASE = 'records/evidence-and-handoff';
export const SCHEMA_NAMESPACE_DIR = 'registries/operating-model';
export const EXPECTED_SCHEMA_BASENAME = 'evidence-and-handoff.schema.json';
export const ENVELOPE_SCHEMA_BASENAME = 'scoped-record.schema.json';
export const EXPECTED_SCHEMA_REF = `${SCHEMA_NAMESPACE_DIR}/${EXPECTED_SCHEMA_BASENAME}`;
export const ENVELOPE_SCHEMA_REF = `${SCHEMA_NAMESPACE_DIR}/${ENVELOPE_SCHEMA_BASENAME}`;

// The one area a handoff may occupy, and why each other area is excluded.
export const ALLOWED_SCOPE_TYPE = 'run-state';
const SCOPE_REJECTION_REASON = {
  'built-in-methodology': 'built-in-methodology is Kernel methodology, not a place for one run\'s result handoff',
  'user-profile': 'user-profile holds a user\'s rules and settings, not a run\'s result handoff',
  'organization-profile': 'organization-profile holds an organisation\'s rules and settings, not a run\'s result handoff',
  'project-workspace': 'project-workspace holds the project\'s goals and decisions; a run\'s result handoff is scoped to run-state',
  'repository-scope': 'repository-scope holds facts true for one repository; a run may span repositories and its handoff is scoped to run-state',
};

// Which record_type each pinned-reference slot must carry, and whether that slot
// carries a run_id (the run it belongs to).
export const PINNED_REF_SLOTS = {
  execution_run_ref: { recordType: 'execution-run', runId: 'self' },
  task_specification_ref: { recordType: 'task-specification', runId: 'forbidden' },
  human_control_ref: { recordType: 'run-human-control', runId: 'required' },
  context_manifest_ref: { recordType: 'context-manifest', runId: 'required' },
};

// The CLOSED key set of a resolved entry — the transformer's confirmed view of
// ONE edition of ONE record. The entry is closed to EXACTLY the common part
// (reused from context-manifest.mjs) plus the slot-specific field(s):
//   - execution-run    → resolved_state
//   - run-human-control → linked_run_ref
//   - context-manifest  → linked_run_ref
//   - task-specification→ acceptance_criteria (the criterion ids, no spec body)
//                         AND mandatory_checks (the mandatory-check references, a
//                         minimal machine list, no spec body) so the FULL set of
//                         mandatory checks is closed against the specification
//   - evidence-result   → observed_result, the SUBJECT it confirms — covers (the
//                         assertion ids this evidence bears on) and/or check_ref
//                         (the mandatory check this is the recorded result OF) —
//                         and, for a specialised-evidence-record entry only,
//                         specialised_contract + recorded_verdict
export const RESOLVED_ENTRY_KEYS_BY_TYPE = {
  'execution-run': [...RESOLVED_ENTRY_COMMON_KEYS, 'resolved_state'],
  'run-human-control': [...RESOLVED_ENTRY_COMMON_KEYS, 'linked_run_ref'],
  'context-manifest': [...RESOLVED_ENTRY_COMMON_KEYS, 'linked_run_ref'],
  'task-specification': [...RESOLVED_ENTRY_COMMON_KEYS, 'acceptance_criteria', 'mandatory_checks'],
  'evidence-result': [...RESOLVED_ENTRY_COMMON_KEYS, 'observed_result', 'covers', 'check_ref', 'specialised_contract', 'recorded_verdict'],
};

// What the external boundary confirms an evidence artefact actually shows. For
// an assertion, 'confirmed' means the assertion holds; for a mandatory check's
// result evidence, 'confirmed' means the check passed and 'contradicted' means
// it failed.
export const OBSERVED_RESULTS = ['confirmed', 'contradicted', 'inconclusive'];

export const EVIDENCE_KINDS = [
  'check-run', 'observation', 'artifact-inspection', 'external-confirmation', 'specialised-evidence-record',
];
export const CHECK_STATUSES = ['passed', 'failed', 'unable'];
export const CLAIMED_RESULT_STATUSES = ['established', 'not_established'];
export const ASSERTION_STATUSES = ['verified', 'unverified'];
export const ACCEPTANCE_STATUSES = ['covered', 'uncovered'];
export const CHANGE_KINDS = ['added', 'modified', 'removed', 'renamed'];
export const DEVIATION_SEVERITIES = ['blocking', 'non_blocking'];
export const WORKTREE_STATES = ['not_created', 'removed', 'retained'];
export const RETAINED_ONLY_FIELDS = ['reason', 'responsible', 'cleanup_condition'];
export const OUTCOME_STATUSES = ['complete', 'blocked', 'handed_off_incomplete'];

// Fields that belong to a LATER package, to a specialised evidence contract, or
// that would turn the bounded handoff into an unbounded archive. Named here so a
// leak produces a pointed message, not just "additional property not allowed".
export const FORBIDDEN_PAYLOAD_FIELDS = [
  // an unbounded material dump is not a bounded handoff
  'file_contents', 'full_text', 'full_texts', 'raw_context', 'context_dump',
  'command_log', 'commands', 'transcript', 'messages', 'chat_history',
  'conversation', 'directory_dump', 'dir_listing', 'tree_dump', 'attachments',
  // the bodies of the referenced records are not absorbed
  'task_specification', 'execution_run', 'human_control', 'context_manifest',
  'run_state_checkpoint', 'transition_history', 'switch_history',
  'role_assignments', 'supervision_mode', 'human_authority',
  'authoritative_sources', 'applicable_norms',
  // specialised evidence contracts keep authority over their own verdict — a
  // full body is not embedded here
  'functional_parity_record', 'functional_parity_evidence', 'parity_comparison',
  'baseline', 'post_change_result', 'smoke_record', 'smoke_evidence',
  'regression_record', 'regression_evidence', 'preserved_contract',
  // meridian-field-evaluation (package 8)
  'field_metrics', 'evaluation_metrics', 'observation_log',
];

const SEMANTIC_ID_RE = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;
const SHA256_RE = /^[0-9a-fA-F]{64}$/;

const isObject = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);
const blank = (s) => typeof s !== 'string' || s.trim() === '';
const hasCyrillic = (s) => typeof s === 'string' && /[Ѐ-ӿ]/.test(s);
const isPositiveInt = (n) => typeof n === 'number' && Number.isInteger(n) && n > 0;
const sameStepValue = (a, b) => (a == null ? null : a) === (b == null ? null : b);
const isValidSha256 = (s) => typeof s === 'string' && SHA256_RE.test(s.trim());

// Is this { revision, sha256 } pair a sufficient pin? A sha256 digest always
// is; otherwise the revision must classify as 'exact' by the reused closed
// rule. Returns null when pinned, or a reason string when not.
function pinDefect(revision, sha256) {
  if (isValidSha256(sha256)) return null;
  const rc = classifyRevision(revision);
  if (rc === 'exact') return null;
  if (rc === 'floating') {
    return `revision ${JSON.stringify(revision)} is a branch or channel reference; a branch or channel is never a pin, whatever digits it carries`;
  }
  if (rc === 'abbrev-sha') {
    return `revision ${JSON.stringify(revision)} looks like an abbreviated or ambiguous Git SHA (not a full 40- or 64-hex object name); pin the full SHA or add a sha256 digest`;
  }
  if (rc === 'weak') {
    return `revision ${JSON.stringify(revision)} is not a recognised exact revision (a full Git SHA, a strict v?X.Y.Z tag or a SHA-256 digest); a provider-specific or unknown revision format requires a sha256 digest`;
  }
  return 'it is pinned by neither an exact revision nor a SHA-256 digest';
}

// Validate one pinned_ref slot. Pushes problems. `runId` is execution_run_ref.id
// (or null when it is not yet known).
function checkPinnedRef(problems, id, field, ref, runId) {
  const slot = PINNED_REF_SLOTS[field];
  if (!isObject(ref) || Array.isArray(ref)) {
    problems.push(`evidence and handoff "${id}" ${field} is not a structured pinned reference; it is a closed { record_type, id, reference, revision?/sha256? } object, never a plain string and never an embedded body`);
    return;
  }
  if (ref.record_type !== slot.recordType) {
    problems.push(`evidence and handoff "${id}" ${field} names record_type "${ref.record_type}", not "${slot.recordType}"`);
  }
  if (typeof ref.id !== 'string' || !SEMANTIC_ID_RE.test(ref.id)) {
    problems.push(`evidence and handoff "${id}" ${field} has no stable semantic id`);
  }
  if (blank(ref.reference)) {
    problems.push(`evidence and handoff "${id}" ${field} has no portable reference`);
  } else {
    const r = nonPortableReason(ref.reference);
    if (r) problems.push(`evidence and handoff "${id}" ${field} reference contains ${r}; the reference is portable and is not an absolute machine path`);
  }
  const defect = pinDefect(ref.revision, ref.sha256);
  if (defect) {
    problems.push(`evidence and handoff "${id}" ${field} is not pinned to an exact edition: ${defect}`);
  }
  if (typeof ref.revision === 'string' && ref.revision !== '') {
    const rr = nonPortableReason(ref.revision);
    if (rr) problems.push(`evidence and handoff "${id}" ${field} revision contains ${rr}`);
  }
  const hasRunId = typeof ref.run_id === 'string' && ref.run_id !== '';
  if (slot.runId === 'forbidden' && hasRunId) {
    problems.push(`evidence and handoff "${id}" ${field} carries run_id "${ref.run_id}"; a task specification is not run-scoped and names no run_id`);
  }
  if (slot.runId === 'required' && !hasRunId) {
    problems.push(`evidence and handoff "${id}" ${field} carries no run_id; a ${slot.recordType} record explicitly names the run it belongs to`);
  }
  if (slot.runId === 'self' && hasRunId && ref.run_id !== ref.id) {
    problems.push(`evidence and handoff "${id}" ${field} run_id "${ref.run_id}" is not the run's own id "${ref.id}"`);
  }
  if (slot.runId === 'required' && hasRunId && runId != null && ref.run_id !== runId) {
    problems.push(`evidence and handoff "${id}" ${field} run_id "${ref.run_id}" is not the referenced run "${runId}"; the ${slot.recordType} record must belong to the same run`);
  }
}

// Shared: a list of { id, ... } items — every id present, a semantic id, unique
// within the handoff, every listed prose string portable. Returns the set of
// seen ids.
function checkIdentifiedItems(problems, id, label, list, fields) {
  const seen = new Set();
  const arr = Array.isArray(list) ? list : [];
  arr.forEach((item, i) => {
    const it = isObject(item) ? item : {};
    const iid = typeof it.id === 'string' ? it.id : null;
    const at = iid || `#${i}`;
    if (!iid || !SEMANTIC_ID_RE.test(iid)) {
      problems.push(`evidence and handoff "${id}" ${label} entry ${at} has no stable semantic id`);
    } else {
      if (seen.has(iid)) problems.push(`evidence and handoff "${id}" ${label} id "${iid}" is used more than once`);
      seen.add(iid);
    }
    for (const f of fields) {
      if (blank(it[f])) {
        problems.push(`evidence and handoff "${id}" ${label} entry ${at} has no ${f}`);
      } else {
        const r = nonPortableReason(it[f]);
        if (r) problems.push(`evidence and handoff "${id}" ${label} entry ${at} ${f} contains ${r}`);
      }
    }
  });
  return seen;
}

// Shared: a list of unique portable reference strings.
function checkRefList(problems, id, label, list) {
  const seen = new Set();
  const arr = Array.isArray(list) ? list : [];
  arr.forEach((ref, i) => {
    if (blank(ref)) {
      problems.push(`evidence and handoff "${id}" ${label}[${i}] is empty or whitespace-only`);
      return;
    }
    const r = nonPortableReason(ref);
    if (r) problems.push(`evidence and handoff "${id}" ${label}[${i}] contains ${r}`);
    const key = ref.trim();
    if (seen.has(key)) problems.push(`evidence and handoff "${id}" ${label} repeats reference "${ref}"`);
    seen.add(key);
  });
  return seen;
}

// ---------------------------------------------------------------------------
// The external record-resolution boundary — the SAME closed contract as the
// bounded context manifest. The four pinned references are resolved through a
// resolver the caller supplies; the resolved entry is closed to EXACTLY the
// declared key set for its type, every value type- and value-checked, and a
// wrong type produces a closed-contract error BEFORE any later comparison.
// ---------------------------------------------------------------------------

// The SHA-256 the transformer confirms for a resolved edition.
function confirmedDigest(entry) {
  const cd = typeof entry.content_digest === 'string' && SHA256_RE.test(entry.content_digest)
    ? entry.content_digest : null;
  if (typeof entry.source_bytes === 'string') {
    const h = createHash('sha256').update(entry.source_bytes, 'utf8').digest('hex');
    if (cd && cd.toLowerCase() !== h.toLowerCase()) {
      return { digest: h, inconsistent: cd };
    }
    return { digest: h };
  }
  if (cd) return { digest: cd };
  return { digest: null };
}

// One resolved_state axis that must be an ARRAY OF UNIQUE STRINGS.
function checkResolvedStateArray(problems, id, field, axis, value, kind) {
  const at = `${field}: the resolved execution-run record's resolved_state.${axis}`;
  if (!Array.isArray(value)) {
    problems.push(`evidence and handoff "${id}" ${at} is ${value === null ? 'null' : typeof value}, not an array of ${kind === 'id' ? 'semantic-id' : 'portable-reference'} strings`);
    return;
  }
  const seen = new Set();
  value.forEach((el, i) => {
    if (typeof el !== 'string' || el.trim() === '') {
      problems.push(`evidence and handoff "${id}" ${at}[${i}] is not a non-empty string`);
      return;
    }
    const key = el.trim();
    if (seen.has(key)) problems.push(`evidence and handoff "${id}" ${at} repeats "${el}"`);
    seen.add(key);
    if (kind === 'id') {
      if (!SEMANTIC_ID_RE.test(el)) problems.push(`evidence and handoff "${id}" ${at}[${i}] "${el}" is not a stable semantic id`);
    } else {
      const r = nonPortableReason(el);
      if (r) problems.push(`evidence and handoff "${id}" ${at}[${i}] contains ${r}`);
    }
  });
}

// The CLOSED contract for an execution-run's resolved_state: an object with
// EXACTLY the declared axes, each type- and value-checked against the same
// closed pools the execution state model uses.
function checkResolvedState(problems, id, field, rs) {
  if (!isObject(rs)) {
    problems.push(`evidence and handoff "${id}" ${field}: the resolver returned an execution-run record with no resolved_state; the handoff axes cannot be checked against the run's own state`);
    return;
  }
  for (const f of REQUIRED_RESOLVED_STATE_FIELDS) {
    if (!(f in rs)) {
      problems.push(`evidence and handoff "${id}" ${field}: the resolved execution-run record's resolved_state is missing required field "${f}"`);
    }
  }
  for (const k of Object.keys(rs)) {
    if (!REQUIRED_RESOLVED_STATE_FIELDS.includes(k)) {
      problems.push(`evidence and handoff "${id}" ${field}: the resolved execution-run record's resolved_state carries an unknown field "${k}"; resolved_state is closed to its ${REQUIRED_RESOLVED_STATE_FIELDS.length} declared axes`);
    }
  }
  if ('task_specification_ref' in rs) {
    if (blank(rs.task_specification_ref)) {
      problems.push(`evidence and handoff "${id}" ${field}: the resolved execution-run record's resolved_state.task_specification_ref is not a non-empty string`);
    } else {
      const r = nonPortableReason(rs.task_specification_ref);
      if (r) problems.push(`evidence and handoff "${id}" ${field}: the resolved execution-run record's resolved_state.task_specification_ref contains ${r}`);
    }
  }
  if ('scope_revision' in rs && !isPositiveInt(rs.scope_revision)) {
    problems.push(`evidence and handoff "${id}" ${field}: the resolved execution-run record's resolved_state.scope_revision ${JSON.stringify(rs.scope_revision)} is not a positive integer`);
  }
  if ('lifecycle_stage' in rs && !LIFECYCLE_STAGES.includes(rs.lifecycle_stage)) {
    problems.push(`evidence and handoff "${id}" ${field}: the resolved execution-run record's resolved_state.lifecycle_stage ${JSON.stringify(rs.lifecycle_stage)} is not one of the closed lifecycle pool`);
  }
  if ('work_status' in rs && !WORK_STATUSES.includes(rs.work_status)) {
    problems.push(`evidence and handoff "${id}" ${field}: the resolved execution-run record's resolved_state.work_status ${JSON.stringify(rs.work_status)} is not one of the closed work-status pool`);
  }
  for (const f of ['next_action', 'next_gate']) {
    if (f in rs && rs[f] !== null) {
      if (blank(rs[f])) {
        problems.push(`evidence and handoff "${id}" ${field}: the resolved execution-run record's resolved_state.${f} is present but neither null nor a non-empty string`);
      } else {
        const r = nonPortableReason(rs[f]);
        if (r) problems.push(`evidence and handoff "${id}" ${field}: the resolved execution-run record's resolved_state.${f} contains ${r}`);
      }
    }
  }
  if ('blocker_ids' in rs) checkResolvedStateArray(problems, id, field, 'blocker_ids', rs.blocker_ids, 'id');
  if ('resolved_norms' in rs) checkResolvedStateArray(problems, id, field, 'resolved_norms', rs.resolved_norms, 'ref');
  if ('completed_checks' in rs) checkResolvedStateArray(problems, id, field, 'completed_checks', rs.completed_checks, 'ref');
}

// The CLOSED contract for a resolved task-specification: it carries the
// acceptance-criterion identifiers (a set of unique semantic ids) AND the
// mandatory-check references (a set of unique portable references) — a minimal
// machine list, NOT the spec body. An absent, non-array, non-semantic-id,
// duplicated or (for acceptance_criteria) EMPTY set is a precise error, not a
// skipped comparison: a canonical task specification requires a non-empty
// acceptance-criteria set.
function checkResolvedAcceptanceCriteria(problems, id, field, entry) {
  const ac = entry.acceptance_criteria;
  if (!Array.isArray(ac)) {
    problems.push(`evidence and handoff "${id}" ${field}: the resolver returned a task-specification record with no acceptance_criteria array; the handoff closes the loop with the specification's acceptance criteria and the transformer must confirm their identifiers`);
    return;
  }
  if (ac.length === 0) {
    problems.push(`evidence and handoff "${id}" ${field}: the resolved task-specification record's acceptance_criteria is empty; a canonical task specification requires a non-empty acceptance-criteria set, so a handoff that closes its coverage loop against an empty set is rejected`);
  }
  const seen = new Set();
  ac.forEach((c, i) => {
    if (typeof c !== 'string' || !SEMANTIC_ID_RE.test(c)) {
      problems.push(`evidence and handoff "${id}" ${field}: the resolved task-specification record's acceptance_criteria[${i}] is not a stable semantic id`);
      return;
    }
    if (seen.has(c)) problems.push(`evidence and handoff "${id}" ${field}: the resolved task-specification record's acceptance_criteria repeats "${c}"`);
    seen.add(c);
  });
}

// The CLOSED contract for a resolved task-specification's mandatory_checks: a
// minimal machine representation of the authoritative expected set — a list of
// unique portable check references, NOT the spec body. It lets the handoff's
// FULL mandatory_checks set be compared exactly against the specification, so
// dropping any one is rejected. An absent, non-array, non-portable or duplicated
// entry is a precise error.
function checkResolvedMandatoryChecks(problems, id, field, entry) {
  const mc = entry.mandatory_checks;
  if (!Array.isArray(mc)) {
    problems.push(`evidence and handoff "${id}" ${field}: the resolver returned a task-specification record with no mandatory_checks array; the handoff closes the FULL set of mandatory checks against the specification and the transformer must confirm their references`);
    return;
  }
  const seen = new Set();
  mc.forEach((c, i) => {
    if (typeof c !== 'string' || c.trim() === '') {
      problems.push(`evidence and handoff "${id}" ${field}: the resolved task-specification record's mandatory_checks[${i}] is not a non-empty portable reference`);
      return;
    }
    const r = nonPortableReason(c);
    if (r) problems.push(`evidence and handoff "${id}" ${field}: the resolved task-specification record's mandatory_checks[${i}] contains ${r}`);
    const key = c.trim();
    if (seen.has(key)) problems.push(`evidence and handoff "${id}" ${field}: the resolved task-specification record's mandatory_checks repeats "${c}"`);
    seen.add(key);
  });
}

// Resolve ONE pinned reference through the boundary and check it against the
// CLOSED transformer-response contract. Returns the resolved entry, or null when
// it could not be resolved (a problem is pushed). `opts.wanted` overrides the
// slot-derived record type (used for evidence-result); `opts.completeness`
// replaces the default slot-completeness tail (given the resolved entry).
function resolvePinnedReference(problems, id, field, ref, resolve, opts = {}) {
  if (!isObject(ref) || Array.isArray(ref)) return null;
  const slot = PINNED_REF_SLOTS[field];
  const wanted = opts.wanted || (slot ? slot.recordType : null);
  const entry = resolve(ref);
  if (!isObject(entry)) {
    problems.push(`evidence and handoff "${id}" ${field} does not resolve to an actual ${wanted || 'record'} through the external resolver; a pinned reference that resolves to nothing is not a verified pin and the handoff fails closed`);
    return null;
  }

  const allowedKeys = RESOLVED_ENTRY_KEYS_BY_TYPE[wanted] || RESOLVED_ENTRY_COMMON_KEYS;
  for (const k of Object.keys(entry)) {
    if (!allowedKeys.includes(k)) {
      problems.push(`evidence and handoff "${id}" ${field}: the resolver returned a record with an unknown field "${k}"; the transformer response is closed to { ${allowedKeys.join(', ')} }`);
    }
  }

  if (typeof entry.record_type !== 'string' || entry.record_type === '') {
    problems.push(`evidence and handoff "${id}" ${field}: the resolver returned a record with no record_type; the transformer response is a closed contract`);
  } else if (wanted && entry.record_type !== wanted) {
    problems.push(`evidence and handoff "${id}" ${field} resolves to a "${entry.record_type}" record, not "${wanted}"`);
  }

  if (typeof entry.id !== 'string' || entry.id.trim() === '') {
    problems.push(`evidence and handoff "${id}" ${field}: the resolver returned a ${wanted || 'record'} with no id; a resolved record without an identity cannot be checked against the pin`);
  } else if (!SEMANTIC_ID_RE.test(entry.id)) {
    problems.push(`evidence and handoff "${id}" ${field}: the resolver returned a ${wanted || 'record'} whose id "${entry.id}" is not a stable semantic identifier`);
  } else if (typeof ref.id === 'string' && entry.id !== ref.id) {
    problems.push(`evidence and handoff "${id}" ${field} pins id "${ref.id}" but the reference resolves to record id "${entry.id}"`);
  }

  if ('reference' in entry) {
    if (typeof entry.reference !== 'string' || entry.reference.trim() === '') {
      problems.push(`evidence and handoff "${id}" ${field}: the resolver's stated reference is present but not a non-empty string`);
    } else {
      const r = nonPortableReason(entry.reference);
      if (r) {
        problems.push(`evidence and handoff "${id}" ${field}: the resolver's stated reference contains ${r}`);
      } else if (typeof ref.reference === 'string' && entry.reference !== ref.reference) {
        problems.push(`evidence and handoff "${id}" ${field}: the resolver's stated reference "${entry.reference}" is not the resolved reference "${ref.reference}"`);
      }
    }
  }

  if ('content_digest' in entry
    && (typeof entry.content_digest !== 'string' || !SHA256_RE.test(entry.content_digest))) {
    problems.push(`evidence and handoff "${id}" ${field}: the resolver's content_digest ${JSON.stringify(entry.content_digest)} is not exactly 64 hexadecimal characters`);
  }
  if ('source_bytes' in entry && typeof entry.source_bytes !== 'string') {
    problems.push(`evidence and handoff "${id}" ${field}: the resolver's source_bytes is ${entry.source_bytes === null ? 'null' : typeof entry.source_bytes}, not a string`);
  }
  const hasRevisionString = typeof entry.revision === 'string' && entry.revision.trim() !== '';
  let revisionExact = false;
  if ('revision' in entry) {
    if (typeof entry.revision !== 'string') {
      const shown = entry.revision === null ? 'null'
        : Array.isArray(entry.revision) ? 'an array'
        : ({ number: 'a number', boolean: 'a boolean', object: 'an object' }[typeof entry.revision] || `a ${typeof entry.revision}`);
      problems.push(`evidence and handoff "${id}" ${field}: the resolver's revision is ${shown}, not a string`);
    } else if (entry.revision.trim() === '') {
      problems.push(`evidence and handoff "${id}" ${field}: the resolver's revision is present but empty`);
    } else if (classifyRevision(entry.revision) !== 'exact') {
      problems.push(`evidence and handoff "${id}" ${field}: the resolver confirmed revision ${JSON.stringify(entry.revision)}, which is not an exact edition (a full Git SHA, a strict v?X.Y.Z tag or a SHA-256 digest); the transformer confirms a fixed edition, not a moving reference`);
    } else {
      revisionExact = true;
    }
  }
  const { digest, inconsistent } = confirmedDigest(entry);
  if (!revisionExact && !digest) {
    problems.push(`evidence and handoff "${id}" ${field}: the resolver confirmed neither an exact revision nor a content digest for this edition; the pinned edition is unverified`);
  }
  if (inconsistent) {
    problems.push(`evidence and handoff "${id}" ${field}: the resolver's content_digest "${inconsistent}" does not match the SHA-256 of the resolved source bytes "${digest}"`);
  }
  if (typeof ref.revision === 'string' && ref.revision.trim() !== '') {
    if (!hasRevisionString) {
      problems.push(`evidence and handoff "${id}" ${field} pins revision "${ref.revision}" but the resolver confirmed no exact edition for this reference`);
    } else if (entry.revision !== ref.revision) {
      problems.push(`evidence and handoff "${id}" ${field} pins revision "${ref.revision}" but the resolver confirmed edition "${entry.revision}"`);
    }
  }
  if (typeof ref.sha256 === 'string' && ref.sha256.trim() !== '') {
    if (!digest) {
      problems.push(`evidence and handoff "${id}" ${field} pins sha256 "${ref.sha256}" but the resolver confirmed no content digest for this edition; a digest is verified against resolved source, never against a second copy of itself`);
    } else if (digest.toLowerCase() !== ref.sha256.toLowerCase()) {
      problems.push(`evidence and handoff "${id}" ${field} pins sha256 "${ref.sha256}" but the SHA-256 of the resolved source is "${digest}"`);
    }
  }

  if (typeof opts.completeness === 'function') {
    opts.completeness(problems, entry);
  } else if (wanted === 'execution-run') {
    checkResolvedState(problems, id, field, entry.resolved_state);
  } else if (wanted === 'run-human-control' || wanted === 'context-manifest') {
    if (typeof entry.linked_run_ref !== 'string' || entry.linked_run_ref.trim() === '') {
      problems.push(`evidence and handoff "${id}" ${field}: the resolver returned a ${wanted} record with no linked_run_ref; the run it belongs to is unconfirmed`);
    } else {
      const r = nonPortableReason(entry.linked_run_ref);
      if (r) problems.push(`evidence and handoff "${id}" ${field}: the resolved ${wanted} record's linked_run_ref contains ${r}`);
    }
  } else if (wanted === 'task-specification') {
    checkResolvedAcceptanceCriteria(problems, id, field, entry);
    checkResolvedMandatoryChecks(problems, id, field, entry);
  }

  return entry;
}

// Resolve ONE evidence entry through the boundary as an 'evidence-result'
// transformer response: the entry is pinned (an exact revision or a SHA-256
// digest under the reused closed rule), resolves to a CLOSED evidence-result
// carrying observed_result (confirmed / contradicted / inconclusive), and — for
// a specialised-evidence-record entry only — confirms specialised_contract and
// recorded_verdict by EXACT match against the handoff's stated values. Returns
// { resolved, clean, observedResult } — `clean` is true only when the entry
// produced no problem, and an entry only makes an assertion verified or confirms
// a check when `clean` and observedResult are right.
function resolveEvidenceEntry(problems, id, at, eo, resolve) {
  const synthRef = { record_type: 'evidence-result', id: eo.id, reference: eo.reference };
  if (typeof eo.revision === 'string' && eo.revision !== '') synthRef.revision = eo.revision;
  if (typeof eo.sha256 === 'string' && eo.sha256 !== '') synthRef.sha256 = eo.sha256;
  const before = problems.length;
  const entry = resolvePinnedReference(problems, id, `evidence entry ${at}`, synthRef, resolve, {
    wanted: 'evidence-result',
    completeness: (probs, e) => {
      if (!('observed_result' in e)) {
        probs.push(`evidence and handoff "${id}" evidence entry ${at}: the resolved evidence-result confirms no observed_result; the transformer says whether the artefact confirmed, contradicted or was inconclusive about its subject`);
      } else if (!OBSERVED_RESULTS.includes(e.observed_result)) {
        probs.push(`evidence and handoff "${id}" evidence entry ${at}: the resolved evidence-result's observed_result ${JSON.stringify(e.observed_result)} is not one of { ${OBSERVED_RESULTS.join(', ')} }`);
      }
      // The closed evidence-result confirms the SUBJECT observed_result is about:
      // the assertion ids this evidence bears on (covers) and/or the mandatory
      // check it is the recorded result of (check_ref). Field shape first.
      let confirmedCovers = null;
      if ('covers' in e) {
        if (!Array.isArray(e.covers)) {
          probs.push(`evidence and handoff "${id}" evidence entry ${at}: the resolved evidence-result's covers is ${e.covers === null ? 'null' : typeof e.covers}, not an array of assertion ids`);
        } else {
          confirmedCovers = new Set();
          e.covers.forEach((cv, k) => {
            if (typeof cv !== 'string' || !SEMANTIC_ID_RE.test(cv)) {
              probs.push(`evidence and handoff "${id}" evidence entry ${at}: the resolved evidence-result's covers[${k}] is not a stable semantic id`);
            } else if (confirmedCovers.has(cv)) {
              probs.push(`evidence and handoff "${id}" evidence entry ${at}: the resolved evidence-result's covers repeats "${cv}"`);
            } else {
              confirmedCovers.add(cv);
            }
          });
        }
      }
      if ('check_ref' in e) {
        if (blank(e.check_ref)) {
          probs.push(`evidence and handoff "${id}" evidence entry ${at}: the resolved evidence-result's check_ref is present but not a non-empty reference`);
        } else {
          const r = nonPortableReason(e.check_ref);
          if (r) probs.push(`evidence and handoff "${id}" evidence entry ${at}: the resolved evidence-result's check_ref contains ${r}`);
        }
      }
      // Subject binding: every assertion the HANDOFF claims this evidence covers
      // must be one the transformer confirms it bears on. A covers link is not
      // transferred to a foreign assertion by the handoff's own say-so.
      const handoffCovers = Array.isArray(eo.covers)
        ? eo.covers.filter((cv) => typeof cv === 'string' && SEMANTIC_ID_RE.test(cv)) : [];
      if (handoffCovers.length) {
        if (confirmedCovers === null) {
          probs.push(`evidence and handoff "${id}" evidence entry ${at}: the resolved evidence-result confirms no covered subject; which assertion(s) an evidence entry bears on is confirmed by the transformer, not asserted by the handoff, and summary + covers from the handoff alone are not enough`);
        } else {
          for (const cv of handoffCovers) {
            if (!confirmedCovers.has(cv)) {
              probs.push(`evidence and handoff "${id}" evidence entry ${at}: the resolved evidence-result does not confirm that this evidence bears on assertion "${cv}"; a resolved evidence entry's covers is not transferred to a foreign assertion`);
            }
          }
        }
      }
      if (eo.kind === 'specialised-evidence-record') {
        for (const f of ['specialised_contract', 'recorded_verdict']) {
          if (blank(e[f])) {
            probs.push(`evidence and handoff "${id}" evidence entry ${at}: the resolved specialised evidence record confirms no ${f}; a specialised verdict is confirmed by the resolved record, not by the handoff's own words`);
          } else if (e[f] !== eo[f]) {
            probs.push(`evidence and handoff "${id}" evidence entry ${at}: the resolved specialised evidence record's ${f} "${e[f]}" does not match the handoff's stated ${f} ${JSON.stringify(eo[f] ?? null)}`);
          }
        }
      } else {
        for (const f of ['specialised_contract', 'recorded_verdict']) {
          if (f in e) {
            probs.push(`evidence and handoff "${id}" evidence entry ${at}: the resolved evidence-result carries "${f}" but the evidence entry kind is "${eo.kind}", not "specialised-evidence-record"`);
          }
        }
      }
    },
  });
  const clean = problems.length === before;
  const observedResult = isObject(entry) && OBSERVED_RESULTS.includes(entry.observed_result) ? entry.observed_result : null;
  return { resolved: isObject(entry), clean, observedResult, entry: isObject(entry) ? entry : null };
}

// Set equality over the string members of two arrays.
function sameRefSet(a, b) {
  if (!Array.isArray(a) || !Array.isArray(b)) return true;
  const sa = new Set(a.filter((x) => typeof x === 'string').map((x) => x.trim()));
  const sb = new Set(b.filter((x) => typeof x === 'string').map((x) => x.trim()));
  return sa.size === sb.size && [...sa].every((x) => sb.has(x));
}

// A repository's pin as a comparable tuple.
function repoPinKey(entry) {
  const rev = typeof entry.revision === 'string' ? entry.revision.trim() : '';
  const sha = typeof entry.sha256 === 'string' ? entry.sha256.trim().toLowerCase() : '';
  return `${rev} ${sha}`;
}

// One repository-state list: non-empty, every entry pinned by the closed rule,
// no repository listed twice. Returns a Map repository_ref -> entry.
function checkRepositoryStates(problems, id, label, list) {
  const byRepo = new Map();
  const arr = Array.isArray(list) ? list : [];
  if (arr.length === 0) {
    problems.push(`evidence and handoff "${id}" ${label} lists no repository state; source and result states are pinned per repository`);
  }
  arr.forEach((s, i) => {
    const so = isObject(s) ? s : {};
    const repo = typeof so.repository_ref === 'string' ? so.repository_ref : null;
    const at = repo || `#${i}`;
    if (blank(so.repository_ref)) {
      problems.push(`evidence and handoff "${id}" ${label} entry ${at} has no repository_ref`);
    } else {
      const r = nonPortableReason(so.repository_ref);
      if (r) problems.push(`evidence and handoff "${id}" ${label} entry ${at} repository_ref contains ${r}`);
      const key = so.repository_ref.trim();
      if (byRepo.has(key)) {
        problems.push(`evidence and handoff "${id}" ${label} repository_ref "${so.repository_ref}" is listed more than once`);
      } else {
        byRepo.set(key, so);
      }
    }
    if (typeof so.revision === 'string' && so.revision !== '') {
      const r = nonPortableReason(so.revision);
      if (r) problems.push(`evidence and handoff "${id}" ${label} entry ${at} revision contains ${r}`);
    }
    const defect = pinDefect(so.revision, so.sha256);
    if (defect) problems.push(`evidence and handoff "${id}" ${label} entry ${at} is not pinned to an exact revision: ${defect}`);
  });
  return byRepo;
}

// The whole composition pipeline for one evidence-and-handoff record.
export function evaluateEvidenceAndHandoff(doc, { recordSchema, envelopeSchema, resolveRecords } = {}) {
  const problems = [];

  // 1. the canonical scoped-record envelope, validated separately and always.
  const envErrs = [];
  try { validate(doc, envelopeSchema, envelopeSchema, '', envErrs); }
  catch (e) { return [`record envelope schema could not be applied: ${e.message}`]; }
  problems.push(...envErrs.map((m) => `envelope ${m}`));

  // 2. the complete specialised schema — the same one a record declares.
  const bodyErrs = [];
  try { validate(doc, recordSchema, recordSchema, '', bodyErrs); }
  catch (e) { return [`evidence-and-handoff schema could not be applied: ${e.message}`]; }
  problems.push(...bodyErrs);

  if (!isObject(doc)) {
    return problems.length ? problems : ['the evidence and handoff record is not an object'];
  }

  const id = typeof doc.id === 'string' && doc.id !== '' ? doc.id : '(no id)';

  // 3. the record's $schema declaration must be a portable relative reference
  //    that RESOLVES, within the Meridian namespace, to the specialised schema.
  const declared = doc.$schema;
  if (typeof declared !== 'string' || declared === '') {
    problems.push(`evidence and handoff "${id}" declares no $schema; a record names ${EXPECTED_SCHEMA_BASENAME} so a consumer validates the whole contract — id, scope, origin, authority and body — not only the envelope`);
  } else {
    const portability = nonPortableReason(declared);
    if (portability) {
      problems.push(`evidence and handoff "${id}" $schema "${declared}" is not portable (${portability}); the schema is named by a relative reference resolved inside the Meridian namespace (canonical logical base ${CANONICAL_RECORD_BASE})`);
    } else {
      const resolved = resolveSchemaRef(declared);
      if (resolved === ENVELOPE_SCHEMA_REF) {
        problems.push(`evidence and handoff "${id}" $schema "${declared}" resolves to the record envelope (${ENVELOPE_SCHEMA_BASENAME}); it must name ${EXPECTED_SCHEMA_BASENAME}, which composes the envelope with the body`);
      } else if (resolved !== EXPECTED_SCHEMA_REF) {
        problems.push(`evidence and handoff "${id}" $schema "${declared}" does not resolve to the logical address ${EXPECTED_SCHEMA_REF} within the Meridian namespace; the specialised schema is named by a portable relative reference (a bare basename, a missing namespace segment, a wrong segment and a reference climbing out of the namespace all resolve elsewhere)`);
      }
    }
  }

  // 4. identity and the human-readable Russian name come from the envelope.
  if (doc.record_type !== RECORD_TYPE) {
    problems.push(`evidence and handoff "${id}" declares record_type "${doc.record_type}", not "${RECORD_TYPE}"; the record type names the handoff and does not open a second envelope`);
  }
  if (typeof doc.id !== 'string' || !SEMANTIC_ID_RE.test(doc.id)) {
    problems.push(`evidence and handoff "${id}" has no stable semantic id on the record envelope`);
  }
  if (typeof doc.title !== 'string' || doc.title.trim() === '') {
    problems.push(`evidence and handoff "${id}" has no human-readable title`);
  } else if (!hasCyrillic(doc.title)) {
    problems.push(`evidence and handoff "${id}" title "${doc.title}" carries no Russian (Cyrillic) text; the handoff name is stated in Russian for the human reader`);
  }
  const titleReason = nonPortableReason(doc.title);
  if (titleReason) problems.push(`evidence and handoff "${id}" title contains ${titleReason}`);

  // 5. the handoff lives only in run-state; scope.id identifies the run and
  //    scope.workspace_id is mandatory.
  const scope = isObject(doc.scope) ? doc.scope : {};
  if (typeof scope.type === 'string' && scope.type !== ALLOWED_SCOPE_TYPE) {
    const why = SCOPE_REJECTION_REASON[scope.type] || 'it is not run-state';
    problems.push(`evidence and handoff "${id}" is scoped to "${scope.type}"; an evidence-and-handoff record lives in run-state only — ${why}`);
  }
  if (scope.type === ALLOWED_SCOPE_TYPE) {
    if (typeof scope.id !== 'string' || !SEMANTIC_ID_RE.test(scope.id)) {
      problems.push(`evidence and handoff "${id}" run-state scope carries no stable scope.id identifying the run`);
    }
    if (typeof scope.workspace_id !== 'string' || !SEMANTIC_ID_RE.test(scope.workspace_id)) {
      problems.push(`evidence and handoff "${id}" run-state scope carries no workspace_id; a run belongs to a project workspace (workspace-scope-model.md §1)`);
    }
  }

  const origin = isObject(doc.origin) ? doc.origin : {};
  if (origin.kind === 'built-in') {
    problems.push(`evidence and handoff "${id}" declares origin.kind "built-in"; a handoff is written in a workspace, not shipped with the methodology`);
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
      if (r) problems.push(`evidence and handoff "${id}" ${label} contains ${r}; a handoff is portable and carries no rooted machine path`);
    }
  }

  const payload = isObject(doc.payload) ? doc.payload : {};

  // 6. an unbounded material dump, a specialised-evidence body, or a field of a
  //    later package.
  for (const f of FORBIDDEN_PAYLOAD_FIELDS) {
    if (f in payload) {
      problems.push(`evidence and handoff "${id}" payload carries "${f}"; the handoff is a bounded record of what happened and what to do next — an unbounded material dump, a full specialised-evidence body or the body of a referenced record belongs elsewhere, not here`);
    }
  }
  if ('record_type' in payload) {
    problems.push(`evidence and handoff "${id}" repeats record_type inside the payload; the record type is declared once, on the envelope`);
  }

  // 7. the four structured pinned references and the DETERMINISTIC single-run
  //    link.
  const runRef = payload.execution_run_ref;
  const runId = isObject(runRef) && typeof runRef.id === 'string' && runRef.id !== '' ? runRef.id : null;
  for (const field of ['execution_run_ref', 'task_specification_ref', 'human_control_ref', 'context_manifest_ref']) {
    if (!(field in payload)) {
      problems.push(`evidence and handoff "${id}" names no ${field}; a handoff carries a structured pinned reference to exactly one such record`);
      continue;
    }
    checkPinnedRef(problems, id, field, payload[field], runId);
  }
  if (scope.type === ALLOWED_SCOPE_TYPE && typeof scope.id === 'string' && scope.id !== '' && runId != null
    && scope.id !== runId) {
    problems.push(`evidence and handoff "${id}" scope identifies run "${scope.id}" but execution_run_ref.id is "${runId}"; the two must be the exact same string, not one a substring of the other`);
  }

  // 8. claimed results.
  const claimedIds = new Set();
  {
    const arr = Array.isArray(payload.claimed_results) ? payload.claimed_results : [];
    arr.forEach((c, i) => {
      const co = isObject(c) ? c : {};
      const cid = typeof co.id === 'string' ? co.id : null;
      const at = cid || `#${i}`;
      if (!cid || !SEMANTIC_ID_RE.test(cid)) {
        problems.push(`evidence and handoff "${id}" claimed result ${at} has no stable semantic id`);
      } else {
        if (claimedIds.has(cid)) problems.push(`evidence and handoff "${id}" claimed result id "${cid}" is used more than once`);
        claimedIds.add(cid);
      }
      if (blank(co.statement)) {
        problems.push(`evidence and handoff "${id}" claimed result ${at} has no statement`);
      } else {
        const r = nonPortableReason(co.statement);
        if (r) problems.push(`evidence and handoff "${id}" claimed result ${at} statement contains ${r}`);
      }
    });
  }

  // 9. verifiable assertions — each names its one claimed result.
  const assertionIds = new Set();
  const assertionById = new Map();
  const assertionsByResult = new Map();
  {
    const arr = Array.isArray(payload.verifiable_assertions) ? payload.verifiable_assertions : [];
    arr.forEach((a, i) => {
      const ao = isObject(a) ? a : {};
      const aid = typeof ao.id === 'string' ? ao.id : null;
      const at = aid || `#${i}`;
      if (!aid || !SEMANTIC_ID_RE.test(aid)) {
        problems.push(`evidence and handoff "${id}" verifiable assertion ${at} has no stable semantic id`);
      } else {
        if (assertionIds.has(aid)) problems.push(`evidence and handoff "${id}" verifiable assertion id "${aid}" is used more than once`);
        assertionIds.add(aid);
        assertionById.set(aid, ao);
      }
      if (blank(ao.statement)) {
        problems.push(`evidence and handoff "${id}" verifiable assertion ${at} has no statement`);
      } else {
        const r = nonPortableReason(ao.statement);
        if (r) problems.push(`evidence and handoff "${id}" verifiable assertion ${at} statement contains ${r}`);
      }
      if (typeof ao.claimed_result_id !== 'string' || !SEMANTIC_ID_RE.test(ao.claimed_result_id)) {
        problems.push(`evidence and handoff "${id}" verifiable assertion ${at} names no claimed_result_id`);
      } else if (!claimedIds.has(ao.claimed_result_id)) {
        problems.push(`evidence and handoff "${id}" verifiable assertion ${at} names claimed_result_id "${ao.claimed_result_id}", which is not a declared claimed result`);
      } else {
        if (!assertionsByResult.has(ao.claimed_result_id)) assertionsByResult.set(ao.claimed_result_id, []);
        assertionsByResult.get(ao.claimed_result_id).push(aid);
      }
      if (ao.status === 'unverified') {
        if (blank(ao.unverified_reason)) {
          problems.push(`evidence and handoff "${id}" verifiable assertion ${at} is unverified but states no unverified_reason; the unproven stays explicitly unproven`);
        } else {
          const r = nonPortableReason(ao.unverified_reason);
          if (r) problems.push(`evidence and handoff "${id}" verifiable assertion ${at} unverified_reason contains ${r}`);
        }
      } else if (ao.status === 'verified' && 'unverified_reason' in ao) {
        problems.push(`evidence and handoff "${id}" verifiable assertion ${at} is verified but carries an unverified_reason`);
      }
    });
  }

  // 10. evidence — pinned, and (in §20) resolved through the external boundary;
  //     the structural covers link is checked here, the SUPPORTING link is §20.
  const evidenceIds = new Set();
  const evidenceById = new Map();
  {
    const arr = Array.isArray(payload.evidence) ? payload.evidence : [];
    arr.forEach((e, i) => {
      const eo = isObject(e) ? e : {};
      const eid = typeof eo.id === 'string' ? eo.id : null;
      const at = eid || `#${i}`;
      if (!eid || !SEMANTIC_ID_RE.test(eid)) {
        problems.push(`evidence and handoff "${id}" evidence entry ${at} has no stable semantic id`);
      } else {
        if (evidenceIds.has(eid)) problems.push(`evidence and handoff "${id}" evidence id "${eid}" is used more than once`);
        evidenceIds.add(eid);
        evidenceById.set(eid, eo);
      }
      for (const [f, human] of [['reference', 'reference'], ['summary', 'summary']]) {
        if (blank(eo[f])) {
          problems.push(`evidence and handoff "${id}" evidence entry ${at} has no ${human}`);
        } else {
          const r = nonPortableReason(eo[f]);
          if (r) problems.push(`evidence and handoff "${id}" evidence entry ${at} ${f} contains ${r}`);
        }
      }
      if (typeof eo.revision === 'string' && eo.revision !== '') {
        const rr = nonPortableReason(eo.revision);
        if (rr) problems.push(`evidence and handoff "${id}" evidence entry ${at} revision contains ${rr}`);
      }
      const pinReason = pinDefect(eo.revision, eo.sha256);
      if (pinReason) {
        problems.push(`evidence and handoff "${id}" evidence entry ${at} is not pinned to an exact edition: ${pinReason}; a plain reference is not verifiable evidence`);
      }
      const covers = Array.isArray(eo.covers) ? eo.covers : [];
      const seenCover = new Set();
      covers.forEach((cv, j) => {
        if (typeof cv !== 'string' || !SEMANTIC_ID_RE.test(cv)) {
          problems.push(`evidence and handoff "${id}" evidence entry ${at} covers[${j}] is not a stable semantic id`);
          return;
        }
        if (seenCover.has(cv)) problems.push(`evidence and handoff "${id}" evidence entry ${at} covers "${cv}" more than once`);
        seenCover.add(cv);
        if (!assertionIds.has(cv)) {
          problems.push(`evidence and handoff "${id}" evidence entry ${at} covers "${cv}", which is not a declared verifiable assertion; a narrow evidence entry is not widened to an undeclared assertion`);
        }
      });
      const limitations = Array.isArray(eo.limitations) ? eo.limitations : [];
      limitations.forEach((lm, j) => {
        if (blank(lm)) {
          problems.push(`evidence and handoff "${id}" evidence entry ${at} limitations[${j}] is empty or whitespace-only`);
        } else {
          const r = nonPortableReason(lm);
          if (r) problems.push(`evidence and handoff "${id}" evidence entry ${at} limitations[${j}] contains ${r}`);
        }
      });
      if (eo.kind === 'specialised-evidence-record') {
        for (const [f, human] of [['specialised_contract', 'specialised_contract'], ['recorded_verdict', 'recorded_verdict']]) {
          if (blank(eo[f])) {
            problems.push(`evidence and handoff "${id}" evidence entry ${at} is a specialised-evidence-record but states no ${human}; the specialised contract keeps authority over its own verdict, which this handoff records rather than re-derives`);
          } else {
            const r = nonPortableReason(eo[f]);
            if (r) problems.push(`evidence and handoff "${id}" evidence entry ${at} ${f} contains ${r}`);
          }
        }
      } else {
        for (const f of ['specialised_contract', 'recorded_verdict']) {
          if (f in eo) {
            problems.push(`evidence and handoff "${id}" evidence entry ${at} carries "${f}" but its kind is "${eo.kind}", not "specialised-evidence-record"`);
          }
        }
      }
    });
  }

  // 11. the established / not_established inference — checked on BOTH sides. The
  //     "verified needs SUPPORTING resolved evidence" rule is §20 (it needs the
  //     external boundary); here the status is checked against the assertion
  //     set it names, so it cannot be asserted against its own evidence.
  {
    const arr = Array.isArray(payload.claimed_results) ? payload.claimed_results : [];
    arr.forEach((c) => {
      const co = isObject(c) ? c : {};
      const cid = typeof co.id === 'string' ? co.id : null;
      const linked = (cid && assertionsByResult.get(cid)) || [];
      const unverified = linked.filter((aid) => {
        const ao = assertionById.get(aid);
        return !ao || ao.status !== 'verified';
      });
      const allVerified = linked.length > 0 && unverified.length === 0;
      if (co.status === 'established') {
        if (linked.length === 0) {
          problems.push(`evidence and handoff "${id}" claimed result "${cid}" is established but no verifiable assertion links to it; an established result carries at least one verified assertion`);
        } else if (unverified.length) {
          problems.push(`evidence and handoff "${id}" claimed result "${cid}" is established but assertion(s) ${unverified.join(', ')} linked to it are not verified; the unproven cannot be established`);
        }
      } else if (co.status === 'not_established') {
        if (allVerified) {
          problems.push(`evidence and handoff "${id}" claimed result "${cid}" is "not_established" but it has at least one linked verifiable assertion and every one of them is verified; the status is derived from the evidence, not asserted — this result is established`);
        }
      }
    });
  }

  // 12. mandatory checks — three distinct statuses, each backed by evidence of
  //     its result: passed / failed name a result_evidence_id whose resolved
  //     evidence-result confirms 'confirmed' / 'contradicted' AND records THIS
  //     check's check_ref (§20); unable forbids result evidence and carries a
  //     verifiable reason. completed_checks of the resolved run confirms the
  //     fact a check RAN: a passed OR a failed check's check_ref is there, only
  //     an unable one is absent (§20).
  const executedCheckRefs = new Set();
  const notExecutedCheckRefs = new Set();
  const checkResultExpect = [];
  let hasFailedOrUnableCheck = false;
  {
    const seenId = new Set();
    const seenRef = new Set();
    const arr = Array.isArray(payload.mandatory_checks) ? payload.mandatory_checks : [];
    arr.forEach((m, i) => {
      const mo = isObject(m) ? m : {};
      const mid = typeof mo.id === 'string' ? mo.id : null;
      const at = mid || `#${i}`;
      if (!mid || !SEMANTIC_ID_RE.test(mid)) {
        problems.push(`evidence and handoff "${id}" mandatory check ${at} has no stable semantic id`);
      } else {
        if (seenId.has(mid)) problems.push(`evidence and handoff "${id}" mandatory check id "${mid}" is used more than once`);
        seenId.add(mid);
      }
      if (blank(mo.name)) {
        problems.push(`evidence and handoff "${id}" mandatory check ${at} has no name`);
      } else {
        const r = nonPortableReason(mo.name);
        if (r) problems.push(`evidence and handoff "${id}" mandatory check ${at} name contains ${r}`);
      }
      const ref = typeof mo.check_ref === 'string' ? mo.check_ref.trim() : null;
      if (blank(mo.check_ref)) {
        problems.push(`evidence and handoff "${id}" mandatory check ${at} has no check_ref`);
      } else {
        const r = nonPortableReason(mo.check_ref);
        if (r) problems.push(`evidence and handoff "${id}" mandatory check ${at} check_ref contains ${r}`);
        if (seenRef.has(ref)) problems.push(`evidence and handoff "${id}" mandatory check check_ref "${mo.check_ref}" is listed more than once`);
        seenRef.add(ref);
      }
      const rev = typeof mo.result_evidence_id === 'string' ? mo.result_evidence_id : null;
      const revValid = rev != null && SEMANTIC_ID_RE.test(rev);
      if (mo.status === 'passed' || mo.status === 'failed') {
        const want = mo.status === 'passed' ? 'confirmed' : 'contradicted';
        if (!revValid) {
          problems.push(`evidence and handoff "${id}" mandatory check ${at} is "${mo.status}" but names no result_evidence_id; a ${mo.status} status is backed by an evidence entry that shows the ${mo.status === 'passed' ? 'successful' : 'failing'} result, not by the completed_checks list`);
        } else if (!evidenceIds.has(rev)) {
          problems.push(`evidence and handoff "${id}" mandatory check ${at} result_evidence_id "${rev}" is not a declared evidence entry`);
        } else {
          checkResultExpect.push({ at, evidenceId: rev, expect: want, status: mo.status, checkRef: ref });
        }
      }
      if (mo.status === 'passed') {
        if ('reason' in mo) {
          problems.push(`evidence and handoff "${id}" mandatory check ${at} passed but carries a reason; a reason is stated for a failed or unable check`);
        }
        if (ref) executedCheckRefs.add(ref);
      } else if (mo.status === 'failed') {
        hasFailedOrUnableCheck = true;
        if (blank(mo.reason)) {
          problems.push(`evidence and handoff "${id}" mandatory check ${at} is "failed" but states no reason`);
        } else {
          const r = nonPortableReason(mo.reason);
          if (r) problems.push(`evidence and handoff "${id}" mandatory check ${at} reason contains ${r}`);
        }
        if (ref) executedCheckRefs.add(ref);
      } else if (mo.status === 'unable') {
        hasFailedOrUnableCheck = true;
        if (blank(mo.reason)) {
          problems.push(`evidence and handoff "${id}" mandatory check ${at} is "unable" but states no verifiable reason; an unable check is not a pass and does not become one silently`);
        } else {
          const r = nonPortableReason(mo.reason);
          if (r) problems.push(`evidence and handoff "${id}" mandatory check ${at} reason contains ${r}`);
        }
        if ('result_evidence_id' in mo) {
          problems.push(`evidence and handoff "${id}" mandatory check ${at} is "unable" but names a result_evidence_id; a check that could not run has no result to evidence`);
        }
        if (ref) notExecutedCheckRefs.add(ref);
      }
    });
  }

  // 12b. acceptance-criteria coverage — each criterion is addressed by a
  //      verifiable assertion or explicitly uncovered with a reason. The
  //      criterion id set is closed against the resolved task specification
  //      (§20), which also forces a non-empty mandatory_checks list.
  const acCoverage = new Map();
  {
    const seenId = new Set();
    const arr = Array.isArray(payload.acceptance_criteria) ? payload.acceptance_criteria : [];
    arr.forEach((c, i) => {
      const co = isObject(c) ? c : {};
      const cid = typeof co.id === 'string' ? co.id : null;
      const at = cid || `#${i}`;
      if (!cid || !SEMANTIC_ID_RE.test(cid)) {
        problems.push(`evidence and handoff "${id}" acceptance criterion ${at} has no stable semantic id`);
      } else {
        if (seenId.has(cid)) problems.push(`evidence and handoff "${id}" acceptance criterion id "${cid}" is used more than once`);
        seenId.add(cid);
      }
      const addressedBy = Array.isArray(co.addressed_by) ? co.addressed_by : [];
      const seenA = new Set();
      addressedBy.forEach((aid, j) => {
        if (typeof aid !== 'string' || !SEMANTIC_ID_RE.test(aid)) {
          problems.push(`evidence and handoff "${id}" acceptance criterion ${at} addressed_by[${j}] is not a stable semantic id`);
          return;
        }
        if (seenA.has(aid)) problems.push(`evidence and handoff "${id}" acceptance criterion ${at} addresses "${aid}" more than once`);
        seenA.add(aid);
        if (!assertionIds.has(aid)) {
          problems.push(`evidence and handoff "${id}" acceptance criterion ${at} is addressed_by "${aid}", which is not a declared verifiable assertion`);
        }
      });
      if (co.status === 'covered') {
        if (addressedBy.length === 0) {
          problems.push(`evidence and handoff "${id}" acceptance criterion ${at} is "covered" but names no addressed_by verifiable assertion`);
        }
        if ('uncovered_reason' in co) {
          problems.push(`evidence and handoff "${id}" acceptance criterion ${at} is "covered" but carries an uncovered_reason`);
        }
      } else if (co.status === 'uncovered') {
        if (addressedBy.length > 0) {
          problems.push(`evidence and handoff "${id}" acceptance criterion ${at} is "uncovered" but names addressed_by assertions`);
        }
        if (blank(co.uncovered_reason)) {
          problems.push(`evidence and handoff "${id}" acceptance criterion ${at} is "uncovered" but states no uncovered_reason`);
        } else {
          const r = nonPortableReason(co.uncovered_reason);
          if (r) problems.push(`evidence and handoff "${id}" acceptance criterion ${at} uncovered_reason contains ${r}`);
        }
      }
      if (cid) acCoverage.set(cid, { status: co.status, addressedBy: addressedBy.filter((x) => typeof x === 'string') });
    });
  }

  // 13. source and result states, pinned per repository by the closed rule and
  //     pinning the SAME set of repositories.
  const srcByRepo = checkRepositoryStates(problems, id, 'source_state', payload.source_state);
  const resByRepo = checkRepositoryStates(problems, id, 'result_state', payload.result_state);
  if (srcByRepo.size > 0 && resByRepo.size > 0) {
    const extraInSrc = [...srcByRepo.keys()].filter((k) => !resByRepo.has(k));
    const extraInRes = [...resByRepo.keys()].filter((k) => !srcByRepo.has(k));
    if (extraInSrc.length || extraInRes.length) {
      problems.push(`evidence and handoff "${id}" source_state and result_state pin different sets of repositories (${
        extraInSrc.length ? `source-only: ${extraInSrc.join(', ')}` : ''}${extraInSrc.length && extraInRes.length ? '; ' : ''}${
        extraInRes.length ? `result-only: ${extraInRes.join(', ')}` : ''}); a run's before and after states cover the same repositories`);
    }
  }

  // 14. changed paths — every repository named appears in both states with a pin
  //     that differs.
  {
    const seenId = new Set();
    const seenPair = new Set();
    const arr = Array.isArray(payload.changed_paths) ? payload.changed_paths : [];
    arr.forEach((c, i) => {
      const co = isObject(c) ? c : {};
      const cid = typeof co.id === 'string' ? co.id : null;
      const at = cid || `#${i}`;
      if (!cid || !SEMANTIC_ID_RE.test(cid)) {
        problems.push(`evidence and handoff "${id}" changed path ${at} has no stable semantic id`);
      } else {
        if (seenId.has(cid)) problems.push(`evidence and handoff "${id}" changed path id "${cid}" is used more than once`);
        seenId.add(cid);
      }
      for (const [f, human] of [['repository_ref', 'repository_ref'], ['path', 'path']]) {
        if (blank(co[f])) {
          problems.push(`evidence and handoff "${id}" changed path ${at} has no ${human}`);
        } else {
          const r = nonPortableReason(co[f]);
          if (r) problems.push(`evidence and handoff "${id}" changed path ${at} ${f} contains ${r}`);
        }
      }
      const repo = typeof co.repository_ref === 'string' ? co.repository_ref.trim() : null;
      const p = typeof co.path === 'string' ? co.path.trim() : null;
      if (repo && p) {
        const pair = `${repo} ${p}`;
        if (seenPair.has(pair)) problems.push(`evidence and handoff "${id}" changed path repeats "${co.path}" in repository "${co.repository_ref}"`);
        seenPair.add(pair);
      }
      if (repo) {
        const inSrc = srcByRepo.get(repo);
        const inRes = resByRepo.get(repo);
        if (!inSrc || !inRes) {
          problems.push(`evidence and handoff "${id}" changed path ${at} names repository "${co.repository_ref}", which is not pinned in both source_state and result_state; a change has a before and an after revision`);
        } else if (repoPinKey(inSrc) === repoPinKey(inRes)) {
          problems.push(`evidence and handoff "${id}" changed path ${at} names repository "${co.repository_ref}", but its source_state and result_state pins are identical; a change produced a new revision`);
        }
      }
    });
  }

  // 15. external effects, deviations, open gaps, required owner decisions.
  {
    const seenId = new Set();
    const arr = Array.isArray(payload.external_effects) ? payload.external_effects : [];
    arr.forEach((e, i) => {
      const eo = isObject(e) ? e : {};
      const eid = typeof eo.id === 'string' ? eo.id : null;
      const at = eid || `#${i}`;
      if (!eid || !SEMANTIC_ID_RE.test(eid)) {
        problems.push(`evidence and handoff "${id}" external effect ${at} has no stable semantic id`);
      } else {
        if (seenId.has(eid)) problems.push(`evidence and handoff "${id}" external effect id "${eid}" is used more than once`);
        seenId.add(eid);
      }
      if (blank(eo.description)) problems.push(`evidence and handoff "${id}" external effect ${at} has no description`);
      else {
        const r = nonPortableReason(eo.description);
        if (r) problems.push(`evidence and handoff "${id}" external effect ${at} description contains ${r}`);
      }
      if (typeof eo.reversible !== 'boolean') {
        problems.push(`evidence and handoff "${id}" external effect ${at} does not state whether it is reversible`);
      }
    });
  }
  let hasBlockingDeviation = false;
  {
    const seenId = new Set();
    const arr = Array.isArray(payload.deviations) ? payload.deviations : [];
    arr.forEach((d, i) => {
      const dobj = isObject(d) ? d : {};
      const did = typeof dobj.id === 'string' ? dobj.id : null;
      const at = did || `#${i}`;
      if (!did || !SEMANTIC_ID_RE.test(did)) {
        problems.push(`evidence and handoff "${id}" deviation ${at} has no stable semantic id`);
      } else {
        if (seenId.has(did)) problems.push(`evidence and handoff "${id}" deviation id "${did}" is used more than once`);
        seenId.add(did);
      }
      for (const [f, human] of [['description', 'description'], ['disposition', 'disposition']]) {
        if (blank(dobj[f])) problems.push(`evidence and handoff "${id}" deviation ${at} has no ${human}`);
        else {
          const r = nonPortableReason(dobj[f]);
          if (r) problems.push(`evidence and handoff "${id}" deviation ${at} ${f} contains ${r}`);
        }
      }
      if (dobj.severity === 'blocking') hasBlockingDeviation = true;
    });
  }
  checkIdentifiedItems(problems, id, 'open_gaps', payload.open_gaps, ['description']);
  let hasBlockingOwnerDecision = false;
  {
    const seenId = new Set();
    const arr = Array.isArray(payload.required_owner_decisions) ? payload.required_owner_decisions : [];
    arr.forEach((o, i) => {
      const oo = isObject(o) ? o : {};
      const oid = typeof oo.id === 'string' ? oo.id : null;
      const at = oid || `#${i}`;
      if (!oid || !SEMANTIC_ID_RE.test(oid)) {
        problems.push(`evidence and handoff "${id}" required owner decision ${at} has no stable semantic id`);
      } else {
        if (seenId.has(oid)) problems.push(`evidence and handoff "${id}" required owner decision id "${oid}" is used more than once`);
        seenId.add(oid);
      }
      if (blank(oo.question)) problems.push(`evidence and handoff "${id}" required owner decision ${at} has no question`);
      else {
        const r = nonPortableReason(oo.question);
        if (r) problems.push(`evidence and handoff "${id}" required owner decision ${at} question contains ${r}`);
      }
      if (typeof oo.blocking !== 'boolean') {
        problems.push(`evidence and handoff "${id}" required owner decision ${at} does not state whether it is blocking`);
      } else if (oo.blocking === true) {
        hasBlockingOwnerDecision = true;
      }
    });
  }

  // 16. blockers — same shape as the execution state model.
  const blockerIds = new Set();
  {
    const arr = Array.isArray(payload.blockers) ? payload.blockers : [];
    arr.forEach((b, i) => {
      const bo = isObject(b) ? b : {};
      const bid = typeof bo.id === 'string' ? bo.id : null;
      const at = bid || `#${i}`;
      if (!bid || !SEMANTIC_ID_RE.test(bid)) {
        problems.push(`evidence and handoff "${id}" blocker ${at} has no stable id`);
      } else {
        if (blockerIds.has(bid)) problems.push(`evidence and handoff "${id}" blocker id "${bid}" is used more than once`);
        blockerIds.add(bid);
      }
      if (blank(bo.description)) problems.push(`evidence and handoff "${id}" blocker ${at} has no description`);
      if (blank(bo.resumption_condition)) problems.push(`evidence and handoff "${id}" blocker ${at} has no verifiable resumption condition`);
      for (const s of [bo.description, bo.resumption_condition]) {
        const r = nonPortableReason(s);
        if (r) problems.push(`evidence and handoff "${id}" blocker ${at} contains ${r}`);
      }
    });
  }

  // 17. the worktree disposition — the closed set of version-control-flow.md §5.4.
  {
    const wt = isObject(payload.worktree_disposition) ? payload.worktree_disposition : null;
    if (wt) {
      if (wt.state === 'retained') {
        for (const f of RETAINED_ONLY_FIELDS) {
          if (blank(wt[f])) {
            problems.push(`evidence and handoff "${id}" worktree_disposition is "retained" but states no ${f}; a retained worktree carries a reason, a responsible party and a verifiable cleanup condition`);
          } else {
            const r = nonPortableReason(wt[f]);
            if (r) problems.push(`evidence and handoff "${id}" worktree_disposition ${f} contains ${r}`);
          }
        }
      } else if (wt.state === 'not_created' || wt.state === 'removed') {
        for (const f of RETAINED_ONLY_FIELDS) {
          if (f in wt) {
            problems.push(`evidence and handoff "${id}" worktree_disposition is "${wt.state}" but carries "${f}"; reason, responsible and cleanup_condition belong to the "retained" state only`);
          }
        }
      }
    }
  }

  // 18. the next step, stated explicitly.
  const nextStep = isObject(payload.next_step) ? payload.next_step : {};
  for (const f of ['action', 'gate', 'actor_ref']) {
    if (f in nextStep && nextStep[f] !== null && blank(nextStep[f])) {
      problems.push(`evidence and handoff "${id}" next_step.${f} is present but empty; state a value, or null where there is none`);
    }
    const r = nonPortableReason(nextStep[f]);
    if (r) problems.push(`evidence and handoff "${id}" next_step.${f} contains ${r}`);
  }

  // 19. the overall outcome and its axis rules.
  const outcome = isObject(payload.outcome) ? payload.outcome : {};
  if (blank(outcome.statement)) {
    problems.push(`evidence and handoff "${id}" outcome states no statement`);
  } else {
    const r = nonPortableReason(outcome.statement);
    if (r) problems.push(`evidence and handoff "${id}" outcome statement contains ${r}`);
  }
  const resultsArr = Array.isArray(payload.claimed_results) ? payload.claimed_results : [];
  const everyResultEstablished = resultsArr.length > 0
    && resultsArr.every((c) => isObject(c) && c.status === 'established');
  const everyCheckPassed = (Array.isArray(payload.mandatory_checks) ? payload.mandatory_checks : [])
    .every((m) => isObject(m) && m.status === 'passed');
  const openGapCount = Array.isArray(payload.open_gaps) ? payload.open_gaps.length : 0;
  const blockedTrigger = blockerIds.size > 0 || hasFailedOrUnableCheck || hasBlockingDeviation;
  if (outcome.status === 'complete') {
    if (!everyResultEstablished) {
      problems.push(`evidence and handoff "${id}" outcome.status is "complete" but not every claimed result is established`);
    }
    if (!everyCheckPassed) {
      problems.push(`evidence and handoff "${id}" outcome.status is "complete" but a mandatory check is not "passed"; a failed or unable check is not a completion`);
    }
    if (blockerIds.size > 0 || hasBlockingDeviation) {
      problems.push(`evidence and handoff "${id}" outcome.status is "complete" but a blocker or a blocking deviation remains`);
    }
    if (hasBlockingOwnerDecision) {
      problems.push(`evidence and handoff "${id}" outcome.status is "complete" but a required owner decision is blocking; the next step cannot proceed, so the run is handed off, not complete`);
    }
    if (openGapCount > 0) {
      problems.push(`evidence and handoff "${id}" outcome.status is "complete" but ${openGapCount} open gap(s) remain; a completed run carries no acknowledged incompleteness — record it as a deviation or leave the run handed_off_incomplete`);
    }
  } else if (outcome.status === 'blocked') {
    if (!blockedTrigger) {
      problems.push(`evidence and handoff "${id}" outcome.status is "blocked" but no blocker, failed or unable mandatory check, or blocking deviation is recorded`);
    }
  } else if (outcome.status === 'handed_off_incomplete') {
    if (blockedTrigger) {
      problems.push(`evidence and handoff "${id}" outcome.status is "handed_off_incomplete" but a blocker, a failed or unable mandatory check, or a blocking deviation is recorded; that is a "blocked" outcome`);
    }
  }
  if (blockerIds.size > 0 && outcome.status !== undefined && outcome.status !== 'blocked') {
    problems.push(`evidence and handoff "${id}" carries ${blockerIds.size} blocker(s) but outcome.status is "${outcome.status}"; a non-empty blocker set is a "blocked" outcome`);
  }

  // 20. THE EXTERNAL RESOLUTION BOUNDARY. Resolve the four pinned references
  //     through a resolver the handoff does not control, and check the handoff's
  //     own axes against the state the RESOLVED execution-run record carries.
  //     With no resolver the handoff cannot be verified and fails closed.
  const resolve = typeof resolveRecords === 'function' ? resolveRecords : null;
  if (!resolve) {
    problems.push(`evidence and handoff "${id}" cannot be verified: no external record resolver was supplied; the pinned execution-run, task-specification, run-human-control and context-manifest records, and every evidence entry, are resolved OUTSIDE the handoff and checked against it — without that boundary the handoff is only an unanchored self-report`);
  } else {
    const runEntry = ('execution_run_ref' in payload)
      ? resolvePinnedReference(problems, id, 'execution_run_ref', payload.execution_run_ref, resolve) : null;
    const specEntry = ('task_specification_ref' in payload)
      ? resolvePinnedReference(problems, id, 'task_specification_ref', payload.task_specification_ref, resolve) : null;
    const hcEntry = ('human_control_ref' in payload)
      ? resolvePinnedReference(problems, id, 'human_control_ref', payload.human_control_ref, resolve) : null;
    const cmEntry = ('context_manifest_ref' in payload)
      ? resolvePinnedReference(problems, id, 'context_manifest_ref', payload.context_manifest_ref, resolve) : null;

    const runState = runEntry && isObject(runEntry.resolved_state) ? runEntry.resolved_state : null;
    const resolvedCriteria = specEntry && Array.isArray(specEntry.acceptance_criteria) && specEntry.acceptance_criteria.length > 0
      ? new Set(specEntry.acceptance_criteria.filter((c) => typeof c === 'string')) : null;
    const resolvedMandatoryChecks = specEntry && Array.isArray(specEntry.mandatory_checks)
      ? new Set(specEntry.mandatory_checks.filter((c) => typeof c === 'string').map((c) => c.trim())) : null;

    // 20a. resolve every evidence entry through the external boundary. An entry
    //      only SUPPORTS an assertion, or confirms a check's result, when it
    //      resolves cleanly and its observed_result is right.
    const evidenceResolution = new Map();
    const supportedAssertionIds = new Set();
    {
      const arr = Array.isArray(payload.evidence) ? payload.evidence : [];
      arr.forEach((e, i) => {
        const eo = isObject(e) ? e : {};
        const at = typeof eo.id === 'string' ? eo.id : `#${i}`;
        const res = resolveEvidenceEntry(problems, id, at, eo, resolve);
        if (typeof eo.id === 'string') evidenceResolution.set(eo.id, res);
        if (res.clean && res.observedResult === 'confirmed') {
          const covers = Array.isArray(eo.covers) ? eo.covers : [];
          for (const cv of covers) {
            if (typeof cv === 'string' && assertionIds.has(cv)) supportedAssertionIds.add(cv);
          }
        }
      });
    }

    // 20b. a verified assertion needs a resolved, pin-checked evidence entry
    //      whose observed_result is "confirmed" — a plain reference is not proof.
    for (const [aid, ao] of assertionById) {
      if (ao.status === 'verified' && !supportedAssertionIds.has(aid)) {
        problems.push(`evidence and handoff "${id}" verifiable assertion "${aid}" is "verified" but no resolved, pin-checked evidence entry confirms it (a covering evidence entry that resolves and whose observed_result is "confirmed"); a plain reference is not proof`);
      }
    }

    // 20c. each passed / failed mandatory check's result evidence resolves with
    //      the matching observed_result AND records THIS check's check_ref: the
    //      transformer confirms which check a result belongs to, so a result
    //      cannot be moved between checks and one check's result is not another
    //      check's result.
    for (const { at, evidenceId, expect, status, checkRef } of checkResultExpect) {
      const res = evidenceResolution.get(evidenceId);
      if (!res) continue;
      if (!res.clean) {
        problems.push(`evidence and handoff "${id}" mandatory check ${at} is "${status}" but its result evidence "${evidenceId}" did not resolve cleanly through the external boundary`);
      } else if (res.observedResult !== expect) {
        problems.push(`evidence and handoff "${id}" mandatory check ${at} is "${status}" but its result evidence "${evidenceId}" resolves with observed_result ${JSON.stringify(res.observedResult)}, not "${expect}"`);
      }
      const confirmedCheckRef = res.entry && typeof res.entry.check_ref === 'string' && res.entry.check_ref.trim() !== ''
        ? res.entry.check_ref.trim() : null;
      if (res.resolved) {
        if (confirmedCheckRef === null) {
          problems.push(`evidence and handoff "${id}" mandatory check ${at} names result evidence "${evidenceId}", but the resolved evidence-result records no check_ref; the transformer confirms which check a recorded result is the result of, and summary + covers from the handoff alone are not enough`);
        } else if (checkRef != null && confirmedCheckRef !== checkRef) {
          problems.push(`evidence and handoff "${id}" mandatory check ${at} names result evidence "${evidenceId}", but the resolved evidence-result records check_ref "${confirmedCheckRef}", not this check's check_ref "${checkRef}"; a check's result is not another check's result`);
        }
      }
    }

    // 20d. acceptance-criteria coverage is closed against the resolved task
    //      specification; a non-empty criterion set forces a non-empty
    //      mandatory_checks list.
    if (resolvedCriteria) {
      const handoffCriteria = new Set(acCoverage.keys());
      for (const c of handoffCriteria) {
        if (!resolvedCriteria.has(c)) {
          problems.push(`evidence and handoff "${id}" acceptance criterion "${c}" is not among the resolved task-specification record's acceptance criteria`);
        }
      }
      for (const c of resolvedCriteria) {
        if (!handoffCriteria.has(c)) {
          problems.push(`evidence and handoff "${id}" does not address acceptance criterion "${c}" declared by the resolved task-specification record; every criterion is covered or explicitly uncovered`);
        }
      }
      if (resolvedCriteria.size > 0
        && (!Array.isArray(payload.mandatory_checks) || payload.mandatory_checks.length === 0)) {
        problems.push(`evidence and handoff "${id}" lists no mandatory_checks, but the resolved task-specification record declares ${resolvedCriteria.size} acceptance criteria; a self-selected empty check list does not discharge the specification's mandatory checks`);
      }
    }

    // 20d'. the FULL set of mandatory_checks is closed against the authoritative
    //       specification: the resolved task-specification carries a minimal
    //       machine list of expected check references. Every expected check is
    //       present exactly once; an unknown or a MISSING one is rejected, so
    //       dropping a mandatory check from the handoff cannot pass.
    if (resolvedMandatoryChecks) {
      const handoffChecks = new Set();
      const arr = Array.isArray(payload.mandatory_checks) ? payload.mandatory_checks : [];
      for (const m of arr) {
        if (isObject(m) && typeof m.check_ref === 'string' && m.check_ref.trim() !== '') {
          handoffChecks.add(m.check_ref.trim());
        }
      }
      for (const c of handoffChecks) {
        if (!resolvedMandatoryChecks.has(c)) {
          problems.push(`evidence and handoff "${id}" mandatory check "${c}" is not among the resolved task-specification record's mandatory checks; the full set of mandatory checks is closed against the specification, and "list is non-empty" is not enough`);
        }
      }
      for (const c of resolvedMandatoryChecks) {
        if (!handoffChecks.has(c)) {
          problems.push(`evidence and handoff "${id}" omits mandatory check "${c}" declared by the resolved task-specification record; removing a mandatory check from the handoff is rejected`);
        }
      }
    }

    // 20e. outcome.status "complete" is the WHOLE run finished: the resolved run
    //      is work_status "completed", next_step is null, and every acceptance
    //      criterion is covered by a verified assertion.
    if (outcome.status === 'complete') {
      if (runState && runState.work_status !== 'completed') {
        problems.push(`evidence and handoff "${id}" outcome.status is "complete" but the resolved execution-run record's work_status is "${runState.work_status}", not "completed"; a complete outcome is the whole run finished`);
      }
      if (nextStep.action != null || nextStep.gate != null) {
        problems.push(`evidence and handoff "${id}" outcome.status is "complete" but next_step still carries an executable ${nextStep.action != null ? 'action' : 'gate'}; a completed run has no next step`);
      }
      for (const [cid, cov] of acCoverage) {
        if (cov.status !== 'covered') {
          problems.push(`evidence and handoff "${id}" outcome.status is "complete" but acceptance criterion "${cid}" is not covered`);
          continue;
        }
        const unver = cov.addressedBy.filter((aid) => {
          const a = assertionById.get(aid);
          return !a || a.status !== 'verified';
        });
        if (unver.length) {
          problems.push(`evidence and handoff "${id}" outcome.status is "complete" but acceptance criterion "${cid}" is addressed only through unverified assertion(s) ${unver.join(', ')}`);
        }
      }
    }

    if (runState && isObject(payload.task_specification_ref)
      && typeof runState.task_specification_ref === 'string'
      && typeof payload.task_specification_ref.reference === 'string'
      && runState.task_specification_ref !== payload.task_specification_ref.reference) {
      problems.push(`evidence and handoff "${id}" task_specification_ref names "${payload.task_specification_ref.reference}", but the resolved execution-run record names task specification "${runState.task_specification_ref}"; the handoff carries the SAME specification the run resolved, not an independent one`);
    }

    if (runState) {
      if ('next_action' in runState && !sameStepValue(nextStep.action, runState.next_action)) {
        problems.push(`evidence and handoff "${id}" next_step.action ${JSON.stringify(nextStep.action ?? null)} does not match the resolved execution-run record's next_action ${JSON.stringify(runState.next_action ?? null)}`);
      }
      if ('next_gate' in runState && !sameStepValue(nextStep.gate, runState.next_gate)) {
        problems.push(`evidence and handoff "${id}" next_step.gate ${JSON.stringify(nextStep.gate ?? null)} does not match the resolved execution-run record's next_gate ${JSON.stringify(runState.next_gate ?? null)}`);
      }
      if (Array.isArray(runState.blocker_ids) && !sameRefSet([...blockerIds], runState.blocker_ids)) {
        problems.push(`evidence and handoff "${id}" blockers do not match the resolved execution-run record's blocker_ids; the two sets of blocker ids must be the same`);
      }
      if (Array.isArray(runState.completed_checks)) {
        const completed = new Set(runState.completed_checks.filter((x) => typeof x === 'string').map((x) => x.trim()));
        for (const ref of executedCheckRefs) {
          if (!completed.has(ref)) {
            problems.push(`evidence and handoff "${id}" mandatory check "${ref}" ran (its status is "passed" or "failed") but its check_ref is not among the resolved execution-run record's completed_checks; a check that ran — passed OR failed — is a completed check of the run, and completed_checks confirms the fact it ran, not the verdict`);
          }
        }
        for (const ref of notExecutedCheckRefs) {
          if (completed.has(ref)) {
            problems.push(`evidence and handoff "${id}" mandatory check "${ref}" is "unable" but appears among the resolved execution-run record's completed_checks; a check that could not run is not a completed check`);
          }
        }
      }
      if (TERMINAL_STATUSES.includes(runState.work_status)) {
        for (const f of ['action', 'gate']) {
          if (nextStep[f] !== null && nextStep[f] !== undefined) {
            problems.push(`evidence and handoff "${id}" the resolved execution-run record's work_status is "${runState.work_status}" but next_step.${f} carries an executable value; a completed or cancelled run does not carry a next executable step`);
          }
        }
      }
      if (runState.work_status === 'blocked' && outcome.status !== undefined && outcome.status !== 'blocked') {
        problems.push(`evidence and handoff "${id}" the resolved execution-run record's work_status is "blocked" but outcome.status is "${outcome.status}"`);
      }
    }

    if (hcEntry && isObject(hcEntry) && isObject(payload.execution_run_ref)
      && typeof hcEntry.linked_run_ref === 'string'
      && typeof payload.execution_run_ref.reference === 'string'
      && hcEntry.linked_run_ref !== payload.execution_run_ref.reference) {
      problems.push(`evidence and handoff "${id}" human_control_ref resolves to a run-human-control record whose linked_run_ref is "${hcEntry.linked_run_ref}", not the handoff's pinned run "${payload.execution_run_ref.reference}"`);
    }
    if (cmEntry && isObject(cmEntry) && isObject(payload.execution_run_ref)
      && typeof cmEntry.linked_run_ref === 'string'
      && typeof payload.execution_run_ref.reference === 'string'
      && cmEntry.linked_run_ref !== payload.execution_run_ref.reference) {
      problems.push(`evidence and handoff "${id}" context_manifest_ref resolves to a context-manifest record whose linked_run_ref is "${cmEntry.linked_run_ref}", not the handoff's pinned run "${payload.execution_run_ref.reference}"`);
    }
  }

  return problems;
}
