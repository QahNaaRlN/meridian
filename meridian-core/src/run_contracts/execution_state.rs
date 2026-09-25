//! `execution-state-model`: the typed state of ONE execution run
//! (`registries/operating-model/execution-state.schema.json`,
//! `standards/workspace/execution-state-model.md`).
//!
//! [`ExecutionRunInput`] is the schema-clean record, typed: every closed
//! pool is an enum, every `minLength: 1` string a [`RecordText`]-shaped
//! value, every ordinal a positive newtype, and the scope is always
//! `run-state`. Everything the schema already rules out — a missing axis,
//! a universal `status` field, a leaked role/supervision/manifest field, a
//! scope other than `run-state`, an unknown stage — is unrepresentable
//! here, so the corresponding Node checks have no typed counterpart. What
//! remains are the cross-field and history rules
//! [`check_execution_run`] enforces; it is the ONLY way to obtain an
//! accepted [`ExecutionRun`].

use std::collections::HashSet;

use crate::task_contracts::non_portable_reason;
use crate::types::{Diagnostic, SemanticId};

use super::envelope::{check_envelope, RecordEnvelope, RecordFamily};
use super::identity::{
    ActorRef, PortableRef, RecordText, RunStateScope, ScopeRevision, TransitionSequence,
};
use super::vocabulary::{LifecycleStage, WorkStatus};
use super::{fail, option_text, NextStep};

/// One structured blocker (shared with the context manifest's own list).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blocker {
    pub id: SemanticId,
    pub description: RecordText,
    pub resumption_condition: RecordText,
}

/// An explicit statement that a lifecycle stage is deliberately skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageSkip {
    pub stage: LifecycleStage,
    pub rationale: RecordText,
}

/// One entry of the ordered transition history. `from_stage`/`from_status`
/// are `None` for the schema's explicit `null` (and for an absent key,
/// which every rule treats identically).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition {
    pub sequence: TransitionSequence,
    pub from_stage: Option<LifecycleStage>,
    pub to_stage: LifecycleStage,
    pub from_status: Option<WorkStatus>,
    pub to_status: WorkStatus,
    pub scope_revision: ScopeRevision,
    pub reason: RecordText,
    pub skipped_stages: Vec<StageSkip>,
    pub backward_rationale: Option<RecordText>,
    pub reopen_rationale: Option<RecordText>,
}

/// The independent axes of the run's current state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionRunState {
    pub task_specification_ref: PortableRef,
    pub lifecycle_stage: LifecycleStage,
    pub work_status: WorkStatus,
    pub scope_revision: ScopeRevision,
    pub current_actor: ActorRef,
    pub resolved_norms: Vec<PortableRef>,
    pub completed_checks: Vec<PortableRef>,
    pub blockers: Vec<Blocker>,
    pub next_action: NextStep,
    pub next_gate: NextStep,
    pub transition_history: Vec<Transition>,
}

/// One schema-clean execution-run record, not yet domain-checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionRunInput {
    pub envelope: RecordEnvelope,
    pub scope: RunStateScope,
    pub state: ExecutionRunState,
}

/// An execution run with zero domain problems. Only
/// [`check_execution_run`] constructs one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionRun(ExecutionRunInput);

impl ExecutionRun {
    pub fn envelope(&self) -> &RecordEnvelope {
        &self.0.envelope
    }

    pub fn scope(&self) -> &RunStateScope {
        &self.0.scope
    }

    pub fn state(&self) -> &ExecutionRunState {
        &self.0.state
    }
}

