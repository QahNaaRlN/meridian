//! Strict domain types for one `controlled-rule-intake` rule candidate.
//!
//! Every type here is constructed only through a validating function; a
//! value that is syntactically well-formed but semantically invalid (a
//! `line-range` boundary with `start >= end`, an `applicability_state` of
//! `"candidate"` carrying an `owner_decision`, a candidate whose
//! `origin.source_ref` names a different source than `payload.source_ref`)
//! cannot be represented, let alone reach [`super::checks`].

use core::fmt;

use crate::migration::OwnerDecision;
use crate::types::{
    Authority, AuthorityKind, ContentDigest, Diagnostic, NonEmptyString, NonEmptyStringError,
    Origin, OriginKind, Revision, Scope, ScopeType, SemanticId,
};

use super::fail;

/// The fixed `record_type` of a rule candidate's own envelope
/// (`scoped-record.schema.json`).
pub const RECORD_TYPE: &str = "rule-candidate";

/// The fixed `record_type` of a resolved instruction-source entry
/// (`instruction-source-registry.md`).
pub const SOURCE_RECORD_TYPE: &str = "instruction-source";

/// `payload.source_ref` — a closed, pinned reference to ONE
/// instruction-source-registry entry. Unlike the flexible revision-OR-digest
/// pin used elsewhere in Meridian, a rule candidate pins BOTH an explicit
/// revision AND a SHA-256 digest together (property 2).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PinnedSourceRef {
    id: SemanticId,
    reference: NonEmptyString,
    revision: Revision,
    sha256: ContentDigest,
}

impl PinnedSourceRef {
    pub fn new(
        id: SemanticId,
        reference: NonEmptyString,
        revision: Revision,
        sha256: ContentDigest,
    ) -> Self {
        Self {
            id,
            reference,
            revision,
            sha256,
        }
    }

    pub fn id(&self) -> &SemanticId {
        &self.id
    }

    pub fn reference(&self) -> &str {
        self.reference.as_str()
    }

    pub fn revision(&self) -> &Revision {
        &self.revision
    }

    pub fn sha256(&self) -> &ContentDigest {
        &self.sha256
    }
}

/// A value rejected while building a [`Boundary`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryError {
    /// `start` is not strictly less than `end` — a range must name a
    /// non-empty span. The one numeric check the JSON Schema subset cannot
    /// express (the schema already enforces the mutually-exclusive shapes
    /// of the four units).
    NonPositiveSpan { start: u64, end: u64 },
}

impl fmt::Display for BoundaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BoundaryError::NonPositiveSpan { start, end } => write!(
                f,
                "boundary start ({start}) is not less than end ({end}); a range names a non-empty span"
            ),
        }
    }
}

impl std::error::Error for BoundaryError {}

/// `payload.boundary` — a rule candidate's exact span inside its source.
/// The four forms are mutually exclusive by construction: only
/// `AgentsMdSection` carries a `region`, only `LineRange`/`ByteRange` carry
/// `start`/`end`, and `WholeSource` carries neither.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Boundary {
    AgentsMdSection { region: NonEmptyString },
    LineRange { start: u64, end: u64 },
    ByteRange { start: u64, end: u64 },
    WholeSource,
}

impl Boundary {
    pub fn agents_md_section(region: NonEmptyString) -> Self {
        Boundary::AgentsMdSection { region }
    }

    pub fn line_range(start: u64, end: u64) -> Result<Self, BoundaryError> {
        if start >= end {
            return Err(BoundaryError::NonPositiveSpan { start, end });
        }
        Ok(Boundary::LineRange { start, end })
    }

    pub fn byte_range(start: u64, end: u64) -> Result<Self, BoundaryError> {
        if start >= end {
            return Err(BoundaryError::NonPositiveSpan { start, end });
        }
        Ok(Boundary::ByteRange { start, end })
    }

    pub fn whole_source() -> Self {
        Boundary::WholeSource
    }
}

/// The closed reason a `not-applicable` candidate carries
/// (`payload.not_applicable_reason`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NotApplicableReason {
    OwnerRejected,
    LostConflictingDecision,
}

