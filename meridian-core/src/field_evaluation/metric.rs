//! The eight independent characteristics of practical evaluation
//! (`meridian-operating-upgrade-plan.md` §10) and the closed pools around
//! them. Each metric has ONE measurement kind; a classification metric has
//! its OWN closed outcome pool and tracked outcome. No composite score
//! exists anywhere in this vocabulary.

use core::fmt;

/// One of the eight characteristics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MetricId {
    MechanismCorrectness,
    ContextEntryTime,
    ReworkReturns,
    StopCorrectness,
    MissedNorms,
    ResumptionSuccess,
    OwnerCost,
    PostAcceptanceDefects,
}

/// A classification metric.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClassificationMetric {
    MechanismCorrectness,
    StopCorrectness,
    MissedNorms,
    ResumptionSuccess,
}

/// A duration metric.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DurationMetric {
    ContextEntryTime,
    OwnerCost,
}

/// A count metric. Only `post-acceptance-defects` names an observation
/// window and a coverage flag; `rework-returns` is a per-run tally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CountMetric {
    ReworkReturns,
    PostAcceptanceDefects,
}

/// How a metric is measured — the metric's kind with the metric itself, so
/// a kind and a metric can never disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetricShape {
    Classification(ClassificationMetric),
    Duration(DurationMetric),
    Count(CountMetric),
}

/// The closed measurement-kind pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MeasurementKind {
    Classification,
    Duration,
    Count,
}

impl MeasurementKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            MeasurementKind::Classification => "classification",
            MeasurementKind::Duration => "duration",
            MeasurementKind::Count => "count",
        }
    }
}

impl MetricId {
    /// The eight metrics, in the contract's order.
    pub const ALL: [MetricId; 8] = [
        MetricId::MechanismCorrectness,
        MetricId::ContextEntryTime,
        MetricId::ReworkReturns,
        MetricId::StopCorrectness,
        MetricId::MissedNorms,
        MetricId::ResumptionSuccess,
        MetricId::OwnerCost,
        MetricId::PostAcceptanceDefects,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            MetricId::MechanismCorrectness => "mechanism-correctness",
            MetricId::ContextEntryTime => "context-entry-time",
            MetricId::ReworkReturns => "rework-returns",
            MetricId::StopCorrectness => "stop-correctness",
            MetricId::MissedNorms => "missed-norms",
            MetricId::ResumptionSuccess => "resumption-success",
            MetricId::OwnerCost => "owner-cost",
            MetricId::PostAcceptanceDefects => "post-acceptance-defects",
        }
    }

    pub fn parse(value: &str) -> Option<MetricId> {
        Self::ALL.into_iter().find(|m| m.as_str() == value)
    }

    /// `{ a, b, … }` of all eight, for diagnostics.
    pub fn pool_text() -> String {
        Self::ALL.map(MetricId::as_str).join(", ")
    }

    pub const fn shape(self) -> MetricShape {
        match self {
            MetricId::MechanismCorrectness => {
                MetricShape::Classification(ClassificationMetric::MechanismCorrectness)
            }
            MetricId::StopCorrectness => {
                MetricShape::Classification(ClassificationMetric::StopCorrectness)
            }
            MetricId::MissedNorms => MetricShape::Classification(ClassificationMetric::MissedNorms),
            MetricId::ResumptionSuccess => {
                MetricShape::Classification(ClassificationMetric::ResumptionSuccess)
            }
            MetricId::ContextEntryTime => MetricShape::Duration(DurationMetric::ContextEntryTime),
            MetricId::OwnerCost => MetricShape::Duration(DurationMetric::OwnerCost),
            MetricId::ReworkReturns => MetricShape::Count(CountMetric::ReworkReturns),
            MetricId::PostAcceptanceDefects => {
                MetricShape::Count(CountMetric::PostAcceptanceDefects)
            }
        }
    }

    pub const fn kind(self) -> MeasurementKind {
        match self.shape() {
            MetricShape::Classification(_) => MeasurementKind::Classification,
            MetricShape::Duration(_) => MeasurementKind::Duration,
            MetricShape::Count(_) => MeasurementKind::Count,
        }
    }

    /// Only `post-acceptance-defects` names an observation window.
    pub const fn requires_window(self) -> bool {
        matches!(self, MetricId::PostAcceptanceDefects)
    }
}

impl fmt::Display for MetricId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Every classification outcome label of every pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OutcomeLabel {
    Correct,
    Incorrect,
    CorrectStop,
    FalseStop,
    Missed,
    NotMissed,
    Successful,
    Unsuccessful,
    Unknown,
}

impl OutcomeLabel {
    pub const fn as_str(self) -> &'static str {
        match self {
            OutcomeLabel::Correct => "correct",
            OutcomeLabel::Incorrect => "incorrect",
            OutcomeLabel::CorrectStop => "correct_stop",
            OutcomeLabel::FalseStop => "false_stop",
            OutcomeLabel::Missed => "missed",
            OutcomeLabel::NotMissed => "not_missed",
            OutcomeLabel::Successful => "successful",
            OutcomeLabel::Unsuccessful => "unsuccessful",
            OutcomeLabel::Unknown => "unknown",
        }
    }
}

impl fmt::Display for OutcomeLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl ClassificationMetric {
    pub const fn metric(self) -> MetricId {
        match self {
            ClassificationMetric::MechanismCorrectness => MetricId::MechanismCorrectness,
            ClassificationMetric::StopCorrectness => MetricId::StopCorrectness,
            ClassificationMetric::MissedNorms => MetricId::MissedNorms,
            ClassificationMetric::ResumptionSuccess => MetricId::ResumptionSuccess,
        }
    }

