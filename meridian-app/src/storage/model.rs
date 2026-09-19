//! Storage-port model types (`meridian-rust-target-architecture.md` §3, §5).
//!
//! Every type here is adapter-neutral: it names only [`meridian_core`]
//! domain types, [`serde_json`] (already a dependency of this crate for
//! [`crate::source_format`]) and this crate's own opaque wrappers. None of
//! it names `rusqlite`, a `Connection`, a `Row` or a rowid — an adapter
//! (`meridian-storage-sqlite`) translates between its own storage
//! representation and these types at its own boundary, never the reverse.

use core::fmt;

use meridian_core::types::{
    Authority, ContentDigest, EvidenceRef, NonEmptyString, NonEmptyStringError, Origin, Scope,
    ScopeType, SemanticId,
};

/// The schema_version a managed record's envelope declares
/// (`scoped-record.schema.json` `schema_version: {const: 1}`).
///
/// A closed enum of one variant rather than a raw integer, so a future
/// second schema generation is added as a new variant — an unsupported
/// number can never be constructed and silently accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RecordSchemaVersion {
    V1,
}

impl RecordSchemaVersion {
    /// The current, and so far only, generation.
    pub const CURRENT: RecordSchemaVersion = RecordSchemaVersion::V1;

    /// The literal integer the contract uses for this generation.
    pub fn as_u32(self) -> u32 {
        match self {
            RecordSchemaVersion::V1 => 1,
        }
    }

    /// Builds the version named by `value`, or `None` for an integer this
    /// generation does not recognise — never silently mapped to [`Self::V1`].
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            1 => Some(RecordSchemaVersion::V1),
            _ => None,
        }
    }
}

impl fmt::Display for RecordSchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_u32())
    }
}

/// A value rejected while building a [`SchemaRef`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaRefError(NonEmptyStringError);

impl fmt::Display for SchemaRefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid $schema: {}", self.0)
    }
}

impl std::error::Error for SchemaRefError {}

/// The `$schema` a managed record's envelope declares
/// (`scoped-record.schema.json` `required: ["$schema", ...]`) — the exact
/// schema identifier the record was written against, carried verbatim.
///
/// Unlike [`RecordSchemaVersion`] (a closed, Kernel-defined generation
/// number), `$schema` is an open, caller-supplied reference: two records may
/// legitimately declare two different specialised schemas. This type only
/// guarantees the value is a non-empty, non-blank string — it is never
/// defaulted, normalised, or substituted with a generic fallback.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SchemaRef(NonEmptyString);

impl SchemaRef {
    pub fn new(value: impl Into<String>) -> Result<Self, SchemaRefError> {
        NonEmptyString::new(value).map(Self).map_err(SchemaRefError)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for SchemaRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.as_str())
    }
}

/// A one-based revision number within one record's history
/// (`meridian-rust-sqlite-architecture.md` §4.1, `record_revisions`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RevisionNumber(u64);

impl RevisionNumber {
    /// The first revision every record is minted with.
    pub const FIRST: RevisionNumber = RevisionNumber(1);

    /// Wraps an already-known revision number (for example one read back
    /// from storage). Not a public general constructor — callers do not
    /// invent revision numbers, storage does.
    pub fn from_u64(value: u64) -> Self {
        RevisionNumber(value)
    }

    /// The revision immediately after this one.
    pub fn next(self) -> Self {
        RevisionNumber(self.0 + 1)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl fmt::Display for RevisionNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A value rejected while building an [`IdempotencyKey`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotencyKeyError(NonEmptyStringError);

impl fmt::Display for IdempotencyKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid idempotency key: {}", self.0)
    }
}

impl std::error::Error for IdempotencyKeyError {}

/// An opaque key naming one intended write so a repeated `put` of the exact
/// same content is recognised as a repeat rather than a second effect
/// (`meridian-rust-migration-program-plan.md` §6.5a).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdempotencyKey(NonEmptyString);

impl IdempotencyKey {
    pub fn new(value: impl Into<String>) -> Result<Self, IdempotencyKeyError> {
        NonEmptyString::new(value)
            .map(Self)
            .map_err(IdempotencyKeyError)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for IdempotencyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.as_str())
    }
}

/// A value rejected while building a [`Payload`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PayloadError {
    /// `payload` must be a JSON object (`scoped-record.schema.json`
    /// `payload: {type: object}`) — an array, string, number, bool or null
    /// is rejected, not silently wrapped.
    NotAnObject,
}

