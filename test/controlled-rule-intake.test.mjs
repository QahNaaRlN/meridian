#!/usr/bin/env node
// Standalone verification for the controlled rule intake contract
// (registries/operating-model/controlled-rule-intake.schema.json).
//
// It calls the same implementation the gate calls
// (scripts/lib/controlled-rule-intake.mjs) so the two cannot drift: the
// record envelope against the existing scoped-record.schema.json, the
// candidate payload against the specialised controlled-rule-intake.schema.json,
// and the rules JSON Schema cannot state — source_ref resolution through the
// external boundary, boundary range sanity, semantic-key cluster coherence,
// the symmetric order-independent conflict graph, and the closed
// applicability-state / not-applicable-reason evidence rules.
//
// Usage: node test/controlled-rule-intake.test.mjs

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { assertSupportedDeep } from '../scripts/lib/json-schema.mjs';
import {
  evaluateControlledRuleIntake,
  resolveAndCheckSourceRef,
  checkBoundary,
  checkOriginLink,
  checkOwnerAuthority,
  makeRecordResolver,
  RECORD_TYPE, SOURCE_RECORD_TYPE, BOUNDARY_UNITS, APPLICABILITY_STATES, NOT_APPLICABLE_REASONS,
  REQUIRED_ORIGIN_KIND, HUMAN_OWNER_AUTHORITY_BY_SCOPE, RESOLVED_READ_CHANNEL_KEYS, DIGEST_KEYS,
  canonicalOriginSourceRef,
  READ_CHANNEL_KINDS, MERIDIAN_VISIBILITY, CURRENCY,
} from '../scripts/lib/controlled-rule-intake.mjs';

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

const registrySchema = loadJson('registries/operating-model/controlled-rule-intake.schema.json');
const envelopeSchema = loadJson('registries/operating-model/scoped-record.schema.json');
const fixtures = loadJson('registries/operating-model/fixtures/controlled-rule-intake.fixtures.json');

const resolveSource = makeRecordResolver(fixtures.resolution);
const opts = { registrySchema, envelopeSchema, resolveSource };
const evaluate = (doc) => evaluateControlledRuleIntake(doc, opts);
const clone = (x) => JSON.parse(JSON.stringify(x));

const DIG_A = 'aaaa1111'.repeat(8);
const DIG_B = 'bbbb2222'.repeat(8);
const REF_A = 'instruction-source-registry:sample-repo-agents-md@rev-000001';
const REF_B = 'instruction-source-registry:sample-repo-claude-md@rev-000002';

// A topologically complete valid single-candidate document, used as the base
// for targeted mutations.
const BASE = {
  $schema: '../../registries/operating-model/controlled-rule-intake.schema.json',
  schema_version: 1,
  registry_id: 'controlled-rule-intake',
  title: 'Пример реестра приёма правил (тест)',
  rule_candidates: [
    {
      $schema: '../../registries/operating-model/scoped-record.schema.json',
      schema_version: 1,
      id: 'sample-candidate',
      title: 'Кандидат',
      record_type: 'rule-candidate',
      scope: { type: 'repository-scope', id: 'sample-repository', workspace_id: 'sample-workspace' },
      origin: { kind: 'derived', source_ref: 'instruction-source:sample-repo-agents-md' },
      authority: { kind: 'delegated-run', authority_ref: 'controlled-rule-intake-run:test' },
      payload: {
        source_ref: { record_type: 'instruction-source', id: 'sample-repo-agents-md', reference: REF_A, revision: 'rev-000001', sha256: DIG_A },
        boundary: { unit: 'agents-md-section', region: 'sample-region' },
        raw_excerpt: 'Do the thing.',
        normalized_text: 'do the thing',
        semantic_key: 'do-the-thing',
        classification_basis: 'участок объявлен владельцем репозитория',
        applicability_state: 'candidate',
      },
    },
  ],
};
const entry0 = (d) => d.rule_candidates[0];
const payload0 = (d) => entry0(d).payload;
const mutate = (fn) => { const d = clone(BASE); fn(d); return d; };

// ---------------------------------------------------------------------------
// schema + real fixtures
// ---------------------------------------------------------------------------
check('обе схемы используют поддерживаемое подмножество JSON Schema', () => {
  assertSupportedDeep(registrySchema, 'controlled-rule-intake.schema.json');
  assertSupportedDeep(envelopeSchema, 'scoped-record.schema.json');
});

check('специализированная схема — не второй конверт записи', () => {
  assert(registrySchema.properties.registry_id.const === 'controlled-rule-intake', 'registry_id не закреплён');
  assert(registrySchema.definitions.entry.properties.record_type.const === 'rule-candidate',
    'record_type записи не закреплён как rule-candidate');
});

check('схема не содержит абсолютных машинных путей и продуктовых сведений', () => {
  const text = JSON.stringify(registrySchema);
  assert(!/\/home\/|\/Users\/|[A-Za-z]:\\\\/.test(text), 'в схеме найден абсолютный машинный путь');
  assert(!/MERIDIAN_INSTANCE/.test(text), 'схема ссылается на обязательный отдельный Экземпляр');
});

check('базовый документ проходит композитную проверку без замечаний', () => {
  const problems = evaluate(BASE);
  assert(problems.length === 0, problems[0]);
});

check('пустой реестр кандидатов допустим', () => {
  assert(evaluate({ ...BASE, rule_candidates: [] }).length === 0, 'пустой реестр отклонён');
});

check('бандл фикстур: каждая valid проходит, каждая invalid отклоняется', () => {
  assert(Array.isArray(fixtures.valid) && fixtures.valid.length >= 2, 'нет минимум двух положительных примеров');
  assert(Array.isArray(fixtures.invalid) && fixtures.invalid.length > 0, 'нет отрицательных примеров');
  for (const c of fixtures.valid) {
    const problems = evaluate(c.registry);
    assert(problems.length === 0, `valid-фикстура отклонена (${c.note}): ${problems[0]}`);
  }
  for (const c of fixtures.invalid) {
    assert(evaluate(c.registry).length > 0, `invalid-фикстура прошла чисто (${c.note})`);
  }
});

// ---------------------------------------------------------------------------
// свойство 1 / 8 — кандидат недоверен; закрытый минимальный набор состояний
// ---------------------------------------------------------------------------
check('candidate не несёт owner_decision или not_applicable_reason', () => {
  assert(evaluate(mutate((d) => {
    payload0(d).owner_decision = { decision_ref: 'x', decided_at: '2026-09-13', reason: 'x' };
  })).length > 0, 'candidate с owner_decision принят');
  assert(evaluate(mutate((d) => { payload0(d).not_applicable_reason = 'owner-rejected'; })).length > 0,
    'candidate с not_applicable_reason принят');
});

