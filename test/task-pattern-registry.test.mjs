#!/usr/bin/env node
// Standalone verification for the built-in task-pattern catalog
// (standards/workspace/task-pattern-registry.yaml), which is a MANDATORY part
// of the Kernel.
//
// It calls the same implementation the gate calls
// (scripts/lib/task-pattern-registry.mjs) so the two cannot drift: the record
// envelope against the existing scoped-record.schema.json, the pattern body
// against the specialised task-pattern-registry.schema.json, the cross-record
// rules, and the path-confinement rule for every `status: present` canonical
// link — proven by construction (absolute / "..", / backslash / symlink escape
// / directory / untracked), not by "the external file happens to be missing".
//
// Usage: node test/task-pattern-registry.test.mjs

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { execFileSync } from 'node:child_process';

import { assertSupportedDeep } from '../scripts/lib/json-schema.mjs';
import { yamlParse } from '../scripts/lib/yaml.mjs';
import {
  evaluateTaskPatternRegistry, checkKernelLinkTarget, checkRuleResolutionBugfixConsistency,
  WORK_KINDS, CHANGE_CLASSES, REQUIRED_PAIRS, LINK_FIELDS,
  REFACTOR_PROTOCOL, REFACTOR_EVIDENCE, BUGFIX_SKILL,
} from '../scripts/lib/task-pattern-registry.mjs';

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

const loadJson = (rel) => JSON.parse(fs.readFileSync(path.join(root, rel), 'utf8'));

const registrySchema = loadJson('registries/operating-model/task-pattern-registry.schema.json');
const envelopeSchema = loadJson('registries/operating-model/scoped-record.schema.json');
const registry = yamlParse(fs.readFileSync(path.join(root, 'standards/workspace/task-pattern-registry.yaml'), 'utf8'));
const fixtures = loadJson('registries/operating-model/fixtures/task-pattern-registry.fixtures.json');

const tracked = new Set(execFileSync('git', ['-C', root, 'ls-files'], { encoding: 'utf8' }).split('\n').filter(Boolean));
const isTracked = (p) => tracked.has(p);
const evalOpts = () => ({ registrySchema, envelopeSchema, kernelRoot: root, isTracked });
const evaluate = (doc) => evaluateTaskPatternRegistry(doc, evalOpts());

const presentPaths = (links) => (Array.isArray(links) ? links : [])
  .filter((l) => l && l.status === 'present' && typeof l.path === 'string').map((l) => l.path);
const pairLabel = (wk, cc) => `(${wk}${cc ? `, ${cc}` : ''})`;

const clone = (x) => JSON.parse(JSON.stringify(x));
const bare = (() => { const d = clone(registry); delete d.$schema; return d; })();
const byPair = (doc, wk, cc) => doc.task_patterns.find((p) => p.payload.work_kind === wk && (p.payload.change_class || null) === (cc || null));

// ---------------------------------------------------------------------------
// schema + real catalog
// ---------------------------------------------------------------------------
check('обе схемы используют поддерживаемое подмножество JSON Schema', () => {
  assertSupportedDeep(registrySchema, 'task-pattern-registry.schema.json');
  assertSupportedDeep(envelopeSchema, 'scoped-record.schema.json');
});

check('каталог не вводит второй словарь work_kind или change_class', () => {
  assert(JSON.stringify(registrySchema.definitions.payload.properties.work_kind.enum) === JSON.stringify(WORK_KINDS),
    'пул work_kind в схеме разошёлся с каноническими четырьмя');
  assert(JSON.stringify(registrySchema.definitions.payload.properties.change_class.enum) === JSON.stringify(CHANGE_CLASSES),
    'пул change_class в схеме разошёлся с каноническими четырьмя');
});

check('действующий каталог проходит композитную проверку без замечаний', () => {
  const problems = evaluate(bare);
  assert(problems.length === 0, problems[0]);
});

check('присутствуют ровно семь шаблонов, по одному на классификационную пару', () => {
  assert(registry.task_patterns.length === 7, `шаблонов: ${registry.task_patterns.length}`);
  for (const [wk, cc] of REQUIRED_PAIRS) assert(byPair(registry, wk, cc), `нет шаблона для пары ${pairLabel(wk, cc)}`);
});

