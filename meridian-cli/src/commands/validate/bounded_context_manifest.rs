//! Presentation/composition shim over the real port-based implementation:
//! `meridian_app::operating_model::bounded_context_manifest`, reading through
//! `crate::adapters::workspace_reader::FsWorkspaceReader`. No filesystem
//! I/O, `serde_json::Value`, schema/fixture parsing or domain rules of this
//! family live in this crate (`rust-architecture-conformance-5`).
//!
//! Every message [`app::evaluate`] produces is prefix-free; this is the ONE
//! place the `bounded-context-manifest: ` presentation prefix is applied.

use std::path::Path;

use meridian_app::operating_model::bounded_context_manifest as app;

use super::{split_diagnostics, with_family_prefix};
use crate::adapters::workspace_reader::FsWorkspaceReader;

const FAMILY: &str = "bounded-context-manifest";

pub struct Outcome {
    pub failures: Vec<String>,
}

pub fn run(kernel_root: &Path) -> Outcome {
    let reader = FsWorkspaceReader::new(kernel_root);
    let outcome = app::evaluate(&reader);
    let (failures, _warnings) = split_diagnostics(outcome.diagnostics);
    Outcome {
        failures: with_family_prefix(FAMILY, failures),
    }
}
