#!/usr/bin/env node
// Standalone verification for the evidence-and-handoff contract
// (registries/operating-model/evidence-and-handoff.schema.json).
//
// It calls the same implementation the gate calls
// (scripts/lib/evidence-and-handoff.mjs) so the two cannot drift: the canonical
// scoped-record envelope (validated separately and always), the complete
// specialised schema a record declares in its own $schema, and the rules JSON
// Schema cannot state — the portable, logically-resolved $schema declaration;
// the DETERMINISTIC single-run link (four closed structured pinned references,
// scope.id == execution_run_ref.id by exact comparison, human_control_ref and
// context_manifest_ref bound to the same run, task_specification_ref carrying no
// run_id); the CLOSED exact-revision rule reused from context-manifest.mjs,
// applied to every pinned reference, to source_state / result_state revisions
// AND to every evidence entry; claimed results, verifiable assertions and
// evidence as three separate id-linked lists where a verified assertion needs a
// RESOLVED, pin-checked covering evidence entry whose observed_result is
// "confirmed", and a claimed result's status is derived on BOTH sides; the
// three distinct mandatory-check statuses, each passed / failed check backed by
// a resolved result-evidence entry, and "unable" never a pass; acceptance-
// criteria coverage closed against the resolved task specification; source_state
// and result_state pinning the same repository set; the closed worktree-
// disposition set aligned with version-control-flow.md §5.4; and outcome.status
// "complete" meaning the WHOLE run is finished (resolved run work_status
// "completed", null next step, empty open gaps, every criterion covered by a
// verified assertion).
//
// Usage: node test/evidence-and-handoff.test.mjs

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { assertSupportedDeep, UnsupportedSchema, validate } from '../scripts/lib/json-schema.mjs';
import {
  evaluateEvidenceAndHandoff,
  makeRecordResolver,
  nonPortableReason,
  resolveSchemaRef,
  classifyRevision,
  RECORD_TYPE,
  ALLOWED_SCOPE_TYPE,
  CANONICAL_RECORD_BASE,
  EXPECTED_SCHEMA_BASENAME,
  EXPECTED_SCHEMA_REF,
  PINNED_REF_SLOTS,
  RESOLVED_ENTRY_KEYS_BY_TYPE,
  EVIDENCE_KINDS,
  CHECK_STATUSES,
  OBSERVED_RESULTS,
  ACCEPTANCE_STATUSES,
  WORKTREE_STATES,
  OUTCOME_STATUSES,
  FORBIDDEN_PAYLOAD_FIELDS,
  LIFECYCLE_STAGES,
  WORK_STATUSES,
} from '../scripts/lib/evidence-and-handoff.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
let passed = 0;
const failures = [];

