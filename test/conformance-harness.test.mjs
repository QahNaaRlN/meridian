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
import { evaluateControlledRuleIntake } from '../scripts/lib/controlled-rule-intake.mjs';
import { makeRecordResolver } from '../scripts/lib/context-manifest.mjs';
import { evaluateInstructionSourceRegistry } from '../scripts/lib/instruction-source-registry.mjs';
import { evaluateExistingProjectCompatibilityMode } from '../scripts/lib/existing-project-compatibility-mode.mjs';

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

// A bare "divergent" match above is not enough for this one case: it would
// also pass if a real FAIL/WARN regression appeared alongside the expected
// exit-code difference. `meridian validate` is expected to differ from the
// Node reference on exit code ONLY (BLOCKED_CHECKS is non-empty, so it never
// returns 0 on this Kernel, even though it has zero real failures of its
// own) — so this asserts that exact shape, not merely "some divergence".
check('real-node-rust-cli-validate-clean-kernel: единственное расхождение — код завершения (BLOCKED_CHECKS), диагностики совпадают полностью', () => {
  const result = corpusResults.find((r) => r.name === 'real-node-rust-cli-validate-clean-kernel');
  assert(result, 'fixture case real-node-rust-cli-validate-clean-kernel not found');
  const comparison = result.detail.comparison;
  assert(comparison.exit_code.match === false, `ожидалось расхождение кода завершения (Node ${comparison.exit_code.left} vs Rust ${comparison.exit_code.right} из-за непустого BLOCKED_CHECKS), получено match=${comparison.exit_code.match}`);
  assert(comparison.exit_code.left === 0, `ожидался Node exit 0 (эталон полностью проверяет чистое дерево), получено ${comparison.exit_code.left}`);
  assert(comparison.exit_code.right === 1, `ожидался Rust exit 1 (validate не возвращает 0 пока BLOCKED_CHECKS непусто), получено ${comparison.exit_code.right}`);
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
// upgrade-integration-qualification's (7d, still `BLOCKED_CHECKS`) own
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
    env: { ...process.env, MERIDIAN_KERNEL: dir },
    encoding: 'utf8',
  });
  const nodeFails = nodeRun.stdout
    .split('\n')
    .filter((l) => l.startsWith('FAIL'))
    .map((l) => l.replace(/^FAIL\s+/, ''));
  const meridianBin = path.join(ROOT, 'target', 'debug', process.platform === 'win32' ? 'meridian.exe' : 'meridian');
  const rustRun = spawnSync(meridianBin, ['validate', '--kernel', dir, '--format', 'json'], { encoding: 'utf8' });
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
// name, which cascades this one mutation into families this Kernel's Rust
// side has not yet ported (upgrade-integration-qualification is 7d,
// BLOCKED_CHECKS) and can therefore never report — an unavoidable, permanent
// divergence that has nothing to do with task-pattern-registry itself. This
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
    const assertDeltaMultisetsEqual = assertMutationDeltaMultisetsEqual(
      mutationDir,
      family.expectedFailPrefix,
      family.id,
    );
    check(
      `реальный Node/Rust validate: schema-valid мутация "${family.id}" даёт полностью равные как мультимножество multiset-дельты (с учётом кратности) относительно baseline на обеих сторонах, непустые и несущие префикс "${family.expectedFailPrefix}" (подпакет 7b)`,
      () => {
        assertDeltaMultisetsEqual(family.write);
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
// "unknown field" checks (`meridian-app/src/operating_model/bounded_context_manifest.rs`,
// `check_resolved_state` and `resolve_pinned_reference`) obtain a genuine
// Rust-native improvement over the Node reference: they iterate
// `serde_json::Map` — a `BTreeMap` in this workspace — in stable
// alphabetical order, never Node's `Object.keys()` source-text order. This
// is kept, not reverted (a Rust unit test,
// `unknown_fields_on_a_resolved_entry_are_reported_in_stable_alphabetical_order_across_repeated_calls`,
// pins the alphabetical order and byte-identical repeated output). This is
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
        env: { ...process.env, MERIDIAN_KERNEL: dir },
        encoding: 'utf8',
      });
      const meridianBin = path.join(ROOT, 'target', 'debug', process.platform === 'win32' ? 'meridian.exe' : 'meridian');
      const rustRun = spawnSync(meridianBin, ['validate', '--kernel', dir, '--format', 'json'], { encoding: 'utf8' });
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
    id: 'instruction-topics-wrong-yaml-type',
    write: (dir) => {
      const yamlFile = path.join(dir, 'standards', 'workspace', 'instruction-topics.yaml');
      fs.writeFileSync(yamlFile, 'schema_version: 1\ntopics: true\n');
    },
    expectedFailPrefix: 'instruction-topics:',
    // The Node reference's own diagnostic here is an *incidental* engine
    // TypeError message ("(val || []).map is not a function"), not an
    // authored one — `scripts/kernel-validate.mjs` never states that exact
    // text as a deliberate contract the way every other `fail(...)` call
    // does. This port deliberately reports a legible, authored failure
    // instead of reproducing an uncaught-exception string
    // (`meridian-cli/src/commands/validate/instruction_topics.rs`,
    // `AGENTS.md` §9 — Rust may make an ill-defined reference state more
    // robust when the boundary is named and tested, as it is here). Exact
    // text equality is therefore not required for this one case; both
    // sides independently producing a new FAIL under the same prefix is
    // the contract actually being proven.
    exactTextNotRequired: true,
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
      if (family.exactTextNotRequired) {
        // Same exit code, and each side independently produced its own new
        // FAIL under the expected prefix — not byte-identical diagnostic
        // text (see the family's own comment for why).
        assert(
          result.comparison.exit_code.match,
          `ожидался одинаковый код завершения для "${family.id}", получено ${JSON.stringify(result.comparison.exit_code)}`,
        );
        assert(
          result.comparison.fail.left.some((m) => m.startsWith(family.expectedFailPrefix)),
          `мутация "${family.id}" должна была произвести новый FAIL с префиксом "${family.expectedFailPrefix}" на стороне Node, получено: ${JSON.stringify(result.comparison.fail.left)}`,
        );
        assert(
          result.comparison.fail.right.some((m) => m.startsWith(family.expectedFailPrefix)),
          `мутация "${family.id}" должна была произвести новый FAIL с префиксом "${family.expectedFailPrefix}" на стороне Rust, получено: ${JSON.stringify(result.comparison.fail.right)}`,
        );
        return;
      }
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
      env: { ...process.env, MERIDIAN_KERNEL: dir },
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
    const rustRun = spawnSync(meridianBin, ['validate', '--kernel', dir, '--format', 'json'], { encoding: 'utf8' });
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

// --- temp-directory hygiene: every singleCaseFixture() directory must have
// already been removed by check()'s own finally block, per check, not by
// this being the last line of a successful whole-suite run ---

check('ни один временный каталог singleCaseFixture не остался неудалённым к концу набора', () => {
  assert(createdTempDirs.length === 0, `${createdTempDirs.length} временный каталог(ов) не были удалены: ${createdTempDirs.join(', ')}`);
});

console.log(`\n${passed} passed, ${failures.length} failed`);
if (failures.length) process.exit(1);
