//! `existing-project-compatibility-mode` — the composite-consistency
//! algorithm behind the `meridian validate` check of the same name (package
//! 7, subpackage 7c; typed by `rust-architecture-conformance-2`, which
//! composes it over the typed `instruction_source_registry` and the
//! accepted `controlled_rule_intake` pilot rather than a third independent
//! `Value`-centric copy).
//! `registries/operating-model/existing-project-compatibility-mode.schema.json`
//! is the specialised payload schema for a workspace connection scan
//! (`record_type: workspace-connection-scan`) — the product-neutral
//! connection mode that discovers an existing project's instruction
//! sources through an explicit, bounded discovery plan without ever
//! writing to a file of the connected project.
//!
//! Composition, not a competing format: `discovered_sources` are FULL
//! instruction-source-registry entries, converted through the REAL
//! [`super::instruction_source_registry::convert_registry`] against the
//! REAL `instruction-source-registry.schema.json` the caller supplies —
//! never a second copy of that boundary or its checks;
//! `rule_candidates[].candidate` are FULL controlled-rule-intake
//! `rule-candidate` records, checked by calling the REAL
//! [`super::controlled_rule_intake::evaluate_controlled_rule_intake`]
//! against the REAL `controlled-rule-intake.schema.json` the caller
//! supplies, resolved through [`SourceResolution::Prefetched`] built ONCE
//! from this SAME scan's own already-typed `discovered_sources` — no
//! `serde_json::Value` round-trip, no resolver callback/closure standing in
//! for a port (`rust-architecture-conformance-2`, §4).
//!
//! This module's OWN typed concepts (workspace connection, discovery plan,
//! findings, next step) live in `domain`, in this crate rather than
//! `meridian-core` — see that module's own doc comment for why.
//!
//! `scanDiscoveryPlan` (the Node reference's filesystem-reading, read-only
//! directory scan) is deliberately NOT ported here — same reasoning as the
//! pre-typed version of this module: `evaluateExistingProjectCompatibilityMode`
//! never calls it, and porting a filesystem-dependent function into
//! `meridian-app` would need a port boundary this package does not need to
//! open for `validate` to compose real fixtures.
//!
//! # Intentional difference from the Node reference
//!
//! Same shape as `instruction_source_registry`'s own documented difference:
//! a connection entry the closed transport DTO cannot parse
//! (`#[serde(deny_unknown_fields)]`, or a field whose JSON type does not
//! match) skips `evaluate_connection` entirely for that ONE entry; Node's
//! plain property access has no equivalent "closed shape" concept and keeps
//! computing every business check regardless. See `COMPATIBILITY.md` and
//! the matched Node/Rust library-level test pair.

mod convert;
mod domain;
mod dto;

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde_json::Value;

use meridian_core::instruction_source::{DivergenceStatus, InstructionSource};
use meridian_core::types::{Diagnostic, DiagnosticLevel, SemanticId, WorkspaceId};

use crate::source_format::json_schema;

use super::controlled_rule_intake::{
    evaluate_controlled_rule_intake, EvalOpts as CriEvalOpts, ResolvedInstructionSourceDto,
    SourceResolution, SourceResolverError,
};
use super::instruction_source_registry::{convert_registry, EvalSchemas as IsrEvalSchemas};

pub use domain::{
    compute_next_step, ConnectionMode, DiscoveryPlanSlot, DiscoveryStatus, Finding, FindingKind,
    ManagedModeDecision, MissingSource, NextStep, PreviousKnowledge, RepositoryRef, ScanKind,
    UnreadableSource,
};

pub const RECORD_TYPE: &str = "workspace-connection-scan";
pub const REQUIRED_ORIGIN_KIND: &str = "declared";
pub const REQUIRED_AUTHORITY_KIND: &str = "delegated-run";

fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message).expect("message is non-empty")
}

fn is_object(v: &Value) -> bool {
    v.is_object()
}

fn json_stringify(v: Option<&Value>) -> String {
    match v {
        None => "undefined".to_string(),
        Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "undefined".to_string()),
    }
}

/// The canonical portable reference this module derives for a discovered
/// source, identical in shape to the Node reference's `deriveSourceReference`.
pub fn derive_source_reference(id: &str, revision: Option<&str>) -> String {
    format!(
        "instruction-source-registry:{id}@{}",
        revision.unwrap_or("undefined")
    )
}

/// Converts this scan's own successfully-converted discovered sources
/// (already [`InstructionSource`] values, produced by the REAL
/// `instruction_source_registry` boundary) into the typed lookup table
/// [`evaluate_controlled_rule_intake`] consumes directly via
/// `SourceResolution::Prefetched` — no `serde_json::Value` anywhere in this
/// function (`rust-architecture-conformance-2`, §4). Built ONCE, keyed by
/// SOURCE ID, the same id-first key the pilot's own `Prefetched` resolution
/// already looks entries up by. Every id present in `discovered_ids` gets
/// an entry: `Ok` for one that converted cleanly, `Err(Failed(..))` for one
/// that is present but failed its own instruction-source-registry
/// conversion — a malformed source is never silently dropped down to the
/// same `NotFound` a genuinely absent id produces.
pub fn build_resolved_sources(
    discovered_ids: &BTreeSet<String>,
    sources: &BTreeMap<String, InstructionSource>,
) -> HashMap<String, Result<ResolvedInstructionSourceDto, SourceResolverError>> {
    discovered_ids
        .iter()
        .map(|id| {
            let entry = match sources.get(id) {
                Some(source) => {
                    let revision = source.payload().recorded_state().revision().as_str();
                    let reference = derive_source_reference(id, Some(revision));
                    Ok(ResolvedInstructionSourceDto::from_typed(
                        source.id(),
                        &reference,
                        source.payload().recorded_state(),
                        source.payload().read_channel(),
                    ))
                }
                None => Err(SourceResolverError::Failed(
                    "discovered source is missing required payload/payload.recorded_state, so no resolved instruction source could be assembled".to_string(),
                )),
            };
            (id.clone(), entry)
        })
        .collect()
}

struct RegistrySchemaIdentitySpec {
    id: &'static str,
    registry_id: &'static str,
    entries_field: &'static str,
    entry_record_type: &'static str,
}

const REGISTRY_SCHEMA_SPEC: RegistrySchemaIdentitySpec = RegistrySchemaIdentitySpec {
    id: "https://meridian.invalid/registries/operating-model/existing-project-compatibility-mode.schema.json",
    registry_id: "existing-project-compatibility-mode",
    entries_field: "workspace_connections",
    entry_record_type: RECORD_TYPE,
};
const SOURCE_REGISTRY_SCHEMA_SPEC: RegistrySchemaIdentitySpec = RegistrySchemaIdentitySpec {
    id: "https://meridian.invalid/registries/operating-model/instruction-source-registry.schema.json",
    registry_id: "instruction-source-registry",
    entries_field: "instruction_sources",
    entry_record_type: "instruction-source",
};
const RULE_INTAKE_SCHEMA_SPEC: RegistrySchemaIdentitySpec = RegistrySchemaIdentitySpec {
    id: "https://meridian.invalid/registries/operating-model/controlled-rule-intake.schema.json",
    registry_id: "controlled-rule-intake",
    entries_field: "rule_candidates",
    entry_record_type: "rule-candidate",
};

