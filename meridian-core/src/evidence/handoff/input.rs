//! The typed input of one evidence-and-handoff record: every value the
//! schema already constrained is typed (closed pools are enums, ids are
//! [`SemanticId`], pins are [`PinSha256`]), and every value only the domain
//! can judge — blank or non-portable text, cross-references, statuses that
//! must agree with the evidence — is carried as stated, for
//! [`super::check_handoff`] to accept or reject.

use crate::run_contracts::{
    PinSha256, PinnedRecordKind, PinnedRef, PortableRef, RecordEnvelope, RecordText, RunStateScope,
};
use crate::types::SemanticId;

use crate::evidence::linkage::{AssertionStatus, ClaimedResultStatus};
use crate::evidence::types::PinnedEvidence;

/// The run-bound pinned-reference slots of a handoff, in the fixed order
/// every walk uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HandoffSlot {
    ExecutionRun,
    TaskSpecification,
    HumanControl,
    ContextManifest,
}

impl HandoffSlot {
    pub const ALL: [HandoffSlot; 4] = [
        HandoffSlot::ExecutionRun,
        HandoffSlot::TaskSpecification,
        HandoffSlot::HumanControl,
        HandoffSlot::ContextManifest,
    ];

    /// The payload field name of this slot.
    pub const fn field(self) -> &'static str {
        match self {
            HandoffSlot::ExecutionRun => "execution_run_ref",
            HandoffSlot::TaskSpecification => "task_specification_ref",
            HandoffSlot::HumanControl => "human_control_ref",
            HandoffSlot::ContextManifest => "context_manifest_ref",
        }
    }

    /// The record kind this slot must name.
    pub const fn kind(self) -> PinnedRecordKind {
        match self {
            HandoffSlot::ExecutionRun => PinnedRecordKind::ExecutionRun,
            HandoffSlot::TaskSpecification => PinnedRecordKind::TaskSpecification,
            HandoffSlot::HumanControl => PinnedRecordKind::RunHumanControl,
            HandoffSlot::ContextManifest => PinnedRecordKind::ContextManifest,
        }
    }
}

/// The four structured pinned references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandoffPins {
    pub execution_run: PinnedRef,
    pub task_specification: PinnedRef,
    pub human_control: PinnedRef,
    pub context_manifest: PinnedRef,
}

impl HandoffPins {
    pub fn get(&self, slot: HandoffSlot) -> &PinnedRef {
        match slot {
            HandoffSlot::ExecutionRun => &self.execution_run,
            HandoffSlot::TaskSpecification => &self.task_specification,
            HandoffSlot::HumanControl => &self.human_control,
            HandoffSlot::ContextManifest => &self.context_manifest,
        }
    }
}

/// The closed outcome pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutcomeStatus {
    Complete,
    Blocked,
    HandedOffIncomplete,
}

impl OutcomeStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            OutcomeStatus::Complete => "complete",
            OutcomeStatus::Blocked => "blocked",
            OutcomeStatus::HandedOffIncomplete => "handed_off_incomplete",
        }
    }

    pub fn parse(value: &str) -> Option<OutcomeStatus> {
        match value {
            "complete" => Some(OutcomeStatus::Complete),
            "blocked" => Some(OutcomeStatus::Blocked),
            "handed_off_incomplete" => Some(OutcomeStatus::HandedOffIncomplete),
            _ => None,
        }
    }
}

/// The overall outcome as the record states it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutcomeInput {
    pub statement: RecordText,
    pub status: OutcomeStatus,
}

/// One claimed result with its STATED status (checked against the
/// computed one).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedResultInput {
    pub id: SemanticId,
    pub statement: RecordText,
    pub status: ClaimedResultStatus,
}

/// One verifiable assertion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssertionInput {
    pub id: SemanticId,
    pub statement: RecordText,
    pub claimed_result_id: SemanticId,
    pub status: AssertionStatus,
    pub unverified_reason: Option<RecordText>,
}

/// One piece of handoff evidence: the shared pinned evidence plus the
/// assertions it claims to cover, its limitations and — for a
/// `specialised-evidence-record` — the specialised contract and verdict it
/// records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandoffEvidenceInput {
    pub evidence: PinnedEvidence,
    pub covers: Vec<SemanticId>,
    pub limitations: Vec<RecordText>,
    pub specialised_contract: Option<RecordText>,
    pub recorded_verdict: Option<RecordText>,
}

/// The closed mandatory-check pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CheckStatus {
    Passed,
    Failed,
    Unable,
}

impl CheckStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            CheckStatus::Passed => "passed",
            CheckStatus::Failed => "failed",
            CheckStatus::Unable => "unable",
        }
    }

    pub fn parse(value: &str) -> Option<CheckStatus> {
        match value {
            "passed" => Some(CheckStatus::Passed),
            "failed" => Some(CheckStatus::Failed),
            "unable" => Some(CheckStatus::Unable),
            _ => None,
        }
    }

    /// A passed or failed check ran; an unable one did not.
    pub const fn ran(self) -> bool {
        matches!(self, CheckStatus::Passed | CheckStatus::Failed)
    }
}