impl NotApplicableReason {
    pub fn as_str(self) -> &'static str {
        match self {
            NotApplicableReason::OwnerRejected => "owner-rejected",
            NotApplicableReason::LostConflictingDecision => "lost-conflicting-decision",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "owner-rejected" => Some(NotApplicableReason::OwnerRejected),
            "lost-conflicting-decision" => Some(NotApplicableReason::LostConflictingDecision),
            _ => None,
        }
    }
}

impl fmt::Display for NotApplicableReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The closed, MINIMAL three-value applicability state a candidate carries
/// (property 4). Each variant carries exactly the fields the contract
/// permits it to carry: `Candidate` carries no [`OwnerDecision`] at all —
/// there is no field for one — and only `NotApplicable` carries a reason.
/// An invalid combination (a `"candidate"` with an `OwnerDecision`, an
/// `"accepted"` with a `not_applicable_reason`) is unrepresentable.
///
/// `owner_decision` reuses [`crate::migration::OwnerDecision`]
/// (`decision_ref`/`decided_at: IsoDate`/`reason`) rather than a second,
/// shape-identical type: both contracts need the same generic "a named
/// decision, with a reference, a date and a reason" record, and reusing it
/// keeps `decided_at` on `IsoDate` here too (corrective round item 6).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Applicability {
    Candidate,
    Accepted {
        owner_decision: OwnerDecision,
    },
    NotApplicable {
        owner_decision: OwnerDecision,
        reason: NotApplicableReason,
    },
}

impl Applicability {
    /// The literal string the contract uses for this state.
    pub fn state_label(&self) -> &'static str {
        match self {
            Applicability::Candidate => "candidate",
            Applicability::Accepted { .. } => "accepted",
            Applicability::NotApplicable { .. } => "not-applicable",
        }
    }

    pub fn owner_decision(&self) -> Option<&OwnerDecision> {
        match self {
            Applicability::Candidate => None,
            Applicability::Accepted { owner_decision } => Some(owner_decision),
            Applicability::NotApplicable { owner_decision, .. } => Some(owner_decision),
        }
    }

    pub fn not_applicable_reason(&self) -> Option<NotApplicableReason> {
        match self {
            Applicability::NotApplicable { reason, .. } => Some(*reason),
            _ => None,
        }
    }

    /// Whether the owner has decided this candidate one way or the other
    /// (`accepted` or `not-applicable` — never `candidate`).
    pub fn is_decided(&self) -> bool {
        !matches!(self, Applicability::Candidate)
    }

    pub fn is_accepted(&self) -> bool {
        matches!(self, Applicability::Accepted { .. })
    }
}

impl fmt::Display for Applicability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.state_label())
    }
}

/// `payload.semantic_key` / `payload.conflicts_with` entries — an
/// EXPLICITLY asserted content identity used to cluster semantically
/// duplicate candidates. Distinct from [`SemanticId`]: a semantic key is
/// non-empty free text, never required to be `kebab-case`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SemanticKey(NonEmptyString);

impl SemanticKey {
    pub fn new(value: impl Into<String>) -> Result<Self, NonEmptyStringError> {
        NonEmptyString::new(value).map(Self)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for SemanticKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// `payload` — everything about a rule candidate beyond its scoped-record
/// envelope (id/scope/origin/authority). Every field is independently
/// valid by the time a value of this type exists; the CROSS-field checks
/// that also need `origin`/`authority` live on [`RuleCandidate::try_new`],
/// not here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleCandidatePayload {
    source_ref: PinnedSourceRef,
    boundary: Boundary,
    raw_excerpt: NonEmptyString,
    normalized_text: NonEmptyString,
    semantic_key: SemanticKey,
    classification_basis: NonEmptyString,
    conflicts_with: Vec<SemanticKey>,
    applicability: Applicability,
}

impl RuleCandidatePayload {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_ref: PinnedSourceRef,
        boundary: Boundary,
        raw_excerpt: NonEmptyString,
        normalized_text: NonEmptyString,
        semantic_key: SemanticKey,
        classification_basis: NonEmptyString,
        conflicts_with: Vec<SemanticKey>,
        applicability: Applicability,
    ) -> Self {
        Self {
            source_ref,
            boundary,
            raw_excerpt,
            normalized_text,
            semantic_key,
            classification_basis,
            conflicts_with,
            applicability,
        }
    }

    pub fn source_ref(&self) -> &PinnedSourceRef {
        &self.source_ref
    }

