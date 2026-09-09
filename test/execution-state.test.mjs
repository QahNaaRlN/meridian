#!/usr/bin/env node
// Standalone verification for the execution state model
// (registries/operating-model/execution-state.schema.json).
//
// It calls the same implementation the gate calls
// (scripts/lib/execution-state.mjs) so the two cannot drift: the canonical
// scoped-record envelope (validated separately and always), the complete
// specialised schema a record declares in its own $schema, and the rules JSON
// Schema cannot state — the portable, logically-resolved $schema declaration,
// the one allowed scope (run-state), the single task-specification reference,
// the independent axes and their mutual consistency, and the ordered
// transition history with its no-break chain, its explicit initial state, its
// last-record match, and its explicit basis for a backward move, a skipped
// stage or a transition after a terminal state.
//
// Usage: node test/execution-state.test.mjs

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { assertSupportedDeep, UnsupportedSchema, validate } from '../scripts/lib/json-schema.mjs';
import {
  evaluateExecutionState,
  nonPortableReason,
  resolveSchemaRef,
  stageIndex,
  ALLOWED_SCOPE_TYPE,
  CANONICAL_RECORD_BASE,
  EXPECTED_SCHEMA_BASENAME,
  EXPECTED_SCHEMA_REF,
  ENVELOPE_SCHEMA_REF,
  LIFECYCLE_STAGES,
  WORK_STATUSES,
  TERMINAL_STATUSES,
  RECORD_TYPE,
  REQUIRED_AXES,
  FORBIDDEN_PAYLOAD_FIELDS,
} from '../scripts/lib/execution-state.mjs';

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

const recordSchema = loadJson('registries/operating-model/execution-state.schema.json');
const envelopeSchema = loadJson('registries/operating-model/scoped-record.schema.json');
const fixtures = loadJson('registries/operating-model/fixtures/execution-state.fixtures.json');

const opts = { recordSchema, envelopeSchema };
const evaluate = (doc) => evaluateExecutionState(doc, opts);

// A topologically complete valid run record, the base for targeted mutations.
const BASE = clone(fixtures.valid[0].spec);
const payload = (d) => d.payload;
const mutate = (fn) => { const d = clone(BASE); fn(d); return d; };

// A VERIFICATION ADAPTER, not a universal physical record path: it maps the
// resolved LOGICAL address of the declared $schema onto the built-in schema
// file shipped with this Kernel checkout, purely so this test can inspect the
// schema's contents. A production storage adapter is free to map the same
// logical address onto working data in a local store, a remote service or the
// transitional Instance. Returns an absolute path, or null when the reference
// does not resolve to the specialised schema's logical address.
const verificationAdapterSchemaFile = (ref) => {
  const rel = resolveSchemaRef(ref);
  return rel === EXPECTED_SCHEMA_REF ? path.join(root, rel) : null;
};

// ---------------------------------------------------------------------------
// schema shape
// ---------------------------------------------------------------------------
check('both schemas use the supported JSON Schema subset', () => {
  assertSupportedDeep(recordSchema, 'execution-state.schema.json');
  assertSupportedDeep(envelopeSchema, 'scoped-record.schema.json');
});

check('an unsupported schema keyword is rejected up front (assertSupportedDeep)', () => {
  const bad = clone(recordSchema);
  bad.definitions.payload.patternProperties = { '^x': { type: 'string' } };
  let threw = null;
  try { assertSupportedDeep(bad, 'execution-state.schema.json'); }
  catch (e) { threw = e; }
  assert(threw instanceof UnsupportedSchema, 'assertSupportedDeep did not reject the unsupported keyword');
});

check('the specialised schema COMPOSES the envelope with the body (one pass, one model)', () => {
  assert(recordSchema.properties.record_type.const === RECORD_TYPE, 'record_type is not pinned to execution-run');
  assert(recordSchema.additionalProperties === false, 'the record schema is not a closed object');
  for (const k of ['$schema', 'schema_version', 'id', 'title', 'record_type', 'scope', 'origin', 'authority', 'payload']) {
    assert(recordSchema.required.includes(k), `the record schema does not require the envelope field "${k}"`);
  }
  assert(recordSchema.definitions.payload.additionalProperties === false, 'payload is not a closed object');
  for (const k of REQUIRED_AXES) {
    assert(recordSchema.definitions.payload.required.includes(k), `payload does not require the axis "${k}"`);
  }
  assert(JSON.stringify(recordSchema.definitions.scope_ref.properties.type.enum) === JSON.stringify([ALLOWED_SCOPE_TYPE]),
    'the schema scope enum is not exactly {run-state}');
  assert(recordSchema.definitions.scope_ref.required.includes('workspace_id'), 'the schema scope_ref does not require workspace_id');
});

