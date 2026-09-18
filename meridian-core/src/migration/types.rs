//! Domain structures of an instance-data migration plan
//! (`registries/operating-model/instance-data-migration.schema.json`,
//! `standards/workspace/instance-data-migration.md` §1–9).
//!
//! Every schema `allOf` conditional this module can express by construction
//! (a mutually-exclusive rollback variant, a disposition's required target,
//! `field_basis.origin` always `assigned`) is expressed that way, so the
//! remaining checks in [`super::checks`] are exactly the ones that need more
//! than one field — or more than one mapping — together.

use core::fmt;

use crate::types::{Authority, ContentDigest, EvidenceRef, NonEmptyString, Scope, Verdict};

/// A value rejected while building a migration-plan type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationTypeError {
    /// `qualification: reproducible` requires `working_tree_clean: true`
    /// (`instance-data-migration.schema.json` `source_state.allOf`).
    ReproducibleRequiresCleanWorkingTree,
    /// A record unit's `classification_basis` equalled its `unit_ref`
    /// verbatim — classification must not be derived from the path or
    /// reference alone (`instance-data-migration.md` §3, `checkCoverage`).
    ClassificationBasisEqualsUnitRef,
    /// A migrated/merged target's `authority.decision_ref` is absent —
    /// `target_authority` requires it (`instance-data-migration.schema.json`
    /// `definitions.target_authority`).
    TargetAuthorityRequiresDecisionRef,
    /// `mapping.owner_decision.decision_ref` and
    /// `target.authority.decision_ref` name different decisions
    /// (`checkTargetAuthority`).
    OwnerDecisionMismatch,
    /// The plan's scope is neither `project-workspace` nor
    /// `repository-scope` (`instance-data-migration.md` §8).
    ScopeNotAllowedForMigrationPlan,
    /// `record_units` or `mappings` was empty — a migration plan requires at
    /// least one of each (`instance-data-migration.schema.json`
    /// `payload.record_units`/`mappings`: `minItems: 1`).
    EmptyList { field: &'static str },
    /// A `merged` disposition's group requires at least two contributing
    /// units, checked where a `Mapping` cannot express it alone; kept here
    /// as a documented variant even though `super::checks::check_target_groups`
    /// is where it is actually raised (a single `Mapping` cannot see its
    /// sibling mappings).
    Other(String),
}

impl fmt::Display for MigrationTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MigrationTypeError::ReproducibleRequiresCleanWorkingTree => write!(
                f,
                "source.qualification is \"reproducible\" but working_tree_clean is not true; a dirty working tree is not compatible with a reproducibility claim"
            ),
            MigrationTypeError::ClassificationBasisEqualsUnitRef => write!(
                f,
                "classification_basis equals unit_ref verbatim; classification must not be derived from the path or reference alone"
            ),
            MigrationTypeError::TargetAuthorityRequiresDecisionRef => {
                write!(f, "a migrated or merged target's authority requires a decision_ref")
            }
            MigrationTypeError::OwnerDecisionMismatch => write!(
                f,
                "target.authority.decision_ref does not match mapping.owner_decision.decision_ref; both must name the same owner decision"
            ),
            MigrationTypeError::ScopeNotAllowedForMigrationPlan => write!(
                f,
                "a migration plan is scoped to project-workspace or repository-scope only"
            ),
            MigrationTypeError::EmptyList { field } => {
                write!(f, "{field} must carry at least one entry; an empty migration plan is never valid")
            }
            MigrationTypeError::Other(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for MigrationTypeError {}

/// Whether a pinned source qualifies as a reproducible base
/// (`instance-data-migration.md` §2). `NotReproducible` always carries its
/// reason — the schema's `qualification_reason` requirement is expressed by
/// the variant, not a separate presence check.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Qualification {
    Reproducible,
    NotReproducible { reason: NonEmptyString },
}

/// The exact, pinned source revision a plan reads from
/// (`instance-data-migration.md` §2). `repository_ref` is an opaque
/// identifier, never a filesystem path — reusing [`EvidenceRef`]'s
/// `checkOpaqueRef` rule.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceState {
    revision: crate::types::Revision,
    digest: ContentDigest,
    repository_ref: EvidenceRef,
    working_tree_clean: bool,
    qualification: Qualification,
}

impl SourceState {
    pub fn new(
        revision: crate::types::Revision,
        digest: ContentDigest,
        repository_ref: EvidenceRef,
        working_tree_clean: bool,
        qualification: Qualification,
    ) -> Result<Self, MigrationTypeError> {
        if matches!(qualification, Qualification::Reproducible) && !working_tree_clean {
            return Err(MigrationTypeError::ReproducibleRequiresCleanWorkingTree);
        }
        Ok(Self {
            revision,
            digest,
            repository_ref,
            working_tree_clean,
            qualification,
        })
    }

