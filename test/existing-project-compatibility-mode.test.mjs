#!/usr/bin/env node
// Standalone verification for the existing-project compatibility mode contract
// (registries/operating-model/existing-project-compatibility-mode.schema.json).
//
// It calls the same implementation the gate calls
// (scripts/lib/existing-project-compatibility-mode.mjs) so the two cannot
// drift: the record envelope against the existing scoped-record.schema.json,
// the scan payload against the specialised
// existing-project-compatibility-mode.schema.json, and the rules JSON Schema
// cannot state — bounded discovery-plan confinement, composition with the
// REAL instruction-source-registry and controlled-rule-intake contracts (not
// a copy of either), the "discovery mints no decision" rule, the
// finding-on-change rule, and the deterministic next_step.
//
// It also exercises scanDiscoveryPlan — the read-only directory scan — against
// a real temporary project directory to prove the zero-write guarantee on
// actual bytes, not only on hand-written fixtures.
//
// Usage: node test/existing-project-compatibility-mode.test.mjs

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

import { assertSupportedDeep } from '../scripts/lib/json-schema.mjs';
import {
  evaluateExistingProjectCompatibilityMode,
  scanDiscoveryPlan,
  checkPlanLocation,
  buildSourceResolver,
  deriveSourceReference,
  computeNextStep,
  RECORD_TYPE, CONNECTION_MODES, SCAN_KINDS, FINDING_KINDS, NEXT_STEPS, DISCOVERY_STATUSES,
  ALLOWED_SCOPE_TYPES, REQUIRED_ORIGIN_KIND, REQUIRED_AUTHORITY_KIND, MANAGED_MODE_AUTHORITY_BY_SCOPE,
  MEDIA,
} from '../scripts/lib/existing-project-compatibility-mode.mjs';

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

const registrySchema = loadJson('registries/operating-model/existing-project-compatibility-mode.schema.json');
const envelopeSchema = loadJson('registries/operating-model/scoped-record.schema.json');
const sourceRegistrySchema = loadJson('registries/operating-model/instruction-source-registry.schema.json');
const ruleIntakeSchema = loadJson('registries/operating-model/controlled-rule-intake.schema.json');
const fixtures = loadJson('registries/operating-model/fixtures/existing-project-compatibility-mode.fixtures.json');

const opts = { registrySchema, envelopeSchema, sourceRegistrySchema, ruleIntakeSchema };
const evaluate = (doc) => evaluateExistingProjectCompatibilityMode(doc, opts);
const clone = (x) => JSON.parse(JSON.stringify(x));

const DIG_A = 'aaaa1111'.repeat(8);

// A topologically complete valid single-entry document, used as the base for
// targeted mutations. Reused directly from the fixtures bundle's second valid
// case (initial read of one meridian-observed source) so this file carries no
// second, hand-duplicated copy of a "known-good" document.
const BASE = clone(fixtures.valid.find((c) => c.note.includes('meridian-observed')).registry);
const entry0 = (d) => d.workspace_connections[0];
const payload0 = (d) => entry0(d).payload;
const mutate = (fn) => { const d = clone(BASE); fn(d); return d; };

// ---------------------------------------------------------------------------
// baseline
// ---------------------------------------------------------------------------
check('базовый документ проходит композитную проверку без замечаний', () => {
  const problems = evaluate(BASE);
  assert(problems.length === 0, problems[0]);
});

check('пустой реестр подключений допустим', () => {
  assert(evaluate({ ...BASE, workspace_connections: [] }).length === 0, 'пустой реестр отклонён');
});

check('бандл фикстур: каждая valid проходит, каждая invalid отклоняется', () => {
  assert(Array.isArray(fixtures.valid) && fixtures.valid.length >= 10, 'нет минимум десяти положительных примеров');
  assert(Array.isArray(fixtures.invalid) && fixtures.invalid.length >= 17, 'нет минимум семнадцати отрицательных примеров');
  for (const c of fixtures.valid) {
    const problems = evaluate(c.registry);
    assert(problems.length === 0, `valid-фикстура отклонена (${c.note}): ${problems[0]}`);
  }
  for (const c of fixtures.invalid) {
    assert(evaluate(c.registry).length > 0, `invalid-фикстура прошла чисто (${c.note})`);
  }
});

// ---------------------------------------------------------------------------
// обязательность четырёх композиционных схем — независимо от содержимого
// ---------------------------------------------------------------------------
check('отсутствие registrySchema отклоняется закрыто, даже на честном документе', () => {
  const p = evaluateExistingProjectCompatibilityMode(BASE, { ...opts, registrySchema: undefined });
  assert(p.some((x) => /requires .*registrySchema/.test(x)), 'отсутствие registrySchema принято');
});
check('отсутствие envelopeSchema отклоняется закрыто, даже на честном документе', () => {
  const p = evaluateExistingProjectCompatibilityMode(BASE, { ...opts, envelopeSchema: undefined });
  assert(p.some((x) => /requires .*envelopeSchema/.test(x)), 'отсутствие envelopeSchema принято');
});
check('отсутствие sourceRegistrySchema отклоняется закрыто, даже без discovered_sources', () => {
  const emptyDoc = mutate((x) => {
    payload0(x).discovery_plan = [];
    payload0(x).discovered_sources = [];
  });
  const p = evaluateExistingProjectCompatibilityMode(emptyDoc, { ...opts, sourceRegistrySchema: undefined });
  assert(p.some((x) => /requires .*sourceRegistrySchema/.test(x)), 'отсутствие sourceRegistrySchema на пустом discovered_sources принято');
});
check('отсутствие ruleIntakeSchema отклоняется закрыто, даже без rule_candidates', () => {
  const emptyDoc = mutate((x) => { payload0(x).rule_candidates = []; });
  const p = evaluateExistingProjectCompatibilityMode(emptyDoc, { ...opts, ruleIntakeSchema: undefined });
  assert(p.some((x) => /requires .*ruleIntakeSchema/.test(x)), 'отсутствие ruleIntakeSchema на пустом rule_candidates принято');
});
check('неприменимая (не объект) sourceRegistrySchema отклоняется закрыто', () => {
  const p = evaluateExistingProjectCompatibilityMode(BASE, { ...opts, sourceRegistrySchema: 'not-a-schema' });
  assert(p.some((x) => /requires .*sourceRegistrySchema/.test(x)), 'неприменимая sourceRegistrySchema принята');
});
check('документ с обнаруженным источником без sourceRegistrySchema даёт явную проблему, а не проходит чисто', () => {
  const p = evaluateExistingProjectCompatibilityMode(BASE, { ...opts, sourceRegistrySchema: undefined });
  assert(p.length > 0, 'документ с discovered_sources прошёл чисто без sourceRegistrySchema');
});

