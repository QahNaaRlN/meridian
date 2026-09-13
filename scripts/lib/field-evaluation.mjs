// ---------------------------------------------------------------------------
// meridian-field-evaluation: the one checkable implementation
// ---------------------------------------------------------------------------
// registries/operating-model/field-evaluation.schema.json is the COMPLETE
// schema for TWO record types that compose the practical-evaluation contract:
//   - record_type: field-evaluation-observation — one measured data point about
//     ONE metric of ONE execution run, pinned to that run and grounded by
//     externally-resolved evidence;
//   - record_type: field-evaluation-report — a DETERMINISTIC aggregation,
//     scoped to a project workspace over a stated period, built from a pinned
//     set of included observations (and an explicit, reasoned set of excluded
//     ones), never a single rolled-up score.
// A record declares the shared schema in its own $schema, so a consumer that
// follows the declaration validates the whole record — the reused
// scoped-record envelope AND the specialised body — in one pass. Evaluation
// DATA is Instance data (later the working-data area) — the Kernel ships the
// schema, the product-neutral fixtures and this module.
// scripts/kernel-validate.mjs (the `meridian-field-evaluation` section) and
// test/field-evaluation.test.mjs both call the functions here so the gate and
// the standalone set cannot drift.
//
// Nothing in this module reads process state, touches the filesystem or
// exits; the only Node core it uses is a SHA-256 hash over caller-supplied
// bytes. It takes a parsed record, the two schemas and an external record
// resolver, and returns a flat list of problem strings — empty means valid.
//
// The EIGHT characteristics of plan §10 are modelled SEPARATELY, never folded
// into one score:
//   mechanism-correctness, context-entry-time, rework-returns,
//   stop-correctness, missed-norms, resumption-success, owner-cost,
//   post-acceptance-defects.
// Each is a CLOSED measurement kind — classification / duration / count — and
// a classification metric's outcome pool is closed PER METRIC (METRIC_ID is
// authoritative for: which kind applies, which outcomes are possible, and
// which single outcome the report's primary rate tracks). An observation
// records ONE instance (one run, one metric, one point in time); the REPORT
// aggregates many observations into per-metric counts and rates, recomputed
// from the RESOLVED observations, never taken on the report's own say-so.
//
// Absence of observation is not zero. An observation's status is one of FOUR
// distinct states — observed / unknown / not_applicable / unmeasurable — and
// every non-observed status carries a reason; a ratio's denominator of zero
// never yields 0% or 100% (the aggregate carries no percentage at all), and a
// classification's "unknown" outcome is never silently counted as the
// tracked positive/negative outcome.
//
// An observation, once recorded, is never silently overwritten: a correction
// is a NEW observation that names the superseded one (supersedes) with a
// correction_reason, and a report that includes both the superseded and the
// superseding observation double-counts the same underlying fact and is
// rejected.
//
// Boundaries this module enforces that the JSON Schema subset cannot:
//   - the record's $schema declaration is a portable relative reference that
//     RESOLVES, within the Meridian namespace, to field-evaluation.schema.json
//     (resolveSchemaRef / nonPortableReason reused from
//     scripts/lib/task-specification.mjs via scripts/lib/context-manifest.mjs);
//   - the canonical scoped-record envelope is validated SEPARATELY and is
//     never dropped;
//   - an observation lives only in scope.type run-state (scope.id equals
//     execution_run_ref.id exactly, scope.workspace_id mandatory); a report
//     lives only in scope.type project-workspace; origin.kind built-in is
//     rejected for both;
//   - an observation's measurement kind matches its metric_id's closed kind,
//     its classification outcome is one of that metric's closed pool, a
//     classification carries a non-empty classification basis (the status or
//     work_status alone is never a basis), a duration is non-negative with no
//     impossible interval and distinguishes calendar length from active
//     effort (never summed together), and a count is a non-negative integer;
//   - status 'observed' requires a non-empty evidence list where at least one
//     entry is PINNED (an exact revision or a SHA-256 digest) and RESOLVES
//     through the external boundary to a closed 'evidence-result' carrying
//     observed_result "confirmed" AND a metric_ref confirming it bears on
//     THIS observation's own metric — evidence of a different metric is
//     rejected, and a plain unresolved reference is not proof; every other
//     status forbids both measurement and evidence;
//   - a correction (supersedes) is a pinned reference to another
//     field-evaluation-observation, resolved and checked to actually exist,
//     carrying a correction_reason; the superseded observation's own facts
//     are never edited in place;
//   - a report's included_observations and excluded_observations are pinned,
//     externally resolved, and checked for the same workspace and the same
//     reporting period (scope.workspace_id equality, observed_at inside
//     [period.start_date, period.end_date]) — mixing incomparable samples is
//     rejected; no observation id is counted twice, directly or through an
//     un-superseded correction pair;
//   - every one of the eight metrics is represented exactly once in
//     per_metric — an unknown, duplicate or missing metric_id is rejected;
//   - each per_metric aggregate is RECOMPUTED from the resolved, included
//     observations and compared EXACTLY against the report's own stated
//     aggregate — a report that diverges from what its observations actually
//     show is rejected, not merely "plausible";
//   - a classification aggregate's rate always carries both numerator and
//     denominator; a zero denominator forbids a percentage and forces status
//     "unknown", never 0% or 100%;
//   - an excluded observation that would have contributed to a metric forces
//     that metric's per_metric entry to carry a non-empty limitation — a
//     hidden exclusion is rejected;
//   - a zero post-acceptance-defect count over an incomplete ("partial")
//     observation window forces a non-empty limitation stating that zero
//     defects is not proof of no defects;
//   - no overall score, grade, readiness or pass/fail field is accepted on
//     either record — FORBIDDEN_PAYLOAD_FIELDS names them so a leak produces
//     a pointed message;
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
  makeRecordResolver,
} from './context-manifest.mjs';

// Reused unchanged so this package carries no divergent second copy: the
// Meridian-namespace address math, the rooted-path detection, the closed
// exact-revision rule and the resolver factory.
export {
  classifyRevision, isFloatingRevision, nonPortableReason, resolveSchemaRef, makeRecordResolver,
};

export const OBSERVATION_RECORD_TYPE = 'field-evaluation-observation';
export const REPORT_RECORD_TYPE = 'field-evaluation-report';
export const RECORD_TYPES = [OBSERVATION_RECORD_TYPE, REPORT_RECORD_TYPE];

// The canonical LOGICAL resolution base within the Meridian namespace. Both
// record types declare the SAME schema file; resolution is delegated to
// resolveSchemaRef (reused, see above).
export const CANONICAL_RECORD_BASE = 'records/field-evaluation';
export const SCHEMA_NAMESPACE_DIR = 'registries/operating-model';
export const EXPECTED_SCHEMA_BASENAME = 'field-evaluation.schema.json';
export const ENVELOPE_SCHEMA_BASENAME = 'scoped-record.schema.json';
export const EXPECTED_SCHEMA_REF = `${SCHEMA_NAMESPACE_DIR}/${EXPECTED_SCHEMA_BASENAME}`;
export const ENVELOPE_SCHEMA_REF = `${SCHEMA_NAMESPACE_DIR}/${ENVELOPE_SCHEMA_BASENAME}`;

// The one area each record type may occupy.
export const OBSERVATION_SCOPE_TYPE = 'run-state';
export const REPORT_SCOPE_TYPE = 'project-workspace';
const SCOPE_REJECTION_REASON = {
  'built-in-methodology': 'built-in-methodology is Kernel methodology, not a place for practical-evaluation data',
  'user-profile': 'user-profile holds a user\'s rules and settings, not practical-evaluation data',
  'organization-profile': 'organization-profile holds an organisation\'s rules and settings, not practical-evaluation data',
  'repository-scope': 'repository-scope holds facts true for one repository; evaluation data is scoped elsewhere',
};

// The eight characteristics of plan §10, modelled SEPARATELY. Each has a
// CLOSED measurement kind; a classification metric's outcome pool is closed
// per metric, and CLASSIFICATION_TRACKED_OUTCOME names the single outcome the
// report's primary rate tracks for that metric (the complement is always
// still visible in the full `counts` breakdown, so nothing is hidden).
export const METRIC_IDS = [
  'mechanism-correctness',
  'context-entry-time',
  'rework-returns',
  'stop-correctness',
  'missed-norms',
  'resumption-success',
  'owner-cost',
  'post-acceptance-defects',
];

export const MEASUREMENT_KINDS = ['classification', 'duration', 'count'];

export const METRIC_MEASUREMENT_KIND = {
  'mechanism-correctness': 'classification',
  'context-entry-time': 'duration',
  'rework-returns': 'count',
  'stop-correctness': 'classification',
  'missed-norms': 'classification',
  'resumption-success': 'classification',
  'owner-cost': 'duration',
  'post-acceptance-defects': 'count',
};

// Only post-acceptance-defects names an explicit observation window and a
// coverage completeness flag — the other count metric (rework-returns) is a
// per-run tally and carries neither.
export const METRICS_REQUIRING_WINDOW = ['post-acceptance-defects'];

export const CLASSIFICATION_OUTCOMES = {
  'mechanism-correctness': ['correct', 'incorrect', 'unknown'],
  'stop-correctness': ['correct_stop', 'false_stop', 'unknown'],
  'missed-norms': ['missed', 'not_missed', 'unknown'],
  'resumption-success': ['successful', 'unsuccessful', 'unknown'],
};

// The single outcome each classification metric's primary rate tracks.
// mechanism-correctness and resumption-success track their named SUCCESS;
// stop-correctness and missed-norms track their named RISK. Documented here,
// and restated in the aggregate's own `note`, so the direction is never
// implicit.
export const CLASSIFICATION_TRACKED_OUTCOME = {
  'mechanism-correctness': 'correct',
  'stop-correctness': 'false_stop',
  'missed-norms': 'missed',
  'resumption-success': 'successful',
};

