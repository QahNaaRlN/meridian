//! `MigrationRepository` over SQLite (package `meridian-cli-migration`,
//! `meridian-rust-migration-program-plan.md` §5.22.4–§5.22.5): one
//! transaction per apply, a verified checkpoint per run, a rollback that
//! restores the exact pre-state and refuses every unsafe state unchanged.

use std::path::{Path, PathBuf};

use meridian_app::storage::{
    DatabaseMetadata, DatabaseRole, IdempotencyKey, MigrationApplyRequest, MigrationPortError,
    MigrationRepository, Payload, PutRecordRequest, RecordKey, RecordRepository,
    RecordSchemaVersion, SchemaRef,
};
use meridian_core::migration::run::{MigrationRunId, RollbackRefusal, RunRollback, RunStatus};
use meridian_core::types::{
    Authority, AuthorityKind, ContentDigest, NonEmptyString, Origin, Revision, Scope, SemanticId,
};
use meridian_storage_sqlite::SqliteStorage;

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "meridian-storage-migration-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
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

fn metadata() -> DatabaseMetadata {
    DatabaseMetadata::new(DatabaseRole::Workspace, Revision::new("0.6.0").unwrap())
}

fn open(dir: &TempDir) -> SqliteStorage {
    SqliteStorage::open_path(dir.join("workspace.sqlite3"), metadata())
        .unwrap()
        .with_checkpoint_dir(dir.join("checkpoints"))
}

fn sid(s: &str) -> SemanticId {
    SemanticId::new(s).unwrap()
}

fn key(id: &str) -> RecordKey {
    RecordKey::new(
        Scope::project_workspace(sid("sample-project"), None),
        sid(id),
    )
}

fn put(id: &str, body: &str, idempotency: &str) -> PutRecordRequest {
    PutRecordRequest::new(
        key(id),
        SchemaRef::new("registries/operating-model/scoped-record.schema.json").unwrap(),
        RecordSchemaVersion::CURRENT,
        NonEmptyString::new(format!("Record {id}")).unwrap(),
        sid("workspace-file"),
        Origin::migrated(format!("record-unit:{id}")).unwrap(),
        Authority::new(
            AuthorityKind::ProjectOwner,
            "workspace-owner",
            Some("owner-decision:sample".to_string()),
        )
        .unwrap(),
        Payload::new(serde_json::json!({ "content": body })).unwrap(),
        IdempotencyKey::new(idempotency).unwrap(),
    )
    .unwrap()
}

fn fingerprint(label: &str) -> ContentDigest {
    ContentDigest::of_str(label)
}

fn apply_request(label: &str, ids: &[&str]) -> MigrationApplyRequest {
    MigrationApplyRequest {
        plan_ref: "sample-plan".to_string(),
        plan_fingerprint: fingerprint(label),
        idempotency_key: fingerprint(&format!("key-{label}")),
        source_repository_ref: "sample-repository".to_string(),
        source_revision: "0123456789abcdef0123456789abcdef01234567".to_string(),
        records: ids
            .iter()
            .map(|id| put(id, &format!("body of {id}"), &format!("{label}:{id}")))
            .collect(),
    }
}

fn checkpoint_files(dir: &TempDir) -> Vec<String> {
    let path = dir.join("checkpoints");
    let Ok(entries) = std::fs::read_dir(&path) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

/// Every row of a journal table, every column verbatim (including
/// `recorded_at`), in `migration_run_id` order.
fn raw_rows(path: &Path, table: &str) -> Vec<Vec<rusqlite::types::Value>> {
    let conn = rusqlite::Connection::open(path).unwrap();
    let mut stmt = conn
        .prepare(&format!("SELECT * FROM {table} ORDER BY migration_run_id"))
        .unwrap();
    let width = stmt.column_count();
    stmt.query_map([], |row| {
        (0..width)
            .map(|i| row.get::<_, rusqlite::types::Value>(i))
            .collect::<rusqlite::Result<Vec<_>>>()
    })
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

fn text(value: &rusqlite::types::Value) -> &str {
    match value {
        rusqlite::types::Value::Text(t) => t,
        other => panic!("not text: {other:?}"),
    }
}

fn raw_count(path: &Path, table: &str) -> i64 {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
        row.get(0)
    })
    .unwrap()
}

