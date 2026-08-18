#!/usr/bin/env node
// Regression suite for scripts/kernel-validate.mjs.
//
// Every rule the validator claims to enforce is exercised adversarially here:
// a synthetic kernel and instance are built in a temp directory, a defect is
// planted, and the suite asserts that the run actually goes red with the
// expected message. A rule whose test cannot run (e.g. symlinks on a platform
// that forbids them) is reported as SKIP, never silently passed — the same
// UNVERIFIED-over-OK discipline the validator itself follows.
//
// The product names and paths used below are fictional. This file is part of
// the Kernel and is subject to the same purity scan as everything else, so it
// must never contain a real product's literals; the personal-path plant is
// assembled from fragments for the same reason the validator's own regex is.
//
// Usage: node test/kernel-validate.test.mjs

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import { spawnSync, execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const VALIDATOR = path.join(__dirname, '..', 'scripts', 'kernel-validate.mjs');

let passed = 0;
let failed = 0;
let skipped = 0;
const failures = [];

function sh(cmd, args, cwd) {
  execFileSync(cmd, args, { cwd, stdio: ['ignore', 'ignore', 'ignore'] });
}

try {
  sh('git', ['--version'], process.cwd());
} catch {
  console.error('SKIP-ALL: git is unavailable; the suite cannot build tracked synthetic kernels.');
  process.exit(2);
}

const workRoot = fs.mkdtempSync(path.join(os.tmpdir(), 'meridian-validate-test-'));

function write(root, rel, content) {
  const p = path.join(root, rel);
  fs.mkdirSync(path.dirname(p), { recursive: true });
  fs.writeFileSync(p, content);
  return p;
}

function sha256(text) {
  return crypto.createHash('sha256').update(text, 'utf8').digest('hex');
}

// Fictional literals only. Never real product terms.
const LIT = 'zephyrblade';
const PAT_WORD = 'Gleamfall';

function buildKernel(root) {
  write(root, 'README.md', '# Synthetic kernel\n\nSee [the note](docs/note.md).\n');
  write(root, 'docs/note.md', 'A note.\n');
  const skill = '# Demo skill\n\nA fictional vendored skill used only by this suite.\n';
  write(root, 'skills/demo/SKILL.md', skill);
  write(root, 'skills/demo/PIN.yaml', [
    'schema_version: 1',
    'name: demo',
    'artifact: SKILL.md',
    `sha256: ${sha256(skill)}`,
    'state: vendored',
    "pinned_at: '2026-01-01'",
    'pinned_by: test-suite',
    '',
  ].join('\n'));
  sh('git', ['init', '-q'], root);
  sh('git', ['add', '-A'], root);
  sh('git', ['-c', 'user.name=t', '-c', 'user.email=t@t.invalid', 'commit', '-q', '-m', 'synthetic'], root);
}

function buildInstance(root) {
  write(root, 'product.yaml', [
    'schema_version: 1',
    'product:',
    '  name: Quorvath',
    '  description: Fictional product used only by the validator test suite.',
    'canonical_wiki:',
    '  tool: none',
    '  base_url: https://wiki.quorvath.invalid',
    '  space_key: QVX',
    'tooling:',
    '  task_tracker:',
    '    base_url: https://tracker.quorvath.invalid/',
    '  repository:',
    '    base_url: https://scm.quorvath.invalid',
    '    namespace: quorvath/apps',
    'forbidden_literals:',
    `  - ${LIT}`,
    'forbidden_patterns:',
    `  - '\\b${PAT_WORD}\\b'`,
    'owner: test-suite',
    "last_verified: '2026-01-01'",
    '',
  ].join('\n'));
  write(root, 'external-dependencies.yaml', 'schema_version: 1\ndependencies: []\n');
  write(root, 'inventory/repositories.yaml', 'schema_version: 1\nrepositories: []\n');
  write(root, 'data/thing.schema.json', JSON.stringify({
    $schema: 'http://json-schema.org/draft-07/schema#',
    type: 'object',
    required: ['schema_version', 'when'],
    properties: {
      $schema: { type: 'string' },
      schema_version: { const: 1 },
      when: { type: 'string', format: 'date-time' },
      maybe: { type: 'string' },
    },
  }, null, 2));
  write(root, 'data/thing.yaml', [
    '$schema: ./thing.schema.json',
    'schema_version: 1',
    "when: '2026-01-01T00:00:00Z'",
    '',
  ].join('\n'));
}

function freshPair(name) {
  const kernel = path.join(workRoot, name, 'kernel');
  const instance = path.join(workRoot, name, 'instance');
  buildKernel(kernel);
  buildInstance(instance);
  return { kernel, instance };
}

function run(kernel, instance) {
  const env = { ...process.env, MERIDIAN_KERNEL: kernel };
  if (instance) env.MERIDIAN_INSTANCE = instance;
  else delete env.MERIDIAN_INSTANCE;
  const r = spawnSync(process.execPath, [VALIDATOR], { env, encoding: 'utf8' });
  return { out: `${r.stdout}\n${r.stderr}`, code: r.status };
}

function check(name, res, { expectExit, mustMatch = [], mustNotMatch = [] }) {
  const problems = [];
  if (expectExit !== undefined && res.code !== expectExit) {
    problems.push(`exit ${res.code}, expected ${expectExit}`);
  }
  for (const re of mustMatch) if (!re.test(res.out)) problems.push(`missing ${re}`);
  for (const re of mustNotMatch) if (re.test(res.out)) problems.push(`unexpected ${re}`);
  if (problems.length) {
    failed++;
    failures.push(`${name}: ${problems.join('; ')}\n--- output ---\n${res.out.trim()}\n---`);
    console.log(`FAIL  ${name}`);
  } else {
    passed++;
    console.log(`ok    ${name}`);
  }
}

// t01 — clean baseline is green and reports full coverage
{
  const { kernel, instance } = freshPair('t01');
  check('t01 clean baseline passes', run(kernel, instance), {
    expectExit: 0,
    mustMatch: [/kernel-purity: \d+ tracked text files clean/],
    mustNotMatch: [/^FAIL/m],
  });
}

// t02 — a product literal planted in a tracked file goes red
{
  const { kernel, instance } = freshPair('t02');
  fs.appendFileSync(path.join(kernel, 'docs', 'note.md'), `mentions ${LIT} here\n`);
  check('t02 planted literal detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/kernel-purity: product literal/],
  });
}

