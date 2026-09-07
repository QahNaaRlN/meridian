#!/usr/bin/env node
// Regression suite for scripts/validate-branch-name.mjs.
//
// Proves the two things the script claims: (1) the pure function accepts
// exactly the names the canonical expression of version-control-flow.md §13
// accepts and rejects everything else; (2) the CLI is exit-code honest —
// 0 for an allowed name, non-zero for a disallowed name, non-zero for a
// missing argument.
//
// It also makes the MECHANISM BOUNDARY explicit: a syntactically correct
// transliteration or a generic slug passes the regular expression, yet is
// still forbidden by the substantive norm (§3, §3.1) and must be rejected
// at review. The regex cannot prove English meaning; this suite states so
// with a live assertion rather than a comment.
//
// Usage: node test/branch-name-validation.test.mjs

import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

import { isValidBranchName } from '../scripts/validate-branch-name.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SCRIPT = path.join(__dirname, '..', 'scripts', 'validate-branch-name.mjs');

let passed = 0;
let failed = 0;
const failures = [];

function check(name, cond, detail) {
  if (cond) { passed++; console.log(`ok    ${name}`); }
  else { failed++; failures.push(`${name}: ${detail}`); console.log(`FAIL  ${name}`); }
}

function accepts(name) {
  check(`accepts ${JSON.stringify(name)}`, isValidBranchName(name) === true,
    'expected the syntax check to accept it');
}

function rejects(name, why) {
  check(`rejects ${JSON.stringify(name)} (${why})`, isValidBranchName(name) === false,
    'expected the syntax check to reject it');
}

// --- permanent lines -------------------------------------------------------
accepts('main');
accepts('dev');

// --- every allowed temporary-branch type with a valid slug ---------------
for (const type of ['feature', 'bugfix', 'hotfix', 'promotion', 'chore', 'docs', 'refactor', 'test', 'ci', 'build']) {
  accepts(`${type}/branch-name-validation`);
}
accepts('feature/a');            // single-character slug segment
accepts('feature/a1b2-c3');      // digits are allowed inside a segment
accepts('test/flaky-link-check'); // §3.1: generic type is fine when the slug is not

// --- release/MAJOR.MINOR.PATCH ------------------------------------------------
accepts('release/0.5.0');
accepts('release/1.2.3');
accepts('release/12.34.567');

// --- uppercase, underscores, double and edge dashes ----------------------
rejects('feature/Branch-Name', 'uppercase letter in the slug');
rejects('feature/Add-Name-Check', 'uppercase letters in a multi-word slug');
rejects('feature/branch_name', 'underscore in the slug');
rejects('feature/branch--name', 'double dash in the slug');
rejects('feature/-branch-name', 'leading dash in the slug');
rejects('feature/branch-name-', 'trailing dash in the slug');

// --- missing or unknown type ----------------------------------------------
rejects('branch-name-validation', 'no type prefix at all');
rejects('feat/branch-name', 'unknown type "feat"');
rejects('feature-branch-name', 'no slash between type and slug');
rejects('feature/', 'type prefix but an empty slug');
rejects('/branch-name', 'empty type');

// --- an extra segment via "/" -------------------------------------------------
rejects('feature/branch/name', 'a second "/" segment');
rejects('release/1.2.3/rc1', 'a second "/" segment after a version');

// --- Cyrillic and spaces ----------------------------------------------------
rejects('feature/ветка', 'Cyrillic letters in the slug');
rejects('feature/branch name', 'a space in the slug');
rejects('feature/branch-name ', 'a trailing space');
rejects(' feature/branch-name', 'a leading space');

// --- a malformed version form ---------------------------------------------
rejects('release/1.2', 'version is missing the PATCH component');
rejects('release/1.2.3.4', 'version has a fourth component');
rejects('release/v1.2.3', 'version carries a "v" prefix');
rejects('release/1.2.x', 'version PATCH is not a number');
rejects('release/1.2.3-rc1', 'a pre-release suffix is not part of the closed template');
accepts('release/01.02.03');   // shape-only: the expression checks MAJOR.MINOR.PATCH shape, not SemVer numeric rules

// --- non-string input is not a name -------------------------------------------
check('rejects a non-string argument', isValidBranchName(undefined) === false && isValidBranchName(42) === false,
  'a non-string must never be reported as a valid name');

// --- CLI: exit-code honesty --------------------------------------------------
function cli(args) {
  const r = spawnSync(process.execPath, [SCRIPT, ...args], { encoding: 'utf8' });
  return { code: r.status, out: r.stdout ?? '', err: r.stderr ?? '' };
}

{
  const r = cli(['feature/branch-name-validation']);
  check('cli — allowed name exits 0', r.code === 0, `exit ${r.code}`);
}
{
  const r = cli(['release/0.5.0']);
  check('cli — allowed release name exits 0', r.code === 0, `exit ${r.code}`);
}
{
  const r = cli(['feature/Branch_Name']);
  check('cli — disallowed name exits exactly 1', r.code === 1, `exit ${r.code}`);
  check('cli — disallowed name explains itself', /not a syntactically allowed/.test(r.err), r.err);
}
{
  const r = cli([]);
  check('cli — no argument exits exactly 2', r.code === 2, `exit ${r.code}`);
  check('cli — no argument prints usage', /usage:/.test(r.err), r.err);
}
{
  const r = cli(['feature/branch-name-validation']);
  // The success message must claim only SYNTACTIC allowedness — not "valid",
  // not "correct", not anything that would imply the meaning was checked.
  check('cli — success message claims only syntactic allowedness',
    r.code === 0 && /^ok: .+ is a syntactically allowed branch name\s*$/m.test(r.out),
    `exit ${r.code}: ${JSON.stringify(r.out)}`);
  check('cli — success message does not overclaim beyond syntax',
    !/\b(valid|correct|meaningful|English)\b/i.test(r.out),
    `success stdout must not imply more than a syntax check: ${JSON.stringify(r.out)}`);
}

// --- MECHANISM BOUNDARY: the regex admits names the norm still forbids ----
// A transliteration of Russian and a bare generic slug are syntactically
// well-formed. version-control-flow.md §3, §3.1 forbid both, but a regular
// expression cannot prove "English words" or "meaningful". These MUST pass
// the syntax check here and MUST be rejected by a human at review.
{
  const transliteration = 'feature/dobavit-proverku-imeni';   // "add name check", transliterated
  const genericSlug = 'feature/stuff';
  check('boundary — a transliteration passes the SYNTAX check (regex cannot catch it)',
    isValidBranchName(transliteration) === true,
    'if this fails the regex changed; the boundary claim in the docs must change too');
  check('boundary — a generic slug passes the SYNTAX check (regex cannot catch it)',
    isValidBranchName(genericSlug) === true,
    'if this fails the regex changed; the boundary claim in the docs must change too');
  check('boundary — the CLI also exits 0 for the transliteration, proving the gap is real',
    cli([transliteration]).code === 0,
    'the transliteration must reach review, not be silently blocked here');
  console.log('note  the two names above are syntactically valid and substantively FORBIDDEN');
  console.log('note  (§3, §3.1): English meaning and no-transliteration stay review checks.');
}

console.log('---');
console.log(`${passed} passed, ${failed} failed`);
if (failures.length) console.log('\n' + failures.join('\n'));
process.exit(failed > 0 ? 1 : 0);
