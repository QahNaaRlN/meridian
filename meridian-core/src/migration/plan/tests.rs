#![cfg(test)]
//! Direct tests of the nine migration-plan properties through the one
//! production path, [`check_migration_plans`], over typed inputs.

use super::check::derive_origin_source_ref;
use super::*;
use crate::migration::content::Encoding;
use crate::migration::record::RecordHead;
use crate::migration::resolved::{
    DigestResponse, EvidenceResponse, RestorationEvidenceResponse, RollbackSnapshotResponse,
    SourceSnapshotResponse, SupersededPlanResponse,
};
use crate::resolver::IsoDate;
use crate::run_contracts::{
    PortableRef, RecordAuthority, RecordOrigin, RecordText, ResponseField, SourcedOriginKind,
};
use crate::types::{AuthorityKind, OriginKind, SemanticId, Verdict, WorkspaceId};

pub(crate) fn text(s: &str) -> RecordText {
    RecordText::new(s).unwrap()
}
pub(crate) fn sid(s: &str) -> SemanticId {
    SemanticId::new(s).unwrap()
}
fn present(s: &str) -> ResponseField<String> {
    ResponseField::Present(s.to_string())
}

pub(crate) fn head(
    id: &str,
    record_type: &str,
    origin: OriginKind,
    source_ref: &str,
) -> RecordHead {
    RecordHead {
        declared_schema: "../../registries/operating-model/scoped-record.schema.json".to_string(),
        id: sid(id),
        title: text("Запись"),
        record_type: sid(record_type),
        scope: Scope::project_workspace(sid("sample-project"), None),
        origin: RecordOrigin::Sourced {
            kind: SourcedOriginKind::new(origin).unwrap(),
            source_ref: PortableRef::new(source_ref).unwrap(),
        },
        authority: RecordAuthority {
            kind: AuthorityKind::DelegatedRun,
            authority_ref: PortableRef::new("run:sample").unwrap(),
            decision_ref: None,
        },
    }
}

pub(crate) fn content(body: &str) -> DeclaredContent {
    DeclaredContent {
        media_type: text("text/plain"),
        encoding: Encoding::Utf8,
        content: text(body),
        digest: ContentDigest::of_str(body),
    }
}

pub(crate) fn target(id: &str, units: &[&str]) -> TargetInput {
    let origin = derive_origin_source_ref(units);
    TargetInput {
        schema_ref: text("../../registries/operating-model/scoped-record.schema.json"),
        id: sid(id),
        title: text("Перенесённая запись"),
        record_type: sid("norm"),
        scope: Scope::project_workspace(sid("sample-project"), None),
        field_basis: FieldBasisInput {
            schema: FieldBasisValue::Assigned,
            id: FieldBasisValue::Assigned,
            title: FieldBasisValue::Assigned,
            record_type: FieldBasisValue::Preserved,
            scope: FieldBasisValue::Assigned,
            schema_version: FieldBasisValue::Assigned,
            origin: FieldBasisValue::Assigned,
            authority: FieldBasisValue::Assigned,
            payload: FieldBasisValue::Assigned,
        },
        origin_source_ref: text(&origin),
        authority: TargetAuthorityInput {
            kind: AuthorityKind::ProjectOwner,
            authority_ref: text("sample-owner"),
            decision_ref: text(&format!("owner-decision:{id}")),
        },
        payload: content(&format!("text of {id}")),
    }
}

fn decision(target_id: &str) -> OwnerDecisionInput {
    OwnerDecisionInput {
        decision_ref: text(&format!("owner-decision:{target_id}")),
        decided_at: IsoDate::new("2026-09-08").unwrap(),
        reason: text("accepted"),
    }
}

pub(crate) fn migrated(unit: &str, target_id: &str) -> MappingInput {
    MappingInput {
        unit_id: sid(unit),
        disposition: DispositionInput::Migrated {
            target: target(target_id, &[unit]),
            owner_decision: decision(target_id),
        },
    }
}

pub(crate) fn unit(id: &str) -> RecordUnitInput {
    RecordUnitInput {
        id: sid(id),
        unit_ref: text(&format!("sample/{id}.yaml")),
        classification_basis: text(&format!("basis of {id}")),
    }
}

