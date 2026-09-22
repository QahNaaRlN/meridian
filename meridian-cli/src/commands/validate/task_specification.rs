//! Presentation/composition shim over the real port-based implementation:
//! `meridian_app::operating_model::task_specification`, reading through
//! `crate::adapters::workspace_reader::FsWorkspaceReader`. No predicate
//! logic, `serde_json::Value`, schema navigation or fixture loop lives in
//! this crate for this check any more (`rust-architecture-conformance-3`).
//!
//! `catalog` is the [`TaskPatternCatalog`] `super::task_pattern_registry::run`
//! already built — this module never re-reads or re-parses
//! `task-pattern-registry.yaml`.
//!
//! Every message [`app::evaluate`] produces, AND this module's own "no
//! catalog available" short-circuit message, are prefix-free; this is the
//! ONE place the `task-specification-contract: ` presentation prefix is
//! applied — both sources go through the same [`with_family_prefix`] call,
//! so there is exactly one code path that can ever add it
//! (`rust-architecture-conformance-3` corrective round, "presentation
//! ownership").

use std::path::Path;

use meridian_app::operating_model::task_specification as app;
use meridian_app::validation::mechanical_integrity::OperationError;
use meridian_core::task_contracts::TaskPatternCatalog;

use super::{split_diagnostics, with_family_prefix};
use crate::adapters::workspace_reader::FsWorkspaceReader;

const FAMILY: &str = "task-specification-contract";

pub struct Outcome {
    pub failures: Vec<String>,
}

pub fn run(
    kernel_root: &Path,
    catalog: Option<&TaskPatternCatalog>,
) -> Result<Outcome, OperationError> {
    let Some(catalog) = catalog else {
        return Ok(Outcome {
            failures: with_family_prefix(
                FAMILY,
                vec![
                    "no task-pattern catalog is available; task-pattern-registry must be checked first".to_string(),
                ],
            ),
        });
    };
    let reader = FsWorkspaceReader::new(kernel_root);
    let outcome = app::evaluate(&reader, catalog)?;
    let (failures, _warnings) = split_diagnostics(outcome.diagnostics);
    Ok(Outcome {
        failures: with_family_prefix(FAMILY, failures),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `rust-architecture-conformance-3` corrective round item 1's last
    /// bullet: task specification must never run against a partial/absent
    /// catalog. `catalog: None` (what `task_pattern_registry::run` now
    /// yields for ANY registry failure class, per the corrective round)
    /// must fail closed before this module ever touches the workspace — a
    /// bogus, nonexistent `kernel_root` proves no `FsWorkspaceReader` I/O
    /// was attempted, since that would surface as a different diagnostic or
    /// an `Err`, not this exact "no catalog" failure.
    #[test]
    fn no_catalog_fails_closed_without_touching_the_workspace() {
        let bogus_root = Path::new("/nonexistent-kernel-root-used-only-to-prove-no-io-happens");
        let outcome = run(bogus_root, None).unwrap();
        assert_eq!(
            outcome.failures,
            vec![
                "task-specification-contract: no task-pattern catalog is available; task-pattern-registry must be checked first"
                    .to_string()
            ]
        );
    }

    /// Corrective round: a representative APP-generated failure (the
    /// specification route's own "the task specification contract is a
    /// mandatory part of this Kernel" gate, triggered by an empty workspace
    /// with no schema at all) exits this CLI wrapper with EXACTLY ONE
    /// `task-specification-contract: ` prefix — an exact string match, so a
    /// doubled or missing prefix would fail this test either way. The
    /// catalog is real (built from the real Kernel), so the ONLY reason
    /// this run fails is the missing schema in the empty root, not a
    /// short-circuited "no catalog" path.
    #[test]
    fn a_representative_app_generated_failure_carries_exactly_one_family_prefix() {
        let real_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let git = crate::adapters::git_inspector::RealGitInspector::new(&real_root);
        let catalog = super::super::task_pattern_registry::run(&real_root, &git)
            .unwrap()
            .catalog
            .expect("real catalog builds");

        let empty_root = std::env::temp_dir().join(format!(
            "task-specification-cli-prefix-test-{}-{}",
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

        let outcome = run(&empty_root, Some(&catalog)).unwrap();
        assert_eq!(
            outcome.failures,
            vec![
                "task-specification-contract: registries/operating-model/task-specification.schema.json is missing; the task specification contract is a mandatory part of this Kernel, not an optional add-on"
                    .to_string()
            ]
        );
    }
}
