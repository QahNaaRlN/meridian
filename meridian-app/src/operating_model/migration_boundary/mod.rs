//! The app-side boundary shared by the four migration and qualification
//! families (`instance-data-migration`, `instance-canonical-export`,
//! `workspace-compatibility-qualification`,
//! `upgrade-integration-qualification`; `rust-architecture-conformance-7`).
//! Private to [`crate::operating_model`].
//!
//! Their fixture bundles carry CONTAINERS (`{ schema_version, registry_id,
//! title, <entries> }`), not single records, so one container travels
//!
//! ```text
//! serde_json::Value (fixture `registry` / a resolved composed record)
//!   -> container gate: the family schema over the container, then the
//!      scoped-record envelope over each entry (and, for an export, over
//!      each exported record) — the Node reference's problem order
//!   -> private closed DTO (#[serde(deny_unknown_fields)])
//!   -> typed meridian_core input
//!   -> the family's core check
//! ```
//!
//! and ends in one [`CaseOutcome`] of the shared run-contract boundary, so
//! valid/invalid fixture reporting, drift reporting and the `NotFound`/`Io`
//! distinction are the ones every other typed family already uses.

pub(crate) mod head;
pub(crate) mod resolution;
#[cfg(test)]
pub(crate) mod test_support;

use meridian_core::canonical::CanonicalJson;
use meridian_core::migration::resolved::OpenObject;
use meridian_core::qualification::pin::ResolvedRecord;
use meridian_core::types::Diagnostic;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use super::record_resolution::text_field;
use super::run_contract_boundary::{
    assert_supported, fail, non_empty_array, parse_json, unreadable, CaseOutcome, CasePhrases,
};
use crate::source_format::json_schema;
use crate::workspace::{ReadError, WorkspaceReader};

/// The fixture-case wording every container family shares.
pub(crate) const PHRASES: CasePhrases = CasePhrases {
    valid_rejected: "a fixture that must be a valid registry was rejected",
    invalid_clean: "a fixture that must be rejected validated clean",
    record: "registry",
};

/// One extra schema a family composes with, loaded after its own schema.
pub(crate) struct ComposedSchema {
    pub file_name: &'static str,
    /// After `<path> is missing; `.
    pub missing: &'static str,
}

/// The mandatory files and wording of one container family.
pub(crate) struct ContainerFamily {
    pub schema_name: &'static str,
    /// After `<schema path> is missing; `.
    pub missing_schema: &'static str,
    /// Every other schema, envelope included, in the reference's load order.
    pub composed: &'static [ComposedSchema],
    pub fixtures_name: &'static str,
}

pub(crate) const OPERATING_MODEL_DIR: &str = "registries/operating-model";

fn schema_path(name: &str) -> String {
    format!("{OPERATING_MODEL_DIR}/{name}")
}

/// One fixture case: its note (the reference interpolates `c && c.note`,
/// so an absent one reads `undefined`) and its container document.
pub(crate) struct ContainerCase {
    pub note: String,
    pub registry: Value,
}

