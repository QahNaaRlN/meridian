#!/usr/bin/env node
// Standalone verification for the instance-data migration contract
// (registries/operating-model/instance-data-migration.schema.json).
//
// It calls the same implementation the gate calls
// (scripts/lib/instance-data-migration.mjs) so the two cannot drift: the
// record envelope against the existing scoped-record.schema.json, the plan
// payload against the specialised instance-data-migration.schema.json, and
// the rules JSON Schema cannot state — unit-to-mapping coverage, target-group
// consistency (many-to-one merge, implicit multi-unit migrated, identical
// target descriptions, origin tracing to the real contributing units), a
// target's permanent authority against the closed owner-authority table for
// its own scope, a "reproducible" source resolved against an EXTERNAL
// snapshot boundary, a "verified" sub-verdict resolved against an EXTERNAL
// evidence boundary, the recomputed deterministic plan_fingerprint, the
// derived idempotency_key and scope/revision-checked supersedes.
//
// Usage: node test/instance-data-migration.test.mjs

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { assertSupportedDeep } from '../scripts/lib/json-schema.mjs';
import {
  evaluateInstanceDataMigration,
  computePlanFingerprint, computeIdempotencyKey, deriveOriginSourceRef,
  makeSourceSnapshotResolver, makeEvidenceResolver,
  makeRollbackSnapshotResolver, makeDeterministicPlanResolver, makeRestorationEvidenceResolver,
  makeSupersededPlanResolver,
  checkOpaqueRef,
  RECORD_TYPE, ALLOWED_SCOPE_TYPES, REQUIRED_ORIGIN_KIND, REQUIRED_AUTHORITY_KIND,
  DISPOSITIONS, QUALIFICATIONS, VERIFICATION_STATES, OVERALL_STATES, ROLLBACK_PLANS, FIELD_BASES,
  TARGET_ORIGIN_KIND, SOURCE_SNAPSHOT_RECORD_TYPE, EVIDENCE_RECORD_TYPE, EVIDENCE_KINDS,
  ROLLBACK_SNAPSHOT_RECORD_TYPE, DETERMINISTIC_PLAN_RECORD_TYPE, RESTORATION_EVIDENCE_RECORD_TYPE,
  SUPERSEDED_PLAN_RECORD_TYPE,
  TARGET_AUTHORITY_BY_SCOPE,
} from '../scripts/lib/instance-data-migration.mjs';

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

const loadJson = (rel) => JSON.parse(fs.readFileSync(path.join(root, rel), 'utf8'));

const registrySchema = loadJson('registries/operating-model/instance-data-migration.schema.json');
const envelopeSchema = loadJson('registries/operating-model/scoped-record.schema.json');
const fixtures = loadJson('registries/operating-model/fixtures/instance-data-migration.fixtures.json');

const opts = {
  registrySchema,
  envelopeSchema,
  resolveSourceSnapshot: makeSourceSnapshotResolver(fixtures.source_snapshot_resolution),
  resolveEvidence: makeEvidenceResolver(fixtures.evidence_resolution),
  resolveRollbackSnapshot: makeRollbackSnapshotResolver(fixtures.rollback_snapshot_resolution),
  resolveDeterministicPlan: makeDeterministicPlanResolver(fixtures.deterministic_plan_resolution),
  resolveRestorationEvidence: makeRestorationEvidenceResolver(fixtures.restoration_evidence_resolution),
  resolveSupersededPlan: makeSupersededPlanResolver(fixtures.superseded_plan_resolution),
};
const evaluate = (doc) => evaluateInstanceDataMigration(doc, opts);
const clone = (x) => JSON.parse(JSON.stringify(x));

const DIG_A = 'aaaa1111'.repeat(8);
const DIG_B = 'bbbb2222'.repeat(8);
const REPO_REF = 'sample-test-repository';
const REPO_REF_B = 'sample-test-repository-b';
const REVISION = 'test-revision-0001';
const SCOPE = { type: 'project-workspace', id: 'sample-test-project' };
const ROLLBACK_SNAPSHOT_REF = 'snapshot:sample-instance@test-revision-0001';
const DETERMINISTIC_PLAN_REF = 'rollback-plan:test-revision-0001';
const RESTORATION_EVIDENCE_REF = 'evidence:restoration-sample';
const PLAN_ONE_ID = 'plan-one';

// A local resolution universe, independent of the bundled fixtures', so BASE
// mutations never risk colliding with a fixtures-file entry.
const snapshotMap = {
  [`${REPO_REF}@${REVISION}`]: {
    record_type: SOURCE_SNAPSHOT_RECORD_TYPE,
    repository_ref: REPO_REF,
    revision: REVISION,
    digest: { algorithm: 'sha-256', value: DIG_A },
    working_tree_clean: true,
  },
  // A DIFFERENT repository that happens to reuse the identical revision TEXT
  // — used to prove idempotency_key does not collide across repositories.
  [`${REPO_REF_B}@${REVISION}`]: {
    record_type: SOURCE_SNAPSHOT_RECORD_TYPE,
    repository_ref: REPO_REF_B,
    revision: REVISION,
    digest: { algorithm: 'sha-256', value: DIG_B },
    working_tree_clean: true,
  },
};
// property 5 (reversibility) local resolution universe: rollback.source_snapshot_ref
// is checked against payload.source only (never plan_fingerprint — a rollback
// snapshot's identity is the source it was taken from, not the plan's own
// content), so this map needs no fingerprint and can be built before BASE_PAYLOAD.
const rollbackSnapshotMap = {
  [ROLLBACK_SNAPSHOT_REF]: {
    record_type: ROLLBACK_SNAPSHOT_RECORD_TYPE,
    source_snapshot_ref: ROLLBACK_SNAPSHOT_REF,
    repository_ref: REPO_REF,
    revision: REVISION,
    digest: { algorithm: 'sha-256', value: DIG_A },
  },
};

function envelope(id, payload, overrides = {}) {
  return {
    $schema: '../../registries/operating-model/scoped-record.schema.json',
    schema_version: 1,
    id,
    title: `План миграции — ${id}`,
    record_type: RECORD_TYPE,
    scope: SCOPE,
    origin: { kind: 'declared', source_ref: `owner-decision:${id}` },
    authority: { kind: 'delegated-run', authority_ref: `instance-data-migration-run:${id}` },
    payload,
    ...overrides,
  };
}

function finalize(payload) {
  const idempotency_key = computeIdempotencyKey(SCOPE, payload.source);
  const withKey = { ...payload, idempotency_key };
  const plan_fingerprint = computePlanFingerprint(withKey);
  return { ...withKey, plan_fingerprint };
}

const BASE_PAYLOAD = finalize({
  source: {
    revision: REVISION,
    digest: { algorithm: 'sha-256', value: DIG_A },
    repository_ref: REPO_REF,
    working_tree_clean: true,
    qualification: 'reproducible',
  },
  record_units: [
    { id: 'unit-one', unit_ref: 'sample/path/one.yaml', classification_basis: 'единица объявлена решением владельца проекта' },
  ],
  mappings: [
    {
      unit_id: 'unit-one',
      disposition: 'migrated',
      target: {
        id: 'target-one',
        record_type: 'norm',
        scope: { type: 'project-workspace', id: 'sample-test-project' },
        field_basis: { id: 'assigned', record_type: 'preserved', scope: 'preserved', origin: 'assigned', authority: 'assigned' },
        origin: { kind: 'migrated', source_ref: deriveOriginSourceRef(['unit-one']) },
        authority: { kind: 'project-owner', authority_ref: 'sample-project-owner', decision_ref: 'owner-decision:migrate-unit-one' },
      },
      owner_decision: { decision_ref: 'owner-decision:migrate-unit-one', decided_at: '2026-09-10', reason: 'владелец подтвердил перенос' },
    },
  ],
  rollback: {
    source_snapshot_ref: ROLLBACK_SNAPSHOT_REF,
    plan: 'deterministic-reconstruction',
    deterministic_plan_ref: DETERMINISTIC_PLAN_REF,
    rewrites_published_history: false,
  },
  verification: {
    coverage: { status: 'verified', evidence_ref: 'evidence:coverage-base' },
    applicability_preservation: { status: 'verified', evidence_ref: 'evidence:applicability-base' },
    overall_status: 'VERIFIED',
  },
});

