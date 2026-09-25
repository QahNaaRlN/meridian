//! Strict domain types for `existing-project-compatibility-mode`. Moved
//! here unchanged from `meridian-app`'s own `domain.rs` by the corrective
//! round — see this module's parent doc comment.

use crate::instruction_source::Location;
use crate::resolver::IsoDate;
use crate::types::{Authority, Diagnostic, NonEmptyString, Scope, SemanticId, WorkspaceId};

use super::fail;

/// The closed `payload.connection_mode` pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionMode {
    Compatibility,
    Managed,
}

impl ConnectionMode {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "compatibility" => Some(ConnectionMode::Compatibility),
            "managed" => Some(ConnectionMode::Managed),
            _ => None,
        }
    }
}

/// The closed `payload.scan_kind` pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanKind {
    Initial,
    Rescan,
}

impl ScanKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "initial" => Some(ScanKind::Initial),
            "rescan" => Some(ScanKind::Rescan),
            _ => None,
        }
    }
}

/// The closed `rule_candidates[].discovery_status` pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryStatus {
    New,
    CarriedOver,
}

impl DiscoveryStatus {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "new" => Some(DiscoveryStatus::New),
            "carried-over" => Some(DiscoveryStatus::CarriedOver),
            _ => None,
        }
    }
}

/// The closed `findings[].kind` pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingKind {
    SourceChanged,
    SourceMissing,
    SourceUnreadable,
    Conflict,
    AmbiguousScope,
    UnresolvedDecision,
    Other,
}

impl FindingKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FindingKind::SourceChanged => "source-changed",
            FindingKind::SourceMissing => "source-missing",
            FindingKind::SourceUnreadable => "source-unreadable",
            FindingKind::Conflict => "conflict",
            FindingKind::AmbiguousScope => "ambiguous-scope",
            FindingKind::UnresolvedDecision => "unresolved-decision",
            FindingKind::Other => "other",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "source-changed" => Some(FindingKind::SourceChanged),
            "source-missing" => Some(FindingKind::SourceMissing),
            "source-unreadable" => Some(FindingKind::SourceUnreadable),
            "conflict" => Some(FindingKind::Conflict),
            "ambiguous-scope" => Some(FindingKind::AmbiguousScope),
            "unresolved-decision" => Some(FindingKind::UnresolvedDecision),
            "other" => Some(FindingKind::Other),
            _ => None,
        }
    }
}

/// The ONE closed priority `next_step` a scan's findings set computes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextStep {
    ContinueCompatibilityMode,
    ResolveConflict,
    ResolveAmbiguity,
    AwaitOwnerDecision,
}

impl NextStep {
    pub fn as_str(self) -> &'static str {
        match self {
            NextStep::ContinueCompatibilityMode => "continue-compatibility-mode",
            NextStep::ResolveConflict => "resolve-conflict",
            NextStep::ResolveAmbiguity => "resolve-ambiguity",
            NextStep::AwaitOwnerDecision => "await-owner-decision",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "continue-compatibility-mode" => Some(NextStep::ContinueCompatibilityMode),
            "resolve-conflict" => Some(NextStep::ResolveConflict),
            "resolve-ambiguity" => Some(NextStep::ResolveAmbiguity),
            "await-owner-decision" => Some(NextStep::AwaitOwnerDecision),
            _ => None,
        }
    }
}

impl core::fmt::Display for NextStep {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One discovery-plan slot: an explicit, bounded candidate location. Reuses
/// [`crate::instruction_source::Location`] directly — the same
/// confined-path/opaque-reference discipline a registered source's own
/// location already enforces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryPlanSlot {
    id: SemanticId,
    location: Location,
}

impl DiscoveryPlanSlot {
    pub fn new(id: SemanticId, location: Location) -> Self {
        Self { id, location }
    }

    pub fn id(&self) -> &SemanticId {
        &self.id
    }