function check(name, fn) {
  try {
    fn();
    passed++;
    console.log(`PASS ${name}`);
  } catch (error) {
    failures.push(`${name}: ${error.message}`);
    console.log(`FAIL ${name}: ${error.message}`);
  }
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

const loadText = (rel) => fs.readFileSync(path.join(root, rel), 'utf8');
const loadJson = (rel) => JSON.parse(loadText(rel));
const clone = (x) => JSON.parse(JSON.stringify(x));

const recordSchema = loadJson('registries/operating-model/evidence-and-handoff.schema.json');
const envelopeSchema = loadJson('registries/operating-model/scoped-record.schema.json');
const fixtures = loadJson('registries/operating-model/fixtures/evidence-and-handoff.fixtures.json');

// The record resolver is EXTERNAL to every record under test: it is built from
// the bundle's companion `resolution` set, never from fields inside the record.
const resolveRecords = makeRecordResolver(fixtures.resolution);
const opts = { recordSchema, envelopeSchema, resolveRecords };
const evaluate = (doc) => evaluateEvidenceAndHandoff(doc, opts);
const rejectsBecause = (doc, re) => evaluate(doc).some((m) => re.test(m));
// A resolver from the real set with entries mutated in place.
const resolverWith = (mut) => { const m = clone(fixtures.resolution); mut(m); return makeRecordResolver(m); };
const evalWith = (doc, resolver) => evaluateEvidenceAndHandoff(doc, { recordSchema, envelopeSchema, resolveRecords: resolver });

// A topologically complete valid handoff record, the base for targeted mutations.
const BASE = clone(fixtures.valid[0].spec);
const COMPLETE = clone(fixtures.valid.find((c) => c.spec.payload.outcome.status === 'complete').spec);
const payload = (d) => d.payload;
const mutate = (fn) => { const d = clone(BASE); fn(d); return d; };
const mutateComplete = (fn) => { const d = clone(COMPLETE); fn(d); return d; };

// ---------------------------------------------------------------------------
// schema shape
// ---------------------------------------------------------------------------
check('both schemas use the supported JSON Schema subset', () => {
  assertSupportedDeep(recordSchema, 'evidence-and-handoff.schema.json');
  assertSupportedDeep(envelopeSchema, 'scoped-record.schema.json');
});

check('an unsupported schema keyword is rejected up front (assertSupportedDeep)', () => {
  const bad = clone(recordSchema);
  bad.definitions.payload.patternProperties = { '^x': { type: 'string' } };
  let threw = null;
  try { assertSupportedDeep(bad, 'evidence-and-handoff.schema.json'); }
  catch (e) { threw = e; }
  assert(threw instanceof UnsupportedSchema, 'assertSupportedDeep did not reject the unsupported keyword');
});

check('the specialised schema COMPOSES the envelope with the body (one pass, one model)', () => {
  assert(recordSchema.properties.record_type.const === RECORD_TYPE, 'record_type is not pinned to evidence-and-handoff');
  assert(recordSchema.additionalProperties === false, 'the record schema is not a closed object');
  for (const k of ['$schema', 'schema_version', 'id', 'title', 'record_type', 'scope', 'origin', 'authority', 'payload']) {
    assert(recordSchema.required.includes(k), `the record schema does not require the envelope field "${k}"`);
  }
  assert(recordSchema.definitions.payload.additionalProperties === false, 'payload is not a closed object');
  for (const k of [
    'execution_run_ref', 'task_specification_ref', 'human_control_ref', 'context_manifest_ref',
    'outcome', 'claimed_results', 'verifiable_assertions', 'evidence', 'mandatory_checks', 'acceptance_criteria',
    'source_state', 'result_state', 'changed_paths', 'external_effects', 'deviations',
    'open_gaps', 'required_owner_decisions', 'blockers', 'worktree_disposition', 'next_step',
  ]) {
    assert(recordSchema.definitions.payload.required.includes(k), `payload does not require "${k}"`);
  }
  assert(recordSchema.definitions.pinned_ref.additionalProperties === false, 'pinned_ref is not a closed object');
  assert(JSON.stringify(recordSchema.definitions.pinned_ref.properties.record_type.enum)
    === JSON.stringify(['execution-run', 'task-specification', 'run-human-control', 'context-manifest']),
    'the pinned_ref record_type enum is not the expected four');
  assert(JSON.stringify(recordSchema.definitions.scope_ref.properties.type.enum) === JSON.stringify([ALLOWED_SCOPE_TYPE]),
    'the schema scope enum is not exactly {run-state}');
  assert(recordSchema.definitions.payload.properties.claimed_results.minItems === 1,
    'claimed_results is not required non-empty');
  assert(recordSchema.definitions.payload.properties.source_state.minItems === 1
    && recordSchema.definitions.payload.properties.result_state.minItems === 1,
    'source_state / result_state are not required non-empty');
  for (const k of ['revision', 'sha256']) {
    assert(k in recordSchema.definitions.evidence_entry.properties, `evidence_entry has no "${k}" property (a pinned evidence entry)`);
  }
  assert('result_evidence_id' in recordSchema.definitions.mandatory_check.properties, 'mandatory_check has no result_evidence_id');
  assert(recordSchema.definitions.acceptance_criterion_coverage.additionalProperties === false, 'acceptance_criterion_coverage is not closed');
});

check('the enums and slot maps line up with the library', () => {
  assert(JSON.stringify(Object.keys(PINNED_REF_SLOTS).sort())
    === JSON.stringify(['context_manifest_ref', 'execution_run_ref', 'human_control_ref', 'task_specification_ref']),
    'the pinned-reference slots are not the expected four');
  assert(PINNED_REF_SLOTS.human_control_ref.runId === 'required' && PINNED_REF_SLOTS.context_manifest_ref.runId === 'required'
    && PINNED_REF_SLOTS.task_specification_ref.runId === 'forbidden' && PINNED_REF_SLOTS.execution_run_ref.runId === 'self',
    'the pinned-reference run_id semantics are wrong');
  assert(RESOLVED_ENTRY_KEYS_BY_TYPE['task-specification'].includes('acceptance_criteria')
    && RESOLVED_ENTRY_KEYS_BY_TYPE['task-specification'].includes('mandatory_checks'),
    'the task-specification resolved entry needs acceptance_criteria and mandatory_checks');
  assert(RESOLVED_ENTRY_KEYS_BY_TYPE['evidence-result'].includes('observed_result')
    && RESOLVED_ENTRY_KEYS_BY_TYPE['evidence-result'].includes('covers')
    && RESOLVED_ENTRY_KEYS_BY_TYPE['evidence-result'].includes('check_ref')
    && RESOLVED_ENTRY_KEYS_BY_TYPE['evidence-result'].includes('specialised_contract')
    && RESOLVED_ENTRY_KEYS_BY_TYPE['evidence-result'].includes('recorded_verdict'),
    'the evidence-result resolved entry is missing a slot field');
  assert(JSON.stringify(recordSchema.definitions.evidence_entry.properties.kind.enum) === JSON.stringify(EVIDENCE_KINDS), 'evidence kind enum drift');
  assert(JSON.stringify(recordSchema.definitions.mandatory_check.properties.status.enum) === JSON.stringify(CHECK_STATUSES), 'check status enum drift');
  assert(JSON.stringify(recordSchema.definitions.acceptance_criterion_coverage.properties.status.enum) === JSON.stringify(ACCEPTANCE_STATUSES), 'acceptance status enum drift');
  assert(JSON.stringify(recordSchema.definitions.worktree_disposition.properties.state.enum) === JSON.stringify(WORKTREE_STATES), 'worktree state enum drift');
  assert(JSON.stringify(recordSchema.definitions.outcome.properties.status.enum) === JSON.stringify(OUTCOME_STATUSES), 'outcome status enum drift');
  assert(JSON.stringify(OBSERVED_RESULTS) === JSON.stringify(['confirmed', 'contradicted', 'inconclusive']), 'OBSERVED_RESULTS drift');
});

check('the implementation source carries no NUL or control byte', () => {
  const buf = fs.readFileSync(path.join(root, 'scripts/lib/evidence-and-handoff.mjs'));
  assert(buf.indexOf(0) === -1, 'a NUL byte is present in scripts/lib/evidence-and-handoff.mjs');
  for (let i = 0; i < buf.length; i++) {
    const b = buf[i];
    assert(b === 9 || b === 10 || b === 13 || b >= 32, `a control byte 0x${b.toString(16)} is present at offset ${i}`);
  }
});

// ---------------------------------------------------------------------------
// the declared $schema reaches the specialised contract, logically
// ---------------------------------------------------------------------------
check('every VALID fixture declares $schema resolving to the specialised schema', () => {
  fixtures.valid.forEach((c, i) => {
    assert(resolveSchemaRef(c.spec.$schema) === EXPECTED_SCHEMA_REF,
      `valid[${i}] (${c.note}) $schema does not resolve to ${EXPECTED_SCHEMA_REF}`);
  });
});

check('an absent, envelope-only, bare-basename, out-of-namespace or absolute $schema is rejected', () => {
  assert(rejectsBecause(mutate((x) => { delete x.$schema; }), /declares no \$schema/), 'absent $schema accepted');
  assert(rejectsBecause(mutate((x) => { x.$schema = '../../registries/operating-model/scoped-record.schema.json'; }), /resolves to the record envelope/), 'envelope-only $schema accepted');
  assert(rejectsBecause(mutate((x) => { x.$schema = EXPECTED_SCHEMA_BASENAME; }), /does not resolve to the logical address/), 'bare basename accepted');
  assert(rejectsBecause(mutate((x) => { x.$schema = `../../../registries/operating-model/${EXPECTED_SCHEMA_BASENAME}`; }), /does not resolve to the logical address/), 'out-of-namespace $schema accepted');
  assert(rejectsBecause(mutate((x) => { x.$schema = '/opt/meridian/registries/operating-model/evidence-and-handoff.schema.json'; }), /is not portable/), 'absolute $schema path accepted');
  assert(CANONICAL_RECORD_BASE === 'records/evidence-and-handoff', 'the canonical logical base changed unexpectedly');
});

// ---------------------------------------------------------------------------
// real fixtures
// ---------------------------------------------------------------------------
check('every valid fixture passes with no problem', () => {
  fixtures.valid.forEach((c, i) => {
    const p = evaluate(c.spec);
    assert(p.length === 0, `valid[${i}] (${c.note}) was rejected: ${p[0]}`);
  });
});

check('every invalid fixture produces at least one problem', () => {
  fixtures.invalid.forEach((c, i) => {
    assert(evaluate(c.spec).length > 0, `invalid[${i}] (${c.note}) was accepted`);
  });
});

check('the valid set exercises the required equivalence classes', () => {
  const statuses = new Set(fixtures.valid.map((c) => c.spec.payload.outcome.status));
  for (const s of ['complete', 'blocked', 'handed_off_incomplete']) {
    assert(statuses.has(s), `no valid fixture exercises outcome.status "${s}"`);
  }
  const wt = new Set(fixtures.valid.map((c) => c.spec.payload.worktree_disposition.state));
  for (const s of ['not_created', 'removed', 'retained']) {
    assert(wt.has(s), `no valid fixture exercises worktree state "${s}"`);
  }
  const checkStatuses = new Set(fixtures.valid.flatMap((c) => c.spec.payload.mandatory_checks.map((m) => m.status)));
  for (const s of ['passed', 'failed', 'unable']) {
    assert(checkStatuses.has(s), `no valid fixture exercises a mandatory check status "${s}"`);
  }
  const acStatuses = new Set(fixtures.valid.flatMap((c) => c.spec.payload.acceptance_criteria.map((a) => a.status)));
  assert(acStatuses.has('covered') && acStatuses.has('uncovered'), 'the valid set does not exercise both acceptance-criteria statuses');
  assert(fixtures.valid.some((c) => c.spec.payload.evidence.some((e) => e.kind === 'specialised-evidence-record')),
    'no valid fixture carries a specialised-evidence-record evidence entry');
  assert(fixtures.valid.some((c) => c.spec.payload.external_effects.length > 0), 'no valid fixture records an external effect');
  // three exact pin forms across the valid set (references, and evidence)
  const allPinned = fixtures.valid.flatMap((c) => [
    c.spec.payload.execution_run_ref, c.spec.payload.task_specification_ref,
    c.spec.payload.human_control_ref, c.spec.payload.context_manifest_ref,
    ...c.spec.payload.evidence,
  ]);
  assert(allPinned.some((r) => classifyRevision(r.revision) === 'exact' && /^[0-9a-f]{40}$/i.test(r.revision || '')), 'no valid input pinned by a full Git SHA');
  assert(allPinned.some((r) => classifyRevision(r.revision) === 'exact' && /^v?\d+\.\d+\.\d+$/.test(r.revision || '')), 'no valid input pinned by a strict SemVer tag');
  assert(allPinned.some((r) => !r.revision && typeof r.sha256 === 'string' && /^[0-9a-fA-F]{64}$/.test(r.sha256)), 'no valid input pinned by a lone SHA-256 digest');
});

// ---------------------------------------------------------------------------
// 1. deterministic single-run link
// ---------------------------------------------------------------------------
check('the four references are CLOSED structured pinned objects, never plain strings', () => {
  for (const f of ['execution_run_ref', 'task_specification_ref', 'human_control_ref', 'context_manifest_ref']) {
    assert(rejectsBecause(mutate((x) => { payload(x)[f] = 'run:example-run-eh-001'; }), /is not a structured pinned reference|expected object/),
      `a plain-string ${f} was accepted`);
    assert(evaluate(mutate((x) => { payload(x)[f].extra_body_field = 'x'; })).length > 0, `an extra key on ${f} (embedded body) was accepted`);
  }
});

check('scope.id must EQUAL execution_run_ref.id exactly — no substring or prefix match', () => {
  assert(rejectsBecause(mutate((x) => { x.scope.id = 'example-run-eh'; }), /the exact same string, not one a substring of the other/), 'a scope.id prefix was accepted');
  assert(rejectsBecause(mutate((x) => { x.scope.id = 'run-eh-001'; }), /the exact same string/), 'a scope.id substring was accepted');
});

check('human_control_ref and context_manifest_ref belong to the same run; task_specification_ref carries no run_id', () => {
  assert(rejectsBecause(mutate((x) => { payload(x).human_control_ref.run_id = 'example-run-eh-other'; }), /is not the referenced run|must belong to the same run/), 'a human_control_ref pointing at another run was accepted');
  assert(rejectsBecause(mutate((x) => { delete payload(x).context_manifest_ref.run_id; }), /context_manifest_ref carries no run_id/), 'a context_manifest_ref with no run_id was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).task_specification_ref.run_id = 'example-run-eh-001'; }), /is not run-scoped and names no run_id/), 'a task_specification_ref carrying a run_id was accepted');
});

// ---------------------------------------------------------------------------
// 2. the closed exact-revision rule, reused — for references, repository states
//    AND evidence entries
// ---------------------------------------------------------------------------
check('the SAME pin rule is applied to pinned references, repository states and evidence entries', () => {
  assert(rejectsBecause(mutate((x) => { payload(x).execution_run_ref.revision = 'feature/package-7'; }),
    /execution_run_ref is not pinned to an exact edition: revision "feature\/package-7" is a branch or channel reference/), 'a branch-pinned execution_run_ref was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).source_state = [{ repository_ref: 'repo:example-service', revision: 'release/0.6.0' }]; }),
    /source_state entry .* is not pinned to an exact revision: revision "release\/0.6.0" is a branch or channel reference/), 'a branch-pinned source_state repo was accepted');
  assert(rejectsBecause(mutate((x) => { delete payload(x).evidence[0].revision; }),
    /evidence entry unit-default is not pinned to an exact edition/), 'an unpinned evidence entry was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).evidence[0].revision = 'feature/x'; }),
    /evidence entry unit-default is not pinned to an exact edition: revision "feature\/x" is a branch or channel reference/), 'a branch-pinned evidence entry was accepted');
});

