//! Acceptance tests for `meridian_core::migration` — the nine properties of
//! `instance-data-migration.md`, exercised through the ported check
//! functions: incomplete coverage, implicit merges, target authority,
//! contradictory rollback, unconfirmed `VERIFIED`, and fingerprint/
//! idempotency-key determinism and field-sensitivity.

use meridian_core::migration::checks::*;
use meridian_core::migration::resolved::*;
use meridian_core::migration::*;
use meridian_core::resolver::IsoDate;
use meridian_core::types::{
    Authority, AuthorityKind, ContentDigest, EvidenceRef, NonEmptyString, Revision, Scope,
    SemanticId, Verdict, WorkspaceId,
};

fn digest(seed: &str) -> ContentDigest {
    ContentDigest::of_str(seed)
}

fn scope() -> Scope {
    Scope::project_workspace(SemanticId::new("sample-project").unwrap(), None)
}

fn nes(s: &str) -> NonEmptyString {
    NonEmptyString::new(s).unwrap()
}

fn sid(s: &str) -> SemanticId {
    SemanticId::new(s).unwrap()
}

fn target(
    id: &str,
    decision_ref: &str,
    target_scope: Scope,
    authority_kind: AuthorityKind,
) -> TargetRecord {
    TargetRecord::new(
        nes("scoped-record.schema.json"),
        sid(id),
        nes("A migrated norm"),
        sid("norm"),
        target_scope,
        FieldBasis::new(
            FieldBasisValue::Assigned,
            FieldBasisValue::Preserved,
            FieldBasisValue::Preserved,
            FieldBasisValue::Preserved,
            FieldBasisValue::Assigned,
            FieldBasisValue::Assigned,
            FieldBasisValue::Assigned,
            FieldBasisValue::Preserved,
        ),
        TargetOrigin::new(nes("record-unit:legacy-1")),
        Authority::new(
            authority_kind,
            "workspace-owner",
            Some(decision_ref.to_string()),
        )
        .unwrap(),
        ContentEnvelope::new(
            nes("text/plain"),
            Encoding::Utf8,
            nes("norm text"),
            digest("norm text"),
        )
        .unwrap(),
    )
    .unwrap()
}

fn owner_decision(decision_ref: &str) -> OwnerDecision {
    OwnerDecision::new(
        nes(decision_ref),
        IsoDate::new("2026-09-08").unwrap(),
        nes("accepted as-is"),
    )
}

fn record_unit(id: &str) -> RecordUnit {
    RecordUnit::new(
        sid(id),
        nes(&format!("legacy/{id}.md")),
        nes("kept because it is a stable norm"),
    )
    .unwrap()
}

fn source_state(revision: &str, reproducible: bool) -> SourceState {
    SourceState::new(
        Revision::new(revision).unwrap(),
        digest("source-content"),
        EvidenceRef::new("instance:cbs").unwrap(),
        reproducible,
        if reproducible {
            Qualification::Reproducible
        } else {
            Qualification::NotReproducible {
                reason: nes("working tree is dirty"),
            }
        },
    )
    .unwrap()
}

fn rollback() -> Rollback {
    Rollback::new(
        EvidenceRef::new("snapshot:legacy-1").unwrap(),
        RollbackPlan::VerifiedRestoration {
            restoration_evidence_ref: EvidenceRef::new("evidence:restore-1").unwrap(),
        },
    )
}

fn unverified() -> Verification {
    Verification::new(
        SubVerdict::Unverified,
        SubVerdict::Unverified,
        Verdict::Unverified,
    )
}

fn build_plan(
    id: &str,
    record_units: Vec<RecordUnit>,
    mappings: Vec<Mapping>,
    source: SourceState,
    verification: Verification,
) -> MigrationPlan {
    let fp = digest("fingerprint-placeholder");
    let ik = digest("idempotency-placeholder");
    MigrationPlan::new(
        sid(id),
        scope(),
        source,
        record_units,
        mappings,
        rollback(),
        verification,
        fp,
        ik,
        None,
    )
    .unwrap()
}

// --- checkCoverage -----------------------------------------------------------

