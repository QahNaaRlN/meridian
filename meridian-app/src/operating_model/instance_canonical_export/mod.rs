//! `instance-canonical-export` — the app operation of the
//! `rust-architecture-conformance-7` route for the checkable proof that one
//! migration plan is fully and faithfully realised:
//!
//! ```text
//! WorkspaceReader
//!   -> instance-canonical-export.schema.json + scoped-record.schema.json + fixture bundle
//!   -> plan_resolution / source_content_resolution -> typed catalogues
//!   -> container gate (every exported record's envelope too) -> closed DTO -> Vec<ExportInput>
//!   -> meridian_core::migration::export::check_canonical_exports(PlanBoundary::Resolver)
//!   -> accepted CanonicalExports + Vec<Diagnostic> (prefix-free)
//! ```
//!
//! `typed_export` is the composition point of a workspace qualification,
//! which checks the typed export against its OWN pinned plan
//! (`PlanBoundary::Pinned`) — never against this family's plan resolver.

mod dto;

use meridian_core::migration::export::{
    check_canonical_exports, CanonicalExport, ExportInput, PlanBoundary,
};
use meridian_core::migration::resolved::{
    MigrationPlanResponse, ResponseCatalogue, SourceContentResponse,
};
use meridian_core::types::Diagnostic;
use serde_json::Value;

use super::migration_boundary::resolution::{plan_responses, source_contents};
use super::migration_boundary::{
    checked, load_container_bundle, single_entry_container, stopped, stopped_diagnostics,
    typed_container, ComposedSchema, ContainerFamily, ContainerSchemas, Typed, PHRASES,
};
use super::run_contract_boundary::{report_invalid_case, report_valid_case, CaseOutcome};
use crate::workspace::WorkspaceReader;

pub(crate) const SCHEMA_NAME: &str = "instance-canonical-export.schema.json";
const REGISTRY_ID: &str = "instance-canonical-export";
const ENTRIES_KEY: &str = "exports";

const FAMILY: ContainerFamily = ContainerFamily {
    schema_name: SCHEMA_NAME,
    missing_schema:
        "the canonical export contract is a mandatory part of this Kernel, not an optional add-on",
    composed: &[ComposedSchema {
        file_name: "scoped-record.schema.json",
        missing:
            "a canonical export composes with the record envelope and cannot be checked without it",
    }],
    fixtures_name: "instance-canonical-export.fixtures.json",
};

/// The operation's result: prefix-free diagnostics.
pub struct Outcome {
    pub diagnostics: Vec<Diagnostic>,
}

pub(crate) fn schemas<'a>(registry: &'a Value, envelope: &'a Value) -> ContainerSchemas<'a> {
    ContainerSchemas {
        registry,
        envelope,
        registry_not_object: "registrySchema is not an object; the instance-canonical-export contract cannot be checked without its schema",
        envelope_not_object: "envelopeSchema is not an object; a canonical export composes with the record envelope and cannot be checked without it",
        entries_key: ENTRIES_KEY,
        nested_records: true,
    }
}

/// One fixture container through the whole route, against the family's
/// own plan resolver.
pub(crate) fn check_container(
    doc: &Value,
    schemas: &ContainerSchemas<'_>,
    plans: &ResponseCatalogue<MigrationPlanResponse>,
    content: &ResponseCatalogue<SourceContentResponse>,
) -> CaseOutcome<Vec<CanonicalExport>> {
    match typed_container(doc, schemas, dto::ContainerDto::into_inputs) {
        Typed::Stopped(outcome) => stopped(outcome),
        Typed::Input(inputs) => {
            let outcome = check_canonical_exports(&inputs, PlanBoundary::Resolver(plans), content);
            checked(
                outcome.diagnostics,
                outcome.accepted.into_iter().flatten().collect(),
            )
        }
    }
}

/// The composition point: one resolved export record through the same gate
/// and DTO, stopping at the typed input.
pub(crate) fn typed_export(
    record: &Value,
    title: String,
    schemas: &ContainerSchemas<'_>,
) -> Result<ExportInput, Vec<Diagnostic>> {
    let container = single_entry_container(REGISTRY_ID, title, ENTRIES_KEY, record);
    match typed_container(&container, schemas, dto::ContainerDto::into_inputs) {
        Typed::Stopped(outcome) => Err(stopped_diagnostics(outcome)),
        Typed::Input(mut inputs) if inputs.len() == 1 => Ok(inputs.remove(0)),
        Typed::Input(_) => Err(vec![super::run_contract_boundary::fail(
            "internal: a single-entry container did not convert to exactly one export",
        )]),
    }
}

/// The whole `instance-canonical-export` production route.
pub fn evaluate(reader: &dyn WorkspaceReader) -> Outcome {
    let mut diagnostics = Vec::new();
    let Some(bundle) = load_container_bundle(reader, &FAMILY, &mut diagnostics) else {
        return Outcome { diagnostics };
    };
    let Some(envelope) = bundle.composed("scoped-record.schema.json") else {
        return Outcome { diagnostics };
    };
    let schemas = schemas(&bundle.schema, envelope);
    let plans = plan_responses(&bundle.bundle);
    let content = source_contents(&bundle.bundle);
    for case in &bundle.valid {
        let outcome = check_container(&case.registry, &schemas, &plans, &content);
        report_valid_case(outcome, &case.note, &PHRASES, &mut diagnostics);
    }
    for case in &bundle.invalid {
        let outcome = check_container(&case.registry, &schemas, &plans, &content);
        report_invalid_case(outcome, &case.note, &PHRASES, &mut diagnostics);
    }
    Outcome { diagnostics }
}

#[cfg(test)]
mod tests;
