//! Port of `scripts/kernel-validate.mjs`'s generic, in-gate registry schema
//! validation: every YAML document under the Kernel that declares a local
//! `$schema` is validated against it, using exactly the ported strict Draft
//! 7 adapter (`meridian_app::source_format::json_schema`) — the same
//! allowlist and diagnostics the Node reference's `scripts/lib/json-schema.mjs`
//! already carried before the port (`rust-source-format-adapters`, accepted).

use std::fs;
use std::path::{Component, Path, PathBuf};

use meridian_app::source_format::{json_schema, parse_yaml};
use serde_json::Value;

use super::CollectError;
use crate::kernel::walk_files_with_extensions;

pub struct Outcome {
    pub failures: Vec<String>,
    pub warnings: Vec<String>,
    pub validated: usize,
    pub attempted: usize,
}

pub fn run(kernel_root: &Path) -> Result<Outcome, CollectError> {
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
        let mut schema_path = resolve_like_the_reference(&dir.join(&schema_ref))?;
        let mut schema_raw = fs::read_to_string(&schema_path).ok();
        if schema_raw.is_none() && schema_ref.starts_with("./") {
            if let Some(registry) = dir.file_name().and_then(|n| n.to_str()) {
                // `path.join(KERNEL_ROOT, 'registries', registry, …)`: normalized,
                // but NOT made absolute — the reference reports this fallback
                // under the Kernel path exactly as it was given.
                let candidate = normalize(
                    &kernel_root
                        .join("registries")
                        .join(registry)
                        .join(&schema_ref[2..]),
                );
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
        // The reference rejects an unsupported keyword or format anywhere in
        // the schema tree BEFORE validating, locating it under the schema's
        // own path (`assertSupportedDeep(schema, schemaPath)`); the walk
        // inside `json_schema::validate` alone would locate it under `/`.
        let checked = json_schema::assert_supported_deep(&schema, &schema_path.to_string_lossy())
            .and_then(|()| json_schema::validate(&doc, &schema));
        match checked {
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

/// The reference's `path.resolve(path.dirname(file), schemaRef)`: a
/// relative result is resolved against the current directory — a relative
/// `--kernel` (`../k`) therefore reports the schema under its absolute path,
/// as Node does — and then normalized by [`normalize`].
fn resolve_like_the_reference(path: &Path) -> Result<PathBuf, CollectError> {
    if path.is_absolute() {
        return Ok(normalize(path));
    }
    let cwd = std::env::current_dir()
        .map_err(|error| CollectError::CurrentDirectory(error.to_string()))?;
    Ok(normalize(&cwd.join(path)))
}

/// The reference's `path.normalize`: `.` segments dropped and each `..`
/// applied to the preceding named segment. A `..` with no named segment
/// before it is kept on a relative path and absorbed at the root of an
/// absolute one; an empty result is `.`.
fn normalize(path: &Path) -> PathBuf {
    let mut resolved = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match resolved.components().next_back() {
                Some(Component::Normal(_)) => {
                    resolved.pop();
                }
                Some(Component::RootDir | Component::Prefix(_)) => {}
                Some(Component::ParentDir | Component::CurDir) | None => resolved.push(".."),
            },
            other => resolved.push(other.as_os_str()),
        }
    }
    if resolved.as_os_str().is_empty() {
        resolved.push(".");
    }
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relative_schema_reference_is_resolved_like_the_reference() {
        assert_eq!(
            normalize(Path::new("/k/registries/x/./y.schema.json")),
            PathBuf::from("/k/registries/x/y.schema.json")
        );
        assert_eq!(
            normalize(Path::new("/k/registries/x/../y/z.schema.json")),
            PathBuf::from("/k/registries/y/z.schema.json")
        );
        assert_eq!(
            normalize(Path::new("/k/../../x.schema.json")),
            PathBuf::from("/x.schema.json")
        );
    }

    /// `path.normalize` keeps a leading `..` on a relative path (the
    /// `./`-fallback candidate under a relative `--kernel`); the primary
    /// schema path is made absolute first — see
    /// `binary_runs::validate_reports_a_relative_kernel_schema_under_its_absolute_path`.
    #[test]
    fn normalizing_a_relative_path_keeps_its_leading_parent_segments() {
        assert_eq!(
            normalize(Path::new(
                "../../../stack-profiles/./stack-profiles.schema.json"
            )),
            PathBuf::from("../../../stack-profiles/stack-profiles.schema.json")
        );
        assert_eq!(
            normalize(Path::new(
                "../../../standards/workspace/../../registries/operating-model/foundation.schema.json"
            )),
            PathBuf::from("../../../registries/operating-model/foundation.schema.json")
        );
        assert_eq!(
            normalize(Path::new("k/../../x.json")),
            PathBuf::from("../x.json")
        );
    }

    /// Regression (`rust-business-contract-qualification`): an unsupported
    /// keyword or format was located under `/` (`at //properties/m`)
    /// instead of under the declaring schema file, as the reference does.
    #[test]
    fn an_unsupported_schema_construct_is_located_under_the_schema_file() {
        let root = std::env::temp_dir().join(format!(
            "meridian-registry-schema-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("probe.schema.json"),
            r#"{"type":"object","properties":{"m":{"type":"string","format":"email"}}}"#,
        )
        .unwrap();
        fs::write(
            root.join("probe.yaml"),
            "$schema: ./probe.schema.json\nm: a\n",
        )
        .unwrap();

        let outcome = run(&root).unwrap();
        let _ = fs::remove_dir_all(&root);

        let schema_path = root.join("probe.schema.json");
        assert_eq!(
            outcome.failures,
            [format!(
                "schema: {} could not be validated: format \"email\" at {}/properties/m is not implemented by this validator",
                root.join("probe.yaml").display(),
                schema_path.display()
            )]
        );
        assert_eq!(outcome.validated, 0);
        assert_eq!(outcome.attempted, 1);
    }
}