#[test]
fn coverage_rejects_a_unit_with_no_mapping() {
    let units = vec![record_unit("legacy-1"), record_unit("legacy-2")];
    let mappings = vec![Mapping::new(
        sid("legacy-1"),
        Disposition::RetainedTransitional {
            retained_reason: nes("still authoritative"),
        },
    )
    .unwrap()];
    let problems = check_coverage(&units, &mappings);
    assert!(problems
        .iter()
        .any(|d| d.message().contains("legacy-2") && d.message().contains("no mapping")));
}

#[test]
fn coverage_rejects_a_unit_with_two_mappings() {
    let units = vec![record_unit("legacy-1")];
    let mappings = vec![
        Mapping::new(
            sid("legacy-1"),
            Disposition::RetainedTransitional {
                retained_reason: nes("reason a"),
            },
        )
        .unwrap(),
        Mapping::new(
            sid("legacy-1"),
            Disposition::RetainedTransitional {
                retained_reason: nes("reason b"),
            },
        )
        .unwrap(),
    ];
    let problems = check_coverage(&units, &mappings);
    assert!(problems.iter().any(|d| d.message().contains("2 mappings")));
}

#[test]
fn coverage_rejects_a_dangling_mapping_reference() {
    let units = vec![record_unit("legacy-1")];
    let mappings = vec![
        Mapping::new(
            sid("legacy-1"),
            Disposition::RetainedTransitional {
                retained_reason: nes("reason a"),
            },
        )
        .unwrap(),
        Mapping::new(
            sid("legacy-2"),
            Disposition::RetainedTransitional {
                retained_reason: nes("reason b"),
            },
        )
        .unwrap(),
    ];
    let problems = check_coverage(&units, &mappings);
    assert!(problems
        .iter()
        .any(|d| d.message().contains("not a declared record unit")));
}

#[test]
fn coverage_accepts_exactly_one_mapping_per_unit() {
    let units = vec![record_unit("legacy-1"), record_unit("legacy-2")];
    let mappings = vec![
        Mapping::new(
            sid("legacy-1"),
            Disposition::RetainedTransitional {
                retained_reason: nes("reason a"),
            },
        )
        .unwrap(),
        Mapping::new(
            sid("legacy-2"),
            Disposition::RetainedTransitional {
                retained_reason: nes("reason b"),
            },
        )
        .unwrap(),
    ];
    assert!(check_coverage(&units, &mappings).is_empty());
}

// --- checkTargetGroups ---------------------------------------------------------

#[test]
fn target_groups_rejects_undeclared_implicit_merge() {
    let t1 = target(
        "shared-target",
        "owner-decision:x",
        scope(),
        AuthorityKind::ProjectOwner,
    );
    let t2 = target(
        "shared-target",
        "owner-decision:x",
        scope(),
        AuthorityKind::ProjectOwner,
    );
    let mappings = vec![
        Mapping::new(
            sid("legacy-1"),
            Disposition::Migrated {
                target: t1,
                owner_decision: owner_decision("owner-decision:x"),
            },
        )
        .unwrap(),
        Mapping::new(
            sid("legacy-2"),
            Disposition::Migrated {
                target: t2,
                owner_decision: owner_decision("owner-decision:x"),
            },
        )
        .unwrap(),
    ];
    let problems = check_target_groups(&mappings);
    assert!(problems
        .iter()
        .any(|d| d.message().contains("implicit multi-unit")));
}

#[test]
fn target_groups_rejects_a_merge_with_only_one_contributing_unit() {
    let t1 = target(
        "merged-target",
        "owner-decision:x",
        scope(),
        AuthorityKind::ProjectOwner,
    );
    let mappings = vec![Mapping::new(
        sid("legacy-1"),
        Disposition::Merged {
            target: t1,
            owner_decision: owner_decision("owner-decision:x"),
            merge_rule_ref: EvidenceRef::new("merge-rule:billing-consolidation").unwrap(),
        },
    )
    .unwrap()];
    let problems = check_target_groups(&mappings);
    assert!(problems
        .iter()
        .any(|d| d.message().contains("at least two source units")));
}

