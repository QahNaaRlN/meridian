//! [`SqliteStorage`] — the only type in this crate that knows SQLite exists,
//! implementing [`RecordRepository`] and [`EvidenceRepository`]
//! (`meridian-rust-target-architecture.md` §4).

use std::fmt;
use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, Row, Transaction};

use meridian_app::storage::{
    DatabaseMetadata, EvidenceRepository, ManagedRecord, PortError, PutEvidenceRequest,
    PutRecordOutcome, PutRecordRequest, RecordKey, RecordRepository, RecordRevision,
    RecordSchemaVersion, RevisionNumber, RoledStorage, SchemaRef, StoredEvidence,
};
use meridian_core::types::EvidenceRef;

use crate::codec::{
    canonical_content_json, content_digest, decode_authority, decode_origin, decode_payload,
    decode_scope, encode_authority, encode_origin, encode_payload, encode_scope,
};
use crate::open_error::OpenError;
use crate::schema;

/// A SQLite-backed implementation of the storage ports. `Connection` is
/// `Send` but not `Sync`; wrapping it in a [`Mutex`] is what makes
/// `SqliteStorage` safely shareable behind an `Arc` without any adapter
/// method needing `&mut self` — every port method here takes `&self`.
///
/// `metadata` is read back from the database's own `database_metadata`
/// table once, at open time (`crate::schema::prepare`), and never from the
/// file path — every write is checked against it
/// (`meridian-rust-migration-program-plan.md` §5.4, item 1).
pub struct SqliteStorage {
    conn: Mutex<Connection>,
    metadata: DatabaseMetadata,
}

/// A hand-written `Debug` that never reveals the underlying `Connection` (no
/// file path, no open statements) — only that a `SqliteStorage` exists. Also
/// what makes `.unwrap_err()` on a `Result<SqliteStorage, _>` (used by tests
/// that assert an `open_path` call fails) compile at all: `unwrap_err`
/// requires the `Ok` type to implement `Debug`, and `rusqlite::Connection`
/// does not derive it for us.
impl fmt::Debug for SqliteStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqliteStorage").finish_non_exhaustive()
    }
}

fn map_sql_err(e: rusqlite::Error) -> PortError {
    PortError::Storage(e.to_string())
}

impl SqliteStorage {
    /// Opens (creating if absent) the database file at `path`, applying
    /// `PRAGMA foreign_keys = ON` and either bootstrapping a fresh schema,
    /// migrating an older one forward, or verifying an existing one's role
    /// (`crate::schema::prepare`).
    ///
    /// `metadata` names the role and Kernel edition this open asserts. It is
    /// required, and never inferred from `path`: for a fresh database or one
    /// still at schema version 1, it is the only place that information
    /// comes from; for a database already at the current version, it is
    /// checked against what the database itself already records — both role
    /// and Kernel edition — and any mismatch is refused
    /// (`OpenError::DatabaseMetadataMismatch`). The metadata this
    /// `SqliteStorage` stores and later exposes through
    /// [`Self::database_metadata`] is always what `schema::prepare` reads
    /// back from the database itself, never a copy of this argument.
    pub fn open_path(
        path: impl AsRef<Path>,
        metadata: DatabaseMetadata,
    ) -> Result<Self, OpenError> {
        let mut conn = Connection::open(path).map_err(|e| OpenError::Sqlite(e.to_string()))?;
        let recorded = schema::prepare(&mut conn, &metadata)?;
        Ok(Self {
            conn: Mutex::new(conn),
            metadata: recorded,
        })
    }

    /// Opens a private in-memory database — used by this crate's own tests
    /// and available to any caller that wants a throwaway store with no
    /// file at all. See [`Self::open_path`] for what `metadata` asserts and
    /// what is actually stored.
    pub fn open_in_memory(metadata: DatabaseMetadata) -> Result<Self, OpenError> {
        let mut conn =
            Connection::open_in_memory().map_err(|e| OpenError::Sqlite(e.to_string()))?;
        let recorded = schema::prepare(&mut conn, &metadata)?;
        Ok(Self {
            conn: Mutex::new(conn),
            metadata: recorded,
        })
    }

    /// The role and Kernel edition this database recorded about itself at
    /// open time (`crate::schema::prepare`) — the same accessor
    /// [`RoledStorage::database_metadata`] exposes at the port boundary.
    pub fn database_metadata(&self) -> &DatabaseMetadata {
        &self.metadata
    }

