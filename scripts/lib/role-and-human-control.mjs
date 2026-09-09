// ---------------------------------------------------------------------------
// role and human control: the one checkable implementation
// ---------------------------------------------------------------------------
// This package ships TWO product-neutral Kernel contracts and one checkable
// implementation shared by scripts/kernel-validate.mjs (the
// `role-and-human-control` section) and test/role-and-human-control.test.mjs, so
// the gate and the standalone set cannot drift:
//
//   1. registries/operating-model/role-registry.schema.json — the COMPLETE
//      schema for the built-in, product-neutral catalogue of universal roles
//      (record_type: role-registry). The catalogue is ONE scoped record; it
//      declares the schema in its own $schema, so a consumer that follows the
//      declaration validates the reused scoped-record envelope AND the body in
//      one pass. standards/workspace/role-registry.yaml is the catalogue.
//
//   2. registries/operating-model/human-control.schema.json — the COMPLETE
//      schema for the human-control state of ONE execution run
//      (record_type: run-human-control): role assignments, the acting
//      participant, the permanent human-in-command posture and its holder, the
//      switchable supervision mode, the communication mode, an optional
//      independent-review requirement and the ordered history of changes to
//      every mutable control axis. Concrete control records are Instance data
//      (later the working-data area); the Kernel ships the schema, the
//      product-neutral fixtures and this module — no canonical data file.
//
// Nothing in this module reads process state, touches the filesystem or exits.
// It takes a parsed record and the two schemas and returns a flat list of
// problem strings — empty means valid.
//
// Boundaries this module enforces that the JSON Schema subset cannot:
//   - the record's $schema declaration is a portable relative reference that
//     RESOLVES, within the Meridian namespace, to the right specialised schema —
//     not merely a matching basename, and not the bare record envelope. The
//     resolution and the rooted-path rules are REUSED from
//     scripts/lib/task-specification.mjs (resolveSchemaRef, nonPortableReason);
//   - the canonical scoped-record envelope is validated SEPARATELY and is never
//     dropped;
//   - the built-in role catalogue lives only in built-in-methodology, carries
//     every one of the seven universal roles exactly once, and names no
//     programme, AI model, vendor or person;
//   - a concrete control record lives only in run-state, carries workspace_id
//     and a portable reference to EXACTLY ONE execution run, and rejects
//     origin.kind built-in;
//   - human-in-command is the permanent basis: it is not a supervision mode,
//     cannot be disabled, and its holder is a participant assigned the owner
//     role; human-in-the-loop needs an hitl block and FORBIDS an hotl block;
//     human-on-the-loop needs an hotl block and FORBIDS an hitl block;
//   - role, supervision mode, human authority, the acting actor and the
//     communication mode are independent axes; one actor may hold several
//     roles; the base owner-plus-one-executor model is accepted with no
//     reviewer; a required independence needs a separate suitable actor and the
//     independent reviewer is never the reviewed actor; a required:false block
//     carries no reviewer/reviewed reference; every present reference is
//     portable whatever the value of required;
//   - the acting actor, and every actor a switch record moves to, is assigned
//     in the matching role-assignment set;
//   - the switch history is ordered by a strictly increasing sequence, its
//     first record states the initial establishment (all four from_* axes
//     null), each later record continues the previous one on EVERY mutable
//     axis with no break, every record states a basis and preserves a
//     verifiable checkpoint, and the last record matches the WHOLE current
//     control state — supervision mode, communication mode, acting actor and
//     the role-assignment set; a change never opens a new run and never
//     restates execution-run body state;
//   - every reference and prose string is portable; the human-readable name is
//     stated in Russian.
// ---------------------------------------------------------------------------
import { validate } from './json-schema.mjs';
import { nonPortableReason, resolveSchemaRef } from './task-specification.mjs';

// Reused unchanged from the task specification contract: the same Meridian-
// namespace address math and the same rooted-path detection, so this package
// carries no divergent second copy of either rule.
export { nonPortableReason, resolveSchemaRef };

export const ROLE_REGISTRY_RECORD_TYPE = 'role-registry';
export const HUMAN_CONTROL_RECORD_TYPE = 'run-human-control';

// The closed pool of universal roles. Named from the operating glossary
// (`role`): a set of powers and responsibilities, never a participant, a
// programme, an AI model or a vendor. One participant may hold several.
export const ROLES = [
  'owner', 'operator', 'executor', 'reviewer', 'verifier', 'git_integrator', 'deployer',
];

