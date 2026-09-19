#!/usr/bin/env node
// Conformance harness — an independent verdict-comparison mechanism.
//
// This module is the ONE implementation of: running two verdict producers,
// capturing their exit code/stdout/stderr separately, normalizing what each
// side observably emitted, and comparing the two normalized results. It is
// not a command of the future Meridian CLI, it is not part of the Node
// reference implementation, and it does not know about Verdict/Diagnostic
// types, a resolver, or any Meridian-domain concept — it only understands
// "a process ran, here is what it printed and how it exited".
//
// test/conformance-harness.test.mjs, hooks/pre-push (via that test file) and
// .github/workflows/gate.yml (via the same test file) all exercise this
// exact module — there is no second, slightly different comparison
// algorithm living in any of those three places.
//
// Two distinct aggregation policies sit on top of the one runCase()
// algorithm, and they must never be confused:
//   - runConformanceCheck() — the PUBLIC one, used by main() below (this
//     file's CLI). Exit 0 only when every requested comparison actually
//     came back "conformant". A real "divergent" or "harness_error" is
//     never turned into a zero exit code, no matter what any fixture
//     declared it expected.
//   - runCorpus() — a SELF-CHECK used only by test/conformance-harness.test.mjs
//     to prove the mechanism above actually reaches "divergent"/
//     "harness_error" on the controlled corpus's adversarial cases, by
//     comparing the actual status against each case's own declared
//     `expected_status`. This is the only place `expected_status` is read;
//     the public CLI never reads it and never runs this function.
//

// The corpus retains synthetic pairs that prove the mechanism itself and,
// from package 4, also runs the real Node.js and Rust source-format producers
// over one shared adversarial corpus through these unchanged functions.
//
// Producer spec shape (both `left` and `right` of a case use it):
//   { command: string, args: string[], cwd: string, env: Record<string,string> }
// `command`/`args` are passed to child_process.spawnSync with shell:false —
// no shell string is ever assembled, so there is nothing here for a shell to
// re-interpret. `env` is used verbatim as the child's entire environment
// (no ambient process.env leaks through), which is also why the harness
// needs no network access: nothing it runs is handed a route to one.
//
// Diagnostic line convention recognized by normalizeDiagnostics: a line of
// the form "FAIL  <message>" or "WARN  <message>" (level, exactly two
// spaces, then a non-empty message) — the same convention already used by
// scripts/kernel-validate.mjs's `fail`/`warn` helpers. A line that starts
// with the token FAIL or WARN but does not match that exact form is kept as
// `unparseable`, never silently dropped and never silently treated as a
// pass — callers of runCase() turn a non-empty `unparseable` list into an
// explicit harness_error instead of guessing.

import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';

const STRICT_DIAGNOSTIC_LINE = /^(FAIL|WARN) {2}(.+)$/;
const LOOKS_LIKE_DIAGNOSTIC_LINE = /^(FAIL|WARN)/;

/**
 * Run one side of a comparison. Never throws: every failure to obtain a
 * trustworthy exit code (spawn error, signal, invalid config, no numeric
 * status) comes back as an explicit `{ ok: false, reason, detail }` object —
 * a harness-level refusal, not something a caller could mistake for a
 * producer's own successful run.
 */
