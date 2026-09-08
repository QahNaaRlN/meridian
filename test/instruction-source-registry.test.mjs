#!/usr/bin/env node
// Standalone verification for the instruction source registry contract
// (registries/operating-model/instruction-source-registry.schema.json).
//
// It calls the same implementation the gate calls
// (scripts/lib/instruction-source-registry.mjs) so the two cannot drift: the
// record envelope against the existing scoped-record.schema.json, the source
// snapshot against the specialised instruction-source-registry.schema.json, and
// the rules JSON Schema cannot state — location confinement, the temporal
// model (recorded_state vs divergence.previous_state vs divergence.current_state),
// the currency binding, the read-channel boundary and the divergence claim.
//
// Usage: node test/instruction-source-registry.test.mjs

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { assertSupportedDeep } from '../scripts/lib/json-schema.mjs';
import {
  evaluateInstructionSourceRegistry,
  computeDivergence,
  checkDivergenceClaim,
  checkLocationPath,
  MEDIA, FORMATS, READ_CHANNEL_KINDS, MERIDIAN_VISIBILITY, CURRENCY, DIVERGENCE_STATES,
} from '../scripts/lib/instruction-source-registry.mjs';

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

const registrySchema = loadJson('registries/operating-model/instruction-source-registry.schema.json');
const envelopeSchema = loadJson('registries/operating-model/scoped-record.schema.json');
const fixtures = loadJson('registries/operating-model/fixtures/instruction-source-registry.fixtures.json');

const opts = { registrySchema, envelopeSchema };
const evaluate = (doc) => evaluateInstructionSourceRegistry(doc, opts);
const clone = (x) => JSON.parse(JSON.stringify(x));

const DIG = (h) => ({ algorithm: 'sha-256', value: String(h).repeat(64).slice(0, 64) });
const A = DIG('1');
const B = DIG('2');
const C = DIG('3');

// A topologically complete valid registry with one entry, used as the base for
// targeted mutations. Mirrors the "ordinary discoverable source" fixture:
// previous == current == recorded, status unchanged.
const BASE = {
  $schema: '../../registries/operating-model/instruction-source-registry.schema.json',
  schema_version: 1,
  registry_id: 'instruction-source-registry',
  title: 'Пример реестра источников инструкций',
  instruction_sources: [
    {
      $schema: '../../registries/operating-model/scoped-record.schema.json',
      schema_version: 1,
      id: 'sample-repo-agents-md',
      title: 'AGENTS.md рабочего репозитория',
      record_type: 'instruction-source',
      scope: { type: 'repository-scope', id: 'sample-repository', workspace_id: 'sample-workspace' },
      origin: { kind: 'declared', source_ref: 'owner-decision:register-sample-repo-agents-md' },
      authority: { kind: 'repository-maintainer', authority_ref: 'sample-repository-maintainer' },
      payload: {
        normative_status: 'not-a-norm',
        medium: 'file',
        location: { path: 'AGENTS.md', container_ref: 'sample-repository', missing_behavior: 'fail-closed' },
        recorded_state: {
          revision: 'rev-000001',
          digest: clone(A),
          revision_verified: true,
          currency: 'current',
        },
        format: 'agents-md',
        read_channel: { kind: 'meridian-observed', meridian_visibility: 'full', agent_auto_read: false },
        divergence: {
          status: 'unchanged',
          previous_state: { revision: 'rev-000001', digest: clone(A), verified: true },
          current_state: { revision: 'rev-000001', digest: clone(A), verified: true },
        },
      },
    },
  ],
};
const entry0 = (d) => d.instruction_sources[0];
const payload0 = (d) => entry0(d).payload;
const mutate = (fn) => { const d = clone(BASE); fn(d); return d; };

// ---------------------------------------------------------------------------
// schema + real fixtures
// ---------------------------------------------------------------------------
check('обе схемы используют поддерживаемое подмножество JSON Schema', () => {
  assertSupportedDeep(registrySchema, 'instruction-source-registry.schema.json');
  assertSupportedDeep(envelopeSchema, 'scoped-record.schema.json');
});

check('специализированная схема — не второй конверт записи', () => {
  assert(registrySchema.properties.registry_id.const === 'instruction-source-registry', 'registry_id не закреплён');
  assert(registrySchema.definitions.entry.properties.record_type.const === 'instruction-source',
    'record_type записи не закреплён как instruction-source');
  assert(registrySchema.definitions.payload.properties.normative_status.const === 'not-a-norm',
    'normative_status не закреплён как not-a-norm');
});

