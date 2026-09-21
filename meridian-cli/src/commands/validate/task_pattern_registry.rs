//! Port of `scripts/kernel-validate.mjs`'s `task-pattern-registry` check
//! (subpackage 7b, `governance/plans/meridian-rust-migration-program-plan.md`
//! §5.5c). `standards/workspace/task-pattern-registry.yaml` is a mandatory
//! part of every Kernel; its schema, envelope and fixtures are read here,
//! and the composite algorithm itself lives in
//! [`meridian_app::operating_model::task_pattern_registry`] as a pure
//! function over already-parsed values.
//!
//! This module owns exactly the filesystem-facing part the composite
//! algorithm cannot: reading the YAML/schema/fixtures files, building the
//! tracked-file set, and resolving a canonical link's target through
//! `std::fs::canonicalize`/`std::fs::metadata` against it. That last piece
//! is expressed as the `check_kernel_link` callback
//! [`meridian_app::operating_model::task_pattern_registry::EvalContext`]
//! declares — this module supplies the real implementation and passes it
//! down, rather than the composite algorithm reaching for `std::fs` itself.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use meridian_app::operating_model::task_pattern_registry::{
    evaluate_task_pattern_registry, EvalContext, CHANGE_CLASSES, WORK_KINDS,
};
use meridian_app::source_format::{json_schema, parse_yaml};
use serde_json::Value;

/// The filesystem half of `checkKernelLinkTarget`: a `present` canonical
/// link's `path` must point at a regular, tracked file that is genuinely
/// inside the Kernel — proven by construction, not by "the external file
/// happens to be absent". This is the `check_kernel_link` callback
/// `meridian_app::operating_model::task_pattern_registry::EvalContext`
/// declares; the non-filesystem half (empty/backslash/absolute/`..` path
/// shape) is pure and lives there instead, run before this callback is
/// ever invoked — `run` below never calls this function with a path shape
/// that half would already reject.
fn check_kernel_link_target(
    rel_path: &str,
    kernel_root: &Path,
    is_tracked: &dyn Fn(&str) -> bool,
) -> Result<(), String> {
    let root = kernel_root
        .canonicalize()
        .map_err(|_| "the Kernel root itself cannot be resolved".to_string())?;
    let lexical = root.join(rel_path);

    let real_target =
        fs::canonicalize(&lexical).map_err(|_| "the target does not exist".to_string())?;
    if !real_target.starts_with(&root) {
        return Err(
            "the target resolves outside the Kernel once symbolic links are followed".to_string(),
        );
    }

    let metadata =
        fs::metadata(&real_target).map_err(|_| "the target cannot be inspected".to_string())?;
    if !metadata.is_file() {
        return Err("the target is not a regular file".to_string());
    }

    if !is_tracked(rel_path) {
        return Err("the target is not in the Kernel's tracked file set".to_string());
    }
    Ok(())
}

pub struct Outcome {
    pub failures: Vec<String>,
}