// ---------------------------------------------------------------------------
// 3. claims / assertions / evidence linkage, resolved through the boundary
// ---------------------------------------------------------------------------
check('CHANGES_REQUESTED 1 — a verified assertion needs a RESOLVED, pin-checked, confirming evidence entry', () => {
  // a plain, unpinned reference
  assert(rejectsBecause(mutate((x) => { payload(x).evidence[0].reference = 'reports/latest.json'; delete payload(x).evidence[0].revision; }),
    /evidence entry unit-default is not pinned to an exact edition/), 'a verified assertion covered only by an unpinned reference was accepted');
  // pinned but resolves to nothing
  assert(rejectsBecause(mutate((x) => { payload(x).evidence[0].reference = 'check:example-parser-default-missing'; }),
    /evidence entry unit-default does not resolve to an actual evidence-result/), 'a verified assertion covered only by an unresolvable entry was accepted');
  // resolves, but observed_result is not "confirmed"
  const withInconclusive = evalWith(clone(BASE), resolverWith((m) => { m[EVIDENCE_REF('unit-default')].observed_result = 'inconclusive'; }));
  assert(withInconclusive.some((msg) => /verifiable assertion "(default-returned|default-logged)" is "verified" but no resolved, pin-checked evidence entry confirms it/.test(msg)),
    'a verified assertion whose only evidence resolves "inconclusive" was accepted');
  // with no resolver at all
  assert(evaluateEvidenceAndHandoff(clone(BASE), { recordSchema, envelopeSchema })
    .some((m) => /cannot be verified: no external record resolver/.test(m)), 'a handoff with no resolver was not failed closed');
});