    pub fn revision(&self) -> &crate::types::Revision {
        &self.revision
    }
    pub fn digest(&self) -> &ContentDigest {
        &self.digest
    }
    pub fn repository_ref(&self) -> &EvidenceRef {
        &self.repository_ref
    }
    pub fn working_tree_clean(&self) -> bool {
        self.working_tree_clean
    }
    pub fn qualification(&self) -> &Qualification {
        &self.qualification
    }
}

/// One declared source unit a plan accounts for
/// (`instance-data-migration.md` §3).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecordUnit {
    id: crate::types::SemanticId,
    unit_ref: NonEmptyString,
    classification_basis: NonEmptyString,
}

impl RecordUnit {
    /// `classification_basis` must differ from `unit_ref` verbatim —
    /// classification is never derived from the path or reference alone.
    pub fn new(
        id: crate::types::SemanticId,
        unit_ref: NonEmptyString,
        classification_basis: NonEmptyString,
    ) -> Result<Self, MigrationTypeError> {
        if classification_basis.as_str() == unit_ref.as_str() {
            return Err(MigrationTypeError::ClassificationBasisEqualsUnitRef);
        }
        Ok(Self {
            id,
            unit_ref,
            classification_basis,
        })
    }

    pub fn id(&self) -> &crate::types::SemanticId {
        &self.id
    }
    pub fn unit_ref(&self) -> &str {
        self.unit_ref.as_str()
    }
    pub fn classification_basis(&self) -> &str {
        self.classification_basis.as_str()
    }
}

/// The encoding of a [`ContentEnvelope`]'s `content`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Encoding {
    Utf8,
    Base64,
}

impl Encoding {
    pub fn as_str(self) -> &'static str {
        match self {
            Encoding::Utf8 => "utf-8",
            Encoding::Base64 => "base64",
        }
    }
}

/// A target's payload content, in its closed envelope shape
/// (`instance-data-migration.md` §4, `content_envelope`).
///
/// [`ContentEnvelope::new`] is the ONLY constructor, and it performs the
/// FULL `checkContentEnvelope` check (`instance-data-migration.md` §4.1),
/// not merely the closed form: `media_type`/`encoding` compatibility, an
/// independently RECOMPUTED SHA-256 over the actual represented bytes
/// (never trusting the declared `digest`), strict canonical Base64
/// decoding when `encoding: base64` (`super::base64::decode_canonical`,
/// private — round-trip decode-then-re-encode), and canonical-JSON-form
/// checking when `media_type` is JSON (`crate::json::is_canonical`,
/// private). There is no second, "unchecked" constructor and no field is
/// mutable after construction — a value of this type is never merely
/// closed-form-valid while semantically unverified.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContentEnvelope {
    media_type: NonEmptyString,
    encoding: Encoding,
    content: NonEmptyString,
    digest: ContentDigest,
}

/// A value rejected while building a [`ContentEnvelope`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentEnvelopeError {
    /// A JSON (`application/json` or `*+json`) or `text/*` media type was
    /// declared with `encoding: base64`.
    TextualMediaTypeRequiresUtf8,
    /// A non-textual media type was declared with `encoding: utf-8`.
    BinaryMediaTypeRequiresBase64,
    /// `encoding: base64` but `content` is not well-formed CANONICAL
    /// Base64 (wrong alphabet/padding, or a syntactically valid encoding
    /// that does not round-trip through decode-then-re-encode —
    /// `isCanonicalBase64`).
    NotCanonicalBase64,
    /// `media_type` is JSON but `content` is not already the canonical
    /// JSON serialization of its own parsed value (recursively sorted
    /// object keys, no incidental whitespace) — `checkContentEnvelope`'s
    /// JSON branch.
    NotCanonicalJson,
    /// The declared `digest` does not match the SHA-256 independently
    /// recomputed over the actual represented bytes (the UTF-8 bytes of
    /// `content` for `encoding: utf-8`, the decoded binary bytes for
    /// `encoding: base64`) — a digest is never trusted on its own,
    /// including one that is merely internally self-consistent.
    DigestMismatch,
}

impl fmt::Display for ContentEnvelopeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContentEnvelopeError::TextualMediaTypeRequiresUtf8 => {
                write!(f, "a JSON or text/* media type requires encoding \"utf-8\"")
            }
            ContentEnvelopeError::BinaryMediaTypeRequiresBase64 => {
                write!(f, "a non-textual media type requires encoding \"base64\"")
            }
            ContentEnvelopeError::NotCanonicalBase64 => write!(
                f,
                "content is not well-formed canonical base64 (wrong alphabet/padding, or does not round-trip through decode-then-re-encode)"
            ),
            ContentEnvelopeError::NotCanonicalJson => write!(
                f,
                "content is not in canonical JSON form (recursively sorted object keys, no incidental whitespace); a structured fragment is represented as canonical JSON, never an equivalent but differently-formatted one"
            ),
            ContentEnvelopeError::DigestMismatch => write!(
                f,
                "digest does not match the SHA-256 actually recomputed over the represented bytes; a digest is never trusted on its own, including one that is internally self-consistent but simply false"
            ),
        }
    }
}

