#!/usr/bin/env node
/**
 * Standalone verification for the workspace compatibility qualification
 * contract (registries/operating-model/workspace-compatibility-qualification.schema.json).
 *
 * It calls the same implementation the gate calls
 * (scripts/lib/workspace-compatibility-qualification.mjs) so the two cannot
 * drift: the record envelope against scoped-record.schema.json, the
 * qualification payload against workspace-compatibility-qualification.schema.json,
 * pinned connection references (workspace_connection_refs[]) plus
 * migration_plan_ref/canonical_export_ref, resolved through external boundaries, EACH checked
 * against a recomputed sha256 content digest of what the resolver actually
 * returned, composed against the REAL evaluateExistingProjectCompatibilityMode/
 * evaluateInstanceDataMigration/evaluateInstanceCanonicalExport, scope
 * agreement across the resolved records, whether any resolved rule candidate
 * is still an undecided applicability_state "candidate", and the recomputed
 * qualification_state/blockers/open_questions relation.
 *
 * Two independent things are proven here, and this file keeps them visibly
 * separate:
 *
 *   1. The DECISION MATRIX (workspace-compatibility-qualification.md §4.1) —
 *      the nine table rows (eight logical steps) of computeQualificationState
 *      — covered by
 *      registries/operating-model/fixtures/workspace-compatibility-qualification.fixtures.json's
 *      `valid`/`invalid` arrays.
 *   2. The program's ACCEPTANCE SCENARIOS (workspace-compatibility-qualification.md
 *      §4.2) — eight named properties the whole meridian-workspace-compatibility
 *      program must hold, each proven below by a dedicated, mostly non-fixture
 *      test built on REAL rule_candidates/discovered_sources/filesystem state,
 *      never by one hand-added finding on an otherwise empty document. Seven
 *      of the eight are proven fully here. The eighth,
 *      migration-applicability-preservation, is proven only in its KERNEL
 *      part below (applicability_preservation accepted only with evidence
 *      resolved through an external boundary) — this Kernel package does not
 *      re-derive or independently re-prove a product's actual set of
 *      applicable norms, which is field evidence the next, separate
 *      Instance-repository package supplies (§4.3).
 *
 * Usage: node test/workspace-compatibility-qualification.test.mjs
 */
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

