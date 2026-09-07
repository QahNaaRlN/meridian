#!/usr/bin/env node
// Regression suite for scripts/rule-resolver.mjs (PHASE C).
//
// The ten acceptance cases of §4 / §9 of the normative model are each covered
// by a named test — cases 9 and 10 are separate tests — plus append-only
// precedence in both directions, same-day precedence via an explicit
// `supersedes` relationship. EVERY declared `supersedes` relationship is
// validated before an authoritative record is returned — in the greatest-date
// cohort and in every historical cohort — and a `supersedes` that cannot name
// exactly one same-date target fails closed, including on a record that is
// alone on its applicability date. Covered: valid head selection, a
// no-relationship tie fail-closed, the intake verdict never a precedence key,
// a missing / cross-identity / self / cyclic pointer on a unique latest record,
// an invalid relationship in a historical cohort while a newer unique record
// exists, a valid historical same-day cohort followed by a newer unique date,
// ambiguous / missing target, multiple incomparable heads, different-date
// precedence unchanged, byte-identical output under reversed input for every
// successful case, and the schema shape at the resolver boundary. Then
// authoritative current-source provenance: the schema shape and the supersedes
// graph of every cohort stay fail-closed, the exact intake pointer of EVERY
// matching non-kernel record (historical included) still resolves fail-closed,
// but the single current text is hashed only against the AUTHORITATIVE record's
// digest — a legitimately changed text no longer lets an older record's now-stale
// digest block a newer authoritative record, while a stale AUTHORITATIVE digest
// is still fail-closed. Then protocol-route scope isolation, provenance of a lone
// (authoritative) unresolved / undetermined record, strict work-item validation,
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
//   5  "acceptance 5 — REFACTOR routes its own protocol with provenance"
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

// case 5 — a pure REFACTOR now routes its own canonical protocol, with
// source/scope/digest provenance, and no longer sits in unresolved_applicability
// (the PHASE D evidence contract and the PHASE E execution protocol exist).
check('acceptance 5 — REFACTOR routes its own protocol with provenance', () => {
  const out = resolveRules({
    repository_id: 'app-frontend', work_kind: 'change', change_class: 'REFACTOR',
    candidate_paths: ['packages/entities/order/model/mappers.ts'], changed_paths: [],
  }, assemble(STD));
  const se = [];
  validate(out, OUTPUT_SCHEMA, OUTPUT_SCHEMA, '', se);
  assert(se.length === 0, `output schema: ${se[0]}`);
  const rp = out.applicable_protocols.find((e) => e.protocol === 'refactor-protocol');
  assert(rp && rp.routed_from === 'REFACTOR' && rp.source === 'kernel' && rp.scope === 'universal',
    'REFACTOR must route refactor-protocol from a kernel universal route');
  assert((rp.digest && /^[0-9a-f]{64}$/.test(rp.digest)) || (rp.revision && /^[0-9a-f]{7,40}$/.test(rp.revision)),
    'the refactor-protocol route must carry a digest or a revision');
  assert(!out.unresolved_applicability.some((x) => x.subject === 'REFACTOR'), 'REFACTOR is no longer unresolved');
  assert(!hasProtocol(out, 'bugfix-protocol') && !hasProtocol(out, 'test-planning'), 'no other class’s protocol for REFACTOR');
});