pub(crate) const PLAN: &str = "sample-plan";
pub(crate) const REPO: &str = "sample-repository";
pub(crate) const REV: &str = "sample-revision";
pub(crate) const SNAPSHOT: &str = "snapshot:sample";
pub(crate) const RESTORE: &str = "evidence:restore";

pub(crate) fn source_digest() -> ContentDigest {
    ContentDigest::of_str("source")
}

/// A plan with RECOMPUTED fingerprint and key, one migrated unit and one
/// retained one, fully verified.
pub(crate) fn plan() -> PlanInput {
    let mut input = PlanInput {
        head: head(PLAN, RECORD_TYPE, OriginKind::Declared, "declared:sample"),
        payload: PlanPayloadInput {
            source: SourceInput {
                revision: text(REV),
                digest: source_digest(),
                repository_ref: text(REPO),
                working_tree_clean: true,
                qualification: Qualification::Reproducible,
            },
            record_units: vec![unit("unit-a"), unit("unit-b")],
            mappings: vec![
                migrated("unit-a", "target-a"),
                MappingInput {
                    unit_id: sid("unit-b"),
                    disposition: DispositionInput::RetainedTransitional {
                        retained_reason: text("still authoritative"),
                    },
                },
            ],
            rollback: RollbackInput {
                source_snapshot_ref: text(SNAPSHOT),
                plan: RollbackPlanInput::VerifiedRestoration {
                    restoration_evidence_ref: text(RESTORE),
                },
            },
            verification: VerificationInput {
                coverage: SubVerdictInput::Verified {
                    evidence_ref: text("evidence:coverage"),
                },
                applicability_preservation: SubVerdictInput::Verified {
                    evidence_ref: text("evidence:applicability"),
                },
                overall_status: Verdict::Verified,
            },
            plan_fingerprint: source_digest(),
            idempotency_key: source_digest(),
            supersedes: None,
        },
    };
    recompute(&mut input);
    input
}

pub(crate) fn recompute(input: &mut PlanInput) {
    input.payload.plan_fingerprint = compute_plan_fingerprint(&input.payload);
    input.payload.idempotency_key =
        compute_idempotency_key(&input.head.scope, &input.payload.source);
}

fn digest_response(d: &ContentDigest) -> ResponseField<DigestResponse> {
    ResponseField::Present(DigestResponse {
        algorithm: present("sha-256"),
        value: present(d.value()),
        unknown_keys: vec![],
    })
}

/// Every external fact [`plan`] needs, pinned to `input`'s fingerprint.
pub(crate) fn resolution_for(input: &PlanInput) -> PlanResolution {
    let fp = compute_plan_fingerprint(&input.payload);
    let mut r = PlanResolution::default();
    r.source_snapshots.insert(
        format!("{REPO}@{REV}"),
        SourceSnapshotResponse {
            record_type: present("instance-source-snapshot"),
            repository_ref: present(REPO),
            revision: present(REV),
            digest: digest_response(&source_digest()),
            working_tree_clean: ResponseField::Present(true),
            unknown_keys: vec![],
        },
    );
    r.rollback_snapshots.insert(
        SNAPSHOT,
        RollbackSnapshotResponse {
            record_type: present("instance-rollback-snapshot"),
            source_snapshot_ref: present(SNAPSHOT),
            repository_ref: present(REPO),
            revision: present(REV),
            digest: digest_response(&source_digest()),
            unknown_keys: vec![],
        },
    );
    r.restoration_evidence.insert(
        RESTORE,
        RestorationEvidenceResponse {
            record_type: present("instance-restoration-evidence"),
            evidence_ref: present(RESTORE),
            plan_ref: present(PLAN),
            plan_fingerprint: present(fp.value()),
            source_snapshot_ref: present(SNAPSHOT),
            confirms: ResponseField::Present(true),
            unknown_keys: vec![],
        },
    );
    for (reference, kind) in [
        ("evidence:coverage", "coverage"),
        ("evidence:applicability", "applicability_preservation"),
    ] {
        r.evidence.insert(
            reference,
            EvidenceResponse {
                record_type: present("instance-migration-evidence"),
                evidence_ref: present(reference),
                kind: present(kind),
                plan_ref: present(PLAN),
                plan_fingerprint: present(fp.value()),
                confirms: ResponseField::Present(true),
                unknown_keys: vec![],
            },
        );
    }
    r
}