check('accepted и not-applicable требуют owner_decision', () => {
  assert(evaluate(mutate((d) => { payload0(d).applicability_state = 'accepted'; })).length > 0,
    'accepted без owner_decision принят');
  assert(evaluate(mutate((d) => { payload0(d).applicability_state = 'not-applicable'; })).length > 0,
    'not-applicable без owner_decision принят');
});

check('not-applicable требует not_applicable_reason; accepted его не несёт', () => {
  const withDecision = (d) => {
    entry0(d).authority = { kind: 'repository-maintainer', authority_ref: 'sample-repository-maintainer', decision_ref: 'owner-decision:x' };
    payload0(d).owner_decision = { decision_ref: 'owner-decision:x', decided_at: '2026-09-13', reason: 'x' };
  };
  assert(evaluate(mutate((d) => { payload0(d).applicability_state = 'not-applicable'; withDecision(d); })).length > 0,
    'not-applicable без not_applicable_reason принят');
  assert(evaluate(mutate((d) => {
    payload0(d).applicability_state = 'accepted'; withDecision(d); payload0(d).not_applicable_reason = 'owner-rejected';
  })).length > 0, 'accepted с not_applicable_reason принят');
  const ok = mutate((d) => {
    payload0(d).applicability_state = 'not-applicable'; withDecision(d); payload0(d).not_applicable_reason = 'owner-rejected';
  });
  assert(evaluate(ok).length === 0, `честное not-applicable отклонено: ${evaluate(ok)[0]}`);
});

check('applicability_state и not_applicable_reason — закрытые пулы', () => {
  assert(evaluate(mutate((d) => { payload0(d).applicability_state = 'rejected'; })).length > 0, 'неизвестное состояние принято');
  assert(APPLICABILITY_STATES.length === 3, 'закрытый набор применимости — не три значения');
  assert(NOT_APPLICABLE_REASONS.length === 2, 'закрытый набор причин — не два значения');
});

// ---------------------------------------------------------------------------
// свойство 2 — закрытая точная редакция источника, разрешаемая через границу
// ---------------------------------------------------------------------------
check('неизвестный источник отклоняется резолвером', () => {
  assert(evaluate(mutate((d) => { payload0(d).source_ref.reference = 'instruction-source-registry:nowhere@rev-1'; })).length > 0,
    'неизвестная ссылка на источник принята');
});

check('без резолвера контракт фейлится закрыто', () => {
  const bare = evaluateControlledRuleIntake(BASE, { registrySchema, envelopeSchema });
  assert(bare.length > 0, 'документ без резолвера прошёл чисто');
});

check('непроверенная резолвером редакция отклоняется', () => {
  const d = mutate((x) => {
    payload0(x).source_ref = { record_type: 'instruction-source', id: 'sample-repo-unverified', reference: 'instruction-source-registry:sample-repo-unverified@rev-000009', revision: 'rev-000009', sha256: 'dddd4444'.repeat(8) };
  });
  assert(evaluate(d).some((p) => /revision_verified is not true/.test(p)), `не поймано: ${evaluate(d)[0]}`);
});

check('изменившаяся редакция источника отклоняется (revision и digest порознь)', () => {
  const movedRevision = mutate((x) => {
    payload0(x).source_ref = { record_type: 'instruction-source', id: 'sample-repo-moved', reference: 'instruction-source-registry:sample-repo-moved@rev-000010', revision: 'rev-000010', sha256: DIG_A };
  });
  assert(evaluate(movedRevision).some((p) => /pinned edition has changed/.test(p)), `revision-расхождение не поймано: ${evaluate(movedRevision)[0]}`);

  const rehashed = mutate((x) => {
    payload0(x).source_ref = { record_type: 'instruction-source', id: 'sample-repo-rehashed', reference: 'instruction-source-registry:sample-repo-rehashed@rev-000012', revision: 'rev-000012', sha256: DIG_A };
  });
  assert(evaluate(rehashed).some((p) => /pinned edition has changed/.test(p)), `digest-расхождение не поймано: ${evaluate(rehashed)[0]}`);
});

// ---------------------------------------------------------------------------
// свойство 2/4 — временная актуальность (currency) снимка источника
// ---------------------------------------------------------------------------
check('unverified currency отклоняется независимо от applicability_state', () => {
  const d = mutate((x) => {
    payload0(x).source_ref = { record_type: 'instruction-source', id: 'sample-repo-unverified', reference: 'instruction-source-registry:sample-repo-unverified@rev-000009', revision: 'rev-000009', sha256: 'dddd4444'.repeat(8) };
  });
  assert(evaluate(d).some((p) => /recorded_state\.currency is "unverified"/.test(p)), 'unverified currency принята');
});

check('accepted опирается на stale currency — отклоняется (проверенное прошлое не считается текущим)', () => {
  const d = mutate((x) => {
    entry0(x).origin = { kind: 'derived', source_ref: 'instruction-source:sample-repo-stale' };
    entry0(x).authority = { kind: 'repository-maintainer', authority_ref: 'sample-repository-maintainer', decision_ref: 'owner-decision:x' };
    payload0(x).source_ref = { record_type: 'instruction-source', id: 'sample-repo-stale', reference: 'instruction-source-registry:sample-repo-stale@rev-000013', revision: 'rev-000013', sha256: 'eeee5555'.repeat(8) };
    payload0(x).applicability_state = 'accepted';
    payload0(x).owner_decision = { decision_ref: 'owner-decision:x', decided_at: '2026-09-13', reason: 'x' };
  });
  assert(evaluate(d).some((p) => /currency is "stale"/.test(p)), 'accepted из stale-снимка принят');
});

check('not-applicable опирается на stale currency — отклоняется', () => {
  const d = mutate((x) => {
    entry0(x).origin = { kind: 'derived', source_ref: 'instruction-source:sample-repo-stale' };
    entry0(x).authority = { kind: 'repository-maintainer', authority_ref: 'sample-repository-maintainer', decision_ref: 'owner-decision:x' };
    payload0(x).source_ref = { record_type: 'instruction-source', id: 'sample-repo-stale', reference: 'instruction-source-registry:sample-repo-stale@rev-000013', revision: 'rev-000013', sha256: 'eeee5555'.repeat(8) };
    payload0(x).applicability_state = 'not-applicable';
    payload0(x).not_applicable_reason = 'owner-rejected';
    payload0(x).owner_decision = { decision_ref: 'owner-decision:x', decided_at: '2026-09-13', reason: 'x' };
  });
  assert(evaluate(d).some((p) => /currency is "stale"/.test(p)), 'not-applicable из stale-снимка принят');
});

