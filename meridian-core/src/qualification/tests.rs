#![cfg(test)]
//! Direct tests of the two qualification contracts over typed inputs.

use crate::canonical::CanonicalJson;
use crate::evidence::handoff::input::OutcomeStatus;
use crate::existing_project_compatibility_mode::NextStep;
use crate::migration::plan::tests::{head, plan, resolution_for, sid, text};
use crate::migration::resolved::{OpenObject, PlanResolution, ResponseCatalogue};
use crate::run_contracts::{LifecycleStage, PinSha256, ResponseField};
use crate::types::{OriginKind, Scope, Verdict};

use super::pin::{QualificationPin, ResolvedRecord};
use super::upgrade::{
    self, JourneyFacts, RecordSlot, RunFacts, ScenarioId, ScenarioInput, ScenarioSlots,
    SpecificationFacts, UpgradeComposition, UpgradePayloadInput, UpgradeQualificationInput,
};
use super::workspace::{
    self, compute_qualification_state, ConnectionFacts, ConnectionSlot, DecisionInputs,
    MigrationBoundaries, PlanSignal, RefSlot, TypedRecord, WorkspaceComposition,
    WorkspacePayloadInput, WorkspaceQualificationInput,
};
use super::{Composed, QualificationState};

fn present(s: &str) -> ResponseField<String> {
    ResponseField::Present(s.to_string())
}

fn record(kind: &str, id: &str, title: &str) -> ResolvedRecord {
    ResolvedRecord {
        record_type: present(kind),
        id: present(id),
        members: OpenObject::new(vec![
            ("id".to_string(), CanonicalJson::string(id)),
            ("record_type".to_string(), CanonicalJson::string(kind)),
            ("title".to_string(), CanonicalJson::string(title)),
        ]),
    }
}

fn pin_for(kind: &str, record: &ResolvedRecord, id: &str) -> QualificationPin {
    QualificationPin {
        declared_kind: text(kind),
        id: sid(id),
        reference: text(&format!("records/{id}")),
        sha256: PinSha256::new(record.content_digest().value()).unwrap(),
    }
}

// ----- content pins --------------------------------------------------------

#[test]
fn qualification_a_content_pin_covers_seven_members_in_any_order() {
    let a = record("execution-run", "run-1", "Прогон");
    let mut reordered = a.clone();
    reordered.members = OpenObject::new(vec![
        ("title".to_string(), CanonicalJson::string("Прогон")),
        (
            "record_type".to_string(),
            CanonicalJson::string("execution-run"),
        ),
        ("id".to_string(), CanonicalJson::string("run-1")),
        ("$schema".to_string(), CanonicalJson::string("not pinned")),
    ]);
    assert_eq!(a.content_digest(), reordered.content_digest());
    let changed = record("execution-run", "run-1", "Другое");
    let pin = pin_for("execution-run", &a, "run-1");
    assert!(pin.confirms(&a.content_digest()));
    assert!(!pin.confirms(&changed.content_digest()));
}

// ----- workspace decision matrix -----------------------------------------

fn inputs(
    steps: &[Option<NextStep>],
    undecided: bool,
    plan: PlanSignal,
    export: bool,
) -> DecisionInputs {
    DecisionInputs {
        next_steps: steps.to_vec(),
        has_undecided_candidates: undecided,
        plan,
        has_accepted_export: export,
    }
}

#[test]
fn qualification_workspace_decision_matrix_covers_all_nine_rows() {
    use NextStep::*;
    use QualificationState::*;
    let go = Some(ContinueCompatibilityMode);
    let verified = |mints| PlanSignal::Accepted {
        overall: Verdict::Verified,
        mints_records: mints,
    };
    let rows = [
        (
            inputs(&[Some(ResolveConflict)], false, PlanSignal::Absent, false),
            Blocked,
        ),
        (
            inputs(&[Some(ResolveAmbiguity)], false, PlanSignal::Absent, false),
            Blocked,
        ),
        (
            inputs(
                &[Some(AwaitOwnerDecision)],
                false,
                PlanSignal::Absent,
                false,
            ),
            Unverified,
        ),
        (inputs(&[go], true, PlanSignal::Absent, false), Unverified),
        (inputs(&[go], false, PlanSignal::Absent, false), Qualified),
        (
            inputs(
                &[go],
                false,
                PlanSignal::Accepted {
                    overall: Verdict::Blocked,
                    mints_records: true,
                },
                false,
            ),
            Blocked,
        ),
        (
            inputs(
                &[go],
                false,
                PlanSignal::Accepted {
                    overall: Verdict::Unverified,
                    mints_records: false,
                },
                false,
            ),
            Unverified,
        ),
        (inputs(&[go], false, verified(true), false), Unverified),
        (inputs(&[go], false, verified(true), true), Qualified),
    ];
    for (i, (row, expected)) in rows.iter().enumerate() {
        assert_eq!(compute_qualification_state(row), *expected, "row {}", i + 1);
    }
}

