#!/usr/bin/env node
// Regression suite for scripts/rule-resolver.mjs (PHASE C).
//
// The ten acceptance cases of §4 / §9 of the normative model are each covered
// by a named test — cases 9 and 10 are separate tests — plus append-only
// precedence in both directions, protocol-route scope isolation, provenance
// checks on unresolved and undetermined records, strict work-item validation,
// container-region fail-closed behaviour, the several/adjacent "**" glob cases,
// and negative tests for every other fail-closed path and for stable ordering.
// All fixture data is product-neutral: test/fixtures/rule-resolver.fixtures.json
// names fictional repositories, norms and paths, and this file computes each
// norm's SHA-256 from its text — a digest is never hard-coded.
//
// Acceptance-case → test-name map:
//   1  "acceptance 1 — deferred intake norm still applies; naming + lint returned"
//   2  "acceptance 2 — fragments stay separate; BUGFIX routes bugfix-protocol"
//   3  "acceptance 3 — assessment skill + degraded-state norm, no false conflict"
//   4  "acceptance 4a — layer-architecture unresolved before the new intake record"
//      "acceptance 4b — layer-architecture repository-scoped once the new record exists"
//   5  "acceptance 5 — REFACTOR yields unresolved_applicability, no protocol"
//   6  "acceptance 6 — defect in REFACTOR: separate BUGFIX child, original not reclassified"
//   7  "acceptance 7 — initiative returns decomposition view and blockers"
//      "acceptance 7b — a decomposed initiative reports no blocker"
//   8  "acceptance 8 — changed_paths beyond candidate_paths force requires_reresolution"
//   9  "acceptance 9 — a work item resolves with no reviewer assigned"
//   10 "acceptance 10 — a reviewer assignment does not change the technical result"
//
// Usage: node test/rule-resolver.test.mjs

import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import { resolveRules, sha256, globToRegExp, regionSourceText, ResolverError, UnsupportedGlob } from '../scripts/rule-resolver.mjs';
import { validate } from '../scripts/lib/json-schema.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const RESOLVER = path.join(__dirname, '..', 'scripts', 'rule-resolver.mjs');
const FX = JSON.parse(fs.readFileSync(path.join(__dirname, 'fixtures', 'rule-resolver.fixtures.json'), 'utf8'));
const OUTPUT_SCHEMA = JSON.parse(fs.readFileSync(
  path.join(__dirname, '..', 'registries', 'rule-resolution', 'resolver-output.schema.json'), 'utf8'));

let passed = 0;
let failed = 0;
const failures = [];
function ok(name) { passed++; console.log(`ok    ${name}`); }
function bad(name, msg) { failed++; failures.push(`${name}: ${msg}`); console.log(`FAIL  ${name}`); }
function check(name, fn) {
  try { fn(); ok(name); }
  catch (e) { bad(name, e && e.message ? e.message : String(e)); }
}
function assert(cond, msg) { if (!cond) throw new Error(msg || 'assertion failed'); }
function throws(fn, re, msg) {
  let threw = null;
  try { fn(); } catch (e) { threw = e; }
  if (!threw) throw new Error(msg || `expected a throw${re ? ` matching ${re}` : ''}`);
  if (re && !re.test(threw.message)) throw new Error(`throw message ${JSON.stringify(threw.message)} does not match ${re}`);
  return threw;
}

// ---------------------------------------------------------------------------
// assemble sources from the product-neutral fixture
// ---------------------------------------------------------------------------
function normIdOf(n) {
  if (n.source === 'kernel') return n.path;
  const region = n.region ? `:${n.region}` : '';
  return `${n.intake.register}#${n.path}${region}@${n.intake.recorded_at}/${n.intake.verdict}`;
}
function textKeyOf(n) {
  return `${n.repository}::${n.path}${n.region ? `::${n.region}` : ''}`;
}
function toApplicabilityRecord(n) {
  const rec = {
    norm: { repository: n.repository, path: n.path, ...(n.region ? { region: n.region } : {}) },
    digest: sha256(n.text),
    scope: n.scope,
    activation: n.activation,
    source: n.source,
    status: n.status,
    recorded_at: n.recorded_at,
  };
  if (n.scope === 'repository') rec.repository = n.repository_scope;
  if (n.scope === 'profile') rec.technology_profile = n.technology_profile;
  if (n.scope === 'product-domain') rec.product_domain = n.product_domain;
  if (n.activation === 'path-glob') rec.globs = n.globs;
  if (n.activation === 'task-class') rec.task_class = n.task_class;
  if (n.source !== 'kernel') {
    rec.intake_record = { register: n.intake.register, recorded_at: n.intake.recorded_at, verdict: n.intake.verdict };
  }
  if (n.status === 'unresolved') rec.resume_condition = n.resume_condition;
  return rec;
}
function assemble(includeIds = null, extra = {}) {
  const norms = FX.norms.filter((n) => !includeIds || includeIds.includes(n.id));
  const applicability_records = norms.map(toApplicabilityRecord);
  const norm_texts = {};
  for (const n of norms) norm_texts[textKeyOf(n)] = n.text;

  const byRegister = new Map();
  for (const n of norms) {
    if (n.source === 'kernel') continue;
    if (!byRegister.has(n.intake.register)) {
      byRegister.set(n.intake.register, { register: n.intake.register, repository: n.repository, records: [] });
    }
    byRegister.get(n.intake.register).records.push({
      artifact: n.path,
      ...(n.region ? { region: n.region } : {}),
      recorded_at: n.intake.recorded_at,
      verdict: n.intake.verdict,
      delivery: n.intake.delivery,
    });
  }
  const verification_routes = (FX.verification_routes || []).map((vr) => {
    const target = FX.norms.find((n) => n.id === vr.applies_to_norm_id);
    return { applies_to: target ? normIdOf(target) : (vr.applies_to || '*'), path: vr.path };
  });

  return {
    repository_inventory: FX.repository_inventory,
    applicability_records,
    intake_registers: [...byRegister.values()],
    protocol_routes: FX.protocol_routes,
    verification_routes,
    norm_texts,
    prior_state: {},
    ...extra,
  };
}
const fxNorm = (id) => FX.norms.find((n) => n.id === id);
const hasNorm = (out, id) => out.applicable_norms.some((e) => e.norm === normIdOf(fxNorm(id)));
const normEntry = (out, id) => out.applicable_norms.find((e) => e.norm === normIdOf(fxNorm(id)));
const hasUnresolved = (out, subject) => out.unresolved_applicability.some((u) => u.subject === subject);
const hasProtocol = (out, p) => out.applicable_protocols.some((e) => e.protocol === p);

