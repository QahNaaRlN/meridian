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

// Front Matter for a synthetic Kernel document. The suite's own kernel has to
// satisfy document-identity, otherwise every later case would go red for the
// baseline's sake rather than for the defect it plants.
function fm(title, type, extra = []) {
  return ['---', `title: ${title}`, `document_type: ${type}`, 'status: maintained',
          'scope: workspace', 'owner: test-suite', 'created: 2026-01-01',
          'updated: 2026-01-01', ...extra, '---', ''].join('\n');
}

// The topic pool is deliberately split between data and prose, so the synthetic
// kernel carries a miniature of both halves: without them every case would run
// against an absent pool and the drift check would never be exercised.
// `outside` places signature-shaped rows after the end marker: material the
// gate must refuse to count, however much it looks like the pool.
function writeTopicPool(root, { names = ['naming'], documented = ['naming'], outside = [] } = {}) {
  write(root, 'standards/workspace/instruction-topics.yaml',
    ['schema_version: 1', 'topics:', ...names.map((n) => `  - ${n}`), ''].join('\n'));
  write(root, 'standards/workspace/instruction-topics.md',
    `${fm('Synthetic topics', 'reference')}\n# Synthetic topics\n\n`
    + '<!-- meridian:begin topic-pool -->\n\n| Тема | Вопрос |\n|---|---|\n'
    + documented.map((n) => `| \`${n}\` | synthetic |\n`).join('')
    + '\n<!-- meridian:end topic-pool -->\n'
    + (outside.length
      ? `\n## Прочее\n\n| Что | Зачем |\n|---|---|\n${outside.map((n) => `| \`${n}\` | вне пула |\n`).join('')}`
      : ''));
}

function buildKernel(root) {
  write(root, 'README.md', `${fm('Synthetic kernel', 'readme')}\n# Synthetic kernel\n\nSee [the note](docs/note.md).\n`);
  writeTopicPool(root);
  write(root, 'docs/note.md', `${fm('A note', 'reference')}\n# A note\n\nA note.\n`);
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

function commitAll(root) {
  sh('git', ['add', '-A'], root);
  sh('git', ['-c', 'user.name=t', '-c', 'user.email=t@t.invalid', 'commit', '-q', '-m', 'planted'], root);
}

function freshPair(name) {
  const kernel = path.join(workRoot, name, 'kernel');
  const instance = path.join(workRoot, name, 'instance');
  buildKernel(kernel);
  buildInstance(instance);
  return { kernel, instance };
}

function run(kernel, instance, extraEnv = {}) {
  const env = { ...process.env, MERIDIAN_KERNEL: kernel, ...extraEnv };
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

// t16 — a file name that breaks the level/case rule goes red
{
  const { kernel, instance } = freshPair('t16');
  write(kernel, 'docs/BadName.md', `${fm('Bad name', 'reference')}\n# Bad name\n`);
  commitAll(kernel);
  check('t16 non-conforming file name detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/document-identity: "docs\/BadName\.md" is not lower kebab-case/],
  });
}

// t17 — a Kernel document with no Front Matter goes red
{
  const { kernel, instance } = freshPair('t17');
  write(kernel, 'docs/plain.md', '# Plain\n\nNo Front Matter at all.\n');
  commitAll(kernel);
  check('t17 missing front matter detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/document-identity: docs\/plain\.md has no Front Matter/],
  });
}

// t18 — a document_type outside the closed pool goes red
{
  const { kernel, instance } = freshPair('t18');
  write(kernel, 'docs/typed.md', `${fm('Invented', 'memo')}\n# Invented\n`);
  commitAll(kernel);
  check('t18 type outside the pool detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/declares document_type "memo", which is not in the pool/],
  });
}

// t19 — unclassified without a recorded reason goes red; with one it passes,
// so the state itself is not treated as a defect
{
  const { kernel, instance } = freshPair('t19');
  write(kernel, 'docs/unclear.md', `${fm('Unclear', 'unclassified')}\n# Unclear\n`);
  commitAll(kernel);
  check('t19 unclassified without a reason detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/is unclassified with no unclassified_reason/],
  });
}

// t20 — a template body carries no Front Matter by design and is not flagged
{
  const { kernel, instance } = freshPair('t20');
  write(kernel, 'docs/adr-body.md', '| Field | Value |\n| --- | --- |\n');
  write(kernel, 'docs/unclear.md', `${fm('Unclear', 'unclassified', ['unclassified_reason: two signatures at once'])}\n# Unclear\n`);
  commitAll(kernel);
  check('t20 templates exempt and a reasoned unclassified passes', run(kernel, instance), {
    expectExit: 0,
    mustMatch: [/document-identity: \d+ tracked names conform/],
    mustNotMatch: [/document-identity: docs\/adr-body\.md/, /unclassified with no/],
  });
}

// t21 — "skill" was a type until it was found to duplicate "protocol" and to
// differ from it only in packaging. Its removal is enforced, not just written
// down: a document declaring the retired type goes red like any other name
// outside the pool.
{
  const { kernel, instance } = freshPair('t21');
  write(kernel, 'docs/packaged.md', `${fm('Packaged', 'skill')}\n# Packaged\n`);
  commitAll(kernel);
  check('t21 retired type "skill" is rejected by the pool', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/declares document_type "skill", which is not in the pool/],
  });
}

// --- instruction-intake -----------------------------------------------------
// The register's shape is checked by the generic $schema pass; these cases
// exercise the three claims that need their own code. The real schema is copied
// into each synthetic kernel rather than re-invented here, so a schema the
// validator cannot load would turn these cases red too.
const REAL_INTAKE_SCHEMA = path.join(__dirname, '..', 'registries', 'instruction-intake', 'intake.schema.json');