    /// Creates a consistent, independently-openable snapshot of the current
    /// database at `destination`, using SQLite's own `VACUUM INTO`
    /// (`meridian-rust-target-architecture.md` §4.2, "Резервная копия").
    /// The source database is left open and unmodified; `destination` must
    /// not already exist (`VACUUM INTO` refuses to overwrite a file).
    pub fn backup_to(&self, destination: impl AsRef<Path>) -> Result<(), PortError> {
        let dest = destination.as_ref().to_str().ok_or_else(|| {
            PortError::Storage("backup destination path is not valid UTF-8".to_string())
        })?;
        let conn = self.conn.lock().expect("storage mutex poisoned");
        conn.execute("VACUUM INTO ?1", [dest])
            .map_err(map_sql_err)?;
        Ok(())
    }

    /// Diagnostic-only: whether `PRAGMA foreign_keys` is currently on for
    /// this connection (`meridian-rust-target-architecture.md` §4.2) —
    /// checked directly, not only inferred from its effects.
    pub fn foreign_keys_enabled(&self) -> Result<bool, PortError> {
        let conn = self.conn.lock().expect("storage mutex poisoned");
        let value: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .map_err(map_sql_err)?;
        Ok(value != 0)
    }

    /// The schema version this database currently records.
    pub fn schema_version(&self) -> Result<i64, PortError> {
        let conn = self.conn.lock().expect("storage mutex poisoned");
        conn.query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .map_err(map_sql_err)
    }

    /// Diagnostic-only row count of one of this adapter's own tables (see
    /// [`crate::TABLE_NAMES`]) — used to confirm the nine tables exist and
    /// are queryable, and as a cheap "nothing partial was written" check
    /// after a rejected operation. `table` is validated against the closed
    /// table list before use, so this never interpolates arbitrary input
    /// into SQL.
    pub fn table_row_count(&self, table: &str) -> Result<i64, PortError> {
        if !crate::TABLE_NAMES.contains(&table) {
            return Err(PortError::Storage(format!(
                "\"{table}\" is not one of this adapter's tables"
            )));
        }
        let conn = self.conn.lock().expect("storage mutex poisoned");
        let sql = format!("SELECT COUNT(*) FROM {table}");
        conn.query_row(&sql, [], |row| row.get(0))
            .map_err(map_sql_err)
    }

    /// The canonical JSON export of every current record
    /// (`meridian-rust-target-architecture.md` §4.2, "Канонический
    /// экспорт"): a JSON array, each element carrying exactly `$schema`,
    /// `id`, `title`, `record_type`, `scope`, `origin`, `authority`,
    /// `payload` and `schema_version` — no SQLite rowid, internal key or
    /// database path. Records are ordered by [`RecordKey::storage_key`],
    /// and the whole document is serialized with keys sorted recursively
    /// at every level (`crate::codec`), so two calls against unchanged
    /// state produce byte-identical text.
    pub fn canonical_export_json(&self) -> Result<String, PortError> {
        let records = RecordRepository::export_all(self)?;
        let mut array = Vec::with_capacity(records.len());
        for record in &records {
            // The exact `$schema` this specific record declared — never a
            // fixed or generic fallback (`meridian-app::storage::SchemaRef`).
            let value = canonical_content_json(
                record.key(),
                record.schema(),
                record.schema_version().as_u32(),
                record.title(),
                record.record_type().as_str(),
                record.origin(),
                record.authority(),
                record.payload(),
            );
            array.push(value);
        }
        serde_json::to_string(&serde_json::Value::Array(array))
            .map_err(|e| PortError::Storage(e.to_string()))
    }

