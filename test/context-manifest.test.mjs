#!/usr/bin/env node
// Standalone verification for the bounded context manifest
// (registries/operating-model/context-manifest.schema.json).
//
// It calls the same implementation the gate calls
// (scripts/lib/context-manifest.mjs) so the two cannot drift: the canonical
// scoped-record envelope (validated separately and always), the complete
// specialised schema a record declares in its own $schema, and the rules JSON
// Schema cannot state — the portable, logically-resolved $schema declaration;
// the DETERMINISTIC single-run link (closed structured pinned references,
// scope.id == execution_run_ref.id by exact comparison, human_control_ref bound
// to the same run, task_specification_ref carrying no run_id); the pinned
// run_state_checkpoint that repeats the three references field for field and
// whose resolved_norms fixes the applicable-norm set; the CLOSED exact-revision
// rule (a full Git SHA, a strict v?X.Y.Z tag, or a SHA-256 digest — everything
// else is floating or weak and needs a sha256) applied identically to pinned
// references, applicable norms and mutable sources; separate decision /
// question / action / check / gap / blocker lists with no duplicate entry; and
// rejection of an unbounded material dump or a full evidence / handoff /
// field-evaluation field that belongs to a later package.
//
// Usage: node test/context-manifest.test.mjs

import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';

import { assertSupportedDeep, UnsupportedSchema, validate } from '../scripts/lib/json-schema.mjs';
import {
  evaluateContextManifest,
  makeRecordResolver,
  nonPortableReason,
  resolveSchemaRef,
  classifyRevision,
  isFloatingRevision,
  ALLOWED_SCOPE_TYPE,
  CANONICAL_RECORD_BASE,
  EXPECTED_SCHEMA_BASENAME,
  EXPECTED_SCHEMA_REF,
  ENVELOPE_SCHEMA_REF,
  RECORD_TYPE,
  PINNED_REF_SLOTS,
  FORBIDDEN_PAYLOAD_FIELDS,
  FLOATING_REVISION_TOKENS,
  LIFECYCLE_STAGES,
  WORK_STATUSES,
  TERMINAL_STATUSES,
} from '../scripts/lib/context-manifest.mjs';

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

const recordSchema = loadJson('registries/operating-model/context-manifest.schema.json');
const envelopeSchema = loadJson('registries/operating-model/scoped-record.schema.json');
const fixtures = loadJson('registries/operating-model/fixtures/context-manifest.fixtures.json');

// The record resolver is EXTERNAL to every manifest.spec: it is built from the
// bundle's companion `resolution` set, never from fields inside the record under
// test. This is the boundary that anchors the run_state_checkpoint to the
// actual execution-run record.
const resolveRecords = makeRecordResolver(fixtures.resolution);
const opts = { recordSchema, envelopeSchema, resolveRecords };
const evaluate = (doc) => evaluateContextManifest(doc, opts);
// True when at least one problem string matches `re`.
const rejectsBecause = (doc, re) => evaluate(doc).some((m) => re.test(m));

// A topologically complete valid manifest record, the base for targeted
// mutations.
const BASE = clone(fixtures.valid[0].spec);
const payload = (d) => d.payload;
const cp = (d) => d.payload.run_state_checkpoint;
const mutate = (fn) => { const d = clone(BASE); fn(d); return d; };

// ---------------------------------------------------------------------------
// schema shape
// ---------------------------------------------------------------------------
check('both schemas use the supported JSON Schema subset', () => {
  assertSupportedDeep(recordSchema, 'context-manifest.schema.json');
  assertSupportedDeep(envelopeSchema, 'scoped-record.schema.json');
});

check('an unsupported schema keyword is rejected up front (assertSupportedDeep)', () => {
  const bad = clone(recordSchema);
  bad.definitions.payload.patternProperties = { '^x': { type: 'string' } };
  let threw = null;
  try { assertSupportedDeep(bad, 'context-manifest.schema.json'); }
  catch (e) { threw = e; }
  assert(threw instanceof UnsupportedSchema, 'assertSupportedDeep did not reject the unsupported keyword');
});

check('the specialised schema COMPOSES the envelope with the body (one pass, one model)', () => {
  assert(recordSchema.properties.record_type.const === RECORD_TYPE, 'record_type is not pinned to context-manifest');
  assert(recordSchema.additionalProperties === false, 'the record schema is not a closed object');
  for (const k of ['$schema', 'schema_version', 'id', 'title', 'record_type', 'scope', 'origin', 'authority', 'payload']) {
    assert(recordSchema.required.includes(k), `the record schema does not require the envelope field "${k}"`);
  }
  assert(recordSchema.definitions.payload.additionalProperties === false, 'payload is not a closed object');
  for (const k of [
    'execution_run_ref', 'task_specification_ref', 'human_control_ref',
    'scope_revision', 'next_action', 'next_gate',
    'authoritative_sources', 'applicable_norms', 'decisions', 'open_questions',
    'completed_actions', 'completed_checks', 'known_gaps', 'blockers',
    'run_state_checkpoint',
  ]) {
    assert(recordSchema.definitions.payload.required.includes(k), `payload does not require "${k}"`);
  }
  for (const k of [
    'execution_run_ref', 'task_specification_ref', 'human_control_ref',
    'scope_revision', 'lifecycle_stage', 'work_status', 'next_action', 'next_gate',
    'blocker_ids', 'resolved_norms', 'completed_checks',
  ]) {
    assert(recordSchema.definitions.run_state_checkpoint.required.includes(k), `run_state_checkpoint does not require "${k}"`);
  }
  assert(recordSchema.definitions.pinned_ref.additionalProperties === false, 'pinned_ref is not a closed object');
  assert(JSON.stringify(recordSchema.definitions.scope_ref.properties.type.enum) === JSON.stringify([ALLOWED_SCOPE_TYPE]),
    'the schema scope enum is not exactly {run-state}');
  assert(recordSchema.definitions.payload.properties.authoritative_sources.minItems === 1,
    'authoritative_sources is not required non-empty');
});