function plantIntake(kernel, instance, { records, repoPath }) {
  write(kernel, 'registries/instruction-intake/intake.schema.json', fs.readFileSync(REAL_INTAKE_SCHEMA, 'utf8'));
  write(instance, 'inventory/repositories.yaml', [
    'schema_version: 1',
    'repositories:',
    '  - id: synthetic',
    `    path: ${repoPath.split(path.sep).join('/')}`,
    "    last_verified: '2026-01-01'",
    '',
  ].join('\n'));
  write(instance, 'instruction-intake/synthetic.yaml', [
    '$schema: ./intake.schema.json',
    'schema_version: 1',
    'repository: synthetic',
    'records:',
    ...records,
    '',
  ].join('\n'));
}

function intakeRecord(artifact, digest, { verdict = 'keep-local', topic = 'naming', delivery = 'cursor-rule', extra = [], recordedAt = '2026-01-01' } = {}) {
  return [
    `  - artifact: ${artifact}`,
    `    digest: ${digest}`,
    `    delivery: ${delivery}`,
    `    topic: ${topic}`,
    '    genre: standard',
    '    scope: repository',
    '    observed_activation: always',
    `    verdict: ${verdict}`,
    '    verdict_basis: synthetic case',
    ...extra,
    `    recorded_at: '${recordedAt}'`,
  ];
}

// t22 — a norm present in the tree but absent from the register goes red:
// completeness is the one thing here that is mechanically provable
{
  const { kernel, instance } = freshPair('t22');
  plantIntake(kernel, instance, { records: intakeRecord('README.md', sha256('x')), repoPath: kernel });
  commitAll(kernel);
  check('t22 norm in the tree with no record detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/instruction-intake: synthetic — \d+ norm\(s\) in the tree have no record/],
  });
}

// t23 — a record removed from the working file, before the removal is
// committed. The easiest half of preservation, and the only half the first
// implementation could see; t36 and t37 cover the halves it could not.
{
  const { kernel, instance } = freshPair('t23');
  const both = [...intakeRecord('skills/demo/SKILL.md', sha256('a')), ...intakeRecord('README.md', sha256('b'))];
  plantIntake(kernel, instance, { records: both, repoPath: kernel });
  commitAll(kernel);
  sh('git', ['init', '-q'], instance);
  commitAll(instance);
  // the second entry is dropped from the working file after the commit
  plantIntake(kernel, instance, { records: intakeRecord('skills/demo/SKILL.md', sha256('a')), repoPath: kernel });
  check('t23 uncommitted loss of a record detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/instruction-intake: instruction-intake\/synthetic\.yaml lost 1 record\(s\)/, /append-only/],
  });
}

// A parent reference is a triple: repository, path, revision. The helper below
// builds one, so the cases that follow differ only in the thing under test.
function editionRecord({ repository = 'synthetic', parentPath = 'docs/note.md', revision, digest }) {
  return intakeRecord('README.md', sha256('x'), {
    verdict: 'adopt-edition',
    extra: [
      '    derived_from:',
      `      repository: ${repository}`,
      `      path: ${parentPath}`,
      `      revision: ${revision}`,
      `    derived_from_digest: ${digest}`,
      '    narrowing:',
      '      - synthetic narrowing',
    ],
  });
}
function headOf(repo) {
  return execFileSync('git', ['rev-parse', 'HEAD'], { cwd: repo, encoding: 'utf8' }).trim();
}
// The register must be complete for the synthetic tree, so that the parent
// reference is the only thing in these cases that can go red.
const completingRecord = () => intakeRecord('skills/demo/SKILL.md', sha256('b'));

// t24 — the reference resolves at the revision it names, and that revision
// holds a different text than the digest claims. No re-dating can fix it: the
// record names the wrong parent.
{
  const { kernel, instance } = freshPair('t24');
  plantIntake(kernel, instance, { records: completingRecord(), repoPath: kernel });
  commitAll(kernel);
  const edition = editionRecord({
    revision: headOf(kernel),
    digest: sha256('a text that docs/note.md is not'),
  });
  plantIntake(kernel, instance, { records: [...edition, ...completingRecord()], repoPath: kernel });
  check('t24 a parent reference naming a different text detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/README\.md records parent digest .* but "docs\/note\.md" at [0-9a-f]{7} hashes to/],
  });
}

// t25 — a record naming a topic outside the pool goes red: existence of the
// topic is checkable, its correctness is not, and only the first is claimed
{
  const { kernel, instance } = freshPair('t25');
  const records = [
    ...intakeRecord('skills/demo/SKILL.md', sha256('a'), { topic: 'invented-topic', delivery: 'skill-package' }),
  ];
  plantIntake(kernel, instance, { records, repoPath: kernel });
  commitAll(kernel);
  check('t25 topic outside the pool detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/instruction-intake: .* names 1 topic\(s\) outside the pool \(invented-topic\)/],
  });
}

// t26 — the two halves of the pool disagreeing goes red before any record is
// judged: a name with no signature and a signature with no name are both drift
{
  const { kernel, instance } = freshPair('t26');
  writeTopicPool(kernel, { names: ['naming', 'orphan'], documented: ['naming'] });
  commitAll(kernel);
  check('t26 topic pool halves disagreeing detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/instruction-topics: the two halves of the pool disagree .*orphan/],
  });
}

// t27 — packaging discipline: a skill whose directory does not carry its name,
// and a package smuggling the rule vocabulary into its own front matter
{
  const { kernel, instance } = freshPair('t27');
  write(kernel, 'skills/demo/SKILL.md', '---\nname: something-else\nalwaysApply: true\n---\n\n# Demo skill\n');
  write(kernel, 'skills/demo/PIN.yaml', [
    'schema_version: 1', 'name: demo', 'artifact: SKILL.md',
    `sha256: ${sha256(fs.readFileSync(path.join(kernel, 'skills/demo/SKILL.md'), 'utf8'))}`,
    'state: vendored', "pinned_at: '2026-01-01'", 'pinned_by: test-suite', '',
  ].join('\n'));
  plantIntake(kernel, instance, {
    records: intakeRecord('skills/demo/SKILL.md', sha256('a'), { delivery: 'skill-package' }),
    repoPath: kernel,
  });
  commitAll(kernel);
  check('t27 skill name and double activation vocabulary detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [
      /is a skill named "something-else" in a directory named "demo"/,
      /declares activation twice — alwaysApply/,
    ],
  });
}

