#!/usr/bin/env node
// Standalone verification for the meridian-field-evaluation contract
// (registries/operating-model/field-evaluation.schema.json).
//
// It calls the same implementation the gate calls
// (scripts/lib/field-evaluation.mjs) so the two cannot drift: the canonical
// scoped-record envelope (validated separately and always), the complete
// specialised schema a record declares in its own $schema (selecting the
// observation or report body by record_type), and the rules JSON Schema
// cannot state — the portable, logically-resolved $schema declaration; the
// eight characteristics modelled separately, each with a metric-fixed
// measurement kind and (for classification) a metric-closed outcome pool;
// the four distinct observation statuses, each but 'observed' carrying a
// reason and forbidding a measurement; the externally-resolved, pinned
// evidence required for 'observed', bound to THIS observation's own metric;
// the correction (supersedes) mechanism; and, for a report, the resolved
// comparability rules (same workspace, same period), the rejection of double
// counting (a repeated id, or a superseded+superseding pair both included),
// the eight-of-eight per_metric completeness, and the exact recomputation of
// every aggregate from the resolved, named sample.
//
// Usage: node test/field-evaluation.test.mjs

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { assertSupportedDeep, UnsupportedSchema } from '../scripts/lib/json-schema.mjs';
import {
  evaluateFieldEvaluation,
  buildFieldEvaluationReport,
  computeMetricAggregate,
  roundPercentage,
  isValidDate,
  isValidDateTime,
  makeRecordResolver,
  nonPortableReason,
  resolveSchemaRef,
  classifyRevision,
  OBSERVATION_RECORD_TYPE,
  REPORT_RECORD_TYPE,
  RECORD_TYPES,
  OBSERVATION_SCOPE_TYPE,
  REPORT_SCOPE_TYPE,
  CANONICAL_RECORD_BASE,
  EXPECTED_SCHEMA_BASENAME,
  EXPECTED_SCHEMA_REF,
  METRIC_IDS,
  METRIC_MEASUREMENT_KIND,
  CLASSIFICATION_OUTCOMES,
  CLASSIFICATION_TRACKED_OUTCOME,
  METRICS_REQUIRING_WINDOW,
  OBSERVATION_STATUSES,
  METRIC_REPORT_STATUSES,
  EVIDENCE_KINDS,
  OBSERVED_RESULTS,
  FORBIDDEN_PAYLOAD_FIELDS,
} from '../scripts/lib/field-evaluation.mjs';

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

const recordSchema = loadJson('registries/operating-model/field-evaluation.schema.json');
const envelopeSchema = loadJson('registries/operating-model/scoped-record.schema.json');
const fixtures = loadJson('registries/operating-model/fixtures/field-evaluation.fixtures.json');

// The record resolver is EXTERNAL to every record under test: it is built
// from the bundle's companion `resolution` set, never from fields inside the
// record.
const resolveRecords = makeRecordResolver(fixtures.resolution);
const opts = { recordSchema, envelopeSchema, resolveRecords };
const evaluate = (doc) => evaluateFieldEvaluation(doc, opts);
const rejectsBecause = (doc, re) => evaluate(doc).some((m) => re.test(m));
const evalNoResolver = (doc) => evaluateFieldEvaluation(doc, { recordSchema, envelopeSchema });

const findValid = (recordType, id) => clone(fixtures.valid.find(
  (c) => c.spec.record_type === recordType && c.spec.id === id,
).spec);

const OBS = (id) => findValid(OBSERVATION_RECORD_TYPE, id);
const REPORT = findValid(REPORT_RECORD_TYPE, 'report-2026-08');

const mutateObs = (id, fn) => { const d = OBS(id); fn(d); return d; };
const mutateReport = (fn) => { const d = clone(REPORT); fn(d); return d; };

// ---------------------------------------------------------------------------
// schema shape
// ---------------------------------------------------------------------------
check('both schemas use the supported JSON Schema subset', () => {
  assertSupportedDeep(recordSchema, 'field-evaluation.schema.json');
  assertSupportedDeep(envelopeSchema, 'scoped-record.schema.json');
});

check('an unsupported schema keyword is rejected up front (assertSupportedDeep)', () => {
  const bad = clone(recordSchema);
  bad.definitions.observation_payload.patternProperties = { '^x': { type: 'string' } };
  let threw = null;
  try { assertSupportedDeep(bad, 'field-evaluation.schema.json'); }
  catch (e) { threw = e; }
  assert(threw instanceof UnsupportedSchema, 'assertSupportedDeep did not reject the unsupported keyword');
});

check('the specialised schema selects the observation/report body by record_type', () => {
  assert(JSON.stringify(recordSchema.properties.record_type.enum) === JSON.stringify(RECORD_TYPES),
    'record_type enum drift');
  assert(recordSchema.additionalProperties === false, 'the record schema is not a closed object');
  assert(recordSchema.definitions.observation_payload.additionalProperties === false, 'observation_payload is not closed');
  assert(recordSchema.definitions.report_payload.additionalProperties === false, 'report_payload is not closed');
  assert(recordSchema.definitions.report_payload.properties.per_metric.minItems === 8
    && recordSchema.definitions.report_payload.properties.per_metric.maxItems === 8,
    'per_metric is not fixed to exactly eight entries');
  assert(JSON.stringify(recordSchema.definitions.metric_id.enum) === JSON.stringify(METRIC_IDS), 'metric_id enum drift');
  assert(JSON.stringify(recordSchema.definitions.evidence_entry.properties.kind.enum) === JSON.stringify(EVIDENCE_KINDS), 'evidence kind enum drift');
  assert(JSON.stringify(recordSchema.definitions.observation_payload.properties.status.enum) === JSON.stringify(OBSERVATION_STATUSES), 'observation status enum drift');
  assert(JSON.stringify(recordSchema.definitions.per_metric_entry.properties.status.enum) === JSON.stringify(METRIC_REPORT_STATUSES), 'per-metric status enum drift');
});

