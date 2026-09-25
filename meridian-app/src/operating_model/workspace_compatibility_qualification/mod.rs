//! `workspace-compatibility-qualification` — the app operation of the
//! `rust-architecture-conformance-7` route for the closed verdict over one
//! workspace's connection scans, migration plan and canonical export:
//!
//! ```text
//! WorkspaceReader
//!   -> the family schema + envelope + the five composed contract schemas + fixture bundle
//!   -> connection/plan/export record maps, migration_resolution, export_resolution
//!   -> container gate -> closed DTO -> Vec<WorkspaceQualificationInput>
//!   -> per entry: resolve each pinned record; compose every pin-confirmed
//!      connection through ONE existing-project-compatibility-mode
//!      composition; type the plan and export through their own routes
//!   -> meridian_core::qualification::workspace::check_workspace_qualifications
//!      (the plan and export checks run there; the export only against the
//!      plan the qualification itself pinned)
//!   -> Vec<Diagnostic> (prefix-free)
//! ```

mod dto;

use meridian_core::qualification::workspace::{
    check_workspace_qualifications, ConnectionSlot, MigrationBoundaries, RefSlot, TypedRecord,
    WorkspaceComposition, WorkspaceQualification, WorkspaceQualificationInput,
};
use meridian_core::types::Diagnostic;
use serde_json::{Map, Value};

use super::existing_project_compatibility_mode::{compose_connections, EvalOpts};
use super::migration_boundary::resolution::{plan_resolution, source_contents};
use super::migration_boundary::{
    checked, load_container_bundle, lookup, object_member, resolved_record, stopped,
    typed_container, ComposedSchema, ContainerBundle, ContainerFamily, ContainerSchemas, Typed,
    PHRASES,
};
use super::run_contract_boundary::{report_invalid_case, report_valid_case, CaseOutcome};
use super::{instance_canonical_export as export_route, instance_data_migration as plan_route};
use crate::workspace::WorkspaceReader;

const ENVELOPE: &str = "scoped-record.schema.json";
const COMPAT: &str = "existing-project-compatibility-mode.schema.json";
const SOURCE_REGISTRY: &str = "instruction-source-registry.schema.json";
const RULE_INTAKE: &str = "controlled-rule-intake.schema.json";

const FAMILY: ContainerFamily = ContainerFamily {
    schema_name: "workspace-compatibility-qualification.schema.json",
    missing_schema: "the workspace compatibility qualification contract is a mandatory part of this Kernel, not an optional add-on",
    composed: &[
        ComposedSchema {
            file_name: ENVELOPE,
            missing: "the composed records' envelope cannot be checked without it",
        },
        ComposedSchema {
            file_name: COMPAT,
            missing: "the composed workspace connection cannot be checked without it",
        },
        ComposedSchema {
            file_name: SOURCE_REGISTRY,
            missing: "the composed workspace connection's own discovered sources cannot be checked without it",
        },
        ComposedSchema {
            file_name: RULE_INTAKE,
            missing: "the composed workspace connection's own rule candidates cannot be checked without it",
        },
        ComposedSchema {
            file_name: plan_route::SCHEMA_NAME,
            missing: "a composed migration plan cannot be checked without it",
        },
        ComposedSchema {
            file_name: export_route::SCHEMA_NAME,
            missing: "a composed canonical export cannot be checked without it",
        },
    ],
    fixtures_name: "workspace-compatibility-qualification.fixtures.json",
};

/// The operation's result: prefix-free diagnostics.
pub struct Outcome {
    pub diagnostics: Vec<Diagnostic>,
}

/// Every schema and resolver the route composes, loaded once.
struct Composition<'a> {
    schemas: ContainerSchemas<'a>,
    compat: EvalOpts<'a>,
    plan_schemas: ContainerSchemas<'a>,
    export_schemas: ContainerSchemas<'a>,
    connections: Option<&'a Map<String, Value>>,
    plans: Option<&'a Map<String, Value>>,
    exports: Option<&'a Map<String, Value>>,
}

fn composed<'a>(bundle: &'a ContainerBundle, name: &str) -> &'a Value {
    // Static invariant: `load_container_bundle` returns a bundle only when
    // every declared composed schema loaded; `Value::Null` is never reached
    // and would fail every case closed if it were.
    static MISSING: Value = Value::Null;
    bundle.composed(name).unwrap_or(&MISSING)
}

