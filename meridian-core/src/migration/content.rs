//! The content-preservation rule (`checkContentEnvelope`,
//! `scripts/lib/instance-data-migration.mjs`; `instance-data-migration.md`
//! §4.1) — the ONE place a `content_envelope`'s declared digest is checked
//! against an independently recomputed SHA-256 of the bytes it represents.
//!
//! `envelope_faults` is the rule. Two views read it: the typed
//! [`ContentEnvelope::new`] constructor (a checked value) and
//! [`check_content_envelope`], which renders the faults as the diagnostics a
//! migration target, an exported record and a resolved source-content
//! response each receive. The rule reads [`EnvelopeFields`], not a typed
//! envelope, because an exported record's `payload` and a resolver's
//! response are not schema-closed: any field may be absent or of another
//! JSON kind.

use core::fmt;

use crate::json::{json_form, JsonForm};
use crate::run_contracts::ResponseField;
use crate::types::{ContentDigest, Diagnostic, NonEmptyString};

use super::fail;

/// The encoding of a content envelope's `content`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Encoding {
    Utf8,
    Base64,
}

impl Encoding {
    pub fn as_str(self) -> &'static str {
        match self {
            Encoding::Utf8 => "utf-8",
            Encoding::Base64 => "base64",
        }
    }

    pub fn parse(value: &str) -> Option<Encoding> {
        match value {
            "utf-8" => Some(Encoding::Utf8),
            "base64" => Some(Encoding::Base64),
            _ => None,
        }
    }
}

/// A content envelope as a contract reads it before trusting it: each of
/// `media_type`, `encoding`, `content` and `digest.{algorithm,value}` may be
/// absent or of another JSON kind. A `digest` that is not an object reads
/// as both digest fields absent (`isObject(p.digest) ? p.digest : {}`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvelopeFields {
    pub media_type: ResponseField<String>,
    pub encoding: ResponseField<String>,
    pub content: ResponseField<String>,
    pub digest_algorithm: ResponseField<String>,
    pub digest_value: ResponseField<String>,
}

impl EnvelopeFields {
    /// The fields of a schema-closed envelope, every one present.
    pub fn declared(
        media_type: &str,
        encoding: Encoding,
        content: &str,
        digest: &ContentDigest,
    ) -> Self {
        Self {
            media_type: ResponseField::Present(media_type.to_string()),
            encoding: ResponseField::Present(encoding.as_str().to_string()),
            content: ResponseField::Present(content.to_string()),
            digest_algorithm: ResponseField::Present(digest.algorithm().as_str().to_string()),
            digest_value: ResponseField::Present(digest.value().to_string()),
        }
    }

    /// Field-for-field equality as `checkPayloadPreservation` compares an
    /// exported payload with resolved content: each field `===` the other.
    pub(crate) fn same_bytes(&self, other: &EnvelopeFields) -> bool {
        let eq = |a: &ResponseField<String>, b: &ResponseField<String>| match (a, b) {
            (ResponseField::Absent, ResponseField::Absent) => true,
            (ResponseField::Present(x), ResponseField::Present(y)) => x == y,
            _ => false,
        };
        eq(&self.media_type, &other.media_type)
            && eq(&self.encoding, &other.encoding)
            && eq(&self.content, &other.content)
            && eq(&self.digest_algorithm, &other.digest_algorithm)
            && eq(&self.digest_value, &other.digest_value)
    }
}

/// One way a content envelope fails the rule, in the rule's own order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EnvelopeFault {
    MediaTypeMissing,
    EncodingOutsidePool,
    ContentMissing,
    AlgorithmNotSha256,
    DigestValueMalformed,
    JsonRequiresUtf8,
    JsonUnparseable,
    JsonNotCanonical,
    TextRequiresUtf8,
    BinaryRequiresBase64,
    NotCanonicalBase64,
    DigestMismatch(Encoding),
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn is_json_media_type(media_type: &str) -> bool {
    media_type == "application/json" || media_type.ends_with("+json")
}

