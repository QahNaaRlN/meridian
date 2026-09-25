//! `instruction-source-registry` — the composite-consistency algorithm
//! behind the `meridian validate` check of the same name (package 7,
//! subpackage 7b; typed by `rust-architecture-conformance-2`, following the
//! `controlled-rule-intake` pilot's own boundary sequence).
//! `registries/operating-model/instruction-source-registry.schema.json` is
//! the specialised payload schema for a registered instruction source.
//! Registry DATA is Instance, like the intake register — the Kernel ships
//! the schema, the product-neutral fixtures and this module.
//!
//! # Architecture (mirrors `controlled_rule_intake`)
//!
//! 1. **Syntax** — [`crate::source_format::json_schema::validate`] against
//!    the container/payload schema (whole document) and, separately, each
//!    entry against the record-envelope schema.
//! 2. **Transport** — `dto`: closed `serde` DTOs,
//!    `#[serde(deny_unknown_fields)]` on every object shape.
//! 3. **Domain validation and construction** — `convert::build_source`
//!    walks each `dto::EntryDto` into a
//!    [`meridian_core::instruction_source::InstructionSource`], accumulating
//!    every independent field defect before deciding success, then calls
//!    [`meridian_core::instruction_source::InstructionSource::try_new`] —
//!    the ONLY constructor, which itself enforces the
//!    `recorded_state`/`divergence` temporal coherence.
//! 4. **Pure predicate checks** — entirely in `meridian-core`
//!    ([`meridian_core::instruction_source`]): location/reference
//!    confinement, the read-channel coherence rule and the temporal
//!    coherence check all live on typed constructors there. None of these
//!    ever see a `Value` or a DTO.
//!
//! Unlike `controlled_rule_intake`, this contract does NOT gate domain
//! construction on the JSON Schema pass at all: an entry that fails the
//! container/payload or envelope schema still has `convert::build_source`
//! attempted on it, exactly matching the Node reference's own
//! `evaluateInstructionSourceRegistry`, which never short-circuits either
//! — `controlled-rule-intake`'s document-level short-circuit was a
//! `rust-architecture-conformance-1`-specific corrective decision, not a
//! rule this contract inherits.
//!
//! # Intentional difference from the Node reference
//!
//! The ONE place this module's per-entry diagnostics CAN fall short of
//! Node's: a `serde` transport-PARSE failure (`#[serde(deny_unknown_fields)]`
//! rejecting an unrecognised field, or a field whose JSON type does not
//! match the closed DTO shape — both stricter than anything the JSON Schema
//! subset [`crate::source_format::json_schema`] can express) skips
//! `convert::build_source` for that ONE entry entirely; Node's plain
//! property access has no equivalent "closed shape" concept and keeps
//! computing every business check over whatever fields it does recognise,
//! regardless of an extra one sitting alongside them. See `COMPATIBILITY.md`
//! and the matched Node/Rust library-level test pair (`test/conformance-harness.test.mjs`,
//! `meridian-app/src/operating_model/instruction_source_registry/mod.rs::tests::a_transport_parse_failure_skips_business_checks_for_that_entry_only`).
//!
//! [`convert_registry`] is the composition entry point
//! (`rust-architecture-conformance-2` §4):
//! `operating_model::existing_project_compatibility_mode` calls it for its
//! own embedded `discovered_sources`, reusing the SAME schema validation
//! and domain conversion this module runs for a top-level registry
//! document, rather than a second copy of either.

mod convert;
mod dto;

use std::collections::{BTreeMap, BTreeSet};

use meridian_core::instruction_source::InstructionSource;
use meridian_core::types::{Diagnostic, DiagnosticLevel};
use serde_json::Value;

use crate::source_format::json_schema;

pub(crate) fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message).expect("message is non-empty")
}