check('candidate вправе ссылаться на stale-снимок как на историческую находку', () => {
  const d = mutate((x) => {
    entry0(x).origin = { kind: 'derived', source_ref: 'instruction-source:sample-repo-stale' };
    payload0(x).source_ref = { record_type: 'instruction-source', id: 'sample-repo-stale', reference: 'instruction-source-registry:sample-repo-stale@rev-000013', revision: 'rev-000013', sha256: 'eeee5555'.repeat(8) };
  });
  assert(evaluate(d).length === 0, `candidate из stale-снимка отклонён: ${evaluate(d)[0]}`);
});

check('resolveAndCheckSourceRef — закрытый ответ резолвера', () => {
  const problems = [];
  const badKeyResolver = () => ({ record_type: SOURCE_RECORD_TYPE, id: 'sample-repo-agents-md', reference: REF_A, recorded_state: { revision: 'rev-000001', digest: { algorithm: 'sha-256', value: DIG_A }, revision_verified: true, currency: 'current' }, extra: 1 });
  resolveAndCheckSourceRef(problems, 'x', { record_type: 'instruction-source', id: 'sample-repo-agents-md', reference: REF_A, revision: 'rev-000001', sha256: DIG_A }, badKeyResolver);
  assert(problems.some((p) => /unknown field "extra"/.test(p)), 'лишнее поле ответа резолвера принято');
});

check('resolveAndCheckSourceRef — record_type ответа сверяется', () => {
  const problems = [];
  const wrongType = () => ({ record_type: 'norm', id: 'sample-repo-agents-md', recorded_state: { revision: 'rev-000001', digest: { algorithm: 'sha-256', value: DIG_A }, revision_verified: true, currency: 'current' } });
  resolveAndCheckSourceRef(problems, 'x', { record_type: 'instruction-source', id: 'sample-repo-agents-md', reference: REF_A, revision: 'rev-000001', sha256: DIG_A }, wrongType);
  assert(problems.some((p) => /resolves to a "norm" record/.test(p)), 'резолвер, вернувший чужой record_type, принят');
});

// ---------------------------------------------------------------------------
// свойство 10 — read_channel разрешённого источника проверяем и не
// переинтерпретируется приёмкой (agent-native остаётся отдельным каналом)
// ---------------------------------------------------------------------------
const AGENT_NATIVE_REF = 'instruction-source-registry:sample-repo-agent-native@rev-000014';
const AGENT_NATIVE_FULL_REF = 'instruction-source-registry:sample-repo-agent-native-full-claim@rev-000015';
const DIG_AGENT_NATIVE = 'ffff6666'.repeat(8);
const DIG_AGENT_NATIVE_FULL = '11117777'.repeat(8);

check('RESOLVED_READ_CHANNEL_KEYS — закрытые три поля', () => {
  assert(JSON.stringify(RESOLVED_READ_CHANNEL_KEYS) === JSON.stringify(['kind', 'meridian_visibility', 'agent_auto_read']),
    'закрытый набор полей read_channel разошёлся');
});

check('resolveAndCheckSourceRef — read_channel обязателен в закрытом ответе резолвера', () => {
  const problems = [];
  const noReadChannel = () => ({ record_type: SOURCE_RECORD_TYPE, id: 'sample-repo-agents-md', reference: REF_A, recorded_state: { revision: 'rev-000001', digest: { algorithm: 'sha-256', value: DIG_A }, revision_verified: true, currency: 'current' } });
  resolveAndCheckSourceRef(problems, 'x', { record_type: 'instruction-source', id: 'sample-repo-agents-md', reference: REF_A, revision: 'rev-000001', sha256: DIG_A }, noReadChannel);
  assert(problems.some((p) => /carries no read_channel/.test(p)), 'отсутствие read_channel в ответе резолвера принято');
});

check('принятие кандидата из корректного agent-native источника с partial-видимостью проходит', () => {
  const d = mutate((x) => {
    entry0(x).origin = { kind: 'derived', source_ref: 'instruction-source:sample-repo-agent-native' };
    entry0(x).authority = { kind: 'repository-maintainer', authority_ref: 'sample-repository-maintainer', decision_ref: 'owner-decision:agent-native' };
    payload0(x).source_ref = { record_type: 'instruction-source', id: 'sample-repo-agent-native', reference: AGENT_NATIVE_REF, revision: 'rev-000014', sha256: DIG_AGENT_NATIVE };
    payload0(x).applicability_state = 'accepted';
    payload0(x).owner_decision = { decision_ref: 'owner-decision:agent-native', decided_at: '2026-09-13', reason: 'владелец согласился на содержимое, которое инструмент агента читает сам' };
  });
  assert(evaluate(d).length === 0, `честный agent-native кандидат отклонён: ${evaluate(d)[0]}`);
});

check('резолвер, заявляющий полный контроль над agent-native каналом, отклоняется', () => {
  const d = mutate((x) => {
    entry0(x).origin = { kind: 'derived', source_ref: 'instruction-source:sample-repo-agent-native-full-claim' };
    payload0(x).source_ref = { record_type: 'instruction-source', id: 'sample-repo-agent-native-full-claim', reference: AGENT_NATIVE_FULL_REF, revision: 'rev-000015', sha256: DIG_AGENT_NATIVE_FULL };
  });
  assert(evaluate(d).some((p) => /meridian_visibility is "full"/.test(p)), 'agent-native с заявленной full-видимостью принят');
});

check('read_channel сохраняется резолвером без переинтерпретации', () => {
  const problems = [];
  const rc = { kind: 'agent-native', meridian_visibility: 'partial', agent_auto_read: true };
  const resolver = () => ({
    record_type: SOURCE_RECORD_TYPE, id: 'sample-repo-agent-native', reference: AGENT_NATIVE_REF,
    recorded_state: { revision: 'rev-000014', digest: { algorithm: 'sha-256', value: DIG_AGENT_NATIVE }, revision_verified: true, currency: 'current' },
    read_channel: rc,
  });
  const returned = resolveAndCheckSourceRef(
    problems, 'x',
    { record_type: 'instruction-source', id: 'sample-repo-agent-native', reference: AGENT_NATIVE_REF, revision: 'rev-000014', sha256: DIG_AGENT_NATIVE },
    resolver, 'accepted',
  );
  assert(problems.length === 0, `честный agent-native ответ отклонён: ${problems[0]}`);
  assert(returned.read_channel === rc, 'read_channel переписан или переинтерпретирован проверкой вместо буквального сохранения');
  assert(returned.read_channel.kind === 'agent-native' && returned.read_channel.meridian_visibility === 'partial',
    'read_channel не сохранил исходные значения kind/meridian_visibility');
});