// Every proof below is pinned to BASE_PAYLOAD's OWN recomputed fingerprint,
// not only to PLAN_ONE_ID (property 6/5: evidence/rollback proofs are bound
// to a plan's EXACT content). Built only now, after BASE_PAYLOAD exists.
const evidenceMap = {
  'evidence:coverage-base': { record_type: EVIDENCE_RECORD_TYPE, evidence_ref: 'evidence:coverage-base', kind: 'coverage', plan_ref: PLAN_ONE_ID, plan_fingerprint: BASE_PAYLOAD.plan_fingerprint, confirms: true },
  'evidence:applicability-base': { record_type: EVIDENCE_RECORD_TYPE, evidence_ref: 'evidence:applicability-base', kind: 'applicability_preservation', plan_ref: PLAN_ONE_ID, plan_fingerprint: BASE_PAYLOAD.plan_fingerprint, confirms: true },
  'evidence:non-confirming': { record_type: EVIDENCE_RECORD_TYPE, evidence_ref: 'evidence:non-confirming', kind: 'coverage', plan_ref: PLAN_ONE_ID, plan_fingerprint: BASE_PAYLOAD.plan_fingerprint, confirms: false },
};
const deterministicPlanMap = {
  [DETERMINISTIC_PLAN_REF]: {
    record_type: DETERMINISTIC_PLAN_RECORD_TYPE,
    deterministic_plan_ref: DETERMINISTIC_PLAN_REF,
    source_snapshot_ref: ROLLBACK_SNAPSHOT_REF,
    plan_ref: PLAN_ONE_ID,
    plan_fingerprint: BASE_PAYLOAD.plan_fingerprint,
    applicable: true,
  },
};
const restorationEvidenceMap = {
  [RESTORATION_EVIDENCE_REF]: {
    record_type: RESTORATION_EVIDENCE_RECORD_TYPE,
    evidence_ref: RESTORATION_EVIDENCE_REF,
    plan_ref: PLAN_ONE_ID,
    plan_fingerprint: BASE_PAYLOAD.plan_fingerprint,
    source_snapshot_ref: ROLLBACK_SNAPSHOT_REF,
    confirms: true,
  },
};
const localOpts = {
  registrySchema, envelopeSchema,
  resolveSourceSnapshot: makeSourceSnapshotResolver(snapshotMap),
  resolveEvidence: makeEvidenceResolver(evidenceMap),
  resolveRollbackSnapshot: makeRollbackSnapshotResolver(rollbackSnapshotMap),
  resolveDeterministicPlan: makeDeterministicPlanResolver(deterministicPlanMap),
  resolveRestorationEvidence: makeRestorationEvidenceResolver(restorationEvidenceMap),
};
const evaluateLocal = (doc) => evaluateInstanceDataMigration(doc, localOpts);

// A shallow-copy helper for re-minting one resolved record with an updated
// plan_fingerprint — used by tests that materially change BASE_PAYLOAD's
// mapping/target/rollback content (and therefore its fingerprint) and must
// re-mint the proof for the NEW content to stay legal, exactly as a real
// integration would need fresh evidence after a material plan change.
const withFingerprint = (entry, fp) => ({ ...entry, plan_fingerprint: fp });

function baseDoc() {
  return {
    $schema: '../../registries/operating-model/instance-data-migration.schema.json',
    schema_version: 1,
    registry_id: 'instance-data-migration',
    title: 'Тестовый реестр миграции данных Экземпляра',
    migration_plans: [envelope(PLAN_ONE_ID, clone(BASE_PAYLOAD))],
  };
}
const plan0 = (d) => d.migration_plans[0];
const payload0 = (d) => plan0(d).payload;
const mutate = (fn) => { const d = baseDoc(); fn(d); return d; };
const refingerprint = (d) => { payload0(d).plan_fingerprint = computePlanFingerprint(payload0(d)); return d; };
const mutateAndRefingerprint = (fn) => refingerprint(mutate(fn));

// ---------------------------------------------------------------------------
// schema shape and real bundled fixtures
// ---------------------------------------------------------------------------
check('обе схемы используют поддерживаемое подмножество JSON Schema', () => {
  assertSupportedDeep(registrySchema, 'instance-data-migration.schema.json');
  assertSupportedDeep(envelopeSchema, 'scoped-record.schema.json');
});

check('специализированная схема — не второй конверт записи', () => {
  assert(registrySchema.properties.registry_id.const === 'instance-data-migration', 'registry_id не закреплён');
  assert(registrySchema.definitions.entry.properties.record_type.const === RECORD_TYPE, 'record_type записи не закреплён');
});

check('неподдерживаемое ключевое слово схемы отклоняется движком', () => {
  const broken = clone(registrySchema);
  broken.definitions.payload.patternProperties = { '^x': { type: 'string' } };
  let threw = false;
  try { assertSupportedDeep(broken, 'broken'); } catch { threw = true; }
  assert(threw, 'assertSupportedDeep пропустил неподдерживаемое ключевое слово');
});

check('фикстуры этого контракта несут непустые valid и invalid', () => {
  assert(Array.isArray(fixtures.valid) && fixtures.valid.length > 0, 'valid пуст');
  assert(Array.isArray(fixtures.invalid) && fixtures.invalid.length > 0, 'invalid пуст');
});

check('каждая valid-фикстура проходит evaluateInstanceDataMigration без проблем', () => {
  for (const c of fixtures.valid) {
    const problems = evaluate(c.registry);
    assert(problems.length === 0, `фикстура "${c.note}" отклонена: ${problems[0]}`);
  }
});

check('каждая invalid-фикстура отклоняется evaluateInstanceDataMigration', () => {
  for (const c of fixtures.invalid) {
    const problems = evaluate(c.registry);
    assert(problems.length > 0, `фикстура "${c.note}" прошла проверку чисто`);
  }
});

// ---------------------------------------------------------------------------
// BASE topology sanity
// ---------------------------------------------------------------------------
check('BASE-документ этого набора сам по себе валиден', () => {
  assert(evaluateLocal(baseDoc()).length === 0, JSON.stringify(evaluateLocal(baseDoc())));
});

check('пустой реестр (без планов) легален', () => {
  assert(evaluate({ schema_version: 1, registry_id: 'instance-data-migration', title: 'Пустой', migration_plans: [] }).length === 0);
});

// ---------------------------------------------------------------------------
// property 1 — storage neutrality
// ---------------------------------------------------------------------------
check('checkOpaqueRef отклоняет корневой POSIX-путь', () => {
  assert(checkOpaqueRef('/abs/path', 'label') !== null);
});
check('checkOpaqueRef отклоняет путь Windows', () => {
  assert(checkOpaqueRef('C:\\Users\\x', 'label') !== null);
});
check('checkOpaqueRef отклоняет file:// URL', () => {
  assert(checkOpaqueRef('file:///home/x', 'label') !== null);
});
check('checkOpaqueRef отклоняет ".."-побег', () => {
  assert(checkOpaqueRef('a/../b', 'label') !== null);
});
check('checkOpaqueRef принимает непрозрачный идентификатор', () => {
  assert(checkOpaqueRef('sample-instance-repository', 'label') === null);
});
check('[negative] абсолютный путь в source.repository_ref отклоняется', () => {
  const d = mutateAndRefingerprint((d2) => { payload0(d2).source.repository_ref = '/abs/machine/path'; });
  assert(evaluateLocal(d).length > 0);
});
check('[negative] file:// URL в rollback.source_snapshot_ref отклоняется', () => {
  const d = mutate((d2) => { payload0(d2).rollback.source_snapshot_ref = 'file:///home/x/snap'; });
  assert(evaluateLocal(d).length > 0);
});
// Kernel-purity (absence of real product/machine-local literals in the tree)
// is a generic, product-neutral property of the whole Kernel, already proven
// by scripts/kernel-validate.mjs's own "kernel-purity" section against the
// real Instance's product record and regression-tested generically (a
// synthetic product record, a synthetic literal) in test/kernel-validate.test.mjs.
// This specialised suite does not restate that check with a hardcoded literal
// of its own: doing so would itself be exactly the kind of real, machine- or
// product-specific literal the check exists to keep out of the Kernel.

