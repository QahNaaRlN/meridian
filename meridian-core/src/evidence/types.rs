//! The evidence vocabulary both evidence-bearing contracts share
//! (`evidence-and-handoff-contract.md` §5, `field-evaluation.schema.json`'s
//! `evidence_entry`): the closed evidence kinds, the closed observed-result
//! pool, and one pinned piece of evidence.
//!
//! A piece of evidence is pinned by the closed exact-revision rule
//! (`run_contracts::revision`) — an exact revision or a SHA-256 digest,
//! never a floating branch — and is resolved OUTSIDE the record to an
//! `evidence-result` transformer response (`super::resolve`). Whether it
//! confirms anything is a property of that resolution, never of the
//! record's own words.

use core::fmt;

use crate::run_contracts::{PinSha256, PinnedRecordKind, PinnedRef, PortableRef, RecordText};
use crate::types::SemanticId;

/// The closed pool of evidence kinds. `specialised-evidence-record` exists
/// only in the handoff contract; the field-evaluation schema excludes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvidenceKind {
    CheckRun,
    Observation,
    ArtifactInspection,
    ExternalConfirmation,
    SpecialisedEvidenceRecord,
}

impl EvidenceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            EvidenceKind::CheckRun => "check-run",
            EvidenceKind::Observation => "observation",
            EvidenceKind::ArtifactInspection => "artifact-inspection",
            EvidenceKind::ExternalConfirmation => "external-confirmation",
            EvidenceKind::SpecialisedEvidenceRecord => "specialised-evidence-record",
        }
    }

    pub fn parse(value: &str) -> Option<EvidenceKind> {
        match value {
            "check-run" => Some(EvidenceKind::CheckRun),
            "observation" => Some(EvidenceKind::Observation),
            "artifact-inspection" => Some(EvidenceKind::ArtifactInspection),
            "external-confirmation" => Some(EvidenceKind::ExternalConfirmation),
            "specialised-evidence-record" => Some(EvidenceKind::SpecialisedEvidenceRecord),
            _ => None,
        }
    }
}

impl fmt::Display for EvidenceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The closed pool an evidence result observes
/// (`evidence-and-handoff-contract.md` §5.3): `confirmed`, `contradicted`
/// and `inconclusive` are three distinct states, never merged with each
/// other or with [`crate::types::Verdict`] (a different contract's
/// differently-shaped closed set).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObservedResult {
    Confirmed,
    Contradicted,
    Inconclusive,
}

impl ObservedResult {
    /// The pool, in the contract's order.
    pub const ALL: [ObservedResult; 3] = [
        ObservedResult::Confirmed,
        ObservedResult::Contradicted,
        ObservedResult::Inconclusive,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            ObservedResult::Confirmed => "confirmed",
            ObservedResult::Contradicted => "contradicted",
            ObservedResult::Inconclusive => "inconclusive",
        }
    }

    pub fn parse(value: &str) -> Option<ObservedResult> {
        Self::ALL.into_iter().find(|r| r.as_str() == value)
    }
}

impl fmt::Display for ObservedResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One pinned piece of evidence as a record states it — the fields both
/// contracts' `evidence_entry` share.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedEvidence {
    pub id: SemanticId,
    pub kind: EvidenceKind,
    pub reference: PortableRef,
    pub revision: Option<RecordText>,
    pub sha256: Option<PinSha256>,
    pub summary: RecordText,
}

impl PinnedEvidence {
    /// The `evidence-result` reference this evidence is resolved through
    /// (`resolveEvidenceEntry`'s synthetic reference).
    pub(crate) fn result_pin(&self) -> PinnedRef {
        PinnedRef {
            record_type: PinnedRecordKind::EvidenceResult,
            id: self.id.clone(),
            run_id: None,
            reference: self.reference.clone(),
            revision: self.revision.clone(),
            sha256: self.sha256.clone(),
        }
    }
}
