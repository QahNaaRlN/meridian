//! The app-side boundary shared by the three run-contract families
//! (`execution-state-model`, `role-and-human-control`,
//! `bounded-context-manifest`; `rust-architecture-conformance-5`). Private
//! to [`crate::operating_model`]: nothing here is public API.
//!
//! One record travels
//!
//! ```text
//! serde_json::Value (fixture spec / parsed YAML)
//!   -> schema gate: scoped-record envelope + the family's own schema
//!   -> private closed DTO (#[serde(deny_unknown_fields)])
//!   -> typed meridian_core::run_contracts input
//!   -> the family's pure core check
//! ```
//!
//! and ends in exactly one [`CaseOutcome`]: rejected by the schema, the
//! schema could not be applied, schema-clean but the DTO/typed conversion
//! DRIFTED from the schema (always a harness bug, never an ordinary
//! rejection), rejected by the domain, or accepted as the core's typed
//! value. [`report_valid_case`]/[`report_invalid_case`] turn an outcome
//! into the family's prefix-free diagnostics.

pub(crate) mod envelope;

use meridian_core::types::{Diagnostic, DiagnosticLevel, WorkspaceRelativePath};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::source_format::json_schema;
use crate::workspace::ReadError;

/// Every caller passes a message that begins with fixed, non-empty text,
/// so the non-blank [`Diagnostic`] invariant holds by construction.
pub(crate) fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message)
        .expect("every run-contract boundary message starts with fixed non-empty text")
}

/// A workspace path from one of this package's compile-time path constants.
pub(crate) fn workspace_path(literal: &'static str) -> WorkspaceRelativePath {
    WorkspaceRelativePath::new(literal)
        .expect("a compile-time run-contract path constant is a valid workspace path")
}

/// The one message for a mandatory file that exists but could not be read
/// (`ReadError::Io`): never folded into the family's "is missing" text
/// (`COMPATIBILITY.md`, run-contract mandatory-read boundary).
pub(crate) fn unreadable(path: &str, error: &ReadError) -> String {
    format!("{path} could not be read: {error}")
}

/// The scoped-record envelope schema and one family schema, both parsed.
pub(crate) struct RecordSchemas<'a> {
    pub envelope: &'a Value,
    pub record: &'a Value,
    /// How the "could not be applied" message names the family schema.
    pub label: &'static str,
}

/// The typed outcome of one record.
pub(crate) enum CaseOutcome<T> {
    SchemaRejected(Vec<Diagnostic>),
    SchemaApplyFailed(Diagnostic),
    ConversionDrift(Diagnostic),
    DomainRejected(Vec<Diagnostic>),
    Accepted(T),
}

enum GateFailure {
    Rejected(Vec<Diagnostic>),
    ApplyFailed(Diagnostic),
}

/// The envelope schema, then the family schema — the Node reference's
/// problem order (envelope problems are prefixed `envelope `).
fn schema_gate(doc: &Value, schemas: &RecordSchemas<'_>) -> Result<(), GateFailure> {
    let mut problems = Vec::new();
    match json_schema::validate(doc, schemas.envelope) {
        Ok(errors) => problems.extend(errors.into_iter().map(|m| fail(format!("envelope {m}")))),
        Err(error) => {
            return Err(GateFailure::ApplyFailed(fail(format!(
                "record envelope schema could not be applied: {error}"
            ))))
        }
    }
    match json_schema::validate(doc, schemas.record) {
        Ok(errors) => problems.extend(errors.into_iter().map(fail)),
        Err(error) => {
            return Err(GateFailure::ApplyFailed(fail(format!(
                "{} schema could not be applied: {error}",
                schemas.label
            ))))
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(GateFailure::Rejected(problems))
    }
}

/// One record through the whole boundary. `convert` maps the closed DTO to
/// the core input (an `Err` is drift); `check` is the family's core check.
pub(crate) fn evaluate_record<Dto, Input, T>(
    doc: &Value,
    schemas: &RecordSchemas<'_>,
    convert: impl FnOnce(Dto) -> Result<Input, String>,
    check: impl FnOnce(Input) -> (Option<T>, Vec<Diagnostic>),
) -> CaseOutcome<T>
where
    Dto: DeserializeOwned,
{
    match schema_gate(doc, schemas) {
        Ok(()) => {}
        Err(GateFailure::Rejected(p)) => return CaseOutcome::SchemaRejected(p),
        Err(GateFailure::ApplyFailed(d)) => return CaseOutcome::SchemaApplyFailed(d),
    }
    let dto = match Dto::deserialize(doc) {
        Ok(dto) => dto,
        Err(error) => {
            return CaseOutcome::ConversionDrift(fail(format!(
                "the record could not be parsed into the closed transport shape: {error}"
            )))
        }
    };
    let input = match convert(dto) {
        Ok(input) => input,
        Err(message) => {
            return CaseOutcome::ConversionDrift(fail(format!(
                "the record could not be converted into its typed domain input: {message}"
            )))
        }
    };
    match check(input) {
        (Some(accepted), problems) if problems.is_empty() => CaseOutcome::Accepted(accepted),
        (_, problems) if !problems.is_empty() => CaseOutcome::DomainRejected(problems),
        _ => CaseOutcome::ConversionDrift(fail(
            "internal: the domain check returned neither an accepted value nor a problem",
        )),
    }
}

/// One fixture case of a bundle array: its note (for diagnostics) and its
/// record document (`spec`, `null` when absent).
pub(crate) struct FixtureCase {
    pub note: String,
    pub spec: Value,
}

pub(crate) fn fixture_cases(cases: &[Value]) -> Vec<FixtureCase> {
    cases
        .iter()
        .map(|c| FixtureCase {
            note: c
                .get("note")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            spec: c.get("spec").cloned().unwrap_or(Value::Null),
        })
        .collect()
}

/// A bundle member that must be a non-empty array.
pub(crate) fn non_empty_array<'a>(owner: &'a Value, key: &str) -> Option<&'a Vec<Value>> {
    owner
        .get(key)
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty())
}

