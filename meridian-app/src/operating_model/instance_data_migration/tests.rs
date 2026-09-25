#![cfg(test)]
//! The `instance-data-migration` production route over the real schemas
//! and fixtures, a fake reader and the real adapter.

use serde_json::Value;

use super::*;
use crate::operating_model::migration_boundary::test_support::{
    fixtures, fs_reader, io, real, real_json, real_reader, schema, texts,
};

const FIXTURES: &str = "instance-data-migration.fixtures.json";
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
fn instance_data_migration_the_real_kernel_contract_is_clean_through_the_real_adapter() {
    assert_eq!(run(&fs_reader()), Vec::<String>::new());
    assert_eq!(run(&real_reader(&paths())), Vec::<String>::new());
}

/// Every one of the 43 real cases goes through the SAME production
/// pipeline: a valid one is accepted, an invalid one is rejected by the
/// schema or the domain — never by conversion drift.
#[test]
fn instance_data_migration_every_real_fixture_takes_the_production_pipeline() {
    let registry = real_json(&schema(SCHEMA_NAME));
    let envelope = real_json(&schema(ENVELOPE));
    let bundle = real_json(&fixtures(FIXTURES));
    let schemas = schemas(&registry, &envelope);
    let resolution = plan_resolution(bundle.as_object().unwrap());
    let valid = bundle["valid"].as_array().unwrap();
    let invalid = bundle["invalid"].as_array().unwrap();
    assert_eq!((valid.len(), invalid.len()), (6, 37));
    for case in valid {
        let outcome = check_container(&case["registry"], &schemas, &resolution);
        assert!(
            matches!(outcome, CaseOutcome::Accepted(_)),
            "{}",
            case["note"]
        );
    }
    let mut domain = 0;
    for case in invalid {
        match check_container(&case["registry"], &schemas, &resolution) {
            CaseOutcome::SchemaRejected(_) => {}
            CaseOutcome::DomainRejected(_) => domain += 1,
            _ => panic!("{} was not rejected by the schema or domain", case["note"]),
        }
    }
    assert!(
        domain > 20,
        "most invalid plans are schema-clean domain rejections: {domain}"
    );
}

#[test]
fn instance_data_migration_a_missing_or_unreadable_mandatory_file_fails_closed_and_is_distinguished(
) {
    let missing = run(&real_reader(&paths()[1..]));
    assert_eq!(
        missing,
        ["registries/operating-model/instance-data-migration.schema.json is missing; the instance-data migration contract is a mandatory part of this Kernel, not an optional add-on"]
    );
    let unreadable = run(&real_reader(&paths()).with_error(&schema(SCHEMA_NAME), io("denied")));
    assert_eq!(unreadable.len(), 1);
    assert!(unreadable[0].starts_with(
        "registries/operating-model/instance-data-migration.schema.json could not be read: "
    ));
    let no_envelope = run(&real_reader(&[schema(SCHEMA_NAME), fixtures(FIXTURES)]));
    assert_eq!(
        no_envelope,
        ["registries/operating-model/scoped-record.schema.json is missing; the migration plan composes with the record envelope and cannot be checked without it"]
    );
    let no_fixtures = run(&real_reader(&paths()[..2]));
    assert_eq!(
        no_fixtures,
        ["the schema carries no fixtures (registries/operating-model/fixtures/instance-data-migration.fixtures.json); a schema no run exercises is not one this gate has reached"]
    );
    let unreadable_fixtures =
        run(&real_reader(&paths()).with_error(&fixtures(FIXTURES), io("denied")));
    assert!(unreadable_fixtures[0]
        .contains("instance-data-migration.fixtures.json could not be read: "));
}

