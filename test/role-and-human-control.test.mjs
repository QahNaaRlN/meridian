#!/usr/bin/env node
// Standalone verification for the role-and-human-control package:
// registries/operating-model/role-registry.schema.json (the built-in catalogue
// of universal roles) and registries/operating-model/human-control.schema.json
// (one execution run's human-control state).
//
// It calls the same implementation the gate calls
// (scripts/lib/role-and-human-control.mjs) so the two cannot drift: the reused
// scoped-record envelope (validated separately and always), each complete
// specialised schema a record declares in its own $schema, and the rules JSON
// Schema cannot state — the portable, logically-resolved $schema declaration;
// the built-in role catalogue confined to built-in-methodology with the seven
// universal roles present exactly once; a control record confined to run-state
// with a workspace_id and one portable execution_run_ref; permanent
// human-in-command bound to an assigned owner and neither a supervision mode
// nor disableable; human-in-the-loop with an hitl block that forbids hotl;
// human-on-the-loop with an hotl block that forbids hitl; independent role,
// supervision, authority, acting-actor and communication axes; combined roles
// for one actor; the base owner-plus-one-executor model with no reviewer; a
// required independence that needs a separate suitable actor and never the
// reviewed actor, a required:false block that carries no references, and
// portability of any present reference whatever the value of required; the
// acting actor and every switch to_acting_actor present in the matching
// role-assignment set; and the ordered switch history whose first record states
// the initial establishment, whose every later record continues the previous
// one on EVERY mutable axis with no break, and whose last record matches the
// whole current control state.
//
// Usage: node test/role-and-human-control.test.mjs

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { assertSupportedDeep, UnsupportedSchema, validate } from '../scripts/lib/json-schema.mjs';
import { yamlParse } from '../scripts/lib/yaml.mjs';
import {
  evaluateRoleRegistry,
  evaluateHumanControl,
  nonPortableReason,
  resolveSchemaRef,
  ROLES,
  SUPERVISION_MODES,
  COMMUNICATION_MODES,
  HUMAN_AUTHORITY_POSTURE,
  HUMAN_AUTHORITY_ROLE,
  INDEPENDENT_REVIEW_ROLES,
  CONTROL_AXES,
  ROLE_REGISTRY_RECORD_TYPE,
  HUMAN_CONTROL_RECORD_TYPE,
  REGISTRY_SCHEMA_REF,
  CONTROL_SCHEMA_REF,
  REGISTRY_SCHEMA_BASENAME,
  CONTROL_SCHEMA_BASENAME,
} from '../scripts/lib/role-and-human-control.mjs';

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

const registrySchema = loadJson('registries/operating-model/role-registry.schema.json');
const controlSchema = loadJson('registries/operating-model/human-control.schema.json');
const envelopeSchema = loadJson('registries/operating-model/scoped-record.schema.json');
const fixtures = loadJson('registries/operating-model/fixtures/role-and-human-control.fixtures.json');

const regOpts = { registrySchema, envelopeSchema };
const ctlOpts = { recordSchema: controlSchema, envelopeSchema };
const evalRegistry = (doc) => evaluateRoleRegistry(doc, regOpts);
const evalControl = (doc) => evaluateHumanControl(doc, ctlOpts);

// Topologically complete valid records, the base for targeted mutations.
const BASE_REGISTRY = clone(fixtures.registry.valid[0].spec);
const BASE_CONTROL = clone(fixtures.control.valid[0].spec);
const cpl = (d) => d.payload;
const lastSwitch = (d) => d.payload.switch_history[d.payload.switch_history.length - 1];
const mutateReg = (fn) => { const d = clone(BASE_REGISTRY); fn(d); return d; };
const mutateCtl = (fn) => { const d = clone(BASE_CONTROL); fn(d); return d; };

// ---------------------------------------------------------------------------
// schema shape
// ---------------------------------------------------------------------------
check('both new schemas use the supported JSON Schema subset', () => {
  assertSupportedDeep(registrySchema, 'role-registry.schema.json');
  assertSupportedDeep(controlSchema, 'human-control.schema.json');
});

check('an unsupported schema keyword is rejected up front (assertSupportedDeep)', () => {
  const bad = clone(controlSchema);
  bad.definitions.payload.patternProperties = { '^x': { type: 'string' } };
  let threw = null;
  try { assertSupportedDeep(bad, 'human-control.schema.json'); }
  catch (e) { threw = e; }
  assert(threw instanceof UnsupportedSchema, 'assertSupportedDeep did not reject the unsupported keyword');
});

check('each specialised schema COMPOSES the envelope with the body (one pass, one model)', () => {
  assert(registrySchema.properties.record_type.const === ROLE_REGISTRY_RECORD_TYPE, 'role-registry record_type is not pinned');
  assert(controlSchema.properties.record_type.const === HUMAN_CONTROL_RECORD_TYPE, 'human-control record_type is not pinned');
  for (const s of [registrySchema, controlSchema]) {
    assert(s.additionalProperties === false, 'a record schema is not a closed object');
    for (const k of ['$schema', 'schema_version', 'id', 'title', 'record_type', 'scope', 'origin', 'authority', 'payload']) {
      assert(s.required.includes(k), `a record schema does not require the envelope field "${k}"`);
    }
    assert(s.definitions.payload.additionalProperties === false, 'a payload is not a closed object');
  }
  assert(JSON.stringify(registrySchema.definitions.scope_ref.properties.type.enum) === JSON.stringify(['built-in-methodology']),
    'the role-registry scope enum is not exactly {built-in-methodology}');
  assert(JSON.stringify(controlSchema.definitions.scope_ref.properties.type.enum) === JSON.stringify(['run-state']),
    'the human-control scope enum is not exactly {run-state}');
  assert(controlSchema.definitions.scope_ref.required.includes('workspace_id'), 'human-control scope_ref does not require workspace_id');
});

