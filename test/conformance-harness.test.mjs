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

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import { runProducer, normalizeDiagnostics, compareVerdicts, runCase, runCorpus, runConformanceCheck } from '../verification/conformance-harness/conformance-harness.mjs';

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

const rustProducersBuild = spawnSync('cargo', ['build', '-p', 'meridian-app', '--examples'], {
  cwd: ROOT,
  encoding: 'utf8',
});
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

// --- temp-directory hygiene: every singleCaseFixture() directory must have
// already been removed by check()'s own finally block, per check, not by
// this being the last line of a successful whole-suite run ---

check('ни один временный каталог singleCaseFixture не остался неудалённым к концу набора', () => {
  assert(createdTempDirs.length === 0, `${createdTempDirs.length} временный каталог(ов) не были удалены: ${createdTempDirs.join(', ')}`);
});

console.log(`\n${passed} passed, ${failures.length} failed`);
if (failures.length) process.exit(1);
