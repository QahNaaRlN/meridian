// Regenerates the synthetic frozen-Instance fixture of package
// `meridian-cli-migration` (meridian-cli/tests/binary_runs.rs).
//
//   node meridian-cli/tests/fixtures/frozen-instance/generate.mjs
//
// The pinned source is a Git commit with a FIXED author, committer, date
// and message over the files in `source/`, so its commit id is the same on
// every machine; the bundle in `bundle/` pins that id. Every fingerprint,
// key and digest is computed by the Kernel's own Node reference
// (scripts/lib/instance-data-migration.mjs), and the generated bundle is
// checked by the reference evaluators before it is written — so the Rust
// tests exercise a bundle both implementations accept. The fixture carries
// no product data.

import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const KERNEL = path.resolve(HERE, '..', '..', '..', '..');
const lib = await import(path.join(KERNEL, 'scripts/lib/instance-data-migration.mjs'));
const { yamlParse } = await import(path.join(KERNEL, 'scripts/lib/yaml.mjs'));

const readJson = (p) => JSON.parse(fs.readFileSync(path.join(KERNEL, p), 'utf8'));
const planSchema = readJson('registries/operating-model/instance-data-migration.schema.json');
const exportSchema = readJson('registries/operating-model/instance-canonical-export.schema.json');
const envelopeSchema = readJson('registries/operating-model/scoped-record.schema.json');

export const COMMIT_ENV = {
  GIT_AUTHOR_NAME: 'Meridian Fixture',
  GIT_AUTHOR_EMAIL: 'fixture@meridian.invalid',
  GIT_AUTHOR_DATE: '2026-01-01T00:00:00+00:00',
  GIT_COMMITTER_NAME: 'Meridian Fixture',
  GIT_COMMITTER_EMAIL: 'fixture@meridian.invalid',
  GIT_COMMITTER_DATE: '2026-01-01T00:00:00+00:00',
};

const SOURCE = path.join(HERE, 'source');
const BUNDLE = path.join(HERE, 'bundle');
const SCHEMA = 'registries/operating-model/scoped-record.schema.json';
const REPOSITORY_REF = 'sample-frozen-instance';
const PLAN_ID = 'sample-frozen-instance-plan';
const EXPORT_ID = 'sample-frozen-instance-export';
const WORKSPACE = { type: 'project-workspace', id: 'sample' };
const REPOSITORY = { type: 'repository-scope', id: 'sample-repo', workspace_id: 'sample' };
const DECISION = 'owner-decision:sample-frozen-instance-migration';

const git = (cwd, args, env = {}) =>
  execFileSync('git', ['-C', cwd, '-c', 'core.autocrlf=false', '-c', 'commit.gpgsign=false', ...args], {
    encoding: 'utf8',
    env: { ...process.env, ...env },
  });

function listFiles(dir, prefix = '') {
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
    e.isDirectory() ? listFiles(path.join(dir, e.name), `${prefix}${e.name}/`) : [`${prefix}${e.name}`],
  );
}