function EVIDENCE_REF(name) {
  return BASE.payload.evidence.find((e) => e.id === name).reference;
}

check('CHANGES_REQUESTED 2 — a specialised verdict is confirmed by the resolved specialised record, not the handoff', () => {
  const spec = COMPLETE.payload.evidence.find((e) => e.kind === 'specialised-evidence-record');
  assert(spec, 'the complete fixture has no specialised-evidence-record entry');
  // the handoff states a verdict the resolved record does not confirm
  assert(rejectsBecause(mutateComplete((x) => {
    payload(x).evidence.find((e) => e.id === spec.id).recorded_verdict = 'UNVERIFIED';
  }), /the resolved specialised evidence record's recorded_verdict .* does not match the handoff's stated recorded_verdict/),
    'a specialised verdict the resolved record does not confirm was accepted');
  // the resolver confirms a different contract identity
  assert(evalWith(clone(COMPLETE), resolverWith((m) => { m[spec.reference].specialised_contract = 'smoke-protocol'; }))
    .some((msg) => /the resolved specialised evidence record's specialised_contract "smoke-protocol" does not match/.test(msg)),
    'a mismatched specialised contract identity was accepted');
});

check('narrow evidence is not widened: covers may only name a declared assertion', () => {
  assert(rejectsBecause(mutate((x) => { payload(x).evidence[0].covers.push('undeclared-assertion'); }),
    /covers "undeclared-assertion", which is not a declared verifiable assertion/), 'an evidence entry covering an undeclared assertion was accepted');
});

check('CHANGES_REQUESTED 6 — the claimed_result inference is two-sided', () => {
  assert(rejectsBecause(mutate((x) => { payload(x).claimed_results.push({ id: 'ghost', statement: 'без доказательств', status: 'established' }); }),
    /claimed result "ghost" is established but no verifiable assertion links to it/), 'an established result with no assertion was accepted');
  assert(rejectsBecause(mutate((x) => {
    payload(x).verifiable_assertions[2].status = 'unverified';
    payload(x).verifiable_assertions[2].unverified_reason = 'не проверено';
  }), /is established but assertion\(s\) .* are not verified/), 'an established result with an unverified linked assertion was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).claimed_results[0].status = 'not_established'; }),
    /is "not_established" but it has at least one linked verifiable assertion and every one of them is verified/),
    'a not_established result over a fully verified assertion set was accepted');
});

