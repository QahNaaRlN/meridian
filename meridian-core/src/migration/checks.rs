//! The composite migration-plan checks that need more than one field, or
//! more than one mapping, together — `checkCoverage`, `checkTargetGroups`,
//! `checkTargetAuthority`, `checkVerification`,
//! `computePlanFingerprint`/`computeIdempotencyKey`, `checkIdempotency` and
//! `checkSupersedes` from `scripts/lib/instance-data-migration.mjs`, ported
//! as functions returning [`Diagnostic`]s (`FAIL` — the same severity every
//! one of Node's `problems.push(...)` entries carries) rather than throwing
//! on the first problem, exactly matching `evaluateInstanceDataMigration`'s
//! own accumulate-everything behaviour.
//!
//! Every external fact these checks need — a resolved source snapshot, a
//! resolved evidence record, a resolved rollback/restoration record, a
//! resolved predecessor plan — is a parameter
//! ([`crate::migration::resolved`]), never resolved here.

use std::collections::HashMap;

use crate::types::{
    AuthorityKind, ContentDigest, Diagnostic, DiagnosticLevel, ScopeType, SemanticId, Verdict,
};

use super::canonical;
use super::resolved::{
    EvidenceKind, ResolvedDeterministicPlan, ResolvedEvidence, ResolvedRestorationEvidence,
    ResolvedRollbackSnapshot, ResolvedSourceSnapshot, ResolvedSupersededPlan,
};
use super::types::{
    Disposition, Mapping, MigrationPlan, Qualification, RecordUnit, RollbackPlan, SubVerdict,
};

fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message).expect("check message is never empty")
}

/// `deriveOriginSourceRef`: the canonical `origin.source_ref` a target's
/// origin must trace to — the sorted, de-duplicated set of the record unit
/// id(s) actually contributing to it.
pub fn derive_origin_source_ref(unit_ids: &[&SemanticId]) -> String {
    let mut sorted: Vec<&str> = unit_ids.iter().map(|id| id.as_str()).collect();
    sorted.sort();
    sorted.dedup();
    if sorted.len() == 1 {
        format!("record-unit:{}", sorted[0])
    } else {
        format!("record-units:{}", sorted.join(","))
    }
}

/// `checkCoverage` (property 3): every declared `record_units[]` id carries
/// exactly one mapping — never omitted, never duplicated, never dangling —
/// and no unit id is declared twice.
pub fn check_coverage(units: &[RecordUnit], mappings: &[Mapping]) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    let mut seen_units: Vec<&str> = Vec::new();
    for u in units {
        if seen_units.contains(&u.id().as_str()) {
            problems.push(fail(format!(
                "record_units id \"{}\" is declared more than once",
                u.id()
            )));
        } else {
            seen_units.push(u.id().as_str());
        }
    }

    let mut counts: HashMap<&str, u32> = HashMap::new();
    for m in mappings {
        let id = m.unit_id().as_str();
        if !seen_units.contains(&id) {
            problems.push(fail(format!(
                "mapping references unit_id \"{id}\", which is not a declared record unit; a mapping cannot correspond to more than was declared"
            )));
            continue;
        }
        *counts.entry(id).or_insert(0) += 1;
    }
    for id in &seen_units {
        let count = counts.get(id).copied().unwrap_or(0);
        if count == 0 {
            problems.push(fail(format!(
                "record unit \"{id}\" has no mapping; every declared unit requires exactly one correspondence decision (migrated, retained-transitional or merged)"
            )));
        } else if count > 1 {
            problems.push(fail(format!(
                "record unit \"{id}\" has {count} mappings; exactly one is required per unit — a duplicate mapping is never a valid correspondence"
            )));
        }
    }
    problems
}