check('the enums and per-metric mappings line up with the library', () => {
  assert(METRIC_IDS.length === 8, 'there are not exactly eight characteristics');
  for (const mid of METRIC_IDS) assert(['classification', 'duration', 'count'].includes(METRIC_MEASUREMENT_KIND[mid]), `metric "${mid}" has no closed measurement kind`);
  for (const mid of Object.keys(CLASSIFICATION_OUTCOMES)) {
    assert(CLASSIFICATION_OUTCOMES[mid].includes('unknown'), `classification pool for "${mid}" has no "unknown" outcome`);
    assert(CLASSIFICATION_OUTCOMES[mid].includes(CLASSIFICATION_TRACKED_OUTCOME[mid]), `tracked outcome for "${mid}" is not in its own pool`);
  }
  assert(METRICS_REQUIRING_WINDOW.length === 1 && METRICS_REQUIRING_WINDOW[0] === 'post-acceptance-defects',
    'only post-acceptance-defects uses an observation window');
  assert(OBSERVATION_STATUSES.length === 4, 'observation status pool drift');
  assert(JSON.stringify(OBSERVED_RESULTS) === JSON.stringify(['confirmed', 'contradicted', 'inconclusive']), 'OBSERVED_RESULTS drift');
});

check('the implementation source carries no NUL or control byte', () => {
  const buf = fs.readFileSync(path.join(root, 'scripts/lib/field-evaluation.mjs'));
  assert(buf.indexOf(0) === -1, 'a NUL byte is present in scripts/lib/field-evaluation.mjs');
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
  assert(rejectsBecause(mutateObs('obs-rework-1', (x) => { delete x.$schema; }), /declares no \$schema/), 'absent $schema accepted');
  assert(rejectsBecause(mutateObs('obs-rework-1', (x) => { x.$schema = '../../registries/operating-model/scoped-record.schema.json'; }), /resolves to the record envelope/), 'envelope-only $schema accepted');
  assert(rejectsBecause(mutateObs('obs-rework-1', (x) => { x.$schema = EXPECTED_SCHEMA_BASENAME; }), /does not resolve to the logical address/), 'bare basename accepted');
  assert(rejectsBecause(mutateObs('obs-rework-1', (x) => { x.$schema = `../../../registries/operating-model/${EXPECTED_SCHEMA_BASENAME}`; }), /does not resolve to the logical address/), 'out-of-namespace $schema accepted');
  assert(rejectsBecause(mutateObs('obs-rework-1', (x) => { x.$schema = '/opt/meridian/registries/operating-model/field-evaluation.schema.json'; }), /is not portable/), 'absolute $schema path accepted');
  assert(CANONICAL_RECORD_BASE === 'records/field-evaluation', 'the canonical logical base changed unexpectedly');
});

// ---------------------------------------------------------------------------
// real fixtures — the bulk check
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
  assert(fixtures.invalid.length >= 15, 'fewer than 15 invalid fixtures are bundled');
});

check('no external resolver means the record fails closed', () => {
  assert(evalNoResolver(OBS('obs-mech-correct-1')).length > 0, 'an observation validated clean with no resolver');
  assert(evalNoResolver(REPORT).length > 0, 'a report validated clean with no resolver');
});

// ===========================================================================
// positive scenarios
// ===========================================================================
check('a full synthetic set of observations plus a report pass together', () => {
  assert(fixtures.valid.filter((c) => c.spec.record_type === OBSERVATION_RECORD_TYPE).length >= 15,
    'fewer than 15 valid observation fixtures are bundled');
  assert(fixtures.valid.some((c) => c.spec.record_type === REPORT_RECORD_TYPE), 'no valid report fixture is bundled');
});

check('partial coverage with explicit unknown values is accepted', () => {
  const unk = fixtures.valid.find((c) => c.spec.payload && c.spec.payload.status === 'unknown');
  assert(unk, 'no valid fixture demonstrates status "unknown"');
  assert(evaluate(unk.spec).length === 0, 'the unknown-status fixture was rejected');
});

check('a confirmed zero count is accepted and distinguished from "no data"', () => {
  const zero = OBS('obs-rework-1');
  assert(zero.payload.measurement.value === 0, 'fixture obs-rework-1 is not the confirmed-zero example');
  assert(evaluate(zero).length === 0, 'a confirmed zero count was rejected');
});

check('correct inapplicability (not_applicable) is accepted', () => {
  const na = fixtures.valid.find((c) => c.spec.payload && c.spec.payload.status === 'not_applicable');
  assert(na, 'no valid fixture demonstrates status "not_applicable"');
  assert(evaluate(na.spec).length === 0, 'the not_applicable fixture was rejected');
});

check('several runs are represented separately in the same metric sample', () => {
  const mechRun = (oid) => fixtures.resolution[`field-evaluation/observations/${oid}`];
  const a = mechRun('obs-mech-correct-1');
  const b = mechRun('obs-mech-incorrect-1');
  assert(a && b, 'expected runs are missing from the resolution bundle');
  const runA = fixtures.resolution[OBS('obs-mech-correct-1').payload.execution_run_ref.reference].id;
  const runB = fixtures.resolution[OBS('obs-mech-incorrect-1').payload.execution_run_ref.reference].id;
  assert(runA !== runB, 'the two contributing observations are not actually separate runs');
});