// t28 — a file that exists in the tree but was never added is invisible to
// every check above. The run stays green, because nothing was found wrong;
// the point of the case is that it must not stay silent. Ignored files are
// deliberately outside the release unit and must not be reported.
{
  const { kernel, instance } = freshPair('t28');
  write(kernel, '.gitignore', 'ignored/\n');
  commitAll(kernel);
  write(kernel, 'docs/unseen.md', '# Unseen\n\nNo Front Matter, and never added.\n');
  write(kernel, 'ignored/local-note.md', '# Local\n');
  check('t28 untracked file reported, ignored file not', run(kernel, instance), {
    expectExit: 0,
    mustMatch: [/kernel-purity: 1 file\(s\) in the Kernel tree are untracked .*docs\/unseen\.md/],
    mustNotMatch: [/local-note/],
  });
}

// t29 — the same file, added: the defect the previous case could not see is
// now found. Together the two cases show the WARN is not decorative.
{
  const { kernel, instance } = freshPair('t29');
  write(kernel, 'docs/unseen.md', '# Unseen\n\nNo Front Matter, and never added.\n');
  commitAll(kernel);
  check('t29 the same file, tracked, goes red', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/document-identity: docs\/unseen\.md has no Front Matter/],
    mustNotMatch: [/are untracked/],
  });
}

// t30 — a signature-shaped row outside the marked region must not be counted
// as documentation. Declared "orphan" is documented nowhere inside the pool
// region; the row that looks like its signature sits outside it, and the drift
// has to surface anyway.
{
  const { kernel, instance } = freshPair('t30');
  writeTopicPool(kernel, { names: ['naming', 'orphan'], documented: ['naming'], outside: ['orphan'] });
  commitAll(kernel);
  check('t30 a table outside the pool region does not mask a drift', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/instruction-topics: the two halves of the pool disagree .*orphan/],
  });
}

// t31 — an absent pool fails closed. Nothing in any register can be judged
// against a pool that is not there, and a warning would let invented topics
// through green.
{
  const { kernel, instance } = freshPair('t31');
  fs.rmSync(path.join(kernel, 'standards', 'workspace', 'instruction-topics.yaml'));
  commitAll(kernel);
  check('t31 an absent topic pool fails closed', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/instruction-topics: the topic pool is missing from the Kernel/],
  });
}

// t32 — markers that do not pair up are an error, not a reason to read the
// whole file instead
{
  const { kernel, instance } = freshPair('t32');
  const p = path.join(kernel, 'standards', 'workspace', 'instruction-topics.md');
  fs.writeFileSync(p, fs.readFileSync(p, 'utf8').replace('<!-- meridian:end topic-pool -->', ''));
  commitAll(kernel);
  check('t32 unbalanced pool markers fail closed', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/pool region of instruction-topics\.md is not readable/],
  });
}

// --- conditional obligations of the record schema ---------------------------
// Eight if/then blocks decide what a verdict must carry with it. Two of them
// are the ones that let a decision be recorded without its justification, and
// both are exercised in the two directions that matter: the defective record
// is refused, and the minimal legal one passes. A conditional block that
// refused everything would look identical in a red-only test.

// t33 — retiring a norm without naming either a successor or a reason
{
  const { kernel, instance } = freshPair('t33');
  plantIntake(kernel, instance, {
    records: intakeRecord('skills/demo/SKILL.md', sha256('a'), { verdict: 'retire' }),
    repoPath: kernel,
  });
  commitAll(kernel);
  check('t33 retire without successor or reason detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/schema: .*\/records\/0\/allOf\/\d+: matches none of anyOf/],
  });
}

// t34 — the same verdict with a reason and no successor is legal: content can
// stop holding without going anywhere
{
  const { kernel, instance } = freshPair('t34');
  plantIntake(kernel, instance, {
    records: intakeRecord('skills/demo/SKILL.md', sha256('a'), {
      verdict: 'retire',
      extra: ['    retire_reason: the practice it described no longer exists'],
    }),
    repoPath: kernel,
  });
  commitAll(kernel);
  check('t34 retire with a reason alone passes', run(kernel, instance), {
    expectExit: 0,
    mustNotMatch: [/matches none of anyOf/],
  });
}

// t35 — deferring without saying what would bring the decision back
{
  const { kernel, instance } = freshPair('t35');
  plantIntake(kernel, instance, {
    records: intakeRecord('skills/demo/SKILL.md', sha256('a'), { verdict: 'deferred' }),
    repoPath: kernel,
  });
  commitAll(kernel);
  check('t35 deferred without a resume condition detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/schema: .*\/records\/0\/allOf\/\d+\/resume_condition: required property missing/],
  });
}

// --- preservation, the two halves the path-set comparison could not see ------

// t36 — the loss is committed. Comparing the working file with HEAD sees
// nothing here: after the commit, HEAD is the truncated file.
{
  const { kernel, instance } = freshPair('t36');
  const both = [...intakeRecord('skills/demo/SKILL.md', sha256('a')), ...intakeRecord('README.md', sha256('b'))];
  plantIntake(kernel, instance, { records: both, repoPath: kernel });
  commitAll(kernel);
  sh('git', ['init', '-q'], instance);
  commitAll(instance);
  plantIntake(kernel, instance, { records: intakeRecord('skills/demo/SKILL.md', sha256('a')), repoPath: kernel });
  commitAll(instance);
  check('t36 committed loss of a record detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/lost 1 record\(s\) that earlier revisions of this file carried/, /README\.md @ /],
  });
}

