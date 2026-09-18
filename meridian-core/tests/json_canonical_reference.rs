//! Reference tests for `ContentEnvelope`'s JSON-canonicality check
//! (`meridian_core::migration::checks`'s `checkContentEnvelope` JSON
//! branch, `instance-data-migration.md` §4.1) against the ACTUAL result of
//! the real, unmodified `checkContentEnvelope`
//! (`scripts/lib/instance-data-migration.mjs`) — not merely Rust's own
//! internal consistency.
//!
//! Every `content` string below was fed to a real Node.js script that
//! constructed the full `content_envelope` payload (`media_type:
//! "application/json"`, the SHA-256 of `content`, `content` itself) and
//! called the real, unmodified `checkContentEnvelope('$', payload,
//! problems)`, then recorded whether `problems` stayed empty (accepted) or
//! not (rejected):
//!
//! ```js
//! import { checkContentEnvelope } from '<MERIDIAN_KERNEL>/scripts/lib/instance-data-migration.mjs';
//! import { createHash } from 'node:crypto';
//! function sha256(s) { return createHash('sha256').update(s, 'utf8').digest('hex'); }
//! function check(content) {
//!   const problems = [];
//!   checkContentEnvelope('$', { media_type: 'application/json', encoding: 'utf-8', content,
//!     digest: { algorithm: 'sha-256', value: sha256(content) } }, problems);
//!   return problems.length === 0;
//! }
//! ```
//!
//! run once against each `content` literal below to produce the
//! accept/reject ground truth each test asserts. The `content` strings
//! themselves are exact literals (not reconstructed by any Rust logic
//! under test), so a reviewer can independently reproduce every result by
//! pasting the same string into that script.
//!
//! The key-order fixture's `content` was additionally obtained by calling
//! Node's OWN `canonicalize` + `JSON.stringify` on `{ "": 1,
//! "\u{10000}": 2 }`, to get the exact canonical text (and therefore key
//! order) the real implementation produces — proving Rust's
//! [`meridian_core`] port picks the SAME order, not merely "some
//! deterministic order". The round-5 lone-surrogate-KEY fixtures below
//! (`accepts_a_lone_high_surrogate_key_matching_the_node_reference` onward)
//! were obtained the same way, against the same real, unmodified
//! `checkContentEnvelope`.

use meridian_core::migration::{ContentEnvelope, ContentEnvelopeError, Encoding};
use meridian_core::types::{ContentDigest, NonEmptyString};

fn accepts(content: &str, digest_hex: &str) {
    let result = ContentEnvelope::new(
        NonEmptyString::new("application/json").unwrap(),
        Encoding::Utf8,
        NonEmptyString::new(content).unwrap(),
        ContentDigest::from_hex(digest_hex).unwrap(),
    );
    assert!(result.is_ok(), "expected {content:?} to be accepted as canonical JSON (matching the real checkContentEnvelope), got {:?}", result.unwrap_err());
}

fn rejects_as_non_canonical(content: &str, digest_hex: &str) {
    let result = ContentEnvelope::new(
        NonEmptyString::new("application/json").unwrap(),
        Encoding::Utf8,
        NonEmptyString::new(content).unwrap(),
        ContentDigest::from_hex(digest_hex).unwrap(),
    );
    assert_eq!(
        result.unwrap_err(),
        ContentEnvelopeError::NotCanonicalJson,
        "expected {content:?} to be rejected as non-canonical JSON (matching the real checkContentEnvelope)"
    );
}

#[test]
fn accepts_a_fractional_number_matching_the_node_reference() {
    accepts(
        r#"{"a":1.5}"#,
        "3b1cb40b22933d83e9242f8ac724f114d812ddd49909917b365375c24b69a455",
    );
}

#[test]
fn accepts_canonical_exponential_notation_matching_the_node_reference() {
    accepts(
        r#"{"a":1e+21}"#,
        "a18fbc0bcef4f91fedea4a6ff70089101462846a93a65795324a14b10cb5c4d9",
    );
}

#[test]
fn rejects_uppercase_e_exponential_matching_the_node_reference() {
    rejects_as_non_canonical(
        r#"{"a":1E+21}"#,
        "1c5fe2823729433c12d05ec279a1d9a9b06b7e33b06124de1b95bacbb6b37bc3",
    );
}

