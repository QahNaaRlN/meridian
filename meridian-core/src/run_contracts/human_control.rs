//! `run-human-control`: the human-control state of ONE execution run
//! (`registries/operating-model/human-control.schema.json`).
//!
//! Role, supervision mode, human authority, the acting participant and the
//! communication mode are independent axes. Human-in-command is the
//! permanent basis, not a supervision mode: the schema fixes its posture,
//! so only its holder is a field. The schema's mutually exclusive
//! `hitl`/`hotl` blocks and the `review_independence.required`
//! conditionals are enum forms ([`Supervision`], [`ReviewIndependence`]);
//! [`RoleAssignment::new`] rejects an empty or repeated role list the
//! schema also rejects. [`check_human_control`] enforces what remains —
//! blank and non-portable text, owner-held authority, assignment
//! membership, review independence and the unbroken switch history — and
//! is the ONLY way to obtain an accepted [`HumanControl`].

use std::collections::HashMap;
use std::fmt;

use crate::task_contracts::non_portable_reason;
use crate::types::Diagnostic;

use super::envelope::{check_envelope, RecordEnvelope, RecordFamily};
use super::identity::{ActorRef, PortableRef, RecordText, RunStateScope, TransitionSequence};
use super::vocabulary::{CommunicationMode, SupervisionMode, UniversalRole};
use super::{fail, option_text};

/// A value rejected by [`RoleAssignment::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoleAssignmentError {
    NoRoles,
    RepeatedRole(UniversalRole),
}

impl fmt::Display for RoleAssignmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RoleAssignmentError::NoRoles => write!(f, "a role assignment lists no role"),
            RoleAssignmentError::RepeatedRole(r) => {
                write!(f, "a role assignment repeats role \"{r}\"")
            }
        }
    }
}

impl std::error::Error for RoleAssignmentError {}

/// One participant and the non-empty, repetition-free set of roles it
/// holds in this run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleAssignment {
    actor: ActorRef,
    roles: Vec<UniversalRole>,
}

impl RoleAssignment {
    pub fn new(actor: ActorRef, roles: Vec<UniversalRole>) -> Result<Self, RoleAssignmentError> {
        if roles.is_empty() {
            return Err(RoleAssignmentError::NoRoles);
        }
        for (i, role) in roles.iter().enumerate() {
            if roles[..i].contains(role) {
                return Err(RoleAssignmentError::RepeatedRole(*role));
            }
        }
        Ok(Self { actor, roles })
    }

    pub fn actor(&self) -> &ActorRef {
        &self.actor
    }

    pub fn roles(&self) -> &[UniversalRole] {
        &self.roles
    }
}

/// The switchable supervision mode together with the block the schema
/// requires for it (and forbids for the other mode).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Supervision {
    HumanInTheLoop {
        required_human_action: RecordText,
        gate: RecordText,
    },
    HumanOnTheLoop {
        autonomy_bounds: Vec<RecordText>,
        intervention: RecordText,
    },
}

impl Supervision {
    pub fn mode(&self) -> SupervisionMode {
        match self {
            Supervision::HumanInTheLoop { .. } => SupervisionMode::HumanInTheLoop,
            Supervision::HumanOnTheLoop { .. } => SupervisionMode::HumanOnTheLoop,
        }
    }
}

/// The optional independent-review requirement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewIndependence {
    Required {
        reviewer_actor: ActorRef,
        reviewed_actor: ActorRef,
    },
    NotRequired,
}

/// One entry of the ordered switch history; `from_*` are `None` for the
/// schema's explicit `null`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlSwitch {
    pub sequence: TransitionSequence,
    pub from_supervision_mode: Option<SupervisionMode>,
    pub to_supervision_mode: SupervisionMode,
    pub from_communication_mode: Option<CommunicationMode>,
    pub to_communication_mode: CommunicationMode,
    pub from_acting_actor: Option<ActorRef>,
    pub to_acting_actor: ActorRef,
    pub from_role_assignments: Option<Vec<RoleAssignment>>,
    pub to_role_assignments: Vec<RoleAssignment>,
    pub checkpoint_ref: RecordText,
    pub reason: RecordText,
}

/// The run's current control axes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanControlState {
    pub execution_run_ref: PortableRef,
    /// The participant holding the permanent human-in-command posture.
    pub authority_holder: ActorRef,
    pub supervision: Supervision,
    pub acting_actor: ActorRef,
    pub role_assignments: Vec<RoleAssignment>,
    pub review_independence: Option<ReviewIndependence>,
    pub communication_mode: CommunicationMode,
    pub switch_history: Vec<ControlSwitch>,
}

