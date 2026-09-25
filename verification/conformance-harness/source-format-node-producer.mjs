#!/usr/bin/env node

import fs from 'node:fs';
import { yamlParse } from '../../scripts/lib/yaml.mjs';
import { assertSupportedDeep, validate } from '../../scripts/lib/json-schema.mjs';

const corpus = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));

function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.keys(value).sort().map((key) => [key, canonical(value[key])]));
  }
  return value;
}

for (const testCase of corpus.yaml) {
  let observation;
  try {
    observation = `accepted:${JSON.stringify(canonical(yamlParse(testCase.source)))}`;
  } catch {
    observation = 'rejected';
  }
  console.log(`WARN  yaml/${testCase.name}: ${observation}`);
}

for (const testCase of corpus.schema) {
  let observation;
  try {
    assertSupportedDeep(testCase.schema, '/');
    const errors = [];
    validate(testCase.data, testCase.schema, testCase.schema, '', errors);
    errors.sort();
    observation = `accepted:${JSON.stringify(errors)}`;
  } catch {
    observation = 'rejected';
  }
  console.log(`WARN  schema/${testCase.name}: ${observation}`);
}
