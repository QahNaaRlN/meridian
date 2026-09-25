//! The typed responses of the migration contracts' external resolution
//! boundaries (`scripts/lib/instance-data-migration.mjs`:
//! `makeSourceSnapshotResolver`, `makeRefResolver` and the maps a fixture
//! bundle carries for them) and the catalogues they are looked up in.
//!
//! A response is not schema-checked before it reaches this crate: its
//! closed field set IS the contract's domain rule. So every known field is
//! a [`ResponseField`] (absent, of the expected shape, or a
//! [`ForeignValue`](crate::run_contracts::ForeignValue) kept only for its
//! diagnostic) and every other key is kept by name, sorted, so the rule can
//! report it deterministically. `meridian-core` never looks a response up
//! anywhere but in a [`ResponseCatalogue`] its caller built once.

use std::collections::BTreeMap;

use crate::canonical::CanonicalJson;
use crate::run_contracts::{ResponseField, ResponseItem};

/// Responses keyed by the exact string the contract queries with. A map
/// entry that is not an object never enters the catalogue: it does not
/// resolve, exactly as the reference resolver's `isObject` check.
#[derive(Debug, Clone, PartialEq)]
pub struct ResponseCatalogue<T> {
    entries: BTreeMap<String, T>,
}

impl<T> Default for ResponseCatalogue<T> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
}

impl<T> ResponseCatalogue<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: impl Into<String>, response: T) {
        self.entries.insert(key.into(), response);
    }

    pub fn resolve(&self, key: &str) -> Option<&T> {
        self.entries.get(key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// A resolved `{ algorithm, value }` digest object. A non-object `digest`
/// reads as `Absent`/`Foreign` at the enclosing field and compares as `{}`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DigestResponse {
    pub algorithm: ResponseField<String>,
    pub value: ResponseField<String>,
    pub unknown_keys: Vec<String>,
}

/// `resolveSourceSnapshot`'s response (`instance-source-snapshot`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceSnapshotResponse {
    pub record_type: ResponseField<String>,
    pub repository_ref: ResponseField<String>,
    pub revision: ResponseField<String>,
    pub digest: ResponseField<DigestResponse>,
    pub working_tree_clean: ResponseField<bool>,
    pub unknown_keys: Vec<String>,
}

/// `resolveEvidence`'s response (`instance-migration-evidence`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EvidenceResponse {
    pub record_type: ResponseField<String>,
    pub evidence_ref: ResponseField<String>,
    pub kind: ResponseField<String>,
    pub plan_ref: ResponseField<String>,
    pub plan_fingerprint: ResponseField<String>,
    pub confirms: ResponseField<bool>,
    pub unknown_keys: Vec<String>,
}

/// `resolveRollbackSnapshot`'s response (`instance-rollback-snapshot`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RollbackSnapshotResponse {
    pub record_type: ResponseField<String>,
    pub source_snapshot_ref: ResponseField<String>,
    pub repository_ref: ResponseField<String>,
    pub revision: ResponseField<String>,
    pub digest: ResponseField<DigestResponse>,
    pub unknown_keys: Vec<String>,
}

/// `resolveDeterministicPlan`'s response
/// (`instance-deterministic-reconstruction-plan`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeterministicPlanResponse {
    pub record_type: ResponseField<String>,
    pub deterministic_plan_ref: ResponseField<String>,
    pub source_snapshot_ref: ResponseField<String>,
    pub plan_ref: ResponseField<String>,
    pub plan_fingerprint: ResponseField<String>,
    pub applicable: ResponseField<bool>,
    pub unknown_keys: Vec<String>,
}

/// `resolveRestorationEvidence`'s response (`instance-restoration-evidence`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RestorationEvidenceResponse {
    pub record_type: ResponseField<String>,
    pub evidence_ref: ResponseField<String>,
    pub plan_ref: ResponseField<String>,
    pub plan_fingerprint: ResponseField<String>,
    pub source_snapshot_ref: ResponseField<String>,
    pub confirms: ResponseField<bool>,
    pub unknown_keys: Vec<String>,
}

/// A resolved scope object.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScopeResponse {
    pub scope_type: ResponseField<String>,
    pub id: ResponseField<String>,
    pub workspace_id: ResponseField<String>,
    pub organization_profile_id: ResponseField<String>,
    pub unknown_keys: Vec<String>,
}

