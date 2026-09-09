// ---------------------------------------------------------------------------
// bounded context manifest: the one checkable implementation
// ---------------------------------------------------------------------------
// registries/operating-model/context-manifest.schema.json is the COMPLETE
// schema for the bounded context manifest of ONE execution run
// (record_type: context-manifest): a record declares it in its own $schema, so
// a consumer that follows the declaration validates the whole record — the
// reused scoped-record envelope AND the specialised body — in one pass.
// Manifest DATA is Instance data (later the working-data area) — the Kernel
// ships the schema, the product-neutral fixtures and this module.
// scripts/kernel-validate.mjs (the `bounded-context-manifest` section) and
// test/context-manifest.test.mjs both call the functions here so the gate and
// the standalone set cannot drift.
//
// Nothing in this module reads process state, touches the filesystem or exits;
// the only Node core it uses is a SHA-256 hash over caller-supplied bytes. It
// takes a parsed record, the two schemas and an external record resolver, and
// returns a flat list of problem strings — empty means valid.
//
// The manifest relates to EXACTLY ONE run, and the link is DETERMINISTIC, not a
// substring heuristic:
//   - execution_run_ref, task_specification_ref and human_control_ref are
//     CLOSED structured pinned references — { record_type, id, reference,
//     run_id?, revision?, sha256? } — never a plain string, never an embedded
//     body;
//   - scope.id equals execution_run_ref.id by exact string comparison
//     (example-run and example-run-other are two different runs);
//   - human_control_ref.run_id equals execution_run_ref.id; task_specification_ref
//     carries no run_id (it is not run-scoped);
//   - run_state_checkpoint is the PINNED authoritative snapshot. It is NOT
//     proven by a second copy of the same fields inside the manifest. Every
//     pinned reference — the manifest's and the checkpoint's — is RESOLVED
//     through a boundary the manifest does not control (a resolver the caller
//     supplies), and the checkpoint's own axes (scope_revision, lifecycle_stage,
//     work_status, next_action, next_gate, blocker_ids, resolved_norms,
//     completed_checks) are checked against the state the RESOLVED execution-run
//     record carries. A fabricated sha256, and a coordinated edit of both the
//     manifest copy and the checkpoint copy, are rejected because the resolved
//     record is the anchor. With no resolver the manifest fails closed.
//
// An exact revision is a CLOSED, fail-closed set (classifyRevision): a full Git
// SHA (40 or 64 hex — not an abbreviation), a strict v?X.Y.Z SemVer tag, or a
// SHA-256 digest. A branch / channel reference (a slash, a bare word, or a
// known moving token) is FLOATING whatever digits it carries. Any other
// provider-specific or unknown revision format is WEAK and needs a sha256. The
// ONE rule is applied identically to pinned references, applicable norms and
// mutable authoritative sources.
//
// Boundaries this module enforces that the JSON Schema subset cannot:
//   - the record's $schema declaration is a portable relative reference that
//     RESOLVES, within the Meridian namespace, to context-manifest.schema.json —
//     not merely a matching basename, and not the bare record envelope. The
//     resolution and the rooted-path rules are REUSED from
//     scripts/lib/task-specification.mjs (resolveSchemaRef, nonPortableReason);
//   - the canonical scoped-record envelope is validated SEPARATELY and is never
//     dropped;
//   - the manifest lives only in scope.type run-state; scope.id identifies the
//     run and scope.workspace_id is mandatory. project-workspace,
//     repository-scope, built-in-methodology, user-profile and
//     organization-profile are rejected with the rationale, and origin.kind
//     built-in is rejected;
//   - the deterministic single-run link above;
//   - only the authoritative sources actually needed to continue are listed;
//     every mutable source is pinned by the closed exact-revision rule or a
//     SHA-256 digest; no source is listed twice by id or by reference;
//   - every applicable norm carries a portable reference, an origin, a pin and
//     an applicability rationale; the SET of references equals the checkpoint's
//     resolved_norms; no norm is listed twice;
//   - decisions and open questions, completed actions and completed checks,
//     known gaps and blockers are separate lists with no duplicate entry;
//   - the manifest's scope revision, next step, next gate, blockers and
//     completed checks agree with the pinned run_state_checkpoint, and the
//     checkpoint in turn agrees with the EXTERNALLY RESOLVED execution-run
//     record; a completed
//     or cancelled checkpoint carries no executable next step; waiting_human
//     names a concrete next action; a non-empty blocker set agrees with
//     work_status blocked and vice versa;
//   - an unbounded material dump and a full evidence / handoff / field-evaluation
//     field that belongs to a later package are rejected;
//   - every reference and prose string is portable;
//   - the human-readable name is stated in Russian for the reader.
// ---------------------------------------------------------------------------
import { createHash } from 'node:crypto';
import { validate } from './json-schema.mjs';
import { nonPortableReason, resolveSchemaRef } from './task-specification.mjs';
import { LIFECYCLE_STAGES, WORK_STATUSES, TERMINAL_STATUSES } from './execution-state.mjs';

// Reused unchanged from the task specification contract: the same Meridian-
// namespace address math and the same rooted-path detection, so this package
// carries no divergent second copy of either rule.
export { nonPortableReason, resolveSchemaRef };

// Reused unchanged from the execution state model: the same closed lifecycle
// and work-status pools, so the checkpoint is checked against one model.
export { LIFECYCLE_STAGES, WORK_STATUSES, TERMINAL_STATUSES };

export const RECORD_TYPE = 'context-manifest';

// The canonical LOGICAL resolution base for a context-manifest record within
// the Meridian namespace. It is a logical address — not a filesystem directory,
// a repository or a storage mechanism. Resolution itself is delegated to
// task-specification.mjs's resolveSchemaRef (see the note above); this constant
// documents the conceptual base and is not re-implemented here.
export const CANONICAL_RECORD_BASE = 'records/context-manifest';
export const SCHEMA_NAMESPACE_DIR = 'registries/operating-model';
export const EXPECTED_SCHEMA_BASENAME = 'context-manifest.schema.json';
export const ENVELOPE_SCHEMA_BASENAME = 'scoped-record.schema.json';
export const EXPECTED_SCHEMA_REF = `${SCHEMA_NAMESPACE_DIR}/${EXPECTED_SCHEMA_BASENAME}`;
export const ENVELOPE_SCHEMA_REF = `${SCHEMA_NAMESPACE_DIR}/${ENVELOPE_SCHEMA_BASENAME}`;

// The one area a manifest may occupy, and why each other area is excluded.
export const ALLOWED_SCOPE_TYPE = 'run-state';
const SCOPE_REJECTION_REASON = {
  'built-in-methodology': 'built-in-methodology is Kernel methodology, not a place for one run\'s context manifest',
  'user-profile': 'user-profile holds a user\'s rules and settings, not a run\'s context manifest',
  'organization-profile': 'organization-profile holds an organisation\'s rules and settings, not a run\'s context manifest',
  'project-workspace': 'project-workspace holds the project\'s goals and decisions; a run\'s context manifest is scoped to run-state',
  'repository-scope': 'repository-scope holds facts true for one repository; a run may span repositories and its manifest is scoped to run-state',
};

// Which record_type each pinned-reference slot must carry, and whether that slot
// carries a run_id (the run it belongs to).
export const PINNED_REF_SLOTS = {
  execution_run_ref: { recordType: 'execution-run', runId: 'self' },
  task_specification_ref: { recordType: 'task-specification', runId: 'forbidden' },
  human_control_ref: { recordType: 'run-human-control', runId: 'required' },
};

// Known moving tokens that are never a pin. A branch or channel name identifies
// a moving target, not a fixed edition.
export const FLOATING_REVISION_TOKENS = new Set([
  'latest', 'current', 'head', 'tip', 'stable', 'unstable', 'newest', 'rolling',
  'edge', 'nightly', 'trunk', 'main', 'master', 'develop', 'dev', 'default', 'release',
]);

