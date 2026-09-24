#!/usr/bin/env node
// Regression suite for verification/conformance-harness/conformance-harness.mjs.
//
// Three layers, all through the module's real exported functions — never a
// second, separately-written comparison or aggregation algorithm:
//   - white-box (in-process): normalizeDiagnostics/compareVerdicts/
//     runProducer/runCase are exercised directly against small in-memory
//     inputs, including runConformanceCheck's own aggregation rule.
//   - self-check (in-process, library call): the full controlled corpus
//     (verification/conformance-harness/fixtures/conformance-harness.fixtures.json)
//     is run through runCorpus() — the exact same route hooks/pre-push and
//     .github/workflows/gate.yml take by running this file. `expected_status`
//     is read ONLY here, never by the public CLI.
//   - black-box (real subprocess): conformance-harness.mjs is spawned as an
//     actual child process — the same way an operator or pre-push would run
//     it — and its REAL process exit code is asserted, not merely an
//     in-memory `.match`/`.status` field. This is the layer that proves the
//     public CLI's exit-code contract (conformant → 0; divergent or
//     harness_error → non-zero, regardless of any fixture's own
//     `expected_status`) actually holds for the file operators run.
//
// Every black-box case needs its own one-case fixtures file on disk
// (singleCaseFixture() mkdtemp's one per call). check() removes each such
// directory in a finally block scoped to that one check, immediately after
// it runs — on a pass, on a failed assert(), or on any other thrown error —
// so no temp directory is ever left depending on the rest of the suite (or
// the whole process) exiting cleanly.
//
// Usage: node test/conformance-harness.test.mjs
//
// Selective run: `node --test --test-name-pattern '<pattern>' <this file>`
// hands the pattern to this process (process.execArgv). When a pattern is
// given, ONLY the selectable sections run (today: `meridian-cli-migration`,
// package 8, and `accepted-rust-native`, package 9), their checks are
// filtered by the pattern, and the process
// exits right after them — the full suite is never run half-filtered.

import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import { runProducer, normalizeDiagnostics, compareVerdicts, runCase, runCorpus, runConformanceCheck } from '../verification/conformance-harness/conformance-harness.mjs';
import { evaluateControlledRuleIntake } from '../scripts/lib/controlled-rule-intake.mjs';
import { makeRecordResolver } from '../scripts/lib/context-manifest.mjs';
import { evaluateInstructionSourceRegistry } from '../scripts/lib/instruction-source-registry.mjs';
import { evaluateExistingProjectCompatibilityMode } from '../scripts/lib/existing-project-compatibility-mode.mjs';
import { evaluateEvidenceAndHandoff } from '../scripts/lib/evidence-and-handoff.mjs';
import { evaluateFieldEvaluation } from '../scripts/lib/field-evaluation.mjs';
import { yamlParse } from '../scripts/lib/yaml.mjs';
import {
  evaluateInstanceDataMigration, evaluateInstanceCanonicalExport, checkOpaqueRef,
  makeSourceSnapshotResolver, makeEvidenceResolver, makeRollbackSnapshotResolver,
  makeDeterministicPlanResolver, makeRestorationEvidenceResolver, makeSupersededPlanResolver,
  makeMigrationPlanResolver, makeSourceContentResolver, makeRefResolver,
  computePlanFingerprint, computeIdempotencyKey, computeExportDigest,
} from '../scripts/lib/instance-data-migration.mjs';
import { evaluateWorkspaceCompatibilityQualification, computeConnectionDigest } from '../scripts/lib/workspace-compatibility-qualification.mjs';
import { evaluateUpgradeIntegrationQualification, computeContentDigest } from '../scripts/lib/upgrade-integration-qualification.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.join(__dirname, '..');
const FIXTURES = path.join(__dirname, '..', 'verification', 'conformance-harness', 'fixtures', 'conformance-harness.fixtures.json');
const CLI = path.join(__dirname, '..', 'verification', 'conformance-harness', 'conformance-harness.mjs');
const CORPUS_RAW = JSON.parse(fs.readFileSync(FIXTURES, 'utf8'));

// Every directory singleCaseFixture() mkdtemp's is tracked here so it can be
// removed deterministically — never left for the OS or for the whole suite's
// own successful exit to clean up.
const createdTempDirs = [];

// Build a one-case fixtures file re-using an existing controlled-corpus case
// verbatim (same producers, same declared expected_status) — this is fixture
// data reuse for a different aggregation policy, not a second algorithm.
// The directory is recorded in createdTempDirs immediately after mkdtemp
// succeeds — before the write that follows — so a failure in that write
// still leaves the directory tracked for cleanup rather than orphaned.
function singleCaseFixture(name) {
  const caseDef = CORPUS_RAW.cases.find((c) => c.name === name);
  if (!caseDef) throw new Error(`no controlled-corpus case named "${name}"`);
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'conformance-harness-cli-test-'));
  createdTempDirs.push(dir);
  const file = path.join(dir, 'single-case.json');
  fs.writeFileSync(file, JSON.stringify({ cases: [caseDef] }));
  return file;
}

