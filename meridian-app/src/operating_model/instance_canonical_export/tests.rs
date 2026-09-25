#![cfg(test)]
//! The `instance-canonical-export` production route over the real schemas
//! and fixtures, a fake reader and the real adapter.

use serde_json::Value;

use super::*;
use crate::operating_model::migration_boundary::test_support::{
    fixtures, fs_reader, io, real_json, real_reader, schema, texts,
};

const FIXTURES: &str = "instance-canonical-export.fixtures.json";
const ENVELOPE: &str = "scoped-record.schema.json";

fn paths() -> Vec<String> {
    vec![schema(SCHEMA_NAME), schema(ENVELOPE), fixtures(FIXTURES)]
}

fn run(reader: &dyn WorkspaceReader) -> Vec<String> {
    texts(&evaluate(reader).diagnostics)
}

fn with_bundle(mutate: impl FnOnce(&mut Value)) -> Vec<String> {
    let mut bundle = real_json(&fixtures(FIXTURES));
    mutate(&mut bundle);
    let reader = real_reader(&paths()).with_file(&fixtures(FIXTURES), &bundle.to_string());
    run(&reader)
}

#[test]
fn instance_canonical_export_the_real_kernel_contract_is_clean_through_the_real_adapter() {
    assert_eq!(run(&fs_reader()), Vec::<String>::new());
    assert_eq!(run(&real_reader(&paths())), Vec::<String>::new());
}

#[test]
fn instance_canonical_export_every_real_fixture_takes_the_production_pipeline() {
    let registry = real_json(&schema(SCHEMA_NAME));
    let envelope = real_json(&schema(ENVELOPE));
    let bundle = real_json(&fixtures(FIXTURES));
    let schemas = schemas(&registry, &envelope);
    let plans = plan_responses(bundle.as_object().unwrap());
    let content = source_contents(bundle.as_object().unwrap());
    let valid = bundle["valid"].as_array().unwrap();
    let invalid = bundle["invalid"].as_array().unwrap();
    assert_eq!((valid.len(), invalid.len()), (4, 23));
    for case in valid {
        let outcome = check_container(&case["registry"], &schemas, &plans, &content);
        assert!(
            matches!(outcome, CaseOutcome::Accepted(_)),
            "{}",
            case["note"]
        );
    }
    for case in invalid {
        let outcome = check_container(&case["registry"], &schemas, &plans, &content);
        assert!(
            matches!(
                outcome,
                CaseOutcome::SchemaRejected(_) | CaseOutcome::DomainRejected(_)
            ),
            "{} was not rejected by the schema or domain",
            case["note"]
        );
    }
}

#[test]
fn instance_canonical_export_missing_malformed_and_unreadable_files_fail_closed() {
    assert_eq!(
        run(&real_reader(&paths()[1..])),
        ["registries/operating-model/instance-canonical-export.schema.json is missing; the canonical export contract is a mandatory part of this Kernel, not an optional add-on"]
    );
    let unreadable = run(&real_reader(&paths()).with_error(&schema(ENVELOPE), io("denied")));
    assert_eq!(
        unreadable,
        ["registries/operating-model/scoped-record.schema.json could not be read: denied"]
    );
    assert_eq!(
        run(&real_reader(&[schema(SCHEMA_NAME), fixtures(FIXTURES)])),
        ["registries/operating-model/scoped-record.schema.json is missing; a canonical export composes with the record envelope and cannot be checked without it"]
    );
    let bad = run(&real_reader(&paths()).with_file(&fixtures(FIXTURES), "{"));
    assert!(bad[0].starts_with("the fixtures file is not valid JSON: "));
}

#[test]
fn instance_canonical_export_a_missing_resolver_map_fails_the_valid_exports_that_need_it() {
    for key in ["plan_resolution", "source_content_resolution"] {
        let removed = with_bundle(|b| {
            b.as_object_mut().unwrap().remove(key);
        });
        assert!(
            !removed.is_empty()
                && removed
                    .iter()
                    .all(|m| m.starts_with("a fixture that must be a valid registry was rejected")),
            "{key}: {removed:?}"
        );
    }
}

/// The resolved plan's own record kind and fingerprint are checked: a
/// wrong kind and a stale fingerprint are distinct rejections.
#[test]
fn instance_canonical_export_wrong_plan_kind_and_stale_plan_fingerprint_are_rejected() {
    let wrong_kind = with_bundle(|b| {
        for plan in b["plan_resolution"].as_object_mut().unwrap().values_mut() {
            plan["record_type"] = Value::from("instance-canonical-export");
        }
    });
    assert!(wrong_kind.iter().any(|m| m.contains(
        "resolved record_type is \"instance-canonical-export\", not \"instance-migration-plan\""
    )));
    let stale = with_bundle(|b| {
        for plan in b["plan_resolution"].as_object_mut().unwrap().values_mut() {
            plan["plan_fingerprint"] = Value::from("f".repeat(64));
        }
    });
    assert!(stale
        .iter()
        .any(|m| m
            .contains("an export pinned to a stale or different version of the plan's content")));
}