// Корректирующий раунд: {} (или любой другой объект) более не проходит как
// применимая схема ни для одной из четырёх композиционных зависимостей —
// применимость проверяется по идентичности контракта ($id, registry_id.const,
// массив записей и его контракт), а не только по факту "это объект и движок
// его не отверг".
check('пустая схема {} вместо любой из четырёх композиционных схем отклоняется закрыто', () => {
  for (const key of ['registrySchema', 'envelopeSchema', 'sourceRegistrySchema', 'ruleIntakeSchema']) {
    const p = evaluateExistingProjectCompatibilityMode(BASE, { ...opts, [key]: {} });
    assert(p.some((x) => new RegExp(`requires .*${key}`).test(x)), `{} вместо ${key} принята как применимая схема`);
  }
});

check('поддерживаемая, но чужая (не своего контракта) схема на месте композиционной зависимости отклоняется', () => {
  // sourceRegistrySchema и ruleIntakeSchema — обе реальные, полностью
  // поддерживаемые движком схемы реестров; их перестановка доказывает, что
  // идентичность проверяется ПОВЕРХ применимости движка, а не выводится из
  // неё — движок принял бы содержимое любой из них как валидную схему.
  const p1 = evaluateExistingProjectCompatibilityMode(BASE, { ...opts, sourceRegistrySchema: ruleIntakeSchema });
  assert(p1.some((x) => /requires .*sourceRegistrySchema/.test(x)), 'схема controlled-rule-intake принята вместо instruction-source-registry');

  const p2 = evaluateExistingProjectCompatibilityMode(BASE, { ...opts, ruleIntakeSchema: sourceRegistrySchema });
  assert(p2.some((x) => /requires .*ruleIntakeSchema/.test(x)), 'схема instruction-source-registry принята вместо controlled-rule-intake');

  const p3 = evaluateExistingProjectCompatibilityMode(BASE, { ...opts, registrySchema: envelopeSchema });
  assert(p3.some((x) => /requires .*registrySchema/.test(x)), 'схема scoped-record принята вместо existing-project-compatibility-mode');
});

check('неподдерживаемое ключевое слово в глубокой, никогда не посещаемой ветви sourceRegistrySchema отклоняется даже при пустых discovered_sources', () => {
  const emptyDoc = mutate((x) => {
    payload0(x).discovery_plan = [];
    payload0(x).discovered_sources = [];
  });
  const broken = clone(sourceRegistrySchema);
  broken.definitions.payload.patternProperties = { '^x': { type: 'string' } };
  const p = evaluateExistingProjectCompatibilityMode(emptyDoc, { ...opts, sourceRegistrySchema: broken });
  assert(p.some((x) => /requires .*sourceRegistrySchema/.test(x)), 'сломанная sourceRegistrySchema принята при пустых discovered_sources');
  assert(p.some((x) => /not fully supported by the validation engine/.test(x)), 'неподдерживаемое ключевое слово в невызванной ветви не выявлено');
});

check('обнаруженный источник с посторонним полем payload при sourceRegistrySchema: {} всё равно отклоняется', () => {
  const d = mutate((x) => { payload0(x).discovered_sources[0].payload.alien_field = 'unexpected'; });
  const p = evaluateExistingProjectCompatibilityMode(d, { ...opts, sourceRegistrySchema: {} });
  assert(p.length > 0, 'постороннее поле обнаруженного источника прошло чисто при sourceRegistrySchema: {}');
  assert(p.some((x) => /requires .*sourceRegistrySchema/.test(x)), 'sourceRegistrySchema: {} не отклонена как несостоятельная схема');
});
// "документ с кандидатом без ruleIntakeSchema" — see below, after withCandidate is defined.

// ---------------------------------------------------------------------------
// свойство 1 — compatibility по умолчанию, managed никогда автоматически
// ---------------------------------------------------------------------------
check('managed без managed_mode_decision отклоняется', () => {
  assert(evaluate(mutate((d) => { payload0(d).connection_mode = 'managed'; }))
    .some((p) => /managed mode never activates automatically/.test(p)), 'автоматический managed принят');
});

check('managed с decision, но неверным полномочием области, отклоняется', () => {
  const d = mutate((x) => {
    payload0(x).connection_mode = 'managed';
    payload0(x).managed_mode_decision = {
      decision_ref: 'owner-decision:managed', decided_at: '2026-09-13', reason: 'x',
      authority: { kind: 'project-owner', authority_ref: 'workspace-owner' },
    };
  });
  assert(evaluate(d).some((p) => /requires owner authority "repository-maintainer"/.test(p)), 'неверное полномочие managed принято');
});

check('managed с корректным decision и полномочием проходит', () => {
  const d = mutate((x) => {
    payload0(x).connection_mode = 'managed';
    payload0(x).managed_mode_decision = {
      decision_ref: 'owner-decision:managed', decided_at: '2026-09-13', reason: 'x',
      authority: { kind: 'repository-maintainer', authority_ref: 'sample-repository-maintainer' },
    };
  });
  assert(evaluate(d).length === 0, `корректный managed_mode_decision отклонён: ${evaluate(d)[0]}`);
});