// Remove every tracked temp directory created at or after `fromIndex`. Used
// by check() below as a finally-block, so it runs whether the check's body
// returned normally, threw a failed assertion, or threw anything else.
function cleanupTempDirsFrom(fromIndex) {
  while (createdTempDirs.length > fromIndex) {
    const dir = createdTempDirs.pop();
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

function runCli(fixturePath) {
  return spawnSync(process.execPath, [CLI, fixturePath], { encoding: 'utf8' });
}

let passed = 0;
const failures = [];

function check(name, fn) {
  const tempDirsBefore = createdTempDirs.length;
  try {
    fn();
    passed++;
    console.log(`PASS ${name}`);
  } catch (error) {
    failures.push(`${name}: ${error.message}`);
    console.log(`FAIL ${name}: ${error.message}`);
  } finally {
    // Guaranteed cleanup of whatever this specific check created, on every
    // exit path — success, a failed assert(), or any other thrown error.
    // Never deferred to the whole suite finishing.
    cleanupTempDirsFrom(tempDirsBefore);
  }
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

// The one environment of every Kernel-only child process: a copy of this
// process's environment with `overrides` (such as `MERIDIAN_KERNEL`)
// applied and `MERIDIAN_INSTANCE` removed. A Kernel-validation comparison
// must never let an ambient frozen Instance inject product diagnostics into
// one side only — the Rust binary never reads that variable, so neither
// side of a Kernel-only comparison may see it.
function kernelOnlyEnv(overrides = {}) {
  const env = { ...process.env, ...overrides };
  delete env.MERIDIAN_INSTANCE;
  return env;
}

// --- selective runs: the pattern `node --test --test-name-pattern` passes ---

function testNamePattern(execArgv) {
  for (let i = 0; i < execArgv.length; i += 1) {
    const arg = execArgv[i];
    if (arg.startsWith('--test-name-pattern=')) return new RegExp(arg.slice('--test-name-pattern='.length));
    if (arg === '--test-name-pattern' && i + 1 < execArgv.length) return new RegExp(execArgv[i + 1]);
  }
  return null;
}
const NAME_PATTERN = testNamePattern(process.execArgv);

// --- package meridian-cli-migration (§5.22): Node reference vs the Rust
// `import`/`migration` binary over the REAL frozen bundle. The source is
// named by MERIDIAN_INSTANCE for this harness only; the Rust binary is
// handed it exclusively through `--source`, and its environment has
// MERIDIAN_INSTANCE removed. ---

function runMeridianCliMigrationConformance() {
  const selected = (name) => !NAME_PATTERN || NAME_PATTERN.test(name);
  const mcheck = (name, fn) => { if (selected(name)) check(name, fn); };
  const source = process.env.MERIDIAN_INSTANCE;
  if (!source) {
    mcheck('meridian-cli-migration: харнессу нужен MERIDIAN_INSTANCE с замороженным источником (UNVERIFIED без него)', () => {
      assert(false, 'MERIDIAN_INSTANCE не задан — доказательства пакета 8 не выполнены');
    });
    return;
  }
  const build = spawnSync('cargo', ['build', '-p', 'meridian-cli', '--bin', 'meridian'], { cwd: ROOT, encoding: 'utf8' });
  const bin = path.join(ROOT, 'target', 'debug', process.platform === 'win32' ? 'meridian.exe' : 'meridian');
  // The frozen source reaches the binary ONLY through `--source`: its
  // environment is the Kernel-only one, without MERIDIAN_INSTANCE.
  const env = kernelOnlyEnv();
  const meridian = (args) => spawnSync(bin, args, { cwd: ROOT, encoding: 'utf8', env, maxBuffer: 1 << 28 });
  const json = (r) => JSON.parse(r.stdout);
  const canonicalize = (v) => {
    if (Array.isArray(v)) return v.map(canonicalize);
    if (v !== null && typeof v === 'object') {
      const out = {};
      for (const k of Object.keys(v).sort()) out[k] = canonicalize(v[k]);
      return out;
    }
    return v;
  };
  const canon = (v) => JSON.stringify(canonicalize(v));
  const bundleDir = path.join(source, 'migration', 'instance-data');
  const readBundle = (rel) => JSON.parse(fs.readFileSync(path.join(bundleDir, rel), 'utf8'));
  const registry = readBundle('registry.json');
  const exportDoc = readBundle('canonical-export.json');
  const plan = registry.migration_plans[0];
  const payload = plan.payload;
  const revision = payload.source.revision;
  const nodeFingerprint = computePlanFingerprint(payload);
  const nodeKey = computeIdempotencyKey(plan.scope, payload.source);
  const lsTree = spawnSync('git', ['-C', source, 'ls-tree', '-r', '--format=%(objectmode) %(objecttype) %(objectname) %(path)', revision], { encoding: 'utf8' });
  const nodeTreeDigest = crypto.createHash('sha256').update(`${lsTree.stdout.trim().split('\n').filter(Boolean).sort().join('\n')}\n`).digest('hex');
  const count = (d) => payload.mappings.filter((m) => m.disposition === d).length;
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'meridian-cli-migration-conformance-'));
  try {
    runMigrationChecks({ mcheck, meridian, json, canon, source, registry, exportDoc, payload, revision, nodeFingerprint, nodeKey, nodeTreeDigest, count, dir, build, readBundle });
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

function runMigrationChecks({ mcheck, meridian, json, canon, source, registry, exportDoc, payload, revision, nodeFingerprint, nodeKey, nodeTreeDigest, count, dir, build, readBundle }) {
  const init = (name) => {
    const r = meridian(['init', '--kernel', ROOT, '--workspace', path.join(dir, name), '--tool-db', path.join(dir, `${name}-tool.db`), '--workspace-db', path.join(dir, `${name}-ws.db`), '--format', 'json']);
    return { r, tool: path.join(dir, `${name}-tool.db`), ws: path.join(dir, `${name}-ws.db`) };
  };

  mcheck('meridian-cli-migration: Rust-бинарник собран', () => {
    assert(build.status === 0, `cargo build завершился кодом ${build.status}: ${build.stderr}`);
  });

  let planResult = null;
  mcheck('meridian-cli-migration: Node-эталон принимает реальный bundle (evaluateInstanceDataMigration без проблем), Rust `migration plan` принимает тот же bundle и пересчитывает тот же plan fingerprint, idempotency key, tree digest и числа 346/336/10/0', () => {
    const problems = evaluateInstanceDataMigration(registry, {
      registrySchema: JSON.parse(fs.readFileSync(path.join(ROOT, 'registries/operating-model/instance-data-migration.schema.json'), 'utf8')),
      envelopeSchema: JSON.parse(fs.readFileSync(path.join(ROOT, 'registries/operating-model/scoped-record.schema.json'), 'utf8')),
      resolveSourceSnapshot: makeSourceSnapshotResolver(readBundle('source-snapshot.json').resolution),
      resolveEvidence: makeEvidenceResolver({ ...readBundle('evidence/coverage.json').resolution, ...readBundle('evidence/applicability-preservation.json').resolution }),
      resolveRollbackSnapshot: makeRollbackSnapshotResolver(readBundle('rollback-snapshot.json').resolution),
      resolveDeterministicPlan: makeDeterministicPlanResolver(readBundle('deterministic-reconstruction-plan.json').resolution),
      resolveRestorationEvidence: makeRestorationEvidenceResolver({}),
      resolveSupersededPlan: makeSupersededPlanResolver({}),
    });
    assert(problems.length === 0, `Node отклонил bundle: ${problems.slice(0, 3).join(' | ')}`);
    const r = meridian(['migration', 'plan', '--kernel', ROOT, '--source', source, '--format', 'json']);
    assert(r.status === 0, `migration plan завершился кодом ${r.status}: ${r.stderr}`);
    planResult = json(r).result;
    const head = spawnSync('git', ['-C', source, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).stdout.trim();
    assert(planResult.bundle_revision === head, `bundle commit ${planResult.bundle_revision} ≠ HEAD ${head}`);
    assert(planResult.plan.plan_fingerprint.value === nodeFingerprint, `fingerprint Rust ${planResult.plan.plan_fingerprint.value} ≠ Node ${nodeFingerprint}`);
    assert(planResult.plan.idempotency_key.value === nodeKey, 'idempotency key расходится');
    assert(planResult.source.digest.value === nodeTreeDigest, 'tree digest расходится');
    assert(planResult.source.digest.value === payload.source.digest.value, 'tree digest ≠ заявленного source digest');
    const c = planResult.plan.counts;
    assert(c.record_units === payload.record_units.length && c.record_units === 346, `record_units ${c.record_units}`);
    assert(c.migrated === count('migrated') && c.migrated === 336, `migrated ${c.migrated}`);
    assert(c.retained === count('retained-transitional') && c.retained === 10, `retained ${c.retained}`);
    assert(c.merged === count('merged') && c.merged === 0, `merged ${c.merged}`);
    assert(planResult.canonical_export.digest.value === computeExportDigest(exportDoc.exports[0].payload), 'export digest расходится с Node computeExportDigest');
  });

  const first = init('first');
  let e1 = null;
  let verifyResult = null;
  mcheck('meridian-cli-migration: Rust `import frozen-instance` импортирует 336 записей, удерживает 10, verify = verified (missing/extra/changed/duplicate = 0)', () => {
    assert(first.r.status === 0, `init: ${first.r.stderr}`);
    const r = meridian(['import', '--kernel', ROOT, '--kind', 'frozen-instance', '--source', source, '--tool-db', first.tool, '--workspace-db', first.ws, '--confirm', nodeFingerprint, '--format', 'json']);
    assert(r.status === 0, `import завершился кодом ${r.status}: ${r.stderr} ${r.stdout.slice(0, 400)}`);
    const i = json(r).result;
    verifyResult = i.verify;
    assert(i.apply.effects.created === 336 && i.apply.retained === 10, JSON.stringify(i.apply.effects));
    for (const [k, v] of Object.entries({ expected: 336, imported: 336, missing: 0, extra: 0, changed: 0, duplicate: 0, retained: 10 })) {
      assert(i.verify[k] === v, `verify.${k} = ${i.verify[k]}, ожидалось ${v}`);
    }
    const x = meridian(['export', '--kernel', ROOT, '--tool-db', first.tool, '--workspace-db', first.ws, '--format', 'json']);
    assert(x.status === 0, `export: ${x.stderr}`);
    e1 = x.stdout;
  });

  mcheck('meridian-cli-migration: каждая запись, прочитанная из SQLite через Rust `export`, равна записи принятого Node-экспорта bundle по $schema/id/title/record_type/scope/origin/authority/payload/schema_version', () => {
    assert(e1 !== null, 'нет экспорта');
    const key = (r) => `${r.scope.type}\u001f${r.scope.id}\u001f${r.scope.workspace_id ?? ''}\u001f${r.id}`;
    const rust = new Map(JSON.parse(e1).result.map((r) => [key(r), canon(r)]));
    const node = exportDoc.exports[0].payload.records;
    assert(rust.size === 336 && node.length === 336, `Rust ${rust.size}, Node ${node.length}`);
    const differing = node.filter((r) => rust.get(key(r)) !== canon(r)).map((r) => r.id);
    assert(differing.length === 0, `расходятся: ${differing.slice(0, 5).join(', ')}`);
  });

  mcheck('meridian-cli-migration: эквивалентность применимости — Node-эталон по закреплённому источнику и по payload реально прочитанных SQLite-записей даёт 61 норму, lost/added/changed/duplicate = 0, как и Rust verify', () => {
    assert(e1 !== null && verifyResult !== null, 'нет импорта');
    const register = 'rule-resolution/applicability.yaml';
    const before = yamlParse(spawnSync('git', ['-C', source, 'show', `${revision}:${register}`], { encoding: 'utf8' }).stdout).records;
    const units = new Map(payload.record_units.map((u) => [u.id, u.unit_ref]));
    const after = JSON.parse(e1).result
      .filter((r) => r.origin.kind === 'migrated' && (units.get(r.origin.source_ref.replace(/^record-unit:/, '')) || '').startsWith(`${register}#records/`))
      .map((r) => JSON.parse(r.payload.content));
    const identity = (rec) => JSON.stringify([rec.norm.repository, rec.norm.path, rec.norm.region ?? '', rec.intake_record?.register ?? '', rec.intake_record?.recorded_at ?? '', rec.intake_record?.verdict ?? '', rec.recorded_at ?? '']);
    const outcome = (rec) => canon({ scope: rec.scope, activation: rec.activation, status: rec.status, repository: rec.repository ?? null, technology_profile: rec.technology_profile ?? null, globs: rec.globs ? [...rec.globs].sort() : null });
    const index = (records) => { const m = new Map(); let dup = 0; for (const r of records) { if (m.has(identity(r))) dup += 1; else m.set(identity(r), outcome(r)); } return { m, dup }; };
    const b = index(before);
    const a = index(after);
    const lost = [...b.m.keys()].filter((k) => !a.m.has(k)).length;
    const added = [...a.m.keys()].filter((k) => !b.m.has(k)).length;
    const changed = [...b.m.keys()].filter((k) => a.m.has(k) && a.m.get(k) !== b.m.get(k)).length;
    assert(b.m.size === 61 && after.length === 61, `источник ${b.m.size}, импортировано ${after.length}`);
    assert(lost === 0 && added === 0 && changed === 0 && b.dup === 0 && a.dup === 0, `lost=${lost} added=${added} changed=${changed} dup=${b.dup + a.dup}`);
    const ra = verifyResult.applicability;
    assert(ra.verdict === 'equivalent' && ra.controlled === 61 && ra.imported === 61 && ra.lost === 0 && ra.added === 0 && ra.changed === 0 && ra.duplicate === 0, `Rust: ${JSON.stringify(ra)}`);
  });

  mcheck('meridian-cli-migration: повторные `import frozen-instance` и `migration apply` дают already-applied и не меняют экспорт', () => {
    assert(e1 !== null, 'нет импорта');
    const r = meridian(['import', '--kernel', ROOT, '--kind', 'frozen-instance', '--source', source, '--tool-db', first.tool, '--workspace-db', first.ws, '--confirm', nodeFingerprint, '--format', 'json']);
    assert(r.status === 0 && json(r).result.apply.status === 'already-applied', r.stdout.slice(0, 300));
    const a = meridian(['migration', 'apply', '--kernel', ROOT, '--source', source, '--workspace-db', first.ws, '--dry-run', 'false', '--confirm', nodeFingerprint, '--format', 'json']);
    assert(a.status === 0 && json(a).result.status === 'already-applied', a.stdout.slice(0, 300));
    const x = meridian(['export', '--kernel', ROOT, '--tool-db', first.tool, '--workspace-db', first.ws, '--format', 'json']);
    assert(x.stdout === e1, 'экспорт изменился после повтора');
  });

  mcheck('meridian-cli-migration: round trip import → export → import → export байт-в-байт', () => {
    assert(e1 !== null, 'нет импорта');
    const input = path.join(dir, 'export.json');
    fs.writeFileSync(input, e1);
    const second = init('second');
    const digest = crypto.createHash('sha256').update(e1).digest('hex');
    const r = meridian(['import', '--kernel', ROOT, '--kind', 'canonical-records', '--input', input, '--tool-db', second.tool, '--workspace-db', second.ws, '--confirm', digest, '--format', 'json']);
    assert(r.status === 0, `canonical import: ${r.stderr}`);
    const x = meridian(['export', '--kernel', ROOT, '--tool-db', second.tool, '--workspace-db', second.ws, '--format', 'json']);
    assert(x.stdout === e1, 'второй экспорт не равен первому байт-в-байт');
  });

  mcheck('meridian-cli-migration: реальный apply → rollback возвращает канонический экспорт к pre-state; повторный rollback отклоняется', () => {
    const third = init('third');
    const pre = meridian(['export', '--kernel', ROOT, '--tool-db', third.tool, '--workspace-db', third.ws, '--format', 'json']).stdout;
    const a = meridian(['migration', 'apply', '--kernel', ROOT, '--source', source, '--workspace-db', third.ws, '--dry-run', 'false', '--confirm', nodeFingerprint, '--format', 'json']);
    assert(a.status === 0, `apply: ${a.stderr}`);
    const run = json(a).result.run_id;
    const rb = meridian(['migration', 'rollback', '--kernel', ROOT, '--source', source, '--workspace-db', third.ws, '--run', run, '--confirm', run, '--format', 'json']);
    assert(rb.status === 0, `rollback: ${rb.stderr} ${rb.stdout}`);
    const post = meridian(['export', '--kernel', ROOT, '--tool-db', third.tool, '--workspace-db', third.ws, '--format', 'json']).stdout;
    assert(post === pre, 'экспорт после rollback ≠ pre-state');
    const again = meridian(['migration', 'rollback', '--kernel', ROOT, '--source', source, '--workspace-db', third.ws, '--run', run, '--confirm', run, '--format', 'json']);
    assert(again.status === 1 && json(again).result.refusal === 'already-rolled-back', again.stdout);
  });
}

if (NAME_PATTERN) {
  runMeridianCliMigrationConformance();
  runAcceptedRustNativeDivergences();
  console.log(`\n${passed} passed, ${failures.length} failed (selective run: ${NAME_PATTERN})`);
  process.exit(failures.length ? 1 : 0);
}

// --- white-box: normalization ---

check('нормализация унифицирует CRLF/LF и не зависит от порядка строк', () => {
  const a = normalizeDiagnostics('WARN  x\r\nWARN  y\r\nWARN  x\n', '');
  assert(a.warn.length === 3, `получено ${a.warn.length} WARN вместо 3`);
  assert(a.fail.length === 0, 'FAIL не должен появиться из WARN-строк');
});

check('строка, похожая на диагностику, но не разбираемая, попадает в unparseable, а не пропадает молча', () => {
  const a = normalizeDiagnostics('FAILURE: not a real diagnostic\n', '');
  assert(a.unparseable.length === 1, 'нераспознанная FAIL-подобная строка потеряна');
  assert(a.fail.length === 0 && a.warn.length === 0, 'нераспознанная строка не должна попасть в FAIL/WARN');
});

check('обычная строка вывода, не похожая на диагностику, не считается unparseable', () => {
  const a = normalizeDiagnostics('some ordinary informational line\n', '');
  assert(a.unparseable.length === 0, 'обычная строка ошибочно помечена как нераспознанная диагностика');
});

check('диагностики читаются из stdout и stderr одинаково', () => {
  const a = normalizeDiagnostics('FAIL  from stdout\n', 'WARN  from stderr\n');
  assert(a.fail.length === 1 && a.warn.length === 1, 'один из потоков проигнорирован');
});

// --- white-box: comparison ---

check('совпадающие коды завершения и мультимножества FAIL/WARN дают match', () => {
  const cmp = compareVerdicts({ exitCode: 1, fail: ['a', 'b'], warn: [] }, { exitCode: 1, fail: ['b', 'a'], warn: [] });
  assert(cmp.match === true, 'ожидался match при совпадающем мультимножестве в разном порядке');
});

check('разные коды завершения дают расхождение', () => {
  const cmp = compareVerdicts({ exitCode: 1, fail: [], warn: [] }, { exitCode: 2, fail: [], warn: [] });
  assert(cmp.match === false && cmp.exit_code.match === false, 'ожидалось расхождение кода завершения');
});

check('отсутствующий FAIL справа обнаруживается как missing', () => {
  const cmp = compareVerdicts({ exitCode: 1, fail: ['a', 'b'], warn: [] }, { exitCode: 1, fail: ['a'], warn: [] });
  assert(cmp.match === false, 'ожидалось расхождение');
  assert(cmp.fail.missing.some((m) => m.message === 'b'), 'пропавший FAIL не найден в missing');
});

check('дополнительный WARN справа обнаруживается как added', () => {
  const cmp = compareVerdicts({ exitCode: 0, fail: [], warn: ['x'] }, { exitCode: 0, fail: [], warn: ['x', 'y'] });
  assert(cmp.warn.added.some((m) => m.message === 'y'), 'добавленный WARN не найден в added');
});

check('изменение текста внутри диагностики того же уровня — расхождение', () => {
  const cmp = compareVerdicts({ exitCode: 1, fail: ["field 'name' is required"], warn: [] }, { exitCode: 1, fail: ["field 'title' is required"], warn: [] });
  assert(cmp.match === false, 'разный текст диагностики не должен считаться совпадением');
  assert(cmp.fail.missing.length === 1 && cmp.fail.added.length === 1, 'изменённый текст должен дать ровно одну missing и одну added запись');
});

check('повторяющаяся диагностика только с одной стороны обнаруживается, а не схлопывается дедупликацией', () => {
  const cmp = compareVerdicts({ exitCode: 0, fail: [], warn: ['x', 'x'] }, { exitCode: 0, fail: [], warn: ['x'] });
  assert(cmp.match === false, 'потерянный повтор не должен считаться совпадением');
  const missing = cmp.warn.missing.find((m) => m.message === 'x');
  assert(missing && missing.left_count === 2 && missing.right_count === 1, 'счётчик повтора потерян нормализацией');
});

check('одинаковый текст с разным уровнем FAIL/WARN — расхождение в обоих множествах', () => {
  const cmp = compareVerdicts({ exitCode: 1, fail: ['z'], warn: [] }, { exitCode: 1, fail: [], warn: ['z'] });
  assert(cmp.fail.missing.some((m) => m.message === 'z'), 'смена уровня не обнаружена в FAIL');
  assert(cmp.warn.added.some((m) => m.message === 'z'), 'смена уровня не обнаружена в WARN');
});

// --- white-box: producer execution ---

check('ошибка запуска несуществующей команды даёт явный отказ, а не совпадение', () => {
  const result = runProducer({ command: 'this-command-does-not-exist-conformance-harness-probe', args: [], cwd: __dirname, env: {} });
  assert(result.ok === false, 'запуск несуществующей команды должен быть явным отказом');
  assert(result.reason === 'spawn_error', `ожидалась причина spawn_error, получена ${result.reason}`);
});

check('невалидная конфигурация запуска (пустая команда) отклоняется как отказ, а не выполняется', () => {
  const result = runProducer({ command: '', args: [], cwd: __dirname, env: {} });
  assert(result.ok === false, 'пустая команда должна быть отклонена как невалидная конфигурация');
  assert(result.reason === 'invalid_spec', `ожидалась причина invalid_spec, получена ${result.reason}`);
});

check('runProducer выполняет процесс напрямую (без shell) и раздельно захватывает код завершения, stdout и stderr', () => {
  const result = runProducer({
    command: process.execPath,
    args: ['-e', "process.stdout.write('FAIL  x\\n');process.stderr.write('WARN  y\\n');process.exit(3);"],
    cwd: __dirname,
    env: {},
  });
  assert(result.ok === true, 'штатный запуск должен завершиться успешно с точки зрения харнесса');
  assert(result.exitCode === 3, `ожидался код завершения 3, получен ${result.exitCode}`);
  assert(result.stdout.includes('FAIL  x'), 'stdout не захвачен раздельно');
  assert(result.stderr.includes('WARN  y'), 'stderr не захвачен раздельно');
});

check('runCase на несуществующей команде с одной стороны даёт harness_error, а не divergent', () => {
  const outcome = runCase({
    left: { command: process.execPath, args: ['-e', 'process.exit(0);'], cwd: __dirname, env: {} },
    right: { command: 'this-command-does-not-exist-conformance-harness-probe', args: [], cwd: __dirname, env: {} },
  });
  assert(outcome.status === 'harness_error', `ожидался harness_error, получен ${outcome.status}`);
});

check('runCase на несовпадающих вердиктах даёт divergent, и внутренний объект сравнения несёт match: false (это проверка поля, НЕ кода завершения процесса — см. чёрноящичные тесты ниже)', () => {
  const outcome = runCase({
    left: { command: process.execPath, args: ['-e', "process.stdout.write('FAIL  a\\n');process.exit(1);"], cwd: __dirname, env: {} },
    right: { command: process.execPath, args: ['-e', 'process.exit(0);'], cwd: __dirname, env: {} },
  });
  assert(outcome.status === 'divergent', `ожидался divergent, получен ${outcome.status}`);
  assert(outcome.comparison.match === false, 'сравнение внутри divergent-исхода не должно сообщать match');
});

// --- white-box: public aggregation policy (runConformanceCheck) ---

check('runConformanceCheck игнорирует expected_status: совпадающая пара даёт conformant:true', () => {
  const result = runConformanceCheck(singleCaseFixture('identical-exit-and-diagnostics'));
  assert(result.conformant === true, 'совпадающая пара должна дать conformant: true');
  assert(result.results.length === 1 && result.results[0].status === 'conformant', 'ожидался ровно один conformant-результат');
});

check('runConformanceCheck: намеренно расходящаяся пара (с expected_status: "divergent" в самой фикстуре) всё равно даёт conformant:false — expected_status не читается', () => {
  const fixturePath = singleCaseFixture('intentional-divergence-proof');
  const declaredExpectedStatus = JSON.parse(fs.readFileSync(fixturePath, 'utf8')).cases[0].expected_status;
  assert(declaredExpectedStatus === 'divergent', 'этот тест предполагает, что фикстура сама объявляет expected_status: "divergent"');
  const result = runConformanceCheck(fixturePath);
  assert(result.conformant === false, 'реальное расхождение не должно превращаться в conformant:true только потому, что фикстура его "ожидала"');
});

check('runConformanceCheck: harness_error (ошибка запуска производителя) даёт conformant:false', () => {
  const result = runConformanceCheck(singleCaseFixture('spawn-error-on-right'));
  assert(result.conformant === false, 'ошибка запуска производителя не должна давать conformant:true');
  assert(result.results[0].status === 'harness_error', `ожидался harness_error, получен ${result.results[0].status}`);
});

// --- black-box: the public CLI as a REAL child process — the actual exit code, not an in-memory field ---

check('чёрный ящик: публичный CLI на совпадающей паре завершается кодом процесса 0', () => {
  const r = runCli(singleCaseFixture('identical-exit-and-diagnostics'));
  assert(r.status === 0, `ожидался код завершения процесса 0, получен ${r.status}; stdout:\n${r.stdout}`);
});

check('чёрный ящик: публичный CLI на намеренно расходящейся паре завершается НЕНУЛЕВЫМ кодом процесса, даже когда фикстура сама объявляет expected_status: "divergent"', () => {
  const fixturePath = singleCaseFixture('intentional-divergence-proof');
  const declaredExpectedStatus = JSON.parse(fs.readFileSync(fixturePath, 'utf8')).cases[0].expected_status;
  assert(declaredExpectedStatus === 'divergent', 'этот тест предполагает, что фикстура сама объявляет expected_status: "divergent"');
  const r = runCli(fixturePath);
  assert(r.status !== 0, `expected_status: "divergent" не должен превращать реальное расхождение в успешный код процесса; получен ${r.status}`);
});

check('чёрный ящик: публичный CLI при ошибке запуска производителя завершается ненулевым кодом процесса', () => {
  const r = runCli(singleCaseFixture('spawn-error-on-right'));
  assert(r.status !== 0, `ошибка запуска производителя должна давать ненулевой код завершения процесса, получен ${r.status}`);
});

check('чёрный ящик: повторный запуск публичного CLI на одном и том же входе детерминирован', () => {
  const fixturePath = singleCaseFixture('same-diagnostics-different-order');
  const first = runCli(fixturePath);
  const second = runCli(fixturePath);
  assert(first.status === second.status && first.stdout === second.stdout,
    'повторный запуск публичного CLI на тех же входах дал другой результат');
});

// --- self-check: full corpus through the exact same path CI/pre-push use, via the test file ---

const rustProducersBuild = spawnSync(
  'cargo',
  ['build', '-p', 'meridian-app', '-p', 'meridian-cli', '--bins', '--examples'],
  { cwd: ROOT, encoding: 'utf8' },
);
check('реальные Rust-производители собраны перед сравнением', () => {
  assert(rustProducersBuild.status === 0,
    `cargo build завершился кодом ${rustProducersBuild.status}; stderr:\n${rustProducersBuild.stderr}`);
});

const corpusResults = runCorpus(FIXTURES);

check('контролируемый корпус содержит хотя бы один совпадающий, один расходящийся и один harness_error случай', () => {
  assert(corpusResults.some((r) => r.expected_status === 'conformant'), 'нет совпадающего случая (conformant)');
  assert(corpusResults.some((r) => r.expected_status === 'divergent'), 'нет расходящегося случая (divergent)');
  assert(corpusResults.some((r) => r.expected_status === 'harness_error'), 'нет случая явного отказа харнесса (harness_error)');
});

for (const result of corpusResults) {
  check(`контролируемый корпус: ${result.name} → ${result.expected_status}`, () => {
    assert(result.ok, `ожидалось ${result.expected_status}, получено ${result.actual_status}`);
  });
}

check('повторный прогон контролируемого корпуса на тех же входах детерминирован', () => {
  const again = runCorpus(FIXTURES);
  assert(JSON.stringify(again) === JSON.stringify(corpusResults), 'повторный прогон дал другой результат на тех же входах');
});

// `rust-architecture-conformance-7` ported the last four mandatory gate
// families (7d) and removed `BLOCKED_CHECKS`, so on the clean checkout the
// real Node reference and the real `meridian validate` now agree on
// EVERYTHING — exit code included. A bare "conformant" match already
// implies this; the exact shape is asserted too, so a regression that
// reintroduced a Rust-only non-zero exit could not hide.
check('real-node-rust-cli-validate-clean-kernel: Node и Rust совпадают полностью — код завершения 0 на обеих сторонах, FAIL/WARN-множества равны', () => {
  const result = corpusResults.find((r) => r.name === 'real-node-rust-cli-validate-clean-kernel');
  assert(result, 'fixture case real-node-rust-cli-validate-clean-kernel not found');
  const comparison = result.detail.comparison;
  assert(comparison.exit_code.match === true, `ожидалось совпадение кода завершения, получено Node ${comparison.exit_code.left} vs Rust ${comparison.exit_code.right}`);
  assert(comparison.exit_code.left === 0 && comparison.exit_code.right === 0, `ожидался код 0 на обеих сторонах, получено Node ${comparison.exit_code.left} vs Rust ${comparison.exit_code.right}`);
  assert(comparison.fail.missing.length === 0 && comparison.fail.added.length === 0, `FAIL-множества должны совпадать полностью: ${JSON.stringify(comparison.fail)}`);
  assert(comparison.warn.missing.length === 0 && comparison.warn.added.length === 0, `WARN-множества должны совпадать полностью: ${JSON.stringify(comparison.warn)}`);
});

// --- package meridian-cli-foundation, item 2: real Node/Rust `validate`
// comparison on a *mutated* Kernel tree, not only the real, currently-clean
// checkout above (real-node-rust-cli-validate-clean-kernel). A full copy of
// this repository (without `.git`/`target`) is made once, both producers are
// compared against it unmodified (still expected conformant — proves the
// copy itself introduces no spurious divergence), then exactly one
// kernel-purity-triggering mutation is applied and both producers are
// compared again: they must agree that the SAME new FAIL now exists. Copying
// without `.git` is deliberate, not an oversight — both `gitTrackedFiles()`
// (Node) and `list_git_tracked_files` (Rust) then fall back to the same
// full-filesystem-walk path identically on both sides, which is itself
// already proven consistent by the clean-checkout case above. ---

function copyRepoWithoutGitOrTarget(destination) {
  fs.cpSync(ROOT, destination, {
    recursive: true,
    filter: (src) => {
      const rel = path.relative(ROOT, src);
      return rel !== '.git' && rel !== 'target' && !rel.startsWith(`.git${path.sep}`) && !rel.startsWith(`target${path.sep}`);
    },
  });
}

const VALIDATE_CLI_PRODUCER = path.join(ROOT, 'target', 'debug', 'examples', 'validate_cli_producer');

function mutatedKernelValidateCase(name, kernelDir) {
  return {
    name,
    left: { command: process.execPath, args: [path.join(ROOT, 'scripts', 'kernel-validate.mjs')], cwd: ROOT, env: { MERIDIAN_KERNEL: kernelDir } },
    // `validate_cli_producer` spawns the real compiled `meridian validate`
    // binary itself and only reformats that subprocess's own real JSON
    // stdout — it never calls collect_diagnostics or any other library
    // function (meridian-cli/examples/validate_cli_producer.rs).
    right: { command: VALIDATE_CLI_PRODUCER, args: [kernelDir], cwd: ROOT, env: {} },
  };
}

// One family per already-ported check ("Ported for real, in full" —
// meridian-cli/src/commands/validate/mod.rs's module doc): a single
// negative mutation applied to an otherwise-clean full copy of this
// repository, proving the real Node.js reference and the real compiled
// `meridian` binary report the exact same new diagnostic for that family —
// not merely that each one's own unit tests pass in isolation
// (meridian-cli/tests/binary_runs.rs already proves the Rust side alone;
// this proves cross-language agreement). `rule-resolution` and
// `git-provenance` are exercised by the dedicated corpora above
// (real-node-rust-cli-resolve, real-node-rust-rule-resolution) and by
// git-provenance's own narrow "no Instance configured" advisory respectively
// — not repeated here as file-content mutations.
const VALIDATE_MUTATION_FAMILIES = [
  {
    id: 'kernel-purity',
    // Built from fragments, like the pattern it is meant to trip: this
    // literal must never appear unbroken in this source file either.
    write: (dir) => {
      const personalPath = ['C:', '\\', 'Users', '\\', 'mutation-probe'].join('');
      fs.writeFileSync(path.join(dir, 'mutation-probe.md'), `leaked path: ${personalPath}\\secret\n`);
      return [path.join(dir, 'mutation-probe.md')];
    },
    expectedFailPrefix: 'kernel-purity: personal home path',
  },
  {
    id: 'document-identity',
    write: (dir) => {
      fs.writeFileSync(path.join(dir, 'MutationNotKebabCase.txt'), 'content\n');
      return [path.join(dir, 'MutationNotKebabCase.txt')];
    },
    expectedFailPrefix: 'document-identity:',
  },
  {
    // rust-business-contract-qualification, production panic audit: an
    // empty Front Matter block (`---\n---`) made Rust's former
    // `&text[4..end]` a reversed byte range, so `meridian validate`
    // panicked (exit 101, no stdout) where the reference's `slice(4, 3)` is
    // `''` and it reports the missing fields. Both `document-identity` and
    // `agent-instruction-identity` read this block
    // (meridian_app::source_format::front_matter_block).
    id: 'document-identity-empty-front-matter',
    write: (dir) => {
      fs.writeFileSync(path.join(dir, 'empty-front-matter-probe.md'), '---\n---\n# probe\n');
      return [path.join(dir, 'empty-front-matter-probe.md')];
    },
    expectedFailPrefix: 'document-identity:',
  },
  {
    id: 'duplicate-fm',
    write: (dir) => {
      fs.writeFileSync(
        path.join(dir, 'mutation-duplicate-fm.md'),
        '---\ntitle: Doc\nstatus: draft\nscope: workspace\nowner: workspace-owner\ncreated: 2026-01-01\nupdated: 2026-01-01\ndocument_type: unclassified\nunclassified_reason: test fixture\n---\n\nBody text.\n\n---\ntitle: leaked\n---\n',
      );
      return [path.join(dir, 'mutation-duplicate-fm.md')];
    },
    expectedFailPrefix: 'duplicate-fm:',
  },
  {
    id: 'link-check',
    write: (dir) => {
      fs.writeFileSync(
        path.join(dir, 'mutation-link-check.md'),
        'See [missing](./does-not-exist-mutation-probe.md) for details.\n',
      );
      return [path.join(dir, 'mutation-link-check.md')];
    },
    expectedFailPrefix: 'link:',
  },
  {
    id: 'registry-schema',
    write: (dir) => {
      fs.writeFileSync(
        path.join(dir, 'mutation-schema.json'),
        '{"$schema":"http://json-schema.org/draft-07/schema#","type":"object","required":["name"],"properties":{"name":{"type":"string"}}}',
      );
      fs.writeFileSync(path.join(dir, 'mutation-data.yaml'), '$schema: ./mutation-schema.json\nage: 5\n');
      return [path.join(dir, 'mutation-schema.json'), path.join(dir, 'mutation-data.yaml')];
    },
    expectedFailPrefix: 'schema:',
  },
  {
    // rust-business-contract-qualification: the strictness layer (RFC
    // decision D-A) on the real gate path. The reference walks the whole
    // schema tree under the schema file's own resolved path
    // (`assertSupportedDeep(schema, schemaPath)`); Rust had located the
    // unsupported construct under `/` (`at //properties/m`) and kept `./`
    // in the schema path.
    id: 'registry-schema-unsupported-keyword-and-format',
    write: (dir) => {
      fs.writeFileSync(
        path.join(dir, 'mutation-pp-schema.json'),
        '{"$schema":"http://json-schema.org/draft-07/schema#","type":"object","patternProperties":{"^x":{"type":"string"}}}',
      );
      fs.writeFileSync(path.join(dir, 'mutation-pp-data.yaml'), '$schema: ./mutation-pp-schema.json\nxa: 1\n');
      fs.writeFileSync(
        path.join(dir, 'mutation-fmt-schema.json'),
        '{"$schema":"http://json-schema.org/draft-07/schema#","type":"object","properties":{"m":{"type":"string","format":"email"}}}',
      );
      fs.writeFileSync(path.join(dir, 'mutation-fmt-data.yaml'), '$schema: ./mutation-fmt-schema.json\nm: a\n');
      return ['mutation-pp-schema.json', 'mutation-pp-data.yaml', 'mutation-fmt-schema.json', 'mutation-fmt-data.yaml']
        .map((name) => path.join(dir, name));
    },
    expectedFailPrefix: 'schema:',
  },
  {
    // rust-business-contract-qualification: every rejecting YAML source of
    // the shared source-format corpus, through the real gate path, where the
    // rejection TEXT is observable (the corpus itself compares only
    // accepted/rejected). Rust had handed the input to the library before
    // its own strict line lint (RFC decision D-A), so tab indentation and a
    // second document carried the library's wording.
    id: 'registry-yaml-strict-lint-rejections',
    write: (dir) => {
      const corpus = JSON.parse(fs.readFileSync(path.join(ROOT, 'verification', 'conformance-harness', 'fixtures', 'source-format-corpus.json'), 'utf8'));
      const rejecting = corpus.yaml.filter((testCase) => testCase.name.startsWith('reject-'));
      assert(rejecting.length === 5, `ожидалось 5 отклоняющих YAML-случаев корпуса, получено ${rejecting.length}`);
      // Plus the subset reader's own flow-collection rejections, which the
      // library used to pre-empt with its "unclosed bracket" wording.
      const cases = [
        ...rejecting,
        { name: 'reject-unclosed-flow-sequence', source: 'a: [1, 2\n' },
        { name: 'reject-unclosed-flow-mapping', source: 'a: {b: 1\n' },
      ];
      return cases.map((testCase) => {
        const file = path.join(dir, `mutation-${testCase.name}.yaml`);
        fs.writeFileSync(file, testCase.source);
        return file;
      });
    },
    expectedFailPrefix: 'schema: cannot parse',
  },
];

{
  const mutationDir = fs.mkdtempSync(path.join(os.tmpdir(), 'conformance-harness-kernel-validate-mutation-'));
  createdTempDirs.push(mutationDir);
  try {
    copyRepoWithoutGitOrTarget(mutationDir);

    check('реальный Node/Rust validate на копии текущего дерева без .git совпадает (без мутации)', () => {
      const result = runCase(mutatedKernelValidateCase('cli-validate-copy-baseline', mutationDir));
      assert(result.status === 'conformant', `ожидался conformant, получено ${result.status}: ${JSON.stringify(result)}`);
    });

    for (const family of VALIDATE_MUTATION_FAMILIES) {
      const writtenPaths = family.write(mutationDir);
      try {
        check(`реальный Node/Rust validate: мутация "${family.id}" даёт один и тот же новый FAIL на обеих сторонах`, () => {
          const result = runCase(mutatedKernelValidateCase(`cli-validate-mutation-${family.id}`, mutationDir));
          assert(
            result.status === 'conformant',
            `ожидался conformant (оба сообщают один и тот же новый FAIL для "${family.id}"), получено ${result.status}: ${JSON.stringify(result)}`,
          );
        });
      } finally {
        for (const writtenPath of writtenPaths) fs.rmSync(writtenPath, { force: true });
      }
    }
  } finally {
    fs.rmSync(mutationDir, { recursive: true, force: true });
    createdTempDirs.splice(createdTempDirs.indexOf(mutationDir), 1);
  }
}

// One family per check ported by subpackage `validate-mechanical-integrity`
// (package 7, subpackage 7a — meridian-cli/src/commands/validate/mod.rs's
// module doc): a single negative mutation on its own full, fresh copy of
// this repository, proving the real Node.js reference and the real compiled
// `meridian` binary report the exact same new diagnostic. A dedicated copy
// per family (rather than the single shared `mutationDir` the five families
// above reuse) is used here because three of these five mutations *modify*
// an existing Kernel pool file in place — a shared-directory cleanup that
// only removes newly-created paths cannot restore a modified file for the
// next family's turn, and a fresh copy per family sidesteps that instead of
// adding restore logic to the loop above.
const VALIDATE_MUTATION_FAMILIES_7A = [
  {
    id: 'sha-provenance',
    write: (dir) => {
      fs.mkdirSync(path.join(dir, 'skills', 'mutation-probe-skill'), { recursive: true });
      fs.writeFileSync(path.join(dir, 'skills', 'mutation-probe-skill', 'SKILL.md'), 'mutation probe skill\n');
      fs.writeFileSync(
        path.join(dir, 'skills', 'mutation-probe-skill', 'PIN.yaml'),
        'artifact: SKILL.md\nsha256: deadbeef000000000000000000000000000000000000000000000000000000000000\n',
      );
    },
    expectedFailPrefix: 'sha-provenance:',
  },
  {
    id: 'instruction-topics',
    write: (dir) => {
      const file = path.join(dir, 'standards', 'workspace', 'instruction-topics.yaml');
      fs.appendFileSync(file, '  - mutation-probe-topic\n');
    },
    expectedFailPrefix: 'instruction-topics:',
  },
  {
    id: 'operating-foundation',
    write: (dir) => {
      const file = path.join(dir, 'standards', 'workspace', 'operating-foundation.yaml');
      const text = fs.readFileSync(file, 'utf8');
      const marker = '\nprinciples:';
      const idx = text.indexOf(marker);
      assert(idx !== -1, 'operating-foundation.yaml must contain a top-level "principles:" key to mutate before');
      const insertion = '  - id: mutation-probe-term\n    canonical_ru: Мутация\n    canonical_en: Mutation\n';
      fs.writeFileSync(file, text.slice(0, idx + 1) + insertion + text.slice(idx + 1));
    },
    expectedFailPrefix: 'operating-foundation:',
  },
  {
    id: 'stack-profiles',
    write: (dir) => {
      const file = path.join(dir, 'stack-profiles', 'stack-profiles.yaml');
      fs.appendFileSync(file, '  - name: mutation-probe-profile\n    manifest: package.json\n');
    },
    expectedFailPrefix: 'stack-profiles:',
  },
  {
    id: 'agent-instruction-identity',
    write: (dir) => {
      fs.writeFileSync(
        path.join(dir, 'mutation-probe-instruction-identity.md'),
        '---\ntitle: Mutation probe\nstatus: draft\nscope: workspace\nowner: workspace-owner\ncreated: 2026-01-01\nupdated: 2026-01-01\ndocument_type: unclassified\nunclassified_reason: test fixture\ntopic: agent-conduct\n---\n\nBody text.\n',
      );
    },
    expectedFailPrefix: 'agent-instruction-identity:',
  },
  {
    // rust-business-contract-qualification: the `rule-resolution` family of
    // `validate` (PHASE B schemas exercised against their bundled fixtures)
    // had no negative Node/Rust case — the resolver corpora exercise the
    // resolver, not this fixture check. One must-be-valid fixture of each
    // group is declared invalid and one must-be-invalid fixture valid, so
    // both the "validated clean" and the first-schema-error texts compare.
    id: 'rule-resolution-fixtures',
    write: (dir) => {
      const p = path.join(dir, 'registries', 'rule-resolution', 'fixtures', 'rule-resolution.fixtures.json');
      const bundle = JSON.parse(fs.readFileSync(p, 'utf8'));
      assert(Array.isArray(bundle) && bundle.length === 2, 'rule-resolution.fixtures.json must carry the two PHASE B groups');
      for (const group of bundle) {
        const wasValid = group.valid[0];
        const wasInvalid = group.invalid[0];
        group.valid[0] = wasInvalid;
        group.invalid[0] = wasValid;
      }
      fs.writeFileSync(p, JSON.stringify(bundle));
    },
    expectedFailPrefix: 'rule-resolution:',
  },
];

for (const family of VALIDATE_MUTATION_FAMILIES_7A) {
  const mutationDir = fs.mkdtempSync(path.join(os.tmpdir(), `conformance-harness-kernel-validate-mutation-7a-${family.id}-`));
  createdTempDirs.push(mutationDir);
  try {
    copyRepoWithoutGitOrTarget(mutationDir);
    family.write(mutationDir);
    check(`реальный Node/Rust validate: мутация "${family.id}" даёт один и тот же новый FAIL на обеих сторонах (подпакет 7a)`, () => {
      const result = runCase(mutatedKernelValidateCase(`cli-validate-mutation-7a-${family.id}`, mutationDir));
      assert(
        result.status === 'conformant',
        `ожидался conformant (оба сообщают один и тот же новый FAIL для "${family.id}"), получено ${result.status}: ${JSON.stringify(result)}`,
      );
    });
  } finally {
    fs.rmSync(mutationDir, { recursive: true, force: true });
    createdTempDirs.splice(createdTempDirs.indexOf(mutationDir), 1);
  }
}

// ---------------------------------------------------------------------------
// Package meridian-cli-foundation, subpackage validate-operating-contracts
// (package 7, subpackage 7b), corrective round: one negative mutation per
// one of the seven families this subpackage ported, each on its own full,
// fresh copy of this repository (same reason as VALIDATE_MUTATION_FAMILIES_7A
// — several of these mutate an existing Kernel file in place).
//
// The previous round's five fixtures-truncation mutations (emptying a
// schema's bundled "invalid" array) only ever exercised the WIRING code's
// own bundle-shape check ("the fixtures file has no non-empty ... array") —
// never the family's actual bespoke composite algorithm
// (`evaluate*`/`check*` in `meridian_app::operating_model::*`), which is
// what this package's whole purpose is to port. This round replaces all
// five with a SCHEMA-VALID mutation of one bundled "valid" fixture's own
// document, chosen so it violates exactly one composite-only rule the JSON
// Schema itself cannot state (never a `uniqueItems`/type/enum violation the
// generic `$schema` pass would already catch on its own, which would leave
// it ambiguous which layer actually caught the mutation). `task-pattern-registry`
// and `role-and-human-control` keep their existing real-Kernel-data
// mutations (`standards/workspace/{task-pattern-registry,role-registry}.yaml`)
// unchanged — they already exercise their own composite algorithms, not
// merely the wiring code.
//
// Each mutation is verified against a BASELINE computed on the SAME
// unmutated copy immediately before mutating it — not the generic
// corpus-wide `conformant`/`divergent` harness, whose full-diagnostic-set
// comparison is fragile against anything else in the tree changing (a
// lesson learned the hard way earlier in this same round: duplicating
// task-pattern-registry.yaml's whole list once collided every real pattern
// id, cascading into task-specification-contract's and — worse —
// upgrade-integration-qualification's own
// fixtures, an unavoidable, permanent `divergent` that had nothing to do
// with task-pattern-registry itself; see `duplicateOneTaskPatternWithNewId`
// below for the narrower mutation that replaced it).
//
// The check itself (`assertMutationDeltaMultisetsEqual`) does not stop
// at "the two sides intersect somewhere" — a corrective round tightened it
// after that weaker form let a real ordering bug through undetected (see
// `governance/plans/meridian-rust-migration-program-plan.md` §5.9 pt. 1). It
// computes the full multiset DELTA (`multisetDelta`) of the mutated run over
// the baseline on each side — every new line, WITH its repeat count, not
// merely which distinct strings are new — and asserts the delta is
// non-empty, carries the family's `expectedFailPrefix`, and is EQUAL AS A
// MULTISET between Node and Rust: the exact same lines, the exact same
// number of times each, never merely an intersecting subset. This proves
// the underlying composite algorithm agrees completely on what changed, not
// just that "something new, and coincidentally one same line," failed on
// both.
function computeFailLines(dir) {
  const nodeRun = spawnSync(process.execPath, [path.join(ROOT, 'scripts', 'kernel-validate.mjs')], {
    cwd: ROOT,
    env: kernelOnlyEnv({ MERIDIAN_KERNEL: dir }),
    encoding: 'utf8',
  });
  const nodeFails = nodeRun.stdout
    .split('\n')
    .filter((l) => l.startsWith('FAIL'))
    .map((l) => l.replace(/^FAIL\s+/, ''));
  const meridianBin = path.join(ROOT, 'target', 'debug', process.platform === 'win32' ? 'meridian.exe' : 'meridian');
  const rustRun = spawnSync(meridianBin, ['validate', '--kernel', dir, '--format', 'json'], {
    encoding: 'utf8',
    env: kernelOnlyEnv({ MERIDIAN_KERNEL: dir }),
  });
  let rustFails;
  try {
    rustFails = JSON.parse(rustRun.stdout.trim()).result.failures;
  } catch (e) {
    throw new Error(
      `expected exactly one JSON result document on the real meridian binary's stdout, got: ${JSON.stringify(rustRun.stdout)} (${e.message}); stderr: ${rustRun.stderr}`,
    );
  }
  return { nodeFails, rustFails };
}

// A multiset (bag) of diagnostic lines, counting repeats — a mutation that
// duplicates an already-present line, or that a family's own composite
// algorithm reports once per affected fixture, must be compared by COUNT,
// not merely by which distinct strings appear.
function multisetCounts(lines) {
  const counts = new Map();
  for (const line of lines) counts.set(line, (counts.get(line) ?? 0) + 1);
  return counts;
}

function multisetsEqual(a, b) {
  const ca = multisetCounts(a);
  const cb = multisetCounts(b);
  if (ca.size !== cb.size) return false;
  for (const [line, count] of ca) {
    if (cb.get(line) !== count) return false;
  }
  return true;
}

// The multiset DELTA of `mutated` over `baseline`: for each distinct line,
// however many more times it appears in `mutated` than in `baseline`
// (never negative — a line the mutation made LESS frequent is not part of
// what the mutation introduced). This is the full set of what changed,
// not merely the lines that happen to also carry a chosen prefix.
function multisetDelta(baseline, mutated) {
  const baselineCounts = multisetCounts(baseline);
  const mutatedCounts = multisetCounts(mutated);
  const delta = [];
  for (const [line, mutatedCount] of mutatedCounts) {
    const extra = mutatedCount - (baselineCounts.get(line) ?? 0);
    for (let i = 0; i < extra; i += 1) delta.push(line);
  }
  return delta;
}

function assertMutationDeltaMultisetsEqual(dir, expectedFailPrefix, label) {
  const baseline = computeFailLines(dir);
  return (mutate) => {
    mutate(dir);
    const mutated = computeFailLines(dir);
    const nodeDelta = multisetDelta(baseline.nodeFails, mutated.nodeFails);
    const rustDelta = multisetDelta(baseline.rustFails, mutated.rustFails);
    assert(
      nodeDelta.length > 0,
      `${label}: expected a non-empty Node delta over baseline; baseline=${JSON.stringify(baseline.nodeFails)} mutated=${JSON.stringify(mutated.nodeFails)}`,
    );
    assert(
      rustDelta.length > 0,
      `${label}: expected a non-empty Rust delta over baseline; baseline=${JSON.stringify(baseline.rustFails)} mutated=${JSON.stringify(mutated.rustFails)}`,
    );
    assert(
      nodeDelta.some((l) => l.startsWith(expectedFailPrefix)),
      `${label}: expected the Node delta to carry a line with prefix "${expectedFailPrefix}", got delta: ${JSON.stringify(nodeDelta)}`,
    );
    assert(
      rustDelta.some((l) => l.startsWith(expectedFailPrefix)),
      `${label}: expected the Rust delta to carry a line with prefix "${expectedFailPrefix}", got delta: ${JSON.stringify(rustDelta)}`,
    );
    assert(
      multisetsEqual(nodeDelta, rustDelta),
      `${label}: expected the Node and Rust deltas to be equal as multisets (same lines, same counts), not merely intersecting; node-delta=${JSON.stringify(nodeDelta)} rust-delta=${JSON.stringify(rustDelta)}`,
    );
  };
}

// Removes exactly ONE occurrence of `target` from `lines`, asserting it was
// actually present — never a silent no-op, so a caller that expected the
// line to already be there finds out immediately if it is not.
function removeOneOccurrence(lines, target) {
  const idx = lines.indexOf(target);
  assert(
    idx !== -1,
    `expected to find exactly one occurrence of ${JSON.stringify(target)} to remove; not found in ${JSON.stringify(lines)}`,
  );
  const copy = lines.slice();
  copy.splice(idx, 1);
  return copy;
}

// The ONE accepted, documented Rust-native divergence this mutation
// produces (`COMPATIBILITY.md`, "Намеренные Rust-native усиления",
// `rust-architecture-conformance-3` row): Node's `task-specification-contract`
// block (`scripts/kernel-validate.mjs`, ~line 1225) re-reads and re-parses
// `task-pattern-registry.yaml` directly, INDEPENDENT of whether the earlier
// `task-pattern-registry` block's own check (`tprOk`) succeeded — a
// duplicate pattern id elsewhere in the file never stops it from resolving
// the `task_pattern` references its own fixtures use. Rust's
// `task_pattern_registry::evaluate` only ever publishes a
// `TaskPatternCatalog` once EVERY catalogue-level rule has already passed
// (`rust-architecture-conformance-3` corrective round item 1: fail-closed
// `TaskPatternCatalog::build` — a duplicate id anywhere yields `catalog:
// None`, not a partially-built one), and `task_specification`'s own CLI
// wrapper (`meridian-cli/src/commands/validate/task_specification.rs`)
// reports a dependent `"task-specification-contract: no task-pattern
// catalog is available; task-pattern-registry must be checked first"`
// whenever that catalog is absent — a second, structurally coupled
// diagnostic Node's independent re-parse can never produce, since Node has
// no equivalent shared-catalogue gate to fail closed on.
const TASK_SPECIFICATION_NO_CATALOG_LINE =
  'task-specification-contract: no task-pattern catalog is available; task-pattern-registry must be checked first';

// `rust-architecture-conformance-7`: `upgrade-integration-qualification`
// composes task specifications against the SAME published catalogue, so it
// fails closed the same way (one dependent line). Node's upgrade block
// re-parses the raw YAML itself and reports nothing new. This extends the
// accepted boundary above to its second consumer — accepted by the
// architect in rust-architecture-conformance-7 (COMPATIBILITY.md).
const UPGRADE_NO_CATALOG_LINE =
  'upgrade-integration-qualification: no task-pattern catalog is available; task-pattern-registry must be checked first';

// `task-pattern-registry`'s own accepted-boundary check, replacing the
// generic `assertMutationDeltaMultisetsEqual` for this ONE family only:
// proves the `task-pattern-registry` diagnostics themselves still agree
// completely (as a multiset) between Node and Rust, that Rust's delta
// carries EXACTLY the two documented extra dependent lines above
// (`task-specification-contract` and `upgrade-integration-qualification`)
// and Node's never does, and that removing exactly those two lines from
// Rust's delta leaves the two sides identical — not merely
// "intersecting," and not a blanket relaxation that would also hide an
// unrelated real divergence in either family.
function assertTaskPatternRegistryMutationAcceptedDivergence(dir, expectedFailPrefix, label) {
  const baseline = computeFailLines(dir);
  return (mutate) => {
    mutate(dir);
    const mutated = computeFailLines(dir);
    const nodeDelta = multisetDelta(baseline.nodeFails, mutated.nodeFails);
    const rustDelta = multisetDelta(baseline.rustFails, mutated.rustFails);
    assert(
      nodeDelta.length > 0,
      `${label}: expected a non-empty Node delta over baseline; baseline=${JSON.stringify(baseline.nodeFails)} mutated=${JSON.stringify(mutated.nodeFails)}`,
    );
    assert(
      rustDelta.length > 0,
      `${label}: expected a non-empty Rust delta over baseline; baseline=${JSON.stringify(baseline.rustFails)} mutated=${JSON.stringify(mutated.rustFails)}`,
    );
    assert(
      nodeDelta.some((l) => l.startsWith(expectedFailPrefix)),
      `${label}: expected the Node delta to carry a line with prefix "${expectedFailPrefix}", got delta: ${JSON.stringify(nodeDelta)}`,
    );
    assert(
      rustDelta.some((l) => l.startsWith(expectedFailPrefix)),
      `${label}: expected the Rust delta to carry a line with prefix "${expectedFailPrefix}", got delta: ${JSON.stringify(rustDelta)}`,
    );
    assert(
      !nodeDelta.includes(TASK_SPECIFICATION_NO_CATALOG_LINE),
      `${label}: Node must never produce the dependent "no task-pattern catalog is available" line — its task-specification-contract block re-parses the raw catalogue independently of task-pattern-registry's own verdict; node-delta=${JSON.stringify(nodeDelta)}`,
    );
    const rustNoCatalogCount = rustDelta.filter((l) => l === TASK_SPECIFICATION_NO_CATALOG_LINE).length;
    assert(
      rustNoCatalogCount === 1,
      `${label}: expected the Rust delta to carry the dependent "no task-pattern catalog is available" line exactly once (the one accepted rust-architecture-conformance-3 divergence, COMPATIBILITY.md), found ${rustNoCatalogCount}; rust-delta=${JSON.stringify(rustDelta)}`,
    );
    assert(
      !nodeDelta.includes(UPGRADE_NO_CATALOG_LINE),
      `${label}: Node must never produce the upgrade qualification's dependent "no task-pattern catalog" line; node-delta=${JSON.stringify(nodeDelta)}`,
    );
    const rustUpgradeNoCatalogCount = rustDelta.filter((l) => l === UPGRADE_NO_CATALOG_LINE).length;
    assert(
      rustUpgradeNoCatalogCount === 1,
      `${label}: expected the Rust delta to carry the upgrade qualification's dependent "no task-pattern catalog" line exactly once (COMPATIBILITY.md, rust-architecture-conformance-7, accepted), found ${rustUpgradeNoCatalogCount}; rust-delta=${JSON.stringify(rustDelta)}`,
    );
    const rustDeltaWithoutAcceptedLine = removeOneOccurrence(
      removeOneOccurrence(rustDelta, TASK_SPECIFICATION_NO_CATALOG_LINE),
      UPGRADE_NO_CATALOG_LINE,
    );
    assert(
      multisetsEqual(nodeDelta, rustDeltaWithoutAcceptedLine),
      `${label}: after removing the TWO accepted dependent Rust lines (task-specification-contract, rust-architecture-conformance-3; upgrade-integration-qualification, rust-architecture-conformance-7), expected the Node and Rust deltas to be equal as multisets (same lines, same counts) — anything else here would be a REAL, undocumented divergence; node-delta=${JSON.stringify(nodeDelta)} rust-delta-without-accepted-lines=${JSON.stringify(rustDeltaWithoutAcceptedLine)}`,
    );
  };
}

function duplicateYamlListToEnd(filePath, marker) {
  const text = fs.readFileSync(filePath, 'utf8');
  const idx = text.indexOf(marker);
  assert(idx !== -1, `${filePath} must contain "${marker.trim()}" to mutate after`);
  const listBlock = text.slice(idx + marker.length);
  fs.writeFileSync(filePath, text + listBlock);
}

// Duplicating task-pattern-registry.yaml's whole task_patterns list (as
// duplicateYamlListToEnd would) collides EVERY real pattern id at once —
// including the seven ids task-specification-contract's and
// upgrade-integration-qualification's own bundled fixtures reference by
// name, which cascades this one mutation into composing families whose
// Rust side fails closed on an unpublished catalogue while Node re-parses
// the raw list — a divergence that has nothing to do with
// task-pattern-registry itself. This
// narrower mutation instead appends two copies of the FIRST pattern entry
// under one brand-new id ("mutation-probe-pattern") no other bundled fixture
// references by name, so the only families it can possibly disturb are
// task-pattern-registry's own duplicate-id/duplicate-classification-pair
// checks.
function duplicateOneTaskPatternWithNewId(filePath) {
  const text = fs.readFileSync(filePath, 'utf8');
  const marker = 'task_patterns:\n';
  const idx = text.indexOf(marker);
  assert(idx !== -1, `${filePath} must contain "${marker.trim()}" to mutate after`);
  const listText = text.slice(idx + marker.length);
  const parts = listText.split(/(?=^ {2}- \$schema:)/m).filter(Boolean);
  assert(parts.length >= 1, `${filePath} must carry at least one task pattern entry`);
  const first = parts[0];
  assert(
    first.includes('id: assess-existing-state'),
    `${filePath}'s first task pattern entry must be "assess-existing-state" to mutate`,
  );
  const duplicate = first.replace('id: assess-existing-state', 'id: mutation-probe-pattern');
  fs.writeFileSync(filePath, text + duplicate + duplicate);
}

// Each `write` mutates exactly one bundled "valid" fixture's own document,
// changing a value the JSON Schema itself places no constraint on (so the
// generic `$schema` pass stays silent and only the family's bespoke
// composite algorithm can object), chosen to violate exactly one composite
// rule with no side effect on any other fixture or family:
//   - functional-parity: drops one of three post-change `contract_links`
//     entries from an otherwise-complete VERIFIED record — rule 6 ("a
//     per-assertion VERIFIED needs BOTH covering evidence AND a link");
//   - instruction-source-registry: flips a verified, source-missing
//     source's `recorded_state.currency` from "stale" to "current" — a
//     source known gone cannot have a *current* snapshot;
//   - task-specification-contract: appends a second acceptance criterion
//     under a brand-new id whose statement/verification duplicate the
//     first criterion's — schema places no `uniqueItems` on
//     `acceptance_criteria` (unlike `constraints`/`resolved_norms`, which
//     do carry `uniqueItems` and were avoided for exactly that reason);
//   - execution-state-model: sets `current_actor` to a rooted POSIX path —
//     `current_actor` is schema-typed as a bare non-empty string with no
//     path-shape constraint, so only `nonPortableReason` can object;
//   - bounded-context-manifest: sets one authoritative source's `purpose`
//     to a rooted POSIX path — same reasoning, on a field with no
//     interaction with the checkpoint/resolver machinery.
function VALIDATE_MUTATION_FAMILIES_7B_WRITE_functionalParity(dir) {
  const p = path.join(
    dir,
    'verification',
    'functional-parity',
    'fixtures',
    'functional-parity-evidence.fixtures.json',
  );
  const bundle = JSON.parse(fs.readFileSync(p, 'utf8'));
  const rec = bundle[0].valid[0].doc.records[0];
  rec.post_change_evidence.contract_links = rec.post_change_evidence.contract_links.filter(
    (l) => l.assertion_id !== 'io.mapping',
  );
  fs.writeFileSync(p, JSON.stringify(bundle));
}

function VALIDATE_MUTATION_FAMILIES_7B_WRITE_instructionSourceRegistry(dir) {
  const p = path.join(
    dir,
    'registries',
    'operating-model',
    'fixtures',
    'instruction-source-registry.fixtures.json',
  );
  const bundle = JSON.parse(fs.readFileSync(p, 'utf8'));
  const c = bundle.valid.find((c) => c.note.startsWith('source reported missing'));
  assert(c, `${p} must carry a "source reported missing" valid fixture to mutate`);
  c.registry.instruction_sources[0].payload.recorded_state.currency = 'current';
  fs.writeFileSync(p, JSON.stringify(bundle));
}

function VALIDATE_MUTATION_FAMILIES_7B_WRITE_taskSpecificationContract(dir) {
  const p = path.join(dir, 'registries', 'operating-model', 'fixtures', 'task-specification.fixtures.json');
  const bundle = JSON.parse(fs.readFileSync(p, 'utf8'));
  const spec = bundle.valid[0].spec;
  const first = spec.payload.acceptance_criteria[0];
  spec.payload.acceptance_criteria.push({
    id: 'default-applied-dup',
    statement: first.statement,
    verification: { ...first.verification },
  });
  fs.writeFileSync(p, JSON.stringify(bundle));
}

function VALIDATE_MUTATION_FAMILIES_7B_WRITE_executionStateModel(dir) {
  const p = path.join(dir, 'registries', 'operating-model', 'fixtures', 'execution-state.fixtures.json');
  const bundle = JSON.parse(fs.readFileSync(p, 'utf8'));
  bundle.valid[0].spec.payload.current_actor = '/etc/passwd';
  fs.writeFileSync(p, JSON.stringify(bundle));
}

function VALIDATE_MUTATION_FAMILIES_7B_WRITE_boundedContextManifest(dir) {
  const p = path.join(dir, 'registries', 'operating-model', 'fixtures', 'context-manifest.fixtures.json');
  const bundle = JSON.parse(fs.readFileSync(p, 'utf8'));
  bundle.valid[0].spec.payload.authoritative_sources[0].purpose = '/etc/passwd';
  fs.writeFileSync(p, JSON.stringify(bundle));
}

const VALIDATE_MUTATION_FAMILIES_7B = [
  {
    id: 'functional-parity',
    write: VALIDATE_MUTATION_FAMILIES_7B_WRITE_functionalParity,
    expectedFailPrefix: 'functional-parity:',
  },
  {
    id: 'task-pattern-registry',
    write: (dir) => {
      duplicateOneTaskPatternWithNewId(
        path.join(dir, 'standards', 'workspace', 'task-pattern-registry.yaml'),
      );
    },
    expectedFailPrefix: 'task-pattern-registry:',
  },
  {
    id: 'instruction-source-registry',
    write: VALIDATE_MUTATION_FAMILIES_7B_WRITE_instructionSourceRegistry,
    expectedFailPrefix: 'instruction-source-registry:',
  },
  {
    id: 'task-specification-contract',
    write: VALIDATE_MUTATION_FAMILIES_7B_WRITE_taskSpecificationContract,
    expectedFailPrefix: 'task-specification-contract:',
  },
  {
    id: 'execution-state-model',
    write: VALIDATE_MUTATION_FAMILIES_7B_WRITE_executionStateModel,
    expectedFailPrefix: 'execution-state-model:',
  },
  {
    id: 'role-and-human-control',
    write: (dir) => {
      duplicateYamlListToEnd(
        path.join(dir, 'standards', 'workspace', 'role-registry.yaml'),
        '  roles:\n',
      );
    },
    expectedFailPrefix: 'role-and-human-control:',
  },
  {
    id: 'bounded-context-manifest',
    write: VALIDATE_MUTATION_FAMILIES_7B_WRITE_boundedContextManifest,
    expectedFailPrefix: 'bounded-context-manifest:',
  },
];

for (const family of VALIDATE_MUTATION_FAMILIES_7B) {
  const mutationDir = fs.mkdtempSync(path.join(os.tmpdir(), `conformance-harness-kernel-validate-mutation-7b-${family.id}-`));
  createdTempDirs.push(mutationDir);
  try {
    copyRepoWithoutGitOrTarget(mutationDir);
    // `task-pattern-registry` carries one documented, accepted
    // rust-architecture-conformance-3 divergence (see
    // `assertTaskPatternRegistryMutationAcceptedDivergence`'s own doc
    // comment and `COMPATIBILITY.md`): it gets its own accepted-boundary
    // check instead of the strict full-equality one every other 7b family
    // still uses unchanged.
    const isTaskPatternRegistry = family.id === 'task-pattern-registry';
    const assertDelta = isTaskPatternRegistry
      ? assertTaskPatternRegistryMutationAcceptedDivergence(mutationDir, family.expectedFailPrefix, family.id)
      : assertMutationDeltaMultisetsEqual(mutationDir, family.expectedFailPrefix, family.id);
    check(
      isTaskPatternRegistry
        ? `реальный Node/Rust validate: schema-valid мутация "task-pattern-registry" даёт равные как multiset registry-диагностики на обеих сторонах и ровно одну дополнительную ожидаемую Rust-only diagnostic "${TASK_SPECIFICATION_NO_CATALOG_LINE}" (принятое Rust-native усиление rust-architecture-conformance-3, COMPATIBILITY.md), которую Node не производит; после удаления этой единственной строки дельты равны как мультимножество (подпакет 7b)`
        : `реальный Node/Rust validate: schema-valid мутация "${family.id}" даёт полностью равные как мультимножество multiset-дельты (с учётом кратности) относительно baseline на обеих сторонах, непустые и несущие префикс "${family.expectedFailPrefix}" (подпакет 7b)`,
      () => {
        assertDelta(family.write);
      },
    );
  } finally {
    fs.rmSync(mutationDir, { recursive: true, force: true });
    createdTempDirs.splice(createdTempDirs.indexOf(mutationDir), 1);
  }
}

// Each `write` mutates exactly one bundled "valid" fixture's own document,
// changing a value the JSON Schema itself places no constraint on (or an
// enum member the schema accepts but the composite algorithm's own
// recomputation rejects), chosen to violate exactly one composite rule with
// no side effect on any other fixture or family (subpackage 7c):
//   - evidence-and-handoff-contract: sets `payload.outcome.statement` to a
//     rooted POSIX path — `outcome.statement` is schema-typed as a bare
//     non-empty string with no path-shape constraint, so only
//     `nonPortableReason` can object;
//   - meridian-field-evaluation: sets an "observed" classification
//     observation's `measurement.basis` to a rooted POSIX path — same
//     reasoning, on the classification-basis field;
//   - controlled-rule-intake: appends `-tampered` to a candidate's
//     `origin.source_ref`, still a well-formed portable string the schema
//     accepts, but no longer the canonical `instruction-source:<id>`
//     mapping `checkOriginLink` requires (property 2/9);
//   - existing-project-compatibility-mode: changes a connection's
//     `payload.next_step` from the value its (empty) `findings` set
//     actually computes to a DIFFERENT schema-valid enum member —
//     `computeNextStep`'s closed priority is recomputed and compared
//     exactly, not merely checked for enum membership.
function VALIDATE_MUTATION_FAMILIES_7C_WRITE_evidenceAndHandoff(dir) {
  const p = path.join(dir, 'registries', 'operating-model', 'fixtures', 'evidence-and-handoff.fixtures.json');
  const bundle = JSON.parse(fs.readFileSync(p, 'utf8'));
  bundle.valid[0].spec.payload.outcome.statement = '/etc/passwd';
  fs.writeFileSync(p, JSON.stringify(bundle));
}

function VALIDATE_MUTATION_FAMILIES_7C_WRITE_fieldEvaluation(dir) {
  const p = path.join(dir, 'registries', 'operating-model', 'fixtures', 'field-evaluation.fixtures.json');
  const bundle = JSON.parse(fs.readFileSync(p, 'utf8'));
  const c = bundle.valid.find((c) => c.note === 'наблюдение obs-mech-correct-1');
  assert(c, `${p} must carry a "наблюдение obs-mech-correct-1" valid fixture to mutate`);
  c.spec.payload.measurement.basis = '/etc/passwd';
  fs.writeFileSync(p, JSON.stringify(bundle));
}

function VALIDATE_MUTATION_FAMILIES_7C_WRITE_controlledRuleIntake(dir) {
  const p = path.join(dir, 'registries', 'operating-model', 'fixtures', 'controlled-rule-intake.fixtures.json');
  const bundle = JSON.parse(fs.readFileSync(p, 'utf8'));
  bundle.valid[0].registry.rule_candidates[0].origin.source_ref += '-tampered';
  fs.writeFileSync(p, JSON.stringify(bundle));
}

function VALIDATE_MUTATION_FAMILIES_7C_WRITE_existingProjectCompatibilityMode(dir) {
  const p = path.join(dir, 'registries', 'operating-model', 'fixtures', 'existing-project-compatibility-mode.fixtures.json');
  const bundle = JSON.parse(fs.readFileSync(p, 'utf8'));
  const conn = bundle.valid[0].registry.workspace_connections[0];
  assert(
    conn.payload.next_step === 'continue-compatibility-mode',
    `${p} valid[0]'s first connection must have next_step "continue-compatibility-mode" to mutate`,
  );
  conn.payload.next_step = 'resolve-conflict';
  fs.writeFileSync(p, JSON.stringify(bundle));
}

const VALIDATE_MUTATION_FAMILIES_7C = [
  {
    id: 'evidence-and-handoff-contract',
    write: VALIDATE_MUTATION_FAMILIES_7C_WRITE_evidenceAndHandoff,
    expectedFailPrefix: 'evidence-and-handoff-contract:',
  },
  {
    id: 'meridian-field-evaluation',
    write: VALIDATE_MUTATION_FAMILIES_7C_WRITE_fieldEvaluation,
    expectedFailPrefix: 'meridian-field-evaluation:',
  },
  {
    id: 'controlled-rule-intake',
    write: VALIDATE_MUTATION_FAMILIES_7C_WRITE_controlledRuleIntake,
    expectedFailPrefix: 'controlled-rule-intake:',
  },
  {
    id: 'existing-project-compatibility-mode',
    write: VALIDATE_MUTATION_FAMILIES_7C_WRITE_existingProjectCompatibilityMode,
    expectedFailPrefix: 'existing-project-compatibility-mode:',
  },
];

for (const family of VALIDATE_MUTATION_FAMILIES_7C) {
  const mutationDir = fs.mkdtempSync(path.join(os.tmpdir(), `conformance-harness-kernel-validate-mutation-7c-${family.id}-`));
  createdTempDirs.push(mutationDir);
  try {
    copyRepoWithoutGitOrTarget(mutationDir);
    const assertDeltaMultisetsEqual = assertMutationDeltaMultisetsEqual(
      mutationDir,
      family.expectedFailPrefix,
      family.id,
    );
    check(
      `реальный Node/Rust validate: schema-valid мутация "${family.id}" даёт полностью равные как мультимножество multiset-дельты (с учётом кратности) относительно baseline на обеих сторонах, непустые и несущие префикс "${family.expectedFailPrefix}" (подпакет 7c)`,
      () => {
        assertDeltaMultisetsEqual(family.write);
      },
    );
  } finally {
    fs.rmSync(mutationDir, { recursive: true, force: true });
    createdTempDirs.splice(createdTempDirs.indexOf(mutationDir), 1);
  }
}

// ---------------------------------------------------------------------------
// `rust-architecture-conformance-7`: the four migration/qualification
// families (historical 7d) through the same real Node/Rust `validate`
// multiset-delta comparison. Each `write` mutates one bundled VALID
// fixture's own document in a way the schema accepts and that touches no
// pinned or fingerprinted content, so exactly one composite rule objects:
//   - instance-data-migration: a plan declares itself as `supersedes`
//     (`supersedes` is outside the fingerprint and the idempotency key);
//   - instance-canonical-export: an export's `idempotency_key` is replaced
//     by another well-formed hex value (outside the export digest);
//   - workspace-compatibility-qualification: the envelope's
//     `origin.source_ref` no longer names the pinned connection set (the
//     qualification record itself is not pinned by anything);
//   - upgrade-integration-qualification: a QUALIFIED record declares a
//     blocker, which is closed to BLOCKED.
// ---------------------------------------------------------------------------
function readBundle(dir, name) {
  const p = path.join(dir, 'registries', 'operating-model', 'fixtures', name);
  return [p, JSON.parse(fs.readFileSync(p, 'utf8'))];
}

function VALIDATE_MUTATION_FAMILIES_7D_WRITE_instanceDataMigration(dir) {
  const [p, bundle] = readBundle(dir, 'instance-data-migration.fixtures.json');
  const plan = bundle.valid[1].registry.migration_plans[0];
  plan.payload.supersedes = plan.id;
  fs.writeFileSync(p, JSON.stringify(bundle));
}

function VALIDATE_MUTATION_FAMILIES_7D_WRITE_instanceCanonicalExport(dir) {
  const [p, bundle] = readBundle(dir, 'instance-canonical-export.fixtures.json');
  bundle.valid[1].registry.exports[0].payload.idempotency_key = 'a'.repeat(64);
  fs.writeFileSync(p, JSON.stringify(bundle));
}

function VALIDATE_MUTATION_FAMILIES_7D_WRITE_workspaceCompatibilityQualification(dir) {
  const [p, bundle] = readBundle(dir, 'workspace-compatibility-qualification.fixtures.json');
  const c = bundle.valid.find((v) => v.note.startsWith('decision-matrix branch 5'));
  assert(c, `${p} must carry a "decision-matrix branch 5" valid fixture to mutate`);
  c.registry.qualifications[0].origin.source_ref = 'workspace-connection-scan:tampered';
  fs.writeFileSync(p, JSON.stringify(bundle));
}

function VALIDATE_MUTATION_FAMILIES_7D_WRITE_upgradeIntegrationQualification(dir) {
  const [p, bundle] = readBundle(dir, 'upgrade-integration-qualification.fixtures.json');
  const q = bundle.valid[0].registry.qualifications[0];
  assert(q.payload.qualification_state === 'QUALIFIED', `${p} valid[0] must be QUALIFIED to mutate`);
  q.payload.blockers = ['a blocker declared over a QUALIFIED record'];
  fs.writeFileSync(p, JSON.stringify(bundle));
}

const VALIDATE_MUTATION_FAMILIES_7D = [
  { id: 'instance-data-migration', write: VALIDATE_MUTATION_FAMILIES_7D_WRITE_instanceDataMigration },
  { id: 'instance-canonical-export', write: VALIDATE_MUTATION_FAMILIES_7D_WRITE_instanceCanonicalExport },
  { id: 'workspace-compatibility-qualification', write: VALIDATE_MUTATION_FAMILIES_7D_WRITE_workspaceCompatibilityQualification },
  { id: 'upgrade-integration-qualification', write: VALIDATE_MUTATION_FAMILIES_7D_WRITE_upgradeIntegrationQualification },
];

// The adversarial composition cases of §5.21.3 point 14, each with the
// family that must object:
//   - stale pin: a composed connection record's content changes under the
//     SAME reference, so its recomputed digest no longer equals the pin;
//   - wrong kind: a composed execution run declares another record kind;
//   - scope mismatch: an export's resolved plan is scoped to another
//     workspace;
//   - incomplete scenario set: one of the three neutral scenarios is gone.
function ADVERSARIAL_7D_stalePin(dir) {
  const [p, bundle] = readBundle(dir, 'workspace-compatibility-qualification.fixtures.json');
  for (const record of Object.values(bundle.connection_record_resolution)) {
    record.title = `${record.title} (изменено под той же ссылкой)`;
  }
  fs.writeFileSync(p, JSON.stringify(bundle));
}

function ADVERSARIAL_7D_wrongKind(dir) {
  const [p, bundle] = readBundle(dir, 'upgrade-integration-qualification.fixtures.json');
  for (const record of Object.values(bundle.execution_state_resolution)) {
    record.record_type = 'task-specification';
  }
  fs.writeFileSync(p, JSON.stringify(bundle));
}

function ADVERSARIAL_7D_scopeMismatch(dir) {
  const [p, bundle] = readBundle(dir, 'instance-canonical-export.fixtures.json');
  bundle.plan_resolution['sample-migration-plan-one'].scope = { type: 'project-workspace', id: 'another-project' };
  fs.writeFileSync(p, JSON.stringify(bundle));
}

function ADVERSARIAL_7D_incompleteScenarioSet(dir) {
  const [p, bundle] = readBundle(dir, 'upgrade-integration-qualification.fixtures.json');
  bundle.valid[0].registry.qualifications[0].payload.scenario_classifications.pop();
  fs.writeFileSync(p, JSON.stringify(bundle));
}

const ADVERSARIAL_7D = [
  { id: 'stale-pin', family: 'workspace-compatibility-qualification', write: ADVERSARIAL_7D_stalePin },
  { id: 'wrong-kind', family: 'upgrade-integration-qualification', write: ADVERSARIAL_7D_wrongKind },
  { id: 'scope-mismatch', family: 'instance-canonical-export', write: ADVERSARIAL_7D_scopeMismatch },
  { id: 'incomplete-scenario-set', family: 'upgrade-integration-qualification', write: ADVERSARIAL_7D_incompleteScenarioSet },
];

for (const c of [
  ...VALIDATE_MUTATION_FAMILIES_7D.map((f) => ({ id: f.id, family: f.id, write: f.write, kind: 'мутация семейства' })),
  ...ADVERSARIAL_7D.map((a) => ({ ...a, kind: 'состязательный случай' })),
]) {
  const mutationDir = fs.mkdtempSync(path.join(os.tmpdir(), `conformance-harness-kernel-validate-mutation-7d-${c.id}-`));
  createdTempDirs.push(mutationDir);
  try {
    copyRepoWithoutGitOrTarget(mutationDir);
    const assertDeltaMultisetsEqual = assertMutationDeltaMultisetsEqual(mutationDir, `${c.family}:`, c.id);
    check(
      `реальный Node/Rust validate: ${c.kind} "${c.id}" даёт равные как мультимножество непустые multiset-дельты с префиксом "${c.family}:" (rust-architecture-conformance-7)`,
      () => {
        assertDeltaMultisetsEqual(c.write);
      },
    );
  } finally {
    fs.rmSync(mutationDir, { recursive: true, force: true });
    createdTempDirs.splice(createdTempDirs.indexOf(mutationDir), 1);
  }
}

// Plan substitution: a plan resolver placed next to the composed export's
// own boundary, mapping the SAME plan id to a different plan, reaches
// neither implementation — both check the composed export only against
// the plan the qualification itself pinned. Both deltas stay EMPTY.
{
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'conformance-harness-kernel-validate-mutation-7d-plan-substitution-'));
  createdTempDirs.push(dir);
  try {
    copyRepoWithoutGitOrTarget(dir);
    const baseline = computeFailLines(dir);
    const [p, bundle] = readBundle(dir, 'workspace-compatibility-qualification.fixtures.json');
    const branch9 = bundle.valid.find((v) => v.note.startsWith('decision-matrix branch 9'));
    assert(branch9, `${p} must carry a "decision-matrix branch 9" valid fixture`);
    const planId = branch9.registry.qualifications[0].payload.migration_plan_ref.id;
    bundle.export_resolution.plan_resolution = {
      [planId]: {
        record_type: 'instance-migration-plan', plan_ref: planId, plan_fingerprint: '0'.repeat(64),
        scope: { type: 'project-workspace', id: 'substitute' }, source: {}, record_units: [], mappings: [],
      },
    };
    fs.writeFileSync(p, JSON.stringify(bundle));
    const mutated = computeFailLines(dir);
    check(
      'реальный Node/Rust validate: подмена плана под тем же id рядом с границей экспорта не влияет ни на одну сторону — обе дельты пусты (rust-architecture-conformance-7)',
      () => {
        assert(multisetDelta(baseline.nodeFails, mutated.nodeFails).length === 0, 'Node delta must be empty');
        assert(multisetDelta(baseline.rustFails, mutated.rustFails).length === 0, 'Rust delta must be empty');
      },
    );
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
    createdTempDirs.splice(createdTempDirs.indexOf(dir), 1);
  }
}

// ---------------------------------------------------------------------------
// Corrective round `rust-architecture-conformance-1`, item 10: an
// INTENTIONAL, permanent divergence, not a defect to converge — recorded
// in `COMPATIBILITY.md`. `scripts/lib/controlled-rule-intake.mjs`'s
// `checkCluster` compares two `semantic_key`-cluster members' scope with
// `scopeKey = \`${type}::${id}\`` (`type`/`id` only); the Rust
// implementation (`meridian_core::controlled_rule_intake::checks::check_cluster`)
// compares full `Scope` identity, the same rule `same_scope`/`checkSupersedes`
// already applies to instance-data-migration plans. This adds a second
// cluster member that is IDENTICAL to the bundled valid fixture's one
// candidate except for a different `id`, a different `boundary` (so the
// "same origin repeated" check stays silent — only the scope divergence is
// exercised) and a `scope.workspace_id` naming a different workspace. Node
// accepts this cleanly (its `scopeKey` never looks at `workspace_id`); Rust
// rejects it. This is NOT run through the generic `conformant`/`divergent`
// harness (which would wrongly report a "defect") — it states plainly,
// like the resolver-unknown-field-order boundary above, that the two sides
// genuinely and permanently differ here.
{
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'conformance-harness-kernel-validate-mutation-7c-cluster-scope-workspace-divergence-'));
  createdTempDirs.push(dir);
  try {
    copyRepoWithoutGitOrTarget(dir);
    const baseline = computeFailLines(dir);

    const fixturesPath = path.join(dir, 'registries', 'operating-model', 'fixtures', 'controlled-rule-intake.fixtures.json');
    const bundle = JSON.parse(fs.readFileSync(fixturesPath, 'utf8'));
    const template = bundle.valid[0];
    assert(
      template?.registry?.rule_candidates?.[0]?.scope?.workspace_id === 'sample-workspace',
      `${fixturesPath} valid[0]'s first candidate must be scoped to workspace_id "sample-workspace" to mutate`,
    );
    const original = template.registry.rule_candidates[0];
    const divergentMember = JSON.parse(JSON.stringify(original));
    divergentMember.id = 'sample-agents-md-branch-naming-candidate-workspace-divergence';
    divergentMember.scope.workspace_id = 'a-different-workspace';
    divergentMember.payload.boundary = { unit: 'whole-source' };
    template.registry.rule_candidates.push(divergentMember);
    fs.writeFileSync(fixturesPath, JSON.stringify(bundle));

    const mutated = computeFailLines(dir);
    const nodeDelta = multisetDelta(baseline.nodeFails, mutated.nodeFails);
    const rustDelta = multisetDelta(baseline.rustFails, mutated.rustFails);

    check(
      'реальный Node/Rust validate: намеренная граница «cluster scope divergence» (COMPATIBILITY.md) — Node принимает кластер с расходящимся workspace_id чисто (scopeKey смотрит только на type/id), новых FAIL нет',
      () => {
        assert(nodeDelta.length === 0, `ожидалась пустая Node-дельта (Node не видит расхождения по workspace_id), получено: ${JSON.stringify(nodeDelta)}`);
      },
    );

    check(
      'реальный Node/Rust validate: намеренная граница «cluster scope divergence» (COMPATIBILITY.md) — Rust отклоняет тот же кластер: полное тождество Scope ловит расхождение по workspace_id, которое Node молча пропускает',
      () => {
        assert(rustDelta.length > 0, `ожидалась непустая Rust-дельта, получено: ${JSON.stringify(rustDelta)}`);
        assert(
          rustDelta.every((l) => l.includes('more than one scope')),
          `ожидалось, что вся Rust-дельта состоит из диагностик "more than one scope", получено: ${JSON.stringify(rustDelta)}`,
        );
      },
    );

    check('намеренная граница «cluster scope divergence»: полный вывод действительно расходится — этот случай НЕ помечается conformant', () => {
      assert(
        !multisetsEqual(mutated.nodeFails, mutated.rustFails),
        'ожидалось настоящее расхождение полного вывода (иначе граница не названа честно, а спрятана)',
      );
    });
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
    createdTempDirs.splice(createdTempDirs.indexOf(dir), 1);
  }
}

