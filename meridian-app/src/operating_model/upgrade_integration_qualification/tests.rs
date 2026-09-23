#![cfg(test)]
//! The `upgrade-integration-qualification` production route over the real
//! schemas, fixtures and task-pattern catalogue — and the adversarial
//! composition cases: stale pin, wrong kind and scenario coverage.

use serde_json::Value;

use super::*;
use crate::operating_model::migration_boundary::test_support::{
    fixtures, fs_reader, io, real_json, real_reader, real_task_pattern_catalog, schema, texts,
};

const SCHEMA: &str = "upgrade-integration-qualification.schema.json";
const FIXTURES: &str = "upgrade-integration-qualification.fixtures.json";

fn paths() -> Vec<String> {
    let mut paths = vec![schema(SCHEMA)];
    paths.extend(FAMILY.composed.iter().map(|c| schema(c.file_name)));
    paths.push(fixtures(FIXTURES));
    paths
}

fn run(reader: &dyn WorkspaceReader) -> Vec<String> {
    texts(&evaluate(reader, &real_task_pattern_catalog()).diagnostics)
}

fn with_bundle(mutate: impl FnOnce(&mut Value)) -> Vec<String> {
    let mut bundle = real_json(&fixtures(FIXTURES));
    mutate(&mut bundle);
    let reader = real_reader(&paths()).with_file(&fixtures(FIXTURES), &bundle.to_string());
    run(&reader)
}

#[test]
fn upgrade_integration_qualification_the_real_kernel_contract_is_clean() {
    assert_eq!(run(&fs_reader()), Vec::<String>::new());
    assert_eq!(run(&real_reader(&paths())), Vec::<String>::new());
}

#[test]
fn upgrade_integration_qualification_every_real_fixture_takes_the_production_pipeline() {
    let bundle = real_json(&fixtures(FIXTURES));
    assert_eq!(
        (
            bundle["valid"].as_array().unwrap().len(),
            bundle["invalid"].as_array().unwrap().len()
        ),
        (4, 14)
    );
    let flipped = with_bundle(|b| {
        let valid = b["valid"].take();
        b["valid"] = b["invalid"].take();
        b["invalid"] = valid;
    });
    assert_eq!(flipped.len(), 18, "{flipped:?}");
    assert!(
        flipped.iter().all(|m| !m.starts_with("internal:")),
        "{flipped:?}"
    );
}

#[test]
fn upgrade_integration_qualification_every_composed_schema_is_mandatory() {
    for dependency in FAMILY.composed {
        let path = schema(dependency.file_name);
        let reader_paths: Vec<String> = paths().into_iter().filter(|p| *p != path).collect();
        assert_eq!(
            run(&real_reader(&reader_paths)),
            [format!("{path} is missing; {}", dependency.missing)]
        );
        assert_eq!(
            run(&real_reader(&paths()).with_error(&path, io("denied"))),
            [format!("{path} could not be read: denied")]
        );
    }
}

#[test]
fn upgrade_integration_qualification_a_stale_task_journey_pin_is_rejected() {
    let messages = with_bundle(|b| {
        for record in b["task_journey_resolution"]
            .as_object_mut()
            .unwrap()
            .values_mut()
        {
            record["title"] = Value::from("изменённое содержимое под той же ссылкой");
        }
    });
    assert!(!messages.is_empty());
    assert!(messages
        .iter()
        .filter(|m| m.starts_with("a fixture that must be a valid registry was rejected"))
        .all(|m| m.contains("does not equal the resolved record's own recomputed content digest")));
}

#[test]
fn upgrade_integration_qualification_a_wrong_kind_is_rejected() {
    let messages = with_bundle(|b| {
        for record in b["execution_state_resolution"]
            .as_object_mut()
            .unwrap()
            .values_mut()
        {
            record["record_type"] = Value::from("task-specification");
        }
    });
    assert!(messages.iter().any(
        |m| m.contains("resolved record_type is \"task-specification\", not \"execution-run\"")
    ));
}

/// An incomplete scenario set is rejected even when every declared
/// scenario composes cleanly.
#[test]
fn upgrade_integration_qualification_an_incomplete_scenario_set_is_rejected() {
    let messages = with_bundle(|b| {
        let valid = b["valid"][0]["registry"]["qualifications"][0]["payload"]
            ["scenario_classifications"]
            .as_array_mut()
            .unwrap();
        valid.pop();
    });
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert!(
        messages[0].contains("scenario_classifications"),
        "{messages:?}"
    );
}