#[test]
fn rejects_exponential_missing_explicit_plus_matching_the_node_reference() {
    rejects_as_non_canonical(
        r#"{"a":1e21}"#,
        "bb9ed33b16ee0cf4aa69d078774a4aa0362740206489d9bef60be130c6e1fa3a",
    );
}

#[test]
fn rejects_a_duplicate_key_matching_the_node_reference() {
    rejects_as_non_canonical(
        r#"{"a":1,"a":2}"#,
        "1c53ee0df7b12fd4d65b976120c7fa6b847dc41dffd7f0331c3237a1ceab1756",
    );
}

#[test]
fn accepts_the_full_digit_form_at_the_1e21_boundary_matching_the_node_reference() {
    // 1e20 is canonicalized to a plain 21-digit integer (n=21=k+20<=21,
    // case A), NOT exponential notation — the boundary is exclusive.
    accepts(
        r#"{"a":100000000000000000000}"#,
        "c125828e7797f96ed628b16510a00360bfb7ea19ea08f003b43c9668f195f087",
    );
}

#[test]
fn accepts_the_decimal_form_at_the_1e_minus_6_boundary_matching_the_node_reference() {
    accepts(
        r#"{"a":0.000001}"#,
        "73316665f7916cb06cb29b316b8987b6cc89a7fea4a4fff0ce661eb6ad1eeffb",
    );
}

#[test]
fn accepts_exponential_just_past_the_1e_minus_6_boundary_matching_the_node_reference() {
    accepts(
        r#"{"a":1e-7}"#,
        "b248271d18d09a840564eafaaeb935f11a4d44d6d90ac5b3baa9714420782816",
    );
}

#[test]
fn accepts_the_minimum_positive_denormal_matching_the_node_reference() {
    accepts(
        r#"{"a":5e-324}"#,
        "7496f25994a875ed4269000497c1e2cccfe55d0a6c2078b65b32eef2700d750a",
    );
}

#[test]
fn accepts_the_maximum_finite_double_matching_the_node_reference() {
    accepts(
        r#"{"a":1.7976931348623157e+308}"#,
        "bf1da90ca7a9fda2ad93c0377d3355e67dfd54ac28852da8b2ba2a6727f1780c",
    );
}

#[test]
fn rejects_negative_zero_integer_matching_the_node_reference() {
    // JSON.parse("-0") is the float -0; JSON.stringify(-0) is "0", not
    // "-0", so the literal "-0" is never canonical.
    rejects_as_non_canonical(
        r#"{"a":-0}"#,
        "dbd681e539ad171ed90bfe0b80b788200b28a669b886b868605c53fb1ee9a962",
    );
}

#[test]
fn rejects_negative_zero_float_matching_the_node_reference() {
    rejects_as_non_canonical(
        r#"{"a":-0.0}"#,
        "952b7dc455870c265da6a6fb15ef60891abf562f0e3327f095914610b3d00811",
    );
}

#[test]
fn accepts_the_classic_0_1_plus_0_2_float_artifact_matching_the_node_reference() {
    accepts(
        r#"{"a":0.30000000000000004}"#,
        "29e5dbf7117c7b85bd446837b66c63ef1020f8eb93226b77db3928ddf09aa353",
    );
}

#[test]
fn accepts_the_round_to_even_tie_boundary_matching_the_node_reference() {
    // -86666878403640.625 is an exact decimal tie between two 16-digit
    // representations; ECMA-262's round-to-even rule (and real
    // JSON.stringify) picks the EVEN "...62", not the odd "...63" that
    // Rust's own `{:e}` formatting picks — this is the minimal counterexample
    // that proved round 3's `{:e}`-based digit source wrong.
    accepts(
        r#"{"n":-86666878403640.62}"#,
        "cc13f18993ca34067b4cf67117cec3aed80f61aa9d273a04e4ba4120ab802f65",
    );
}

#[test]
fn accepts_a_lone_high_surrogate_matching_the_node_reference() {
    accepts(
        "{\"a\":\"\\ud800\"}",
        "89f8ca88ea20e6cd48ed0ab6b731b66377cab0c8fb9a7187ea96ac6813ed24e4",
    );
}

