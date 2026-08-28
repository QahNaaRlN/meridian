// ---------------------------------------------------------------------------
// YAML: documented subset. Supports block mappings, block sequences, plain and
// quoted scalars, `|`/`>` block scalars with optional `-` chomping, comments,
// empty values as null.
// Throws UnsupportedYaml on flow style, anchors, aliases, tags, multi-document
// streams and tab indentation.
// ---------------------------------------------------------------------------
// Extracted verbatim from scripts/kernel-validate.mjs so that the validator and
// scripts/rule-resolver.mjs read YAML through one implementation, not two
// copies. The documented subset and every throw are unchanged: a construct this
// reader does not implement still turns into an error, never a quiet misread.
// ---------------------------------------------------------------------------
export class UnsupportedYaml extends Error {}

// A block scalar header may carry a chomping indicator. `-` strips the trailing
// newline, which is what this reader does anyway, so it is accepted; `+` keeps
// trailing newlines, which this reader does not model, so it is refused rather
// than quietly read as `|`. Anything else after the indicator (an explicit
// indent, say) is likewise refused. This mattered: `>-` used to fall through to
// the plain-scalar branch, and every indented line under it was then read as a
// sibling key — the fields of a record silently vanished instead of the reader
// saying it could not read them.
function blockScalarStyle(v) {
  const m = /^([|>])([-+]?)$/.exec(v);
  if (!m) return null;
  if (m[2] === '+') throw new UnsupportedYaml(`block scalar with keep chomping ("${v}") is not implemented by this reader`);
  return m[1];
}

function stripComment(line) {
  let inS = false, inD = false;
  for (let i = 0; i < line.length; i++) {
    const c = line[i];
    if (c === "'" && !inD) inS = !inS;
    else if (c === '"' && !inS) inD = !inD;
    else if (c === '#' && !inS && !inD && (i === 0 || /\s/.test(line[i - 1]))) return line.slice(0, i);
  }
  return line;
}

// Flow-style collections: [], {}, [a, b], {k: v}, nested. Quoted strings are
// honoured; ':' only terminates a token when a key is being read inside {},
// so plain scalars containing ':' (URLs) survive inside sequences.
function parseFlow(src) {
  let i = 0;
  const ws = () => { while (i < src.length && /\s/.test(src[i])) i++; };
  const token = (isKey) => {
    ws();
    if (src[i] === "'" || src[i] === '"') {
      const q = src[i++]; let out = '';
      while (i < src.length && src[i] !== q) out += src[i++];
      if (src[i] !== q) throw new UnsupportedYaml('unterminated quoted string in flow collection');
      i++; return out;
    }
    const stop = isKey ? ',]}:' : ',]}';
    const start = i;
    while (i < src.length && !stop.includes(src[i])) i++;
    const raw = src.slice(start, i).trim();
    return isKey ? raw : parseScalar(raw);
  };
  const value = () => {
    ws();
    if (src[i] === '[') {
      i++; const arr = []; ws();
      if (src[i] === ']') { i++; return arr; }
      for (;;) {
        arr.push(value()); ws();
        if (src[i] === ',') { i++; continue; }
        if (src[i] === ']') { i++; return arr; }
        throw new UnsupportedYaml('malformed flow sequence');
      }
    }
    if (src[i] === '{') {
      i++; const obj = {}; ws();
      if (src[i] === '}') { i++; return obj; }
      for (;;) {
        const k = token(true); ws();
        if (src[i] !== ':') throw new UnsupportedYaml('malformed flow mapping');
        i++; obj[k] = value(); ws();
        if (src[i] === ',') { i++; continue; }
        if (src[i] === '}') { i++; return obj; }
        throw new UnsupportedYaml('malformed flow mapping');
      }
    }
    return token(false);
  };
  const out = value();
  ws();
  if (i !== src.length) throw new UnsupportedYaml('trailing content after flow collection');
  return out;
}

