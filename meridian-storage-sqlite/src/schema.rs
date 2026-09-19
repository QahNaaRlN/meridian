//! The one supported SQLite schema version and its bootstrap
//! (`meridian-rust-target-architecture.md` §4.1).
//!
//! `PRAGMA foreign_keys = ON` is applied on every open — SQLite does not
//! enforce declared `REFERENCES` constraints without it, even though the
//! constraints are always present in the schema.

use rusqlite::Connection;

use crate::open_error::OpenError;

/// The only schema version this build understands.
pub const SUPPORTED_SCHEMA_VERSION: i64 = 1;

/// All eight tables of `meridian-rust-target-architecture.md` §4.1, created
/// together with `schema_migrations` in one transaction. `WITHOUT ROWID` on
/// every record-bearing table makes the point structurally, not just by
/// convention: there is no rowid to accidentally leak as identity, because
/// none exists.
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
const SCHEMA_DDL: &str = r#"
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

/// Names of all eight tables `SCHEMA_DDL` creates, for the "all eight tables
/// exist" test to check against without duplicating the DDL's own list.
pub const TABLE_NAMES: [&str; 8] = [
    "schema_migrations",
    "records",
    "record_revisions",
    "sources",
    "evidence",
    "owner_decisions",
    "execution_runs",
    "migration_runs",
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

fn bootstrap(conn: &mut Connection) -> Result<(), OpenError> {
    let tx = conn.transaction().map_err(map_err)?;
    tx.execute_batch(SCHEMA_DDL).map_err(map_err)?;
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

/// Applies `PRAGMA foreign_keys = ON`, then either bootstraps a fresh
/// database (no `schema_migrations` table yet) or verifies an existing
/// one's version — transactionally and idempotently: calling this twice on
/// the same already-prepared database is a no-op the second time, not a
/// second migration attempt.
pub fn prepare(conn: &mut Connection) -> Result<(), OpenError> {
    conn.pragma_update(None, "foreign_keys", true)
        .map_err(map_err)?;

    if !table_exists(conn, "schema_migrations")? {
        return bootstrap(conn);
    }

    match read_schema_version(conn)? {
        None => Err(OpenError::MissingSchemaVersion),
        Some(v) if v == SUPPORTED_SCHEMA_VERSION => Ok(()),
        Some(found) => Err(OpenError::UnsupportedSchemaVersion { found }),
    }
}
