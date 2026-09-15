/**
 * workspace-compatibility-qualification: the one checkable implementation.
 *
 * registries/operating-model/workspace-compatibility-qualification.schema.json
 * is the specialised payload schema for the final, product-neutral
 * qualification (record_type: workspace-compatibility-qualification) of one
 * workspace or repository's run through the meridian-workspace-compatibility
 * program — workspace-scope-model, instruction-source-registry,
 * controlled-rule-intake, existing-project-compatibility-mode and
 * instance-data-migration (including its instance-canonical-export
 * addendum). This module composes those FIVE already-checkable contracts
 * into one closed verdict; it duplicates none of their logic.
 *
 * Bounded, not embedded (property: no persisted raw content). A qualification
 * record never carries a composed workspace_connection, migration_plan or
 * canonical_export INLINE — only a closed, pinned reference to each:
 *
 *   payload.workspace_connection_ref  — { record_type, id, reference, sha256 }
 *   payload.migration_plan_ref        — { record_type, id, reference, sha256 } | null
 *   payload.canonical_export_ref      — { record_type, id, reference, sha256 } | null
 *
 * Each reference is resolved through an EXTERNAL boundary this module does
 * not control (resolveConnectionRecord / resolvePlanRecord /
 * resolveExportRecord, supplied by the caller). The resolved FULL record is
 * used ONLY inside this evaluation — to compose the real
 * evaluateExistingProjectCompatibilityMode / evaluateInstanceDataMigration /
 * evaluateInstanceCanonicalExport against it — and is never written back
 * into `doc` or returned. A discovered source's raw_excerpt/normalized_text,
 * and every other field of the composed records, therefore never reaches a
 * persisted qualification record: only the closed reference does.
 *
 * Exact-version pinning (property: no same-id substitution). A bare id is
 * NEVER proof of identity for any of the three references: each MUST carry
 * `sha256`, checked against the RESOLVED record's own recomputed content
 * digest — computePlanFingerprint / computeExportDigest for the migration
 * plan and canonical export (the same pure functions
 * instance-data-migration.mjs already uses to reject a stale or substituted
 * plan/export), and this module's OWN computeConnectionDigest for the
 * workspace connection scan, which existing-project-compatibility-mode.md
 * carries no analogous fingerprint function for. A resolved record whose
 * content differs from what was pinned — even under the exact same id AND
 * reference string — is rejected: the digest is recomputed from what the
 * resolver actually returns, never trusted on the strength of a matching
 * label (checked requirement, not a documentation promise — see the
 * dedicated "подмена содержимого" test). When a canonical_export_ref is
 * composed, this module does not forward the caller's own
 * resolveMigrationPlan boundary to evaluateInstanceCanonicalExport at all: it
 * builds a NEW, single-entry resolver from the ALREADY-resolved,
 * ALREADY-fingerprint-pinned migration_plan_ref record, so an independently
 * configured resolver map can never substitute a different plan sharing the
 * same id for the export's own plan check.
 *
 * No silent QUALIFIED over a pending decision (property: undecided
 * candidates block full qualification). A workspace connection's own
 * next_step can read "continue-compatibility-mode" while a rule candidate it
 * carries is still applicability_state "candidate" — existing-project-compatibility-mode.md's
 * own next_step priority does not look inside rule_candidates at all, only
 * at findings. This module closes that gap itself: whenever the RESOLVED
 * connection carries at least one rule candidate still "candidate",
 * qualification_state can never be QUALIFIED — it is UNVERIFIED (an
 * unresolved, checkable owner decision is exactly the explicit-unknown case
 * open_questions exists for), regardless of migration plan state.
 *
 * Decision matrix, not "the eight scenarios" (property: separated concerns).
 * computeQualificationState is a closed, pure PRIORITY FUNCTION with nine
 * mutually exclusive BRANCHES — an internal implementation detail of how one
 * qualification_state is derived from the composed records' own next_step /
 * pending rule-candidate decisions / verification.overall_status /
 * canonical-export coverage. It is not the program's acceptance scenarios:
 * those are a separate, named set (empty-project-no-instance,
 * existing-project-zero-write, duplicate-rule-provenance,
 * conflict-order-independent, source-change-no-silent-replacement,
 * agent-native-partial-visibility, multi-repository-workspace,
 * migration-applicability-preservation), documented in
 * workspace-compatibility-qualification.md §4.2 and proven by dedicated
 * fixtures/tests in test/workspace-compatibility-qualification.test.mjs —
 * never conflated with this function's branch count (workspace-compatibility-qualification.md
 * §4.1 explains why the two counts are not the same claim, and why they no
 * longer even coincide numerically).
 *
 * Closed result axes (property: three, not two). qualification_state,
 * blockers and open_questions are checked as ONE closed relation, not three
 * independent fields: blockers is non-empty EXACTLY when qualification_state
 * is BLOCKED; open_questions is non-empty EXACTLY when qualification_state is
 * UNVERIFIED — so QUALIFIED carries neither, BLOCKED never hides behind a
 * merely-open question, and UNVERIFIED always names a checkable pending
 * reason rather than resting on a bare label.
 */
