// ---------------------------------------------------------------------------
// instruction-source registry: the one checkable implementation
// ---------------------------------------------------------------------------
// registries/operating-model/instruction-source-registry.schema.json is the
// specialised payload schema for a registered instruction source. Registry
// DATA is Instance, like the intake register — the Kernel ships the schema,
// the product-neutral fixtures and this module. scripts/kernel-validate.mjs
// (the `instruction-source-registry` section) and
// test/instruction-source-registry.test.mjs both call the functions here so the
// gate and the standalone set cannot drift apart.
//
// Nothing in this module reads process state, touches the filesystem or exits.
// It takes a parsed document plus the two schemas and returns a flat list of
// problem strings — empty means valid. Location confinement is a pure string
// check: a registered source may be external or simply absent from this
// checkout, so its existence is never required (product-neutral, and with no
// dependency on a mandatory separate Instance).
//
// read_channel describes only who observes the SOURCE and how — the agent's own
// tool ingests the file (agent-native), Meridian's registry reads it to
// snapshot it (meridian-observed), or a person reports it (manual). It is not a
// norm-delivery channel: registering or reading a source grants its text no
// authority and does not replace controlled-rule-intake.
//
// Temporal model (standards/workspace/instruction-source-registry.md §2.6–§2.7):
//   - recorded_state       — the snapshot the registry currently holds.
//   - divergence.previous_state — the snapshot held before the last check
//                            (historical; may differ from recorded_state,
//                            EXCEPT on source-missing / source-unreadable,
//                            where the last-known recorded_state == previous_state).
//   - divergence.current_state  — the fresh observation that check produced.
//                            It and recorded_state describe one present
//                            observation and must agree on revision, digest AND
//                            verification state.
// Two verified states are `unchanged` only when BOTH revision and SHA-256
// digest match; a difference in either is `changed`. `unknown` is only for
// genuinely insufficient evidence. `source-missing` / `source-unreadable`
// carry no current_state. currency binds to revision_verified and to
// divergence.status: `stale` is a verified last-known state, never a stand-in
// for an unverified one.
// ---------------------------------------------------------------------------
import { validate } from './json-schema.mjs';

// The closed pools the schema also carries. Re-stated here only so the
// standalone set can assert the two halves have not drifted.
export const MEDIA = ['file', 'external-service'];
export const FORMATS = ['agents-md', 'claude-md', 'cursor-rule', 'kernel-doc', 'markdown-section', 'plain-text'];
export const READ_CHANNEL_KINDS = ['meridian-observed', 'agent-native', 'manual'];
export const MERIDIAN_VISIBILITY = ['full', 'partial', 'none'];
export const CURRENCY = ['current', 'stale', 'unverified'];
export const DIVERGENCE_STATES = ['unknown', 'unchanged', 'changed', 'source-missing', 'source-unreadable'];

// The divergence statuses that are positive evidence the source can no longer
// be observed as the held snapshot. `changed` is deliberately NOT here: on a
// `changed` result the registry records the fresh observation, so
// recorded_state == the new current_state and is itself current.
const SOURCE_GONE = new Set(['source-missing', 'source-unreadable']);

const isObject = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);

// A location path is confined to its declared medium: relative, already
// normalised, no absolute form and no escape through "..". The source file
// itself is never required to exist. Returns { ok: true } or
// { ok: false, reason }.
export function checkLocationPath(p) {
  if (typeof p !== 'string' || p === '') {
    return { ok: false, reason: 'the location path is empty' };
  }
  if (p.includes('\\')) {
    return { ok: false, reason: 'the location path uses a backslash as a separator; a source path is POSIX-relative' };
  }
  if (/^[a-zA-Z]:/.test(p) || p.startsWith('/')) {
    return { ok: false, reason: 'the location path is absolute; a registered source is located by a relative path inside its declared medium' };
  }
  const segments = p.split('/');
  if (segments.some((s) => s === '' || s === '.' || s === '..')) {
    return { ok: false, reason: 'the location path carries an empty, "." or ".." segment; it must already be normalised and must not escape the declared medium' };
  }
  return { ok: true };
}

