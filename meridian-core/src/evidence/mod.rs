//! The one owner of evidence in Meridian's domain: the claim / assertion /
//! evidence linkage (`evidence-and-handoff-contract.md` §5), one pinned
//! piece of evidence and its external `evidence-result` resolution, and the
//! whole `evidence-and-handoff-contract` ([`handoff`]).
//!
//! - [`types`] — evidence kinds, the observed-result pool, one pinned piece
//!   of evidence;
//! - `resolve` — its shape and pin, and its resolution through the shared
//!   `run_contracts` resolution boundary (used by the handoff and by
//!   `crate::field_evaluation`);
//! - [`linkage`] — computing, never trusting, assertion support and a
//!   claimed result's status;
//! - [`handoff`] — the typed handoff record, its twenty sections and the
//!   accepted [`handoff::Handoff`].
//!
//! Diagnostics are prefix-free; no serde, `serde_json::Value`, JSON Schema,
//! file, process, env or output here.

pub mod handoff;
pub mod linkage;
pub(crate) mod resolve;
pub mod types;

use crate::run_contracts::envelope::RecordFamily;
use crate::types::{Diagnostic, DiagnosticLevel};

pub use linkage::{
    claimed_result_status, supports, AssertionStatus, ClaimedResultStatus, ResolvedEvidenceResult,
};
pub use types::{EvidenceKind, ObservedResult, PinnedEvidence};

/// Every message these modules build starts with a fixed, non-empty record
/// subject (`evidence and handoff "…"`, `field evaluation report "…"`), so
/// the non-blank [`Diagnostic`] invariant holds by construction — an
/// internal invariant, not a property of external data.
pub(crate) fn fail(message: String) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message)
        .expect("every evidence diagnostic starts with a non-empty record subject")
}

/// `<label> "<id>"` — how a diagnostic names its record.
pub(crate) fn record_subject(family: RecordFamily, id: &str) -> String {
    format!("{} \"{id}\"", family.label())
}