#[test]
fn accepts_a_lone_low_surrogate_matching_the_node_reference() {
    accepts(
        "{\"a\":\"\\udc00\"}",
        "05f326659e7c6bbba06066b7512f1f9b14fd2e821289eb12c3d8ed7e06f2d655",
    );
}

#[test]
fn accepts_a_valid_surrogate_pair_as_the_literal_astral_character_matching_the_node_reference() {
    // The canonical form of a valid high+low pair is the literal astral
    // character (U+10000), never the two \u escapes — see
    // `rejects_a_non_canonical_escaped_surrogate_pair_matching_the_node_reference`.
    accepts(
        "{\"a\":\"\u{10000}\"}",
        "2c31be0b348c3794c46409d17cb7d08ad83417809971fa2e1b0ca412784efee8",
    );
}

#[test]
fn accepts_adjacent_lone_high_surrogates_matching_the_node_reference() {
    accepts(
        "{\"a\":\"\\ud800\\ud800\"}",
        "6abe4ea4fdfd36f7c49c401f785c6c36ff1100a62bc3883bd21ca30ecbf9056e",
    );
}

#[test]
fn rejects_a_non_canonical_escaped_surrogate_pair_matching_the_node_reference() {
    // Valid JSON, and parses to the SAME string as the literal-astral-char
    // form above, but a high+low pair's canonical re-serialization is
    // always the literal character, never the two \u escapes — this is the
    // "canonical re-escaping" case: the writer must recognize the pair and
    // combine it, not echo back individually-escaped surrogates.
    rejects_as_non_canonical(
        "{\"a\":\"\\ud800\\udc00\"}",
        "6c113780818d30f11cb58a6b6aa7cd9cc1912daaebc8b7c07713374e8287c74e",
    );
}

#[test]
fn key_order_for_an_astral_and_a_private_use_area_key_matches_the_node_reference() {
    // Node's own canonicalize()+JSON.stringify() on { "": 1,
    // "\u{10000}": 2 } produces this exact text — the U+10000 key (whose
    // UTF-16 surrogate pair starts 0xD800) sorts BEFORE the U+E000 key
    // (0xE000), the opposite of Unicode-scalar-value order.
    let content = "{\"\u{10000}\":2,\"\u{E000}\":1}";
    accepts(
        content,
        "9d4cdc71dda603c42f9b21d88d0c2ffc31a76cd1bd461d7359406cf169845f1e",
    );
}

// Round 5: object/literal KEYS use the same UTF-16 code-unit model as
// string values (`meridian_core`'s `crate::json::Json::Object` now stores
// `Vec<(Vec<u16>, Json)>`, not `Vec<(String, Json)>`) — a key containing a
// lone surrogate is represented exactly rather than rejected. Ground truth
// for every case below was obtained the same way as the header documents:
// feeding the exact `content` literal to the real, unmodified
// `checkContentEnvelope`.

#[test]
fn accepts_a_lone_high_surrogate_key_matching_the_node_reference() {
    accepts(
        "{\"\\ud800\":1}",
        "02724f63cc02840d4a3eaa7063b83466eb5a385438138fdbdd235c86874d8ff7",
    );
}

#[test]
fn accepts_a_lone_low_surrogate_key_matching_the_node_reference() {
    accepts(
        "{\"\\udc00\":1}",
        "e3667620e5308faa2d8bba085de4fa1fcc2cdd4227354de96d155b3f35f9a929",
    );
}

#[test]
fn accepts_a_valid_surrogate_pair_key_as_the_literal_astral_character_matching_the_node_reference()
{
    accepts(
        "{\"\u{10000}\":1}",
        "6a2ff334d501bc259d61b6ae49a3d92b36da54c55f9faf9062e0ad608af62ce6",
    );
}

#[test]
fn rejects_a_non_canonical_escaped_surrogate_pair_key_matching_the_node_reference() {
    // Same rule as the value case above (`rejects_a_non_canonical_escaped_
    // surrogate_pair_matching_the_node_reference`), applied to a KEY: valid
    // JSON, and parses to the same key as the literal-astral-char form
    // above, but the canonical re-serialization of a valid pair is always
    // the literal character, never the two `\u` escapes.
    rejects_as_non_canonical(
        "{\"\\ud800\\udc00\":1}",
        "be7433baded836f70df574c5e70c054339c79af873c5d5441f7b5e80d7bf2c9c",
    );
}

