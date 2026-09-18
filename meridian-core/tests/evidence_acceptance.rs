//! Acceptance tests for `meridian_core::evidence`: evidence cannot be
//! reassigned to a foreign assertion, `contradicted`/`inconclusive` never
//! verify an assertion, and a claimed result's status is computed from its
//! assertions, never asserted directly.

use std::collections::HashMap;

use meridian_core::evidence::*;
use meridian_core::types::{ContentDigest, EvidenceRef, NonEmptyString, SemanticId};

fn sid(s: &str) -> SemanticId {
    SemanticId::new(s).unwrap()
}
fn nes(s: &str) -> NonEmptyString {
    NonEmptyString::new(s).unwrap()
}
fn digest_pin() -> Pin {
    Pin::Digest(ContentDigest::of_str("evidence content"))
}

fn evidence_entry(id: &str, covers: Vec<&str>) -> EvidenceEntry {
    EvidenceEntry::new(
        sid(id),
        EvidenceRef::new(format!("evidence:{id}")).unwrap(),
        digest_pin(),
        nes("observed the check run"),
        covers.into_iter().map(sid).collect(),
        vec![],
    )
}

#[test]
fn an_assertion_with_no_covering_evidence_is_unverified() {
    let assertion =
        VerifiableAssertion::new(sid("assertion-1"), nes("tests pass"), sid("result-1"));
    let verdict = verify_assertion(&assertion, &[], &HashMap::new());
    assert!(matches!(verdict, AssertionVerdict::Unverified { .. }));
}

#[test]
fn evidence_cannot_be_reassigned_to_a_foreign_assertion() {
    // The evidence entry CLAIMS to cover assertion-1 (its own `covers`
    // list), but the externally resolved evidence-result only confirms it
    // covers assertion-2 — the handoff's own say-so is not proof.
    let assertion_1 =
        VerifiableAssertion::new(sid("assertion-1"), nes("tests pass"), sid("result-1"));
    let evidence = vec![evidence_entry("evidence-1", vec!["assertion-1"])];
    let mut resolved = HashMap::new();
    resolved.insert(
        sid("evidence-1"),
        ResolvedEvidenceResult {
            observed_result: ObservedResult::Confirmed,
            covers: vec![sid("assertion-2")],
        },
    );
    let verdict = verify_assertion(&assertion_1, &evidence, &resolved);
    assert!(matches!(verdict, AssertionVerdict::Unverified { .. }));
}

#[test]
fn a_confirmed_and_transformer_confirmed_coverage_verifies_the_assertion() {
    let assertion_1 =
        VerifiableAssertion::new(sid("assertion-1"), nes("tests pass"), sid("result-1"));
    let evidence = vec![evidence_entry("evidence-1", vec!["assertion-1"])];
    let mut resolved = HashMap::new();
    resolved.insert(
        sid("evidence-1"),
        ResolvedEvidenceResult {
            observed_result: ObservedResult::Confirmed,
            covers: vec![sid("assertion-1")],
        },
    );
    let verdict = verify_assertion(&assertion_1, &evidence, &resolved);
    assert_eq!(verdict, AssertionVerdict::Verified);
}

#[test]
fn contradicted_never_verifies_an_assertion() {
    let assertion_1 =
        VerifiableAssertion::new(sid("assertion-1"), nes("tests pass"), sid("result-1"));
    let evidence = vec![evidence_entry("evidence-1", vec!["assertion-1"])];
    let mut resolved = HashMap::new();
    resolved.insert(
        sid("evidence-1"),
        ResolvedEvidenceResult {
            observed_result: ObservedResult::Contradicted,
            covers: vec![sid("assertion-1")],
        },
    );
    let verdict = verify_assertion(&assertion_1, &evidence, &resolved);
    assert!(matches!(verdict, AssertionVerdict::Unverified { .. }));
}

#[test]
fn inconclusive_never_verifies_an_assertion() {
    let assertion_1 =
        VerifiableAssertion::new(sid("assertion-1"), nes("tests pass"), sid("result-1"));
    let evidence = vec![evidence_entry("evidence-1", vec!["assertion-1"])];
    let mut resolved = HashMap::new();
    resolved.insert(
        sid("evidence-1"),
        ResolvedEvidenceResult {
            observed_result: ObservedResult::Inconclusive,
            covers: vec![sid("assertion-1")],
        },
    );
    let verdict = verify_assertion(&assertion_1, &evidence, &resolved);
    assert!(matches!(verdict, AssertionVerdict::Unverified { .. }));
}

