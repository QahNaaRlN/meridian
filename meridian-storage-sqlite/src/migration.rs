//! [`MigrationRepository`] over SQLite (`meridian-rust-migration-program-plan.md`
//! §5.22.4 items 3, 9–11; §5.22.5).
//!
//! Transaction model: a frozen-plan apply takes a checkpoint of the exact
//! pre-state FIRST (SQLite's online backup API — never a filesystem copy of
//! an open database file), then writes every record and the `applied`
//! journal row inside ONE immediate transaction whose first act re-checks
//! the pre-state. Any failure drops the transaction (a full rollback) and
//! removes the checkpoint it created.
//!
//! Rollback model: the journal decides admissibility
//! (`meridian_core::migration::run::check_rollback_preconditions`), the
//! checkpoint is re-hashed and re-opened read-only to prove it holds the
//! run's pre-state, and the journal is carried across the restore rather
//! than replaced by it. The checkpoint predates the run, so its journal
//! lacks the run's own `applied` fact; the checkpoint is therefore first
//! copied (backup API) into a staging database, whose journal must be
//! exactly the live journal minus that one fact. The staging copy then
//! receives the live `applied` row verbatim and a SEPARATE append-only
//! `migration_rollbacks` fact, in one transaction, and only that staged
//! database is restored into the live one. The live database is reopened
//! and re-verified (role, Kernel edition, schema, record-state digest,
//! both journal facts). No journal or revision row is ever deleted or
//! edited by a statement: the append-only triggers stay in force
//! throughout, and a restore never silently drops recorded history.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::backup::Backup;
use rusqlite::types::Value as SqlValue;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Row, TransactionBehavior, MAIN_DB};

use meridian_app::storage::{
    AppliedMigration, Checkpoint, DatabaseMetadata, MigrationApplyRequest, MigrationPortError,
    MigrationRepository, PortError, RolledBackMigration,
};
use meridian_core::migration::run::{
    check_rollback_preconditions, MigrationRun, MigrationRunId, RecordState, RollbackRefusal,
    RunRollback, RunSequence,
};
use meridian_core::types::ContentDigest;

use crate::schema;
use crate::storage::{canonical_export_on, SqliteStorage, StorageLocation};

const CHECKPOINT_EXTENSION: &str = "sqlite3";

fn sql(error: rusqlite::Error) -> MigrationPortError {
    MigrationPortError::Storage(error.to_string())
}

fn port(error: PortError) -> MigrationPortError {
    MigrationPortError::Storage(error.to_string())
}

fn checkpoint_error(message: impl Into<String>) -> MigrationPortError {
    MigrationPortError::Checkpoint(message.into())
}

/// The record state of `conn`: the digest of its canonical record export
/// and its number of stored revisions.
pub(crate) fn record_state_on(conn: &Connection) -> Result<RecordState, MigrationPortError> {
    let export = canonical_export_on(conn).map_err(port)?;
    let revisions: i64 = conn
        .query_row("SELECT COUNT(*) FROM record_revisions", [], |row| {
            row.get(0)
        })
        .map_err(sql)?;
    let revision_count = u64::try_from(revisions)
        .map_err(|_| MigrationPortError::Storage("negative revision count".to_string()))?;
    Ok(RecordState {
        digest: ContentDigest::of_bytes(export.as_bytes()),
        revision_count,
    })
}

fn digest_column(value: String, column: &str) -> rusqlite::Result<ContentDigest> {
    ContentDigest::from_hex(value).map_err(|e| corrupt(format!("{column}: {e}")))
}

fn corrupt(message: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::other(format!(
            "corrupt migration_runs row: {message}"
        ))),
    )
}

fn count_column(value: i64, column: &str) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|_| corrupt(format!("{column} is negative")))
}

/// Every run column plus the run's rollback fact, if one exists. The
/// status is never read from a column: it is the presence of the separate
/// `migration_rollbacks` fact.
const RUN_SELECT: &str = "SELECT r.*, b.restored_state_digest, b.restored_revision_count
     FROM migration_runs r LEFT JOIN migration_rollbacks b USING (migration_run_id)";