// The standard perimeter for the axis/protocol cases: everything except the two
// layer-architecture records and the app-legacy undetermined record, which
// specific cases opt into.
const STD = ['naming', 'frag-naming', 'frag-layer', 'mod-query', 'state-review',
  'degraded-state', 'module-boundary', 'kernel-conduct'];

// ===========================================================================
// The ten acceptance cases (normative model §4 / §9)
// ===========================================================================

// case 1 — a new Vue file with the wrong name: the naming norm is applicable
// even though its intake verdict is deferred, and lint:naming is required.
check('acceptance 1 — deferred intake norm still applies; naming + lint returned', () => {
  const out = resolveRules({
    repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE',
    candidate_paths: ['apps/store/src/features/cart/useCartTotals.ts'], changed_paths: [],
  }, assemble(STD));
  const se = [];
  validate(out, OUTPUT_SCHEMA, OUTPUT_SCHEMA, '', se);
  assert(se.length === 0, `output schema: ${se[0]}`);
  const e = normEntry(out, 'naming');
  assert(e, 'naming norm not applicable');
  assert(e.activation_reason === 'always', `activation_reason ${e.activation_reason}`);
  assert(e.source === 'repository' && e.delivery === 'cursor-rule', 'naming source/delivery wrong');
  assert(e.digest === sha256(fxNorm('naming').text), 'naming digest not recomputed');
  assert(out.required_verification.includes('lint:naming'), 'lint:naming not required');
  assert(hasProtocol(out, 'test-planning'), 'FEATURE should route to test-planning');
  assert(out.conflicts.length === 0 && out.unresolved_applicability.length === 0, 'unexpected conflict/unresolved');
});

// case 2 — a query composable change: three fragments come back as separate
// entries, and BUGFIX routes the bugfix protocol independently.
check('acceptance 2 — fragments stay separate; BUGFIX routes bugfix-protocol', () => {
  const out = resolveRules({
    repository_id: 'app-frontend', work_kind: 'change', change_class: 'BUGFIX',
    candidate_paths: ['packages/entities/order/api/useGetOrderQuery.ts'], changed_paths: [],
  }, assemble(STD));
  const ids = ['frag-naming', 'frag-layer', 'mod-query'].map((id) => normIdOf(fxNorm(id)));
  for (const id of ids) assert(out.applicable_norms.some((e) => e.norm === id), `${id} missing`);
  assert(new Set(out.applicable_norms.map((e) => e.norm)).size === out.applicable_norms.length, 'entries not distinct');
  const bp = out.applicable_protocols.find((e) => e.protocol === 'bugfix-protocol');
  assert(bp && bp.routed_from === 'BUGFIX' && bp.source === 'kernel' && bp.digest, 'bugfix-protocol route wrong');
  assert(out.conflicts.length === 0, 'unexpected conflict');
});

// case 3 — skeleton/loading state: the state-review skill applies for both an
// assessment and a change; the degraded-state norm is not a false conflict.
check('acceptance 3 — assessment skill + degraded-state norm, no false conflict', () => {
  const src = assemble(STD);
  const asmt = resolveRules({
    repository_id: 'app-frontend', work_kind: 'assessment',
    candidate_paths: ['packages/features/cart/ui/CartSummary.vue'], changed_paths: [],
  }, src);
  const sr = normEntry(asmt, 'state-review');
  assert(sr && sr.activation_reason === 'task-class' && sr.delivery === 'skill-package', 'state-review not applicable as skill');
  assert(hasNorm(asmt, 'degraded-state'), 'degraded-state norm missing');
  assert(asmt.conflicts.length === 0, 'false conflict between error-handling and loading-state');
  assert(hasProtocol(asmt, 'code-review'), 'assessment should route code-review');
  const chg = resolveRules({
    repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE',
    candidate_paths: ['packages/features/cart/ui/CartSummary.vue'], changed_paths: [],
  }, src);
  assert(hasNorm(chg, 'state-review'), 'state-review should also apply for a change');
});

// case 4 — an import between hFSD layers: unresolved scope before the new
// repository-scope intake record, applicable repository-scoped after it; the
// module-boundary norm is applicable either way.
check('acceptance 4a — layer-architecture unresolved before the new intake record', () => {
  const out = resolveRules({
    repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE',
    candidate_paths: ['packages/entities/order/model/mappers.ts'], changed_paths: [],
  }, assemble([...STD, 'layer-arch-profile']));
  assert(hasUnresolved(out, normIdOf(fxNorm('layer-arch-profile'))), 'layer-architecture not in unresolved_applicability');
  assert(!out.applicable_norms.some((e) => /layer-architecture/.test(e.norm)), 'layer-architecture must not be applicable yet');
  assert(hasNorm(out, 'module-boundary'), 'module-boundary should be applicable regardless');
});
check('acceptance 4b — layer-architecture repository-scoped once the new record exists', () => {
  const out = resolveRules({
    repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE',
    candidate_paths: ['packages/entities/order/model/mappers.ts'], changed_paths: [],
  }, assemble([...STD, 'layer-arch-profile', 'layer-arch-repository']));
  const e = normEntry(out, 'layer-arch-repository');
  assert(e && e.source === 'repository' && e.delivery === 'cursor-rule', 'layer-architecture not applicable repository-scoped');
  assert(!out.unresolved_applicability.some((u) => /layer-architecture/.test(u.subject)), 'stale unresolved entry survived');
  assert(hasNorm(out, 'module-boundary'), 'module-boundary should still be applicable');
});