const ENVELOPE_SCHEMA_ID: &str =
    "https://meridian.invalid/registries/operating-model/scoped-record.schema.json";
const ENVELOPE_REQUIRED_FIELDS: [&str; 8] = [
    "schema_version",
    "id",
    "title",
    "record_type",
    "scope",
    "origin",
    "authority",
    "payload",
];
const ENVELOPE_STRUCTURAL_PROPERTIES: [&str; 5] =
    ["record_type", "scope", "origin", "authority", "payload"];

/// A composed dependency schema must be an object, ENTIRELY supported by
/// the validation engine, and the EXPECTED contract — identity is checked
/// on top, never inferred from mere applicability. This checks the SCHEMA
/// DOCUMENT itself, which is inherently JSON, not the data it validates —
/// the ongoing `Value` use here is the "general JSON/Schema adapter"
/// exception `rust-migration-quality.md` §4 names, not a lapsed boundary.
fn check_registry_schema_identity(
    schema: Option<&Value>,
    label: &str,
    spec: &RegistrySchemaIdentitySpec,
) -> Option<String> {
    let schema = schema.filter(|v| is_object(v))?;
    if let Err(error) = json_schema::assert_supported_deep(schema, label) {
        return Some(format!(
            "{label} is not fully supported by the validation engine: {error}"
        ));
    }
    let schema_id = schema.get("$id").and_then(Value::as_str);
    if schema_id != Some(spec.id) {
        return Some(format!(
            "{label} has $id {}, not the expected \"{}\"; this is not the {} contract schema ({{}} and a foreign registry's schema are both rejected here)",
            json_stringify(schema.get("$id")),
            spec.id,
            spec.registry_id
        ));
    }
    let registry_id_const = schema.pointer("/properties/registry_id/const");
    if registry_id_const.and_then(Value::as_str) != Some(spec.registry_id) {
        return Some(format!(
            "{label} properties.registry_id.const is {}, not \"{}\"",
            json_stringify(registry_id_const),
            spec.registry_id
        ));
    }
    let entries_prop = schema.pointer(&format!("/properties/{}", spec.entries_field));
    let entries_ok = entries_prop.is_some_and(|p| {
        is_object(p)
            && p.get("type").and_then(Value::as_str) == Some("array")
            && p.pointer("/items/$ref").and_then(Value::as_str) == Some("#/definitions/entry")
    });
    if !entries_ok {
        return Some(format!(
            "{label} properties.{} is not declared as an array of \"#/definitions/entry\"; this is not the {} contract's entries array",
            spec.entries_field, spec.registry_id
        ));
    }
    let entry_record_type_const = schema.pointer("/definitions/entry/properties/record_type/const");
    if entry_record_type_const.and_then(Value::as_str) != Some(spec.entry_record_type) {
        return Some(format!(
            "{label} definitions.entry.properties.record_type.const is {}, not \"{}\"; this is not the {} contract's entry definition",
            json_stringify(entry_record_type_const),
            spec.entry_record_type,
            spec.registry_id
        ));
    }
    None
}

fn check_envelope_schema_identity(schema: Option<&Value>, label: &str) -> Option<String> {
    let schema = schema.filter(|v| is_object(v))?;
    if let Err(error) = json_schema::assert_supported_deep(schema, label) {
        return Some(format!(
            "{label} is not fully supported by the validation engine: {error}"
        ));
    }
    let schema_id = schema.get("$id").and_then(Value::as_str);
    if schema_id != Some(ENVELOPE_SCHEMA_ID) {
        return Some(format!(
            "{label} has $id {}, not the expected \"{ENVELOPE_SCHEMA_ID}\"; this is not the record-envelope contract schema ({{}} and a foreign schema are both rejected here)",
            json_stringify(schema.get("$id"))
        ));
    }
    let required: BTreeSet<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let missing_required: Vec<&str> = ENVELOPE_REQUIRED_FIELDS
        .into_iter()
        .filter(|f| !required.contains(f))
        .collect();
    if !missing_required.is_empty() {
        return Some(format!(
            "{label} required is missing {}; this is not the full record-envelope contract",
            missing_required.join(", ")
        ));
    }
    let missing_props: Vec<&str> = ENVELOPE_STRUCTURAL_PROPERTIES
        .into_iter()
        .filter(|f| {
            !schema
                .pointer(&format!("/properties/{f}"))
                .is_some_and(is_object)
        })
        .collect();
    if !missing_props.is_empty() {
        return Some(format!(
            "{label} properties is missing {}; this is not the full record-envelope contract",
            missing_props.join(", ")
        ));
    }
    None
}

pub struct EvalOpts<'a> {
    pub registry_schema: &'a Value,
    pub envelope_schema: &'a Value,
    pub source_registry_schema: &'a Value,
    pub rule_intake_schema: &'a Value,
}