    /// This metric's closed outcome pool, in the contract's order.
    pub const fn pool(self) -> [OutcomeLabel; 3] {
        use OutcomeLabel as O;
        match self {
            ClassificationMetric::MechanismCorrectness => [O::Correct, O::Incorrect, O::Unknown],
            ClassificationMetric::StopCorrectness => [O::CorrectStop, O::FalseStop, O::Unknown],
            ClassificationMetric::MissedNorms => [O::Missed, O::NotMissed, O::Unknown],
            ClassificationMetric::ResumptionSuccess => [O::Successful, O::Unsuccessful, O::Unknown],
        }
    }

    /// The single outcome this metric's rate tracks.
    pub const fn tracked(self) -> OutcomeLabel {
        match self {
            ClassificationMetric::MechanismCorrectness => OutcomeLabel::Correct,
            ClassificationMetric::StopCorrectness => OutcomeLabel::FalseStop,
            ClassificationMetric::MissedNorms => OutcomeLabel::Missed,
            ClassificationMetric::ResumptionSuccess => OutcomeLabel::Successful,
        }
    }

    /// This metric's outcome named `value`, if it is in the pool.
    pub fn outcome(self, value: &str) -> Option<ClassificationOutcome> {
        self.pool()
            .into_iter()
            .find(|o| o.as_str() == value)
            .map(|label| ClassificationOutcome {
                metric: self,
                label,
            })
    }

    pub fn pool_text(self) -> String {
        self.pool().map(OutcomeLabel::as_str).join(", ")
    }
}

/// An outcome of ONE classification metric's pool: an outcome from another
/// metric's pool is unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClassificationOutcome {
    metric: ClassificationMetric,
    label: OutcomeLabel,
}

impl ClassificationOutcome {
    pub fn metric(self) -> ClassificationMetric {
        self.metric
    }

    pub fn label(self) -> OutcomeLabel {
        self.label
    }
}

/// The closed observation-status pool: an observed value, or one of three
/// distinct kinds of absence — never a measured zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObservationStatus {
    Observed,
    Unknown,
    NotApplicable,
    Unmeasurable,
}

impl ObservationStatus {
    pub const ALL: [ObservationStatus; 4] = [
        ObservationStatus::Observed,
        ObservationStatus::Unknown,
        ObservationStatus::NotApplicable,
        ObservationStatus::Unmeasurable,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            ObservationStatus::Observed => "observed",
            ObservationStatus::Unknown => "unknown",
            ObservationStatus::NotApplicable => "not_applicable",
            ObservationStatus::Unmeasurable => "unmeasurable",
        }
    }

    pub fn parse(value: &str) -> Option<ObservationStatus> {
        Self::ALL.into_iter().find(|s| s.as_str() == value)
    }

    pub fn pool_text() -> String {
        Self::ALL.map(ObservationStatus::as_str).join(", ")
    }
}

impl fmt::Display for ObservationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Calendar length and active effort are distinguished, never summed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DurationKind {
    Calendar,
    Active,
}

impl DurationKind {
    pub const ALL: [DurationKind; 2] = [DurationKind::Calendar, DurationKind::Active];

    pub const fn as_str(self) -> &'static str {
        match self {
            DurationKind::Calendar => "calendar",
            DurationKind::Active => "active",
        }
    }

    pub fn parse(value: &str) -> Option<DurationKind> {
        Self::ALL.into_iter().find(|k| k.as_str() == value)
    }
}

/// Whether an observation window was covered completely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Coverage {
    Complete,
    Partial,
}

impl Coverage {
    pub const ALL: [Coverage; 2] = [Coverage::Complete, Coverage::Partial];

    pub const fn as_str(self) -> &'static str {
        match self {
            Coverage::Complete => "complete",
            Coverage::Partial => "partial",
        }
    }

    pub fn parse(value: &str) -> Option<Coverage> {
        Self::ALL.into_iter().find(|c| c.as_str() == value)
    }
}

/// A report's per-metric conclusion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetricReportStatus {
    Known,
    Unknown,
    NotApplicable,
}

impl MetricReportStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            MetricReportStatus::Known => "known",
            MetricReportStatus::Unknown => "unknown",
            MetricReportStatus::NotApplicable => "not_applicable",
        }
    }

    pub fn parse(value: &str) -> Option<MetricReportStatus> {
        match value {
            "known" => Some(MetricReportStatus::Known),
            "unknown" => Some(MetricReportStatus::Unknown),
            "not_applicable" => Some(MetricReportStatus::NotApplicable),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_evaluation_the_eight_metrics_have_fixed_kinds_and_closed_pools() {
        assert_eq!(MetricId::ALL.len(), 8);
        for m in MetricId::ALL {
            assert_eq!(MetricId::parse(m.as_str()), Some(m));
        }
        assert_eq!(MetricId::ContextEntryTime.kind(), MeasurementKind::Duration);
        assert_eq!(MetricId::ReworkReturns.kind(), MeasurementKind::Count);
        assert!(MetricId::PostAcceptanceDefects.requires_window());
        assert!(!MetricId::ReworkReturns.requires_window());
        let stop = ClassificationMetric::StopCorrectness;
        assert_eq!(stop.tracked(), OutcomeLabel::FalseStop);
        assert!(stop.outcome("false_stop").is_some());
        assert!(
            stop.outcome("correct").is_none(),
            "another metric's outcome"
        );
        assert_eq!(
            ClassificationMetric::MissedNorms.pool_text(),
            "missed, not_missed, unknown"
        );
    }
}
