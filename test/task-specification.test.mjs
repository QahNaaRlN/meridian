#!/usr/bin/env node
// Standalone verification for the task specification contract
// (registries/operating-model/task-specification.schema.json).
//
// It calls the same implementation the gate calls
// (scripts/lib/task-specification.mjs) so the two cannot drift: the canonical
// scoped-record envelope (validated separately and always), the complete
// specialised schema a record declares in its own $schema, and the rules JSON
// Schema cannot state — the portable $schema declaration, task-pattern
// resolution against the built-in catalogue, work_kind / change_class
// agreement, the two allowed scopes, non-empty and verifiable acceptance
// criteria, non-empty constraints, run-state rejection, portability and the
// Russian name.
//
// Usage: node test/task-specification.test.mjs

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { assertSupportedDeep, UnsupportedSchema, validate } from '../scripts/lib/json-schema.mjs';
import { yamlParse } from '../scripts/lib/yaml.mjs';
import {
  evaluateTaskSpecification,
  nonPortableReason,
  resolveSchemaRef,
  ALLOWED_SCOPE_TYPES,
  CANONICAL_RECORD_BASE,
  EXPECTED_SCHEMA_BASENAME,
  EXPECTED_SCHEMA_REF,
  ENVELOPE_SCHEMA_REF,
  RECORD_TYPE,
  RUN_STATE_FIELDS,
} from '../scripts/lib/task-specification.mjs';

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

const recordSchema = loadJson('registries/operating-model/task-specification.schema.json');
const envelopeSchema = loadJson('registries/operating-model/scoped-record.schema.json');
const fixtures = loadJson('registries/operating-model/fixtures/task-specification.fixtures.json');

// The built-in task-pattern catalogue, read the same way the gate reads it.
const catalogueDoc = yamlParse(loadText('standards/workspace/task-pattern-registry.yaml'));
const taskPatterns = (Array.isArray(catalogueDoc.task_patterns) ? catalogueDoc.task_patterns : [])
  .map((p) => ({
    id: p && p.id,
    work_kind: p && p.payload && p.payload.work_kind,
    change_class: (p && p.payload && p.payload.change_class) ?? null,
  }));

const opts = { recordSchema, envelopeSchema, taskPatterns };
const evaluate = (doc) => evaluateTaskSpecification(doc, opts);

// A topologically complete valid specification, the base for targeted mutations.
const BASE = clone(fixtures.valid[0].spec);
const payload = (d) => d.payload;
const mutate = (fn) => { const d = clone(BASE); fn(d); return d; };

// A VERIFICATION ADAPTER, not a universal physical record path: it maps the
// resolved LOGICAL address of the declared $schema onto the built-in schema
// file shipped with this Kernel checkout, purely so this test can inspect the
// schema's contents. A production storage adapter is free to map the same
// logical address onto working data in a local store, a remote service or the
// transitional Instance. Returns an absolute path, or null when the reference
// does not resolve to the specialised schema's logical address.
const verificationAdapterSchemaFile = (ref) => {
  const rel = resolveSchemaRef(ref);
  return rel === EXPECTED_SCHEMA_REF ? path.join(root, rel) : null;
};

// ---------------------------------------------------------------------------
// schema shape
// ---------------------------------------------------------------------------
check('both schemas use the supported JSON Schema subset', () => {
  assertSupportedDeep(recordSchema, 'task-specification.schema.json');
  assertSupportedDeep(envelopeSchema, 'scoped-record.schema.json');
});

check('an unsupported schema keyword is rejected up front', () => {
  const bad = clone(recordSchema);
  bad.definitions.payload.patternProperties = { '^x': { type: 'string' } };
  let threw = null;
  try { assertSupportedDeep(bad, 'task-specification.schema.json'); }
  catch (e) { threw = e; }
  assert(threw instanceof UnsupportedSchema, 'assertSupportedDeep did not reject the unsupported keyword');
});

