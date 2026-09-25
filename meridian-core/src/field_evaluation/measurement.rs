//! The ONE measurement rule (`measurementIssues`): the same rule checks a
//! record's own `payload.measurement` and a resolved observation's
//! `measurement` before anything is aggregated. It reads
//! [`MeasurementFields`] and yields either a typed [`Measurement`] — whose
//! shape is fixed by the metric, so a classification, a duration and a
//! count can never be mixed — or the prefix-free issues.

use crate::run_contracts::{JsonNumber, MeasurementFields, RecordText, ResponseField};
use crate::task_contracts::non_portable_reason;

use super::metric::{
    ClassificationMetric, ClassificationOutcome, CountMetric, DurationKind, DurationMetric,
    MetricId, MetricShape,
};
use super::time::{TimeInterval, Timestamp};

/// A non-negative, finite number of seconds.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Seconds(f64);

impl Seconds {
    pub fn new(value: f64) -> Option<Seconds> {
        (value.is_finite() && value >= 0.0).then_some(Seconds(value))
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

/// A duration measurement of one duration metric. Only a calendar span may
/// subtract paused time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DurationMeasurement {
    metric: DurationMetric,
    kind: DurationKind,
    seconds: Seconds,
    paused: Option<Seconds>,
    interval: Option<TimeInterval>,
}

impl DurationMeasurement {
    pub fn metric(&self) -> DurationMetric {
        self.metric
    }
    pub fn kind(&self) -> DurationKind {
        self.kind
    }
    pub fn seconds(&self) -> Seconds {
        self.seconds
    }
    pub fn paused(&self) -> Option<Seconds> {
        self.paused
    }
    pub fn interval(&self) -> Option<TimeInterval> {
        self.interval
    }
}

/// One valid measurement.
#[derive(Debug, Clone, PartialEq)]
pub enum Measurement {
    Classification {
        outcome: ClassificationOutcome,
        basis: RecordText,
    },
    Duration(DurationMeasurement),
    Count {
        metric: CountMetric,
        value: u64,
    },
}

/// `"${x}"` of a field — the quoted string coercion.
fn quoted(field: &ResponseField<String>) -> String {
    format!("\"{}\"", field.string_or_undefined())
}

fn non_negative(field: &ResponseField<JsonNumber>) -> Option<Seconds> {
    field.value().and_then(|n| Seconds::new(n.value()))
}

/// `measurementIssues(metricId, m)`: `Ok` with the typed measurement, or
/// every issue, prefix-free.
pub(crate) fn check_measurement(
    metric: MetricId,
    m: &MeasurementFields,
) -> Result<Measurement, Vec<String>> {
    let want = metric.kind().as_str();
    if m.kind.text() != Some(want) {
        return Err(vec![format!(
            "measurement.kind is {}, but metric \"{metric}\" is measured by kind \"{want}\"",
            quoted(&m.kind)
        )]);
    }
    match metric.shape() {
        MetricShape::Classification(cm) => check_classification(cm, m),
        MetricShape::Duration(dm) => check_duration(dm, m),
        MetricShape::Count(cm) => check_count(cm, m),
    }
}

fn check_classification(
    metric: ClassificationMetric,
    m: &MeasurementFields,
) -> Result<Measurement, Vec<String>> {
    let mut issues = Vec::new();
    let outcome = m.outcome.text().and_then(|o| metric.outcome(o));
    if outcome.is_none() {
        issues.push(format!(
            "measurement.outcome {} is not one of the closed pool for \"{}\" {{ {} }}",
            m.outcome.json_or_undefined(),
            metric.metric(),
            metric.pool_text()
        ));
    }
    let basis = match m.basis.non_blank() {
        None => {
            issues.push("measurement has no classification basis; a correct/false or missed/not_missed classification needs a stated basis — the status or work_status alone is never proof, and an unknown classification is never auto-treated as the tracked outcome".to_string());
            None
        }
        Some(b) => {
            if let Some(r) = non_portable_reason(Some(b)) {
                issues.push(format!("measurement.basis contains {r}"));
            }
            RecordText::new(b).ok()
        }
    };
    match (outcome, basis) {
        (Some(outcome), Some(basis)) if issues.is_empty() => {
            Ok(Measurement::Classification { outcome, basis })
        }
        _ => Err(issues),
    }
}

fn check_duration(
    metric: DurationMetric,
    m: &MeasurementFields,
) -> Result<Measurement, Vec<String>> {
    let mut issues = Vec::new();
    let kind = m.duration_kind.text().and_then(DurationKind::parse);
    if kind.is_none() {
        issues.push(format!(
            "measurement.duration_kind {} is not one of {{ {} }}; calendar length and active effort are distinguished, never summed together",
            m.duration_kind.json_or_undefined(),
            DurationKind::ALL.map(DurationKind::as_str).join(", ")
        ));
    }
    if m.unit.text() != Some("seconds") {
        issues.push(format!(
            "measurement.unit is {}, not \"seconds\"; a duration is always stated in seconds",
            m.unit.json_or_undefined()
        ));
    }
    let seconds = non_negative(&m.seconds);
    if seconds.is_none() {
        issues.push(format!(
            "measurement.seconds {} is not a non-negative number; a negative duration is not measurable",
            m.seconds.json_or_undefined()
        ));
    }
    let mut paused = None;
    if m.paused_seconds.is_present() {
        if kind != Some(DurationKind::Calendar) {
            issues.push(format!(
                "measurement carries paused_seconds but duration_kind is {}, not \"calendar\"; pause accounting subtracts paused time from a CALENDAR span to reach the active-effort figure, which is its own separate observation, not a second subtraction on top of it",
                quoted(&m.duration_kind)
            ));
        } else {
            match non_negative(&m.paused_seconds) {
                None => issues.push(format!(
                    "measurement.paused_seconds {} is not a non-negative number",
                    m.paused_seconds.json_or_undefined()
                )),
                Some(p) => {
                    if seconds.is_some_and(|s| p.get() > s.get()) {
                        issues.push(format!(
                            "measurement.paused_seconds ({}) exceeds the calendar seconds ({}); paused time cannot exceed the span it is paused within",
                            m.paused_seconds.string_or_undefined(),
                            m.seconds.string_or_undefined()
                        ));
                    }
                    paused = Some(p);
                }
            }
        }
    }
    let mut interval = None;
    match (m.start_ts.is_present(), m.end_ts.is_present()) {
        (true, true) => {
            let start = m.start_ts.text().and_then(Timestamp::parse);
            let end = m.end_ts.text().and_then(Timestamp::parse);
            match (start, end) {
                (Some(start), Some(end)) => match TimeInterval::new(start, end) {
                    Some(i) => interval = Some(i),
                    None => issues.push("measurement end_ts is not after start_ts; an interval that ends before (or exactly when) it starts is not a possible time interval".to_string()),
                },
                _ => issues.push("measurement start_ts/end_ts is not a valid timestamp (a well-formed timestamp naming a calendar date and time that actually exist, not merely a digit pattern Date.parse would silently normalise)".to_string()),
            }
        }
        (false, false) => {}
        _ => issues.push("measurement states only one of start_ts / end_ts; an interval names both ends or neither".to_string()),
    }
    match (kind, seconds) {
        (Some(kind), Some(seconds)) if issues.is_empty() => {
            Ok(Measurement::Duration(DurationMeasurement {
                metric,
                kind,
                seconds,
                paused,
                interval,
            }))
        }
        _ => Err(issues),
    }
}

fn check_count(metric: CountMetric, m: &MeasurementFields) -> Result<Measurement, Vec<String>> {
    let mut issues = Vec::new();
    if m.unit.text() != Some("count") {
        issues.push(format!(
            "measurement.unit is {}, not \"count\"",
            m.unit.json_or_undefined()
        ));
    }
    let value = m
        .value
        .value()
        .filter(|n| n.is_integer() && n.value() >= 0.0 && n.value() <= u64::MAX as f64)
        .map(|n| n.value() as u64);
    if value.is_none() {
        issues.push(format!(
            "measurement.value {} is not a non-negative integer",
            m.value.json_or_undefined()
        ));
    }
    match value {
        Some(value) if issues.is_empty() => Ok(Measurement::Count { metric, value }),
        _ => Err(issues),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_contracts::{ForeignValue, ResponseValueKind};

    fn text(s: &str) -> ResponseField<String> {
        ResponseField::Present(s.to_string())
    }

    fn num(v: f64) -> ResponseField<JsonNumber> {
        ResponseField::Present(JsonNumber::new(v, v.to_string()).unwrap())
    }

    fn duration(seconds: f64) -> MeasurementFields {
        MeasurementFields {
            kind: text("duration"),
            duration_kind: text("calendar"),
            unit: text("seconds"),
            seconds: num(seconds),
            ..MeasurementFields::default()
        }
    }

    #[test]
    fn field_evaluation_the_kind_is_fixed_by_the_metric() {
        let issues = check_measurement(MetricId::ReworkReturns, &duration(5.0)).unwrap_err();
        assert_eq!(
            issues,
            ["measurement.kind is \"duration\", but metric \"rework-returns\" is measured by kind \"count\""]
        );
        let absent = MeasurementFields::default();
        assert_eq!(
            check_measurement(MetricId::OwnerCost, &absent).unwrap_err(),
            ["measurement.kind is \"undefined\", but metric \"owner-cost\" is measured by kind \"duration\""]
        );
    }

    #[test]
    fn field_evaluation_a_classification_outcome_belongs_to_its_own_metric() {
        let m = MeasurementFields {
            kind: text("classification"),
            outcome: text("false_stop"),
            basis: text("основание"),
            ..MeasurementFields::default()
        };
        assert!(check_measurement(MetricId::StopCorrectness, &m).is_ok());
        let issues = check_measurement(MetricId::MechanismCorrectness, &m).unwrap_err();
        assert_eq!(issues, ["measurement.outcome \"false_stop\" is not one of the closed pool for \"mechanism-correctness\" { correct, incorrect, unknown }"]);
    }

    #[test]
    fn field_evaluation_paused_time_cannot_exceed_its_calendar_span() {
        let mut m = duration(10.0);
        m.paused_seconds = num(11.0);
        let issues = check_measurement(MetricId::ContextEntryTime, &m).unwrap_err();
        assert!(issues[0]
            .starts_with("measurement.paused_seconds (11) exceeds the calendar seconds (10)"));
        m.duration_kind = text("active");
        let issues = check_measurement(MetricId::ContextEntryTime, &m).unwrap_err();
        assert!(issues[0]
            .starts_with("measurement carries paused_seconds but duration_kind is \"active\""));
    }

    #[test]
    fn field_evaluation_an_interval_must_exist_and_move_forward() {
        let mut m = duration(0.1);
        m.start_ts = text("2026-01-01T00:00:00.100Z");
        m.end_ts = text("2026-01-01T00:00:00.200Z");
        assert!(check_measurement(MetricId::ContextEntryTime, &m).is_ok());
        m.end_ts = text("2026-01-01T00:00:00.100Z");
        assert!(
            check_measurement(MetricId::ContextEntryTime, &m).unwrap_err()[0]
                .starts_with("measurement end_ts is not after start_ts")
        );
        m.end_ts = ResponseField::Absent;
        assert!(
            check_measurement(MetricId::ContextEntryTime, &m).unwrap_err()[0]
                .starts_with("measurement states only one of start_ts / end_ts")
        );
    }

    #[test]
    fn field_evaluation_a_count_is_a_non_negative_integer() {
        let mut m = MeasurementFields {
            kind: text("count"),
            unit: text("count"),
            value: num(2.0),
            ..MeasurementFields::default()
        };
        assert_eq!(
            check_measurement(MetricId::ReworkReturns, &m),
            Ok(Measurement::Count {
                metric: CountMetric::ReworkReturns,
                value: 2
            })
        );
        m.value =
            ResponseField::Foreign(ForeignValue::new(ResponseValueKind::String, "\"2\"", "2"));
        assert_eq!(
            check_measurement(MetricId::ReworkReturns, &m).unwrap_err(),
            ["measurement.value \"2\" is not a non-negative integer"]
        );
    }
}