#[test]
fn migration_apply_writes_every_record_and_the_journal_row_together() {
    let dir = TempDir::new("apply");
    let storage = open(&dir);
    let pre = storage.record_state().unwrap();
    let applied = storage
        .apply_migration(apply_request("one", &["a", "b", "c"]))
        .unwrap();
    assert_eq!(applied.outcomes.len(), 3);
    assert_eq!(applied.run.status(), RunStatus::Applied);
    assert_eq!(applied.run.rollback, None);
    assert_eq!(applied.run.pre_state, pre);
    assert_eq!(applied.run.post_state, storage.record_state().unwrap());
    assert_eq!(applied.run.post_state.revision_count, 3);
    assert_eq!(applied.run.written_records, 3);
    assert_eq!(
        checkpoint_files(&dir),
        vec![format!("{}.sqlite3", applied.run.id)]
    );
    assert_eq!(
        storage
            .run_by_idempotency_key(&applied.run.idempotency_key)
            .unwrap(),
        Some(applied.run.clone())
    );
    assert_eq!(storage.newest_run().unwrap(), Some(applied.run));
}

#[test]
fn migration_a_repeated_apply_of_the_same_key_writes_nothing() {
    let dir = TempDir::new("repeat");
    let storage = open(&dir);
    let first = storage
        .apply_migration(apply_request("one", &["a"]))
        .unwrap();
    let state = storage.record_state().unwrap();
    let err = storage
        .apply_migration(apply_request("one", &["a"]))
        .unwrap_err();
    assert_eq!(
        err,
        MigrationPortError::RunAlreadyRecorded { run: first.run.id }
    );
    assert_eq!(storage.record_state().unwrap(), state);
    assert_eq!(checkpoint_files(&dir).len(), 1);
}

#[test]
fn migration_a_failure_in_the_middle_of_the_batch_leaves_the_exact_pre_state() {
    let dir = TempDir::new("mid-batch");
    let db = dir.join("workspace.sqlite3");
    let storage = open(&dir);
    storage.put(put("seed", "seed", "seed")).unwrap();
    let pre = storage.record_state().unwrap();
    let export_before = storage.canonical_export_json().unwrap();
    drop(storage);
    {
        let conn = rusqlite::Connection::open(&db).unwrap();
        conn.execute_batch(
            "CREATE TRIGGER injected_failure BEFORE INSERT ON records
             WHEN NEW.record_id = 'e'
             BEGIN SELECT RAISE(ABORT, 'injected failure'); END;",
        )
        .unwrap();
    }
    let storage = open(&dir);
    let err = storage
        .apply_migration(apply_request("one", &["a", "b", "c", "d", "e", "f", "g"]))
        .unwrap_err();
    assert!(matches!(err, MigrationPortError::Record(_)), "{err:?}");
    assert_eq!(storage.record_state().unwrap(), pre);
    assert_eq!(storage.canonical_export_json().unwrap(), export_before);
    assert_eq!(raw_count(&db, "migration_runs"), 0);
    assert_eq!(raw_count(&db, "record_revisions"), 1);
    assert!(
        checkpoint_files(&dir).is_empty(),
        "{:?}",
        checkpoint_files(&dir)
    );
}

