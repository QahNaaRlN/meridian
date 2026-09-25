#![cfg(test)]
//! Direct tests of the canonical-export contract through the one
//! production path, [`check_canonical_exports`], over typed inputs — and
//! the proof that a pinned plan boundary cannot be substituted.

use super::*;
use crate::canonical::CanonicalJson;
use crate::migration::plan::tests::{
    head, plan, recompute, sid, source_digest, text, PLAN, REPO, REV,
};
use crate::migration::plan::{PinnedPlan, PlanInput};
use crate::migration::resolved::{OpenObject, SourceContentResponse};
use crate::run_contracts::{
    PortableRef, RecordOrigin, ResponseField, ResponseItem, SourcedOriginKind,
};
use crate::types::{OriginKind, Scope};

fn present(s: &str) -> ResponseField<String> {
    ResponseField::Present(s.to_string())
}

fn string(s: &str) -> CanonicalJson {
    CanonicalJson::string(s)
}

/// The exported record of `plan()`'s one migrated target, built from the
/// target itself.
fn exported(input: &PlanInput) -> ExportedRecordInput {
    let target = input.payload.mappings[0].target().unwrap();
    let body = target.payload.content.as_str();
    let digest = &target.payload.digest;
    let payload = OpenObject::new(vec![
        ("media_type".to_string(), string("text/plain")),
        ("encoding".to_string(), string("utf-8")),
        ("content".to_string(), string(body)),
        (
            "digest".to_string(),
            CanonicalJson::object(vec![
                ("algorithm".to_string(), string("sha-256")),
                ("value".to_string(), string(digest.value())),
            ]),
        ),
    ]);
    let mut record_head = head(
        target.id.as_str(),
        target.record_type.as_str(),
        OriginKind::Migrated,
        target.origin_source_ref.as_str(),
    );
    record_head.declared_schema = target.schema_ref.as_str().to_string();
    record_head.title = target.title.clone();
    record_head.authority = crate::run_contracts::RecordAuthority {
        kind: target.authority.kind,
        authority_ref: PortableRef::new(target.authority.authority_ref.as_str()).unwrap(),
        decision_ref: Some(PortableRef::new(target.authority.decision_ref.as_str()).unwrap()),
    };
    ExportedRecordInput {
        head: record_head,
        payload: ExportedPayload::new(
            payload,
            crate::migration::content::EnvelopeFields::declared(
                "text/plain",
                crate::migration::content::Encoding::Utf8,
                body,
                digest,
            ),
        ),
    }
}

fn export_of(input: &PlanInput) -> ExportInput {
    let mut export_head = head(
        "sample-export",
        "instance-canonical-export",
        OriginKind::Derived,
        &format!("migration-plan:{PLAN}"),
    );
    export_head.origin = RecordOrigin::Sourced {
        kind: SourcedOriginKind::new(OriginKind::Derived).unwrap(),
        source_ref: PortableRef::new(format!("migration-plan:{PLAN}")).unwrap(),
    };
    let fingerprint = input.payload.plan_fingerprint.clone();
    let mut export = ExportInput {
        head: export_head,
        payload: ExportPayloadInput {
            plan_ref: text(PLAN),
            plan_fingerprint: fingerprint.clone(),
            source: ExportSourceInput {
                repository_ref: text(REPO),
                revision: text(REV),
                digest: source_digest(),
            },
            records: vec![exported(input)],
            retained: vec![RetainedInput {
                unit_id: sid("unit-b"),
                reason: text("still authoritative"),
            }],
            idempotency_key: compute_export_idempotency_key(PLAN, fingerprint.value()),
            digest: fingerprint,
        },
    };
    export.payload.digest = compute_export_digest(&export.payload);
    export
}

fn content_for(input: &PlanInput) -> ResponseCatalogue<SourceContentResponse> {
    let target = input.payload.mappings[0].target().unwrap();
    let mut catalogue = ResponseCatalogue::new();
    let unit_refs = vec!["sample/unit-a.yaml".to_string()];
    catalogue.insert(
        compute_source_content_key(REPO, REV, &unit_refs, None),
        SourceContentResponse {
            record_type: present("instance-source-content"),
            repository_ref: present(REPO),
            revision: present(REV),
            unit_refs: ResponseField::Present(vec![ResponseItem::Text(unit_refs[0].clone())]),
            merge_rule_ref: ResponseField::Absent,
            media_type: present("text/plain"),
            encoding: present("utf-8"),
            content: present(target.payload.content.as_str()),
            digest: ResponseField::Present(crate::migration::resolved::DigestResponse {
                algorithm: present("sha-256"),
                value: present(target.payload.digest.value()),
                unknown_keys: vec![],
            }),
            unknown_keys: vec![],
        },
    );
    catalogue
}

fn pinned(input: &PlanInput) -> PinnedPlan {
    PinnedPlan::confirm(input.clone(), input.payload.plan_fingerprint.value()).unwrap()
}

fn problems(export: ExportInput, boundary: PlanBoundary<'_>, input: &PlanInput) -> Vec<String> {
    check_canonical_exports(&[export], boundary, &content_for(input))
        .diagnostics
        .iter()
        .map(|d| d.message().to_string())
        .collect()
}

