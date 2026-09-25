//! `role-and-human-control` — the app operation of the
//! `rust-architecture-conformance-5` route for the built-in universal-role
//! catalogue and one run's human-control record:
//!
//! ```text
//! WorkspaceReader
//!   -> role-registry.schema.json + human-control.schema.json
//!      + standards/workspace/role-registry.yaml + scoped-record.schema.json
//!      + fixture bundle { registry, control }
//!   -> schema gate -> private closed DTOs (dto.rs)
//!   -> meridian_core::run_contracts::{RoleRegistryInput, HumanControlInput}
//!   -> check_role_registry / check_human_control
//!   -> Option<RoleCatalogue | HumanControl> + Vec<Diagnostic> (prefix-free)
//! ```
//!
//! The canonical catalogue `standards/workspace/role-registry.yaml` goes
//! through the SAME route as the registry fixtures. Every file is
//! mandatory; `ReadError::NotFound` keeps the Node reference's text and any
//! other `ReadError::Io` is reported as "… could not be read: …"
//! (`COMPATIBILITY.md`).

mod dto;

use meridian_core::run_contracts::{
    check_human_control, check_role_registry, HumanControl, RoleCatalogue,
};
use meridian_core::types::Diagnostic;
use serde_json::Value;

use super::run_contract_boundary::{
    assert_supported, evaluate_record, fail, fixture_cases, non_empty_array, parse_json,
    report_invalid_case, report_valid_case, unreadable, workspace_path, CaseOutcome, CasePhrases,
    RecordSchemas,
};
use crate::source_format::parse_yaml;
use crate::workspace::{ReadError, WorkspaceReader};

const REGISTRY_SCHEMA_NAME: &str = "role-registry.schema.json";
const REGISTRY_SCHEMA_PATH: &str = "registries/operating-model/role-registry.schema.json";
const CONTROL_SCHEMA_NAME: &str = "human-control.schema.json";
const CONTROL_SCHEMA_PATH: &str = "registries/operating-model/human-control.schema.json";
const CATALOGUE_PATH: &str = "standards/workspace/role-registry.yaml";
const ENVELOPE_PATH: &str = "registries/operating-model/scoped-record.schema.json";
const FIXTURES_PATH: &str =
    "registries/operating-model/fixtures/role-and-human-control.fixtures.json";

const REGISTRY_PHRASES: CasePhrases = CasePhrases {
    valid_rejected: "a fixture that must be a valid role registry was rejected",
    invalid_clean: "a role registry fixture that must be rejected validated clean",
    record: "role registry",
};

const CONTROL_PHRASES: CasePhrases = CasePhrases {
    valid_rejected: "a fixture that must be a valid human-control record was rejected",
    invalid_clean: "a human-control fixture that must be rejected validated clean",
    record: "human-control record",
};

/// The operation's result: prefix-free diagnostics.
pub struct Outcome {
    pub diagnostics: Vec<Diagnostic>,
}

fn evaluate_registry(doc: &Value, schemas: &RecordSchemas<'_>) -> CaseOutcome<RoleCatalogue> {
    evaluate_record(
        doc,
        schemas,
        dto::RoleRegistryDto::into_input,
        check_role_registry,
    )
}

fn evaluate_control(doc: &Value, schemas: &RecordSchemas<'_>) -> CaseOutcome<HumanControl> {
    evaluate_record(
        doc,
        schemas,
        dto::HumanControlDto::into_input,
        check_human_control,
    )
}

/// The first bundle-shape defect (the Node reference reports only one).
fn bundle_shape_defect(bundle: &Value) -> Option<String> {
    if !bundle.is_object() {
        return Some(
            "the fixtures file must be an object carrying \"registry\" and \"control\" groups, each with non-empty \"valid\" and \"invalid\" arrays"
                .to_string(),
        );
    }
    for group in ["registry", "control"] {
        let Some(members) = bundle.get(group).filter(|g| g.is_object() || g.is_array()) else {
            return Some(format!("the fixtures file has no \"{group}\" group"));
        };
        for key in ["valid", "invalid"] {
            if non_empty_array(members, key).is_none() {
                return Some(format!(
                    "the fixtures \"{group}\" group has no non-empty \"{key}\" array"
                ));
            }
        }
    }
    None
}

