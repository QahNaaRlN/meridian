//! Pure composite-consistency algorithms for the operating-model registries
//! `meridian validate` checks (subpackage 7b,
//! `governance/plans/meridian-rust-migration-program-plan.md` §5.5c). Each
//! submodule is a verbatim port of one `scripts/lib/*.mjs` module: it takes
//! an already-parsed [`serde_json::Value`] document plus the schema(s) it
//! composes with, and returns a flat list of problem strings — empty means
//! valid. None of them read a file, spawn Git, touch an environment
//! variable or exit a process; that I/O, and the JSON Schema/YAML parsing
//! that produces the values these functions take, stays in
//! `meridian-cli`'s own `commands::validate` modules, exactly as
//! [`crate::source_format`] itself owns no I/O.

pub mod bounded_context_manifest;
pub mod execution_state;
pub mod functional_parity;
pub mod instruction_source_registry;
pub mod role_and_human_control;
pub mod task_pattern_registry;
pub mod task_specification;
