//! The accepted frozen-Instance bundle: ONE source view through the
//! accepted production operations of `rust-architecture-conformance-7`
//! (`meridian-rust-migration-program-plan.md` §5.22.4, items 5–8, 12).
//!
//! ```text
//! FrozenSource (one bundle commit, one pinned tree)
//!   -> bundle files: bytes -> JSON (transport)
//!   -> registry.json: instance-data-migration container gate -> closed DTO
//!      -> check_migration_plans with the bundle's own resolver maps
//!   -> pinned tree: recomputed digest == plan source digest; every tracked
//!      carrier is some unit's carrier and vice versa
//!   -> every unit's actual content from the pinned revision (fragment)
//!   -> canonical-export.json: instance-canonical-export container gate
//!      -> closed DTO -> check_canonical_exports against the PINNED plan
//!         and that reconstructed content
//!   -> FrozenWriteSet (workspace-only) + the pinned applicability register
//! ```
//!
//! A failed input read is an [`BundleError`]; every domain problem is a
//! diagnostic of a [`BundleRejection`]. Nothing is written anywhere.

use core::fmt;

use meridian_core::migration::export::{compute_source_content_key, CanonicalExport};
use meridian_core::migration::plan::{
    compute_idempotency_key, compute_plan_fingerprint, MigrationPlan, PinnedPlan, PlanInput,
};
use meridian_core::migration::resolved::{
    DigestResponse, ResponseCatalogue, SourceContentResponse,
};
use meridian_core::migration::write_set::{FrozenWriteSet, UnitCounts};
use meridian_core::resolver::ApplicabilityRecord;
use meridian_core::run_contracts::{ResponseField, ResponseItem};
use meridian_core::types::{ContentDigest, Diagnostic, SemanticId, WorkspaceRelativePath};
use serde_json::{Map, Value};

use super::fragment::{resolve_units, Register, ResolvedUnits};
use super::records::target_payload;
use super::source::{BundleFile, FrozenSource, SourceError};
use crate::operating_model::migration_boundary::outcome_diagnostics;
use crate::operating_model::migration_boundary::resolution::plan_resolution;
use crate::operating_model::run_contract_boundary::{fail, CaseOutcome};
use crate::operating_model::{instance_canonical_export, instance_data_migration};
use crate::rule_resolution::typed_applicability_register;
use crate::source_format::json_schema;
use crate::workspace::WorkspaceReader;
use crate::workspace_state::{PayloadRegistry, PayloadSubject};

/// The Kernel schemas every migration operation checks against, loaded
/// from the named Kernel through the workspace port.
pub struct KernelSchemas {
    plan: Value,
    export: Value,
    envelope: Value,
    applicability: Value,
    payloads: PayloadRegistry,
}

const SCHEMA_PATHS: [&str; 4] = [
    "registries/operating-model/instance-data-migration.schema.json",
    "registries/operating-model/instance-canonical-export.schema.json",
    "registries/operating-model/scoped-record.schema.json",
    "registries/rule-resolution/applicability.schema.json",
];

impl KernelSchemas {
    pub fn load(reader: &dyn WorkspaceReader) -> Result<KernelSchemas, BundleError> {
        let mut loaded = Vec::with_capacity(SCHEMA_PATHS.len());
        for path in SCHEMA_PATHS {
            let rel = WorkspaceRelativePath::new(path)
                .map_err(|e| BundleError::Kernel(format!("{path}: {e}")))?;
            let text = reader
                .read_text(&rel)
                .map_err(|e| BundleError::Kernel(format!("{path} cannot be read: {e}")))?;
            let value: Value = serde_json::from_str(&text)
                .map_err(|e| BundleError::Kernel(format!("{path} is not valid JSON: {e}")))?;
            json_schema::assert_supported_deep(&value, path)
                .map_err(|e| BundleError::Kernel(format!("{path}: {e}")))?;
            loaded.push(value);
        }
        let payloads =
            PayloadRegistry::load(reader).map_err(|e| BundleError::Kernel(e.to_string()))?;
        let mut loaded = loaded.into_iter();
        let mut next = || loaded.next().unwrap_or(Value::Null);
        Ok(KernelSchemas {
            plan: next(),
            export: next(),
            envelope: next(),
            applicability: next(),
            payloads,
        })
    }

