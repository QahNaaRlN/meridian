//! `execution-state-model` — the app operation of the
//! `rust-architecture-conformance-5` route for one execution run's state:
//!
//! ```text
//! WorkspaceReader
//!   -> execution-state.schema.json + scoped-record.schema.json + fixture bundle
//!   -> schema gate -> private closed DTO (dto.rs)
//!   -> meridian_core::run_contracts::ExecutionRunInput
//!   -> meridian_core::run_contracts::check_execution_run
//!   -> Option<ExecutionRun> + Vec<Diagnostic> (prefix-free)
//! ```
//!
//! Run-state DATA is Instance; the Kernel ships the schema and the
//! product-neutral fixtures, and this operation checks that every valid
//! fixture is ACCEPTED by the production route and every invalid one is
//! rejected by the schema or the domain — never by conversion drift.
//!
//! Every file is mandatory. `ReadError::NotFound` keeps the Node
//! reference's "is missing"/"carries no fixtures" text; any other
//! `ReadError::Io` (a directory in place of the file, a permission error)
//! is reported as "… could not be read: …" instead of being folded into
//! absence (`COMPATIBILITY.md`, run-contract mandatory-read boundary).

mod dto;

pub(crate) use dto::BlockerDto;

use meridian_core::run_contracts::{check_execution_run, ExecutionRun};
use meridian_core::types::Diagnostic;
use serde_json::Value;

use super::run_contract_boundary::{
    assert_supported, evaluate_record, fail, fixture_cases, non_empty_array, parse_json,
    report_invalid_case, report_valid_case, unreadable, workspace_path, CaseOutcome, CasePhrases,
    RecordSchemas,
};
use crate::workspace::{ReadError, WorkspaceReader};

const SCHEMA_NAME: &str = "execution-state.schema.json";
const SCHEMA_PATH: &str = "registries/operating-model/execution-state.schema.json";
const ENVELOPE_PATH: &str = "registries/operating-model/scoped-record.schema.json";
const FIXTURES_PATH: &str = "registries/operating-model/fixtures/execution-state.fixtures.json";

const PHRASES: CasePhrases = CasePhrases {
    valid_rejected: "a fixture that must be a valid run record was rejected",
    invalid_clean: "a fixture that must be rejected validated clean",
    record: "run record",
};

/// The operation's result: prefix-free diagnostics, empty when the Kernel's
/// execution-state contract is consistent.
pub struct Outcome {
    pub diagnostics: Vec<Diagnostic>,
}

fn evaluate_case(doc: &Value, schemas: &RecordSchemas<'_>) -> CaseOutcome<ExecutionRun> {
    evaluate_record(
        doc,
        schemas,
        dto::ExecutionRunDto::into_input,
        check_execution_run,
    )
}