// The closed, switchable supervision-mode pool. human-in-command is NOT a
// member: it is the permanent basis on its own axis (human_authority).
export const SUPERVISION_MODES = ['human-in-the-loop', 'human-on-the-loop'];

// The permanent human command posture. It is a constant, not a choice.
export const HUMAN_AUTHORITY_POSTURE = 'human-in-command';

// The closed communication-mode pool, modelled apart from role and supervision.
export const COMMUNICATION_MODES = ['owner_relayed', 'direct'];

// The role that holds human-in-command.
export const HUMAN_AUTHORITY_ROLE = 'owner';

// Roles that make an actor a suitable INDEPENDENT reviewer of a result.
export const INDEPENDENT_REVIEW_ROLES = ['reviewer', 'verifier'];

// The mutable control axes a switch record tracks, each with a from_/to_ pair.
export const CONTROL_AXES = ['supervision_mode', 'communication_mode', 'acting_actor', 'role_assignments'];

// The canonical LOGICAL resolution bases within the Meridian namespace. They
// are logical addresses — not filesystem directories, repositories or storage
// mechanisms. Resolution itself is delegated to task-specification.mjs's
// resolveSchemaRef; these constants document the conceptual bases.
export const CANONICAL_REGISTRY_BASE = 'records/role-registry';
export const CANONICAL_CONTROL_BASE = 'records/run-human-control';
export const SCHEMA_NAMESPACE_DIR = 'registries/operating-model';
export const REGISTRY_SCHEMA_BASENAME = 'role-registry.schema.json';
export const CONTROL_SCHEMA_BASENAME = 'human-control.schema.json';
export const ENVELOPE_SCHEMA_BASENAME = 'scoped-record.schema.json';
export const REGISTRY_SCHEMA_REF = `${SCHEMA_NAMESPACE_DIR}/${REGISTRY_SCHEMA_BASENAME}`;
export const CONTROL_SCHEMA_REF = `${SCHEMA_NAMESPACE_DIR}/${CONTROL_SCHEMA_BASENAME}`;
export const ENVELOPE_SCHEMA_REF = `${SCHEMA_NAMESPACE_DIR}/${ENVELOPE_SCHEMA_BASENAME}`;

export const REGISTRY_SCOPE_TYPE = 'built-in-methodology';
export const CONTROL_SCOPE_TYPE = 'run-state';

const CONTROL_SCOPE_REJECTION_REASON = {
  'built-in-methodology': 'built-in-methodology is Kernel methodology, not a place for one run\'s control state',
  'user-profile': 'user-profile holds a user\'s rules and settings, not run control state',
  'organization-profile': 'organization-profile holds an organisation\'s rules and settings, not run control state',
  'project-workspace': 'project-workspace holds the project\'s goals and decisions; a run\'s control state is scoped to run-state',
  'repository-scope': 'repository-scope holds facts true for one repository; a run may span repositories and is scoped to run-state',
};

const SEMANTIC_ID_RE = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;
const isObject = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);
const blank = (s) => typeof s !== 'string' || s.trim() === '';
const hasCyrillic = (s) => typeof s === 'string' && /[Ѐ-ӿ]/.test(s);
const isPositiveInt = (n) => typeof n === 'number' && Number.isInteger(n) && n > 0;
// Exact structural equality, used to check continuity of a snapshot axis
// (the role-assignment set) between one switch record and the next.
const sameAssignmentSet = (a, b) => JSON.stringify(a) === JSON.stringify(b);

// Shared: apply the reused envelope schema separately, then the specialised
// schema. Returns { problems, fatal } — fatal means a schema could not be
// applied at all and the caller returns immediately.
function applySchemas(doc, envelopeSchema, recordSchema, label) {
  const problems = [];
  const envErrs = [];
  try { validate(doc, envelopeSchema, envelopeSchema, '', envErrs); }
  catch (e) { return { problems: [`record envelope schema could not be applied: ${e.message}`], fatal: true }; }
  problems.push(...envErrs.map((m) => `envelope ${m}`));

  const bodyErrs = [];
  try { validate(doc, recordSchema, recordSchema, '', bodyErrs); }
  catch (e) { return { problems: [`${label} schema could not be applied: ${e.message}`], fatal: true }; }
  problems.push(...bodyErrs);
  return { problems, fatal: false };
}

