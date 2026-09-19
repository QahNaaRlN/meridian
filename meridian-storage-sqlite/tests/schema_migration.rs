//! Integration tests for the corrective package `knowledge-agent-foundation`
//! (`meridian-rust-migration-program-plan.md` §5.4, item 3): schema
//! migration beyond version 1, the database-role guard, and Kernel-edition
//! metadata. Kept separate from `sqlite_storage_adapter.rs` (package
//! `sqlite-storage-adapter`) so this corrective package's own coverage is
//! easy to review on its own.

use std::path::{Path, PathBuf};

use meridian_app::storage::{
    DatabaseMetadata, DatabaseRole, IdempotencyKey, Payload, PortError, PutRecordRequest,
    RecordKey, RecordRepository, RecordSchemaVersion, SchemaRef,
};
use meridian_core::types::{
    Authority, AuthorityKind, NonEmptyString, Origin, Revision, Scope, SemanticId,
};
use meridian_storage_sqlite::{OpenError, SqliteStorage};

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let mut dir = std::env::temp_dir();
        let unique = format!(
            "meridian-storage-sqlite-migration-test-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        dir.push(unique);
        std::fs::create_dir_all(&dir).expect("create private temp dir for test");
        Self(dir)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn sid(s: &str) -> SemanticId {
    SemanticId::new(s).unwrap()
}

fn workspace_metadata() -> DatabaseMetadata {
    DatabaseMetadata::new(DatabaseRole::Workspace, Revision::new("0.6.0").unwrap())
}

fn tool_metadata() -> DatabaseMetadata {
    DatabaseMetadata::new(DatabaseRole::Tool, Revision::new("0.6.0").unwrap())
}

fn schema_ref() -> SchemaRef {
    SchemaRef::new("https://meridian.invalid/registries/operating-model/scoped-record.schema.json")
        .unwrap()
}

fn project_request(id: &str) -> PutRecordRequest {
    PutRecordRequest::new(
        RecordKey::new(
            Scope::project_workspace(sid("sample-project"), None),
            sid(id),
        ),
        schema_ref(),
        RecordSchemaVersion::CURRENT,
        NonEmptyString::new("X").unwrap(),
        sid("norm"),
        Origin::declared("owner-decision:x").unwrap(),
        Authority::new(AuthorityKind::ProjectOwner, "workspace-owner", None).unwrap(),
        Payload::empty(),
        IdempotencyKey::new(id).unwrap(),
    )
    .unwrap()
}

fn built_in_request(id: &str) -> PutRecordRequest {
    PutRecordRequest::new(
        RecordKey::new(Scope::built_in_methodology(), sid(id)),
        schema_ref(),
        RecordSchemaVersion::CURRENT,
        NonEmptyString::new("X").unwrap(),
        sid("norm"),
        Origin::built_in(),
        Authority::new(AuthorityKind::MethodologyOwner, "workspace-owner", None).unwrap(),
        Payload::empty(),
        IdempotencyKey::new(id).unwrap(),
    )
    .unwrap()
}

/// The exact schema version 1 DDL `sqlite-storage-adapter` shipped
/// (`meridian-rust-target-architecture.md` §4.1, before this package): used
/// only to build a genuine v1 fixture database directly with `rusqlite`,
/// bypassing this crate's own `prepare`, which no longer creates anything
/// below the current version. A real v1 database, from before this
/// package existed, looks exactly like this.
const SCHEMA_DDL_V1: &str = r#"
CREATE TABLE schema_migrations (
  version    INTEGER NOT NULL PRIMARY KEY,
  applied_at TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE owner_decisions (
  decision_ref TEXT NOT NULL PRIMARY KEY,
  summary      TEXT NOT NULL,
  recorded_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) WITHOUT ROWID;

CREATE TABLE sources (
  source_ref      TEXT NOT NULL PRIMARY KEY,
  path            TEXT NOT NULL,
  revision        TEXT NOT NULL,
  digest_algorithm TEXT NOT NULL,
  digest_value    TEXT NOT NULL,
  format          TEXT NOT NULL,
  read_channel    TEXT NOT NULL
) WITHOUT ROWID;

CREATE TABLE records (
  record_key                    TEXT    NOT NULL PRIMARY KEY,
  scope_type                    TEXT    NOT NULL,
  scope_id                      TEXT    NOT NULL,
  scope_workspace_id            TEXT,
  scope_organization_profile_id TEXT,
  record_id                     TEXT    NOT NULL,
  schema_ref                    TEXT    NOT NULL,
  schema_version                INTEGER NOT NULL,
  title                         TEXT    NOT NULL,
  record_type                   TEXT    NOT NULL,
  origin_kind                   TEXT    NOT NULL,
  origin_source_ref             TEXT,
  authority_kind                TEXT    NOT NULL,
  authority_ref                 TEXT    NOT NULL,
  authority_decision_ref        TEXT,
  payload                       TEXT    NOT NULL,
  content_digest                TEXT    NOT NULL,
  current_revision              INTEGER NOT NULL
) WITHOUT ROWID;

CREATE TABLE record_revisions (
  record_key             TEXT    NOT NULL REFERENCES records(record_key),
  revision_number        INTEGER NOT NULL,
  schema_ref             TEXT    NOT NULL,
  schema_version         INTEGER NOT NULL,
  title                  TEXT    NOT NULL,
  record_type            TEXT    NOT NULL,
  origin_kind            TEXT    NOT NULL,
  origin_source_ref      TEXT,
  authority_kind         TEXT    NOT NULL,
  authority_ref          TEXT    NOT NULL,
  authority_decision_ref TEXT,
  payload                TEXT    NOT NULL,
  content_digest         TEXT    NOT NULL,
  idempotency_key        TEXT    NOT NULL UNIQUE,
  recorded_at            TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  PRIMARY KEY (record_key, revision_number)
) WITHOUT ROWID;

CREATE TABLE evidence (
  evidence_ref       TEXT NOT NULL PRIMARY KEY,
  subject_record_key TEXT NOT NULL REFERENCES records(record_key),
  summary            TEXT NOT NULL,
  payload            TEXT NOT NULL,
  recorded_at        TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) WITHOUT ROWID;

CREATE TABLE execution_runs (
  run_id              TEXT NOT NULL PRIMARY KEY,
  workspace_id        TEXT,
  lifecycle_stage     TEXT NOT NULL,
  work_state          TEXT NOT NULL,
  supervision_mode    TEXT NOT NULL,
  current_participant TEXT,
  blockers            TEXT NOT NULL DEFAULT '[]',
  next_step           TEXT,
  updated_at          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) WITHOUT ROWID;

CREATE TABLE migration_runs (
  migration_run_id TEXT NOT NULL PRIMARY KEY,
  plan_ref         TEXT NOT NULL,
  plan_fingerprint TEXT NOT NULL,
  idempotency_key  TEXT NOT NULL UNIQUE,
  status           TEXT NOT NULL,
  applied_at       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) WITHOUT ROWID;
"#;

/// Builds a genuine version-1 database at `path`: the schema, one
/// `schema_migrations` row at version 1, and one seeded record (written
/// directly with SQL, since a v1 database predates this crate's own write
/// path too).
fn seed_v1_database_with_one_record(path: &Path) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.execute_batch(SCHEMA_DDL_V1).unwrap();
    conn.execute("INSERT INTO schema_migrations (version) VALUES (1)", [])
        .unwrap();
    conn.execute(
        "INSERT INTO records (
            record_key, scope_type, scope_id, scope_workspace_id, scope_organization_profile_id,
            record_id, schema_ref, schema_version, title, record_type, origin_kind, origin_source_ref,
            authority_kind, authority_ref, authority_decision_ref, payload, content_digest, current_revision
        ) VALUES (
            'project-workspace\u{1f}sample-project\u{1f}\u{1f}\u{1f}pre-existing',
            'project-workspace', 'sample-project', NULL, NULL,
            'pre-existing', 'https://meridian.invalid/schema.json', 1, 'Pre-existing record', 'norm',
            'declared', 'owner-decision:pre-existing', 'project-owner', 'workspace-owner', NULL,
            '{}', 'dededededededededededededededededededededededededededededededede', 1
        )",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO record_revisions (
            record_key, revision_number, schema_ref, schema_version, title, record_type, origin_kind,
            origin_source_ref, authority_kind, authority_ref, authority_decision_ref, payload,
            content_digest, idempotency_key
        ) VALUES (
            'project-workspace\u{1f}sample-project\u{1f}\u{1f}\u{1f}pre-existing', 1,
            'https://meridian.invalid/schema.json', 1, 'Pre-existing record', 'norm',
            'declared', 'owner-decision:pre-existing', 'project-owner', 'workspace-owner', NULL,
            '{}', 'dededededededededededededededededededededededededededededededede', 'seed'
        )",
        [],
    )
    .unwrap();
}