import { createHash } from 'node:crypto';
import { validate } from './json-schema.mjs';
import { evaluateExistingProjectCompatibilityMode } from './existing-project-compatibility-mode.mjs';
import {
  evaluateInstanceDataMigration, evaluateInstanceCanonicalExport,
  computePlanFingerprint, computeExportDigest,
} from './instance-data-migration.mjs';

const isObject = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);

export const RECORD_TYPE = 'workspace-compatibility-qualification';
export const ALLOWED_SCOPE_TYPES = ['project-workspace', 'repository-scope'];

/**
 * The qualification record is a technical computation over already-composed
 * records, never itself an act of declaration or an owner decision: its
 * envelope origin is always "derived" (computed from the connection scan it
 * pins) and its envelope authority is always a run's delegated authority.
 */
export const REQUIRED_ORIGIN_KIND = 'derived';
export const REQUIRED_AUTHORITY_KIND = 'delegated-run';

export const QUALIFICATION_STATES = ['QUALIFIED', 'BLOCKED', 'UNVERIFIED'];

/** The closed field set of one pinned reference this module resolves. */
export const PINNED_REF_KEYS = ['record_type', 'id', 'reference', 'sha256'];

const SHA256_HEX = /^[0-9a-f]{64}$/;

/**
 * Canonical, deterministic origin.source_ref for a qualification record —
 * the same "kind:id" convention canonicalOriginSourceRef
 * (controlled-rule-intake) and deriveExportOriginSourceRef
 * (instance-data-migration) already use for their own composed origin.
 */
export function canonicalConnectionSourceRef(connectionId) {
  return `workspace-connection-scan:${connectionId}`;
}

/**
 * Recursive, key-order-insensitive canonicalisation — the same shape of
 * projection computePlanFingerprint/computeExportDigest already apply to
 * their own inputs (instance-data-migration.mjs), reimplemented here because
 * that module keeps its own canonicalize private and this module composes,
 * never duplicates, a package's PUBLIC contract, not its internal helpers.
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
 * The deterministic content digest of one FULLY RESOLVED workspace-connection-scan
 * record — the fingerprint workspace_connection_ref.sha256 pins.
 * existing-project-compatibility-mode.md defines no fingerprint function of
 * its own to reuse (unlike a migration plan's plan_fingerprint or a
 * canonical export's digest), so this module defines one, over the record's
 * complete meaningful content (id, title, record_type, scope, origin,
 * authority, payload) — nothing is excluded, because a connection scan
 * carries no self-describing bookkeeping field analogous to a migration
 * plan's own verification/plan_fingerprint/idempotency_key. SHA-256 over the
 * canonicalised (recursively sorted-key) JSON projection, insensitive to
 * declaration order.
 */