// ---------------------------------------------------------------------------
// свойство 2/10 — закрытая ФОРМА разрешённого ответа резолвера, а не только
// согласованность его значений (checkReadChannel не ловит изобретённые или
// неверно типизированные значения сама по себе)
// ---------------------------------------------------------------------------
const validResolvedEntry = () => ({
  record_type: SOURCE_RECORD_TYPE,
  id: 'sample-repo-agents-md',
  reference: REF_A,
  recorded_state: { revision: 'rev-000001', digest: { algorithm: 'sha-256', value: DIG_A }, revision_verified: true, currency: 'current' },
  read_channel: { kind: 'meridian-observed', meridian_visibility: 'full', agent_auto_read: false },
});
const checkResolvedForm = (mutate) => {
  const entry = validResolvedEntry();
  mutate(entry);
  const problems = [];
  resolveAndCheckSourceRef(
    problems, 'x',
    { record_type: 'instruction-source', id: 'sample-repo-agents-md', reference: REF_A, revision: 'rev-000001', sha256: DIG_A },
    () => entry, 'candidate',
  );
  return problems;
};

check('READ_CHANNEL_KINDS/MERIDIAN_VISIBILITY/CURRENCY переиспользованы, не продублированы', () => {
  assert(Array.isArray(READ_CHANNEL_KINDS) && READ_CHANNEL_KINDS.length === 3, 'READ_CHANNEL_KINDS не переиспользован');
  assert(Array.isArray(MERIDIAN_VISIBILITY) && MERIDIAN_VISIBILITY.length === 3, 'MERIDIAN_VISIBILITY не переиспользован');
  assert(Array.isArray(CURRENCY) && CURRENCY.length === 3, 'CURRENCY не переиспользован');
  assert(JSON.stringify(DIGEST_KEYS) === JSON.stringify(['algorithm', 'value']), 'DIGEST_KEYS разошёлся');
});

check('честный ответ резолвера проходит все проверки формы чисто', () => {
  const problems = checkResolvedForm(() => {});
  assert(problems.length === 0, `честный ответ отклонён: ${problems[0]}`);
});

check('read_channel.kind вне закрытого пула отклоняется', () => {
  const problems = checkResolvedForm((e) => { e.read_channel.kind = 'invented-channel'; });
  assert(problems.some((p) => /read_channel\.kind "invented-channel" is not one of/.test(p)), 'изобретённый read_channel.kind принят');
});

check('read_channel.meridian_visibility вне закрытого пула отклоняется', () => {
  const problems = checkResolvedForm((e) => { e.read_channel.meridian_visibility = 'invented-visibility'; });
  assert(problems.some((p) => /meridian_visibility "invented-visibility" is not one of/.test(p)), 'изобретённая meridian_visibility принята');
});

check('read_channel.agent_auto_read не boolean (строка) отклоняется', () => {
  const problems = checkResolvedForm((e) => { e.read_channel.agent_auto_read = 'false'; });
  assert(problems.some((p) => /agent_auto_read is "false", not a boolean/.test(p)), 'agent_auto_read строкой "false" принят');
});

check('recorded_state.currency вне закрытого пула отклоняется', () => {
  const problems = checkResolvedForm((e) => { e.recorded_state.currency = 'invented-currency'; });
  assert(problems.some((p) => /recorded_state\.currency "invented-currency" is not one of/.test(p)), 'изобретённая currency принята');
});

check('recorded_state.revision_verified не boolean (строка) отклоняется', () => {
  const problems = checkResolvedForm((e) => { e.recorded_state.revision_verified = 'true'; });
  assert(problems.some((p) => /revision_verified is "true", not a boolean/.test(p)), 'revision_verified строкой "true" принят');
});

check('отсутствующий reference в ответе резолвера отклоняется', () => {
  const problems = checkResolvedForm((e) => { delete e.reference; });
  assert(problems.some((p) => /resolver returned no reference/.test(p)), 'отсутствующий reference принят');
});

check('неверный тип recorded_state.revision отклоняется', () => {
  const problems = checkResolvedForm((e) => { e.recorded_state.revision = 12345; });
  assert(problems.some((p) => /recorded_state\.revision is not a non-empty string/.test(p)), 'revision неверного типа принят');
});

check('неполная форма digest (без value) отклоняется', () => {
  const problems = checkResolvedForm((e) => { delete e.recorded_state.digest.value; });
  assert(problems.some((p) => /digest is missing required field "value"/.test(p)), 'digest без value принят');
});

check('неверная форма digest.value (не 64 lowercase hex) отклоняется', () => {
  const problems = checkResolvedForm((e) => { e.recorded_state.digest.value = 'not-a-real-digest'; });
  assert(problems.some((p) => /digest\.value is not 64 lowercase hex characters/.test(p)), 'digest.value неверной формы принят');
});

check('лишнее поле digest отклоняется — закрыт ровно к algorithm/value', () => {
  const problems = checkResolvedForm((e) => { e.recorded_state.digest.extra = 1; });
  assert(problems.some((p) => /digest carries an unknown field "extra"/.test(p)), 'лишнее поле digest принято');
});

check('пустая recorded_state.revision отклоняется', () => {
  const problems = checkResolvedForm((e) => { e.recorded_state.revision = ''; });
  assert(problems.some((p) => /recorded_state\.revision is not a non-empty string/.test(p)), 'пустая revision принята');
});

check('digest без algorithm отклоняется — неполная форма', () => {
  const problems = checkResolvedForm((e) => { delete e.recorded_state.digest.algorithm; });
  assert(problems.some((p) => /digest is missing required field "algorithm"/.test(p)), 'digest без algorithm принят');
});

check('digest.algorithm, отличный от sha-256, отклоняется', () => {
  const problems = checkResolvedForm((e) => { e.recorded_state.digest.algorithm = 'md5'; });
  assert(problems.some((p) => /digest\.algorithm is not "sha-256"/.test(p)), 'digest.algorithm "md5" принят');
});