check('MANAGED_MODE_AUTHORITY_BY_SCOPE — по одному полномочию на каждую из двух областей', () => {
  assert(MANAGED_MODE_AUTHORITY_BY_SCOPE['project-workspace'] === 'project-owner', 'project-workspace разошёлся');
  assert(MANAGED_MODE_AUTHORITY_BY_SCOPE['repository-scope'] === 'repository-maintainer', 'repository-scope разошёлся');
});

// ---------------------------------------------------------------------------
// свойство 2 — нулевая запись в проект: структурно (нет write-вызовов) и
// поведенчески (реальный каталог не меняется байт в байт)
// ---------------------------------------------------------------------------
check('модуль не содержит ни одного вызова записи/удаления/переименования файла', () => {
  const src = fs.readFileSync(path.join(root, 'scripts', 'lib', 'existing-project-compatibility-mode.mjs'), 'utf8');
  const forbidden = /fs\.(writeFile|appendFile|unlink|rename|mkdir|rmdir|rm|chmod|copyFile|truncate|symlink|link)(Sync)?\s*\(/;
  assert(!forbidden.test(src), 'найден вызов, способный изменить файловую систему');
});

check('постороннее поле, несущее запись в проект, отклоняется схемой', () => {
  const d = mutate((x) => { payload0(x).project_file_writes = [{ path: 'AGENTS.md', content: 'x' }]; });
  assert(evaluate(d).length > 0, 'поле записи в проект принято');
});

function sha256(buf) { return crypto.createHash('sha256').update(buf).digest('hex'); }
function snapshotTree(dir) {
  const out = {};
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, entry.name);
    if (entry.isDirectory()) Object.assign(out, snapshotTree(p));
    else out[p] = fs.readFileSync(p);
  }
  return out;
}
function sameTree(a, b) {
  const ak = Object.keys(a).sort();
  const bk = Object.keys(b).sort();
  if (JSON.stringify(ak) !== JSON.stringify(bk)) return false;
  return ak.every((k) => Buffer.compare(a[k], b[k]) === 0);
}

check('сканирование реального каталога не меняет ни единого байта — до/после первоначального и повторного сканирования', () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'epcm-zero-write-'));
  try {
    fs.writeFileSync(path.join(tmp, 'AGENTS.md'), 'Branches follow feature/<slug>.\n');
    fs.mkdirSync(path.join(tmp, '.cursor', 'rules'), { recursive: true });
    fs.writeFileSync(path.join(tmp, '.cursor', 'rules', 'sample.mdc'), 'rule body\n');
    fs.writeFileSync(path.join(tmp, 'unrelated.txt'), 'not part of any discovery plan\n');

    const plan = [
      { id: 'root-agents-md', medium: 'file', path: 'AGENTS.md', container_ref: 'sample-repository' },
      { id: 'root-cursor-rule', medium: 'file', path: '.cursor/rules/sample.mdc', container_ref: 'sample-repository' },
      { id: 'root-missing', medium: 'file', path: 'CLAUDE.md', container_ref: 'sample-repository' },
    ];

    const before = snapshotTree(tmp);
    const initial = scanDiscoveryPlan(plan, { containerRoot: tmp });
    const afterInitial = snapshotTree(tmp);
    assert(sameTree(before, afterInitial), 'первоначальное сканирование изменило рабочее дерево');

    const byId = Object.fromEntries(initial.map((r) => [r.planId, r]));
    assert(byId['root-agents-md'].status === 'discovered', 'AGENTS.md не обнаружен');
    assert(byId['root-cursor-rule'].status === 'discovered', 'правило Cursor не обнаружено');
    assert(byId['root-missing'].status === 'missing', 'CLAUDE.md ошибочно не отмечен как отсутствующий');

    const rescan = scanDiscoveryPlan(plan, { containerRoot: tmp });
    const afterRescan = snapshotTree(tmp);
    assert(sameTree(before, afterRescan), 'повторное сканирование изменило рабочее дерево');
    assert(sameTree(afterInitial, afterRescan), 'дерево разошлось между первоначальным и повторным сканированием');

    const rescanById = Object.fromEntries(rescan.map((r) => [r.planId, r]));
    assert(rescanById['root-agents-md'].digest === byId['root-agents-md'].digest, 'дайджест неизменного файла разошёлся между сканированиями');
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
});

check('external-service слот не сканируется локально и не выдаёт ложного discovered', () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'epcm-external-'));
  try {
    const r = scanDiscoveryPlan(
      [{ id: 'remote-wiki', medium: 'external-service', service_ref: 'wiki', resource_ref: 'page-1' }],
      { containerRoot: tmp },
    );
    assert(r[0].status === 'unreadable' && r[0].reason === 'external-service-not-scanned-locally', 'external-service слот обработан локально как файл');
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
});

// ---------------------------------------------------------------------------
// файловая граница scanDiscoveryPlan закрыта до любого stat/read
// ---------------------------------------------------------------------------
check('план "../outside.txt" не читает outside.txt и не возвращает его дайджест', () => {
  const parent = fs.mkdtempSync(path.join(os.tmpdir(), 'epcm-escape-'));
  try {
    const outsideContent = 'секрет вне root\n';
    fs.writeFileSync(path.join(parent, 'outside.txt'), outsideContent);
    const rootDir = path.join(parent, 'root');
    fs.mkdirSync(rootDir);
    const outsideDigest = sha256(Buffer.from(outsideContent));

    const r = scanDiscoveryPlan(
      [{ id: 'escape-dotdot', medium: 'file', path: '../outside.txt', container_ref: 'x' }],
      { containerRoot: rootDir },
    );
    assert(r[0].status === 'invalid', `путь с ".." дал статус "${r[0].status}", а не "invalid"`);
    assert(r[0].status !== 'missing', 'выход через ".." классифицирован как missing, а не отклонён явно');
    assert(r[0].digest !== outsideDigest, 'дайджест outside.txt всё же вычислен');
    assert(!('digest' in r[0]), 'результат несёт дайджест, хотя чтение обязано быть заблокировано');
  } finally {
    fs.rmSync(parent, { recursive: true, force: true });
  }
});