// t03 — a forbidden case-sensitive pattern goes red
{
  const { kernel, instance } = freshPair('t03');
  fs.appendFileSync(path.join(kernel, 'README.md'), `The ${PAT_WORD} backend.\n`);
  check('t03 forbidden pattern detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/kernel-purity: forbidden pattern/],
  });
}

// t04 — a personal home-directory path goes red (assembled from fragments so
// this file does not itself match the validator's scan)
{
  const { kernel, instance } = freshPair('t04');
  const personal = ['C:', '\\', 'Users', '\\', 'someone', '\\', 'notes.txt'].join('');
  fs.appendFileSync(path.join(kernel, 'docs', 'note.md'), `see ${personal}\n`);
  check('t04 personal path detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/kernel-purity: personal home path/],
  });
}

// t05 — a relative link that leaves the kernel root goes red
{
  const { kernel, instance } = freshPair('t05');
  write(path.dirname(kernel), 'outside.md', 'outside\n');
  fs.appendFileSync(path.join(kernel, 'README.md'), '[esc](../outside.md)\n');
  check('t05 link escaping the kernel detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/points outside the Kernel/],
  });
}

// t06 — a link that stays inside the kernel textually but escapes through a
// symlinked directory goes red (SKIP where symlinks cannot be created)
{
  const { kernel, instance } = freshPair('t06');
  const outside = path.join(path.dirname(kernel), 'elsewhere');
  write(outside, 'esc.md', 'elsewhere\n');
  let linked = false;
  try {
    fs.symlinkSync(outside, path.join(kernel, 'sub'), 'junction');
    linked = true;
  } catch {
    skipped++;
    console.log('SKIP  t06 symlink escape (symlinks cannot be created on this system)');
  }
  if (linked) {
    fs.appendFileSync(path.join(kernel, 'README.md'), '[esc](sub/esc.md)\n');
    check('t06 symlink escape detected', run(kernel, instance), {
      expectExit: 1,
      mustMatch: [/points outside the Kernel/],
    });
  }
}