/// The whole `role-and-human-control` production route.
pub fn evaluate(reader: &dyn WorkspaceReader) -> Outcome {
    let mut diagnostics = Vec::new();
    let registry_raw = reader.read_text(&workspace_path(REGISTRY_SCHEMA_PATH));
    let control_raw = reader.read_text(&workspace_path(CONTROL_SCHEMA_PATH));
    let catalogue_raw = reader.read_text(&workspace_path(CATALOGUE_PATH));
    let (registry_raw, control_raw) = match (registry_raw, control_raw) {
        (Ok(registry), Ok(control)) => (registry, control),
        (registry, control) => {
            let (path, error) = match (registry, control) {
                (Err(error), _) => (REGISTRY_SCHEMA_PATH, error),
                (_, Err(error)) => (CONTROL_SCHEMA_PATH, error),
                (Ok(_), Ok(_)) => return Outcome { diagnostics },
            };
            diagnostics.push(fail(match error {
                ReadError::NotFound => format!(
                    "{path} is missing; the role and human control contract is a mandatory part of this Kernel, not an optional add-on"
                ),
                error => unreadable(path, &error),
            }));
            return Outcome { diagnostics };
        }
    };
    let catalogue_raw = match catalogue_raw {
        Ok(text) => text,
        Err(ReadError::NotFound) => {
            diagnostics.push(fail(format!(
                "{CATALOGUE_PATH} is missing; the built-in universal-role catalogue is a mandatory part of this Kernel"
            )));
            return Outcome { diagnostics };
        }
        Err(error) => {
            diagnostics.push(fail(unreadable(CATALOGUE_PATH, &error)));
            return Outcome { diagnostics };
        }
    };

    let registry_schema = parse_json(&registry_raw, REGISTRY_SCHEMA_NAME, &mut diagnostics);
    let control_schema = parse_json(&control_raw, CONTROL_SCHEMA_NAME, &mut diagnostics);
    let catalogue = match parse_yaml(&catalogue_raw) {
        Ok(value) => Some(value),
        Err(error) => {
            diagnostics.push(fail(format!("cannot parse role-registry.yaml: {error}")));
            None
        }
    };
    let envelope = match reader.read_text(&workspace_path(ENVELOPE_PATH)) {
        Ok(raw) => parse_json(&raw, "scoped-record.schema.json", &mut diagnostics),
        Err(ReadError::NotFound) => {
            diagnostics.push(fail(format!(
                "{ENVELOPE_PATH} is missing; each record composes with the record envelope and cannot be checked without it"
            )));
            None
        }
        Err(error) => {
            diagnostics.push(fail(unreadable(ENVELOPE_PATH, &error)));
            None
        }
    };
    let mut supported = true;
    for (schema, name) in [
        (&registry_schema, REGISTRY_SCHEMA_NAME),
        (&control_schema, CONTROL_SCHEMA_NAME),
    ] {
        if let Some(schema) = schema {
            supported &= assert_supported(schema, name, &mut diagnostics);
        }
    }

    let fixtures_raw = match reader.read_text(&workspace_path(FIXTURES_PATH)) {
        Ok(text) => text,
        Err(ReadError::NotFound) => {
            diagnostics.push(fail(format!(
                "the schemas carry no fixtures ({FIXTURES_PATH}); a schema no run exercises is not one this gate has reached"
            )));
            return Outcome { diagnostics };
        }
        Err(error) => {
            diagnostics.push(fail(unreadable(FIXTURES_PATH, &error)));
            return Outcome { diagnostics };
        }
    };
    let (true, Some(registry_schema), Some(control_schema), Some(catalogue), Some(envelope)) = (
        supported,
        registry_schema,
        control_schema,
        catalogue,
        envelope,
    ) else {
        return Outcome { diagnostics };
    };

    let Some(bundle) = parse_json(&fixtures_raw, "the fixtures file", &mut diagnostics) else {
        return Outcome { diagnostics };
    };
    if let Some(defect) = bundle_shape_defect(&bundle) {
        diagnostics.push(fail(defect));
        return Outcome { diagnostics };
    }
    let cases = |group: &str, key: &str| {
        fixture_cases(non_empty_array(&bundle[group], key).map_or(&[][..], Vec::as_slice))
    };

    let registry_schemas = RecordSchemas {
        envelope: &envelope,
        record: &registry_schema,
        label: "role-registry",
    };
    for case in cases("registry", "valid") {
        let outcome = evaluate_registry(&case.spec, &registry_schemas);
        report_valid_case(outcome, &case.note, &REGISTRY_PHRASES, &mut diagnostics);
    }
    for case in cases("registry", "invalid") {
        let outcome = evaluate_registry(&case.spec, &registry_schemas);
        report_invalid_case(outcome, &case.note, &REGISTRY_PHRASES, &mut diagnostics);
    }
    let control_schemas = RecordSchemas {
        envelope: &envelope,
        record: &control_schema,
        label: "human-control",
    };
    for case in cases("control", "valid") {
        let outcome = evaluate_control(&case.spec, &control_schemas);
        report_valid_case(outcome, &case.note, &CONTROL_PHRASES, &mut diagnostics);
    }
    for case in cases("control", "invalid") {
        let outcome = evaluate_control(&case.spec, &control_schemas);
        report_invalid_case(outcome, &case.note, &CONTROL_PHRASES, &mut diagnostics);
    }

    // The shipped catalogue is the canonical valid role registry — checked
    // through the same route, once the fixtures agree.
    if diagnostics.is_empty() {
        report_catalogue(
            evaluate_registry(&catalogue, &registry_schemas),
            &mut diagnostics,
        );
    }
    Outcome { diagnostics }
}