export const OBSERVATION_STATUSES = ['observed', 'unknown', 'not_applicable', 'unmeasurable'];
export const DURATION_KINDS = ['calendar', 'active'];
export const COVERAGE_VALUES = ['complete', 'partial'];
export const EVIDENCE_KINDS = ['check-run', 'observation', 'artifact-inspection', 'external-confirmation'];
export const OBSERVED_RESULTS = ['confirmed', 'contradicted', 'inconclusive'];
export const METRIC_REPORT_STATUSES = ['known', 'unknown', 'not_applicable'];

// Which record_type each pinned-reference slot must carry.
export const PINNED_REF_SLOTS = {
  execution_run_ref: { recordType: 'execution-run' },
  supersedes: { recordType: OBSERVATION_RECORD_TYPE },
  included_observation: { recordType: OBSERVATION_RECORD_TYPE },
  excluded_observation: { recordType: OBSERVATION_RECORD_TYPE },
};

// The CLOSED key set of a resolved entry — the transformer's confirmed view
// of ONE edition of ONE record.
export const RESOLVED_ENTRY_COMMON_KEYS = [
  'record_type', 'id', 'reference', 'revision', 'content_digest', 'source_bytes',
];
export const RESOLVED_ENTRY_KEYS_BY_TYPE = {
  'execution-run': [...RESOLVED_ENTRY_COMMON_KEYS],
  [OBSERVATION_RECORD_TYPE]: [...RESOLVED_ENTRY_COMMON_KEYS,
    'metric_id', 'status', 'measurement', 'workspace_id', 'observed_at', 'supersedes_id',
    'observation_period', 'coverage'],
  'evidence-result': [...RESOLVED_ENTRY_COMMON_KEYS, 'observed_result', 'metric_ref'],
};

// Fields that would turn a bounded evaluation record into an unbounded
// archive, duplicate a later package's subject matter, or smuggle in a single
// rolled-up score or an unwarranted release verdict this contract forbids by
// design (plan §10: "единый итоговый балл не вводится").
export const FORBIDDEN_PAYLOAD_FIELDS = [
  'file_contents', 'full_text', 'full_texts', 'raw_context', 'context_dump',
  'command_log', 'commands', 'transcript', 'messages', 'chat_history',
  'conversation', 'directory_dump', 'dir_listing', 'tree_dump', 'attachments',
  'score', 'overall_score', 'total_score', 'composite_score', 'grade',
  'readiness', 'release_ready', 'release_readiness', 'pass_fail', 'verdict',
  'go_no_go', 'rating', 'rank',
  'task_specification', 'execution_run', 'human_control', 'context_manifest',
  'claimed_results', 'verifiable_assertions', 'mandatory_checks',
  'acceptance_criteria', 'outcome', 'next_step', 'worktree_disposition',
];

const SEMANTIC_ID_RE = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;
const SHA256_RE = /^[0-9a-fA-F]{64}$/;
const DATE_RE = /^(\d{4})-(\d{2})-(\d{2})$/;
const DATETIME_RE = /^(\d{4})-(\d{2})-(\d{2})[Tt](\d{2}):(\d{2}):(\d{2})(?:\.\d+)?(?:[Zz]|[+-]\d{2}:\d{2})$/;

const isObject = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);
const blank = (s) => typeof s !== 'string' || s.trim() === '';
const hasCyrillic = (s) => typeof s === 'string' && /[Ѐ-ӿ]/.test(s);
const isNonNegInt = (n) => typeof n === 'number' && Number.isInteger(n) && n >= 0;
const isNonNegNumber = (n) => typeof n === 'number' && Number.isFinite(n) && n >= 0;
const isValidSha256 = (s) => typeof s === 'string' && SHA256_RE.test(s.trim());

function isLeapYear(y) {
  return (y % 4 === 0 && y % 100 !== 0) || y % 400 === 0;
}

// The real number of days a calendar month has in a given year — not merely
// a digit-count pattern and not Date.parse, which silently NORMALISES an
// impossible date ("2026-02-31" rolls over to a later, valid date) instead
// of rejecting it.
const DAYS_IN_MONTH = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
function isValidCalendarDate(y, mo, d) {
  if (!(mo >= 1 && mo <= 12)) return false;
  const max = mo === 2 && isLeapYear(y) ? 29 : DAYS_IN_MONTH[mo - 1];
  return d >= 1 && d <= max;
}

// A date string is valid ONLY when it is well-formed AND names a calendar
// date that actually exists ("2026-02-31", "2026-04-31" and a non-leap
// "2026-02-29" are all rejected, never silently rolled over).
export function isValidDate(s) {
  if (typeof s !== 'string') return false;
  const m = DATE_RE.exec(s);
  if (!m) return false;
  return isValidCalendarDate(Number(m[1]), Number(m[2]), Number(m[3]));
}

// The same calendar-existence rule for a date-TIME string (start_ts/end_ts):
// well-formed, a real calendar date, AND hour/minute/second within range.
// Only once a string passes this is Date.parse on it safe to use for
// ordering — no further normalisation can happen to an already-valid value.
export function isValidDateTime(s) {
  if (typeof s !== 'string') return false;
  const m = DATETIME_RE.exec(s);
  if (!m) return false;
  const y = Number(m[1]); const mo = Number(m[2]); const d = Number(m[3]);
  const h = Number(m[4]); const mi = Number(m[5]); const se = Number(m[6]);
  if (!isValidCalendarDate(y, mo, d)) return false;
  if (h > 23 || mi > 59 || se > 59) return false;
  return !Number.isNaN(Date.parse(s));
}

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

// Round to two decimal places the same way everywhere, so a recomputed
// percentage can be compared by strict equality, never by float tolerance.
export function roundPercentage(numerator, denominator) {
  if (!(denominator > 0)) return null;
  return Math.round((numerator / denominator) * 10000) / 100;
}

// Validate one pinned_ref slot (no run_id semantics in this package — unlike
// the handoff/manifest contracts, a field-evaluation observation or report
// never needs a SECOND reference confirmed to belong to "the same run").
function checkPinnedRef(problems, label, id, field, ref) {
  const slot = PINNED_REF_SLOTS[field];
  if (!isObject(ref) || Array.isArray(ref)) {
    problems.push(`${label} "${id}" ${field} is not a structured pinned reference; it is a closed { record_type, id, reference, revision?/sha256? } object, never a plain string and never an embedded body`);
    return;
  }
  if (ref.record_type !== slot.recordType) {
    problems.push(`${label} "${id}" ${field} names record_type "${ref.record_type}", not "${slot.recordType}"`);
  }
  if (typeof ref.id !== 'string' || !SEMANTIC_ID_RE.test(ref.id)) {
    problems.push(`${label} "${id}" ${field} has no stable semantic id`);
  }
  if (blank(ref.reference)) {
    problems.push(`${label} "${id}" ${field} has no portable reference`);
  } else {
    const r = nonPortableReason(ref.reference);
    if (r) problems.push(`${label} "${id}" ${field} reference contains ${r}; the reference is portable and is not an absolute machine path`);
  }
  const defect = pinDefect(ref.revision, ref.sha256);
  if (defect) {
    problems.push(`${label} "${id}" ${field} is not pinned to an exact edition: ${defect}`);
  }
  if (typeof ref.revision === 'string' && ref.revision !== '') {
    const rr = nonPortableReason(ref.revision);
    if (rr) problems.push(`${label} "${id}" ${field} revision contains ${rr}`);
  }
}

// ---------------------------------------------------------------------------
// The external record-resolution boundary — the SAME closed-contract shape as
// the other operating-model packages. A pinned reference is resolved through
// a resolver the caller supplies; the resolved entry is closed to EXACTLY the
// declared key set for its type, every value type- and value-checked.
// ---------------------------------------------------------------------------

function confirmedDigest(entry) {
  const cd = typeof entry.content_digest === 'string' && SHA256_RE.test(entry.content_digest)
    ? entry.content_digest : null;
  if (typeof entry.source_bytes === 'string') {
    const h = createHash('sha256').update(entry.source_bytes, 'utf8').digest('hex');
    if (cd && cd.toLowerCase() !== h.toLowerCase()) return { digest: h, inconsistent: cd };
    return { digest: h };
  }
  if (cd) return { digest: cd };
  return { digest: null };
}

