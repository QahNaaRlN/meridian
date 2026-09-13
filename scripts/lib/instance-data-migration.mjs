// ---------------------------------------------------------------------------
// instance-data-migration: the one checkable implementation
// ---------------------------------------------------------------------------
// registries/operating-model/instance-data-migration.schema.json is the
// specialised payload schema for a migration plan (record_type:
// instance-migration-plan) — the product-neutral, storage-neutral contract
// that describes, verifies and reproducibly applies the migration of a
// transitional Instance's records into Meridian's logical scope areas
// (workspace-scope-model.md §1) without choosing a persistent store. Plan
// DATA is Instance, like every other operating-model register the Kernel
// hosts a contract for: the Kernel ships the schema, the product-neutral
// fixtures and this module. scripts/kernel-validate.mjs (the
// `instance-data-migration` section) and test/instance-data-migration.test.mjs
// both call the functions here so the gate and the standalone set cannot
// drift apart.
//
// Nothing in this module writes, reads the filesystem or touches process
// state. evaluateInstanceDataMigration takes a parsed document, the registry
// and envelope schemas, and two EXTERNAL resolution boundaries this module
// does not control (resolveSourceSnapshot, resolveEvidence — the same
// "closed transformer response" discipline as controlled-rule-intake.mjs's
// resolveSource) and returns a flat list of problem strings — empty means
// valid. computePlanFingerprint and computeIdempotencyKey are pure functions
// of a plan's own declared content, exported so both this module and a
// caller building a plan can derive — and a rerun can re-derive — the SAME
// values from the SAME pinned input (repeatability, property 7).
//
// Nine required contract properties, and where each is actually enforced:
//
//  1. Storage neutrality  — the schema carries no field that selects a
//     persistent store; a physical path is never accepted as an identity
//     (checkOpaqueRef, below, rejects any ref shaped like a filesystem path,
//     a Windows path or a file:// URL wherever a portable ref is declared).
//  2. Precise source      — payload.source pins an explicit revision, a
//     SHA-256 digest and a repository_ref, and declares source.qualification;
//     "reproducible" requires working_tree_clean: true (schema-level
//     if/then) AND a resolved source snapshot — through the external
//     resolveSourceSnapshot boundary — whose own repository_ref, revision,
//     FULLY CLOSED digest (no extra or missing field) and working_tree_clean
//     actually match the declared ones (resolveAndCheckSourceSnapshot): a
//     claimed repository_ref/revision/digest with no matching resolved
//     snapshot never confirms reproducibility, and a snapshot resolved for a
//     DIFFERENT repository never confirms THIS one's. A not-reproducible
//     source forces the whole plan BLOCKED (checkVerification).
//  3. Complete correspondence — every declared record_units[] id carries
//     EXACTLY ONE mapping (checkCoverage): omitted, duplicated or dangling
//     mapping references are all rejected; record_units and mappings each
//     require at least one entry (schema minItems) — an empty migration plan
//     is never valid, though an empty registry container still is. A
//     many-to-one "merged" disposition requires an explicit shared
//     merge_rule_ref across at least two units, and a target.id claimed by
//     more than one "migrated" mapping (an undeclared, implicit merge) is
//     rejected (checkTargetGroups).
//  4. Semantics preservation — a migrated/merged target's field_basis is
//     closed to id/record_type/scope/origin/authority and explicit
//     (preserved vs assigned) for every one of them (schema); target.origin
//     is now a mandatory field in the canonical envelope shape
//     (kind pinned "migrated", source_ref required) and is checked
//     (checkTargetGroups) to trace to the ACTUAL contributing record
//     unit(s) — never freely asserted; field_basis.origin is checked to
//     always be "assigned" (a pre-Meridian record never carried this
//     canonical origin shape, so it can never be "preserved"). Every mapping
//     sharing one target must describe a STRUCTURALLY IDENTICAL target
//     record (checkTargetGroups) — a merge cannot describe "the same"
//     result two different ways. Target authority is checked against the
//     closed owner-authority table for its OWN scope (checkTargetAuthority),
//     and delegated-run — a run's own authority, never a human owner's — is
//     rejected as a target's permanent authority even where the table is
//     silent (run-state has none).
//  5. Reversibility        — rollback.source_snapshot_ref and a concrete
//     rollback.plan are schema-mandatory on every plan; rollback.plan is
//     CLOSED to two MUTUALLY EXCLUSIVE variants — deterministic-reconstruction
//     requires deterministic_plan_ref and FORBIDS restoration_evidence_ref;
//     verified-restoration requires restoration_evidence_ref and FORBIDS
//     deterministic_plan_ref (schema allOf/not) — an inactive ref for the
//     other variant is a schema violation, never a silently-ignored extra
//     field. rewrites_published_history is pinned to the literal false
//     (schema const) — rollback never means rewriting published history.
//     Reversibility is actually CHECKED, not merely schema-shaped:
//     rollback.source_snapshot_ref is resolved through the external
//     resolveRollbackSnapshot boundary and the resolved record is checked to
//     name the SAME repository_ref/revision/digest this plan's own
//     payload.source pins (resolveAndCheckRollbackSnapshot) — a ref that
//     resolves to a snapshot of a different source never confirms
//     reversibility of this one. A "deterministic-reconstruction" plan's
//     deterministic_plan_ref is resolved through resolveDeterministicPlan and
//     checked to name this plan's own rollback.source_snapshot_ref, this
//     plan's own id (plan_ref) and its own RECOMPUTED plan_fingerprint, and
//     to be marked applicable (resolveAndCheckDeterministicPlan). A
//     "verified-restoration" plan's restoration_evidence_ref is resolved
//     through resolveRestorationEvidence and checked to name THIS plan's own
//     id, its own RECOMPUTED plan_fingerprint and rollback.source_snapshot_ref
//     and to confirm (resolveAndCheckRestorationEvidence) — a bare non-empty
//     ref string (invented, cross-plan, cross-snapshot or pinned to a STALE
//     version of this plan's own content) never backs a rollback claim.
//  6. Verifiable correspondence — verification.overall_status "VERIFIED" is
//     checked (checkVerification), never merely asserted, against the actual
//     coverage/applicability_preservation sub-verdicts and source.qualification.
//     A sub-verdict's own "verified" status is itself checked, independent of
//     overall_status: its evidence_ref is resolved through the external
//     resolveEvidence boundary and the resolved record is checked to
//     actually confirm THIS plan's id AND its own RECOMPUTED plan_fingerprint
//     — not merely its id — AND THIS kind of result (resolveAndCheckEvidence)
//     — a non-empty evidence_ref string alone, or evidence pinned to a STALE
//     fingerprint of a plan whose mapping/target/owner_decision has since
//     changed, never backs a verified claim. A not-reproducible source
//     forces "BLOCKED", never a softer state.
//  7. Repeatability        — computePlanFingerprint recomputes a plan's
//     fingerprint as a documented canonical projection (source, record_units,
//     mappings, rollback — a sorted-key JSON canonicalisation of each,
//     insensitive only to the array ORDER of record_units/mappings) and the
//     declared plan_fingerprint must match; this SAME recomputed fingerprint
//     is what every evidence/rollback resolver check above pins its proof to
//     (properties 5/6), so a materially changed plan invalidates evidence
//     minted for its earlier content even when the id is unchanged.
//     computeIdempotencyKey derives idempotency_key from the plan's own scope
//     AND the canonical identity of its pinned source — repository_ref,
//     revision AND the full digest, not revision alone — so two DIFFERENT
//     source repositories that happen to share a revision TEXT never collide
//     on the same key. The SAME pinned input always computes the SAME key, so
//     two independently authored plans over one pinned input collide under
//     container-wide uniqueness (checkIdempotency) rather than passing as
//     unrelated records under arbitrary keys. checkSupersedes requires a
//     superseding plan to share its predecessor's FULL scope, the SAME source
//     repository_ref, and pin a DIFFERENT source revision; when the
//     predecessor is not present in the same document, its identity is
//     resolved through the external resolveSupersededPlan boundary and
//     checked closed — an unknown or unresolved predecessor id is NEVER
//     accepted automatically.
//  8. Closed form           — additionalProperties: false at every schema
//     level; checkOpaqueRef rejects a root POSIX path, a Windows path, a
//     file:// URL or a personal machine path in every portable ref field;
//     target_scope carries the SAME structural constraints as
//     workspace-scope-model.md's own scope envelope (built-in-methodology
//     fixes id, repository-scope/run-state require workspace_id, only
//     project-workspace may carry organization_profile_id) — never a
//     diverging simplified copy.
//  9. Validator integration — scripts/kernel-validate.mjs requires this
//     schema and its fixtures unconditionally (a FAIL, not a skip, if
//     either is absent) and calls evaluateInstanceDataMigration with
//     resolvers built from the fixtures' own resolution maps, exactly as
//     test/instance-data-migration.test.mjs does.
// ---------------------------------------------------------------------------
import { createHash } from 'node:crypto';
import { validate } from './json-schema.mjs';

