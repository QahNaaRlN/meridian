//! Thin re-export facade over
//! [`meridian_core::task_contracts::portability`] (`rust-architecture-conformance-3`
//! corrective round item 2): `non_portable_reason` and `resolve_schema_ref`
//! moved into `meridian-core` so
//! [`meridian_core::task_contracts::specification::check_task_specification`]
//! could gate its own [`meridian_core::task_contracts::TaskSpecification`]
//! construction on portability itself, rather than leaving a second,
//! optional pass to this crate's orchestration.
//!
//! This module stays in place, unchanged in its public shape, for its one
//! remaining consumer: `super::task_specification`'s own
//! `$schema`-envelope check. The run-contract, evidence-and-handoff and
//! field-evaluation families check portability inside `meridian-core`,
//! which calls the core owner directly (`rust-architecture-conformance-6`
//! removed the last facade consumers).

pub use meridian_core::task_contracts::{non_portable_reason, resolve_schema_ref};

/// The canonical logical base every operating-model record's `$schema`
/// reference is written relative to.
pub use meridian_core::task_contracts::CANONICAL_RECORD_BASE;