#[test]
fn rejects_a_duplicate_lone_surrogate_key_matching_the_node_reference() {
    // `JSON.parse` last-value-wins semantics apply to a lone surrogate key
    // exactly like any other key — the two-entry source text can never be
    // canonical (see the collapsed single-entry form accepted below).
    rejects_as_non_canonical(
        "{\"\\ud800\":1,\"\\ud800\":2}",
        "4372f637ccb02603fecc09b6abd29af81349ae7640852847b3d2cd788b7b98eb",
    );
}

#[test]
fn accepts_the_collapsed_form_of_a_duplicate_lone_surrogate_key_matching_the_node_reference() {
    accepts(
        "{\"\\ud800\":2}",
        "5580135c50a82df6ccfdd3c7debefd7f52f5b9e02c0a280e299725c3c1be9efd",
    );
}

#[test]
fn key_order_for_a_lone_surrogate_and_a_plain_bmp_key_matches_the_node_reference() {
    // "a" (0x61) is LESS than 0xD800 (the lone surrogate) as a bare code
    // unit, so it sorts first — the real reference's own canonical order,
    // not merely asserted a priori.
    accepts(
        "{\"a\":2,\"\\ud800\":1}",
        "8010d792a06085ad06638eaa2b7c7bc87237dbe7a550b7b37d99f8a5befda972",
    );
}

#[test]
fn key_order_for_a_lone_surrogate_and_an_astral_key_matches_the_node_reference() {
    // A lone high surrogate key `[0xD800]` and an astral key encoding to
    // `[0xD800, 0xDC00]` agree on their first code unit; the shorter
    // sequence (the lone surrogate) sorts first — the same prefix rule
    // JS's own UTF-16 string comparison uses.
    accepts(
        "{\"\\ud800\":1,\"\u{10000}\":2}",
        "e4a675a17c5b9bbd13b9520a363f5d47e4b842551e1029c885f303e62315d85b",
    );
}

#[test]
fn key_order_for_a_lone_surrogate_and_a_private_use_area_key_matches_the_node_reference() {
    // 0xD800 (the lone surrogate) is LESS than 0xE000 as a bare code unit,
    // matching the astral-vs-PUA ordering proven above for values.
    accepts(
        "{\"\\ud800\":1,\"\u{E000}\":2}",
        "471a5b08a3005d5de84e554599660bc7922c8c7aebfe16684cc1354c6150f2ad",
    );
}

// Round 6: `JSON.stringify`'s own key enumeration (`OrdinaryOwnPropertyKeys`)
// lists array-index keys FIRST, in ascending numeric order, before any
// other string key — a single UTF-16 lexicographic sort over every key
// (round 5's implementation) gets this backwards for `"2"` vs `"10"`. And
// `canonicalize()`'s own `out[k] = ...` assignment silently drops a
// `"__proto__"` key instead of copying it, because
// `Object.prototype.__proto__` is an inherited ACCESSOR, not an own data
// property. Ground truth for every case below was obtained the same way
// as the header documents: feeding the exact `content` literal to the
// real, unmodified `checkContentEnvelope`.

#[test]
fn accepts_array_index_keys_in_ascending_numeric_order_matching_the_node_reference() {
    accepts(
        r#"{"2":2,"10":10}"#,
        "2bec7b02bc0c52d3d0f2e8626a26f5d34675af1c4a0eb6def39c145e9c28f61f",
    );
}

#[test]
fn rejects_array_index_keys_in_lexicographic_order_matching_the_node_reference() {
    // Same two keys as above, written in plain-UTF16-lexicographic order
    // ("10" before "2") instead of ascending numeric order — rejected,
    // proving the real reference does NOT use a single lexicographic sort.
    rejects_as_non_canonical(
        r#"{"10":10,"2":2}"#,
        "5355e5c9c48ddb40379a8a11e74e08cf4c2a364f10bf4b2324e32d5d20489385",
    );
}

#[test]
fn accepts_the_zero_array_index_key_matching_the_node_reference() {
    accepts(
        r#"{"0":1}"#,
        "7adc61a4b6a44039bf7635dbffef81ff8cb6bb5cbd8cc67bc3a55eec93c94e81",
    );
}

