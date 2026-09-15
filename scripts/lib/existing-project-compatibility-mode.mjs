// ---------------------------------------------------------------------------
// existing-project compatibility mode: the one checkable implementation
// ---------------------------------------------------------------------------
// registries/operating-model/existing-project-compatibility-mode.schema.json
// is the specialised payload schema for a workspace connection scan
// (record_type: workspace-connection-scan) — the product-neutral connection
// mode that discovers an existing project's instruction sources through an
// explicit, bounded discovery plan without ever writing to a file of the
// connected project. Scan DATA is Instance, like the source registry and the
// intake register: the Kernel ships the schema, the product-neutral fixtures
// and this module. scripts/kernel-validate.mjs (the
// `existing-project-compatibility-mode` section) and
// test/existing-project-compatibility-mode.test.mjs both call the functions
// here so the gate and the standalone set cannot drift apart.
//
// Nothing in this module WRITES. scanDiscoveryPlan reads a real directory
// (fs.statSync / fs.readFileSync only) to prove the zero-write guarantee is
// something the standalone test can actually exercise against real bytes, not
// a claim resting on hand-written fixtures alone. evaluateExistingProjectCompatibilityMode
// takes a parsed document plus schemas and returns a flat list of problem
// strings — empty means valid. Neither function touches process state or exits.
//
// Composition, not a competing format (properties 4/5 of the package brief):
//   - discovered_sources are FULL instruction-source-registry entries, checked
//     by calling the REAL evaluateInstructionSourceRegistry against the REAL
//     instruction-source-registry.schema.json the caller supplies — this
//     module carries no second copy of that contract's shape;
//   - rule_candidates[].candidate are FULL controlled-rule-intake rule-candidate
//     records, checked by calling the REAL evaluateControlledRuleIntake against
//     the REAL controlled-rule-intake.schema.json the caller supplies, resolved
//     through a source_ref resolver THIS module builds from the SAME scan's own
//     discovered_sources (never a second, external plumbing requirement) —
//     an id absent from discovered_sources, or a source_ref pinning a
//     revision/digest the discovered source no longer holds, is rejected there
//     exactly as controlled-rule-intake.md §2 already requires.
//
// Bounded discovery (property 3): payload.discovery_plan is a closed, explicit
// list of candidate locations — never a recursive guess over a project's tree.
// Every discovered source, missing-source record, unreadable-source record and
// location-bearing finding traces back to ONE discovery_plan slot by its
// stable id; a location outside the declared plan is rejected.
//
// Never a silent overwrite (property 7): a discovered source whose divergence
// is "changed" needs a matching "source-changed" finding; a missing source
// asserted previously_known needs a matching "source-missing" finding; every
// unreadable source needs a matching "source-unreadable" finding. A change
// that reaches no finding is a silent replace, and is rejected.
//
// Discovery mints no decision (property 5): a rule candidate whose
// discovery_status is "new" (parsed for the first time from a source THIS
// scan found) must stay applicability_state "candidate" — checked directly,
// on top of whatever the composed controlled-rule-intake evaluation already
// enforces. A "carried-over" candidate may hold a decided state, but only
// because it is independently re-checked by that same composed evaluation
// against the CURRENT discovered source, not because this module trusts it.
//
// Agent-native visibility (property 9) needs no new code here: a discovered
// source's own read_channel is part of the instruction-source-registry entry
// composed above, so agent-native / partial-or-none-visibility and the
// rejection of a claimed "full" visibility on that channel come from the
// REUSED instruction-source-registry.mjs check, not a duplicated one.
// ---------------------------------------------------------------------------
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { validate, assertSupportedDeep } from './json-schema.mjs';
import { checkLocationPath, MEDIA } from './instruction-source-registry.mjs';
import { evaluateInstructionSourceRegistry } from './instruction-source-registry.mjs';
import { evaluateControlledRuleIntake } from './controlled-rule-intake.mjs';

export { MEDIA };

export const RECORD_TYPE = 'workspace-connection-scan';
export const CONNECTION_MODES = ['compatibility', 'managed'];
export const SCAN_KINDS = ['initial', 'rescan'];
export const FINDING_KINDS = ['source-changed', 'source-missing', 'source-unreadable', 'conflict', 'ambiguous-scope', 'unresolved-decision', 'other'];
export const NEXT_STEPS = ['continue-compatibility-mode', 'resolve-conflict', 'resolve-ambiguity', 'await-owner-decision'];
export const DISCOVERY_STATUSES = ['new', 'carried-over'];