import { assertSupportedDeep } from '../scripts/lib/json-schema.mjs';
import {
  evaluateWorkspaceCompatibilityQualification,
  computeQualificationState,
  canonicalConnectionSourceRef,
  computeConnectionDigest,
  RECORD_TYPE, ALLOWED_SCOPE_TYPES, REQUIRED_ORIGIN_KIND, REQUIRED_AUTHORITY_KIND,
  QUALIFICATION_STATES, PINNED_REF_KEYS, REPOSITORY_SCOPE_MAX_CONNECTION_REFS,
} from '../scripts/lib/workspace-compatibility-qualification.mjs';
import { evaluateExistingProjectCompatibilityMode, scanDiscoveryPlan } from '../scripts/lib/existing-project-compatibility-mode.mjs';
import {
  makeSourceSnapshotResolver, makeEvidenceResolver, makeRollbackSnapshotResolver,
  makeDeterministicPlanResolver, makeRestorationEvidenceResolver, makeSupersededPlanResolver,
  makeSourceContentResolver, makeRefResolver, computePlanFingerprint, computeExportDigest,
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
const clone = (x) => JSON.parse(JSON.stringify(x));

const registrySchema = loadJson('registries/operating-model/workspace-compatibility-qualification.schema.json');
const envelopeSchema = loadJson('registries/operating-model/scoped-record.schema.json');
const compatSchema = loadJson('registries/operating-model/existing-project-compatibility-mode.schema.json');
const sourceRegistrySchema = loadJson('registries/operating-model/instruction-source-registry.schema.json');
const ruleIntakeSchema = loadJson('registries/operating-model/controlled-rule-intake.schema.json');
const migrationSchema = loadJson('registries/operating-model/instance-data-migration.schema.json');
const exportSchema = loadJson('registries/operating-model/instance-canonical-export.schema.json');
const fixtures = loadJson('registries/operating-model/fixtures/workspace-compatibility-qualification.fixtures.json');

/**
 * Builds the full options bundle evaluateWorkspaceCompatibilityQualification
 * expects, given the three top-level resolution maps (this package's own
 * external boundaries). Acceptance-scenario tests below pass their own
 * connection/plan/export maps merged with the fixture bundle's composed
 * sub-boundaries (migration_resolution/export_resolution), which are shared
 * across every plan/export this suite constructs.
 */
function buildOpts({ connectionMap = {}, planMap = {}, exportMap = {} } = {}) {
  return {
    registrySchema,
    envelopeSchema,
    resolveConnectionRecord: makeRefResolver(connectionMap),
    resolvePlanRecord: makeRefResolver(planMap),
    resolveExportRecord: makeRefResolver(exportMap),
    compatOptions: {
      registrySchema: compatSchema,
      envelopeSchema,
      sourceRegistrySchema,
      ruleIntakeSchema,
    },
    migrationOptions: {
      registrySchema: migrationSchema,
      envelopeSchema,
      resolveSourceSnapshot: makeSourceSnapshotResolver(fixtures.migration_resolution.source_snapshot_resolution),
      resolveEvidence: makeEvidenceResolver(fixtures.migration_resolution.evidence_resolution),
      resolveRollbackSnapshot: makeRollbackSnapshotResolver(fixtures.migration_resolution.rollback_snapshot_resolution),
      resolveDeterministicPlan: makeDeterministicPlanResolver(fixtures.migration_resolution.deterministic_plan_resolution),
      resolveRestorationEvidence: makeRestorationEvidenceResolver(fixtures.migration_resolution.restoration_evidence_resolution),
      resolveSupersededPlan: makeSupersededPlanResolver(fixtures.migration_resolution.superseded_plan_resolution),
    },
    exportOptions: {
      registrySchema: exportSchema,
      envelopeSchema,
      resolveSourceContent: makeSourceContentResolver(fixtures.export_resolution.source_content_resolution),
    },
  };
}

const opts = buildOpts({
  connectionMap: fixtures.connection_record_resolution,
  planMap: fixtures.plan_record_resolution,
  exportMap: fixtures.export_record_resolution,
});
const evaluate = (doc) => evaluateWorkspaceCompatibilityQualification(doc, opts);

function registryOf(entry) {
  return {
    $schema: '../workspace-compatibility-qualification.schema.json',
    schema_version: 1,
    registry_id: 'workspace-compatibility-qualification',
    title: 'Ad hoc реестр для теста',
    qualifications: [entry],
  };
}

/** Strips position-dependent index prefixes before comparing two problem
 * lists for semantic equality — "порядок объявления не влияет на результат"
 * must survive incidental "entry N" numbering, not just candidate content. */
function normalizeProblems(problems) {
  return [...problems].map((p) => p.replace(/entry \d+/g, 'entry N')).sort();
}

/* ===========================================================================
 * Schema and library constant sanity
 * ======================================================================== */

check('схема разбирается и не использует неподдерживаемых ключевых слов', () => {
  assertSupportedDeep(registrySchema, 'workspace-compatibility-qualification.schema.json');
});

check('константы схемы совпадают с библиотекой', () => {
  assert(registrySchema.properties.registry_id.const === 'workspace-compatibility-qualification');
  assert(registrySchema.definitions.entry.properties.record_type.const === RECORD_TYPE);
  assert(RECORD_TYPE === 'workspace-compatibility-qualification');
  assert(REQUIRED_ORIGIN_KIND === 'derived');
  assert(REQUIRED_AUTHORITY_KIND === 'delegated-run');
  assert(JSON.stringify([...ALLOWED_SCOPE_TYPES].sort()) === JSON.stringify(['project-workspace', 'repository-scope'].sort()));
  assert(JSON.stringify([...QUALIFICATION_STATES].sort()) === JSON.stringify(['BLOCKED', 'QUALIFIED', 'UNVERIFIED'].sort()));
  assert(JSON.stringify([...PINNED_REF_KEYS].sort()) === JSON.stringify(['id', 'reference', 'record_type', 'sha256'].sort()));
  assert(registrySchema.definitions.pinned_ref.required.includes('sha256'), 'sha256 обязателен во всех ссылках, включая каждый элемент workspace_connection_refs');
});

check('неподдерживаемое ключевое слово схемы отклоняется движком', () => {
  const broken = clone(registrySchema);
  broken.definitions.payload.patternProperties = { '^x': { type: 'string' } };
  let threw = false;
  try { assertSupportedDeep(broken, 'broken'); } catch { threw = true; }
  assert(threw, 'assertSupportedDeep пропустил неподдерживаемое ключевое слово');
});

/* ===========================================================================
 * Fixture bundle sanity — the DECISION MATRIX (§4.1), not the program's
 * acceptance scenarios (§4.2, proven further below).
 * ======================================================================== */

check('фикстуры этого контракта несут непустые valid и invalid', () => {
  assert(Array.isArray(fixtures.valid) && fixtures.valid.length > 0, 'valid пуст');
  assert(Array.isArray(fixtures.invalid) && fixtures.invalid.length > 0, 'invalid пуст');
});

check('valid несёт ровно девять строк закрытой матрицы решений §4.1', () => {
  assert(fixtures.valid.length === 9, `ожидалось 9 строк, найдено ${fixtures.valid.length}`);
  assert(fixtures.valid.every((c) => c.note.startsWith('decision-matrix branch')), 'valid-фикстуры этого файла обязаны быть помечены как строки матрицы решений, не как сценарии программы');
});

check('каждая valid-фикстура проходит evaluateWorkspaceCompatibilityQualification без проблем', () => {
  for (const c of fixtures.valid) {
    const problems = evaluate(c.registry);
    assert(problems.length === 0, `фикстура "${c.note}" отклонена: ${problems[0]}`);
  }
});

check('каждая invalid-фикстура отклоняется evaluateWorkspaceCompatibilityQualification', () => {
  for (const c of fixtures.invalid) {
    const problems = evaluate(c.registry);
    assert(problems.length > 0, `фикстура "${c.note}" прошла проверку чисто`);
  }
});

check('valid-фикстуры покрывают все три состояния квалификации', () => {
  const states = new Set(fixtures.valid.map((c) => c.registry.qualifications[0].payload.qualification_state));
  assert(states.has('QUALIFIED') && states.has('BLOCKED') && states.has('UNVERIFIED'), JSON.stringify([...states]));
});

check('branch 4 (нерешённые кандидаты) реально несёт два разрешённых rule_candidates с applicability_state "candidate"', () => {
  const c = fixtures.valid.find((x) => x.note.includes('branch 4'));
  const ref = c.registry.qualifications[0].payload.workspace_connection_refs[0];
  const connection = fixtures.connection_record_resolution[ref.reference];
  const undecided = connection.payload.rule_candidates.filter((w) => w.candidate.payload.applicability_state === 'candidate');
  assert(undecided.length === 2, 'сценарий обязан опираться на настоящие нерешённые rule_candidates, а не на одну находку при пустом массиве');
  assert(c.registry.qualifications[0].payload.qualification_state === 'UNVERIFIED');
  assert(c.registry.qualifications[0].payload.open_questions.length > 0);
});

/* Point 8, items 1-3: three of the four required negative tests are already
 * exercised by "каждая invalid-фикстура отклонена" above; the assertions
 * below pin the exact fixture note for direct traceability. */
check('негативный тест: попытка встроить raw imported content напрямую в payload отклонена', () => {
  const c = fixtures.invalid.find((x) => x.note.includes('raw_excerpt'));
  assert(c, 'фикстура для этого негативного теста не найдена');
  assert(evaluate(c.registry).length > 0);
});
check('негативный тест: попытка встроить secret-bearing поле напрямую в payload отклонена', () => {
  const c = fixtures.invalid.find((x) => x.note.includes('секретное значение'));
  assert(c, 'фикстура для этого негативного теста не найдена');
  assert(evaluate(c.registry).length > 0);
});
check('негативный тест: canonical_export_ref, разрешающийся в экспорт другого plan_fingerprint, отклонён', () => {
  const c = fixtures.invalid.find((x) => x.note.includes('называющий не тот составленный план'));
  assert(c, 'фикстура для этого негативного теста не найдена');
  assert(evaluate(c.registry).length > 0);
});
check('негативный тест: QUALIFIED с непустым open_questions отклонён', () => {
  const c = fixtures.invalid.find((x) => x.note.includes('QUALIFIED с непустым open_questions'));
  assert(c, 'фикстура для этого негативного теста не найдена');
  assert(evaluate(c.registry).length > 0);
});
check('негативный тест: заявленный QUALIFIED поверх нерешённых кандидатов правил отклонён', () => {
  const c = fixtures.invalid.find((x) => x.note.includes('поверх нерешённых кандидатов'));
  assert(c, 'фикстура для этого негативного теста не найдена');
  assert(evaluate(c.registry).length > 0);
});
check('негативный тест: workspace_connection_refs[0].sha256 расходится с пересчитанным digest отклонён', () => {
  const c = fixtures.invalid.find((x) => x.note.includes('workspace_connection_refs[0].sha256'));
  assert(c, 'фикстура для этого негативного теста не найдена');
  const problems = evaluate(c.registry);
  assert(problems.length > 0);
  assert(problems.some((p) => p.includes('workspace_connection_refs[0].sha256')), JSON.stringify(problems));
});

/* ===========================================================================
 * computeQualificationState — pure decision-matrix function, now aggregated
 * over an array of next_steps (one per declared workspace_connection_refs
 * entry) rather than one scalar. A single-element array reproduces every
 * branch a single connection already exercised; the additional checks below
 * prove the "any of N" aggregation and its order-independence directly.
 * ======================================================================== */

check('computeQualificationState: resolve-conflict/resolve-ambiguity → BLOCKED', () => {
  assert(computeQualificationState({ nextSteps: ['resolve-conflict'] }) === 'BLOCKED');
  assert(computeQualificationState({ nextSteps: ['resolve-ambiguity'] }) === 'BLOCKED');
});
check('computeQualificationState: await-owner-decision → UNVERIFIED', () => {
  assert(computeQualificationState({ nextSteps: ['await-owner-decision'] }) === 'UNVERIFIED');
});
check('computeQualificationState: нераспознанный next_step → UNVERIFIED (explicit-unknown)', () => {
  assert(computeQualificationState({ nextSteps: ['invented-next-step'] }) === 'UNVERIFIED');
});
check('computeQualificationState: пустой массив next_steps → UNVERIFIED (fail-closed по умолчанию)', () => {
  assert(computeQualificationState({ nextSteps: [] }) === 'UNVERIFIED');
});
check('computeQualificationState: устаревший скалярный nextStep поддержан как алиас массива из одного элемента — не превращается молча в UNVERIFIED', () => {
  assert(computeQualificationState({ nextStep: 'continue-compatibility-mode', hasUndecidedCandidates: false, hasMigrationPlan: false }) === 'QUALIFIED', 'старый вызов с одним чистым next_step обязан остаться QUALIFIED, а не молча стать UNVERIFIED из-за пустого nextSteps');
  assert(computeQualificationState({ nextStep: 'resolve-conflict' }) === 'BLOCKED');
  assert(computeQualificationState({ nextStep: 'await-owner-decision' }) === 'UNVERIFIED');
  assert(computeQualificationState({ nextSteps: [], nextStep: 'continue-compatibility-mode' }) === 'UNVERIFIED', 'явно переданный nextSteps (включая пустой массив) обязан иметь приоритет над устаревшим nextStep');
});
check('computeQualificationState: continue-compatibility-mode, все кандидаты решены, без плана → QUALIFIED', () => {
  assert(computeQualificationState({ nextSteps: ['continue-compatibility-mode'], hasUndecidedCandidates: false, hasMigrationPlan: false }) === 'QUALIFIED');
});
check('computeQualificationState: несколько connection, все continue-compatibility-mode, без плана → QUALIFIED', () => {
  assert(computeQualificationState({ nextSteps: ['continue-compatibility-mode', 'continue-compatibility-mode', 'continue-compatibility-mode'], hasUndecidedCandidates: false, hasMigrationPlan: false }) === 'QUALIFIED');
});
check('computeQualificationState: любой один конфликт среди чистых connection → BLOCKED, независимо от позиции', () => {
  assert(computeQualificationState({ nextSteps: ['continue-compatibility-mode', 'resolve-conflict', 'continue-compatibility-mode'], hasUndecidedCandidates: false, hasMigrationPlan: false }) === 'BLOCKED');
  assert(computeQualificationState({ nextSteps: ['resolve-conflict', 'continue-compatibility-mode', 'continue-compatibility-mode'], hasUndecidedCandidates: false, hasMigrationPlan: false }) === 'BLOCKED');
  assert(computeQualificationState({ nextSteps: ['continue-compatibility-mode', 'continue-compatibility-mode', 'resolve-conflict'], hasUndecidedCandidates: false, hasMigrationPlan: false }) === 'BLOCKED');
});
check('computeQualificationState: один await-owner-decision среди чистых connection → UNVERIFIED', () => {
  assert(computeQualificationState({ nextSteps: ['continue-compatibility-mode', 'await-owner-decision'], hasUndecidedCandidates: false, hasMigrationPlan: false }) === 'UNVERIFIED');
});
check('computeQualificationState: continue-compatibility-mode, но есть нерешённый кандидат → UNVERIFIED, даже без плана', () => {
  assert(computeQualificationState({ nextSteps: ['continue-compatibility-mode'], hasUndecidedCandidates: true, hasMigrationPlan: false }) === 'UNVERIFIED');
});
check('computeQualificationState: нерешённый кандидат форсирует UNVERIFIED даже при VERIFIED-плане с экспортом', () => {
  assert(computeQualificationState({
    nextSteps: ['continue-compatibility-mode'], hasUndecidedCandidates: true, hasMigrationPlan: true,
    migrationOverallStatus: 'VERIFIED', mintsRecords: true, hasCanonicalExport: true,
  }) === 'UNVERIFIED');
});
check('computeQualificationState: план BLOCKED форсирует BLOCKED', () => {
  assert(computeQualificationState({
    nextSteps: ['continue-compatibility-mode'], hasUndecidedCandidates: false, hasMigrationPlan: true, migrationOverallStatus: 'BLOCKED',
  }) === 'BLOCKED');
});
check('computeQualificationState: план UNVERIFIED → UNVERIFIED', () => {
  assert(computeQualificationState({
    nextSteps: ['continue-compatibility-mode'], hasUndecidedCandidates: false, hasMigrationPlan: true, migrationOverallStatus: 'UNVERIFIED',
  }) === 'UNVERIFIED');
});
check('computeQualificationState: VERIFIED без минченных записей и без экспорта → QUALIFIED', () => {
  assert(computeQualificationState({
    nextSteps: ['continue-compatibility-mode'], hasUndecidedCandidates: false, hasMigrationPlan: true, migrationOverallStatus: 'VERIFIED',
    mintsRecords: false, hasCanonicalExport: false,
  }) === 'QUALIFIED');
});
check('computeQualificationState: VERIFIED, минтит запись, без экспорта → UNVERIFIED', () => {
  assert(computeQualificationState({
    nextSteps: ['continue-compatibility-mode'], hasUndecidedCandidates: false, hasMigrationPlan: true, migrationOverallStatus: 'VERIFIED',
    mintsRecords: true, hasCanonicalExport: false,
  }) === 'UNVERIFIED');
});
check('computeQualificationState: VERIFIED, минтит запись, экспорт составлен → QUALIFIED', () => {
  assert(computeQualificationState({
    nextSteps: ['continue-compatibility-mode'], hasUndecidedCandidates: false, hasMigrationPlan: true, migrationOverallStatus: 'VERIFIED',
    mintsRecords: true, hasCanonicalExport: true,
  }) === 'QUALIFIED');
});
check('canonicalConnectionSourceRef: множество из нескольких id детерминировано и не зависит от порядка', () => {
  assert(canonicalConnectionSourceRef(['repo-b', 'repo-a']) === canonicalConnectionSourceRef(['repo-a', 'repo-b']));
  assert(canonicalConnectionSourceRef(['repo-a', 'repo-b']) === 'workspace-connection-scan:repo-a+repo-b');
});
check('canonicalConnectionSourceRef — детерминированная ссылка на сканирование', () => {
  assert(canonicalConnectionSourceRef('sample-scan') === 'workspace-connection-scan:sample-scan');
});
check('computeConnectionDigest — детерминированная функция содержимого, чувствительная к изменению payload', () => {
  const a = { id: 'x', title: 'T', record_type: 'workspace-connection-scan', scope: { type: 'project-workspace', id: 'p' }, origin: { kind: 'declared', source_ref: 'o' }, authority: { kind: 'delegated-run', authority_ref: 'a' }, payload: { next_step: 'continue-compatibility-mode' } };
  const b = clone(a);
  b.payload.next_step = 'await-owner-decision';
  assert(computeConnectionDigest(a) === computeConnectionDigest(clone(a)), 'digest не детерминирован');
  assert(computeConnectionDigest(a) !== computeConnectionDigest(b), 'digest не чувствителен к изменению содержимого');
});

/* ===========================================================================
 * Mandatory composition schemas: absent regardless of content
 * ======================================================================== */

const base = () => clone(fixtures.valid.find((c) => c.note.includes('branch 5')).registry);

check('registrySchema отсутствует — отказ, а не тихий пропуск', () => {
  const p = evaluateWorkspaceCompatibilityQualification(base(), { ...opts, registrySchema: undefined });
  assert(p.length === 1 && /registrySchema/.test(p[0]), JSON.stringify(p));
});
check('envelopeSchema отсутствует — отказ, а не тихий пропуск', () => {
  const p = evaluateWorkspaceCompatibilityQualification(base(), { ...opts, envelopeSchema: undefined });
  assert(p.length === 1 && /envelopeSchema/.test(p[0]), JSON.stringify(p));
});
check('composed schema отсутствует независимо от содержимого — отказ (compatOptions.ruleIntakeSchema)', () => {
  const brokenOpts = clone(opts);
  delete brokenOpts.compatOptions.ruleIntakeSchema;
  const p = evaluateWorkspaceCompatibilityQualification(base(), brokenOpts);
  assert(p.length === 1 && /ruleIntakeSchema/.test(p[0]), JSON.stringify(p));
});
check('composed schema отсутствует независимо от содержимого — отказ (migrationOptions.registrySchema), даже если ни один plan не составлен', () => {
  const brokenOpts = clone(opts);
  delete brokenOpts.migrationOptions.registrySchema;
  const p = evaluateWorkspaceCompatibilityQualification(base(), brokenOpts);
  assert(p.length === 1 && /migrationOptions.registrySchema/.test(p[0]), JSON.stringify(p));
});
check('composed schema отсутствует независимо от содержимого — отказ (exportOptions.envelopeSchema), даже если ни один export не составлен', () => {
  const brokenOpts = clone(opts);
  delete brokenOpts.exportOptions.envelopeSchema;
  const p = evaluateWorkspaceCompatibilityQualification(base(), brokenOpts);
  assert(p.length === 1 && /exportOptions.envelopeSchema/.test(p[0]), JSON.stringify(p));
});

/* ===========================================================================
 * Targeted structural mutations
 * ======================================================================== */

check('дублирующийся id квалификации отклоняется', () => {
  const d = base();
  const dup = clone(d.qualifications[0]);
  d.qualifications.push(dup);
  const p = evaluate(d);
  assert(p.some((x) => x.includes('is declared more than once')), JSON.stringify(p));
});
check('origin.source_ref, не совпадающий с canonicalConnectionSourceRef, отклоняется', () => {
  const d = base();
  d.qualifications[0].origin.source_ref = 'workspace-connection-scan:some-other-scan';
  assert(evaluate(d).length > 0);
});
check('scope.type вне ALLOWED_SCOPE_TYPES отклоняется', () => {
  const d = base();
  d.qualifications[0].scope = { type: 'run-state', id: 'sample-run', workspace_id: 'sample-project' };
  assert(evaluate(d).length > 0);
});
check('payload.workspace_connection_refs отсутствует (null) — отказ', () => {
  const d = base();
  d.qualifications[0].payload.workspace_connection_refs = null;
  const p = evaluate(d);
  assert(p.some((x) => x.includes('workspace_connection_refs is missing or empty')), JSON.stringify(p));
});
check('payload.workspace_connection_refs[0] не разрешается через внешнюю границу — отказ', () => {
  const d = base();
  d.qualifications[0].payload.workspace_connection_refs[0].reference = 'unknown-reference';
  const p = evaluate(d);
  assert(p.some((x) => x.includes('does not resolve through the external boundary')), JSON.stringify(p));
});

/* ===========================================================================
 * Point 1 — each entry of workspace_connection_refs is a REAL pinned
 * reference: sha256 is mandatory and content substitution under the SAME
 * id/reference is caught.
 * ======================================================================== */

check('workspace_connection_refs[0]: подмена содержимого при тех же id/reference отклонена (пересчитанный digest расходится)', () => {
  const good = base();
  const connectionRefObj = good.qualifications[0].payload.workspace_connection_refs[0];
  const original = fixtures.connection_record_resolution[connectionRefObj.reference];
  assert(connectionRefObj.sha256 === computeConnectionDigest(original), 'фикстура сама должна быть согласована перед тестом подмены');

  const tampered = clone(original);
  tampered.payload.next_step = 'await-owner-decision';
  assert(computeConnectionDigest(tampered) !== computeConnectionDigest(original), 'тестовая подмена обязана менять digest');

  const tamperedOpts = buildOpts({
    connectionMap: { [connectionRefObj.reference]: tampered },
    planMap: fixtures.plan_record_resolution,
    exportMap: fixtures.export_record_resolution,
  });
  const problems = evaluateWorkspaceCompatibilityQualification(good, tamperedOpts);
  assert(problems.some((p) => p.includes('workspace_connection_refs[0].sha256') && p.includes('does not equal the resolved connection')), JSON.stringify(problems));
});

/* ===========================================================================
 * payload.workspace_connection_refs — multiple entries: array-level
 * properties (minItems, no duplicate declared ref, exact repository set,
 * per-entry independence, order-independence) proven directly, on top of the
 * richer multi-repository-workspace acceptance scenario further below.
 * ======================================================================== */

function secondCleanConnection(overrides = {}) {
  const base = fixtures.connection_record_resolution['connection:sample-connection-clean'];
  return {
    ...clone(base),
    id: overrides.id ?? 'sample-connection-clean-two',
    scope: overrides.scope ?? base.scope,
    payload: {
      ...clone(base.payload),
      repository: overrides.repository ?? { id: 'sample-project-repository-two', workspace_id: 'sample-project' },
    },
  };
}

check('негативный тест: payload.workspace_connection_refs пустой массив отклонён', () => {
  const d = base();
  d.qualifications[0].payload.workspace_connection_refs = [];
  const p = evaluate(d);
  assert(p.some((x) => x.includes('workspace_connection_refs is missing or empty')), JSON.stringify(p));
});

check('негативный тест: повтор reference в workspace_connection_refs отклонён', () => {
  const d = base();
  const ref = d.qualifications[0].payload.workspace_connection_refs[0];
  d.qualifications[0].payload.workspace_connection_refs = [ref, clone(ref)];
  const p = evaluate(d);
  assert(p.some((x) => x.includes('repeats') && x.includes('.reference')), JSON.stringify(p));
});

check('негативный тест: повтор id (разный reference) в workspace_connection_refs отклонён', () => {
  const second = secondCleanConnection({ id: 'sample-connection-clean' });
  const d = base();
  const ref = d.qualifications[0].payload.workspace_connection_refs[0];
  const secondRef = { record_type: 'workspace-connection-scan', id: 'sample-connection-clean', reference: 'connection:sample-connection-clean-alias', sha256: computeConnectionDigest(second) };
  d.qualifications[0].payload.workspace_connection_refs = [ref, secondRef];
  const localOpts = buildOpts({
    connectionMap: { ...fixtures.connection_record_resolution, 'connection:sample-connection-clean-alias': second },
    planMap: fixtures.plan_record_resolution,
    exportMap: fixtures.export_record_resolution,
  });
  const p = evaluateWorkspaceCompatibilityQualification(d, localOpts);
  assert(p.some((x) => x.includes('repeats') && x.includes('.id')), JSON.stringify(p));
});

check('негативный тест: два разных connection называют один и тот же payload.repository.id — отклонено', () => {
  const second = secondCleanConnection({ id: 'sample-connection-clean-same-repo', repository: { id: 'sample-project-repository', workspace_id: 'sample-project' } });
  const d = base();
  const ref = d.qualifications[0].payload.workspace_connection_refs[0];
  const secondRef = { record_type: 'workspace-connection-scan', id: second.id, reference: 'connection:sample-connection-clean-same-repo', sha256: computeConnectionDigest(second) };
  d.qualifications[0].payload.workspace_connection_refs = [ref, secondRef];
  d.qualifications[0].origin.source_ref = canonicalConnectionSourceRef([ref.id, second.id]);
  const localOpts = buildOpts({
    connectionMap: { ...fixtures.connection_record_resolution, 'connection:sample-connection-clean-same-repo': second },
    planMap: fixtures.plan_record_resolution,
    exportMap: fixtures.export_record_resolution,
  });
  const p = evaluateWorkspaceCompatibilityQualification(d, localOpts);
  assert(p.some((x) => x.includes('all naming the same payload.repository.id')), JSON.stringify(p));
});

check('негативный тест: workspace_repository_ids расходится с фактически просканированными репозиториями', () => {
  const d = base();
  d.qualifications[0].payload.workspace_repository_ids = ['a-repository-never-scanned'];
  const p = evaluate(d);
  assert(p.some((x) => x.includes('workspace_repository_ids is missing') || x.includes('workspace_repository_ids declares')), JSON.stringify(p));
});

check('негативный тест: connection другого workspace в том же массиве — отклонено (scope не совпадает)', () => {
  const second = secondCleanConnection({
    id: 'sample-connection-other-workspace',
    scope: { type: 'project-workspace', id: 'sample-project-other' },
    repository: { id: 'sample-project-repository-two', workspace_id: 'sample-project-other' },
  });
  const d = base();
  const ref = d.qualifications[0].payload.workspace_connection_refs[0];
  const secondRef = { record_type: 'workspace-connection-scan', id: second.id, reference: 'connection:sample-connection-other-workspace', sha256: computeConnectionDigest(second) };
  d.qualifications[0].payload.workspace_connection_refs = [ref, secondRef];
  d.qualifications[0].payload.workspace_repository_ids = ['sample-project-repository', 'sample-project-repository-two'];
  d.qualifications[0].origin.source_ref = canonicalConnectionSourceRef([ref.id, second.id]);
  const localOpts = buildOpts({
    connectionMap: { ...fixtures.connection_record_resolution, 'connection:sample-connection-other-workspace': second },
    planMap: fixtures.plan_record_resolution,
    exportMap: fixtures.export_record_resolution,
  });
  const p = evaluateWorkspaceCompatibilityQualification(d, localOpts);
  assert(p.some((x) => x.includes("scope does not match the resolved payload.workspace_connection_refs[1] record's scope")), JSON.stringify(p));
});

check('негативный тест: неразрешённая вторая ссылка в массиве — отклонено', () => {
  const d = base();
  const ref = d.qualifications[0].payload.workspace_connection_refs[0];
  const secondRef = { record_type: 'workspace-connection-scan', id: 'sample-connection-nowhere', reference: 'connection:does-not-exist', sha256: '0'.repeat(64) };
  d.qualifications[0].payload.workspace_connection_refs = [ref, secondRef];
  d.qualifications[0].payload.workspace_repository_ids = ['sample-project-repository'];
  d.qualifications[0].origin.source_ref = canonicalConnectionSourceRef([ref.id, secondRef.id]);
  const p = evaluate(d);
  assert(p.some((x) => x.includes('payload.workspace_connection_refs[1] "connection:does-not-exist" does not resolve through the external boundary')), JSON.stringify(p));
});

check('негативный тест: неверный digest одного из двух connection в массиве отклонён', () => {
  const second = secondCleanConnection();
  const d = base();
  const ref = d.qualifications[0].payload.workspace_connection_refs[0];
  const secondRef = { record_type: 'workspace-connection-scan', id: second.id, reference: 'connection:sample-connection-clean-two', sha256: '1'.repeat(64) };
  d.qualifications[0].payload.workspace_connection_refs = [ref, secondRef];
  d.qualifications[0].payload.workspace_repository_ids = ['sample-project-repository', 'sample-project-repository-two'];
  d.qualifications[0].origin.source_ref = canonicalConnectionSourceRef([ref.id, second.id]);
  const localOpts = buildOpts({
    connectionMap: { ...fixtures.connection_record_resolution, 'connection:sample-connection-clean-two': second },
    planMap: fixtures.plan_record_resolution,
    exportMap: fixtures.export_record_resolution,
  });
  const p = evaluateWorkspaceCompatibilityQualification(d, localOpts);
  assert(p.some((x) => x.includes('payload.workspace_connection_refs[1].sha256') && x.includes('does not equal the resolved connection')), JSON.stringify(p));
});

/* ===========================================================================
 * repository-scope: payload.workspace_connection_refs is closed to AT MOST
 * ONE entry — one repository, one scan; a second scan of the SAME single
 * repository is never a second legal entry, and multiple repositories are
 * expressed only through one project-workspace record (§2), never through
 * several repository-scope records.
 * ======================================================================== */

const REPO_SCOPE_SOLO = { type: 'repository-scope', id: 'sample-repository-solo', workspace_id: 'sample-workspace-solo' };
function repoScopeConnection(id) {
  return {
    $schema: '../scoped-record.schema.json',
    schema_version: 1,
    id,
    title: `Сканирование — ${id}`,
    record_type: 'workspace-connection-scan',
    scope: REPO_SCOPE_SOLO,
    origin: { kind: 'declared', source_ref: `owner-decision:connect-${id}` },
    authority: { kind: 'delegated-run', authority_ref: `existing-project-compatibility-mode-run:${id}` },
    payload: {
      connection_mode: 'compatibility',
      repository: { id: REPO_SCOPE_SOLO.id, workspace_id: REPO_SCOPE_SOLO.workspace_id },
      scan_kind: 'initial',
      discovery_plan: [],
      discovered_sources: [],
      missing_sources: [],
      unreadable_sources: [],
      rule_candidates: [],
      findings: [],
      next_step: 'continue-compatibility-mode',
    },
  };
}

check('константа REPOSITORY_SCOPE_MAX_CONNECTION_REFS равна 1 и совпадает со схемой', () => {
  assert(REPOSITORY_SCOPE_MAX_CONNECTION_REFS === 1);
});

check('позитивный тест: repository-scope с РОВНО одним workspace_connection_refs проходит', () => {
  const solo = repoScopeConnection('scenario-repo-scope-solo');
  const result = qualifyConnection(solo, { state: 'QUALIFIED', reason: 'ровно один разрешённый скан единственного репозитория этой области; миграция не заявлена' });
  assert(result.problems.length === 0, JSON.stringify(result.problems));
});

check('негативный тест: repository-scope с ДВУМЯ элементами workspace_connection_refs отклонён (превышен maxItems)', () => {
  const soloA = repoScopeConnection('scenario-repo-scope-solo-a');
  const soloB = repoScopeConnection('scenario-repo-scope-solo-b');
  const result = qualifyConnection([soloA, soloB], { state: 'QUALIFIED', reason: 'намеренно два элемента в repository-scope — проверка обязана отклонить' });
  assert(result.problems.some((p) => p.includes('may carry at most') || p.includes('more than 1 items')), JSON.stringify(result.problems));
});

/* ===========================================================================
 * Point 6 (exact-version linkage): an independently configured
 * exportOptions.resolveMigrationPlan cannot substitute a different plan
 * sharing the same id. This module never even forwards it.
 * ======================================================================== */

check('exportOptions.resolveMigrationPlan, даже настроенный на другой план с тем же id, не влияет на результат', () => {
  const good = clone(fixtures.valid.find((c) => c.note.includes('branch 9')).registry);
  const cleanProblems = evaluate(good);
  assert(cleanProblems.length === 0, JSON.stringify(cleanProblems));

  const maliciousOpts = {
    ...opts,
    exportOptions: {
      ...opts.exportOptions,
      resolveMigrationPlan: () => ({
        record_type: 'instance-migration-plan',
        plan_ref: 'sample-migration-plan-one',
        plan_fingerprint: '0'.repeat(64),
        scope: { type: 'project-workspace', id: 'sample-project' },
        source: { repository_ref: 'attacker-controlled', revision: 'attacker-revision', digest: { algorithm: 'sha-256', value: '1'.repeat(64) } },
        record_units: [],
        mappings: [],
      }),
    },
  };
  const withMaliciousResolver = evaluateWorkspaceCompatibilityQualification(good, maliciousOpts);
  assert(withMaliciousResolver.length === 0, `сторонний resolveMigrationPlan повлиял на результат: ${JSON.stringify(withMaliciousResolver)}`);
});

/* ===========================================================================
 * Acceptance scenarios of the program (workspace-compatibility-qualification.md
 * §4.2) — a separate, named set from the decision matrix above. Each is
 * proven on REAL data (real rule_candidates/discovered_sources/filesystem
 * state), never by one hand-added finding on an otherwise empty document.
 * ======================================================================== */

const PROJECT_SCOPE = { type: 'project-workspace', id: 'sample-project' };
function baseConnectionEnvelope(id) {
  return {
    $schema: '../scoped-record.schema.json',
    schema_version: 1,
    id,
    title: `Сканирование — ${id}`,
    record_type: 'workspace-connection-scan',
    scope: PROJECT_SCOPE,
    origin: { kind: 'declared', source_ref: `owner-decision:connect-${id}` },
    authority: { kind: 'delegated-run', authority_ref: `existing-project-compatibility-mode-run:${id}` },
  };
}
function discoveredSource({
  id, scope = PROJECT_SCOPE, revision, digest, format = 'agents-md', filePath = 'AGENTS.md',
  containerRef = 'sample-project-repository', readChannel = { kind: 'meridian-observed', meridian_visibility: 'full', agent_auto_read: false },
  divergence = { status: 'unknown' },
}) {
  return {
    $schema: '../scoped-record.schema.json',
    schema_version: 1,
    id,
    title: `Источник — ${id}`,
    record_type: 'instruction-source',
    scope,
    origin: { kind: 'declared', source_ref: `workspace-connection-scan:${id}-scan` },
    authority: { kind: 'delegated-run', authority_ref: 'existing-project-compatibility-mode-run:sample-pass' },
    payload: {
      normative_status: 'not-a-norm',
      medium: 'file',
      location: { path: filePath, container_ref: containerRef, missing_behavior: 'fail-closed' },
      recorded_state: { revision, digest: { algorithm: 'sha-256', value: digest }, revision_verified: true, currency: 'current' },
      format,
      read_channel: readChannel,
      divergence,
    },
  };
}
function ruleCandidate({
  id, scope = PROJECT_SCOPE, sourceId, revision, digest, boundaryStart, boundaryEnd,
  semanticKey, classificationBasis, applicabilityState, notApplicableReason, conflictsWith,
  ownerDecisionRef, authorityKind = 'project-owner', authorityRef = 'sample-project-owner',
}) {
  const reference = `instruction-source-registry:${sourceId}@${revision}`;
  const authority = applicabilityState === 'candidate'
    ? { kind: 'delegated-run', authority_ref: 'controlled-rule-intake-run:sample-pass' }
    : { kind: authorityKind, authority_ref: authorityRef, decision_ref: ownerDecisionRef };
  const payload = {
    source_ref: { record_type: 'instruction-source', id: sourceId, reference, revision, sha256: digest },
    boundary: { unit: 'line-range', start: boundaryStart, end: boundaryEnd },
    raw_excerpt: `дословный текст ${id}`,
    normalized_text: `нормализованный текст ${id}`,
    semantic_key: semanticKey,
    classification_basis: classificationBasis,
    applicability_state: applicabilityState,
  };
  if (conflictsWith) payload.conflicts_with = conflictsWith;
  if (applicabilityState !== 'candidate') {
    payload.owner_decision = { decision_ref: ownerDecisionRef, decided_at: '2026-09-10', reason: `решение владельца по ${id}` };
  }
  if (applicabilityState === 'not-applicable') payload.not_applicable_reason = notApplicableReason;
  return {
    $schema: '../scoped-record.schema.json',
    schema_version: 1,
    id,
    title: `Кандидат — ${id}`,
    record_type: 'rule-candidate',
    scope,
    origin: { kind: 'derived', source_ref: `instruction-source:${sourceId}` },
    authority,
    payload,
  };
}

/**
 * Wraps one or more hand-built workspace-connection-scan records in a full
 * workspace-compatibility-qualification entry and evaluates it end to end
 * (through the SAME resolver-based composition the fixtures use), returning
 * { problems, entry }. `connectionOrConnections` accepts either a single
 * connection (every single-repository scenario below) or an array of two or
 * more (the multi-repository-workspace scenario) — the SAME helper, not a
 * second code path, so a single-connection caller proves nothing a
 * multi-connection caller could not also prove. `expected` supplies the
 * declared qualification_state/blockers/open_questions/reason this test
 * asserts against — the caller states the property under test, this helper
 * never guesses it. Each connection's own sha256 pin is always computed here
 * from the ACTUAL connection object passed in, mirroring what a real caller
 * would do. `expected.workspaceRepositoryIds`, when supplied, overrides the
 * default (the exact set of payload.repository.id every connection declares)
 * — used only by tests that deliberately mismatch it.
 */
function qualifyConnection(connectionOrConnections, expected) {
  const connections = Array.isArray(connectionOrConnections) ? connectionOrConnections : [connectionOrConnections];
  const scope = connections[0].scope;
  const label = connections.map((c) => c.id).join('-and-');
  const workspaceConnectionRefs = connections.map((c) => (
    { record_type: 'workspace-connection-scan', id: c.id, reference: `ad-hoc:${c.id}`, sha256: computeConnectionDigest(c) }
  ));
  const defaultRepositoryIds = [...new Set(connections.map((c) => c.payload?.repository?.id).filter((v) => typeof v === 'string'))];
  const entry = {
    $schema: '../scoped-record.schema.json',
    schema_version: 1,
    id: `qualification-of-${label}`,
    title: `Квалификация — ${label}`,
    record_type: 'workspace-compatibility-qualification',
    scope,
    origin: { kind: 'derived', source_ref: canonicalConnectionSourceRef(connections.map((c) => c.id)) },
    authority: { kind: 'delegated-run', authority_ref: `workspace-compatibility-qualification-run:${label}` },
    payload: {
      workspace_connection_refs: workspaceConnectionRefs,
      ...(scope.type === 'project-workspace' ? { workspace_repository_ids: expected.workspaceRepositoryIds ?? defaultRepositoryIds } : {}),
      migration_plan_ref: expected.migrationPlanRef ?? null,
      canonical_export_ref: expected.canonicalExportRef ?? null,
      qualification_state: expected.state,
      qualification_reason: expected.reason,
      blockers: expected.blockers ?? [],
      open_questions: expected.openQuestions ?? [],
    },
  };
  const localOpts = buildOpts({
    connectionMap: Object.fromEntries(connections.map((c) => [`ad-hoc:${c.id}`, c])),
    planMap: { ...fixtures.plan_record_resolution, ...(expected.extraPlanMap || {}) },
    exportMap: { ...fixtures.export_record_resolution, ...(expected.extraExportMap || {}) },
  });
  const problems = evaluateWorkspaceCompatibilityQualification(registryOf(entry), localOpts);
  return { problems, entry };
}

/* --- scenario:empty-project-no-instance --- */
check('scenario:empty-project-no-instance — реальный пустой временный каталог: scanDiscoveryPlan и квалификация не создают ни Instance-каталога, ни любого файла', () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'wcq-empty-project-'));
  try {
    const before = fs.readdirSync(tmp);
    assert(before.length === 0, 'временный каталог не пуст перед сканированием');

    const plan = [{ id: 'root-agents-md', medium: 'file', path: 'AGENTS.md', container_ref: 'sample-project-repository' }];
    const results = scanDiscoveryPlan(plan, { containerRoot: tmp });
    assert(results.length === 1 && results[0].status === 'missing', JSON.stringify(results));

    const afterScan = fs.readdirSync(tmp);
    assert(afterScan.length === 0, 'scanDiscoveryPlan создал файл или каталог в пустом проекте');

    const connection = {
      ...baseConnectionEnvelope('scenario-empty-project-real'),
      payload: {
        connection_mode: 'compatibility',
        repository: { id: 'sample-project-repository', workspace_id: 'sample-project' },
        scan_kind: 'initial',
        discovery_plan: plan,
        discovered_sources: [],
        missing_sources: [{ plan_id: 'root-agents-md', previously_known: false }],
        unreadable_sources: [],
        rule_candidates: [],
        findings: [],
        next_step: 'continue-compatibility-mode',
      },
    };
    const { problems } = qualifyConnection(connection, {
      state: 'QUALIFIED',
      reason: 'пустой проект: единственный объявленный слот плана не находит ничего, ни один Instance-каталог или файл не создаётся',
    });
    assert(problems.length === 0, JSON.stringify(problems));

    const afterQualification = fs.readdirSync(tmp);
    assert(afterQualification.length === 0, 'композиция квалификации создала файл или каталог во временном пустом проекте');
    assert(!fs.existsSync(path.join(tmp, '.meridian')), 'композиция квалификации создала каталог Экземпляра .meridian');
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
});

