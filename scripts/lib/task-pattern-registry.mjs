// ---------------------------------------------------------------------------
// task-pattern catalog: the one checkable implementation
// ---------------------------------------------------------------------------
// standards/workspace/task-pattern-registry.yaml is a mandatory part of the
// Kernel. scripts/kernel-validate.mjs and test/task-pattern-registry.test.mjs
// both call the functions here so the gate and the standalone set cannot drift
// apart. Nothing in this module reads process state or exits; it takes a parsed
// document, the two schemas, the Kernel root and a "is this path tracked?"
// predicate, and returns a flat list of problem strings — empty means valid.
// ---------------------------------------------------------------------------
import fs from 'node:fs';
import path from 'node:path';

import { validate } from './json-schema.mjs';

// The existing closed pools (standards/workspace/rule-resolution.md §2–§3).
// The catalog reuses them; it must not grow a second vocabulary.
export const WORK_KINDS = ['assessment', 'operation', 'initiative', 'change'];
export const CHANGE_CLASSES = ['BUGFIX', 'FEATURE', 'BEHAVIOR_CHANGE', 'REFACTOR'];
export const REQUIRED_PAIRS = [
  ['assessment', null], ['operation', null], ['initiative', null],
  ['change', 'BUGFIX'], ['change', 'FEATURE'], ['change', 'BEHAVIOR_CHANGE'], ['change', 'REFACTOR'],
];

// The three reference axes are different glossary entities: a `protocol` is a
// mandatory order, a `skill` is a way of carrying work out, an evidence
// contract says what proof must contain. They are kept in separate arrays and
// a value from one may not appear in another (see checkLinkClassification).
export const LINK_FIELDS = ['applicable_protocols', 'applicable_skills', 'applicable_evidence_contracts'];

// Canonical routes that must stay wired.
export const REFACTOR_PROTOCOL = 'verification/functional-parity/refactor-protocol.md';
export const REFACTOR_EVIDENCE = 'verification/functional-parity/functional-parity-evidence-contract.md';
export const BUGFIX_SKILL = 'skills/bugfix-protocol/SKILL.md';

// A skill package artifact: SKILL.md directly under a single-segment folder in
// skills/. This is the shape the Kernel already uses for vendored skills.
const SKILL_ARTIFACT_RE = /^skills\/[^/]+\/SKILL\.md$/;
const isSkillArtifactPath = (p) => typeof p === 'string' && SKILL_ARTIFACT_RE.test(p);
const isUnderSkills = (p) => typeof p === 'string' && (p === 'skills' || p.startsWith('skills/'));

const presentLinks = (links) => (Array.isArray(links) ? links : []).filter((l) => l && l.status === 'present');
const presentPaths = (links) => presentLinks(links).map((l) => l.path).filter((p) => typeof p === 'string');
const pairKey = (p) => `${p && p.payload && p.payload.work_kind} :: ${(p && p.payload && p.payload.change_class) || ''}`;
const pairLabel = (wk, cc) => `(${wk}${cc ? `, ${cc}` : ''})`;

// A `present` canonical link must point at a regular, tracked file that is
// genuinely inside the Kernel — proven by construction, not by "the external
// file happens to be absent". Returns { ok: true } or { ok: false, reason }.
export function checkKernelLinkTarget(relPath, { kernelRoot, isTracked }) {
  if (typeof relPath !== 'string' || relPath === '') {
    return { ok: false, reason: 'the path is empty' };
  }
  if (relPath.includes('\\')) {
    return { ok: false, reason: 'the path uses a backslash as a separator; a Kernel path is POSIX-relative' };
  }
  if (/^[a-zA-Z]:/.test(relPath) || path.posix.isAbsolute(relPath) || path.win32.isAbsolute(relPath)) {
    return { ok: false, reason: 'the path is absolute; a canonical link is a relative path inside the Kernel' };
  }
  const segments = relPath.split('/');
  if (segments.some((s) => s === '' || s === '.' || s === '..')) {
    return { ok: false, reason: 'the path carries an empty, "." or ".." segment; it must already be normalised' };
  }

  const root = path.resolve(kernelRoot);
  const lexical = path.resolve(root, relPath);
  const lexicalRel = path.relative(root, lexical);
  if (lexicalRel === '' || lexicalRel.startsWith('..') || path.isAbsolute(lexicalRel)) {
    return { ok: false, reason: 'the path does not resolve lexically inside the Kernel root' };
  }

  let realRoot;
  try { realRoot = fs.realpathSync(root); }
  catch { return { ok: false, reason: 'the Kernel root itself cannot be resolved' }; }
  let realTarget;
  try { realTarget = fs.realpathSync(lexical); }
  catch { return { ok: false, reason: 'the target does not exist' }; }
  const realRel = path.relative(realRoot, realTarget);
  if (realRel === '' || realRel.startsWith('..') || path.isAbsolute(realRel)) {
    return { ok: false, reason: 'the target resolves outside the Kernel once symbolic links are followed' };
  }

  let st;
  try { st = fs.statSync(realTarget); }
  catch { return { ok: false, reason: 'the target cannot be inspected' }; }
  if (!st.isFile()) {
    return { ok: false, reason: 'the target is not a regular file' };
  }

  if (typeof isTracked === 'function' && !isTracked(relPath)) {
    return { ok: false, reason: 'the target is not in the Kernel\'s tracked file set' };
  }
  return { ok: true };
}

