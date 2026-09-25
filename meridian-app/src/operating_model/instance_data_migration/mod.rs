//! `instance-data-migration` — the app operation of the
//! `rust-architecture-conformance-7` route for storage-neutral migration
//! plans:
//!
//! ```text
//! WorkspaceReader
//!   -> instance-data-migration.schema.json + scoped-record.schema.json + fixture bundle
//!   -> the bundle's six resolver maps -> typed PlanResolution (migration_boundary::resolution)
//!   -> container gate -> private closed DTO (dto.rs) -> Vec<PlanInput>
//!   -> meridian_core::migration::plan::check_migration_plans
//!   -> accepted MigrationPlans + Vec<Diagnostic> (prefix-free)
//! ```
//!
//! `typed_plan` is the composition point a workspace qualification uses:
//! the SAME gate and DTO over the single-entry container the reference
//! builds around one resolved plan, stopping at the typed input — the
//! qualification pins it by its recomputed fingerprint and runs the same
//! core check itself.

mod dto;

use meridian_core::migration::plan::{check_migration_plans, MigrationPlan, PlanInput};
use meridian_core::migration::resolved::PlanResolution;
use meridian_core::types::Diagnostic;
use serde_json::Value;

use super::migration_boundary::resolution::plan_resolution;
use super::migration_boundary::{
    checked, load_container_bundle, single_entry_container, stopped, stopped_diagnostics,
    typed_container, ComposedSchema, ContainerFamily, ContainerSchemas, Typed, PHRASES,
};
use super::run_contract_boundary::{report_invalid_case, report_valid_case, CaseOutcome};
use crate::workspace::WorkspaceReader;

pub(crate) const SCHEMA_NAME: &str = "instance-data-migration.schema.json";
const REGISTRY_ID: &str = "instance-data-migration";
const ENTRIES_KEY: &str = "migration_plans";

const FAMILY: ContainerFamily = ContainerFamily {
    schema_name: SCHEMA_NAME,
    missing_schema: "the instance-data migration contract is a mandatory part of this Kernel, not an optional add-on",
    composed: &[ComposedSchema {
        file_name: "scoped-record.schema.json",
        missing: "the migration plan composes with the record envelope and cannot be checked without it",
    }],
    fixtures_name: "instance-data-migration.fixtures.json",
};

/// The operation's result: prefix-free diagnostics.
pub struct Outcome {
    pub diagnostics: Vec<Diagnostic>,
}

pub(crate) fn schemas<'a>(registry: &'a Value, envelope: &'a Value) -> ContainerSchemas<'a> {
    ContainerSchemas {
        registry,
        envelope,
        registry_not_object: "registrySchema is not an object; the instance-data-migration contract cannot be checked without its schema",
        envelope_not_object: "envelopeSchema is not an object; a migration plan composes with the record envelope and cannot be checked without it",
        entries_key: ENTRIES_KEY,
        nested_records: false,
    }
}

/// One fixture container through the whole route.
pub(crate) fn check_container(
    doc: &Value,
    schemas: &ContainerSchemas<'_>,
    resolution: &PlanResolution,
) -> CaseOutcome<Vec<MigrationPlan>> {
    match typed_container(doc, schemas, dto::ContainerDto::into_inputs) {
        Typed::Stopped(outcome) => stopped(outcome),
        Typed::Input(inputs) => {
            let outcome = check_migration_plans(&inputs, resolution);
            checked(
                outcome.diagnostics,
                outcome.accepted.into_iter().flatten().collect(),
            )
        }
    }
}

/// The composition point: one resolved plan record, wrapped the way the
/// reference wraps it, through the same gate and DTO. `Err` carries that
/// route's own rejection, prefix-free.
pub(crate) fn typed_plan(
    record: &Value,
    title: String,
    schemas: &ContainerSchemas<'_>,
) -> Result<PlanInput, Vec<Diagnostic>> {
    let container = single_entry_container(REGISTRY_ID, title, ENTRIES_KEY, record);
    match typed_container(&container, schemas, dto::ContainerDto::into_inputs) {
        Typed::Stopped(outcome) => Err(stopped_diagnostics(outcome)),
        Typed::Input(mut inputs) if inputs.len() == 1 => Ok(inputs.remove(0)),
        Typed::Input(_) => Err(vec![super::run_contract_boundary::fail(
            "internal: a single-entry container did not convert to exactly one plan",
        )]),
    }
}

/// The whole `instance-data-migration` production route.
pub fn evaluate(reader: &dyn WorkspaceReader) -> Outcome {
    let mut diagnostics = Vec::new();
    let Some(bundle) = load_container_bundle(reader, &FAMILY, &mut diagnostics) else {
        return Outcome { diagnostics };
    };
    let Some(envelope) = bundle.composed("scoped-record.schema.json") else {
        return Outcome { diagnostics };
    };
    let schemas = schemas(&bundle.schema, envelope);
    let resolution = plan_resolution(&bundle.bundle);
    for case in &bundle.valid {
        let outcome = check_container(&case.registry, &schemas, &resolution);
        report_valid_case(outcome, &case.note, &PHRASES, &mut diagnostics);
    }
    for case in &bundle.invalid {
        let outcome = check_container(&case.registry, &schemas, &resolution);
        report_invalid_case(outcome, &case.note, &PHRASES, &mut diagnostics);
    }
    Outcome { diagnostics }
}

#[cfg(test)]
mod tests;