export const RECORD_TYPE = 'instance-migration-plan';
export const ALLOWED_SCOPE_TYPES = ['project-workspace', 'repository-scope'];
export const REQUIRED_ORIGIN_KIND = 'declared';
export const REQUIRED_AUTHORITY_KIND = 'delegated-run';
export const DISPOSITIONS = ['migrated', 'retained-transitional', 'merged'];
export const QUALIFICATIONS = ['reproducible', 'not-reproducible'];
export const VERIFICATION_STATES = ['verified', 'unverified'];
export const OVERALL_STATES = ['VERIFIED', 'UNVERIFIED', 'BLOCKED'];
export const ROLLBACK_PLANS = ['deterministic-reconstruction', 'verified-restoration'];
export const FIELD_BASES = ['preserved', 'assigned'];
export const TARGET_ORIGIN_KIND = 'migrated';
export const SOURCE_SNAPSHOT_RECORD_TYPE = 'instance-source-snapshot';
export const EVIDENCE_RECORD_TYPE = 'instance-migration-evidence';
export const EVIDENCE_KINDS = ['coverage', 'applicability_preservation'];
export const ROLLBACK_SNAPSHOT_RECORD_TYPE = 'instance-rollback-snapshot';
export const DETERMINISTIC_PLAN_RECORD_TYPE = 'instance-deterministic-reconstruction-plan';
export const RESTORATION_EVIDENCE_RECORD_TYPE = 'instance-restoration-evidence';
export const SUPERSEDED_PLAN_RECORD_TYPE = 'instance-superseded-plan';

// The closed, minimal human owner authority that may hold a migrated or
// merged target's PERMANENT authority, one per scope this contract allows a
// target to occupy. Mirrors controlled-rule-intake.md §9.1's table: a
// delegated-run — the run carrying out the migration itself — legitimately
// drives the migration act, but never mints a record's own permanent
// authority (property 4). run-state carries no entry: an episodic run has no
// human owner, so a target can never be classified into it — even though
// target_scope's own structural shape (property 8) permits a run-state value
// with a workspace_id, business-rule permissibility (property 4) is a
// separate, stricter check.
export const TARGET_AUTHORITY_BY_SCOPE = {
  'built-in-methodology': 'methodology-owner',
  'user-profile': 'user',
  'organization-profile': 'organization',
  'project-workspace': 'project-owner',
  'repository-scope': 'repository-maintainer',
};

const isObject = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);
const isNonEmptyString = (v) => typeof v === 'string' && v !== '';

// An opaque identifier (a portable ref field) must not be a disguised
// filesystem location: no root POSIX path, no Windows drive path, no
// backslash separator, no "file://" URL and no ".." escape segment. This is
// a small, self-contained string safety check — not a copy of
// instruction-source-registry.mjs's private opaqueRefProblem, which that
// module does not export, restated here because every operating-model
// contract that carries portable refs needs its own closed check (property 1
// and property 8: storage-neutral identity, closed to a personal machine
// path).
export function checkOpaqueRef(ref, label) {
  if (typeof ref !== 'string' || ref === '') return `${label} is empty`;
  if (/^file:\/\//i.test(ref)) {
    return `${label} "${ref}" is a file:// URL; a portable ref must be an opaque identifier, not a filesystem location`;
  }
  if (ref.includes('\\') || /^[a-zA-Z]:/.test(ref) || ref.startsWith('/')) {
    return `${label} "${ref}" looks like an absolute machine path; it must be an opaque identifier, not a filesystem location`;
  }
  if (ref.split('/').some((s) => s === '..')) {
    return `${label} "${ref}" carries a ".." segment; it must be an opaque identifier`;
  }
  return null;
}

// A deterministic, order-insensitive canonicalisation: object keys are
// sorted recursively before JSON.stringify, so two structurally identical
// values always canonicalise to the same string regardless of the key order
// they happened to be declared in. Arrays keep their own element order —
// callers that want array-order-independence sort the array themselves
// BEFORE canonicalising it (see computePlanFingerprint).
function canonicalize(value) {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (isObject(value)) {
    const out = {};
    for (const k of Object.keys(value).sort()) out[k] = canonicalize(value[k]);
    return out;
  }
  return value;
}

// The canonical origin.source_ref a target's origin must trace to: the
// sorted, de-duplicated set of the record_unit id(s) actually contributing
// to it. A single contributing unit ("migrated") gets "record-unit:<id>"; a
// many-to-one merge gets "record-units:<id1>,<id2>,...". This is what makes
// origin a checkable fact instead of a freely asserted string (property 4).
export function deriveOriginSourceRef(unitIds) {
  const sorted = [...new Set(Array.isArray(unitIds) ? unitIds : [])].sort();
  return sorted.length === 1 ? `record-unit:${sorted[0]}` : `record-units:${sorted.join(',')}`;
}

// The documented canonical projection plan_fingerprint is computed from
// (property 7): source (including digest), the full record_units (including
// unit_ref/classification_basis) and the full mappings (including
// disposition, the complete target — id/record_type/scope/origin/authority/
// field_basis — merge_rule_ref and owner_decision), and rollback. Every one
// of these, changed, changes the fingerprint. record_units and mappings are
// each SORTED by a stable key before canonicalisation so the array order
// they happen to be declared in never affects the result;
// verification/plan_fingerprint/idempotency_key/supersedes are deliberately
// excluded — they describe the plan's own bookkeeping and verdict, not its
// migration input and result.
export function computePlanFingerprint(payload) {
  const p = isObject(payload) ? payload : {};
  const source = canonicalize(isObject(p.source) ? p.source : {});

  const units = (Array.isArray(p.record_units) ? p.record_units : [])
    .filter(isObject)
    .map(canonicalize)
    .sort((a, b) => String(a.id ?? '').localeCompare(String(b.id ?? '')));

  const mappings = (Array.isArray(p.mappings) ? p.mappings : [])
    .filter(isObject)
    .map(canonicalize)
    .sort((a, b) => {
      const ka = `${a.unit_id ?? ''}::${(isObject(a.target) ? a.target.id : '') ?? ''}`;
      const kb = `${b.unit_id ?? ''}::${(isObject(b.target) ? b.target.id : '') ?? ''}`;
      return ka.localeCompare(kb);
    });

  const rollback = canonicalize(isObject(p.rollback) ? p.rollback : {});

  const canonical = JSON.stringify({ source, record_units: units, mappings, rollback });
  return createHash('sha256').update(canonical).digest('hex');
}

