// ---------------------------------------------------------------------------
// controlled rule intake: the one checkable implementation
// ---------------------------------------------------------------------------
// registries/operating-model/controlled-rule-intake.schema.json is the
// specialised payload schema for a rule candidate — text parsed out of a
// registered instruction source (instruction-source-registry.md) and proposed
// as a possible rule. Registry DATA is Instance, like the source registry and
// the intake register: the Kernel ships the schema, the product-neutral
// fixtures and this module. scripts/kernel-validate.mjs (the
// `controlled-rule-intake` section) and test/controlled-rule-intake.test.mjs
// both call the functions here so the gate and the standalone set cannot
// drift apart.
//
// Nothing in this module reads process state, touches the filesystem or
// exits. It takes a parsed document, the two schemas and an external source
// resolver, and returns a flat list of problem strings — empty means valid.
//
// Property 1 — a candidate is untrusted until an explicit owner decision.
// Property 2 — payload.source_ref pins ONE instruction-source-registry entry
//   by id, an explicit revision AND a SHA-256 digest (both mandatory, unlike
//   the flexible revision-OR-digest pin elsewhere in Meridian). The pin is
//   resolved through an EXTERNAL BOUNDARY this module does not control (the
//   caller-supplied `resolveSource`, reusing the makeRecordResolver shape from
//   context-manifest.mjs) and checked against that entry's own recorded_state:
//   an unknown source, an unverified revision, or a revision/digest mismatch
//   against the registry's CURRENT snapshot are all rejected.
// Property 3 — payload.boundary preserves the candidate's exact span inside
//   the source; normalizing raw_excerpt into normalized_text never severs
//   that provenance link.
// Property 4 — scope is an explicit classification from the six logical
//   areas of workspace-scope-model.md, carried with a mandatory
//   classification_basis; it is never derived from the source's physical
//   path.
// Property 5 — semantic_key is an EXPLICITLY asserted content identity used
//   to cluster duplicate candidates found through different origins. Matching
//   ids or array/read order are never treated as proof of identity. All
//   entries sharing a semantic_key must carry the same scope, the same
//   applicability_state/owner_decision and the same conflicts_with set; each
//   entry's own (source_ref, boundary) origin stays distinct within the
//   cluster.
// Property 6 — conflicts_with must be declared SYMMETRICALLY and are
//   evaluated as a graph over the WHOLE candidate set at once — never by
//   folding records in read order. Reported pairs are always named in sorted
//   order so the message text itself does not depend on which side was read
//   first.
// Property 7 — a candidate set fails closed (never a silent third state)
//   when: two symmetrically conflicting clusters are BOTH accepted; a
//   duplicate cluster disagrees with itself on scope, applicability_state or
//   conflicts_with; or a not-applicable reason is not evidenced by the actual
//   data.
// Property 8 — applicability_state is the closed MINIMAL three-value state:
//   'candidate' (undecided, no norm authority, carries no owner_decision at
//   all), 'accepted' and 'not-applicable' (both require owner_decision;
//   not-applicable additionally requires an evidenced not_applicable_reason).
// Property 9 — every entry carries identity+title, scope, origin, the pinned
//   source_ref/boundary, applicability_state and (once decided) owner_decision
//   plus its own semantic_key/conflicts_with. origin.kind is pinned to
//   "derived" and origin.source_ref must name the SAME source as
//   payload.source_ref.id (the canonical mapping is
//   `instruction-source:<payload.source_ref.id>`) — a candidate cannot claim
//   provenance from one source in its envelope while pinning another in its
//   payload.
// Property 10 — this module never reads, changes or reinterprets a source's
//   read_channel/meridian_visibility; accepting a candidate is not a claim of
//   fuller observation of its source. The resolved source projection carries
//   read_channel (kind, meridian_visibility, agent_auto_read) so an
//   agent-native channel is actually checkable here, reusing
//   instruction-source-registry.mjs's own coherence rule rather than a second
//   copy of it.
// Property 11 — raw_excerpt/normalized_text are opaque strings: never parsed
//   as code, a command or an instruction here.
// Property 2/4 (currency) — accepted and not-applicable both require the
//   pinned source's CURRENT recorded_state.currency; "stale" is a verified
//   last-known state and is never silently treated as current, so it may
//   back only a "candidate" entry (kept as a historical finding, never
//   applicable); "unverified" backs no entry at all, decided or not.
// Property 9 (owner authority) — accepted/not-applicable require the SCOPE's
//   own closed human owner authority (HUMAN_OWNER_AUTHORITY_BY_SCOPE);
//   "delegated-run" is a run's authority to carry a candidate's parsing and
//   can never itself decide accepted or not-applicable, even when its
//   decision_ref happens to match payload.owner_decision.decision_ref.
// ---------------------------------------------------------------------------
import { validate } from './json-schema.mjs';
import { nonPortableReason, resolveSchemaRef, classifyRevision, makeRecordResolver } from './context-manifest.mjs';
import { checkReadChannel, READ_CHANNEL_KINDS, MERIDIAN_VISIBILITY, CURRENCY } from './instruction-source-registry.mjs';