/// `resolveSupersededPlan`'s response (`instance-superseded-plan`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SupersededPlanResponse {
    pub record_type: ResponseField<String>,
    pub plan_ref: ResponseField<String>,
    pub scope: ResponseField<ScopeResponse>,
    pub repository_ref: ResponseField<String>,
    pub revision: ResponseField<String>,
    pub unknown_keys: Vec<String>,
}

/// Every external boundary a migration plan is checked against, each
/// already parsed into its typed catalogue.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlanResolution {
    /// Keyed `"<repository_ref>@<revision>"`.
    pub source_snapshots: ResponseCatalogue<SourceSnapshotResponse>,
    pub evidence: ResponseCatalogue<EvidenceResponse>,
    pub rollback_snapshots: ResponseCatalogue<RollbackSnapshotResponse>,
    pub deterministic_plans: ResponseCatalogue<DeterministicPlanResponse>,
    pub restoration_evidence: ResponseCatalogue<RestorationEvidenceResponse>,
    pub superseded_plans: ResponseCatalogue<SupersededPlanResponse>,
}

/// A resolved source-unit record as `resolveMigrationPlan` returns it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecordUnitResponse {
    pub id: ResponseField<String>,
    pub unit_ref: ResponseField<String>,
}

/// An open object, kept only as its members' canonical values — the
/// target record a resolved mapping carries, compared field for field.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OpenObject {
    members: Vec<(String, CanonicalJson)>,
}

impl OpenObject {
    /// A repeated member keeps its LAST value.
    pub fn new(members: Vec<(String, CanonicalJson)>) -> Self {
        let mut deduped: Vec<(String, CanonicalJson)> = Vec::with_capacity(members.len());
        for (name, value) in members {
            match deduped.iter_mut().find(|(n, _)| *n == name) {
                Some(slot) => slot.1 = value,
                None => deduped.push((name, value)),
            }
        }
        Self { members: deduped }
    }

    pub fn member(&self, name: &str) -> Option<&CanonicalJson> {
        self.members.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    /// The object of just the named members that are present —
    /// `JSON.stringify` of `{ a: o.a, b: o.b, … }` drops undefined ones.
    pub(crate) fn pick(&self, names: &[&str]) -> CanonicalJson {
        CanonicalJson::object(
            names
                .iter()
                .filter_map(|n| self.member(n).map(|v| ((*n).to_string(), v.clone())))
                .collect(),
        )
    }

    pub(crate) fn whole(&self) -> CanonicalJson {
        CanonicalJson::object(self.members.clone())
    }
}

/// A resolved mapping's target: its `id` as the contract reads it, and the
/// whole object for the structural comparison.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TargetResponse {
    pub id: ResponseField<String>,
    pub record: OpenObject,
}

/// A resolved mapping. `target` is `None` when the mapping carries no
/// target object.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MappingResponse {
    pub unit_id: ResponseField<String>,
    pub disposition: ResponseField<String>,
    pub target: Option<TargetResponse>,
    pub merge_rule_ref: ResponseField<String>,
    pub retained_reason: ResponseField<String>,
}

/// A resolved `{ repository_ref, revision, digest }` source identity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceIdentityResponse {
    pub repository_ref: ResponseField<String>,
    pub revision: ResponseField<String>,
    pub digest: ResponseField<DigestResponse>,
}

/// `resolveMigrationPlan`'s response (`instance-migration-plan`): the
/// closed projection of a plan a canonical export is checked against.
/// Array members that are not objects are dropped, as `filter(isObject)`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MigrationPlanResponse {
    pub record_type: ResponseField<String>,
    pub plan_ref: ResponseField<String>,
    pub plan_fingerprint: ResponseField<String>,
    pub scope: ResponseField<ScopeResponse>,
    pub source: ResponseField<SourceIdentityResponse>,
    pub record_units: Vec<RecordUnitResponse>,
    pub mappings: Vec<MappingResponse>,
    pub unknown_keys: Vec<String>,
}

/// `resolveSourceContent`'s response (`instance-source-content`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SourceContentResponse {
    pub record_type: ResponseField<String>,
    pub repository_ref: ResponseField<String>,
    pub revision: ResponseField<String>,
    pub unit_refs: ResponseField<Vec<ResponseItem>>,
    pub merge_rule_ref: ResponseField<Option<String>>,
    pub media_type: ResponseField<String>,
    pub encoding: ResponseField<String>,
    pub content: ResponseField<String>,
    pub digest: ResponseField<DigestResponse>,
    pub unknown_keys: Vec<String>,
}
