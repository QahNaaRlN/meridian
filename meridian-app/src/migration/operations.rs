//! The five migration operations (`meridian-rust-migration-program-plan.md`
//! §5.22.2–§5.22.5), each a typed result over ports only.
//!
//! `plan`, `apply` and `verify` share one loaded, accepted bundle each;
//! `import_frozen` is exactly their composition over ONE load:
//! `load → confirm → apply → verify`. `import_canonical` accepts a complete
//! `export` envelope, checks every record, refuses duplicate identities,
//! and only then splits by database role, compensating the first database
//! from its checkpoint when the second one fails.

use meridian_core::migration::applicability::{compare_applicability, ApplicabilityEquivalence};
use meridian_core::migration::run::{
    decide_apply, ApplyDecision, MigrationRun, MigrationRunId, RecordState, RollbackRefusal,
};
use meridian_core::migration::write_set::{verify_read_back, ReadBackRecord, VerificationDiff};
use meridian_core::resolver::ApplicabilityRecord;
use meridian_core::types::{ContentDigest, NonEmptyString, ScopeType};
use serde::Deserialize;
use serde_json::{Map, Value};

use super::bundle::{
    load_bundle, AcceptedBundle, BundleError, BundleRejection, BundleVerdict, KernelSchemas,
};
use super::records::{canonical_request, read_back, target_request};
use super::source::FrozenSource;
use crate::events::{EventKind, EventSink, ObservedEvent};
use crate::operating_model::migration_boundary::head::{head, HeadParts, ScopeDto};
use crate::operating_model::run_contract_boundary::envelope::{AuthorityDto, OriginDto};
use crate::rule_resolution::typed_applicability_register;
use crate::source_format::json_schema;
use crate::storage::{
    Checkpoint, DatabaseRole, MigrationApplyRequest, MigrationPortError, MigrationRepository,
    PutRecordOutcome, PutRecordRequest, RecordRepository, RolledBackMigration,
};
use crate::workspace_state::PayloadSubject;

fn emit(sink: &dyn EventSink, kind: EventKind, summary: String) {
    if let Ok(summary) = NonEmptyString::new(summary) {
        sink.record(&ObservedEvent::new(kind, summary));
    }
}

/// An input or environment failure of an operation: no domain result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationError {
    Bundle(BundleError),
    /// A database could not be opened with the access the operation needs.
    Open(String),
    Storage(MigrationPortError),
    /// The canonical-records input is not a complete, valid `export`
    /// envelope.
    CanonicalInput(Vec<String>),
    /// A two-database import failed on the second database AND the first
    /// could not be restored from its checkpoint; the checkpoint is kept.
    CompensationFailed {
        failure: String,
        compensation: String,
    },
}

impl std::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationError::Bundle(error) => write!(f, "{error}"),
            MigrationError::Open(message) => write!(f, "{message}"),
            MigrationError::Storage(error) => write!(f, "{error}"),
            MigrationError::CanonicalInput(problems) => write!(
                f,
                "the input is not a complete canonical export: {}",
                problems.join("; ")
            ),
            MigrationError::CompensationFailed {
                failure,
                compensation,
            } => write!(
                f,
                "the import failed ({failure}) and the tool database could not be restored from its checkpoint ({compensation}); the checkpoint is kept"
            ),
        }
    }
}

impl std::error::Error for MigrationError {}

impl From<BundleError> for MigrationError {
    fn from(error: BundleError) -> Self {
        MigrationError::Bundle(error)
    }
}

impl From<MigrationPortError> for MigrationError {
    fn from(error: MigrationPortError) -> Self {
        MigrationError::Storage(error)
    }
}

fn storage(error: impl std::fmt::Display) -> MigrationError {
    MigrationError::Storage(MigrationPortError::Storage(error.to_string()))
}

/// The bundle an operation ran against: accepted, or rejected with its
/// diagnostics.
pub type PlanResult = BundleVerdict;