// Guard against the one normative contradiction the owner resolved: the catalog
// treats skills/bugfix-protocol/SKILL.md as a `skill` and records that no
// separate BUGFIX `protocol` document exists. standards/workspace/rule-resolution.md
// must not, at the same time, still call `BUGFIX → bugfix-protocol` a route to a
// Kernel *protocol*. This is a text-level check on that exact wording; it does
// not touch any other resolution rule. Returns a flat list of problem strings.
export function checkRuleResolutionBugfixConsistency(ruleResolutionText) {
  const problems = [];
  const text = typeof ruleResolutionText === 'string' ? ruleResolutionText : '';
  if (/BUGFIX\s*(?:→|-&gt;|->|-->)\s*bugfix-protocol/.test(text)) {
    problems.push('rule-resolution.md still routes "BUGFIX → bugfix-protocol" as a protocol route; bugfix-protocol is a skill (owner decision), and no separate BUGFIX protocol exists');
  }
  for (const line of text.split('\n')) {
    if (/bugfix-protocol/.test(line) && /протокол\s+ядра/i.test(line)) {
      problems.push('rule-resolution.md names bugfix-protocol on the same line as "протокол ядра"; bugfix-protocol is a skill, not a Kernel protocol');
      break;
    }
  }
  return problems;
}

// The classification guard: a skill artifact may only sit in applicable_skills,
// and applicable_protocols / applicable_evidence_contracts may not carry a
// skill artifact. This is what mechanically proves "a skill cannot be put in
// the protocol array and a protocol cannot be put in the skill array".
function checkLinkClassification(patternId, field, link) {
  const p = link && link.path;
  if (field === 'applicable_skills') {
    if (!isSkillArtifactPath(p)) {
      return `pattern "${patternId}" applicable_skills link "${p}" is not a skill package (skills/<name>/SKILL.md); a protocol or contract is a different axis`;
    }
    return null;
  }
  if (isUnderSkills(p)) {
    return `pattern "${patternId}" ${field} link "${p}" points at a skill package; a way of carrying work out belongs in applicable_skills, not among protocols or evidence contracts`;
  }
  return null;
}