    fn managed_record_from_row(row: &Row<'_>) -> rusqlite::Result<ManagedRecord> {
        let scope_type: String = row.get("scope_type")?;
        let scope_id: String = row.get("scope_id")?;
        let scope_workspace_id: Option<String> = row.get("scope_workspace_id")?;
        let scope_organization_profile_id: Option<String> =
            row.get("scope_organization_profile_id")?;
        let record_id: String = row.get("record_id")?;
        let schema_ref: String = row.get("schema_ref")?;
        let schema_version: i64 = row.get("schema_version")?;
        let title: String = row.get("title")?;
        let record_type: String = row.get("record_type")?;
        let origin_kind: String = row.get("origin_kind")?;
        let origin_source_ref: Option<String> = row.get("origin_source_ref")?;
        let authority_kind: String = row.get("authority_kind")?;
        let authority_ref: String = row.get("authority_ref")?;
        let authority_decision_ref: Option<String> = row.get("authority_decision_ref")?;
        let payload: String = row.get("payload")?;
        let content_digest: String = row.get("content_digest")?;
        let current_revision: i64 = row.get("current_revision")?;

        let to_port_err = |e: PortError| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::other(e.to_string())),
            )
        };

        let scope = decode_scope(
            &scope_type,
            &scope_id,
            scope_workspace_id.as_deref(),
            scope_organization_profile_id.as_deref(),
        )
        .map_err(to_port_err)?;
        let id = meridian_core::types::SemanticId::new(record_id)
            .map_err(|e| to_port_err(PortError::Storage(format!("corrupt record_id: {e}"))))?;
        let key = RecordKey::new(scope, id);
        let origin =
            decode_origin(&origin_kind, origin_source_ref.as_deref()).map_err(to_port_err)?;
        let authority = decode_authority(
            &authority_kind,
            &authority_ref,
            authority_decision_ref.as_deref(),
        )
        .map_err(to_port_err)?;
        let payload = decode_payload(&payload).map_err(to_port_err)?;
        let schema_version =
            RecordSchemaVersion::from_u32(schema_version as u32).ok_or_else(|| {
                to_port_err(PortError::Storage(format!(
                    "corrupt schema_version: {schema_version}"
                )))
            })?;
        let title = meridian_core::types::NonEmptyString::new(title)
            .map_err(|e| to_port_err(PortError::Storage(format!("corrupt title: {e}"))))?;
        let record_type = meridian_core::types::SemanticId::new(record_type)
            .map_err(|e| to_port_err(PortError::Storage(format!("corrupt record_type: {e}"))))?;
        let digest = meridian_core::types::ContentDigest::from_hex(content_digest)
            .map_err(|e| to_port_err(PortError::Storage(format!("corrupt content_digest: {e}"))))?;
        let schema = SchemaRef::new(schema_ref)
            .map_err(|e| to_port_err(PortError::Storage(format!("corrupt schema_ref: {e}"))))?;

        Ok(ManagedRecord::from_parts(
            key,
            schema,
            schema_version,
            title,
            record_type,
            origin,
            authority,
            payload,
            RevisionNumber::from_u64(current_revision as u64),
            digest,
        ))
    }

    fn revision_from_row(row: &Row<'_>, key: RecordKey) -> rusqlite::Result<RecordRevision> {
        let revision_number: i64 = row.get("revision_number")?;
        let schema_ref: String = row.get("schema_ref")?;
        let schema_version: i64 = row.get("schema_version")?;
        let title: String = row.get("title")?;
        let record_type: String = row.get("record_type")?;
        let origin_kind: String = row.get("origin_kind")?;
        let origin_source_ref: Option<String> = row.get("origin_source_ref")?;
        let authority_kind: String = row.get("authority_kind")?;
        let authority_ref: String = row.get("authority_ref")?;
        let authority_decision_ref: Option<String> = row.get("authority_decision_ref")?;
        let payload: String = row.get("payload")?;
        let content_digest: String = row.get("content_digest")?;

        let to_port_err = |e: PortError| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::other(e.to_string())),
            )
        };
        let origin =
            decode_origin(&origin_kind, origin_source_ref.as_deref()).map_err(to_port_err)?;
        let authority = decode_authority(
            &authority_kind,
            &authority_ref,
            authority_decision_ref.as_deref(),
        )
        .map_err(to_port_err)?;
        let payload = decode_payload(&payload).map_err(to_port_err)?;
        let schema_version =
            RecordSchemaVersion::from_u32(schema_version as u32).ok_or_else(|| {
                to_port_err(PortError::Storage(format!(
                    "corrupt schema_version: {schema_version}"
                )))
            })?;
        let title = meridian_core::types::NonEmptyString::new(title)
            .map_err(|e| to_port_err(PortError::Storage(format!("corrupt title: {e}"))))?;
        let record_type = meridian_core::types::SemanticId::new(record_type)
            .map_err(|e| to_port_err(PortError::Storage(format!("corrupt record_type: {e}"))))?;
        let digest = meridian_core::types::ContentDigest::from_hex(content_digest)
            .map_err(|e| to_port_err(PortError::Storage(format!("corrupt content_digest: {e}"))))?;
        let schema = SchemaRef::new(schema_ref)
            .map_err(|e| to_port_err(PortError::Storage(format!("corrupt schema_ref: {e}"))))?;

        Ok(RecordRevision::from_parts(
            key,
            RevisionNumber::from_u64(revision_number as u64),
            schema,
            schema_version,
            title,
            record_type,
            origin,
            authority,
            payload,
            digest,
        ))
    }

    /// Applies one request within an already-open transaction — the unit
    /// `put_batch` repeats for every request, so a batch either fully
    /// applies or fully rolls back with the connection's own transaction.
    ///
    /// Checks `role.accepts_scope_type` before touching a single row: this
    /// is the actual persistence boundary, so it is where the role guard
    /// lives, not only in whatever composition chose to call this adapter
    /// (`meridian-rust-migration-program-plan.md` §5.4, item 1).
    fn apply_one(
        tx: &Transaction<'_>,
        request: &PutRecordRequest,
        role: meridian_app::storage::DatabaseRole,
    ) -> Result<PutRecordOutcome, PortError> {
        let key = request.key();
        let scope_type = key.scope().scope_type();
        if !role.accepts_scope_type(scope_type) {
            return Err(PortError::ScopeNotAllowedForDatabaseRole { role, scope_type });
        }
        let storage_key = key.storage_key();
        let requested_digest = content_digest(
            key,
            request.schema(),
            request.schema_version().as_u32(),
            request.title(),
            request.record_type().as_str(),
            request.origin(),
            request.authority(),
            request.payload(),
        );

        let existing_for_key: Option<(String, String)> = tx
            .query_row(
                "SELECT record_key, content_digest FROM record_revisions WHERE idempotency_key = ?1",
                [request.idempotency_key().as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(map_sql_err)?;

        if let Some((existing_record_key, existing_digest)) = existing_for_key {
            if existing_record_key == storage_key && existing_digest == requested_digest.value() {
                let record = Self::load_record(tx, &storage_key)?
                    .ok_or_else(|| PortError::Storage(format!(
                        "idempotency key \"{}\" is recorded but its record \"{storage_key}\" is missing",
                        request.idempotency_key()
                    )))?;
                return Ok(PutRecordOutcome::AlreadyApplied(record));
            }
            return Err(PortError::IdempotencyConflict {
                idempotency_key: request.idempotency_key().as_str().to_string(),
                existing_content_digest: existing_digest,
                requested_content_digest: requested_digest.value().to_string(),
            });
        }

        let scope = encode_scope(key.scope());
        let origin = encode_origin(request.origin());
        let authority = encode_authority(request.authority());
        let payload_text = encode_payload(request.payload());
        let schema_version = request.schema_version().as_u32();

        let current_revision: Option<i64> = tx
            .query_row(
                "SELECT current_revision FROM records WHERE record_key = ?1",
                [&storage_key],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_sql_err)?;

        let (revision_number, created) = match current_revision {
            None => (1i64, true),
            Some(current) => (current + 1, false),
        };

        if created {
            tx.execute(
                "INSERT INTO records (
                    record_key, scope_type, scope_id, scope_workspace_id, scope_organization_profile_id,
                    record_id, schema_ref, schema_version, title, record_type, origin_kind, origin_source_ref,
                    authority_kind, authority_ref, authority_decision_ref, payload, content_digest, current_revision
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
                rusqlite::params![
                    storage_key,
                    scope.scope_type,
                    scope.scope_id,
                    scope.workspace_id,
                    scope.organization_profile_id,
                    key.id().as_str(),
                    request.schema(),
                    schema_version,
                    request.title(),
                    request.record_type().as_str(),
                    origin.kind,
                    origin.source_ref,
                    authority.kind,
                    authority.authority_ref,
                    authority.decision_ref,
                    payload_text,
                    requested_digest.value(),
                    revision_number,
                ],
            )
            .map_err(map_sql_err)?;
        } else {
            tx.execute(
                "UPDATE records SET
                    schema_ref = ?2, schema_version = ?3, title = ?4, record_type = ?5, origin_kind = ?6, origin_source_ref = ?7,
                    authority_kind = ?8, authority_ref = ?9, authority_decision_ref = ?10, payload = ?11,
                    content_digest = ?12, current_revision = ?13
                 WHERE record_key = ?1",
                rusqlite::params![
                    storage_key,
                    request.schema(),
                    schema_version,
                    request.title(),
                    request.record_type().as_str(),
                    origin.kind,
                    origin.source_ref,
                    authority.kind,
                    authority.authority_ref,
                    authority.decision_ref,
                    payload_text,
                    requested_digest.value(),
                    revision_number,
                ],
            )
            .map_err(map_sql_err)?;
        }

        tx.execute(
            "INSERT INTO record_revisions (
                record_key, revision_number, schema_ref, schema_version, title, record_type, origin_kind, origin_source_ref,
                authority_kind, authority_ref, authority_decision_ref, payload, content_digest, idempotency_key
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            rusqlite::params![
                storage_key,
                revision_number,
                request.schema(),
                schema_version,
                request.title(),
                request.record_type().as_str(),
                origin.kind,
                origin.source_ref,
                authority.kind,
                authority.authority_ref,
                authority.decision_ref,
                payload_text,
                requested_digest.value(),
                request.idempotency_key().as_str(),
            ],
        )
        .map_err(map_sql_err)?;

        let record = Self::load_record(tx, &storage_key)?
            .expect("the record just written must be readable in the same transaction");
        Ok(if created {
            PutRecordOutcome::Created(record)
        } else {
            PutRecordOutcome::Updated(record)
        })
    }

    fn load_record(
        tx: &Transaction<'_>,
        storage_key: &str,
    ) -> Result<Option<ManagedRecord>, PortError> {
        tx.query_row(
            "SELECT * FROM records WHERE record_key = ?1",
            [storage_key],
            Self::managed_record_from_row,
        )
        .optional()
        .map_err(map_sql_err)
    }
}

impl RecordRepository for SqliteStorage {
    fn put(&self, request: PutRecordRequest) -> Result<PutRecordOutcome, PortError> {
        let mut conn = self.conn.lock().expect("storage mutex poisoned");
        let tx = conn.transaction().map_err(map_sql_err)?;
        let outcome = Self::apply_one(&tx, &request, self.metadata.role())?;
        tx.commit().map_err(map_sql_err)?;
        Ok(outcome)
    }

    fn put_batch(
        &self,
        requests: Vec<PutRecordRequest>,
    ) -> Result<Vec<PutRecordOutcome>, PortError> {
        let mut conn = self.conn.lock().expect("storage mutex poisoned");
        let tx = conn.transaction().map_err(map_sql_err)?;
        let mut outcomes = Vec::with_capacity(requests.len());
        for request in &requests {
            // A `?`-propagated error drops `tx` here without `commit()`,
            // which rolls back everything applied earlier in this loop —
            // the batch either fully applies or fully does not.
            outcomes.push(Self::apply_one(&tx, request, self.metadata.role())?);
        }
        tx.commit().map_err(map_sql_err)?;
        Ok(outcomes)
    }

    fn get(&self, key: &RecordKey) -> Result<Option<ManagedRecord>, PortError> {
        let conn = self.conn.lock().expect("storage mutex poisoned");
        conn.query_row(
            "SELECT * FROM records WHERE record_key = ?1",
            [key.storage_key()],
            Self::managed_record_from_row,
        )
        .optional()
        .map_err(map_sql_err)
    }

    fn get_revision(
        &self,
        key: &RecordKey,
        revision_number: RevisionNumber,
    ) -> Result<Option<RecordRevision>, PortError> {
        let conn = self.conn.lock().expect("storage mutex poisoned");
        let storage_key = key.storage_key();
        conn.query_row(
            "SELECT * FROM record_revisions WHERE record_key = ?1 AND revision_number = ?2",
            rusqlite::params![storage_key, revision_number.as_u64() as i64],
            |row| Self::revision_from_row(row, key.clone()),
        )
        .optional()
        .map_err(map_sql_err)
    }

    fn list_revisions(&self, key: &RecordKey) -> Result<Vec<RecordRevision>, PortError> {
        let conn = self.conn.lock().expect("storage mutex poisoned");
        let storage_key = key.storage_key();
        let mut stmt = conn
            .prepare(
                "SELECT * FROM record_revisions WHERE record_key = ?1 ORDER BY revision_number ASC",
            )
            .map_err(map_sql_err)?;
        let rows = stmt
            .query_map([storage_key], |row| {
                Self::revision_from_row(row, key.clone())
            })
            .map_err(map_sql_err)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(map_sql_err)?);
        }
        Ok(out)
    }

    fn export_all(&self) -> Result<Vec<ManagedRecord>, PortError> {
        let conn = self.conn.lock().expect("storage mutex poisoned");
        let mut stmt = conn
            .prepare("SELECT * FROM records ORDER BY record_key ASC")
            .map_err(map_sql_err)?;
        let rows = stmt
            .query_map([], Self::managed_record_from_row)
            .map_err(map_sql_err)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(map_sql_err)?);
        }
        Ok(out)
    }
}

