//! Presentation/composition shim over the real port-based implementation:
//! `meridian_app::operating_model::upgrade_integration_qualification`,
//! reading through `crate::adapters::workspace_reader::FsWorkspaceReader`
//! and the task-pattern catalogue `task_pattern_registry` already
//! published. No filesystem I/O, `serde_json::Value`, schema/fixture
//! parsing, resolver construction or domain rules of this family live in
//! this crate (`rust-architecture-conformance-7`).
//!
//! Every message [`app::evaluate`] produces is prefix-free; this is the ONE
//! place the `upgrade-integration-qualification: ` presentation prefix is
//! applied. Like `task-specification-contract`, the family never runs
//! against a partial or absent catalogue (`COMPATIBILITY.md`).

use std::path::Path;

use meridian_app::operating_model::upgrade_integration_qualification as app;
use meridian_core::task_contracts::TaskPatternCatalog;

use super::{split_diagnostics, with_family_prefix};
use crate::adapters::workspace_reader::FsWorkspaceReader;

const FAMILY: &str = "upgrade-integration-qualification";

pub struct Outcome {
    pub failures: Vec<String>,
}

pub fn run(kernel_root: &Path, catalog: Option<&TaskPatternCatalog>) -> Outcome {
    let Some(catalog) = catalog else {
        return Outcome {
            failures: with_family_prefix(
                FAMILY,
                vec![
                    "no task-pattern catalog is available; task-pattern-registry must be checked first"
                        .to_string(),
                ],
            ),
        };
    };
    let reader = FsWorkspaceReader::new(kernel_root);
    let outcome = app::evaluate(&reader, catalog);
    let (failures, _warnings) = split_diagnostics(outcome.diagnostics);
    Outcome {
        failures: with_family_prefix(FAMILY, failures),
    }
}