/// Runs `result`, pushing a `FAIL` diagnostic prefixed with `at` and
/// returning `None` on error rather than stopping the caller — the shared
/// accumulate-don't-fail-fast primitive every per-field conversion in
/// [`convert`] uses, mirroring `controlled_rule_intake`'s own `try_field`.
pub(crate) fn try_field<T, E: core::fmt::Display>(
    problems: &mut Vec<Diagnostic>,
    at: &str,
    result: Result<T, E>,
) -> Option<T> {
    match result {
        Ok(v) => Some(v),
        Err(e) => {
            problems.push(fail(format!("{at} {e}")));
            None
        }
    }
}

pub struct EvalSchemas<'a> {
    pub registry_schema: &'a Value,
    pub envelope_schema: &'a Value,
}

/// The result of converting one instruction-source-registry document: every
/// entry that fully converted, keyed by its declared id (deterministic
/// iteration via `BTreeMap`); the ids of entries that were PRESENT with an
/// id but failed schema or domain conversion (distinguishable from an id
/// simply absent from the document); and the full flat diagnostics list —
/// identical to what [`evaluate_instruction_source_registry`] returns.
pub struct ConvertedRegistry {
    pub sources: BTreeMap<String, InstructionSource>,
    pub malformed_ids: BTreeSet<String>,
    pub problems: Vec<Diagnostic>,
}

/// The whole composition pipeline for one registry document: container +
/// payload schema, per-entry envelope schema, transport parse and domain
/// construction per entry, then the cross-cutting rules the JSON Schema
/// subset cannot state (delegated to
/// [`meridian_core::instruction_source::InstructionSource::try_new`]).
pub fn convert_registry(doc: &Value, schemas: &EvalSchemas) -> ConvertedRegistry {
    let mut problems = Vec::new();

    match json_schema::validate(doc, schemas.registry_schema) {
        Ok(errors) => problems.extend(errors.into_iter().map(fail)),
        Err(error) => {
            return ConvertedRegistry {
                sources: BTreeMap::new(),
                malformed_ids: BTreeSet::new(),
                problems: vec![fail(format!(
                    "container/payload schema could not be applied: {error}"
                ))],
            }
        }
    }

    let empty = Vec::new();
    let entries: &Vec<Value> = doc
        .get("instruction_sources")
        .and_then(Value::as_array)
        .unwrap_or(&empty);

    for (i, entry) in entries.iter().enumerate() {
        match json_schema::validate(entry, schemas.envelope_schema) {
            Ok(errors) => problems.extend(
                errors
                    .into_iter()
                    .map(|m| fail(format!("entry {i} envelope {m}"))),
            ),
            Err(error) => problems.push(fail(format!(
                "entry {i} envelope could not be applied: {error}"
            ))),
        }
    }

    let mut seen_ids: BTreeSet<String> = BTreeSet::new();
    let mut sources: BTreeMap<String, InstructionSource> = BTreeMap::new();
    let mut malformed_ids: BTreeSet<String> = BTreeSet::new();

    for (index, entry_value) in entries.iter().enumerate() {
        if !entry_value.is_object() {
            continue;
        }
        let declared_id = entry_value.get("id").and_then(Value::as_str);
        let id_text = declared_id
            .map(str::to_string)
            .unwrap_or_else(|| format!("#{index}"));
        if let Some(declared_id) = declared_id {
            if !seen_ids.insert(declared_id.to_string()) {
                problems.push(fail(format!(
                    "instruction source id \"{declared_id}\" is declared more than once"
                )));
            }
        }

        match serde_json::from_value::<dto::EntryDto>(entry_value.clone()) {
            Ok(entry_dto) => match convert::build_source(&entry_dto) {
                Ok(source) => {
                    sources.insert(source.id().as_str().to_string(), source);
                }
                Err(errs) => {
                    problems.extend(errs);
                    malformed_ids.insert(id_text);
                }
            },
            Err(error) => {
                problems.push(fail(format!(
                    "instruction source \"{id_text}\" could not be parsed into the closed transport shape: {error}"
                )));
                malformed_ids.insert(id_text);
            }
        }
    }

    ConvertedRegistry {
        sources,
        malformed_ids,
        problems,
    }
}