function parseScalar(raw) {
  const s = raw.trim();
  if (s === '' || s === '~' || s === 'null') return null;
  if (s.startsWith('{') || s.startsWith('[')) return parseFlow(s);
  if (s.startsWith('&') || s.startsWith('*')) throw new UnsupportedYaml('anchor or alias');
  if (s.startsWith('!')) throw new UnsupportedYaml('explicit tag');
  if (/^'(.*)'$/s.test(s)) return s.slice(1, -1).replace(/''/g, "'");
  if (/^"(.*)"$/s.test(s)) return s.slice(1, -1).replace(/\\"/g, '"');
  if (s === 'true') return true;
  if (s === 'false') return false;
  if (/^-?\d+$/.test(s)) return Number.parseInt(s, 10);
  if (/^-?\d+\.\d+$/.test(s)) return Number.parseFloat(s);
  return s;
}

export function yamlParse(text) {
  const rawLines = text.split(/\r?\n/);
  const lines = [];
  rawLines.forEach((raw, i) => {
    if (/^\t/.test(raw)) throw new UnsupportedYaml(`tab indentation at line ${i + 1}`);
    if (i > 0 && /^---\s*$/.test(raw)) throw new UnsupportedYaml(`multi-document stream at line ${i + 1}`);
    const stripped = stripComment(raw);
    if (stripped.trim() === '') return;
    lines.push({ indent: stripped.match(/^ */)[0].length, text: stripped.trim(), n: i + 1 });
  });

  let pos = 0;

  function readBlockScalar(parentIndent, style) {
    const parts = [];
    while (pos < lines.length && lines[pos].indent > parentIndent) parts.push(lines[pos++].text);
    return style === '|' ? parts.join('\n') : parts.join(' ');
  }

  function parseNode(indent) {
    if (pos >= lines.length || lines[pos].indent < indent) return null;
    if (lines[pos].text.startsWith('- ') || lines[pos].text === '-') {
      const arr = [];
      while (pos < lines.length && lines[pos].indent === indent &&
             (lines[pos].text.startsWith('- ') || lines[pos].text === '-')) {
        const line = lines[pos];
        const rest = line.text === '-' ? '' : line.text.slice(2).trim();
        if (rest === '') { pos++; arr.push(parseNode(indent + 2)); continue; }
        const m = rest.match(/^([A-Za-z0-9_$.-]+):(?:\s+(.*))?$/);
        if (m) {
          // sequence item that is a mapping; its first key sits on this line
          const itemIndent = indent + 2;
          const obj = {};
          const key = m[1];
          const inlineVal = (m[2] ?? '').trim();
          pos++;
          const inlineStyle = blockScalarStyle(inlineVal);
          if (inlineStyle) obj[key] = readBlockScalar(itemIndent, inlineStyle);
          else if (inlineVal === '') {
            const child = (pos < lines.length && lines[pos].indent > itemIndent) ? parseNode(lines[pos].indent) : null;
            obj[key] = child;
          } else obj[key] = parseScalar(inlineVal);
          const more = parseNode(itemIndent);
          if (more && typeof more === 'object' && !Array.isArray(more)) Object.assign(obj, more);
          arr.push(obj);
        } else { pos++; arr.push(parseScalar(rest)); }
      }
      return arr;
    }

    const obj = {};
    while (pos < lines.length && lines[pos].indent === indent) {
      const line = lines[pos];
      if (line.text.startsWith('- ')) break;
      const m = line.text.match(/^([A-Za-z0-9_$.-]+):(?:\s+(.*))?$/);
      if (!m) throw new UnsupportedYaml(`unrecognized construct at line ${line.n}: ${line.text}`);
      const key = m[1];
      const val = (m[2] ?? '').trim();
      pos++;
      const style = blockScalarStyle(val);
      if (style) { obj[key] = readBlockScalar(indent, style); continue; }
      if (val === '') {
        if (pos < lines.length && lines[pos].indent > indent) obj[key] = parseNode(lines[pos].indent);
        else if (pos < lines.length && lines[pos].indent === indent &&
                 (lines[pos].text.startsWith('- ') || lines[pos].text === '-')) obj[key] = parseNode(indent);
        else obj[key] = null;
        continue;
      }
      obj[key] = parseScalar(val);
    }
    return obj;
  }

  const result = parseNode(lines.length ? lines[0].indent : 0);
  return result ?? {};
}