#[test]
fn instance_data_migration_a_malformed_schema_or_bundle_fails_closed() {
    let bad_schema = run(&real_reader(&paths()).with_file(&schema(SCHEMA_NAME), "{ not json"));
    assert!(bad_schema[0].starts_with("instance-data-migration.schema.json is not valid JSON: "));
    let bad_bundle = run(&real_reader(&paths()).with_file(&fixtures(FIXTURES), "[]"));
    assert_eq!(
        bad_bundle,
        ["the fixtures file must be an object with non-empty \"valid\" and \"invalid\" arrays"]
    );
    let no_valid = with_bundle(|b| b["valid"] = Value::Array(Vec::new()));
    assert_eq!(
        no_valid,
        ["the fixtures file has no non-empty \"valid\" array"]
    );
}

/// A resolver map that is missing, or not an object, resolves nothing: the
/// valid plans that need it are rejected, never silently accepted.
#[test]
fn instance_data_migration_a_missing_resolver_map_fails_the_valid_plans_that_need_it() {
    for key in [
        "source_snapshot_resolution",
        "evidence_resolution",
        "rollback_snapshot_resolution",
        "deterministic_plan_resolution",
        "restoration_evidence_resolution",
    ] {
        let removed = with_bundle(|b| {
            b.as_object_mut().unwrap().remove(key);
        });
        assert!(!removed.is_empty(), "{key}");
        assert!(
            removed
                .iter()
                .all(|m| m.starts_with("a fixture that must be a valid registry was rejected")),
            "{key}: {removed:?}"
        );
        let malformed = with_bundle(|b| b[key] = Value::from("not a map"));
        assert_eq!(removed, malformed, "{key}");
    }
}

/// A resolved record of the wrong kind, or pinned to a stale fingerprint,
/// is rejected with its own distinct diagnostic.
#[test]
fn instance_data_migration_wrong_kind_and_stale_pin_are_distinct_rejections() {
    let wrong_kind = with_bundle(|b| {
        for entry in b["evidence_resolution"]
            .as_object_mut()
            .unwrap()
            .values_mut()
        {
            entry["record_type"] = Value::from("instance-rollback-snapshot");
        }
    });
    assert!(wrong_kind
        .iter()
        .any(|m| m.contains("resolved record_type is \"instance-rollback-snapshot\", not \"instance-migration-evidence\"")));
    let stale = with_bundle(|b| {
        for entry in b["evidence_resolution"]
            .as_object_mut()
            .unwrap()
            .values_mut()
        {
            entry["plan_fingerprint"] = Value::from("0".repeat(64));
        }
    });
    assert!(stale
        .iter()
        .any(|m| m.contains("does not match this plan's own recomputed fingerprint")));
    assert_ne!(wrong_kind, stale);
}

#[test]
fn instance_data_migration_diagnostics_are_deterministic() {
    let broken = |b: &mut Value| {
        b["source_snapshot_resolution"] = Value::Null;
        b["evidence_resolution"] = Value::Null;
    };
    assert_eq!(with_bundle(broken), with_bundle(broken));
}

/// The composition point stops at typed input for a schema-clean plan and
/// returns the plan route's own prefix-free rejection otherwise.
#[test]
fn instance_data_migration_the_composition_point_types_one_resolved_plan() {
    let registry = real_json(&schema(SCHEMA_NAME));
    let envelope = real_json(&schema(ENVELOPE));
    let schemas = schemas(&registry, &envelope);
    let bundle = real_json(&fixtures(FIXTURES));
    let plan = bundle["valid"][1]["registry"]["migration_plans"][0].clone();
    let typed = typed_plan(&plan, "q — composed migration plan".to_string(), &schemas).unwrap();
    assert_eq!(typed.head.id.as_str(), plan["id"].as_str().unwrap());
    let mut broken = plan;
    broken["payload"]["unexpected"] = Value::from(1);
    let rejected = typed_plan(&broken, "q".to_string(), &schemas).unwrap_err();
    assert!(!rejected.is_empty());
    assert!(!rejected[0]
        .message()
        .starts_with("instance-data-migration:"));
    let _ = real(&schema(SCHEMA_NAME));
}

