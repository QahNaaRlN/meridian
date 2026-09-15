#!/usr/bin/env node
/**
 * Standalone verification for the upgrade integration qualification contract
 * (registries/operating-model/upgrade-integration-qualification.schema.json)
 * — package 9 of meridian-operating-upgrade.
 *
 * It calls the same implementation the gate calls
 * (scripts/lib/upgrade-integration-qualification.mjs) so the two cannot
 * drift: the record envelope against scoped-record.schema.json, the
 * qualification payload against upgrade-integration-qualification.schema.json,
 * payload.task_journey_ref resolved through an external boundary and
 * composed against the REAL evaluateEvidenceAndHandoff (the terminal
 * composition point for packages 2-7), payload.field_evaluation_report_ref
 * (package 8, nullable) composed against the REAL evaluateFieldEvaluation,
 * exactly three payload.scenario_classifications entries (one per required
 * neutral scenario) each composed against the REAL
 * evaluateTaskSpecification/evaluateExecutionState and checked against that
 * scenario's own closed expectation, and the recomputed
 * qualification_state/blockers/open_questions relation.
 *
 * Two things are proven here, kept visibly separate:
 *
 *   1. The DECISION MATRIX (upgrade-integration-qualification.md §5) —
 *      computeIntegrationQualificationState's own branches — covered by
 *      registries/operating-model/fixtures/upgrade-integration-qualification.fixtures.json's
 *      `valid`/`invalid` arrays, and unit-tested directly as a pure function
 *      below.
 *   2. The three REQUIRED NEUTRAL SCENARIOS (upgrade-integration-qualification.md
 *      §3) — single-module-refactor, multi-repository-decomposition and
 *      language-change-limit-case — each checked below against its own
 *      closed expectation (task_pattern id, and, for the two
 *      initiative-shaped scenarios, the resolved execution-run's
 *      lifecycle_stage relative to "classification").
 *
 * Usage: node test/upgrade-integration-qualification.test.mjs
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { assertSupportedDeep } from '../scripts/lib/json-schema.mjs';
import {
  evaluateUpgradeIntegrationQualification,
  computeIntegrationQualificationState,
  computeContentDigest,
  RECORD_TYPE, ALLOWED_SCOPE_TYPE, REQUIRED_ORIGIN_KIND, REQUIRED_AUTHORITY_KIND,
  QUALIFICATION_STATES, SCENARIO_IDS, EXPECTED_PATTERN_BY_SCENARIO, LIFECYCLE_STAGE_ORDER,
} from '../scripts/lib/upgrade-integration-qualification.mjs';
import { makeRefResolver } from '../scripts/lib/instance-data-migration.mjs';
import { makeRecordResolver } from '../scripts/lib/context-manifest.mjs';
import { yamlParse } from '../scripts/lib/yaml.mjs';

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

const registrySchema = loadJson('registries/operating-model/upgrade-integration-qualification.schema.json');
const envelopeSchema = loadJson('registries/operating-model/scoped-record.schema.json');
const ehSchema = loadJson('registries/operating-model/evidence-and-handoff.schema.json');
const feSchema = loadJson('registries/operating-model/field-evaluation.schema.json');
const tsSchema = loadJson('registries/operating-model/task-specification.schema.json');
const esSchema = loadJson('registries/operating-model/execution-state.schema.json');
const tprDoc = yamlParse(fs.readFileSync(path.join(root, 'standards/workspace/task-pattern-registry.yaml'), 'utf8'));
const taskPatterns = tprDoc.task_patterns.map((p) => ({ id: p.id, work_kind: p.payload.work_kind, change_class: p.payload.change_class ?? null }));
const fixtures = loadJson('registries/operating-model/fixtures/upgrade-integration-qualification.fixtures.json');

const opts = {
  registrySchema,
  envelopeSchema,
  resolveTaskJourney: makeRefResolver(fixtures.task_journey_resolution),
  resolveFieldEvaluationReport: makeRefResolver(fixtures.field_evaluation_resolution),
  resolveTaskSpecification: makeRefResolver(fixtures.task_specification_resolution),
  resolveExecutionState: makeRefResolver(fixtures.execution_state_resolution),
  taskJourneyOptions: { recordSchema: ehSchema, envelopeSchema, resolveRecords: makeRecordResolver(fixtures.task_journey_nested_resolution) },
  fieldEvaluationOptions: { recordSchema: feSchema, envelopeSchema, resolveRecords: makeRecordResolver(fixtures.field_evaluation_nested_resolution) },
  taskSpecificationOptions: { recordSchema: tsSchema, envelopeSchema, taskPatterns },
  executionStateOptions: { recordSchema: esSchema, envelopeSchema },
};
const evaluate = (doc) => evaluateUpgradeIntegrationQualification(doc, opts);

/* ===========================================================================
 * Schema and library constant sanity
 * ======================================================================== */