// Resolve ONE pinned reference through the boundary and check it against the
// CLOSED transformer-response contract. `opts.wanted` overrides the
// slot-derived record type; `opts.completeness` checks the slot-specific
// tail. Returns the resolved entry, or null when it could not be resolved (a
// problem is pushed).
function resolvePinnedReference(problems, label, id, field, ref, resolve, opts = {}) {
  if (!isObject(ref) || Array.isArray(ref)) return null;
  const slot = PINNED_REF_SLOTS[field];
  const wanted = opts.wanted || (slot ? slot.recordType : null);
  const entry = resolve(ref);
  if (!isObject(entry)) {
    problems.push(`${label} "${id}" ${field} does not resolve to an actual ${wanted || 'record'} through the external resolver; a pinned reference that resolves to nothing is not a verified pin and the record fails closed`);
    return null;
  }

  const allowedKeys = RESOLVED_ENTRY_KEYS_BY_TYPE[wanted] || RESOLVED_ENTRY_COMMON_KEYS;
  for (const k of Object.keys(entry)) {
    if (!allowedKeys.includes(k)) {
      problems.push(`${label} "${id}" ${field}: the resolver returned a record with an unknown field "${k}"; the transformer response is closed to { ${allowedKeys.join(', ')} }`);
    }
  }

  if (typeof entry.record_type !== 'string' || entry.record_type === '') {
    problems.push(`${label} "${id}" ${field}: the resolver returned a record with no record_type; the transformer response is a closed contract`);
  } else if (wanted && entry.record_type !== wanted) {
    problems.push(`${label} "${id}" ${field} resolves to a "${entry.record_type}" record, not "${wanted}"`);
  }

  if (typeof entry.id !== 'string' || entry.id.trim() === '') {
    problems.push(`${label} "${id}" ${field}: the resolver returned a ${wanted || 'record'} with no id; a resolved record without an identity cannot be checked against the pin`);
  } else if (!SEMANTIC_ID_RE.test(entry.id)) {
    problems.push(`${label} "${id}" ${field}: the resolver returned a ${wanted || 'record'} whose id "${entry.id}" is not a stable semantic identifier`);
  } else if (typeof ref.id === 'string' && entry.id !== ref.id) {
    problems.push(`${label} "${id}" ${field} pins id "${ref.id}" but the reference resolves to record id "${entry.id}"; a pinned reference that resolves to a DIFFERENT record is not the same record that was pinned`);
  }

  if ('reference' in entry) {
    if (typeof entry.reference !== 'string' || entry.reference.trim() === '') {
      problems.push(`${label} "${id}" ${field}: the resolver's stated reference is present but not a non-empty string`);
    } else {
      const r = nonPortableReason(entry.reference);
      if (r) {
        problems.push(`${label} "${id}" ${field}: the resolver's stated reference contains ${r}`);
      } else if (typeof ref.reference === 'string' && entry.reference !== ref.reference) {
        problems.push(`${label} "${id}" ${field}: the resolver's stated reference "${entry.reference}" is not the resolved reference "${ref.reference}"`);
      }
    }
  }

  if ('content_digest' in entry
    && (typeof entry.content_digest !== 'string' || !SHA256_RE.test(entry.content_digest))) {
    problems.push(`${label} "${id}" ${field}: the resolver's content_digest ${JSON.stringify(entry.content_digest)} is not exactly 64 hexadecimal characters`);
  }
  if ('source_bytes' in entry && typeof entry.source_bytes !== 'string') {
    problems.push(`${label} "${id}" ${field}: the resolver's source_bytes is ${entry.source_bytes === null ? 'null' : typeof entry.source_bytes}, not a string`);
  }
  const hasRevisionString = typeof entry.revision === 'string' && entry.revision.trim() !== '';
  let revisionExact = false;
  if ('revision' in entry) {
    if (typeof entry.revision !== 'string') {
      const shown = entry.revision === null ? 'null'
        : Array.isArray(entry.revision) ? 'an array'
        : ({ number: 'a number', boolean: 'a boolean', object: 'an object' }[typeof entry.revision] || `a ${typeof entry.revision}`);
      problems.push(`${label} "${id}" ${field}: the resolver's revision is ${shown}, not a string`);
    } else if (entry.revision.trim() === '') {
      problems.push(`${label} "${id}" ${field}: the resolver's revision is present but empty`);
    } else if (classifyRevision(entry.revision) !== 'exact') {
      problems.push(`${label} "${id}" ${field}: the resolver confirmed revision ${JSON.stringify(entry.revision)}, which is not an exact edition (a full Git SHA, a strict v?X.Y.Z tag or a SHA-256 digest); the transformer confirms a fixed edition, not a moving reference`);
    } else {
      revisionExact = true;
    }
  }
  const { digest, inconsistent } = confirmedDigest(entry);
  if (!revisionExact && !digest) {
    problems.push(`${label} "${id}" ${field}: the resolver confirmed neither an exact revision nor a content digest for this edition; the pinned edition is unverified`);
  }
  if (inconsistent) {
    problems.push(`${label} "${id}" ${field}: the resolver's content_digest "${inconsistent}" does not match the SHA-256 of the resolved source bytes "${digest}"`);
  }
  if (typeof ref.revision === 'string' && ref.revision.trim() !== '') {
    if (!hasRevisionString) {
      problems.push(`${label} "${id}" ${field} pins revision "${ref.revision}" but the resolver confirmed no exact edition for this reference`);
    } else if (entry.revision !== ref.revision) {
      problems.push(`${label} "${id}" ${field} pins revision "${ref.revision}" but the resolver confirmed edition "${entry.revision}"`);
    }
  }
  if (typeof ref.sha256 === 'string' && ref.sha256.trim() !== '') {
    if (!digest) {
      problems.push(`${label} "${id}" ${field} pins sha256 "${ref.sha256}" but the resolver confirmed no content digest for this edition; a digest is verified against resolved source, never against a second copy of itself`);
    } else if (digest.toLowerCase() !== ref.sha256.toLowerCase()) {
      problems.push(`${label} "${id}" ${field} pins sha256 "${ref.sha256}" but the SHA-256 of the resolved source is "${digest}"`);
    }
  }

  if (typeof opts.completeness === 'function') opts.completeness(problems, entry);

  return entry;
}

// Resolve ONE evidence entry as an 'evidence-result' transformer response:
// pinned by the closed rule, resolved to a closed evidence-result carrying
// observed_result AND metric_ref — the SUBJECT it confirms. Returns
// { resolved, clean, observedResult, entry }.
function resolveEvidenceEntry(problems, label, id, at, metricId, eo, resolve) {
  const synthRef = { record_type: 'evidence-result', id: eo.id, reference: eo.reference };
  if (typeof eo.revision === 'string' && eo.revision !== '') synthRef.revision = eo.revision;
  if (typeof eo.sha256 === 'string' && eo.sha256 !== '') synthRef.sha256 = eo.sha256;
  const before = problems.length;
  const entry = resolvePinnedReference(problems, label, id, `evidence entry ${at}`, synthRef, resolve, {
    wanted: 'evidence-result',
    completeness: (probs, e) => {
      if (!('observed_result' in e)) {
        probs.push(`${label} "${id}" evidence entry ${at}: the resolved evidence-result confirms no observed_result; the transformer says whether the artefact confirmed, contradicted or was inconclusive about its subject`);
      } else if (!OBSERVED_RESULTS.includes(e.observed_result)) {
        probs.push(`${label} "${id}" evidence entry ${at}: the resolved evidence-result's observed_result ${JSON.stringify(e.observed_result)} is not one of { ${OBSERVED_RESULTS.join(', ')} }`);
      }
      if (blank(e.metric_ref)) {
        probs.push(`${label} "${id}" evidence entry ${at}: the resolved evidence-result confirms no metric_ref; which metric an evidence entry bears on is confirmed by the transformer, not asserted by the record`);
      } else if (e.metric_ref !== metricId) {
        probs.push(`${label} "${id}" evidence entry ${at}: the resolved evidence-result's metric_ref "${e.metric_ref}" is not this observation's own metric "${metricId}"; evidence of a different indicator does not support this one`);
      }
    },
  });
  const clean = problems.length === before;
  const observedResult = isObject(entry) && OBSERVED_RESULTS.includes(entry.observed_result) ? entry.observed_result : null;
  return { resolved: isObject(entry), clean, observedResult, entry: isObject(entry) ? entry : null };
}

// ---------------------------------------------------------------------------
// Measurement — the shape varies by kind, but the kind itself is fixed per
// metric_id (checked here, not merely by an unconstrained oneOf). A PURE
// function returning bare issue strings (no label/id prefix), so the exact
// same rule applies both to a record's own payload.measurement (checkMeasurement)
// and to the RESOLVED projection of an observation before it is ever used for
// aggregation (resolvedObservationIssues) — one rule, not two copies.
// ---------------------------------------------------------------------------
function measurementIssues(metricId, m) {
  const issues = [];
  const wantKind = METRIC_MEASUREMENT_KIND[metricId];
  if (!isObject(m)) {
    issues.push('measurement is not an object');
    return issues;
  }
  if (m.kind !== wantKind) {
    issues.push(`measurement.kind is "${m.kind}", but metric "${metricId}" is measured by kind "${wantKind}"`);
    return issues;
  }
  if (wantKind === 'classification') {
    const pool = CLASSIFICATION_OUTCOMES[metricId] || [];
    if (!pool.includes(m.outcome)) {
      issues.push(`measurement.outcome ${JSON.stringify(m.outcome)} is not one of the closed pool for "${metricId}" { ${pool.join(', ')} }`);
    }
    if (blank(m.basis)) {
      issues.push('measurement has no classification basis; a correct/false or missed/not_missed classification needs a stated basis — the status or work_status alone is never proof, and an unknown classification is never auto-treated as the tracked outcome');
    } else {
      const r = nonPortableReason(m.basis);
      if (r) issues.push(`measurement.basis contains ${r}`);
    }
  } else if (wantKind === 'duration') {
    if (!DURATION_KINDS.includes(m.duration_kind)) {
      issues.push(`measurement.duration_kind ${JSON.stringify(m.duration_kind)} is not one of { ${DURATION_KINDS.join(', ')} }; calendar length and active effort are distinguished, never summed together`);
    }
    if (m.unit !== 'seconds') {
      issues.push(`measurement.unit is ${JSON.stringify(m.unit)}, not "seconds"; a duration is always stated in seconds`);
    }
    if (!isNonNegNumber(m.seconds)) {
      issues.push(`measurement.seconds ${JSON.stringify(m.seconds)} is not a non-negative number; a negative duration is not measurable`);
    }
    if ('paused_seconds' in m) {
      if (m.duration_kind !== 'calendar') {
        issues.push(`measurement carries paused_seconds but duration_kind is "${m.duration_kind}", not "calendar"; pause accounting subtracts paused time from a CALENDAR span to reach the active-effort figure, which is its own separate observation, not a second subtraction on top of it`);
      } else if (!isNonNegNumber(m.paused_seconds)) {
        issues.push(`measurement.paused_seconds ${JSON.stringify(m.paused_seconds)} is not a non-negative number`);
      } else if (isNonNegNumber(m.seconds) && m.paused_seconds > m.seconds) {
        issues.push(`measurement.paused_seconds (${m.paused_seconds}) exceeds the calendar seconds (${m.seconds}); paused time cannot exceed the span it is paused within`);
      }
    }
    const hasStart = 'start_ts' in m;
    const hasEnd = 'end_ts' in m;
    if (hasStart !== hasEnd) {
      issues.push('measurement states only one of start_ts / end_ts; an interval names both ends or neither');
    } else if (hasStart && hasEnd) {
      if (!isValidDateTime(m.start_ts) || !isValidDateTime(m.end_ts)) {
        issues.push('measurement start_ts/end_ts is not a valid timestamp (a well-formed timestamp naming a calendar date and time that actually exist, not merely a digit pattern Date.parse would silently normalise)');
      } else {
        const s = Date.parse(m.start_ts);
        const e = Date.parse(m.end_ts);
        if (e <= s) {
          issues.push('measurement end_ts is not after start_ts; an interval that ends before (or exactly when) it starts is not a possible time interval');
        }
      }
    }
  } else if (wantKind === 'count') {
    if (m.unit !== 'count') {
      issues.push(`measurement.unit is ${JSON.stringify(m.unit)}, not "count"`);
    }
    if (!isNonNegInt(m.value)) {
      issues.push(`measurement.value ${JSON.stringify(m.value)} is not a non-negative integer`);
    }
  }
  return issues;
}

