//! A minimal, private canonical-JSON value, writer and reader.
//!
//! This is deliberately **not** the general "transport JSON structures and
//! parsers" the package instructions forbid in `meridian-core`: [`Json`] is
//! never exported from this crate, never accepts or represents an arbitrary
//! top-level transport document, and this module exists for exactly two
//! narrowly-scoped, internal purposes:
//!
//! 1. serializing a migration plan's own already-typed fields into the
//!    EXACT canonical byte sequence `computePlanFingerprint`/
//!    `computeIdempotencyKey` (`scripts/lib/instance-data-migration.mjs`)
//!    hash — so `plan_fingerprint`/`idempotency_key` are byte-compatible
//!    with the Node reference, not merely "some deterministic Rust format"
//!    ([`super::canonical`]);
//! 2. checking whether a [`super::ContentEnvelope`]'s declared JSON content
//!    is already in that same canonical form
//!    (`checkContentEnvelope`'s JSON branch, `instance-data-migration.md`
//!    §4.1) — the parsed value is discarded immediately after the
//!    byte-for-byte comparison, never handed back to a caller.
//!
//! [`is_canonical`] targets behavioural equivalence with
//! `content === JSON.stringify(canonicalize(JSON.parse(content)))`
//! (`scripts/lib/instance-data-migration.mjs`), not merely "some
//! deterministic Rust format that happens to look similar" — specifically:
//!
//! - **numbers**: every finite JSON number (integer, fractional,
//!   exponential) is parsed to the `f64` it would be in JS (`JSON.parse`
//!   always produces a float64, exactly like this module's `Json::Number`)
//!   and re-serialized via [`js_number_to_string`], a direct port of the
//!   ECMA-262 `Number::toString` formatting rules — the same case split
//!   (plain integer / decimal-point / leading-zeros form / exponential)
//!   `JSON.stringify` uses. The significant-digit string itself comes from
//!   the `ryu` crate's shortest-round-trip formatting, NOT Rust's own `{:e}`
//!   (`LowerExp`): differential testing against real `JSON.stringify`
//!   PROVED `{:e}` disagrees with ECMA-262's round-to-even tie-break rule at
//!   exact decimal ties (counterexample: `-86666878403640.625`, where `{:e}`
//!   picks the odd digit `...63` and JS picks the even digit `...62`).
//!   Compatibility is backed by a large deterministic differential corpus
//!   (`meridian-core/tests/`) generated from raw `f64` bit patterns and
//!   checked against real `JSON.stringify`/`checkContentEnvelope` output —
//!   this is strong evidence, not a mathematical proof of agreement for
//!   every one of the ~2^64 finite `f64` values, so this module does not
//!   claim unconditional exact equivalence. A number that overflows to
//!   infinity when parsed is rejected (`JSON.stringify` of a non-finite
//!   number is never the same text as a finite numeral, so it is never
//!   canonical either way).
//! - **strings**: JS strings are sequences of UTF-16 CODE UNITS, not Unicode
//!   scalar values, and can contain "lone"/unpaired surrogates (a code unit
//!   in `0xD800..=0xDFFF` that is not part of a valid high+low pair) —
//!   `JSON.parse`/`JSON.stringify` round-trip these correctly, so a parsed
//!   string's content is kept as `Vec<u16>` rather than a Rust `String`
//!   (which cannot represent a lone surrogate at all). [`write_json_string`]
//!   reproduces `QuoteJSONString`'s surrogate handling exactly: a code unit
//!   that is part of a valid pair is combined and emitted as the literal
//!   astral character, while a lone/unpaired surrogate is individually
//!   `\u`-escaped — confirmed against real `JSON.stringify` output for
//!   lone-high, lone-low, valid-pair, and adjacent-lone-surrogate cases (see
//!   `meridian-core/tests/`). Object KEYS use the EXACT SAME UTF-16
//!   code-unit model as string values — [`Json::Object`] stores each key as
//!   `Vec<u16>`, so a parsed key containing a lone surrogate (e.g. the `$`
//!   key of `{"\ud800":1}`, which real `checkContentEnvelope` accepts) is
//!   represented exactly, not converted to a Rust `String` and rejected.
//!   [`Json::Literal`] keeps its `&'static str` keys: those are always
//!   compile-time-known Rust identifiers, never a lone surrogate, so no
//!   `Vec<u16>` representation is needed for them.
//! - **duplicate object keys**: `JSON.parse` semantics — the LAST value for
//!   a repeated key wins, collapsing the object to one entry per unique
//!   key — are reproduced in [`Parser::parse_object`] by comparing keys as
//!   UTF-16 code unit sequences (`Vec<u16>` equality), so source text with a
//!   duplicate key (including a duplicated lone surrogate key) can never be
//!   mistaken for canonical (its re-serialized form always has fewer
//!   entries than the literal text).
//! - **key order is NOT a single lexicographic sort**: `canonicalize()`
//!   itself only does `Object.keys(value).sort()` (UTF-16 code-unit order —
//!   see [`array_index_value`]'s documentation for why this alone is
//!   insufficient) while copying each key into a fresh plain object `out`
//!   — but the canonical TEXT is `JSON.stringify(canonicalize(...))`, and
//!   `JSON.stringify` enumerates `out`'s keys using ECMA-262
//!   `OrdinaryOwnPropertyKeys`, which does NOT preserve insertion order for
//!   every key: it lists every "array index" key (the canonical decimal
//!   string of an integer `0..=2^32-2`, e.g. `"2"`, `"10"`, but NOT `"00"`
//!   or `"4294967295"`) FIRST, in ascending NUMERIC order, and only THEN
//!   every other string key, in whatever order they were inserted into
//!   `out` (which — because that insertion happened during the
//!   `Object.keys(value).sort()` loop — is UTF-16 code-unit order for the
//!   non-index keys specifically, not for the object's keys as a whole).
//!   [`Json::Object`]'s writer ([`array_index_value`]) reproduces exactly
//!   this two-tier order: `{"2":2,"10":10}` is the real
//!   `checkContentEnvelope`'s canonical form for that data — `"10"` would
//!   sort BEFORE `"2"` under plain UTF-16 order (`'1'` < `'2'`), but as an
//!   array index it is numerically LARGER, so it is written second, and
//!   `{"10":10,"2":2}` (the plain-lexicographic order) is REJECTED as
//!   non-canonical — confirmed against the real reference, along with the
//!   `"0"`/`"00"` and `"4294967294"`/`"4294967295"` index-boundary cases,
//!   in `meridian-core/tests/json_canonical_reference.rs`.
//! - **`"__proto__"` is dropped, not merely reordered**: see
//!   [`is_dunder_proto_key`]'s documentation for the ECMAScript mechanism
//!   (an inherited ACCESSOR property, not an own one, triggered by
//!   `canonicalize()`'s own `out[k] = ...` assignment) and the reference
//!   cases (top-level, nested, and a `null` value) this reproduces.
//!
//! The reader still fail-closes (never treated as canonical) on anything it
//! cannot parse or cannot represent with certainty.

