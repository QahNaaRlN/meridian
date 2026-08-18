#!/usr/bin/env node
// Read-only validator for this agent workspace. Modifies nothing.
//
// Design rule this file follows itself: a check that cannot actually be
// performed must report UNVERIFIED (warn/fail), never OK. A green run must
// mean "verified", not "not looked at". See
// docs/agent-standards/workspace/kernel-boundary.md.
//
// Checks
//   1. kernel-purity     — every declared Kernel file (Markdown, YAML *and*
//                          this script) is free of the current product's
//                          literals and of personal home-directory paths.
//   2. duplicate-fm      — no scanned Markdown file (Kernel *or* .agent) ends
//                          with an orphaned second Front-Matter block.
//   3. front-matter      — .agent artifacts carry leading Front Matter (WARN).
//   4. path-placement    — active/ vs done/archive/ vs status (WARN).
//   5. links             — relative Markdown links resolve.
//   6. schema            — every registry declaring `$schema` is parsed and
//                          validated against it, in-gate.
//   7. sha-provenance    — installed skill matches its pinned SHA-256.
//   8. ext-dependencies  — declared external dependencies are resolvable, or
//                          are explicitly and legibly unresolved.
//   9. inventory-git     — recorded revision/ref/dirty state is compared with
//                          the repository's actual current state.
//  10. git-provenance    — whether the control directories are under VCS.
//
// Deliberate limits, stated rather than hidden:
//   - The YAML reader and the JSON Schema validator below implement documented
//     subsets. Both THROW on any construct or keyword they do not implement,
//     so an unsupported input turns the gate red instead of silently passing.
//   - The Kernel file list is a hand-maintained mirror of kernel-boundary.md.
//     This script does not parse that document; update both together.
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

function listFiles(dir, exts) {
  let out = [];
  let entries;
  try { entries = fs.readdirSync(dir, { withFileTypes: true }); } catch { return out; }
  for (const e of entries) {
    if (EXCLUDED_DIRS.has(e.name)) continue;
    const full = path.join(dir, e.name);
    if (e.isDirectory()) out = out.concat(listFiles(full, exts));
    else if (exts.some((ext) => e.name.endsWith(ext))) out.push(full);
  }
  return out;
}

// ---------------------------------------------------------------------------
// YAML: documented subset. Supports block mappings, block sequences, plain and
// quoted scalars, `|` and `>` block scalars, comments, empty values as null.
// Throws UnsupportedYaml on flow style, anchors, aliases, tags, multi-document
// streams and tab indentation.
// ---------------------------------------------------------------------------
class UnsupportedYaml extends Error {}

function stripComment(line) {
  let inS = false, inD = false;
  for (let i = 0; i < line.length; i++) {
    const c = line[i];
    if (c === "'" && !inD) inS = !inS;
    else if (c === '"' && !inS) inD = !inD;
    else if (c === '#' && !inS && !inD && (i === 0 || /\s/.test(line[i - 1]))) return line.slice(0, i);
  }
  return line;
}