// idempotency_key is DERIVED — a deterministic function of the plan's own
// scope AND the canonical identity of its pinned source: repository_ref,
// revision and the full digest (property 7). All four are load-bearing:
// repository_ref is included so that two DIFFERENT source repositories that
// happen to share the same revision TEXT (e.g. both tagged "v1") never
// collide on the same key, and digest is included so that a source whose
// revision label was reused for genuinely different content is not silently
// treated as the same pinned input. Two plans independently authored over
// the identical (scope, repository_ref, revision, digest) tuple always
// compute the SAME key and therefore collide under checkIdempotency's
// container-wide uniqueness check, instead of passing as two unrelated
// records under two different arbitrary keys.
export function computeIdempotencyKey(scope, source) {
  const s = isObject(scope) ? scope : {};
  const src = isObject(source) ? source : {};
  const digest = isObject(src.digest) ? src.digest : {};
  const canonical = JSON.stringify({
    scope: { type: s.type ?? '', id: s.id ?? '', workspace_id: s.workspace_id ?? '' },
    repository_ref: src.repository_ref ?? '',
    revision: src.revision ?? '',
    digest: { algorithm: digest.algorithm ?? '', value: digest.value ?? '' },
  });
  return createHash('sha256').update(canonical).digest('hex');
}

// Builds a (query) => resolvedSnapshot | null resolver from a plain map keyed
// "<repository_ref>@<revision>", the shape kernel-validate.mjs and the
// standalone test build from a fixtures bundle's own
// "source_snapshot_resolution" object. Not exported logic beyond the lookup
// itself: the RESOLVED response is still checked closed by
// resolveAndCheckSourceSnapshot below, exactly as a hand-written resolver's
// response would be.
export function makeSourceSnapshotResolver(map) {
  const m = isObject(map) ? map : {};
  return (query) => {
    if (!isObject(query)) return null;
    const key = `${query.repository_ref ?? ''}@${query.revision ?? ''}`;
    return Object.prototype.hasOwnProperty.call(m, key) ? m[key] : null;
  };
}

// Builds a (ref) => resolvedRecord | null resolver from a plain map keyed by
// a single opaque ref string — the shape every other external boundary this
// module uses (evidence_ref, rollback.source_snapshot_ref,
// rollback.deterministic_plan_ref, rollback.restoration_evidence_ref) shares.
// Each RESOLVED response is still checked closed by its own
// resolveAndCheck* function below, exactly as a hand-written resolver's
// response would be.
export function makeRefResolver(map) {
  const m = isObject(map) ? map : {};
  return (ref) => (typeof ref === 'string' && Object.prototype.hasOwnProperty.call(m, ref) ? m[ref] : null);
}

// Builds an (evidenceRef) => resolvedEvidence | null resolver from a plain
// map keyed by evidence_ref, the shape kernel-validate.mjs and the standalone
// test build from a fixtures bundle's own "evidence_resolution" object.
export const makeEvidenceResolver = makeRefResolver;

// Builds a (source_snapshot_ref) => resolvedRollbackSnapshot | null resolver
// from a fixtures bundle's own "rollback_snapshot_resolution" object.
export const makeRollbackSnapshotResolver = makeRefResolver;

// Builds a (deterministic_plan_ref) => resolvedPlan | null resolver from a
// fixtures bundle's own "deterministic_plan_resolution" object.
export const makeDeterministicPlanResolver = makeRefResolver;

// Builds a (restoration_evidence_ref) => resolvedEvidence | null resolver
// from a fixtures bundle's own "restoration_evidence_resolution" object.
export const makeRestorationEvidenceResolver = makeRefResolver;

// Builds a (supersedes id) => resolvedPredecessor | null resolver from a
// fixtures bundle's own "superseded_plan_resolution" object — used only when
// the named predecessor is NOT present in the same document.
export const makeSupersededPlanResolver = makeRefResolver;

const RESOLVED_SNAPSHOT_KEYS = ['record_type', 'repository_ref', 'revision', 'digest', 'working_tree_clean'];
const DIGEST_KEYS = ['algorithm', 'value'];

// Property 2: a claimed "reproducible" source is checked against a snapshot
// resolved through the EXTERNAL resolveSourceSnapshot boundary this module
// does not control — the same "closed transformer response" discipline as
// controlled-rule-intake.mjs's resolveAndCheckSourceRef. An absent resolver,
// an unresolved query, an unknown/wrong-shaped response, or a resolved
// snapshot that disagrees with the declared revision, digest or
// working_tree_clean, is rejected: a claimed value with no resolved backing
// never confirms reproducibility. A "not-reproducible" source needs no
// resolved backing — it is already known unusable.
function resolveAndCheckSourceSnapshot(at, source, resolveSourceSnapshot, problems) {
  if (source.qualification !== 'reproducible') return;
  const doResolve = typeof resolveSourceSnapshot === 'function' ? resolveSourceSnapshot : () => null;
  const resolved = doResolve({ repository_ref: source.repository_ref, revision: source.revision });
  if (!isObject(resolved)) {
    problems.push(`${at} source.qualification is "reproducible" but its revision does not resolve to a known source snapshot through the external resolver; a claimed value with no resolved backing never confirms reproducibility (property 2)`);
    return;
  }
  for (const k of Object.keys(resolved)) {
    if (!RESOLVED_SNAPSHOT_KEYS.includes(k)) {
      problems.push(`${at} source: the resolver returned a snapshot with unknown field "${k}"; the resolved response is closed to { ${RESOLVED_SNAPSHOT_KEYS.join(', ')} }`);
    }
  }
  if (resolved.record_type !== SOURCE_SNAPSHOT_RECORD_TYPE) {
    problems.push(`${at} source: resolved snapshot record_type is "${resolved.record_type}", not "${SOURCE_SNAPSHOT_RECORD_TYPE}"`);
  }
  if (resolved.repository_ref !== source.repository_ref) {
    problems.push(`${at} source.repository_ref "${source.repository_ref}" does not match the resolved snapshot's repository_ref "${resolved.repository_ref}"; a snapshot resolved for a different repository never confirms reproducibility (property 2)`);
  }
  if (resolved.revision !== source.revision) {
    problems.push(`${at} source.revision "${source.revision}" does not match the resolved snapshot's revision "${resolved.revision}"; a claimed revision with no matching resolved snapshot never confirms reproducibility (property 2)`);
  }
  const rd = isObject(resolved.digest) ? resolved.digest : {};
  const sd = isObject(source.digest) ? source.digest : {};
  for (const k of Object.keys(rd)) {
    if (!DIGEST_KEYS.includes(k)) {
      problems.push(`${at} source: resolved snapshot digest has unknown field "${k}"; the resolved digest is closed to { ${DIGEST_KEYS.join(', ')} }`);
    }
  }
  if (rd.algorithm !== sd.algorithm || rd.value !== sd.value) {
    problems.push(`${at} source.digest does not match the resolved snapshot's digest (extra, missing or differing field); a claimed digest with no matching resolved snapshot never confirms reproducibility (property 2)`);
  }
  if (resolved.working_tree_clean !== true) {
    problems.push(`${at} source.qualification is "reproducible" but the resolved snapshot's working_tree_clean is not true`);
  }
}

