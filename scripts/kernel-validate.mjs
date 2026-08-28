#!/usr/bin/env node
// Read-only validator for this agent workspace. Modifies nothing.
//
// Design rule this file follows itself: a check that cannot actually be
// performed must report UNVERIFIED (warn/fail), never OK. A green run must
// mean "verified", not "not looked at". See
// docs/agent-standards/workspace/kernel-boundary.md.
//
// Checks
//   1. kernel-purity     — every Git-tracked file of this repository is free
//                          of the current product's literals, its forbidden
//                          patterns and of personal home-directory paths.
//                          Binary artifacts are covered by sha-provenance
//                          instead of the text scan, and are counted as such.
//   2. duplicate-fm      — no scanned Markdown file (Kernel *or* .agent) ends
//                          with an orphaned second Front-Matter block.
//   3. front-matter      — .agent artifacts carry leading Front Matter (WARN).
//   4. path-placement    — active/ vs done/archive/ vs status (WARN).
//   5. links             — relative Markdown links resolve.
//   5b. document-identity — file names follow the level/case rule and every
//                          Kernel document declares a document_type from the
//                          closed pool. Templates, vendored artifacts and
//                          fixture data are exempt by pattern, not by list.
//   6. schema            — every registry declaring `$schema` is parsed and
//                          validated against it, in-gate.
//   6b. rule-resolution  — the PHASE B rule-resolution schemas parse, use only
//                          keywords this validator implements, and accept their
//                          representative valid fixtures while rejecting the
//                          invalid ones. Reached before any Instance population.
//   6c. functional-parity — the PHASE D functional-parity evidence schema parses,
//                          uses only implemented keywords, and — together with
//                          the record-level inference rules JSON Schema cannot
//                          express — classifies its product-neutral fixtures:
//                          every valid one satisfied, every invalid one rejected.
//   7. sha-provenance    — installed skill matches its pinned SHA-256.
//   8. ext-dependencies  — declared external dependencies are resolvable, or
//                          are explicitly and legibly unresolved.
//   9. inventory-git     — recorded revision/ref/dirty state is compared with
//                          the repository's actual current state.
//  10. instruction-topics — the two halves of the topic pool agree.
//  10b. stack-profiles  — the two halves of the stack profile pool agree, and
//                          every inventoried repository declares a profile from
//                          the pool that its own manifest supports.
//  11. instruction-intake — the register of agent instruction artifacts covers
//                          the tree, loses no record (against the file's whole
//                          history, and against the published branch when
//                          MERIDIAN_CHECK_PUBLISHED is set), names topics that
//                          exist and parents that hash as recorded.
//  12. git-provenance    — whether the control directories are under VCS.
//
// Deliberate limits, stated rather than hidden:
//   - The YAML reader and the JSON Schema validator below implement documented
//     subsets. Both THROW on any construct or keyword they do not implement.
//     The whole schema tree is walked up front (assertSupportedDeep), so an
//     unsupported keyword in a branch the data never visits still turns the
//     gate red instead of silently passing.
//   - The Kernel file set is enumerated from `git ls-files`, not from a
//     hand-maintained list, so it cannot drift from what the repository
//     actually tracks. kernel-boundary.md remains the normative statement of
//     what belongs on which side; this script covers everything tracked.
//     "Everything tracked" is exactly the limit: a file that exists in the
//     tree but has never been added is invisible to every check here, so the
//     untracked set is enumerated too and reported as an explicit WARN rather
//     than left for the reader to infer from a count.
//
// Usage: node scripts/kernel-validate.mjs
// Roots: MERIDIAN_KERNEL defaults to this repository. MERIDIAN_INSTANCE has no
// default: the Instance is a separate private repository, and its absence makes
// product-specific checks report UNVERIFIED rather than pass.

import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

// The YAML subset reader, the JSON Schema subset engine and the marked-region
// reader live in scripts/lib/ so this validator and scripts/rule-resolver.mjs
// share one implementation of each rather than each carrying its own copy.
// Behaviour is unchanged: all three modules are the verbatim code that used to
// sit inline here.
import { yamlParse } from './lib/yaml.mjs';
import { validate, assertSupportedDeep } from './lib/json-schema.mjs';
import { markedRegion, instructionRegions } from './lib/regions.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// The Kernel is this repository. The Instance is elsewhere, by construction:
// if it were reachable by a fixed relative path, it could be committed here.
const KERNEL_ROOT = process.env.MERIDIAN_KERNEL || path.resolve(__dirname, '..');
const INSTANCE_ROOT = process.env.MERIDIAN_INSTANCE || null;
const AGENT_ROOT = INSTANCE_ROOT ? path.join(INSTANCE_ROOT, '.agent') : null;

const EXCLUDED_DIRS = new Set(['_to_delete', 'node_modules', '.git']);
const INVENTORY_TTL_DAYS = 3;

let failures = 0;
let warnings = 0;
const report = [];
const fail = (m) => { failures++; report.push(`FAIL  ${m}`); };
const warn = (m) => { warnings++; report.push(`WARN  ${m}`); };
const ok = (m) => report.push(`OK    ${m}`);
const info = (m) => report.push(`INFO  ${m}`);

const readIfExists = (p) => { try { return fs.readFileSync(p, 'utf8'); } catch { return null; } };

// ---------------------------------------------------------------------------
// blankFencedBlocks and markedRegion moved verbatim to scripts/lib/regions.mjs
// (imported above) so this validator and scripts/rule-resolver.mjs read marked
// regions through one implementation. The vocabulary is one pair of HTML
// comments — <!-- meridian:begin <name> [key=value ...] --> / <!-- meridian:end
// <name> --> — fenced examples are blanked before the search, and anything
// other than exactly one well-ordered pair is an error, never a fallback to the
// whole file. The parsing rules and every error string are unchanged.
// ---------------------------------------------------------------------------

function listFiles(dir, exts) {
  let out = [];
  let entries;
  try { entries = fs.readdirSync(dir, { withFileTypes: true }); } catch { return out; }
  for (const e of entries) {
    if (EXCLUDED_DIRS.has(e.name)) continue;
    const full = path.join(dir, e.name);
    if (e.isDirectory()) out = out.concat(listFiles(full, exts));
    else if (!exts || exts.some((ext) => e.name.endsWith(ext))) out.push(full);
  }
  return out;
}

// The YAML subset reader (UnsupportedYaml, blockScalarStyle, stripComment,
// parseFlow, parseScalar, yamlParse) and the JSON Schema subset engine
// (UnsupportedSchema, SUPPORTED_KEYWORDS, assertSupported, FORMAT_CHECKS,
// assertSupportedDeep, resolveRef, typeOf, validate) used to sit inline here.
// They now live in scripts/lib/yaml.mjs and scripts/lib/json-schema.mjs,
// imported at the top of this file, so that scripts/rule-resolver.mjs reads
// YAML and validates against JSON Schema through the same implementations
// rather than a second copy of each. The code moved verbatim; the documented
// subsets, the keyword set, the format checks and every throw are unchanged.

// ---------------------------------------------------------------------------
// Kernel file set. Enumerated from Git so it cannot drift from the repository:
// everything this repository tracks is Kernel and is subject to the purity
// scan — including VERSION, LICENSE, .gitignore, .github/, hooks/ and the
// test fixture, which a hand-maintained list once omitted. Binary artifacts
// cannot be text-scanned; they are covered by sha-provenance and counted
// separately so the "clean" claim never silently over-reaches.
// ---------------------------------------------------------------------------
const BINARY_EXTS = ['.zip'];
function gitTrackedFiles(root) {
  try {
    const out = execFileSync('git', ['-C', root, 'ls-files', '-z'], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] });
    const files = out.split('\0').filter(Boolean).map((f) => path.join(root, f));
    return files.length ? files : null;
  } catch { return null; }
}
// The complement of the set above, and the reason it has to be reported: the
// file set comes from the index, so a file that exists in the tree but has
// never been added is scanned by nothing — while the "N tracked text files
// clean" line below reads as "everything here is clean". Files excluded by
// .gitignore are deliberately outside the release unit and stay out; anything
// else untracked is named.
function gitUntrackedFiles(root) {
  try {
    const out = execFileSync('git', ['-C', root, 'ls-files', '--others', '--exclude-standard', '-z'], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] });
    return out.split('\0').filter(Boolean).map((f) => path.join(root, f));
  } catch { return null; }
}
const trackedKernelFiles = gitTrackedFiles(KERNEL_ROOT);
if (!trackedKernelFiles) {
  warn('kernel-purity: Git enumeration unavailable; scanning a filesystem walk instead (untracked files included)');
}
const KERNEL_FILES = trackedKernelFiles ?? listFiles(KERNEL_ROOT, null);
// Files under the Instance root are Instance data by definition, even when the
// Instance is the synthetic fixture tracked inside this repository. Scanning
// the fixture against its own declared literals would fail every fixture run
// on a self-match, so the Instance subtree is excluded — and the exclusion is
// reported, not hidden. With a real (external) Instance nothing is excluded
// and the fixture subtree IS scanned against the real product's literals.
const RESOLVED_INSTANCE_ROOT = INSTANCE_ROOT ? path.resolve(INSTANCE_ROOT) : null;
const underInstance = (f) => RESOLVED_INSTANCE_ROOT !== null
  && path.resolve(f).startsWith(RESOLVED_INSTANCE_ROOT + path.sep);
const kernelBinaryFiles = [...new Set(KERNEL_FILES)].filter((f) => BINARY_EXTS.some((e) => f.endsWith(e)));
const instanceExcluded = [...new Set(KERNEL_FILES)].filter((f) => underInstance(f));
if (instanceExcluded.length) {
  info(`kernel-purity: ${instanceExcluded.length} tracked file(s) under the Instance root are Instance data, excluded from the Kernel scan`);
}
// Untracked files in the Kernel tree: named here so that no later "clean" line
// can be read as covering them.
const untrackedKernelFiles = trackedKernelFiles
  ? (gitUntrackedFiles(KERNEL_ROOT) ?? []).filter((f) => !underInstance(f))
  : [];
if (untrackedKernelFiles.length) {
  const rels = untrackedKernelFiles
    .map((f) => path.relative(KERNEL_ROOT, f).split(path.sep).join('/'));
  warn(`kernel-purity: ${untrackedKernelFiles.length} file(s) in the Kernel tree are untracked and were therefore NOT scanned `
     + `by any check in this run (${rels.slice(0, 5).join(', ')}${rels.length > 5 ? ', …' : ''}); `
     + 'add them to the index or to .gitignore — a file the gate cannot see is not a file the gate approved');
}
const kernelFiles = [...new Set(KERNEL_FILES)]
  .filter((f) => fs.existsSync(f) && !BINARY_EXTS.some((e) => f.endsWith(e)) && !underInstance(f));

// forbidden literals are derived from the Instance product record
const productYamlPath = INSTANCE_ROOT ? path.join(INSTANCE_ROOT, 'product.yaml') : null;
const productRaw = productYamlPath ? readIfExists(productYamlPath) : null;
let forbiddenLiterals = [];
let forbiddenPatterns = [];
let productDoc = null;
if (productRaw) {
  try {
    productDoc = yamlParse(productRaw);
    forbiddenLiterals = [
      productDoc?.product?.name,
      productDoc?.canonical_wiki?.space_key,
      productDoc?.canonical_wiki?.base_url,
      productDoc?.tooling?.task_tracker?.base_url,
      // The repository axis identifies the product as surely as the wiki does:
      // a self-hosted git host and its namespace are product facts.
      productDoc?.tooling?.repository?.base_url,
      productDoc?.tooling?.repository?.namespace,
      // Anything else the Instance declares off-limits. Derivation from known
      // fields can only cover the fields that exist; this list covers the rest,
      // and being over-inclusive here costs nothing.
      ...(Array.isArray(productDoc?.forbidden_literals) ? productDoc.forbidden_literals : []),
    ].filter(Boolean);
    // Case-sensitive regex patterns for terms too common to match loosely:
    // a case-insensitive word-boundary literal for a product component named
    // after an everyday word would drown the gate in false positives, so the
    // Instance declares an exact pattern instead.
    forbiddenPatterns = (Array.isArray(productDoc?.forbidden_patterns) ? productDoc.forbidden_patterns : [])
      .map((p) => new RegExp(String(p)));
    ok(`product record parsed; ${forbiddenLiterals.length} kernel-purity literals and ${forbiddenPatterns.length} patterns derived`);
  } catch (e) {
    fail(`product record: ${e.message}`);
  }
} else if (!INSTANCE_ROOT) {
  fail('MERIDIAN_INSTANCE is not set; product literals were NOT checked. '
     + 'This run verifies personal-path leaks only and must not be reported as a clean kernel-purity result.');
} else {
  fail(`product record not found at ${productYamlPath}; kernel-purity cannot be verified`);
}

