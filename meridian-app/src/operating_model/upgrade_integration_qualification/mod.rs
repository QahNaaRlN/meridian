//! `upgrade-integration-qualification` — the app operation of the
//! `rust-architecture-conformance-7` route for the closed verdict over one
//! workspace's task journey, field-evaluation report and three neutral
//! scenarios:
//!
//! ```text
//! WorkspaceReader (+ the TaskPatternCatalog task-pattern-registry published)
//!   -> the family schema + envelope + the four composed contract schemas + fixture bundle
//!   -> the pinned-record maps and the two nested `resolution` maps
//!      (the ONE record_resolution conversion)
//!   -> container gate -> closed DTO -> Vec<UpgradeQualificationInput>
//!   -> per entry: resolve and content-pin every reference; compose each
//!      pin-confirmed record through its own family's typed composition
//!      point (evidence_and_handoff::check_record,
//!      field_evaluation::check_record, task_specification::evaluate_document,
//!      execution_state::evaluate_case)
//!   -> meridian_core::qualification::upgrade::check_upgrade_qualifications
//!   -> Vec<Diagnostic> (prefix-free)
//! ```

mod dto;

use meridian_core::field_evaluation::FieldEvaluationRecord;
use meridian_core::qualification::pin::QualificationPin;
use meridian_core::qualification::upgrade::{
    check_upgrade_qualifications, JourneyFacts, RecordSlot, ReportFacts, RunFacts, ScenarioSlots,
    SpecificationFacts, UpgradeComposition, UpgradeQualification, UpgradeQualificationInput,
};
use meridian_core::qualification::Composed;
use meridian_core::run_contracts::ResolutionCatalogue;
use meridian_core::task_contracts::TaskPatternCatalog;
use meridian_core::types::Diagnostic;
use serde_json::{Map, Value};

use super::migration_boundary::{
    checked, load_container_bundle, lookup, object_member, outcome_diagnostics, resolved_record,
    stopped, typed_container, ComposedSchema, ContainerBundle, ContainerFamily, ContainerSchemas,
    Typed, PHRASES,
};
use super::record_resolution::catalogue;
use super::run_contract_boundary::{
    report_invalid_case, report_valid_case, CaseOutcome, RecordSchemas,
};
use super::{evidence_and_handoff, execution_state, field_evaluation, task_specification};
use crate::workspace::WorkspaceReader;

const ENVELOPE: &str = "scoped-record.schema.json";
const HANDOFF: &str = "evidence-and-handoff.schema.json";
const FIELD: &str = "field-evaluation.schema.json";
const SPECIFICATION: &str = "task-specification.schema.json";
const EXECUTION: &str = "execution-state.schema.json";

const FAMILY: ContainerFamily = ContainerFamily {
    schema_name: "upgrade-integration-qualification.schema.json",
    missing_schema: "the upgrade integration qualification contract is a mandatory part of this Kernel, not an optional add-on",
    composed: &[
        ComposedSchema {
            file_name: ENVELOPE,
            missing: "the composed records' envelope cannot be checked without it",
        },
        ComposedSchema {
            file_name: HANDOFF,
            missing: "the composed task journey cannot be checked without it",
        },
        ComposedSchema {
            file_name: FIELD,
            missing: "a composed field-evaluation report cannot be checked without it",
        },
        ComposedSchema {
            file_name: SPECIFICATION,
            missing: "a composed scenario task specification cannot be checked without it",
        },
        ComposedSchema {
            file_name: EXECUTION,
            missing: "a composed scenario execution run cannot be checked without it",
        },
    ],
    fixtures_name: "upgrade-integration-qualification.fixtures.json",
};

/// The operation's result: prefix-free diagnostics.
pub struct Outcome {
    pub diagnostics: Vec<Diagnostic>,
}

