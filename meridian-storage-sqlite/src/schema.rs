//! The SQLite schema, its bootstrap, and its migration ladder
//! (`meridian-rust-target-architecture.md` §4.1;
//! `meridian-rust-migration-program-plan.md` §5.4, item 3).
//!
//! `PRAGMA foreign_keys = ON` is applied on every open — SQLite does not
//! enforce declared `REFERENCES` constraints without it, even though the
//! constraints are always present in the schema.
//!
//! Schema evolution is sequential and atomic, not "only version 1 is
//! supported": [`prepare`] walks a fresh database straight to
//! [`SUPPORTED_SCHEMA_VERSION`] in one transaction, and walks an existing
//! older database forward one migration step at a time, each step its own
//! transaction. A step that fails rolls back that step alone (dropping a
//! `rusqlite::Transaction` without calling `commit()` rolls it back), so a
//! failed migration never leaves the schema partially applied — the
//! database is always observably at some whole, complete version, never
//! between two.

use rusqlite::{Connection, OptionalExtension, Transaction};

use meridian_app::storage::{DatabaseMetadata, DatabaseRole};
use meridian_core::types::ScopeType;

use crate::open_error::OpenError;

/// The current, highest schema version this build understands and migrates
/// existing databases up to.
pub const SUPPORTED_SCHEMA_VERSION: i64 = 2;

/// All eight record-bearing and journal tables of package
/// `sqlite-storage-adapter`, created together with `schema_migrations` in
/// one transaction on a fresh database. `WITHOUT ROWID` on every
/// record-bearing table makes "no rowid leaks as identity" structural, not
/// just conventional: there is no rowid to leak, because none exists.
///
/// Foreign key direction is chosen so that the natural write order never
/// needs a row to reference one that does not exist yet within the same
/// transaction:
/// - `record_revisions.record_key` references `records.record_key` — a
///   revision is written only after (or, for the very first revision, in
///   the same transaction immediately after) its record row exists;
/// - `evidence.subject_record_key` references `records.record_key` —
///   evidence about a record that does not exist is rejected, not stored;
/// - `records.current_revision` is a plain integer, not FK-constrained: the
///   very first `INSERT` into `records` necessarily happens before its
///   first revision row exists (both committed together, in one
///   transaction, so the two are never observably inconsistent to a
///   reader).
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

-- `authority_decision_ref` carries no `REFERENCES owner_decisions(decision_ref)`
-- constraint: this package adds no operation that registers an owner
-- decision (no third port, per architect instruction — `owner_decisions`
-- exists as a content-ful table for a later package to populate), so an FK
-- here would make any `Authority` carrying a `decision_ref` unwritable.
-- This link is therefore NOT enforced by `sqlite-storage-adapter`; treat it
-- as an opaque reference only, not a proven join, until a later package adds
-- the registration operation and the constraint together.
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

-- Same non-enforcement note as `records.authority_decision_ref` above.
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

-- Immutability is not left to application discipline alone
-- (`meridian-rust-target-architecture.md` §4.2, "Неизменяемые редакции и
-- доказательства"): a direct UPDATE or DELETE against `record_revisions` or
-- `evidence` — bypassing this crate's port methods entirely — is refused by
-- SQLite itself, not merely unsupported by the Rust API.
CREATE TRIGGER record_revisions_no_update
BEFORE UPDATE ON record_revisions
BEGIN
  SELECT RAISE(ABORT, 'record_revisions rows are immutable; write a new revision instead of editing one');
END;

CREATE TRIGGER record_revisions_no_delete
BEFORE DELETE ON record_revisions
BEGIN
  SELECT RAISE(ABORT, 'record_revisions rows are immutable and append-only; delete is not permitted');
END;

CREATE TRIGGER evidence_no_update
BEFORE UPDATE ON evidence
BEGIN
  SELECT RAISE(ABORT, 'evidence rows are immutable; a correction is new evidence, never an edit of stored evidence');
END;

CREATE TRIGGER evidence_no_delete
BEFORE DELETE ON evidence
BEGIN
  SELECT RAISE(ABORT, 'evidence rows are immutable and append-only; delete is not permitted');
END;
"#;

/// Schema version 2's one addition over version 1: a single-row table
/// carrying this database's own role and Kernel edition
/// (`meridian-rust-migration-program-plan.md` §5.4, items 1–2). The `CHECK
/// (id = 1)` is what makes "single-row" structural: a second `INSERT` can
/// only collide with the existing primary key, never silently add a second
/// row this crate would then have to pick between.
const DATABASE_METADATA_TABLE_DDL: &str = r#"
CREATE TABLE database_metadata (
  id             INTEGER NOT NULL PRIMARY KEY CHECK (id = 1),
  role           TEXT    NOT NULL,
  kernel_edition TEXT    NOT NULL
) WITHOUT ROWID;
"#;

/// Names of every table a fully-migrated (current-version) database
/// carries, for the "all tables exist" test to check against without
/// duplicating the DDL's own list.
pub const TABLE_NAMES: [&str; 9] = [
    "schema_migrations",
    "records",
    "record_revisions",
    "sources",
    "evidence",
    "owner_decisions",
    "execution_runs",
    "migration_runs",
    "database_metadata",
];

fn map_err(e: rusqlite::Error) -> OpenError {
    let message = e.to_string();
    let lower = message.to_ascii_lowercase();
    if lower.contains("file is not a database")
        || lower.contains("database disk image is malformed")
    {
        OpenError::CorruptDatabase(message)
    } else {
        OpenError::Sqlite(message)
    }
}

fn table_exists(conn: &Connection, name: &str) -> Result<bool, OpenError> {
    let mut stmt = conn
        .prepare("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1")
        .map_err(map_err)?;
    stmt.exists([name]).map_err(map_err)
}

fn read_schema_version(conn: &Connection) -> Result<Option<i64>, OpenError> {
    conn.query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
        row.get::<_, Option<i64>>(0)
    })
    .map_err(map_err)
}

