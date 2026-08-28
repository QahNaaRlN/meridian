#!/usr/bin/env node
// Regression for hooks/pre-push, two properties it must hold before it runs a
// single gate:
//
//   1. ENVIRONMENT ISOLATION — Git runs the hook with GIT_DIR, GIT_WORK_TREE,
//      GIT_INDEX_FILE, … (`git rev-parse --local-env-vars`) exported. A gate
//      that builds a throwaway synthetic repository and runs `git init/add/
//      commit` inside it would inherit those and operate on the repository
//      being pushed — a synthetic test once committed straight onto a Kernel
//      branch that way. hooks/lib/git-env-isolate.sh unsets every repo-local
//      Git variable; it does NOT touch MERIDIAN_*.
//
//   2. KERNEL BINDING — scripts/kernel-validate.mjs and scripts/rule-resolver
//      .mjs prefer $MERIDIAN_KERNEL over their own location. An ambient
//      MERIDIAN_KERNEL from the operator's shell (a different, possibly dirty
//      checkout) makes the gates validate the wrong tree and falsely block the
//      push. After the scrub the hook pins MERIDIAN_KERNEL to the worktree Git
//      is pushing from ($ROOT), so every gate sees the pushed Kernel.
//
// The push checks run the VERBATIM production hook + helper of this package,
// copied into a synthetic Kernel repo, with every gate replaced by a shim that
// records what it received and fails on a leaked Git var or a wrong
// MERIDIAN_KERNEL. Deleting or moving the `. …/git-env-isolate.sh` line or the
// `export MERIDIAN_KERNEL="$ROOT"` line makes these checks red.
//
// Everything is local — synthetic repositories in a temp dir and a local bare
// remote. No network, no real push, no `--no-verify`.
//
// Usage: node test/pre-push-git-isolation.test.mjs

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync, spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const KERNEL_ROOT = path.resolve(__dirname, '..');
const ISOLATE = path.join(KERNEL_ROOT, 'hooks', 'lib', 'git-env-isolate.sh');
const PRE_PUSH = path.join(KERNEL_ROOT, 'hooks', 'pre-push');

let passed = 0;
let failed = 0;
const failures = [];
function check(name, fn) {
  try { fn(); passed++; console.log(`ok    ${name}`); }
  catch (e) { failed++; failures.push(`${name}: ${e && e.message ? e.message : String(e)}`); console.log(`FAIL  ${name}`); }
}
function assert(cond, msg) { if (!cond) throw new Error(msg || 'assertion failed'); }

// Every repository-local Git variable Git can export (git rev-parse
// --local-env-vars on git 2.53).
const LOCAL_GIT_VARS = [
  'GIT_ALTERNATE_OBJECT_DIRECTORIES', 'GIT_CONFIG', 'GIT_CONFIG_PARAMETERS', 'GIT_CONFIG_COUNT',
  'GIT_OBJECT_DIRECTORY', 'GIT_DIR', 'GIT_WORK_TREE', 'GIT_IMPLICIT_WORK_TREE', 'GIT_GRAFT_FILE',
  'GIT_INDEX_FILE', 'GIT_NO_REPLACE_OBJECTS', 'GIT_REPLACE_REF_BASE', 'GIT_PREFIX',
  'GIT_SHALLOW_FILE', 'GIT_COMMON_DIR',
];

// A clean, deterministic environment: PATH etc. preserved, every repo-local Git
// var removed, a fixed identity, no global/system git config, and no
// MERIDIAN_* unless a check sets it explicitly.
function cleanEnv(extra) {
  const e = { ...process.env };
  for (const v of LOCAL_GIT_VARS) delete e[v];
  delete e.MERIDIAN_INSTANCE;
  delete e.MERIDIAN_KERNEL;
  e.GIT_AUTHOR_NAME = 't';
  e.GIT_AUTHOR_EMAIL = 't@t.invalid';
  e.GIT_COMMITTER_NAME = 't';
  e.GIT_COMMITTER_EMAIL = 't@t.invalid';
  e.GIT_CONFIG_GLOBAL = '/dev/null';
  e.GIT_CONFIG_NOSYSTEM = '1';
  return { ...e, ...(extra || {}) };
}

const work = fs.mkdtempSync(path.join(os.tmpdir(), 'meridian-prepush-iso-'));
process.on('exit', () => { try { fs.rmSync(work, { recursive: true, force: true }); } catch { /* best effort */ } });

