//! `upgrade-integration-qualification`
//! (`registries/operating-model/upgrade-integration-qualification.schema.json`,
//! `scripts/lib/upgrade-integration-qualification.mjs`): the closed verdict
//! over one workspace's task journey (one evidence-and-handoff record), an
//! optional field-evaluation report and exactly three neutral scenarios,
//! each a task specification and its execution run.
//!
//! Every composed record is checked by its own app-owned route through that
//! family's typed composition point; this module receives each slot's
//! resolution, its content pin, the composing check's diagnostics and — only
//! for an accepted record — the typed facts the matrix reads.

use crate::evidence::handoff::input::OutcomeStatus;
use crate::migration::record::RecordHead;
use crate::run_contracts::{LifecycleStage, PinnedRecordKind, RecordText};
use crate::types::{AuthorityKind, Diagnostic, OriginKind, ScopeType, SemanticId};

use super::pin::{check_resolution, QualificationPin, ResolvedRecord};
use super::{fail, prefixed, Composed, QualificationState};

const RECORD_TYPE: &str = "upgrade-integration-qualification";

/// The three required neutral scenarios (`definitions.scenario_id`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScenarioId {
    SingleModuleRefactor,
    MultiRepositoryDecomposition,
    LanguageChangeLimitCase,
}

impl ScenarioId {
    pub const ALL: [ScenarioId; 3] = [
        ScenarioId::SingleModuleRefactor,
        ScenarioId::MultiRepositoryDecomposition,
        ScenarioId::LanguageChangeLimitCase,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            ScenarioId::SingleModuleRefactor => "single-module-refactor",
            ScenarioId::MultiRepositoryDecomposition => "multi-repository-decomposition",
            ScenarioId::LanguageChangeLimitCase => "language-change-limit-case",
        }
    }

    pub fn parse(value: &str) -> Option<ScenarioId> {
        Self::ALL.into_iter().find(|s| s.as_str() == value)
    }

    /// The task pattern this scenario's specification must be classified as.
    const fn expected_pattern(self) -> &'static str {
        match self {
            ScenarioId::SingleModuleRefactor => "refactor-preserving-behavior",
            ScenarioId::MultiRepositoryDecomposition | ScenarioId::LanguageChangeLimitCase => {
                "decompose-initiative"
            }
        }
    }
}

/// `definitions.scenario_classification`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioInput {
    pub scenario_id: ScenarioId,
    pub task_specification_ref: QualificationPin,
    pub execution_state_ref: QualificationPin,
}

/// `definitions.payload` (`workspace_transition_compatibility.status` is the
/// schema constant `kernel-template-only`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradePayloadInput {
    pub task_journey_ref: QualificationPin,
    pub field_evaluation_report_ref: Option<QualificationPin>,
    pub scenario_classifications: Vec<ScenarioInput>,
    pub transition_notes: RecordText,
    pub qualification_state: QualificationState,
    pub qualification_reason: RecordText,
    pub blockers: Vec<RecordText>,
    pub open_questions: Vec<RecordText>,
}

/// One entry of `qualifications`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeQualificationInput {
    pub head: RecordHead,
    pub payload: UpgradePayloadInput,
}

/// What the qualification reads from the accepted task journey.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JourneyFacts {
    pub outcome: OutcomeStatus,
    pub workspace_id: String,
}

/// What the qualification reads from an accepted field-evaluation report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportFacts {
    pub workspace_id: String,
}

/// What the qualification reads from an accepted task specification: its
/// resolved task pattern and the workspace its scope names (`scope.id` of a
/// `project-workspace`, else `scope.workspace_id`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecificationFacts {
    pub task_pattern: SemanticId,
    pub workspace_id: Option<String>,
}

/// What the qualification reads from an accepted execution run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunFacts {
    pub lifecycle_stage: LifecycleStage,
    pub task_specification_ref: String,
    pub workspace_id: String,
}

