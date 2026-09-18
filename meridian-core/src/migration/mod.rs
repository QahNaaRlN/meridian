//! Instance-data migration plans: domain structures (for example
//! [`MigrationPlan`]) and the checks expressing the nine properties of
//! `standards/workspace/instance-data-migration.md` §1–9
//! ([`checks`]).
//!
//! `meridian-core` does not resolve a source snapshot, a piece of evidence,
//! a predecessor plan or rollback/restoration data itself — it does not
//! know `EvidenceRepository`, SQLite, Git or the filesystem exist. Every
//! checking function that needs one of those external facts takes it as an
//! already-resolved parameter ([`resolved`]).

mod base64;
mod canonical;
pub mod checks;
pub mod resolved;
mod types;

pub use types::{
    ContentEnvelope, ContentEnvelopeError, Disposition, Encoding, FieldBasis, FieldBasisValue,
    Mapping, MigrationPlan, MigrationTypeError, OwnerDecision, Qualification, RecordUnit, Rollback,
    RollbackPlan, SourceState, SubVerdict, TargetOrigin, TargetRecord, Verification,
};