check('каждый шаблон — запись встроенной методологии с нужными областью, происхождением и полномочием', () => {
  for (const p of registry.task_patterns) {
    assert(p.scope.type === 'built-in-methodology' && p.scope.id === 'built-in-methodology', `${p.id}: область не built-in-methodology`);
    assert(p.origin.kind === 'built-in', `${p.id}: происхождение не built-in`);
    assert(p.authority.kind === 'methodology-owner', `${p.id}: полномочие не methodology-owner`);
    assert(p.record_type === 'task-pattern', `${p.id}: record_type не task-pattern`);
  }
});

check('change_class обязателен и допустим только при work_kind: change', () => {
  for (const p of registry.task_patterns) {
    const hasCc = 'change_class' in p.payload;
    assert(p.payload.work_kind === 'change' ? hasCc : !hasCc, `${p.id}: нарушено правило change_class`);
  }
});

check('initiative требует декомпозиции и не получает change_class', () => {
  const i = byPair(registry, 'initiative', null);
  assert(i.payload.decomposition_required === true, 'decomposition_required не true');
  assert(!('change_class' in i.payload), 'у инициативы есть change_class');
  for (const p of registry.task_patterns) {
    if (p.id === i.id) continue;
    assert(!('decomposition_required' in p.payload), `${p.id}: decomposition_required не только у инициативы`);
  }
});

// ---------------------------------------------------------------------------
// замечание 3 — протокол, способ выполнения и контракт доказательств разделены
// ---------------------------------------------------------------------------
check('три оси ссылок присутствуют и обязательны у каждого шаблона', () => {
  for (const p of registry.task_patterns) {
    for (const field of LINK_FIELDS) {
      assert(Array.isArray(p.payload[field]) && p.payload[field].length >= 1, `${p.id}: ось ${field} отсутствует или пуста`);
    }
  }
});

check('REFACTOR ссылается на настоящий протокол функционального паритета', () => {
  const r = byPair(registry, 'change', 'REFACTOR');
  assert(presentPaths(r.payload.applicable_protocols).includes(REFACTOR_PROTOCOL), 'нет ссылки на refactor-protocol.md в applicable_protocols');
  assert(presentPaths(r.payload.applicable_evidence_contracts).includes(REFACTOR_EVIDENCE), 'нет ссылки на контракт паритета');
  assert(fs.existsSync(path.join(root, REFACTOR_PROTOCOL)) && fs.existsSync(path.join(root, REFACTOR_EVIDENCE)), 'канонические файлы отсутствуют');
});

check('BUGFIX связан со способом выполнения skills/bugfix-protocol/SKILL.md как со способом, не как с протоколом', () => {
  const b = byPair(registry, 'change', 'BUGFIX');
  assert(presentPaths(b.payload.applicable_skills).includes(BUGFIX_SKILL), 'нет ссылки на bugfix skill в applicable_skills');
  assert(!presentPaths(b.payload.applicable_protocols).includes(BUGFIX_SKILL), 'способ выполнения оказался в applicable_protocols');
  assert(b.payload.applicable_protocols.every((l) => l.status === 'absent'), 'у BUGFIX появился present-протокол вместо честного absent');
});

check('способ выполнения нельзя поместить в applicable_protocols', () => {
  const d = clone(bare);
  byPair(d, 'change', 'BUGFIX').payload.applicable_protocols = [{ status: 'present', id: 'bugfix-protocol', path: BUGFIX_SKILL }];
  const problems = evaluate(d);
  assert(problems.some((p) => /applicable_protocols link ".*SKILL\.md" points at a skill package/.test(p)), `не отклонено: ${problems[0]}`);
});

check('способ выполнения нельзя поместить в applicable_evidence_contracts', () => {
  const d = clone(bare);
  byPair(d, 'change', 'BUGFIX').payload.applicable_evidence_contracts = [{ status: 'present', id: 'bugfix-protocol', path: BUGFIX_SKILL }];
  assert(evaluate(d).some((p) => /applicable_evidence_contracts link ".*SKILL\.md" points at a skill package/.test(p)), 'не отклонено');
});

check('настоящий протокол нельзя поместить в applicable_skills', () => {
  const d = clone(bare);
  byPair(d, 'change', 'REFACTOR').payload.applicable_skills = [{ status: 'present', id: 'refactor-execution-protocol', path: REFACTOR_PROTOCOL }];
  assert(evaluate(d).some((p) => /applicable_skills link .* is not a skill package/.test(p)), 'не отклонено');
});