fn run_from_row(row: &Row<'_>) -> rusqlite::Result<MigrationRun> {
    let id: String = row.get("migration_run_id")?;
    let sequence: i64 = row.get("run_sequence")?;
    let restored_digest: Option<String> = row.get("restored_state_digest")?;
    let restored_count: Option<i64> = row.get("restored_revision_count")?;
    let rollback = match (restored_digest, restored_count) {
        (None, None) => None,
        (Some(digest), Some(count)) => Some(RunRollback {
            restored_state: RecordState {
                digest: digest_column(digest, "restored_state_digest")?,
                revision_count: count_column(count, "restored_revision_count")?,
            },
        }),
        _ => return Err(corrupt("a partial migration_rollbacks row".to_string())),
    };
    Ok(MigrationRun {
        id: MigrationRunId::parse(&id).map_err(|e| corrupt(e.to_string()))?,
        sequence: RunSequence::new(count_column(sequence, "run_sequence")?)
            .ok_or_else(|| corrupt("run_sequence is zero".to_string()))?,
        plan_ref: row.get("plan_ref")?,
        plan_fingerprint: digest_column(row.get("plan_fingerprint")?, "plan_fingerprint")?,
        idempotency_key: digest_column(row.get("idempotency_key")?, "idempotency_key")?,
        source_repository_ref: row.get("source_repository_ref")?,
        source_revision: row.get("source_revision")?,
        pre_state: RecordState {
            digest: digest_column(row.get("pre_state_digest")?, "pre_state_digest")?,
            revision_count: count_column(row.get("pre_revision_count")?, "pre_revision_count")?,
        },
        post_state: RecordState {
            digest: digest_column(row.get("post_state_digest")?, "post_state_digest")?,
            revision_count: count_column(row.get("post_revision_count")?, "post_revision_count")?,
        },
        checkpoint_digest: digest_column(row.get("checkpoint_digest")?, "checkpoint_digest")?,
        written_records: count_column(row.get("written_record_count")?, "written_record_count")?,
        rollback,
    })
}

fn run_by_idempotency_key_on(
    conn: &Connection,
    key: &ContentDigest,
) -> Result<Option<MigrationRun>, MigrationPortError> {
    conn.query_row(
        &format!("{RUN_SELECT} WHERE r.idempotency_key = ?1"),
        [key.value()],
        run_from_row,
    )
    .optional()
    .map_err(sql)
}

fn run_by_id_on(
    conn: &Connection,
    id: &MigrationRunId,
) -> Result<Option<MigrationRun>, MigrationPortError> {
    conn.query_row(
        &format!("{RUN_SELECT} WHERE r.migration_run_id = ?1"),
        [id.as_str()],
        run_from_row,
    )
    .optional()
    .map_err(sql)
}

fn newest_run_on(conn: &Connection) -> Result<Option<MigrationRun>, MigrationPortError> {
    conn.query_row(
        &format!("{RUN_SELECT} ORDER BY r.run_sequence DESC LIMIT 1"),
        [],
        run_from_row,
    )
    .optional()
    .map_err(sql)
}

fn as_i64(value: u64, what: &str) -> Result<i64, MigrationPortError> {
    i64::try_from(value).map_err(|_| MigrationPortError::Storage(format!("{what} overflows")))
}

/// Appends the `applied` fact of `run`.
fn insert_run(conn: &Connection, run: &MigrationRun) -> Result<(), MigrationPortError> {
    conn.execute(
        "INSERT INTO migration_runs (
            migration_run_id, run_sequence, plan_ref, plan_fingerprint, idempotency_key,
            source_repository_ref, source_revision, pre_state_digest, pre_revision_count,
            post_state_digest, post_revision_count, checkpoint_digest, written_record_count
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        rusqlite::params![
            run.id.as_str(),
            as_i64(run.sequence.get(), "run_sequence")?,
            run.plan_ref,
            run.plan_fingerprint.value(),
            run.idempotency_key.value(),
            run.source_repository_ref,
            run.source_revision,
            run.pre_state.digest.value(),
            as_i64(run.pre_state.revision_count, "pre_revision_count")?,
            run.post_state.digest.value(),
            as_i64(run.post_state.revision_count, "post_revision_count")?,
            run.checkpoint_digest.value(),
            as_i64(run.written_records, "written_record_count")?,
        ],
    )
    .map_err(sql)?;
    Ok(())
}

