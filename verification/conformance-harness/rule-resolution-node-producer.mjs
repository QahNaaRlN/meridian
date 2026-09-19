#!/usr/bin/env node

import fs from 'node:fs';
import { resolveRules } from '../../scripts/rule-resolver.mjs';

const corpus = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));

function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.keys(value).sort().map((key) => [key, canonical(value[key])]));
  }
  return value;
}

for (const testCase of corpus.cases) {
  let observation;
  try {
    const { work_item, repository_inventory, applicability, ...optional } = testCase.request;
    const output = resolveRules(work_item, {
      repository_inventory,
      applicability_records: applicability.records,
      intake_registers: [],
      protocol_routes: [],
      verification_routes: [],
      norm_texts: {},
      prior_state: {},
      ...optional,
    });
    observation = `accepted:${JSON.stringify(canonical(output))}`;
  } catch {
    observation = 'rejected';
  }
  console.log(`WARN  resolver/${testCase.name}: ${observation}`);
}
