//! [`RecordRepository`] — read and write managed records
//! (`meridian-rust-target-architecture.md` §3).
//!
//! This port is implemented by an adapter (`meridian-storage-sqlite`), never
//! by `meridian-app` itself: this crate defines the contract and
//! orchestrates calls to it, and does not open a database, a file, a
//! network connection or an environment variable to satisfy it.

use super::error::PortError;
use super::model::{
    ManagedRecord, PutRecordOutcome, PutRecordRequest, RecordKey, RecordRevision, RevisionNumber,
};

/// Reads and writes managed records in a permanent store
/// (`scoped-record.schema.json`).
pub trait RecordRepository {
    /// Creates or updates the record named by `request.key()`.
    ///
    /// Every call is idempotent: applying the exact same
    /// [`super::IdempotencyKey`] a second time for content whose digest is
    /// unchanged returns [`PutRecordOutcome::AlreadyApplied`] and creates no
    /// second revision; applying it again for content whose digest differs
    /// is rejected as [`PortError::IdempotencyConflict`], never silently
    /// accepted as a new revision.
    fn put(&self, request: PutRecordRequest) -> Result<PutRecordOutcome, PortError>;

    /// Applies every request in `requests` as one atomic unit: either all of
    /// them take effect, in order, or — on the first error — none of them
    /// do. A partial batch is never observable by a later read.
    fn put_batch(
        &self,
        requests: Vec<PutRecordRequest>,
    ) -> Result<Vec<PutRecordOutcome>, PortError>;

    /// The current state of the record named by `key`, or `Ok(None)` when no
    /// such record exists — genuine absence is never an error.
    fn get(&self, key: &RecordKey) -> Result<Option<ManagedRecord>, PortError>;

    /// One specific past revision of the record named by `key`, or
    /// `Ok(None)` when that record or that revision does not exist.
    fn get_revision(
        &self,
        key: &RecordKey,
        revision_number: RevisionNumber,
    ) -> Result<Option<RecordRevision>, PortError>;

    /// Every revision of the record named by `key`, oldest first, or an
    /// empty vector when the record does not exist.
    fn list_revisions(&self, key: &RecordKey) -> Result<Vec<RecordRevision>, PortError>;

    /// Every current record in storage, in a stable, deterministic order
    /// (`storage_key` order) — the basis of the canonical export
    /// (`meridian-rust-target-architecture.md` §4.2, "Канонический
    /// экспорт"). Two calls against unchanged state return an identical
    /// sequence.
    fn export_all(&self) -> Result<Vec<ManagedRecord>, PortError>;
}