export function computeConnectionDigest(connection) {
  const c = isObject(connection) ? connection : {};
  const projection = {
    id: c.id,
    title: c.title,
    record_type: c.record_type,
    scope: c.scope,
    origin: c.origin,
    authority: c.authority,
    payload: c.payload,
  };
  return createHash('sha256').update(JSON.stringify(canonicalize(projection))).digest('hex');
}

const SCOPE_FIELD_KEYS = ['type', 'id', 'workspace_id', 'organization_profile_id'];
function sameScope(a, b) {
  const x = isObject(a) ? a : {};
  const y = isObject(b) ? b : {};
  return SCOPE_FIELD_KEYS.every((k) => x[k] === y[k]);
}

/**
 * Checks the closed shape of one pinned reference — { record_type, id,
 * reference, sha256 } — never a plain string, never an embedded body. All
 * three references this module resolves (workspace_connection_ref,
 * migration_plan_ref, canonical_export_ref) require `sha256`: a bare id and
 * reference string are never proof of identity on their own — each pins the
 * resolved record's own recomputed content digest (computeConnectionDigest /
 * computePlanFingerprint / computeExportDigest).
 *
 * Returns true when the shape itself is well-formed enough to attempt
 * resolution (a caller still checks `typeof ref.reference === 'string'`
 * before calling a resolver — a malformed reference is reported here AND
 * left unresolved, never guessed at).
 */
function checkPinnedRefShape(problems, at, field, ref) {
  const requireSha256 = true;
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
  if (requireSha256) {
    if (typeof ref.sha256 !== 'string' || !SHA256_HEX.test(ref.sha256)) {
      problems.push(`${at} ${field}.sha256 is required and must be 64 lowercase hex characters — the exact recomputed content pin, never a bare id`);
      ok = false;
    }
  } else if ('sha256' in ref && (typeof ref.sha256 !== 'string' || !SHA256_HEX.test(ref.sha256))) {
    problems.push(`${at} ${field}.sha256, when present, must be 64 lowercase hex characters`);
    ok = false;
  }
  return ok;
}

/**
 * Resolves one pinned reference through an external boundary and checks the
 * resolved response's own record_type/id echo the pinned values — the same
 * "a claimed value with no resolved, matching backing never confirms
 * anything" discipline every other external boundary in this program
 * applies. Returns the resolved record, or null when it could not be
 * resolved or did not echo correctly (the caller still receives the
 * resolved object in the latter case so composition can proceed and surface
 * further, more specific problems — a wrong echo is reported once here, not
 * silently treated as a hard stop for every later check).
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
  return resolved;
}

/**
 * The closed, pure decision matrix this package adds — nine mutually
 * exclusive BRANCHES of one priority, not the program's acceptance
 * scenarios (see the module-level comment above). It reads five signals off
 * records the composed evaluators have ALREADY checked clean — it never
 * recomputes next_step or overall_status itself (those belong to the
 * contracts that own them) and never reaches into an array by position:
 *
 *   1. a blocking "resolve-conflict"/"resolve-ambiguity" next_step is a
 *      fail-closed ambiguity — BLOCKED, never merely unverified;
 *   2. any other non-"continue-compatibility-mode" next_step (chiefly
 *      "await-owner-decision", and any value this priority does not
 *      otherwise recognise) means the compatibility chain is not yet
 *      resolved — UNVERIFIED, the explicit-unknown default;
 *   3. at least one resolved rule candidate still applicability_state
 *      "candidate" — an owner decision this program's own next_step
 *      priority does not itself look for — is UNVERIFIED, never QUALIFIED,
 *      regardless of migration plan state;
 *   4. "continue-compatibility-mode" with every candidate decided and no
 *      migration plan composed qualifies the workspace as-is — QUALIFIED;
 *   5. a composed migration plan's own BLOCKED verification forces BLOCKED
 *      here too — never softened;
 *   6. anything other than a plan's own VERIFIED (chiefly UNVERIFIED, and
 *      any value this priority does not otherwise recognise) is UNVERIFIED;
 *   7. a VERIFIED plan that mints at least one migrated/merged record still
 *      needs a composed canonical export proving that record was actually
 *      produced — absent one, UNVERIFIED;
 *   8. otherwise — VERIFIED, and either nothing was minted or a canonical
 *      export already accounts for what was — QUALIFIED.
 *
 * (Branch 1 covers two table rows — resolve-conflict and resolve-ambiguity —
 * which is why workspace-compatibility-qualification.md §4.1 tables nine
 * rows over these eight logical steps.)
 *
 * The same input, read through any order of object-key enumeration, always
 * computes the same output.
 */
