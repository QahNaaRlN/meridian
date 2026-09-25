//! Domain types and pure checks for `controlled-rule-intake`
//! (`standards/workspace/controlled-rule-intake.md`,
//! `registries/operating-model/controlled-rule-intake.schema.json`) —
//! moved into `meridian-core` by the `rust-architecture-conformance-1`
//! corrective round (item 4): this crate owns the predicate algorithms and
//! the types whose invalid states are made unrepresentable by
//! construction. It depends on nothing outside `meridian-core` itself — no
//! serde, no JSON, no ports, no filesystem, no Git.
//!
//! `meridian-app`'s own `operating_model::controlled_rule_intake` owns
//! the closed transport DTOs, the DTO -> domain conversion (which DOES
//! touch `serde_json::Value`, legitimately, at that boundary — see
//! `standards/workspace/rust-migration-quality.md` §4) and orchestration.
//! This crate never receives a `Value`; every function here takes an
//! already-validated domain type and returns one.
//!
//! [`types::RuleCandidate`] can only be built through
//! [`types::RuleCandidate::try_new`], which itself runs the cross-field
//! consistency checks (origin/source link, owner authority, decision_ref)
//! before allowing construction — an inconsistent candidate is
//! unrepresentable, not merely flagged after the fact (item 5). Checks that
//! need more than one candidate ([`checks::check_cluster`],
//! [`checks::check_conflict_graph`]) or external data
//! ([`checks::check_source_ref`]) stay separate, pure functions over
//! already-valid [`types::RuleCandidate`] values.

mod types;

pub mod checks;

// `Currency`/`RecordedState`/`ResolvedInstructionSource` used to be defined
// here (`resolved.rs`); `rust-architecture-conformance-2` (§1) moved them to
// `crate::instruction_source`, canonical for any resolved instruction-source
// snapshot, not specific to this contract. Re-exported unchanged so every
// existing `meridian_core::controlled_rule_intake::{Currency, RecordedState,
// ResolvedInstructionSource}` import keeps working.
pub use crate::instruction_source::{Currency, RecordedState, ResolvedInstructionSource};
pub use types::{
    Applicability, Boundary, BoundaryError, NotApplicableReason, PinnedSourceRef, RuleCandidate,
    RuleCandidatePayload, SemanticKey, RECORD_TYPE, SOURCE_RECORD_TYPE,
};

use crate::types::{Diagnostic, DiagnosticLevel};

pub(crate) fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message).expect("message is non-empty")
}