// Reused unchanged so this module carries no divergent second copy.
export {
  nonPortableReason, resolveSchemaRef, classifyRevision, makeRecordResolver,
  checkReadChannel, READ_CHANNEL_KINDS, MERIDIAN_VISIBILITY, CURRENCY,
};

export const RECORD_TYPE = 'rule-candidate';
export const SOURCE_RECORD_TYPE = 'instruction-source';
export const BOUNDARY_UNITS = ['agents-md-section', 'line-range', 'byte-range', 'whole-source'];
export const APPLICABILITY_STATES = ['candidate', 'accepted', 'not-applicable'];
export const NOT_APPLICABLE_REASONS = ['owner-rejected', 'lost-conflicting-decision'];

// The origin.kind a rule candidate's envelope must carry: it is always text
// parsed out of a registered source, never built-in, declared, migrated or
// imported (property 9).
export const REQUIRED_ORIGIN_KIND = 'derived';

// The canonical portable form of origin.source_ref for a rule candidate: it
// must name the SAME instruction-source id that payload.source_ref pins.
// This is the one mapping the envelope's origin and the payload's pinned
// reference are checked against — the envelope carries a short portable
// name, the payload carries the full pinned edition (property 2/9).
export function canonicalOriginSourceRef(sourceId) {
  return `instruction-source:${sourceId}`;
}

// The closed, minimal set of human owner authorities that may decide a rule
// candidate's applicability, one per logical scope area
// (workspace-scope-model.md §1). "delegated-run" never appears here: it is a
// run's authority to carry a candidate's parsing (see the base fixture, which
// legitimately pins delegated-run authority to a "candidate" entry), never an
// owner's decision that the candidate is applicable or not. A scope with no
// entry here (run-state) has no defined human owner: a candidate scoped there
// can never leave the candidate state.
export const HUMAN_OWNER_AUTHORITY_BY_SCOPE = {
  'built-in-methodology': 'methodology-owner',
  'user-profile': 'user',
  'organization-profile': 'organization',
  'project-workspace': 'project-owner',
  'repository-scope': 'repository-maintainer',
};

// The closed key set of a resolved instruction-source entry — the same
// "closed transformer response" discipline as context-manifest.mjs: a missing
// AND an unknown key are both precise errors.
const RESOLVED_SOURCE_KEYS = ['record_type', 'id', 'reference', 'recorded_state', 'read_channel'];
const RESOLVED_RECORDED_STATE_KEYS = ['revision', 'digest', 'revision_verified', 'currency'];
const RESOLVED_READ_CHANNEL_KEYS = ['kind', 'meridian_visibility', 'agent_auto_read'];
const DIGEST_KEYS = ['algorithm', 'value'];
export { RESOLVED_READ_CHANNEL_KEYS, DIGEST_KEYS };