export function computeQualificationState({
  nextStep, hasUndecidedCandidates, hasMigrationPlan, migrationOverallStatus, mintsRecords, hasCanonicalExport,
} = {}) {
  if (nextStep === 'resolve-conflict' || nextStep === 'resolve-ambiguity') return 'BLOCKED';
  if (nextStep !== 'continue-compatibility-mode') return 'UNVERIFIED';
  if (hasUndecidedCandidates) return 'UNVERIFIED';
  if (!hasMigrationPlan) return 'QUALIFIED';
  if (migrationOverallStatus === 'BLOCKED') return 'BLOCKED';
  if (migrationOverallStatus !== 'VERIFIED') return 'UNVERIFIED';
  if (mintsRecords && !hasCanonicalExport) return 'UNVERIFIED';
  return 'QUALIFIED';
}

function mintsAnyRecord(migrationPlan) {
  const mappings = isObject(migrationPlan) && isObject(migrationPlan.payload) && Array.isArray(migrationPlan.payload.mappings)
    ? migrationPlan.payload.mappings : [];
  return mappings.some((m) => isObject(m) && (m.disposition === 'migrated' || m.disposition === 'merged'));
}

/**
 * True when the resolved workspace-connection-scan record carries at least
 * one rule_candidates[] entry whose candidate.payload.applicability_state is
 * still "candidate" — an owner decision genuinely pending. Reads only the
 * named field of each wrapper via Array#some; the result never depends on
 * where in the array that entry sits.
 */
function hasUndecidedRuleCandidates(connection) {
  const wrappers = isObject(connection) && isObject(connection.payload) && Array.isArray(connection.payload.rule_candidates)
    ? connection.payload.rule_candidates : [];
  return wrappers.some((w) => isObject(w) && isObject(w.candidate) && isObject(w.candidate.payload)
    && w.candidate.payload.applicability_state === 'candidate');
}

/**
 * The whole composition pipeline for one workspace-compatibility-qualification
 * document: container + payload schema, per-entry envelope schema,
 * resolution of workspace_connection_ref/migration_plan_ref/canonical_export_ref
 * through the caller-supplied external boundaries, composition of the
 * RESOLVED records against the REAL evaluators of the three contracts they
 * name, then the cross-cutting rules the JSON Schema subset cannot state
 * (exact-version pinning, scope agreement, the recomputed qualification_state,
 * and the blockers/open_questions/qualification_state closed relation).
 *
 * `compatOptions` / `migrationOptions` / `exportOptions` carry the composed
 * contracts' OWN schemas and OWN external boundaries (never this module's
 * three top-level resolvers): compatOptions needs registrySchema/envelopeSchema/
 * sourceRegistrySchema/ruleIntakeSchema; migrationOptions needs
 * registrySchema/envelopeSchema plus its six resolveX boundaries;
 * exportOptions needs registrySchema/envelopeSchema/resolveSourceContent —
 * `exportOptions.resolveMigrationPlan`, even if supplied, is IGNORED: this
 * module always derives the export's plan boundary itself from the
 * already-resolved, already-fingerprint-pinned migration_plan_ref record.
 */
