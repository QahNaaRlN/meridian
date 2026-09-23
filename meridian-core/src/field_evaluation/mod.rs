//! `meridian-field-evaluation` — practical evaluation of Meridian's own
//! mechanisms (`registries/operating-model/field-evaluation.schema.json`):
//! two record types, one contract.
//!
//! - an OBSERVATION ([`observation`]) records one measured data point about
//!   ONE of the eight characteristics ([`metric`]) of ONE run, pinned to it
//!   and grounded by externally resolved evidence;
//! - a REPORT ([`report`]) aggregates resolved observations per metric and
//!   is accepted only when every stated aggregate equals the one recomputed
//!   from the typed samples ([`aggregate`]).
//!
//! The eight metrics stay separate: there is no composite score, no
//! readiness verdict, and a report never absorbs a handoff. Dates,
//! timestamps, windows and intervals are validated types ([`time`]); a
//! measurement is typed by its metric through the ONE measurement rule
//! ([`measurement`]); a resolved observation is validated before it is ever
//! counted (`resolved`).
//!
//! [`check_field_evaluation`] is the one constructor of an accepted
//! [`FieldEvaluationRecord`]. Diagnostics are prefix-free; no serde,
//! `serde_json::Value`, JSON Schema, file, process, env or output here.

pub mod aggregate;
pub mod measurement;
pub mod metric;
pub mod observation;
pub mod report;
pub(crate) mod resolved;
#[cfg(test)]
mod tests;
pub mod time;

use crate::evidence::{fail, record_subject};
use crate::run_contracts::envelope::{check_envelope_around, RecordEnvelope, RecordFamily};
use crate::run_contracts::{ResolutionCatalogue, RunStateScope};
use crate::types::{Diagnostic, SemanticId};

pub use aggregate::{
    AggregateCoverage, DurationSummary, MetricAggregate, Percentage, Rate, StatedAggregate,
    StatedDurationSummary, StatedRate,
};
pub use measurement::{DurationMeasurement, Measurement, Seconds};
pub use metric::{
    ClassificationMetric, ClassificationOutcome, CountMetric, Coverage, DurationKind,
    DurationMetric, MeasurementKind, MetricId, MetricReportStatus, MetricShape, ObservationStatus,
    OutcomeLabel,
};
pub use observation::{
    Correction, CoveredWindow, FieldObservation, ObservationInput, ObservationState, WindowInput,
};
pub use report::{
    ExclusionInput, FieldEvaluationReport, MetricReport, MetricReportInput, ReportInput,
};
pub use time::{CalendarDate, DateWindow, TimeInterval, Timestamp};

/// The two areas a field-evaluation record may name. An observation lives
/// in `run-state`, a report in `project-workspace`; the other one is a
/// domain rejection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldScope {
    RunState(RunStateScope),
    ProjectWorkspace(SemanticId),
}

impl FieldScope {
    fn type_name(&self) -> &'static str {
        match self {
            FieldScope::RunState(_) => "run-state",
            FieldScope::ProjectWorkspace(_) => "project-workspace",
        }
    }
}

/// One schema-clean field-evaluation record, selected by `record_type`.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldEvaluationInput {
    Observation(Box<ObservationInput>),
    Report(Box<ReportInput>),
}

/// An accepted field-evaluation record.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldEvaluationRecord {
    Observation(Box<FieldObservation>),
    Report(Box<FieldEvaluationReport>),
}

impl FieldEvaluationInput {
    fn family(&self) -> RecordFamily {
        match self {
            FieldEvaluationInput::Observation(_) => RecordFamily::FieldEvaluationObservation,
            FieldEvaluationInput::Report(_) => RecordFamily::FieldEvaluationReport,
        }
    }

    fn envelope(&self) -> &RecordEnvelope {
        match self {
            FieldEvaluationInput::Observation(o) => &o.envelope,
            FieldEvaluationInput::Report(r) => &r.envelope,
        }
    }

    fn scope(&self) -> &FieldScope {
        match self {
            FieldEvaluationInput::Observation(o) => &o.scope,
            FieldEvaluationInput::Report(r) => &r.scope,
        }
    }
}

/// Every field-evaluation rule. `resolution` is the external boundary;
/// `None` fails closed.
pub fn check_field_evaluation(
    input: FieldEvaluationInput,
    resolution: Option<&ResolutionCatalogue>,
) -> (Option<FieldEvaluationRecord>, Vec<Diagnostic>) {
    let family = input.family();
    let mut problems = Vec::new();
    check_envelope_around(family, input.envelope(), &mut problems, |problems| {
        let want = match family {
            RecordFamily::FieldEvaluationReport => "project-workspace",
            _ => "run-state",
        };
        let have = input.scope().type_name();
        if have != want {
            problems.push(fail(format!(
                "{} is scoped to \"{have}\"; a {} lives in {want} only",
                record_subject(family, input.envelope().id.as_str()),
                family.label()
            )));
        }
    });
    let accepted = match input {
        FieldEvaluationInput::Observation(o) => {
            observation::check_observation(*o, resolution, &mut problems)
                .map(|o| FieldEvaluationRecord::Observation(Box::new(o)))
        }
        FieldEvaluationInput::Report(r) => report::check_report(*r, resolution, &mut problems)
            .map(|r| FieldEvaluationRecord::Report(Box::new(r))),
    };
    if problems.is_empty() {
        (accepted, problems)
    } else {
        (None, problems)
    }
}