// t37 — the artifact keeps its path and loses one of its two decisions. A
// comparison over the set of artifact paths sees a complete register.
{
  const { kernel, instance } = freshPair('t37');
  const deferred = intakeRecord('skills/demo/SKILL.md', sha256('a'), {
    verdict: 'deferred', extra: ['    resume_condition: the owner decides whether the practice survives'],
  });
  const retired = intakeRecord('skills/demo/SKILL.md', sha256('a'), {
    verdict: 'retire', extra: ['    retire_reason: the practice it described no longer exists'],
  });
  plantIntake(kernel, instance, { records: [...deferred, ...retired], repoPath: kernel });
  commitAll(kernel);
  sh('git', ['init', '-q'], instance);
  commitAll(instance);
  plantIntake(kernel, instance, { records: retired, repoPath: kernel });
  check('t37 a lost decision behind a surviving path detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/lost 1 record\(s\) that earlier revisions of this file carried/, /→ deferred/],
  });
}

// t38 — local history rewritten so that no revision of the file ever held the
// lost record. Level 1 is blind by construction; the published branch is not.
// Both directions are asserted in one case, because the pair is the argument
// for the second level existing at all.
{
  const { kernel, instance } = freshPair('t38');
  const both = [...intakeRecord('skills/demo/SKILL.md', sha256('a')), ...intakeRecord('README.md', sha256('b'))];
  plantIntake(kernel, instance, { records: both, repoPath: kernel });
  commitAll(kernel);
  sh('git', ['init', '-q'], instance);
  commitAll(instance);
  const remote = path.join(workRoot, 't38', 'remote.git');
  sh('git', ['init', '-q', '--bare', remote], workRoot);
  sh('git', ['remote', 'add', 'origin', remote], instance);
  const branch = execFileSync('git', ['rev-parse', '--abbrev-ref', 'HEAD'], { cwd: instance, encoding: 'utf8' }).trim();
  sh('git', ['push', '-q', '-u', 'origin', branch], instance);
  // the record is dropped and the commit that carried it is amended away
  plantIntake(kernel, instance, { records: intakeRecord('skills/demo/SKILL.md', sha256('a')), repoPath: kernel });
  sh('git', ['add', '-A'], instance);
  sh('git', ['-c', 'user.name=t', '-c', 'user.email=t@t.invalid', 'commit', '-q', '--amend', '-m', 'rewritten'], instance);
  check('t38 local history is blind to an amended-away record', run(kernel, instance), {
    expectExit: 0,
    mustNotMatch: [/lost 1 record/],
  });
  check('t38 the published branch still remembers it', run(kernel, instance, { MERIDIAN_CHECK_PUBLISHED: '1' }), {
    expectExit: 1,
    mustMatch: [/lost 1 record\(s\) that are published on origin\//, /rewriting local history does not unpublish a decision/],
  });
}

// t39 — asked to compare with the published state and given no upstream to
// compare with: a warning that names the gap, not a silent pass
{
  const { kernel, instance } = freshPair('t39');
  plantIntake(kernel, instance, { records: intakeRecord('skills/demo/SKILL.md', sha256('a')), repoPath: kernel });
  commitAll(kernel);
  sh('git', ['init', '-q'], instance);
  commitAll(instance);
  check('t39 no upstream is reported, not skipped', run(kernel, instance, { MERIDIAN_CHECK_PUBLISHED: '1' }), {
    expectExit: 0,
    mustMatch: [/WARN.*the Instance has no upstream branch; the published state of the register was NOT compared/],
  });
}

// --- what a qualified parent reference buys ---------------------------------

// t40 — the parent legitimately moves on after the edition was taken. The
// record is not false: it is anchored to a revision that still holds exactly
// what it claims. The finding is that the edition has fallen behind, and a
// red run here would teach the operator to re-date the entry instead.
{
  const { kernel, instance } = freshPair('t40');
  plantIntake(kernel, instance, { records: completingRecord(), repoPath: kernel });
  commitAll(kernel);
  const revision = headOf(kernel);
  const parentAtRevision = fs.readFileSync(path.join(kernel, 'docs', 'note.md'), 'utf8');
  fs.appendFileSync(path.join(kernel, 'docs', 'note.md'), '\nA sentence added after the edition was taken.\n');
  commitAll(kernel);
  const edition = editionRecord({ revision, digest: sha256(parentAtRevision) });
  plantIntake(kernel, instance, { records: [...edition, ...completingRecord()], repoPath: kernel });
  check('t40 a parent that moved on is a stale edition, not a broken record', run(kernel, instance), {
    expectExit: 0,
    mustMatch: [/WARN.*the edition is behind its parent/],
    mustNotMatch: [/^FAIL/m],
  });
}

// t41 — the same edition against an unchanged parent says nothing at all. A
// warning that fired either way would carry no information.
{
  const { kernel, instance } = freshPair('t41');
  plantIntake(kernel, instance, { records: completingRecord(), repoPath: kernel });
  commitAll(kernel);
  const edition = editionRecord({
    revision: headOf(kernel),
    digest: sha256(fs.readFileSync(path.join(kernel, 'docs', 'note.md'), 'utf8')),
  });
  plantIntake(kernel, instance, { records: [...edition, ...completingRecord()], repoPath: kernel });
  check('t41 an edition level with its parent is silent', run(kernel, instance), {
    expectExit: 0,
    mustNotMatch: [/behind its parent/, /^FAIL/m],
  });
}

// t42 — a reference into a repository the inventory does not name resolves for
// nobody, here or anywhere else, and is a defect rather than an UNVERIFIED
{
  const { kernel, instance } = freshPair('t42');
  plantIntake(kernel, instance, { records: completingRecord(), repoPath: kernel });
  commitAll(kernel);
  const edition = editionRecord({
    repository: 'nowhere',
    revision: headOf(kernel),
    digest: sha256('anything'),
  });
  plantIntake(kernel, instance, { records: [...edition, ...completingRecord()], repoPath: kernel });
  check('t42 a parent in an unnamed repository detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/derives from repository "nowhere", which the Instance inventory does not name/],
  });
}

