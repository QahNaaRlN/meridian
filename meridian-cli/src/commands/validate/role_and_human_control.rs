//! Port of `scripts/kernel-validate.mjs`'s `role-and-human-control` check
//! (subpackage 7b): two product-neutral Kernel contracts checked by one
//! implementation, exactly as the `execution-state-model` block:
//!   1. `role-registry.schema.json` — the built-in catalogue of the seven
//!      universal roles. It lives ONLY in `built-in-methodology`; the
//!      catalogue itself is `standards/workspace/role-registry.yaml`.
//!   2. `human-control.schema.json` — one execution run's human-control
//!      state in `run-state`. Concrete records are Instance data; the
//!      Kernel ships the schema, the fixtures and the composite algorithm
//!      ([`meridian_app::operating_model::role_and_human_control`]).
//!
//! A missing schema, catalogue or fixtures is a FAIL, not an informational
//! skip.

use std::fs;
use std::path::Path;

use meridian_app::operating_model::role_and_human_control::{
    evaluate_human_control, evaluate_role_registry, HumanControlSchemas, RoleRegistrySchemas,
};
use meridian_app::source_format::{json_schema, parse_yaml};
use serde_json::Value;

const REGISTRY_SCHEMA_NAME: &str = "role-registry.schema.json";
const CONTROL_SCHEMA_NAME: &str = "human-control.schema.json";

pub struct Outcome {
    pub failures: Vec<String>,
}