function checkMeasurement(problems, label, id, metricId, m) {
  for (const issue of measurementIssues(metricId, m)) {
    problems.push(`${label} "${id}" ${issue}`);
  }
}

// ---------------------------------------------------------------------------
// Resolved observation projection — validated BEFORE it is ever used for
// aggregation, counted, or trusted for anything. The closed key set alone
// (checked in resolvePinnedReference) says nothing about whether the VALUES
// of those subject fields are themselves valid: a resolver could return a
// record with an unknown metric_id, a negative count, an impossible date, or
// a measurement of the wrong kind, and the closed-key check alone would not
// catch it. An invalid or unresolvable record is never silently treated as
// absent from a report — it is a precise, named error.
// ---------------------------------------------------------------------------
function resolvedObservationIssues(entry) {
  const issues = [];
  if (!isObject(entry)) {
    issues.push('did not resolve to an object');
    return issues;
  }
  if (!METRIC_IDS.includes(entry.metric_id)) {
    issues.push(`metric_id ${JSON.stringify(entry.metric_id)} is not one of the eight closed characteristics`);
  }
  if (!OBSERVATION_STATUSES.includes(entry.status)) {
    issues.push(`status ${JSON.stringify(entry.status)} is not one of the closed pool { ${OBSERVATION_STATUSES.join(', ')} }`);
  }
  if (blank(entry.workspace_id) || !SEMANTIC_ID_RE.test(entry.workspace_id)) {
    issues.push(`workspace_id ${JSON.stringify(entry.workspace_id)} is missing or not a stable semantic id`);
  }
  if (blank(entry.observed_at) || !isValidDate(entry.observed_at)) {
    issues.push(`observed_at ${JSON.stringify(entry.observed_at)} is not a valid calendar date`);
  }
  if (entry.status === 'observed') {
    if (!isObject(entry.measurement)) {
      issues.push('status is "observed" but measurement is not an object');
    } else if (METRIC_IDS.includes(entry.metric_id)) {
      for (const issue of measurementIssues(entry.metric_id, entry.measurement)) {
        issues.push(`measurement ${issue}`);
      }
    }
  } else if ('measurement' in entry && entry.measurement !== null && entry.measurement !== undefined) {
    issues.push(`status is "${entry.status}" but measurement is present`);
  }
  if ('supersedes_id' in entry && entry.supersedes_id !== null) {
    if (typeof entry.supersedes_id !== 'string' || !SEMANTIC_ID_RE.test(entry.supersedes_id)) {
      issues.push(`supersedes_id ${JSON.stringify(entry.supersedes_id)} is present but neither null nor a stable semantic id`);
    }
  }
  const requiresWindow = METRIC_IDS.includes(entry.metric_id) && METRICS_REQUIRING_WINDOW.includes(entry.metric_id);
  const hasWindow = 'observation_period' in entry && entry.observation_period != null;
  if (entry.status === 'observed' && requiresWindow) {
    if (!hasWindow) {
      issues.push(`metric "${entry.metric_id}" requires an observation_period when observed`);
    } else {
      const w = isObject(entry.observation_period) ? entry.observation_period : {};
      if (!isValidDate(w.start_date) || !isValidDate(w.end_date)) {
        issues.push('observation_period does not carry two valid dates');
      } else if (Date.parse(w.end_date) < Date.parse(w.start_date)) {
        issues.push('observation_period end_date is before start_date');
      }
      if (!COVERAGE_VALUES.includes(entry.coverage)) {
        issues.push(`coverage ${JSON.stringify(entry.coverage)} is not one of the closed pool { ${COVERAGE_VALUES.join(', ')} }`);
      }
    }
  } else if (hasWindow || (('coverage' in entry) && entry.coverage != null)) {
    issues.push(`carries observation_period/coverage but metric "${entry.metric_id}" at status "${entry.status}" does not use an observation window`);
  }
  return issues;
}

// ---------------------------------------------------------------------------
// evaluateFieldEvaluation — the whole composition pipeline, branching on
// record_type.
// ---------------------------------------------------------------------------
export function evaluateFieldEvaluation(doc, opts = {}) {
  const { recordSchema, envelopeSchema, resolveRecords } = opts;
  const problems = [];

  const envErrs = [];
  try { validate(doc, envelopeSchema, envelopeSchema, '', envErrs); }
  catch (e) { return [`record envelope schema could not be applied: ${e.message}`]; }
  problems.push(...envErrs.map((m) => `envelope ${m}`));

  const bodyErrs = [];
  try { validate(doc, recordSchema, recordSchema, '', bodyErrs); }
  catch (e) { return [`field-evaluation schema could not be applied: ${e.message}`]; }
  problems.push(...bodyErrs);

  if (!isObject(doc)) {
    return problems.length ? problems : ['the field-evaluation record is not an object'];
  }

  const id = typeof doc.id === 'string' && doc.id !== '' ? doc.id : '(no id)';
  const recordType = doc.record_type;
  const label = recordType === REPORT_RECORD_TYPE ? 'field evaluation report' : 'field evaluation observation';

  if (!RECORD_TYPES.includes(recordType)) {
    problems.push(`field evaluation record "${id}" declares record_type "${recordType}", not one of { ${RECORD_TYPES.join(', ')} }`);
  }

  const declared = doc.$schema;
  if (typeof declared !== 'string' || declared === '') {
    problems.push(`${label} "${id}" declares no $schema; a record names ${EXPECTED_SCHEMA_BASENAME} so a consumer validates the whole contract — id, scope, origin, authority and body — not only the envelope`);
  } else {
    const portability = nonPortableReason(declared);
    if (portability) {
      problems.push(`${label} "${id}" $schema "${declared}" is not portable (${portability}); the schema is named by a relative reference resolved inside the Meridian namespace (canonical logical base ${CANONICAL_RECORD_BASE})`);
    } else {
      const resolved = resolveSchemaRef(declared);
      if (resolved === ENVELOPE_SCHEMA_REF) {
        problems.push(`${label} "${id}" $schema "${declared}" resolves to the record envelope (${ENVELOPE_SCHEMA_BASENAME}); it must name ${EXPECTED_SCHEMA_BASENAME}, which composes the envelope with the body`);
      } else if (resolved !== EXPECTED_SCHEMA_REF) {
        problems.push(`${label} "${id}" $schema "${declared}" does not resolve to the logical address ${EXPECTED_SCHEMA_REF} within the Meridian namespace; the specialised schema is named by a portable relative reference (a bare basename, a missing namespace segment, a wrong segment and a reference climbing out of the namespace all resolve elsewhere)`);
      }
    }
  }

  if (typeof doc.id !== 'string' || !SEMANTIC_ID_RE.test(doc.id)) {
    problems.push(`${label} "${id}" has no stable semantic id on the record envelope`);
  }
  if (typeof doc.title !== 'string' || doc.title.trim() === '') {
    problems.push(`${label} "${id}" has no human-readable title`);
  } else if (!hasCyrillic(doc.title)) {
    problems.push(`${label} "${id}" title "${doc.title}" carries no Russian (Cyrillic) text; the record name is stated in Russian for the human reader`);
  }
  const titleReason = nonPortableReason(doc.title);
  if (titleReason) problems.push(`${label} "${id}" title contains ${titleReason}`);

  const scope = isObject(doc.scope) ? doc.scope : {};
  const wantScopeType = recordType === REPORT_RECORD_TYPE ? REPORT_SCOPE_TYPE : OBSERVATION_SCOPE_TYPE;
  if (typeof scope.type === 'string' && scope.type !== wantScopeType) {
    const why = SCOPE_REJECTION_REASON[scope.type] || `a ${label} lives in ${wantScopeType} only`;
    problems.push(`${label} "${id}" is scoped to "${scope.type}"; ${why}`);
  }

  const origin = isObject(doc.origin) ? doc.origin : {};
  if (origin.kind === 'built-in') {
    problems.push(`${label} "${id}" declares origin.kind "built-in"; evaluation data is written in a workspace, not shipped with the methodology`);
  }
  const authority = isObject(doc.authority) ? doc.authority : {};
  for (const [obj, field, flabel] of [
    [origin, 'source_ref', 'origin.source_ref'],
    [authority, 'authority_ref', 'authority.authority_ref'],
    [authority, 'decision_ref', 'authority.decision_ref'],
  ]) {
    const val = obj[field];
    if (typeof val === 'string' && val !== '') {
      const r = nonPortableReason(val);
      if (r) problems.push(`${label} "${id}" ${flabel} contains ${r}; a record is portable and carries no rooted machine path`);
    }
  }

  const payload = isObject(doc.payload) ? doc.payload : {};
  for (const f of FORBIDDEN_PAYLOAD_FIELDS) {
    if (f in payload) {
      problems.push(`${label} "${id}" payload carries "${f}"; practical evaluation measures separately — it does not roll up into one score, grade or release verdict, and does not absorb another contract's body`);
    }
  }
  if ('record_type' in payload) {
    problems.push(`${label} "${id}" repeats record_type inside the payload; the record type is declared once, on the envelope`);
  }

  const resolve = typeof resolveRecords === 'function' ? resolveRecords : null;

  if (recordType === OBSERVATION_RECORD_TYPE) {
    evaluateObservationBody(problems, label, id, scope, payload, resolve);
  } else if (recordType === REPORT_RECORD_TYPE) {
    evaluateReportBody(problems, label, id, scope, payload, resolve);
  }

  return problems;
}

