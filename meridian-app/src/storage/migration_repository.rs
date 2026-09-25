//! [`MigrationRepository`] — the adapter-neutral port through which one
//! database applies, verifies and rolls back a migration run
//! (`meridian-rust-migration-program-plan.md` §5.22.4 items 3, 9–11;
//! §5.22.5).
//!
//! The port names no path, file, driver or SQL. How a checkpoint is stored,
//! where, and how it is restored is the adapter's own concern; the port
//! only promises the semantics: a checkpoint is a consistent copy of the
//! exact pre-state taken through the store's own backup mechanism, a
//! migration's writes and its journal row commit together or not at all,
//! and a rollback restores a verified checkpoint without deleting single
//! append-only revisions.

use core::fmt;

use meridian_core::migration::run::{MigrationRun, MigrationRunId, RecordState, RollbackRefusal};
use meridian_core::types::ContentDigest;

use super::error::PortError;
use super::model::{PutRecordOutcome, PutRecordRequest};

/// Everything one frozen-plan apply writes: the plan identity the journal
/// row records, and the accepted records, in order.
#[derive(Debug, Clone, PartialEq)]
pub struct MigrationApplyRequest {
    pub plan_ref: String,
    pub plan_fingerprint: ContentDigest,
    pub idempotency_key: ContentDigest,
    pub source_repository_ref: String,
    pub source_revision: String,
    pub records: Vec<PutRecordRequest>,
}

/// A committed apply: the journal row and every record outcome.
#[derive(Debug, Clone, PartialEq)]
pub struct AppliedMigration {
    pub run: MigrationRun,
    pub outcomes: Vec<PutRecordOutcome>,
}

/// A recorded rollback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolledBackMigration {
    /// The run as the journal now records it: its unchanged applied fact
    /// and its separate rollback fact.
    pub run: MigrationRun,
    /// The record state of the reopened, restored database — equal to the
    /// run's pre-state by construction of a successful rollback.
    pub restored: RecordState,
}

/// A verified checkpoint of one database's exact current state, owned by
/// the adapter that took it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    name: String,
    digest: ContentDigest,
    state: RecordState,
}

impl Checkpoint {
    /// Built only by an adapter from what it actually wrote and hashed.
    pub fn new(name: String, digest: ContentDigest, state: RecordState) -> Self {
        Self {
            name,
            digest,
            state,
        }
    }
    /// The adapter's opaque name of the checkpoint.
    pub fn name(&self) -> &str {
        &self.name
    }
    /// The digest of the checkpoint itself.
    pub fn digest(&self) -> &ContentDigest {
        &self.digest
    }
    /// The record state the checkpoint holds.
    pub fn state(&self) -> &RecordState {
        &self.state
    }
}

/// Why a migration-repository operation did not complete. Every variant
/// leaves the database in the state it was in before the call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationPortError {
    /// A record write failed; the whole batch and its journal row rolled
    /// back.
    Record(PortError),
    /// A run already carries this idempotency key.
    RunAlreadyRecorded { run: MigrationRunId },
    /// The database changed between the pre-state read and the write
    /// transaction; nothing was written.
    ConcurrentChange,
    /// The rollback was refused before any restore.
    RollbackRefused(RollbackRefusal),
    /// The checkpoint could not be written, read or restored.
    Checkpoint(String),
    /// This adapter instance cannot take checkpoints (no location given).
    CheckpointsUnavailable,
    /// Any other storage failure.
    Storage(String),
}

impl fmt::Display for MigrationPortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MigrationPortError::Record(error) => write!(f, "{error}"),
            MigrationPortError::RunAlreadyRecorded { run } => write!(
                f,
                "migration run \"{run}\" already carries this plan's idempotency key"
            ),
            MigrationPortError::ConcurrentChange => write!(
                f,
                "the database changed while the migration was being prepared; nothing was written"
            ),
            MigrationPortError::RollbackRefused(refusal) => write!(f, "{refusal}"),
            MigrationPortError::Checkpoint(message) => write!(f, "checkpoint error: {message}"),
            MigrationPortError::CheckpointsUnavailable => {
                write!(f, "this storage has no checkpoint location")
            }
            MigrationPortError::Storage(message) => write!(f, "storage error: {message}"),
        }
    }
}

impl std::error::Error for MigrationPortError {}

impl From<PortError> for MigrationPortError {
    fn from(error: PortError) -> Self {
        MigrationPortError::Record(error)
    }
}

/// Applies, inspects and rolls back migration runs of one database.
pub trait MigrationRepository {
    /// The current record state: canonical-export digest and revision count.
    fn record_state(&self) -> Result<RecordState, MigrationPortError>;

    /// The run recorded under `idempotency_key`, if any.
    fn run_by_idempotency_key(
        &self,
        idempotency_key: &ContentDigest,
    ) -> Result<Option<MigrationRun>, MigrationPortError>;

    /// The run named `id`, if any.
    fn run(&self, id: &MigrationRunId) -> Result<Option<MigrationRun>, MigrationPortError>;

    /// The newest recorded run, if any.
    fn newest_run(&self) -> Result<Option<MigrationRun>, MigrationPortError>;

    /// Takes a checkpoint of the exact current state, then writes every
    /// record and the `applied` journal row in ONE transaction. Any failure
    /// rolls the whole transaction back and discards the checkpoint.
    fn apply_migration(
        &self,
        request: MigrationApplyRequest,
    ) -> Result<AppliedMigration, MigrationPortError>;

    /// Verifies the run's preconditions and checkpoint, restores the
    /// checkpoint, reopens the database and verifies the restored state
    /// against the run's pre-state. The restore never replaces recorded
    /// history: afterwards the journal holds the run's original applied
    /// fact unchanged AND a separate append-only rollback fact, and the
    /// restore and both facts land together or not at all.
    fn rollback_migration(
        &self,
        run: &MigrationRunId,
        plan_fingerprint: &ContentDigest,
    ) -> Result<RolledBackMigration, MigrationPortError>;

    /// A standalone checkpoint of the exact current state (the compensation
    /// basis of a two-database import).
    fn checkpoint(&self) -> Result<Checkpoint, MigrationPortError>;

    /// Restores `checkpoint` after verifying it, and returns the reopened
    /// database's record state (equal to the checkpoint's own).
    fn restore(&self, checkpoint: &Checkpoint) -> Result<RecordState, MigrationPortError>;

    /// Removes a standalone checkpoint that is no longer needed.
    fn discard(&self, checkpoint: Checkpoint) -> Result<(), MigrationPortError>;
}
