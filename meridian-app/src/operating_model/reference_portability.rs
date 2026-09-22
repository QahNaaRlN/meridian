//! Thin re-export facade over
//! [`meridian_core::task_contracts::portability`] (`rust-architecture-conformance-3`
//! corrective round item 2): `non_portable_reason` and `resolve_schema_ref`
//! moved into `meridian-core` so
//! [`meridian_core::task_contracts::specification::check_task_specification`]
//! could gate its own [`meridian_core::task_contracts::TaskSpecification`]
//! construction on portability itself, rather than leaving a second,
//! optional pass to this crate's orchestration.
//!
//! This module stays in place, unchanged in its public shape, so every
//! existing consumer — `super::task_specification`'s own `$schema`-envelope
//! check, and (through that module's still-documented temporary facade)
//! [`super::execution_state`], [`super::role_and_human_control`],
//! [`super::bounded_context_manifest`], [`super::evidence_and_handoff`] and
//! [`super::field_evaluation`] — keeps importing from exactly the same path
//! it already did; only the definitions' home crate changed.

pub use meridian_core::task_contracts::{non_portable_reason, resolve_schema_ref};

/// The canonical logical base every operating-model record's `$schema`
/// reference is written relative to.
pub use meridian_core::task_contracts::CANONICAL_RECORD_BASE;