// ---------------------------------------------------------------------------
// Corrective round `rust-architecture-conformance-1`, item 3: this
// disproves, by REAL executable path, an over-broad claim an earlier round
// of this pilot made in a doc comment — replaced here with what the
// executable paths actually show, per that round's own "document and test
// the divergence, OR fix the wrong statement" instruction.
//
// Source-level fact (verified by reading both files directly, unchanged by
// this test): `scripts/lib/controlled-rule-intake.mjs`'s
// `evaluateControlledRuleIntake` never stops at a schema violation (only at
// a schema-COMPILE exception) and unconditionally keeps computing the full
// composite analysis; the Rust `evaluate_controlled_rule_intake` stops
// immediately at any schema violation (earlier corrective-round item 2).
// This is a real, intentional difference in how MANY diagnostics each
// function computes internally.
//
// But `scripts/kernel-validate.mjs`'s own controlled-rule-intake self-test
// (`fail(\`...was rejected (...): ${p[0]}\`)`, line ~1952) and this
// crate's own CLI wrapper (`meridian-cli/src/commands/validate/controlled_rule_intake.rs`,
// `problems[0].message()`) BOTH report only the FIRST diagnostic for a
// wrongly-rejected fixture — and a schema violation is always pushed
// before any composite check on both sides, so `[0]` is the same schema
// diagnostic either way. The diagnostic-COUNT difference above is
// therefore NOT observable through this Kernel's only current executable
// path for controlled-rule-intake documents (there is no standalone CLI
// command reading an arbitrary such document directly — the contract's
// registry DATA is Instance data, per this module's own doc comment, and
// Instance is frozen). This mutates ONE candidate with a schema violation
// (deletes the required `payload.classification_basis`) alongside a
// SEPARATE composite-level defect (tampers `origin.source_ref`, the same
// mutation `VALIDATE_MUTATION_FAMILIES_7C`'s own "controlled-rule-intake"
// family already proves is composite-invalid on its own) and asserts what
// is ACTUALLY true at this executable surface: both sides' deltas carry
// ONLY the schema diagnostic, identically — CONFORMANT here, not divergent.
{
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'conformance-harness-kernel-validate-mutation-7c-schema-short-circuit-'));
  createdTempDirs.push(dir);
  try {
    copyRepoWithoutGitOrTarget(dir);
    const baseline = computeFailLines(dir);

    const fixturesPath = path.join(dir, 'registries', 'operating-model', 'fixtures', 'controlled-rule-intake.fixtures.json');
    const bundle = JSON.parse(fs.readFileSync(fixturesPath, 'utf8'));
    const candidate = bundle.valid[0]?.registry?.rule_candidates?.[0];
    assert(candidate?.payload?.classification_basis, `${fixturesPath} valid[0]'s first candidate must carry payload.classification_basis to mutate`);
    candidate.origin.source_ref += '-tampered';
    delete candidate.payload.classification_basis;
    fs.writeFileSync(fixturesPath, JSON.stringify(bundle));

    const mutated = computeFailLines(dir);
    const nodeDelta = multisetDelta(baseline.nodeFails, mutated.nodeFails);
    const rustDelta = multisetDelta(baseline.rustFails, mutated.rustFails);

    check(
      'реальный Node/Rust validate: «schema short-circuit» — оба self-test wrapper (kernel-validate.mjs строка ~1952 и Rust CLI) сообщают ТОЛЬКО первую диагностику (schema), поэтому разница в количестве вычисляемых диагностик внутри библиотек не наблюдаема на этом исполняемом пути — Node- и Rust-дельта несут одну и ту же диагностику схемы',
      () => {
        assert(
          nodeDelta.some((l) => l.includes('classification_basis') || l.includes('required')),
          `ожидалась диагностика схемы (classification_basis/required) в Node-дельте, получено: ${JSON.stringify(nodeDelta)}`,
        );
        assert(
          nodeDelta.every((l) => !l.includes('does not name the same source')),
          `Node-дельта на этом исполняемом пути НЕ должна нести составную диагностику origin/source_ref (wrapper сообщает только p[0]), получено: ${JSON.stringify(nodeDelta)}`,
        );
        assert(
          rustDelta.every((l) => !l.includes('does not name the same source')),
          `Rust-дельта не должна нести диагностику origin/source_ref, получено: ${JSON.stringify(rustDelta)}`,
        );
      },
    );

    check('«schema short-circuit»: на этом исполняемом пути стороны действительно conformant (полный wrapped-вывод совпадает) — библиотечное расхождение реально, но здесь не наблюдаемо, что и требовалось честно установить', () => {
      assert(
        multisetsEqual(mutated.nodeFails, mutated.rustFails),
        `ожидалось совпадение полного wrapped-вывода на этом исполняемом пути, получено node=${JSON.stringify(mutated.nodeFails)} rust=${JSON.stringify(mutated.rustFails)}`,
      );
    });
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
    createdTempDirs.splice(createdTempDirs.indexOf(dir), 1);
  }
}