const isObject = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);
const blank = (s) => typeof s !== 'string' || s.trim() === '';
const isSha256 = (s) => typeof s === 'string' && /^[0-9a-f]{64}$/.test(s);

// A boundary's declared unit and its region/start/end are consistent. The
// schema already enforces the mutually-exclusive shapes; this adds the one
// numeric comparison the schema subset cannot express.
export function checkBoundary(boundary) {
  if (!isObject(boundary)) return { ok: false, reason: 'boundary is not an object' };
  if (boundary.unit === 'line-range' || boundary.unit === 'byte-range') {
    const { start, end } = boundary;
    if (typeof start === 'number' && typeof end === 'number' && !(start < end)) {
      return { ok: false, reason: `boundary start (${start}) is not less than end (${end}); a range names a non-empty span` };
    }
  }
  return { ok: true };
}

// Resolve payload.source_ref through the external boundary and check it
// against the closed instruction-source-registry snapshot contract. Returns
// the resolved entry, or null when it could not be resolved (a problem is
// pushed). `resolve` is `(sourceRef) => resolvedEntry | null`.
// `applicabilityState` is the candidate's OWN payload.applicability_state —
// passed through only to gate the currency check below (accepted /
// not-applicable need a CURRENT snapshot; a bare "candidate" does not).
export function resolveAndCheckSourceRef(problems, id, sourceRef, resolve, applicabilityState) {
  const at = `rule candidate "${id}"`;
  if (!isObject(sourceRef)) {
    problems.push(`${at} payload.source_ref is not an object`);
    return null;
  }
  const doResolve = typeof resolve === 'function' ? resolve : () => null;
  const entry = doResolve(sourceRef);
  if (!isObject(entry)) {
    problems.push(`${at} source_ref does not resolve to a registered instruction source through the external resolver; an unknown source is never accepted and the candidate fails closed`);
    return null;
  }

  for (const k of Object.keys(entry)) {
    if (!RESOLVED_SOURCE_KEYS.includes(k)) {
      problems.push(`${at} source_ref: the resolver returned a record with an unknown field "${k}"; the transformer response is closed to { ${RESOLVED_SOURCE_KEYS.join(', ')} }`);
    }
  }

  if (entry.record_type !== SOURCE_RECORD_TYPE) {
    problems.push(`${at} source_ref resolves to a "${entry.record_type}" record, not "${SOURCE_RECORD_TYPE}"`);
  }
  if (typeof entry.id !== 'string' || entry.id === '') {
    problems.push(`${at} source_ref: the resolver returned an instruction source with no id`);
  } else if (entry.id !== sourceRef.id) {
    problems.push(`${at} source_ref pins id "${sourceRef.id}" but the reference resolves to record id "${entry.id}"`);
  }
  // reference is REQUIRED (not merely checked when present): a resolved
  // response with no reference, or a reference that does not name the same
  // pinned edition as payload.source_ref.reference, is rejected.
  if (blank(entry.reference)) {
    problems.push(`${at} source_ref: the resolver returned no reference (or a blank one); reference is a required field of the resolved response and must equal payload.source_ref.reference`);
  } else if (typeof sourceRef.reference === 'string' && entry.reference !== sourceRef.reference) {
    problems.push(`${at} source_ref: the resolver's stated reference "${entry.reference}" is not the resolved reference "${sourceRef.reference}"`);
  }

  // Property 10: the resolved projection must carry read_channel so an
  // agent-native channel is actually checkable here — accepting a candidate
  // is not a claim of fuller observation, and this module never rewrites or
  // reinterprets these fields; it only reuses instruction-source-registry's
  // own coherence rule (checkReadChannel) against whatever the resolver
  // states.
  const rc = isObject(entry.read_channel) ? entry.read_channel : null;
  if (!rc) {
    problems.push(`${at} source_ref: the resolved instruction source carries no read_channel; the resolved projection must state kind, meridian_visibility and agent_auto_read so an agent-native channel does not go unchecked (property 10)`);
  } else {
    for (const f of RESOLVED_READ_CHANNEL_KEYS) {
      if (!(f in rc)) problems.push(`${at} source_ref: the resolved instruction source's read_channel is missing required field "${f}"`);
    }
    for (const k of Object.keys(rc)) {
      if (!RESOLVED_READ_CHANNEL_KEYS.includes(k)) {
        problems.push(`${at} source_ref: the resolved instruction source's read_channel carries an unknown field "${k}"; read_channel is closed to its ${RESOLVED_READ_CHANNEL_KEYS.length} declared fields`);
      }
    }
    // Closed SHAPE, not just cross-field coherence: checkReadChannel below
    // only compares against literal known values and never itself catches an
    // invented value or a wrong-typed one — an "invented-channel" kind or a
    // string "false" for agent_auto_read matches none of its comparisons and
    // would otherwise pass silently. These checks run first, independently,
    // and unconditionally; checkReadChannel runs after them regardless, so a
    // value bad in both ways is rejected for both reasons.
    if ('kind' in rc && !READ_CHANNEL_KINDS.includes(rc.kind)) {
      problems.push(`${at} source_ref: the resolved instruction source's read_channel.kind "${rc.kind}" is not one of ${READ_CHANNEL_KINDS.join(', ')}`);
    }
    if ('meridian_visibility' in rc && !MERIDIAN_VISIBILITY.includes(rc.meridian_visibility)) {
      problems.push(`${at} source_ref: the resolved instruction source's read_channel.meridian_visibility "${rc.meridian_visibility}" is not one of ${MERIDIAN_VISIBILITY.join(', ')}`);
    }
    if ('agent_auto_read' in rc && typeof rc.agent_auto_read !== 'boolean') {
      problems.push(`${at} source_ref: the resolved instruction source's read_channel.agent_auto_read is ${JSON.stringify(rc.agent_auto_read)}, not a boolean`);
    }
    const rcProblems = [];
    checkReadChannel(sourceRef.id, rc, rcProblems);
    for (const m of rcProblems) problems.push(`${at} source_ref: resolved ${m}`);
  }

  const rs = isObject(entry.recorded_state) ? entry.recorded_state : null;
  if (!rs) {
    problems.push(`${at} source_ref: the resolved instruction source carries no recorded_state; the pinned edition cannot be checked`);
    return entry;
  }
  for (const f of RESOLVED_RECORDED_STATE_KEYS) {
    if (!(f in rs)) problems.push(`${at} source_ref: the resolved instruction source's recorded_state is missing required field "${f}"`);
  }
  for (const k of Object.keys(rs)) {
    if (!RESOLVED_RECORDED_STATE_KEYS.includes(k)) {
      problems.push(`${at} source_ref: the resolved instruction source's recorded_state carries an unknown field "${k}"; recorded_state is closed to its ${RESOLVED_RECORDED_STATE_KEYS.length} declared fields`);
    }
  }

  // Closed SHAPE of recorded_state, not just the coherence rules below: an
  // invented currency ("invented-currency") or a non-boolean
  // revision_verified matches none of the specific comparisons further down
  // and would otherwise pass silently. Checked first, independently, and
  // unconditionally — the currency×applicability_state rules that follow
  // still run afterward regardless of whether the shape was already bad.
  if ('revision' in rs && blank(rs.revision)) {
    problems.push(`${at} source_ref: the resolved instruction source's recorded_state.revision is not a non-empty string`);
  }
  if ('revision_verified' in rs && typeof rs.revision_verified !== 'boolean') {
    problems.push(`${at} source_ref: the resolved instruction source's recorded_state.revision_verified is ${JSON.stringify(rs.revision_verified)}, not a boolean`);
  }
  if ('currency' in rs && !CURRENCY.includes(rs.currency)) {
    problems.push(`${at} source_ref: the resolved instruction source's recorded_state.currency "${rs.currency}" is not one of ${CURRENCY.join(', ')}`);
  }
  if ('digest' in rs) {
    if (!isObject(rs.digest)) {
      problems.push(`${at} source_ref: the resolved instruction source's recorded_state.digest is not an object`);
    } else {
      for (const f of DIGEST_KEYS) {
        if (!(f in rs.digest)) problems.push(`${at} source_ref: the resolved instruction source's recorded_state.digest is missing required field "${f}"`);
      }
      for (const k of Object.keys(rs.digest)) {
        if (!DIGEST_KEYS.includes(k)) problems.push(`${at} source_ref: the resolved instruction source's recorded_state.digest carries an unknown field "${k}"; digest is closed to algorithm/value`);
      }
      if ('algorithm' in rs.digest && rs.digest.algorithm !== 'sha-256') {
        problems.push(`${at} source_ref: the resolved instruction source's recorded_state.digest.algorithm is not "sha-256"`);
      }
      if ('value' in rs.digest && !isSha256(rs.digest.value)) {
        problems.push(`${at} source_ref: the resolved instruction source's recorded_state.digest.value is not 64 lowercase hex characters`);
      }
    }
  }

  if (rs.revision_verified !== true) {
    problems.push(`${at} source_ref pins a source whose recorded_state.revision_verified is not true; a candidate cannot be accepted from an unverified revision (property 2)`);
  }
  if (typeof rs.revision === 'string' && typeof sourceRef.revision === 'string' && rs.revision !== sourceRef.revision) {
    problems.push(`${at} source_ref pins revision "${sourceRef.revision}" but the instruction source registry now holds revision "${rs.revision}"; the pinned edition has changed (property 2)`);
  }
  const rsDigest = isObject(rs.digest) ? rs.digest.value : undefined;
  if (typeof rsDigest === 'string' && typeof sourceRef.sha256 === 'string' && rsDigest.toLowerCase() !== sourceRef.sha256.toLowerCase()) {
    problems.push(`${at} source_ref pins sha256 "${sourceRef.sha256}" but the instruction source registry now holds digest "${rsDigest}"; the pinned edition has changed (property 2)`);
  }

  // Currency (property 2/4): "unverified" backs no candidate at all, decided
  // or not. "stale" is a verified LAST-KNOWN state, never a silent stand-in
  // for "current" — accepted and not-applicable both require a snapshot that
  // is actually current now; a stale snapshot may back only a "candidate"
  // entry, kept as a historical finding without applicability.
  if (rs.currency === 'unverified') {
    problems.push(`${at} source_ref pins a source whose recorded_state.currency is "unverified"; an unverified snapshot never backs a rule candidate (property 2)`);
  }
  if ((applicabilityState === 'accepted' || applicabilityState === 'not-applicable') && rs.currency === 'stale') {
    problems.push(`${at} applicability_state "${applicabilityState}" relies on source_ref whose recorded_state.currency is "stale"; accepted and not-applicable both require a CURRENT verified snapshot — a stale snapshot is never silently treated as current and may back only a "candidate" entry kept as a historical finding (property 2/4)`);
  }

  return entry;
}

