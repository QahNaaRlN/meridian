//! `field-evaluation-observation`: one measured data point about ONE metric
//! of ONE execution run, pinned to that run and grounded by externally
//! resolved evidence (`evaluateObservationBody`).

use std::collections::HashSet;

use crate::evidence::resolve::{check_pinned_evidence, resolve_evidence_result};
use crate::evidence::{fail, record_subject, ObservedResult, PinnedEvidence};
use crate::run_contracts::envelope::RecordFamily;
use crate::run_contracts::pinned_ref::check_pin_shape;
use crate::run_contracts::resolution::check_resolved_entry;
use crate::run_contracts::{
    MeasurementFields, PinnedRecordKind, PinnedRef, RecordEnvelope, RecordText,
    ResolutionCatalogue, RunStateScope,
};
use crate::task_contracts::non_portable_reason;
use crate::types::{Diagnostic, SemanticId};

use super::measurement::{check_measurement, Measurement};
use super::metric::{Coverage, MetricId, ObservationStatus};
use super::time::{CalendarDate, DateWindow};
use super::FieldScope;

const FAMILY: RecordFamily = RecordFamily::FieldEvaluationObservation;

/// An observation window as stated: two `YYYY-MM-DD` texts the domain
/// checks for calendar existence and order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowInput {
    pub start_date: RecordText,
    pub end_date: RecordText,
}

/// One schema-clean observation record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationInput {
    pub envelope: RecordEnvelope,
    pub scope: FieldScope,
    pub run: PinnedRef,
    pub metric: MetricId,
    pub observed_at: RecordText,
    pub status: ObservationStatus,
    pub status_reason: Option<RecordText>,
    pub measurement: Option<MeasurementFields>,
    pub observation_period: Option<WindowInput>,
    pub coverage: Option<Coverage>,
    pub coverage_note: Option<RecordText>,
    pub evidence: Vec<PinnedEvidence>,
    pub supersedes: Option<PinnedRef>,
    pub correction_reason: Option<RecordText>,
}

/// A covered observation window of `post-acceptance-defects`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoveredWindow {
    pub window: DateWindow,
    pub coverage: Coverage,
    pub note: Option<RecordText>,
}

/// What an accepted observation says: a measured value grounded by
/// confirmed evidence, or one of three distinct, explained absences.
#[derive(Debug, Clone, PartialEq)]
pub enum ObservationState {
    Observed {
        measurement: Measurement,
        window: Option<CoveredWindow>,
        evidence: Vec<PinnedEvidence>,
    },
    Absent {
        status: ObservationStatus,
        reason: RecordText,
    },
}

/// An accepted correction: the observation replaced and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Correction {
    pub supersedes: PinnedRef,
    pub reason: RecordText,
}

/// An accepted observation.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldObservation {
    envelope: RecordEnvelope,
    scope: RunStateScope,
    run: PinnedRef,
    metric: MetricId,
    observed_at: CalendarDate,
    state: ObservationState,
    correction: Option<Correction>,
}

impl FieldObservation {
    pub fn id(&self) -> &SemanticId {
        &self.envelope.id
    }
    pub fn scope(&self) -> &RunStateScope {
        &self.scope
    }
    pub fn run(&self) -> &PinnedRef {
        &self.run
    }
    pub fn metric(&self) -> MetricId {
        self.metric
    }
    pub fn observed_at(&self) -> CalendarDate {
        self.observed_at
    }
    pub fn state(&self) -> &ObservationState {
        &self.state
    }
    pub fn correction(&self) -> Option<&Correction> {
        self.correction.as_ref()
    }
}

fn blank(value: Option<&RecordText>) -> bool {
    value.is_none_or(RecordText::is_blank)
}