check('the closed pools match the plan', () => {
  assert(JSON.stringify(recordSchema.definitions.lifecycle_stage.enum) === JSON.stringify(LIFECYCLE_STAGES),
    'the schema lifecycle_stage enum diverges from LIFECYCLE_STAGES');
  assert(JSON.stringify(recordSchema.definitions.work_status.enum) === JSON.stringify(WORK_STATUSES),
    'the schema work_status enum diverges from WORK_STATUSES');
  assert(LIFECYCLE_STAGES.length === 11 && LIFECYCLE_STAGES[0] === 'intake' && LIFECYCLE_STAGES[10] === 'completion',
    'the lifecycle order is not intake → … → completion');
  assert(WORK_STATUSES.length === 8, 'the work-status pool is not the closed set of eight');
  assert(JSON.stringify(TERMINAL_STATUSES) === JSON.stringify(['completed', 'cancelled']),
    'the terminal statuses are not exactly {completed, cancelled}');
});

// ---------------------------------------------------------------------------
// the declared $schema reaches the specialised contract, logically
// ---------------------------------------------------------------------------
check('every VALID fixture declares $schema = the specialised schema, by a portable relative reference that RESOLVES', () => {
  fixtures.valid.forEach((c, i) => {
    const ref = c.spec && c.spec.$schema;
    assert(typeof ref === 'string' && ref !== '', `valid[${i}] (${c.note}) has no $schema`);
    assert(!path.isAbsolute(ref) && ref.startsWith('../'), `valid[${i}] (${c.note}) $schema "${ref}" is not a portable relative reference`);
    assert(resolveSchemaRef(ref) === EXPECTED_SCHEMA_REF,
      `valid[${i}] (${c.note}) $schema "${ref}" does not resolve to the logical address ${EXPECTED_SCHEMA_REF}`);
  });
});

check('the $schema reference resolves as a filesystem-independent logical address', () => {
  assert(CANONICAL_RECORD_BASE === 'records/execution-run', `the canonical logical base changed unexpectedly: ${CANONICAL_RECORD_BASE}`);

  // (a) resolution yields a LOGICAL address: a namespace-relative string, never
  //     an absolute path and never tied to this checkout's location on disk.
  const addr = resolveSchemaRef('../../registries/operating-model/execution-state.schema.json');
  assert(addr === EXPECTED_SCHEMA_REF, `the canonical reference no longer resolves to the specialised schema's logical address: ${addr}`);
  assert(!path.isAbsolute(addr) && !addr.includes(root), 'the resolved value is not a pure logical address');
  assert(resolveSchemaRef('../../registries/operating-model/scoped-record.schema.json') === ENVELOPE_SCHEMA_REF,
    'the envelope reference no longer resolves to scoped-record');

  // (b) resolution is pure address math, not a filesystem lookup.
  assert(resolveSchemaRef('../../made-up/segment/thing.json') === 'made-up/segment/thing.json',
    'resolution of an arbitrary in-namespace reference did not produce a logical address');

  // (c) resolution does NOT depend on the process working directory.
  const cwd0 = process.cwd();
  try {
    process.chdir(os.tmpdir());
    const fromTmp = resolveSchemaRef('../../registries/operating-model/execution-state.schema.json');
    process.chdir(path.parse(root).root);
    const fromFsRoot = resolveSchemaRef('../../registries/operating-model/execution-state.schema.json');
    assert(fromTmp === addr && fromFsRoot === addr,
      `resolveSchemaRef changed with process.cwd(): ${fromTmp} / ${fromFsRoot} vs ${addr}`);
  } finally {
    process.chdir(cwd0);
  }

  // (d) a SEPARATE verification adapter maps the logical address onto the
  //     Kernel schema file; the logical layer itself never touched the disk.
  const mapped = verificationAdapterSchemaFile('../../registries/operating-model/execution-state.schema.json');
  assert(mapped && path.isAbsolute(mapped) && fs.existsSync(mapped),
    'the verification adapter did not map the logical address onto a real Kernel schema file');
  assert(String(JSON.parse(fs.readFileSync(mapped, 'utf8')).$id || '').endsWith(EXPECTED_SCHEMA_BASENAME),
    'the mapped file is not the specialised schema');
});