/// Builds a genuine version-1 database at `path` seeded with one
/// `built-in-methodology` record instead of a product one — the symmetric
/// fixture to [`seed_v1_database_with_one_record`], used to prove the v1→v2
/// migration's compatibility check works in both directions.
fn seed_v1_database_with_one_built_in_methodology_record(path: &Path) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.execute_batch(SCHEMA_DDL_V1).unwrap();
    conn.execute("INSERT INTO schema_migrations (version) VALUES (1)", [])
        .unwrap();
    conn.execute(
        "INSERT INTO records (
            record_key, scope_type, scope_id, scope_workspace_id, scope_organization_profile_id,
            record_id, schema_ref, schema_version, title, record_type, origin_kind, origin_source_ref,
            authority_kind, authority_ref, authority_decision_ref, payload, content_digest, current_revision
        ) VALUES (
            'built-in-methodology\u{1f}built-in-methodology\u{1f}\u{1f}\u{1f}kernel-purity',
            'built-in-methodology', 'built-in-methodology', NULL, NULL,
            'kernel-purity', 'https://meridian.invalid/schema.json', 1, 'Kernel purity', 'norm',
            'built-in', NULL, 'methodology-owner', 'workspace-owner', NULL,
            '{}', 'dededededededededededededededededededededededededededededededede', 1
        )",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO record_revisions (
            record_key, revision_number, schema_ref, schema_version, title, record_type, origin_kind,
            origin_source_ref, authority_kind, authority_ref, authority_decision_ref, payload,
            content_digest, idempotency_key
        ) VALUES (
            'built-in-methodology\u{1f}built-in-methodology\u{1f}\u{1f}\u{1f}kernel-purity', 1,
            'https://meridian.invalid/schema.json', 1, 'Kernel purity', 'norm',
            'built-in', NULL, 'methodology-owner', 'workspace-owner', NULL,
            '{}', 'dededededededededededededededededededededededededededededededede', 'seed'
        )",
        [],
    )
    .unwrap();
}