#[test]
fn accepts_a_leading_zero_key_as_an_ordinary_non_index_key_matching_the_node_reference() {
    // "00" denotes the same integer as "0" but is NOT the canonical index
    // string for it, so it is treated as an ordinary (non-index) key —
    // this single-key object is trivially canonical either way; the
    // distinction only has a visible effect with a second key present
    // (see the mixed-key-kinds case below).
    accepts(
        r#"{"00":1}"#,
        "8ea937844167476541176885caac8e7079ac06cc9ab7aa32e66402b93368a754",
    );
}

#[test]
fn accepts_the_maximum_array_index_key_matching_the_node_reference() {
    accepts(
        r#"{"4294967294":1}"#,
        "fe7d8b03a3a1eefd2735d4c63301b465306f0704eea6c466bba5cf869188fcc9",
    );
}

#[test]
fn accepts_one_past_the_maximum_array_index_as_an_ordinary_key_matching_the_node_reference() {
    // 2^32 - 1 is a valid array LENGTH but never a valid array INDEX, so
    // "4294967295" is an ordinary (non-index) key — again trivially
    // canonical alone; see the mixed-key-kinds case for the ordering
    // consequence.
    accepts(
        r#"{"4294967295":1}"#,
        "f6f35f4a404aa7628ebfe922dabca447eadb0b293217dacf3b22db92020bb4e4",
    );
}

#[test]
fn key_order_for_a_mix_of_index_plain_and_surrogate_keys_matches_the_node_reference() {
    // Canonical order (from the real reference): "2","10" (array indices,
    // ascending numeric) then "a","\ud800","\u{10000}" (ordinary keys,
    // UTF-16 lexicographic).
    accepts(
        "{\"2\":2,\"10\":1,\"a\":3,\"\\ud800\":4,\"\u{10000}\":5}",
        "ec0d7bca44d314155ea712ad389a3aa8338c0183eead96bf499eda835f9997f1",
    );
}

#[test]
fn rejects_the_same_mixed_keys_in_a_non_canonical_declaration_order_matching_the_node_reference() {
    // Same five keys/values as above, declared in a different (also
    // non-canonical) order — neither insertion order nor plain UTF-16
    // order, reinforcing that only the two-tier index/UTF-16 order above
    // is accepted.
    rejects_as_non_canonical(
        "{\"10\":1,\"2\":2,\"a\":3,\"\\ud800\":4,\"\u{10000}\":5}",
        "5e78dfafc86eb7144c3b45154759a13a0d8b82027c265677d9cf30b1d9c3ea5e",
    );
}

#[test]
fn rejects_a_dunder_proto_key_matching_the_node_reference() {
    // `canonicalize()`'s `out["__proto__"] = canonicalize(1)` invokes
    // `Object.prototype.__proto__`'s inherited SETTER instead of creating
    // an own property, so this canonicalizes to `{}` — never the same
    // text as the original.
    rejects_as_non_canonical(
        r#"{"__proto__":1}"#,
        "5a01b4879e11f6261f39c2f190ffde6edb6b012c42064d68312ee2f6eaf1957a",
    );
}

#[test]
fn rejects_a_nested_dunder_proto_key_matching_the_node_reference() {
    rejects_as_non_canonical(
        r#"{"a":{"__proto__":1}}"#,
        "7c4a5c5e46b84251787de606207bbb0bee77180c5888d8ea2fdda7db355059d4",
    );
}

#[test]
fn rejects_a_dunder_proto_key_with_a_null_value_matching_the_node_reference() {
    // The inherited setter DOES run for a `null` value (it changes `out`'s
    // own `[[Prototype]]` to `null`) — but this STILL never creates an own
    // `"__proto__"` property, so the result is the same rejection as the
    // number-valued case above.
    rejects_as_non_canonical(
        r#"{"__proto__":null}"#,
        "9adcac7fbd98b3edf2b71884c8a55162d660648a5a2955a0ca776f07aebe0dc4",
    );
}

#[test]
fn accepts_an_ordinary_constructor_key_matching_the_node_reference() {
    // "constructor" is also `Object.prototype`-inherited, but as a normal
    // (non-accessor) data property — assigning to it creates an own
    // property like any other key, so it round-trips normally, unlike
    // "__proto__" above.
    accepts(
        r#"{"constructor":1}"#,
        "6eaeb60054235b10b94336919c168d26c45156f5ef8d9f0c21a0fbe9b0eae3b3",
    );
}