fn relative_slash(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

pub fn run(kernel_root: &Path, tracked_files: &[PathBuf]) -> Outcome {
    const TPR_YAML_REL: &str = "standards/workspace/task-pattern-registry.yaml";
    const TPR_REG_SCHEMA_REL: &str = "registries/operating-model/task-pattern-registry.schema.json";
    const TPR_ENV_SCHEMA_REL: &str = "registries/operating-model/scoped-record.schema.json";
    const TPR_FX_REL: &str =
        "registries/operating-model/fixtures/task-pattern-registry.fixtures.json";

    let mut failures = Vec::new();

    let tracked: HashSet<String> = tracked_files
        .iter()
        .map(|p| relative_slash(kernel_root, p))
        .collect();
    let is_tracked = |rel: &str| tracked.contains(rel);
    let check_kernel_link =
        |rel_path: &str| check_kernel_link_target(rel_path, kernel_root, &is_tracked);

    let Some(yaml_raw) = fs::read_to_string(kernel_root.join(TPR_YAML_REL)).ok() else {
        failures.push(format!(
            "task-pattern-registry: {TPR_YAML_REL} is missing; the built-in task-type catalog is a mandatory part of this Kernel, not an optional add-on"
        ));
        return Outcome { failures };
    };

    let mut tpr_ok = true;
    let registry_schema = match load_schema(kernel_root, TPR_REG_SCHEMA_REL, "registry") {
        Ok(schema) => Some(schema),
        Err(error) => {
            failures.push(format!("task-pattern-registry: {error}"));
            tpr_ok = false;
            None
        }
    };
    let envelope_schema = match load_schema(kernel_root, TPR_ENV_SCHEMA_REL, "envelope") {
        Ok(schema) => Some(schema),
        Err(error) => {
            failures.push(format!("task-pattern-registry: {error}"));
            tpr_ok = false;
            None
        }
    };

    if let (true, Some(registry_schema)) = (tpr_ok, &registry_schema) {
        let wk_enum = registry_schema
            .pointer("/definitions/payload/properties/work_kind/enum")
            .cloned();
        let cc_enum = registry_schema
            .pointer("/definitions/payload/properties/change_class/enum")
            .cloned();
        let expected_wk: Value = serde_json::json!(WORK_KINDS);
        let expected_cc: Value = serde_json::json!(CHANGE_CLASSES);
        if wk_enum.as_ref() != Some(&expected_wk) {
            failures.push(format!(
                "task-pattern-registry: the schema's work_kind pool {} diverges from the canonical four (rule-resolution.md §2)",
                serde_json::to_string(&wk_enum).unwrap_or_default()
            ));
            tpr_ok = false;
        }
        if cc_enum.as_ref() != Some(&expected_cc) {
            failures.push(format!(
                "task-pattern-registry: the schema's change_class pool {} diverges from the canonical four (rule-resolution.md §3)",
                serde_json::to_string(&cc_enum).unwrap_or_default()
            ));
            tpr_ok = false;
        }
    }

    let mut tpr_doc: Option<Value> = None;
    if tpr_ok {
        match parse_yaml(&yaml_raw) {
            Ok(doc) => tpr_doc = Some(doc),
            Err(error) => {
                failures.push(format!(
                    "task-pattern-registry: cannot parse {TPR_YAML_REL}: {error}"
                ));
                tpr_ok = false;
            }
        }
    }

    if let (true, Some(doc), Some(registry_schema), Some(envelope_schema)) =
        (tpr_ok, &tpr_doc, &registry_schema, &envelope_schema)
    {
        let ctx = EvalContext {
            registry_schema,
            envelope_schema,
            check_kernel_link: &check_kernel_link,
        };
        let problems = evaluate_task_pattern_registry(doc, &ctx);
        if !problems.is_empty() {
            for p in problems.iter().take(10) {
                failures.push(format!("task-pattern-registry: {p}"));
            }
            tpr_ok = false;
        }
    }

    if let Ok(rr_raw) =
        fs::read_to_string(kernel_root.join("standards/workspace/rule-resolution.md"))
    {
        for p in
            meridian_app::operating_model::task_pattern_registry::check_rule_resolution_bugfix_consistency(
                &rr_raw,
            )
        {
            failures.push(format!("task-pattern-registry: {p}"));
            tpr_ok = false;
        }
    }

    let Some(fx_raw) = fs::read_to_string(kernel_root.join(TPR_FX_REL)).ok() else {
        failures.push(format!(
            "task-pattern-registry: the mandatory catalog carries no fixtures ({TPR_FX_REL}); a schema no run exercises is not one this gate has reached"
        ));
        return Outcome { failures };
    };

    if !tpr_ok {
        return Outcome { failures };
    }

    let bundle: Value = match serde_json::from_str(&fx_raw) {
        Ok(value) => value,
        Err(error) => {
            failures.push(format!(
                "task-pattern-registry: the fixtures file is not valid JSON: {error}"
            ));
            return Outcome { failures };
        }
    };

    let mut bundle_ok = true;
    for key in ["valid", "invalid"] {
        let is_nonempty = bundle
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty());
        if !is_nonempty {
            bundle_ok = false;
            failures.push(format!(
                "task-pattern-registry: the fixtures file has no non-empty \"{key}\" array"
            ));
        }
    }
    if !bundle_ok {
        return Outcome { failures };
    }

    let registry_schema = registry_schema.as_ref().unwrap();
    let envelope_schema = envelope_schema.as_ref().unwrap();
    let ctx = EvalContext {
        registry_schema,
        envelope_schema,
        check_kernel_link: &check_kernel_link,
    };

    for case in bundle["valid"].as_array().unwrap() {
        let registry = case.get("registry").cloned().unwrap_or(Value::Null);
        let problems = evaluate_task_pattern_registry(&registry, &ctx);
        if !problems.is_empty() {
            failures.push(format!(
                "task-pattern-registry: a fixture that must be a valid catalog was rejected ({}): {}",
                case.get("note").and_then(Value::as_str).unwrap_or(""),
                problems[0]
            ));
        }
    }
    for case in bundle["invalid"].as_array().unwrap() {
        let registry = case.get("registry").cloned().unwrap_or(Value::Null);
        let problems = evaluate_task_pattern_registry(&registry, &ctx);
        if problems.is_empty() {
            failures.push(format!(
                "task-pattern-registry: a fixture that must be rejected validated clean ({})",
                case.get("note").and_then(Value::as_str).unwrap_or("")
            ));
        }
    }

    Outcome { failures }
}