// ---------------------------------------------------------------------------
// Observation body
// ---------------------------------------------------------------------------
function evaluateObservationBody(problems, label, id, scope, payload, resolve) {
  if (scope.type === OBSERVATION_SCOPE_TYPE) {
    if (typeof scope.id !== 'string' || !SEMANTIC_ID_RE.test(scope.id)) {
      problems.push(`${label} "${id}" run-state scope carries no stable scope.id identifying the run`);
    }
    if (typeof scope.workspace_id !== 'string' || !SEMANTIC_ID_RE.test(scope.workspace_id)) {
      problems.push(`${label} "${id}" run-state scope carries no workspace_id; a run belongs to a project workspace (workspace-scope-model.md §1)`);
    }
  }

  if (!('execution_run_ref' in payload)) {
    problems.push(`${label} "${id}" names no execution_run_ref; an observation carries a structured pinned reference to exactly one run`);
  } else {
    checkPinnedRef(problems, label, id, 'execution_run_ref', payload.execution_run_ref);
    if (resolve) {
      resolvePinnedReference(problems, label, id, 'execution_run_ref', payload.execution_run_ref, resolve);
    } else {
      problems.push(`${label} "${id}" cannot be verified: no external record resolver was supplied; the pinned run is resolved OUTSIDE the record and checked against it`);
    }
  }
  const runRef = payload.execution_run_ref;
  if (scope.type === OBSERVATION_SCOPE_TYPE && typeof scope.id === 'string' && scope.id !== ''
    && isObject(runRef) && typeof runRef.id === 'string' && runRef.id !== '' && scope.id !== runRef.id) {
    problems.push(`${label} "${id}" scope identifies run "${scope.id}" but execution_run_ref.id is "${runRef.id}"; the two must be the exact same string, not one a substring of the other`);
  }

  const metricId = payload.metric_id;
  if (!METRIC_IDS.includes(metricId)) {
    problems.push(`${label} "${id}" metric_id ${JSON.stringify(metricId)} is not one of the eight closed characteristics { ${METRIC_IDS.join(', ')} }`);
  }

  if (blank(payload.observed_at) || !isValidDate(payload.observed_at)) {
    problems.push(`${label} "${id}" observed_at is not a valid date (YYYY-MM-DD); the record states when it was itself recorded`);
  }

  const status = payload.status;
  if (!OBSERVATION_STATUSES.includes(status)) {
    problems.push(`${label} "${id}" status ${JSON.stringify(status)} is not one of the closed pool { ${OBSERVATION_STATUSES.join(', ')} }; absence of observation is distinguished from a measured zero`);
  }

  if (status === 'observed') {
    if ('status_reason' in payload) {
      problems.push(`${label} "${id}" is "observed" but carries a status_reason; a reason is stated for unknown, not_applicable or unmeasurable, where the absence of a value needs explaining`);
    }
    if (!('measurement' in payload)) {
      problems.push(`${label} "${id}" is "observed" but carries no measurement`);
    } else if (METRIC_IDS.includes(metricId)) {
      checkMeasurement(problems, label, id, metricId, payload.measurement);
    }
  } else if (OBSERVATION_STATUSES.includes(status)) {
    if (blank(payload.status_reason)) {
      problems.push(`${label} "${id}" is "${status}" but states no status_reason; unknown, not_applicable and unmeasurable are distinct and each needs its own explanation, not a shared silence`);
    } else {
      const r = nonPortableReason(payload.status_reason);
      if (r) problems.push(`${label} "${id}" status_reason contains ${r}`);
    }
    if ('measurement' in payload) {
      problems.push(`${label} "${id}" is "${status}" but carries a measurement; a value that was not observed is not measured`);
    }
  }

  const requiresWindow = METRICS_REQUIRING_WINDOW.includes(metricId);
  const hasWindow = 'observation_period' in payload;
  if (status === 'observed' && requiresWindow) {
    if (!hasWindow) {
      problems.push(`${label} "${id}" measures "${metricId}", which requires an observation_period (the window actually covered) and a coverage completeness flag`);
    } else {
      const w = isObject(payload.observation_period) ? payload.observation_period : {};
      if (!isValidDate(w.start_date) || !isValidDate(w.end_date)) {
        problems.push(`${label} "${id}" observation_period does not carry two valid dates`);
      } else if (Date.parse(w.end_date) < Date.parse(w.start_date)) {
        problems.push(`${label} "${id}" observation_period end_date is before start_date; an impossible window`);
      }
      if (!COVERAGE_VALUES.includes(payload.coverage)) {
        problems.push(`${label} "${id}" measures "${metricId}" but coverage ${JSON.stringify(payload.coverage)} is not one of { ${COVERAGE_VALUES.join(', ')} }`);
      } else if (payload.coverage === 'partial' && blank(payload.coverage_note)) {
        problems.push(`${label} "${id}" coverage is "partial" but states no coverage_note; an incomplete window needs to say what is not covered`);
      } else if (payload.coverage === 'complete' && 'coverage_note' in payload) {
        problems.push(`${label} "${id}" coverage is "complete" but carries a coverage_note; a note belongs to a partial window`);
      }
    }
  } else if (hasWindow || 'coverage' in payload || 'coverage_note' in payload) {
    problems.push(`${label} "${id}" carries observation_period/coverage, but metric "${metricId}" at status "${status}" does not use an observation window; only "${METRICS_REQUIRING_WINDOW.join(', ')}" while "observed" does`);
  }

  // Evidence: required non-empty only when observed; forbidden otherwise.
  const evidence = Array.isArray(payload.evidence) ? payload.evidence : [];
  if (status === 'observed' && evidence.length === 0) {
    problems.push(`${label} "${id}" is "observed" but carries no evidence; a measured value is grounded by at least one resolvable piece of evidence, not asserted on its own say-so`);
  }
  if (status !== 'observed' && 'evidence' in payload && evidence.length > 0) {
    problems.push(`${label} "${id}" is "${status}" but carries evidence; there is nothing to evidence when nothing was observed`);
  }
  const evidenceIds = new Set();
  let anyConfirmed = false;
  evidence.forEach((e, i) => {
    const eo = isObject(e) ? e : {};
    const eid = typeof eo.id === 'string' ? eo.id : null;
    const at = eid || `#${i}`;
    if (!eid || !SEMANTIC_ID_RE.test(eid)) {
      problems.push(`${label} "${id}" evidence entry ${at} has no stable semantic id`);
    } else {
      if (evidenceIds.has(eid)) problems.push(`${label} "${id}" evidence id "${eid}" is used more than once`);
      evidenceIds.add(eid);
    }
    if (!EVIDENCE_KINDS.includes(eo.kind)) {
      problems.push(`${label} "${id}" evidence entry ${at} kind ${JSON.stringify(eo.kind)} is not one of { ${EVIDENCE_KINDS.join(', ')} }`);
    }
    for (const [f, human] of [['reference', 'reference'], ['summary', 'summary']]) {
      if (blank(eo[f])) {
        problems.push(`${label} "${id}" evidence entry ${at} has no ${human}`);
      } else {
        const r = nonPortableReason(eo[f]);
        if (r) problems.push(`${label} "${id}" evidence entry ${at} ${f} contains ${r}`);
      }
    }
    if (typeof eo.revision === 'string' && eo.revision !== '') {
      const rr = nonPortableReason(eo.revision);
      if (rr) problems.push(`${label} "${id}" evidence entry ${at} revision contains ${rr}`);
    }
    const pinReason = pinDefect(eo.revision, eo.sha256);
    if (pinReason) {
      problems.push(`${label} "${id}" evidence entry ${at} is not pinned to an exact edition: ${pinReason}; an unpinned reference is not verifiable evidence`);
    }
    if (resolve && status === 'observed' && METRIC_IDS.includes(metricId)) {
      const res = resolveEvidenceEntry(problems, label, id, at, metricId, eo, resolve);
      if (res.clean && res.observedResult === 'confirmed') anyConfirmed = true;
    }
  });
  if (status === 'observed' && resolve && evidence.length > 0 && !anyConfirmed) {
    problems.push(`${label} "${id}" is "observed" but no evidence entry resolves cleanly with observed_result "confirmed" for this metric; an unresolved or contradicted/inconclusive entry does not ground a measured value`);
  } else if (status === 'observed' && !resolve) {
    problems.push(`${label} "${id}" cannot be verified: no external record resolver was supplied; evidence is resolved OUTSIDE the record and checked against it — without that boundary the record is only an unanchored self-report`);
  }

  // Correction: supersedes <-> correction_reason, both or neither.
  const hasSupersedes = 'supersedes' in payload;
  const hasReason = 'correction_reason' in payload;
  if (hasSupersedes !== hasReason) {
    problems.push(`${label} "${id}" states only one of supersedes / correction_reason; a correction names BOTH the observation it corrects and why`);
  } else if (hasSupersedes) {
    checkPinnedRef(problems, label, id, 'supersedes', payload.supersedes);
    if (blank(payload.correction_reason)) {
      problems.push(`${label} "${id}" correction_reason is blank`);
    } else {
      const r = nonPortableReason(payload.correction_reason);
      if (r) problems.push(`${label} "${id}" correction_reason contains ${r}`);
    }
    if (isObject(payload.supersedes) && payload.supersedes.id === id) {
      problems.push(`${label} "${id}" supersedes itself; a correction names a DIFFERENT, earlier observation`);
    }
    if (resolve) {
      resolvePinnedReference(problems, label, id, 'supersedes', payload.supersedes, resolve);
    }
  }
}