// Fields that belong to a LATER package, or that would turn the bounded manifest
// into an unbounded archive. Named here so a leak produces a pointed message,
// not just "additional property not allowed".
export const FORBIDDEN_PAYLOAD_FIELDS = [
  // an unbounded material dump is not a bounded context manifest
  'file_contents', 'full_text', 'full_texts', 'raw_context', 'context_dump',
  'command_log', 'commands', 'transcript', 'messages', 'chat_history',
  'conversation', 'directory_dump', 'dir_listing', 'tree_dump', 'attachments',
  // evidence-and-handoff-contract (package 7)
  'evidence', 'evidence_contract', 'claims', 'claim_evidence_map',
  'handoff', 'handoff_contract', 'external_effects', 'worktree_state',
  'worktree_status', 'verification_verdict', 'check_classification',
  // meridian-field-evaluation (package 8)
  'field_metrics', 'evaluation_metrics', 'observation_log',
  // the bodies of the referenced records are not absorbed
  'task_specification', 'execution_run', 'human_control',
  'goal', 'initial_state', 'target_model', 'task_pattern', 'constraints',
  'acceptance_criteria', 'transition_history', 'switch_history',
  'role_assignments', 'supervision_mode', 'human_authority', 'hitl', 'hotl',
  'communication_mode',
];

const SEMANTIC_ID_RE = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;
const FULL_GIT_SHA_RE = /^[0-9a-f]{40}$/i;
const SHA256_RE = /^[0-9a-fA-F]{64}$/;
const STRICT_SEMVER_TAG_RE = /^v?(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)$/;
const HEX_ONLY_RE = /^[0-9a-f]+$/i;
const BARE_WORD_RE = /^[A-Za-z][A-Za-z0-9]*$/;

const isObject = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);
const blank = (s) => typeof s !== 'string' || s.trim() === '';
const hasCyrillic = (s) => typeof s === 'string' && /[Ѐ-ӿ]/.test(s);
const isPositiveInt = (n) => typeof n === 'number' && Number.isInteger(n) && n > 0;
const sameStepValue = (a, b) => (a == null ? null : a) === (b == null ? null : b);
const isValidSha256 = (s) => typeof s === 'string' && SHA256_RE.test(s.trim());

// The CLOSED, fail-closed classification of a revision string.
//   'absent'   — not a non-empty string
//   'exact'    — a full Git SHA (40 or 64 hex), a strict v?X.Y.Z tag, or a
//                SHA-256 digest — a machine-recognisable fixed edition
//   'floating' — a branch or channel: a known moving token, anything with a
//                slash (feature/x, refs/heads/x), or a bare single word —
//                never a pin whatever digits it carries
//   'abbrev-sha'— an all-hex string that is not a full 40- or 64-hex object
//                name: an abbreviated or ambiguous Git SHA. Needs a sha256, or
//                the full SHA.
//   'weak'     — any other provider-specific or unknown form ("branch-2026",
//                "2026-09-01", …): not proven immutable, so it needs a sha256
//                digest alongside it
export function classifyRevision(rev) {
  if (typeof rev !== 'string') return 'absent';
  const t = rev.trim();
  if (t === '') return 'absent';
  if (FULL_GIT_SHA_RE.test(t) || SHA256_RE.test(t) || STRICT_SEMVER_TAG_RE.test(t)) return 'exact';
  if (FLOATING_REVISION_TOKENS.has(t.toLowerCase())) return 'floating';
  if (t.includes('/')) return 'floating';
  if (HEX_ONLY_RE.test(t)) return 'abbrev-sha';
  if (BARE_WORD_RE.test(t)) return 'floating';
  return 'weak';
}

// Kept for backward-compatible API and the standalone test: a revision that is
// a branch or channel reference.
export function isFloatingRevision(rev) {
  return classifyRevision(rev) === 'floating';
}

// Is this { revision, sha256 } pair a sufficient pin? A sha256 digest always
// is; otherwise the revision must classify as 'exact'. Returns null when
// pinned, or a reason string when not.
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

// Validate one pinned_ref slot. Returns nothing; pushes problems. `runId` is
// execution_run_ref.id (or null when it is not yet known).
function checkPinnedRef(problems, id, field, ref, runId) {
  const slot = PINNED_REF_SLOTS[field.replace(/^run_state_checkpoint\./, '')];
  if (!isObject(ref) || Array.isArray(ref)) {
    problems.push(`context manifest "${id}" ${field} is not a structured pinned reference; it is a closed { record_type, id, reference, revision?/sha256? } object, never a plain string and never an embedded body`);
    return;
  }
  if (ref.record_type !== slot.recordType) {
    problems.push(`context manifest "${id}" ${field} names record_type "${ref.record_type}", not "${slot.recordType}"`);
  }
  if (typeof ref.id !== 'string' || !SEMANTIC_ID_RE.test(ref.id)) {
    problems.push(`context manifest "${id}" ${field} has no stable semantic id`);
  }
  if (blank(ref.reference)) {
    problems.push(`context manifest "${id}" ${field} has no portable reference`);
  } else {
    const r = nonPortableReason(ref.reference);
    if (r) problems.push(`context manifest "${id}" ${field} reference contains ${r}; the reference is portable and is not an absolute machine path`);
  }
  const defect = pinDefect(ref.revision, ref.sha256);
  if (defect) {
    problems.push(`context manifest "${id}" ${field} is not pinned to an exact edition: ${defect}`);
  }
  if (typeof ref.revision === 'string' && ref.revision !== '') {
    const rr = nonPortableReason(ref.revision);
    if (rr) problems.push(`context manifest "${id}" ${field} revision contains ${rr}`);
  }
  // run_id: self for the run, forbidden for the specification, required for the
  // human-control record and equal to the run's id.
  const hasRunId = typeof ref.run_id === 'string' && ref.run_id !== '';
  if (slot.runId === 'forbidden' && hasRunId) {
    problems.push(`context manifest "${id}" ${field} carries run_id "${ref.run_id}"; a task specification is not run-scoped and names no run_id`);
  }
  if (slot.runId === 'required' && !hasRunId) {
    problems.push(`context manifest "${id}" ${field} carries no run_id; a run-human-control record explicitly names the run it belongs to`);
  }
  if (slot.runId === 'self' && hasRunId && ref.run_id !== ref.id) {
    problems.push(`context manifest "${id}" ${field} run_id "${ref.run_id}" is not the run's own id "${ref.id}"`);
  }
  if (slot.runId === 'required' && hasRunId && runId != null && ref.run_id !== runId) {
    problems.push(`context manifest "${id}" ${field} run_id "${ref.run_id}" is not the referenced run "${runId}"; the human-control record must belong to the same run`);
  }
}

