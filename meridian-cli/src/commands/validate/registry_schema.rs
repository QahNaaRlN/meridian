//! Port of `scripts/kernel-validate.mjs`'s generic, in-gate registry schema
//! validation: every YAML document under the Kernel that declares a local
//! `$schema` is validated against it, using exactly the ported strict Draft
//! 7 adapter (`meridian_app::source_format::json_schema`) — the same
//! allowlist and diagnostics the Node reference's `scripts/lib/json-schema.mjs`
//! already carried before the port (`rust-source-format-adapters`, accepted).

use std::fs;
use std::path::Path;

use meridian_app::source_format::{json_schema, parse_yaml};
use serde_json::Value;

use crate::kernel::{walk_files_with_extensions, WalkError};

pub struct Outcome {
    pub failures: Vec<String>,
    pub warnings: Vec<String>,
    pub validated: usize,
    pub attempted: usize,
}

pub fn run(kernel_root: &Path) -> Result<Outcome, WalkError> {
    let files = walk_files_with_extensions(kernel_root, &["yaml", "yml"])?;
    let mut failures = Vec::new();
    let mut warnings = Vec::new();
    let mut validated = 0usize;
    let mut attempted = 0usize;

    for file in &files {
        let Ok(raw) = fs::read_to_string(file) else {
            continue;
        };
        let doc = match parse_yaml(&raw) {
            Ok(doc) => doc,
            Err(error) => {
                failures.push(format!("schema: cannot parse {}: {error}", file.display()));
                continue;
            }
        };
        let schema_ref = doc
            .get("$schema")
            .and_then(Value::as_str)
            .map(str::to_owned);
        let Some(schema_ref) = schema_ref else {
            continue;
        };
        attempted += 1;

        let dir = file.parent().unwrap_or(kernel_root);
        let mut schema_path = dir.join(&schema_ref);
        let mut schema_raw = fs::read_to_string(&schema_path).ok();
        if schema_raw.is_none() && schema_ref.starts_with("./") {
            if let Some(registry) = dir.file_name().and_then(|n| n.to_str()) {
                let candidate = kernel_root
                    .join("registries")
                    .join(registry)
                    .join(&schema_ref[2..]);
                if let Ok(alt) = fs::read_to_string(&candidate) {
                    schema_path = candidate;
                    schema_raw = Some(alt);
                }
            }
        }
        let Some(schema_raw) = schema_raw else {
            failures.push(format!(
                "schema: {} references {schema_ref}, which resolves to no file in the Kernel",
                file.display()
            ));
            continue;
        };
        let schema: Value = match serde_json::from_str(&schema_raw) {
            Ok(value) => value,
            Err(error) => {
                failures.push(format!(
                    "schema: {} is not valid JSON: {error}",
                    schema_path.display()
                ));
                continue;
            }
        };
        match json_schema::validate(&doc, &schema) {
            Ok(errors) => {
                if errors.is_empty() {
                    validated += 1;
                } else {
                    for error in errors.into_iter().take(10) {
                        failures.push(format!("schema: {} {error}", file.display()));
                    }
                }
            }
            Err(error) => {
                failures.push(format!(
                    "schema: {} could not be validated: {error}",
                    file.display()
                ));
            }
        }
    }

    if attempted == 0 {
        warnings.push("schema: no registry declared a $schema; nothing was validated".to_string());
    }

    Ok(Outcome {
        failures,
        warnings,
        validated,
        attempted,
    })
}