/// One schema-clean run-human-control record, not yet domain-checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanControlInput {
    pub envelope: RecordEnvelope,
    pub scope: RunStateScope,
    pub control: HumanControlState,
}

/// A human-control record with zero domain problems.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanControl(HumanControlInput);

impl HumanControl {
    pub fn envelope(&self) -> &RecordEnvelope {
        &self.0.envelope
    }

    pub fn scope(&self) -> &RunStateScope {
        &self.0.scope
    }

    pub fn control(&self) -> &HumanControlState {
        &self.0.control
    }
}

/// Which actors one assignment set names, and the roles each holds.
struct AssignmentIndex<'a> {
    roles_by_actor: HashMap<&'a str, Vec<UniversalRole>>,
}

impl AssignmentIndex<'_> {
    fn is_assigned(&self, actor: &str) -> bool {
        self.roles_by_actor.contains_key(actor)
    }

    fn holds(&self, actor: &str, role: UniversalRole) -> bool {
        self.roles_by_actor
            .get(actor)
            .is_some_and(|roles| roles.contains(&role))
    }
}

/// Validates one assignment set. An actor named twice is reported, and its
/// second assignment's roles are merged into the first's, so a role held
/// in both is also reported as repeated (the Node reference's behaviour).
fn check_assignment_set<'a>(
    id: &str,
    label: &str,
    assignments: &'a [RoleAssignment],
    problems: &mut Vec<Diagnostic>,
) -> AssignmentIndex<'a> {
    let mut index = AssignmentIndex {
        roles_by_actor: HashMap::new(),
    };
    if assignments.is_empty() {
        problems.push(fail(format!(
            "human-control record \"{id}\" {label} carries no role assignments; a run has at least one assigned participant"
        )));
        return index;
    }
    for (i, assignment) in assignments.iter().enumerate() {
        let actor = assignment.actor.as_str();
        let named = !assignment.actor.is_blank();
        let at = if named {
            format!("\"{actor}\"")
        } else {
            format!("#{i}")
        };
        if named {
            if let Some(r) = non_portable_reason(Some(actor)) {
                problems.push(fail(format!(
                    "human-control record \"{id}\" {label} assignment {at} actor contains {r}"
                )));
            }
            if index.is_assigned(actor) {
                problems.push(fail(format!(
                    "human-control record \"{id}\" {label} assigns actor \"{actor}\" more than once; combine an actor's roles in one assignment"
                )));
            }
        } else {
            problems.push(fail(format!(
                "human-control record \"{id}\" {label} assignment {at} names no actor; an empty or ambiguous assignment is rejected"
            )));
        }
        let mut held = if named {
            index.roles_by_actor.get(actor).cloned().unwrap_or_default()
        } else {
            Vec::new()
        };
        for role in &assignment.roles {
            if held.contains(role) {
                problems.push(fail(format!(
                    "human-control record \"{id}\" {label} assignment {at} assigns role \"{role}\" more than once"
                )));
            } else {
                held.push(*role);
            }
        }
        if named {
            index.roles_by_actor.insert(actor, held);
        }
    }
    index
}