// The envelope's origin must name the SAME source as payload.source_ref
// (property 2/9): origin.kind is pinned to "derived" (a candidate is always
// text parsed out of a registered source, never built-in, declared, migrated
// or imported), and origin.source_ref must equal the canonical mapping of
// payload.source_ref.id. A candidate cannot claim provenance from one source
// in its envelope while pinning another in its payload.
export function checkOriginLink(problems, id, entry) {
  const at = `rule candidate "${id}"`;
  const payload = isObject(entry.payload) ? entry.payload : {};
  const sourceId = isObject(payload.source_ref) ? payload.source_ref.id : undefined;
  if (typeof sourceId !== 'string' || sourceId === '') return; // already flagged elsewhere
  const origin = isObject(entry.origin) ? entry.origin : {};

  if (origin.kind !== REQUIRED_ORIGIN_KIND) {
    problems.push(`${at} origin.kind is "${origin.kind}"; a rule candidate is parsed out of a registered source and its origin.kind must be "${REQUIRED_ORIGIN_KIND}" (property 9)`);
  }
  if (blank(origin.source_ref)) {
    problems.push(`${at} origin carries no source_ref though payload.source_ref pins instruction source "${sourceId}"; origin.source_ref and payload.source_ref must name the same source (property 2/9)`);
    return;
  }
  const expected = canonicalOriginSourceRef(sourceId);
  if (origin.source_ref !== expected) {
    problems.push(`${at} origin.source_ref "${origin.source_ref}" does not name the same source as payload.source_ref (id "${sourceId}"); the canonical mapping is "${expected}" — a candidate cannot claim provenance from one source in its envelope while pinning another in its payload (property 2/9)`);
  }
}

