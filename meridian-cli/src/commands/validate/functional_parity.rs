//! Port of `scripts/kernel-validate.mjs`'s `functional-parity` check
//! (subpackage 7b): `verification/functional-parity/` carries a portable
//! functional-parity evidence contract, its JSON Schema and product-neutral
//! fixtures. The generic `$schema` pass never reaches a JSON Schema file or
//! a `.json` fixtures bundle, so — as with `rule-resolution` — the schema
//! is parsed, walked for unsupported keywords, and exercised against its
//! bundled fixtures here, before any Instance population against it
//! exists. The inference rules the Draft 7 subset cannot express live in
//! [`meridian_app::operating_model::functional_parity`].

use std::fs;
use std::path::Path;

use meridian_app::operating_model::functional_parity::functional_parity_consistency;
use meridian_app::source_format::json_schema;
use serde_json::Value;

const SCHEMA_NAME: &str = "functional-parity-evidence.schema.json";

pub struct Outcome {
    pub failures: Vec<String>,
}

pub fn run(kernel_root: &Path) -> Outcome {
    let dir = kernel_root.join("verification").join("functional-parity");
    let mut failures = Vec::new();

    let Some(schema_raw) = fs::read_to_string(dir.join(SCHEMA_NAME)).ok() else {
        // Mirrors the Node reference's own `info(...)` skip: unlike
        // task-pattern-registry and instruction-source-registry, this
        // contract is reachable only once the PHASE D evidence schema
        // exists in the Kernel — its absence is not itself a mandatory
        // gap this check enforces.
        return Outcome { failures };
    };

    let mut fp_ok = true;
    let mut schema: Option<Value> = None;
    match serde_json::from_str::<Value>(&schema_raw) {
        Ok(parsed) => schema = Some(parsed),
        Err(error) => {
            failures.push(format!(
                "functional-parity: {SCHEMA_NAME} is not valid JSON: {error}"
            ));
            fp_ok = false;
        }
    }
    if let Some(schema) = &schema {
        if let Err(error) = json_schema::assert_supported_deep(schema, SCHEMA_NAME) {
            failures.push(format!(
                "functional-parity: {SCHEMA_NAME} uses a construct this validator cannot check: {error}"
            ));
            fp_ok = false;
        }
    }

    let fixtures_path = dir
        .join("fixtures")
        .join("functional-parity-evidence.fixtures.json");
    let Ok(fx_raw) = fs::read_to_string(&fixtures_path) else {
        failures.push(
            "functional-parity: the PHASE D evidence schema carries no fixtures (verification/functional-parity/fixtures/functional-parity-evidence.fixtures.json); a schema no run exercises is not one this gate has reached"
                .to_string(),
        );
        return Outcome { failures };
    };
    if !fp_ok {
        return Outcome { failures };
    }

    let bundle: Value = match serde_json::from_str(&fx_raw) {
        Ok(value) => value,
        Err(error) => {
            failures.push(format!(
                "functional-parity: the fixtures file is not valid JSON: {error}"
            ));
            return Outcome { failures };
        }
    };
    let Some(groups) = bundle.as_array() else {
        failures.push(
            "functional-parity: the fixtures file must be an array carrying one group for the evidence schema; it is not an array"
                .to_string(),
        );
        return Outcome { failures };
    };
    if groups.is_empty() {
        failures.push(
            "functional-parity: the fixtures file is an empty array; the evidence schema is not covered"
                .to_string(),
        );
        return Outcome { failures };
    }

    let matching: Vec<&Value> = groups
        .iter()
        .filter(|g| g.get("schema").and_then(Value::as_str) == Some(SCHEMA_NAME))
        .collect();
    let strays: Vec<&Value> = groups
        .iter()
        .filter(|g| g.get("schema").and_then(Value::as_str) != Some(SCHEMA_NAME))
        .collect();
    if let Some(stray) = strays.first() {
        failures.push(format!(
            "functional-parity: a fixture group names schema \"{}\", which is not the PHASE D evidence schema",
            stray.get("schema").and_then(Value::as_str).unwrap_or("")
        ));
        return Outcome { failures };
    }
    if matching.is_empty() {
        failures.push(format!(
            "functional-parity: no fixture group for \"{SCHEMA_NAME}\"; the evidence schema must be covered"
        ));
        return Outcome { failures };
    }
    if matching.len() > 1 {
        failures.push(format!(
            "functional-parity: schema \"{SCHEMA_NAME}\" has more than one fixture group; exactly one is expected"
        ));
        return Outcome { failures };
    }

    let group = matching[0];
    let mut group_shape_ok = true;
    for key in ["valid", "invalid"] {
        let is_nonempty = group
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty());
        if !is_nonempty {
            failures.push(format!(
                "functional-parity: fixture group \"{SCHEMA_NAME}\" has no non-empty \"{key}\" array"
            ));
            group_shape_ok = false;
        }
    }
    if !group_shape_ok {
        return Outcome { failures };
    }

    let schema = schema.as_ref().unwrap();
    for case in group["valid"].as_array().unwrap() {
        let doc = case.get("doc").cloned().unwrap_or(Value::Null);
        let note = case.get("note").and_then(Value::as_str).unwrap_or("");
        match json_schema::validate(&doc, schema) {
            Err(error) => failures.push(format!(
                "functional-parity: a fixture that must satisfy {SCHEMA_NAME} threw ({note}): {error}"
            )),
            Ok(errors) if !errors.is_empty() => failures.push(format!(
                "functional-parity: a fixture that must satisfy {SCHEMA_NAME} did not ({note}): {}",
                errors[0]
            )),
            Ok(_) => {
                let semantic = functional_parity_consistency(&doc);
                if !semantic.is_empty() {
                    failures.push(format!(
                        "functional-parity: a fixture that must satisfy the evidence inference rules did not ({note}): {}",
                        semantic[0]
                    ));
                }
            }
        }
    }
    for case in group["invalid"].as_array().unwrap() {
        let doc = case.get("doc").cloned().unwrap_or(Value::Null);
        let note = case.get("note").and_then(Value::as_str).unwrap_or("");
        match json_schema::validate(&doc, schema) {
            Err(error) => failures.push(format!(
                "functional-parity: an invalid fixture for {SCHEMA_NAME} threw instead of being rejected with errors ({note}): {error}"
            )),
            Ok(errors) => {
                let semantic = if errors.is_empty() {
                    functional_parity_consistency(&doc)
                } else {
                    Vec::new()
                };
                if errors.is_empty() && semantic.is_empty() {
                    failures.push(format!(
                        "functional-parity: a fixture that must be rejected by {SCHEMA_NAME} or its inference rules validated clean ({note})"
                    ));
                }
            }
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