/// The observation body. Returns the accepted observation only when no
/// problem was found (the caller also requires a clean envelope).
pub(super) fn check_observation(
    input: ObservationInput,
    catalogue: Option<&ResolutionCatalogue>,
    problems: &mut Vec<Diagnostic>,
) -> Option<FieldObservation> {
    let before = problems.len();
    let id = input.envelope.id.as_str();
    let subject = record_subject(FAMILY, id);

    check_pin_shape(
        FAMILY,
        id,
        "execution_run_ref",
        PinnedRecordKind::ExecutionRun,
        &input.run,
        problems,
    );
    match catalogue {
        Some(catalogue) => {
            check_resolved_entry(
                FAMILY,
                id,
                "execution_run_ref",
                PinnedRecordKind::ExecutionRun,
                &input.run,
                catalogue,
                problems,
            );
        }
        None => problems.push(fail(format!(
            "{subject} cannot be verified: no external record resolver was supplied; the pinned run is resolved OUTSIDE the record and checked against it"
        ))),
    }
    if let FieldScope::RunState(scope) = &input.scope {
        let scope_run = scope.run_id().as_str();
        if scope_run != input.run.id.as_str() {
            problems.push(fail(format!(
                "{subject} scope identifies run \"{scope_run}\" but execution_run_ref.id is \"{}\"; the two must be the exact same string, not one a substring of the other",
                input.run.id.as_str()
            )));
        }
    }

    let metric = input.metric;
    let observed_at = CalendarDate::parse(input.observed_at.as_str());
    if observed_at.is_none() {
        problems.push(fail(format!(
            "{subject} observed_at is not a valid date (YYYY-MM-DD); the record states when it was itself recorded"
        )));
    }

    let status = input.status;
    let mut measurement = None;
    if status == ObservationStatus::Observed {
        if input.status_reason.is_some() {
            problems.push(fail(format!(
                "{subject} is \"observed\" but carries a status_reason; a reason is stated for unknown, not_applicable or unmeasurable, where the absence of a value needs explaining"
            )));
        }
        match &input.measurement {
            None => problems.push(fail(format!(
                "{subject} is \"observed\" but carries no measurement"
            ))),
            Some(fields) => match check_measurement(metric, fields) {
                Ok(m) => measurement = Some(m),
                Err(issues) => {
                    problems.extend(issues.into_iter().map(|i| fail(format!("{subject} {i}"))))
                }
            },
        }
    } else {
        match input.status_reason.as_ref().filter(|r| !r.is_blank()) {
            None => problems.push(fail(format!(
                "{subject} is \"{status}\" but states no status_reason; unknown, not_applicable and unmeasurable are distinct and each needs its own explanation, not a shared silence"
            ))),
            Some(r) => {
                if let Some(reason) = non_portable_reason(Some(r.as_str())) {
                    problems.push(fail(format!("{subject} status_reason contains {reason}")));
                }
            }
        }
        if input.measurement.is_some() {
            problems.push(fail(format!(
                "{subject} is \"{status}\" but carries a measurement; a value that was not observed is not measured"
            )));
        }
    }

    let window = check_window(&subject, &input, problems);
    let evidence_grounded = check_evidence(id, &subject, &input, catalogue, problems);
    let correction = check_correction(id, &subject, &input, catalogue, problems);

    if problems.len() != before {
        return None;
    }
    let FieldScope::RunState(scope) = input.scope else {
        return None;
    };
    let state = match (status, measurement) {
        (ObservationStatus::Observed, Some(measurement)) if evidence_grounded => {
            ObservationState::Observed {
                measurement,
                window,
                evidence: input.evidence,
            }
        }
        (ObservationStatus::Observed, _) => return None,
        (status, _) => ObservationState::Absent {
            status,
            reason: input.status_reason?,
        },
    };
    Some(FieldObservation {
        envelope: input.envelope,
        scope,
        run: input.run,
        metric,
        observed_at: observed_at?,
        state,
        correction,
    })
}

/// The observation window of `post-acceptance-defects`, used only while
/// observed.
fn check_window(
    subject: &str,
    input: &ObservationInput,
    problems: &mut Vec<Diagnostic>,
) -> Option<CoveredWindow> {
    let metric = input.metric;
    if input.status == ObservationStatus::Observed && metric.requires_window() {
        let Some(period) = &input.observation_period else {
            problems.push(fail(format!(
                "{subject} measures \"{metric}\", which requires an observation_period (the window actually covered) and a coverage completeness flag"
            )));
            return None;
        };
        let window = match (
            CalendarDate::parse(period.start_date.as_str()),
            CalendarDate::parse(period.end_date.as_str()),
        ) {
            (Some(start), Some(end)) => {
                let window = DateWindow::new(start, end);
                if window.is_none() {
                    problems.push(fail(format!(
                        "{subject} observation_period end_date is before start_date; an impossible window"
                    )));
                }
                window
            }
            _ => {
                problems.push(fail(format!(
                    "{subject} observation_period does not carry two valid dates"
                )));
                None
            }
        };
        match input.coverage {
            None => problems.push(fail(format!(
                "{subject} measures \"{metric}\" but coverage undefined is not one of {{ {} }}",
                Coverage::ALL.map(Coverage::as_str).join(", ")
            ))),
            Some(Coverage::Partial) if blank(input.coverage_note.as_ref()) => {
                problems.push(fail(format!(
                    "{subject} coverage is \"partial\" but states no coverage_note; an incomplete window needs to say what is not covered"
                )))
            }
            Some(Coverage::Complete) if input.coverage_note.is_some() => {
                problems.push(fail(format!(
                    "{subject} coverage is \"complete\" but carries a coverage_note; a note belongs to a partial window"
                )))
            }
            Some(_) => {}
        }
        return window
            .zip(input.coverage)
            .map(|(window, coverage)| CoveredWindow {
                window,
                coverage,
                note: input.coverage_note.clone(),
            });
    }
    if input.observation_period.is_some()
        || input.coverage.is_some()
        || input.coverage_note.is_some()
    {
        problems.push(fail(format!(
            "{subject} carries observation_period/coverage, but metric \"{metric}\" at status \"{}\" does not use an observation window; only \"{}\" while \"observed\" does",
            input.status,
            MetricId::PostAcceptanceDefects
        )));
    }
    None
}