impl fmt::Display for PayloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PayloadError::NotAnObject => write!(f, "payload must be a JSON object"),
        }
    }
}

impl std::error::Error for PayloadError {}

/// A record's opaque, arbitrary application payload — a JSON object,
/// nothing more specific (`scoped-record.schema.json` `payload: {type:
/// object}`). Carried as a value, never interpreted by this crate.
#[derive(Debug, Clone, PartialEq)]
pub struct Payload(serde_json::Map<String, serde_json::Value>);

impl Payload {
    /// Validates that `value` is a JSON object and wraps it.
    pub fn new(value: serde_json::Value) -> Result<Self, PayloadError> {
        match value {
            serde_json::Value::Object(map) => Ok(Self(map)),
            _ => Err(PayloadError::NotAnObject),
        }
    }

    /// The empty payload — a record that carries no application data.
    pub fn empty() -> Self {
        Self(serde_json::Map::new())
    }

    /// Borrows the underlying JSON object.
    pub fn as_map(&self) -> &serde_json::Map<String, serde_json::Value> {
        &self.0
    }

    /// Consumes `self` as a [`serde_json::Value::Object`].
    pub fn into_value(self) -> serde_json::Value {
        serde_json::Value::Object(self.0)
    }
}

/// The full logical identity of a managed record: the [`Scope`] it belongs
/// to, plus its own [`SemanticId`] within that scope
/// (`workspace-scope-model.md` §1–2). Never a storage rowid and never a
/// database path — two adapters backing two different files must compute the
/// same identity for the same logical record.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecordKey {
    scope: Scope,
    id: SemanticId,
}

impl RecordKey {
    pub fn new(scope: Scope, id: SemanticId) -> Self {
        Self { scope, id }
    }

    pub fn scope(&self) -> &Scope {
        &self.scope
    }

    pub fn id(&self) -> &SemanticId {
        &self.id
    }

    /// A stable, storage-neutral encoding of this identity as one opaque
    /// string: scope type, scope id, scope workspace id, scope organization
    /// profile id (each empty when the scope carries none) and the record's
    /// own id, joined by the ASCII Unit Separator (`U+001F`) — a byte no
    /// [`SemanticId`] or [`meridian_core::types::WorkspaceId`] segment can
    /// ever contain (both are restricted to `[a-z0-9-]`), so the join can
    /// never be ambiguous between two different identities.
    ///
    /// This matches [`Scope::same_scope`] exactly: `organization_profile_id`
    /// IS part of full scope identity there (`type`, `id`, `workspace_id`
    /// AND `organization_profile_id` all equal), so two `project-workspace`
    /// records sharing `type`/`id` but naming two different organization
    /// profiles are two distinct identities, not one — the same rule
    /// `instance-data-migration.md` §7's `checkSupersedes` (`sameScope`)
    /// already applies. Each field occupies its own fixed segment position,
    /// so `None` (encoded as an empty segment) can never be confused with a
    /// value that happens to be the empty string — `SemanticId` and
    /// `WorkspaceId` cannot be empty in the first place.
    pub fn storage_key(&self) -> String {
        format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
            self.scope.scope_type().as_str(),
            self.scope.id(),
            self.scope.workspace_id().map(|w| w.as_str()).unwrap_or(""),
            self.scope
                .organization_profile_id()
                .map(|o| o.as_str())
                .unwrap_or(""),
            self.id.as_str(),
        )
    }
}

impl fmt::Display for RecordKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.storage_key())
    }
}

/// The current, readable state of one managed record — the "current
/// revision" view (`meridian-rust-sqlite-architecture.md` §4.1, `records`).
#[derive(Debug, Clone, PartialEq)]
pub struct ManagedRecord {
    key: RecordKey,
    schema: SchemaRef,
    schema_version: RecordSchemaVersion,
    title: NonEmptyString,
    record_type: SemanticId,
    origin: Origin,
    authority: Authority,
    payload: Payload,
    current_revision: RevisionNumber,
    content_digest: ContentDigest,
}