impl RoledStorage for SqliteStorage {
    fn database_metadata(&self) -> &DatabaseMetadata {
        &self.metadata
    }
}

impl EvidenceRepository for SqliteStorage {
    fn put(&self, request: PutEvidenceRequest) -> Result<StoredEvidence, PortError> {
        let subject_scope_type = request.subject().scope().scope_type();
        if !self.metadata.role().accepts_scope_type(subject_scope_type) {
            return Err(PortError::ScopeNotAllowedForDatabaseRole {
                role: self.metadata.role(),
                scope_type: subject_scope_type,
            });
        }

        let mut conn = self.conn.lock().expect("storage mutex poisoned");
        let tx = conn.transaction().map_err(map_sql_err)?;

        let subject_key = request.subject().storage_key();
        let subject_exists: bool = tx
            .query_row(
                "SELECT 1 FROM records WHERE record_key = ?1",
                [&subject_key],
                |_| Ok(()),
            )
            .optional()
            .map_err(map_sql_err)?
            .is_some();
        if !subject_exists {
            return Err(PortError::ReferenceNotFound {
                reference: subject_key,
            });
        }

        let already_exists: bool = tx
            .query_row(
                "SELECT 1 FROM evidence WHERE evidence_ref = ?1",
                [request.evidence_ref().as_str()],
                |_| Ok(()),
            )
            .optional()
            .map_err(map_sql_err)?
            .is_some();
        if already_exists {
            return Err(PortError::EvidenceAlreadyExists {
                evidence_ref: request.evidence_ref().as_str().to_string(),
            });
        }

        let payload_text = encode_payload(request.payload());
        tx.execute(
            "INSERT INTO evidence (evidence_ref, subject_record_key, summary, payload) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                request.evidence_ref().as_str(),
                subject_key,
                request.summary(),
                payload_text,
            ],
        )
        .map_err(map_sql_err)?;