/// Appends the separate rollback fact of run `id`.
fn insert_rollback(
    conn: &Connection,
    id: &MigrationRunId,
    rollback: &RunRollback,
) -> Result<(), MigrationPortError> {
    conn.execute(
        "INSERT INTO migration_rollbacks (
            migration_run_id, restored_state_digest, restored_revision_count
        ) VALUES (?1, ?2, ?3)",
        rusqlite::params![
            id.as_str(),
            rollback.restored_state.digest.value(),
            as_i64(
                rollback.restored_state.revision_count,
                "restored_revision_count"
            )?,
        ],
    )
    .map_err(sql)?;
    Ok(())
}

/// The two journal tables, each keyed by `migration_run_id`.
const JOURNAL_TABLES: [&str; 2] = ["migration_runs", "migration_rollbacks"];

/// One journal table read verbatim: its column names and every row's
/// values (including `recorded_at`), in `migration_run_id` order.
#[derive(Debug, PartialEq)]
struct JournalTable {
    columns: Vec<String>,
    rows: Vec<Vec<SqlValue>>,
}

impl JournalTable {
    fn read(conn: &Connection, table: &str) -> Result<JournalTable, MigrationPortError> {
        let mut stmt = conn
            .prepare(&format!(
                "SELECT * FROM {table} ORDER BY migration_run_id ASC"
            ))
            .map_err(sql)?;
        let columns: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();
        let width = columns.len();
        let rows = stmt
            .query_map([], |row| {
                (0..width)
                    .map(|i| row.get::<_, SqlValue>(i))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })
            .map_err(sql)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sql)?;
        Ok(JournalTable { columns, rows })
    }

    fn key_of(&self, row: &[SqlValue]) -> Option<String> {
        let at = self.columns.iter().position(|c| c == "migration_run_id")?;
        match row.get(at)? {
            SqlValue::Text(text) => Some(text.clone()),
            _ => None,
        }
    }

    fn insert_verbatim(
        &self,
        conn: &Connection,
        table: &str,
        row: &[SqlValue],
    ) -> Result<(), MigrationPortError> {
        let placeholders: Vec<String> = (1..=self.columns.len()).map(|i| format!("?{i}")).collect();
        conn.execute(
            &format!(
                "INSERT INTO {table} ({}) VALUES ({})",
                self.columns.join(", "),
                placeholders.join(", ")
            ),
            rusqlite::params_from_iter(row.iter()),
        )
        .map_err(sql)?;
        Ok(())
    }
}

/// Both journal tables of one database, read verbatim.
fn journal_on(conn: &Connection) -> Result<[JournalTable; 2], MigrationPortError> {
    Ok([
        JournalTable::read(conn, JOURNAL_TABLES[0])?,
        JournalTable::read(conn, JOURNAL_TABLES[1])?,
    ])
}

/// Hashes the checkpoint file's exact bytes.
fn file_digest(path: &Path) -> Result<ContentDigest, MigrationPortError> {
    let bytes = fs::read(path)
        .map_err(|e| checkpoint_error(format!("cannot read {}: {e}", path.display())))?;
    Ok(ContentDigest::of_bytes(&bytes))
}

/// Opens a checkpoint strictly read-only and returns the record state it
/// holds, after checking its schema, role and Kernel edition.
fn checkpoint_state(
    path: &Path,
    metadata: &DatabaseMetadata,
) -> Result<RecordState, MigrationPortError> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| checkpoint_error(format!("cannot open {}: {e}", path.display())))?;
    schema::verify_read_only(&conn, metadata)
        .map_err(|e| checkpoint_error(format!("{}: {e}", path.display())))?;
    record_state_on(&conn)
}

