#![cfg(test)]
//! The two-database canonical import over in-memory stores: acceptance
//! before any database is opened, role routing, and the compensating
//! restore of the tool database when the workspace database refuses.

use std::sync::{Arc, Mutex};

use meridian_core::migration::run::{MigrationRun, MigrationRunId, RecordState};
use meridian_core::types::{ContentDigest, NonEmptyString, WorkspaceRelativePath};

use super::*;
use crate::storage::{
    AppliedMigration, ManagedRecord, PortError, RecordKey, RecordRevision, RevisionNumber,
    SchemaRef,
};
use crate::workspace::{DirEntry, ReadError, WorkspaceReader};

struct SchemaReader;

impl WorkspaceReader for SchemaReader {
    fn read_text(&self, path: &WorkspaceRelativePath) -> Result<String, ReadError> {
        Ok(match path.as_str() {
            "registries/operating-model/instance-data-migration.schema.json" => {
                include_str!(
                    "../../../../registries/operating-model/instance-data-migration.schema.json"
                )
            }
            "registries/operating-model/instance-canonical-export.schema.json" => {
                include_str!(
                    "../../../../registries/operating-model/instance-canonical-export.schema.json"
                )
            }
            "registries/operating-model/scoped-record.schema.json" => {
                include_str!("../../../../registries/operating-model/scoped-record.schema.json")
            }
            "registries/rule-resolution/applicability.schema.json" => {
                include_str!("../../../../registries/rule-resolution/applicability.schema.json")
            }
            _ => return Err(ReadError::NotFound),
        }
        .to_string())
    }
    fn read_bytes(&self, path: &WorkspaceRelativePath) -> Result<Vec<u8>, ReadError> {
        self.read_text(path).map(String::into_bytes)
    }
    fn list_dir(&self, _: &WorkspaceRelativePath) -> Result<Vec<DirEntry>, ReadError> {
        Err(ReadError::NotFound)
    }
}

#[derive(Default)]
struct Fake {
    records: Mutex<Vec<PutRecordRequest>>,
    checkpoints: Mutex<Vec<(String, Vec<PutRecordRequest>)>>,
    fail_batch: bool,
    fail_restore: bool,
    restores: Mutex<usize>,
}

#[derive(Clone)]
struct Shared(Arc<Fake>);

fn managed(request: &PutRecordRequest) -> ManagedRecord {
    ManagedRecord::from_parts(
        request.key().clone(),
        SchemaRef::new(request.schema()).unwrap(),
        request.schema_version(),
        NonEmptyString::new(request.title()).unwrap(),
        request.record_type().clone(),
        request.origin().clone(),
        request.authority().clone(),
        request.payload().clone(),
        RevisionNumber::FIRST,
        ContentDigest::of_str(request.title()),
    )
}

fn state(records: &[PutRecordRequest]) -> RecordState {
    let keys: Vec<String> = records.iter().map(|r| r.key().storage_key()).collect();
    RecordState {
        digest: ContentDigest::of_str(&keys.join("|")),
        revision_count: records.len() as u64,
    }
}

fn unused<T>() -> Result<T, MigrationPortError> {
    Err(MigrationPortError::Storage(
        "not used by this test".to_string(),
    ))
}

impl RecordRepository for Shared {
    fn put(&self, _: PutRecordRequest) -> Result<PutRecordOutcome, PortError> {
        Err(PortError::Storage("not used".to_string()))
    }
    fn put_batch(
        &self,
        requests: Vec<PutRecordRequest>,
    ) -> Result<Vec<PutRecordOutcome>, PortError> {
        if self.0.fail_batch {
            return Err(PortError::Storage("injected batch failure".to_string()));
        }
        let outcomes = requests
            .iter()
            .map(|r| PutRecordOutcome::Created(managed(r)))
            .collect();
        self.0.records.lock().unwrap().extend(requests);
        Ok(outcomes)
    }
    fn get(&self, _: &RecordKey) -> Result<Option<ManagedRecord>, PortError> {
        Ok(None)
    }
    fn get_revision(
        &self,
        _: &RecordKey,
        _: RevisionNumber,
    ) -> Result<Option<RecordRevision>, PortError> {
        Ok(None)
    }
    fn list_revisions(&self, _: &RecordKey) -> Result<Vec<RecordRevision>, PortError> {
        Ok(Vec::new())
    }
    fn export_all(&self) -> Result<Vec<ManagedRecord>, PortError> {
        Ok(self.0.records.lock().unwrap().iter().map(managed).collect())
    }
}