const RESOLVED_EVIDENCE_KEYS = ['record_type', 'evidence_ref', 'kind', 'plan_ref', 'plan_fingerprint', 'confirms'];

// Property 6: a sub-verdict's "verified" status is checked against evidence
// resolved through the EXTERNAL resolveEvidence boundary — a non-empty
// evidence_ref string is never itself accepted as proof. The resolved record
// is checked closed (record_type, evidence_ref echo, the queried kind, a
// plan_ref naming THIS plan — evidence from a different plan cannot back
// this one — and a plan_fingerprint matching THIS plan's own RECOMPUTED
// fingerprint, not merely its id: evidence pinned to a plan whose
// mapping/target/owner_decision/etc. has since changed is pinned to a plan
// that, in every checkable sense, no longer exists) and must explicitly
// confirms: true. An absent resolver, an unresolved evidence_ref, an
// unknown/wrong-shaped/mismatched response, a stale plan_fingerprint, or a
// non-confirming one, is rejected.
function resolveAndCheckEvidence(at, planId, planFingerprint, kind, evidenceRef, resolveEvidence, problems) {
  const doResolve = typeof resolveEvidence === 'function' ? resolveEvidence : () => null;
  const resolved = doResolve(evidenceRef);
  if (!isObject(resolved)) {
    problems.push(`${at} verification.${kind}.evidence_ref "${evidenceRef}" does not resolve to a known evidence record through the external resolver; an unresolved evidence_ref never confirms a verified status (property 6)`);
    return;
  }
  for (const k of Object.keys(resolved)) {
    if (!RESOLVED_EVIDENCE_KEYS.includes(k)) {
      problems.push(`${at} evidence "${evidenceRef}": the resolver returned a record with unknown field "${k}"; the resolved response is closed to { ${RESOLVED_EVIDENCE_KEYS.join(', ')} }`);
    }
  }
  if (resolved.record_type !== EVIDENCE_RECORD_TYPE) {
    problems.push(`${at} evidence "${evidenceRef}": resolved record_type is "${resolved.record_type}", not "${EVIDENCE_RECORD_TYPE}"`);
  }
  if (resolved.evidence_ref !== evidenceRef) {
    problems.push(`${at} evidence "${evidenceRef}": resolved evidence_ref "${resolved.evidence_ref}" does not match the queried "${evidenceRef}"`);
  }
  if (resolved.kind !== kind) {
    problems.push(`${at} evidence "${evidenceRef}": resolved kind "${resolved.kind}" does not match the expected "${kind}"; evidence for one sub-verdict cannot confirm another`);
  }
  if (resolved.plan_ref !== planId) {
    problems.push(`${at} evidence "${evidenceRef}": resolved plan_ref "${resolved.plan_ref}" does not name this plan "${planId}"; evidence from a different plan cannot back this one`);
  }
  if (resolved.plan_fingerprint !== planFingerprint) {
    problems.push(`${at} evidence "${evidenceRef}": resolved plan_fingerprint "${resolved.plan_fingerprint}" does not match this plan's own recomputed fingerprint "${planFingerprint}"; evidence pinned to a stale or different version of this plan's content never confirms the current one (property 6)`);
  }
  if (resolved.confirms !== true) {
    problems.push(`${at} evidence "${evidenceRef}": resolved evidence does not confirm the claimed result (confirms is not true)`);
  }
}

const RESOLVED_ROLLBACK_SNAPSHOT_KEYS = ['record_type', 'source_snapshot_ref', 'repository_ref', 'revision', 'digest'];

// Property 5: rollback.source_snapshot_ref is checked against a record
// resolved through the EXTERNAL resolveRollbackSnapshot boundary — a bare
// non-empty ref string (including a fabricated one, e.g. "snapshot:invented")
// is never itself accepted as a reversible link. The resolved record is
// checked closed and is required to echo the queried ref AND to name the
// SAME repository_ref/revision/digest this plan's own payload.source pins —
// a rollback ref that resolves to a snapshot of a DIFFERENT source never
// confirms reversibility of THIS plan. This check does not depend on
// source.qualification: reversibility is mandatory on every plan regardless
// of whether the source qualifies as a reproducible forward-migration base.
function resolveAndCheckRollbackSnapshot(at, source, rollback, resolveRollbackSnapshot, problems) {
  const doResolve = typeof resolveRollbackSnapshot === 'function' ? resolveRollbackSnapshot : () => null;
  const ref = rollback.source_snapshot_ref;
  const resolved = doResolve(ref);
  if (!isObject(resolved)) {
    problems.push(`${at} rollback.source_snapshot_ref "${ref}" does not resolve to a known source snapshot through the external resolver; a bare ref string never confirms reversibility (property 5)`);
    return;
  }
  for (const k of Object.keys(resolved)) {
    if (!RESOLVED_ROLLBACK_SNAPSHOT_KEYS.includes(k)) {
      problems.push(`${at} rollback.source_snapshot_ref "${ref}": the resolver returned a record with unknown field "${k}"; the resolved response is closed to { ${RESOLVED_ROLLBACK_SNAPSHOT_KEYS.join(', ')} }`);
    }
  }
  if (resolved.record_type !== ROLLBACK_SNAPSHOT_RECORD_TYPE) {
    problems.push(`${at} rollback.source_snapshot_ref "${ref}": resolved record_type is "${resolved.record_type}", not "${ROLLBACK_SNAPSHOT_RECORD_TYPE}"`);
  }
  if (resolved.source_snapshot_ref !== ref) {
    problems.push(`${at} rollback.source_snapshot_ref "${ref}": resolved source_snapshot_ref "${resolved.source_snapshot_ref}" does not echo the queried ref`);
  }
  if (resolved.repository_ref !== source.repository_ref || resolved.revision !== source.revision) {
    problems.push(`${at} rollback.source_snapshot_ref "${ref}" resolves to repository_ref/revision "${resolved.repository_ref}"/"${resolved.revision}", which does not match this plan's own source "${source.repository_ref}"/"${source.revision}"; a rollback snapshot must uniquely link to the SAME confirmed source snapshot this plan pins (property 5)`);
  }
  const rd = isObject(resolved.digest) ? resolved.digest : {};
  const sd = isObject(source.digest) ? source.digest : {};
  for (const k of Object.keys(rd)) {
    if (!DIGEST_KEYS.includes(k)) {
      problems.push(`${at} rollback.source_snapshot_ref "${ref}": resolved digest has unknown field "${k}"; the resolved digest is closed to { ${DIGEST_KEYS.join(', ')} }`);
    }
  }
  if (rd.algorithm !== sd.algorithm || rd.value !== sd.value) {
    problems.push(`${at} rollback.source_snapshot_ref "${ref}" resolves to a digest that does not match this plan's own source.digest (extra, missing or differing field); a rollback snapshot must uniquely link to the SAME confirmed source snapshot (property 5)`);
  }
}

const RESOLVED_DETERMINISTIC_PLAN_KEYS = ['record_type', 'deterministic_plan_ref', 'source_snapshot_ref', 'plan_ref', 'plan_fingerprint', 'applicable'];

