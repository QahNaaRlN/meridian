#!/usr/bin/env node
// Deterministic rule resolver (PHASE C of MERIDIAN-RULE-RESOLUTION-001).
//
// Read-only mechanic of the Kernel, a sibling of scripts/kernel-validate.mjs.
// It answers one question — "which agent norms, protocols and verification
// reach this unit of work, and why" — as a pure function of an explicit input,
// per standards/workspace/rule-resolution.md and the two PHASE B schemas
// registries/rule-resolution/{applicability,resolver-output}.schema.json.
//
// Two layers:
//   resolveRules(workItem, sources)  — the pure core. No file I/O, no globals
//                                      read except the two PHASE B schemas
//                                      (loaded once at module load, below).
//                                      The same input on the same source
//                                      revision returns a byte-identical,
//                                      identically ordered result.
//   the CLI (main)                   — reads the request and the applicability
//                                      register from explicit paths, reaches
//                                      the Instance only through
//                                      MERIDIAN_INSTANCE, and prints the
//                                      resolver output as JSON on stdout. It
//                                      is fail-closed: a missing mandatory
//                                      source ends the run with a clear
//                                      diagnostic on stderr and a non-zero
//                                      exit, never a partial guess.
//
// This file does NOT extend kernel-validate.mjs and kernel-validate.mjs stays
// the gate. The YAML reader, the JSON Schema engine and the marked-region
// reader are the shared scripts/lib/ modules, imported once by both scripts —
// no second copy. A container norm's region text is recomputed for its digest
// through the same instructionRegions() the validator uses; a region that is
// named but missing, duplicated or unclosed is fail-closed, never a silent
// read of the whole file.
//
// Not built here, on purpose (their permanent form is not decided by the
// program yet): a canonical registry or schema for protocol routes, for
// verification routes, or for architecture_profile. Those sources are passed
// in explicitly (dependency injection); the CLI takes them only from
// explicit flags, with no hidden default path. The injected protocol-route
// shape is deliberately temporary and carries its own exact scope selectors —
// `repository` for scope "repository", `product_domain` for scope
// "product-domain" — so a route is filtered against the work context before
// any conflict is computed and a repository route for one repository is never
// applied to another.

import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

import { yamlParse } from './lib/yaml.mjs';
import { validate, assertSupportedDeep } from './lib/json-schema.mjs';
import { instructionRegions } from './lib/regions.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const KERNEL_ROOT = process.env.MERIDIAN_KERNEL || path.resolve(__dirname, '..');

// The two PHASE B schemas are the contract this resolver is written against.
// Loaded once, here, from the Kernel that owns them: a data record is checked
// with the same engine kernel-validate uses, and the resolver's own output is
// checked against the output contract before it is returned.
const RR_DIR = path.join(KERNEL_ROOT, 'registries', 'rule-resolution');
const APPLICABILITY_SCHEMA = JSON.parse(fs.readFileSync(path.join(RR_DIR, 'applicability.schema.json'), 'utf8'));
const OUTPUT_SCHEMA = JSON.parse(fs.readFileSync(path.join(RR_DIR, 'resolver-output.schema.json'), 'utf8'));
assertSupportedDeep(APPLICABILITY_SCHEMA, 'applicability.schema.json');
assertSupportedDeep(OUTPUT_SCHEMA, 'resolver-output.schema.json');
const RECORD_SCHEMA = APPLICABILITY_SCHEMA.definitions.record;

// Fail-closed: any of these ends resolution rather than letting a guess through.
export class ResolverError extends Error {}
// A path mask whose syntax is outside the documented grammar. Never approximated.
export class UnsupportedGlob extends ResolverError {}

const WORK_KINDS = new Set(['change', 'assessment', 'operation', 'initiative']);
const CHANGE_CLASSES = new Set(['BUGFIX', 'FEATURE', 'BEHAVIOR_CHANGE', 'REFACTOR']);

export function sha256(text) {
  return crypto.createHash('sha256').update(String(text), 'utf8').digest('hex');
}

// ---------------------------------------------------------------------------
// Path-glob grammar — exact, and nothing outside it is interpreted.
//
//   **        zero or more WHOLE path segments; only as a complete segment
//             ("a/**/b", "**/x", "pkg/**"), never a fragment ("a**b"). Every
//             separate "**" is its own "zero or more segments"; several and
//             adjacent "**" are allowed ("**/**/x" matches "x", "a/x", "a/b/x";
//             "a/**/**/b" matches "a/b", "a/x/b", "a/x/y/b") and collapse to one.
//   *         any run of characters except "/"
//   ?         exactly one character except "/"
//   literal   every other character matches itself
//
// Refused with UnsupportedGlob, never approximated: brace expansion "{a,b}",
// character classes "[...]", extglob "!(...)" "+(...)" "@(...)", a leading "!"
// negation, and a "**" that is not a whole segment. Matching is anchored
// against the full candidate path (^...$).
// ---------------------------------------------------------------------------
function globSegToRegExp(seg) {
  let re = '';
  for (const ch of seg) {
    if (ch === '*') re += '[^/]*';
    else if (ch === '?') re += '[^/]';
    else re += ch.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  }
  return re;
}