// The one area a scan may occupy: a bounded discovery plan concerns one
// project's workspace or one repository within it, never the built-in
// methodology, a personal or organisational profile, or an episodic run.
export const ALLOWED_SCOPE_TYPES = ['project-workspace', 'repository-scope'];

// The scan itself is carried out by a run: it never mints an owner decision on
// its own, so its envelope authority is always a run's delegated authority
// (property 5). Its envelope origin is always "declared" — an explicit act of
// connecting a project, never parsed out of another source ("derived") nor a
// built-in, imported or migrated record.
export const REQUIRED_ORIGIN_KIND = 'declared';
export const REQUIRED_AUTHORITY_KIND = 'delegated-run';

// The closed, minimal human owner authority that may authorise managed mode,
// one per scope this contract allows (workspace-scope-model.md §1). Managed
// mode never activates automatically (property 1): connection_mode "managed"
// requires a separate, verifiable managed_mode_decision the schema already
// makes mandatory; this map additionally pins WHOSE authority counts.
export const MANAGED_MODE_AUTHORITY_BY_SCOPE = {
  'project-workspace': 'project-owner',
  'repository-scope': 'repository-maintainer',
};

const isObject = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);

// An opaque identifier (container_ref, service_ref, resource_ref) must not be
// a disguised filesystem location. The same rule instruction-source-registry.mjs
// applies to its own location refs, restated here because that module does not
// export its private helper; a discovery plan slot is a distinct concept (a
// candidate location to check, not yet a registered source) and this is a
// small, self-contained string safety check, not a second source format.
function checkOpaqueRef(ref, label) {
  if (typeof ref !== 'string' || ref === '') return `${label} is empty`;
  if (ref.includes('\\') || /^[a-zA-Z]:/.test(ref) || ref.startsWith('/')) {
    return `${label} "${ref}" looks like an absolute machine path; it must be an opaque identifier, not a filesystem location`;
  }
  if (ref.split('/').some((s) => s === '..')) {
    return `${label} "${ref}" carries a ".." segment; it must be an opaque identifier`;
  }
  return null;
}

// One discovery_plan slot's location is well-formed: declared medium,
// normalised relative path (no absolute form, no ".." escape, no backslash
// separator) and opaque references only. Returns { ok: true } or
// { ok: false, reason }.
export function checkPlanLocation(slot) {
  if (!isObject(slot)) return { ok: false, reason: 'discovery plan slot is not an object' };
  if (!MEDIA.includes(slot.medium)) {
    return { ok: false, reason: `medium "${slot.medium}" is not one of ${MEDIA.join(', ')}; an undeclared medium is never scanned` };
  }
  if (slot.medium === 'file') {
    const v = checkLocationPath(slot.path);
    if (!v.ok) return v;
    const r = checkOpaqueRef(slot.container_ref, 'container_ref');
    if (r) return { ok: false, reason: r };
  } else {
    const r1 = checkOpaqueRef(slot.service_ref, 'service_ref');
    if (r1) return { ok: false, reason: r1 };
    const r2 = checkOpaqueRef(slot.resource_ref, 'resource_ref');
    if (r2) return { ok: false, reason: r2 };
  }
  return { ok: true };
}

// Does a discovered source's own payload (medium sibling to location, plus
// location itself) match the SAME location the plan slot declared? Compared
// field-for-field so a discovered source cannot silently drift to a location
// the plan never named.
function sameLocation(slot, sourcePayload) {
  if (!isObject(sourcePayload) || !isObject(sourcePayload.location)) return false;
  if ((slot.medium ?? null) !== (sourcePayload.medium ?? null)) return false;
  const loc = sourcePayload.location;
  return ['path', 'container_ref', 'service_ref', 'resource_ref']
    .every((f) => (slot[f] ?? null) === (loc[f] ?? null));
}

// The canonical portable reference this module derives for a discovered
// source, so a rule candidate's pinned payload.source_ref.reference is
// checked against something meaningful (id AND the currently held revision)
// rather than an unconstrained echo.
export function deriveSourceReference(id, revision) {
  return `instruction-source-registry:${id}@${revision}`;
}