/// `checkTargetGroups` (properties 3/4): every `target.id` claimed by a
/// `migrated`/`merged` mapping is checked as a group, not mapping-by-mapping.
pub fn check_target_groups(mappings: &[Mapping]) -> Vec<Diagnostic> {
    let mut problems = Vec::new();

    let mut groups: HashMap<&str, Vec<&Mapping>> = HashMap::new();
    for m in mappings {
        if let Some(target) = m.target() {
            groups.entry(target.id().as_str()).or_default().push(m);
        }
    }

    for (target_id, group) in groups {
        let has_migrated = group
            .iter()
            .any(|m| matches!(m.disposition(), Disposition::Migrated { .. }));
        let has_merged = group
            .iter()
            .any(|m| matches!(m.disposition(), Disposition::Merged { .. }));

        if has_migrated && has_merged {
            problems.push(fail(format!(
                "target \"{target_id}\" is claimed by mappings with mixed dispositions (migrated and merged); a shared target is consistently one or the other"
            )));
            continue;
        }

        if has_migrated {
            if group.len() > 1 {
                problems.push(fail(format!(
                    "target \"{target_id}\" is claimed by {} \"migrated\" mappings; a target claimed by more than one unit requires the explicit \"merged\" disposition, not an implicit multi-unit \"migrated\"",
                    group.len()
                )));
                continue;
            }
        } else if group.len() < 2 {
            problems.push(fail(format!(
                "merge target \"{target_id}\" is claimed by only {} mapping(s); a \"merged\" disposition requires at least two source units combining into one target — a single unit is \"migrated\", not \"merged\"",
                group.len()
            )));
            continue;
        } else {
            let rules: Vec<&str> = group
                .iter()
                .filter_map(|m| match m.disposition() {
                    Disposition::Merged { merge_rule_ref, .. } => Some(merge_rule_ref.as_str()),
                    _ => None,
                })
                .collect();
            let mut distinct_rules = rules.clone();
            distinct_rules.sort();
            distinct_rules.dedup();
            if distinct_rules.len() > 1 {
                problems.push(fail(format!(
                    "merge target \"{target_id}\" is claimed by mappings citing different merge_rule_ref values ({}); every mapping merging into one target must cite the same explicit rule",
                    distinct_rules.join(", ")
                )));
            }
        }

        let all_identical = group.windows(2).all(|w| w[0].target() == w[1].target());
        if !all_identical {
            problems.push(fail(format!(
                "target \"{target_id}\" is described inconsistently across the mapping(s) that share it; every mapping contributing to the same target must declare an identical target record"
            )));
        }

        let unit_ids: Vec<&SemanticId> = group.iter().map(|m| m.unit_id()).collect();
        let expected_origin_ref = derive_origin_source_ref(&unit_ids);
        for m in &group {
            if let Some(target) = m.target() {
                if target.origin().source_ref() != expected_origin_ref {
                    problems.push(fail(format!(
                        "target \"{target_id}\" origin.source_ref \"{}\" does not trace to the actual contributing unit(s); expected \"{expected_origin_ref}\" (property 4)",
                        target.origin().source_ref()
                    )));
                }
            }
        }
    }

    problems
}

fn required_authority_for_scope(scope_type: ScopeType) -> Option<AuthorityKind> {
    match scope_type {
        ScopeType::BuiltInMethodology => Some(AuthorityKind::MethodologyOwner),
        ScopeType::UserProfile => Some(AuthorityKind::User),
        ScopeType::OrganizationProfile => Some(AuthorityKind::Organization),
        ScopeType::ProjectWorkspace => Some(AuthorityKind::ProjectOwner),
        ScopeType::RepositoryScope => Some(AuthorityKind::RepositoryMaintainer),
        ScopeType::RunState => None,
    }
}