/// One pinned reference after resolution and composition.
#[derive(Debug, Clone, PartialEq)]
pub enum RecordSlot<F> {
    Unresolved,
    Resolved {
        record: ResolvedRecord,
        /// `None` when the pin does not confirm the resolved content, so the
        /// record was never composed.
        composed: Option<Composed<F>>,
    },
}

/// One scenario's two slots.
#[derive(Debug, Clone, PartialEq)]
pub struct ScenarioSlots {
    pub specification: RecordSlot<SpecificationFacts>,
    pub run: RecordSlot<RunFacts>,
}

/// Everything a qualification entry composes. `field_report` is `None`
/// exactly when the reference is `null`; `scenarios` is parallel to
/// `scenario_classifications`.
#[derive(Debug, Clone, PartialEq)]
pub struct UpgradeComposition {
    pub journey: RecordSlot<JourneyFacts>,
    pub field_report: Option<RecordSlot<ReportFacts>>,
    pub scenarios: Vec<ScenarioSlots>,
}

/// The inputs of the closed decision matrix
/// (`upgrade-integration-qualification.md` §5), all from accepted records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecisionInputs {
    /// `None` when the journey is not accepted.
    pub journey_outcome: Option<OutcomeStatus>,
    pub scenario_coverage_ok: bool,
    pub all_scenarios_clean: bool,
    /// `None` for a `null` reference, else whether the report is accepted.
    pub field_report_accepted: Option<bool>,
}

/// `computeIntegrationQualificationState`.
pub fn compute_qualification_state(inputs: DecisionInputs) -> QualificationState {
    let Some(outcome) = inputs.journey_outcome else {
        return QualificationState::Blocked;
    };
    if outcome == OutcomeStatus::Blocked
        || !inputs.scenario_coverage_ok
        || !inputs.all_scenarios_clean
    {
        return QualificationState::Blocked;
    }
    if outcome == OutcomeStatus::HandedOffIncomplete {
        return QualificationState::Unverified;
    }
    match inputs.field_report_accepted {
        Some(false) => QualificationState::Blocked,
        None => QualificationState::Unverified,
        Some(true) => QualificationState::Qualified,
    }
}

/// An accepted qualification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeQualification {
    pub id: SemanticId,
    pub state: QualificationState,
}

fn same_workspace_note() -> &'static str {
    "the task journey, the field-evaluation report and every scenario must belong to the same workspace"
}

/// `resolveNamedRecord` (echo and content pin) and composition of one
/// slot; returns the accepted facts. `None` from here means "not accepted".
fn compose_slot<'a, F>(
    at: &str,
    field: &str,
    pin: &QualificationPin,
    slot: &'a RecordSlot<F>,
    expected: PinnedRecordKind,
    problems: &mut Vec<Diagnostic>,
) -> SlotOutcome<'a, F> {
    let (record, composed) = match slot {
        RecordSlot::Resolved { record, composed } => (Some(record), composed.as_ref()),
        RecordSlot::Unresolved => (None, None),
    };
    if !check_resolution(at, field, pin, record, expected, problems) {
        return SlotOutcome::NotComposed;
    }
    let Some(record) = record else {
        return SlotOutcome::NotComposed;
    };
    let digest = record.content_digest();
    if !pin.confirms(&digest) {
        problems.push(fail(format!(
            "{at} {field} \"{}\": sha256 \"{}\" does not equal the resolved record's own recomputed content digest \"{}\"; a pinned reference names the exact composed version, never a bare id/reference an independent resolver could satisfy with different content under the same label",
            pin.reference,
            pin.sha256,
            digest.value()
        )));
        return SlotOutcome::NotComposed;
    }
    let Some(composed) = composed else {
        return SlotOutcome::NotComposed;
    };
    problems.extend(prefixed(&format!("{at} {field}: "), &composed.diagnostics));
    match &composed.accepted {
        Some(facts) if composed.diagnostics.is_empty() => SlotOutcome::Accepted(facts),
        _ => SlotOutcome::Rejected,
    }
}

enum SlotOutcome<'a, F> {
    NotComposed,
    Rejected,
    Accepted(&'a F),
}