// Build a resolveSource function (the shape controlled-rule-intake.mjs's
// evaluateControlledRuleIntake expects) FROM this scan's own discovered_sources
// — the external boundary controlled-rule-intake.md §2 requires, populated
// here from data the scan itself already carries rather than a second,
// separately wired registry. An id absent from discoveredById resolves to
// null and fails the candidate closed, exactly as an unknown source does in
// controlled-rule-intake.mjs.
export function buildSourceResolver(discoveredById) {
  return (sourceRef) => {
    if (!isObject(sourceRef) || typeof sourceRef.id !== 'string') return null;
    const src = discoveredById.get(sourceRef.id);
    if (!src || !isObject(src.payload) || !isObject(src.payload.recorded_state)) return null;
    const rs = src.payload.recorded_state;
    return {
      record_type: 'instruction-source',
      id: src.id,
      reference: deriveSourceReference(src.id, rs.revision),
      recorded_state: rs,
      read_channel: src.payload.read_channel,
    };
  };
}

// The ONE closed priority next_step is a deterministic function of: a
// blocking "conflict" finding outranks a blocking "ambiguous-scope" finding,
// which outranks any other blocking finding, which outranks having no
// blocking finding at all. Built entirely from Array#some over the findings
// set, so the same set computes the same value regardless of array order.
export function computeNextStep(findings) {
  const list = Array.isArray(findings) ? findings.filter(isObject) : [];
  const blocking = list.filter((f) => f.blocking === true);
  if (blocking.some((f) => f.kind === 'conflict')) return 'resolve-conflict';
  if (blocking.some((f) => f.kind === 'ambiguous-scope')) return 'resolve-ambiguity';
  if (blocking.length > 0) return 'await-owner-decision';
  return 'continue-compatibility-mode';
}

// Expected identity anchors for the three composed REGISTRY-shaped schemas
// (this module's own registrySchema, plus the instruction-source-registry and
// controlled-rule-intake contracts it composes with). All three share one
// shape: a top-level $id, a properties.registry_id.const naming the registry,
// one closed entries array whose items $ref #/definitions/entry, and that
// entry definition's own record_type.const. Checking all four anchors is what
// tells a genuine (but WRONG) registry schema apart from the expected one —
// a mere $id string match alone would not catch a schema copy-pasted with a
// stale $id, and a registry_id match alone would not catch a swapped, merely
// same-shaped, contract.
const REGISTRY_SCHEMA_IDENTITY = {
  registrySchema: {
    id: 'https://meridian.invalid/registries/operating-model/existing-project-compatibility-mode.schema.json',
    registryId: 'existing-project-compatibility-mode',
    entriesField: 'workspace_connections',
    entryRecordType: RECORD_TYPE,
  },
  sourceRegistrySchema: {
    id: 'https://meridian.invalid/registries/operating-model/instruction-source-registry.schema.json',
    registryId: 'instruction-source-registry',
    entriesField: 'instruction_sources',
    entryRecordType: 'instruction-source',
  },
  ruleIntakeSchema: {
    id: 'https://meridian.invalid/registries/operating-model/controlled-rule-intake.schema.json',
    registryId: 'controlled-rule-intake',
    entriesField: 'rule_candidates',
    entryRecordType: 'rule-candidate',
  },
};

// envelopeSchema (scoped-record.schema.json) is the one composed dependency
// that is NOT itself a registry — it is the shared per-entry envelope reused
// by every registry's entries, so it carries no registry_id and no single
// record_type.const. Its identity anchors are instead its own $id and the
// full required/properties shape of the envelope contract.
const ENVELOPE_SCHEMA_ID = 'https://meridian.invalid/registries/operating-model/scoped-record.schema.json';
const ENVELOPE_REQUIRED_FIELDS = ['schema_version', 'id', 'title', 'record_type', 'scope', 'origin', 'authority', 'payload'];
const ENVELOPE_STRUCTURAL_PROPERTIES = ['record_type', 'scope', 'origin', 'authority', 'payload'];