/// `checkTargetAuthority` (property 4): a migrated/merged target's
/// permanent authority is the human owner authority closed to its OWN
/// scope — never `delegated-run`, and never a scope with no human owner
/// (`run-state`). No-op for a `retained-transitional` mapping.
pub fn check_target_authority(mapping: &Mapping) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    let Some(target) = mapping.target() else {
        return problems;
    };
    let authority = target.authority();
    if authority.kind() == AuthorityKind::DelegatedRun {
        problems.push(fail(format!(
            "target \"{}\" authority.kind is \"delegated-run\"; the run carrying out the migration cannot itself mint the record's permanent authority (property 4)",
            target.id()
        )));
        return problems;
    }
    match required_authority_for_scope(target.scope().scope_type()) {
        None => problems.push(fail(format!(
            "target \"{}\" scope.type is \"{}\"; a migrated or merged record cannot be classified into a scope with no human owner authority",
            target.id(),
            target.scope().scope_type()
        ))),
        Some(required) if authority.kind() != required => problems.push(fail(format!(
            "target \"{}\" authority.kind is \"{}\", but scope \"{}\" requires owner authority \"{}\"",
            target.id(),
            authority.kind(),
            target.scope().scope_type(),
            required
        ))),
        Some(_) => {}
    }
    problems
}

/// `computePlanFingerprint` (property 7): SHA-256 of the canonical JSON
/// text `plan_fingerprint_preimage` builds — matching
/// `createHash('sha256').update(canonical)` over `JSON.stringify({source,
/// record_units, mappings, rollback})` in the Node reference.
pub fn compute_plan_fingerprint(plan: &MigrationPlan) -> ContentDigest {
    let preimage = canonical::plan_fingerprint_preimage(
        plan.source(),
        plan.record_units(),
        plan.mappings(),
        plan.rollback(),
    );
    ContentDigest::of_str(&preimage)
}

/// `computeIdempotencyKey` (property 7): SHA-256 of the canonical JSON text
/// `idempotency_key_preimage` builds.
pub fn compute_idempotency_key(plan: &MigrationPlan) -> ContentDigest {
    let preimage = canonical::idempotency_key_preimage(plan.scope(), plan.source());
    ContentDigest::of_str(&preimage)
}

/// Recomputes `plan_fingerprint` and `idempotency_key` and reports a
/// mismatch against the plan's own declared values.
pub fn check_repeatability(plan: &MigrationPlan) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    let fingerprint = compute_plan_fingerprint(plan);
    if &fingerprint != plan.declared_plan_fingerprint() {
        problems.push(fail(
            "plan_fingerprint does not match the recomputed fingerprint of its own documented canonical projection (source, record_units, mappings, rollback); the same pinned input must always compute the same fingerprint",
        ));
    }
    let key = compute_idempotency_key(plan);
    if &key != plan.declared_idempotency_key() {
        problems.push(fail(
            "idempotency_key does not match the recomputed key derived from this plan's own scope and source (repository_ref, revision, digest); idempotency_key is never an arbitrary free-form string",
        ));
    }
    problems
}