pub fn evaluate_instruction_source_registry(doc: &Value, schemas: &EvalSchemas) -> Vec<Diagnostic> {
    convert_registry(doc, schemas).problems
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kernel_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }

    fn read_json(rel: &str) -> Value {
        serde_json::from_str(&std::fs::read_to_string(kernel_root().join(rel)).unwrap()).unwrap()
    }

    #[test]
    fn the_real_kernel_schema_and_fixtures_agree() {
        let registry_schema =
            read_json("registries/operating-model/instruction-source-registry.schema.json");
        let envelope_schema = read_json("registries/operating-model/scoped-record.schema.json");
        let fixtures = read_json(
            "registries/operating-model/fixtures/instruction-source-registry.fixtures.json",
        );
        let schemas = EvalSchemas {
            registry_schema: &registry_schema,
            envelope_schema: &envelope_schema,
        };
        for case in fixtures["valid"].as_array().unwrap() {
            let problems = evaluate_instruction_source_registry(&case["registry"], &schemas);
            assert!(
                problems.is_empty(),
                "note={:?} problems={:?}",
                case.get("note"),
                problems
            );
        }
        for case in fixtures["invalid"].as_array().unwrap() {
            let problems = evaluate_instruction_source_registry(&case["registry"], &schemas);
            assert!(!problems.is_empty(), "note={:?}", case.get("note"));
        }
    }

    /// Every real valid fixture also converts into a full typed
    /// [`InstructionSource`] set with no malformed entries — the
    /// composition path `existing_project_compatibility_mode` relies on.
    #[test]
    fn every_real_valid_fixture_converts_with_no_malformed_entries() {
        let registry_schema =
            read_json("registries/operating-model/instruction-source-registry.schema.json");
        let envelope_schema = read_json("registries/operating-model/scoped-record.schema.json");
        let fixtures = read_json(
            "registries/operating-model/fixtures/instruction-source-registry.fixtures.json",
        );
        let schemas = EvalSchemas {
            registry_schema: &registry_schema,
            envelope_schema: &envelope_schema,
        };
        for case in fixtures["valid"].as_array().unwrap() {
            let converted = convert_registry(&case["registry"], &schemas);
            assert!(
                converted.malformed_ids.is_empty(),
                "note={:?}",
                case.get("note")
            );
            let declared = case["registry"]["instruction_sources"]
                .as_array()
                .map(Vec::len)
                .unwrap_or(0);
            assert_eq!(
                converted.sources.len(),
                declared,
                "note={:?}",
                case.get("note")
            );
        }
    }

    /// A schema-invalid entry does not stop its SIBLING entries from being
    /// evaluated (the documented, intentional difference from
    /// `controlled-rule-intake`'s document-level short-circuit).
    #[test]
    fn a_schema_invalid_entry_does_not_block_evaluation_of_its_siblings() {
        let registry_schema =
            read_json("registries/operating-model/instruction-source-registry.schema.json");
        let envelope_schema = read_json("registries/operating-model/scoped-record.schema.json");
        let fixtures = read_json(
            "registries/operating-model/fixtures/instruction-source-registry.fixtures.json",
        );
        let schemas = EvalSchemas {
            registry_schema: &registry_schema,
            envelope_schema: &envelope_schema,
        };
        let mut valid_entry = fixtures["valid"][1]["registry"]["instruction_sources"][0].clone();
        assert!(
            valid_entry.is_object(),
            "expected fixtures[\"valid\"][1] to carry at least one instruction source"
        );
        valid_entry["id"] = Value::String("sibling-valid".to_string());

        let mut broken_entry = valid_entry.clone();
        broken_entry["id"] = Value::String("sibling-broken".to_string());
        broken_entry["payload"]["medium"] = Value::String("carrier-pigeon".to_string());

        let doc = serde_json::json!({
            "schema_version": 1,
            "registry_id": "instruction-source-registry",
            "title": "sibling test",
            "instruction_sources": [broken_entry, valid_entry],
        });
        let converted = convert_registry(&doc, &schemas);
        assert!(
            converted.malformed_ids.contains("sibling-broken"),
            "{:?}",
            converted.malformed_ids
        );
        assert!(
            converted.sources.contains_key("sibling-valid"),
            "{:?}",
            converted.sources.keys().collect::<Vec<_>>()
        );
    }

    /// A container-level schema-invalid document (missing the required
    /// top-level `registry_id`) creates NO domain objects at all — the
    /// container/payload schema failure is reported and conversion stops
    /// before any entry is even looked at.
    #[test]
    fn a_container_level_schema_invalid_document_creates_no_domain_objects() {
        let registry_schema =
            read_json("registries/operating-model/instruction-source-registry.schema.json");
        let envelope_schema = read_json("registries/operating-model/scoped-record.schema.json");
        let schemas = EvalSchemas {
            registry_schema: &registry_schema,
            envelope_schema: &envelope_schema,
        };
        let doc = serde_json::json!({
            "schema_version": 1,
            "title": "missing registry_id",
            "instruction_sources": [],
        });
        let converted = convert_registry(&doc, &schemas);
        assert!(!converted.problems.is_empty());
        assert!(converted.sources.is_empty());
        assert!(converted.malformed_ids.is_empty());
    }

    /// Repeated conversion of the same document produces byte-for-byte
    /// identical diagnostics — the diagnostic order is stable, not merely
    /// each individual entry's own checks.
    #[test]
    fn evaluation_of_the_same_document_is_deterministic() {
        let registry_schema =
            read_json("registries/operating-model/instruction-source-registry.schema.json");
        let envelope_schema = read_json("registries/operating-model/scoped-record.schema.json");
        let fixtures = read_json(
            "registries/operating-model/fixtures/instruction-source-registry.fixtures.json",
        );
        let schemas = EvalSchemas {
            registry_schema: &registry_schema,
            envelope_schema: &envelope_schema,
        };
        let doc = &fixtures["valid"][1]["registry"];
        let first = evaluate_instruction_source_registry(doc, &schemas);
        let second = evaluate_instruction_source_registry(doc, &schemas);
        assert_eq!(first, second);
    }

    /// The Rust half of the direct library-level Node/Rust matched pair
    /// documented in this module's own doc comment and `COMPATIBILITY.md`:
    /// an entry the closed transport DTO cannot parse (here, an
    /// unrecognised field alongside a SEPARATE read-channel coherence
    /// defect) reports ONLY the transport-parse diagnostic — the
    /// read-channel defect is never computed at all, because
    /// `convert::build_source` is never reached for this entry. The Node
    /// half (`test/conformance-harness.test.mjs`) uses the SAME real
    /// fixture and the SAME two mutations and asserts the opposite: BOTH
    /// diagnostics are present, because Node's plain property access has no
    /// equivalent closed-shape concept to reject the extra field on.
    #[test]
    fn a_transport_parse_failure_skips_business_checks_for_that_entry_only() {
        let registry_schema =
            read_json("registries/operating-model/instruction-source-registry.schema.json");
        let envelope_schema = read_json("registries/operating-model/scoped-record.schema.json");
        let fixtures = read_json(
            "registries/operating-model/fixtures/instruction-source-registry.fixtures.json",
        );
        let schemas = EvalSchemas {
            registry_schema: &registry_schema,
            envelope_schema: &envelope_schema,
        };
        let mut doc = fixtures["valid"][1]["registry"].clone();
        let entry = &mut doc["instruction_sources"][0];
        entry["payload"]["unexpected_field"] = Value::Bool(true);
        entry["payload"]["read_channel"]["agent_auto_read"] = Value::Bool(true);

        let problems = evaluate_instruction_source_registry(&doc, &schemas);
        assert!(
            problems.iter().any(|p| p
                .message()
                .contains("could not be parsed into the closed transport shape")),
            "{problems:?}"
        );
        assert!(
            !problems
                .iter()
                .any(|p| p.message().contains("agent_auto_read is true but kind is")),
            "the read-channel business diagnostic must NOT appear — domain construction is unreachable for this entry: {problems:?}"
        );
    }
}