check('REGRESSION: a valid run with transition_history removed is RED against the schema named in its own $schema', () => {
  const spec = clone(fixtures.valid[0].spec);
  const schemaFile = verificationAdapterSchemaFile(spec.$schema);
  assert(schemaFile && fs.existsSync(schemaFile), `the declared $schema "${spec.$schema}" does not resolve to the specialised schema's logical address`);
  const declaredSchema = JSON.parse(fs.readFileSync(schemaFile, 'utf8'));
  delete spec.payload.transition_history;
  const errs = [];
  validate(spec, declaredSchema, declaredSchema, '', errs);
  assert(errs.length > 0, 'a run missing payload.transition_history validated clean against the schema named in its $schema');
  assert(errs.some((m) => /transition_history/.test(m)), `the failure did not point at transition_history: ${errs[0]}`);
});

check('an absent $schema is rejected', () => {
  const d = mutate((x) => { delete x.$schema; });
  assert(evaluate(d).some((m) => /declares no \$schema/.test(m) || /\$schema.*required/.test(m)),
    'a record with no $schema was accepted');
});

check('a $schema that resolves to a different schema (right directory, wrong file) is rejected', () => {
  const d = mutate((x) => { x.$schema = '../../registries/operating-model/some-other.schema.json'; });
  assert(evaluate(d).some((m) => /does not resolve to the logical address registries\/operating-model\/execution-state\.schema\.json/.test(m)),
    'a $schema resolving to a different file was accepted');
});

check('a $schema that resolves to the record envelope is rejected', () => {
  const d = mutate((x) => { x.$schema = '../../registries/operating-model/scoped-record.schema.json'; });
  assert(evaluate(d).some((m) => /resolves to the record envelope/.test(m)),
    'a $schema pointing only at scoped-record was accepted');
});

check('a $schema given as an absolute machine path is rejected', () => {
  const d = mutate((x) => { x.$schema = '/opt/meridian/registries/operating-model/execution-state.schema.json'; });
  assert(evaluate(d).some((m) => /\$schema .* is not portable/.test(m)), 'an absolute $schema path was accepted');
});

check('a $schema given as a bare basename does not resolve and is rejected', () => {
  assert(resolveSchemaRef('execution-state.schema.json') !== EXPECTED_SCHEMA_REF, 'a bare basename resolved to the specialised schema');
  const d = mutate((x) => { x.$schema = 'execution-state.schema.json'; });
  assert(evaluate(d).some((m) => /does not resolve to the logical address/.test(m)), 'a bare-basename $schema was accepted');
});

check('a $schema that climbs out of the Meridian namespace root does not resolve and is rejected', () => {
  assert(resolveSchemaRef('../../../registries/operating-model/execution-state.schema.json') === null,
    'a reference climbing past the namespace root still resolved');
  const d = mutate((x) => { x.$schema = '../../../registries/operating-model/execution-state.schema.json'; });
  assert(evaluate(d).some((m) => /does not resolve to/.test(m)), 'an out-of-namespace $schema was accepted');
});

check('the canonical scoped-record envelope is still validated separately (not dropped)', () => {
  const d = mutate((x) => { delete x.authority; });
  assert(evaluate(d).some((m) => /^envelope /.test(m)), 'a missing authority block was not reported through the canonical envelope schema');
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
    const p = evaluate(c.spec);
    assert(p.length > 0, `invalid[${i}] (${c.note}) was accepted`);
  });
});

check('every valid fixture is scoped to run-state only, with scope.id and workspace_id', () => {
  fixtures.valid.forEach((c, i) => {
    assert(c.spec.scope.type === ALLOWED_SCOPE_TYPE, `valid[${i}] (${c.note}) uses scope "${c.spec.scope.type}"`);
    assert(typeof c.spec.scope.id === 'string' && c.spec.scope.id.length > 0, `valid[${i}] (${c.note}) has no scope.id`);
    assert(typeof c.spec.scope.workspace_id === 'string' && c.spec.scope.workspace_id.length > 0, `valid[${i}] (${c.note}) has no workspace_id`);
  });
});

