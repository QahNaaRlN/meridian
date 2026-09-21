//! Port of `scripts/kernel-validate.mjs`'s `kernel-purity` check, restricted
//! to the personal-path-leak half that does not require an Instance
//! `product.yaml` (this package wires no `--instance` flag at all). The
//! product-literal half stays exactly as unverified here as it is in the
//! Node reference when `MERIDIAN_INSTANCE` is unset — reported as the same
//! fixed `WARN`, never silently upgraded to a clean result.

use std::fs;
use std::path::{Path, PathBuf};

use fancy_regex::Regex;

const BINARY_EXTS: &[&str] = &[".zip"];

/// Built from fragments, exactly like the Node reference, so this file does
/// not itself contain the literal sequence it searches for.
fn personal_path_pattern() -> Regex {
    let pattern = ["[A-Z]:", r"\\", "Users", r"\\", r"[^\\/\r\n]+"].join("");
    Regex::new(&format!("(?i){pattern}")).expect("personal-path pattern compiles")
}

pub struct Outcome {
    pub failures: Vec<String>,
    pub warnings: Vec<String>,
}

/// `files` is the Kernel's own file universe (tracked, or the fallback
/// walk), already resolved by `crate::commands::validate`. `used_git`
/// controls whether the untracked-files warning (only meaningful when Git
/// enumeration succeeded) is attempted at all.
pub fn run(kernel_root: &Path, files: &[PathBuf], used_git: bool) -> Outcome {
    let mut failures = Vec::new();
    let mut warnings = Vec::new();

    if used_git {
        if let Some(untracked) = crate::kernel::list_git_untracked_files(kernel_root) {
            if !untracked.is_empty() {
                let mut rels: Vec<String> = untracked
                    .iter()
                    .map(|f| relative_slash(kernel_root, f))
                    .collect();
                rels.sort();
                let shown = rels.iter().take(5).cloned().collect::<Vec<_>>().join(", ");
                let suffix = if rels.len() > 5 { ", …" } else { "" };
                warnings.push(format!(
                    "kernel-purity: {} file(s) in the Kernel tree are untracked and were therefore NOT scanned by any check in this run ({shown}{suffix}); add them to the index or to .gitignore — a file the gate cannot see is not a file the gate approved",
                    rels.len()
                ));
            }
        }
    }

    // The product-literal half genuinely cannot run without an Instance
    // product record; this package wires no Instance at all
    // (`meridian-cli-rfc.md` §"Состав CLI" defers `import`/Instance wiring
    // to package 8). Reported UNVERIFIED, never silently upgraded to a
    // clean `kernel-purity` result — matches the Node reference's own
    // `MERIDIAN_INSTANCE` unset branch verbatim.
    warnings.push(
        "kernel-purity: MERIDIAN_INSTANCE is not set; product literals were NOT checked (UNVERIFIED). \
This run verifies personal-path leaks only and must not be reported as a clean kernel-purity result."
            .to_string(),
    );

    let personal_path = personal_path_pattern();
    for file in files {
        if BINARY_EXTS
            .iter()
            .any(|ext| file.to_string_lossy().ends_with(ext))
        {
            continue;
        }
        let Ok(text) = fs::read_to_string(file) else {
            continue;
        };
        if let Ok(Some(found)) = personal_path.find(&text) {
            failures.push(format!(
                "kernel-purity: personal home path \"{}\" found in {}",
                found.as_str(),
                file.display()
            ));
        }
    }

    Outcome { failures, warnings }
}

fn relative_slash(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}