fn insert_database_metadata_row(
    tx: &Transaction<'_>,
    metadata: &DatabaseMetadata,
) -> Result<(), OpenError> {
    tx.execute(
        "INSERT INTO database_metadata (id, role, kernel_edition) VALUES (1, ?1, ?2)",
        rusqlite::params![metadata.role().as_str(), metadata.kernel_edition().as_str(),],
    )
    .map_err(map_err)?;
    Ok(())
}

/// Reads back the single `database_metadata` row. Called only once the
/// caller already knows the database is at [`SUPPORTED_SCHEMA_VERSION`] —
/// an older database simply has no such table yet, which is not this
/// function's concern.
pub fn read_database_metadata(conn: &Connection) -> Result<DatabaseMetadata, OpenError> {
    let row: Option<(String, String)> = conn
        .query_row(
            "SELECT role, kernel_edition FROM database_metadata WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(map_err)?;
    let Some((role, kernel_edition)) = row else {
        return Err(OpenError::MissingDatabaseMetadata);
    };
    let role = DatabaseRole::try_from(role.as_str())
        .map_err(|_| OpenError::UnknownDatabaseRole { found: role })?;
    let kernel_edition = meridian_core::types::Revision::new(kernel_edition)
        .map_err(|e| OpenError::Sqlite(format!("corrupt database_metadata.kernel_edition: {e}")))?;
    Ok(DatabaseMetadata::new(role, kernel_edition))
}

/// Creates a brand-new database directly at [`SUPPORTED_SCHEMA_VERSION`] —
/// there is no reason to walk a fresh file through history it never had.
fn bootstrap(conn: &mut Connection, metadata: &DatabaseMetadata) -> Result<(), OpenError> {
    let tx = conn.transaction().map_err(map_err)?;
    tx.execute_batch(SCHEMA_DDL_V1).map_err(map_err)?;
    tx.execute_batch(DATABASE_METADATA_TABLE_DDL)
        .map_err(map_err)?;
    insert_database_metadata_row(&tx, metadata)?;
    tx.execute(
        "INSERT INTO schema_migrations (version) VALUES (?1)",
        [SUPPORTED_SCHEMA_VERSION],
    )
    .map_err(map_err)?;
    // Dropping `tx` without `commit()` would roll back automatically, so an
    // error above never leaves a partially created schema — this `commit()`
    // is the only path that makes the bootstrap durable.
    tx.commit().map_err(map_err)
}

/// The `ScopeType` a `records.scope_type` column value names, or `None` for
/// a value that is not one of the six known areas.
fn scope_type_from_str(raw: &str) -> Option<ScopeType> {
    match raw {
        "built-in-methodology" => Some(ScopeType::BuiltInMethodology),
        "user-profile" => Some(ScopeType::UserProfile),
        "organization-profile" => Some(ScopeType::OrganizationProfile),
        "project-workspace" => Some(ScopeType::ProjectWorkspace),
        "repository-scope" => Some(ScopeType::RepositoryScope),
        "run-state" => Some(ScopeType::RunState),
        _ => None,
    }
}

/// Checks every distinct `records.scope_type` already present against the
/// role a v1→v2 migration is about to assign, inside the migration step's
/// own transaction and before any DDL or write for this step runs
/// (`meridian-rust-migration-program-plan.md` §5.4, item 3). An
/// unrecognised scope type or one the role does not accept stops the
/// migration outright — the transaction this call is part of is never
/// committed on that path, so nothing this step would otherwise have
/// written (the `database_metadata` table, its row, the version-2 journal
/// entry) is observable afterward.
fn check_existing_records_compatible_with_role(
    tx: &Transaction<'_>,
    role: DatabaseRole,
) -> Result<(), OpenError> {
    let mut stmt = tx
        .prepare("SELECT DISTINCT scope_type FROM records")
        .map_err(map_err)?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(map_err)?;
    for row in rows {
        let raw = row.map_err(map_err)?;
        let scope_type = scope_type_from_str(&raw).ok_or_else(|| {
            OpenError::UnrecognizedScopeTypeInExistingRecords { found: raw.clone() }
        })?;
        if !role.accepts_scope_type(scope_type) {
            return Err(OpenError::MigrationRoleIncompatibleWithExistingRecords {
                role,
                scope_type: raw,
            });
        }
    }
    Ok(())
}

/// Applies exactly one migration step, atomically, and returns the version
/// it lands on. A future second step is added as a new match arm here, not
/// by rewriting this function's shape.
fn apply_migration_step(
    conn: &mut Connection,
    from_version: i64,
    metadata: &DatabaseMetadata,
) -> Result<i64, OpenError> {
    let tx = conn.transaction().map_err(map_err)?;
    let landed_on = match from_version {
        1 => {
            check_existing_records_compatible_with_role(&tx, metadata.role())?;
            tx.execute_batch(DATABASE_METADATA_TABLE_DDL)
                .map_err(map_err)?;
            insert_database_metadata_row(&tx, metadata)?;
            tx.execute("INSERT INTO schema_migrations (version) VALUES (2)", [])
                .map_err(map_err)?;
            2
        }
        other => return Err(OpenError::UnsupportedSchemaVersion { found: other }),
    };
    // As in `bootstrap`: an error above drops `tx` uncommitted, rolling
    // back this one step in full — the version row and the table it
    // describes are never observed out of step with each other.
    tx.commit().map_err(map_err)?;
    Ok(landed_on)
}

/// Walks an existing database from `current` forward to
/// [`SUPPORTED_SCHEMA_VERSION`], one step — one transaction — at a time.
fn migrate_to_current(
    conn: &mut Connection,
    mut current: i64,
    metadata: &DatabaseMetadata,
) -> Result<(), OpenError> {
    while current < SUPPORTED_SCHEMA_VERSION {
        current = apply_migration_step(conn, current, metadata)?;
    }
    Ok(())
}

/// Applies `PRAGMA foreign_keys = ON`, then either bootstraps a fresh
/// database, migrates an older one forward, or — for a database already at
/// [`SUPPORTED_SCHEMA_VERSION`] — leaves the schema untouched. In every
/// case, returns the [`DatabaseMetadata`] read back from the database
/// itself after that work — never a copy of the `metadata` argument — so a
/// caller (`crate::storage::SqliteStorage`) stores what the database
/// actually records, not what it merely asked for. Calling this twice in a
/// row with the same metadata on the same already-prepared database is a
/// complete no-op the second time, not a second migration attempt.
///
/// `metadata` is supplied by the caller, never guessed from a file path or
/// name (`meridian-rust-migration-program-plan.md` §5.4, item 3): it is the
/// only source for a fresh bootstrap's or a v1→v2 migration's role and
/// Kernel edition, because neither exists anywhere else to read at that
/// point. For a database already at [`SUPPORTED_SCHEMA_VERSION`], `metadata`
/// is instead an assertion checked against what is already recorded: role
/// and Kernel edition must both match exactly — no edition-compatibility
/// rule is defined yet, so "compatible" currently means "identical" for
/// either role — and any difference is refused
/// (`OpenError::DatabaseMetadataMismatch`) rather than silently accepted or
/// silently overwritten.
pub fn prepare(
    conn: &mut Connection,
    metadata: &DatabaseMetadata,
) -> Result<DatabaseMetadata, OpenError> {
    conn.pragma_update(None, "foreign_keys", true)
        .map_err(map_err)?;

    if !table_exists(conn, "schema_migrations")? {
        bootstrap(conn, metadata)?;
    } else {
        let version = match read_schema_version(conn)? {
            None => return Err(OpenError::MissingSchemaVersion),
            Some(v) => v,
        };

        if !(1..=SUPPORTED_SCHEMA_VERSION).contains(&version) {
            return Err(OpenError::UnsupportedSchemaVersion { found: version });
        }

        if version < SUPPORTED_SCHEMA_VERSION {
            migrate_to_current(conn, version, metadata)?;
        }
    }

    // Now at SUPPORTED_SCHEMA_VERSION, whichever of the three paths above
    // got it there (or found it already there): read back the recorded
    // metadata — the only authoritative source — and require it to match
    // the caller's assertion on both fields before returning it.
    let recorded = read_database_metadata(conn)?;
    if recorded != *metadata {
        return Err(OpenError::DatabaseMetadataMismatch {
            expected: metadata.clone(),
            found: recorded,
        });
    }
    Ok(recorded)
}