impl ManagedRecord {
    /// Assembles a `ManagedRecord` from its already-validated parts. Not a
    /// general-purpose constructor for application code to mint records
    /// with — it exists so an adapter can build the port's return type
    /// from what it read back, without exposing its own fields as `pub`.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        key: RecordKey,
        schema: SchemaRef,
        schema_version: RecordSchemaVersion,
        title: NonEmptyString,
        record_type: SemanticId,
        origin: Origin,
        authority: Authority,
        payload: Payload,
        current_revision: RevisionNumber,
        content_digest: ContentDigest,
    ) -> Self {
        Self {
            key,
            schema,
            schema_version,
            title,
            record_type,
            origin,
            authority,
            payload,
            current_revision,
            content_digest,
        }
    }

    pub fn key(&self) -> &RecordKey {
        &self.key
    }
    /// The exact `$schema` this record's envelope declares — carried
    /// verbatim, never defaulted or substituted (see [`SchemaRef`]).
    pub fn schema(&self) -> &str {
        self.schema.as_str()
    }
    pub fn schema_version(&self) -> RecordSchemaVersion {
        self.schema_version
    }
    pub fn title(&self) -> &str {
        self.title.as_str()
    }
    pub fn record_type(&self) -> &SemanticId {
        &self.record_type
    }
    pub fn origin(&self) -> &Origin {
        &self.origin
    }
    pub fn authority(&self) -> &Authority {
        &self.authority
    }
    pub fn payload(&self) -> &Payload {
        &self.payload
    }
    pub fn current_revision(&self) -> RevisionNumber {
        self.current_revision
    }
    /// The digest of this record's current canonical content — the same
    /// digest an idempotent repeat of the write that produced it is checked
    /// against (`super::IdempotencyKey`).
    pub fn content_digest(&self) -> &ContentDigest {
        &self.content_digest
    }
}

/// One immutable, append-only edition of a record
/// (`meridian-rust-sqlite-architecture.md` §4.1, `record_revisions`). Once
/// written, a revision is never updated or deleted through the port — a
/// correction is a new revision, never an edit of this one.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordRevision {
    key: RecordKey,
    revision_number: RevisionNumber,
    schema: SchemaRef,
    schema_version: RecordSchemaVersion,
    title: NonEmptyString,
    record_type: SemanticId,
    origin: Origin,
    authority: Authority,
    payload: Payload,
    content_digest: ContentDigest,
}

impl RecordRevision {
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        key: RecordKey,
        revision_number: RevisionNumber,
        schema: SchemaRef,
        schema_version: RecordSchemaVersion,
        title: NonEmptyString,
        record_type: SemanticId,
        origin: Origin,
        authority: Authority,
        payload: Payload,
        content_digest: ContentDigest,
    ) -> Self {
        Self {
            key,
            revision_number,
            schema,
            schema_version,
            title,
            record_type,
            origin,
            authority,
            payload,
            content_digest,
        }
    }

    pub fn key(&self) -> &RecordKey {
        &self.key
    }
    pub fn revision_number(&self) -> RevisionNumber {
        self.revision_number
    }
    /// The exact `$schema` this revision's envelope declared at the time it
    /// was written — carried verbatim (see [`SchemaRef`]).
    pub fn schema(&self) -> &str {
        self.schema.as_str()
    }
    pub fn schema_version(&self) -> RecordSchemaVersion {
        self.schema_version
    }
    pub fn title(&self) -> &str {
        self.title.as_str()
    }
    pub fn record_type(&self) -> &SemanticId {
        &self.record_type
    }
    pub fn origin(&self) -> &Origin {
        &self.origin
    }
    pub fn authority(&self) -> &Authority {
        &self.authority
    }
    pub fn payload(&self) -> &Payload {
        &self.payload
    }
    pub fn content_digest(&self) -> &ContentDigest {
        &self.content_digest
    }
}

/// A value rejected while building a [`PutRecordRequest`]
/// (`instance-data-migration.md` §4: the `built-in-methodology` ⇄
/// `origin.kind: built-in` ⇄ `authority.kind: methodology-owner`
/// cross-field rule `scoped-record.schema.json` states as two `if/then`
/// implications — enforced here at the port boundary, since
/// [`Scope`]/[`Origin`]/[`Authority`] are independent `meridian-core`
/// types that do not (and should not) know about each other).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PutRecordRequestError {
    /// `scope` is `built-in-methodology` but `origin.kind` is not
    /// `built-in`.
    BuiltInScopeRequiresBuiltInOrigin,
    /// `scope` is `built-in-methodology` but `authority.kind` is not
    /// `methodology-owner`.
    BuiltInScopeRequiresMethodologyOwnerAuthority,
    /// `origin.kind` is `built-in` but `scope` is not
    /// `built-in-methodology`.
    BuiltInOriginRequiresBuiltInScope,
}