// ---------------------------------------------------------------------------
// Corrective round `rust-architecture-conformance-1`, third round item 6,
// fourth round item 3.3, fifth round item 1: a DIRECT library-level test —
// `evaluateControlledRuleIntake` imported and called straight from
// `scripts/lib/controlled-rule-intake.mjs`, bypassing
// `scripts/kernel-validate.mjs`'s self-test wrapper entirely (which the
// "schema short-circuit" check right above this one already proved hides
// the library-level difference by only ever reporting `p[0]`). This is the
// Node half of a matched pair with the Rust half in
// `meridian-app/src/operating_model/controlled_rule_intake/mod.rs`'s own
// `#[test] fn schema_and_composite_defect_together_produce_exactly_one_schema_diagnostic_matching_the_node_reference`
// (also a direct library call, via `cargo test`, not through any CLI
// wrapper): both halves load THIS SAME real fixture file
// (`registries/operating-model/fixtures/controlled-rule-intake.fixtures.json`,
// `valid[0].registry`) and apply the SAME two mutations below — not an
// "analogous shape" input on each side, the identical document.
//
// The mutation is deliberately NOT "delete a required field" (fifth round
// item 1: the previous mutation, deleting `payload.classification_basis`,
// had no teeth on the Rust side — that field is both schema-required AND a
// non-optional `String` of `dto::PayloadDto`, so Rust's own closed-DTO
// deserialization would already produce a single diagnostic even with the
// schema gate entirely removed). Instead: `registry_id` is changed to a
// wrong, non-empty string — the schema's `registry_id: {const:
// "controlled-rule-intake"}` rejects it, while Rust's own
// `dto::RegistryDto.registry_id` is a plain, unvalidated `String` that
// would accept any value equally well, so this mutation can only be caught
// by the schema gate specifically (empirically confirmed on the Rust side;
// see that test's own doc comment and this round's transfer record).
//
// That Rust test asserts EXACTLY 1 diagnostic (schema only, explicitly NOT
// the origin/source_ref one); this Node test asserts AT LEAST 2 (schema AND
// composite), using this Kernel's own real schema/fixtures files. Together
// they are the direct proof, on both real language runtimes, of the
// library-level fact `COMPATIBILITY.md` records.
{
  const criSchema = JSON.parse(fs.readFileSync(path.join(ROOT, 'registries/operating-model/controlled-rule-intake.schema.json'), 'utf8'));
  const envelopeSchema = JSON.parse(fs.readFileSync(path.join(ROOT, 'registries/operating-model/scoped-record.schema.json'), 'utf8'));
  const bundle = JSON.parse(fs.readFileSync(path.join(ROOT, 'registries/operating-model/fixtures/controlled-rule-intake.fixtures.json'), 'utf8'));
  const registry = JSON.parse(JSON.stringify(bundle.valid[0].registry));
  assert(
    registry.registry_id === 'controlled-rule-intake',
    `controlled-rule-intake.fixtures.json valid[0].registry.registry_id must start as "controlled-rule-intake", got: ${JSON.stringify(registry.registry_id)}`,
  );
  registry.registry_id = 'wrong-registry-id';
  const candidate = registry.rule_candidates[0];
  candidate.origin.source_ref += '-tampered';

  check(
    'библиотечный (не через kernel-validate.mjs) Node-прогон: evaluateControlledRuleIntake, вызванная напрямую, возвращает ОБЕ независимые диагностики (схема И origin/source_ref) на документе с двумя одновременными независимыми дефектами — прямое доказательство корректирующего раунда item 6, парное с Rust-юнит-тестом schema_and_composite_defect_together_produce_exactly_one_schema_diagnostic_matching_the_node_reference',
    () => {
      const problems = evaluateControlledRuleIntake(registry, {
        registrySchema: criSchema,
        envelopeSchema,
        resolveSource: makeRecordResolver(bundle.resolution),
      });
      assert(
        problems.some((p) => p.includes('registry_id')),
        `ожидалась диагностика схемы по registry_id, получено: ${JSON.stringify(problems)}`,
      );
      assert(
        problems.some((p) => p.includes('does not name the same source')),
        `ожидалась диагностика origin/source_ref, получено: ${JSON.stringify(problems)}`,
      );
      assert(problems.length >= 2, `ожидалось минимум 2 диагностики (схема + composite), получено ${problems.length}: ${JSON.stringify(problems)}`);
    },
  );
}

// ---------------------------------------------------------------------------
// `rust-architecture-conformance-2`: direct library-level Node half of the
// matched pair documented in `meridian-app/src/operating_model/instruction_source_registry/mod.rs`'s
// own doc comment and `COMPATIBILITY.md` — `evaluateInstructionSourceRegistry`
// has no closed-transport-DTO concept the way the Rust port's
// `#[serde(deny_unknown_fields)]` does, so an entry carrying BOTH an unknown
// field AND a separate read-channel coherence defect still gets its
// business check computed on the Node side; the Rust unit test
// `a_transport_parse_failure_skips_business_checks_for_that_entry_only`
// (same real fixture, same two mutations) asserts the opposite: the
// business diagnostic never appears there, because domain construction is
// unreachable once the closed DTO parse itself fails.
{
  const isrSchema = JSON.parse(fs.readFileSync(path.join(ROOT, 'registries/operating-model/instruction-source-registry.schema.json'), 'utf8'));
  const envelopeSchema = JSON.parse(fs.readFileSync(path.join(ROOT, 'registries/operating-model/scoped-record.schema.json'), 'utf8'));
  const bundle = JSON.parse(fs.readFileSync(path.join(ROOT, 'registries/operating-model/fixtures/instruction-source-registry.fixtures.json'), 'utf8'));
  const registry = JSON.parse(JSON.stringify(bundle.valid[1].registry));
  const entry = registry.instruction_sources[0];
  assert(entry, 'instruction-source-registry.fixtures.json valid[1].registry must carry at least one instruction source');
  entry.payload.unexpected_field = true;
  entry.payload.read_channel.agent_auto_read = true;

  check(
    'библиотечный Node-прогон: evaluateInstructionSourceRegistry, вызванная напрямую, возвращает ОБЕ независимые диагностики (лишнее поле payload.unexpected_field И read_channel coherence) на записи с двумя одновременными независимыми дефектами',
    () => {
      const problems = evaluateInstructionSourceRegistry(registry, { registrySchema: isrSchema, envelopeSchema });
      assert(
        problems.some((p) => p.includes('unexpected_field') || p.includes('additionalProperties')),
        `ожидалась диагностика схемы по лишнему полю, получено: ${JSON.stringify(problems)}`,
      );
      assert(
        problems.some((p) => p.includes('agent_auto_read is true but kind is')),
        `ожидалась диагностика read_channel coherence, получено: ${JSON.stringify(problems)}`,
      );
    },
  );
}