fn report_catalogue(outcome: CaseOutcome<RoleCatalogue>, diagnostics: &mut Vec<Diagnostic>) {
    let first = match outcome {
        CaseOutcome::Accepted(_) => return,
        CaseOutcome::SchemaRejected(p) | CaseOutcome::DomainRejected(p) => p
            .first()
            .map(|d| d.message().to_string())
            .unwrap_or_default(),
        CaseOutcome::SchemaApplyFailed(d) => d.message().to_string(),
        CaseOutcome::ConversionDrift(d) => {
            diagnostics.push(fail(format!(
                "internal: {CATALOGUE_PATH} passed the schema but this harness's own typed conversion drifted from it: {}",
                d.message()
            )));
            return;
        }
    };
    diagnostics.push(fail(format!(
        "{CATALOGUE_PATH} is not a valid role registry: {first}"
    )));
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
        [
            REGISTRY_SCHEMA_PATH,
            CONTROL_SCHEMA_PATH,
            CATALOGUE_PATH,
            ENVELOPE_PATH,
            FIXTURES_PATH,
        ]
        .into_iter()
        .fold(FakeReader::new(), |r, p| r.with_file(p, &real(p)))
    }

    fn messages(reader: &dyn WorkspaceReader) -> Vec<String> {
        evaluate(reader)
            .diagnostics
            .iter()
            .map(|d| d.message().to_string())
            .collect()
    }

    fn json(path: &str) -> Value {
        serde_json::from_str(&real(path)).unwrap()
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
    fn every_real_fixture_and_the_canonical_catalogue_reach_their_typed_outcome() {
        let bundle = json(FIXTURES_PATH);
        let envelope = json(ENVELOPE_PATH);
        let registry = json(REGISTRY_SCHEMA_PATH);
        let control = json(CONTROL_SCHEMA_PATH);
        let registry_schemas = RecordSchemas {
            envelope: &envelope,
            record: &registry,
            label: "role-registry",
        };
        let control_schemas = RecordSchemas {
            envelope: &envelope,
            record: &control,
            label: "human-control",
        };

        let catalogue = parse_yaml(&real(CATALOGUE_PATH)).unwrap();
        let CaseOutcome::Accepted(accepted) = evaluate_registry(&catalogue, &registry_schemas)
        else {
            panic!("the canonical catalogue must be accepted");
        };
        assert_eq!(accepted.roles().len(), 7);

        for case in fixture_cases(bundle["registry"]["valid"].as_array().unwrap()) {
            assert!(
                matches!(
                    evaluate_registry(&case.spec, &registry_schemas),
                    CaseOutcome::Accepted(_)
                ),
                "{}",
                case.note
            );
        }
        for case in fixture_cases(bundle["control"]["valid"].as_array().unwrap()) {
            match evaluate_control(&case.spec, &control_schemas) {
                CaseOutcome::Accepted(control) => {
                    assert!(!control.control().switch_history.is_empty())
                }
                _ => panic!("valid control fixture not accepted: {}", case.note),
            }
        }
        let (mut schema, mut domain) = (0, 0);
        for case in fixture_cases(bundle["registry"]["invalid"].as_array().unwrap()) {
            match evaluate_registry(&case.spec, &registry_schemas) {
                CaseOutcome::SchemaRejected(_) => schema += 1,
                CaseOutcome::DomainRejected(_) => domain += 1,
                _ => panic!("invalid registry fixture not rejected: {}", case.note),
            }
        }
        for case in fixture_cases(bundle["control"]["invalid"].as_array().unwrap()) {
            match evaluate_control(&case.spec, &control_schemas) {
                CaseOutcome::SchemaRejected(_) => schema += 1,
                CaseOutcome::DomainRejected(_) => domain += 1,
                _ => panic!("invalid control fixture not rejected: {}", case.note),
            }
        }
        assert!(schema > 0 && domain > 0, "schema={schema} domain={domain}");
    }

    /// The conformance harness's mutation: the catalogue's role list
    /// duplicated to its end — rejected by the schema's `maxItems`, reported
    /// with the first schema problem.
    #[test]
    fn a_duplicated_canonical_catalogue_is_rejected_through_the_same_route() {
        let yaml = real(CATALOGUE_PATH);
        let marker = "  roles:\n";
        let at = yaml.find(marker).unwrap() + marker.len();
        let mutated = format!("{yaml}{}", &yaml[at..]);
        let m = messages(&real_reader().with_file(CATALOGUE_PATH, &mutated));
        assert_eq!(m.len(), 1, "{m:?}");
        assert!(
            m[0].starts_with(
                "standards/workspace/role-registry.yaml is not a valid role registry: "
            ),
            "{m:?}"
        );
    }

    #[test]
    fn missing_and_unreadable_mandatory_files_are_distinct_failures() {
        let m = messages(&FakeReader::new());
        assert_eq!(m, ["registries/operating-model/role-registry.schema.json is missing; the role and human control contract is a mandatory part of this Kernel, not an optional add-on"]);
        let m = messages(&real_reader().with_error(CONTROL_SCHEMA_PATH, io("Is a directory")));
        assert_eq!(m, ["registries/operating-model/human-control.schema.json could not be read: Is a directory"]);
        let without_catalogue = [
            REGISTRY_SCHEMA_PATH,
            CONTROL_SCHEMA_PATH,
            ENVELOPE_PATH,
            FIXTURES_PATH,
        ]
        .into_iter()
        .fold(FakeReader::new(), |r, p| r.with_file(p, &real(p)));
        assert_eq!(
            messages(&without_catalogue),
            ["standards/workspace/role-registry.yaml is missing; the built-in universal-role catalogue is a mandatory part of this Kernel"]
        );
        let m = messages(&real_reader().with_error(CATALOGUE_PATH, io("Is a directory")));
        assert_eq!(
            m,
            ["standards/workspace/role-registry.yaml could not be read: Is a directory"]
        );
        let m = messages(&real_reader().with_error(FIXTURES_PATH, io("Is a directory")));
        assert_eq!(m, ["registries/operating-model/fixtures/role-and-human-control.fixtures.json could not be read: Is a directory"]);
        let m = messages(&real_reader().with_error(ENVELOPE_PATH, io("permission denied")));
        assert_eq!(m, ["registries/operating-model/scoped-record.schema.json could not be read: permission denied"]);
    }

    #[test]
    fn bundle_shape_defects_report_only_the_first() {
        let m = messages(&real_reader().with_file(FIXTURES_PATH, "{}"));
        assert_eq!(m, ["the fixtures file has no \"registry\" group"]);
        let m =
            messages(&real_reader().with_file(FIXTURES_PATH, r#"{"registry": [], "control": 1}"#));
        assert_eq!(
            m,
            ["the fixtures \"registry\" group has no non-empty \"valid\" array"]
        );
        let m = messages(&real_reader().with_file(FIXTURES_PATH, "[]"));
        assert!(
            m[0].starts_with("the fixtures file must be an object carrying"),
            "{m:?}"
        );
    }

    #[test]
    fn a_schema_clean_control_defect_keeps_the_node_first_problem_text() {
        let mut bundle = json(FIXTURES_PATH);
        let case = &mut bundle["control"]["valid"][0];
        let holder = case["spec"]["payload"]["human_authority"]["holder"].clone();
        case["spec"]["payload"]["human_authority"]["holder"] = Value::from("unassigned-actor");
        let note = case["note"].as_str().unwrap().to_string();
        let id = case["spec"]["id"].as_str().unwrap().to_string();
        assert_ne!(holder, "unassigned-actor");
        let m = messages(&real_reader().with_file(FIXTURES_PATH, &bundle.to_string()));
        assert_eq!(
            m,
            [format!("a fixture that must be a valid human-control record was rejected ({note}): human-control record \"{id}\" human_authority.holder \"unassigned-actor\" is not a participant assigned the owner role; human-in-command is held by an assigned owner")]
        );
    }
}
