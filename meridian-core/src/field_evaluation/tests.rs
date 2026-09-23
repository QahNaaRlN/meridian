//! Direct core tests of `meridian-field-evaluation`: every one of the
//! eight metrics, the observation rules, and a report whose aggregates are
//! recomputed from typed, resolved observations.
#![cfg(test)]

use super::*;
use crate::evidence::{EvidenceKind, PinnedEvidence};
use crate::run_contracts::envelope::{RecordAuthority, RecordOrigin, SourcedOriginKind};
use crate::run_contracts::{
    EvidenceResultResponse, ForeignValue, JsonNumber, MeasurementFields, ObservationResponse,
    PinnedRecordKind, PinnedRef, PortableRef, RecordText, ResolutionCatalogue, ResolvedEntry,
    ResponseField, ResponseValueKind, RunId, WindowFields,
};
use crate::types::{AuthorityKind, OriginKind, WorkspaceId};

const EXACT: &str = "v1.0.0";
const RUN_REF: &str = "records/execution-run/run-1";

fn sid(s: &str) -> SemanticId {
    SemanticId::new(s).unwrap()
}
fn t(s: &str) -> RecordText {
    RecordText::new(s).unwrap()
}
fn p(s: &str) -> PortableRef {
    PortableRef::new(s).unwrap()
}
fn text(s: &str) -> ResponseField<String> {
    ResponseField::Present(s.to_string())
}
fn num(v: f64) -> ResponseField<JsonNumber> {
    ResponseField::Present(JsonNumber::new(v, v.to_string()).unwrap())
}

fn envelope(id: &str) -> RecordEnvelope {
    RecordEnvelope {
        declared_schema: t("../../registries/operating-model/field-evaluation.schema.json"),
        id: sid(id),
        title: t("Полевая оценка"),
        origin: RecordOrigin::Sourced {
            kind: SourcedOriginKind::new(OriginKind::Declared).unwrap(),
            source_ref: p("owner-decision:evaluation"),
        },
        authority: RecordAuthority {
            kind: AuthorityKind::ProjectOwner,
            authority_ref: p("example-owner"),
            decision_ref: None,
        },
    }
}

fn pin(kind: PinnedRecordKind, id: &str, reference: &str) -> PinnedRef {
    PinnedRef {
        record_type: kind,
        id: sid(id),
        run_id: None,
        reference: p(reference),
        revision: Some(t(EXACT)),
        sha256: None,
    }
}

/// A valid measurement of `metric`.
fn measurement(metric: MetricId) -> MeasurementFields {
    match metric.shape() {
        MetricShape::Classification(cm) => MeasurementFields {
            kind: text("classification"),
            outcome: text(cm.pool()[0].as_str()),
            basis: text("основание классификации"),
            ..MeasurementFields::default()
        },
        MetricShape::Duration(_) => MeasurementFields {
            kind: text("duration"),
            duration_kind: text("calendar"),
            unit: text("seconds"),
            seconds: num(600.0),
            ..MeasurementFields::default()
        },
        MetricShape::Count(_) => MeasurementFields {
            kind: text("count"),
            unit: text("count"),
            value: num(2.0),
            ..MeasurementFields::default()
        },
    }
}

fn observation(metric: MetricId) -> ObservationInput {
    let defects = metric.requires_window();
    ObservationInput {
        envelope: envelope("obs-1"),
        scope: FieldScope::RunState(RunStateScope::new(
            RunId::new("run-1").unwrap(),
            WorkspaceId::new("workspace-1").unwrap(),
        )),
        run: pin(PinnedRecordKind::ExecutionRun, "run-1", RUN_REF),
        metric,
        observed_at: t("2026-08-10"),
        status: ObservationStatus::Observed,
        status_reason: None,
        measurement: Some(measurement(metric)),
        observation_period: defects.then(|| WindowInput {
            start_date: t("2026-08-01"),
            end_date: t("2026-08-31"),
        }),
        coverage: defects.then_some(Coverage::Complete),
        coverage_note: None,
        evidence: vec![PinnedEvidence {
            id: sid("ev-1"),
            kind: EvidenceKind::Observation,
            reference: p("evidence/obs-1"),
            revision: Some(t(EXACT)),
            sha256: None,
            summary: t("Наблюдение за запуском"),
        }],
        supersedes: None,
        correction_reason: None,
    }
}