// ---------------------------------------------------------------------------
// 4. three distinct check statuses, each backed by resolved result evidence
// ---------------------------------------------------------------------------
check('CHANGES_REQUESTED 2/3 — mandatory checks are backed by resolved result evidence, not by completed_checks', () => {
  assert(rejectsBecause(mutate((x) => { delete payload(x).mandatory_checks[0].result_evidence_id; }),
    /is "passed" but names no result_evidence_id/), 'a passed check with no result evidence was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).mandatory_checks[0].result_evidence_id = 'no-such-evidence'; }),
    /result_evidence_id "no-such-evidence" is not a declared evidence entry/), 'a passed check pointing at no evidence was accepted');
  // a passed check whose result evidence resolves "contradicted"
  assert(evalWith(clone(BASE), resolverWith((m) => { m[EVIDENCE_REF('kv-run')].observed_result = 'contradicted'; }))
    .some((msg) => /mandatory check kernel-validate is "passed" but its result evidence "kv-run" resolves with observed_result "contradicted", not "confirmed"/.test(msg)),
    'a passed check whose result evidence contradicts was accepted');
});

check('CHANGES_REQUESTED — "unable" is a distinct status: no result evidence, a verifiable reason, never a pass', () => {
  assert(rejectsBecause(mutate((x) => { payload(x).mandatory_checks[1] = { id: 'handoff-suite', name: 'x', check_ref: 'check:example-evidence-handoff-suite', status: 'unable' }; }),
    /is "unable" but states no verifiable reason; an unable check is not a pass/), 'an unable check with no reason was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).mandatory_checks[1] = { id: 'handoff-suite', name: 'x', check_ref: 'check:example-evidence-handoff-suite', status: 'unable', reason: 'r', result_evidence_id: 'suite-run' }; }),
    /is "unable" but names a result_evidence_id/), 'an unable check naming result evidence was accepted');
});

check('completed_checks of the resolved run confirms COMPLETION only, not the verdict', () => {
  assert(rejectsBecause(mutate((x) => { payload(x).mandatory_checks.push({ id: 'extra', name: 'x', check_ref: 'check:example-not-completed', status: 'passed', result_evidence_id: 'kv-run' }); }),
    /mandatory check "check:example-not-completed" ran .* not among the resolved execution-run record's completed_checks/), 'a passed check not in completed_checks was accepted');
});

// ---------------------------------------------------------------------------
// 5. acceptance-criteria coverage, closed against the resolved task spec
// ---------------------------------------------------------------------------
check('CHANGES_REQUESTED 3 — every acceptance criterion of the resolved task spec is addressed or explicitly uncovered', () => {
  assert(rejectsBecause(mutate((x) => { payload(x).acceptance_criteria[0].id = 'invented-criterion'; }),
    /acceptance criterion "invented-criterion" is not among the resolved task-specification record's acceptance criteria/), 'an unknown criterion id was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).acceptance_criteria = [payload(x).acceptance_criteria[0]]; }),
    /does not address acceptance criterion "default-logged-info" declared by the resolved task-specification record/), 'a missing criterion was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).acceptance_criteria.push({ id: 'default-applied', status: 'covered', addressed_by: ['default-returned'] }); }),
    /acceptance criterion id "default-applied" is used more than once/), 'a duplicate criterion id was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).acceptance_criteria[0] = { id: 'default-applied', status: 'covered', addressed_by: [] }; }),
    /is "covered" but names no addressed_by verifiable assertion/), 'a covered criterion with no assertion was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).acceptance_criteria[0].addressed_by = ['no-such-assertion']; }),
    /addressed_by "no-such-assertion", which is not a declared verifiable assertion/), 'a covered criterion addressing an undeclared assertion was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).acceptance_criteria[0] = { id: 'default-applied', status: 'uncovered', addressed_by: [] }; }),
    /is "uncovered" but states no uncovered_reason/), 'an uncovered criterion with no reason was accepted');
});

check('CHANGES_REQUESTED 2 — a self-selected empty mandatory_checks list does not discharge the spec acceptance criteria', () => {
  assert(rejectsBecause(mutate((x) => { payload(x).mandatory_checks = []; }),
    /lists no mandatory_checks, but the resolved task-specification record declares \d+ acceptance criteria/), 'an empty mandatory_checks list with spec criteria was accepted');
});

// ---------------------------------------------------------------------------
// 6. source / result state — same repository set, differing pins for changes
// ---------------------------------------------------------------------------
check('CHANGES_REQUESTED 5 — source_state and result_state pin the same set of repositories', () => {
  assert(rejectsBecause(mutate((x) => { payload(x).result_state.push({ repository_ref: 'repo:example-shared', revision: 'e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4' }); }),
    /source_state and result_state pin different sets of repositories/), 'differing repository sets were accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).result_state = [{ repository_ref: 'repo:example-service', revision: payload(x).source_state[0].revision }]; }),
    /its source_state and result_state pins are identical; a change produced a new revision/), 'a changed repo with identical source/result pins was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).source_state = []; }), /lists no repository state|fewer than 1 items/), 'an empty source_state was accepted');
});