check('digest.value неправильной длины отклоняется', () => {
  const problems = checkResolvedForm((e) => { e.recorded_state.digest.value = 'aaaa1111'.repeat(7); });
  assert(problems.some((p) => /digest\.value is not 64 lowercase hex characters/.test(p)), 'digest.value неправильной длины принят');
});

check('digest.value с не-hex символами отклоняется', () => {
  const problems = checkResolvedForm((e) => { e.recorded_state.digest.value = 'zzzz1111'.repeat(8); });
  assert(problems.some((p) => /digest\.value is not 64 lowercase hex characters/.test(p)), 'digest.value с не-hex символами принят');
});

check('digest.value верхним регистром отклоняется', () => {
  const problems = checkResolvedForm((e) => { e.recorded_state.digest.value = 'AAAA1111'.repeat(8); });
  assert(problems.some((p) => /digest\.value is not 64 lowercase hex characters/.test(p)), 'digest.value верхним регистром принят');
});

check('reference, не совпадающий с payload.source_ref.reference, отклоняется', () => {
  const problems = checkResolvedForm((e) => { e.reference = 'instruction-source-registry:sample-repo-agents-md@rev-999999'; });
  assert(problems.some((p) => /stated reference .* is not the resolved reference/.test(p)), 'несовпадающий reference принят');
});

check('неизвестное верхнеуровневое поле ответа резолвера отклоняется', () => {
  const problems = checkResolvedForm((e) => { e.extra_top_level_field = true; });
  assert(problems.some((p) => /unknown field "extra_top_level_field"/.test(p)), 'неизвестное верхнеуровневое поле принято');
});

// ---------------------------------------------------------------------------
// свойство 3 — граница разбора
// ---------------------------------------------------------------------------
check('checkBoundary — прямые случаи', () => {
  assert(checkBoundary({ unit: 'line-range', start: 1, end: 5 }).ok === true, 'нормальный диапазон отклонён');
  assert(checkBoundary({ unit: 'whole-source' }).ok === true, 'источник целиком отклонён');
  assert(checkBoundary({ unit: 'line-range', start: 5, end: 5 }).ok === false, 'равные границы приняты');
  assert(checkBoundary({ unit: 'line-range', start: 9, end: 3 }).ok === false, 'перевёрнутый диапазон принят');
});

check('взаимоисключающие формы boundary проверяются схемой', () => {
  assert(evaluate(mutate((d) => { payload0(d).boundary = { unit: 'agents-md-section', region: 'x', start: 1, end: 2 }; })).length > 0,
    'участок со start/end принят');
  assert(evaluate(mutate((d) => { payload0(d).boundary = { unit: 'line-range', start: 1, end: 2, region: 'x' }; })).length > 0,
    'диапазон с region принят');
});

check('BOUNDARY_UNITS — закрытый пул', () => {
  assert(JSON.stringify(BOUNDARY_UNITS) === JSON.stringify(registrySchema.definitions.boundary.properties.unit.enum), 'пул boundary.unit разошёлся');
});

// ---------------------------------------------------------------------------
// свойство 4 — классификация из модели областей
// ---------------------------------------------------------------------------
check('scope вне модели шести областей отклоняется', () => {
  assert(evaluate(mutate((d) => { entry0(d).scope = { type: 'team-scope', id: 'x' }; })).length > 0, 'неизвестная область принята');
});

check('classification_basis обязателен', () => {
  assert(evaluate(mutate((d) => { delete payload0(d).classification_basis; })).length > 0, 'запись без classification_basis принята');
});

// ---------------------------------------------------------------------------
// свойство 5 — повторы по явному semantic_key, не по id или порядку
// ---------------------------------------------------------------------------
check('семантический кластер требует единой области, состояния и conflicts_with', () => {
  const d1 = mutate((x) => {}); // placeholder to keep structure symmetrical
  const withSecondOrigin = (mutator) => {
    const doc = clone(BASE);
    const second = clone(entry0(doc));
    second.id = 'sample-candidate-second';
    second.origin = { kind: 'derived', source_ref: 'instruction-source:sample-repo-claude-md' };
    second.payload.source_ref = { record_type: 'instruction-source', id: 'sample-repo-claude-md', reference: REF_B, revision: 'rev-000002', sha256: DIG_B };
    second.payload.boundary = { unit: 'whole-source' };
    mutator(second);
    doc.rule_candidates.push(second);
    return doc;
  };

  assert(evaluate(withSecondOrigin((s) => { s.scope = { type: 'user-profile', id: 'someone' }; }))
    .some((p) => /more than one scope/.test(p)), 'кластер с разной областью принят');

  assert(evaluate(withSecondOrigin((s) => {
    s.payload.applicability_state = 'accepted';
    s.authority = { kind: 'project-owner', authority_ref: 'workspace-owner', decision_ref: 'owner-decision:x' };
    s.payload.owner_decision = { decision_ref: 'owner-decision:x', decided_at: '2026-09-13', reason: 'x' };
  })).some((p) => /more than one applicability_state/.test(p)), 'кластер с разным состоянием принят');

  assert(evaluate(withSecondOrigin((s) => { s.payload.conflicts_with = ['something-else']; }))
    .some((p) => /inconsistent conflicts_with/.test(p)), 'кластер с разными conflicts_with принят');

  const okDoc = withSecondOrigin(() => {});
  assert(evaluate(okDoc).length === 0, `согласованный кластер отклонён: ${evaluate(okDoc)[0]}`);
  void d1;
});

check('повторённое происхождение внутри кластера отклоняется', () => {
  const doc = clone(BASE);
  const dup = clone(entry0(doc));
  dup.id = 'sample-candidate-duplicate-origin';
  doc.rule_candidates.push(dup);
  assert(evaluate(doc).some((p) => /same origin.*more than once/.test(p)), 'дублированное происхождение принято');
});

// ---------------------------------------------------------------------------
// свойство 5/9 — единое authority у решённого (accepted/not-applicable)
// кластера; совпадающий owner_decision не подменяет authority
// ---------------------------------------------------------------------------
const decidedClusterDoc = (authorityA, authorityB, state = 'accepted') => {
  const doc = clone(BASE);
  const decision = { decision_ref: 'owner-decision:cluster-shared', decided_at: '2026-09-13', reason: 'shared' };
  const first = entry0(doc);
  first.payload.applicability_state = state;
  if (state === 'not-applicable') first.payload.not_applicable_reason = 'owner-rejected';
  first.authority = authorityA;
  first.payload.owner_decision = { ...decision, decision_ref: authorityA.decision_ref };
  const second = clone(first);
  second.id = 'sample-cluster-authority-second';
  second.origin = { kind: 'derived', source_ref: 'instruction-source:sample-repo-claude-md' };
  second.payload = { ...second.payload };
  second.payload.source_ref = { record_type: 'instruction-source', id: 'sample-repo-claude-md', reference: REF_B, revision: 'rev-000002', sha256: DIG_B };
  second.payload.boundary = { unit: 'whole-source' };
  second.authority = authorityB;
  second.payload.owner_decision = { ...decision, decision_ref: authorityB.decision_ref };
  doc.rule_candidates.push(second);
  return doc;
};