check('the closed pools match the model', () => {
  assert(JSON.stringify(ROLES) === JSON.stringify(['owner', 'operator', 'executor', 'reviewer', 'verifier', 'git_integrator', 'deployer']),
    'the universal role pool is not the closed set of seven');
  assert(JSON.stringify(registrySchema.definitions.role_id.enum) === JSON.stringify(ROLES), 'the schema role_id enum diverges from ROLES');
  assert(JSON.stringify(controlSchema.definitions.role_id.enum) === JSON.stringify(ROLES), 'the control schema role_id enum diverges from ROLES');
  assert(JSON.stringify(SUPERVISION_MODES) === JSON.stringify(['human-in-the-loop', 'human-on-the-loop']),
    'the supervision-mode pool is not exactly {HITL, HOTL}');
  assert(!SUPERVISION_MODES.includes(HUMAN_AUTHORITY_POSTURE), 'human-in-command leaked into the switchable supervision pool');
  assert(JSON.stringify(controlSchema.definitions.supervision_mode.enum) === JSON.stringify(SUPERVISION_MODES),
    'the schema supervision_mode enum diverges from SUPERVISION_MODES');
  assert(JSON.stringify(COMMUNICATION_MODES) === JSON.stringify(['owner_relayed', 'direct']),
    'the communication-mode pool is not exactly {owner_relayed, direct}');
  assert(controlSchema.definitions.human_authority.properties.posture.const === HUMAN_AUTHORITY_POSTURE,
    'the schema does not pin human_authority.posture to human-in-command');
  assert(HUMAN_AUTHORITY_ROLE === 'owner', 'human-in-command is not bound to the owner role');
  assert(JSON.stringify(CONTROL_AXES) === JSON.stringify(['supervision_mode', 'communication_mode', 'acting_actor', 'role_assignments']),
    'the mutable control axes are not the expected four');
});

check('the implementation source carries no NUL or control byte', () => {
  const buf = fs.readFileSync(path.join(root, 'scripts/lib/role-and-human-control.mjs'));
  assert(buf.indexOf(0) === -1, 'a NUL byte is present in scripts/lib/role-and-human-control.mjs');
  for (let i = 0; i < buf.length; i++) {
    const b = buf[i];
    assert(b === 9 || b === 10 || b === 13 || b >= 32, `a control byte 0x${b.toString(16)} is present at offset ${i}`);
  }
});

// ---------------------------------------------------------------------------
// the declared $schema reaches the specialised contract, logically
// ---------------------------------------------------------------------------
check('every VALID fixture declares $schema resolving to its specialised schema', () => {
  fixtures.registry.valid.forEach((c, i) => {
    assert(resolveSchemaRef(c.spec.$schema) === REGISTRY_SCHEMA_REF, `registry.valid[${i}] (${c.note}) $schema does not resolve to ${REGISTRY_SCHEMA_REF}`);
  });
  fixtures.control.valid.forEach((c, i) => {
    assert(resolveSchemaRef(c.spec.$schema) === CONTROL_SCHEMA_REF, `control.valid[${i}] (${c.note}) $schema does not resolve to ${CONTROL_SCHEMA_REF}`);
  });
});

check('an absent, envelope-only, bare-basename or out-of-namespace $schema is rejected (both records)', () => {
  assert(evalRegistry(mutateReg((x) => { delete x.$schema; })).some((m) => /declares no \$schema/.test(m)), 'registry: absent $schema accepted');
  assert(evalControl(mutateCtl((x) => { delete x.$schema; })).some((m) => /declares no \$schema/.test(m)), 'control: absent $schema accepted');
  assert(evalRegistry(mutateReg((x) => { x.$schema = '../../registries/operating-model/scoped-record.schema.json'; })).some((m) => /resolves to the record envelope/.test(m)), 'registry: envelope-only $schema accepted');
  assert(evalControl(mutateCtl((x) => { x.$schema = '../../registries/operating-model/scoped-record.schema.json'; })).some((m) => /resolves to the record envelope/.test(m)), 'control: envelope-only $schema accepted');
  assert(evalRegistry(mutateReg((x) => { x.$schema = REGISTRY_SCHEMA_BASENAME; })).some((m) => /does not resolve to the logical address/.test(m)), 'registry: bare basename accepted');
  assert(evalControl(mutateCtl((x) => { x.$schema = `../../../registries/operating-model/${CONTROL_SCHEMA_BASENAME}`; })).some((m) => /does not resolve to the logical address/.test(m)), 'control: out-of-namespace $schema accepted');
});

check('an absolute machine path as $schema is rejected as non-portable', () => {
  assert(evalControl(mutateCtl((x) => { x.$schema = '/opt/meridian/registries/operating-model/human-control.schema.json'; })).some((m) => /is not portable/.test(m)),
    'an absolute $schema path was accepted');
});

// ---------------------------------------------------------------------------
// real fixtures
// ---------------------------------------------------------------------------
check('every valid fixture passes with no problem', () => {
  fixtures.registry.valid.forEach((c, i) => {
    const p = evalRegistry(c.spec);
    assert(p.length === 0, `registry.valid[${i}] (${c.note}) was rejected: ${p[0]}`);
  });
  fixtures.control.valid.forEach((c, i) => {
    const p = evalControl(c.spec);
    assert(p.length === 0, `control.valid[${i}] (${c.note}) was rejected: ${p[0]}`);
  });
});

check('every invalid fixture produces at least one problem', () => {
  fixtures.registry.invalid.forEach((c, i) => {
    assert(evalRegistry(c.spec).length > 0, `registry.invalid[${i}] (${c.note}) was accepted`);
  });
  fixtures.control.invalid.forEach((c, i) => {
    assert(evalControl(c.spec).length > 0, `control.invalid[${i}] (${c.note}) was accepted`);
  });
});