check('the specialised schema COMPOSES the envelope with the body (one pass, one model)', () => {
  assert(recordSchema.properties.record_type.const === RECORD_TYPE, 'record_type is not pinned to task-specification');
  assert(recordSchema.additionalProperties === false, 'the record schema is not a closed object');
  for (const k of ['$schema', 'schema_version', 'id', 'title', 'record_type', 'scope', 'origin', 'authority', 'payload']) {
    assert(recordSchema.required.includes(k), `the record schema does not require the envelope field "${k}"`);
  }
  assert(recordSchema.definitions.payload.additionalProperties === false, 'payload is not a closed object');
  const req = recordSchema.definitions.payload.required;
  for (const k of ['goal', 'initial_state', 'target_model', 'task_pattern', 'constraints', 'acceptance_criteria']) {
    assert(req.includes(k), `payload does not require "${k}"`);
  }
  // scope is narrowed to exactly the two allowed areas.
  assert(JSON.stringify(recordSchema.definitions.scope_ref.properties.type.enum.slice().sort())
    === JSON.stringify(ALLOWED_SCOPE_TYPES.slice().sort()),
    'the schema scope enum is not exactly {project-workspace, repository-scope}');
});

// ---------------------------------------------------------------------------
// 1. the declared $schema reaches the specialised contract
// ---------------------------------------------------------------------------
check('every VALID fixture declares $schema = the specialised schema, by a portable relative reference that RESOLVES', () => {
  fixtures.valid.forEach((c, i) => {
    const ref = c.spec && c.spec.$schema;
    assert(typeof ref === 'string' && ref !== '', `valid[${i}] (${c.note}) has no $schema`);
    assert(!path.isAbsolute(ref) && ref.startsWith('../'), `valid[${i}] (${c.note}) $schema "${ref}" is not a portable relative reference`);
    assert(resolveSchemaRef(ref) === EXPECTED_SCHEMA_REF,
      `valid[${i}] (${c.note}) $schema "${ref}" does not resolve to the logical address ${EXPECTED_SCHEMA_REF} from the canonical logical base`);
  });
});

check('the $schema reference resolves as a filesystem-independent logical address', () => {
  assert(CANONICAL_RECORD_BASE === 'records/task-specification', `the canonical logical base changed unexpectedly: ${CANONICAL_RECORD_BASE}`);

  // (a) resolution yields a LOGICAL address: a namespace-relative string, never
  //     an absolute path and never tied to this checkout's location on disk.
  const addr = resolveSchemaRef('../../registries/operating-model/task-specification.schema.json');
  assert(addr === EXPECTED_SCHEMA_REF, `the canonical reference no longer resolves to the specialised schema's logical address: ${addr}`);
  assert(!path.isAbsolute(addr) && !addr.includes(root), 'the resolved value is not a pure logical address');
  assert(resolveSchemaRef('../../registries/operating-model/scoped-record.schema.json') === ENVELOPE_SCHEMA_REF,
    'the envelope reference no longer resolves to scoped-record');

  // (b) resolution is pure address math, not a filesystem lookup: an arbitrary
  //     in-namespace reference resolves to a logical address whether or not any
  //     file backs it.
  assert(resolveSchemaRef('../../made-up/segment/thing.json') === 'made-up/segment/thing.json',
    'resolution of an arbitrary in-namespace reference did not produce a logical address');

  // (c) resolution does NOT depend on the process working directory.
  const cwd0 = process.cwd();
  try {
    process.chdir(os.tmpdir());
    const fromTmp = resolveSchemaRef('../../registries/operating-model/task-specification.schema.json');
    process.chdir(path.parse(root).root);
    const fromFsRoot = resolveSchemaRef('../../registries/operating-model/task-specification.schema.json');
    assert(fromTmp === addr && fromFsRoot === addr,
      `resolveSchemaRef changed with process.cwd(): ${fromTmp} / ${fromFsRoot} vs ${addr}`);
  } finally {
    process.chdir(cwd0);
  }

  // (d) a SEPARATE verification adapter maps the logical address onto the
  //     Kernel schema file; the logical layer itself never touched the disk.
  const mapped = verificationAdapterSchemaFile('../../registries/operating-model/task-specification.schema.json');
  assert(mapped && path.isAbsolute(mapped) && fs.existsSync(mapped),
    'the verification adapter did not map the logical address onto a real Kernel schema file');
  assert(String(JSON.parse(fs.readFileSync(mapped, 'utf8')).$id || '').endsWith(EXPECTED_SCHEMA_BASENAME),
    'the mapped file is not the specialised schema');

  // A future storage adapter MAY back this logical address with a matching
  // physical path; the contract does not forbid it, so nothing here asserts the
  // absence (or presence) of any such directory.
});

