//! Presentation/composition shim over
//! `meridian_app::validation::mechanical_integrity::instruction_topics`,
//! reading through `crate::adapters::workspace_reader::FsWorkspaceReader`
//! (`meridian-cli-foundation-architecture-remediation`).

use std::path::Path;

use meridian_app::validation::mechanical_integrity::instruction_topics as app;
use meridian_app::validation::mechanical_integrity::OperationError;

pub use app::TopicPool;

use super::split_diagnostics;
use crate::adapters::workspace_reader::FsWorkspaceReader;

pub struct Outcome {
    pub failures: Vec<String>,
    pub topic_pool: Option<TopicPool>,
}

pub fn run(kernel_root: &Path) -> Result<Outcome, OperationError> {
    let reader = FsWorkspaceReader::new(kernel_root);
    let outcome = app::run(&reader)?;
    let (failures, _warnings) = split_diagnostics(outcome.diagnostics);
    Ok(Outcome {
        failures,
        topic_pool: outcome.topic_pool,
    })
}