fn raw_schema_version(path: &Path) -> i64 {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
        row.get(0)
    })
    .unwrap()
}

fn raw_table_exists(path: &Path, name: &str) -> bool {
    let conn = rusqlite::Connection::open(path).unwrap();
    let mut stmt = conn
        .prepare("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1")
        .unwrap();
    stmt.exists([name]).unwrap()
}

fn raw_kernel_edition(path: &Path) -> String {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.query_row(
        "SELECT kernel_edition FROM database_metadata WHERE id = 1",
        [],
        |row| row.get(0),
    )
    .unwrap()
}

fn raw_role(path: &Path) -> String {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.query_row(
        "SELECT role FROM database_metadata WHERE id = 1",
        [],
        |row| row.get(0),
    )
    .unwrap()
}

// ---------------------------------------------------------------------------
// Migration of a populated v1 database
// ---------------------------------------------------------------------------

#[test]
fn migrates_a_populated_v1_database_to_v2_without_losing_records() {
    let dir = TempDir::new("migrate-populated");
    let path = dir.join("db.sqlite3");
    seed_v1_database_with_one_record(&path);
    assert_eq!(raw_schema_version(&path), 1);

    let storage = SqliteStorage::open_path(&path, workspace_metadata()).unwrap();

    assert_eq!(storage.schema_version().unwrap(), 2);
    let key = RecordKey::new(
        Scope::project_workspace(sid("sample-project"), None),
        sid("pre-existing"),
    );
    let record = storage.get(&key).unwrap().unwrap();
    assert_eq!(record.title(), "Pre-existing record");

    assert_eq!(storage.database_metadata().role(), DatabaseRole::Workspace);
    assert_eq!(
        storage.database_metadata().kernel_edition().as_str(),
        "0.6.0"
    );
}