// t43 — a revision that holds no such file: the reference points at nothing
{
  const { kernel, instance } = freshPair('t43');
  plantIntake(kernel, instance, { records: completingRecord(), repoPath: kernel });
  commitAll(kernel);
  const edition = editionRecord({
    parentPath: 'docs/never-existed.md',
    revision: headOf(kernel),
    digest: sha256('anything'),
  });
  plantIntake(kernel, instance, { records: [...edition, ...completingRecord()], repoPath: kernel });
  check('t43 a parent path absent at the named revision detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/that revision holds no such file; the reference points at nothing/],
  });
}

// t44 — a bare path is refused by the schema before any of the above runs
{
  const { kernel, instance } = freshPair('t44');
  const edition = intakeRecord('README.md', sha256('x'), {
    verdict: 'adopt-edition',
    extra: [
      '    derived_from: docs/note.md',
      `    derived_from_digest: ${sha256('anything')}`,
      '    narrowing:',
      '      - synthetic narrowing',
    ],
  });
  plantIntake(kernel, instance, { records: [...edition, ...completingRecord()], repoPath: kernel });
  commitAll(kernel);
  check('t44 a bare-path parent reference refused by the schema', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/\/derived_from: expected object, got string/],
  });
}

// --- §7 applied to the Kernel's own documents --------------------------------
const NORM_FM = ['topic: naming', 'profile: universal', 'delivery: kernel-doc', 'activation: always'];

// t45 — a document that declares one of the four answers and not the rest.
// Half a declaration is the drift the standard was written against.
{
  const { kernel, instance } = freshPair('t45');
  write(kernel, 'docs/half.md', `${fm('Half a norm', 'standard', ['delivery: kernel-doc'])}\n# Half a norm\n`);
  commitAll(kernel);
  check('t45 a half-declared norm detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/docs\/half\.md declares delivery but not topic, profile, activation/],
  });
}

// t46 — an activation outside the pool of §5
{
  const { kernel, instance } = freshPair('t46');
  write(kernel, 'docs/odd.md', `${fm('Odd activation', 'standard', [...NORM_FM.slice(0, 3), 'activation: whenever'])}\n# Odd\n`);
  commitAll(kernel);
  check('t46 activation outside the pool detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/declares activation "whenever", which is not in the pool/],
  });
}

// t47 — a topic that the registry does not contain. The Kernel's own documents
// are held to the same rule as the records in a register.
{
  const { kernel, instance } = freshPair('t47');
  write(kernel, 'docs/stray.md', `${fm('Stray topic', 'standard', ['topic: invented-topic', ...NORM_FM.slice(1)])}\n# Stray\n`);
  commitAll(kernel);
  check('t47 a Kernel document naming a topic outside the pool detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/docs\/stray\.md names topic "invented-topic", which is not in the pool/],
  });
}

// t48 — the whole declaration, correct: green, and counted. Without this the
// three cases above would be satisfied by a check that refuses everything.
{
  const { kernel, instance } = freshPair('t48');
  write(kernel, 'docs/norm.md', `${fm('A declared norm', 'standard', NORM_FM)}\n# A declared norm\n`);
  commitAll(kernel);
  check('t48 a fully declared norm passes and is counted', run(kernel, instance), {
    expectExit: 0,
    mustMatch: [/agent-instruction-identity: 1 Kernel document\(s\) declare themselves agent instruction norms/],
  });
}

// --- containers: the unit of intake is the region ---------------------------
// A container holds several subjects and part of it is written by a generator.
// These cases exercise the unit the protocol declares (§3.1), not the file.

function regionMarkers({ id, owner = 'workspace-owner', generated = false, body = 'A rule for this subject.' }) {
  const attrs = [`id=${id}`, owner ? `owner=${owner}` : '', generated ? 'generated=yes' : '']
    .filter(Boolean).join(' ');
  return `<!-- meridian:begin instruction-section ${attrs} -->\n${body}\n<!-- meridian:end instruction-section id=${id} -->\n`;
}
function writeContainer(root, regions, { tail = '' } = {}) {
  write(root, 'AGENTS.md',
    `${fm('Container', 'reference')}\n# Container\n\n${regions.map(regionMarkers).join('\n')}${tail}`);
}
function regionRecord(id, opts = {}) {
  return intakeRecord('AGENTS.md', sha256(id), {
    delivery: 'agents-md-section',
    extra: [`    region: ${id}`, ...(opts.extra ?? [])],
  });
}

// t49 — a declared region with no record. The file is in the register; the
// subject inside it is not, and a file-level check would have called that
// complete.
{
  const { kernel, instance } = freshPair('t49');
  writeContainer(kernel, [{ id: 'naming-rules' }, { id: 'review-rules' }]);
  plantIntake(kernel, instance, {
    records: [...completingRecord(), ...regionRecord('naming-rules')],
    repoPath: kernel,
  });
  commitAll(kernel);
  check('t49 an uncovered region of a container detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/norm\(s\) in the tree have no record \(AGENTS\.md#review-rules\)/],
  });
}