/* --- scenario:existing-project-zero-write --- */
check('scenario:existing-project-zero-write — реальное сканирование AGENTS.md/CLAUDE.md/Cursor-правила не меняет ни единого байта дерева', () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'wcq-zero-write-'));
  try {
    fs.writeFileSync(path.join(tmp, 'AGENTS.md'), 'Follow feature/<slug> branches.\n');
    fs.writeFileSync(path.join(tmp, 'CLAUDE.md'), 'Claude-specific project notes.\n');
    fs.mkdirSync(path.join(tmp, '.cursor', 'rules'), { recursive: true });
    fs.writeFileSync(path.join(tmp, '.cursor', 'rules', 'sample.mdc'), 'rule body\n');

    const plan = [
      { id: 'root-agents-md', medium: 'file', path: 'AGENTS.md', container_ref: 'sample-project-repository' },
      { id: 'root-claude-md', medium: 'file', path: 'CLAUDE.md', container_ref: 'sample-project-repository' },
      { id: 'root-cursor-rule', medium: 'file', path: '.cursor/rules/sample.mdc', container_ref: 'sample-project-repository' },
    ];

    function snapshotTree(dir) {
      const out = {};
      for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const p = path.join(dir, entry.name);
        if (entry.isDirectory()) Object.assign(out, snapshotTree(p));
        else out[p] = fs.readFileSync(p);
      }
      return out;
    }
    function sameTree(a, b) {
      const ak = Object.keys(a).sort();
      const bk = Object.keys(b).sort();
      if (JSON.stringify(ak) !== JSON.stringify(bk)) return false;
      return ak.every((k) => Buffer.compare(a[k], b[k]) === 0);
    }

    const before = snapshotTree(tmp);
    const initial = scanDiscoveryPlan(plan, { containerRoot: tmp });
    const afterInitial = snapshotTree(tmp);
    assert(sameTree(before, afterInitial), 'первоначальное сканирование изменило рабочее дерево');

    const byId = Object.fromEntries(initial.map((r) => [r.planId, r]));
    assert(byId['root-agents-md'].status === 'discovered');
    assert(byId['root-claude-md'].status === 'discovered');
    assert(byId['root-cursor-rule'].status === 'discovered');

    const discovered = [
      discoveredSource({ id: 'root-agents-md', revision: byId['root-agents-md'].revision, digest: byId['root-agents-md'].digest, format: 'agents-md', filePath: 'AGENTS.md' }),
      discoveredSource({ id: 'root-claude-md', revision: byId['root-claude-md'].revision, digest: byId['root-claude-md'].digest, format: 'claude-md', filePath: 'CLAUDE.md' }),
      discoveredSource({ id: 'root-cursor-rule', revision: byId['root-cursor-rule'].revision, digest: byId['root-cursor-rule'].digest, format: 'cursor-rule', filePath: '.cursor/rules/sample.mdc' }),
    ];
    const connection = {
      ...baseConnectionEnvelope('scenario-zero-write'),
      payload: {
        connection_mode: 'compatibility',
        repository: { id: 'sample-project-repository', workspace_id: 'sample-project' },
        scan_kind: 'initial',
        discovery_plan: plan,
        discovered_sources: discovered,
        missing_sources: [],
        unreadable_sources: [],
        rule_candidates: [],
        findings: [],
        next_step: 'continue-compatibility-mode',
      },
    };

    const connectionProblems = evaluateExistingProjectCompatibilityMode(
      { schema_version: 1, registry_id: 'existing-project-compatibility-mode', title: 't', workspace_connections: [connection] },
      opts.compatOptions,
    );
    assert(connectionProblems.length === 0, JSON.stringify(connectionProblems));

    const { problems } = qualifyConnection(connection, {
      state: 'QUALIFIED',
      reason: 'три реальных источника обнаружены без единой записи в проект; совместимость квалифицирована',
    });
    assert(problems.length === 0, JSON.stringify(problems));

    const rescan = scanDiscoveryPlan(plan, { containerRoot: tmp });
    const afterRescan = snapshotTree(tmp);
    assert(sameTree(before, afterRescan), 'повторное сканирование изменило рабочее дерево');
    assert(sameTree(afterInitial, afterRescan), 'дерево разошлось между первоначальным и повторным сканированием');
    const rescanById = Object.fromEntries(rescan.map((r) => [r.planId, r]));
    assert(rescanById['root-agents-md'].digest === byId['root-agents-md'].digest, 'дайджест неизменного файла разошёлся между сканированиями');
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
});