/// Every cross-field and history rule of the execution-state contract, in
/// the Node reference's diagnostic order. Returns `Some` only when no
/// problem was found.
pub fn check_execution_run(input: ExecutionRunInput) -> (Option<ExecutionRun>, Vec<Diagnostic>) {
    let mut problems = Vec::new();
    check_envelope(RecordFamily::ExecutionRun, &input.envelope, &mut problems);
    let id = input.envelope.id.as_str();
    let state = &input.state;

    let tsr = &state.task_specification_ref;
    if tsr.is_blank() {
        problems.push(fail(format!(
            "execution run \"{id}\" task_specification_ref is empty or whitespace-only"
        )));
    } else if let Some(r) = non_portable_reason(Some(tsr.as_str())) {
        problems.push(fail(format!(
            "execution run \"{id}\" task_specification_ref contains {r}; the reference is portable and is not an absolute machine path"
        )));
    }

    check_blockers(id, state, &mut problems);
    check_next_steps(id, state, &mut problems);

    for (label, refs) in [
        ("resolved_norms", &state.resolved_norms),
        ("completed_checks", &state.completed_checks),
    ] {
        check_unique_refs(
            &format!("execution run \"{id}\""),
            label,
            refs,
            &mut problems,
        );
    }

    if let Some(r) = non_portable_reason(Some(state.current_actor.as_str())) {
        problems.push(fail(format!(
            "execution run \"{id}\" current_actor contains {r}; the actor is a portable opaque reference and grants no role or authority"
        )));
    }

    check_history(id, state, &mut problems);

    let accepted = problems.is_empty().then_some(ExecutionRun(input));
    (accepted, problems)
}

/// A structured blocker list: unique ids, a description and a verifiable
/// resumption condition, portable prose. Shared by the execution run and
/// the context manifest (`subject` names the record); returns the ids.
pub(crate) fn check_blocker_list(
    subject: &str,
    blockers: &[Blocker],
    problems: &mut Vec<Diagnostic>,
) -> HashSet<String> {
    let mut seen = HashSet::new();
    for blocker in blockers {
        let at = blocker.id.as_str();
        if !seen.insert(at.to_string()) {
            problems.push(fail(format!(
                "{subject} blocker id \"{at}\" is used more than once"
            )));
        }
        if blocker.description.is_blank() {
            problems.push(fail(format!("{subject} blocker {at} has no description")));
        }
        if blocker.resumption_condition.is_blank() {
            problems.push(fail(format!(
                "{subject} blocker {at} has no verifiable resumption condition"
            )));
        }
        for text in [&blocker.description, &blocker.resumption_condition] {
            if let Some(r) = non_portable_reason(Some(text.as_str())) {
                problems.push(fail(format!("{subject} blocker {at} contains {r}")));
            }
        }
    }
    seen
}

fn check_blockers(id: &str, state: &ExecutionRunState, problems: &mut Vec<Diagnostic>) {
    check_blocker_list(
        &format!("execution run \"{id}\""),
        &state.blockers,
        problems,
    );
    let blocked = state.work_status == WorkStatus::Blocked;
    if !state.blockers.is_empty() && !blocked {
        problems.push(fail(format!(
            "execution run \"{id}\" carries {} blocker(s) but work_status is \"{}\"; a non-empty blocker agrees with work_status \"blocked\" (waiting_human and blocked are not interchangeable)",
            state.blockers.len(),
            state.work_status
        )));
    }
    if state.blockers.is_empty() && blocked {
        problems.push(fail(format!(
            "execution run \"{id}\" work_status is \"blocked\" with no blocker; \"blocked\" means continuation is impossible until a stated external condition changes"
        )));
    }
}

fn check_next_steps(id: &str, state: &ExecutionRunState, problems: &mut Vec<Diagnostic>) {
    let steps = [
        ("next_action", &state.next_action),
        ("next_gate", &state.next_gate),
    ];
    for (label, step) in steps {
        if step.as_ref().is_some_and(RecordText::is_blank) {
            problems.push(fail(format!(
                "execution run \"{id}\" {label} is present but empty; state an action, or null where there is no continuation"
            )));
        }
    }
    if state.work_status.is_terminal() {
        for (label, step) in steps {
            if let Some(value) = step {
                problems.push(fail(format!(
                    "execution run \"{id}\" work_status is \"{}\" but {label} carries an executable value \"{value}\"; a completed or cancelled run does not silently carry a next executable step",
                    state.work_status
                )));
            }
        }
    }
    if state.work_status == WorkStatus::WaitingHuman
        && state.next_action.as_ref().is_none_or(RecordText::is_blank)
    {
        problems.push(fail(format!(
            "execution run \"{id}\" work_status is \"waiting_human\" but next_action names no concrete human action; \"waiting_human\" means a specific required human action is known"
        )));
    }
}

