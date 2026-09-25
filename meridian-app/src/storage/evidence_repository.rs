//! [`EvidenceRepository`] — record and resolve evidence
//! (`meridian-rust-target-architecture.md` §3).

use meridian_core::types::EvidenceRef;

use super::error::PortError;
use super::model::{PutEvidenceRequest, StoredEvidence};

/// Records immutable evidence and resolves an [`EvidenceRef`] back to it
/// (`evidence-and-handoff-contract.md` §5.3).
pub trait EvidenceRepository {
    /// Records one piece of evidence.
    ///
    /// Evidence is write-once: a second `put` for an
    /// [`EvidenceRef`] that already exists is rejected as
    /// [`PortError::EvidenceAlreadyExists`], regardless of whether its
    /// content matches the first — this port never trusts a caller's claim
    /// that a stored piece of evidence should now read differently.
    ///
    /// `request.subject()` must name a record [`super::RecordRepository`]
    /// already knows about; otherwise this returns
    /// [`PortError::ReferenceNotFound`] and records nothing.
    fn put(&self, request: PutEvidenceRequest) -> Result<StoredEvidence, PortError>;

    /// Resolves `evidence_ref` back to the evidence it names, or `Ok(None)`
    /// when no such evidence has ever been recorded — genuine absence is
    /// never conflated with a storage failure, which is a distinct `Err`.
    fn resolve(&self, evidence_ref: &EvidenceRef) -> Result<Option<StoredEvidence>, PortError>;
}