function git(cwd, args, env) {
  return execFileSync('git', args, { cwd, encoding: 'utf8', env: env || cleanEnv(), stdio: ['ignore', 'pipe', 'pipe'] }).trim();
}
function head(dir) { return git(dir, ['rev-parse', 'HEAD']); }
function makeRepo(name) {
  const dir = path.join(work, name);
  fs.mkdirSync(dir, { recursive: true });
  git(dir, ['init', '-q']);
  fs.writeFileSync(path.join(dir, 'seed.txt'), 'seed\n');
  git(dir, ['add', '-A']);
  git(dir, ['commit', '-q', '-m', 'seed']);
  return dir;
}

// A gate shim: stands in for one gate the production hook invokes. It appends
// to a probe log what repository-local Git variables and what MERIDIAN_KERNEL
// it was handed, and EXITS NON-ZERO if it saw any leaked Git var or a
// MERIDIAN_KERNEL that is not the Kernel being pushed — so a hook that fails to
// isolate the environment or to pin MERIDIAN_KERNEL blocks its own push.
function gateShimSource(probeLog, wantKernelRoot) {
  return [
    "import fs from 'node:fs';",
    "import path from 'node:path';",
    `const LOCAL = ${JSON.stringify(LOCAL_GIT_VARS)};`,
    `const WANT = ${JSON.stringify(wantKernelRoot)};`,
    'const leaked = LOCAL.filter((v) => Object.prototype.hasOwnProperty.call(process.env, v));',
    "const rawMK = process.env.MERIDIAN_KERNEL;",
    "const shownMK = rawMK === undefined ? '<unset>' : rawMK;",
    "let normMK = null; try { normMK = rawMK ? fs.realpathSync(rawMK) : null; } catch { normMK = rawMK || null; }",
    `fs.appendFileSync(${JSON.stringify(probeLog)}, 'gate ' + path.basename(process.argv[1] || '?') + ' leaked=[' + leaked.join(',') + '] MERIDIAN_KERNEL=' + shownMK + '\\n');`,
    "if (leaked.length) { process.stderr.write('gate shim: repo-local Git vars leaked into a gate: ' + leaked.join(',') + '\\n'); process.exit(1); }",
    "if (normMK !== WANT) { process.stderr.write('gate shim: MERIDIAN_KERNEL is ' + shownMK + ', expected the pushed Kernel ' + WANT + '\\n'); process.exit(1); }",
    "process.stdout.write('gate shim ok\\n');",
    '',
  ].join('\n');
}

// Build a synthetic Kernel repository carrying the VERBATIM production hook and
// helper of this package, with every gate replaced by the shim above. The two
// optional mutations delete exactly one load-bearing line from the copied hook.
function buildSyntheticKernel(name, { dropSourcingLine = false, dropKernelPin = false } = {}) {
  const dir = path.join(work, name);
  fs.mkdirSync(path.join(dir, 'hooks', 'lib'), { recursive: true });
  fs.mkdirSync(path.join(dir, 'test', 'instance-fixture'), { recursive: true });
  fs.mkdirSync(path.join(dir, 'scripts'), { recursive: true });
  const realDir = fs.realpathSync(dir);

  let hook = fs.readFileSync(PRE_PUSH, 'utf8');

  const SOURCING_RE = /^\. "\$ROOT\/hooks\/lib\/git-env-isolate\.sh"[ \t]*$/m;
  const PIN_RE = /^export MERIDIAN_KERNEL="\$ROOT"[ \t]*$/m;
  assert(SOURCING_RE.test(hook),
    'hooks/pre-push no longer sources hooks/lib/git-env-isolate.sh on its own line — the isolation line was removed or moved');
  assert(PIN_RE.test(hook),
    'hooks/pre-push no longer pins MERIDIAN_KERNEL to "$ROOT" on its own line — the kernel-binding line was removed or moved');

  if (dropSourcingLine) hook = hook.replace(new RegExp(SOURCING_RE.source + '\\n', 'm'), '');
  if (dropKernelPin) hook = hook.replace(new RegExp(PIN_RE.source + '\\n', 'm'), '');
  fs.writeFileSync(path.join(dir, 'hooks', 'pre-push'), hook);
  fs.chmodSync(path.join(dir, 'hooks', 'pre-push'), 0o755);
  fs.copyFileSync(ISOLATE, path.join(dir, 'hooks', 'lib', 'git-env-isolate.sh'));

  const probe = path.join(dir, 'probe.log');
  const shim = gateShimSource(probe, realDir);
  for (const rel of [
    'test/kernel-validate.test.mjs',
    'test/rule-resolver.test.mjs',
    'test/pre-push-git-isolation.test.mjs',
    'scripts/kernel-validate.mjs',
    'scripts/validate-and-log.mjs',
  ]) {
    fs.writeFileSync(path.join(dir, rel), shim);
  }
  fs.writeFileSync(path.join(dir, 'seed.txt'), 'seed\n');
  return { dir, realDir, probe };
}