/// Every human-control rule, in the Node reference's diagnostic order.
pub fn check_human_control(input: HumanControlInput) -> (Option<HumanControl>, Vec<Diagnostic>) {
    let mut problems = Vec::new();
    check_envelope(RecordFamily::HumanControl, &input.envelope, &mut problems);
    let id = input.envelope.id.as_str();
    let control = &input.control;

    let run_ref = &control.execution_run_ref;
    if run_ref.is_blank() {
        problems.push(fail(format!(
            "human-control record \"{id}\" execution_run_ref is empty or whitespace-only"
        )));
    } else if let Some(r) = non_portable_reason(Some(run_ref.as_str())) {
        problems.push(fail(format!(
            "human-control record \"{id}\" execution_run_ref contains {r}; the reference is portable and is not an absolute machine path"
        )));
    }

    let current = check_assignment_set(id, "current", &control.role_assignments, &mut problems);

    let holder = control.authority_holder.as_str();
    if let Some(r) = non_portable_reason(Some(holder)) {
        problems.push(fail(format!(
            "human-control record \"{id}\" human_authority.holder contains {r}"
        )));
    }
    if !current.holds(holder, UniversalRole::HUMAN_AUTHORITY) {
        problems.push(fail(format!(
            "human-control record \"{id}\" human_authority.holder \"{holder}\" is not a participant assigned the {} role; human-in-command is held by an assigned owner",
            UniversalRole::HUMAN_AUTHORITY
        )));
    }

    check_supervision(id, &control.supervision, &mut problems);

    let acting = &control.acting_actor;
    if acting.is_blank() {
        problems.push(fail(format!(
            "human-control record \"{id}\" names no acting_actor"
        )));
    } else {
        if let Some(r) = non_portable_reason(Some(acting.as_str())) {
            problems.push(fail(format!(
                "human-control record \"{id}\" acting_actor contains {r}"
            )));
        }
        if !current.is_assigned(acting.as_str()) {
            problems.push(fail(format!(
                "human-control record \"{id}\" acting_actor \"{acting}\" is not assigned in this run; any participant in control must be present in role_assignments"
            )));
        }
    }

    if let Some(ReviewIndependence::Required {
        reviewer_actor,
        reviewed_actor,
    }) = &control.review_independence
    {
        check_review_independence(id, reviewer_actor, reviewed_actor, &current, &mut problems);
    }

    check_switch_history(id, control, &mut problems);

    let accepted = problems.is_empty().then_some(HumanControl(input));
    (accepted, problems)
}

fn check_supervision(id: &str, supervision: &Supervision, problems: &mut Vec<Diagnostic>) {
    match supervision {
        Supervision::HumanInTheLoop {
            required_human_action,
            gate,
        } => {
            if required_human_action.is_blank() || gate.is_blank() {
                problems.push(fail(format!(
                    "human-control record \"{id}\" is human-in-the-loop but names no concrete required human action and declared gate; HITL means a specific human action on a stated gate is known"
                )));
            }
            for text in [required_human_action, gate] {
                if let Some(r) = non_portable_reason(Some(text.as_str())) {
                    problems.push(fail(format!(
                        "human-control record \"{id}\" hitl block contains {r}"
                    )));
                }
            }
        }
        Supervision::HumanOnTheLoop {
            autonomy_bounds,
            intervention,
        } => {
            if autonomy_bounds.is_empty() || intervention.is_blank() {
                problems.push(fail(format!(
                    "human-control record \"{id}\" is human-on-the-loop but states no autonomy bounds or intervention capability; HOTL means bounded autonomy inside pre-set bounds with a standing way to intervene"
                )));
            }
            for text in autonomy_bounds.iter().chain(std::iter::once(intervention)) {
                if let Some(r) = non_portable_reason(Some(text.as_str())) {
                    problems.push(fail(format!(
                        "human-control record \"{id}\" hotl block contains {r}"
                    )));
                }
            }
        }
    }
}

fn check_review_independence(
    id: &str,
    reviewer: &ActorRef,
    reviewed: &ActorRef,
    current: &AssignmentIndex<'_>,
    problems: &mut Vec<Diagnostic>,
) {
    for (actor, name) in [(reviewer, "reviewer_actor"), (reviewed, "reviewed_actor")] {
        if let Some(r) = non_portable_reason(Some(actor.as_str())) {
            problems.push(fail(format!(
                "human-control record \"{id}\" review_independence.{name} contains {r}"
            )));
        }
    }
    if reviewer.is_blank() || reviewed.is_blank() {
        problems.push(fail(format!(
            "human-control record \"{id}\" requires review independence but does not name both a reviewer_actor and the reviewed_actor"
        )));
        return;
    }
    if reviewer == reviewed {
        problems.push(fail(format!(
            "human-control record \"{id}\" requires review independence but names the same actor \"{reviewer}\" as reviewer and reviewed; an independent reviewer cannot be the participant whose result is independently reviewed"
        )));
    }
    let suitable = UniversalRole::INDEPENDENT_REVIEW
        .iter()
        .any(|role| current.holds(reviewer.as_str(), *role));
    if !suitable {
        problems.push(fail(format!(
            "human-control record \"{id}\" requires review independence but no separate actor is assigned a reviewing role ({}); the requirement needs a suitable independent participant",
            UniversalRole::INDEPENDENT_REVIEW
                .iter()
                .map(|r| r.as_str())
                .collect::<Vec<_>>()
                .join(" or ")
        )));
    }
    if !current.is_assigned(reviewed.as_str()) {
        problems.push(fail(format!(
            "human-control record \"{id}\" review_independence.reviewed_actor \"{reviewed}\" is not assigned in this run"
        )));
    }
}

