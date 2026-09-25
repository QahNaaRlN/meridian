//! `bounded-context-manifest` — the app operation of the
//! `rust-architecture-conformance-5` route for one run's bounded context
//! manifest:
//!
//! ```text
//! WorkspaceReader
//!   -> context-manifest.schema.json + scoped-record.schema.json + fixture bundle
//!   -> bundle `resolution` map -> typed ResolutionCatalogue (super::record_resolution)
//!   -> schema gate -> private closed DTO (dto.rs)
//!   -> meridian_core::run_contracts::ContextManifestInput
//!   -> meridian_core::run_contracts::check_context_manifest(input, Some(&catalogue))
//!   -> Option<ContextManifest> + Vec<Diagnostic> (prefix-free)
//! ```
//!
//! Every file is mandatory; `ReadError::NotFound` keeps the Node
//! reference's text and any other `ReadError::Io` is reported as
//! "… could not be read: …" (`COMPATIBILITY.md`).

mod dto;

use meridian_core::run_contracts::{check_context_manifest, ContextManifest, ResolutionCatalogue};
use meridian_core::types::Diagnostic;
use serde_json::Value;

use super::record_resolution as resolution;
use super::run_contract_boundary::{
    assert_supported, evaluate_record, fail, fixture_cases, non_empty_array, parse_json,
    report_invalid_case, report_valid_case, unreadable, workspace_path, CaseOutcome, CasePhrases,
    RecordSchemas,
};
use crate::workspace::{ReadError, WorkspaceReader};

const SCHEMA_NAME: &str = "context-manifest.schema.json";
const SCHEMA_PATH: &str = "registries/operating-model/context-manifest.schema.json";
const ENVELOPE_PATH: &str = "registries/operating-model/scoped-record.schema.json";
const FIXTURES_PATH: &str = "registries/operating-model/fixtures/context-manifest.fixtures.json";

const PHRASES: CasePhrases = CasePhrases {
    valid_rejected: "a fixture that must be a valid context manifest was rejected",
    invalid_clean: "a fixture that must be rejected validated clean",
    record: "context manifest",
};

/// The operation's result: prefix-free diagnostics.
pub struct Outcome {
    pub diagnostics: Vec<Diagnostic>,
}

fn evaluate_case(
    doc: &Value,
    schemas: &RecordSchemas<'_>,
    catalogue: &ResolutionCatalogue,
) -> CaseOutcome<ContextManifest> {
    evaluate_record(doc, schemas, dto::ContextManifestDto::into_input, |input| {
        check_context_manifest(input, Some(catalogue))
    })
}