/// The whole composition pipeline for one existing-project-compatibility-mode
/// document.
pub fn evaluate_existing_project_compatibility_mode(
    doc: &Value,
    opts: &EvalOpts,
) -> Vec<Diagnostic> {
    let registry_failure = check_registry_schema_identity(
        Some(opts.registry_schema),
        "registrySchema",
        &REGISTRY_SCHEMA_SPEC,
    );
    let envelope_failure =
        check_envelope_schema_identity(Some(opts.envelope_schema), "envelopeSchema");
    let source_registry_failure = check_registry_schema_identity(
        Some(opts.source_registry_schema),
        "sourceRegistrySchema",
        &SOURCE_REGISTRY_SCHEMA_SPEC,
    );
    let rule_intake_failure = check_registry_schema_identity(
        Some(opts.rule_intake_schema),
        "ruleIntakeSchema",
        &RULE_INTAKE_SCHEMA_SPEC,
    );
    let failing: Vec<(&str, &Option<String>)> = [
        ("registrySchema", &registry_failure),
        ("envelopeSchema", &envelope_failure),
        ("sourceRegistrySchema", &source_registry_failure),
        ("ruleIntakeSchema", &rule_intake_failure),
    ]
    .into_iter()
    .filter(|(_, f)| f.is_some())
    .collect();
    if !failing.is_empty() {
        let failing_keys: Vec<&str> = failing.iter().map(|(k, _)| *k).collect();
        let detail: Vec<&str> = failing.iter().map(|(_, f)| f.as_deref().unwrap()).collect();
        return vec![fail(format!(
            "existing-project-compatibility-mode composition requires the real {} contract schema(s); discovered_sources compose with the instruction-source-registry contract and rule_candidates compose with the controlled-rule-intake contract regardless of whether either array is populated in this document, and an absent, inapplicable, unsupported or wrong-contract ({{}} included) schema is rejected closed, never silently skipped: {}",
            failing_keys.join(", "),
            detail.join("; ")
        ))];
    }

    let mut problems = Vec::new();

    match json_schema::validate(doc, opts.registry_schema) {
        Ok(errors) => problems.extend(errors.into_iter().map(fail)),
        Err(error) => {
            return vec![fail(format!(
                "container/payload schema could not be applied: {error}"
            ))]
        }
    }

    let empty = Vec::new();
    let entries = doc
        .get("workspace_connections")
        .and_then(Value::as_array)
        .unwrap_or(&empty);

    for (i, entry) in entries.iter().enumerate() {
        match json_schema::validate(entry, opts.envelope_schema) {
            Ok(errors) => problems.extend(
                errors
                    .into_iter()
                    .map(|m| fail(format!("entry {i} envelope {m}"))),
            ),
            Err(error) => {
                problems.push(fail(format!(
                    "entry {i} envelope could not be applied: {error}"
                )));
            }
        }
    }

    let mut seen_ids: BTreeSet<String> = BTreeSet::new();
    for (i, entry_value) in entries.iter().enumerate() {
        if !is_object(entry_value) {
            continue;
        }
        let declared_id = entry_value.get("id").and_then(Value::as_str);
        let id_text = declared_id
            .map(str::to_string)
            .unwrap_or_else(|| format!("#{i}"));
        let at = format!("workspace connection scan \"{id_text}\"");
        if let Some(declared_id) = declared_id {
            if !seen_ids.insert(declared_id.to_string()) {
                problems.push(fail(format!("{at} is declared more than once")));
            }
        }

        let entry_dto: dto::ConnectionEntryDto = match serde_json::from_value(entry_value.clone()) {
            Ok(v) => v,
            Err(error) => {
                problems.push(fail(format!(
                    "{at} could not be parsed into the closed transport shape: {error}"
                )));
                continue;
            }
        };
        if entry_dto.record_type != RECORD_TYPE {
            problems.push(fail(format!(
                "{at} declares record_type \"{}\", not \"{RECORD_TYPE}\"",
                entry_dto.record_type
            )));
        }

        evaluate_connection(&at, &entry_dto, opts, &mut problems);
    }

    problems
}