// The scope's closed human owner authority must decide accepted/not-applicable
// (property 9). "delegated-run" — a run's authority to carry a candidate's
// parsing — never itself decides applicability, even when its decision_ref
// happens to match payload.owner_decision.decision_ref: a matching
// decision_ref never substitutes for the correct authority.kind.
export function checkOwnerAuthority(problems, id, entry) {
  const state = isObject(entry.payload) ? entry.payload.applicability_state : undefined;
  if (state !== 'accepted' && state !== 'not-applicable') return;
  const at = `rule candidate "${id}"`;
  const authority = isObject(entry.authority) ? entry.authority : {};
  const scopeType = isObject(entry.scope) ? entry.scope.type : undefined;

  if (authority.kind === 'delegated-run') {
    problems.push(`${at} applicability_state "${state}" is authorized by "delegated-run"; a run's delegated authority may carry the candidate's parsing, but only the scope's human owner authority may decide accepted or not-applicable — a matching decision_ref does not substitute for the correct authority.kind (property 9)`);
    return;
  }
  const required = HUMAN_OWNER_AUTHORITY_BY_SCOPE[scopeType];
  if (!required) {
    problems.push(`${at} applicability_state "${state}" is scoped to "${scopeType}", which has no defined human owner authority for an applicability decision; a candidate there cannot leave the candidate state (property 9)`);
    return;
  }
  if (authority.kind !== required) {
    problems.push(`${at} applicability_state "${state}" is authorized by "${authority.kind}", but scope "${scopeType}" requires owner authority "${required}"; a matching decision_ref does not substitute for the correct authority.kind (property 9)`);
  }
}

