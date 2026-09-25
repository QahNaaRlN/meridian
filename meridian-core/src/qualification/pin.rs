//! Pinned references to composed records and the content digest that pins
//! them (`computeConnectionDigest`, `computeContentDigest` of
//! `scripts/lib/{workspace-compatibility,upgrade-integration}-qualification.mjs`
//! — one projection, one owner).

use crate::migration::resolved::OpenObject;
use crate::run_contracts::{PinSha256, PinnedRecordKind, RecordText, ResponseField};
use crate::types::{ContentDigest, Diagnostic, SemanticId};

use super::fail;

/// `definitions.pinned_ref` of both qualification schemas: a closed
/// reference, never an embedded body. `declared_kind` is the schema's
/// free `non_empty` `record_type`; the kind a slot requires is checked on
/// the RESOLVED record, never trusted from this label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualificationPin {
    pub declared_kind: RecordText,
    pub id: SemanticId,
    pub reference: RecordText,
    pub sha256: PinSha256,
}

impl QualificationPin {
    /// Whether `digest` is exactly the content this pin names.
    pub fn confirms(&self, digest: &ContentDigest) -> bool {
        self.sha256.as_str() == digest.value()
    }
}

/// The fields of a resolved record that identify and pin it: the kind and
/// id it declares, and its top-level members.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedRecord {
    pub record_type: ResponseField<String>,
    pub id: ResponseField<String>,
    pub members: OpenObject,
}

/// The seven members a composed record's content digest covers.
const DIGEST_MEMBERS: [&str; 7] = [
    "id",
    "title",
    "record_type",
    "scope",
    "origin",
    "authority",
    "payload",
];

impl ResolvedRecord {
    /// SHA-256 of the canonical projection of the record's complete
    /// meaningful content (`id`, `title`, `record_type`, `scope`, `origin`,
    /// `authority`, `payload`), insensitive to declaration order.
    pub fn content_digest(&self) -> ContentDigest {
        self.members.pick(&DIGEST_MEMBERS).digest()
    }
}

/// `resolveNamedRecord`'s echo checks: the resolved record must declare
/// the slot's kind and the pinned id. `None` resolution is reported and
/// yields `false`; a wrong echo is reported but the record still counts as
/// resolved, as in the reference.
pub(crate) fn check_resolution(
    at: &str,
    field: &str,
    pin: &QualificationPin,
    resolved: Option<&ResolvedRecord>,
    expected: PinnedRecordKind,
    problems: &mut Vec<Diagnostic>,
) -> bool {
    let reference = pin.reference.as_str();
    let Some(record) = resolved else {
        problems.push(fail(format!(
            "{at} {field} \"{reference}\" does not resolve through the external boundary; an unresolved composed record is never accepted"
        )));
        return false;
    };
    if record.record_type.text() != Some(expected.as_str()) {
        problems.push(fail(format!(
            "{at} {field} \"{reference}\": resolved record_type is \"{}\", not \"{}\"",
            record.record_type.string_or_undefined(),
            expected.as_str()
        )));
    }
    if record.id.text() != Some(pin.id.as_str()) {
        problems.push(fail(format!(
            "{at} {field} \"{reference}\": resolved id \"{}\" does not echo the pinned id \"{}\"",
            record.id.string_or_undefined(),
            pin.id
        )));
    }
    true
}