/// A list of unique portable references: blank, non-portable and
/// (trimmed) repeated members are reported per element. Shared by the
/// execution run and the context manifest (`subject` names the record).
pub(crate) fn check_unique_refs(
    subject: &str,
    label: &str,
    refs: &[PortableRef],
    problems: &mut Vec<Diagnostic>,
) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for (i, r) in refs.iter().enumerate() {
        if r.is_blank() {
            problems.push(fail(format!(
                "{subject} {label}[{i}] is empty or whitespace-only"
            )));
            continue;
        }
        if let Some(reason) = non_portable_reason(Some(r.as_str())) {
            problems.push(fail(format!("{subject} {label}[{i}] contains {reason}")));
        }
        let key = r.as_str().trim().to_string();
        if seen.contains(&key) {
            problems.push(fail(format!("{subject} {label} repeats reference \"{r}\"")));
        } else {
            seen.push(key);
        }
    }
    seen
}

fn check_history(id: &str, state: &ExecutionRunState, problems: &mut Vec<Diagnostic>) {
    let history = &state.transition_history;
    let Some(last) = history.last() else {
        problems.push(fail(format!(
            "execution run \"{id}\" transition_history is empty; the history is mandatory and its first record states the initial state"
        )));
        return;
    };

    let mut prev: Option<&Transition> = None;
    for (i, t) in history.iter().enumerate() {
        let at = format!("transition #{i}");
        if t.reason.is_blank() {
            problems.push(fail(format!(
                "execution run \"{id}\" {at} states no reason; every transition states why it happened"
            )));
        }
        for (value, label) in [
            (Some(&t.reason), "reason"),
            (t.backward_rationale.as_ref(), "backward_rationale"),
            (t.reopen_rationale.as_ref(), "reopen_rationale"),
        ] {
            if let Some(r) = value.and_then(|v| non_portable_reason(Some(v.as_str()))) {
                problems.push(fail(format!(
                    "execution run \"{id}\" {at} {label} contains {r}"
                )));
            }
        }

        if let Some(p) = prev {
            let (s, before) = (t.sequence, p.sequence);
            if s == before {
                problems.push(fail(format!(
                    "execution run \"{id}\" {at} repeats sequence ordinal {s}; ordinals do not repeat"
                )));
            } else if s < before {
                problems.push(fail(format!(
                    "execution run \"{id}\" {at} sequence ordinal {s} is below the previous {before}; ordinals do not decrease"
                )));
            }
            if t.scope_revision < p.scope_revision {
                problems.push(fail(format!(
                    "execution run \"{id}\" {at} scope_revision {} is below the previous {}; the scope revision does not decrease",
                    t.scope_revision, p.scope_revision
                )));
            }
        }

        match prev {
            None => {
                if t.from_stage.is_some() || t.from_status.is_some() {
                    problems.push(fail(format!(
                        "execution run \"{id}\" {at} is the first record but names a prior from_stage/from_status; the first record states the initial state (from_stage and from_status are null)"
                    )));
                }
            }
            Some(p) => {
                if t.from_stage != Some(p.to_stage) {
                    problems.push(fail(format!(
                        "execution run \"{id}\" {at} from_stage \"{}\" does not continue the previous to_stage \"{}\"; the history has no break",
                        option_text(t.from_stage),
                        p.to_stage
                    )));
                }
                if t.from_status != Some(p.to_status) {
                    problems.push(fail(format!(
                        "execution run \"{id}\" {at} from_status \"{}\" does not continue the previous to_status \"{}\"; the history has no break",
                        option_text(t.from_status),
                        p.to_status
                    )));
                }
                if let Some(from) = t.from_stage {
                    check_stage_move(id, &at, t, from, problems);
                }
            }
        }

        for skip in &t.skipped_stages {
            if skip.rationale.is_blank() {
                problems.push(fail(format!(
                    "execution run \"{id}\" {at} skipped_stages entry for \"{}\" has no rationale",
                    skip.stage
                )));
            }
            if let Some(r) = non_portable_reason(Some(skip.rationale.as_str())) {
                problems.push(fail(format!(
                    "execution run \"{id}\" {at} skipped_stages rationale contains {r}"
                )));
            }
        }

        let after_terminal = prev.is_some_and(|p| p.to_status.is_terminal());
        if after_terminal && t.reopen_rationale.as_ref().is_none_or(RecordText::is_blank) {
            problems.push(fail(format!(
                "execution run \"{id}\" {at} follows a completed or cancelled state with no reopen_rationale; a new transition after a terminal state is rejected without an explicitly allowed reopen basis"
            )));
        }
        prev = Some(t);
    }

    if last.to_stage != state.lifecycle_stage {
        problems.push(fail(format!(
            "execution run \"{id}\" last transition to_stage \"{}\" does not match the current lifecycle_stage \"{}\"",
            last.to_stage, state.lifecycle_stage
        )));
    }
    if last.to_status != state.work_status {
        problems.push(fail(format!(
            "execution run \"{id}\" last transition to_status \"{}\" does not match the current work_status \"{}\"",
            last.to_status, state.work_status
        )));
    }
    if last.scope_revision != state.scope_revision {
        problems.push(fail(format!(
            "execution run \"{id}\" last transition scope_revision {} does not match the current scope_revision {}",
            last.scope_revision, state.scope_revision
        )));
    }
}