#[test]
fn qualification_workspace_matrix_is_order_independent_and_fails_closed() {
    use NextStep::*;
    let a = inputs(
        &[Some(ContinueCompatibilityMode), Some(ResolveConflict)],
        false,
        PlanSignal::Absent,
        false,
    );
    let b = inputs(
        &[Some(ResolveConflict), Some(ContinueCompatibilityMode)],
        false,
        PlanSignal::Absent,
        false,
    );
    assert_eq!(
        compute_qualification_state(&a),
        compute_qualification_state(&b)
    );
    let unknown = inputs(
        &[Some(ContinueCompatibilityMode), None],
        false,
        PlanSignal::Absent,
        false,
    );
    assert_eq!(
        compute_qualification_state(&unknown),
        QualificationState::Unverified
    );
    let rejected = inputs(
        &[Some(ContinueCompatibilityMode)],
        false,
        PlanSignal::Rejected,
        false,
    );
    assert_eq!(
        compute_qualification_state(&rejected),
        QualificationState::Unverified
    );
}

// ----- workspace composition --------------------------------------------

const QUALIFICATION: &str = "sample-qualification";

fn connection_record() -> ResolvedRecord {
    record("workspace-connection-scan", "sample-connection", "Скан")
}

fn workspace_entry(
    connection: &QualificationPin,
    state: QualificationState,
) -> WorkspaceQualificationInput {
    let mut h = head(
        QUALIFICATION,
        "workspace-compatibility-qualification",
        OriginKind::Derived,
        "workspace-connection-scan:sample-connection",
    );
    h.scope = Scope::project_workspace(sid("sample-project"), None);
    WorkspaceQualificationInput {
        head: h,
        payload: WorkspacePayloadInput {
            workspace_connection_refs: vec![connection.clone()],
            workspace_repository_ids: Some(vec![sid("sample-repository")]),
            migration_plan_ref: None,
            canonical_export_ref: None,
            qualification_state: state,
            qualification_reason: text("причина"),
            blockers: vec![],
            open_questions: vec![],
        },
    }
}

fn accepted_connection(scope: Scope) -> ConnectionFacts {
    ConnectionFacts {
        scope,
        repository_id: sid("sample-repository"),
        next_step: NextStep::ContinueCompatibilityMode,
        has_undecided_candidates: false,
    }
}

fn workspace_problems(
    entry: WorkspaceQualificationInput,
    composition: WorkspaceComposition,
    plans: &PlanResolution,
) -> Vec<String> {
    let content = ResponseCatalogue::new();
    workspace::check_workspace_qualifications(
        &[(entry, composition)],
        MigrationBoundaries {
            plan: plans,
            source_content: &content,
        },
    )
    .diagnostics
    .iter()
    .map(|d| d.message().to_string())
    .collect()
}

fn one_connection(accepted: Option<ConnectionFacts>) -> WorkspaceComposition {
    WorkspaceComposition {
        connections: vec![ConnectionSlot::Resolved {
            record: Box::new(connection_record()),
            accepted,
        }],
        connection_diagnostics: vec![],
        plan: RefSlot::NotDeclared,
        export: RefSlot::NotDeclared,
    }
}

