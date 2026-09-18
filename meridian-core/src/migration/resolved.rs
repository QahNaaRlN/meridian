//! The already-resolved external data a migration-plan check compares
//! against — `meridian-core` never resolves an `EvidenceRef` or a source
//! snapshot itself (it does not know `EvidenceRepository`, SQLite, Git or
//! the filesystem exist); the caller resolves it through whatever port it
//! has and passes the closed result in.
//!
//! These mirror the "closed transformer response" shapes
//! `resolveAndCheckSourceSnapshot`/`resolveAndCheckEvidence`/
//! `resolveAndCheckRollbackSnapshot`/`resolveAndCheckDeterministicPlan`/
//! `resolveAndCheckRestorationEvidence`/`checkSupersedes` check against in
//! `scripts/lib/instance-data-migration.mjs`, minus the fields that check
//! only that the (JSON) response itself was closed to a known field set —
//! a `meridian-core` caller can only ever construct one of these typed
//! structs, so there is no "unknown field" to reject.

use crate::types::{ContentDigest, EvidenceRef, Revision, Scope, SemanticId};

/// The result of resolving `payload.source` through `resolveSourceSnapshot`
/// (`instance-data-migration.md` §2). `record_type` matching is not
/// modelled — a caller can only construct a `ResolvedSourceSnapshot`, never
/// a same-shaped response of the wrong kind.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResolvedSourceSnapshot {
    pub repository_ref: EvidenceRef,
    pub revision: Revision,
    pub digest: ContentDigest,
    pub working_tree_clean: bool,
}

/// Which sub-verdict a resolved evidence record confirms
/// (`instance-data-migration.md` §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvidenceKind {
    Coverage,
    ApplicabilityPreservation,
}

/// The result of resolving a `verification.*.evidence_ref` through
/// `resolveEvidence` (`instance-data-migration.md` §6).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResolvedEvidence {
    pub evidence_ref: EvidenceRef,
    pub kind: EvidenceKind,
    pub plan_ref: SemanticId,
    pub plan_fingerprint: ContentDigest,
    pub confirms: bool,
}

/// The result of resolving `rollback.source_snapshot_ref` through
/// `resolveRollbackSnapshot` (`instance-data-migration.md` §5).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResolvedRollbackSnapshot {
    pub source_snapshot_ref: EvidenceRef,
    pub repository_ref: EvidenceRef,
    pub revision: Revision,
    pub digest: ContentDigest,
}

/// The result of resolving `rollback.deterministic_plan_ref` through
/// `resolveDeterministicPlan` (`instance-data-migration.md` §5).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResolvedDeterministicPlan {
    pub deterministic_plan_ref: EvidenceRef,
    pub source_snapshot_ref: EvidenceRef,
    pub plan_ref: SemanticId,
    pub plan_fingerprint: ContentDigest,
    pub applicable: bool,
}

/// The result of resolving `rollback.restoration_evidence_ref` through
/// `resolveRestorationEvidence` (`instance-data-migration.md` §5).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResolvedRestorationEvidence {
    pub evidence_ref: EvidenceRef,
    pub plan_ref: SemanticId,
    pub plan_fingerprint: ContentDigest,
    pub source_snapshot_ref: EvidenceRef,
    pub confirms: bool,
}

/// The result of resolving `payload.supersedes` through
/// `resolveSupersededPlan`, used only when the named predecessor is not
/// present in the same batch of plans being checked
/// (`instance-data-migration.md` §7).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResolvedSupersededPlan {
    pub plan_ref: SemanticId,
    pub scope: Scope,
    pub repository_ref: EvidenceRef,
    pub revision: Revision,
}