check('the shipped catalogue standards/workspace/role-registry.yaml is a valid role registry', () => {
  const cat = yamlParse(loadText('standards/workspace/role-registry.yaml'));
  const genErrs = [];
  validate(cat, registrySchema, registrySchema, '', genErrs);
  assert(genErrs.length === 0, `role-registry.yaml fails its declared schema: ${genErrs[0]}`);
  const p = evalRegistry(cat);
  assert(p.length === 0, `role-registry.yaml was rejected by the composition: ${p[0]}`);
});

check('the base control fixture exercises both supervision modes and both communication modes across the valid set', () => {
  const sup = new Set();
  const comm = new Set();
  for (const c of fixtures.control.valid) {
    sup.add(c.spec.payload.supervision_mode);
    comm.add(c.spec.payload.communication_mode);
  }
  for (const m of SUPERVISION_MODES) assert(sup.has(m), `no valid control fixture uses supervision_mode "${m}"`);
  for (const m of COMMUNICATION_MODES) assert(comm.has(m), `no valid control fixture uses communication_mode "${m}"`);
});

// ---------------------------------------------------------------------------
// the built-in role catalogue
// ---------------------------------------------------------------------------
check('the catalogue lives only in built-in-methodology', () => {
  const d = mutateReg((x) => { x.scope = { type: 'run-state', id: 'example-run', workspace_id: 'example-workspace' }; x.origin = { kind: 'declared', source_ref: 'owner-decision:x' }; x.authority = { kind: 'delegated-run', authority_ref: 'x' }; });
  assert(evalRegistry(d).some((m) => /built-in role catalogue is Kernel methodology/.test(m) || /^envelope /.test(m) || /not in enum/.test(m)),
    'a non-built-in-methodology scope was accepted for the catalogue');
});

check('all seven universal roles are present, none duplicated, none unknown', () => {
  assert(evalRegistry(mutateReg((x) => { cpl(x).roles = cpl(x).roles.slice(0, 6); })).some((m) => /missing universal role/.test(m) || /fewer than 7/.test(m)),
    'a catalogue missing a role was accepted');
  assert(evalRegistry(mutateReg((x) => { cpl(x).roles[1] = clone(cpl(x).roles[0]); })).some((m) => /declared more than once/.test(m) || /not unique/.test(m) || /missing universal role/.test(m)),
    'a catalogue with a duplicated role id was accepted');
  assert(evalRegistry(mutateReg((x) => { cpl(x).roles[6].id = 'auditor'; })).some((m) => /not one of the closed pool/.test(m) || /not in enum/.test(m)),
    'a catalogue with an unknown role id was accepted');
});

check('a role with no responsibilities, or a rooted path in a responsibility, is rejected', () => {
  assert(evalRegistry(mutateReg((x) => { cpl(x).roles[0].responsibilities = []; })).some((m) => /no responsibilities/.test(m) || /fewer than 1/.test(m)),
    'a role with no responsibilities was accepted');
  assert(evalRegistry(mutateReg((x) => { cpl(x).roles[0].responsibilities[0] = 'см. /etc/example/policy'; })).some((m) => /rooted \(absolute\) POSIX path/.test(m)),
    'a rooted machine path in a responsibility was accepted');
});

check('a concrete programme, model, vendor or person name is not part of a universal role (rejected as an extra field)', () => {
  assert(evalRegistry(mutateReg((x) => { cpl(x).roles[0].assigned_model = 'some-model'; })).length > 0, 'an extra field on a role was accepted');
  assert(evalRegistry(mutateReg((x) => { cpl(x).default_role = 'executor'; })).length > 0, 'an extra payload field was accepted');
});

check('git_integrator is defined as integration preparation and verification, not as always performing the final merge', () => {
  const cat = yamlParse(loadText('standards/workspace/role-registry.yaml'));
  const gi = cat.payload.roles.find((r) => r.id === 'git_integrator');
  assert(gi, 'the catalogue has no git_integrator role');
  const text = `${gi.summary} ${gi.responsibilities.join(' ')}`.toLowerCase();
  assert(/готов/.test(text) && /провер/.test(text), 'git_integrator is not described as preparing and verifying integration');
  assert(/назнач/.test(text) && /поток/.test(text), 'git_integrator does not say it performs only the Git operations the flow assigns');
  assert(/владельц/.test(text), 'git_integrator does not acknowledge the final merge may be handed to the owner');
  assert(!/всегда выполняет.*слияни/.test(text), 'git_integrator still claims it always performs the merge');
});

// ---------------------------------------------------------------------------
// scope and reference of one run's control record
// ---------------------------------------------------------------------------
check('a control record lives only in run-state, with a workspace_id', () => {
  for (const [t, extra] of [
    ['project-workspace', { id: 'example-workspace' }],
    ['repository-scope', { id: 'example-repo', workspace_id: 'example-workspace' }],
    ['built-in-methodology', { id: 'built-in-methodology' }],
    ['user-profile', { id: 'example-user' }],
    ['organization-profile', { id: 'example-org' }],
  ]) {
    const d = mutateCtl((x) => { x.scope = { type: t, ...extra }; });
    assert(evalControl(d).some((m) => new RegExp(`scoped to "${t}"`).test(m) || /^envelope /.test(m) || /not in enum/.test(m)),
      `scope "${t}" was accepted for a control record`);
  }
  assert(evalControl(mutateCtl((x) => { delete x.scope.workspace_id; })).some((m) => /workspace_id/.test(m)),
    'a run-state control record with no workspace_id was accepted');
});

