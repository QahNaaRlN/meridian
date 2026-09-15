// ---------------------------------------------------------------------------
// task specification: the one checkable implementation
// ---------------------------------------------------------------------------
// registries/operating-model/task-specification.schema.json is the COMPLETE
// schema for one task specification (record_type: task-specification): a record
// declares it in its own $schema, so a consumer that follows the declaration
// validates the whole record — the reused scoped-record envelope AND the
// specialised body — in one pass. Specification DATA is Instance, like the
// intake register and the instruction source registry — the Kernel ships the
// schema, the product-neutral fixtures and this module. scripts/kernel-validate.mjs
// (the `task-specification-contract` section) and test/task-specification.test.mjs
// both call the functions here so the gate and the standalone set cannot drift.
//
// Nothing in this module reads process state, touches the filesystem or exits.
// It takes a parsed record, the two schemas and the built-in task-pattern list
// ({ id, work_kind, change_class }), and returns a flat list of problem
// strings — empty means valid.
//
// Boundaries this module enforces that the JSON Schema subset cannot:
//   - the record's $schema declaration is a portable relative reference that
//     RESOLVES, within the Meridian namespace from the canonical logical base,
//     to task-specification.schema.json — not merely a matching basename, and
//     not the bare record envelope. The base is a logical address, not a
//     filesystem directory, a repository or a storage mechanism;
//   - the canonical scoped-record envelope is validated SEPARATELY and is never
//     dropped, even though the specialised schema re-states its shape;
//   - a project task specification lives only in project-workspace or
//     repository-scope; user-profile, organization-profile, run-state and
//     built-in-methodology are rejected with the rationale;
//   - the envelope reference strings (origin.source_ref, authority.authority_ref
//     and, when present, authority.decision_ref) are portable too: no rooted
//     machine path and no file:// URL;
//   - the task_pattern reference resolves against the existing catalogue and
//     fails closed on an unknown, ambiguous or inconsistent identifier;
//   - acceptance criteria are non-empty, stably identified, non-duplicate and
//     carry a verifiable condition (a method plus an expected result), not a
//     free phrase;
//   - run state (lifecycle stage, work status, actor, transition history, next
//     step) does not leak into the statement of the work;
//   - the specification is portable: no rooted machine path anywhere in its
//     text — whatever ordinary separator precedes it and whatever the first
//     path segment contains (detection uses no allow-list for that segment:
//     "/@scope", "/$private", "/~service", "/💾", "/Проект", "/数据" all count);
//   - the human-readable name is stated in Russian for the reader.
// ---------------------------------------------------------------------------
import path from 'node:path';

import { validate } from './json-schema.mjs';

export const RECORD_TYPE = 'task-specification';
export const WORK_KINDS = ['assessment', 'operation', 'initiative', 'change'];
export const CHANGE_CLASSES = ['BUGFIX', 'FEATURE', 'BEHAVIOR_CHANGE', 'REFACTOR'];

// The declared $schema is a RESOLVABLE LOGICAL reference — not a basename, and
// not a filesystem path.
// ---------------------------------------------------------------------------
// A task-specification record declares its schema in $schema as a relative
// reference, resolved inside the MERIDIAN NAMESPACE from the canonical logical
// base of a task-specification record. That base is a LOGICAL address; it is
// not a filesystem directory, a repository or a storage mechanism:
//
//   - CANONICAL_RECORD_BASE is the canonical logical resolution base for a
//     task-specification record within the Meridian namespace;
//   - a logical address names no repository, no filesystem directory and no
//     storage mechanism;
//   - a storage adapter maps the logical address onto an actual resource — one
//     adapter may read the built-in schema shipped with the Kernel, another may
//     read working data from a local store, a remote service or the
//     transitional Instance;
//   - moving a record between storage mechanisms changes neither its id, scope,
//     origin and authority, nor the meaning of its $schema;
//   - this module defines the logical resolution contract only; it does not
//     implement persistent storage, Instance migration or any new adapter.
//
// The reference is written from CANONICAL_RECORD_BASE with the same shape and
// depth that standards/workspace/task-pattern-registry.yaml uses for the same
// schema namespace ("../../registries/operating-model/..."). A portable
// reference written from that base resolves to exactly the logical address
// registries/operating-model/task-specification.schema.json; a bare basename, a
// reference into a namespace segment that is not there, and a correct basename
// under a different namespace segment all resolve elsewhere and are rejected.
// Resolution is genuine logical-address resolution — never a basename search.
export const CANONICAL_RECORD_BASE = 'records/task-specification';
export const SCHEMA_NAMESPACE_DIR = 'registries/operating-model';
export const EXPECTED_SCHEMA_BASENAME = 'task-specification.schema.json';
export const ENVELOPE_SCHEMA_BASENAME = 'scoped-record.schema.json';
export const EXPECTED_SCHEMA_REF = `${SCHEMA_NAMESPACE_DIR}/${EXPECTED_SCHEMA_BASENAME}`;
export const ENVELOPE_SCHEMA_REF = `${SCHEMA_NAMESPACE_DIR}/${ENVELOPE_SCHEMA_BASENAME}`;