check('REGRESSION: a valid spec with payload.goal removed is RED against the schema named in its own $schema', () => {
  const spec = clone(fixtures.valid[0].spec);
  const schemaFile = verificationAdapterSchemaFile(spec.$schema);
  assert(schemaFile && fs.existsSync(schemaFile), `the declared $schema "${spec.$schema}" does not resolve to the specialised schema's logical address`);
  const declaredSchema = JSON.parse(fs.readFileSync(schemaFile, 'utf8'));
  assert(String(declaredSchema.$id || '').endsWith(EXPECTED_SCHEMA_BASENAME),
    'the declared $schema is not the specialised task-specification schema');
  delete spec.payload.goal;
  const errs = [];
  validate(spec, declaredSchema, declaredSchema, '', errs);
  assert(errs.length > 0, 'a spec missing payload.goal validated clean against the schema named in its $schema');
  assert(errs.some((m) => /goal/.test(m)), `the failure did not point at goal: ${errs[0]}`);
});

check('an absent $schema is rejected', () => {
  const d = mutate((x) => { delete x.$schema; });
  assert(evaluate(d).some((m) => /declares no \$schema/.test(m) || /\$schema.*required/.test(m)),
    'a record with no $schema was accepted');
});

check('a $schema that resolves to a different schema (right directory, wrong file) is rejected', () => {
  const d = mutate((x) => { x.$schema = '../../registries/operating-model/some-other.schema.json'; });
  assert(evaluate(d).some((m) => /does not resolve to the logical address registries\/operating-model\/task-specification\.schema\.json/.test(m)),
    'a $schema resolving to a different file was accepted');
});

check('a $schema that resolves to the record envelope is rejected', () => {
  const d = mutate((x) => { x.$schema = '../../registries/operating-model/scoped-record.schema.json'; });
  assert(evaluate(d).some((m) => /resolves to the record envelope/.test(m)),
    'a $schema pointing only at scoped-record was accepted');
});

check('a $schema given as an absolute machine path is rejected', () => {
  const d = mutate((x) => { x.$schema = '/opt/meridian/registries/operating-model/task-specification.schema.json'; });
  assert(evaluate(d).some((m) => /\$schema .* is not portable/.test(m)), 'an absolute $schema path was accepted');
});

// 1. the declared reference must genuinely RESOLVE, as a logical address, to
//    the specialised schema — not merely carry the expected basename.
check('a $schema into a namespace segment that is not there does not resolve and is rejected', () => {
  assert(resolveSchemaRef('missing/task-specification.schema.json') === 'records/task-specification/missing/task-specification.schema.json',
    'the missing-segment reference did not resolve under the canonical logical base');
  const d = mutate((x) => { x.$schema = 'missing/task-specification.schema.json'; });
  assert(evaluate(d).some((m) => /does not resolve to the logical address registries\/operating-model\/task-specification\.schema\.json/.test(m)),
    'a $schema into a namespace segment that is not there was accepted');
});

check('a $schema given as a bare basename (no relative path) does not resolve and is rejected', () => {
  assert(resolveSchemaRef('task-specification.schema.json') === 'records/task-specification/task-specification.schema.json',
    'the bare basename did not resolve under the canonical logical base');
  const d = mutate((x) => { x.$schema = 'task-specification.schema.json'; });
  assert(evaluate(d).some((m) => /does not resolve to the logical address registries\/operating-model\/task-specification\.schema\.json/.test(m)),
    'a bare-basename $schema was accepted');
});

check('a $schema with the right basename under a different namespace segment is rejected', () => {
  assert(resolveSchemaRef('../../registries/other-model/task-specification.schema.json') === 'registries/other-model/task-specification.schema.json',
    'the wrong-segment reference did not resolve as expected');
  const d = mutate((x) => { x.$schema = '../../registries/other-model/task-specification.schema.json'; });
  assert(evaluate(d).some((m) => /does not resolve to the logical address registries\/operating-model\/task-specification\.schema\.json/.test(m)),
    'a right-basename / wrong-segment $schema was accepted');
});

check('a $schema that climbs out of the Meridian namespace root does not resolve and is rejected', () => {
  assert(resolveSchemaRef('../../../registries/operating-model/task-specification.schema.json') === null,
    'a reference climbing past the namespace root still resolved');
  const d = mutate((x) => { x.$schema = '../../../registries/operating-model/task-specification.schema.json'; });
  assert(evaluate(d).some((m) => /does not resolve to/.test(m)), 'an out-of-namespace $schema was accepted');
});

