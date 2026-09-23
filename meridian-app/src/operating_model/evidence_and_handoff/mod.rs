//! `evidence-and-handoff-contract` — the app operation of the
//! `rust-architecture-conformance-6` route for the portable handoff of the
//! state and result of one execution run:
//!
//! ```text
//! WorkspaceReader
//!   -> evidence-and-handoff.schema.json + scoped-record.schema.json + fixture bundle
//!   -> bundle `resolution` map -> typed ResolutionCatalogue (super::record_resolution)
//!   -> schema gate -> private closed DTO (dto.rs)
//!   -> meridian_core::evidence::handoff::input::HandoffInput
//!   -> meridian_core::evidence::handoff::check_handoff(input, Some(&catalogue))
//!   -> Option<Handoff> + Vec<Diagnostic> (prefix-free)
//! ```
//!
//! Handoff DATA is Instance data; the Kernel ships the schema, the
//! product-neutral fixtures and this route. Every file is mandatory:
//! `ReadError::NotFound` keeps the Node reference's text and any other
//! `ReadError::Io` is reported as "… could not be read: …"
//! (`COMPATIBILITY.md`). `check_record` is the one-record entry point a
//! later composing family reuses; the fixture gate calls exactly it.

mod dto;

use meridian_core::evidence::handoff::{check_handoff, Handoff};
use meridian_core::run_contracts::ResolutionCatalogue;
use meridian_core::types::Diagnostic;
use serde_json::Value;

use super::run_contract_boundary::{
    evaluate_record, load_resolving_bundle, report_invalid_case, report_valid_case, CaseOutcome,
    CasePhrases, RecordSchemas, ResolvingFamily,
};
use crate::workspace::WorkspaceReader;

const FAMILY: ResolvingFamily = ResolvingFamily {
    schema_name: "evidence-and-handoff.schema.json",
    schema_path: "registries/operating-model/evidence-and-handoff.schema.json",
    fixtures_path: "registries/operating-model/fixtures/evidence-and-handoff.fixtures.json",
    missing_schema: "the evidence and handoff contract is a mandatory part of this Kernel, not an optional add-on",
    missing_envelope: "the handoff record composes with the record envelope and cannot be checked without it",
    missing_resolution: "the fixtures file carries no \"resolution\" object; the pinned execution-run, task-specification, run-human-control and context-manifest records AND every evidence entry are resolved OUTSIDE the handoff, and a bundle that resolves nothing cannot exercise the handoff against its actual run and evidence",
};

/// How the schema gate names this family's schema.
pub(crate) const SCHEMA_LABEL: &str = "evidence-and-handoff";

const PHRASES: CasePhrases = CasePhrases {
    valid_rejected: "a fixture that must be a valid handoff was rejected",
    invalid_clean: "a fixture that must be rejected validated clean",
    record: "handoff",
};

/// The operation's result: prefix-free diagnostics.
pub struct Outcome {
    pub diagnostics: Vec<Diagnostic>,
}

/// One already-parsed record through the whole route: schema gate, closed
/// DTO, typed input, the core check. `None` resolution fails closed in the
/// core.
pub(crate) fn check_record(
    doc: &Value,
    schemas: &RecordSchemas<'_>,
    resolution: Option<&ResolutionCatalogue>,
) -> CaseOutcome<Handoff> {
    evaluate_record(doc, schemas, dto::HandoffDto::into_input, |input| {
        check_handoff(input, resolution)
    })
}

/// The whole `evidence-and-handoff-contract` production route.
pub fn evaluate(reader: &dyn WorkspaceReader) -> Outcome {
    let mut diagnostics = Vec::new();
    let Some(bundle) = load_resolving_bundle(reader, &FAMILY, &mut diagnostics) else {
        return Outcome { diagnostics };
    };
    let schemas = RecordSchemas {
        envelope: &bundle.envelope,
        record: &bundle.schema,
        label: SCHEMA_LABEL,
    };
    for case in &bundle.valid {
        let outcome = check_record(&case.spec, &schemas, Some(&bundle.catalogue));
        report_valid_case(outcome, &case.note, &PHRASES, &mut diagnostics);
    }
    for case in &bundle.invalid {
        let outcome = check_record(&case.spec, &schemas, Some(&bundle.catalogue));
        report_invalid_case(outcome, &case.note, &PHRASES, &mut diagnostics);
    }
    Outcome { diagnostics }
}

#[cfg(test)]
mod tests;
