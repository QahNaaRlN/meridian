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