fn problems(input: PlanInput) -> Vec<String> {
    let resolution = resolution_for(&input);
    problems_with(vec![input], &resolution)
}

fn problems_with(entries: Vec<PlanInput>, resolution: &PlanResolution) -> Vec<String> {
    check_migration_plans(&entries, resolution)
        .diagnostics
        .iter()
        .map(|d| d.message().to_string())
        .collect()
}

fn has(found: &[String], needle: &str) -> bool {
    found.iter().any(|m| m.contains(needle))
}

#[test]
fn migration_a_complete_verified_plan_is_accepted_with_its_recomputed_pins() {
    let input = plan();
    let outcome = check_migration_plans(std::slice::from_ref(&input), &resolution_for(&input));
    assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    let accepted = outcome.accepted[0].as_ref().unwrap();
    assert_eq!(accepted.fingerprint(), &input.payload.plan_fingerprint);
    assert!(accepted.mints_records());
    assert_eq!(accepted.overall_status(), Verdict::Verified);
}

#[test]
fn migration_property_1_and_8_a_path_shaped_ref_is_rejected() {
    let mut input = plan();
    input.payload.rollback.source_snapshot_ref = text("file:///tmp/snapshot");
    recompute(&mut input);
    assert!(has(
        &problems(input),
        "rollback.source_snapshot_ref \"file:///tmp/snapshot\" is a file:// URL"
    ));
}

#[test]
fn migration_property_2_reproducibility_needs_a_matching_snapshot() {
    let input = plan();
    let mut resolution = resolution_for(&input);
    resolution.source_snapshots = Default::default();
    assert!(has(
        &problems_with(vec![input.clone()], &resolution),
        "does not resolve to a known source snapshot"
    ));
    let mut resolution = resolution_for(&input);
    let mut snapshot = resolution
        .source_snapshots
        .resolve(&format!("{REPO}@{REV}"))
        .cloned()
        .unwrap();
    snapshot.repository_ref = present("other-repository");
    snapshot.unknown_keys = vec!["extra".to_string()];
    resolution
        .source_snapshots
        .insert(format!("{REPO}@{REV}"), snapshot);
    let found = problems_with(vec![input], &resolution);
    assert!(has(&found, "unknown field \"extra\""));
    assert!(has(
        &found,
        "does not match the resolved snapshot's repository_ref \"other-repository\""
    ));
}

#[test]
fn migration_property_3_coverage_is_exactly_one_mapping_per_unit() {
    let mut input = plan();
    input.payload.record_units.push(unit("unit-c"));
    input.payload.mappings.push(migrated("unit-z", "target-z"));
    input.payload.record_units[0].classification_basis =
        input.payload.record_units[0].unit_ref.clone();
    recompute(&mut input);
    let found = problems(input);
    assert!(has(
        &found,
        "record unit \"unit-a\" classification_basis equals unit_ref verbatim"
    ));
    assert!(has(&found, "mapping references unit_id \"unit-z\""));
    assert!(has(&found, "record unit \"unit-c\" has no mapping"));
}

#[test]
fn migration_property_3_and_4_target_groups_are_checked_in_first_seen_order() {
    let mut input = plan();
    input.payload.record_units = vec![unit("unit-a"), unit("unit-b"), unit("unit-c")];
    input.payload.mappings = vec![
        migrated("unit-c", "target-z"),
        migrated("unit-a", "target-a"),
        migrated("unit-b", "target-a"),
    ];
    input.payload.mappings[2] = MappingInput {
        unit_id: sid("unit-b"),
        disposition: DispositionInput::Migrated {
            target: target("target-z", &["unit-b"]),
            owner_decision: decision("target-z"),
        },
    };
    recompute(&mut input);
    let found = problems(input);
    let first = found
        .iter()
        .position(|m| m.contains("target \"target-z\" is claimed by 2 \"migrated\" mappings"))
        .unwrap();
    assert!(first < found.len());
}