check('a correction preserves history: the original and the fix are both present', () => {
  const fix = fixtures.valid.find((c) => c.spec.id === 'obs-correction-fix');
  assert(fix, 'no correction fixture bundled');
  assert(fix.spec.payload.supersedes && fix.spec.payload.correction_reason, 'the correction fixture carries no supersedes/correction_reason');
  const original = fixtures.valid.find((c) => c.spec.id === fix.spec.payload.supersedes.id);
  assert(original, 'the superseded original observation is not itself a bundled record');
});

check('rebuilding the report from the same observations is idempotent', () => {
  const refs = REPORT.payload.included_observations;
  const first = buildFieldEvaluationReport({
    workspaceId: REPORT.payload.workspace_id, period: REPORT.payload.period,
    includedRefs: refs, excludedEntries: REPORT.payload.excluded_observations, resolveRecords,
  });
  const second = buildFieldEvaluationReport({
    workspaceId: REPORT.payload.workspace_id, period: REPORT.payload.period,
    includedRefs: refs, excludedEntries: REPORT.payload.excluded_observations, resolveRecords,
  });
  assert(JSON.stringify(first) === JSON.stringify(second), 'rebuilding from the same inputs did not reproduce the same report');
  assert(JSON.stringify(first.per_metric) === JSON.stringify(REPORT.payload.per_metric),
    'the builder output does not match the hand-assembled valid report fixture');
});

check('permuting the included references does not change the built report', () => {
  const refs = REPORT.payload.included_observations;
  const shuffled = [...refs].reverse();
  const a = buildFieldEvaluationReport({
    workspaceId: REPORT.payload.workspace_id, period: REPORT.payload.period,
    includedRefs: refs, excludedEntries: REPORT.payload.excluded_observations, resolveRecords,
  });
  const b = buildFieldEvaluationReport({
    workspaceId: REPORT.payload.workspace_id, period: REPORT.payload.period,
    includedRefs: shuffled, excludedEntries: REPORT.payload.excluded_observations, resolveRecords,
  });
  assert(JSON.stringify(a) === JSON.stringify(b), 'permuting included_observations changed the built report');
});

// ===========================================================================
// computeMetricAggregate / roundPercentage — unit-level positive checks
// ===========================================================================
check('roundPercentage rounds to two decimals and is null at a zero denominator', () => {
  assert(roundPercentage(1, 3) === 33.33, `expected 33.33, got ${roundPercentage(1, 3)}`);
  assert(roundPercentage(0, 0) === null, 'a zero denominator did not return null');
  assert(roundPercentage(5, 0) === null, 'a zero denominator with a non-zero numerator did not return null');
});

check('a classification aggregate over zero usable observations has total 0 and no percentage', () => {
  const agg = computeMetricAggregate('mechanism-correctness', []);
  assert(agg.total === 0, 'empty sample did not produce total 0');
  assert(agg.rate.denominator === 0 && agg.rate.numerator === 0, 'empty sample rate is not 0/0');
  assert(!('percentage' in agg.rate), 'a zero-denominator rate carries a percentage');
});

check('a duration aggregate keeps calendar and active figures separate', () => {
  const agg = computeMetricAggregate('context-entry-time', [
    { id: 'a', measurement: { duration_kind: 'calendar', seconds: 100 } },
    { id: 'b', measurement: { duration_kind: 'active', seconds: 40 } },
  ]);
  assert(agg.calendar.total_seconds === 100 && agg.calendar.sample_count === 1, 'calendar figure is wrong');
  assert(agg.active.total_seconds === 40 && agg.active.sample_count === 1, 'active figure is wrong');
});

// ===========================================================================
// negative scenarios — observation level. Each asserts the rejection names
// the SPECIFIC mechanism, not merely "some" unrelated structural error.
// ===========================================================================
check('CHANGES_REQUESTED — missing data is never silently replaced by zero (no evidence at all)', () => {
  assert(rejectsBecause(mutateObs('obs-rework-1', (d) => { d.payload.evidence = []; }),
    /carries no evidence/), 'an observed value with no evidence at all was accepted');
});

check('CHANGES_REQUESTED — a negative duration is rejected', () => {
  assert(rejectsBecause(mutateObs('obs-context-time-1', (d) => { d.payload.measurement.seconds = -1; }),
    /not a non-negative number/), 'a negative duration was accepted');
});

check('CHANGES_REQUESTED — the wrong duration unit is rejected', () => {
  assert(rejectsBecause(mutateObs('obs-context-time-1', (d) => { d.payload.measurement.unit = 'minutes'; }),
    /not "seconds"/), 'a wrong duration unit was accepted');
});

check('CHANGES_REQUESTED — an impossible time interval (end not after start) is rejected', () => {
  assert(rejectsBecause(mutateObs('obs-context-time-1', (d) => {
    d.payload.measurement.start_ts = '2026-08-03T12:00:00Z';
    d.payload.measurement.end_ts = '2026-08-03T10:00:00Z';
  }), /not a possible time interval/), 'an impossible interval was accepted');
});

check('CHANGES_REQUESTED — an unpinned evidence reference is rejected', () => {
  assert(rejectsBecause(mutateObs('obs-rework-2', (d) => { delete d.payload.evidence[0].sha256; }),
    /not pinned to an exact edition/), 'unpinned evidence was accepted');
});

