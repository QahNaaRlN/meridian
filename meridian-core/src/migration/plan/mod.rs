//! Instance-data migration plans (`instance-data-migration.md` §1–9,
//! `scripts/lib/instance-data-migration.mjs`'s
//! `evaluateInstanceDataMigration`).
//!
//! [`check_migration_plans`] is the one path from schema-shaped input
//! ([`input`]) to an accepted [`MigrationPlan`]: every per-plan property
//! (`check`), then the two container-wide rules — a unique recomputed
//! `idempotency_key` and a checked `supersedes` — in the Node reference's
//! order and wording. A plan is accepted only when none of the problems is
//! its own; the fingerprint and key it carries are the RECOMPUTED ones.
//!
//! [`PinnedPlan`] is a schema-shaped plan whose recomputed fingerprint
//! equals a qualification's pin — the only plan a composed canonical
//! export may be checked against (`crate::migration::export`).

mod check;
pub mod input;

use crate::types::{ContentDigest, Diagnostic, Scope};

use super::projection;
use super::resolved::PlanResolution;

pub use input::{
    DeclaredContent, DispositionInput, FieldBasisInput, FieldBasisValue, MappingInput,
    OwnerDecisionInput, PlanInput, PlanPayloadInput, Qualification, RecordUnitInput, RollbackInput,
    RollbackPlanInput, SourceInput, SubVerdictInput, TargetAuthorityInput, TargetInput,
    VerificationInput,
};

pub(crate) use check::{same_resolved_scope, RECORD_TYPE};

/// `computePlanFingerprint(payload)`: SHA-256 of the documented canonical
/// projection (`source`, `record_units`, `mappings`, `rollback`), order of
/// the two arrays neutral.
pub fn compute_plan_fingerprint(payload: &PlanPayloadInput) -> ContentDigest {
    projection::plan_fingerprint(payload)
}

/// `computeIdempotencyKey(scope, source)`.
pub fn compute_idempotency_key(scope: &Scope, source: &SourceInput) -> ContentDigest {
    projection::plan_idempotency_key(scope, source)
}

/// A migration plan with zero problems of its own. Only
/// [`check_migration_plans`] builds one; its fingerprint and key are the
/// recomputed values, equal to the declared ones by construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationPlan {
    input: PlanInput,
    fingerprint: ContentDigest,
    idempotency_key: ContentDigest,
}

impl MigrationPlan {
    pub fn input(&self) -> &PlanInput {
        &self.input
    }
    pub fn id(&self) -> &crate::types::SemanticId {
        &self.input.head.id
    }
    pub fn scope(&self) -> &Scope {
        &self.input.head.scope
    }
    pub fn fingerprint(&self) -> &ContentDigest {
        &self.fingerprint
    }
    pub fn idempotency_key(&self) -> &ContentDigest {
        &self.idempotency_key
    }
    pub fn overall_status(&self) -> crate::types::Verdict {
        self.input.payload.verification.overall_status
    }
    /// Whether any mapping mints a record (`migrated` or `merged`).
    pub fn mints_records(&self) -> bool {
        self.input
            .payload
            .mappings
            .iter()
            .any(|m| m.target().is_some())
    }
}

/// A schema-shaped plan whose RECOMPUTED fingerprint equals the pin a
/// composing record declared for it. Nothing else can build one, so a
/// check handed a `PinnedPlan` is handed exactly the pinned content — never
/// a different plan that merely shares its id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedPlan {
    input: PlanInput,
    fingerprint: ContentDigest,
}

impl PinnedPlan {
    /// `Ok` when `pinned_sha256` is this plan's recomputed fingerprint;
    /// otherwise the recomputed fingerprint the pin failed to name.
    pub fn confirm(input: PlanInput, pinned_sha256: &str) -> Result<PinnedPlan, ContentDigest> {
        let fingerprint = compute_plan_fingerprint(&input.payload);
        if fingerprint.value() == pinned_sha256 {
            Ok(PinnedPlan { input, fingerprint })
        } else {
            Err(fingerprint)
        }
    }