// ---------------------------------------------------------------------------
// property 2 — precise source, resolved against an external snapshot boundary
// ---------------------------------------------------------------------------
check('qualification "reproducible" требует working_tree_clean true (схема)', () => {
  const d = mutate((d2) => { payload0(d2).source.working_tree_clean = false; });
  assert(evaluateLocal(d).length > 0);
});
check('qualification "not-reproducible" без qualification_reason отклоняется (схема)', () => {
  const d = mutate((d2) => {
    payload0(d2).source.working_tree_clean = false;
    payload0(d2).source.qualification = 'not-reproducible';
    payload0(d2).verification.overall_status = 'BLOCKED';
  });
  assert(evaluateLocal(d).length > 0);
});
check('источник, не признанный воспроизводимым, форсирует BLOCKED', () => {
  const d = mutate((d2) => {
    payload0(d2).source.working_tree_clean = false;
    payload0(d2).source.qualification = 'not-reproducible';
    payload0(d2).source.qualification_reason = 'рабочее дерево источника грязное';
  });
  const problems = evaluateLocal(d);
  assert(problems.length > 0 && problems.some((p) => p.includes('BLOCKED')), 'не отклонено требованием BLOCKED');
});
check('источник, признанный воспроизводимым, но overall_status UNVERIFIED, — легален', () => {
  const d = mutate((d2) => {
    payload0(d2).verification.coverage = { status: 'unverified' };
    payload0(d2).verification.overall_status = 'UNVERIFIED';
  });
  assert(evaluateLocal(d).length === 0);
});
check('[negative] reproducible-источник без разрешённого снимка отклоняется', () => {
  const d = mutateAndRefingerprint((d2) => { payload0(d2).source.revision = 'test-revision-unregistered'; });
  const problems = evaluateLocal(d);
  assert(problems.length > 0 && problems.some((p) => p.includes('resolved backing')));
});
check('[negative] заявленный digest, не совпадающий с разрешённым снимком, отклоняется', () => {
  const d = mutate((d2) => { payload0(d2).source.digest.value = 'bbbb2222'.repeat(8); });
  const problems = evaluateLocal(d);
  assert(problems.length > 0 && problems.some((p) => p.includes('digest')));
});
check('[negative] снимок, разрешённый для другого repository_ref, не подтверждает воспроизводимость', () => {
  const otherRepoSnapshotMap = {
    ...snapshotMap,
    [`${REPO_REF}@${REVISION}`]: { record_type: SOURCE_SNAPSHOT_RECORD_TYPE, repository_ref: 'some-other-repository', revision: REVISION, digest: { algorithm: 'sha-256', value: DIG_A }, working_tree_clean: true },
  };
  const opts2 = { ...localOpts, resolveSourceSnapshot: makeSourceSnapshotResolver(otherRepoSnapshotMap) };
  const problems = evaluateInstanceDataMigration(baseDoc(), opts2);
  assert(problems.length > 0 && problems.some((p) => p.includes('repository_ref') && p.includes('does not match')), JSON.stringify(problems));
});
check('[negative] разрешённый снимок с лишним полем внутри digest отклоняется', () => {
  const extraFieldSnapshotMap = {
    ...snapshotMap,
    [`${REPO_REF}@${REVISION}`]: { record_type: SOURCE_SNAPSHOT_RECORD_TYPE, repository_ref: REPO_REF, revision: REVISION, digest: { algorithm: 'sha-256', value: DIG_A, note: 'unexpected' }, working_tree_clean: true },
  };
  const opts2 = { ...localOpts, resolveSourceSnapshot: makeSourceSnapshotResolver(extraFieldSnapshotMap) };
  const problems = evaluateInstanceDataMigration(baseDoc(), opts2);
  assert(problems.length > 0 && problems.some((p) => p.includes('unknown field') && p.includes('digest')), JSON.stringify(problems));
});
check('[negative] разрешённый снимок с отсутствующим полем внутри digest отклоняется', () => {
  const missingFieldSnapshotMap = {
    ...snapshotMap,
    [`${REPO_REF}@${REVISION}`]: { record_type: SOURCE_SNAPSHOT_RECORD_TYPE, repository_ref: REPO_REF, revision: REVISION, digest: { value: DIG_A }, working_tree_clean: true },
  };
  const opts2 = { ...localOpts, resolveSourceSnapshot: makeSourceSnapshotResolver(missingFieldSnapshotMap) };
  const problems = evaluateInstanceDataMigration(baseDoc(), opts2);
  assert(problems.length > 0 && problems.some((p) => p.includes('digest')), JSON.stringify(problems));
});
check('[negative] resolveSourceSnapshot отсутствует — reproducible-план отклоняется закрыто', () => {
  const d = baseDoc();
  const problems = evaluateInstanceDataMigration(d, { registrySchema, envelopeSchema });
  assert(problems.some((p) => p.includes('resolved backing')), 'отсутствующий resolver должен закрыто отклонить claim');
});

// ---------------------------------------------------------------------------
// property 3 — complete correspondence
// ---------------------------------------------------------------------------
check('[negative] единица без mapping отклоняется', () => {
  const d = mutateAndRefingerprint((d2) => {
    payload0(d2).record_units.push({ id: 'unit-orphan', unit_ref: 'sample/orphan.yaml', classification_basis: 'забытая единица' });
  });
  assert(evaluateLocal(d).length > 0);
});
check('дублирующий mapping на ту же единицу отклоняется', () => {
  const d = mutate((d2) => { payload0(d2).mappings.push(clone(payload0(d2).mappings[0])); });
  assert(evaluateLocal(d).length > 0);
});
check('mapping на необъявленную единицу отклоняется', () => {
  const d = mutate((d2) => { payload0(d2).mappings[0].unit_id = 'unit-does-not-exist'; });
  assert(evaluateLocal(d).length > 0);
});
check('classification_basis, равный unit_ref дословно, отклоняется', () => {
  const d = mutate((d2) => { payload0(d2).record_units[0].classification_basis = payload0(d2).record_units[0].unit_ref; });
  assert(evaluateLocal(d).length > 0);
});
check('[negative] пустой VERIFIED-план (без record_units и mappings) отклоняется схемой', () => {
  const d = mutate((d2) => { payload0(d2).record_units = []; payload0(d2).mappings = []; });
  assert(evaluateLocal(d).length > 0);
});
check('пустой контейнер реестра (без планов) остаётся легальным', () => {
  assert(evaluate({ schema_version: 1, registry_id: 'instance-data-migration', title: 'x', migration_plans: [] }).length === 0);
});

// ---------------------------------------------------------------------------
// property 3/4 — target groups: many-to-one merge, implicit multi-unit
// migrated, identical target description, origin tracing
// ---------------------------------------------------------------------------
const mergeFixture = fixtures.valid.find((c) => c.note.startsWith('many-to-one merge'));
assert(mergeFixture, 'фикстура many-to-one merge не найдена — набор фикстур изменился');

check('merge с ровно одной единицей (мнимое слияние) отклоняется', () => {
  const d = clone(mergeFixture.registry);
  const p = d.migration_plans[0].payload;
  p.mappings.pop();
  p.record_units.pop();
  p.plan_fingerprint = computePlanFingerprint(p);
  assert(evaluate(d).length > 0);
});
check('merge с расходящимся merge_rule_ref отклоняется', () => {
  const d = clone(mergeFixture.registry);
  d.migration_plans[0].payload.mappings[1].merge_rule_ref = 'merge-rule:other';
  assert(evaluate(d).length > 0);
});
check('merge с расходящимся описанием цели (record_type) отклоняется', () => {
  const d = clone(mergeFixture.registry);
  d.migration_plans[0].payload.mappings[1].target.record_type = 'something-else';
  assert(evaluate(d).length > 0);
});
check('merge с тремя единицами, одним правилом и общей целью легален (не только ровно две)', () => {
  const d = clone(mergeFixture.registry);
  const p = d.migration_plans[0].payload;
  const thirdUnit = clone(p.record_units[0]);
  thirdUnit.id = 'sample-unit-c';
  thirdUnit.unit_ref = 'sample/path/c.yaml';
  thirdUnit.classification_basis = 'третья единица того же объединения, то же явное правило';
  p.record_units.push(thirdUnit);
  const thirdMapping = clone(p.mappings[0]);
  thirdMapping.unit_id = 'sample-unit-c';
  p.mappings.push(thirdMapping);
  const expectedOrigin = deriveOriginSourceRef(['sample-unit-a', 'sample-unit-b', 'sample-unit-c']);
  for (const m of p.mappings) m.target.origin.source_ref = expectedOrigin;
  p.plan_fingerprint = computePlanFingerprint(p);
  // The fingerprint changed (a third unit/mapping was added) — the plan's
  // evidence and deterministic-reconstruction proofs must be re-minted for
  // the NEW content, exactly like a real integration would need fresh
  // evidence after a material plan change (property 6/5).
  const covRef = 'evidence:coverage-migration-plan-three';
  const appRef = 'evidence:applicability-migration-plan-three';
  const detRef = 'rollback-plan:sample-revision-0003-to-migration-plan-three';
  const opts2 = {
    ...opts,
    resolveEvidence: makeEvidenceResolver({
      ...fixtures.evidence_resolution,
      [covRef]: withFingerprint(fixtures.evidence_resolution[covRef], p.plan_fingerprint),
      [appRef]: withFingerprint(fixtures.evidence_resolution[appRef], p.plan_fingerprint),
    }),
    resolveDeterministicPlan: makeDeterministicPlanResolver({
      ...fixtures.deterministic_plan_resolution,
      [detRef]: withFingerprint(fixtures.deterministic_plan_resolution[detRef], p.plan_fingerprint),
    }),
  };
  assert(evaluateInstanceDataMigration(d, opts2).length === 0, JSON.stringify(evaluateInstanceDataMigration(d, opts2)));
});
check('[negative] две "migrated" mapping, неявно делящие одну цель без merged, отклоняются', () => {
  const d = mutateAndRefingerprint((d2) => {
    const p = payload0(d2);
    const secondUnit = clone(p.record_units[0]);
    secondUnit.id = 'unit-two';
    secondUnit.unit_ref = 'sample/path/two.yaml';
    p.record_units.push(secondUnit);
    const secondMapping = clone(p.mappings[0]);
    secondMapping.unit_id = 'unit-two';
    p.mappings.push(secondMapping);
  });
  d.migration_plans[0].payload.idempotency_key = computeIdempotencyKey(SCOPE, payload0(d).source);
  refingerprint(d);
  const problems = evaluateLocal(d);
  assert(problems.length > 0 && problems.some((p) => p.includes('implicit')));
});
check('target.origin.source_ref не прослеживается к реальной единице — отклоняется', () => {
  const d = mutate((d2) => { payload0(d2).mappings[0].target.origin.source_ref = 'record-unit:some-other-unit'; });
  const problems = evaluateLocal(d);
  assert(problems.length > 0 && problems.some((p) => p.includes('does not trace')));
});