#[test]
fn target_groups_rejects_differing_merge_rule_ref_within_one_group() {
    let mk = |unit: &str, rule: &str| {
        let t = target(
            "merged-target",
            "owner-decision:x",
            scope(),
            AuthorityKind::ProjectOwner,
        );
        Mapping::new(
            sid(unit),
            Disposition::Merged {
                target: t,
                owner_decision: owner_decision("owner-decision:x"),
                merge_rule_ref: EvidenceRef::new(rule).unwrap(),
            },
        )
        .unwrap()
    };
    let mappings = vec![
        mk("legacy-1", "merge-rule:a"),
        mk("legacy-2", "merge-rule:b"),
    ];
    let problems = check_target_groups(&mappings);
    assert!(problems
        .iter()
        .any(|d| d.message().contains("different merge_rule_ref")));
}

#[test]
fn target_groups_requires_origin_source_ref_to_trace_to_actual_units() {
    let mut t = target(
        "merged-target",
        "owner-decision:x",
        scope(),
        AuthorityKind::ProjectOwner,
    );
    // Replace origin with a fabricated source_ref not derivable from the
    // group's actual unit ids.
    t = TargetRecord::new(
        nes(t.schema_ref()),
        t.id().clone(),
        nes(t.title()),
        t.record_type().clone(),
        t.scope().clone(),
        FieldBasis::new(
            FieldBasisValue::Assigned,
            FieldBasisValue::Preserved,
            FieldBasisValue::Preserved,
            FieldBasisValue::Preserved,
            FieldBasisValue::Assigned,
            FieldBasisValue::Assigned,
            FieldBasisValue::Assigned,
            FieldBasisValue::Preserved,
        ),
        TargetOrigin::new(nes("record-unit:invented")),
        t.authority().clone(),
        t.payload().clone(),
    )
    .unwrap();
    let mappings = vec![Mapping::new(
        sid("legacy-1"),
        Disposition::Migrated {
            target: t,
            owner_decision: owner_decision("owner-decision:x"),
        },
    )
    .unwrap()];
    let problems = check_target_groups(&mappings);
    assert!(problems.iter().any(|d| d
        .message()
        .contains("does not trace to the actual contributing unit")));
}

#[test]
fn target_groups_accepts_a_valid_merge_with_matching_origin() {
    let mk = |unit: &str| {
        let t = TargetRecord::new(
            nes("scoped-record.schema.json"),
            sid("merged-target"),
            nes("Merged norm"),
            sid("norm"),
            scope(),
            FieldBasis::new(
                FieldBasisValue::Assigned,
                FieldBasisValue::Preserved,
                FieldBasisValue::Preserved,
                FieldBasisValue::Preserved,
                FieldBasisValue::Assigned,
                FieldBasisValue::Assigned,
                FieldBasisValue::Assigned,
                FieldBasisValue::Preserved,
            ),
            TargetOrigin::new(nes("record-units:legacy-1,legacy-2")),
            Authority::new(
                AuthorityKind::ProjectOwner,
                "workspace-owner",
                Some("owner-decision:x".to_string()),
            )
            .unwrap(),
            ContentEnvelope::new(
                nes("text/plain"),
                Encoding::Utf8,
                nes("norm text"),
                digest("norm text"),
            )
            .unwrap(),
        )
        .unwrap();
        Mapping::new(
            sid(unit),
            Disposition::Merged {
                target: t,
                owner_decision: owner_decision("owner-decision:x"),
                merge_rule_ref: EvidenceRef::new("merge-rule:billing-consolidation").unwrap(),
            },
        )
        .unwrap()
    };
    let mappings = vec![mk("legacy-1"), mk("legacy-2")];
    assert!(check_target_groups(&mappings).is_empty());
}

// --- checkTargetAuthority ------------------------------------------------------

#[test]
fn target_authority_rejects_delegated_run() {
    let t = target(
        "t1",
        "owner-decision:x",
        scope(),
        AuthorityKind::DelegatedRun,
    );
    let m = Mapping::new(
        sid("legacy-1"),
        Disposition::Migrated {
            target: t,
            owner_decision: owner_decision("owner-decision:x"),
        },
    )
    .unwrap();
    let problems = check_target_authority(&m);
    assert!(problems
        .iter()
        .any(|d| d.message().contains("delegated-run")));
}