// Built from fragments so that this script does not itself contain the literal
// sequence it searches for (which would be an unavoidable self-match).
const PERSONAL_PATH_RE = new RegExp(['[A-Z]:', '\\\\', 'Users', '\\\\', '[^\\\\/\\r\\n]+'].join(''), 'i');

let purityFailures = 0;
const pfail = (m) => { purityFailures++; fail(m); };
for (const file of kernelFiles) {
  const text = readIfExists(file);
  if (text === null) continue;
  for (const lit of forbiddenLiterals) {
    const esc = String(lit).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const re = new RegExp(/^https?:\/\//i.test(lit) ? esc : `\\b${esc}\\b`, 'i');
    if (re.test(text)) pfail(`kernel-purity: product literal "${lit}" found in ${file}`);
  }
  for (const re of forbiddenPatterns) {
    const m = text.match(re);
    if (m) pfail(`kernel-purity: forbidden pattern ${re} matched "${m[0]}" in ${file}`);
  }
  const personal = text.match(PERSONAL_PATH_RE);
  if (personal) pfail(`kernel-purity: personal home path "${personal[0]}" found in ${file}`);
}
if (purityFailures === 0 && productDoc) {
  ok(`kernel-purity: ${kernelFiles.length} tracked text files clean; `
   + `${kernelBinaryFiles.length} binary artifact(s) covered by sha-provenance, not by this text scan`
   + (untrackedKernelFiles.length ? `; ${untrackedKernelFiles.length} untracked file(s) outside this claim, see WARN above` : ''));
}

// ---------------------------------------------------------------------------
// .agent artifacts
// ---------------------------------------------------------------------------
const agentMd = AGENT_ROOT
  ? ['analysis', 'plans', 'reports', 'technical-specifications', 'publication-drafts']
      .flatMap((d) => listFiles(path.join(AGENT_ROOT, d), ['.md']))
  : [];
if (!AGENT_ROOT) warn('front-matter/path-placement: no Instance root, working-memory artifacts were NOT checked');

const TERMINAL_STATUSES = ['completed', 'archived', 'superseded', 'fulfilled', 'invalidated', 'rejected', 'withdrawn'];
for (const file of agentMd) {
  const text = readIfExists(file);
  if (text === null) continue;
  if (!text.startsWith('---')) { warn(`front-matter: ${file} has no leading Front Matter block`); continue; }
  const st = text.match(/\n\s*status:\s*([a-z_-]+)/i)?.[1]?.toLowerCase();
  if (!st) continue;
  if (/[\\/]active[\\/]/i.test(file) && TERMINAL_STATUSES.includes(st)) {
    warn(`path-placement: ${file} sits in "active" but status is terminal ("${st}")`);
  }
  if (/[\\/](done|archive)[\\/]/i.test(file) && st === 'active') {
    warn(`path-placement: ${file} sits in "done"/"archive" but status is "active"`);
  }
}

// duplicate/orphaned trailing Front Matter — over Kernel *and* .agent
const allMd = [...kernelFiles.filter((f) => f.endsWith('.md')), ...agentMd];
const TRAILING_FM = /\n---[ \t]*\r?\n(?:[a-z_]+:[^\n]*\r?\n)+---[ \t]*\r?\n?\s*$/i;
for (const file of allMd) {
  const text = readIfExists(file);
  if (text === null || !text.startsWith('---')) continue;
  const closeIdx = text.indexOf('\n---', 3);
  if (closeIdx === -1) continue;
  if (TRAILING_FM.test('\n' + text.slice(closeIdx + 4))) {
    fail(`duplicate-fm: orphaned trailing Front Matter block at end of ${file}`);
  }
}
ok(`duplicate-fm: ${allMd.length} Markdown files checked (Kernel and .agent)`);

// links
// Confinement is decided on real paths, not textual prefixes: a symlink or an
// NTFS junction inside the Kernel can make a textual descendant of KERNEL_ROOT
// live physically elsewhere, and a prefix comparison would bless the escape.
// The deepest existing ancestor is realpath'd and the remaining segments are
// re-appended, so a link through a symlinked directory is judged by where it
// actually lands.
function realResolve(p) {
  let base = p;
  const rest = [];
  while (!fs.existsSync(base)) {
    const parent = path.dirname(base);
    if (parent === base) break;
    rest.unshift(path.basename(base));
    base = parent;
  }
  let real;
  try { real = fs.realpathSync(base); } catch { real = base; }
  return path.join(real, ...rest);
}
const REAL_KERNEL_ROOT = (() => {
  try { return fs.realpathSync(KERNEL_ROOT); } catch { return path.resolve(KERNEL_ROOT); }
})();
function escapesKernel(target) {
  const rel = path.relative(REAL_KERNEL_ROOT, realResolve(target));
  return rel !== '' && (rel.startsWith('..') || path.isAbsolute(rel));
}
const LINK_RE = /\]\((\.\.?\/[^)#\s]+|[^):#\s]+\.md)\)/g;
for (const file of allMd) {
  const text = readIfExists(file);
  if (text === null) continue;
  let m;
  while ((m = LINK_RE.exec(text))) {
    if (/^https?:\/\//i.test(m[1])) continue;
    const resolved = path.resolve(path.dirname(file), m[1]);
    // A Kernel link that escapes the Kernel root resolves only by accident of
    // whatever happens to sit next to this checkout. On a clean clone elsewhere
    // it breaks, so "it resolved here" is not evidence that it is correct.
    if (file.startsWith(KERNEL_ROOT + path.sep) && escapesKernel(resolved)) {
      fail(`link: ${file} -> "${m[1]}" points outside the Kernel; reference Instance material as a $MERIDIAN_INSTANCE path, do not link to it`);
      continue;
    }
    if (!fs.existsSync(resolved)) fail(`link: ${file} -> "${m[1]}" does not resolve`);
  }
}
ok(`link check ran over ${allMd.length} files`);

// ---------------------------------------------------------------------------
// document identity: file names and declared document types
// Rule, not list: see standards/workspace/document-identity.md. The exemptions
// below are shaped as patterns for the same reason the Kernel file set comes
// from git ls-files — a hand-maintained list of files drifts from the tree.
// ---------------------------------------------------------------------------
// "skill" is deliberately absent: its signature duplicated "protocol" and
// differed only in how the artifact is delivered. Delivery is a field of its
// own now (agent-instruction-identity.md), not a type.
const DOCUMENT_TYPES = new Set([
  'standard', 'contract', 'protocol', 'template', 'readme', 'reference',
  'tutorial', 'how-to', 'explanation', 'rfc', 'adr', 'concept', 'problem',
  'incident', 'analysis', 'plan', 'report', 'technical-specification',
  'changelog', 'unclassified',
]);
// Names fixed by conventions this Kernel does not own and must not "correct".
const EXTERNAL_NAMES = new Set(['README.md', 'SKILL.md', 'AGENTS.md', 'PIN.yaml', 'LICENSE', 'VERSION']);
// Upper case is legal only for the release unit's root set: the files someone
// who has just opened the repository is expected to find without looking.
const ROOT_UPPERCASE = new Set(['README.md', 'LICENSE', 'VERSION', 'CHANGELOG.md', 'COMPATIBILITY.md', 'MANUAL.md']);
const LOWER_SEGMENT = /^\.?[a-z0-9]+([._-][a-z0-9]+)*$/;
// State that belongs in Front Matter, where changing it does not break links.
const NAME_SMELL = /(^|-)(final|new|old|copy|tmp|draft)(-|\.|$)|\d{4}-\d{2}-\d{2}|(^|-)v?\d+\.\d+(\.\d+)?(-|\.|$)/;
// A file that exists to be copied carries no Front Matter of its own (it would
// carry the Kernel's metadata into another release unit); a vendored artifact
// is pinned by SHA and must not be edited to add one; fixture data is test
// input, not documentation.
const carriesOwnFrontMatter = (rel) =>
  !/-(template|body)\.md$/.test(rel)
  && !rel.startsWith('instance-template/')
  && !rel.startsWith('test/')
  && path.posix.basename(rel) !== 'SKILL.md';

let identityFailures = 0;
const idfail = (m) => { identityFailures++; fail(m); };
const relKernelFiles = [...new Set(KERNEL_FILES)]
  .filter((f) => !underInstance(f))
  .map((f) => path.relative(KERNEL_ROOT, f).split(path.sep).join('/'));

// The upper-case root set belongs to the root of a RELEASE UNIT, not to the
// root of the repository: the Kernel now carries a nested unit with its own
// version line, and reading the rule as "top level only" would have forced that
// unit to rename the very files that make it a unit. A release unit is
// recognised by carrying its own VERSION — a checkable property, not a list to
// keep in step by hand.
const RELEASE_UNIT_ROOTS = new Set(['']);
for (const rel of relKernelFiles) {
  if (path.posix.basename(rel) === 'VERSION') {
    const dir = path.posix.dirname(rel);
    RELEASE_UNIT_ROOTS.add(dir === '.' ? '' : dir);
  }
}

for (const rel of relKernelFiles) {
  const segs = rel.split('/');
  const base = segs[segs.length - 1];
  for (const seg of segs.slice(0, -1)) {
    if (!LOWER_SEGMENT.test(seg)) {
      idfail(`document-identity: directory "${seg}" in "${rel}" is not lower kebab-case`);
    }
  }
  const dir = segs.length === 1 ? '' : segs.slice(0, -1).join('/');
  const rootAllowed = ROOT_UPPERCASE.has(base) && RELEASE_UNIT_ROOTS.has(dir);
  if (!EXTERNAL_NAMES.has(base) && !rootAllowed && !LOWER_SEGMENT.test(base)) {
    idfail(`document-identity: "${rel}" is not lower kebab-case; upper case is reserved for the release unit's root set`);
  }
  if (NAME_SMELL.test(base)) {
    idfail(`document-identity: "${rel}" carries a date, a version or one of final/new/old/copy/tmp/draft in its name; that state belongs in Front Matter`);
  }
}

let typedDocs = 0;
for (const rel of relKernelFiles.filter((r) => r.endsWith('.md') && carriesOwnFrontMatter(r))) {
  const text = readIfExists(path.join(KERNEL_ROOT, rel));
  if (text === null) continue;
  if (!text.startsWith('---')) {
    idfail(`document-identity: ${rel} has no Front Matter; a Kernel document declares its type rather than leaving it to be guessed`);
    continue;
  }
  const end = text.indexOf('\n---', 3);
  const fm = end === -1 ? '' : text.slice(4, end);
  // `[^\S\r\n]` and not `\s`: `\s` matches the newline, so `field:\s*\S` was
  // satisfied by the first character of the NEXT line and an empty field
  // passed. Five of the six required fields could be blank and the run stayed
  // green — the check that was supposed to catch exactly that.
  for (const field of ['title', 'status', 'scope', 'owner', 'created', 'updated']) {
    if (!new RegExp(`^${field}:[^\\S\\r\\n]*\\S`, 'm').test(fm)) {
      idfail(`document-identity: ${rel} is missing required Front Matter field "${field}"`);
    }
  }
  const type = fm.match(/^document_type:\s*(\S+)/m)?.[1];
  if (!type) {
    idfail(`document-identity: ${rel} declares no document_type`);
  } else if (!DOCUMENT_TYPES.has(type)) {
    idfail(`document-identity: ${rel} declares document_type "${type}", which is not in the pool; a new type is a change to the standard, not to one file`);
  } else if (type === 'unclassified' && !/^unclassified_reason:[^\S\r\n]*\S/m.test(fm)) {
    idfail(`document-identity: ${rel} is unclassified with no unclassified_reason; the state is legal, an unexplained one is not`);
  } else {
    typedDocs++;
  }
}
if (identityFailures === 0) {
  ok(`document-identity: ${relKernelFiles.length} tracked names conform; `
   + `${typedDocs} documents declare a type from the pool`);
}


// ---------------------------------------------------------------------------
// registry schema validation, in-gate
// ---------------------------------------------------------------------------
const registryYaml = [
  ...(INSTANCE_ROOT ? listFiles(INSTANCE_ROOT, ['.yaml', '.yml']) : []),
  ...listFiles(KERNEL_ROOT, ['.yaml', '.yml']),
];
let validated = 0;
let attempted = 0;
for (const file of registryYaml) {
  const raw = readIfExists(file);
  if (raw === null) continue;
  let doc;
  try { doc = yamlParse(raw); }
  catch (e) { fail(`schema: cannot parse ${file}: ${e.message}`); continue; }
  const schemaRef = typeof doc?.$schema === 'string' ? doc.$schema : null;
  if (!schemaRef) continue;
  attempted++;
  let schemaPath = path.resolve(path.dirname(file), schemaRef);
  let schemaRaw = readIfExists(schemaPath);
  if (schemaRaw === null && schemaRef.startsWith('./')) {
    // Registry data is Instance; the schema describing it is Kernel. Resolve by
    // registry directory name, never by bare filename: `commands` and
    // `inventory` both declare a `repositories.schema.json`, and matching on the
    // filename alone would silently validate a document against the wrong schema.
    const registry = path.basename(path.dirname(file));
    const candidate = path.join(KERNEL_ROOT, 'registries', registry, schemaRef.slice(2));
    const alt = readIfExists(candidate);
    if (alt !== null) { schemaPath = candidate; schemaRaw = alt; }
  }
  if (schemaRaw === null) { fail(`schema: ${file} references ${schemaRef}, which resolves to no file in the Instance or the Kernel`); continue; }
  let schema;
  try { schema = JSON.parse(schemaRaw); }
  catch (e) { fail(`schema: ${schemaPath} is not valid JSON: ${e.message}`); continue; }
  const errs = [];
  try {
    // Reject unsupported keywords anywhere in the schema before validating,
    // including branches this particular document never exercises.
    assertSupportedDeep(schema, schemaPath);
    validate(doc, schema, schema, '', errs);
  }
  catch (e) { fail(`schema: ${file} could not be validated: ${e.message}`); continue; }
  if (errs.length) errs.slice(0, 10).forEach((er) => fail(`schema: ${file} ${er}`));
  else validated++;
}
if (attempted === 0) warn('schema: no registry declared a $schema; nothing was validated');
else ok(`schema: ${validated}/${attempted} registries passed in-gate validation against their declared JSON Schema`);

// ---------------------------------------------------------------------------
// rule-resolution: the PHASE B schemas are reachable by this gate
// ---------------------------------------------------------------------------
// The generic pass above only reaches a document that declares `$schema`
// pointing at a local file; a JSON Schema is not such a document and would
// otherwise sit unchecked until PHASE F writes data against it. Here the two
// PHASE B schemas are parsed, walked for unsupported keywords (the same
// assertSupportedDeep the generic pass uses), and exercised against bundled
// fixtures. The check is fail-closed on the fixture bundle's own shape: the
// success line is printed only when the bundle is an array carrying exactly
// one recognised group for each of the two schemas, every group's `valid` and
// `invalid` are non-empty arrays, and every fixture behaved as declared. A
// bundle that is an object, empty, short a group, carrying a stray or
// duplicate group, or carrying an empty `valid`/`invalid` is a FAIL — a
// schema no run exercises is not one this gate has reached.
{
  const rrDir = path.join(KERNEL_ROOT, 'registries', 'rule-resolution');
  const rrNames = ['applicability.schema.json', 'resolver-output.schema.json'];
  const rrRaw = new Map(rrNames.map((n) => [n, readIfExists(path.join(rrDir, n))]));
  const rrPresent = rrNames.filter((n) => rrRaw.get(n) !== null);
  if (rrPresent.length === 0) {
    info('rule-resolution: no PHASE B schema in this Kernel; nothing to check');
  } else {
    let rrOk = true;
    const rrSchemas = new Map();
    for (const n of rrNames) {
      const raw = rrRaw.get(n);
      if (raw === null) {
        fail(`rule-resolution: ${n} is missing while the other PHASE B schema is present; that is a gap, not an opt-out`);
        rrOk = false; continue;
      }
      let parsed;
      try { parsed = JSON.parse(raw); }
      catch (e) { fail(`rule-resolution: ${n} is not valid JSON: ${e.message}`); rrOk = false; continue; }
      try { assertSupportedDeep(parsed, n); }
      catch (e) { fail(`rule-resolution: ${n} uses a construct this validator cannot check: ${e.message}`); rrOk = false; continue; }
      rrSchemas.set(n, parsed);
    }
    let satisfied = 0;
    let rejected = 0;
    let coverageComplete = false;
    const fxRaw = readIfExists(path.join(rrDir, 'fixtures', 'rule-resolution.fixtures.json'));
    if (fxRaw === null) {
      fail('rule-resolution: the PHASE B schemas carry no fixtures (registries/rule-resolution/fixtures/rule-resolution.fixtures.json); a schema no run exercises is not one this gate has reached');
      rrOk = false;
    } else if (rrOk) {
      let bundle;
      let bundleOk = true;
      try { bundle = JSON.parse(fxRaw); }
      catch (e) { bundleOk = false; fail(`rule-resolution: the fixtures file is not valid JSON: ${e.message}`); }
      if (bundleOk && !Array.isArray(bundle)) {
        bundleOk = false;
        fail('rule-resolution: the fixtures file must be an array of one group per PHASE B schema; it is not an array');
      }
      if (bundleOk && bundle.length === 0) {
        bundleOk = false;
        fail('rule-resolution: the fixtures file is an empty array; neither PHASE B schema is covered');
      }
      if (!bundleOk) {
        rrOk = false;
      } else {
        // Exactly one recognised group per schema: no stray, no duplicate, none missing.
        const byName = new Map();
        for (const group of bundle) {
          const name = group?.schema;
          if (!rrSchemas.has(name)) {
            fail(`rule-resolution: a fixture group names schema "${name}", which is not one of the two PHASE B schemas`);
            rrOk = false; continue;
          }
          if (byName.has(name)) {
            fail(`rule-resolution: schema "${name}" has more than one fixture group; exactly one is expected`);
            rrOk = false; continue;
          }
          byName.set(name, group);
        }
        for (const n of rrNames) {
          if (!byName.has(n)) { fail(`rule-resolution: no fixture group for "${n}"; both PHASE B schemas must be covered`); rrOk = false; }
        }
        for (const [name, group] of byName) {
          const schema = rrSchemas.get(name);
          const groupShapeOk = ['valid', 'invalid'].every((k) => {
            if (Array.isArray(group[k]) && group[k].length > 0) return true;
            fail(`rule-resolution: fixture group "${name}" has no non-empty "${k}" array`);
            rrOk = false;
            return false;
          });
          if (!groupShapeOk) continue;
          for (const c of group.valid) {
            const e = [];
            let threw = null;
            try { validate(c?.doc, schema, schema, '', e); } catch (err) { threw = err; }
            if (threw) { fail(`rule-resolution: a fixture that must satisfy ${name} threw (${c?.note}): ${threw.message}`); rrOk = false; }
            else if (e.length) { fail(`rule-resolution: a fixture that must satisfy ${name} did not (${c?.note}): ${e[0]}`); rrOk = false; }
            else satisfied++;
          }
          for (const c of group.invalid) {
            const e = [];
            let threw = null;
            try { validate(c?.doc, schema, schema, '', e); } catch (err) { threw = err; }
            if (threw) { fail(`rule-resolution: an invalid fixture for ${name} threw instead of being rejected with errors (${c?.note}): ${threw.message}`); rrOk = false; }
            else if (e.length === 0) { fail(`rule-resolution: a fixture that must violate ${name} validated clean (${c?.note})`); rrOk = false; }
            else rejected++;
          }
        }
        coverageComplete = rrOk && rrNames.every((n) => byName.has(n));
      }
    }
    if (rrOk && coverageComplete) {
      ok('rule-resolution: 2/2 PHASE B schemas parsed and keyword-checked; '
       + `${satisfied} representative fixture(s) satisfied them and ${rejected} were rejected as declared`);
    }
  }
}

// ---------------------------------------------------------------------------
// functional-parity: the PHASE D evidence schema is reachable by this gate
// ---------------------------------------------------------------------------
// verification/functional-parity/ carries a portable functional-parity evidence
// contract, its JSON Schema and product-neutral fixtures. The generic $schema
// pass never reaches a JSON Schema file or a .json fixtures bundle, so — as with
// the rule-resolution block above — the schema is parsed, walked for unsupported
// keywords, and exercised against its bundled fixtures here, before any Instance
// population against it exists. Beyond the schema, the inference rules the
// draft-07 subset cannot express because they relate one part of a record to
// another are enforced against every fixture by functionalParityConsistency():
//    1. the document carries at least one record;
//    2. every preserved-contract assertion id is unique across the whole
//       record, the four facets included, and maps to exactly one owning facet;
//    3. every per-assertion verdict names a declared assertion, and every
//       declared assertion carries exactly one per-assertion verdict;
//    4. every evidence `covers` id resolves to a declared assertion;
//    5. every post-change `contract_links` entry resolves to the exact
//       {facet, assertion_id} pair of a declared assertion; a right id under
//       the wrong facet is rejected;
//    6. a per-assertion VERIFIED needs BOTH a covering evidence entry AND a
//       post-change contract link that resolves to it; `covers` alone is not
//       enough (an empty `contract_links` array is legal only when nothing is
//       VERIFIED);
//    7. any UNVERIFIED per-assertion state forces an UNVERIFIED overall;
//    8. a record-scoped gap forces every declared per-assertion verdict
//       UNVERIFIED and the overall UNVERIFIED; an assertion-scoped gap forces
//       only the assertions it names, leaving the rest free to stand;
//    9. `relationship: same` means the post-change observation referenced every
//       baseline condition id and no others, with no duplicate baseline
//       condition id; the word in a description is not evidence of it;
//   10. a baseline whose provenance is not established forces every declared
//       per-assertion verdict UNVERIFIED and the overall UNVERIFIED, whatever
//       gaps are recorded;
//   11. the baseline and post-change source states are a distinguishable pair;
//       the same {identifier_kind, identifier} for both is not a before/after
//       comparison.
// Fail-closed on the bundle's own shape, exactly as the rule-resolution block
// is: the success line prints only when the bundle is a non-empty array
// carrying exactly one group for the evidence schema, with a non-empty `valid`
// and `invalid`, and every fixture behaved as declared.
function functionalParityConsistency(rec) {
  const problems = [];
  const records = Array.isArray(rec?.records) ? rec.records : [];
  const FACETS = ['public_api', 'observable_io', 'side_effects_and_interactions', 'user_visible_behavior'];

  // 1. the document carries at least one record
  if (records.length === 0) {
    problems.push('the evidence document carries no records; a functional-parity document with no record proves nothing');
  }

  for (let ri = 0; ri < records.length; ri++) {
    const r = records[ri];
    const at = records.length > 1 ? ` (record ${ri})` : '';
    const pc = r?.preserved_contract ?? {};

    // 2. assertion catalogue: id -> its one owning facet, unique record-wide
    const catalog = new Map();
    for (const fn of FACETS) {
      for (const a of (pc[fn]?.assertions ?? [])) {
        const id = String(a?.id ?? '');
        if (!id) continue;
        if (catalog.has(id)) {
          problems.push(`assertion id "${id}"${at} is declared more than once (facets "${catalog.get(id)}" and "${fn}"); ids are unique across the whole record`);
        } else {
          catalog.set(id, fn);
        }
      }
    }
    const declared = new Set(catalog.keys());
    if (declared.size === 0) {
      problems.push(`no preserved-contract assertion is declared${at}; a parity record with no assertion proves nothing`);
    }

    // 3. one per-assertion verdict per declared assertion, none stray
    const verdict = r?.verdict ?? {};
    const perAssertion = Array.isArray(verdict.per_assertion) ? verdict.per_assertion : [];
    const seen = new Map();
    for (const v of perAssertion) {
      const id = String(v?.assertion_id ?? '');
      seen.set(id, (seen.get(id) ?? 0) + 1);
      if (!declared.has(id)) problems.push(`the verdict names assertion "${id}"${at}, which no preserved-contract facet declares`);
    }
    for (const id of declared) {
      const n = seen.get(id) ?? 0;
      if (n === 0) problems.push(`assertion "${id}"${at} carries no per-assertion verdict`);
      else if (n > 1) problems.push(`assertion "${id}"${at} carries more than one per-assertion verdict`);
    }
    const stateOf = new Map(perAssertion.map((v) => [String(v?.assertion_id ?? ''), String(v?.state ?? '')]));

    // 4. evidence `covers` -> declared assertion
    const covered = new Set();
    for (const ev of (Array.isArray(r?.evidence) ? r.evidence : [])) {
      for (const id of (Array.isArray(ev?.covers) ? ev.covers : [])) {
        const s = String(id);
        if (!declared.has(s)) problems.push(`an evidence entry covers assertion "${s}"${at}, which no preserved-contract facet declares`);
        else covered.add(s);
      }
    }

    // 5. post-change contract link -> exact {facet, assertion_id} pair
    const linked = new Set();
    for (const link of (Array.isArray(r?.post_change_evidence?.contract_links) ? r.post_change_evidence.contract_links : [])) {
      const s = String(link?.assertion_id ?? '');
      const f = String(link?.facet ?? '');
      if (!declared.has(s)) {
        problems.push(`a post-change contract link names assertion "${s}"${at}, which no preserved-contract facet declares`);
      } else if (catalog.get(s) !== f) {
        problems.push(`a post-change contract link names assertion "${s}"${at} under facet "${f}", but it is declared under facet "${catalog.get(s)}"`);
      } else {
        linked.add(s);
      }
    }

    // 6. a per-assertion VERIFIED needs BOTH covering evidence AND a link
    //    (an empty contract_links array is legal only when nothing is VERIFIED)
    for (const id of declared) {
      if (stateOf.get(id) !== 'VERIFIED') continue;
      if (!covered.has(id)) problems.push(`assertion "${id}"${at} is VERIFIED but no evidence entry covers it`);
      if (!linked.has(id)) problems.push(`assertion "${id}"${at} is VERIFIED but no post-change contract link resolves to it; evidence coverage alone is not sufficient`);
    }

    // 7. any UNVERIFIED per-assertion state forces an UNVERIFIED overall
    const anyUnverified = [...stateOf.values()].some((s) => s === 'UNVERIFIED');
    if (anyUnverified && String(verdict.overall ?? '') !== 'UNVERIFIED') {
      problems.push(`a per-assertion verdict is UNVERIFIED${at} but the overall verdict is not`);
    }

    // 8. gaps force their assertion(s), or the whole record, UNVERIFIED. A
    //    record-scoped gap reaches every declared assertion, not just the
    //    overall verdict; an assertion-scoped gap reaches only the assertions
    //    it names.
    for (const g of (Array.isArray(r?.gaps) ? r.gaps : [])) {
      if (g?.scope === 'record') {
        for (const id of declared) {
          if (stateOf.get(id) === 'VERIFIED') {
            problems.push(`a record-scoped gap is recorded${at} but assertion "${id}" is VERIFIED; a record-scoped gap leaves every assertion UNVERIFIED`);
          }
        }
        if (String(verdict.overall ?? '') !== 'UNVERIFIED') {
          problems.push(`a record-scoped gap is recorded${at} but the overall verdict is not UNVERIFIED`);
        }
      } else if (g?.scope === 'assertion') {
        for (const id of (Array.isArray(g.assertion_ids) ? g.assertion_ids : [])) {
          const s = String(id);
          if (!declared.has(s)) problems.push(`a gap names assertion "${s}"${at}, which no preserved-contract facet declares`);
          else if (stateOf.get(s) !== 'UNVERIFIED') problems.push(`a gap leaves assertion "${s}"${at} unclosed but its verdict is not UNVERIFIED`);
        }
      }
    }

    // 9. `relationship: same` means every baseline condition id, and no others
    const baseCondIds = new Set();
    for (const c of (Array.isArray(r?.baseline?.inputs_and_conditions) ? r.baseline.inputs_and_conditions : [])) {
      const id = String(c?.id ?? '');
      if (!id) continue;
      if (baseCondIds.has(id)) problems.push(`baseline condition id "${id}"${at} is declared more than once`);
      else baseCondIds.add(id);
    }
    const pcCond = r?.post_change_evidence?.inputs_and_conditions ?? {};
    if (pcCond.relationship === 'same') {
      const refs = (Array.isArray(pcCond.baseline_condition_ids) ? pcCond.baseline_condition_ids : []).map(String);
      const refSet = new Set(refs);
      for (const ref of refs) {
        if (!baseCondIds.has(ref)) problems.push(`post-change conditions are declared "same"${at} but reference baseline condition "${ref}", which the baseline does not define`);
      }
      for (const id of baseCondIds) {
        if (!refSet.has(id)) problems.push(`post-change conditions are declared "same"${at} but do not reproduce baseline condition "${id}"`);
      }
    }

    // 10. an unestablished baseline forces EVERY declared assertion, and the
    //     overall, UNVERIFIED — a gap does not rescue any assertion
    if (r?.baseline?.provenance?.established === false) {
      for (const id of declared) {
        if (stateOf.get(id) === 'VERIFIED') {
          problems.push(`the baseline provenance was not established${at} but assertion "${id}" is VERIFIED; an unestablished baseline leaves every assertion UNVERIFIED`);
        }
      }
      if (String(verdict.overall ?? '') !== 'UNVERIFIED') {
        problems.push(`the baseline provenance was not established${at} but the overall verdict is not UNVERIFIED`);
      }
    }

    // 11. the baseline and post-change source states must be a distinguishable
    //     pair. Both fields equal is not a before/after comparison; the same
    //     identifier string under a different identifier_kind is a different
    //     pair and is not rejected on that ground.
    const bss = r?.baseline?.source_state ?? {};
    const pss = r?.post_change_evidence?.source_state ?? {};
    const bId = String(bss.identifier ?? '');
    const bKind = String(bss.identifier_kind ?? '');
    if (bId !== '' && bId === String(pss.identifier ?? '') && bKind === String(pss.identifier_kind ?? '')) {
      problems.push(`the baseline and post-change source states are the identical pair {identifier_kind: "${bKind}", identifier: "${bId}"}${at}; a before/after comparison needs two distinguishable states`);
    }
  }
  return problems;
}
{
  const fpDir = path.join(KERNEL_ROOT, 'verification', 'functional-parity');
  const fpSchemaName = 'functional-parity-evidence.schema.json';
  const fpSchemaRaw = readIfExists(path.join(fpDir, fpSchemaName));
  if (fpSchemaRaw === null) {
    info('functional-parity: no PHASE D evidence schema in this Kernel; nothing to check');
  } else {
    let fpOk = true;
    let fpSchema = null;
    try { fpSchema = JSON.parse(fpSchemaRaw); }
    catch (e) { fail(`functional-parity: ${fpSchemaName} is not valid JSON: ${e.message}`); fpOk = false; }
    if (fpSchema) {
      try { assertSupportedDeep(fpSchema, fpSchemaName); }
      catch (e) { fail(`functional-parity: ${fpSchemaName} uses a construct this validator cannot check: ${e.message}`); fpOk = false; }
    }
    let satisfied = 0;
    let rejected = 0;
    let coverageComplete = false;
    const fxRaw = readIfExists(path.join(fpDir, 'fixtures', 'functional-parity-evidence.fixtures.json'));
    if (fxRaw === null) {
      fail('functional-parity: the PHASE D evidence schema carries no fixtures (verification/functional-parity/fixtures/functional-parity-evidence.fixtures.json); a schema no run exercises is not one this gate has reached');
      fpOk = false;
    } else if (fpOk) {
      let bundle;
      let bundleOk = true;
      try { bundle = JSON.parse(fxRaw); }
      catch (e) { bundleOk = false; fail(`functional-parity: the fixtures file is not valid JSON: ${e.message}`); }
      if (bundleOk && !Array.isArray(bundle)) {
        bundleOk = false;
        fail('functional-parity: the fixtures file must be an array carrying one group for the evidence schema; it is not an array');
      }
      if (bundleOk && Array.isArray(bundle) && bundle.length === 0) {
        bundleOk = false;
        fail('functional-parity: the fixtures file is an empty array; the evidence schema is not covered');
      }
      if (!bundleOk) {
        fpOk = false;
      } else {
        const groups = bundle.filter((g) => g?.schema === fpSchemaName);
        const strays = bundle.filter((g) => g?.schema !== fpSchemaName);
        if (strays.length) {
          fail(`functional-parity: a fixture group names schema "${strays[0]?.schema}", which is not the PHASE D evidence schema`);
          fpOk = false;
        }
        if (groups.length === 0) {
          fail(`functional-parity: no fixture group for "${fpSchemaName}"; the evidence schema must be covered`);
          fpOk = false;
        } else if (groups.length > 1) {
          fail(`functional-parity: schema "${fpSchemaName}" has more than one fixture group; exactly one is expected`);
          fpOk = false;
        } else {
          const group = groups[0];
          const groupShapeOk = ['valid', 'invalid'].every((k) => {
            if (Array.isArray(group[k]) && group[k].length > 0) return true;
            fail(`functional-parity: fixture group "${fpSchemaName}" has no non-empty "${k}" array`);
            fpOk = false;
            return false;
          });
          if (groupShapeOk) {
            for (const c of group.valid) {
              const e = [];
              let threw = null;
              try { validate(c?.doc, fpSchema, fpSchema, '', e); } catch (err) { threw = err; }
              const semantic = threw ? [] : functionalParityConsistency(c?.doc);
              if (threw) { fail(`functional-parity: a fixture that must satisfy ${fpSchemaName} threw (${c?.note}): ${threw.message}`); fpOk = false; }
              else if (e.length) { fail(`functional-parity: a fixture that must satisfy ${fpSchemaName} did not (${c?.note}): ${e[0]}`); fpOk = false; }
              else if (semantic.length) { fail(`functional-parity: a fixture that must satisfy the evidence inference rules did not (${c?.note}): ${semantic[0]}`); fpOk = false; }
              else satisfied++;
            }
            for (const c of group.invalid) {
              const e = [];
              let threw = null;
              try { validate(c?.doc, fpSchema, fpSchema, '', e); } catch (err) { threw = err; }
              const semantic = threw ? [] : functionalParityConsistency(c?.doc);
              if (threw) { fail(`functional-parity: an invalid fixture for ${fpSchemaName} threw instead of being rejected with errors (${c?.note}): ${threw.message}`); fpOk = false; }
              else if (e.length === 0 && semantic.length === 0) { fail(`functional-parity: a fixture that must be rejected by ${fpSchemaName} or its inference rules validated clean (${c?.note})`); fpOk = false; }
              else rejected++;
            }
            coverageComplete = fpOk;
          }
        }
      }
    }
    if (fpOk && coverageComplete) {
      ok('functional-parity: the PHASE D evidence schema parsed and keyword-checked; '
       + `${satisfied} representative fixture(s) satisfied it and its inference rules, and ${rejected} were rejected as declared`);
    }
  }
}

// ---------------------------------------------------------------------------
// SHA provenance
// ---------------------------------------------------------------------------
// Every vendored skill carries a PIN.yaml next to it. The pin is verified by
// recomputing the digest here; a pin that is merely recorded proves nothing.
const skillDirs = (() => {
  try {
    return fs.readdirSync(path.join(KERNEL_ROOT, 'skills'), { withFileTypes: true })
      .filter((e) => e.isDirectory()).map((e) => e.name);
  } catch { return []; }
})();
if (skillDirs.length === 0) warn('sha-provenance: no vendored skills found');
for (const name of skillDirs) {
  const dir = path.join(KERNEL_ROOT, 'skills', name);
  const pinRaw = readIfExists(path.join(dir, 'PIN.yaml'));
  if (pinRaw === null) { fail(`sha-provenance: ${name} has no PIN.yaml; its provenance is unverifiable`); continue; }
  let pin;
  try { pin = yamlParse(pinRaw); } catch (e) { fail(`sha-provenance: ${name} PIN.yaml: ${e.message}`); continue; }
  const artifact = readIfExists(path.join(dir, pin?.artifact ?? ''));
  if (artifact === null) { fail(`sha-provenance: ${name} pins "${pin?.artifact}", which does not exist`); continue; }
  const actual = crypto.createHash('sha256').update(artifact, 'utf8').digest('hex');
  if (!pin?.sha256) fail(`sha-provenance: ${name} PIN.yaml records no sha256`);
  else if (actual !== pin.sha256) fail(`sha-provenance: ${name} artifact ${actual.slice(0, 12)} != pinned ${String(pin.sha256).slice(0, 12)}`);
  else ok(`sha-provenance: ${name} matches its pin (${actual.slice(0, 12)}...)`);

  // A derived skill may declare that it is inapplicable without an Instance
  // context file (its product conventions live there). Missing context is a
  // blocker by the skill's own rule, so it is enforced here rather than
  // trusted to be remembered.
  const ctxRel = pin?.requires_instance_context;
  if (ctxRel) {
    if (!INSTANCE_ROOT) {
      warn(`instance-context: ${name} requires "${ctxRel}" from the Instance; no Instance root set, presence UNVERIFIED`);
    } else if (readIfExists(path.join(INSTANCE_ROOT, ctxRel)) === null) {
      fail(`instance-context: ${name} requires "${ctxRel}", which is missing from the Instance; the skill declares this a blocker, not a licence to guess`);
    } else {
      ok(`instance-context: ${name} context "${ctxRel}" present in the Instance`);
    }
  }

  const arch = pin?.source_archive;
  if (arch?.path && arch?.sha256) {
    const bytes = (() => { try { return fs.readFileSync(path.join(dir, arch.path)); } catch { return null; } })();
    if (bytes === null) fail(`sha-provenance: ${name} source archive ${arch.path} is missing`);
    else {
      const aActual = crypto.createHash('sha256').update(bytes).digest('hex');
      if (aActual !== arch.sha256) fail(`sha-provenance: ${name} source archive ${aActual.slice(0, 12)} != pinned ${String(arch.sha256).slice(0, 12)}`);
      else ok(`sha-provenance: ${name} source archive matches its pin (${aActual.slice(0, 12)}...)`);
    }
  }
}

// ---------------------------------------------------------------------------
// external dependencies
// ---------------------------------------------------------------------------
const extRaw = INSTANCE_ROOT ? readIfExists(path.join(INSTANCE_ROOT, 'external-dependencies.yaml')) : null;
if (extRaw) {
  try {
    const deps = yamlParse(extRaw)?.dependencies || [];
    for (const d of deps) {
      if (d.state === 'vendored') {
        info(`ext-dependency: ${d.id} vendored`);
      } else if (d.expected_sha256 || d.entry_sha256) {
        info(`ext-dependency: ${d.id} — ${d.state}, SHA pinned but file not vendored into Kernel`);
      } else {
        warn(`ext-dependency: ${d.id} is "${d.state}" with no pinned SHA — unverifiable on another machine`);
      }
    }
    ok(`ext-dependencies: ${deps.length} declared`);
  } catch (e) { fail(`ext-dependencies: ${e.message}`); }
} else warn(INSTANCE_ROOT ? 'ext-dependencies: record not found' : 'ext-dependencies: no Instance root, not checked');

// ---------------------------------------------------------------------------
// inventory: compare recorded Git facts with the repositories' actual state
// ---------------------------------------------------------------------------
function gitFacts(repoPath) {
  try {
    const g = (args) => execFileSync('git', ['-C', repoPath, ...args], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
    return { revision: g(['rev-parse', 'HEAD']), ref: g(['rev-parse', '--abbrev-ref', 'HEAD']), dirty: g(['status', '--porcelain']).length > 0 };
  } catch { return null; }
}

const invRaw = INSTANCE_ROOT ? readIfExists(path.join(INSTANCE_ROOT, 'inventory', 'repositories.yaml')) : null;
if (invRaw) {
  try {
    const repos = yamlParse(invRaw)?.repositories || [];
    let verified = 0;
    const now = Date.now();
    for (const r of repos) {
      const recorded = r.vcs || {};
      const actual = gitFacts(r.path);
      if (!actual) {
        // Unreachable-from-this-environment is the expected state for product
        // repositories when the validator runs outside the product workspace.
        // A warning that fires on every green run trains the operator to
        // ignore warnings, so the per-repository lines are informational and
        // the aggregate "0/N confirmed" below stays the single warning.
        info(`inventory-git: ${r.id} — repository at ${r.path} is not reachable from here; recorded revision is UNVERIFIED, not confirmed`);
        continue;
      }
      const problems = [];
      if (recorded.revision != null && String(recorded.revision) !== actual.revision) {
        problems.push(`revision recorded ${String(recorded.revision).slice(0, 7)} but HEAD is ${actual.revision.slice(0, 7)}`);
      }
      if (recorded.ref != null && String(recorded.ref) !== actual.ref) problems.push(`ref recorded "${recorded.ref}" but HEAD is on "${actual.ref}"`);
      const recordedDirty = recorded.working_tree === 'dirty';
      if (recorded.working_tree && recordedDirty !== actual.dirty) {
        problems.push(`working tree recorded "${recorded.working_tree}" but is now ${actual.dirty ? 'dirty' : 'clean'}`);
      }
      if (problems.length) fail(`inventory-git: ${r.id} — ${problems.join('; ')}; revalidate the entry, do not merely re-date it`);
      else verified++;

      const ts = Date.parse(r.last_verified ?? '');
      if (Number.isNaN(ts)) warn(`inventory-git: ${r.id} has no parseable last_verified`);
      else {
        const ageDays = (now - ts) / 86_400_000;
        if (ageDays > INVENTORY_TTL_DAYS) {
          warn(`inventory-git: ${r.id} last_verified is ${ageDays.toFixed(1)}d old (TTL ${INVENTORY_TTL_DAYS}d) — recheck due`);
        }
      }
    }
    if (verified) ok(`inventory-git: ${verified}/${repos.length} entries confirmed against actual repository state`);
    if (!verified && repos.length) warn(`inventory-git: 0/${repos.length} entries could be confirmed`);
  } catch (e) { fail(`inventory-git: ${e.message}`); }
} else warn(INSTANCE_ROOT ? 'inventory-git: repository inventory not found' : 'inventory-git: no Instance root, not checked');

// ---------------------------------------------------------------------------
// instruction-intake: nothing leaves the register silently
// ---------------------------------------------------------------------------
// Record shape is checked by the generic $schema pass above — but that pass is
// opt-in: it validates a file only if the file declares `$schema`. A register
// that simply omits the line was validated by nothing and still reported as
// covered, so the declaration is required here rather than assumed. What needs
// its own code beyond the shape is what a schema cannot express. Three claims,
// and each degrades
// to UNVERIFIED rather than to a pass when its evidence is out of reach:
//   completeness — every norm in the repository tree has a record;
//   preservation — no record disappeared relative to the previous revision;
//   parentage    — an edition was derived from the text it names.
const INTAKE_MASKS = [/\.mdc$/, /(^|\/)SKILL\.md$/, /(^|\/)AGENTS\.md$/, /(^|\/)CLAUDE\.md$/];

// Preservation is checked over records, not over the set of artifact paths the
// records name. A path-set comparison cannot see an artifact lose two of its
// three entries, because the path is still there — and the history is the
// whole point: "deferred, then retired" is two records, and the question people
// ask afterwards is about the transition. A record's identity is therefore the
// decision it carries and the thing it was carried about: artifact, region,
// date, verdict. Everything else in a record may be corrected in place; those
// four may not be edited away.
//
// `region` belongs in the key for the same reason `artifact` does. Without it,
// two records about one container, taken on one day with one verdict, differ
// only in which section they judged — and re-pointing one of them at the other
// section erased a decision while the multiset stayed the same size.
const KEY_SEP = '␟';
const recordKey = (r) => [r?.artifact, r?.region, r?.recorded_at, r?.verdict]
  .map((v) => String(v ?? '')).join(KEY_SEP);
const readableKey = (k) => {
  const [a, region, d, v] = k.split(KEY_SEP);
  return `${a}${region ? `#${region}` : ''} @ ${d} → ${v}`;
};
const countKeys = (keys) => {
  const m = new Map();
  for (const k of keys) m.set(k, (m.get(k) ?? 0) + 1);
  return m;
};
// Two identical records are two records, so the comparison counts rather than
// tests membership. Across revisions the requirement is per-key: the file must
// hold at least as many copies of a key as the fullest revision that ever did.
const lostKeys = (required, present) =>
  [...required].filter(([k, n]) => (present.get(k) ?? 0) < n).map(([k]) => k);
function mergeHighWaterMark(into, counts) {
  for (const [k, n] of counts) into.set(k, Math.max(into.get(k) ?? 0, n));
  return into;
}
// `--follow`: without it, renaming the register file in the same commit that
// drops a record leaves one revision to compare against — the truncated one —
// and the loss reads as "every record ever committed is still present". A
// rename is cheaper to perform than an amend, so the hole was the wider of the
// two.
// Following the rename is only half of it: at an earlier revision the file
// answered to its earlier name, so each revision is paired with the path the
// file had there. Following without that pairing turns every pre-rename
// revision into "could not be read", which is a warning where the answer
// should have been a red run.
function gitRevisionsOf(repo, rel) {
  try {
    const out = execFileSync('git', ['-C', repo, 'log', '--follow', '--format=%x01%H', '--name-status', '--', rel], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] });
    return out.split('\x01').filter((b) => b.trim()).map((block) => {
      const lines = block.split('\n').filter((l) => l.trim());
      const statusLine = lines.slice(1).find((l) => l.includes('\t'));
      return { rev: lines[0].trim(), path: statusLine ? statusLine.split('\t').pop().trim() : rel };
    });
  } catch { return null; }
}
// `git show <rev>:<path>` fails the same way for "this revision has no such
// file" and for "this revision is not in this checkout" (a shallow or partial
// clone). The two deserve opposite verdicts, so the revision is resolved first.
function gitHasRevision(repo, rev) {
  try {
    execFileSync('git', ['-C', repo, 'cat-file', '-e', `${rev}^{commit}`], { stdio: ['ignore', 'ignore', 'ignore'] });
    return true;
  } catch { return false; }
}
function gitShowAt(repo, rev, rel) {
  try { return execFileSync('git', ['-C', repo, 'show', `${rev}:${rel}`], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }); }
  catch { return null; }
}
function gitFilesAt(repo, rev, relDir) {
  try {
    return execFileSync('git', ['-C', repo, 'ls-tree', '-r', '--name-only', '-z', rev, '--', relDir], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] })
      .split('\0').filter((p) => /\.ya?ml$/i.test(p));
  } catch { return null; }
}
function gitDirectoryRevisions(repo, relDir) {
  try {
    return execFileSync('git', ['-C', repo, 'log', '--format=%H', '--', relDir], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] })
      .split('\n').map((s) => s.trim()).filter(Boolean);
  } catch { return null; }
}
function intakeDocumentsAt(repo, rev) {
  const paths = gitFilesAt(repo, rev, 'instruction-intake');
  if (paths === null) return null;
  return paths.map((rel) => {
    const raw = gitShowAt(repo, rev, rel);
    if (raw === null) return { rel, error: 'file could not be read' };
    try { return { rel, doc: yamlParse(raw) }; }
    catch (e) { return { rel, error: e.message }; }
  });
}
function gitUpstreamOf(repo) {
  try {
    return execFileSync('git', ['-C', repo, 'rev-parse', '--abbrev-ref', '--symbolic-full-name', '@{u}'], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim() || null;
  } catch { return null; }
}
function gitFetch(repo) {
  try { execFileSync('git', ['-C', repo, 'fetch', '--quiet'], { stdio: ['ignore', 'ignore', 'ignore'] }); return true; }
  catch { return false; }
}
// Container files hold several subjects at once and are partly written by a
// generator. The unit of intake for them is a declared region, not the file:
// one topic assigned to a whole AGENTS.md would be false about most of it.
// Regions declare themselves with the same marker vocabulary the topic pool
// uses, plus the two attributes a container needs — who owns the region, and
// whether a generator writes it.
const CONTAINER_MASKS = [/(^|\/)AGENTS\.md$/, /(^|\/)CLAUDE\.md$/];
// REGION_TOKEN, parseMarkerAttrs and instructionRegions moved verbatim to
// scripts/lib/regions.mjs (imported above). instructionRegions still blanks a
// leading Front Matter block and fenced examples, refuses nested, unnamed,
// duplicate, cross-closed and unclosed regions, and reports the lines that lie
// outside every declared region. The parsing rules and every error string are
// unchanged, so this validator's regression suite is unaffected.

// The published half of preservation needs the network, so it is opt-in: the
// Instance's pre-push hook sets this, and an ordinary offline run neither pays
// for it nor pretends to have done it.
const CHECK_PUBLISHED = Boolean(process.env.MERIDIAN_CHECK_PUBLISHED);
let publishedFetchAttempted = false;
let publishedFetchOk = false;

// The topic pool lives in two files on purpose: names where the gate can read
// them, signatures where a person can compare them. Two files holding one pool
// are two pools unless something checks they agree, so that is checked first
// and the pool is refused outright when they do not.
const topicsYamlRaw = readIfExists(path.join(KERNEL_ROOT, 'standards', 'workspace', 'instruction-topics.yaml'));
const topicsMdRaw = readIfExists(path.join(KERNEL_ROOT, 'standards', 'workspace', 'instruction-topics.md'));
let TOPIC_POOL = null;
if (topicsYamlRaw === null || topicsMdRaw === null) {
  // Fail-closed. The pool is a Kernel artifact, not an optional extra: without
  // it not one topic in any register can be judged, and reporting that as a
  // warning would let a register full of invented topics finish green. An
  // absent pool is a broken Kernel, and the gate says so.
  const missing = [
    topicsYamlRaw === null ? 'standards/workspace/instruction-topics.yaml' : null,
    topicsMdRaw === null ? 'standards/workspace/instruction-topics.md' : null,
  ].filter(Boolean);
  fail(`instruction-topics: the topic pool is missing from the Kernel (${missing.join(', ')}); `
     + 'no topic in any register could be checked against it, and an unreadable pool is a defect of the Kernel, not a reason to skip the check');
} else {
  let names = [];
  try { names = (yamlParse(topicsYamlRaw)?.topics || []).map(String); }
  catch (e) { fail(`instruction-topics: ${e.message}`); }
  // Signatures are read from the marked pool region only. Reading the whole
  // file would enrol any future table whose first cell is a back-quoted
  // identifier into the set of documented topics — and a name that is
  // "documented" by accident masks exactly the drift this check exists to
  // find. The region declares itself; an unmarked file is an error, not a
  // licence to fall back to the whole text.
  const region = markedRegion(topicsMdRaw, 'topic-pool');
  if (region.error) {
    fail(`instruction-topics: the pool region of instruction-topics.md is not readable — ${region.error}; `
       + 'the signatures the gate compares against are the ones inside the markers, and nothing else');
  } else {
    const documented = new Set(
      [...region.text.matchAll(/^\|\s*`([a-z0-9-]+)`\s*\|/gm)].map((m) => m[1]),
    );
    const declared = new Set(names);
    const undocumented = [...declared].filter((n) => !documented.has(n));
    const unlisted = [...documented].filter((n) => !declared.has(n));
    if (undocumented.length || unlisted.length) {
      const parts = [];
      if (undocumented.length) parts.push(`named in the data but carrying no signature: ${undocumented.join(', ')}`);
      if (unlisted.length) parts.push(`carrying a signature but absent from the data: ${unlisted.join(', ')}`);
      fail(`instruction-topics: the two halves of the pool disagree — ${parts.join('; ')}`);
    } else {
      TOPIC_POOL = declared;
      ok(`instruction-topics: ${declared.size} topics; names and signatures agree`);
    }
  }
}

// ---------------------------------------------------------------------------
// stack profiles: the pool, and every repository's declaration against it
// ---------------------------------------------------------------------------
// A profile is declared, never inferred. This check therefore does two
// separable things, and neither works without the other: it refuses a name the
// pool does not contain, and it refuses a declaration the repository's own
// manifest does not support. Inferring the profile from the manifest would be
// guessing; accepting the declaration unchecked would be a fact free to drift
// away from reality in silence.
const profilesYamlRaw = readIfExists(path.join(KERNEL_ROOT, 'stack-profiles', 'stack-profiles.yaml'));
const profilesMdRaw = readIfExists(path.join(KERNEL_ROOT, 'stack-profiles', 'stack-profiles.md'));
let STACK_PROFILES = null;
if (profilesYamlRaw === null || profilesMdRaw === null) {
  // Fail-closed, for the same reason the topic pool is: without the pool not
  // one declaration can be judged, and a register of invented profile names
  // would finish green.
  const missing = [
    profilesYamlRaw === null ? 'stack-profiles/stack-profiles.yaml' : null,
    profilesMdRaw === null ? 'stack-profiles/stack-profiles.md' : null,
  ].filter(Boolean);
  fail(`stack-profiles: the profile pool is missing from the Kernel (${missing.join(', ')}); `
     + 'no declaration could be checked against it, and an unreadable pool is a defect of the Kernel, not a reason to skip the check');
} else {
  let entries = [];
  let parsed = true;
  try { entries = yamlParse(profilesYamlRaw)?.profiles || []; }
  catch (e) { parsed = false; fail(`stack-profiles: ${e.message}`); }
  const region = markedRegion(profilesMdRaw, 'stack-profile-pool');
  if (region.error) {
    fail(`stack-profiles: the pool region of stack-profiles.md is not readable — ${region.error}; `
       + 'the signatures the gate compares against are the ones inside the markers, and nothing else');
  } else if (parsed) {
    const documented = new Set(
      [...region.text.matchAll(/^\|\s*`([a-z0-9-]+)`\s*\|/gm)].map((m) => m[1]),
    );
    const declared = new Set(entries.map((e) => String(e?.name ?? '')));
    const undocumented = [...declared].filter((n) => !documented.has(n));
    const unlisted = [...documented].filter((n) => !declared.has(n));
    if (undocumented.length || unlisted.length) {
      const parts = [];
      if (undocumented.length) parts.push(`named in the data but carrying no signature: ${undocumented.join(', ')}`);
      if (unlisted.length) parts.push(`carrying a signature but absent from the data: ${unlisted.join(', ')}`);
      fail(`stack-profiles: the two halves of the pool disagree — ${parts.join('; ')}`);
    } else if (declared.has('universal')) {
      // "universal" means the profile axis does not apply. Letting it into the
      // pool would make "no profile" and "this profile" the same declaration.
      fail('stack-profiles: "universal" is not a stack profile and must not be listed in the pool; it is the declared absence of one');
    } else {
      STACK_PROFILES = new Map(entries.map((e) => [String(e.name), e]));
      ok(`stack-profiles: ${STACK_PROFILES.size} profiles; names and signatures agree`);
    }
  }
}

// A manifest section is an object of dependency-name keys. Anything else is
// not a section, and a predicate written against it is unsatisfiable rather
// than trivially satisfied.
function manifestSection(doc, name) {
  const v = doc?.[name];
  return v && typeof v === 'object' && !Array.isArray(v) ? v : null;
}

function profileMismatches(spec, manifest) {
  const problems = [];
  for (const [section, names] of Object.entries(spec.requires || {})) {
    const sec = manifestSection(manifest, section);
    if (!sec) { problems.push(`manifest has no "${section}" section, which the profile requires`); continue; }
    const absent = names.filter((n) => !Object.prototype.hasOwnProperty.call(sec, String(n)));
    if (absent.length) problems.push(`"${section}" does not carry ${absent.map((n) => `"${n}"`).join(', ')}`);
  }
  for (const [section, names] of Object.entries(spec.forbids || {})) {
    const sec = manifestSection(manifest, section);
    if (!sec) continue;
    const present = names.filter((n) => Object.prototype.hasOwnProperty.call(sec, String(n)));
    if (present.length) problems.push(`"${section}" carries ${present.map((n) => `"${n}"`).join(', ')}, which this profile excludes`);
  }
  for (const field of spec.forbids_fields || []) {
    if (Object.prototype.hasOwnProperty.call(manifest ?? {}, String(field))) {
      problems.push(`manifest declares "${field}", which this profile excludes`);
    }
  }
  return problems;
}

if (invRaw && STACK_PROFILES) {
  try {
    const repos = yamlParse(invRaw)?.repositories || [];
    let confirmed = 0;
    let unconfirmed = 0;
    for (const r of repos) {
      const name = r.profile == null ? '' : String(r.profile).trim();
      if (!name) {
        fail(`stack-profile: ${r.id} declares no profile; a repository whose type is unstated cannot have its norms judged applicable or not`);
        continue;
      }
      if (name === 'universal') {
        fail(`stack-profile: ${r.id} declares "universal", which is the absence of a profile, not the type of a project`);
        continue;
      }
      if (!STACK_PROFILES.has(name)) {
        fail(`stack-profile: ${r.id} declares "${name}", which is not in the pool; a profile is named from the pool or added to it by decision, never invented at the point of use`);
        continue;
      }
      const spec = STACK_PROFILES.get(name);
      const manifestPath = (r.sources?.manifests || [])
        .map(String)
        .find((m) => path.basename(m.replace(/\\/g, '/')) === spec.manifest);
      if (!manifestPath) {
        fail(`stack-profile: ${r.id} declares "${name}", whose evidence is ${spec.manifest}, but the entry lists no such manifest`);
        continue;
      }
      const raw = readIfExists(manifestPath.replace(/\\/g, path.sep));
      if (raw === null) {
        // Same posture as inventory-git: unreachable is UNVERIFIED, never green.
        info(`stack-profile: ${r.id} — ${spec.manifest} at ${manifestPath} is not readable from here; the declared "${name}" is UNVERIFIED, not confirmed`);
        unconfirmed++;
        continue;
      }
      let manifest;
      try { manifest = JSON.parse(raw); }
      catch (e) { fail(`stack-profile: ${r.id} — ${manifestPath} is not valid JSON: ${e.message}`); continue; }
      const problems = profileMismatches(spec, manifest);
      if (problems.length) {
        fail(`stack-profile: ${r.id} declares "${name}" but its manifest does not support it — ${problems.join('; ')}; correct the declaration or the pool, do not widen the predicate to fit`);
      } else confirmed++;
    }
    if (confirmed) ok(`stack-profile: ${confirmed}/${repos.length} declarations confirmed against the repository's own manifest`);
    if (unconfirmed) warn(`stack-profile: ${unconfirmed}/${repos.length} declarations could not be confirmed; the manifest was not reachable from here`);
  } catch (e) { fail(`stack-profile: ${e.message}`); }
} else if (STACK_PROFILES && INSTANCE_ROOT) {
  warn('stack-profile: repository inventory not found; no declaration was checked');
} else if (STACK_PROFILES) {
  warn('stack-profile: no Instance root, declarations were not checked');
}

const intakeDir = INSTANCE_ROOT ? path.join(INSTANCE_ROOT, 'instruction-intake') : null;
const intakeFiles = intakeDir && fs.existsSync(intakeDir) ? listFiles(intakeDir, ['.yaml', '.yml']) : [];

// A per-file check cannot see the most destructive loss: removing the whole
// register also removes the loop that would have checked its history. Keep a
// directory-level identity guard keyed by `repository`, which is stable across
// file renames. This guard intentionally checks existence only; the per-file
// comparison below remains responsible for record-level high-water marks.
if (INSTANCE_ROOT) {
  const currentRepoIds = new Set();
  const currentRepoFiles = new Map();
  for (const file of intakeFiles) {
    try {
      const repoId = String(yamlParse(readIfExists(file) ?? '')?.repository ?? '');
      if (repoId) {
        const rel = path.relative(INSTANCE_ROOT, file).split(path.sep).join('/');
        if (currentRepoFiles.has(repoId)) {
          fail(`instruction-intake: repository "${repoId}" has more than one current register (${currentRepoFiles.get(repoId)}, ${rel}); `
             + 'one identity split across files has no unambiguous history');
        } else {
          currentRepoFiles.set(repoId, rel);
        }
        currentRepoIds.add(repoId);
      }
    } catch { /* the ordinary register pass reports the parse failure */ }
  }
  const revisions = gitDirectoryRevisions(INSTANCE_ROOT, 'instruction-intake');
  if (revisions === null) {
    if (intakeFiles.length === 0) {
      warn('instruction-intake: the Instance history is unreadable and no register is present; whether a whole register disappeared is UNVERIFIED, not confirmed');
    }
  } else {
    const historical = new Map();
    for (const rev of revisions) {
      const docs = intakeDocumentsAt(INSTANCE_ROOT, rev);
      if (docs === null) continue;
      for (const item of docs) {
        if (item.error) { fail(`instruction-intake: revision ${rev.slice(0, 7)} of ${item.rel} does not parse: ${item.error}`); continue; }
        const repoId = String(item.doc?.repository ?? '');
        if (repoId && !historical.has(repoId)) historical.set(repoId, item.rel);
      }
    }
    for (const [repoId, oldRel] of historical) {
      if (!currentRepoIds.has(repoId)) {
        fail(`instruction-intake: the complete register for repository "${repoId}" disappeared (it existed as ${oldRel}); `
           + 'append-only protects the register itself as well as records inside it');
      }
    }
  }
}

if (!INSTANCE_ROOT) {
  warn('instruction-intake: no Instance root, not checked');
} else if (intakeFiles.length === 0) {
  // Intake is a process, not a precondition: an absent register means it has
  // not started, which is a fact to state rather than a defect to fail on.
  info('instruction-intake: no register in the Instance; intake has not started');
} else {
  const repoPaths = new Map();
  try {
    for (const r of (yamlParse(readIfExists(path.join(INSTANCE_ROOT, 'inventory', 'repositories.yaml')) ?? '') || {}).repositories || []) {
      if (r?.id) repoPaths.set(String(r.id), r.path);
    }
  } catch { /* inventory problems are reported by inventory-git, not twice */ }

  let recordCount = 0;
  let coveredRepos = 0;
  for (const file of intakeFiles) {
    const rel = path.relative(INSTANCE_ROOT, file).split(path.sep).join('/');
    let doc;
    try { doc = yamlParse(readIfExists(file) ?? ''); }
    catch (e) { fail(`instruction-intake: cannot parse ${rel}: ${e.message}`); continue; }
    if (typeof doc?.$schema !== 'string' || !doc.$schema.trim()) {
      fail(`instruction-intake: ${rel} declares no $schema, so the shape of every record in it was validated by nothing; `
         + 'the checks below assume that shape and would report a defective register as covered');
    }
    const records = Array.isArray(doc?.records) ? doc.records : [];
    recordCount += records.length;
    const repoId = String(doc?.repository ?? '');
    const repoKnown = repoPaths.has(repoId);
    if (!repoKnown) {
      fail(`instruction-intake: ${rel} names repository "${repoId}", which the Instance inventory does not name; `
         + 'this is an unresolvable reference, not an unreachable repository');
    }
    const covered = new Set(records.map((r) => String(r?.artifact ?? '')));

    // preservation, level 1 — against the file's own history. Comparing with
    // HEAD alone was blind to the case that matters most: once the deletion is
    // itself committed, HEAD *is* the truncated file and the difference is
    // gone. The baseline is therefore every revision of this file, which is
    // what "append-only" literally says.
    const currentCounts = countKeys(records.map(recordKey));
    const revisions = gitRevisionsOf(INSTANCE_ROOT, rel);
    if (revisions === null) {
      warn(`instruction-intake: ${rel} — the Instance's history is unreadable from here; preservation is UNVERIFIED, not confirmed`);
    } else if (revisions.length === 0) {
      info(`instruction-intake: ${rel} has never been committed; preservation is UNVERIFIED for this run, not confirmed`);
    } else {
      const everRecorded = new Map();
      let unreadable = 0;
      for (const { rev, path: pathThen } of revisions) {
        const text = gitShowAt(INSTANCE_ROOT, rev, pathThen);
        if (text === null) { unreadable++; continue; }
        try { mergeHighWaterMark(everRecorded, countKeys((yamlParse(text)?.records || []).map(recordKey))); }
        catch (e) { fail(`instruction-intake: revision ${rev.slice(0, 7)} of ${pathThen} does not parse: ${e.message}`); }
      }
      if (unreadable) {
        warn(`instruction-intake: ${unreadable} of ${revisions.length} revision(s) of ${rel} could not be read; preservation is confirmed only against the rest`);
      }
      const gone = lostKeys(everRecorded, currentCounts);
      if (gone.length) {
        fail(`instruction-intake: ${rel} lost ${gone.length} record(s) that earlier revisions of this file carried `
           + `(${gone.slice(0, 3).map(readableKey).join('; ')}); the register is append-only — a decision is superseded by a new record, never edited away`);
      } else {
        info(`instruction-intake: ${rel} — ${revisions.length} revision(s) of the file compared; every record ever committed is still present`);
      }
    }

    // preservation, level 2 — against what has been published. Local history
    // can be rewritten: an amend or a rebase removes the revision that held the
    // lost record, and level 1 then has nothing to compare with. The branch
    // other people read cannot be changed that quietly. Off unless asked for,
    // because it needs the network — and when it is asked for and the network
    // is not there, that is a warning, never a pass.
    if (CHECK_PUBLISHED) {
      const upstream = gitUpstreamOf(INSTANCE_ROOT);
      if (!upstream) {
        warn(`instruction-intake: ${rel} — the Instance has no upstream branch; the published state of the register was NOT compared`);
      } else {
        if (!publishedFetchAttempted) { publishedFetchAttempted = true; publishedFetchOk = gitFetch(INSTANCE_ROOT); }
        if (!publishedFetchOk) {
          warn(`instruction-intake: ${rel} — ${upstream} could not be fetched; the published state is UNVERIFIED, not confirmed`);
        } else {
          const publishedDocs = intakeDocumentsAt(INSTANCE_ROOT, upstream);
          if (publishedDocs === null) {
            warn(`instruction-intake: ${rel} — the intake directory on ${upstream} could not be read; the published state is UNVERIFIED, not confirmed`);
          } else {
            const malformed = publishedDocs.filter((d) => d.error);
            for (const item of malformed) {
              fail(`instruction-intake: the published revision of ${item.rel} does not parse: ${item.error}`);
            }
            const matches = publishedDocs.filter((d) => !d.error && String(d.doc?.repository ?? '') === repoId);
            if (matches.length > 1) {
              fail(`instruction-intake: ${upstream} carries ${matches.length} registers for repository "${repoId}"; the published baseline is ambiguous`);
            } else if (matches.length === 0) {
              info(`instruction-intake: repository "${repoId}" has no register on ${upstream} yet; there is nothing published to lose`);
            } else {
              const publishedCounts = countKeys((matches[0].doc?.records || []).map(recordKey));
              const gone = lostKeys(publishedCounts, currentCounts);
              if (gone.length) {
                fail(`instruction-intake: ${rel} lost ${gone.length} record(s) that are published on ${upstream} `
                   + `(${gone.slice(0, 3).map(readableKey).join('; ')}); rewriting local history does not unpublish a decision`);
              } else {
                info(`instruction-intake: ${rel} — every record published on ${upstream} is still present`);
              }
            }
          }
        }
      }
    }

    // completeness — the artifact list comes from the tree, never from a hand-kept list
    const repoPath = repoPaths.get(repoId);
    const tracked = (() => {
      if (!repoPath) return null;
      try { return execFileSync('git', ['-C', repoPath, 'ls-files'], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).split('\n').filter(Boolean); }
      catch { return null; }
    })();
    // The completeness list is built from tracked files, so a norm that was
    // never added to version control is invisible to it — and a register that
    // covers every tracked file then reports the tree "complete" while the
    // norms the tool actually loads sit outside the claim. Name them, and
    // refuse to count the tree as confirmed: the same guard the Kernel's own
    // untracked files got, applied where the corpus actually lives.
    const untrackedNorms = (() => {
      if (!repoPath) return [];
      try {
        return execFileSync('git', ['-C', repoPath, 'ls-files', '--others', '--exclude-standard'],
          { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] })
          .split('\n').filter(Boolean)
          .filter((m) => INTAKE_MASKS.some((mask) => mask.test(m)));
      } catch { return []; }
    })();

    // "Untracked" and "ignored" are different answers, and the first guard sees
    // only the first: `--exclude-standard` looks away from anything .gitignore
    // covers. A norm inside an ignored directory is therefore MORE invisible
    // than an untracked one, and the guard written to catch invisibility would
    // have passed over it in silence. Asked the other way round — for the
    // artifacts the register already names — the answer is one cheap call.
    //
    // Declared boundary: this direction cannot DISCOVER an ignored norm nobody
    // recorded. Enumerating every ignored path is not affordable in a working
    // repository, so what is claimed here is exactly what is checked — recorded
    // artifacts, not the ignored tree.
    const ignoredNorms = (() => {
      if (!repoPath || tracked === null) return [];
      const trackedSet = new Set(tracked);
      const candidates = [...new Set(records
        .map((r) => String(r?.artifact ?? ''))
        .filter(Boolean)
        .filter((a) => !trackedSet.has(a)))]
        .filter((a) => fs.existsSync(path.join(repoPath, a)));
      if (!candidates.length) return [];
      try {
        return execFileSync('git', ['-C', repoPath, 'check-ignore', '--stdin'],
          { input: candidates.join('\n'), encoding: 'utf8', stdio: ['pipe', 'pipe', 'ignore'] })
          .split('\n').filter(Boolean);
      } catch { return []; }
    })();

    if (!repoKnown) {
      // The unresolvable identifier is already a FAIL above. Calling it merely
      // unreachable here would turn a data defect into an environmental gap.
    } else if (tracked === null) {
      info(`instruction-intake: ${repoId} — repository not reachable from here; completeness is UNVERIFIED, not confirmed`);
    } else {
      // The unit is the region for containers and the file for everything
      // else. Measuring a container by the file would let one topic stand for
      // a text that holds several, which is the defect §3.1 of the protocol
      // describes — and would report "complete" while saying nothing true.
      const coveredRegions = new Set(records
        .filter((r) => r?.region)
        .map((r) => `${String(r?.artifact ?? '')}#${String(r.region)}`));
      const missing = [];
      let regionsCovered = 0;
      let uncoveredContainers = 0;
      for (const f of tracked.filter((m) => INTAKE_MASKS.some((mask) => mask.test(m)))) {
        if (!CONTAINER_MASKS.some((mask) => mask.test(f))) {
          if (!covered.has(f)) missing.push(f);
          continue;
        }
        const body = readIfExists(path.join(repoPath, f));
        if (body === null) {
          info(`instruction-intake: ${repoId}/${f} could not be read; its regions are UNVERIFIED, not confirmed`);
          continue;
        }
        const parsed = instructionRegions(body);
        for (const e of parsed.errors) fail(`instruction-intake: ${repoId}/${f} — ${e}`);
        if (parsed.errors.length) continue;
        // The reverse direction, checked before the branches below so that it
        // also reaches a container declaring no regions at all: a record naming
        // a region the file does not declare. Alone it looks like nothing — but
        // it is what a typo in the id produces, and the real region then shows
        // up as uncovered while the record that meant to cover it judges a
        // section that is not there.
        const declaredIds = new Set(parsed.regions.map((region) => region.id));
        for (const r of records) {
          if (String(r?.artifact ?? '') !== f || !r?.region) continue;
          if (!declaredIds.has(String(r.region))) {
            fail(`instruction-intake: ${repoId}/${f} — a record names region "${r.region}", which the file does not declare`);
          }
        }
        if (parsed.regions.length === 0) {
          // A container whose regions are not declared is itself the finding,
          // and the protocol says how it is recorded: one deferred entry with
          // that as the reason. Checking only that *some* entry exists let any
          // verdict stand in for that one — including a verdict that assigns a
          // single topic to a text holding several, which is the defect §3.1
          // was written against, arriving through the back door.
          // "Current" is the latest by date, not the last line in the file.
          // Ordering by position let the rule be sidestepped by moving two
          // blocks of YAML: a superseded `deferred` placed at the bottom would
          // stand in for a decision taken six months later.
          const fileRecords = records
            .filter((r) => String(r?.artifact ?? '') === f && !r?.region)
            .map((r, i) => ({ r, i }))
            .sort((a, b) => String(a.r?.recorded_at ?? '').localeCompare(String(b.r?.recorded_at ?? '')) || a.i - b.i)
            .map((x) => x.r);
          if (fileRecords.length === 0) { missing.push(f); continue; }
          const current = fileRecords[fileRecords.length - 1];
          if (String(current?.verdict ?? '') !== 'deferred') {
            fail(`instruction-intake: ${repoId}/${f} declares no regions and its current record is "${current?.verdict}"; `
               + 'a container whose boundaries are not declared is itself the finding and is recorded as deferred with that reason (protocol §3.1)');
          } else {
            info(`instruction-intake: ${repoId}/${f} declares no regions and is recorded as deferred; the unit of intake for it is still undeclared`);
          }
          continue;
        }
        for (const region of parsed.regions) {
          const key = `${f}#${region.id}`;
          if (!region.owner) {
            fail(`instruction-intake: ${repoId}/${f} — region "${region.id}" declares no owner; without one there is no answer to whether the next generation may overwrite it`);
          }
          if (region.generated) {
            if (coveredRegions.has(key)) {
              fail(`instruction-intake: ${repoId}/${f} — region "${region.id}" is declared generated and yet carries an intake record; a generated region is not the owner's norm and is not taken in`);
            }
            continue;
          }
          if (coveredRegions.has(key)) regionsCovered++; else missing.push(key);
        }
        if (parsed.uncoveredLines) {
          uncoveredContainers++;
          warn(`instruction-intake: ${repoId}/${f} — ${parsed.uncoveredLines} line(s) lie outside every declared region and belong to no unit of intake; `
             + 'completeness below is claimed over the regions, not over the file');
        }
      }
      if (ignoredNorms.length) {
        warn(`instruction-intake: ${repoId} — ${ignoredNorms.length} recorded norm(s) are excluded from version control by .gitignore (${ignoredNorms.slice(0, 3).join(', ')}); `
           + 'a norm the repository deliberately keeps out of its history has no revision to cite, and this tree is NOT counted as confirmed complete');
      }
      if (untrackedNorms.length) {
        warn(`instruction-intake: ${repoId} — ${untrackedNorms.length} norm(s) match the intake masks but are not under version control (${untrackedNorms.slice(0, 3).join(', ')}); `
           + 'completeness is measured over tracked files, so this tree is NOT counted as confirmed complete');
      }
      if (missing.length) {
        fail(`instruction-intake: ${repoId} — ${missing.length} norm(s) in the tree have no record (${missing.slice(0, 3).join(', ')}); the register is complete or it proves nothing`);
      } else if (uncoveredContainers) {
        // Every declared unit carries a record and part of a container still
        // belongs to no unit. Counting this tree as confirmed complete would
        // put a claim in the summary line that the warning above just denied.
        info(`instruction-intake: ${repoId} — ${regionsCovered} declared region(s) carry a record, but ${uncoveredContainers} container(s) hold text outside every region; this tree is NOT counted as confirmed complete`);
      } else if (!untrackedNorms.length && !ignoredNorms.length) {
        coveredRepos++;
        if (regionsCovered) {
          info(`instruction-intake: ${repoId} — ${regionsCovered} declared region(s) of container files carry a record`);
        }
      }
    }

    // topic existence — not topic correctness, which no gate can see
    if (TOPIC_POOL) {
      const strays = [...new Set(records
        .map((r) => String(r?.topic ?? ''))
        .filter((t) => t && t !== 'unclassified' && !TOPIC_POOL.has(t)))];
      if (strays.length) {
        fail(`instruction-intake: ${rel} names ${strays.length} topic(s) outside the pool (${strays.slice(0, 3).join(', ')}); a new topic is a change to the registry, not to one record`);
      }
    }

    // packaging discipline, readable only where the repository is reachable
    for (const r of records) {
      if (r?.delivery !== 'skill-package') continue;
      const artifact = String(r?.artifact ?? '');
      if (!repoPath || !/\/SKILL\.md$/.test(artifact)) continue;
      const body = readIfExists(path.join(repoPath, artifact));
      if (body === null) {
        info(`instruction-intake: ${artifact} not reachable; its packaging is UNVERIFIED, not confirmed`);
        continue;
      }
      const declaredName = body.match(/^name:\s*(\S+)\s*$/m)?.[1];
      const dirName = path.basename(path.dirname(artifact));
      if (declaredName && declaredName !== dirName) {
        fail(`instruction-intake: ${artifact} is a skill named "${declaredName}" in a directory named "${dirName}"; a norm that cannot be found by its own name makes every reference to it dangling`);
      }
      const front = body.match(/^---\n([\s\S]*?)\n---/)?.[1] ?? '';
      const foreign = ['alwaysApply', 'globs'].filter((k) => new RegExp(`^${k}\\s*:`, 'm').test(front));
      if (foreign.length) {
        fail(`instruction-intake: ${artifact} declares activation twice — ${foreign.join(' and ')} belong to the rule vocabulary, not the package one; which one applies is then decided by the tool, not by the author`);
      }
    }

    // parentage — the reference is qualified (repository, path, revision) and
    // is resolved at the revision it names. The earlier version searched two
    // roots for a bare path and hashed whatever it found today, which conflated
    // two different findings under one verdict:
    //   the reference points at a different text     — a defect of the record;
    //   the parent has moved on since it was taken   — an edition that lags.
    // The first is FAIL, the second is WARN. Treating a legitimate change to
    // the parent as a broken record trains the operator to re-date entries,
    // which is the one repair the protocol forbids.
    //
    // The digest still proves only that the parent is that text; whether the
    // narrowing describes it honestly is not checkable and is stated as such.
    for (const r of records) {
      if (r?.verdict !== 'adopt-edition' || !r?.derived_from) continue;
      const ref = r.derived_from;
      if (typeof ref !== 'object' || Array.isArray(ref) || !ref.repository || !ref.path || !ref.revision) {
        fail(`instruction-intake: ${r.artifact} names a parent that is not a qualified reference (repository, path, revision); a bare path resolves to a different text in every checkout that reads it`);
        continue;
      }
      const refRepoId = String(ref.repository);
      const parentRepo = refRepoId === 'kernel' ? KERNEL_ROOT : repoPaths.get(refRepoId);
      if (parentRepo === undefined) {
        fail(`instruction-intake: ${r.artifact} derives from repository "${refRepoId}", which the Instance inventory does not name; the reference cannot be resolved by anyone, here or elsewhere`);
        continue;
      }
      if (!gitFacts(parentRepo)) {
        info(`instruction-intake: repository "${refRepoId}" is not reachable from here; the parent of ${r.artifact} is UNVERIFIED, not confirmed`);
        continue;
      }
      const shortRev = String(ref.revision).slice(0, 7);
      if (!gitHasRevision(parentRepo, String(ref.revision))) {
        // Not a defect of the record: a shallow or partial clone simply does
        // not carry that commit. Failing here would be a red run on a correct
        // entry, which is the mistake this whole check was rewritten to stop.
        info(`instruction-intake: revision ${shortRev} of ${refRepoId} is not present in this checkout; the parent of ${r.artifact} is UNVERIFIED, not confirmed`);
        continue;
      }
      const atRevision = gitShowAt(parentRepo, String(ref.revision), String(ref.path));
      if (atRevision === null) {
        fail(`instruction-intake: ${r.artifact} derives from "${ref.path}" at ${shortRev} in ${refRepoId}, and that revision holds no such file; the reference points at nothing`);
        continue;
      }
      const takenDigest = crypto.createHash('sha256').update(atRevision, 'utf8').digest('hex');
      if (takenDigest !== String(r.derived_from_digest)) {
        fail(`instruction-intake: ${r.artifact} records parent digest ${String(r.derived_from_digest).slice(0, 12)} but "${ref.path}" at ${shortRev} hashes to ${takenDigest.slice(0, 12)}; `
           + 'the reference names a different text, which no re-dating can fix');
        continue;
      }
      const atHead = gitShowAt(parentRepo, 'HEAD', String(ref.path));
      if (atHead === null) {
        warn(`instruction-intake: ${r.artifact} — its parent "${ref.path}" no longer exists at HEAD of ${refRepoId}; the edition stays anchored to ${shortRev}, but the text it narrows has been removed or moved`);
      } else if (crypto.createHash('sha256').update(atHead, 'utf8').digest('hex') !== takenDigest) {
        warn(`instruction-intake: ${r.artifact} — the edition is behind its parent: "${ref.path}" has changed in ${refRepoId} since ${shortRev}. `
           + 'This is not a defective record: re-take the edition against the current parent when the difference matters');
      }
    }
  }
  ok(`instruction-intake: ${recordCount} record(s) across ${intakeFiles.length} register file(s); ${coveredRepos} repository tree(s) confirmed complete`);
}