// ---------------------------------------------------------------------------
// property 4 — semantics preservation, target origin, field_basis
// ---------------------------------------------------------------------------
check('[negative] target без origin отклоняется схемой', () => {
  const d = mutate((d2) => { delete payload0(d2).mappings[0].target.origin; });
  assert(evaluateLocal(d).length > 0);
});
check('target.origin.kind, отличный от migrated, отклоняется (схема)', () => {
  const d = mutate((d2) => { payload0(d2).mappings[0].target.origin.kind = 'declared'; });
  assert(evaluateLocal(d).length > 0);
});
check('field_basis.origin "preserved" отклоняется (всегда assigned)', () => {
  const d = mutate((d2) => { payload0(d2).mappings[0].target.field_basis.origin = 'preserved'; });
  const problems = evaluateLocal(d);
  assert(problems.length > 0 && problems.some((p) => p.includes('field_basis.origin')));
});
check('delegated-run как постоянное полномочие цели отклоняется', () => {
  const d = mutate((d2) => { payload0(d2).mappings[0].target.authority.kind = 'delegated-run'; });
  assert(evaluateLocal(d).length > 0);
});
check('несовпадающее с областью полномочие цели отклоняется', () => {
  const d = mutate((d2) => { payload0(d2).mappings[0].target.authority.kind = 'repository-maintainer'; });
  assert(evaluateLocal(d).length > 0);
});
check('цель, классифицированная в run-state, отклоняется (структурно легальна, но без полномочия)', () => {
  const d = mutate((d2) => { payload0(d2).mappings[0].target.scope = { type: 'run-state', id: 'sample-test-project', workspace_id: 'sample-test-project' }; });
  assert(evaluateLocal(d).length > 0);
});
check('[negative] repository-scope цель без workspace_id отклоняется схемой', () => {
  const d = mutate((d2) => {
    payload0(d2).mappings[0].target.scope = { type: 'repository-scope', id: 'sample-repository' };
    payload0(d2).mappings[0].target.authority.kind = 'repository-maintainer';
  });
  assert(evaluateLocal(d).length > 0);
});
check('repository-scope цель С workspace_id структурно легальна', () => {
  const d = mutate((d2) => {
    payload0(d2).mappings[0].target.scope = { type: 'repository-scope', id: 'sample-repository', workspace_id: 'sample-workspace' };
    payload0(d2).mappings[0].target.authority.kind = 'repository-maintainer';
  });
  refingerprint(d);
  // mappings changed -> fingerprint changed -> the evidence/deterministic-plan
  // proofs pinned to BASE_PAYLOAD's OLD fingerprint must be re-minted for the
  // NEW one (property 6/5), exactly like a real integration would.
  const fp = payload0(d).plan_fingerprint;
  const opts2 = {
    ...localOpts,
    resolveEvidence: makeEvidenceResolver({
      ...evidenceMap,
      'evidence:coverage-base': withFingerprint(evidenceMap['evidence:coverage-base'], fp),
      'evidence:applicability-base': withFingerprint(evidenceMap['evidence:applicability-base'], fp),
    }),
    resolveDeterministicPlan: makeDeterministicPlanResolver({
      ...deterministicPlanMap,
      [DETERMINISTIC_PLAN_REF]: withFingerprint(deterministicPlanMap[DETERMINISTIC_PLAN_REF], fp),
    }),
  };
  const problems = evaluateInstanceDataMigration(d, opts2);
  assert(problems.length === 0, JSON.stringify(problems));
});
check('расхождение authority.decision_ref и owner_decision.decision_ref отклоняется', () => {
  const d = mutate((d2) => { payload0(d2).mappings[0].target.authority.decision_ref = 'owner-decision:other'; });
  assert(evaluateLocal(d).length > 0);
});
check('field_basis без одного из пяти полей отклоняется (схема)', () => {
  const d = mutate((d2) => { delete payload0(d2).mappings[0].target.field_basis.record_type; });
  assert(evaluateLocal(d).length > 0);
});
check('migrated mapping без owner_decision отклоняется (схема)', () => {
  const d = mutate((d2) => { delete payload0(d2).mappings[0].owner_decision; });
  assert(evaluateLocal(d).length > 0);
});
check('retained-transitional с target отклоняется (схема)', () => {
  const d = mutate((d2) => {
    payload0(d2).mappings[0].disposition = 'retained-transitional';
    payload0(d2).mappings[0].retained_reason = 'причина';
  });
  assert(evaluateLocal(d).length > 0);
});

for (const [scopeType, authorityKind] of Object.entries(TARGET_AUTHORITY_BY_SCOPE)) {
  check(`таблица полномочий: scope "${scopeType}" требует authority.kind "${authorityKind}"`, () => {
    assert(TARGET_AUTHORITY_BY_SCOPE[scopeType] === authorityKind);
  });
}

// ---------------------------------------------------------------------------
// property 5 — reversibility
// ---------------------------------------------------------------------------
check('rewrites_published_history: true отклоняется (схема const false)', () => {
  const d = mutate((d2) => { payload0(d2).rollback.rewrites_published_history = true; });
  assert(evaluateLocal(d).length > 0);
});
check('rollback без source_snapshot_ref отклоняется (схема)', () => {
  const d = mutate((d2) => { delete payload0(d2).rollback.source_snapshot_ref; });
  assert(evaluateLocal(d).length > 0);
});
check('rollback.plan "deterministic-reconstruction" без deterministic_plan_ref отклоняется (схема)', () => {
  const d = mutate((d2) => { delete payload0(d2).rollback.deterministic_plan_ref; });
  assert(evaluateLocal(d).length > 0);
});
// Rollback variants are strictly MUTUALLY EXCLUSIVE (schema-level): an inactive
// ref for the OTHER variant is never silently tolerated as an ignored extra
// field — it is a schema violation, even when it does not resolve at all.
check('[negative] deterministic-reconstruction с посторонним restoration_evidence_ref отклоняется (схема)', () => {
  const d = mutate((d2) => { payload0(d2).rollback.restoration_evidence_ref = 'evidence:invented-and-never-checked'; });
  assert(evaluateLocal(d).length > 0);
});
check('[negative] verified-restoration с посторонним deterministic_plan_ref отклоняется (схема)', () => {
  const d = mutate((d2) => {
    payload0(d2).rollback = {
      source_snapshot_ref: ROLLBACK_SNAPSHOT_REF,
      plan: 'verified-restoration',
      restoration_evidence_ref: RESTORATION_EVIDENCE_REF,
      deterministic_plan_ref: 'rollback-plan:invented-and-never-checked',
      rewrites_published_history: false,
    };
  });
  assert(evaluateLocal(d).length > 0);
});
check('rollback.plan "verified-restoration" с restoration_evidence_ref легален', () => {
  const d = mutate((d2) => {
    payload0(d2).rollback = {
      source_snapshot_ref: ROLLBACK_SNAPSHOT_REF,
      plan: 'verified-restoration',
      restoration_evidence_ref: RESTORATION_EVIDENCE_REF,
      rewrites_published_history: false,
    };
  });
  refingerprint(d);
  // rollback changed -> fingerprint changed -> every proof pinned to this
  // plan's content (both evidence sub-verdicts AND the restoration evidence
  // itself) must be re-minted for the NEW fingerprint (property 6/5).
  const fp = payload0(d).plan_fingerprint;
  const opts2 = {
    ...localOpts,
    resolveEvidence: makeEvidenceResolver({
      ...evidenceMap,
      'evidence:coverage-base': withFingerprint(evidenceMap['evidence:coverage-base'], fp),
      'evidence:applicability-base': withFingerprint(evidenceMap['evidence:applicability-base'], fp),
    }),
    resolveRestorationEvidence: makeRestorationEvidenceResolver({
      ...restorationEvidenceMap,
      [RESTORATION_EVIDENCE_REF]: withFingerprint(restorationEvidenceMap[RESTORATION_EVIDENCE_REF], fp),
    }),
  };
  const problems = evaluateInstanceDataMigration(d, opts2);
  assert(problems.length === 0, JSON.stringify(problems));
});

