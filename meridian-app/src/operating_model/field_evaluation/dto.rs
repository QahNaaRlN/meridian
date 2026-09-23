//! Closed transport DTOs for one field-evaluation record
//! (`registries/operating-model/field-evaluation.schema.json`) and their
//! conversion into [`meridian_core::field_evaluation::FieldEvaluationInput`].
//! The record's `record_type` selects the payload DTO, exactly as the
//! schema's `allOf`/`if` selects the payload definition. Every object is
//! `#[serde(deny_unknown_fields)]`; the schema's `oneOf` measurement and
//! aggregate shapes are each one closed DTO over the union of their keys,
//! discriminated by `kind`. A conversion error is drift.

use std::collections::BTreeMap;

use meridian_core::evidence::{EvidenceKind, PinnedEvidence};
use meridian_core::field_evaluation::{
    Coverage, ExclusionInput, FieldEvaluationInput, FieldScope, MetricId, MetricReportInput,
    MetricReportStatus, ObservationInput, ObservationStatus, ReportInput, StatedAggregate,
    StatedDurationSummary, StatedRate, WindowInput,
};
use meridian_core::run_contracts::{MeasurementFields, ResponseField, RunStateScope};
use meridian_core::types::WorkspaceId;
use serde::Deserialize;
use serde_json::{Number, Value};

use crate::operating_model::run_contract_boundary::envelope::{
    closed, optional_text, portable, record_envelope, semantic_id, sha256, text, AuthorityDto,
    Converted, EnvelopeParts, OriginDto, PinnedRefDto,
};
use meridian_core::run_contracts::{JsonNumber, RunId};

