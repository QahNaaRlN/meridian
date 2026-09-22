//! Presentation/composition shim over
//! `meridian_app::validation::mechanical_integrity::agent_instruction_identity`,
//! reading through `crate::adapters::workspace_reader::FsWorkspaceReader`
//! (`meridian-cli-foundation-architecture-remediation`). This shim's only
//! remaining job is concrete filesystem adaptation: turning each absolute
//! `PathBuf` the CLI's own (out-of-scope) file walk found into a validated
//! `WorkspaceRelativePath`. Which of those paths are actual candidates —
//! `carries_own_front_matter`, sort, dedup — and everything else about §7
//! now lives entirely in `meridian-app` (corrective round, item 3): this
//! module no longer imports `document_identity` at all.
//!
//! A path that does not convert to a `WorkspaceRelativePath` is never
//! silently dropped from the input list — it stops the whole check with a
//! typed [`OperationError`], the same discipline every other read failure
//! in this package follows.

use std::path::{Path, PathBuf};

use meridian_app::validation::mechanical_integrity::agent_instruction_identity as app;
use meridian_app::validation::mechanical_integrity::instruction_topics::TopicPool;
use meridian_app::validation::mechanical_integrity::OperationError;
use meridian_core::types::WorkspaceRelativePath;

use super::split_diagnostics;
use crate::adapters::workspace_reader::{workspace_relative, FsWorkspaceReader};

pub struct Outcome {
    pub failures: Vec<String>,
    pub declared_norms: usize,
    pub undeclared_prescriptive: usize,
    pub undeclared_other: usize,
}

pub fn run(
    kernel_root: &Path,
    markdown_files: &[PathBuf],
    topic_pool: Option<&TopicPool>,
) -> Result<Outcome, OperationError> {
    let mut all_paths: Vec<WorkspaceRelativePath> = Vec::with_capacity(markdown_files.len());
    for absolute in markdown_files {
        let relative =
            workspace_relative(kernel_root, absolute).map_err(|error| OperationError {
                path: absolute.display().to_string(),
                message: format!("not a valid workspace-relative path: {error}"),
            })?;
        all_paths.push(relative);
    }

    let reader = FsWorkspaceReader::new(kernel_root);
    let outcome = app::run(&reader, &all_paths, topic_pool)?;
    let (failures, _warnings) = split_diagnostics(outcome.diagnostics);
    Ok(Outcome {
        failures,
        declared_norms: outcome.declared_norms,
        undeclared_prescriptive: outcome.undeclared_prescriptive,
        undeclared_other: outcome.undeclared_other,
    })
}