// Flow-style collections: [], {}, [a, b], {k: v}, nested. Quoted strings are
// honoured; ':' only terminates a token when a key is being read inside {},
// so plain scalars containing ':' (URLs) survive inside sequences.
function parseFlow(src) {
  let i = 0;
  const ws = () => { while (i < src.length && /\s/.test(src[i])) i++; };
  const token = (isKey) => {
    ws();
    if (src[i] === "'" || src[i] === '"') {
      const q = src[i++]; let out = '';
      while (i < src.length && src[i] !== q) out += src[i++];
      if (src[i] !== q) throw new UnsupportedYaml('unterminated quoted string in flow collection');
      i++; return out;
    }
    const stop = isKey ? ',]}:' : ',]}';
    const start = i;
    while (i < src.length && !stop.includes(src[i])) i++;
    const raw = src.slice(start, i).trim();
    return isKey ? raw : parseScalar(raw);
  };
  const value = () => {
    ws();
    if (src[i] === '[') {
      i++; const arr = []; ws();
      if (src[i] === ']') { i++; return arr; }
      for (;;) {
        arr.push(value()); ws();
        if (src[i] === ',') { i++; continue; }
        if (src[i] === ']') { i++; return arr; }
        throw new UnsupportedYaml('malformed flow sequence');
      }
    }
    if (src[i] === '{') {
      i++; const obj = {}; ws();
      if (src[i] === '}') { i++; return obj; }
      for (;;) {
        const k = token(true); ws();
        if (src[i] !== ':') throw new UnsupportedYaml('malformed flow mapping');
        i++; obj[k] = value(); ws();
        if (src[i] === ',') { i++; continue; }
        if (src[i] === '}') { i++; return obj; }
        throw new UnsupportedYaml('malformed flow mapping');
      }
    }
    return token(false);
  };
  const out = value();
  ws();
  if (i !== src.length) throw new UnsupportedYaml('trailing content after flow collection');
  return out;
}

