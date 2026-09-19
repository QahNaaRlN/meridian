//! Verification-only producer for the cross-language conformance harness.

use meridian_app::source_format::{json_schema, yaml};
use serde_json::Value;

fn main() {
    let source =
        include_str!("../../verification/conformance-harness/fixtures/source-format-corpus.json");
    let corpus: Value = serde_json::from_str(source).expect("parse embedded source-format corpus");

    for case in corpus["yaml"].as_array().expect("yaml cases") {
        let name = case["name"].as_str().expect("yaml case name");
        let source = case["source"].as_str().expect("yaml source");
        let observation = match yaml::parse(source) {
            Ok(value) => format!("accepted:{}", json(&value)),
            Err(_) => "rejected".to_owned(),
        };
        println!("WARN  yaml/{name}: {observation}");
    }

    for case in corpus["schema"].as_array().expect("schema cases") {
        let name = case["name"].as_str().expect("schema case name");
        let observation = match json_schema::validate(&case["data"], &case["schema"]) {
            Ok(mut errors) => {
                errors.sort();
                format!(
                    "accepted:{}",
                    json(&Value::Array(
                        errors.into_iter().map(Value::String).collect()
                    ))
                )
            }
            Err(_) => "rejected".to_owned(),
        };
        println!("WARN  schema/{name}: {observation}");
    }
}

fn json(value: &Value) -> String {
    serde_json::to_string(value).expect("serialize observation")
}