// case 5 — a pure REFACTOR gets unresolved_applicability, not another class's
// protocol, until PHASE D/E exist.
check('acceptance 5 — REFACTOR yields unresolved_applicability, no protocol', () => {
  const out = resolveRules({
    repository_id: 'app-frontend', work_kind: 'change', change_class: 'REFACTOR',
    candidate_paths: ['packages/entities/order/model/mappers.ts'], changed_paths: [],
  }, assemble(STD));
  const u = out.unresolved_applicability.find((x) => x.subject === 'REFACTOR');
  assert(u && /PHASE D/.test(u.reason) && /PHASE E/.test(u.reason), 'REFACTOR reason must cite PHASE D and PHASE E');
  assert(out.applicable_protocols.length === 0, 'REFACTOR must not route a protocol');
  assert(!hasProtocol(out, 'bugfix-protocol') && !hasProtocol(out, 'test-planning'), 'no residual protocol for REFACTOR');
});

// case 6 — a defect found during a REFACTOR needs a separate BUGFIX child /
// owner decision and does not reclassify the original work.
check('acceptance 6 — defect in REFACTOR: separate BUGFIX child, original not reclassified', () => {
  const src = assemble(STD, { prior_state: { refactor_findings: [{ type: 'behavior-change' }] } });
  const parent = resolveRules({
    repository_id: 'app-frontend', work_kind: 'change', change_class: 'REFACTOR',
    candidate_paths: ['packages/entities/order/model/mappers.ts'], changed_paths: [],
  }, src);
  assert(parent.unresolved_applicability.some((u) => u.subject === 'REFACTOR'), 'REFACTOR still unresolved');
  const f = parent.unresolved_applicability.find((u) => u.subject === 'refactor-in-progress-finding');
  assert(f && /BUGFIX child work item/.test(f.reason) && /not reclassified/.test(f.reason), 'finding entry missing/weak');
  assert(parent.applicable_protocols.length === 0, 'parent REFACTOR still routes no protocol');
  const child = resolveRules({
    repository_id: 'app-frontend', work_kind: 'change', change_class: 'BUGFIX',
    candidate_paths: ['packages/entities/order/model/mappers.ts'], changed_paths: [],
  }, src);
  assert(hasProtocol(child, 'bugfix-protocol'), 'child BUGFIX must route bugfix-protocol');
  assert(!child.unresolved_applicability.some((u) => u.subject === 'REFACTOR'), 'child must not carry the REFACTOR class');
  assert(!child.applicable_protocols.some((e) => e.routed_from === 'REFACTOR'), 'nothing routed_from REFACTOR');
});

// case 7 — a mixed consolidation initiative: decomposition_required, the
// declared decomposition protocol, pre-decomposition norms, and the missing
// source-repository list as a decomposition blocker (kept apart from
// unresolved_applicability).
check('acceptance 7 — initiative returns decomposition view and blockers', () => {
  const out = resolveRules({
    repository_id: 'app-frontend', work_kind: 'initiative', candidate_paths: [], changed_paths: [],
  }, assemble(STD, {
    prior_state: {
      initiative_protocol: FX.initiative_protocol,
      pre_decomposition_norms: FX.pre_decomposition_norms,
      source_repository_ids: ['app-frontend'],
    },
  }));
  assert(out.decomposition_required === true, 'decomposition_required must be true');
  assert(out.applicable_initiative_protocol === 'consolidation-decomposition', 'initiative protocol not surfaced');
  assert(JSON.stringify(out.pre_decomposition_norms) === JSON.stringify(['consolidation-inventory-required', 'parity-registry-required']), 'pre_decomposition_norms wrong/unsorted');
  assert(out.unresolved_items.length === 1 && /source repositor/i.test(out.unresolved_items[0].item), 'missing decomposition blocker');
  assert(out.unresolved_applicability.length === 0, 'a decomposition blocker must not land in unresolved_applicability');
  assert(out.applicable_norms.length === 0 && out.applicable_protocols.length === 0, 'initiative level does not resolve child norms/protocols');
});
check('acceptance 7b — a decomposed initiative reports no blocker', () => {
  const out = resolveRules({
    repository_id: 'app-frontend', work_kind: 'initiative', candidate_paths: [], changed_paths: [],
  }, assemble(STD, {
    prior_state: {
      initiative_protocol: FX.initiative_protocol,
      pre_decomposition_norms: FX.pre_decomposition_norms,
      source_repository_ids: ['app-frontend', 'app-legacy', 'app-shared'],
      decomposition: { child_work_items: [{ repository_id: 'app-frontend', work_kind: 'change', change_class: 'REFACTOR' }] },
    },
  }));
  assert(out.decomposition_required === false, 'decomposition already supplied');
  assert(out.unresolved_items.length === 0, 'no blocker once decomposed');
});

// case 8 — changed_paths widen the set a previous resolution used.
check('acceptance 8 — changed_paths beyond candidate_paths force requires_reresolution', () => {
  const src = assemble(STD);
  const widened = resolveRules({
    repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE',
    candidate_paths: ['a.ts', 'b.ts'], changed_paths: ['a.ts', 'b.ts', 'c.ts'],
  }, { ...src, prior_state: { previous_resolution: { candidate_paths: ['a.ts', 'b.ts'] } } });
  assert(widened.requires_reresolution === true, 'expansion must set requires_reresolution');
  const same = resolveRules({
    repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE',
    candidate_paths: ['a.ts', 'b.ts'], changed_paths: ['a.ts', 'b.ts'],
  }, { ...src, prior_state: { previous_resolution: { candidate_paths: ['a.ts', 'b.ts'] } } });
  assert(same.requires_reresolution === false, 'no expansion, no reresolution');
});

// case 9 — one agent, no reviewer assigned: resolution is not blocked by the
// absence of a reviewer, and the resolver does not decide whether one is needed.
const WI_9_10 = {
  repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE',
  candidate_paths: ['packages/entities/order/api/useGetOrderQuery.ts'], changed_paths: [],
};
check('acceptance 9 — a work item resolves with no reviewer assigned', () => {
  const out = resolveRules(WI_9_10, assemble(STD));
  const se = [];
  validate(out, OUTPUT_SCHEMA, OUTPUT_SCHEMA, '', se);
  assert(se.length === 0, `output schema: ${se[0]}`);
  assert(out.applicable_norms.length > 0, 'norms should still resolve without a reviewer');
  assert(hasProtocol(out, 'test-planning'), 'protocol should still route without a reviewer');
  assert(!('reviewer' in out) && !('handoff' in out), 'resolver output must not carry a reviewer/handoff notion');
});