    pub fn boundary(&self) -> &Boundary {
        &self.boundary
    }

    pub fn semantic_key(&self) -> &SemanticKey {
        &self.semantic_key
    }

    pub fn conflicts_with(&self) -> &[SemanticKey] {
        &self.conflicts_with
    }

    pub fn applicability(&self) -> &Applicability {
        &self.applicability
    }
}

/// The closed, minimal set of human owner authorities that may decide a
/// rule candidate's applicability, one per logical scope area
/// (`workspace-scope-model.md` §1). `RunState` maps to `None`: a scope with
/// no entry here has no defined human owner, and `DelegatedRun` never
/// appears as a value — a run's authority never itself decides
/// applicability. Exhaustive match: a sixth `ScopeType` variant fails to
/// compile here, not silently falls through to `None`.
fn human_owner_authority_by_scope(scope_type: ScopeType) -> Option<AuthorityKind> {
    match scope_type {
        ScopeType::BuiltInMethodology => Some(AuthorityKind::MethodologyOwner),
        ScopeType::UserProfile => Some(AuthorityKind::User),
        ScopeType::OrganizationProfile => Some(AuthorityKind::Organization),
        ScopeType::ProjectWorkspace => Some(AuthorityKind::ProjectOwner),
        ScopeType::RepositoryScope => Some(AuthorityKind::RepositoryMaintainer),
        ScopeType::RunState => None,
    }
}

/// The canonical portable form of `origin.source_ref` for a rule candidate:
/// it must name the SAME instruction-source id that `payload.source_ref`
/// pins.
fn canonical_origin_source_ref(source_id: &str) -> String {
    format!("instruction-source:{source_id}")
}

/// The envelope's origin must name the SAME source as `payload.source_ref`
/// (property 2/9). A blank `origin.source_ref` is unrepresentable by the
/// time this runs — [`Origin`]'s own constructors already reject it — so
/// this only checks the cases a valid `Origin` can still get wrong: the
/// wrong `kind`, or a well-formed but mismatched `source_ref`.
fn check_origin_link(
    at: &str,
    origin: &Origin,
    source_id: &SemanticId,
    problems: &mut Vec<Diagnostic>,
) {
    if origin.kind() != OriginKind::Derived {
        problems.push(fail(format!(
            "{at} origin.kind is \"{}\"; a rule candidate is parsed out of a registered source and its origin.kind must be \"derived\" (property 9)",
            origin.kind()
        )));
    }
    let Some(origin_source_ref) = origin.source_ref() else {
        problems.push(fail(format!(
            "{at} origin carries no source_ref though payload.source_ref pins instruction source \"{source_id}\"; origin.source_ref and payload.source_ref must name the same source (property 2/9)"
        )));
        return;
    };
    let expected = canonical_origin_source_ref(source_id.as_str());
    if origin_source_ref != expected {
        problems.push(fail(format!(
            "{at} origin.source_ref \"{origin_source_ref}\" does not name the same source as payload.source_ref (id \"{source_id}\"); the canonical mapping is \"{expected}\" — a candidate cannot claim provenance from one source in its envelope while pinning another in its payload (property 2/9)"
        )));
    }
}

/// The scope's closed human owner authority must decide
/// accepted/not-applicable, and the envelope's `authority.decision_ref`
/// must name the SAME decision as `payload.owner_decision.decision_ref`
/// (property 9). A no-op for a bare `candidate` — [`Applicability`] makes
/// "decided but carries no owner_decision" unrepresentable.
fn check_owner_authority(
    at: &str,
    applicability: &Applicability,
    authority: &Authority,
    scope: &Scope,
    problems: &mut Vec<Diagnostic>,
) {
    let Some(owner_decision) = applicability.owner_decision() else {
        return;
    };
    let state = applicability.state_label();

    if authority.decision_ref() != Some(owner_decision.decision_ref()) {
        let authority_dref = authority
            .decision_ref()
            .map(|s| format!("\"{s}\""))
            .unwrap_or_else(|| "undefined".to_string());
        problems.push(fail(format!(
            "{at} authority.decision_ref {authority_dref} does not match payload.owner_decision.decision_ref \"{}\"; both name the same decision",
            owner_decision.decision_ref()
        )));
    }

    if authority.kind() == AuthorityKind::DelegatedRun {
        problems.push(fail(format!(
            "{at} applicability_state \"{state}\" is authorized by \"delegated-run\"; a run's delegated authority may carry the candidate's parsing, but only the scope's human owner authority may decide accepted or not-applicable — a matching decision_ref does not substitute for the correct authority.kind (property 9)"
        )));
        return;
    }
    let scope_type = scope.scope_type();
    let Some(required) = human_owner_authority_by_scope(scope_type) else {
        problems.push(fail(format!(
            "{at} applicability_state \"{state}\" is scoped to {scope_type}, which has no defined human owner authority for an applicability decision; a candidate there cannot leave the candidate state (property 9)"
        )));
        return;
    };
    if authority.kind() != required {
        problems.push(fail(format!(
            "{at} applicability_state \"{state}\" is authorized by \"{}\", but scope \"{scope_type}\" requires owner authority \"{required}\"; a matching decision_ref does not substitute for the correct authority.kind (property 9)",
            authority.kind()
        )));
    }
}