// One semantic_key cluster: every member must agree on scope, on
// applicability_state/owner_decision, and on the conflicts_with SET (property
// 5). Each member's own (source_ref.reference, boundary) origin stays
// distinct within the cluster.
function checkCluster(problems, key, members) {
  const scopeKey = (s) => (isObject(s) ? `${s.type ?? ''}::${s.id ?? ''}` : '');
  const decisionKey = (od) => (isObject(od) ? `${od.decision_ref ?? ''}::${od.decided_at ?? ''}::${od.reason ?? ''}` : '');
  const conflictSetKey = (cw) => JSON.stringify([...new Set(Array.isArray(cw) ? cw : [])].sort());

  const scopes = new Set(members.map((m) => scopeKey(m.scope)));
  if (scopes.size > 1) {
    problems.push(`semantic cluster "${key}" classifies into more than one scope across its origins (${members.map((m) => m.id).join(', ')}); a duplicate cluster shares one classification`);
  }
  const states = new Set(members.map((m) => m.payload.applicability_state));
  if (states.size > 1) {
    problems.push(`semantic cluster "${key}" carries more than one applicability_state across its origins (${members.map((m) => m.id).join(', ')}); a duplicate cluster is decided once, not per origin`);
  }
  const decisions = new Set(members.map((m) => decisionKey(m.payload.owner_decision)));
  if (decisions.size > 1) {
    problems.push(`semantic cluster "${key}" carries inconsistent owner_decision values across its origins (${members.map((m) => m.id).join(', ')})`);
  }
  const conflictSets = new Set(members.map((m) => conflictSetKey(m.payload.conflicts_with)));
  if (conflictSets.size > 1) {
    problems.push(`semantic cluster "${key}" carries inconsistent conflicts_with declarations across its origins (${members.map((m) => m.id).join(', ')})`);
  }

  // A matching owner_decision is not itself proof a single owner decided:
  // members actually DECIDED (accepted or not-applicable) must share ONE full
  // authority — kind, authority_ref AND decision_ref — not just an equal
  // owner_decision. This does NOT reach members still carrying "candidate":
  // different origins of the same semantic_key legitimately keep distinct
  // delegated-run authorities while undecided.
  const authorityKey = (a) => (isObject(a) ? `${a.kind ?? ''}::${a.authority_ref ?? ''}::${a.decision_ref ?? ''}` : '');
  const decidedMembers = members.filter((m) => m.payload.applicability_state === 'accepted' || m.payload.applicability_state === 'not-applicable');
  const authorities = new Set(decidedMembers.map((m) => authorityKey(m.authority)));
  if (authorities.size > 1) {
    problems.push(`semantic cluster "${key}" carries inconsistent authority across its decided origins (${decidedMembers.map((m) => m.id).join(', ')}); a cluster decided accepted or not-applicable is decided by ONE authority — kind, authority_ref and decision_ref must all agree, and a matching owner_decision does not substitute for that`);
  }

  const seenOrigin = new Set();
  for (const m of members) {
    const originKey = `${(m.payload.source_ref && m.payload.source_ref.reference) ?? ''}::${JSON.stringify(m.payload.boundary ?? null)}`;
    if (seenOrigin.has(originKey)) {
      problems.push(`semantic cluster "${key}" lists the same origin (source reference and boundary) more than once; each origin is preserved once, not repeated`);
    }
    seenOrigin.add(originKey);
  }
}