impl std::error::Error for ContentEnvelopeError {}

fn is_json_media_type(media_type: &str) -> bool {
    media_type == "application/json" || media_type.ends_with("+json")
}

impl ContentEnvelope {
    pub fn new(
        media_type: NonEmptyString,
        encoding: Encoding,
        content: NonEmptyString,
        digest: ContentDigest,
    ) -> Result<Self, ContentEnvelopeError> {
        let is_json = is_json_media_type(media_type.as_str());
        let requires_utf8 = is_json || media_type.as_str().starts_with("text/");
        match (requires_utf8, encoding) {
            (true, Encoding::Base64) => {
                return Err(ContentEnvelopeError::TextualMediaTypeRequiresUtf8)
            }
            (false, Encoding::Utf8) => {
                return Err(ContentEnvelopeError::BinaryMediaTypeRequiresBase64)
            }
            _ => {}
        }

        let actual_bytes: Vec<u8> = match encoding {
            Encoding::Utf8 => content.as_str().as_bytes().to_vec(),
            Encoding::Base64 => super::base64::decode_canonical(content.as_str())
                .ok_or(ContentEnvelopeError::NotCanonicalBase64)?,
        };
        if ContentDigest::of_bytes(&actual_bytes) != digest {
            return Err(ContentEnvelopeError::DigestMismatch);
        }

        if is_json && !crate::json::is_canonical(content.as_str()) {
            return Err(ContentEnvelopeError::NotCanonicalJson);
        }

        Ok(Self {
            media_type,
            encoding,
            content,
            digest,
        })
    }

    pub fn media_type(&self) -> &str {
        self.media_type.as_str()
    }
    pub fn encoding(&self) -> Encoding {
        self.encoding
    }
    pub fn content(&self) -> &str {
        self.content.as_str()
    }
    pub fn digest(&self) -> &ContentDigest {
        &self.digest
    }
}

/// Whether a target record field was `preserved` from the pre-Meridian
/// record or `assigned` by the migration itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FieldBasisValue {
    Preserved,
    Assigned,
}

/// Per-field basis for every field semantics preservation requires
/// (`instance-data-migration.md` §4, `field_basis`).
///
/// `origin` is always [`FieldBasisValue::Assigned`] — a pre-Meridian record
/// never carried this canonical origin shape, so it can never be claimed
/// `preserved` (`checkTargetGroups`). There is no constructor parameter for
/// it, so the invariant cannot be violated by construction.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldBasis {
    schema: FieldBasisValue,
    id: FieldBasisValue,
    title: FieldBasisValue,
    record_type: FieldBasisValue,
    scope: FieldBasisValue,
    schema_version: FieldBasisValue,
    authority: FieldBasisValue,
    payload: FieldBasisValue,
}

impl FieldBasis {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema: FieldBasisValue,
        id: FieldBasisValue,
        title: FieldBasisValue,
        record_type: FieldBasisValue,
        scope: FieldBasisValue,
        schema_version: FieldBasisValue,
        authority: FieldBasisValue,
        payload: FieldBasisValue,
    ) -> Self {
        Self {
            schema,
            id,
            title,
            record_type,
            scope,
            schema_version,
            authority,
            payload,
        }
    }

    pub fn schema(&self) -> FieldBasisValue {
        self.schema
    }
    pub fn id(&self) -> FieldBasisValue {
        self.id
    }
    pub fn title(&self) -> FieldBasisValue {
        self.title
    }
    pub fn record_type(&self) -> FieldBasisValue {
        self.record_type
    }
    pub fn scope(&self) -> FieldBasisValue {
        self.scope
    }
    pub fn schema_version(&self) -> FieldBasisValue {
        self.schema_version
    }
    /// Always [`FieldBasisValue::Assigned`].
    pub fn origin(&self) -> FieldBasisValue {
        FieldBasisValue::Assigned
    }
    pub fn authority(&self) -> FieldBasisValue {
        self.authority
    }
    pub fn payload(&self) -> FieldBasisValue {
        self.payload
    }
}

/// A migrated/merged target's origin — `kind` is always `migrated`
/// (`instance-data-migration.md` §4, `target_origin`). `source_ref` is
/// checked against the group's actual contributing unit id(s) by
/// [`super::checks::check_target_groups`], not by this constructor, which
/// cannot see sibling mappings.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TargetOrigin {
    source_ref: NonEmptyString,
}