impl<'a, F> SlotOutcome<'a, F> {
    fn accepted(&self) -> Option<&'a F> {
        match self {
            SlotOutcome::Accepted(f) => Some(f),
            _ => None,
        }
    }
}

fn check_head(at: &str, head: &RecordHead, problems: &mut Vec<Diagnostic>) {
    if head.record_type.as_str() != RECORD_TYPE {
        problems.push(fail(format!(
            "{at} declares record_type \"{}\", not \"{RECORD_TYPE}\"",
            head.record_type
        )));
    }
    let scope_type = head.scope.scope_type();
    if scope_type != ScopeType::ProjectWorkspace {
        problems.push(fail(format!(
            "{at} scope.type is \"{scope_type}\"; an upgrade-integration-qualification is scoped to project-workspace only"
        )));
    }
    let origin = head.origin_kind();
    if origin != OriginKind::Derived {
        problems.push(fail(format!(
            "{at} origin.kind is \"{origin}\", not \"derived\"; a qualification is computed from the composed records it pins, never declared on its own"
        )));
    }
    let authority = head.authority.kind;
    if authority != AuthorityKind::DelegatedRun {
        problems.push(fail(format!(
            "{at} authority.kind is \"{authority}\", not \"delegated-run\"; computing a qualification is carried out by a run and never itself mints an owner decision"
        )));
    }
}

/// `checkScenarioExpectation`.
fn check_expectation(
    at: &str,
    scenario: ScenarioId,
    spec: &SpecificationFacts,
    run: &RunFacts,
    problems: &mut Vec<Diagnostic>,
) {
    let expected = scenario.expected_pattern();
    if spec.task_pattern.as_str() != expected {
        problems.push(fail(format!(
            "{at} scenario \"{}\" resolves a task-specification classified as task_pattern \"{}\", not the expected \"{expected}\"",
            scenario.as_str(),
            spec.task_pattern
        )));
    }
    let stage = run.lifecycle_stage;
    match scenario {
        ScenarioId::LanguageChangeLimitCase if stage > LifecycleStage::Classification => {
            problems.push(fail(format!(
                "{at} scenario \"language-change-limit-case\" resolves an execution-run at lifecycle_stage \"{stage}\", past \"classification\"; this scenario proves classification only — no execution of a product rewrite is ever expected to have proceeded"
            )));
        }
        ScenarioId::MultiRepositoryDecomposition if stage <= LifecycleStage::Classification => {
            problems.push(fail(format!(
                "{at} scenario \"multi-repository-decomposition\" resolves an execution-run at lifecycle_stage \"{stage}\", not past \"classification\"; a decomposition initiative that never proceeds past classification does not demonstrate decomposition actually happening"
            )));
        }
        _ => {}
    }
}

fn check_duplicate_scenario_refs<'p>(
    at: &str,
    field: &str,
    pins: impl Iterator<Item = &'p QualificationPin>,
    problems: &mut Vec<Diagnostic>,
) {
    let mut seen: Vec<(&str, usize)> = Vec::new();
    for (i, pin) in pins.enumerate() {
        let reference = pin.reference.as_str();
        match seen.iter().find(|(r, _)| *r == reference) {
            Some((_, first)) => problems.push(fail(format!(
                "{at} {field}[{i}].reference \"{reference}\" repeats {field}[{first}].reference; each scenario names a distinct composed record"
            ))),
            None => seen.push((reference, i)),
        }
    }
}

