//! Port of `scripts/kernel-validate.mjs`'s `rule-resolution` check: the two
//! PHASE B schemas (`registries/rule-resolution/{applicability,resolver-output}.schema.json`)
//! are reachable by this gate and exercised against their bundled fixtures
//! (`registries/rule-resolution/fixtures/rule-resolution.fixtures.json`),
//! using exactly the ported strict Draft 7 adapter
//! (`meridian_app::source_format::json_schema`) — no composite algorithm
//! beyond schema validation is required for this specific check, unlike
//! `functional-parity` and the other composite operating-model contracts
//! (`meridian-cli/src/commands/validate/mod.rs`, "blocked").

use std::fs;
use std::path::Path;

use meridian_app::source_format::json_schema;
use serde_json::Value;

const NAMES: [&str; 2] = ["applicability.schema.json", "resolver-output.schema.json"];

pub struct Outcome {
    pub failures: Vec<String>,
    pub satisfied: usize,
    pub rejected: usize,
}

pub fn run(kernel_root: &Path) -> Outcome {
    let dir = kernel_root.join("registries").join("rule-resolution");
    let mut failures = Vec::new();

    let raw: Vec<Option<String>> = NAMES
        .iter()
        .map(|name| fs::read_to_string(dir.join(name)).ok())
        .collect();
    if raw.iter().all(Option::is_none) {
        return Outcome {
            failures,
            satisfied: 0,
            rejected: 0,
        };
    }

    let mut schemas: Vec<Option<Value>> = vec![None, None];
    let mut ok = true;
    for (index, name) in NAMES.iter().enumerate() {
        match &raw[index] {
            None => {
                failures.push(format!(
                    "rule-resolution: {name} is missing while the other PHASE B schema is present; that is a gap, not an opt-out"
                ));
                ok = false;
            }
            Some(text) => match serde_json::from_str::<Value>(text) {
                Ok(parsed) => match json_schema::assert_supported_deep(&parsed, name) {
                    Ok(()) => schemas[index] = Some(parsed),
                    Err(error) => {
                        failures.push(format!(
                            "rule-resolution: {name} uses a construct this validator cannot check: {error}"
                        ));
                        ok = false;
                    }
                },
                Err(error) => {
                    failures.push(format!(
                        "rule-resolution: {name} is not valid JSON: {error}"
                    ));
                    ok = false;
                }
            },
        }
    }

    let mut satisfied = 0usize;
    let mut rejected = 0usize;

    let fixtures_path = dir.join("fixtures").join("rule-resolution.fixtures.json");
    let Ok(fixtures_raw) = fs::read_to_string(&fixtures_path) else {
        failures.push(
            "rule-resolution: the PHASE B schemas carry no fixtures (registries/rule-resolution/fixtures/rule-resolution.fixtures.json); a schema no run exercises is not one this gate has reached"
                .to_string(),
        );
        return Outcome {
            failures,
            satisfied,
            rejected,
        };
    };
    if !ok {
        return Outcome {
            failures,
            satisfied,
            rejected,
        };
    }

    let bundle: Value = match serde_json::from_str(&fixtures_raw) {
        Ok(value) => value,
        Err(error) => {
            failures.push(format!(
                "rule-resolution: the fixtures file is not valid JSON: {error}"
            ));
            return Outcome {
                failures,
                satisfied,
                rejected,
            };
        }
    };
    let Some(groups) = bundle.as_array() else {
        failures.push(
            "rule-resolution: the fixtures file must be an array of one group per PHASE B schema; it is not an array"
                .to_string(),
        );
        return Outcome {
            failures,
            satisfied,
            rejected,
        };
    };
    if groups.is_empty() {
        failures.push(
            "rule-resolution: the fixtures file is an empty array; neither PHASE B schema is covered"
                .to_string(),
        );
        return Outcome {
            failures,
            satisfied,
            rejected,
        };
    }

    let mut by_name: std::collections::HashMap<String, &Value> = std::collections::HashMap::new();
    for group in groups {
        let name = group.get("schema").and_then(Value::as_str).unwrap_or("");
        if !NAMES.contains(&name) {
            failures.push(format!(
                "rule-resolution: a fixture group names schema \"{name}\", which is not one of the two PHASE B schemas"
            ));
            continue;
        }
        if by_name.contains_key(name) {
            failures.push(format!(
                "rule-resolution: schema \"{name}\" has more than one fixture group; exactly one is expected"
            ));
            continue;
        }
        by_name.insert(name.to_string(), group);
    }
    for name in NAMES {
        if !by_name.contains_key(name) {
            failures.push(format!(
                "rule-resolution: no fixture group for \"{name}\"; both PHASE B schemas must be covered"
            ));
        }
    }

    for (index, name) in NAMES.iter().enumerate() {
        let Some(group) = by_name.get(*name) else {
            continue;
        };
        let Some(schema) = &schemas[index] else {
            continue;
        };
        let mut group_shape_ok = true;
        for key in ["valid", "invalid"] {
            let is_nonempty_array = group
                .get(key)
                .and_then(Value::as_array)
                .is_some_and(|a| !a.is_empty());
            if !is_nonempty_array {
                failures.push(format!(
                    "rule-resolution: fixture group \"{name}\" has no non-empty \"{key}\" array"
                ));
                group_shape_ok = false;
            }
        }
        if !group_shape_ok {
            continue;
        }
        for case in group["valid"].as_array().unwrap() {
            let doc = &case["doc"];
            match json_schema::validate(doc, schema) {
                Ok(errors) if errors.is_empty() => satisfied += 1,
                Ok(errors) => failures.push(format!(
                    "rule-resolution: a fixture that must satisfy {name} did not ({}): {}",
                    case.get("note").and_then(Value::as_str).unwrap_or(""),
                    errors[0]
                )),
                Err(error) => failures.push(format!(
                    "rule-resolution: a fixture that must satisfy {name} threw ({}): {error}",
                    case.get("note").and_then(Value::as_str).unwrap_or("")
                )),
            }
        }
        for case in group["invalid"].as_array().unwrap() {
            let doc = &case["doc"];
            match json_schema::validate(doc, schema) {
                Ok(errors) if !errors.is_empty() => rejected += 1,
                Ok(_) => failures.push(format!(
                    "rule-resolution: a fixture that must violate {name} validated clean ({})",
                    case.get("note").and_then(Value::as_str).unwrap_or("")
                )),
                Err(error) => failures.push(format!(
                    "rule-resolution: an invalid fixture for {name} threw instead of being rejected with errors ({}): {error}",
                    case.get("note").and_then(Value::as_str).unwrap_or("")
                )),
            }
        }
    }

    Outcome {
        failures,
        satisfied,
        rejected,
    }
}
