//! The frozen-Instance write set, its expected record state and the
//! verification diff of what storage actually holds
//! (`meridian-rust-migration-program-plan.md` §5.22.4, item 1).
//!
//! A [`FrozenWriteSet`] is built only from an accepted [`MigrationPlan`]:
//! every `migrated`/`merged` target minted exactly once, every
//! `retained-transitional` unit kept as an explicit retained entry, never
//! written. [`verify_read_back`] compares the records storage reports back
//! against those targets field for field — `$schema`, id, title, record
//! type, scope, origin, authority, payload and schema version — and names
//! every missing, extra, changed or duplicate record. Nothing here reads a
//! database: the caller hands over what it read.

use core::fmt;

use crate::canonical::CanonicalJson;
use crate::run_contracts::RecordText;
use crate::types::{Authority, Origin, Scope, ScopeType, SemanticId};

use super::plan::{DispositionInput, MigrationPlan, TargetInput};
use super::projection;

/// One minted target of an accepted plan, with the units that minted it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteSetEntry {
    unit_ids: Vec<SemanticId>,
    target: TargetInput,
}

impl WriteSetEntry {
    /// The contributing units, sorted.
    pub fn unit_ids(&self) -> &[SemanticId] {
        &self.unit_ids
    }
    pub fn target(&self) -> &TargetInput {
        &self.target
    }
    /// The target's canonical payload object (`content_envelope`).
    pub fn payload(&self) -> CanonicalJson {
        CanonicalJson::from_json(projection::content(&self.target.payload))
    }
}

/// One `retained-transitional` unit: accounted for, never written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedUnit {
    pub unit_id: SemanticId,
    pub reason: RecordText,
}

/// How the plan's record units are accounted for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitCounts {
    pub record_units: usize,
    pub migrated: usize,
    pub retained: usize,
    pub merged: usize,
    pub minted_targets: usize,
}

/// A write set that cannot be applied to a frozen-Instance workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteSetError {
    /// A target of the `built-in-methodology` scope: a frozen Instance
    /// carries product records only, and such a record is never redirected
    /// to another database — the whole operation stops before any write.
    BuiltInMethodologyTarget { target_id: String },
}

impl fmt::Display for WriteSetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WriteSetError::BuiltInMethodologyTarget { target_id } => write!(
                f,
                "target \"{target_id}\" has the built-in-methodology scope; a frozen Instance import writes workspace records only and never redirects a record to the tool database"
            ),
        }
    }
}

impl std::error::Error for WriteSetError {}

/// The accepted write set of one frozen-Instance plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenWriteSet {
    entries: Vec<WriteSetEntry>,
    retained: Vec<RetainedUnit>,
    counts: UnitCounts,
    provenance_refs: Vec<String>,
}

impl FrozenWriteSet {
    /// Every target of `plan`, minted once, sorted by target id; every
    /// retained unit, sorted by unit id. Refuses the whole plan when any
    /// target is not admissible for a workspace database.
    pub fn from_plan(plan: &MigrationPlan) -> Result<FrozenWriteSet, WriteSetError> {
        let payload = &plan.input().payload;
        let mut entries: Vec<WriteSetEntry> = Vec::new();
        let mut retained = Vec::new();
        let mut migrated = 0;
        let mut merged = 0;
        for mapping in &payload.mappings {
            match &mapping.disposition {
                DispositionInput::RetainedTransitional { retained_reason } => {
                    retained.push(RetainedUnit {
                        unit_id: mapping.unit_id.clone(),
                        reason: retained_reason.clone(),
                    });
                    continue;
                }
                DispositionInput::Migrated { .. } => migrated += 1,
                DispositionInput::Merged { .. } => merged += 1,
            }
            let Some(target) = mapping.target() else {
                continue;
            };
            if target.scope.scope_type() == ScopeType::BuiltInMethodology {
                return Err(WriteSetError::BuiltInMethodologyTarget {
                    target_id: target.id.as_str().to_string(),
                });
            }
            match entries.iter_mut().find(|e| e.target.id == target.id) {
                Some(entry) => entry.unit_ids.push(mapping.unit_id.clone()),
                None => entries.push(WriteSetEntry {
                    unit_ids: vec![mapping.unit_id.clone()],
                    target: target.clone(),
                }),
            }
        }
        for entry in &mut entries {
            entry.unit_ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        }
        entries.sort_by(|a, b| a.target.id.as_str().cmp(b.target.id.as_str()));
        retained.sort_by(|a, b| a.unit_id.as_str().cmp(b.unit_id.as_str()));
        let mut provenance_refs: Vec<String> = payload
            .record_units
            .iter()
            .map(|u| format!("record-unit:{}", u.id.as_str()))
            .chain(
                entries
                    .iter()
                    .map(|e| e.target.origin_source_ref.as_str().to_string()),
            )
            .collect();
        provenance_refs.sort();
        provenance_refs.dedup();
        let counts = UnitCounts {
            record_units: payload.record_units.len(),
            migrated,
            retained: retained.len(),
            merged,
            minted_targets: entries.len(),
        };
        Ok(FrozenWriteSet {
            entries,
            retained,
            counts,
            provenance_refs,
        })
    }

    pub fn entries(&self) -> &[WriteSetEntry] {
        &self.entries
    }
    pub fn retained(&self) -> &[RetainedUnit] {
        &self.retained
    }
    pub fn counts(&self) -> UnitCounts {
        self.counts
    }