/// Every schema, map and catalogue the route composes, loaded once.
struct Route<'a> {
    schemas: ContainerSchemas<'a>,
    handoff: RecordSchemas<'a>,
    field: RecordSchemas<'a>,
    execution: RecordSchemas<'a>,
    specification_schema: &'a Value,
    envelope: &'a Value,
    catalog: &'a TaskPatternCatalog,
    journeys: Option<&'a Map<String, Value>>,
    journey_resolution: ResolutionCatalogue,
    reports: Option<&'a Map<String, Value>>,
    report_resolution: ResolutionCatalogue,
    specifications: Option<&'a Map<String, Value>>,
    runs: Option<&'a Map<String, Value>>,
}

fn composed<'a>(bundle: &'a ContainerBundle, name: &str) -> &'a Value {
    // Static invariant: `load_container_bundle` returns a bundle only when
    // every declared composed schema loaded; `Value::Null` is never reached
    // and would fail every case closed if it were.
    static MISSING: Value = Value::Null;
    bundle.composed(name).unwrap_or(&MISSING)
}

/// A composed route's outcome in the qualification's terms.
fn composed_outcome<T, F>(
    outcome: CaseOutcome<T>,
    facts: impl FnOnce(T) -> Option<F>,
) -> Composed<F> {
    let diagnostics = outcome_diagnostics(&outcome);
    match outcome {
        CaseOutcome::Accepted(value) => Composed {
            diagnostics,
            accepted: facts(value),
        },
        _ => Composed::rejected(diagnostics),
    }
}

/// Resolution, content pin and — only for a pin-confirmed record —
/// composition of one slot.
fn slot<F>(
    map: Option<&Map<String, Value>>,
    pin: &QualificationPin,
    compose: impl FnOnce(&Value) -> Composed<F>,
) -> RecordSlot<F> {
    let Some((value, record)) =
        lookup(map, pin.reference.as_str()).and_then(|v| Some((v, resolved_record(v)?)))
    else {
        return RecordSlot::Unresolved;
    };
    let composed = pin
        .confirms(&record.content_digest())
        .then(|| compose(value));
    RecordSlot::Resolved { record, composed }
}

fn compose_entry(entry: &UpgradeQualificationInput, route: &Route<'_>) -> UpgradeComposition {
    let payload = &entry.payload;
    let journey = slot(route.journeys, &payload.task_journey_ref, |value| {
        composed_outcome(
            evidence_and_handoff::check_record(
                value,
                &route.handoff,
                Some(&route.journey_resolution),
            ),
            |handoff| {
                Some(JourneyFacts {
                    outcome: handoff.outcome(),
                    workspace_id: handoff.scope().workspace_id().as_str().to_string(),
                })
            },
        )
    });
    let field_report = payload.field_evaluation_report_ref.as_ref().map(|pin| {
        slot(route.reports, pin, |value| {
            composed_outcome(
                field_evaluation::check_record(value, &route.field, Some(&route.report_resolution)),
                |record| match record {
                    FieldEvaluationRecord::Report(report) => Some(ReportFacts {
                        workspace_id: report.workspace().as_str().to_string(),
                    }),
                    FieldEvaluationRecord::Observation(_) => None,
                },
            )
        })
    });
    let scenarios = payload
        .scenario_classifications
        .iter()
        .map(|scenario| ScenarioSlots {
            specification: slot(
                route.specifications,
                &scenario.task_specification_ref,
                |value| {
                    let (diagnostics, resolved) = task_specification::evaluate_document(
                        value,
                        route.specification_schema,
                        route.envelope,
                        route.catalog,
                    );
                    let accepted = resolved.filter(|_| diagnostics.is_empty()).map(|spec| {
                        let scope = spec.scope();
                        SpecificationFacts {
                            task_pattern: spec.task_pattern().clone(),
                            workspace_id: if scope.scope_type.as_deref()
                                == Some("project-workspace")
                            {
                                scope.id.clone()
                            } else {
                                scope.workspace_id.clone()
                            },
                        }
                    });
                    Composed {
                        diagnostics,
                        accepted,
                    }
                },
            ),
            run: slot(route.runs, &scenario.execution_state_ref, |value| {
                composed_outcome(
                    execution_state::evaluate_case(value, &route.execution),
                    |run| {
                        let state = run.state();
                        Some(RunFacts {
                            lifecycle_stage: state.lifecycle_stage,
                            task_specification_ref: state
                                .task_specification_ref
                                .as_str()
                                .to_string(),
                            workspace_id: run.scope().workspace_id().as_str().to_string(),
                        })
                    },
                )
            }),
        })
        .collect();
    UpgradeComposition {
        journey,
        field_report,
        scenarios,
    }
}