check('решённый кластер с одинаковым owner_decision, но разными authority_ref, отклоняется', () => {
  const shared = 'owner-decision:cluster-shared-ref';
  const doc = decidedClusterDoc(
    { kind: 'repository-maintainer', authority_ref: 'sample-repository-maintainer', decision_ref: shared },
    { kind: 'repository-maintainer', authority_ref: 'sample-other-maintainer', decision_ref: shared },
  );
  assert(evaluate(doc).some((p) => /inconsistent authority/.test(p)), 'разные authority_ref внутри решённого кластера приняты');
});

check('решённый кластер с разными authority.kind отклоняется', () => {
  const shared = 'owner-decision:cluster-shared-kind';
  const doc = decidedClusterDoc(
    { kind: 'repository-maintainer', authority_ref: 'sample-repository-maintainer', decision_ref: shared },
    { kind: 'project-owner', authority_ref: 'sample-repository-maintainer', decision_ref: shared },
  );
  assert(evaluate(doc).some((p) => /inconsistent authority/.test(p)), 'разные authority.kind внутри решённого кластера приняты');
});

check('решённый кластер с разными authority.decision_ref отклоняется', () => {
  const doc = decidedClusterDoc(
    { kind: 'repository-maintainer', authority_ref: 'sample-repository-maintainer', decision_ref: 'owner-decision:cluster-decision-a' },
    { kind: 'repository-maintainer', authority_ref: 'sample-repository-maintainer', decision_ref: 'owner-decision:cluster-decision-b' },
  );
  assert(evaluate(doc).some((p) => /inconsistent authority/.test(p)), 'разные authority.decision_ref внутри решённого кластера приняты');
});

check('честный решённый кластер с одинаковым authority (kind/authority_ref/decision_ref) проходит', () => {
  const shared = 'owner-decision:cluster-shared-consistent';
  const authority = { kind: 'repository-maintainer', authority_ref: 'sample-repository-maintainer', decision_ref: shared };
  const doc = decidedClusterDoc(authority, { ...authority });
  assert(evaluate(doc).length === 0, `согласованное authority решённого кластера отклонено: ${evaluate(doc)[0]}`);
});

// Общая норма проверена не только на accepted: not-applicable — второе
// решённое состояние, и единая authority обязана применяться к нему так же.
check('решённый кластер not-applicable с одинаковым owner_decision, но разными authority_ref, отклоняется', () => {
  const shared = 'owner-decision:cluster-na-shared-ref';
  const doc = decidedClusterDoc(
    { kind: 'repository-maintainer', authority_ref: 'sample-repository-maintainer', decision_ref: shared },
    { kind: 'repository-maintainer', authority_ref: 'sample-other-maintainer', decision_ref: shared },
    'not-applicable',
  );
  assert(evaluate(doc).some((p) => /inconsistent authority/.test(p)), 'разные authority_ref внутри решённого not-applicable кластера приняты');
});

check('решённый кластер not-applicable с разными authority.kind отклоняется', () => {
  const shared = 'owner-decision:cluster-na-shared-kind';
  const doc = decidedClusterDoc(
    { kind: 'repository-maintainer', authority_ref: 'sample-repository-maintainer', decision_ref: shared },
    { kind: 'project-owner', authority_ref: 'sample-repository-maintainer', decision_ref: shared },
    'not-applicable',
  );
  assert(evaluate(doc).some((p) => /inconsistent authority/.test(p)), 'разные authority.kind внутри решённого not-applicable кластера приняты');
});

check('решённый кластер not-applicable с разными authority.decision_ref отклоняется', () => {
  const doc = decidedClusterDoc(
    { kind: 'repository-maintainer', authority_ref: 'sample-repository-maintainer', decision_ref: 'owner-decision:cluster-na-decision-a' },
    { kind: 'repository-maintainer', authority_ref: 'sample-repository-maintainer', decision_ref: 'owner-decision:cluster-na-decision-b' },
    'not-applicable',
  );
  assert(evaluate(doc).some((p) => /inconsistent authority/.test(p)), 'разные authority.decision_ref внутри решённого not-applicable кластера приняты');
});

check('честный решённый кластер not-applicable с одинаковым authority проходит', () => {
  const shared = 'owner-decision:cluster-na-shared-consistent';
  const authority = { kind: 'repository-maintainer', authority_ref: 'sample-repository-maintainer', decision_ref: shared };
  const doc = decidedClusterDoc(authority, { ...authority }, 'not-applicable');
  assert(evaluate(doc).length === 0, `согласованное authority решённого not-applicable кластера отклонено: ${evaluate(doc)[0]}`);
});

check('кандидаты без решения (candidate) сохраняют разные delegated-run происхождения без нарушения', () => {
  const doc = clone(BASE);
  const second = clone(entry0(doc));
  second.id = 'sample-candidate-second-delegated';
  second.origin = { kind: 'derived', source_ref: 'instruction-source:sample-repo-claude-md' };
  second.authority = { kind: 'delegated-run', authority_ref: 'controlled-rule-intake-run:second-pass' };
  second.payload = { ...second.payload };
  second.payload.source_ref = { record_type: 'instruction-source', id: 'sample-repo-claude-md', reference: REF_B, revision: 'rev-000002', sha256: DIG_B };
  second.payload.boundary = { unit: 'whole-source' };
  doc.rule_candidates.push(second);
  assert(evaluate(doc).length === 0, `разные delegated-run у обоих candidate-происхождений отклонены: ${evaluate(doc)[0]}`);
});