    /// The one payload validator of product records
    /// (`crate::workspace_state::PayloadRegistry`), shared by both import
    /// kinds.
    pub(crate) fn payloads(&self) -> &PayloadRegistry {
        &self.payloads
    }

    pub(crate) fn envelope(&self) -> &Value {
        &self.envelope
    }

    pub(crate) fn applicability(&self) -> &Value {
        &self.applicability
    }
}

/// An input or environment failure: no domain result exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BundleError {
    Kernel(String),
    Source(SourceError),
    Malformed { file: &'static str, message: String },
}

impl fmt::Display for BundleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BundleError::Kernel(message) => write!(f, "Kernel schema: {message}"),
            BundleError::Source(error) => write!(f, "{error}"),
            BundleError::Malformed { file, message } => write!(f, "{file}: {message}"),
        }
    }
}

impl std::error::Error for BundleError {}

impl From<SourceError> for BundleError {
    fn from(error: SourceError) -> Self {
        BundleError::Source(error)
    }
}

/// The pinned source identity, with its RECOMPUTED tree digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceIdentity {
    pub repository_ref: String,
    pub revision: String,
    pub digest: ContentDigest,
}

/// What is known about the plan even when it is rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanFacts {
    pub plan_id: String,
    pub fingerprint: ContentDigest,
    pub idempotency_key: ContentDigest,
    pub counts: UnitCounts,
    pub source_repository_ref: String,
    pub source_revision: String,
    pub declared_source_digest: ContentDigest,
    pub declared_status: String,
}

/// The applicability basis of the proof: the pinned register's typed
/// records and the targets its record units minted.
#[derive(Debug, Clone, PartialEq)]
pub struct ApplicabilityBasis {
    pub before: Vec<ApplicabilityRecord>,
    pub declared_schema: String,
    pub target_ids: Vec<SemanticId>,
}

/// A bundle every accepted operation passed.
#[derive(Debug, Clone, PartialEq)]
pub struct AcceptedBundle {
    pub bundle_revision: String,
    pub facts: PlanFacts,
    pub plan: MigrationPlan,
    pub export: CanonicalExport,
    pub write_set: FrozenWriteSet,
    pub source: SourceIdentity,
    pub applicability: ApplicabilityBasis,
}

/// A bundle one of the accepted operations rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleRejection {
    pub bundle_revision: String,
    pub facts: Option<PlanFacts>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BundleVerdict {
    Accepted(Box<AcceptedBundle>),
    Rejected(Box<BundleRejection>),
}

fn read_json(source: &dyn FrozenSource, file: BundleFile) -> Result<Value, BundleError> {
    let bytes = source.bundle_file(file)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| BundleError::Malformed {
        file: file.path(),
        message: "is not UTF-8".to_string(),
    })?;
    serde_json::from_str(text).map_err(|e| BundleError::Malformed {
        file: file.path(),
        message: format!("is not valid JSON: {e}"),
    })
}

/// A bundle map file's `resolution` object.
fn resolution_of(value: &Value, file: BundleFile) -> Result<Map<String, Value>, BundleError> {
    value
        .get("resolution")
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(|| BundleError::Malformed {
            file: file.path(),
            message: "carries no \"resolution\" object".to_string(),
        })
}

struct BundleDocuments {
    registry: Value,
    export: Value,
    resolver_maps: Map<String, Value>,
}