/// The whole `execution-state-model` production route.
pub fn evaluate(reader: &dyn WorkspaceReader) -> Outcome {
    let mut diagnostics = Vec::new();
    let schema_raw = match reader.read_text(&workspace_path(SCHEMA_PATH)) {
        Ok(text) => text,
        Err(ReadError::NotFound) => {
            diagnostics.push(fail(format!(
                "{SCHEMA_PATH} is missing; the execution state model contract is a mandatory part of this Kernel, not an optional add-on"
            )));
            return Outcome { diagnostics };
        }
        Err(error) => {
            diagnostics.push(fail(unreadable(SCHEMA_PATH, &error)));
            return Outcome { diagnostics };
        }
    };

    let mut ok = true;
    let schema = parse_json(&schema_raw, SCHEMA_NAME, &mut diagnostics);
    ok &= schema.is_some();

    let envelope = match reader.read_text(&workspace_path(ENVELOPE_PATH)) {
        Ok(raw) => parse_json(&raw, "scoped-record.schema.json", &mut diagnostics),
        Err(ReadError::NotFound) => {
            diagnostics.push(fail(format!(
                "{ENVELOPE_PATH} is missing; the run record composes with the record envelope and cannot be checked without it"
            )));
            None
        }
        Err(error) => {
            diagnostics.push(fail(unreadable(ENVELOPE_PATH, &error)));
            None
        }
    };
    ok &= envelope.is_some();

    if let Some(schema) = &schema {
        ok &= assert_supported(schema, SCHEMA_NAME, &mut diagnostics);
    }

    let fixtures_raw = match reader.read_text(&workspace_path(FIXTURES_PATH)) {
        Ok(text) => text,
        Err(ReadError::NotFound) => {
            diagnostics.push(fail(format!(
                "the schema carries no fixtures ({FIXTURES_PATH}); a schema no run exercises is not one this gate has reached"
            )));
            return Outcome { diagnostics };
        }
        Err(error) => {
            diagnostics.push(fail(unreadable(FIXTURES_PATH, &error)));
            return Outcome { diagnostics };
        }
    };
    let (true, Some(schema), Some(envelope)) = (ok, schema, envelope) else {
        return Outcome { diagnostics };
    };

    let Some(bundle) = parse_json(&fixtures_raw, "the fixtures file", &mut diagnostics) else {
        return Outcome { diagnostics };
    };
    if !bundle.is_object() {
        diagnostics.push(fail(
            "the fixtures file must be an object with non-empty \"valid\" and \"invalid\" arrays",
        ));
        return Outcome { diagnostics };
    }
    let mut groups = Vec::new();
    for key in ["valid", "invalid"] {
        let Some(cases) = non_empty_array(&bundle, key) else {
            diagnostics.push(fail(format!(
                "the fixtures file has no non-empty \"{key}\" array"
            )));
            return Outcome { diagnostics };
        };
        groups.push(fixture_cases(cases));
    }

    let schemas = RecordSchemas {
        envelope: &envelope,
        record: &schema,
        label: "execution-state",
    };
    let mut groups = groups.into_iter();
    for case in groups.next().unwrap_or_default() {
        let outcome = evaluate_case(&case.spec, &schemas);
        report_valid_case(outcome, &case.note, &PHRASES, &mut diagnostics);
    }
    for case in groups.next().unwrap_or_default() {
        let outcome = evaluate_case(&case.spec, &schemas);
        report_invalid_case(outcome, &case.note, &PHRASES, &mut diagnostics);
    }
    Outcome { diagnostics }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation::mechanical_integrity::tests::{real_fs_reader, FakeReader};

    fn kernel_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }

    fn real(path: &str) -> String {
        std::fs::read_to_string(kernel_root().join(path)).unwrap()
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

    fn with_schemas<R>(f: impl FnOnce(&RecordSchemas<'_>) -> R) -> R {
        let envelope: Value = serde_json::from_str(&real(ENVELOPE_PATH)).unwrap();
        let record: Value = serde_json::from_str(&real(SCHEMA_PATH)).unwrap();
        f(&RecordSchemas {
            envelope: &envelope,
            record: &record,
            label: "execution-state",
        })
    }

    fn bundle() -> Value {
        serde_json::from_str(&real(FIXTURES_PATH)).unwrap()
    }

    #[test]
    fn the_real_kernel_contract_is_consistent_through_the_production_route() {
        assert_eq!(
            messages(&real_fs_reader(kernel_root())),
            Vec::<String>::new()
        );
    }

    /// Every real fixture takes the production route: valid ones are
    /// ACCEPTED as a typed run, invalid ones are rejected by the schema or
    /// the domain — none drifts.
    #[test]
    fn every_real_fixture_reaches_its_declared_typed_outcome() {
        let bundle = bundle();
        with_schemas(|schemas| {
            for case in fixture_cases(bundle["valid"].as_array().unwrap()) {
                match evaluate_case(&case.spec, schemas) {
                    CaseOutcome::Accepted(run) => {
                        assert_eq!(run.scope().run_id().as_str(), case.spec["scope"]["id"]);
                        assert!(!run.state().transition_history.is_empty());
                    }
                    _ => panic!("valid fixture not accepted: {}", case.note),
                }
            }
            let (mut schema, mut domain) = (0, 0);
            for case in fixture_cases(bundle["invalid"].as_array().unwrap()) {
                match evaluate_case(&case.spec, schemas) {
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

    #[test]
    fn a_schema_clean_domain_defect_keeps_the_node_first_problem_text() {
        let mut bundle = bundle();
        bundle["valid"][0]["spec"]["payload"]["current_actor"] = Value::from("/etc/passwd");
        let note = bundle["valid"][0]["note"].as_str().unwrap().to_string();
        let id = bundle["valid"][0]["spec"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let reader = real_reader().with_file(FIXTURES_PATH, &bundle.to_string());
        assert_eq!(
            messages(&reader),
            [format!(
                "a fixture that must be a valid run record was rejected ({note}): execution run \"{id}\" current_actor contains a rooted (absolute) POSIX path; the actor is a portable opaque reference and grants no role or authority"
            )]
        );
    }

    #[test]
    fn schema_clean_conversion_drift_is_a_failure_even_for_an_invalid_fixture() {
        let permissive = serde_json::json!({"type": "object"});
        let mut doc = bundle()["valid"][0]["spec"].clone();
        doc["payload"]["unexpected_field"] = Value::Bool(true);
        let schemas = RecordSchemas {
            envelope: &permissive,
            record: &permissive,
            label: "execution-state",
        };
        let outcome = evaluate_case(&doc, &schemas);
        assert!(matches!(outcome, CaseOutcome::ConversionDrift(_)));
        let mut diagnostics = Vec::new();
        report_invalid_case(outcome, "drifted", &PHRASES, &mut diagnostics);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0]
            .message()
            .starts_with("internal: an invalid run record fixture passed the schema"));
    }

    #[test]
    fn a_missing_schema_keeps_the_node_text_and_an_unreadable_one_is_distinct() {
        let missing = messages(&FakeReader::new());
        assert_eq!(
            missing,
            ["registries/operating-model/execution-state.schema.json is missing; the execution state model contract is a mandatory part of this Kernel, not an optional add-on"]
        );
        let unreadable = messages(&real_reader().with_error(SCHEMA_PATH, io("Is a directory")));
        assert_eq!(
            unreadable,
            ["registries/operating-model/execution-state.schema.json could not be read: Is a directory"]
        );
    }

    #[test]
    fn a_missing_or_unreadable_envelope_fails_closed_before_any_fixture() {
        let reader = FakeReader::new()
            .with_file(SCHEMA_PATH, &real(SCHEMA_PATH))
            .with_file(FIXTURES_PATH, &real(FIXTURES_PATH));
        assert_eq!(
            messages(&reader),
            ["registries/operating-model/scoped-record.schema.json is missing; the run record composes with the record envelope and cannot be checked without it"]
        );
        let reader = real_reader().with_error(ENVELOPE_PATH, io("permission denied"));
        assert_eq!(
            messages(&reader),
            ["registries/operating-model/scoped-record.schema.json could not be read: permission denied"]
        );
    }

    #[test]
    fn missing_and_unreadable_fixtures_are_distinct_failures() {
        let reader = FakeReader::new()
            .with_file(SCHEMA_PATH, &real(SCHEMA_PATH))
            .with_file(ENVELOPE_PATH, &real(ENVELOPE_PATH));
        assert_eq!(
            messages(&reader),
            ["the schema carries no fixtures (registries/operating-model/fixtures/execution-state.fixtures.json); a schema no run exercises is not one this gate has reached"]
        );
        let reader = real_reader().with_error(FIXTURES_PATH, io("Is a directory"));
        assert_eq!(
            messages(&reader),
            ["registries/operating-model/fixtures/execution-state.fixtures.json could not be read: Is a directory"]
        );
    }

    #[test]
    fn malformed_files_and_bundle_shapes_report_the_first_defect_only() {
        let m = messages(&real_reader().with_file(SCHEMA_PATH, "{ nope"));
        assert_eq!(m.len(), 1);
        assert!(
            m[0].starts_with("execution-state.schema.json is not valid JSON: "),
            "{m:?}"
        );
        let m = messages(&real_reader().with_file(FIXTURES_PATH, "[]"));
        assert_eq!(m, ["the fixtures file must be an object with non-empty \"valid\" and \"invalid\" arrays"]);
        let m = messages(&real_reader().with_file(FIXTURES_PATH, "{}"));
        assert_eq!(m, ["the fixtures file has no non-empty \"valid\" array"]);
        let m = messages(&real_reader().with_file(FIXTURES_PATH, "not json"));
        assert_eq!(m.len(), 1);
        assert!(
            m[0].starts_with("the fixtures file is not valid JSON: "),
            "{m:?}"
        );
    }

    #[test]
    fn a_valid_fixture_rejected_by_the_schema_reports_the_first_schema_problem() {
        let mut bundle = bundle();
        bundle["valid"][0]["spec"]["payload"]["status"] = Value::from("active");
        let reader = real_reader().with_file(FIXTURES_PATH, &bundle.to_string());
        let m = messages(&reader);
        assert_eq!(m.len(), 1, "{m:?}");
        assert!(
            m[0].starts_with("a fixture that must be a valid run record was rejected ("),
            "{m:?}"
        );
    }
}