/// The scenarios; returns `(coverage_ok, all_clean)`.
fn check_scenarios(
    at: &str,
    workspace: &str,
    entry: &UpgradeQualificationInput,
    composition: &UpgradeComposition,
    problems: &mut Vec<Diagnostic>,
) -> (bool, bool) {
    let scenarios = &entry.payload.scenario_classifications;
    if scenarios.len() != ScenarioId::ALL.len() {
        problems.push(fail(format!(
            "{at} payload.scenario_classifications carries {} entries, not exactly {}; every required neutral scenario is covered, with no gap and no extra",
            scenarios.len(),
            ScenarioId::ALL.len()
        )));
    }
    let mut coverage_ok = true;
    for want in ScenarioId::ALL {
        let count = scenarios.iter().filter(|s| s.scenario_id == want).count();
        if count != 1 {
            coverage_ok = false;
            problems.push(fail(format!(
                "{at} payload.scenario_classifications declares scenario_id \"{}\" {count} time(s), expected exactly 1",
                want.as_str()
            )));
        }
    }
    check_duplicate_scenario_refs(
        at,
        "payload.scenario_classifications[].task_specification_ref",
        scenarios.iter().map(|s| &s.task_specification_ref),
        problems,
    );
    check_duplicate_scenario_refs(
        at,
        "payload.scenario_classifications[].execution_state_ref",
        scenarios.iter().map(|s| &s.execution_state_ref),
        problems,
    );

    let mut all_clean = !scenarios.is_empty();
    for (i, scenario) in scenarios.iter().enumerate() {
        let field = format!("payload.scenario_classifications[{i}]");
        let unresolved = ScenarioSlots {
            specification: RecordSlot::Unresolved,
            run: RecordSlot::Unresolved,
        };
        let slots = composition.scenarios.get(i).unwrap_or(&unresolved);

        let spec_field = format!("{field}.task_specification_ref");
        let spec = compose_slot(
            at,
            &spec_field,
            &scenario.task_specification_ref,
            &slots.specification,
            PinnedRecordKind::TaskSpecification,
            problems,
        );
        if let Some(facts) = spec.accepted() {
            let spec_workspace = facts.workspace_id.as_deref();
            if spec_workspace != Some(workspace) {
                problems.push(fail(format!(
                    "{at} {spec_field} resolves to workspace \"{}\", not this qualification's own workspace \"{workspace}\"; {}",
                    spec_workspace.unwrap_or("undefined"),
                    same_workspace_note()
                )));
                all_clean = false;
            }
        }

        let run_field = format!("{field}.execution_state_ref");
        let run = compose_slot(
            at,
            &run_field,
            &scenario.execution_state_ref,
            &slots.run,
            PinnedRecordKind::ExecutionRun,
            problems,
        );
        if let Some(facts) = run.accepted() {
            if facts.workspace_id != workspace {
                problems.push(fail(format!(
                    "{at} {run_field} resolves to scope.workspace_id \"{}\", not this qualification's own workspace \"{workspace}\"; {}",
                    facts.workspace_id,
                    same_workspace_note()
                )));
                all_clean = false;
            }
            let own = scenario.task_specification_ref.reference.as_str();
            if facts.task_specification_ref != own {
                problems.push(fail(format!(
                    "{at} {field}: resolved execution-run's payload.task_specification_ref \"{}\" does not equal this scenario's own task_specification_ref.reference \"{own}\"; an execution-run belonging to a different specification is not this scenario's own run",
                    facts.task_specification_ref
                )));
                all_clean = false;
            }
        }

        let (Some(spec), Some(run)) = (spec.accepted(), run.accepted()) else {
            all_clean = false;
            continue;
        };
        let before = problems.len();
        check_expectation(at, scenario.scenario_id, spec, run, problems);
        if problems.len() > before {
            all_clean = false;
        }
    }
    (coverage_ok, all_clean)
}