// A synthetic namespace root the reference is resolved beneath, so that a
// reference climbing past the Meridian namespace root is detectable rather than
// silently clamped. It is a lexical anchor for address normalisation, not a
// filesystem path.
const NAMESPACE_ROOT = '/__meridian_namespace__';

// Resolve a record's declared $schema as a logical address within the Meridian
// namespace, from the canonical logical base. Returns the namespace-root-
// relative address it resolves to, or null when it is not a resolvable
// in-namespace relative reference (an absolute path, a drive-letter path, a
// backslash path, or a reference that climbs out of the namespace root).
export function resolveSchemaRef(declared) {
  if (typeof declared !== 'string' || declared === '') return null;
  if (declared.includes('\\')) return null;
  if (path.posix.isAbsolute(declared) || /^[A-Za-z]:/.test(declared)) return null;
  const from = `${NAMESPACE_ROOT}/${CANONICAL_RECORD_BASE}`;
  const resolved = path.posix.normalize(`${from}/${declared}`);
  if (resolved !== NAMESPACE_ROOT && !resolved.startsWith(`${NAMESPACE_ROOT}/`)) return null;
  return resolved === NAMESPACE_ROOT ? '' : resolved.slice(NAMESPACE_ROOT.length + 1);
}

// The only two workspace-scope-model areas a project task specification may
// occupy, and the reason each other area is excluded.
export const ALLOWED_SCOPE_TYPES = ['project-workspace', 'repository-scope'];
const SCOPE_REJECTION_REASON = {
  'built-in-methodology': 'built-in-methodology is Kernel methodology, not a place for a concrete work statement',
  'user-profile': 'user-profile holds a user\'s rules and settings, not a work statement',
  'organization-profile': 'organization-profile holds an organisation\'s rules and settings, not a work statement',
  'run-state': 'run-state holds one execution run\'s episodic state, and a specification exists before any run and may drive several',
};

// Fields that describe the RUN, not the statement of the work. execution-state-model
// owns them (standards/workspace/task-specification.md §5). Named here so a
// leak produces a pointed message, not just "additional property not allowed".
export const RUN_STATE_FIELDS = [
  'lifecycle_stage', 'work_status', 'scope_revision', 'current_actor', 'actor',
  'supervision_mode', 'resolved_norms', 'completed_checks', 'blockers',
  'next_action', 'next_gate', 'transition_history', 'workflow', 'run_state',
];

const SEMANTIC_ID_RE = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;
const isObject = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);
const blank = (s) => typeof s !== 'string' || s.trim() === '';
const hasCyrillic = (s) => typeof s === 'string' && /[Ѐ-ӿ]/.test(s);

