//! Direct core tests of the twenty sections of the evidence-and-handoff
//! contract, over one typed reference handoff and its resolution
//! catalogue. Each test breaks exactly one rule and asserts the exact
//! prefix-free diagnostic, and that no accepted value is produced.
#![cfg(test)]

use super::input::*;
use super::*;
use crate::evidence::{AssertionStatus, ClaimedResultStatus, EvidenceKind, PinnedEvidence};
use crate::run_contracts::envelope::{RecordAuthority, RecordOrigin, SourcedOriginKind};
use crate::run_contracts::{
    EvidenceResultResponse, PinSha256, PinnedRecordKind, PinnedRef, PortableRef, RecordText,
    ResolutionCatalogue, ResolvedEntry, ResolvedStateResponse, ResponseField, ResponseItem, RunId,
    SpecificationResponse,
};
use crate::types::{AuthorityKind, OriginKind, SemanticId, WorkspaceId};

const RUN_REF: &str = "records/execution-run/run-1";
const SPEC_REF: &str = "records/task-specification/spec-1";
const HC_REF: &str = "records/run-human-control/hc-1";
const CM_REF: &str = "records/context-manifest/cm-1";
const EVIDENCE_REF: &str = "evidence/run-1/unit-tests";
const CHECK_REF: &str = "checks/unit-tests";
const EXACT: &str = "v1.0.0";
const H: &str = "evidence and handoff \"handoff-1\"";

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
fn items(values: &[&str]) -> ResponseField<Vec<ResponseItem>> {
    ResponseField::Present(
        values
            .iter()
            .map(|v| ResponseItem::Text(v.to_string()))
            .collect(),
    )
}

fn pin(kind: PinnedRecordKind, id: &str, run_id: Option<&str>, reference: &str) -> PinnedRef {
    PinnedRef {
        record_type: kind,
        id: sid(id),
        run_id: run_id.map(sid),
        reference: p(reference),
        revision: Some(t(EXACT)),
        sha256: None,
    }
}

fn input() -> HandoffInput {
    HandoffInput {
        envelope: RecordEnvelope {
            declared_schema: t("../../registries/operating-model/evidence-and-handoff.schema.json"),
            id: sid("handoff-1"),
            title: t("Передача запуска"),
            origin: RecordOrigin::Sourced {
                kind: SourcedOriginKind::new(OriginKind::Declared).unwrap(),
                source_ref: p("owner-decision:handoff"),
            },
            authority: RecordAuthority {
                kind: AuthorityKind::DelegatedRun,
                authority_ref: p("example-owner"),
                decision_ref: None,
            },
        },
        scope: RunStateScope::new(
            RunId::new("run-1").unwrap(),
            WorkspaceId::new("workspace-1").unwrap(),
        ),
        body: HandoffBody {
            pins: HandoffPins {
                execution_run: pin(PinnedRecordKind::ExecutionRun, "run-1", None, RUN_REF),
                task_specification: pin(
                    PinnedRecordKind::TaskSpecification,
                    "spec-1",
                    None,
                    SPEC_REF,
                ),
                human_control: pin(
                    PinnedRecordKind::RunHumanControl,
                    "hc-1",
                    Some("run-1"),
                    HC_REF,
                ),
                context_manifest: pin(
                    PinnedRecordKind::ContextManifest,
                    "cm-1",
                    Some("run-1"),
                    CM_REF,
                ),
            },
            outcome: OutcomeInput {
                statement: t("Передано владельцу на приёмку"),
                status: OutcomeStatus::HandedOffIncomplete,
            },
            claimed_results: vec![ClaimedResultInput {
                id: sid("result-1"),
                statement: t("Разбор null исправлен"),
                status: ClaimedResultStatus::Established,
            }],
            assertions: vec![AssertionInput {
                id: sid("assertion-1"),
                statement: t("Модульные тесты проходят"),
                claimed_result_id: sid("result-1"),
                status: AssertionStatus::Verified,
                unverified_reason: None,
            }],
            evidence: vec![HandoffEvidenceInput {
                evidence: PinnedEvidence {
                    id: sid("evidence-1"),
                    kind: EvidenceKind::CheckRun,
                    reference: p(EVIDENCE_REF),
                    revision: Some(t(EXACT)),
                    sha256: None,
                    summary: t("Прогон модульных тестов"),
                },
                covers: vec![sid("assertion-1")],
                limitations: vec![],
                specialised_contract: None,
                recorded_verdict: None,
            }],
            mandatory_checks: vec![MandatoryCheckInput {
                id: sid("check-1"),
                name: t("Модульные тесты"),
                check_ref: p(CHECK_REF),
                status: CheckStatus::Passed,
                result_evidence_id: Some(sid("evidence-1")),
                reason: None,
            }],
            acceptance_criteria: vec![CriterionInput {
                id: sid("ac-1"),
                status: CoverageStatus::Covered,
                addressed_by: vec![sid("assertion-1")],
                uncovered_reason: None,
            }],
            source_state: vec![RepositoryStateInput {
                repository_ref: p("repo:example"),
                revision: Some(t(&"a".repeat(40))),
                sha256: None,
            }],
            result_state: vec![RepositoryStateInput {
                repository_ref: p("repo:example"),
                revision: Some(t(&"b".repeat(40))),
                sha256: None,
            }],
            changed_paths: vec![ChangedPathInput {
                id: sid("path-1"),
                repository_ref: p("repo:example"),
                path: p("src/parser.rs"),
                change_kind: ChangeKind::Modified,
            }],
            external_effects: vec![],
            deviations: vec![],
            open_gaps: vec![],
            owner_decisions: vec![],
            blockers: vec![],
            worktree: WorktreeInput {
                state: WorktreeState::Removed,
                reason: None,
                responsible: None,
                cleanup_condition: None,
            },
            next_step: NextStepInput {
                action: Some(t("Принять результат")),
                gate: None,
                actor_ref: Some(t("example-owner")),
            },
        },
    }
}