fn read_documents(source: &dyn FrozenSource) -> Result<BundleDocuments, BundleVerdictStep> {
    let registry = read_json(source, BundleFile::Registry)?;
    let export = read_json(source, BundleFile::CanonicalExport)?;
    let snapshot = read_json(source, BundleFile::SourceSnapshot)?;
    let rollback = read_json(source, BundleFile::RollbackSnapshot)?;
    let deterministic = read_json(source, BundleFile::DeterministicPlan)?;
    let coverage = read_json(source, BundleFile::CoverageEvidence)?;
    let applicability = read_json(source, BundleFile::ApplicabilityEvidence)?;

    let mut evidence = resolution_of(&coverage, BundleFile::CoverageEvidence)?;
    for (reference, entry) in resolution_of(&applicability, BundleFile::ApplicabilityEvidence)? {
        if let Some(existing) = evidence.get(&reference) {
            if *existing != entry {
                return Err(BundleVerdictStep::Rejected(vec![fail(format!(
                    "evidence \"{reference}\" is resolved differently by two bundle evidence files"
                ))]));
            }
        }
        evidence.insert(reference, entry);
    }
    let mut maps = Map::new();
    maps.insert(
        "source_snapshot_resolution".to_string(),
        Value::Object(resolution_of(&snapshot, BundleFile::SourceSnapshot)?),
    );
    maps.insert("evidence_resolution".to_string(), Value::Object(evidence));
    maps.insert(
        "rollback_snapshot_resolution".to_string(),
        Value::Object(resolution_of(&rollback, BundleFile::RollbackSnapshot)?),
    );
    maps.insert(
        "deterministic_plan_resolution".to_string(),
        Value::Object(resolution_of(
            &deterministic,
            BundleFile::DeterministicPlan,
        )?),
    );
    Ok(BundleDocuments {
        registry,
        export,
        resolver_maps: maps,
    })
}

/// An internal early exit: an input error, or a rejection.
enum BundleVerdictStep {
    Error(BundleError),
    Rejected(Vec<Diagnostic>),
}

impl From<BundleError> for BundleVerdictStep {
    fn from(error: BundleError) -> Self {
        BundleVerdictStep::Error(error)
    }
}

impl From<SourceError> for BundleVerdictStep {
    fn from(error: SourceError) -> Self {
        BundleVerdictStep::Error(BundleError::Source(error))
    }
}

fn counts_of(input: &PlanInput) -> UnitCounts {
    let mut migrated = 0;
    let mut merged = 0;
    let mut retained = 0;
    let mut targets: Vec<&str> = Vec::new();
    for m in &input.payload.mappings {
        match m.disposition.as_str() {
            "migrated" => migrated += 1,
            "merged" => merged += 1,
            _ => retained += 1,
        }
        if let Some(t) = m.target() {
            if !targets.contains(&t.id.as_str()) {
                targets.push(t.id.as_str());
            }
        }
    }
    UnitCounts {
        record_units: input.payload.record_units.len(),
        migrated,
        retained,
        merged,
        minted_targets: targets.len(),
    }
}

fn facts_of(input: &PlanInput) -> PlanFacts {
    let source = &input.payload.source;
    PlanFacts {
        plan_id: input.head.id.as_str().to_string(),
        fingerprint: compute_plan_fingerprint(&input.payload),
        idempotency_key: compute_idempotency_key(&input.head.scope, source),
        counts: counts_of(input),
        source_repository_ref: source.repository_ref.as_str().to_string(),
        source_revision: source.revision.as_str().to_string(),
        declared_source_digest: source.digest.clone(),
        declared_status: input
            .payload
            .verification
            .overall_status
            .as_str()
            .to_string(),
    }
}

/// The plan's typed input, for its facts, through the same route.
fn typed_facts(registry: &Value, schemas: &KernelSchemas) -> Option<PlanFacts> {
    let plans = registry.get("migration_plans")?.as_array()?;
    let [record] = plans.as_slice() else {
        return None;
    };
    let route = instance_data_migration::schemas(&schemas.plan, &schemas.envelope);
    let title = "frozen-instance plan".to_string();
    instance_data_migration::typed_plan(record, title, &route)
        .ok()
        .map(|input| facts_of(&input))
}