check('символическая ссылка внутри root, ведущая наружу, не читается', () => {
  const parent = fs.mkdtempSync(path.join(os.tmpdir(), 'epcm-symlink-'));
  try {
    const outsideContent = 'секрет вне root через симлинк\n';
    fs.writeFileSync(path.join(parent, 'outside.txt'), outsideContent);
    const rootDir = path.join(parent, 'root');
    fs.mkdirSync(rootDir);
    fs.symlinkSync(path.join('..', 'outside.txt'), path.join(rootDir, 'link-out'));

    const r = scanDiscoveryPlan(
      [{ id: 'escape-symlink', medium: 'file', path: 'link-out', container_ref: 'x' }],
      { containerRoot: rootDir },
    );
    assert(r[0].status === 'invalid', `симлинк наружу дал статус "${r[0].status}", а не "invalid"`);
    assert(!('digest' in r[0]), 'содержимое цели симлинка вне root всё же прочитано');
  } finally {
    fs.rmSync(parent, { recursive: true, force: true });
  }
});

check('некорректный слот (абсолютный путь) отклоняется явно, а не классифицируется как missing', () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'epcm-invalid-slot-'));
  try {
    const r = scanDiscoveryPlan(
      [{ id: 'bad-slot', medium: 'file', path: '/etc/passwd', container_ref: 'x' }],
      { containerRoot: tmp },
    );
    assert(r[0].status === 'invalid', `абсолютный путь дал статус "${r[0].status}", а не "invalid"`);
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
});

check('некорректный containerRoot отклоняет file-слот явно, без обращения к диску', () => {
  for (const badRoot of [undefined, '', '   ']) {
    const r = scanDiscoveryPlan(
      [{ id: 'no-root', medium: 'file', path: 'AGENTS.md', container_ref: 'x' }],
      { containerRoot: badRoot },
    );
    assert(r[0].status === 'invalid', `containerRoot ${JSON.stringify(badRoot)} дал статус "${r[0].status}", а не "invalid"`);
  }
});

check('честный вложенный путь внутри root по-прежнему обнаруживается (граница не ложно-позитивна)', () => {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'epcm-nested-ok-'));
  try {
    fs.mkdirSync(path.join(tmp, 'a', 'b'), { recursive: true });
    fs.writeFileSync(path.join(tmp, 'a', 'b', 'c.md'), 'rule\n');
    const r = scanDiscoveryPlan(
      [{ id: 'nested', medium: 'file', path: 'a/b/c.md', container_ref: 'x' }],
      { containerRoot: tmp },
    );
    assert(r[0].status === 'discovered', `честный вложенный путь дал статус "${r[0].status}", а не "discovered"`);
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
});

// ---------------------------------------------------------------------------
// свойство 3 — ограниченное обнаружение
// ---------------------------------------------------------------------------
check('checkPlanLocation отклоняет абсолютный путь', () => {
  assert(!checkPlanLocation({ medium: 'file', path: '/etc/passwd', container_ref: 'x' }).ok, 'абсолютный путь принят');
});
check('checkPlanLocation отклоняет выход через ..', () => {
  assert(!checkPlanLocation({ medium: 'file', path: '../../etc/passwd', container_ref: 'x' }).ok, 'выход через .. принят');
});
check('checkPlanLocation отклоняет неизвестный носитель', () => {
  assert(!checkPlanLocation({ medium: 'ftp', path: 'x', container_ref: 'x' }).ok, 'неизвестный носитель принят');
});
check('checkPlanLocation отклоняет обратную косую черту', () => {
  assert(!checkPlanLocation({ medium: 'file', path: 'a\\b', container_ref: 'x' }).ok, 'обратная косая черта принята');
});
check('checkPlanLocation отклоняет непрозрачный container_ref в виде машинного пути', () => {
  assert(!checkPlanLocation({ medium: 'file', path: 'a', container_ref: '/abs/path' }).ok, 'абсолютный container_ref принят');
});
check('checkPlanLocation принимает честный file-слот', () => {
  assert(checkPlanLocation({ medium: 'file', path: 'AGENTS.md', container_ref: 'sample-repository' }).ok, 'честный слот отклонён');
});
check('checkPlanLocation принимает честный external-service-слот', () => {
  assert(checkPlanLocation({ medium: 'external-service', service_ref: 'wiki', resource_ref: 'page-1' }).ok, 'честный external-service слот отклонён');
});

check('обнаруженный источник вне объявленного плана отклоняется', () => {
  const d = mutate((x) => { payload0(x).discovery_plan = []; });
  assert(evaluate(d).some((p) => /does not correspond to any declared discovery_plan slot/.test(p)), 'источник вне плана принят');
});

check('missing_sources plan_id вне плана отклоняется', () => {
  const d = mutate((x) => {
    payload0(x).missing_sources = [{ plan_id: 'ghost-slot', previously_known: false }];
  });
  assert(evaluate(d).some((p) => /missing_sources plan_id "ghost-slot" does not correspond/.test(p)), 'plan_id вне плана принят');
});

check('unreadable_sources plan_id вне плана отклоняется', () => {
  const d = mutate((x) => {
    payload0(x).unreadable_sources = [{ plan_id: 'ghost-slot', reason: 'x', previously_known: false }];
  });
  assert(evaluate(d).some((p) => /unreadable_sources plan_id "ghost-slot" does not correspond/.test(p)), 'plan_id вне плана принят');
});