// ---------------------------------------------------------------------------
// свойство 6 / 7 — граф конфликтов: симметрия и независимость от порядка
// ---------------------------------------------------------------------------
function twoCandidateDoc(a, b) {
  const doc = clone(BASE);
  doc.rule_candidates = [];
  const make = (over, ref, dig, srcId) => {
    const e = clone(entry0(BASE));
    Object.assign(e, over.entry || {});
    e.payload = { ...e.payload, ...(over.payload || {}) };
    e.payload.source_ref = { record_type: 'instruction-source', id: srcId, reference: ref, revision: srcId === 'sample-repo-agents-md' ? 'rev-000001' : 'rev-000002', sha256: dig };
    e.payload.boundary = { unit: 'whole-source' };
    return e;
  };
  doc.rule_candidates.push(make(a, REF_A, DIG_A, 'sample-repo-agents-md'));
  doc.rule_candidates.push(make(b, REF_B, DIG_B, 'sample-repo-claude-md'));
  return doc;
}

check('конфликт, объявленный только одной стороной, отклоняется независимо от порядка', () => {
  const doc = twoCandidateDoc(
    { entry: { id: 'conflict-a' }, payload: { semantic_key: 'ka', conflicts_with: ['kb'] } },
    { entry: { id: 'conflict-b' }, payload: { semantic_key: 'kb', conflicts_with: [] } },
  );
  assert(evaluate(doc).some((p) => /declared only one-sided/.test(p)), 'асимметричный конфликт принят');

  const reversed = clone(doc);
  reversed.rule_candidates.reverse();
  const problemsA = evaluate(doc).filter((p) => /declared only one-sided/.test(p));
  const problemsB = evaluate(reversed).filter((p) => /declared only one-sided/.test(p));
  assert(problemsA.length === 1 && problemsB.length === 1 && problemsA[0] === problemsB[0],
    'сообщение о конфликте зависит от порядка записей во входном массиве');
});

check('оба конфликтующих кластера, принятых одновременно, отклоняются в любом порядке', () => {
  const decision = (ref) => ({ decision_ref: ref, decided_at: '2026-09-13', reason: 'x' });
  const authority = (ref) => ({ kind: 'project-owner', authority_ref: 'workspace-owner', decision_ref: ref });
  const doc = twoCandidateDoc(
    { entry: { id: 'both-a', authority: authority('owner-decision:a') }, payload: { semantic_key: 'ka2', conflicts_with: ['kb2'], applicability_state: 'accepted', owner_decision: decision('owner-decision:a') } },
    { entry: { id: 'both-b', authority: authority('owner-decision:b') }, payload: { semantic_key: 'kb2', conflicts_with: ['ka2'], applicability_state: 'accepted', owner_decision: decision('owner-decision:b') } },
  );
  const forward = evaluate(doc);
  assert(forward.some((p) => /both are accepted/.test(p)), 'одновременное принятие конфликтующих кластеров принято');

  const reversed = clone(doc);
  reversed.rule_candidates.reverse();
  const backward = evaluate(reversed);
  const fMsg = forward.find((p) => /both are accepted/.test(p));
  const bMsg = backward.find((p) => /both are accepted/.test(p));
  assert(fMsg === bMsg, 'текст сообщения о конфликте зависит от порядка чтения записей');
});

check('lost-conflicting-decision без реально принятого оппонента отклоняется', () => {
  const doc = twoCandidateDoc(
    { entry: { id: 'loser', authority: { kind: 'project-owner', authority_ref: 'workspace-owner', decision_ref: 'owner-decision:x' } },
      payload: { semantic_key: 'kl', conflicts_with: ['kw'], applicability_state: 'not-applicable', not_applicable_reason: 'lost-conflicting-decision', owner_decision: { decision_ref: 'owner-decision:x', decided_at: '2026-09-13', reason: 'x' } } },
    { entry: { id: 'winner' }, payload: { semantic_key: 'kw', conflicts_with: ['kl'], applicability_state: 'candidate' } },
  );
  assert(evaluate(doc).some((p) => /reason is not evidenced/.test(p)), 'непроверенная причина проигрыша конфликта принята');
});

check('conflicts_with на несуществующий semantic_key отклоняется', () => {
  assert(evaluate(mutate((d) => { payload0(d).conflicts_with = ['nowhere']; })).some((p) => /not the semantic_key of any candidate/.test(p)),
    'ссылка на несуществующий кластер принята');
});

check('кандидат, называющий себя в своём же conflicts_with, отклоняется', () => {
  assert(evaluate(mutate((d) => { payload0(d).conflicts_with = [payload0(d).semantic_key]; })).some((p) => /own conflicts_with/.test(p)),
    'самоконфликт принят');
});

// ---------------------------------------------------------------------------
// свойство 9 — authority.decision_ref конверта согласован с owner_decision
// ---------------------------------------------------------------------------
check('authority.decision_ref конверта обязан совпадать с owner_decision.decision_ref', () => {
  const d = mutate((x) => {
    payload0(x).applicability_state = 'accepted';
    entry0(x).authority = { kind: 'project-owner', authority_ref: 'workspace-owner', decision_ref: 'owner-decision:envelope' };
    payload0(x).owner_decision = { decision_ref: 'owner-decision:payload', decided_at: '2026-09-13', reason: 'x' };
  });
  assert(evaluate(d).some((p) => /does not match payload.owner_decision.decision_ref/.test(p)), 'расхождение decision_ref принято');
});

// ---------------------------------------------------------------------------
// свойство 2/9 — правило связи origin и payload.source_ref
// ---------------------------------------------------------------------------
check('canonicalOriginSourceRef — каноническая форма', () => {
  assert(canonicalOriginSourceRef('sample-repo-agents-md') === 'instruction-source:sample-repo-agents-md',
    'каноническая форма origin.source_ref разошлась с payload.source_ref.id');
});

check('REQUIRED_ORIGIN_KIND — кандидат правила всегда derived', () => {
  assert(REQUIRED_ORIGIN_KIND === 'derived', 'закреплённый origin.kind для rule-candidate не "derived"');
});

check('другой origin.source_ref, чем payload.source_ref.id, отклоняется', () => {
  assert(evaluate(mutate((d) => { entry0(d).origin.source_ref = 'instruction-source:sample-repo-claude-md'; }))
    .some((p) => /does not name the same source/.test(p)), 'подменённый origin.source_ref принят');
});

check('недопустимый origin.kind для кандидата правила отклоняется', () => {
  assert(evaluate(mutate((d) => { entry0(d).origin.kind = 'imported'; }))
    .some((p) => /origin\.kind is "imported"/.test(p)), 'origin.kind "imported" принят вместо "derived"');
});

check('отсутствующая связь origin↔source_ref отклоняется, даже когда payload.source_ref указан', () => {
  assert(evaluate(mutate((d) => { delete entry0(d).origin.source_ref; }))
    .some((p) => /origin carries no source_ref/.test(p)), 'кандидат без origin.source_ref принят');
});