fn content_catalogue(
    plan: &MigrationPlan,
    units: &ResolvedUnits,
    diagnostics: &mut Vec<Diagnostic>,
) -> ResponseCatalogue<SourceContentResponse> {
    let payload = &plan.input().payload;
    let repository_ref = payload.source.repository_ref.as_str();
    let revision = payload.source.revision.as_str();
    let mut catalogue = ResponseCatalogue::new();
    for mapping in &payload.mappings {
        if mapping.target().is_none() {
            continue;
        }
        let Some(unit) = payload
            .record_units
            .iter()
            .find(|u| u.id == mapping.unit_id)
        else {
            continue;
        };
        if mapping.merge_rule_ref().is_some() {
            diagnostics.push(fail(format!(
                "unit \"{}\" is merged; the content of a merge group cannot be reconstructed from the pinned source by this import",
                unit.id.as_str()
            )));
            continue;
        }
        let unit_ref = unit.unit_ref.as_str();
        match units.contents.get(unit_ref) {
            Some(Ok(content)) => {
                let key = compute_source_content_key(
                    repository_ref,
                    revision,
                    &[unit_ref.to_string()],
                    None,
                );
                catalogue.insert(
                    key,
                    SourceContentResponse {
                        record_type: ResponseField::Present("instance-source-content".to_string()),
                        repository_ref: ResponseField::Present(repository_ref.to_string()),
                        revision: ResponseField::Present(revision.to_string()),
                        unit_refs: ResponseField::Present(vec![ResponseItem::Text(
                            unit_ref.to_string(),
                        )]),
                        merge_rule_ref: ResponseField::Absent,
                        media_type: ResponseField::Present(content.media_type.to_string()),
                        encoding: ResponseField::Present("utf-8".to_string()),
                        content: ResponseField::Present(content.content.clone()),
                        digest: ResponseField::Present(DigestResponse {
                            algorithm: ResponseField::Present("sha-256".to_string()),
                            value: ResponseField::Present(content.digest.value().to_string()),
                            unknown_keys: Vec::new(),
                        }),
                        unknown_keys: Vec::new(),
                    },
                );
            }
            Some(Err(reason)) => diagnostics.push(fail(format!(
                "unit \"{}\" ({unit_ref}): its actual content cannot be reconstructed from the pinned revision: {reason}",
                unit.id.as_str()
            ))),
            None => diagnostics.push(fail(format!(
                "unit \"{}\" ({unit_ref}): no content was resolved",
                unit.id.as_str()
            ))),
        }
    }
    catalogue
}

fn accepted_one<T>(outcome: CaseOutcome<Vec<T>>, what: &str) -> Result<T, Vec<Diagnostic>> {
    let diagnostics = outcome_diagnostics(&outcome);
    match outcome {
        CaseOutcome::Accepted(mut items) if items.len() == 1 => Ok(items.remove(0)),
        CaseOutcome::Accepted(items) => Err(vec![fail(format!(
            "the bundle carries {} {what}(s); a frozen-Instance import applies exactly one",
            items.len()
        ))]),
        _ => Err(diagnostics),
    }
}

/// The one accepted applicability register of the pinned source. The
/// proof compares exactly one register: none (nothing to prove) and several
/// (no single basis) both reject the bundle before any plan, apply or
/// verify can report it equivalent.
fn single_register(registers: &[Register]) -> Result<&Register, Vec<Diagnostic>> {
    match registers {
        [register] => Ok(register),
        [] => Err(vec![fail(
            "the pinned source carries no applicability register; the applicability proof requires exactly one accepted register"
                .to_string(),
        )]),
        many => Err(vec![fail(format!(
            "the pinned source carries {} applicability registers; the proof compares exactly one",
            many.len()
        ))]),
    }
}