/// The whole `bounded-context-manifest` production route.
pub fn evaluate(reader: &dyn WorkspaceReader) -> Outcome {
    let mut diagnostics = Vec::new();
    let schema_raw = match reader.read_text(&workspace_path(SCHEMA_PATH)) {
        Ok(text) => text,
        Err(ReadError::NotFound) => {
            diagnostics.push(fail(format!(
                "{SCHEMA_PATH} is missing; the bounded context manifest contract is a mandatory part of this Kernel, not an optional add-on"
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
                "{ENVELOPE_PATH} is missing; the manifest record composes with the record envelope and cannot be checked without it"
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
    let (Some(valid), Some(invalid)) = (
        non_empty_array(&bundle, "valid"),
        non_empty_array(&bundle, "invalid"),
    ) else {
        let key = if non_empty_array(&bundle, "valid").is_none() {
            "valid"
        } else {
            "invalid"
        };
        diagnostics.push(fail(format!(
            "the fixtures file has no non-empty \"{key}\" array"
        )));
        return Outcome { diagnostics };
    };
    let Some(resolution) = bundle.get("resolution").and_then(Value::as_object) else {
        diagnostics.push(fail(
            "the fixtures file carries no \"resolution\" object; the pinned execution-run, task-specification and run-human-control records are resolved OUTSIDE the manifest, and a bundle that resolves nothing cannot exercise the checkpoint against its actual run",
        ));
        return Outcome { diagnostics };
    };
    let catalogue = resolution::catalogue(resolution);

    let schemas = RecordSchemas {
        envelope: &envelope,
        record: &schema,
        label: "context-manifest",
    };
    for case in fixture_cases(valid) {
        let outcome = evaluate_case(&case.spec, &schemas, &catalogue);
        report_valid_case(outcome, &case.note, &PHRASES, &mut diagnostics);
    }
    for case in fixture_cases(invalid) {
        let outcome = evaluate_case(&case.spec, &schemas, &catalogue);
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

    fn bundle() -> Value {
        serde_json::from_str(&real(FIXTURES_PATH)).unwrap()
    }

    fn io(message: &str) -> ReadError {
        ReadError::Io(message.to_string())
    }

    #[test]
    fn the_real_kernel_contract_is_consistent_through_the_production_route() {
        assert_eq!(
            messages(&real_fs_reader(kernel_root())),
            Vec::<String>::new()
        );
    }

    #[test]
    fn every_real_fixture_reaches_its_declared_typed_outcome() {
        let bundle = bundle();
        let envelope: Value = serde_json::from_str(&real(ENVELOPE_PATH)).unwrap();
        let record: Value = serde_json::from_str(&real(SCHEMA_PATH)).unwrap();
        let schemas = RecordSchemas {
            envelope: &envelope,
            record: &record,
            label: "context-manifest",
        };
        let catalogue = resolution::catalogue(bundle["resolution"].as_object().unwrap());
        for case in fixture_cases(bundle["valid"].as_array().unwrap()) {
            match evaluate_case(&case.spec, &schemas, &catalogue) {
                CaseOutcome::Accepted(manifest) => {
                    assert_eq!(
                        manifest.checkpoint().pins.execution_run.id.as_str(),
                        manifest.scope().run_id().as_str()
                    );
                }
                _ => panic!("valid fixture not accepted: {}", case.note),
            }
        }
        let (mut schema, mut domain) = (0, 0);
        for case in fixture_cases(bundle["invalid"].as_array().unwrap()) {
            match evaluate_case(&case.spec, &schemas, &catalogue) {
                CaseOutcome::SchemaRejected(_) => schema += 1,
                CaseOutcome::DomainRejected(_) => domain += 1,
                _ => panic!(
                    "invalid fixture not rejected by schema or domain: {}",
                    case.note
                ),
            }
        }
        assert!(schema > 0 && domain > 0, "schema={schema} domain={domain}");
    }

    /// The same mutation the conformance harness applies
    /// (`VALIDATE_MUTATION_FAMILIES_7B_WRITE_boundedContextManifest`).
    #[test]
    fn a_schema_clean_domain_defect_keeps_the_node_first_problem_text() {
        let mut bundle = bundle();
        bundle["valid"][0]["spec"]["payload"]["authoritative_sources"][0]["purpose"] =
            Value::from("/etc/passwd");
        let note = bundle["valid"][0]["note"].as_str().unwrap().to_string();
        let id = bundle["valid"][0]["spec"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let source = bundle["valid"][0]["spec"]["payload"]["authoritative_sources"][0]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let reader = real_reader().with_file(FIXTURES_PATH, &bundle.to_string());
        assert_eq!(
            messages(&reader),
            [format!(
                "a fixture that must be a valid context manifest was rejected ({note}): context manifest \"{id}\" authoritative source {source} purpose contains a rooted (absolute) POSIX path"
            )]
        );
    }

    /// A resolver response with two unknown fields is reported in sorted
    /// field order — the existing Rust-native boundary (`COMPATIBILITY.md`),
    /// now through the typed catalogue.
    #[test]
    fn unknown_resolver_fields_are_reported_in_stable_sorted_order() {
        let mut bundle = bundle();
        let entry = bundle["resolution"]["records/execution-run/example-run-001"]
            .as_object_mut()
            .unwrap();
        entry.insert("zzzz_extra_field".to_string(), Value::from("z"));
        entry.insert("aaaa_extra_field".to_string(), Value::from("a"));
        let reader = real_reader().with_file(FIXTURES_PATH, &bundle.to_string());
        let m = messages(&reader);
        assert!(!m.is_empty());
        assert!(
            m.iter()
                .all(|l| l.contains("unknown field \"aaaa_extra_field\"")),
            "{m:?}"
        );
    }

    #[test]
    fn a_missing_resolution_map_fails_closed() {
        let mut bundle = bundle();
        bundle.as_object_mut().unwrap().remove("resolution");
        let reader = real_reader().with_file(FIXTURES_PATH, &bundle.to_string());
        let m = messages(&reader);
        assert_eq!(m.len(), 1);
        assert!(
            m[0].starts_with("the fixtures file carries no \"resolution\" object"),
            "{m:?}"
        );
    }

    #[test]
    fn missing_and_unreadable_mandatory_files_are_distinct_failures() {
        assert_eq!(
            messages(&FakeReader::new()),
            ["registries/operating-model/context-manifest.schema.json is missing; the bounded context manifest contract is a mandatory part of this Kernel, not an optional add-on"]
        );
        assert_eq!(
            messages(&real_reader().with_error(SCHEMA_PATH, io("Is a directory"))),
            ["registries/operating-model/context-manifest.schema.json could not be read: Is a directory"]
        );
        let no_fixtures = FakeReader::new()
            .with_file(SCHEMA_PATH, &real(SCHEMA_PATH))
            .with_file(ENVELOPE_PATH, &real(ENVELOPE_PATH));
        assert_eq!(
            messages(&no_fixtures),
            ["the schema carries no fixtures (registries/operating-model/fixtures/context-manifest.fixtures.json); a schema no run exercises is not one this gate has reached"]
        );
        assert_eq!(
            messages(&real_reader().with_error(FIXTURES_PATH, io("Is a directory"))),
            ["registries/operating-model/fixtures/context-manifest.fixtures.json could not be read: Is a directory"]
        );
        assert_eq!(
            messages(&real_reader().with_error(ENVELOPE_PATH, io("permission denied"))),
            ["registries/operating-model/scoped-record.schema.json could not be read: permission denied"]
        );
    }

    #[test]
    fn without_a_catalogue_the_core_fails_closed() {
        let bundle = bundle();
        let envelope: Value = serde_json::from_str(&real(ENVELOPE_PATH)).unwrap();
        let record: Value = serde_json::from_str(&real(SCHEMA_PATH)).unwrap();
        let schemas = RecordSchemas {
            envelope: &envelope,
            record: &record,
            label: "context-manifest",
        };
        let outcome = evaluate_record(
            &bundle["valid"][0]["spec"],
            &schemas,
            dto::ContextManifestDto::into_input,
            |input| check_context_manifest(input, None),
        );
        let CaseOutcome::DomainRejected(problems) = outcome else {
            panic!("a manifest without a resolver must be domain-rejected");
        };
        assert!(problems.iter().any(|p| p
            .message()
            .contains("no external record resolver was supplied")));
    }
}