check('схема разбирается и не использует неподдерживаемых ключевых слов', () => {
  assertSupportedDeep(registrySchema, 'upgrade-integration-qualification.schema.json');
});

check('константы схемы совпадают с библиотекой', () => {
  assert(registrySchema.properties.registry_id.const === 'upgrade-integration-qualification');
  assert(registrySchema.definitions.entry.properties.record_type.const === RECORD_TYPE);
  assert(RECORD_TYPE === 'upgrade-integration-qualification');
  assert(ALLOWED_SCOPE_TYPE === 'project-workspace');
  assert(REQUIRED_ORIGIN_KIND === 'derived');
  assert(REQUIRED_AUTHORITY_KIND === 'delegated-run');
  assert(JSON.stringify(QUALIFICATION_STATES) === JSON.stringify(['QUALIFIED', 'BLOCKED', 'UNVERIFIED']));
  assert(SCENARIO_IDS.length === 3);
});

check('фикстуры несут непустые массивы valid и invalid, и все резолверы', () => {
  assert(Array.isArray(fixtures.valid) && fixtures.valid.length > 0);
  assert(Array.isArray(fixtures.invalid) && fixtures.invalid.length > 0);
  for (const key of ['task_journey_resolution', 'task_journey_nested_resolution', 'field_evaluation_resolution', 'field_evaluation_nested_resolution', 'task_specification_resolution', 'execution_state_resolution']) {
    assert(fixtures[key] && typeof fixtures[key] === 'object', `отсутствует "${key}"`);
  }
});

/* ===========================================================================
 * Decision-matrix fixtures (§5)
 * ======================================================================== */

for (const [i, c] of fixtures.valid.entries()) {
  check(`valid[${i}] проходит без замечаний: ${c.note}`, () => {
    const problems = evaluate(c.registry);
    assert(problems.length === 0, problems[0]);
  });
}

for (const [i, c] of fixtures.invalid.entries()) {
  check(`invalid[${i}] отклоняется: ${c.note}`, () => {
    const problems = evaluate(c.registry);
    assert(problems.length > 0, 'фикстура, которая обязана быть отклонена, прошла чисто');
  });
}

/* ===========================================================================
 * computeIntegrationQualificationState — pure decision matrix, unit-tested
 * directly against every branch (upgrade-integration-qualification.md §5).
 * ======================================================================== */

check('матрица решений: неразрешённый задачный путь -> BLOCKED', () => {
  assert(computeIntegrationQualificationState({ taskJourneyResolved: false }) === 'BLOCKED');
});
check('матрица решений: outcome.status blocked -> BLOCKED', () => {
  assert(computeIntegrationQualificationState({ taskJourneyResolved: true, outcomeStatus: 'blocked' }) === 'BLOCKED');
});
check('матрица решений: неполное покрытие сценариев -> BLOCKED', () => {
  assert(computeIntegrationQualificationState({
    taskJourneyResolved: true, outcomeStatus: 'complete', scenarioCoverageOk: false,
  }) === 'BLOCKED');
});
check('матрица решений: сценарий не разрешился/не соответствует -> BLOCKED', () => {
  assert(computeIntegrationQualificationState({
    taskJourneyResolved: true, outcomeStatus: 'complete', scenarioCoverageOk: true, allScenariosClean: false,
  }) === 'BLOCKED');
});
check('матрица решений: outcome.status handed_off_incomplete -> UNVERIFIED', () => {
  assert(computeIntegrationQualificationState({
    taskJourneyResolved: true, outcomeStatus: 'handed_off_incomplete', scenarioCoverageOk: true, allScenariosClean: true,
  }) === 'UNVERIFIED');
});
check('матрица решений: заявленный отчёт полевой оценки не разрешился/не составлен чисто -> BLOCKED', () => {
  assert(computeIntegrationQualificationState({
    taskJourneyResolved: true, outcomeStatus: 'complete', scenarioCoverageOk: true, allScenariosClean: true,
    fieldEvaluationRefPresent: true, fieldEvaluationClean: false,
  }) === 'BLOCKED');
});
check('матрица решений: отчёт полевой оценки не заявлен (null) -> UNVERIFIED', () => {
  assert(computeIntegrationQualificationState({
    taskJourneyResolved: true, outcomeStatus: 'complete', scenarioCoverageOk: true, allScenariosClean: true,
    fieldEvaluationRefPresent: false,
  }) === 'UNVERIFIED');
});
check('матрица решений: все сигналы чисты -> QUALIFIED', () => {
  assert(computeIntegrationQualificationState({
    taskJourneyResolved: true, outcomeStatus: 'complete', scenarioCoverageOk: true, allScenariosClean: true,
    fieldEvaluationRefPresent: true, fieldEvaluationClean: true,
  }) === 'QUALIFIED');
});

