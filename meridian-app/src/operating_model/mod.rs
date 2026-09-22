//! Composite-consistency algorithms for the operating-model registries
//! `meridian validate` checks (subpackage 7b,
//! `governance/plans/meridian-rust-migration-program-plan.md` §5.5c).
//! Node.js is each submodule's fixture and semantics source, never its
//! target structure (`standards/workspace/rust-migration-quality.md`).
//!
//! [`controlled_rule_intake`] is the `rust-architecture-conformance-1`
//! reference pilot: its domain types and pure checks live in
//! [`meridian_core::controlled_rule_intake`], this crate owns only its
//! closed transport DTOs, DTO -> domain conversion and orchestration, and
//! it returns typed [`meridian_core::types::Diagnostic`]s, not strings. The
//! remaining submodules are not yet converted: each still takes an
//! already-parsed [`serde_json::Value`] document plus the schema(s) it
//! composes with, and returns a flat list of problem strings — empty means
//! valid. None of them read a file, spawn Git, touch an environment
//! variable or exit a process; that I/O, and the JSON Schema/YAML parsing
//! that produces the values these functions take, stays in
//! `meridian-cli`'s own `commands::validate` modules, exactly as
//! [`crate::source_format`] itself owns no I/O.

pub mod bounded_context_manifest;
pub mod controlled_rule_intake;
pub mod evidence_and_handoff;
pub mod execution_state;
pub mod existing_project_compatibility_mode;
pub mod field_evaluation;
pub mod functional_parity;
pub mod instruction_source_registry;
pub mod reference_portability;
pub mod role_and_human_control;
pub mod task_pattern_registry;
pub mod task_specification;