#[test]
fn target_authority_rejects_wrong_authority_for_scope() {
    let t = target(
        "t1",
        "owner-decision:x",
        scope(),
        AuthorityKind::RepositoryMaintainer,
    );
    let m = Mapping::new(
        sid("legacy-1"),
        Disposition::Migrated {
            target: t,
            owner_decision: owner_decision("owner-decision:x"),
        },
    )
    .unwrap();
    let problems = check_target_authority(&m);
    assert!(problems
        .iter()
        .any(|d| d.message().contains("requires owner authority")));
}

#[test]
fn target_authority_rejects_run_state_scope() {
    let run_state = Scope::run_state(sid("run-1"), WorkspaceId::new("sample-project").unwrap());
    let t = target(
        "t1",
        "owner-decision:x",
        run_state,
        AuthorityKind::ProjectOwner,
    );
    let m = Mapping::new(
        sid("legacy-1"),
        Disposition::Migrated {
            target: t,
            owner_decision: owner_decision("owner-decision:x"),
        },
    )
    .unwrap();
    let problems = check_target_authority(&m);
    assert!(problems
        .iter()
        .any(|d| d.message().contains("no human owner authority")));
}

#[test]
fn target_authority_accepts_the_matching_owner_for_project_workspace() {
    let t = target(
        "t1",
        "owner-decision:x",
        scope(),
        AuthorityKind::ProjectOwner,
    );
    let m = Mapping::new(
        sid("legacy-1"),
        Disposition::Migrated {
            target: t,
            owner_decision: owner_decision("owner-decision:x"),
        },
    )
    .unwrap();
    assert!(check_target_authority(&m).is_empty());
}

#[test]
fn target_authority_is_a_noop_for_retained_transitional() {
    let m = Mapping::new(
        sid("legacy-1"),
        Disposition::RetainedTransitional {
            retained_reason: nes("kept"),
        },
    )
    .unwrap();
    assert!(check_target_authority(&m).is_empty());
}

// --- checkVerification ----------------------------------------------------------

fn plan_with(source: SourceState, verification: Verification) -> MigrationPlan {
    build_plan(
        "plan-1",
        vec![record_unit("legacy-1")],
        vec![Mapping::new(
            sid("legacy-1"),
            Disposition::RetainedTransitional {
                retained_reason: nes("kept"),
            },
        )
        .unwrap()],
        source,
        verification,
    )
}

#[test]
fn verification_forces_blocked_for_a_not_reproducible_source() {
    let plan = plan_with(source_state("rev-1", false), unverified());
    let fp = compute_plan_fingerprint(&plan);
    let problems = check_verification(&plan, &fp, None, None);
    assert!(problems.iter().any(|d| d.message().contains("BLOCKED")));
}

#[test]
fn verification_rejects_verified_without_both_subverdicts_and_reproducible_source() {
    let verification = Verification::new(
        SubVerdict::Unverified,
        SubVerdict::Unverified,
        Verdict::Verified,
    );
    let plan = plan_with(source_state("rev-1", true), verification);
    let fp = compute_plan_fingerprint(&plan);
    let problems = check_verification(&plan, &fp, None, None);
    assert!(problems.iter().any(|d| d.message().contains("property 6")));
}

#[test]
fn verification_rejects_a_verified_subverdict_with_no_resolved_evidence() {
    let verification = Verification::new(
        SubVerdict::Verified {
            evidence_ref: EvidenceRef::new("evidence:coverage-1").unwrap(),
        },
        SubVerdict::Unverified,
        Verdict::Unverified,
    );
    let plan = plan_with(source_state("rev-1", true), verification);
    let fp = compute_plan_fingerprint(&plan);
    let problems = check_verification(&plan, &fp, None, None);
    assert!(problems
        .iter()
        .any(|d| d.message().contains("does not resolve")));
}

