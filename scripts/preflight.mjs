#!/usr/bin/env node
// Workspace preflight: verifies that an agent session is actually inside a
// correctly wired Meridian workspace BEFORE any work starts. The failure mode
// this closes is silent: a session opened against stale roots (an old
// directory layout, a moved kernel, an unset MERIDIAN_INSTANCE) will happily
// read outdated rules and never notice. Discovery must fail loudly, not
// degrade quietly.
//
// Usage: node scripts/preflight.mjs
//   MERIDIAN_KERNEL   defaults to this repository
//   MERIDIAN_INSTANCE no default; absence is reported as a red preflight,
//                     because product work without an Instance means the
//                     session is not wired to its data.
// Exit 0 = wired correctly; exit 1 = do not start work from this session.

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const KERNEL = process.env.MERIDIAN_KERNEL || path.resolve(__dirname, '..');
const INSTANCE = process.env.MERIDIAN_INSTANCE || null;

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

if (!INSTANCE) {
  say(false, 'MERIDIAN_INSTANCE is not set — the session is not wired to any Instance; product work must not start');
} else {
  const productYaml = path.join(INSTANCE, 'product.yaml');
  const instOk = fs.existsSync(productYaml);
  say(instOk, `instance root: ${INSTANCE}${instOk ? '' : ' — no product.yaml; this is not a Meridian instance'}`);
  if (instOk) {
    say(fs.existsSync(path.join(INSTANCE, '.git')) || path.resolve(INSTANCE).startsWith(path.resolve(KERNEL) + path.sep),
      'instance is under Git (or is the in-kernel fixture)');
  }
}

console.log('---');
console.log(bad === 0
  ? 'preflight: workspace wired correctly'
  : `preflight: ${bad} problem(s) — fix the wiring before starting work`);
process.exit(bad === 0 ? 0 : 1);
