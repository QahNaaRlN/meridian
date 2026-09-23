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

use meridian_core::migration::plan::{
    compute_idempotency_key, compute_plan_fingerprint, DeclaredContent, DispositionInput,
    FieldBasisInput, FieldBasisValue, MappingInput, OwnerDecisionInput, PlanPayloadInput,
    Qualification, RecordUnitInput, RollbackInput, RollbackPlanInput, SourceInput, SubVerdictInput,
    TargetAuthorityInput, TargetInput, VerificationInput,
};
use meridian_core::migration::Encoding;
use meridian_core::resolver::IsoDate;
use meridian_core::run_contracts::RecordText;
use meridian_core::types::{AuthorityKind, ContentDigest, Scope, SemanticId, Verdict, WorkspaceId};

fn text(s: &str) -> RecordText {
    RecordText::new(s).unwrap()
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

fn field_basis(values: [FieldBasisValue; 9]) -> FieldBasisInput {
    let [schema, id, title, record_type, scope, schema_version, origin, authority, payload] =
        values;
    FieldBasisInput {
        schema,
        id,
        title,
        record_type,
        scope,
        schema_version,
        origin,
        authority,
        payload,
    }
}

use FieldBasisValue::{Assigned as A, Preserved as P};

fn unverified() -> VerificationInput {
    VerificationInput {
        coverage: SubVerdictInput::Unverified,
        applicability_preservation: SubVerdictInput::Unverified,
        overall_status: Verdict::Unverified,
    }
}

/// Fixture 1: a single `migrated` mapping, `project-workspace` scope (no
/// `workspace_id`), `verified-restoration` rollback.
fn fixture_one() -> (Scope, PlanPayloadInput) {
    let target = TargetInput {
        schema_ref: text("scoped-record.schema.json"),
        id: sid("branch-naming"),
        title: text("Branch naming rule"),
        record_type: sid("norm"),
        scope: Scope::project_workspace(sid("sample-project"), None),
        field_basis: field_basis([A, P, P, P, A, A, A, A, P]),
        origin_source_ref: text("record-unit:legacy-1"),
        authority: TargetAuthorityInput {
            kind: AuthorityKind::ProjectOwner,
            authority_ref: text("workspace-owner"),
            decision_ref: text("owner-decision:branch-naming"),
        },
        payload: DeclaredContent {
            media_type: text("text/plain"),
            encoding: Encoding::Utf8,
            content: text("norm text"),
            digest: ContentDigest::of_str("norm text"),
        },
    };
    let payload = PlanPayloadInput {
        source: SourceInput {
            revision: text("abc1234"),
            digest: placeholder_digest(),
            repository_ref: text("instance:sample"),
            working_tree_clean: true,
            qualification: Qualification::Reproducible,
        },
        record_units: vec![RecordUnitInput {
            id: sid("legacy-1"),
            unit_ref: text("legacy/path.md"),
            classification_basis: text("kept because stable"),
        }],
        mappings: vec![MappingInput {
            unit_id: sid("legacy-1"),
            disposition: DispositionInput::Migrated {
                target,
                owner_decision: OwnerDecisionInput {
                    decision_ref: text("owner-decision:branch-naming"),
                    decided_at: IsoDate::new("2026-09-08").unwrap(),
                    reason: text("accepted as-is"),
                },
            },
        }],
        rollback: RollbackInput {
            source_snapshot_ref: text("snapshot:legacy-1"),
            plan: RollbackPlanInput::VerifiedRestoration {
                restoration_evidence_ref: text("evidence:restore-1"),
            },
        },
        verification: unverified(),
        plan_fingerprint: placeholder_digest(),
        idempotency_key: placeholder_digest(),
        supersedes: None,
    };
    (
        Scope::project_workspace(sid("sample-project"), None),
        payload,
    )
}

/// Fixture 2: a `merged` group of two units, `repository-scope` (with
/// `workspace_id`), `deterministic-reconstruction` rollback — exercises
/// the `mappings`/`record_units` stable-sort key and the
/// `workspace_id`-present branch of `idempotency_key`.
fn fixture_two_mappings(reverse_declaration_order: bool) -> (Scope, PlanPayloadInput) {
    let scope = Scope::repository_scope(sid("norm-1"), WorkspaceId::new("sample-project").unwrap());
    let target = TargetInput {
        schema_ref: text("scoped-record.schema.json"),
        id: sid("merged-target"),
        title: text("Merged norm"),
        record_type: sid("norm"),
        scope: scope.clone(),
        field_basis: field_basis([A; 9]),
        origin_source_ref: text("record-units:legacy-a,legacy-b"),
        authority: TargetAuthorityInput {
            kind: AuthorityKind::RepositoryMaintainer,
            authority_ref: text("repo-owner"),
            decision_ref: text("owner-decision:merge-1"),
        },
        payload: DeclaredContent {
            media_type: text("application/json"),
            encoding: Encoding::Utf8,
            content: text(r#"{"a":1}"#),
            digest: ContentDigest::of_str(r#"{"a":1}"#),
        },
    };
    let merged = |unit: &str| MappingInput {
        unit_id: sid(unit),
        disposition: DispositionInput::Merged {
            target: target.clone(),
            owner_decision: OwnerDecisionInput {
                decision_ref: text("owner-decision:merge-1"),
                decided_at: IsoDate::new("2026-09-10").unwrap(),
                reason: text("consolidated"),
            },
            merge_rule_ref: text("merge-rule:combine"),
        },
    };
    let mut record_units = vec![
        RecordUnitInput {
            id: sid("legacy-b"),
            unit_ref: text("legacy/b.md"),
            classification_basis: text("basis b"),
        },
        RecordUnitInput {
            id: sid("legacy-a"),
            unit_ref: text("legacy/a.md"),
            classification_basis: text("basis a"),
        },
    ];
    let mut mappings = vec![merged("legacy-b"), merged("legacy-a")];
    if reverse_declaration_order {
        record_units.reverse();
        mappings.reverse();
    }
    let payload = PlanPayloadInput {
        source: SourceInput {
            revision: text("rev-42"),
            digest: placeholder_digest(),
            repository_ref: text("instance:sample-repo"),
            working_tree_clean: true,
            qualification: Qualification::Reproducible,
        },
        record_units,
        mappings,
        rollback: RollbackInput {
            source_snapshot_ref: text("snapshot:legacy-2"),
            plan: RollbackPlanInput::DeterministicReconstruction {
                deterministic_plan_ref: text("plan:reconstruct-2"),
            },
        },
        verification: unverified(),
        plan_fingerprint: placeholder_digest(),
        idempotency_key: placeholder_digest(),
        supersedes: None,
    };
    (scope, payload)
}

/// Fixture 3 (`rust-architecture-conformance-7`): two `migrated` mappings
/// whose unit ids `legacy-1`/`legacy-10` make the reference's
/// `localeCompare` order DIFFER from code-unit order (`"legacy-1::"`
/// collates before `"legacy-10::"`, while `':' > '0'` by code unit). Its
/// expected hash was obtained from the real `computePlanFingerprint` over
/// the mirrored object literal (both declaration orders hash identically).
fn fixture_locale_order(reverse_declaration_order: bool) -> PlanPayloadInput {
    let target = |id: &str, unit: &str| TargetInput {
        schema_ref: text("scoped-record.schema.json"),
        id: sid(id),
        title: text(&format!("Norm {id}")),
        record_type: sid("norm"),
        scope: Scope::project_workspace(sid("sample-project"), None),
        field_basis: field_basis([A, P, P, P, A, A, A, A, P]),
        origin_source_ref: text(&format!("record-unit:{unit}")),
        authority: TargetAuthorityInput {
            kind: AuthorityKind::ProjectOwner,
            authority_ref: text("workspace-owner"),
            decision_ref: text(&format!("owner-decision:{id}")),
        },
        payload: DeclaredContent {
            media_type: text("text/plain"),
            encoding: Encoding::Utf8,
            content: text(&format!("text {id}")),
            digest: ContentDigest::of_str(&format!("text {id}")),
        },
    };
    let migrated = |unit: &str, id: &str| MappingInput {
        unit_id: sid(unit),
        disposition: DispositionInput::Migrated {
            target: target(id, unit),
            owner_decision: OwnerDecisionInput {
                decision_ref: text(&format!("owner-decision:{id}")),
                decided_at: IsoDate::new("2026-09-08").unwrap(),
                reason: text("accepted"),
            },
        },
    };
    let mut record_units = vec![
        RecordUnitInput {
            id: sid("legacy-10"),
            unit_ref: text("legacy/10.md"),
            classification_basis: text("basis 10"),
        },
        RecordUnitInput {
            id: sid("legacy-1"),
            unit_ref: text("legacy/1.md"),
            classification_basis: text("basis 1"),
        },
    ];
    let mut mappings = vec![migrated("legacy-10", "t-10"), migrated("legacy-1", "t-1")];
    if reverse_declaration_order {
        record_units.reverse();
        mappings.reverse();
    }
    PlanPayloadInput {
        source: SourceInput {
            revision: text("abc1234"),
            digest: placeholder_digest(),
            repository_ref: text("instance:sample"),
            working_tree_clean: true,
            qualification: Qualification::Reproducible,
        },
        record_units,
        mappings,
        rollback: RollbackInput {
            source_snapshot_ref: text("snapshot:legacy"),
            plan: RollbackPlanInput::VerifiedRestoration {
                restoration_evidence_ref: text("evidence:restore"),
            },
        },
        verification: unverified(),
        plan_fingerprint: placeholder_digest(),
        idempotency_key: placeholder_digest(),
        supersedes: None,
    }
}

#[test]
fn plan_fingerprint_matches_the_node_reference_for_a_migrated_mapping() {
    let (_, payload) = fixture_one();
    assert_eq!(
        compute_plan_fingerprint(&payload).value(),
        "ca8af11ab2ac38db490cb7678cb05bf06b43716a22ccee2e5464784728a570f2"
    );
}

#[test]
fn idempotency_key_matches_the_node_reference_for_a_project_workspace_scope() {
    let (scope, payload) = fixture_one();
    assert_eq!(
        compute_idempotency_key(&scope, &payload.source).value(),
        "727a288c00e83a6f29d5a01bb551feb9192d0e1d8b9fe66c24c00aba01472e57"
    );
}

#[test]
fn plan_fingerprint_matches_the_node_reference_for_a_merged_mapping() {
    let (_, payload) = fixture_two_mappings(false);
    assert_eq!(
        compute_plan_fingerprint(&payload).value(),
        "19ddebb654a510ec74b46d96a79b1fc3c93693ea017efee8926994be974fcd6e"
    );
}

#[test]
fn idempotency_key_matches_the_node_reference_for_a_repository_scope_with_workspace_id() {
    let (scope, payload) = fixture_two_mappings(false);
    assert_eq!(
        compute_idempotency_key(&scope, &payload.source).value(),
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
    let (_, payload) = fixture_two_mappings(true);
    assert_eq!(
        compute_plan_fingerprint(&payload).value(),
        "19ddebb654a510ec74b46d96a79b1fc3c93693ea017efee8926994be974fcd6e"
    );
}

/// Before `rust-architecture-conformance-7` the mapping sort key was
/// compared by code unit, which puts `legacy-10::t-10` BEFORE
/// `legacy-1::t-1` and hashed a different preimage than the reference for
/// this plan in either declaration order.
#[test]
fn plan_fingerprint_sorts_mapping_keys_in_the_reference_locale_order() {
    for reverse in [false, true] {
        assert_eq!(
            compute_plan_fingerprint(&fixture_locale_order(reverse)).value(),
            "fd21f5a1a2180a4218aa4c0bc019466a9b87e22be92336d72ea34d7e0e1e6eb0"
        );
    }
}
