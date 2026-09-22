//! Pure, typed checks that need more than one already-valid
//! [`RuleCandidate`] or external data a single candidate's own construction
//! cannot see: semantic-cluster/conflict-graph coherence (properties 5/6/7)
//! and the resolved `source_ref` comparison (property 2). None of these
//! functions touch `serde_json::Value` or any transport type.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::instruction_source::{Currency, ResolvedInstructionSource};
use crate::migration::OwnerDecision;
use crate::types::{Authority, Diagnostic, Scope};

use super::fail;
use super::types::{Boundary, NotApplicableReason, PinnedSourceRef, RuleCandidate, SemanticKey};

/// The resolved entry is checked against the closed instruction-source-
/// registry snapshot contract: id/reference must match the pin, the pinned
/// revision must still be the CURRENT one and verified, the digest must
/// still match, and `currency` gates whether the snapshot may back a
/// decided (`accepted`/`not-applicable`) candidate at all.
/// `resolved: None` means the resolver found nothing (or its response
/// failed the app-side shape validation, which is reported separately,
/// before this function is even called) — a candidate never accepted from
/// an unknown source.
pub fn check_source_ref(
    candidate_id: &str,
    pinned: &PinnedSourceRef,
    resolved: Option<&ResolvedInstructionSource>,
    applicability_state: &str,
) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    let at = format!("rule candidate \"{candidate_id}\"");

    let Some(resolved) = resolved else {
        problems.push(fail(format!(
            "{at} source_ref does not resolve to a registered instruction source through the external resolver; an unknown source is never accepted and the candidate fails closed"
        )));
        return problems;
    };

    if resolved.id() != pinned.id() {
        problems.push(fail(format!(
            "{at} source_ref pins id \"{}\" but the reference resolves to record id \"{}\"",
            pinned.id(),
            resolved.id()
        )));
    }
    if resolved.reference() != pinned.reference() {
        problems.push(fail(format!(
            "{at} source_ref: the resolver's stated reference \"{}\" is not the resolved reference \"{}\"",
            resolved.reference(),
            pinned.reference()
        )));
    }
    if !resolved.recorded_state().revision_verified() {
        problems.push(fail(format!(
            "{at} source_ref pins a source whose recorded_state.revision_verified is not true; a candidate cannot be accepted from an unverified revision (property 2)"
        )));
    }
    if resolved.recorded_state().revision() != pinned.revision() {
        problems.push(fail(format!(
            "{at} source_ref pins revision \"{}\" but the instruction source registry now holds revision \"{}\"; the pinned edition has changed (property 2)",
            pinned.revision(),
            resolved.recorded_state().revision()
        )));
    }
    if resolved.recorded_state().digest() != pinned.sha256() {
        problems.push(fail(format!(
            "{at} source_ref pins sha256 \"{}\" but the instruction source registry now holds digest \"{}\"; the pinned edition has changed (property 2)",
            pinned.sha256().value(),
            resolved.recorded_state().digest().value()
        )));
    }
    if resolved.recorded_state().currency() == Currency::Unverified {
        problems.push(fail(format!(
            "{at} source_ref pins a source whose recorded_state.currency is \"unverified\"; an unverified snapshot never backs a rule candidate (property 2)"
        )));
    }
    if matches!(applicability_state, "accepted" | "not-applicable")
        && resolved.recorded_state().currency() == Currency::Stale
    {
        problems.push(fail(format!(
            "{at} applicability_state \"{applicability_state}\" relies on source_ref whose recorded_state.currency is \"stale\"; accepted and not-applicable both require a CURRENT verified snapshot — a stale snapshot is never silently treated as current and may back only a \"candidate\" entry kept as a historical finding (property 2/4)"
        )));
    }

    problems
}