/* --- scenario:duplicate-rule-provenance --- */
{
  const src = discoveredSource({ id: 'dup-source', revision: 'content-dup-1', digest: 'a1a1a1a1'.repeat(8) });
  const plan = [{ id: 'dup-source', medium: 'file', path: 'AGENTS.md', container_ref: 'sample-project-repository' }];
  const candA = ruleCandidate({
    id: 'dup-candidate-a', sourceId: 'dup-source', revision: 'content-dup-1', digest: 'a1a1a1a1'.repeat(8),
    boundaryStart: 1, boundaryEnd: 5, semanticKey: 'shared-rule-text', classificationBasis: 'явно классифицировано в область проекта, а не по пути',
    applicabilityState: 'candidate',
  });
  const candB = ruleCandidate({
    id: 'dup-candidate-b', sourceId: 'dup-source', revision: 'content-dup-1', digest: 'a1a1a1a1'.repeat(8),
    boundaryStart: 20, boundaryEnd: 25, semanticKey: 'shared-rule-text', classificationBasis: 'то же явное обоснование, второе происхождение',
    applicabilityState: 'candidate',
  });

  function connectionWith(candidates) {
    return {
      ...baseConnectionEnvelope('scenario-duplicate-provenance'),
      payload: {
        connection_mode: 'compatibility',
        repository: { id: 'sample-project-repository', workspace_id: 'sample-project' },
        scan_kind: 'initial',
        discovery_plan: plan,
        discovered_sources: [src],
        missing_sources: [],
        unreadable_sources: [],
        rule_candidates: candidates.map((c) => ({ discovery_status: 'new', candidate: c })),
        findings: [],
        next_step: 'continue-compatibility-mode',
      },
    };
  }

  check('scenario:duplicate-rule-provenance — два разных происхождения одного semantic_key образуют один кластер, оба остаются candidate — квалификация UNVERIFIED с открытым вопросом', () => {
    const connection = connectionWith([candA, candB]);
    const { problems } = qualifyConnection(connection, {
      state: 'UNVERIFIED',
      reason: 'один и тот же кандидат правила найден дважды через разные происхождения; кластер согласован, но решение владельца ещё не принято ни для одного происхождения',
      openQuestions: ['владелец ещё не решил кластер "shared-rule-text" (dup-candidate-a, dup-candidate-b)'],
    });
    assert(problems.length === 0, JSON.stringify(problems));
    assert(candA.payload.boundary.start !== candB.payload.boundary.end, 'происхождения обязаны различаться по границе');
  });

  check('scenario:duplicate-rule-provenance (негативный) — заявленный QUALIFIED поверх нерешённого дублирующего кластера отклонён', () => {
    const connection = connectionWith([candA, candB]);
    const { problems } = qualifyConnection(connection, {
      state: 'QUALIFIED',
      reason: 'намеренно неверное состояние — проверка обязана отклонить',
    });
    assert(problems.length > 0, 'QUALIFIED поверх двух нерешённых кандидатов принято без замечаний');
  });

  check('scenario:duplicate-rule-provenance (негативный) — кластер с расходящейся классификацией отклонён', () => {
    const candBWrongScope = clone(candB);
    candBWrongScope.scope = { type: 'repository-scope', id: 'sample-project-repository', workspace_id: 'sample-project' };
    const connection = connectionWith([candA, candBWrongScope]);
    const { problems } = qualifyConnection(connection, {
      state: 'UNVERIFIED',
      reason: 'намеренно противоречивая фикстура — проверка обязана отклонить, а не заявленное состояние',
      openQuestions: ['заведомо некорректный открытый вопрос'],
    });
    assert(problems.length > 0, 'кластер с двумя разными scope одного semantic_key принят без замечаний');
  });
}