// A composed dependency schema must be: an object; ENTIRELY supported by the
// validation engine regardless of what this document happens to contain
// (assertSupportedDeep walks every branch, including one no fixture or
// document here ever takes — validate() alone only asserts the shallow
// keyword set of a node it actually visits, so a document that never
// populates, say, rule_candidates would otherwise let an unsupported keyword
// sitting in ruleIntakeSchema's rule_candidates branch through unnoticed);
// and the EXPECTED contract — {} and any other object satisfy "is an object"
// and pass every keyword the engine implements trivially (there is nothing to
// reject), so identity is checked on top, never inferred from mere
// applicability. Returns null (schema accepted) or a reason string.
function checkRegistrySchemaIdentity(schema, label, spec) {
  if (!isObject(schema)) return `${label} is not an object`;
  try { assertSupportedDeep(schema, label); }
  catch (e) { return `${label} is not fully supported by the validation engine: ${e.message}`; }
  if (schema.$id !== spec.id) {
    return `${label} has $id ${JSON.stringify(schema.$id ?? null)}, not the expected "${spec.id}"; this is not the ${spec.registryId} contract schema ({} and a foreign registry's schema are both rejected here)`;
  }
  const registryIdConst = schema.properties?.registry_id?.const;
  if (registryIdConst !== spec.registryId) {
    return `${label} properties.registry_id.const is ${JSON.stringify(registryIdConst ?? null)}, not "${spec.registryId}"`;
  }
  const entriesProp = schema.properties?.[spec.entriesField];
  if (!isObject(entriesProp) || entriesProp.type !== 'array' || entriesProp.items?.$ref !== '#/definitions/entry') {
    return `${label} properties.${spec.entriesField} is not declared as an array of "#/definitions/entry"; this is not the ${spec.registryId} contract's entries array`;
  }
  const entryRecordTypeConst = schema.definitions?.entry?.properties?.record_type?.const;
  if (entryRecordTypeConst !== spec.entryRecordType) {
    return `${label} definitions.entry.properties.record_type.const is ${JSON.stringify(entryRecordTypeConst ?? null)}, not "${spec.entryRecordType}"; this is not the ${spec.registryId} contract's entry definition`;
  }
  return null;
}

// Same purpose as checkRegistrySchemaIdentity, for the one non-registry
// composed dependency (see ENVELOPE_SCHEMA_ID comment above).
function checkEnvelopeSchemaIdentity(schema, label) {
  if (!isObject(schema)) return `${label} is not an object`;
  try { assertSupportedDeep(schema, label); }
  catch (e) { return `${label} is not fully supported by the validation engine: ${e.message}`; }
  if (schema.$id !== ENVELOPE_SCHEMA_ID) {
    return `${label} has $id ${JSON.stringify(schema.$id ?? null)}, not the expected "${ENVELOPE_SCHEMA_ID}"; this is not the record-envelope contract schema ({} and a foreign schema are both rejected here)`;
  }
  const required = Array.isArray(schema.required) ? schema.required : [];
  const missingRequired = ENVELOPE_REQUIRED_FIELDS.filter((f) => !required.includes(f));
  if (missingRequired.length) {
    return `${label} required is missing ${missingRequired.join(', ')}; this is not the full record-envelope contract`;
  }
  const missingProps = ENVELOPE_STRUCTURAL_PROPERTIES.filter((f) => !isObject(schema.properties?.[f]));
  if (missingProps.length) {
    return `${label} properties is missing ${missingProps.join(', ')}; this is not the full record-envelope contract`;
  }
  return null;
}