/// The loaded, shape-checked bundle of a container family.
pub(crate) struct ContainerBundle {
    pub schema: Value,
    /// Every composed schema, by file name, in load order.
    pub composed: Vec<(&'static str, Value)>,
    pub valid: Vec<ContainerCase>,
    pub invalid: Vec<ContainerCase>,
    /// The whole bundle object, for the family's own resolver maps.
    pub bundle: Map<String, Value>,
}

impl ContainerBundle {
    /// A composed schema this family declared; every declared one is
    /// present once the bundle loaded.
    pub(crate) fn composed(&self, file_name: &str) -> Option<&Value> {
        self.composed
            .iter()
            .find(|(name, _)| *name == file_name)
            .map(|(_, v)| v)
    }
}

fn read(
    reader: &dyn WorkspaceReader,
    path: &str,
    missing: impl FnOnce() -> String,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<String> {
    let rel = match meridian_core::types::WorkspaceRelativePath::new(path) {
        Ok(rel) => rel,
        Err(error) => {
            diagnostics.push(fail(format!("{path} is not a workspace path: {error}")));
            return None;
        }
    };
    match reader.read_text(&rel) {
        Ok(text) => Some(text),
        Err(ReadError::NotFound) => {
            diagnostics.push(fail(missing()));
            None
        }
        Err(error) => {
            diagnostics.push(fail(unreadable(path, &error)));
            None
        }
    }
}

/// The mandatory-file and bundle-shape gate, in the reference's order: the
/// family schema (a missing one stops everything), each composed schema,
/// the supported-construct check, the fixture bundle (a missing one stops),
/// then — only when every schema is usable — the bundle's JSON, its object
/// shape and the FIRST missing non-empty `valid`/`invalid` array.
/// `NotFound` keeps the reference text; any other read error is the
/// distinct fail-closed "could not be read".
pub(crate) fn load_container_bundle(
    reader: &dyn WorkspaceReader,
    family: &ContainerFamily,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<ContainerBundle> {
    let own_path = schema_path(family.schema_name);
    let raw = read(
        reader,
        &own_path,
        || format!("{own_path} is missing; {}", family.missing_schema),
        diagnostics,
    )?;
    let schema = parse_json(&raw, family.schema_name, diagnostics);
    let mut ok = schema.is_some();
    let mut composed = Vec::with_capacity(family.composed.len());
    for dependency in family.composed {
        let path = schema_path(dependency.file_name);
        let loaded = read(
            reader,
            &path,
            || format!("{path} is missing; {}", dependency.missing),
            diagnostics,
        )
        .and_then(|raw| parse_json(&raw, dependency.file_name, diagnostics));
        match loaded {
            Some(value) => composed.push((dependency.file_name, value)),
            None => ok = false,
        }
    }
    if let Some(schema) = &schema {
        ok &= assert_supported(schema, family.schema_name, diagnostics);
    }
    let fixtures_path = format!("{OPERATING_MODEL_DIR}/fixtures/{}", family.fixtures_name);
    let fixtures_raw = read(
        reader,
        &fixtures_path,
        || {
            format!(
                "the schema carries no fixtures ({fixtures_path}); a schema no run exercises is not one this gate has reached"
            )
        },
        diagnostics,
    )?;
    let (true, Some(schema)) = (ok, schema) else {
        return None;
    };
    let bundle = parse_json(&fixtures_raw, "the fixtures file", diagnostics)?;
    let Value::Object(bundle) = bundle else {
        diagnostics.push(fail(
            "the fixtures file must be an object with non-empty \"valid\" and \"invalid\" arrays",
        ));
        return None;
    };
    let wrapped = Value::Object(bundle);
    let mut arrays = Vec::with_capacity(2);
    for key in ["valid", "invalid"] {
        let Some(cases) = non_empty_array(&wrapped, key) else {
            diagnostics.push(fail(format!(
                "the fixtures file has no non-empty \"{key}\" array"
            )));
            return None;
        };
        arrays.push(container_cases(cases));
    }
    let Value::Object(bundle) = wrapped else {
        return None;
    };
    let invalid = arrays.pop().unwrap_or_default();
    let valid = arrays.pop().unwrap_or_default();
    Some(ContainerBundle {
        schema,
        composed,
        valid,
        invalid,
        bundle,
    })
}

fn container_cases(cases: &[Value]) -> Vec<ContainerCase> {
    cases
        .iter()
        .map(|c| ContainerCase {
            note: match c.get("note") {
                Some(Value::String(s)) => s.clone(),
                Some(other) => super::record_resolution::string_coercion(other),
                None => "undefined".to_string(),
            },
            registry: c.get("registry").cloned().unwrap_or(Value::Null),
        })
        .collect()
}

/// A map member of the bundle, read as `isObject(map) ? map : {}`.
pub(crate) fn object_member<'a>(
    owner: &'a Map<String, Value>,
    key: &str,
) -> Option<&'a Map<String, Value>> {
    owner.get(key).and_then(Value::as_object)
}

/// The schemas one container is gated with.
pub(crate) struct ContainerSchemas<'a> {
    pub registry: &'a Value,
    pub envelope: &'a Value,
    /// `registrySchema is not an object; …` / `envelopeSchema is not an
    /// object; …` — the reference's own texts for this family.
    pub registry_not_object: &'static str,
    pub envelope_not_object: &'static str,
    /// The container key of the entries.
    pub entries_key: &'static str,
    /// For an export: each entry's `payload.records` are envelope-checked too.
    pub nested_records: bool,
}