function parseScalar(raw) {
  const s = raw.trim();
  if (s === '' || s === '~' || s === 'null') return null;
  if (s.startsWith('{') || s.startsWith('[')) return parseFlow(s);
  if (s.startsWith('&') || s.startsWith('*')) throw new UnsupportedYaml('anchor or alias');
  if (s.startsWith('!')) throw new UnsupportedYaml('explicit tag');
  if (/^'(.*)'$/s.test(s)) return s.slice(1, -1).replace(/''/g, "'");
  if (/^"(.*)"$/s.test(s)) return s.slice(1, -1).replace(/\\"/g, '"');
  if (s === 'true') return true;
  if (s === 'false') return false;
  if (/^-?\d+$/.test(s)) return Number.parseInt(s, 10);
  if (/^-?\d+\.\d+$/.test(s)) return Number.parseFloat(s);
  return s;
}

function yamlParse(text) {
  const rawLines = text.split(/\r?\n/);
  const lines = [];
  rawLines.forEach((raw, i) => {
    if (/^\t/.test(raw)) throw new UnsupportedYaml(`tab indentation at line ${i + 1}`);
    if (i > 0 && /^---\s*$/.test(raw)) throw new UnsupportedYaml(`multi-document stream at line ${i + 1}`);
    const stripped = stripComment(raw);
    if (stripped.trim() === '') return;
    lines.push({ indent: stripped.match(/^ */)[0].length, text: stripped.trim(), n: i + 1 });
  });

  let pos = 0;

  function readBlockScalar(parentIndent, style) {
    const parts = [];
    while (pos < lines.length && lines[pos].indent > parentIndent) parts.push(lines[pos++].text);
    return style === '|' ? parts.join('\n') : parts.join(' ');
  }

  function parseNode(indent) {
    if (pos >= lines.length || lines[pos].indent < indent) return null;
    if (lines[pos].text.startsWith('- ') || lines[pos].text === '-') {
      const arr = [];
      while (pos < lines.length && lines[pos].indent === indent &&
             (lines[pos].text.startsWith('- ') || lines[pos].text === '-')) {
        const line = lines[pos];
        const rest = line.text === '-' ? '' : line.text.slice(2).trim();
        if (rest === '') { pos++; arr.push(parseNode(indent + 2)); continue; }
        const m = rest.match(/^([A-Za-z0-9_$.-]+):(?:\s+(.*))?$/);
        if (m) {
          // sequence item that is a mapping; its first key sits on this line
          const itemIndent = indent + 2;
          const obj = {};
          const key = m[1];
          const inlineVal = (m[2] ?? '').trim();
          pos++;
          if (inlineVal === '|' || inlineVal === '>') obj[key] = readBlockScalar(itemIndent, inlineVal);
          else if (inlineVal === '') {
            const child = (pos < lines.length && lines[pos].indent > itemIndent) ? parseNode(lines[pos].indent) : null;
            obj[key] = child;
          } else obj[key] = parseScalar(inlineVal);
          const more = parseNode(itemIndent);
          if (more && typeof more === 'object' && !Array.isArray(more)) Object.assign(obj, more);
          arr.push(obj);
        } else { pos++; arr.push(parseScalar(rest)); }
      }
      return arr;
    }

    const obj = {};
    while (pos < lines.length && lines[pos].indent === indent) {
      const line = lines[pos];
      if (line.text.startsWith('- ')) break;
      const m = line.text.match(/^([A-Za-z0-9_$.-]+):(?:\s+(.*))?$/);
      if (!m) throw new UnsupportedYaml(`unrecognized construct at line ${line.n}: ${line.text}`);
      const key = m[1];
      const val = (m[2] ?? '').trim();
      pos++;
      if (val === '|' || val === '>') { obj[key] = readBlockScalar(indent, val); continue; }
      if (val === '') {
        if (pos < lines.length && lines[pos].indent > indent) obj[key] = parseNode(lines[pos].indent);
        else if (pos < lines.length && lines[pos].indent === indent &&
                 (lines[pos].text.startsWith('- ') || lines[pos].text === '-')) obj[key] = parseNode(indent);
        else obj[key] = null;
        continue;
      }
      obj[key] = parseScalar(val);
    }
    return obj;
  }

  const result = parseNode(lines.length ? lines[0].indent : 0);
  return result ?? {};
}

// ---------------------------------------------------------------------------
// JSON Schema: documented subset. Throws UnsupportedSchema on any keyword it
// does not implement, so an unvalidatable schema fails the gate.
// ---------------------------------------------------------------------------
class UnsupportedSchema extends Error {}

const SUPPORTED_KEYWORDS = new Set([
  '$schema', '$id', 'id', 'title', 'description', 'examples', 'default', 'comment', '$comment',
  'type', 'properties', 'required', 'items', 'additionalProperties', 'enum', 'const',
  'pattern', 'minLength', 'maxLength', 'minItems', 'maxItems', 'uniqueItems',
  'minimum', 'maximum', 'exclusiveMinimum', 'exclusiveMaximum',
  'anyOf', 'oneOf', 'allOf', 'not', '$ref', 'definitions', '$defs', 'format',
  'if', 'then', 'else', 'propertyNames',
]);

function assertSupported(schema, where) {
  if (typeof schema !== 'object' || schema === null) return;
  for (const k of Object.keys(schema)) {
    if (!SUPPORTED_KEYWORDS.has(k)) {
      throw new UnsupportedSchema(`keyword "${k}" at ${where} is not implemented by this validator`);
    }
  }
}

function resolveRef(ref, root) {
  if (!ref.startsWith('#/')) throw new UnsupportedSchema(`external $ref "${ref}"`);
  let node = root;
  for (const seg of ref.slice(2).split('/')) {
    node = node?.[seg.replace(/~1/g, '/').replace(/~0/g, '~')];
    if (node === undefined) throw new UnsupportedSchema(`unresolvable $ref "${ref}"`);
  }
  return node;
}

function typeOf(v) {
  if (v === null) return 'null';
  if (Array.isArray(v)) return 'array';
  if (Number.isInteger(v)) return 'integer';
  return typeof v;
}

function validate(data, schema, root, ptr, errs) {
  assertSupported(schema, ptr || '/');
  if (schema.$ref) return validate(data, resolveRef(schema.$ref, root), root, ptr, errs);

  if (schema.type) {
    const types = Array.isArray(schema.type) ? schema.type : [schema.type];
    const actual = typeOf(data);
    const okType = types.some((t) => t === actual || (t === 'number' && actual === 'integer'));
    if (!okType) { errs.push(`${ptr || '/'}: expected ${types.join('|')}, got ${actual}`); return; }
  }
  if (schema.enum && !schema.enum.some((e) => JSON.stringify(e) === JSON.stringify(data))) {
    errs.push(`${ptr || '/'}: value ${JSON.stringify(data)} not in enum`);
  }
  if ('const' in schema && JSON.stringify(schema.const) !== JSON.stringify(data)) {
    errs.push(`${ptr || '/'}: value must equal ${JSON.stringify(schema.const)}`);
  }
  if (typeof data === 'string') {
    if (schema.pattern && !new RegExp(schema.pattern).test(data)) errs.push(`${ptr}: does not match /${schema.pattern}/`);
    if (schema.minLength != null && data.length < schema.minLength) errs.push(`${ptr}: shorter than ${schema.minLength}`);
    if (schema.maxLength != null && data.length > schema.maxLength) errs.push(`${ptr}: longer than ${schema.maxLength}`);
  }
  if (typeof data === 'number') {
    if (schema.minimum != null && data < schema.minimum) errs.push(`${ptr}: below minimum`);
    if (schema.maximum != null && data > schema.maximum) errs.push(`${ptr}: above maximum`);
    if (schema.exclusiveMinimum != null && data <= schema.exclusiveMinimum) errs.push(`${ptr}: at/below exclusiveMinimum`);
    if (schema.exclusiveMaximum != null && data >= schema.exclusiveMaximum) errs.push(`${ptr}: at/above exclusiveMaximum`);
  }
  if (Array.isArray(data)) {
    if (schema.minItems != null && data.length < schema.minItems) errs.push(`${ptr}: fewer than ${schema.minItems} items`);
    if (schema.maxItems != null && data.length > schema.maxItems) errs.push(`${ptr}: more than ${schema.maxItems} items`);
    if (schema.uniqueItems) {
      const seen = new Set(data.map((d) => JSON.stringify(d)));
      if (seen.size !== data.length) errs.push(`${ptr}: items are not unique`);
    }
    if (schema.items) data.forEach((d, i) => validate(d, schema.items, root, `${ptr}/${i}`, errs));
  }
  if (data && typeof data === 'object' && !Array.isArray(data)) {
    for (const req of schema.required || []) {
      if (!(req in data) || data[req] === undefined) errs.push(`${ptr || ''}/${req}: required property missing`);
    }
    if (schema.propertyNames) {
      for (const k of Object.keys(data)) validate(k, schema.propertyNames, root, `${ptr}/${k}<name>`, errs);
    }
    for (const [k, v] of Object.entries(data)) {
      const sub = schema.properties?.[k];
      if (sub) validate(v, sub, root, `${ptr}/${k}`, errs);
      else if (schema.additionalProperties === false) errs.push(`${ptr}/${k}: additional property not allowed`);
      else if (typeof schema.additionalProperties === 'object') validate(v, schema.additionalProperties, root, `${ptr}/${k}`, errs);
    }
  }
  for (const kw of ['allOf']) if (schema[kw]) schema[kw].forEach((s, i) => validate(data, s, root, `${ptr}/allOf/${i}`, errs));
  for (const kw of ['anyOf', 'oneOf']) {
    if (!schema[kw]) continue;
    const passing = schema[kw].filter((s) => { const e = []; validate(data, s, root, ptr, e); return e.length === 0; }).length;
    if (kw === 'anyOf' && passing === 0) errs.push(`${ptr || '/'}: matches none of anyOf`);
    if (kw === 'oneOf' && passing !== 1) errs.push(`${ptr || '/'}: matches ${passing} of oneOf (expected exactly 1)`);
  }
  if (schema.not) { const e = []; validate(data, schema.not, root, ptr, e); if (e.length === 0) errs.push(`${ptr || '/'}: must not match "not" schema`); }
  if (schema.if) {
    const probe = [];
    validate(data, schema.if, root, ptr, probe);
    const branch = probe.length === 0 ? schema.then : schema.else;
    if (branch) validate(data, branch, root, ptr, errs);
  }
}

// ---------------------------------------------------------------------------
// Kernel file set (mirror of kernel-boundary.md)
// ---------------------------------------------------------------------------
const KERNEL_FILES = [
  ...listFiles(path.join(KERNEL_ROOT, 'standards'), ['.md']),
  ...listFiles(path.join(KERNEL_ROOT, 'workflows'), ['.md']),
  ...listFiles(path.join(KERNEL_ROOT, 'verification'), ['.md']),
  // skills carry their pins alongside them; both are Kernel
  ...listFiles(path.join(KERNEL_ROOT, 'skills'), ['.md', '.yaml']),
  // registry rules and their JSON Schemas are Kernel; the data they describe
  // is Instance. Schemas are scanned too: an example value inside a schema is
  // just as much a leak as a sentence in a document.
  ...listFiles(path.join(KERNEL_ROOT, 'registries'), ['.md', '.json']),
  path.join(KERNEL_ROOT, 'README.md'),
  path.join(KERNEL_ROOT, 'CHANGELOG.md'),
  path.join(KERNEL_ROOT, 'COMPATIBILITY.md'),
  // the validator is declared Kernel by kernel-boundary.md, so it must be
  // subject to the same purity rule it enforces on everything else
  path.join(KERNEL_ROOT, 'scripts', 'kernel-validate.mjs'),
];
const kernelFiles = [...new Set(KERNEL_FILES)].filter((f) => fs.existsSync(f));

// forbidden literals are derived from the Instance product record
const productYamlPath = INSTANCE_ROOT ? path.join(INSTANCE_ROOT, 'product.yaml') : null;
const productRaw = productYamlPath ? readIfExists(productYamlPath) : null;
let forbiddenLiterals = [];
let productDoc = null;
if (productRaw) {
  try {
    productDoc = yamlParse(productRaw);
    forbiddenLiterals = [
      productDoc?.product?.name,
      productDoc?.canonical_wiki?.space_key,
      productDoc?.canonical_wiki?.base_url,
      productDoc?.tooling?.task_tracker?.base_url,
    ].filter(Boolean);
    ok(`product record parsed; ${forbiddenLiterals.length} kernel-purity literals derived`);
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

for (const file of kernelFiles) {
  const text = readIfExists(file);
  if (text === null) continue;
  for (const lit of forbiddenLiterals) {
    const esc = String(lit).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const re = new RegExp(/^https?:\/\//i.test(lit) ? esc : `\\b${esc}\\b`, 'i');
    if (re.test(text)) fail(`kernel-purity: product literal "${lit}" found in ${file}`);
  }
  const personal = text.match(PERSONAL_PATH_RE);
  if (personal) fail(`kernel-purity: personal home path "${personal[0]}" found in ${file}`);
}
if (failures === 0) ok(`kernel-purity: ${kernelFiles.length} kernel files clean (Markdown, YAML and this script)`);

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
    if (file.startsWith(KERNEL_ROOT + path.sep) && !resolved.startsWith(KERNEL_ROOT + path.sep)) {
      fail(`link: ${file} -> "${m[1]}" points outside the Kernel; reference Instance material as a $MERIDIAN_INSTANCE path, do not link to it`);
      continue;
    }
    if (!fs.existsSync(resolved)) fail(`link: ${file} -> "${m[1]}" does not resolve`);
  }
}
ok(`link check ran over ${allMd.length} files`);

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
  try { validate(doc, schema, schema, '', errs); }
  catch (e) { fail(`schema: ${file} could not be validated: ${e.message}`); continue; }
  if (errs.length) errs.slice(0, 10).forEach((er) => fail(`schema: ${file} ${er}`));
  else validated++;
}
if (attempted === 0) warn('schema: no registry declared a $schema; nothing was validated');
else ok(`schema: ${validated}/${attempted} registries passed in-gate validation against their declared JSON Schema`);

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
        warn(`inventory-git: ${r.id} — repository at ${r.path} is not reachable from here; recorded revision is UNVERIFIED, not confirmed`);
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