// An opaque identifier (container_ref, service_ref, resource_ref) must not be a
// disguised filesystem location. Returns a problem string or null.
function opaqueRefProblem(ref, label) {
  if (typeof ref !== 'string' || ref === '') return null;
  if (ref.includes('\\') || /^[a-zA-Z]:/.test(ref) || ref.startsWith('/')) {
    return `${label} "${ref}" looks like an absolute machine path; it must be an opaque identifier, not a filesystem location`;
  }
  if (ref.split('/').some((s) => s === '..')) {
    return `${label} "${ref}" carries a ".." segment; it must be an opaque identifier`;
  }
  return null;
}

// Is `s` a complete, verified observation the divergence rules can compare?
function isVerifiedSnapshot(s) {
  return isObject(s)
    && s.verified === true
    && typeof s.revision === 'string' && s.revision !== ''
    && isObject(s.digest)
    && s.digest.algorithm === 'sha-256'
    && typeof s.digest.value === 'string';
}

// The canonical divergence status computed from two states. Two VERIFIED states
// are "unchanged" only when both the revision AND the SHA-256 digest match; a
// difference in either is "changed". Anything short of two verified states is
// "unknown" — the comparison cannot be made.
export function computeDivergence(previous, current) {
  if (!isVerifiedSnapshot(previous) || !isVerifiedSnapshot(current)) return 'unknown';
  const sameRevision = previous.revision === current.revision;
  const sameDigest = previous.digest.value === current.digest.value;
  return sameRevision && sameDigest ? 'unchanged' : 'changed';
}

// A declared divergence.status must be consistent with its evidence. Returns a
// flat list of problem strings.
export function checkDivergenceClaim(divergence) {
  const problems = [];
  const d = isObject(divergence) ? divergence : {};
  const status = d.status;
  const prev = d.previous_state;
  const cur = d.current_state;
  const bothVerified = isVerifiedSnapshot(prev) && isVerifiedSnapshot(cur);

  if (status === 'unchanged' || status === 'changed') {
    if (!bothVerified) {
      problems.push(`divergence status "${status}" needs a verifiable previous and current state (each with a revision, a sha-256 digest and verified: true); a changed, missing, unreadable or unverified source is never treated as matching`);
    } else {
      const sameRevision = prev.revision === cur.revision;
      const sameDigest = prev.digest.value === cur.digest.value;
      const computed = sameRevision && sameDigest ? 'unchanged' : 'changed';
      if (status !== computed) {
        let detail;
        if (sameRevision && !sameDigest) detail = 'the SHA-256 digest differs while the revision is unchanged';
        else if (!sameRevision && sameDigest) detail = 'the revision differs while the SHA-256 digest is unchanged';
        else if (!sameRevision && !sameDigest) detail = 'both the revision and the SHA-256 digest differ';
        else detail = 'the revision and the SHA-256 digest both match';
        problems.push(`divergence status "${status}" contradicts the two verified states: ${detail}, which computes "${computed}"`);
      }
    }
  } else if (status === 'unknown') {
    if (bothVerified) {
      problems.push('divergence status "unknown" is declared with two complete verified states; when both states are present and verified the divergence is computable and the declared status must equal the computed one');
    }
  } else if (SOURCE_GONE.has(status)) {
    if (isObject(cur)) {
      problems.push(`divergence status "${status}" carries a current_state; a source that is missing or unreadable has no current state to observe`);
    }
  }
  return problems;
}

