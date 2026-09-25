//! Recomputing — never trusting — one metric's aggregate from the typed
//! samples a report names (`computeMetricAggregate`), and comparing it
//! EXACTLY with the aggregate the report states (`sameAggregate`).
//!
//! Deterministic: samples are sorted by observation id before anything is
//! accumulated, so no input permutation changes the result. A rate over an
//! empty denominator has no percentage — expressed by `Option<Percentage>`,
//! never by a sentinel. Two aggregates agree iff their canonical JSON forms
//! would be equal: same shape, same key set, the same numbers.

use super::measurement::Measurement;
use super::metric::{ClassificationMetric, Coverage, MetricId, MetricShape, OutcomeLabel};
use super::resolved::Sample;
use super::time::CalendarDate;

/// A percentage in `[0, 100]`, rounded to two decimals the way every side
/// rounds it (`roundPercentage`), so it compares by exact equality.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Percentage(f64);

impl Percentage {
    /// `numerator / denominator` as a rounded percentage; `None` for a zero
    /// denominator — there is no percentage of nothing.
    pub fn of(numerator: u64, denominator: u64) -> Option<Percentage> {
        (denominator > 0).then(|| {
            Percentage(((numerator as f64 / denominator as f64) * 10_000.0).round() / 100.0)
        })
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

/// `Math.round(x * 100) / 100` for a non-negative mean.
fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// A classification metric's tracked-outcome rate.
#[derive(Debug, Clone, PartialEq)]
pub struct Rate {
    pub numerator: u64,
    pub denominator: u64,
    pub outcome: OutcomeLabel,
    pub note: String,
    pub percentage: Option<Percentage>,
}

/// One duration kind's sample summary.
#[derive(Debug, Clone, PartialEq)]
pub struct DurationSummary {
    pub sample_count: u64,
    pub total_seconds: f64,
    pub mean_seconds: Option<f64>,
}

/// Whether a count aggregate's windows were all complete, all partial, or
/// both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AggregateCoverage {
    Complete,
    Partial,
    Mixed,
}

impl AggregateCoverage {
    pub const fn as_str(self) -> &'static str {
        match self {
            AggregateCoverage::Complete => "complete",
            AggregateCoverage::Partial => "partial",
            AggregateCoverage::Mixed => "mixed",
        }
    }

    /// Complete coverage is not incomplete; partial and mixed are.
    pub const fn is_incomplete(self) -> bool {
        !matches!(self, AggregateCoverage::Complete)
    }
}

/// One metric's recomputed aggregate, shaped by the metric.
#[derive(Debug, Clone, PartialEq)]
pub enum MetricAggregate {
    Classification {
        metric: ClassificationMetric,
        counts: [(OutcomeLabel, u64); 3],
        total: u64,
        rate: Rate,
    },
    Duration {
        calendar: DurationSummary,
        active: DurationSummary,
    },
    Count {
        sample_count: u64,
        total: f64,
        mean: Option<f64>,
        window: Option<(CalendarDate, CalendarDate)>,
        coverage: Option<AggregateCoverage>,
    },
}

impl MetricAggregate {
    /// A classification with nothing classified — its status is unknown.
    pub fn classified_nothing(&self) -> bool {
        matches!(self, MetricAggregate::Classification { total: 0, .. })
    }

    /// A zero `post-acceptance-defects` count over an incomplete window.
    pub(crate) fn zero_over_incomplete_window(&self) -> Option<AggregateCoverage> {
        match self {
            MetricAggregate::Count {
                total,
                coverage: Some(c),
                ..
            } if *total == 0.0 && c.is_incomplete() => Some(*c),
            _ => None,
        }
    }
}