// t07 — a dangling relative link goes red
{
  const { kernel, instance } = freshPair('t07');
  fs.appendFileSync(path.join(kernel, 'README.md'), '[gone](docs/missing.md)\n');
  check('t07 dangling link detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/does not resolve/],
  });
}

// t08 — a format violation in schema-validated data goes red
{
  const { kernel, instance } = freshPair('t08');
  write(instance, 'data/thing.yaml', "$schema: ./thing.schema.json\nschema_version: 1\nwhen: 'yesterday'\n");
  check('t08 format violation detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/does not satisfy format "date-time"/],
  });
}

// t09 — an unsupported keyword in a branch the data never visits goes red
{
  const { kernel, instance } = freshPair('t09');
  const p = path.join(instance, 'data', 'thing.schema.json');
  const s = JSON.parse(fs.readFileSync(p, 'utf8'));
  s.properties.never_used = { patternProperties: { '^a': { type: 'string' } } };
  fs.writeFileSync(p, JSON.stringify(s, null, 2));
  check('t09 unvisited unsupported keyword detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/keyword "patternProperties".*not implemented/],
  });
}

// t10 — an unimplemented format value goes red instead of passing silently
{
  const { kernel, instance } = freshPair('t10');
  const p = path.join(instance, 'data', 'thing.schema.json');
  const s = JSON.parse(fs.readFileSync(p, 'utf8'));
  s.properties.maybe = { type: 'string', format: 'email' };
  fs.writeFileSync(p, JSON.stringify(s, null, 2));
  check('t10 unknown format value detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/format "email".*not implemented/],
  });
}

// t11 — a vendored artifact that does not match its pin goes red
{
  const { kernel, instance } = freshPair('t11');
  fs.appendFileSync(path.join(kernel, 'skills', 'demo', 'SKILL.md'), 'tampered\n');
  check('t11 sha-provenance mismatch detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/sha-provenance: demo artifact .* != pinned/],
  });
}

// t12 — a declared-but-missing instance context file goes red
{
  const { kernel, instance } = freshPair('t12');
  fs.appendFileSync(path.join(kernel, 'skills', 'demo', 'PIN.yaml'),
    'requires_instance_context: skills/demo/context.md\n');
  check('t12 missing instance context detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/instance-context: demo requires .* missing from the Instance/],
  });
}

// t13 — an orphaned trailing Front Matter block goes red
{
  const { kernel, instance } = freshPair('t13');
  write(kernel, 'docs/note.md',
    '---\ntitle: note\n---\n\nBody.\n\n---\nstatus: active\n---\n');
  check('t13 duplicate front matter detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/duplicate-fm/],
  });
}

// t14 — a run without an Instance declares itself unverified and exits red
{
  const { kernel } = freshPair('t14');
  check('t14 missing instance is an explicit failure', run(kernel, null), {
    expectExit: 1,
    mustMatch: [/MERIDIAN_INSTANCE is not set/],
  });
}

// t15 — an instance tracked inside the kernel (fixture) is excluded from the
// purity scan instead of failing on a self-match, and the exclusion is printed
{
  const { kernel } = freshPair('t15');
  const fixture = path.join(kernel, 'test-instance');
  buildInstance(fixture);
  sh('git', ['add', '-A'], kernel);
  sh('git', ['-c', 'user.name=t', '-c', 'user.email=t@t.invalid', 'commit', '-q', '-m', 'fixture'], kernel);
  check('t15 in-kernel instance excluded from purity scan', run(kernel, fixture), {
    expectExit: 0,
    mustMatch: [/excluded from the Kernel scan/],
    mustNotMatch: [/kernel-purity: product literal/],
  });
}

fs.rmSync(workRoot, { recursive: true, force: true });

console.log('---');
console.log(`${passed} passed, ${failed} failed, ${skipped} skipped`);
if (failures.length) {
  console.log('\n' + failures.join('\n\n'));
}
process.exit(failed > 0 ? 1 : 0);