fn has(found: &[String], needle: &str) -> bool {
    found.iter().any(|m| m.contains(needle))
}

#[test]
fn migration_export_of_a_pinned_plan_is_accepted_with_its_recomputed_digest() {
    let input = plan();
    let plan = pinned(&input);
    let export = export_of(&input);
    let outcome = check_canonical_exports(
        std::slice::from_ref(&export),
        PlanBoundary::Pinned(&plan),
        &content_for(&input),
    );
    assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    assert_eq!(
        outcome.accepted[0].as_ref().unwrap().digest(),
        &export.payload.digest
    );
}

/// Plan substitution is constructively impossible: a resolver catalogue
/// mapping the SAME plan id to a different plan rejects the export, while
/// the pinned boundary checks it against exactly the pinned content — and
/// resolves no other id at all.
#[test]
fn migration_export_a_pinned_boundary_cannot_be_substituted_under_the_same_id() {
    let input = plan();
    let export = export_of(&input);
    let mut substitute = input.clone();
    substitute.payload.record_units[1].classification_basis = text("a different basis");
    recompute(&mut substitute);
    let substitute_plan = pinned(&substitute);
    let mut catalogue = ResponseCatalogue::new();
    catalogue.insert(PLAN, pinned_plan_response(&substitute_plan));
    let through_substitute = problems(export.clone(), PlanBoundary::Resolver(&catalogue), &input);
    assert!(has(
        &through_substitute,
        "an export pinned to a stale or different version"
    ));

    let original = pinned(&input);
    assert!(problems(export.clone(), PlanBoundary::Pinned(&original), &input).is_empty());

    let mut other_id = export;
    other_id.payload.plan_ref = text("another-plan");
    assert!(has(
        &problems(other_id, PlanBoundary::Pinned(&original), &input),
        "plan_ref \"another-plan\" does not resolve to a known migration plan"
    ));
}

#[test]
fn migration_export_completeness_rejects_missing_extra_and_misreasoned_entries() {
    let input = plan();
    let plan = pinned(&input);
    let mut export = export_of(&input);
    let extra = {
        let mut r = exported(&input);
        r.head.id = sid("target-extra");
        r
    };
    export.payload.records.push(extra);
    export.payload.retained[0].reason = text("a restated reason");
    export.payload.digest = compute_export_digest(&export.payload);
    let found = problems(export, PlanBoundary::Pinned(&plan), &input);
    assert!(has(
        &found,
        "exported record \"target-extra\" does not correspond"
    ));
    assert!(has(
        &found,
        "reason does not match the plan's own retained_reason"
    ));

    let mut export = export_of(&input);
    export.payload.records.clear();
    export.payload.retained.clear();
    export.payload.digest = compute_export_digest(&export.payload);
    let found = problems(export, PlanBoundary::Pinned(&plan), &input);
    assert!(has(
        &found,
        "the plan's target \"target-a\" has no corresponding exported record"
    ));
    assert!(has(
        &found,
        "retained-transitional unit \"unit-b\" has no corresponding retained entry"
    ));
}

#[test]
fn migration_export_content_preservation_is_checked_byte_for_byte() {
    let input = plan();
    let plan = pinned(&input);
    let mut export = export_of(&input);
    let mut record = exported(&input);
    record.payload = ExportedPayload::new(
        OpenObject::new(vec![("content".to_string(), string("changed"))]),
        crate::migration::content::EnvelopeFields {
            content: present("changed"),
            ..Default::default()
        },
    );
    export.payload.records = vec![record];
    export.payload.digest = compute_export_digest(&export.payload);
    let found = problems(export, PlanBoundary::Pinned(&plan), &input);
    assert!(has(&found, "diverges from the plan's own target"));
    assert!(has(&found, "payload media_type is missing or empty"));
    assert!(has(&found, "does not match the resolved actual content"));
}

#[test]
fn migration_export_stale_digest_key_and_scope_are_rejected() {
    let input = plan();
    let plan = pinned(&input);
    let mut export = export_of(&input);
    export.payload.idempotency_key = ContentDigest::of_str("arbitrary");
    export.payload.digest = ContentDigest::of_str("stale");
    export.head.scope = Scope::project_workspace(sid("another-project"), None);
    let found = problems(export, PlanBoundary::Pinned(&plan), &input);
    assert!(has(
        &found,
        "idempotency_key is never an arbitrary free-form string"
    ));
    assert!(has(
        &found,
        "the same pinned content must always compute the same digest"
    ));
    assert!(has(
        &found,
        "scope does not match the resolved plan's own scope"
    ));
}

#[test]
fn migration_export_the_digest_is_neutral_to_declaration_order() {
    let input = plan();
    let mut export = export_of(&input);
    let mut second = exported(&input);
    second.head.id = sid("target-0");
    export.payload.records.push(second);
    let forward = compute_export_digest(&export.payload);
    export.payload.records.reverse();
    assert_eq!(forward, compute_export_digest(&export.payload));
}