/// One mandatory check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MandatoryCheckInput {
    pub id: SemanticId,
    pub name: RecordText,
    pub check_ref: PortableRef,
    pub status: CheckStatus,
    pub result_evidence_id: Option<SemanticId>,
    pub reason: Option<RecordText>,
}

/// The closed coverage pool of an acceptance criterion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoverageStatus {
    Covered,
    Uncovered,
}

impl CoverageStatus {
    pub fn parse(value: &str) -> Option<CoverageStatus> {
        match value {
            "covered" => Some(CoverageStatus::Covered),
            "uncovered" => Some(CoverageStatus::Uncovered),
            _ => None,
        }
    }
}

/// One acceptance criterion's coverage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CriterionInput {
    pub id: SemanticId,
    pub status: CoverageStatus,
    pub addressed_by: Vec<SemanticId>,
    pub uncovered_reason: Option<RecordText>,
}

/// One repository's pinned state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryStateInput {
    pub repository_ref: PortableRef,
    pub revision: Option<RecordText>,
    pub sha256: Option<PinSha256>,
}

/// The closed change-kind pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChangeKind {
    Added,
    Modified,
    Removed,
    Renamed,
}

impl ChangeKind {
    pub fn parse(value: &str) -> Option<ChangeKind> {
        match value {
            "added" => Some(ChangeKind::Added),
            "modified" => Some(ChangeKind::Modified),
            "removed" => Some(ChangeKind::Removed),
            "renamed" => Some(ChangeKind::Renamed),
            _ => None,
        }
    }
}

/// One changed path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedPathInput {
    pub id: SemanticId,
    pub repository_ref: PortableRef,
    pub path: PortableRef,
    pub change_kind: ChangeKind,
}

/// One external effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalEffectInput {
    pub id: SemanticId,
    pub description: RecordText,
    pub reversible: bool,
}

/// The closed deviation-severity pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviationSeverity {
    Blocking,
    NonBlocking,
}

impl DeviationSeverity {
    pub fn parse(value: &str) -> Option<DeviationSeverity> {
        match value {
            "blocking" => Some(DeviationSeverity::Blocking),
            "non_blocking" => Some(DeviationSeverity::NonBlocking),
            _ => None,
        }
    }
}

/// One deviation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviationInput {
    pub id: SemanticId,
    pub description: RecordText,
    pub severity: DeviationSeverity,
    pub disposition: RecordText,
}

/// One acknowledged open gap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenGapInput {
    pub id: SemanticId,
    pub description: RecordText,
}

/// One decision the owner still has to take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerDecisionInput {
    pub id: SemanticId,
    pub question: RecordText,
    pub blocking: bool,
}

/// One blocker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockerInput {
    pub id: SemanticId,
    pub description: RecordText,
    pub resumption_condition: RecordText,
}

/// The closed worktree-disposition pool (`version-control-flow.md` §5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorktreeState {
    NotCreated,
    Removed,
    Retained,
}

impl WorktreeState {
    pub const fn as_str(self) -> &'static str {
        match self {
            WorktreeState::NotCreated => "not_created",
            WorktreeState::Removed => "removed",
            WorktreeState::Retained => "retained",
        }
    }

    pub fn parse(value: &str) -> Option<WorktreeState> {
        match value {
            "not_created" => Some(WorktreeState::NotCreated),
            "removed" => Some(WorktreeState::Removed),
            "retained" => Some(WorktreeState::Retained),
            _ => None,
        }
    }
}

/// The worktree disposition as stated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeInput {
    pub state: WorktreeState,
    pub reason: Option<RecordText>,
    pub responsible: Option<RecordText>,
    pub cleanup_condition: Option<RecordText>,
}

/// The next step: each axis a value, or `None` for the schema's explicit
/// `null`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextStepInput {
    pub action: Option<RecordText>,
    pub gate: Option<RecordText>,
    pub actor_ref: Option<RecordText>,
}

/// The whole handoff body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandoffBody {
    pub pins: HandoffPins,
    pub outcome: OutcomeInput,
    pub claimed_results: Vec<ClaimedResultInput>,
    pub assertions: Vec<AssertionInput>,
    pub evidence: Vec<HandoffEvidenceInput>,
    pub mandatory_checks: Vec<MandatoryCheckInput>,
    pub acceptance_criteria: Vec<CriterionInput>,
    pub source_state: Vec<RepositoryStateInput>,
    pub result_state: Vec<RepositoryStateInput>,
    pub changed_paths: Vec<ChangedPathInput>,
    pub external_effects: Vec<ExternalEffectInput>,
    pub deviations: Vec<DeviationInput>,
    pub open_gaps: Vec<OpenGapInput>,
    pub owner_decisions: Vec<OwnerDecisionInput>,
    pub blockers: Vec<BlockerInput>,
    pub worktree: WorktreeInput,
    pub next_step: NextStepInput,
}

/// One schema-clean evidence-and-handoff record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandoffInput {
    pub envelope: RecordEnvelope,
    pub scope: RunStateScope,
    pub body: HandoffBody,
}