fn entry(kind: &str, id: &str, reference: &str) -> ResolvedEntry {
    ResolvedEntry {
        record_type: text(kind),
        id: text(id),
        reference: text(reference),
        revision: text(EXACT),
        ..ResolvedEntry::default()
    }
}

fn catalogue() -> ResolutionCatalogue {
    let mut c = ResolutionCatalogue::new();
    let mut run = entry("execution-run", "run-1", RUN_REF);
    run.resolved_state = ResponseField::Present(ResolvedStateResponse {
        task_specification_ref: text(SPEC_REF),
        scope_revision: ResponseField::Present(1),
        lifecycle_stage: text("acceptance"),
        work_status: text("active"),
        next_action: ResponseField::Present(Some("Принять результат".to_string())),
        next_gate: ResponseField::Present(None),
        blocker_ids: items(&[]),
        resolved_norms: items(&[]),
        completed_checks: items(&[CHECK_REF]),
        unknown_fields: vec![],
    });
    c.insert(RUN_REF, run);
    let mut spec = entry("task-specification", "spec-1", SPEC_REF);
    spec.specification = SpecificationResponse {
        acceptance_criteria: items(&["ac-1"]),
        mandatory_checks: items(&[CHECK_REF]),
    };
    c.insert(SPEC_REF, spec);
    let mut hc = entry("run-human-control", "hc-1", HC_REF);
    hc.linked_run_ref = text(RUN_REF);
    c.insert(HC_REF, hc);
    let mut cm = entry("context-manifest", "cm-1", CM_REF);
    cm.linked_run_ref = text(RUN_REF);
    c.insert(CM_REF, cm);
    let mut ev = entry("evidence-result", "evidence-1", EVIDENCE_REF);
    ev.evidence_result = EvidenceResultResponse {
        observed_result: text("confirmed"),
        covers: items(&["assertion-1"]),
        check_ref: text(CHECK_REF),
        ..EvidenceResultResponse::default()
    };
    c.insert(EVIDENCE_REF, ev);
    c
}

fn run(input: HandoffInput, catalogue: &ResolutionCatalogue) -> Vec<String> {
    let (accepted, problems) = check_handoff(input, Some(catalogue));
    assert_eq!(accepted.is_some(), problems.is_empty());
    problems.iter().map(|d| d.message().to_string()).collect()
}

fn problems(mutate: impl FnOnce(&mut HandoffInput)) -> Vec<String> {
    let mut i = input();
    mutate(&mut i);
    run(i, &catalogue())
}

