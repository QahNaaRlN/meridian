//! Port of `scripts/kernel-validate.mjs`'s `git-provenance` check, restricted
//! to the Kernel half — this package wires no `--instance` path, so the
//! Instance half stays the same fixed `WARN` the Node reference already
//! prints when `MERIDIAN_INSTANCE` is unset.

use std::path::Path;

pub struct Outcome {
    pub failures: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn run(kernel_root: &Path) -> Outcome {
    let mut failures = Vec::new();
    let mut warnings = Vec::new();
    if kernel_root.join(".git").exists() {
        // No OK line is emitted here: this module reports only FAIL/WARN,
        // matching the diagnostic-only convention every other check in this
        // package follows (`crate::commands::validate`).
    } else {
        failures.push(
            "git-provenance: Kernel is not under Git; no revision can be cited for it".to_string(),
        );
    }
    warnings
        .push("git-provenance: Instance root not supplied, its VCS state is unknown".to_string());
    Outcome { failures, warnings }
}