        tx.commit().map_err(map_sql_err)?;

        Ok(StoredEvidence::from_parts(
            request.evidence_ref().clone(),
            request.subject().clone(),
            meridian_core::types::NonEmptyString::new(request.summary().to_string())
                .expect("request.summary() was already a valid NonEmptyString"),
            request.payload().clone(),
        ))
    }

    fn resolve(&self, evidence_ref: &EvidenceRef) -> Result<Option<StoredEvidence>, PortError> {
        let conn = self.conn.lock().expect("storage mutex poisoned");
        let row: Option<(String, String, String)> = conn
            .query_row(
                "SELECT summary, payload, subject_record_key FROM evidence WHERE evidence_ref = ?1",
                [evidence_ref.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .map_err(map_sql_err)?;
        let Some((summary, payload, subject_record_key)) = row else {
            return Ok(None);
        };

        let subject = conn
            .query_row(
                "SELECT * FROM records WHERE record_key = ?1",
                [&subject_record_key],
                Self::managed_record_from_row,
            )
            .map_err(map_sql_err)?
            .key()
            .clone();

        let summary = meridian_core::types::NonEmptyString::new(summary)
            .map_err(|e| PortError::Storage(format!("corrupt evidence summary: {e}")))?;
        let payload = decode_payload(&payload)?;

        Ok(Some(StoredEvidence::from_parts(
            evidence_ref.clone(),
            subject,
            summary,
            payload,
        )))
    }
}
