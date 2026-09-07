// ---------------------------------------------------------------------------
// JSON Schema: documented subset. Throws UnsupportedSchema on any keyword it
// does not implement, so an unvalidatable schema fails the gate.
// ---------------------------------------------------------------------------
// Extracted verbatim from scripts/kernel-validate.mjs so that the validator and
// scripts/rule-resolver.mjs check documents against JSON Schema through one
// engine, not two copies. The supported keyword set, the format checks and
// every throw are unchanged.
// ---------------------------------------------------------------------------
export class UnsupportedSchema extends Error {}

export const SUPPORTED_KEYWORDS = new Set([
  '$schema', '$id', 'id', 'title', 'description', 'examples', 'default', 'comment', '$comment',
  'type', 'properties', 'required', 'items', 'additionalProperties', 'enum', 'const',
  'pattern', 'minLength', 'maxLength', 'minItems', 'maxItems', 'uniqueItems',
  'minimum', 'maximum', 'exclusiveMinimum', 'exclusiveMaximum',
  'anyOf', 'oneOf', 'allOf', 'not', '$ref', 'definitions', '$defs', 'format',
  'if', 'then', 'else', 'propertyNames', 'contains',
]);

export function assertSupported(schema, where) {
  if (typeof schema !== 'object' || schema === null) return;
  for (const k of Object.keys(schema)) {
    if (!SUPPORTED_KEYWORDS.has(k)) {
      throw new UnsupportedSchema(`keyword "${k}" at ${where} is not implemented by this validator`);
    }
  }
}

// `format` is enforced, not merely tolerated: only the formats implemented
// here are accepted, and any other format value fails the schema up front.
// A format that was silently ignored would let a green run claim more than
// was checked.
export const FORMAT_CHECKS = {
  'date': (s) => /^\d{4}-\d{2}-\d{2}$/.test(s) && !Number.isNaN(Date.parse(s)),
  'date-time': (s) => /^\d{4}-\d{2}-\d{2}[Tt]\d{2}:\d{2}:\d{2}(\.\d+)?([Zz]|[+-]\d{2}:\d{2})$/.test(s) && !Number.isNaN(Date.parse(s)),
  'uri': (s) => { try { new URL(s); return true; } catch { return false; } },
};

// Walk the whole schema tree once, before any data is validated. Per-node
// assertion during validation only reaches the branches the data happens to
// visit; an unsupported keyword in an optional, never-taken branch would
// otherwise pass silently.
const SCHEMA_MAP_KEYWORDS = ['properties', 'definitions', '$defs'];
const SCHEMA_CHILD_KEYWORDS = ['items', 'additionalProperties', 'not', 'if', 'then', 'else', 'propertyNames', 'contains'];
const SCHEMA_LIST_KEYWORDS = ['anyOf', 'oneOf', 'allOf'];
export function assertSupportedDeep(schema, where) {
  if (typeof schema !== 'object' || schema === null || Array.isArray(schema)) return;
  assertSupported(schema, where);
  if (schema.format != null && !FORMAT_CHECKS[schema.format]) {
    throw new UnsupportedSchema(`format "${schema.format}" at ${where} is not implemented by this validator`);
  }
  for (const k of SCHEMA_MAP_KEYWORDS) {
    if (schema[k] && typeof schema[k] === 'object') {
      for (const [name, sub] of Object.entries(schema[k])) assertSupportedDeep(sub, `${where}/${k}/${name}`);
    }
  }
  for (const k of SCHEMA_CHILD_KEYWORDS) {
    if (typeof schema[k] === 'object' && schema[k] !== null) assertSupportedDeep(schema[k], `${where}/${k}`);
  }
  for (const k of SCHEMA_LIST_KEYWORDS) {
    if (Array.isArray(schema[k])) schema[k].forEach((sub, i) => assertSupportedDeep(sub, `${where}/${k}/${i}`));
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

export function validate(data, schema, root, ptr, errs) {
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
    if (schema.format != null) {
      const check = FORMAT_CHECKS[schema.format];
      if (!check) throw new UnsupportedSchema(`format "${schema.format}" at ${ptr || '/'} is not implemented by this validator`);
      if (!check(data)) errs.push(`${ptr}: does not satisfy format "${schema.format}"`);
    }
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
    if (schema.contains) {
      const hit = data.some((d) => { const e = []; validate(d, schema.contains, root, ptr, e); return e.length === 0; });
      if (!hit) errs.push(`${ptr || '/'}: no item matches "contains"`);
    }
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
