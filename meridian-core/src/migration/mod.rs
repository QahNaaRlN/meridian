//! The migration owner: instance-data migration plans ([`plan`]) and the
//! canonical export that proves one ([`export`]) —
//! `standards/workspace/instance-data-migration.md`,
//! `scripts/lib/instance-data-migration.mjs`.
//!
//! Every contract here takes typed, schema-shaped input the app built from
//! a schema-clean document, and the external facts it is checked against
//! as typed resolver catalogues ([`resolved`]) the app parsed once.
//! `meridian-core` never resolves anything itself. Each check returns every
//! problem in the reference order as a prefix-free [`Diagnostic`] and an
//! accepted record only when none of the problems is that record's own.
//!
//! One owner per rule: the content-preservation rule ([`content`]), the
//! portable-ref rule (`opaque`), the canonical projections behind every
//! fingerprint, digest and key (`projection`) and the resolved-scope
//! comparison live here once and are reused by both contracts and by the
//! qualifications that compose them (`crate::qualification`).

mod base64;
pub mod content;
pub mod export;
mod opaque;
pub mod plan;
pub(crate) mod projection;
pub mod record;
pub mod resolved;

mod owner_decision;

use crate::types::{Diagnostic, DiagnosticLevel};

pub use content::{
    check_content_envelope, ContentEnvelope, ContentEnvelopeError, Encoding, EnvelopeFields,
};
pub use owner_decision::OwnerDecision;
pub use record::RecordHead;

/// Static invariant: every message of this module family is a `format!`
/// whose template carries fixed non-whitespace text, so it is never empty
/// or blank and `Diagnostic::new` cannot refuse it.
pub(crate) fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message)
        .expect("every migration message carries fixed non-whitespace template text")
}