/// `checkVerification` + the `evidence_ref` resolution half of property 6:
/// a claimed `overall_status: VERIFIED` is checked against the actual
/// sub-verdicts and `source.qualification`, and every `verified` sub-verdict
/// is checked against its already-resolved evidence.
pub fn check_verification(
    plan: &MigrationPlan,
    computed_fingerprint: &ContentDigest,
    resolved_coverage_evidence: Option<&ResolvedEvidence>,
    resolved_applicability_evidence: Option<&ResolvedEvidence>,
) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    let v = plan.verification();
    let reproducible = matches!(plan.source().qualification(), Qualification::Reproducible);

    if !reproducible && v.overall_status() != Verdict::Blocked {
        problems.push(fail(format!(
            "source.qualification is \"not-reproducible\" but verification.overall_status is \"{}\"; a dirty, incomplete, ambiguous or changed source is never accepted as a reproducible base, and the plan is BLOCKED, not merely unverified",
            v.overall_status()
        )));
    }

    let coverage_verified = matches!(v.coverage(), SubVerdict::Verified { .. });
    let applicability_verified =
        matches!(v.applicability_preservation(), SubVerdict::Verified { .. });
    let fully_verified = coverage_verified && applicability_verified && reproducible;
    if v.overall_status() == Verdict::Verified && !fully_verified {
        let mut reasons = Vec::new();
        if !coverage_verified {
            reasons.push("coverage is not verified");
        }
        if !applicability_verified {
            reasons.push("applicability preservation is not verified");
        }
        if !reproducible {
            reasons.push("the source is not reproducible");
        }
        problems.push(fail(format!(
            "verification.overall_status is \"VERIFIED\" but {}; a claimed result is checked against actual evidence, never asserted on its own (property 6)",
            reasons.join("; ")
        )));
    }

    for (sub, kind, resolved) in [
        (
            v.coverage(),
            EvidenceKind::Coverage,
            resolved_coverage_evidence,
        ),
        (
            v.applicability_preservation(),
            EvidenceKind::ApplicabilityPreservation,
            resolved_applicability_evidence,
        ),
    ] {
        if let SubVerdict::Verified { evidence_ref } = sub {
            match resolved {
                None => problems.push(fail(format!(
                    "verification evidence_ref \"{evidence_ref}\" does not resolve to a known evidence record through the external resolver; an unresolved evidence_ref never confirms a verified status (property 6)"
                ))),
                Some(r) => {
                    if &r.evidence_ref != evidence_ref {
                        problems.push(fail("resolved evidence_ref does not echo the queried ref"));
                    }
                    if r.kind != kind {
                        problems.push(fail(
                            "resolved evidence kind does not match the expected sub-verdict; evidence for one sub-verdict cannot confirm another",
                        ));
                    }
                    if &r.plan_ref != plan.id() {
                        problems.push(fail(
                            "resolved evidence plan_ref does not name this plan; evidence from a different plan cannot back this one",
                        ));
                    }
                    if &r.plan_fingerprint != computed_fingerprint {
                        problems.push(fail(
                            "resolved evidence plan_fingerprint does not match this plan's own recomputed fingerprint; evidence pinned to a stale or different version of this plan's content never confirms the current one (property 6)",
                        ));
                    }
                    if !r.confirms {
                        problems.push(fail("resolved evidence does not confirm the claimed result (confirms is not true)"));
                    }
                }
            }
        }
    }

    problems
}

/// The `resolveAndCheckSourceSnapshot` half of property 2: a claimed
/// `reproducible` source is checked against an already-resolved snapshot.
pub fn check_reproducibility(
    plan: &MigrationPlan,
    resolved: Option<&ResolvedSourceSnapshot>,
) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    if !matches!(plan.source().qualification(), Qualification::Reproducible) {
        return problems;
    }
    match resolved {
        None => problems.push(fail(
            "source.qualification is \"reproducible\" but its revision does not resolve to a known source snapshot through the external resolver; a claimed value with no resolved backing never confirms reproducibility (property 2)",
        )),
        Some(r) => {
            if &r.repository_ref != plan.source().repository_ref() {
                problems.push(fail(
                    "source.repository_ref does not match the resolved snapshot's repository_ref; a snapshot resolved for a different repository never confirms reproducibility (property 2)",
                ));
            }
            if &r.revision != plan.source().revision() {
                problems.push(fail(
                    "source.revision does not match the resolved snapshot's revision; a claimed revision with no matching resolved snapshot never confirms reproducibility (property 2)",
                ));
            }
            if &r.digest != plan.source().digest() {
                problems.push(fail(
                    "source.digest does not match the resolved snapshot's digest; a claimed digest with no matching resolved snapshot never confirms reproducibility (property 2)",
                ));
            }
            if !r.working_tree_clean {
                problems.push(fail(
                    "source.qualification is \"reproducible\" but the resolved snapshot's working_tree_clean is not true",
                ));
            }
        }
    }
    problems
}