const repo = fs.mkdtempSync(path.join(os.tmpdir(), 'meridian-frozen-fixture-'));
try {
  git(repo, ['init', '-q']);
  for (const rel of listFiles(SOURCE)) {
    fs.mkdirSync(path.dirname(path.join(repo, rel)), { recursive: true });
    fs.copyFileSync(path.join(SOURCE, rel), path.join(repo, rel));
  }
  git(repo, ['add', '-A']);
  git(repo, ['commit', '-q', '-m', 'frozen instance fixture'], COMMIT_ENV);
  const revision = git(repo, ['rev-parse', 'HEAD']).trim();
  const listing = git(repo, ['ls-tree', '-r', '--format=%(objectmode) %(objecttype) %(objectname) %(path)', revision]);
  const manifest = listing.trim().split('\n').filter(Boolean).sort().join('\n') + '\n';
  const treeDigest = createHash('sha256').update(manifest).digest('hex');
  const show = (p) => execFileSync('git', ['-C', repo, 'show', `${revision}:${p}`]);

  const canonicalize = (v) => {
    if (Array.isArray(v)) return v.map(canonicalize);
    if (v !== null && typeof v === 'object') {
      const out = {};
      for (const k of Object.keys(v).sort()) out[k] = canonicalize(v[k]);
      return out;
    }
    return v;
  };
  const sha = (x) => createHash('sha256').update(x).digest('hex');
  const jsonEnvelope = (value) => {
    const content = JSON.stringify(canonicalize(value));
    return { media_type: 'application/json', encoding: 'utf-8', content, digest: { algorithm: 'sha-256', value: sha(content) } };
  };
  const textEnvelope = (p, mediaType) => {
    const buf = show(p);
    return { media_type: mediaType, encoding: 'utf-8', content: buf.toString('utf8'), digest: { algorithm: 'sha-256', value: sha(buf) } };
  };

  const items = yamlParse(show('inventory/items.yaml').toString('utf8')).items;
  const rules = yamlParse(show('rule-resolution/applicability.yaml').toString('utf8')).records;
  const units = [
    { id: 'notes-readme-txt', unit_ref: 'notes/readme.txt', scope: WORKSPACE, type: 'workspace-file', title: 'notes/readme.txt', payload: textEnvelope('notes/readme.txt', 'text/plain') },
    ...items.map((item) => ({ id: `inventory-items-${item.id.replace(/ /g, "-")}`, unit_ref: `inventory/items.yaml#items/${encodeURIComponent(item.id)}`, scope: WORKSPACE, type: 'inventory-item', title: `Item ${item.id}`, payload: jsonEnvelope(item) })),
    ...rules.map((rule, i) => ({ id: `rule-applicability-${i}`, unit_ref: `rule-resolution/applicability.yaml#records/${i}`, scope: REPOSITORY, type: 'rule-applicability-record', title: `Applicability of ${rule.norm.path}#${rule.norm.region}`, payload: jsonEnvelope(rule) })),
    { id: 'foreign-keep-txt', unit_ref: 'foreign/keep.txt', retained: 'foreign content stays transitional until its owner decides' },
  ];

  const makeBundle = (units, sourceDigest) => {
    const target = (u) => ({
      $schema: SCHEMA,
      id: u.id,
      title: u.title,
      record_type: u.type,
      scope: u.scope,
      schema_version: 1,
      field_basis: { $schema: 'assigned', id: 'assigned', title: 'assigned', record_type: 'assigned', scope: 'assigned', schema_version: 'assigned', origin: 'assigned', authority: 'assigned', payload: 'preserved' },
      origin: { kind: 'migrated', source_ref: `record-unit:${u.id}` },
      authority: { kind: lib.TARGET_AUTHORITY_BY_SCOPE[u.scope.type], authority_ref: 'sample-owner', decision_ref: DECISION },
      payload: u.payload,
    });
    const source = { revision, digest: { algorithm: 'sha-256', value: sourceDigest }, repository_ref: REPOSITORY_REF, working_tree_clean: true, qualification: 'reproducible' };
    const snapshotRef = `snapshot:${REPOSITORY_REF}@${revision}`;
    const deterministicRef = `deterministic-plan:${PLAN_ID}`;
    const payload = {
      source,
      record_units: units.map((u) => ({ id: u.id, unit_ref: u.unit_ref, classification_basis: `fixture unit ${u.id}, classified by its own carrier and position` })),
      mappings: units.map((u) => (u.retained
        ? { unit_id: u.id, disposition: 'retained-transitional', retained_reason: u.retained }
        : { unit_id: u.id, disposition: 'migrated', target: target(u), owner_decision: { decision_ref: DECISION, decided_at: '2026-01-01', reason: 'the fixture owner accepted the migration of every own unit' } })),
      rollback: { source_snapshot_ref: snapshotRef, plan: 'deterministic-reconstruction', deterministic_plan_ref: deterministicRef, rewrites_published_history: false },
      verification: {
        coverage: { status: 'verified', evidence_ref: `evidence:coverage-${PLAN_ID}` },
        applicability_preservation: { status: 'verified', evidence_ref: `evidence:applicability-${PLAN_ID}` },
        overall_status: 'VERIFIED',
      },
    };
    payload.plan_fingerprint = lib.computePlanFingerprint(payload);
    payload.idempotency_key = lib.computeIdempotencyKey(WORKSPACE, source);
    const fingerprint = payload.plan_fingerprint;
    const plan = {
      $schema: SCHEMA, schema_version: 1, id: PLAN_ID, title: 'Sample frozen-Instance migration plan', record_type: lib.RECORD_TYPE, scope: WORKSPACE,
      origin: { kind: 'declared', source_ref: 'owner-directive:sample-frozen-instance' },
      authority: { kind: 'delegated-run', authority_ref: `instance-data-migration-run:${PLAN_ID}` },
      payload,
    };
    const registry = { $schema: '../../registries/operating-model/instance-data-migration.schema.json', schema_version: 1, registry_id: 'instance-data-migration', title: 'Sample frozen-Instance migration', migration_plans: [plan] };

    const records = units.filter((u) => !u.retained).map((u) => {
      const t = target(u);
      delete t.field_basis;
      return t;
    });
    const exportPayload = {
      plan_ref: PLAN_ID,
      plan_fingerprint: fingerprint,
      source: { repository_ref: REPOSITORY_REF, revision, digest: source.digest },
      records,
      retained: units.filter((u) => u.retained).map((u) => ({ unit_id: u.id, reason: u.retained })),
    };
    exportPayload.idempotency_key = lib.computeExportIdempotencyKey(PLAN_ID, fingerprint);
    exportPayload.digest = lib.computeExportDigest(exportPayload);
    const canonicalExport = {
      $schema: '../../registries/operating-model/instance-canonical-export.schema.json', schema_version: 1, registry_id: 'instance-canonical-export', title: 'Sample frozen-Instance canonical export',
      exports: [{
        $schema: SCHEMA, schema_version: 1, id: EXPORT_ID, title: 'Sample canonical export', record_type: lib.EXPORT_RECORD_TYPE, scope: WORKSPACE,
        origin: { kind: 'derived', source_ref: lib.deriveExportOriginSourceRef(PLAN_ID) },
        authority: { kind: 'delegated-run', authority_ref: `instance-canonical-export-run:${EXPORT_ID}` },
        payload: exportPayload,
      }],
    };

    const snapshotMap = { [`${REPOSITORY_REF}@${revision}`]: { record_type: lib.SOURCE_SNAPSHOT_RECORD_TYPE, repository_ref: REPOSITORY_REF, revision, digest: source.digest, working_tree_clean: true } };
    const rollbackMap = { [snapshotRef]: { record_type: lib.ROLLBACK_SNAPSHOT_RECORD_TYPE, source_snapshot_ref: snapshotRef, repository_ref: REPOSITORY_REF, revision, digest: source.digest } };
    const deterministicMap = { [deterministicRef]: { record_type: lib.DETERMINISTIC_PLAN_RECORD_TYPE, deterministic_plan_ref: deterministicRef, source_snapshot_ref: snapshotRef, plan_ref: PLAN_ID, plan_fingerprint: fingerprint, applicable: true } };
    const evidence = (kind, ref) => ({ [ref]: { record_type: lib.EVIDENCE_RECORD_TYPE, evidence_ref: ref, kind, plan_ref: PLAN_ID, plan_fingerprint: fingerprint, confirms: true } });
    const coverageMap = evidence('coverage', `evidence:coverage-${PLAN_ID}`);
    const applicabilityMap = evidence('applicability_preservation', `evidence:applicability-${PLAN_ID}`);

    const planProblems = lib.evaluateInstanceDataMigration(registry, {
      registrySchema: planSchema, envelopeSchema,
      resolveSourceSnapshot: lib.makeSourceSnapshotResolver(snapshotMap),
      resolveEvidence: lib.makeEvidenceResolver({ ...coverageMap, ...applicabilityMap }),
      resolveRollbackSnapshot: lib.makeRollbackSnapshotResolver(rollbackMap),
      resolveDeterministicPlan: lib.makeDeterministicPlanResolver(deterministicMap),
      resolveRestorationEvidence: lib.makeRestorationEvidenceResolver({}),
      resolveSupersededPlan: lib.makeSupersededPlanResolver({}),
    });
    const contentMap = {};
    for (const u of units.filter((x) => !x.retained)) {
      contentMap[lib.computeSourceContentKey({ repository_ref: REPOSITORY_REF, revision, unit_refs: [u.unit_ref] })] = {
        record_type: lib.SOURCE_CONTENT_RECORD_TYPE, repository_ref: REPOSITORY_REF, revision, unit_refs: [u.unit_ref], ...u.payload,
      };
    }
    const exportProblems = lib.evaluateInstanceCanonicalExport(canonicalExport, {
      registrySchema: exportSchema, envelopeSchema,
      resolveMigrationPlan: lib.makeMigrationPlanResolver({ [PLAN_ID]: {
        record_type: lib.RECORD_TYPE, plan_ref: PLAN_ID, plan_fingerprint: fingerprint, scope: WORKSPACE, source,
        record_units: payload.record_units.map((u) => ({ id: u.id, unit_ref: u.unit_ref })),
        mappings: payload.mappings.map((m) => ({ unit_id: m.unit_id, disposition: m.disposition, ...(m.target ? { target: m.target } : {}), ...(m.retained_reason ? { retained_reason: m.retained_reason } : {}) })),
      } }),
      resolveSourceContent: lib.makeSourceContentResolver(contentMap),
    });


    return { registry, canonicalExport, snapshotMap, rollbackMap, deterministicMap, coverageMap, applicabilityMap, payload, fingerprint, exportPayload, contentMap, planProblems, exportProblems, records };
  };
  const base = makeBundle(units, treeDigest);
  if (base.planProblems.length || base.exportProblems.length) {
    console.error([...base.planProblems, ...base.exportProblems].join('\n'));
    process.exit(1);
  }
  const { registry, canonicalExport, snapshotMap, rollbackMap, deterministicMap, coverageMap, applicabilityMap, payload, fingerprint, exportPayload, records } = base;

  const writeBundle = (dir, b) => {
    const write = (rel, value) => {
      fs.mkdirSync(path.dirname(path.join(dir, rel)), { recursive: true });
      fs.writeFileSync(path.join(dir, rel), `${JSON.stringify(value, null, 2)}\n`);
    };
    write('registry.json', b.registry);
    write('canonical-export.json', b.canonicalExport);
    write('source-snapshot.json', { note: 'Resolution map for resolveSourceSnapshot.', resolution: b.snapshotMap });
    write('rollback-snapshot.json', { note: 'Resolution map for resolveRollbackSnapshot.', resolution: b.rollbackMap });
    write('deterministic-reconstruction-plan.json', { note: 'Resolution map for resolveDeterministicPlan.', resolution: b.deterministicMap });
    write('evidence/coverage.json', { note: 'Resolution map for resolveEvidence (coverage).', resolution: b.coverageMap });
    write('evidence/applicability-preservation.json', { note: 'Resolution map for resolveEvidence (applicability).', resolution: b.applicabilityMap });
  };
  writeBundle(BUNDLE, base);
  // Internally consistent variants every accepted Node check passes, each
  // wrong in exactly one way only the pinned source itself can reveal.
  writeBundle(path.join(HERE, 'variants', 'wrong-digest'), makeBundle(units, sha('not the pinned tree')));
  const tampered = units.map((u) => (u.id === 'inventory-items-alpha'
    ? { ...u, payload: jsonEnvelope({ ...items[0], size: 99 }) }
    : u));
  writeBundle(path.join(HERE, 'variants', 'content-mismatch'), makeBundle(tampered, treeDigest));
  const builtIn = units.map((u) => (u.id === 'notes-readme-txt'
    ? { ...u, scope: { type: 'built-in-methodology', id: 'built-in-methodology' } }
    : u));
  writeBundle(path.join(HERE, 'variants', 'built-in-target'), makeBundle(builtIn, treeDigest));
  fs.writeFileSync(path.join(HERE, 'expected.json'), `${JSON.stringify({ revision, tree_digest: treeDigest, plan_fingerprint: fingerprint, idempotency_key: payload.idempotency_key, export_digest: exportPayload.digest, record_units: units.length, migrated: records.length, retained: 1, applicability_records: rules.length }, null, 2)}\n`);
  console.log(`fixture written: revision ${revision}, plan fingerprint ${fingerprint}`);
} finally {
  fs.rmSync(repo, { recursive: true, force: true });
}
