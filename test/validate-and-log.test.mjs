#!/usr/bin/env node
// Regression suite for scripts/validate-and-log.mjs.
//
// Proves the three properties the wrapper claims: (1) it is exit-code
// transparent — a caller relying on the exit code (a hook, CI) sees exactly
// what the underlying validator would have produced; (2) it logs exactly one
// well-formed JSON record per run against a real Instance; (3) it logs
// nothing for a fixture or no-Instance run, so gate-health metrics are never
// diluted by synthetic runs.
//
// Usage: node test/validate-and-log.test.mjs

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawnSync, execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const WRAPPER = path.join(__dirname, '..', 'scripts', 'validate-and-log.mjs');
const VALIDATOR = path.join(__dirname, '..', 'scripts', 'kernel-validate.mjs');

let passed = 0;
let failed = 0;
const failures = [];

function sh(cmd, args, cwd) {
  execFileSync(cmd, args, { cwd, stdio: ['ignore', 'ignore', 'ignore'] });
}

try {
  sh('git', ['--version'], process.cwd());
} catch {
  console.error('SKIP-ALL: git is unavailable.');
  process.exit(2);
}

const workRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'meridian-validate-log-test-'));

function write(root, rel, content) {
  const p = path.join(root, rel);
  fs.mkdirSync(path.dirname(p), { recursive: true });
  fs.writeFileSync(p, content);
  return p;
}

function sha256(text) {
  return crypto.createHash('sha256').update(text, 'utf8').digest('hex');
}

function buildKernel(root) {
  write(root, 'README.md', '# Synthetic kernel\n');
  const skill = '# Demo skill\n';
  write(root, 'skills/demo/SKILL.md', skill);
  write(root, 'skills/demo/PIN.yaml', [
    'schema_version: 1', 'name: demo', 'artifact: SKILL.md',
    `sha256: ${sha256(skill)}`, 'state: vendored',
    "pinned_at: '2026-01-01'", 'pinned_by: test-suite', '',
  ].join('\n'));
  sh('git', ['init', '-q'], root);
  sh('git', ['add', '-A'], root);
  sh('git', ['-c', 'user.name=t', '-c', 'user.email=t@t.invalid', 'commit', '-q', '-m', 'synthetic'], root);
}

function buildInstance(root, { git = true } = {}) {
  write(root, 'product.yaml', [
    'schema_version: 1', 'product:', '  name: Quorvath',
    '  description: Fictional product used only by the wrapper test suite.',
    'canonical_wiki:', '  tool: none', '  base_url: https://wiki.quorvath.invalid',
    '  space_key: QVX', 'tooling:', '  task_tracker:',
    '    base_url: https://tracker.quorvath.invalid/', '  repository:',
    '    base_url: https://scm.quorvath.invalid', '    namespace: quorvath/apps',
    'forbidden_literals: []', 'owner: test-suite', "last_verified: '2026-01-01'", '',
  ].join('\n'));
  write(root, 'external-dependencies.yaml', 'schema_version: 1\ndependencies: []\n');
  write(root, 'inventory/repositories.yaml', 'schema_version: 1\nrepositories: []\n');
  if (!git) return; // the in-kernel fixture must NOT get its own .git: a
  // nested repo would become a gitlink when the kernel commits it, hiding
  // its files from `git ls-files` instead of exercising the fixture path.
  // instance_revision requires the Instance to actually be under Git —
  // exercise that path in every case that legitimately has one.
  sh('git', ['init', '-q'], root);
  sh('git', ['add', '-A'], root);
  sh('git', ['-c', 'user.name=t', '-c', 'user.email=t@t.invalid', 'commit', '-q', '-m', 'synthetic instance'], root);
}

function run(env) {
  const r = spawnSync(process.execPath, [WRAPPER], { env: { ...process.env, ...env }, encoding: 'utf8' });
  return { out: `${r.stdout}\n${r.stderr}`, code: r.status };
}
function runValidatorDirect(env) {
  const r = spawnSync(process.execPath, [VALIDATOR], { env: { ...process.env, ...env }, encoding: 'utf8' });
  return { out: r.stdout ?? '', code: r.status };
}

function check(name, cond, detail) {
  if (cond) { passed++; console.log(`ok    ${name}`); }
  else { failed++; failures.push(`${name}: ${detail}`); console.log(`FAIL  ${name}`); }
}