// Property 5: rollback.deterministic_plan_ref (required when
// rollback.plan is "deterministic-reconstruction") is checked against a
// record resolved through the EXTERNAL resolveDeterministicPlan boundary —
// closed, echoing the queried ref (as deterministic_plan_ref — NOT to be
// confused with plan_ref below, which names the MIGRATION plan), naming the
// SAME rollback.source_snapshot_ref this plan pins (a reconstruction plan
// for a different snapshot cannot back this rollback), naming THIS
// migration plan's own id (plan_ref) and its own RECOMPUTED plan_fingerprint
// — the same "pinned to exact content, not just id" discipline as
// resolveAndCheckEvidence (property 6) — and explicitly marked applicable.
function resolveAndCheckDeterministicPlan(at, planId, planFingerprint, rollback, resolveDeterministicPlan, problems) {
  const doResolve = typeof resolveDeterministicPlan === 'function' ? resolveDeterministicPlan : () => null;
  const ref = rollback.deterministic_plan_ref;
  const resolved = doResolve(ref);
  if (!isObject(resolved)) {
    problems.push(`${at} rollback.deterministic_plan_ref "${ref}" does not resolve to a known deterministic-reconstruction plan through the external resolver; a bare ref string never confirms reversibility (property 5)`);
    return;
  }
  for (const k of Object.keys(resolved)) {
    if (!RESOLVED_DETERMINISTIC_PLAN_KEYS.includes(k)) {
      problems.push(`${at} rollback.deterministic_plan_ref "${ref}": the resolver returned a record with unknown field "${k}"; the resolved response is closed to { ${RESOLVED_DETERMINISTIC_PLAN_KEYS.join(', ')} }`);
    }
  }
  if (resolved.record_type !== DETERMINISTIC_PLAN_RECORD_TYPE) {
    problems.push(`${at} rollback.deterministic_plan_ref "${ref}": resolved record_type is "${resolved.record_type}", not "${DETERMINISTIC_PLAN_RECORD_TYPE}"`);
  }
  if (resolved.deterministic_plan_ref !== ref) {
    problems.push(`${at} rollback.deterministic_plan_ref "${ref}": resolved deterministic_plan_ref "${resolved.deterministic_plan_ref}" does not echo the queried ref`);
  }
  if (resolved.source_snapshot_ref !== rollback.source_snapshot_ref) {
    problems.push(`${at} rollback.deterministic_plan_ref "${ref}": resolved source_snapshot_ref "${resolved.source_snapshot_ref}" does not name this plan's rollback.source_snapshot_ref "${rollback.source_snapshot_ref}"; a reconstruction plan for a different snapshot cannot back this rollback (property 5)`);
  }
  if (resolved.plan_ref !== planId) {
    problems.push(`${at} rollback.deterministic_plan_ref "${ref}": resolved plan_ref "${resolved.plan_ref}" does not name this plan "${planId}"; a reconstruction plan bound to a different migration plan cannot back this one`);
  }
  if (resolved.plan_fingerprint !== planFingerprint) {
    problems.push(`${at} rollback.deterministic_plan_ref "${ref}": resolved plan_fingerprint "${resolved.plan_fingerprint}" does not match this plan's own recomputed fingerprint "${planFingerprint}"; a reconstruction plan pinned to a stale or different version of this plan's content never confirms the current one (property 5)`);
  }
  if (resolved.applicable !== true) {
    problems.push(`${at} rollback.deterministic_plan_ref "${ref}": resolved plan is not marked applicable`);
  }
}

const RESOLVED_RESTORATION_EVIDENCE_KEYS = ['record_type', 'evidence_ref', 'plan_ref', 'plan_fingerprint', 'source_snapshot_ref', 'confirms'];

// Property 5: rollback.restoration_evidence_ref (required when
// rollback.plan is "verified-restoration") is checked against a record
// resolved through the EXTERNAL resolveRestorationEvidence boundary —
// closed, echoing the queried ref, naming THIS plan's own id (restoration
// evidence from a different plan cannot back this one — the same
// cross-plan discipline as resolveAndCheckEvidence, property 6) AND this
// plan's own RECOMPUTED plan_fingerprint (evidence pinned to a stale or
// different version of this plan's content never confirms the current
// one), naming THIS plan's own rollback.source_snapshot_ref (evidence of
// restoring a different snapshot cannot confirm this rollback) and
// explicitly confirming.
function resolveAndCheckRestorationEvidence(at, planId, planFingerprint, rollback, resolveRestorationEvidence, problems) {
  const doResolve = typeof resolveRestorationEvidence === 'function' ? resolveRestorationEvidence : () => null;
  const ref = rollback.restoration_evidence_ref;
  const resolved = doResolve(ref);
  if (!isObject(resolved)) {
    problems.push(`${at} rollback.restoration_evidence_ref "${ref}" does not resolve to a known restoration-evidence record through the external resolver; a bare ref string never confirms reversibility (property 5)`);
    return;
  }
  for (const k of Object.keys(resolved)) {
    if (!RESOLVED_RESTORATION_EVIDENCE_KEYS.includes(k)) {
      problems.push(`${at} rollback.restoration_evidence_ref "${ref}": the resolver returned a record with unknown field "${k}"; the resolved response is closed to { ${RESOLVED_RESTORATION_EVIDENCE_KEYS.join(', ')} }`);
    }
  }
  if (resolved.record_type !== RESTORATION_EVIDENCE_RECORD_TYPE) {
    problems.push(`${at} rollback.restoration_evidence_ref "${ref}": resolved record_type is "${resolved.record_type}", not "${RESTORATION_EVIDENCE_RECORD_TYPE}"`);
  }
  if (resolved.evidence_ref !== ref) {
    problems.push(`${at} rollback.restoration_evidence_ref "${ref}": resolved evidence_ref "${resolved.evidence_ref}" does not echo the queried ref`);
  }
  if (resolved.plan_ref !== planId) {
    problems.push(`${at} rollback.restoration_evidence_ref "${ref}": resolved plan_ref "${resolved.plan_ref}" does not name this plan "${planId}"; restoration evidence from a different plan cannot back this one (property 5)`);
  }
  if (resolved.plan_fingerprint !== planFingerprint) {
    problems.push(`${at} rollback.restoration_evidence_ref "${ref}": resolved plan_fingerprint "${resolved.plan_fingerprint}" does not match this plan's own recomputed fingerprint "${planFingerprint}"; restoration evidence pinned to a stale or different version of this plan's content never confirms the current one (property 5)`);
  }
  if (resolved.source_snapshot_ref !== rollback.source_snapshot_ref) {
    problems.push(`${at} rollback.restoration_evidence_ref "${ref}": resolved source_snapshot_ref "${resolved.source_snapshot_ref}" does not name this plan's rollback.source_snapshot_ref "${rollback.source_snapshot_ref}"; restoration evidence for a different snapshot cannot confirm this rollback (property 5)`);
  }
  if (resolved.confirms !== true) {
    problems.push(`${at} rollback.restoration_evidence_ref "${ref}": resolved evidence does not confirm restoration (confirms is not true)`);
  }
}