fn resolved_problems(mutate: impl FnOnce(&mut ResolutionCatalogue)) -> Vec<String> {
    let mut c = catalogue();
    mutate(&mut c);
    run(input(), &c)
}

fn has(found: &[String], expected: &str) {
    assert!(
        found.iter().any(|m| m == expected),
        "expected {expected:?} among {found:#?}"
    );
}

fn with_run_state(c: &mut ResolutionCatalogue, f: impl FnOnce(&mut ResolvedStateResponse)) {
    let mut e = c.resolve(RUN_REF).cloned().unwrap();
    if let ResponseField::Present(state) = &mut e.resolved_state {
        f(state);
    }
    c.insert(RUN_REF, e);
}

fn with_entry(c: &mut ResolutionCatalogue, reference: &str, f: impl FnOnce(&mut ResolvedEntry)) {
    let mut e = c.resolve(reference).cloned().unwrap();
    f(&mut e);
    c.insert(reference, e);
}

#[test]
fn evidence_handoff_the_reference_handoff_is_accepted_with_computed_statuses() {
    let (accepted, problems) = check_handoff(input(), Some(&catalogue()));
    assert!(problems.is_empty(), "{problems:#?}");
    let h = accepted.unwrap();
    assert_eq!(h.run_id().as_str(), "run-1");
    assert_eq!(h.outcome(), OutcomeStatus::HandedOffIncomplete);
    let claimed: Vec<_> = h.claimed_results().map(|(_, s)| s).collect();
    assert_eq!(claimed, [ClaimedResultStatus::Established]);
    let (_, resolved) = h.evidence().next().unwrap();
    assert_eq!(resolved.covers, [sid("assertion-1")]);
    assert_eq!(h.executed_checks().count(), 1);
}

#[test]
fn evidence_handoff_without_a_resolver_fails_closed() {
    let (accepted, problems) = check_handoff(input(), None);
    assert!(accepted.is_none());
    assert_eq!(problems.len(), 1);
    assert!(problems[0].message().starts_with(&format!(
        "{H} cannot be verified: no external record resolver was supplied"
    )));
}

#[test]
fn evidence_handoff_section_3_schema_declaration() {
    let found = problems(|i| {
        i.envelope.declared_schema = t("../../registries/operating-model/scoped-record.schema.json")
    });
    assert!(found[0].starts_with(&format!(
        "{H} $schema \"../../registries/operating-model/scoped-record.schema.json\" resolves to the record envelope"
    )));
}

#[test]
fn evidence_handoff_section_4_russian_title() {
    has(
        &problems(|i| i.envelope.title = t("English only")),
        &format!("{H} title \"English only\" carries no Russian (Cyrillic) text; the handoff name is stated in Russian for the human reader"),
    );
}

#[test]
fn evidence_handoff_section_5_run_identity_and_origin() {
    has(
        &problems(|i| i.envelope.origin = RecordOrigin::BuiltIn),
        &format!("{H} declares origin.kind \"built-in\"; a handoff is written in a workspace, not shipped with the methodology"),
    );
    has(
        &problems(|i| i.scope = RunStateScope::new(RunId::new("run-10").unwrap(), WorkspaceId::new("workspace-1").unwrap())),
        &format!("{H} scope identifies run \"run-10\" but execution_run_ref.id is \"run-1\"; the two must be the exact same string, not one a substring of the other"),
    );
}

#[test]
fn evidence_handoff_section_5a_envelope_references_are_portable() {
    has(
        &problems(|i| i.envelope.authority.decision_ref = Some(p("/etc/passwd"))),
        &format!("{H} authority.decision_ref contains a rooted (absolute) POSIX path; a handoff is portable and carries no rooted machine path"),
    );
}