/// Property 5 (reversibility): every rollback reference is checked against
/// its already-resolved record, linked to THIS plan's own source and id
/// regardless of `source.qualification`.
pub fn check_reversibility(
    plan: &MigrationPlan,
    computed_fingerprint: &ContentDigest,
    resolved_rollback_snapshot: Option<&ResolvedRollbackSnapshot>,
    resolved_deterministic_plan: Option<&ResolvedDeterministicPlan>,
    resolved_restoration_evidence: Option<&ResolvedRestorationEvidence>,
) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    let rollback = plan.rollback();

    match resolved_rollback_snapshot {
        None => problems.push(fail(
            "rollback.source_snapshot_ref does not resolve to a known source snapshot through the external resolver; a bare ref string never confirms reversibility (property 5)",
        )),
        Some(r) => {
            if &r.source_snapshot_ref != rollback.source_snapshot_ref() {
                problems.push(fail("resolved rollback snapshot does not echo the queried source_snapshot_ref"));
            }
            if &r.repository_ref != plan.source().repository_ref() || &r.revision != plan.source().revision() {
                problems.push(fail(
                    "rollback.source_snapshot_ref resolves to a repository_ref/revision that does not match this plan's own source; a rollback snapshot must uniquely link to the SAME confirmed source snapshot this plan pins (property 5)",
                ));
            }
            if &r.digest != plan.source().digest() {
                problems.push(fail(
                    "rollback.source_snapshot_ref resolves to a digest that does not match this plan's own source.digest; a rollback snapshot must uniquely link to the SAME confirmed source snapshot (property 5)",
                ));
            }
        }
    }

    match rollback.plan() {
        RollbackPlan::DeterministicReconstruction { deterministic_plan_ref } => match resolved_deterministic_plan {
            None => problems.push(fail(
                "rollback.deterministic_plan_ref does not resolve to a known deterministic-reconstruction plan through the external resolver; a bare ref string never confirms reversibility (property 5)",
            )),
            Some(r) => {
                if &r.deterministic_plan_ref != deterministic_plan_ref {
                    problems.push(fail("resolved deterministic plan does not echo the queried ref"));
                }
                if &r.source_snapshot_ref != rollback.source_snapshot_ref() {
                    problems.push(fail(
                        "resolved deterministic plan's source_snapshot_ref does not name this plan's rollback.source_snapshot_ref; a reconstruction plan for a different snapshot cannot back this rollback (property 5)",
                    ));
                }
                if &r.plan_ref != plan.id() {
                    problems.push(fail(
                        "resolved deterministic plan's plan_ref does not name this plan; a reconstruction plan bound to a different migration plan cannot back this one",
                    ));
                }
                if &r.plan_fingerprint != computed_fingerprint {
                    problems.push(fail(
                        "resolved deterministic plan's plan_fingerprint does not match this plan's own recomputed fingerprint; a reconstruction plan pinned to a stale or different version of this plan's content never confirms the current one (property 5)",
                    ));
                }
                if !r.applicable {
                    problems.push(fail("resolved deterministic plan is not marked applicable"));
                }
            }
        },
        RollbackPlan::VerifiedRestoration { restoration_evidence_ref } => match resolved_restoration_evidence {
            None => problems.push(fail(
                "rollback.restoration_evidence_ref does not resolve to a known restoration-evidence record through the external resolver; a bare ref string never confirms reversibility (property 5)",
            )),
            Some(r) => {
                if &r.evidence_ref != restoration_evidence_ref {
                    problems.push(fail("resolved restoration evidence does not echo the queried ref"));
                }
                if &r.plan_ref != plan.id() {
                    problems.push(fail(
                        "resolved restoration evidence's plan_ref does not name this plan; restoration evidence from a different plan cannot back this one (property 5)",
                    ));
                }
                if &r.plan_fingerprint != computed_fingerprint {
                    problems.push(fail(
                        "resolved restoration evidence's plan_fingerprint does not match this plan's own recomputed fingerprint; restoration evidence pinned to a stale or different version of this plan's content never confirms the current one (property 5)",
                    ));
                }
                if &r.source_snapshot_ref != rollback.source_snapshot_ref() {
                    problems.push(fail(
                        "resolved restoration evidence's source_snapshot_ref does not name this plan's rollback.source_snapshot_ref; restoration evidence for a different snapshot cannot confirm this rollback (property 5)",
                    ));
                }
                if !r.confirms {
                    problems.push(fail("resolved restoration evidence does not confirm restoration (confirms is not true)"));
                }
            }
        },
    }

    problems
}