check('the pinned_ref slots and the checkpoint reuse the execution-state pools', () => {
  assert(JSON.stringify(Object.keys(PINNED_REF_SLOTS).sort())
    === JSON.stringify(['execution_run_ref', 'human_control_ref', 'task_specification_ref']),
    'the pinned-reference slots are not the expected three');
  assert(PINNED_REF_SLOTS.execution_run_ref.recordType === 'execution-run', 'execution_run_ref slot record type wrong');
  assert(PINNED_REF_SLOTS.human_control_ref.runId === 'required', 'human_control_ref must require a run_id');
  assert(PINNED_REF_SLOTS.task_specification_ref.runId === 'forbidden', 'task_specification_ref must forbid a run_id');
  assert(JSON.stringify(recordSchema.definitions.lifecycle_stage.enum) === JSON.stringify(LIFECYCLE_STAGES),
    'the schema lifecycle_stage enum diverges from LIFECYCLE_STAGES');
  assert(JSON.stringify(recordSchema.definitions.work_status.enum) === JSON.stringify(WORK_STATUSES),
    'the schema work_status enum diverges from WORK_STATUSES');
  assert(TERMINAL_STATUSES.every((s) => WORK_STATUSES.includes(s)), 'a terminal status is not in the work-status pool');
});

check('the implementation source carries no NUL or control byte', () => {
  const buf = fs.readFileSync(path.join(root, 'scripts/lib/context-manifest.mjs'));
  assert(buf.indexOf(0) === -1, 'a NUL byte is present in scripts/lib/context-manifest.mjs');
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

check('an absent, envelope-only, bare-basename or out-of-namespace $schema is rejected', () => {
  assert(rejectsBecause(mutate((x) => { delete x.$schema; }), /declares no \$schema/), 'absent $schema accepted');
  assert(rejectsBecause(mutate((x) => { x.$schema = '../../registries/operating-model/scoped-record.schema.json'; }), /resolves to the record envelope/), 'envelope-only $schema accepted');
  assert(rejectsBecause(mutate((x) => { x.$schema = EXPECTED_SCHEMA_BASENAME; }), /does not resolve to the logical address/), 'bare basename accepted');
  assert(rejectsBecause(mutate((x) => { x.$schema = `../../../registries/operating-model/${EXPECTED_SCHEMA_BASENAME}`; }), /does not resolve to the logical address/), 'out-of-namespace $schema accepted');
});

check('an absolute machine path as $schema is rejected as non-portable', () => {
  assert(rejectsBecause(mutate((x) => { x.$schema = '/opt/meridian/registries/operating-model/context-manifest.schema.json'; }), /is not portable/),
    'an absolute $schema path was accepted');
  assert(CANONICAL_RECORD_BASE === 'records/context-manifest', 'the canonical logical base changed unexpectedly');
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

check('the valid set exercises the required positive scenarios', () => {
  const statuses = new Set(fixtures.valid.map((c) => c.spec.payload.run_state_checkpoint.work_status));
  for (const s of ['active', 'waiting_human', 'blocked', 'completed']) {
    assert(statuses.has(s), `no valid fixture exercises work_status "${s}"`);
  }
  assert(fixtures.valid.some((c) => c.spec.payload.next_action === null && c.spec.payload.next_gate === null),
    'no valid fixture states an explicit null next step and gate');
  // an exact pin by a full Git SHA, by a strict SemVer tag and by a lone SHA-256 digest — all three
  const allRefs = fixtures.valid.flatMap((c) => [
    c.spec.payload.execution_run_ref, c.spec.payload.task_specification_ref, c.spec.payload.human_control_ref,
    ...c.spec.payload.authoritative_sources,
  ]);
  assert(allRefs.some((r) => classifyRevision(r.revision) === 'exact' && /^[0-9a-f]{40}$/i.test(r.revision || '')),
    'no valid input pinned by a full Git SHA');
  assert(allRefs.some((r) => classifyRevision(r.revision) === 'exact' && /^v?\d+\.\d+\.\d+$/.test(r.revision || '')),
    'no valid input pinned by a strict SemVer tag');
  assert(allRefs.some((r) => !r.revision && typeof r.sha256 === 'string' && /^[0-9a-fA-F]{64}$/.test(r.sha256)),
    'no valid input pinned by a lone SHA-256 digest');
});

// ---------------------------------------------------------------------------
// 1. deterministic single-run link
// ---------------------------------------------------------------------------
check('the three references are CLOSED structured pinned objects, never plain strings', () => {
  for (const f of ['execution_run_ref', 'task_specification_ref', 'human_control_ref']) {
    assert(rejectsBecause(mutate((x) => { payload(x)[f] = 'run:example-run-001'; }), /is not a structured pinned reference/),
      `a plain-string ${f} was accepted`);
    assert(evaluate(mutate((x) => { payload(x)[f].extra_body_field = 'x'; })).length > 0,
      `an extra key on ${f} (embedded body) was accepted`);
  }
});

check('scope.id must EQUAL execution_run_ref.id exactly — no substring, prefix or middle match', () => {
  // exact different id
  assert(rejectsBecause(mutate((x) => {
    x.scope.id = 'example-run';
    payload(x).execution_run_ref.id = 'example-run-other';
    payload(x).execution_run_ref.reference = 'records/execution-run/example-run-other';
    cp(x).execution_run_ref = clone(payload(x).execution_run_ref);
    payload(x).human_control_ref.run_id = 'example-run-other';
    cp(x).human_control_ref.run_id = 'example-run-other';
  }), /the exact same string, not one a substring of the other/),
    'example-run vs example-run-other accepted as the same run');
  // prefix
  assert(rejectsBecause(mutate((x) => { x.scope.id = 'example-run-00'; }), /the exact same string/),
    'a scope.id that is only a prefix of the run id was accepted');
  // middle / substring
  assert(rejectsBecause(mutate((x) => { x.scope.id = 'run-001'; }), /the exact same string/),
    'a scope.id that is only a substring of the run id was accepted');
});

check('human_control_ref must belong to the same run and carry a run_id; task_specification_ref carries none', () => {
  assert(rejectsBecause(mutate((x) => {
    payload(x).human_control_ref.run_id = 'example-run-other';
    cp(x).human_control_ref = clone(payload(x).human_control_ref);
  }), /is not the referenced run|must belong to the same run/),
    'a human_control_ref pointing at another run was accepted');
  assert(rejectsBecause(mutate((x) => {
    delete payload(x).human_control_ref.run_id;
    delete cp(x).human_control_ref.run_id;
  }), /carries no run_id/),
    'a human_control_ref with no run_id was accepted');
  assert(rejectsBecause(mutate((x) => {
    payload(x).task_specification_ref.run_id = 'example-run-001';
    cp(x).task_specification_ref.run_id = 'example-run-001';
  }), /is not run-scoped and names no run_id/),
    'a task_specification_ref carrying a run_id was accepted');
});

check('a wrong record_type in a pinned-reference slot is rejected', () => {
  assert(rejectsBecause(mutate((x) => { payload(x).execution_run_ref.record_type = 'task-specification'; }),
    /names record_type "task-specification", not "execution-run"|not in enum/),
    'a mistyped pinned reference was accepted');
});

// ---------------------------------------------------------------------------
// 2. the checkpoint is bound to the EXTERNALLY RESOLVED run, not to a second
//    in-document copy of the same fields
// ---------------------------------------------------------------------------
check("the checkpoint's pinned references are resolved through the external boundary, not compared to a second copy", () => {
  // a checkpoint run reference pinned to an edition the resolver does not confirm
  assert(rejectsBecause(mutate((x) => {
    cp(x).execution_run_ref = { record_type: 'execution-run', id: payload(x).execution_run_ref.id, reference: payload(x).execution_run_ref.reference, revision: 'a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2' };
  }), /run_state_checkpoint\.execution_run_ref pins revision "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2" but the resolver confirmed edition "v0.5.0"/),
    'a checkpoint run reference pinned to an unconfirmed edition was accepted');
  // a checkpoint human-control reference that resolves to nothing
  assert(rejectsBecause(mutate((x) => { cp(x).human_control_ref.reference = 'records/run-human-control/example-run-001-copy'; }),
    /run_state_checkpoint\.human_control_ref does not resolve to an actual run-human-control through the external resolver/),
    'a checkpoint human-control reference that resolves to nothing was accepted');
});

check('a manifest that pins only the specification, with no pinned run or human-control input, cannot be built', () => {
  const d = mutate((x) => {
    delete payload(x).execution_run_ref;
    delete payload(x).human_control_ref;
    delete cp(x).execution_run_ref;
    delete cp(x).human_control_ref;
  });
  const p = evaluate(d);
  assert(p.some((m) => /execution_run_ref/.test(m)) && p.some((m) => /human_control_ref/.test(m)),
    'a spec-only manifest was accepted');
});

check('task_specification_ref must match the specification the RESOLVED execution-run record names', () => {
  assert(rejectsBecause(mutate((x) => {
    payload(x).task_specification_ref = { record_type: 'task-specification', id: 'example-other-spec', reference: 'records/task-specification/example-other-spec', sha256: '00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff' };
  }), /the resolved execution-run record names task specification "records\/task-specification\/example-clarify-null-handling"|task_specification_ref does not resolve to an actual task-specification/),
    'a task_specification_ref that disagrees with the resolved run was accepted');
});

check('applicable_norms must equal the resolved_norms of the pinned run — the manifest adds a rationale, not a norm', () => {
  assert(rejectsBecause(mutate((x) => { payload(x).applicable_norms = [payload(x).applicable_norms[0]]; }),
    /applicable_norms references do not match the pinned run state/),
    'an applicable-norm set smaller than resolved_norms was accepted');
  assert(rejectsBecause(mutate((x) => {
    payload(x).applicable_norms.push({ id: 'extra-norm', reference: 'standards/workspace/kernel-boundary.md', origin: 'built-in-methodology', revision: 'v0.5.0', applicability_rationale: 'r' });
  }), /applicable_norms references do not match the pinned run state/),
    'an applicable-norm not present in resolved_norms was accepted');
});

check('the manifest scope_revision, next step and next gate agree with the checkpoint', () => {
  assert(rejectsBecause(mutate((x) => { payload(x).scope_revision = cp(x).scope_revision + 1; }), /contradicts the pinned run state/),
    'a scope_revision that contradicts the checkpoint was accepted');
  assert(rejectsBecause(mutate((x) => { cp(x).next_action = 'something-else'; }), /next_action .* contradicts the pinned run state/),
    'a next_action that contradicts the checkpoint was accepted');
  assert(rejectsBecause(mutate((x) => { cp(x).next_gate = 'acceptance'; }), /next_gate .* contradicts the pinned run state/),
    'a next_gate that contradicts the checkpoint was accepted');
});

check('the manifest blockers and completed checks match the checkpoint sets, and the checkpoint obeys the axis rules', () => {
  const blocked = fixtures.valid.find((c) => c.spec.payload.run_state_checkpoint.work_status === 'blocked');
  assert(blocked, 'the valid set has no blocked fixture');
  const d = clone(blocked.spec);
  d.payload.run_state_checkpoint.blocker_ids = ['different-id'];
  assert(rejectsBecause(d, /blockers do not match the pinned run state/),
    'a manifest whose blockers do not match the checkpoint blocker_ids was accepted');
  assert(rejectsBecause(mutate((x) => { cp(x).completed_checks = []; }), /completed_checks do not match the pinned run state/),
    'a manifest whose completed_checks do not match the checkpoint was accepted');
  assert(rejectsBecause(mutate((x) => { cp(x).work_status = 'blocked'; }), /work_status is "blocked" with no blocker id/),
    'a blocked checkpoint with no blocker id was accepted');
  assert(rejectsBecause(mutate((x) => {
    cp(x).work_status = 'completed';
    cp(x).lifecycle_stage = 'completion';
  }), /a completed or cancelled run does not silently carry a next executable step/),
    'a completed checkpoint carrying a next step was accepted');
  assert(rejectsBecause(mutate((x) => {
    cp(x).work_status = 'waiting_human';
    cp(x).next_action = null;
    payload(x).next_action = null;
  }), /"waiting_human" means a specific required human action is known/),
    'a waiting_human checkpoint with no next action was accepted');
});

// ---------------------------------------------------------------------------
// 3. the closed exact-revision rule
// ---------------------------------------------------------------------------
check('classifyRevision recognises exactly the closed set of exact revisions', () => {
  for (const r of ['a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2', '0'.repeat(40), '0'.repeat(64), 'v0.5.0', '0.5.0', 'v12.0.34']) {
    assert(classifyRevision(r) === 'exact', `classifyRevision("${r}") should be exact`);
  }
  for (const r of ['feature/package-2', 'release/0.6.0', 'refs/heads/dev-2', 'main', 'develop', 'trunk', 'HEAD', 'latest', 'stable']) {
    assert(classifyRevision(r) === 'floating', `classifyRevision("${r}") should be floating`);
    assert(isFloatingRevision(r), `isFloatingRevision("${r}") should be true`);
  }
  for (const r of ['branch-2026', '2026-09-01', 'rev_42', 'v1.2', '1.2.3-rc1']) {
    assert(classifyRevision(r) === 'weak', `classifyRevision("${r}") should be weak`);
  }
  assert(classifyRevision('a1b2c3d') === 'abbrev-sha', 'an abbreviated hex SHA should classify as abbrev-sha');
  assert(FLOATING_REVISION_TOKENS.has('head'), 'the floating-token set lost "head"');
});

check('the SAME pin rule is applied to pinned references, applicable norms and mutable sources', () => {
  // pinned reference — floating branch
  assert(rejectsBecause(mutate((x) => {
    payload(x).execution_run_ref.revision = 'feature/package-2';
    delete payload(x).execution_run_ref.sha256;
    cp(x).execution_run_ref = clone(payload(x).execution_run_ref);
  }), /execution_run_ref is not pinned to an exact edition: revision "feature\/package-2" is a branch or channel reference/),
    'execution_run_ref pinned only by a branch reference was accepted');
  // pinned reference — abbreviated SHA
  assert(rejectsBecause(mutate((x) => {
    payload(x).execution_run_ref.revision = 'a1b2c3d';
    cp(x).execution_run_ref = clone(payload(x).execution_run_ref);
  }), /looks like an abbreviated or ambiguous Git SHA/),
    'execution_run_ref pinned only by an abbreviated SHA was accepted');
  // applicable norm — branch with a digit
  assert(rejectsBecause(mutate((x) => {
    payload(x).applicable_norms[0].revision = 'release/0.6.0';
    delete payload(x).applicable_norms[0].sha256;
  }), /applicable norm .* is not pinned: revision "release\/0.6.0" is a branch or channel reference/),
    'an applicable norm pinned only by "release/0.6.0" was accepted');
  // applicable norm — weak provider form
  assert(rejectsBecause(mutate((x) => {
    payload(x).applicable_norms[0].revision = 'branch-2026';
    delete payload(x).applicable_norms[0].sha256;
  }), /applicable norm .* is not pinned: revision "branch-2026" is not a recognised exact revision/),
    'an applicable norm pinned only by "branch-2026" was accepted');
  // mutable source — feature branch
  assert(rejectsBecause(mutate((x) => {
    payload(x).authoritative_sources[0].revision = 'feature/package-2';
    delete payload(x).authoritative_sources[0].sha256;
  }), /is mutable but not pinned: revision "feature\/package-2" is a branch or channel reference/),
    'a mutable source pinned only by "feature/package-2" was accepted');
  // mutable source — refs/heads
  assert(rejectsBecause(mutate((x) => { payload(x).authoritative_sources[0].revision = 'refs/heads/dev-2'; }),
    /is mutable but not pinned: revision "refs\/heads\/dev-2" is a branch or channel reference/),
    'a mutable source pinned only by "refs/heads/dev-2" was accepted');
  // mutable source — unknown provider form
  assert(rejectsBecause(mutate((x) => { payload(x).authoritative_sources[0].revision = 'branch-2026'; }),
    /is mutable but not pinned: revision "branch-2026" is not a recognised exact revision/),
    'a mutable source pinned only by "branch-2026" was accepted');
});

check('a branch reference on an immutable source is still rejected', () => {
  assert(rejectsBecause(mutate((x) => { payload(x).authoritative_sources[2].revision = 'main'; }),
    /declared immutable but its revision "main" is a branch or channel reference/),
    'an immutable source whose revision is a branch was accepted');
});

check('a sha256 digest is always a sufficient pin, whatever the revision', () => {
  const d = mutate((x) => {
    payload(x).authoritative_sources[0].revision = 'feature/package-2';
    payload(x).authoritative_sources[0].sha256 = '00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff';
  });
  assert(evaluate(d).length === 0, `a mutable source with a branch label but a valid sha256 was rejected: ${evaluate(d)[0]}`);
});

// ---------------------------------------------------------------------------
// bounded sources / norms / lists
// ---------------------------------------------------------------------------
check('authoritative_sources must be non-empty and carry no duplicate id or reference', () => {
  assert(rejectsBecause(mutate((x) => { payload(x).authoritative_sources = []; }), /lists no authoritative sources|fewer than 1 items/),
    'an empty authoritative_sources list was accepted');
  assert(rejectsBecause(mutate((x) => {
    const s = payload(x).authoritative_sources;
    s.push({ ...clone(s[0]), reference: 'standards/workspace/other.md' });
  }), /authoritative source id ".*" is used more than once/),
    'a duplicate source id was accepted');
  assert(rejectsBecause(mutate((x) => {
    const s = payload(x).authoritative_sources;
    s.push({ ...clone(s[0]), id: 'other-id' });
  }), /authoritative source reference ".*" is listed more than once/),
    'a duplicate source reference was accepted');
});

check('every applicable norm carries a reference, an origin, a pin and an applicability rationale; no duplicate id', () => {
  assert(rejectsBecause(mutate((x) => { delete payload(x).applicable_norms[0].applicability_rationale; }),
    /applicable norm .* has no applicability rationale/),
    'an applicable norm with no rationale was accepted');
  assert(rejectsBecause(mutate((x) => {
    delete payload(x).applicable_norms[0].revision;
    delete payload(x).applicable_norms[0].sha256;
  }), /applicable norm .* is not pinned/),
    'an unpinned applicable norm was accepted');
  assert(rejectsBecause(mutate((x) => {
    payload(x).applicable_norms[1].id = payload(x).applicable_norms[0].id;
  }), /applicable norm id ".*" is used more than once/),
    'a duplicate applicable norm id was accepted');
});

check('decisions, questions, gaps, actions and checks are separate lists with no duplicate entry', () => {
  assert(rejectsBecause(mutate((x) => { payload(x).decisions.push({ id: payload(x).decisions[0].id, statement: 'dup' }); }),
    /decisions id ".*" is used more than once/), 'a duplicate decision id was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).open_questions = [{ id: 'q', question: 'a' }, { id: 'q', question: 'b' }]; }),
    /open_questions id "q" is used more than once/), 'a duplicate open-question id was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).known_gaps = [{ id: 'g', description: 'a' }, { id: 'g', description: 'b' }]; }),
    /known_gaps id "g" is used more than once/), 'a duplicate known-gap id was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).completed_actions = ['action:a', 'action:a']; }),
    /completed_actions repeats reference|items are not unique/), 'a duplicate completed action was accepted');
  assert(rejectsBecause(mutate((x) => {
    payload(x).completed_checks = ['check:a', 'check:a'];
    cp(x).completed_checks = ['check:a', 'check:a'];
  }), /completed_checks repeats reference|items are not unique/), 'a duplicate completed check was accepted');
});

check('a blocker with no verifiable resumption condition is rejected', () => {
  const blocked = fixtures.valid.find((c) => c.spec.payload.run_state_checkpoint.work_status === 'blocked');
  const d = clone(blocked.spec);
  delete d.payload.blockers[0].resumption_condition;
  assert(rejectsBecause(d, /blocker .* has no verifiable resumption condition|resumption_condition.*required/),
    'a blocker with no resumption condition was accepted');
});

// ---------------------------------------------------------------------------
// scope / origin / boundedness
// ---------------------------------------------------------------------------
check('a manifest lives only in run-state, with a workspace_id', () => {
  for (const [t, extra] of [
    ['project-workspace', { id: 'example-workspace' }],
    ['repository-scope', { id: 'example-repo', workspace_id: 'example-workspace' }],
    ['built-in-methodology', { id: 'built-in-methodology' }],
    ['user-profile', { id: 'example-user' }],
    ['organization-profile', { id: 'example-org' }],
  ]) {
    const d = mutate((x) => { x.scope = { type: t, ...extra }; });
    assert(evaluate(d).some((m) => new RegExp(`scoped to "${t}"`).test(m) || /^envelope /.test(m) || /not in enum/.test(m)),
      `scope "${t}" was accepted for a manifest`);
  }
  assert(rejectsBecause(mutate((x) => { delete x.scope.workspace_id; }), /workspace_id/),
    'a run-state manifest with no workspace_id was accepted');
  assert(rejectsBecause(mutate((x) => { x.origin = { kind: 'built-in' }; }), /origin\.kind "built-in"|source_ref/),
    'a built-in origin was accepted for a manifest');
});

check('scope_revision must be a positive integer', () => {
  assert(rejectsBecause(mutate((x) => { payload(x).scope_revision = 0; cp(x).scope_revision = 0; }),
    /scope_revision "0" is not a positive integer|below minimum/),
    'a zero scope_revision was accepted');
});

check('an unbounded material dump is rejected by construction', () => {
  for (const f of ['command_log', 'transcript', 'chat_history', 'file_contents', 'directory_dump']) {
    assert(evaluate(mutate((x) => { payload(x)[f] = f === 'command_log' ? ['a'] : 'a'; })).length > 0,
      `an unbounded "${f}" field was accepted`);
  }
});

check('a full evidence / handoff / field-evaluation field belongs to a later package and is rejected', () => {
  for (const f of ['evidence', 'handoff', 'worktree_state', 'verification_verdict', 'field_metrics']) {
    assert(evaluate(mutate((x) => { payload(x)[f] = 'x'; })).length > 0, `a "${f}" field was accepted`);
  }
  assert(FORBIDDEN_PAYLOAD_FIELDS.includes('handoff') && FORBIDDEN_PAYLOAD_FIELDS.includes('command_log')
    && FORBIDDEN_PAYLOAD_FIELDS.includes('role_assignments'),
    'the forbidden-field list lost a load-bearing entry');
});

// ---------------------------------------------------------------------------
// portability
// ---------------------------------------------------------------------------
check('the reused nonPortableReason still catches rooted machine paths and passes relative ones', () => {
  assert(nonPortableReason('/data/private/build') !== null, 'a rooted path was treated as portable');
  assert(nonPortableReason('см. ~/build') !== null, 'a "~/" reference was treated as portable');
  assert(nonPortableReason('см. file:///opt/thing') !== null, 'a file:// URL was treated as portable');
  assert(nonPortableReason('records/task-specification/example') === null, 'a repo-relative path was flagged');
  assert(nonPortableReason('execution-run:example-run-001') === null, 'an opaque identifier was flagged');
});

check('a rooted machine path in a reference, a purpose or a rationale breaks portability', () => {
  assert(rejectsBecause(mutate((x) => { payload(x).authoritative_sources[0].reference = '/home/example/meridian/x.md'; }),
    /reference contains a rooted \(absolute\) POSIX path/), 'a rooted path in a source reference was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).authoritative_sources[0].purpose = 'см. /etc/example/policy'; }),
    /purpose contains a rooted \(absolute\) POSIX path/), 'a rooted path in a source purpose was accepted');
  assert(rejectsBecause(mutate((x) => { payload(x).execution_run_ref.reference = '/opt/runs/example-run-001'; }),
    /execution_run_ref reference contains a rooted \(absolute\) POSIX path/), 'a rooted execution_run_ref reference was accepted');
});