#[test]
fn evidence_handoff_section_7_four_pinned_references_and_their_run() {
    has(
        &problems(|i| i.body.pins.context_manifest.record_type = PinnedRecordKind::TaskSpecification),
        &format!("{H} context_manifest_ref names record_type \"task-specification\", not \"context-manifest\""),
    );
    has(
        &problems(|i| i.body.pins.task_specification.revision = Some(t("main"))),
        &format!("{H} task_specification_ref is not pinned to an exact edition: revision \"main\" is a branch or channel reference; a branch or channel is never a pin, whatever digits it carries"),
    );
    has(
        &problems(|i| i.body.pins.task_specification.run_id = Some(sid("run-1"))),
        &format!("{H} task_specification_ref carries run_id \"run-1\"; a task specification is not run-scoped and names no run_id"),
    );
    has(
        &problems(|i| i.body.pins.context_manifest.run_id = None),
        &format!("{H} context_manifest_ref carries no run_id; a context-manifest record explicitly names the run it belongs to"),
    );
    has(
        &problems(|i| i.body.pins.human_control.run_id = Some(sid("run-2"))),
        &format!("{H} human_control_ref run_id \"run-2\" is not the referenced run \"run-1\"; the run-human-control record must belong to the same run"),
    );
    has(
        &problems(|i| i.body.pins.execution_run.run_id = Some(sid("run-2"))),
        &format!("{H} execution_run_ref run_id \"run-2\" is not the run's own id \"run-1\""),
    );
    has(
        &problems(|i| i.body.pins.execution_run.reference = p("/abs/run")),
        &format!("{H} execution_run_ref reference contains a rooted (absolute) POSIX path; the reference is portable and is not an absolute machine path"),
    );
}

#[test]
fn evidence_handoff_section_8_claimed_results() {
    has(
        &problems(|i| {
            let c = i.body.claimed_results[0].clone();
            i.body.claimed_results.push(c);
        }),
        &format!("{H} claimed result id \"result-1\" is used more than once"),
    );
    has(
        &problems(|i| i.body.claimed_results[0].statement = t("  ")),
        &format!("{H} claimed result result-1 has no statement"),
    );
}

#[test]
fn evidence_handoff_section_9_assertions() {
    has(
        &problems(|i| i.body.assertions[0].claimed_result_id = sid("result-9")),
        &format!("{H} verifiable assertion assertion-1 names claimed_result_id \"result-9\", which is not a declared claimed result"),
    );
    has(
        &problems(|i| i.body.assertions[0].unverified_reason = Some(t("причина"))),
        &format!(
            "{H} verifiable assertion assertion-1 is verified but carries an unverified_reason"
        ),
    );
    has(
        &problems(|i| {
            i.body.assertions[0].status = AssertionStatus::Unverified;
            i.body.claimed_results[0].status = ClaimedResultStatus::NotEstablished;
        }),
        &format!("{H} verifiable assertion assertion-1 is unverified but states no unverified_reason; the unproven stays explicitly unproven"),
    );
}

#[test]
fn evidence_handoff_section_10_evidence_is_pinned_and_narrow() {
    has(
        &problems(|i| i.body.evidence[0].evidence.revision = Some(t("abc123"))),
        &format!("{H} evidence entry evidence-1 is not pinned to an exact edition: revision \"abc123\" looks like an abbreviated or ambiguous Git SHA (not a full 40- or 64-hex object name); pin the full SHA or add a sha256 digest; a plain reference is not verifiable evidence"),
    );
    has(
        &problems(|i| i.body.evidence[0].covers.push(sid("assertion-9"))),
        &format!("{H} evidence entry evidence-1 covers \"assertion-9\", which is not a declared verifiable assertion; a narrow evidence entry is not widened to an undeclared assertion"),
    );
    has(
        &problems(|i| i.body.evidence[0].limitations.push(t(" "))),
        &format!("{H} evidence entry evidence-1 limitations[0] is empty or whitespace-only"),
    );
    has(
        &problems(|i| i.body.evidence[0].recorded_verdict = Some(t("accepted"))),
        &format!("{H} evidence entry evidence-1 carries \"recorded_verdict\" but its kind is \"check-run\", not \"specialised-evidence-record\""),
    );
    has(
        &problems(|i| i.body.evidence[0].evidence.kind = EvidenceKind::SpecialisedEvidenceRecord),
        &format!("{H} evidence entry evidence-1 is a specialised-evidence-record but states no specialised_contract; the specialised contract keeps authority over its own verdict, which this handoff records rather than re-derives"),
    );
}

#[test]
fn evidence_handoff_section_11_established_is_computed_on_both_sides() {
    has(
        &problems(|i| {
            i.body.assertions[0].status = AssertionStatus::Unverified;
            i.body.assertions[0].unverified_reason = Some(t("не проверено"));
        }),
        &format!("{H} claimed result \"result-1\" is established but assertion(s) assertion-1 linked to it are not verified; the unproven cannot be established"),
    );
    has(
        &problems(|i| i.body.claimed_results[0].status = ClaimedResultStatus::NotEstablished),
        &format!("{H} claimed result \"result-1\" is \"not_established\" but it has at least one linked verifiable assertion and every one of them is verified; the status is derived from the evidence, not asserted — this result is established"),
    );
}