fn base_entry(kind: &str, id: &str, reference: &str) -> ResolvedEntry {
    ResolvedEntry {
        record_type: text(kind),
        id: text(id),
        reference: text(reference),
        revision: text(EXACT),
        ..ResolvedEntry::default()
    }
}

fn observation_catalogue(metric: MetricId, observed: &str) -> ResolutionCatalogue {
    let mut c = ResolutionCatalogue::new();
    c.insert(RUN_REF, base_entry("execution-run", "run-1", RUN_REF));
    let mut ev = base_entry("evidence-result", "ev-1", "evidence/obs-1");
    ev.evidence_result = EvidenceResultResponse {
        observed_result: text(observed),
        metric_ref: text(metric.as_str()),
        ..EvidenceResultResponse::default()
    };
    c.insert("evidence/obs-1", ev);
    c
}

fn run_observation(input: ObservationInput, c: &ResolutionCatalogue) -> Vec<String> {
    let (accepted, problems) =
        check_field_evaluation(FieldEvaluationInput::Observation(Box::new(input)), Some(c));
    assert_eq!(accepted.is_some(), problems.is_empty());
    problems.iter().map(|d| d.message().to_string()).collect()
}

fn observation_problems(
    metric: MetricId,
    mutate: impl FnOnce(&mut ObservationInput),
) -> Vec<String> {
    let mut i = observation(metric);
    mutate(&mut i);
    run_observation(i, &observation_catalogue(metric, "confirmed"))
}

fn has(found: &[String], expected: &str) {
    assert!(
        found.iter().any(|m| m == expected),
        "expected {expected:?} among {found:#?}"
    );
}

const O: &str = "field evaluation observation \"obs-1\"";
const R: &str = "field evaluation report \"report-1\"";

#[test]
fn field_evaluation_each_of_the_eight_metrics_accepts_its_own_measurement_kind() {
    for metric in MetricId::ALL {
        let (accepted, problems) = check_field_evaluation(
            FieldEvaluationInput::Observation(Box::new(observation(metric))),
            Some(&observation_catalogue(metric, "confirmed")),
        );
        assert!(problems.is_empty(), "{metric}: {problems:#?}");
        let Some(FieldEvaluationRecord::Observation(o)) = accepted else {
            panic!("{metric}: an observation is accepted");
        };
        assert_eq!(o.metric(), metric);
        let ObservationState::Observed {
            measurement,
            window,
            ..
        } = o.state()
        else {
            panic!("{metric}: observed");
        };
        let shape_matches = matches!(
            (metric.shape(), measurement),
            (MetricShape::Classification(cm), Measurement::Classification { outcome, .. }) if outcome.metric() == cm
        ) || matches!(
            (metric.shape(), measurement),
            (MetricShape::Duration(dm), Measurement::Duration(d)) if d.metric() == dm
        ) || matches!(
            (metric.shape(), measurement),
            (MetricShape::Count(cm), Measurement::Count { metric: m, .. }) if *m == cm
        );
        assert!(shape_matches, "{metric}: {measurement:?}");
        assert_eq!(window.is_some(), metric.requires_window());
    }
}

#[test]
fn field_evaluation_each_metric_rejects_another_metrics_measurement_kind() {
    for metric in MetricId::ALL {
        let other = MetricId::ALL
            .into_iter()
            .find(|m| m.kind() != metric.kind())
            .unwrap();
        let found = observation_problems(metric, |i| i.measurement = Some(measurement(other)));
        assert_eq!(
            found[0],
            format!(
                "{O} measurement.kind is \"{}\", but metric \"{metric}\" is measured by kind \"{}\"",
                other.kind().as_str(),
                metric.kind().as_str()
            )
        );
    }
}

