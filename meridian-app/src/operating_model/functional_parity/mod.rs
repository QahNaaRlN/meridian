//! `functional-parity` — app-owned orchestration for the
//! `rust-architecture-conformance-4` production route:
//!
//! ```text
//! WorkspaceReader
//!   -> private DTO/schema boundary (this module + super::dto/super::convert)
//!   -> typed meridian-core evidence input (meridian_core::functional_parity)
//!   -> constructor/inference checks (meridian_core::functional_parity::check_document)
//!   -> Option<FunctionalParityEvidence> + Vec<Diagnostic>
//!   -> existing CLI presentation
//! ```
//!
//! `verification/functional-parity/functional-parity-evidence.schema.json`
//! is the COMPLETE schema for one functional-parity evidence document; the
//! eleven inference rules a JSON Schema Draft 7 subset cannot express
//! (because they relate one part of a record to another) live in
//! [`meridian_core::functional_parity::checks`], never here.
//!
//! Unlike `task-specification-contract` and `instruction-source-registry`,
//! this operation does NOT validate any Instance evidence document: the
//! current production family checks the Kernel's OWN bundled schema and
//! fixtures for internal self-consistency — exactly what
//! `meridian-cli/src/commands/validate/functional_parity.rs`'s prior
//! `Value`-walking port already did
//! (`governance/plans/meridian-rust-migration-program-plan.md` §5.18.4).
//!
//! # Five outcomes per fixture case (corrective round item 3)
//!
//! `evaluate_case` (private) returns one of five typed `CaseOutcome` variants —
//! never a flat "diagnostics + optional evidence" pair a caller has to
//! re-derive meaning from:
//!
//! 1. `SchemaRejected` — the document failed the whole-document JSON Schema
//!    pass;
//! 2. `SchemaApplyFailed` — the schema itself could not even be applied;
//! 3. `ConversionDrift` — the document is schema-VALID but its closed
//!    transport DTO or [`meridian_core::functional_parity`] typed
//!    construction still failed: ALWAYS a bug in this fixture harness's own
//!    typed model, never treated as an ordinary "invalid fixture correctly
//!    rejected" outcome even when the fixture under test was declared
//!    `invalid`;
//! 4. `DomainRejected` — schema-clean and converted, but the eleven
//!    inference rules produced at least one diagnostic (the legitimate
//!    rejection path for a schema-clean `invalid` fixture);
//! 5. `Accepted` — schema-clean, converted, and the eleven inference rules
//!    produced zero diagnostics; carries the full checked
//!    [`FunctionalParityEvidence`].
//!
//! # Intentional Rust-native difference from the Node reference
//!
//! The Node reference (`scripts/kernel-validate.mjs`'s `readIfExists`)
//! swallows ANY read error — both "file does not exist" and every other I/O
//! failure — and treats both identically as "absent". This port keeps that
//! behaviour for the schema's ABSENCE (`ReadError::NotFound`: nothing to
//! check, matching the Node reference's own `info(...)` skip) but no longer
//! for a DIFFERENT I/O failure reading that same schema file (permission
//! denied, the path being a directory, an encoding failure, ...): that now
//! reports a `Fail` diagnostic instead of being silently folded into the
//! same "absent" outcome. See `COMPATIBILITY.md` and
//! `tests::a_schema_io_error_other_than_not_found_fails_closed_instead_of_skipping`,
//! and the real matched Node/Rust mutated-tree case in
//! `test/conformance-harness.test.mjs` (corrective round item 5). Reading
//! the FIXTURES file also now distinguishes `ReadError::NotFound` from any
//! other `ReadError::Io` (corrective round item 4): both still fail closed
//! (the fixtures file is mandatory once the schema is reachable), but with
//! distinct diagnostic text, so a missing-fixtures-file and an
//! unreadable-fixtures-file are never reported identically.

mod convert;
mod dto;

use meridian_core::functional_parity::FunctionalParityEvidence;
use meridian_core::types::{Diagnostic, DiagnosticLevel, WorkspaceRelativePath};
use serde_json::Value;

use crate::source_format::json_schema;
use crate::workspace::{ReadError, WorkspaceReader};