#[test]
fn qualification_workspace_an_accepted_clean_connection_qualifies() {
    let pin = pin_for(
        "workspace-connection-scan",
        &connection_record(),
        "sample-connection",
    );
    let entry = workspace_entry(&pin, QualificationState::Qualified);
    let composition = one_connection(Some(accepted_connection(entry.head.scope.clone())));
    assert_eq!(
        workspace_problems(entry, composition, &PlanResolution::default()),
        Vec::<String>::new()
    );
}

#[test]
fn qualification_workspace_stale_pin_wrong_kind_and_scope_mismatch_are_distinct() {
    let record = connection_record();
    let mut stale = pin_for("workspace-connection-scan", &record, "sample-connection");
    stale.sha256 = PinSha256::new("0".repeat(64)).unwrap();
    let entry = workspace_entry(&stale, QualificationState::Unverified);
    let found = workspace_problems(entry, one_connection(None), &PlanResolution::default());
    assert!(found
        .iter()
        .any(|m| m
            .contains("does not equal the resolved connection's own recomputed content digest")));

    let wrong = record_with_kind("instance-migration-plan");
    let pin = pin_for("workspace-connection-scan", &wrong, "sample-connection");
    let entry = workspace_entry(&pin, QualificationState::Unverified);
    let composition = WorkspaceComposition {
        connections: vec![ConnectionSlot::Resolved {
            record: Box::new(wrong),
            accepted: None,
        }],
        ..one_connection(None)
    };
    let found = workspace_problems(entry, composition, &PlanResolution::default());
    assert!(found.iter().any(|m| m.contains(
        "resolved record_type is \"instance-migration-plan\", not \"workspace-connection-scan\""
    )));

    let pin = pin_for("workspace-connection-scan", &record, "sample-connection");
    let entry = workspace_entry(&pin, QualificationState::Qualified);
    let other = Scope::project_workspace(sid("another-project"), None);
    let found = workspace_problems(
        entry,
        one_connection(Some(accepted_connection(other))),
        &PlanResolution::default(),
    );
    assert!(found.iter().any(|m| m.contains(
        "scope does not match the resolved payload.workspace_connection_refs[0] record's scope"
    )));
}

fn record_with_kind(kind: &str) -> ResolvedRecord {
    record(kind, "sample-connection", "Скан")
}

/// A pinned plan its own check rejects does not hide the independent
/// problems of the declared export (criterion 9), and the matrix treats the
/// rejected plan as an explicit unknown.
#[test]
fn qualification_workspace_a_rejected_plan_does_not_hide_its_neighbours() {
    let connection = connection_record();
    let pin = pin_for(
        "workspace-connection-scan",
        &connection,
        "sample-connection",
    );
    let mut entry = workspace_entry(&pin, QualificationState::Qualified);
    let plan_input = plan();
    let plan_record = record(
        "instance-migration-plan",
        plan_input.head.id.as_str(),
        "План",
    );
    entry.payload.migration_plan_ref = Some(QualificationPin {
        declared_kind: text("instance-migration-plan"),
        id: plan_input.head.id.clone(),
        reference: text("records/plan"),
        sha256: PinSha256::new(plan_input.payload.plan_fingerprint.value()).unwrap(),
    });
    entry.payload.canonical_export_ref = Some(QualificationPin {
        declared_kind: text("instance-canonical-export"),
        id: sid("sample-export"),
        reference: text("records/export"),
        sha256: PinSha256::new("0".repeat(64)).unwrap(),
    });
    let composition = WorkspaceComposition {
        plan: RefSlot::Resolved(TypedRecord {
            record: plan_record,
            typed: Ok(plan_input),
        }),
        export: RefSlot::Unresolved,
        ..one_connection(Some(accepted_connection(entry.head.scope.clone())))
    };
    // No resolution at all: the plan's own snapshot/rollback/evidence
    // checks reject it.
    let found = workspace_problems(entry, composition, &PlanResolution::default());
    assert!(found
        .iter()
        .any(|m| m.contains("migration_plan_ref: migration plan")));
    assert!(found
        .iter()
        .any(|m| m.contains("payload.canonical_export_ref \"records/export\" does not resolve")));
    assert!(found.iter().any(|m| m.contains("computes \"UNVERIFIED\"")));
}