/// The rule itself, in `checkContentEnvelope`'s exact control flow: the
/// three presence faults stop the check, the two digest-shape faults do
/// not, and every media-type/encoding fault stops before any byte is
/// hashed.
fn envelope_faults(fields: &EnvelopeFields) -> Vec<EnvelopeFault> {
    let mut faults = Vec::new();
    let Some(media_type) = fields.media_type.text().filter(|m| !m.is_empty()) else {
        return vec![EnvelopeFault::MediaTypeMissing];
    };
    let Some(encoding) = fields.encoding.text().and_then(Encoding::parse) else {
        return vec![EnvelopeFault::EncodingOutsidePool];
    };
    let Some(content) = fields.content.text().filter(|c| !c.is_empty()) else {
        return vec![EnvelopeFault::ContentMissing];
    };
    if fields.digest_algorithm.text() != Some("sha-256") {
        faults.push(EnvelopeFault::AlgorithmNotSha256);
    }
    let declared_value = fields.digest_value.text();
    if !declared_value.is_some_and(is_sha256_hex) {
        faults.push(EnvelopeFault::DigestValueMalformed);
    }

    if is_json_media_type(media_type) {
        if encoding != Encoding::Utf8 {
            faults.push(EnvelopeFault::JsonRequiresUtf8);
            return faults;
        }
        match json_form(content) {
            JsonForm::Unparseable => {
                faults.push(EnvelopeFault::JsonUnparseable);
                return faults;
            }
            JsonForm::NonCanonical => {
                faults.push(EnvelopeFault::JsonNotCanonical);
                return faults;
            }
            JsonForm::Canonical => {}
        }
    } else if media_type.starts_with("text/") {
        if encoding != Encoding::Utf8 {
            faults.push(EnvelopeFault::TextRequiresUtf8);
            return faults;
        }
    } else if encoding != Encoding::Base64 {
        faults.push(EnvelopeFault::BinaryRequiresBase64);
        return faults;
    }

    let actual = match encoding {
        Encoding::Utf8 => ContentDigest::of_str(content),
        Encoding::Base64 => match super::base64::decode_canonical(content) {
            Some(bytes) => ContentDigest::of_bytes(&bytes),
            None => {
                faults.push(EnvelopeFault::NotCanonicalBase64);
                return faults;
            }
        },
    };
    if declared_value.is_some_and(|v| v != actual.value()) {
        faults.push(EnvelopeFault::DigestMismatch(encoding));
    }
    faults
}

fn render(at: &str, fields: &EnvelopeFields, fault: EnvelopeFault) -> String {
    let media_type = fields.media_type.string_or_undefined();
    let encoding = fields.encoding.string_or_undefined();
    match fault {
        EnvelopeFault::MediaTypeMissing => format!("{at} media_type is missing or empty"),
        EnvelopeFault::EncodingOutsidePool => {
            format!("{at} encoding \"{encoding}\" is not one of \"utf-8\"/\"base64\"")
        }
        EnvelopeFault::ContentMissing => format!("{at} content is missing or empty"),
        EnvelopeFault::AlgorithmNotSha256 => format!(
            "{at} digest.algorithm is \"{}\", not \"sha-256\"",
            fields.digest_algorithm.string_or_undefined()
        ),
        EnvelopeFault::DigestValueMalformed => {
            format!("{at} digest.value is not a well-formed SHA-256 hex digest")
        }
        EnvelopeFault::JsonRequiresUtf8 => format!(
            "{at} media_type \"{media_type}\" requires encoding \"utf-8\" (canonical JSON is always UTF-8 text), got \"{encoding}\""
        ),
        EnvelopeFault::JsonUnparseable => format!(
            "{at} content is not valid JSON for media_type \"{media_type}\": the content does not parse as a single JSON value"
        ),
        EnvelopeFault::JsonNotCanonical => format!(
            "{at} content is not in canonical JSON form for media_type \"{media_type}\" (recursively sorted object keys, no incidental whitespace); a structured fragment is represented as canonical JSON, never an equivalent but differently-formatted one"
        ),
        EnvelopeFault::TextRequiresUtf8 => format!(
            "{at} media_type \"{media_type}\" requires encoding \"utf-8\", got \"{encoding}\""
        ),
        EnvelopeFault::BinaryRequiresBase64 => format!(
            "{at} media_type \"{media_type}\" is treated as binary content and requires encoding \"base64\", got \"{encoding}\""
        ),
        EnvelopeFault::NotCanonicalBase64 => format!(
            "{at} content is not well-formed canonical base64 (wrong alphabet/padding, or does not round-trip through decode-then-re-encode)"
        ),
        EnvelopeFault::DigestMismatch(encoding) => format!(
            "{at} digest.value does not match the SHA-256 actually recomputed over the represented bytes ({}); a digest is never trusted on its own — including one that is internally consistent across target, export and an external resolver's response, but simply false (property: content preservation)",
            match encoding {
                Encoding::Utf8 => "the UTF-8 byte sequence of content",
                Encoding::Base64 => "the base64-decoded binary bytes, never the base64 text itself",
            }
        ),
    }
}