impl MigrationRepository for Shared {
    fn record_state(&self) -> Result<RecordState, MigrationPortError> {
        Ok(state(&self.0.records.lock().unwrap()))
    }
    fn run_by_idempotency_key(
        &self,
        _: &ContentDigest,
    ) -> Result<Option<MigrationRun>, MigrationPortError> {
        unused()
    }
    fn run(&self, _: &MigrationRunId) -> Result<Option<MigrationRun>, MigrationPortError> {
        unused()
    }
    fn newest_run(&self) -> Result<Option<MigrationRun>, MigrationPortError> {
        unused()
    }
    fn apply_migration(
        &self,
        _: MigrationApplyRequest,
    ) -> Result<AppliedMigration, MigrationPortError> {
        unused()
    }
    fn rollback_migration(
        &self,
        _: &MigrationRunId,
        _: &ContentDigest,
    ) -> Result<RolledBackMigration, MigrationPortError> {
        unused()
    }
    fn checkpoint(&self) -> Result<Checkpoint, MigrationPortError> {
        let records = self.0.records.lock().unwrap().clone();
        let mut checkpoints = self.0.checkpoints.lock().unwrap();
        let name = format!("checkpoint-{}", checkpoints.len());
        let s = state(&records);
        checkpoints.push((name.clone(), records));
        Ok(Checkpoint::new(name, s.digest.clone(), s))
    }
    fn restore(&self, checkpoint: &Checkpoint) -> Result<RecordState, MigrationPortError> {
        if self.0.fail_restore {
            return Err(MigrationPortError::Checkpoint(
                "injected restore failure".to_string(),
            ));
        }
        let checkpoints = self.0.checkpoints.lock().unwrap();
        let (_, records) = checkpoints
            .iter()
            .find(|(n, _)| n == checkpoint.name())
            .unwrap();
        *self.0.records.lock().unwrap() = records.clone();
        *self.0.restores.lock().unwrap() += 1;
        Ok(state(records))
    }
    fn discard(&self, checkpoint: Checkpoint) -> Result<(), MigrationPortError> {
        self.0
            .checkpoints
            .lock()
            .unwrap()
            .retain(|(n, _)| n != checkpoint.name());
        Ok(())
    }
}

const BUILT_IN: &str = r#"{"$schema":"registries/operating-model/scoped-record.schema.json","schema_version":1,"id":"norm-one","title":"Norm one","record_type":"norm","scope":{"type":"built-in-methodology","id":"built-in-methodology"},"origin":{"kind":"built-in"},"authority":{"kind":"methodology-owner","authority_ref":"kernel-owner"},"payload":{"text":"one"}}"#;
const WORKSPACE: &str = r#"{"$schema":"registries/operating-model/scoped-record.schema.json","schema_version":1,"id":"note-one","title":"Note one","record_type":"note","scope":{"type":"project-workspace","id":"sample"},"origin":{"kind":"declared","source_ref":"owner-decision:note"},"authority":{"kind":"project-owner","authority_ref":"sample-owner"},"payload":{"text":"two"}}"#;

fn input(records: &[&str]) -> Vec<u8> {
    format!(
        r#"{{"status":"ok","command":"export","result":[{}]}}"#,
        records.join(",")
    )
    .into_bytes()
}

struct Run {
    tool: Shared,
    workspace: Shared,
    opened: Mutex<usize>,
}

impl Run {
    fn new(tool: Fake, workspace: Fake) -> Run {
        Run {
            tool: Shared(Arc::new(tool)),
            workspace: Shared(Arc::new(workspace)),
            opened: Mutex::new(0),
        }
    }

    fn import(&self, bytes: &[u8], confirm: &str) -> Result<CanonicalOutcome, MigrationError> {
        let schemas = KernelSchemas::load(&SchemaReader).unwrap();
        let open_tool = |access: StoreAccess| -> Result<Box<dyn MigrationStore>, MigrationError> {
            assert_eq!(access, StoreAccess::ReadWrite);
            *self.opened.lock().unwrap() += 1;
            Ok(Box::new(self.tool.clone()))
        };
        let open_workspace = |_: StoreAccess| -> Result<Box<dyn MigrationStore>, MigrationError> {
            *self.opened.lock().unwrap() += 1;
            Ok(Box::new(self.workspace.clone()))
        };
        import_canonical(
            &schemas,
            bytes,
            confirm,
            &open_tool,
            &open_workspace,
            &crate::events::NoOpEventSink,
        )
    }
}

fn digest(bytes: &[u8]) -> String {
    ContentDigest::of_bytes(bytes).value().to_string()
}

#[test]
fn migration_canonical_import_routes_each_record_by_its_scope() {
    let run = Run::new(Fake::default(), Fake::default());
    let bytes = input(&[BUILT_IN, WORKSPACE]);
    let outcome = run.import(&bytes, &digest(&bytes)).unwrap();
    let CanonicalOutcome::Imported {
        tool, workspace, ..
    } = outcome
    else {
        panic!("{outcome:?}");
    };
    assert_eq!((tool.records, workspace.records), (1, 1));
    assert_eq!(
        run.tool.0.records.lock().unwrap()[0].key().id().as_str(),
        "norm-one"
    );
    assert_eq!(
        run.workspace.0.records.lock().unwrap()[0]
            .key()
            .id()
            .as_str(),
        "note-one"
    );
    assert!(run.tool.0.checkpoints.lock().unwrap().is_empty());
    assert!(run.workspace.0.checkpoints.lock().unwrap().is_empty());
}