check('a control record references exactly one execution run, portably, and never embeds it', () => {
  assert(evalControl(mutateCtl((x) => { delete cpl(x).execution_run_ref; })).some((m) => /names no execution_run_ref/.test(m) || /execution_run_ref.*required/.test(m)),
    'a control record with no execution_run_ref was accepted');
  assert(evalControl(mutateCtl((x) => { cpl(x).execution_run_ref = { id: 'example-run' }; })).length > 0,
    'an embedded run object was accepted as the reference');
  assert(evalControl(mutateCtl((x) => { cpl(x).execution_run_ref = '/opt/runs/example'; })).some((m) => /rooted \(absolute\) POSIX path/.test(m)),
    'an absolute execution_run_ref was accepted');
});

check('the control record carries no execution-run body axis (closed payload)', () => {
  for (const leaked of ['lifecycle_stage', 'work_status', 'blockers', 'transition_history', 'next_action', 'scope_revision']) {
    const d = mutateCtl((x) => { cpl(x)[leaked] = leaked === 'blockers' || leaked === 'transition_history' ? [] : 'x'; });
    assert(evalControl(d).length > 0, `the leaked execution-run field "${leaked}" was accepted in the control payload`);
  }
});

check('origin.kind built-in is rejected for a concrete control record', () => {
  assert(evalControl(mutateCtl((x) => { x.origin = { kind: 'built-in' }; })).length > 0, 'a built-in origin was accepted for a control record');
});

// ---------------------------------------------------------------------------
// human authority — HIC is permanent, not a mode, bound to an assigned owner
// ---------------------------------------------------------------------------
check('human-in-command is stated, is not a supervision mode, and cannot be disabled', () => {
  assert(evalControl(mutateCtl((x) => { delete cpl(x).human_authority; })).some((m) => /human-in-command is the permanent basis/.test(m) || /human_authority.*required/.test(m)),
    'a control record with no human_authority axis was accepted');
  assert(evalControl(mutateCtl((x) => { cpl(x).human_authority = { posture: 'human-on-the-loop', holder: cpl(x).role_assignments[0].actor }; })).some((m) => /human-in-command is the permanent basis/.test(m) || /must equal/.test(m)),
    'human-in-command replaced by a supervision value was accepted');
  const d = mutateCtl((x) => {
    cpl(x).supervision_mode = HUMAN_AUTHORITY_POSTURE;
    lastSwitch(x).to_supervision_mode = HUMAN_AUTHORITY_POSTURE;
  });
  assert(evalControl(d).some((m) => /not a switchable supervision mode/.test(m) || /not in enum/.test(m)),
    'human-in-command was accepted as a supervision_mode');
});

check('human-in-command is bound to a participant assigned the owner role', () => {
  assert(evalControl(mutateCtl((x) => { delete cpl(x).human_authority.holder; })).some((m) => /names no holder/.test(m) || /holder.*required/.test(m)),
    'a human_authority with no holder was accepted');
  const notOwner = mutateCtl((x) => { cpl(x).human_authority.holder = cpl(x).role_assignments[1].actor; });
  assert(evalControl(notOwner).some((m) => /is not a participant assigned the owner role/.test(m)),
    'a holder that is not an assigned owner was accepted');
  const unassigned = mutateCtl((x) => { cpl(x).human_authority.holder = 'actor:example-ghost'; });
  assert(evalControl(unassigned).some((m) => /is not a participant assigned the owner role/.test(m)),
    'a holder reference to an unassigned participant was accepted');
  const nonPortable = mutateCtl((x) => { cpl(x).human_authority.holder = '/opt/owners/x'; });
  assert(evalControl(nonPortable).some((m) => /human_authority.holder contains/.test(m)),
    'a non-portable holder reference was accepted');
});

// ---------------------------------------------------------------------------
// supervision modes — HITL / HOTL requirements and mutual exclusion
// ---------------------------------------------------------------------------
check('an unknown supervision mode is rejected', () => {
  assert(evalControl(mutateCtl((x) => { cpl(x).supervision_mode = 'autonomous'; lastSwitch(x).to_supervision_mode = 'autonomous'; })).length > 0,
    'an unknown supervision_mode was accepted');
});

check('HITL needs a concrete action and gate; HITL FORBIDS an hotl block', () => {
  const hitlValid = fixtures.control.valid.find((c) => c.spec.payload.supervision_mode === 'human-in-the-loop');
  assert(hitlValid, 'the valid set has no human-in-the-loop fixture');
  assert(evalControl(hitlValid.spec).length === 0, 'a well-formed HITL record was rejected');
  const noBlock = mutateCtl((x) => {
    cpl(x).supervision_mode = 'human-in-the-loop';
    delete cpl(x).hotl;
    lastSwitch(x).to_supervision_mode = 'human-in-the-loop';
  });
  assert(evalControl(noBlock).some((m) => /no concrete required human action and declared gate/.test(m) || /hitl.*required/.test(m)),
    'HITL with no hitl block was accepted');
  const noGate = mutateCtl((x) => {
    cpl(x).supervision_mode = 'human-in-the-loop';
    delete cpl(x).hotl;
    cpl(x).hitl = { required_human_action: 'Владелец подтверждает.' };
    lastSwitch(x).to_supervision_mode = 'human-in-the-loop';
  });
  assert(evalControl(noGate).some((m) => /no concrete required human action and declared gate/.test(m) || /gate.*required/.test(m)),
    'HITL with no gate was accepted');
  const withHotl = mutateCtl((x) => {
    cpl(x).supervision_mode = 'human-in-the-loop';
    cpl(x).hitl = { required_human_action: 'Владелец подтверждает.', gate: 'verification' };
    cpl(x).hotl = { autonomy_bounds: ['b'], intervention: 'i' };
    lastSwitch(x).to_supervision_mode = 'human-in-the-loop';
  });
  assert(evalControl(withHotl).some((m) => /human-in-the-loop forbids hotl/.test(m) || /must not match/.test(m)),
    'HITL with an hotl block present was accepted');
});