#[test]
fn verification_rejects_evidence_pinned_to_a_stale_fingerprint() {
    let verification = Verification::new(
        SubVerdict::Verified {
            evidence_ref: EvidenceRef::new("evidence:coverage-1").unwrap(),
        },
        SubVerdict::Verified {
            evidence_ref: EvidenceRef::new("evidence:applicability-1").unwrap(),
        },
        Verdict::Verified,
    );
    let plan = plan_with(source_state("rev-1", true), verification);
    let fp = compute_plan_fingerprint(&plan);
    let stale_fp = digest("a completely different, stale plan content");
    let coverage_evidence = ResolvedEvidence {
        evidence_ref: EvidenceRef::new("evidence:coverage-1").unwrap(),
        kind: EvidenceKind::Coverage,
        plan_ref: plan.id().clone(),
        plan_fingerprint: stale_fp,
        confirms: true,
    };
    let applicability_evidence = ResolvedEvidence {
        evidence_ref: EvidenceRef::new("evidence:applicability-1").unwrap(),
        kind: EvidenceKind::ApplicabilityPreservation,
        plan_ref: plan.id().clone(),
        plan_fingerprint: fp.clone(),
        confirms: true,
    };
    let problems = check_verification(
        &plan,
        &fp,
        Some(&coverage_evidence),
        Some(&applicability_evidence),
    );
    assert!(problems.iter().any(|d| d.message().contains("stale")));
}

#[test]
fn verification_accepts_verified_with_matching_resolved_evidence() {
    let verification = Verification::new(
        SubVerdict::Verified {
            evidence_ref: EvidenceRef::new("evidence:coverage-1").unwrap(),
        },
        SubVerdict::Verified {
            evidence_ref: EvidenceRef::new("evidence:applicability-1").unwrap(),
        },
        Verdict::Verified,
    );
    let plan = plan_with(source_state("rev-1", true), verification);
    let fp = compute_plan_fingerprint(&plan);
    let coverage_evidence = ResolvedEvidence {
        evidence_ref: EvidenceRef::new("evidence:coverage-1").unwrap(),
        kind: EvidenceKind::Coverage,
        plan_ref: plan.id().clone(),
        plan_fingerprint: fp.clone(),
        confirms: true,
    };
    let applicability_evidence = ResolvedEvidence {
        evidence_ref: EvidenceRef::new("evidence:applicability-1").unwrap(),
        kind: EvidenceKind::ApplicabilityPreservation,
        plan_ref: plan.id().clone(),
        plan_fingerprint: fp.clone(),
        confirms: true,
    };
    let problems = check_verification(
        &plan,
        &fp,
        Some(&coverage_evidence),
        Some(&applicability_evidence),
    );
    assert!(problems.is_empty());
}

// --- fingerprint / idempotency key: deterministic and field-sensitive --------

#[test]
fn plan_fingerprint_is_deterministic_for_identical_content() {
    let a = plan_with(source_state("rev-1", true), unverified());
    let b = plan_with(source_state("rev-1", true), unverified());
    assert_eq!(compute_plan_fingerprint(&a), compute_plan_fingerprint(&b));
}

#[test]
fn plan_fingerprint_changes_when_source_revision_changes() {
    let a = plan_with(source_state("rev-1", true), unverified());
    let b = plan_with(source_state("rev-2", true), unverified());
    assert_ne!(compute_plan_fingerprint(&a), compute_plan_fingerprint(&b));
}

#[test]
fn plan_fingerprint_is_insensitive_to_record_unit_and_mapping_declaration_order() {
    let units_forward = vec![record_unit("legacy-1"), record_unit("legacy-2")];
    let mappings_forward = vec![
        Mapping::new(
            sid("legacy-1"),
            Disposition::RetainedTransitional {
                retained_reason: nes("a"),
            },
        )
        .unwrap(),
        Mapping::new(
            sid("legacy-2"),
            Disposition::RetainedTransitional {
                retained_reason: nes("b"),
            },
        )
        .unwrap(),
    ];
    let a = build_plan(
        "plan-1",
        units_forward,
        mappings_forward,
        source_state("rev-1", true),
        unverified(),
    );

    let units_reversed = vec![record_unit("legacy-2"), record_unit("legacy-1")];
    let mappings_reversed = vec![
        Mapping::new(
            sid("legacy-2"),
            Disposition::RetainedTransitional {
                retained_reason: nes("b"),
            },
        )
        .unwrap(),
        Mapping::new(
            sid("legacy-1"),
            Disposition::RetainedTransitional {
                retained_reason: nes("a"),
            },
        )
        .unwrap(),
    ];
    let b = build_plan(
        "plan-1",
        units_reversed,
        mappings_reversed,
        source_state("rev-1", true),
        unverified(),
    );

    assert_eq!(compute_plan_fingerprint(&a), compute_plan_fingerprint(&b));
}