/// `computeMetricAggregate(metric, usable)`.
pub(crate) fn compute(metric: MetricId, usable: &[&Sample]) -> MetricAggregate {
    let mut sorted: Vec<&Sample> = usable.to_vec();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    match metric.shape() {
        MetricShape::Classification(cm) => {
            let mut counts = cm.pool().map(|label| (label, 0u64));
            for s in &sorted {
                if let Measurement::Classification { outcome, .. } = &s.measurement {
                    if let Some(slot) = counts.iter_mut().find(|(l, _)| *l == outcome.label()) {
                        slot.1 += 1;
                    }
                }
            }
            let total: u64 = counts.iter().map(|(_, n)| n).sum();
            let tracked = cm.tracked();
            let numerator = counts
                .iter()
                .find(|(l, _)| *l == tracked)
                .map_or(0, |(_, n)| *n);
            MetricAggregate::Classification {
                metric: cm,
                counts,
                total,
                rate: Rate {
                    numerator,
                    denominator: total,
                    outcome: tracked,
                    note: format!(
                        "доля наблюдений с исходом \"{tracked}\" среди {total} классифицированных; ноль в знаменателе не даёт процента"
                    ),
                    percentage: Percentage::of(numerator, total),
                },
            }
        }
        MetricShape::Duration(_) => {
            let summarise = |kind: super::metric::DurationKind| {
                let values: Vec<f64> = sorted
                    .iter()
                    .filter_map(|s| match &s.measurement {
                        Measurement::Duration(d) if d.kind() == kind => Some(d.seconds().get()),
                        _ => None,
                    })
                    .collect();
                let total_seconds = values.iter().fold(0.0, |acc, v| acc + v);
                let n = values.len() as u64;
                DurationSummary {
                    sample_count: n,
                    total_seconds,
                    mean_seconds: (n > 0).then(|| round2(total_seconds / n as f64)),
                }
            };
            MetricAggregate::Duration {
                calendar: summarise(super::metric::DurationKind::Calendar),
                active: summarise(super::metric::DurationKind::Active),
            }
        }
        MetricShape::Count(_) => {
            let values: Vec<u64> = sorted
                .iter()
                .filter_map(|s| match &s.measurement {
                    Measurement::Count { value, .. } => Some(*value),
                    _ => None,
                })
                .collect();
            let total = values.iter().fold(0.0, |acc, v| acc + *v as f64);
            let n = values.len() as u64;
            let windows: Vec<_> = sorted.iter().filter_map(|s| s.window).collect();
            let (window, coverage) = if metric.requires_window() && !windows.is_empty() {
                let start = windows.iter().map(|(w, _)| w.start()).min();
                let end = windows.iter().map(|(w, _)| w.end()).max();
                let complete = windows.iter().any(|(_, c)| *c == Coverage::Complete);
                let partial = windows.iter().any(|(_, c)| *c == Coverage::Partial);
                let coverage = match (complete, partial) {
                    (true, true) => AggregateCoverage::Mixed,
                    (false, true) => AggregateCoverage::Partial,
                    _ => AggregateCoverage::Complete,
                };
                (start.zip(end), Some(coverage))
            } else {
                (None, None)
            };
            MetricAggregate::Count {
                sample_count: n,
                total,
                mean: (n > 0).then(|| round2(total / n as f64)),
                window,
                coverage,
            }
        }
    }
}

/// A classification rate as a report states it.
#[derive(Debug, Clone, PartialEq)]
pub struct StatedRate {
    pub numerator: u64,
    pub denominator: u64,
    pub percentage: Option<f64>,
    pub outcome: String,
    pub note: String,
}

/// One duration kind's summary as a report states it.
#[derive(Debug, Clone, PartialEq)]
pub struct StatedDurationSummary {
    pub sample_count: u64,
    pub total_seconds: f64,
    pub mean_seconds: Option<f64>,
}

/// An aggregate as a report states it — the schema's three closed shapes.
/// `counts` is the schema's open object: each key with its numeric value,
/// or `None` for a value that is not a number (which never agrees).
#[derive(Debug, Clone, PartialEq)]
pub enum StatedAggregate {
    Classification {
        counts: Vec<(String, Option<f64>)>,
        total: u64,
        rate: StatedRate,
    },
    Duration {
        calendar: StatedDurationSummary,
        active: StatedDurationSummary,
    },
    Count {
        sample_count: u64,
        total: u64,
        mean: Option<f64>,
        window: Option<(String, String)>,
        coverage: Option<String>,
    },
}

fn same_summary(stated: &StatedDurationSummary, computed: &DurationSummary) -> bool {
    stated.sample_count == computed.sample_count
        && stated.total_seconds == computed.total_seconds
        && stated.mean_seconds == computed.mean_seconds
}

/// `sameAggregate(stated, computed)`: exact, key for key.
pub fn agrees(stated: &StatedAggregate, computed: &MetricAggregate) -> bool {
    match (stated, computed) {
        (
            StatedAggregate::Classification {
                counts: stated_counts,
                total: stated_total,
                rate: stated_rate,
            },
            MetricAggregate::Classification {
                counts,
                total,
                rate,
                ..
            },
        ) => {
            stated_counts.len() == counts.len()
                && counts.iter().all(|(label, n)| {
                    stated_counts
                        .iter()
                        .any(|(k, v)| k == label.as_str() && *v == Some(*n as f64))
                })
                && stated_total == total
                && stated_rate.numerator == rate.numerator
                && stated_rate.denominator == rate.denominator
                && stated_rate.outcome == rate.outcome.as_str()
                && stated_rate.note == rate.note
                && stated_rate.percentage == rate.percentage.map(Percentage::get)
        }
        (
            StatedAggregate::Duration {
                calendar: sc,
                active: sa,
            },
            MetricAggregate::Duration { calendar, active },
        ) => same_summary(sc, calendar) && same_summary(sa, active),
        (
            StatedAggregate::Count {
                sample_count: stated_n,
                total: stated_total,
                mean: stated_mean,
                window: stated_window,
                coverage: stated_coverage,
            },
            MetricAggregate::Count {
                sample_count,
                total,
                mean,
                window,
                coverage,
            },
        ) => {
            stated_n == sample_count
                && *stated_total as f64 == *total
                && stated_mean == mean
                && *stated_window == window.map(|(s, e)| (s.to_string(), e.to_string()))
                && stated_coverage.as_deref() == coverage.map(AggregateCoverage::as_str)
        }
        _ => false,
    }
}