#[test]
fn migration_property_4_merge_rules_origin_and_field_basis_are_checked() {
    let mut input = plan();
    let merged = |unit: &str, rule: &str| MappingInput {
        unit_id: sid(unit),
        disposition: DispositionInput::Merged {
            target: target("target-m", &["unit-a", "unit-b"]),
            owner_decision: decision("target-m"),
            merge_rule_ref: text(rule),
        },
    };
    input.payload.mappings = vec![
        merged("unit-a", "merge-rule:one"),
        merged("unit-b", "merge-rule:two"),
    ];
    if let DispositionInput::Merged { target, .. } = &mut input.payload.mappings[1].disposition {
        target.field_basis.origin = FieldBasisValue::Preserved;
    }
    recompute(&mut input);
    let found = problems(input);
    assert!(has(
        &found,
        "different merge_rule_ref values (merge-rule:one, merge-rule:two)"
    ));
    assert!(has(&found, "is described inconsistently"));
    assert!(has(
        &found,
        "field_basis.origin is \"preserved\", not \"assigned\""
    ));
}

#[test]
fn migration_property_4_a_single_unit_merge_and_a_false_origin_are_rejected() {
    let mut input = plan();
    input.payload.mappings[0] = MappingInput {
        unit_id: sid("unit-a"),
        disposition: DispositionInput::Merged {
            target: target("target-m", &["unit-q"]),
            owner_decision: decision("target-m"),
            merge_rule_ref: text("merge-rule:one"),
        },
    };
    recompute(&mut input);
    assert!(has(
        &problems(input),
        "merge target \"target-m\" is claimed by only 1 mapping(s)"
    ));
    let mut input = plan();
    if let DispositionInput::Migrated { target, .. } = &mut input.payload.mappings[0].disposition {
        target.origin_source_ref = text("record-unit:unit-z");
    }
    recompute(&mut input);
    assert!(has(
        &problems(input),
        "expected \"record-unit:unit-a\" (property 4)"
    ));
}

#[test]
fn migration_property_4_authority_is_the_owner_of_the_targets_own_scope() {
    let mut input = plan();
    if let DispositionInput::Migrated {
        target,
        owner_decision,
    } = &mut input.payload.mappings[0].disposition
    {
        target.authority.kind = AuthorityKind::DelegatedRun;
        owner_decision.decision_ref = text("owner-decision:other");
        target.payload.digest = ContentDigest::of_str("not the content");
    }
    recompute(&mut input);
    let found = problems(input);
    assert!(has(&found, "authority.kind is \"delegated-run\""));
    assert!(has(
        &found,
        "does not match mapping.owner_decision.decision_ref"
    ));
    assert!(has(&found, "digest.value does not match the SHA-256"));
    let mut input = plan();
    if let DispositionInput::Migrated { target, .. } = &mut input.payload.mappings[0].disposition {
        target.scope = Scope::run_state(sid("run-1"), WorkspaceId::new("sample-project").unwrap());
    }
    recompute(&mut input);
    assert!(has(
        &problems(input),
        "scope.type is \"run-state\"; a migrated or merged record cannot be classified"
    ));
}

#[test]
fn migration_property_5_rollback_is_resolved_and_linked_to_this_plan() {
    let input = plan();
    let mut resolution = resolution_for(&input);
    let mut evidence = resolution
        .restoration_evidence
        .resolve(RESTORE)
        .cloned()
        .unwrap();
    evidence.plan_fingerprint = present(&"0".repeat(64));
    evidence.confirms = ResponseField::Present(false);
    resolution.restoration_evidence.insert(RESTORE, evidence);
    let found = problems_with(vec![input], &resolution);
    assert!(has(
        &found,
        "restoration evidence pinned to a stale or different version"
    ));
    assert!(has(&found, "does not confirm restoration"));
}