// Shared: the record's $schema declaration must be a portable relative
// reference that RESOLVES, within the Meridian namespace, to `expectedRef`.
function checkSchemaDeclaration(problems, doc, id, { expectedRef, expectedBasename, canonicalBase, label }) {
  const declared = doc.$schema;
  if (typeof declared !== 'string' || declared === '') {
    problems.push(`${label} "${id}" declares no $schema; a record names ${expectedBasename} so a consumer validates the whole contract — id, scope, origin, authority and body — not only the envelope`);
    return;
  }
  const portability = nonPortableReason(declared);
  if (portability) {
    problems.push(`${label} "${id}" $schema "${declared}" is not portable (${portability}); the schema is named by a relative reference resolved inside the Meridian namespace (canonical logical base ${canonicalBase})`);
    return;
  }
  const resolved = resolveSchemaRef(declared);
  if (resolved === ENVELOPE_SCHEMA_REF) {
    problems.push(`${label} "${id}" $schema "${declared}" resolves to the record envelope (${ENVELOPE_SCHEMA_BASENAME}); it must name ${expectedBasename}, which composes the envelope with the body`);
  } else if (resolved !== expectedRef) {
    problems.push(`${label} "${id}" $schema "${declared}" does not resolve to the logical address ${expectedRef} within the Meridian namespace; the specialised schema is named by a portable relative reference (a bare basename, a missing namespace segment, a wrong segment and a reference climbing out of the namespace all resolve elsewhere)`);
  }
}

// Shared: the envelope reference strings travel inside the portable record too.
function checkEnvelopeRefs(problems, doc, id, label) {
  const origin = isObject(doc.origin) ? doc.origin : {};
  const authority = isObject(doc.authority) ? doc.authority : {};
  for (const [obj, field, name] of [
    [origin, 'source_ref', 'origin.source_ref'],
    [authority, 'authority_ref', 'authority.authority_ref'],
    [authority, 'decision_ref', 'authority.decision_ref'],
  ]) {
    const val = obj[field];
    if (typeof val === 'string' && val !== '') {
      const r = nonPortableReason(val);
      if (r) problems.push(`${label} "${id}" ${name} contains ${r}; the record is portable and carries no rooted machine path`);
    }
  }
}

// Shared: validate one role-assignment set. Returns { actors: Set, actorRoles:
// Map<actor, Set<role>> } and pushes problems. Rejects a blank actor, an empty
// or unknown roles list, a duplicate role within an actor, a duplicate actor
// and a duplicate (actor, role) pair. Pair uniqueness uses a nested
// Map/Set — no concatenated text key, no control or binary characters.
function checkAssignmentSet(problems, id, label, list) {
  const actors = new Set();
  const actorRoles = new Map();
  const arr = Array.isArray(list) ? list : null;
  if (!arr || arr.length === 0) {
    problems.push(`human-control record "${id}" ${label} carries no role assignments; a run has at least one assigned participant`);
    return { actors, actorRoles };
  }
  arr.forEach((a, i) => {
    const ao = isObject(a) ? a : {};
    const actor = typeof ao.actor === 'string' ? ao.actor : null;
    const at = actor && !blank(actor) ? `"${actor}"` : `#${i}`;
    if (!actor || blank(actor)) {
      problems.push(`human-control record "${id}" ${label} assignment ${at} names no actor; an empty or ambiguous assignment is rejected`);
    } else {
      const r = nonPortableReason(actor);
      if (r) problems.push(`human-control record "${id}" ${label} assignment ${at} actor contains ${r}`);
      if (actors.has(actor)) problems.push(`human-control record "${id}" ${label} assigns actor "${actor}" more than once; combine an actor's roles in one assignment`);
      actors.add(actor);
      if (!actorRoles.has(actor)) actorRoles.set(actor, new Set());
    }
    const roles = Array.isArray(ao.roles) ? ao.roles : null;
    if (!roles || roles.length === 0) {
      problems.push(`human-control record "${id}" ${label} assignment ${at} lists no role; an empty or ambiguous assignment is rejected`);
      return;
    }
    const here = actor && !blank(actor) ? actorRoles.get(actor) : new Set();
    roles.forEach((role) => {
      if (typeof role !== 'string' || !ROLES.includes(role)) {
        problems.push(`human-control record "${id}" ${label} assignment ${at} names role "${role}", which is not in the closed pool ${ROLES.join(', ')}`);
        return;
      }
      if (here.has(role)) problems.push(`human-control record "${id}" ${label} assignment ${at} assigns role "${role}" more than once`);
      here.add(role);
    });
  });
  return { actors, actorRoles };
}

