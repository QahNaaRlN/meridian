//! Presentation/composition shim over
//! `meridian_app::validation::mechanical_integrity::stack_profiles`, reading
//! through `crate::adapters::workspace_reader::FsWorkspaceReader`
//! (`meridian-cli-foundation-architecture-remediation`).

use std::path::Path;

use meridian_app::validation::mechanical_integrity::stack_profiles as app;
use meridian_app::validation::mechanical_integrity::OperationError;

use super::split_diagnostics;
use crate::adapters::workspace_reader::FsWorkspaceReader;

pub struct Outcome {
    pub failures: Vec<String>,
    pub pool_loaded: bool,
}

pub fn run(kernel_root: &Path) -> Result<Outcome, OperationError> {
    let reader = FsWorkspaceReader::new(kernel_root);
    let outcome = app::run(&reader)?;
    let (failures, _warnings) = split_diagnostics(outcome.diagnostics);
    Ok(Outcome {
        failures,
        pool_loaded: outcome.pool.is_some(),
    })
}