function probeLines(probe) {
  try { return fs.readFileSync(probe, 'utf8').trim().split('\n').filter(Boolean); }
  catch { return []; }
}

// git init + one commit + a local bare remote, then a normal `git push` that
// fires the synthetic repo's (production-copied) pre-push. No `--no-verify`.
// `ambient` seeds the push environment — e.g. a stale MERIDIAN_KERNEL.
function pushThrough(syn, ambient) {
  const caller = syn.dir;
  git(caller, ['init', '-q']);
  git(caller, ['add', '-A']);
  git(caller, ['commit', '-q', '-m', 'synthetic kernel snapshot']);
  git(caller, ['config', 'core.hooksPath', 'hooks']);
  const remote = `${caller}-remote.git`;
  execFileSync('git', ['init', '-q', '--bare', remote], { encoding: 'utf8', env: cleanEnv() });
  git(caller, ['remote', 'add', 'origin', remote]);
  const branch = git(caller, ['rev-parse', '--abbrev-ref', 'HEAD']);
  const callerBefore = head(caller);
  const kernelBefore = git(KERNEL_ROOT, ['rev-parse', 'HEAD']);
  const r = spawnSync('git', ['push', 'origin', branch], { cwd: caller, encoding: 'utf8', env: cleanEnv(ambient) });
  return { r, branch, remote, caller, callerBefore, kernelBefore };
}

// ===========================================================================
check('env scrub — repo-local Git vars unset; MERIDIAN_* preserved unchanged (helper does not normalize)', () => {
  const polluted = {
    ...process.env,
    GIT_DIR: '/some/other/.git',
    GIT_WORK_TREE: '/some/other',
    GIT_INDEX_FILE: '/some/other/.git/index',
    GIT_PREFIX: 'sub/',
    GIT_COMMON_DIR: '/some/other/.git',
    MERIDIAN_KERNEL: '/an/ambient/kernel',
    MERIDIAN_INSTANCE: '/an/ambient/instance',
  };
  const out = execFileSync('sh', ['-c', `set -e; . "${ISOLATE}"; echo "rc=$?"; env`], { encoding: 'utf8', env: polluted });
  assert(/(^|\n)rc=0(\n|$)/.test(out), 'the helper did not return 0 on success');
  const kv = new Map(out.split('\n').filter((l) => l.includes('=')).map((l) => [l.slice(0, l.indexOf('=')), l.slice(l.indexOf('=') + 1)]));
  for (const v of ['GIT_DIR', 'GIT_WORK_TREE', 'GIT_INDEX_FILE', 'GIT_PREFIX', 'GIT_COMMON_DIR']) {
    assert(!kv.has(v), `${v} should have been unset by the isolate script`);
  }
  assert(kv.get('MERIDIAN_KERNEL') === '/an/ambient/kernel',
    'the helper must NOT normalize MERIDIAN_KERNEL — that is the production hook\'s job');
  assert(kv.get('MERIDIAN_INSTANCE') === '/an/ambient/instance', 'the helper must leave MERIDIAN_INSTANCE untouched');
  assert(kv.has('PATH'), 'PATH must be preserved');
});

// ---------------------------------------------------------------------------
check('control — WITHOUT the scrub, an inherited GIT_DIR moves the caller HEAD', () => {
  const caller = makeRepo('control-caller');
  const before = head(caller);
  const gateCwd = fs.mkdtempSync(path.join(work, 'control-gate-'));
  const leakEnv = cleanEnv({ GIT_DIR: path.join(caller, '.git'), GIT_WORK_TREE: caller });
  spawnSync('git', ['commit', '--allow-empty', '-q', '-m', 'synthetic-leak'], { cwd: gateCwd, env: leakEnv });
  assert(head(caller) !== before,
    'control did not reproduce the environment leak — the rest of this suite would not prove the fix');
});