const SCHEMA_PATH: &str = "verification/functional-parity/functional-parity-evidence.schema.json";
const SCHEMA_BASENAME: &str = "functional-parity-evidence.schema.json";
const FIXTURES_PATH: &str =
    "verification/functional-parity/fixtures/functional-parity-evidence.fixtures.json";

fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message).expect("message is non-empty")
}

fn wp(literal: &str) -> WorkspaceRelativePath {
    WorkspaceRelativePath::new(literal).expect("literal path constant is a valid workspace path")
}

pub struct Outcome {
    pub diagnostics: Vec<Diagnostic>,
}

/// One fixture case's typed outcome (corrective round item 3) — see this
/// module's own doc comment.
enum CaseOutcome {
    SchemaRejected(Vec<Diagnostic>),
    SchemaApplyFailed(Diagnostic),
    ConversionDrift(Vec<Diagnostic>),
    Accepted(FunctionalParityEvidence),
    /// Schema-clean, converted cleanly, but
    /// [`meridian_core::functional_parity::check_document`] itself found a
    /// domain defect — a DIFFERENT outcome from [`CaseOutcome::ConversionDrift`]:
    /// the typed model matched the schema exactly, and the eleven inference
    /// rules did their job.
    DomainRejected(Vec<Diagnostic>),
}

fn first_message(problems: &[Diagnostic]) -> &str {
    problems.first().map(Diagnostic::message).unwrap_or("")
}

/// Converts a document already confirmed schema-VALID into typed
/// `meridian-core` input and runs the eleven inference rules over it — the
/// ONLY place [`convert::build_record`] and
/// [`meridian_core::functional_parity::check_document`] are called from.
/// Never returns [`CaseOutcome::SchemaRejected`]/[`CaseOutcome::SchemaApplyFailed`]
/// — those are [`evaluate_case`]'s own, prior step.
fn convert_and_check(doc: &Value) -> CaseOutcome {
    match serde_json::from_value::<dto::DocumentDto>(doc.clone()) {
        Ok(document_dto) => {
            let mut drift_problems = Vec::new();
            let mut records = Vec::with_capacity(document_dto.records.len());
            for record_dto in &document_dto.records {
                match convert::build_record(record_dto) {
                    Ok(record) => records.push(record),
                    Err(errs) => drift_problems.extend(errs),
                }
            }
            if !drift_problems.is_empty() {
                // A schema-clean document whose typed conversion still
                // failed is drift between this DTO and the schema, not an
                // ordinary rejection (§5.18.3 point 6, corrective round
                // item 3) — never folded into `DomainRejected`.
                return CaseOutcome::ConversionDrift(drift_problems);
            }
            let (check_problems, resolved) =
                meridian_core::functional_parity::check_document(&records);
            match resolved {
                Some(evidence) => CaseOutcome::Accepted(evidence),
                None => CaseOutcome::DomainRejected(check_problems),
            }
        }
        Err(error) => CaseOutcome::ConversionDrift(vec![fail(format!(
            "the evidence document could not be parsed into the closed transport shape: {error}"
        ))]),
    }
}

/// One fixture case, schema-validated then (only if schema-clean)
/// converted and checked.
fn evaluate_case(doc: &Value, schema: &Value) -> CaseOutcome {
    match json_schema::validate(doc, schema) {
        Ok(errors) if errors.is_empty() => convert_and_check(doc),
        Ok(errors) => CaseOutcome::SchemaRejected(errors.into_iter().map(fail).collect()),
        Err(error) => CaseOutcome::SchemaApplyFailed(fail(format!(
            "{SCHEMA_BASENAME} could not be applied: {error}"
        ))),
    }
}

