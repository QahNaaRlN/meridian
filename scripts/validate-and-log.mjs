#!/usr/bin/env node
// Thin, transparent wrapper around kernel-validate.mjs: runs it unchanged,
// prints its output unchanged, exits with its exact exit code — and, only
// when a real (non-fixture) Instance is configured, appends one machine-
// readable record of the run to that Instance's metrics log.
//
// This exists so that "is the gate getting healthier or noisier over time"
// is a question answerable from a file, not from memory. It is intentionally
// dumb: it does not interpret trends, does not fail the build on a bad
// trend, and writes nothing when there is nothing meaningful to correlate
// (no Instance, or the in-repository CI fixture).
//
// Usage: node scripts/validate-and-log.mjs   (same env as kernel-validate.mjs)
// Drop-in replacement for `node scripts/kernel-validate.mjs` in hooks/CI.

import fs from 'node:fs';
import path from 'node:path';
import { spawnSync, execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const KERNEL_ROOT = process.env.MERIDIAN_KERNEL || path.resolve(__dirname, '..');
const INSTANCE_ROOT = process.env.MERIDIAN_INSTANCE || null;
const VALIDATOR = path.join(__dirname, 'kernel-validate.mjs');

function gitRev(root) {
  try {
    return execFileSync('git', ['-C', root, 'rev-parse', 'HEAD'], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
  } catch { return null; }
}

const result = spawnSync(process.execPath, [VALIDATOR], {
  env: process.env,
  encoding: 'utf8',
});
const out = result.stdout ?? '';
process.stdout.write(out);
if (result.stderr) process.stderr.write(result.stderr);
const exitCode = result.status ?? 1;

// Classify the Instance the same way kernel-validate.mjs does: unset, the
// in-repository fixture, or a real external Instance. Only a real Instance
// gets a log entry — fixture and no-Instance runs are noise, not signal.
let mode = 'none';
let resolvedInstance = null;
if (INSTANCE_ROOT) {
  resolvedInstance = path.resolve(INSTANCE_ROOT);
  const resolvedKernel = path.resolve(KERNEL_ROOT);
  mode = resolvedInstance.startsWith(resolvedKernel + path.sep) ? 'fixture' : 'real';
}

if (mode !== 'real') {
  console.log(`\nmetrics: not logged (mode=${mode}; only a real Instance is logged)`);
  process.exit(exitCode);
}

const summary = out.match(/^(\d+) failing, (\d+) warnings?, (\d+) informational\/ok lines/m);
const lines = out.split(/\r?\n/);
const failMessages = lines.filter((l) => l.startsWith('FAIL')).map((l) => l.replace(/^FAIL\s+/, ''));
const warnMessages = lines.filter((l) => l.startsWith('WARN')).map((l) => l.replace(/^WARN\s+/, ''));

const record = {
  ts: new Date().toISOString(),
  kernel_version: (() => { try { return fs.readFileSync(path.join(KERNEL_ROOT, 'VERSION'), 'utf8').trim(); } catch { return null; } })(),
  kernel_revision: gitRev(KERNEL_ROOT),
  instance_revision: gitRev(resolvedInstance),
  exit_code: exitCode,
  failing: summary ? Number(summary[1]) : null,
  warnings: summary ? Number(summary[2]) : null,
  info_ok: summary ? Number(summary[3]) : null,
  fail_messages: failMessages.slice(0, 20),
  warn_messages: warnMessages.slice(0, 20),
};

const metricsDir = path.join(resolvedInstance, '.agent', 'metrics');
try {
  fs.mkdirSync(metricsDir, { recursive: true });
  fs.appendFileSync(path.join(metricsDir, 'validate-log.jsonl'), JSON.stringify(record) + '\n');
  console.log(`\nmetrics: logged to ${path.join(metricsDir, 'validate-log.jsonl')}`);
} catch (e) {
  // Logging must never be why a gate run fails; a write problem is reported,
  // not escalated into a red exit code that has nothing to do with the gate.
  console.log(`\nmetrics: could not write log (${e.message})`);
}

process.exit(exitCode);