#[test]
fn migration_canonical_import_restores_the_tool_database_when_the_workspace_refuses() {
    let run = Run::new(
        Fake::default(),
        Fake {
            fail_batch: true,
            ..Fake::default()
        },
    );
    let bytes = input(&[BUILT_IN, WORKSPACE]);
    let outcome = run.import(&bytes, &digest(&bytes)).unwrap();
    let CanonicalOutcome::WriteFailed {
        role,
        restored_tool_state,
        ..
    } = outcome
    else {
        panic!("{outcome:?}");
    };
    assert_eq!(role, DatabaseRole::Workspace);
    assert_eq!(restored_tool_state, Some(state(&[])));
    assert!(
        run.tool.0.records.lock().unwrap().is_empty(),
        "no partial success"
    );
    assert_eq!(*run.tool.0.restores.lock().unwrap(), 1);
    assert!(run.tool.0.checkpoints.lock().unwrap().is_empty());
}

#[test]
fn migration_canonical_import_reports_a_failed_compensation_and_keeps_the_checkpoint() {
    let run = Run::new(
        Fake {
            fail_restore: true,
            ..Fake::default()
        },
        Fake {
            fail_batch: true,
            ..Fake::default()
        },
    );
    let bytes = input(&[BUILT_IN, WORKSPACE]);
    let error = run.import(&bytes, &digest(&bytes)).unwrap_err();
    assert!(
        matches!(error, MigrationError::CompensationFailed { .. }),
        "{error:?}"
    );
    assert_eq!(run.tool.0.checkpoints.lock().unwrap().len(), 1);
}

#[test]
fn migration_canonical_import_a_tool_refusal_never_touches_the_workspace() {
    let run = Run::new(
        Fake {
            fail_batch: true,
            ..Fake::default()
        },
        Fake::default(),
    );
    let bytes = input(&[BUILT_IN, WORKSPACE]);
    let outcome = run.import(&bytes, &digest(&bytes)).unwrap();
    assert!(matches!(
        outcome,
        CanonicalOutcome::WriteFailed {
            role: DatabaseRole::Tool,
            restored_tool_state: None,
            ..
        }
    ));
    assert!(run.workspace.0.records.lock().unwrap().is_empty());
}

#[test]
fn migration_canonical_import_opens_no_database_before_confirmation_and_acceptance() {
    let run = Run::new(Fake::default(), Fake::default());
    let bytes = input(&[BUILT_IN, WORKSPACE]);
    let outcome = run.import(&bytes, "0000").unwrap();
    assert!(matches!(
        outcome,
        CanonicalOutcome::ConfirmationMismatch { .. }
    ));

    let duplicate = input(&[WORKSPACE, WORKSPACE]);
    let error = run.import(&duplicate, &digest(&duplicate)).unwrap_err();
    assert!(matches!(error, MigrationError::CanonicalInput(_)));

    let invalid = input(&[&WORKSPACE.replace("\"note\"", "\"Not A Semantic Id\"")]);
    let error = run.import(&invalid, &digest(&invalid)).unwrap_err();
    assert!(matches!(error, MigrationError::CanonicalInput(_)));

    let wrong = br#"{"status":"ok","command":"validate","result":[]}"#;
    let error = run.import(wrong, &digest(wrong)).unwrap_err();
    assert!(matches!(error, MigrationError::CanonicalInput(_)));
    assert_eq!(*run.opened.lock().unwrap(), 0);
}

#[test]
fn migration_verify_empty_applicability_sides_never_verify() {
    let fingerprint = ContentDigest::of_str("plan");
    let empty = RecordState {
        digest: ContentDigest::of_str("[]"),
        revision_count: 0,
    };
    let run = MigrationRun {
        id: MigrationRunId::for_run(
            meridian_core::migration::run::RunSequence::FIRST,
            &fingerprint,
        ),
        sequence: meridian_core::migration::run::RunSequence::FIRST,
        plan_ref: "plan".to_string(),
        plan_fingerprint: fingerprint,
        idempotency_key: ContentDigest::of_str("key"),
        source_repository_ref: "repo".to_string(),
        source_revision: "rev".to_string(),
        pre_state: empty.clone(),
        post_state: empty,
        checkpoint_digest: ContentDigest::of_str("checkpoint"),
        written_records: 0,
        rollback: None,
    };
    let clean = VerificationDiff {
        expected: 0,
        imported: 0,
        missing: Vec::new(),
        extra: Vec::new(),
        changed: Vec::new(),
        duplicate: Vec::new(),
    };
    assert!(clean.is_clean());
    // An applied run and a clean diff, but both applicability sides empty:
    // nothing is proven, so nothing is verified.
    let applicability = compare_applicability(&[], &[]);
    assert!(!is_verified(Some(&run), &clean, &applicability));
    assert!(!is_verified(None, &clean, &applicability));
}