// ---------------------------------------------------------------------------
// `rust-architecture-conformance-2`: same matched-pair shape as directly
// above, for `existing-project-compatibility-mode` — Node half. Rust half:
// `meridian-app/src/operating_model/existing_project_compatibility_mode/mod.rs::tests::a_transport_parse_failure_skips_business_checks_for_that_entry_only`.
{
  const epcmSchema = JSON.parse(fs.readFileSync(path.join(ROOT, 'registries/operating-model/existing-project-compatibility-mode.schema.json'), 'utf8'));
  const envelopeSchema = JSON.parse(fs.readFileSync(path.join(ROOT, 'registries/operating-model/scoped-record.schema.json'), 'utf8'));
  const sourceRegistrySchema = JSON.parse(fs.readFileSync(path.join(ROOT, 'registries/operating-model/instruction-source-registry.schema.json'), 'utf8'));
  const ruleIntakeSchema = JSON.parse(fs.readFileSync(path.join(ROOT, 'registries/operating-model/controlled-rule-intake.schema.json'), 'utf8'));
  const bundle = JSON.parse(fs.readFileSync(path.join(ROOT, 'registries/operating-model/fixtures/existing-project-compatibility-mode.fixtures.json'), 'utf8'));
  const registry = JSON.parse(JSON.stringify(bundle.valid[1].registry));
  const payload = registry.workspace_connections[0].payload;
  payload.unexpected_field = true;
  payload.next_step = 'resolve-conflict';

  check(
    'библиотечный Node-прогон: evaluateExistingProjectCompatibilityMode, вызванная напрямую, возвращает ОБЕ независимые диагностики (лишнее поле payload.unexpected_field И неверный next_step) на записи с двумя одновременными независимыми дефектами',
    () => {
      const problems = evaluateExistingProjectCompatibilityMode(registry, {
        registrySchema: epcmSchema, envelopeSchema, sourceRegistrySchema, ruleIntakeSchema,
      });
      assert(
        problems.some((p) => p.includes('unexpected_field') || p.includes('additionalProperties')),
        `ожидалась диагностика схемы по лишнему полю, получено: ${JSON.stringify(problems)}`,
      );
      assert(
        problems.some((p) => p.includes('next_step is')),
        `ожидалась диагностика next_step, получено: ${JSON.stringify(problems)}`,
      );
    },
  );
}

// ---------------------------------------------------------------------------
// `rust-architecture-conformance-2` corrective round, item 5: intentional
// Rust-native boundary, accepted by the architect (`COMPATIBILITY.md`, same
// precedent as `controlled-rule-intake`'s schema short-circuit). Node's
// `sameLocation` compares a discovery_plan slot's RAW path/container_ref
// fields against a discovered source's RAW location fields UNCONDITIONALLY —
// even when the slot's own `checkPlanLocation` already flagged it invalid.
// Rust's `plan_by_id` only holds slots whose OWN domain construction (a typed
// `Location`) succeeded, so when a slot's location is itself invalid, Rust
// skips the location-match comparison for any discovered source naming that
// slot entirely. The slot's own primary defect and the connection's overall
// verdict are unchanged on both sides; outcome partition still counts the
// slot by its independently typed `SemanticId`. Only the SECONDARY "location
// does not match" diagnostic is absent on the Rust side. This is the Node
// half of a matched pair with the Rust half in
// `meridian-app/src/operating_model/existing_project_compatibility_mode/mod.rs::tests::a_discovered_source_naming_an_invalid_plan_slot_gets_no_secondary_location_mismatch_diagnostic`
// — same real fixture, same two mutations.
{
  const epcmSchema = JSON.parse(fs.readFileSync(path.join(ROOT, 'registries/operating-model/existing-project-compatibility-mode.schema.json'), 'utf8'));
  const envelopeSchema = JSON.parse(fs.readFileSync(path.join(ROOT, 'registries/operating-model/scoped-record.schema.json'), 'utf8'));
  const sourceRegistrySchema = JSON.parse(fs.readFileSync(path.join(ROOT, 'registries/operating-model/instruction-source-registry.schema.json'), 'utf8'));
  const ruleIntakeSchema = JSON.parse(fs.readFileSync(path.join(ROOT, 'registries/operating-model/controlled-rule-intake.schema.json'), 'utf8'));
  const bundle = JSON.parse(fs.readFileSync(path.join(ROOT, 'registries/operating-model/fixtures/existing-project-compatibility-mode.fixtures.json'), 'utf8'));
  const registry = JSON.parse(JSON.stringify(bundle.valid[1].registry));
  const payload = registry.workspace_connections[0].payload;
  payload.discovery_plan[0].path = '../escape.md';
  payload.discovered_sources[0].payload.location.path = 'a-different-valid-path.md';

  check(
    'намеренная граница «invalid plan slot secondary location match» (принято архитектором): библиотечный Node-прогон сообщает "location does not match" в дополнение к собственному дефекту слота, где Rust-сторона намеренно сообщает только первичный дефект слота (см. doc-комментарий парного Rust-теста и COMPATIBILITY.md)',
    () => {
      const problems = evaluateExistingProjectCompatibilityMode(registry, {
        registrySchema: epcmSchema, envelopeSchema, sourceRegistrySchema, ruleIntakeSchema,
      });
      assert(
        problems.some((p) => p.includes('root-agents-md') && (p.includes('".."') || p.includes('segment'))),
        `ожидалась диагностика собственного дефекта слота, получено: ${JSON.stringify(problems)}`,
      );
      assert(
        problems.some((p) => p.includes('location does not match')),
        `ожидалась диагностика location does not match (текущее поведение Node), получено: ${JSON.stringify(problems)}`,
      );
    },
  );
}

// ---------------------------------------------------------------------------
// Subpackage 7c, corrective round: `meridian-field-evaluation`'s
// `measurement.start_ts`/`end_ts` interval check
// (`meridian-app/src/operating_model/field_evaluation.rs`,
// `parse_datetime_millis`) matched `(?:\.\d+)?` for fractional seconds
// WITHOUT capturing them, so it silently dropped fractional seconds
// entirely — two timestamps differing only in their fraction (e.g.
// `.100Z` vs `.200Z`) parsed to the SAME millisecond, and a genuinely
// later `end_ts` compared as not-after `start_ts`: a real, schema-valid
// interval was wrongly rejected. This is the opposite shape from every
// `VALIDATE_MUTATION_FAMILIES_*` check above (which mutate a valid fixture
// into something that MUST be rejected) — this one adds a genuinely valid
// fixture the fix must ACCEPT, and both real Node and real Rust `validate`
// must agree on that, not merely each pass its own unit tests in
// isolation. The added fixture reuses `obs-context-time-1`'s own
// `execution_run_ref`/evidence identity (already resolvable through this
// bundle's own `resolution` map) rather than inventing a new one, cloned
// from the bundled invalid fixture that already proves the OPPOSITE
// interval (`end_ts` before `start_ts`) is rejected — so only the
// fractional-seconds precision itself is exercised, nothing else about
// the composition.
function VALIDATE_MUTATION_FAMILIES_7C_EXTRA_WRITE_fieldEvaluationSubSecondInterval(dir) {
  const p = path.join(dir, 'registries', 'operating-model', 'fixtures', 'field-evaluation.fixtures.json');
  const bundle = JSON.parse(fs.readFileSync(p, 'utf8'));
  const template = bundle.invalid.find((c) => c.note === 'невозможный временной интервал (конец раньше начала)');
  assert(template, `${p} must carry a "невозможный временной интервал (конец раньше начала)" invalid fixture to clone`);
  const spec = JSON.parse(JSON.stringify(template.spec));
  spec.id = 'obs-context-time-fraction-check';
  spec.title = 'Наблюдение: context-entry-time (доля секунды различает интервал)';
  spec.payload.measurement.start_ts = '2026-08-03T12:00:00.100Z';
  spec.payload.measurement.end_ts = '2026-08-03T12:00:00.200Z';
  bundle.valid.push({
    note: 'интервал длиной 100мс, различающийся только долей секунды после запятой — обязан приниматься',
    spec,
  });
  fs.writeFileSync(p, JSON.stringify(bundle));
}

{
  const mutationDir = fs.mkdtempSync(path.join(os.tmpdir(), 'conformance-harness-kernel-validate-mutation-7c-field-evaluation-subsecond-'));
  createdTempDirs.push(mutationDir);
  try {
    copyRepoWithoutGitOrTarget(mutationDir);
    VALIDATE_MUTATION_FAMILIES_7C_EXTRA_WRITE_fieldEvaluationSubSecondInterval(mutationDir);
    check(
      'реальный Node/Rust validate: добавленная meridian-field-evaluation valid-fixture с интервалом, различающимся только долей секунды (.100Z раньше .200Z), принимается ОДИНАКОВО на обеих сторонах — регрессия для исправления fractional-seconds в parse_datetime_millis (подпакет 7c)',
      () => {
        const result = runCase(mutatedKernelValidateCase('cli-validate-mutation-7c-field-evaluation-subsecond-interval', mutationDir));
        assert(
          result.status === 'conformant',
          `ожидался conformant (обе стороны принимают интервал .100Z → .200Z как валидный — доля секунды не отбрасывается), получено ${result.status}: ${JSON.stringify(result)}`,
        );
      },
    );
  } finally {
    fs.rmSync(mutationDir, { recursive: true, force: true });
    createdTempDirs.splice(createdTempDirs.indexOf(mutationDir), 1);
  }
}

// ---------------------------------------------------------------------------
// Package meridian-cli-foundation, subpackage validate-operating-contracts,
// corrective round, item 2: `bounded-context-manifest`'s resolved-entry
// "unknown field" checks obtain a genuine Rust-native improvement over the
// Node reference: unknown keys are reported in stable sorted order, never
// Node's `Object.keys()` source-text order. Since
// `rust-architecture-conformance-5` the typed resolution catalogue carries
// them by name in that order
// (`meridian-app/src/operating_model/bounded_context_manifest/resolution.rs`)
// and `meridian_core::run_contracts::resolution` reports them; kept, not
// reverted (Rust unit test
// `bounded_context_manifest::tests::unknown_resolver_fields_are_reported_in_stable_sorted_order`,
// `COMPATIBILITY.md`). This is
// the real-process counterpart: it constructs a resolver response with two
// unknown fields named so their SOURCE-TEXT order is the opposite of their
// alphabetical order — "zzzz_extra_field" written first, "aaaa_extra_field"
// last — and runs both the real Node reference and the real compiled
// `meridian` binary against it directly, WITHOUT going through the generic
// `conformant`/`divergent` harness. It computes an explicit baseline/delta
// (`computeFailLines`/`multisetDelta`, the same machinery
// `VALIDATE_MUTATION_FAMILIES_7B` uses) and pins the EXACT full delta on
// each side: every line in the Node delta names "zzzz_extra_field" and none
// name "aaaa_extra_field" (and vice versa for the Rust delta) — no
// additional, unaccounted-for new FAIL is tolerated on either side. It also
// proves the two deltas are otherwise the SAME set of diagnostics by
// remapping the field name and comparing as multisets, so the only named
// difference is genuinely that one field name, nothing else. The two sides'
// full diagnostic text genuinely differs by construction (which unknown
// field is named first), so this test states plainly that they are NOT
// conformant, while pinning that both still fail closed on the same
// underlying defect — never silently accepting the malformed resolver
// response, and never crashing instead of reporting it.
{
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'conformance-harness-kernel-validate-mutation-7b-unknown-field-order-'));
  createdTempDirs.push(dir);
  try {
    copyRepoWithoutGitOrTarget(dir);
    const baseline = computeFailLines(dir);

    const fixturesPath = path.join(dir, 'registries', 'operating-model', 'fixtures', 'context-manifest.fixtures.json');
    const bundle = JSON.parse(fs.readFileSync(fixturesPath, 'utf8'));
    const resolutionKey = 'records/execution-run/example-run-001';
    const original = bundle.resolution[resolutionKey];
    assert(original, `${fixturesPath} must carry a "${resolutionKey}" resolution entry to mutate`);
    const mutatedEntry = {};
    mutatedEntry.zzzz_extra_field = 'z';
    for (const k of Object.keys(original)) mutatedEntry[k] = original[k];
    mutatedEntry.aaaa_extra_field = 'a';
    bundle.resolution[resolutionKey] = mutatedEntry;
    fs.writeFileSync(fixturesPath, JSON.stringify(bundle));

    const mutated = computeFailLines(dir);
    const nodeDelta = multisetDelta(baseline.nodeFails, mutated.nodeFails);
    const rustDelta = multisetDelta(baseline.rustFails, mutated.rustFails);

    check('намеренная граница: обе стороны fail-closed на резолвере с несколькими неизвестными полями (ни одна не считает документ чистым)', () => {
      assert(nodeDelta.length > 0, `ожидалась непустая Node-дельта, получено: ${JSON.stringify(nodeDelta)}`);
      assert(rustDelta.length > 0, `ожидалась непустая Rust-дельта, получено: ${JSON.stringify(rustDelta)}`);
      assert(
        nodeDelta.every((l) => l.includes('unknown field')),
        `ожидалось, что вся Node-дельта состоит из диагностик "unknown field", получено: ${JSON.stringify(nodeDelta)}`,
      );
      assert(
        rustDelta.every((l) => l.includes('unknown field')),
        `ожидалось, что вся Rust-дельта состоит из диагностик "unknown field", получено: ${JSON.stringify(rustDelta)}`,
      );
    });

    check('намеренная граница: точный полный набор — вся Node-дельта называет "zzzz_extra_field" (порядок исходного текста) и не называет "aaaa_extra_field"; никаких дополнительных новых FAIL', () => {
      assert(
        nodeDelta.every((l) => l.includes('"zzzz_extra_field"')),
        `ожидалось, что каждая строка Node-дельты называет "zzzz_extra_field", получено: ${JSON.stringify(nodeDelta)}`,
      );
      assert(
        nodeDelta.every((l) => !l.includes('"aaaa_extra_field"')),
        `Node-дельта не должна называть "aaaa_extra_field" ни в одной строке, получено: ${JSON.stringify(nodeDelta)}`,
      );
    });

    check('намеренная граница: точный полный набор — вся Rust-дельта называет "aaaa_extra_field" (алфавитный порядок) и не называет "zzzz_extra_field"; никаких дополнительных новых FAIL', () => {
      assert(
        rustDelta.every((l) => l.includes('"aaaa_extra_field"')),
        `ожидалось, что каждая строка Rust-дельты называет "aaaa_extra_field", получено: ${JSON.stringify(rustDelta)}`,
      );
      assert(
        rustDelta.every((l) => !l.includes('"zzzz_extra_field"')),
        `Rust-дельта не должна называть "zzzz_extra_field" ни в одной строке, получено: ${JSON.stringify(rustDelta)}`,
      );
    });

    check('намеренная граница: обе дельты совпадают как множества строк ПОСЛЕ замены имени поля — единственное отличие между сторонами это имя первого названного неизвестного поля, не что-то ещё', () => {
      const nodeDeltaAsIfAlphabetical = nodeDelta.map((l) => l.replaceAll('"zzzz_extra_field"', '"aaaa_extra_field"'));
      assert(
        multisetsEqual(nodeDeltaAsIfAlphabetical, rustDelta),
        `ожидалось, что Node-дельта после замены "zzzz_extra_field" на "aaaa_extra_field" побайтово совпадает с Rust-дельтой как multiset; node(remapped)=${JSON.stringify(nodeDeltaAsIfAlphabetical)} rust=${JSON.stringify(rustDelta)}`,
      );
    });

    check('намеренная граница: обе стороны fail-closed (ненулевой код завершения, result.ok: false у Rust)', () => {
      const nodeRun = spawnSync(process.execPath, [path.join(ROOT, 'scripts', 'kernel-validate.mjs')], {
        cwd: ROOT,
        env: kernelOnlyEnv({ MERIDIAN_KERNEL: dir }),
        encoding: 'utf8',
      });
      const meridianBin = path.join(ROOT, 'target', 'debug', process.platform === 'win32' ? 'meridian.exe' : 'meridian');
      const rustRun = spawnSync(meridianBin, ['validate', '--kernel', dir, '--format', 'json'], {
        encoding: 'utf8',
        env: kernelOnlyEnv({ MERIDIAN_KERNEL: dir }),
      });
      let rustResult;
      try {
        rustResult = JSON.parse(rustRun.stdout.trim());
      } catch (e) {
        throw new Error(`expected exactly one JSON result document on stdout, got: ${JSON.stringify(rustRun.stdout)} (${e.message})`);
      }
      assert(nodeRun.status !== 0, `ожидался ненулевой код завершения Node, получено ${nodeRun.status}`);
      assert(rustRun.status === 1, `ожидался exit_code::DOMAIN_NEGATIVE (1), получено ${rustRun.status}`);
      assert(rustResult.result.ok === false, 'ожидался result.ok: false');
    });

    check('намеренная граница: полный вывод действительно расходится — этот случай НЕ помечается conformant', () => {
      assert(
        !multisetsEqual(mutated.nodeFails, mutated.rustFails),
        'ожидалось настоящее расхождение полного вывода (иначе граница не названа честно, а спрятана)',
      );
    });
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
    createdTempDirs.splice(createdTempDirs.indexOf(dir), 1);
  }
}

// ---------------------------------------------------------------------------
// Package meridian-cli-foundation, subpackage validate-mechanical-integrity,
// second CHANGES_REQUESTED round on 7a, item 2: cross-language conformance
// for the specific adversarial shapes that round named, beyond the single
// simple mutation per family above:
//   - several simultaneous missing/extra elements, in non-alphabetical
//     order (proves insertion-order-preserving diagnostics, not just that a
//     single-element disagreement is detected at all);
//   - a "topics"/"profiles" key of the wrong YAML type;
//   - a duplicated pool region marker, and a missing one;
//   - a symlinked directory under skills/ (proves discovery does not follow
//     it — Node's own Dirent.isDirectory() never does either).
// Each gets its own full repository copy, for the same reason
// VALIDATE_MUTATION_FAMILIES_7A does: several of these mutate an existing
// Kernel file in place, which a shared-directory cleanup could not restore
// for the next family's turn.
// ---------------------------------------------------------------------------
const VALIDATE_MUTATION_FAMILIES_7A_ADVERSARIAL = [
  {
    id: 'instruction-topics-several-elements-out-of-order',
    write: (dir) => {
      const yamlFile = path.join(dir, 'standards', 'workspace', 'instruction-topics.yaml');
      fs.appendFileSync(yamlFile, '  - zzz-mutation-probe-topic\n  - aaa-mutation-probe-topic\n');
      const mdFile = path.join(dir, 'standards', 'workspace', 'instruction-topics.md');
      const text = fs.readFileSync(mdFile, 'utf8');
      const endMarker = '<!-- meridian:end topic-pool -->';
      const idx = text.indexOf(endMarker);
      assert(idx !== -1, 'instruction-topics.md must contain the topic-pool end marker to mutate before');
      const insertion = '| `zzz-mutation-doc-only` | q | q | q |\n| `aaa-mutation-doc-only` | q | q | q |\n';
      fs.writeFileSync(mdFile, text.slice(0, idx) + insertion + text.slice(idx));
    },
    expectedFailPrefix: 'instruction-topics:',
  },
  {
    id: 'stack-profiles-duplicated-pool-region',
    write: (dir) => {
      const mdFile = path.join(dir, 'stack-profiles', 'stack-profiles.md');
      const text = fs.readFileSync(mdFile, 'utf8');
      const beginMarker = '<!-- meridian:begin stack-profile-pool -->';
      const endMarker = '<!-- meridian:end stack-profile-pool -->';
      const begin = text.indexOf(beginMarker);
      const end = text.indexOf(endMarker);
      assert(begin !== -1 && end !== -1, 'stack-profiles.md must contain a stack-profile-pool region to duplicate');
      const regionEnd = end + endMarker.length;
      const region = text.slice(begin, regionEnd);
      fs.writeFileSync(mdFile, text.slice(0, regionEnd) + '\n' + region + text.slice(regionEnd));
    },
    expectedFailPrefix: 'stack-profiles:',
  },
  {
    id: 'operating-foundation-missing-pool-region',
    write: (dir) => {
      const mdFile = path.join(dir, 'standards', 'workspace', 'operating-glossary.md');
      const text = fs.readFileSync(mdFile, 'utf8');
      const beginMarker = '<!-- meridian:begin operating-term-pool -->';
      const endMarker = '<!-- meridian:end operating-term-pool -->';
      const begin = text.indexOf(beginMarker);
      const end = text.indexOf(endMarker);
      assert(begin !== -1 && end !== -1, 'operating-glossary.md must contain an operating-term-pool region to remove');
      fs.writeFileSync(mdFile, text.slice(0, begin) + text.slice(end + endMarker.length));
    },
    expectedFailPrefix: 'operating-foundation:',
  },
  {
    id: 'sha-provenance-symlinked-skill-is-not-followed',
    write: (dir) => {
      const targetDir = path.join(dir, 'skills', 'mutation-probe-symlink-target');
      fs.mkdirSync(targetDir, { recursive: true });
      fs.writeFileSync(path.join(targetDir, 'SKILL.md'), 'mutation probe skill\n');
      // A deliberately wrong pin: the real target directory itself fails
      // sha-provenance identically on both sides (both see it as a normal,
      // non-symlink directory). The point under test is whether a *symlink*
      // to this directory is discovered as a second, separate "skill" —
      // which must not happen on either side; before this round's fix,
      // Rust's is_dir() followed the symlink and read a real PIN.yaml
      // through it, producing a second FAIL under the symlink's own name
      // that the Node reference (whose Dirent.isDirectory() never follows a
      // symlink) never produces.
      fs.writeFileSync(
        path.join(targetDir, 'PIN.yaml'),
        'artifact: SKILL.md\nsha256: deadbeef000000000000000000000000000000000000000000000000000000000000\n',
      );
      fs.symlinkSync(targetDir, path.join(dir, 'skills', 'mutation-probe-symlink'), 'dir');
    },
    expectedFailPrefix: 'sha-provenance:',
  },
];

for (const family of VALIDATE_MUTATION_FAMILIES_7A_ADVERSARIAL) {
  const mutationDir = fs.mkdtempSync(path.join(os.tmpdir(), `conformance-harness-kernel-validate-mutation-7a-adversarial-${family.id}-`));
  createdTempDirs.push(mutationDir);
  try {
    copyRepoWithoutGitOrTarget(mutationDir);
    family.write(mutationDir);
    check(`реальный Node/Rust validate: состязательная мутация "${family.id}" совпадает на обеих сторонах (7a, второй раунд CHANGES_REQUESTED)`, () => {
      const result = runCase(mutatedKernelValidateCase(`cli-validate-mutation-7a-adversarial-${family.id}`, mutationDir));
      assert(
        result.status === 'conformant',
        `ожидался conformant для "${family.id}", получено ${result.status}: ${JSON.stringify(result)}`,
      );
      const leftFail = result.comparison.fail.left;
      assert(
        leftFail.some((m) => m.startsWith(family.expectedFailPrefix)),
        `мутация "${family.id}" должна была произвести хотя бы один новый FAIL с префиксом "${family.expectedFailPrefix}", получено: ${JSON.stringify(leftFail)}`,
      );
    });
  } finally {
    fs.rmSync(mutationDir, { recursive: true, force: true });
    createdTempDirs.splice(createdTempDirs.indexOf(mutationDir), 1);
  }
}

// ---------------------------------------------------------------------------
// Package rust-business-contract-qualification (plan §5.23), corrective
// round: the four observable Node/Rust `validate` deltas the architect
// classified ACCEPTED_RUST_NATIVE (COMPATIBILITY.md, GAP-10…GAP-13). Each
// case runs the real Node reference and the real compiled `meridian` on one
// mutated full copy of this repository and pins the EXACT shape of the
// accepted divergence: same exit code, equal WARN multisets, and a FAIL
// multiset difference that is exactly the declared Node-only and Rust-only
// messages — nothing else may differ. Selectable:
// `--test-name-pattern accepted-rust-native`.
// ---------------------------------------------------------------------------

// Removes one match per expected entry (an exact string or a RegExp) from
// `actual`; every expected entry must be matched and nothing may be left.
function assertExactlyMatched(actual, expected, side) {
  const rest = [...actual];
  for (const want of expected) {
    const i = rest.findIndex((m) => (want instanceof RegExp ? want.test(m) : m === want));
    assert(i !== -1, `${side}: нет ожидаемого FAIL ${want} среди ${JSON.stringify(rest)}`);
    rest.splice(i, 1);
  }
  assert(rest.length === 0, `${side}: необъявленные FAIL ${JSON.stringify(rest)}`);
}

function assertDeclaredDivergence(result, { nodeOnly, rustOnly }) {
  assert(result.status === 'divergent', `ожидалось объявленное расхождение, получено ${result.status}: ${JSON.stringify(result).slice(0, 4000)}`);
  const c = result.comparison;
  assert(c.exit_code.match && c.exit_code.left === 1, `коды завершения должны совпасть (1): ${JSON.stringify(c.exit_code)}`);
  assert(c.warn.missing.length === 0 && c.warn.added.length === 0, `WARN-множества должны совпадать полностью: ${JSON.stringify(c.warn)}`);
  const extra = (diff) => diff.flatMap((d) => Array(Math.abs(d.left_count - d.right_count)).fill(d.message));
  assertExactlyMatched(extra(c.fail.missing), nodeOnly, 'Node');
  assertExactlyMatched(extra(c.fail.added), rustOnly, 'Rust');
}