/// `checkContentEnvelope(at, payload, problems)`: every fault of `fields`
/// as a diagnostic prefixed by `at`.
pub fn check_content_envelope(at: &str, fields: &EnvelopeFields) -> Vec<Diagnostic> {
    envelope_faults(fields)
        .into_iter()
        .map(|fault| fail(render(at, fields, fault)))
        .collect()
}

/// A content envelope that passed the whole rule: its declared digest IS
/// the SHA-256 of the bytes it represents, and JSON content is canonical.
/// There is no unchecked constructor and no mutable field.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContentEnvelope {
    media_type: NonEmptyString,
    encoding: Encoding,
    content: NonEmptyString,
    digest: ContentDigest,
}

/// The first fault [`ContentEnvelope::new`] met.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentEnvelopeError {
    /// A JSON (`application/json`, `*+json`) or `text/*` media type was
    /// declared with `encoding: base64`.
    TextualMediaTypeRequiresUtf8,
    /// A non-textual media type was declared with `encoding: utf-8`.
    BinaryMediaTypeRequiresBase64,
    /// `encoding: base64` but `content` is not canonical base64.
    NotCanonicalBase64,
    /// A JSON media type whose content does not parse, or is not already
    /// its own canonical serialization.
    NotCanonicalJson,
    /// The declared digest is not the SHA-256 of the represented bytes.
    DigestMismatch,
}

impl fmt::Display for ContentEnvelopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentEnvelopeError::TextualMediaTypeRequiresUtf8 => {
                write!(f, "a JSON or text/* media type requires encoding \"utf-8\"")
            }
            ContentEnvelopeError::BinaryMediaTypeRequiresBase64 => {
                write!(f, "a non-textual media type requires encoding \"base64\"")
            }
            ContentEnvelopeError::NotCanonicalBase64 => write!(
                f,
                "content is not well-formed canonical base64 (wrong alphabet/padding, or does not round-trip through decode-then-re-encode)"
            ),
            ContentEnvelopeError::NotCanonicalJson => write!(
                f,
                "content is not in canonical JSON form (recursively sorted object keys, no incidental whitespace); a structured fragment is represented as canonical JSON, never an equivalent but differently-formatted one"
            ),
            ContentEnvelopeError::DigestMismatch => write!(
                f,
                "digest does not match the SHA-256 actually recomputed over the represented bytes; a digest is never trusted on its own, including one that is internally self-consistent but simply false"
            ),
        }
    }
}

impl std::error::Error for ContentEnvelopeError {}

impl ContentEnvelope {
    /// Applies the whole rule to typed arguments. The presence and
    /// digest-shape faults cannot arise from typed arguments, so the first
    /// remaining fault is the error.
    pub fn new(
        media_type: NonEmptyString,
        encoding: Encoding,
        content: NonEmptyString,
        digest: ContentDigest,
    ) -> Result<Self, ContentEnvelopeError> {
        let fields =
            EnvelopeFields::declared(media_type.as_str(), encoding, content.as_str(), &digest);
        let first = envelope_faults(&fields)
            .into_iter()
            .find_map(|fault| match fault {
                EnvelopeFault::JsonRequiresUtf8 | EnvelopeFault::TextRequiresUtf8 => {
                    Some(ContentEnvelopeError::TextualMediaTypeRequiresUtf8)
                }
                EnvelopeFault::BinaryRequiresBase64 => {
                    Some(ContentEnvelopeError::BinaryMediaTypeRequiresBase64)
                }
                EnvelopeFault::NotCanonicalBase64 => Some(ContentEnvelopeError::NotCanonicalBase64),
                EnvelopeFault::JsonUnparseable | EnvelopeFault::JsonNotCanonical => {
                    Some(ContentEnvelopeError::NotCanonicalJson)
                }
                EnvelopeFault::DigestMismatch(_) => Some(ContentEnvelopeError::DigestMismatch),
                EnvelopeFault::MediaTypeMissing
                | EnvelopeFault::EncodingOutsidePool
                | EnvelopeFault::ContentMissing
                | EnvelopeFault::AlgorithmNotSha256
                | EnvelopeFault::DigestValueMalformed => None,
            });
        match first {
            Some(error) => Err(error),
            None => Ok(Self {
                media_type,
                encoding,
                content,
                digest,
            }),
        }
    }