check('ось applicable_skills обязательна — её удаление отклоняется схемой', () => {
  const d = clone(bare);
  delete byPair(d, 'change', 'BUGFIX').payload.applicable_skills;
  assert(evaluate(d).length > 0, 'удаление оси applicable_skills прошло чисто');
});

check('BUGFIX не несёт present-протокол — applicable_protocols только absent', () => {
  const b = byPair(registry, 'change', 'BUGFIX');
  assert(b.payload.applicable_protocols.every((l) => l.status === 'absent'),
    'у BUGFIX появился present-протокол вместо честного absent (отдельного протокола BUGFIX в Ядре нет)');
});

check('rule-resolution.md не называет bugfix-protocol протоколом Ядра', () => {
  const rr = fs.readFileSync(path.join(root, 'standards/workspace/rule-resolution.md'), 'utf8');
  const problems = checkRuleResolutionBugfixConsistency(rr);
  assert(problems.length === 0, problems[0]);
});

check('текстовая защита ловит возврат формулировки BUGFIX → bugfix-protocol', () => {
  assert(checkRuleResolutionBugfixConsistency('маршрут `BUGFIX → bugfix-protocol` к протоколу ядра').length > 0,
    'формулировка со стрелкой не поймана');
  assert(checkRuleResolutionBugfixConsistency('строка про bugfix-protocol как протокол ядра').length > 0,
    'формулировка «протокол ядра» на одной строке с bugfix-protocol не поймана');
  assert(checkRuleResolutionBugfixConsistency('для `BUGFIX` — способ выполнения `bugfix-protocol` (skill, не protocol)').length === 0,
    'корректная формулировка ошибочно помечена');
});

// ---------------------------------------------------------------------------
// замечание 2 — выход ссылок за пределы Ядра закрыт, принадлежность доказана
// ---------------------------------------------------------------------------
const work = fs.mkdtempSync(path.join(os.tmpdir(), 'tpr-link-'));
const kdir = path.join(work, 'kernel');
fs.mkdirSync(path.join(kdir, 'sub'), { recursive: true });
fs.writeFileSync(path.join(kdir, 'sub', 'tracked.md'), 'tracked\n');
fs.writeFileSync(path.join(kdir, 'sub', 'untracked.md'), 'untracked\n');
execFileSync('git', ['-C', kdir, 'init', '-q']);
execFileSync('git', ['-C', kdir, 'add', 'sub/tracked.md']);
execFileSync('git', ['-C', kdir, '-c', 'user.name=t', '-c', 'user.email=t@t.invalid', 'commit', '-q', '-m', 'seed']);
const kTracked = new Set(execFileSync('git', ['-C', kdir, 'ls-files'], { encoding: 'utf8' }).split('\n').filter(Boolean));
const kIsTracked = (p) => kTracked.has(p);
const external = path.join(work, 'outside.md');
fs.writeFileSync(external, 'a real external file\n');
let symlinkOk = true;
try { fs.symlinkSync(external, path.join(kdir, 'sub', 'escape.md')); }
catch { symlinkOk = false; }
const linkCheck = (rel) => checkKernelLinkTarget(rel, { kernelRoot: kdir, isTracked: kIsTracked });

check('present-ссылка на обычный отслеживаемый файл внутри Ядра допустима', () => {
  const v = linkCheck('sub/tracked.md');
  assert(v.ok === true, `ожидался ok, получено: ${v.reason}`);
});

check('абсолютный путь отклоняется', () => {
  const v = linkCheck(path.join(kdir, 'sub', 'tracked.md'));
  assert(v.ok === false && /absolute/.test(v.reason), v.reason);
});

check('../outside.md отклоняется по принадлежности, хотя внешний файл существует', () => {
  assert(fs.existsSync(external), 'внешний файл не создан — сценарий недействителен');
  const v = linkCheck('../outside.md');
  assert(v.ok === false, 'ссылка за пределы Ядра принята');
  assert(!/does not exist/.test(v.reason), `отклонено по отсутствию файла, а не по принадлежности: ${v.reason}`);
});

check('символическая ссылка из Ядра на существующий внешний файл отклоняется', () => {
  if (!symlinkOk) { console.log('  (SKIP: символические ссылки недоступны в этой системе)'); return; }
  const v = linkCheck('sub/escape.md');
  assert(v.ok === false && /symbolic links/.test(v.reason), v.reason);
});