function runAcceptedRustNativeDivergences() {
  const selected = (name) => !NAME_PATTERN || NAME_PATTERN.test(name);
  const acheck = (name, fn) => { if (selected(name)) check(name, fn); };
  const build = spawnSync('cargo', ['build', '-p', 'meridian-cli', '--bin', 'meridian', '--example', 'validate_cli_producer'], { cwd: ROOT, encoding: 'utf8' });
  const producer = path.join(ROOT, 'target', 'debug', 'examples', process.platform === 'win32' ? 'validate_cli_producer.exe' : 'validate_cli_producer');
  const onMutatedCopy = (id, mutate, verify) => {
    const name = `accepted-rust-native: ${id}`;
    if (!selected(name)) return;
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), `conformance-harness-accepted-rust-native-${id}-`));
    createdTempDirs.push(dir);
    try {
      copyRepoWithoutGitOrTarget(dir);
      const context = mutate(dir);
      check(name, () => {
        assert(build.status === 0, `cargo build: ${build.stderr}`);
        const result = runCase({
          name: `accepted-rust-native-${id}`,
          left: { command: process.execPath, args: [path.join(ROOT, 'scripts', 'kernel-validate.mjs')], cwd: ROOT, env: kernelOnlyEnv({ MERIDIAN_KERNEL: dir }) },
          right: { command: producer, args: [dir], cwd: ROOT, env: kernelOnlyEnv() },
        });
        verify(result, dir, context);
      });
    } finally {
      fs.rmSync(dir, { recursive: true, force: true });
      createdTempDirs.splice(createdTempDirs.indexOf(dir), 1);
    }
  };

  // GAP-10: the authored "<family>: <file> is not valid JSON: " prefix and
  // the verdict are the contract; the parser's own tail (V8 against
  // serde_json) is not. Every independent production parser owner gets
  // matched Node/Rust evidence: same exit code, equal WARN multisets, the
  // same authored prefixes (pinned by name), a differing parser tail only,
  // and no undeclared FAIL.
  const JSON_MARKER = ' is not valid JSON: ';
  const BROKEN_JSON = '{"type": "object",}';
  const assertParserTailOnly = (result, expectedPrefixes, declaredRustOnly = []) => {
    assert(result.status === 'divergent', `ожидалось расхождение только в хвосте, получено ${result.status}`);
    const c = result.comparison;
    assert(c.exit_code.match && c.exit_code.left === 1, `коды завершения: ${JSON.stringify(c.exit_code)}`);
    assert(c.warn.missing.length === 0 && c.warn.added.length === 0, `WARN: ${JSON.stringify(c.warn)}`);
    const extra = (diff) => diff.flatMap((d) => Array(Math.abs(d.left_count - d.right_count)).fill(d.message));
    const rustExtra = extra(c.fail.added);
    for (const declared of declaredRustOnly) {
      const i = rustExtra.indexOf(declared);
      assert(i !== -1, `Rust: нет объявленного FAIL ${JSON.stringify(declared)} среди ${JSON.stringify(rustExtra)}`);
      rustExtra.splice(i, 1);
    }
    const tails = { Node: new Set(), Rust: new Set() };
    const split = (messages, side) => messages.map((m) => {
      const at = m.indexOf(JSON_MARKER);
      assert(at !== -1 && m.length > at + JSON_MARKER.length, `${side}: необъявленный FAIL (расходится не хвост невалидного JSON): ${m}`);
      tails[side].add(m.slice(at + JSON_MARKER.length));
      return m.slice(0, at + JSON_MARKER.length);
    }).sort();
    const nodePrefixes = split(extra(c.fail.missing), 'Node');
    const rustPrefixes = split(rustExtra, 'Rust');
    const expected = [...expectedPrefixes].sort();
    assert(JSON.stringify(nodePrefixes) === JSON.stringify(expected), `Node: авторские префиксы ${JSON.stringify(nodePrefixes)}, ожидались ровно ${JSON.stringify(expected)}`);
    assert(JSON.stringify(rustPrefixes) === JSON.stringify(expected), `Rust: авторские префиксы ${JSON.stringify(rustPrefixes)}, ожидались ровно ${JSON.stringify(expected)}`);
    for (const tail of tails.Rust) assert(!tails.Node.has(tail), `хвост ${JSON.stringify(tail)} совпал — это не parser-specific расхождение`);
  };

  // Combined routes on one copy: the generic `$schema` pass (cli
  // registry_schema), a CLI-owned fixture reader (cli
  // rule_resolution_fixtures), the shared run-contract/migration schema
  // reader (app run_contract_boundary::parse_json — upgrade-integration-
  // qualification composes execution-state.schema.json through the same
  // reader) and an app-owned fixture reader (app functional_parity).
  onMutatedCopy('GAP-10 invalid-JSON tail: combined routes', (dir) => {
    fs.writeFileSync(path.join(dir, 'registries', 'operating-model', 'execution-state.schema.json'), BROKEN_JSON);
    fs.writeFileSync(path.join(dir, 'registries', 'rule-resolution', 'fixtures', 'rule-resolution.fixtures.json'), BROKEN_JSON);
    fs.writeFileSync(path.join(dir, 'verification', 'functional-parity', 'fixtures', 'functional-parity-evidence.fixtures.json'), BROKEN_JSON);
    fs.writeFileSync(path.join(dir, 'mutation-broken.schema.json'), BROKEN_JSON);
    fs.writeFileSync(path.join(dir, 'mutation-broken-schema-data.yaml'), '$schema: ./mutation-broken.schema.json\n');
  }, (result, dir) => assertParserTailOnly(result, [
    `schema: ${path.join(dir, 'mutation-broken.schema.json')}${JSON_MARKER}`,
    `rule-resolution: the fixtures file${JSON_MARKER}`,
    `functional-parity: the fixtures file${JSON_MARKER}`,
    `execution-state-model: execution-state.schema.json${JSON_MARKER}`,
    `upgrade-integration-qualification: execution-state.schema.json${JSON_MARKER}`,
  ]));

  // One isolated copy per remaining independent parser owner, so that one
  // family's cascade cannot mask another's.
  const ISOLATED_JSON_ROUTES = [
    // cli commands::validate::instruction_source_registry
    ['instruction-source-registry', []],
    // cli commands::validate::controlled_rule_intake
    ['controlled-rule-intake', []],
    // cli commands::validate::existing_project_compatibility_mode
    ['existing-project-compatibility-mode', []],
    // app operating_model::task_pattern_registry. Any failure of the
    // registry leaves no catalog; the two dependent Rust-only lines are the
    // separately accepted "catalog published whole" boundaries
    // (COMPATIBILITY.md, `task-pattern-registry`/`task-specification-contract`
    // and `upgrade-integration-qualification` without a catalog), declared
    // here by exact text rather than tolerated.
    ['task-pattern-registry', [
      'task-specification-contract: no task-pattern catalog is available; task-pattern-registry must be checked first',
      'upgrade-integration-qualification: no task-pattern catalog is available; task-pattern-registry must be checked first',
    ]],
    // app operating_model::task_specification
    ['task-specification', []],
  ];
  const FAMILY_OF = { 'task-specification': 'task-specification-contract' };
  for (const [fixture, declaredRustOnly] of ISOLATED_JSON_ROUTES) {
    onMutatedCopy(`GAP-10 invalid-JSON tail: ${fixture} fixtures`, (dir) => {
      fs.writeFileSync(path.join(dir, 'registries', 'operating-model', 'fixtures', `${fixture}.fixtures.json`), BROKEN_JSON);
    }, (result) => assertParserTailOnly(
      result,
      [`${FAMILY_OF[fixture] ?? fixture}: the fixtures file${JSON_MARKER}`],
      declaredRustOnly,
    ));
  }

  // GAP-11: the production Rust route reads no Instance, so a `$schema`
  // that resolves nowhere is reported against the Kernel only.
  onMutatedCopy('GAP-11 missing schema names the Kernel only', (dir) => {
    fs.writeFileSync(path.join(dir, 'mutation-missing-schema.yaml'), '$schema: ./nope.schema.json\n');
  }, (result, dir) => {
    const file = path.join(dir, 'mutation-missing-schema.yaml');
    assertDeclaredDivergence(result, {
      nodeOnly: [`schema: ${file} references ./nope.schema.json, which resolves to no file in the Instance or the Kernel`],
      rustOnly: [`schema: ${file} references ./nope.schema.json, which resolves to no file in the Kernel`],
    });
  });

  // GAP-12: three texts the Node YAML subset accepts while silently
  // dropping data (`"unterminated` keeps its quote; a mis-indented key and a
  // mapping key after a top-level sequence vanish). Rust rejects all three
  // fail-closed; a well-formed control document under the same schema is
  // accepted by both sides.
  onMutatedCopy('GAP-12 lossy YAML is rejected fail-closed', (dir) => {
    fs.writeFileSync(path.join(dir, 'mutation-lossy.schema.json'), '{"type":"object"}');
    const files = {
      unterminated: '$schema: ./mutation-lossy.schema.json\na: "unterminated\n',
      misindented: '$schema: ./mutation-lossy.schema.json\na:\n  - b\n c: d\n',
      'sequence-then-mapping': '- a\nb: 1\n',
      control: '$schema: ./mutation-lossy.schema.json\na: "terminated"\n',
    };
    for (const [name, text] of Object.entries(files)) fs.writeFileSync(path.join(dir, `mutation-lossy-${name}.yaml`), text);
    return Object.keys(files).filter((name) => name !== 'control');
  }, (result, dir, lossy) => {
    const nodeView = lossy.map((name) => yamlParse(fs.readFileSync(path.join(dir, `mutation-lossy-${name}.yaml`), 'utf8')));
    assert(
      JSON.stringify(nodeView) === JSON.stringify([
        { $schema: './mutation-lossy.schema.json', a: '"unterminated' },
        { $schema: './mutation-lossy.schema.json', a: ['b'] },
        ['a'],
      ]),
      `Node-подмножество должно принимать все три входа с потерей данных, получено ${JSON.stringify(nodeView)}`,
    );
    assertDeclaredDivergence(result, {
      nodeOnly: [],
      rustOnly: lossy.map((name) => new RegExp(`^schema: cannot parse ${path.join(dir, `mutation-lossy-${name}.yaml`).replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}: malformed YAML: \\S`)),
    });
  });

  // GAP-13: `topics` of the wrong YAML type. Node fails through an
  // uncaught engine TypeError text inside its try/catch; Rust reports one
  // authored FAIL. Nothing else may differ.
  onMutatedCopy('GAP-13 instruction-topics of the wrong type', (dir) => {
    fs.writeFileSync(path.join(dir, 'standards', 'workspace', 'instruction-topics.yaml'), 'schema_version: 1\ntopics: true\n');
  }, (result) => {
    assertDeclaredDivergence(result, {
      nodeOnly: [/^instruction-topics: \S.* is not a function$/],
      rustOnly: ['instruction-topics: "topics" must be a list, found a boolean'],
    });
  });
}

if (!NAME_PATTERN) runAcceptedRustNativeDivergences();

// ---------------------------------------------------------------------------
// Package meridian-cli-foundation, subpackage validate-mechanical-integrity,
// third CHANGES_REQUESTED round, item 2: a wrong-typed
// stack-profiles.profiles is a genuine, previously-undocumented divergence —
// not one this round declares "conformant" and not one it silently
// approximates. The Node reference crashes with an UNCAUGHT TypeError
// partway through its run (scripts/kernel-validate.mjs, "entries.map is not
// a function" at the `comparePool` call site for stack-profiles): every
// diagnostic it would otherwise have produced for the checks that already
// ran is lost, because `ok()`/`fail()`/`warn()` only print at a normal exit,
// never as each check completes. `meridian-cli/src/commands/validate/stack_profiles.rs`
// (this round's own fix) reports one authored FAIL and completes normally,
// with the rest of `validate` still running to a well-formed JSON result.
// Both sides correctly refuse a zero/success exit code for this input
// (neither fails open), but by two very different mechanisms — an opaque
// crash losing all information vs. a legible, complete negative result —
// and this test pins that exact boundary directly against both real
// binaries, rather than through the generic conformant/divergent harness
// machinery (which would need at least one matching structured diagnostic
// on each side to compare, and the Node side here produces none at all).
// The Node reference itself is not modified — only observed, per
// instruction: "Node-эталон не исправлять без отдельного назначения
// владельца."
// ---------------------------------------------------------------------------
{
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'conformance-harness-stack-profiles-wrong-type-'));
  createdTempDirs.push(dir);
  try {
    copyRepoWithoutGitOrTarget(dir);
    fs.writeFileSync(
      path.join(dir, 'stack-profiles', 'stack-profiles.yaml'),
      'schema_version: 1\nprofiles: true\n',
    );

    const nodeRun = spawnSync(process.execPath, [path.join(ROOT, 'scripts', 'kernel-validate.mjs')], {
      cwd: ROOT,
      env: kernelOnlyEnv({ MERIDIAN_KERNEL: dir }),
      encoding: 'utf8',
    });
    check('намеренная граница: Node-эталон падает с необработанным TypeError на profiles неверного типа, теряя все диагностики (не fail-open)', () => {
      assert(
        nodeRun.error === undefined,
        `spawnSync не должен был сообщить об ошибке запуска, получено: ${nodeRun.error}`,
      );
      assert(nodeRun.signal === null, `ожидался процесс, завершившийся без сигнала, получено signal: ${nodeRun.signal}`);
      assert(nodeRun.status === 1, `ожидался код завершения Node 1 (необработанное исключение), получено ${nodeRun.status}`);
      assert(
        nodeRun.stdout.trim() === '',
        `Node буферизует все ok()/fail()/warn() до нормального завершения; авария должна терять их полностью, получено stdout: ${JSON.stringify(nodeRun.stdout)}`,
      );
      assert(
        /TypeError/.test(nodeRun.stderr) && /entries\.map is not a function/.test(nodeRun.stderr),
        `ожидался необработанный TypeError "entries.map is not a function" на stderr, получено: ${nodeRun.stderr}`,
      );
    });

    const meridianBin = path.join(ROOT, 'target', 'debug', process.platform === 'win32' ? 'meridian.exe' : 'meridian');
    const rustRun = spawnSync(meridianBin, ['validate', '--kernel', dir, '--format', 'json'], {
      encoding: 'utf8',
      env: kernelOnlyEnv({ MERIDIAN_KERNEL: dir }),
    });
    check('намеренная граница: реальный `meridian validate` на profiles неверного типа даёт явный authored FAIL и полный JSON-результат, не аварию (не fail-open)', () => {
      assert(
        rustRun.status === 1,
        `ожидался exit_code::DOMAIN_NEGATIVE (1), получено ${rustRun.status}; stderr: ${rustRun.stderr}`,
      );
      assert(
        rustRun.stderr.trim() === '',
        `ожидался пустой stderr на валидном JSON-результате, получено: ${rustRun.stderr}`,
      );
      let value;
      try {
        value = JSON.parse(rustRun.stdout.trim());
      } catch (e) {
        throw new Error(`ожидался ровно один JSON-документ на stdout, получено: ${JSON.stringify(rustRun.stdout)} (${e.message})`);
      }
      assert(value.status === 'fail', `ожидался status: "fail", получено ${JSON.stringify(value.status)}`);
      assert(value.result.ok === false, `ожидался result.ok: false, получено ${JSON.stringify(value.result.ok)}`);
      assert(
        value.result.failures.some((f) => f.startsWith('stack-profiles: "profiles" must be a list')),
        `ожидался authored FAIL с префиксом \`stack-profiles: "profiles" must be a list\`, получено: ${JSON.stringify(value.result.failures)}`,
      );
    });
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
    createdTempDirs.splice(createdTempDirs.indexOf(dir), 1);
  }
}

// ---------------------------------------------------------------------------
// Package `meridian-cli-foundation-architecture-remediation`
// (`meridian-rust-migration-program-plan.md` §5.16), corrective round, item
// 7: sha-provenance path confinement. `scripts/kernel-validate.mjs` resolves
// a pin's `artifact`/`source_archive.path` with `path.join(dir, ...)`, which
// does not confine the result to `dir` — a pinned name of `../../<file>`
// walks back out of the skill's own directory and is verified against
// whatever real file sits there. `meridian_core::types::WorkspaceRelativePath`
// rejects a `..` component at construction, so the Rust CLI reports the
// SAME "which does not exist"/"source archive ... is missing" text an
// ordinary missing file already produces — see this suite's own
// `COMPATIBILITY.md` entry and
// `meridian-app/src/validation/mechanical_integrity/sha_provenance.rs`'s own
// doc comment. Both fields are exercised, on the SAME real mutated Kernel
// tree (`copyRepoWithoutGitOrTarget`), each with its own escaping skill so
// the two cases never interfere with each other's digest check.
// ---------------------------------------------------------------------------
function shaProvenancePathConfinementCase(fieldLabel, mutate) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), `conformance-harness-sha-provenance-escape-${fieldLabel}-`));
  createdTempDirs.push(dir);
  try {
    copyRepoWithoutGitOrTarget(dir);
    const skillName = `escape-test-skill-${fieldLabel}`;
    fs.mkdirSync(path.join(dir, 'skills', skillName), { recursive: true });
    mutate(dir, skillName);

    const nodeRun = spawnSync(process.execPath, [path.join(ROOT, 'scripts', 'kernel-validate.mjs')], {
      cwd: ROOT,
      env: kernelOnlyEnv({ MERIDIAN_KERNEL: dir }),
      encoding: 'utf8',
    });
    check(`намеренная граница: Node-эталон следует за экранирующим ${fieldLabel} и верифицирует файл ВНЕ каталога скилла (path.join не ограничивает)`, () => {
      assert(nodeRun.error === undefined, `spawnSync не должен был сообщить об ошибке запуска, получено: ${nodeRun.error}`);
      assert(
        nodeRun.stdout.includes(`${skillName} matches its pin`) ||
          nodeRun.stdout.includes(`${skillName} source archive matches its pin`),
        `ожидалась строка OK, подтверждающая, что Node прочитал файл вне каталога скилла и его дайджест совпал, получено stdout: ${JSON.stringify(nodeRun.stdout)}`,
      );
      assert(
        !nodeRun.stdout.includes(`FAIL  sha-provenance: ${skillName}`),
        `Node не должен был сообщить об отказе для этого скилла — тест доказывает обратное (эталон следует за экранирующим путём), получено stdout: ${JSON.stringify(nodeRun.stdout)}`,
      );
    });

    const meridianBin = path.join(ROOT, 'target', 'debug', process.platform === 'win32' ? 'meridian.exe' : 'meridian');
    const rustRun = spawnSync(meridianBin, ['validate', '--kernel', dir, '--format', 'json'], {
      encoding: 'utf8',
      env: kernelOnlyEnv({ MERIDIAN_KERNEL: dir }),
    });
    check(`намеренная граница: реальный \`meridian validate\` отклоняет экранирующий ${fieldLabel} как несуществующий (WorkspaceRelativePath не пересекает границу скилла)`, () => {
      assert(rustRun.stderr.trim() === '', `ожидался пустой stderr, получено: ${rustRun.stderr}`);
      let value;
      try {
        value = JSON.parse(rustRun.stdout.trim());
      } catch (e) {
        throw new Error(`ожидался ровно один JSON-документ на stdout, получено: ${JSON.stringify(rustRun.stdout)} (${e.message})`);
      }
      const relevant = value.result.failures.filter((f) => f.includes(skillName));
      assert(relevant.length > 0, `ожидался хотя бы один FAIL, упоминающий ${skillName}, получено: ${JSON.stringify(value.result.failures)}`);
      assert(
        relevant.every((f) => f.includes('which does not exist') || f.includes('is missing')),
        `ожидался ТОТ ЖЕ текст диагностики, что и для обычного отсутствующего файла, получено: ${JSON.stringify(relevant)}`,
      );
    });
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
    createdTempDirs.splice(createdTempDirs.indexOf(dir), 1);
  }
}

shaProvenancePathConfinementCase('artifact', (dir, skillName) => {
  const secretContent = 'top secret, outside the skill directory\n';
  const secretDigest = crypto.createHash('sha256').update(secretContent, 'utf8').digest('hex');
  fs.writeFileSync(path.join(dir, 'escaped-secret.txt'), secretContent);
  fs.writeFileSync(
    path.join(dir, 'skills', skillName, 'PIN.yaml'),
    `artifact: ../../escaped-secret.txt\nsha256: ${secretDigest}\n`,
  );
});

shaProvenancePathConfinementCase('source_archive_path', (dir, skillName) => {
  const artifactContent = 'ok\n';
  const artifactDigest = crypto.createHash('sha256').update(artifactContent, 'utf8').digest('hex');
  const archiveBytes = Buffer.from('not really a zip, but real bytes outside the skill directory');
  const archiveDigest = crypto.createHash('sha256').update(archiveBytes).digest('hex');
  fs.writeFileSync(path.join(dir, 'skills', skillName, 'SKILL.md'), artifactContent);
  fs.writeFileSync(path.join(dir, 'escaped-archive.zip'), archiveBytes);
  fs.writeFileSync(
    path.join(dir, 'skills', skillName, 'PIN.yaml'),
    `artifact: SKILL.md\nsha256: ${artifactDigest}\nsource_archive:\n  path: ../../escaped-archive.zip\n  sha256: ${archiveDigest}\n`,
  );
});

// Positive counterpart, on the SAME real repository copy: an ordinary
// confined relative path (including a subdirectory) must still verify
// cleanly on BOTH sides — the confinement fix rejects only an escaping
// path, never a normal one.
{
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'conformance-harness-sha-provenance-confined-'));
  createdTempDirs.push(dir);
  try {
    copyRepoWithoutGitOrTarget(dir);
    const skillName = 'confined-nested-skill';
    const content = 'nested body\n';
    const digest = crypto.createHash('sha256').update(content, 'utf8').digest('hex');
    // A lower-kebab-case, non-Markdown nested path on purpose: this fixture
    // only proves the confined-path join itself, and a `.md` name would
    // also trip the UNRELATED `document-identity` check (naming/Front
    // Matter), which is not what this test is about.
    fs.mkdirSync(path.join(dir, 'skills', skillName, 'docs'), { recursive: true });
    fs.writeFileSync(path.join(dir, 'skills', skillName, 'docs', 'artifact.txt'), content);
    fs.writeFileSync(
      path.join(dir, 'skills', skillName, 'PIN.yaml'),
      `artifact: docs/artifact.txt\nsha256: ${digest}\n`,
    );

    const nodeRun = spawnSync(process.execPath, [path.join(ROOT, 'scripts', 'kernel-validate.mjs')], {
      cwd: ROOT,
      env: kernelOnlyEnv({ MERIDIAN_KERNEL: dir }),
      encoding: 'utf8',
    });
    check('позитивный контроль: обычный вложенный относительный путь артефакта верифицируется на Node-эталоне', () => {
      assert(nodeRun.stdout.includes(`${skillName} matches its pin`), `получено stdout: ${JSON.stringify(nodeRun.stdout)}`);
    });

    const meridianBin = path.join(ROOT, 'target', 'debug', process.platform === 'win32' ? 'meridian.exe' : 'meridian');
    const rustRun = spawnSync(meridianBin, ['validate', '--kernel', dir, '--format', 'json'], {
      encoding: 'utf8',
      env: kernelOnlyEnv({ MERIDIAN_KERNEL: dir }),
    });
    check('позитивный контроль: обычный вложенный относительный путь артефакта верифицируется через WorkspaceRelativePath (Rust)', () => {
      let value;
      try {
        value = JSON.parse(rustRun.stdout.trim());
      } catch (e) {
        throw new Error(`ожидался ровно один JSON-документ на stdout, получено: ${JSON.stringify(rustRun.stdout)} (${e.message})`);
      }
      const relevant = value.result.failures.filter((f) => f.includes(skillName));
      assert(relevant.length === 0, `ожидалось отсутствие FAIL для ${skillName}, получено: ${JSON.stringify(relevant)}`);
    });
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
    createdTempDirs.splice(createdTempDirs.indexOf(dir), 1);
  }
}

// ---------------------------------------------------------------------------
// Package `meridian-cli-foundation-architecture-remediation`, second
// corrective round, item 4: the two architect-approved intentional
// differences from the first corrective round's item 2 — an incomplete
// `source_archive` declaration (`sha-provenance`) and an `operating-foundation`
// data entry with no `id` — each finalized here with a real Node/Rust
// mutated-tree comparison, on the SAME fixture, asserting the EXACT
// diagnostic/verdict difference. Node is never modified to agree.
// ---------------------------------------------------------------------------
{
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'conformance-harness-sha-provenance-incomplete-archive-'));
  createdTempDirs.push(dir);
  try {
    copyRepoWithoutGitOrTarget(dir);
    const skillName = 'incomplete-archive-skill';
    const artifactContent = 'ok\n';
    const artifactDigest = crypto.createHash('sha256').update(artifactContent, 'utf8').digest('hex');
    fs.mkdirSync(path.join(dir, 'skills', skillName), { recursive: true });
    fs.writeFileSync(path.join(dir, 'skills', skillName, 'SKILL.md'), artifactContent);
    // `source_archive` declares only `path`, never `sha256` — Node's own
    // `if (arch?.path && arch?.sha256)` guard is false, so the whole block
    // is skipped in silence (no ok, no fail, no warn at all for it).
    fs.writeFileSync(
      path.join(dir, 'skills', skillName, 'PIN.yaml'),
      `artifact: SKILL.md\nsha256: ${artifactDigest}\nsource_archive:\n  path: source/archive.zip\n`,
    );

    const nodeRun = spawnSync(process.execPath, [path.join(ROOT, 'scripts', 'kernel-validate.mjs')], {
      cwd: ROOT,
      env: kernelOnlyEnv({ MERIDIAN_KERNEL: dir }),
      encoding: 'utf8',
    });
    check('намеренная граница: Node-эталон молча пропускает неполный source_archive (ни OK, ни FAIL)', () => {
      assert(nodeRun.error === undefined, `spawnSync не должен был сообщить об ошибке запуска, получено: ${nodeRun.error}`);
      assert(
        nodeRun.stdout.includes(`${skillName} matches its pin`),
        `ожидался OK для самого артефакта, получено stdout: ${JSON.stringify(nodeRun.stdout)}`,
      );
      assert(
        !nodeRun.stdout.includes(`${skillName} source archive`),
        `Node не должен был сообщить ни OK, ни FAIL про source archive этого скилла вообще, получено stdout: ${JSON.stringify(nodeRun.stdout)}`,
      );
    });

    const meridianBin = path.join(ROOT, 'target', 'debug', process.platform === 'win32' ? 'meridian.exe' : 'meridian');
    const rustRun = spawnSync(meridianBin, ['validate', '--kernel', dir, '--format', 'json'], {
      encoding: 'utf8',
      env: kernelOnlyEnv({ MERIDIAN_KERNEL: dir }),
    });
    check('намеренная граница: реальный `meridian validate` сообщает FAIL про неполный source_archive (Incomplete — ошибка конструктора, а не домен-вариант)', () => {
      assert(rustRun.stderr.trim() === '', `ожидался пустой stderr, получено: ${rustRun.stderr}`);
      let value;
      try {
        value = JSON.parse(rustRun.stdout.trim());
      } catch (e) {
        throw new Error(`ожидался ровно один JSON-документ на stdout, получено: ${JSON.stringify(rustRun.stdout)} (${e.message})`);
      }
      const relevant = value.result.failures.filter((f) => f.includes(skillName));
      assert(relevant.length === 1, `ожидался ровно один FAIL про ${skillName}, получено: ${JSON.stringify(value.result.failures)}`);
      assert(
        relevant[0].includes('missing path, sha256, or both'),
        `получено: ${JSON.stringify(relevant)}`,
      );
    });
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
    createdTempDirs.splice(createdTempDirs.indexOf(dir), 1);
  }
}