// The whole composition pipeline for one existing-project-compatibility-mode
// document: container + payload schema, per-entry envelope schema, then the
// cross-cutting rules the JSON Schema subset cannot state — bounded discovery,
// finding-on-change, discovery-mints-no-decision, and delegation of the
// discovered sources and the rule candidates to the ACTUAL registered
// contracts they compose with.
export function evaluateExistingProjectCompatibilityMode(doc, {
  registrySchema, envelopeSchema, sourceRegistrySchema, ruleIntakeSchema,
} = {}) {
  // The four composition dependencies are a mandatory part of this contract,
  // independent of the document's own content: whether discovered_sources or
  // rule_candidates happen to be populated never decides whether their
  // composed schema is required, checked, or genuine. Each is confirmed to be
  // an object, entirely supported by the validation engine (including
  // branches this particular document never visits), AND the schema of its
  // expected contract — never {}, never another registry's schema, however
  // well-formed. Any failure on any of the four fails the whole evaluation
  // closed, before the document itself is even inspected.
  const schemaFailures = {
    registrySchema: checkRegistrySchemaIdentity(registrySchema, 'registrySchema', REGISTRY_SCHEMA_IDENTITY.registrySchema),
    envelopeSchema: checkEnvelopeSchemaIdentity(envelopeSchema, 'envelopeSchema'),
    sourceRegistrySchema: checkRegistrySchemaIdentity(sourceRegistrySchema, 'sourceRegistrySchema', REGISTRY_SCHEMA_IDENTITY.sourceRegistrySchema),
    ruleIntakeSchema: checkRegistrySchemaIdentity(ruleIntakeSchema, 'ruleIntakeSchema', REGISTRY_SCHEMA_IDENTITY.ruleIntakeSchema),
  };
  const failingKeys = Object.keys(schemaFailures).filter((k) => schemaFailures[k] !== null);
  if (failingKeys.length) {
    const detail = failingKeys.map((k) => schemaFailures[k]).join('; ');
    return [`existing-project-compatibility-mode composition requires the real ${failingKeys.join(', ')} contract schema(s); discovered_sources compose with the instruction-source-registry contract and rule_candidates compose with the controlled-rule-intake contract regardless of whether either array is populated in this document, and an absent, inapplicable, unsupported or wrong-contract ({} included) schema is rejected closed, never silently skipped: ${detail}`];
  }

  const problems = [];

  const containerErrs = [];
  try { validate(doc, registrySchema, registrySchema, '', containerErrs); }
  catch (e) { return [`container/payload schema could not be applied: ${e.message}`]; }
  problems.push(...containerErrs);

  const entries = Array.isArray(doc && doc.workspace_connections) ? doc.workspace_connections : [];

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
    const at = `workspace connection scan "${id}"`;
    if (typeof e.id === 'string') {
      if (seenIds.has(e.id)) problems.push(`${at} is declared more than once`);
      seenIds.add(e.id);
    }
    if (e.record_type !== RECORD_TYPE) {
      problems.push(`${at} declares record_type "${e.record_type}", not "${RECORD_TYPE}"`);
    }

    const scope = isObject(e.scope) ? e.scope : {};
    if (!ALLOWED_SCOPE_TYPES.includes(scope.type)) {
      problems.push(`${at} scope.type is "${scope.type}"; a workspace connection scan is scoped to project-workspace or repository-scope only`);
    }
    const origin = isObject(e.origin) ? e.origin : {};
    if (origin.kind !== REQUIRED_ORIGIN_KIND) {
      problems.push(`${at} origin.kind is "${origin.kind}", not "${REQUIRED_ORIGIN_KIND}"; a scan is an explicit declared act of connecting a project`);
    }
    const authority = isObject(e.authority) ? e.authority : {};
    if (authority.kind !== REQUIRED_AUTHORITY_KIND) {
      problems.push(`${at} authority.kind is "${authority.kind}", not "${REQUIRED_AUTHORITY_KIND}"; the scan itself is carried out by a run's delegated authority and never mints an owner decision`);
    }

    const payload = isObject(e.payload) ? e.payload : {};

    // Property 6: exact workspace AND repository scope, checked against the
    // record's own envelope scope, not merely declared free-floating.
    const repo = isObject(payload.repository) ? payload.repository : {};
    if (scope.type === 'project-workspace') {
      if (repo.workspace_id !== scope.id) {
        problems.push(`${at} payload.repository.workspace_id "${repo.workspace_id}" does not match scope.id "${scope.id}"; scope names the exact workspace this scan covers`);
      }
    } else if (scope.type === 'repository-scope') {
      if (repo.workspace_id !== scope.workspace_id) {
        problems.push(`${at} payload.repository.workspace_id "${repo.workspace_id}" does not match scope.workspace_id "${scope.workspace_id}"`);
      }
      if (repo.id !== scope.id) {
        problems.push(`${at} payload.repository.id "${repo.id}" does not match scope.id "${scope.id}"; scope names the exact repository this scan covers`);
      }
    }

    // Property 1: managed mode never activates automatically.
    if (payload.connection_mode === 'managed') {
      const dec = payload.managed_mode_decision;
      if (!isObject(dec)) {
        problems.push(`${at} connection_mode is "managed" with no managed_mode_decision; managed mode never activates automatically and requires a separate, verifiable owner decision`);
      } else {
        const required = MANAGED_MODE_AUTHORITY_BY_SCOPE[scope.type];
        const decAuthority = isObject(dec.authority) ? dec.authority : {};
        if (required && decAuthority.kind !== required) {
          problems.push(`${at} managed_mode_decision.authority.kind is "${decAuthority.kind}", but scope "${scope.type}" requires owner authority "${required}"`);
        }
      }
    }

    const scanKind = payload.scan_kind;

    // Property 3: bounded discovery plan.
    const plan = Array.isArray(payload.discovery_plan) ? payload.discovery_plan : [];
    const planIds = new Set();
    for (const slot of plan) {
      if (!isObject(slot)) continue;
      if (typeof slot.id === 'string') {
        if (planIds.has(slot.id)) problems.push(`${at} discovery_plan slot id "${slot.id}" is declared more than once`);
        planIds.add(slot.id);
      }
      const v = checkPlanLocation(slot);
      if (!v.ok) problems.push(`${at} discovery_plan slot "${slot.id}": ${v.reason}`);
    }
    const planById = new Map(plan.filter(isObject).filter((s) => typeof s.id === 'string').map((s) => [s.id, s]));

    // Property 4: discovered sources compose with the ACTUAL
    // instruction-source-registry contract, never a copy of its shape.
    const discovered = Array.isArray(payload.discovered_sources) ? payload.discovered_sources : [];
    const discoveredById = new Map();
    if (discovered.length) {
      const container = {
        schema_version: 1,
        registry_id: 'instruction-source-registry',
        title: `${id} — discovered sources`,
        instruction_sources: discovered,
      };
      const srcProblems = evaluateInstructionSourceRegistry(container, { registrySchema: sourceRegistrySchema, envelopeSchema });
      problems.push(...srcProblems.map((p) => `${at} discovered source: ${p}`));
    }
    for (const src of discovered) {
      if (!isObject(src)) continue;
      const sid = typeof src.id === 'string' ? src.id : undefined;
      if (sid) discoveredById.set(sid, src);
      if (!sid || !planById.has(sid)) {
        problems.push(`${at} discovered source "${sid}" does not correspond to any declared discovery_plan slot; discovery is bounded to the declared plan and never guesses beyond it`);
        continue;
      }
      const slot = planById.get(sid);
      if (!sameLocation(slot, src.payload)) {
        problems.push(`${at} discovered source "${sid}" location does not match its declared discovery_plan slot "${sid}"`);
      }
      const divStatus = isObject(src.payload) && isObject(src.payload.divergence) ? src.payload.divergence.status : undefined;
      if (divStatus === 'source-missing' || divStatus === 'source-unreadable') {
        problems.push(`${at} discovered source "${sid}" declares divergence.status "${divStatus}"; a source currently missing or unreadable belongs in missing_sources/unreadable_sources, not discovered_sources`);
      } else if (scanKind === 'initial' && divStatus !== 'unknown') {
        problems.push(`${at} discovered source "${sid}" declares divergence.status "${divStatus}" on an initial scan, which has no prior baseline to compare against; an initial scan's divergence is always "unknown"`);
      }
    }

    // Property 7 (bucket separation + previously_known gate): missing and
    // unreadable sources trace back to a declared plan slot, and an initial
    // scan can never assert previously_known.
    const missing = Array.isArray(payload.missing_sources) ? payload.missing_sources : [];
    for (const m of missing) {
      if (!isObject(m)) continue;
      if (!planById.has(m.plan_id)) {
        problems.push(`${at} missing_sources plan_id "${m.plan_id}" does not correspond to any declared discovery_plan slot`);
      }
      if (scanKind === 'initial' && m.previously_known === true) {
        problems.push(`${at} missing_sources plan_id "${m.plan_id}" asserts previously_known on an initial scan, which has no prior scan to know from`);
      }
    }
    const unreadable = Array.isArray(payload.unreadable_sources) ? payload.unreadable_sources : [];
    for (const u of unreadable) {
      if (!isObject(u)) continue;
      if (!planById.has(u.plan_id)) {
        problems.push(`${at} unreadable_sources plan_id "${u.plan_id}" does not correspond to any declared discovery_plan slot`);
      }
      if (scanKind === 'initial' && u.previously_known === true) {
        problems.push(`${at} unreadable_sources plan_id "${u.plan_id}" asserts previously_known on an initial scan, which has no prior scan to know from`);
      }
    }

    // Property 3/7: discovery_plan outcomes form a COMPLETE, EXCLUSIVE
    // partition — every declared slot has exactly one outcome across
    // discovered_sources/missing_sources/unreadable_sources, never zero
    // (omitted), never more than one (the same slot appearing in two of the
    // three arrays, or twice within one of them). Counted by plan id, never
    // by array position, so this holds regardless of array order.
    {
      const outcomeCounts = new Map();
      const bumpOutcome = (planId) => {
        if (typeof planId !== 'string' || !planById.has(planId)) return;
        outcomeCounts.set(planId, (outcomeCounts.get(planId) || 0) + 1);
      };
      for (const src of discovered) { if (isObject(src)) bumpOutcome(src.id); }
      for (const m of missing) { if (isObject(m)) bumpOutcome(m.plan_id); }
      for (const u of unreadable) { if (isObject(u)) bumpOutcome(u.plan_id); }
      for (const planId of planById.keys()) {
        const count = outcomeCounts.get(planId) || 0;
        if (count === 0) {
          problems.push(`${at} discovery_plan slot "${planId}" has no outcome in discovered_sources/missing_sources/unreadable_sources; every declared slot requires exactly one`);
        } else if (count > 1) {
          problems.push(`${at} discovery_plan slot "${planId}" has ${count} outcomes across discovered_sources/missing_sources/unreadable_sources; exactly one is required per slot, whether the extras overlap two different arrays or repeat within one`);
        }
      }
    }

    // Property 7: a change never reaches no finding — never a silent overwrite.
    const findings = Array.isArray(payload.findings) ? payload.findings : [];
    const findingKey = (kind, planId) => `${kind}::${planId ?? ''}`;
    const findingKeys = new Set(findings.filter(isObject).map((f) => findingKey(f.kind, f.plan_id)));
    for (const src of discovered) {
      if (!isObject(src) || typeof src.id !== 'string') continue;
      const divStatus = isObject(src.payload) && isObject(src.payload.divergence) ? src.payload.divergence.status : undefined;
      if (divStatus === 'changed' && !findingKeys.has(findingKey('source-changed', src.id))) {
        problems.push(`${at} discovered source "${src.id}" changed with no matching "source-changed" finding for plan_id "${src.id}"; a change is never silently absorbed into a rewritten snapshot`);
      }
    }
    for (const m of missing) {
      if (!isObject(m) || m.previously_known !== true) continue;
      if (!findingKeys.has(findingKey('source-missing', m.plan_id))) {
        problems.push(`${at} missing_sources plan_id "${m.plan_id}" is previously_known with no matching "source-missing" finding`);
      }
    }
    for (const u of unreadable) {
      if (!isObject(u)) continue;
      if (!findingKeys.has(findingKey('source-unreadable', u.plan_id))) {
        problems.push(`${at} unreadable_sources plan_id "${u.plan_id}" has no matching "source-unreadable" finding`);
      }
    }

    // The final permitted next step is the ONE value the closed priority
    // (computeNextStep) computes from the findings set — never a looser
    // two-way blocking/non-blocking split. The same findings set, in any
    // order, always computes the same next_step.
    const expectedNextStep = computeNextStep(findings);
    if (payload.next_step !== expectedNextStep) {
      problems.push(`${at} next_step is "${payload.next_step}", but the declared priority (a blocking "conflict" finding requires "resolve-conflict"; else a blocking "ambiguous-scope" finding requires "resolve-ambiguity"; else any other blocking finding requires "await-owner-decision"; else "continue-compatibility-mode") computes "${expectedNextStep}"`);
    }

    // Property 5: discovery, a snapshot or a rescan mints no decision. Rule
    // candidates compose with the ACTUAL controlled-rule-intake contract,
    // resolved through a boundary built from this SAME scan's discovered
    // sources — never a copy of that contract's shape.
    const wrappers = Array.isArray(payload.rule_candidates) ? payload.rule_candidates : [];
    const candidateEntries = wrappers.filter(isObject).map((w) => w.candidate).filter(isObject);
    if (candidateEntries.length) {
      const container = {
        schema_version: 1,
        registry_id: 'controlled-rule-intake',
        title: `${id} — rule candidates`,
        rule_candidates: candidateEntries,
      };
      const resolveSource = buildSourceResolver(discoveredById);
      const criProblems = evaluateControlledRuleIntake(container, { registrySchema: ruleIntakeSchema, envelopeSchema, resolveSource });
      problems.push(...criProblems.map((p) => `${at} rule candidate: ${p}`));
    }
    for (const w of wrappers) {
      if (!isObject(w)) continue;
      const c = isObject(w.candidate) ? w.candidate : {};
      const cid = typeof c.id === 'string' ? c.id : '?';
      const state = isObject(c.payload) ? c.payload.applicability_state : undefined;
      if (w.discovery_status === 'new' && state !== 'candidate') {
        problems.push(`${at} rule candidate "${cid}" has discovery_status "new" but applicability_state "${state}"; a candidate first found by THIS scan can never itself be accepted or not-applicable (property 5)`);
      }
    }
  }

  return problems;
}