check('every valid fixture references exactly one task specification and does not embed it', () => {
  fixtures.valid.forEach((c, i) => {
    const tsr = c.spec.payload.task_specification_ref;
    assert(typeof tsr === 'string' && tsr.length > 0, `valid[${i}] (${c.note}) has no task_specification_ref`);
    for (const bodyField of ['goal', 'initial_state', 'target_model', 'acceptance_criteria', 'constraints', 'task_pattern']) {
      assert(!(bodyField in c.spec.payload), `valid[${i}] (${c.note}) embeds a specification body field "${bodyField}"`);
    }
  });
});

check('several runs may reference one task specification', () => {
  const byRef = new Map();
  for (const c of fixtures.valid) {
    const ref = c.spec.payload.task_specification_ref;
    byRef.set(ref, (byRef.get(ref) || []).concat([c.spec.id]));
  }
  const shared = [...byRef.entries()].find(([, ids]) => ids.length > 1);
  assert(shared, 'no task_specification_ref is exercised by more than one valid run fixture');
  assert(new Set(shared[1]).size === shared[1].length, `two run fixtures for ${shared[0]} share an id: ${shared[1].join(', ')}`);
});

// ---------------------------------------------------------------------------
// lifecycle stages and work statuses — full coverage
// ---------------------------------------------------------------------------
check('every lifecycle_stage value is covered as a to_stage across the valid fixtures', () => {
  const seen = new Set();
  for (const c of fixtures.valid) for (const t of c.spec.payload.transition_history) seen.add(t.to_stage);
  for (const stage of LIFECYCLE_STAGES) assert(seen.has(stage), `no valid fixture transitions to lifecycle stage "${stage}"`);
});

check('every work_status value is covered as a to_status across the valid fixtures', () => {
  const seen = new Set();
  for (const c of fixtures.valid) for (const t of c.spec.payload.transition_history) seen.add(t.to_status);
  for (const status of WORK_STATUSES) assert(seen.has(status), `no valid fixture transitions to work status "${status}"`);
});

check('an unknown lifecycle_stage is rejected', () => {
  const d = mutate((x) => { payload(x).lifecycle_stage = 'deploying'; });
  assert(evaluate(d).length > 0, 'an unknown lifecycle_stage was accepted');
});

check('an unknown work_status is rejected', () => {
  const d = mutate((x) => { payload(x).work_status = 'in_progress'; });
  assert(evaluate(d).length > 0, 'an unknown work_status was accepted');
});

// ---------------------------------------------------------------------------
// independent axes — one universal status is forbidden; missing axis is a gap
// ---------------------------------------------------------------------------
check('one universal "status" field standing in for the axes is rejected', () => {
  const d = mutate((x) => { payload(x).status = 'in_progress'; });
  assert(evaluate(d).some((m) => /"status"/.test(m) || /additional property/.test(m)), 'a single universal status field was accepted');
});

for (const axis of REQUIRED_AXES) {
  check(`a missing ${axis} axis is a stated gap, not a default`, () => {
    const d = mutate((x) => { delete payload(x)[axis]; });
    assert(evaluate(d).length > 0, `a record without the ${axis} axis was accepted`);
  });
}

for (const f of FORBIDDEN_PAYLOAD_FIELDS) {
  check(`a leaked "${f}" field in the payload is rejected`, () => {
    const d = mutate((x) => { payload(x)[f] = f === 'context_manifest' || f === 'handoff' || f === 'evidence' ? {} : 'x'; });
    assert(evaluate(d).length > 0, `the leaked field "${f}" was accepted inside the run payload`);
  });
}

// ---------------------------------------------------------------------------
// scope
// ---------------------------------------------------------------------------
for (const [scopeType, extra] of [
  ['project-workspace', { id: 'example-workspace' }],
  ['repository-scope', { id: 'example-repository', workspace_id: 'example-workspace' }],
  ['built-in-methodology', { id: 'built-in-methodology' }],
  ['user-profile', { id: 'example-user' }],
  ['organization-profile', { id: 'example-organization' }],
]) {
  check(`scope "${scopeType}" is rejected for a run record`, () => {
    const d = mutate((x) => { x.scope = { type: scopeType, ...extra }; });
    const p = evaluate(d);
    assert(p.some((m) => new RegExp(`scoped to "${scopeType}"`).test(m) || /^envelope /.test(m) || /not in enum/.test(m)),
      `scope "${scopeType}" was accepted: ${JSON.stringify(p)}`);
  });
}

