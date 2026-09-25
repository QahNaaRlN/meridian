//! Acceptance tests for `meridian_core::evidence`'s public linkage rules:
//! evidence cannot be reassigned to a foreign assertion,
//! `contradicted`/`inconclusive` never support an assertion, and a claimed
//! result's status is computed from its assertions, never asserted.

use meridian_core::evidence::*;
use meridian_core::types::SemanticId;

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
fn evidence_an_assertion_the_entry_does_not_cover_is_not_supported() {
    let r = resolved(ObservedResult::Confirmed, &["assertion-1"]);
    assert!(!supports(&[], &r, &sid("assertion-1")));
}

#[test]
fn evidence_cannot_be_reassigned_to_a_foreign_assertion() {
    // The entry CLAIMS to cover assertion-1, but the transformer only
    // confirms it bears on assertion-2 — the record's own say-so is not proof.
    let r = resolved(ObservedResult::Confirmed, &["assertion-2"]);
    assert!(!supports(&[sid("assertion-1")], &r, &sid("assertion-1")));
}

#[test]
fn evidence_a_confirmed_and_transformer_bound_entry_supports_the_assertion() {
    let r = resolved(ObservedResult::Confirmed, &["assertion-1"]);
    assert!(supports(&[sid("assertion-1")], &r, &sid("assertion-1")));
}

#[test]
fn evidence_contradicted_and_inconclusive_never_support() {
    for observed in [ObservedResult::Contradicted, ObservedResult::Inconclusive] {
        let r = resolved(observed, &["assertion-1"]);
        assert!(!supports(&[sid("assertion-1")], &r, &sid("assertion-1")));
    }
}

#[test]
fn evidence_a_claimed_result_with_no_linked_assertion_is_never_established() {
    assert_eq!(
        claimed_result_status(&[]),
        ClaimedResultStatus::NotEstablished
    );
}

#[test]
fn evidence_a_claimed_result_is_established_only_when_every_linked_assertion_is_verified() {
    assert_eq!(
        claimed_result_status(&[AssertionStatus::Verified, AssertionStatus::Unverified]),
        ClaimedResultStatus::NotEstablished
    );
    assert_eq!(
        claimed_result_status(&[AssertionStatus::Verified, AssertionStatus::Verified]),
        ClaimedResultStatus::Established
    );
}

#[test]
fn evidence_the_closed_pools_round_trip() {
    for r in ObservedResult::ALL {
        assert_eq!(ObservedResult::parse(r.as_str()), Some(r));
    }
    assert_eq!(
        EvidenceKind::parse("specialised-evidence-record"),
        Some(EvidenceKind::SpecialisedEvidenceRecord)
    );
    assert_eq!(ObservedResult::parse("verified"), None);
}