function checkReadChannel(id, rc, problems) {
  const at = `instruction source "${id}"`;
  const kind = rc.kind;
  const visibility = rc.meridian_visibility;
  const auto = rc.agent_auto_read;
  if (auto === true && kind !== 'agent-native') {
    problems.push(`${at}: read_channel agent_auto_read is true but kind is "${kind}"; a source the agent's own tooling ingests by itself is the agent-native channel`);
  }
  if (auto === true && visibility === 'full') {
    problems.push(`${at}: read_channel agent_auto_read is true with meridian_visibility "full"; Meridian cannot fully observe a source the agent's own tool ingests`);
  }
  if (kind === 'agent-native' && auto !== true) {
    problems.push(`${at}: read_channel kind is "agent-native" but agent_auto_read is not true; the agent-native channel is defined by the agent ingesting the source itself`);
  }
  if (kind === 'meridian-observed' && auto === true) {
    problems.push(`${at}: read_channel kind is "meridian-observed" but agent_auto_read is true; the meridian-observed channel is Meridian's own read of the source, not the agent's`);
  }
  if (visibility === 'full' && kind !== 'meridian-observed') {
    problems.push(`${at}: read_channel meridian_visibility is "full" but kind is "${kind}"; full visibility of the source is possible only on the meridian-observed channel`);
  }
}

// recorded_state and divergence together describe one present observation.
// This is where the temporal model is enforced.
function checkStateCoherence(id, payload, problems) {
  const at = `instruction source "${id}"`;
  const rs = isObject(payload.recorded_state) ? payload.recorded_state : {};
  const d = isObject(payload.divergence) ? payload.divergence : {};
  const status = d.status;
  const verified = rs.revision_verified === true;

  // currency <-> revision_verified
  if (verified && rs.currency === 'unverified') {
    problems.push(`${at}: recorded_state.revision_verified is true but currency is "unverified"; a verified snapshot is "current" or "stale", never "unverified"`);
  }
  if (!verified && rs.currency !== 'unverified') {
    problems.push(`${at}: recorded_state.revision_verified is not true, so currency must be "unverified"`);
  }

  // currency "current": verified, and the source is not known to be gone
  if (rs.currency === 'current') {
    if (!verified) {
      problems.push(`${at}: recorded_state.currency is "current" but revision_verified is not true; an unverified revision is not declared current`);
    }
    if (SOURCE_GONE.has(status)) {
      problems.push(`${at}: recorded_state.currency is "current" but divergence.status is "${status}"; a source that is missing or unreadable cannot have a current snapshot — record the verified last-known state as "stale"`);
    }
  }

  // currency "stale": a verified last-known state, needs positive evidence the
  // source has since gone. It is never a stand-in for an unverified snapshot.
  if (rs.currency === 'stale') {
    if (!verified) {
      problems.push(`${at}: recorded_state.currency is "stale" but revision_verified is not true; "stale" is a verified last-known state — an unverified snapshot is "unverified"`);
    }
    if (!SOURCE_GONE.has(status)) {
      problems.push(`${at}: recorded_state.currency is "stale" but divergence.status is "${status}"; "stale" needs positive evidence (source-missing or source-unreadable) that the source no longer matches the held snapshot`);
    }
  }

  // recorded_state and divergence.current_state describe the same present
  // observation: they must not carry contradicting revisions, digests OR
  // verification state.
  const cur = d.current_state;
  if (isObject(cur)) {
    if (typeof cur.revision === 'string' && typeof rs.revision === 'string' && cur.revision !== rs.revision) {
      problems.push(`${at}: divergence.current_state.revision "${cur.revision}" contradicts recorded_state.revision "${rs.revision}"; both describe the source as it is now and must agree`);
    }
    const curVal = isObject(cur.digest) ? cur.digest.value : undefined;
    const rsVal = isObject(rs.digest) ? rs.digest.value : undefined;
    if (typeof curVal === 'string' && typeof rsVal === 'string' && curVal !== rsVal) {
      problems.push(`${at}: divergence.current_state digest contradicts recorded_state digest; both describe the source as it is now and must agree`);
    }
    if (typeof cur.verified === 'boolean' && typeof rs.revision_verified === 'boolean'
        && cur.verified !== rs.revision_verified) {
      problems.push(`${at}: divergence.current_state.verified is ${cur.verified} but recorded_state.revision_verified is ${rs.revision_verified}; one present observation cannot be both verified and not verified`);
    }
  }

  // source-missing / source-unreadable keep the last known state: when a
  // previous_state is carried, recorded_state (the last-known state) must equal
  // it on revision and digest.
  const prev = d.previous_state;
  if (SOURCE_GONE.has(status) && isObject(prev)) {
    if (typeof prev.revision === 'string' && typeof rs.revision === 'string' && prev.revision !== rs.revision) {
      problems.push(`${at}: divergence.status is "${status}" with a previous_state whose revision "${prev.revision}" contradicts recorded_state.revision "${rs.revision}"; the last-known state must match the snapshot the registry holds`);
    }
    const prevVal = isObject(prev.digest) ? prev.digest.value : undefined;
    const rsVal2 = isObject(rs.digest) ? rs.digest.value : undefined;
    if (typeof prevVal === 'string' && typeof rsVal2 === 'string' && prevVal !== rsVal2) {
      problems.push(`${at}: divergence.status is "${status}" with a previous_state whose digest contradicts recorded_state digest; the last-known state must match the snapshot the registry holds`);
    }
  }
}