check('run-state without workspace_id is rejected', () => {
  const d = mutate((x) => { x.scope = { type: 'run-state', id: 'example-run-x' }; });
  assert(evaluate(d).some((m) => /workspace_id/.test(m)), 'run-state without workspace_id was accepted');
});

check('origin.kind "built-in" is rejected — a run is started in a workspace', () => {
  const d = mutate((x) => { x.origin = { kind: 'built-in' }; });
  assert(evaluate(d).length > 0, 'a built-in origin was accepted for a run record');
});

// ---------------------------------------------------------------------------
// task specification reference
// ---------------------------------------------------------------------------
check('a missing task_specification_ref is rejected', () => {
  const d = mutate((x) => { delete payload(x).task_specification_ref; });
  assert(evaluate(d).some((m) => /task_specification_ref/.test(m)), 'a run without a task_specification_ref was accepted');
});

check('an embedded task specification body instead of a reference is rejected', () => {
  const d = mutate((x) => { payload(x).task_specification_ref = { goal: 'встроенная постановка' }; });
  assert(evaluate(d).length > 0, 'an embedded specification object was accepted as the reference');
});

check('an absolute machine path as the task_specification_ref is rejected', () => {
  const d = mutate((x) => { payload(x).task_specification_ref = '/opt/specs/example'; });
  assert(evaluate(d).some((m) => /task_specification_ref contains a rooted \(absolute\) POSIX path/.test(m)),
    'an absolute task_specification_ref was accepted');
});

// ---------------------------------------------------------------------------
// axis consistency
// ---------------------------------------------------------------------------
check('waiting_human names a concrete next action; it is distinct from blocked', () => {
  const wh = fixtures.valid.find((c) => c.spec.payload.work_status === 'waiting_human');
  const bl = fixtures.valid.find((c) => c.spec.payload.work_status === 'blocked');
  assert(wh && bl, 'the valid set is missing a waiting_human or a blocked fixture');
  assert(typeof wh.spec.payload.next_action === 'string' && wh.spec.payload.next_action.trim().length > 0,
    'the waiting_human fixture has no concrete next_action');
  assert(wh.spec.payload.blockers.length === 0, 'the waiting_human fixture carries a blocker');
  assert(bl.spec.payload.blockers.length >= 1, 'the blocked fixture carries no blocker');
  const d = mutate((x) => { payload(x).work_status = 'waiting_human'; payload(x).next_action = null; payload(x).transition_history.at(-1).to_status = 'waiting_human'; });
  assert(evaluate(d).some((m) => /waiting_human.*no concrete human action/.test(m)), 'waiting_human with no next_action was accepted');
});

check('blocked carries a verifiable resumption condition; blocked with no blocker is rejected', () => {
  const bl = fixtures.valid.find((c) => c.spec.payload.work_status === 'blocked');
  assert(bl.spec.payload.blockers.every((b) => typeof b.resumption_condition === 'string' && b.resumption_condition.trim().length > 0),
    'a blocker in the blocked fixture has no resumption_condition');
  const d = mutate((x) => { payload(x).work_status = 'blocked'; payload(x).blockers = []; payload(x).transition_history.at(-1).to_status = 'blocked'; });
  assert(evaluate(d).some((m) => /work_status is "blocked" with no blocker/.test(m)), 'blocked with no blocker was accepted');
});

check('a blocker while work_status is not blocked is rejected', () => {
  const d = mutate((x) => {
    payload(x).blockers = [{ id: 'x', description: 'd', resumption_condition: 'r' }];
  });
  assert(evaluate(d).some((m) => /blocker\(s\) but work_status is/.test(m)), 'a blocker on a non-blocked run was accepted');
});

check('a blocker without a resumption condition is rejected', () => {
  const d = mutate((x) => {
    payload(x).work_status = 'blocked';
    payload(x).blockers = [{ id: 'x', description: 'd' }];
    payload(x).next_action = null;
    payload(x).next_gate = null;
    payload(x).transition_history.at(-1).to_status = 'blocked';
  });
  assert(evaluate(d).length > 0, 'a blocker with no resumption condition was accepted');
});

