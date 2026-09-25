//! [`RoledStorage`] and [`StorageRouter`] — the SQLite-neutral boundary
//! that routes an operation to the `tool` or `workspace` database
//! (`meridian-rust-migration-program-plan.md` §5.4, item 4).
//!
//! Nothing here names `rusqlite`, a `Connection` or a file path: a router
//! is built from two adapters that already implement [`RoledStorage`] and
//! [`RecordRepository`] — today always two [`meridian_storage_sqlite::SqliteStorage`]
//! instances, but this module does not know that, and does not need to
//! change if a second storage adapter is ever added
//! (`meridian-rust-target-architecture.md` §3.1).
//!
//! [`meridian_storage_sqlite::SqliteStorage`]: ../../../meridian_storage_sqlite/struct.SqliteStorage.html

use core::fmt;

use meridian_core::types::ScopeType;

use super::database_metadata::DatabaseMetadata;
use super::database_role::DatabaseRole;
use super::error::PortError;
use super::model::{ManagedRecord, PutRecordOutcome, PutRecordRequest, RecordKey};
use super::record_repository::RecordRepository;

/// Exposes the [`DatabaseMetadata`] a storage adapter carries about itself
/// — read from inside the database it backs, never guessed from a path.
pub trait RoledStorage {
    fn database_metadata(&self) -> &DatabaseMetadata;
}

/// A value rejected while building a [`StorageRouter`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouterError {
    /// The adapter passed as `tool` does not actually carry the `tool`
    /// role.
    ToolStorageHasWrongRole { found: DatabaseRole },
    /// The adapter passed as `workspace` does not actually carry the
    /// `workspace` role.
    WorkspaceStorageHasWrongRole { found: DatabaseRole },
}

impl fmt::Display for RouterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RouterError::ToolStorageHasWrongRole { found } => {
                write!(
                    f,
                    "the storage passed as \"tool\" actually carries role \"{found}\""
                )
            }
            RouterError::WorkspaceStorageHasWrongRole { found } => write!(
                f,
                "the storage passed as \"workspace\" actually carries role \"{found}\""
            ),
        }
    }
}

impl std::error::Error for RouterError {}

/// Routes record writes and reads to whichever of two backing storages
/// (`tool` or `workspace`) a record's [`meridian_core::types::Scope`]
/// belongs to (`meridian-rust-migration-program-plan.md` §5.4, item 4).
///
/// Routing is a convenience, not the safety boundary: each backing storage
/// still refuses a wrong-role write on its own
/// (`PortError::ScopeNotAllowedForDatabaseRole`), so calling an adapter
/// directly — bypassing this router — is refused just the same. The router
/// exists so ordinary callers never have to make that choice themselves.
#[derive(Debug)]
pub struct StorageRouter<R> {
    tool: R,
    workspace: R,
}

impl<R: RoledStorage> StorageRouter<R> {
    /// Builds a router from two already-open storages, verifying each
    /// carries the role its position claims. Roles are read from
    /// [`RoledStorage::database_metadata`] — never inferred from which
    /// argument position a caller happened to pass a value in.
    pub fn new(tool: R, workspace: R) -> Result<Self, RouterError> {
        let tool_role = tool.database_metadata().role();
        if tool_role != DatabaseRole::Tool {
            return Err(RouterError::ToolStorageHasWrongRole { found: tool_role });
        }
        let workspace_role = workspace.database_metadata().role();
        if workspace_role != DatabaseRole::Workspace {
            return Err(RouterError::WorkspaceStorageHasWrongRole {
                found: workspace_role,
            });
        }
        Ok(Self { tool, workspace })
    }

    /// The `tool`-role storage, for callers that need to operate on it
    /// directly (for example a full built-in-methodology export).
    pub fn tool(&self) -> &R {
        &self.tool
    }

    /// The `workspace`-role storage, for callers that need to operate on it
    /// directly.
    pub fn workspace(&self) -> &R {
        &self.workspace
    }

    fn target_for(&self, scope_type: ScopeType) -> &R {
        if scope_type == ScopeType::BuiltInMethodology {
            &self.tool
        } else {
            &self.workspace
        }
    }
}

impl<R: RoledStorage + RecordRepository> StorageRouter<R> {
    /// Routes `request` to the storage whose role its scope belongs to.
    pub fn put(&self, request: PutRecordRequest) -> Result<PutRecordOutcome, PortError> {
        self.target_for(request.key().scope().scope_type())
            .put(request)
    }

    /// Routes a whole batch, atomically, to a single backing storage.
    ///
    /// Every request in `requests` must belong to the same role — a batch
    /// is one atomic unit inside one database transaction, and silently
    /// splitting it across two databases would claim an atomicity that does
    /// not exist. A mixed-role batch is refused with
    /// [`PortError::MixedDatabaseRolesInBatch`] instead.
    pub fn put_batch(
        &self,
        requests: Vec<PutRecordRequest>,
    ) -> Result<Vec<PutRecordOutcome>, PortError> {
        let Some(first) = requests.first() else {
            return Ok(Vec::new());
        };
        let role = first.key().scope().scope_type() == ScopeType::BuiltInMethodology;
        for request in &requests {
            let same_role =
                (request.key().scope().scope_type() == ScopeType::BuiltInMethodology) == role;
            if !same_role {
                return Err(PortError::MixedDatabaseRolesInBatch);
            }
        }
        let target = if role { &self.tool } else { &self.workspace };
        target.put_batch(requests)
    }