impl fmt::Display for PutRecordRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            PutRecordRequestError::BuiltInScopeRequiresBuiltInOrigin => {
                "a built-in-methodology scope requires origin.kind = built-in"
            }
            PutRecordRequestError::BuiltInScopeRequiresMethodologyOwnerAuthority => {
                "a built-in-methodology scope requires authority.kind = methodology-owner"
            }
            PutRecordRequestError::BuiltInOriginRequiresBuiltInScope => {
                "origin.kind = built-in requires a built-in-methodology scope"
            }
        };
        f.write_str(msg)
    }
}

impl std::error::Error for PutRecordRequestError {}

/// A request to create or update a managed record
/// (`meridian-rust-target-architecture.md` §3, `RecordRepository`).
///
/// Every `put` is idempotent by construction: [`Self::idempotency_key`] is
/// mandatory, not an optional afterthought, so a caller cannot accidentally
/// perform a non-idempotent write.
#[derive(Debug, Clone, PartialEq)]
pub struct PutRecordRequest {
    key: RecordKey,
    schema: SchemaRef,
    schema_version: RecordSchemaVersion,
    title: NonEmptyString,
    record_type: SemanticId,
    origin: Origin,
    authority: Authority,
    payload: Payload,
    idempotency_key: IdempotencyKey,
}

impl PutRecordRequest {
    /// Validates the built-in-methodology cross-field rule and builds the
    /// request.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        key: RecordKey,
        schema: SchemaRef,
        schema_version: RecordSchemaVersion,
        title: NonEmptyString,
        record_type: SemanticId,
        origin: Origin,
        authority: Authority,
        payload: Payload,
        idempotency_key: IdempotencyKey,
    ) -> Result<Self, PutRecordRequestError> {
        let is_built_in_scope = key.scope().scope_type() == ScopeType::BuiltInMethodology;
        let is_built_in_origin = matches!(origin, Origin::BuiltIn);
        let is_methodology_owner_authority =
            authority.kind() == meridian_core::types::AuthorityKind::MethodologyOwner;

        if is_built_in_scope && !is_built_in_origin {
            return Err(PutRecordRequestError::BuiltInScopeRequiresBuiltInOrigin);
        }
        if is_built_in_scope && !is_methodology_owner_authority {
            return Err(PutRecordRequestError::BuiltInScopeRequiresMethodologyOwnerAuthority);
        }
        if is_built_in_origin && !is_built_in_scope {
            return Err(PutRecordRequestError::BuiltInOriginRequiresBuiltInScope);
        }

        Ok(Self {
            key,
            schema,
            schema_version,
            title,
            record_type,
            origin,
            authority,
            payload,
            idempotency_key,
        })
    }

    pub fn key(&self) -> &RecordKey {
        &self.key
    }
    /// The `$schema` this write declares for the record's envelope —
    /// stored, read back, and exported verbatim (see [`SchemaRef`]).
    pub fn schema(&self) -> &str {
        self.schema.as_str()
    }
    pub fn schema_version(&self) -> RecordSchemaVersion {
        self.schema_version
    }
    pub fn title(&self) -> &str {
        self.title.as_str()
    }
    pub fn record_type(&self) -> &SemanticId {
        &self.record_type
    }
    pub fn origin(&self) -> &Origin {
        &self.origin
    }
    pub fn authority(&self) -> &Authority {
        &self.authority
    }
    pub fn payload(&self) -> &Payload {
        &self.payload
    }
    pub fn idempotency_key(&self) -> &IdempotencyKey {
        &self.idempotency_key
    }
}

/// The outcome of one successful [`super::RecordRepository::put`]
/// (`meridian-cli-rfc.md` §"E. Идемпотентность" — a repository write always
/// tells the caller which of the three cases happened, never just "ok").
#[derive(Debug, Clone, PartialEq)]
pub enum PutRecordOutcome {
    /// No record existed for this key: minted at [`RevisionNumber::FIRST`].
    Created(ManagedRecord),
    /// A record already existed for this key and a new revision was
    /// appended, atomically switching the current revision.
    Updated(ManagedRecord),
    /// The same [`IdempotencyKey`] was already applied and produced this
    /// exact content (same [`ContentDigest`]) — no second revision, no
    /// second effect. The already-current record is returned unchanged.
    AlreadyApplied(ManagedRecord),
}