{
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'conformance-harness-operating-foundation-entry-without-id-'));
  createdTempDirs.push(dir);
  try {
    copyRepoWithoutGitOrTarget(dir);
    const yamlPath = path.join(dir, 'standards', 'workspace', 'operating-foundation.yaml');
    const original = fs.readFileSync(yamlPath, 'utf8');
    // Adds one raw `terms[]` entry with no `id` at all, immediately after
    // the `terms:` key — Node's own `String(entry?.id ?? '')` plus its
    // every truthy-`id` guard (`duplicateData`'s filter, `undocumented`'s
    // filter) makes this entry invisible to every one of `comparePool`'s
    // checks; it produces neither OK nor FAIL. Every other real term/row
    // stays untouched, so this mutation cannot introduce any OTHER
    // disagreement on either side.
    assert(original.includes('\nterms:\n'), 'ожидался ключ "terms:" в operating-foundation.yaml для мутации');
    const mutated = original.replace(
      '\nterms:\n',
      '\nterms:\n  - canonical_ru: "Без идентификатора"\n    canonical_en: "No id"\n',
    );
    fs.writeFileSync(yamlPath, mutated);

    const nodeRun = spawnSync(process.execPath, [path.join(ROOT, 'scripts', 'kernel-validate.mjs')], {
      cwd: ROOT,
      env: kernelOnlyEnv({ MERIDIAN_KERNEL: dir }),
      encoding: 'utf8',
    });
    check('намеренная граница: Node-эталон молча пропускает entry без id в operating-foundation (ни один FAIL про него)', () => {
      assert(nodeRun.error === undefined, `spawnSync не должен был сообщить об ошибке запуска, получено: ${nodeRun.error}`);
      assert(
        !nodeRun.stdout.includes('have no id'),
        `Node не должен был сообщить ни про один entry без id, получено stdout: ${JSON.stringify(nodeRun.stdout)}`,
      );
      assert(
        !nodeRun.stdout.includes('FAIL  operating-foundation:'),
        `остальные term/principle записи не должны были начать расходиться, получено stdout: ${JSON.stringify(nodeRun.stdout)}`,
      );
    });

    const meridianBin = path.join(ROOT, 'target', 'debug', process.platform === 'win32' ? 'meridian.exe' : 'meridian');
    const rustRun = spawnSync(meridianBin, ['validate', '--kernel', dir, '--format', 'json'], {
      encoding: 'utf8',
      env: kernelOnlyEnv({ MERIDIAN_KERNEL: dir }),
    });
    check('намеренная граница: реальный `meridian validate` сообщает FAIL про entry без id (транспортный уровень считает и сообщает, домен строит только валидные)', () => {
      assert(rustRun.stderr.trim() === '', `ожидался пустой stderr, получено: ${rustRun.stderr}`);
      let value;
      try {
        value = JSON.parse(rustRun.stdout.trim());
      } catch (e) {
        throw new Error(`ожидался ровно один JSON-документ на stdout, получено: ${JSON.stringify(rustRun.stdout)} (${e.message})`);
      }
      const relevant = value.result.failures.filter((f) => f.includes('have no id'));
      assert(relevant.length === 1, `ожидался ровно один FAIL про entry без id, получено: ${JSON.stringify(value.result.failures)}`);
      assert(relevant[0].includes('1 term entry'), `получено: ${JSON.stringify(relevant)}`);
      const otherFoundationFailures = value.result.failures.filter(
        (f) => f.includes('operating-foundation:') && !f.includes('have no id'),
      );
      assert(
        otherFoundationFailures.length === 0,
        `остальные term/principle записи не должны были начать расходиться, получено: ${JSON.stringify(otherFoundationFailures)}`,
      );
    });
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
    createdTempDirs.splice(createdTempDirs.indexOf(dir), 1);
  }
}

// ---------------------------------------------------------------------------
// Package `rust-architecture-conformance-4` (§5.18.3 point 7), corrective
// round item 5: the one pre-accepted schema-I/O divergence — Node's
// `readIfExists` (`scripts/kernel-validate.mjs`) catches ANY error reading
// `verification/functional-parity/functional-parity-evidence.schema.json`,
// not only `ENOENT`, and folds all of them into the same silent
// `info('...nothing to check')` skip; the Rust port
// (`meridian_app::operating_model::functional_parity::evaluate`) keeps that
// behaviour ONLY for `ReadError::NotFound` and now reports a `FAIL` for any
// other I/O failure. A directory sitting where the schema FILE is expected
// is the safe, portable way to force a non-`ENOENT` read failure on both
// sides without relying on platform-specific permission bits: `fs.readFileSync`
// on a directory throws `EISDIR` (still caught by `readIfExists`), and Rust's
// `fs::read_to_string` returns an `Err` whose `ErrorKind` is not `NotFound`
// either way `meridian_app::workspace::ReadError`'s own `map_io_error`
// classifies it. Manually confirmed on this real toolchain before being
// written here: Node prints exactly `INFO  functional-parity: no PHASE D
// evidence schema in this Kernel; nothing to check` and no functional-parity
// FAIL; the real compiled `meridian` binary reports exactly one FAIL,
// `functional-parity: functional-parity-evidence.schema.json: Is a
// directory (os error 21)` (message text is OS-dependent, so only the
// prefix and basename are asserted below, not the trailing OS string).
// ---------------------------------------------------------------------------
{
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'conformance-harness-functional-parity-schema-io-'));
  createdTempDirs.push(dir);
  try {
    copyRepoWithoutGitOrTarget(dir);
    const schemaPath = path.join(
      dir,
      'verification',
      'functional-parity',
      'functional-parity-evidence.schema.json',
    );
    fs.rmSync(schemaPath);
    fs.mkdirSync(schemaPath);

    const nodeRun = spawnSync(process.execPath, [path.join(ROOT, 'scripts', 'kernel-validate.mjs')], {
      cwd: ROOT,
      env: kernelOnlyEnv({ MERIDIAN_KERNEL: dir }),
      encoding: 'utf8',
    });
    check('намеренная граница: Node-эталон молча пропускает functional-parity, когда путь схемы — каталог (readIfExists глотает EISDIR так же, как ENOENT)', () => {
      assert(nodeRun.error === undefined, `spawnSync не должен был сообщить об ошибке запуска, получено: ${nodeRun.error}`);
      assert(
        nodeRun.stdout.includes('functional-parity: no PHASE D evidence schema in this Kernel; nothing to check'),
        `ожидалась строка INFO про "nothing to check", получено stdout: ${JSON.stringify(nodeRun.stdout)}`,
      );
      assert(
        !nodeRun.stdout.includes('FAIL  functional-parity:'),
        `Node не должен был сообщить ни один FAIL для functional-parity, получено stdout: ${JSON.stringify(nodeRun.stdout)}`,
      );
    });

    const meridianBin = path.join(ROOT, 'target', 'debug', process.platform === 'win32' ? 'meridian.exe' : 'meridian');
    const rustRun = spawnSync(meridianBin, ['validate', '--kernel', dir, '--format', 'json'], {
      encoding: 'utf8',
      env: kernelOnlyEnv({ MERIDIAN_KERNEL: dir }),
    });
    check('намеренная граница: реальный `meridian validate` сообщает FAIL, когда путь схемы functional-parity — каталог, а не ENOENT (fail-closed, не молчаливый skip)', () => {
      assert(rustRun.stderr.trim() === '', `ожидался пустой stderr, получено: ${rustRun.stderr}`);
      let value;
      try {
        value = JSON.parse(rustRun.stdout.trim());
      } catch (e) {
        throw new Error(`ожидался ровно один JSON-документ на stdout, получено: ${JSON.stringify(rustRun.stdout)} (${e.message})`);
      }
      const relevant = value.result.failures.filter((f) => f.startsWith('functional-parity: functional-parity-evidence.schema.json:'));
      assert(
        relevant.length === 1,
        `ожидался ровно один FAIL с префиксом "functional-parity: functional-parity-evidence.schema.json:", получено: ${JSON.stringify(value.result.failures)}`,
      );
      const otherFunctionalParityFailures = value.result.failures.filter(
        (f) => f.startsWith('functional-parity:') && !relevant.includes(f),
      );
      assert(
        otherFunctionalParityFailures.length === 0,
        `не должно быть других functional-parity FAIL, получено: ${JSON.stringify(otherFunctionalParityFailures)}`,
      );
    });
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
    createdTempDirs.splice(createdTempDirs.indexOf(dir), 1);
  }
}

// ---------------------------------------------------------------------------
// Package `rust-architecture-conformance-4`, second corrective round item 3:
// the fixture-I/O boundary (`COMPATIBILITY.md`, functional-parity boundary 2
// of 2). Once the schema itself is readable, a non-`ENOENT` failure reading
// `verification/functional-parity/fixtures/functional-parity-evidence.fixtures.json`
// is folded by Node's `readIfExists` into `null` and reported with the SAME
// generic "carries no fixtures" text as a missing file; the Rust port
// distinguishes it and reports "<fixtures path> could not be read: ...".
// The verdict is failing on BOTH sides — only the exact diagnostic text
// differs. As in the schema-I/O case above, a directory standing where the
// fixtures FILE is expected is the portable non-`ENOENT` failure; the
// OS-dependent tail of the Rust message is not compared.
// ---------------------------------------------------------------------------
{
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'conformance-harness-functional-parity-fixtures-io-'));
  createdTempDirs.push(dir);
  try {
    copyRepoWithoutGitOrTarget(dir);
    const fixturesPath = path.join(
      dir,
      'verification',
      'functional-parity',
      'fixtures',
      'functional-parity-evidence.fixtures.json',
    );
    fs.rmSync(fixturesPath);
    fs.mkdirSync(fixturesPath);

    const nodeRun = spawnSync(process.execPath, [path.join(ROOT, 'scripts', 'kernel-validate.mjs')], {
      cwd: ROOT,
      env: kernelOnlyEnv({ MERIDIAN_KERNEL: dir }),
      encoding: 'utf8',
    });
    check('намеренная граница: Node-эталон сообщает generic "carries no fixtures", когда путь fixtures functional-parity — каталог (readIfExists глотает EISDIR)', () => {
      assert(nodeRun.error === undefined, `spawnSync не должен был сообщить об ошибке запуска, получено: ${nodeRun.error}`);
      assert(nodeRun.status !== 0, `Node должен остаться failing, получен код ${nodeRun.status}`);
      const relevant = nodeRun.stdout
        .split(/\r?\n/)
        .filter((l) => l.startsWith('FAIL  functional-parity:'));
      assert(
        relevant.length === 1,
        `ожидался ровно один functional-parity FAIL, получено: ${JSON.stringify(relevant)}`,
      );
      assert(
        relevant[0].startsWith('FAIL  functional-parity: the PHASE D evidence schema carries no fixtures ('),
        `ожидался generic "carries no fixtures", получено: ${JSON.stringify(relevant[0])}`,
      );
    });

    const meridianBin = path.join(ROOT, 'target', 'debug', process.platform === 'win32' ? 'meridian.exe' : 'meridian');
    const rustRun = spawnSync(meridianBin, ['validate', '--kernel', dir, '--format', 'json'], {
      encoding: 'utf8',
      env: kernelOnlyEnv({ MERIDIAN_KERNEL: dir }),
    });
    check('намеренная граница: реальный `meridian validate` сообщает отдельный "could not be read", когда путь fixtures functional-parity — каталог (вердикт failing, как и у Node)', () => {
      assert(rustRun.stderr.trim() === '', `ожидался пустой stderr, получено: ${rustRun.stderr}`);
      assert(rustRun.status !== 0, `Rust должен остаться failing, получен код ${rustRun.status}`);
      let value;
      try {
        value = JSON.parse(rustRun.stdout.trim());
      } catch (e) {
        throw new Error(`ожидался ровно один JSON-документ на stdout, получено: ${JSON.stringify(rustRun.stdout)} (${e.message})`);
      }
      assert(value.result.ok === false, `ожидался result.ok: false, получено ${JSON.stringify(value.result.ok)}`);
      const relevant = value.result.failures.filter((f) => f.startsWith('functional-parity:'));
      assert(
        relevant.length === 1,
        `ожидался ровно один functional-parity FAIL, получено: ${JSON.stringify(relevant)}`,
      );
      assert(
        relevant[0].startsWith(
          'functional-parity: verification/functional-parity/fixtures/functional-parity-evidence.fixtures.json could not be read:',
        ),
        `ожидался отдельный "could not be read", получено: ${JSON.stringify(relevant[0])}`,
      );
      assert(
        !relevant[0].includes('carries no fixtures'),
        `Rust не должен сворачивать I/O-ошибку в generic "carries no fixtures", получено: ${JSON.stringify(relevant[0])}`,
      );
    });
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
    createdTempDirs.splice(createdTempDirs.indexOf(dir), 1);
  }
}

// ---------------------------------------------------------------------------
// Package `rust-architecture-conformance-5` (§5.19.3 point 10): the
// mandatory-read I/O boundary of `execution-state-model`,
// `role-and-human-control` and `bounded-context-manifest` (`COMPATIBILITY.md`).
// Every file of the three families is mandatory. Node's `readIfExists` folds
// ANY read error into `null`, so an existing-but-unreadable file is reported
// with the same "is missing" / "carries no fixtures" text as an absent one;
// the Rust port keeps that text only for `ReadError::NotFound` and reports
// any other `ReadError::Io` as "<path> could not be read: …". One mutated
// tree replaces one mandatory file per family with a directory (the portable
// non-`ENOENT` failure): the schema of `execution-state-model`, the canonical
// catalogue of `role-and-human-control`, the fixture bundle of
// `bounded-context-manifest`. Both sides stay failing with exactly one FAIL
// per family; only the text differs. Manually confirmed on this toolchain
// before being written here; the OS-dependent tail is not compared.
// ---------------------------------------------------------------------------
{
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'conformance-harness-run-contracts-mandatory-io-'));
  createdTempDirs.push(dir);
  try {
    copyRepoWithoutGitOrTarget(dir);
    const cases = [
      {
        family: 'execution-state-model',
        rel: 'registries/operating-model/execution-state.schema.json',
        nodeText: 'execution-state-model: registries/operating-model/execution-state.schema.json is missing; ',
      },
      {
        family: 'role-and-human-control',
        rel: 'standards/workspace/role-registry.yaml',
        nodeText: 'role-and-human-control: standards/workspace/role-registry.yaml is missing; ',
      },
      {
        family: 'bounded-context-manifest',
        rel: 'registries/operating-model/fixtures/context-manifest.fixtures.json',
        nodeText: 'bounded-context-manifest: the schema carries no fixtures (registries/operating-model/fixtures/context-manifest.fixtures.json); ',
      },
    ];
    for (const c of cases) {
      const target = path.join(dir, ...c.rel.split('/'));
      fs.rmSync(target);
      fs.mkdirSync(target);
    }

    const nodeRun = spawnSync(process.execPath, [path.join(ROOT, 'scripts', 'kernel-validate.mjs')], {
      cwd: ROOT,
      env: kernelOnlyEnv({ MERIDIAN_KERNEL: dir }),
      encoding: 'utf8',
    });
    check('намеренная граница: Node-эталон сообщает прежний "is missing"/"carries no fixtures", когда обязательный файл execution-state-model / role-and-human-control / bounded-context-manifest — каталог (readIfExists глотает EISDIR)', () => {
      assert(nodeRun.error === undefined, `spawnSync не должен был сообщить об ошибке запуска, получено: ${nodeRun.error}`);
      assert(nodeRun.status !== 0, `Node должен остаться failing, получен код ${nodeRun.status}`);
      const lines = nodeRun.stdout.split(/\r?\n/);
      for (const c of cases) {
        const relevant = lines.filter((l) => l.startsWith(`FAIL  ${c.family}:`));
        assert(relevant.length === 1, `ожидался ровно один ${c.family} FAIL, получено: ${JSON.stringify(relevant)}`);
        assert(
          relevant[0].startsWith(`FAIL  ${c.nodeText}`),
          `ожидался прежний Node-текст "${c.nodeText}", получено: ${JSON.stringify(relevant[0])}`,
        );
      }
    });

    const meridianBin = path.join(ROOT, 'target', 'debug', process.platform === 'win32' ? 'meridian.exe' : 'meridian');
    const rustRun = spawnSync(meridianBin, ['validate', '--kernel', dir, '--format', 'json'], {
      encoding: 'utf8',
      env: kernelOnlyEnv({ MERIDIAN_KERNEL: dir }),
    });
    check('намеренная граница: реальный `meridian validate` сообщает отдельный "<путь> could not be read", когда обязательный файл трёх run-contract семейств — каталог (вердикт failing, как и у Node)', () => {
      assert(rustRun.stderr.trim() === '', `ожидался пустой stderr, получено: ${rustRun.stderr}`);
      assert(rustRun.status !== 0, `Rust должен остаться failing, получен код ${rustRun.status}`);
      let value;
      try {
        value = JSON.parse(rustRun.stdout.trim());
      } catch (e) {
        throw new Error(`ожидался ровно один JSON-документ на stdout, получено: ${JSON.stringify(rustRun.stdout)} (${e.message})`);
      }
      assert(value.result.ok === false, `ожидался result.ok: false, получено ${JSON.stringify(value.result.ok)}`);
      for (const c of cases) {
        const relevant = value.result.failures.filter((f) => f.startsWith(`${c.family}:`));
        assert(relevant.length === 1, `ожидался ровно один ${c.family} FAIL, получено: ${JSON.stringify(relevant)}`);
        assert(
          relevant[0].startsWith(`${c.family}: ${c.rel} could not be read:`),
          `ожидался отдельный "could not be read", получено: ${JSON.stringify(relevant[0])}`,
        );
        assert(
          !relevant[0].includes('is missing') && !relevant[0].includes('carries no fixtures'),
          `Rust не должен сворачивать I/O-ошибку в текст отсутствия, получено: ${JSON.stringify(relevant[0])}`,
        );
      }
    });
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
    createdTempDirs.splice(createdTempDirs.indexOf(dir), 1);
  }
}

// ---------------------------------------------------------------------------
// Package `rust-architecture-conformance-6` (§5.20.3 points 10 and 14):
// the mandatory-read I/O boundary of `evidence-and-handoff-contract` and
// `meridian-field-evaluation` (`COMPATIBILITY.md`), the same accepted
// Rust-native distinction package 5 established for the run-contract
// families. One mutated tree replaces the fixture bundle of the handoff
// family and the schema of the field-evaluation family with directories
// (the portable non-`ENOENT` failure). Node's `readIfExists` reports its
// old "carries no fixtures" / "is missing" text; the Rust port reports the
// distinct "<path> could not be read: …". Both sides stay failing with
// exactly one FAIL per family; the OS-dependent tail is not compared.
// ---------------------------------------------------------------------------
{
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'conformance-harness-evidence-field-mandatory-io-'));
  createdTempDirs.push(dir);
  try {
    copyRepoWithoutGitOrTarget(dir);
    const cases = [
      {
        family: 'evidence-and-handoff-contract',
        rel: 'registries/operating-model/fixtures/evidence-and-handoff.fixtures.json',
        nodeText: 'evidence-and-handoff-contract: the schema carries no fixtures (registries/operating-model/fixtures/evidence-and-handoff.fixtures.json); ',
      },
      {
        family: 'meridian-field-evaluation',
        rel: 'registries/operating-model/field-evaluation.schema.json',
        nodeText: 'meridian-field-evaluation: registries/operating-model/field-evaluation.schema.json is missing; ',
      },
    ];
    for (const c of cases) {
      const target = path.join(dir, ...c.rel.split('/'));
      fs.rmSync(target);
      fs.mkdirSync(target);
    }

    const nodeRun = spawnSync(process.execPath, [path.join(ROOT, 'scripts', 'kernel-validate.mjs')], {
      cwd: ROOT,
      env: kernelOnlyEnv({ MERIDIAN_KERNEL: dir }),
      encoding: 'utf8',
    });
    check('намеренная граница: Node-эталон сообщает прежний "carries no fixtures"/"is missing", когда обязательный файл evidence-and-handoff-contract / meridian-field-evaluation — каталог (readIfExists глотает EISDIR)', () => {
      assert(nodeRun.error === undefined, `spawnSync не должен был сообщить об ошибке запуска, получено: ${nodeRun.error}`);
      assert(nodeRun.status !== 0, `Node должен остаться failing, получен код ${nodeRun.status}`);
      const lines = nodeRun.stdout.split(/\r?\n/);
      for (const c of cases) {
        const relevant = lines.filter((l) => l.startsWith(`FAIL  ${c.family}:`));
        assert(relevant.length === 1, `ожидался ровно один ${c.family} FAIL, получено: ${JSON.stringify(relevant)}`);
        assert(
          relevant[0].startsWith(`FAIL  ${c.nodeText}`),
          `ожидался прежний Node-текст "${c.nodeText}", получено: ${JSON.stringify(relevant[0])}`,
        );
      }
    });

    const meridianBin = path.join(ROOT, 'target', 'debug', process.platform === 'win32' ? 'meridian.exe' : 'meridian');
    const rustRun = spawnSync(meridianBin, ['validate', '--kernel', dir, '--format', 'json'], {
      encoding: 'utf8',
      env: kernelOnlyEnv({ MERIDIAN_KERNEL: dir }),
    });
    check('намеренная граница: реальный `meridian validate` сообщает отдельный "<путь> could not be read", когда обязательный файл evidence-and-handoff-contract / meridian-field-evaluation — каталог (вердикт failing, как и у Node)', () => {
      assert(rustRun.stderr.trim() === '', `ожидался пустой stderr, получено: ${rustRun.stderr}`);
      assert(rustRun.status !== 0, `Rust должен остаться failing, получен код ${rustRun.status}`);
      let value;
      try {
        value = JSON.parse(rustRun.stdout.trim());
      } catch (e) {
        throw new Error(`ожидался ровно один JSON-документ на stdout, получено: ${JSON.stringify(rustRun.stdout)} (${e.message})`);
      }
      assert(value.result.ok === false, `ожидался result.ok: false, получено ${JSON.stringify(value.result.ok)}`);
      for (const c of cases) {
        const relevant = value.result.failures.filter((f) => f.startsWith(`${c.family}:`));
        assert(relevant.length === 1, `ожидался ровно один ${c.family} FAIL, получено: ${JSON.stringify(relevant)}`);
        assert(
          relevant[0].startsWith(`${c.family}: ${c.rel} could not be read:`),
          `ожидался отдельный "could not be read", получено: ${JSON.stringify(relevant[0])}`,
        );
        assert(
          !relevant[0].includes('is missing') && !relevant[0].includes('carries no fixtures'),
          `Rust не должен сворачивать I/O-ошибку в текст отсутствия, получено: ${JSON.stringify(relevant[0])}`,
        );
      }
    });
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
    createdTempDirs.splice(createdTempDirs.indexOf(dir), 1);
  }
}