impl TargetOrigin {
    pub fn new(source_ref: NonEmptyString) -> Self {
        Self { source_ref }
    }

    pub fn source_ref(&self) -> &str {
        self.source_ref.as_str()
    }
}

/// The full canonical envelope a migrated or merged target is sufficient to
/// build a complete scoped-record from (`instance-data-migration.md` §4,
/// `target_record`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TargetRecord {
    schema_ref: NonEmptyString,
    id: crate::types::SemanticId,
    title: NonEmptyString,
    record_type: crate::types::SemanticId,
    scope: Scope,
    field_basis: FieldBasis,
    origin: TargetOrigin,
    authority: Authority,
    payload: ContentEnvelope,
}

impl TargetRecord {
    /// `authority` must carry a `decision_ref` — semantics preservation
    /// requires an explicit owner decision, never an implied one.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_ref: NonEmptyString,
        id: crate::types::SemanticId,
        title: NonEmptyString,
        record_type: crate::types::SemanticId,
        scope: Scope,
        field_basis: FieldBasis,
        origin: TargetOrigin,
        authority: Authority,
        payload: ContentEnvelope,
    ) -> Result<Self, MigrationTypeError> {
        if authority.decision_ref().is_none() {
            return Err(MigrationTypeError::TargetAuthorityRequiresDecisionRef);
        }
        Ok(Self {
            schema_ref,
            id,
            title,
            record_type,
            scope,
            field_basis,
            origin,
            authority,
            payload,
        })
    }

    pub fn schema_ref(&self) -> &str {
        self.schema_ref.as_str()
    }
    pub fn id(&self) -> &crate::types::SemanticId {
        &self.id
    }
    pub fn title(&self) -> &str {
        self.title.as_str()
    }
    pub fn record_type(&self) -> &crate::types::SemanticId {
        &self.record_type
    }
    pub fn scope(&self) -> &Scope {
        &self.scope
    }
    pub fn field_basis(&self) -> &FieldBasis {
        &self.field_basis
    }
    pub fn origin(&self) -> &TargetOrigin {
        &self.origin
    }
    pub fn authority(&self) -> &Authority {
        &self.authority
    }
    pub fn payload(&self) -> &ContentEnvelope {
        &self.payload
    }
}

/// An owner's decision to migrate or merge a unit
/// (`instance-data-migration.md` §4, `owner_decision`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OwnerDecision {
    decision_ref: NonEmptyString,
    decided_at: crate::resolver::IsoDate,
    reason: NonEmptyString,
}

impl OwnerDecision {
    pub fn new(
        decision_ref: NonEmptyString,
        decided_at: crate::resolver::IsoDate,
        reason: NonEmptyString,
    ) -> Self {
        Self {
            decision_ref,
            decided_at,
            reason,
        }
    }

    pub fn decision_ref(&self) -> &str {
        self.decision_ref.as_str()
    }
    pub fn decided_at(&self) -> &crate::resolver::IsoDate {
        &self.decided_at
    }
    pub fn reason(&self) -> &str {
        self.reason.as_str()
    }
}

/// One record unit's correspondence decision
/// (`instance-data-migration.md` §3, `mapping`). Each variant carries
/// exactly the fields its disposition requires or forbids — the schema's
/// `allOf` per disposition is expressed structurally.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Disposition {
    Migrated {
        target: TargetRecord,
        owner_decision: OwnerDecision,
    },
    Merged {
        target: TargetRecord,
        owner_decision: OwnerDecision,
        merge_rule_ref: EvidenceRef,
    },
    RetainedTransitional {
        retained_reason: NonEmptyString,
    },
}

/// One [`RecordUnit`]'s mapping.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Mapping {
    unit_id: crate::types::SemanticId,
    disposition: Disposition,
}

impl Mapping {
    /// For `migrated`/`merged`, `target.authority.decision_ref` and
    /// `owner_decision.decision_ref` must name the same decision
    /// (`checkTargetAuthority`).
    pub fn new(
        unit_id: crate::types::SemanticId,
        disposition: Disposition,
    ) -> Result<Self, MigrationTypeError> {
        let (target, owner_decision) = match &disposition {
            Disposition::Migrated {
                target,
                owner_decision,
            }
            | Disposition::Merged {
                target,
                owner_decision,
                ..
            } => (Some(target), Some(owner_decision)),
            Disposition::RetainedTransitional { .. } => (None, None),
        };
        if let (Some(target), Some(owner_decision)) = (target, owner_decision) {
            if target.authority().decision_ref() != Some(owner_decision.decision_ref()) {
                return Err(MigrationTypeError::OwnerDecisionMismatch);
            }
        }
        Ok(Self {
            unit_id,
            disposition,
        })
    }