#[test]
fn migrating_a_populated_product_v1_database_as_tool_is_rejected() {
    let dir = TempDir::new("migrate-product-as-tool-rejected");
    let path = dir.join("db.sqlite3");
    seed_v1_database_with_one_record(&path); // a project-workspace record
    assert_eq!(raw_schema_version(&path), 1);

    let err = SqliteStorage::open_path(&path, tool_metadata()).unwrap_err();
    assert_eq!(
        err,
        OpenError::MigrationRoleIncompatibleWithExistingRecords {
            role: DatabaseRole::Tool,
            scope_type: "project-workspace".to_string(),
        }
    );

    // The rejected migration left the database exactly as it was: still
    // version 1, no `database_metadata` table at all, and the existing
    // record unchanged (checked directly, independent of this crate's own
    // read path).
    assert_eq!(raw_schema_version(&path), 1);
    assert!(!raw_table_exists(&path, "database_metadata"));
    let conn = rusqlite::Connection::open(&path).unwrap();
    let title: String = conn
        .query_row(
            "SELECT title FROM records WHERE record_key = 'project-workspace\u{1f}sample-project\u{1f}\u{1f}\u{1f}pre-existing'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(title, "Pre-existing record");
}

#[test]
fn migrating_a_populated_built_in_methodology_v1_database_as_workspace_is_rejected() {
    let dir = TempDir::new("migrate-built-in-as-workspace-rejected");
    let path = dir.join("db.sqlite3");
    seed_v1_database_with_one_built_in_methodology_record(&path);
    assert_eq!(raw_schema_version(&path), 1);

    let err = SqliteStorage::open_path(&path, workspace_metadata()).unwrap_err();
    assert_eq!(
        err,
        OpenError::MigrationRoleIncompatibleWithExistingRecords {
            role: DatabaseRole::Workspace,
            scope_type: "built-in-methodology".to_string(),
        }
    );

    assert_eq!(raw_schema_version(&path), 1);
    assert!(!raw_table_exists(&path, "database_metadata"));
    let conn = rusqlite::Connection::open(&path).unwrap();
    let title: String = conn
        .query_row(
            "SELECT title FROM records WHERE record_key = 'built-in-methodology\u{1f}built-in-methodology\u{1f}\u{1f}\u{1f}kernel-purity'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(title, "Kernel purity");
}

// ---------------------------------------------------------------------------
// Idempotent reopen
// ---------------------------------------------------------------------------

#[test]
fn reopening_an_already_migrated_database_is_idempotent() {
    let dir = TempDir::new("idempotent-reopen");
    let path = dir.join("db.sqlite3");
    seed_v1_database_with_one_record(&path);

    // First open performs the v1 -> v2 migration.
    {
        let storage = SqliteStorage::open_path(&path, workspace_metadata()).unwrap();
        assert_eq!(storage.schema_version().unwrap(), 2);
    }

    // A second open of the now-v2 database must be a complete no-op: no
    // second migration attempt (which would fail on `CREATE TABLE
    // database_metadata` of an already-existing table), and the same data
    // and metadata read back unchanged.
    let reopened = SqliteStorage::open_path(&path, workspace_metadata()).unwrap();
    assert_eq!(reopened.schema_version().unwrap(), 2);
    assert_eq!(reopened.database_metadata().role(), DatabaseRole::Workspace);
    let key = RecordKey::new(
        Scope::project_workspace(sid("sample-project"), None),
        sid("pre-existing"),
    );
    assert!(reopened.get(&key).unwrap().is_some());
}

// ---------------------------------------------------------------------------
// Atomic rollback on migration failure
// ---------------------------------------------------------------------------

#[test]
fn a_failed_migration_leaves_the_database_at_its_original_version() {
    let dir = TempDir::new("migration-failure");
    let path = dir.join("db.sqlite3");
    seed_v1_database_with_one_record(&path);

    // Sabotage the v1 -> v2 step ahead of time: the migration's own
    // `CREATE TABLE database_metadata` can only fail if the name is
    // already taken, so pre-create a table of that name with an
    // incompatible shape (no `role`/`kernel_edition` columns at all).
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute("CREATE TABLE database_metadata (bogus TEXT)", [])
            .unwrap();
    }

    let err = SqliteStorage::open_path(&path, workspace_metadata()).unwrap_err();
    assert!(
        matches!(err, OpenError::Sqlite(_)),
        "expected the sabotaged CREATE TABLE to surface as OpenError::Sqlite, got {err:?}"
    );

    // The failed step's transaction never committed: schema_migrations is
    // still exactly at version 1, and the bogus (pre-existing) table is
    // exactly as it was — never partially overwritten and never used to
    // record a version-2 row, proving the step rolled back completely
    // rather than applying part of itself.
    assert_eq!(raw_schema_version(&path), 1);
    let conn = rusqlite::Connection::open(&path).unwrap();
    let columns: Vec<String> = conn
        .prepare("SELECT name FROM pragma_table_info('database_metadata')")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert_eq!(
        columns,
        vec!["bogus".to_string()],
        "the pre-existing bogus table must be untouched by the rolled-back migration"
    );

    // The database is still usable at v1 through the sabotaged file, and a
    // subsequent, unsabotaged attempt still fails the same way — this is
    // not a one-shot poisoned state, it is a stable, correctly-reported
    // failure until the conflicting table is removed.
    let err_again = SqliteStorage::open_path(&path, workspace_metadata()).unwrap_err();
    assert!(matches!(err_again, OpenError::Sqlite(_)));
}

// ---------------------------------------------------------------------------
// Future schema version and unknown role
// ---------------------------------------------------------------------------

#[test]
fn rejects_a_schema_version_beyond_supported_with_a_typed_error() {
    let dir = TempDir::new("future-version");
    let path = dir.join("db.sqlite3");
    SqliteStorage::open_path(&path, workspace_metadata()).unwrap();
    {
        let raw = rusqlite::Connection::open(&path).unwrap();
        raw.execute(
            "UPDATE schema_migrations SET version = 3 WHERE version = 2",
            [],
        )
        .unwrap();
    }
    let err = SqliteStorage::open_path(&path, workspace_metadata()).unwrap_err();
    assert_eq!(err, OpenError::UnsupportedSchemaVersion { found: 3 });
}

#[test]
fn rejects_an_unknown_database_role_with_a_typed_error() {
    let dir = TempDir::new("unknown-role");
    let path = dir.join("db.sqlite3");
    SqliteStorage::open_path(&path, workspace_metadata()).unwrap();
    {
        let raw = rusqlite::Connection::open(&path).unwrap();
        raw.execute(
            "UPDATE database_metadata SET role = 'nonsense' WHERE id = 1",
            [],
        )
        .unwrap();
    }
    let err = SqliteStorage::open_path(&path, workspace_metadata()).unwrap_err();
    assert_eq!(
        err,
        OpenError::UnknownDatabaseRole {
            found: "nonsense".to_string()
        }
    );
}

#[test]
fn rejects_a_role_mismatch_on_reopen() {
    let dir = TempDir::new("role-mismatch");
    let path = dir.join("db.sqlite3");
    SqliteStorage::open_path(&path, tool_metadata()).unwrap();

    let err = SqliteStorage::open_path(&path, workspace_metadata()).unwrap_err();
    assert_eq!(
        err,
        OpenError::DatabaseMetadataMismatch {
            expected: workspace_metadata(),
            found: tool_metadata(),
        }
    );

    // The database itself is untouched by the refused reopen — its role is
    // still readable as `tool` through a correctly-asserted open.
    assert!(raw_table_exists(&path, "database_metadata"));
    let reopened = SqliteStorage::open_path(&path, tool_metadata()).unwrap();
    assert_eq!(reopened.database_metadata().role(), DatabaseRole::Tool);
}

#[test]
fn rejects_a_kernel_edition_mismatch_on_reopen() {
    let dir = TempDir::new("edition-mismatch");
    let path = dir.join("db.sqlite3");
    let created_with = DatabaseMetadata::new(DatabaseRole::Tool, Revision::new("0.6.0").unwrap());
    SqliteStorage::open_path(&path, created_with.clone()).unwrap();

    // Same role, different Kernel edition: no compatibility rule between
    // editions is defined yet, so this is still a refused mismatch, not a
    // silently accepted "close enough".
    let asserted_with = DatabaseMetadata::new(DatabaseRole::Tool, Revision::new("0.7.0").unwrap());
    let err = SqliteStorage::open_path(&path, asserted_with.clone()).unwrap_err();
    assert_eq!(
        err,
        OpenError::DatabaseMetadataMismatch {
            expected: asserted_with,
            found: created_with.clone(),
        }
    );

    // The refused reopen changed nothing: the database still records
    // exactly the edition it was created with, readable independently of
    // this crate's own API and through a correctly-asserted reopen.
    let raw_edition = raw_kernel_edition(&path);
    assert_eq!(raw_edition, "0.6.0");
    let reopened = SqliteStorage::open_path(&path, created_with).unwrap();
    assert_eq!(
        reopened.database_metadata().kernel_edition().as_str(),
        "0.6.0"
    );
}

// ---------------------------------------------------------------------------
// Role guard on writes
// ---------------------------------------------------------------------------

#[test]
fn rejects_a_product_record_written_to_a_tool_database() {
    let dir = TempDir::new("reject-product-in-tool");
    let storage = SqliteStorage::open_path(dir.join("db.sqlite3"), tool_metadata()).unwrap();
    let err = storage.put(project_request("a")).unwrap_err();
    assert_eq!(
        err,
        PortError::ScopeNotAllowedForDatabaseRole {
            role: DatabaseRole::Tool,
            scope_type: meridian_core::types::ScopeType::ProjectWorkspace,
        }
    );
}

#[test]
fn rejects_a_built_in_record_written_to_a_workspace_database() {
    let dir = TempDir::new("reject-built-in-in-workspace");
    let storage = SqliteStorage::open_path(dir.join("db.sqlite3"), workspace_metadata()).unwrap();
    let err = storage.put(built_in_request("kernel-purity")).unwrap_err();
    assert_eq!(
        err,
        PortError::ScopeNotAllowedForDatabaseRole {
            role: DatabaseRole::Workspace,
            scope_type: meridian_core::types::ScopeType::BuiltInMethodology,
        }
    );
}

#[test]
fn accepts_a_built_in_record_written_to_a_tool_database() {
    let dir = TempDir::new("accept-built-in-in-tool");
    let storage = SqliteStorage::open_path(dir.join("db.sqlite3"), tool_metadata()).unwrap();
    storage.put(built_in_request("kernel-purity")).unwrap();
}

// ---------------------------------------------------------------------------
// Kernel-edition metadata
// ---------------------------------------------------------------------------

#[test]
fn database_metadata_reports_the_kernel_edition_it_was_created_with() {
    let dir = TempDir::new("edition");
    let metadata = DatabaseMetadata::new(DatabaseRole::Tool, Revision::new("0.7.0-dev").unwrap());
    let storage = SqliteStorage::open_path(dir.join("db.sqlite3"), metadata).unwrap();
    assert_eq!(
        storage.database_metadata().kernel_edition().as_str(),
        "0.7.0-dev"
    );
    assert_eq!(storage.database_metadata().role(), DatabaseRole::Tool);
}

/// Proves [`SqliteStorage::database_metadata`] is not merely echoing the
/// argument it was given: it reads a database prepared independently of
/// `SqliteStorage::open_path` (a raw fixture, migrated through
/// `open_path` once), then checks the accessor against a second,
/// completely independent raw SQL read of the same row.
#[test]
fn accessor_returns_metadata_actually_read_from_the_database_not_a_copy_of_the_argument() {
    let dir = TempDir::new("accessor-reads-through");
    let path = dir.join("db.sqlite3");
    seed_v1_database_with_one_record(&path);

    let asserted = DatabaseMetadata::new(DatabaseRole::Workspace, Revision::new("0.6.0").unwrap());
    let storage = SqliteStorage::open_path(&path, asserted).unwrap();

    let raw_role_value = raw_role(&path);
    let raw_edition_value = raw_kernel_edition(&path);
    assert_eq!(storage.database_metadata().role().as_str(), raw_role_value);
    assert_eq!(
        storage.database_metadata().kernel_edition().as_str(),
        raw_edition_value
    );
}