fn check_switch_history(id: &str, control: &HumanControlState, problems: &mut Vec<Diagnostic>) {
    let history = &control.switch_history;
    let Some(last) = history.last() else {
        problems.push(fail(format!(
            "human-control record \"{id}\" switch_history is empty; the history is mandatory and its first record states the initial establishment"
        )));
        return;
    };

    let mut prev: Option<&ControlSwitch> = None;
    for (i, s) in history.iter().enumerate() {
        let at = format!("switch #{i}");
        if s.reason.is_blank() {
            problems.push(fail(format!(
                "human-control record \"{id}\" {at} states no reason; every change states its basis"
            )));
        }
        if s.checkpoint_ref.is_blank() {
            problems.push(fail(format!(
                "human-control record \"{id}\" {at} names no checkpoint_ref; a change preserves a verifiable checkpoint and never opens a new run"
            )));
        }
        for (text, name) in [
            (s.reason.as_str(), "reason"),
            (s.checkpoint_ref.as_str(), "checkpoint_ref"),
            (s.to_acting_actor.as_str(), "to_acting_actor"),
        ] {
            if let Some(r) = non_portable_reason(Some(text)) {
                problems.push(fail(format!(
                    "human-control record \"{id}\" {at} {name} contains {r}"
                )));
            }
        }
        if s.to_acting_actor.is_blank() {
            problems.push(fail(format!(
                "human-control record \"{id}\" {at} names no to_acting_actor"
            )));
        }

        let to = check_assignment_set(
            id,
            &format!("{at} to_role_assignments"),
            &s.to_role_assignments,
            problems,
        );
        if !s.to_acting_actor.is_blank() && !to.is_assigned(s.to_acting_actor.as_str()) {
            problems.push(fail(format!(
                "human-control record \"{id}\" {at} moves to acting actor \"{}\", who is not present in that transition's role assignments",
                s.to_acting_actor
            )));
        }

        match prev {
            None => {
                let priors: Vec<&str> = [
                    ("from_supervision_mode", s.from_supervision_mode.is_some()),
                    (
                        "from_communication_mode",
                        s.from_communication_mode.is_some(),
                    ),
                    ("from_acting_actor", s.from_acting_actor.is_some()),
                    ("from_role_assignments", s.from_role_assignments.is_some()),
                ]
                .into_iter()
                .filter_map(|(name, present)| present.then_some(name))
                .collect();
                if !priors.is_empty() {
                    problems.push(fail(format!(
                        "human-control record \"{id}\" {at} is the first record but names a prior {}; the first record states the initial establishment (every from_* axis is null)",
                        priors.join(", ")
                    )));
                }
            }
            Some(p) => {
                if s.sequence == p.sequence {
                    problems.push(fail(format!(
                        "human-control record \"{id}\" {at} repeats sequence ordinal {}; ordinals do not repeat",
                        s.sequence
                    )));
                } else if s.sequence < p.sequence {
                    problems.push(fail(format!(
                        "human-control record \"{id}\" {at} sequence ordinal {} is below the previous {}; ordinals do not decrease",
                        s.sequence, p.sequence
                    )));
                }
                check_continuity(id, &at, s, p, problems);
            }
        }
        prev = Some(s);
    }

    if last.to_supervision_mode != control.supervision.mode() {
        problems.push(fail(format!(
            "human-control record \"{id}\" last switch to_supervision_mode \"{}\" does not match the current supervision_mode \"{}\"",
            last.to_supervision_mode,
            control.supervision.mode()
        )));
    }
    if last.to_communication_mode != control.communication_mode {
        problems.push(fail(format!(
            "human-control record \"{id}\" last switch to_communication_mode \"{}\" does not match the current communication_mode \"{}\"",
            last.to_communication_mode, control.communication_mode
        )));
    }
    if last.to_acting_actor != control.acting_actor {
        problems.push(fail(format!(
            "human-control record \"{id}\" last switch to_acting_actor \"{}\" does not match the current acting_actor \"{}\"",
            last.to_acting_actor, control.acting_actor
        )));
    }
    if last.to_role_assignments != control.role_assignments {
        problems.push(fail(format!(
            "human-control record \"{id}\" last switch to_role_assignments does not match the current role_assignments"
        )));
    }
}