#[test]
fn field_evaluation_absence_is_explained_and_never_measured() {
    let metric = MetricId::OwnerCost;
    let absent = |i: &mut ObservationInput| {
        i.status = ObservationStatus::Unmeasurable;
        i.measurement = None;
        i.evidence = vec![];
    };
    has(
        &observation_problems(metric, absent),
        &format!("{O} is \"unmeasurable\" but states no status_reason; unknown, not_applicable and unmeasurable are distinct and each needs its own explanation, not a shared silence"),
    );
    let found = observation_problems(metric, |i| {
        absent(i);
        i.status_reason = Some(t("Нет таймера"));
    });
    assert!(found.is_empty(), "{found:#?}");
    has(
        &observation_problems(metric, |i| {
            i.status = ObservationStatus::Unknown;
            i.status_reason = Some(t("Нет данных"));
        }),
        &format!("{O} is \"unknown\" but carries a measurement; a value that was not observed is not measured"),
    );
}

#[test]
fn field_evaluation_an_observation_is_bound_to_its_run_scope_and_date() {
    let metric = MetricId::MechanismCorrectness;
    has(
        &observation_problems(metric, |i| i.scope = FieldScope::ProjectWorkspace(sid("workspace-1"))),
        &format!("{O} is scoped to \"project-workspace\"; a field evaluation observation lives in run-state only"),
    );
    has(
        &observation_problems(metric, |i| i.run.id = sid("run-2")),
        &format!("{O} scope identifies run \"run-1\" but execution_run_ref.id is \"run-2\"; the two must be the exact same string, not one a substring of the other"),
    );
    has(
        &observation_problems(metric, |i| i.observed_at = t("2026-02-30")),
        &format!("{O} observed_at is not a valid date (YYYY-MM-DD); the record states when it was itself recorded"),
    );
}

#[test]
fn field_evaluation_only_post_acceptance_defects_names_a_window() {
    let defects = MetricId::PostAcceptanceDefects;
    has(
        &observation_problems(defects, |i| i.observation_period = None),
        &format!("{O} measures \"post-acceptance-defects\", which requires an observation_period (the window actually covered) and a coverage completeness flag"),
    );
    has(
        &observation_problems(defects, |i| i.coverage = Some(Coverage::Partial)),
        &format!("{O} coverage is \"partial\" but states no coverage_note; an incomplete window needs to say what is not covered"),
    );
    has(
        &observation_problems(defects, |i| {
            i.observation_period = Some(WindowInput {
                start_date: t("2026-08-31"),
                end_date: t("2026-08-01"),
            })
        }),
        &format!("{O} observation_period end_date is before start_date; an impossible window"),
    );
    has(
        &observation_problems(MetricId::ReworkReturns, |i| i.coverage = Some(Coverage::Complete)),
        &format!("{O} carries observation_period/coverage, but metric \"rework-returns\" at status \"observed\" does not use an observation window; only \"post-acceptance-defects\" while \"observed\" does"),
    );
}

#[test]
fn field_evaluation_a_measured_value_is_grounded_by_confirmed_evidence_of_its_metric() {
    let metric = MetricId::StopCorrectness;
    has(
        &observation_problems(metric, |i| i.evidence = vec![]),
        &format!("{O} is \"observed\" but carries no evidence; a measured value is grounded by at least one resolvable piece of evidence, not asserted on its own say-so"),
    );
    let contradicted = run_observation(
        observation(metric),
        &observation_catalogue(metric, "contradicted"),
    );
    has(
        &contradicted,
        &format!("{O} is \"observed\" but no evidence entry resolves cleanly with observed_result \"confirmed\" for this metric; an unresolved or contradicted/inconclusive entry does not ground a measured value"),
    );
    let foreign_metric = run_observation(
        observation(metric),
        &observation_catalogue(MetricId::MissedNorms, "confirmed"),
    );
    has(
        &foreign_metric,
        &format!("{O} evidence entry ev-1: the resolved evidence-result's metric_ref \"missed-norms\" is not this observation's own metric \"stop-correctness\"; evidence of a different indicator does not support this one"),
    );
    let (accepted, problems) = check_field_evaluation(
        FieldEvaluationInput::Observation(Box::new(observation(metric))),
        None,
    );
    assert!(accepted.is_none());
    has(
        &problems.iter().map(|d| d.message().to_string()).collect::<Vec<_>>(),
        &format!("{O} cannot be verified: no external record resolver was supplied; evidence is resolved OUTSIDE the record and checked against it — without that boundary the record is only an unanchored self-report"),
    );
}