// The conflict graph over the WHOLE candidate set, computed once — never by
// folding records in the order they were read (property 6). Pairs are always
// reported in sorted order so the message text is order-independent too.
function checkConflictGraph(problems, clusters) {
  const keys = [...clusters.keys()];
  const stateOf = (k) => clusters.get(k)[0].payload.applicability_state;
  const conflictsOf = (k) => new Set(Array.isArray(clusters.get(k)[0].payload.conflicts_with) ? clusters.get(k)[0].payload.conflicts_with : []);

  for (const k of keys) {
    for (const other of conflictsOf(k)) {
      if (!clusters.has(other)) {
        problems.push(`semantic cluster "${k}" declares conflicts_with "${other}", which is not the semantic_key of any candidate in this document`);
        continue;
      }
      if (other === k) {
        problems.push(`semantic cluster "${k}" declares itself in its own conflicts_with`);
        continue;
      }
      if (!conflictsOf(other).has(k)) {
        problems.push(`conflict between "${k}" and "${other}" is declared only one-sided; conflicts_with is declared symmetrically so evaluation does not depend on which record is read first`);
      }
    }
  }

  const reportedPairs = new Set();
  for (const k of keys) {
    for (const other of conflictsOf(k)) {
      if (!clusters.has(other) || !conflictsOf(other).has(k)) continue;
      const pair = [k, other].sort();
      const pairKey = pair.join('::');
      if (reportedPairs.has(pairKey)) continue;
      if (stateOf(pair[0]) === 'accepted' && stateOf(pair[1]) === 'accepted') {
        reportedPairs.add(pairKey);
        problems.push(`semantic clusters "${pair[0]}" and "${pair[1]}" are declared as conflicting but both are accepted; an unresolved conflict must not be applied as a norm on either side (property 7)`);
      }
    }
  }

  for (const k of keys) {
    const member = clusters.get(k)[0];
    if (member.payload.not_applicable_reason === 'lost-conflicting-decision') {
      const hasAcceptedOpponent = [...conflictsOf(k)].some((other) => clusters.has(other) && stateOf(other) === 'accepted');
      if (!hasAcceptedOpponent) {
        problems.push(`semantic cluster "${k}" declares not_applicable_reason "lost-conflicting-decision" but none of its declared conflicts_with clusters is accepted; the reason is not evidenced`);
      }
    }
  }
}