impl PutRecordOutcome {
    /// The resulting record, whichever of the three cases produced it.
    pub fn record(&self) -> &ManagedRecord {
        match self {
            PutRecordOutcome::Created(r)
            | PutRecordOutcome::Updated(r)
            | PutRecordOutcome::AlreadyApplied(r) => r,
        }
    }
}

/// A request to record one piece of evidence about an existing record
/// (`meridian-rust-target-architecture.md` §3, `EvidenceRepository`).
#[derive(Debug, Clone, PartialEq)]
pub struct PutEvidenceRequest {
    evidence_ref: EvidenceRef,
    subject: RecordKey,
    summary: NonEmptyString,
    payload: Payload,
}

impl PutEvidenceRequest {
    pub fn new(
        evidence_ref: EvidenceRef,
        subject: RecordKey,
        summary: NonEmptyString,
        payload: Payload,
    ) -> Self {
        Self {
            evidence_ref,
            subject,
            summary,
            payload,
        }
    }

    pub fn evidence_ref(&self) -> &EvidenceRef {
        &self.evidence_ref
    }
    pub fn subject(&self) -> &RecordKey {
        &self.subject
    }
    pub fn summary(&self) -> &str {
        self.summary.as_str()
    }
    pub fn payload(&self) -> &Payload {
        &self.payload
    }
}

/// One piece of evidence as read back from storage.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredEvidence {
    evidence_ref: EvidenceRef,
    subject: RecordKey,
    summary: NonEmptyString,
    payload: Payload,
}

impl StoredEvidence {
    pub fn from_parts(
        evidence_ref: EvidenceRef,
        subject: RecordKey,
        summary: NonEmptyString,
        payload: Payload,
    ) -> Self {
        Self {
            evidence_ref,
            subject,
            summary,
            payload,
        }
    }