// --- reversibility is actually CHECKED, not merely schema-shaped: every
// rollback ref is resolved through its own external boundary. ---
check('[negative] rollback.source_snapshot_ref, не разрешённый ни одним резолвером, отклоняется', () => {
  const d = mutate((d2) => { payload0(d2).rollback.source_snapshot_ref = 'snapshot:invented'; });
  const problems = evaluateLocal(d);
  assert(problems.length > 0 && problems.some((p) => p.includes('rollback.source_snapshot_ref') && p.includes('does not resolve')), JSON.stringify(problems));
});
check('[negative] rollback.source_snapshot_ref, разрешённый для чужого источника (другой repository_ref/revision), отклоняется', () => {
  const otherRollbackMap = { [ROLLBACK_SNAPSHOT_REF]: { record_type: ROLLBACK_SNAPSHOT_RECORD_TYPE, source_snapshot_ref: ROLLBACK_SNAPSHOT_REF, repository_ref: 'some-other-repository', revision: 'some-other-revision', digest: { algorithm: 'sha-256', value: DIG_B } } };
  const opts2 = { ...localOpts, resolveRollbackSnapshot: makeRollbackSnapshotResolver(otherRollbackMap) };
  const problems = evaluateInstanceDataMigration(baseDoc(), opts2);
  assert(problems.length > 0 && problems.some((p) => p.includes('does not match this plan')), JSON.stringify(problems));
});
check('[negative] rollback.source_snapshot_ref, разрешённый с расходящимся digest, отклоняется', () => {
  const mismatchedDigestMap = { [ROLLBACK_SNAPSHOT_REF]: { record_type: ROLLBACK_SNAPSHOT_RECORD_TYPE, source_snapshot_ref: ROLLBACK_SNAPSHOT_REF, repository_ref: REPO_REF, revision: REVISION, digest: { algorithm: 'sha-256', value: DIG_B } } };
  const opts2 = { ...localOpts, resolveRollbackSnapshot: makeRollbackSnapshotResolver(mismatchedDigestMap) };
  const problems = evaluateInstanceDataMigration(baseDoc(), opts2);
  assert(problems.length > 0 && problems.some((p) => p.includes('digest that does not match')), JSON.stringify(problems));
});
check('[negative] resolveRollbackSnapshot отсутствует — план отклоняется закрыто', () => {
  const opts2 = { registrySchema, envelopeSchema, resolveSourceSnapshot: makeSourceSnapshotResolver(snapshotMap), resolveEvidence: makeEvidenceResolver(evidenceMap) };
  const problems = evaluateInstanceDataMigration(baseDoc(), opts2);
  assert(problems.some((p) => p.includes('rollback.source_snapshot_ref') && p.includes('does not resolve')), 'отсутствующий resolveRollbackSnapshot должен закрыто отклонить claim');
});
check('[negative] вымышленный deterministic_plan_ref не подтверждает обратимость', () => {
  const d = mutate((d2) => { payload0(d2).rollback.deterministic_plan_ref = 'rollback-plan:invented'; });
  const problems = evaluateLocal(d);
  assert(problems.length > 0 && problems.some((p) => p.includes('rollback.deterministic_plan_ref') && p.includes('does not resolve')), JSON.stringify(problems));
});
check('[negative] deterministic_plan_ref, называющий чужой rollback.source_snapshot_ref (cross-source), отклоняется', () => {
  const crossSourceMap = { [DETERMINISTIC_PLAN_REF]: { record_type: DETERMINISTIC_PLAN_RECORD_TYPE, plan_ref: DETERMINISTIC_PLAN_REF, source_snapshot_ref: 'snapshot:some-other-snapshot', applicable: true } };
  const opts2 = { ...localOpts, resolveDeterministicPlan: makeDeterministicPlanResolver(crossSourceMap) };
  const problems = evaluateInstanceDataMigration(baseDoc(), opts2);
  assert(problems.length > 0 && problems.some((p) => p.includes('reconstruction plan for a different snapshot')), JSON.stringify(problems));
});
check('[negative] deterministic_plan_ref, разрешённый как неприменимый (applicable: false), отклоняется', () => {
  const notApplicableMap = { [DETERMINISTIC_PLAN_REF]: { record_type: DETERMINISTIC_PLAN_RECORD_TYPE, plan_ref: DETERMINISTIC_PLAN_REF, source_snapshot_ref: ROLLBACK_SNAPSHOT_REF, applicable: false } };
  const opts2 = { ...localOpts, resolveDeterministicPlan: makeDeterministicPlanResolver(notApplicableMap) };
  const problems = evaluateInstanceDataMigration(baseDoc(), opts2);
  assert(problems.length > 0 && problems.some((p) => p.includes('not marked applicable')), JSON.stringify(problems));
});
check('[negative] вымышленный restoration_evidence_ref не подтверждает обратимость', () => {
  const d = mutate((d2) => {
    payload0(d2).rollback = { source_snapshot_ref: ROLLBACK_SNAPSHOT_REF, plan: 'verified-restoration', restoration_evidence_ref: 'evidence:invented', rewrites_published_history: false };
  });
  refingerprint(d);
  const problems = evaluateLocal(d);
  assert(problems.length > 0 && problems.some((p) => p.includes('rollback.restoration_evidence_ref') && p.includes('does not resolve')), JSON.stringify(problems));
});
check('[negative] restoration_evidence_ref, разрешённый с confirms:false, не подтверждает обратимость', () => {
  const nonConfirmingMap = { [RESTORATION_EVIDENCE_REF]: { record_type: RESTORATION_EVIDENCE_RECORD_TYPE, evidence_ref: RESTORATION_EVIDENCE_REF, plan_ref: 'plan-one', source_snapshot_ref: ROLLBACK_SNAPSHOT_REF, confirms: false } };
  const d = mutate((d2) => {
    payload0(d2).rollback = { source_snapshot_ref: ROLLBACK_SNAPSHOT_REF, plan: 'verified-restoration', restoration_evidence_ref: RESTORATION_EVIDENCE_REF, rewrites_published_history: false };
  });
  refingerprint(d);
  const opts2 = { ...localOpts, resolveRestorationEvidence: makeRestorationEvidenceResolver(nonConfirmingMap) };
  const problems = evaluateInstanceDataMigration(d, opts2);
  assert(problems.length > 0 && problems.some((p) => p.includes('does not confirm restoration')), JSON.stringify(problems));
});
check('[negative] restoration_evidence_ref, разрешённый для чужого плана (cross-plan), отклоняется', () => {
  const crossPlanMap = { [RESTORATION_EVIDENCE_REF]: { record_type: RESTORATION_EVIDENCE_RECORD_TYPE, evidence_ref: RESTORATION_EVIDENCE_REF, plan_ref: 'some-other-plan', source_snapshot_ref: ROLLBACK_SNAPSHOT_REF, confirms: true } };
  const d = mutate((d2) => {
    payload0(d2).rollback = { source_snapshot_ref: ROLLBACK_SNAPSHOT_REF, plan: 'verified-restoration', restoration_evidence_ref: RESTORATION_EVIDENCE_REF, rewrites_published_history: false };
  });
  refingerprint(d);
  const opts2 = { ...localOpts, resolveRestorationEvidence: makeRestorationEvidenceResolver(crossPlanMap) };
  const problems = evaluateInstanceDataMigration(d, opts2);
  assert(problems.length > 0 && problems.some((p) => p.includes('does not name this plan')), JSON.stringify(problems));
});
check('[negative] restoration_evidence_ref, разрешённый для чужого снимка (cross-source), отклоняется', () => {
  const crossSourceMap = { [RESTORATION_EVIDENCE_REF]: { record_type: RESTORATION_EVIDENCE_RECORD_TYPE, evidence_ref: RESTORATION_EVIDENCE_REF, plan_ref: 'plan-one', source_snapshot_ref: 'snapshot:some-other-snapshot', confirms: true } };
  const d = mutate((d2) => {
    payload0(d2).rollback = { source_snapshot_ref: ROLLBACK_SNAPSHOT_REF, plan: 'verified-restoration', restoration_evidence_ref: RESTORATION_EVIDENCE_REF, rewrites_published_history: false };
  });
  refingerprint(d);
  const opts2 = { ...localOpts, resolveRestorationEvidence: makeRestorationEvidenceResolver(crossSourceMap) };
  const problems = evaluateInstanceDataMigration(d, opts2);
  assert(problems.length > 0 && problems.some((p) => p.includes('evidence for a different snapshot')), JSON.stringify(problems));
});
check('обратимость не зависит от source.qualification: rollback-резолверы проверяются и для not-reproducible источника', () => {
  const d = mutate((d2) => {
    payload0(d2).source.working_tree_clean = false;
    payload0(d2).source.qualification = 'not-reproducible';
    payload0(d2).source.qualification_reason = 'рабочее дерево источника грязное';
    payload0(d2).verification.overall_status = 'BLOCKED';
    payload0(d2).rollback.source_snapshot_ref = 'snapshot:invented';
  });
  const problems = evaluateLocal(d);
  assert(problems.length > 0 && problems.some((p) => p.includes('rollback.source_snapshot_ref') && p.includes('does not resolve')), JSON.stringify(problems));
});

