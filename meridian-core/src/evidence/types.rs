//! The pure claim/assertion/evidence structures
//! (`evidence-and-handoff-contract.md` §5), scoped to what package
//! `rust-domain-core` needs: the linkage and confirmation rules, not the
//! full handoff record (`execution_run_ref`, `mandatory_checks`,
//! `acceptance_criteria`, `worktree_disposition`, …), which belongs to a
//! later package once `meridian-app` has the ports (`EvidenceRepository`,
//! `SourceResolver`) this crate deliberately does not know about.

use core::fmt;

use crate::types::{ContentDigest, EvidenceRef, NonEmptyString, Revision, SemanticId};

/// A claimed result's exact edition pin
/// (`evidence-and-handoff-contract.md` §4/§5.3: "closed exact-revision
/// rule" — an exact revision or a SHA-256 digest, never a floating branch).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Pin {
    Revision(Revision),
    Digest(ContentDigest),
}

/// One result a run claims to have produced
/// (`evidence-and-handoff-contract.md` §5.1, `claimed_result`).
///
/// Deliberately carries no `status` field: whether a claimed result is
/// established is never asserted, only computed — see
/// [`super::aggregate::claimed_result_status`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClaimedResult {
    id: SemanticId,
    statement: NonEmptyString,
}

impl ClaimedResult {
    pub fn new(id: SemanticId, statement: NonEmptyString) -> Self {
        Self { id, statement }
    }

    pub fn id(&self) -> &SemanticId {
        &self.id
    }
    pub fn statement(&self) -> &str {
        self.statement.as_str()
    }
}

/// One checkable statement about what is actually established, tied to
/// exactly one declared [`ClaimedResult`] by `claimed_result_id`
/// (`evidence-and-handoff-contract.md` §5.2, `verifiable_assertion`).
///
/// Deliberately carries no `status` field, for the same reason
/// [`ClaimedResult`] does not: verified/unverified is computed from
/// evidence, never declared — see [`super::aggregate::verify_assertion`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VerifiableAssertion {
    id: SemanticId,
    statement: NonEmptyString,
    claimed_result_id: SemanticId,
}

impl VerifiableAssertion {
    pub fn new(id: SemanticId, statement: NonEmptyString, claimed_result_id: SemanticId) -> Self {
        Self {
            id,
            statement,
            claimed_result_id,
        }
    }

    pub fn id(&self) -> &SemanticId {
        &self.id
    }
    pub fn statement(&self) -> &str {
        self.statement.as_str()
    }
    pub fn claimed_result_id(&self) -> &SemanticId {
        &self.claimed_result_id
    }
}

/// One piece of evidence (`evidence-and-handoff-contract.md` §5.3,
/// `evidence_entry`). Pinned by an exact [`Pin`]; `covers` names the
/// assertion ids this evidence CLAIMS to bear on — whether that claim is
/// actually confirmed is a property of the resolved
/// [`super::types::ResolvedEvidenceResult`], not of this struct alone (an
/// evidence entry's own `covers` list is never itself proof — see
/// [`super::aggregate::verify_assertion`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EvidenceEntry {
    id: SemanticId,
    reference: EvidenceRef,
    pin: Pin,
    summary: NonEmptyString,
    covers: Vec<SemanticId>,
    limitations: Vec<NonEmptyString>,
}

impl EvidenceEntry {
    pub fn new(
        id: SemanticId,
        reference: EvidenceRef,
        pin: Pin,
        summary: NonEmptyString,
        covers: Vec<SemanticId>,
        limitations: Vec<NonEmptyString>,
    ) -> Self {
        Self {
            id,
            reference,
            pin,
            summary,
            covers,
            limitations,
        }
    }

    pub fn id(&self) -> &SemanticId {
        &self.id
    }
    pub fn reference(&self) -> &EvidenceRef {
        &self.reference
    }
    pub fn pin(&self) -> &Pin {
        &self.pin
    }
    pub fn summary(&self) -> &str {
        self.summary.as_str()
    }
    pub fn covers(&self) -> &[SemanticId] {
        &self.covers
    }
    pub fn limitations(&self) -> &[NonEmptyString] {
        &self.limitations
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
    pub fn as_str(self) -> &'static str {
        match self {
            ObservedResult::Confirmed => "confirmed",
            ObservedResult::Contradicted => "contradicted",
            ObservedResult::Inconclusive => "inconclusive",
        }
    }
}

impl fmt::Display for ObservedResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The result of resolving one [`EvidenceEntry`] through the external
/// boundary `meridian-core` does not implement
/// (`evidence-and-handoff-contract.md` §12: "the checking function...
/// resolves ... `(pinned_ref) → resolved record | ∅`", supplied here by the
/// caller rather than performed by this crate). `covers` is the
/// TRANSFORMER-CONFIRMED subject of `observed_result` — the one set an
/// assertion's verified status is actually checked against, never the
/// evidence entry's own self-declared `covers`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResolvedEvidenceResult {
    pub observed_result: ObservedResult,
    pub covers: Vec<SemanticId>,
}