/// `migration plan`: the bundle through every accepted operation. Reads no
/// database and writes nothing.
pub fn plan(
    schemas: &KernelSchemas,
    source: &dyn FrozenSource,
    sink: &dyn EventSink,
) -> Result<PlanResult, MigrationError> {
    let verdict = load_bundle(source, schemas)?;
    emit(
        sink,
        EventKind::Check,
        match &verdict {
            BundleVerdict::Accepted(b) => {
                format!("migration plan: bundle accepted, plan {}", b.facts.plan_id)
            }
            BundleVerdict::Rejected(r) => {
                format!(
                    "migration plan: bundle rejected with {} diagnostic(s)",
                    r.diagnostics.len()
                )
            }
        },
    );
    Ok(verdict)
}

/// How an apply may change state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyIntent {
    /// Never writes. A present confirmation must still match.
    DryRun { confirm: Option<String> },
    /// Writes only when `confirm` is the recomputed plan fingerprint.
    Apply { confirm: String },
}

/// Per-record effect counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EffectCounts {
    pub created: usize,
    pub updated: usize,
    pub unchanged: usize,
}

/// What an apply did (or, for a dry run, would do).
#[derive(Debug, Clone, PartialEq)]
pub enum ApplyOutcome {
    ConfirmationMismatch {
        expected: ContentDigest,
        given: String,
    },
    DryRun {
        would: EffectCounts,
        decision: DryRunDecision,
    },
    Applied {
        run: MigrationRun,
        effects: EffectCounts,
    },
    AlreadyApplied {
        run: MigrationRun,
    },
    Conflict {
        run: MigrationRunId,
        recorded: ContentDigest,
        requested: ContentDigest,
    },
    PreviouslyRolledBack {
        run: MigrationRun,
    },
    /// A record write failed; the whole transaction rolled back.
    WriteFailed {
        reason: String,
    },
}

/// What a confirmed apply of the same state would decide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DryRunDecision {
    WouldApply,
    AlreadyApplied(MigrationRunId),
    Conflict(MigrationRunId),
    PreviouslyRolledBack(MigrationRunId),
}

impl ApplyOutcome {
    /// Whether the outcome is a positive result.
    pub fn is_positive(&self) -> bool {
        match self {
            ApplyOutcome::Applied { .. } | ApplyOutcome::AlreadyApplied { .. } => true,
            ApplyOutcome::DryRun { decision, .. } => matches!(
                decision,
                DryRunDecision::WouldApply | DryRunDecision::AlreadyApplied(_)
            ),
            _ => false,
        }
    }
}

/// A read-and-write repository of one database.
pub trait MigrationStore: RecordRepository + MigrationRepository {}
impl<T: RecordRepository + MigrationRepository> MigrationStore for T {}

/// The access an operation opens its database with. A database is opened
/// only after the operation's confirmation matched, and read-only whenever
/// nothing may change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreAccess {
    ReadOnly,
    ReadWrite,
}

/// Opens the operation's database on demand (the CLI's composition).
pub type StoreOpener<'a> =
    &'a dyn Fn(StoreAccess) -> Result<Box<dyn MigrationStore>, MigrationError>;

fn confirmation_mismatch(bundle: &AcceptedBundle, intent: &ApplyIntent) -> Option<ApplyOutcome> {
    let given = match intent {
        ApplyIntent::DryRun { confirm } => confirm.as_deref()?,
        ApplyIntent::Apply { confirm } => confirm.as_str(),
    };
    (given != bundle.facts.fingerprint.value()).then(|| ApplyOutcome::ConfirmationMismatch {
        expected: bundle.facts.fingerprint.clone(),
        given: given.to_string(),
    })
}

fn stored_records(store: &dyn MigrationStore) -> Result<Vec<ReadBackRecord>, MigrationError> {
    Ok(store
        .export_all()
        .map_err(storage)?
        .iter()
        .map(read_back)
        .collect())
}

fn predicted_effects(
    bundle: &AcceptedBundle,
    store: &dyn MigrationStore,
) -> Result<EffectCounts, MigrationError> {
    let diff = verify_read_back(&bundle.write_set, &stored_records(store)?);
    let created = diff.missing.len();
    let updated = diff.changed.len();
    Ok(EffectCounts {
        created,
        updated,
        unchanged: diff.expected - created - updated,
    })
}

