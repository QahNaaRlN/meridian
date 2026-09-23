//! `field-evaluation-report`: a DETERMINISTIC aggregation, scoped to a
//! project workspace over a stated period, built from a pinned set of
//! included observations and an explicit, reasoned set of excluded ones —
//! never a single rolled-up score (`evaluateReportBody`).
//!
//! Membership (same workspace, same period, no double counting, supersedes,
//! sample completeness) reads each resolved observation's own fields as the
//! transformer stated them; every aggregate is recomputed from the typed
//! samples (`super::resolved::contribution`) and compared exactly.

use std::collections::HashSet;

use crate::evidence::{fail, record_subject};
use crate::ordered::{OrderedMap, OrderedSet};
use crate::run_contracts::envelope::RecordFamily;
use crate::run_contracts::pinned_ref::check_pin_shape;
use crate::run_contracts::resolution::check_resolved_entry;
use crate::run_contracts::{
    PinnedRecordKind, PinnedRef, RecordEnvelope, RecordText, ResolutionCatalogue, ResolvedEntry,
};
use crate::task_contracts::non_portable_reason;
use crate::types::{Diagnostic, SemanticId};

use super::aggregate::{agrees, compute, MetricAggregate, StatedAggregate};
use super::metric::{MetricId, MetricReportStatus};
use super::resolved::{contribution, observation_issues, Contribution, Sample};
use super::time::{CalendarDate, DateWindow};
use super::FieldScope;

const FAMILY: RecordFamily = RecordFamily::FieldEvaluationReport;

/// One excluded observation: its pinned reference and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExclusionInput {
    pub reference: PinnedRef,
    pub reason: RecordText,
}

/// One per-metric entry as the report states it.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricReportInput {
    pub metric: MetricId,
    pub status: MetricReportStatus,
    pub status_reason: Option<RecordText>,
    pub aggregate: StatedAggregate,
    pub samples: Vec<SemanticId>,
    pub limitations: Vec<RecordText>,
}

/// One schema-clean report record.
#[derive(Debug, Clone, PartialEq)]
pub struct ReportInput {
    pub envelope: RecordEnvelope,
    pub scope: FieldScope,
    pub workspace: SemanticId,
    pub period_start: RecordText,
    pub period_end: RecordText,
    pub included: Vec<PinnedRef>,
    pub excluded: Vec<ExclusionInput>,
    pub per_metric: Vec<MetricReportInput>,
}

/// One metric's accepted conclusion: its recomputed aggregate.
#[derive(Debug, Clone, PartialEq)]
pub struct MetricReport {
    pub metric: MetricId,
    pub status: MetricReportStatus,
    pub aggregate: MetricAggregate,
    pub samples: Vec<SemanticId>,
    pub limitations: Vec<RecordText>,
}

/// An accepted report: eight metrics, each recomputed from its resolved,
/// named sample.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldEvaluationReport {
    envelope: RecordEnvelope,
    workspace: SemanticId,
    period: DateWindow,
    included: Vec<PinnedRef>,
    excluded: Vec<ExclusionInput>,
    per_metric: Vec<MetricReport>,
}

impl FieldEvaluationReport {
    pub fn id(&self) -> &SemanticId {
        &self.envelope.id
    }
    pub fn workspace(&self) -> &SemanticId {
        &self.workspace
    }
    pub fn period(&self) -> DateWindow {
        self.period
    }
    pub fn included(&self) -> &[PinnedRef] {
        &self.included
    }
    pub fn excluded(&self) -> &[ExclusionInput] {
        &self.excluded
    }
    /// The eight metrics, in the report's order.
    pub fn per_metric(&self) -> &[MetricReport] {
        &self.per_metric
    }
}

/// One resolved included or excluded observation.
struct Resolved<'c> {
    at: usize,
    entry: Option<&'c ResolvedEntry>,
}

impl<'c> Resolved<'c> {
    fn id(&self) -> Option<&'c str> {
        self.entry.and_then(|e| e.id.text())
    }
    fn metric(&self) -> Option<&'c str> {
        self.entry.and_then(|e| e.observation.metric_id.text())
    }
}