// case 10 — the same work item with a reviewer assignment in the execution
// context: the technical result is identical; a reviewer is not a resolver input.
check('acceptance 10 — a reviewer assignment does not change the technical result', () => {
  const noReviewer = resolveRules(WI_9_10, assemble(STD));
  const withReviewer = resolveRules(WI_9_10, assemble(STD, { reviewer_assignment: { reviewer: 'agent-x', integrator: 'agent-y' } }));
  assert(JSON.stringify(noReviewer) === JSON.stringify(withReviewer), 'reviewer assignment changed the technical result');
  assert(JSON.stringify(noReviewer) === JSON.stringify(resolveRules(WI_9_10, assemble(STD))), 'result not reproducible');
});

// ===========================================================================
// extra behavioural coverage
// ===========================================================================
// A genuine status:unresolved record that *does* match the work item on
// scope/path is surfaced in unresolved_applicability under ITS OWN
// resume_condition (not a generic string), and never as an applicable_norm.
check('extra — a status:unresolved record surfaces under its own resume_condition', () => {
  const out = resolveRules({
    repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE',
    candidate_paths: ['packages/entities/order/model/mappers.ts'], changed_paths: [],
  }, assemble([...STD, 'layer-arch-profile']));
  const u = out.unresolved_applicability.find((x) => x.subject === normIdOf(fxNorm('layer-arch-profile')));
  assert(u, 'the status:unresolved record is not surfaced');
  assert(u.reason === fxNorm('layer-arch-profile').resume_condition, 'the record\'s own resume_condition is not the reason');
  assert(!hasNorm(out, 'layer-arch-profile'), 'a status:unresolved record must not become an applicable_norm');
});

// A real activation:undetermined record is surfaced as unresolved, not dropped.
check('extra — an activation:undetermined record is surfaced as unresolved', () => {
  const out = resolveRules({
    repository_id: 'app-legacy', work_kind: 'change', change_class: 'FEATURE',
    candidate_paths: ['src/x.ts'], changed_paths: [],
  }, assemble(['activation-undetermined', 'kernel-conduct']));
  assert(out.unresolved_applicability.some((u) => /undetermined/.test(u.reason)), 'undetermined activation not surfaced');
  assert(!hasNorm(out, 'activation-undetermined'), 'an undetermined record must not become an applicable_norm');
});
check('extra — a kernel-source norm needs no intake pointer and is delivered kernel-doc', () => {
  const out = resolveRules({
    repository_id: 'app-frontend', work_kind: 'operation', candidate_paths: [], changed_paths: [],
  }, assemble(['kernel-conduct']));
  const e = normEntry(out, 'kernel-conduct');
  assert(e && e.source === 'kernel' && e.delivery === 'kernel-doc', 'kernel norm entry wrong');
});
check('extra — separate axes are conjunctive: profile mismatch drops the norm', () => {
  const out = resolveRules({
    repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE',
    candidate_paths: ['packages/entities/order/model/mappers.ts'], changed_paths: [],
    declared_profiles: { technology_profile: 'other-runtime' },
  }, assemble([...STD, 'layer-arch-profile']));
  assert(!hasUnresolved(out, normIdOf(fxNorm('layer-arch-profile'))), 'profile axis should have excluded the norm');
});

// ===========================================================================
// append-only precedence (normative model §4; rule-resolution.md §4, §9)
// ===========================================================================
// A pair of append-only applicability records for one norm identity, built
// inline so the two recorded_at values are the only thing under test.
function precedencePair(olderStatus, olderAt, newerStatus, newerAt) {
  const base = {
    norm: { repository: 'app-frontend', path: 'rules/precede.md' },
    digest: sha256('# Precedence\n\nA rule under append-only test.\n'),
    scope: 'repository', repository: 'app-frontend',
    activation: 'always', source: 'repository',
    intake_record: { register: 'instruction-intake/app-frontend.yaml', recorded_at: '2026-08-01', verdict: 'keep-local' },
  };
  const mk = (status, at) => {
    const r = { ...base, status, recorded_at: at };
    if (status === 'unresolved') r.resume_condition = `resume when the ${at} record is revisited`;
    return r;
  };
  const src = assemble(['kernel-conduct']);
  src.applicability_records.push(mk(olderStatus, olderAt), mk(newerStatus, newerAt));
  src.norm_texts['app-frontend::rules/precede.md'] = '# Precedence\n\nA rule under append-only test.\n';
  src.intake_registers = [{
    register: 'instruction-intake/app-frontend.yaml', repository: 'app-frontend',
    records: [{ artifact: 'rules/precede.md', recorded_at: '2026-08-01', verdict: 'keep-local', delivery: 'cursor-rule' }],
  }];
  return src;
}
const precWi = {
  repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE',
  candidate_paths: ['x'], changed_paths: [],
};
const PRECEDE_ID = 'instruction-intake/app-frontend.yaml#rules/precede.md@2026-08-01/keep-local';

check('precedence — a newer status:unresolved suppresses an older status:resolved', () => {
  const out = resolveRules(precWi, precedencePair('resolved', '2026-08-10', 'unresolved', '2026-08-20'));
  assert(!out.applicable_norms.some((e) => e.norm === PRECEDE_ID), 'older resolved record must not win');
  const u = out.unresolved_applicability.find((x) => x.subject === PRECEDE_ID);
  assert(u && /2026-08-20/.test(u.reason), 'newer unresolved record must set the outcome and its own reason');
});
check('precedence — a newer status:resolved suppresses an older status:unresolved (case 4b)', () => {
  const out = resolveRules(precWi, precedencePair('unresolved', '2026-08-10', 'resolved', '2026-08-20'));
  assert(out.applicable_norms.some((e) => e.norm === PRECEDE_ID), 'newer resolved record must win');
  assert(!out.unresolved_applicability.some((x) => x.subject === PRECEDE_ID), 'older unresolved entry must not survive');
});
check('precedence — records that cannot be ordered within identity/date are fail-closed', () => {
  const src = precedencePair('resolved', '2026-08-15', 'unresolved', '2026-08-15');
  throws(() => resolveRules(precWi, src), /sharing the latest recorded_at|append-only precedence cannot be decided/);
});

