//! Port of `scripts/kernel-validate.mjs`'s
//! `existing-project-compatibility-mode` check (package 7, subpackage 7c):
//! `registries/operating-model/existing-project-compatibility-mode.schema.json`
//! is the specialised container/payload schema for a workspace-connection
//! registry document; each connection's own envelope is validated
//! separately against `scoped-record.schema.json`. Scan DATA is Instance,
//! like the source registry and the intake register: the Kernel ships the
//! schema, the product-neutral fixtures and the composite algorithm
//! ([`meridian_app::operating_model::existing_project_compatibility_mode`]),
//! never a canonical data file. The contract is a MANDATORY part of this
//! Kernel: a missing schema or missing fixtures is a FAIL. Unlike the other
//! three subpackage 7c families, this fixtures bundle carries no top-level
//! `"resolution"` object — the source-ref resolver a composed rule
//! candidate needs is built internally, per workspace connection, from
//! that SAME connection's own `discovered_sources`
//! ([`meridian_app::operating_model::existing_project_compatibility_mode::build_source_resolver`]).
//! Discovered sources compose with the REAL
//! `instruction-source-registry.schema.json`, and rule candidates with the
//! REAL `controlled-rule-intake.schema.json` — both loaded here alongside
//! this family's own schema and passed through, never a second copy of
//! either contract's shape.

use std::fs;
use std::path::Path;

use meridian_app::operating_model::existing_project_compatibility_mode::{
    evaluate_existing_project_compatibility_mode, EvalOpts,
};
use meridian_app::source_format::json_schema;
use serde_json::Value;

const SCHEMA_NAME: &str = "existing-project-compatibility-mode.schema.json";

pub struct Outcome {
    pub failures: Vec<String>,
}

fn read_schema(
    dir: &Path,
    name: &str,
    label: &str,
    failures: &mut Vec<String>,
    ok: &mut bool,
) -> Option<Value> {
    match fs::read_to_string(dir.join(name)) {
        Ok(raw) => match serde_json::from_str::<Value>(&raw) {
            Ok(parsed) => Some(parsed),
            Err(error) => {
                failures.push(format!(
                    "existing-project-compatibility-mode: {name} is not valid JSON: {error}"
                ));
                *ok = false;
                None
            }
        },
        Err(_) => {
            failures.push(format!(
                "existing-project-compatibility-mode: registries/operating-model/{name} is missing; {label}"
            ));
            *ok = false;
            None
        }
    }
}

pub fn run(kernel_root: &Path) -> Outcome {
    let dir = kernel_root.join("registries").join("operating-model");
    let mut failures = Vec::new();

    let Some(schema_raw) = fs::read_to_string(dir.join(SCHEMA_NAME)).ok() else {
        failures.push(format!(
            "existing-project-compatibility-mode: registries/operating-model/{SCHEMA_NAME} is missing; the existing-project compatibility mode contract is a mandatory part of this Kernel, not an optional add-on"
        ));
        return Outcome { failures };
    };

    let mut ok = true;
    let mut schema: Option<Value> = None;
    match serde_json::from_str::<Value>(&schema_raw) {
        Ok(parsed) => schema = Some(parsed),
        Err(error) => {
            failures.push(format!(
                "existing-project-compatibility-mode: {SCHEMA_NAME} is not valid JSON: {error}"
            ));
            ok = false;
        }
    }

    let envelope = read_schema(
        &dir,
        "scoped-record.schema.json",
        "the scan record composes with the record envelope and cannot be checked without it",
        &mut failures,
        &mut ok,
    );
    let source_schema = read_schema(
        &dir,
        "instruction-source-registry.schema.json",
        "discovered sources compose with that contract and cannot be checked without it",
        &mut failures,
        &mut ok,
    );
    let rule_intake_schema = read_schema(
        &dir,
        "controlled-rule-intake.schema.json",
        "rule candidates compose with that contract and cannot be checked without it",
        &mut failures,
        &mut ok,
    );

    if let Some(schema) = &schema {
        if let Err(error) = json_schema::assert_supported_deep(schema, SCHEMA_NAME) {
            failures.push(format!(
                "existing-project-compatibility-mode: the schema uses a construct this validator cannot check: {error}"
            ));
            ok = false;
        }
    }

    let fixtures_path = dir
        .join("fixtures")
        .join("existing-project-compatibility-mode.fixtures.json");
    let Ok(fx_raw) = fs::read_to_string(&fixtures_path) else {
        failures.push(
            "existing-project-compatibility-mode: the schema carries no fixtures (registries/operating-model/fixtures/existing-project-compatibility-mode.fixtures.json); a schema no run exercises is not one this gate has reached"
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
                "existing-project-compatibility-mode: the fixtures file is not valid JSON: {error}"
            ));
            return Outcome { failures };
        }
    };
    if !bundle.is_object() {
        failures.push(
            "existing-project-compatibility-mode: the fixtures file must be an object with non-empty \"valid\" and \"invalid\" arrays"
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
                "existing-project-compatibility-mode: the fixtures file has no non-empty \"{key}\" array"
            ));
        }
    }
    if !bundle_ok {
        return Outcome { failures };
    }

    let opts = EvalOpts {
        registry_schema: schema.as_ref().unwrap(),
        envelope_schema: envelope.as_ref().unwrap(),
        source_registry_schema: source_schema.as_ref().unwrap(),
        rule_intake_schema: rule_intake_schema.as_ref().unwrap(),
    };

    for case in bundle["valid"].as_array().unwrap() {
        let registry = case.get("registry").cloned().unwrap_or(Value::Null);
        let problems = evaluate_existing_project_compatibility_mode(&registry, &opts);
        if !problems.is_empty() {
            failures.push(format!(
                "existing-project-compatibility-mode: a fixture that must be a valid registry was rejected ({}): {}",
                case.get("note").and_then(Value::as_str).unwrap_or(""),
                problems[0].message()
            ));
        }
    }
    for case in bundle["invalid"].as_array().unwrap() {
        let registry = case.get("registry").cloned().unwrap_or(Value::Null);
        let problems = evaluate_existing_project_compatibility_mode(&registry, &opts);
        if problems.is_empty() {
            failures.push(format!(
                "existing-project-compatibility-mode: a fixture that must be rejected validated clean ({})",
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