fn applied_effects(outcomes: &[PutRecordOutcome]) -> EffectCounts {
    let mut effects = EffectCounts::default();
    for outcome in outcomes {
        match outcome {
            PutRecordOutcome::Created(_) => effects.created += 1,
            PutRecordOutcome::Updated(_) => effects.updated += 1,
            PutRecordOutcome::AlreadyApplied(_) => effects.unchanged += 1,
        }
    }
    effects
}

/// `apply` over an already accepted bundle — the one apply route of both
/// `migration apply` and `import frozen-instance`.
pub fn apply_accepted(
    bundle: &AcceptedBundle,
    store: &dyn MigrationStore,
    intent: &ApplyIntent,
    sink: &dyn EventSink,
) -> Result<ApplyOutcome, MigrationError> {
    let fingerprint = &bundle.facts.fingerprint;
    if let Some(mismatch) = confirmation_mismatch(bundle, intent) {
        emit(
            sink,
            EventKind::Check,
            "migration apply: confirmation does not match the recomputed plan fingerprint"
                .to_string(),
        );
        return Ok(mismatch);
    }
    let existing = store.run_by_idempotency_key(&bundle.facts.idempotency_key)?;
    let decision = decide_apply(existing.as_ref(), fingerprint);
    if let ApplyIntent::DryRun { .. } = intent {
        let decision = match decision {
            ApplyDecision::Proceed => DryRunDecision::WouldApply,
            ApplyDecision::AlreadyApplied(run) => DryRunDecision::AlreadyApplied(run.id),
            ApplyDecision::Conflict { run, .. } => DryRunDecision::Conflict(run),
            ApplyDecision::RolledBack(run) => DryRunDecision::PreviouslyRolledBack(run.id),
        };
        let would = match decision {
            DryRunDecision::WouldApply => predicted_effects(bundle, store)?,
            _ => EffectCounts::default(),
        };
        return Ok(ApplyOutcome::DryRun { would, decision });
    }
    match decision {
        ApplyDecision::AlreadyApplied(run) => {
            emit(
                sink,
                EventKind::Check,
                format!("migration apply: plan already applied as {}", run.id),
            );
            return Ok(ApplyOutcome::AlreadyApplied { run });
        }
        ApplyDecision::Conflict {
            run,
            recorded_fingerprint,
            requested_fingerprint,
        } => {
            return Ok(ApplyOutcome::Conflict {
                run,
                recorded: recorded_fingerprint,
                requested: requested_fingerprint,
            })
        }
        ApplyDecision::RolledBack(run) => return Ok(ApplyOutcome::PreviouslyRolledBack { run }),
        ApplyDecision::Proceed => {}
    }
    let records: Vec<PutRecordRequest> = bundle
        .write_set
        .entries()
        .iter()
        .map(|entry| target_request(entry, &bundle.facts.idempotency_key))
        .collect::<Result<_, _>>()
        .map_err(|reason| MigrationError::Storage(MigrationPortError::Storage(reason)))?;
    let request = MigrationApplyRequest {
        plan_ref: bundle.facts.plan_id.clone(),
        plan_fingerprint: fingerprint.clone(),
        idempotency_key: bundle.facts.idempotency_key.clone(),
        source_repository_ref: bundle.source.repository_ref.clone(),
        source_revision: bundle.source.revision.clone(),
        records,
    };
    match store.apply_migration(request) {
        Ok(applied) => {
            emit(
                sink,
                EventKind::Change,
                format!(
                    "migration apply: run {} wrote {} record(s)",
                    applied.run.id, applied.run.written_records
                ),
            );
            Ok(ApplyOutcome::Applied {
                effects: applied_effects(&applied.outcomes),
                run: applied.run,
            })
        }
        Err(MigrationPortError::Record(error)) => {
            emit(
                sink,
                EventKind::Error,
                "migration apply: a record write failed; nothing was written".to_string(),
            );
            Ok(ApplyOutcome::WriteFailed {
                reason: error.to_string(),
            })
        }
        Err(MigrationPortError::RunAlreadyRecorded { run }) => {
            let run = store
                .run(&run)?
                .ok_or_else(|| storage("a recorded run vanished"))?;
            Ok(ApplyOutcome::AlreadyApplied { run })
        }
        Err(other) => Err(MigrationError::Storage(other)),
    }
}