check('duplicate blocker ids are rejected', () => {
  const d = mutate((x) => {
    payload(x).work_status = 'blocked';
    payload(x).blockers = [
      { id: 'same', description: 'a', resumption_condition: 'ra' },
      { id: 'same', description: 'b', resumption_condition: 'rb' },
    ];
    payload(x).next_action = null;
    payload(x).next_gate = null;
    payload(x).transition_history.at(-1).to_status = 'blocked';
  });
  assert(evaluate(d).some((m) => /blocker id "same" is used more than once/.test(m)), 'duplicate blocker ids were accepted');
});

check('a completed run explicitly has no next action and no next gate', () => {
  const done = fixtures.valid.find((c) => c.spec.payload.work_status === 'completed');
  assert(done, 'the valid set is missing a completed fixture');
  assert(done.spec.payload.next_action === null && done.spec.payload.next_gate === null,
    'the completed fixture still carries a next_action or next_gate');
  const d = mutate((x) => {
    payload(x).work_status = 'completed';
    payload(x).next_action = 'do-more';
    payload(x).transition_history.at(-1).to_status = 'completed';
  });
  assert(evaluate(d).some((m) => /completed or cancelled run does not silently carry a next executable step/.test(m)),
    'a completed run with an executable next_action was accepted');
});

check('a cancelled run explicitly has no next action and no next gate', () => {
  const cancelled = fixtures.valid.find((c) => c.spec.payload.work_status === 'cancelled');
  assert(cancelled, 'the valid set is missing a cancelled fixture');
  assert(cancelled.spec.payload.next_action === null && cancelled.spec.payload.next_gate === null,
    'the cancelled fixture still carries a next_action or next_gate');
  const d = mutate((x) => {
    payload(x).work_status = 'cancelled';
    payload(x).next_gate = 'verification';
    payload(x).transition_history.at(-1).to_status = 'cancelled';
  });
  assert(evaluate(d).some((m) => /completed or cancelled run does not silently carry a next executable step/.test(m)),
    'a cancelled run with an executable next_gate was accepted');
});

check('duplicate resolved_norms are rejected', () => {
  const d = mutate((x) => { payload(x).resolved_norms = ['norm:a', 'norm:a']; });
  assert(evaluate(d).some((m) => /repeats reference/.test(m) || /not unique/.test(m)), 'duplicate resolved_norms were accepted');
});

check('a non-portable current_actor is rejected', () => {
  const d = mutate((x) => { payload(x).current_actor = 'file:///actors/x'; });
  assert(evaluate(d).some((m) => /current_actor contains a file:\/\/ URL/.test(m)), 'a file:// current_actor was accepted');
});

check('current_actor stays an opaque portable reference (an ordinary identifier passes)', () => {
  const d = mutate((x) => { payload(x).current_actor = 'actor:example-operator'; });
  assert(evaluate(d).length === 0, `an opaque actor reference was rejected: ${evaluate(d)[0]}`);
});

check('a zero, negative or fractional scope_revision is rejected', () => {
  for (const bad of [0, -1, 1.5]) {
    const d = mutate((x) => { payload(x).scope_revision = bad; });
    assert(evaluate(d).length > 0, `scope_revision ${bad} was accepted`);
  }
});

// ---------------------------------------------------------------------------
// transition history
// ---------------------------------------------------------------------------
check('an empty transition history is rejected', () => {
  const d = mutate((x) => { payload(x).transition_history = []; });
  assert(evaluate(d).length > 0, 'an empty transition history was accepted');
});

check('the first record states the initial state explicitly (from_stage and from_status null)', () => {
  fixtures.valid.forEach((c, i) => {
    const first = c.spec.payload.transition_history[0];
    assert(first.from_stage === null && first.from_status === null, `valid[${i}] (${c.note}) first transition does not state the initial state`);
  });
  const d = mutate((x) => { payload(x).transition_history[0].from_stage = 'intake'; payload(x).transition_history[0].from_status = 'planned'; });
  assert(evaluate(d).some((m) => /first record but names a prior from_stage/.test(m)), 'a first record with a prior state was accepted');
});