#[test]
fn migration_rollback_restores_the_exact_pre_state_and_keeps_both_journal_facts() {
    let dir = TempDir::new("rollback");
    let db = dir.join("workspace.sqlite3");
    let storage = open(&dir);
    storage.put(put("seed", "seed", "seed")).unwrap();
    let export_before = storage.canonical_export_json().unwrap();
    let pre = storage.record_state().unwrap();
    let applied = storage
        .apply_migration(apply_request("one", &["a", "b"]))
        .unwrap();
    assert_ne!(storage.canonical_export_json().unwrap(), export_before);
    // The applied fact exactly as the journal holds it before the rollback.
    let applied_rows = raw_rows(&db, "migration_runs");
    assert_eq!(applied_rows.len(), 1);
    assert!(raw_rows(&db, "migration_rollbacks").is_empty());

    let rolled = storage
        .rollback_migration(&applied.run.id, &applied.run.plan_fingerprint)
        .unwrap();
    assert_eq!(rolled.restored, pre);
    assert_eq!(rolled.run.status(), RunStatus::RolledBack);
    assert_eq!(
        rolled.run.rollback,
        Some(RunRollback {
            restored_state: pre.clone()
        })
    );
    // The applied part of the run is unchanged by its rollback.
    assert_eq!(
        meridian_core::migration::run::MigrationRun {
            rollback: None,
            ..rolled.run.clone()
        },
        applied.run
    );
    assert_eq!(storage.canonical_export_json().unwrap(), export_before);
    assert_eq!(storage.schema_version().unwrap(), 3);
    drop(storage);

    // After reopening: the restored records, and BOTH journal facts — the
    // original applied row byte for byte (the checkpoint predates it, so a
    // plain restore would have dropped it) and one separate rollback row.
    let reopened = open(&dir);
    assert_eq!(reopened.canonical_export_json().unwrap(), export_before);
    assert_eq!(reopened.database_metadata(), &metadata());
    assert_eq!(raw_count(&db, "record_revisions"), 1);
    assert_eq!(raw_rows(&db, "migration_runs"), applied_rows);
    let rollbacks = raw_rows(&db, "migration_rollbacks");
    assert_eq!(rollbacks.len(), 1);
    assert_eq!(text(&rollbacks[0][0]), applied.run.id.as_str());
    assert_eq!(text(&rollbacks[0][1]), pre.digest.value());
    assert_eq!(
        rollbacks[0][2],
        rusqlite::types::Value::Integer(pre.revision_count as i64)
    );
    assert_eq!(reopened.run(&applied.run.id).unwrap(), Some(rolled.run));
    // No staging file is left beside the run's checkpoint.
    assert_eq!(
        checkpoint_files(&dir),
        vec![format!("{}.sqlite3", applied.run.id)]
    );
}

#[test]
fn migration_a_rollback_carries_every_earlier_journal_fact_across_the_restore() {
    let dir = TempDir::new("rollback-history");
    let db = dir.join("workspace.sqlite3");
    let storage = open(&dir);
    let first = storage
        .apply_migration(apply_request("one", &["a"]))
        .unwrap();
    storage
        .rollback_migration(&first.run.id, &first.run.plan_fingerprint)
        .unwrap();
    let second = storage
        .apply_migration(apply_request("two", &["b"]))
        .unwrap();
    let runs_before = raw_rows(&db, "migration_runs");
    let rollbacks_before = raw_rows(&db, "migration_rollbacks");
    assert_eq!((runs_before.len(), rollbacks_before.len()), (2, 1));

    storage
        .rollback_migration(&second.run.id, &second.run.plan_fingerprint)
        .unwrap();
    drop(storage);
    assert_eq!(raw_rows(&db, "migration_runs"), runs_before);
    let rollbacks = raw_rows(&db, "migration_rollbacks");
    assert_eq!(rollbacks.len(), 2);
    assert_eq!(rollbacks[0], rollbacks_before[0]);
    let ids: Vec<&str> = rollbacks.iter().map(|r| text(&r[0])).collect();
    let mut expected = vec![first.run.id.as_str(), second.run.id.as_str()];
    expected.sort();
    assert_eq!(ids, expected);
    let reopened = open(&dir);
    for run in [&first.run, &second.run] {
        assert_eq!(
            reopened.run(&run.id).unwrap().unwrap().status(),
            RunStatus::RolledBack
        );
    }
}