// ---------------------------------------------------------------------------
// Report body
// ---------------------------------------------------------------------------
function resolvedIsUsable(entry) {
  return isObject(entry) && entry.status === 'observed' && isObject(entry.measurement);
}

// Recompute the aggregate for ONE metric from its USABLE (status "observed",
// resolved) contributing observations. Deterministic: sorted by observation
// id before any accumulation, so permutation of the input never changes the
// result.
export function computeMetricAggregate(metricId, usableEntries) {
  const kind = METRIC_MEASUREMENT_KIND[metricId];
  const sorted = [...usableEntries].sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
  if (kind === 'classification') {
    const pool = CLASSIFICATION_OUTCOMES[metricId];
    const counts = {};
    for (const o of pool) counts[o] = 0;
    for (const e of sorted) {
      const outcome = e.measurement.outcome;
      if (pool.includes(outcome)) counts[outcome] += 1;
    }
    const total = pool.reduce((s, o) => s + counts[o], 0);
    const tracked = CLASSIFICATION_TRACKED_OUTCOME[metricId];
    const numerator = counts[tracked] || 0;
    const percentage = total > 0 ? roundPercentage(numerator, total) : null;
    const rate = {
      numerator, denominator: total, outcome: tracked,
      note: `доля наблюдений с исходом "${tracked}" среди ${total} классифицированных; ноль в знаменателе не даёт процента`,
    };
    if (percentage !== null) rate.percentage = percentage;
    return { kind: 'classification', counts, total, rate };
  }
  if (kind === 'duration') {
    const byKind = { calendar: [], active: [] };
    for (const e of sorted) {
      const dk = e.measurement.duration_kind;
      if (byKind[dk]) byKind[dk].push(e.measurement.seconds);
    }
    const summarise = (arr) => {
      const sample_count = arr.length;
      const total_seconds = arr.reduce((s, v) => s + v, 0);
      const out = { sample_count, total_seconds };
      if (sample_count > 0) out.mean_seconds = Math.round((total_seconds / sample_count) * 100) / 100;
      return out;
    };
    return { kind: 'duration', calendar: summarise(byKind.calendar), active: summarise(byKind.active) };
  }
  // count
  const values = sorted.map((e) => e.measurement.value);
  const sample_count = values.length;
  const total = values.reduce((s, v) => s + v, 0);
  const out = { kind: 'count', sample_count, total };
  if (sample_count > 0) out.mean = Math.round((total / sample_count) * 100) / 100;
  if (METRICS_REQUIRING_WINDOW.includes(metricId)) {
    const withWindow = sorted.filter((e) => isObject(e.observation_period));
    if (withWindow.length > 0) {
      const starts = withWindow.map((e) => e.observation_period.start_date).sort();
      const ends = withWindow.map((e) => e.observation_period.end_date).sort();
      out.observation_window = { start_date: starts[0], end_date: ends[ends.length - 1] };
      const coverages = new Set(withWindow.map((e) => e.coverage));
      out.coverage = coverages.size > 1 ? 'mixed' : [...coverages][0];
    }
  }
  return out;
}

// A true canonical form — object keys sorted RECURSIVELY, at every depth —
// so two values compare equal iff they are deep-equal regardless of key
// insertion order. (A flat top-level key filter is NOT sufficient here: it
// would silently drop nested fields whose own key names are not among the
// top-level keys, masking a real divergence as a false match.)
function canonicalJSON(x) {
  if (Array.isArray(x)) return `[${x.map(canonicalJSON).join(',')}]`;
  if (x !== null && typeof x === 'object') {
    const keys = Object.keys(x).sort();
    return `{${keys.map((k) => `${JSON.stringify(k)}:${canonicalJSON(x[k])}`).join(',')}}`;
  }
  return JSON.stringify(x);
}

// This is an EXACTNESS check, not a fuzzy one: both sides are produced by the
// same deterministic computation, so any divergence is a real one.
function sameAggregate(a, b) {
  return canonicalJSON(a) === canonicalJSON(b);
}