#[test]
fn plan_fingerprint_excludes_verification_and_is_insensitive_to_its_change() {
    let a = plan_with(source_state("rev-1", true), unverified());
    let b = plan_with(
        source_state("rev-1", true),
        Verification::new(
            SubVerdict::Unverified,
            SubVerdict::Unverified,
            Verdict::Blocked,
        ),
    );
    assert_eq!(compute_plan_fingerprint(&a), compute_plan_fingerprint(&b));
}

#[test]
fn idempotency_key_is_sensitive_to_repository_ref_not_only_revision() {
    let source_a = SourceState::new(
        Revision::new("v1").unwrap(),
        digest("d"),
        EvidenceRef::new("instance:repo-a").unwrap(),
        true,
        Qualification::Reproducible,
    )
    .unwrap();
    let source_b = SourceState::new(
        Revision::new("v1").unwrap(),
        digest("d"),
        EvidenceRef::new("instance:repo-b").unwrap(),
        true,
        Qualification::Reproducible,
    )
    .unwrap();
    let a = plan_with(source_a, unverified());
    let b = plan_with(source_b, unverified());
    assert_ne!(compute_idempotency_key(&a), compute_idempotency_key(&b));
}

#[test]
fn idempotency_key_is_deterministic_for_the_same_scope_and_source() {
    let a = plan_with(source_state("rev-1", true), unverified());
    let b = plan_with(source_state("rev-1", true), unverified());
    assert_eq!(compute_idempotency_key(&a), compute_idempotency_key(&b));
}

#[test]
fn check_repeatability_reports_a_stale_declared_fingerprint() {
    let source = source_state("rev-1", true);
    let plan = MigrationPlan::new(
        sid("plan-1"),
        scope(),
        source,
        vec![record_unit("legacy-1")],
        vec![Mapping::new(
            sid("legacy-1"),
            Disposition::RetainedTransitional {
                retained_reason: nes("kept"),
            },
        )
        .unwrap()],
        rollback(),
        unverified(),
        digest("this-is-not-the-real-fingerprint"),
        digest("this-is-not-the-real-idempotency-key"),
        None,
    )
    .unwrap();
    let problems = check_repeatability(&plan);
    assert_eq!(problems.len(), 2);
}

// --- checkIdempotency (container-level) ---------------------------------------

#[test]
fn idempotency_uniqueness_rejects_two_plans_sharing_a_declared_key() {
    let shared_key = digest("shared-key");
    let a = MigrationPlan::new(
        sid("plan-a"),
        scope(),
        source_state("rev-1", true),
        vec![record_unit("legacy-1")],
        vec![Mapping::new(
            sid("legacy-1"),
            Disposition::RetainedTransitional {
                retained_reason: nes("kept"),
            },
        )
        .unwrap()],
        rollback(),
        unverified(),
        digest("fp-a"),
        shared_key.clone(),
        None,
    )
    .unwrap();
    let b = MigrationPlan::new(
        sid("plan-b"),
        scope(),
        source_state("rev-2", true),
        vec![record_unit("legacy-1")],
        vec![Mapping::new(
            sid("legacy-1"),
            Disposition::RetainedTransitional {
                retained_reason: nes("kept"),
            },
        )
        .unwrap()],
        rollback(),
        unverified(),
        digest("fp-b"),
        shared_key,
        None,
    )
    .unwrap();
    let problems = check_idempotency_uniqueness([&a, &b]);
    assert_eq!(problems.len(), 1);
}

