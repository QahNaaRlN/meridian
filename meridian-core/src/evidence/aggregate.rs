//! Computing — never trusting — an assertion's verified status and a
//! claimed result's established status
//! (`evidence-and-handoff-contract.md` §5.1–§5.3,
//! `operating-principles.md`'s `evidence-before-status`).

use std::collections::HashMap;

use crate::types::{Diagnostic, DiagnosticLevel, SemanticId};

use super::types::{
    ClaimedResult, EvidenceEntry, ObservedResult, ResolvedEvidenceResult, VerifiableAssertion,
};

/// Whether one [`VerifiableAssertion`] is verified — computed, never
/// asserted. `Unverified` always carries an explicit reason, so "not
/// verified" is never represented by an absent or defaulted value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssertionVerdict {
    Verified,
    Unverified { reason: String },
}

/// Whether one [`ClaimedResult`] is established — computed, never asserted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClaimedResultStatus {
    Established,
    NotEstablished,
}

/// An assertion is verified only when at least one evidence entry both
/// CLAIMS to cover it (`entry.covers`) and is independently, externally
/// confirmed to cover it (`resolved.covers`) with `observed_result:
/// confirmed`. An evidence entry's own `covers` list is never itself
/// proof — the same reason `contradicted`/`inconclusive` never verify an
/// assertion, and the same rule that stops a broad piece of evidence from
/// being silently reassigned to an assertion it does not actually bear on.
pub fn verify_assertion(
    assertion: &VerifiableAssertion,
    evidence: &[EvidenceEntry],
    resolved: &HashMap<SemanticId, ResolvedEvidenceResult>,
) -> AssertionVerdict {
    for entry in evidence {
        if !entry.covers().contains(assertion.id()) {
            continue;
        }
        let Some(result) = resolved.get(entry.id()) else {
            continue;
        };
        if matches!(result.observed_result, ObservedResult::Confirmed)
            && result.covers.contains(assertion.id())
        {
            return AssertionVerdict::Verified;
        }
    }
    AssertionVerdict::Unverified {
        reason: format!(
            "no resolved evidence entry both covers assertion \"{}\" and is externally confirmed (observed_result: confirmed) to do so",
            assertion.id()
        ),
    }
}

/// A claimed result is `Established` iff it has at least one linked
/// assertion and every linked assertion is [`AssertionVerdict::Verified`];
/// `NotEstablished` otherwise — including when it has no linked assertion
/// at all. A result is never established by a single verified assertion
/// while another linked assertion remains unverified.
pub fn claimed_result_status(
    claimed_result: &ClaimedResult,
    assertions: &[VerifiableAssertion],
    assertion_verdicts: &HashMap<SemanticId, AssertionVerdict>,
) -> ClaimedResultStatus {
    let linked: Vec<&VerifiableAssertion> = assertions
        .iter()
        .filter(|a| a.claimed_result_id() == claimed_result.id())
        .collect();
    if linked.is_empty() {
        return ClaimedResultStatus::NotEstablished;
    }
    let all_verified = linked.iter().all(|a| {
        matches!(
            assertion_verdicts.get(a.id()),
            Some(AssertionVerdict::Verified)
        )
    });
    if all_verified {
        ClaimedResultStatus::Established
    } else {
        ClaimedResultStatus::NotEstablished
    }
}

/// The full, deterministic evaluation of one handoff's claim/assertion/
/// evidence linkage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceEvaluation {
    pub assertion_verdicts: HashMap<SemanticId, AssertionVerdict>,
    pub claimed_result_statuses: HashMap<SemanticId, ClaimedResultStatus>,
    /// `FAIL` diagnostics for structural problems the type system does not
    /// already prevent — currently, an assertion naming an undeclared
    /// `claimed_result_id`.
    pub diagnostics: Vec<Diagnostic>,
}

/// Evaluates every assertion and every claimed result of one handoff
/// against already-resolved evidence. Deterministic: the same input always
/// produces the same verdicts, independent of `evidence`'s declaration
/// order (each assertion's evidence search considers every entry) and of
/// `assertions`'/`claimed_results`' declaration order (each is evaluated
/// independently and the results are keyed by id, not position).
pub fn evaluate(
    claimed_results: &[ClaimedResult],
    assertions: &[VerifiableAssertion],
    evidence: &[EvidenceEntry],
    resolved: &HashMap<SemanticId, ResolvedEvidenceResult>,
) -> EvidenceEvaluation {
    let mut diagnostics = Vec::new();
    let claimed_ids: std::collections::HashSet<&SemanticId> =
        claimed_results.iter().map(ClaimedResult::id).collect();

    let mut assertion_verdicts = HashMap::new();
    for assertion in assertions {
        if !claimed_ids.contains(assertion.claimed_result_id()) {
            diagnostics.push(
                Diagnostic::new(
                    DiagnosticLevel::Fail,
                    format!(
                        "verifiable assertion \"{}\" names claimed_result_id \"{}\", which is not among the declared claimed results",
                        assertion.id(),
                        assertion.claimed_result_id()
                    ),
                )
                .expect("message is never empty"),
            );
        }
        assertion_verdicts.insert(
            assertion.id().clone(),
            verify_assertion(assertion, evidence, resolved),
        );
    }

    let mut claimed_result_statuses = HashMap::new();
    for claimed_result in claimed_results {
        claimed_result_statuses.insert(
            claimed_result.id().clone(),
            claimed_result_status(claimed_result, assertions, &assertion_verdicts),
        );
    }

    EvidenceEvaluation {
        assertion_verdicts,
        claimed_result_statuses,
        diagnostics,
    }
}
