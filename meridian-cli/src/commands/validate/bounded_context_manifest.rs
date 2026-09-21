//! Port of `scripts/kernel-validate.mjs`'s `bounded-context-manifest`
//! check (subpackage 7b): `registries/operating-model/context-manifest.schema.json`
//! is the specialised schema for the bounded context manifest of ONE
//! execution run (`record_type: context-manifest`). Its envelope is
//! validated against the existing `scoped-record.schema.json`. Manifest
//! DATA is Instance, like the task specification and the human-control
//! record: the Kernel ships the schema, the product-neutral fixtures and
//! the composite algorithm
//! ([`meridian_app::operating_model::bounded_context_manifest`]), never a
//! canonical data file. The contract is a MANDATORY part of this Kernel: a
//! missing schema or missing fixtures is a FAIL, not an informational
//! skip. The fixtures bundle carries its own companion `"resolution"`
//! object — the external record-resolution boundary the manifest's
//! `run_state_checkpoint` is checked against.

use std::fs;
use std::path::Path;

use meridian_app::operating_model::bounded_context_manifest::{
    evaluate_context_manifest, make_record_resolver, EvalSchemas,
};
use meridian_app::source_format::json_schema;
use serde_json::Value;

const SCHEMA_NAME: &str = "context-manifest.schema.json";

pub struct Outcome {
    pub failures: Vec<String>,
}

pub fn run(kernel_root: &Path) -> Outcome {
    let dir = kernel_root.join("registries").join("operating-model");
    let mut failures = Vec::new();

    let Some(schema_raw) = fs::read_to_string(dir.join(SCHEMA_NAME)).ok() else {
        failures.push(format!(
            "bounded-context-manifest: registries/operating-model/{SCHEMA_NAME} is missing; the bounded context manifest contract is a mandatory part of this Kernel, not an optional add-on"
        ));
        return Outcome { failures };
    };

    let mut ok = true;
    let mut schema: Option<Value> = None;
    match serde_json::from_str::<Value>(&schema_raw) {
        Ok(parsed) => schema = Some(parsed),
        Err(error) => {
            failures.push(format!(
                "bounded-context-manifest: {SCHEMA_NAME} is not valid JSON: {error}"
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
                    "bounded-context-manifest: scoped-record.schema.json is not valid JSON: {error}"
                ));
                ok = false;
            }
        },
        Err(_) => {
            failures.push(
                "bounded-context-manifest: registries/operating-model/scoped-record.schema.json is missing; the manifest record composes with the record envelope and cannot be checked without it"
                    .to_string(),
            );
            ok = false;
        }
    }

    if let Some(schema) = &schema {
        if let Err(error) = json_schema::assert_supported_deep(schema, SCHEMA_NAME) {
            failures.push(format!(
                "bounded-context-manifest: the schema uses a construct this validator cannot check: {error}"
            ));
            ok = false;
        }
    }

    let fixtures_path = dir.join("fixtures").join("context-manifest.fixtures.json");
    let Ok(fx_raw) = fs::read_to_string(&fixtures_path) else {
        failures.push(
            "bounded-context-manifest: the schema carries no fixtures (registries/operating-model/fixtures/context-manifest.fixtures.json); a schema no run exercises is not one this gate has reached"
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
                "bounded-context-manifest: the fixtures file is not valid JSON: {error}"
            ));
            return Outcome { failures };
        }
    };
    if !bundle.is_object() {
        failures.push(
            "bounded-context-manifest: the fixtures file must be an object with non-empty \"valid\" and \"invalid\" arrays"
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
                "bounded-context-manifest: the fixtures file has no non-empty \"{key}\" array"
            ));
        }
    }
    if !bundle.get("resolution").is_some_and(|r| r.is_object()) {
        bundle_ok = false;
        failures.push(
            "bounded-context-manifest: the fixtures file carries no \"resolution\" object; the pinned execution-run, task-specification and run-human-control records are resolved OUTSIDE the manifest, and a bundle that resolves nothing cannot exercise the checkpoint against its actual run"
                .to_string(),
        );
    }
    if !bundle_ok {
        return Outcome { failures };
    }

    let schemas = EvalSchemas {
        record_schema: schema.as_ref().unwrap(),
        envelope_schema: envelope.as_ref().unwrap(),
    };
    let resolver = make_record_resolver(&bundle["resolution"]);

    for case in bundle["valid"].as_array().unwrap() {
        let spec = case.get("spec").cloned().unwrap_or(Value::Null);
        let problems = evaluate_context_manifest(&spec, &schemas, Some(&resolver));
        if !problems.is_empty() {
            failures.push(format!(
                "bounded-context-manifest: a fixture that must be a valid context manifest was rejected ({}): {}",
                case.get("note").and_then(Value::as_str).unwrap_or(""),
                problems[0]
            ));
        }
    }
    for case in bundle["invalid"].as_array().unwrap() {
        let spec = case.get("spec").cloned().unwrap_or(Value::Null);
        let problems = evaluate_context_manifest(&spec, &schemas, Some(&resolver));
        if problems.is_empty() {
            failures.push(format!(
                "bounded-context-manifest: a fixture that must be rejected validated clean ({})",
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