/// `migration apply`.
pub fn apply(
    schemas: &KernelSchemas,
    source: &dyn FrozenSource,
    open: StoreOpener<'_>,
    intent: &ApplyIntent,
    sink: &dyn EventSink,
) -> Result<Result<(Box<AcceptedBundle>, ApplyOutcome), BundleRejection>, MigrationError> {
    let bundle = match plan(schemas, source, sink)? {
        BundleVerdict::Rejected(rejection) => return Ok(Err(*rejection)),
        BundleVerdict::Accepted(bundle) => bundle,
    };
    if let Some(mismatch) = confirmation_mismatch(&bundle, intent) {
        return Ok(Ok((bundle, mismatch)));
    }
    let access = match intent {
        ApplyIntent::DryRun { .. } => StoreAccess::ReadOnly,
        ApplyIntent::Apply { .. } => StoreAccess::ReadWrite,
    };
    let store = open(access)?;
    let outcome = apply_accepted(&bundle, store.as_ref(), intent, sink)?;
    Ok(Ok((bundle, outcome)))
}

/// The verification of one accepted bundle against one database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verification {
    pub run: Option<MigrationRun>,
    pub diff: VerificationDiff,
    pub applicability: ApplicabilityEquivalence,
    pub retained: usize,
    pub verified: bool,
}

fn after_side(
    bundle: &AcceptedBundle,
    stored: &[crate::storage::ManagedRecord],
    schemas: &KernelSchemas,
) -> Vec<ApplicabilityRecord> {
    let mut after = Vec::new();
    for target_id in &bundle.applicability.target_ids {
        let Some(entry) = bundle
            .write_set
            .entries()
            .iter()
            .find(|e| e.target().id == *target_id)
        else {
            continue;
        };
        let Some(record) = stored.iter().find(|r| {
            r.key().id() == target_id && r.key().scope().same_scope(&entry.target().scope)
        }) else {
            continue;
        };
        let Some(content) = record
            .payload()
            .as_map()
            .get("content")
            .and_then(Value::as_str)
        else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(content) else {
            continue;
        };
        let mut document = Map::new();
        document.insert(
            "$schema".to_string(),
            Value::String(bundle.applicability.declared_schema.clone()),
        );
        document.insert("schema_version".to_string(), Value::from(1));
        document.insert("records".to_string(), Value::Array(vec![value]));
        if let Ok(records) =
            typed_applicability_register(&Value::Object(document), schemas.applicability())
        {
            after.extend(records);
        }
    }
    after
}

/// The one verification verdict: an applied (not rolled-back) run, a clean
/// read-back diff and a NON-EMPTY equivalent applicability proof. An empty
/// proof never verifies (`ApplicabilityEquivalence::is_equivalent`).
fn is_verified(
    run: Option<&MigrationRun>,
    diff: &VerificationDiff,
    applicability: &ApplicabilityEquivalence,
) -> bool {
    run.is_some_and(|r| r.status() == meridian_core::migration::run::RunStatus::Applied)
        && diff.is_clean()
        && applicability.is_equivalent()
}

/// `verify` over an already accepted bundle — the one verify route of both
/// `migration verify` and `import frozen-instance`.
pub fn verify_accepted(
    bundle: &AcceptedBundle,
    store: &dyn MigrationStore,
    schemas: &KernelSchemas,
    sink: &dyn EventSink,
) -> Result<Verification, MigrationError> {
    let run = store.run_by_idempotency_key(&bundle.facts.idempotency_key)?;
    let managed = store.export_all().map_err(storage)?;
    let stored: Vec<ReadBackRecord> = managed.iter().map(read_back).collect();
    let diff = verify_read_back(&bundle.write_set, &stored);
    let applicability = compare_applicability(
        &bundle.applicability.before,
        &after_side(bundle, &managed, schemas),
    );
    let verified = is_verified(run.as_ref(), &diff, &applicability);
    emit(
        sink,
        EventKind::Check,
        format!(
            "migration verify: {} of {} expected record(s) imported, applicability {}",
            diff.imported,
            diff.expected,
            if applicability.is_equivalent() {
                "equivalent"
            } else {
                "not equivalent"
            }
        ),
    );
    Ok(Verification {
        retained: bundle.write_set.retained().len(),
        run,
        diff,
        applicability,
        verified,
    })
}