check('HOTL needs autonomy bounds and an intervention capability; HOTL FORBIDS an hitl block', () => {
  assert(evalControl(BASE_CONTROL).length === 0, 'the base HOTL fixture was rejected');
  const noBounds = mutateCtl((x) => { cpl(x).hotl = { autonomy_bounds: [], intervention: 'Владелец вмешивается.' }; });
  assert(evalControl(noBounds).some((m) => /no autonomy bounds or intervention capability/.test(m) || /fewer than 1 items/.test(m)),
    'HOTL with no autonomy bounds was accepted');
  const noIntervention = mutateCtl((x) => { cpl(x).hotl = { autonomy_bounds: ['b'] }; });
  assert(evalControl(noIntervention).some((m) => /no autonomy bounds or intervention capability/.test(m) || /intervention.*required/.test(m)),
    'HOTL with no intervention capability was accepted');
  const withHitl = mutateCtl((x) => { cpl(x).hitl = { required_human_action: 'Владелец подтверждает.', gate: 'verification' }; });
  assert(evalControl(withHitl).some((m) => /human-on-the-loop forbids hitl/.test(m) || /must not match/.test(m)),
    'HOTL with an hitl block present was accepted');
});

// ---------------------------------------------------------------------------
// roles, combination, base model, independence
// ---------------------------------------------------------------------------
check('role, supervision mode, human authority, acting actor and communication mode are independent axes on the payload', () => {
  const req = controlSchema.definitions.payload.required;
  for (const axis of ['human_authority', 'supervision_mode', 'acting_actor', 'role_assignments', 'communication_mode', 'switch_history']) {
    assert(req.includes(axis), `payload does not require the independent axis "${axis}"`);
  }
});

check('an unknown role, an empty assignment or an ambiguous assignment is rejected', () => {
  assert(evalControl(mutateCtl((x) => { cpl(x).role_assignments.push({ actor: 'actor:x', roles: ['auditor'] }); })).some((m) => /not in the closed pool/.test(m) || /not in enum/.test(m)),
    'an unknown role in an assignment was accepted');
  assert(evalControl(mutateCtl((x) => { cpl(x).role_assignments = []; })).some((m) => /no role assignments/.test(m) || /fewer than 1 items/.test(m)),
    'no role assignments was accepted');
  assert(evalControl(mutateCtl((x) => { cpl(x).role_assignments = [{ actor: '   ', roles: ['owner'] }]; })).some((m) => /names no actor/.test(m)),
    'a blank actor was accepted');
  assert(evalControl(mutateCtl((x) => { cpl(x).role_assignments = [{ actor: 'actor:x', roles: [] }]; })).some((m) => /lists no role/.test(m) || /fewer than 1 items/.test(m)),
    'an assignment with no roles was accepted');
});

check('a duplicate actor and a repeated role within an assignment are rejected', () => {
  assert(evalControl(mutateCtl((x) => { cpl(x).role_assignments.push({ actor: cpl(x).role_assignments[0].actor, roles: ['verifier'] }); })).some((m) => /assigned actor .* more than once|assigns actor .* more than once/.test(m)),
    'a duplicate actor was accepted');
  assert(evalControl(mutateCtl((x) => { cpl(x).role_assignments[0].roles = [cpl(x).role_assignments[0].roles[0], cpl(x).role_assignments[0].roles[0]]; })).some((m) => /assigns role .* more than once|not unique/.test(m)),
    'a repeated role within an assignment was accepted');
});

check('one actor may hold several roles', () => {
  const combined = fixtures.control.valid.find((c) => c.spec.payload.role_assignments.some((a) => a.roles.length > 1));
  assert(combined, 'the valid set has no fixture combining roles for one actor');
  assert(evalControl(combined.spec).length === 0, 'a valid combined-roles record was rejected');
  const d = mutateCtl((x) => {
    cpl(x).role_assignments[1].roles = ['executor', 'verifier', 'deployer'];
    lastSwitch(x).to_role_assignments = clone(cpl(x).role_assignments);
  });
  assert(evalControl(d).length === 0, `combining three roles for one actor was rejected: ${evalControl(d)[0]}`);
});

check('the base owner-plus-one-executor model is accepted with no reviewer', () => {
  const base = fixtures.control.valid.find((c) => !('review_independence' in c.spec.payload)
    && c.spec.payload.role_assignments.length === 2);
  assert(base, 'the valid set has no base two-role model fixture');
  assert(evalControl(base.spec).length === 0, 'the base owner-plus-one-executor model was rejected');
  assert(!base.spec.payload.role_assignments.some((a) => a.roles.includes('reviewer')), 'the base fixture assigns a reviewer');
});

check('review_independence required: true needs a separate suitable actor and never the reviewed actor', () => {
  const ind = fixtures.control.valid.find((c) => c.spec.payload.review_independence && c.spec.payload.review_independence.required);
  assert(ind, 'the valid set has no explicitly-independent-review fixture');
  assert(evalControl(ind.spec).length === 0, 'a valid independent-review record was rejected');
  assert(ind.spec.payload.review_independence.reviewer_actor !== ind.spec.payload.review_independence.reviewed_actor,
    'the independent-review fixture names the same actor twice');
  const same = mutateCtl((x) => {
    const a = cpl(x).role_assignments[1].actor;
    cpl(x).role_assignments[1].roles = ['executor', 'reviewer'];
    lastSwitch(x).to_role_assignments = clone(cpl(x).role_assignments);
    cpl(x).review_independence = { required: true, reviewer_actor: a, reviewed_actor: a };
  });
  assert(evalControl(same).some((m) => /cannot be the participant whose result is independently reviewed/.test(m)),
    'a self-review under a required independence was accepted');
  const noSep = mutateCtl((x) => {
    cpl(x).review_independence = { required: true, reviewer_actor: 'actor:example-reviewer-agent', reviewed_actor: cpl(x).role_assignments[1].actor };
  });
  assert(evalControl(noSep).some((m) => /no separate actor is assigned a reviewing role/.test(m)),
    'a required independence with no suitable separate actor was accepted');
  assert(INDEPENDENT_REVIEW_ROLES.length > 0, 'no reviewing role is recognised for independence');
});