fn applicability_basis(
    plan: &MigrationPlan,
    units: &ResolvedUnits,
    schemas: &KernelSchemas,
) -> Result<ApplicabilityBasis, Vec<Diagnostic>> {
    let payload = &plan.input().payload;
    let register = single_register(&units.registers)?;
    let carrier = &register.carrier;
    let before =
        typed_applicability_register(&register.document, &schemas.applicability).map_err(|e| {
            vec![fail(format!(
                "applicability register \"{carrier}\" at the pinned revision: {e}"
            ))]
        })?;
    if before.is_empty() {
        return Err(vec![fail(format!(
            "applicability register \"{carrier}\" carries no record; an empty register proves nothing"
        ))]);
    }
    let declared_schema = register
        .document
        .get("$schema")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            vec![fail(format!(
                "applicability register \"{carrier}\" declares no \"$schema\""
            ))]
        })?
        .to_string();
    let mut target_ids = Vec::new();
    for unit_ref in &register.record_refs {
        let unit = payload
            .record_units
            .iter()
            .find(|u| u.unit_ref.as_str() == unit_ref)
            .ok_or_else(|| {
                vec![fail(format!(
                    "applicability register \"{carrier}\": record \"{unit_ref}\" is not a record unit of the plan"
                ))]
            })?;
        let target = payload
            .mappings
            .iter()
            .find(|m| m.unit_id == unit.id)
            .and_then(|m| m.target());
        if let Some(target) = target {
            if !target_ids.contains(&target.id) {
                target_ids.push(target.id.clone());
            }
        }
    }
    if target_ids.is_empty() {
        return Err(vec![fail(format!(
            "applicability register \"{carrier}\": no record unit mints a target, so no stored record can be compared"
        ))]);
    }
    target_ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    Ok(ApplicabilityBasis {
        before,
        declared_schema,
        target_ids,
    })
}

fn accept(
    source: &dyn FrozenSource,
    schemas: &KernelSchemas,
    documents: &BundleDocuments,
) -> Result<Box<AcceptedBundle>, BundleVerdictStep> {
    let plan_schemas = instance_data_migration::schemas(&schemas.plan, &schemas.envelope);
    let resolution = plan_resolution(&documents.resolver_maps);
    let plan = accepted_one(
        instance_data_migration::check_container(&documents.registry, &plan_schemas, &resolution),
        "migration plan",
    )
    .map_err(BundleVerdictStep::Rejected)?;
    let facts = facts_of(plan.input());
    let revision = facts.source_revision.clone();

    let tree = source.pinned_tree(&revision)?;
    let mut diagnostics = Vec::new();
    if tree.digest != facts.declared_source_digest {
        diagnostics.push(fail(format!(
            "the pinned source tree of revision {revision} has digest {}, not the plan's declared source digest {}",
            tree.digest.value(),
            facts.declared_source_digest.value()
        )));
    }
    let unit_refs: Vec<&str> = plan
        .input()
        .payload
        .record_units
        .iter()
        .map(|u| u.unit_ref.as_str())
        .collect();
    let mut carriers: Vec<&str> = unit_refs
        .iter()
        .map(|r| r.split_once('#').map_or(*r, |(c, _)| c))
        .collect();
    carriers.sort();
    carriers.dedup();
    let untracked: Vec<&&str> = carriers
        .iter()
        .filter(|c| tree.paths.binary_search_by(|p| p.as_str().cmp(c)).is_err())
        .collect();
    let unclaimed: Vec<&String> = tree
        .paths
        .iter()
        .filter(|p| carriers.binary_search(&p.as_str()).is_err())
        .collect();
    if !untracked.is_empty() || !unclaimed.is_empty() {
        diagnostics.push(fail(format!(
            "the plan's carriers and the files tracked at revision {revision} differ (carriers not tracked: {untracked:?}; tracked files no unit accounts for: {unclaimed:?})"
        )));
    }
    let read = |path: &str| source.file_at(&revision, path);
    let units = resolve_units(&unit_refs, &read)?;
    let content = content_catalogue(&plan, &units, &mut diagnostics);
    if !diagnostics.is_empty() {
        return Err(BundleVerdictStep::Rejected(diagnostics));
    }

    let pinned = PinnedPlan::confirm(plan.input().clone(), plan.fingerprint().value()).map_err(
        |recomputed| {
            BundleVerdictStep::Rejected(vec![fail(format!(
                "internal: the accepted plan does not confirm its own fingerprint {}",
                recomputed.value()
            ))])
        },
    )?;
    let export_schemas = instance_canonical_export::schemas(&schemas.export, &schemas.envelope);
    let export = accepted_one(
        instance_canonical_export::check_pinned_container(
            &documents.export,
            &export_schemas,
            &pinned,
            &content,
        ),
        "canonical export",
    )
    .map_err(BundleVerdictStep::Rejected)?;

    let write_set = FrozenWriteSet::from_plan(&plan)
        .map_err(|e| BundleVerdictStep::Rejected(vec![fail(e.to_string())]))?;
    let payload_problems = payload_contract_problems(&write_set, schemas.payloads());
    if !payload_problems.is_empty() {
        return Err(BundleVerdictStep::Rejected(payload_problems));
    }
    let applicability =
        applicability_basis(&plan, &units, schemas).map_err(BundleVerdictStep::Rejected)?;
    Ok(Box::new(AcceptedBundle {
        bundle_revision: source.bundle_revision().to_string(),
        source: SourceIdentity {
            repository_ref: facts.source_repository_ref.clone(),
            revision,
            digest: tree.digest,
        },
        facts,
        plan,
        export,
        write_set,
        applicability,
    }))
}

