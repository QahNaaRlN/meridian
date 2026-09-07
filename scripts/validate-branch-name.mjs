#!/usr/bin/env node
// Branch-name syntax check for the merge request into Kernel's protected
// lines (standards/workspace/version-control-flow.md §3.1, §13.2).
//
// What this proves and what it does not:
//   - it checks the SYNTAX of a branch name against the one canonical
//     expression of version-control-flow.md §13 — the permanent lines
//     `main` / `dev` plus the closed temporary-branch template;
//   - it does NOT and CANNOT prove that a `<slug>` is made of English
//     words, is not a transliteration of Russian, is meaningful rather
//     than generic, or that the branch type matches the actual work.
//     Those stay substantive review checks (§3, §3.1, §2.7).
//
// Guarantee boundary. There is no platform rule in the owner's public
// GitHub repository that forbids CREATING or PUSHING a branch by a regular
// expression. Creating or pushing a branch with a disallowed name is
// therefore technically possible. This script is wired into the already
// mandatory `Kernel validate (synthetic instance)` check (§13.1) for
// `pull_request` events only, so a merge request whose source branch is
// syntactically invalid cannot be merged into the protected `main` or
// `dev`. That is the whole mechanical guarantee — no more.
//
// Portable Node.js, no third-party dependencies.
//
// Library use:
//   import { isValidBranchName } from './scripts/validate-branch-name.mjs';
//
// CLI use:
//   node scripts/validate-branch-name.mjs <branch-name>
//     exit 0  — name is syntactically allowed
//     exit 1  — name is syntactically disallowed
//     exit 2  — no branch name given on the command line

import path from 'node:path';
import { fileURLToPath } from 'node:url';

// The one canonical expression of version-control-flow.md §13 (set B),
// transcribed verbatim: permanent lines `main` / `dev`, the closed set of
// temporary-branch types with a kebab-case slug, and `release/<semver>`
// with an exact MAJOR.MINOR.PATCH.
export const BRANCH_NAME_PATTERN =
  /^(?:main|dev|(?:(?:feature|bugfix|hotfix|promotion|chore|docs|refactor|test|ci|build)\/[a-z0-9]+(?:-[a-z0-9]+)*|release\/[0-9]+\.[0-9]+\.[0-9]+))$/;

/**
 * Pure syntax check. Returns true when `name` matches the canonical
 * expression of version-control-flow.md §13, false otherwise (including
 * for a non-string argument). It makes no claim about the meaning of the
 * name — see the guarantee boundary at the top of this file.
 *
 * @param {unknown} name
 * @returns {boolean}
 */
export function isValidBranchName(name) {
  return typeof name === 'string' && BRANCH_NAME_PATTERN.test(name);
}

function main(argv) {
  const name = argv[0];
  if (name === undefined) {
    process.stderr.write(
      'usage: node scripts/validate-branch-name.mjs <branch-name>\n' +
      'error: no branch name given\n',
    );
    return 2;
  }
  if (isValidBranchName(name)) {
    process.stdout.write(`ok: "${name}" is a syntactically allowed branch name\n`);
    return 0;
  }
  process.stderr.write(
    `error: "${name}" is not a syntactically allowed branch name\n` +
    'allowed: "main", "dev", "<type>/<kebab-case-slug>" for type in ' +
    'feature|bugfix|hotfix|promotion|chore|docs|refactor|test|ci|build, ' +
    'or "release/MAJOR.MINOR.PATCH" ' +
    '(version-control-flow.md §3.1, §13.2).\n' +
    'This check is syntax only: an English-meaning and no-transliteration ' +
    'check stays a review responsibility.\n',
  );
  return 1;
}

if (process.argv[1] && path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url))) {
  process.exit(main(process.argv.slice(2)));
}