// t50 — every owner region recorded; the generated one is not, and must not be
{
  const { kernel, instance } = freshPair('t50');
  writeContainer(kernel, [
    { id: 'naming-rules' },
    { id: 'generated-index', generated: true, owner: 'toolchain' },
  ]);
  plantIntake(kernel, instance, {
    records: [...completingRecord(), ...regionRecord('naming-rules')],
    repoPath: kernel,
  });
  commitAll(kernel);
  check('t50 regions covered and a generated region left out passes', run(kernel, instance), {
    expectExit: 0,
    mustMatch: [/1 declared region\(s\) of container files carry a record/],
    mustNotMatch: [/^FAIL/m, /lie outside every declared region/],
  });
}

// t51 — a generated region taken into the register. It is not the owner's
// norm; recording it claims authorship of text a generator overwrites.
{
  const { kernel, instance } = freshPair('t51');
  writeContainer(kernel, [{ id: 'generated-index', generated: true, owner: 'toolchain' }]);
  plantIntake(kernel, instance, {
    records: [...completingRecord(), ...regionRecord('generated-index')],
    repoPath: kernel,
  });
  commitAll(kernel);
  check('t51 a generated region carrying a record detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/region "generated-index" is declared generated and yet carries an intake record/],
  });
}

// t52 — a region with no declared owner: nothing answers whether the next
// generation may overwrite it
{
  const { kernel, instance } = freshPair('t52');
  writeContainer(kernel, [{ id: 'naming-rules', owner: '' }]);
  plantIntake(kernel, instance, {
    records: [...completingRecord(), ...regionRecord('naming-rules')],
    repoPath: kernel,
  });
  commitAll(kernel);
  check('t52 a region without an owner detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/region "naming-rules" declares no owner/],
  });
}

// t53 — prose outside every region. Completeness is still claimed, but only
// over the regions, and the part it does not cover is named.
{
  const { kernel, instance } = freshPair('t53');
  writeContainer(kernel, [{ id: 'naming-rules' }], {
    tail: '\nA rule that sits in no region at all.\nAnd a second line of it.\n',
  });
  plantIntake(kernel, instance, {
    records: [...completingRecord(), ...regionRecord('naming-rules')],
    repoPath: kernel,
  });
  commitAll(kernel);
  check('t53 text outside every region is named, not counted as covered', run(kernel, instance), {
    expectExit: 0,
    mustMatch: [/AGENTS\.md — 2 line\(s\) lie outside every declared region/],
  });
}

// t54 — a marker opened and never closed. Everything after it would otherwise
// belong to that region by accident.
{
  const { kernel, instance } = freshPair('t54');
  write(kernel, 'AGENTS.md',
    `${fm('Container', 'reference')}\n# Container\n\n<!-- meridian:begin instruction-section id=naming-rules owner=workspace-owner -->\nA rule.\n`);
  plantIntake(kernel, instance, { records: completingRecord(), repoPath: kernel });
  commitAll(kernel);
  check('t54 an unclosed region marker detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/region "naming-rules" is opened and never closed/],
  });
}

// --- second round: the holes the first round of these checks still had ------

// t55 — a container that documents the region syntax. The example inside a
// fenced block is not a declaration, and reading it as one would open a region
// that no record can ever cover.
{
  const { kernel, instance } = freshPair('t55');
  write(kernel, 'AGENTS.md',
    `${fm('Container', 'reference')}\n# Container\n\n`
    + regionMarkers({ id: 'naming-rules' })
    + '\n## Как объявить участок\n\n```text\n'
    + '<!-- meridian:begin instruction-section id=example owner=someone -->\n'
    + '<!-- meridian:end instruction-section id=example -->\n'
    + '```\n');
  plantIntake(kernel, instance, {
    records: [...completingRecord(), ...regionRecord('naming-rules')],
    repoPath: kernel,
  });
  commitAll(kernel);
  check('t55 a region example in a code fence is not a declaration', run(kernel, instance), {
    expectExit: 0,
    mustNotMatch: [/#example/, /^FAIL/m],
  });
}

// t56 — a container with no declared regions, recorded with a verdict other
// than deferred. The protocol's answer for it is one deferred entry; any other
// verdict assigns one topic to a text that holds several.
{
  const { kernel, instance } = freshPair('t56');
  write(kernel, 'AGENTS.md', `${fm('Container', 'reference')}\n# Container\n\nThree unrelated rules, no markers.\n`);
  const asAdopted = intakeRecord('AGENTS.md', sha256('c'), { verdict: 'adopt-core' });
  plantIntake(kernel, instance, { records: [...completingRecord(), ...asAdopted], repoPath: kernel });
  commitAll(kernel);
  check('t56 an undeclared container adopted wholesale detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/declares no regions and its current record is "adopt-core"/],
  });
  const asDeferred = intakeRecord('AGENTS.md', sha256('c'), {
    verdict: 'deferred', extra: ['    resume_condition: after the container declares its region boundaries'],
  });
  plantIntake(kernel, instance, { records: [...completingRecord(), ...asDeferred], repoPath: kernel });
  check('t56 the same container recorded as deferred passes', run(kernel, instance), {
    expectExit: 0,
    mustMatch: [/declares no regions and is recorded as deferred/],
  });
}

// t57 — a record naming a region the file does not declare. On its own it
// looks harmless; it is what a typo produces, and it hides which section was
// actually meant.
{
  const { kernel, instance } = freshPair('t57');
  writeContainer(kernel, [{ id: 'naming-rules' }]);
  plantIntake(kernel, instance, {
    records: [...completingRecord(), ...regionRecord('naming-rules'), ...regionRecord('naming-rulez')],
    repoPath: kernel,
  });
  commitAll(kernel);
  check('t57 a record naming an undeclared region detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/a record names region "naming-rulez", which the file does not declare/],
  });
}