check('checkOriginLink напрямую — честная связь не даёт замечаний', () => {
  const problems = [];
  checkOriginLink(problems, 'x', entry0(BASE));
  assert(problems.length === 0, `честная связь origin/source_ref отклонена: ${problems[0]}`);
});

// ---------------------------------------------------------------------------
// свойство 9 — матрица scope → допустимое человеческое полномочие владельца
// ---------------------------------------------------------------------------
const acceptedWithAuthority = (authority) => mutate((d) => {
  payload0(d).applicability_state = 'accepted';
  entry0(d).authority = authority;
  payload0(d).owner_decision = { decision_ref: authority.decision_ref, decided_at: '2026-09-13', reason: 'x' };
});

check('HUMAN_OWNER_AUTHORITY_BY_SCOPE — не несёт delegated-run ни для одной области', () => {
  assert(Object.values(HUMAN_OWNER_AUTHORITY_BY_SCOPE).every((k) => k !== 'delegated-run'),
    'delegated-run закреплён как полномочие владельца для какой-то области');
});

check('честное repository-maintainer-решение для repository-scope проходит', () => {
  const d = acceptedWithAuthority({ kind: 'repository-maintainer', authority_ref: 'sample-repository-maintainer', decision_ref: 'owner-decision:x' });
  assert(evaluate(d).length === 0, `честное repository-maintainer-решение отклонено: ${evaluate(d)[0]}`);
});

check('accepted с authority.kind delegated-run отклоняется', () => {
  const d = acceptedWithAuthority({ kind: 'delegated-run', authority_ref: 'controlled-rule-intake-run:x', decision_ref: 'owner-decision:x' });
  assert(evaluate(d).some((p) => /authorized by "delegated-run"/.test(p)), 'accepted с delegated-run принят');
});

check('not-applicable с authority.kind delegated-run отклоняется', () => {
  const d = mutate((x) => {
    payload0(x).applicability_state = 'not-applicable';
    payload0(x).not_applicable_reason = 'owner-rejected';
    entry0(x).authority = { kind: 'delegated-run', authority_ref: 'controlled-rule-intake-run:x', decision_ref: 'owner-decision:x' };
    payload0(x).owner_decision = { decision_ref: 'owner-decision:x', decided_at: '2026-09-13', reason: 'x' };
  });
  assert(evaluate(d).some((p) => /authorized by "delegated-run"/.test(p)), 'not-applicable с delegated-run принят');
});

check('полномочие, не соответствующее области, отклоняется', () => {
  const d = acceptedWithAuthority({ kind: 'project-owner', authority_ref: 'workspace-owner', decision_ref: 'owner-decision:x' });
  assert(evaluate(d).some((p) => /requires owner authority "repository-maintainer"/.test(p)), 'authority.kind не по области принят');
});

check('совпадающий decision_ref не маскирует неправильный authority.kind', () => {
  const d = mutate((x) => {
    payload0(x).applicability_state = 'accepted';
    entry0(x).authority = { kind: 'delegated-run', authority_ref: 'controlled-rule-intake-run:x', decision_ref: 'owner-decision:same' };
    payload0(x).owner_decision = { decision_ref: 'owner-decision:same', decided_at: '2026-09-13', reason: 'x' };
  });
  assert(evaluate(d).some((p) => /authorized by "delegated-run"/.test(p)),
    'совпадающий authority.decision_ref/owner_decision.decision_ref маскирует неверный authority.kind');
});

check('область без закреплённого человеческого владельца (run-state) не может стать accepted', () => {
  const d = mutate((x) => {
    entry0(x).scope = { type: 'run-state', id: 'sample-run', workspace_id: 'sample-workspace' };
    payload0(x).applicability_state = 'accepted';
    entry0(x).authority = { kind: 'project-owner', authority_ref: 'workspace-owner', decision_ref: 'owner-decision:x' };
    payload0(x).owner_decision = { decision_ref: 'owner-decision:x', decided_at: '2026-09-13', reason: 'x' };
  });
  assert(evaluate(d).some((p) => /has no defined human owner authority/.test(p)), 'run-state кандидат стал accepted');
});

check('checkOwnerAuthority напрямую — candidate не проверяется, delegated-run в candidate легален', () => {
  const problems = [];
  checkOwnerAuthority(problems, 'x', entry0(BASE));
  assert(problems.length === 0, 'candidate с delegated-run отклонён напрямую checkOwnerAuthority');
});

// ---------------------------------------------------------------------------
// свойство 1 / 11 — источник данных, не исполняемый код; закрытый конверт
// ---------------------------------------------------------------------------
check('payload закрыт — постороннее поле отклоняется', () => {
  assert(evaluate(mutate((d) => { payload0(d).exec = 'rm -rf /'; })).length > 0, 'постороннее поле в payload принято');
});

check('источник — не принятая норма без явного решения (record_type/normative-адрес)', () => {
  assert(evaluate(mutate((d) => { entry0(d).record_type = 'norm'; })).length > 0, 'record_type norm принят');
});

// ---------------------------------------------------------------------------
// пулы схемы и библиотеки не разошлись
// ---------------------------------------------------------------------------
check('закрытые пулы схемы и библиотеки совпадают', () => {
  const p = registrySchema.definitions.payload.properties;
  assert(JSON.stringify(p.applicability_state.enum) === JSON.stringify(APPLICABILITY_STATES), 'пул applicability_state разошёлся');
  assert(JSON.stringify(p.not_applicable_reason.enum) === JSON.stringify(NOT_APPLICABLE_REASONS), 'пул not_applicable_reason разошёлся');
  assert(registrySchema.definitions.pinned_source_ref.properties.record_type.const === SOURCE_RECORD_TYPE, 'source_ref record_type разошёлся');
  assert(registrySchema.definitions.entry.properties.record_type.const === RECORD_TYPE, 'entry record_type разошёлся');
});

check('неподдерживаемое ключевое слово схемы отклоняется движком', () => {
  const broken = clone(registrySchema);
  broken.definitions.payload.patternProperties = { '^x': { type: 'string' } };
  let threw = false;
  try { assertSupportedDeep(broken, 'broken'); } catch { threw = true; }
  assert(threw, 'assertSupportedDeep пропустил неподдерживаемое ключевое слово');
});

console.log(`\n${passed} passed, ${failures.length} failed`);
if (failures.length) process.exit(1);