/// One fixture container through the whole route.
fn check_container(doc: &Value, route: &Route<'_>) -> CaseOutcome<Vec<UpgradeQualification>> {
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
            let outcome = check_upgrade_qualifications(&entries);
            checked(
                outcome.diagnostics,
                outcome.accepted.into_iter().flatten().collect(),
            )
        }
    }
}

fn nested_catalogue(bundle: &Map<String, Value>, key: &str) -> ResolutionCatalogue {
    object_member(bundle, key)
        .map(catalogue)
        .unwrap_or_default()
}

/// Builds every composed schema and catalogue of `bundle` once and hands
/// the route to `f` — the one composition `evaluate` checks every fixture
/// case against.
fn with_route<R>(
    bundle: &ContainerBundle,
    catalog: &TaskPatternCatalog,
    f: impl FnOnce(&Route<'_>) -> R,
) -> R {
    let envelope = composed(bundle, ENVELOPE);
    let route = Route {
        schemas: ContainerSchemas {
            registry: &bundle.schema,
            envelope,
            registry_not_object: "registrySchema is not an object; the upgrade-integration-qualification contract cannot be checked without its schema",
            envelope_not_object: "envelopeSchema is not an object; a qualification record composes with the record envelope and cannot be checked without it",
            entries_key: "qualifications",
            nested_records: false,
        },
        handoff: RecordSchemas {
            envelope,
            record: composed(bundle, HANDOFF),
            label: evidence_and_handoff::SCHEMA_LABEL,
        },
        field: RecordSchemas {
            envelope,
            record: composed(bundle, FIELD),
            label: field_evaluation::SCHEMA_LABEL,
        },
        execution: RecordSchemas {
            envelope,
            record: composed(bundle, EXECUTION),
            label: execution_state::SCHEMA_LABEL,
        },
        specification_schema: composed(bundle, SPECIFICATION),
        envelope,
        catalog,
        journeys: object_member(&bundle.bundle, "task_journey_resolution"),
        journey_resolution: nested_catalogue(&bundle.bundle, "task_journey_nested_resolution"),
        reports: object_member(&bundle.bundle, "field_evaluation_resolution"),
        report_resolution: nested_catalogue(&bundle.bundle, "field_evaluation_nested_resolution"),
        specifications: object_member(&bundle.bundle, "task_specification_resolution"),
        runs: object_member(&bundle.bundle, "execution_state_resolution"),
    };
    f(&route)
}

/// The whole `upgrade-integration-qualification` production route.
/// `catalog` is the task-pattern catalogue the caller already built
/// through `task_pattern_registry`'s own route.
pub fn evaluate(reader: &dyn WorkspaceReader, catalog: &TaskPatternCatalog) -> Outcome {
    let mut diagnostics = Vec::new();
    let Some(bundle) = load_container_bundle(reader, &FAMILY, &mut diagnostics) else {
        return Outcome { diagnostics };
    };
    with_route(&bundle, catalog, |route| {
        for case in &bundle.valid {
            let outcome = check_container(&case.registry, route);
            report_valid_case(outcome, &case.note, &PHRASES, &mut diagnostics);
        }
        for case in &bundle.invalid {
            let outcome = check_container(&case.registry, route);
            report_invalid_case(outcome, &case.note, &PHRASES, &mut diagnostics);
        }
    });
    Outcome { diagnostics }
}

#[cfg(test)]
mod tests;