/// Container/envelope schema short-circuit (`COMPATIBILITY.md`): the real
/// valid[1] container with the harness's own domain defect — the plan
/// declares itself as `supersedes`.
#[test]
fn instance_data_migration_a_container_or_envelope_schema_violation_short_circuits_before_the_domain(
) {
    let bundle = real_json(&fixtures(FIXTURES));
    let mut doc = bundle["valid"][1]["registry"].clone();
    let id = doc["migration_plans"][0]["id"].clone();
    doc["migration_plans"][0]["payload"]["supersedes"] = id;
    let registry = real_json(&schema(SCHEMA_NAME));
    let envelope = real_json(&schema(ENVELOPE));
    let schemas = schemas(&registry, &envelope);
    let resolution = plan_resolution(bundle.as_object().unwrap());
    crate::operating_model::migration_boundary::test_support::assert_schema_short_circuit(
        &doc,
        "migration_plans",
        "migration plan \"sample-migration-plan-one\" supersedes its own id; a plan cannot supersede itself",
        |doc| check_container(doc, &schemas, &resolution),
    );
}

/// The Rust half of the matched Node/Rust invalid-JSON content case
/// (`COMPATIBILITY.md`; `test/conformance-harness.test.mjs`, block
/// `rust-architecture-conformance-7` invalid JSON content): the real
/// valid[1] container whose first mapping target's `application/json`
/// content becomes `{not valid json`. Both sides reject it with the SAME
/// five diagnostics in the SAME order (the changed content also changes
/// the fingerprint the plan's own pins name); the ONLY difference is the
/// tail of the invalid-JSON line after `media_type "application/json": ` —
/// Node appends V8's parser message, Rust a neutral one.
#[test]
fn instance_data_migration_invalid_json_content_is_rejected_like_the_reference() {
    let bundle = real_json(&fixtures(FIXTURES));
    let mut doc = bundle["valid"][1]["registry"].clone();
    doc["migration_plans"][0]["payload"]["mappings"][0]["target"]["payload"]["content"] =
        Value::from("{not valid json");
    let registry = real_json(&schema(SCHEMA_NAME));
    let envelope = real_json(&schema(ENVELOPE));
    let schemas = schemas(&registry, &envelope);
    let resolution = plan_resolution(bundle.as_object().unwrap());
    let outcome = check_container(&doc, &schemas, &resolution);
    assert!(matches!(outcome, CaseOutcome::DomainRejected(_)));
    let plan = "migration plan \"sample-migration-plan-one\"";
    let stale = "resolved plan_fingerprint \"3c465ef1b7f0b4a65d2ed1e7497fcc749e1e1be0c018b9db827cd90d8971f9ae\" does not match this plan's own recomputed fingerprint \"02a92cef0a7bb16f79379b189f3ca914ac3343a3b6b6d5e7c29f3ae2cc536b2f\"";
    assert_eq!(
        texts(&crate::operating_model::migration_boundary::outcome_diagnostics(&outcome)),
        [
            format!("{plan} rollback.deterministic_plan_ref \"rollback-plan:sample-revision-0001-to-migration-plan-one\": {stale}; a reconstruction plan pinned to a stale or different version of this plan's content never confirms the current one (property 5)"),
            format!("{plan} target \"sample-migrated-record-one\" payload content is not valid JSON for media_type \"application/json\": the content does not parse as a single JSON value"),
            format!("{plan} evidence \"evidence:coverage-migration-plan-one\": {stale}; evidence pinned to a stale or different version of this plan's content never confirms the current one (property 6)"),
            format!("{plan} evidence \"evidence:applicability-migration-plan-one\": {stale}; evidence pinned to a stale or different version of this plan's content never confirms the current one (property 6)"),
            format!("{plan} plan_fingerprint \"3c465ef1b7f0b4a65d2ed1e7497fcc749e1e1be0c018b9db827cd90d8971f9ae\" does not match the recomputed fingerprint \"02a92cef0a7bb16f79379b189f3ca914ac3343a3b6b6d5e7c29f3ae2cc536b2f\" of its own documented canonical projection (source, record_units, mappings, rollback); the same pinned input must always compute the same fingerprint"),
        ]
    );
}