#[test]
fn a_claimed_result_with_no_linked_assertion_is_never_established() {
    let claimed = ClaimedResult::new(sid("result-1"), nes("the migration succeeded"));
    let status = claimed_result_status(&claimed, &[], &HashMap::new());
    assert_eq!(status, ClaimedResultStatus::NotEstablished);
}

#[test]
fn a_claimed_result_is_established_only_when_every_linked_assertion_is_verified() {
    let claimed = ClaimedResult::new(sid("result-1"), nes("the migration succeeded"));
    let a1 = VerifiableAssertion::new(
        sid("assertion-1"),
        nes("coverage is complete"),
        sid("result-1"),
    );
    let a2 = VerifiableAssertion::new(sid("assertion-2"), nes("rollback works"), sid("result-1"));
    let mut verdicts = HashMap::new();
    verdicts.insert(sid("assertion-1"), AssertionVerdict::Verified);
    verdicts.insert(
        sid("assertion-2"),
        AssertionVerdict::Unverified {
            reason: "not yet run".to_string(),
        },
    );

    let status = claimed_result_status(&claimed, &[a1, a2], &verdicts);
    assert_eq!(status, ClaimedResultStatus::NotEstablished);
}

#[test]
fn a_claimed_result_is_established_when_all_linked_assertions_are_verified() {
    let claimed = ClaimedResult::new(sid("result-1"), nes("the migration succeeded"));
    let a1 = VerifiableAssertion::new(
        sid("assertion-1"),
        nes("coverage is complete"),
        sid("result-1"),
    );
    let a2 = VerifiableAssertion::new(sid("assertion-2"), nes("rollback works"), sid("result-1"));
    let mut verdicts = HashMap::new();
    verdicts.insert(sid("assertion-1"), AssertionVerdict::Verified);
    verdicts.insert(sid("assertion-2"), AssertionVerdict::Verified);

    let status = claimed_result_status(&claimed, &[a1, a2], &verdicts);
    assert_eq!(status, ClaimedResultStatus::Established);
}

#[test]
fn evaluate_flags_an_assertion_naming_an_unknown_claimed_result() {
    let assertion =
        VerifiableAssertion::new(sid("assertion-1"), nes("tests pass"), sid("no-such-result"));
    let result = evaluate(&[], &[assertion], &[], &HashMap::new());
    assert_eq!(result.diagnostics.len(), 1);
    assert!(result.diagnostics[0].message().contains("no-such-result"));
}

#[test]
fn evaluate_is_end_to_end_deterministic_and_order_independent() {
    let claimed = ClaimedResult::new(sid("result-1"), nes("the migration succeeded"));
    let a1 = VerifiableAssertion::new(
        sid("assertion-1"),
        nes("coverage is complete"),
        sid("result-1"),
    );
    let a2 = VerifiableAssertion::new(sid("assertion-2"), nes("rollback works"), sid("result-1"));
    let e1 = evidence_entry("evidence-1", vec!["assertion-1"]);
    let e2 = evidence_entry("evidence-2", vec!["assertion-2"]);
    let mut resolved = HashMap::new();
    resolved.insert(
        sid("evidence-1"),
        ResolvedEvidenceResult {
            observed_result: ObservedResult::Confirmed,
            covers: vec![sid("assertion-1")],
        },
    );
    resolved.insert(
        sid("evidence-2"),
        ResolvedEvidenceResult {
            observed_result: ObservedResult::Confirmed,
            covers: vec![sid("assertion-2")],
        },
    );

    let forward = evaluate(
        std::slice::from_ref(&claimed),
        &[a1.clone(), a2.clone()],
        &[e1.clone(), e2.clone()],
        &resolved,
    );
    let reversed = evaluate(&[claimed], &[a2, a1], &[e2, e1], &resolved);

    assert_eq!(forward, reversed);
    assert_eq!(
        forward.claimed_result_statuses[&sid("result-1")],
        ClaimedResultStatus::Established
    );
    assert!(forward.diagnostics.is_empty());
}