// Any ROOTED machine path, a drive-letter path, a backslash-separated path, a
// "~/" home reference or a file:// URL anywhere in a specification's text
// breaks portability. Returns a short reason string or null. Well-formed URLs
// with an http(s)/ftp/ssh/git scheme are scrubbed first so their internal
// slashes are not read as filesystem paths. Detection of a rooted path uses NO
// allow-list for the first segment: a "/" at the start of the string is a
// rooted path whatever the segment contains ("/@scope/file", "/$private/file",
// "/~service/file", "/💾", "/Проект/файл", "/数据/文件"); a "/" after an
// ordinary text separator ("a=/x", "a,/x", "a:/x") is rooted too, unless the
// "/" is part of a relative path or a scrubbed URL. An ordinary repo-relative
// path ("src/config/parser", "документы/описание", "packages/@scope/module",
// "relative/$private/file"), "n/a", "24/7", "owner-decision:2026-09-09:example"
// and a normal web URL ("https://example.invalid/api") are NOT flagged. A word
// ending in ":/" ("path:/etc") is a rooted POSIX path, not a drive-letter path
// — only a lone ASCII letter before ":" makes a drive letter.
export function nonPortableReason(s) {
  if (typeof s !== 'string' || s === '') return null;
  if (/\bfile:\/\//i.test(s)) return 'a file:// URL';
  const scrubbed = s.replace(/\b(?:https?|ftp|ftps|ssh|git|mailto):(?:\/\/)?[^\s"'()<>[\]]+/gi, ' ');
  if (/(?:^|[^\p{L}\p{N}._~-])~\//u.test(scrubbed)) return 'a "~/" home-directory reference';
  // A drive-letter path: a LONE ASCII letter, then ":" then a separator. The
  // letter must not be the tail of a longer word, so "path:/etc" is not one.
  if (/(?:^|[^A-Za-z0-9])[A-Za-z]:[\\/][^\s"')>\]]+/.test(scrubbed)) return 'a drive-letter path';
  // A rooted (absolute) POSIX path: "/" then at least one segment character,
  // where the "/" is at the start of the string or right after a character that
  // could not be part of a preceding path segment (letters of any script,
  // digits, ".", "_", "-", "~" are "path characters"; anything else is a
  // boundary). The segment character after "/" is NOT drawn from an allow-list:
  // it is anything that is not whitespace, not "/" and not a quoting/bracketing
  // delimiter — so "/@x", "/$x", "/~x" and "/💾" all count. So "path=/etc",
  // "src:/custom", "/Проект/файл" and "/数据/файл" are caught, but a "/" inside
  // "src/config", "документы/описание", "packages/@scope", "n/a" or "24/7" is
  // not (its "/" follows a path character, so no boundary precedes it).
  if (/(?:^|[^\p{L}\p{N}._~-])\/[^\s/"'()<>[\]][^\s"')>\]]*/u.test(scrubbed)) return 'a rooted (absolute) POSIX path';
  if (/[\p{L}\p{N}_.-]+\\[\p{L}\p{N}_.-]+/u.test(scrubbed)) return 'a backslash-separated path';
  return null;
}

// The whole composition pipeline for one task specification: the canonical
// record envelope (reused, always run), the complete specialised schema (which
// a $schema-follower would also use), then the cross-cutting rules the JSON
// Schema subset cannot state.
export function evaluateTaskSpecification(doc, { recordSchema, envelopeSchema, taskPatterns } = {}) {
  const problems = [];

  // 1. the canonical scoped-record envelope, validated separately and always.
  const envErrs = [];
  try { validate(doc, envelopeSchema, envelopeSchema, '', envErrs); }
  catch (e) { return [`record envelope schema could not be applied: ${e.message}`]; }
  problems.push(...envErrs.map((m) => `envelope ${m}`));

  // 2. the complete specialised schema — the same one a record declares.
  const bodyErrs = [];
  try { validate(doc, recordSchema, recordSchema, '', bodyErrs); }
  catch (e) { return [`task-specification schema could not be applied: ${e.message}`]; }
  problems.push(...bodyErrs);

  if (!isObject(doc)) {
    return problems.length ? problems : ['the task specification is not an object'];
  }

  const id = typeof doc.id === 'string' && doc.id !== '' ? doc.id : '(no id)';

  // 3. the record's $schema declaration must be a portable relative reference
  //    that RESOLVES, within the Meridian namespace from the canonical logical
  //    base, to the specialised schema — not merely carry the right basename.
  const declared = doc.$schema;
  if (typeof declared !== 'string' || declared === '') {
    problems.push(`task specification "${id}" declares no $schema; a record names ${EXPECTED_SCHEMA_BASENAME} so a consumer validates the whole contract, not only the envelope`);
  } else {
    const portability = nonPortableReason(declared);
    if (portability) {
      problems.push(`task specification "${id}" $schema "${declared}" is not portable (${portability}); the schema is named by a relative reference resolved from the canonical logical resolution base (${CANONICAL_RECORD_BASE})`);
    } else {
      const resolved = resolveSchemaRef(declared);
      if (resolved === ENVELOPE_SCHEMA_REF) {
        problems.push(`task specification "${id}" $schema "${declared}" resolves to the record envelope (${ENVELOPE_SCHEMA_BASENAME}); it must name ${EXPECTED_SCHEMA_BASENAME}, which composes the envelope with the body`);
      } else if (resolved !== EXPECTED_SCHEMA_REF) {
        problems.push(`task specification "${id}" $schema "${declared}" does not resolve to the logical address ${EXPECTED_SCHEMA_REF} from the canonical logical resolution base (${CANONICAL_RECORD_BASE}); the specialised schema is named by a relative reference written from that base`);
      }
    }
  }

  // 4. identity and the human-readable Russian name come from the envelope.
  if (doc.record_type !== RECORD_TYPE) {
    problems.push(`task specification "${id}" declares record_type "${doc.record_type}", not "${RECORD_TYPE}"`);
  }
  if (typeof doc.id !== 'string' || !SEMANTIC_ID_RE.test(doc.id)) {
    problems.push(`task specification "${id}" has no stable semantic id on the record envelope`);
  }
  if (typeof doc.title !== 'string' || doc.title.trim() === '') {
    problems.push(`task specification "${id}" has no human-readable title`);
  } else if (!hasCyrillic(doc.title)) {
    problems.push(`task specification "${id}" title "${doc.title}" carries no Russian (Cyrillic) text; the specification name is stated in Russian for the human reader`);
  }

  // 5. a project task specification lives only in project-workspace or
  //    repository-scope.
  const scope = isObject(doc.scope) ? doc.scope : {};
  if (typeof scope.type === 'string' && !ALLOWED_SCOPE_TYPES.includes(scope.type)) {
    const why = SCOPE_REJECTION_REASON[scope.type] || 'it is not one of the two areas a project task specification may occupy';
    problems.push(`task specification "${id}" is scoped to "${scope.type}"; a project task specification lives in project-workspace or repository-scope only — ${why}`);
  }
  const origin = isObject(doc.origin) ? doc.origin : {};
  if (origin.kind === 'built-in') {
    problems.push(`task specification "${id}" declares origin.kind "built-in"; a concrete specification is authored in a workspace, not shipped with the methodology`);
  }
  const authority = isObject(doc.authority) ? doc.authority : {};

  // 5a. the envelope reference strings travel inside the portable record too: a
  //     rooted machine path or a file:// URL in origin.source_ref,
  //     authority.authority_ref or authority.decision_ref breaks portability
  //     exactly as it does in the prose fields.
  for (const [obj, field, label] of [
    [origin, 'source_ref', 'origin.source_ref'],
    [authority, 'authority_ref', 'authority.authority_ref'],
    [authority, 'decision_ref', 'authority.decision_ref'],
  ]) {
    const val = obj[field];
    if (typeof val === 'string' && val !== '') {
      const r = nonPortableReason(val);
      if (r) problems.push(`task specification "${id}" ${label} contains ${r}; a specification is portable and carries no rooted machine path`);
    }
  }

  const payload = isObject(doc.payload) ? doc.payload : {};

  // 6. run state must not leak into the statement of the work.
  for (const f of RUN_STATE_FIELDS) {
    if (f in payload) {
      problems.push(`task specification "${id}" payload carries "${f}"; run state (stage, status, actor, transition history, next step) belongs to execution-state-model, not to the specification`);
    }
  }
  if ('record_type' in payload) {
    problems.push(`task specification "${id}" repeats record_type inside the payload; the record type is declared once, on the envelope`);
  }

  // 7. the mandatory prose inputs: present, not whitespace-only, portable.
  for (const [f, label] of [['goal', 'goal'], ['initial_state', 'initial state'], ['target_model', 'target model']]) {
    if (!(f in payload)) {
      problems.push(`task specification "${id}" states no ${label}; it is a required input with no default`);
    } else if (blank(payload[f])) {
      problems.push(`task specification "${id}" ${label} is empty or whitespace-only`);
    } else {
      const r = nonPortableReason(payload[f]);
      if (r) problems.push(`task specification "${id}" ${label} contains ${r}; a specification is portable and carries no rooted machine path`);
    }
  }
  const t = nonPortableReason(doc.title);
  if (t) problems.push(`task specification "${id}" title contains ${t}`);

  // 8. constraints: a non-empty set, every item real and portable.
  const constraints = Array.isArray(payload.constraints) ? payload.constraints : null;
  if (!constraints || constraints.length === 0) {
    problems.push(`task specification "${id}" carries no constraints; the set must be non-empty`);
  } else {
    const seen = new Set();
    constraints.forEach((c, i) => {
      if (blank(c)) {
        problems.push(`task specification "${id}" constraint #${i} is empty or whitespace-only`);
        return;
      }
      const r = nonPortableReason(c);
      if (r) problems.push(`task specification "${id}" constraint #${i} contains ${r}`);
      const key = c.trim();
      if (seen.has(key)) problems.push(`task specification "${id}" repeats constraint "${c}"`);
      seen.add(key);
    });
  }

  // 9. acceptance criteria: non-empty, stably identified, non-duplicate, and
  //    each one verifiable — a method plus an observable expected result.
  const crits = Array.isArray(payload.acceptance_criteria) ? payload.acceptance_criteria : null;
  if (!crits || crits.length === 0) {
    problems.push(`task specification "${id}" carries no acceptance criteria; the set must be non-empty`);
  } else {
    const seenId = new Set();
    const seenBody = new Set();
    crits.forEach((c, i) => {
      const co = isObject(c) ? c : {};
      const cid = typeof co.id === 'string' ? co.id : null;
      const at = cid || `#${i}`;
      if (!cid || !SEMANTIC_ID_RE.test(cid)) {
        problems.push(`task specification "${id}" acceptance criterion ${at} has no stable identifier (a semantic id)`);
      } else {
        if (seenId.has(cid)) problems.push(`task specification "${id}" acceptance criterion id "${cid}" is used more than once`);
        seenId.add(cid);
      }
      if (blank(co.statement)) {
        problems.push(`task specification "${id}" acceptance criterion ${at} has no statement`);
      }
      const v = isObject(co.verification) ? co.verification : null;
      if (!v || blank(v.method) || blank(v.expected_result)) {
        problems.push(`task specification "${id}" acceptance criterion ${at} has no verifiable condition; a criterion states a method and an observable expected_result, not a free phrase`);
      }
      for (const s of [co.statement, v && v.method, v && v.expected_result]) {
        const r = nonPortableReason(s);
        if (r) problems.push(`task specification "${id}" acceptance criterion ${at} contains ${r}`);
      }
      const bodyKey = JSON.stringify([
        typeof co.statement === 'string' ? co.statement.trim() : co.statement,
        v ? [String(v.method).trim(), String(v.expected_result).trim()] : v,
      ]);
      if (seenBody.has(bodyKey)) {
        problems.push(`task specification "${id}" acceptance criterion ${at} duplicates another criterion's statement and verification`);
      }
      seenBody.add(bodyKey);
    });
  }

  // 10. the task-pattern reference resolves against the existing catalogue.
  const tp = isObject(payload.task_pattern) ? payload.task_pattern : null;
  if (!tp || typeof tp.id !== 'string' || tp.id === '') {
    problems.push(`task specification "${id}" names no task pattern; the specification selects exactly one pattern from task-pattern-registry`);
  } else {
    const catalogue = Array.isArray(taskPatterns) ? taskPatterns : [];
    const matches = catalogue.filter((p) => p && p.id === tp.id);
    if (matches.length === 0) {
      problems.push(`task specification "${id}" references task pattern "${tp.id}", which is not in task-pattern-registry`);
    } else if (matches.length > 1) {
      problems.push(`task specification "${id}" references task pattern "${tp.id}", which is declared ${matches.length} times in task-pattern-registry — the reference is ambiguous`);
    } else {
      const p = matches[0];
      const patternCc = p.change_class == null ? null : p.change_class;
      if (tp.work_kind !== undefined && tp.work_kind !== p.work_kind) {
        problems.push(`task specification "${id}" declares work_kind "${tp.work_kind}" but task pattern "${tp.id}" is work_kind "${p.work_kind}"`);
      }
      if (tp.change_class !== undefined && (tp.change_class || null) !== patternCc) {
        problems.push(`task specification "${id}" declares change_class "${tp.change_class ?? 'none'}" but task pattern "${tp.id}" is change_class "${patternCc ?? 'none'}"`);
      }
    }
  }

  return problems;
}
