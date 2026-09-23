//! Computing — never trusting — the claim/assertion/evidence linkage
//! (`evidence-and-handoff-contract.md` §5.1–§5.3,
//! `operating-principles.md`'s `evidence-before-status`).
//!
//! Three separate id-linked lists: a claimed result is supported by the
//! verifiable assertions that name it, an assertion by the evidence that
//! covers it. An assertion a record declares `verified` must be SUPPORTED —
//! some evidence entry that covers it resolved cleanly as an
//! `evidence-result` observing `confirmed` and the transformer confirmed it
//! bears on that very assertion ([`supports`]). A claimed result's status is
//! fully determined by the declared statuses of its linked assertions
//! ([`claimed_result_status`]), so a handoff can only state the computed one.

use core::fmt;

use crate::types::SemanticId;

use super::types::ObservedResult;

/// An assertion's declared status. `Verified` is only accepted when the
/// assertion is supported by resolved evidence; `Unverified` always carries
/// an explicit reason in the record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssertionStatus {
    Verified,
    Unverified,
}

impl AssertionStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            AssertionStatus::Verified => "verified",
            AssertionStatus::Unverified => "unverified",
        }
    }

    pub fn parse(value: &str) -> Option<AssertionStatus> {
        match value {
            "verified" => Some(AssertionStatus::Verified),
            "unverified" => Some(AssertionStatus::Unverified),
            _ => None,
        }
    }
}

/// Whether a claimed result is established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClaimedResultStatus {
    Established,
    NotEstablished,
}

impl ClaimedResultStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            ClaimedResultStatus::Established => "established",
            ClaimedResultStatus::NotEstablished => "not_established",
        }
    }

    pub fn parse(value: &str) -> Option<ClaimedResultStatus> {
        match value {
            "established" => Some(ClaimedResultStatus::Established),
            "not_established" => Some(ClaimedResultStatus::NotEstablished),
            _ => None,
        }
    }
}

impl fmt::Display for ClaimedResultStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A CLEAN resolution of one evidence entry: the transformer's observed
/// result and the assertion ids it confirms the evidence bears on.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResolvedEvidenceResult {
    pub observed_result: ObservedResult,
    pub covers: Vec<SemanticId>,
}

/// Does one evidence entry support `assertion`? Only when the entry itself
/// claims to cover it AND its clean resolution observed `confirmed` AND the
/// transformer confirms it bears on that assertion. An entry's own `covers`
/// is never itself proof, and `contradicted`/`inconclusive` never support.
pub fn supports(
    entry_covers: &[SemanticId],
    resolved: &ResolvedEvidenceResult,
    assertion: &SemanticId,
) -> bool {
    resolved.observed_result == ObservedResult::Confirmed
        && entry_covers.contains(assertion)
        && resolved.covers.contains(assertion)
}

/// A claimed result is `Established` iff it has at least one linked
/// assertion and every linked assertion is `Verified`; `NotEstablished`
/// otherwise — including when no assertion links to it at all.
pub fn claimed_result_status(linked: &[AssertionStatus]) -> ClaimedResultStatus {
    if !linked.is_empty() && linked.iter().all(|s| *s == AssertionStatus::Verified) {
        ClaimedResultStatus::Established
    } else {
        ClaimedResultStatus::NotEstablished
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sid(s: &str) -> SemanticId {
        SemanticId::new(s).unwrap()
    }

    fn resolved(observed: ObservedResult, covers: &[&str]) -> ResolvedEvidenceResult {
        ResolvedEvidenceResult {
            observed_result: observed,
            covers: covers.iter().map(|c| sid(c)).collect(),
        }
    }

    #[test]
    fn evidence_linkage_is_never_reassigned_to_a_foreign_assertion() {
        let r = resolved(ObservedResult::Confirmed, &["assertion-2"]);
        assert!(!supports(&[sid("assertion-1")], &r, &sid("assertion-1")));
        assert!(!supports(&[sid("assertion-2")], &r, &sid("assertion-1")));
        assert!(supports(&[sid("assertion-2")], &r, &sid("assertion-2")));
    }

    #[test]
    fn evidence_linkage_contradicted_and_inconclusive_never_support() {
        for observed in [ObservedResult::Contradicted, ObservedResult::Inconclusive] {
            let r = resolved(observed, &["assertion-1"]);
            assert!(!supports(&[sid("assertion-1")], &r, &sid("assertion-1")));
        }
    }

    #[test]
    fn evidence_linkage_a_claimed_result_is_established_only_by_every_linked_verified_assertion() {
        use AssertionStatus::{Unverified, Verified};
        assert_eq!(
            claimed_result_status(&[]),
            ClaimedResultStatus::NotEstablished
        );
        assert_eq!(
            claimed_result_status(&[Verified, Unverified]),
            ClaimedResultStatus::NotEstablished
        );
        assert_eq!(
            claimed_result_status(&[Verified, Verified]),
            ClaimedResultStatus::Established
        );
    }
}