#[test]
fn migration_a_second_rollback_is_refused_without_change() {
    let dir = TempDir::new("rollback-twice");
    let storage = open(&dir);
    let applied = storage
        .apply_migration(apply_request("one", &["a"]))
        .unwrap();
    storage
        .rollback_migration(&applied.run.id, &applied.run.plan_fingerprint)
        .unwrap();
    let state = storage.record_state().unwrap();
    let err = storage
        .rollback_migration(&applied.run.id, &applied.run.plan_fingerprint)
        .unwrap_err();
    assert!(matches!(
        err,
        MigrationPortError::RollbackRefused(RollbackRefusal::AlreadyRolledBack { .. })
    ));
    assert_eq!(storage.record_state().unwrap(), state);
    // The same plan is never silently re-applied after its rollback.
    assert!(matches!(
        storage.apply_migration(apply_request("one", &["a"])),
        Err(MigrationPortError::RunAlreadyRecorded { .. })
    ));
}

#[test]
fn migration_rollback_over_a_newer_revision_is_refused_without_change() {
    let dir = TempDir::new("rollback-newer");
    let storage = open(&dir);
    let applied = storage
        .apply_migration(apply_request("one", &["a"]))
        .unwrap();
    storage.put(put("a", "a newer body", "later-edit")).unwrap();
    let state = storage.record_state().unwrap();
    let export = storage.canonical_export_json().unwrap();
    let err = storage
        .rollback_migration(&applied.run.id, &applied.run.plan_fingerprint)
        .unwrap_err();
    assert!(matches!(
        err,
        MigrationPortError::RollbackRefused(RollbackRefusal::StateChanged { .. })
    ));
    assert_eq!(storage.record_state().unwrap(), state);
    assert_eq!(storage.canonical_export_json().unwrap(), export);
}

#[test]
fn migration_rollback_of_an_older_run_after_a_newer_one_is_refused() {
    let dir = TempDir::new("rollback-older");
    let storage = open(&dir);
    let first = storage
        .apply_migration(apply_request("one", &["a"]))
        .unwrap();
    storage
        .apply_migration(apply_request("two", &["b"]))
        .unwrap();
    let err = storage
        .rollback_migration(&first.run.id, &first.run.plan_fingerprint)
        .unwrap_err();
    assert!(matches!(
        err,
        MigrationPortError::RollbackRefused(RollbackRefusal::NewerRun { .. })
    ));
}

#[test]
fn migration_rollback_of_an_older_run_after_a_rolled_back_newer_one_is_refused() {
    let dir = TempDir::new("rollback-older-after-newer-rollback");
    let db = dir.join("workspace.sqlite3");
    let storage = open(&dir);
    let a = storage
        .apply_migration(apply_request("one", &["a"]))
        .unwrap();
    let b = storage
        .apply_migration(apply_request("two", &["b"]))
        .unwrap();
    storage
        .rollback_migration(&b.run.id, &b.run.plan_fingerprint)
        .unwrap();

    let state = storage.record_state().unwrap();
    let export = storage.canonical_export_json().unwrap();
    let runs = raw_rows(&db, "migration_runs");
    let rollbacks = raw_rows(&db, "migration_rollbacks");
    let checkpoints = checkpoint_files(&dir);
    assert_eq!((runs.len(), rollbacks.len(), checkpoints.len()), (2, 1, 2));

    // B is the newest recorded run even though it is rolled back, so A
    // cannot be rolled back past it.
    let err = storage
        .rollback_migration(&a.run.id, &a.run.plan_fingerprint)
        .unwrap_err();
    assert_eq!(
        err,
        MigrationPortError::RollbackRefused(RollbackRefusal::NewerRun {
            run: a.run.id.clone(),
            newer: b.run.id.clone(),
        })
    );
    assert_eq!(storage.record_state().unwrap(), state);
    assert_eq!(storage.canonical_export_json().unwrap(), export);
    assert_eq!(raw_rows(&db, "migration_runs"), runs);
    assert_eq!(raw_rows(&db, "migration_rollbacks"), rollbacks);
    assert_eq!(checkpoint_files(&dir), checkpoints);

    // The requested run's own AlreadyRolledBack still takes precedence.
    assert!(matches!(
        storage.rollback_migration(&b.run.id, &b.run.plan_fingerprint),
        Err(MigrationPortError::RollbackRefused(
            RollbackRefusal::AlreadyRolledBack { .. }
        ))
    ));
}