    pub fn location(&self) -> &Location {
        &self.location
    }
}

/// Whether a `missing_sources`/`unreadable_sources` entry was already known
/// from a prior scan. Enum-shaped so the schema's own "`id` required iff
/// `previously_known: true`" rule is unrepresentable any other way — there
/// is no constructor for `PreviouslyKnown` without an `id`, and `New` never
/// carries one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviousKnowledge {
    New,
    PreviouslyKnown { id: SemanticId },
}

/// A discovery-plan slot checked this scan and found absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingSource {
    plan_id: SemanticId,
    knowledge: PreviousKnowledge,
}

impl MissingSource {
    pub fn new(plan_id: SemanticId, knowledge: PreviousKnowledge) -> Self {
        Self { plan_id, knowledge }
    }

    pub fn plan_id(&self) -> &SemanticId {
        &self.plan_id
    }

    pub fn knowledge(&self) -> &PreviousKnowledge {
        &self.knowledge
    }
}

/// A discovery-plan slot checked this scan and found present but not
/// readable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreadableSource {
    plan_id: SemanticId,
    reason: NonEmptyString,
    knowledge: PreviousKnowledge,
}

impl UnreadableSource {
    pub fn new(plan_id: SemanticId, reason: NonEmptyString, knowledge: PreviousKnowledge) -> Self {
        Self {
            plan_id,
            reason,
            knowledge,
        }
    }

    pub fn plan_id(&self) -> &SemanticId {
        &self.plan_id
    }

    pub fn knowledge(&self) -> &PreviousKnowledge {
        &self.knowledge
    }
}

/// One separately tracked observation of a scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    kind: FindingKind,
    plan_id: Option<SemanticId>,
    blocking: bool,
}

impl Finding {
    pub fn new(kind: FindingKind, plan_id: Option<SemanticId>, blocking: bool) -> Self {
        Self {
            kind,
            plan_id,
            blocking,
        }
    }

    pub fn kind(&self) -> FindingKind {
        self.kind
    }

    pub fn plan_id(&self) -> Option<&SemanticId> {
        self.plan_id.as_ref()
    }

    pub fn blocking(&self) -> bool {
        self.blocking
    }
}

/// The exact workspace and repository a scan covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryRef {
    id: SemanticId,
    workspace_id: WorkspaceId,
}

impl RepositoryRef {
    pub fn new(id: SemanticId, workspace_id: WorkspaceId) -> Self {
        Self { id, workspace_id }
    }

    pub fn id(&self) -> &SemanticId {
        &self.id
    }

    pub fn workspace_id(&self) -> &WorkspaceId {
        &self.workspace_id
    }
}

/// The separate, verifiable owner decision `managed` connection mode
/// requires before it may ever activate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedModeDecision {
    decided_at: IsoDate,
    authority: Authority,
}

impl ManagedModeDecision {
    pub fn new(decided_at: IsoDate, authority: Authority) -> Self {
        Self {
            decided_at,
            authority,
        }
    }

    pub fn authority(&self) -> &Authority {
        &self.authority
    }
}

/// The area a scan may occupy — only two of the six areas
/// `workspace-scope-model.md` §1 defines.
pub fn build_connection_scope(
    at: &str,
    scope_type: &str,
    id: SemanticId,
    workspace_id: Option<WorkspaceId>,
) -> Result<Scope, Vec<Diagnostic>> {
    match scope_type {
        "project-workspace" => Ok(Scope::project_workspace(id, None)),
        "repository-scope" => match workspace_id {
            Some(workspace_id) => Ok(Scope::repository_scope(id, workspace_id)),
            None => Err(vec![fail(format!(
                "{at} scope.workspace_id is required for scope.type \"repository-scope\""
            ))]),
        },
        other => Err(vec![fail(format!(
            "{at} scope.type \"{other}\" is not one of the two areas a workspace connection scan may occupy (project-workspace, repository-scope)"
        ))]),
    }
}