check('the canonical scoped-record envelope is still validated separately (not dropped)', () => {
  const d = mutate((x) => { delete x.authority; });
  assert(evaluate(d).some((m) => /^envelope /.test(m)), 'a missing authority block was not reported through the canonical envelope schema');
});

// ---------------------------------------------------------------------------
// real fixtures + real catalogue
// ---------------------------------------------------------------------------
check('the catalogue carries the seven built-in patterns', () => {
  assert(taskPatterns.length === 7, `expected 7 task patterns, read ${taskPatterns.length}`);
  for (const id of ['assess-existing-state', 'operate-environment-action', 'decompose-initiative',
    'fix-defect', 'add-capability', 'change-behavior', 'refactor-preserving-behavior']) {
    assert(taskPatterns.some((p) => p.id === id), `catalogue is missing pattern "${id}"`);
  }
});

check('every valid fixture passes with no problem', () => {
  fixtures.valid.forEach((c, i) => {
    const p = evaluate(c.spec);
    assert(p.length === 0, `valid[${i}] (${c.note}) was rejected: ${p[0]}`);
  });
});

check('every valid fixture is scoped to project-workspace or repository-scope only', () => {
  fixtures.valid.forEach((c, i) => {
    assert(ALLOWED_SCOPE_TYPES.includes(c.spec.scope.type), `valid[${i}] (${c.note}) uses scope "${c.spec.scope.type}"`);
  });
});

check('a correct specification exists for every active task pattern', () => {
  const covered = new Set(fixtures.valid.map((c) => c.spec.payload.task_pattern.id));
  for (const p of taskPatterns) {
    assert(covered.has(p.id), `no valid fixture selects task pattern "${p.id}"`);
  }
});

check('every invalid fixture produces at least one problem', () => {
  fixtures.invalid.forEach((c, i) => {
    const p = evaluate(c.spec);
    assert(p.length > 0, `invalid[${i}] (${c.note}) was accepted`);
  });
});

// ---------------------------------------------------------------------------
// 2. allowed scopes — one negative per forbidden area
// ---------------------------------------------------------------------------
for (const [scopeType, extra] of [
  ['built-in-methodology', { id: 'built-in-methodology' }],
  ['user-profile', { id: 'example-user' }],
  ['organization-profile', { id: 'example-organization' }],
  ['run-state', { id: 'example-run', workspace_id: 'example-workspace' }],
]) {
  check(`scope "${scopeType}" is rejected for a task specification`, () => {
    const d = mutate((x) => { x.scope = { type: scopeType, ...extra }; });
    const p = evaluate(d);
    assert(p.some((m) => new RegExp(`scoped to "${scopeType}"`).test(m) || /built-in-methodology/.test(m) || /^envelope /.test(m)),
      `scope "${scopeType}" was accepted: ${JSON.stringify(p)}`);
  });
}

check('the two allowed scopes pass', () => {
  const ws = mutate((x) => { x.scope = { type: 'project-workspace', id: 'example-workspace' }; });
  assert(evaluate(ws).length === 0, `project-workspace scope was rejected: ${evaluate(ws)[0]}`);
  const rs = mutate((x) => { x.scope = { type: 'repository-scope', id: 'example-repository', workspace_id: 'example-workspace' }; });
  assert(evaluate(rs).length === 0, `repository-scope scope was rejected: ${evaluate(rs)[0]}`);
});

// ---------------------------------------------------------------------------
// 3. any rooted POSIX path is non-portable, regardless of the first directory
// ---------------------------------------------------------------------------
check('any rooted POSIX path is non-portable, whatever the first directory is named', () => {
  for (const p of ['/data/private/build', '/workspace/project', '/nix/store/example', '/custom/root/file',
    '/home/example/thing', '/etc/hosts', '/srv/app/data']) {
    assert(nonPortableReason(`см. ${p} здесь`) !== null, `a rooted path was treated as portable: ${p}`);
    assert(nonPortableReason(p) !== null, `a rooted path at string start was treated as portable: ${p}`);
  }
});

check('a rooted path in a spec field breaks portability (each of the four required roots)', () => {
  for (const p of ['/data/private/build', '/workspace/project', '/nix/store/example', '/custom/root/file']) {
    const d = mutate((x) => { x.payload.initial_state = `Артефакт лежит в ${p} и не воспроизводится.`; });
    assert(evaluate(d).some((m) => /rooted \(absolute\) POSIX path/.test(m)), `a rooted path "${p}" was accepted in a field`);
  }
});