#[test]
fn field_evaluation_a_correction_names_both_the_replaced_observation_and_why() {
    let metric = MetricId::ResumptionSuccess;
    has(
        &observation_problems(metric, |i| i.correction_reason = Some(t("Исправлено"))),
        &format!("{O} states only one of supersedes / correction_reason; a correction names BOTH the observation it corrects and why"),
    );
    has(
        &observation_problems(metric, |i| {
            i.supersedes = Some(pin(
                PinnedRecordKind::FieldEvaluationObservation,
                "obs-1",
                "obs/obs-1",
            ));
            i.correction_reason = Some(t("Исправлено"));
        }),
        &format!("{O} supersedes itself; a correction names a DIFFERENT, earlier observation"),
    );
}

// --- report ---------------------------------------------------------------

struct Obs {
    id: &'static str,
    metric: MetricId,
    measurement: MeasurementFields,
    window: bool,
}

fn resolved_observation(o: &Obs) -> ResolvedEntry {
    let reference = format!("obs/{}", o.id);
    let mut e = base_entry("field-evaluation-observation", o.id, &reference);
    e.observation = ObservationResponse {
        metric_id: text(o.metric.as_str()),
        status: text("observed"),
        measurement: ResponseField::Present(Some(o.measurement.clone())),
        workspace_id: text("workspace-1"),
        observed_at: text("2026-08-10"),
        supersedes_id: ResponseField::Absent,
        observation_period: if o.window {
            ResponseField::Present(Some(WindowFields {
                start_date: text("2026-08-01"),
                end_date: text("2026-08-31"),
            }))
        } else {
            ResponseField::Absent
        },
        coverage: if o.window {
            ResponseField::Present(Some("partial".to_string()))
        } else {
            ResponseField::Absent
        },
    };
    e
}

fn classification(outcome: &str) -> MeasurementFields {
    MeasurementFields {
        outcome: text(outcome),
        ..measurement(MetricId::MechanismCorrectness)
    }
}

fn observations() -> Vec<Obs> {
    vec![
        Obs {
            id: "obs-b",
            metric: MetricId::MechanismCorrectness,
            measurement: classification("incorrect"),
            window: false,
        },
        Obs {
            id: "obs-a",
            metric: MetricId::MechanismCorrectness,
            measurement: classification("correct"),
            window: false,
        },
        Obs {
            id: "obs-c",
            metric: MetricId::ContextEntryTime,
            measurement: measurement(MetricId::ContextEntryTime),
            window: false,
        },
        Obs {
            id: "obs-d",
            metric: MetricId::PostAcceptanceDefects,
            measurement: MeasurementFields {
                value: num(0.0),
                ..measurement(MetricId::PostAcceptanceDefects)
            },
            window: true,
        },
    ]
}

fn report_catalogue(obs: &[Obs]) -> ResolutionCatalogue {
    let mut c = ResolutionCatalogue::new();
    for o in obs {
        c.insert(format!("obs/{}", o.id), resolved_observation(o));
    }
    c
}

fn empty_aggregate(metric: MetricId) -> StatedAggregate {
    let empty = StatedDurationSummary {
        sample_count: 0,
        total_seconds: 0.0,
        mean_seconds: None,
    };
    match metric.shape() {
        MetricShape::Classification(cm) => StatedAggregate::Classification {
            counts: cm.pool().map(|o| (o.as_str().to_string(), Some(0.0))).to_vec(),
            total: 0,
            rate: StatedRate {
                numerator: 0,
                denominator: 0,
                percentage: None,
                outcome: cm.tracked().as_str().to_string(),
                note: format!("доля наблюдений с исходом \"{}\" среди 0 классифицированных; ноль в знаменателе не даёт процента", cm.tracked()),
            },
        },
        MetricShape::Duration(_) => StatedAggregate::Duration {
            calendar: empty.clone(),
            active: empty,
        },
        MetricShape::Count(_) => StatedAggregate::Count {
            sample_count: 0,
            total: 0,
            mean: None,
            window: None,
            coverage: None,
        },
    }
}