// ---------------------------------------------------------------------------
// property 6 — verifiable correspondence, resolved against an external
// evidence boundary
// ---------------------------------------------------------------------------
check('VERIFIED с unverified coverage отклоняется', () => {
  const d = mutate((d2) => { payload0(d2).verification.coverage = { status: 'unverified' }; });
  assert(evaluateLocal(d).length > 0);
});
check('VERIFIED с unverified applicability_preservation отклоняется', () => {
  const d = mutate((d2) => { payload0(d2).verification.applicability_preservation = { status: 'unverified' }; });
  assert(evaluateLocal(d).length > 0);
});
check('verified sub-verdict без evidence_ref отклоняется (схема)', () => {
  const d = mutate((d2) => { delete payload0(d2).verification.coverage.evidence_ref; });
  assert(evaluateLocal(d).length > 0);
});
check('unverified sub-verdict с evidence_ref отклоняется (схема)', () => {
  const d = mutate((d2) => { payload0(d2).verification.coverage = { status: 'unverified', evidence_ref: 'x' }; });
  assert(evaluateLocal(d).length > 0);
});
check('[negative] непустая строка evidence_ref без разрешения через резолвер не подтверждает VERIFIED', () => {
  const d = mutate((d2) => { payload0(d2).verification.coverage.evidence_ref = 'evidence:fabricated-and-never-registered'; });
  const problems = evaluateLocal(d);
  assert(problems.length > 0 && problems.some((p) => p.includes('does not resolve')));
});
check('[negative] resolveEvidence отсутствует — VERIFIED отклоняется закрыто', () => {
  const d = baseDoc();
  const problems = evaluateInstanceDataMigration(d, { registrySchema, envelopeSchema, resolveSourceSnapshot: makeSourceSnapshotResolver(snapshotMap) });
  assert(problems.some((p) => p.includes('does not resolve')), 'отсутствующий resolveEvidence должен закрыто отклонить claim');
});
check('[negative] разрешённое доказательство с confirms:false не подтверждает VERIFIED', () => {
  const d = mutate((d2) => { payload0(d2).verification.coverage.evidence_ref = 'evidence:non-confirming'; });
  const problems = evaluateLocal(d);
  assert(problems.length > 0 && problems.some((p) => p.includes('does not confirm')));
});
check('[negative] разрешённое доказательство, называющее другой план, не подтверждает VERIFIED', () => {
  const d = mutate((d2) => { plan0(d2).id = 'plan-two-borrowing-evidence'; });
  // evidence:coverage-base pins plan_ref "plan-one"; the plan itself is now
  // "plan-two-borrowing-evidence" — a mismatch.
  const problems = evaluateLocal(d);
  assert(problems.length > 0 && problems.some((p) => p.includes('does not name this plan')));
});
check('разрешённое доказательство, подтверждающее именно этот план и вид, — легально', () => {
  assert(evaluateLocal(baseDoc()).length === 0);
});
check('[negative] застоявшееся доказательство: материальное изменение owner_decision меняет отпечаток, старое evidence больше не подтверждает VERIFIED', () => {
  const d = mutate((d2) => {
    payload0(d2).mappings[0].owner_decision.reason = 'существенно другое обоснование решения владельца после пересмотра плана';
  });
  refingerprint(d);
  // evidenceMap is still pinned to BASE_PAYLOAD's OLD plan_fingerprint — the
  // evidence records are left exactly as they were, unrefreshed, exactly per
  // the counterexample this checks: a materially changed plan (same id!)
  // must not still be confirmed by evidence minted for its earlier content.
  const problems = evaluateLocal(d);
  assert(problems.length > 0 && problems.some((p) => p.includes('evidence pinned to a stale or different version')), JSON.stringify(problems));
});
check('[negative] застоявшееся доказательство: материальное изменение target меняет отпечаток, старый deterministic_plan_ref больше не подтверждает обратимость', () => {
  const d = mutate((d2) => {
    payload0(d2).mappings[0].target.record_type = 'something-materially-different';
  });
  refingerprint(d);
  const problems = evaluateLocal(d);
  assert(problems.length > 0 && problems.some((p) => p.includes('reconstruction plan pinned to a stale or different version')), JSON.stringify(problems));
});

// ---------------------------------------------------------------------------
// property 7 — repeatability: canonical plan_fingerprint, derived
// idempotency_key, checked supersedes
// ---------------------------------------------------------------------------
check('несовпадающий plan_fingerprint отклоняется', () => {
  const d = mutate((d2) => { payload0(d2).plan_fingerprint = '0'.repeat(64); });
  assert(evaluateLocal(d).length > 0);
});
check('computePlanFingerprint — чистая детерминированная функция (повтор даёт тот же результат)', () => {
  const a = computePlanFingerprint(BASE_PAYLOAD);
  const b = computePlanFingerprint(clone(BASE_PAYLOAD));
  assert(a === b && /^[0-9a-f]{64}$/.test(a));
});
check('computePlanFingerprint не зависит от порядка record_units/mappings', () => {
  const reordered = clone(BASE_PAYLOAD);
  reordered.record_units = [...reordered.record_units].reverse();
  reordered.mappings = [...reordered.mappings].reverse();
  assert(computePlanFingerprint(BASE_PAYLOAD) === computePlanFingerprint(reordered));
});
check('изменение source.revision меняет отпечаток', () => {
  const changed = clone(BASE_PAYLOAD);
  changed.source.revision = 'test-revision-0002';
  assert(computePlanFingerprint(BASE_PAYLOAD) !== computePlanFingerprint(changed));
});
check('[required] изменение source.digest меняет отпечаток', () => {
  const changed = clone(BASE_PAYLOAD);
  changed.source.digest.value = 'bbbb2222'.repeat(8);
  assert(computePlanFingerprint(BASE_PAYLOAD) !== computePlanFingerprint(changed));
});
check('[required] изменение unit_ref/classification_basis меняет отпечаток', () => {
  const changed = clone(BASE_PAYLOAD);
  changed.record_units[0].classification_basis = 'другое обоснование классификации';
  assert(computePlanFingerprint(BASE_PAYLOAD) !== computePlanFingerprint(changed));
});
check('[required] изменение disposition меняет отпечаток', () => {
  const changed = clone(BASE_PAYLOAD);
  changed.mappings[0].disposition = 'retained-transitional';
  assert(computePlanFingerprint(BASE_PAYLOAD) !== computePlanFingerprint(changed));
});
check('[required] изменение полного target (record_type) меняет отпечаток', () => {
  const changed = clone(BASE_PAYLOAD);
  changed.mappings[0].target.record_type = 'something-else';
  assert(computePlanFingerprint(BASE_PAYLOAD) !== computePlanFingerprint(changed));
});
check('[required] изменение merge_rule_ref меняет отпечаток', () => {
  const base = clone(mergeFixture.registry.migration_plans[0].payload);
  const changed = clone(base);
  changed.mappings[0].merge_rule_ref = 'merge-rule:different';
  assert(computePlanFingerprint(base) !== computePlanFingerprint(changed));
});
check('[required] изменение owner_decision меняет отпечаток', () => {
  const changed = clone(BASE_PAYLOAD);
  changed.mappings[0].owner_decision.reason = 'другое обоснование решения владельца';
  assert(computePlanFingerprint(BASE_PAYLOAD) !== computePlanFingerprint(changed));
});
check('[required] изменение rollback меняет отпечаток', () => {
  const changed = clone(BASE_PAYLOAD);
  changed.rollback.deterministic_plan_ref = 'rollback-plan:different';
  assert(computePlanFingerprint(BASE_PAYLOAD) !== computePlanFingerprint(changed));
});
check('изменение verification/plan_fingerprint/idempotency_key НЕ входит в проекцию отпечатка', () => {
  const changed = clone(BASE_PAYLOAD);
  changed.verification.overall_status = 'UNVERIFIED';
  changed.verification.coverage = { status: 'unverified' };
  changed.verification.applicability_preservation = { status: 'unverified' };
  assert(computePlanFingerprint(BASE_PAYLOAD) === computePlanFingerprint(changed), 'verification не должна входить в проекцию отпечатка');
});