check('дублирующийся id слота плана отклоняется', () => {
  const d = mutate((x) => {
    payload0(x).discovery_plan.push({ id: 'root-agents-md', medium: 'file', path: 'AGENTS2.md', container_ref: 'sample-repository' });
  });
  assert(evaluate(d).some((p) => /discovery_plan slot id "root-agents-md" is declared more than once/.test(p)), 'дублирующийся slot id принят');
});

// ---------------------------------------------------------------------------
// исходы discovery_plan — полное взаимоисключающее разбиение (ровно один
// исход на слот): omitted, overlap, duplicate — прямые тесты и фикстуры.
// ---------------------------------------------------------------------------
check('пропущенный исход (omitted) отклоняется', () => {
  const d = mutate((x) => {
    payload0(x).discovery_plan.push({ id: 'root-claude-md', medium: 'file', path: 'CLAUDE.md', container_ref: 'sample-repository' });
  });
  assert(evaluate(d).some((p) => /discovery_plan slot "root-claude-md" has no outcome/.test(p)), 'слот без исхода принят');
});

check('пересечение исходов (overlap: одновременно discovered и missing) отклоняется', () => {
  const d = mutate((x) => {
    payload0(x).missing_sources.push({ plan_id: 'root-agents-md', previously_known: false });
  });
  assert(evaluate(d).some((p) => /discovery_plan slot "root-agents-md" has 2 outcomes/.test(p)), 'слот с исходом в двух массивах принят');
});

check('повтор исхода (duplicate: дважды в одном массиве) отклоняется', () => {
  const d = mutate((x) => {
    payload0(x).discovery_plan = [{ id: 'root-cursor-rule', medium: 'file', path: '.cursor/rules/sample.mdc', container_ref: 'sample-repository' }];
    payload0(x).discovered_sources = [];
    payload0(x).unreadable_sources = [
      { plan_id: 'root-cursor-rule', reason: 'permission denied', previously_known: false },
      { plan_id: 'root-cursor-rule', reason: 'permission denied', previously_known: false },
    ];
    payload0(x).findings = [{ id: 'f1', kind: 'source-unreadable', detail: 'x', plan_id: 'root-cursor-rule', blocking: false }];
  });
  assert(evaluate(d).some((p) => /discovery_plan slot "root-cursor-rule" has 2 outcomes/.test(p)), 'повтор исхода в одном массиве принят');
});

check('результат для необъявленного слота не подменяет учёт исходов', () => {
  const d = mutate((x) => {
    payload0(x).missing_sources.push({ plan_id: 'never-declared', previously_known: false });
  });
  const problems = evaluate(d);
  assert(problems.some((p) => /missing_sources plan_id "never-declared" does not correspond/.test(p)), 'необъявленный слот не отклонён');
  assert(!problems.some((p) => /"root-agents-md" has no outcome/.test(p)), 'необъявленный слот ошибочно засчитан в исход объявленного');
});

check('фикстуры overlap/duplicate/omitted присутствуют и отклоняются', () => {
  for (const substr of ['overlap', 'дважды в одном массиве', 'без исхода']) {
    const c = fixtures.invalid.find((x) => x.note.includes(substr));
    assert(c, `нет invalid-фикстуры для "${substr}"`);
    assert(evaluate(c.registry).length > 0, `фикстура "${substr}" прошла чисто`);
  }
});

// ---------------------------------------------------------------------------
// свойство 4 — композиция с instruction-source-registry (реальный контракт)
// ---------------------------------------------------------------------------
check('обнаруженный источник без обязательного normative_status отклоняется реальной проверкой реестра источников', () => {
  const d = mutate((x) => { delete payload0(x).discovered_sources[0].payload.normative_status; });
  assert(evaluate(d).some((p) => /discovered source:/.test(p)), 'дефектный источник принят без замечания композиции');
});

check('agent-native источник с полной видимостью Meridian отклоняется реальной проверкой реестра источников', () => {
  const d = mutate((x) => {
    payload0(x).discovered_sources[0].payload.read_channel = { kind: 'agent-native', meridian_visibility: 'full', agent_auto_read: true };
  });
  assert(evaluate(d).some((p) => /meridian_visibility is "full"/.test(p)), 'agent-native с full-видимостью принят');
});

check('discovered_sources со статусом source-missing отклоняется — не тот массив', () => {
  const d = mutate((x) => { payload0(x).discovered_sources[0].payload.divergence = { status: 'source-missing' }; });
  assert(evaluate(d).some((p) => /belongs in missing_sources\/unreadable_sources/.test(p)), 'source-missing в discovered_sources принят');
});

check('initial-скан с divergence, отличным от unknown, отклоняется', () => {
  const d = mutate((x) => {
    payload0(x).discovered_sources[0].payload.divergence = {
      status: 'unchanged',
      previous_state: { revision: 'content-a1', digest: { algorithm: 'sha-256', value: DIG_A }, verified: true },
      current_state: { revision: 'content-a1', digest: { algorithm: 'sha-256', value: DIG_A }, verified: true },
    };
  });
  assert(evaluate(d).some((p) => /initial scan, which has no prior baseline/.test(p)), 'нерегламентированный divergence на initial принят');
});

check('discovered source, чьё расположение расходится со слотом плана, отклоняется', () => {
  const d = mutate((x) => { payload0(x).discovered_sources[0].payload.location.path = 'OTHER.md'; });
  assert(evaluate(d).some((p) => /location does not match its declared discovery_plan slot/.test(p)), 'расхождение расположения принято');
});