const OBSERVATION: &str = "field-evaluation-observation";
const REPORT: &str = "field-evaluation-report";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FieldRecordDto {
    #[serde(rename = "$schema")]
    declared_schema: String,
    schema_version: u64,
    id: String,
    title: String,
    record_type: String,
    scope: ScopeDto,
    origin: OriginDto,
    authority: AuthorityDto,
    /// Selected by `record_type` in [`FieldRecordDto::into_input`].
    payload: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScopeDto {
    #[serde(rename = "type")]
    scope_type: String,
    id: String,
    #[serde(default)]
    workspace_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WindowDto {
    start_date: String,
    end_date: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MeasurementDto {
    kind: String,
    #[serde(default)]
    outcome: Option<String>,
    #[serde(default)]
    basis: Option<String>,
    #[serde(default)]
    duration_kind: Option<String>,
    #[serde(default)]
    unit: Option<String>,
    #[serde(default)]
    seconds: Option<Number>,
    #[serde(default)]
    paused_seconds: Option<Number>,
    #[serde(default)]
    start_ts: Option<String>,
    #[serde(default)]
    end_ts: Option<String>,
    #[serde(default)]
    value: Option<Number>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceDto {
    id: String,
    kind: String,
    reference: String,
    #[serde(default)]
    revision: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
    summary: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ObservationPayloadDto {
    execution_run_ref: PinnedRefDto,
    metric_id: String,
    observed_at: String,
    status: String,
    #[serde(default)]
    status_reason: Option<String>,
    #[serde(default)]
    measurement: Option<MeasurementDto>,
    #[serde(default)]
    observation_period: Option<WindowDto>,
    #[serde(default)]
    coverage: Option<String>,
    #[serde(default)]
    coverage_note: Option<String>,
    evidence: Vec<EvidenceDto>,
    #[serde(default)]
    supersedes: Option<PinnedRefDto>,
    #[serde(default)]
    correction_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExclusionDto {
    #[serde(rename = "ref")]
    reference: PinnedRefDto,
    exclusion_reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RateDto {
    numerator: u64,
    denominator: u64,
    #[serde(default)]
    percentage: Option<f64>,
    outcome: String,
    note: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SummaryDto {
    sample_count: u64,
    total_seconds: f64,
    #[serde(default)]
    mean_seconds: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AggregateDto {
    kind: String,
    /// The schema's open `counts` object.
    #[serde(default)]
    counts: Option<BTreeMap<String, Value>>,
    #[serde(default)]
    total: Option<u64>,
    #[serde(default)]
    rate: Option<RateDto>,
    #[serde(default)]
    calendar: Option<SummaryDto>,
    #[serde(default)]
    active: Option<SummaryDto>,
    #[serde(default)]
    sample_count: Option<u64>,
    #[serde(default)]
    mean: Option<f64>,
    #[serde(default)]
    observation_window: Option<WindowDto>,
    #[serde(default)]
    coverage: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PerMetricDto {
    metric_id: String,
    status: String,
    #[serde(default)]
    status_reason: Option<String>,
    aggregate: AggregateDto,
    sample_observation_ids: Vec<String>,
    limitations: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportPayloadDto {
    workspace_id: String,
    period: WindowDto,
    included_observations: Vec<PinnedRefDto>,
    excluded_observations: Vec<ExclusionDto>,
    per_metric: Vec<PerMetricDto>,
}

fn present<T>(value: Option<T>) -> ResponseField<T> {
    value.map_or(ResponseField::Absent, ResponseField::Present)
}

fn number(field: &str, value: Option<Number>) -> Converted<ResponseField<JsonNumber>> {
    value
        .map(|n| {
            n.as_f64()
                .and_then(|v| JsonNumber::new(v, n.to_string()))
                .ok_or_else(|| format!("{field}: {n} is not a finite number"))
        })
        .transpose()
        .map(present)
}

impl MeasurementDto {
    fn into_fields(self) -> Converted<MeasurementFields> {
        Ok(MeasurementFields {
            kind: ResponseField::Present(self.kind),
            outcome: present(self.outcome),
            basis: present(self.basis),
            duration_kind: present(self.duration_kind),
            unit: present(self.unit),
            seconds: number("measurement.seconds", self.seconds)?,
            paused_seconds: number("measurement.paused_seconds", self.paused_seconds)?,
            start_ts: present(self.start_ts),
            end_ts: present(self.end_ts),
            value: number("measurement.value", self.value)?,
        })
    }
}

impl WindowDto {
    fn into_input(self, field: &str) -> Converted<WindowInput> {
        Ok(WindowInput {
            start_date: text(field, self.start_date)?,
            end_date: text(field, self.end_date)?,
        })
    }
}

fn scope(dto: ScopeDto) -> Converted<FieldScope> {
    match (dto.scope_type.as_str(), dto.workspace_id) {
        ("run-state", Some(workspace)) => {
            let run = RunId::new(dto.id).map_err(|e| format!("scope.id: {e}"))?;
            let workspace =
                WorkspaceId::new(workspace).map_err(|e| format!("scope.workspace_id: {e}"))?;
            Ok(FieldScope::RunState(RunStateScope::new(run, workspace)))
        }
        ("project-workspace", None) => Ok(FieldScope::ProjectWorkspace(semantic_id(
            "scope.id", dto.id,
        )?)),
        (other, workspace) => Err(format!(
            "scope: \"{other}\" with workspace_id {} is outside the schema's two scopes",
            if workspace.is_some() {
                "present"
            } else {
                "absent"
            }
        )),
    }
}

fn evidence(e: EvidenceDto) -> Converted<PinnedEvidence> {
    let kind = closed("evidence.kind", &e.kind, EvidenceKind::parse)?;
    if kind == EvidenceKind::SpecialisedEvidenceRecord {
        return Err(
            "evidence.kind: specialised-evidence-record is outside the field-evaluation pool"
                .to_string(),
        );
    }
    Ok(PinnedEvidence {
        id: semantic_id("evidence.id", e.id)?,
        kind,
        reference: portable("evidence.reference", e.reference)?,
        revision: optional_text("evidence.revision", e.revision)?,
        sha256: sha256("evidence.sha256", e.sha256)?,
        summary: text("evidence.summary", e.summary)?,
    })
}

fn texts(
    field: &str,
    values: Vec<String>,
) -> Converted<Vec<meridian_core::run_contracts::RecordText>> {
    values.into_iter().map(|v| text(field, v)).collect()
}

impl AggregateDto {
    fn into_stated(self) -> Converted<StatedAggregate> {
        let shape = |ok: bool| {
            if ok {
                Ok(())
            } else {
                Err(format!(
                    "aggregate: the \"{}\" shape carries a key of another shape or lacks a required one",
                    self.kind
                ))
            }
        };
        match self.kind.as_str() {
            "classification" => {
                shape(
                    self.calendar.is_none()
                        && self.active.is_none()
                        && self.sample_count.is_none()
                        && self.mean.is_none()
                        && self.observation_window.is_none()
                        && self.coverage.is_none(),
                )?;
                let (Some(counts), Some(total), Some(rate)) = (self.counts, self.total, self.rate)
                else {
                    return Err(
                        "aggregate: a classification lacks counts, total or rate".to_string()
                    );
                };
                Ok(StatedAggregate::Classification {
                    counts: counts
                        .into_iter()
                        .map(|(k, v)| (k, v.as_f64().filter(|_| v.is_number())))
                        .collect(),
                    total,
                    rate: StatedRate {
                        numerator: rate.numerator,
                        denominator: rate.denominator,
                        percentage: rate.percentage,
                        outcome: rate.outcome,
                        note: rate.note,
                    },
                })
            }
            "duration" => {
                shape(
                    self.counts.is_none()
                        && self.total.is_none()
                        && self.rate.is_none()
                        && self.sample_count.is_none()
                        && self.mean.is_none()
                        && self.observation_window.is_none()
                        && self.coverage.is_none(),
                )?;
                let (Some(calendar), Some(active)) = (self.calendar, self.active) else {
                    return Err("aggregate: a duration lacks calendar or active".to_string());
                };
                let summary = |s: SummaryDto| StatedDurationSummary {
                    sample_count: s.sample_count,
                    total_seconds: s.total_seconds,
                    mean_seconds: s.mean_seconds,
                };
                Ok(StatedAggregate::Duration {
                    calendar: summary(calendar),
                    active: summary(active),
                })
            }
            "count" => {
                shape(
                    self.counts.is_none()
                        && self.rate.is_none()
                        && self.calendar.is_none()
                        && self.active.is_none(),
                )?;
                let (Some(sample_count), Some(total)) = (self.sample_count, self.total) else {
                    return Err("aggregate: a count lacks sample_count or total".to_string());
                };
                Ok(StatedAggregate::Count {
                    sample_count,
                    total,
                    mean: self.mean,
                    window: self.observation_window.map(|w| (w.start_date, w.end_date)),
                    coverage: self.coverage,
                })
            }
            other => Err(format!(
                "aggregate.kind: \"{other}\" is outside the closed pool"
            )),
        }
    }
}

impl FieldRecordDto {
    pub(super) fn into_input(self) -> Converted<FieldEvaluationInput> {
        let record_type = self.record_type.clone();
        let envelope = record_envelope(
            EnvelopeParts {
                declared_schema: self.declared_schema,
                schema_version: self.schema_version,
                id: self.id,
                title: self.title,
                record_type: self.record_type,
                origin: self.origin,
                authority: self.authority,
            },
            &record_type,
        )?;
        let scope = scope(self.scope)?;
        match record_type.as_str() {
            OBSERVATION => {
                let p = ObservationPayloadDto::deserialize(&self.payload)
                    .map_err(|e| format!("payload: {e}"))?;
                Ok(FieldEvaluationInput::Observation(Box::new(
                    ObservationInput {
                        envelope,
                        scope,
                        run: p.execution_run_ref.into_pin("execution_run_ref")?,
                        metric: closed("metric_id", &p.metric_id, MetricId::parse)?,
                        observed_at: text("observed_at", p.observed_at)?,
                        status: closed("status", &p.status, ObservationStatus::parse)?,
                        status_reason: optional_text("status_reason", p.status_reason)?,
                        measurement: p.measurement.map(MeasurementDto::into_fields).transpose()?,
                        observation_period: p
                            .observation_period
                            .map(|w| w.into_input("observation_period"))
                            .transpose()?,
                        coverage: p
                            .coverage
                            .map(|c| closed("coverage", &c, Coverage::parse))
                            .transpose()?,
                        coverage_note: optional_text("coverage_note", p.coverage_note)?,
                        evidence: p
                            .evidence
                            .into_iter()
                            .map(evidence)
                            .collect::<Converted<_>>()?,
                        supersedes: p.supersedes.map(|s| s.into_pin("supersedes")).transpose()?,
                        correction_reason: optional_text("correction_reason", p.correction_reason)?,
                    },
                )))
            }
            REPORT => {
                let p = ReportPayloadDto::deserialize(&self.payload)
                    .map_err(|e| format!("payload: {e}"))?;
                let period = p.period.into_input("period")?;
                Ok(FieldEvaluationInput::Report(Box::new(ReportInput {
                    envelope,
                    scope,
                    workspace: semantic_id("workspace_id", p.workspace_id)?,
                    period_start: period.start_date,
                    period_end: period.end_date,
                    included: p
                        .included_observations
                        .into_iter()
                        .map(|r| r.into_pin("included_observations"))
                        .collect::<Converted<_>>()?,
                    excluded: p
                        .excluded_observations
                        .into_iter()
                        .map(|x| {
                            Ok(ExclusionInput {
                                reference: x.reference.into_pin("excluded_observations.ref")?,
                                reason: text(
                                    "excluded_observations.exclusion_reason",
                                    x.exclusion_reason,
                                )?,
                            })
                        })
                        .collect::<Converted<_>>()?,
                    per_metric: p
                        .per_metric
                        .into_iter()
                        .map(|m| {
                            Ok(MetricReportInput {
                                metric: closed(
                                    "per_metric.metric_id",
                                    &m.metric_id,
                                    MetricId::parse,
                                )?,
                                status: closed(
                                    "per_metric.status",
                                    &m.status,
                                    MetricReportStatus::parse,
                                )?,
                                status_reason: optional_text(
                                    "per_metric.status_reason",
                                    m.status_reason,
                                )?,
                                aggregate: m.aggregate.into_stated()?,
                                samples: m
                                    .sample_observation_ids
                                    .into_iter()
                                    .map(|s| semantic_id("per_metric.sample_observation_ids", s))
                                    .collect::<Converted<_>>()?,
                                limitations: texts("per_metric.limitations", m.limitations)?,
                            })
                        })
                        .collect::<Converted<_>>()?,
                })))
            }
            other => Err(format!(
                "record_type: \"{other}\" is outside the closed pool"
            )),
        }
    }
}