// Shared: a list of { id, ... } items — every id present, a semantic id, unique
// within the manifest, every listed prose string portable. `fields` names the
// prose fields to require and portability-check. Returns the set of seen ids.
function checkIdentifiedItems(problems, id, label, list, fields) {
  const seen = new Set();
  const arr = Array.isArray(list) ? list : [];
  arr.forEach((item, i) => {
    const it = isObject(item) ? item : {};
    const iid = typeof it.id === 'string' ? it.id : null;
    const at = iid || `#${i}`;
    if (!iid || !SEMANTIC_ID_RE.test(iid)) {
      problems.push(`context manifest "${id}" ${label} entry ${at} has no stable semantic id`);
    } else {
      if (seen.has(iid)) problems.push(`context manifest "${id}" ${label} id "${iid}" is used more than once`);
      seen.add(iid);
    }
    for (const f of fields) {
      if (blank(it[f])) {
        problems.push(`context manifest "${id}" ${label} entry ${at} has no ${f}`);
      } else {
        const r = nonPortableReason(it[f]);
        if (r) problems.push(`context manifest "${id}" ${label} entry ${at} ${f} contains ${r}`);
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
      problems.push(`context manifest "${id}" ${label}[${i}] is empty or whitespace-only`);
      return;
    }
    const r = nonPortableReason(ref);
    if (r) problems.push(`context manifest "${id}" ${label}[${i}] contains ${r}`);
    const key = ref.trim();
    if (seen.has(key)) problems.push(`context manifest "${id}" ${label} repeats reference "${ref}"`);
    seen.add(key);
  });
  return seen;
}

// ---------------------------------------------------------------------------
// The external record-resolution boundary.
// ---------------------------------------------------------------------------
// The checkpoint is NOT proven by a second copy of the same fields inside the
// manifest. It is proven by RESOLVING each pinned reference through a boundary
// the manifest does not control — a resolver the caller supplies — and checking
// the manifest and the checkpoint against the record that boundary returns.
//
// The resolver is `(pinnedRef) => resolvedEntry | null`. A resolvedEntry is the
// transformer's confirmed view of ONE edition of ONE record, and its shape is a
// CLOSED contract: it is closed to EXACTLY the declared key set, every present
// value is type- and value-checked, and a wrong type produces a closed-contract
// error BEFORE any later comparison — never a silently skipped comparison.
//   {
//     record_type,                 // REQUIRED — a non-empty string, the slot's type
//     id,                          // REQUIRED — a stable semantic id, exactly
//                                  //   the pinned id; an entry without an
//                                  //   identity cannot be checked against the pin
//     reference?,                  // when stated, a non-empty portable string
//                                  //   equal to the resolved reference; otherwise
//                                  //   the canonical identity is (record_type, id)
//     revision?,                   // when the KEY is present: a non-empty string
//                                  //   naming an EXACT edition (a full Git SHA, a
//                                  //   strict v?X.Y.Z tag or a SHA-256 digest) —
//                                  //   a number, null, an object or "" is a
//                                  //   contract error, NOT a missing field, even
//                                  //   with a valid content_digest / source_bytes
//     content_digest?,             // when stated, exactly 64 hexadecimal chars in
//                                  //   the string itself — no surrounding space
//     source_bytes?,               // when stated, a string; it is hashed here and
//                                  //   must agree with a stated content_digest
//     //  ... and at least ONE valid edition confirmation (an exact revision, a
//     //  content_digest, or source_bytes) is mandatory.
//     resolved_state,              // REQUIRED for execution-run — the run's OWN
//                                  //   state, itself a CLOSED object with EXACTLY
//                                  //   these axes, each type- and pool-checked
//                                  //   (next_action / next_gate: null or a
//                                  //   non-empty portable string; blocker_ids:
//                                  //   unique semantic ids; resolved_norms /
//                                  //   completed_checks: unique portable refs):
//       { task_specification_ref, scope_revision, lifecycle_stage, work_status,
//         next_action, next_gate, blocker_ids, resolved_norms, completed_checks }
//     linked_run_ref,              // REQUIRED for run-human-control — a non-empty
//                                  //   portable string, the run it belongs to
//   }
// An unknown key on the entry, or on resolved_state, is a contract violation
// with a pointed reason — never a tolerated extra.
// The resolved records are NOT embedded in manifest.spec: the caller passes a
// companion resolution set (fixtures) or a live transformer, kept outside the
// checked record. The checkpoint's OWN references are resolved too, checked for
// field-for-field equality with the manifest's (§14a) AND for resolving to the
// same canonical identity, edition and digest (§14d) — a checkpoint that pins a
// DIFFERENT existing run, specification or control record is rejected.
export function makeRecordResolver(resolutionMap) {
  const map = isObject(resolutionMap) ? resolutionMap : {};
  return (ref) => {
    if (!isObject(ref) || typeof ref.reference !== 'string') return null;
    const entry = map[ref.reference];
    return isObject(entry) ? entry : null;
  };
}

// The SHA-256 the transformer confirms for a resolved edition: computed here
// from source_bytes when present (and cross-checked against a stated
// content_digest), otherwise the stated content_digest. Returns
// { digest, inconsistent? } — digest is null when the transformer confirmed
// none.
function confirmedDigest(entry) {
  // A stated content_digest counts ONLY when it is a well-formed SHA-256 —
  // EXACTLY 64 hex in the string itself, no surrounding whitespace. Anything
  // else is not a pin; resolvePinnedReference reports it precisely.
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

// Field-for-field equality of two pinned_ref objects — the internal consistency
// layer: the checkpoint pins the SAME record edition the manifest does.
function samePinnedRef(a, b) {
  if (!isObject(a) || !isObject(b)) return false;
  return a.record_type === b.record_type
    && a.id === b.id
    && (a.run_id ?? null) === (b.run_id ?? null)
    && a.reference === b.reference
    && (a.revision ?? null) === (b.revision ?? null)
    && (a.sha256 ?? null) === (b.sha256 ?? null);
}

// Every field a resolved_state (execution-run) MUST carry, and NOTHING else. A
// missing one AND an unknown one are both precise errors, not skipped
// comparisons. next_action / next_gate may be null, so presence is tested with
// `in`, not truthiness.
export const REQUIRED_RESOLVED_STATE_FIELDS = [
  'task_specification_ref', 'scope_revision', 'lifecycle_stage', 'work_status',
  'next_action', 'next_gate', 'blocker_ids', 'resolved_norms', 'completed_checks',
];

// The CLOSED key set of a resolved entry — the transformer's confirmed view of
// ONE edition of ONE record. The entry is closed to EXACTLY the common part
// plus the one slot-specific field; any other key is a contract violation.
export const RESOLVED_ENTRY_COMMON_KEYS = [
  'record_type', 'id', 'reference', 'revision', 'content_digest', 'source_bytes',
];
export const RESOLVED_ENTRY_KEYS_BY_TYPE = {
  'execution-run': [...RESOLVED_ENTRY_COMMON_KEYS, 'resolved_state'],
  'run-human-control': [...RESOLVED_ENTRY_COMMON_KEYS, 'linked_run_ref'],
  'task-specification': [...RESOLVED_ENTRY_COMMON_KEYS],
};

// One resolved_state axis that must be an ARRAY OF UNIQUE STRINGS — semantic ids
// for blocker_ids, portable references for resolved_norms and completed_checks.
// A non-array, a non-string member, a duplicate, or a non-portable / non-semantic
// member is a CLOSED-CONTRACT error here — never a reason for the later axis
// comparison (§14d) to be silently skipped.
function checkResolvedStateArray(problems, id, field, axis, value, kind) {
  const at = `${field}: the resolved execution-run record's resolved_state.${axis}`;
  if (!Array.isArray(value)) {
    problems.push(`context manifest "${id}" ${at} is ${value === null ? 'null' : typeof value}, not an array of ${kind === 'id' ? 'semantic-id' : 'portable-reference'} strings`);
    return;
  }
  const seen = new Set();
  value.forEach((el, i) => {
    if (typeof el !== 'string' || el.trim() === '') {
      problems.push(`context manifest "${id}" ${at}[${i}] is not a non-empty string`);
      return;
    }
    const key = el.trim();
    if (seen.has(key)) problems.push(`context manifest "${id}" ${at} repeats "${el}"`);
    seen.add(key);
    if (kind === 'id') {
      if (!SEMANTIC_ID_RE.test(el)) problems.push(`context manifest "${id}" ${at}[${i}] "${el}" is not a stable semantic id`);
    } else {
      const r = nonPortableReason(el);
      if (r) problems.push(`context manifest "${id}" ${at}[${i}] contains ${r}`);
    }
  });
}

// The CLOSED contract for an execution-run's resolved_state: an object with
// EXACTLY the declared axes (a missing one AND an unknown one are both precise
// errors), each axis type- and value-checked against the same closed pools the
// execution state model uses. A wrong type here becomes a contract error, so
// the later "checkpoint axis vs resolved run" comparison (§14d) never silently
// passes on a value it could not compare.
function checkResolvedState(problems, id, field, rs) {
  if (!isObject(rs)) {
    problems.push(`context manifest "${id}" ${field}: the resolver returned an execution-run record with no resolved_state; the checkpoint axes cannot be checked against the run's own state`);
    return;
  }
  for (const f of REQUIRED_RESOLVED_STATE_FIELDS) {
    if (!(f in rs)) {
      problems.push(`context manifest "${id}" ${field}: the resolved execution-run record's resolved_state is missing required field "${f}"`);
    }
  }
  for (const k of Object.keys(rs)) {
    if (!REQUIRED_RESOLVED_STATE_FIELDS.includes(k)) {
      problems.push(`context manifest "${id}" ${field}: the resolved execution-run record's resolved_state carries an unknown field "${k}"; resolved_state is closed to its ${REQUIRED_RESOLVED_STATE_FIELDS.length} declared axes`);
    }
  }
  if ('task_specification_ref' in rs) {
    if (blank(rs.task_specification_ref)) {
      problems.push(`context manifest "${id}" ${field}: the resolved execution-run record's resolved_state.task_specification_ref is not a non-empty string`);
    } else {
      const r = nonPortableReason(rs.task_specification_ref);
      if (r) problems.push(`context manifest "${id}" ${field}: the resolved execution-run record's resolved_state.task_specification_ref contains ${r}`);
    }
  }
  if ('scope_revision' in rs && !isPositiveInt(rs.scope_revision)) {
    problems.push(`context manifest "${id}" ${field}: the resolved execution-run record's resolved_state.scope_revision ${JSON.stringify(rs.scope_revision)} is not a positive integer`);
  }
  if ('lifecycle_stage' in rs && !LIFECYCLE_STAGES.includes(rs.lifecycle_stage)) {
    problems.push(`context manifest "${id}" ${field}: the resolved execution-run record's resolved_state.lifecycle_stage ${JSON.stringify(rs.lifecycle_stage)} is not one of the closed lifecycle pool`);
  }
  if ('work_status' in rs && !WORK_STATUSES.includes(rs.work_status)) {
    problems.push(`context manifest "${id}" ${field}: the resolved execution-run record's resolved_state.work_status ${JSON.stringify(rs.work_status)} is not one of the closed work-status pool`);
  }
  for (const f of ['next_action', 'next_gate']) {
    if (f in rs && rs[f] !== null) {
      if (blank(rs[f])) {
        problems.push(`context manifest "${id}" ${field}: the resolved execution-run record's resolved_state.${f} is present but neither null nor a non-empty string`);
      } else {
        const r = nonPortableReason(rs[f]);
        if (r) problems.push(`context manifest "${id}" ${field}: the resolved execution-run record's resolved_state.${f} contains ${r}`);
      }
    }
  }
  if ('blocker_ids' in rs) checkResolvedStateArray(problems, id, field, 'blocker_ids', rs.blocker_ids, 'id');
  if ('resolved_norms' in rs) checkResolvedStateArray(problems, id, field, 'resolved_norms', rs.resolved_norms, 'ref');
  if ('completed_checks' in rs) checkResolvedStateArray(problems, id, field, 'completed_checks', rs.completed_checks, 'ref');
}

// Resolve ONE pinned reference through the boundary and check it against the
// CLOSED transformer-response contract: exactly the declared key set, a real
// record type, a stable semantic id matching the pin, a well-formed and matching
// stated reference, every stated edition confirmation well-formed with at least
// one present, and the slot-specific completeness (resolved_state — itself a
// closed object — for execution-run; a portable linked_run_ref for
// run-human-control). Returns the resolved entry, or null when it could not be
// resolved (a problem is pushed).
function resolvePinnedReference(problems, id, field, ref, resolve) {
  if (!isObject(ref) || Array.isArray(ref)) return null;
  const slot = PINNED_REF_SLOTS[field.replace(/^run_state_checkpoint\./, '')];
  const wanted = slot ? slot.recordType : null;
  const entry = resolve(ref);
  if (!isObject(entry)) {
    problems.push(`context manifest "${id}" ${field} does not resolve to an actual ${wanted || 'record'} through the external resolver; a pinned reference that resolves to nothing is not a verified pin and the manifest fails closed`);
    return null;
  }

  // the entry is CLOSED to exactly the declared key set for its type.
  const allowedKeys = RESOLVED_ENTRY_KEYS_BY_TYPE[wanted] || RESOLVED_ENTRY_COMMON_KEYS;
  for (const k of Object.keys(entry)) {
    if (!allowedKeys.includes(k)) {
      problems.push(`context manifest "${id}" ${field}: the resolver returned a record with an unknown field "${k}"; the transformer response is closed to { ${allowedKeys.join(', ')} }`);
    }
  }

  // record_type — required, non-empty, and the slot's type.
  if (typeof entry.record_type !== 'string' || entry.record_type === '') {
    problems.push(`context manifest "${id}" ${field}: the resolver returned a record with no record_type; the transformer response is a closed contract`);
  } else if (wanted && entry.record_type !== wanted) {
    problems.push(`context manifest "${id}" ${field} resolves to a "${entry.record_type}" record, not "${wanted}"`);
  }

  // id — required, a stable semantic identifier, and an exact match of the pin.
  if (typeof entry.id !== 'string' || entry.id.trim() === '') {
    problems.push(`context manifest "${id}" ${field}: the resolver returned a ${wanted || 'record'} with no id; a resolved record without an identity cannot be checked against the pin`);
  } else if (!SEMANTIC_ID_RE.test(entry.id)) {
    problems.push(`context manifest "${id}" ${field}: the resolver returned a ${wanted || 'record'} whose id "${entry.id}" is not a stable semantic identifier`);
  } else if (typeof ref.id === 'string' && entry.id !== ref.id) {
    problems.push(`context manifest "${id}" ${field} pins id "${ref.id}" but the reference resolves to record id "${entry.id}"`);
  }

  // reference — when stated, a non-empty portable string equal to the resolved
  // reference; otherwise the canonical identity is (record_type, id), above.
  if ('reference' in entry) {
    if (typeof entry.reference !== 'string' || entry.reference.trim() === '') {
      problems.push(`context manifest "${id}" ${field}: the resolver's stated reference is present but not a non-empty string`);
    } else {
      const r = nonPortableReason(entry.reference);
      if (r) {
        problems.push(`context manifest "${id}" ${field}: the resolver's stated reference contains ${r}`);
      } else if (typeof ref.reference === 'string' && entry.reference !== ref.reference) {
        problems.push(`context manifest "${id}" ${field}: the resolver's stated reference "${entry.reference}" is not the resolved reference "${ref.reference}"`);
      }
    }
  }

  // pinned edition — every stated confirmation is well-formed, and at least one
  // VALID confirmation is mandatory.
  //
  // content_digest: EXACTLY 64 hex in the string itself — a value with
  // surrounding whitespace is malformed, not merely padded.
  if ('content_digest' in entry
    && (typeof entry.content_digest !== 'string' || !SHA256_RE.test(entry.content_digest))) {
    problems.push(`context manifest "${id}" ${field}: the resolver's content_digest ${JSON.stringify(entry.content_digest)} is not exactly 64 hexadecimal characters`);
  }
  if ('source_bytes' in entry && typeof entry.source_bytes !== 'string') {
    problems.push(`context manifest "${id}" ${field}: the resolver's source_bytes is ${entry.source_bytes === null ? 'null' : typeof entry.source_bytes}, not a string`);
  }
  // revision: when the KEY is present it must be a non-empty string naming an
  // exact edition. A number, null, an object or an empty string is a contract
  // error in its own right — NOT a missing optional field — even when the entry
  // also carries a valid content_digest or source_bytes.
  const hasRevisionString = typeof entry.revision === 'string' && entry.revision.trim() !== '';
  let revisionExact = false;
  if ('revision' in entry) {
    if (typeof entry.revision !== 'string') {
      const shown = entry.revision === null ? 'null'
        : Array.isArray(entry.revision) ? 'an array'
        : ({ number: 'a number', boolean: 'a boolean', object: 'an object' }[typeof entry.revision] || `a ${typeof entry.revision}`);
      problems.push(`context manifest "${id}" ${field}: the resolver's revision is ${shown}, not a string`);
    } else if (entry.revision.trim() === '') {
      problems.push(`context manifest "${id}" ${field}: the resolver's revision is present but empty`);
    } else if (classifyRevision(entry.revision) !== 'exact') {
      problems.push(`context manifest "${id}" ${field}: the resolver confirmed revision ${JSON.stringify(entry.revision)}, which is not an exact edition (a full Git SHA, a strict v?X.Y.Z tag or a SHA-256 digest); the transformer confirms a fixed edition, not a moving reference`);
    } else {
      revisionExact = true;
    }
  }
  const { digest, inconsistent } = confirmedDigest(entry);
  if (!revisionExact && !digest) {
    problems.push(`context manifest "${id}" ${field}: the resolver confirmed neither an exact revision nor a content digest for this edition; the pinned edition is unverified`);
  }
  if (inconsistent) {
    problems.push(`context manifest "${id}" ${field}: the resolver's content_digest "${inconsistent}" does not match the SHA-256 of the resolved source bytes "${digest}"`);
  }
  if (typeof ref.revision === 'string' && ref.revision.trim() !== '') {
    if (!hasRevisionString) {
      problems.push(`context manifest "${id}" ${field} pins revision "${ref.revision}" but the resolver confirmed no exact edition for this reference`);
    } else if (entry.revision !== ref.revision) {
      problems.push(`context manifest "${id}" ${field} pins revision "${ref.revision}" but the resolver confirmed edition "${entry.revision}"`);
    }
  }
  if (typeof ref.sha256 === 'string' && ref.sha256.trim() !== '') {
    if (!digest) {
      problems.push(`context manifest "${id}" ${field} pins sha256 "${ref.sha256}" but the resolver confirmed no content digest for this edition; a digest is verified against resolved source, never against a second copy of itself`);
    } else if (digest.toLowerCase() !== ref.sha256.toLowerCase()) {
      problems.push(`context manifest "${id}" ${field} pins sha256 "${ref.sha256}" but the SHA-256 of the resolved source is "${digest}"`);
    }
  }

  // slot-specific completeness — itself a closed contract.
  if (wanted === 'execution-run') {
    checkResolvedState(problems, id, field, entry.resolved_state);
  }
  if (wanted === 'run-human-control') {
    if (typeof entry.linked_run_ref !== 'string' || entry.linked_run_ref.trim() === '') {
      problems.push(`context manifest "${id}" ${field}: the resolver returned a run-human-control record with no linked_run_ref; the run it belongs to is unconfirmed`);
    } else {
      const r = nonPortableReason(entry.linked_run_ref);
      if (r) problems.push(`context manifest "${id}" ${field}: the resolved run-human-control record's linked_run_ref contains ${r}`);
    }
  }

  return entry;
}

// The manifest's reference and the checkpoint's reference for the same slot,
// resolved SEPARATELY, must land on one canonical identity, one edition and one
// digest. A checkpoint that resolves to a different — even if real — record is
// rejected.
function sameResolvedEdition(problems, id, slotField, manifestEntry, checkpointEntry) {
  if (!isObject(manifestEntry) || !isObject(checkpointEntry)) return;
  if (typeof manifestEntry.id === 'string' && typeof checkpointEntry.id === 'string'
    && manifestEntry.id !== checkpointEntry.id) {
    problems.push(`context manifest "${id}" run_state_checkpoint.${slotField} resolves to record "${checkpointEntry.id}", not the manifest's "${manifestEntry.id}"; the checkpoint pins the SAME record, not a parallel one that also exists`);
  }
  if ((manifestEntry.record_type ?? null) !== (checkpointEntry.record_type ?? null)) {
    problems.push(`context manifest "${id}" run_state_checkpoint.${slotField} resolves to a "${checkpointEntry.record_type}" record, not the manifest's "${manifestEntry.record_type}"`);
  }
  if ((manifestEntry.revision ?? null) !== (checkpointEntry.revision ?? null)) {
    problems.push(`context manifest "${id}" run_state_checkpoint.${slotField} resolves to edition ${JSON.stringify(checkpointEntry.revision ?? null)}, not the manifest's ${JSON.stringify(manifestEntry.revision ?? null)}; both must pin the same edition`);
  }
  const dm = confirmedDigest(manifestEntry).digest;
  const dc = confirmedDigest(checkpointEntry).digest;
  if (dm && dc && dm.toLowerCase() !== dc.toLowerCase()) {
    problems.push(`context manifest "${id}" run_state_checkpoint.${slotField} resolves to a source whose digest differs from the manifest's ${slotField}`);
  }
}

// Set equality over the string members of two arrays.
function sameRefSet(a, b) {
  if (!Array.isArray(a) || !Array.isArray(b)) return true;
  const sa = new Set(a.filter((x) => typeof x === 'string').map((x) => x.trim()));
  const sb = new Set(b.filter((x) => typeof x === 'string').map((x) => x.trim()));
  return sa.size === sb.size && [...sa].every((x) => sb.has(x));
}

// The whole composition pipeline for one context-manifest record: the canonical
// record envelope (reused, always run), the complete specialised schema (which
// a $schema-follower would also use), then the cross-cutting rules the JSON
// Schema subset cannot state.
export function evaluateContextManifest(doc, { recordSchema, envelopeSchema, resolveRecords } = {}) {
  const problems = [];

  // 1. the canonical scoped-record envelope, validated separately and always.
  const envErrs = [];
  try { validate(doc, envelopeSchema, envelopeSchema, '', envErrs); }
  catch (e) { return [`record envelope schema could not be applied: ${e.message}`]; }
  problems.push(...envErrs.map((m) => `envelope ${m}`));

  // 2. the complete specialised schema — the same one a record declares.
  const bodyErrs = [];
  try { validate(doc, recordSchema, recordSchema, '', bodyErrs); }
  catch (e) { return [`context-manifest schema could not be applied: ${e.message}`]; }
  problems.push(...bodyErrs);

  if (!isObject(doc)) {
    return problems.length ? problems : ['the context manifest record is not an object'];
  }

  const id = typeof doc.id === 'string' && doc.id !== '' ? doc.id : '(no id)';

  // 3. the record's $schema declaration must be a portable relative reference
  //    that RESOLVES, within the Meridian namespace, to the specialised schema
  //    — not merely carry the right basename, and not the bare record envelope.
  const declared = doc.$schema;
  if (typeof declared !== 'string' || declared === '') {
    problems.push(`context manifest "${id}" declares no $schema; a record names ${EXPECTED_SCHEMA_BASENAME} so a consumer validates the whole contract — id, scope, origin, authority and body — not only the envelope`);
  } else {
    const portability = nonPortableReason(declared);
    if (portability) {
      problems.push(`context manifest "${id}" $schema "${declared}" is not portable (${portability}); the schema is named by a relative reference resolved inside the Meridian namespace (canonical logical base ${CANONICAL_RECORD_BASE})`);
    } else {
      const resolved = resolveSchemaRef(declared);
      if (resolved === ENVELOPE_SCHEMA_REF) {
        problems.push(`context manifest "${id}" $schema "${declared}" resolves to the record envelope (${ENVELOPE_SCHEMA_BASENAME}); it must name ${EXPECTED_SCHEMA_BASENAME}, which composes the envelope with the body`);
      } else if (resolved !== EXPECTED_SCHEMA_REF) {
        problems.push(`context manifest "${id}" $schema "${declared}" does not resolve to the logical address ${EXPECTED_SCHEMA_REF} within the Meridian namespace; the specialised schema is named by a portable relative reference (a bare basename, a missing namespace segment, a wrong segment and a reference climbing out of the namespace all resolve elsewhere)`);
      }
    }
  }

  // 4. identity and the human-readable Russian name come from the envelope.
  if (doc.record_type !== RECORD_TYPE) {
    problems.push(`context manifest "${id}" declares record_type "${doc.record_type}", not "${RECORD_TYPE}"; the record type names the manifest and does not open a second envelope`);
  }
  if (typeof doc.id !== 'string' || !SEMANTIC_ID_RE.test(doc.id)) {
    problems.push(`context manifest "${id}" has no stable semantic id on the record envelope`);
  }
  if (typeof doc.title !== 'string' || doc.title.trim() === '') {
    problems.push(`context manifest "${id}" has no human-readable title`);
  } else if (!hasCyrillic(doc.title)) {
    problems.push(`context manifest "${id}" title "${doc.title}" carries no Russian (Cyrillic) text; the manifest name is stated in Russian for the human reader`);
  }
  const titleReason = nonPortableReason(doc.title);
  if (titleReason) problems.push(`context manifest "${id}" title contains ${titleReason}`);

  // 5. the manifest lives only in run-state; scope.id identifies the run and
  //    scope.workspace_id is mandatory. The physical directory, the Instance
  //    repository and the storage mechanism are not the area.
  const scope = isObject(doc.scope) ? doc.scope : {};
  if (typeof scope.type === 'string' && scope.type !== ALLOWED_SCOPE_TYPE) {
    const why = SCOPE_REJECTION_REASON[scope.type] || 'it is not run-state';
    problems.push(`context manifest "${id}" is scoped to "${scope.type}"; a context manifest lives in run-state only — ${why}`);
  }
  if (scope.type === ALLOWED_SCOPE_TYPE) {
    if (typeof scope.id !== 'string' || !SEMANTIC_ID_RE.test(scope.id)) {
      problems.push(`context manifest "${id}" run-state scope carries no stable scope.id identifying the run`);
    }
    if (typeof scope.workspace_id !== 'string' || !SEMANTIC_ID_RE.test(scope.workspace_id)) {
      problems.push(`context manifest "${id}" run-state scope carries no workspace_id; a run belongs to a project workspace (workspace-scope-model.md §1)`);
    }
  }

  const origin = isObject(doc.origin) ? doc.origin : {};
  if (origin.kind === 'built-in') {
    problems.push(`context manifest "${id}" declares origin.kind "built-in"; a manifest is written in a workspace, not shipped with the methodology`);
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
      if (r) problems.push(`context manifest "${id}" ${label} contains ${r}; a manifest is portable and carries no rooted machine path`);
    }
  }

  const payload = isObject(doc.payload) ? doc.payload : {};

  // 6. an unbounded material dump, or a field of a later package.
  for (const f of FORBIDDEN_PAYLOAD_FIELDS) {
    if (f in payload) {
      problems.push(`context manifest "${id}" payload carries "${f}"; the manifest is a bounded list of sources and state — an unbounded material dump, a full evidence / handoff contract or the body of a referenced record belongs elsewhere, not here`);
    }
  }
  if ('record_type' in payload) {
    problems.push(`context manifest "${id}" repeats record_type inside the payload; the record type is declared once, on the envelope`);
  }

  // 7. the three structured pinned references, and the DETERMINISTIC single-run
  //    link: scope.id == execution_run_ref.id, human_control_ref belongs to the
  //    same run, task_specification_ref carries no run_id.
  const runRef = payload.execution_run_ref;
  const runId = isObject(runRef) && typeof runRef.id === 'string' && runRef.id !== '' ? runRef.id : null;
  for (const field of ['execution_run_ref', 'task_specification_ref', 'human_control_ref']) {
    if (!(field in payload)) {
      problems.push(`context manifest "${id}" names no ${field}; a manifest carries a structured pinned reference to exactly one such record`);
      continue;
    }
    checkPinnedRef(problems, id, field, payload[field], runId);
  }
  if (scope.type === ALLOWED_SCOPE_TYPE && typeof scope.id === 'string' && scope.id !== '' && runId != null
    && scope.id !== runId) {
    problems.push(`context manifest "${id}" scope identifies run "${scope.id}" but execution_run_ref.id is "${runId}"; the two must be the exact same string, not one a substring of the other`);
  }

  // 8. the pinned scope revision is a positive integer — enforced once, by the
  //    schema (payload.scope_revision minimum 1). Its agreement with the pinned
  //    run state is §14.

  // 9. authoritative sources: non-empty, every one needed, uniquely identified,
  //    portable, and every MUTABLE source pinned by the closed exact-revision
  //    rule or a SHA-256 digest.
  const sources = Array.isArray(payload.authoritative_sources) ? payload.authoritative_sources : null;
  if (!sources || sources.length === 0) {
    problems.push(`context manifest "${id}" lists no authoritative sources; a resumable run names at least the inputs it needs to continue`);
  } else {
    const seenId = new Set();
    const seenRef = new Set();
    sources.forEach((s, i) => {
      const so = isObject(s) ? s : {};
      const sid = typeof so.id === 'string' ? so.id : null;
      const at = sid || `#${i}`;
      if (!sid || !SEMANTIC_ID_RE.test(sid)) {
        problems.push(`context manifest "${id}" authoritative source ${at} has no stable semantic id`);
      } else {
        if (seenId.has(sid)) problems.push(`context manifest "${id}" authoritative source id "${sid}" is used more than once`);
        seenId.add(sid);
      }
      if (blank(so.reference)) {
        problems.push(`context manifest "${id}" authoritative source ${at} has no reference`);
      } else {
        const r = nonPortableReason(so.reference);
        if (r) problems.push(`context manifest "${id}" authoritative source ${at} reference contains ${r}`);
        const key = so.reference.trim();
        if (seenRef.has(key)) problems.push(`context manifest "${id}" authoritative source reference "${so.reference}" is listed more than once`);
        seenRef.add(key);
      }
      if (blank(so.purpose)) {
        problems.push(`context manifest "${id}" authoritative source ${at} states no purpose; a source not needed to continue is not listed`);
      } else {
        const r = nonPortableReason(so.purpose);
        if (r) problems.push(`context manifest "${id}" authoritative source ${at} purpose contains ${r}`);
      }
      if (typeof so.revision === 'string' && so.revision !== '') {
        const r = nonPortableReason(so.revision);
        if (r) problems.push(`context manifest "${id}" authoritative source ${at} revision contains ${r}`);
      }
      if (so.mutable === true) {
        const defect = pinDefect(so.revision, so.sha256);
        if (defect) problems.push(`context manifest "${id}" authoritative source ${at} is mutable but not pinned: ${defect}`);
      } else if (classifyRevision(so.revision) === 'floating') {
        problems.push(`context manifest "${id}" authoritative source ${at} is declared immutable but its revision ${JSON.stringify(so.revision)} is a branch or channel reference`);
      }
    });
  }

  // 10. applicable norms: reference, origin, a pin and an applicability
  //     rationale; no norm listed twice.
  const norms = Array.isArray(payload.applicable_norms) ? payload.applicable_norms : [];
  const normRefs = new Set();
  {
    const seenId = new Set();
    norms.forEach((n, i) => {
      const no = isObject(n) ? n : {};
      const nid = typeof no.id === 'string' ? no.id : null;
      const at = nid || `#${i}`;
      if (!nid || !SEMANTIC_ID_RE.test(nid)) {
        problems.push(`context manifest "${id}" applicable norm ${at} has no stable semantic id`);
      } else {
        if (seenId.has(nid)) problems.push(`context manifest "${id}" applicable norm id "${nid}" is used more than once`);
        seenId.add(nid);
      }
      for (const [f, human] of [['reference', 'reference'], ['origin', 'origin'], ['applicability_rationale', 'applicability rationale']]) {
        if (blank(no[f])) {
          problems.push(`context manifest "${id}" applicable norm ${at} has no ${human}`);
        } else {
          const r = nonPortableReason(no[f]);
          if (r) problems.push(`context manifest "${id}" applicable norm ${at} ${f} contains ${r}`);
        }
      }
      if (typeof no.reference === 'string' && no.reference.trim() !== '') {
        const key = no.reference.trim();
        if (normRefs.has(key)) problems.push(`context manifest "${id}" applicable norm reference "${no.reference}" is listed more than once`);
        normRefs.add(key);
      }
      if (typeof no.revision === 'string' && no.revision !== '') {
        const r = nonPortableReason(no.revision);
        if (r) problems.push(`context manifest "${id}" applicable norm ${at} revision contains ${r}`);
      }
      const defect = pinDefect(no.revision, no.sha256);
      if (defect) problems.push(`context manifest "${id}" applicable norm ${at} is not pinned: ${defect}`);
    });
  }

  // 11. decisions and open questions, known gaps — separate lists, each entry
  //     identified, unique and portable.
  checkIdentifiedItems(problems, id, 'decisions', payload.decisions, ['statement']);
  checkIdentifiedItems(problems, id, 'open_questions', payload.open_questions, ['question']);
  checkIdentifiedItems(problems, id, 'known_gaps', payload.known_gaps, ['description']);

  // 12. completed actions and completed checks — separate lists of unique
  //     portable references.
  checkRefList(problems, id, 'completed_actions', payload.completed_actions);
  checkRefList(problems, id, 'completed_checks', payload.completed_checks);

  // 13. blockers — each structured with a verifiable resumption condition.
  const blockers = Array.isArray(payload.blockers) ? payload.blockers : [];
  const blockerIds = new Set();
  blockers.forEach((b, i) => {
    const bo = isObject(b) ? b : {};
    const bid = typeof bo.id === 'string' ? bo.id : null;
    const at = bid || `#${i}`;
    if (!bid || !SEMANTIC_ID_RE.test(bid)) {
      problems.push(`context manifest "${id}" blocker ${at} has no stable id`);
    } else {
      if (blockerIds.has(bid)) problems.push(`context manifest "${id}" blocker id "${bid}" is used more than once`);
      blockerIds.add(bid);
    }
    if (blank(bo.description)) problems.push(`context manifest "${id}" blocker ${at} has no description`);
    if (blank(bo.resumption_condition)) problems.push(`context manifest "${id}" blocker ${at} has no verifiable resumption condition`);
    for (const s of [bo.description, bo.resumption_condition]) {
      const r = nonPortableReason(s);
      if (r) problems.push(`context manifest "${id}" blocker ${at} contains ${r}`);
    }
  });

  // 14. the run_state_checkpoint — the PINNED authoritative snapshot. Its
  //     agreement with the manifest is a consistency layer (§14b); its PROOF is
  //     resolution against the actual execution-run record (§14d).
  const cp = isObject(payload.run_state_checkpoint) ? payload.run_state_checkpoint : null;
  if (!cp) {
    if ('run_state_checkpoint' in payload) {
      problems.push(`context manifest "${id}" run_state_checkpoint is not an object`);
    }
  } else {
    // 14a. the checkpoint's own pinned references are well-formed structured
    //      pins AND equal the manifest's field for field — the internal
    //      consistency layer. §14d then resolves both through the external
    //      boundary and confirms they land on one edition, so a checkpoint that
    //      points at a different existing record is caught even if the two
    //      in-document copies were edited together.
    for (const field of ['execution_run_ref', 'task_specification_ref', 'human_control_ref']) {
      if (!(field in cp)) {
        problems.push(`context manifest "${id}" run_state_checkpoint names no ${field}; the pinned snapshot carries its own pinned references`);
        continue;
      }
      checkPinnedRef(problems, id, `run_state_checkpoint.${field}`, cp[field], runId);
      if (field in payload && !samePinnedRef(cp[field], payload[field])) {
        problems.push(`context manifest "${id}" run_state_checkpoint.${field} does not equal the manifest's ${field} field for field; the checkpoint pins the SAME record edition as the manifest, not a parallel one`);
      }
    }

    if (typeof cp.lifecycle_stage === 'string' && !LIFECYCLE_STAGES.includes(cp.lifecycle_stage)) {
      problems.push(`context manifest "${id}" run_state_checkpoint.lifecycle_stage "${cp.lifecycle_stage}" is not one of the closed lifecycle pool`);
    }
    if (typeof cp.work_status === 'string' && !WORK_STATUSES.includes(cp.work_status)) {
      problems.push(`context manifest "${id}" run_state_checkpoint.work_status "${cp.work_status}" is not one of the closed work-status pool`);
    }
    if ('scope_revision' in cp && !isPositiveInt(cp.scope_revision)) {
      problems.push(`context manifest "${id}" run_state_checkpoint.scope_revision "${cp.scope_revision}" is not a positive integer`);
    }
    for (const f of ['next_action', 'next_gate']) {
      if (f in cp && cp[f] !== null && blank(cp[f])) {
        problems.push(`context manifest "${id}" run_state_checkpoint.${f} is present but empty; state an action, or null where there is no continuation`);
      }
      const r = nonPortableReason(cp[f]);
      if (r) problems.push(`context manifest "${id}" run_state_checkpoint.${f} contains ${r}`);
    }
    const cpChecks = checkRefList(problems, id, 'run_state_checkpoint.completed_checks', cp.completed_checks);
    const cpNorms = checkRefList(problems, id, 'run_state_checkpoint.resolved_norms', cp.resolved_norms);
    const cpBlockerIds = new Set(Array.isArray(cp.blocker_ids) ? cp.blocker_ids.filter((x) => typeof x === 'string') : []);

    // 14b. the manifest's own axes agree with the checkpoint.
    if (isPositiveInt(payload.scope_revision) && isPositiveInt(cp.scope_revision)
      && payload.scope_revision !== cp.scope_revision) {
      problems.push(`context manifest "${id}" scope_revision ${payload.scope_revision} contradicts the pinned run state (run_state_checkpoint.scope_revision ${cp.scope_revision})`);
    }
    for (const f of ['next_action', 'next_gate']) {
      if (f in payload && f in cp && !sameStepValue(payload[f], cp[f])) {
        problems.push(`context manifest "${id}" ${f} ${JSON.stringify(payload[f])} contradicts the pinned run state (run_state_checkpoint.${f} ${JSON.stringify(cp[f])})`);
      }
    }
    if (Array.isArray(cp.blocker_ids)) {
      const extraInManifest = [...blockerIds].filter((x) => !cpBlockerIds.has(x));
      const missingInManifest = [...cpBlockerIds].filter((x) => !blockerIds.has(x));
      if (extraInManifest.length || missingInManifest.length) {
        problems.push(`context manifest "${id}" blockers do not match the pinned run state (run_state_checkpoint.blocker_ids); the two sets of blocker ids must be the same`);
      }
    }
    if (Array.isArray(cp.completed_checks) && Array.isArray(payload.completed_checks)) {
      const manifestChecks = new Set(payload.completed_checks.filter((x) => typeof x === 'string').map((x) => x.trim()));
      const mismatch = manifestChecks.size !== cpChecks.size || [...manifestChecks].some((x) => !cpChecks.has(x));
      if (mismatch) {
        problems.push(`context manifest "${id}" completed_checks do not match the pinned run state (run_state_checkpoint.completed_checks); the two sets must be the same`);
      }
    }
    // the applicable-norm reference set MUST equal the run's resolved_norms:
    // the manifest adds an applicability rationale, it does not substitute the
    // resolved set.
    if (Array.isArray(cp.resolved_norms)) {
      const extra = [...normRefs].filter((x) => !cpNorms.has(x));
      const missing = [...cpNorms].filter((x) => !normRefs.has(x));
      if (extra.length || missing.length) {
        problems.push(`context manifest "${id}" applicable_norms references do not match the pinned run state (run_state_checkpoint.resolved_norms); the manifest adds an applicability rationale but cannot add to or drop from the run's resolved norm set`);
      }
    }

    // 14c. the checkpoint's own axis consistency — the same rules the execution
    //      state model states, applied to the pinned snapshot.
    const ws = cp.work_status;
    if (ws === 'blocked' && cpBlockerIds.size === 0) {
      problems.push(`context manifest "${id}" run_state_checkpoint work_status is "blocked" with no blocker id; "blocked" means continuation is impossible until a stated external condition changes`);
    }
    if (ws !== 'blocked' && ws !== undefined && cpBlockerIds.size > 0) {
      problems.push(`context manifest "${id}" run_state_checkpoint carries ${cpBlockerIds.size} blocker id(s) but work_status is "${ws}"; a non-empty blocker set agrees with work_status "blocked"`);
    }
    if (TERMINAL_STATUSES.includes(ws)) {
      for (const f of ['next_action', 'next_gate']) {
        if (cp[f] !== null && cp[f] !== undefined) {
          problems.push(`context manifest "${id}" run_state_checkpoint work_status is "${ws}" but ${f} carries an executable value; a completed or cancelled run does not silently carry a next executable step`);
        }
      }
    }
    if (ws === 'waiting_human' && blank(cp.next_action)) {
      problems.push(`context manifest "${id}" run_state_checkpoint work_status is "waiting_human" but next_action names no concrete human action; "waiting_human" means a specific required human action is known`);
    }
  }

  // 14d. THE EXTERNAL RESOLUTION BOUNDARY. Resolve the manifest's and the
  //      checkpoint's pinned references through a resolver the manifest does not
  //      control, and check the checkpoint's snapshot axes against the state the
  //      RESOLVED execution-run record carries. Two identical copies of a
  //      reference inside one document are not proof; a fabricated sha256, and a
  //      coordinated edit of both the manifest copy and the checkpoint copy, are
  //      rejected here because the resolved record is the anchor. With no
  //      resolver the manifest cannot be verified and fails closed.
  const resolve = typeof resolveRecords === 'function' ? resolveRecords : null;
  if (!resolve) {
    problems.push(`context manifest "${id}" cannot be verified: no external record resolver was supplied; the pinned execution-run, task-specification and run-human-control records are resolved OUTSIDE the manifest and checked against it — without that boundary the run_state_checkpoint is only a second unanchored copy`);
  } else {
    // Resolve the MANIFEST's three references.
    const runTop = ('execution_run_ref' in payload)
      ? resolvePinnedReference(problems, id, 'execution_run_ref', payload.execution_run_ref, resolve) : null;
    const specTop = ('task_specification_ref' in payload)
      ? resolvePinnedReference(problems, id, 'task_specification_ref', payload.task_specification_ref, resolve) : null;
    const hcTop = ('human_control_ref' in payload)
      ? resolvePinnedReference(problems, id, 'human_control_ref', payload.human_control_ref, resolve) : null;

    // Resolve the CHECKPOINT's own three references — separately — and keep the
    // results. They are not discarded: the axes below are checked against the
    // run the checkpoint itself points at.
    let runCp = null;
    let hcCp = null;
    if (cp) {
      runCp = ('execution_run_ref' in cp)
        ? resolvePinnedReference(problems, id, 'run_state_checkpoint.execution_run_ref', cp.execution_run_ref, resolve) : null;
      const specCp = ('task_specification_ref' in cp)
        ? resolvePinnedReference(problems, id, 'run_state_checkpoint.task_specification_ref', cp.task_specification_ref, resolve) : null;
      hcCp = ('human_control_ref' in cp)
        ? resolvePinnedReference(problems, id, 'run_state_checkpoint.human_control_ref', cp.human_control_ref, resolve) : null;

      // the checkpoint's resolved records ARE the manifest's — same canonical
      // identity, same edition, same digest. A different real record is rejected.
      sameResolvedEdition(problems, id, 'execution_run_ref', runTop, runCp);
      sameResolvedEdition(problems, id, 'task_specification_ref', specTop, specCp);
      sameResolvedEdition(problems, id, 'human_control_ref', hcTop, hcCp);
    }

    // The axes are checked against the run resolved FROM run_state_checkpoint's
    // OWN execution_run_ref — and sameResolvedEdition above has confirmed that is
    // the same edition as the manifest's top-level execution_run_ref.
    const runForAxes = cp ? runCp : null;
    const runState = runForAxes && isObject(runForAxes.resolved_state) ? runForAxes.resolved_state : null;

    // the resolved execution-run's OWN view of the specification it resolved.
    // resolvePinnedReference has already run the CLOSED resolved_state contract
    // (checkResolvedState): a wrong-typed axis has produced a precise
    // closed-contract error above, so the type guards below are a crash guard,
    // NOT the basis for accepting an unvalidated value.
    if (runState && isObject(payload.task_specification_ref)
      && typeof runState.task_specification_ref === 'string'
      && runState.task_specification_ref !== payload.task_specification_ref.reference) {
      problems.push(`context manifest "${id}" task_specification_ref names "${payload.task_specification_ref.reference}", but the resolved execution-run record names task specification "${runState.task_specification_ref}"; the manifest carries the SAME specification the run resolved, not an independent one`);
    }

    // the resolved run-human-control record's OWN view of its run
    const hcForLink = cp ? hcCp : null;
    if (hcForLink && isObject(hcForLink) && isObject(payload.execution_run_ref)
      && typeof hcForLink.linked_run_ref === 'string'
      && hcForLink.linked_run_ref !== payload.execution_run_ref.reference) {
      problems.push(`context manifest "${id}" human_control_ref resolves to a run-human-control record whose linked_run_ref is "${hcForLink.linked_run_ref}", not the manifest's pinned run "${payload.execution_run_ref.reference}"`);
    }

    // the checkpoint snapshot IS the run's own resolved state, not a copy of it.
    if (cp && runState) {
      for (const [axis, want] of [
        ['scope_revision', runState.scope_revision],
        ['lifecycle_stage', runState.lifecycle_stage],
        ['work_status', runState.work_status],
      ]) {
        if (want !== undefined && cp[axis] !== undefined && cp[axis] !== want) {
          problems.push(`context manifest "${id}" run_state_checkpoint.${axis} ${JSON.stringify(cp[axis])} does not match the resolved execution-run record (${JSON.stringify(want)}); the pinned snapshot reproduces the run's own resolved state, not an unanchored copy`);
        }
      }
      for (const [axis, want] of [['next_action', runState.next_action], ['next_gate', runState.next_gate]]) {
        if (want !== undefined && cp[axis] !== undefined && !sameStepValue(cp[axis], want)) {
          problems.push(`context manifest "${id}" run_state_checkpoint.${axis} ${JSON.stringify(cp[axis])} does not match the resolved execution-run record (${JSON.stringify(want)}); the pinned snapshot reproduces the run's own resolved state, not an unanchored copy`);
        }
      }
      for (const [axis, want] of [
        ['blocker_ids', runState.blocker_ids],
        ['resolved_norms', runState.resolved_norms],
        ['completed_checks', runState.completed_checks],
      ]) {
        // `want` is a validated array of strings (checkResolvedState) OR a
        // closed-contract error is already recorded; the isArray guard here is
        // a crash guard, not a licence to accept an unvalidated axis.
        if (Array.isArray(want) && Array.isArray(cp[axis]) && !sameRefSet(cp[axis], want)) {
          problems.push(`context manifest "${id}" run_state_checkpoint.${axis} does not match the resolved execution-run record's ${axis}; the pinned snapshot reproduces the run's own resolved set, not an unanchored copy`);
        }
      }
    }
  }

  // 15. the manifest's own next_action / next_gate, stated explicitly.
  for (const f of ['next_action', 'next_gate']) {
    if (f in payload && payload[f] !== null && blank(payload[f])) {
      problems.push(`context manifest "${id}" ${f} is present but empty; state an action, or null where there is no continuation`);
    }
    const r = nonPortableReason(payload[f]);
    if (r) problems.push(`context manifest "${id}" ${f} contains ${r}`);
  }

  return problems;
}