/// The whole `functional-parity` production route: reads the Kernel's own
/// bundled schema and fixtures through `reader`, and — self-check only, see
/// this module's own doc comment — confirms every "valid" fixture resolves
/// cleanly to a typed [`FunctionalParityEvidence`] and every "invalid"
/// fixture is rejected by the schema OR by
/// [`meridian_core::functional_parity::check_document`] — NEVER by a
/// conversion drift, which is always reported as a harness bug regardless
/// of which array the fixture came from (corrective round item 3).
pub fn evaluate(reader: &dyn WorkspaceReader) -> Outcome {
    let schema_raw = match reader.read_text(&wp(SCHEMA_PATH)) {
        Ok(text) => text,
        Err(ReadError::NotFound) => {
            // Mirrors the Node reference's own `info(...)` skip: this
            // contract is reachable only once the PHASE D evidence schema
            // exists in the Kernel — its absence is not itself a mandatory
            // gap this check enforces.
            return Outcome {
                diagnostics: Vec::new(),
            };
        }
        Err(ReadError::Io(message)) => {
            // Rust-native fail-closed difference from the Node reference's
            // `readIfExists` (this module's own doc comment; `COMPATIBILITY.md`):
            // an I/O failure OTHER than "does not exist" is never folded
            // into the same silent "absent" outcome.
            return Outcome {
                diagnostics: vec![fail(format!("{SCHEMA_BASENAME}: {message}"))],
            };
        }
    };

    let mut diagnostics = Vec::new();
    let mut ok = true;
    let mut schema: Option<Value> = None;
    match serde_json::from_str::<Value>(&schema_raw) {
        Ok(parsed) => schema = Some(parsed),
        Err(error) => {
            diagnostics.push(fail(format!(
                "{SCHEMA_BASENAME} is not valid JSON: {error}"
            )));
            ok = false;
        }
    }
    if let Some(schema) = &schema {
        if let Err(error) = json_schema::assert_supported_deep(schema, SCHEMA_BASENAME) {
            diagnostics.push(fail(format!(
                "{SCHEMA_BASENAME} uses a construct this validator cannot check: {error}"
            )));
            ok = false;
        }
    }

    // Fixtures NotFound vs any other Io failure (corrective round item 4):
    // both fail closed — the fixtures file is mandatory once the schema is
    // reachable, matching the Node reference's own unconditional `fail(...)`
    // for a missing bundle — but with distinct diagnostic text, so an
    // operator can tell "the file was never written" from "the file exists
    // but could not be read" (permission, encoding, or similar).
    let fx_raw = match reader.read_text(&wp(FIXTURES_PATH)) {
        Ok(text) => text,
        Err(ReadError::NotFound) => {
            diagnostics.push(fail(format!(
                "the PHASE D evidence schema carries no fixtures ({FIXTURES_PATH}); a schema no run exercises is not one this gate has reached"
            )));
            return Outcome { diagnostics };
        }
        Err(ReadError::Io(message)) => {
            diagnostics.push(fail(format!(
                "{FIXTURES_PATH} could not be read: {message}"
            )));
            return Outcome { diagnostics };
        }
    };
    if !ok {
        return Outcome { diagnostics };
    }

    let bundle: Value = match serde_json::from_str(&fx_raw) {
        Ok(value) => value,
        Err(error) => {
            diagnostics.push(fail(format!(
                "the fixtures file is not valid JSON: {error}"
            )));
            return Outcome { diagnostics };
        }
    };
    let Some(groups) = bundle.as_array() else {
        diagnostics.push(fail(
            "the fixtures file must be an array carrying one group for the evidence schema; it is not an array",
        ));
        return Outcome { diagnostics };
    };
    if groups.is_empty() {
        diagnostics.push(fail(
            "the fixtures file is an empty array; the evidence schema is not covered",
        ));
        return Outcome { diagnostics };
    }

    let matching: Vec<&Value> = groups
        .iter()
        .filter(|g| g.get("schema").and_then(Value::as_str) == Some(SCHEMA_BASENAME))
        .collect();
    let strays: Vec<&Value> = groups
        .iter()
        .filter(|g| g.get("schema").and_then(Value::as_str) != Some(SCHEMA_BASENAME))
        .collect();
    if let Some(stray) = strays.first() {
        diagnostics.push(fail(format!(
            "a fixture group names schema \"{}\", which is not the PHASE D evidence schema",
            stray.get("schema").and_then(Value::as_str).unwrap_or("")
        )));
        return Outcome { diagnostics };
    }
    if matching.is_empty() {
        diagnostics.push(fail(format!(
            "no fixture group for \"{SCHEMA_BASENAME}\"; the evidence schema must be covered"
        )));
        return Outcome { diagnostics };
    }
    if matching.len() > 1 {
        diagnostics.push(fail(format!(
            "schema \"{SCHEMA_BASENAME}\" has more than one fixture group; exactly one is expected"
        )));
        return Outcome { diagnostics };
    }

    let group = matching[0];
    let mut group_shape_ok = true;
    for key in ["valid", "invalid"] {
        let is_nonempty = group
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty());
        if !is_nonempty {
            diagnostics.push(fail(format!(
                "fixture group \"{SCHEMA_BASENAME}\" has no non-empty \"{key}\" array"
            )));
            group_shape_ok = false;
        }
    }
    if !group_shape_ok {
        return Outcome { diagnostics };
    }

    let schema = schema
        .as_ref()
        .expect("ok stayed true only when schema parsed");
    for case in group["valid"]
        .as_array()
        .expect("group_shape_ok confirmed \"valid\" is a non-empty array")
    {
        let doc = case.get("doc").cloned().unwrap_or(Value::Null);
        let note = case.get("note").and_then(Value::as_str).unwrap_or("");
        match evaluate_case(&doc, schema) {
            CaseOutcome::Accepted(evidence) => {
                // `check_document` only ever returns `Some` alongside a
                // non-empty record list (`records.is_empty()` is itself
                // rule 1's own rejection) — reads the accepted evidence's
                // own content rather than discarding it once accepted,
                // catching a future regression of that invariant here
                // rather than never reading `FunctionalParityEvidence` at
                // all.
                if evidence.records().is_empty() {
                    diagnostics.push(fail(format!(
                        "internal: a fixture that must satisfy {SCHEMA_BASENAME} was accepted with zero records ({note})"
                    )));
                }
            }
            CaseOutcome::SchemaRejected(problems) => diagnostics.push(fail(format!(
                "a fixture that must satisfy {SCHEMA_BASENAME} did not ({note}): {}",
                first_message(&problems)
            ))),
            CaseOutcome::SchemaApplyFailed(d) => diagnostics.push(fail(format!(
                "a fixture that must satisfy {SCHEMA_BASENAME} threw ({note}): {}",
                d.message()
            ))),
            CaseOutcome::DomainRejected(problems) => diagnostics.push(fail(format!(
                "a fixture that must satisfy the evidence inference rules did not ({note}): {}",
                first_message(&problems)
            ))),
            CaseOutcome::ConversionDrift(problems) => diagnostics.push(fail(format!(
                "internal: a fixture that must satisfy {SCHEMA_BASENAME} passed the schema but this harness's own typed conversion drifted from it ({note}): {}",
                first_message(&problems)
            ))),
        }
    }
    for case in group["invalid"]
        .as_array()
        .expect("group_shape_ok confirmed \"invalid\" is a non-empty array")
    {
        let doc = case.get("doc").cloned().unwrap_or(Value::Null);
        let note = case.get("note").and_then(Value::as_str).unwrap_or("");
        match evaluate_case(&doc, schema) {
            CaseOutcome::SchemaRejected(_) | CaseOutcome::DomainRejected(_) => {
                // Correctly rejected, either by the schema or by the eleven
                // inference rules.
            }
            CaseOutcome::SchemaApplyFailed(d) => diagnostics.push(fail(format!(
                "an invalid fixture for {SCHEMA_BASENAME} threw instead of being rejected with errors ({note}): {}",
                d.message()
            ))),
            CaseOutcome::Accepted(_) => diagnostics.push(fail(format!(
                "a fixture that must be rejected by {SCHEMA_BASENAME} or its inference rules validated clean ({note})"
            ))),
            CaseOutcome::ConversionDrift(problems) => diagnostics.push(fail(format!(
                "internal: an invalid fixture for {SCHEMA_BASENAME} passed the schema but this harness's own typed conversion drifted instead of being rejected by the domain inference rules ({note}): {}",
                first_message(&problems)
            ))),
        }
    }

    Outcome { diagnostics }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation::mechanical_integrity::tests::{real_fs_reader, FakeReader};
    use std::collections::HashMap;

    fn real_kernel_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }

    fn real_schema_raw() -> String {
        std::fs::read_to_string(real_kernel_root().join(SCHEMA_PATH)).unwrap()
    }

    fn real_schema_value() -> Value {
        serde_json::from_str(&real_schema_raw()).unwrap()
    }

    fn real_valid_doc() -> Value {
        let raw = std::fs::read_to_string(real_kernel_root().join(FIXTURES_PATH)).unwrap();
        let bundle: Value = serde_json::from_str(&raw).unwrap();
        bundle[0]["valid"][0]["doc"].clone()
    }

    // --- evaluate_case: the five typed outcomes directly (corrective round item 7) ---

    #[test]
    fn evaluate_case_returns_accepted_for_a_real_valid_document() {
        let outcome = evaluate_case(&real_valid_doc(), &real_schema_value());
        match outcome {
            CaseOutcome::Accepted(evidence) => {
                assert_eq!(evidence.records().len(), 1);
                assert!(evidence.records()[0].overall().is_verified());
            }
            other => panic!(
                "expected Accepted, got a different CaseOutcome variant: {}",
                variant_name(&other)
            ),
        }
    }

    #[test]
    fn evaluate_case_returns_schema_rejected_for_a_schema_invalid_document() {
        let mut doc = real_valid_doc();
        doc.as_object_mut().unwrap().remove("records");
        let outcome = evaluate_case(&doc, &real_schema_value());
        match outcome {
            CaseOutcome::SchemaRejected(problems) => assert!(!problems.is_empty()),
            other => panic!(
                "expected SchemaRejected, got a different CaseOutcome variant: {}",
                variant_name(&other)
            ),
        }
    }

    #[test]
    fn evaluate_case_returns_domain_rejected_for_a_schema_clean_but_domain_invalid_document() {
        // Drops one of three post-change contract_links from an otherwise
        // complete VERIFIED record — schema-clean (the schema places no
        // constraint on contract_links' cardinality), but rule 6 rejects
        // it: the SAME mutation
        // `test/conformance-harness.test.mjs`'s
        // `VALIDATE_MUTATION_FAMILIES_7B_WRITE_functionalParity` applies to
        // the real fixture bundle.
        let mut doc = real_valid_doc();
        let links = doc["records"][0]["post_change_evidence"]["contract_links"]
            .as_array_mut()
            .unwrap();
        links.retain(|l| l["assertion_id"] != "io.mapping");
        let outcome = evaluate_case(&doc, &real_schema_value());
        match outcome {
            CaseOutcome::DomainRejected(problems) => assert!(
                problems.iter().any(|p| p
                    .message()
                    .contains("VERIFIED but no post-change contract link")),
                "{problems:?}"
            ),
            other => panic!(
                "expected DomainRejected, got a different CaseOutcome variant: {}",
                variant_name(&other)
            ),
        }
    }

    /// Corrective round item 3: a schema-clean document whose closed
    /// transport DTO cannot parse it (here, an unrecognised field the
    /// permissive test schema allows but `deny_unknown_fields` does not) is
    /// `ConversionDrift`, never silently treated as `DomainRejected`.
    #[test]
    fn evaluate_case_returns_conversion_drift_when_the_closed_dto_cannot_parse_a_schema_clean_document(
    ) {
        let permissive_schema = serde_json::json!({"type": "object"});
        let mut doc = real_valid_doc();
        doc["records"][0]["an_unrecognised_field_the_permissive_schema_allows"] = Value::Bool(true);
        let outcome = evaluate_case(&doc, &permissive_schema);
        match outcome {
            CaseOutcome::ConversionDrift(problems) => assert!(!problems.is_empty()),
            other => panic!(
                "expected ConversionDrift, got a different CaseOutcome variant: {}",
                variant_name(&other)
            ),
        }
    }

    #[test]
    fn evaluate_case_returns_schema_apply_failed_when_the_schema_itself_cannot_be_compiled() {
        let broken_schema = serde_json::json!({"type": "object", "unsupported_keyword_xyz": true});
        let outcome = evaluate_case(&real_valid_doc(), &broken_schema);
        assert!(matches!(outcome, CaseOutcome::SchemaApplyFailed(_)));
    }

    fn variant_name(outcome: &CaseOutcome) -> &'static str {
        match outcome {
            CaseOutcome::SchemaRejected(_) => "SchemaRejected",
            CaseOutcome::SchemaApplyFailed(_) => "SchemaApplyFailed",
            CaseOutcome::ConversionDrift(_) => "ConversionDrift",
            CaseOutcome::Accepted(_) => "Accepted",
            CaseOutcome::DomainRejected(_) => "DomainRejected",
        }
    }

    // --- evaluate(): the real Kernel bundle end to end ---

    #[test]
    fn the_real_kernel_schema_and_fixtures_agree() {
        let reader = real_fs_reader(real_kernel_root());
        let outcome = evaluate(&reader);
        let failures: Vec<&str> = outcome
            .diagnostics
            .iter()
            .filter(|d| d.level() == DiagnosticLevel::Fail)
            .map(Diagnostic::message)
            .collect();
        assert!(failures.is_empty(), "{failures:?}");
    }

    /// A minimal fake `WorkspaceReader` backed by an in-memory path -> text
    /// map: any path not in the map reports `ReadError::NotFound`, unless
    /// it is also listed in `io_errors`, which reports `ReadError::Io`
    /// instead — used by every bundle-shape/fixture-read test below so each
    /// one only has to state the one or two paths it actually cares about.
    struct MapReader {
        files: HashMap<&'static str, String>,
        io_errors: HashMap<&'static str, String>,
    }
    impl MapReader {
        fn new() -> Self {
            Self {
                files: HashMap::new(),
                io_errors: HashMap::new(),
            }
        }
        fn with_schema(mut self) -> Self {
            self.files.insert(SCHEMA_PATH, real_schema_raw());
            self
        }
        fn with_schema_text(mut self, raw: impl Into<String>) -> Self {
            self.files.insert(SCHEMA_PATH, raw.into());
            self
        }
        fn with_fixtures(mut self, raw: impl Into<String>) -> Self {
            self.files.insert(FIXTURES_PATH, raw.into());
            self
        }
        fn with_io_error(mut self, path: &'static str, message: impl Into<String>) -> Self {
            self.io_errors.insert(path, message.into());
            self
        }
    }
    impl WorkspaceReader for MapReader {
        fn read_text(&self, path: &WorkspaceRelativePath) -> Result<String, ReadError> {
            if let Some(message) = self.io_errors.get(path.as_str()) {
                return Err(ReadError::Io(message.clone()));
            }
            self.files
                .get(path.as_str())
                .cloned()
                .ok_or(ReadError::NotFound)
        }
        fn read_bytes(&self, _path: &WorkspaceRelativePath) -> Result<Vec<u8>, ReadError> {
            Err(ReadError::NotFound)
        }
        fn list_dir(
            &self,
            _path: &WorkspaceRelativePath,
        ) -> Result<Vec<crate::workspace::DirEntry>, ReadError> {
            Err(ReadError::NotFound)
        }
    }

    #[test]
    fn a_missing_schema_produces_no_diagnostics() {
        let reader = FakeReader::new();
        let outcome = evaluate(&reader);
        assert!(outcome.diagnostics.is_empty());
    }

    /// Rust-native fail-closed difference (this module's own doc comment,
    /// `COMPATIBILITY.md`): an I/O error reading the schema OTHER than "not
    /// found" is reported as a `Fail`, never silently folded into the same
    /// "nothing to check" outcome the Node reference's `readIfExists` would
    /// produce for ANY read failure.
    #[test]
    fn a_schema_io_error_other_than_not_found_fails_closed_instead_of_skipping() {
        let reader = MapReader::new().with_io_error(SCHEMA_PATH, "permission denied");
        let outcome = evaluate(&reader);
        assert_eq!(outcome.diagnostics.len(), 1);
        assert_eq!(outcome.diagnostics[0].level(), DiagnosticLevel::Fail);
        assert!(
            outcome.diagnostics[0]
                .message()
                .contains("permission denied"),
            "{}",
            outcome.diagnostics[0].message()
        );
    }

    /// Corrective round item 7: a malformed (not valid JSON) schema is its
    /// own distinct `Fail`, and — since `ok` becomes `false` — the fixtures
    /// bundle-shape checks never even run.
    #[test]
    fn a_malformed_schema_is_reported_and_fixtures_are_never_checked() {
        let reader = MapReader::new()
            .with_schema_text("{ not valid json")
            .with_fixtures("[]");
        let outcome = evaluate(&reader);
        assert_eq!(outcome.diagnostics.len(), 1, "{:?}", outcome.diagnostics);
        assert!(
            outcome.diagnostics[0]
                .message()
                .contains("is not valid JSON"),
            "{}",
            outcome.diagnostics[0].message()
        );
    }

    #[test]
    fn a_missing_fixtures_file_fails_closed_with_the_not_found_text() {
        let reader = MapReader::new().with_schema();
        let outcome = evaluate(&reader);
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(
            outcome.diagnostics[0]
                .message()
                .contains("carries no fixtures"),
            "{}",
            outcome.diagnostics[0].message()
        );
    }

    /// Corrective round item 4: an I/O error reading the fixtures file
    /// OTHER than "not found" is reported with DISTINCT text from the
    /// missing-file case — an operator can tell the two apart.
    #[test]
    fn an_unreadable_fixtures_file_fails_closed_with_distinct_text_from_missing() {
        let reader = MapReader::new()
            .with_schema()
            .with_io_error(FIXTURES_PATH, "permission denied");
        let outcome = evaluate(&reader);
        assert_eq!(outcome.diagnostics.len(), 1);
        let message = outcome.diagnostics[0].message();
        assert!(message.contains("could not be read"), "{message}");
        assert!(message.contains("permission denied"), "{message}");
        assert!(!message.contains("carries no fixtures"), "{message}");
    }

    /// Corrective round item 4/7: a malformed (not valid JSON) fixtures
    /// file is its own distinct `Fail`.
    #[test]
    fn a_malformed_fixtures_file_is_reported() {
        let reader = MapReader::new()
            .with_schema()
            .with_fixtures("{ not valid json");
        let outcome = evaluate(&reader);
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(
            outcome.diagnostics[0]
                .message()
                .contains("fixtures file is not valid JSON"),
            "{}",
            outcome.diagnostics[0].message()
        );
    }

    /// Corrective round item 7: bundle-shape checks — not an array.
    #[test]
    fn a_fixtures_bundle_that_is_not_an_array_is_rejected() {
        let reader = MapReader::new().with_schema().with_fixtures("{}");
        let outcome = evaluate(&reader);
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(
            outcome.diagnostics[0]
                .message()
                .contains("it is not an array"),
            "{}",
            outcome.diagnostics[0].message()
        );
    }

    /// Corrective round item 7: bundle-shape checks — empty array.
    #[test]
    fn an_empty_fixtures_bundle_is_rejected() {
        let reader = MapReader::new().with_schema().with_fixtures("[]");
        let outcome = evaluate(&reader);
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(
            outcome.diagnostics[0]
                .message()
                .contains("the fixtures file is an empty array"),
            "{}",
            outcome.diagnostics[0].message()
        );
    }

    /// Corrective round item 7: bundle-shape checks — a group naming a
    /// different schema.
    #[test]
    fn a_fixtures_bundle_with_only_a_stray_group_is_rejected() {
        let reader = MapReader::new()
            .with_schema()
            .with_fixtures(r#"[{"schema": "made-up.schema.json", "valid": [], "invalid": []}]"#);
        let outcome = evaluate(&reader);
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(
            outcome.diagnostics[0]
                .message()
                .contains("which is not the PHASE D evidence schema"),
            "{}",
            outcome.diagnostics[0].message()
        );
    }

    /// Corrective round item 7: bundle-shape checks — more than one group
    /// for the same schema.
    #[test]
    fn a_fixtures_bundle_with_two_groups_for_the_same_schema_is_rejected() {
        let group =
            format!(r#"{{"schema": "{SCHEMA_BASENAME}", "valid": [{{}}], "invalid": [{{}}]}}"#);
        let reader = MapReader::new()
            .with_schema()
            .with_fixtures(format!("[{group}, {group}]"));
        let outcome = evaluate(&reader);
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(
            outcome.diagnostics[0]
                .message()
                .contains("has more than one fixture group"),
            "{}",
            outcome.diagnostics[0].message()
        );
    }

    /// Corrective round item 7: bundle-shape checks — an empty "valid"
    /// array in the one matching group.
    #[test]
    fn a_fixtures_group_with_an_empty_valid_array_is_rejected() {
        let reader = MapReader::new().with_schema().with_fixtures(format!(
            r#"[{{"schema": "{SCHEMA_BASENAME}", "valid": [], "invalid": [{{}}]}}]"#
        ));
        let outcome = evaluate(&reader);
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(
            outcome.diagnostics[0]
                .message()
                .contains("no non-empty \"valid\" array"),
            "{}",
            outcome.diagnostics[0].message()
        );
    }
}
