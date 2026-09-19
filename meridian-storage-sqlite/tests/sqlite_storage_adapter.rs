//! Integration tests for package `sqlite-storage-adapter`
//! (`meridian-rust-migration-program-plan.md` §4, §5.2 predecessor,
//! `meridian-rust-target-architecture.md` §4).
//!
//! Every test opens its own file-backed database under a private temporary
//! directory, cleaned up deterministically on drop (see [`TempDir`]) —
//! nothing here leaves an artifact behind, in the repository or in a shared
//! `/tmp` location.

use std::path::PathBuf;

use meridian_app::storage::{
    IdempotencyKey, Payload, PortError, PutEvidenceRequest, PutRecordOutcome, PutRecordRequest,
    RecordKey, RecordRepository, RecordSchemaVersion, RevisionNumber, SchemaRef,
};
use meridian_core::types::{
    Authority, AuthorityKind, EvidenceRef, NonEmptyString, Origin, Scope, SemanticId, WorkspaceId,
};
use meridian_storage_sqlite::{OpenError, SqliteStorage, TABLE_NAMES};

/// A private temporary directory, removed on drop even if a test panics
/// mid-way (`Drop` still runs during unwinding) — the "guaranteed pinpoint
/// cleanup" this package's tests are required to provide, without adding a
/// new dependency for it.
struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let mut dir = std::env::temp_dir();
        let unique = format!(
            "meridian-storage-sqlite-test-{label}-{}-{}",
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
fn wid(s: &str) -> WorkspaceId {
    WorkspaceId::new(s).unwrap()
}
fn nes(s: &str) -> NonEmptyString {
    NonEmptyString::new(s).unwrap()
}
fn key(scope: Scope, id: &str) -> RecordKey {
    RecordKey::new(scope, sid(id))
}
fn ik(s: &str) -> IdempotencyKey {
    IdempotencyKey::new(s).unwrap()
}

const DEFAULT_SCHEMA: &str =
    "https://meridian.invalid/registries/operating-model/scoped-record.schema.json";

fn schema_ref(s: &str) -> SchemaRef {
    SchemaRef::new(s).unwrap()
}

#[allow(clippy::too_many_arguments)]
fn request(
    key: RecordKey,
    title: &str,
    record_type: &str,
    origin: Origin,
    authority: Authority,
    payload: serde_json::Value,
    idempotency_key: &str,
) -> PutRecordRequest {
    request_with_schema(
        DEFAULT_SCHEMA,
        key,
        title,
        record_type,
        origin,
        authority,
        payload,
        idempotency_key,
    )
}

/// Like [`request`], but with an explicit, possibly non-default `$schema` —
/// used to prove `$schema` is carried per-record, never a fixed fallback.
#[allow(clippy::too_many_arguments)]
fn request_with_schema(
    schema: &str,
    key: RecordKey,
    title: &str,
    record_type: &str,
    origin: Origin,
    authority: Authority,
    payload: serde_json::Value,
    idempotency_key: &str,
) -> PutRecordRequest {
    PutRecordRequest::new(
        key,
        schema_ref(schema),
        RecordSchemaVersion::CURRENT,
        nes(title),
        sid(record_type),
        origin,
        authority,
        Payload::new(payload).unwrap(),
        ik(idempotency_key),
    )
    .unwrap()
}

fn project_key(id: &str) -> RecordKey {
    key(Scope::project_workspace(sid("sample-project"), None), id)
}

fn project_owner() -> Authority {
    Authority::new(AuthorityKind::ProjectOwner, "workspace-owner", None).unwrap()
}

fn declared(source_ref: &str) -> Origin {
    Origin::declared(source_ref).unwrap()
}

// ---------------------------------------------------------------------------
// 1. Creation and reopening
// ---------------------------------------------------------------------------

#[test]
fn creates_and_reopens_the_same_database_without_reapplying_the_migration() {
    let dir = TempDir::new("reopen");
    let path = dir.join("kernel.sqlite3");

    {
        let storage = SqliteStorage::open_path(&path).unwrap();
        assert_eq!(storage.schema_version().unwrap(), 1);
        storage
            .put(request(
                project_key("branch-naming"),
                "Branch naming",
                "norm",
                declared("owner-decision:branch-naming"),
                project_owner(),
                serde_json::json!({}),
                "seed",
            ))
            .unwrap();
    }

    // Reopening must not re-run the bootstrap migration (which would fail on
    // `CREATE TABLE` of already-existing tables) and must still see the data
    // written before the first handle was dropped.
    let reopened = SqliteStorage::open_path(&path).unwrap();
    assert_eq!(reopened.schema_version().unwrap(), 1);
    let record = reopened
        .get(&project_key("branch-naming"))
        .unwrap()
        .unwrap();
    assert_eq!(record.title(), "Branch naming");
}

// ---------------------------------------------------------------------------
// 2. foreign_keys pragma and table/schema presence
// ---------------------------------------------------------------------------

#[test]
fn foreign_keys_pragma_is_on() {
    let __dir_fk_pragma = TempDir::new("fk-pragma");
    let storage = SqliteStorage::open_path(__dir_fk_pragma.join("db.sqlite3")).unwrap();
    assert!(storage.foreign_keys_enabled().unwrap());
}

#[test]
fn all_eight_tables_exist_and_are_queryable_and_schema_version_is_the_one_supported_version() {
    let __dir_tables = TempDir::new("tables");
    let storage = SqliteStorage::open_path(__dir_tables.join("db.sqlite3")).unwrap();
    assert_eq!(TABLE_NAMES.len(), 8);
    for table in TABLE_NAMES {
        // Every table exists and is queryable; `schema_migrations` alone is
        // never empty post-bootstrap — it carries exactly the one row
        // recording the version this database was created at.
        let expected = if table == "schema_migrations" { 1 } else { 0 };
        assert_eq!(
            storage.table_row_count(table).unwrap(),
            expected,
            "table {table} has an unexpected row count"
        );
    }
    assert_eq!(storage.schema_version().unwrap(), 1);
}

// ---------------------------------------------------------------------------
// 3. Incompatible / missing schema version, corrupt file
// ---------------------------------------------------------------------------

#[test]
fn rejects_an_unrecognised_schema_version_with_a_typed_error() {
    let dir = TempDir::new("bad-version");
    let path = dir.join("db.sqlite3");
    SqliteStorage::open_path(&path).unwrap();
    {
        let raw = rusqlite::Connection::open(&path).unwrap();
        raw.execute(
            "UPDATE schema_migrations SET version = 99 WHERE version = 1",
            [],
        )
        .unwrap();
    }
    let err = SqliteStorage::open_path(&path).unwrap_err();
    assert_eq!(err, OpenError::UnsupportedSchemaVersion { found: 99 });
}

#[test]
fn rejects_a_missing_schema_version_with_a_typed_error() {
    let dir = TempDir::new("missing-version");
    let path = dir.join("db.sqlite3");
    SqliteStorage::open_path(&path).unwrap();
    {
        let raw = rusqlite::Connection::open(&path).unwrap();
        raw.execute("DELETE FROM schema_migrations", []).unwrap();
    }
    let err = SqliteStorage::open_path(&path).unwrap_err();
    assert_eq!(err, OpenError::MissingSchemaVersion);
}

#[test]
fn rejects_a_corrupt_file_with_a_typed_error_instead_of_treating_it_as_empty() {
    let dir = TempDir::new("corrupt");
    let path = dir.join("not-a-database.sqlite3");
    std::fs::write(
        &path,
        b"this is not a sqlite database file at all, just bytes",
    )
    .unwrap();
    let err = SqliteStorage::open_path(&path).unwrap_err();
    assert!(
        matches!(err, OpenError::CorruptDatabase(_)),
        "expected CorruptDatabase, got {err:?}"
    );
    // The bogus file is left exactly as it was — never silently replaced.
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(
        bytes,
        b"this is not a sqlite database file at all, just bytes"
    );
}

// ---------------------------------------------------------------------------
// 4. Record creation and revisioning
// ---------------------------------------------------------------------------

#[test]
fn put_mints_a_new_record_at_revision_one() {
    let __dir_create = TempDir::new("create");
    let storage = SqliteStorage::open_path(__dir_create.join("db.sqlite3")).unwrap();
    let outcome = storage
        .put(request(
            project_key("branch-naming"),
            "Branch naming",
            "norm",
            declared("owner-decision:branch-naming"),
            project_owner(),
            serde_json::json!({"pattern": "^feature/"}),
            "k1",
        ))
        .unwrap();
    let PutRecordOutcome::Created(record) = outcome else {
        panic!("expected Created, got {outcome:?}")
    };
    assert_eq!(record.current_revision(), RevisionNumber::FIRST);
    assert_eq!(record.title(), "Branch naming");

    let revisions = storage
        .list_revisions(&project_key("branch-naming"))
        .unwrap();
    assert_eq!(revisions.len(), 1);
    assert_eq!(revisions[0].revision_number(), RevisionNumber::FIRST);
}

#[test]
fn a_second_put_appends_a_revision_without_changing_the_previous_one() {
    let __dir_revise = TempDir::new("revise");
    let storage = SqliteStorage::open_path(__dir_revise.join("db.sqlite3")).unwrap();
    let k = project_key("branch-naming");

    storage
        .put(request(
            k.clone(),
            "Branch naming v1",
            "norm",
            declared("owner-decision:branch-naming"),
            project_owner(),
            serde_json::json!({"pattern": "^feature/"}),
            "k1",
        ))
        .unwrap();

    let outcome = storage
        .put(request(
            k.clone(),
            "Branch naming v2",
            "norm",
            declared("owner-decision:branch-naming"),
            project_owner(),
            serde_json::json!({"pattern": "^(feature|bugfix)/"}),
            "k2",
        ))
        .unwrap();
    let PutRecordOutcome::Updated(record) = outcome else {
        panic!("expected Updated, got {outcome:?}")
    };
    assert_eq!(record.current_revision(), RevisionNumber::FIRST.next());
    assert_eq!(record.title(), "Branch naming v2");

    // The first revision is untouched.
    let rev1 = storage
        .get_revision(&k, RevisionNumber::FIRST)
        .unwrap()
        .unwrap();
    assert_eq!(rev1.title(), "Branch naming v1");

    let revisions = storage.list_revisions(&k).unwrap();
    assert_eq!(revisions.len(), 2);
    assert_eq!(revisions[0].title(), "Branch naming v1");
    assert_eq!(revisions[1].title(), "Branch naming v2");
}

// ---------------------------------------------------------------------------
// 5. Immutability enforced at the SQLite level, not just by API absence
// ---------------------------------------------------------------------------

#[test]
fn direct_update_or_delete_of_record_revisions_is_refused_by_sqlite_itself() {
    let dir = TempDir::new("immutable-revisions");
    let path = dir.join("db.sqlite3");
    let storage = SqliteStorage::open_path(&path).unwrap();
    storage
        .put(request(
            project_key("branch-naming"),
            "Branch naming",
            "norm",
            declared("owner-decision:branch-naming"),
            project_owner(),
            serde_json::json!({}),
            "k1",
        ))
        .unwrap();
    drop(storage);

    let raw = rusqlite::Connection::open(&path).unwrap();
    let update_err = raw.execute("UPDATE record_revisions SET title = 'tampered'", []);
    assert!(
        update_err.is_err(),
        "direct UPDATE of record_revisions must be refused"
    );
    let delete_err = raw.execute("DELETE FROM record_revisions", []);
    assert!(
        delete_err.is_err(),
        "direct DELETE of record_revisions must be refused"
    );

    let count: i64 = raw
        .query_row("SELECT COUNT(*) FROM record_revisions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        count, 1,
        "the row must survive both rejected attempts unchanged"
    );
}

#[test]
fn direct_update_or_delete_of_evidence_is_refused_by_sqlite_itself() {
    let dir = TempDir::new("immutable-evidence");
    let path = dir.join("db.sqlite3");
    let storage = SqliteStorage::open_path(&path).unwrap();
    let k = project_key("branch-naming");
    storage
        .put(request(
            k.clone(),
            "Branch naming",
            "norm",
            declared("owner-decision:branch-naming"),
            project_owner(),
            serde_json::json!({}),
            "k1",
        ))
        .unwrap();
    storage
        .put_evidence(PutEvidenceRequest::new(
            EvidenceRef::new("evidence:coverage-1").unwrap(),
            k,
            nes("coverage report"),
            Payload::empty(),
        ))
        .unwrap();
    drop(storage);

    let raw = rusqlite::Connection::open(&path).unwrap();
    assert!(raw
        .execute("UPDATE evidence SET summary = 'tampered'", [])
        .is_err());
    assert!(raw.execute("DELETE FROM evidence", []).is_err());
    let count: i64 = raw
        .query_row("SELECT COUNT(*) FROM evidence", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

// ---------------------------------------------------------------------------
// 6. Batch atomicity
// ---------------------------------------------------------------------------

#[test]
fn a_batch_that_fails_partway_leaves_no_trace_of_its_earlier_writes() {
    let __dir_batch_rollback = TempDir::new("batch-rollback");
    let storage = SqliteStorage::open_path(__dir_batch_rollback.join("db.sqlite3")).unwrap();

    // Seed one record under idempotency key "seed".
    storage
        .put(request(
            project_key("seeded"),
            "Seeded",
            "norm",
            declared("owner-decision:seeded"),
            project_owner(),
            serde_json::json!({}),
            "seed",
        ))
        .unwrap();

    // A batch whose second item reuses "seed" for an entirely different
    // record: a genuine conflict, not a legal repeat.
    let batch = vec![
        request(
            project_key("new-in-batch"),
            "New in batch",
            "norm",
            declared("owner-decision:new-in-batch"),
            project_owner(),
            serde_json::json!({}),
            "batch-1",
        ),
        request(
            project_key("conflicting"),
            "Conflicting",
            "norm",
            declared("owner-decision:conflicting"),
            project_owner(),
            serde_json::json!({}),
            "seed",
        ),
    ];

    let err = storage.put_batch(batch).unwrap_err();
    assert!(
        matches!(err, PortError::IdempotencyConflict { .. }),
        "expected IdempotencyConflict, got {err:?}"
    );

    // The first item of the failed batch must not have been persisted.
    assert!(storage.get(&project_key("new-in-batch")).unwrap().is_none());
    assert!(storage.get(&project_key("conflicting")).unwrap().is_none());
    // And the seed itself is untouched.
    assert_eq!(
        storage
            .get(&project_key("seeded"))
            .unwrap()
            .unwrap()
            .title(),
        "Seeded"
    );
}

// ---------------------------------------------------------------------------
// 7. Foreign key violation leaves no partial state
// ---------------------------------------------------------------------------

#[test]
fn evidence_for_a_nonexistent_record_is_rejected_and_nothing_is_written() {
    let __dir_fk_violation = TempDir::new("fk-violation");
    let storage = SqliteStorage::open_path(__dir_fk_violation.join("db.sqlite3")).unwrap();
    let missing = project_key("does-not-exist");
    let err = storage
        .put_evidence(PutEvidenceRequest::new(
            EvidenceRef::new("evidence:orphan").unwrap(),
            missing.clone(),
            nes("orphan evidence"),
            Payload::empty(),
        ))
        .unwrap_err();
    assert_eq!(
        err,
        PortError::ReferenceNotFound {
            reference: missing.storage_key()
        }
    );
    assert_eq!(storage.table_row_count("evidence").unwrap(), 0);
}

// ---------------------------------------------------------------------------
// 8. Idempotency: exact repeat vs. conflict
// ---------------------------------------------------------------------------

#[test]
fn an_exact_idempotent_repeat_creates_no_second_revision() {
    let __dir_idempotent_repeat = TempDir::new("idempotent-repeat");
    let storage = SqliteStorage::open_path(__dir_idempotent_repeat.join("db.sqlite3")).unwrap();
    let make = || {
        request(
            project_key("branch-naming"),
            "Branch naming",
            "norm",
            declared("owner-decision:branch-naming"),
            project_owner(),
            serde_json::json!({"pattern": "^feature/"}),
            "same-key",
        )
    };

    let first = storage.put(make()).unwrap();
    assert!(matches!(first, PutRecordOutcome::Created(_)));

    let second = storage.put(make()).unwrap();
    let PutRecordOutcome::AlreadyApplied(record) = second else {
        panic!("expected AlreadyApplied, got {second:?}")
    };
    assert_eq!(record.current_revision(), RevisionNumber::FIRST);

    let revisions = storage
        .list_revisions(&project_key("branch-naming"))
        .unwrap();
    assert_eq!(
        revisions.len(),
        1,
        "a repeat with unchanged content must not add a second revision"
    );
}

#[test]
fn the_same_idempotency_key_with_different_content_is_a_conflict_not_a_repeat() {
    let __dir_idempotent_conflict = TempDir::new("idempotent-conflict");
    let storage = SqliteStorage::open_path(__dir_idempotent_conflict.join("db.sqlite3")).unwrap();
    let k = project_key("branch-naming");

    storage
        .put(request(
            k.clone(),
            "Branch naming",
            "norm",
            declared("owner-decision:branch-naming"),
            project_owner(),
            serde_json::json!({"pattern": "^feature/"}),
            "same-key",
        ))
        .unwrap();

    let err = storage
        .put(request(
            k.clone(),
            "Branch naming — DIFFERENT",
            "norm",
            declared("owner-decision:branch-naming"),
            project_owner(),
            serde_json::json!({"pattern": "^feature/"}),
            "same-key",
        ))
        .unwrap_err();
    assert!(
        matches!(err, PortError::IdempotencyConflict { .. }),
        "expected IdempotencyConflict, got {err:?}"
    );

    // The original content must be unchanged and still at revision 1.
    let record = storage.get(&k).unwrap().unwrap();
    assert_eq!(record.title(), "Branch naming");
    assert_eq!(record.current_revision(), RevisionNumber::FIRST);
}

// ---------------------------------------------------------------------------
// 9. Evidence storage and resolution
// ---------------------------------------------------------------------------

#[test]
fn evidence_is_stored_and_resolves_back_to_the_same_content() {
    let __dir_evidence = TempDir::new("evidence");
    let storage = SqliteStorage::open_path(__dir_evidence.join("db.sqlite3")).unwrap();
    let k = project_key("branch-naming");
    storage
        .put(request(
            k.clone(),
            "Branch naming",
            "norm",
            declared("owner-decision:branch-naming"),
            project_owner(),
            serde_json::json!({}),
            "k1",
        ))
        .unwrap();

    let evidence_ref = EvidenceRef::new("evidence:coverage-2026-09-20").unwrap();
    storage
        .put_evidence(PutEvidenceRequest::new(
            evidence_ref.clone(),
            k.clone(),
            nes("coverage report shows the rule fires"),
            Payload::new(serde_json::json!({"runner": "conformance-harness"})).unwrap(),
        ))
        .unwrap();

    let resolved = storage.resolve_evidence(&evidence_ref).unwrap().unwrap();
    assert_eq!(resolved.summary(), "coverage report shows the rule fires");
    assert_eq!(resolved.subject(), &k);

    let never_used = EvidenceRef::new("evidence:never-recorded").unwrap();
    assert!(storage.resolve_evidence(&never_used).unwrap().is_none());
}

#[test]
fn a_second_put_for_the_same_evidence_ref_is_rejected_even_with_identical_content() {
    let __dir_evidence_writeonce = TempDir::new("evidence-writeonce");
    let storage = SqliteStorage::open_path(__dir_evidence_writeonce.join("db.sqlite3")).unwrap();
    let k = project_key("branch-naming");
    storage
        .put(request(
            k.clone(),
            "Branch naming",
            "norm",
            declared("owner-decision:branch-naming"),
            project_owner(),
            serde_json::json!({}),
            "k1",
        ))
        .unwrap();

    let make_evidence = || {
        PutEvidenceRequest::new(
            EvidenceRef::new("evidence:coverage-1").unwrap(),
            k.clone(),
            nes("coverage report"),
            Payload::empty(),
        )
    };
    storage.put_evidence(make_evidence()).unwrap();
    let err = storage.put_evidence(make_evidence()).unwrap_err();
    assert_eq!(
        err,
        PortError::EvidenceAlreadyExists {
            evidence_ref: "evidence:coverage-1".to_string()
        }
    );
}

// ---------------------------------------------------------------------------
// 10. Canonical export: deterministic, byte-stable, no internal fields
// ---------------------------------------------------------------------------

#[test]
fn canonical_export_is_byte_identical_across_repeated_calls_on_unchanged_state() {
    let __dir_export_stable = TempDir::new("export-stable");
    let storage = SqliteStorage::open_path(__dir_export_stable.join("db.sqlite3")).unwrap();
    storage
        .put(request(
            project_key("branch-naming"),
            "Branch naming",
            "norm",
            declared("owner-decision:branch-naming"),
            project_owner(),
            serde_json::json!({"z": 1, "a": 2, "m": {"y": 1, "b": 2}}),
            "k1",
        ))
        .unwrap();
    storage
        .put(request(
            project_key("another-rule"),
            "Another rule",
            "norm",
            declared("owner-decision:another-rule"),
            project_owner(),
            serde_json::json!({}),
            "k2",
        ))
        .unwrap();

    let first = storage.canonical_export_json().unwrap();
    let second = storage.canonical_export_json().unwrap();
    assert_eq!(
        first, second,
        "repeated export of unchanged state must be byte-identical"
    );

    // No internal storage detail (rowid, our own composite record_key, a
    // database path) appears anywhere in the export.
    assert!(!first.contains("rowid"));
    assert!(
        !first.contains('\u{1f}'),
        "the internal composite storage key separator must never leak"
    );
    assert!(!first.to_lowercase().contains(".sqlite3"));

    // Nested object keys are sorted recursively, not just at the top level.
    assert!(
        first.contains(r#""m":{"b":2,"y":1}"#),
        "export was: {first}"
    );

    let parsed: serde_json::Value = serde_json::from_str(&first).unwrap();
    let array = parsed.as_array().unwrap();
    assert_eq!(array.len(), 2);
    for record in array {
        for field in [
            "$schema",
            "id",
            "title",
            "record_type",
            "scope",
            "origin",
            "authority",
            "payload",
            "schema_version",
        ] {
            assert!(
                record.get(field).is_some(),
                "exported record missing field \"{field}\": {record}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 11. Backup
// ---------------------------------------------------------------------------

#[test]
fn backup_produces_an_independently_openable_copy_with_the_same_exported_state() {
    let dir = TempDir::new("backup");
    let source_path = dir.join("source.sqlite3");
    let backup_path = dir.join("backup.sqlite3");

    let source = SqliteStorage::open_path(&source_path).unwrap();
    source
        .put(request(
            project_key("branch-naming"),
            "Branch naming",
            "norm",
            declared("owner-decision:branch-naming"),
            project_owner(),
            serde_json::json!({"pattern": "^feature/"}),
            "k1",
        ))
        .unwrap();

    source.backup_to(&backup_path).unwrap();

    // The source is untouched and still fully usable after taking a backup.
    assert!(source.get(&project_key("branch-naming")).unwrap().is_some());

    let backup = SqliteStorage::open_path(&backup_path).unwrap();
    assert_eq!(
        source.canonical_export_json().unwrap(),
        backup.canonical_export_json().unwrap()
    );
}

// ---------------------------------------------------------------------------
// 12. All six Scope variants
// ---------------------------------------------------------------------------

#[test]
fn round_trips_a_record_in_every_one_of_the_six_scope_variants() {
    let __dir_scopes = TempDir::new("scopes");
    let storage = SqliteStorage::open_path(__dir_scopes.join("db.sqlite3")).unwrap();

    let cases: Vec<(RecordKey, Authority)> = vec![
        (
            key(Scope::built_in_methodology(), "kernel-purity"),
            Authority::new(AuthorityKind::MethodologyOwner, "workspace-owner", None).unwrap(),
        ),
        (
            key(Scope::user_profile(sid("alice")), "editor-shortcuts"),
            Authority::new(AuthorityKind::User, "alice", None).unwrap(),
        ),
        (
            key(Scope::organization_profile(sid("acme")), "review-policy"),
            Authority::new(AuthorityKind::Organization, "acme", None).unwrap(),
        ),
        (
            key(
                Scope::project_workspace(sid("sample-project"), Some(sid("acme"))),
                "branch-naming",
            ),
            project_owner(),
        ),
        (
            key(
                Scope::repository_scope(sid("kernel-repo"), wid("sample-project")),
                "lint-rule",
            ),
            Authority::new(AuthorityKind::RepositoryMaintainer, "workspace-owner", None).unwrap(),
        ),
        (
            key(
                Scope::run_state(sid("run-42"), wid("sample-project")),
                "current-step",
            ),
            Authority::new(AuthorityKind::DelegatedRun, "run:42", None).unwrap(),
        ),
    ];

    for (i, (record_key, authority)) in cases.into_iter().enumerate() {
        let origin = if authority.kind() == AuthorityKind::MethodologyOwner {
            Origin::built_in()
        } else {
            declared("owner-decision:scope-test")
        };
        let req = request(
            record_key.clone(),
            "Scope round-trip",
            "norm",
            origin,
            authority,
            serde_json::json!({}),
            &format!("scope-case-{i}"),
        );
        storage.put(req).unwrap();
        let stored = storage.get(&record_key).unwrap().unwrap();
        assert_eq!(
            stored.key(),
            &record_key,
            "scope round-trip mismatch for case {i}"
        );
    }
}

// ---------------------------------------------------------------------------
// 13. Identity does not depend on the database file path
// ---------------------------------------------------------------------------

#[test]
fn record_identity_is_independent_of_which_database_file_backs_it() {
    let dir = TempDir::new("identity-vs-path");
    let storage_a = SqliteStorage::open_path(dir.join("a.sqlite3")).unwrap();
    let storage_b =
        SqliteStorage::open_path(dir.join("completely-different-name-b.sqlite3")).unwrap();

    let k = project_key("branch-naming");
    let record_a = storage_a
        .put(request(
            k.clone(),
            "Branch naming",
            "norm",
            declared("owner-decision:x"),
            project_owner(),
            serde_json::json!({}),
            "k1",
        ))
        .unwrap();
    let record_b = storage_b
        .put(request(
            k.clone(),
            "Branch naming",
            "norm",
            declared("owner-decision:x"),
            project_owner(),
            serde_json::json!({}),
            "k1",
        ))
        .unwrap();

    assert_eq!(
        record_a.record().key().storage_key(),
        record_b.record().key().storage_key(),
        "the same logical record must compute the same identity regardless of which file stores it"
    );
}

// ---------------------------------------------------------------------------
// 14. `$schema` is carried verbatim, per record, never a fixed fallback
// ---------------------------------------------------------------------------

#[test]
fn schema_ref_is_preserved_exactly_across_write_reopen_export_and_backup() {
    const SCHEMA_A: &str =
        "https://meridian.invalid/registries/operating-model/scoped-record.schema.json";
    const SCHEMA_B: &str = "https://meridian.invalid/registries/custom/other-record.schema.json";

    let dir = TempDir::new("schema-ref");
    let source_path = dir.join("source.sqlite3");
    let backup_path = dir.join("backup.sqlite3");

    let storage = SqliteStorage::open_path(&source_path).unwrap();
    storage
        .put(request_with_schema(
            SCHEMA_A,
            project_key("record-a"),
            "Record A",
            "norm",
            declared("owner-decision:record-a"),
            project_owner(),
            serde_json::json!({}),
            "k-a",
        ))
        .unwrap();
    storage
        .put(request_with_schema(
            SCHEMA_B,
            project_key("record-b"),
            "Record B",
            "norm",
            declared("owner-decision:record-b"),
            project_owner(),
            serde_json::json!({}),
            "k-b",
        ))
        .unwrap();

    // 1. Immediately after write.
    assert_eq!(
        storage
            .get(&project_key("record-a"))
            .unwrap()
            .unwrap()
            .schema(),
        SCHEMA_A
    );
    assert_eq!(
        storage
            .get(&project_key("record-b"))
            .unwrap()
            .unwrap()
            .schema(),
        SCHEMA_B
    );

    // 2. After closing and reopening the same file.
    drop(storage);
    let reopened = SqliteStorage::open_path(&source_path).unwrap();
    assert_eq!(
        reopened
            .get(&project_key("record-a"))
            .unwrap()
            .unwrap()
            .schema(),
        SCHEMA_A
    );
    assert_eq!(
        reopened
            .get(&project_key("record-b"))
            .unwrap()
            .unwrap()
            .schema(),
        SCHEMA_B
    );

    // 3. In the canonical export — each record keeps its own `$schema`, not
    // a shared or generic one.
    let export = reopened.canonical_export_json().unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&export).unwrap();
    let array = parsed.as_array().unwrap();
    assert_eq!(array.len(), 2);
    for record in array {
        let id = record.get("id").and_then(|v| v.as_str()).unwrap();
        let schema = record.get("$schema").and_then(|v| v.as_str()).unwrap();
        match id {
            "record-a" => assert_eq!(schema, SCHEMA_A),
            "record-b" => assert_eq!(schema, SCHEMA_B),
            other => panic!("unexpected record id in export: {other}"),
        }
    }

    // 4. In a backup snapshot.
    reopened.backup_to(&backup_path).unwrap();
    let backup = SqliteStorage::open_path(&backup_path).unwrap();
    assert_eq!(
        backup
            .get(&project_key("record-a"))
            .unwrap()
            .unwrap()
            .schema(),
        SCHEMA_A
    );
    assert_eq!(
        backup
            .get(&project_key("record-b"))
            .unwrap()
            .unwrap()
            .schema(),
        SCHEMA_B
    );
    assert_eq!(
        reopened.canonical_export_json().unwrap(),
        backup.canonical_export_json().unwrap()
    );
}

// ---------------------------------------------------------------------------
// 15. organization_profile_id distinguishes otherwise-identical records
// ---------------------------------------------------------------------------

#[test]
fn two_project_workspace_records_with_the_same_type_id_and_record_id_but_different_organization_profile_id_do_not_collide(
) {
    let __dir_org_profile_distinct = TempDir::new("org-profile-distinct");
    let storage = SqliteStorage::open_path(__dir_org_profile_distinct.join("db.sqlite3")).unwrap();

    let key_acme = key(
        Scope::project_workspace(sid("sample-project"), Some(sid("acme"))),
        "branch-naming",
    );
    let key_other = key(
        Scope::project_workspace(sid("sample-project"), Some(sid("other-org"))),
        "branch-naming",
    );
    let key_none = key(
        Scope::project_workspace(sid("sample-project"), None),
        "branch-naming",
    );

    // All three storage keys must be pairwise distinct.
    assert_ne!(key_acme.storage_key(), key_other.storage_key());
    assert_ne!(key_acme.storage_key(), key_none.storage_key());
    assert_ne!(key_other.storage_key(), key_none.storage_key());

    storage
        .put(request(
            key_acme.clone(),
            "Branch naming (acme)",
            "norm",
            declared("owner-decision:acme"),
            project_owner(),
            serde_json::json!({"org": "acme"}),
            "k-acme",
        ))
        .unwrap();
    storage
        .put(request(
            key_other.clone(),
            "Branch naming (other-org)",
            "norm",
            declared("owner-decision:other-org"),
            project_owner(),
            serde_json::json!({"org": "other-org"}),
            "k-other",
        ))
        .unwrap();
    storage
        .put(request(
            key_none.clone(),
            "Branch naming (no org)",
            "norm",
            declared("owner-decision:none"),
            project_owner(),
            serde_json::json!({"org": null}),
            "k-none",
        ))
        .unwrap();

    // Each of the three records is independently readable with its own
    // distinct content — none was overwritten by another.
    let acme = storage.get(&key_acme).unwrap().unwrap();
    let other = storage.get(&key_other).unwrap().unwrap();
    let none = storage.get(&key_none).unwrap().unwrap();
    assert_eq!(acme.title(), "Branch naming (acme)");
    assert_eq!(other.title(), "Branch naming (other-org)");
    assert_eq!(none.title(), "Branch naming (no org)");
    assert_eq!(storage.table_row_count("records").unwrap(), 3);
    assert_eq!(
        storage.list_revisions(&key_acme).unwrap().len()
            + storage.list_revisions(&key_other).unwrap().len()
            + storage.list_revisions(&key_none).unwrap().len(),
        3,
        "each key must own exactly one revision — no put silently updated another key's record"
    );
}

// Small helper trait so the tests above can call `put_evidence`/`resolve_evidence`
// without repeating the fully qualified `<SqliteStorage as EvidenceRepository>`
// syntax at every call site — plain inherent-looking names, backed by the
// real port trait.
trait EvidenceOps {
    fn put_evidence(
        &self,
        request: PutEvidenceRequest,
    ) -> Result<meridian_app::storage::StoredEvidence, PortError>;
    fn resolve_evidence(
        &self,
        evidence_ref: &EvidenceRef,
    ) -> Result<Option<meridian_app::storage::StoredEvidence>, PortError>;
}

impl EvidenceOps for SqliteStorage {
    fn put_evidence(
        &self,
        request: PutEvidenceRequest,
    ) -> Result<meridian_app::storage::StoredEvidence, PortError> {
        // Scoped to this function only: `RecordRepository` (imported at file
        // scope, for the `.put`/`.get`/... calls in the tests above) also
        // declares a `put` method, and importing both traits at file scope
        // would make every `.put(...)` call ambiguous.
        use meridian_app::storage::EvidenceRepository;
        EvidenceRepository::put(self, request)
    }
    fn resolve_evidence(
        &self,
        evidence_ref: &EvidenceRef,
    ) -> Result<Option<meridian_app::storage::StoredEvidence>, PortError> {
        use meridian_app::storage::EvidenceRepository;
        EvidenceRepository::resolve(self, evidence_ref)
    }
}