// The whole composition pipeline for one registry document: container + payload
// schema, per-entry envelope schema, then the cross-cutting rules the JSON
// Schema subset cannot state.
export function evaluateInstructionSourceRegistry(doc, { registrySchema, envelopeSchema }) {
  const problems = [];

  const containerErrs = [];
  try { validate(doc, registrySchema, registrySchema, '', containerErrs); }
  catch (e) { return [`container/payload schema could not be applied: ${e.message}`]; }
  problems.push(...containerErrs);

  const entries = Array.isArray(doc && doc.instruction_sources) ? doc.instruction_sources : [];

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
    if (typeof e.id === 'string') {
      if (seenIds.has(e.id)) problems.push(`instruction source id "${e.id}" is declared more than once`);
      seenIds.add(e.id);
    }
    if (e.record_type !== 'instruction-source') {
      problems.push(`instruction source "${id}" declares record_type "${e.record_type}", not "instruction-source"; registering a source never turns it into a norm`);
    }
    const payload = isObject(e.payload) ? e.payload : {};

    if (payload.normative_status !== 'not-a-norm') {
      problems.push(`instruction source "${id}" payload.normative_status is "${payload.normative_status}", not "not-a-norm"; registration grants the source's text no norm authority, accepts no rule and resolves no conflict`);
    }

    const location = isObject(payload.location) ? payload.location : {};
    if (payload.medium === 'file') {
      const verdict = checkLocationPath(location.path);
      if (!verdict.ok) {
        problems.push(`instruction source "${id}" location.path "${location.path}": ${verdict.reason}`);
      }
    }
    for (const [field, label] of [
      ['container_ref', 'location.container_ref'],
      ['service_ref', 'location.service_ref'],
      ['resource_ref', 'location.resource_ref'],
    ]) {
      const refProblem = opaqueRefProblem(location[field], `instruction source "${id}" ${label}`);
      if (refProblem) problems.push(refProblem);
    }
    if ('missing_behavior' in location && location.missing_behavior !== 'fail-closed') {
      problems.push(`instruction source "${id}" location.missing_behavior is "${location.missing_behavior}"; the only declared behaviour for an unresolvable source is "fail-closed" — no hidden fallback resolution`);
    }

    checkStateCoherence(id, payload, problems);
    if (isObject(payload.read_channel)) checkReadChannel(id, payload.read_channel, problems);

    for (const p of checkDivergenceClaim(payload.divergence)) {
      problems.push(`instruction source "${id}" ${p}`);
    }
  }

  return problems;
}