// The whole composition pipeline for one controlled-rule-intake document: the
// container/payload schema, per-entry envelope schema, resolution of every
// source_ref through the external boundary, and the cross-cutting rules the
// JSON Schema subset cannot state (clusters, conflicts, decision coherence).
export function evaluateControlledRuleIntake(doc, { registrySchema, envelopeSchema, resolveSource } = {}) {
  const problems = [];

  const containerErrs = [];
  try { validate(doc, registrySchema, registrySchema, '', containerErrs); }
  catch (e) { return [`container/payload schema could not be applied: ${e.message}`]; }
  problems.push(...containerErrs);

  const entries = Array.isArray(doc && doc.rule_candidates) ? doc.rule_candidates : [];

  for (let i = 0; i < entries.length; i += 1) {
    const envErrs = [];
    try { validate(entries[i], envelopeSchema, envelopeSchema, '', envErrs); }
    catch (e) { problems.push(`entry ${i} envelope could not be applied: ${e.message}`); continue; }
    problems.push(...envErrs.map((m) => `entry ${i} envelope ${m}`));
  }

  const seenIds = new Set();
  const bySemanticKey = new Map();
  for (const e of entries) {
    if (!isObject(e)) continue;
    const id = typeof e.id === 'string' ? e.id : `#${entries.indexOf(e)}`;
    if (typeof e.id === 'string') {
      if (seenIds.has(e.id)) problems.push(`rule candidate id "${e.id}" is declared more than once`);
      seenIds.add(e.id);
    }
    if (e.record_type !== RECORD_TYPE) {
      problems.push(`rule candidate "${id}" declares record_type "${e.record_type}", not "${RECORD_TYPE}"`);
    }
    const payload = isObject(e.payload) ? e.payload : {};

    const boundaryVerdict = checkBoundary(payload.boundary);
    if (!boundaryVerdict.ok) problems.push(`rule candidate "${id}" boundary: ${boundaryVerdict.reason}`);

    resolveAndCheckSourceRef(problems, id, payload.source_ref, resolveSource, payload.applicability_state);
    checkOriginLink(problems, id, e);
    checkOwnerAuthority(problems, id, e);

    const authority = isObject(e.authority) ? e.authority : {};
    const decision = isObject(payload.owner_decision) ? payload.owner_decision : null;
    if (decision) {
      if (authority.decision_ref !== decision.decision_ref) {
        problems.push(`rule candidate "${id}" authority.decision_ref "${authority.decision_ref}" does not match payload.owner_decision.decision_ref "${decision.decision_ref}"; both name the same decision`);
      }
    }

    if (typeof payload.semantic_key === 'string' && payload.semantic_key !== '') {
      const cluster = bySemanticKey.get(payload.semantic_key) || [];
      cluster.push({ id, scope: e.scope, authority: e.authority, payload });
      bySemanticKey.set(payload.semantic_key, cluster);
    }
  }

  for (const [key, members] of bySemanticKey) {
    checkCluster(problems, key, members);
  }
  checkConflictGraph(problems, bySemanticKey);

  return problems;
}