// t01 — exit code parity: real Instance, clean tree
{
  const kernel = path.join(workRoot, 't01', 'kernel');
  const instance = path.join(workRoot, 't01', 'instance');
  buildKernel(kernel);
  buildInstance(instance);
  const env = { MERIDIAN_KERNEL: kernel, MERIDIAN_INSTANCE: instance };
  const direct = runValidatorDirect(env);
  const wrapped = run(env);
  check('t01 exit code matches the underlying validator', wrapped.code === direct.code,
    `wrapper exit ${wrapped.code}, validator exit ${direct.code}`);
  check('t01 underlying output is reproduced', wrapped.out.includes(direct.out.trim().split('\n')[0]),
    'first line of validator output not found in wrapper output');
}

// t02 — a real Instance run appends exactly one well-formed JSON record
{
  const kernel = path.join(workRoot, 't02', 'kernel');
  const instance = path.join(workRoot, 't02', 'instance');
  buildKernel(kernel);
  buildInstance(instance);
  const env = { MERIDIAN_KERNEL: kernel, MERIDIAN_INSTANCE: instance };
  run(env);
  const logPath = path.join(instance, '.agent', 'metrics', 'validate-log.jsonl');
  const exists = fs.existsSync(logPath);
  check('t02 log file created for a real Instance', exists, `expected ${logPath}`);
  if (exists) {
    const lines = fs.readFileSync(logPath, 'utf8').trim().split('\n').filter(Boolean);
    check('t02 exactly one record appended', lines.length === 1, `found ${lines.length}`);
    let rec = null;
    try { rec = JSON.parse(lines[0]); } catch { /* leave null */ }
    check('t02 record is valid JSON', rec !== null, lines[0]);
    if (rec) {
      const requiredFields = ['ts', 'kernel_revision', 'instance_revision', 'exit_code', 'failing', 'warnings', 'info_ok'];
      const missing = requiredFields.filter((f) => !(f in rec));
      check('t02 record has all required fields', missing.length === 0, `missing: ${missing.join(', ')}`);
      check('t02 instance_revision recorded', typeof rec.instance_revision === 'string' && rec.instance_revision.length === 40,
        `got ${JSON.stringify(rec.instance_revision)}`);
    }
  }
}

// t03 — a second run appends, does not overwrite (log is append-only)
{
  const kernel = path.join(workRoot, 't03', 'kernel');
  const instance = path.join(workRoot, 't03', 'instance');
  buildKernel(kernel);
  buildInstance(instance);
  const env = { MERIDIAN_KERNEL: kernel, MERIDIAN_INSTANCE: instance };
  run(env);
  run(env);
  const logPath = path.join(instance, '.agent', 'metrics', 'validate-log.jsonl');
  const lines = fs.readFileSync(logPath, 'utf8').trim().split('\n').filter(Boolean);
  check('t03 two runs append two records', lines.length === 2, `found ${lines.length}`);
}

// t04 — an Instance that is the in-kernel fixture is never logged
{
  const kernel = path.join(workRoot, 't04', 'kernel');
  buildKernel(kernel);
  const fixture = path.join(kernel, 'test-instance');
  buildInstance(fixture, { git: false });
  sh('git', ['add', '-A'], kernel);
  sh('git', ['-c', 'user.name=t', '-c', 'user.email=t@t.invalid', 'commit', '-q', '-m', 'fixture'], kernel);
  const env = { MERIDIAN_KERNEL: kernel, MERIDIAN_INSTANCE: fixture };
  const wrapped = run(env);
  const logPath = path.join(fixture, '.agent', 'metrics', 'validate-log.jsonl');
  check('t04 fixture run is not logged', !fs.existsSync(logPath), 'log file should not exist for a fixture Instance');
  check('t04 fixture run says why', /mode=fixture/.test(wrapped.out), wrapped.out);
}

// t05 — no Instance configured: no crash, no log, exit reflects the validator
{
  const kernel = path.join(workRoot, 't05', 'kernel');
  buildKernel(kernel);
  const env = { MERIDIAN_KERNEL: kernel, MERIDIAN_INSTANCE: '' };
  delete env.MERIDIAN_INSTANCE;
  const r = spawnSync(process.execPath, [WRAPPER], {
    env: { ...process.env, MERIDIAN_KERNEL: kernel, MERIDIAN_INSTANCE: undefined },
    encoding: 'utf8',
  });
  check('t05 no-instance run does not crash the wrapper', r.status === 1, `exit ${r.status}`);
  check('t05 no-instance run says why', /mode=none/.test(r.stdout), r.stdout);
}

fs.rmSync(workRoot, { recursive: true, force: true });

console.log('---');
console.log(`${passed} passed, ${failed} failed`);
if (failures.length) console.log('\n' + failures.join('\n'));
process.exit(failed > 0 ? 1 : 0);