/// Evidence: required and grounded while observed, forbidden otherwise.
/// `true` when some entry resolved cleanly as `confirmed` for this metric.
fn check_evidence(
    id: &str,
    subject: &str,
    input: &ObservationInput,
    catalogue: Option<&ResolutionCatalogue>,
    problems: &mut Vec<Diagnostic>,
) -> bool {
    let observed = input.status == ObservationStatus::Observed;
    let metric = input.metric;
    if observed && input.evidence.is_empty() {
        problems.push(fail(format!(
            "{subject} is \"observed\" but carries no evidence; a measured value is grounded by at least one resolvable piece of evidence, not asserted on its own say-so"
        )));
    }
    if !observed && !input.evidence.is_empty() {
        problems.push(fail(format!(
            "{subject} is \"{}\" but carries evidence; there is nothing to evidence when nothing was observed",
            input.status
        )));
    }
    let mut seen = HashSet::new();
    let mut any_confirmed = false;
    for e in &input.evidence {
        check_pinned_evidence(FAMILY, id, e, &mut seen, problems);
        let (Some(catalogue), true) = (catalogue, observed) else {
            continue;
        };
        let before = problems.len();
        let at = e.id.as_str();
        let resolution = resolve_evidence_result(FAMILY, id, e, catalogue, problems);
        if let Some(entry) = resolution.entry {
            let metric_ref = &entry.evidence_result.metric_ref;
            match metric_ref.non_blank() {
                None => problems.push(fail(format!(
                    "{subject} evidence entry {at}: the resolved evidence-result confirms no metric_ref; which metric an evidence entry bears on is confirmed by the transformer, not asserted by the record"
                ))),
                Some(r) if r != metric.as_str() => problems.push(fail(format!(
                    "{subject} evidence entry {at}: the resolved evidence-result's metric_ref \"{r}\" is not this observation's own metric \"{metric}\"; evidence of a different indicator does not support this one"
                ))),
                Some(_) => {}
            }
        }
        if problems.len() == before && resolution.observed == Some(ObservedResult::Confirmed) {
            any_confirmed = true;
        }
    }
    if observed && catalogue.is_some() && !input.evidence.is_empty() && !any_confirmed {
        problems.push(fail(format!(
            "{subject} is \"observed\" but no evidence entry resolves cleanly with observed_result \"confirmed\" for this metric; an unresolved or contradicted/inconclusive entry does not ground a measured value"
        )));
    } else if observed && catalogue.is_none() {
        problems.push(fail(format!(
            "{subject} cannot be verified: no external record resolver was supplied; evidence is resolved OUTSIDE the record and checked against it — without that boundary the record is only an unanchored self-report"
        )));
    }
    any_confirmed
}

/// Correction: `supersedes` and `correction_reason`, both or neither.
fn check_correction(
    id: &str,
    subject: &str,
    input: &ObservationInput,
    catalogue: Option<&ResolutionCatalogue>,
    problems: &mut Vec<Diagnostic>,
) -> Option<Correction> {
    match (&input.supersedes, &input.correction_reason) {
        (None, None) => None,
        (Some(supersedes), Some(reason)) => {
            check_pin_shape(
                FAMILY,
                id,
                "supersedes",
                PinnedRecordKind::FieldEvaluationObservation,
                supersedes,
                problems,
            );
            if reason.is_blank() {
                problems.push(fail(format!("{subject} correction_reason is blank")));
            } else if let Some(r) = non_portable_reason(Some(reason.as_str())) {
                problems.push(fail(format!("{subject} correction_reason contains {r}")));
            }
            if supersedes.id.as_str() == id {
                problems.push(fail(format!(
                    "{subject} supersedes itself; a correction names a DIFFERENT, earlier observation"
                )));
            }
            if let Some(catalogue) = catalogue {
                check_resolved_entry(
                    FAMILY,
                    id,
                    "supersedes",
                    PinnedRecordKind::FieldEvaluationObservation,
                    supersedes,
                    catalogue,
                    problems,
                );
            }
            Some(Correction {
                supersedes: supersedes.clone(),
                reason: reason.clone(),
            })
        }
        _ => {
            problems.push(fail(format!(
                "{subject} states only one of supersedes / correction_reason; a correction names BOTH the observation it corrects and why"
            )));
            None
        }
    }
}