/// Every member of a `semantic_key` cluster must agree on scope, on
/// applicability_state/owner_decision, and on the `conflicts_with` SET
/// (property 5); a cluster decided accepted/not-applicable must agree on
/// authority too, and no origin (source reference + boundary) repeats.
///
/// **Intentional Rust-native difference** from the Node reference
/// (documented in `COMPATIBILITY.md`, corrective round item 10): scope
/// agreement is checked with [`Scope`]'s own full-identity equality (the
/// same `same_scope` the migration contract's `checkSupersedes` already
/// uses), not a `type::id`-only string key. Two members that agree on
/// `type`/`id` but disagree on `workspace_id` or `organization_profile_id`
/// are now correctly flagged as inconsistent; Node's `scopeKey` silently
/// ignores those fields.
pub fn check_cluster(key: &str, members: &[&RuleCandidate]) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    let ids = || {
        members
            .iter()
            .map(|m| m.id().as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };

    let scopes: HashSet<&Scope> = members.iter().map(|m| m.scope()).collect();
    if scopes.len() > 1 {
        problems.push(fail(format!(
            "semantic cluster \"{key}\" classifies into more than one scope across its origins ({}); a duplicate cluster shares one classification",
            ids()
        )));
    }
    let states: HashSet<&str> = members
        .iter()
        .map(|m| m.payload().applicability().state_label())
        .collect();
    if states.len() > 1 {
        problems.push(fail(format!(
            "semantic cluster \"{key}\" carries more than one applicability_state across its origins ({}); a duplicate cluster is decided once, not per origin",
            ids()
        )));
    }
    let decisions: HashSet<Option<&OwnerDecision>> = members
        .iter()
        .map(|m| m.payload().applicability().owner_decision())
        .collect();
    if decisions.len() > 1 {
        problems.push(fail(format!(
            "semantic cluster \"{key}\" carries inconsistent owner_decision values across its origins ({})",
            ids()
        )));
    }
    let conflict_sets: HashSet<BTreeSet<&SemanticKey>> = members
        .iter()
        .map(|m| m.payload().conflicts_with().iter().collect())
        .collect();
    if conflict_sets.len() > 1 {
        problems.push(fail(format!(
            "semantic cluster \"{key}\" carries inconsistent conflicts_with declarations across its origins ({})",
            ids()
        )));
    }

    let decided_members: Vec<&&RuleCandidate> = members
        .iter()
        .filter(|m| m.payload().applicability().is_decided())
        .collect();
    let authorities: HashSet<&Authority> = decided_members.iter().map(|m| m.authority()).collect();
    if authorities.len() > 1 {
        let decided_ids: Vec<&str> = decided_members.iter().map(|m| m.id().as_str()).collect();
        problems.push(fail(format!(
            "semantic cluster \"{key}\" carries inconsistent authority across its decided origins ({}); a cluster decided accepted or not-applicable is decided by ONE authority — kind, authority_ref and decision_ref must all agree, and a matching owner_decision does not substitute for that",
            decided_ids.join(", ")
        )));
    }

    let mut seen_origin: HashSet<(&str, &Boundary)> = HashSet::new();
    for m in members {
        let reference = m.payload().source_ref().reference();
        let boundary = m.payload().boundary();
        if !seen_origin.insert((reference, boundary)) {
            problems.push(fail(format!(
                "semantic cluster \"{key}\" lists the same origin (source reference and boundary) more than once; each origin is preserved once, not repeated"
            )));
        }
    }

    problems
}

/// The conflict graph over the WHOLE candidate set, computed once — never
/// by folding records in the order they were read (property 6). The
/// returned `Vec<Diagnostic>`'s order is itself deterministic end to end,
/// not merely each pair's canonical `(min, max)` name order within its own
/// message: the outer walk is over `clusters: &BTreeMap<..>` (its own
/// key order), and every per-key neighbour walk uses a `BTreeSet`, never a
/// `HashSet`, so the full sequence of diagnostics is stable across runs and
/// independent of the process's random hash seed — the same input always
/// produces the same `Vec<Diagnostic>` byte for byte.
pub fn check_conflict_graph(
    clusters: &BTreeMap<SemanticKey, Vec<&RuleCandidate>>,
) -> Vec<Diagnostic> {
    let mut problems = Vec::new();

    let state_of = |k: &SemanticKey| -> Option<&'static str> {
        clusters
            .get(k)
            .and_then(|m| m.first())
            .map(|m| m.payload().applicability().state_label())
    };
    // `BTreeSet`, not `HashSet`: this closure's output is iterated below to
    // produce `problems` in observable order, so membership alone is not
    // enough — the order itself must be deterministic (lexicographic on
    // `SemanticKey`), independent of the process's random hash seed.
    let conflicts_of = |k: &SemanticKey| -> BTreeSet<&SemanticKey> {
        clusters
            .get(k)
            .and_then(|m| m.first())
            .map(|m| m.payload().conflicts_with().iter().collect())
            .unwrap_or_default()
    };

    for k in clusters.keys() {
        for other in conflicts_of(k) {
            if !clusters.contains_key(other) {
                problems.push(fail(format!(
                    "semantic cluster \"{k}\" declares conflicts_with \"{other}\", which is not the semantic_key of any candidate in this document"
                )));
                continue;
            }
            if other == k {
                problems.push(fail(format!(
                    "semantic cluster \"{k}\" declares itself in its own conflicts_with"
                )));
                continue;
            }
            if !conflicts_of(other).contains(k) {
                problems.push(fail(format!(
                    "conflict between \"{k}\" and \"{other}\" is declared only one-sided; conflicts_with is declared symmetrically so evaluation does not depend on which record is read first"
                )));
            }
        }
    }

    let mut reported_pairs: HashSet<(&SemanticKey, &SemanticKey)> = HashSet::new();
    for k in clusters.keys() {
        for other in conflicts_of(k) {
            if !clusters.contains_key(other) || !conflicts_of(other).contains(k) {
                continue;
            }
            let pair = if k < other { (k, other) } else { (other, k) };
            if !reported_pairs.insert(pair) {
                continue;
            }
            if state_of(pair.0) == Some("accepted") && state_of(pair.1) == Some("accepted") {
                problems.push(fail(format!(
                    "semantic clusters \"{}\" and \"{}\" are declared as conflicting but both are accepted; an unresolved conflict must not be applied as a norm on either side (property 7)",
                    pair.0, pair.1
                )));
            }
        }
    }

    for (k, members) in clusters {
        let Some(member) = members.first() else {
            continue;
        };
        if member.payload().applicability().not_applicable_reason()
            == Some(NotApplicableReason::LostConflictingDecision)
        {
            let has_accepted_opponent = conflicts_of(k)
                .iter()
                .any(|other| clusters.contains_key(*other) && state_of(other) == Some("accepted"));
            if !has_accepted_opponent {
                problems.push(fail(format!(
                    "semantic cluster \"{k}\" declares not_applicable_reason \"lost-conflicting-decision\" but none of its declared conflicts_with clusters is accepted; the reason is not evidenced"
                )));
            }
        }
    }

    problems
}

