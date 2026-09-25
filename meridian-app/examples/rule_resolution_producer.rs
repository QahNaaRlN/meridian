//! Verification-only producer for rule-resolution cross-language conformance.
//!
//! Resolves the shared corpus embedded below or, when a path is given as the
//! only argument, the corpus at that path — the fail-closed corpus of
//! `rust-business-contract-qualification`, which the CLI producer cannot
//! share because it reports a rejection together with its exit code.

use meridian_app::rule_resolution;
use serde_json::Value;

fn main() {
    let corpus: Value = match std::env::args().nth(1) {
        Some(path) => serde_json::from_str(
            &std::fs::read_to_string(&path).expect("read the rule-resolution corpus"),
        )
        .expect("parse the rule-resolution corpus"),
        None => serde_json::from_str(include_str!(
            "../../verification/conformance-harness/fixtures/rule-resolution-corpus.json"
        ))
        .expect("parse embedded rule-resolution corpus"),
    };
    let applicability_schema: Value = serde_json::from_str(include_str!(
        "../../registries/rule-resolution/applicability.schema.json"
    ))
    .expect("parse applicability schema");
    let output_schema: Value = serde_json::from_str(include_str!(
        "../../registries/rule-resolution/resolver-output.schema.json"
    ))
    .expect("parse output schema");

    for case in corpus["cases"].as_array().expect("rule-resolution cases") {
        let name = case["name"].as_str().expect("case name");
        let observation =
            match rule_resolution::resolve(&case["request"], &applicability_schema, &output_schema)
            {
                Ok(value) => format!("accepted:{}", json(&value)),
                Err(_) => "rejected".to_owned(),
            };
        println!("WARN  resolver/{name}: {observation}");
    }
}

fn json(value: &Value) -> String {
    serde_json::to_string(value).expect("serialize observation")
}