/// The report body. Returns the accepted report only when no problem was
/// found.
pub(super) fn check_report(
    input: ReportInput,
    catalogue: Option<&ResolutionCatalogue>,
    problems: &mut Vec<Diagnostic>,
) -> Option<FieldEvaluationReport> {
    let before = problems.len();
    let id = input.envelope.id.as_str();
    let subject = record_subject(FAMILY, id);
    let workspace = input.workspace.as_str();

    let period = match (
        CalendarDate::parse(input.period_start.as_str()),
        CalendarDate::parse(input.period_end.as_str()),
    ) {
        (Some(start), Some(end)) => {
            let window = DateWindow::new(start, end);
            if window.is_none() {
                problems.push(fail(format!(
                    "{subject} period end_date is before start_date; an impossible reporting window"
                )));
            }
            window
        }
        _ => {
            problems.push(fail(format!(
                "{subject} period does not carry two valid dates"
            )));
            None
        }
    };

    let mut seen_included = HashSet::new();
    for (i, r) in input.included.iter().enumerate() {
        check_pin_shape(
            FAMILY,
            id,
            "included_observation",
            PinnedRecordKind::FieldEvaluationObservation,
            r,
            problems,
        );
        let reference = r.reference.as_str();
        if !seen_included.insert(reference) {
            problems.push(fail(format!(
                "{subject} included_observations repeats reference \"{reference}\" at #{i}; the same observation is not counted twice"
            )));
        }
    }
    let mut seen_excluded = HashSet::new();
    for (i, x) in input.excluded.iter().enumerate() {
        check_pin_shape(
            FAMILY,
            id,
            "excluded_observation",
            PinnedRecordKind::FieldEvaluationObservation,
            &x.reference,
            problems,
        );
        if x.reason.is_blank() {
            problems.push(fail(format!(
                "{subject} excluded_observations[{i}] has no exclusion_reason; an exclusion is explicit and reasoned, never a silent drop"
            )));
        } else if let Some(r) = non_portable_reason(Some(x.reason.as_str())) {
            problems.push(fail(format!(
                "{subject} excluded_observations[{i}] exclusion_reason contains {r}"
            )));
        }
        let reference = x.reference.reference.as_str();
        if !seen_excluded.insert(reference) {
            problems.push(fail(format!(
                "{subject} excluded_observations repeats reference \"{reference}\" at #{i}; a repeated excluded reference is not legalised by a different exclusion_reason"
            )));
        }
        if seen_included.contains(reference) {
            problems.push(fail(format!(
                "{subject} reference \"{reference}\" appears in both included_observations and excluded_observations; an observation is either included or excluded, never both"
            )));
        }
    }

    let mut included: Vec<Resolved> = Vec::new();
    let mut excluded: Vec<Resolved> = Vec::new();
    match catalogue {
        Some(catalogue) => {
            for (lists, name, refs) in [
                (&mut included, "included", input.included.iter().collect::<Vec<_>>()),
                (
                    &mut excluded,
                    "excluded",
                    input.excluded.iter().map(|x| &x.reference).collect(),
                ),
            ] {
                for (at, r) in refs.into_iter().enumerate() {
                    let entry = check_resolved_entry(
                        FAMILY,
                        id,
                        &format!("{name}_observation"),
                        PinnedRecordKind::FieldEvaluationObservation,
                        r,
                        catalogue,
                        problems,
                    );
                    if let Some(e) = entry {
                        for issue in observation_issues(e) {
                            problems.push(fail(format!(
                                "{subject} {name}_observations[{at}]: resolved observation {issue}"
                            )));
                        }
                    }
                    lists.push(Resolved { at, entry });
                }
            }
        }
        None => problems.push(fail(format!(
            "{subject} cannot be verified: no external record resolver was supplied; every included and excluded observation is resolved OUTSIDE the report and checked against it"
        ))),
    }

    // Double counting: no observation id repeated.
    let mut by_id: OrderedMap<&str, &ResolvedEntry> = OrderedMap::new();
    for r in &included {
        let (Some(oid), Some(entry)) = (r.id(), r.entry) else {
            continue;
        };
        if !by_id.insert(oid, entry) {
            problems.push(fail(format!(
                "{subject} included_observations resolves two entries to the SAME observation id \"{oid}\"; a repeated record is not counted twice"
            )));
        }
    }
    let mut excluded_seen = HashSet::new();
    for r in &excluded {
        let Some(oid) = r.id() else { continue };
        let at = r.at;
        if !excluded_seen.insert(oid) {
            problems.push(fail(format!(
                "{subject} excluded_observations[{at}] resolves to observation \"{oid}\", which another excluded_observations entry already resolves to; two different references naming the same observation are not both excluded"
            )));
        }
        if by_id.contains(&oid) {
            problems.push(fail(format!(
                "{subject} excluded_observations[{at}] resolves to observation \"{oid}\", which is also an included observation; an observation is either included or excluded, never both"
            )));
        }
    }
    // Comparability: same workspace, observed inside the period, and no
    // superseded + superseding pair.
    for r in &included {
        let Some(entry) = r.entry else { continue };
        let o = &entry.observation;
        let at = r.at;
        if let Some(ws) = o.workspace_id.text() {
            if ws != workspace {
                problems.push(fail(format!(
                    "{subject} included_observations[{at}] belongs to workspace \"{ws}\", not this report's workspace \"{workspace}\"; samples from different workspaces are not mixed without an explicit comparability rule"
                )));
            }
        }
        if let (Some(period), Some(observed)) =
            (period, o.observed_at.text().and_then(CalendarDate::parse))
        {
            if !period.contains(observed) {
                problems.push(fail(format!(
                    "{subject} included_observations[{at}] was observed at \"{observed}\", outside the report period [{}, {}]; samples from different periods are not mixed without an explicit comparability rule",
                    input.period_start, input.period_end
                )));
            }
        }
        if let Some(sup) = superseded_id(entry) {
            if by_id.contains(&sup) {
                problems.push(fail(format!(
                    "{subject} includes both observation \"{}\" and the observation \"{sup}\" it supersedes; the superseded record's facts are replaced, not additionally counted",
                    entry.id.string_or_undefined()
                )));
            }
        }
    }

    // per_metric: the eight metrics, each exactly once.
    let mut by_metric: OrderedMap<MetricId, &MetricReportInput> = OrderedMap::new();
    for pm in &input.per_metric {
        if !by_metric.insert(pm.metric, pm) {
            problems.push(fail(format!(
                "{subject} per_metric repeats metric_id \"{}\"",
                pm.metric
            )));
        }
    }
    for metric in MetricId::ALL {
        if !by_metric.contains(&metric) {
            problems.push(fail(format!(
                "{subject} per_metric omits metric_id \"{metric}\"; all eight characteristics are represented, even when the result is \"unknown\""
            )));
        }
    }
    for (metric, pm) in by_metric.iter() {
        check_metric_statement(&subject, *metric, pm, &by_id, problems);
    }

    let mut recomputed: OrderedMap<MetricId, MetricAggregate> = OrderedMap::new();
    if catalogue.is_some() {
        let superseded: HashSet<&str> = included
            .iter()
            .filter_map(|r| r.entry.and_then(superseded_id))
            .filter(|sup| by_id.contains(sup))
            .collect();
        for metric in MetricId::ALL {
            let Some(pm) = by_metric.get(&metric) else {
                continue;
            };
            if let Some(aggregate) = check_metric_recomputation(
                &subject,
                metric,
                pm,
                &by_id,
                &superseded,
                &excluded,
                problems,
            ) {
                recomputed.insert(metric, aggregate);
            }
        }
    }

    if problems.len() != before {
        return None;
    }
    let period = period?;
    let per_metric = input
        .per_metric
        .iter()
        .map(|pm| {
            recomputed.get(&pm.metric).map(|aggregate| MetricReport {
                metric: pm.metric,
                status: pm.status,
                aggregate: aggregate.clone(),
                samples: pm.samples.clone(),
                limitations: pm.limitations.clone(),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(FieldEvaluationReport {
        envelope: input.envelope,
        workspace: input.workspace,
        period,
        included: input.included,
        excluded: input.excluded,
        per_metric,
    })
}

/// The observation id an entry supersedes, when it states one.
fn superseded_id(entry: &ResolvedEntry) -> Option<&str> {
    entry
        .observation
        .supersedes_id
        .value()
        .and_then(|s| s.as_deref())
}

/// Per-metric status/reason consistency and `sample_observation_ids`
/// membership.
fn check_metric_statement(
    subject: &str,
    metric: MetricId,
    pm: &MetricReportInput,
    by_id: &OrderedMap<&str, &ResolvedEntry>,
    problems: &mut Vec<Diagnostic>,
) {
    let status = pm.status.as_str();
    if pm.status == MetricReportStatus::Known {
        if pm.status_reason.is_some() {
            problems.push(fail(format!(
                "{subject} per_metric \"{metric}\" is \"known\" but carries a status_reason; a known value needs no absence explanation"
            )));
        }
    } else if pm.status_reason.as_ref().is_none_or(RecordText::is_blank) {
        problems.push(fail(format!(
            "{subject} per_metric \"{metric}\" is \"{status}\" but states no status_reason"
        )));
    }
    let mut seen = HashSet::new();
    for sid in &pm.samples {
        let sid = sid.as_str();
        if !seen.insert(sid) {
            problems.push(fail(format!(
                "{subject} per_metric \"{metric}\" sample_observation_ids repeats \"{sid}\""
            )));
        }
        match by_id.get(&sid) {
            None => problems.push(fail(format!(
                "{subject} per_metric \"{metric}\" sample_observation_ids names \"{sid}\", which is not an included observation"
            ))),
            Some(entry) if entry.observation.metric_id.text() != Some(metric.as_str()) => {
                problems.push(fail(format!(
                    "{subject} per_metric \"{metric}\" sample_observation_ids names \"{sid}\", which is an observation of metric \"{}\", not \"{metric}\"",
                    entry.observation.metric_id.string_or_undefined()
                )))
            }
            Some(_) => {}
        }
    }
    for (j, lm) in pm.limitations.iter().enumerate() {
        if lm.is_blank() {
            problems.push(fail(format!(
                "{subject} per_metric \"{metric}\" limitations[{j}] is empty or whitespace-only"
            )));
        } else if let Some(r) = non_portable_reason(Some(lm.as_str())) {
            problems.push(fail(format!(
                "{subject} per_metric \"{metric}\" limitations[{j}] contains {r}"
            )));
        }
    }
}

/// Sample completeness, the expected status and the EXACT recomputed
/// aggregate of one metric. Returns the recomputed aggregate when it could
/// be computed from typed samples.
#[allow(clippy::too_many_arguments)]
fn check_metric_recomputation(
    subject: &str,
    metric: MetricId,
    pm: &MetricReportInput,
    by_id: &OrderedMap<&str, &ResolvedEntry>,
    superseded: &HashSet<&str>,
    excluded: &[Resolved<'_>],
    problems: &mut Vec<Diagnostic>,
) -> Option<MetricAggregate> {
    let samples: OrderedSet<&str> = pm.samples.iter().map(SemanticId::as_str).collect();
    let of_metric =
        |entry: &ResolvedEntry| entry.observation.metric_id.text() == Some(metric.as_str());

    let actual: OrderedSet<&str> = by_id
        .iter()
        .filter(|(oid, entry)| of_metric(entry) && !superseded.contains(**oid))
        .map(|(oid, _)| *oid)
        .collect();
    for oid in actual.iter() {
        if !samples.contains(oid) {
            problems.push(fail(format!(
                "{subject} per_metric \"{metric}\" sample_observation_ids omits included observation \"{oid}\"; an included observation is either counted in its metric's sample or formally excluded with a reason, never silently left out of the aggregate"
            )));
        }
    }
    for oid in samples.iter() {
        if !actual.contains(oid) {
            problems.push(fail(format!(
                "{subject} per_metric \"{metric}\" sample_observation_ids names \"{oid}\", which is not an included observation of this metric (or was superseded); a claimed sample member is not one of the report's own included observations"
            )));
        }
    }

    let mut usable: Vec<Sample> = Vec::new();
    let mut any_not_applicable = false;
    let mut any_other = false;
    let mut computable = true;
    for (oid, entry) in by_id.iter() {
        if !of_metric(entry) || !samples.contains(oid) {
            continue;
        }
        match contribution(entry, oid, metric) {
            Contribution::Usable(sample) => usable.push(sample),
            Contribution::NotApplicable => any_not_applicable = true,
            Contribution::Other => any_other = true,
            Contribution::NotComputable => computable = false,
        }
    }
    // A usable observation the closed contract refused has already been
    // reported; its values are never aggregated.
    if !computable {
        return None;
    }
    let usable: Vec<&Sample> = usable.iter().collect();
    let expected = compute(metric, &usable);
    let expected_status = if !usable.is_empty() {
        if expected.classified_nothing() {
            MetricReportStatus::Unknown
        } else {
            MetricReportStatus::Known
        }
    } else if any_not_applicable && !any_other {
        MetricReportStatus::NotApplicable
    } else {
        MetricReportStatus::Unknown
    };
    if pm.status != expected_status {
        problems.push(fail(format!(
            "{subject} per_metric \"{metric}\" states status \"{}\", but the resolved, named sample implies \"{}\"; the report's conclusion must agree with what its own observations show",
            pm.status.as_str(),
            expected_status.as_str()
        )));
    }
    if !agrees(&pm.aggregate, &expected) {
        problems.push(fail(format!(
            "{subject} per_metric \"{metric}\" aggregate does not match the value recomputed from its resolved sample; a stated aggregate that diverges from the actual observations is rejected, whatever the status"
        )));
    }
    if metric == MetricId::PostAcceptanceDefects && pm.limitations.is_empty() {
        if let Some(coverage) = expected.zero_over_incomplete_window() {
            problems.push(fail(format!(
                "{subject} per_metric \"post-acceptance-defects\" shows zero defects over a \"{}\" observation window but carries no limitation; a zero count over incomplete coverage is not proof of no defects and must say so",
                coverage.as_str()
            )));
        }
    }
    let excluded_for_metric = excluded.iter().any(|r| r.metric() == Some(metric.as_str()));
    if excluded_for_metric && pm.limitations.is_empty() {
        problems.push(fail(format!(
            "{subject} per_metric \"{metric}\" has an excluded observation of this metric but discloses no limitation; an exclusion that would have contributed to a metric is disclosed, not hidden"
        )));
    }
    Some(expected)
}