// containerRoot must itself be a genuine, non-empty path. Returns
// { ok: true } or { ok: false, reason }.
function checkContainerRoot(containerRoot) {
  if (typeof containerRoot !== 'string' || containerRoot.trim() === '') {
    return { ok: false, reason: 'containerRoot is not a non-empty string' };
  }
  return { ok: true };
}

// A pure, read-only directory scan proving the zero-write guarantee is real:
// only fs.lstatSync, fs.realpathSync, fs.statSync and fs.readFileSync are
// used, never a write, rename or delete.
//
// The file boundary is closed BEFORE any stat/read of the slot's own
// candidate path, in this order, for every "file" slot:
//   1. the slot itself passes checkPlanLocation (declared medium, normalised
//      relative path, opaque references) — an invalid slot is rejected
//      explicitly as status "invalid" and is never classified as "missing";
//   2. containerRoot is a genuine non-empty path;
//   3. the candidate, resolved against the NORMALISED ABSOLUTE root, is
//      proven — by string containment (path.relative) — to still sit inside
//      that root; a slot whose declared path cannot pass this (it cannot,
//      once checkPlanLocation already forbids "..", an absolute form and a
//      backslash separator, but the containment is proven again here rather
//      than merely assumed from step 1) is rejected as "invalid";
//   4. once something exists at the candidate path (lstat, not following the
//      final symlink), the FULLY RESOLVED real path of both the candidate and
//      the root are compared: a candidate whose real target — through a
//      symlink anywhere along the path, not only the final segment — falls
//      outside the root's real path is rejected as "invalid", never read.
//
// Only after all four hold does this function stat and read the candidate.
// medium "external-service" and any other declared medium never reach the
// filesystem at all here — an adapter supplies their state separately; they
// report as unreadable with a stated reason. This is the WHOLE guarantee the
// standard document may claim for this function: it does not defend against
// a root replaced between calls (TOCTOU) or a filesystem mutated by another
// process during the scan.
export function scanDiscoveryPlan(discoveryPlan, { containerRoot } = {}) {
  const results = [];
  for (const slot of Array.isArray(discoveryPlan) ? discoveryPlan : []) {
    if (!isObject(slot) || typeof slot.id !== 'string') continue;

    const locationVerdict = checkPlanLocation(slot);
    if (!locationVerdict.ok) {
      results.push({ planId: slot.id, status: 'invalid', reason: locationVerdict.reason });
      continue;
    }

    if (slot.medium !== 'file') {
      results.push({ planId: slot.id, status: 'unreadable', reason: 'external-service-not-scanned-locally' });
      continue;
    }

    const rootVerdict = checkContainerRoot(containerRoot);
    if (!rootVerdict.ok) {
      results.push({ planId: slot.id, status: 'invalid', reason: rootVerdict.reason });
      continue;
    }

    const normalizedRoot = path.resolve(containerRoot);
    const candidate = path.resolve(normalizedRoot, slot.path);
    const relFromRoot = path.relative(normalizedRoot, candidate);
    if (relFromRoot === '' || relFromRoot === '..' || relFromRoot.startsWith(`..${path.sep}`) || path.isAbsolute(relFromRoot)) {
      results.push({ planId: slot.id, status: 'invalid', reason: 'resolved candidate escapes containerRoot' });
      continue;
    }

    let rootReal;
    try { rootReal = fs.realpathSync(normalizedRoot); }
    catch { results.push({ planId: slot.id, status: 'invalid', reason: 'containerRoot does not resolve to a real directory' }); continue; }

    try { fs.lstatSync(candidate); }
    catch { results.push({ planId: slot.id, status: 'missing' }); continue; }

    let candidateReal;
    try { candidateReal = fs.realpathSync(candidate); }
    catch (e) { results.push({ planId: slot.id, status: 'unreadable', reason: e.message }); continue; }

    const withinRoot = candidateReal === rootReal || candidateReal.startsWith(`${rootReal}${path.sep}`);
    if (!withinRoot) {
      results.push({ planId: slot.id, status: 'invalid', reason: 'resolved candidate escapes containerRoot via a symbolic link' });
      continue;
    }

    let stat;
    try { stat = fs.statSync(candidate); }
    catch (e) { results.push({ planId: slot.id, status: 'unreadable', reason: e.message }); continue; }
    if (!stat.isFile()) {
      results.push({ planId: slot.id, status: 'unreadable', reason: 'not-a-regular-file' });
      continue;
    }
    let bytes;
    try { bytes = fs.readFileSync(candidate); }
    catch (e) { results.push({ planId: slot.id, status: 'unreadable', reason: e.message }); continue; }
    const digest = createHash('sha256').update(bytes).digest('hex');
    results.push({ planId: slot.id, status: 'discovered', revision: stat.mtime.toISOString(), digest });
  }
  return results;
}