// ---------------------------------------------------------------------------
check('isolation — WITH the scrub, a synthetic git init/commit stays in the temp dir', () => {
  const caller = makeRepo('iso-caller');
  const before = head(caller);
  const gateCwd = fs.mkdtempSync(path.join(work, 'iso-gate-'));
  const script = [
    'set -e',
    `. "${ISOLATE}"`,
    `cd "${gateCwd}"`,
    'git init -q',
    'echo x > f.txt',
    'git add -A',
    'git commit -q -m synthetic',
    'git rev-parse HEAD',
  ].join('\n');
  const leakEnv = cleanEnv({ GIT_DIR: path.join(caller, '.git'), GIT_WORK_TREE: caller });
  const synthHead = execFileSync('sh', ['-c', script], { encoding: 'utf8', env: leakEnv }).trim();
  assert(head(caller) === before, 'the caller HEAD moved despite the isolate script');
  assert(/^[0-9a-f]{40}$/.test(synthHead) && synthHead !== before, 'the synthetic commit did not land in the temp repo');
});

// ---------------------------------------------------------------------------
check('push — the REAL production hook scrubs the Git env before the first gate', () => {
  const syn = buildSyntheticKernel('scrub');
  const { r, branch, remote, caller, callerBefore, kernelBefore } = pushThrough(syn);
  assert(r.status === 0, `push through the real hook failed (exit ${r.status}); stderr:\n${r.stderr}`);
  const lines = probeLines(syn.probe);
  assert(lines.length >= 3, `the gate shims did not all run; probe log: ${JSON.stringify(lines)}`);
  assert(lines.every((l) => /leaked=\[\] /.test(l)), `a gate saw a repo-local Git var: ${JSON.stringify(lines)}`);
  assert(/leaked=\[\] /.test(lines[0]), `the FIRST gate ran before the environment was scrubbed: ${lines[0]}`);
  assert(head(caller) === callerBefore, 'the caller HEAD moved');
  assert(git(KERNEL_ROOT, ['rev-parse', 'HEAD']) === kernelBefore, 'the real Kernel HEAD moved during the simulation');
  assert(git(remote, ['rev-parse', branch]) === callerBefore, 'the local bare remote did not receive the pushed commit');
});

// ---------------------------------------------------------------------------
check('push — the REAL production hook pins MERIDIAN_KERNEL to the pushed root under a stale ambient value', () => {
  const stale = fs.mkdtempSync(path.join(work, 'stale-ambient-kernel-'));
  const syn = buildSyntheticKernel('kernel-pin');
  const { r, branch, remote, caller, callerBefore, kernelBefore } = pushThrough(syn, { MERIDIAN_KERNEL: stale });
  assert(r.status === 0, `push failed despite the pin (exit ${r.status}); stderr:\n${r.stderr}`);
  const lines = probeLines(syn.probe);
  assert(lines.length >= 3, `the gate shims did not all run; probe log: ${JSON.stringify(lines)}`);
  for (const l of lines) {
    assert(l.includes(`MERIDIAN_KERNEL=${syn.realDir}`), `a gate saw a MERIDIAN_KERNEL other than the pushed root: ${l}`);
    assert(!l.includes(stale), `a gate saw the stale ambient MERIDIAN_KERNEL: ${l}`);
  }
  assert(lines[0].includes(`MERIDIAN_KERNEL=${syn.realDir}`), `the FIRST gate did not see the pushed root: ${lines[0]}`);
  assert(head(caller) === callerBefore && git(KERNEL_ROOT, ['rev-parse', 'HEAD']) === kernelBefore, 'a HEAD moved');
  assert(git(remote, ['rev-parse', branch]) === callerBefore, 'the bare remote did not receive the pushed commit');
});