// Property 3: every declared record_units[] id carries EXACTLY ONE mapping —
// never omitted (a unit with no correspondence and no explicit
// retained-transitional decision), never duplicated (two mappings claiming
// the same unit), never dangling (a mapping naming a unit that was never
// declared).
function checkCoverage(at, units, mappings, problems) {
  const unitIds = new Set();
  for (const u of units) {
    if (!isObject(u) || typeof u.id !== 'string') continue;
    if (unitIds.has(u.id)) problems.push(`${at} record_units id "${u.id}" is declared more than once`);
    unitIds.add(u.id);
    if (isObject(u) && typeof u.unit_ref === 'string' && u.unit_ref === u.classification_basis) {
      problems.push(`${at} record unit "${u.id}" classification_basis equals unit_ref verbatim; classification must not be derived from the path or reference alone`);
    }
  }

  const counts = new Map();
  for (const m of mappings) {
    if (!isObject(m) || typeof m.unit_id !== 'string') continue;
    if (!unitIds.has(m.unit_id)) {
      problems.push(`${at} mapping references unit_id "${m.unit_id}", which is not a declared record unit; a mapping cannot correspond to more than was declared`);
      continue;
    }
    counts.set(m.unit_id, (counts.get(m.unit_id) || 0) + 1);
  }
  for (const id of unitIds) {
    const count = counts.get(id) || 0;
    if (count === 0) {
      problems.push(`${at} record unit "${id}" has no mapping; every declared unit requires exactly one correspondence decision (migrated, retained-transitional or merged)`);
    } else if (count > 1) {
      problems.push(`${at} record unit "${id}" has ${count} mappings; exactly one is required per unit — a duplicate mapping is never a valid correspondence`);
    }
  }
}

// Property 3/4 (target groups): every target.id claimed by a "migrated" or
// "merged" mapping is checked as a GROUP, not mapping-by-mapping —
//   - a target.id claimed by more than one "migrated" mapping is an
//     undeclared, implicit merge and is rejected: a shared target requires
//     the explicit "merged" disposition;
//   - a "merged" group requires at least two mappings and a single shared
//     merge_rule_ref (a merge without an explicit, consistent rule, or one
//     that in fact carries only one unit, is rejected);
//   - every member of a group must describe a STRUCTURALLY IDENTICAL target
//     record — two different descriptions of "the same" resulting record are
//     self-contradictory;
//   - every member's target.origin.source_ref must equal the canonical
//     reference derived from the group's ACTUAL contributing unit id(s)
//     (property 4: origin traces to real input, never freely asserted);
//   - every member's target.field_basis.origin must be "assigned": a
//     pre-Meridian record never carried this canonical origin shape, so it
//     can never be claimed "preserved".
function checkTargetGroups(at, mappings, problems) {
  const groups = new Map();
  for (const m of mappings) {
    if (!isObject(m)) continue;
    if (m.disposition !== 'migrated' && m.disposition !== 'merged') continue;
    const targetId = isObject(m.target) ? m.target.id : undefined;
    if (typeof targetId !== 'string') continue;
    if (!groups.has(targetId)) groups.set(targetId, []);
    groups.get(targetId).push(m);
  }

  for (const [targetId, group] of groups) {
    const dispositions = new Set(group.map((m) => m.disposition));

    if (dispositions.has('migrated') && dispositions.has('merged')) {
      problems.push(`${at} target "${targetId}" is claimed by mappings with mixed dispositions (migrated and merged); a shared target is consistently one or the other`);
      continue;
    }
    if (dispositions.has('migrated')) {
      if (group.length > 1) {
        problems.push(`${at} target "${targetId}" is claimed by ${group.length} "migrated" mappings; a target claimed by more than one unit requires the explicit "merged" disposition, not an implicit multi-unit "migrated"`);
        continue;
      }
    } else if (group.length < 2) {
      problems.push(`${at} merge target "${targetId}" is claimed by only ${group.length} mapping(s); a "merged" disposition requires at least two source units combining into one target — a single unit is "migrated", not "merged"`);
      continue;
    } else {
      const rules = new Set(group.map((m) => m.merge_rule_ref).filter((r) => typeof r === 'string'));
      if (rules.size > 1) {
        problems.push(`${at} merge target "${targetId}" is claimed by mappings citing different merge_rule_ref values (${[...rules].join(', ')}); every mapping merging into one target must cite the same explicit rule`);
      }
    }

    const canonicalTargets = new Set(group.map((m) => JSON.stringify(canonicalize(m.target))));
    if (canonicalTargets.size > 1) {
      problems.push(`${at} target "${targetId}" is described inconsistently across the mapping(s) that share it; every mapping contributing to the same target must declare an identical target record`);
    }

    const expectedOriginRef = deriveOriginSourceRef(group.map((m) => m.unit_id));
    for (const m of group) {
      const origin = isObject(m.target) && isObject(m.target.origin) ? m.target.origin : {};
      if (origin.source_ref !== expectedOriginRef) {
        problems.push(`${at} target "${targetId}" origin.source_ref "${origin.source_ref}" does not trace to the actual contributing unit(s); expected "${expectedOriginRef}" (property 4)`);
      }
      const fieldBasis = isObject(m.target) && isObject(m.target.field_basis) ? m.target.field_basis : {};
      if (fieldBasis.origin !== 'assigned') {
        problems.push(`${at} target "${targetId}" field_basis.origin is "${fieldBasis.origin}", not "assigned"; a pre-Meridian record never carried this canonical origin shape, so it can never be claimed "preserved" (property 4)`);
      }
    }
  }
}

// Property 4: a migrated or merged target's permanent authority is the human
// owner authority closed to its OWN scope — never the delegated-run authority
// that carried out the migration act itself, and never a scope (run-state)
// this contract does not permit a target to occupy at all.
function checkTargetAuthority(at, mapping, problems) {
  const target = isObject(mapping.target) ? mapping.target : {};
  const scope = isObject(target.scope) ? target.scope : {};
  const authority = isObject(target.authority) ? target.authority : {};
  const ownerDecision = isObject(mapping.owner_decision) ? mapping.owner_decision : {};

  if (authority.kind === 'delegated-run') {
    problems.push(`${at} target "${target.id}" authority.kind is "delegated-run"; the run carrying out the migration cannot itself mint the record's permanent authority (property 4)`);
  } else {
    const required = TARGET_AUTHORITY_BY_SCOPE[scope.type];
    if (!required) {
      problems.push(`${at} target "${target.id}" scope.type is "${scope.type}"; a migrated or merged record cannot be classified into a scope with no human owner authority`);
    } else if (authority.kind !== required) {
      problems.push(`${at} target "${target.id}" authority.kind is "${authority.kind}", but scope "${scope.type}" requires owner authority "${required}"`);
    }
  }

  if (typeof authority.decision_ref === 'string' && typeof ownerDecision.decision_ref === 'string'
      && authority.decision_ref !== ownerDecision.decision_ref) {
    problems.push(`${at} target "${target.id}" authority.decision_ref "${authority.decision_ref}" does not match mapping.owner_decision.decision_ref "${ownerDecision.decision_ref}"; both name the same owner decision`);
  }

  for (const [field, label] of [
    ['id', 'target.id'], ['record_type', 'target.record_type'],
  ]) {
    if (typeof target[field] !== 'string' || target[field] === '') {
      problems.push(`${at} ${label} is missing`);
    }
  }
}