fn metric_report(metric: MetricId) -> MetricReportInput {
    match metric {
        MetricId::MechanismCorrectness => MetricReportInput {
            metric,
            status: MetricReportStatus::Known,
            status_reason: None,
            aggregate: StatedAggregate::Classification {
                counts: vec![
                    ("correct".to_string(), Some(1.0)),
                    ("incorrect".to_string(), Some(1.0)),
                    ("unknown".to_string(), Some(0.0)),
                ],
                total: 2,
                rate: StatedRate {
                    numerator: 1,
                    denominator: 2,
                    percentage: Some(50.0),
                    outcome: "correct".to_string(),
                    note: "доля наблюдений с исходом \"correct\" среди 2 классифицированных; ноль в знаменателе не даёт процента".to_string(),
                },
            },
            samples: vec![sid("obs-b"), sid("obs-a")],
            limitations: vec![],
        },
        MetricId::ContextEntryTime => MetricReportInput {
            metric,
            status: MetricReportStatus::Known,
            status_reason: None,
            aggregate: StatedAggregate::Duration {
                calendar: StatedDurationSummary {
                    sample_count: 1,
                    total_seconds: 600.0,
                    mean_seconds: Some(600.0),
                },
                active: StatedDurationSummary {
                    sample_count: 0,
                    total_seconds: 0.0,
                    mean_seconds: None,
                },
            },
            samples: vec![sid("obs-c")],
            limitations: vec![],
        },
        MetricId::PostAcceptanceDefects => MetricReportInput {
            metric,
            status: MetricReportStatus::Known,
            status_reason: None,
            aggregate: StatedAggregate::Count {
                sample_count: 1,
                total: 0,
                mean: Some(0.0),
                window: Some(("2026-08-01".to_string(), "2026-08-31".to_string())),
                coverage: Some("partial".to_string()),
            },
            samples: vec![sid("obs-d")],
            limitations: vec![t("Окно наблюдения покрыто частично")],
        },
        other => MetricReportInput {
            metric: other,
            status: MetricReportStatus::Unknown,
            status_reason: Some(t("Нет наблюдений")),
            aggregate: empty_aggregate(other),
            samples: vec![],
            limitations: vec![],
        },
    }
}

fn report(obs: &[Obs]) -> ReportInput {
    ReportInput {
        envelope: envelope("report-1"),
        scope: FieldScope::ProjectWorkspace(sid("workspace-1")),
        workspace: sid("workspace-1"),
        period_start: t("2026-08-01"),
        period_end: t("2026-08-31"),
        included: obs
            .iter()
            .map(|o| {
                pin(
                    PinnedRecordKind::FieldEvaluationObservation,
                    o.id,
                    &format!("obs/{}", o.id),
                )
            })
            .collect(),
        excluded: vec![],
        per_metric: MetricId::ALL.map(metric_report).to_vec(),
    }
}

fn run_report(input: ReportInput, c: &ResolutionCatalogue) -> Vec<String> {
    let (accepted, problems) =
        check_field_evaluation(FieldEvaluationInput::Report(Box::new(input)), Some(c));
    assert_eq!(accepted.is_some(), problems.is_empty());
    problems.iter().map(|d| d.message().to_string()).collect()
}

fn report_problems(mutate: impl FnOnce(&mut ReportInput)) -> Vec<String> {
    let obs = observations();
    let mut r = report(&obs);
    mutate(&mut r);
    run_report(r, &report_catalogue(&obs))
}

#[test]
fn field_evaluation_a_report_is_accepted_with_eight_recomputed_aggregates() {
    let obs = observations();
    let (accepted, problems) = check_field_evaluation(
        FieldEvaluationInput::Report(Box::new(report(&obs))),
        Some(&report_catalogue(&obs)),
    );
    assert!(problems.is_empty(), "{problems:#?}");
    let Some(FieldEvaluationRecord::Report(r)) = accepted else {
        panic!("a report is accepted");
    };
    assert_eq!(r.per_metric().len(), 8);
    let mechanism = &r.per_metric()[0];
    let MetricAggregate::Classification { rate, total, .. } = &mechanism.aggregate else {
        panic!("classification");
    };
    assert_eq!(*total, 2);
    assert_eq!(rate.percentage, Percentage::of(1, 2));
    let rework = r
        .per_metric()
        .iter()
        .find(|m| m.metric == MetricId::ReworkReturns)
        .unwrap();
    assert!(matches!(
        rework.aggregate,
        MetricAggregate::Count { mean: None, .. }
    ));
}