check('схема не содержит абсолютных машинных путей и продуктовых сведений', () => {
  const text = JSON.stringify(registrySchema);
  assert(!/\/home\/|\/Users\/|[A-Za-z]:\\\\/.test(text), 'в схеме найден абсолютный машинный путь');
  assert(!/MERIDIAN_INSTANCE/.test(text), 'схема ссылается на обязательный отдельный Экземпляр');
});

check('базовый реестр проходит композитную проверку без замечаний', () => {
  const problems = evaluate(BASE);
  assert(problems.length === 0, problems[0]);
});

check('пустой реестр допустим — регистрация ничего не является нормой', () => {
  assert(evaluate({ ...BASE, instruction_sources: [] }).length === 0, 'пустой реестр отклонён');
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

check('положительные примеры покрывают обычный и автоматически читаемый агентом источник', () => {
  const entries = fixtures.valid.flatMap((c) => (c.registry.instruction_sources || []));
  assert(entries.some((e) => e.payload && e.payload.read_channel && e.payload.read_channel.kind === 'meridian-observed'),
    'нет обычного наблюдаемого Meridian источника');
  assert(entries.some((e) => e.payload && e.payload.read_channel
    && e.payload.read_channel.kind === 'agent-native' && e.payload.read_channel.agent_auto_read === true),
    'нет источника, автоматически читаемого агентом');
});

// ---------------------------------------------------------------------------
// свойство 2 — область из принятой модели, не из физического расположения
// ---------------------------------------------------------------------------
check('область вне модели шести областей отклоняется', () => {
  assert(evaluate(mutate((d) => { entry0(d).scope = { type: 'team-scope', id: 'x' }; })).length > 0, 'неизвестная область принята');
});

check('одинаковое физическое расположение допустимо в разных областях', () => {
  const a = mutate((d) => { entry0(d).scope = { type: 'user-profile', id: 'sample-user' }; });
  const b = mutate((d) => { entry0(d).scope = { type: 'organization-profile', id: 'sample-organization' }; entry0(d).authority = { kind: 'organization', authority_ref: 'sample-organization' }; });
  assert(evaluate(a).length === 0, `область пользователя отклонена: ${evaluate(a)[0]}`);
  assert(evaluate(b).length === 0, `область организации отклонена: ${evaluate(b)[0]}`);
});

// ---------------------------------------------------------------------------
// свойство 1 / 8 — устойчивая идентичность, регистрация не даёт полномочий нормы
// ---------------------------------------------------------------------------
check('неустойчивый идентификатор отклоняется', () => {
  assert(evaluate(mutate((d) => { entry0(d).id = 'AGENTS'; })).length > 0, 'нестабильный id принят');
});

check('запись без человекочитаемого названия отклоняется', () => {
  assert(evaluate(mutate((d) => { delete entry0(d).title; })).length > 0, 'запись без title принята');
});

check('источник нельзя объявить принятой нормой', () => {
  assert(evaluate(mutate((d) => { payload0(d).normative_status = 'accepted-norm'; })).length > 0, 'normative_status accepted-norm принят');
  assert(evaluate(mutate((d) => { entry0(d).record_type = 'norm'; })).length > 0, 'record_type norm принят');
});

// ---------------------------------------------------------------------------
// свойство 3 — расположение ограничено носителем, без абсолютов, .., запаса
// ---------------------------------------------------------------------------
check('для medium: file обязательны и path, и container_ref', () => {
  assert(evaluate(mutate((d) => { delete payload0(d).location.container_ref; })).length > 0, 'файловый источник без container_ref принят');
  assert(evaluate(mutate((d) => { delete payload0(d).location.path; })).length > 0, 'файловый источник без path принят');
});

check('container_ref остаётся непрозрачной идентичностью, не машинным путём', () => {
  assert(evaluate(mutate((d) => { payload0(d).location.container_ref = '/srv/checkouts/repo'; })).length > 0, 'container_ref как абсолютный путь принят');
  assert(evaluate(mutate((d) => { payload0(d).location.container_ref = 'a/../b'; })).length > 0, 'container_ref с .. принят');
});

check('абсолютный путь расположения отклоняется', () => {
  assert(evaluate(mutate((d) => { payload0(d).location.path = '/etc/agents.md'; })).length > 0, 'абсолютный путь принят');
});

check('выход через .. отклоняется', () => {
  assert(evaluate(mutate((d) => { payload0(d).location.path = '../outside/AGENTS.md'; })).length > 0, '.. принят');
});

check('обратная косая черта в пути отклоняется', () => {
  assert(evaluate(mutate((d) => { payload0(d).location.path = 'sub\\AGENTS.md'; })).length > 0, 'backslash принят');
});

check('непрямое запасное поведение отклоняется', () => {
  assert(evaluate(mutate((d) => { payload0(d).location.missing_behavior = 'resolve-parent'; })).length > 0, 'нестандартное missing_behavior принято');
});

check('необъявленное поведение при отсутствии источника отклоняется', () => {
  assert(evaluate(mutate((d) => { delete payload0(d).location.missing_behavior; })).length > 0, 'отсутствие missing_behavior принято');
});

check('формы location взаимоисключающие — file не несёт service_ref/resource_ref', () => {
  assert(evaluate(mutate((d) => { payload0(d).location.service_ref = 'sample-wiki'; })).length > 0, 'file с service_ref принят');
  assert(evaluate(mutate((d) => { payload0(d).location.resource_ref = 'spaces/x'; })).length > 0, 'file с resource_ref принят');
});

check('формы location взаимоисключающие — external-service не несёт path/container_ref', () => {
  const ext = (extra) => mutate((x) => {
    payload0(x).medium = 'external-service';
    payload0(x).location = { service_ref: 'sample-wiki', resource_ref: 'spaces/eng/pages/x', missing_behavior: 'fail-closed', ...extra };
  });
  assert(evaluate(ext({})).length === 0, `корректный external-service отклонён: ${evaluate(ext({}))[0]}`);
  assert(evaluate(ext({ path: 'AGENTS.md' })).length > 0, 'external-service с path принят');
  assert(evaluate(ext({ container_ref: 'sample-repository' })).length > 0, 'external-service с container_ref принят');
});

check('checkLocationPath — прямые случаи', () => {
  assert(checkLocationPath('AGENTS.md').ok === true, 'нормальный относительный путь отклонён');
  assert(checkLocationPath('.cursor/rules/style.mdc').ok === true, 'скрытый каталог отклонён');
  assert(checkLocationPath('').ok === false, 'пустой путь принят');
  assert(checkLocationPath('/abs').ok === false, 'абсолютный путь принят');
  assert(checkLocationPath('C:/x').ok === false, 'диск принят');
  assert(checkLocationPath('a/../b').ok === false, '.. сегмент принят');
  assert(checkLocationPath('a\\b').ok === false, 'backslash принят');
});

// ---------------------------------------------------------------------------
// свойство 4 — явная редакция и SHA-256; непроверенная редакция не «актуальна»
// ---------------------------------------------------------------------------
check('отсутствие редакции отклоняется', () => {
  assert(evaluate(mutate((d) => { delete payload0(d).recorded_state.revision; })).length > 0, 'запись без revision принята');
});

check('неверный дайджест отклоняется', () => {
  assert(evaluate(mutate((d) => { payload0(d).recorded_state.digest = { algorithm: 'sha-1', value: 'abcd' }; })).length > 0, 'не-SHA-256 принят');
});

check('непроверенная редакция не объявляется актуальной', () => {
  assert(evaluate(mutate((d) => { payload0(d).recorded_state.revision_verified = false; })).length > 0,
    'currency: current при revision_verified: false принят');
  const okDoc = mutate((d) => {
    payload0(d).recorded_state.revision_verified = false;
    payload0(d).recorded_state.currency = 'unverified';
    payload0(d).divergence = { status: 'unknown' };
  });
  assert(evaluate(okDoc).length === 0, `честная непроверенная редакция отклонена: ${evaluate(okDoc)[0]}`);
});

// ---------------------------------------------------------------------------
// свойство 2 — временная семантика recorded_state / previous_state / current_state
// ---------------------------------------------------------------------------
check('recorded_state, противоречащий divergence.current_state, отклоняется', () => {
  const d = mutate((x) => {
    payload0(x).divergence = {
      status: 'changed',
      previous_state: { revision: 'rev-000000', digest: clone(B), verified: true },
      current_state: { revision: 'rev-000002', digest: clone(C), verified: true },
    };
  });
  assert(evaluate(d).some((p) => /contradicts recorded_state/.test(p)), `противоречие recorded_state/current_state не поймано: ${evaluate(d)[0]}`);
});

check('проверенный исторический снимок при отсутствующем источнике — currency: stale, не unverified', () => {
  const okDoc = mutate((d) => {
    payload0(d).recorded_state = { revision: 'rev-00000a', digest: clone(B), revision_verified: true, currency: 'stale' };
    payload0(d).divergence = { status: 'source-missing', previous_state: { revision: 'rev-00000a', digest: clone(B), verified: true } };
  });
  assert(evaluate(okDoc).length === 0, `честный stale-снимок отклонён: ${evaluate(okDoc)[0]}`);
  // подстановка unverified вместо проверенного исторического состояния запрещена
  assert(evaluate(mutate((d) => {
    payload0(d).recorded_state = { revision: 'rev-00000a', digest: clone(B), revision_verified: true, currency: 'unverified' };
    payload0(d).divergence = { status: 'source-missing' };
  })).length > 0, 'verified-снимок с currency: unverified принят');
});

check('source-missing/source-unreadable не допускают ложного currency: current', () => {
  assert(evaluate(mutate((d) => { payload0(d).divergence = { status: 'source-missing' }; })).some((p) => /"current" but divergence.status is "source-missing"/.test(p)),
    'source-missing при currency: current принят');
  assert(evaluate(mutate((d) => { payload0(d).divergence = { status: 'source-unreadable' }; })).length > 0,
    'source-unreadable при currency: current принят');
});

check('currency: stale требует доказательства исчезновения источника (одна семантика — только source-missing/source-unreadable)', () => {
  assert(evaluate(mutate((d) => { payload0(d).recorded_state.currency = 'stale'; })).some((p) => /"stale" but divergence.status is "unchanged"/.test(p)),
    'stale без source-missing/unreadable принят');
  // "changed" не даёт stale: свежее наблюдение перезаписывается, снимок current
  assert(evaluate(mutate((d) => {
    payload0(d).recorded_state.currency = 'stale';
    payload0(d).divergence = {
      status: 'changed',
      previous_state: { revision: 'rev-000000', digest: clone(B), verified: true },
      current_state: { revision: 'rev-000001', digest: clone(A), verified: true },
    };
  })).some((p) => /"stale" but divergence.status is "changed"/.test(p)), 'stale при status changed принят');
  const okDoc = mutate((d) => {
    payload0(d).recorded_state.currency = 'stale';
    payload0(d).divergence = { status: 'source-unreadable' };
  });
  assert(evaluate(okDoc).length === 0, `stale при source-unreadable отклонён: ${evaluate(okDoc)[0]}`);
});

check('одно текущее наблюдение — признаки verified обязаны согласовываться (обе стороны)', () => {
  assert(evaluate(mutate((d) => { payload0(d).divergence.current_state.verified = false; }))
    .some((p) => /current_state\.verified is false but recorded_state\.revision_verified is true/.test(p)),
    'recorded_state.revision_verified: true при current_state.verified: false принят');
  assert(evaluate(mutate((d) => {
    payload0(d).recorded_state.revision_verified = false;
    payload0(d).recorded_state.currency = 'unverified';
    payload0(d).divergence.status = 'unknown';
    delete payload0(d).divergence.previous_state;
    // current_state.verified остаётся true
  })).some((p) => /current_state\.verified is true but recorded_state\.revision_verified is false/.test(p)),
    'обратное противоречие (recorded false / current true) принято');
  const okDoc = mutate((d) => {
    payload0(d).recorded_state.revision_verified = false;
    payload0(d).recorded_state.currency = 'unverified';
    payload0(d).divergence = { status: 'unknown', current_state: { revision: 'rev-000001', digest: clone(A), verified: false } };
  });
  assert(evaluate(okDoc).length === 0, `согласованная непроверенная пара отклонена: ${evaluate(okDoc)[0]}`);
});

check('source-missing/source-unreadable — previous_state не противоречит сохранённому recorded_state', () => {
  const okDoc = mutate((d) => {
    payload0(d).recorded_state.currency = 'stale';
    payload0(d).divergence = { status: 'source-missing', previous_state: { revision: 'rev-000001', digest: clone(A), verified: true } };
  });
  assert(evaluate(okDoc).length === 0, `честный последний известный снимок отклонён: ${evaluate(okDoc)[0]}`);
  assert(evaluate(mutate((d) => {
    payload0(d).recorded_state = { revision: 'rev-000002', digest: clone(B), revision_verified: true, currency: 'stale' };
    payload0(d).divergence = { status: 'source-missing', previous_state: { revision: 'rev-000001', digest: clone(A), verified: true } };
  })).some((p) => /"source-missing" with a previous_state whose revision "rev-000001" contradicts recorded_state\.revision "rev-000002"/.test(p)),
    'source-missing с расходящимися revision/digest previous_state ↔ recorded_state принят');
});

// ---------------------------------------------------------------------------
// свойство 5 / 6 — формат и канал раздельно; read_channel описывает наблюдение
// источника, не доставку нормы; агент-нативный канал сохраняет отдельное
// отображение
// ---------------------------------------------------------------------------
check('неизвестный формат и неизвестный канал отклоняются', () => {
  assert(evaluate(mutate((d) => { payload0(d).format = 'notion-page'; })).length > 0, 'неизвестный формат принят');
  assert(evaluate(mutate((d) => { payload0(d).read_channel.kind = 'telepathy'; })).length > 0, 'неизвестный канал принят');
});

check('read_channel описывает наблюдение источника — «meridian-managed» больше не значение', () => {
  assert(!READ_CHANNEL_KINDS.includes('meridian-managed'), 'значение meridian-managed всё ещё в пуле');
  assert(READ_CHANNEL_KINDS.includes('meridian-observed'), 'нет канала meridian-observed (Meridian сам читает источник)');
  assert(READ_CHANNEL_KINDS.includes('agent-native'), 'нет отдельного отображения agent-native');
  const schemaText = JSON.stringify(registrySchema);
  assert(/do not bypass controlled-rule-intake/.test(schemaText) && /grant its text no authority/.test(schemaText),
    'схема не фиксирует, что регистрация и чтение не дают полномочий и не обходят controlled-rule-intake');
});

check('формат и канал независимы — их значения не выводятся друг из друга', () => {
  const d = mutate((x) => {
    payload0(x).format = 'plain-text';
    payload0(x).read_channel = { kind: 'meridian-observed', meridian_visibility: 'full', agent_auto_read: false };
  });
  assert(evaluate(d).length === 0, `независимая пара формат/канал отклонена: ${evaluate(d)[0]}`);
});

check('файлы, которые агент читает сам — отдельный канал agent-native с явной границей видимости Meridian', () => {
  assert(evaluate(mutate((d) => { payload0(d).read_channel = { kind: 'meridian-observed', meridian_visibility: 'full', agent_auto_read: true }; })).length > 0,
    'meridian-observed с agent_auto_read: true принят');
  assert(evaluate(mutate((d) => { payload0(d).read_channel = { kind: 'agent-native', meridian_visibility: 'full', agent_auto_read: true }; })).length > 0,
    'agent-native с полной видимостью Meridian принят');
  assert(evaluate(mutate((d) => { payload0(d).read_channel = { kind: 'agent-native', meridian_visibility: 'partial', agent_auto_read: false }; })).length > 0,
    'agent-native без agent_auto_read принят');
  const okDoc = mutate((d) => {
    payload0(d).format = 'claude-md';
    payload0(d).location.path = 'CLAUDE.md';
    payload0(d).read_channel = { kind: 'agent-native', meridian_visibility: 'partial', agent_auto_read: true };
    payload0(d).divergence = { status: 'unknown' };
  });
  assert(evaluate(okDoc).length === 0, `корректный агент-нативный канал отклонён: ${evaluate(okDoc)[0]}`);
});

// ---------------------------------------------------------------------------
// свойство 1 / 3 / 7 — расхождение считается только по проверяемым состояниям
// ---------------------------------------------------------------------------
check('computeDivergence — unchanged только при совпадении и revision, и digest', () => {
  const v = (rev, dig, verified = true) => ({ revision: rev, digest: clone(dig), verified });
  assert(computeDivergence(v('r1', A), v('r1', A)) === 'unchanged', 'совпадение revision и digest не «unchanged»');
  assert(computeDivergence(v('r1', A), v('r2', A)) === 'changed', 'разные revision при одном digest не «changed»');
  assert(computeDivergence(v('r1', A), v('r1', B)) === 'changed', 'разные digest при одном revision не «changed»');
  assert(computeDivergence(v('r1', A), v('r2', B)) === 'changed', 'разные revision и digest не «changed»');
  assert(computeDivergence(v('r1', A), v('r1', A, false)) === 'unknown', 'непроверенное текущее состояние даёт совпадение');
  assert(computeDivergence(undefined, v('r1', A)) === 'unknown', 'отсутствующее предыдущее состояние даёт совпадение');
});

check('unchanged при различии только revision отклоняется', () => {
  const d = mutate((x) => { payload0(x).divergence.previous_state.revision = 'rev-000000'; });
  assert(evaluate(d).some((p) => /the revision differs while the SHA-256 digest is unchanged/.test(p)), `не поймано: ${evaluate(d)[0]}`);
});

check('unchanged при различии только digest отклоняется', () => {
  const d = mutate((x) => {
    payload0(x).recorded_state.digest = clone(B);
    payload0(x).divergence.current_state.digest = clone(B);
  });
  assert(evaluate(d).some((p) => /the SHA-256 digest differs while the revision is unchanged/.test(p)), `не поймано: ${evaluate(d)[0]}`);
});

check('unknown при двух полных проверенных состояниях отклоняется', () => {
  const d = mutate((x) => { payload0(x).divergence.status = 'unknown'; });
  assert(evaluate(d).some((p) => /status "unknown" is declared with two complete verified states/.test(p)), `не поймано: ${evaluate(d)[0]}`);
});

check('unknown допустим, когда доказательств действительно недостаточно', () => {
  const noPrevious = mutate((x) => {
    payload0(x).divergence = { status: 'unknown', current_state: { revision: 'rev-000001', digest: clone(A), verified: true } };
  });
  assert(evaluate(noPrevious).length === 0, `unknown без previous_state отклонён: ${evaluate(noPrevious)[0]}`);
});

check('ложное состояние совпадения без доказательства отклоняется', () => {
  assert(evaluate(mutate((d) => { payload0(d).divergence = { status: 'unchanged' }; })).length > 0, 'unchanged без состояний принят');
  assert(evaluate(mutate((d) => { payload0(d).divergence.current_state.verified = false; })).length > 0, 'unchanged с непроверенным текущим состоянием принят');
});

check('source-missing и source-unreadable не несут текущее состояние', () => {
  assert(checkDivergenceClaim({
    status: 'source-missing',
    current_state: { revision: 'r', digest: clone(A), verified: true },
  }).length > 0, 'source-missing с current_state принят');
  assert(checkDivergenceClaim({
    status: 'source-unreadable',
    current_state: { revision: 'r', digest: clone(A), verified: false },
  }).length > 0, 'source-unreadable с непроверенным current_state принят');
  assert(checkDivergenceClaim({ status: 'source-missing' }).length === 0, 'честный source-missing отклонён');
  assert(checkDivergenceClaim({ status: 'unknown' }).length === 0, 'unknown без состояний отклонён');
});

// ---------------------------------------------------------------------------
// пулы схемы и библиотеки не разошлись
// ---------------------------------------------------------------------------
check('закрытые пулы схемы и библиотеки совпадают', () => {
  const p = registrySchema.definitions.payload.properties;
  assert(JSON.stringify(p.medium.enum) === JSON.stringify(MEDIA), 'пул medium разошёлся');
  assert(JSON.stringify(p.format.enum) === JSON.stringify(FORMATS), 'пул format разошёлся');
  const rc = registrySchema.definitions.read_channel.properties;
  assert(JSON.stringify(rc.kind.enum) === JSON.stringify(READ_CHANNEL_KINDS), 'пул read_channel.kind разошёлся');
  assert(JSON.stringify(rc.meridian_visibility.enum) === JSON.stringify(MERIDIAN_VISIBILITY), 'пул meridian_visibility разошёлся');
  assert(JSON.stringify(registrySchema.definitions.recorded_state.properties.currency.enum) === JSON.stringify(CURRENCY), 'пул currency разошёлся');
  assert(JSON.stringify(registrySchema.definitions.divergence.properties.status.enum) === JSON.stringify(DIVERGENCE_STATES), 'пул divergence.status разошёлся');
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