// ---------------------------------------------------------------------------
// 1. the built-in role catalogue
// ---------------------------------------------------------------------------
export function evaluateRoleRegistry(doc, { registrySchema, envelopeSchema } = {}) {
  const { problems, fatal } = applySchemas(doc, envelopeSchema, registrySchema, 'role-registry');
  if (fatal) return problems;

  if (!isObject(doc)) {
    return problems.length ? problems : ['the role registry record is not an object'];
  }
  const id = typeof doc.id === 'string' && doc.id !== '' ? doc.id : '(no id)';

  checkSchemaDeclaration(problems, doc, id, {
    expectedRef: REGISTRY_SCHEMA_REF,
    expectedBasename: REGISTRY_SCHEMA_BASENAME,
    canonicalBase: CANONICAL_REGISTRY_BASE,
    label: 'role registry',
  });

  if (doc.record_type !== ROLE_REGISTRY_RECORD_TYPE) {
    problems.push(`role registry "${id}" declares record_type "${doc.record_type}", not "${ROLE_REGISTRY_RECORD_TYPE}"; the record type names the catalogue and does not open a second envelope`);
  }
  if (typeof doc.id !== 'string' || !SEMANTIC_ID_RE.test(doc.id)) {
    problems.push(`role registry "${id}" has no stable semantic id on the record envelope`);
  }
  if (typeof doc.title !== 'string' || doc.title.trim() === '') {
    problems.push(`role registry "${id}" has no human-readable title`);
  } else if (!hasCyrillic(doc.title)) {
    problems.push(`role registry "${id}" title "${doc.title}" carries no Russian (Cyrillic) text; the catalogue name is stated in Russian for the human reader`);
  }
  const titleReason = nonPortableReason(doc.title);
  if (titleReason) problems.push(`role registry "${id}" title contains ${titleReason}`);

  const scope = isObject(doc.scope) ? doc.scope : {};
  if (typeof scope.type === 'string' && scope.type !== REGISTRY_SCOPE_TYPE) {
    problems.push(`role registry "${id}" is scoped to "${scope.type}"; the built-in role catalogue is Kernel methodology and lives in built-in-methodology only`);
  }
  const origin = isObject(doc.origin) ? doc.origin : {};
  if ('kind' in origin && origin.kind !== 'built-in') {
    problems.push(`role registry "${id}" declares origin.kind "${origin.kind}"; the built-in role catalogue is shipped with the methodology (origin.kind built-in)`);
  }
  const authority = isObject(doc.authority) ? doc.authority : {};
  if ('kind' in authority && authority.kind !== 'methodology-owner') {
    problems.push(`role registry "${id}" declares authority.kind "${authority.kind}"; the built-in role catalogue is methodology-owner authority`);
  }
  checkEnvelopeRefs(problems, doc, id, 'role registry');

  const payload = isObject(doc.payload) ? doc.payload : {};
  const roles = Array.isArray(payload.roles) ? payload.roles : null;
  if (!roles) {
    if ('roles' in payload) problems.push(`role registry "${id}" payload.roles is not a list`);
    else problems.push(`role registry "${id}" states no roles; the catalogue body carries the closed pool of universal roles`);
  } else {
    const seen = new Set();
    roles.forEach((r, i) => {
      const ro = isObject(r) ? r : {};
      const rid = typeof ro.id === 'string' ? ro.id : null;
      const at = rid || `#${i}`;
      if (!rid || !ROLES.includes(rid)) {
        problems.push(`role registry "${id}" role ${at} is not one of the closed pool ${ROLES.join(', ')}`);
      } else {
        if (seen.has(rid)) problems.push(`role registry "${id}" role "${rid}" is declared more than once`);
        seen.add(rid);
      }
      if (blank(ro.title)) problems.push(`role registry "${id}" role ${at} has no Russian title`);
      else if (!hasCyrillic(ro.title)) problems.push(`role registry "${id}" role ${at} title "${ro.title}" carries no Russian (Cyrillic) text`);
      if (blank(ro.summary)) problems.push(`role registry "${id}" role ${at} has no summary`);
      const resp = Array.isArray(ro.responsibilities) ? ro.responsibilities : null;
      if (!resp || resp.length === 0) {
        problems.push(`role registry "${id}" role ${at} states no responsibilities; a universal role names its powers and duties`);
      } else {
        resp.forEach((s, j) => {
          if (blank(s)) problems.push(`role registry "${id}" role ${at} responsibility #${j} is empty or whitespace-only`);
          const pr = nonPortableReason(s);
          if (pr) problems.push(`role registry "${id}" role ${at} responsibility #${j} contains ${pr}`);
        });
      }
      for (const s of [ro.title, ro.summary]) {
        const pr = nonPortableReason(s);
        if (pr) problems.push(`role registry "${id}" role ${at} contains ${pr}`);
      }
    });
    const missing = ROLES.filter((r) => !seen.has(r));
    if (missing.length) {
      problems.push(`role registry "${id}" is missing universal role(s) ${missing.join(', ')}; the pool is closed and fully covered`);
    }
  }

  return problems;
}