check('review_independence required: false carries no reviewer/reviewed reference; present references are portable regardless of required', () => {
  const noReq = fixtures.control.valid.find((c) => c.spec.payload.review_independence && c.spec.payload.review_independence.required === false);
  assert(noReq, 'the valid set has no required:false review_independence fixture');
  assert(evalControl(noReq.spec).length === 0, 'a valid required:false review_independence record was rejected');
  assert(!('reviewer_actor' in noReq.spec.payload.review_independence) && !('reviewed_actor' in noReq.spec.payload.review_independence),
    'the required:false fixture still names references');
  const withRefs = mutateCtl((x) => { cpl(x).review_independence = { required: false, reviewer_actor: 'actor:example-reviewer-agent', reviewed_actor: cpl(x).role_assignments[1].actor }; });
  assert(evalControl(withRefs).some((m) => /required is false but still names a reviewer_actor or reviewed_actor/.test(m) || /must not match/.test(m)),
    'required:false with reviewer_actor/reviewed_actor was accepted');
  const nonPortableFalse = mutateCtl((x) => { cpl(x).review_independence = { required: false, reviewer_actor: '/opt/reviewers/x' }; });
  assert(evalControl(nonPortableFalse).some((m) => /review_independence.reviewer_actor contains/.test(m) || /must not match/.test(m)),
    'required:false with a non-portable reviewer_actor produced no portability finding');
  const nonPortableTrue = mutateCtl((x) => {
    cpl(x).role_assignments.push({ actor: 'actor:example-reviewer-agent', roles: ['reviewer'] });
    lastSwitch(x).to_role_assignments = clone(cpl(x).role_assignments);
    cpl(x).review_independence = { required: true, reviewer_actor: 'actor:example-reviewer-agent', reviewed_actor: '~/reviewed/x' };
  });
  assert(evalControl(nonPortableTrue).some((m) => /review_independence.reviewed_actor contains/.test(m)),
    'required:true with a non-portable reviewed_actor produced no portability finding');
});

// ---------------------------------------------------------------------------
// acting actor
// ---------------------------------------------------------------------------
check('acting_actor is portable and one of the currently assigned participants', () => {
  assert(evalControl(mutateCtl((x) => { cpl(x).acting_actor = 'actor:example-ghost'; })).some((m) => /acting_actor "actor:example-ghost" is not assigned/.test(m)),
    'an unassigned acting_actor was accepted');
  assert(evalControl(mutateCtl((x) => { cpl(x).acting_actor = '/opt/actors/x'; })).some((m) => /acting_actor contains/.test(m)),
    'a non-portable acting_actor was accepted');
});

// ---------------------------------------------------------------------------
// switch history — every mutable axis, continuity, and last-record agreement
// ---------------------------------------------------------------------------
check('an empty switch history is rejected', () => {
  assert(evalControl(mutateCtl((x) => { cpl(x).switch_history = []; })).some((m) => /switch_history is empty/.test(m) || /fewer than 1 items/.test(m)),
    'an empty switch history was accepted');
});

check('the first switch record states the initial establishment (every from_* axis null)', () => {
  fixtures.control.valid.forEach((c, i) => {
    const first = c.spec.payload.switch_history[0];
    assert(first.from_supervision_mode === null && first.from_communication_mode === null
      && first.from_acting_actor === null && first.from_role_assignments === null,
      `control.valid[${i}] (${c.note}) first switch does not state the initial establishment`);
  });
  for (const [axis, val] of [
    ['from_supervision_mode', 'human-in-the-loop'],
    ['from_communication_mode', 'owner_relayed'],
    ['from_acting_actor', 'actor:example-owner'],
    ['from_role_assignments', [{ actor: 'actor:example-owner', roles: ['owner'] }]],
  ]) {
    const d = mutateCtl((x) => { cpl(x).switch_history[0][axis] = val; });
    assert(evalControl(d).some((m) => /first record but names a prior/.test(m)), `a first switch with a prior ${axis} was accepted`);
  }
});

