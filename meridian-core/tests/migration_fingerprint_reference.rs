//! Reference tests for `compute_plan_fingerprint`/`compute_idempotency_key`
//! against FIXED expected hashes obtained by calling the real Node.js
//! reference implementation (`computePlanFingerprint`/
//! `computeIdempotencyKey`, `scripts/lib/instance-data-migration.mjs`) —
//! not merely Rust's own internal determinism, which a single
//! self-consistency test cannot distinguish from "a deterministic but
//! wrong" canonicalization.
//!
//! Each fixture below is the field-for-field Rust construction of the
//! EXACT same logical `payload`/`scope`/`source` a companion Node.js
//! script built as plain object literals and passed to the real,
//! unmodified `computePlanFingerprint`/`computeIdempotencyKey` imported
//! from `scripts/lib/instance-data-migration.mjs` (not reimplemented, not
//! stubbed):
//!
//! ```js
//! import { computePlanFingerprint, computeIdempotencyKey } from
//!   '<MERIDIAN_KERNEL>/scripts/lib/instance-data-migration.mjs';
//! const payload = { source: {...}, record_units: [...], mappings: [...], rollback: {...} };
//! console.log(computePlanFingerprint(payload));
//! console.log(computeIdempotencyKey({ type: '...', id: '...' /* + workspace_id */ }, payload.source));
//! ```
//!
//! run once with `node` against the exact object literals mirrored by
//! `fixture_one`/`fixture_two_mappings` below, to produce the hex constants
//! this file asserts against. The reviewer can reproduce this independently
//! by writing out the same two object literals and running that script.
//!
//! Any future change to the canonical projection in
//! `meridian-core/src/migration/canonical.rs` that is not ALSO made in
//! `scripts/lib/instance-data-migration.mjs` (or vice versa) will change
//! one side's hash and not the other's, and these tests will fail —
//! exactly the guarantee a byte-compatible canonicalization is for.
//!
//! Kernel-purity fix-up: `fixture_one`/`fixture_two_mappings`' source
//! `repository_ref` originally carried a product-specific Instance literal,
//! not permitted in this Kernel repository. Replaced with the neutral
//! `"instance:sample"`/`"instance:sample-repo"`; since `repository_ref` is
//! itself part of both hashed projections, all four hex constants below
//! were re-derived by running the exact same companion Node.js script
//! above against object literals with ONLY that field changed (verified by
//! first reproducing the PRIOR hashes against the PRIOR literal, to
//! confirm the mirrored object literal was faithful, before trusting the
//! new ones) — not computed or guessed by hand.

use meridian_core::migration::checks::{compute_idempotency_key, compute_plan_fingerprint};
use meridian_core::migration::*;
use meridian_core::resolver::IsoDate;
use meridian_core::types::{
    Authority, AuthorityKind, ContentDigest, EvidenceRef, NonEmptyString, Revision, Scope,
    SemanticId, Verdict, WorkspaceId,
};

fn nes(s: &str) -> NonEmptyString {
    NonEmptyString::new(s).unwrap()
}
fn sid(s: &str) -> SemanticId {
    SemanticId::new(s).unwrap()
}

/// The exact SHA-256 hex digest of the empty string, reused only as a
/// syntactically well-formed 64-hex-character placeholder digest value —
/// its own preimage is irrelevant to this test, only its exact 64
/// characters, which appear verbatim in the Node fixture too.
const PLACEHOLDER_DIGEST_HEX: &str =
    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

fn placeholder_digest() -> ContentDigest {
    ContentDigest::from_hex(PLACEHOLDER_DIGEST_HEX).unwrap()
}

/// Fixture 1: a single `migrated` mapping, `project-workspace` scope (no
/// `workspace_id`), `verified-restoration` rollback.
fn fixture_one() -> MigrationPlan {
    let source = SourceState::new(
        Revision::new("abc1234").unwrap(),
        placeholder_digest(),
        EvidenceRef::new("instance:sample").unwrap(),
        true,
        Qualification::Reproducible,
    )
    .unwrap();

    let record_units = vec![RecordUnit::new(
        sid("legacy-1"),
        nes("legacy/path.md"),
        nes("kept because stable"),
    )
    .unwrap()];

    let target = TargetRecord::new(
        nes("scoped-record.schema.json"),
        sid("branch-naming"),
        nes("Branch naming rule"),
        sid("norm"),
        Scope::project_workspace(sid("sample-project"), None),
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
            AuthorityKind::ProjectOwner,
            "workspace-owner",
            Some("owner-decision:branch-naming".to_string()),
        )
        .unwrap(),
        ContentEnvelope::new(
            nes("text/plain"),
            Encoding::Utf8,
            nes("norm text"),
            ContentDigest::of_str("norm text"),
        )
        .unwrap(),
    )
    .unwrap();
    let owner_decision = OwnerDecision::new(
        nes("owner-decision:branch-naming"),
        IsoDate::new("2026-09-08").unwrap(),
        nes("accepted as-is"),
    );
    let mappings = vec![Mapping::new(
        sid("legacy-1"),
        Disposition::Migrated {
            target,
            owner_decision,
        },
    )
    .unwrap()];

    let rollback = Rollback::new(
        EvidenceRef::new("snapshot:legacy-1").unwrap(),
        RollbackPlan::VerifiedRestoration {
            restoration_evidence_ref: EvidenceRef::new("evidence:restore-1").unwrap(),
        },
    );

    MigrationPlan::new(
        sid("plan-1"),
        Scope::project_workspace(sid("sample-project"), None),
        source,
        record_units,
        mappings,
        rollback,
        Verification::new(
            SubVerdict::Unverified,
            SubVerdict::Unverified,
            Verdict::Unverified,
        ),
        placeholder_digest(),
        placeholder_digest(),
        None,
    )
    .unwrap()
}