#[test]
fn upgrade_integration_qualification_a_missing_nested_resolution_fails_the_valid_cases() {
    for key in [
        "task_journey_nested_resolution",
        "task_journey_resolution",
        "task_specification_resolution",
        "execution_state_resolution",
    ] {
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

/// Every diagnostic of ONE container (not only the first one `validate`
/// reports) through the real production composition over `bundle`.
fn full_diagnostics(bundle: &Value, doc: &Value) -> Vec<String> {
    let reader = real_reader(&paths()).with_file(&fixtures(FIXTURES), &bundle.to_string());
    let mut load = Vec::new();
    let loaded = load_container_bundle(&reader, &FAMILY, &mut load).expect("the bundle loads");
    assert!(load.is_empty(), "{load:?}");
    let catalog = real_task_pattern_catalog();
    let outcome = with_route(&loaded, &catalog, |route| check_container(doc, route));
    texts(&outcome_diagnostics(&outcome))
}

/// The Rust half of the matched Node/Rust library case
/// (`test/conformance-harness.test.mjs`, block `rust-architecture-conformance-7`
/// raw decision inputs; `COMPATIBILITY.md`): the real `QUALIFIED` valid[0]
/// fixture with ONE mutation — the resolved field-evaluation report's
/// `payload.workspace_id` becomes `ws-other`, re-pinned to the new content
/// digest (the value Node's `computeContentDigest` returns). The report's
/// own contract rejects it (18 included observations belong to another
/// workspace). Node additionally reads the rejected report's raw
/// `workspace_id` and reports a workspace-identity line; the Rust route
/// checks workspace identity only on an accepted report. Everything else —
/// the first diagnostic, the 18 composed lines, the recomputed `BLOCKED`
/// and the blockers relation — is common to both, in the same order.
#[test]
fn upgrade_integration_qualification_a_rejected_report_contributes_no_raw_workspace() {
    let mut bundle = real_json(&fixtures(FIXTURES));
    let reference = "field-evaluation-report:report-2026-08";
    bundle["field_evaluation_resolution"][reference]["payload"]["workspace_id"] =
        Value::from("ws-other");
    let digest = resolved_record(&bundle["field_evaluation_resolution"][reference])
        .expect("the mutated report still resolves")
        .content_digest();
    assert_eq!(
        digest.value(),
        "055a75ace5235a222d11fc36275b9afbb0e3bea21555e57d7b4998aa8798ff8d"
    );
    let mut doc = bundle["valid"][0]["registry"].clone();
    doc["qualifications"][0]["payload"]["field_evaluation_report_ref"]["sha256"] =
        Value::from(digest.value());
    let at = "qualification \"uiq-qualified-example\"";
    let mut expected: Vec<String> = (0..18)
        .map(|i| format!(
            "{at} payload.field_evaluation_report_ref: field evaluation report \"report-2026-08\" included_observations[{i}] belongs to workspace \"ws-meridian\", not this report's workspace \"ws-other\"; samples from different workspaces are not mixed without an explicit comparability rule"
        ))
        .collect();
    expected.push(format!("{at} payload.qualification_state is declared \"QUALIFIED\" but recomputes to \"BLOCKED\"; a qualification_state is recomputed, never trusted on its own"));
    expected.push(format!(
        "{at} qualification_state recomputes to BLOCKED but payload.blockers is empty"
    ));
    let actual = full_diagnostics(&bundle, &doc);
    assert!(
        !actual
            .iter()
            .any(|m| m.contains("resolves to payload.workspace_id")),
        "{actual:#?}"
    );
    assert_eq!(actual, expected);
}

/// Container/envelope schema short-circuit (`COMPATIBILITY.md`): the real
/// `QUALIFIED` valid[0] container with the harness's own domain defect — a
/// blocker declared over a `QUALIFIED` record.
#[test]
fn upgrade_integration_qualification_a_container_or_envelope_schema_violation_short_circuits_before_the_domain(
) {
    let bundle = real_json(&fixtures(FIXTURES));
    let mut doc = bundle["valid"][0]["registry"].clone();
    doc["qualifications"][0]["payload"]["blockers"] =
        serde_json::json!(["a blocker declared over a QUALIFIED record"]);
    let reader = real_reader(&paths());
    let mut load = Vec::new();
    let loaded = load_container_bundle(&reader, &FAMILY, &mut load).expect("the bundle loads");
    let catalog = real_task_pattern_catalog();
    with_route(&loaded, &catalog, |route| {
        crate::operating_model::migration_boundary::test_support::assert_schema_short_circuit(
            &doc,
            "qualifications",
            "qualification \"uiq-qualified-example\" qualification_state recomputes to \"QUALIFIED\" but payload.blockers is non-empty; blockers is closed to BLOCKED",
            |doc| check_container(doc, route),
        );
    });
}