/// A backward move needs a rationale; a forward jump over a stage needs an
/// explicit, non-blank `skipped_stages` entry for every stage it passes.
fn check_stage_move(
    id: &str,
    at: &str,
    t: &Transition,
    from: LifecycleStage,
    problems: &mut Vec<Diagnostic>,
) {
    let to = t.to_stage;
    if to < from
        && t.backward_rationale
            .as_ref()
            .is_none_or(RecordText::is_blank)
    {
        problems.push(fail(format!(
            "execution run \"{id}\" {at} moves back from \"{from}\" to \"{to}\" with no backward_rationale; a backward lifecycle move is not accepted silently"
        )));
    }
    for skipped in LifecycleStage::between(from, to) {
        let covered = t
            .skipped_stages
            .iter()
            .any(|s| s.stage == *skipped && !s.rationale.is_blank());
        if !covered {
            problems.push(fail(format!(
                "execution run \"{id}\" {at} jumps over \"{skipped}\" with no explicit inapplicability basis; a skipped stage is stated, not silently treated as passed"
            )));
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::run_contracts::envelope::tests::envelope;
    use crate::types::WorkspaceId;

    fn text(s: &str) -> RecordText {
        RecordText::new(s).unwrap()
    }

    fn transition(
        seq: u64,
        from: Option<(LifecycleStage, WorkStatus)>,
        to: (LifecycleStage, WorkStatus),
    ) -> Transition {
        Transition {
            sequence: TransitionSequence::new(seq).unwrap(),
            from_stage: from.map(|f| f.0),
            to_stage: to.0,
            from_status: from.map(|f| f.1),
            to_status: to.1,
            scope_revision: ScopeRevision::new(1).unwrap(),
            reason: text("Причина перехода."),
            skipped_stages: Vec::new(),
            backward_rationale: None,
            reopen_rationale: None,
        }
    }

    pub(crate) fn valid_input() -> ExecutionRunInput {
        use LifecycleStage::*;
        use WorkStatus::*;
        ExecutionRunInput {
            envelope: envelope(RecordFamily::ExecutionRun),
            scope: RunStateScope::new(
                super::super::identity::RunId::new("example-run").unwrap(),
                WorkspaceId::new("example-workspace").unwrap(),
            ),
            state: ExecutionRunState {
                task_specification_ref: PortableRef::new("records/task-specification/x").unwrap(),
                lifecycle_stage: Classification,
                work_status: Active,
                scope_revision: ScopeRevision::new(1).unwrap(),
                current_actor: ActorRef::new("executor-a").unwrap(),
                resolved_norms: vec![PortableRef::new("standards/workspace/a.md").unwrap()],
                completed_checks: Vec::new(),
                blockers: Vec::new(),
                next_action: Some(text("classify")),
                next_gate: None,
                transition_history: vec![
                    transition(1, None, (Intake, Planned)),
                    transition(2, Some((Intake, Planned)), (Classification, Active)),
                ],
            },
        }
    }

    fn messages(input: ExecutionRunInput) -> (bool, Vec<String>) {
        let (accepted, problems) = check_execution_run(input);
        (
            accepted.is_some(),
            problems.iter().map(|d| d.message().to_string()).collect(),
        )
    }

    #[test]
    fn a_consistent_run_is_accepted_with_its_full_state() {
        let (accepted, problems) = check_execution_run(valid_input());
        assert!(problems.is_empty(), "{problems:?}");
        let run = accepted.unwrap();
        assert_eq!(run.state().work_status, WorkStatus::Active);
        assert_eq!(run.scope().run_id().as_str(), "example-run");
        assert_eq!(run.state().transition_history.len(), 2);
    }

    #[test]
    fn blockers_must_agree_with_blocked_status() {
        let mut input = valid_input();
        input.state.work_status = WorkStatus::Blocked;
        input.state.transition_history[1].to_status = WorkStatus::Blocked;
        let (accepted, m) = messages(input);
        assert!(!accepted);
        assert_eq!(m.len(), 1, "{m:?}");
        assert!(m[0].contains("work_status is \"blocked\" with no blocker"));
    }

    #[test]
    fn a_silent_skip_and_history_break_and_current_mismatch_are_all_reported_in_order() {
        use LifecycleStage::*;
        let mut input = valid_input();
        input.state.transition_history[1].from_stage = Some(Classification);
        input.state.transition_history[1].to_stage = Execution;
        input.state.transition_history[1].sequence = TransitionSequence::new(1).unwrap();
        let (accepted, m) = messages(input);
        assert!(!accepted);
        assert_eq!(
            m,
            [
                "execution run \"example-record\" transition #1 repeats sequence ordinal 1; ordinals do not repeat",
                "execution run \"example-record\" transition #1 from_stage \"classification\" does not continue the previous to_stage \"intake\"; the history has no break",
                "execution run \"example-record\" transition #1 jumps over \"norm_resolution\" with no explicit inapplicability basis; a skipped stage is stated, not silently treated as passed",
                "execution run \"example-record\" transition #1 jumps over \"planning\" with no explicit inapplicability basis; a skipped stage is stated, not silently treated as passed",
                "execution run \"example-record\" last transition to_stage \"execution\" does not match the current lifecycle_stage \"classification\"",
            ]
        );
    }

    #[test]
    fn terminal_runs_carry_no_next_step_and_reopen_needs_a_rationale() {
        use LifecycleStage::*;
        use WorkStatus::*;
        let mut input = valid_input();
        input.state.work_status = Completed;
        input.state.lifecycle_stage = Classification;
        input.state.transition_history[1].to_status = Completed;
        let mut third = transition(
            3,
            Some((Classification, Completed)),
            (Classification, Completed),
        );
        third.reopen_rationale = Some(text("  "));
        input.state.transition_history.push(third);
        let (_, m) = messages(input);
        assert!(
            m[0].contains("but next_action carries an executable value \"classify\""),
            "{m:?}"
        );
        assert!(
            m[1].contains(
                "transition #2 follows a completed or cancelled state with no reopen_rationale"
            ),
            "{m:?}"
        );
    }

    #[test]
    fn duplicate_references_are_compared_trimmed() {
        let mut input = valid_input();
        input.state.completed_checks = vec![
            PortableRef::new("check-a").unwrap(),
            PortableRef::new(" check-a ").unwrap(),
        ];
        let (_, m) = messages(input);
        assert_eq!(
            m,
            ["execution run \"example-record\" completed_checks repeats reference \" check-a \""]
        );
    }
}