/* --- scenario:conflict-order-independent --- */
{
  const src = discoveredSource({ id: 'conflict-source', revision: 'content-conflict-1', digest: 'b2b2b2b2'.repeat(8) });
  const plan = [{ id: 'conflict-source', medium: 'file', path: 'AGENTS.md', container_ref: 'sample-project-repository' }];

  /* Resolved conflict: one accepted, one lost. Kept as its own, separate,
   * successful-owner-resolution test, per the explicit request to not
   * collapse it into the unresolved-conflict test below. */
  const resolvedA = ruleCandidate({
    id: 'conflict-resolved-a', sourceId: 'conflict-source', revision: 'content-conflict-1', digest: 'b2b2b2b2'.repeat(8),
    boundaryStart: 1, boundaryEnd: 5, semanticKey: 'resolved-rule-a', classificationBasis: 'явно классифицировано как правило проекта A',
    applicabilityState: 'accepted', conflictsWith: ['resolved-rule-b'], ownerDecisionRef: 'owner-decision:accept-resolved-rule-a',
  });
  const resolvedB = ruleCandidate({
    id: 'conflict-resolved-b', sourceId: 'conflict-source', revision: 'content-conflict-1', digest: 'b2b2b2b2'.repeat(8),
    boundaryStart: 10, boundaryEnd: 15, semanticKey: 'resolved-rule-b', classificationBasis: 'явно классифицировано как правило проекта B',
    applicabilityState: 'not-applicable', notApplicableReason: 'lost-conflicting-decision', conflictsWith: ['resolved-rule-a'],
    ownerDecisionRef: 'owner-decision:reject-resolved-rule-b',
  });
  function resolvedConnectionWith(order) {
    return {
      ...baseConnectionEnvelope('scenario-conflict-resolved'),
      payload: {
        connection_mode: 'compatibility',
        repository: { id: 'sample-project-repository', workspace_id: 'sample-project' },
        scan_kind: 'initial',
        discovery_plan: plan,
        discovered_sources: [src],
        missing_sources: [],
        unreadable_sources: [],
        rule_candidates: order.map((c) => ({ discovery_status: 'carried-over', candidate: c })),
        findings: [],
        next_step: 'continue-compatibility-mode',
      },
    };
  }

  check('scenario:conflict-order-independent — успешное owner resolution (один принят, другой проиграл) проходит в обоих порядках', () => {
    const forward = qualifyConnection(resolvedConnectionWith([resolvedA, resolvedB]), {
      state: 'QUALIFIED', reason: 'симметричный конфликт разрешён владельцем; ничего не блокирует квалификацию — прямой порядок',
    });
    const reversed = qualifyConnection(resolvedConnectionWith([resolvedB, resolvedA]), {
      state: 'QUALIFIED', reason: 'симметричный конфликт разрешён владельцем; ничего не блокирует квалификацию — обратный порядок',
    });
    assert(forward.problems.length === 0 && reversed.problems.length === 0, JSON.stringify({ forward: forward.problems, reversed: reversed.problems }));
    assert(JSON.stringify(normalizeProblems(forward.problems)) === JSON.stringify(normalizeProblems(reversed.problems)), 'порядок объявления кандидатов повлиял на результат проверки');
  });

  /* Unresolved conflict: BOTH candidates remain "candidate" (undecided), with
   * a symmetric conflicts_with already declared between them, AND a blocking
   * "conflict" finding representing that the scan itself recognises the
   * standing conflict. next_step must compute resolve-conflict, and
   * qualification_state must be BLOCKED, in EITHER declaration order. */
  const unresolvedA = ruleCandidate({
    id: 'conflict-unresolved-a', sourceId: 'conflict-source', revision: 'content-conflict-1', digest: 'b2b2b2b2'.repeat(8),
    boundaryStart: 30, boundaryEnd: 35, semanticKey: 'unresolved-rule-a', classificationBasis: 'явно классифицировано, конфликт ещё не разрешён',
    applicabilityState: 'candidate', conflictsWith: ['unresolved-rule-b'],
  });
  const unresolvedB = ruleCandidate({
    id: 'conflict-unresolved-b', sourceId: 'conflict-source', revision: 'content-conflict-1', digest: 'b2b2b2b2'.repeat(8),
    boundaryStart: 40, boundaryEnd: 45, semanticKey: 'unresolved-rule-b', classificationBasis: 'явно классифицировано, конфликт ещё не разрешён',
    applicabilityState: 'candidate', conflictsWith: ['unresolved-rule-a'],
  });
  function unresolvedConnectionWith(order) {
    return {
      ...baseConnectionEnvelope('scenario-conflict-unresolved'),
      payload: {
        connection_mode: 'compatibility',
        repository: { id: 'sample-project-repository', workspace_id: 'sample-project' },
        scan_kind: 'initial',
        discovery_plan: plan,
        discovered_sources: [src],
        missing_sources: [],
        unreadable_sources: [],
        rule_candidates: order.map((c) => ({ discovery_status: 'new', candidate: c })),
        findings: [{ id: 'finding-conflict-unresolved-1', kind: 'conflict', detail: 'два нерешённых кандидата симметрично конфликтуют', blocking: true }],
        next_step: 'resolve-conflict',
      },
    };
  }

  check('scenario:conflict-order-independent — два НЕРЕШЁННЫХ конфликтующих кандидата дают BLOCKED/resolve-conflict в обоих порядках', () => {
    const forward = qualifyConnection(unresolvedConnectionWith([unresolvedA, unresolvedB]), {
      state: 'BLOCKED', reason: 'два нерешённых кандидата симметрично конфликтуют — прямой порядок', blockers: ['unresolved rule-candidate conflict (finding-conflict-unresolved-1)'],
    });
    const reversed = qualifyConnection(unresolvedConnectionWith([unresolvedB, unresolvedA]), {
      state: 'BLOCKED', reason: 'два нерешённых кандидата симметрично конфликтуют — обратный порядок', blockers: ['unresolved rule-candidate conflict (finding-conflict-unresolved-1)'],
    });
    assert(forward.problems.length === 0 && reversed.problems.length === 0, JSON.stringify({ forward: forward.problems, reversed: reversed.problems }));
    assert(forward.entry.payload.qualification_state === 'BLOCKED' && reversed.entry.payload.qualification_state === 'BLOCKED');
    assert(JSON.stringify(normalizeProblems(forward.problems)) === JSON.stringify(normalizeProblems(reversed.problems)), 'порядок объявления нерешённых конфликтующих кандидатов повлиял на результат проверки');
  });

  check('scenario:conflict-order-independent (негативный) — заявленный вердикт, расходящийся с вычисленным, отклонён одинаково в обоих порядках', () => {
    const forward = qualifyConnection(unresolvedConnectionWith([unresolvedA, unresolvedB]), { state: 'QUALIFIED', reason: 'намеренно неверное состояние' });
    const reversed = qualifyConnection(unresolvedConnectionWith([unresolvedB, unresolvedA]), { state: 'QUALIFIED', reason: 'намеренно неверное состояние' });
    assert(forward.problems.length > 0 && reversed.problems.length > 0, 'QUALIFIED поверх нерешённого конфликта принято без замечаний в одном из порядков');
    assert(JSON.stringify(normalizeProblems(forward.problems)) === JSON.stringify(normalizeProblems(reversed.problems)), 'расхождение отклонения между порядками объявления');
  });

  check('scenario:conflict-order-independent (негативный) — оба конфликтующих кластера accepted одновременно отклонены независимо от порядка', () => {
    const bothAccepted = clone(resolvedB);
    bothAccepted.payload.applicability_state = 'accepted';
    delete bothAccepted.payload.not_applicable_reason;
    bothAccepted.payload.owner_decision.decision_ref = 'owner-decision:accept-resolved-rule-b-too';
    bothAccepted.authority = { kind: 'project-owner', authority_ref: 'sample-project-owner', decision_ref: 'owner-decision:accept-resolved-rule-b-too' };

    const forward = qualifyConnection(resolvedConnectionWith([resolvedA, bothAccepted]), { state: 'QUALIFIED', reason: 'намеренно противоречивая фикстура' });
    const reversed = qualifyConnection(resolvedConnectionWith([bothAccepted, resolvedA]), { state: 'QUALIFIED', reason: 'намеренно противоречивая фикстура' });
    assert(forward.problems.length > 0, 'два одновременно принятых конфликтующих кластера прошли проверку чисто (прямой порядок)');
    assert(reversed.problems.length > 0, 'два одновременно принятых конфликтующих кластера прошли проверку чисто (обратный порядок)');
  });
}