    pub fn unit_id(&self) -> &crate::types::SemanticId {
        &self.unit_id
    }
    pub fn disposition(&self) -> &Disposition {
        &self.disposition
    }
    pub fn target(&self) -> Option<&TargetRecord> {
        match &self.disposition {
            Disposition::Migrated { target, .. } | Disposition::Merged { target, .. } => {
                Some(target)
            }
            Disposition::RetainedTransitional { .. } => None,
        }
    }
}

/// A rollback plan's concrete variant, mutually exclusive by construction
/// (`instance-data-migration.md` §5, `rollback.plan`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RollbackPlan {
    DeterministicReconstruction {
        deterministic_plan_ref: EvidenceRef,
    },
    VerifiedRestoration {
        restoration_evidence_ref: EvidenceRef,
    },
}

/// A plan's mandatory reversibility declaration (`instance-data-migration.md`
/// §5). `rewrites_published_history` is always `false` — there is no
/// constructor parameter for it, so it cannot be declared `true`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Rollback {
    source_snapshot_ref: EvidenceRef,
    plan: RollbackPlan,
}

impl Rollback {
    pub fn new(source_snapshot_ref: EvidenceRef, plan: RollbackPlan) -> Self {
        Self {
            source_snapshot_ref,
            plan,
        }
    }

    pub fn source_snapshot_ref(&self) -> &EvidenceRef {
        &self.source_snapshot_ref
    }
    pub fn plan(&self) -> &RollbackPlan {
        &self.plan
    }
    /// Always `false`.
    pub fn rewrites_published_history(&self) -> bool {
        false
    }
}

/// One sub-verdict of [`Verification`] — `evidence_ref` is carried only by
/// `Verified`, matching the schema's `if status=verified then required
/// evidence_ref, else forbidden`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SubVerdict {
    Verified { evidence_ref: EvidenceRef },
    Unverified,
}

/// A plan's claimed verification result, distinguished from actual evidence
/// (`instance-data-migration.md` §6). `overall_status` reuses
/// [`crate::types::Verdict`] directly — the same closed
/// `VERIFIED`/`UNVERIFIED`/`BLOCKED` set the type names.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Verification {
    coverage: SubVerdict,
    applicability_preservation: SubVerdict,
    overall_status: Verdict,
}

impl Verification {
    pub fn new(
        coverage: SubVerdict,
        applicability_preservation: SubVerdict,
        overall_status: Verdict,
    ) -> Self {
        Self {
            coverage,
            applicability_preservation,
            overall_status,
        }
    }

    pub fn coverage(&self) -> &SubVerdict {
        &self.coverage
    }
    pub fn applicability_preservation(&self) -> &SubVerdict {
        &self.applicability_preservation
    }
    pub fn overall_status(&self) -> Verdict {
        self.overall_status
    }
}

/// One instance-data migration plan
/// (`instance-data-migration.schema.json`, `record_type:
/// instance-migration-plan`).
///
/// This models `id`, `scope` and the `payload` fields the ported checks
/// operate on. `title`, `origin` (always `declared`) and `authority`
/// (always `delegated-run`) are envelope-level constants no
/// `instance-data-migration`-specific check consumes; the generic envelope
/// validity they and the rest of the nine-field record require is
/// [`crate::types`]'s `Scope`/`Origin`/`Authority`/`SemanticId` composed at
/// the `meridian-app` layer, not duplicated here (see the package report).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MigrationPlan {
    id: crate::types::SemanticId,
    scope: Scope,
    source: SourceState,
    record_units: Vec<RecordUnit>,
    mappings: Vec<Mapping>,
    rollback: Rollback,
    verification: Verification,
    declared_plan_fingerprint: ContentDigest,
    declared_idempotency_key: ContentDigest,
    supersedes: Option<NonEmptyString>,
}