check('neither the schema carries a rooted machine path', () => {
  const text = loadText('registries/operating-model/context-manifest.schema.json').replace(/https?:\/\//g, 'x');
  assert(!/\s\/[a-z]+\/[a-z]/i.test(text), 'the schema appears to carry a rooted machine path');
});

check('the shipped normative document and schema agree on the record type and scope', () => {
  const norm = loadText('standards/workspace/bounded-context-manifest.md');
  assert(/context-manifest/.test(norm), 'the normative document does not name the record type');
  assert(/run-state/.test(norm), 'the normative document does not name the run-state scope');
  assert(/run_state_checkpoint/.test(norm), 'the normative document does not describe the pinned checkpoint');
  assert(/resolv/i.test(norm), 'the normative document does not describe external resolution of the pinned records');
  assert(validate && typeof validate === 'function', 'the generic validator is unavailable');
});

// ---------------------------------------------------------------------------
// REGRESSION — CHANGES_REQUESTED: the run_state_checkpoint used to be checked
// only against a second copy of the same fields inside the manifest, so a
// coordinated edit of BOTH copies, and a fabricated run sha256, passed. The
// checkpoint is now resolved against the ACTUAL execution-run record through a
// boundary the manifest does not control. Each assertion below FAILED on the
// pre-fix implementation (it compared payload.X to run_state_checkpoint.X and
// found them equal) and passes now.
// ---------------------------------------------------------------------------
check('regression: replacing BOTH copies of task_specification_ref with one consistent fake is rejected', () => {
  const d = mutate((x) => {
    const fake = { record_type: 'task-specification', id: 'ghost-spec', reference: 'records/task-specification/ghost-spec', sha256: '00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff' };
    payload(x).task_specification_ref = clone(fake);
    cp(x).task_specification_ref = clone(fake);
  });
  assert(rejectsBecause(d, /task_specification_ref does not resolve to an actual task-specification|the resolved execution-run record names task specification/),
    'two matching copies of a fabricated task_specification_ref were accepted');
});

check('regression: replacing BOTH copies of execution_run_ref.sha256 with a fabricated digest is rejected', () => {
  const fake = 'deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef00';
  const d = mutate((x) => {
    payload(x).execution_run_ref.sha256 = fake;
    cp(x).execution_run_ref.sha256 = fake;
  });
  assert(rejectsBecause(d, /execution_run_ref pins sha256 "deadbeef.*" but the SHA-256 of the resolved source is/),
    'a fabricated run digest, duplicated into the checkpoint, was accepted');
});

check('regression: a coordinated edit of snapshot AND manifest fields, with the resolved run unchanged, is rejected', () => {
  const d = mutate((x) => {
    // both in-document copies stay consistent with each other …
    payload(x).scope_revision = 7;
    cp(x).scope_revision = 7;
    payload(x).next_action = 'smuggle-in-a-different-step';
    cp(x).next_action = 'smuggle-in-a-different-step';
  });
  // … but the resolved execution-run record still says scope_revision 2.
  assert(rejectsBecause(d, /run_state_checkpoint\.scope_revision 7 does not match the resolved execution-run record \(2\)/),
    'a coordinated edit of both copies was accepted because the resolved run was not consulted');
});

check('regression: changing resolved_norms in BOTH internal structures, with the resolved run unchanged, is rejected', () => {
  const d = mutate((x) => {
    payload(x).applicable_norms = [{
      id: 'smuggled-norm', reference: 'standards/workspace/kernel-boundary.md',
      origin: 'built-in-methodology', revision: 'v0.5.0',
      applicability_rationale: 'внесено согласованно в обе внутренние структуры',
    }];
    cp(x).resolved_norms = ['standards/workspace/kernel-boundary.md'];
  });
  assert(rejectsBecause(d, /run_state_checkpoint\.resolved_norms does not match the resolved execution-run record's resolved_norms/),
    'a coordinated resolved_norms edit was accepted because the resolved run was not consulted');
});

check('regression: a missing or non-matching resolved source fails closed with a precise reason', () => {
  // no resolver at all
  const noResolver = evaluateContextManifest(clone(BASE), { recordSchema, envelopeSchema });
  assert(noResolver.some((m) => /cannot be verified: no external record resolver was supplied/.test(m)),
    'a manifest with no external resolver was not failed closed');
  // a resolver that confirms a different edition than the manifest pins
  const wrongEdition = makeRecordResolver({
    ...fixtures.resolution,
    'records/execution-run/example-run-001': {
      ...fixtures.resolution['records/execution-run/example-run-001'],
      revision: 'v9.9.9',
    },
  });
  const p = evaluateContextManifest(clone(BASE), { recordSchema, envelopeSchema, resolveRecords: wrongEdition });
  assert(p.some((m) => /execution_run_ref pins revision "v0.5.0" but the resolver confirmed edition "v9.9.9"/.test(m)),
    'a manifest whose pin disagrees with the resolved edition was accepted');
});

// ---------------------------------------------------------------------------
// REGRESSION — CHANGES_REQUESTED (round 2): the external boundary was open —
// the checkpoint's OWN resolved references were discarded (axes were compared
// to the top-level run, so the checkpoint could point at another real run), and
// resolved_state / linked_run_ref / their fields were only checked when present
// (an incomplete transformer response passed clean). Each assertion below
// FAILED before this round and passes now; each pins the SPECIFIC cause.
// ---------------------------------------------------------------------------
const evalWith = (doc, resolver) => evaluateContextManifest(doc, { recordSchema, envelopeSchema, resolveRecords: resolver });
// a resolver built from the real set with ONE entry mutated in place
const resolutionWith = (ref, mutateEntry) => {
  const m = clone(fixtures.resolution);
  mutateEntry(m[ref]);
  return makeRecordResolver(m);
};

check('regression: run_state_checkpoint.execution_run_ref pointing at ANOTHER existing run is rejected', () => {
  const d = mutate((x) => {
    cp(x).execution_run_ref = { record_type: 'execution-run', id: 'example-run-await-owner', reference: 'records/execution-run/example-run-await-owner', revision: 'v0.5.0' };
  });
  const p = evaluate(d);
  assert(p.some((m) => /run_state_checkpoint\.execution_run_ref resolves to record "example-run-await-owner", not the manifest's "example-run-001"/.test(m)),
    'a checkpoint run reference resolving to a different real run was accepted');
});

check('regression: run_state_checkpoint.task_specification_ref pointing at ANOTHER existing specification is rejected', () => {
  const d = mutate((x) => {
    cp(x).task_specification_ref = { record_type: 'task-specification', id: 'example-change-rounding-rule', reference: 'records/task-specification/example-change-rounding-rule', sha256: '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef' };
  });
  const p = evaluate(d);
  assert(p.some((m) => /run_state_checkpoint\.task_specification_ref resolves to record "example-change-rounding-rule", not the manifest's "example-clarify-null-handling"/.test(m)),
    'a checkpoint specification reference resolving to a different real specification was accepted');
});

check('regression: run_state_checkpoint.human_control_ref pointing at ANOTHER existing control record is rejected', () => {
  const d = mutate((x) => {
    cp(x).human_control_ref = { record_type: 'run-human-control', id: 'example-run-human-control-await-owner', run_id: 'example-run-001', reference: 'records/run-human-control/example-run-await-owner', revision: 'v0.5.0' };
  });
  const p = evaluate(d);
  assert(p.some((m) => /run_state_checkpoint\.human_control_ref resolves to record "example-run-human-control-await-owner", not the manifest's "example-run-human-control-001"/.test(m)),
    'a checkpoint control reference resolving to a different real control record was accepted');
});

check('regression: an execution-run resolved WITHOUT resolved_state is rejected, not skipped', () => {
  const r = resolutionWith('records/execution-run/example-run-001', (e) => { delete e.resolved_state; });
  const p = evalWith(clone(BASE), r);
  assert(p.some((m) => /the resolver returned an execution-run record with no resolved_state/.test(m)),
    'an execution-run with no resolved_state was accepted');
});

check('regression: resolved_state missing ANY required field is rejected, not skipped', () => {
  const REQUIRED = ['task_specification_ref', 'scope_revision', 'lifecycle_stage', 'work_status', 'next_action', 'next_gate', 'blocker_ids', 'resolved_norms', 'completed_checks'];
  for (const f of REQUIRED) {
    const r = resolutionWith('records/execution-run/example-run-001', (e) => { delete e.resolved_state[f]; });
    const p = evalWith(clone(BASE), r);
    assert(p.some((m) => m.includes(`resolved_state is missing required field "${f}"`)),
      `resolved_state without "${f}" was accepted`);
  }
});

check('regression: a run-human-control resolved WITHOUT linked_run_ref is rejected, not skipped', () => {
  const r = resolutionWith('records/run-human-control/example-run-001', (e) => { delete e.linked_run_ref; });
  const p = evalWith(clone(BASE), r);
  assert(p.some((m) => /the resolver returned a run-human-control record with no linked_run_ref/.test(m)),
    'a run-human-control with no linked_run_ref was accepted');
});

check('regression: a resolved record returned WITHOUT an id is rejected', () => {
  const r = resolutionWith('records/execution-run/example-run-001', (e) => { delete e.id; });
  const p = evalWith(clone(BASE), r);
  assert(p.some((m) => /the resolver returned a execution-run with no id; a resolved record without an identity cannot be checked against the pin/.test(m)),
    'a resolved record with no id was accepted');
});

check('regression: a resolved record whose stated reference does not match is rejected', () => {
  const r = resolutionWith('records/execution-run/example-run-001', (e) => { e.reference = 'records/execution-run/some-other-place'; });
  const p = evalWith(clone(BASE), r);
  assert(p.some((m) => /the resolver's stated reference "records\/execution-run\/some-other-place" is not the resolved reference "records\/execution-run\/example-run-001"/.test(m)),
    'a resolved record with a mismatched stated reference was accepted');
});

check('regression: the checkpoint that field-for-field diverges from the manifest is still an internal-consistency failure', () => {
  assert(rejectsBecause(mutate((x) => { cp(x).execution_run_ref.reference = 'records/execution-run/example-run-001-alias'; }),
    /run_state_checkpoint\.execution_run_ref does not equal the manifest's execution_run_ref field for field/),
    'a checkpoint reference diverging field-for-field from the manifest was accepted');
});

// ---------------------------------------------------------------------------
// REGRESSION — CHANGES_REQUESTED (round 3): the declared CLOSED transformer-
// response contract only checked that keys were PRESENT — not their types, and
// not that the entry (or its resolved_state) carried no extra key. A wrong-typed
// value then slipped past the later comparison (an Array.isArray / typeof guard
// read it as "nothing to compare") and the manifest validated clean. The
// contract is now actually closed: exactly the declared keys, every value type-
// and pool-checked, a wrong type producing a closed-contract error BEFORE any
// axis comparison. Each assertion below FAILED on the pre-fix implementation
// (the mutated manifest validated with zero problems) and passes now.
// ---------------------------------------------------------------------------
const sha256Hex = (s) => createHash('sha256').update(s, 'utf8').digest('hex');
// a resolver from the real set with example-run-001's resolved_state mutated
const withRunState = (mutateRs) => resolutionWith('records/execution-run/example-run-001', (e) => mutateRs(e.resolved_state));

check('regression: a resolved_state axis of the wrong type is a closed-contract error, not a skipped comparison', () => {
  // Each pair FAILED pre-fix: the wrong-typed axis was read past by a guard.
  const cases = [
    ['task_specification_ref', 42, /resolved_state\.task_specification_ref is not a non-empty string/],
    ['scope_revision', 'two', /resolved_state\.scope_revision "two" is not a positive integer/],
    ['lifecycle_stage', 7, /resolved_state\.lifecycle_stage 7 is not one of the closed lifecycle pool/],
    ['work_status', ['active'], /resolved_state\.work_status \["active"\] is not one of the closed work-status pool/],
    ['next_action', 5, /resolved_state\.next_action is present but neither null nor a non-empty string/],
    ['next_gate', {}, /resolved_state\.next_gate is present but neither null nor a non-empty string/],
    ['blocker_ids', 'not-an-array', /resolved_state\.blocker_ids is string, not an array of semantic-id strings/],
    ['resolved_norms', 'not-an-array', /resolved_state\.resolved_norms is string, not an array of portable-reference strings/],
    ['completed_checks', 'not-an-array', /resolved_state\.completed_checks is string, not an array of portable-reference strings/],
  ];
  for (const [f, val, re] of cases) {
    const p = evalWith(clone(BASE), withRunState((rs) => { rs[f] = val; }));
    assert(p.some((m) => re.test(m)), `resolved_state.${f} = ${JSON.stringify(val)} was not a closed-contract error`);
  }
});

check('regression: resolved_state arrays must hold unique portable / semantic members', () => {
  assert(evalWith(clone(BASE), withRunState((rs) => { rs.blocker_ids = ['dup-id', 'dup-id']; }))
    .some((m) => /resolved_state\.blocker_ids repeats "dup-id"/.test(m)), 'a duplicate blocker id was accepted');
  assert(evalWith(clone(BASE), withRunState((rs) => { rs.blocker_ids = ['Not A Semantic Id']; }))
    .some((m) => /resolved_state\.blocker_ids\[0\] "Not A Semantic Id" is not a stable semantic id/.test(m)), 'a non-semantic blocker id was accepted');
  assert(evalWith(clone(BASE), withRunState((rs) => { rs.resolved_norms = ['~/private/norm.md']; }))
    .some((m) => /resolved_state\.resolved_norms\[0\] contains/.test(m)), 'a non-portable resolved norm was accepted');
  assert(evalWith(clone(BASE), withRunState((rs) => { rs.completed_checks = ['check:a', 'check:a']; }))
    .some((m) => /resolved_state\.completed_checks repeats "check:a"/.test(m)), 'a duplicate completed check was accepted');
});

check('regression: an out-of-pool lifecycle_stage or work_status, or a non-positive scope_revision, in resolved_state is rejected', () => {
  assert(evalWith(clone(BASE), withRunState((rs) => { rs.lifecycle_stage = 'brainstorm'; }))
    .some((m) => /resolved_state\.lifecycle_stage "brainstorm" is not one of the closed lifecycle pool/.test(m)), 'an out-of-pool lifecycle_stage was accepted');
  assert(evalWith(clone(BASE), withRunState((rs) => { rs.work_status = 'paused'; }))
    .some((m) => /resolved_state\.work_status "paused" is not one of the closed work-status pool/.test(m)), 'an out-of-pool work_status was accepted');
  assert(evalWith(clone(BASE), withRunState((rs) => { rs.scope_revision = 0; }))
    .some((m) => /resolved_state\.scope_revision 0 is not a positive integer/.test(m)), 'a zero scope_revision was accepted');
});

check('regression: an empty next_action, next_gate or task_specification_ref in resolved_state is rejected', () => {
  assert(evalWith(clone(BASE), withRunState((rs) => { rs.next_action = ''; }))
    .some((m) => /resolved_state\.next_action is present but neither null nor a non-empty string/.test(m)), 'an empty next_action was accepted');
  assert(evalWith(clone(BASE), withRunState((rs) => { rs.next_gate = '   '; }))
    .some((m) => /resolved_state\.next_gate is present but neither null nor a non-empty string/.test(m)), 'a whitespace next_gate was accepted');
  assert(evalWith(clone(BASE), withRunState((rs) => { rs.task_specification_ref = ''; }))
    .some((m) => /resolved_state\.task_specification_ref is not a non-empty string/.test(m)), 'an empty task_specification_ref was accepted');
});

check('regression: a malformed content_digest in the resolved entry is rejected even with a revision present', () => {
  const r = resolutionWith('records/execution-run/example-run-001', (e) => { e.content_digest = 'not-a-sha256'; });
  const p = evalWith(clone(BASE), r);
  assert(p.some((m) => /execution_run_ref: the resolver's content_digest "not-a-sha256" is not exactly 64 hexadecimal characters/.test(m)),
    'a malformed content_digest alongside a valid revision was accepted');
});

check('regression: a non-string source_bytes in the resolved entry is rejected', () => {
  const r = resolutionWith('records/execution-run/example-run-001', (e) => { e.source_bytes = 123; });
  const p = evalWith(clone(BASE), r);
  assert(p.some((m) => /execution_run_ref: the resolver's source_bytes is number, not a string/.test(m)),
    'a non-string source_bytes was accepted');
});

check('regression: an unknown field on the resolved entry is rejected', () => {
  const r = resolutionWith('records/execution-run/example-run-001', (e) => { e.smuggled = 'x'; });
  const p = evalWith(clone(BASE), r);
  assert(p.some((m) => /execution_run_ref: the resolver returned a record with an unknown field "smuggled"; the transformer response is closed to \{/.test(m)),
    'an unknown field on the resolved entry was accepted');
});

check('regression: an unknown field in resolved_state is rejected', () => {
  const p = evalWith(clone(BASE), withRunState((rs) => { rs.smuggled_axis = 'x'; }));
  assert(p.some((m) => /resolved_state carries an unknown field "smuggled_axis"; resolved_state is closed to its 9 declared axes/.test(m)),
    'an unknown field in resolved_state was accepted');
});

check('the resolver SHA-256 is computed from source_bytes and accepted when it matches the pin', () => {
  const bytes = 'канонические исходные байты постановки задачи — пример';
  const digest = sha256Hex(bytes);
  const r = resolutionWith('records/task-specification/example-clarify-null-handling', (e) => {
    delete e.content_digest;
    e.source_bytes = bytes;
  });
  const d = mutate((x) => {
    payload(x).task_specification_ref.sha256 = digest;
    cp(x).task_specification_ref.sha256 = digest;
  });
  const p = evalWith(d, r);
  assert(p.length === 0, `a source_bytes-derived digest that matches the pin was rejected: ${p[0]}`);
});

check('source_bytes whose SHA-256 diverges from the stated content_digest is rejected', () => {
  const r = resolutionWith('records/task-specification/example-clarify-null-handling', (e) => {
    e.source_bytes = 'изменённые байты, не совпадают с заявленным дайджестом';
  });
  const p = evalWith(clone(BASE), r);
  assert(p.some((m) => /task_specification_ref: the resolver's content_digest ".*" does not match the SHA-256 of the resolved source bytes/.test(m)),
    'a source_bytes / content_digest divergence was accepted');
});

// ---------------------------------------------------------------------------
// REGRESSION — CHANGES_REQUESTED (round 4): a resolved `revision` was read
// through `typeof x === 'string' ? x.trim() : ''`, so a present-but-wrong key
// (a number, null, an object, "") was treated as ABSENT — and with a valid
// content_digest the entry then passed clean. And content_digest was tested
// after trim(), so a 64-hex string with surrounding spaces was accepted. The
// target here is a task-specification resolved entry, pinned ONLY by sha256, so
// a broken `revision` is the sole reason to fail. Each assertion FAILED on the
// round-3 implementation (the entry validated with zero problems / no
// revision-specific problem) and passes now, each pinning its concrete cause.
// ---------------------------------------------------------------------------
const TS_SPEC = 'records/task-specification/example-clarify-null-handling';
const evalTsRevision = (mutateEntry) => evalWith(clone(BASE), resolutionWith(TS_SPEC, mutateEntry));

check('regression: a present-but-non-string revision on a sha256-pinned resolved record is a precise error', () => {
  assert(evalTsRevision((e) => { e.revision = 42; })
    .some((m) => /task_specification_ref: the resolver's revision is a number, not a string/.test(m)),
    'revision = 42 was treated as an absent optional field');
  assert(evalTsRevision((e) => { e.revision = null; })
    .some((m) => /task_specification_ref: the resolver's revision is null, not a string/.test(m)),
    'revision = null was treated as an absent optional field');
  assert(evalTsRevision((e) => { e.revision = {}; })
    .some((m) => /task_specification_ref: the resolver's revision is an object, not a string/.test(m)),
    'revision = {} was treated as an absent optional field');
});

check('regression: a present-but-empty revision on a sha256-pinned resolved record is a precise error', () => {
  assert(evalTsRevision((e) => { e.revision = ''; })
    .some((m) => /task_specification_ref: the resolver's revision is present but empty/.test(m)),
    'revision = "" was treated as an absent optional field');
});

check('regression: a content_digest of 64 hex with surrounding whitespace is rejected as malformed', () => {
  const padded = ` ${clone(fixtures.resolution)[TS_SPEC].content_digest} `;
  assert(evalTsRevision((e) => { e.content_digest = padded; })
    .some((m) => /task_specification_ref: the resolver's content_digest ".*" is not exactly 64 hexadecimal characters/.test(m)),
    'a 64-hex content_digest with surrounding spaces was accepted');
});

// ---------------------------------------------------------------------------
if (failures.length) {
  console.log(`\n${failures.length} failing / ${passed} passing`);
  for (const f of failures) console.log(`  - ${f}`);
  process.exit(1);
}
console.log(`\nAll ${passed} checks passed.`);