/* --- scenario:source-change-no-silent-replacement --- */
{
  const OLD_REVISION = 'content-a1';
  const OLD_DIGEST = 'aaaa1111'.repeat(8);
  const NEW_REVISION = 'content-a2';
  const NEW_DIGEST = 'c3c3c3c3'.repeat(8);

  function changedSource() {
    return discoveredSource({
      id: 'changing-source', revision: NEW_REVISION, digest: NEW_DIGEST,
      divergence: {
        status: 'changed',
        previous_state: { revision: OLD_REVISION, digest: { algorithm: 'sha-256', value: OLD_DIGEST }, verified: true },
        current_state: { revision: NEW_REVISION, digest: { algorithm: 'sha-256', value: NEW_DIGEST }, verified: true },
      },
    });
  }
  function newContentCandidate(applicabilityState) {
    return ruleCandidate({
      id: 'source-change-new-candidate', sourceId: 'changing-source', revision: NEW_REVISION, digest: NEW_DIGEST,
      boundaryStart: 1, boundaryEnd: 5, semanticKey: 'source-change-rule', classificationBasis: 'разобрано из новой редакции источника после изменения',
      applicabilityState,
      ownerDecisionRef: applicabilityState === 'accepted' ? 'owner-decision:auto-accept-new-revision' : undefined,
    });
  }
  /* The rule-candidate representing the PREVIOUSLY accepted norm, still
   * pinned to the OLD revision — "прежнее owner decision" the scenario must
   * show is never silently carried over onto the new content. */
  function staleCarriedOverCandidate() {
    return ruleCandidate({
      id: 'source-change-old-candidate', sourceId: 'changing-source', revision: OLD_REVISION, digest: OLD_DIGEST,
      boundaryStart: 1, boundaryEnd: 5, semanticKey: 'source-change-rule', classificationBasis: 'ранее принятая норма прежней редакции',
      applicabilityState: 'accepted', ownerDecisionRef: 'owner-decision:accept-old-revision',
    });
  }
  function connectionWith({ withFinding, ruleCandidateWrappers }) {
    return {
      ...baseConnectionEnvelope('scenario-source-change'),
      payload: {
        connection_mode: 'compatibility',
        repository: { id: 'sample-project-repository', workspace_id: 'sample-project' },
        scan_kind: 'rescan',
        discovery_plan: [{ id: 'changing-source', medium: 'file', path: 'AGENTS.md', container_ref: 'sample-project-repository' }],
        discovered_sources: [changedSource()],
        missing_sources: [],
        unreadable_sources: [],
        rule_candidates: ruleCandidateWrappers,
        findings: withFinding ? [{ id: 'finding-source-changed-1', kind: 'source-changed', plan_id: 'changing-source', detail: 'редакция источника изменилась между сканированиями', blocking: false }] : [],
        next_step: 'continue-compatibility-mode',
      },
    };
  }

  check('scenario:source-change-no-silent-replacement — изменённая редакция даёт source-changed, новый кандидат остаётся candidate, итог UNVERIFIED до решения владельца', () => {
    const connection = connectionWith({ withFinding: true, ruleCandidateWrappers: [{ discovery_status: 'new', candidate: newContentCandidate('candidate') }] });
    const { problems } = qualifyConnection(connection, {
      state: 'UNVERIFIED',
      reason: 'источник изменился между сканированиями (отражено находкой source-changed); новый кандидат из изменённой редакции ещё не решён владельцем',
      openQuestions: ['владелец ещё не принял решение по кандидату из новой редакции "source-change-new-candidate"'],
    });
    assert(problems.length === 0, JSON.stringify(problems));
  });

  check('scenario:source-change-no-silent-replacement (негативный) — изменённый источник без находки source-changed отклонён', () => {
    const connection = connectionWith({ withFinding: false, ruleCandidateWrappers: [{ discovery_status: 'new', candidate: newContentCandidate('candidate') }] });
    const { problems } = qualifyConnection(connection, {
      state: 'UNVERIFIED',
      reason: 'намеренно недостающая находка — проверка обязана отклонить',
      openQuestions: ['заведомо некорректный открытый вопрос'],
    });
    assert(problems.length > 0, 'изменённый источник без обязательной находки source-changed принят без замечаний');
  });

  check('scenario:source-change-no-silent-replacement (негативный) — автоматическое принятие новой редакции (discovery_status "new" + accepted) отклонено', () => {
    const connection = connectionWith({ withFinding: true, ruleCandidateWrappers: [{ discovery_status: 'new', candidate: newContentCandidate('accepted') }] });
    const { problems } = qualifyConnection(connection, {
      state: 'QUALIFIED',
      reason: 'намеренная автоматическая приёмка новой редакции — проверка обязана отклонить',
    });
    assert(problems.length > 0, 'кандидат из новой редакции с discovery_status "new" принят как решённый (accepted) без замечаний');
  });

  check('scenario:source-change-no-silent-replacement (негативный) — прежнее owner decision, привязанное к устаревшей редакции, не переносится автоматически', () => {
    const connection = connectionWith({
      withFinding: true,
      ruleCandidateWrappers: [
        { discovery_status: 'new', candidate: newContentCandidate('candidate') },
        { discovery_status: 'carried-over', candidate: staleCarriedOverCandidate() },
      ],
    });
    const { problems } = qualifyConnection(connection, {
      state: 'UNVERIFIED',
      reason: 'старое решение, привязанное к прежней редакции источника, включено как будто оно всё ещё действует — проверка обязана отклонить',
      openQuestions: ['заведомо некорректный открытый вопрос'],
    });
    assert(problems.length > 0, 'кандидат, пиновавший устаревшую (сменившуюся) редакцию источника, принят как всё ещё действующее решение без замечаний');
  });
}