function evaluateReportBody(problems, label, id, scope, payload, resolve) {
  if (typeof payload.workspace_id !== 'string' || !SEMANTIC_ID_RE.test(payload.workspace_id)) {
    problems.push(`${label} "${id}" names no workspace_id; a report is scoped to the project workspace it covers`);
  }
  const period = isObject(payload.period) ? payload.period : {};
  let periodOk = false;
  if (!isValidDate(period.start_date) || !isValidDate(period.end_date)) {
    problems.push(`${label} "${id}" period does not carry two valid dates`);
  } else if (Date.parse(period.end_date) < Date.parse(period.start_date)) {
    problems.push(`${label} "${id}" period end_date is before start_date; an impossible reporting window`);
  } else {
    periodOk = true;
  }

  const included = Array.isArray(payload.included_observations) ? payload.included_observations : [];
  const excluded = Array.isArray(payload.excluded_observations) ? payload.excluded_observations : [];

  const seenIncludedRef = new Set();
  included.forEach((ref, i) => {
    checkPinnedRef(problems, label, id, 'included_observation', ref);
    if (isObject(ref) && typeof ref.reference === 'string') {
      if (seenIncludedRef.has(ref.reference)) {
        problems.push(`${label} "${id}" included_observations repeats reference "${ref.reference}" at #${i}; the same observation is not counted twice`);
      }
      seenIncludedRef.add(ref.reference);
    }
  });
  // excluded_observations: no repeated reference (even under a different
  // exclusion_reason), and no reference already named in included_observations
  // — included and excluded are disjoint sets, not merely each internally
  // reasoned.
  const seenExcludedRef = new Set();
  excluded.forEach((x, i) => {
    const xo = isObject(x) ? x : {};
    checkPinnedRef(problems, label, id, 'excluded_observation', xo.ref);
    if (blank(xo.exclusion_reason)) {
      problems.push(`${label} "${id}" excluded_observations[${i}] has no exclusion_reason; an exclusion is explicit and reasoned, never a silent drop`);
    } else {
      const r = nonPortableReason(xo.exclusion_reason);
      if (r) problems.push(`${label} "${id}" excluded_observations[${i}] exclusion_reason contains ${r}`);
    }
    if (isObject(xo.ref) && typeof xo.ref.reference === 'string') {
      if (seenExcludedRef.has(xo.ref.reference)) {
        problems.push(`${label} "${id}" excluded_observations repeats reference "${xo.ref.reference}" at #${i}; a repeated excluded reference is not legalised by a different exclusion_reason`);
      }
      seenExcludedRef.add(xo.ref.reference);
      if (seenIncludedRef.has(xo.ref.reference)) {
        problems.push(`${label} "${id}" reference "${xo.ref.reference}" appears in both included_observations and excluded_observations; an observation is either included or excluded, never both`);
      }
    }
  });

  // Resolve every included/excluded observation. A resolved entry is the
  // closed transformer view; resolution failures are reported per-entry, and
  // the VALUES of its subject fields (metric_id, status, measurement,
  // workspace_id, observed_at, supersedes_id, observation_period, coverage)
  // are validated here, BEFORE anything is aggregated or counted — an
  // invalid or unresolvable record is a precise error, not a silent absence.
  const includedResolved = [];
  const excludedResolved = [];
  if (!resolve) {
    problems.push(`${label} "${id}" cannot be verified: no external record resolver was supplied; every included and excluded observation is resolved OUTSIDE the report and checked against it`);
  } else {
    included.forEach((ref, i) => {
      const entry = resolvePinnedReference(problems, label, id, 'included_observation', ref, resolve, { wanted: OBSERVATION_RECORD_TYPE });
      if (isObject(entry)) {
        for (const issue of resolvedObservationIssues(entry)) {
          problems.push(`${label} "${id}" included_observations[${i}]: resolved observation ${issue}`);
        }
      }
      includedResolved.push({ ref, entry, at: i });
    });
    excluded.forEach((x, i) => {
      const xo = isObject(x) ? x : {};
      const entry = isObject(xo.ref)
        ? resolvePinnedReference(problems, label, id, 'excluded_observation', xo.ref, resolve, { wanted: OBSERVATION_RECORD_TYPE })
        : null;
      if (isObject(entry)) {
        for (const issue of resolvedObservationIssues(entry)) {
          problems.push(`${label} "${id}" excluded_observations[${i}]: resolved observation ${issue}`);
        }
      }
      excludedResolved.push({ xo, entry, at: i });
    });
  }

  // Comparability: same workspace, observed_at inside the period. Double
  // counting: no observation id repeated, and no superseded+superseding pair
  // both included.
  const byId = new Map();
  for (const { entry } of includedResolved) {
    if (isObject(entry) && typeof entry.id === 'string') {
      if (byId.has(entry.id)) {
        problems.push(`${label} "${id}" included_observations resolves two entries to the SAME observation id "${entry.id}"; a repeated record is not counted twice`);
      }
      byId.set(entry.id, entry);
    }
  }

  // The same disjointness rule, at the level of what the references actually
  // RESOLVE to: two different excluded references naming the same observation,
  // or an excluded reference resolving to an observation that is also
  // included — a reference-string check alone (above) cannot catch either.
  const excludedIdSeen = new Set();
  for (const { entry, at } of excludedResolved) {
    if (!isObject(entry) || typeof entry.id !== 'string') continue;
    if (excludedIdSeen.has(entry.id)) {
      problems.push(`${label} "${id}" excluded_observations[${at}] resolves to observation "${entry.id}", which another excluded_observations entry already resolves to; two different references naming the same observation are not both excluded`);
    }
    excludedIdSeen.add(entry.id);
    if (byId.has(entry.id)) {
      problems.push(`${label} "${id}" excluded_observations[${at}] resolves to observation "${entry.id}", which is also an included observation; an observation is either included or excluded, never both`);
    }
  }
  for (const { entry, at } of includedResolved) {
    if (!isObject(entry)) continue;
    if (typeof entry.workspace_id === 'string' && typeof payload.workspace_id === 'string'
      && entry.workspace_id !== payload.workspace_id) {
      problems.push(`${label} "${id}" included_observations[${at}] belongs to workspace "${entry.workspace_id}", not this report's workspace "${payload.workspace_id}"; samples from different workspaces are not mixed without an explicit comparability rule`);
    }
    if (periodOk && isValidDate(entry.observed_at)) {
      const t = Date.parse(entry.observed_at);
      if (t < Date.parse(period.start_date) || t > Date.parse(period.end_date)) {
        problems.push(`${label} "${id}" included_observations[${at}] was observed at "${entry.observed_at}", outside the report period [${period.start_date}, ${period.end_date}]; samples from different periods are not mixed without an explicit comparability rule`);
      }
    }
    if (typeof entry.supersedes_id === 'string' && byId.has(entry.supersedes_id)) {
      problems.push(`${label} "${id}" includes both observation "${entry.id}" and the observation "${entry.supersedes_id}" it supersedes; the superseded record's facts are replaced, not additionally counted`);
    }
  }

  // per_metric: exactly the eight metric_ids, each exactly once.
  const perMetric = Array.isArray(payload.per_metric) ? payload.per_metric : [];
  const seenMetric = new Set();
  const byMetric = new Map();
  perMetric.forEach((pm, i) => {
    const pmo = isObject(pm) ? pm : {};
    const mid = pmo.metric_id;
    if (!METRIC_IDS.includes(mid)) {
      problems.push(`${label} "${id}" per_metric[${i}] metric_id ${JSON.stringify(mid)} is not one of the eight closed characteristics`);
    } else {
      if (seenMetric.has(mid)) problems.push(`${label} "${id}" per_metric repeats metric_id "${mid}"`);
      seenMetric.add(mid);
      byMetric.set(mid, pmo);
    }
  });
  for (const mid of METRIC_IDS) {
    if (!seenMetric.has(mid)) {
      problems.push(`${label} "${id}" per_metric omits metric_id "${mid}"; all eight characteristics are represented, even when the result is "unknown"`);
    }
  }

  // Per-metric status/reason consistency, sample_observation_ids membership.
  for (const [mid, pmo] of byMetric) {
    if (!METRIC_REPORT_STATUSES.includes(pmo.status)) {
      problems.push(`${label} "${id}" per_metric "${mid}" status ${JSON.stringify(pmo.status)} is not one of { ${METRIC_REPORT_STATUSES.join(', ')} }`);
    }
    if (pmo.status !== 'known') {
      if (blank(pmo.status_reason)) {
        problems.push(`${label} "${id}" per_metric "${mid}" is "${pmo.status}" but states no status_reason`);
      }
    } else if ('status_reason' in pmo) {
      problems.push(`${label} "${id}" per_metric "${mid}" is "known" but carries a status_reason; a known value needs no absence explanation`);
    }
    const sids = Array.isArray(pmo.sample_observation_ids) ? pmo.sample_observation_ids : [];
    const seenS = new Set();
    sids.forEach((sid) => {
      if (seenS.has(sid)) problems.push(`${label} "${id}" per_metric "${mid}" sample_observation_ids repeats "${sid}"`);
      seenS.add(sid);
      if (!byId.has(sid)) {
        problems.push(`${label} "${id}" per_metric "${mid}" sample_observation_ids names "${sid}", which is not an included observation`);
      } else if (byId.get(sid).metric_id !== mid) {
        problems.push(`${label} "${id}" per_metric "${mid}" sample_observation_ids names "${sid}", which is an observation of metric "${byId.get(sid).metric_id}", not "${mid}"`);
      }
    });
    const limitations = Array.isArray(pmo.limitations) ? pmo.limitations : [];
    limitations.forEach((lm, j) => {
      if (blank(lm)) problems.push(`${label} "${id}" per_metric "${mid}" limitations[${j}] is empty or whitespace-only`);
      else {
        const r = nonPortableReason(lm);
        if (r) problems.push(`${label} "${id}" per_metric "${mid}" limitations[${j}] contains ${r}`);
      }
    });
  }

  if (!resolve) return; // nothing further can be recomputed without the boundary.

  // An observation superseded by another INCLUDED observation is not counted
  // at all (its facts are replaced) — it is excluded from the completeness
  // check below the same way the builder drops it from computation.
  const supersededIds = new Set();
  for (const { entry } of includedResolved) {
    if (isObject(entry) && typeof entry.supersedes_id === 'string' && byId.has(entry.supersedes_id)) {
      supersededIds.add(entry.supersedes_id);
    }
  }

  // Recompute every metric's aggregate from the RESOLVED, usable observations
  // actually named as that metric's samples, and compare EXACTLY against the
  // report's own stated aggregate.
  for (const mid of METRIC_IDS) {
    const pmo = byMetric.get(mid);
    if (!pmo) continue;
    const sids = new Set(Array.isArray(pmo.sample_observation_ids) ? pmo.sample_observation_ids : []);

    // Completeness: sample_observation_ids for this metric must equal EXACTLY
    // the full set of included (non-superseded) observations of this metric —
    // under-listing would silently drop an included, inconvenient observation
    // from the aggregate without a formal, reasoned exclusion; over-listing
    // would claim a sample that was never actually included.
    const actualForMetric = new Set();
    for (const [oid, entry] of byId) {
      if (entry.metric_id === mid && !supersededIds.has(oid)) actualForMetric.add(oid);
    }
    for (const oid of actualForMetric) {
      if (!sids.has(oid)) {
        problems.push(`${label} "${id}" per_metric "${mid}" sample_observation_ids omits included observation "${oid}"; an included observation is either counted in its metric's sample or formally excluded with a reason, never silently left out of the aggregate`);
      }
    }
    for (const oid of sids) {
      if (!actualForMetric.has(oid)) {
        problems.push(`${label} "${id}" per_metric "${mid}" sample_observation_ids names "${oid}", which is not an included observation of this metric (or was superseded); a claimed sample member is not one of the report's own included observations`);
      }
    }

    const usable = [];
    let anyNotApplicable = false;
    let anyOther = false;
    for (const [oid, entry] of byId) {
      if (entry.metric_id !== mid) continue;
      if (!sids.has(oid)) continue;
      if (resolvedIsUsable(entry)) usable.push(entry);
      else if (entry.status === 'not_applicable') anyNotApplicable = true;
      else anyOther = true;
    }

    const expectedAggregate = computeMetricAggregate(mid, usable);
    let expectedStatus;
    if (usable.length > 0) {
      expectedStatus = 'known';
      if (METRIC_MEASUREMENT_KIND[mid] === 'classification' && expectedAggregate.total === 0) expectedStatus = 'unknown';
    } else if (anyNotApplicable && !anyOther) {
      expectedStatus = 'not_applicable';
    } else {
      expectedStatus = 'unknown';
    }
    if (pmo.status !== expectedStatus) {
      problems.push(`${label} "${id}" per_metric "${mid}" states status "${pmo.status}", but the resolved, named sample implies "${expectedStatus}"; the report's conclusion must agree with what its own observations show`);
    }
    // The aggregate is recomputed and compared EXACTLY regardless of status:
    // an "unknown" or "not_applicable" entry is not exempt from this check —
    // its aggregate is the SAME canonical empty/inapplicable shape
    // computeMetricAggregate produces for its (empty) usable sample, never an
    // arbitrary formally-well-shaped value.
    if (!sameAggregate(pmo.aggregate, expectedAggregate)) {
      problems.push(`${label} "${id}" per_metric "${mid}" aggregate does not match the value recomputed from its resolved sample; a stated aggregate that diverges from the actual observations is rejected, whatever the status`);
    }

    // A zero post-acceptance-defect count over an incomplete window is not
    // proof of no defects: forced, non-empty limitation.
    if (mid === 'post-acceptance-defects' && expectedAggregate.total === 0
      && (expectedAggregate.coverage === 'partial' || expectedAggregate.coverage === 'mixed')) {
      const limitations = Array.isArray(pmo.limitations) ? pmo.limitations : [];
      if (limitations.length === 0) {
        problems.push(`${label} "${id}" per_metric "post-acceptance-defects" shows zero defects over a "${expectedAggregate.coverage}" observation window but carries no limitation; a zero count over incomplete coverage is not proof of no defects and must say so`);
      }
    }

    // Hidden exclusion: an excluded observation of THIS metric forces a
    // disclosed limitation. Reuses the already-resolved, already-validated
    // excludedResolved entries rather than re-resolving ad hoc.
    const excludedForMetric = excludedResolved.some(({ entry }) => isObject(entry) && entry.metric_id === mid);
    if (excludedForMetric) {
      const limitations = Array.isArray(pmo.limitations) ? pmo.limitations : [];
      if (limitations.length === 0) {
        problems.push(`${label} "${id}" per_metric "${mid}" has an excluded observation of this metric but discloses no limitation; an exclusion that would have contributed to a metric is disclosed, not hidden`);
      }
    }
  }
}