/// A JSON value, restricted to what this module needs to build or check.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Json {
    Null,
    Bool(bool),
    /// A JSON number, stored as the `f64` `JSON.parse` would produce —
    /// every JSON number, integer or not, is one `f64` in JS. Serialized
    /// via [`js_number_to_string`].
    Number(f64),
    /// A JSON string's content, as UTF-16 CODE UNITS rather than a Rust
    /// `String` — see the module documentation for why (lone surrogates).
    Str(Vec<u16>),
    Array(Vec<Json>),
    /// Serialized with array-index keys first (ascending numeric order)
    /// then every other key by UTF-16 code unit sequence, and with any
    /// `"__proto__"` key silently dropped — matches `canonicalize()`'s
    /// `Object.keys(value).sort()` + `out[k] = ...` copy followed by
    /// `JSON.stringify`'s own key enumeration (see the module
    /// documentation for why this is neither a single sort nor a plain
    /// copy — NOT Rust's own `str` ordering either). Every value the
    /// [`parse`] reader produces uses this variant, with at most one entry
    /// per unique key (a parsed duplicate key collapses to its LAST value,
    /// matching `JSON.parse`); a parsed document never contains
    /// [`Json::Literal`].
    ///
    /// A key is `Vec<u16>` — the same UTF-16 code-unit model
    /// [`Json::Str`] uses for string values, not a Rust `String` — so a
    /// key containing a lone surrogate is represented exactly rather than
    /// being rejected (see the module documentation).
    Object(Vec<(Vec<u16>, Json)>),
    /// A JSON object whose key order is exactly as declared — never
    /// sorted. Used only when building output for the two outer wrapper
    /// objects `computePlanFingerprint`/`computeIdempotencyKey` construct
    /// as plain JavaScript object literals rather than passing through
    /// `canonicalize()` (`scripts/lib/instance-data-migration.mjs`), and
    /// for object literals nested directly inside them
    /// (`computeIdempotencyKey`'s own `scope`/`digest`). Composes with
    /// [`Json::Object`]/[`Json::Array`] like any other value.
    Literal(Vec<(&'static str, Json)>),
}

impl Json {
    /// Builds a [`Json::Str`] from a known-valid Rust string (always a
    /// sequence of Unicode scalar values, never a lone surrogate) by
    /// encoding it to UTF-16 code units. For content that may itself
    /// contain a lone surrogate (parsed JSON string values), construct
    /// [`Json::Str`] directly from the parser's `Vec<u16>` instead.
    pub(crate) fn str(s: impl Into<String>) -> Self {
        Json::Str(s.into().encode_utf16().collect())
    }

    /// Builds a [`Json::Object`] key from a known-valid Rust string slice
    /// (always a sequence of Unicode scalar values, never a lone
    /// surrogate) by encoding it to UTF-16 code units — the object-key
    /// counterpart of [`Json::str`] for values. For a key that may itself
    /// contain a lone surrogate (a parsed JSON object key), use the
    /// parser's own `Vec<u16>` directly instead.
    pub(crate) fn key(s: &str) -> Vec<u16> {
        s.encode_utf16().collect()
    }

    /// Serializes into canonical JSON text.
    pub(crate) fn to_canonical_string(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Json::Number(n) => out.push_str(&js_number_to_string(*n)),
            Json::Str(units) => write_json_string(units, out),
            Json::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Json::Object(entries) => {
                // `canonicalize()`'s `out[k] = canonicalize(value[k])` never
                // creates an own `"__proto__"` property on `out` — see
                // `is_dunder_proto_key`'s documentation — so that key (at
                // ANY value type, and at any nesting depth, since every
                // nested `Json::Object` applies this same filter
                // independently) is dropped before anything else runs.
                let visible = entries.iter().filter(|(k, _)| !is_dunder_proto_key(k));

                // ECMA-262 `OwnPropertyKeys` order for an ordinary object
                // (what `JSON.stringify`'s own key enumeration follows):
                // "array index" string keys first, in ascending NUMERIC
                // order, then every other string key in insertion order —
                // and `canonicalize()` inserts non-index keys into `out` in
                // `Object.keys(value).sort()` order, i.e. UTF-16 code-unit
                // order. See `array_index_value` and the module
                // documentation for why a single UTF-16 lexicographic sort
                // over ALL keys is not this order.
                let mut index_entries: Vec<(u32, &(Vec<u16>, Json))> = Vec::new();
                let mut other_entries: Vec<&(Vec<u16>, Json)> = Vec::new();
                for entry in visible {
                    match array_index_value(&entry.0) {
                        Some(i) => index_entries.push((i, entry)),
                        None => other_entries.push(entry),
                    }
                }
                index_entries.sort_by_key(|(i, _)| *i);
                other_entries.sort_by(|a, b| a.0.cmp(&b.0));

                out.push('{');
                let mut first = true;
                for (k, v) in index_entries
                    .into_iter()
                    .map(|(_, entry)| entry)
                    .chain(other_entries)
                {
                    if !first {
                        out.push(',');
                    }
                    first = false;
                    write_json_string(k, out);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
            Json::Literal(entries) => {
                out.push('{');
                for (i, (k, v)) in entries.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_json_key_string(k, out);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }
}

/// A [`Json::Literal`] key is always a compile-time `&'static str` (see the
/// module documentation for why lone-surrogate keys never arise there), so
/// it is always a valid sequence of Unicode scalar values — encoding it to
/// UTF-16 code units first can never produce a lone surrogate, and
/// [`write_json_string`]'s pairing logic degenerates to "emit every
/// character literally or escape it," matching [`Json::str`]'s own
/// encoding step. [`Json::Object`] keys are already `Vec<u16>` and go
/// straight to [`write_json_string`] instead.
fn write_json_key_string(k: &str, out: &mut String) {
    let units: Vec<u16> = k.encode_utf16().collect();
    write_json_string(&units, out);
}

/// `QuoteJSONString` (ECMA-262), operating on UTF-16 CODE UNITS rather than
/// Unicode scalar values (see the module documentation): `"` and `\` are
/// backslash-escaped, the named single-character escapes are used for
/// U+0008/U+0009/U+000A/U+000C/U+000D, every other code unit below U+0020
/// becomes `\u00XX` (lowercase hex), a high surrogate immediately followed
/// by a low surrogate is combined and emitted as the literal astral
/// character it represents, and every other code unit — including a
/// LONE/unpaired surrogate and the full non-ASCII BMP range — is emitted
/// literally if it is a valid scalar value on its own, or individually
/// `\u`-escaped if it is a lone surrogate. This matches `JSON.stringify`'s
/// actual behaviour, confirmed against real Node output for lone-high,
/// lone-low, valid-pair, and adjacent-lone-surrogate inputs (see
/// `meridian-core/tests/`).
fn write_json_string(units: &[u16], out: &mut String) {
    out.push('"');
    let mut i = 0;
    while i < units.len() {
        let u = units[i];
        match u {
            0x22 => out.push_str("\\\""),
            0x5C => out.push_str("\\\\"),
            0x08 => out.push_str("\\b"),
            0x09 => out.push_str("\\t"),
            0x0A => out.push_str("\\n"),
            0x0C => out.push_str("\\f"),
            0x0D => out.push_str("\\r"),
            _ if u < 0x20 => out.push_str(&format!("\\u{u:04x}")),
            0xD800..=0xDBFF => {
                // High surrogate: combine with an immediately-following low
                // surrogate into the astral character it represents; a lone
                // high surrogate (no following low) is individually escaped.
                let low = units.get(i + 1).copied();
                if let Some(low) = low.filter(|l| (0xDC00..=0xDFFF).contains(l)) {
                    let c32 = 0x10000u32 + (((u as u32) - 0xD800) << 10) + ((low as u32) - 0xDC00);
                    let c = char::from_u32(c32)
                        .expect("a valid high+low surrogate pair always decodes");
                    out.push(c);
                    i += 2;
                    continue;
                }
                out.push_str(&format!("\\u{u:04x}"));
            }
            // A low surrogate reaching this branch was never consumed as
            // the second half of a pair above, so it is lone by
            // construction.
            0xDC00..=0xDFFF => out.push_str(&format!("\\u{u:04x}")),
            _ => {
                let c = char::from_u32(u as u32)
                    .expect("a non-surrogate UTF-16 code unit is always a valid scalar value");
                out.push(c);
            }
        }
        i += 1;
    }
    out.push('"');
}

/// Whether `key` is the EXACT UTF-16 spelling of `"__proto__"`.
///
/// `canonicalize()` builds each object's canonical form as
/// `const out = {}; for (const k of Object.keys(value).sort()) out[k] =
/// canonicalize(value[k]);`. `out[k] = ...` is an ordinary ECMAScript
/// property assignment — for every OTHER key this performs `[[Set]]`,
/// which (since `out` has no matching own property yet) falls through to
/// `CreateDataProperty` and adds a normal own property. But
/// `Object.prototype.__proto__` is an inherited ACCESSOR property (a
/// getter/setter pair, `Annex B.2.2.1`), so `[[Set]]` finds it on the
/// prototype chain and invokes its SETTER instead: the setter either
/// changes `out`'s own `[[Prototype]]` (when the assigned value is an
/// Object or `null`) or silently does nothing at all (any other value
/// type, per the setter's own spec algorithm) — either way, `out` never
/// gains an own `"__proto__"` property. This holds regardless of what
/// `value["__proto__"]` actually was (a number, `null`, a nested object,
/// …), so this key is unconditionally absent from a canonicalized
/// object's enumerable keys — confirmed end-to-end against the real
/// `checkContentEnvelope` for `{"__proto__":1}` (rejected: canonicalizes
/// to `{}`), `{"a":{"__proto__":1}}` (rejected: canonicalizes to
/// `{"a":{}}`), and `{"__proto__":null}` (also rejected: the setter DOES
/// run for `null` and changes the prototype, but STILL creates no own
/// property) — see `meridian-core/tests/json_canonical_reference.rs`.
///
/// This is a property of the SPECIFIC `out[k] = ...` assignment
/// `canonicalize()` happens to use, not of JSON objects or of
/// `"__proto__"` in general — an ordinary own data property (e.g.
/// `"constructor"`, also `Object.prototype`-inherited but as a normal
/// data property, not an accessor) is unaffected and round-trips
/// normally; only [`Json::Object`] (the parser's/`canonicalize()`'s own
/// representation) applies this filter — [`Json::Literal`]'s keys are
/// fixed Rust identifiers that are never `"__proto__"`, so it needs no
/// equivalent (and, being built directly as a JS object-literal
/// `{k: v}`, would be governed by the UNRELATED `__proto__`-in-object-
/// initializer grammar production, `Annex B.3.1`, not this one).
fn is_dunder_proto_key(key: &[u16]) -> bool {
    const UNITS: [u16; 9] = [0x5f, 0x5f, 0x70, 0x72, 0x6f, 0x74, 0x6f, 0x5f, 0x5f];
    key == UNITS.as_slice()
}

/// The ECMA-262 array-index VALUE of `key`, if `key` is the EXACT
/// canonical decimal string of an integer `i` with `0 <= i <= 2^32 - 2`
/// (`6.1.7 Array Index`: an array index is an integer index `i` in range
/// `0..=2^32-2` — NOT `0..=2^32-1`; `2^32-1` is a valid array LENGTH but
/// is deliberately excluded from being an index, so `"4294967295"` is
/// never one). "Exact canonical decimal string" means: ASCII digits
/// only (no sign, no whitespace, no other radix); no leading zero unless
/// the key is the single digit `"0"` itself (so `"00"` is NOT an index,
/// even though it denotes the same integer). `None` for every other key,
/// including one containing a surrogate.
///
/// This is [`Json::write`]'s Object arm's KEY-ORDERING primitive, not a
/// general string-to-integer helper: `JSON.stringify`'s own key
/// enumeration for a plain object (ECMA-262 `OrdinaryOwnPropertyKeys`)
/// lists every array-index key FIRST, in ascending numeric order, before
/// any other string key — this is why `{"2":2,"10":10}` is the real
/// `checkContentEnvelope`'s canonical form and `{"10":10,"2":2}` is
/// rejected as non-canonical, even though `"10"` sorts before `"2"` under
/// plain UTF-16 lexicographic order (`'1'` = `0x31` < `'2'` = `0x32`) — a
/// single lexicographic sort over ALL keys is therefore not this order;
/// see the module documentation and
/// `meridian-core/tests/json_canonical_reference.rs` for the
/// `{"2":2,"10":10}`/`{"10":10,"2":2}` reference pair (and the
/// `"0"`/`"00"`/`"4294967294"`/`"4294967295"` boundary cases) proven
/// against the real reference.
fn array_index_value(key: &[u16]) -> Option<u32> {
    // The largest array index, "4294967294", is 10 ASCII digits; any
    // longer key can never be one, and rejecting it here up front keeps
    // the accumulator below comfortably clear of `u64` overflow.
    if key.is_empty() || key.len() > 10 {
        return None;
    }
    if key[0] == 0x30 && key.len() > 1 {
        return None; // a leading zero, e.g. "00" — not the canonical form.
    }
    let mut value: u64 = 0;
    for &unit in key {
        if !(0x30..=0x39).contains(&unit) {
            return None; // not an ASCII digit — a surrogate included.
        }
        value = value * 10 + u64::from(unit - 0x30);
    }
    const MAX_ARRAY_INDEX: u64 = (1u64 << 32) - 2; // 4294967294
    if value <= MAX_ARRAY_INDEX {
        Some(value as u32)
    } else {
        None // e.g. "4294967295" (2^32 - 1): a valid length, never an index.
    }
}

/// Extracts the ECMA-262 `Number::toString` `(s, n)` pair — the shortest
/// round-trip decimal digit string `s` and the decimal exponent `n` such
/// that the value equals `s` (as a `k`-digit integer, `k = s.len()`) times
/// `10^(n-k)` — from `ryu`'s formatted output for a positive finite nonzero
/// `f64`. `ryu` always prints either plain decimal notation (`ddd.ddd`,
/// with a mandatory single `0` on whichever side has no significant digits)
/// or scientific notation (`d[.ddd]e[-]dd`, mantissa in `[1, 10)`, exponent
/// with no leading zeros or explicit `+`) — both are parsed here into the
/// same normalized `(s, n)` form the four ECMA formatting cases share.
fn ryu_shortest_digits(formatted: &str) -> (String, i32) {
    let (mantissa, exp) = match formatted.split_once('e') {
        Some((m, e)) => (
            m,
            e.parse::<i32>().expect("ryu's exponent is a plain integer"),
        ),
        None => (formatted, 0),
    };
    let (int_part, frac_part) = match mantissa.split_once('.') {
        Some((i, f)) => (i, f),
        None => (mantissa, ""),
    };

    if exp != 0 {
        // Scientific notation: int_part is a single nonzero digit, so no
        // leading/trailing-zero stripping is needed — ryu's shortest-form
        // guarantee already excludes a redundant trailing zero here.
        (format!("{int_part}{frac_part}"), exp + 1)
    } else if int_part == "0" {
        // Plain decimal, value in (0, 1): frac_part may have leading zeros
        // ("001" for 0.001) but never a trailing one (shortest form).
        let leading_zeros = frac_part.chars().take_while(|&c| c == '0').count();
        (
            frac_part[leading_zeros..].to_string(),
            -(leading_zeros as i32),
        )
    } else if frac_part.is_empty() || frac_part == "0" {
        // Plain decimal, whole number: strip int_part's OWN trailing zeros
        // to recover the true significant-digit string (e.g. "100.0" has
        // one significant digit, "1", not three) while `n` stays the full
        // digit count before the point.
        (
            int_part.trim_end_matches('0').to_string(),
            int_part.len() as i32,
        )
    } else {
        // Plain decimal with a genuine fractional part; frac_part has no
        // trailing zero (shortest form), so no stripping is needed.
        (format!("{int_part}{frac_part}"), int_part.len() as i32)
    }
}

/// ECMA-262 `Number::toString` (radix 10), which `JSON.stringify` uses
/// verbatim for every numeric value: the shortest decimal digit string `s`
/// (of length `k`) and decimal exponent `n` such that the `f64` value equals
/// `s` (as a `k`-digit integer) times `10^(n-k)`, formatted per four cases
/// on `n`/`k` — plain integer, embedded decimal point, leading-zeros
/// decimal, or exponential notation. `+0`/`-0` both render as `"0"`.
///
/// The shortest round-trip digit string and its decimal exponent come from
/// [`ryu_shortest_digits`] over the `ryu` crate's formatting, NOT Rust's own
/// `{:e}` (`LowerExp`): differential testing against real `JSON.stringify`
/// proved `{:e}` picks the wrong digit at an exact decimal tie
/// (`-86666878403640.625`, where ECMA-262's round-to-even rule requires the
/// even digit `...62`, but `{:e}` gives the odd `...63`) — see the module
/// documentation and `meridian-core/tests/` for the differential corpus that
/// backs this. Only the ECMA CASE-SELECTION and PUNCTUATION rules below are
/// hand-written, not digit generation itself.
fn js_number_to_string(x: f64) -> String {
    if x == 0.0 {
        // Covers BOTH +0.0 and -0.0 (IEEE 754 equality), matching the
        // spec's explicit "if x is +0 or -0, return \"0\"" step — JS never
        // prints "-0" for JSON.stringify(-0).
        return "0".to_string();
    }
    let negative = x < 0.0;
    let x_abs = x.abs();

    let mut buf = ryu::Buffer::new();
    let (digits, n) = ryu_shortest_digits(buf.format(x_abs));
    let k = digits.len() as i32;

    let mut result = String::new();
    if negative {
        result.push('-');
    }

    if n >= k && n <= 21 {
        // Case A: plain integer, digits followed by (n-k) zeros.
        result.push_str(&digits);
        result.extend(std::iter::repeat_n('0', (n - k) as usize));
    } else if n > 0 && n <= 21 {
        // Case B: decimal point embedded within the digit string.
        result.push_str(&digits[..n as usize]);
        result.push('.');
        result.push_str(&digits[n as usize..]);
    } else if n <= 0 && n > -6 {
        // Case C: "0." followed by (-n) leading zeros then the digits.
        result.push_str("0.");
        result.extend(std::iter::repeat_n('0', (-n) as usize));
        result.push_str(&digits);
    } else {
        // Case D: exponential notation.
        if k == 1 {
            result.push_str(&digits);
        } else {
            result.push_str(&digits[..1]);
            result.push('.');
            result.push_str(&digits[1..]);
        }
        result.push('e');
        let exp = n - 1;
        if exp >= 0 {
            result.push('+');
        } else {
            result.push('-');
        }
        result.push_str(&exp.abs().to_string());
    }
    result
}

struct Parser<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl<'a> Parser<'a> {
    fn new(s: &'a str) -> Self {
        Self {
            chars: s.chars().peekable(),
        }
    }

    fn skip_ws(&mut self) {
        while matches!(self.chars.peek(), Some(' ' | '\t' | '\n' | '\r')) {
            self.chars.next();
        }
    }

    fn parse_value(&mut self) -> Option<Json> {
        self.skip_ws();
        match *self.chars.peek()? {
            '{' => self.parse_object(),
            '[' => self.parse_array(),
            '"' => self.parse_string().map(Json::Str),
            't' => self.parse_keyword("true", Json::Bool(true)),
            'f' => self.parse_keyword("false", Json::Bool(false)),
            'n' => self.parse_keyword("null", Json::Null),
            '-' | '0'..='9' => self.parse_number(),
            _ => None,
        }
    }

    fn parse_keyword(&mut self, kw: &str, value: Json) -> Option<Json> {
        for expected in kw.chars() {
            if self.chars.next()? != expected {
                return None;
            }
        }
        Some(value)
    }

    fn parse_object(&mut self) -> Option<Json> {
        self.chars.next();
        let mut entries: Vec<(Vec<u16>, Json)> = Vec::new();
        self.skip_ws();
        if self.chars.peek() == Some(&'}') {
            self.chars.next();
            return Some(Json::Object(entries));
        }
        loop {
            self.skip_ws();
            if self.chars.peek() != Some(&'"') {
                return None;
            }
            // A key stays UTF-16 code units — the same model
            // [`Parser::parse_string`] produces for a string VALUE (see the
            // module documentation) — so a key containing a lone surrogate
            // is represented exactly, not rejected.
            let key = self.parse_string()?;
            self.skip_ws();
            if self.chars.next()? != ':' {
                return None;
            }
            let value = self.parse_value()?;
            // `JSON.parse` semantics: a later duplicate key's value
            // REPLACES the earlier one, collapsing the object to one entry
            // per unique key. Source text with a duplicate key therefore
            // never round-trips back to itself (see `is_canonical`).
            match entries.iter_mut().find(|(k, _)| *k == key) {
                Some(existing) => existing.1 = value,
                None => entries.push((key, value)),
            }
            self.skip_ws();
            match self.chars.next()? {
                ',' => continue,
                '}' => break,
                _ => return None,
            }
        }
        Some(Json::Object(entries))
    }

    fn parse_array(&mut self) -> Option<Json> {
        self.chars.next();
        let mut items = Vec::new();
        self.skip_ws();
        if self.chars.peek() == Some(&']') {
            self.chars.next();
            return Some(Json::Array(items));
        }
        loop {
            let v = self.parse_value()?;
            items.push(v);
            self.skip_ws();
            match self.chars.next()? {
                ',' => continue,
                ']' => break,
                _ => return None,
            }
        }
        Some(Json::Array(items))
    }

    /// Parses a JSON string literal into its UTF-16 CODE UNIT content —
    /// see the module documentation for why this is `Vec<u16>`, not a Rust
    /// `String`. A raw (non-escaped) source character is always a valid
    /// Unicode scalar value (never a lone surrogate), so it is encoded to
    /// one or two code units via [`char::encode_utf16`]; a `\uXXXX` escape
    /// is, per the JSON grammar, EXACTLY one code unit, pushed unconditionally
    /// with no surrogate-range rejection or pairing validation — a lone
    /// high/low surrogate, two adjacent lone surrogates, and a validly
    /// paired high+low surrogate are all accepted uniformly here, matching
    /// `JSON.parse`'s own behaviour.
    fn parse_string(&mut self) -> Option<Vec<u16>> {
        self.chars.next();
        let mut out: Vec<u16> = Vec::new();
        let mut buf = [0u16; 2];
        loop {
            let c = self.chars.next()?;
            match c {
                '"' => return Some(out),
                '\\' => {
                    let esc = self.chars.next()?;
                    match esc {
                        '"' => out.push(0x22),
                        '\\' => out.push(0x5C),
                        '/' => out.push(0x2F),
                        'b' => out.push(0x08),
                        'f' => out.push(0x0C),
                        'n' => out.push(0x0A),
                        'r' => out.push(0x0D),
                        't' => out.push(0x09),
                        'u' => {
                            let cp = self.parse_hex4()?;
                            out.push(cp as u16);
                        }
                        _ => return None,
                    }
                }
                c if (c as u32) < 0x20 => return None,
                c => out.extend_from_slice(c.encode_utf16(&mut buf)),
            }
        }
    }

    fn parse_hex4(&mut self) -> Option<u32> {
        let mut v: u32 = 0;
        for _ in 0..4 {
            let c = self.chars.next()?;
            let d = c.to_digit(16)?;
            v = v * 16 + d;
        }
        Some(v)
    }

    /// The full JSON number grammar: `["-"] int ["." digit+] [("e"|"E")
    /// ["+"|"-"] digit+]`, `int` being `"0"` or a non-zero digit followed by
    /// more digits (no leading zeros). Grammar validity is enforced here;
    /// the VALUE is then obtained from Rust's own correctly-rounding `f64`
    /// parser (`str::parse`), matching what `JSON.parse` — which also
    /// produces the nearest `f64` — would compute for the same text.
    fn parse_number(&mut self) -> Option<Json> {
        let mut text = String::new();
        if self.chars.peek() == Some(&'-') {
            text.push(self.chars.next().unwrap());
        }
        match *self.chars.peek()? {
            '0' => text.push(self.chars.next().unwrap()),
            '1'..='9' => {
                while matches!(self.chars.peek(), Some('0'..='9')) {
                    text.push(self.chars.next().unwrap());
                }
            }
            _ => return None,
        }
        if self.chars.peek() == Some(&'.') {
            text.push(self.chars.next().unwrap());
            if !matches!(self.chars.peek(), Some('0'..='9')) {
                return None;
            }
            while matches!(self.chars.peek(), Some('0'..='9')) {
                text.push(self.chars.next().unwrap());
            }
        }
        if matches!(self.chars.peek(), Some('e' | 'E')) {
            text.push(self.chars.next().unwrap());
            if matches!(self.chars.peek(), Some('+' | '-')) {
                text.push(self.chars.next().unwrap());
            }
            if !matches!(self.chars.peek(), Some('0'..='9')) {
                return None;
            }
            while matches!(self.chars.peek(), Some('0'..='9')) {
                text.push(self.chars.next().unwrap());
            }
        }
        let value: f64 = text.parse().ok()?;
        if !value.is_finite() {
            // An exponent large enough to overflow to +/-infinity can never
            // be canonical: JSON.stringify of a non-finite number is never
            // this (or any) finite numeral's text.
            return None;
        }
        Some(Json::Number(value))
    }
}

fn parse(text: &str) -> Option<Json> {
    let mut p = Parser::new(text);
    let v = p.parse_value()?;
    p.skip_ws();
    if p.chars.next().is_some() {
        return None;
    }
    Some(v)
}

/// Whether `content` is EXACTLY the canonical JSON serialization of its own
/// parsed value — `content == JSON.stringify(canonicalize(JSON.parse(content)))`,
/// `checkContentEnvelope`'s JSON branch (`instance-data-migration.md`
/// §4.1). A value this module cannot parse, or cannot represent with
/// certainty (a number that overflows to infinity), is never treated as
/// canonical.
pub(crate) fn is_canonical(content: &str) -> bool {
    match parse(content) {
        Some(value) => value.to_canonical_string() == content,
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // GENERATED deterministic differential corpus — see
    // `meridian-core/tests/json_number_corpus_reference.rs` for the
    // generator script and provenance.
    include!("json_number_corpus_data.rs");

    #[test]
    fn matches_the_node_reference_for_a_large_deterministic_bit_pattern_corpus() {
        // Direct, exact-string-equality test of `js_number_to_string`
        // itself (not merely round-trip acceptance) against several
        // thousand `f64` values constructed from bit patterns, each with
        // ground truth from real `JSON.stringify`. A "few hand-picked
        // boundary examples" is explicitly NOT what this is: see the
        // corpus data file's header for exactly how the ~6200 entries were
        // chosen (uniformly random bit patterns, the round-to-even tie
        // counterexample and its neighbours, and the power-of-ten
        // boundaries). Exact equality here implies `is_canonical` also
        // accepts `{"n":<expected>}` for every entry, since a value
        // `js_number_to_string` reproduces exactly round-trips trivially —
        // that public-API-level acceptance is additionally verified for
        // every entry in `json_number_corpus_reference.rs`.
        for &(bits, expected) in NUMBER_CORPUS {
            let x = f64::from_bits(bits);
            assert_eq!(
                js_number_to_string(x),
                expected,
                "bits=0x{bits:016x} x={x:e}"
            );
        }
    }

    #[test]
    fn writes_object_keys_in_sorted_order_regardless_of_insertion_order() {
        let v = Json::Object(vec![
            (Json::key("b"), Json::Number(2.0)),
            (Json::key("a"), Json::Number(1.0)),
        ]);
        assert_eq!(v.to_canonical_string(), r#"{"a":1,"b":2}"#);
    }

    #[test]
    fn escapes_quote_and_backslash() {
        assert_eq!(Json::str("a\"b").to_canonical_string(), "\"a\\\"b\"");
        assert_eq!(Json::str("a\\b").to_canonical_string(), "\"a\\\\b\"");
    }

    #[test]
    fn escapes_named_control_characters() {
        assert_eq!(Json::str("\u{8}").to_canonical_string(), "\"\\b\"");
        assert_eq!(Json::str("\t").to_canonical_string(), "\"\\t\"");
        assert_eq!(Json::str("\n").to_canonical_string(), "\"\\n\"");
        assert_eq!(Json::str("\u{c}").to_canonical_string(), "\"\\f\"");
        assert_eq!(Json::str("\r").to_canonical_string(), "\"\\r\"");
    }

    #[test]
    fn escapes_other_control_characters_as_lowercase_u_hex() {
        assert_eq!(Json::str("\u{1}").to_canonical_string(), "\"\\u0001\"");
        assert_eq!(Json::str("\u{1f}").to_canonical_string(), "\"\\u001f\"");
    }

    #[test]
    fn does_not_escape_non_ascii_text() {
        let v = Json::str("héllo — мир");
        assert_eq!(v.to_canonical_string(), "\"héllo — мир\"");
    }

    #[test]
    fn escapes_a_lone_high_surrogate() {
        assert_eq!(Json::Str(vec![0xD800]).to_canonical_string(), "\"\\ud800\"");
    }

    #[test]
    fn escapes_a_lone_low_surrogate() {
        assert_eq!(Json::Str(vec![0xDC00]).to_canonical_string(), "\"\\udc00\"");
    }

    #[test]
    fn combines_a_valid_surrogate_pair_into_the_literal_astral_character() {
        assert_eq!(
            Json::Str(vec![0xD800, 0xDC00]).to_canonical_string(),
            "\"\u{10000}\""
        );
    }

    #[test]
    fn escapes_adjacent_lone_high_surrogates_individually() {
        assert_eq!(
            Json::Str(vec![0xD800, 0xD800]).to_canonical_string(),
            "\"\\ud800\\ud800\""
        );
    }

    #[test]
    fn round_trips_a_document_containing_a_lone_surrogate() {
        // A canonical document containing a lone surrogate parses and
        // re-serializes to EXACTLY itself — the parser must accept the
        // escape (no surrogate-range rejection) and the writer must
        // reproduce the same individual `\u` escape (canonical
        // re-escaping), not reject it or combine it with anything else.
        let canonical = r#"{"a":"\ud800"}"#;
        assert!(is_canonical(canonical));
    }

    #[test]
    fn rejects_a_valid_pair_spelled_as_two_escapes_as_non_canonical() {
        // Valid JSON, and parses to the same string as the literal astral
        // character, but the canonical re-serialization of a valid pair is
        // always the literal character — this text is therefore rejected.
        assert!(!is_canonical(r#"{"a":"\ud800\udc00"}"#));
        assert!(is_canonical("{\"a\":\"\u{10000}\"}"));
    }

    #[test]
    fn rejects_a_valid_pair_key_spelled_as_two_escapes_as_non_canonical() {
        // Same rule as the value case above, applied to a KEY: a valid
        // surrogate pair's canonical re-serialization is always the literal
        // astral character, never the two `\u` escapes.
        assert!(!is_canonical("{\"\\ud800\\udc00\":1}"));
        assert!(is_canonical("{\"\u{10000}\":1}"));
    }

    #[test]
    fn accepts_a_document_with_a_lone_surrogate_in_a_key() {
        // A key uses the same UTF-16 code-unit model as a string value (see
        // the module documentation), so a lone surrogate key is accepted —
        // matching the real `checkContentEnvelope`, confirmed end-to-end
        // against it in `meridian-core/tests/json_canonical_reference.rs`.
        assert!(is_canonical("{\"\\ud800\":1}"));
        assert!(is_canonical("{\"\\udc00\":1}"));
    }

    #[test]
    fn literal_preserves_declared_key_order() {
        let lit = Json::Literal(vec![("z", Json::Number(1.0)), ("a", Json::Number(2.0))]);
        assert_eq!(lit.to_canonical_string(), r#"{"z":1,"a":2}"#);
    }

    #[test]
    fn literal_composes_inside_object_and_array() {
        let v = Json::Object(vec![(
            Json::key("b"),
            Json::Array(vec![Json::Literal(vec![
                ("y", Json::Number(1.0)),
                ("x", Json::Number(2.0)),
            ])]),
        )]);
        assert_eq!(v.to_canonical_string(), r#"{"b":[{"y":1,"x":2}]}"#);
    }

    #[test]
    fn round_trips_a_canonical_document() {
        let canonical = r#"{"a":[1,2,3],"b":{"c":"d","e":null},"f":true}"#;
        assert!(is_canonical(canonical));
    }

    #[test]
    fn rejects_whitespace_as_non_canonical() {
        assert!(!is_canonical(r#"{"a": 1}"#));
        assert!(!is_canonical("{\"a\":1}\n"));
    }

    #[test]
    fn rejects_unsorted_keys_as_non_canonical() {
        assert!(!is_canonical(r#"{"b":1,"a":2}"#));
    }

    #[test]
    fn accepts_a_fractional_number() {
        assert!(is_canonical(r#"{"a":1.5}"#));
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(!is_canonical("{"));
        assert!(!is_canonical(r#"{"a":}"#));
        assert!(!is_canonical(r#"{"a":1,}"#));
    }

    #[test]
    fn rejects_negative_zero_as_non_canonical() {
        assert!(!is_canonical(r#"{"a":-0}"#));
        assert!(!is_canonical(r#"{"a":-0.0}"#));
    }

    #[test]
    fn rejects_a_duplicate_key_as_non_canonical() {
        // JSON.parse collapses {"a":1,"a":2} to {a: 2}; re-serializing
        // gives {"a":2}, which is not the original two-entry text.
        assert!(!is_canonical(r#"{"a":1,"a":2}"#));
    }

    #[test]
    fn accepts_canonical_exponential_notation() {
        assert!(is_canonical(r#"{"a":1e+21}"#));
        assert!(is_canonical(r#"{"a":5e-324}"#));
    }

    #[test]
    fn rejects_non_canonical_exponential_spellings() {
        assert!(!is_canonical(r#"{"a":1E+21}"#)); // uppercase E
        assert!(!is_canonical(r#"{"a":1e21}"#)); // missing explicit "+"
    }

    #[test]
    fn sorts_keys_by_utf16_code_unit_not_by_unicode_scalar_value() {
        // U+10000 (an astral character) encodes as the UTF-16 surrogate
        // pair starting 0xD800, which is LESS than U+E000 as a bare code
        // unit even though U+10000 > U+E000 as a Unicode scalar value —
        // Rust's own `str`/`char` ordering would get this backwards.
        let v = Json::Object(vec![
            (Json::key("\u{E000}"), Json::Number(1.0)),
            (Json::key("\u{10000}"), Json::Number(2.0)),
        ]);
        assert_eq!(v.to_canonical_string(), "{\"\u{10000}\":2,\"\u{E000}\":1}");
    }

    #[test]
    fn sorts_a_lone_surrogate_key_before_a_private_use_area_bmp_key() {
        // 0xD800 (the lone surrogate) is LESS than 0xE000 as a bare code
        // unit, matching real `Object.keys(value).sort()` order — confirmed
        // against the real `checkContentEnvelope` in
        // `meridian-core/tests/json_canonical_reference.rs`.
        let v = Json::Object(vec![
            (Json::key("\u{E000}"), Json::Number(2.0)),
            (vec![0xD800], Json::Number(1.0)),
        ]);
        assert_eq!(v.to_canonical_string(), "{\"\\ud800\":1,\"\u{E000}\":2}");
    }

    #[test]
    fn sorts_a_lone_surrogate_key_before_the_astral_key_sharing_its_high_half() {
        // A lone high surrogate key `[0xD800]` and an astral key encoding to
        // `[0xD800, 0xDC00]` agree on their first code unit; the SHORTER
        // sequence sorts first (the same prefix rule `Vec<u16>`'s own `Ord`
        // and JS UTF-16 string comparison both use).
        let v = Json::Object(vec![
            (Json::key("\u{10000}"), Json::Number(2.0)),
            (vec![0xD800], Json::Number(1.0)),
        ]);
        assert_eq!(v.to_canonical_string(), "{\"\\ud800\":1,\"\u{10000}\":2}");
    }

    #[test]
    fn a_duplicate_lone_surrogate_key_collapses_to_its_last_value() {
        // `JSON.parse` last-value-wins semantics apply to a lone surrogate
        // key exactly like any other key: the duplicate-key source is
        // rejected, but the already-collapsed single-entry form is
        // canonical.
        assert!(!is_canonical("{\"\\ud800\":1,\"\\ud800\":2}"));
        assert!(is_canonical("{\"\\ud800\":2}"));
    }

    #[test]
    fn array_index_value_accepts_zero_and_the_maximum_index() {
        assert_eq!(array_index_value(&Json::key("0")), Some(0));
        assert_eq!(array_index_value(&Json::key("2")), Some(2));
        assert_eq!(array_index_value(&Json::key("10")), Some(10));
        assert_eq!(
            array_index_value(&Json::key("4294967294")),
            Some(4_294_967_294)
        );
    }

    #[test]
    fn array_index_value_rejects_a_leading_zero() {
        assert_eq!(array_index_value(&Json::key("00")), None);
        assert_eq!(array_index_value(&Json::key("01")), None);
    }

    #[test]
    fn array_index_value_rejects_one_past_the_maximum_index() {
        // 2^32 - 1: a valid array LENGTH, never a valid array INDEX.
        assert_eq!(array_index_value(&Json::key("4294967295")), None);
    }

    #[test]
    fn array_index_value_rejects_non_digit_and_surrogate_keys() {
        assert_eq!(array_index_value(&Json::key("a")), None);
        assert_eq!(array_index_value(&Json::key("-1")), None);
        assert_eq!(array_index_value(&Json::key("1.0")), None);
        assert_eq!(array_index_value(&[0xD800]), None);
        assert_eq!(array_index_value(&[]), None);
    }

    #[test]
    fn is_dunder_proto_key_matches_only_the_exact_spelling() {
        assert!(is_dunder_proto_key(&Json::key("__proto__")));
        assert!(!is_dunder_proto_key(&Json::key("constructor")));
        assert!(!is_dunder_proto_key(&Json::key("__proto___")));
        assert!(!is_dunder_proto_key(&Json::key("_proto_")));
        assert!(!is_dunder_proto_key(&[]));
    }

    #[test]
    fn writes_array_index_keys_before_other_keys_in_ascending_numeric_order() {
        // "10" sorts BEFORE "2" under plain UTF-16 order ('1' < '2'), but
        // as array indices 2 < 10 — the writer must use the numeric order,
        // not a single lexicographic pass over every key.
        let v = Json::Object(vec![
            (Json::key("10"), Json::Number(10.0)),
            (Json::key("2"), Json::Number(2.0)),
        ]);
        assert_eq!(v.to_canonical_string(), r#"{"2":2,"10":10}"#);
    }

    #[test]
    fn rejects_array_index_keys_written_in_lexicographic_rather_than_numeric_order() {
        assert!(is_canonical(r#"{"2":2,"10":10}"#));
        assert!(!is_canonical(r#"{"10":10,"2":2}"#));
    }

    #[test]
    fn writes_a_mix_of_index_plain_and_surrogate_keys_in_the_correct_two_tier_order() {
        let v = Json::Object(vec![
            (Json::key("10"), Json::Number(1.0)),
            (Json::key("2"), Json::Number(2.0)),
            (Json::key("a"), Json::Number(3.0)),
            (vec![0xD800], Json::Number(4.0)),
            (Json::key("\u{10000}"), Json::Number(5.0)),
        ]);
        assert_eq!(
            v.to_canonical_string(),
            "{\"2\":2,\"10\":1,\"a\":3,\"\\ud800\":4,\"\u{10000}\":5}"
        );
    }

    #[test]
    fn drops_a_dunder_proto_key_when_writing_an_object() {
        let v = Json::Object(vec![(Json::key("__proto__"), Json::Number(1.0))]);
        assert_eq!(v.to_canonical_string(), "{}");
    }

    #[test]
    fn a_document_with_a_dunder_proto_key_is_never_canonical() {
        // Real `checkContentEnvelope` rejects these: the key is silently
        // dropped by `canonicalize()`'s own `out[k] = ...` assignment (see
        // `is_dunder_proto_key`'s documentation), so the re-serialized form
        // is never the same text — regardless of the value's own type.
        assert!(!is_canonical(r#"{"__proto__":1}"#));
        assert!(!is_canonical(r#"{"a":{"__proto__":1}}"#));
        assert!(!is_canonical(r#"{"__proto__":null}"#));
        assert!(is_canonical("{}"));
    }

    #[test]
    fn a_dunder_proto_key_is_dropped_regardless_of_other_keys_present() {
        let v = Json::Object(vec![
            (Json::key("__proto__"), Json::Number(2.0)),
            (Json::key("a"), Json::Number(1.0)),
        ]);
        assert_eq!(v.to_canonical_string(), r#"{"a":1}"#);
    }

    #[test]
    fn an_ordinary_constructor_key_is_not_dropped() {
        // Unlike "__proto__", "constructor" is an ordinary (non-accessor)
        // inherited property — assigning to it creates a normal own
        // property, so it round-trips like any other key.
        assert!(is_canonical(r#"{"constructor":1}"#));
    }
}