/// The family-specific wording of fixture-case diagnostics.
pub(crate) struct CasePhrases {
    /// `a fixture that must be a valid run record was rejected`
    pub valid_rejected: &'static str,
    /// `a fixture that must be rejected validated clean`
    pub invalid_clean: &'static str,
    /// `run record` — names the record kind in drift diagnostics.
    pub record: &'static str,
}

fn first_message(problems: &[Diagnostic]) -> &str {
    problems
        .first()
        .map(Diagnostic::message)
        .unwrap_or_default()
}

/// A fixture declared valid: anything but `Accepted` is a failure; drift is
/// reported as a harness bug, distinct from a rejection.
pub(crate) fn report_valid_case<T>(
    outcome: CaseOutcome<T>,
    note: &str,
    phrases: &CasePhrases,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<T> {
    let rejected = |first: &str| fail(format!("{} ({note}): {first}", phrases.valid_rejected));
    match outcome {
        CaseOutcome::Accepted(value) => return Some(value),
        CaseOutcome::SchemaRejected(p) | CaseOutcome::DomainRejected(p) => {
            diagnostics.push(rejected(first_message(&p)))
        }
        CaseOutcome::SchemaApplyFailed(d) => diagnostics.push(rejected(d.message())),
        CaseOutcome::ConversionDrift(d) => diagnostics.push(fail(format!(
            "internal: a fixture that must be a valid {} passed the schema but this harness's own typed conversion drifted from it ({note}): {}",
            phrases.record,
            d.message()
        ))),
    }
    None
}

/// A fixture declared invalid: a schema or domain rejection is the expected
/// outcome. A schema that cannot be applied also counts as a rejection here
/// (the Node reference's behaviour) — every valid fixture of the same run
/// fails on it, so the gate still fails closed. Drift is always a failure.
pub(crate) fn report_invalid_case<T>(
    outcome: CaseOutcome<T>,
    note: &str,
    phrases: &CasePhrases,
    diagnostics: &mut Vec<Diagnostic>,
) {
    match outcome {
        CaseOutcome::SchemaRejected(_)
        | CaseOutcome::DomainRejected(_)
        | CaseOutcome::SchemaApplyFailed(_) => {}
        CaseOutcome::Accepted(_) => {
            diagnostics.push(fail(format!("{} ({note})", phrases.invalid_clean)))
        }
        CaseOutcome::ConversionDrift(d) => diagnostics.push(fail(format!(
            "internal: an invalid {} fixture passed the schema but this harness's own typed conversion drifted instead of being rejected by the domain rules ({note}): {}",
            phrases.record,
            d.message()
        ))),
    }
}

/// Parses one mandatory JSON schema, reporting `<name> is not valid JSON`.
pub(crate) fn parse_json(
    raw: &str,
    name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Value> {
    match serde_json::from_str::<Value>(raw) {
        Ok(value) => Some(value),
        Err(error) => {
            diagnostics.push(fail(format!("{name} is not valid JSON: {error}")));
            None
        }
    }
}

/// The family schema must use only constructs the validator can check.
pub(crate) fn assert_supported(
    schema: &Value,
    name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    match json_schema::assert_supported_deep(schema, name) {
        Ok(()) => true,
        Err(error) => {
            diagnostics.push(fail(format!(
                "the schema uses a construct this validator cannot check: {error}"
            )));
            false
        }
    }
}