// ---------------------------------------------------------------------------
// 7. the six separate event axes, each required, no duplicate id
// ---------------------------------------------------------------------------
check('changed paths, external effects, deviations, open gaps, owner decisions and blockers are separate required lists', () => {
  for (const f of ['changed_paths', 'external_effects', 'deviations', 'open_gaps', 'required_owner_decisions', 'blockers', 'acceptance_criteria']) {
    assert(rejectsBecause(mutate((x) => { delete payload(x)[f]; }), /required property missing/), `an omitted "${f}" list was accepted`);
  }
  assert(rejectsBecause(mutate((x) => { payload(x).deviations.push({ id: payload(x).deviations[0].id, description: 'd', severity: 'non_blocking', disposition: 'p' }); }),
    /deviation id ".*" is used more than once/), 'a duplicate deviation id was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).blockers = [{ id: 'b', description: 'd' }]; }),
    /has no verifiable resumption condition|resumption_condition.*required/), 'a blocker with no resumption condition was accepted');
});

// ---------------------------------------------------------------------------
// 8. worktree disposition — the closed set of version-control-flow.md §5.4
// ---------------------------------------------------------------------------
check('worktree_disposition uses the closed set and retained requires reason, responsible and cleanup condition', () => {
  assert(rejectsBecause(mutate((x) => { delete payload(x).worktree_disposition.cleanup_condition; }),
    /worktree_disposition is "retained" but states no cleanup_condition/), 'a retained worktree with no cleanup condition was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).worktree_disposition = { state: 'removed', reason: 'x' }; }),
    /is "removed" but carries "reason"; reason, responsible and cleanup_condition belong to the "retained" state only/), 'a removed worktree carrying a retained-only field was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).worktree_disposition = { state: 'purged' }; }), /not in enum/), 'a worktree state outside the closed set was accepted');
});