export function globToRegExp(glob) {
  const g = String(glob);
  if (/[{}\[\]()!]/.test(g)) {
    throw new UnsupportedGlob(`path-glob "${g}" uses unsupported syntax (brace expansion, character class, extglob or "!" negation); the resolver does not interpret it approximately`);
  }
  const raw = g.split('/');
  for (const seg of raw) {
    if (seg !== '**' && seg.includes('**')) {
      throw new UnsupportedGlob(`path-glob "${g}" contains "**" that is not a whole path segment; write it as its own segment ("a/**/b") or use "*"`);
    }
  }
  // Adjacent "**" segments mean the same as one: "zero or more segments" twice
  // is still "zero or more segments". Collapse them first; then every remaining
  // "**" has a literal neighbour or a path boundary on each side, so the
  // separator it absorbs is unambiguous.
  const segs = [];
  for (const seg of raw) {
    if (seg === '**' && segs.length > 0 && segs[segs.length - 1] === '**') continue;
    segs.push(seg);
  }
  let body = '';
  for (let i = 0; i < segs.length; i++) {
    const seg = segs[i];
    const first = i === 0;
    const last = i === segs.length - 1;
    if (seg === '**') {
      if (first && last) body += '.*';           // the whole glob is "**": any path
      else if (first) body += '(?:[^/]+/)*';     // leading: zero or more segments, each with its trailing "/"
      else if (last) body += '(?:/[^/]+)*';      // trailing: zero or more segments, each with its leading "/"
      else body += '(?:/[^/]+)*/';               // middle: zero or more segments, then one "/" before the next literal
    } else {
      // A literal needs a "/" before it only after another literal; after a
      // "**" the separator is already inside the fragment above (leading and
      // middle end with "/"; trailing needs none - the next token is "$").
      if (!first && segs[i - 1] !== '**') body += '/';
      body += globSegToRegExp(seg);
    }
  }
  return new RegExp(`^${body}$`);
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------
const isArr = Array.isArray;
const asArr = (v) => (isArr(v) ? v : []);
const uniqSorted = (xs) => [...new Set(xs)].sort();

// Identifier of a norm: its Kernel path when it never went through intake,
// otherwise the full intake pointer — register, artifact, region, date,
// verdict — so the string picks out exactly one append-only record.
function normId(rec) {
  if (rec.source === 'kernel') return rec.norm.path;
  const p = rec.intake_record;
  const region = rec.norm.region ? `:${rec.norm.region}` : '';
  return `${p.register}#${rec.norm.path}${region}@${p.recorded_at}/${p.verdict}`;
}

function normTextKey(rec) {
  return `${rec.norm.repository}::${rec.norm.path}${rec.norm.region ? `::${rec.norm.region}` : ''}`;
}

// True when applicability record `rec` is the one a `supersedes` pointer `p`
// names: the §9 record identity — register + artifact + region + recorded_at +
// verdict (+ revision when the pointer carries it) — matched component by
// component. A kernel-source record has no intake_record identity and matches
// nothing.
function matchesIntakePointer(rec, p) {
  const ir = rec.intake_record;
  if (!ir) return false;
  if (ir.register !== p.register) return false;
  if (ir.recorded_at !== p.recorded_at) return false;
  if (ir.verdict !== p.verdict) return false;
  if (rec.norm.path !== p.path) return false;
  if ((rec.norm.region ?? null) !== (p.region ?? null)) return false;
  if (p.revision != null && ir.revision !== p.revision) return false;
  return true;
}

// Fail closed if the `supersedes` edges within one applicability recorded_at
// cohort contain a cycle: an append-only supersession chain that loops back on
// itself is contradictory data, not an ordering (rule-resolution.md §9).
// Detection is a three-colour DFS and its verdict does not depend on node order
// — a cycle is a cycle from whichever record the walk enters it.
function assertNoSupersedesCycle(nodes, edges) {
  const adj = new Map(nodes.map((n) => [n, []]));
  for (const e of edges) adj.get(e.from).push(e.to);
  const WHITE = 0, GRAY = 1, BLACK = 2;
  const colour = new Map(nodes.map((n) => [n, WHITE]));
  const stack = [];
  const visit = (u) => {
    colour.set(u, GRAY);
    stack.push(u);
    for (const v of adj.get(u)) {
      if (colour.get(v) === GRAY) {
        const loop = stack.slice(stack.indexOf(v)).concat(v).map((n) => normId(n)).join(' → ');
        throw new ResolverError(`the "supersedes" relationship among the applicability records sharing recorded_at ${nodes[0].recorded_at} forms a cycle (${loop}); an append-only supersession chain cannot be cyclic, so it is fail-closed (rule-resolution.md §9)`);
      }
      if (colour.get(v) === WHITE) visit(v);
    }
    colour.set(u, BLACK);
    stack.pop();
  };
  for (const n of nodes) if (colour.get(n) === WHITE) visit(n);
}

// Resolve one record's `supersedes` pointer against its OWN applicability
// recorded_at cohort, fail-closed on every ill-formed relationship. Returns the
// target record, or null when `r` carries no `supersedes`.
//   `cohort` — every record of the norm group sharing `r`'s applicability
//              recorded_at (a superseded record must share the carrier's date,
//              rule-resolution.md §9).
//   `group`  — the whole norm group, used only to tell "names a record on
//              another applicability date" apart from "names no record at all".
function resolveSupersedesEdge(r, cohort, group) {
  if (!r.supersedes) return null;
  const p = r.supersedes;
  if (p.path !== r.norm.path || (p.region ?? null) !== (r.norm.region ?? null)) {
    throw new ResolverError(`applicability record "${normId(r)}" carries a "supersedes" pointer to ${p.path}${p.region ? `#${p.region}` : ''}, a different norm identity than its own ${r.norm.path}${r.norm.region ? `#${r.norm.region}` : ''}; supersession is only between records of one norm identity (rule-resolution.md §9)`);
  }
  const hits = cohort.filter((t) => matchesIntakePointer(t, p));
  if (hits.length > 1) {
    throw new ResolverError(`the "supersedes" pointer of "${normId(r)}" is ambiguous: ${hits.length} records sharing applicability recorded_at ${r.recorded_at} match register+path+region+recorded_at+verdict; fail-closed (rule-resolution.md §9)`);
  }
  const target = hits[0];
  if (!target) {
    if (group.some((t) => t !== r && matchesIntakePointer(t, p))) {
      throw new ResolverError(`the "supersedes" pointer of "${normId(r)}" names a record that does not share this record's applicability recorded_at ${r.recorded_at}; a "supersedes" relationship orders only records of one applicability date, so it is fail-closed (rule-resolution.md §9)`);
    }
    throw new ResolverError(`the "supersedes" pointer of "${normId(r)}" resolves to no applicability record (register ${p.register}, ${p.path}${p.region ? `#${p.region}` : ''} @ ${p.recorded_at} → ${p.verdict}); fail-closed (rule-resolution.md §9)`);
  }
  if (target === r) {
    throw new ResolverError(`applicability record "${normId(r)}" declares that it supersedes itself; a "supersedes" pointer must name a different record (rule-resolution.md §9)`);
  }
  return target;
}

// Validate the `supersedes` relationship graph of ONE applicability recorded_at
// cohort of a norm group, fail-closed (rule-resolution.md §9). Every declared
// relationship is data and is validated whether or not this cohort is the
// authoritative one. Returns { declaresOrdering, head }: `head` is the single
// un-superseded record when the cohort declares ordering, else null.
function validateCohortSupersedes(cohort, group) {
  const edges = [];
  const supersededBy = new Map(); // superseded record -> a record that supersedes it
  for (const r of cohort) {
    const target = resolveSupersedesEdge(r, cohort, group);
    if (!target) continue;
    edges.push({ from: r, to: target });
    supersededBy.set(target, r);
  }
  if (edges.length === 0) return { declaresOrdering: false, head: null };

  assertNoSupersedesCycle(cohort, edges);
  const heads = cohort.filter((rec) => !supersededBy.has(rec));
  if (heads.length !== 1) {
    throw new ResolverError(`norm "${normId(cohort[0])}" has ${cohort.length} applicability records sharing recorded_at ${cohort[0].recorded_at} and their "supersedes" relationship leaves ${heads.length} un-superseded head(s) (${heads.map((h) => normId(h)).sort().join(', ')}); a cohort that declares ordering must resolve to exactly one head, so it is fail-closed (rule-resolution.md §4, §9)`);
  }
  return { declaresOrdering: true, head: heads[0] };
}

// The state of one norm identity is decided by its authoritative append-only
// applicability record. Two steps, in this order:
//   1. VALIDATE every declared `supersedes` relationship. The norm group is
//      split into applicability recorded_at cohorts, and the `supersedes` graph
//      of EVERY cohort — current or historical — is validated fail-closed
//      (rule-resolution.md §9): a pointer with no target, a target on another
//      applicability date, an ambiguous target, a cross-identity target, a
//      self-pointer, a cycle, or a cohort that declares ordering yet leaves more
//      than one head all end resolution here. A `supersedes` on a record that is
//      alone on its date has no same-date target and therefore fails closed — it
//      is never treated as harmless stray data.
//   2. SELECT by date. The greatest applicability recorded_at picks the current
//      cohort. `recorded_at` keeps its meaning — establishment / last-sync date
//      — and is never a synthetic sequence. A unique record in that cohort is
//      authoritative. A tied cohort is authoritative only through a valid
//      one-head `supersedes` graph (validated in step 1); a tied cohort with no
//      such relationship is fail-closed. The intake verdict is never itself a
//      precedence key.
// The result does not depend on the input order of `recs`: cohort keys are
// sorted, a unique latest record is the sole member of its cohort, and a valid
// tied cohort yields exactly one head.
function pickAuthoritative(recs) {
  const cohorts = new Map();
  for (const r of recs) {
    const k = String(r.recorded_at ?? '');
    if (!cohorts.has(k)) cohorts.set(k, []);
    cohorts.get(k).push(r);
  }
  const dates = [...cohorts.keys()].sort();

  // step 1 — validate every cohort's declared relationships
  const validated = new Map();
  for (const k of dates) validated.set(k, validateCohortSupersedes(cohorts.get(k), recs));

  // step 2 — select by date
  const latestKey = dates[dates.length - 1];
  const latest = cohorts.get(latestKey);
  if (latest.length === 1) return latest[0];

  const v = validated.get(latestKey);
  if (!v.declaresOrdering) {
    throw new ResolverError(`norm "${normId(latest[0])}" has ${latest.length} applicability records sharing the latest recorded_at ${latestKey}; append-only precedence cannot be decided deterministically within the available identity/date, so it is fail-closed — no "supersedes" relationship orders them and JSON order is not a tie-breaker (rule-resolution.md §4, §9)`);
  }
  return v.head;
}

// The one intake record an applicability entry is synced to. Zero matches or
// more than one is fail-closed: an unresolvable or ambiguous pointer is not a
// norm the resolver may return as applicable (rule-resolution.md §9).
function resolveIntakePointer(rec, sources) {
  const ptr = rec.intake_record;
  const registers = asArr(sources.intake_registers);
  const reg = registers.find((r) => r && r.register === ptr.register);
  if (!reg) {
    throw new ResolverError(`intake pointer of "${normId(rec)}" names register "${ptr.register}", which is not among the supplied intake registers`);
  }
  const region = rec.norm.region ?? null;
  const matches = asArr(reg.records).filter((ir) =>
    ir
    && ir.artifact === rec.norm.path
    && (region === null ? (ir.region === undefined || ir.region === null) : ir.region === region)
    && ir.recorded_at === ptr.recorded_at
    && ir.verdict === ptr.verdict);
  if (matches.length === 0) {
    throw new ResolverError(`intake pointer of "${normId(rec)}" resolves to no record in ${ptr.register} (artifact ${rec.norm.path}${region ? `#${region}` : ''} @ ${ptr.recorded_at} → ${ptr.verdict})`);
  }
  if (matches.length > 1) {
    throw new ResolverError(`intake pointer of "${normId(rec)}" is ambiguous: ${matches.length} records in ${ptr.register} match on artifact, region, date and verdict`);
  }
  const ir = matches[0];
  if (!ir.delivery) throw new ResolverError(`intake record for "${normId(rec)}" carries no delivery`);
  return ir;
}

// The three applicability axes an MVP record can carry, checked SEPARATELY and
// CONJUNCTIVELY (rule-resolution.md §5): a record applies only if it applies on
// every axis it declares. Technology profile is never inferred from anything;
// architecture profile is not an axis of the MVP schema at all.
//
// Returns: 'no-match' | 'applies' | 'activation-undetermined'
function classifyRecord(rec, ctx) {
  // --- scope axis ---
  let scopeOk;
  switch (rec.scope) {
    case 'universal': scopeOk = true; break;
    case 'repository': scopeOk = rec.repository === ctx.repository_id; break;
    case 'profile': scopeOk = ctx.technology_profile != null && rec.technology_profile === ctx.technology_profile; break;
    case 'product-domain': scopeOk = ctx.product_domains.includes(rec.product_domain); break;
    default: scopeOk = false;
  }
  if (!scopeOk) return 'no-match';

  // --- activation axis ---
  switch (rec.activation) {
    case 'always':
      return 'applies';
    case 'path-glob': {
      const hit = asArr(rec.globs).some((glob) => {
        const re = globToRegExp(glob); // throws UnsupportedGlob — fail-closed, propagates
        return ctx.candidate_paths.some((p) => re.test(p));
      });
      return hit ? 'applies' : 'no-match';
    }
    case 'task-class': {
      const tc = rec.task_class || {};
      if (!asArr(tc.work_kind).includes(ctx.work_kind)) return 'no-match';
      if (isArr(tc.change_class)) {
        if (ctx.work_kind !== 'change') return 'no-match';
        if (!tc.change_class.includes(ctx.change_class)) return 'no-match';
      }
      return 'applies';
    }
    case 'explicit':
      // Activated only when the work item explicitly names the norm. The §3.1
      // input has no such field, so in the MVP an explicit norm is simply not
      // activated by this call — not applicable, and not unresolved either.
      return 'no-match';
    case 'undetermined':
      // "undetermined" is a finding, not a blank (instruction-intake.md): the
      // resolver cannot decide whether this norm activates, so it is surfaced
      // as unresolved rather than dropped or asserted.
      return 'activation-undetermined';
    default:
      return 'no-match';
  }
}

function requireSource(sources, key) {
  if (!isArr(sources[key])) {
    throw new ResolverError(`missing required source: "${key}" must be an array (dependency injection — pass it explicitly)`);
  }
}

// ---------------------------------------------------------------------------
// resolveRules — the pure core
// ---------------------------------------------------------------------------
// workItem: exactly the shape of §3.1 of the normative model
//   { repository_id, work_kind, change_class?, candidate_paths, changed_paths,
//     declared_profiles?: { technology_profile?, architecture_profile? } }
//
// sources: explicitly supplied environmental data, NOT extra work-item fields
//   repository_inventory   [ { id, profile?, semantic_areas? } ]              (required)
//   applicability_records  [ <record per applicability.schema.json> ]         (required)
//   intake_registers       [ { register, repository?, records: [ <intake record> ] } ]
//   protocol_routes        [ { routed_from, protocol, source, scope, digest?|revision?, mandatory? } ]
//   verification_routes    [ { applies_to: <norm id> | "*", path } ]
//   norm_texts             { "<repo>::<path>[::<region>]": "<current text>" }
//   prior_state            { previous_resolution?: { candidate_paths },
//                            decomposition?: { child_work_items },
//                            initiative_protocol?: <string> | null,
//                            pre_decomposition_norms?: [<string>],
//                            source_repository_ids?: [<string>],
//                            refactor_findings?: [ { type } ] }
//
// A key `sources` does not know (a reviewer assignment, an executor identity)
// is ignored: the technical resolver's output does not depend on it
// (rule-resolution.md §8).
const WORK_ITEM_KEYS = new Set([
  'repository_id', 'work_kind', 'change_class', 'candidate_paths', 'changed_paths', 'declared_profiles',
]);

// §3.1 of the normative model, checked strictly: a missing or malformed field
// is fail-closed, never coerced. In particular candidate_paths and changed_paths
// are mandatory string arrays — a missing or non-array value is NOT read as an
// empty list, because the "a widened file list forces re-resolution" invariant
// (acceptance case 8) cannot be checked if the two are allowed to default away.
function requireStringArray(v, name) {
  if (!isArr(v)) {
    throw new ResolverError(`work item: ${name} is required and must be an array of strings (a missing or non-array value is not treated as an empty list — §3.1)`);
  }
  v.forEach((x, i) => {
    if (typeof x !== 'string') {
      throw new ResolverError(`work item: ${name}[${i}] must be a string, got ${x === null ? 'null' : Array.isArray(x) ? 'array' : typeof x}`);
    }
  });
  return v.slice();
}

export function resolveRules(workItem, sources = {}) {
  const src = sources || {};

  // --- work item shape (§3.1), fail-closed ---
  if (workItem === null || typeof workItem !== 'object' || Array.isArray(workItem)) {
    throw new ResolverError('work item: the request must be an object in the shape of §3.1');
  }
  const wi = workItem;
  for (const k of Object.keys(wi)) {
    if (!WORK_ITEM_KEYS.has(k)) {
      throw new ResolverError(`work item: unknown field "${k}" (allowed: repository_id, work_kind, change_class, candidate_paths, changed_paths, declared_profiles)`);
    }
  }
  if (typeof wi.repository_id !== 'string' || wi.repository_id === '') {
    throw new ResolverError('work item: repository_id must be a non-empty string');
  }
  if (!WORK_KINDS.has(wi.work_kind)) {
    throw new ResolverError(`work item: work_kind "${wi.work_kind}" is not one of change | assessment | operation | initiative`);
  }
  const isChange = wi.work_kind === 'change';
  if (isChange) {
    if (!CHANGE_CLASSES.has(wi.change_class)) {
      throw new ResolverError(`work item: work_kind is "change", so change_class is required and must be BUGFIX | FEATURE | BEHAVIOR_CHANGE | REFACTOR (got "${wi.change_class}")`);
    }
  } else if (wi.change_class !== undefined && wi.change_class !== null) {
    throw new ResolverError(`work item: change_class is meaningful only inside work_kind "change" (rule-resolution.md §3); it must not be set for work_kind "${wi.work_kind}"`);
  }
  if (wi.declared_profiles !== undefined && wi.declared_profiles !== null) {
    const dp = wi.declared_profiles;
    if (typeof dp !== 'object' || Array.isArray(dp)) {
      throw new ResolverError('work item: declared_profiles must be an object when present');
    }
    for (const k of Object.keys(dp)) {
      if (k !== 'technology_profile' && k !== 'architecture_profile') {
        throw new ResolverError(`work item: declared_profiles has unknown key "${k}" (allowed: technology_profile, architecture_profile)`);
      }
      if (dp[k] !== undefined && dp[k] !== null && typeof dp[k] !== 'string') {
        throw new ResolverError(`work item: declared_profiles.${k} must be a string when present`);
      }
    }
  }
  const candidatePaths = requireStringArray(wi.candidate_paths, 'candidate_paths');
  const changedPaths = requireStringArray(wi.changed_paths, 'changed_paths');

  // --- required sources ---
  requireSource(src, 'repository_inventory');
  requireSource(src, 'applicability_records');

  // --- repository axis: EXACT match against the inventory, no fallback ---
  const repo = src.repository_inventory.find((r) => r && r.id === wi.repository_id);
  if (!repo) {
    throw new ResolverError(`unknown repository_id "${wi.repository_id}": no entry with that exact id in repository_inventory (no fuzzy, path or basename fallback)`);
  }
  const ctx = {
    repository_id: wi.repository_id,
    technology_profile: wi.declared_profiles?.technology_profile ?? repo.profile ?? null,
    product_domains: asArr(repo.semantic_areas).map(String),
    work_kind: wi.work_kind,
    change_class: isChange ? wi.change_class : null,
    candidate_paths: candidatePaths,
  };

  // --- output skeleton: every field always present (rule-resolution schema §15) ---
  const out = {
    applicable_norms: [],
    applicable_protocols: [],
    required_verification: [],
    conflicts: [],
    unresolved_applicability: [],
    unresolved_items: [],
    requires_reresolution: false,
    decomposition_required: false,
    applicable_initiative_protocol: null,
    pre_decomposition_norms: [],
  };

  // --- requires_reresolution: changed_paths expanding the set a prior
  //     resolution used (its candidate_paths if given, else this call's) ---
  const priorCandidates = src.prior_state?.previous_resolution?.candidate_paths;
  const refPaths = isArr(priorCandidates) ? priorCandidates.map(String) : candidatePaths;
  out.requires_reresolution = changedPaths.some((p) => !refPaths.includes(p));

  // --- work_kind: initiative — decomposition view only; child work items are
  //     resolved by their own calls once decomposition is supplied (§6) ---
  if (wi.work_kind === 'initiative') {
    const decomp = src.prior_state?.decomposition;
    const decomposed = decomp && isArr(decomp.child_work_items) && decomp.child_work_items.length > 0;
    out.decomposition_required = !decomposed;
    const initProto = src.prior_state?.initiative_protocol;
    out.applicable_initiative_protocol = (typeof initProto === 'string' && initProto) ? initProto : null;
    out.pre_decomposition_norms = uniqSorted(asArr(src.prior_state?.pre_decomposition_norms).map(String));
    if (!decomposed) {
      const srcRepos = src.prior_state?.source_repository_ids;
      if (!isArr(srcRepos) || srcRepos.length < 2) {
        out.unresolved_items.push({
          item: 'complete list of source repositories',
          reason: 'the request carries a single repository_id and no complete source-repository list was supplied; a consolidation initiative cannot be decomposed into child work items without it (rule-resolution.md §6)',
        });
      }
    }
    return finalize(out);
  }

  // --- protocol routing from work_kind / change_class ---
  // REFACTOR is routed like any other class: the functional-parity evidence
  // contract (PHASE D) and the REFACTOR execution protocol (PHASE E) now exist,
  // so REFACTOR resolves to its own protocol with provenance, not to another
  // class's protocol by residual match (rule-resolution.md §3.1, §7).
  const routeKey = isChange ? wi.change_class : wi.work_kind;
  routeProtocols(routeKey, src, out, ctx);

  // A behavior change found inside a REFACTOR work item does not silently become
  // part of it: the original REFACTOR keeps its class and its protocol route,
  // and a separate BUGFIX child work item is created or the owner decides
  // (rule-resolution.md §3.2). This is surfaced as an unresolved item on the
  // REFACTOR work item, alongside — not instead of — its resolved protocol.
  if (routeKey === 'REFACTOR') {
    const findings = asArr(src.prior_state?.refactor_findings);
    if (findings.length > 0) {
      out.unresolved_applicability.push({
        subject: 'refactor-in-progress-finding',
        reason: 'a behavior change was found inside this REFACTOR work item; it is not fixed silently — a separate BUGFIX child work item must be created or the owner must decide, and the original REFACTOR work item is not reclassified (rule-resolution.md §3.2)',
      });
    }
  }

  // --- norm applicability ---
  resolveNorms(src, ctx, out);

  // --- required verification, keyed on the norms that came out applicable ---
  const applicableNormIds = new Set(out.applicable_norms.map((n) => n.norm));
  const verif = [];
  for (const vr of asArr(src.verification_routes)) {
    if (!vr || typeof vr.path !== 'string' || vr.path === '') continue;
    if (vr.applies_to === '*' || applicableNormIds.has(vr.applies_to)) verif.push(vr.path);
  }
  out.required_verification = uniqSorted(verif);

  return finalize(out);
}

// A protocol route reaches this work item only if its own scope selector picks
// this work context. This runs BEFORE any conflict is computed, so a repository
// route for one repository is never even a candidate for another, and a
// local/universal disagreement that survives scope filtering is a real conflict
// rather than a silent override (rule-resolution.md §7). The route shape is the
// injected temporary one; its exact selectors are `repository` (scope
// "repository") and `product_domain` (scope "product-domain"). A malformed
// selector is fail-closed, not applied broadly.
function routeInScope(r, ctx) {
  switch (r.scope) {
    case 'universal':
      return true;
    case 'repository':
      if (typeof r.repository !== 'string' || r.repository === '') {
        throw new ResolverError(`protocol route for "${r.protocol}" is scope "repository" but carries no exact "repository" selector; an unscoped repository route is fail-closed, not applied to every repository (rule-resolution.md §7)`);
      }
      return r.repository === ctx.repository_id;
    case 'product-domain':
      if (typeof r.product_domain !== 'string' || r.product_domain === '') {
        throw new ResolverError(`protocol route for "${r.protocol}" is scope "product-domain" but carries no exact "product_domain" selector; fail-closed (rule-resolution.md §7)`);
      }
      return ctx.product_domains.includes(r.product_domain);
    default:
      throw new ResolverError(`protocol route for "${r.protocol}" has unsupported scope "${r.scope}" (universal | product-domain | repository)`);
  }
}

function routeProtocols(routeKey, src, out, ctx) {
  let routes = asArr(src.protocol_routes)
    .filter((r) => r && r.routed_from === routeKey)
    .filter((r) => routeInScope(r, ctx));
  const mandatory = routes.filter((r) => r.mandatory === true);
  const distinctMandatory = uniqSorted(mandatory.map((r) => String(r.protocol)));
  if (distinctMandatory.length > 1) {
    // A local route silently overriding an incompatible universal one is a
    // conflict, not a quiet pick (rule-resolution.md §7).
    out.conflicts.push({
      norms: distinctMandatory,
      reason: `${distinctMandatory.length} incompatible mandatory routes for "${routeKey}"; a local protocol route does not silently override a universal one (rule-resolution.md §7)`,
    });
    routes = routes.filter((r) => r.mandatory !== true);
  }
  const sourceRank = { kernel: 0, instance: 1, repository: 2 };
  const byProtocol = new Map();
  for (const r of routes) {
    const key = String(r.protocol);
    const prev = byProtocol.get(key);
    if (!prev
      || (sourceRank[r.source] ?? 9) < (sourceRank[prev.source] ?? 9)
      || ((sourceRank[r.source] ?? 9) === (sourceRank[prev.source] ?? 9) && JSON.stringify(r) < JSON.stringify(prev))) {
      byProtocol.set(key, r);
    }
  }
  for (const key of [...byProtocol.keys()].sort()) {
    const r = byProtocol.get(key);
    const entry = { protocol: key, routed_from: routeKey, source: r.source, scope: r.scope };
    if (typeof r.digest === 'string' && r.digest) entry.digest = r.digest;
    else if (typeof r.revision === 'string' && r.revision) entry.revision = r.revision;
    else throw new ResolverError(`protocol route for "${key}" carries neither digest nor revision; a route's provenance is required (rule-resolution.md §7)`);
    out.applicable_protocols.push(entry);
  }
}

// Provenance of one matching applicability record, fail-closed (rule-resolution
// .md §9): the norm text (or region text) must be supplied, its SHA-256 must
// equal the digest the record carries, and for a non-kernel norm the exact
// intake pointer must resolve to exactly one append-only intake record. This
// runs for EVERY record that matches the work item on scope/path/task-class —
// resolved, unresolved and activation-undetermined alike — not only for the one
// that ends up in applicable_norms. A record that does not match on
// scope/path/task-class is out of play and its text is never read.
function verifyProvenance(rec, src) {
  const text = src.norm_texts?.[normTextKey(rec)];
  if (typeof text !== 'string') {
    throw new ResolverError(`no current text supplied for norm "${normId(rec)}"; its digest cannot be recomputed, so it is fail-closed rather than treated as applicable (rule-resolution.md §9)`);
  }
  if (sha256(text) !== rec.digest) {
    throw new ResolverError(`stale digest for norm "${normId(rec)}": the supplied text hashes to ${sha256(text).slice(0, 12)}… but the record records ${String(rec.digest).slice(0, 12)}…; a norm that has moved since its record was written is not treated as applicable`);
  }
  if (rec.source === 'kernel') return { delivery: 'kernel-doc' };
  return { delivery: resolveIntakePointer(rec, src).delivery };
}

function resolveNorms(src, ctx, out) {
  // Shape-check every applicability record with the same engine kernel-validate
  // uses. A malformed record is fail-closed, never skipped.
  src.applicability_records.forEach((rec, i) => {
    const errs = [];
    validate(rec, RECORD_SCHEMA, APPLICABILITY_SCHEMA, '', errs);
    if (errs.length) throw new ResolverError(`malformed applicability record #${i}: ${errs[0]}`);
  });

  // One norm identity can carry several append-only records (a superseding
  // resolved record next to an older unresolved one, and vice versa). Group by
  // the norm's identity, not by the record.
  const groups = new Map();
  for (const rec of src.applicability_records) {
    const k = `${rec.norm.repository}::${rec.norm.path}::${rec.norm.region ?? ''}`;
    if (!groups.has(k)) groups.set(k, []);
    groups.get(k).push(rec);
  }

  for (const recs of groups.values()) {
    const tagged = recs.map((rec) => ({ rec, cls: classifyRecord(rec, ctx) }));

    // The authoritative record — greatest recorded_at across the whole
    // identity, ties fail-closed (rule-resolution.md §4, §9). Its own status and
    // classification decide the norm: a newer unresolved suppresses an older
    // resolved, and a newer resolved suppresses an older unresolved.
    const authoritative = pickAuthoritative(recs);
    const at = tagged.find((t) => t.rec === authoritative);

    // If the latest record says the norm does not reach this work item on
    // scope/path/task-class, the norm is simply out of scope here — and no
    // record's text needs to be read (rule-resolution.md §4).
    if (at.cls === 'no-match') continue;

    // Every record of this identity that matches the work item must have
    // resolvable provenance, not only the authoritative one.
    const deliveryByRec = new Map();
    for (const t of tagged) {
      if (t.cls === 'applies' || t.cls === 'activation-undetermined') {
        deliveryByRec.set(t.rec, verifyProvenance(t.rec, src));
      }
    }

    if (at.cls === 'applies' && authoritative.status === 'resolved') {
      out.applicable_norms.push({
        norm: normId(authoritative),
        activation_reason: authoritative.activation,
        source: authoritative.source,
        digest: authoritative.digest,
        delivery: deliveryByRec.get(authoritative).delivery,
      });
    } else if (at.cls === 'activation-undetermined') {
      out.unresolved_applicability.push({
        subject: normId(authoritative),
        reason: 'observed activation is "undetermined": whether this norm activates for the work item is not deterministically decidable (rule-resolution.md §4)',
      });
    } else if (at.cls === 'applies' && authoritative.status === 'unresolved') {
      out.unresolved_applicability.push({
        subject: normId(authoritative),
        reason: authoritative.resume_condition,
      });
    }
  }
}

// Sort every array by a stable key, then check the whole object against the
// output contract before returning it. A resolver that emitted an output the
// contract rejects would be a defect in the resolver, so it is loud.
function finalize(out) {
  out.applicable_norms.sort((a, b) =>
    a.norm.localeCompare(b.norm) || a.source.localeCompare(b.source) || a.digest.localeCompare(b.digest));
  out.applicable_protocols.sort((a, b) =>
    a.protocol.localeCompare(b.protocol) || String(a.routed_from).localeCompare(String(b.routed_from)));
  out.required_verification = uniqSorted(out.required_verification);
  out.conflicts.sort((a, b) => JSON.stringify(a.norms).localeCompare(JSON.stringify(b.norms)) || a.reason.localeCompare(b.reason));
  out.unresolved_applicability.sort((a, b) => a.subject.localeCompare(b.subject) || a.reason.localeCompare(b.reason));
  out.unresolved_items.sort((a, b) => a.item.localeCompare(b.item) || a.reason.localeCompare(b.reason));
  out.pre_decomposition_norms = uniqSorted(out.pre_decomposition_norms);

  const errs = [];
  validate(out, OUTPUT_SCHEMA, OUTPUT_SCHEMA, '', errs);
  if (errs.length) {
    throw new ResolverError(`internal: resolver produced output that violates resolver-output.schema.json (${errs[0]}); this is a resolver defect, not a data problem`);
  }
  return out;
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------
function parseArgs(argv) {
  const opts = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--request') opts.request = argv[++i];
    else if (a === '--applicability') opts.applicability = argv[++i];
    else if (a === '--protocol-routes') opts.protocolRoutes = argv[++i];
    else if (a === '--verification-routes') opts.verificationRoutes = argv[++i];
    else if (a === '--prior-state') opts.priorState = argv[++i];
    else if (a === '--help' || a === '-h') opts.help = true;
    else throw new ResolverError(`unknown argument "${a}"`);
  }
  return opts;
}

const USAGE = `Usage:
  MERIDIAN_INSTANCE=<instance root> \\
  node scripts/rule-resolver.mjs --request <request.json> --applicability <register.yaml|json> \\
      [--protocol-routes <routes.json|yaml>] [--verification-routes <routes.json|yaml>] \\
      [--prior-state <prior.json|yaml>]

  --request              JSON: { "work_item": { <§3.1 shape> } }
  --applicability         the PHASE B applicability register file — the whole
                         { "$schema", "schema_version", "records" } envelope. It
                         is validated against
                         registries/rule-resolution/applicability.schema.json
                         before use; a malformed or record-less document is
                         fail-closed with exit 2. A bare top-level array is not
                         accepted.
  --protocol-routes       not-yet-standardised route records; only used if passed, no hidden default
  --verification-routes   not-yet-standardised verification route records; same
  --prior-state           previous resolution / decomposition data for requires_reresolution and initiative

  The Instance is reached only through MERIDIAN_INSTANCE. inventory/repositories.yaml
  and the instruction-intake/*.yaml registers named by the applicability records are
  read from there. Output JSON is printed to stdout. A missing mandatory source is
  fail-closed: a clear message on stderr and a non-zero exit.`;

function readStructured(file) {
  const raw = fs.readFileSync(file, 'utf8');
  if (/\.ya?ml$/i.test(file)) return yamlParse(raw);
  return JSON.parse(raw);
}

// The VERBATIM source text of one declared instruction-section region, for
// digest recomputation of a container norm. This is NOT a second region parser:
// it runs the same scripts/lib/regions.mjs instructionRegions() the validator
// uses — markers are still located in the fenced-blanked buffer, so a marker
// quoted in a code fence is not a declaration, and nested, unnamed, duplicate,
// cross-closed and unclosed regions are still refused. What it returns is
// region.sourceText: the slice of the ORIGINAL raw file between the markers,
// every character intact including fenced code, never the space-blanked parser
// view. A region that is named but missing, duplicated or structurally broken
// is fail-closed here — never a silent fallback to the whole file.
export function regionSourceText(fullText, id) {
  const parsed = instructionRegions(fullText);
  if (parsed.errors.length) {
    throw new ResolverError(`container region "${id}" cannot be read: ${parsed.errors[0]}`);
  }
  const matches = parsed.regions.filter((r) => r.id === id);
  if (matches.length === 0) {
    throw new ResolverError(`container names region "${id}", which the file does not declare; fail-closed rather than reading the whole file`);
  }
  if (matches.length > 1) {
    throw new ResolverError(`container declares region "${id}" ${matches.length} times; an ambiguous region is fail-closed`);
  }
  return matches[0].sourceText;
}

function main() {
  let opts;
  try {
    opts = parseArgs(process.argv.slice(2));
  } catch (e) {
    process.stderr.write(`rule-resolver: ${e.message}\n\n${USAGE}\n`);
    process.exit(2);
  }
  if (opts.help) { process.stdout.write(`${USAGE}\n`); process.exit(0); }

  const INSTANCE_ROOT = process.env.MERIDIAN_INSTANCE || null;
  try {
    if (!INSTANCE_ROOT) {
      throw new ResolverError('MERIDIAN_INSTANCE is not set; the resolver reaches the Instance only through it and will not guess a path');
    }
    if (!opts.request) throw new ResolverError('--request <file> is required');
    if (!opts.applicability) throw new ResolverError('--applicability <file> is required');

    const request = readStructured(opts.request);
    const workItem = request && request.work_item ? request.work_item : request;
    if (!workItem || typeof workItem !== 'object') {
      throw new ResolverError('--request must contain a "work_item" object (or be the work item itself)');
    }

    // --applicability is the full PHASE B register envelope. The WHOLE document
    // is checked against registries/rule-resolution/applicability.schema.json
    // with the shared engine before records are touched: a bare {}, a missing
    // "records", a wrong schema_version, an additional property or a non-array
    // "records" is fail-closed here, not coerced. A top-level raw array is not
    // an accepted CLI shape — the pure core still takes an injected
    // applicability_records array, but the file the CLI reads is the envelope.
    const applDoc = readStructured(opts.applicability);
    {
      const errs = [];
      validate(applDoc, APPLICABILITY_SCHEMA, APPLICABILITY_SCHEMA, '', errs);
      if (errs.length) {
        throw new ResolverError(`--applicability document does not satisfy registries/rule-resolution/applicability.schema.json: ${errs[0]}`);
      }
    }
    const applicabilityRecords = applDoc.records;

    // repository inventory
    const invPath = path.join(INSTANCE_ROOT, 'inventory', 'repositories.yaml');
    let inventory;
    try {
      inventory = asArr(yamlParse(fs.readFileSync(invPath, 'utf8')).repositories);
    } catch (e) {
      throw new ResolverError(`could not read the repository inventory at ${invPath}: ${e.message}`);
    }
    const repositoryInventory = inventory.map((r) => ({
      id: r.id,
      profile: r.profile ?? null,
      semantic_areas: asArr(r.semantic_areas),
      path: r.path ?? null,
    }));
    const repoPathById = new Map(inventory.filter((r) => r && r.id).map((r) => [String(r.id), r.path]));

    // instruction-intake registers named by the applicability records
    const registerNames = [...new Set(
      applicabilityRecords
        .filter((rec) => rec && rec.source !== 'kernel' && rec.intake_record && rec.intake_record.register)
        .map((rec) => rec.intake_record.register),
    )];
    const intakeRegisters = [];
    for (const register of registerNames) {
      const p = path.join(INSTANCE_ROOT, register);
      let doc;
      try { doc = yamlParse(fs.readFileSync(p, 'utf8')); }
      catch (e) { throw new ResolverError(`applicability records point at intake register "${register}", which could not be read at ${p}: ${e.message}`); }
      intakeRegisters.push({ register, repository: doc.repository ?? null, records: asArr(doc.records) });
    }

    // current norm texts, for digest recomputation
    const normTexts = {};
    for (const rec of applicabilityRecords) {
      if (!rec || !rec.norm) continue;
      const key = `${rec.norm.repository}::${rec.norm.path}${rec.norm.region ? `::${rec.norm.region}` : ''}`;
      if (key in normTexts) continue;
      let base;
      if (rec.norm.repository === 'kernel') base = KERNEL_ROOT;
      else base = repoPathById.get(String(rec.norm.repository)) ?? null;
      if (!base) continue; // resolver will fail-closed with a precise message if this norm is needed
      let full;
      try { full = fs.readFileSync(path.join(base, rec.norm.path), 'utf8'); }
      catch { continue; }
      if (rec.norm.region) {
        // The verbatim source slice of the region, fenced code intact — that is
        // what the digest is over. Fail-closed on a broken region declaration,
        // never a silent fallback to the whole file. If this norm turns out to
        // be irrelevant to the work item, resolveRules never consults the key
        // and its text was not needed; if it is relevant, resolveRules is
        // fail-closed on the missing text.
        try { normTexts[key] = regionSourceText(full, rec.norm.region); }
        catch { continue; }
      } else {
        normTexts[key] = full;
      }
    }

    const sources = {
      repository_inventory: repositoryInventory,
      applicability_records: applicabilityRecords,
      intake_registers: intakeRegisters,
      protocol_routes: opts.protocolRoutes ? asArr(readStructured(opts.protocolRoutes).routes ?? readStructured(opts.protocolRoutes)) : [],
      verification_routes: opts.verificationRoutes ? asArr(readStructured(opts.verificationRoutes).routes ?? readStructured(opts.verificationRoutes)) : [],
      norm_texts: normTexts,
      prior_state: opts.priorState ? readStructured(opts.priorState) : {},
    };

    const result = resolveRules(workItem, sources);
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
    process.exit(0);
  } catch (e) {
    if (e instanceof ResolverError) {
      process.stderr.write(`rule-resolver: ${e.message}\n`);
      process.exit(2);
    }
    process.stderr.write(`rule-resolver: unexpected error: ${e.stack || e.message}\n`);
    process.exit(1);
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url))) {
  main();
}