check('a repeated or decreasing sequence ordinal is rejected', () => {
  const dup = mutate((x) => { payload(x).transition_history[2].sequence = payload(x).transition_history[1].sequence; });
  assert(evaluate(dup).some((m) => /repeats sequence ordinal/.test(m)), 'a repeated sequence ordinal was accepted');
  const dec = mutate((x) => { payload(x).transition_history[3].sequence = 1; });
  assert(evaluate(dec).some((m) => /below the previous/.test(m)), 'a decreasing sequence ordinal was accepted');
});

check('a break in the from/to chain is rejected', () => {
  const d = mutate((x) => { payload(x).transition_history[2].from_stage = 'planning'; });
  assert(evaluate(d).some((m) => /does not continue the previous to_stage/.test(m)), 'a broken from/to chain was accepted');
});

check('the last record must match the current lifecycle_stage, work_status and scope_revision', () => {
  const stageMismatch = mutate((x) => { payload(x).lifecycle_stage = 'verification'; });
  assert(evaluate(stageMismatch).some((m) => /does not match the current lifecycle_stage/.test(m)), 'a last-record stage mismatch was accepted');
  const statusMismatch = mutate((x) => { payload(x).work_status = 'waiting_human'; payload(x).next_action = 'owner-does-something'; });
  assert(evaluate(statusMismatch).some((m) => /does not match the current work_status/.test(m)), 'a last-record status mismatch was accepted');
  const revMismatch = mutate((x) => { payload(x).scope_revision = 7; });
  assert(evaluate(revMismatch).some((m) => /does not match the current scope_revision/.test(m)), 'a last-record scope_revision mismatch was accepted');
});

check('a decreasing per-transition scope_revision is rejected', () => {
  const d = mutate((x) => {
    payload(x).transition_history[2].scope_revision = 5;
    payload(x).transition_history[3].scope_revision = 2;
  });
  assert(evaluate(d).some((m) => /is below the previous/.test(m)), 'a decreasing per-transition scope_revision was accepted');
});

check('every transition states a reason', () => {
  const d = mutate((x) => { delete payload(x).transition_history[2].reason; });
  assert(evaluate(d).some((m) => /states no reason/.test(m)), 'a transition with no reason was accepted');
});

check('a silently skipped lifecycle stage is rejected; an explicit inapplicability basis is accepted', () => {
  const skipFixture = fixtures.valid.find((c) => c.spec.payload.transition_history.some((t) => Array.isArray(t.skipped_stages) && t.skipped_stages.length));
  assert(skipFixture, 'the valid set has no fixture that explicitly marks a stage inapplicable');
  const d = mutate((x) => {
    const h = payload(x).transition_history;
    h[h.length - 1].to_stage = 'acceptance';
    payload(x).lifecycle_stage = 'acceptance';
  });
  assert(evaluate(d).some((m) => /jumps over "verification" with no explicit inapplicability basis/.test(m)),
    'a silent stage skip was accepted');
});

check('a backward lifecycle move needs a backward_rationale; with one it is accepted', () => {
  const backFixture = fixtures.valid.find((c) => c.spec.payload.transition_history.some((t) => t.backward_rationale));
  assert(backFixture, 'the valid set has no fixture with an explicitly justified backward move');
  const d = mutate((x) => {
    const h = payload(x).transition_history;
    h.push({ sequence: h.at(-1).sequence + 1, from_stage: 'execution', from_status: 'active', to_stage: 'planning', to_status: 'active', scope_revision: h.at(-1).scope_revision, reason: 'возврат' });
    payload(x).lifecycle_stage = 'planning';
  });
  assert(evaluate(d).some((m) => /moves back from "execution" to "planning" with no backward_rationale/.test(m)),
    'a silent backward move was accepted');
});

check('a transition after a completed or cancelled state needs a reopen_rationale', () => {
  const reopenFixture = fixtures.valid.find((c) => c.spec.payload.transition_history.some((t) => t.reopen_rationale));
  assert(reopenFixture, 'the valid set has no fixture with an explicitly justified reopen');
  const d = mutate((x) => {
    const h = payload(x).transition_history;
    const last = h.at(-1);
    last.to_status = 'completed';
    h.push({ sequence: last.sequence + 1, from_stage: last.to_stage, from_status: 'completed', to_stage: last.to_stage, to_status: 'active', scope_revision: last.scope_revision, reason: 'продолжение' });
    payload(x).work_status = 'active';
    payload(x).next_action = 'continue';
  });
  assert(evaluate(d).some((m) => /follows a completed or cancelled state with no reopen_rationale/.test(m)),
    'a silent transition after a terminal state was accepted');
});