    pub fn evidence_ref(&self) -> &EvidenceRef {
        &self.evidence_ref
    }
    pub fn subject(&self) -> &RecordKey {
        &self.subject
    }
    pub fn summary(&self) -> &str {
        self.summary.as_str()
    }
    pub fn payload(&self) -> &Payload {
        &self.payload
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use meridian_core::types::{AuthorityKind, WorkspaceId};

    fn sid(s: &str) -> SemanticId {
        SemanticId::new(s).unwrap()
    }
    fn wid(s: &str) -> WorkspaceId {
        WorkspaceId::new(s).unwrap()
    }
    fn nes(s: &str) -> NonEmptyString {
        NonEmptyString::new(s).unwrap()
    }
    fn test_schema() -> SchemaRef {
        SchemaRef::new(
            "https://meridian.invalid/registries/operating-model/scoped-record.schema.json",
        )
        .unwrap()
    }

    #[test]
    fn storage_key_differs_across_scope_types_with_the_same_bare_id() {
        let a = RecordKey::new(Scope::user_profile(sid("x")), sid("branch-naming"));
        let b = RecordKey::new(Scope::organization_profile(sid("x")), sid("branch-naming"));
        assert_ne!(a.storage_key(), b.storage_key());
    }

    #[test]
    fn storage_key_is_stable_and_does_not_depend_on_anything_outside_scope_and_id() {
        let a = RecordKey::new(
            Scope::repository_scope(sid("branch-naming"), wid("sample-project")),
            sid("rule-1"),
        );
        let b = RecordKey::new(
            Scope::repository_scope(sid("branch-naming"), wid("sample-project")),
            sid("rule-1"),
        );
        assert_eq!(a.storage_key(), b.storage_key());
    }

    #[test]
    fn organization_profile_id_is_a_distinguishing_part_of_identity() {
        // Matches `Scope::same_scope`: two project-workspace scopes with the
        // same `type`/`id` but different `organization_profile_id` are two
        // distinct identities, not one.
        let with_acme = RecordKey::new(
            Scope::project_workspace(sid("sample-project"), Some(sid("acme"))),
            sid("rule-1"),
        );
        let with_other_org = RecordKey::new(
            Scope::project_workspace(sid("sample-project"), Some(sid("other-org"))),
            sid("rule-1"),
        );
        let without_org = RecordKey::new(
            Scope::project_workspace(sid("sample-project"), None),
            sid("rule-1"),
        );
        assert!(!with_acme.scope().same_scope(with_other_org.scope()));
        assert_ne!(with_acme.storage_key(), with_other_org.storage_key());
        assert_ne!(with_acme.storage_key(), without_org.storage_key());
        assert_ne!(with_other_org.storage_key(), without_org.storage_key());
    }

    #[test]
    fn payload_rejects_non_object_json() {
        assert_eq!(
            Payload::new(serde_json::json!([1, 2, 3])).unwrap_err(),
            PayloadError::NotAnObject
        );
        assert_eq!(
            Payload::new(serde_json::json!("x")).unwrap_err(),
            PayloadError::NotAnObject
        );
    }

    #[test]
    fn put_request_rejects_built_in_scope_with_non_built_in_origin() {
        let err = PutRecordRequest::new(
            RecordKey::new(Scope::built_in_methodology(), sid("x")),
            test_schema(),
            RecordSchemaVersion::CURRENT,
            nes("X"),
            sid("norm"),
            Origin::declared("owner-decision:x").unwrap(),
            Authority::new(AuthorityKind::MethodologyOwner, "workspace-owner", None).unwrap(),
            Payload::empty(),
            IdempotencyKey::new("k1").unwrap(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            PutRecordRequestError::BuiltInScopeRequiresBuiltInOrigin
        );
    }

    #[test]
    fn put_request_rejects_built_in_scope_with_non_methodology_owner_authority() {
        let err = PutRecordRequest::new(
            RecordKey::new(Scope::built_in_methodology(), sid("x")),
            test_schema(),
            RecordSchemaVersion::CURRENT,
            nes("X"),
            sid("norm"),
            Origin::built_in(),
            Authority::new(AuthorityKind::User, "alice", None).unwrap(),
            Payload::empty(),
            IdempotencyKey::new("k1").unwrap(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            PutRecordRequestError::BuiltInScopeRequiresMethodologyOwnerAuthority
        );
    }

    #[test]
    fn put_request_rejects_built_in_origin_with_non_built_in_scope() {
        let err = PutRecordRequest::new(
            RecordKey::new(Scope::user_profile(sid("alice")), sid("x")),
            test_schema(),
            RecordSchemaVersion::CURRENT,
            nes("X"),
            sid("norm"),
            Origin::built_in(),
            Authority::new(AuthorityKind::MethodologyOwner, "workspace-owner", None).unwrap(),
            Payload::empty(),
            IdempotencyKey::new("k1").unwrap(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            PutRecordRequestError::BuiltInOriginRequiresBuiltInScope
        );
    }

    #[test]
    fn put_request_accepts_a_consistent_built_in_triple() {
        assert!(PutRecordRequest::new(
            RecordKey::new(Scope::built_in_methodology(), sid("x")),
            test_schema(),
            RecordSchemaVersion::CURRENT,
            nes("X"),
            sid("norm"),
            Origin::built_in(),
            Authority::new(AuthorityKind::MethodologyOwner, "workspace-owner", None).unwrap(),
            Payload::empty(),
            IdempotencyKey::new("k1").unwrap(),
        )
        .is_ok());
    }

    #[test]
    fn put_request_accepts_an_ordinary_non_built_in_record() {
        assert!(PutRecordRequest::new(
            RecordKey::new(
                Scope::project_workspace(sid("sample-project"), None),
                sid("x")
            ),
            test_schema(),
            RecordSchemaVersion::CURRENT,
            nes("X"),
            sid("norm"),
            Origin::declared("owner-decision:x").unwrap(),
            Authority::new(AuthorityKind::ProjectOwner, "workspace-owner", None).unwrap(),
            Payload::empty(),
            IdempotencyKey::new("k1").unwrap(),
        )
        .is_ok());
    }

    #[test]
    fn record_schema_version_round_trips_through_u32() {
        assert_eq!(
            RecordSchemaVersion::from_u32(1),
            Some(RecordSchemaVersion::V1)
        );
        assert_eq!(RecordSchemaVersion::from_u32(2), None);
        assert_eq!(RecordSchemaVersion::CURRENT.as_u32(), 1);
    }

    #[test]
    fn revision_number_next_increments_by_one() {
        assert_eq!(RevisionNumber::FIRST.next().as_u64(), 2);
    }
}
