#![cfg(test)]
//! The `workspace-compatibility-qualification` production route over the
//! real schemas and fixtures, a fake reader and the real adapter — and the
//! adversarial composition cases: stale pin, wrong kind, plan substitution
//! and scope mismatch.

use serde_json::Value;

use super::*;
use crate::operating_model::migration_boundary::test_support::{
    fixtures, fs_reader, io, real_json, real_reader, schema, texts,
};

const SCHEMA: &str = "workspace-compatibility-qualification.schema.json";
const FIXTURES: &str = "workspace-compatibility-qualification.fixtures.json";

fn paths() -> Vec<String> {
    let mut paths = vec![schema(SCHEMA)];
    paths.extend(FAMILY.composed.iter().map(|c| schema(c.file_name)));
    paths.push(fixtures(FIXTURES));
    paths
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

/// The valid case whose plan is VERIFIED, mints a record and composes a
/// canonical export (decision-matrix branch 9).
fn branch_nine(bundle: &Value) -> usize {
    bundle["valid"]
        .as_array()
        .unwrap()
        .iter()
        .position(|c| {
            c["note"]
                .as_str()
                .unwrap()
                .starts_with("decision-matrix branch 9")
        })
        .unwrap()
}

#[test]
fn workspace_compatibility_qualification_the_real_kernel_contract_is_clean() {
    assert_eq!(run(&fs_reader()), Vec::<String>::new());
    assert_eq!(run(&real_reader(&paths())), Vec::<String>::new());
}

/// All 31 real cases take the production route; `validate` reports
/// nothing, so every valid case was accepted and every invalid one was
/// rejected by the schema or the domain (a drift would be reported).
#[test]
fn workspace_compatibility_qualification_every_real_fixture_takes_the_production_pipeline() {
    let bundle = real_json(&fixtures(FIXTURES));
    assert_eq!(
        (
            bundle["valid"].as_array().unwrap().len(),
            bundle["invalid"].as_array().unwrap().len()
        ),
        (9, 22)
    );
    let flipped = with_bundle(|b| {
        let valid = b["valid"].take();
        b["valid"] = b["invalid"].take();
        b["invalid"] = valid;
    });
    assert_eq!(flipped.len(), 31, "{flipped:?}");
    assert!(
        flipped.iter().all(|m| !m.starts_with("internal:")),
        "{flipped:?}"
    );
}

#[test]
fn workspace_compatibility_qualification_every_composed_schema_is_mandatory() {
    for dependency in FAMILY.composed {
        let path = schema(dependency.file_name);
        let reader_paths: Vec<String> = paths().into_iter().filter(|p| *p != path).collect();
        let missing = run(&real_reader(&reader_paths));
        assert_eq!(
            missing,
            [format!("{path} is missing; {}", dependency.missing)],
            "{path}"
        );
        let unreadable = run(&real_reader(&paths()).with_error(&path, io("denied")));
        assert_eq!(unreadable, [format!("{path} could not be read: denied")]);
    }
}

/// A resolved connection whose content changed under the SAME reference no
/// longer matches its pin: the stale pin is rejected, never composed.
#[test]
fn workspace_compatibility_qualification_a_stale_connection_pin_is_rejected() {
    let messages = with_bundle(|b| {
        for record in b["connection_record_resolution"]
            .as_object_mut()
            .unwrap()
            .values_mut()
        {
            record["title"] = Value::from("изменённое содержимое под той же ссылкой");
        }
    });
    assert!(!messages.is_empty());
    assert!(messages.iter().all(|m| m
        .contains("does not equal the resolved connection's own recomputed content digest")
        || !m.starts_with("a fixture that must be a valid registry was rejected")));
}

#[test]
fn workspace_compatibility_qualification_a_wrong_kind_is_rejected() {
    let messages = with_bundle(|b| {
        for record in b["plan_record_resolution"]
            .as_object_mut()
            .unwrap()
            .values_mut()
        {
            record["record_type"] = Value::from("instance-canonical-export");
        }
    });
    assert!(messages
        .iter()
        .any(|m| m.contains("payload.migration_plan_ref") && m.contains("resolved record_type is \"instance-canonical-export\", not \"instance-migration-plan\"")));
}

/// Plan substitution: a plan resolver configured next to the export
/// boundary, mapping the SAME plan id to different content, cannot reach
/// the composed export check — the export is checked only against the plan
/// the qualification itself pinned. The route stays clean.
#[test]
fn workspace_compatibility_qualification_an_independent_plan_under_the_same_id_cannot_substitute() {
    let clean = with_bundle(|b| {
        let branch = branch_nine(b);
        let export_id = b["valid"][branch]["registry"]["qualifications"][0]["payload"]
            ["migration_plan_ref"]["id"]
            .clone();
        let mut substitute = serde_json::Map::new();
        substitute.insert(
            export_id.as_str().unwrap().to_string(),
            serde_json::json!({
                "record_type": "instance-migration-plan",
                "plan_ref": export_id,
                "plan_fingerprint": "0".repeat(64),
                "scope": { "type": "project-workspace", "id": "substitute" },
                "source": {}, "record_units": [], "mappings": []
            }),
        );
        b["export_resolution"]["plan_resolution"] = Value::Object(substitute);
    });
    assert_eq!(clean, Vec::<String>::new());
}

/// Scope mismatch between the qualification and a resolved, pinned
/// connection is its own rejection.
#[test]
fn workspace_compatibility_qualification_a_scope_mismatch_is_rejected() {
    let bundle = real_json(&fixtures(FIXTURES));
    let note = "scope конверта расходится со scope разрешённого элемента workspace_connection_refs";
    assert!(bundle["invalid"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["note"] == note));
    let flipped = with_bundle(|b| {
        let invalid = b["invalid"].as_array_mut().unwrap();
        let index = invalid.iter().position(|c| c["note"] == note).unwrap();
        let case = invalid.remove(index);
        b["valid"].as_array_mut().unwrap().push(case);
    });
    assert!(
        flipped.iter().any(|m| m.contains(note)
            && m.contains("scope does not match the resolved payload.workspace_connection_refs")),
        "{flipped:?}"
    );
}

#[test]
fn workspace_compatibility_qualification_a_missing_record_map_fails_the_valid_cases() {
    for key in [
        "connection_record_resolution",
        "plan_record_resolution",
        "export_record_resolution",
        "migration_resolution",
        "export_resolution",
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
    let outcome = with_route(&loaded, |route, boundaries| {
        check_container(doc, route, boundaries)
    });
    texts(&crate::operating_model::migration_boundary::outcome_diagnostics(&outcome))
}

/// The Rust half of the matched Node/Rust library case
/// (`test/conformance-harness.test.mjs`, block `rust-architecture-conformance-7`
/// raw decision inputs; `COMPATIBILITY.md`): the real branch-5 fixture
/// (`QUALIFIED`, one clean connection) with ONE mutation — the resolved
/// connection's `scope.id` becomes `sample-project-other`, its
/// `payload.repository.id` becomes `sample-project-repository-unscanned`,
/// and the pin is re-pinned to the new digest (the same value Node's
/// `computeConnectionDigest` returns). The connection's own contract
/// rejects it (`repository.workspace_id` no longer matches `scope.id`).
/// Node still reads the rejected record's raw `scope` (a scope-mismatch
/// line, FIRST), raw `repository.id` (two repository-set lines) and raw
/// `next_step` (`continue-compatibility-mode`: the matrix agrees with
/// `QUALIFIED`, no line). The Rust route reads decision inputs only from
/// accepted records: none of those three lines, and the rejected
/// connection's `next_step` is an explicit unknown, so the matrix computes
/// `UNVERIFIED`. The composition's own diagnostic is common to both and
/// is the Rust route's first line.
#[test]
fn workspace_compatibility_qualification_a_rejected_connection_contributes_no_raw_decision_input() {
    let mut bundle = real_json(&fixtures(FIXTURES));
    let reference = "connection:sample-connection-clean";
    bundle["connection_record_resolution"][reference]["scope"]["id"] =
        Value::from("sample-project-other");
    bundle["connection_record_resolution"][reference]["payload"]["repository"]["id"] =
        Value::from("sample-project-repository-unscanned");
    let digest = crate::operating_model::migration_boundary::resolved_record(
        &bundle["connection_record_resolution"][reference],
    )
    .expect("the mutated connection still resolves")
    .content_digest();
    assert_eq!(
        digest.value(),
        "bc0bb0c68d12be6a79df75c125216e1fa59c51cce77dc5d3fe8f5070cc8bdbdb"
    );
    let index = bundle["valid"]
        .as_array()
        .unwrap()
        .iter()
        .position(|c| {
            c["note"]
                .as_str()
                .unwrap()
                .starts_with("decision-matrix branch 5")
        })
        .unwrap();
    let mut doc = bundle["valid"][index]["registry"].clone();
    doc["qualifications"][0]["payload"]["workspace_connection_refs"][0]["sha256"] =
        Value::from(digest.value());
    assert_eq!(
        full_diagnostics(&bundle, &doc),
        [
            "qualification \"sample-qualification-compatibility-only\" workspace_connection_refs: workspace connection scan \"sample-connection-clean\" payload.repository.workspace_id \"sample-project\" does not match scope.id \"sample-project-other\"; scope names the exact workspace this scan covers",
            "qualification \"sample-qualification-compatibility-only\" qualification_state is \"QUALIFIED\", but the closed decision matrix over the composed records' own next_steps/pending rule-candidate decisions/verification.overall_status/canonical export coverage computes \"UNVERIFIED\"",
        ]
    );
}

/// The Rust halves of the two matched Node/Rust schema-invalid composed
/// record cases (`test/conformance-harness.test.mjs`, block
/// `rust-architecture-conformance-7` schema-invalid composition;
/// `COMPATIBILITY.md`): the real branch-9 fixture (plan + export,
/// `QUALIFIED`) where the resolved plan — respectively the resolved
/// export — carries one field its own schema forbids
/// (`payload.source.unexpected_field`). Node pins the raw JSON: the field
/// changes the raw fingerprint/digest, so Node reports only a `sha256 …
/// does not equal …` mismatch. The Rust route pins only a typed,
/// schema-shaped record: it reports the record's own schema diagnostic
/// under the composed prefix and never a fingerprint/digest.
fn branch_nine_with(map: &str, reference: &str) -> Vec<String> {
    let mut bundle = real_json(&fixtures(FIXTURES));
    bundle[map][reference]["payload"]["source"]["unexpected_field"] = Value::from("schema-only");
    let doc = bundle["valid"][branch_nine(&bundle)]["registry"].clone();
    full_diagnostics(&bundle, &doc)
}

#[test]
fn workspace_compatibility_qualification_a_schema_invalid_composed_plan_is_never_pinned_by_raw_json(
) {
    let messages = branch_nine_with("plan_record_resolution", "plan:sample-migration-plan-one");
    let at = "qualification \"sample-qualification-migrated-and-exported\"";
    assert_eq!(
        messages,
        [
            format!("{at} migration_plan_ref: /migration_plans/0/payload/source/unexpected_field: additional property not allowed"),
            format!("{at} payload.canonical_export_ref is present without a resolvable payload.migration_plan_ref; an export proves a plan's own targets and cannot be composed without one"),
        ]
    );
    assert!(
        !messages.iter().any(|m| m.contains("sha256")),
        "{messages:#?}"
    );
}

#[test]
fn workspace_compatibility_qualification_a_schema_invalid_composed_export_is_never_pinned_by_raw_json(
) {
    let messages = branch_nine_with(
        "export_record_resolution",
        "export:sample-canonical-export-one",
    );
    let at = "qualification \"sample-qualification-migrated-and-exported\"";
    assert_eq!(
        messages,
        [
            format!("{at} canonical_export_ref: /exports/0/payload/source/unexpected_field: additional property not allowed"),
            format!("{at} qualification_state is \"QUALIFIED\", but the closed decision matrix over the composed records' own next_steps/pending rule-candidate decisions/verification.overall_status/canonical export coverage computes \"UNVERIFIED\""),
        ]
    );
    assert!(
        !messages.iter().any(|m| m.contains("sha256")),
        "{messages:#?}"
    );
}

/// Container/envelope schema short-circuit (`COMPATIBILITY.md`): the real
/// branch-5 container with the harness's own domain defect — the
/// envelope's `origin.source_ref` no longer names the pinned connections.
#[test]
fn workspace_compatibility_qualification_a_container_or_envelope_schema_violation_short_circuits_before_the_domain(
) {
    let bundle = real_json(&fixtures(FIXTURES));
    let index = bundle["valid"]
        .as_array()
        .unwrap()
        .iter()
        .position(|c| {
            c["note"]
                .as_str()
                .unwrap()
                .starts_with("decision-matrix branch 5")
        })
        .unwrap();
    let mut doc = bundle["valid"][index]["registry"].clone();
    doc["qualifications"][0]["origin"]["source_ref"] =
        Value::from("workspace-connection-scan:tampered");
    let reader = real_reader(&paths());
    let mut load = Vec::new();
    let loaded = load_container_bundle(&reader, &FAMILY, &mut load).expect("the bundle loads");
    with_route(&loaded, |route, boundaries| {
        crate::operating_model::migration_boundary::test_support::assert_schema_short_circuit(
            &doc,
            "qualifications",
            "qualification \"sample-qualification-compatibility-only\" origin.source_ref \"workspace-connection-scan:tampered\" does not equal \"workspace-connection-scan:sample-connection-clean\"; the envelope and payload must pin the SAME set of composed workspace connections",
            |doc| check_container(doc, route, boundaries),
        );
    });
}