/// The plan the qualification pinned is the plan it composes: a clean
/// plan with its own resolution is accepted and its verdict is read.
#[test]
fn qualification_workspace_a_pinned_accepted_plan_feeds_the_matrix() {
    let connection = connection_record();
    let pin = pin_for(
        "workspace-connection-scan",
        &connection,
        "sample-connection",
    );
    let mut entry = workspace_entry(&pin, QualificationState::Unverified);
    entry.payload.open_questions = vec![text("экспорт ещё не составлен")];
    let plan_input = plan();
    let resolution = resolution_for(&plan_input);
    entry.payload.migration_plan_ref = Some(QualificationPin {
        declared_kind: text("instance-migration-plan"),
        id: plan_input.head.id.clone(),
        reference: text("records/plan"),
        sha256: PinSha256::new(plan_input.payload.plan_fingerprint.value()).unwrap(),
    });
    let composition = WorkspaceComposition {
        plan: RefSlot::Resolved(TypedRecord {
            record: record(
                "instance-migration-plan",
                plan_input.head.id.as_str(),
                "План",
            ),
            typed: Ok(plan_input),
        }),
        ..one_connection(Some(accepted_connection(entry.head.scope.clone())))
    };
    assert_eq!(
        workspace_problems(entry, composition, &resolution),
        Vec::<String>::new()
    );
}

// ----- upgrade decision matrix and composition ----------------------------

#[test]
fn qualification_upgrade_decision_matrix_rows() {
    use upgrade::DecisionInputs as I;
    use QualificationState::*;
    let base = I {
        journey_outcome: Some(OutcomeStatus::Complete),
        scenario_coverage_ok: true,
        all_scenarios_clean: true,
        field_report_accepted: Some(true),
    };
    let state = upgrade::compute_qualification_state;
    assert_eq!(state(base), Qualified);
    assert_eq!(
        state(I {
            journey_outcome: None,
            ..base
        }),
        Blocked
    );
    assert_eq!(
        state(I {
            journey_outcome: Some(OutcomeStatus::Blocked),
            ..base
        }),
        Blocked
    );
    assert_eq!(
        state(I {
            scenario_coverage_ok: false,
            ..base
        }),
        Blocked
    );
    assert_eq!(
        state(I {
            all_scenarios_clean: false,
            ..base
        }),
        Blocked
    );
    assert_eq!(
        state(I {
            journey_outcome: Some(OutcomeStatus::HandedOffIncomplete),
            ..base
        }),
        Unverified
    );
    assert_eq!(
        state(I {
            field_report_accepted: Some(false),
            ..base
        }),
        Blocked
    );
    assert_eq!(
        state(I {
            field_report_accepted: None,
            ..base
        }),
        Unverified
    );
}

fn upgrade_pin(kind: &str, record: &ResolvedRecord, id: &str) -> QualificationPin {
    pin_for(kind, record, id)
}

fn accepted<F>(record: ResolvedRecord, facts: F) -> RecordSlot<F> {
    RecordSlot::Resolved {
        record,
        composed: Some(Composed::accepted(facts)),
    }
}