const SOURCE_A = { repository_ref: REPO_REF, revision: REVISION, digest: { algorithm: 'sha-256', value: DIG_A } };
check('computeIdempotencyKey — чистая детерминированная функция области и источника (repository_ref/revision/digest)', () => {
  const a = computeIdempotencyKey(SCOPE, SOURCE_A);
  const b = computeIdempotencyKey({ ...SCOPE }, { ...SOURCE_A, digest: { ...SOURCE_A.digest } });
  assert(a === b && /^[0-9a-f]{64}$/.test(a));
});
check('computeIdempotencyKey меняется при изменении revision', () => {
  assert(computeIdempotencyKey(SCOPE, SOURCE_A) !== computeIdempotencyKey(SCOPE, { ...SOURCE_A, revision: 'other-revision' }));
});
check('computeIdempotencyKey меняется при изменении области', () => {
  assert(computeIdempotencyKey(SCOPE, SOURCE_A) !== computeIdempotencyKey({ type: 'repository-scope', id: 'x', workspace_id: 'y' }, SOURCE_A));
});
check('[required] computeIdempotencyKey меняется при изменении digest', () => {
  assert(computeIdempotencyKey(SCOPE, SOURCE_A) !== computeIdempotencyKey(SCOPE, { ...SOURCE_A, digest: { algorithm: 'sha-256', value: DIG_B } }));
});
check('[required] computeIdempotencyKey меняется при изменении repository_ref (та же область, тот же текст revision)', () => {
  const a = computeIdempotencyKey(SCOPE, SOURCE_A);
  const b = computeIdempotencyKey(SCOPE, { ...SOURCE_A, repository_ref: REPO_REF_B });
  assert(a !== b, 'разные исходные репозитории не должны сталкиваться на одном ключе только из-за совпавшего текста revision');
});
check('[required] тот же вход с другим произвольным idempotency_key отклоняется', () => {
  const d = mutate((d2) => { payload0(d2).idempotency_key = '1'.repeat(64); });
  const problems = evaluateLocal(d);
  assert(problems.length > 0 && problems.some((p) => p.includes('idempotency_key')));
});
check('дублирующийся (корректно вычисленный) idempotency_key между двумя планами одного входа отклоняется', () => {
  const d = baseDoc();
  const dup = clone(plan0(d));
  dup.id = 'plan-one-dup';
  d.migration_plans.push(dup);
  assert(evaluateLocal(d).length > 0);
});
check('два плана одного pinned-входа с разными идентификаторами, но одинаковым (корректным) idempotency_key, не проходят как независимые', () => {
  const d = baseDoc();
  const dup = clone(plan0(d));
  dup.id = 'plan-one-second-attempt';
  d.migration_plans.push(dup);
  const problems = evaluateLocal(d);
  assert(problems.some((p) => p.includes('more than one migration plan')), 'повторный прогон должен быть узнан, а не пройти как независимый план');
});
check('разные ревизии одной области естественно получают разные (корректные) idempotency_key — легальны вместе', () => {
  const d = baseDoc();
  const second = clone(plan0(d));
  second.id = 'plan-two';
  second.payload.source.revision = 'test-revision-0002';
  second.payload.source.digest.value = 'bbbb2222'.repeat(8);
  second.payload.mappings[0].target.id = 'target-two';
  second.payload.mappings[0].target.origin.source_ref = deriveOriginSourceRef(['unit-one']);
  second.payload.mappings[0].target.authority.decision_ref = 'owner-decision:migrate-unit-one-v2';
  second.payload.mappings[0].owner_decision.decision_ref = 'owner-decision:migrate-unit-one-v2';
  // A new revision needs its own rollback linkage too — reusing plan-one's
  // rollback refs would resolve to plan-one's OWN (now different) source.
  const rbRefTwo = 'snapshot:sample-instance@test-revision-0002';
  const planRefTwo = 'rollback-plan:test-revision-0002';
  second.payload.rollback = {
    source_snapshot_ref: rbRefTwo,
    plan: 'deterministic-reconstruction',
    deterministic_plan_ref: planRefTwo,
    rewrites_published_history: false,
  };
  second.payload.idempotency_key = computeIdempotencyKey(SCOPE, second.payload.source);
  second.payload.supersedes = 'plan-one';
  second.payload.verification.coverage.evidence_ref = 'evidence:coverage-base-two';
  second.payload.verification.applicability_preservation.evidence_ref = 'evidence:applicability-base-two';
  second.payload.plan_fingerprint = computePlanFingerprint(second.payload);
  const localSnapshotMap = { ...snapshotMap, [`${REPO_REF}@test-revision-0002`]: { record_type: SOURCE_SNAPSHOT_RECORD_TYPE, repository_ref: REPO_REF, revision: 'test-revision-0002', digest: { algorithm: 'sha-256', value: 'bbbb2222'.repeat(8) }, working_tree_clean: true } };
  const localEvidenceMap = {
    ...evidenceMap,
    'evidence:coverage-base-two': { record_type: EVIDENCE_RECORD_TYPE, evidence_ref: 'evidence:coverage-base-two', kind: 'coverage', plan_ref: second.id, plan_fingerprint: second.payload.plan_fingerprint, confirms: true },
    'evidence:applicability-base-two': { record_type: EVIDENCE_RECORD_TYPE, evidence_ref: 'evidence:applicability-base-two', kind: 'applicability_preservation', plan_ref: second.id, plan_fingerprint: second.payload.plan_fingerprint, confirms: true },
  };
  const localRollbackSnapshotMap = { ...rollbackSnapshotMap, [rbRefTwo]: { record_type: ROLLBACK_SNAPSHOT_RECORD_TYPE, source_snapshot_ref: rbRefTwo, repository_ref: REPO_REF, revision: 'test-revision-0002', digest: { algorithm: 'sha-256', value: 'bbbb2222'.repeat(8) } } };
  const localDeterministicPlanMap = { ...deterministicPlanMap, [planRefTwo]: { record_type: DETERMINISTIC_PLAN_RECORD_TYPE, deterministic_plan_ref: planRefTwo, source_snapshot_ref: rbRefTwo, plan_ref: second.id, plan_fingerprint: second.payload.plan_fingerprint, applicable: true } };
  d.migration_plans.push(second);
  const localOptsTwo = {
    registrySchema, envelopeSchema,
    resolveSourceSnapshot: makeSourceSnapshotResolver(localSnapshotMap),
    resolveEvidence: makeEvidenceResolver(localEvidenceMap),
    resolveRollbackSnapshot: makeRollbackSnapshotResolver(localRollbackSnapshotMap),
    resolveDeterministicPlan: makeDeterministicPlanResolver(localDeterministicPlanMap),
    resolveRestorationEvidence: makeRestorationEvidenceResolver(restorationEvidenceMap),
  };
  const problems = evaluateInstanceDataMigration(d, localOptsTwo);
  assert(problems.length === 0, JSON.stringify(problems));
});
check('[required] разные repository_ref при одинаковых scope и revision дают разные idempotency_key и не сталкиваются как один и тот же вход', () => {
  const d = baseDoc();
  const second = clone(plan0(d));
  second.id = 'plan-repo-b';
  second.payload.source.repository_ref = REPO_REF_B;
  second.payload.source.digest.value = DIG_B;
  second.payload.idempotency_key = computeIdempotencyKey(SCOPE, second.payload.source);
  assert(second.payload.idempotency_key !== payload0(d).idempotency_key, 'разные repository_ref при одинаковой области и одинаковом тексте revision должны давать разный idempotency_key');
  const rbRefB = 'snapshot:sample-instance-b@test-revision-0001';
  const planRefB = 'rollback-plan:sample-instance-b@test-revision-0001';
  second.payload.rollback = {
    source_snapshot_ref: rbRefB,
    plan: 'deterministic-reconstruction',
    deterministic_plan_ref: planRefB,
    rewrites_published_history: false,
  };
  second.payload.verification.coverage.evidence_ref = 'evidence:coverage-repo-b';
  second.payload.verification.applicability_preservation.evidence_ref = 'evidence:applicability-repo-b';
  second.payload.plan_fingerprint = computePlanFingerprint(second.payload);
  d.migration_plans.push(second);
  const opts2 = {
    registrySchema, envelopeSchema,
    resolveSourceSnapshot: makeSourceSnapshotResolver(snapshotMap),
    resolveEvidence: makeEvidenceResolver({
      ...evidenceMap,
      'evidence:coverage-repo-b': { record_type: EVIDENCE_RECORD_TYPE, evidence_ref: 'evidence:coverage-repo-b', kind: 'coverage', plan_ref: second.id, plan_fingerprint: second.payload.plan_fingerprint, confirms: true },
      'evidence:applicability-repo-b': { record_type: EVIDENCE_RECORD_TYPE, evidence_ref: 'evidence:applicability-repo-b', kind: 'applicability_preservation', plan_ref: second.id, plan_fingerprint: second.payload.plan_fingerprint, confirms: true },
    }),
    resolveRollbackSnapshot: makeRollbackSnapshotResolver({
      ...rollbackSnapshotMap,
      [rbRefB]: { record_type: ROLLBACK_SNAPSHOT_RECORD_TYPE, source_snapshot_ref: rbRefB, repository_ref: REPO_REF_B, revision: REVISION, digest: { algorithm: 'sha-256', value: DIG_B } },
    }),
    resolveDeterministicPlan: makeDeterministicPlanResolver({
      ...deterministicPlanMap,
      [planRefB]: { record_type: DETERMINISTIC_PLAN_RECORD_TYPE, deterministic_plan_ref: planRefB, source_snapshot_ref: rbRefB, plan_ref: second.id, plan_fingerprint: second.payload.plan_fingerprint, applicable: true },
    }),
    resolveRestorationEvidence: makeRestorationEvidenceResolver(restorationEvidenceMap),
  };
  const problems = evaluateInstanceDataMigration(d, opts2);
  assert(problems.length === 0, JSON.stringify(problems));
});
check('supersedes, равный собственному id, отклоняется', () => {
  const d = mutate((d2) => { payload0(d2).supersedes = 'plan-one'; });
  const problems = evaluateLocal(d);
  assert(problems.length > 0 && problems.some((p) => p.includes('cannot supersede itself')));
});
// A predecessor not present in this document is NEVER accepted automatically:
// its identity is resolved through the external resolveSupersededPlan
// boundary, closed exactly like every other external boundary this module
// uses, and checked against the successor's own full scope and repository_ref.
check('[negative] supersedes неизвестного/неразрешённого predecessor (не в документе, не разрешён резолвером) отклоняется', () => {
  const d = mutate((d2) => { payload0(d2).supersedes = 'plan-unknown-and-unregistered'; });
  const problems = evaluateLocal(d);
  assert(problems.length > 0 && problems.some((p) => p.includes('does not resolve to a known predecessor')), JSON.stringify(problems));
});
check('[negative] supersedes predecessor, разрешённого с несовпадающей областью, отклоняется', () => {
  const d = mutate((d2) => { payload0(d2).supersedes = 'plan-zero'; });
  const supersededMap = {
    'plan-zero': { record_type: SUPERSEDED_PLAN_RECORD_TYPE, plan_ref: 'plan-zero', scope: { type: 'repository-scope', id: 'other-repo', workspace_id: 'other-workspace' }, repository_ref: REPO_REF, revision: 'test-revision-0000' },
  };
  const opts2 = { ...localOpts, resolveSupersededPlan: makeSupersededPlanResolver(supersededMap) };
  const problems = evaluateInstanceDataMigration(d, opts2);
  assert(problems.length > 0 && problems.some((p) => p.includes('resolved predecessor is scoped differently')), JSON.stringify(problems));
});
check('supersedes честного разрешённого predecessor того же источника и новой редакции — легален', () => {
  const d = mutate((d2) => { payload0(d2).supersedes = 'plan-zero'; });
  const supersededMap = {
    'plan-zero': { record_type: SUPERSEDED_PLAN_RECORD_TYPE, plan_ref: 'plan-zero', scope: { ...SCOPE }, repository_ref: REPO_REF, revision: 'test-revision-0000' },
  };
  const opts2 = { ...localOpts, resolveSupersededPlan: makeSupersededPlanResolver(supersededMap) };
  const problems = evaluateInstanceDataMigration(d, opts2);
  assert(problems.length === 0, JSON.stringify(problems));
});
check('[negative] supersedes предшественника с другой областью отклоняется', () => {
  const predecessor = envelope('plan-predecessor-diff-scope', clone(BASE_PAYLOAD), {
    scope: { type: 'repository-scope', id: 'other-repo', workspace_id: 'other-workspace' },
  });
  const successorPayload = clone(BASE_PAYLOAD);
  successorPayload.supersedes = 'plan-predecessor-diff-scope';
  const successor = envelope('plan-successor-diff-scope', successorPayload);
  const d = {
    $schema: '../../registries/operating-model/instance-data-migration.schema.json',
    schema_version: 1, registry_id: 'instance-data-migration', title: 'x',
    migration_plans: [predecessor, successor],
  };
  const problems = evaluateLocal(d);
  assert(problems.some((p) => p.includes('scoped differently')), JSON.stringify(problems));
});
check('[negative] supersedes предшественника с той же source.revision отклоняется', () => {
  const predecessor = envelope('plan-predecessor-same-rev', clone(BASE_PAYLOAD));
  const successorPayload = clone(BASE_PAYLOAD);
  successorPayload.supersedes = 'plan-predecessor-same-rev';
  const successor = envelope('plan-successor-same-rev', successorPayload);
  const d = {
    $schema: '../../registries/operating-model/instance-data-migration.schema.json',
    schema_version: 1, registry_id: 'instance-data-migration', title: 'x',
    migration_plans: [predecessor, successor],
  };
  const problems = evaluateLocal(d);
  assert(problems.some((p) => p.includes('identical source.revision')), JSON.stringify(problems));
});
check('[negative] преемник из другого source.repository_ref отклоняется (та же область, in-document predecessor)', () => {
  const predecessorPayload = clone(BASE_PAYLOAD);
  predecessorPayload.source.repository_ref = REPO_REF_B;
  predecessorPayload.source.revision = 'test-revision-0000';
  const predecessor = envelope('plan-predecessor-diff-repo', predecessorPayload);
  const successorPayload = clone(BASE_PAYLOAD);
  successorPayload.supersedes = 'plan-predecessor-diff-repo';
  const successor = envelope('plan-successor-diff-repo', successorPayload);
  const d = {
    $schema: '../../registries/operating-model/instance-data-migration.schema.json',
    schema_version: 1, registry_id: 'instance-data-migration', title: 'x',
    migration_plans: [predecessor, successor],
  };
  const problems = evaluateLocal(d);
  assert(problems.some((p) => p.includes('different source.repository_ref')), JSON.stringify(problems));
});