fn check_entry(
    entry: &UpgradeQualificationInput,
    composition: &UpgradeComposition,
    problems: &mut Vec<Diagnostic>,
) -> QualificationState {
    let at = format!("qualification \"{}\"", entry.head.id);
    check_head(&at, &entry.head, problems);
    let workspace = entry.head.scope.id();

    let journey = compose_slot(
        &at,
        "payload.task_journey_ref",
        &entry.payload.task_journey_ref,
        &composition.journey,
        PinnedRecordKind::EvidenceAndHandoff,
        problems,
    );
    if let Some(facts) = journey.accepted() {
        if facts.workspace_id != workspace {
            problems.push(fail(format!(
                "{at} payload.task_journey_ref resolves to scope.workspace_id \"{}\", not this qualification's own workspace \"{workspace}\"; {}",
                facts.workspace_id,
                same_workspace_note()
            )));
        }
    }

    let field_report_accepted = match (
        &entry.payload.field_evaluation_report_ref,
        &composition.field_report,
    ) {
        (None, _) => None,
        (Some(pin), slot) => {
            let unresolved = RecordSlot::Unresolved;
            let slot = slot.as_ref().unwrap_or(&unresolved);
            let report = compose_slot(
                &at,
                "payload.field_evaluation_report_ref",
                pin,
                slot,
                PinnedRecordKind::FieldEvaluationReport,
                problems,
            );
            if let Some(facts) = report.accepted() {
                if facts.workspace_id != workspace {
                    problems.push(fail(format!(
                        "{at} payload.field_evaluation_report_ref resolves to payload.workspace_id \"{}\", not this qualification's own workspace \"{workspace}\"; {}",
                        facts.workspace_id,
                        same_workspace_note()
                    )));
                }
            }
            Some(report.accepted().is_some())
        }
    };

    let (scenario_coverage_ok, all_scenarios_clean) =
        check_scenarios(&at, workspace, entry, composition, problems);

    let computed = compute_qualification_state(DecisionInputs {
        journey_outcome: journey.accepted().map(|j| j.outcome),
        scenario_coverage_ok,
        all_scenarios_clean,
        field_report_accepted,
    });
    let declared = entry.payload.qualification_state;
    if declared != computed {
        problems.push(fail(format!(
            "{at} payload.qualification_state is declared \"{declared}\" but recomputes to \"{computed}\"; a qualification_state is recomputed, never trusted on its own"
        )));
    }
    let blockers = &entry.payload.blockers;
    let questions = &entry.payload.open_questions;
    if computed == QualificationState::Blocked && blockers.is_empty() {
        problems.push(fail(format!(
            "{at} qualification_state recomputes to BLOCKED but payload.blockers is empty"
        )));
    }
    if computed != QualificationState::Blocked && !blockers.is_empty() {
        problems.push(fail(format!(
            "{at} qualification_state recomputes to \"{computed}\" but payload.blockers is non-empty; blockers is closed to BLOCKED"
        )));
    }
    if computed == QualificationState::Unverified && questions.is_empty() {
        problems.push(fail(format!(
            "{at} qualification_state recomputes to UNVERIFIED but payload.open_questions is empty"
        )));
    }
    if computed != QualificationState::Unverified && !questions.is_empty() {
        problems.push(fail(format!(
            "{at} qualification_state recomputes to \"{computed}\" but payload.open_questions is non-empty; open_questions is closed to UNVERIFIED"
        )));
    }
    computed
}

/// The result of one batch of qualifications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeBatchOutcome {
    pub diagnostics: Vec<Diagnostic>,
    pub accepted: Vec<Option<UpgradeQualification>>,
}

/// `evaluateUpgradeIntegrationQualification` over one schema-clean
/// container's `qualifications`, each with its own composition.
pub fn check_upgrade_qualifications(
    entries: &[(UpgradeQualificationInput, UpgradeComposition)],
) -> UpgradeBatchOutcome {
    let mut diagnostics = Vec::new();
    let mut accepted = Vec::with_capacity(entries.len());
    let mut seen: Vec<&str> = Vec::new();
    for (entry, composition) in entries {
        let mut problems = Vec::new();
        let id = entry.head.id.as_str();
        if seen.contains(&id) {
            problems.push(fail(format!(
                "qualification \"{id}\" is declared more than once"
            )));
        } else {
            seen.push(id);
        }
        let state = check_entry(entry, composition, &mut problems);
        accepted.push(problems.is_empty().then(|| UpgradeQualification {
            id: entry.head.id.clone(),
            state,
        }));
        diagnostics.extend(problems);
    }
    UpgradeBatchOutcome {
        diagnostics,
        accepted,
    }
}