/// Every migrated target's payload through the one payload validator
/// (`rust-workspace-state-validation`, M-05b): a target no contract accepts
/// is refused here, before any database is opened for writing.
fn payload_contract_problems(
    write_set: &FrozenWriteSet,
    payloads: &PayloadRegistry,
) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    for entry in write_set.entries() {
        let target = entry.target();
        let Value::Object(payload) = target_payload(entry) else {
            continue;
        };
        let subject = PayloadSubject {
            id: &target.id,
            scope: &target.scope,
            title: target.title.as_str(),
            record_type: target.record_type.as_str(),
        };
        if let Err(rejection) = payloads.check(subject, &payload) {
            for problem in rejection.problems {
                problems.push(fail(format!(
                    "payload-contract: target \"{}\" ({}): {problem}",
                    target.id.as_str(),
                    target.record_type.as_str()
                )));
            }
        }
    }
    problems
}

/// Reads the bundle once from `source` and runs it through every accepted
/// operation. `Err` = no domain result exists.
pub fn load_bundle(
    source: &dyn FrozenSource,
    schemas: &KernelSchemas,
) -> Result<BundleVerdict, BundleError> {
    let bundle_revision = source.bundle_revision().to_string();
    let documents = match read_documents(source) {
        Ok(documents) => documents,
        Err(BundleVerdictStep::Error(error)) => return Err(error),
        Err(BundleVerdictStep::Rejected(diagnostics)) => {
            return Ok(BundleVerdict::Rejected(Box::new(BundleRejection {
                bundle_revision,
                facts: None,
                diagnostics,
            })))
        }
    };
    match accept(source, schemas, &documents) {
        Ok(bundle) => Ok(BundleVerdict::Accepted(bundle)),
        Err(BundleVerdictStep::Error(error)) => Err(error),
        Err(BundleVerdictStep::Rejected(diagnostics)) => {
            Ok(BundleVerdict::Rejected(Box::new(BundleRejection {
                bundle_revision,
                facts: typed_facts(&documents.registry, schemas),
                diagnostics,
            })))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn register(carrier: &str) -> Register {
        Register {
            carrier: carrier.to_string(),
            document: serde_json::json!({"$schema": "./applicability.schema.json", "records": []}),
            record_refs: Vec::new(),
        }
    }

    #[test]
    fn migration_bundle_without_exactly_one_applicability_register_is_rejected() {
        let none = single_register(&[]).unwrap_err();
        assert_eq!(none.len(), 1);
        assert!(
            none[0].message().contains("no applicability register"),
            "{none:?}"
        );
        let two = [register("a.yaml"), register("b.yaml")];
        let many = single_register(&two).unwrap_err();
        assert!(
            many[0].message().contains("2 applicability registers"),
            "{many:?}"
        );
        let one = [register("a.yaml")];
        assert_eq!(single_register(&one).unwrap().carrier, "a.yaml");
    }
}