// ---------------------------------------------------------------------------
// свойство 5 — композиция с controlled-rule-intake; discovery не создаёт решений
// ---------------------------------------------------------------------------
const withCandidate = (overrides = {}, discoveryStatus = 'new') => mutate((d) => {
  payload0(d).rule_candidates = [{
    discovery_status: discoveryStatus,
    candidate: {
      $schema: '../scoped-record.schema.json',
      schema_version: 1,
      id: 'sample-inline-candidate',
      title: 'Кандидат',
      record_type: 'rule-candidate',
      scope: { type: 'repository-scope', id: 'sample-repository', workspace_id: 'sample-workspace' },
      origin: { kind: 'derived', source_ref: 'instruction-source:root-agents-md' },
      authority: { kind: 'delegated-run', authority_ref: 'controlled-rule-intake-run:x' },
      payload: {
        source_ref: {
          record_type: 'instruction-source', id: 'root-agents-md',
          reference: deriveSourceReference('root-agents-md', 'content-a1'),
          revision: 'content-a1', sha256: DIG_A,
        },
        boundary: { unit: 'agents-md-section', region: 'branch-naming' },
        raw_excerpt: 'Branches follow feature/<slug>.',
        normalized_text: 'branches follow feature slug',
        semantic_key: 'branch-naming-feature-slug',
        classification_basis: 'x',
        applicability_state: 'candidate',
        ...overrides,
      },
    },
  }];
});

check('документ с кандидатом без ruleIntakeSchema даёт явную проблему, а не проходит чисто', () => {
  const withCand = withCandidate({}, 'new');
  const p = evaluateExistingProjectCompatibilityMode(withCand, { ...opts, ruleIntakeSchema: undefined });
  assert(p.length > 0, 'документ с rule_candidates прошёл чисто без ruleIntakeSchema');
});

check('кандидат правила с посторонним полем payload при ruleIntakeSchema: {} всё равно отклоняется', () => {
  const withCand = withCandidate({ alien_field: 'unexpected' }, 'new');
  const p = evaluateExistingProjectCompatibilityMode(withCand, { ...opts, ruleIntakeSchema: {} });
  assert(p.length > 0, 'постороннее поле кандидата правила прошло чисто при ruleIntakeSchema: {}');
  assert(p.some((x) => /requires .*ruleIntakeSchema/.test(x)), 'ruleIntakeSchema: {} не отклонена как несостоятельная схема');
});

check('новый (discovery_status new) кандидат не может быть accepted', () => {
  const d = withCandidate({
    applicability_state: 'accepted',
    owner_decision: { decision_ref: 'owner-decision:x', decided_at: '2026-09-13', reason: 'x' },
  }, 'new');
  d.workspace_connections[0].payload.rule_candidates[0].candidate.authority = { kind: 'repository-maintainer', authority_ref: 'sample-repository-maintainer', decision_ref: 'owner-decision:x' };
  assert(evaluate(d).some((p) => /can never itself be accepted or not-applicable \(property 5\)/.test(p)), 'новый accepted-кандидат принят');
});

check('новый (discovery_status new) кандидат-candidate проходит честно', () => {
  const d = withCandidate({}, 'new');
  assert(evaluate(d).length === 0, `честный новый кандидат отклонён: ${evaluate(d)[0]}`);
});

check('carried-over кандидат с корректным решением владельца проходит', () => {
  const d = withCandidate({
    applicability_state: 'accepted',
    owner_decision: { decision_ref: 'owner-decision:x', decided_at: '2026-09-13', reason: 'x' },
  }, 'carried-over');
  d.workspace_connections[0].payload.rule_candidates[0].candidate.authority = { kind: 'repository-maintainer', authority_ref: 'sample-repository-maintainer', decision_ref: 'owner-decision:x' };
  assert(evaluate(d).length === 0, `честный carried-over кандидат отклонён: ${evaluate(d)[0]}`);
});

check('carried-over кандидат с delegated-run авторизацией accepted всё равно отклоняется (реальная проверка controlled-rule-intake)', () => {
  const d = withCandidate({
    applicability_state: 'accepted',
    owner_decision: { decision_ref: 'owner-decision:x', decided_at: '2026-09-13', reason: 'x' },
  }, 'carried-over');
  assert(evaluate(d).some((p) => /authorized by "delegated-run"/.test(p)), 'delegated-run авторизация accepted принята');
});

check('кандидат с неизвестным source_ref.id (не входит в discovered_sources) отклоняется', () => {
  const d = withCandidate({ source_ref: { record_type: 'instruction-source', id: 'unknown-source', reference: deriveSourceReference('unknown-source', 'content-a1'), revision: 'content-a1', sha256: DIG_A } }, 'new');
  assert(evaluate(d).some((p) => /does not resolve to a registered instruction source/.test(p)), 'кандидат с неизвестным источником принят');
});

check('кандидат, чей source_ref пинует устаревшую редакцию, отклоняется', () => {
  const d = withCandidate({ source_ref: { record_type: 'instruction-source', id: 'root-agents-md', reference: deriveSourceReference('root-agents-md', 'content-old'), revision: 'content-old', sha256: DIG_A } }, 'new');
  assert(evaluate(d).some((p) => /the pinned edition has changed/.test(p)), 'устаревший пин принят');
});

check('buildSourceResolver разрешает известный источник и отказывает неизвестному', () => {
  const discoveredById = new Map([['root-agents-md', payload0(BASE).discovered_sources[0]]]);
  const resolver = buildSourceResolver(discoveredById);
  assert(resolver({ id: 'root-agents-md' }) !== null, 'известный источник не разрешён');
  assert(resolver({ id: 'nowhere' }) === null, 'неизвестный источник разрешён');
});

// ---------------------------------------------------------------------------
// свойство 7 — изменение всегда создаёт finding, никогда не переписывает молча
// ---------------------------------------------------------------------------
check('changed без finding отклоняется', () => {
  const changed = fixtures.invalid.find((c) => c.note.includes('молча заменяющее'));
  assert(evaluate(changed.registry).some((p) => /no matching "source-changed" finding/.test(p)), 'молчаливая замена принята');
});

check('previously_known missing без finding отклоняется', () => {
  const d = mutate((x) => {
    payload0(x).scan_kind = 'rescan';
    payload0(x).discovered_sources = [];
    payload0(x).missing_sources = [{ plan_id: 'root-agents-md', previously_known: true, id: 'root-agents-md' }];
  });
  assert(evaluate(d).some((p) => /previously_known with no matching "source-missing" finding/.test(p)), 'пропажа без finding принята');
});