// ===========================================================================
// protocol-route scope is exact (normative model §7; rule-resolution.md §7)
// ===========================================================================
check('route scope — a repository route for one repository does not reach another', () => {
  const isolated = resolveRules({
    repository_id: 'app-legacy', work_kind: 'change', change_class: 'FEATURE',
    candidate_paths: ['src/x.ts'], changed_paths: [],
  }, assemble(STD));
  assert(!hasProtocol(isolated, 'test-planning'), 'a repository:app-frontend route must not apply to app-legacy');
  const applied = resolveRules({
    repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE',
    candidate_paths: ['src/x.ts'], changed_paths: [],
  }, assemble(STD));
  assert(hasProtocol(applied, 'test-planning'), 'the same route must apply to app-frontend');
});
check('route scope — a product-domain route reaches only repositories carrying that domain', () => {
  const inDomain = resolveRules({
    repository_id: 'app-frontend', work_kind: 'change', change_class: 'BEHAVIOR_CHANGE',
    candidate_paths: ['src/x.ts'], changed_paths: [],
  }, assemble(STD));
  assert(hasProtocol(inDomain, 'admin-domain-review'), 'app-frontend carries the Admin domain and should get the route');
  const outOfDomain = resolveRules({
    repository_id: 'app-legacy', work_kind: 'change', change_class: 'BEHAVIOR_CHANGE',
    candidate_paths: ['src/x.ts'], changed_paths: [],
  }, assemble(STD));
  assert(!hasProtocol(outOfDomain, 'admin-domain-review'), 'app-legacy does not carry the Admin domain');
});
check('route scope — a repository route with no repository selector is fail-closed', () => {
  const src = assemble(STD);
  src.protocol_routes = [{ routed_from: 'FEATURE', protocol: 'p', source: 'repository', scope: 'repository', digest: 'a'.repeat(64), mandatory: true }];
  throws(() => resolveRules({
    repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE', candidate_paths: ['x'], changed_paths: [],
  }, src), /no exact "repository" selector/);
});

// ===========================================================================
// provenance is checked for EVERY matching record — resolved, unresolved and
// activation:undetermined alike (normative model §4; rule-resolution.md §9)
// ===========================================================================
for (const [label, fxId, wi] of [
  ['a status:unresolved', 'layer-arch-profile', {
    repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE',
    candidate_paths: ['packages/entities/order/model/mappers.ts'], changed_paths: [],
  }],
  ['an activation:undetermined', 'activation-undetermined', {
    repository_id: 'app-legacy', work_kind: 'change', change_class: 'FEATURE',
    candidate_paths: ['src/x.ts'], changed_paths: [],
  }],
]) {
  const set = fxId === 'layer-arch-profile' ? [...STD, fxId] : [fxId, 'kernel-conduct'];
  check(`neg — a stale digest on a ${label} record is fail-closed`, () => {
    const src = assemble(set);
    src.norm_texts[textKeyOf(fxNorm(fxId))] = 'text that no longer hashes to the record digest\n';
    throws(() => resolveRules(wi, src), /stale digest/);
  });
  check(`neg — missing text for a ${label} record is fail-closed`, () => {
    const src = assemble(set);
    delete src.norm_texts[textKeyOf(fxNorm(fxId))];
    throws(() => resolveRules(wi, src), /no current text supplied/);
  });
  check(`neg — an unresolvable intake pointer on a ${label} record is fail-closed`, () => {
    const src = assemble(set);
    src.intake_registers = [];
    throws(() => resolveRules(wi, src), /not among the supplied intake registers/);
  });
}