// ---------------------------------------------------------------------------
check('push — deleting the isolation sourcing line makes the push fail closed', () => {
  const syn = buildSyntheticKernel('no-sourcing', { dropSourcingLine: true });
  const { r, branch, remote, caller, callerBefore, kernelBefore } = pushThrough(syn);
  assert(r.status !== 0, `push should have been BLOCKED without the sourcing line (exit ${r.status})`);
  const lines = probeLines(syn.probe);
  assert(lines.length >= 1, 'the first gate shim should still have been reached');
  assert(/leaked=\[GIT_/.test(lines[0]), `without the scrub the first gate must see repo-local Git vars: ${lines[0]}`);
  assert(spawnSync('git', ['-C', remote, 'rev-parse', branch], { encoding: 'utf8' }).status !== 0,
    'the blocked push still reached the bare remote');
  assert(head(caller) === callerBefore, 'the caller HEAD moved despite the blocked push');
  assert(git(KERNEL_ROOT, ['rev-parse', 'HEAD']) === kernelBefore, 'the real Kernel HEAD moved');
});

// ---------------------------------------------------------------------------
check('push — deleting the MERIDIAN_KERNEL pin lets a stale ambient value reach the gates and fails the push', () => {
  const stale = fs.mkdtempSync(path.join(work, 'stale-ambient-kernel-2-'));
  const syn = buildSyntheticKernel('no-kernel-pin', { dropKernelPin: true });
  const { r, branch, remote, caller, callerBefore, kernelBefore } = pushThrough(syn, { MERIDIAN_KERNEL: stale });
  assert(r.status !== 0, `push should have been BLOCKED once a gate saw the stale MERIDIAN_KERNEL (exit ${r.status})`);
  const lines = probeLines(syn.probe);
  assert(lines.length >= 1, 'the first gate shim should still have been reached');
  assert(/leaked=\[\] /.test(lines[0]), `the Git env should still be scrubbed: ${lines[0]}`);
  assert(lines[0].includes(`MERIDIAN_KERNEL=${stale}`),
    `without the pin the first gate must see the stale ambient value: ${lines[0]}`);
  assert(!lines[0].includes(`MERIDIAN_KERNEL=${syn.realDir}`), 'the pushed root leaked in despite the missing pin');
  assert(spawnSync('git', ['-C', remote, 'rev-parse', branch], { encoding: 'utf8' }).status !== 0,
    'the blocked push still reached the bare remote');
  assert(head(caller) === callerBefore && git(KERNEL_ROOT, ['rev-parse', 'HEAD']) === kernelBefore, 'a HEAD moved');
});

// ---------------------------------------------------------------------------
check('rev-parse failure — helper and hook fail closed, no gate runs', () => {
  const realGit = execFileSync('sh', ['-c', 'command -v git'], { encoding: 'utf8' }).trim();
  const binDir = path.join(work, 'poison-bin');
  fs.mkdirSync(binDir);
  fs.writeFileSync(path.join(binDir, 'git'), [
    '#!/bin/sh',
    'if [ "$1" = "rev-parse" ] && [ "$2" = "--local-env-vars" ]; then',
    '  echo "fatal: simulated rev-parse --local-env-vars failure" >&2',
    '  exit 7',
    'fi',
    `exec ${JSON.stringify(realGit)} "$@"`,
    '',
  ].join('\n'));
  fs.chmodSync(path.join(binDir, 'git'), 0o755);
  const poisonedPath = `${binDir}:${process.env.PATH}`;

  // (a) sourcing the helper alone must fail non-zero and NOT "succeed empty".
  const a = spawnSync('sh', ['-c', `set -e; . "${ISOLATE}"; echo SCRUBBED-OK`],
    { encoding: 'utf8', env: cleanEnv({ PATH: poisonedPath }) });
  assert(a.status !== 0, `sourcing the helper should fail when rev-parse fails (exit ${a.status})`);
  assert(!/SCRUBBED-OK/.test(a.stdout), 'the helper continued past a rev-parse failure into a silent empty scrub');
  assert(/refusing to run pre-push gates without a scrubbed Git environment/.test(a.stderr),
    `the helper printed no fail-closed diagnostic: ${a.stderr}`);

  // (b) the real production hook, with the same poisoned git, must exit
  //     non-zero and start no gate at all.
  const syn = buildSyntheticKernel('rev-parse-fail');
  git(syn.dir, ['init', '-q']);
  git(syn.dir, ['add', '-A']);
  git(syn.dir, ['commit', '-q', '-m', 'synthetic kernel snapshot']);
  const callerBefore = head(syn.dir);
  const kernelBefore = git(KERNEL_ROOT, ['rev-parse', 'HEAD']);
  const b = spawnSync('sh', [path.join(syn.dir, 'hooks', 'pre-push')],
    { cwd: syn.dir, encoding: 'utf8', env: cleanEnv({ PATH: poisonedPath }) });
  assert(b.status !== 0, `the hook should fail closed on a rev-parse failure (exit ${b.status})`);
  assert(probeLines(syn.probe).length === 0,
    `a gate ran despite the fail-closed helper: ${JSON.stringify(probeLines(syn.probe))}`);
  assert(head(syn.dir) === callerBefore, 'the caller HEAD moved during the fail-closed run');
  assert(git(KERNEL_ROOT, ['rev-parse', 'HEAD']) === kernelBefore, 'the real Kernel HEAD moved during the fail-closed run');
});

// ---------------------------------------------------------------------------
console.log(`\n${passed} passed, ${failed} failed`);
if (failed) {
  console.log('\n--- failures ---');
  for (const f of failures) console.log(f);
  process.exit(1);
}