// The whole composition pipeline for one registry document: container + payload
// schema, per-record envelope schema, then the cross-record rules the JSON
// Schema subset cannot state.
export function evaluateTaskPatternRegistry(doc, { registrySchema, envelopeSchema, kernelRoot, isTracked }) {
  const problems = [];

  const containerErrs = [];
  try { validate(doc, registrySchema, registrySchema, '', containerErrs); }
  catch (e) { return [`container/payload schema could not be applied: ${e.message}`]; }
  problems.push(...containerErrs);

  const entries = Array.isArray(doc && doc.task_patterns) ? doc.task_patterns : [];
  for (let i = 0; i < entries.length; i += 1) {
    const envErrs = [];
    try { validate(entries[i], envelopeSchema, envelopeSchema, '', envErrs); }
    catch (e) { problems.push(`entry ${i} envelope could not be applied: ${e.message}`); continue; }
    problems.push(...envErrs.map((m) => `entry ${i} envelope ${m}`));
  }

  const seenIds = new Set();
  for (const p of entries) {
    const id = p && p.id;
    if (typeof id !== 'string') continue;
    if (seenIds.has(id)) problems.push(`pattern id "${id}" is declared more than once`);
    seenIds.add(id);
    if (p.record_type !== 'task-pattern') {
      problems.push(`pattern "${id}" declares record_type "${p.record_type}", not "task-pattern"`);
    }
  }

  const pairCount = new Map();
  for (const p of entries) {
    const wk = p && p.payload && p.payload.work_kind;
    const cc = (p && p.payload && p.payload.change_class) || null;
    const k = pairKey(p);
    pairCount.set(k, (pairCount.get(k) || 0) + 1);
    if (wk !== undefined && !REQUIRED_PAIRS.some(([w, c]) => w === wk && c === cc)) {
      problems.push(`pattern "${p && p.id}" carries classification pair ${pairLabel(wk, cc)}, which is not one of the seven active pairs`);
    }
  }
  for (const [k, n] of pairCount) {
    if (n <= 1) continue;
    const [wk, cc] = k.split(' :: ');
    problems.push(`classification pair ${pairLabel(wk, cc || null)} is declared ${n} times; each active pair appears exactly once`);
  }
  for (const [wk, cc] of REQUIRED_PAIRS) {
    if (!pairCount.has(`${wk} :: ${cc || ''}`)) {
      problems.push(`no pattern for classification pair ${pairLabel(wk, cc)}; all seven are mandatory`);
    }
  }

  for (const p of entries) {
    for (const field of LINK_FIELDS) {
      const links = p && p.payload && p.payload[field];
      for (const link of (Array.isArray(links) ? links : [])) {
        if (!link || link.status !== 'present') continue;
        const classProblem = checkLinkClassification(p && p.id, field, link);
        if (classProblem) { problems.push(classProblem); continue; }
        const verdict = checkKernelLinkTarget(link.path, { kernelRoot, isTracked });
        if (!verdict.ok) {
          problems.push(`pattern "${p && p.id}" ${field} link "${link.path}": ${verdict.reason}`);
        }
      }
    }
  }

  const find = (wk, cc) => entries.find((p) => p && p.payload
    && p.payload.work_kind === wk && ((p.payload.change_class) || null) === (cc || null));

  const refactor = find('change', 'REFACTOR');
  if (refactor) {
    if (!presentPaths(refactor.payload.applicable_protocols).includes(REFACTOR_PROTOCOL)) {
      problems.push(`the REFACTOR pattern does not route to the functional-parity protocol "${REFACTOR_PROTOCOL}"`);
    }
    if (!presentPaths(refactor.payload.applicable_evidence_contracts).includes(REFACTOR_EVIDENCE)) {
      problems.push(`the REFACTOR pattern does not reference the functional-parity evidence contract "${REFACTOR_EVIDENCE}"`);
    }
  }

  const bugfix = find('change', 'BUGFIX');
  if (bugfix) {
    const bugfixSkills = new Set(presentPaths(bugfix.payload.applicable_skills));
    if (!bugfixSkills.has(BUGFIX_SKILL)) {
      problems.push(`the BUGFIX pattern does not reference the bugfix way of carrying work out "${BUGFIX_SKILL}" in applicable_skills`);
    }
    for (const [label, other] of [['FEATURE', find('change', 'FEATURE')], ['BEHAVIOR_CHANGE', find('change', 'BEHAVIOR_CHANGE')]]) {
      if (!other) continue;
      const shared = [
        ...presentPaths(other.payload.applicable_skills),
        ...presentPaths(other.payload.applicable_protocols),
      ].filter((x) => bugfixSkills.has(x));
      if (shared.length) {
        problems.push(`the BUGFIX pattern shares "${shared[0]}" with the ${label} pattern; BUGFIX must not be conflated with FEATURE or BEHAVIOR_CHANGE`);
      }
    }
  }

  const initiative = find('initiative', null);
  if (initiative) {
    if (initiative.payload.decomposition_required !== true) problems.push('the initiative pattern does not require decomposition');
    if ('change_class' in (initiative.payload || {})) problems.push('the initiative pattern carries a change_class');
  }

  return problems;
}