#[test]
fn migration_property_6_a_claimed_verdict_needs_resolved_confirming_evidence() {
    let mut input = plan();
    input.payload.verification.coverage = SubVerdictInput::Unverified;
    recompute(&mut input);
    assert!(has(
        &problems(input),
        "verification.overall_status is \"VERIFIED\" but coverage is not verified"
    ));
    let mut input = plan();
    input.payload.source.qualification = Qualification::NotReproducible {
        reason: text("dirty"),
    };
    input.payload.source.working_tree_clean = false;
    input.payload.verification.overall_status = Verdict::Unverified;
    recompute(&mut input);
    assert!(has(
        &problems(input),
        "and the plan is BLOCKED, not merely unverified"
    ));
    let input = plan();
    let mut resolution = resolution_for(&input);
    let mut evidence = resolution
        .evidence
        .resolve("evidence:coverage")
        .cloned()
        .unwrap();
    evidence.kind = present("applicability_preservation");
    evidence.record_type = present("instance-restoration-evidence");
    resolution.evidence.insert("evidence:coverage", evidence);
    let found = problems_with(vec![input], &resolution);
    assert!(has(
        &found,
        "resolved record_type is \"instance-restoration-evidence\""
    ));
    assert!(has(
        &found,
        "evidence for one sub-verdict cannot confirm another"
    ));
}

#[test]
fn migration_property_7_stale_pins_and_duplicate_keys_are_rejected() {
    let mut input = plan();
    input.payload.plan_fingerprint = ContentDigest::of_str("stale");
    input.payload.idempotency_key = ContentDigest::of_str("arbitrary");
    let found = problems(input);
    assert!(has(&found, "does not match the recomputed fingerprint"));
    assert!(has(
        &found,
        "idempotency_key is never an arbitrary free-form string"
    ));

    let a = plan();
    let mut b = plan();
    b.head.id = sid("sample-plan-copy");
    let resolution = resolution_for(&a);
    let outcome = check_migration_plans(&[a, b], &resolution);
    assert!(outcome.diagnostics.iter().any(|d| d.message().contains(
        "is declared by more than one migration plan (\"sample-plan\" and \"sample-plan-copy\")"
    )));
}

#[test]
fn migration_property_7_supersedes_is_checked_present_or_resolved() {
    let mut input = plan();
    input.payload.supersedes = Some(text(PLAN));
    assert!(has(&problems(input), "supersedes its own id"));

    let mut input = plan();
    input.payload.supersedes = Some(text("sample-predecessor"));
    let mut resolution = resolution_for(&input);
    assert!(has(
        &problems_with(vec![input.clone()], &resolution),
        "does not resolve to a known predecessor"
    ));
    resolution.superseded_plans.insert(
        "sample-predecessor",
        SupersededPlanResponse {
            record_type: present("instance-superseded-plan"),
            plan_ref: present("sample-predecessor"),
            scope: ResponseField::Absent,
            repository_ref: present(REPO),
            revision: present(REV),
            unknown_keys: vec![],
        },
    );
    let found = problems_with(vec![input], &resolution);
    assert!(has(
        &found,
        "the resolved predecessor is scoped differently"
    ));
    assert!(has(&found, "pins the identical source.revision"));
}

#[test]
fn migration_envelope_kind_and_scope_are_domain_rules_not_conversion_failures() {
    let mut input = plan();
    input.head.scope = Scope::built_in_methodology();
    input.head.authority.kind = AuthorityKind::ProjectOwner;
    recompute(&mut input);
    let found = problems(input);
    assert!(has(
        &found,
        "a migration plan is scoped to project-workspace or repository-scope only"
    ));
    assert!(has(
        &found,
        "authority.kind is \"project-owner\", not \"delegated-run\""
    ));
}

/// A defect of one plan never hides or blocks its independent neighbour.
#[test]
fn migration_a_neighbours_defect_does_not_reject_an_independent_plan() {
    let good = plan();
    let mut bad = plan();
    bad.head.id = sid("sample-plan-bad");
    bad.payload.source.revision = text("other-revision");
    recompute(&mut bad);
    let outcome = check_migration_plans(&[good, bad], &resolution_for(&plan()));
    assert!(outcome.accepted[0].is_some());
    assert!(outcome.accepted[1].is_none());
    assert!(!outcome.diagnostics.is_empty());
}

#[test]
fn migration_a_pinned_plan_is_confirmed_only_by_its_recomputed_fingerprint() {
    let input = plan();
    let fingerprint = input.payload.plan_fingerprint.value().to_string();
    assert!(PinnedPlan::confirm(input.clone(), &fingerprint).is_ok());
    let recomputed = PinnedPlan::confirm(input, &"0".repeat(64)).unwrap_err();
    assert_eq!(recomputed.value(), fingerprint);
}
