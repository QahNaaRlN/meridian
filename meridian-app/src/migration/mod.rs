//! The migration application layer of package `meridian-cli-migration`
//! (`meridian-rust-migration-program-plan.md` §5.22): orchestration of
//! `import` and `migration plan|apply|verify|rollback` over ports only.
//!
//! ```text
//! FrozenSource port (one source view) + WorkspaceReader (Kernel schemas)
//!   -> bundle: the accepted rust-architecture-conformance-7 operations
//!      (instance-data-migration, instance-canonical-export against the
//!      pinned plan) + the actual unit content of the pinned revision
//!   -> meridian-core: FrozenWriteSet, verification diff, run decisions,
//!      applicability equivalence
//!   -> MigrationRepository + RecordRepository ports (one transaction per
//!      apply, a verified checkpoint per run)
//!   -> typed outcomes the CLI presents
//! ```
//!
//! This module opens no file, database, process or network connection.

pub mod bundle;
mod fragment;
pub mod operations;
mod records;
pub mod source;

pub use bundle::{
    load_bundle, AcceptedBundle, ApplicabilityBasis, BundleError, BundleRejection, BundleVerdict,
    KernelSchemas, PlanFacts, SourceIdentity,
};
pub use operations::{
    apply, apply_accepted, import_canonical, import_frozen, plan, rollback, verify,
    verify_accepted, ApplyIntent, ApplyOutcome, CanonicalOutcome, DryRunDecision, EffectCounts,
    FrozenImport, MigrationError, MigrationStore, RoleEffects, RollbackOutcome, StoreAccess,
    StoreOpener, Verification,
};
pub use source::{BundleFile, FrozenSource, PinnedTree, SourceError};