/// One rule-candidate document entry: its scoped-record envelope
/// (`id`/`scope`/`origin`/`authority`, reusing `meridian-core`'s own
/// envelope types) plus its `controlled-rule-intake` payload.
///
/// The ONLY way to construct a value of this type is [`RuleCandidate::try_new`]
/// (corrective round item 5): there is no public constructor that skips its
/// cross-field consistency checks, so an inconsistent
/// origin/source/authority/decision combination cannot exist as a
/// `RuleCandidate` at all, not even transiently.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleCandidate {
    id: SemanticId,
    scope: Scope,
    origin: Origin,
    authority: Authority,
    payload: RuleCandidatePayload,
}

impl RuleCandidate {
    /// Validates the cross-field consistency this type's own invariant
    /// requires (origin/source link, owner authority, decision_ref) and
    /// only then builds `Self`. Every argument is already individually
    /// valid (a [`SemanticId`], a [`Scope`], ...) by the time this runs —
    /// per-field validation is the caller's job (the transport/domain
    /// conversion boundary); this function's own job is exactly the
    /// checks that need more than one field at once.
    pub fn try_new(
        id: SemanticId,
        scope: Scope,
        origin: Origin,
        authority: Authority,
        payload: RuleCandidatePayload,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut problems = Vec::new();
        let at = format!("rule candidate \"{id}\"");
        check_origin_link(&at, &origin, payload.source_ref().id(), &mut problems);
        check_owner_authority(
            &at,
            payload.applicability(),
            &authority,
            &scope,
            &mut problems,
        );
        if problems.is_empty() {
            Ok(Self {
                id,
                scope,
                origin,
                authority,
                payload,
            })
        } else {
            Err(problems)
        }
    }

    pub fn id(&self) -> &SemanticId {
        &self.id
    }

    pub fn scope(&self) -> &Scope {
        &self.scope
    }

    pub fn origin(&self) -> &Origin {
        &self.origin
    }

    pub fn authority(&self) -> &Authority {
        &self.authority
    }

    pub fn payload(&self) -> &RuleCandidatePayload {
        &self.payload
    }
}

#[cfg(test)]
mod tests {
    use crate::resolver::IsoDate;
    use crate::types::NonEmptyString;

    use super::*;

    fn owner_decision() -> OwnerDecision {
        OwnerDecision::new(
            NonEmptyString::new("owner-decision:x").unwrap(),
            IsoDate::new("2026-09-21").unwrap(),
            NonEmptyString::new("no longer relevant").unwrap(),
        )
    }

    fn payload_with(source_id: &str, applicability: Applicability) -> RuleCandidatePayload {
        RuleCandidatePayload::new(
            PinnedSourceRef::new(
                SemanticId::new(source_id).unwrap(),
                NonEmptyString::new("sources/src-1").unwrap(),
                Revision::new("a".repeat(40)).unwrap(),
                ContentDigest::from_hex("b".repeat(64)).unwrap(),
            ),
            Boundary::whole_source(),
            NonEmptyString::new("raw").unwrap(),
            NonEmptyString::new("normalized").unwrap(),
            SemanticKey::new("shared-key").unwrap(),
            NonEmptyString::new("basis").unwrap(),
            Vec::new(),
            applicability,
        )
    }