#[test]
fn evidence_handoff_section_12_mandatory_checks() {
    has(
        &problems(|i| i.body.mandatory_checks[0].result_evidence_id = None),
        &format!("{H} mandatory check check-1 is \"passed\" but names no result_evidence_id; a passed status is backed by an evidence entry that shows the successful result, not by the completed_checks list"),
    );
    has(
        &problems(|i| i.body.mandatory_checks[0].result_evidence_id = Some(sid("evidence-9"))),
        &format!("{H} mandatory check check-1 result_evidence_id \"evidence-9\" is not a declared evidence entry"),
    );
    has(
        &problems(|i| {
            let m = &mut i.body.mandatory_checks[0];
            m.status = CheckStatus::Unable;
        }),
        &format!("{H} mandatory check check-1 is \"unable\" but names a result_evidence_id; a check that could not run has no result to evidence"),
    );
}

#[test]
fn evidence_handoff_section_12b_acceptance_criteria() {
    has(
        &problems(|i| i.body.acceptance_criteria[0].addressed_by = vec![]),
        &format!("{H} acceptance criterion ac-1 is \"covered\" but names no addressed_by verifiable assertion"),
    );
    has(
        &problems(|i| {
            i.body.acceptance_criteria[0].status = CoverageStatus::Uncovered;
        }),
        &format!(
            "{H} acceptance criterion ac-1 is \"uncovered\" but names addressed_by assertions"
        ),
    );
}

#[test]
fn evidence_handoff_section_13_repository_states_cover_one_set() {
    has(
        &problems(|i| i.body.result_state[0].repository_ref = p("repo:other")),
        &format!("{H} source_state and result_state pin different sets of repositories (source-only: repo:example; result-only: repo:other); a run's before and after states cover the same repositories"),
    );
    has(
        &problems(|i| i.body.source_state[0].revision = Some(t("main"))),
        &format!("{H} source_state entry repo:example is not pinned to an exact revision: revision \"main\" is a branch or channel reference; a branch or channel is never a pin, whatever digits it carries"),
    );
}

#[test]
fn evidence_handoff_section_14_changed_paths_have_a_before_and_after() {
    has(
        &problems(|i| i.body.result_state[0].revision = Some(t(&"a".repeat(40)))),
        &format!("{H} changed path path-1 names repository \"repo:example\", but its source_state and result_state pins are identical; a change produced a new revision"),
    );
    has(
        &problems(|i| {
            let mut c = i.body.changed_paths[0].clone();
            c.id = sid("path-2");
            i.body.changed_paths.push(c);
        }),
        &format!("{H} changed path repeats \"src/parser.rs\" in repository \"repo:example\""),
    );
}

#[test]
fn evidence_handoff_section_15_effects_deviations_gaps_and_decisions() {
    has(
        &problems(|i| {
            i.body.external_effects.push(ExternalEffectInput {
                id: sid("effect-1"),
                description: t(" "),
                reversible: true,
            })
        }),
        &format!("{H} external effect effect-1 has no description"),
    );
    has(
        &problems(|i| {
            i.body.deviations.push(DeviationInput {
                id: sid("dev-1"),
                description: t("Отклонение"),
                severity: DeviationSeverity::NonBlocking,
                disposition: t("/abs"),
            })
        }),
        &format!("{H} deviation dev-1 disposition contains a rooted (absolute) POSIX path"),
    );
    has(
        &problems(|i| {
            i.body.open_gaps.push(OpenGapInput {
                id: sid("gap-1"),
                description: t(" "),
            })
        }),
        &format!("{H} open_gaps entry gap-1 has no description"),
    );
}