// ---------------------------------------------------------------------------
// property 8 — closed form / envelope composition / target_scope structural
// parity with workspace-scope-model
// ---------------------------------------------------------------------------
check('payload закрыт — постороннее поле отклоняется', () => {
  const d = mutate((d2) => { payload0(d2).exec = 'rm -rf /'; });
  assert(evaluateLocal(d).length > 0);
});
check('запись вне project-workspace/repository-scope отклоняется', () => {
  const d = mutate((d2) => { plan0(d2).scope = { type: 'user-profile', id: 'sample-user' }; });
  assert(evaluateLocal(d).length > 0);
});
check('origin.kind, отличный от declared, отклоняется', () => {
  const d = mutate((d2) => { plan0(d2).origin = { kind: 'derived', source_ref: 'x' }; });
  assert(evaluateLocal(d).length > 0);
});
check('authority.kind, отличный от delegated-run, на конверте плана отклоняется', () => {
  const d = mutate((d2) => { plan0(d2).authority = { kind: 'project-owner', authority_ref: 'x' }; });
  assert(evaluateLocal(d).length > 0);
});
check('record_type, отличный от instance-migration-plan, отклоняется', () => {
  const d = mutate((d2) => { plan0(d2).record_type = 'norm'; });
  assert(evaluateLocal(d).length > 0);
});
check('target_scope built-in-methodology фиксирует id (структурная параллель scope_ref)', () => {
  const t = registrySchema.definitions.target_scope.allOf[0];
  assert(t.then.properties.id.const === 'built-in-methodology');
});
check('target_scope требует workspace_id для repository-scope и run-state (структурная параллель scope_ref)', () => {
  const t = registrySchema.definitions.target_scope.allOf[1];
  assert(JSON.stringify(t.if.properties.type.enum) === JSON.stringify(['repository-scope', 'run-state']));
  assert(JSON.stringify(t.then.required) === JSON.stringify(['workspace_id']));
});

// ---------------------------------------------------------------------------
// closed pools: schema and library exports must not have drifted apart
// ---------------------------------------------------------------------------
check('закрытые пулы схемы и библиотеки совпадают', () => {
  const sourceState = registrySchema.definitions.source_state.properties;
  assert(JSON.stringify(sourceState.qualification.enum) === JSON.stringify(QUALIFICATIONS), 'пул qualification разошёлся');
  const mapping = registrySchema.definitions.mapping.properties;
  assert(JSON.stringify(mapping.disposition.enum) === JSON.stringify(DISPOSITIONS), 'пул disposition разошёлся');
  const verification = registrySchema.definitions.verification.properties;
  assert(JSON.stringify(verification.overall_status.enum) === JSON.stringify(OVERALL_STATES), 'пул overall_status разошёлся');
  const subVerdict = registrySchema.definitions.sub_verdict.properties;
  assert(JSON.stringify(subVerdict.status.enum) === JSON.stringify(VERIFICATION_STATES), 'пул verification status разошёлся');
  const rollback = registrySchema.definitions.rollback.properties;
  assert(JSON.stringify(rollback.plan.enum) === JSON.stringify(ROLLBACK_PLANS), 'пул rollback.plan разошёлся');
  const fieldBasis = registrySchema.definitions.field_basis.properties;
  assert(JSON.stringify(fieldBasis.id.enum) === JSON.stringify(FIELD_BASES), 'пул field_basis разошёлся');
  const targetOrigin = registrySchema.definitions.target_origin.properties;
  assert(targetOrigin.kind.const === TARGET_ORIGIN_KIND, 'target_origin.kind разошёлся');
  assert(registrySchema.definitions.entry.properties.record_type.const === RECORD_TYPE, 'record_type разошёлся');
});

check('константы окружения записи (origin/authority) совпадают с библиотекой', () => {
  assert(REQUIRED_ORIGIN_KIND === 'declared');
  assert(REQUIRED_AUTHORITY_KIND === 'delegated-run');
  assert(JSON.stringify(ALLOWED_SCOPE_TYPES) === JSON.stringify(['project-workspace', 'repository-scope']));
  assert(JSON.stringify(EVIDENCE_KINDS) === JSON.stringify(['coverage', 'applicability_preservation']));
});

console.log(`\n${passed} passed, ${failures.length} failed`);
if (failures.length) process.exit(1);