impl MigrationPlan {
    /// `scope` must be `project-workspace` or `repository-scope`
    /// (`instance-data-migration.md` §8); `record_units` and `mappings`
    /// must each carry at least one entry.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: crate::types::SemanticId,
        scope: Scope,
        source: SourceState,
        record_units: Vec<RecordUnit>,
        mappings: Vec<Mapping>,
        rollback: Rollback,
        verification: Verification,
        declared_plan_fingerprint: ContentDigest,
        declared_idempotency_key: ContentDigest,
        supersedes: Option<NonEmptyString>,
    ) -> Result<Self, MigrationTypeError> {
        use crate::types::ScopeType;
        if !matches!(
            scope.scope_type(),
            ScopeType::ProjectWorkspace | ScopeType::RepositoryScope
        ) {
            return Err(MigrationTypeError::ScopeNotAllowedForMigrationPlan);
        }
        if record_units.is_empty() {
            return Err(MigrationTypeError::EmptyList {
                field: "record_units",
            });
        }
        if mappings.is_empty() {
            return Err(MigrationTypeError::EmptyList { field: "mappings" });
        }
        Ok(Self {
            id,
            scope,
            source,
            record_units,
            mappings,
            rollback,
            verification,
            declared_plan_fingerprint,
            declared_idempotency_key,
            supersedes,
        })
    }

    pub fn id(&self) -> &crate::types::SemanticId {
        &self.id
    }
    pub fn scope(&self) -> &Scope {
        &self.scope
    }
    pub fn source(&self) -> &SourceState {
        &self.source
    }
    pub fn record_units(&self) -> &[RecordUnit] {
        &self.record_units
    }
    pub fn mappings(&self) -> &[Mapping] {
        &self.mappings
    }
    pub fn rollback(&self) -> &Rollback {
        &self.rollback
    }
    pub fn verification(&self) -> &Verification {
        &self.verification
    }
    pub fn declared_plan_fingerprint(&self) -> &ContentDigest {
        &self.declared_plan_fingerprint
    }
    pub fn declared_idempotency_key(&self) -> &ContentDigest {
        &self.declared_idempotency_key
    }
    pub fn supersedes(&self) -> Option<&str> {
        self.supersedes.as_ref().map(NonEmptyString::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AuthorityKind, SemanticId};

    fn digest() -> ContentDigest {
        ContentDigest::of_str("x")
    }

    #[test]
    fn source_state_rejects_reproducible_with_dirty_working_tree() {
        let err = SourceState::new(
            crate::types::Revision::new("abc1234").unwrap(),
            digest(),
            EvidenceRef::new("instance:cbs").unwrap(),
            false,
            Qualification::Reproducible,
        )
        .unwrap_err();
        assert_eq!(
            err,
            MigrationTypeError::ReproducibleRequiresCleanWorkingTree
        );
    }

    #[test]
    fn source_state_accepts_not_reproducible_with_dirty_working_tree() {
        assert!(SourceState::new(
            crate::types::Revision::new("abc1234").unwrap(),
            digest(),
            EvidenceRef::new("instance:cbs").unwrap(),
            false,
            Qualification::NotReproducible {
                reason: NonEmptyString::new("dirty working tree").unwrap()
            },
        )
        .is_ok());
    }

    #[test]
    fn record_unit_rejects_classification_basis_equal_to_unit_ref() {
        let err = RecordUnit::new(
            SemanticId::new("legacy-1").unwrap(),
            NonEmptyString::new("legacy/path.md").unwrap(),
            NonEmptyString::new("legacy/path.md").unwrap(),
        )
        .unwrap_err();
        assert_eq!(err, MigrationTypeError::ClassificationBasisEqualsUnitRef);
    }

    #[test]
    fn content_envelope_requires_utf8_for_json_media_type() {
        let err = ContentEnvelope::new(
            NonEmptyString::new("application/json").unwrap(),
            Encoding::Base64,
            NonEmptyString::new("{}").unwrap(),
            digest(),
        )
        .unwrap_err();
        assert_eq!(err, ContentEnvelopeError::TextualMediaTypeRequiresUtf8);
    }

    #[test]
    fn content_envelope_requires_base64_for_binary_media_type() {
        let err = ContentEnvelope::new(
            NonEmptyString::new("application/octet-stream").unwrap(),
            Encoding::Utf8,
            NonEmptyString::new("stub").unwrap(),
            digest(),
        )
        .unwrap_err();
        assert_eq!(err, ContentEnvelopeError::BinaryMediaTypeRequiresBase64);
    }

    #[test]
    fn content_envelope_rejects_a_false_digest_for_utf8_content() {
        let err = ContentEnvelope::new(
            NonEmptyString::new("text/plain").unwrap(),
            Encoding::Utf8,
            NonEmptyString::new("the actual content").unwrap(),
            ContentDigest::of_str("a completely different string"),
        )
        .unwrap_err();
        assert_eq!(err, ContentEnvelopeError::DigestMismatch);
    }

    #[test]
    fn content_envelope_rejects_a_false_digest_for_base64_content() {
        // "aGVsbG8=" is the canonical base64 of "hello"; the digest below
        // is of a different byte sequence entirely.
        let err = ContentEnvelope::new(
            NonEmptyString::new("application/octet-stream").unwrap(),
            Encoding::Base64,
            NonEmptyString::new("aGVsbG8=").unwrap(),
            ContentDigest::of_str("not hello"),
        )
        .unwrap_err();
        assert_eq!(err, ContentEnvelopeError::DigestMismatch);
    }

    #[test]
    fn content_envelope_rejects_corrupted_base64() {
        let err = ContentEnvelope::new(
            NonEmptyString::new("application/octet-stream").unwrap(),
            Encoding::Base64,
            NonEmptyString::new("not-valid-base64!!").unwrap(),
            digest(),
        )
        .unwrap_err();
        assert_eq!(err, ContentEnvelopeError::NotCanonicalBase64);
    }

    #[test]
    fn content_envelope_rejects_valid_but_non_canonical_base64() {
        // "/x==" is syntactically valid base64 (correct alphabet and
        // padding position) but is a non-canonical spelling of the byte
        // 0xff — its don't-care low bits are non-zero. The digest is
        // irrelevant here: base64 canonicality is checked before content
        // bytes are ever compared against a digest for this reason.
        let err = ContentEnvelope::new(
            NonEmptyString::new("application/octet-stream").unwrap(),
            Encoding::Base64,
            NonEmptyString::new("/x==").unwrap(),
            digest(),
        )
        .unwrap_err();
        assert_eq!(err, ContentEnvelopeError::NotCanonicalBase64);
    }

    #[test]
    fn content_envelope_rejects_non_canonical_json_whitespace() {
        let content = r#"{"a": 1}"#; // a space after the colon
        let err = ContentEnvelope::new(
            NonEmptyString::new("application/json").unwrap(),
            Encoding::Utf8,
            NonEmptyString::new(content).unwrap(),
            ContentDigest::of_str(content),
        )
        .unwrap_err();
        assert_eq!(err, ContentEnvelopeError::NotCanonicalJson);
    }

    #[test]
    fn content_envelope_rejects_non_canonical_json_key_order() {
        let content = r#"{"b":1,"a":2}"#; // unsorted keys
        let err = ContentEnvelope::new(
            NonEmptyString::new("application/json").unwrap(),
            Encoding::Utf8,
            NonEmptyString::new(content).unwrap(),
            ContentDigest::of_str(content),
        )
        .unwrap_err();
        assert_eq!(err, ContentEnvelopeError::NotCanonicalJson);
    }

    #[test]
    fn content_envelope_accepts_canonical_json_with_a_matching_digest() {
        let content = r#"{"a":1,"b":[true,null]}"#;
        assert!(ContentEnvelope::new(
            NonEmptyString::new("application/json").unwrap(),
            Encoding::Utf8,
            NonEmptyString::new(content).unwrap(),
            ContentDigest::of_str(content),
        )
        .is_ok());
    }

    #[test]
    fn field_basis_origin_is_always_assigned() {
        let fb = FieldBasis::new(
            FieldBasisValue::Assigned,
            FieldBasisValue::Assigned,
            FieldBasisValue::Preserved,
            FieldBasisValue::Assigned,
            FieldBasisValue::Assigned,
            FieldBasisValue::Assigned,
            FieldBasisValue::Assigned,
            FieldBasisValue::Preserved,
        );
        assert_eq!(fb.origin(), FieldBasisValue::Assigned);
    }

    fn sample_target(decision_ref: &str, scope: Scope) -> TargetRecord {
        TargetRecord::new(
            NonEmptyString::new("scoped-record.schema.json").unwrap(),
            SemanticId::new("branch-naming").unwrap(),
            NonEmptyString::new("Branch naming rule").unwrap(),
            SemanticId::new("norm").unwrap(),
            scope,
            FieldBasis::new(
                FieldBasisValue::Assigned,
                FieldBasisValue::Preserved,
                FieldBasisValue::Preserved,
                FieldBasisValue::Preserved,
                FieldBasisValue::Assigned,
                FieldBasisValue::Assigned,
                FieldBasisValue::Assigned,
                FieldBasisValue::Preserved,
            ),
            TargetOrigin::new(NonEmptyString::new("record-unit:legacy-1").unwrap()),
            Authority::new(
                AuthorityKind::ProjectOwner,
                "workspace-owner",
                Some(decision_ref.to_string()),
            )
            .unwrap(),
            ContentEnvelope::new(
                NonEmptyString::new("text/plain").unwrap(),
                Encoding::Utf8,
                NonEmptyString::new("branch naming text").unwrap(),
                ContentDigest::of_str("branch naming text"),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn workspace_scope() -> Scope {
        Scope::project_workspace(SemanticId::new("sample-project").unwrap(), None)
    }

    #[test]
    fn target_record_requires_a_decision_ref() {
        let err = TargetRecord::new(
            NonEmptyString::new("scoped-record.schema.json").unwrap(),
            SemanticId::new("branch-naming").unwrap(),
            NonEmptyString::new("Branch naming rule").unwrap(),
            SemanticId::new("norm").unwrap(),
            workspace_scope(),
            FieldBasis::new(
                FieldBasisValue::Assigned,
                FieldBasisValue::Preserved,
                FieldBasisValue::Preserved,
                FieldBasisValue::Preserved,
                FieldBasisValue::Assigned,
                FieldBasisValue::Assigned,
                FieldBasisValue::Assigned,
                FieldBasisValue::Preserved,
            ),
            TargetOrigin::new(NonEmptyString::new("record-unit:legacy-1").unwrap()),
            Authority::new(AuthorityKind::ProjectOwner, "workspace-owner", None).unwrap(),
            ContentEnvelope::new(
                NonEmptyString::new("text/plain").unwrap(),
                Encoding::Utf8,
                NonEmptyString::new("t").unwrap(),
                ContentDigest::of_str("t"),
            )
            .unwrap(),
        )
        .unwrap_err();
        assert_eq!(err, MigrationTypeError::TargetAuthorityRequiresDecisionRef);
    }

    #[test]
    fn mapping_rejects_owner_decision_mismatch() {
        let target = sample_target("owner-decision:branch-naming", workspace_scope());
        let owner_decision = OwnerDecision::new(
            NonEmptyString::new("owner-decision:different").unwrap(),
            crate::resolver::IsoDate::new("2026-09-08").unwrap(),
            NonEmptyString::new("accepted as-is").unwrap(),
        );
        let err = Mapping::new(
            SemanticId::new("legacy-1").unwrap(),
            Disposition::Migrated {
                target,
                owner_decision,
            },
        )
        .unwrap_err();
        assert_eq!(err, MigrationTypeError::OwnerDecisionMismatch);
    }

    #[test]
    fn mapping_accepts_matching_owner_decision() {
        let target = sample_target("owner-decision:branch-naming", workspace_scope());
        let owner_decision = OwnerDecision::new(
            NonEmptyString::new("owner-decision:branch-naming").unwrap(),
            crate::resolver::IsoDate::new("2026-09-08").unwrap(),
            NonEmptyString::new("accepted as-is").unwrap(),
        );
        assert!(Mapping::new(
            SemanticId::new("legacy-1").unwrap(),
            Disposition::Migrated {
                target,
                owner_decision
            },
        )
        .is_ok());
    }

    #[test]
    fn migration_plan_rejects_run_state_scope() {
        let scope = Scope::run_state(
            SemanticId::new("run-1").unwrap(),
            crate::types::WorkspaceId::new("sample-project").unwrap(),
        );
        let source = SourceState::new(
            crate::types::Revision::new("abc1234").unwrap(),
            digest(),
            EvidenceRef::new("instance:cbs").unwrap(),
            true,
            Qualification::Reproducible,
        )
        .unwrap();
        let err = MigrationPlan::new(
            SemanticId::new("plan-1").unwrap(),
            scope,
            source,
            vec![RecordUnit::new(
                SemanticId::new("legacy-1").unwrap(),
                NonEmptyString::new("legacy/path.md").unwrap(),
                NonEmptyString::new("kept as norm text").unwrap(),
            )
            .unwrap()],
            vec![Mapping::new(
                SemanticId::new("legacy-1").unwrap(),
                Disposition::RetainedTransitional {
                    retained_reason: NonEmptyString::new("still authoritative in Instance")
                        .unwrap(),
                },
            )
            .unwrap()],
            Rollback::new(
                EvidenceRef::new("snapshot:legacy-1").unwrap(),
                RollbackPlan::VerifiedRestoration {
                    restoration_evidence_ref: EvidenceRef::new("evidence:restore-1").unwrap(),
                },
            ),
            Verification::new(
                SubVerdict::Unverified,
                SubVerdict::Unverified,
                Verdict::Unverified,
            ),
            digest(),
            digest(),
            None,
        )
        .unwrap_err();
        assert_eq!(err, MigrationTypeError::ScopeNotAllowedForMigrationPlan);
    }

    #[test]
    fn migration_plan_rejects_empty_record_units_or_mappings() {
        let source = SourceState::new(
            crate::types::Revision::new("abc1234").unwrap(),
            digest(),
            EvidenceRef::new("instance:cbs").unwrap(),
            true,
            Qualification::Reproducible,
        )
        .unwrap();
        let err = MigrationPlan::new(
            SemanticId::new("plan-1").unwrap(),
            workspace_scope(),
            source,
            vec![],
            vec![],
            Rollback::new(
                EvidenceRef::new("snapshot:legacy-1").unwrap(),
                RollbackPlan::VerifiedRestoration {
                    restoration_evidence_ref: EvidenceRef::new("evidence:restore-1").unwrap(),
                },
            ),
            Verification::new(
                SubVerdict::Unverified,
                SubVerdict::Unverified,
                Verdict::Unverified,
            ),
            digest(),
            digest(),
            None,
        )
        .unwrap_err();
        assert_eq!(
            err,
            MigrationTypeError::EmptyList {
                field: "record_units"
            }
        );
    }
}