// case 6 — a defect found during a REFACTOR needs a separate BUGFIX child /
// owner decision and does not reclassify the original work.
check('acceptance 6 — defect in REFACTOR: separate BUGFIX child, original not reclassified', () => {
  const src = assemble(STD, { prior_state: { refactor_findings: [{ type: 'behavior-change' }] } });
  const parent = resolveRules({
    repository_id: 'app-frontend', work_kind: 'change', change_class: 'REFACTOR',
    candidate_paths: ['packages/entities/order/model/mappers.ts'], changed_paths: [],
  }, src);
  const f = parent.unresolved_applicability.find((u) => u.subject === 'refactor-in-progress-finding');
  assert(f && /BUGFIX child work item/.test(f.reason) && /not reclassified/.test(f.reason), 'finding entry missing/weak');
  assert(!parent.unresolved_applicability.some((u) => u.subject === 'REFACTOR'), 'the REFACTOR class itself is not unresolved');
  const prp = parent.applicable_protocols.find((e) => e.protocol === 'refactor-protocol' && e.routed_from === 'REFACTOR');
  assert(prp, 'the parent REFACTOR keeps its own protocol route alongside the finding');
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
// authoritative current-source provenance (rule-resolution.md §9):
//   - the schema shape and the supersedes graph of every cohort are still
//     validated fail-closed;
//   - the exact intake pointer of EVERY matching non-kernel record still
//     resolves fail-closed, historical records included;
//   - the single current text is hashed only against the AUTHORITATIVE record's
//     digest — a historical record's now-stale digest no longer blocks a newer
//     authoritative record;
//   - a stale AUTHORITATIVE digest is still fail-closed.
// ===========================================================================
// A pair of append-only applicability records for ONE norm identity whose text
// legitimately changed between them: the older record's digest is of the old
// text, the newer record's digest is of the new text, and the single supplied
// current text is the NEW one. Each record carries its own distinct intake
// pointer so a broken historical pointer can be exercised in isolation.
const CHG_OLD_TEXT = '# Changed\n\nThe original text of an append-only norm.\n';
const CHG_NEW_TEXT = '# Changed\n\nThe rewritten text after a legitimate edit.\n';
const CHG_OLD_ID = 'instruction-intake/app-frontend.yaml#rules/changed.md@2026-08-01/keep-local';
const CHG_NEW_ID = 'instruction-intake/app-frontend.yaml#rules/changed.md@2026-08-02/adopt-core';
const chgWi = {
  repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE',
  candidate_paths: ['x'], changed_paths: [],
};
function changedTextPair(olderStatus, newerStatus, opts = {}) {
  const { staleAuthoritative = false, breakHistoricalPointer = false, order = 'forward' } = opts;
  const base = {
    norm: { repository: 'app-frontend', path: 'rules/changed.md' },
    scope: 'repository', repository: 'app-frontend',
    activation: 'always', source: 'repository',
  };
  const older = {
    ...base, digest: sha256(CHG_OLD_TEXT), status: olderStatus, recorded_at: '2026-08-10',
    intake_record: { register: 'instruction-intake/app-frontend.yaml', recorded_at: '2026-08-01', verdict: 'keep-local' },
  };
  if (olderStatus === 'unresolved') older.resume_condition = 'resume when the 2026-08-10 record is revisited';
  const newer = {
    ...base,
    digest: staleAuthoritative ? sha256('a text that is neither the old nor the new one\n') : sha256(CHG_NEW_TEXT),
    status: newerStatus, recorded_at: '2026-08-20',
    intake_record: { register: 'instruction-intake/app-frontend.yaml', recorded_at: '2026-08-02', verdict: 'adopt-core' },
  };
  if (newerStatus === 'unresolved') newer.resume_condition = 'resume when the 2026-08-20 record is revisited';
  const src = assemble(['kernel-conduct']);
  const pair = order === 'reversed' ? [newer, older] : [older, newer];
  src.applicability_records.push(...pair);
  src.norm_texts['app-frontend::rules/changed.md'] = CHG_NEW_TEXT;
  src.intake_registers = [{
    register: 'instruction-intake/app-frontend.yaml', repository: 'app-frontend',
    records: [
      ...(breakHistoricalPointer ? [] : [{ artifact: 'rules/changed.md', recorded_at: '2026-08-01', verdict: 'keep-local', delivery: 'cursor-rule' }]),
      { artifact: 'rules/changed.md', recorded_at: '2026-08-02', verdict: 'adopt-core', delivery: 'agents-md-section' },
    ],
  }];
  return src;
}

check('provenance — a newer resolved record supersedes an older unresolved of the same identity whose digest is now stale', () => {
  const out = resolveRules(chgWi, changedTextPair('unresolved', 'resolved'));
  assert(out.applicable_norms.some((e) => e.norm === CHG_NEW_ID), 'the newer resolved record must be authoritative and applicable');
  const e = out.applicable_norms.find((x) => x.norm === CHG_NEW_ID);
  assert(e.digest === sha256(CHG_NEW_TEXT) && e.delivery === 'agents-md-section', 'authoritative digest and delivery come from the newer record only');
  assert(!out.applicable_norms.some((e2) => e2.norm === CHG_OLD_ID), 'the older record must not be applicable');
  assert(!out.unresolved_applicability.some((u) => u.subject === CHG_OLD_ID || u.subject === CHG_NEW_ID), 'no unresolved entry — the stale historical digest is not recomputed against the current text');
});

check('provenance — a newer unresolved record suppresses an older resolved of the same identity, historical digest not recomputed', () => {
  const out = resolveRules(chgWi, changedTextPair('resolved', 'unresolved'));
  assert(!out.applicable_norms.some((e) => e.norm === CHG_OLD_ID), 'the older resolved record must not survive as applicable');
  const u = out.unresolved_applicability.find((x) => x.subject === CHG_NEW_ID);
  assert(u && /2026-08-20/.test(u.reason), 'the authoritative newer unresolved record sets the outcome and its own reason');
});

check('provenance — a stale AUTHORITATIVE digest is still fail-closed', () => {
  throws(() => resolveRules(chgWi, changedTextPair('unresolved', 'resolved', { staleAuthoritative: true })), /stale digest/);
});

check('provenance — a missing intake pointer on a matching HISTORICAL record is fail-closed despite a newer authoritative record', () => {
  throws(() => resolveRules(chgWi, changedTextPair('unresolved', 'resolved', { breakHistoricalPointer: true })), /resolves to no record in|not among the supplied intake registers/);
});

check('provenance — the changed-text resolution is byte-identical under reversed input order', () => {
  const forward = JSON.stringify(resolveRules(chgWi, changedTextPair('unresolved', 'resolved', { order: 'forward' })));
  const reversed = JSON.stringify(resolveRules(chgWi, changedTextPair('unresolved', 'resolved', { order: 'reversed' })));
  assert(forward === reversed, `output differs under reordering:\n${forward}\n${reversed}`);
});

// ===========================================================================
// same-day append-only precedence via an explicit `supersedes` relationship
// (rule-resolution.md §4, §9; MERIDIAN-RULE-RESOLUTION — same-day precedence)
// ===========================================================================
// Append-only applicability records for ONE norm identity, each with its own
// intake pointer (a distinct verdict), built inline so the only thing under
// test is the `supersedes` wiring. `recorded_at` per spec puts records into
// applicability-date cohorts. Provenance is made to pass (one norm text,
// digests recomputed, an intake register with an entry per verdict) so a valid
// relationship reaches applicable_norms. Every declared `supersedes` is
// validated — current cohort and historical — before a record is returned;
// a `supersedes` with no same-date target fails closed, never ignored.
const SUP_TEXT = '# Supersede\n\nA rule under same-day precedence test.\n';
function supersedeSet(specs) {
  const src = assemble(['kernel-conduct']);
  const base = {
    norm: { repository: 'app-frontend', path: 'rules/supersede.md' },
    digest: sha256(SUP_TEXT),
    scope: 'repository', repository: 'app-frontend',
    activation: 'always', source: 'repository',
  };
  for (const s of specs) {
    const r = {
      ...base,
      intake_record: { register: 'instruction-intake/app-frontend.yaml', recorded_at: '2026-08-01', verdict: s.verdict },
      status: s.status || 'resolved',
      recorded_at: s.recorded_at || '2026-08-29',
    };
    if ((s.status || 'resolved') === 'unresolved') r.resume_condition = `resume when the ${s.verdict} record is revisited`;
    if (s.supersedes) r.supersedes = s.supersedes;
    src.applicability_records.push(r);
  }
  src.norm_texts['app-frontend::rules/supersede.md'] = SUP_TEXT;
  src.intake_registers = [{
    register: 'instruction-intake/app-frontend.yaml', repository: 'app-frontend',
    records: ['keep-local', 'deferred', 'adopt-edition', 'adopt-core'].map((verdict) => ({
      artifact: 'rules/supersede.md', recorded_at: '2026-08-01', verdict, delivery: 'cursor-rule',
    })),
  }];
  return src;
}
const supWi = {
  repository_id: 'app-frontend', work_kind: 'change', change_class: 'FEATURE',
  candidate_paths: ['x'], changed_paths: [],
};
const supId = (v) => `instruction-intake/app-frontend.yaml#rules/supersede.md@2026-08-01/${v}`;
const supPtr = (v, over = {}) => ({
  register: 'instruction-intake/app-frontend.yaml', path: 'rules/supersede.md',
  recorded_at: '2026-08-01', verdict: v, ...over,
});

// 1 — a valid explicit supersession selects a unique authoritative head, and the
//     result is byte-identical under reversed input order.
check('supersedes — the declared successor is authoritative on a same-day pair', () => {
  const specs = [
    { verdict: 'deferred', status: 'unresolved' },
    { verdict: 'keep-local', status: 'resolved', supersedes: supPtr('deferred') },
  ];
  const out = resolveRules(supWi, supersedeSet(specs));
  assert(out.applicable_norms.some((e) => e.norm === supId('keep-local')), 'the keep-local successor must be authoritative');
  assert(!out.applicable_norms.some((e) => e.norm === supId('deferred')), 'the superseded deferred record must not win');
  assert(!out.unresolved_applicability.some((u) => u.subject === supId('deferred')), 'the superseded record must not surface as unresolved');
});
check('supersedes — resolution is byte-identical under reversed input order', () => {
  const specs = [
    { verdict: 'deferred', status: 'unresolved' },
    { verdict: 'keep-local', status: 'resolved', supersedes: supPtr('deferred') },
  ];
  const forward = JSON.stringify(resolveRules(supWi, supersedeSet(specs)));
  const reversed = JSON.stringify(resolveRules(supWi, supersedeSet([...specs].reverse())));
  assert(forward === reversed, `output differs under reordering:\n${forward}\n${reversed}`);
});

// 2 — the same pair with no explicit relationship stays fail-closed; the intake
//     verdict does not by itself order them (no adopt-edition > deferred).
check('supersedes — a same-day pair with no supersedes relationship stays fail-closed', () => {
  const src = supersedeSet([
    { verdict: 'deferred', status: 'unresolved' },
    { verdict: 'adopt-edition', status: 'unresolved' },
  ]);
  throws(() => resolveRules(supWi, src),
    /no "supersedes" relationship orders them|append-only precedence cannot be decided/);
});
check('supersedes — intake verdict alone never orders a same-day pair', () => {
  // adopt-edition is "stronger" as an intake verdict, but nothing here says it
  // supersedes the deferred record, so the resolver must not pick it.
  const src = supersedeSet([
    { verdict: 'adopt-edition', status: 'resolved' },
    { verdict: 'deferred', status: 'resolved' },
  ]);
  throws(() => resolveRules(supWi, src), /no "supersedes" relationship orders them/);
});

// 3 — a supersedes target that does not exist among the same-day records.
check('supersedes — a pointer to a non-existent record is fail-closed', () => {
  const src = supersedeSet([
    { verdict: 'deferred', status: 'unresolved' },
    { verdict: 'keep-local', status: 'resolved', supersedes: supPtr('adopt-core') },
  ]);
  throws(() => resolveRules(supWi, src), /resolves to no applicability record/);
});

// 4 — a supersedes pointer that names a different norm identity.
check('supersedes — a cross-norm-identity pointer is fail-closed', () => {
  const src = supersedeSet([
    { verdict: 'deferred', status: 'unresolved' },
    { verdict: 'keep-local', status: 'resolved', supersedes: supPtr('deferred', { path: 'rules/other.md' }) },
  ]);
  throws(() => resolveRules(supWi, src), /different norm identity/);
});

// 5 — a self-referential supersedes pointer.
check('supersedes — a self-referential pointer is fail-closed', () => {
  const src = supersedeSet([
    { verdict: 'deferred', status: 'unresolved' },
    { verdict: 'keep-local', status: 'resolved', supersedes: supPtr('keep-local') },
  ]);
  throws(() => resolveRules(supWi, src), /supersedes itself/);
});

// 6 — a cycle in the supersedes relationship.
check('supersedes — a cycle in the relationship is fail-closed', () => {
  const src = supersedeSet([
    { verdict: 'deferred', status: 'resolved', supersedes: supPtr('keep-local') },
    { verdict: 'keep-local', status: 'resolved', supersedes: supPtr('deferred') },
  ]);
  throws(() => resolveRules(supWi, src), /forms a cycle/);
});

// 7 — multiple incomparable heads: a third same-day record that nothing orders.
check('supersedes — multiple incomparable heads are fail-closed', () => {
  const src = supersedeSet([
    { verdict: 'deferred', status: 'unresolved' },
    { verdict: 'keep-local', status: 'resolved', supersedes: supPtr('deferred') },
    { verdict: 'adopt-edition', status: 'resolved' },
  ]);
  throws(() => resolveRules(supWi, src), /un-superseded head\(s\)/);
});

// 8 — a `supersedes` on the UNIQUE greatest-date record is not harmless stray
//     data: it has no same-date target, so it must fail closed (replaces the
//     former "ignores a stray supersedes" test).
check('supersedes — a dangling pointer on the unique latest record is fail-closed', () => {
  const src = supersedeSet([
    { verdict: 'deferred', status: 'unresolved', recorded_at: '2026-08-10' },
    { verdict: 'keep-local', status: 'resolved', recorded_at: '2026-08-20', supersedes: supPtr('adopt-core') },
  ]);
  throws(() => resolveRules(supWi, src), /resolves to no applicability record/);
});
check('supersedes — a cross-identity pointer on the unique latest record is fail-closed', () => {
  const src = supersedeSet([
    { verdict: 'deferred', status: 'unresolved', recorded_at: '2026-08-10' },
    { verdict: 'keep-local', status: 'resolved', recorded_at: '2026-08-20', supersedes: supPtr('deferred', { path: 'rules/other.md' }) },
  ]);
  throws(() => resolveRules(supWi, src), /different norm identity/);
});
check('supersedes — a self-pointer on the unique latest record is fail-closed', () => {
  const src = supersedeSet([
    { verdict: 'deferred', status: 'unresolved', recorded_at: '2026-08-10' },
    { verdict: 'keep-local', status: 'resolved', recorded_at: '2026-08-20', supersedes: supPtr('keep-local') },
  ]);
  throws(() => resolveRules(supWi, src), /supersedes itself/);
});
check('supersedes — a pointer to a record on another applicability date is fail-closed', () => {
  // keep-local (latest, 2026-08-20) points at the deferred record's intake
  // identity, but that record lives in the 2026-08-10 cohort — not the carrier's.
  const src = supersedeSet([
    { verdict: 'deferred', status: 'unresolved', recorded_at: '2026-08-10' },
    { verdict: 'keep-local', status: 'resolved', recorded_at: '2026-08-20', supersedes: supPtr('deferred') },
  ]);
  throws(() => resolveRules(supWi, src), /does not share this record's applicability recorded_at/);
});

// 8b — an INVALID relationship in a historical (non-authoritative) cohort fails
//      closed even though a newer unique record would otherwise be authoritative.
check('supersedes — a cyclic relationship in a historical cohort fails closed despite a newer unique record', () => {
  const src = supersedeSet([
    { verdict: 'deferred', status: 'resolved', recorded_at: '2026-08-10', supersedes: supPtr('keep-local') },
    { verdict: 'keep-local', status: 'resolved', recorded_at: '2026-08-10', supersedes: supPtr('deferred') },
    { verdict: 'adopt-edition', status: 'resolved', recorded_at: '2026-08-20' },
  ]);
  throws(() => resolveRules(supWi, src), /forms a cycle/);
});
check('supersedes — a historical cohort that declares ordering but leaves two heads fails closed', () => {
  const src = supersedeSet([
    { verdict: 'deferred', status: 'unresolved', recorded_at: '2026-08-10' },
    { verdict: 'keep-local', status: 'resolved', recorded_at: '2026-08-10', supersedes: supPtr('deferred') },
    { verdict: 'adopt-edition', status: 'resolved', recorded_at: '2026-08-10' },
    { verdict: 'adopt-core', status: 'resolved', recorded_at: '2026-08-20' },
  ]);
  throws(() => resolveRules(supWi, src), /un-superseded head\(s\)/);
});

// 8c — a VALID historical same-day cohort validates, and then the newer unique
//      date wins; order-independent.
check('supersedes — a valid historical same-day cohort validates, then the newer unique date wins', () => {
  const specs = [
    { verdict: 'deferred', status: 'unresolved', recorded_at: '2026-08-10' },
    { verdict: 'keep-local', status: 'resolved', recorded_at: '2026-08-10', supersedes: supPtr('deferred') },
    { verdict: 'adopt-edition', status: 'resolved', recorded_at: '2026-08-20' },
  ];
  const out = resolveRules(supWi, supersedeSet(specs));
  assert(out.applicable_norms.some((e) => e.norm === supId('adopt-edition')), 'the newer unique date must be authoritative');
  assert(!out.applicable_norms.some((e) => e.norm === supId('keep-local')), 'the historical cohort head is not authoritative');
  assert(!out.unresolved_applicability.some((u) => u.subject === supId('deferred')), 'a superseded historical record must not surface');
  const reversed = JSON.stringify(resolveRules(supWi, supersedeSet([...specs].reverse())));
  assert(JSON.stringify(out) === reversed, 'resolution must be byte-identical under reversed input order');
});

// 8c-bis — the relaxed current-text provenance check does NOT weaken supersedes
//          validation: a dangling supersedes in a historical cohort still fails
//          closed even though a newer unique record would be authoritative and
//          its digest verifies.
check('supersedes — a dangling supersedes in a historical cohort still fails closed under the authoritative-only text check', () => {
  const specs = [
    { verdict: 'deferred', status: 'unresolved', recorded_at: '2026-08-10' },
    { verdict: 'keep-local', status: 'resolved', recorded_at: '2026-08-10', supersedes: supPtr('adopt-core') },
    { verdict: 'adopt-edition', status: 'resolved', recorded_at: '2026-08-20' },
  ];
  throws(() => resolveRules(supWi, supersedeSet(specs)),
    /resolves to no applicability record|does not share this record's applicability recorded_at|un-superseded head/);
});

// 8d — different-date records with no supersedes relation keep the existing
//      precedence (a newer unresolved suppresses an older resolved), unchanged.
check('supersedes — different-date records with no relationship keep existing precedence', () => {
  const specs = [
    { verdict: 'keep-local', status: 'resolved', recorded_at: '2026-08-10' },
    { verdict: 'deferred', status: 'unresolved', recorded_at: '2026-08-20' },
  ];
  const out = resolveRules(supWi, supersedeSet(specs));
  assert(!out.applicable_norms.some((e) => e.norm === supId('keep-local')), 'older resolved must not win');
  assert(out.unresolved_applicability.some((u) => u.subject === supId('deferred')), 'newer unresolved must set the outcome');
  const reversed = JSON.stringify(resolveRules(supWi, supersedeSet([...specs].reverse())));
  assert(JSON.stringify(out) === reversed, 'different-date resolution must be order-independent');
});

// 9 — schema shape of the relationship is enforced at the resolver boundary too.
check('supersedes — a malformed supersedes pointer is rejected as a malformed record', () => {
  const src = supersedeSet([
    { verdict: 'deferred', status: 'unresolved' },
    { verdict: 'keep-local', status: 'resolved', supersedes: { register: 'instruction-intake/app-frontend.yaml', path: 'rules/supersede.md', recorded_at: '2026-08-01' } },
  ]);
  throws(() => resolveRules(supWi, src), /malformed applicability record/);
});
check('supersedes — a kernel-source record must not carry supersedes (malformed record)', () => {
  const src = assemble(['kernel-conduct']);
  src.applicability_records.push({
    norm: { repository: 'kernel', path: 'standards/workspace/x.md' },
    digest: sha256(SUP_TEXT), scope: 'universal', activation: 'always', source: 'kernel',
    status: 'resolved', recorded_at: '2026-08-29',
    supersedes: supPtr('keep-local', { path: 'standards/workspace/x.md' }),
  });
  src.norm_texts['kernel::standards/workspace/x.md'] = SUP_TEXT;
  throws(() => resolveRules(supWi, src), /malformed applicability record/);
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
// provenance of a lone (hence authoritative) matching record — resolved,
// unresolved and activation:undetermined alike — is fail-closed on a stale
// digest, on missing current text, and on an unresolvable intake pointer
// (normative model §4; rule-resolution.md §9). When a norm identity carries
// several append-only records the current-text check applies to the
// authoritative record only, while every matching non-kernel record still has
// its intake pointer resolved — see the "authoritative current-source
// provenance" and "supersedes" blocks above.
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