check('существующий каталог вместо файла отклоняется', () => {
  const v = linkCheck('sub');
  assert(v.ok === false && /not a regular file/.test(v.reason), v.reason);
});

check('существующий, но неотслеживаемый файл отклоняется', () => {
  const v = linkCheck('sub/untracked.md');
  assert(v.ok === false && /tracked file set/.test(v.reason), v.reason);
});

check('путь с обратной косой чертой отклоняется', () => {
  const v = linkCheck('sub\\tracked.md');
  assert(v.ok === false && /backslash/.test(v.reason), v.reason);
});

check('путь с сегментом ".." или "." отклоняется как ненормализованный', () => {
  assert(linkCheck('sub/../sub/tracked.md').ok === false, '".." сегмент принят');
  assert(linkCheck('./sub/tracked.md').ok === false, '"." сегмент принят');
});

fs.rmSync(work, { recursive: true, force: true });

// ---------------------------------------------------------------------------
// bundle + adversarial rejections
// ---------------------------------------------------------------------------
check('для отсутствующей канонической связи используется явный status: absent без id/path', () => {
  let sawAbsent = false;
  for (const p of registry.task_patterns) {
    for (const link of LINK_FIELDS.flatMap((f) => p.payload[f])) {
      if (link.status === 'absent') {
        sawAbsent = true;
        assert(typeof link.absence_reason === 'string' && link.absence_reason.length > 0, `${p.id}: absent без причины`);
        assert(!('id' in link) && !('path' in link), `${p.id}: absent несёт id/path`);
      } else {
        assert(typeof link.id === 'string' && typeof link.path === 'string', `${p.id}: present без id/path`);
      }
    }
  }
  assert(sawAbsent, 'ни одна ссылка не помечена absent — проверка представления отсутствия не выполнена');
});

check('бандл фикстур: каждая valid проходит, каждая invalid отклоняется', () => {
  assert(Array.isArray(fixtures.valid) && fixtures.valid.length > 0, 'нет непустого массива valid');
  assert(Array.isArray(fixtures.invalid) && fixtures.invalid.length > 0, 'нет непустого массива invalid');
  for (const c of fixtures.valid) {
    const problems = evaluate(c.registry);
    assert(problems.length === 0, `valid-фикстура отклонена (${c.note}): ${problems[0]}`);
  }
  for (const c of fixtures.invalid) {
    assert(evaluate(c.registry).length > 0, `invalid-фикстура прошла чисто (${c.note})`);
  }
});

