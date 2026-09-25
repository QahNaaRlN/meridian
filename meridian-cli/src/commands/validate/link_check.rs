//! Port of `scripts/kernel-validate.mjs`'s link check: every relative
//! Markdown link resolves to a real file, and none escapes the named Kernel
//! root (a link that only resolves "here" by accident of what happens to sit
//! next to this checkout breaks on a clean clone elsewhere).

use std::fs;
use std::path::{Path, PathBuf};

use fancy_regex::Regex;

pub struct Outcome {
    pub failures: Vec<String>,
    pub checked: usize,
}

pub fn run(kernel_root: &Path, markdown_files: &[PathBuf]) -> Outcome {
    let link_re = Regex::new(r"\]\((\.\.?/[^)#\s]+|[^):#\s]+\.md)\)").unwrap();
    let real_kernel_root =
        fs::canonicalize(kernel_root).unwrap_or_else(|_| kernel_root.to_path_buf());

    let mut failures = Vec::new();
    for file in markdown_files {
        let Ok(text) = fs::read_to_string(file) else {
            continue;
        };
        let Some(dir) = file.parent() else {
            continue;
        };
        let mut start = 0usize;
        while let Ok(Some(found)) = link_re.find_from_pos(&text, start) {
            let target = found
                .as_str()
                .trim_start_matches("](")
                .trim_end_matches(')');
            start = found.end();
            if target.starts_with("http://") || target.starts_with("https://") {
                continue;
            }
            let resolved = normalize(&dir.join(target));
            if escapes_kernel(&real_kernel_root, &resolved) {
                failures.push(format!(
                    "link: {} -> \"{}\" points outside the Kernel; reference Instance material as a $MERIDIAN_INSTANCE path, do not link to it",
                    file.display(),
                    target
                ));
                continue;
            }
            if !resolved.exists() {
                failures.push(format!(
                    "link: {} -> \"{}\" does not resolve",
                    file.display(),
                    target
                ));
            }
        }
    }
    Outcome {
        failures,
        checked: markdown_files.len(),
    }
}

/// Resolves the deepest already-existing ancestor of `target` to its real
/// (symlink-free) path and re-appends the remaining, not-yet-existing
/// segments — the same discipline as the Node reference's `realResolve`, so
/// confinement is judged by where a link through a symlinked directory
/// actually lands, not by a textual prefix.
fn normalize(target: &Path) -> PathBuf {
    let mut base = target.to_path_buf();
    let mut rest = Vec::new();
    while !base.exists() {
        let Some(parent) = base.parent() else {
            break;
        };
        if let Some(name) = base.file_name() {
            rest.push(name.to_owned());
        }
        base = parent.to_path_buf();
    }
    let real = fs::canonicalize(&base).unwrap_or(base);
    rest.iter().rev().fold(real, |acc, seg| acc.join(seg))
}

fn escapes_kernel(real_kernel_root: &Path, resolved: &Path) -> bool {
    resolved.strip_prefix(real_kernel_root).is_err()
}