/* ===========================================================================
 * The three required neutral scenarios (§3) — each proven against the
 * qualified valid[0] fixture entry, which composes all three correctly.
 * ======================================================================== */

const qualifiedEntry = fixtures.valid.find((c) => c.registry.qualifications[0].payload.qualification_state === 'QUALIFIED');
assert(qualifiedEntry, 'ожидалась хотя бы одна QUALIFIED фикстура для проверки трёх сценариев');
const scenarioByBId = new Map(
  qualifiedEntry.registry.qualifications[0].payload.scenario_classifications.map((s) => [s.scenario_id, s]),
);

check('ровно три сценария объявлены — без пропуска и без дубля', () => {
  assert(SCENARIO_IDS.every((id) => scenarioByBId.has(id)));
  assert(scenarioByBId.size === SCENARIO_IDS.length);
});

for (const scenarioId of SCENARIO_IDS) {
  check(`сценарий "${scenarioId}" разрешает task-specification с ожидаемым task_pattern`, () => {
    const s = scenarioByBId.get(scenarioId);
    const resolved = fixtures.task_specification_resolution[s.task_specification_ref.reference];
    assert(resolved, 'постановка не найдена в task_specification_resolution');
    assert(resolved.payload.task_pattern.id === EXPECTED_PATTERN_BY_SCENARIO[scenarioId],
      `ожидался task_pattern "${EXPECTED_PATTERN_BY_SCENARIO[scenarioId]}", получен "${resolved.payload.task_pattern.id}"`);
  });
}

check('сценарий "language-change-limit-case" не проходит дальше classification (только классификация, без исполнения)', () => {
  const s = scenarioByBId.get('language-change-limit-case');
  const run = fixtures.execution_state_resolution[s.execution_state_ref.reference];
  const idx = LIFECYCLE_STAGE_ORDER.indexOf(run.payload.lifecycle_stage);
  assert(idx >= 0 && idx <= LIFECYCLE_STAGE_ORDER.indexOf('classification'),
    `lifecycle_stage "${run.payload.lifecycle_stage}" уже прошёл classification`);
});

check('сценарий "multi-repository-decomposition" проходит дальше classification (декомпозиция фактически продвигается)', () => {
  const s = scenarioByBId.get('multi-repository-decomposition');
  const run = fixtures.execution_state_resolution[s.execution_state_ref.reference];
  const idx = LIFECYCLE_STAGE_ORDER.indexOf(run.payload.lifecycle_stage);
  assert(idx > LIFECYCLE_STAGE_ORDER.indexOf('classification'),
    `lifecycle_stage "${run.payload.lifecycle_stage}" не прошёл дальше classification`);
});

/* ===========================================================================
 * §3.1 — exact specification/run linkage: every scenario's resolved
 * execution-run must name THIS scenario's own task_specification_ref, not a
 * borrowed run from a different specification.
 * ======================================================================== */

for (const scenarioId of SCENARIO_IDS) {
  check(`сценарий "${scenarioId}" — разрешённый запуск называет ту же самую постановку (§3.1)`, () => {
    const s = scenarioByBId.get(scenarioId);
    const run = fixtures.execution_state_resolution[s.execution_state_ref.reference];
    assert(run.payload.task_specification_ref === s.task_specification_ref.reference,
      `запуск называет постановку "${run.payload.task_specification_ref}", сценарий пиновал "${s.task_specification_ref.reference}"`);
  });
}