// ---------------------------------------------------------------------------
// agent-instruction norms living in the Kernel: §7 applied to the Kernel itself
// ---------------------------------------------------------------------------
// The standard used to require four fields of Kernel documents and nothing
// checked whether its own documents carried them — the exact shape of defect
// the whole system exists to catch, found in the first package written under
// its rules.
//
// Which Kernel documents are agent instruction norms is declared, not derived:
// a document says so by carrying `delivery`. Deriving it from the genre would
// sweep in every prescriptive document in the tree, most of which no tool ever
// loads into an agent's context, and a claim that broad would be false in the
// same way the old one was. The cost of declaring is stated in §7.2 of the
// standard: a norm that never declares itself is invisible here, and the count
// below is printed so that the size of that gap is visible on every run.
const NORM_DELIVERY = new Set(['kernel-doc', 'skill-package', 'cursor-rule', 'agents-md-section']);
const NORM_ACTIVATION = new Set(['always', 'path-glob', 'task-class', 'explicit']);
const NORM_FIELDS = ['topic', 'profile', 'delivery', 'activation'];
const PRESCRIPTIVE_TYPES = new Set(['standard', 'contract', 'protocol']);

let normFailures = 0;
const nfail = (m) => { normFailures++; fail(m); };
let declaredNorms = 0;
let undeclaredPrescriptive = 0;
let undeclaredOther = 0;
for (const rel of relKernelFiles.filter((r) => r.endsWith('.md') && carriesOwnFrontMatter(r))) {
  const text = readIfExists(path.join(KERNEL_ROOT, rel));
  if (text === null || !text.startsWith('---')) continue;
  const end = text.indexOf('\n---', 3);
  const fm = end === -1 ? '' : text.slice(4, end);
  const present = NORM_FIELDS.filter((f) => new RegExp(`^${f}:[^\\S\\r\\n]*\\S`, 'm').test(fm));
  // `derived_from` is a field of this standard too, so declaring it alone is
  // also a half-declaration; without this the whole check below could be
  // stepped over by declaring parentage and nothing else.
  const declaresParent = /^derived_from:/m.test(fm);
  if (present.length === 0 && !declaresParent) {
    const type = fm.match(/^document_type:\s*(\S+)/m)?.[1];
    if (PRESCRIPTIVE_TYPES.has(type)) undeclaredPrescriptive++; else undeclaredOther++;
    continue;
  }
  declaredNorms++;
  const absent = NORM_FIELDS.filter((f) => !present.includes(f));
  if (absent.length) {
    nfail(`agent-instruction-identity: ${rel} declares ${present.join(', ')} but not ${absent.join(', ')}; `
        + '§7 is four independent answers, and a half-declared norm is exactly the drift this standard was written against');
  }
  const delivery = fm.match(/^delivery:\s*(\S+)/m)?.[1];
  if (delivery && !NORM_DELIVERY.has(delivery)) {
    nfail(`agent-instruction-identity: ${rel} declares delivery "${delivery}", which is not in the pool of §5`);
  }
  const activation = fm.match(/^activation:\s*(\S+)/m)?.[1];
  if (activation && !NORM_ACTIVATION.has(activation)) {
    nfail(`agent-instruction-identity: ${rel} declares activation "${activation}", which is not in the pool of §5`);
  }
  const topic = fm.match(/^topic:\s*(\S+)/m)?.[1];
  if (topic === 'unclassified') {
    if (!/^unclassified_reason:[^\S\r\n]*\S/m.test(fm)) {
      nfail(`agent-instruction-identity: ${rel} carries topic "unclassified" with no unclassified_reason; the state is legal, an unexplained one is not`);
    }
  } else if (topic && TOPIC_POOL && !TOPIC_POOL.has(topic)) {
    nfail(`agent-instruction-identity: ${rel} names topic "${topic}", which is not in the pool; a new topic is a change to the registry, not to one document`);
  }
  // derived_from in Front Matter obeys the same rule as in the register: a
  // qualified reference, or it is not a reference at all. A scalar value sits
  // on the same line as the key; a mapping does not, which is the whole
  // difference being tested.
  if (/^derived_from:[^\S\r\n]*\S/m.test(fm)) {
    nfail(`agent-instruction-identity: ${rel} records derived_from as a scalar; the parent reference is a mapping of repository, path and revision`);
  } else if (declaresParent && !/^narrowing:/m.test(fm)) {
    // §8 claims this is checked always; before this line it was checked
    // nowhere for a Kernel document, and only by the register's schema for a
    // record. An edition with no list of narrowings is indistinguishable from
    // an independent text on the same subject.
    nfail(`agent-instruction-identity: ${rel} declares derived_from with no narrowing list; an edition that does not say what it removed is not distinguishable from a text written independently`);
  }
}
if (normFailures === 0) {
  ok(`agent-instruction-identity: ${declaredNorms} Kernel document(s) declare themselves agent instruction norms and carry all four fields of §7`);
}
// Both numbers, because a document is free to answer "what does it assert"
// with a genre outside the prescriptive three and still be loaded into an
// agent's context. Printing only the prescriptive count would describe the
// gap as smaller than it is.
info(`agent-instruction-identity: ${undeclaredPrescriptive} prescriptive and ${undeclaredOther} other Kernel document(s) declare no delivery, so §7 does not reach them; `
   + 'a norm that does not declare itself is invisible to this check, and that is its declared boundary, not its result');