#[cfg(test)]
mod tests {
    use super::super::types::{Applicability, RuleCandidatePayload};
    use super::*;
    use crate::types::{
        AuthorityKind, ContentDigest, NonEmptyString, Origin, Revision, SemanticId, WorkspaceId,
    };

    fn candidate(id: &str, semantic_key: &str, conflicts_with: &[&str]) -> RuleCandidate {
        let payload = RuleCandidatePayload::new(
            PinnedSourceRef::new(
                SemanticId::new(format!("src-{id}")).unwrap(),
                NonEmptyString::new("sources/src-1").unwrap(),
                Revision::new("a".repeat(40)).unwrap(),
                ContentDigest::from_hex("b".repeat(64)).unwrap(),
            ),
            Boundary::whole_source(),
            NonEmptyString::new("raw").unwrap(),
            NonEmptyString::new("normalized").unwrap(),
            SemanticKey::new(semantic_key).unwrap(),
            NonEmptyString::new("basis").unwrap(),
            conflicts_with
                .iter()
                .map(|k| SemanticKey::new(*k).unwrap())
                .collect(),
            Applicability::Candidate,
        );
        RuleCandidate::try_new(
            SemanticId::new(id).unwrap(),
            Scope::repository_scope(
                SemanticId::new("sample-repository").unwrap(),
                WorkspaceId::new("sample-workspace").unwrap(),
            ),
            Origin::derived(format!("instruction-source:src-{id}")).unwrap(),
            Authority::new(AuthorityKind::DelegatedRun, "run:1", None).unwrap(),
            payload,
        )
        .unwrap()
    }

    /// Corrective round item 1: `check_conflict_graph`'s output order must
    /// not depend on iterating a `HashSet` of a cluster's neighbours.
    /// `"a-key"` declares conflicts_with TWO different neighbours in one
    /// call — `"b-key"`, which exists but does not declare the conflict
    /// back (a one-sided-declaration diagnostic), and `"d-key"`, which is
    /// not the `semantic_key` of any candidate at all (an undeclared-
    /// cluster diagnostic) — the exact shape a `HashSet`'s unordered walk
    /// could silently reorder between runs. This exercises the real
    /// `check_conflict_graph`, not a helper projection over it.
    #[test]
    fn check_conflict_graph_reports_two_neighbour_diagnostics_in_a_fixed_order() {
        let a = candidate("cand-a", "a-key", &["b-key", "d-key"]);
        let b = candidate("cand-b", "b-key", &[]);

        let mut clusters: BTreeMap<SemanticKey, Vec<&RuleCandidate>> = BTreeMap::new();
        clusters.insert(a.payload().semantic_key().clone(), vec![&a]);
        clusters.insert(b.payload().semantic_key().clone(), vec![&b]);

        let expected = vec![
            fail(
                "conflict between \"a-key\" and \"b-key\" is declared only one-sided; conflicts_with is declared symmetrically so evaluation does not depend on which record is read first",
            ),
            fail(
                "semantic cluster \"a-key\" declares conflicts_with \"d-key\", which is not the semantic_key of any candidate in this document",
            ),
        ];

        let first_run = check_conflict_graph(&clusters);
        assert_eq!(first_run, expected, "{first_run:?}");

        // Repeated calls on the identical input must be byte-for-byte
        // identical too, not merely internally self-consistent once.
        let second_run = check_conflict_graph(&clusters);
        assert_eq!(
            second_run, expected,
            "repeated calls on the same input must produce identical diagnostics"
        );
    }
}