check('чужой запуск (принадлежащий другой постановке) отклоняется (§3.1)', () => {
  const bad = clone(qualifiedEntry.registry);
  const bp = bad.qualifications[0].payload;
  // Swap the multi-repository-decomposition scenario's run for
  // single-module-refactor's own run — a real, individually-valid
  // execution-run, but pinned to a DIFFERENT specification than
  // task_specification_ref still names.
  const refactorRun = bp.scenario_classifications.find((s) => s.scenario_id === 'single-module-refactor').execution_state_ref;
  const decompose = bp.scenario_classifications.find((s) => s.scenario_id === 'multi-repository-decomposition');
  decompose.execution_state_ref = refactorRun;
  const problems = evaluate(bad);
  assert(problems.some((p) => p.includes('belonging to a different specification')), 'подмена запуска чужой постановкой не была отклонена');
});

/* ===========================================================================
 * §6.1 — unified workspace identity: the task journey, the field-evaluation
 * report and every scenario's specification/run must belong to the same
 * workspace as the qualification itself.
 * ======================================================================== */

check('задачный путь, отчёт полевой оценки и все сценарии принадлежат одному рабочему пространству (§6.1)', () => {
  const p = qualifiedEntry.registry.qualifications[0].payload;
  const qualificationWs = qualifiedEntry.registry.qualifications[0].scope.id;
  const journey = fixtures.task_journey_resolution[p.task_journey_ref.reference];
  assert(journey.scope.workspace_id === qualificationWs, 'task_journey_ref принадлежит другому рабочему пространству');
  const field = fixtures.field_evaluation_resolution[p.field_evaluation_report_ref.reference];
  assert(field.payload.workspace_id === qualificationWs, 'field_evaluation_report_ref принадлежит другому рабочему пространству');
  for (const s of p.scenario_classifications) {
    const spec = fixtures.task_specification_resolution[s.task_specification_ref.reference];
    const specWs = spec.scope.type === 'project-workspace' ? spec.scope.id : spec.scope.workspace_id;
    assert(specWs === qualificationWs, `сценарий "${s.scenario_id}": постановка принадлежит другому рабочему пространству`);
    const run = fixtures.execution_state_resolution[s.execution_state_ref.reference];
    assert(run.scope.workspace_id === qualificationWs, `сценарий "${s.scenario_id}": запуск принадлежит другому рабочему пространству`);
  }
});

check('composed-запись из другого рабочего пространства отклоняется (§6.1)', () => {
  const bad = clone(qualifiedEntry.registry);
  bad.qualifications[0].scope.id = 'a-completely-different-workspace';
  const problems = evaluate(bad);
  assert(problems.some((p) => p.includes('must belong to the same workspace')), 'разошедшееся рабочее пространство не было отклонено');
});

/* ===========================================================================
 * Pinned-reference discipline: exact-version pinning, closed shape.
 * ======================================================================== */

check('computeContentDigest пересчитывается детерминированно независимо от порядка ключей', () => {
  const a = { id: 'x', title: 't', record_type: 'r', scope: { type: 'a', id: 'b' }, origin: { kind: 'derived' }, authority: { kind: 'delegated-run', authority_ref: 'x' }, payload: { a: 1, b: 2 } };
  const b = { payload: { b: 2, a: 1 }, authority: { authority_ref: 'x', kind: 'delegated-run' }, origin: { kind: 'derived' }, scope: { id: 'b', type: 'a' }, record_type: 'r', title: 't', id: 'x' };
  assert(computeContentDigest(a) === computeContentDigest(b), 'дайджест зависит от порядка ключей');
});

check('подмена содержимого под тем же id/reference отклоняется (испорченный sha256)', () => {
  const good = clone(qualifiedEntry.registry);
  good.qualifications[0].payload.task_journey_ref.sha256 = '0'.repeat(64);
  const problems = evaluate(good);
  assert(problems.some((p) => p.includes('sha256')), 'испорченный дайджест не был отклонён');
});

console.log(`\n${passed} passed, ${failures.length} failed`);
if (failures.length) process.exit(1);