check('CHANGES_REQUESTED — an unresolvable evidence reference is rejected', () => {
  assert(rejectsBecause(mutateObs('obs-rework-2', (d) => {
    d.payload.evidence[0].reference = 'field-evaluation/evidence/does-not-exist';
    d.payload.evidence[0].sha256 = '0'.repeat(64);
  }), /does not resolve to an actual evidence-result/), 'an unresolvable evidence reference was accepted');
});

check('CHANGES_REQUESTED — evidence of a different metric is rejected (metric_ref subject binding)', () => {
  const stopObs = OBS('obs-stop-correct-1');
  const foreign = OBS('obs-missed-missed-1').payload.evidence[0];
  const mutated = mutateObs('obs-stop-correct-1', (d) => {
    d.payload.evidence[0].reference = foreign.reference;
    d.payload.evidence[0].sha256 = foreign.sha256;
  });
  assert(rejectsBecause(mutated, /is not this observation's own metric/), 'evidence of a different metric was accepted');
});

check('CHANGES_REQUESTED — a wrong classification of a stop without a basis is rejected', () => {
  assert(rejectsBecause(mutateObs('obs-stop-correct-1', (d) => { delete d.payload.measurement.basis; }),
    /classification basis/), 'a classification with no basis was accepted');
});

check('CHANGES_REQUESTED — a classification outcome outside the metric\'s own closed pool is rejected', () => {
  assert(rejectsBecause(mutateObs('obs-missed-missed-1', (d) => { d.payload.measurement.outcome = 'partially_missed'; }),
    /is not one of the closed pool for/), 'an outcome outside the closed pool was accepted');
});

check('CHANGES_REQUESTED — a measurement kind that does not match the metric is rejected', () => {
  assert(rejectsBecause(mutateObs('obs-rework-1', (d) => {
    d.payload.measurement = { kind: 'duration', duration_kind: 'calendar', unit: 'seconds', seconds: 10 };
  }), /is measured by kind/), 'a mismatched measurement kind was accepted');
});

check('CHANGES_REQUESTED — status "observed" with a status_reason is rejected', () => {
  assert(rejectsBecause(mutateObs('obs-rework-1', (d) => { d.payload.status_reason = 'на всякий случай'; }),
    /carries a status_reason/), 'observed + status_reason was accepted');
});

check('CHANGES_REQUESTED — a non-observed status carrying a measurement is rejected', () => {
  const unk = fixtures.valid.find((c) => c.spec.payload && c.spec.payload.status === 'unknown');
  const d = clone(unk.spec);
  d.payload.measurement = { kind: 'classification', outcome: 'correct', basis: 'x' };
  assert(rejectsBecause(d, /carries a measurement; a value that was not observed is not measured/), 'unknown + measurement was accepted');
});

check('CHANGES_REQUESTED — a correction with no correction_reason is rejected', () => {
  const fix = fixtures.valid.find((c) => c.spec.id === 'obs-correction-fix');
  const d = clone(fix.spec);
  delete d.payload.correction_reason;
  assert(rejectsBecause(d, /states only one of supersedes \/ correction_reason/), 'supersedes with no correction_reason was accepted');
});

check('CHANGES_REQUESTED — run substitution (pin resolves to a different run) is rejected', () => {
  const mutated = mutateObs('obs-mech-correct-1', (d) => {
    d.payload.execution_run_ref.reference = 'runs/run-beta';
    d.payload.execution_run_ref.sha256 = fixtures.resolution['runs/run-beta'].content_digest;
  });
  assert(rejectsBecause(mutated, /resolves to a DIFFERENT record than was pinned|resolves to a DIFFERENT|pins id "run-alpha" but the reference resolves to record id "run-beta"/),
    'run substitution was accepted');
});

check('FORBIDDEN_PAYLOAD_FIELDS — a forbidden field on an observation is rejected', () => {
  assert(FORBIDDEN_PAYLOAD_FIELDS.includes('overall_score'), 'overall_score is not in the forbidden list');
  assert(rejectsBecause(mutateObs('obs-mech-correct-1', (d) => { d.payload.overall_score = 95; }),
    /payload carries "overall_score"/), 'a forbidden overall_score field was accepted on an observation');
});

check('a scope other than run-state is rejected for an observation', () => {
  assert(rejectsBecause(mutateObs('obs-mech-correct-1', (d) => { d.scope.type = 'project-workspace'; }),
    /lives in run-state only/), 'an observation scoped to project-workspace was accepted');
});

// ===========================================================================
// negative scenarios — report level
// ===========================================================================
check('CHANGES_REQUESTED — a repeated observation reference is not counted twice', () => {
  assert(rejectsBecause(mutateReport((d) => { d.payload.included_observations.push(clone(d.payload.included_observations[0])); }),
    /repeats reference/), 'a duplicated included reference was accepted');
});

check('CHANGES_REQUESTED — numerator greater than denominator is rejected', () => {
  const d = mutateReport((x) => {
    const pm = x.payload.per_metric.find((p) => p.metric_id === 'mechanism-correctness');
    pm.aggregate.rate.numerator = pm.aggregate.rate.denominator + 5;
    pm.aggregate.rate.percentage = 100;
  });
  assert(rejectsBecause(d, /does not match the value recomputed/), 'numerator greater than denominator was accepted');
});

check('CHANGES_REQUESTED — a ratio at a zero denominator never yields a percentage', () => {
  const d = mutateReport((x) => {
    const pm = x.payload.per_metric.find((p) => p.metric_id === 'resumption-success');
    pm.aggregate.rate.denominator = 0;
    pm.aggregate.rate.numerator = 0;
    pm.aggregate.rate.percentage = 0;
    pm.aggregate.total = 0;
  });
  assert(rejectsBecause(d, /does not match the value recomputed|states status/), 'a zero-denominator percentage was accepted');
});

check('CHANGES_REQUESTED — mixing incomparable periods is rejected', () => {
  const outOfPeriod = OBS('obs-out-of-period');
  const d = mutateReport((x) => {
    x.payload.included_observations.push({
      record_type: 'field-evaluation-observation', id: outOfPeriod.id,
      reference: `field-evaluation/observations/${outOfPeriod.id}`,
      sha256: fixtures.resolution[`field-evaluation/observations/${outOfPeriod.id}`].content_digest,
    });
    const pm = x.payload.per_metric.find((p) => p.metric_id === 'mechanism-correctness');
    pm.sample_observation_ids.push(outOfPeriod.id);
  });
  assert(rejectsBecause(d, /outside the report period/), 'an out-of-period observation was accepted into the report');
});

check('CHANGES_REQUESTED — mixing incomparable workspaces is rejected', () => {
  const otherWs = OBS('obs-other-workspace');
  const d = mutateReport((x) => {
    x.payload.included_observations.push({
      record_type: 'field-evaluation-observation', id: otherWs.id,
      reference: `field-evaluation/observations/${otherWs.id}`,
      sha256: fixtures.resolution[`field-evaluation/observations/${otherWs.id}`].content_digest,
    });
    const pm = x.payload.per_metric.find((p) => p.metric_id === 'mechanism-correctness');
    pm.sample_observation_ids.push(otherWs.id);
  });
  assert(rejectsBecause(d, /not this report's workspace/), 'a different-workspace observation was accepted into the report');
});

check('CHANGES_REQUESTED — a report diverging from its own observations is rejected', () => {
  const d = mutateReport((x) => {
    const pm = x.payload.per_metric.find((p) => p.metric_id === 'rework-returns');
    pm.aggregate.total = 999;
  });
  assert(rejectsBecause(d, /does not match the value recomputed/), 'a report whose aggregate diverges from its sample was accepted');
});

check('CHANGES_REQUESTED — a false conclusion of no defects over a partial window is rejected', () => {
  // A GENUINELY zero-defect, partial-coverage sample (not merely a hand-edited
  // stated total that would instead be caught as a divergent aggregate): a
  // custom resolver backs obs-defects-2 with value 0, keeping its own
  // coverage "partial". The builder discloses this correctly; the test then
  // hides the disclosure to prove it is required, not optional.
  const resolutionWithZeroDefects = clone(fixtures.resolution);
  resolutionWithZeroDefects['field-evaluation/observations/obs-defects-2'] = {
    ...resolutionWithZeroDefects['field-evaluation/observations/obs-defects-2'],
    measurement: { kind: 'count', unit: 'count', value: 0 },
  };
  const zeroResolve = makeRecordResolver(resolutionWithZeroDefects);
  const built = buildFieldEvaluationReport({
    workspaceId: REPORT.payload.workspace_id,
    period: REPORT.payload.period,
    includedRefs: REPORT.payload.included_observations,
    excludedEntries: REPORT.payload.excluded_observations,
    resolveRecords: zeroResolve,
  });
  const pm = built.per_metric.find((p) => p.metric_id === 'post-acceptance-defects');
  assert(pm.aggregate.total === 0, 'the custom resolver did not actually produce a zero total');
  assert(pm.aggregate.coverage === 'partial' || pm.aggregate.coverage === 'mixed', 'the custom resolver did not keep a partial/mixed coverage');
  assert(pm.limitations.length > 0, 'the builder itself did not disclose the zero-over-partial-coverage limitation');
  pm.limitations = []; // hide the disclosure — this is the defect under test
  const d = clone(REPORT);
  d.payload = built;
  const zeroOpts = { recordSchema, envelopeSchema, resolveRecords: zeroResolve };
  const problems = evaluateFieldEvaluation(d, zeroOpts);
  assert(problems.some((m) => /not proof of no defects/.test(m)), 'zero defects over a partial window with no limitation was accepted');
});

check('CHANGES_REQUESTED — a hidden exclusion with no disclosed limitation is rejected', () => {
  const d = mutateReport((x) => {
    const pm = x.payload.per_metric.find((p) => p.metric_id === 'mechanism-correctness');
    pm.limitations = [];
  });
  assert(rejectsBecause(d, /excluded observation of this metric but discloses no limitation/), 'a hidden exclusion was accepted');
});

check('CHANGES_REQUESTED — sample_observation_ids omitting an included observation is rejected', () => {
  const d = mutateReport((x) => {
    const pm = x.payload.per_metric.find((p) => p.metric_id === 'stop-correctness');
    pm.sample_observation_ids = pm.sample_observation_ids.slice(0, 1);
  });
  assert(rejectsBecause(d, /omits included observation/), 'an incomplete sample_observation_ids set was accepted');
});

check('CHANGES_REQUESTED — including both a superseded observation and its correction double-counts and is rejected', () => {
  const originalRef = {
    record_type: 'field-evaluation-observation', id: 'obs-correction-original',
    reference: 'field-evaluation/observations/obs-correction-original',
    sha256: fixtures.resolution['field-evaluation/observations/obs-correction-original'].content_digest,
  };
  const d = mutateReport((x) => {
    x.payload.included_observations.push(originalRef);
    const pm = x.payload.per_metric.find((p) => p.metric_id === 'mechanism-correctness');
    pm.sample_observation_ids.push('obs-correction-original');
  });
  assert(rejectsBecause(d, /the superseded record's facts are replaced, not additionally counted/), 'a superseded+superseding pair was accepted');
});

check('FORBIDDEN_PAYLOAD_FIELDS — an overall readiness verdict on a report is rejected', () => {
  assert(rejectsBecause(mutateReport((d) => { d.payload.readiness = 'ready'; }),
    /payload carries "readiness"/), 'a forbidden readiness field was accepted on a report');
});

check('per_metric missing one of the eight characteristics is rejected', () => {
  const d = mutateReport((x) => { x.payload.per_metric = x.payload.per_metric.slice(0, 7); });
  const p = evaluate(d);
  assert(p.length > 0, 'a per_metric array with only seven entries was accepted');
});

check('a report scoped outside project-workspace is rejected', () => {
  assert(rejectsBecause(mutateReport((d) => { d.scope.type = 'run-state'; d.scope.workspace_id = 'ws-meridian'; }),
    /lives in project-workspace only/), 'a report scoped to run-state was accepted');
});

// ===========================================================================
// CHANGES_REQUESTED round 2 — six review findings
// ===========================================================================

// A resolver built from the real fixtures.resolution with ONE entry replaced
// by a mutated copy — never mutating the shared bundle itself.
function resolverWith(mutations) {
  const res = clone(fixtures.resolution);
  for (const [ref, mutate] of mutations) res[ref] = mutate(clone(res[ref]));
  return makeRecordResolver(res);
}

const BASE_PERIOD = { start_date: '2026-08-01', end_date: '2026-08-31' };
function refOf(obsId) {
  return {
    record_type: 'field-evaluation-observation', id: obsId,
    reference: `field-evaluation/observations/${obsId}`,
    sha256: fixtures.resolution[`field-evaluation/observations/${obsId}`].content_digest,
  };
}

// ---------------------------------------------------------------------------
// 1. Strict calendar date/time validity
// ---------------------------------------------------------------------------
check('isValidDate accepts a genuine leap-year date and rejects impossible calendar dates', () => {
  assert(isValidDate('2028-02-29') === true, 'a genuine leap-year 29 February was rejected');
  assert(isValidDate('2026-08-15') === true, 'an ordinary valid date was rejected');
  assert(isValidDate('2026-02-31') === false, '31 February was accepted');
  assert(isValidDate('2026-04-31') === false, '31 April (a 30-day month) was accepted');
  assert(isValidDate('2026-02-29') === false, '29 February in a NON-leap year (2026) was accepted');
  assert(isValidDate('2026-13-01') === false, 'month 13 was accepted');
  assert(isValidDate('not-a-date') === false, 'a non-date string was accepted');
});

check('isValidDateTime rejects a timestamp naming an impossible calendar date', () => {
  assert(isValidDateTime('2026-08-15T10:00:00Z') === true, 'a well-formed valid timestamp was rejected');
  assert(isValidDateTime('2026-02-31T10:00:00Z') === false, 'a timestamp naming 31 February was accepted');
  assert(isValidDateTime('2026-08-15T25:00:00Z') === false, 'an impossible hour (25) was accepted');
});

check('CHANGES_REQUESTED — a fixture observed_at of 31 February is rejected (not silently rolled over)', () => {
  const d = mutateObs('obs-mech-correct-1', (x) => { x.payload.observed_at = '2026-02-31'; });
  assert(rejectsBecause(d, /observed_at is not a valid date/), 'observed_at "2026-02-31" was accepted');
});

check('CHANGES_REQUESTED — a fixture observed_at of 31 April is rejected', () => {
  const d = mutateObs('obs-mech-correct-1', (x) => { x.payload.observed_at = '2026-04-31'; });
  assert(rejectsBecause(d, /observed_at is not a valid date/), 'observed_at "2026-04-31" was accepted');
});

check('CHANGES_REQUESTED — a fixture observed_at of 29 February in a non-leap year is rejected', () => {
  const d = mutateObs('obs-mech-correct-1', (x) => { x.payload.observed_at = '2026-02-29'; });
  assert(rejectsBecause(d, /observed_at is not a valid date/), 'observed_at "2026-02-29" (non-leap) was accepted');
});

check('CHANGES_REQUESTED — an impossible start_ts calendar date is rejected even though end_ts is later', () => {
  const d = mutateObs('obs-context-time-1', (x) => {
    x.payload.measurement.start_ts = '2026-02-31T10:00:00Z';
    x.payload.measurement.end_ts = '2026-03-01T10:00:00Z';
  });
  assert(rejectsBecause(d, /not a valid timestamp/), 'an impossible start_ts calendar date was accepted');
});

check('a genuine leap-year observation fixture is bundled and passes', () => {
  const leap = fixtures.valid.find((c) => c.spec.id === 'obs-leap-date-1');
  assert(leap, 'no bundled fixture exercises a leap-year date');
  assert(leap.spec.payload.observed_at === '2028-02-29', 'the leap-date fixture does not actually use 29 February');
  assert(evaluate(leap.spec).length === 0, 'the leap-date fixture was rejected');
});

// ---------------------------------------------------------------------------
// 2. Included and excluded observations are disjoint sets — each scenario is
// a NAMED fixture in fixtures.invalid (not a test-file-only mutation), found
// by its note and evaluated exactly like any other invalid fixture.
// ---------------------------------------------------------------------------
function invalidFixture(note) {
  const c = fixtures.invalid.find((x) => x.note === note);
  assert(c, `no bundled invalid fixture has the note "${note}"`);
  return c.spec;
}

check('CHANGES_REQUESTED — the same reference in both included and excluded is rejected (fixture)', () => {
  const spec = invalidFixture('report: одинаковая reference во включённых и исключённых наблюдениях');
  assert(rejectsBecause(spec, /appears in both included_observations and excluded_observations/), 'the same reference in both lists was accepted');
});

check('CHANGES_REQUESTED — two different references resolving to the same observation (one included, one excluded) are rejected (fixture)', () => {
  const spec = invalidFixture('report: разные reference, разрешающиеся в одно наблюдение между включёнными и исключёнными');
  assert(rejectsBecause(spec, /is also an included observation/), 'two references resolving to the same observation (included + excluded) were accepted');
});

check('CHANGES_REQUESTED — a repeated excluded reference is not legalised by a different exclusion_reason (fixture)', () => {
  const spec = invalidFixture('report: повтор excluded reference с другой причиной исключения');
  assert(rejectsBecause(spec, /repeated excluded reference is not legalised/), 'a repeated excluded reference with a different reason was accepted');
});

check('CHANGES_REQUESTED — two different excluded references resolving to the same observation are rejected (fixture)', () => {
  const spec = invalidFixture('report: разные excluded reference, разрешающиеся в одно наблюдение');
  assert(rejectsBecause(spec, /another excluded_observations entry already resolves to/), 'two excluded references resolving to the same observation were accepted');
});

// ---------------------------------------------------------------------------
// 3. Exact aggregate recomputation regardless of per_metric status — also
// NAMED fixtures now, built with the real buildFieldEvaluationReport and then
// corrupted, not assembled ad hoc inside the test.
// ---------------------------------------------------------------------------
check('CHANGES_REQUESTED — a fabricated aggregate at status "unknown" is rejected (fixture)', () => {
  const spec = invalidFixture('report: выдуманный aggregate при статусе unknown (пустая выборка)');
  const pm = spec.payload.per_metric.find((p) => p.metric_id === 'rework-returns');
  assert(pm.status === 'unknown' && pm.aggregate.total === 20, 'the fixture does not actually fabricate a non-empty aggregate at "unknown"');
  assert(rejectsBecause(spec, /aggregate does not match the value recomputed/), 'a fabricated non-empty aggregate at status "unknown" was accepted');
});

check('CHANGES_REQUESTED — a fabricated aggregate at status "not_applicable" is rejected (fixture)', () => {
  const spec = invalidFixture('report: выдуманный aggregate при статусе not_applicable');
  const pm = spec.payload.per_metric.find((p) => p.metric_id === 'owner-cost');
  assert(pm.status === 'not_applicable' && pm.aggregate.calendar.total_seconds === 999, 'the fixture does not actually fabricate a non-canonical aggregate at "not_applicable"');
  assert(rejectsBecause(spec, /aggregate does not match the value recomputed/), 'a fabricated non-empty aggregate at status "not_applicable" was accepted');
});

// ---------------------------------------------------------------------------
// Resolved-projection validity at the REPORT level, also as named fixtures:
// an invented classification outcome, and a wrong resolved record_type.
// ---------------------------------------------------------------------------
check('CHANGES_REQUESTED — an invented classification outcome in a referenced observation\'s resolved projection is rejected (fixture)', () => {
  const spec = invalidFixture('report: неправильная разрешённая проекция включённого наблюдения (invented outcome)');
  assert(rejectsBecause(spec, /is not one of the closed pool for/), 'an invented classification outcome in the resolved projection was accepted');
});

check('CHANGES_REQUESTED — a wrong record_type in a referenced observation\'s resolved projection is rejected (fixture)', () => {
  const spec = invalidFixture('report: неправильный record_type разрешённой записи включённого наблюдения');
  assert(rejectsBecause(spec, /not "field-evaluation-observation"/), 'a wrong resolved record_type was accepted');
});

// ---------------------------------------------------------------------------
// builder_scenarios — the fixtures bundle's explicitly named section for
// cases that are about buildFieldEvaluationReport itself, not a standalone
// schema record. Every scenario is consumed FROM THE FIXTURE, not hand-rolled.
// ---------------------------------------------------------------------------
check('fixtures.builder_scenarios is present and non-empty', () => {
  assert(Array.isArray(fixtures.builder_scenarios) && fixtures.builder_scenarios.length >= 4,
    'fewer than 4 builder scenarios are bundled');
});

for (const sc of (fixtures.builder_scenarios || [])) {
  check(`builder scenario (fixture) — ${sc.note}`, () => {
    let threw = null;
    try {
      buildFieldEvaluationReport({
        workspaceId: 'ws-meridian', period: BASE_PERIOD,
        includedRefs: sc.includedRefs, excludedEntries: sc.excludedEntries, resolveRecords,
      });
    } catch (e) { threw = e; }
    assert(threw instanceof Error, `buildFieldEvaluationReport did not throw for "${sc.note}"`);
    assert(threw.message.includes(sc.messageContains),
      `"${sc.note}" threw an unexpected message: ${threw.message}`);
  });
}

// ---------------------------------------------------------------------------
// 4. The resolved observation projection is validated before aggregation
// ---------------------------------------------------------------------------
function expectBuildThrows(name, includedRefs, resolver, msgRe) {
  check(name, () => {
    let threw = null;
    try {
      buildFieldEvaluationReport({ workspaceId: 'ws-meridian', period: BASE_PERIOD, includedRefs, excludedEntries: [], resolveRecords: resolver });
    } catch (e) { threw = e; }
    assert(threw instanceof Error, 'buildFieldEvaluationReport did not throw for an invalid/unresolvable resolved observation');
    assert(msgRe.test(threw.message), `unexpected error message: ${threw.message}`);
  });
}

expectBuildThrows(
  'buildFieldEvaluationReport fails closed on an unknown classification outcome in the resolved projection',
  [refOf('obs-stop-correct-1')],
  resolverWith([['field-evaluation/observations/obs-stop-correct-1',
    (e) => ({ ...e, measurement: { ...e.measurement, outcome: 'bogus-outcome' } })]]),
  /invalid observation projection.*is not one of the closed pool/,
);

expectBuildThrows(
  'buildFieldEvaluationReport fails closed on a negative count in the resolved projection',
  [refOf('obs-rework-2')],
  resolverWith([['field-evaluation/observations/obs-rework-2',
    (e) => ({ ...e, measurement: { ...e.measurement, value: -3 } })]]),
  /invalid observation projection.*is not a non-negative integer/,
);

expectBuildThrows(
  'buildFieldEvaluationReport fails closed on a missing workspace_id in the resolved projection',
  [refOf('obs-mech-correct-1')],
  resolverWith([['field-evaluation/observations/obs-mech-correct-1',
    (e) => { const c = { ...e }; delete c.workspace_id; return c; }]]),
  /invalid observation projection.*workspace_id/,
);

expectBuildThrows(
  'buildFieldEvaluationReport fails closed on an impossible observed_at in the resolved projection',
  [refOf('obs-mech-correct-1')],
  resolverWith([['field-evaluation/observations/obs-mech-correct-1',
    (e) => ({ ...e, observed_at: '2026-02-30' })]]),
  /invalid observation projection.*observed_at/,
);

expectBuildThrows(
  'buildFieldEvaluationReport fails closed on an unresolvable included reference, rather than silently dropping it',
  [{ record_type: 'field-evaluation-observation', id: 'does-not-exist', reference: 'field-evaluation/observations/does-not-exist' }],
  resolveRecords,
  /does not resolve to an actual field-evaluation-observation/,
);

check('the validator ALSO rejects a report referencing a resolved observation with an invalid projection, not only the builder', () => {
  const badResolve = resolverWith([['field-evaluation/observations/obs-mech-correct-1',
    (e) => ({ ...e, metric_id: 'not-a-real-metric' })]]);
  const problems = evaluateFieldEvaluation(REPORT, { recordSchema, envelopeSchema, resolveRecords: badResolve });
  assert(problems.some((m) => /resolved observation metric_id/.test(m)), 'the validator accepted a report whose resolved observation projection is invalid');
});

// ---------------------------------------------------------------------------
// 5. Builder determinism under permutation of EITHER input list
// ---------------------------------------------------------------------------
check('permuting excludedEntries alone does not change the built report', () => {
  const a = buildFieldEvaluationReport({
    workspaceId: REPORT.payload.workspace_id, period: REPORT.payload.period,
    includedRefs: REPORT.payload.included_observations, excludedEntries: REPORT.payload.excluded_observations, resolveRecords,
  });
  const b = buildFieldEvaluationReport({
    workspaceId: REPORT.payload.workspace_id, period: REPORT.payload.period,
    includedRefs: REPORT.payload.included_observations, excludedEntries: [...REPORT.payload.excluded_observations].reverse(), resolveRecords,
  });
  assert(JSON.stringify(a) === JSON.stringify(b), 'permuting excludedEntries changed the built report');
});

check('permuting BOTH includedRefs and excludedEntries together does not change the built report', () => {
  const a = buildFieldEvaluationReport({
    workspaceId: REPORT.payload.workspace_id, period: REPORT.payload.period,
    includedRefs: REPORT.payload.included_observations, excludedEntries: REPORT.payload.excluded_observations, resolveRecords,
  });
  const shuffledIncluded = [...REPORT.payload.included_observations].sort((x, y) => (x.reference < y.reference ? 1 : -1));
  const b = buildFieldEvaluationReport({
    workspaceId: REPORT.payload.workspace_id, period: REPORT.payload.period,
    includedRefs: shuffledIncluded, excludedEntries: [...REPORT.payload.excluded_observations].reverse(), resolveRecords,
  });
  assert(JSON.stringify(a) === JSON.stringify(b), 'permuting both input lists together changed the built report');
});

check('buildFieldEvaluationReport rejects (throws) a duplicated included reference rather than silently deduping it', () => {
  let threw = null;
  try {
    buildFieldEvaluationReport({
      workspaceId: 'ws-meridian', period: BASE_PERIOD,
      includedRefs: [refOf('obs-mech-correct-1'), refOf('obs-mech-correct-1')], excludedEntries: [], resolveRecords,
    });
  } catch (e) { threw = e; }
  assert(threw instanceof Error && /repeats reference/.test(threw.message), 'a duplicated included reference was silently accepted instead of rejected');
});

check('buildFieldEvaluationReport rejects (throws) an observation that is both included and excluded', () => {
  let threw = null;
  try {
    buildFieldEvaluationReport({
      workspaceId: 'ws-meridian', period: BASE_PERIOD,
      includedRefs: [refOf('obs-mech-correct-1')],
      excludedEntries: [{ ref: refOf('obs-mech-correct-1'), exclusion_reason: 'опробовать пересечение' }],
      resolveRecords,
    });
  } catch (e) { threw = e; }
  assert(threw instanceof Error, 'an observation both included and excluded was silently accepted by the builder');
});

console.log('---');
console.log(`${passed} passed, ${failures.length} failed`);
if (failures.length) console.log('\n' + failures.join('\n'));
process.exit(failures.length > 0 ? 1 : 0);