// ---------------------------------------------------------------------------
// buildFieldEvaluationReport — the deterministic builder. A pure function:
// the same included/excluded sets and the same resolver produce the EXACT
// same payload, regardless of the order EITHER includedRefs or
// excludedEntries was given in. It is the SAME computeMetricAggregate the
// validator's recomputation check uses, so the builder and the check cannot
// drift. Every reference is resolved AND validated (resolvedObservationIssues)
// up front: an unresolvable or structurally invalid observation never
// silently disappears from the built report — the whole build fails closed
// with a precise error. Duplicates and included/excluded overlaps are
// rejected explicitly, never dissolved by a Map's silent last-write-wins.
// ---------------------------------------------------------------------------
export function buildFieldEvaluationReport({ workspaceId, period, includedRefs, excludedEntries = [], resolveRecords }) {
  if (typeof resolveRecords !== 'function') {
    throw new Error('buildFieldEvaluationReport requires a resolveRecords function; observations are resolved OUTSIDE the report');
  }

  // Resolve and validate ONE reference through the SAME three-part
  // closed-contract boundary the rest of the module uses — never a lighter,
  // ad hoc check, and never a divergent second copy of any of the three
  // rules:
  //   1. checkPinnedRef on the INPUT pinned_ref ITSELF: its own record_type
  //      matches the slot (field-evaluation-observation), it carries a
  //      stable semantic id, a portable reference, and a pin (an exact
  //      revision and/or sha256) — checked independently of whatever the
  //      resolver later returns. resolvePinnedReference alone does NOT
  //      enforce this: it only cross-checks a pin field's VALUE against the
  //      resolved entry WHEN that field happens to be present on the input
  //      ref, so an input ref that simply omits id/record_type/revision/sha256
  //      would sail through resolvePinnedReference's checks with nothing to
  //      cross-check against — checkPinnedRef is what actually requires
  //      those fields to be there and correct in the first place.
  //   2. resolvePinnedReference on the RESOLVED record and its pin: the
  //      resolver's record_type, closed key set, id/reference match, and an
  //      exact revision and/or confirmed digest matching what was pinned.
  //   3. resolvedObservationIssues on the resolved record's subject VALUES
  //      (metric_id, status, measurement, …), only once (1) and (2) are clean.
  // A problem at ANY of the three stops the build with an explicit thrown
  // error before any aggregate is ever computed.
  function resolveAndValidate(ref, field) {
    const problems = [];
    checkPinnedRef(problems, 'field evaluation report', '(builder)', field, ref);
    const entry = resolvePinnedReference(problems, 'field evaluation report', '(builder)', field, ref, resolveRecords);
    if (problems.length > 0 || !isObject(entry)) {
      const refDesc = isObject(ref) && typeof ref.reference === 'string' ? `"${ref.reference}"` : JSON.stringify(ref);
      throw new Error(`buildFieldEvaluationReport: ${field} ${refDesc} failed resolution through the external boundary: ${
        problems.length ? problems.join('; ') : 'did not resolve to an actual observation'}`);
    }
    const issues = resolvedObservationIssues(entry);
    if (issues.length) {
      throw new Error(`buildFieldEvaluationReport: ${field} "${ref.reference}" resolved to an invalid observation projection: ${issues.join('; ')}`);
    }
    return entry;
  }

  const includedList = (includedRefs || []).map((ref) => ({ ref, entry: resolveAndValidate(ref, 'included_observation') }));
  const excludedList = (excludedEntries || []).map((ex) => {
    if (!isObject(ex) || !isObject(ex.ref)) {
      throw new Error('buildFieldEvaluationReport: an excluded_observations entry has no ref');
    }
    return { ex, entry: resolveAndValidate(ex.ref, 'excluded_observation') };
  });

  // Explicit duplicate/overlap rejection — never silently resolved through a
  // Map that would just keep the last write and drop the rest.
  const includedIds = new Set();
  const includedRefStrings = new Set();
  for (const { ref, entry } of includedList) {
    if (includedRefStrings.has(ref.reference)) {
      throw new Error(`buildFieldEvaluationReport: included_observations repeats reference "${ref.reference}"`);
    }
    includedRefStrings.add(ref.reference);
    if (includedIds.has(entry.id)) {
      throw new Error(`buildFieldEvaluationReport: two included references resolve to the same observation "${entry.id}"`);
    }
    includedIds.add(entry.id);
  }
  const excludedIds = new Set();
  const excludedRefStrings = new Set();
  for (const { ex, entry } of excludedList) {
    const refStr = ex.ref.reference;
    if (excludedRefStrings.has(refStr)) {
      throw new Error(`buildFieldEvaluationReport: excluded_observations repeats reference "${refStr}"`);
    }
    excludedRefStrings.add(refStr);
    if (includedRefStrings.has(refStr)) {
      throw new Error(`buildFieldEvaluationReport: reference "${refStr}" appears in both included_observations and excluded_observations`);
    }
    if (excludedIds.has(entry.id)) {
      throw new Error(`buildFieldEvaluationReport: two excluded references resolve to the same observation "${entry.id}"`);
    }
    excludedIds.add(entry.id);
    if (includedIds.has(entry.id)) {
      throw new Error(`buildFieldEvaluationReport: observation "${entry.id}" is both included and excluded`);
    }
  }

  // Canonical, order-independent key: the resolved observation's own stable
  // id, with the pinned reference string as an UNAMBIGUOUS tie-break (ids are
  // already unique per the checks above, so the tie-break is never actually
  // exercised on real data — it exists so the sort is total and deterministic
  // by construction, not merely "usually stable"). Neither list's build order
  // nor Array#sort's own stability is relied upon.
  const sortKey = (rid, ref) => `${rid}::${ref}`;
  const byCanonicalKey = (ka, kb) => (ka < kb ? -1 : ka > kb ? 1 : 0);
  const sortedIncluded = [...includedList].sort((a, b) => byCanonicalKey(
    sortKey(a.entry.id, a.ref.reference), sortKey(b.entry.id, b.ref.reference),
  ));
  const sortedExcluded = [...excludedList].sort((a, b) => byCanonicalKey(
    sortKey(a.entry.id, a.ex.ref.reference), sortKey(b.entry.id, b.ex.ref.reference),
  ));

  const resolvedById = new Map(sortedIncluded.map((x) => [x.entry.id, x.entry]));
  // A correction (supersedes) and the observation it corrects are NEVER both
  // included at once: that would double-count the same underlying fact,
  // whether resolved silently (dropping one from the aggregate), automatically
  // preferring the correction, or left for evaluateFieldEvaluation to reject
  // after the fact. The build itself fails closed, immediately, with a named
  // explicit error.
  for (const entry of resolvedById.values()) {
    if (typeof entry.supersedes_id === 'string' && resolvedById.has(entry.supersedes_id)) {
      throw new Error(`buildFieldEvaluationReport: included_observations contains both observation "${entry.id}" and the observation "${entry.supersedes_id}" it supersedes; a correction and the record it corrects are never both included`);
    }
  }
  const excludedMetric = new Set();
  for (const { entry } of sortedExcluded) {
    if (typeof entry.metric_id === 'string') excludedMetric.add(entry.metric_id);
  }

  const perMetric = METRIC_IDS.map((mid) => {
    const usable = [];
    let anyNotApplicable = false;
    let anyOther = false;
    const sampleIds = [];
    for (const [oid, entry] of resolvedById) {
      if (entry.metric_id !== mid) continue;
      sampleIds.push(oid);
      if (resolvedIsUsable(entry)) usable.push(entry);
      else if (entry.status === 'not_applicable') anyNotApplicable = true;
      else anyOther = true;
    }
    sampleIds.sort();
    const aggregate = computeMetricAggregate(mid, usable);
    let status;
    if (usable.length > 0) {
      status = 'known';
      if (METRIC_MEASUREMENT_KIND[mid] === 'classification' && aggregate.total === 0) status = 'unknown';
    } else if (anyNotApplicable && !anyOther) {
      status = 'not_applicable';
    } else {
      status = 'unknown';
    }
    const limitations = [];
    if (status !== 'known') {
      limitations.push(status === 'not_applicable'
        ? `для показателя "${mid}" в периоде не зафиксировано применимых наблюдений`
        : `для показателя "${mid}" в периоде нет подтверждённого измерения`);
    }
    if (mid === 'post-acceptance-defects' && aggregate.total === 0
      && (aggregate.coverage === 'partial' || aggregate.coverage === 'mixed')) {
      limitations.push('нулевое число дефектов зафиксировано при неполном окне наблюдения и не является доказательством их отсутствия');
    }
    if (excludedMetric.has(mid)) {
      limitations.push(`по показателю "${mid}" есть явно исключённое наблюдение; состав выборки указан в excluded_observations`);
    }
    const entry = {
      metric_id: mid,
      status,
      aggregate,
      sample_observation_ids: sampleIds,
      limitations,
    };
    if (status !== 'known') {
      entry.status_reason = status === 'not_applicable'
        ? 'все относящиеся наблюдения в периоде отмечены как неприменимые'
        : 'в периоде нет ни одного подтверждённого наблюдения этого показателя';
    }
    return entry;
  });

  return {
    workspace_id: workspaceId,
    period,
    included_observations: sortedIncluded.map((x) => x.ref),
    excluded_observations: sortedExcluded.map((x) => x.ex),
    per_metric: perMetric,
  };
}
