//! Port of `scripts/kernel-validate.mjs`'s `instruction-source-registry`
//! check (subpackage 7b): `registries/operating-model/instruction-source-registry.schema.json`
//! is a MANDATORY part of every Kernel — a missing schema or missing
//! fixtures is a FAIL, not an informational skip. Registry DATA is
//! Instance, like the intake register: the Kernel ships only the schema,
//! the product-neutral fixtures and the composite algorithm
//! ([`meridian_app::operating_model::instruction_source_registry`]), never
//! a canonical data file — there is no mandatory YAML to read here, unlike
//! `task-pattern-registry`.
//!
//! File I/O (reading the schema/envelope/fixtures files) stays in this
//! `meridian-cli` module; the pure cross-cutting consistency rules
//! (`evaluateInstructionSourceRegistry`'s port) live in `meridian-app`,
//! taking only already-parsed [`serde_json::Value`]s.

use std::fs;
use std::path::Path;

use meridian_app::operating_model::instruction_source_registry::{
    evaluate_instruction_source_registry, EvalSchemas,
};
use meridian_app::source_format::json_schema;
use serde_json::Value;

const SCHEMA_NAME: &str = "instruction-source-registry.schema.json";

pub struct Outcome {
    pub failures: Vec<String>,
}

pub fn run(kernel_root: &Path) -> Outcome {
    let dir = kernel_root.join("registries").join("operating-model");
    let mut failures = Vec::new();

    let Some(schema_raw) = fs::read_to_string(dir.join(SCHEMA_NAME)).ok() else {
        failures.push(format!(
            "instruction-source-registry: registries/operating-model/{SCHEMA_NAME} is missing; the instruction source registry contract is a mandatory part of this Kernel, not an optional add-on"
        ));
        return Outcome { failures };
    };

    let mut ok = true;
    let mut schema: Option<Value> = None;
    match serde_json::from_str::<Value>(&schema_raw) {
        Ok(parsed) => schema = Some(parsed),
        Err(error) => {
            failures.push(format!(
                "instruction-source-registry: {SCHEMA_NAME} is not valid JSON: {error}"
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
                    "instruction-source-registry: scoped-record.schema.json is not valid JSON: {error}"
                ));
                ok = false;
            }
        },
        Err(_) => {
            failures.push(
                "instruction-source-registry: registries/operating-model/scoped-record.schema.json is missing; the source snapshot composes with the record envelope and cannot be checked without it"
                    .to_string(),
            );
            ok = false;
        }
    }

    if let Some(schema) = &schema {
        if let Err(error) = json_schema::assert_supported_deep(schema, SCHEMA_NAME) {
            failures.push(format!(
                "instruction-source-registry: the schema uses a construct this validator cannot check: {error}"
            ));
            ok = false;
        }
    }

    let fixtures_path = dir
        .join("fixtures")
        .join("instruction-source-registry.fixtures.json");
    let Ok(fx_raw) = fs::read_to_string(&fixtures_path) else {
        failures.push(
            "instruction-source-registry: the schema carries no fixtures (registries/operating-model/fixtures/instruction-source-registry.fixtures.json); a schema no run exercises is not one this gate has reached"
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
                "instruction-source-registry: the fixtures file is not valid JSON: {error}"
            ));
            return Outcome { failures };
        }
    };
    if !bundle.is_object() {
        failures.push(
            "instruction-source-registry: the fixtures file must be an object with non-empty \"valid\" and \"invalid\" arrays"
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
                "instruction-source-registry: the fixtures file has no non-empty \"{key}\" array"
            ));
        }
    }
    if !bundle_ok {
        return Outcome { failures };
    }

    let schemas = EvalSchemas {
        registry_schema: schema.as_ref().unwrap(),
        envelope_schema: envelope.as_ref().unwrap(),
    };

    for case in bundle["valid"].as_array().unwrap() {
        let registry = case.get("registry").cloned().unwrap_or(Value::Null);
        let problems = evaluate_instruction_source_registry(&registry, &schemas);
        if !problems.is_empty() {
            failures.push(format!(
                "instruction-source-registry: a fixture that must be a valid registry was rejected ({}): {}",
                case.get("note").and_then(Value::as_str).unwrap_or(""),
                problems[0]
            ));
        }
    }
    for case in bundle["invalid"].as_array().unwrap() {
        let registry = case.get("registry").cloned().unwrap_or(Value::Null);
        let problems = evaluate_instruction_source_registry(&registry, &schemas);
        if problems.is_empty() {
            failures.push(format!(
                "instruction-source-registry: a fixture that must be rejected validated clean ({})",
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

    #[test]
    fn a_missing_schema_fails_closed() {
        let dir = std::env::temp_dir().join(format!(
            "instruction-source-registry-missing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let outcome = run(&dir);
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].contains("is missing"));
    }
}
