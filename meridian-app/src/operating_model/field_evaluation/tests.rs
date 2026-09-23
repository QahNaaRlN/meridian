//! The `meridian-field-evaluation` production route over the real schemas
//! and fixtures, a fake reader and the real adapter.
#![cfg(test)]

use serde_json::Value;

use meridian_core::field_evaluation::FieldEvaluationRecord;

use super::*;
use crate::operating_model::record_resolution;
use crate::operating_model::run_contract_boundary::{fixture_cases, ENVELOPE_PATH};
use crate::validation::mechanical_integrity::tests::{real_fs_reader, FakeReader};
use crate::workspace::ReadError;

const SCHEMA_PATH: &str = "registries/operating-model/field-evaluation.schema.json";
const FIXTURES_PATH: &str = "registries/operating-model/fixtures/field-evaluation.fixtures.json";

fn kernel_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn real(path: &str) -> String {
    std::fs::read_to_string(kernel_root().join(path)).unwrap()
}

fn real_json(path: &str) -> Value {
    serde_json::from_str(&real(path)).unwrap()
}

fn real_reader() -> FakeReader {
    FakeReader::new()
        .with_file(SCHEMA_PATH, &real(SCHEMA_PATH))
        .with_file(ENVELOPE_PATH, &real(ENVELOPE_PATH))
        .with_file(FIXTURES_PATH, &real(FIXTURES_PATH))
}

fn messages(reader: &dyn WorkspaceReader) -> Vec<String> {
    evaluate(reader)
        .diagnostics
        .iter()
        .map(|d| d.message().to_string())
        .collect()
}

fn io(message: &str) -> ReadError {
    ReadError::Io(message.to_string())
}