fn compose_entry(
    entry: &WorkspaceQualificationInput,
    route: &Composition<'_>,
) -> WorkspaceComposition {
    let id = entry.head.id.as_str();
    let records: Vec<Option<(&Value, _)>> = entry
        .payload
        .workspace_connection_refs
        .iter()
        .map(|pin| {
            let value = lookup(route.connections, pin.reference.as_str())?;
            let record = resolved_record(value)?;
            Some((value, record))
        })
        .collect();
    let confirmed: Vec<usize> = records
        .iter()
        .zip(&entry.payload.workspace_connection_refs)
        .enumerate()
        .filter_map(|(i, (r, pin))| {
            r.as_ref()
                .filter(|(_, record)| pin.confirms(&record.content_digest()))
                .map(|_| i)
        })
        .collect();
    let (connection_diagnostics, mut accepted) = if confirmed.is_empty() {
        (Vec::new(), Vec::new())
    } else {
        let values: Vec<&Value> = confirmed
            .iter()
            .filter_map(|&i| records[i].as_ref().map(|(v, _)| *v))
            .collect();
        let outcome = compose_connections(
            format!("{id} — composed workspace connections"),
            &values,
            &route.compat,
        );
        (outcome.diagnostics, outcome.accepted)
    };
    accepted.resize(confirmed.len(), None);
    let connections = records
        .into_iter()
        .enumerate()
        .map(|(i, r)| match r {
            None => ConnectionSlot::Unresolved,
            Some((_, record)) => ConnectionSlot::Resolved {
                record: Box::new(record),
                accepted: confirmed
                    .iter()
                    .position(|&c| c == i)
                    .and_then(|k| accepted[k].take()),
            },
        })
        .collect();

    let plan = match &entry.payload.migration_plan_ref {
        None => RefSlot::NotDeclared,
        Some(pin) => match lookup(route.plans, pin.reference.as_str())
            .and_then(|v| Some((v, resolved_record(v)?)))
        {
            None => RefSlot::Unresolved,
            Some((value, record)) => RefSlot::Resolved(TypedRecord {
                record,
                typed: plan_route::typed_plan(
                    value,
                    format!("{id} — composed migration plan"),
                    &route.plan_schemas,
                ),
            }),
        },
    };
    let export = match &entry.payload.canonical_export_ref {
        None => RefSlot::NotDeclared,
        Some(pin) => match lookup(route.exports, pin.reference.as_str())
            .and_then(|v| Some((v, resolved_record(v)?)))
        {
            None => RefSlot::Unresolved,
            Some((value, record)) => RefSlot::Resolved(TypedRecord {
                record,
                typed: export_route::typed_export(
                    value,
                    format!("{id} — composed canonical export"),
                    &route.export_schemas,
                ),
            }),
        },
    };
    WorkspaceComposition {
        connections,
        connection_diagnostics,
        plan,
        export,
    }
}

/// One fixture container through the whole route.
fn check_container(
    doc: &Value,
    route: &Composition<'_>,
    boundaries: MigrationBoundaries<'_>,
) -> CaseOutcome<Vec<WorkspaceQualification>> {
    match typed_container(doc, &route.schemas, dto::ContainerDto::into_inputs) {
        Typed::Stopped(outcome) => stopped(outcome),
        Typed::Input(inputs) => {
            let entries: Vec<_> = inputs
                .into_iter()
                .map(|entry| {
                    let composition = compose_entry(&entry, route);
                    (entry, composition)
                })
                .collect();
            let outcome = check_workspace_qualifications(&entries, boundaries);
            checked(
                outcome.diagnostics,
                outcome.accepted.into_iter().flatten().collect(),
            )
        }
    }
}

/// Builds every composed schema and resolver of `bundle` once and hands
/// them to `f` — the one composition `evaluate` checks every fixture case
/// against.
fn with_route<R>(
    bundle: &ContainerBundle,
    f: impl FnOnce(&Composition<'_>, MigrationBoundaries<'_>) -> R,
) -> R {
    let envelope = composed(bundle, ENVELOPE);
    let route = Composition {
        schemas: ContainerSchemas {
            registry: &bundle.schema,
            envelope,
            registry_not_object: "registrySchema is not an object; the workspace-compatibility-qualification contract cannot be checked without its schema",
            envelope_not_object: "envelopeSchema is not an object; a qualification record composes with the record envelope and cannot be checked without it",
            entries_key: "qualifications",
            nested_records: false,
        },
        compat: EvalOpts {
            registry_schema: composed(bundle, COMPAT),
            envelope_schema: envelope,
            source_registry_schema: composed(bundle, SOURCE_REGISTRY),
            rule_intake_schema: composed(bundle, RULE_INTAKE),
        },
        plan_schemas: plan_route::schemas(composed(bundle, plan_route::SCHEMA_NAME), envelope),
        export_schemas: export_route::schemas(composed(bundle, export_route::SCHEMA_NAME), envelope),
        connections: object_member(&bundle.bundle, "connection_record_resolution"),
        plans: object_member(&bundle.bundle, "plan_record_resolution"),
        exports: object_member(&bundle.bundle, "export_record_resolution"),
    };
    let empty = Map::new();
    let plan_resolution =
        plan_resolution(object_member(&bundle.bundle, "migration_resolution").unwrap_or(&empty));
    let source_content =
        source_contents(object_member(&bundle.bundle, "export_resolution").unwrap_or(&empty));
    f(
        &route,
        MigrationBoundaries {
            plan: &plan_resolution,
            source_content: &source_content,
        },
    )
}

/// The whole `workspace-compatibility-qualification` production route.
pub fn evaluate(reader: &dyn WorkspaceReader) -> Outcome {
    let mut diagnostics = Vec::new();
    let Some(bundle) = load_container_bundle(reader, &FAMILY, &mut diagnostics) else {
        return Outcome { diagnostics };
    };
    with_route(&bundle, |route, boundaries| {
        for case in &bundle.valid {
            let outcome = check_container(&case.registry, route, boundaries);
            report_valid_case(outcome, &case.note, &PHRASES, &mut diagnostics);
        }
        for case in &bundle.invalid {
            let outcome = check_container(&case.registry, route, boundaries);
            report_invalid_case(outcome, &case.note, &PHRASES, &mut diagnostics);
        }
    });
    Outcome { diagnostics }
}

#[cfg(test)]
mod tests;
