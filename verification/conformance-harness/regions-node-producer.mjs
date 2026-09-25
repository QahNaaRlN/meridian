#!/usr/bin/env node
// Verification-only producer for the cross-language conformance harness:
// calls the REAL exported scripts/lib/regions.mjs functions directly — no
// second, test-only copy of blankFencedBlocks/markedRegion/instructionRegions
// logic. Its Rust counterpart (meridian-app/examples/regions_producer.rs)
// calls the real compiled meridian_app::source_format::regions functions on
// the same embedded corpus (verification/conformance-harness/fixtures/
// regions-corpus.json) and both are compared byte-for-byte by the
// conformance harness (real-node-rust-regions-adapters fixture).

import fs from 'node:fs';
import { blankFencedBlocks, markedRegion, instructionRegions } from '../../scripts/lib/regions.mjs';

const corpus = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'));

function canonical(value) {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === 'object') {
    return Object.fromEntries(Object.keys(value).sort().map((key) => [key, canonical(value[key])]));
  }
  return value;
}

// Named, intentional boundary (documented on the Rust side too, in
// regions_producer.rs): the Node reference blanks one space per UTF-16 code
// unit, while the Rust port blanks one space per UTF-8 *byte* of the
// original character — required there so every later byte offset used to
// slice the original raw text stays valid, which UTF-16-width blanking
// cannot guarantee for a multi-byte character. Nothing downstream of either
// language's own blanked buffer ever inspects the *width* of a blanked run
// (only whether a line is blanked at all, and the untouched offsets around
// it), so this producer collapses a run of blanked spaces to one fixed
// placeholder before comparison — but ONLY on a line that was actually fully
// blanked by `blankFencedBlocks` (a line whose text differs from the
// corresponding line of `raw`). A line `blankFencedBlocks` left untouched is
// compared byte-for-byte, including any run of ordinary spaces it contains —
// collapsing spaces on every line, blanked or not, would have hidden a real
// mismatch in untouched text behind the same placeholder both languages
// happen to agree on.
function normalizeBlankedRuns(raw, text) {
  const rawLines = raw.split('\n');
  return text
    .split('\n')
    .map((line, i) => (line === rawLines[i] ? line : line.replace(/ +/g, '·BLANKED·')))
    .join('\n');
}

for (const testCase of corpus.blank_fenced_blocks) {
  const result = blankFencedBlocks(testCase.raw);
  const observation = canonical({ text: normalizeBlankedRuns(testCase.raw, result.text), error: result.error ?? null });
  console.log(`WARN  blank_fenced_blocks/${testCase.name}: ${JSON.stringify(observation)}`);
}

for (const testCase of corpus.marked_region) {
  const result = markedRegion(testCase.raw, testCase.region_name);
  const observation = result.error
    ? canonical({ error: result.error })
    : canonical({ text: result.text, attrs: result.attrs });
  console.log(`WARN  marked_region/${testCase.name}: ${JSON.stringify(observation)}`);
}

for (const testCase of corpus.instruction_regions) {
  const result = instructionRegions(testCase.raw);
  const observation = canonical({
    errors: result.errors,
    uncoveredLines: result.uncoveredLines,
    regions: result.regions.map((r) => canonical({
      id: r.id,
      owner: r.owner,
      generated: r.generated,
      text: r.text,
      sourceText: r.sourceText,
    })),
  });
  console.log(`WARN  instruction_regions/${testCase.name}: ${JSON.stringify(observation)}`);
}