fn upgrade_case(
    scenarios: &[ScenarioId],
    stage: LifecycleStage,
) -> (UpgradeQualificationInput, UpgradeComposition) {
    let journey = record("evidence-and-handoff", "journey", "Путь");
    let report = record("field-evaluation-report", "report", "Отчёт");
    let mut inputs = Vec::new();
    let mut slots = Vec::new();
    for (i, scenario) in scenarios.iter().enumerate() {
        let spec = record("task-specification", &format!("spec-{i}"), "Спецификация");
        let run = record("execution-run", &format!("run-{i}"), "Прогон");
        let spec_pin = upgrade_pin("task-specification", &spec, &format!("spec-{i}"));
        inputs.push(ScenarioInput {
            scenario_id: *scenario,
            task_specification_ref: spec_pin.clone(),
            execution_state_ref: upgrade_pin("execution-run", &run, &format!("run-{i}")),
        });
        let pattern = match scenario {
            ScenarioId::SingleModuleRefactor => "refactor-preserving-behavior",
            _ => "decompose-initiative",
        };
        let stage = match scenario {
            ScenarioId::LanguageChangeLimitCase => LifecycleStage::Classification,
            _ => stage,
        };
        slots.push(ScenarioSlots {
            specification: accepted(
                spec,
                SpecificationFacts {
                    task_pattern: sid(pattern),
                    workspace_id: Some("sample-project".to_string()),
                },
            ),
            run: accepted(
                run,
                RunFacts {
                    lifecycle_stage: stage,
                    task_specification_ref: spec_pin.reference.as_str().to_string(),
                    workspace_id: "sample-project".to_string(),
                },
            ),
        });
    }
    let entry = UpgradeQualificationInput {
        head: head(
            QUALIFICATION,
            "upgrade-integration-qualification",
            OriginKind::Derived,
            "derived:sample",
        ),
        payload: UpgradePayloadInput {
            task_journey_ref: upgrade_pin("evidence-and-handoff", &journey, "journey"),
            field_evaluation_report_ref: Some(upgrade_pin(
                "field-evaluation-report",
                &report,
                "report",
            )),
            scenario_classifications: inputs,
            transition_notes: text("шаблон"),
            qualification_state: QualificationState::Qualified,
            qualification_reason: text("причина"),
            blockers: vec![],
            open_questions: vec![],
        },
    };
    let composition = UpgradeComposition {
        journey: accepted(
            journey,
            JourneyFacts {
                outcome: OutcomeStatus::Complete,
                workspace_id: "sample-project".to_string(),
            },
        ),
        field_report: Some(accepted(
            report,
            upgrade::ReportFacts {
                workspace_id: "sample-project".to_string(),
            },
        )),
        scenarios: slots,
    };
    (entry, composition)
}

fn upgrade_problems(
    entry: UpgradeQualificationInput,
    composition: UpgradeComposition,
) -> Vec<String> {
    upgrade::check_upgrade_qualifications(&[(entry, composition)])
        .diagnostics
        .iter()
        .map(|d| d.message().to_string())
        .collect()
}

#[test]
fn qualification_upgrade_three_clean_scenarios_qualify() {
    let (entry, composition) = upgrade_case(&ScenarioId::ALL, LifecycleStage::Execution);
    assert_eq!(upgrade_problems(entry, composition), Vec::<String>::new());
}

/// Scenario coverage: a missing scenario, a duplicate one and a
/// decomposition that never proceeded are each rejected, and the matrix
/// recomputes BLOCKED.
#[test]
fn qualification_upgrade_scenario_coverage_and_expectations_are_checked() {
    let (entry, composition) = upgrade_case(
        &[
            ScenarioId::SingleModuleRefactor,
            ScenarioId::SingleModuleRefactor,
        ],
        LifecycleStage::Execution,
    );
    let found = upgrade_problems(entry, composition);
    assert!(found
        .iter()
        .any(|m| m.contains("carries 2 entries, not exactly 3")));
    assert!(found
        .iter()
        .any(|m| m.contains("declares scenario_id \"single-module-refactor\" 2 time(s)")));
    assert!(found
        .iter()
        .any(|m| m.contains("recomputes to \"BLOCKED\"")));

    let (entry, composition) = upgrade_case(&ScenarioId::ALL, LifecycleStage::Classification);
    let found = upgrade_problems(entry, composition);
    assert!(found.iter().any(|m| m.contains("\"multi-repository-decomposition\" resolves an execution-run at lifecycle_stage \"classification\", not past")));
}

#[test]
fn qualification_upgrade_a_stale_pin_or_wrong_kind_never_composes() {
    let (mut entry, mut composition) = upgrade_case(&ScenarioId::ALL, LifecycleStage::Execution);
    entry.payload.task_journey_ref.sha256 = PinSha256::new("0".repeat(64)).unwrap();
    composition.field_report = Some(RecordSlot::Resolved {
        record: record("field-evaluation-observation", "report", "Отчёт"),
        composed: None,
    });
    let found = upgrade_problems(entry, composition);
    assert!(found
        .iter()
        .any(|m| m.contains("payload.task_journey_ref \"records/journey\": sha256")));
    assert!(found.iter().any(|m| m.contains(
        "resolved record_type is \"field-evaluation-observation\", not \"field-evaluation-report\""
    )));
    assert!(found
        .iter()
        .any(|m| m.contains("recomputes to \"BLOCKED\"")));
}