check('unreadable без finding отклоняется', () => {
  const d = mutate((x) => {
    payload0(x).discovered_sources = [];
    payload0(x).unreadable_sources = [{ plan_id: 'root-agents-md', reason: 'x', previously_known: false }];
  });
  assert(evaluate(d).some((p) => /has no matching "source-unreadable" finding/.test(p)), 'нечитаемость без finding принята');
});

check('initial-скан не может заявить previously_known: true', () => {
  const d = mutate((x) => {
    payload0(x).discovered_sources = [];
    payload0(x).missing_sources = [{ plan_id: 'root-agents-md', previously_known: true, id: 'root-agents-md' }];
    payload0(x).findings = [{ id: 'f1', kind: 'source-missing', detail: 'x', plan_id: 'root-agents-md', blocking: true }];
    payload0(x).next_step = 'await-owner-decision';
  });
  assert(evaluate(d).some((p) => /initial scan, which has no prior scan to know from/.test(p)), 'previously_known на initial принят');
});

// ---------------------------------------------------------------------------
// next_step — детерминированная функция состояния findings с одним закрытым
// приоритетом (computeNextStep): conflict > ambiguous-scope > любая другая
// блокирующая находка > ничего не блокирует.
// ---------------------------------------------------------------------------
const f = (kind, blocking) => ({ id: `f-${kind}-${blocking}`, kind, detail: 'x', blocking });

check('computeNextStep: пустой набор находок → continue-compatibility-mode', () => {
  assert(computeNextStep([]) === 'continue-compatibility-mode', 'пустой набор не даёт continue');
});

check('computeNextStep: небlocking находки любого рода не меняют continue-compatibility-mode', () => {
  const all = FINDING_KINDS.map((k) => f(k, false));
  assert(computeNextStep(all) === 'continue-compatibility-mode', 'небlocking находки дали не continue');
});

check('computeNextStep: блокирующий conflict принимает только resolve-conflict', () => {
  assert(computeNextStep([f('conflict', true)]) === 'resolve-conflict', 'блокирующий conflict не дал resolve-conflict');
});

check('computeNextStep: блокирующий ambiguous-scope принимает только resolve-ambiguity', () => {
  assert(computeNextStep([f('ambiguous-scope', true)]) === 'resolve-ambiguity', 'блокирующий ambiguous-scope не дал resolve-ambiguity');
});

check('computeNextStep: любая другая блокирующая находка принимает только await-owner-decision', () => {
  for (const kind of ['unresolved-decision', 'source-changed', 'source-missing', 'source-unreadable', 'other']) {
    assert(computeNextStep([f(kind, true)]) === 'await-owner-decision', `блокирующий "${kind}" не дал await-owner-decision`);
  }
});

check('computeNextStep: смешанный набор следует объявленному приоритету', () => {
  const mixed = [f('other', true), f('ambiguous-scope', true), f('conflict', true), f('source-missing', true)];
  assert(computeNextStep(mixed) === 'resolve-conflict', 'conflict не выиграл приоритет над остальными');

  const noConflict = [f('other', true), f('ambiguous-scope', true), f('source-missing', true)];
  assert(computeNextStep(noConflict) === 'resolve-ambiguity', 'ambiguous-scope не выиграл приоритет без conflict');

  const onlyOthers = [f('other', true), f('source-missing', true), f('unresolved-decision', true)];
  assert(computeNextStep(onlyOthers) === 'await-owner-decision', 'прочие блокирующие находки не дали await-owner-decision');
});

check('computeNextStep: перестановка массива не меняет вычисленный next_step', () => {
  const mixed = [f('other', true), f('ambiguous-scope', true), f('conflict', true), f('source-missing', false)];
  const expected = computeNextStep(mixed);
  for (let i = 0; i < 5; i += 1) {
    const shuffled = [...mixed].sort(() => Math.random() - 0.5);
    assert(computeNextStep(shuffled) === expected, 'перестановка findings изменила next_step');
  }
});

check('одинаковый набор findings принимает ровно одно значение next_step независимо от заявленного значения', () => {
  const mixed = [f('other', true), f('conflict', true)];
  for (const declared of NEXT_STEPS) {
    const d = mutate((x) => { payload0(x).findings = mixed; payload0(x).next_step = declared; });
    const problems = evaluate(d);
    if (declared === 'resolve-conflict') {
      assert(problems.length === 0, `корректный next_step "${declared}" отклонён: ${problems[0]}`);
    } else {
      assert(problems.some((p) => /computes "resolve-conflict"/.test(p)), `неверный next_step "${declared}" принят`);
    }
  }
});

check('continue-compatibility-mode с блокирующей находкой отклоняется композицией', () => {
  const d = mutate((x) => {
    payload0(x).findings = [{ id: 'f1', kind: 'other', detail: 'x', blocking: true }];
  });
  assert(evaluate(d).some((p) => /computes "await-owner-decision"/.test(p)), 'continue с блокирующей находкой принят');
});

check('await-owner-decision без блокирующей находки отклоняется композицией', () => {
  const d = mutate((x) => { payload0(x).next_step = 'await-owner-decision'; });
  assert(evaluate(d).some((p) => /computes "continue-compatibility-mode"/.test(p)), 'await-owner-decision без блокирующей находки принят');
});

check('ambiguous-scope блокирующая находка требует именно resolve-ambiguity, не await-owner-decision', () => {
  const d = mutate((x) => {
    payload0(x).findings = [{ id: 'f1', kind: 'ambiguous-scope', detail: 'x', blocking: true }];
    payload0(x).next_step = 'await-owner-decision';
  });
  assert(evaluate(d).some((p) => /computes "resolve-ambiguity"/.test(p)), 'ambiguous-scope принял await-owner-decision');
});

