//! `evidence-and-handoff-contract` — the portable, machine-checkable handoff
//! of the STATE and RESULT of ONE execution run
//! (`standards/workspace/evidence-and-handoff-contract.md`,
//! `registries/operating-model/evidence-and-handoff.schema.json`).
//!
//! [`check_handoff`] is the one constructor of an accepted [`Handoff`]: it
//! runs the shared envelope check, sections 7–19 over the record itself
//! (`structure`) and section 20 against the external resolution boundary
//! (`resolved`), and returns the accepted value only when all twenty
//! sections found nothing. Without a resolution catalogue the handoff is an
//! unanchored self-report and fails closed.
//!
//! What is computed, never trusted: a claimed result's status (from its
//! linked assertions), whether a `verified` assertion is supported (by
//! clean, confirming, transformer-bound evidence), which checks ran (from
//! their statuses, confirmed by the resolved run's `completed_checks`) and
//! whether the stated outcome is consistent with blockers, checks,
//! deviations, owner decisions, gaps and the resolved run's own state.

pub mod input;
mod resolved;
mod structure;
#[cfg(test)]
mod tests;

use crate::run_contracts::envelope::{check_envelope, RecordEnvelope, RecordFamily};
use crate::run_contracts::{ResolutionCatalogue, RunStateScope};
use crate::types::{Diagnostic, SemanticId};

use super::linkage::{ClaimedResultStatus, ResolvedEvidenceResult};
use super::{fail, record_subject};
use input::{
    ClaimedResultInput, HandoffBody, HandoffEvidenceInput, HandoffInput, MandatoryCheckInput,
    OutcomeStatus,
};

/// An accepted evidence-and-handoff record: every one of the twenty
/// sections held, every pinned record and every evidence entry resolved
/// cleanly through the external boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handoff {
    envelope: RecordEnvelope,
    scope: RunStateScope,
    body: HandoffBody,
    claimed_statuses: Vec<ClaimedResultStatus>,
    evidence_results: Vec<ResolvedEvidenceResult>,
}

impl Handoff {
    pub fn id(&self) -> &SemanticId {
        &self.envelope.id
    }

    pub fn envelope(&self) -> &RecordEnvelope {
        &self.envelope
    }

    pub fn scope(&self) -> &RunStateScope {
        &self.scope
    }

    /// The one run this handoff belongs to.
    pub fn run_id(&self) -> &SemanticId {
        &self.body.pins.execution_run.id
    }

    pub fn body(&self) -> &HandoffBody {
        &self.body
    }

    /// The consistent, verified outcome.
    pub fn outcome(&self) -> OutcomeStatus {
        self.body.outcome.status
    }

    /// Every claimed result with its COMPUTED status.
    pub fn claimed_results(
        &self,
    ) -> impl Iterator<Item = (&ClaimedResultInput, ClaimedResultStatus)> {
        self.body
            .claimed_results
            .iter()
            .zip(self.claimed_statuses.iter().copied())
    }

    /// Every evidence entry with its clean external resolution.
    pub fn evidence(
        &self,
    ) -> impl Iterator<Item = (&HandoffEvidenceInput, &ResolvedEvidenceResult)> {
        self.body.evidence.iter().zip(&self.evidence_results)
    }

    /// The mandatory checks that ran (passed or failed), confirmed by the
    /// resolved run's `completed_checks`.
    pub fn executed_checks(&self) -> impl Iterator<Item = &MandatoryCheckInput> {
        self.body.mandatory_checks.iter().filter(|m| m.status.ran())
    }
}

/// Every evidence-and-handoff rule. `resolution` is the external boundary;
/// `None` fails closed.
pub fn check_handoff(
    input: HandoffInput,
    resolution: Option<&ResolutionCatalogue>,
) -> (Option<Handoff>, Vec<Diagnostic>) {
    let family = RecordFamily::EvidenceAndHandoff;
    let mut problems = Vec::new();
    check_envelope(family, &input.envelope, &mut problems);
    let structure = structure::check_structure(&input, &mut problems);
    let evidence_results = match resolution {
        None => {
            problems.push(fail(format!(
                "{} cannot be verified: no external record resolver was supplied; the pinned execution-run, task-specification, run-human-control and context-manifest records, and every evidence entry, are resolved OUTSIDE the handoff and checked against it — without that boundary the handoff is only an unanchored self-report",
                record_subject(family, input.envelope.id.as_str())
            )));
            Vec::new()
        }
        Some(catalogue) => resolved::check_resolution(&input, &structure, catalogue, &mut problems),
    };
    let claimed_statuses = structure.claimed_statuses;
    if !problems.is_empty() {
        return (None, problems);
    }
    // Zero problems: every evidence entry resolved cleanly.
    let Some(evidence_results) = evidence_results.into_iter().collect::<Option<Vec<_>>>() else {
        return (
            None,
            vec![fail(format!(
                "{} internal: an evidence entry has no clean resolution although no problem was reported",
                record_subject(family, input.envelope.id.as_str())
            ))],
        );
    };
    let HandoffInput {
        envelope,
        scope,
        body,
    } = input;
    (
        Some(Handoff {
            envelope,
            scope,
            body,
            claimed_statuses,
            evidence_results,
        }),
        problems,
    )
}