// t58 — a register that declares no $schema. The generic validation pass is
// opt-in, so the shape of every record in such a file is checked by nothing,
// while the completeness line still reports the tree as covered.
{
  const { kernel, instance } = freshPair('t58');
  plantIntake(kernel, instance, { records: completingRecord(), repoPath: kernel });
  const registerPath = path.join(instance, 'instruction-intake', 'synthetic.yaml');
  fs.writeFileSync(registerPath,
    fs.readFileSync(registerPath, 'utf8').replace('$schema: ./intake.schema.json\n', ''));
  commitAll(kernel);
  check('t58 a register without $schema detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/declares no \$schema, so the shape of every record in it was validated by nothing/],
  });
}

// t59 — a required Front Matter field present but empty. The first form of
// this check accepted it, because `\s` matched the newline and the next line's
// first character satisfied it.
{
  const { kernel, instance } = freshPair('t59');
  write(kernel, 'docs/blank.md',
    '---\ntitle:\ndocument_type: reference\nstatus: maintained\nscope: workspace\n'
    + 'owner: test-suite\ncreated: 2026-01-01\nupdated: 2026-01-01\n---\n\n# Blank\n');
  commitAll(kernel);
  check('t59 an empty required Front Matter field detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/docs\/blank\.md is missing required Front Matter field "title"/],
  });
}

// t60 — a Kernel norm that declares a parent. The qualified form must pass and
// the missing narrowing list must not: §8 claims this is checked always, and
// before this case it was checked nowhere for a Kernel document.
{
  const { kernel, instance } = freshPair('t60');
  const parent = ['derived_from:', '  repository: kernel', '  path: docs/note.md',
                  '  revision: 0123456789abcdef0123456789abcdef01234567'];
  write(kernel, 'docs/edition.md', `${fm('An edition', 'standard', [...NORM_FM, ...parent])}\n# An edition\n`);
  commitAll(kernel);
  check('t60 a parent with no narrowing list detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/docs\/edition\.md declares derived_from with no narrowing list/],
    mustNotMatch: [/records derived_from as a scalar/],
  });
  write(kernel, 'docs/edition.md',
    `${fm('An edition', 'standard', [...NORM_FM, ...parent, 'narrowing:', '  - the product-specific half is gone'])}\n# An edition\n`);
  commitAll(kernel);
  check('t60 the same edition with a narrowing list passes', run(kernel, instance), {
    expectExit: 0,
    mustNotMatch: [/^FAIL/m],
  });
}

// t61 — the register renamed in the same commit that drops a record. Without
// following the rename there is one revision to compare against, and it is the
// truncated one.
{
  const { kernel, instance } = freshPair('t61');
  const both = [...intakeRecord('skills/demo/SKILL.md', sha256('a')), ...intakeRecord('README.md', sha256('b'))];
  plantIntake(kernel, instance, { records: both, repoPath: kernel });
  commitAll(kernel);
  sh('git', ['init', '-q'], instance);
  commitAll(instance);
  sh('git', ['mv', 'instruction-intake/synthetic.yaml', 'instruction-intake/renamed.yaml'], instance);
  fs.writeFileSync(path.join(instance, 'instruction-intake', 'renamed.yaml'), [
    '$schema: ./intake.schema.json', 'schema_version: 1', 'repository: synthetic', 'records:',
    ...intakeRecord('skills/demo/SKILL.md', sha256('a')), '',
  ].join('\n'));
  commitAll(instance);
  check('t61 a record dropped under a rename detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/lost 1 record\(s\) that earlier revisions of this file carried/],
  });
}

// --- third round: what the fix for the code fences brought with it ----------

// t62 — a fence opened and never closed. Blanking to the end of the file would
// hide every norm after it from the count, quietly, on a Markdown typo.
{
  const { kernel, instance } = freshPair('t62');
  write(kernel, 'AGENTS.md',
    `${fm('Container', 'reference')}\n# Container\n\n`
    + regionMarkers({ id: 'naming-rules' })
    + '\n```sh\nnpm test\n\n## Ещё правила\n\nA rule that belongs to no region.\n');
  plantIntake(kernel, instance, {
    records: [...completingRecord(), ...regionRecord('naming-rules')],
    repoPath: kernel,
  });
  commitAll(kernel);
  check('t62 an unclosed code fence is a finding, not a hiding place', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/a code fence opens at line \d+ and is never closed/],
  });
}

// t63 — a shorter fence inside a longer one does not close it, so the example
// it contains is still an example
{
  const { kernel, instance } = freshPair('t63');
  write(kernel, 'AGENTS.md',
    `${fm('Container', 'reference')}\n# Container\n\n`
    + regionMarkers({ id: 'naming-rules' })
    + '\n````markdown\n```\n<!-- meridian:begin instruction-section id=example owner=someone -->\n```\n````\n');
  plantIntake(kernel, instance, {
    records: [...completingRecord(), ...regionRecord('naming-rules')],
    repoPath: kernel,
  });
  commitAll(kernel);
  check('t63 a shorter fence does not close a longer one', run(kernel, instance), {
    expectExit: 0,
    mustNotMatch: [/#example/, /never closed/, /^FAIL/m],
  });
}

// t64 — "current" is the latest record by date, not the last one in the file.
// Ordering by position let the rule be sidestepped by moving two blocks.
{
  const { kernel, instance } = freshPair('t64');
  write(kernel, 'AGENTS.md', `${fm('Container', 'reference')}\n# Container\n\nThree unrelated rules, no markers.\n`);
  const later = intakeRecord('AGENTS.md', sha256('c'), { verdict: 'adopt-core', recordedAt: '2026-06-01' });
  const earlier = intakeRecord('AGENTS.md', sha256('c'), {
    verdict: 'deferred', recordedAt: '2026-01-01',
    extra: ['    resume_condition: after the container declares its region boundaries'],
  });
  // the superseded deferred entry is placed last in the file
  plantIntake(kernel, instance, { records: [...completingRecord(), ...later, ...earlier], repoPath: kernel });
  commitAll(kernel);
  check('t64 the current record is the latest by date, not by position', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/declares no regions and its current record is "adopt-core"/],
  });
}

