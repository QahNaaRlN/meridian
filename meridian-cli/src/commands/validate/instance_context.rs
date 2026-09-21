//! Port of `scripts/kernel-validate.mjs`'s `instance-context` check,
//! restricted to the "no Instance" branch — this package wires no
//! `--instance` path (`meridian-cli-rfc.md` defers `import`/Instance wiring
//! to package 8). Reads each vendored skill's own `PIN.yaml` (mechanical: no
//! new domain algorithm, just a YAML field read through the already-ported
//! strict adapter) and reports the same fixed `WARN` per skill that declares
//! `requires_instance_context`.

use std::fs;
use std::path::Path;

use meridian_app::source_format::parse_yaml;
use serde_json::Value;

use crate::kernel::WalkError;

pub fn run(kernel_root: &Path) -> Result<Vec<String>, WalkError> {
    let skills_dir = kernel_root.join("skills");
    let mut warnings = Vec::new();
    let Ok(entries) = fs::read_dir(&skills_dir) else {
        return Ok(warnings);
    };
    let mut names: Vec<(String, std::path::PathBuf)> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .map(|entry| {
            (
                entry.file_name().to_string_lossy().into_owned(),
                entry.path().join("PIN.yaml"),
            )
        })
        .collect();
    names.sort();
    for (name, pin_path) in names {
        let Ok(raw) = fs::read_to_string(&pin_path) else {
            continue;
        };
        let Ok(doc) = parse_yaml(&raw) else {
            continue;
        };
        if let Some(ctx_rel) = doc.get("requires_instance_context").and_then(Value::as_str) {
            warnings.push(format!(
                "instance-context: {name} requires \"{ctx_rel}\" from the Instance; no Instance root set, presence UNVERIFIED"
            ));
        }
    }
    Ok(warnings)
}
