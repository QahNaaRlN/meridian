//! [`CanonicalJson`] — the one owner of Meridian's canonical-JSON text
//! (`JSON.stringify(canonicalize(value))` of `scripts/lib/*.mjs`) for
//! content a contract leaves OPEN and therefore cannot type any further: an
//! exported record's `payload` (the envelope schema says only `"type":
//! "object"`), a resolver response's target record, and the projection of a
//! resolved composed record a qualification pins by its content digest.
//!
//! This is not a transport value: it has no parser, no navigation and no
//! accessor. A caller builds it once from an already-parsed document (the
//! app's transport boundary does so from its own JSON reader) and can only
//! compare it, render its canonical text or hash it. The writer is the
//! crate's private canonical writer (`crate::json`), so key order, number
//! formatting and string escaping are exactly the ones `plan_fingerprint`
//! already uses — one rule, not a second copy.
//!
//! `locale_order` is the one owner of the `String#localeCompare` order the
//! Node reference sorts semantic ids and mapping keys with before it
//! canonicalizes an array (`computePlanFingerprint`, `computeExportDigest`).

use core::cmp::Ordering;

use crate::json::Json;
use crate::types::ContentDigest;

/// A JSON value in canonical form: object keys are written in the
/// `canonicalize()` + `JSON.stringify` order, numbers in ECMAScript
/// `Number::toString` form. Opaque by design (see the module documentation).
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalJson(Json);

impl CanonicalJson {
    pub fn null() -> Self {
        Self(Json::Null)
    }

    pub fn boolean(value: bool) -> Self {
        Self(Json::Bool(value))
    }

    /// `None` for a non-finite value, which is never a JSON number.
    pub fn number(value: f64) -> Option<Self> {
        value.is_finite().then_some(Self(Json::Number(value)))
    }

    pub fn string(value: &str) -> Self {
        Self(Json::str(value))
    }

    pub fn array(items: Vec<CanonicalJson>) -> Self {
        Self(Json::Array(items.into_iter().map(|item| item.0).collect()))
    }

    /// An object from its members. A repeated key keeps its LAST value, as
    /// `JSON.parse` does.
    pub fn object(members: Vec<(String, CanonicalJson)>) -> Self {
        let mut entries: Vec<(Vec<u16>, Json)> = Vec::with_capacity(members.len());
        for (key, value) in members {
            let key = Json::key(&key);
            match entries.iter_mut().find(|(k, _)| *k == key) {
                Some(slot) => slot.1 = value.0,
                None => entries.push((key, value.0)),
            }
        }
        Self(Json::Object(entries))
    }

    /// `JSON.stringify(canonicalize(value))`.
    pub fn canonical_text(&self) -> String {
        self.0.to_canonical_string()
    }

    /// SHA-256 over the UTF-8 bytes of [`Self::canonical_text`] —
    /// `createHash('sha256').update(JSON.stringify(canonicalize(value)))`.
    pub fn digest(&self) -> ContentDigest {
        ContentDigest::of_str(&self.canonical_text())
    }

    pub(crate) fn into_json(self) -> Json {
        self.0
    }

    pub(crate) fn from_json(json: Json) -> Self {
        Self(json)
    }
}

/// The primary collation weight of one character of the closed alphabet
/// semantic ids and the `unit_id::target_id` mapping key are drawn from, in
/// the ICU root order `localeCompare` uses: punctuation (`-` before `:`),
/// then digits, then letters.
fn collation_weight(c: char) -> Option<u32> {
    match c {
        '-' => Some(0),
        ':' => Some(1),
        '0'..='9' => Some(10 + (c as u32 - '0' as u32)),
        'a'..='z' => Some(100 + (c as u32 - 'a' as u32)),
        _ => None,
    }
}

/// `a.localeCompare(b)` for the strings the Node reference sorts before
/// canonicalizing: semantic ids and `${unit_id}::${target_id}` keys, whose
/// characters are all in `[-:0-9a-z]`. ICU root collation does NOT order
/// those by code unit (`"a:"` sorts before `"a0"`, so `"unit-1::t"` sorts
/// before `"unit-10::t"`); this reproduces its primary order for that
/// alphabet exactly. A string with any other character (never produced by
/// a schema-valid semantic id) falls back to code-unit order, so the order
/// stays total and deterministic.
pub(crate) fn locale_order(a: &str, b: &str) -> Ordering {
    let weights = |s: &str| {
        s.chars()
            .map(collation_weight)
            .collect::<Option<Vec<u32>>>()
    };
    match (weights(a), weights(b)) {
        (Some(wa), Some(wb)) => wa.cmp(&wb).then_with(|| a.cmp(b)),
        _ => a.encode_utf16().cmp(b.encode_utf16()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_keys_are_written_in_canonical_order_and_the_last_duplicate_wins() {
        let value = CanonicalJson::object(vec![
            ("b".to_string(), CanonicalJson::boolean(true)),
            ("a".to_string(), CanonicalJson::string("x")),
            ("b".to_string(), CanonicalJson::null()),
        ]);
        assert_eq!(value.canonical_text(), r#"{"a":"x","b":null}"#);
    }

    #[test]
    fn numbers_use_the_ecmascript_form_and_non_finite_values_are_refused() {
        let value = CanonicalJson::array(vec![
            CanonicalJson::number(1.0).unwrap(),
            CanonicalJson::number(0.5).unwrap(),
        ]);
        assert_eq!(value.canonical_text(), "[1,0.5]");
        assert!(CanonicalJson::number(f64::NAN).is_none());
        assert!(CanonicalJson::number(f64::INFINITY).is_none());
    }

    /// Reference order obtained from Node's own
    /// `[...].sort((a, b) => a.localeCompare(b))` over the same strings.
    #[test]
    fn locale_order_matches_the_node_reference_for_the_semantic_id_alphabet() {
        let mut keys = vec![
            "unit-10::t",
            "unit-1::t",
            "unit-1a::t",
            "unit-a::t",
            "unit::t",
            "u-::t",
            "unit-1::",
            "a-b",
            "a0",
            "a:b",
            "ab",
        ];
        keys.sort_by(|a, b| locale_order(a, b));
        assert_eq!(
            keys,
            vec![
                "a-b",
                "a:b",
                "a0",
                "ab",
                "u-::t",
                "unit-1::",
                "unit-1::t",
                "unit-10::t",
                "unit-1a::t",
                "unit-a::t",
                "unit::t",
            ]
        );
    }
}