fn with_real<T>(f: impl FnOnce(&RecordSchemas<'_>, &ResolutionCatalogue) -> T) -> T {
    let envelope = real_json(ENVELOPE_PATH);
    let record = real_json(SCHEMA_PATH);
    let bundle = real_json(FIXTURES_PATH);
    let schemas = RecordSchemas {
        envelope: &envelope,
        record: &record,
        label: SCHEMA_LABEL,
    };
    let catalogue = record_resolution::catalogue(bundle["resolution"].as_object().unwrap());
    f(&schemas, &catalogue)
}

fn mech_correct(bundle: &mut Value) -> &mut Value {
    bundle["valid"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|c| c["note"] == "наблюдение obs-mech-correct-1")
        .map(|c| &mut c["spec"])
        .unwrap()
}

#[test]
fn field_evaluation_the_real_kernel_contract_is_consistent_through_the_production_route() {
    assert_eq!(
        messages(&real_fs_reader(kernel_root())),
        Vec::<String>::new()
    );
}

#[test]
fn field_evaluation_every_real_fixture_reaches_its_declared_typed_outcome() {
    let bundle = real_json(FIXTURES_PATH);
    with_real(|schemas, catalogue| {
        let valid = fixture_cases(bundle["valid"].as_array().unwrap());
        assert_eq!(valid.len(), 24);
        let (mut observations, mut reports) = (0, 0);
        for case in &valid {
            match check_record(&case.spec, schemas, Some(catalogue)) {
                CaseOutcome::Accepted(FieldEvaluationRecord::Observation(_)) => observations += 1,
                CaseOutcome::Accepted(FieldEvaluationRecord::Report(r)) => {
                    assert_eq!(r.per_metric().len(), 8);
                    reports += 1;
                }
                _ => panic!("valid fixture not accepted: {}", case.note),
            }
        }
        assert!(observations > 0 && reports > 0);
        let invalid = fixture_cases(bundle["invalid"].as_array().unwrap());
        assert_eq!(invalid.len(), 36);
        let (mut schema, mut domain) = (0, 0);
        for case in &invalid {
            match check_record(&case.spec, schemas, Some(catalogue)) {
                CaseOutcome::SchemaRejected(_) => schema += 1,
                CaseOutcome::DomainRejected(_) => domain += 1,
                _ => panic!(
                    "invalid fixture not rejected by schema or domain: {}",
                    case.note
                ),
            }
        }
        assert!(schema > 0 && domain > 0, "schema={schema} domain={domain}");
    });
}

/// The same mutation the conformance harness applies
/// (`VALIDATE_MUTATION_FAMILIES_7C_WRITE_fieldEvaluation`).
#[test]
fn field_evaluation_a_schema_clean_domain_defect_keeps_the_node_first_problem_text() {
    let mut bundle = real_json(FIXTURES_PATH);
    mech_correct(&mut bundle)["payload"]["measurement"]["basis"] = Value::from("/etc/passwd");
    let reader = real_reader().with_file(FIXTURES_PATH, &bundle.to_string());
    assert_eq!(
        messages(&reader),
        ["a fixture that must be valid was rejected (наблюдение obs-mech-correct-1): field evaluation observation \"obs-mech-correct-1\" measurement.basis contains a rooted (absolute) POSIX path"]
    );
}

/// The conformance harness's sub-second regression: an interval that
/// differs only in its fraction of a second is a real, positive interval.
#[test]
fn field_evaluation_a_sub_second_interval_is_accepted() {
    let mut bundle = real_json(FIXTURES_PATH);
    let template = bundle["invalid"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["note"] == "невозможный временной интервал (конец раньше начала)")
        .unwrap()["spec"]
        .clone();
    let mut spec = template;
    spec["id"] = Value::from("obs-context-time-fraction-check");
    spec["title"] = Value::from("Наблюдение: context-entry-time (доля секунды)");
    spec["payload"]["measurement"]["start_ts"] = Value::from("2026-08-03T12:00:00.100Z");
    spec["payload"]["measurement"]["end_ts"] = Value::from("2026-08-03T12:00:00.200Z");
    bundle["valid"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({ "note": "sub-second", "spec": spec }));
    let reader = real_reader().with_file(FIXTURES_PATH, &bundle.to_string());
    assert_eq!(messages(&reader), Vec::<String>::new());
}

/// Rust half of the matched schema short-circuit case (`COMPATIBILITY.md`;
/// the Node half imports `evaluateFieldEvaluation` directly in
/// `test/conformance-harness.test.mjs`): `observed_at` breaks only the
/// schema's `date_str` pattern — the closed DTO accepts any string — with
/// an independent domain defect beside it. The Node library reports the
/// pattern, the date and the basis; the route reports only the schema.
#[test]
fn field_evaluation_a_schema_violation_short_circuits_before_the_domain() {
    let mut bundle = real_json(FIXTURES_PATH);
    let spec = mech_correct(&mut bundle);
    spec["payload"]["observed_at"] = Value::from("2026-8-3");
    spec["payload"]["measurement"]["basis"] = Value::from("/etc/passwd");
    let spec = spec.clone();
    with_real(|schemas, catalogue| {
        let CaseOutcome::SchemaRejected(problems) = check_record(&spec, schemas, Some(catalogue))
        else {
            panic!("a schema violation is a schema rejection");
        };
        let texts: Vec<&str> = problems.iter().map(|d| d.message()).collect();
        assert!(texts.iter().all(|t| t.starts_with('/')), "{texts:#?}");
        assert!(
            texts.iter().any(|t| t.contains("/payload/observed_at")),
            "{texts:#?}"
        );
        assert!(!texts.iter().any(|t| t.contains("measurement.basis")));
    });
}

#[test]
fn field_evaluation_without_a_catalogue_the_core_fails_closed() {
    let mut bundle = real_json(FIXTURES_PATH);
    let spec = mech_correct(&mut bundle).clone();
    with_real(|schemas, _| {
        let CaseOutcome::DomainRejected(problems) = check_record(&spec, schemas, None) else {
            panic!("an observation without a resolver must be domain-rejected");
        };
        assert!(problems.iter().any(|p| p
            .message()
            .contains("no external record resolver was supplied")));
    });
}

#[test]
fn field_evaluation_missing_and_unreadable_mandatory_files_are_distinct_failures() {
    assert_eq!(
        messages(&FakeReader::new()),
        ["registries/operating-model/field-evaluation.schema.json is missing; the field-evaluation contract is a mandatory part of this Kernel, not an optional add-on"]
    );
    assert_eq!(
        messages(&real_reader().with_error(SCHEMA_PATH, io("Is a directory"))),
        ["registries/operating-model/field-evaluation.schema.json could not be read: Is a directory"]
    );
    let no_envelope = FakeReader::new()
        .with_file(SCHEMA_PATH, &real(SCHEMA_PATH))
        .with_file(FIXTURES_PATH, &real(FIXTURES_PATH));
    assert_eq!(
        messages(&no_envelope),
        ["registries/operating-model/scoped-record.schema.json is missing; the observation and report records compose with the record envelope and cannot be checked without it"]
    );
    assert_eq!(
        messages(&real_reader().with_error(ENVELOPE_PATH, io("permission denied"))),
        ["registries/operating-model/scoped-record.schema.json could not be read: permission denied"]
    );
    let no_fixtures = FakeReader::new()
        .with_file(SCHEMA_PATH, &real(SCHEMA_PATH))
        .with_file(ENVELOPE_PATH, &real(ENVELOPE_PATH));
    assert_eq!(
        messages(&no_fixtures),
        ["the schema carries no fixtures (registries/operating-model/fixtures/field-evaluation.fixtures.json); a schema no run exercises is not one this gate has reached"]
    );
    assert_eq!(
        messages(&real_reader().with_error(FIXTURES_PATH, io("Is a directory"))),
        ["registries/operating-model/fixtures/field-evaluation.fixtures.json could not be read: Is a directory"]
    );
}

#[test]
fn field_evaluation_malformed_bundle_shapes_fail_closed() {
    assert_eq!(
        messages(&real_reader().with_file(FIXTURES_PATH, "{\"valid\": [1]}")),
        ["the fixtures file has no non-empty \"invalid\" array"]
    );
    let mut bundle = real_json(FIXTURES_PATH);
    bundle["resolution"] = Value::from("not an object");
    let m = messages(&real_reader().with_file(FIXTURES_PATH, &bundle.to_string()));
    assert_eq!(m.len(), 1);
    assert!(m[0].starts_with("the fixtures file carries no \"resolution\" object; the pinned execution-run / observation records AND every evidence entry"), "{m:?}");
}
