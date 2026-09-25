//! `validate --workspace-db` (`rust-workspace-state-validation`,
//! `meridian-rust-migration-program-plan.md` §5.23.11): composition and
//! presentation only. The concrete adapters — [`FsWorkspaceReader`] over
//! the Kernel, [`RealRepositoryAccess`] over the product repositories,
//! [`SystemClock`] — are handed to the one app operation
//! `meridian_app::workspace_state::validate`, and its typed report becomes
//! the command's failure/warning lines and its `workspace_state` object.
//! No domain rule lives here. `--log-metrics` is the one opt-in effect:
//! [`record_metrics`] appends one gate-run observation after the whole
//! result exists and never changes it.

use std::path::{Path, PathBuf};

use meridian_app::storage::{PutRecordOutcome, RecordRepository};
use meridian_app::workspace_state::{
    self as app, KernelSide, PayloadRegistry, Ports, RepositoryAccess, WorkspaceStateCounts,
};
use meridian_core::mechanical_integrity::instruction_topics::TopicPool;
use meridian_core::types::{DiagnosticLevel, Scope, WorkspaceRelativePath};
use meridian_core::workspace_state::observation::GateRunObservation;
use meridian_core::workspace_state::timestamp::Timestamp;
use serde_json::{json, Value};

use super::CollectError;
use crate::adapters::clock::SystemClock;
use crate::adapters::git_inspector::RealRepositoryAccess;
use crate::adapters::workspace_reader::{workspace_relative, FsWorkspaceReader};

/// What the DB-backed half contributes to one `validate` run.
pub struct Outcome {
    pub failures: Vec<String>,
    pub warnings: Vec<String>,
    pub summary: Summary,
}

/// The part of the report presentation and metrics still need.
pub struct Summary {
    pub counts: WorkspaceStateCounts,
    pub infos: Vec<String>,
    pub now: Timestamp,
    pub workspace: Result<Scope, String>,
}

impl Summary {
    pub fn to_json(&self, database: &str) -> Value {
        json!({
            "database": database,
            "records": self.counts.records,
            "rejected_payloads": self.counts.rejected_payloads,
            "repositories": self.counts.repositories,
            "repositories_confirmed": self.counts.repositories_confirmed,
            "intake_records": self.counts.intake_records,
            "intake_repositories_complete": self.counts.intake_repositories_complete,
            "observations": self.counts.observations,
            "info": self.infos,
        })
    }
}

/// Runs the operation over the Kernel files `kernel-purity` scans.
pub fn run(
    kernel_root: &Path,
    kernel_files: &[PathBuf],
    stack_profile_pool_loaded: bool,
    topic_pool: Option<&TopicPool>,
    records: &dyn RecordRepository,
) -> Result<Outcome, CollectError> {
    let files: Vec<WorkspaceRelativePath> = kernel_files
        .iter()
        .filter_map(|f| workspace_relative(kernel_root, f).ok())
        .collect();
    let kernel = FsWorkspaceReader::new(kernel_root);
    let payloads =
        PayloadRegistry::load(&kernel).map_err(|e| CollectError::WorkspaceState(e.to_string()))?;
    let kernel_repository = kernel_root.display().to_string();
    let report = app::validate(
        &KernelSide {
            reader: &kernel,
            files: &files,
            stack_profile_pool_loaded,
            topic_pool,
            repository_root: &kernel_repository,
        },
        &Ports {
            records,
            repositories: &RealRepositoryAccess,
            clock: &SystemClock,
        },
        &payloads,
    )
    .map_err(|e| CollectError::WorkspaceState(e.to_string()))?;

    let mut failures: Vec<String> = report
        .purity
        .iter()
        .map(|f| {
            f.finding
                .message(&kernel_root.join(f.path.as_str()).display().to_string())
        })
        .collect();
    let mut warnings = Vec::new();
    let mut infos = Vec::new();
    for diagnostic in report.diagnostics {
        let message = diagnostic.message().to_string();
        match diagnostic.level() {
            DiagnosticLevel::Fail => failures.push(message),
            DiagnosticLevel::Warn => warnings.push(message),
            DiagnosticLevel::Info => infos.push(message),
        }
    }
    Ok(Outcome {
        failures,
        warnings,
        summary: Summary {
            counts: report.counts,
            infos,
            now: report.now,
            workspace: report.workspace,
        },
    })
}

/// M-18: one observation of the finished run, written after the result
/// exists. The returned object is the `metrics` member of the result; a
/// failure to write is reported there (and by the caller on stderr), never
/// folded into the validation verdict.
pub fn record_metrics(
    kernel_root: &Path,
    kernel_edition: &str,
    summary: &Summary,
    exit_code: i32,
    failures: &[String],
    warnings: &[String],
    records: &dyn RecordRepository,
) -> Result<Value, String> {
    let workspace = summary.workspace.clone()?;
    let kernel_revision = RealRepositoryAccess
        .vcs_state(&kernel_root.display().to_string())
        .ok()
        .map(|vcs| vcs.revision);
    let observation = GateRunObservation::of_run(
        summary.now,
        kernel_edition.to_string(),
        kernel_revision,
        exit_code,
        failures,
        warnings,
        summary.infos.len(),
    )
    .map_err(|e| e.to_string())?;
    let payloads =
        PayloadRegistry::load(&FsWorkspaceReader::new(kernel_root)).map_err(|e| e.to_string())?;
    let request = app::observation_request(&observation, &workspace, &payloads)?;
    let record_id = request.key().id().as_str().to_string();
    let outcome = records.put(request).map_err(|e| e.to_string())?;
    let status = match outcome {
        PutRecordOutcome::Created(_) | PutRecordOutcome::Updated(_) => "recorded",
        PutRecordOutcome::AlreadyApplied(_) => "already-recorded",
    };
    Ok(json!({"outcome": status, "record_id": record_id}))
}