/// Writes a consistent copy of `conn` to `final_path` through SQLite's
/// online backup API (into a `.partial` file first, renamed only once it is
/// complete), then proves the copy holds exactly `expected`.
fn write_checkpoint(
    conn: &Connection,
    final_path: &Path,
    metadata: &DatabaseMetadata,
    expected: &RecordState,
) -> Result<ContentDigest, MigrationPortError> {
    let partial = final_path.with_extension(format!("{CHECKPOINT_EXTENSION}.partial"));
    let _ = fs::remove_file(&partial);
    let result = (|| {
        {
            let mut destination = Connection::open(&partial).map_err(|e| {
                checkpoint_error(format!("cannot create {}: {e}", partial.display()))
            })?;
            let backup = Backup::new(conn, &mut destination)
                .map_err(|e| checkpoint_error(format!("cannot start backup: {e}")))?;
            backup
                .run_to_completion(256, Duration::ZERO, None)
                .map_err(|e| checkpoint_error(format!("backup failed: {e}")))?;
        }
        let held = checkpoint_state(&partial, metadata)?;
        if held != *expected {
            return Err(checkpoint_error(
                "the written checkpoint does not hold the exact pre-state",
            ));
        }
        fs::rename(&partial, final_path)
            .map_err(|e| checkpoint_error(format!("cannot seal {}: {e}", final_path.display())))?;
        file_digest(final_path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&partial);
        let _ = fs::remove_file(final_path);
    }
    result
}

/// Restores `checkpoint_path` into the live database through the backup
/// API, then replaces the connection with a freshly reopened and
/// re-verified one.
fn restore_into(
    conn: &mut Connection,
    checkpoint_path: &Path,
    location: &StorageLocation,
    metadata: &DatabaseMetadata,
) -> Result<RecordState, MigrationPortError> {
    conn.restore(
        MAIN_DB,
        checkpoint_path,
        None::<fn(rusqlite::backup::Progress)>,
    )
    .map_err(|e| checkpoint_error(format!("restore failed: {e}")))?;
    let mut reopened = Connection::open(&location.db_path)
        .map_err(|e| checkpoint_error(format!("cannot reopen the restored database: {e}")))?;
    schema::prepare(&mut reopened, metadata)
        .map_err(|e| checkpoint_error(format!("the restored database does not verify: {e}")))?;
    let state = record_state_on(&reopened)?;
    *conn = reopened;
    Ok(state)
}

/// Turns on foreign-key enforcement for `conn` (a per-connection setting
/// SQLite leaves off by default) and proves it took effect.
fn enforce_foreign_keys(conn: &Connection) -> Result<(), MigrationPortError> {
    conn.execute_batch("PRAGMA foreign_keys = ON")
        .map_err(sql)?;
    let enabled: i64 = conn
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .map_err(sql)?;
    if enabled == 1 {
        Ok(())
    } else {
        Err(MigrationPortError::Storage(
            "foreign-key enforcement cannot be enabled".to_string(),
        ))
    }
}

/// Every foreign-key violation `PRAGMA foreign_key_check` reports, as
/// `<table> row <rowid> -> <parent>`, in SQLite's report order.
fn foreign_key_violations(conn: &Connection) -> Result<Vec<String>, MigrationPortError> {
    let mut stmt = conn.prepare("PRAGMA foreign_key_check").map_err(sql)?;
    let rows = stmt
        .query_map([], |row| {
            let table: String = row.get(0)?;
            let rowid: Option<i64> = row.get(1)?;
            let parent: String = row.get(2)?;
            Ok(match rowid {
                Some(rowid) => format!("{table} row {rowid} -> {parent}"),
                None => format!("{table} -> {parent}"),
            })
        })
        .map_err(sql)?;
    rows.collect::<rusqlite::Result<Vec<_>>>().map_err(sql)
}

/// Builds the database a rollback restores: a backup-API copy of the run's
/// checkpoint whose journal is proven to be exactly the live journal minus
/// the run's own `applied` fact, which is then appended verbatim together
/// with the separate rollback fact in one transaction. A journal that does
/// not continue the live one refuses the rollback before the live database
/// is touched. The staging connection enforces foreign keys during its
/// write, and the complete staged database must pass
/// `PRAGMA foreign_key_check` before it may be restored.
fn stage_rollback(
    live: &Connection,
    checkpoint: &Path,
    staging: &Path,
    run: &MigrationRun,
    rollback: &RunRollback,
    metadata: &DatabaseMetadata,
) -> Result<(), MigrationPortError> {
    let _ = fs::remove_file(staging);
    let source = Connection::open_with_flags(
        checkpoint,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| checkpoint_error(format!("cannot open {}: {e}", checkpoint.display())))?;
    let mut staged = Connection::open(staging)
        .map_err(|e| checkpoint_error(format!("cannot create {}: {e}", staging.display())))?;
    Backup::new(&source, &mut staged)
        .and_then(|backup| backup.run_to_completion(256, Duration::ZERO, None))
        .map_err(|e| checkpoint_error(format!("cannot stage the checkpoint: {e}")))?;
    drop(source);
    schema::verify_read_only(&staged, metadata)
        .map_err(|e| checkpoint_error(format!("{}: {e}", staging.display())))?;

    let [live_runs, live_rollbacks] = journal_on(live)?;
    let [staged_runs, staged_rollbacks] = journal_on(&staged)?;
    let own = run.id.as_str();
    let mut predecessors = live_runs.rows.clone();
    let own_row = predecessors
        .iter()
        .position(|row| live_runs.key_of(row).as_deref() == Some(own))
        .map(|at| predecessors.remove(at));
    let discontinuity = |reason: &str| {
        MigrationPortError::RollbackRefused(RollbackRefusal::CheckpointStateMismatch {
            run: run.id.clone(),
            reason: reason.to_string(),
        })
    };
    let Some(own_row) = own_row else {
        return Err(discontinuity(
            "the live journal does not hold the run's applied fact",
        ));
    };
    if staged_runs.columns != live_runs.columns
        || staged_rollbacks.columns != live_rollbacks.columns
        || staged_runs.rows != predecessors
        || staged_rollbacks.rows != live_rollbacks.rows
    {
        return Err(discontinuity(
            "its migration journal is not the live journal as it stood before this run",
        ));
    }

    enforce_foreign_keys(&staged)?;
    let tx = staged.transaction().map_err(sql)?;
    live_runs.insert_verbatim(&tx, JOURNAL_TABLES[0], &own_row)?;
    insert_rollback(&tx, &run.id, rollback)?;
    tx.commit().map_err(sql)?;
    let violations = foreign_key_violations(&staged)?;
    if !violations.is_empty() {
        return Err(discontinuity(&format!(
            "the staged database violates a foreign key: {}",
            violations.join("; ")
        )));
    }
    if record_state_on(&staged)? != run.pre_state {
        return Err(discontinuity(
            "the staged copy does not hold the run's pre-state",
        ));
    }
    Ok(())
}

impl SqliteStorage {
    fn writable_location(&self) -> Result<(&StorageLocation, PathBuf), MigrationPortError> {
        let location = self
            .location
            .as_ref()
            .ok_or(MigrationPortError::CheckpointsUnavailable)?;
        if location.read_only {
            return Err(MigrationPortError::Storage(
                "the database was opened read-only".to_string(),
            ));
        }
        let dir = location
            .checkpoint_dir
            .clone()
            .ok_or(MigrationPortError::CheckpointsUnavailable)?;
        fs::create_dir_all(&dir)
            .map_err(|e| checkpoint_error(format!("cannot create {}: {e}", dir.display())))?;
        Ok((location, dir))
    }

    fn checkpoint_path(dir: &Path, name: &str) -> PathBuf {
        dir.join(format!("{name}.{CHECKPOINT_EXTENSION}"))
    }
}

impl MigrationRepository for SqliteStorage {
    fn record_state(&self) -> Result<RecordState, MigrationPortError> {
        let conn = self.lock().map_err(port)?;
        record_state_on(&conn)
    }

    fn run_by_idempotency_key(
        &self,
        idempotency_key: &ContentDigest,
    ) -> Result<Option<MigrationRun>, MigrationPortError> {
        let conn = self.lock().map_err(port)?;
        run_by_idempotency_key_on(&conn, idempotency_key)
    }

    fn run(&self, id: &MigrationRunId) -> Result<Option<MigrationRun>, MigrationPortError> {
        let conn = self.lock().map_err(port)?;
        run_by_id_on(&conn, id)
    }

    fn newest_run(&self) -> Result<Option<MigrationRun>, MigrationPortError> {
        let conn = self.lock().map_err(port)?;
        newest_run_on(&conn)
    }

    fn apply_migration(
        &self,
        request: MigrationApplyRequest,
    ) -> Result<AppliedMigration, MigrationPortError> {
        let (_, dir) = self.writable_location()?;
        let mut conn = self.lock().map_err(port)?;
        if let Some(existing) = run_by_idempotency_key_on(&conn, &request.idempotency_key)? {
            return Err(MigrationPortError::RunAlreadyRecorded { run: existing.id });
        }
        let pre_state = record_state_on(&conn)?;
        let sequence = RunSequence::after(newest_run_on(&conn)?.map(|r| r.sequence));
        let id = MigrationRunId::for_run(sequence, &request.plan_fingerprint);
        let checkpoint = Self::checkpoint_path(&dir, id.as_str());
        let checkpoint_digest = write_checkpoint(&conn, &checkpoint, &self.metadata, &pre_state)?;

        let role = self.metadata.role();
        let result = (|| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(sql)?;
            if record_state_on(&tx)? != pre_state {
                return Err(MigrationPortError::ConcurrentChange);
            }
            let mut outcomes = Vec::with_capacity(request.records.len());
            for record in &request.records {
                outcomes.push(SqliteStorage::apply_one(&tx, record, role)?);
            }
            let post_state = record_state_on(&tx)?;
            let run = MigrationRun {
                id: id.clone(),
                sequence,
                plan_ref: request.plan_ref.clone(),
                plan_fingerprint: request.plan_fingerprint.clone(),
                idempotency_key: request.idempotency_key.clone(),
                source_repository_ref: request.source_repository_ref.clone(),
                source_revision: request.source_revision.clone(),
                pre_state: pre_state.clone(),
                post_state,
                checkpoint_digest: checkpoint_digest.clone(),
                written_records: u64::try_from(outcomes.len()).map_err(|_| {
                    MigrationPortError::Storage("record count overflows".to_string())
                })?,
                rollback: None,
            };
            insert_run(&tx, &run)?;
            tx.commit().map_err(sql)?;
            Ok(AppliedMigration { run, outcomes })
        })();
        if result.is_err() {
            let _ = fs::remove_file(&checkpoint);
        }
        result
    }

    fn rollback_migration(
        &self,
        run_id: &MigrationRunId,
        plan_fingerprint: &ContentDigest,
    ) -> Result<RolledBackMigration, MigrationPortError> {
        let (location, dir) = self.writable_location()?;
        let mut conn = self.lock().map_err(port)?;
        let refused = MigrationPortError::RollbackRefused;
        let run = run_by_id_on(&conn, run_id)?.ok_or_else(|| {
            refused(RollbackRefusal::UnknownRun {
                run: run_id.as_str().to_string(),
            })
        })?;
        // The newest recorded run, rolled back or not: a run is rolled back
        // only while nothing was recorded after it. `AlreadyRolledBack` of
        // the requested run itself is still decided first.
        let newest = newest_run_on(&conn)?
            .map(|r| r.id)
            .unwrap_or_else(|| run.id.clone());
        let current = record_state_on(&conn)?;
        check_rollback_preconditions(&run, plan_fingerprint, &newest, &current).map_err(refused)?;

        let checkpoint = Self::checkpoint_path(&dir, run.id.as_str());
        if !checkpoint.is_file() {
            return Err(refused(RollbackRefusal::CheckpointMissing {
                run: run.id.clone(),
            }));
        }
        let found = file_digest(&checkpoint)?;
        if found != run.checkpoint_digest {
            return Err(refused(RollbackRefusal::CheckpointDigestMismatch {
                run: run.id.clone(),
                expected: run.checkpoint_digest.clone(),
                found,
            }));
        }
        let held = checkpoint_state(&checkpoint, &self.metadata).map_err(|e| {
            refused(RollbackRefusal::CheckpointStateMismatch {
                run: run.id.clone(),
                reason: e.to_string(),
            })
        })?;
        if held != run.pre_state {
            return Err(refused(RollbackRefusal::CheckpointStateMismatch {
                run: run.id.clone(),
                reason: format!(
                    "it holds record state {} ({} revision(s)), the run's pre-state is {} ({} revision(s))",
                    held.digest.value(),
                    held.revision_count,
                    run.pre_state.digest.value(),
                    run.pre_state.revision_count
                ),
            }));
        }

        let rollback = RunRollback {
            restored_state: run.pre_state.clone(),
        };
        let staging = checkpoint.with_extension(format!("{CHECKPOINT_EXTENSION}.rollback"));
        let restored = (|| {
            stage_rollback(
                &conn,
                &checkpoint,
                &staging,
                &run,
                &rollback,
                &self.metadata,
            )?;
            restore_into(&mut conn, &staging, location, &self.metadata)
        })();
        let _ = fs::remove_file(&staging);
        let restored = restored?;
        if restored != run.pre_state {
            return Err(refused(RollbackRefusal::RestoredStateMismatch {
                run: run.id.clone(),
                expected: run.pre_state.digest.clone(),
                found: restored.digest,
            }));
        }
        let recorded = run_by_id_on(&conn, &run.id)?;
        let expected = MigrationRun {
            rollback: Some(rollback),
            ..run
        };
        if recorded.as_ref() != Some(&expected) {
            return Err(MigrationPortError::Storage(format!(
                "after restoring migration run \"{}\" its journal does not hold both the applied and the rolled-back fact",
                expected.id
            )));
        }
        Ok(RolledBackMigration {
            run: expected,
            restored,
        })
    }

    fn checkpoint(&self) -> Result<Checkpoint, MigrationPortError> {
        let (_, dir) = self.writable_location()?;
        let conn = self.lock().map_err(port)?;
        let state = record_state_on(&conn)?;
        let short: String = state.digest.value().chars().take(16).collect();
        let name = format!("import-{short}-{}", state.revision_count);
        let path = Self::checkpoint_path(&dir, &name);
        let digest = write_checkpoint(&conn, &path, &self.metadata, &state)?;
        Ok(Checkpoint::new(name, digest, state))
    }

    fn restore(&self, checkpoint: &Checkpoint) -> Result<RecordState, MigrationPortError> {
        let (location, dir) = self.writable_location()?;
        let path = Self::checkpoint_path(&dir, checkpoint.name());
        if file_digest(&path)? != *checkpoint.digest() {
            return Err(checkpoint_error(format!(
                "checkpoint {} no longer has its recorded digest",
                checkpoint.name()
            )));
        }
        if checkpoint_state(&path, &self.metadata)? != *checkpoint.state() {
            return Err(checkpoint_error(format!(
                "checkpoint {} no longer holds its recorded state",
                checkpoint.name()
            )));
        }
        let mut conn = self.lock().map_err(port)?;
        let restored = restore_into(&mut conn, &path, location, &self.metadata)?;
        if restored != *checkpoint.state() {
            return Err(checkpoint_error(format!(
                "after restoring {} the record state is {}, not {}",
                checkpoint.name(),
                restored.digest.value(),
                checkpoint.state().digest.value()
            )));
        }
        Ok(restored)
    }

    fn discard(&self, checkpoint: Checkpoint) -> Result<(), MigrationPortError> {
        let (_, dir) = self.writable_location()?;
        let path = Self::checkpoint_path(&dir, checkpoint.name());
        fs::remove_file(&path)
            .map_err(|e| checkpoint_error(format!("cannot remove {}: {e}", path.display())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use meridian_app::storage::DatabaseRole;
    use meridian_core::types::Revision;

    struct Dir(PathBuf);

    impl Dir {
        fn new(label: &str) -> Dir {
            let dir = std::env::temp_dir().join(format!(
                "meridian-storage-staging-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or_default()
            ));
            fs::create_dir_all(&dir).unwrap();
            Dir(dir)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn applied(dir: &Dir) -> (SqliteStorage, MigrationRun, PathBuf) {
        let metadata =
            DatabaseMetadata::new(DatabaseRole::Workspace, Revision::new("0.6.0").unwrap());
        let storage = SqliteStorage::open_path(dir.0.join("workspace.sqlite3"), metadata)
            .unwrap()
            .with_checkpoint_dir(dir.0.join("checkpoints"));
        let fingerprint = ContentDigest::of_str("plan");
        let run = storage
            .apply_migration(MigrationApplyRequest {
                plan_ref: "plan".to_string(),
                plan_fingerprint: fingerprint,
                idempotency_key: ContentDigest::of_str("key"),
                source_repository_ref: "repo".to_string(),
                source_revision: "rev".to_string(),
                records: Vec::new(),
            })
            .unwrap()
            .run;
        let checkpoint = dir
            .0
            .join("checkpoints")
            .join(format!("{}.sqlite3", run.id));
        (storage, run, checkpoint)
    }

    #[test]
    fn migration_staging_enforces_foreign_keys_and_refuses_a_violating_copy() {
        let dir = Dir::new("fk");
        let (storage, run, checkpoint) = applied(&dir);
        let rollback = RunRollback {
            restored_state: run.pre_state.clone(),
        };

        // A connection whose enforcement is off can write an orphan that
        // only `foreign_key_check` then reveals; once enforced, it cannot.
        let probe = Connection::open(dir.0.join("probe.sqlite3")).unwrap();
        probe
            .execute_batch(
                "PRAGMA foreign_keys = OFF;
                 CREATE TABLE parent (id TEXT PRIMARY KEY);
                 CREATE TABLE child (id TEXT REFERENCES parent(id));",
            )
            .unwrap();
        probe
            .execute("INSERT INTO child VALUES ('orphan-1')", [])
            .unwrap();
        assert_eq!(foreign_key_violations(&probe).unwrap().len(), 1);
        enforce_foreign_keys(&probe).unwrap();
        assert!(probe
            .execute("INSERT INTO child VALUES ('orphan-2')", [])
            .is_err());

        // A checkpoint copy that carries a foreign-key violation written
        // while enforcement was off is refused before any restore, even
        // though its journal and record state are otherwise exact.
        let crafted = dir.0.join("crafted.sqlite3");
        fs::copy(&checkpoint, &crafted).unwrap();
        let writer = Connection::open(&crafted).unwrap();
        writer.execute_batch("PRAGMA foreign_keys = OFF").unwrap();
        writer
            .execute(
                "INSERT INTO evidence (evidence_ref, subject_record_key, summary, payload)
                 VALUES ('orphan', 'no-such-record', 'orphan evidence', '{}')",
                [],
            )
            .unwrap();
        drop(writer);
        let conn = storage.lock().unwrap();
        let staging = dir.0.join("staging.sqlite3");
        let err = stage_rollback(
            &conn,
            &crafted,
            &staging,
            &run,
            &rollback,
            &storage.metadata,
        )
        .unwrap_err();
        match err {
            MigrationPortError::RollbackRefused(RollbackRefusal::CheckpointStateMismatch {
                reason,
                ..
            }) => assert!(
                reason.contains("foreign key") && reason.contains("evidence"),
                "{reason}"
            ),
            other => panic!("{other:?}"),
        }

        // The run's own checkpoint stages cleanly, with no violation.
        stage_rollback(
            &conn,
            &checkpoint,
            &staging,
            &run,
            &rollback,
            &storage.metadata,
        )
        .unwrap();
        let staged = Connection::open(&staging).unwrap();
        assert!(foreign_key_violations(&staged).unwrap().is_empty());
        assert_eq!(
            staged
                .query_row("SELECT COUNT(*) FROM migration_rollbacks", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
}