// ---------------------------------------------------------------------------
// Package `rust-architecture-conformance-6` (§5.20.3 point 14): the Node
// halves of the two matched library-level schema short-circuit cases
// (`COMPATIBILITY.md`). The Rust halves are
// `meridian-app/src/operating_model/evidence_and_handoff/tests.rs::evidence_and_handoff_a_schema_violation_short_circuits_before_the_domain`
// and `meridian-app/src/operating_model/field_evaluation/tests.rs::field_evaluation_a_schema_violation_short_circuits_before_the_domain`:
// the same real fixture document with the same two mutations — a defect
// ONLY the schema catches (a duplicated `covers` entry against
// `uniqueItems`; an `observed_at` breaking the `date_str` pattern) and an
// independent domain defect. The Rust route stops at the schema gate (only
// schema diagnostics); the Node libraries, called directly (bypassing
// `kernel-validate.mjs`, which reports only `p[0]`, the same schema
// diagnostic on both sides), also compute the domain diagnostics. The
// difference is the full list of SECONDARY diagnostics only.
// ---------------------------------------------------------------------------
{
  const envelopeSchema = JSON.parse(fs.readFileSync(path.join(ROOT, 'registries/operating-model/scoped-record.schema.json'), 'utf8'));
  const ehSchema = JSON.parse(fs.readFileSync(path.join(ROOT, 'registries/operating-model/evidence-and-handoff.schema.json'), 'utf8'));
  const ehBundle = JSON.parse(fs.readFileSync(path.join(ROOT, 'registries/operating-model/fixtures/evidence-and-handoff.fixtures.json'), 'utf8'));
  const handoff = JSON.parse(JSON.stringify(ehBundle.valid[0].spec));
  const covers = handoff.payload.evidence[0].covers;
  assert(Array.isArray(covers) && covers.length > 0, 'evidence-and-handoff.fixtures.json valid[0].payload.evidence[0].covers must be a non-empty array to duplicate');
  covers.push(covers[0]);
  handoff.payload.outcome.statement = '/etc/passwd';
  check('библиотечный (не через kernel-validate.mjs) Node-прогон: evaluateEvidenceAndHandoff возвращает И диагностику схемы (uniqueItems), И независимые доменные диагностики на документе, который Rust-маршрут останавливает на schema gate — парный Rust-тест evidence_and_handoff_a_schema_violation_short_circuits_before_the_domain', () => {
    const problems = evaluateEvidenceAndHandoff(handoff, {
      recordSchema: ehSchema,
      envelopeSchema,
      resolveRecords: makeRecordResolver(ehBundle.resolution),
    });
    assert(problems[0].includes('/payload/evidence/0/covers'), `первой ожидалась диагностика схемы по covers, получено: ${JSON.stringify(problems)}`);
    assert(problems.some((p) => p.includes('outcome statement contains')), `ожидалась доменная диагностика outcome statement, получено: ${JSON.stringify(problems)}`);
    assert(problems.some((p) => p.includes('more than once')), `ожидалась доменная диагностика повторного covers, получено: ${JSON.stringify(problems)}`);
  });

  const feSchema = JSON.parse(fs.readFileSync(path.join(ROOT, 'registries/operating-model/field-evaluation.schema.json'), 'utf8'));
  const feBundle = JSON.parse(fs.readFileSync(path.join(ROOT, 'registries/operating-model/fixtures/field-evaluation.fixtures.json'), 'utf8'));
  const source = feBundle.valid.find((c) => c.note === 'наблюдение obs-mech-correct-1');
  assert(source, 'field-evaluation.fixtures.json must carry a "наблюдение obs-mech-correct-1" valid fixture');
  const observation = JSON.parse(JSON.stringify(source.spec));
  observation.payload.observed_at = '2026-8-3';
  observation.payload.measurement.basis = '/etc/passwd';
  check('библиотечный (не через kernel-validate.mjs) Node-прогон: evaluateFieldEvaluation возвращает И диагностику схемы (date_str pattern), И независимые доменные диагностики на документе, который Rust-маршрут останавливает на schema gate — парный Rust-тест field_evaluation_a_schema_violation_short_circuits_before_the_domain', () => {
    const problems = evaluateFieldEvaluation(observation, {
      recordSchema: feSchema,
      envelopeSchema,
      resolveRecords: makeRecordResolver(feBundle.resolution),
    });
    assert(problems[0].includes('/payload/observed_at'), `первой ожидалась диагностика схемы по observed_at, получено: ${JSON.stringify(problems)}`);
    assert(problems.some((p) => p.includes('observed_at is not a valid date')), `ожидалась доменная диагностика даты, получено: ${JSON.stringify(problems)}`);
    assert(problems.some((p) => p.includes('measurement.basis contains')), `ожидалась доменная диагностика basis, получено: ${JSON.stringify(problems)}`);
  });
}

// ---------------------------------------------------------------------------
// Package `rust-architecture-conformance-7`, corrective round 1: the Node
// halves of the matched library-level cases behind the four accepted
// `COMPATIBILITY.md` boundaries of this package. Each case calls the Node
// library directly (bypassing `kernel-validate.mjs`, which reports only
// `p[0]`) on the SAME real fixture document with the SAME mutation as its
// named Rust half, with the options `kernel-validate.mjs` builds from the
// real schemas and fixture bundles. The expected strings below are the
// Rust route's own output (asserted verbatim by the Rust halves); each
// check proves the Node output differs from it by EXACTLY the accepted
// difference and nothing else.
// ---------------------------------------------------------------------------
{
  const om = (name) => JSON.parse(fs.readFileSync(path.join(ROOT, 'registries', 'operating-model', name), 'utf8'));
  const fixtureBundle = (family) => om(`fixtures/${family}.fixtures.json`);
  const envelope = om('scoped-record.schema.json');
  const planOptions = (b) => ({
    registrySchema: om('instance-data-migration.schema.json'),
    envelopeSchema: envelope,
    resolveSourceSnapshot: makeSourceSnapshotResolver(b.source_snapshot_resolution),
    resolveEvidence: makeEvidenceResolver(b.evidence_resolution),
    resolveRollbackSnapshot: makeRollbackSnapshotResolver(b.rollback_snapshot_resolution),
    resolveDeterministicPlan: makeDeterministicPlanResolver(b.deterministic_plan_resolution),
    resolveRestorationEvidence: makeRestorationEvidenceResolver(b.restoration_evidence_resolution),
    resolveSupersededPlan: makeSupersededPlanResolver(b.superseded_plan_resolution),
  });
  const exportOptions = (b) => ({
    registrySchema: om('instance-canonical-export.schema.json'),
    envelopeSchema: envelope,
    resolveMigrationPlan: makeMigrationPlanResolver(b.plan_resolution),
    resolveSourceContent: makeSourceContentResolver(b.source_content_resolution),
  });
  const workspaceOptions = (b) => {
    const mr = b.migration_resolution || {};
    const er = b.export_resolution || {};
    return {
      registrySchema: om('workspace-compatibility-qualification.schema.json'),
      envelopeSchema: envelope,
      resolveConnectionRecord: makeRefResolver(b.connection_record_resolution),
      resolvePlanRecord: makeRefResolver(b.plan_record_resolution),
      resolveExportRecord: makeRefResolver(b.export_record_resolution),
      compatOptions: {
        registrySchema: om('existing-project-compatibility-mode.schema.json'),
        envelopeSchema: envelope,
        sourceRegistrySchema: om('instruction-source-registry.schema.json'),
        ruleIntakeSchema: om('controlled-rule-intake.schema.json'),
      },
      migrationOptions: planOptions(mr),
      exportOptions: {
        registrySchema: om('instance-canonical-export.schema.json'),
        envelopeSchema: envelope,
        resolveSourceContent: makeSourceContentResolver(er.source_content_resolution),
      },
    };
  };
  const taskPatterns = yamlParse(fs.readFileSync(path.join(ROOT, 'standards', 'workspace', 'task-pattern-registry.yaml'), 'utf8'))
    .task_patterns.map((p) => ({ id: p.id, work_kind: p.payload.work_kind, change_class: p.payload.change_class ?? null }));
  const upgradeOptions = (b) => ({
    registrySchema: om('upgrade-integration-qualification.schema.json'),
    envelopeSchema: envelope,
    resolveTaskJourney: makeRefResolver(b.task_journey_resolution),
    resolveFieldEvaluationReport: makeRefResolver(b.field_evaluation_resolution),
    resolveTaskSpecification: makeRefResolver(b.task_specification_resolution),
    resolveExecutionState: makeRefResolver(b.execution_state_resolution),
    taskJourneyOptions: { recordSchema: om('evidence-and-handoff.schema.json'), envelopeSchema: envelope, resolveRecords: makeRecordResolver(b.task_journey_nested_resolution) },
    fieldEvaluationOptions: { recordSchema: om('field-evaluation.schema.json'), envelopeSchema: envelope, resolveRecords: makeRecordResolver(b.field_evaluation_nested_resolution) },
    taskSpecificationOptions: { recordSchema: om('task-specification.schema.json'), envelopeSchema: envelope, taskPatterns },
    executionStateOptions: { recordSchema: om('execution-state.schema.json'), envelopeSchema: envelope },
  });
  const sameList = (actual, expected, what) => assert(
    JSON.stringify(actual) === JSON.stringify(expected),
    `${what}: expected ${JSON.stringify(expected, null, 1)}, got ${JSON.stringify(actual, null, 1)}`,
  );
  const validCase = (b, prefix) => {
    const c = b.valid.find((v) => v.note.startsWith(prefix));
    assert(c, `the bundle must carry a valid fixture whose note starts with "${prefix}"`);
    return c.registry;
  };

  // (a) Raw decision inputs of a composed record its own contract rejects.
  {
    const b = fixtureBundle('workspace-compatibility-qualification');
    const reference = 'connection:sample-connection-clean';
    const connection = b.connection_record_resolution[reference];
    connection.scope.id = 'sample-project-other';
    connection.payload.repository.id = 'sample-project-repository-unscanned';
    const doc = validCase(b, 'decision-matrix branch 5');
    const digest = computeConnectionDigest(connection);
    doc.qualifications[0].payload.workspace_connection_refs[0].sha256 = digest;
    const at = 'qualification "sample-qualification-compatibility-only"';
    const composed = `${at} workspace_connection_refs: workspace connection scan "sample-connection-clean" payload.repository.workspace_id "sample-project" does not match scope.id "sample-project-other"; scope names the exact workspace this scan covers`;
    const rustOnly = `${at} qualification_state is "QUALIFIED", but the closed decision matrix over the composed records' own next_steps/pending rule-candidate decisions/verification.overall_status/canonical export coverage computes "UNVERIFIED"`;
    const rust = [composed, rustOnly];
    check('библиотечный Node-прогон (rust-architecture-conformance-7, принятая граница): workspace-compatibility-qualification читает scope, repository.id и next_step ОТКЛОНЁННОГО собственным контрактом соединения — первая строка Node (raw scope) отличается от первой строки Rust (диагностика композиции); ровно три raw-строки только у Node, ровно одна строка матрицы (UNVERIFIED) только у Rust — парный Rust-тест workspace_compatibility_qualification_a_rejected_connection_contributes_no_raw_decision_input', () => {
      assert(digest === 'bc0bb0c68d12be6a79df75c125216e1fa59c51cce77dc5d3fe8f5070cc8bdbdb', `Node и Rust должны пинить одно и то же содержимое, получено ${digest}`);
      const node = evaluateWorkspaceCompatibilityQualification(doc, workspaceOptions(b));
      sameList(node, [
        `${at} scope does not match the resolved payload.workspace_connection_refs[0] record's scope; a qualification cannot claim a different scope than any connection scan it pins`,
        composed,
        `${at} payload.workspace_repository_ids is missing ["sample-project-repository-unscanned"], actually scanned by a resolved connection but not declared`,
        `${at} payload.workspace_repository_ids declares ["sample-project-repository"], which no resolved connection's payload.repository.id names`,
      ], 'Node');
      sameList(node.filter((l) => rust.includes(l)), [composed], 'общая с Rust диагностика');
      assert(!node.includes(rustOnly), 'Node не должен вычислять UNVERIFIED: он читает raw next_step отклонённого соединения');
      assert(node[0] !== rust[0], 'первичная диагностика должна различаться ровно так, как записано в COMPATIBILITY.md');
    });
  }
  {
    const b = fixtureBundle('upgrade-integration-qualification');
    const reference = 'field-evaluation-report:report-2026-08';
    b.field_evaluation_resolution[reference].payload.workspace_id = 'ws-other';
    const digest = computeContentDigest(b.field_evaluation_resolution[reference]);
    const doc = b.valid[0].registry;
    doc.qualifications[0].payload.field_evaluation_report_ref.sha256 = digest;
    const at = 'qualification "uiq-qualified-example"';
    const rust = [
      ...Array.from({ length: 18 }, (_, i) => `${at} payload.field_evaluation_report_ref: field evaluation report "report-2026-08" included_observations[${i}] belongs to workspace "ws-meridian", not this report's workspace "ws-other"; samples from different workspaces are not mixed without an explicit comparability rule`),
      `${at} payload.qualification_state is declared "QUALIFIED" but recomputes to "BLOCKED"; a qualification_state is recomputed, never trusted on its own`,
      `${at} qualification_state recomputes to BLOCKED but payload.blockers is empty`,
    ];
    const nodeOnly = `${at} payload.field_evaluation_report_ref resolves to payload.workspace_id "ws-other", not this qualification's own workspace "ws-meridian"; the task journey, the field-evaluation report and every scenario must belong to the same workspace`;
    check('библиотечный Node-прогон (rust-architecture-conformance-7, принятая граница): upgrade-integration-qualification читает workspace_id ОТКЛОНЁННОГО собственным контрактом field-evaluation report — первая диагностика совпадает, Node-вывод равен Rust-выводу плюс ровно одна raw-строка workspace identity — парный Rust-тест upgrade_integration_qualification_a_rejected_report_contributes_no_raw_workspace', () => {
      assert(digest === '055a75ace5235a222d11fc36275b9afbb0e3bea21555e57d7b4998aa8798ff8d', `Node и Rust должны пинить одно и то же содержимое, получено ${digest}`);
      const node = evaluateUpgradeIntegrationQualification(doc, upgradeOptions(b));
      assert(node[0] === rust[0], `первичная диагностика должна совпадать, получено ${JSON.stringify(node[0])}`);
      sameList(node.filter((l) => l !== nodeOnly), rust, 'Node без raw-строки');
      assert(node.filter((l) => l === nodeOnly).length === 1, `ожидалась ровно одна raw-строка workspace identity: ${JSON.stringify(node)}`);
    });
  }

  // (b) A schema-invalid composed plan/export is pinned by Node over raw JSON.
  for (const c of [
    {
      what: 'plan', map: 'plan_record_resolution', reference: 'plan:sample-migration-plan-one', rustTest: 'workspace_compatibility_qualification_a_schema_invalid_composed_plan_is_never_pinned_by_raw_json',
      nodeFirst: /^qualification "sample-qualification-migrated-and-exported" payload\.migration_plan_ref\.sha256 "[0-9a-f]{64}" does not equal the resolved plan's own recomputed plan_fingerprint "[0-9a-f]{64}"; /,
      rustFirst: 'qualification "sample-qualification-migrated-and-exported" migration_plan_ref: /migration_plans/0/payload/source/unexpected_field: additional property not allowed',
      ownRoute: (b, record) => evaluateInstanceDataMigration({ schema_version: 1, registry_id: 'instance-data-migration', title: 'sample-qualification-migrated-and-exported — composed migration plan', migration_plans: [record] }, workspaceOptions(b).migrationOptions),
    },
    {
      what: 'export', map: 'export_record_resolution', reference: 'export:sample-canonical-export-one', rustTest: 'workspace_compatibility_qualification_a_schema_invalid_composed_export_is_never_pinned_by_raw_json',
      nodeFirst: /^qualification "sample-qualification-migrated-and-exported" payload\.canonical_export_ref\.sha256 "[0-9a-f]{64}" does not equal the resolved export's own recomputed digest "[0-9a-f]{64}"; /,
      rustFirst: 'qualification "sample-qualification-migrated-and-exported" canonical_export_ref: /exports/0/payload/source/unexpected_field: additional property not allowed',
      ownRoute: (b, record) => evaluateInstanceCanonicalExport({ schema_version: 1, registry_id: 'instance-canonical-export', title: 'sample-qualification-migrated-and-exported — composed canonical export', exports: [record] }, { ...workspaceOptions(b).exportOptions, resolveMigrationPlan: () => null }),
    },
  ]) {
    const b = fixtureBundle('workspace-compatibility-qualification');
    b[c.map][c.reference].payload.source.unexpected_field = 'schema-only';
    const doc = validCase(b, 'decision-matrix branch 9');
    check(`библиотечный Node-прогон (rust-architecture-conformance-7, принятая граница): schema-invalid composed ${c.what} Node пинит по сырому JSON — первая и единственная диагностика записи у Node — raw sha256-несовпадение, у Rust — собственная диагностика схемы записи под префиксом композиции (та же, что первой сообщает собственный Node-маршрут записи), вторые строки совпадают — парный Rust-тест ${c.rustTest}`, () => {
      const node = evaluateWorkspaceCompatibilityQualification(doc, workspaceOptions(b));
      assert(node.length === 2, `ожидались ровно две Node-диагностики, получено ${JSON.stringify(node)}`);
      assert(c.nodeFirst.test(node[0]), `первой ожидалась raw sha256-диагностика, получено ${JSON.stringify(node[0])}`);
      const own = c.ownRoute(b, b[c.map][c.reference]);
      const prefix = `qualification "sample-qualification-migrated-and-exported" ${c.what === 'plan' ? 'migration_plan_ref' : 'canonical_export_ref'}: `;
      assert(`${prefix}${own[0]}` === c.rustFirst, `собственный Node-маршрут записи должен первой сообщить ту же диагностику схемы, что Rust: ${JSON.stringify(own)}`);
      const rustSecond = c.what === 'plan'
        ? 'qualification "sample-qualification-migrated-and-exported" payload.canonical_export_ref is present without a resolvable payload.migration_plan_ref; an export proves a plan\'s own targets and cannot be composed without one'
        : 'qualification "sample-qualification-migrated-and-exported" qualification_state is "QUALIFIED", but the closed decision matrix over the composed records\' own next_steps/pending rule-candidate decisions/verification.overall_status/canonical export coverage computes "UNVERIFIED"';
      assert(node[1] === rustSecond, `вторая диагностика должна совпадать с Rust, получено ${JSON.stringify(node[1])}`);
    });
  }

  // (c) Container/envelope schema short-circuit in all four families.
  const shortCircuit = [
    {
      family: 'instance-data-migration', entries: 'migration_plans', evaluate: evaluateInstanceDataMigration, options: planOptions,
      rustTest: 'instance_data_migration_a_container_or_envelope_schema_violation_short_circuits_before_the_domain',
      doc: (b) => { const d = b.valid[1].registry; d.migration_plans[0].payload.supersedes = d.migration_plans[0].id; return d; },
      domain: 'migration plan "sample-migration-plan-one" supersedes its own id; a plan cannot supersede itself',
    },
    {
      family: 'instance-canonical-export', entries: 'exports', evaluate: evaluateInstanceCanonicalExport, options: exportOptions,
      rustTest: 'instance_canonical_export_a_container_or_envelope_schema_violation_short_circuits_before_the_domain',
      doc: (b) => { const d = b.valid[1].registry; d.exports[0].payload.idempotency_key = 'a'.repeat(64); return d; },
      domain: `canonical export "sample-canonical-export-one" idempotency_key "${'a'.repeat(64)}" does not match the recomputed key "6a679cf719f91ef64593485de9c3f5a5a62f2a8ad118b6a8b327ac24fb3e3149" derived from this export's own plan_ref and plan_fingerprint; idempotency_key is never an arbitrary free-form string`,
    },
    {
      family: 'workspace-compatibility-qualification', entries: 'qualifications', evaluate: evaluateWorkspaceCompatibilityQualification, options: workspaceOptions,
      rustTest: 'workspace_compatibility_qualification_a_container_or_envelope_schema_violation_short_circuits_before_the_domain',
      doc: (b) => { const d = validCase(b, 'decision-matrix branch 5'); d.qualifications[0].origin.source_ref = 'workspace-connection-scan:tampered'; return d; },
      domain: 'qualification "sample-qualification-compatibility-only" origin.source_ref "workspace-connection-scan:tampered" does not equal "workspace-connection-scan:sample-connection-clean"; the envelope and payload must pin the SAME set of composed workspace connections',
    },
    {
      family: 'upgrade-integration-qualification', entries: 'qualifications', evaluate: evaluateUpgradeIntegrationQualification, options: upgradeOptions,
      rustTest: 'upgrade_integration_qualification_a_container_or_envelope_schema_violation_short_circuits_before_the_domain',
      doc: (b) => { const d = b.valid[0].registry; d.qualifications[0].payload.blockers = ['a blocker declared over a QUALIFIED record']; return d; },
      domain: 'qualification "uiq-qualified-example" qualification_state recomputes to "QUALIFIED" but payload.blockers is non-empty; blockers is closed to BLOCKED',
    },
  ];
  for (const c of shortCircuit) {
    const b = fixtureBundle(c.family);
    const opts = c.options(b);
    const doc = c.doc(b);
    check(`библиотечный Node-прогон (rust-architecture-conformance-7, принятая граница): ${c.family} — независимый доменный дефект один даёт ровно [доменная строка] (как и Rust); с добавленным schema-only дефектом (пустой title контейнера, затем envelope записи) Node возвращает [диагностика схемы, доменная строка], а Rust — только [диагностика схемы] — парный Rust-тест ${c.rustTest}`, () => {
      sameList(c.evaluate(doc, opts), [c.domain], 'только доменный дефект');
      const container = JSON.parse(JSON.stringify(doc));
      container.title = '';
      sameList(c.evaluate(container, opts), ['/title: shorter than 1', c.domain], 'контейнер');
      const entry = JSON.parse(JSON.stringify(doc));
      entry[c.entries][0].title = '';
      sameList(c.evaluate(entry, opts), ['entry 0 envelope /title: shorter than 1', c.domain], 'envelope записи');
    });
  }

  // (d) Invalid JSON content: the same rejection, only the parser tail differs.
  const neutralTail = 'the content does not parse as a single JSON value';
  const invalidJsonLine = /^(.* content is not valid JSON for media_type "application\/json": )(.+)$/;
  const tailOnly = (node, rust, what) => {
    assert(node.length === rust.length, `${what}: одинаковое число диагностик ожидалось, Node ${JSON.stringify(node)}`);
    let tails = 0;
    node.forEach((line, i) => {
      const m = invalidJsonLine.exec(line);
      if (m) {
        tails += 1;
        assert(`${m[1]}${neutralTail}` === rust[i], `${what}: строка ${i} должна отличаться от Rust только хвостом, Node ${JSON.stringify(line)} vs Rust ${JSON.stringify(rust[i])}`);
        assert(m[2] !== neutralTail && m[2].trim() !== '', `${what}: Node-хвост должен быть собственным сообщением парсера, получено ${JSON.stringify(m[2])}`);
      } else {
        assert(line === rust[i], `${what}: строка ${i} должна совпадать с Rust, Node ${JSON.stringify(line)} vs Rust ${JSON.stringify(rust[i])}`);
      }
    });
    assert(tails === 1, `${what}: ровно одна строка invalid-JSON ожидалась, найдено ${tails}`);
  };
  {
    const b = fixtureBundle('instance-data-migration');
    const doc = b.valid[1].registry;
    doc.migration_plans[0].payload.mappings[0].target.payload.content = '{not valid json';
    const plan = 'migration plan "sample-migration-plan-one"';
    const stale = 'resolved plan_fingerprint "3c465ef1b7f0b4a65d2ed1e7497fcc749e1e1be0c018b9db827cd90d8971f9ae" does not match this plan\'s own recomputed fingerprint "02a92cef0a7bb16f79379b189f3ca914ac3343a3b6b6d5e7c29f3ae2cc536b2f"';
    const rust = [
      `${plan} rollback.deterministic_plan_ref "rollback-plan:sample-revision-0001-to-migration-plan-one": ${stale}; a reconstruction plan pinned to a stale or different version of this plan's content never confirms the current one (property 5)`,
      `${plan} target "sample-migrated-record-one" payload content is not valid JSON for media_type "application/json": ${neutralTail}`,
      `${plan} evidence "evidence:coverage-migration-plan-one": ${stale}; evidence pinned to a stale or different version of this plan's content never confirms the current one (property 6)`,
      `${plan} evidence "evidence:applicability-migration-plan-one": ${stale}; evidence pinned to a stale or different version of this plan's content never confirms the current one (property 6)`,
      `${plan} plan_fingerprint "3c465ef1b7f0b4a65d2ed1e7497fcc749e1e1be0c018b9db827cd90d8971f9ae" does not match the recomputed fingerprint "02a92cef0a7bb16f79379b189f3ca914ac3343a3b6b6d5e7c29f3ae2cc536b2f" of its own documented canonical projection (source, record_units, mappings, rollback); the same pinned input must always compute the same fingerprint`,
    ];
    check('библиотечный Node-прогон (rust-architecture-conformance-7, принятая граница): instance-data-migration отклоняет невалидный JSON content тем же набором и порядком диагностик, что Rust; отличается только хвост сообщения парсера — парный Rust-тест instance_data_migration_invalid_json_content_is_rejected_like_the_reference', () => {
      tailOnly(evaluateInstanceDataMigration(doc, planOptions(b)), rust, 'instance-data-migration');
    });
  }
  {
    const b = fixtureBundle('instance-canonical-export');
    const c = b.invalid[21];
    const exp = 'canonical export "sample-canonical-export-one"';
    const rust = [
      `${exp} exported record "sample-migrated-record-one" diverges from the plan's own target for the same group; the exported record and the plan's target must be structurally identical, including $schema (property: completeness)`,
      `${exp} exported record "sample-migrated-record-one" payload content is not valid JSON for media_type "application/json": ${neutralTail}`,
      `${exp} target "sample-migrated-record-one": exported payload does not match the resolved actual content of its contributing source unit(s) byte-for-byte (media_type/encoding/content/digest); a changed value or byte is never accepted as preserved (property: content preservation)`,
    ];
    check('библиотечный Node-прогон (rust-architecture-conformance-7, принятая граница): instance-canonical-export на реальном invalid[21] (невалидный JSON content) даёт тот же набор и порядок диагностик, что Rust; отличается только хвост сообщения парсера — парный Rust-тест instance_canonical_export_invalid_json_content_is_rejected_like_the_reference', () => {
      assert(c.note === 'planted an exported record whose application/json payload content is not valid JSON', `invalid[21] должен быть случаем невалидного JSON, получено ${JSON.stringify(c.note)}`);
      tailOnly(evaluateInstanceCanonicalExport(c.registry, exportOptions(b)), rust, 'instance-canonical-export');
    });
  }

  // Opaque-ref convergence: a multi-byte character straddling the seventh
  // byte. Node returns an ordinary result; the Rust prefix check no longer
  // panics there (`meridian-core/src/types/evidence_ref.rs::a_multibyte_prefix_is_checked_without_panicking`).
  check('сближение opaque-ref (rust-architecture-conformance-7): Node checkOpaqueRef возвращает обычный результат для ref с многобайтным символом на границе седьмого байта — парный Rust-тест a_multibyte_prefix_is_checked_without_panicking', () => {
    assert(checkOpaqueRef('abcdefж/record', 'ref') === null, 'ожидался null (ref допустим)');
    assert(checkOpaqueRef('ффф:x', 'ref') === null, 'ожидался null (ref допустим)');
  });
}

runMeridianCliMigrationConformance();

// --- temp-directory hygiene: every singleCaseFixture() directory must have
// already been removed by check()'s own finally block, per check, not by
// this being the last line of a successful whole-suite run ---

check('ни один временный каталог singleCaseFixture не остался неудалённым к концу набора', () => {
  assert(createdTempDirs.length === 0, `${createdTempDirs.length} временный каталог(ов) не были удалены: ${createdTempDirs.join(', ')}`);
});

console.log(`\n${passed} passed, ${failures.length} failed`);
if (failures.length) process.exit(1);
