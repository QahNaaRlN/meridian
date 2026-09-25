//! The `evidence-and-handoff-contract` production route over the real
//! schemas and fixtures, a fake reader and the real adapter.
#![cfg(test)]

use serde_json::Value;

use super::*;
use crate::operating_model::record_resolution;
use crate::operating_model::run_contract_boundary::{fixture_cases, ENVELOPE_PATH};
use crate::validation::mechanical_integrity::tests::{real_fs_reader, FakeReader};
use crate::workspace::ReadError;

const SCHEMA_PATH: &str = "registries/operating-model/evidence-and-handoff.schema.json";
const FIXTURES_PATH: &str =
    "registries/operating-model/fixtures/evidence-and-handoff.fixtures.json";

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

/// `check_record` over the real schemas and the real bundle's catalogue.
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

#[test]
fn evidence_and_handoff_the_real_kernel_contract_is_consistent_through_the_production_route() {
    assert_eq!(
        messages(&real_fs_reader(kernel_root())),
        Vec::<String>::new()
    );
}

#[test]
fn evidence_and_handoff_every_real_fixture_reaches_its_declared_typed_outcome() {
    let bundle = real_json(FIXTURES_PATH);
    with_real(|schemas, catalogue| {
        let valid = fixture_cases(bundle["valid"].as_array().unwrap());
        assert_eq!(valid.len(), 8);
        for case in &valid {
            match check_record(&case.spec, schemas, Some(catalogue)) {
                CaseOutcome::Accepted(handoff) => {
                    assert_eq!(handoff.run_id().as_str(), handoff.scope().run_id().as_str());
                }
                _ => panic!("valid fixture not accepted: {}", case.note),
            }
        }
        let invalid = fixture_cases(bundle["invalid"].as_array().unwrap());
        assert_eq!(invalid.len(), 95);
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
/// (`VALIDATE_MUTATION_FAMILIES_7C_WRITE_evidenceAndHandoff`).
#[test]
fn evidence_and_handoff_a_schema_clean_domain_defect_keeps_the_node_first_problem_text() {
    let mut bundle = real_json(FIXTURES_PATH);
    bundle["valid"][0]["spec"]["payload"]["outcome"]["statement"] = Value::from("/etc/passwd");
    let note = bundle["valid"][0]["note"].as_str().unwrap().to_string();
    let reader = real_reader().with_file(FIXTURES_PATH, &bundle.to_string());
    assert_eq!(
        messages(&reader),
        [format!(
            "a fixture that must be a valid handoff was rejected ({note}): evidence and handoff \"example-handoff-clarify-null-handling-001\" outcome statement contains a rooted (absolute) POSIX path"
        )]
    );
}

/// Rust half of the matched schema short-circuit case
/// (`COMPATIBILITY.md`; the Node half imports `evaluateEvidenceAndHandoff`
/// directly in `test/conformance-harness.test.mjs`): a duplicated
/// `covers` entry — rejected ONLY by the schema's `uniqueItems`, the closed
/// DTO and the domain would each accept it on their own — together with an
/// independent domain defect. The Node library reports both; the route
/// stops at the schema gate and reports only the schema diagnostic.
#[test]
fn evidence_and_handoff_a_schema_violation_short_circuits_before_the_domain() {
    let mut spec = real_json(FIXTURES_PATH)["valid"][0]["spec"].clone();
    let covers = spec["payload"]["evidence"][0]["covers"]
        .as_array_mut()
        .unwrap();
    let first = covers[0].clone();
    covers.push(first);
    spec["payload"]["outcome"]["statement"] = Value::from("/etc/passwd");
    with_real(|schemas, catalogue| {
        let CaseOutcome::SchemaRejected(problems) = check_record(&spec, schemas, Some(catalogue))
        else {
            panic!("a schema violation is a schema rejection");
        };
        let texts: Vec<&str> = problems.iter().map(|d| d.message()).collect();
        assert_eq!(texts.len(), 1, "{texts:#?}");
        assert!(
            texts[0].contains("/payload/evidence/0/covers"),
            "{texts:#?}"
        );
        assert!(!texts.iter().any(|t| t.contains("outcome statement")));
    });
}

#[test]
fn evidence_and_handoff_without_a_catalogue_the_core_fails_closed() {
    let spec = real_json(FIXTURES_PATH)["valid"][0]["spec"].clone();
    with_real(|schemas, _| {
        let CaseOutcome::DomainRejected(problems) = check_record(&spec, schemas, None) else {
            panic!("a handoff without a resolver must be domain-rejected");
        };
        assert!(problems.iter().any(|p| p
            .message()
            .contains("no external record resolver was supplied")));
    });
}

/// A resolver response with two unknown fields is reported in sorted
/// field order — the existing Rust-native boundary (`COMPATIBILITY.md`),
/// through the shared typed catalogue.
#[test]
fn evidence_and_handoff_unknown_resolver_fields_are_reported_in_stable_sorted_order() {
    let mut bundle = real_json(FIXTURES_PATH);
    let resolution = bundle["resolution"].as_object_mut().unwrap();
    let key = resolution
        .iter()
        .find(|(_, v)| v["record_type"] == "execution-run")
        .map(|(k, _)| k.clone())
        .unwrap();
    let entry = resolution[&key].as_object_mut().unwrap();
    entry.insert("zzzz_extra_field".to_string(), Value::from("z"));
    entry.insert("aaaa_extra_field".to_string(), Value::from("a"));
    let reader = real_reader().with_file(FIXTURES_PATH, &bundle.to_string());
    let m = messages(&reader);
    assert!(!m.is_empty());
    for line in m.iter().filter(|l| l.contains("was rejected")) {
        assert!(
            line.contains("unknown field \"aaaa_extra_field\""),
            "{line}"
        );
    }
}

#[test]
fn evidence_and_handoff_missing_and_unreadable_mandatory_files_are_distinct_failures() {
    assert_eq!(
        messages(&FakeReader::new()),
        ["registries/operating-model/evidence-and-handoff.schema.json is missing; the evidence and handoff contract is a mandatory part of this Kernel, not an optional add-on"]
    );
    assert_eq!(
        messages(&real_reader().with_error(SCHEMA_PATH, io("Is a directory"))),
        ["registries/operating-model/evidence-and-handoff.schema.json could not be read: Is a directory"]
    );
    let no_envelope = FakeReader::new()
        .with_file(SCHEMA_PATH, &real(SCHEMA_PATH))
        .with_file(FIXTURES_PATH, &real(FIXTURES_PATH));
    assert_eq!(
        messages(&no_envelope),
        ["registries/operating-model/scoped-record.schema.json is missing; the handoff record composes with the record envelope and cannot be checked without it"]
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
        ["the schema carries no fixtures (registries/operating-model/fixtures/evidence-and-handoff.fixtures.json); a schema no run exercises is not one this gate has reached"]
    );
    assert_eq!(
        messages(&real_reader().with_error(FIXTURES_PATH, io("Is a directory"))),
        ["registries/operating-model/fixtures/evidence-and-handoff.fixtures.json could not be read: Is a directory"]
    );
}

#[test]
fn evidence_and_handoff_malformed_schema_and_bundle_shapes_fail_closed() {
    let m = messages(&real_reader().with_file(SCHEMA_PATH, "{"));
    assert_eq!(m.len(), 1);
    assert!(
        m[0].starts_with("evidence-and-handoff.schema.json is not valid JSON: "),
        "{m:?}"
    );
    let m = messages(&real_reader().with_file(FIXTURES_PATH, "not json"));
    assert!(
        m[0].starts_with("the fixtures file is not valid JSON: "),
        "{m:?}"
    );
    assert_eq!(
        messages(&real_reader().with_file(FIXTURES_PATH, "[]")),
        ["the fixtures file must be an object with non-empty \"valid\" and \"invalid\" arrays"]
    );
    // Only the FIRST missing array is named, and then nothing else — the
    // Node reference's `bundleOk` short-circuit.
    assert_eq!(
        messages(&real_reader().with_file(FIXTURES_PATH, "{\"valid\": [], \"invalid\": []}")),
        ["the fixtures file has no non-empty \"valid\" array"]
    );
    let mut bundle = real_json(FIXTURES_PATH);
    bundle.as_object_mut().unwrap().remove("resolution");
    let m = messages(&real_reader().with_file(FIXTURES_PATH, &bundle.to_string()));
    assert_eq!(m.len(), 1);
    assert!(m[0].starts_with("the fixtures file carries no \"resolution\" object; the pinned execution-run, task-specification, run-human-control and context-manifest records AND every evidence entry"), "{m:?}");
}