    #[test]
    fn boundary_line_range_rejects_a_non_positive_span() {
        assert!(matches!(
            Boundary::line_range(10, 5).unwrap_err(),
            BoundaryError::NonPositiveSpan { start: 10, end: 5 }
        ));
        assert!(Boundary::line_range(5, 10).is_ok());
    }

    #[test]
    fn applicability_candidate_carries_no_owner_decision_by_construction() {
        let a = Applicability::Candidate;
        assert_eq!(a.owner_decision(), None);
        assert!(!a.is_decided());
    }

    #[test]
    fn try_new_accepts_a_consistent_candidate() {
        let candidate = RuleCandidate::try_new(
            SemanticId::new("cand-1").unwrap(),
            Scope::repository_scope(
                SemanticId::new("sample-repository").unwrap(),
                crate::types::WorkspaceId::new("sample-workspace").unwrap(),
            ),
            Origin::derived("instruction-source:src-1").unwrap(),
            Authority::new(AuthorityKind::DelegatedRun, "run:1", None).unwrap(),
            payload_with("src-1", Applicability::Candidate),
        );
        assert!(candidate.is_ok(), "{candidate:?}");
    }

    /// Corrective round item 5: there is no way to construct a
    /// `RuleCandidate` whose `origin.source_ref` names a different source
    /// than its `payload.source_ref` — `try_new` is the only constructor,
    /// and it rejects the mismatch instead of allowing it to exist and
    /// flagging it afterward.
    #[test]
    fn try_new_rejects_an_origin_that_names_a_different_source_than_the_payload_pins() {
        let candidate = RuleCandidate::try_new(
            SemanticId::new("cand-1").unwrap(),
            Scope::repository_scope(
                SemanticId::new("sample-repository").unwrap(),
                crate::types::WorkspaceId::new("sample-workspace").unwrap(),
            ),
            Origin::derived("instruction-source:a-different-source").unwrap(),
            Authority::new(AuthorityKind::DelegatedRun, "run:1", None).unwrap(),
            payload_with("src-1", Applicability::Candidate),
        );
        let errors = candidate.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|d| d.message().contains("does not name the same source")),
            "{errors:?}"
        );
    }

    #[test]
    fn try_new_rejects_delegated_run_authority_deciding_accepted() {
        let candidate = RuleCandidate::try_new(
            SemanticId::new("cand-1").unwrap(),
            Scope::repository_scope(
                SemanticId::new("sample-repository").unwrap(),
                crate::types::WorkspaceId::new("sample-workspace").unwrap(),
            ),
            Origin::derived("instruction-source:src-1").unwrap(),
            Authority::new(
                AuthorityKind::DelegatedRun,
                "run:1",
                Some("owner-decision:x".to_string()),
            )
            .unwrap(),
            payload_with(
                "src-1",
                Applicability::Accepted {
                    owner_decision: owner_decision(),
                },
            ),
        );
        let errors = candidate.unwrap_err();
        assert!(
            errors.iter().any(|d| d.message().contains("delegated-run")),
            "{errors:?}"
        );
    }

    /// Both simultaneous defects (bad origin link AND bad authority) are
    /// reported together — `try_new` does not stop at the first one
    /// (corrective round item 9).
    #[test]
    fn try_new_accumulates_multiple_simultaneous_cross_field_defects() {
        let candidate = RuleCandidate::try_new(
            SemanticId::new("cand-1").unwrap(),
            Scope::repository_scope(
                SemanticId::new("sample-repository").unwrap(),
                crate::types::WorkspaceId::new("sample-workspace").unwrap(),
            ),
            Origin::derived("instruction-source:a-different-source").unwrap(),
            Authority::new(
                AuthorityKind::DelegatedRun,
                "run:1",
                Some("owner-decision:x".to_string()),
            )
            .unwrap(),
            payload_with(
                "src-1",
                Applicability::Accepted {
                    owner_decision: owner_decision(),
                },
            ),
        );
        let errors = candidate.unwrap_err();
        assert!(
            errors
                .iter()
                .any(|d| d.message().contains("does not name the same source")),
            "{errors:?}"
        );
        assert!(
            errors.iter().any(|d| d.message().contains("delegated-run")),
            "{errors:?}"
        );
        assert_eq!(errors.len(), 2, "{errors:?}");
    }
}
