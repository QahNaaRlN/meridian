#!/usr/bin/env node
// Workspace preflight: verifies that an agent session is actually inside a
// correctly wired Meridian workspace BEFORE any work starts. The failure mode
// this closes is silent: a session opened against a stale root (an old
// directory layout, a moved kernel) will happily read outdated rules and
// never notice. Discovery must fail loudly, not degrade quietly.
//
// Kernel development is self-sufficient by design (AGENTS.md §1/§2): the
// Instance repository is frozen for read as a migration source and is no
// longer the active center of Meridian's own development, so a bare run
// checks only the Kernel and must not fail merely because
// MERIDIAN_INSTANCE is unset.
//
// --require-instance is the explicit transitional mode for the (shrinking)
// set of callers that still need a wired Instance — e.g. reading the frozen
// source during the migration itself. It reproduces the previous strict
// contract: MERIDIAN_INSTANCE absent, or present but not a real Instance
// root, is a red preflight.
//
// Usage: node scripts/preflight.mjs [--require-instance]
//   MERIDIAN_KERNEL   defaults to this repository
//   MERIDIAN_INSTANCE only consulted when --require-instance is given
// Exit 0 = wired correctly; exit 1 = do not start work from this session.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const KERNEL = process.env.MERIDIAN_KERNEL || path.resolve(__dirname, '..');
const INSTANCE = process.env.MERIDIAN_INSTANCE || null;

const KNOWN_FLAGS = new Set(['--require-instance']);
const args = process.argv.slice(2);
const unknown = args.filter((a) => !KNOWN_FLAGS.has(a));
if (unknown.length) {
  console.error(`preflight: unknown argument(s): ${unknown.join(', ')}`);
  console.error('Usage: node scripts/preflight.mjs [--require-instance]');
  process.exit(1);
}
const requireInstance = args.includes('--require-instance');

let bad = 0;
const say = (okFlag, msg) => { if (!okFlag) bad++; console.log(`${okFlag ? 'OK  ' : 'RED '} ${msg}`); };

const kernelExists = fs.existsSync(path.join(KERNEL, 'VERSION'));
say(kernelExists, `kernel root: ${KERNEL}${kernelExists ? '' : ' — no VERSION file; this is not a Meridian kernel'}`);
if (kernelExists) {
  const version = fs.readFileSync(path.join(KERNEL, 'VERSION'), 'utf8').trim();
  say(true, `kernel version: ${version}`);
  say(fs.existsSync(path.join(KERNEL, '.git')), 'kernel is under Git');
  say(fs.existsSync(path.join(KERNEL, 'scripts', 'kernel-validate.mjs')), 'validator present');
}

if (requireInstance) {
  if (!INSTANCE) {
    say(false, '--require-instance was given but MERIDIAN_INSTANCE is not set — the session is not wired to any Instance; this transitional work must not start');
  } else {
    const productYaml = path.join(INSTANCE, 'product.yaml');
    const instOk = fs.existsSync(productYaml);
    say(instOk, `instance root: ${INSTANCE}${instOk ? '' : ' — no product.yaml; this is not a Meridian instance'}`);
    if (instOk) {
      say(fs.existsSync(path.join(INSTANCE, '.git')) || path.resolve(INSTANCE).startsWith(path.resolve(KERNEL) + path.sep),
        'instance is under Git (or is the in-kernel fixture)');
    }
  }
} else {
  // Kernel-only mode never checks Instance, so its value is not evidence of
  // anything here — printing it would leak a product-specific absolute path
  // into a run that did not verify it. Report only whether it is set.
  console.log(`INFO ${INSTANCE ? 'MERIDIAN_INSTANCE is set but not required or checked by this run' : 'MERIDIAN_INSTANCE not required for Kernel-only work'}; pass --require-instance to verify a wired Instance`);
}

console.log('---');
console.log(bad === 0
  ? 'preflight: workspace wired correctly'
  : `preflight: ${bad} problem(s) — fix the wiring before starting work`);
process.exit(bad === 0 ? 0 : 1);