#[test]
fn field_evaluation_the_aggregate_is_independent_of_input_order() {
    let mut obs = observations();
    obs.reverse();
    let found = run_report(report(&obs), &report_catalogue(&obs));
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn field_evaluation_a_stated_aggregate_or_status_that_diverges_is_rejected() {
    has(
        &report_problems(|r| {
            r.per_metric[1].aggregate = StatedAggregate::Duration {
                calendar: StatedDurationSummary {
                    sample_count: 1,
                    total_seconds: 601.0,
                    mean_seconds: Some(601.0),
                },
                active: StatedDurationSummary {
                    sample_count: 0,
                    total_seconds: 0.0,
                    mean_seconds: None,
                },
            }
        }),
        &format!("{R} per_metric \"context-entry-time\" aggregate does not match the value recomputed from its resolved sample; a stated aggregate that diverges from the actual observations is rejected, whatever the status"),
    );
    has(
        &report_problems(|r| {
            r.per_metric[1].status = MetricReportStatus::Unknown;
            r.per_metric[1].status_reason = Some(t("Не знаем"));
        }),
        &format!("{R} per_metric \"context-entry-time\" states status \"unknown\", but the resolved, named sample implies \"known\"; the report's conclusion must agree with what its own observations show"),
    );
}

#[test]
fn field_evaluation_eight_of_eight_and_no_double_counting() {
    has(
        &report_problems(|r| {
            r.per_metric.pop();
        }),
        &format!("{R} per_metric omits metric_id \"post-acceptance-defects\"; all eight characteristics are represented, even when the result is \"unknown\""),
    );
    has(
        &report_problems(|r| {
            let first = r.per_metric[0].clone();
            r.per_metric.push(first);
        }),
        &format!("{R} per_metric repeats metric_id \"mechanism-correctness\""),
    );
    has(
        &report_problems(|r| {
            let again = r.included[0].clone();
            r.included.push(again);
        }),
        &format!("{R} included_observations repeats reference \"obs/obs-b\" at #4; the same observation is not counted twice"),
    );
    has(
        &report_problems(|r| r.per_metric[0].samples.pop().map(|_| ()).unwrap_or(())),
        &format!("{R} per_metric \"mechanism-correctness\" sample_observation_ids omits included observation \"obs-a\"; an included observation is either counted in its metric's sample or formally excluded with a reason, never silently left out of the aggregate"),
    );
}

#[test]
fn field_evaluation_samples_are_comparable_workspace_period_and_supersedes() {
    let obs = observations();
    let mut c = report_catalogue(&obs);
    let mut e = c.resolve("obs/obs-c").cloned().unwrap();
    e.observation.workspace_id = text("workspace-2");
    e.observation.observed_at = text("2026-09-01");
    c.insert("obs/obs-c", e);
    let found = run_report(report(&obs), &c);
    has(&found, &format!("{R} included_observations[2] belongs to workspace \"workspace-2\", not this report's workspace \"workspace-1\"; samples from different workspaces are not mixed without an explicit comparability rule"));
    has(&found, &format!("{R} included_observations[2] was observed at \"2026-09-01\", outside the report period [2026-08-01, 2026-08-31]; samples from different periods are not mixed without an explicit comparability rule"));

    let mut c = report_catalogue(&obs);
    let mut e = c.resolve("obs/obs-a").cloned().unwrap();
    e.observation.supersedes_id = ResponseField::Present(Some("obs-b".to_string()));
    c.insert("obs/obs-a", e);
    has(
        &run_report(report(&obs), &c),
        &format!("{R} includes both observation \"obs-a\" and the observation \"obs-b\" it supersedes; the superseded record's facts are replaced, not additionally counted"),
    );
}

#[test]
fn field_evaluation_exclusions_and_incomplete_zero_counts_are_disclosed() {
    let obs = observations();
    has(
        &report_problems(|r| r.per_metric[7].limitations.clear()),
        &format!("{R} per_metric \"post-acceptance-defects\" shows zero defects over a \"partial\" observation window but carries no limitation; a zero count over incomplete coverage is not proof of no defects and must say so"),
    );
    let mut r = report(&obs);
    let excluded = r.included.remove(2);
    r.excluded.push(ExclusionInput {
        reference: excluded,
        reason: t("Запуск прерван"),
    });
    r.per_metric[1] = MetricReportInput {
        metric: MetricId::ContextEntryTime,
        status: MetricReportStatus::Unknown,
        status_reason: Some(t("Наблюдение исключено")),
        aggregate: empty_aggregate(MetricId::ContextEntryTime),
        samples: vec![],
        limitations: vec![],
    };
    has(
        &run_report(r, &report_catalogue(&obs)),
        &format!("{R} per_metric \"context-entry-time\" has an excluded observation of this metric but discloses no limitation; an exclusion that would have contributed to a metric is disclosed, not hidden"),
    );
}

fn foreign_string(text: &str) -> ForeignValue {
    ForeignValue::new(ResponseValueKind::String, format!("\"{text}\""), text)
}

/// The report — through the real `check_field_evaluation` — with one
/// included observation's response changed.
fn report_with(reference: &str, change: impl FnOnce(&mut ObservationResponse)) -> Vec<String> {
    let obs = observations();
    let mut c = report_catalogue(&obs);
    let mut e = c.resolve(reference).cloned().unwrap();
    change(&mut e.observation);
    c.insert(reference, e);
    run_report(report(&obs), &c)
}

/// An `observed` resolved observation whose measurement, or whose required
/// window, the closed contract refuses is reported by that issue ALONE: it
/// never contributes to its metric, so no secondary status or aggregate
/// diagnostic is derived from the refused value (`COMPATIBILITY.md`,
/// architect-accepted Rust-native boundary). Before this boundary an
/// absent/`null`/non-object measurement counted as "other" and an absent
/// window as "no window", and each case also produced a status and/or
/// aggregate mismatch.
#[test]
fn field_evaluation_a_refused_resolved_measurement_or_window_is_never_aggregated() {
    let at = |i: usize, issue: &str| {
        vec![format!(
            "{R} included_observations[{i}]: resolved observation {issue}"
        )]
    };
    let not_an_object = "status is \"observed\" but measurement is not an object";
    // obs-c: context-entry-time, observed.
    assert_eq!(
        report_with("obs/obs-c", |o| o.measurement = ResponseField::Absent),
        at(2, not_an_object),
        "absent measurement"
    );
    assert_eq!(
        report_with("obs/obs-c", |o| o.measurement =
            ResponseField::Present(None)),
        at(2, not_an_object),
        "null measurement"
    );
    assert_eq!(
        report_with("obs/obs-c", |o| {
            o.measurement = ResponseField::Foreign(foreign_string("600 seconds"))
        }),
        at(2, not_an_object),
        "non-object measurement"
    );
    assert_eq!(
        report_with("obs/obs-c", |o| {
            if let ResponseField::Present(Some(m)) = &mut o.measurement {
                m.seconds = ResponseField::Foreign(foreign_string("600"));
            }
        }),
        at(2, "measurement measurement.seconds \"600\" is not a non-negative number; a negative duration is not measurable"),
        "invalid measurement.seconds"
    );
    // obs-d: post-acceptance-defects, observed, requires a window.
    let requires =
        "metric \"post-acceptance-defects\" requires an observation_period when observed";
    assert_eq!(
        report_with("obs/obs-d", |o| o.observation_period =
            ResponseField::Absent),
        at(3, requires),
        "absent required window"
    );
    assert_eq!(
        report_with("obs/obs-d", |o| o.observation_period =
            ResponseField::Present(None)),
        at(3, requires),
        "null required window"
    );
    assert_eq!(
        report_with("obs/obs-d", |o| {
            o.observation_period = ResponseField::Foreign(foreign_string("august"))
        }),
        at(3, "observation_period does not carry two valid dates"),
        "non-object required window"
    );
}

#[test]
fn field_evaluation_a_report_without_a_resolver_fails_closed() {
    let obs = observations();
    let (accepted, problems) =
        check_field_evaluation(FieldEvaluationInput::Report(Box::new(report(&obs))), None);
    assert!(accepted.is_none());
    has(
        &problems.iter().map(|d| d.message().to_string()).collect::<Vec<_>>(),
        &format!("{R} cannot be verified: no external record resolver was supplied; every included and excluded observation is resolved OUTSIDE the report and checked against it"),
    );
}
