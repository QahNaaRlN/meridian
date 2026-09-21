//! Port of `scripts/kernel-validate.mjs`'s `duplicate-fm` check: no scanned
//! Markdown file ends with a second, orphaned trailing Front Matter block.

use std::fs;
use std::path::{Path, PathBuf};

use fancy_regex::Regex;

pub struct Outcome {
    pub failures: Vec<String>,
    pub checked: usize,
}

pub fn run(markdown_files: &[PathBuf]) -> Outcome {
    let trailing_fm =
        Regex::new(r"(?is)\n---[ \t]*\r?\n(?:[a-z_]+:[^\n]*\r?\n)+---[ \t]*\r?\n?\s*$").unwrap();

    let mut failures = Vec::new();
    for file in markdown_files {
        let Ok(text) = fs::read_to_string(file) else {
            continue;
        };
        if !text.starts_with("---") {
            continue;
        }
        let close_idx = match text[3..].find("\n---") {
            Some(idx) => idx + 3,
            None => continue,
        };
        let tail = format!("\n{}", &text[close_idx + 4..]);
        if trailing_fm.is_match(&tail).unwrap_or(false) {
            failures.push(format!(
                "duplicate-fm: orphaned trailing Front Matter block at end of {}",
                display_path(file)
            ));
        }
    }
    Outcome {
        failures,
        checked: markdown_files.len(),
    }
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}