/// `migration verify`.
pub fn verify(
    schemas: &KernelSchemas,
    source: &dyn FrozenSource,
    open: StoreOpener<'_>,
    sink: &dyn EventSink,
) -> Result<Result<(Box<AcceptedBundle>, Verification), BundleRejection>, MigrationError> {
    match plan(schemas, source, sink)? {
        BundleVerdict::Rejected(rejection) => Ok(Err(*rejection)),
        BundleVerdict::Accepted(bundle) => {
            let store = open(StoreAccess::ReadOnly)?;
            let verification = verify_accepted(&bundle, store.as_ref(), schemas, sink)?;
            Ok(Ok((bundle, verification)))
        }
    }
}

/// What a rollback did.
#[derive(Debug, Clone, PartialEq)]
pub enum RollbackOutcome {
    ConfirmationMismatch { run: String, confirm: String },
    BundleRejected(BundleRejection),
    Refused(RollbackRefusal),
    RolledBack(RolledBackMigration),
}

/// `migration rollback`: `run` and `confirm` must name the same run; the
/// run must belong to the source's accepted plan.
pub fn rollback(
    schemas: &KernelSchemas,
    source: &dyn FrozenSource,
    open: StoreOpener<'_>,
    run: &MigrationRunId,
    confirm: &str,
    sink: &dyn EventSink,
) -> Result<RollbackOutcome, MigrationError> {
    if run.as_str() != confirm {
        return Ok(RollbackOutcome::ConfirmationMismatch {
            run: run.as_str().to_string(),
            confirm: confirm.to_string(),
        });
    }
    let bundle = match plan(schemas, source, sink)? {
        BundleVerdict::Rejected(rejection) => {
            return Ok(RollbackOutcome::BundleRejected(*rejection))
        }
        BundleVerdict::Accepted(bundle) => bundle,
    };
    let store = open(StoreAccess::ReadWrite)?;
    match store.rollback_migration(run, &bundle.facts.fingerprint) {
        Ok(rolled) => {
            emit(
                sink,
                EventKind::Change,
                format!("migration rollback: run {} restored", rolled.run.id),
            );
            Ok(RollbackOutcome::RolledBack(rolled))
        }
        Err(MigrationPortError::RollbackRefused(refusal)) => {
            emit(
                sink,
                EventKind::Check,
                format!("migration rollback: refused ({refusal})"),
            );
            Ok(RollbackOutcome::Refused(refusal))
        }
        Err(other) => Err(MigrationError::Storage(other)),
    }
}

/// `import --kind frozen-instance`: ONE bundle load, then the same apply
/// and verify routes as `migration apply`/`migration verify`.
#[derive(Debug, Clone, PartialEq)]
pub struct FrozenImport {
    pub bundle: Box<AcceptedBundle>,
    pub apply: ApplyOutcome,
    pub verification: Option<Verification>,
}