#[test]
fn evidence_handoff_section_16_blockers_make_the_outcome_blocked() {
    let found = problems(|i| {
        i.body.blockers.push(BlockerInput {
            id: sid("blocker-1"),
            description: t("Ждём доступ"),
            resumption_condition: t("Доступ выдан"),
        })
    });
    has(&found, &format!("{H} outcome.status is \"handed_off_incomplete\" but a blocker, a failed or unable mandatory check, or a blocking deviation is recorded; that is a \"blocked\" outcome"));
    has(&found, &format!("{H} carries 1 blocker(s) but outcome.status is \"handed_off_incomplete\"; a non-empty blocker set is a \"blocked\" outcome"));
    has(&found, &format!("{H} blockers do not match the resolved execution-run record's blocker_ids; the two sets of blocker ids must be the same"));
}

#[test]
fn evidence_handoff_section_17_worktree_disposition() {
    has(
        &problems(|i| i.body.worktree.state = WorktreeState::Retained),
        &format!("{H} worktree_disposition is \"retained\" but states no reason; a retained worktree carries a reason, a responsible party and a verifiable cleanup condition"),
    );
    has(
        &problems(|i| i.body.worktree.reason = Some(t("причина"))),
        &format!("{H} worktree_disposition is \"removed\" but carries \"reason\"; reason, responsible and cleanup_condition belong to the \"retained\" state only"),
    );
}

#[test]
fn evidence_handoff_section_18_next_step_and_the_resolved_run() {
    has(
        &problems(|i| i.body.next_step.gate = Some(t(" "))),
        &format!(
            "{H} next_step.gate is present but empty; state a value, or null where there is none"
        ),
    );
    has(
        &problems(|i| i.body.next_step.action = Some(t("Другое действие"))),
        &format!("{H} next_step.action \"Другое действие\" does not match the resolved execution-run record's next_action \"Принять результат\""),
    );
}

#[test]
fn evidence_handoff_section_19_outcome_axis_rules() {
    let found = problems(|i| i.body.outcome.status = OutcomeStatus::Complete);
    has(&found, &format!("{H} outcome.status is \"complete\" but the resolved execution-run record's work_status is \"active\", not \"completed\"; a complete outcome is the whole run finished"));
    has(&found, &format!("{H} outcome.status is \"complete\" but next_step still carries an executable action; a completed run has no next step"));
    has(
        &problems(|i| i.body.outcome.status = OutcomeStatus::Blocked),
        &format!("{H} outcome.status is \"blocked\" but no blocker, failed or unable mandatory check, or blocking deviation is recorded"),
    );
    has(
        &problems(|i| {
            i.body.outcome.status = OutcomeStatus::Complete;
            i.body.open_gaps.push(OpenGapInput { id: sid("gap-1"), description: t("Пробел") });
        }),
        &format!("{H} outcome.status is \"complete\" but 1 open gap(s) remain; a completed run carries no acknowledged incompleteness — record it as a deviation or leave the run handed_off_incomplete"),
    );
}