enum GateFailure {
    Rejected(Vec<Diagnostic>),
    ApplyFailed(Diagnostic),
}

/// The container schema, then each entry's envelope (then each exported
/// record's envelope), in the reference's order and wording.
fn container_gate(doc: &Value, schemas: &ContainerSchemas<'_>) -> Result<(), GateFailure> {
    if !schemas.registry.is_object() {
        return Err(GateFailure::ApplyFailed(fail(schemas.registry_not_object)));
    }
    if !schemas.envelope.is_object() {
        return Err(GateFailure::ApplyFailed(fail(schemas.envelope_not_object)));
    }
    let mut problems = Vec::new();
    match json_schema::validate(doc, schemas.registry) {
        Ok(errors) => problems.extend(errors.into_iter().map(fail)),
        Err(error) => {
            return Err(GateFailure::ApplyFailed(fail(format!(
                "container/payload schema could not be applied: {error}"
            ))))
        }
    }
    let empty = Vec::new();
    let entries = doc
        .get(schemas.entries_key)
        .and_then(Value::as_array)
        .unwrap_or(&empty);
    for (i, entry) in entries.iter().enumerate() {
        match json_schema::validate(entry, schemas.envelope) {
            Ok(errors) => problems.extend(
                errors
                    .into_iter()
                    .map(|m| fail(format!("entry {i} envelope {m}"))),
            ),
            Err(error) => {
                problems.push(fail(format!(
                    "entry {i} envelope could not be applied: {error}"
                )));
                continue;
            }
        }
        if !schemas.nested_records {
            continue;
        }
        let records = entry
            .get("payload")
            .and_then(|p| p.get("records"))
            .and_then(Value::as_array)
            .unwrap_or(&empty);
        for (j, record) in records.iter().enumerate() {
            match json_schema::validate(record, schemas.envelope) {
                Ok(errors) => problems.extend(
                    errors
                        .into_iter()
                        .map(|m| fail(format!("entry {i} record {j} envelope {m}"))),
                ),
                Err(error) => problems.push(fail(format!(
                    "entry {i} record {j} envelope could not be applied: {error}"
                ))),
            }
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(GateFailure::Rejected(problems))
    }
}

/// The container through the gate and the closed DTO into its typed input
/// (`Err` = drift between the DTO and the schema the document passed).
pub(crate) enum Typed<T> {
    Input(T),
    Stopped(CaseOutcome<std::convert::Infallible>),
}

pub(crate) fn typed_container<Dto, T>(
    doc: &Value,
    schemas: &ContainerSchemas<'_>,
    convert: impl FnOnce(Dto) -> Result<T, String>,
) -> Typed<T>
where
    Dto: DeserializeOwned,
{
    match container_gate(doc, schemas) {
        Ok(()) => {}
        Err(GateFailure::Rejected(p)) => return Typed::Stopped(CaseOutcome::SchemaRejected(p)),
        Err(GateFailure::ApplyFailed(d)) => {
            return Typed::Stopped(CaseOutcome::SchemaApplyFailed(d))
        }
    }
    let dto = match Dto::deserialize(doc) {
        Ok(dto) => dto,
        Err(error) => {
            return Typed::Stopped(CaseOutcome::ConversionDrift(fail(format!(
                "the registry could not be parsed into the closed transport shape: {error}"
            ))))
        }
    };
    match convert(dto) {
        Ok(input) => Typed::Input(input),
        Err(message) => Typed::Stopped(CaseOutcome::ConversionDrift(fail(format!(
            "the registry could not be converted into its typed domain input: {message}"
        )))),
    }
}

/// Re-types a stopped outcome for any accepted type.
pub(crate) fn stopped<T>(outcome: CaseOutcome<std::convert::Infallible>) -> CaseOutcome<T> {
    match outcome {
        CaseOutcome::SchemaRejected(p) => CaseOutcome::SchemaRejected(p),
        CaseOutcome::SchemaApplyFailed(d) => CaseOutcome::SchemaApplyFailed(d),
        CaseOutcome::ConversionDrift(d) => CaseOutcome::ConversionDrift(d),
        CaseOutcome::DomainRejected(p) => CaseOutcome::DomainRejected(p),
        CaseOutcome::Accepted(never) => match never {},
    }
}

/// A stopped outcome's diagnostics — how a composing record reports a
/// composed record its own route stopped before typed input existed.
pub(crate) fn stopped_diagnostics(
    outcome: CaseOutcome<std::convert::Infallible>,
) -> Vec<Diagnostic> {
    match outcome {
        CaseOutcome::SchemaRejected(p) | CaseOutcome::DomainRejected(p) => p,
        CaseOutcome::SchemaApplyFailed(d) | CaseOutcome::ConversionDrift(d) => vec![d],
        CaseOutcome::Accepted(never) => match never {},
    }
}

/// The checked batch's outcome: accepted when it produced no problem.
pub(crate) fn checked<T>(diagnostics: Vec<Diagnostic>, accepted: T) -> CaseOutcome<T> {
    if diagnostics.is_empty() {
        CaseOutcome::Accepted(accepted)
    } else {
        CaseOutcome::DomainRejected(diagnostics)
    }
}

/// Any composed route's outcome as prefix-free diagnostics.
pub(crate) fn outcome_diagnostics<T>(outcome: &CaseOutcome<T>) -> Vec<Diagnostic> {
    match outcome {
        CaseOutcome::SchemaRejected(p) | CaseOutcome::DomainRejected(p) => p.clone(),
        CaseOutcome::SchemaApplyFailed(d) | CaseOutcome::ConversionDrift(d) => vec![d.clone()],
        CaseOutcome::Accepted(_) => Vec::new(),
    }
}

/// A composed container around one resolved record — what the reference
/// builds before it calls the composed contract's own evaluator.
pub(crate) fn single_entry_container(
    registry_id: &str,
    title: String,
    entries_key: &str,
    record: &Value,
) -> Value {
    let mut container = Map::new();
    container.insert("schema_version".to_string(), Value::from(1));
    container.insert("registry_id".to_string(), Value::from(registry_id));
    container.insert("title".to_string(), Value::from(title));
    container.insert(entries_key.to_string(), Value::Array(vec![record.clone()]));
    Value::Object(container)
}

/// `JSON.stringify(canonicalize(value))`'s input, as the one opaque
/// canonical value `meridian-core` renders and hashes.
pub(crate) fn canonical(value: &Value) -> CanonicalJson {
    match value {
        Value::Null => CanonicalJson::null(),
        Value::Bool(b) => CanonicalJson::boolean(*b),
        // Static invariant: a parsed JSON number is always finite.
        Value::Number(n) => n
            .as_f64()
            .and_then(CanonicalJson::number)
            .unwrap_or_else(CanonicalJson::null),
        Value::String(s) => CanonicalJson::string(s),
        Value::Array(items) => CanonicalJson::array(items.iter().map(canonical).collect()),
        Value::Object(members) => CanonicalJson::object(
            members
                .iter()
                .map(|(k, v)| (k.clone(), canonical(v)))
                .collect(),
        ),
    }
}

pub(crate) fn open_object(object: &Map<String, Value>) -> OpenObject {
    OpenObject::new(
        object
            .iter()
            .map(|(k, v)| (k.clone(), canonical(v)))
            .collect(),
    )
}

/// A resolved composed record: its declared kind and id, and its members.
/// A non-object never resolves.
pub(crate) fn resolved_record(value: &Value) -> Option<ResolvedRecord> {
    let object = value.as_object()?;
    Some(ResolvedRecord {
        record_type: text_field(object.get("record_type")),
        id: text_field(object.get("id")),
        members: open_object(object),
    })
}

/// A reference map of full composed records (`makeRefResolver`).
pub(crate) fn lookup<'a>(
    map: Option<&'a Map<String, Value>>,
    reference: &str,
) -> Option<&'a Value> {
    map?.get(reference).filter(|v| v.is_object())
}