// Property 6: a claimed overall_status "VERIFIED" is checked against the
// actual sub-verdicts and against the source's own reproducibility, never
// merely asserted. A not-reproducible source (dirty, incomplete, ambiguous or
// unverifiable working tree) blocks the whole plan outright — it is never
// accepted as a reproducible base, so the plan can only be BLOCKED, not a
// softer UNVERIFIED.
function checkVerification(at, payload, problems) {
  const source = isObject(payload.source) ? payload.source : {};
  const verification = isObject(payload.verification) ? payload.verification : {};
  const coverage = isObject(verification.coverage) ? verification.coverage : {};
  const applicability = isObject(verification.applicability_preservation) ? verification.applicability_preservation : {};
  const overall = verification.overall_status;

  if (source.qualification === 'not-reproducible' && overall !== 'BLOCKED') {
    problems.push(`${at} source.qualification is "not-reproducible" but verification.overall_status is "${overall}"; a dirty, incomplete, ambiguous or changed source is never accepted as a reproducible base, and the plan is BLOCKED, not merely unverified`);
  }

  const fullyVerified = coverage.status === 'verified' && applicability.status === 'verified' && source.qualification === 'reproducible';
  if (overall === 'VERIFIED' && !fullyVerified) {
    const reasons = [];
    if (coverage.status !== 'verified') reasons.push('coverage is not verified');
    if (applicability.status !== 'verified') reasons.push('applicability preservation is not verified');
    if (source.qualification !== 'reproducible') reasons.push('the source is not reproducible');
    problems.push(`${at} verification.overall_status is "VERIFIED" but ${reasons.join('; ')}; a claimed result is checked against actual evidence, never asserted on its own (property 6)`);
  }
}

// Property 7: idempotency_key is unique across the whole container — a rerun
// over the same pinned input recognises the same record and never mints a
// second, duplicate one.
function checkIdempotency(entries, problems) {
  const seen = new Map();
  for (const e of entries) {
    if (!isObject(e) || !isObject(e.payload)) continue;
    const key = e.payload.idempotency_key;
    if (typeof key !== 'string' || key === '') continue;
    if (seen.has(key)) {
      problems.push(`idempotency_key "${key}" is declared by more than one migration plan ("${seen.get(key)}" and "${e.id}"); a rerun over the same pinned input must recognise the existing record, not mint a duplicate`);
    } else {
      seen.set(key, e.id);
    }
  }
}

const SCOPE_FIELD_KEYS = ['type', 'id', 'workspace_id', 'organization_profile_id'];
const RESOLVED_SUPERSEDED_PLAN_KEYS = ['record_type', 'plan_ref', 'scope', 'repository_ref', 'revision'];

const sameScope = (a, b) => a.type === b.type && a.id === b.id
  && a.workspace_id === b.workspace_id && a.organization_profile_id === b.organization_profile_id;

// Property 7: a plan cannot supersede itself; a superseding plan must share
// its predecessor's FULL scope (type/id/workspace_id/organization_profile_id
// — not a partial match), the SAME source repository_ref, and pin a
// DIFFERENT source revision — a plan cannot supersede a plan pinned to the
// identical revision (that is a duplicate, already caught by
// checkIdempotency, not a genuine successor), nor a plan of a different
// scope or a different source repository altogether. When the named
// predecessor IS present in this same document, its own declared scope and
// source are used directly. When it is NOT present (an earlier snapshot
// this run does not carry), the reference is resolved through the EXTERNAL
// resolveSupersededPlan boundary and checked closed exactly like every other
// external boundary this module uses — an unknown or unresolved id is NEVER
// accepted automatically; a supersedes reference this run cannot verify is a
// defect, not a benign gap.
function checkSupersedes(entries, resolveSupersededPlan, problems) {
  const byId = new Map(entries.filter(isObject).filter((e) => typeof e.id === 'string').map((e) => [e.id, e]));
  for (const e of entries) {
    if (!isObject(e) || !isObject(e.payload)) continue;
    const sup = e.payload.supersedes;
    if (!isNonEmptyString(sup)) continue;
    if (sup === e.id) {
      problems.push(`migration plan "${e.id}" supersedes its own id; a plan cannot supersede itself`);
      continue;
    }
    const eScope = isObject(e.scope) ? e.scope : {};
    const eSource = isObject(e.payload.source) ? e.payload.source : {};
    const target = byId.get(sup);

    if (target) {
      const tScope = isObject(target.scope) ? target.scope : {};
      const tSource = isObject(target.payload) && isObject(target.payload.source) ? target.payload.source : {};
      if (!sameScope(eScope, tScope)) {
        problems.push(`migration plan "${e.id}" supersedes "${sup}" but they are scoped differently; a plan supersedes only a predecessor for the SAME full scope (type, id, workspace_id and organization_profile_id)`);
      }
      if (eSource.repository_ref !== tSource.repository_ref) {
        problems.push(`migration plan "${e.id}" supersedes "${sup}" but they pin different source.repository_ref ("${eSource.repository_ref}" vs "${tSource.repository_ref}"); a plan supersedes only a predecessor of the SAME source repository`);
      }
      if (eSource.revision === tSource.revision) {
        problems.push(`migration plan "${e.id}" supersedes "${sup}" but both pin the identical source.revision "${eSource.revision}"; a superseding plan requires a changed source revision, not a restatement of the same one`);
      }
      continue;
    }

    const doResolve = typeof resolveSupersededPlan === 'function' ? resolveSupersededPlan : () => null;
    const resolved = doResolve(sup);
    if (!isObject(resolved)) {
      problems.push(`migration plan "${e.id}" supersedes "${sup}", which is not present in this document and does not resolve to a known predecessor through the external resolver; an unknown or unresolved predecessor is never accepted automatically`);
      continue;
    }
    for (const k of Object.keys(resolved)) {
      if (!RESOLVED_SUPERSEDED_PLAN_KEYS.includes(k)) {
        problems.push(`migration plan "${e.id}" supersedes "${sup}": the resolver returned a record with unknown field "${k}"; the resolved response is closed to { ${RESOLVED_SUPERSEDED_PLAN_KEYS.join(', ')} }`);
      }
    }
    if (resolved.record_type !== SUPERSEDED_PLAN_RECORD_TYPE) {
      problems.push(`migration plan "${e.id}" supersedes "${sup}": resolved record_type is "${resolved.record_type}", not "${SUPERSEDED_PLAN_RECORD_TYPE}"`);
    }
    if (resolved.plan_ref !== sup) {
      problems.push(`migration plan "${e.id}" supersedes "${sup}": resolved plan_ref "${resolved.plan_ref}" does not echo the queried predecessor`);
    }
    const rScope = isObject(resolved.scope) ? resolved.scope : {};
    for (const k of Object.keys(rScope)) {
      if (!SCOPE_FIELD_KEYS.includes(k)) {
        problems.push(`migration plan "${e.id}" supersedes "${sup}": resolved scope has unknown field "${k}"; the resolved scope is closed to { ${SCOPE_FIELD_KEYS.join(', ')} }`);
      }
    }
    if (!sameScope(eScope, rScope)) {
      problems.push(`migration plan "${e.id}" supersedes "${sup}" but the resolved predecessor is scoped differently; a plan supersedes only a predecessor for the SAME full scope (type, id, workspace_id and organization_profile_id)`);
    }
    if (eSource.repository_ref !== resolved.repository_ref) {
      problems.push(`migration plan "${e.id}" supersedes "${sup}" but the resolved predecessor pins a different repository_ref ("${resolved.repository_ref}"); a plan supersedes only a predecessor of the SAME source repository`);
    }
    if (eSource.revision === resolved.revision) {
      problems.push(`migration plan "${e.id}" supersedes "${sup}" but the resolved predecessor pins the identical source.revision "${eSource.revision}"; a superseding plan requires a changed source revision, not a restatement of the same one`);
    }
  }
}