const mutate = (fn) => { const d = clone(bare); fn(d); return d; };
const NEGATIVES = [
  ['неизвестный work_kind', (d) => { byPair(d, 'assessment', null).payload.work_kind = 'inspection'; }],
  ['отсутствующий change_class у change', (d) => { delete byPair(d, 'change', 'FEATURE').payload.change_class; }],
  ['change_class у assessment', (d) => { byPair(d, 'assessment', null).payload.change_class = 'REFACTOR'; }],
  ['change_class у operation', (d) => { byPair(d, 'operation', null).payload.change_class = 'BUGFIX'; }],
  ['change_class у initiative', (d) => { byPair(d, 'initiative', null).payload.change_class = 'REFACTOR'; }],
  ['неизвестный change_class', (d) => { byPair(d, 'change', 'BUGFIX').payload.change_class = 'PATCH'; }],
  ['повторяющаяся классификационная пара', (d) => { const b = byPair(d, 'change', 'BUGFIX'); b.payload.change_class = 'FEATURE'; b.id = 'add-capability-2'; }],
  ['повторяющийся id шаблона', (d) => { byPair(d, 'change', 'FEATURE').id = 'fix-defect'; }],
  ['отсутствие обязательного шаблона', (d) => { d.task_patterns = d.task_patterns.filter((p) => p.id !== 'change-behavior'); }],
  ['пустой обязательный вход', (d) => { byPair(d, 'change', 'REFACTOR').payload.required_inputs = ['']; }],
  ['пустой инвариант (пустой список)', (d) => { byPair(d, 'assessment', null).payload.invariants = []; }],
  ['пустое условие остановки', (d) => { byPair(d, 'operation', null).payload.stop_conditions = ['x', '']; }],
  ['продуктовая область вместо built-in-methodology', (d) => { byPair(d, 'assessment', null).scope = { type: 'project-workspace', id: 'p', workspace_id: 'p' }; }],
  ['неверное происхождение', (d) => { byPair(d, 'operation', null).origin = { kind: 'declared', source_ref: 'x' }; }],
  ['неверное полномочие', (d) => { byPair(d, 'change', 'FEATURE').authority = { kind: 'project-owner', authority_ref: 'x' }; }],
  ['ссылка на несуществующий протокол', (d) => { byPair(d, 'change', 'REFACTOR').payload.applicable_protocols = [{ status: 'present', id: 'ghost', path: 'verification/ghost.md' }]; }],
  ['ссылка на несуществующий контракт доказательств', (d) => { byPair(d, 'change', 'REFACTOR').payload.applicable_evidence_contracts = [{ status: 'present', id: 'ghost', path: 'verification/ghost.md' }]; }],
  ['абсолютный путь в present-ссылке', (d) => { byPair(d, 'assessment', null).payload.applicable_protocols = [{ status: 'present', id: 'abs', path: '/etc/x.md' }]; }],
  ['путь с обратной косой чертой в present-ссылке', (d) => { byPair(d, 'assessment', null).payload.applicable_protocols = [{ status: 'present', id: 'bs', path: 'workflows\\task-lifecycle.md' }]; }],
  ['present-ссылка на каталог', (d) => { byPair(d, 'assessment', null).payload.applicable_protocols = [{ status: 'present', id: 'dir', path: 'workflows' }]; }],
  ['absent-ссылка с id и path', (d) => { byPair(d, 'change', 'FEATURE').payload.applicable_evidence_contracts = [{ status: 'absent', id: 'x', path: 'y', absence_reason: 'r' }]; }],
  ['дополнительное неизвестное поле в payload', (d) => { byPair(d, 'change', 'REFACTOR').payload.extra = 1; }],
  ['дополнительное неизвестное поле в конверте', (d) => { byPair(d, 'assessment', null).legacy_id = 'old'; }],
  ['способ выполнения в applicable_protocols', (d) => { byPair(d, 'change', 'BUGFIX').payload.applicable_protocols = [{ status: 'present', id: 'bugfix-protocol', path: BUGFIX_SKILL }]; }],
  ['протокол в applicable_skills', (d) => { byPair(d, 'change', 'REFACTOR').payload.applicable_skills = [{ status: 'present', id: 'refactor-execution-protocol', path: REFACTOR_PROTOCOL }]; }],
  ['ось applicable_skills удалена', (d) => { delete byPair(d, 'change', 'BUGFIX').payload.applicable_skills; }],
  ['REFACTOR без маршрута функционального паритета', (d) => { byPair(d, 'change', 'REFACTOR').payload.applicable_protocols = [{ status: 'present', id: 'standard-task-lifecycle', path: 'workflows/task-lifecycle.md' }]; }],
  ['BUGFIX без bugfix skill', (d) => { byPair(d, 'change', 'BUGFIX').payload.applicable_skills = [{ status: 'absent', absence_reason: 'none' }]; }],
  ['BUGFIX делит способ выполнения с FEATURE', (d) => { byPair(d, 'change', 'FEATURE').payload.applicable_skills = [{ status: 'present', id: 'bugfix-protocol', path: BUGFIX_SKILL }]; }],
  ['initiative без decomposition_required', (d) => { delete byPair(d, 'initiative', null).payload.decomposition_required; }],
  ['record_type не task-pattern', (d) => { byPair(d, 'operation', null).record_type = 'norm'; }],
];
for (const [name, fn] of NEGATIVES) {
  check(`отклоняется: ${name}`, () => {
    assert(evaluate(mutate(fn)).length > 0, 'дефект прошёл проверку');
  });
}

check('неподдерживаемое ключевое слово схемы отклоняется движком', () => {
  const broken = clone(registrySchema);
  broken.definitions.payload.patternProperties = { '^x': { type: 'string' } };
  let threw = false;
  try { assertSupportedDeep(broken, 'broken'); } catch { threw = true; }
  assert(threw, 'assertSupportedDeep пропустил неподдерживаемое ключевое слово');
});

console.log(`\n${passed} passed, ${failures.length} failed`);
if (failures.length) process.exit(1);