check('conflict блокирующая находка требует именно resolve-conflict, не resolve-ambiguity', () => {
  const d = mutate((x) => {
    payload0(x).findings = [{ id: 'f1', kind: 'conflict', detail: 'x', blocking: true }];
    payload0(x).next_step = 'resolve-ambiguity';
  });
  assert(evaluate(d).some((p) => /computes "resolve-conflict"/.test(p)), 'conflict принял resolve-ambiguity');
});

// ---------------------------------------------------------------------------
// область записи сканирования и точная область workspace/repository
// ---------------------------------------------------------------------------
check('scope.type вне project-workspace/repository-scope отклоняется', () => {
  const d = mutate((x) => { entry0(x).scope = { type: 'organization-profile', id: 'sample-org' }; });
  assert(evaluate(d).some((p) => /scoped to project-workspace or repository-scope only/.test(p)), 'недопустимая область принята');
});

check('origin.kind, отличный от declared, отклоняется', () => {
  const d = mutate((x) => { entry0(x).origin = { kind: 'derived', source_ref: 'x' }; });
  assert(evaluate(d).some((p) => new RegExp(`origin\\.kind is "derived", not "${REQUIRED_ORIGIN_KIND}"`).test(p)), 'недопустимый origin.kind принят');
});

check('authority.kind, отличный от delegated-run, отклоняется на самой записи сканирования', () => {
  const d = mutate((x) => { entry0(x).authority = { kind: 'project-owner', authority_ref: 'workspace-owner' }; });
  assert(evaluate(d).some((p) => new RegExp(`authority\\.kind is "project-owner", not "${REQUIRED_AUTHORITY_KIND}"`).test(p)), 'недопустимый authority.kind сканирования принят');
});

check('repository.workspace_id, расходящийся со scope.workspace_id, отклоняется (repository-scope)', () => {
  const d = mutate((x) => { payload0(x).repository.workspace_id = 'other-workspace'; });
  assert(evaluate(d).some((p) => /workspace_id "other-workspace" does not match scope\.workspace_id/.test(p)), 'расхождение workspace_id принято');
});

check('repository.id, расходящийся со scope.id, отклоняется (repository-scope)', () => {
  const d = mutate((x) => { payload0(x).repository.id = 'other-repository'; });
  assert(evaluate(d).some((p) => /repository\.id "other-repository" does not match scope\.id/.test(p)), 'расхождение repository.id принято');
});

check('repository.workspace_id, расходящийся со scope.id (project-workspace), отклоняется', () => {
  const d = mutate((x) => {
    entry0(x).scope = { type: 'project-workspace', id: 'sample-workspace' };
    payload0(x).repository = { id: 'sample-repository', workspace_id: 'other-workspace' };
  });
  assert(evaluate(d).some((p) => /workspace_id "other-workspace" does not match scope\.id/.test(p)), 'расхождение project-workspace scope принято');
});

// ---------------------------------------------------------------------------
// детерминизм: порядок элементов не влияет на результат
// ---------------------------------------------------------------------------
check('перестановка discovered_sources/rule_candidates/findings не меняет результат честного документа', () => {
  const dupCase = fixtures.valid.find((c) => c.note.includes('дубли') || c.note.includes('одинаковых'));
  const shuffled = clone(dupCase.registry);
  shuffled.workspace_connections[0].payload.rule_candidates.reverse();
  assert(evaluate(shuffled).length === 0, 'перестановка кандидатов изменила честный результат');
});

check('перестановка конфликтующих кандидатов не меняет отклонение', () => {
  const conflictCase = fixtures.invalid.find((c) => c.note.includes('порядку'));
  const shuffled = clone(conflictCase.registry);
  shuffled.workspace_connections[0].payload.rule_candidates.reverse();
  assert(evaluate(shuffled).length > 0, 'перестановка конфликта дала честный результат');
});

// ---------------------------------------------------------------------------
// закрытые пулы схемы и библиотеки не разошлись
// ---------------------------------------------------------------------------
check('закрытые пулы схемы и библиотеки совпадают', () => {
  const p = registrySchema.definitions.payload.properties;
  assert(JSON.stringify(p.connection_mode.enum) === JSON.stringify(CONNECTION_MODES), 'пул connection_mode разошёлся');
  assert(JSON.stringify(p.scan_kind.enum) === JSON.stringify(SCAN_KINDS), 'пул scan_kind разошёлся');
  assert(JSON.stringify(p.next_step.enum) === JSON.stringify(NEXT_STEPS), 'пул next_step разошёлся');
  assert(JSON.stringify(registrySchema.definitions.finding.properties.kind.enum) === JSON.stringify(FINDING_KINDS), 'пул finding.kind разошёлся');
  assert(JSON.stringify(registrySchema.definitions.rule_candidate_entry.properties.discovery_status.enum) === JSON.stringify(DISCOVERY_STATUSES), 'пул discovery_status разошёлся');
  assert(JSON.stringify(registrySchema.definitions.plan_slot.properties.medium.enum) === JSON.stringify(MEDIA), 'пул medium разошёлся');
  assert(registrySchema.definitions.entry.properties.record_type.const === RECORD_TYPE, 'record_type разошёлся');
});

check('неподдерживаемое ключевое слово схемы отклоняется движком', () => {
  const broken = clone(registrySchema);
  broken.definitions.payload.patternProperties = { '^x': { type: 'string' } };
  let threw = false;
  try { assertSupportedDeep(broken, 'broken'); } catch { threw = true; }
  assert(threw, 'assertSupportedDeep пропустил неподдерживаемое ключевое слово');
});

check('ALLOWED_SCOPE_TYPES закрыт к двум областям', () => {
  assert(JSON.stringify(ALLOWED_SCOPE_TYPES) === JSON.stringify(['project-workspace', 'repository-scope']), 'ALLOWED_SCOPE_TYPES разошёлся');
});

console.log(`\n${passed} passed, ${failures.length} failed`);
if (failures.length) process.exit(1);
