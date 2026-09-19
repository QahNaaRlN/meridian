#!/usr/bin/env node
// Regression suite for scripts/preflight.mjs.
//
// Kernel development is self-sufficient (AGENTS.md §1/§2): a bare run must
// pass without MERIDIAN_INSTANCE, and the old strict Instance contract only
// applies to the explicit transitional --require-instance mode. This suite
// proves both halves and the CLI's exit-code honesty around them.
//
// Usage: node test/preflight.test.mjs

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SCRIPT = path.join(__dirname, '..', 'scripts', 'preflight.mjs');

let passed = 0;
let failed = 0;
const failures = [];

function check(name, cond, detail) {
  if (cond) { passed++; console.log(`ok    ${name}`); }
  else { failed++; failures.push(`${name}: ${detail}`); console.log(`FAIL  ${name}`); }
}

function write(root, rel, content) {
  const p = path.join(root, rel);
  fs.mkdirSync(path.dirname(p), { recursive: true });
  fs.writeFileSync(p, content);
  return p;
}

function buildKernel(root) {
  write(root, 'VERSION', '0.0.0-test\n');
  write(root, 'scripts/kernel-validate.mjs', '// synthetic\n');
  fs.mkdirSync(path.join(root, '.git'), { recursive: true }); // presence check only
}

function run(args, env) {
  const r = spawnSync(process.execPath, [SCRIPT, ...args], {
    env: { ...process.env, ...env },
    encoding: 'utf8',
  });
  return { out: `${r.stdout}\n${r.stderr}`, code: r.status };
}

const workRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'meridian-preflight-test-'));

function freshKernel(name) {
  const kernel = path.join(workRoot, name, 'kernel');
  buildKernel(kernel);
  return kernel;
}

// t01 — Kernel-only, no MERIDIAN_INSTANCE at all: passes.
{
  const kernel = freshKernel('t01');
  const env = { ...process.env, MERIDIAN_KERNEL: kernel };
  delete env.MERIDIAN_INSTANCE;
  const r = spawnSync(process.execPath, [SCRIPT], { env, encoding: 'utf8' });
  check('t01 kernel-only run passes without MERIDIAN_INSTANCE',
    r.status === 0, `exit ${r.status}: ${r.stdout}${r.stderr}`);
  check('t01 kernel-only run does not claim to have checked an Instance',
    !/instance root:/.test(r.stdout),
    `unexpected instance-root line: ${r.stdout}`);
}

// t01c — Kernel-only run with MERIDIAN_INSTANCE set to a fake absolute path:
// the run must still pass, and the path value must never appear in output.
{
  const kernel = freshKernel('t01c');
  const fakeInstancePath = path.join(workRoot, 't01c', 'fictitious-product-instance-path-marker');
  const env = { ...process.env, MERIDIAN_KERNEL: kernel, MERIDIAN_INSTANCE: fakeInstancePath };
  const r = spawnSync(process.execPath, [SCRIPT], { env, encoding: 'utf8' });
  const combined = `${r.stdout}${r.stderr}`;
  check('t01c kernel-only run with MERIDIAN_INSTANCE set still passes',
    r.status === 0, `exit ${r.status}: ${combined}`);
  check('t01c kernel-only run never prints the MERIDIAN_INSTANCE value',
    !combined.includes(fakeInstancePath) && !combined.includes('fictitious-product-instance-path-marker'),
    `the fake instance path leaked into output: ${combined}`);
  check('t01c kernel-only run reports the variable is set but not checked',
    /MERIDIAN_INSTANCE is set but not required or checked/.test(r.stdout),
    `missing expected INFO line: ${r.stdout}`);
}

// t02 — --require-instance without MERIDIAN_INSTANCE: rejected.
{
  const kernel = freshKernel('t02');
  const env = { ...process.env, MERIDIAN_KERNEL: kernel };
  delete env.MERIDIAN_INSTANCE;
  const r = spawnSync(process.execPath, [SCRIPT, '--require-instance'], { env, encoding: 'utf8' });
  check('t02 --require-instance without the variable is rejected',
    r.status === 1 && /--require-instance was given but MERIDIAN_INSTANCE is not set/.test(r.stdout),
    `exit ${r.status}: ${r.stdout}${r.stderr}`);
}

// t03 — --require-instance with a correct, valid source: accepted.
{
  const kernel = freshKernel('t03');
  const instance = path.join(workRoot, 't03', 'instance');
  write(instance, 'product.yaml', 'schema_version: 1\nproduct:\n  name: Quorvath\n');
  fs.mkdirSync(path.join(instance, '.git'), { recursive: true });
  const res = run(['--require-instance'], { MERIDIAN_KERNEL: kernel, MERIDIAN_INSTANCE: instance });
  check('t03 --require-instance with a valid Instance is accepted',
    res.code === 0, `exit ${res.code}: ${res.out}`);
}

// t04 — --require-instance with an incorrect source (no product.yaml): rejected.
{
  const kernel = freshKernel('t04');
  const instance = path.join(workRoot, 't04', 'not-an-instance');
  fs.mkdirSync(instance, { recursive: true });
  const res = run(['--require-instance'], { MERIDIAN_KERNEL: kernel, MERIDIAN_INSTANCE: instance });
  check('t04 --require-instance with an invalid Instance is rejected',
    res.code === 1 && /no product\.yaml; this is not a Meridian instance/.test(res.out),
    `exit ${res.code}: ${res.out}`);
}

// t05 — an unknown argument is rejected, with or without --require-instance.
{
  const kernel = freshKernel('t05');
  const res = run(['--bogus-flag'], { MERIDIAN_KERNEL: kernel });
  check('t05 an unknown argument is rejected',
    res.code === 1 && /unknown argument/.test(res.out),
    `exit ${res.code}: ${res.out}`);
}
{
  const kernel = freshKernel('t05b');
  const res = run(['--require-instance', '--bogus-flag'], { MERIDIAN_KERNEL: kernel });
  check('t05b an unknown argument is rejected alongside a known one',
    res.code === 1 && /unknown argument/.test(res.out),
    `exit ${res.code}: ${res.out}`);
}

console.log('---');
console.log(`${passed} passed, ${failed} failed`);
if (failures.length) console.log('\n' + failures.join('\n'));
process.exit(failed > 0 ? 1 : 0);
