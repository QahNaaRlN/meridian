//! A resolved observation — the transformer's view of one
//! `field-evaluation-observation` a report includes or excludes — validated
//! BEFORE it is counted (`resolvedObservationIssues`), and the typed
//! [`Contribution`] it makes to its metric's aggregate.
//!
//! The report's membership rules (same workspace, same period, supersedes,
//! sample ids) read the response's own fields exactly as stated; only the
//! aggregate is built from typed values. An `observed` observation whose
//! measurement is absent, `null`, not an object or refused by the closed
//! contract — or, for `post-acceptance-defects`, whose window is absent,
//! `null`, not an object or invalid — contributes
//! [`Contribution::NotComputable`]: the issue itself already rejects the
//! report, and its metric's aggregate and status are never recomputed from
//! values the contract refused.

use crate::run_contracts::{ObservationResponse, ResolvedEntry, ResponseField, WindowFields};
use crate::types::SemanticId;

use super::measurement::{check_measurement, Measurement};
use super::metric::{Coverage, MetricId, ObservationStatus};
use super::time::{CalendarDate, DateWindow};

/// `"${x}"` of a field.
fn quoted<T: crate::run_contracts::response::ResponseText>(field: &ResponseField<T>) -> String {
    format!("\"{}\"", field.string_or_undefined())
}

fn valid_date(field: &ResponseField<String>) -> Option<CalendarDate> {
    field.text().and_then(CalendarDate::parse)
}

/// `resolvedObservationIssues(entry)`: every issue, prefix-free.
pub(crate) fn observation_issues(entry: &ResolvedEntry) -> Vec<String> {
    let o = &entry.observation;
    let mut issues = Vec::new();
    let metric = o.metric_id.text().and_then(MetricId::parse);
    if metric.is_none() {
        issues.push(format!(
            "metric_id {} is not one of the eight closed characteristics",
            o.metric_id.json_or_undefined()
        ));
    }
    let status = o.status.text().and_then(ObservationStatus::parse);
    if status.is_none() {
        issues.push(format!(
            "status {} is not one of the closed pool {{ {} }}",
            o.status.json_or_undefined(),
            ObservationStatus::pool_text()
        ));
    }
    if o.workspace_id
        .non_blank()
        .is_none_or(|w| SemanticId::new(w).is_err())
    {
        issues.push(format!(
            "workspace_id {} is missing or not a stable semantic id",
            o.workspace_id.json_or_undefined()
        ));
    }
    if valid_date(&o.observed_at).is_none() {
        issues.push(format!(
            "observed_at {} is not a valid calendar date",
            o.observed_at.json_or_undefined()
        ));
    }
    if status == Some(ObservationStatus::Observed) {
        match &o.measurement {
            ResponseField::Present(Some(fields)) => {
                if let Some(metric) = metric {
                    if let Err(found) = check_measurement(metric, fields) {
                        issues.extend(found.into_iter().map(|i| format!("measurement {i}")));
                    }
                }
            }
            _ => issues.push("status is \"observed\" but measurement is not an object".to_string()),
        }
    } else if o.measurement.is_non_null() {
        issues.push(format!(
            "status is {} but measurement is present",
            quoted(&o.status)
        ));
    }
    if o.supersedes_id.is_non_null()
        && o.supersedes_id
            .value()
            .and_then(|s| s.as_deref())
            .is_none_or(|s| SemanticId::new(s).is_err())
    {
        issues.push(format!(
            "supersedes_id {} is present but neither null nor a stable semantic id",
            o.supersedes_id.json_or_undefined()
        ));
    }
    let requires_window = metric.is_some_and(MetricId::requires_window);
    let has_window = o.observation_period.is_non_null();
    if status == Some(ObservationStatus::Observed) && requires_window {
        if !has_window {
            issues.push(format!(
                "metric {} requires an observation_period when observed",
                quoted(&o.metric_id)
            ));
        } else {
            let empty = WindowFields::default();
            let w = match &o.observation_period {
                ResponseField::Present(Some(w)) => w,
                _ => &empty,
            };
            match (valid_date(&w.start_date), valid_date(&w.end_date)) {
                (Some(start), Some(end)) => {
                    if DateWindow::new(start, end).is_none() {
                        issues.push("observation_period end_date is before start_date".to_string());
                    }
                }
                _ => issues.push("observation_period does not carry two valid dates".to_string()),
            }
            if coverage(o).is_none() {
                issues.push(format!(
                    "coverage {} is not one of the closed pool {{ {} }}",
                    o.coverage.json_or_undefined(),
                    Coverage::ALL.map(Coverage::as_str).join(", ")
                ));
            }
        }
    } else if has_window || o.coverage.is_non_null() {
        issues.push(format!(
            "carries observation_period/coverage but metric {} at status {} does not use an observation window",
            quoted(&o.metric_id),
            quoted(&o.status)
        ));
    }
    issues
}

fn coverage(o: &ObservationResponse) -> Option<Coverage> {
    o.coverage
        .value()
        .and_then(|c| c.as_deref())
        .and_then(Coverage::parse)
}

/// One usable observation's aggregation data.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Sample {
    /// The observation's id — the aggregation order.
    pub id: String,
    pub measurement: Measurement,
    /// `post-acceptance-defects` only: its window and coverage.
    pub window: Option<(DateWindow, Coverage)>,
}

/// What one resolved observation contributes to its metric
/// (`resolvedIsUsable` and the status partition of the report check).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Contribution {
    Usable(Sample),
    NotApplicable,
    Other,
    /// `observed`, but its measurement — or its required window — is
    /// absent, `null`, not an object or refused by the closed contract.
    NotComputable,
}

/// The contribution of one resolved observation to `metric`.
pub(crate) fn contribution(entry: &ResolvedEntry, id: &str, metric: MetricId) -> Contribution {
    let o = &entry.observation;
    match o.status.text() {
        Some(s) if s == ObservationStatus::Observed.as_str() => {}
        Some(s) if s == ObservationStatus::NotApplicable.as_str() => {
            return Contribution::NotApplicable
        }
        _ => return Contribution::Other,
    }
    // Observed: only a measurement object that passes the closed contract
    // contributes; an absent, `null` or non-object measurement is refused.
    let ResponseField::Present(Some(fields)) = &o.measurement else {
        return Contribution::NotComputable;
    };
    let Ok(measurement) = check_measurement(metric, fields) else {
        return Contribution::NotComputable;
    };
    // A metric that requires a window contributes only with a present,
    // valid window and coverage; absent, `null` or non-object is refused.
    let window = if metric.requires_window() {
        let ResponseField::Present(Some(w)) = &o.observation_period else {
            return Contribution::NotComputable;
        };
        let window = valid_date(&w.start_date)
            .zip(valid_date(&w.end_date))
            .and_then(|(s, e)| DateWindow::new(s, e));
        match window.zip(coverage(o)) {
            Some(pair) => Some(pair),
            None => return Contribution::NotComputable,
        }
    } else {
        None
    };
    Contribution::Usable(Sample {
        id: id.to_string(),
        measurement,
        window,
    })
}