// --- checkSupersedes -------------------------------------------------------------

#[test]
fn supersedes_rejects_self_reference() {
    let plan = MigrationPlan::new(
        sid("plan-1"),
        scope(),
        source_state("rev-2", true),
        vec![record_unit("legacy-1")],
        vec![Mapping::new(
            sid("legacy-1"),
            Disposition::RetainedTransitional {
                retained_reason: nes("kept"),
            },
        )
        .unwrap()],
        rollback(),
        unverified(),
        digest("fp"),
        digest("ik"),
        Some(nes("plan-1")),
    )
    .unwrap();
    let problems = check_supersedes(std::slice::from_ref(&plan), |_| None);
    assert!(problems
        .iter()
        .any(|d| d.message().contains("cannot supersede itself")));
}

#[test]
fn supersedes_rejects_the_same_revision_as_the_predecessor() {
    let predecessor = MigrationPlan::new(
        sid("plan-0"),
        scope(),
        source_state("rev-1", true),
        vec![record_unit("legacy-1")],
        vec![Mapping::new(
            sid("legacy-1"),
            Disposition::RetainedTransitional {
                retained_reason: nes("kept"),
            },
        )
        .unwrap()],
        rollback(),
        unverified(),
        digest("fp0"),
        digest("ik0"),
        None,
    )
    .unwrap();
    let successor = MigrationPlan::new(
        sid("plan-1"),
        scope(),
        source_state("rev-1", true),
        vec![record_unit("legacy-1")],
        vec![Mapping::new(
            sid("legacy-1"),
            Disposition::RetainedTransitional {
                retained_reason: nes("kept"),
            },
        )
        .unwrap()],
        rollback(),
        unverified(),
        digest("fp1"),
        digest("ik1"),
        Some(nes("plan-0")),
    )
    .unwrap();
    let plans = vec![predecessor, successor];
    let problems = check_supersedes(&plans, |_| None);
    assert!(problems.iter().any(|d| d.message().contains("identical")));
}

#[test]
fn supersedes_accepts_a_valid_in_document_predecessor() {
    let predecessor = MigrationPlan::new(
        sid("plan-0"),
        scope(),
        source_state("rev-1", true),
        vec![record_unit("legacy-1")],
        vec![Mapping::new(
            sid("legacy-1"),
            Disposition::RetainedTransitional {
                retained_reason: nes("kept"),
            },
        )
        .unwrap()],
        rollback(),
        unverified(),
        digest("fp0"),
        digest("ik0"),
        None,
    )
    .unwrap();
    let successor = MigrationPlan::new(
        sid("plan-1"),
        scope(),
        source_state("rev-2", true),
        vec![record_unit("legacy-1")],
        vec![Mapping::new(
            sid("legacy-1"),
            Disposition::RetainedTransitional {
                retained_reason: nes("kept"),
            },
        )
        .unwrap()],
        rollback(),
        unverified(),
        digest("fp1"),
        digest("ik1"),
        Some(nes("plan-0")),
    )
    .unwrap();
    let plans = vec![predecessor, successor];
    let problems = check_supersedes(&plans, |_| None);
    assert!(problems.is_empty());
}

#[test]
fn supersedes_never_accepts_an_unresolved_external_predecessor() {
    let successor = MigrationPlan::new(
        sid("plan-1"),
        scope(),
        source_state("rev-2", true),
        vec![record_unit("legacy-1")],
        vec![Mapping::new(
            sid("legacy-1"),
            Disposition::RetainedTransitional {
                retained_reason: nes("kept"),
            },
        )
        .unwrap()],
        rollback(),
        unverified(),
        digest("fp1"),
        digest("ik1"),
        Some(nes("plan-not-in-this-batch")),
    )
    .unwrap();
    let problems = check_supersedes(std::slice::from_ref(&successor), |_| None);
    assert!(problems
        .iter()
        .any(|d| d.message().contains("never accepted automatically")));
}