#[test]
fn migration_rollback_refuses_unknown_runs_and_foreign_plans() {
    let dir = TempDir::new("rollback-unknown");
    let storage = open(&dir);
    let applied = storage
        .apply_migration(apply_request("one", &["a"]))
        .unwrap();
    let unknown = MigrationRunId::parse("mr-9-0123456789ab").unwrap();
    assert!(matches!(
        storage.rollback_migration(&unknown, &applied.run.plan_fingerprint),
        Err(MigrationPortError::RollbackRefused(
            RollbackRefusal::UnknownRun { .. }
        ))
    ));
    assert!(matches!(
        storage.rollback_migration(&applied.run.id, &fingerprint("other")),
        Err(MigrationPortError::RollbackRefused(
            RollbackRefusal::PlanMismatch { .. }
        ))
    ));
}

#[test]
fn migration_rollback_refuses_a_damaged_or_missing_checkpoint_without_change() {
    let dir = TempDir::new("rollback-damaged");
    let storage = open(&dir);
    let applied = storage
        .apply_migration(apply_request("one", &["a"]))
        .unwrap();
    let state = storage.record_state().unwrap();
    let checkpoint = dir
        .join("checkpoints")
        .join(format!("{}.sqlite3", applied.run.id));
    let original = std::fs::read(&checkpoint).unwrap();

    let mut damaged = original.clone();
    let last = damaged.len() - 1;
    damaged[last] ^= 0xff;
    std::fs::write(&checkpoint, &damaged).unwrap();
    assert!(matches!(
        storage.rollback_migration(&applied.run.id, &applied.run.plan_fingerprint),
        Err(MigrationPortError::RollbackRefused(
            RollbackRefusal::CheckpointDigestMismatch { .. }
        ))
    ));
    assert_eq!(storage.record_state().unwrap(), state);

    std::fs::remove_file(&checkpoint).unwrap();
    assert!(matches!(
        storage.rollback_migration(&applied.run.id, &applied.run.plan_fingerprint),
        Err(MigrationPortError::RollbackRefused(
            RollbackRefusal::CheckpointMissing { .. }
        ))
    ));
    assert_eq!(storage.record_state().unwrap(), state);

    std::fs::write(&checkpoint, &original).unwrap();
    storage
        .rollback_migration(&applied.run.id, &applied.run.plan_fingerprint)
        .unwrap();
}

#[test]
fn migration_standalone_checkpoint_restores_a_compensated_database() {
    let dir = TempDir::new("standalone");
    let storage = open(&dir);
    storage.put(put("seed", "seed", "seed")).unwrap();
    let export = storage.canonical_export_json().unwrap();
    let checkpoint = storage.checkpoint().unwrap();
    storage.put(put("later", "later", "later")).unwrap();
    let restored = storage.restore(&checkpoint).unwrap();
    assert_eq!(&restored, checkpoint.state());
    assert_eq!(storage.canonical_export_json().unwrap(), export);
    storage.discard(checkpoint).unwrap();
    assert!(checkpoint_files(&dir).is_empty());
}

#[test]
fn migration_a_read_only_or_locationless_storage_takes_no_checkpoint() {
    let dir = TempDir::new("read-only");
    drop(open(&dir));
    let read_only =
        SqliteStorage::open_read_only(dir.join("workspace.sqlite3"), metadata()).unwrap();
    assert!(read_only
        .apply_migration(apply_request("one", &["a"]))
        .is_err());
    let memory = SqliteStorage::open_in_memory(metadata()).unwrap();
    assert_eq!(
        memory.checkpoint().unwrap_err(),
        MigrationPortError::CheckpointsUnavailable
    );
}