export function runProducer(spec) {
  if (!spec || typeof spec !== 'object' || Array.isArray(spec)) {
    return { ok: false, reason: 'invalid_spec', detail: 'producer spec must be an object' };
  }
  const { command, args, cwd, env } = spec;
  if (typeof command !== 'string' || command.length === 0) {
    return { ok: false, reason: 'invalid_spec', detail: 'producer spec requires a non-empty string "command"' };
  }
  if (!Array.isArray(args) || !args.every((a) => typeof a === 'string')) {
    return { ok: false, reason: 'invalid_spec', detail: 'producer spec requires "args" as an array of strings' };
  }
  if (typeof cwd !== 'string' || cwd.length === 0) {
    return { ok: false, reason: 'invalid_spec', detail: 'producer spec requires a non-empty string "cwd"' };
  }
  if (env === null || env === undefined || typeof env !== 'object' || Array.isArray(env)) {
    return { ok: false, reason: 'invalid_spec', detail: 'producer spec requires an "env" object (use {} for none)' };
  }
  for (const [key, value] of Object.entries(env)) {
    if (typeof value !== 'string') {
      return { ok: false, reason: 'invalid_spec', detail: `producer env value for "${key}" must be a string` };
    }
  }

  let result;
  try {
    result = spawnSync(command, args, { cwd, env, encoding: 'utf8', shell: false });
  } catch (error) {
    return { ok: false, reason: 'spawn_threw', detail: error.message };
  }

  if (result.error) {
    return { ok: false, reason: 'spawn_error', detail: result.error.message };
  }
  if (result.signal !== null && result.signal !== undefined) {
    return { ok: false, reason: 'signal', detail: result.signal };
  }
  if (typeof result.status !== 'number') {
    return { ok: false, reason: 'no_exit_code', detail: 'process reported no numeric exit code' };
  }

  return {
    ok: true,
    exitCode: result.status,
    stdout: typeof result.stdout === 'string' ? result.stdout : '',
    stderr: typeof result.stderr === 'string' ? result.stderr : '',
  };
}

function toLines(text) {
  return text.replace(/\r\n/g, '\n').split('\n');
}

/**
 * Turn raw stdout/stderr into a normalized verdict observation: a sorted
 * (order-independent) list of FAIL messages, a sorted list of WARN messages
 * — both keeping duplicates, never deduplicated — and a list of lines that
 * looked like a diagnostic but did not parse. Both streams are scanned the
 * same way: this package makes no claim about which stream a future real
 * producer puts diagnostics on, so neither stream is preferred or ignored.
 */
export function normalizeDiagnostics(stdout, stderr) {
  const fail = [];
  const warn = [];
  const unparseable = [];
  for (const stream of [stdout ?? '', stderr ?? '']) {
    for (const rawLine of toLines(stream)) {
      if (rawLine.length === 0) continue;
      const strict = rawLine.match(STRICT_DIAGNOSTIC_LINE);
      if (strict) {
        const [, level, message] = strict;
        (level === 'FAIL' ? fail : warn).push(message);
        continue;
      }
      if (LOOKS_LIKE_DIAGNOSTIC_LINE.test(rawLine)) {
        unparseable.push(rawLine);
      }
    }
  }
  fail.sort();
  warn.sort();
  return { fail, warn, unparseable };
}

function countMessages(list) {
  const counts = new Map();
  for (const message of list) counts.set(message, (counts.get(message) ?? 0) + 1);
  return counts;
}

function byMessage(a, b) {
  return a.message < b.message ? -1 : a.message > b.message ? 1 : 0;
}

/**
 * Multiset diff between two message lists. A message is `missing` when the
 * left side carries more occurrences than the right (present, absent, or a
 * duplicate lost on the right); `added` when the right side carries more.
 * Because this counts occurrences rather than deduplicating, a diagnostic
 * repeated on one side only shows up here, and identical diagnostics in a
 * different order never do.
 */
function multisetDiff(left, right) {
  const leftCount = countMessages(left);
  const rightCount = countMessages(right);
  const keys = new Set([...leftCount.keys(), ...rightCount.keys()]);
  const missing = [];
  const added = [];
  for (const key of keys) {
    const l = leftCount.get(key) ?? 0;
    const r = rightCount.get(key) ?? 0;
    if (l > r) missing.push({ message: key, left_count: l, right_count: r });
    if (r > l) added.push({ message: key, left_count: l, right_count: r });
  }
  missing.sort(byMessage);
  added.sort(byMessage);
  return { missing, added };
}

/**
 * Compare two already-normalized verdict observations:
 *   { exitCode: number, fail: string[], warn: string[] }
 * A diagnostic that changed level (same text, FAIL on one side, WARN on the
 * other) is not collapsed into a third "changed" bucket: it surfaces as a
 * `missing` entry in one level's diff and an `added` entry in the other's —
 * both are reported, so the level change is visible from either side
 * without a fuzzy same-text matching pass this module deliberately does not
 * perform (fuzzy matching is exactly the kind of normalization that could
 * hide a real difference in message text).
 */
