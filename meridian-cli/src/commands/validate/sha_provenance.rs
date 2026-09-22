//! Presentation/composition shim over the real port-based implementation:
//! `meridian_app::validation::mechanical_integrity::sha_provenance`, reading
//! through `crate::adapters::workspace_reader::FsWorkspaceReader`. No
//! predicate logic, `serde_json::Value`, regex or direct `std::fs` lives in
//! this crate for this check any more
//! (`meridian-cli-foundation-architecture-remediation`,
//! `meridian-rust-migration-program-plan.md` §5.16).

use std::path::Path;

use meridian_app::validation::mechanical_integrity::sha_provenance as app;
use meridian_app::validation::mechanical_integrity::OperationError;

use super::split_diagnostics;
use crate::adapters::workspace_reader::FsWorkspaceReader;

pub struct Outcome {
    pub failures: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn run(kernel_root: &Path) -> Result<Outcome, OperationError> {
    let reader = FsWorkspaceReader::new(kernel_root);
    let outcome = app::run(&reader)?;
    let (failures, warnings) = split_diagnostics(outcome.diagnostics);
    Ok(Outcome { failures, warnings })
}
