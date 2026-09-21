//! Port of `scripts/kernel-validate.mjs`'s `task-specification-contract`
//! check (subpackage 7b): `registries/operating-model/task-specification.schema.json`
//! is the specialised payload schema for one task specification
//! (`record_type: task-specification`). Its envelope is validated against
//! the existing `scoped-record.schema.json` (composition, not a second
//! envelope). Specification DATA is Instance, like the intake register —
//! the Kernel ships the schema, the product-neutral fixtures and the
//! composite algorithm
//! ([`meridian_app::operating_model::task_specification`]), never a
//! canonical data file. The contract is a MANDATORY part of this Kernel: a
//! missing schema, a missing task-pattern catalogue (the reference target)
//! or missing fixtures is a FAIL, not an informational skip.
//!
//! File I/O — reading the schema/envelope/fixtures files and the built-in
//! `standards/workspace/task-pattern-registry.yaml` catalogue the
//! `task_pattern` reference resolves against — stays in this `meridian-cli`
//! module; the pure cross-cutting consistency rules live in `meridian-app`.

use std::fs;
use std::path::Path;

use meridian_app::operating_model::task_specification::{
    evaluate_task_specification, EvalSchemas, TaskPatternRef,
};
use meridian_app::source_format::{json_schema, parse_yaml};
use serde_json::Value;

const SCHEMA_NAME: &str = "task-specification.schema.json";

pub struct Outcome {
    pub failures: Vec<String>,
}

pub fn run(kernel_root: &Path) -> Outcome {
    let dir = kernel_root.join("registries").join("operating-model");
    let mut failures = Vec::new();

    let Some(schema_raw) = fs::read_to_string(dir.join(SCHEMA_NAME)).ok() else {
        failures.push(format!(
            "task-specification-contract: registries/operating-model/{SCHEMA_NAME} is missing; the task specification contract is a mandatory part of this Kernel, not an optional add-on"
        ));
        return Outcome { failures };
    };

    let mut ok = true;
    let mut schema: Option<Value> = None;
    match serde_json::from_str::<Value>(&schema_raw) {
        Ok(parsed) => schema = Some(parsed),
        Err(error) => {
            failures.push(format!(
                "task-specification-contract: {SCHEMA_NAME} is not valid JSON: {error}"
            ));
            ok = false;
        }
    }

    let mut envelope: Option<Value> = None;
    match fs::read_to_string(dir.join("scoped-record.schema.json")) {
        Ok(raw) => match serde_json::from_str::<Value>(&raw) {
            Ok(parsed) => envelope = Some(parsed),
            Err(error) => {
                failures.push(format!(
                    "task-specification-contract: scoped-record.schema.json is not valid JSON: {error}"
                ));
                ok = false;
            }
        },
        Err(_) => {
            failures.push(
                "task-specification-contract: registries/operating-model/scoped-record.schema.json is missing; the specification composes with the record envelope and cannot be checked without it"
                    .to_string(),
            );
            ok = false;
        }
    }

    if let Some(schema) = &schema {
        if let Err(error) = json_schema::assert_supported_deep(schema, SCHEMA_NAME) {
            failures.push(format!(
                "task-specification-contract: the schema uses a construct this validator cannot check: {error}"
            ));
            ok = false;
        }
    }

    // The task_pattern reference is resolved against the built-in
    // catalogue, so the catalogue must be readable here too.
    let mut task_patterns: Vec<(String, Option<String>, Option<String>)> = Vec::new();
    match fs::read_to_string(kernel_root.join("standards/workspace/task-pattern-registry.yaml")) {
        Ok(tpr_raw) => match parse_yaml(&tpr_raw) {
            Ok(tpr_doc) => {
                let empty = Vec::new();
                let entries = tpr_doc
                    .get("task_patterns")
                    .and_then(Value::as_array)
                    .unwrap_or(&empty);
                for p in entries {
                    let id = p
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let work_kind = p
                        .get("payload")
                        .and_then(|v| v.get("work_kind"))
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    let change_class = p
                        .get("payload")
                        .and_then(|v| v.get("change_class"))
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    task_patterns.push((id, work_kind, change_class));
                }
            }
            Err(error) => {
                failures.push(format!(
                    "task-specification-contract: cannot parse task-pattern-registry.yaml: {error}"
                ));
                ok = false;
            }
        },
        Err(_) => {
            failures.push(
                "task-specification-contract: standards/workspace/task-pattern-registry.yaml is missing; the specification's task-pattern reference cannot be resolved without the catalogue"
                    .to_string(),
            );
            ok = false;
        }
    }

    let fixtures_path = dir
        .join("fixtures")
        .join("task-specification.fixtures.json");
    let Ok(fx_raw) = fs::read_to_string(&fixtures_path) else {
        failures.push(
            "task-specification-contract: the schema carries no fixtures (registries/operating-model/fixtures/task-specification.fixtures.json); a schema no run exercises is not one this gate has reached"
                .to_string(),
        );
        return Outcome { failures };
    };
    if !ok {
        return Outcome { failures };
    }

    let bundle: Value = match serde_json::from_str(&fx_raw) {
        Ok(value) => value,
        Err(error) => {
            failures.push(format!(
                "task-specification-contract: the fixtures file is not valid JSON: {error}"
            ));
            return Outcome { failures };
        }
    };
    if !bundle.is_object() {
        failures.push(
            "task-specification-contract: the fixtures file must be an object with non-empty \"valid\" and \"invalid\" arrays"
                .to_string(),
        );
        return Outcome { failures };
    }
    let mut bundle_ok = true;
    for key in ["valid", "invalid"] {
        let is_nonempty = bundle
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty());
        if !is_nonempty {
            bundle_ok = false;
            failures.push(format!(
                "task-specification-contract: the fixtures file has no non-empty \"{key}\" array"
            ));
        }
    }
    if !bundle_ok {
        return Outcome { failures };
    }

    let schemas = EvalSchemas {
        record_schema: schema.as_ref().unwrap(),
        envelope_schema: envelope.as_ref().unwrap(),
    };
    let patterns: Vec<TaskPatternRef> = task_patterns
        .iter()
        .map(|(id, wk, cc)| TaskPatternRef {
            id,
            work_kind: wk.as_deref(),
            change_class: cc.as_deref(),
        })
        .collect();

    for case in bundle["valid"].as_array().unwrap() {
        let spec = case.get("spec").cloned().unwrap_or(Value::Null);
        let problems = evaluate_task_specification(&spec, &schemas, &patterns);
        if !problems.is_empty() {
            failures.push(format!(
                "task-specification-contract: a fixture that must be a valid specification was rejected ({}): {}",
                case.get("note").and_then(Value::as_str).unwrap_or(""),
                problems[0]
            ));
        }
    }
    for case in bundle["invalid"].as_array().unwrap() {
        let spec = case.get("spec").cloned().unwrap_or(Value::Null);
        let problems = evaluate_task_specification(&spec, &schemas, &patterns);
        if problems.is_empty() {
            failures.push(format!(
                "task-specification-contract: a fixture that must be rejected validated clean ({})",
                case.get("note").and_then(Value::as_str).unwrap_or("")
            ));
        }
    }

    Outcome { failures }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_real_kernel_schema_and_fixtures_agree() {
        let kernel_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let outcome = run(kernel_root);
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    }
}