    pub fn input(&self) -> &PlanInput {
        &self.input
    }
    pub fn fingerprint(&self) -> &ContentDigest {
        &self.fingerprint
    }
}

/// The result of one batch: every problem in the reference order, and per
/// input plan its accepted form (or `None` when any problem is its own).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanBatchOutcome {
    pub diagnostics: Vec<Diagnostic>,
    pub accepted: Vec<Option<MigrationPlan>>,
}

/// `evaluateInstanceDataMigration` over one schema-clean container's
/// `migration_plans`, resolving every external fact through `resolution`.
pub fn check_migration_plans(
    entries: &[PlanInput],
    resolution: &PlanResolution,
) -> PlanBatchOutcome {
    let mut owned: Vec<(usize, Diagnostic)> = Vec::new();
    let mut recomputed: Vec<(ContentDigest, ContentDigest)> = Vec::with_capacity(entries.len());
    let mut seen_ids: Vec<&str> = Vec::new();

    for (index, entry) in entries.iter().enumerate() {
        let mut problems = Vec::new();
        let id = entry.head.id.as_str();
        if seen_ids.contains(&id) {
            problems.push(super::fail(format!(
                "migration plan \"{id}\" is declared more than once"
            )));
        } else {
            seen_ids.push(id);
        }
        let fingerprint = compute_plan_fingerprint(&entry.payload);
        let key = compute_idempotency_key(&entry.head.scope, &entry.payload.source);
        check::check_plan(entry, &fingerprint, &key, resolution, &mut problems);
        owned.extend(problems.into_iter().map(|p| (index, p)));
        recomputed.push((fingerprint, key));
    }

    // `checkIdempotency`: the DECLARED key, unique across the container.
    let mut first_by_key: Vec<(&str, &str)> = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let key = entry.payload.idempotency_key.value();
        let id = entry.head.id.as_str();
        match first_by_key.iter().find(|(k, _)| *k == key) {
            Some((_, first)) => owned.push((
                index,
                super::fail(format!(
                    "idempotency_key \"{key}\" is declared by more than one migration plan (\"{first}\" and \"{id}\"); a rerun over the same pinned input must recognise the existing record, not mint a duplicate"
                )),
            )),
            None => first_by_key.push((key, id)),
        }
    }

    // `checkSupersedes`: a present predecessor is the LAST plan declaring
    // that id (`new Map(...)` keeps the last).
    for (index, entry) in entries.iter().enumerate() {
        let Some(sup) = entry.payload.supersedes.as_ref().map(|s| s.as_str()) else {
            continue;
        };
        let mut problems = Vec::new();
        if sup == entry.head.id.as_str() {
            problems.push(super::fail(format!(
                "migration plan \"{sup}\" supersedes its own id; a plan cannot supersede itself"
            )));
        } else if let Some(predecessor) = entries.iter().rev().find(|e| e.head.id.as_str() == sup) {
            check::check_present_predecessor(entry, predecessor, &mut problems);
        } else {
            check::check_resolved_predecessor(
                entry,
                sup,
                resolution.superseded_plans.resolve(sup),
                &mut problems,
            );
        }
        owned.extend(problems.into_iter().map(|p| (index, p)));
    }

    let accepted = entries
        .iter()
        .zip(recomputed)
        .enumerate()
        .map(|(index, (entry, (fingerprint, idempotency_key)))| {
            (!owned.iter().any(|(i, _)| *i == index)).then(|| MigrationPlan {
                input: entry.clone(),
                fingerprint,
                idempotency_key,
            })
        })
        .collect();
    PlanBatchOutcome {
        diagnostics: owned.into_iter().map(|(_, d)| d).collect(),
        accepted,
    }
}

#[cfg(test)]
pub(crate) mod tests;