#[test]
fn evidence_handoff_section_20_every_pin_and_evidence_is_resolved() {
    has(
        &resolved_problems(|c| {
            *c = {
                let mut n = ResolutionCatalogue::new();
                for r in [RUN_REF, SPEC_REF, HC_REF, EVIDENCE_REF] {
                    n.insert(r, c.resolve(r).cloned().unwrap());
                }
                n
            }
        }),
        &format!("{H} context_manifest_ref does not resolve to an actual context-manifest through the external resolver; a pinned reference that resolves to nothing is not a verified pin and the handoff fails closed"),
    );
    has(
        &resolved_problems(|c| with_entry(c, EVIDENCE_REF, |e| e.evidence_result.observed_result = text("inconclusive"))),
        &format!("{H} verifiable assertion \"assertion-1\" is \"verified\" but no resolved, pin-checked evidence entry confirms it (a covering evidence entry that resolves and whose observed_result is \"confirmed\"); a plain reference is not proof"),
    );
    has(
        &resolved_problems(|c| with_entry(c, EVIDENCE_REF, |e| e.evidence_result.covers = items(&["assertion-2"]))),
        &format!("{H} evidence entry evidence-1: the resolved evidence-result does not confirm that this evidence bears on assertion \"assertion-1\"; a resolved evidence entry's covers is not transferred to a foreign assertion"),
    );
    has(
        &resolved_problems(|c| with_entry(c, EVIDENCE_REF, |e| e.evidence_result.check_ref = text("checks/other"))),
        &format!("{H} mandatory check check-1 names result evidence \"evidence-1\", but the resolved evidence-result records check_ref \"checks/other\", not this check's check_ref \"checks/unit-tests\"; a check's result is not another check's result"),
    );
    has(
        &resolved_problems(|c| with_entry(c, SPEC_REF, |e| e.specification.acceptance_criteria = items(&["ac-1", "ac-2"]))),
        &format!("{H} does not address acceptance criterion \"ac-2\" declared by the resolved task-specification record; every criterion is covered or explicitly uncovered"),
    );
    has(
        &resolved_problems(|c| with_entry(c, SPEC_REF, |e| e.specification.mandatory_checks = items(&[CHECK_REF, "checks/lint"]))),
        &format!("{H} omits mandatory check \"checks/lint\" declared by the resolved task-specification record; removing a mandatory check from the handoff is rejected"),
    );
    has(
        &resolved_problems(|c| with_run_state(c, |s| s.completed_checks = items(&[]))),
        &format!("{H} mandatory check \"checks/unit-tests\" ran (its status is \"passed\" or \"failed\") but its check_ref is not among the resolved execution-run record's completed_checks; a check that ran — passed OR failed — is a completed check of the run, and completed_checks confirms the fact it ran, not the verdict"),
    );
    has(
        &resolved_problems(|c| with_run_state(c, |s| s.task_specification_ref = text("records/task-specification/spec-2"))),
        &format!("{H} task_specification_ref names \"records/task-specification/spec-1\", but the resolved execution-run record names task specification \"records/task-specification/spec-2\"; the handoff carries the SAME specification the run resolved, not an independent one"),
    );
    has(
        &resolved_problems(|c| with_entry(c, CM_REF, |e| e.linked_run_ref = text("records/execution-run/run-2"))),
        &format!("{H} context_manifest_ref resolves to a context-manifest record whose linked_run_ref is \"records/execution-run/run-2\", not the handoff's pinned run \"records/execution-run/run-1\""),
    );
    has(
        &resolved_problems(|c| with_entry(c, SPEC_REF, |e| e.other_fields = vec!["resolved_state".to_string()])),
        &format!("{H} task_specification_ref: the resolver returned a record with an unknown field \"resolved_state\"; the transformer response is closed to {{ record_type, id, reference, revision, content_digest, source_bytes, acceptance_criteria, mandatory_checks }}"),
    );
}

#[test]
fn evidence_handoff_a_pin_by_digest_is_verified_against_resolved_source() {
    let digest = crate::types::ContentDigest::of_str("spec body")
        .value()
        .to_string();
    let mut i = input();
    i.body.pins.task_specification.revision = None;
    i.body.pins.task_specification.sha256 = Some(PinSha256::new(digest.clone()).unwrap());
    let mut c = catalogue();
    with_entry(&mut c, SPEC_REF, |e| {
        e.revision = ResponseField::Absent;
        e.source_bytes = text("spec body");
    });
    assert_eq!(run(i.clone(), &c), Vec::<String>::new());
    with_entry(&mut c, SPEC_REF, |e| e.source_bytes = text("tampered"));
    let found = run(i, &c);
    assert!(found.iter().any(|m| m.starts_with(&format!(
        "{H} task_specification_ref pins sha256 \"{digest}\" but the SHA-256 of the resolved source is"
    ))));
}

#[test]
fn evidence_handoff_diagnostics_are_deterministic() {
    let mutate = |i: &mut HandoffInput| {
        i.body.assertions.push(AssertionInput {
            id: sid("assertion-2"),
            statement: t("Второе"),
            claimed_result_id: sid("result-1"),
            status: AssertionStatus::Verified,
            unverified_reason: None,
        });
        i.body.assertions.push(AssertionInput {
            id: sid("assertion-0"),
            statement: t("Третье"),
            claimed_result_id: sid("result-1"),
            status: AssertionStatus::Verified,
            unverified_reason: None,
        });
    };
    let first = problems(mutate);
    for _ in 0..5 {
        assert_eq!(problems(mutate), first);
    }
    let unsupported: Vec<&String> = first
        .iter()
        .filter(|m| m.contains("is \"verified\" but no resolved"))
        .collect();
    assert_eq!(unsupported.len(), 2);
    assert!(
        unsupported[0].contains("\"assertion-2\""),
        "declaration order"
    );
    assert!(
        unsupported[1].contains("\"assertion-0\""),
        "declaration order"
    );
}
