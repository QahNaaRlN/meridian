//! Presentation/composition shim over the real port-based implementation:
//! `meridian_app::operating_model::task_pattern_registry`, reading through
//! `crate::adapters::workspace_reader::FsWorkspaceReader` and
//! `crate::adapters::link_target::FsLinkTarget`. No predicate logic,
//! `serde_json::Value`, schema navigation, fixture loop or direct
//! `std::fs`/`std::process` lives in this crate for this check any more
//! (`rust-architecture-conformance-3`,
//! `governance/plans/meridian-rust-migration-program-plan.md` §5.17).
//!
//! `git` is supplied by the caller rather than constructed here: `validate`
//! resolves the ONE real Git tracked-file snapshot for the whole invocation
//! in `commands::validate::collect` and passes it down as a shared
//! [`GitInspector`] (`crate::adapters::git_inspector::CachedGitInspector` in
//! production), so this family never spawns its own second `git ls-files`
//! (`rust-architecture-conformance-3` corrective round, "one Git snapshot
//! for the whole `validate`").
//!
//! Every message [`app::evaluate`] produces is prefix-free; this is the ONE
//! place the `task-pattern-registry: ` presentation prefix is applied
//! (`rust-architecture-conformance-3` corrective round, "presentation
//! ownership" — `meridian_app::operating_model::task_pattern_registry`'s own
//! two Git-availability diagnostics used to bake this same prefix into
//! their own message text, which would have doubled it once this module
//! also started prefixing; they no longer do).

use std::path::Path;

use meridian_app::operating_model::task_pattern_registry::{self as app, Ports};
use meridian_app::validation::mechanical_integrity::OperationError;
use meridian_app::workspace::GitInspector;
use meridian_core::task_contracts::TaskPatternCatalog;

use crate::adapters::link_target::FsLinkTarget;
use crate::adapters::workspace_reader::FsWorkspaceReader;

use super::{split_diagnostics, with_family_prefix};

const FAMILY: &str = "task-pattern-registry";

pub struct Outcome {
    pub failures: Vec<String>,
    pub warnings: Vec<String>,
    pub catalog: Option<TaskPatternCatalog>,
}

pub fn run(kernel_root: &Path, git: &dyn GitInspector) -> Result<Outcome, OperationError> {
    let reader = FsWorkspaceReader::new(kernel_root);
    let link_target = FsLinkTarget::new(kernel_root);
    let ports = Ports {
        reader: &reader,
        git,
        link_target: &link_target,
    };
    let outcome = app::evaluate(&ports)?;
    let (failures, warnings) = split_diagnostics(outcome.diagnostics);
    Ok(Outcome {
        failures: with_family_prefix(FAMILY, failures),
        warnings: with_family_prefix(FAMILY, warnings),
        catalog: outcome.catalog,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Corrective round: a representative APP-generated failure (the
    /// registry route's own "the built-in task-type catalog is a mandatory
    /// part of this Kernel" gate, triggered by an empty workspace with no
    /// `task-pattern-registry.yaml` at all) exits this CLI wrapper with
    /// EXACTLY ONE `task-pattern-registry: ` prefix — an exact string match,
    /// not a substring/`contains` check, so a doubled or missing prefix
    /// would fail this test either way.
    #[test]
    fn a_representative_app_generated_failure_carries_exactly_one_family_prefix() {
        let empty_root = std::env::temp_dir().join(format!(
            "task-pattern-registry-cli-prefix-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&empty_root).unwrap();
        struct Guard(std::path::PathBuf);
        impl Drop for Guard {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _guard = Guard(empty_root.clone());

        // The missing-YAML gate returns before `evaluate` ever consults
        // Git, so any `GitInspector` — including one that never spawns a
        // process — proves this.
        let git = crate::adapters::git_inspector::CachedGitInspector::new(Ok(Vec::new()));
        let outcome = run(&empty_root, &git).unwrap();

        assert_eq!(
            outcome.failures,
            vec![
                "task-pattern-registry: standards/workspace/task-pattern-registry.yaml is missing; the built-in task-type catalog is a mandatory part of this Kernel, not an optional add-on"
                    .to_string()
            ]
        );
    }
}