/// Sequence ordering is checked by the caller BEFORE this — the Node
/// reference reports it ahead of the axis continuity.
fn check_continuity(
    id: &str,
    at: &str,
    s: &ControlSwitch,
    p: &ControlSwitch,
    problems: &mut Vec<Diagnostic>,
) {
    if s.from_supervision_mode != Some(p.to_supervision_mode) {
        problems.push(fail(format!(
            "human-control record \"{id}\" {at} from_supervision_mode \"{}\" does not continue the previous to_supervision_mode \"{}\"; the switch history has a break on the supervision-mode axis",
            option_text(s.from_supervision_mode),
            p.to_supervision_mode
        )));
    }
    if s.from_communication_mode != Some(p.to_communication_mode) {
        problems.push(fail(format!(
            "human-control record \"{id}\" {at} from_communication_mode \"{}\" does not continue the previous to_communication_mode \"{}\"; the switch history has a break on the communication-mode axis",
            option_text(s.from_communication_mode),
            p.to_communication_mode
        )));
    }
    if s.from_acting_actor.as_ref() != Some(&p.to_acting_actor) {
        problems.push(fail(format!(
            "human-control record \"{id}\" {at} from_acting_actor \"{}\" does not continue the previous to_acting_actor \"{}\"; the switch history has a break on the acting-actor axis",
            option_text(s.from_acting_actor.as_ref()),
            p.to_acting_actor
        )));
    }
    if s.from_role_assignments.as_ref() != Some(&p.to_role_assignments) {
        problems.push(fail(format!(
            "human-control record \"{id}\" {at} from_role_assignments does not continue the previous to_role_assignments; the switch history has a break on the role-assignment axis"
        )));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_contracts::envelope::tests::envelope;
    use crate::run_contracts::identity::RunId;
    use crate::types::WorkspaceId;

    fn actor_ref(s: &str) -> ActorRef {
        ActorRef::new(s).unwrap()
    }

    fn text(s: &str) -> RecordText {
        RecordText::new(s).unwrap()
    }

    fn assign(actor: &str, roles: &[UniversalRole]) -> RoleAssignment {
        RoleAssignment::new(actor_ref(actor), roles.to_vec()).unwrap()
    }

    fn assignments() -> Vec<RoleAssignment> {
        vec![
            assign("owner-a", &[UniversalRole::Owner]),
            assign("executor-b", &[UniversalRole::Executor]),
            assign("reviewer-c", &[UniversalRole::Reviewer]),
        ]
    }

    fn switch(seq: u64, first: bool) -> ControlSwitch {
        ControlSwitch {
            sequence: TransitionSequence::new(seq).unwrap(),
            from_supervision_mode: (!first).then_some(SupervisionMode::HumanInTheLoop),
            to_supervision_mode: SupervisionMode::HumanInTheLoop,
            from_communication_mode: (!first).then_some(CommunicationMode::OwnerRelayed),
            to_communication_mode: CommunicationMode::OwnerRelayed,
            from_acting_actor: (!first).then(|| actor_ref("executor-b")),
            to_acting_actor: actor_ref("executor-b"),
            from_role_assignments: (!first).then(assignments),
            to_role_assignments: assignments(),
            checkpoint_ref: text("checkpoint-1"),
            reason: text("Начало."),
        }
    }

    fn valid_input() -> HumanControlInput {
        HumanControlInput {
            envelope: envelope(RecordFamily::HumanControl),
            scope: RunStateScope::new(
                RunId::new("example-run").unwrap(),
                WorkspaceId::new("example-workspace").unwrap(),
            ),
            control: HumanControlState {
                execution_run_ref: PortableRef::new("records/execution-run/example-run").unwrap(),
                authority_holder: actor_ref("owner-a"),
                supervision: Supervision::HumanInTheLoop {
                    required_human_action: text("approve"),
                    gate: text("acceptance"),
                },
                acting_actor: actor_ref("executor-b"),
                role_assignments: assignments(),
                review_independence: Some(ReviewIndependence::Required {
                    reviewer_actor: actor_ref("reviewer-c"),
                    reviewed_actor: actor_ref("executor-b"),
                }),
                communication_mode: CommunicationMode::OwnerRelayed,
                switch_history: vec![switch(1, true), switch(2, false)],
            },
        }
    }

    fn messages(input: HumanControlInput) -> (bool, Vec<String>) {
        let (accepted, problems) = check_human_control(input);
        (
            accepted.is_some(),
            problems.iter().map(|d| d.message().to_string()).collect(),
        )
    }

    #[test]
    fn role_assignment_rejects_empty_and_repeated_roles() {
        assert_eq!(
            RoleAssignment::new(actor_ref("a"), vec![]),
            Err(RoleAssignmentError::NoRoles)
        );
        assert_eq!(
            RoleAssignment::new(
                actor_ref("a"),
                vec![UniversalRole::Owner, UniversalRole::Owner]
            ),
            Err(RoleAssignmentError::RepeatedRole(UniversalRole::Owner))
        );
    }

    #[test]
    fn a_consistent_control_record_is_accepted() {
        let (accepted, problems) = check_human_control(valid_input());
        assert!(problems.is_empty(), "{problems:?}");
        let control = accepted.unwrap();
        assert_eq!(
            control.control().supervision.mode(),
            SupervisionMode::HumanInTheLoop
        );
    }

    #[test]
    fn authority_actor_and_review_rules_are_reported_in_order() {
        let mut input = valid_input();
        input.control.authority_holder = actor_ref("executor-b");
        input.control.acting_actor = actor_ref("stranger");
        input.control.review_independence = Some(ReviewIndependence::Required {
            reviewer_actor: actor_ref("executor-b"),
            reviewed_actor: actor_ref("executor-b"),
        });
        let (accepted, m) = messages(input);
        assert!(!accepted);
        assert_eq!(
            m,
            [
                "human-control record \"example-record\" human_authority.holder \"executor-b\" is not a participant assigned the owner role; human-in-command is held by an assigned owner",
                "human-control record \"example-record\" acting_actor \"stranger\" is not assigned in this run; any participant in control must be present in role_assignments",
                "human-control record \"example-record\" requires review independence but names the same actor \"executor-b\" as reviewer and reviewed; an independent reviewer cannot be the participant whose result is independently reviewed",
                "human-control record \"example-record\" requires review independence but no separate actor is assigned a reviewing role (reviewer or verifier); the requirement needs a suitable independent participant",
                "human-control record \"example-record\" last switch to_acting_actor \"executor-b\" does not match the current acting_actor \"stranger\"",
            ]
        );
    }

    /// An actor named twice merges its roles, so a role in both is also
    /// reported as repeated (the Node reference's behaviour).
    #[test]
    fn a_repeated_actor_merges_roles_and_reports_a_repeated_role() {
        let mut input = valid_input();
        input
            .control
            .role_assignments
            .push(assign("executor-b", &[UniversalRole::Executor]));
        let (_, m) = messages(input);
        assert_eq!(
            &m[..2],
            [
                "human-control record \"example-record\" current assigns actor \"executor-b\" more than once; combine an actor's roles in one assignment",
                "human-control record \"example-record\" current assignment \"executor-b\" assigns role \"executor\" more than once",
            ]
        );
    }

    #[test]
    fn switch_history_breaks_and_first_record_priors_are_reported() {
        let mut input = valid_input();
        input.control.switch_history[0] = switch(1, false);
        input.control.switch_history[1].from_communication_mode = Some(CommunicationMode::Direct);
        input.control.switch_history[1].sequence = TransitionSequence::new(1).unwrap();
        let (_, m) = messages(input);
        assert_eq!(
            m,
            [
                "human-control record \"example-record\" switch #0 is the first record but names a prior from_supervision_mode, from_communication_mode, from_acting_actor, from_role_assignments; the first record states the initial establishment (every from_* axis is null)",
                "human-control record \"example-record\" switch #1 repeats sequence ordinal 1; ordinals do not repeat",
                "human-control record \"example-record\" switch #1 from_communication_mode \"direct\" does not continue the previous to_communication_mode \"owner_relayed\"; the switch history has a break on the communication-mode axis",
            ]
        );
    }

    #[test]
    fn blank_hotl_intervention_is_a_domain_rejection() {
        let mut input = valid_input();
        input.control.supervision = Supervision::HumanOnTheLoop {
            autonomy_bounds: vec![text("only-tests")],
            intervention: text(" "),
        };
        let (accepted, m) = messages(input);
        assert!(!accepted);
        assert!(
            m[0].contains(
                "is human-on-the-loop but states no autonomy bounds or intervention capability"
            ),
            "{m:?}"
        );
    }
}