fn load_schema(kernel_root: &Path, rel: &str, id: &str) -> Result<Value, String> {
    let raw = fs::read_to_string(kernel_root.join(rel)).map_err(|_| {
        format!("{rel} is missing; the mandatory catalog cannot be checked without its {id} schema")
    })?;
    let parsed: Value =
        serde_json::from_str(&raw).map_err(|error| format!("{rel} is not valid JSON: {error}"))?;
    json_schema::assert_supported_deep(&parsed, id).map_err(|error| {
        format!("the {id} schema uses a construct this validator cannot check: {error}")
    })?;
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "task-pattern-registry-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&dir).unwrap();
            TestDir(dir)
        }
    }

    impl std::ops::Deref for TestDir {
        type Target = Path;
        fn deref(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn temp(label: &str) -> TestDir {
        TestDir::new(label)
    }

    #[test]
    fn check_kernel_link_target_rejects_a_path_outside_the_kernel_via_symlink() {
        let dir = temp("symlink-escape");
        let outside = temp("symlink-escape-outside");
        fs::write(outside.join("secret.md"), "x").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.join("secret.md"), dir.join("link.md")).unwrap();
        #[cfg(unix)]
        {
            let err = check_kernel_link_target("link.md", &dir, &|_| true).unwrap_err();
            assert!(err.contains("outside the Kernel"));
        }
    }

    #[test]
    fn check_kernel_link_target_rejects_an_untracked_file() {
        let dir = temp("untracked");
        fs::write(dir.join("a.md"), "x").unwrap();
        let err = check_kernel_link_target("a.md", &dir, &|_| false).unwrap_err();
        assert!(err.contains("tracked file set"));
    }

    #[test]
    fn check_kernel_link_target_accepts_a_tracked_regular_file() {
        let dir = temp("ok");
        fs::write(dir.join("a.md"), "x").unwrap();
        assert!(check_kernel_link_target("a.md", &dir, &|_| true).is_ok());
    }

    #[test]
    fn the_real_kernel_catalog_agrees_with_its_own_schema_and_fixtures() {
        let kernel_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let tracked = crate::kernel::list_git_tracked_files(kernel_root)
            .unwrap_or_else(|| crate::kernel::walk_all_files(kernel_root).unwrap());
        let outcome = run(kernel_root, &tracked);
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    }

    #[test]
    fn a_missing_yaml_file_fails_closed() {
        let dir = temp("missing");
        let outcome = run(&dir, &[]);
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].contains("is missing"));
    }
}