check('a break on ANY mutable axis of the switch chain is rejected', () => {
  // supervision-mode break
  const supBreak = mutateCtl((x) => {
    const h = cpl(x).switch_history;
    h.push({
      sequence: 2, from_supervision_mode: 'human-in-the-loop', from_communication_mode: h[0].to_communication_mode,
      from_acting_actor: h[0].to_acting_actor, from_role_assignments: clone(h[0].to_role_assignments),
      to_supervision_mode: 'human-on-the-loop', to_communication_mode: h[0].to_communication_mode,
      to_acting_actor: h[0].to_acting_actor, to_role_assignments: clone(h[0].to_role_assignments),
      checkpoint_ref: 'checkpoint:b', reason: 'разрыв',
    });
  });
  assert(evalControl(supBreak).some((m) => /break on the supervision-mode axis/.test(m)), 'a supervision-mode break was accepted');
  // communication-mode break
  const commBreak = mutateCtl((x) => {
    const h = cpl(x).switch_history;
    h.push({
      sequence: 2, from_supervision_mode: h[0].to_supervision_mode, from_communication_mode: 'owner_relayed',
      from_acting_actor: h[0].to_acting_actor, from_role_assignments: clone(h[0].to_role_assignments),
      to_supervision_mode: h[0].to_supervision_mode, to_communication_mode: 'owner_relayed',
      to_acting_actor: h[0].to_acting_actor, to_role_assignments: clone(h[0].to_role_assignments),
      checkpoint_ref: 'checkpoint:b', reason: 'разрыв',
    });
    cpl(x).communication_mode = 'owner_relayed';
  });
  assert(evalControl(commBreak).some((m) => /break on the communication-mode axis/.test(m)), 'a communication-mode break was accepted');
  // acting-actor break
  const actorBreak = mutateCtl((x) => {
    cpl(x).role_assignments.push({ actor: 'actor:example-executor-b', roles: ['executor'] });
    const h = cpl(x).switch_history;
    h[0].to_role_assignments = clone(cpl(x).role_assignments);
    h.push({
      sequence: 2, from_supervision_mode: h[0].to_supervision_mode, from_communication_mode: h[0].to_communication_mode,
      from_acting_actor: 'actor:example-executor-b', from_role_assignments: clone(cpl(x).role_assignments),
      to_supervision_mode: h[0].to_supervision_mode, to_communication_mode: h[0].to_communication_mode,
      to_acting_actor: h[0].to_acting_actor, to_role_assignments: clone(cpl(x).role_assignments),
      checkpoint_ref: 'checkpoint:b', reason: 'разрыв',
    });
  });
  assert(evalControl(actorBreak).some((m) => /break on the acting-actor axis/.test(m)), 'an acting-actor break was accepted');
  // role-assignment break
  const roleBreak = mutateCtl((x) => {
    const h = cpl(x).switch_history;
    h.push({
      sequence: 2, from_supervision_mode: h[0].to_supervision_mode, from_communication_mode: h[0].to_communication_mode,
      from_acting_actor: h[0].to_acting_actor,
      from_role_assignments: [{ actor: 'actor:example-owner', roles: ['owner'] }],
      to_supervision_mode: h[0].to_supervision_mode, to_communication_mode: h[0].to_communication_mode,
      to_acting_actor: h[0].to_acting_actor, to_role_assignments: [{ actor: 'actor:example-owner', roles: ['owner'] }],
      checkpoint_ref: 'checkpoint:b', reason: 'разрыв',
    });
    cpl(x).role_assignments = [{ actor: 'actor:example-owner', roles: ['owner'] }];
    cpl(x).acting_actor = 'actor:example-owner';
  });
  assert(evalControl(roleBreak).some((m) => /break on the role-assignment axis/.test(m)), 'a role-assignment break was accepted');
});

check('a decreasing switch sequence ordinal is rejected', () => {
  const dec = mutateCtl((x) => {
    const h = cpl(x).switch_history;
    const t0 = h[0];
    h.push({
      sequence: 1, from_supervision_mode: t0.to_supervision_mode, from_communication_mode: t0.to_communication_mode,
      from_acting_actor: t0.to_acting_actor, from_role_assignments: clone(t0.to_role_assignments),
      to_supervision_mode: t0.to_supervision_mode, to_communication_mode: t0.to_communication_mode,
      to_acting_actor: t0.to_acting_actor, to_role_assignments: clone(t0.to_role_assignments),
      checkpoint_ref: 'checkpoint:b', reason: 'ординал не растёт',
    });
  });
  assert(evalControl(dec).some((m) => /repeats sequence ordinal|is below the previous/.test(m)), 'a non-increasing switch ordinal was accepted');
});

check('every switch states a basis and preserves a checkpoint', () => {
  assert(evalControl(mutateCtl((x) => { delete cpl(x).switch_history[0].reason; })).some((m) => /states no reason|reason.*required/.test(m)),
    'a switch with no reason was accepted');
  assert(evalControl(mutateCtl((x) => { cpl(x).switch_history[0].checkpoint_ref = '  '; })).some((m) => /names no checkpoint_ref/.test(m)),
    'a switch with no checkpoint_ref was accepted');
});