// ---------------------------------------------------------------------------
// 2. one run's human-control record
// ---------------------------------------------------------------------------
export function evaluateHumanControl(doc, { recordSchema, envelopeSchema } = {}) {
  const { problems, fatal } = applySchemas(doc, envelopeSchema, recordSchema, 'human-control');
  if (fatal) return problems;

  if (!isObject(doc)) {
    return problems.length ? problems : ['the human-control record is not an object'];
  }
  const id = typeof doc.id === 'string' && doc.id !== '' ? doc.id : '(no id)';

  checkSchemaDeclaration(problems, doc, id, {
    expectedRef: CONTROL_SCHEMA_REF,
    expectedBasename: CONTROL_SCHEMA_BASENAME,
    canonicalBase: CANONICAL_CONTROL_BASE,
    label: 'human-control record',
  });

  if (doc.record_type !== HUMAN_CONTROL_RECORD_TYPE) {
    problems.push(`human-control record "${id}" declares record_type "${doc.record_type}", not "${HUMAN_CONTROL_RECORD_TYPE}"`);
  }
  if (typeof doc.id !== 'string' || !SEMANTIC_ID_RE.test(doc.id)) {
    problems.push(`human-control record "${id}" has no stable semantic id on the record envelope`);
  }
  if (typeof doc.title !== 'string' || doc.title.trim() === '') {
    problems.push(`human-control record "${id}" has no human-readable title`);
  } else if (!hasCyrillic(doc.title)) {
    problems.push(`human-control record "${id}" title "${doc.title}" carries no Russian (Cyrillic) text; the record name is stated in Russian for the human reader`);
  }
  const titleReason = nonPortableReason(doc.title);
  if (titleReason) problems.push(`human-control record "${id}" title contains ${titleReason}`);

  const scope = isObject(doc.scope) ? doc.scope : {};
  if (typeof scope.type === 'string' && scope.type !== CONTROL_SCOPE_TYPE) {
    const why = CONTROL_SCOPE_REJECTION_REASON[scope.type] || 'it is not run-state';
    problems.push(`human-control record "${id}" is scoped to "${scope.type}"; a run's control state lives in run-state only — ${why}`);
  }
  if (scope.type === CONTROL_SCOPE_TYPE) {
    if (typeof scope.id !== 'string' || !SEMANTIC_ID_RE.test(scope.id)) {
      problems.push(`human-control record "${id}" run-state scope carries no stable scope.id identifying the run`);
    }
    if (typeof scope.workspace_id !== 'string' || !SEMANTIC_ID_RE.test(scope.workspace_id)) {
      problems.push(`human-control record "${id}" run-state scope carries no workspace_id; a run belongs to a project workspace (workspace-scope-model.md §1)`);
    }
  }
  const origin = isObject(doc.origin) ? doc.origin : {};
  if (origin.kind === 'built-in') {
    problems.push(`human-control record "${id}" declares origin.kind "built-in"; a run's control state is established in a workspace, not shipped with the methodology`);
  }
  checkEnvelopeRefs(problems, doc, id, 'human-control record');

  const payload = isObject(doc.payload) ? doc.payload : {};

  // exactly one execution run, by a portable reference, never embedded.
  const runRef = payload.execution_run_ref;
  if (!('execution_run_ref' in payload)) {
    problems.push(`human-control record "${id}" names no execution_run_ref; a control record references exactly one execution run`);
  } else if (isObject(runRef) || Array.isArray(runRef)) {
    problems.push(`human-control record "${id}" execution_run_ref is not a plain reference; the record references the run, it does not embed run state`);
  } else if (blank(runRef)) {
    problems.push(`human-control record "${id}" execution_run_ref is empty or whitespace-only`);
  } else {
    const r = nonPortableReason(runRef);
    if (r) problems.push(`human-control record "${id}" execution_run_ref contains ${r}; the reference is portable and is not an absolute machine path`);
  }

  // current role assignments — validated first, so later checks can look
  // participants up in the assigned set.
  const current = checkAssignmentSet(problems, id, 'current', payload.role_assignments);

  // human-in-command is the permanent basis and is bound to an assigned owner.
  const authorityAxis = isObject(payload.human_authority) ? payload.human_authority : null;
  if (!authorityAxis || authorityAxis.posture !== HUMAN_AUTHORITY_POSTURE) {
    problems.push(`human-control record "${id}" does not state human_authority.posture "${HUMAN_AUTHORITY_POSTURE}"; human-in-command is the permanent basis of Meridian and cannot be replaced by a supervision mode or disabled`);
  }
  if (authorityAxis) {
    const holder = typeof authorityAxis.holder === 'string' ? authorityAxis.holder : null;
    if (!holder || blank(holder)) {
      problems.push(`human-control record "${id}" human_authority names no holder; the record identifies the participant who holds human-in-command`);
    } else {
      const r = nonPortableReason(holder);
      if (r) problems.push(`human-control record "${id}" human_authority.holder contains ${r}`);
      if (!(current.actorRoles.get(holder) && current.actorRoles.get(holder).has(HUMAN_AUTHORITY_ROLE))) {
        problems.push(`human-control record "${id}" human_authority.holder "${holder}" is not a participant assigned the ${HUMAN_AUTHORITY_ROLE} role; human-in-command is held by an assigned owner`);
      }
    }
  }

  // supervision mode: closed pool, not HIC; hitl / hotl mutually exclusive.
  const supervision = payload.supervision_mode;
  if (supervision === HUMAN_AUTHORITY_POSTURE) {
    problems.push(`human-control record "${id}" sets supervision_mode to "${HUMAN_AUTHORITY_POSTURE}"; human-in-command is not a switchable supervision mode — the switchable pool is ${SUPERVISION_MODES.join(', ')}`);
  } else if (typeof supervision === 'string' && !SUPERVISION_MODES.includes(supervision)) {
    problems.push(`human-control record "${id}" supervision_mode "${supervision}" is not in the closed pool ${SUPERVISION_MODES.join(', ')}`);
  }
  const hasHitl = 'hitl' in payload;
  const hasHotl = 'hotl' in payload;
  if (supervision === 'human-in-the-loop') {
    const hitl = isObject(payload.hitl) ? payload.hitl : null;
    if (!hitl || blank(hitl.required_human_action) || blank(hitl.gate)) {
      problems.push(`human-control record "${id}" is human-in-the-loop but names no concrete required human action and declared gate; HITL means a specific human action on a stated gate is known`);
    }
    if (hasHotl) {
      problems.push(`human-control record "${id}" is human-in-the-loop but also carries an hotl block; the modes are mutually exclusive — human-in-the-loop forbids hotl`);
    }
    for (const s of [hitl && hitl.required_human_action, hitl && hitl.gate]) {
      const r = nonPortableReason(s);
      if (r) problems.push(`human-control record "${id}" hitl block contains ${r}`);
    }
  }
  if (supervision === 'human-on-the-loop') {
    const hotl = isObject(payload.hotl) ? payload.hotl : null;
    const bounds = hotl && Array.isArray(hotl.autonomy_bounds) ? hotl.autonomy_bounds : null;
    if (!hotl || !bounds || bounds.length === 0 || blank(hotl.intervention)) {
      problems.push(`human-control record "${id}" is human-on-the-loop but states no autonomy bounds or intervention capability; HOTL means bounded autonomy inside pre-set bounds with a standing way to intervene`);
    }
    if (hasHitl) {
      problems.push(`human-control record "${id}" is human-on-the-loop but also carries an hitl block; the modes are mutually exclusive — human-on-the-loop forbids hitl`);
    }
    for (const s of [...(bounds || []), hotl && hotl.intervention]) {
      const r = nonPortableReason(s);
      if (r) problems.push(`human-control record "${id}" hotl block contains ${r}`);
    }
  }

  // communication mode: closed pool (also enforced by the schema).
  if (typeof payload.communication_mode === 'string' && !COMMUNICATION_MODES.includes(payload.communication_mode)) {
    problems.push(`human-control record "${id}" communication_mode "${payload.communication_mode}" is not in the closed pool ${COMMUNICATION_MODES.join(', ')}`);
  }

  // acting actor: portable, and one of the currently assigned participants.
  const actingActor = payload.acting_actor;
  if (blank(actingActor)) {
    problems.push(`human-control record "${id}" names no acting_actor`);
  } else {
    const r = nonPortableReason(actingActor);
    if (r) problems.push(`human-control record "${id}" acting_actor contains ${r}`);
    if (!current.actors.has(actingActor)) {
      problems.push(`human-control record "${id}" acting_actor "${actingActor}" is not assigned in this run; any participant in control must be present in role_assignments`);
    }
  }

  // review independence: full semantics for required true and false.
  const ri = isObject(payload.review_independence) ? payload.review_independence : null;
  if (ri) {
    const rev = typeof ri.reviewer_actor === 'string' ? ri.reviewer_actor : null;
    const reviewed = typeof ri.reviewed_actor === 'string' ? ri.reviewed_actor : null;
    // portability of any present reference, whatever the value of required.
    for (const [v, name] of [[ri.reviewer_actor, 'reviewer_actor'], [ri.reviewed_actor, 'reviewed_actor']]) {
      if (typeof v === 'string' && v !== '') {
        const r = nonPortableReason(v);
        if (r) problems.push(`human-control record "${id}" review_independence.${name} contains ${r}`);
      }
    }
    if (ri.required === true) {
      if (!rev || blank(rev) || !reviewed || blank(reviewed)) {
        problems.push(`human-control record "${id}" requires review independence but does not name both a reviewer_actor and the reviewed_actor`);
      } else {
        if (rev === reviewed) {
          problems.push(`human-control record "${id}" requires review independence but names the same actor "${rev}" as reviewer and reviewed; an independent reviewer cannot be the participant whose result is independently reviewed`);
        }
        if (!(current.actorRoles.get(rev) && INDEPENDENT_REVIEW_ROLES.some((r) => current.actorRoles.get(rev).has(r)))) {
          problems.push(`human-control record "${id}" requires review independence but no separate actor is assigned a reviewing role (${INDEPENDENT_REVIEW_ROLES.join(' or ')}); the requirement needs a suitable independent participant`);
        }
        if (!current.actors.has(reviewed)) {
          problems.push(`human-control record "${id}" review_independence.reviewed_actor "${reviewed}" is not assigned in this run`);
        }
      }
    } else if (ri.required === false) {
      if (rev !== null || reviewed !== null) {
        problems.push(`human-control record "${id}" review_independence.required is false but still names a reviewer_actor or reviewed_actor; drop both references, or drop the block, when independence is not required`);
      }
    }
  }

  // the switch history — the ordered record of changes to every mutable axis.
  const history = Array.isArray(payload.switch_history) ? payload.switch_history : null;
  if (!history) {
    if ('switch_history' in payload) problems.push(`human-control record "${id}" switch_history is not an ordered list`);
  } else if (history.length === 0) {
    problems.push(`human-control record "${id}" switch_history is empty; the history is mandatory and its first record states the initial establishment`);
  } else {
    let prevSeq = null;
    history.forEach((sw, i) => {
      const s = isObject(sw) ? sw : {};
      const at = `switch #${i}`;

      if (blank(s.reason)) problems.push(`human-control record "${id}" ${at} states no reason; every change states its basis`);
      if (blank(s.checkpoint_ref)) problems.push(`human-control record "${id}" ${at} names no checkpoint_ref; a change preserves a verifiable checkpoint and never opens a new run`);
      for (const [val, name] of [[s.reason, 'reason'], [s.checkpoint_ref, 'checkpoint_ref'], [s.to_acting_actor, 'to_acting_actor']]) {
        const r = nonPortableReason(val);
        if (r) problems.push(`human-control record "${id}" ${at} ${name} contains ${r}`);
      }
      if (blank(s.to_acting_actor)) problems.push(`human-control record "${id}" ${at} names no to_acting_actor`);
      if (typeof s.to_supervision_mode === 'string' && !SUPERVISION_MODES.includes(s.to_supervision_mode)) {
        problems.push(`human-control record "${id}" ${at} to_supervision_mode "${s.to_supervision_mode}" is not in the closed pool ${SUPERVISION_MODES.join(', ')}`);
      }
      if (typeof s.to_communication_mode === 'string' && !COMMUNICATION_MODES.includes(s.to_communication_mode)) {
        problems.push(`human-control record "${id}" ${at} to_communication_mode "${s.to_communication_mode}" is not in the closed pool ${COMMUNICATION_MODES.join(', ')}`);
      }

      // the assignment set this record transitions TO is itself a valid set,
      // and the actor it moves to is present in it.
      const to = checkAssignmentSet(problems, id, `${at} to_role_assignments`, s.to_role_assignments);
      if (!blank(s.to_acting_actor) && !to.actors.has(s.to_acting_actor)) {
        problems.push(`human-control record "${id}" ${at} moves to acting actor "${s.to_acting_actor}", who is not present in that transition's role assignments`);
      }

      if (!isPositiveInt(s.sequence)) {
        problems.push(`human-control record "${id}" ${at} has no positive integer sequence ordinal; the history is ordered by an explicit sequence, not by object or file order`);
      } else if (prevSeq !== null) {
        if (s.sequence === prevSeq) problems.push(`human-control record "${id}" ${at} repeats sequence ordinal ${s.sequence}; ordinals do not repeat`);
        else if (s.sequence < prevSeq) problems.push(`human-control record "${id}" ${at} sequence ordinal ${s.sequence} is below the previous ${prevSeq}; ordinals do not decrease`);
      }
      if (isPositiveInt(s.sequence)) prevSeq = s.sequence;

      if (i === 0) {
        const priors = [
          ['from_supervision_mode', s.from_supervision_mode],
          ['from_communication_mode', s.from_communication_mode],
          ['from_acting_actor', s.from_acting_actor],
          ['from_role_assignments', s.from_role_assignments],
        ].filter(([, v]) => v != null).map(([k]) => k);
        if (priors.length) {
          problems.push(`human-control record "${id}" ${at} is the first record but names a prior ${priors.join(', ')}; the first record states the initial establishment (every from_* axis is null)`);
        }
      } else {
        const prev = isObject(history[i - 1]) ? history[i - 1] : {};
        if (s.from_supervision_mode !== prev.to_supervision_mode) {
          problems.push(`human-control record "${id}" ${at} from_supervision_mode "${s.from_supervision_mode}" does not continue the previous to_supervision_mode "${prev.to_supervision_mode}"; the switch history has a break on the supervision-mode axis`);
        }
        if (s.from_communication_mode !== prev.to_communication_mode) {
          problems.push(`human-control record "${id}" ${at} from_communication_mode "${s.from_communication_mode}" does not continue the previous to_communication_mode "${prev.to_communication_mode}"; the switch history has a break on the communication-mode axis`);
        }
        if (s.from_acting_actor !== prev.to_acting_actor) {
          problems.push(`human-control record "${id}" ${at} from_acting_actor "${s.from_acting_actor}" does not continue the previous to_acting_actor "${prev.to_acting_actor}"; the switch history has a break on the acting-actor axis`);
        }
        if (!sameAssignmentSet(s.from_role_assignments, prev.to_role_assignments)) {
          problems.push(`human-control record "${id}" ${at} from_role_assignments does not continue the previous to_role_assignments; the switch history has a break on the role-assignment axis`);
        }
      }
    });

    // the last record matches the WHOLE current control state.
    const last = isObject(history[history.length - 1]) ? history[history.length - 1] : {};
    if (last.to_supervision_mode !== supervision) {
      problems.push(`human-control record "${id}" last switch to_supervision_mode "${last.to_supervision_mode}" does not match the current supervision_mode "${supervision}"`);
    }
    if (last.to_communication_mode !== payload.communication_mode) {
      problems.push(`human-control record "${id}" last switch to_communication_mode "${last.to_communication_mode}" does not match the current communication_mode "${payload.communication_mode}"`);
    }
    if (last.to_acting_actor !== actingActor) {
      problems.push(`human-control record "${id}" last switch to_acting_actor "${last.to_acting_actor}" does not match the current acting_actor "${actingActor}"`);
    }
    if (!sameAssignmentSet(last.to_role_assignments, payload.role_assignments)) {
      problems.push(`human-control record "${id}" last switch to_role_assignments does not match the current role_assignments`);
    }
  }

  return problems;
}
