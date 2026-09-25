//! Typed concepts and pure checks for `existing-project-compatibility-mode`
//! (`rust-architecture-conformance-2`, corrective round `CHANGES_REQUESTED`
//! item 1): workspace connection, discovery policy (the discovery plan),
//! discovery finding, connection state (scan kind / connection mode /
//! discovery status), compatibility next step, and the pure checks over
//! them.
//!
//! Moved here from `meridian-app::operating_model::existing_project_compatibility_mode::domain`
//! by the corrective round: an earlier pass kept these types in
//! `meridian-app`, reasoning that — unlike
//! [`crate::instruction_source`] — nothing else reused them; the architect
//! rejected that reasoning as inconsistent with the target architecture
//! (`meridian-core` owns predicate algorithms and the types whose invalid
//! states are made unrepresentable by construction, `meridian-app` owns
//! transport, DTO->domain conversion, composition and orchestration —
//! reuse by a second contract is not the test). `meridian-app`'s own
//! `domain.rs` now re-exports these names unchanged so every existing
//! `domain::X` import path in that crate keeps working.
//!
//! Like the rest of `meridian-core`, nothing here touches `serde`,
//! `serde_json::Value`, the filesystem, Git or process I/O; every
//! constructor takes already-syntax-checked scalars and either builds a
//! valid value or returns [`crate::types::Diagnostic`]s. In particular,
//! [`checks::check_discovery_outcomes`] takes typed ID PROJECTIONS
//! (`SemanticId` slices/sets), not the full domain aggregates — a
//! corrective-round fix (item 2-4): the outcome-partition count must
//! reflect every OCCURRENCE of a plan id among `discovered_sources`,
//! including a repeated id and an id whose OTHER fields failed domain
//! construction, so it cannot be computed from `Vec<DiscoveryPlanSlot>` or
//! a deduplicated `Set` the caller happened to build for some other
//! purpose.

mod types;

pub mod checks;

pub use types::{
    build_connection_scope, ConnectionMode, DiscoveryPlanSlot, DiscoveryStatus, Finding,
    FindingKind, ManagedModeDecision, MissingSource, NextStep, PreviousKnowledge, RepositoryRef,
    ScanKind, UnreadableSource,
};

use crate::types::{Diagnostic, DiagnosticLevel};

pub(crate) fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message).expect("message is non-empty")
}