check('REGRESSION: reopen_rationale is owed only by the transition directly after a terminal record — a later ordinary transition does not repeat it', () => {
  // completed → active WITH a non-empty reopen_rationale → an ordinary active
  // transition WITHOUT one. The whole record must be accepted: terminality is a
  // property of the immediately previous history record, not a state that, once
  // seen, sticks to every remaining transition.
  const d = mutate((x) => {
    const h = payload(x).transition_history;
    const term = h.at(-1);
    term.to_status = 'completed';
    h.push({
      sequence: term.sequence + 1,
      from_stage: term.to_stage,
      from_status: 'completed',
      to_stage: term.to_stage,
      to_status: 'active',
      scope_revision: term.scope_revision,
      reason: 'Запуск возобновлён по решению владельца.',
      reopen_rationale: 'Обнаружен пропущенный пограничный случай; владелец санкционировал доработку.',
    });
    h.push({
      sequence: term.sequence + 2,
      from_stage: term.to_stage,
      from_status: 'active',
      to_stage: term.to_stage,
      to_status: 'active',
      scope_revision: term.scope_revision,
      reason: 'Продолжение работ после возобновления.',
    });
    payload(x).work_status = 'active';
  });
  const p = evaluate(d);
  assert(p.length === 0, `a normal transition after an already-justified reopen was rejected: ${p[0]}`);
});

check('a rooted machine path in a transition reason breaks portability', () => {
  const d = mutate((x) => { payload(x).transition_history[2].reason = 'Норма прочитана из /etc/example/policy.'; });
  assert(evaluate(d).some((m) => /reason contains a rooted \(absolute\) POSIX path/.test(m)), 'a rooted path in a reason was accepted');
});

check('the history stores run state, not a command journal — a command_log field on a transition is rejected', () => {
  const d = mutate((x) => { payload(x).transition_history[2].command_log = ['ls']; });
  assert(evaluate(d).length > 0, 'a command_log inside a transition was accepted');
});

check('an ordinary relative path in a reason is NOT flagged', () => {
  const d = mutate((x) => { payload(x).transition_history[1].reason = 'Затронут только src/config/parser и packages/@scope/module.'; });
  assert(evaluate(d).length === 0, `a portable relative path in a reason was rejected: ${evaluate(d)[0]}`);
});

check('stageIndex orders the closed lifecycle', () => {
  assert(stageIndex('intake') === 0 && stageIndex('completion') === 10, 'stageIndex does not order the lifecycle');
  assert(stageIndex('nope') === -1, 'stageIndex did not return -1 for an unknown stage');
  for (let i = 1; i < LIFECYCLE_STAGES.length; i++) {
    assert(stageIndex(LIFECYCLE_STAGES[i]) === stageIndex(LIFECYCLE_STAGES[i - 1]) + 1, 'the lifecycle order has a gap');
  }
});

// ---------------------------------------------------------------------------
// portability — the reused rooted-path detection still fires
// ---------------------------------------------------------------------------
check('the reused nonPortableReason still catches rooted machine paths and passes relative ones', () => {
  assert(nonPortableReason('/data/private/build') !== null, 'a rooted path was treated as portable');
  assert(nonPortableReason('см. ~/build') !== null, 'a "~/" reference was treated as portable');
  assert(nonPortableReason('см. file:///opt/thing') !== null, 'a file:// URL was treated as portable');
  assert(nonPortableReason('src/config/parser') === null, 'a repo-relative path was flagged');
  assert(nonPortableReason('task-specification:example-clarify-null-handling') === null, 'an opaque identifier was flagged');
});

check('the schema carries no rooted machine path', () => {
  const schemaText = loadText('registries/operating-model/execution-state.schema.json');
  assert(!/\s\/[a-z]+\/[a-z]/i.test(schemaText.replace(/https?:\/\//g, 'x')), 'the schema text appears to carry a rooted machine path');
});

// ---------------------------------------------------------------------------
if (failures.length) {
  console.log(`\n${failures.length} failing / ${passed} passing`);
  for (const f of failures) console.log(`  - ${f}`);
  process.exit(1);
}
console.log(`\nAll ${passed} checks passed.`);