///
/// No database is opened, verified or created before `confirm` matches the
/// recomputed plan fingerprint. Only then is the tool database verified
/// (opened read-only through `open_tool`: it must exist with its role and
/// Kernel edition, and is never written by a frozen import), and only then
/// is the workspace database opened for the write.
pub fn import_frozen(
    schemas: &KernelSchemas,
    source: &dyn FrozenSource,
    open_tool: StoreOpener<'_>,
    open_workspace: StoreOpener<'_>,
    confirm: &str,
    sink: &dyn EventSink,
) -> Result<Result<FrozenImport, BundleRejection>, MigrationError> {
    let bundle = match plan(schemas, source, sink)? {
        BundleVerdict::Rejected(rejection) => return Ok(Err(*rejection)),
        BundleVerdict::Accepted(bundle) => bundle,
    };
    let intent = ApplyIntent::Apply {
        confirm: confirm.to_string(),
    };
    if let Some(mismatch) = confirmation_mismatch(&bundle, &intent) {
        return Ok(Ok(FrozenImport {
            bundle,
            apply: mismatch,
            verification: None,
        }));
    }
    drop(open_tool(StoreAccess::ReadOnly)?);
    let store = open_workspace(StoreAccess::ReadWrite)?;
    let apply = apply_accepted(&bundle, store.as_ref(), &intent, sink)?;
    let verification = if apply.is_positive() {
        Some(verify_accepted(&bundle, store.as_ref(), schemas, sink)?)
    } else {
        None
    };
    Ok(Ok(FrozenImport {
        bundle,
        apply,
        verification,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportEnvelopeDto {
    status: String,
    command: String,
    result: Vec<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CanonicalRecordDto {
    #[serde(rename = "$schema")]
    declared_schema: String,
    schema_version: u64,
    id: String,
    title: String,
    record_type: String,
    scope: ScopeDto,
    origin: OriginDto,
    authority: AuthorityDto,
    payload: Map<String, Value>,
}

/// Every record of a complete `export` envelope, accepted and converted,
/// with no two records sharing an identity.
fn accept_canonical(
    bytes: &[u8],
    schemas: &KernelSchemas,
) -> Result<Vec<PutRecordRequest>, MigrationError> {
    let input = |problems: Vec<String>| MigrationError::CanonicalInput(problems);
    let text = std::str::from_utf8(bytes).map_err(|_| input(vec!["not UTF-8".to_string()]))?;
    let value: Value =
        serde_json::from_str(text).map_err(|e| input(vec![format!("not valid JSON: {e}")]))?;
    let envelope: ExportEnvelopeDto = serde_json::from_value(value)
        .map_err(|e| input(vec![format!("not the closed export envelope: {e}")]))?;
    let mut problems = Vec::new();
    if envelope.command != "export" {
        problems.push(format!(
            "command is \"{}\", not \"export\"",
            envelope.command
        ));
    }
    if envelope.status != "ok" {
        problems.push(format!("status is \"{}\", not \"ok\"", envelope.status));
    }
    if !problems.is_empty() {
        return Err(input(problems));
    }
    let mut requests = Vec::with_capacity(envelope.result.len());
    for (i, record) in envelope.result.iter().enumerate() {
        match json_schema::validate(record, schemas.envelope()) {
            Ok(errors) if errors.is_empty() => {}
            Ok(errors) => {
                problems.extend(errors.into_iter().map(|e| format!("result[{i}]: {e}")));
                continue;
            }
            Err(e) => {
                problems.push(format!(
                    "result[{i}]: the envelope schema cannot be applied: {e}"
                ));
                continue;
            }
        }
        let converted = serde_json::from_value::<CanonicalRecordDto>(record.clone())
            .map_err(|e| e.to_string())
            .and_then(|dto| {
                let parsed = head(HeadParts {
                    declared_schema: dto.declared_schema,
                    schema_version: dto.schema_version,
                    id: dto.id,
                    title: dto.title,
                    record_type: dto.record_type,
                    scope: dto.scope,
                    origin: dto.origin,
                    authority: dto.authority,
                })?;
                // A product record's payload passes the one payload
                // validator (M-05b) before anything is written; the tool
                // database's built-in methodology is not a product record.
                if parsed.scope.scope_type() != ScopeType::BuiltInMethodology {
                    let subject = PayloadSubject {
                        id: &parsed.id,
                        scope: &parsed.scope,
                        title: parsed.title.as_str(),
                        record_type: parsed.record_type.as_str(),
                    };
                    schemas
                        .payloads()
                        .check(subject, &dto.payload)
                        .map_err(|r| format!("payload-contract: {}", r.problems.join("; ")))?;
                }
                canonical_request(&parsed, &dto.payload, record)
            });
        match converted {
            Ok(request) => requests.push(request),
            Err(message) => problems.push(format!("result[{i}]: {message}")),
        }
    }
    if !problems.is_empty() {
        return Err(input(problems));
    }
    let mut keys: Vec<String> = requests.iter().map(|r| r.key().storage_key()).collect();
    keys.sort();
    let duplicates: Vec<String> = keys
        .windows(2)
        .filter(|w| w[0] == w[1])
        .map(|w| w[0].replace('\u{1f}', "/"))
        .collect();
    if !duplicates.is_empty() {
        return Err(input(vec![format!(
            "records share an identity (scope and id): {duplicates:?}"
        )]));
    }
    Ok(requests)
}

/// Per-database effects of a canonical import.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RoleEffects {
    pub records: usize,
    pub effects: EffectCounts,
}

/// What a canonical-records import did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalOutcome {
    ConfirmationMismatch {
        expected: ContentDigest,
        given: String,
    },
    Imported {
        input_digest: ContentDigest,
        tool: RoleEffects,
        workspace: RoleEffects,
    },
    /// One database refused its batch; nothing of the import remains:
    /// the refusing database rolled back its own transaction and, when it
    /// was the workspace database, the tool database was restored from its
    /// checkpoint to `restored_tool_state`.
    WriteFailed {
        input_digest: ContentDigest,
        role: DatabaseRole,
        reason: String,
        restored_tool_state: Option<RecordState>,
    },
}

fn discard_all(stores: [(&dyn MigrationStore, Checkpoint); 2]) -> Result<(), MigrationError> {
    for (store, checkpoint) in stores {
        store.discard(checkpoint)?;
    }
    Ok(())
}

/// `import --kind canonical-records`.
pub fn import_canonical(
    schemas: &KernelSchemas,
    bytes: &[u8],
    confirm: &str,
    open_tool: StoreOpener<'_>,
    open_workspace: StoreOpener<'_>,
    sink: &dyn EventSink,
) -> Result<CanonicalOutcome, MigrationError> {
    let input_digest = ContentDigest::of_bytes(bytes);
    if confirm != input_digest.value() {
        return Ok(CanonicalOutcome::ConfirmationMismatch {
            expected: input_digest,
            given: confirm.to_string(),
        });
    }
    let requests = accept_canonical(bytes, schemas)?;
    emit(
        sink,
        EventKind::Check,
        format!(
            "import canonical-records: {} record(s) accepted",
            requests.len()
        ),
    );
    let (tool_requests, workspace_requests): (Vec<_>, Vec<_>) = requests
        .into_iter()
        .partition(|r| r.key().scope().scope_type() == ScopeType::BuiltInMethodology);
    let tool_store = open_tool(StoreAccess::ReadWrite)?;
    let workspace_store = open_workspace(StoreAccess::ReadWrite)?;
    let (tool, workspace) = (tool_store.as_ref(), workspace_store.as_ref());

    let tool_checkpoint = tool.checkpoint()?;
    let workspace_checkpoint = match workspace.checkpoint() {
        Ok(checkpoint) => checkpoint,
        Err(error) => {
            tool.discard(tool_checkpoint)?;
            return Err(error.into());
        }
    };
    let counts = |outcomes: &[PutRecordOutcome]| RoleEffects {
        records: outcomes.len(),
        effects: applied_effects(outcomes),
    };
    let tool_effects = match tool.put_batch(tool_requests) {
        Ok(outcomes) => counts(&outcomes),
        Err(error) => {
            discard_all([(tool, tool_checkpoint), (workspace, workspace_checkpoint)])?;
            return Ok(CanonicalOutcome::WriteFailed {
                input_digest,
                role: DatabaseRole::Tool,
                reason: error.to_string(),
                restored_tool_state: None,
            });
        }
    };
    let workspace_effects = match workspace.put_batch(workspace_requests) {
        Ok(outcomes) => counts(&outcomes),
        Err(error) => {
            let restored = match tool.restore(&tool_checkpoint) {
                Ok(state) => state,
                Err(compensation) => {
                    return Err(MigrationError::CompensationFailed {
                        failure: error.to_string(),
                        compensation: compensation.to_string(),
                    })
                }
            };
            emit(
                sink,
                EventKind::Correction,
                "import canonical-records: tool database restored from its checkpoint".to_string(),
            );
            discard_all([(tool, tool_checkpoint), (workspace, workspace_checkpoint)])?;
            return Ok(CanonicalOutcome::WriteFailed {
                input_digest,
                role: DatabaseRole::Workspace,
                reason: error.to_string(),
                restored_tool_state: Some(restored),
            });
        }
    };
    discard_all([(tool, tool_checkpoint), (workspace, workspace_checkpoint)])?;
    emit(
        sink,
        EventKind::Change,
        format!(
            "import canonical-records: {} tool and {} workspace record(s)",
            tool_effects.records, workspace_effects.records
        ),
    );
    Ok(CanonicalOutcome::Imported {
        input_digest,
        tool: tool_effects,
        workspace: workspace_effects,
    })
}

#[cfg(test)]
mod tests;