check('an ordinary relative path and a normal web URL are NOT flagged', () => {
  assert(nonPortableReason('см. src/config/parser') === null, 'a repo-relative POSIX path was flagged');
  assert(nonPortableReason('см. https://example.invalid/api за подробностями') === null, 'a normal https URL was flagged');
  assert(nonPortableReason('очистка n/a, соотношение 24/7') === null, 'ordinary prose with slashes was flagged');
  const d = mutate((x) => {
    x.payload.initial_state = 'Конфиг в src/config/parser; документация на https://example.invalid/api.';
  });
  assert(evaluate(d).length === 0, `a portable spec with a relative path and a URL was rejected: ${evaluate(d)[0]}`);
});

check('the retained non-portable forms still fire', () => {
  assert(nonPortableReason('см. ~/build') !== null, 'a "~/" reference was treated as portable');
  assert(nonPortableReason('см. C:\\Users\\ci\\build') !== null, 'a drive-letter path was treated as portable');
  assert(nonPortableReason('см. C:/Users/ci/build') !== null, 'a forward-slash drive-letter path was treated as portable');
  assert(nonPortableReason('см. src\\config\\parser') !== null, 'a backslash-separated path was treated as portable');
  assert(nonPortableReason('см. file:///opt/thing') !== null, 'a file:// URL was treated as portable');
});

// 2. a rooted POSIX path is caught after ANY ordinary text separator, not only
//    after whitespace or an opening delimiter.
check('a rooted POSIX path after an ordinary text separator (= , :) is non-portable', () => {
  for (const s of ['path=/etc/hosts', 'artifact,/data/private', 'source:/custom/root/file']) {
    const r = nonPortableReason(s);
    assert(r !== null, `not flagged: ${s}`);
    assert(/rooted \(absolute\) POSIX path/.test(r), `wrong reason for "${s}": ${r}`);
  }
});

check('a word ending in ":/" is a rooted POSIX path, not a Windows drive-letter path', () => {
  const r = nonPortableReason('path:/etc/hosts');
  assert(r !== null, 'path:/etc/hosts was treated as portable');
  assert(!/drive-letter/.test(r), `path:/etc/hosts was misclassified as a drive-letter path: ${r}`);
  assert(/rooted \(absolute\) POSIX path/.test(r), `expected a rooted POSIX reason for path:/etc/hosts, got: ${r}`);
  // a genuine lone drive letter is still a drive-letter path
  assert(/drive-letter/.test(nonPortableReason('см. D:/data/x')), 'a lone drive letter stopped being detected');
});

check('a rooted POSIX path after "=" or ":" inside a spec field breaks portability', () => {
  for (const bad of ['Скрипт читает конфиг по path=/etc/hosts при запуске.',
    'Значение source:/custom/root/file зашито в шаг постановки.']) {
    const d = mutate((x) => { x.payload.initial_state = bad; });
    assert(evaluate(d).some((m) => /rooted \(absolute\) POSIX path/.test(m)), `not caught in a field: ${bad}`);
  }
});

// 2b. a rooted POSIX path is caught whatever the alphabet of the first segment
//     — ASCII, Cyrillic or CJK — not only when it starts with an ASCII letter.
check('a rooted POSIX path is non-portable whatever the alphabet of the first segment', () => {
  for (const p of ['/Проект/файл', '/данные/сборка', '/数据/文件']) {
    assert(nonPortableReason(p) !== null, `a non-ASCII rooted path at string start was treated as portable: ${p}`);
    assert(/rooted \(absolute\) POSIX path/.test(nonPortableReason(p)), `wrong reason for ${p}: ${nonPortableReason(p)}`);
    assert(nonPortableReason(`см. ${p} здесь`) !== null, `a non-ASCII rooted path in prose was treated as portable: ${p}`);
  }
  // the ordinary relative path with a non-ASCII segment is still NOT flagged
  assert(nonPortableReason('см. документы/описание в каталоге') === null, 'a relative path with a Cyrillic segment was flagged');
});

