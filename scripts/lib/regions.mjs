// ---------------------------------------------------------------------------
// Marked regions. When something mechanical has to read a part of a Markdown
// file rather than the whole of it, the part declares its own boundaries. The
// alternative — locating it by a heading, or by the shape of the tables inside
// it — makes the parser depend on prose that any author may legitimately
// rewrite, and a parser that silently reads the wrong region reports agreement
// it never checked. The vocabulary is one pair of HTML comments:
//   <!-- meridian:begin <name> [key=value ...] -->
//   <!-- meridian:end <name> -->
// Comments render as nothing in every Markdown tool, so the marker costs the
// reader nothing and costs the parser no guessing. Anything other than exactly
// one well-ordered pair is an error, never a fallback to the whole file.
// ---------------------------------------------------------------------------
// Extracted verbatim from scripts/kernel-validate.mjs so that the validator and
// scripts/rule-resolver.mjs read marked regions through one implementation, not
// two. The parsing rules and every error string are unchanged: a container this
// reader cannot parse is a finding, never a quiet read of the whole file.
// ---------------------------------------------------------------------------

// Fenced code blocks are examples, not declarations: a document that explains
// this very syntax would otherwise declare a region by quoting one. Blanked,
// not removed, so every offset computed afterwards still points where it did.
// A fence that never closes is an error rather than a licence to blank the
// rest of the file — otherwise a Markdown typo hides everything after it.
// A closing fence must be at least as long as the opening one (CommonMark),
// or a shorter fence inside a longer one ends the example early.
export function blankFencedBlocks(raw) {
  const lines = raw.split('\n');
  let fenceChar = null;
  let fenceLen = 0;
  let openedAt = -1;
  for (let i = 0; i < lines.length; i++) {
    const m = lines[i].match(/^ {0,3}(`{3,}|~{3,})/);
    if (fenceChar === null) {
      if (!m) continue;
      fenceChar = m[1][0];
      fenceLen = m[1].length;
      openedAt = i;
    } else if (m && m[1][0] === fenceChar && m[1].length >= fenceLen) {
      fenceChar = null;
    }
    lines[i] = lines[i].replace(/[^\r]/g, ' ');
  }
  if (fenceChar !== null) {
    const original = raw.split('\n');
    for (let i = openedAt; i < lines.length; i++) lines[i] = original[i] ?? lines[i];
    return {
      text: lines.join('\n'),
      error: `a code fence opens at line ${openedAt + 1} and is never closed; until it is, everything after it can be read either as an example or as a declaration, and neither reading can be trusted`,
    };
  }
  return { text: lines.join('\n'), error: null };
}

export function markedRegion(raw, name) {
  const fenced = blankFencedBlocks(raw);
  if (fenced.error) return { error: fenced.error };
  const text = fenced.text;
  const begin = [...text.matchAll(new RegExp(`<!--\\s*meridian:begin\\s+${name}\\b([^>]*?)-->`, 'g'))];
  const end = [...text.matchAll(new RegExp(`<!--\\s*meridian:end\\s+${name}\\s*-->`, 'g'))];
  if (begin.length !== 1 || end.length !== 1) {
    return { error: `expected exactly one "meridian:begin ${name}" marker and one "meridian:end ${name}" marker, found ${begin.length} and ${end.length}` };
  }
  const from = begin[0].index + begin[0][0].length;
  const to = end[0].index;
  if (to < from) return { error: `"meridian:end ${name}" comes before "meridian:begin ${name}"` };
  return { text: text.slice(from, to), attrs: begin[0][1].trim() };
}

// Container files hold several subjects at once and are partly written by a
// generator. The unit of intake for them is a declared region, not the file:
// one topic assigned to a whole AGENTS.md would be false about most of it.
// Regions declare themselves with the same marker vocabulary the topic pool
// uses, plus the two attributes a container needs — who owns the region, and
// whether a generator writes it.
export const REGION_TOKEN = /<!--\s*meridian:(begin|end)\s+instruction-section\b([^>]*?)-->/g;

export function parseMarkerAttrs(s) {
  const out = {};
  for (const m of String(s).matchAll(/([a-z][a-z0-9-]*)=("([^"]*)"|[^\s"]+)/g)) out[m[1]] = m[3] ?? m[2];
  return out;
}

// Each region comes back with two views of its body, and they must not be
// mixed:
//   .text        — the parser view: the slice of the fenced-blanked buffer, so
//                  a marker quoted inside a fenced example never counts as a
//                  declaration. Semantic analysis that must not see example
//                  markers reads this.
//   .sourceText  — the verbatim slice of the ORIGINAL raw input between the two
//                  markers, every character intact, fenced code included. This
//                  is the text to hash for a container norm's digest. It uses
//                  the same offsets as .text: every blanking step above is
//                  length-preserving, so a buffer offset is a raw offset too.
export function instructionRegions(raw) {
  // A leading Front Matter block is the file's own metadata, not a subject of
  // intake. It is blanked rather than removed so that every offset below still
  // points where it did.
  let text = raw;
  if (text.startsWith('---')) {
    const close = text.indexOf('\n---', 3);
    if (close !== -1) {
      const after = text.indexOf('\n', close + 1);
      const cut = after === -1 ? text.length : after + 1;
      text = text.slice(0, cut).replace(/[^\n]/g, ' ') + text.slice(cut);
    }
  }
  // The same treatment of fenced examples the topic pool gets, from the same
  // helper: one rule, one implementation, one way to be wrong.
  const fenced = blankFencedBlocks(text);
  text = fenced.text;
  const errors = fenced.error ? [fenced.error] : [];
  const regions = [];
  const seen = new Set();
  let open = null;
  let cursor = 0;
  let outside = '';
  for (const m of text.matchAll(REGION_TOKEN)) {
    const attrs = parseMarkerAttrs(m[2] || '');
    if (m[1] === 'begin') {
      if (open) {
        errors.push(`region "${open.id}" is still open where region "${attrs.id ?? '?'}" begins; regions sit side by side, they do not nest`);
        return { errors, regions: [], uncoveredLines: 0 };
      }
      outside += text.slice(cursor, m.index);
      if (!attrs.id) errors.push('a region begins without an id; a region that cannot be named cannot be recorded');
      else if (seen.has(attrs.id)) errors.push(`region id "${attrs.id}" is declared twice; two regions with one name are one name for two subjects`);
      else seen.add(attrs.id);
      const generated = String(attrs.generated ?? 'no');
      if (!['yes', 'no'].includes(generated)) errors.push(`region "${attrs.id}" declares generated="${generated}"; the answer is yes or no`);
      open = { id: attrs.id ?? '', owner: attrs.owner ?? '', generated: generated === 'yes', start: m.index + m[0].length };
    } else {
      if (!open) { errors.push('a region ends where none is open'); return { errors, regions: [], uncoveredLines: 0 }; }
      if (attrs.id && attrs.id !== open.id) errors.push(`region "${open.id}" is closed by a marker naming "${attrs.id}"`);
      regions.push({
        ...open,
        text: text.slice(open.start, m.index),
        sourceText: raw.slice(open.start, m.index),
      });
      open = null;
    }
    cursor = m.index + m[0].length;
  }
  if (open) errors.push(`region "${open.id}" is opened and never closed; everything after it would silently belong to it`);
  outside += text.slice(cursor);
  // Headings are the container's own scaffolding and belong to no subject.
  // Anything else outside every region is text that no unit of intake covers.
  const uncoveredLines = outside.split('\n').filter((l) => l.trim() && !/^#{1,6}\s/.test(l.trim())).length;
  return { errors, regions, uncoveredLines };
}