/* --- scenario:agent-native-partial-visibility --- */
{
  function connectionWith(readChannel) {
    const src = discoveredSource({
      id: 'agent-native-source', revision: 'content-agent-1', digest: 'd4d4d4d4'.repeat(8),
      readChannel,
    });
    return {
      ...baseConnectionEnvelope('scenario-agent-native'),
      payload: {
        connection_mode: 'compatibility',
        repository: { id: 'sample-project-repository', workspace_id: 'sample-project' },
        scan_kind: 'initial',
        discovery_plan: [{ id: 'agent-native-source', medium: 'file', path: 'AGENTS.md', container_ref: 'sample-project-repository' }],
        discovered_sources: [src],
        missing_sources: [],
        unreadable_sources: [],
        rule_candidates: [],
        findings: [],
        next_step: 'continue-compatibility-mode',
      },
    };
  }

  check('scenario:agent-native-partial-visibility — agent-native источник с partial-видимостью проходит', () => {
    const { problems } = qualifyConnection(
      connectionWith({ kind: 'agent-native', meridian_visibility: 'partial', agent_auto_read: true }),
      { state: 'QUALIFIED', reason: 'источник, читаемый самим агентом, остаётся отдельным каналом с частичной видимостью Meridian' },
    );
    assert(problems.length === 0, JSON.stringify(problems));
  });

  check('scenario:agent-native-partial-visibility (негативный) — agent-native с заявленной full-видимостью отклонён', () => {
    const { problems } = qualifyConnection(
      connectionWith({ kind: 'agent-native', meridian_visibility: 'full', agent_auto_read: true }),
      { state: 'QUALIFIED', reason: 'намеренно недопустимая комбинация — проверка обязана отклонить' },
    );
    assert(problems.length > 0, 'agent-native источник с meridian_visibility "full" принят без замечаний');
  });
}

/* --- scenario:multi-repository-workspace ---
 * Rewritten to prove the actual property this program requires: ONE
 * qualification composes AT LEAST TWO real repository connection scans of
 * the SAME project-workspace through payload.workspace_connection_refs —
 * never two independent qualification records standing in for "multi-repo".
 * Both repositories are REAL, independently resolved and independently
 * composed against evaluateExistingProjectCompatibilityMode; the aggregate
 * verdict is proven to read the WORST signal across every declared
 * connection, and to do so regardless of declared order. */
{
  const planOne = fixtures.plan_record_resolution['plan:sample-migration-plan-one'];
  const exportOne = fixtures.export_record_resolution['export:sample-canonical-export-one'];

  function repoConnection(id, repositoryId, payloadOverrides = {}) {
    return {
      $schema: '../scoped-record.schema.json',
      schema_version: 1,
      id,
      title: `Сканирование — ${id}`,
      record_type: 'workspace-connection-scan',
      scope: PROJECT_SCOPE,
      origin: { kind: 'declared', source_ref: `owner-decision:connect-${id}` },
      authority: { kind: 'delegated-run', authority_ref: `existing-project-compatibility-mode-run:${id}` },
      payload: {
        connection_mode: 'compatibility',
        repository: { id: repositoryId, workspace_id: PROJECT_SCOPE.id },
        scan_kind: 'initial',
        discovery_plan: [],
        discovered_sources: [],
        missing_sources: [],
        unreadable_sources: [],
        rule_candidates: [],
        findings: [],
        next_step: 'continue-compatibility-mode',
        ...payloadOverrides,
      },
    };
  }
  const repoA = repoConnection('scenario-multi-repo-a', 'sample-project-repository');
  const repoB = repoConnection('scenario-multi-repo-b', 'sample-project-repository-two');
  const migrationExpected = {
    migrationPlanRef: { record_type: 'instance-migration-plan', id: planOne.id, reference: `ad-hoc-plan:${planOne.id}`, sha256: computePlanFingerprint(planOne.payload) },
    canonicalExportRef: { record_type: 'instance-canonical-export', id: exportOne.id, reference: `ad-hoc-export:${exportOne.id}`, sha256: computeExportDigest(exportOne.payload) },
    extraPlanMap: { [`ad-hoc-plan:${planOne.id}`]: planOne },
    extraExportMap: { [`ad-hoc-export:${exportOne.id}`]: exportOne },
  };

  check('scenario:multi-repository-workspace — одна квалификация компонует ДВА реальных repository-скана одного workspace, подтверждённых миграцией, в любом порядке объявления', () => {
    const forward = qualifyConnection([repoA, repoB], {
      state: 'QUALIFIED',
      reason: 'два независимых репозитория одного workspace просканированы чисто, план миграции проверен и подтверждён каноническим экспортом',
      ...migrationExpected,
    });
    assert(forward.problems.length === 0, JSON.stringify(forward.problems));

    const reversed = qualifyConnection([repoB, repoA], {
      state: 'QUALIFIED',
      reason: 'два независимых репозитория одного workspace просканированы чисто, план миграции проверен и подтверждён каноническим экспортом',
      ...migrationExpected,
    });
    assert(reversed.problems.length === 0, 'перестановка workspace_connection_refs изменила результат: ' + JSON.stringify(reversed.problems));
  });

  check('scenario:multi-repository-workspace (негативный) — блокирующий конфликт в ОДНОМ из двух сканов блокирует всю квалификацию, независимо от порядка', () => {
    const dirtyB = repoConnection('scenario-multi-repo-b-conflict', 'sample-project-repository-two', {
      findings: [{ id: 'finding-multi-repo-conflict', kind: 'conflict', detail: 'конфликтующие кандидаты правил в репозитории B', blocking: true }],
      next_step: 'resolve-conflict',
    });
    const expectedBlocked = {
      state: 'BLOCKED',
      reason: 'сканирование repository B сообщает блокирующий конфликт; квалификация невозможна до его разрешения владельцем, независимо от чистого состояния repository A',
      blockers: ['unresolved rule-candidate conflict (finding-multi-repo-conflict)'],
    };
    const forward = qualifyConnection([repoA, dirtyB], expectedBlocked);
    assert(forward.problems.length === 0, JSON.stringify(forward.problems));

    const reversed = qualifyConnection([dirtyB, repoA], expectedBlocked);
    assert(reversed.problems.length === 0, JSON.stringify(reversed.problems));

    const wronglyDeclaredQualified = qualifyConnection([repoA, dirtyB], { state: 'QUALIFIED', reason: 'намеренно неверно — проверка обязана отклонить' });
    assert(wronglyDeclaredQualified.problems.length > 0, 'заявленный QUALIFIED поверх конфликта в одном из двух сканов принят без замечаний');
  });

  check('scenario:multi-repository-workspace (негативный) — повтор payload.repository.id между двумя сканами отклонён', () => {
    const duplicateRepoB = repoConnection('scenario-multi-repo-b-duplicate-repo', 'sample-project-repository');
    const result = qualifyConnection([repoA, duplicateRepoB], {
      state: 'QUALIFIED',
      reason: 'намеренно повторяющийся repository.id — проверка обязана отклонить',
      workspaceRepositoryIds: ['sample-project-repository'],
    });
    assert(result.problems.some((p) => p.includes('all naming the same payload.repository.id')), JSON.stringify(result.problems));
  });

  check('scenario:multi-repository-workspace (негативный) — workspace_repository_ids не совпадает с фактически просканированными репозиториями', () => {
    const result = qualifyConnection([repoA, repoB], {
      state: 'QUALIFIED',
      reason: 'намеренно неполный список — проверка обязана отклонить',
      workspaceRepositoryIds: ['sample-project-repository'],
    });
    assert(result.problems.some((p) => p.includes('workspace_repository_ids is missing')), JSON.stringify(result.problems));
  });

  check('scenario:multi-repository-workspace (негативный) — next_step "await-owner-decision" на ВТОРОМ скане даёт UNVERIFIED независимо от порядка', () => {
    const pendingB = repoConnection('scenario-multi-repo-b-pending', 'sample-project-repository-two', {
      findings: [{ id: 'finding-multi-repo-pending', kind: 'other', detail: 'обнаруженный кандидат в репозитории B ожидает решения владельца', blocking: true }],
      next_step: 'await-owner-decision',
    });
    const expectedUnverified = {
      state: 'UNVERIFIED',
      reason: 'сканирование repository B ожидает решения владельца; соответствие ещё не проверено, независимо от чистого состояния repository A',
      openQuestions: ['владелец ещё не принял решение по кандидату, приведшему к finding-multi-repo-pending'],
    };
    const forward = qualifyConnection([repoA, pendingB], expectedUnverified);
    assert(forward.problems.length === 0, JSON.stringify(forward.problems));

    const reversed = qualifyConnection([pendingB, repoA], expectedUnverified);
    assert(reversed.problems.length === 0, JSON.stringify(reversed.problems));

    const wronglyDeclaredQualified = qualifyConnection([repoA, pendingB], { state: 'QUALIFIED', reason: 'намеренно неверно — проверка обязана отклонить' });
    assert(wronglyDeclaredQualified.problems.length > 0, 'заявленный QUALIFIED поверх await-owner-decision в одном из двух сканов принят без замечаний');
  });

  check('scenario:multi-repository-workspace (негативный) — настоящий нерешённый кандидат правила на ВТОРОМ скане даёт UNVERIFIED независимо от порядка, даже когда его next_step чист', () => {
    const sourceB = discoveredSource({ id: 'scenario-multi-repo-b-source', revision: 'content-multi-repo-b-1', digest: 'f'.repeat(64), containerRef: 'sample-project-repository-two' });
    const candidateB = ruleCandidate({
      id: 'scenario-multi-repo-b-candidate', sourceId: 'scenario-multi-repo-b-source', revision: 'content-multi-repo-b-1', digest: 'f'.repeat(64),
      boundaryStart: 1, boundaryEnd: 3, semanticKey: 'multi-repo-b-rule', classificationBasis: 'явно классифицировано, решение владельца ещё не принято', applicabilityState: 'candidate',
    });
    const undecidedB = repoConnection('scenario-multi-repo-b-undecided', 'sample-project-repository-two', {
      discovery_plan: [{ id: 'scenario-multi-repo-b-source', medium: 'file', path: 'AGENTS.md', container_ref: 'sample-project-repository-two' }],
      discovered_sources: [sourceB],
      rule_candidates: [{ discovery_status: 'new', candidate: candidateB }],
    });
    const expectedUnverified = {
      state: 'UNVERIFIED',
      reason: 'разрешённый кандидат правила в repository B ещё не получил решения владельца; next_step сканирования не заявляет это блокирующим, но квалификация обязана остановиться сама',
      openQuestions: ['владелец ещё не принял решение по "scenario-multi-repo-b-candidate"'],
    };
    const forward = qualifyConnection([repoA, undecidedB], expectedUnverified);
    assert(forward.problems.length === 0, JSON.stringify(forward.problems));

    const reversed = qualifyConnection([undecidedB, repoA], expectedUnverified);
    assert(reversed.problems.length === 0, JSON.stringify(reversed.problems));

    const wronglyDeclaredQualified = qualifyConnection([repoA, undecidedB], { state: 'QUALIFIED', reason: 'намеренно неверно — проверка обязана отклонить' });
    assert(wronglyDeclaredQualified.problems.length > 0, 'заявленный QUALIFIED поверх нерешённого кандидата в одном из двух сканов принят без замечаний');
  });
}