// ===========================================================================
// work item §3.1 is validated strictly — no coercion of missing/malformed input
// ===========================================================================
for (const field of ['candidate_paths', 'changed_paths']) {
  check(`neg — a missing ${field} is fail-closed, not read as []`, () => {
    const wi = { repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE' };
    wi[field === 'candidate_paths' ? 'changed_paths' : 'candidate_paths'] = [];
    throws(() => resolveRules(wi, assemble(STD)), new RegExp(`${field} is required and must be an array`));
  });
  check(`neg — a non-array ${field} is fail-closed`, () => {
    const wi = { repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE', candidate_paths: [], changed_paths: [] };
    wi[field] = 'x/y.ts';
    throws(() => resolveRules(wi, assemble(STD)), new RegExp(`${field} is required and must be an array`));
  });
  check(`neg — a ${field} array with a non-string element is fail-closed`, () => {
    const wi = { repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE', candidate_paths: [], changed_paths: [] };
    wi[field] = ['ok.ts', 42];
    throws(() => resolveRules(wi, assemble(STD)), new RegExp(`${field}\\[1\\] must be a string`));
  });
}
check('neg — an unknown work-item field is fail-closed', () => {
  throws(() => resolveRules({
    repository_id: 'app-frontend', work_kind: 'assessment', candidate_paths: [], changed_paths: [], reviewer: 'agent-x',
  }, assemble(STD)), /unknown field "reviewer"/);
});
check('neg — a malformed declared_profiles is fail-closed', () => {
  throws(() => resolveRules({
    repository_id: 'app-frontend', work_kind: 'assessment', candidate_paths: [], changed_paths: [],
    declared_profiles: { technology_profile: 7 },
  }, assemble(STD)), /declared_profiles\.technology_profile must be a string/);
});

// ===========================================================================
// container regions — one reader, fail-closed, fenced examples are not
// declarations (normative model finding 4; instruction-intake.md §3.1)
// ===========================================================================
const REGION_DOC = [
  '---', 'title: Container', '---', '', '# Container', '',
  '<!-- meridian:begin instruction-section id=alpha owner=workspace-owner -->',
  'The alpha rule.',
  '<!-- meridian:end instruction-section id=alpha -->', '',
  '<!-- meridian:begin instruction-section id=beta owner=workspace-owner -->',
  'The beta rule.',
  '<!-- meridian:end instruction-section id=beta -->', '',
].join('\n');
check('region — a declared region is returned by its id', () => {
  assert(/The beta rule\./.test(regionSourceText(REGION_DOC, 'beta')), 'beta region text not returned');
  assert(!/alpha/.test(regionSourceText(REGION_DOC, 'beta')), 'region bled into a neighbour');
});
check('region — a region id the file does not declare is fail-closed (no whole-file fallback)', () => {
  throws(() => regionSourceText(REGION_DOC, 'gamma'), /does not declare/);
});
check('region — a duplicated region id is fail-closed', () => {
  const dup = REGION_DOC + [
    '<!-- meridian:begin instruction-section id=beta owner=workspace-owner -->',
    'A second beta.',
    '<!-- meridian:end instruction-section id=beta -->', '',
  ].join('\n');
  throws(() => regionSourceText(dup, 'beta'), /declared twice|cannot be read|declares region "beta" 2 times/);
});
check('region — an unclosed region is fail-closed', () => {
  const unclosed = [
    '# Container', '',
    '<!-- meridian:begin instruction-section id=alpha owner=workspace-owner -->',
    'The alpha rule, never closed.', '',
  ].join('\n');
  throws(() => regionSourceText(unclosed, 'alpha'), /never closed|cannot be read/);
});
check('region — a marker inside a fenced code block is an example, not a declaration', () => {
  const fenced = [
    '# Container', '',
    '```', '<!-- meridian:begin instruction-section id=alpha owner=workspace-owner -->',
    'not a real region', '<!-- meridian:end instruction-section id=alpha -->', '```', '',
    '<!-- meridian:begin instruction-section id=real owner=workspace-owner -->',
    'The real rule.', '<!-- meridian:end instruction-section id=real -->', '',
  ].join('\n');
  throws(() => regionSourceText(fenced, 'alpha'), /does not declare/);
  assert(/The real rule\./.test(regionSourceText(fenced, 'real')), 'a real region outside the fence is still readable');
});
// The digest is over the VERBATIM source of the region — fenced code inside the
// markers must survive intact, not be blanked to spaces, and hash to exactly
// the SHA-256 of the raw slice the markers delimit.
check('region — sourceText keeps fenced code verbatim and matches the raw-slice digest', () => {
  const doc = [
    '---', 'title: Container', '---', '', '# Container', '',
    '<!-- meridian:begin instruction-section id=alpha owner=workspace-owner -->',
    'Before the fence.', '',
    '```js',
    'const answer = 42; // a real code example that lives inside the region',
    'foo(); ///not-a-region marker-shaped bytes: <!-- meridian:end instruction-section id=alpha -->',
    '```', '',
    'After the fence.',
    '<!-- meridian:end instruction-section id=alpha -->', '',
    '<!-- meridian:begin instruction-section id=beta owner=workspace-owner -->',
    'Beta body.',
    '<!-- meridian:end instruction-section id=beta -->', '',
  ].join('\n');
  const got = regionSourceText(doc, 'alpha');
  assert(/const answer = 42;/.test(got), 'fenced code was blanked out of the source text');
  assert(got.includes('```js') && got.includes('```\n'), 'the fence markers were not preserved verbatim');
  assert(/After the fence\./.test(got) && /Before the fence\./.test(got), 'region body truncated');
  assert(!/Beta body\./.test(got), 'the source slice ran past this region');
  // Recompute the raw slice independently, exactly as the markers delimit it:
  // from the end of the begin marker to the start of the REAL end marker (the
  // marker-shaped bytes inside the fence are an example, so the real end marker
  // is the last occurrence in the document).
  const begin = /<!--\s*meridian:begin\s+instruction-section\b[^>]*?-->/.exec(doc);
  const endIdx = doc.lastIndexOf('<!-- meridian:end instruction-section id=alpha -->');
  const rawSlice = doc.slice(begin.index + begin[0].length, endIdx);
  assert(got === rawSlice, 'sourceText is not the verbatim raw slice between the markers');
  assert(sha256(got) === sha256(rawSlice), 'sourceText digest does not equal the raw-slice digest');
});

// ===========================================================================
// path-glob — several and adjacent "**", each "zero or more whole segments"
// ===========================================================================
check('glob — several and adjacent "**" each mean zero-or-more whole segments', () => {
  const m = (g, p) => globToRegExp(g).test(p);
  for (const p of ['x', 'a/x', 'a/b/x']) assert(m('**/**/x', p), `**/**/x should match ${p}`);
  assert(!m('**/**/x', 'a/b/c'), '**/**/x must not match a path not ending in x');
  for (const p of ['a/b', 'a/x/b', 'a/x/y/b']) assert(m('a/**/**/b', p), `a/**/**/b should match ${p}`);
  assert(!m('a/**/**/b', 'a/b/c'), 'a/**/**/b must not match a/b/c');
  for (const p of ['a/b/c', 'a/x/b/y/z/c']) assert(m('a/**/b/**/c', p), `a/**/b/**/c should match ${p}`);
  assert(!m('a/**/b/**/c', 'a/b/x'), 'a/**/b/**/c must not match a/b/x');
  assert(m('**', 'anything/at/all'), '"**" alone matches any path');
});

// ===========================================================================
// negative tests — every fail-closed path
// ===========================================================================
check('neg — unknown repository_id is fail-closed', () => {
  throws(() => resolveRules({ repository_id: 'no-such-repo', work_kind: 'assessment', candidate_paths: [], changed_paths: [] }, assemble(STD)), /unknown repository_id/);
});
check('neg — work_kind change without change_class is rejected', () => {
  throws(() => resolveRules({ repository_id: 'app-frontend', work_kind: 'change', candidate_paths: [], changed_paths: [] }, assemble(STD)), /change_class is required/);
});
check('neg — change_class on a non-change work_kind is rejected', () => {
  throws(() => resolveRules({ repository_id: 'app-frontend', work_kind: 'assessment', change_class: 'BUGFIX', candidate_paths: [], changed_paths: [] }, assemble(STD)), /meaningful only inside work_kind "change"/);
});
check('neg — an unknown work_kind is rejected', () => {
  throws(() => resolveRules({ repository_id: 'app-frontend', work_kind: 'refactor', candidate_paths: [], changed_paths: [] }, assemble(STD)), /not one of change/);
});
check('neg — an out-of-pool change_class is rejected', () => {
  throws(() => resolveRules({ repository_id: 'app-frontend', work_kind: 'change', change_class: 'REFAKTOR', candidate_paths: [], changed_paths: [] }, assemble(STD)), /must be BUGFIX/);
});
check('neg — a malformed applicability record is fail-closed', () => {
  const src = assemble(['kernel-conduct']);
  src.applicability_records.push({
    norm: { repository: 'app-frontend', path: 'rules/broken.md' },
    digest: '0'.repeat(64), scope: 'repository', activation: 'always', source: 'repository',
    intake_record: { register: 'instruction-intake/app-frontend.yaml', recorded_at: '2026-08-20', verdict: 'keep-local' },
    status: 'resolved', recorded_at: '2026-08-27',
  }); // scope repository but no `repository` field
  throws(() => resolveRules({ repository_id: 'app-frontend', work_kind: 'assessment', candidate_paths: [], changed_paths: [] }, src), /malformed applicability record/);
});
check('neg — a stale digest is fail-closed', () => {
  const src = assemble(['naming', 'kernel-conduct']);
  src.norm_texts[textKeyOf(fxNorm('naming'))] = 'a different text than the one hashed into the record\n';
  throws(() => resolveRules({ repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE', candidate_paths: ['x'], changed_paths: [] }, src), /stale digest/);
});
check('neg — an unresolvable intake pointer is fail-closed', () => {
  const src = assemble(['naming', 'kernel-conduct']);
  src.intake_registers = []; // the pointer now names a register that was not supplied
  throws(() => resolveRules({ repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE', candidate_paths: ['x'], changed_paths: [] }, src), /not among the supplied intake registers/);
});
check('neg — an ambiguous intake pointer is fail-closed', () => {
  const src = assemble(['naming', 'kernel-conduct']);
  const reg = src.intake_registers.find((r) => r.register === 'instruction-intake/app-frontend.yaml');
  reg.records.push({ ...reg.records[0] }); // two records match artifact + date + verdict
  throws(() => resolveRules({ repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE', candidate_paths: ['x'], changed_paths: [] }, src), /ambiguous/);
});
check('neg — an unsupported glob is fail-closed, never approximated', () => {
  const src = assemble(['kernel-conduct']);
  src.applicability_records.push({
    norm: { repository: 'app-frontend', path: 'rules/brace.md' },
    digest: sha256('x'), scope: 'universal', activation: 'path-glob', globs: ['src/{a,b}/*.ts'],
    source: 'repository',
    intake_record: { register: 'instruction-intake/app-frontend.yaml', recorded_at: '2026-08-20', verdict: 'keep-local' },
    status: 'resolved', recorded_at: '2026-08-27',
  });
  src.intake_registers = [{ register: 'instruction-intake/app-frontend.yaml', repository: 'app-frontend', records: [
    { artifact: 'rules/brace.md', recorded_at: '2026-08-20', verdict: 'keep-local', delivery: 'cursor-rule' },
  ] }];
  src.norm_texts['app-frontend::rules/brace.md'] = 'x';
  const e = throws(() => resolveRules({ repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE', candidate_paths: ['src/a/x.ts'], changed_paths: [] }, src), /unsupported syntax/);
  assert(e instanceof UnsupportedGlob && e instanceof ResolverError, 'wrong error type for unsupported glob');
});
check('neg — incompatible mandatory protocol routes form a conflict, not a silent pick', () => {
  const src = assemble(STD, {});
  src.protocol_routes = [...FX.protocol_routes, ...FX.protocol_routes_conflicting];
  const out = resolveRules({
    repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE',
    candidate_paths: ['apps/store/src/x.ts'], changed_paths: [],
  }, src);
  const c = out.conflicts.find((x) => x.norms.includes('test-planning') && x.norms.includes('local-feature-flow'));
  assert(c && /universal/.test(c.reason), 'no conflict entry for the incompatible mandatory routes');
  assert(!hasProtocol(out, 'local-feature-flow') && !hasProtocol(out, 'test-planning'), 'a conflicting route must not also be emitted');
});
check('neg — a missing mandatory source is fail-closed', () => {
  throws(() => resolveRules({ repository_id: 'app-frontend', work_kind: 'assessment', candidate_paths: [], changed_paths: [] }, { repository_inventory: FX.repository_inventory }), /missing required source: "applicability_records"/);
});
check('neg — a protocol route with neither digest nor revision is fail-closed', () => {
  const src = assemble(STD);
  src.protocol_routes = [{ routed_from: 'FEATURE', protocol: 'p', source: 'kernel', scope: 'universal' }];
  throws(() => resolveRules({ repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE', candidate_paths: ['x'], changed_paths: [] }, src), /neither digest nor revision/);
});

// ===========================================================================
// determinism and schema conformance
// ===========================================================================
check('order — the result is byte-identical across repeated runs', () => {
  const wi = {
    repository_id: 'app-frontend', work_kind: 'change', change_class: 'BUGFIX',
    candidate_paths: ['packages/entities/order/api/useGetOrderQuery.ts'], changed_paths: [],
  };
  const a = JSON.stringify(resolveRules(wi, assemble(STD)));
  const b = JSON.stringify(resolveRules(wi, assemble(STD)));
  const c = JSON.stringify(resolveRules(wi, assemble(STD)));
  assert(a === b && b === c, 'resolver output is not deterministic');
});
check('order — record order in the input does not change the output', () => {
  const wi = {
    repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE',
    candidate_paths: ['packages/entities/order/api/useGetOrderQuery.ts'], changed_paths: [],
  };
  const s1 = assemble(STD);
  const s2 = assemble(STD);
  s2.applicability_records.reverse();
  assert(JSON.stringify(resolveRules(wi, s1)) === JSON.stringify(resolveRules(wi, s2)), 'output depends on input record order');
});
check('schema — every successful result validates against resolver-output.schema.json', () => {
  const results = [
    resolveRules({ repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE', candidate_paths: ['apps/store/x.ts'], changed_paths: [] }, assemble(STD)),
    resolveRules({ repository_id: 'app-frontend', work_kind: 'change', change_class: 'BUGFIX', candidate_paths: ['packages/entities/order/api/q.ts'], changed_paths: [] }, assemble(STD)),
    resolveRules({ repository_id: 'app-frontend', work_kind: 'assessment', candidate_paths: ['packages/features/cart/ui/x.vue'], changed_paths: [] }, assemble(STD)),
    resolveRules({ repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE', candidate_paths: ['packages/entities/order/model/mappers.ts'], changed_paths: [] }, assemble([...STD, 'layer-arch-profile'])),
    resolveRules({ repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE', candidate_paths: ['packages/entities/order/model/mappers.ts'], changed_paths: [] }, assemble([...STD, 'layer-arch-profile', 'layer-arch-repository'])),
    resolveRules({ repository_id: 'app-frontend', work_kind: 'change', change_class: 'REFACTOR', candidate_paths: ['x'], changed_paths: [] }, assemble(STD)),
    resolveRules({ repository_id: 'app-frontend', work_kind: 'initiative', candidate_paths: [], changed_paths: [] }, assemble(STD, { prior_state: { initiative_protocol: 'consolidation-decomposition', pre_decomposition_norms: ['a'], source_repository_ids: ['app-frontend'] } })),
    resolveRules({ repository_id: 'app-frontend', work_kind: 'operation', candidate_paths: [], changed_paths: [] }, assemble(STD)),
  ];
  results.forEach((r, i) => {
    const errs = [];
    validate(r, OUTPUT_SCHEMA, OUTPUT_SCHEMA, '', errs);
    assert(errs.length === 0, `result ${i}: ${errs[0]}`);
  });
});
check('glob — the supported grammar behaves and refuses the rest', () => {
  assert(globToRegExp('packages/**/api/*.ts').test('packages/a/b/api/x.ts'), '** should span segments');
  assert(globToRegExp('packages/**/api/*.ts').test('packages/api/x.ts'), '** should also span zero segments');
  assert(!globToRegExp('api/*.ts').test('api/sub/x.ts'), '* must not cross /');
  assert(globToRegExp('**/ui/*.vue').test('packages/features/cart/ui/C.vue'), 'leading **');
  for (const g of ['a/[abc].ts', 'a/!(x).ts', 'a/@(x|y).ts', 'a**b/x.ts']) {
    throws(() => globToRegExp(g), /unsupported syntax|not a whole path segment/, `glob "${g}" should be refused`);
  }
});

// ===========================================================================
// CLI smoke — the wrapper is exercised at least at its edges
// ===========================================================================
check('cli — --help exits 0 and prints usage', () => {
  const r = spawnSync(process.execPath, [RESOLVER, '--help'], { encoding: 'utf8' });
  assert(r.status === 0 && /Usage:/.test(r.stdout), `--help: exit ${r.status}`);
});
check('cli — a missing MERIDIAN_INSTANCE is fail-closed (exit 2)', () => {
  const env = { ...process.env };
  delete env.MERIDIAN_INSTANCE;
  const r = spawnSync(process.execPath, [RESOLVER, '--request', 'x.json', '--applicability', 'y.json'], { encoding: 'utf8', env });
  assert(r.status === 2 && /MERIDIAN_INSTANCE is not set/.test(r.stderr), `expected fail-closed, got exit ${r.status}`);
});
check('cli — a missing required flag is fail-closed (exit 2)', () => {
  const r = spawnSync(process.execPath, [RESOLVER, '--applicability', 'y.json'], {
    encoding: 'utf8', env: { ...process.env, MERIDIAN_INSTANCE: __dirname },
  });
  assert(r.status === 2 && /--request <file> is required/.test(r.stderr), `expected fail-closed, got exit ${r.status}`);
});

// A self-contained Instance dir plus request/applicability files, so the CLI is
// exercised end to end (arg parsing → envelope validation → resolve → JSON).
function withCliSandbox(fn) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'rr-cli-'));
  try {
    fs.mkdirSync(path.join(dir, 'inventory'));
    fs.writeFileSync(path.join(dir, 'inventory', 'repositories.yaml'),
      'schema_version: 1\nrepositories:\n  - id: demo-repo\n    profile: demo\n');
    const reqPath = path.join(dir, 'request.json');
    fs.writeFileSync(reqPath, JSON.stringify({
      work_item: { repository_id: 'demo-repo', work_kind: 'assessment', candidate_paths: [], changed_paths: [] },
    }));
    const run = (applDoc) => {
      const applPath = path.join(dir, 'applicability.json');
      fs.writeFileSync(applPath, typeof applDoc === 'string' ? applDoc : JSON.stringify(applDoc));
      return spawnSync(process.execPath, [RESOLVER, '--request', reqPath, '--applicability', applPath], {
        encoding: 'utf8', env: { ...process.env, MERIDIAN_INSTANCE: dir },
      });
    };
    return fn(run);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

check('cli — a valid applicability envelope resolves and exits 0', () => {
  withCliSandbox((run) => {
    const r = run({ $schema: './applicability.schema.json', schema_version: 1, records: [] });
    assert(r.status === 0, `expected exit 0, got ${r.status}; stderr: ${r.stderr}`);
    const out = JSON.parse(r.stdout);
    assert(Array.isArray(out.applicable_norms) && out.applicable_norms.length === 0, 'expected an empty applicable_norms');
    const errs = [];
    validate(out, OUTPUT_SCHEMA, OUTPUT_SCHEMA, '', errs);
    assert(errs.length === 0, `output schema: ${errs[0]}`);
  });
});

for (const [label, doc] of [
  ['records key absent', { $schema: './applicability.schema.json', schema_version: 1 }],
  ['a bare empty object', {}],
  ['a wrong schema_version', { $schema: './applicability.schema.json', schema_version: 2, records: [] }],
  ['records that is not an array', { $schema: './applicability.schema.json', schema_version: 1, records: {} }],
  ['an additional top-level property', { $schema: './applicability.schema.json', schema_version: 1, records: [], extra: true }],
  ['a top-level raw array', [] ],
]) {
  check(`cli — a malformed applicability envelope (${label}) is fail-closed (exit 2)`, () => {
    withCliSandbox((run) => {
      const r = run(doc);
      assert(r.status === 2, `expected exit 2, got ${r.status}; stderr: ${r.stderr}`);
      assert(/applicability\.schema\.json|does not satisfy/.test(r.stderr),
        `stderr did not name the schema failure: ${r.stderr}`);
    });
  });
}

// ---------------------------------------------------------------------------
console.log(`\n${passed} passed, ${failed} failed`);
if (failed) {
  console.log('\n--- failures ---');
  for (const f of failures) console.log(f);
  process.exit(1);
}