/// Fixture 2: a `merged` group of two units, `repository-scope` (with
/// `workspace_id`), `deterministic-reconstruction` rollback — exercises
/// the `mappings`/`record_units` stable-sort key and the
/// `workspace_id`-present branch of `idempotency_key`.
fn fixture_two_mappings(reverse_declaration_order: bool) -> MigrationPlan {
    let source = SourceState::new(
        Revision::new("rev-42").unwrap(),
        placeholder_digest(),
        EvidenceRef::new("instance:sample-repo").unwrap(),
        true,
        Qualification::Reproducible,
    )
    .unwrap();

    let mut record_units = vec![
        RecordUnit::new(sid("legacy-b"), nes("legacy/b.md"), nes("basis b")).unwrap(),
        RecordUnit::new(sid("legacy-a"), nes("legacy/a.md"), nes("basis a")).unwrap(),
    ];

    let scope = Scope::repository_scope(sid("norm-1"), WorkspaceId::new("sample-project").unwrap());
    let build_target = || {
        TargetRecord::new(
            nes("scoped-record.schema.json"),
            sid("merged-target"),
            nes("Merged norm"),
            sid("norm"),
            scope.clone(),
            FieldBasis::new(
                FieldBasisValue::Assigned,
                FieldBasisValue::Assigned,
                FieldBasisValue::Assigned,
                FieldBasisValue::Assigned,
                FieldBasisValue::Assigned,
                FieldBasisValue::Assigned,
                FieldBasisValue::Assigned,
                FieldBasisValue::Assigned,
            ),
            TargetOrigin::new(nes("record-units:legacy-a,legacy-b")),
            Authority::new(
                AuthorityKind::RepositoryMaintainer,
                "repo-owner",
                Some("owner-decision:merge-1".to_string()),
            )
            .unwrap(),
            ContentEnvelope::new(
                nes("application/json"),
                Encoding::Utf8,
                nes(r#"{"a":1}"#),
                ContentDigest::of_str(r#"{"a":1}"#),
            )
            .unwrap(),
        )
        .unwrap()
    };
    let build_owner_decision = || {
        OwnerDecision::new(
            nes("owner-decision:merge-1"),
            IsoDate::new("2026-09-10").unwrap(),
            nes("consolidated"),
        )
    };

    let mut mappings = vec![
        Mapping::new(
            sid("legacy-b"),
            Disposition::Merged {
                target: build_target(),
                owner_decision: build_owner_decision(),
                merge_rule_ref: EvidenceRef::new("merge-rule:combine").unwrap(),
            },
        )
        .unwrap(),
        Mapping::new(
            sid("legacy-a"),
            Disposition::Merged {
                target: build_target(),
                owner_decision: build_owner_decision(),
                merge_rule_ref: EvidenceRef::new("merge-rule:combine").unwrap(),
            },
        )
        .unwrap(),
    ];

    if reverse_declaration_order {
        record_units.reverse();
        mappings.reverse();
    }

    let rollback = Rollback::new(
        EvidenceRef::new("snapshot:legacy-2").unwrap(),
        RollbackPlan::DeterministicReconstruction {
            deterministic_plan_ref: EvidenceRef::new("plan:reconstruct-2").unwrap(),
        },
    );

    MigrationPlan::new(
        sid("plan-2"),
        scope,
        source,
        record_units,
        mappings,
        rollback,
        Verification::new(
            SubVerdict::Unverified,
            SubVerdict::Unverified,
            Verdict::Unverified,
        ),
        placeholder_digest(),
        placeholder_digest(),
        None,
    )
    .unwrap()
}

#[test]
fn plan_fingerprint_matches_the_node_reference_for_a_migrated_mapping() {
    let plan = fixture_one();
    assert_eq!(
        compute_plan_fingerprint(&plan).value(),
        "ca8af11ab2ac38db490cb7678cb05bf06b43716a22ccee2e5464784728a570f2"
    );
}

#[test]
fn idempotency_key_matches_the_node_reference_for_a_project_workspace_scope() {
    let plan = fixture_one();
    assert_eq!(
        compute_idempotency_key(&plan).value(),
        "727a288c00e83a6f29d5a01bb551feb9192d0e1d8b9fe66c24c00aba01472e57"
    );
}

#[test]
fn plan_fingerprint_matches_the_node_reference_for_a_merged_mapping() {
    let plan = fixture_two_mappings(false);
    assert_eq!(
        compute_plan_fingerprint(&plan).value(),
        "19ddebb654a510ec74b46d96a79b1fc3c93693ea017efee8926994be974fcd6e"
    );
}

#[test]
fn idempotency_key_matches_the_node_reference_for_a_repository_scope_with_workspace_id() {
    let plan = fixture_two_mappings(false);
    assert_eq!(
        compute_idempotency_key(&plan).value(),
        "f66c28d2dc56dad8d1372c4ee9ef950ec22f704c511f4049423152d580595b97"
    );
}

#[test]
fn plan_fingerprint_matches_the_node_reference_regardless_of_declaration_order() {
    // The Node reference itself proves record_units/mappings declaration
    // order does not change the fingerprint (both orders were hashed
    // against the real Node function and produced the identical value);
    // this proves the Rust port preserves that same property against the
    // same fixed reference hash, not merely against itself.
    let plan = fixture_two_mappings(true);
    assert_eq!(
        compute_plan_fingerprint(&plan).value(),
        "19ddebb654a510ec74b46d96a79b1fc3c93693ea017efee8926994be974fcd6e"
    );
}