export function evaluateWorkspaceCompatibilityQualification(doc, {
  registrySchema, envelopeSchema,
  resolveConnectionRecord, resolvePlanRecord, resolveExportRecord,
  compatOptions = {}, migrationOptions = {}, exportOptions = {},
} = {}) {
  if (!isObject(registrySchema)) return ['registrySchema is not an object; the workspace-compatibility-qualification contract cannot be checked without its schema'];
  if (!isObject(envelopeSchema)) return ['envelopeSchema is not an object; a qualification record composes with the record envelope and cannot be checked without it'];

  const namespaceFailures = {
    'compatOptions.registrySchema': isObject(compatOptions.registrySchema),
    'compatOptions.envelopeSchema': isObject(compatOptions.envelopeSchema),
    'compatOptions.sourceRegistrySchema': isObject(compatOptions.sourceRegistrySchema),
    'compatOptions.ruleIntakeSchema': isObject(compatOptions.ruleIntakeSchema),
    'migrationOptions.registrySchema': isObject(migrationOptions.registrySchema),
    'migrationOptions.envelopeSchema': isObject(migrationOptions.envelopeSchema),
    'exportOptions.registrySchema': isObject(exportOptions.registrySchema),
    'exportOptions.envelopeSchema': isObject(exportOptions.envelopeSchema),
  };
  const missing = Object.keys(namespaceFailures).filter((k) => !namespaceFailures[k]);
  if (missing.length) {
    return [`workspace-compatibility-qualification composition requires the real composed contract schemas regardless of this document's own content (a workspace_connection_ref is always composed; migration_plan_ref/canonical_export_ref are composed whenever they are not null): missing or not an object — ${missing.join(', ')}`];
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
    if (!ALLOWED_SCOPE_TYPES.includes(scope.type)) {
      problems.push(`${at} scope.type is "${scope.type}"; a qualification is scoped to project-workspace or repository-scope only`);
    }
    const origin = isObject(e.origin) ? e.origin : {};
    if (origin.kind !== REQUIRED_ORIGIN_KIND) {
      problems.push(`${at} origin.kind is "${origin.kind}", not "${REQUIRED_ORIGIN_KIND}"; a qualification is computed from the composed records it pins, never declared on its own`);
    }
    const authority = isObject(e.authority) ? e.authority : {};
    if (authority.kind !== REQUIRED_AUTHORITY_KIND) {
      problems.push(`${at} authority.kind is "${authority.kind}", not "${REQUIRED_AUTHORITY_KIND}"; computing a qualification is carried out by a run and never itself mints an owner decision`);
    }

    const payload = isObject(e.payload) ? e.payload : {};
    const connectionRef = payload.workspace_connection_ref;
    const migrationRef = payload.migration_plan_ref;
    const exportRef = payload.canonical_export_ref;

    /* --- workspace_connection_ref: always required, never nullable, always
       sha256-pinned to the resolved record's own recomputed content digest. --- */
    let connection = null;
    checkPinnedRefShape(problems, at, 'payload.workspace_connection_ref', connectionRef);
    if (isObject(connectionRef)) {
      if (origin.source_ref !== canonicalConnectionSourceRef(connectionRef.id)) {
        problems.push(`${at} origin.source_ref "${origin.source_ref}" does not equal "${canonicalConnectionSourceRef(connectionRef.id)}"; the envelope and payload must pin the same composed workspace connection`);
      }
      if (typeof connectionRef.reference === 'string' && connectionRef.reference !== '') {
        const resolvedConnection = resolveNamedRecord(problems, at, 'payload.workspace_connection_ref', connectionRef, resolveConnectionRecord, 'workspace-connection-scan');
        if (resolvedConnection) {
          const recomputedConnectionDigest = computeConnectionDigest(resolvedConnection);
          if (connectionRef.sha256 !== recomputedConnectionDigest) {
            problems.push(`${at} payload.workspace_connection_ref.sha256 "${connectionRef.sha256}" does not equal the resolved connection's own recomputed content digest "${recomputedConnectionDigest}"; a pinned reference names the exact composed version, never a bare id/reference an independent resolver could satisfy with different content under the same label`);
          } else {
            connection = resolvedConnection;
          }
        }
      }
    }
    if (connection) {
      if (!sameScope(scope, connection.scope)) {
        problems.push(`${at} scope does not match the resolved payload.workspace_connection_ref record's scope; a qualification cannot claim a different scope than the connection scan it pins`);
      }
      const connectionContainer = {
        schema_version: 1,
        registry_id: 'existing-project-compatibility-mode',
        title: `${id} — composed workspace connection`,
        workspace_connections: [connection],
      };
      const connectionProblems = evaluateExistingProjectCompatibilityMode(connectionContainer, compatOptions);
      problems.push(...connectionProblems.map((p) => `${at} workspace_connection_ref: ${p}`));
    }

    /* --- migration_plan_ref: null, or a pinned + fingerprint-checked plan. --- */
    let migrationPlan = null;
    if (migrationRef !== null) {
      if (!isObject(migrationRef)) {
        problems.push(`${at} payload.migration_plan_ref is neither an object nor null`);
      } else {
        checkPinnedRefShape(problems, at, 'payload.migration_plan_ref', migrationRef);
        if (typeof migrationRef.reference === 'string' && migrationRef.reference !== '') {
          const resolvedPlan = resolveNamedRecord(problems, at, 'payload.migration_plan_ref', migrationRef, resolvePlanRecord, 'instance-migration-plan');
          if (resolvedPlan) {
            const recomputedFingerprint = computePlanFingerprint(isObject(resolvedPlan.payload) ? resolvedPlan.payload : {});
            if (migrationRef.sha256 !== recomputedFingerprint) {
              problems.push(`${at} payload.migration_plan_ref.sha256 "${migrationRef.sha256}" does not equal the resolved plan's own recomputed plan_fingerprint "${recomputedFingerprint}"; a pinned reference names the exact composed version, never a bare id an independent resolver could satisfy with a differently-fingerprinted plan of the same id`);
            } else {
              migrationPlan = resolvedPlan;
            }
          }
        }
      }
    }
    if (migrationPlan) {
      if (!sameScope(scope, migrationPlan.scope)) {
        problems.push(`${at} scope does not match the resolved payload.migration_plan_ref record's scope; a composed migration plan must describe the same workspace or repository as the connection it is qualified alongside`);
      }
      const migrationContainer = {
        schema_version: 1,
        registry_id: 'instance-data-migration',
        title: `${id} — composed migration plan`,
        migration_plans: [migrationPlan],
      };
      const migrationProblems = evaluateInstanceDataMigration(migrationContainer, migrationOptions);
      problems.push(...migrationProblems.map((p) => `${at} migration_plan_ref: ${p}`));
    }

    /* --- canonical_export_ref: null, or a pinned + fingerprint-checked export
       anchored to the SAME already-resolved migration plan. --- */
    let canonicalExport = null;
    if (exportRef !== null) {
      if (!isObject(exportRef)) {
        problems.push(`${at} payload.canonical_export_ref is neither an object nor null`);
      } else if (!migrationPlan) {
        problems.push(`${at} payload.canonical_export_ref is present without a resolvable payload.migration_plan_ref; an export proves a plan's own targets and cannot be composed without one`);
      } else {
        checkPinnedRefShape(problems, at, 'payload.canonical_export_ref', exportRef);
        if (typeof exportRef.reference === 'string' && exportRef.reference !== '') {
          const resolvedExport = resolveNamedRecord(problems, at, 'payload.canonical_export_ref', exportRef, resolveExportRecord, 'instance-canonical-export');
          if (resolvedExport) {
            const recomputedDigest = computeExportDigest(isObject(resolvedExport.payload) ? resolvedExport.payload : {});
            if (exportRef.sha256 !== recomputedDigest) {
              problems.push(`${at} payload.canonical_export_ref.sha256 "${exportRef.sha256}" does not equal the resolved export's own recomputed digest "${recomputedDigest}"; a pinned reference names the exact composed version, never a bare id`);
            } else {
              canonicalExport = resolvedExport;
            }
          }
        }
      }
    }
    if (canonicalExport && migrationPlan) {
      const exportPayload = isObject(canonicalExport.payload) ? canonicalExport.payload : {};
      if (exportPayload.plan_ref !== migrationPlan.id) {
        problems.push(`${at} payload.canonical_export_ref's resolved record names plan_ref "${exportPayload.plan_ref}", not "${migrationPlan.id}"; the composed export must prove the SAME pinned plan, not a different one`);
      }
      if (!sameScope(scope, canonicalExport.scope)) {
        problems.push(`${at} scope does not match the resolved payload.canonical_export_ref record's scope; a composed export must describe the same workspace or repository as the plan it proves`);
      }

      /*
       * The export's own plan boundary is derived HERE, from the plan this
       * evaluation already resolved and already fingerprint-pinned above —
       * never from exportOptions.resolveMigrationPlan (ignored). A resolver
       * map the caller configured independently, even one deliberately
       * mapping the same plan id to a DIFFERENT plan's content, cannot reach
       * this check: the only plan this derived resolver can ever return is
       * the one already anchored by migration_plan_ref.sha256.
       */
      const derivedPlanResolver = (queriedPlanRef) => {
        if (queriedPlanRef !== migrationPlan.id) return null;
        const mp = isObject(migrationPlan.payload) ? migrationPlan.payload : {};
        return {
          record_type: 'instance-migration-plan',
          plan_ref: migrationPlan.id,
          plan_fingerprint: computePlanFingerprint(mp),
          scope: migrationPlan.scope,
          source: mp.source,
          record_units: mp.record_units,
          mappings: mp.mappings,
        };
      };
      const exportContainer = {
        schema_version: 1,
        registry_id: 'instance-canonical-export',
        title: `${id} — composed canonical export`,
        exports: [canonicalExport],
      };
      const exportProblems = evaluateInstanceCanonicalExport(exportContainer, {
        registrySchema: exportOptions.registrySchema,
        envelopeSchema: exportOptions.envelopeSchema,
        resolveSourceContent: exportOptions.resolveSourceContent,
        resolveMigrationPlan: derivedPlanResolver,
      });
      problems.push(...exportProblems.map((p) => `${at} canonical_export_ref: ${p}`));
    }

    /* --- the recomputed decision-matrix verdict (see computeQualificationState). --- */
    const nextStep = connection && isObject(connection.payload) ? connection.payload.next_step : undefined;
    const overallStatus = migrationPlan && isObject(migrationPlan.payload) && isObject(migrationPlan.payload.verification)
      ? migrationPlan.payload.verification.overall_status : undefined;
    const expected = computeQualificationState({
      nextStep,
      hasUndecidedCandidates: hasUndecidedRuleCandidates(connection),
      hasMigrationPlan: Boolean(migrationPlan),
      migrationOverallStatus: overallStatus,
      mintsRecords: mintsAnyRecord(migrationPlan),
      hasCanonicalExport: Boolean(canonicalExport),
    });
    if (payload.qualification_state !== expected) {
      problems.push(`${at} qualification_state is "${payload.qualification_state}", but the closed decision matrix over the composed records' own next_step/pending rule-candidate decisions/verification.overall_status/canonical export coverage computes "${expected}"`);
    }

    /* --- the closed blockers/open_questions/qualification_state relation. --- */
    const blockers = Array.isArray(payload.blockers) ? payload.blockers : [];
    const openQuestions = Array.isArray(payload.open_questions) ? payload.open_questions : [];
    const isBlocked = payload.qualification_state === 'BLOCKED';
    const isUnverified = payload.qualification_state === 'UNVERIFIED';
    if (isBlocked !== (blockers.length > 0)) {
      problems.push(`${at} blockers is ${blockers.length ? 'non-empty' : 'empty'} but qualification_state is "${payload.qualification_state}"; blockers is non-empty exactly when qualification_state is "BLOCKED"`);
    }
    if (isUnverified !== (openQuestions.length > 0)) {
      problems.push(`${at} open_questions is ${openQuestions.length ? 'non-empty' : 'empty'} but qualification_state is "${payload.qualification_state}"; open_questions is non-empty exactly when qualification_state is "UNVERIFIED" — a verifiable, unresolved reason must be named, never a bare label`);
    }
  }

  return problems;
}