#[test]
fn supersedes_accepts_a_matching_externally_resolved_predecessor() {
    let successor = MigrationPlan::new(
        sid("plan-1"),
        scope(),
        source_state("rev-2", true),
        vec![record_unit("legacy-1")],
        vec![Mapping::new(
            sid("legacy-1"),
            Disposition::RetainedTransitional {
                retained_reason: nes("kept"),
            },
        )
        .unwrap()],
        rollback(),
        unverified(),
        digest("fp1"),
        digest("ik1"),
        Some(nes("plan-0")),
    )
    .unwrap();
    let resolved = ResolvedSupersededPlan {
        plan_ref: sid("plan-0"),
        scope: scope(),
        repository_ref: EvidenceRef::new("instance:cbs").unwrap(),
        revision: Revision::new("rev-1").unwrap(),
    };
    let problems = check_supersedes(std::slice::from_ref(&successor), |q| {
        if q == "plan-0" {
            Some(resolved.clone())
        } else {
            None
        }
    });
    assert!(problems.is_empty());
}

// --- reproducibility / reversibility resolver-check parity --------------------

#[test]
fn reproducibility_rejects_a_snapshot_resolved_for_a_different_repository() {
    let plan = plan_with(source_state("rev-1", true), unverified());
    let resolved = ResolvedSourceSnapshot {
        repository_ref: EvidenceRef::new("instance:some-other-repo").unwrap(),
        revision: Revision::new("rev-1").unwrap(),
        digest: digest("source-content"),
        working_tree_clean: true,
    };
    let problems = check_reproducibility(&plan, Some(&resolved));
    assert!(problems
        .iter()
        .any(|d| d.message().contains("different repository")));
}

#[test]
fn reproducibility_accepts_a_matching_snapshot() {
    let plan = plan_with(source_state("rev-1", true), unverified());
    let resolved = ResolvedSourceSnapshot {
        repository_ref: EvidenceRef::new("instance:cbs").unwrap(),
        revision: Revision::new("rev-1").unwrap(),
        digest: digest("source-content"),
        working_tree_clean: true,
    };
    assert!(check_reproducibility(&plan, Some(&resolved)).is_empty());
}

#[test]
fn reversibility_rejects_a_deterministic_plan_pinned_to_a_stale_fingerprint() {
    let plan = build_plan(
        "plan-1",
        vec![record_unit("legacy-1")],
        vec![Mapping::new(
            sid("legacy-1"),
            Disposition::RetainedTransitional {
                retained_reason: nes("kept"),
            },
        )
        .unwrap()],
        source_state("rev-1", true),
        unverified(),
    );
    let plan = MigrationPlan::new(
        plan.id().clone(),
        plan.scope().clone(),
        plan.source().clone(),
        plan.record_units().to_vec(),
        plan.mappings().to_vec(),
        Rollback::new(
            EvidenceRef::new("snapshot:legacy-1").unwrap(),
            RollbackPlan::DeterministicReconstruction {
                deterministic_plan_ref: EvidenceRef::new("plan:reconstruction-1").unwrap(),
            },
        ),
        plan.verification().clone(),
        plan.declared_plan_fingerprint().clone(),
        plan.declared_idempotency_key().clone(),
        None,
    )
    .unwrap();
    let fp = compute_plan_fingerprint(&plan);
    let resolved_snapshot = ResolvedRollbackSnapshot {
        source_snapshot_ref: EvidenceRef::new("snapshot:legacy-1").unwrap(),
        repository_ref: EvidenceRef::new("instance:cbs").unwrap(),
        revision: Revision::new("rev-1").unwrap(),
        digest: digest("source-content"),
    };
    let resolved_plan = ResolvedDeterministicPlan {
        deterministic_plan_ref: EvidenceRef::new("plan:reconstruction-1").unwrap(),
        source_snapshot_ref: EvidenceRef::new("snapshot:legacy-1").unwrap(),
        plan_ref: plan.id().clone(),
        plan_fingerprint: digest("stale"),
        applicable: true,
    };
    let problems = check_reversibility(
        &plan,
        &fp,
        Some(&resolved_snapshot),
        Some(&resolved_plan),
        None,
    );
    assert!(problems
        .iter()
        .any(|d| d.message().contains("stale or different version")));
}