check('a switch to_acting_actor absent from its own to_role_assignments is rejected', () => {
  const d = mutateCtl((x) => {
    lastSwitch(x).to_acting_actor = 'actor:example-ghost';
  });
  // acting_actor also mismatches, but the transition-level check must fire.
  assert(evalControl(d).some((m) => /is not present in that transition's role assignments/.test(m)),
    'a switch to an actor absent from its transition assignments was accepted');
});

check('the last switch record must match the WHOLE current control state', () => {
  assert(evalControl(mutateCtl((x) => {
    cpl(x).supervision_mode = 'human-in-the-loop';
    cpl(x).hitl = { required_human_action: 'Владелец подтверждает.', gate: 'verification' };
    delete cpl(x).hotl;
  })).some((m) => /does not match the current supervision_mode/.test(m)),
    'a last-switch supervision_mode mismatch was accepted');
  assert(evalControl(mutateCtl((x) => { cpl(x).communication_mode = 'owner_relayed'; })).some((m) => /does not match the current communication_mode/.test(m)),
    'a last-switch communication_mode mismatch was accepted');
  assert(evalControl(mutateCtl((x) => {
    cpl(x).role_assignments.push({ actor: 'actor:example-executor-b', roles: ['executor'] });
    cpl(x).acting_actor = 'actor:example-executor-b';
  })).some((m) => /does not match the current acting_actor/.test(m)),
    'a last-switch acting_actor mismatch was accepted');
  assert(evalControl(mutateCtl((x) => { cpl(x).role_assignments[1].roles = ['executor', 'verifier']; })).some((m) => /does not match the current role_assignments/.test(m)),
    'a last-switch role_assignments mismatch was accepted');
});

check('a separate change of role, of acting actor, and of communication mode each has a positive fixture', () => {
  const roleChange = fixtures.control.valid.find((c) => {
    const h = c.spec.payload.switch_history;
    return h.length >= 2 && JSON.stringify(h[h.length - 1].to_role_assignments) !== JSON.stringify(h[h.length - 2].to_role_assignments);
  });
  assert(roleChange && evalControl(roleChange.spec).length === 0, 'no accepted fixture shows a standalone role change');
  const actorChange = fixtures.control.valid.find((c) => {
    const h = c.spec.payload.switch_history;
    return h.length >= 2 && h[h.length - 1].to_acting_actor !== h[h.length - 2].to_acting_actor;
  });
  assert(actorChange && evalControl(actorChange.spec).length === 0, 'no accepted fixture shows a standalone acting-actor change');
  const commChange = fixtures.control.valid.find((c) => {
    const h = c.spec.payload.switch_history;
    return h.length >= 2 && h[h.length - 1].to_communication_mode !== h[h.length - 2].to_communication_mode;
  });
  assert(commChange && evalControl(commChange.spec).length === 0, 'no accepted fixture shows a standalone communication-mode change');
});

check('switching supervision mode keeps the same run and a checkpoint on every step and does not restart it', () => {
  const cyc = fixtures.control.valid.find((c) => c.spec.payload.switch_history.length >= 3);
  assert(cyc, 'the valid set has no multi-switch fixture');
  assert(evalControl(cyc.spec).length === 0, 'a valid multi-switch record was rejected');
  const modes = cyc.spec.payload.switch_history.map((s) => s.to_supervision_mode);
  assert(modes.includes('human-in-the-loop') && modes.includes('human-on-the-loop'), 'the multi-switch fixture does not exercise both modes');
  assert(cyc.spec.payload.switch_history.every((s) => typeof s.checkpoint_ref === 'string' && s.checkpoint_ref.trim().length > 0),
    'a switch in the cycle preserves no checkpoint');
  assert(typeof cyc.spec.payload.execution_run_ref === 'string', 'the run reference is not a single stable field');
});

// ---------------------------------------------------------------------------
// MANDATORY PERMANENT REGRESSIONS — previously accepted wrong cases
// ---------------------------------------------------------------------------
check('REGRESSION: HOTL together with an hitl block is rejected', () => {
  const fx = fixtures.control.invalid.find((c) => /human-on-the-loop carrying an hitl block/.test(c.note));
  assert(fx, 'the invalid set has no HOTL-with-hitl fixture');
  assert(evalControl(fx.spec).some((m) => /human-on-the-loop forbids hitl/.test(m) || /must not match/.test(m)),
    'HOTL with an hitl block was accepted');
});

check('REGRESSION: review_independence required: false with reviewer_actor/reviewed_actor is rejected', () => {
  const fx = fixtures.control.invalid.find((c) => /required: false still names reviewer_actor and reviewed_actor/.test(c.note));
  assert(fx, 'the invalid set has no required:false-with-refs fixture');
  assert(evalControl(fx.spec).some((m) => /required is false but still names/.test(m) || /must not match/.test(m)),
    'required:false with references was accepted');
});

check('REGRESSION: a transition to an unassigned participant is rejected', () => {
  const fx = fixtures.control.invalid.find((c) => /to_acting_actor absent from its own to_role_assignments/.test(c.note));
  assert(fx, 'the invalid set has no unassigned-transition-actor fixture');
  assert(evalControl(fx.spec).some((m) => /is not present in that transition's role assignments/.test(m)),
    'a transition to an unassigned participant was accepted');
  const fx2 = fixtures.control.invalid.find((c) => /acting_actor is not one of the assigned participants/.test(c.note));
  assert(fx2 && evalControl(fx2.spec).some((m) => /is not assigned in this run/.test(m)),
    'a payload acting_actor that is unassigned was accepted');
});

check('REGRESSION: a HIC state with no assigned owner is rejected', () => {
  const fx = fixtures.control.invalid.find((c) => /HIC state with no assigned owner/.test(c.note));
  assert(fx, 'the invalid set has no holder-not-owner fixture');
  assert(evalControl(fx.spec).some((m) => /is not a participant assigned the owner role/.test(m)),
    'a HIC holder that is not an assigned owner was accepted');
});

// ---------------------------------------------------------------------------
// portability — the reused rooted-path detection still fires
// ---------------------------------------------------------------------------
check('the reused nonPortableReason still catches rooted machine paths and passes relative ones', () => {
  assert(nonPortableReason('/data/private/build') !== null, 'a rooted path was treated as portable');
  assert(nonPortableReason('см. ~/build') !== null, 'a "~/" reference was treated as portable');
  assert(nonPortableReason('см. file:///opt/thing') !== null, 'a file:// URL was treated as portable');
  assert(nonPortableReason('src/config/parser') === null, 'a repo-relative path was flagged');
  assert(nonPortableReason('execution-run:example-run-001') === null, 'an opaque identifier was flagged');
});

check('a rooted machine path in a switch reason breaks portability; a relative path does not', () => {
  assert(evalControl(mutateCtl((x) => { cpl(x).switch_history[0].reason = 'Решение в /etc/example/policy.'; })).some((m) => /rooted \(absolute\) POSIX path/.test(m)),
    'a rooted path in a switch reason was accepted');
  const rel = fixtures.control.valid.find((c) => /src\/config\/parser|packages\/@scope/.test(JSON.stringify(c.spec.payload.switch_history)));
  assert(rel && evalControl(rel.spec).length === 0, 'a portable relative path in a switch reason was rejected');
});

check('neither schema carries a rooted machine path', () => {
  for (const rel of ['registries/operating-model/role-registry.schema.json', 'registries/operating-model/human-control.schema.json']) {
    const schemaText = loadText(rel);
    assert(!/\s\/[a-z]+\/[a-z]/i.test(schemaText.replace(/https?:\/\//g, 'x')), `${rel} appears to carry a rooted machine path`);
  }
});

// ---------------------------------------------------------------------------
if (failures.length) {
  console.log(`\n${failures.length} failing / ${passed} passing`);
  for (const f of failures) console.log(`  - ${f}`);
  process.exit(1);
}
console.log(`\nAll ${passed} checks passed.`);