// t65 — a record naming a region in a container that declares none. The check
// used to sit behind the branch that returns early for such containers.
{
  const { kernel, instance } = freshPair('t65');
  write(kernel, 'AGENTS.md', `${fm('Container', 'reference')}\n# Container\n\nRules, no markers.\n`);
  const deferred = intakeRecord('AGENTS.md', sha256('c'), {
    verdict: 'deferred', extra: ['    resume_condition: after the container declares its region boundaries'],
  });
  plantIntake(kernel, instance, {
    records: [...completingRecord(), ...deferred, ...regionRecord('ghost-region')],
    repoPath: kernel,
  });
  commitAll(kernel);
  check('t65 a region record against a container declaring no regions detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/a record names region "ghost-region", which the file does not declare/],
  });
}

// t66 — the pool file showing its own marker syntax in a fenced example. One
// rule for quoting a marker, one implementation: inside a fence it is an
// example, outside it is a declaration.
{
  const { kernel, instance } = freshPair('t66');
  const p = path.join(kernel, 'standards', 'workspace', 'instruction-topics.md');
  fs.appendFileSync(p,
    '\n## Как это размечено\n\n```text\n<!-- meridian:begin topic-pool -->\n<!-- meridian:end topic-pool -->\n```\n');
  commitAll(kernel);
  check('t66 the pool file may show its own markers in a fenced example', run(kernel, instance), {
    expectExit: 0,
    mustMatch: [/instruction-topics: 1 topics; names and signatures agree/],
    mustNotMatch: [/found 2 and 2/, /^FAIL/m],
  });
}

// --- fourth round: register identity, not merely the current path -----------

// t67 — a typo in the register's repository id is a broken reference. It must
// not degrade to the same UNVERIFIED result as a known repository whose path is
// temporarily unreachable.
{
  const { kernel, instance } = freshPair('t67');
  plantIntake(kernel, instance, { records: completingRecord(), repoPath: kernel });
  const p = path.join(instance, 'instruction-intake', 'synthetic.yaml');
  fs.writeFileSync(p, fs.readFileSync(p, 'utf8').replace('repository: synthetic', 'repository: misspelled'));
  commitAll(kernel);
  check('t67 unknown register repository detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/names repository "misspelled", which the Instance inventory does not name/, /unresolvable reference/],
  });
}

// t68 — deleting the complete register used to delete the loop that checked
// append-only and was reported as if intake had simply never started.
{
  const { kernel, instance } = freshPair('t68');
  plantIntake(kernel, instance, { records: completingRecord(), repoPath: kernel });
  commitAll(kernel);
  sh('git', ['init', '-q'], instance);
  commitAll(instance);
  fs.rmSync(path.join(instance, 'instruction-intake', 'synthetic.yaml'));
  check('t68 complete register deletion detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/complete register for repository "synthetic" disappeared/, /append-only protects the register itself/],
  });
}

// t69 — published preservation follows the stable repository identity. A
// local rename plus an amend must not make the old upstream path look like a
// register that never existed.
{
  const { kernel, instance } = freshPair('t69');
  const both = [...intakeRecord('skills/demo/SKILL.md', sha256('a')), ...intakeRecord('README.md', sha256('b'))];
  plantIntake(kernel, instance, { records: both, repoPath: kernel });
  commitAll(kernel);
  sh('git', ['init', '-q'], instance);
  commitAll(instance);
  const remote = path.join(workRoot, 't69', 'remote.git');
  sh('git', ['init', '-q', '--bare', remote], workRoot);
  sh('git', ['remote', 'add', 'origin', remote], instance);
  const branch = execFileSync('git', ['rev-parse', '--abbrev-ref', 'HEAD'], { cwd: instance, encoding: 'utf8' }).trim();
  sh('git', ['push', '-q', '-u', 'origin', branch], instance);
  sh('git', ['mv', 'instruction-intake/synthetic.yaml', 'instruction-intake/renamed.yaml'], instance);
  fs.writeFileSync(path.join(instance, 'instruction-intake', 'renamed.yaml'), [
    '$schema: ./intake.schema.json', 'schema_version: 1', 'repository: synthetic', 'records:',
    ...intakeRecord('skills/demo/SKILL.md', sha256('a')), '',
  ].join('\n'));
  sh('git', ['add', '-A'], instance);
  sh('git', ['-c', 'user.name=t', '-c', 'user.email=t@t.invalid', 'commit', '-q', '--amend', '-m', 'renamed and rewritten'], instance);
  check('t69 published register found after local rename', run(kernel, instance, { MERIDIAN_CHECK_PUBLISHED: '1' }), {
    expectExit: 1,
    mustMatch: [/lost 1 record\(s\) that are published on origin\//, /rewriting local history does not unpublish a decision/],
  });
}

// t70 — the stable identity only works when it is unique in the current tree.
// Two files for one repository are not two independent histories.
{
  const { kernel, instance } = freshPair('t70');
  plantIntake(kernel, instance, { records: completingRecord(), repoPath: kernel });
  fs.copyFileSync(
    path.join(instance, 'instruction-intake', 'synthetic.yaml'),
    path.join(instance, 'instruction-intake', 'duplicate.yaml'),
  );
  commitAll(kernel);
  check('t70 duplicate current register identity detected', run(kernel, instance), {
    expectExit: 1,
    mustMatch: [/repository "synthetic" has more than one current register/, /no unambiguous history/],
  });
}

fs.rmSync(workRoot, { recursive: true, force: true });

console.log('---');
console.log(`${passed} passed, ${failed} failed, ${skipped} skipped`);
if (failures.length) {
  console.log('\n' + failures.join('\n\n'));
}
process.exit(failed > 0 ? 1 : 0);