    /// Whether `source_ref` claims provenance from one of this plan's units
    /// (a single unit or a merge group).
    fn claims_provenance(&self, source_ref: &str) -> bool {
        self.provenance_refs
            .binary_search_by(|r| r.as_str().cmp(source_ref))
            .is_ok()
    }
}

/// One record exactly as storage reported it back.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadBackRecord {
    pub scope: Scope,
    pub id: SemanticId,
    pub schema: String,
    pub schema_version: u32,
    pub title: String,
    pub record_type: SemanticId,
    pub origin: Origin,
    pub authority: Authority,
    pub payload: CanonicalJson,
}

/// The fields a read-back record is compared on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecordField {
    Schema,
    Title,
    RecordType,
    Origin,
    Authority,
    Payload,
    SchemaVersion,
}

impl RecordField {
    pub fn as_str(self) -> &'static str {
        match self {
            RecordField::Schema => "$schema",
            RecordField::Title => "title",
            RecordField::RecordType => "record_type",
            RecordField::Origin => "origin",
            RecordField::Authority => "authority",
            RecordField::Payload => "payload",
            RecordField::SchemaVersion => "schema_version",
        }
    }
}

/// A record present for an expected key whose content differs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedRecord {
    pub id: SemanticId,
    pub fields: Vec<RecordField>,
}

/// The verification diff of one write set against storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationDiff {
    pub expected: usize,
    pub imported: usize,
    pub missing: Vec<SemanticId>,
    /// `"<scope type>:<scope id>/<record id>"` of a record claiming this
    /// plan's provenance that the plan never minted there.
    pub extra: Vec<String>,
    pub changed: Vec<ChangedRecord>,
    /// A provenance reference claimed by more than one stored record.
    pub duplicate: Vec<String>,
}

impl VerificationDiff {
    pub fn is_clean(&self) -> bool {
        self.missing.is_empty()
            && self.extra.is_empty()
            && self.changed.is_empty()
            && self.duplicate.is_empty()
            && self.imported == self.expected
    }
}

fn record_label(scope: &Scope, id: &SemanticId) -> String {
    format!(
        "{}:{}/{}",
        scope.scope_type().as_str(),
        scope.id(),
        id.as_str()
    )
}

fn changed_fields(target: &TargetInput, record: &ReadBackRecord) -> Vec<RecordField> {
    let mut fields = Vec::new();
    if record.schema != target.schema_ref.as_str() {
        fields.push(RecordField::Schema);
    }
    if record.title != target.title.as_str() {
        fields.push(RecordField::Title);
    }
    if record.record_type != target.record_type {
        fields.push(RecordField::RecordType);
    }
    let origin_matches = matches!(
        &record.origin,
        Origin::Migrated { source_ref } if source_ref.as_str() == target.origin_source_ref.as_str()
    );
    if !origin_matches {
        fields.push(RecordField::Origin);
    }
    let authority = &record.authority;
    if authority.kind() != target.authority.kind
        || authority.authority_ref() != target.authority.authority_ref.as_str()
        || authority.decision_ref() != Some(target.authority.decision_ref.as_str())
    {
        fields.push(RecordField::Authority);
    }
    // Compared in canonical form: member order is never content.
    let expected_payload = CanonicalJson::from_json(projection::content(&target.payload));
    if record.payload.canonical_text() != expected_payload.canonical_text() {
        fields.push(RecordField::Payload);
    }
    if record.schema_version != 1 {
        fields.push(RecordField::SchemaVersion);
    }
    fields
}

/// Compares every record `stored` holds against `write_set`: each expected
/// target must be present under its own key with identical content; any
/// other stored record claiming this plan's provenance is extra; a
/// provenance claimed by two stored records is a duplicate.
pub fn verify_read_back(write_set: &FrozenWriteSet, stored: &[ReadBackRecord]) -> VerificationDiff {
    let mut missing = Vec::new();
    let mut changed = Vec::new();
    let mut imported = 0;
    for entry in &write_set.entries {
        let target = &entry.target;
        let found = stored
            .iter()
            .find(|r| r.id == target.id && r.scope.same_scope(&target.scope));
        match found {
            None => missing.push(target.id.clone()),
            Some(record) => {
                imported += 1;
                let fields = changed_fields(target, record);
                if !fields.is_empty() {
                    changed.push(ChangedRecord {
                        id: target.id.clone(),
                        fields,
                    });
                }
            }
        }
    }

    let mut extra = Vec::new();
    let mut claims: Vec<(String, usize)> = Vec::new();
    for record in stored {
        let Some(source_ref) = record.origin.source_ref() else {
            continue;
        };
        if !matches!(record.origin, Origin::Migrated { .. })
            || !write_set.claims_provenance(source_ref)
        {
            continue;
        }
        match claims.iter_mut().find(|(r, _)| r == source_ref) {
            Some(slot) => slot.1 += 1,
            None => claims.push((source_ref.to_string(), 1)),
        }
        let expected = write_set
            .entries
            .iter()
            .any(|e| e.target.id == record.id && e.target.scope.same_scope(&record.scope));
        if !expected {
            extra.push(record_label(&record.scope, &record.id));
        }
    }
    let mut duplicate: Vec<String> = claims
        .into_iter()
        .filter(|(_, n)| *n > 1)
        .map(|(r, _)| r)
        .collect();
    extra.sort();
    duplicate.sort();
    VerificationDiff {
        expected: write_set.entries.len(),
        imported,
        missing,
        extra,
        changed,
        duplicate,
    }
}

#[cfg(test)]
mod tests;
