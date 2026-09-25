//! Application-layer storage ports (`meridian-rust-target-architecture.md`
//! §3, package `sqlite-storage-adapter`).
//!
//! [`RecordRepository`] and [`EvidenceRepository`] are traits only: this
//! crate defines them, and the strict domain types
//! ([`meridian_core::types`]) their signatures are built from, but does not
//! implement either. `meridian-storage-sqlite` is the sole adapter that
//! implements them, and the sole crate that knows SQLite exists
//! (`meridian-rust-sqlite-architecture.md` §"Принятое решение" 4).

mod database_metadata;
mod database_role;
mod error;
mod evidence_repository;
mod migration_repository;
mod model;
mod record_repository;
mod role;

pub use database_metadata::DatabaseMetadata;
pub use database_role::{DatabaseRole, DatabaseRoleError};
pub use error::PortError;
pub use evidence_repository::EvidenceRepository;
pub use migration_repository::{
    AppliedMigration, Checkpoint, MigrationApplyRequest, MigrationPortError, MigrationRepository,
    RolledBackMigration,
};
pub use model::{
    IdempotencyKey, IdempotencyKeyError, ManagedRecord, Payload, PayloadError, PutEvidenceRequest,
    PutRecordOutcome, PutRecordRequest, PutRecordRequestError, RecordKey, RecordRevision,
    RecordSchemaVersion, RevisionNumber, SchemaRef, SchemaRefError, StoredEvidence,
};
pub use record_repository::RecordRepository;
pub use role::{RoledStorage, RouterError, StorageRouter};