// ---------------------------------------------------------------------------
// 9. outcome.status rules — complete is the WHOLE run
// ---------------------------------------------------------------------------
check('CHANGES_REQUESTED 4/7 — outcome.status "complete" means the whole run is finished', () => {
  // active resolved run + executable next step
  assert(rejectsBecause(mutate((x) => { payload(x).outcome.status = 'complete'; payload(x).required_owner_decisions = []; payload(x).deviations = []; payload(x).open_gaps = []; }),
    /outcome.status is "complete" but the resolved execution-run record's work_status is "active", not "completed"/), 'a complete outcome over an active run was accepted');
  // a non-empty open_gaps
  assert(rejectsBecause(mutateComplete((x) => { payload(x).open_gaps = [{ id: 'perf', description: 'не измерялось' }]; }),
    /outcome.status is "complete" but \d+ open gap\(s\) remain/), 'a complete outcome with an open gap was accepted');
  // an acceptance criterion covered only by an unverified assertion
  assert(rejectsBecause(mutateComplete((x) => {
    payload(x).verifiable_assertions.find((a) => a.id === 'default-returned').status = 'unverified';
    payload(x).verifiable_assertions.find((a) => a.id === 'default-returned').unverified_reason = 'снято ради примера';
  }), /outcome.status is "complete" but acceptance criterion .* is addressed only through unverified assertion/),
    'a complete outcome with an unverified-only criterion was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).claimed_results[1].status = 'not_established'; payload(x).verifiable_assertions[2].status = 'unverified'; payload(x).verifiable_assertions[2].unverified_reason = 'r'; payload(x).outcome.status = 'complete'; }),
    /outcome.status is "complete" but not every claimed result is established/), 'a complete outcome with a not_established result was accepted');
});

check('outcome.status "blocked" needs a trigger; a non-empty blocker set forces "blocked"', () => {
  assert(rejectsBecause(mutate((x) => { payload(x).outcome.status = 'blocked'; }),
    /outcome.status is "blocked" but no blocker, failed or unable mandatory check, or blocking deviation is recorded/), 'a blocked outcome with no trigger was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).blockers = [{ id: 'b', description: 'd', resumption_condition: 'c' }]; }),
    /a non-empty blocker set is a "blocked" outcome|blockers do not match the resolved execution-run record's blocker_ids/), 'a non-empty blocker set with a non-blocked outcome was accepted');
});

// ---------------------------------------------------------------------------
// 10. the external resolution boundary — the four pinned records
// ---------------------------------------------------------------------------
check('the handoff is checked against the state the RESOLVED execution-run record carries', () => {
  assert(rejectsBecause(mutate((x) => { payload(x).next_step.action = 'do-something-else'; }),
    /next_step.action .* does not match the resolved execution-run record's next_action/), 'a contradicting next_step.action was accepted');
  assert(rejectsBecause(mutate((x) => {
    payload(x).task_specification_ref = { record_type: 'task-specification', id: 'example-extract-parser-module', reference: 'records/task-specification/example-extract-parser-module', sha256: 'fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210' };
  }), /the resolved execution-run record names task specification "records\/task-specification\/example-clarify-null-handling"|does not resolve to an actual task-specification/),
    'a task_specification_ref disagreeing with the resolved run was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).execution_run_ref.revision = 'd4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3'; }),
    /execution_run_ref pins revision "d4e5.*" but the resolver confirmed edition "v0.5.0"/), 'a pin disagreeing with the resolved edition was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).human_control_ref.reference = 'records/run-human-control/example-run-eh-001-missing'; }),
    /human_control_ref does not resolve to an actual run-human-control through the external resolver/), 'a human_control_ref that resolves to nothing was accepted');
});

check('the resolved transformer response is a CLOSED contract (unknown key, missing resolved_state field, wrong type, missing criteria)', () => {
  const p1 = evalWith(clone(BASE), resolverWith((m) => { m['records/execution-run/example-run-eh-001'].smuggled = 'x'; }));
  assert(p1.some((m) => /unknown field "smuggled"; the transformer response is closed to \{/.test(m)), 'an unknown field on the resolved run entry was accepted');
  const p2 = evalWith(clone(BASE), resolverWith((m) => { delete m['records/execution-run/example-run-eh-001'].resolved_state.work_status; }));
  assert(p2.some((m) => /resolved_state is missing required field "work_status"/.test(m)), 'a resolved_state missing a field was accepted');
  const p3 = evalWith(clone(BASE), resolverWith((m) => { m['records/execution-run/example-run-eh-001'].resolved_state.blocker_ids = 'x'; }));
  assert(p3.some((m) => /resolved_state\.blocker_ids is string, not an array/.test(m)), 'a wrong-typed resolved_state axis was accepted');
  const p4 = evalWith(clone(BASE), resolverWith((m) => { delete m['records/task-specification/example-clarify-null-handling'].acceptance_criteria; }));
  assert(p4.some((m) => /the resolver returned a task-specification record with no acceptance_criteria array/.test(m)), 'a task-specification with no acceptance_criteria was accepted');
  const p5 = evalWith(clone(BASE), resolverWith((m) => { m[EVIDENCE_REF('unit-default')].observed_result = 'maybe'; }));
  assert(p5.some((m) => /the resolved evidence-result's observed_result "maybe" is not one of/.test(m)), 'an out-of-pool observed_result was accepted');
});

check('a context-manifest that resolves to another run is rejected', () => {
  assert(rejectsBecause(mutate((x) => {
    payload(x).context_manifest_ref = { record_type: 'context-manifest', id: 'example-manifest-eh-mismatch', run_id: 'example-run-eh-001', reference: 'records/context-manifest/example-run-eh-mismatch', revision: 'v0.5.0' };
  }), /context_manifest_ref resolves to a context-manifest record whose linked_run_ref is .* not the handoff's pinned run/),
    'a context_manifest_ref bound to another run was accepted');
});

// ---------------------------------------------------------------------------
// portability and boundedness
// ---------------------------------------------------------------------------
check('an unbounded material dump, a specialised-evidence body or a referenced-record body is rejected', () => {
  for (const f of ['command_log', 'transcript', 'chat_history', 'parity_comparison', 'context_manifest', 'transition_history', 'field_metrics']) {
    assert(evaluate(mutate((x) => { payload(x)[f] = f === 'command_log' ? ['a'] : 'a'; })).length > 0, `a "${f}" field was accepted in the payload`);
  }
  assert(FORBIDDEN_PAYLOAD_FIELDS.includes('transcript') && FORBIDDEN_PAYLOAD_FIELDS.includes('parity_comparison')
    && FORBIDDEN_PAYLOAD_FIELDS.includes('context_manifest'), 'the forbidden-field list lost a load-bearing entry');
});

check('a rooted machine path in a reference, a path or a prose string breaks portability', () => {
  assert(rejectsBecause(mutate((x) => { payload(x).changed_paths[0].path = '/home/example/src/parser.ext'; }),
    /changed path .* path contains a rooted \(absolute\) POSIX path/), 'a rooted changed path was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).next_step.action = '/usr/local/bin/do-it'; }),
    /next_step.action contains a rooted \(absolute\) POSIX path/), 'a rooted next_step.action was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).execution_run_ref.reference = '/opt/runs/example-run-eh-001'; }),
    /execution_run_ref reference contains a rooted \(absolute\) POSIX path/), 'a rooted execution_run_ref reference was accepted');
  assert(nonPortableReason('records/execution-run/example') === null, 'a repo-relative path was flagged');
});

check('the shipped normative document and schema agree on the record type and the new mechanisms', () => {
  const norm = loadText('standards/workspace/evidence-and-handoff-contract.md');
  assert(/evidence-and-handoff/.test(norm), 'the normative document does not name the record type');
  assert(/run-state/.test(norm), 'the normative document does not name the run-state scope');
  assert(/not_created|removed|retained/.test(norm), 'the normative document does not describe the worktree disposition set');
  assert(/observed_result/.test(norm), 'the normative document does not describe evidence-result resolution');
  assert(/acceptance_criteria|критери/i.test(norm), 'the normative document does not describe acceptance-criteria coverage');
  assert(/result_evidence_id/.test(norm), 'the normative document does not describe result evidence for mandatory checks');
  assert(validate && typeof validate === 'function', 'the generic validator is unavailable');
  assert(Array.isArray(LIFECYCLE_STAGES) && Array.isArray(WORK_STATUSES), 'the reused execution-state pools are unavailable');
});

// ---------------------------------------------------------------------------
// CHANGES_REQUESTED round 2 — four defects and their regression scenarios
//   1. a resolved evidence entry is bound to the EXACT subject it confirms:
//      the closed evidence-result response confirms which assertion(s) (covers)
//      and/or which mandatory check (check_ref) observed_result is about, so a
//      result cannot be moved between checks and a covers link cannot be
//      transferred to a foreign assertion;
//   2. the FULL set of mandatory_checks is closed against the resolved task
//      specification (a minimal machine list, not the spec body) — dropping any
//      one is rejected;
//   3. completed_checks of the resolved run confirms COMPLETION: a passed OR a
//      failed check's check_ref must appear there, only "unable" must be absent;
//   4. the resolved task-specification response rejects an empty
//      acceptance_criteria set — a canonical specification requires a non-empty
//      one.
// ---------------------------------------------------------------------------
const BLOCKED = clone(fixtures.valid.find((c) => c.spec.payload.outcome.status === 'blocked').spec);
const MINIMAL = clone(fixtures.valid.find((c) => /minimal/.test(c.note)).spec);

check('round2/1 — the two result evidences of two checks cannot be permuted', () => {
  const d = mutate((x) => {
    const mc = payload(x).mandatory_checks;
    const a = mc.find((m) => m.id === 'kernel-validate');
    const b = mc.find((m) => m.id === 'handoff-suite');
    const t = a.result_evidence_id; a.result_evidence_id = b.result_evidence_id; b.result_evidence_id = t;
  });
  assert(rejectsBecause(d, /result evidence "[^"]+", but the resolved evidence-result records check_ref "[^"]+", not this check's check_ref/),
    'permuted result evidences of two checks were accepted');
});

check('round2/2 — a resolved evidence entry cannot be re-bound to a foreign assertion', () => {
  const d = mutate((x) => {
    payload(x).verifiable_assertions.push({
      id: 'foreign-assertion',
      statement: 'Постороннее утверждение, не подтверждённое этим доказательством.',
      claimed_result_id: 'null-handling-implemented',
      status: 'verified',
    });
    payload(x).evidence.find((e) => e.id === 'parity-diff').covers.push('foreign-assertion');
  });
  assert(rejectsBecause(d, /does not confirm that this evidence bears on assertion "foreign-assertion"; a resolved evidence entry's covers is not transferred to a foreign assertion/),
    'an evidence entry re-bound to a foreign assertion was accepted');
});

check('round2/3 — dropping a mandatory check the resolved spec declares is rejected', () => {
  const d = mutate((x) => { payload(x).mandatory_checks = payload(x).mandatory_checks.filter((m) => m.id === 'kernel-validate'); });
  assert(rejectsBecause(d, /omits mandatory check "[^"]+" declared by the resolved task-specification record; removing a mandatory check from the handoff is rejected/),
    'a handoff omitting a mandatory check the resolved spec declares was accepted');
});

check('round2/4 — an empty acceptance_criteria set from the transformer is rejected, even with empty handoff lists', () => {
  const specRef = COMPLETE.payload.task_specification_ref.reference;
  const d = mutateComplete((x) => { payload(x).acceptance_criteria = []; payload(x).mandatory_checks = []; });
  const out = evalWith(d, resolverWith((m) => { m[specRef].acceptance_criteria = []; m[specRef].mandatory_checks = []; }));
  assert(out.some((msg) => /the resolved task-specification record's acceptance_criteria is empty; a canonical task specification requires a non-empty acceptance-criteria set/.test(msg)),
    'a completed handoff whose resolved spec has empty acceptance_criteria and mandatory_checks was accepted');
});

check('round2/5 — a failed check whose check_ref IS in the resolved run completed_checks is accepted', () => {
  const runRef = BLOCKED.payload.execution_run_ref.reference;
  const out = evalWith(clone(BLOCKED), resolverWith((m) => {
    m[runRef].resolved_state.completed_checks = ['check:example-kernel-validate', 'check:example-evidence-handoff-suite'];
  }));
  assert(out.length === 0, `a valid blocked handoff whose failed check is a completed check was rejected: ${out[0]}`);
});

check('round2/6 — a failed check whose check_ref is NOT in the resolved run completed_checks is rejected', () => {
  const runRef = BLOCKED.payload.execution_run_ref.reference;
  const out = evalWith(clone(BLOCKED), resolverWith((m) => {
    m[runRef].resolved_state.completed_checks = ['check:example-kernel-validate'];
  }));
  assert(out.some((msg) => /mandatory check "[^"]+" (?:is "failed"|ran).* not among the resolved execution-run record's completed_checks/.test(msg)),
    'a failed check absent from the resolved run completed_checks was accepted');
});

check('round2/7 — an "unable" check whose check_ref IS in the resolved run completed_checks is rejected', () => {
  const runRef = MINIMAL.payload.execution_run_ref.reference;
  const unableRef = MINIMAL.payload.mandatory_checks.find((m) => m.status === 'unable').check_ref;
  const out = evalWith(clone(MINIMAL), resolverWith((m) => {
    const cc = new Set([...(m[runRef].resolved_state.completed_checks || []), unableRef]);
    m[runRef].resolved_state.completed_checks = [...cc];
  }));
  assert(out.some((msg) => /is "unable" but appears among the resolved execution-run record's completed_checks/.test(msg)),
    'an unable check present in the resolved run completed_checks was accepted');
});

check('round2 — the normative document states the four corrected mechanisms', () => {
  const norm = loadText('standards/workspace/evidence-and-handoff-contract.md');
  assert(/check_ref/.test(norm) && /covers/.test(norm),
    'the normative document does not bind a resolved evidence-result to its confirmed subject (covers / check_ref)');
  assert(/mandatory_checks/.test(norm) && /замкнут|закрыт/i.test(norm),
    'the normative document does not close the full mandatory_checks set against the specification');
  assert(/failed.*(completed_checks|числ|присут)/is.test(norm),
    'the normative document does not state that a failed check is also a completed check');
  assert(/непуст[^\s]*\s+набор\s+критери/i.test(norm) && /пуст[^\s]*\s+`?acceptance_criteria`?[\s\S]{0,80}отклоня/i.test(norm),
    'the normative document does not require a non-empty acceptance_criteria set from the resolved specification');
});

// ---------------------------------------------------------------------------
if (failures.length) {
  console.log(`\n${failures.length} failing / ${passed} passing`);
  for (const f of failures) console.log(`  - ${f}`);
  process.exit(1);
}
console.log(`\nAll ${passed} checks passed.`);