// The whole composition pipeline for one instance-data-migration document:
// container + payload schema, per-entry envelope schema, resolution of every
// source snapshot and verified-evidence claim through the external
// boundaries, then the cross-cutting rules the JSON Schema subset cannot
// state.
export function evaluateInstanceDataMigration(doc, {
  registrySchema, envelopeSchema, resolveSourceSnapshot, resolveEvidence,
  resolveRollbackSnapshot, resolveDeterministicPlan, resolveRestorationEvidence,
  resolveSupersededPlan,
} = {}) {
  if (!isObject(registrySchema)) return ['registrySchema is not an object; the instance-data-migration contract cannot be checked without its schema'];
  if (!isObject(envelopeSchema)) return ['envelopeSchema is not an object; a migration plan composes with the record envelope and cannot be checked without it'];

  const problems = [];

  const containerErrs = [];
  try { validate(doc, registrySchema, registrySchema, '', containerErrs); }
  catch (e) { return [`container/payload schema could not be applied: ${e.message}`]; }
  problems.push(...containerErrs);

  const entries = Array.isArray(doc && doc.migration_plans) ? doc.migration_plans : [];

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
    const at = `migration plan "${id}"`;
    if (typeof e.id === 'string') {
      if (seenIds.has(e.id)) problems.push(`${at} is declared more than once`);
      seenIds.add(e.id);
    }
    if (e.record_type !== RECORD_TYPE) {
      problems.push(`${at} declares record_type "${e.record_type}", not "${RECORD_TYPE}"`);
    }

    const scope = isObject(e.scope) ? e.scope : {};
    if (!ALLOWED_SCOPE_TYPES.includes(scope.type)) {
      problems.push(`${at} scope.type is "${scope.type}"; a migration plan is scoped to project-workspace or repository-scope only`);
    }
    const origin = isObject(e.origin) ? e.origin : {};
    if (origin.kind !== REQUIRED_ORIGIN_KIND) {
      problems.push(`${at} origin.kind is "${origin.kind}", not "${REQUIRED_ORIGIN_KIND}"; a migration plan is an explicit declared act, never text derived from another source`);
    }
    const authority = isObject(e.authority) ? e.authority : {};
    if (authority.kind !== REQUIRED_AUTHORITY_KIND) {
      problems.push(`${at} authority.kind is "${authority.kind}", not "${REQUIRED_AUTHORITY_KIND}"; the plan itself is carried out by a run's delegated authority and never mints an owner decision on its own`);
    }

    const payload = isObject(e.payload) ? e.payload : {};
    // Computed once per plan and threaded into every resolver check below
    // (property 6/5): evidence is pinned to the plan's EXACT content, not
    // only its id — a stale evidence record for an id whose mapping/target/
    // owner_decision has since changed must not be accepted as still
    // confirming the current content.
    const computedFingerprint = computePlanFingerprint(payload);

    // Property 1 / 8: storage-neutral, closed portable refs.
    const source = isObject(payload.source) ? payload.source : {};
    if ('repository_ref' in source) {
      const r = checkOpaqueRef(source.repository_ref, `${at} source.repository_ref`);
      if (r) problems.push(r);
    }
    const rollback = isObject(payload.rollback) ? payload.rollback : {};
    for (const [field, label] of [
      ['source_snapshot_ref', 'rollback.source_snapshot_ref'],
      ['deterministic_plan_ref', 'rollback.deterministic_plan_ref'],
      ['restoration_evidence_ref', 'rollback.restoration_evidence_ref'],
    ]) {
      if (field in rollback) {
        const r = checkOpaqueRef(rollback[field], `${at} ${label}`);
        if (r) problems.push(r);
      }
    }
    const verificationRaw = isObject(payload.verification) ? payload.verification : {};
    for (const [obj, label] of [
      [verificationRaw.coverage, 'verification.coverage.evidence_ref'],
      [verificationRaw.applicability_preservation, 'verification.applicability_preservation.evidence_ref'],
    ]) {
      if (isObject(obj) && 'evidence_ref' in obj) {
        const r = checkOpaqueRef(obj.evidence_ref, `${at} ${label}`);
        if (r) problems.push(r);
      }
    }
    for (const m of Array.isArray(payload.mappings) ? payload.mappings : []) {
      if (isObject(m) && 'merge_rule_ref' in m) {
        const r = checkOpaqueRef(m.merge_rule_ref, `${at} mapping "${m.unit_id}" merge_rule_ref`);
        if (r) problems.push(r);
      }
    }

    // Property 2: a claimed reproducible source is resolved and matched
    // against an external snapshot boundary.
    resolveAndCheckSourceSnapshot(at, source, resolveSourceSnapshot, problems);

    // Property 5: reversibility is actually checkable, not merely
    // schema-shaped — every rollback ref is resolved through its own
    // external boundary and checked to link to THIS plan's own source and
    // id, regardless of source.qualification.
    resolveAndCheckRollbackSnapshot(at, source, rollback, resolveRollbackSnapshot, problems);
    if (rollback.plan === 'deterministic-reconstruction') {
      resolveAndCheckDeterministicPlan(at, id, computedFingerprint, rollback, resolveDeterministicPlan, problems);
    } else if (rollback.plan === 'verified-restoration') {
      resolveAndCheckRestorationEvidence(at, id, computedFingerprint, rollback, resolveRestorationEvidence, problems);
    }

    // Property 3: complete correspondence.
    const units = Array.isArray(payload.record_units) ? payload.record_units : [];
    const mappings = Array.isArray(payload.mappings) ? payload.mappings : [];
    checkCoverage(at, units, mappings, problems);
    checkTargetGroups(at, mappings, problems);

    // Property 4: semantics preservation, for every mapping that mints a
    // managed target.
    for (const m of mappings) {
      if (!isObject(m)) continue;
      if (m.disposition === 'migrated' || m.disposition === 'merged') {
        checkTargetAuthority(at, m, problems);
      }
    }

    // Property 6: verifiable correspondence — claimed verdict vs. sub-verdicts,
    // and every "verified" sub-verdict's evidence resolved through the
    // external boundary.
    checkVerification(at, payload, problems);
    for (const kind of EVIDENCE_KINDS) {
      const sub = isObject(verificationRaw[kind]) ? verificationRaw[kind] : {};
      if (sub.status === 'verified') {
        resolveAndCheckEvidence(at, id, computedFingerprint, kind, sub.evidence_ref, resolveEvidence, problems);
      }
    }

    // Property 7: repeatability — the declared fingerprint and idempotency
    // key both recompute.
    const declaredFingerprint = payload.plan_fingerprint;
    if (typeof declaredFingerprint === 'string' && declaredFingerprint !== computedFingerprint) {
      problems.push(`${at} plan_fingerprint "${declaredFingerprint}" does not match the recomputed fingerprint "${computedFingerprint}" of its own documented canonical projection (source, record_units, mappings, rollback); the same pinned input must always compute the same fingerprint`);
    }
    const declaredIdempotencyKey = payload.idempotency_key;
    const computedIdempotencyKey = computeIdempotencyKey(scope, source);
    if (typeof declaredIdempotencyKey === 'string' && declaredIdempotencyKey !== computedIdempotencyKey) {
      problems.push(`${at} idempotency_key "${declaredIdempotencyKey}" does not match the recomputed key "${computedIdempotencyKey}" derived from this plan's own scope and source (repository_ref, revision, digest); idempotency_key is never an arbitrary free-form string`);
    }
  }

  checkIdempotency(entries, problems);
  checkSupersedes(entries, resolveSupersededPlan, problems);

  return problems;
}