    pub fn media_type(&self) -> &str {
        self.media_type.as_str()
    }
    pub fn encoding(&self) -> Encoding {
        self.encoding
    }
    pub fn content(&self) -> &str {
        self.content.as_str()
    }
    pub fn digest(&self) -> &ContentDigest {
        &self.digest
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nes(s: &str) -> NonEmptyString {
        NonEmptyString::new(s).unwrap()
    }

    fn loose(media: &str, encoding: &str, content: &str, value: &str) -> EnvelopeFields {
        EnvelopeFields {
            media_type: ResponseField::Present(media.to_string()),
            encoding: ResponseField::Present(encoding.to_string()),
            content: ResponseField::Present(content.to_string()),
            digest_algorithm: ResponseField::Present("sha-256".to_string()),
            digest_value: ResponseField::Present(value.to_string()),
        }
    }

    fn messages(fields: &EnvelopeFields) -> Vec<String> {
        check_content_envelope("at", fields)
            .into_iter()
            .map(|d| d.message().to_string())
            .collect()
    }

    #[test]
    fn a_matching_canonical_json_envelope_has_no_fault() {
        let content = r#"{"a":1}"#;
        let digest = ContentDigest::of_str(content);
        assert!(messages(&loose("application/json", "utf-8", content, digest.value())).is_empty());
        assert!(ContentEnvelope::new(
            nes("application/json"),
            Encoding::Utf8,
            nes(content),
            digest
        )
        .is_ok());
    }

    #[test]
    fn presence_faults_stop_the_rule() {
        let fields = EnvelopeFields::default();
        assert_eq!(messages(&fields), vec!["at media_type is missing or empty"]);
        let mut fields = loose("text/plain", "utf-16", "x", "0");
        assert_eq!(
            messages(&fields),
            vec!["at encoding \"utf-16\" is not one of \"utf-8\"/\"base64\""]
        );
        fields.encoding = ResponseField::Absent;
        assert_eq!(
            messages(&fields),
            vec!["at encoding \"undefined\" is not one of \"utf-8\"/\"base64\""]
        );
    }

    #[test]
    fn digest_shape_faults_do_not_stop_the_rule() {
        let mut fields = loose("text/plain", "utf-8", "x", "not-hex");
        fields.digest_algorithm = ResponseField::Present("md5".to_string());
        let found = messages(&fields);
        assert_eq!(found.len(), 3, "{found:?}");
        assert!(found[0].contains("digest.algorithm is \"md5\""));
        assert!(found[1].contains("digest.value is not a well-formed"));
        assert!(found[2].contains("does not match the SHA-256"));
    }

    #[test]
    fn json_content_is_parsed_then_checked_for_canonical_form() {
        let bad = "not json";
        let found = messages(&loose(
            "application/json",
            "utf-8",
            bad,
            ContentDigest::of_str(bad).value(),
        ));
        assert_eq!(found.len(), 1);
        assert!(found[0].contains("is not valid JSON"));
        let spaced = r#"{"a": 1}"#;
        let found = messages(&loose(
            "application/json",
            "utf-8",
            spaced,
            ContentDigest::of_str(spaced).value(),
        ));
        assert!(found[0].contains("not in canonical JSON form"));
    }

    #[test]
    fn a_false_base64_digest_is_rejected_by_both_views() {
        let err = ContentEnvelope::new(
            nes("application/octet-stream"),
            Encoding::Base64,
            nes("aGVsbG8="),
            ContentDigest::of_str("not hello"),
        )
        .unwrap_err();
        assert_eq!(err, ContentEnvelopeError::DigestMismatch);
        let found = messages(&loose(
            "application/octet-stream",
            "base64",
            "aGVsbG8=",
            ContentDigest::of_str("not hello").value(),
        ));
        assert!(found[0].contains("the base64-decoded binary bytes"));
    }

    #[test]
    fn non_canonical_base64_is_rejected_before_any_digest_comparison() {
        let err = ContentEnvelope::new(
            nes("application/octet-stream"),
            Encoding::Base64,
            nes("/x=="),
            ContentDigest::of_str("x"),
        )
        .unwrap_err();
        assert_eq!(err, ContentEnvelopeError::NotCanonicalBase64);
    }

    #[test]
    fn media_type_and_encoding_must_agree() {
        assert_eq!(
            ContentEnvelope::new(
                nes("application/json"),
                Encoding::Base64,
                nes("{}"),
                ContentDigest::of_str("{}"),
            )
            .unwrap_err(),
            ContentEnvelopeError::TextualMediaTypeRequiresUtf8
        );
        assert_eq!(
            ContentEnvelope::new(
                nes("application/octet-stream"),
                Encoding::Utf8,
                nes("stub"),
                ContentDigest::of_str("stub"),
            )
            .unwrap_err(),
            ContentEnvelopeError::BinaryMediaTypeRequiresBase64
        );
    }
}
