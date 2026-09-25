//! Re-export shim (`rust-architecture-conformance-2`, corrective round,
//! item 1): the `existing-project-compatibility-mode` domain types and pure
//! checks moved to [`meridian_core::existing_project_compatibility_mode`] —
//! an earlier round of this package kept them here, in `meridian-app`,
//! reasoning that nothing else reused them; the architect rejected that
//! reasoning (reuse by a second contract is not the test for what belongs
//! in `meridian-core` — see the core module's own doc comment). This file
//! defines NOTHING of its own: every name below is the SAME item `core`
//! defines, re-exported so `convert.rs`/`mod.rs`'s existing `domain::X`
//! import paths in this crate keep working unchanged.

pub use meridian_core::existing_project_compatibility_mode::checks::{
    check_changes_have_findings, check_discovery_outcomes, check_managed_mode,
    check_repository_scope, compute_next_step,
};
pub use meridian_core::existing_project_compatibility_mode::{
    build_connection_scope, ConnectionMode, DiscoveryPlanSlot, DiscoveryStatus, Finding,
    FindingKind, ManagedModeDecision, MissingSource, NextStep, PreviousKnowledge, RepositoryRef,
    ScanKind, UnreadableSource,
};