#[test]
fn instance_canonical_export_the_composition_point_types_one_resolved_export() {
    let registry = real_json(&schema(SCHEMA_NAME));
    let envelope = real_json(&schema(ENVELOPE));
    let schemas = schemas(&registry, &envelope);
    let bundle = real_json(&fixtures(FIXTURES));
    let export = bundle["valid"][1]["registry"]["exports"][0].clone();
    let typed = typed_export(&export, "q".to_string(), &schemas).unwrap();
    assert_eq!(typed.head.id.as_str(), export["id"].as_str().unwrap());
    let mut broken = export;
    broken["payload"]["records"][0]
        .as_object_mut()
        .unwrap()
        .remove("authority");
    let rejected = typed_export(&broken, "q".to_string(), &schemas).unwrap_err();
    assert!(rejected[0]
        .message()
        .starts_with("entry 0 record 0 envelope "));
}

/// Container/envelope schema short-circuit (`COMPATIBILITY.md`): the real
/// valid[1] container with the harness's own domain defect — a
/// well-formed but foreign `idempotency_key`.
#[test]
fn instance_canonical_export_a_container_or_envelope_schema_violation_short_circuits_before_the_domain(
) {
    let bundle = real_json(&fixtures(FIXTURES));
    let mut doc = bundle["valid"][1]["registry"].clone();
    doc["exports"][0]["payload"]["idempotency_key"] = Value::from("a".repeat(64));
    let registry = real_json(&schema(SCHEMA_NAME));
    let envelope = real_json(&schema(ENVELOPE));
    let schemas = schemas(&registry, &envelope);
    let plans = plan_responses(bundle.as_object().unwrap());
    let content = source_contents(bundle.as_object().unwrap());
    crate::operating_model::migration_boundary::test_support::assert_schema_short_circuit(
        &doc,
        "exports",
        "canonical export \"sample-canonical-export-one\" idempotency_key \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\" does not match the recomputed key \"6a679cf719f91ef64593485de9c3f5a5a62f2a8ad118b6a8b327ac24fb3e3149\" derived from this export's own plan_ref and plan_fingerprint; idempotency_key is never an arbitrary free-form string",
        |doc| check_container(doc, &schemas, &plans, &content),
    );
}

/// The Rust half of the matched Node/Rust invalid-JSON content case
/// (`COMPATIBILITY.md`; `test/conformance-harness.test.mjs`, block
/// `rust-architecture-conformance-7` invalid JSON content): the real,
/// unmutated invalid[21] fixture ("planted an exported record whose
/// application/json payload content is not valid JSON"). Both sides reject
/// it with the SAME three diagnostics in the SAME order; the ONLY
/// difference is the tail of the invalid-JSON line after
/// `media_type "application/json": ` — Node appends V8's parser message,
/// Rust a neutral one.
#[test]
fn instance_canonical_export_invalid_json_content_is_rejected_like_the_reference() {
    let bundle = real_json(&fixtures(FIXTURES));
    assert_eq!(
        bundle["invalid"][21]["note"],
        "planted an exported record whose application/json payload content is not valid JSON"
    );
    let doc = bundle["invalid"][21]["registry"].clone();
    let registry = real_json(&schema(SCHEMA_NAME));
    let envelope = real_json(&schema(ENVELOPE));
    let schemas = schemas(&registry, &envelope);
    let plans = plan_responses(bundle.as_object().unwrap());
    let content = source_contents(bundle.as_object().unwrap());
    let outcome = check_container(&doc, &schemas, &plans, &content);
    assert!(matches!(outcome, CaseOutcome::DomainRejected(_)));
    let export = "canonical export \"sample-canonical-export-one\"";
    assert_eq!(
        texts(&crate::operating_model::migration_boundary::outcome_diagnostics(&outcome)),
        [
            format!("{export} exported record \"sample-migrated-record-one\" diverges from the plan's own target for the same group; the exported record and the plan's target must be structurally identical, including $schema (property: completeness)"),
            format!("{export} exported record \"sample-migrated-record-one\" payload content is not valid JSON for media_type \"application/json\": the content does not parse as a single JSON value"),
            format!("{export} target \"sample-migrated-record-one\": exported payload does not match the resolved actual content of its contributing source unit(s) byte-for-byte (media_type/encoding/content/digest); a changed value or byte is never accepted as preserved (property: content preservation)"),
        ]
    );
}