// ---------------------------------------------------------------------------
// the same watchdog, on the Instance side
// ---------------------------------------------------------------------------
// The Kernel-only version missed the case that produced it: a report written
// as the evidence for a claim sat untracked in the Instance, so the claim
// still rested on a text that no revision carried. Instance artifacts are
// evidence; an unrecorded one proves nothing, however carefully it is written.
if (INSTANCE_ROOT) {
  const untrackedInstance = gitUntrackedFiles(INSTANCE_ROOT);
  if (untrackedInstance === null) {
    warn('instance-provenance: the Instance is not enumerable by Git from here; whether its artifacts are recorded is UNVERIFIED, not confirmed');
  } else if (untrackedInstance.length) {
    const rels = untrackedInstance.map((f) => path.relative(INSTANCE_ROOT, f).split(path.sep).join('/'));
    warn(`instance-provenance: ${rels.length} file(s) in the Instance are untracked `
       + `(${rels.slice(0, 5).join(', ')}${rels.length > 5 ? ', …' : ''}); an artifact no revision carries cannot be cited as evidence for anything`);
  }
}

// ---------------------------------------------------------------------------
// git provenance of the control directories
// ---------------------------------------------------------------------------
if (fs.existsSync(path.join(KERNEL_ROOT, '.git'))) ok('git-provenance: Kernel is under Git');
else fail('git-provenance: Kernel is not under Git; no revision can be cited for it');
if (!INSTANCE_ROOT) warn('git-provenance: Instance root not supplied, its VCS state is unknown');
else if (fs.existsSync(path.join(INSTANCE_ROOT, '.git'))) ok('git-provenance: Instance is under Git');
else if (path.resolve(INSTANCE_ROOT).startsWith(KERNEL_ROOT + path.sep)) {
  // A synthetic Instance committed inside the Kernel (test/instance-fixture)
  // shares the Kernel's history. Real Instances never live here.
  ok('git-provenance: Instance is inside the Kernel repository (fixture) and shares its history');
} else warn('git-provenance: Instance is not under Git; its recorded facts have no revision to cite');

console.log(report.join('\n'));
console.log('\n---');
console.log(`${failures} failing, ${warnings} warnings, ${report.length - failures - warnings} informational/ok lines`);
process.exit(failures > 0 ? 1 : 0);