pub fn run(kernel_root: &Path) -> Outcome {
    let dir = kernel_root.join("registries").join("operating-model");
    let mut failures = Vec::new();

    let registry_schema_raw = fs::read_to_string(dir.join(REGISTRY_SCHEMA_NAME)).ok();
    let control_schema_raw = fs::read_to_string(dir.join(CONTROL_SCHEMA_NAME)).ok();
    if registry_schema_raw.is_none() || control_schema_raw.is_none() {
        let missing = if registry_schema_raw.is_none() {
            REGISTRY_SCHEMA_NAME
        } else {
            CONTROL_SCHEMA_NAME
        };
        failures.push(format!(
            "role-and-human-control: registries/operating-model/{missing} is missing; the role and human control contract is a mandatory part of this Kernel, not an optional add-on"
        ));
        return Outcome { failures };
    }

    let Some(catalogue_raw) =
        fs::read_to_string(kernel_root.join("standards/workspace/role-registry.yaml")).ok()
    else {
        failures.push(
            "role-and-human-control: standards/workspace/role-registry.yaml is missing; the built-in universal-role catalogue is a mandatory part of this Kernel"
                .to_string(),
        );
        return Outcome { failures };
    };

    let mut ok = true;
    let mut registry_schema: Option<Value> = None;
    match serde_json::from_str::<Value>(&registry_schema_raw.unwrap()) {
        Ok(parsed) => registry_schema = Some(parsed),
        Err(error) => {
            failures.push(format!(
                "role-and-human-control: {REGISTRY_SCHEMA_NAME} is not valid JSON: {error}"
            ));
            ok = false;
        }
    }
    let mut control_schema: Option<Value> = None;
    match serde_json::from_str::<Value>(&control_schema_raw.unwrap()) {
        Ok(parsed) => control_schema = Some(parsed),
        Err(error) => {
            failures.push(format!(
                "role-and-human-control: {CONTROL_SCHEMA_NAME} is not valid JSON: {error}"
            ));
            ok = false;
        }
    }
    let mut catalogue: Option<Value> = None;
    match parse_yaml(&catalogue_raw) {
        Ok(parsed) => catalogue = Some(parsed),
        Err(error) => {
            failures.push(format!(
                "role-and-human-control: cannot parse role-registry.yaml: {error}"
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
                    "role-and-human-control: scoped-record.schema.json is not valid JSON: {error}"
                ));
                ok = false;
            }
        },
        Err(_) => {
            failures.push(
                "role-and-human-control: registries/operating-model/scoped-record.schema.json is missing; each record composes with the record envelope and cannot be checked without it"
                    .to_string(),
            );
            ok = false;
        }
    }

    for (schema, name) in [
        (&registry_schema, REGISTRY_SCHEMA_NAME),
        (&control_schema, CONTROL_SCHEMA_NAME),
    ] {
        if let Some(schema) = schema {
            if let Err(error) = json_schema::assert_supported_deep(schema, name) {
                failures.push(format!(
                    "role-and-human-control: the schema uses a construct this validator cannot check: {error}"
                ));
                ok = false;
            }
        }
    }

    let fixtures_path = dir
        .join("fixtures")
        .join("role-and-human-control.fixtures.json");
    let Ok(fx_raw) = fs::read_to_string(&fixtures_path) else {
        failures.push(
            "role-and-human-control: the schemas carry no fixtures (registries/operating-model/fixtures/role-and-human-control.fixtures.json); a schema no run exercises is not one this gate has reached"
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
                "role-and-human-control: the fixtures file is not valid JSON: {error}"
            ));
            return Outcome { failures };
        }
    };
    if !bundle.is_object() {
        failures.push(
            "role-and-human-control: the fixtures file must be an object carrying \"registry\" and \"control\" groups, each with non-empty \"valid\" and \"invalid\" arrays"
                .to_string(),
        );
        return Outcome { failures };
    }
    let mut bundle_ok = true;
    for group in ["registry", "control"] {
        let Some(group_value) = bundle.get(group).filter(|g| g.is_object()) else {
            bundle_ok = false;
            failures.push(format!(
                "role-and-human-control: the fixtures file has no \"{group}\" group"
            ));
            continue;
        };
        for key in ["valid", "invalid"] {
            let is_nonempty = group_value
                .get(key)
                .and_then(Value::as_array)
                .is_some_and(|a| !a.is_empty());
            if !is_nonempty {
                bundle_ok = false;
                failures.push(format!(
                    "role-and-human-control: the fixtures \"{group}\" group has no non-empty \"{key}\" array"
                ));
            }
        }
    }
    if !bundle_ok {
        return Outcome { failures };
    }

    let reg_schemas = RoleRegistrySchemas {
        registry_schema: registry_schema.as_ref().unwrap(),
        envelope_schema: envelope.as_ref().unwrap(),
    };
    for case in bundle["registry"]["valid"].as_array().unwrap() {
        let spec = case.get("spec").cloned().unwrap_or(Value::Null);
        let problems = evaluate_role_registry(&spec, &reg_schemas);
        if !problems.is_empty() {
            failures.push(format!(
                "role-and-human-control: a fixture that must be a valid role registry was rejected ({}): {}",
                case.get("note").and_then(Value::as_str).unwrap_or(""),
                problems[0]
            ));
        }
    }
    for case in bundle["registry"]["invalid"].as_array().unwrap() {
        let spec = case.get("spec").cloned().unwrap_or(Value::Null);
        let problems = evaluate_role_registry(&spec, &reg_schemas);
        if problems.is_empty() {
            failures.push(format!(
                "role-and-human-control: a role registry fixture that must be rejected validated clean ({})",
                case.get("note").and_then(Value::as_str).unwrap_or("")
            ));
        }
    }

    let ctl_schemas = HumanControlSchemas {
        record_schema: control_schema.as_ref().unwrap(),
        envelope_schema: envelope.as_ref().unwrap(),
    };
    for case in bundle["control"]["valid"].as_array().unwrap() {
        let spec = case.get("spec").cloned().unwrap_or(Value::Null);
        let problems = evaluate_human_control(&spec, &ctl_schemas);
        if !problems.is_empty() {
            failures.push(format!(
                "role-and-human-control: a fixture that must be a valid human-control record was rejected ({}): {}",
                case.get("note").and_then(Value::as_str).unwrap_or(""),
                problems[0]
            ));
        }
    }
    for case in bundle["control"]["invalid"].as_array().unwrap() {
        let spec = case.get("spec").cloned().unwrap_or(Value::Null);
        let problems = evaluate_human_control(&spec, &ctl_schemas);
        if problems.is_empty() {
            failures.push(format!(
                "role-and-human-control: a human-control fixture that must be rejected validated clean ({})",
                case.get("note").and_then(Value::as_str).unwrap_or("")
            ));
        }
    }

    // the shipped catalogue itself is the canonical valid role registry.
    if failures.is_empty() {
        let problems = evaluate_role_registry(catalogue.as_ref().unwrap(), &reg_schemas);
        if !problems.is_empty() {
            failures.push(format!(
                "role-and-human-control: standards/workspace/role-registry.yaml is not a valid role registry: {}",
                problems[0]
            ));
        }
    }

    Outcome { failures }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_real_kernel_schemas_catalogue_and_fixtures_agree() {
        let kernel_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let outcome = run(kernel_root);
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    }
}