export function compareVerdicts(left, right) {
  const failDiff = multisetDiff(left.fail, right.fail);
  const warnDiff = multisetDiff(left.warn, right.warn);
  const exitMatch = left.exitCode === right.exitCode;
  const match =
    exitMatch &&
    failDiff.missing.length === 0 &&
    failDiff.added.length === 0 &&
    warnDiff.missing.length === 0 &&
    warnDiff.added.length === 0;
  return {
    exit_code: { left: left.exitCode, right: right.exitCode, match: exitMatch },
    fail: { left: left.fail, right: right.fail, missing: failDiff.missing, added: failDiff.added },
    warn: { left: left.warn, right: right.warn, missing: warnDiff.missing, added: warnDiff.added },
    match,
  };
}

/**
 * Run one full case: both producers, both normalizations, then the
 * comparison. Returns exactly one of three statuses:
 *   - "harness_error" — the mechanism itself could not reach a trustworthy
 *     verdict for at least one side (spawn failure, invalid config, or a
 *     diagnostic-looking line that would not parse). This is never reported
 *     as a match.
 *   - "conformant"     — both sides produced the same exit code and the same
 *     FAIL/WARN multisets.
 *   - "divergent"       — both sides ran, but disagree on exit code and/or
 *     FAIL/WARN content.
 */
export function runCase(caseDef) {
  const leftRun = runProducer(caseDef.left);
  const rightRun = runProducer(caseDef.right);

  if (!leftRun.ok || !rightRun.ok) {
    return { status: 'harness_error', reason: 'producer_failed', left_run: leftRun, right_run: rightRun };
  }

  const leftNorm = normalizeDiagnostics(leftRun.stdout, leftRun.stderr);
  const rightNorm = normalizeDiagnostics(rightRun.stdout, rightRun.stderr);

  if (leftNorm.unparseable.length > 0 || rightNorm.unparseable.length > 0) {
    return {
      status: 'harness_error',
      reason: 'unparseable_diagnostic_line',
      left_unparseable: leftNorm.unparseable,
      right_unparseable: rightNorm.unparseable,
    };
  }

  const comparison = compareVerdicts(
    { exitCode: leftRun.exitCode, fail: leftNorm.fail, warn: leftNorm.warn },
    { exitCode: rightRun.exitCode, fail: rightNorm.fail, warn: rightNorm.warn },
  );

  return { status: comparison.match ? 'conformant' : 'divergent', comparison };
}

// Fixture-only convenience: fixtures cannot hardcode an absolute node path
// (it varies by machine/CI and would be exactly the kind of personal/local
// literal Kernel files must not carry), so a producer spec may use the
// literal command "$NODE" to mean "this same node binary running the
// harness", and a relative "cwd" resolved against the fixtures file's own
// directory. This substitution is intentionally kept out of runProducer/
// runCase/compareVerdicts above: those stay generic for a future real
// producer spec that will never use either convention.
function resolveFixtureSpec(spec, baseDir) {
  if (!spec || typeof spec !== 'object') return spec;
  const resolved = { ...spec };
  if (resolved.command === '$NODE') {
    resolved.command = process.execPath;
  }
  if (typeof resolved.cwd === 'string') {
    resolved.cwd = path.resolve(baseDir, resolved.cwd);
  }
  return resolved;
}