    /// Routes a read by the key's own scope.
    pub fn get(&self, key: &RecordKey) -> Result<Option<ManagedRecord>, PortError> {
        self.target_for(key.scope().scope_type()).get(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use meridian_core::types::{Revision, Scope, SemanticId};

    use crate::events::{EventKind, EventSink, NoOpEventSink, ObservedEvent};

    #[derive(Debug)]
    struct FakeStorage {
        metadata: DatabaseMetadata,
        records: std::sync::Mutex<std::collections::HashMap<String, PutRecordRequest>>,
    }

    impl FakeStorage {
        fn new(role: DatabaseRole) -> Self {
            Self {
                metadata: DatabaseMetadata::new(role, Revision::new("0.6.0").unwrap()),
                records: std::sync::Mutex::new(std::collections::HashMap::new()),
            }
        }
    }

    impl RoledStorage for FakeStorage {
        fn database_metadata(&self) -> &DatabaseMetadata {
            &self.metadata
        }
    }

    impl RecordRepository for FakeStorage {
        fn put(&self, request: PutRecordRequest) -> Result<PutRecordOutcome, PortError> {
            let scope_type = request.key().scope().scope_type();
            if !self.metadata.role().accepts_scope_type(scope_type) {
                return Err(PortError::ScopeNotAllowedForDatabaseRole {
                    role: self.metadata.role(),
                    scope_type,
                });
            }
            let key = request.key().storage_key();
            self.records.lock().unwrap().insert(key, request.clone());
            // A minimal, valid `ManagedRecord` just to satisfy the return
            // type — this fake never needs to be read back by these tests.
            Ok(PutRecordOutcome::Created(ManagedRecord::from_parts(
                request.key().clone(),
                request_schema(),
                super::super::model::RecordSchemaVersion::CURRENT,
                meridian_core::types::NonEmptyString::new(request.title().to_string()).unwrap(),
                request.record_type().clone(),
                request.origin().clone(),
                request.authority().clone(),
                request.payload().clone(),
                super::super::model::RevisionNumber::FIRST,
                meridian_core::types::ContentDigest::from_hex("0".repeat(64)).unwrap(),
            )))
        }

        fn put_batch(
            &self,
            requests: Vec<PutRecordRequest>,
        ) -> Result<Vec<PutRecordOutcome>, PortError> {
            requests.into_iter().map(|r| self.put(r)).collect()
        }

        fn get(&self, key: &RecordKey) -> Result<Option<ManagedRecord>, PortError> {
            Ok(self
                .records
                .lock()
                .unwrap()
                .get(&key.storage_key())
                .map(|r| {
                    ManagedRecord::from_parts(
                        r.key().clone(),
                        request_schema(),
                        super::super::model::RecordSchemaVersion::CURRENT,
                        meridian_core::types::NonEmptyString::new(r.title().to_string()).unwrap(),
                        r.record_type().clone(),
                        r.origin().clone(),
                        r.authority().clone(),
                        r.payload().clone(),
                        super::super::model::RevisionNumber::FIRST,
                        meridian_core::types::ContentDigest::from_hex("0".repeat(64)).unwrap(),
                    )
                }))
        }

        fn get_revision(
            &self,
            _key: &RecordKey,
            _revision_number: super::super::model::RevisionNumber,
        ) -> Result<Option<super::super::model::RecordRevision>, PortError> {
            Ok(None)
        }

        fn list_revisions(
            &self,
            _key: &RecordKey,
        ) -> Result<Vec<super::super::model::RecordRevision>, PortError> {
            Ok(Vec::new())
        }

        fn export_all(&self) -> Result<Vec<ManagedRecord>, PortError> {
            Ok(Vec::new())
        }
    }

    fn request_schema() -> super::super::model::SchemaRef {
        super::super::model::SchemaRef::new("https://meridian.invalid/schema.json").unwrap()
    }

    fn sid(s: &str) -> SemanticId {
        SemanticId::new(s).unwrap()
    }

    fn built_in_request() -> PutRecordRequest {
        PutRecordRequest::new(
            RecordKey::new(Scope::built_in_methodology(), sid("x")),
            request_schema(),
            super::super::model::RecordSchemaVersion::CURRENT,
            meridian_core::types::NonEmptyString::new("X").unwrap(),
            sid("norm"),
            meridian_core::types::Origin::built_in(),
            meridian_core::types::Authority::new(
                meridian_core::types::AuthorityKind::MethodologyOwner,
                "workspace-owner",
                None,
            )
            .unwrap(),
            super::super::model::Payload::empty(),
            super::super::model::IdempotencyKey::new("k1").unwrap(),
        )
        .unwrap()
    }

    fn project_request(id: &str) -> PutRecordRequest {
        PutRecordRequest::new(
            RecordKey::new(
                Scope::project_workspace(sid("sample-project"), None),
                sid(id),
            ),
            request_schema(),
            super::super::model::RecordSchemaVersion::CURRENT,
            meridian_core::types::NonEmptyString::new("X").unwrap(),
            sid("norm"),
            meridian_core::types::Origin::declared("owner-decision:x").unwrap(),
            meridian_core::types::Authority::new(
                meridian_core::types::AuthorityKind::ProjectOwner,
                "workspace-owner",
                None,
            )
            .unwrap(),
            super::super::model::Payload::empty(),
            super::super::model::IdempotencyKey::new(id).unwrap(),
        )
        .unwrap()
    }

    fn router() -> StorageRouter<FakeStorage> {
        StorageRouter::new(
            FakeStorage::new(DatabaseRole::Tool),
            FakeStorage::new(DatabaseRole::Workspace),
        )
        .unwrap()
    }

    #[test]
    fn new_rejects_a_tool_slot_carrying_the_workspace_role() {
        let err = StorageRouter::new(
            FakeStorage::new(DatabaseRole::Workspace),
            FakeStorage::new(DatabaseRole::Workspace),
        )
        .unwrap_err();
        assert_eq!(
            err,
            RouterError::ToolStorageHasWrongRole {
                found: DatabaseRole::Workspace
            }
        );
    }

    #[test]
    fn new_rejects_a_workspace_slot_carrying_the_tool_role() {
        let err = StorageRouter::new(
            FakeStorage::new(DatabaseRole::Tool),
            FakeStorage::new(DatabaseRole::Tool),
        )
        .unwrap_err();
        assert_eq!(
            err,
            RouterError::WorkspaceStorageHasWrongRole {
                found: DatabaseRole::Tool
            }
        );
    }

    #[test]
    fn routes_a_built_in_methodology_write_to_the_tool_storage() {
        let router = router();
        router.put(built_in_request()).unwrap();
        assert!(router.tool().records.lock().unwrap().len() == 1);
        assert!(router.workspace().records.lock().unwrap().is_empty());
    }

    #[test]
    fn routes_a_product_write_to_the_workspace_storage() {
        let router = router();
        router.put(project_request("a")).unwrap();
        assert!(router.workspace().records.lock().unwrap().len() == 1);
        assert!(router.tool().records.lock().unwrap().is_empty());
    }

    #[test]
    fn routed_get_reads_back_from_the_same_storage_it_was_written_to() {
        let router = router();
        router.put(project_request("a")).unwrap();
        let key = RecordKey::new(
            Scope::project_workspace(sid("sample-project"), None),
            sid("a"),
        );
        assert!(router.get(&key).unwrap().is_some());
    }

    #[test]
    fn put_batch_rejects_a_mix_of_built_in_and_product_requests() {
        let router = router();
        let err = router
            .put_batch(vec![built_in_request(), project_request("a")])
            .unwrap_err();
        assert_eq!(err, PortError::MixedDatabaseRolesInBatch);
    }

    #[test]
    fn put_batch_of_uniform_role_applies_to_one_storage() {
        let router = router();
        router
            .put_batch(vec![project_request("a"), project_request("b")])
            .unwrap();
        assert_eq!(router.workspace().records.lock().unwrap().len(), 2);
    }

    fn sample_event() -> ObservedEvent {
        ObservedEvent::new(
            EventKind::Check,
            meridian_core::types::NonEmptyString::new("ran cargo test").unwrap(),
        )
    }

    /// Runs the exact same real domain operation (`StorageRouter::put`
    /// against a fresh, independent pair of storages) twice: once with
    /// [`NoOpEventSink::record`] called immediately before and after it,
    /// once without any sink call at all. This is the test
    /// `meridian_app::events::sink` points to as proof that
    /// [`NoOpEventSink`] does not change a real domain result or stored
    /// state — not a self-contained function that returns a constant
    /// regardless of what it is given.
    #[test]
    fn a_disabled_event_sink_does_not_change_the_domain_result_or_stored_state() {
        fn run(
            call_sink: bool,
        ) -> (
            PutRecordOutcome,
            std::collections::HashMap<String, PutRecordRequest>,
        ) {
            let router = router();
            if call_sink {
                NoOpEventSink.record(&sample_event());
            }
            let outcome = router.put(project_request("a")).unwrap();
            if call_sink {
                NoOpEventSink.record(&sample_event());
            }
            let state = router.workspace().records.lock().unwrap().clone();
            (outcome, state)
        }

        let (outcome_without_sink, state_without_sink) = run(false);
        let (outcome_with_sink, state_with_sink) = run(true);

        assert_eq!(outcome_without_sink, outcome_with_sink);
        assert_eq!(state_without_sink, state_with_sink);
    }
}