/// `checkIdempotency` (property 7): `idempotency_key` is unique across a
/// batch of plans — a rerun over the same pinned input recognises the
/// existing record rather than minting a duplicate.
pub fn check_idempotency_uniqueness<'a>(
    plans: impl IntoIterator<Item = &'a MigrationPlan>,
) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    let mut seen: HashMap<String, String> = HashMap::new();
    for plan in plans {
        let key = plan.declared_idempotency_key().value().to_string();
        if let Some(prev_id) = seen.get(&key) {
            problems.push(fail(format!(
                "idempotency_key \"{key}\" is declared by more than one migration plan (\"{prev_id}\" and \"{}\"); a rerun over the same pinned input must recognise the existing record, not mint a duplicate",
                plan.id()
            )));
        } else {
            seen.insert(key, plan.id().as_str().to_string());
        }
    }
    problems
}

/// `checkSupersedes` (property 7): a plan cannot supersede itself; a
/// superseding plan must share its predecessor's FULL scope, the SAME
/// source `repository_ref`, and pin a DIFFERENT source `revision`. When the
/// predecessor is not present in `plans`, `resolve_predecessor` supplies an
/// already-resolved record; an unresolved predecessor is never accepted
/// automatically.
pub fn check_supersedes(
    plans: &[MigrationPlan],
    resolve_predecessor: impl Fn(&str) -> Option<ResolvedSupersededPlan>,
) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    let by_id: HashMap<&str, &MigrationPlan> = plans.iter().map(|p| (p.id().as_str(), p)).collect();

    for plan in plans {
        let Some(sup) = plan.supersedes() else {
            continue;
        };
        if sup == plan.id().as_str() {
            problems.push(fail(format!(
                "migration plan \"{}\" supersedes its own id; a plan cannot supersede itself",
                plan.id()
            )));
            continue;
        }
        if let Some(&target) = by_id.get(sup) {
            if !plan.scope().same_scope(target.scope()) {
                problems.push(fail(format!(
                    "migration plan \"{}\" supersedes \"{sup}\" but they are scoped differently; a plan supersedes only a predecessor for the SAME full scope (type, id, workspace_id and organization_profile_id)",
                    plan.id()
                )));
            }
            if plan.source().repository_ref() != target.source().repository_ref() {
                problems.push(fail(format!(
                    "migration plan \"{}\" supersedes \"{sup}\" but they pin different source.repository_ref; a plan supersedes only a predecessor of the SAME source repository",
                    plan.id()
                )));
            }
            if plan.source().revision() == target.source().revision() {
                problems.push(fail(format!(
                    "migration plan \"{}\" supersedes \"{sup}\" but both pin the identical source.revision; a superseding plan requires a changed source revision, not a restatement of the same one",
                    plan.id()
                )));
            }
            continue;
        }

        match resolve_predecessor(sup) {
            None => problems.push(fail(format!(
                "migration plan \"{}\" supersedes \"{sup}\", which is not present in this batch and does not resolve to a known predecessor through the external resolver; an unknown or unresolved predecessor is never accepted automatically",
                plan.id()
            ))),
            Some(resolved) => {
                if resolved.plan_ref.as_str() != sup {
                    problems.push(fail("resolved predecessor plan_ref does not echo the queried predecessor"));
                }
                if !plan.scope().same_scope(&resolved.scope) {
                    problems.push(fail(format!(
                        "migration plan \"{}\" supersedes \"{sup}\" but the resolved predecessor is scoped differently; a plan supersedes only a predecessor for the SAME full scope (type, id, workspace_id and organization_profile_id)",
                        plan.id()
                    )));
                }
                if plan.source().repository_ref() != &resolved.repository_ref {
                    problems.push(fail(format!(
                        "migration plan \"{}\" supersedes \"{sup}\" but the resolved predecessor pins a different repository_ref; a plan supersedes only a predecessor of the SAME source repository",
                        plan.id()
                    )));
                }
                if plan.source().revision() == &resolved.revision {
                    problems.push(fail(format!(
                        "migration plan \"{}\" supersedes \"{sup}\" but the resolved predecessor pins the identical source.revision; a superseding plan requires a changed source revision, not a restatement of the same one",
                        plan.id()
                    )));
                }
            }
        }
    }

    problems
}