/* --- scenario:migration-applicability-preservation --- */
{
  const idm = loadJson('registries/operating-model/fixtures/instance-data-migration.fixtures.json');
  const ice = loadJson('registries/operating-model/fixtures/instance-canonical-export.fixtures.json');
  const planOne = idm.valid[1].registry.migration_plans[0];
  const exportOne = ice.valid[1].registry.exports[0];

  /**
   * NOTE ON SCOPE: this scenario proves the KERNEL mechanism only —
   * qualification accepts applicability_preservation: verified only when
   * backed by evidence resolved through an external boundary, and correctly
   * turns an absent/invented confirmation into UNVERIFIED/rejection. It does
   * NOT independently re-derive or re-prove "applicable norms are preserved"
   * as a Kernel-level fact: that is a product-specific claim this
   * product-neutral Kernel cannot supply evidence for on its own — see
   * workspace-compatibility-qualification.md §4.3. The two helpers below
   * recompute the SAME migration-target-coverage relationship
   * evaluateInstanceCanonicalExport's own checkExportCompleteness already
   * checks (instance-data-migration.md §10.5) — restated here only so the
   * fixture's own shape is legible and so a corruption test has an
   * independent pre-condition to assert on, not as a second, competing
   * Kernel-level proof of anything about norms.
   */
  /** The migrated/merged target ids the plan itself declares — the coverage
   * relationship a canonical export is checked against. */
  function migrationTargetIds(plan) {
    return new Set(
      plan.payload.mappings
        .filter((m) => m.disposition === 'migrated' || m.disposition === 'merged')
        .map((m) => m.target.id),
    );
  }
  /** The record ids one canonical export actually produced. */
  function canonicalExportRecordIds(exportRecord) {
    return new Set(exportRecord.payload.records.map((r) => r.id));
  }
  function sameIdSet(a, b) {
    return a.size === b.size && [...a].every((id) => b.has(id));
  }

  check('scenario:migration-applicability-preservation — Kernel-механизм: applicability_preservation подтверждена реальным разрешённым evidence; покрытие целей миграции каноническим экспортом совпадает (фактическое сохранение норм конкретного продукта — предмет отдельного Instance-среза, §4.3)', () => {
    assert(planOne.payload.verification.coverage.status === 'verified');
    assert(planOne.payload.verification.applicability_preservation.status === 'verified');
    assert(planOne.payload.verification.overall_status === 'VERIFIED');

    const targetIds = migrationTargetIds(planOne);
    const recordIds = canonicalExportRecordIds(exportOne);
    assert(sameIdSet(targetIds, recordIds), `покрытие целей миграции каноническим экспортом расходится: цели плана=${[...targetIds]}, записи экспорта=${[...recordIds]}`);

    const migrationPlanRef = { record_type: 'instance-migration-plan', id: planOne.id, reference: `ad-hoc-plan:${planOne.id}`, sha256: computePlanFingerprint(planOne.payload) };
    const canonicalExportRef = { record_type: 'instance-canonical-export', id: exportOne.id, reference: `ad-hoc-export:${exportOne.id}`, sha256: computeExportDigest(exportOne.payload) };
    const connection = {
      ...baseConnectionEnvelope('scenario-applicability-preserved'),
      payload: {
        connection_mode: 'compatibility',
        repository: { id: 'sample-project-repository', workspace_id: 'sample-project' },
        scan_kind: 'initial',
        discovery_plan: [], discovered_sources: [], missing_sources: [], unreadable_sources: [], rule_candidates: [], findings: [],
        next_step: 'continue-compatibility-mode',
      },
    };
    connection.scope = planOne.scope;
    const { problems, entry } = qualifyConnection(connection, {
      state: 'QUALIFIED',
      reason: 'план подтверждён реальным разрешённым evidence по обеим осям (coverage, applicability_preservation), и составленный экспорт полностью покрывает минченные цели плана — Kernel-механизм признаёт эту версию квалифицируемой; фактическое сохранение конкретных норм продукта остаётся предметом отдельного Instance-среза',
      migrationPlanRef, canonicalExportRef,
      extraPlanMap: { [`ad-hoc-plan:${planOne.id}`]: planOne },
      extraExportMap: { [`ad-hoc-export:${exportOne.id}`]: exportOne },
    });
    assert(problems.length === 0, JSON.stringify(problems));
    assert(entry.payload.qualification_state === 'QUALIFIED');
  });

  function evaluateCorruptedExport(corruptedExport) {
    const migrationPlanRef = { record_type: 'instance-migration-plan', id: planOne.id, reference: `ad-hoc-plan:${planOne.id}`, sha256: computePlanFingerprint(planOne.payload) };
    const canonicalExportRef = { record_type: 'instance-canonical-export', id: corruptedExport.id, reference: `ad-hoc-export-corrupted:${corruptedExport.id}`, sha256: computeExportDigest(corruptedExport.payload) };
    const connection = {
      ...baseConnectionEnvelope('scenario-applicability-corrupted'),
      payload: {
        connection_mode: 'compatibility',
        repository: { id: 'sample-project-repository', workspace_id: 'sample-project' },
        scan_kind: 'initial',
        discovery_plan: [], discovered_sources: [], missing_sources: [], unreadable_sources: [], rule_candidates: [], findings: [],
        next_step: 'continue-compatibility-mode',
      },
    };
    connection.scope = planOne.scope;
    return qualifyConnection(connection, {
      state: 'QUALIFIED',
      reason: 'намеренно повреждённый экспорт — проверка обязана отклонить',
      migrationPlanRef, canonicalExportRef,
      extraPlanMap: { [`ad-hoc-plan:${planOne.id}`]: planOne },
      extraExportMap: { [`ad-hoc-export-corrupted:${corruptedExport.id}`]: corruptedExport },
    });
  }

  check('scenario:migration-applicability-preservation (негативный) — потеря записи, покрывающей минченную цель (пустой records), отклонена реальным evaluator\'ом', () => {
    const corrupted = clone(exportOne);
    corrupted.payload.records = [];
    corrupted.payload.digest = computeExportDigest(corrupted.payload);
    assert(!sameIdSet(migrationTargetIds(planOne), canonicalExportRecordIds(corrupted)), 'тестовая порча обязана нарушать покрытие целей миграции экспортом');
    const { problems } = evaluateCorruptedExport(corrupted);
    assert(problems.length > 0, 'экспорт, потерявший единственную запись, покрывающую минченную цель плана, принят без замечаний');
  });

  check('scenario:migration-applicability-preservation (негативный) — добавление записи, не заявленной планом, отклонено реальным evaluator\'ом', () => {
    const corrupted = clone(exportOne);
    const extra = clone(corrupted.payload.records[0]);
    extra.id = 'sample-migrated-record-extraneous';
    corrupted.payload.records.push(extra);
    corrupted.payload.digest = computeExportDigest(corrupted.payload);
    assert(!sameIdSet(migrationTargetIds(planOne), canonicalExportRecordIds(corrupted)), 'тестовая порча обязана нарушать покрытие целей миграции экспортом');
    const { problems } = evaluateCorruptedExport(corrupted);
    assert(problems.length > 0, 'экспорт с посторонней записью, не заявленной планом, принят без замечаний');
  });

  check('scenario:migration-applicability-preservation (негативный) — изменение id записи, покрывающей минченную цель, отклонено реальным evaluator\'ом', () => {
    const corrupted = clone(exportOne);
    corrupted.payload.records[0].id = 'sample-migrated-record-one-renamed';
    corrupted.payload.digest = computeExportDigest(corrupted.payload);
    assert(!sameIdSet(migrationTargetIds(planOne), canonicalExportRecordIds(corrupted)), 'тестовая порча обязана нарушать покрытие целей миграции экспортом');
    const { problems } = evaluateCorruptedExport(corrupted);
    assert(problems.length > 0, 'экспорт с изменённым id записи, покрывающей минченную цель, принят без замечаний');
  });

  check('scenario:migration-applicability-preservation (промежуточный случай) — coverage verified, applicability_preservation unverified → UNVERIFIED, не QUALIFIED', () => {
    const c = fixtures.valid.find((x) => x.note.includes('branch 7'));
    const plan = fixtures.plan_record_resolution[c.registry.qualifications[0].payload.migration_plan_ref.reference];
    assert(plan.payload.verification.coverage.status === 'verified', 'сценарий обязан опираться на подтверждённое покрытие');
    assert(plan.payload.verification.applicability_preservation.status === 'unverified', 'сценарий обязан опираться на неподтверждённое сохранение применимости');
    assert(plan.payload.verification.overall_status === 'UNVERIFIED');
    const problems = evaluate(c.registry);
    assert(problems.length === 0, JSON.stringify(problems));
    assert(c.registry.qualifications[0].payload.qualification_state === 'UNVERIFIED');
  });

  check('scenario:migration-applicability-preservation (негативный) — заявленный VERIFIED без подтверждающего доказательства отклонён', () => {
    const c = fixtures.valid.find((x) => x.note.includes('branch 7'));
    const planRefObj = c.registry.qualifications[0].payload.migration_plan_ref;
    const corruptedPlan = clone(fixtures.plan_record_resolution[planRefObj.reference]);
    corruptedPlan.payload.verification.applicability_preservation = { status: 'verified', evidence_ref: 'evidence:invented-applicability' };
    corruptedPlan.payload.verification.overall_status = 'VERIFIED';
    corruptedPlan.payload.plan_fingerprint = computePlanFingerprint(corruptedPlan.payload);

    const localOpts = buildOpts({
      connectionMap: fixtures.connection_record_resolution,
      planMap: { ...fixtures.plan_record_resolution, [planRefObj.reference]: corruptedPlan },
      exportMap: fixtures.export_record_resolution,
    });
    const d = clone(c.registry);
    d.qualifications[0].payload.migration_plan_ref = { ...planRefObj, sha256: computePlanFingerprint(corruptedPlan.payload) };
    d.qualifications[0].payload.qualification_state = 'QUALIFIED';
    d.qualifications[0].payload.open_questions = [];
    const problems = evaluateWorkspaceCompatibilityQualification(d, localOpts);
    assert(problems.length > 0, 'план, заявляющий VERIFIED с вымышленным evidence_ref применимости, принят без замечаний');
  });
}

console.log(`\n${passed} passed, ${failures.length} failed`);
if (failures.length) process.exit(1);