// Shared fixture loading for both functions below: parse the file, require a
// non-empty "cases" array, require each case to carry a non-empty "name",
// and resolve each side's producer spec (the "$NODE"/relative-cwd
// convenience above). This is loading/parsing boilerplate, not comparison
// logic — runCase/compareVerdicts/normalizeDiagnostics remain the single
// algorithm both functions below build on.
function loadFixtureCases(fixturePath) {
  const absoluteFixturePath = path.resolve(fixturePath);
  const raw = fs.readFileSync(absoluteFixturePath, 'utf8');
  const fixtures = JSON.parse(raw);
  if (!fixtures || !Array.isArray(fixtures.cases) || fixtures.cases.length === 0) {
    throw new Error(`fixtures file must contain a non-empty "cases" array: ${absoluteFixturePath}`);
  }
  const baseDir = path.dirname(absoluteFixturePath);
  return fixtures.cases.map((caseDef, index) => {
    if (!caseDef || typeof caseDef.name !== 'string' || caseDef.name.length === 0) {
      throw new Error(`fixture case at index ${index} requires a non-empty string "name"`);
    }
    return {
      name: caseDef.name,
      expectedStatus: typeof caseDef.expected_status === 'string' ? caseDef.expected_status : undefined,
      spec: {
        left: resolveFixtureSpec(caseDef.left, baseDir),
        right: resolveFixtureSpec(caseDef.right, baseDir),
      },
    };
  });
}

/**
 * Self-check mode: does the mechanism's actual status for each fixture case
 * match that fixture's own declared `expected_status`? This is how the
 * controlled corpus proves the harness actually distinguishes a matching
 * pair from a diverging one — a case declaring `expected_status: "divergent"`
 * is supposed to come back divergent, and `ok` records whether it did.
 *
 * `expected_status` is consulted ONLY here. This function is used by
 * test/conformance-harness.test.mjs directly (as a library call) and is
 * deliberately NOT what the public CLI below runs — a fixture author's
 * expectation must never be able to turn a real divergence into a
 * process-level success (see runConformanceCheck).
 */
export function runCorpus(fixturePath) {
  return loadFixtureCases(fixturePath).map(({ name, expectedStatus, spec }) => {
    if (typeof expectedStatus !== 'string') {
      throw new Error(`fixture case "${name}" requires a string "expected_status" for self-check`);
    }
    const actual = runCase(spec);
    return {
      name,
      expected_status: expectedStatus,
      actual_status: actual.status,
      ok: actual.status === expectedStatus,
      detail: actual,
    };
  });
}

/**
 * Public conformance check: the actual status of every requested comparison,
 * and nothing else — `expected_status` is never read here. `conformant` is
 * true only when every case's actual status is "conformant"; any actual
 * "divergent" or "harness_error" makes it false, regardless of what any
 * fixture happened to declare it expected. This is what main() below uses to
 * decide the CLI's exit code, and it is the function a future direct
 * single-pair caller (package 4+) would use the same way.
 */
export function runConformanceCheck(fixturePath) {
  const results = loadFixtureCases(fixturePath).map(({ name, spec }) => {
    const actual = runCase(spec);
    return { name, status: actual.status, detail: actual };
  });
  const conformant = results.length > 0 && results.every((r) => r.status === 'conformant');
  return { results, conformant };
}

function main() {
  const fixtureArg = process.argv[2];
  if (!fixtureArg) {
    console.error('usage: conformance-harness.mjs <fixtures.json>');
    process.exit(2);
  }
  const { results, conformant } = runConformanceCheck(fixtureArg);
  let conformantCount = 0;
  for (const result of results) {
    if (result.status === 'conformant') {
      conformantCount++;
      console.log(`CONFORMANT     ${result.name}`);
    } else if (result.status === 'divergent') {
      console.log(`DIVERGENT      ${result.name}`);
      console.log(JSON.stringify(result.detail, null, 2));
    } else {
      console.log(`HARNESS_ERROR  ${result.name}`);
      console.log(JSON.stringify(result.detail, null, 2));
    }
  }
  console.log(`\n${conformantCount}/${results.length} comparison(s) conformant`);
  // Exit 0 only when every requested comparison actually came back
  // conformant. A real divergence or a harness_error is never turned into a
  // zero exit code by this function — see runConformanceCheck above.
  process.exit(conformant ? 0 : 1);
}

const isMainModule = process.argv[1] && path.resolve(process.argv[1]) === new URL(import.meta.url).pathname;
if (isMainModule) {
  main();
}