check('a non-ASCII rooted path in a mandatory field and in an envelope reference breaks portability', () => {
  const inField = mutate((x) => { x.payload.initial_state = 'Артефакт лежит в /Проект/сборка и не воспроизводится.'; });
  assert(evaluate(inField).some((m) => /initial state contains a rooted \(absolute\) POSIX path/.test(m)),
    'a non-ASCII rooted path in initial_state was accepted');
  const inConstraint = mutate((x) => { x.payload.constraints = ['Путь /数据/文件 не зашивается в шаг.']; });
  assert(evaluate(inConstraint).some((m) => /constraint #0 contains a rooted \(absolute\) POSIX path/.test(m)),
    'a CJK rooted path in a constraint was accepted');
  const inEnvelopeRef = mutate((x) => { x.origin.source_ref = '/данные/решения/пример'; });
  assert(evaluate(inEnvelopeRef).some((m) => /origin\.source_ref contains a rooted \(absolute\) POSIX path/.test(m)),
    'a non-ASCII rooted path in origin.source_ref was accepted');
});

// 2c. the first segment of a rooted path is matched with NO allow-list: a "/"
//     at string start is rooted whatever the segment contains.
check('a rooted path whose first segment starts with a symbol or emoji is non-portable', () => {
  for (const p of ['/@scope/file', '/$private/file', '/~service/file', '/💾']) {
    const r = nonPortableReason(p);
    assert(r !== null, `a rooted path was treated as portable: ${p}`);
    assert(/rooted \(absolute\) POSIX path/.test(r), `wrong reason for ${p}: ${r}`);
    assert(nonPortableReason(`см. ${p} тут`) !== null, `a rooted path after whitespace was treated as portable: ${p}`);
  }
  // "/~service" is a rooted path, NOT a "~/" home reference (no "/" after "~")
  assert(/rooted \(absolute\) POSIX path/.test(nonPortableReason('/~service/file')),
    '"/~service/file" was not classified as a rooted path');
});

check('a relative path with a scoped or "$"-prefixed segment is NOT flagged', () => {
  for (const p of ['packages/@scope/module', 'relative/$private/file', 'src/config/parser', 'документы/описание']) {
    assert(nonPortableReason(`см. ${p} в дереве`) === null, `an ordinary relative path was flagged: ${p}`);
  }
  const d = mutate((x) => { x.payload.initial_state = 'Модуль лежит в packages/@scope/module рядом с relative/$private/file.'; });
  assert(evaluate(d).length === 0, `a portable spec with scoped relative paths was rejected: ${evaluate(d)[0]}`);
});

check('a rooted path with a "@"- or emoji-led first segment breaks portability inside a spec field', () => {
  const inGoal = mutate((x) => { x.payload.goal = 'Конфиг читается из /@config/base на запуске.'; });
  assert(evaluate(inGoal).some((m) => /goal contains a rooted \(absolute\) POSIX path/.test(m)),
    'a "/@config/base" rooted path in goal was accepted');
  const inCriterion = mutate((x) => {
    x.payload.acceptance_criteria = [{ id: 'c1', statement: 'Файл читается из /💾/cache.', verification: { method: 'Проверка.', expected_result: 'Результат.' } }];
  });
  assert(evaluate(inCriterion).some((m) => /acceptance criterion c1 contains a rooted \(absolute\) POSIX path/.test(m)),
    'an emoji-led rooted path in a criterion was accepted');
});

// ---------------------------------------------------------------------------
// 3. the envelope reference strings are portable too
// ---------------------------------------------------------------------------
check('a rooted machine path or file:// in origin.source_ref breaks portability', () => {
  const d = mutate((x) => { x.origin.source_ref = 'file:///owner/decisions/example'; });
  assert(evaluate(d).some((m) => /origin\.source_ref contains a file:\/\/ URL/.test(m)),
    'a file:// URL in origin.source_ref was accepted');
});

check('a rooted machine path in authority.authority_ref breaks portability', () => {
  const d = mutate((x) => { x.authority.authority_ref = '/home/ci/owners/example-owner'; });
  assert(evaluate(d).some((m) => /authority\.authority_ref contains a rooted \(absolute\) POSIX path/.test(m)),
    'a rooted path in authority.authority_ref was accepted');
});

check('a file:// URL in the optional authority.decision_ref breaks portability', () => {
  const d = mutate((x) => { x.authority.decision_ref = 'file://decisions/example'; });
  assert(evaluate(d).some((m) => /authority\.decision_ref contains a file:\/\/ URL/.test(m)),
    'a file:// URL in authority.decision_ref was accepted');
});

check('portable envelope references (opaque identifiers) are accepted', () => {
  const d = mutate((x) => {
    x.origin.source_ref = 'owner-decision:example-portable';
    x.authority.authority_ref = 'example-workspace-owner';
    x.authority.decision_ref = 'owner-decision:2026-09-09:example';
  });
  assert(evaluate(d).length === 0, `a portable envelope was rejected: ${evaluate(d)[0]}`);
});

// ---------------------------------------------------------------------------
// task-pattern resolution against the existing registry
// ---------------------------------------------------------------------------
check('an unknown task pattern id fails closed', () => {
  const d = mutate((x) => { payload(x).task_pattern = { id: 'no-such-pattern' }; });
  assert(evaluate(d).some((m) => /not in task-pattern-registry/.test(m)), 'unknown pattern id was not rejected');
});

check('an ambiguous task pattern id fails closed', () => {
  const dupCatalogue = [...taskPatterns, { id: 'fix-defect', work_kind: 'change', change_class: 'BUGFIX' }];
  const d = mutate((x) => { payload(x).task_pattern = { id: 'fix-defect' }; });
  const p = evaluateTaskSpecification(d, { recordSchema, envelopeSchema, taskPatterns: dupCatalogue });
  assert(p.some((m) => /ambiguous/.test(m)), 'a duplicated catalogue id was not reported as ambiguous');
});

check('work_kind that disagrees with the resolved pattern fails closed', () => {
  const d = mutate((x) => { payload(x).task_pattern = { id: 'fix-defect', work_kind: 'assessment' }; });
  assert(evaluate(d).some((m) => /work_kind "assessment" but task pattern "fix-defect" is work_kind "change"/.test(m)),
    'a work_kind mismatch was not rejected');
});

check('change_class that disagrees with the resolved pattern fails closed', () => {
  const d = mutate((x) => { payload(x).task_pattern = { id: 'fix-defect', work_kind: 'change', change_class: 'REFACTOR' }; });
  assert(evaluate(d).some((m) => /change_class "REFACTOR" but task pattern "fix-defect" is change_class "BUGFIX"/.test(m)),
    'a change_class mismatch was not rejected');
});

check('change_class on a pattern that has none fails closed', () => {
  const d = mutate((x) => { payload(x).task_pattern = { id: 'assess-existing-state', work_kind: 'assessment', change_class: 'BUGFIX' }; });
  assert(evaluate(d).some((m) => /change_class "BUGFIX" but task pattern "assess-existing-state" is change_class "none"/.test(m)),
    'a change_class on an assessment pattern was not rejected');
});

// ---------------------------------------------------------------------------
// mandatory inputs — no hidden defaults
// ---------------------------------------------------------------------------
for (const f of ['goal', 'initial_state', 'target_model']) {
  check(`a missing ${f} is rejected`, () => {
    const d = mutate((x) => { delete payload(x)[f]; });
    assert(evaluate(d).length > 0, `a specification without ${f} was accepted`);
  });
  check(`a whitespace-only ${f} is rejected`, () => {
    const d = mutate((x) => { payload(x)[f] = '   '; });
    assert(evaluate(d).some((m) => /empty or whitespace-only/.test(m)), `a whitespace-only ${f} was accepted`);
  });
}

// ---------------------------------------------------------------------------
// constraints
// ---------------------------------------------------------------------------
check('empty constraints are rejected', () => {
  const d = mutate((x) => { payload(x).constraints = []; });
  assert(evaluate(d).length > 0, 'an empty constraints set was accepted');
});

check('a repeated constraint is rejected', () => {
  const d = mutate((x) => { payload(x).constraints = ['Одно ограничение.', 'Одно ограничение.']; });
  assert(evaluate(d).some((m) => /repeats constraint/.test(m) || /not unique/.test(m)), 'a repeated constraint was accepted');
});

// ---------------------------------------------------------------------------
// acceptance criteria
// ---------------------------------------------------------------------------
check('empty acceptance criteria are rejected', () => {
  const d = mutate((x) => { payload(x).acceptance_criteria = []; });
  assert(evaluate(d).length > 0, 'an empty acceptance criteria set was accepted');
});

check('a duplicate criterion id is rejected', () => {
  const d = mutate((x) => {
    payload(x).acceptance_criteria = [
      { id: 'same', statement: 'A.', verification: { method: 'M.', expected_result: 'R1.' } },
      { id: 'same', statement: 'B.', verification: { method: 'M.', expected_result: 'R2.' } },
    ];
  });
  assert(evaluate(d).some((m) => /id "same" is used more than once/.test(m)), 'a duplicate criterion id was accepted');
});

check('two criteria with identical statement and verification are rejected', () => {
  const d = mutate((x) => {
    payload(x).acceptance_criteria = [
      { id: 'a', statement: 'Same.', verification: { method: 'M.', expected_result: 'R.' } },
      { id: 'b', statement: 'Same.', verification: { method: 'M.', expected_result: 'R.' } },
    ];
  });
  assert(evaluate(d).some((m) => /duplicates another criterion/.test(m)), 'two identical criteria were accepted');
});

check('a criterion without a stable identifier is rejected', () => {
  const d = mutate((x) => {
    payload(x).acceptance_criteria = [{ id: '1bad', statement: 'A.', verification: { method: 'M.', expected_result: 'R.' } }];
  });
  assert(evaluate(d).length > 0, 'a criterion with a non-semantic id was accepted');
});

check('a criterion whose verification is a free phrase is rejected', () => {
  const d = mutate((x) => {
    payload(x).acceptance_criteria = [{ id: 'c1', statement: 'A.', verification: 'всё будет хорошо' }];
  });
  assert(evaluate(d).some((m) => /no verifiable condition/.test(m) || /expected .*object/.test(m)),
    'a free-phrase verification was accepted');
});

check('a criterion with an empty expected_result is rejected', () => {
  const d = mutate((x) => {
    payload(x).acceptance_criteria = [{ id: 'c1', statement: 'A.', verification: { method: 'M.', expected_result: '' } }];
  });
  assert(evaluate(d).length > 0, 'an empty expected_result was accepted');
});

// ---------------------------------------------------------------------------
// run state must not leak into the statement of the work
// ---------------------------------------------------------------------------
check('run-state fields in the payload are rejected', () => {
  for (const f of RUN_STATE_FIELDS) {
    const d = mutate((x) => { payload(x)[f] = 'x'; });
    assert(evaluate(d).length > 0, `run-state field "${f}" was accepted inside the specification`);
  }
});

check('a specification that repeats record_type in the payload is rejected', () => {
  const d = mutate((x) => { payload(x).record_type = RECORD_TYPE; });
  assert(evaluate(d).some((m) => /repeats record_type inside the payload/.test(m)), 'a duplicated record_type was accepted');
});

// ---------------------------------------------------------------------------
// identity
// ---------------------------------------------------------------------------
check('the human-readable name must be stated in Russian', () => {
  const d = mutate((x) => { x.title = 'Latin only project title'; });
  assert(evaluate(d).some((m) => /no Russian \(Cyrillic\) text/.test(m)), 'a Latin-only title was accepted');
});

check('a wrong record_type on the envelope is rejected', () => {
  const d = mutate((x) => { x.record_type = 'task-pattern'; });
  assert(evaluate(d).length > 0, 'a wrong record_type was accepted');
});

// ---------------------------------------------------------------------------
// 4. no product names embedded in the Kernel artifacts of this contract —
// checked as a GENERIC portable property (kernel-purity, after temporary
// indexing of the candidate, is the authoritative product-neutrality gate).
// ---------------------------------------------------------------------------
check('the schema and every valid fixture spec carry no rooted machine path', () => {
  const schemaText = loadText('registries/operating-model/task-specification.schema.json');
  assert(nonPortableReason(schemaText) === null || !/\s\/[a-z]+\/[a-z]/i.test(schemaText),
    'the schema text appears to carry a rooted machine path');
  fixtures.valid.forEach((c, i) => {
    const flat = JSON.stringify(c.spec);
    // the declared $schema is a "../"-relative reference; ignore it, check the rest
    const withoutSchemaRef = flat.replace(JSON.stringify(c.spec.$schema), '""');
    assert(nonPortableReason(withoutSchemaRef.replace(/[",{}\[\]]/g, ' ')) === null,
      `valid fixture ${i} (${c.note}) carries a non-portable path`);
  });
});

// ---------------------------------------------------------------------------
if (failures.length) {
  console.log(`\n${failures.length} failing / ${passed} passing`);
  for (const f of failures) console.log(`  - ${f}`);
  process.exit(1);
}
console.log(`\nAll ${passed} checks passed.`);