fn evaluate_connection(
    at: &str,
    entry: &dto::ConnectionEntryDto,
    opts: &EvalOpts,
    problems: &mut Vec<Diagnostic>,
) {
    let payload = &entry.payload;

    if entry.origin.kind != REQUIRED_ORIGIN_KIND {
        problems.push(fail(format!(
            "{at} origin.kind is \"{}\", not \"{REQUIRED_ORIGIN_KIND}\"; a scan is an explicit declared act of connecting a project",
            entry.origin.kind
        )));
    }
    if entry.authority.kind != REQUIRED_AUTHORITY_KIND {
        problems.push(fail(format!(
            "{at} authority.kind is \"{}\", not \"{REQUIRED_AUTHORITY_KIND}\"; the scan itself is carried out by a run's delegated authority and never mints an owner decision",
            entry.authority.kind
        )));
    }

    let scope_id = match SemanticId::new(&entry.scope.id) {
        Ok(v) => Some(v),
        Err(e) => {
            problems.push(fail(format!("{at} scope.id: {e}")));
            None
        }
    };
    let scope_workspace_id = match &entry.scope.workspace_id {
        Some(w) => match WorkspaceId::new(w) {
            Ok(v) => Some(v),
            Err(e) => {
                problems.push(fail(format!("{at} scope.workspace_id: {e}")));
                None
            }
        },
        None => None,
    };
    let scope = scope_id.and_then(|scope_id| {
        match domain::build_connection_scope(
            at,
            &entry.scope.scope_type,
            scope_id,
            scope_workspace_id,
        ) {
            Ok(v) => Some(v),
            Err(errs) => {
                problems.extend(errs);
                None
            }
        }
    });

    let repository = match convert::build_repository_ref(at, &payload.repository) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    if let (Some(scope), Some(repository)) = (&scope, &repository) {
        problems.extend(domain::check_repository_scope(at, scope, repository));
    }

    let connection_mode = match domain::ConnectionMode::parse(&payload.connection_mode) {
        Some(m) => Some(m),
        None => {
            problems.push(fail(format!(
                "{at} connection_mode \"{}\" is not one of the two closed values",
                payload.connection_mode
            )));
            None
        }
    };
    let managed_mode_decision = match &payload.managed_mode_decision {
        Some(dto) => match convert::build_managed_mode_decision(at, dto) {
            Ok(v) => Some(v),
            Err(errs) => {
                problems.extend(errs);
                None
            }
        },
        None => None,
    };
    if let (Some(connection_mode), Some(scope)) = (connection_mode, &scope) {
        problems.extend(domain::check_managed_mode(
            at,
            connection_mode,
            managed_mode_decision.as_ref(),
            scope,
        ));
    }

    let scan_kind = match domain::ScanKind::parse(&payload.scan_kind) {
        Some(k) => Some(k),
        None => {
            problems.push(fail(format!(
                "{at} scan_kind \"{}\" is not one of the two closed values",
                payload.scan_kind
            )));
            None
        }
    };

    // Property 3: bounded discovery plan.
    //
    // Corrective round (`CHANGES_REQUESTED` items 3-4): `declared_plan_id_strings`
    // and `plan_ids` are built from EVERY slot's own declared id, regardless
    // of whether the REST of that slot (its `location`) also validates — a
    // slot with a valid id but a malformed `location` must still occupy a
    // discovery-plan outcome slot; an earlier version built `plan_ids` (then
    // implicitly, from `plan: Vec<DiscoveryPlanSlot>`) only from FULLY valid
    // slots, so a slot whose location failed vanished from outcome-partition
    // checking entirely, hiding a real "no outcome" defect alongside the
    // location defect that was still reported. `plan` (fully valid slots
    // only) is kept separately for location-matching, which genuinely needs
    // the typed `Location`, not just the id.
    let mut declared_plan_id_strings: BTreeSet<String> = BTreeSet::new();
    let mut plan_ids: BTreeSet<SemanticId> = BTreeSet::new();
    let mut plan: Vec<domain::DiscoveryPlanSlot> = Vec::new();
    for slot_dto in &payload.discovery_plan {
        if !declared_plan_id_strings.insert(slot_dto.id.clone()) {
            problems.push(fail(format!(
                "{at} discovery_plan slot id \"{}\" is declared more than once",
                slot_dto.id
            )));
        }
        if let Ok(id) = SemanticId::new(&slot_dto.id) {
            plan_ids.insert(id);
        }
        match convert::build_plan_slot(at, slot_dto) {
            Ok(slot) => plan.push(slot),
            Err(errs) => problems.extend(errs),
        }
    }
    let plan_by_id: BTreeMap<&str, &domain::DiscoveryPlanSlot> =
        plan.iter().map(|s| (s.id().as_str(), s)).collect();

    // Property 4: discovered sources compose with the ACTUAL
    // instruction-source-registry contract.
    let discovered_container = serde_json::json!({
        "schema_version": 1,
        "registry_id": "instruction-source-registry",
        "title": format!("{at} — discovered sources"),
        "instruction_sources": payload.discovered_sources,
    });
    let converted_sources = if payload.discovered_sources.is_empty() {
        None
    } else {
        let src_schemas = IsrEvalSchemas {
            registry_schema: opts.source_registry_schema,
            envelope_schema: opts.envelope_schema,
        };
        Some(convert_registry(&discovered_container, &src_schemas))
    };
    let (sources, discovered_ids): (BTreeMap<String, InstructionSource>, BTreeSet<String>) =
        match &converted_sources {
            Some(converted) => {
                problems.extend(
                    converted
                        .problems
                        .iter()
                        .map(|d| fail(format!("{at} discovered source: {}", d.message()))),
                );
                let discovered_ids: BTreeSet<String> = payload
                    .discovered_sources
                    .iter()
                    .filter_map(|s| s.get("id").and_then(Value::as_str))
                    .map(String::from)
                    .collect();
                (converted.sources.clone(), discovered_ids)
            }
            None => (BTreeMap::new(), BTreeSet::new()),
        };

    // Corrective round (`CHANGES_REQUESTED` items 2-4): `discovered_occurrences`
    // is a MULTISET — one entry per `discovered_sources` array element that
    // carried a syntactically valid id, collected UNCONDITIONALLY (before,
    // and independent of, the "corresponds to a declared plan slot" and
    // "converted cleanly" checks below). An earlier version built
    // `discovered_ids` as a `BTreeSet<String>`, which silently collapsed two
    // `discovered_sources` entries naming the SAME plan id into one outcome
    // instead of two before `check_discovery_outcomes` ever saw them. The
    // "does this id correspond to a declared slot" check now also uses the
    // raw `declared_plan_id_strings` set directly, not `plan_by_id` (which
    // only holds slots whose OWN location also validated) — so a discovered
    // source naming a slot whose location is separately broken is still
    // recognised as corresponding to a declared slot, not misreported as
    // "does not correspond".
    let mut discovered_occurrences: Vec<SemanticId> = Vec::new();
    let mut discovered_changed_ids: BTreeSet<String> = BTreeSet::new();
    for src in &payload.discovered_sources {
        let Some(sid) = src.get("id").and_then(Value::as_str) else {
            problems.push(fail(format!(
                "{at} discovered source {} carries no id",
                json_stringify(Some(src))
            )));
            continue;
        };
        if let Ok(id) = SemanticId::new(sid) {
            discovered_occurrences.push(id);
        }
        if !declared_plan_id_strings.contains(sid) {
            problems.push(fail(format!(
                "{at} discovered source \"{sid}\" does not correspond to any declared discovery_plan slot; discovery is bounded to the declared plan and never guesses beyond it"
            )));
            continue;
        }
        // Location match is only possible when the corresponding plan
        // slot's OWN domain construction succeeded (its own defect, if any,
        // is already reported separately by `convert::build_plan_slot`) AND
        // this discovered source itself converted cleanly.
        if let (Some(slot), Some(source)) = (plan_by_id.get(sid), sources.get(sid)) {
            if source.payload().location() != slot.location() {
                problems.push(fail(format!(
                    "{at} discovered source \"{sid}\" location does not match its declared discovery_plan slot \"{sid}\""
                )));
            }
        }
        let Some(source) = sources.get(sid) else {
            // Malformed (already diagnosed above) — nothing further to
            // compare here, but its occurrence was still counted above.
            continue;
        };
        let status = source.payload().divergence().status();
        if status.is_source_gone() {
            problems.push(fail(format!(
                "{at} discovered source \"{sid}\" declares divergence.status \"{status}\"; a source currently missing or unreadable belongs in missing_sources/unreadable_sources, not discovered_sources"
            )));
        } else if scan_kind == Some(domain::ScanKind::Initial)
            && status != DivergenceStatus::Unknown
        {
            problems.push(fail(format!(
                "{at} discovered source \"{sid}\" declares divergence.status \"{status}\" on an initial scan, which has no prior baseline to compare against; an initial scan's divergence is always \"unknown\""
            )));
        }
        if status == DivergenceStatus::Changed {
            discovered_changed_ids.insert(sid.to_string());
        }
    }

    // Property 7 (bucket separation + previously_known gate).
    //
    // Corrective round (items 3-4): `missing_occurrences`/`unreadable_occurrences`
    // collect each entry's `plan_id` UNCONDITIONALLY — before, and
    // independent of, `convert::build_missing_source`/`build_unreadable_source`
    // succeeding. An entry whose `previously_known`/`id` combination (or, for
    // unreadable sources, `reason`) is malformed still occupies exactly one
    // discovery-plan outcome slot; an earlier version only recorded an
    // outcome for entries that converted cleanly into `missing`/`unreadable`,
    // so a malformed entry's own defect was reported but its outcome
    // silently vanished from partition checking.
    let mut missing: Vec<domain::MissingSource> = Vec::new();
    let mut missing_occurrences: Vec<SemanticId> = Vec::new();
    for m_dto in &payload.missing_sources {
        if !declared_plan_id_strings.contains(&m_dto.plan_id) {
            problems.push(fail(format!(
                "{at} missing_sources plan_id \"{}\" does not correspond to any declared discovery_plan slot",
                m_dto.plan_id
            )));
        }
        if let Ok(id) = SemanticId::new(&m_dto.plan_id) {
            missing_occurrences.push(id);
        }
        match convert::build_missing_source(at, m_dto) {
            Ok(m) => {
                if scan_kind == Some(domain::ScanKind::Initial)
                    && matches!(
                        m.knowledge(),
                        domain::PreviousKnowledge::PreviouslyKnown { .. }
                    )
                {
                    problems.push(fail(format!(
                        "{at} missing_sources plan_id \"{}\" asserts previously_known on an initial scan, which has no prior scan to know from",
                        m.plan_id()
                    )));
                }
                missing.push(m);
            }
            Err(errs) => problems.extend(errs),
        }
    }
    let mut unreadable: Vec<domain::UnreadableSource> = Vec::new();
    let mut unreadable_occurrences: Vec<SemanticId> = Vec::new();
    for u_dto in &payload.unreadable_sources {
        if !declared_plan_id_strings.contains(&u_dto.plan_id) {
            problems.push(fail(format!(
                "{at} unreadable_sources plan_id \"{}\" does not correspond to any declared discovery_plan slot",
                u_dto.plan_id
            )));
        }
        if let Ok(id) = SemanticId::new(&u_dto.plan_id) {
            unreadable_occurrences.push(id);
        }
        match convert::build_unreadable_source(at, u_dto) {
            Ok(u) => {
                if scan_kind == Some(domain::ScanKind::Initial)
                    && matches!(
                        u.knowledge(),
                        domain::PreviousKnowledge::PreviouslyKnown { .. }
                    )
                {
                    problems.push(fail(format!(
                        "{at} unreadable_sources plan_id \"{}\" asserts previously_known on an initial scan, which has no prior scan to know from",
                        u.plan_id()
                    )));
                }
                unreadable.push(u);
            }
            Err(errs) => problems.extend(errs),
        }
    }

    problems.extend(domain::check_discovery_outcomes(
        at,
        &plan_ids,
        &discovered_occurrences,
        &missing_occurrences,
        &unreadable_occurrences,
    ));

    let mut findings: Vec<domain::Finding> = Vec::new();
    for f_dto in &payload.findings {
        match convert::build_finding(at, f_dto) {
            Ok(f) => findings.push(f),
            Err(errs) => problems.extend(errs),
        }
    }

    problems.extend(domain::check_changes_have_findings(
        at,
        &discovered_changed_ids,
        &missing,
        &unreadable,
        &findings,
    ));

    let expected_next_step = domain::compute_next_step(&findings);
    match domain::NextStep::parse(&payload.next_step) {
        Some(declared) if declared == expected_next_step => {}
        _ => problems.push(fail(format!(
            "{at} next_step is \"{}\", but the declared priority (a blocking \"conflict\" finding requires \"resolve-conflict\"; else a blocking \"ambiguous-scope\" finding requires \"resolve-ambiguity\"; else any other blocking finding requires \"await-owner-decision\"; else \"continue-compatibility-mode\") computes \"{expected_next_step}\"",
            payload.next_step
        ))),
    }

    // Property 5: discovery mints no decision. Rule candidates compose with
    // the ACTUAL controlled-rule-intake contract, resolved through a
    // boundary built from this SAME scan's already-typed discovered
    // sources.
    let candidate_entries: Vec<Value> = payload
        .rule_candidates
        .iter()
        .map(|w| w.candidate.clone())
        .collect();
    if !candidate_entries.is_empty() {
        let container = serde_json::json!({
            "schema_version": 1,
            "registry_id": "controlled-rule-intake",
            "title": format!("{at} — rule candidates"),
            "rule_candidates": candidate_entries,
        });
        let resolved_sources = build_resolved_sources(&discovered_ids, &sources);
        let cri_opts = CriEvalOpts {
            registry_schema: opts.rule_intake_schema,
            envelope_schema: opts.envelope_schema,
            resolve_source: SourceResolution::Prefetched(&resolved_sources),
        };
        let cri_problems = evaluate_controlled_rule_intake(&container, &cri_opts);
        problems.extend(
            cri_problems
                .into_iter()
                .map(|p| fail(format!("{at} rule candidate: {}", p.message()))),
        );
    }
    for w in &payload.rule_candidates {
        let Some(discovery_status) = domain::DiscoveryStatus::parse(&w.discovery_status) else {
            problems.push(fail(format!(
                "{at} rule candidate discovery_status \"{}\" is not one of the two closed values",
                w.discovery_status
            )));
            continue;
        };
        if discovery_status != domain::DiscoveryStatus::New {
            continue;
        }
        // A narrow transport-level peek at ONE field of the embedded
        // candidate, not a duplicated business rule: the candidate's own
        // full validity (including this very field's closed pool) is
        // already checked by the real `evaluate_controlled_rule_intake`
        // call above; this only asks whether a "new" discovery ever
        // claims a decided state at all.
        let cid = w.candidate.get("id").and_then(Value::as_str).unwrap_or("?");
        let state = w
            .candidate
            .get("payload")
            .and_then(|p| p.get("applicability_state"))
            .and_then(Value::as_str);
        if state != Some("candidate") {
            problems.push(fail(format!(
                "{at} rule candidate \"{cid}\" has discovery_status \"new\" but applicability_state {}; a candidate first found by THIS scan can never itself be accepted or not-applicable (property 5)",
                json_stringify(state.map(|s| Value::String(s.to_string())).as_ref())
            )));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluate_on_missing_schema_identity_fails_closed_without_panicking() {
        let empty = serde_json::json!({});
        let opts = EvalOpts {
            registry_schema: &empty,
            envelope_schema: &empty,
            source_registry_schema: &empty,
            rule_intake_schema: &empty,
        };
        let problems = evaluate_existing_project_compatibility_mode(&Value::Null, &opts);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].message().contains("registrySchema"),
            "{problems:?}"
        );
        assert!(
            problems[0].message().contains("envelopeSchema"),
            "{problems:?}"
        );
        assert!(
            problems[0].message().contains("sourceRegistrySchema"),
            "{problems:?}"
        );
        assert!(
            problems[0].message().contains("ruleIntakeSchema"),
            "{problems:?}"
        );
    }

    #[test]
    fn compute_next_step_follows_the_closed_priority_regardless_of_finding_order() {
        let conflict = domain::Finding::new(domain::FindingKind::Conflict, None, true);
        let ambiguous = domain::Finding::new(domain::FindingKind::AmbiguousScope, None, true);
        let other = domain::Finding::new(domain::FindingKind::Other, None, true);
        assert_eq!(
            compute_next_step(&[other.clone(), ambiguous.clone(), conflict.clone()]),
            domain::NextStep::ResolveConflict
        );
        assert_eq!(
            compute_next_step(&[other.clone(), ambiguous]),
            domain::NextStep::ResolveAmbiguity
        );
        assert_eq!(
            compute_next_step(&[other]),
            domain::NextStep::AwaitOwnerDecision
        );
        assert_eq!(
            compute_next_step(&[]),
            domain::NextStep::ContinueCompatibilityMode
        );
    }

    #[test]
    fn build_resolved_sources_marks_an_absent_source_as_failed_not_panicking() {
        let discovered_ids: BTreeSet<String> = ["missing-source".to_string()].into_iter().collect();
        let sources = BTreeMap::new();
        let table = build_resolved_sources(&discovered_ids, &sources);
        assert!(matches!(
            table.get("missing-source"),
            Some(Err(SourceResolverError::Failed(_)))
        ));
    }

    /// A malformed source PRESENT among a scan's discovered ids (its own
    /// data failed instruction-source-registry conversion) must stay
    /// distinguishable from a source simply ABSENT from `discovered_ids`
    /// altogether — the same `Failed` vs `NotFound`/absent-key distinction
    /// `controlled_rule_intake`'s own `SourceResolution::Prefetched`
    /// preserves one layer up.
    #[test]
    fn a_malformed_discovered_source_is_distinguished_from_one_never_discovered_at_all() {
        let discovered_ids: BTreeSet<String> = ["malformed-src".to_string()].into_iter().collect();
        let sources: BTreeMap<String, InstructionSource> = BTreeMap::new();
        let table = build_resolved_sources(&discovered_ids, &sources);
        // Present in discovered_ids, absent from `sources`: Failed.
        assert!(matches!(
            table.get("malformed-src"),
            Some(Err(SourceResolverError::Failed(_)))
        ));
        // Never in discovered_ids at all: absent from the table entirely —
        // `SourceResolution::Prefetched`'s own lookup then reports
        // `NotFound`, a third, still-distinguishable outcome.
        assert!(!table.contains_key("never-discovered"));
    }

    fn kernel_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }

    fn read_json(rel: &str) -> Value {
        serde_json::from_str(&std::fs::read_to_string(kernel_root().join(rel)).unwrap()).unwrap()
    }

    fn real_eval_opts() -> (Value, Value, Value, Value) {
        (
            read_json("registries/operating-model/existing-project-compatibility-mode.schema.json"),
            read_json("registries/operating-model/scoped-record.schema.json"),
            read_json("registries/operating-model/instruction-source-registry.schema.json"),
            read_json("registries/operating-model/controlled-rule-intake.schema.json"),
        )
    }

    /// Real valid/invalid fixtures agree at the `meridian-app` level, not
    /// only through the CLI adapter — mirrors
    /// `instruction_source_registry`'s own app-level fixture test.
    #[test]
    fn the_real_kernel_schema_and_fixtures_agree() {
        let (registry_schema, envelope_schema, source_registry_schema, rule_intake_schema) =
            real_eval_opts();
        let fixtures = read_json(
            "registries/operating-model/fixtures/existing-project-compatibility-mode.fixtures.json",
        );
        let opts = EvalOpts {
            registry_schema: &registry_schema,
            envelope_schema: &envelope_schema,
            source_registry_schema: &source_registry_schema,
            rule_intake_schema: &rule_intake_schema,
        };
        for case in fixtures["valid"].as_array().unwrap() {
            let problems = evaluate_existing_project_compatibility_mode(&case["registry"], &opts);
            assert!(
                problems.is_empty(),
                "note={:?} problems={:?}",
                case.get("note"),
                problems
            );
        }
        for case in fixtures["invalid"].as_array().unwrap() {
            let problems = evaluate_existing_project_compatibility_mode(&case["registry"], &opts);
            assert!(!problems.is_empty(), "note={:?}", case.get("note"));
        }
    }

    /// Repeated evaluation of the same document produces byte-for-byte
    /// identical diagnostics — determinism end to end, not merely per
    /// individual check.
    #[test]
    fn evaluation_of_the_same_document_is_deterministic() {
        let (registry_schema, envelope_schema, source_registry_schema, rule_intake_schema) =
            real_eval_opts();
        let fixtures = read_json(
            "registries/operating-model/fixtures/existing-project-compatibility-mode.fixtures.json",
        );
        let opts = EvalOpts {
            registry_schema: &registry_schema,
            envelope_schema: &envelope_schema,
            source_registry_schema: &source_registry_schema,
            rule_intake_schema: &rule_intake_schema,
        };
        let doc = &fixtures["valid"][1]["registry"];
        let first = evaluate_existing_project_compatibility_mode(doc, &opts);
        let second = evaluate_existing_project_compatibility_mode(doc, &opts);
        assert_eq!(first, second);
    }

    /// A connection entry the closed transport DTO cannot parse (here: a
    /// `next_step` value of the wrong JSON type, which the loose container
    /// schema does not itself catch) is reported as ONE diagnostic and does
    /// NOT reach `evaluate_connection` — domain construction is unreachable
    /// for it, not merely unattempted.
    #[test]
    fn a_transport_invalid_connection_does_not_reach_domain_construction() {
        let (registry_schema, envelope_schema, source_registry_schema, rule_intake_schema) =
            real_eval_opts();
        let fixtures = read_json(
            "registries/operating-model/fixtures/existing-project-compatibility-mode.fixtures.json",
        );
        let opts = EvalOpts {
            registry_schema: &registry_schema,
            envelope_schema: &envelope_schema,
            source_registry_schema: &source_registry_schema,
            rule_intake_schema: &rule_intake_schema,
        };
        let mut doc = fixtures["valid"][1]["registry"].clone();
        doc["workspace_connections"][0]["payload"]["next_step"] = serde_json::json!(42);
        let problems = evaluate_existing_project_compatibility_mode(&doc, &opts);
        assert!(
            problems.iter().any(|p| p
                .message()
                .contains("could not be parsed into the closed transport shape")),
            "{problems:?}"
        );
    }

    /// A discovered source's own malformed data is reported through the
    /// REAL `instruction_source_registry` composition — the diagnostic
    /// text is one only that boundary produces, proving this is real
    /// composition, not a stub.
    #[test]
    fn a_malformed_embedded_discovered_source_is_reported_through_the_real_instruction_source_registry_path(
    ) {
        let (registry_schema, envelope_schema, source_registry_schema, rule_intake_schema) =
            real_eval_opts();
        let fixtures = read_json(
            "registries/operating-model/fixtures/existing-project-compatibility-mode.fixtures.json",
        );
        let opts = EvalOpts {
            registry_schema: &registry_schema,
            envelope_schema: &envelope_schema,
            source_registry_schema: &source_registry_schema,
            rule_intake_schema: &rule_intake_schema,
        };
        let mut doc = fixtures["valid"][1]["registry"].clone();
        let source = &mut doc["workspace_connections"][0]["payload"]["discovered_sources"][0];
        source["payload"]["recorded_state"]["currency"] = serde_json::json!("invented-currency");
        let problems = evaluate_existing_project_compatibility_mode(&doc, &opts);
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("discovered source:")
                    && p.message().contains("invented-currency")),
            "{problems:?}"
        );
    }

    /// A rule candidate's own malformed data is reported through the REAL,
    /// already-accepted `controlled_rule_intake` composition — the
    /// diagnostic is prefixed `rule candidate:`, produced only by the real
    /// pilot's own boundary, not a stub.
    #[test]
    fn a_malformed_embedded_rule_candidate_is_reported_through_the_real_controlled_rule_intake_path(
    ) {
        let (registry_schema, envelope_schema, source_registry_schema, rule_intake_schema) =
            real_eval_opts();
        let fixtures = read_json(
            "registries/operating-model/fixtures/existing-project-compatibility-mode.fixtures.json",
        );
        let opts = EvalOpts {
            registry_schema: &registry_schema,
            envelope_schema: &envelope_schema,
            source_registry_schema: &source_registry_schema,
            rule_intake_schema: &rule_intake_schema,
        };
        let candidate_bearing = fixtures["valid"].as_array().unwrap().iter().find(|c| {
            !c["registry"]["workspace_connections"][0]["payload"]["rule_candidates"]
                .as_array()
                .map(Vec::is_empty)
                .unwrap_or(true)
        });
        let Some(case) = candidate_bearing else {
            panic!("expected at least one valid fixture carrying rule_candidates");
        };
        let mut doc = case["registry"].clone();
        let candidate =
            &mut doc["workspace_connections"][0]["payload"]["rule_candidates"][0]["candidate"];
        candidate["payload"]["classification_basis"] = serde_json::json!("");
        let problems = evaluate_existing_project_compatibility_mode(&doc, &opts);
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("rule candidate:")),
            "{problems:?}"
        );
    }

    /// The Rust half of the direct library-level Node/Rust matched pair
    /// documented in this module's own doc comment and `COMPATIBILITY.md`:
    /// a connection entry the closed transport DTO cannot parse (here, an
    /// unrecognised field alongside a SEPARATE `next_step` mismatch) reports
    /// ONLY the transport-parse diagnostic — `evaluate_connection` is never
    /// reached for this entry. The Node half
    /// (`test/conformance-harness.test.mjs`) uses the SAME real fixture and
    /// the SAME two mutations and asserts the opposite: BOTH diagnostics
    /// are present.
    #[test]
    fn a_transport_parse_failure_skips_business_checks_for_that_entry_only() {
        let (registry_schema, envelope_schema, source_registry_schema, rule_intake_schema) =
            real_eval_opts();
        let fixtures = read_json(
            "registries/operating-model/fixtures/existing-project-compatibility-mode.fixtures.json",
        );
        let opts = EvalOpts {
            registry_schema: &registry_schema,
            envelope_schema: &envelope_schema,
            source_registry_schema: &source_registry_schema,
            rule_intake_schema: &rule_intake_schema,
        };
        let mut doc = fixtures["valid"][1]["registry"].clone();
        let payload = &mut doc["workspace_connections"][0]["payload"];
        payload["unexpected_field"] = Value::Bool(true);
        payload["next_step"] = serde_json::json!("resolve-conflict");

        let problems = evaluate_existing_project_compatibility_mode(&doc, &opts);
        assert!(
            problems.iter().any(|p| p
                .message()
                .contains("could not be parsed into the closed transport shape")),
            "{problems:?}"
        );
        assert!(
            !problems.iter().any(|p| p.message().contains("next_step is")),
            "the next_step business diagnostic must NOT appear — domain construction is unreachable for this entry: {problems:?}"
        );
    }

    /// Corrective round (`CHANGES_REQUESTED` item 2): two `discovered_sources`
    /// entries naming the SAME plan id — one well-formed, one with its own
    /// separate business defect (`currency`) — must be reported as BOTH a
    /// duplicate-id diagnostic (produced by the REAL `instruction_source_registry`
    /// composition, which already detects a repeated declared id) AND a
    /// "2 outcomes" partition diagnostic (produced by
    /// `check_discovery_outcomes` over the corrective round's typed,
    /// non-deduplicated occurrence list) — through the REAL
    /// `evaluate_existing_project_compatibility_mode`, not a unit test of
    /// `check_discovery_outcomes` in isolation. Also exercises item 3's
    /// "malformed discovered" multiplicity case: the SECOND, malformed copy
    /// still counts as one occurrence.
    #[test]
    fn two_discovered_sources_naming_the_same_plan_id_get_duplicate_and_outcome_count_diagnostics()
    {
        let (registry_schema, envelope_schema, source_registry_schema, rule_intake_schema) =
            real_eval_opts();
        let fixtures = read_json(
            "registries/operating-model/fixtures/existing-project-compatibility-mode.fixtures.json",
        );
        let opts = EvalOpts {
            registry_schema: &registry_schema,
            envelope_schema: &envelope_schema,
            source_registry_schema: &source_registry_schema,
            rule_intake_schema: &rule_intake_schema,
        };
        let mut doc = fixtures["valid"][1]["registry"].clone();
        let payload = &mut doc["workspace_connections"][0]["payload"];
        let original = payload["discovered_sources"][0].clone();
        let mut duplicate = original.clone();
        duplicate["title"] = serde_json::json!("a second, differently-titled copy");
        duplicate["payload"]["recorded_state"]["currency"] = serde_json::json!("invented-currency");
        payload["discovered_sources"] = serde_json::json!([original, duplicate]);

        let problems = evaluate_existing_project_compatibility_mode(&doc, &opts);
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("declared more than once")),
            "expected a duplicate-id diagnostic: {problems:?}"
        );
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("root-agents-md")
                    && p.message().contains("2 outcomes")),
            "expected a \"2 outcomes\" partition diagnostic: {problems:?}"
        );
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("invented-currency")),
            "expected the second, malformed copy's own defect still reported: {problems:?}"
        );
    }

    /// Corrective round (`CHANGES_REQUESTED` item 3, minimal regression
    /// case): a `discovery_plan` slot with a VALID id but an INVALID
    /// `path` must be reported with BOTH its own location defect AND a
    /// "no outcome" partition defect — an independent check that only
    /// needs the successfully-typed id subset must not disappear because a
    /// sibling field (`location`) failed.
    #[test]
    fn a_discovery_plan_slot_with_a_valid_id_but_invalid_location_gets_both_defects() {
        let (registry_schema, envelope_schema, source_registry_schema, rule_intake_schema) =
            real_eval_opts();
        let fixtures = read_json(
            "registries/operating-model/fixtures/existing-project-compatibility-mode.fixtures.json",
        );
        let opts = EvalOpts {
            registry_schema: &registry_schema,
            envelope_schema: &envelope_schema,
            source_registry_schema: &source_registry_schema,
            rule_intake_schema: &rule_intake_schema,
        };
        let mut doc = fixtures["valid"][1]["registry"].clone();
        let payload = &mut doc["workspace_connections"][0]["payload"];
        payload["discovery_plan"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "id": "broken-slot",
                "medium": "file",
                "path": "../escape.md",
                "container_ref": "sample-repository",
            }));
        // No discovered_sources/missing_sources/unreadable_sources entry
        // names "broken-slot" — it must end up with zero outcomes.

        let problems = evaluate_existing_project_compatibility_mode(&doc, &opts);
        assert!(
            problems.iter().any(|p| p.message().contains("broken-slot")
                && (p.message().contains("\"..\"") || p.message().contains("segment"))),
            "expected the slot's own location defect: {problems:?}"
        );
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("broken-slot") && p.message().contains("no outcome")),
            "expected a \"no outcome\" partition defect for the same slot: {problems:?}"
        );
    }

    /// Corrective round (`CHANGES_REQUESTED` item 3, "аналогично для
    /// malformed... missing"): a `missing_sources` entry with a VALID
    /// `plan_id` but an INVALID `previously_known`/`id` combination must
    /// still occupy that slot's outcome — the partition check must NOT ALSO
    /// report "no outcome" for the same slot merely because the entry's
    /// OTHER field failed domain construction.
    #[test]
    fn a_malformed_missing_source_with_a_valid_plan_id_still_counts_as_its_slots_outcome() {
        let (registry_schema, envelope_schema, source_registry_schema, rule_intake_schema) =
            real_eval_opts();
        let fixtures = read_json(
            "registries/operating-model/fixtures/existing-project-compatibility-mode.fixtures.json",
        );
        let opts = EvalOpts {
            registry_schema: &registry_schema,
            envelope_schema: &envelope_schema,
            source_registry_schema: &source_registry_schema,
            rule_intake_schema: &rule_intake_schema,
        };
        let mut doc = fixtures["valid"][1]["registry"].clone();
        let payload = &mut doc["workspace_connections"][0]["payload"];
        payload["discovery_plan"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "id": "orphan-slot",
                "medium": "file",
                "path": "orphan.md",
                "container_ref": "sample-repository",
            }));
        // previously_known: true with no id — the schema's own allOf and
        // `convert::build_missing_source` both reject this combination.
        payload["missing_sources"] = serde_json::json!([
            {"plan_id": "orphan-slot", "previously_known": true}
        ]);

        let problems = evaluate_existing_project_compatibility_mode(&doc, &opts);
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("id is absent")),
            "expected the entry's own previously_known/id defect: {problems:?}"
        );
        assert!(
            !problems.iter().any(|p| p.message().contains("orphan-slot")
                && p.message().contains("no outcome")),
            "the malformed entry's plan_id must still count as ONE outcome for its slot: {problems:?}"
        );
    }

    /// Structural (`rust-architecture-conformance-2` corrective round, item
    /// 6): `domain.rs` in THIS crate defines no `existing-project-compatibility-mode`
    /// type or check of its own — it is a pure re-export shim over
    /// [`meridian_core::existing_project_compatibility_mode`]. Grepped by
    /// source text rather than asserted in prose, so a future regression
    /// that reintroduces a real definition here (duplicating the type
    /// `meridian-core` now owns) is caught by a failing test.
    #[test]
    fn app_domain_module_defines_no_type_or_check_of_its_own() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/operating_model/existing_project_compatibility_mode/domain.rs");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} is readable: {e}", path.display()));
        for forbidden in ["pub struct ", "pub enum ", "pub fn "] {
            assert!(
                !text.contains(forbidden),
                "{} contains `{forbidden}` — this module must only re-export \
                 meridian_core::existing_project_compatibility_mode, never define its own type or check",
                path.display()
            );
        }
        assert!(
            text.contains("pub use meridian_core::existing_project_compatibility_mode"),
            "{} must re-export from meridian_core::existing_project_compatibility_mode",
            path.display()
        );
    }

    /// Intentional Rust-native boundary, accepted by the architect
    /// (`rust-architecture-conformance-2`, corrective round item 5;
    /// `COMPATIBILITY.md`, same precedent as `controlled-rule-intake`'s
    /// schema short-circuit): Node's `sameLocation`
    /// (`scripts/lib/existing-project-compatibility-mode.mjs`) compares a
    /// discovery_plan slot's RAW `path`/`container_ref`/etc. fields against
    /// a discovered source's RAW `payload.location` fields unconditionally
    /// — even when the slot's OWN `checkPlanLocation` has already flagged
    /// that slot invalid. Rust compares `location` only between TWO
    /// successfully built typed [`meridian_core::instruction_source::Location`]
    /// values — there is no typed value to compare when the slot's own
    /// domain construction failed. The slot's own primary location defect
    /// and the connection's overall `FAIL` verdict are unchanged on both
    /// sides; outcome partition (`check_discovery_outcomes`) still counts
    /// this slot — it stays in `plan_ids` by its independently typed
    /// `SemanticId`, extracted before and regardless of whether `location`
    /// itself built (corrective round items 3-4). Only the SECONDARY
    /// "location does not match" diagnostic is absent on the Rust side. The
    /// matched Node half is in `test/conformance-harness.test.mjs`.
    #[test]
    fn a_discovered_source_naming_an_invalid_plan_slot_gets_no_secondary_location_mismatch_diagnostic(
    ) {
        let (registry_schema, envelope_schema, source_registry_schema, rule_intake_schema) =
            real_eval_opts();
        let fixtures = read_json(
            "registries/operating-model/fixtures/existing-project-compatibility-mode.fixtures.json",
        );
        let opts = EvalOpts {
            registry_schema: &registry_schema,
            envelope_schema: &envelope_schema,
            source_registry_schema: &source_registry_schema,
            rule_intake_schema: &rule_intake_schema,
        };
        let mut doc = fixtures["valid"][1]["registry"].clone();
        let payload = &mut doc["workspace_connections"][0]["payload"];
        // The plan slot's own path is invalid ("..") AND differs from the
        // discovered source's own (valid) path — Node's raw-string
        // `sameLocation` would see two different path values and flag a
        // mismatch; Rust never reaches the comparison because the slot
        // itself never becomes a typed `Location`.
        payload["discovery_plan"][0]["path"] = serde_json::json!("../escape.md");
        payload["discovered_sources"][0]["payload"]["location"]["path"] =
            serde_json::json!("a-different-valid-path.md");

        let problems = evaluate_existing_project_compatibility_mode(&doc, &opts);
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("root-agents-md") && p.message().contains("segment")),
            "expected the slot's own primary location defect to still be reported: {problems:?}"
        );
        assert!(
            !problems
                .iter()
                .any(|p| p.message().contains("root-agents-md") && p.message().contains("no outcome")),
            "the slot must still count as covered by outcome partition (its independently typed SemanticId), not \"no outcome\": {problems:?}"
        );
        assert!(
            !problems
                .iter()
                .any(|p| p.message().contains("location does not match")),
            "the secondary location-mismatch diagnostic is intentionally absent — see this test's doc comment and COMPATIBILITY.md: {problems:?}"
        );
    }

    /// Structural: this crate's own orchestration (`mod.rs`) calls the REAL
    /// `meridian_core::existing_project_compatibility_mode::checks` functions
    /// by name — not a local reimplementation — for every pure check the
    /// corrective round moved to `meridian-core`.
    #[test]
    fn orchestration_calls_the_real_core_checks_by_name() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/operating_model/existing_project_compatibility_mode/mod.rs");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} is readable: {e}", path.display()));
        for called in [
            "domain::check_discovery_outcomes(",
            "domain::check_changes_have_findings(",
            "domain::check_repository_scope(",
            "domain::check_managed_mode(",
            "domain::compute_next_step(",
            "domain::build_connection_scope(",
        ] {
            assert!(
                text.contains(called),
                "{} must call `{called}` — the real core check, through the domain re-export shim",
                path.display()
            );
        }
    }
}
