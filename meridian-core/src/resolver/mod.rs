//! The rule resolver: applicable norms, protocols, verification and
//! conflicts as a pure function of a work item and explicitly-supplied
//! source data (`standards/workspace/rule-resolution.md`,
//! `registries/rule-resolution/{applicability,resolver-output}.schema.json`).
//!
//! This is `scripts/rule-resolver.mjs`'s pure core (`resolveRules` and
//! `finalize`), ported line-by-line: the same scopes (universal, profile,
//! repository, product-domain), the same activation modes (always,
//! path-glob, task-class, explicit, undetermined), the same
//! `requires_reresolution`/initiative-decomposition/`supersedes`/fail-closed
//! rules. Not ported: file I/O, argument parsing, environment variables,
//! JSON/YAML and the CLI `main()` — those stay in the Node reference until
//! package `rust-rule-resolution` carries this algorithm into `meridian-app`.

mod applicability;
mod date;
mod glob;
mod output;
mod resolve;
mod sources;
mod work_item;

pub use applicability::{
    Activation, ActivationKind, ApplicabilityError, ApplicabilityRecord, ApplicabilityScope,
    ApplicabilitySource, ApplicabilitySourceKind, ApplicabilityStatus, Delivery, IntakePointer,
    IntakeVerdict, NormRef, SupersedesPointer, TaskClassSelector,
};
pub use date::{IsoDate, IsoDateError};
pub use glob::{glob_matches, CompiledGlob, GlobError};
pub use output::{
    ApplicableNorm, ApplicableProtocol, Conflict, DecompositionBlocker, ResolverOutput, RouteKey,
    RouteProvenance, RouteScope, RouteSource, Unresolved,
};
pub use resolve::{resolve_rules, ResolverError};
pub use sources::{
    Decomposition, IntakeRecordEntry, IntakeRegister, PreviousResolution, PriorState,
    ProtocolRoute, ProtocolRouteError, ProtocolRouteScope, RefactorFinding,
    RepositoryInventoryEntry, ResolverSources, RouteField, VerificationRoute, VerificationTarget,
};
pub use work_item::{
    ChangeClass, DeclaredProfiles, WorkItem, WorkItemError, WorkItemKind, WorkKind,
};
