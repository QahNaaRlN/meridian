//! Pure checks for `existing-project-compatibility-mode`. Moved here
//! unchanged (except [`check_discovery_outcomes`]'s corrective-round fix —
//! see below) from `meridian-app`'s own `domain.rs` by the corrective
//! round — see the parent module's doc comment.

use std::collections::{BTreeMap, BTreeSet};

use crate::types::{AuthorityKind, Diagnostic, Scope, ScopeType, SemanticId};

use super::fail;
use super::types::{
    ConnectionMode, Finding, FindingKind, ManagedModeDecision, MissingSource, PreviousKnowledge,
    RepositoryRef, UnreadableSource,
};

/// The ONE closed priority `next_step`: a blocking "conflict" finding
/// outranks a blocking "ambiguous-scope" finding, which outranks any other
/// blocking finding, which outranks having no blocking finding at all.
/// Deterministic regardless of the input slice's order.
pub fn compute_next_step(findings: &[Finding]) -> super::NextStep {
    use super::NextStep;
    let blocking: Vec<&Finding> = findings.iter().filter(|f| f.blocking()).collect();
    if blocking.iter().any(|f| f.kind() == FindingKind::Conflict) {
        return NextStep::ResolveConflict;
    }
    if blocking
        .iter()
        .any(|f| f.kind() == FindingKind::AmbiguousScope)
    {
        return NextStep::ResolveAmbiguity;
    }
    if !blocking.is_empty() {
        return NextStep::AwaitOwnerDecision;
    }
    NextStep::ContinueCompatibilityMode
}

/// Property 6: the scan's `payload.repository` must name the EXACT
/// workspace and repository its own `scope` declares.
pub fn check_repository_scope(
    at: &str,
    scope: &Scope,
    repository: &RepositoryRef,
) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    match scope.scope_type() {
        ScopeType::ProjectWorkspace => {
            if repository.workspace_id().as_str() != scope.id() {
                problems.push(fail(format!(
                    "{at} payload.repository.workspace_id \"{}\" does not match scope.id \"{}\"; scope names the exact workspace this scan covers",
                    repository.workspace_id(),
                    scope.id()
                )));
            }
        }
        ScopeType::RepositoryScope => {
            let scope_workspace_id = scope.workspace_id().map(|w| w.as_str()).unwrap_or("");
            if repository.workspace_id().as_str() != scope_workspace_id {
                problems.push(fail(format!(
                    "{at} payload.repository.workspace_id \"{}\" does not match scope.workspace_id \"{scope_workspace_id}\"",
                    repository.workspace_id()
                )));
            }
            if repository.id().as_str() != scope.id() {
                problems.push(fail(format!(
                    "{at} payload.repository.id \"{}\" does not match scope.id \"{}\"; scope names the exact repository this scan covers",
                    repository.id(),
                    scope.id()
                )));
            }
        }
        _ => {}
    }
    problems
}

fn managed_mode_authority_by_scope(scope_type: ScopeType) -> Option<AuthorityKind> {
    match scope_type {
        ScopeType::ProjectWorkspace => Some(AuthorityKind::ProjectOwner),
        ScopeType::RepositoryScope => Some(AuthorityKind::RepositoryMaintainer),
        _ => None,
    }
}

/// Property 1: managed mode never activates automatically.
pub fn check_managed_mode(
    at: &str,
    connection_mode: ConnectionMode,
    managed_mode_decision: Option<&ManagedModeDecision>,
    scope: &Scope,
) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    if connection_mode != ConnectionMode::Managed {
        return problems;
    }
    let Some(decision) = managed_mode_decision else {
        problems.push(fail(format!(
            "{at} connection_mode is \"managed\" with no managed_mode_decision; managed mode never activates automatically and requires a separate, verifiable owner decision"
        )));
        return problems;
    };
    if let Some(required) = managed_mode_authority_by_scope(scope.scope_type()) {
        if decision.authority().kind() != required {
            problems.push(fail(format!(
                "{at} managed_mode_decision.authority.kind is \"{}\", but scope \"{}\" requires owner authority \"{required}\"",
                decision.authority().kind(),
                scope.scope_type()
            )));
        }
    }
    problems
}

/// Property 3/7: `discovery_plan` outcomes form a COMPLETE, EXCLUSIVE
/// partition, counted by plan id, never by array position, and counted by
/// OCCURRENCE, never by distinct value.
///
/// **Corrective-round fix** (`rust-architecture-conformance-2`,
/// `CHANGES_REQUESTED` items 2-4): the previous signature took
/// `discovered_ids: &BTreeSet<String>` — the caller built that set by
/// collapsing every `discovered_sources[].id` into a `Set` BEFORE calling
/// this function, so two `discovered_sources` entries naming the SAME plan
/// id silently counted as one outcome instead of two, hiding a real
/// duplicate. This version takes typed ID PROJECTIONS instead:
///
/// - `plan_ids` is the DECLARED SET of plan-slot ids — deduplication here
///   is correct: a slot declared twice already gets its own "declared more
///   than once" diagnostic from the caller, and this function's own job is
///   only "does each DISTINCT declared id have exactly one outcome".
/// - `discovered`/`missing`/`unreadable` are OCCURRENCE LISTS — one entry
///   per array element that carried a syntactically valid id/`plan_id`,
///   EVEN when the REST of that element failed its own domain construction
///   (a malformed `discovered_sources` entry, or a `missing_sources` entry
///   whose `previously_known`/`id` combination was invalid, still occupies
///   exactly one discovery-plan outcome) — the caller must NOT deduplicate
///   these, and must NOT gate collecting an id on the rest of that entry
///   building successfully, or the multiplicity/partial-construction defect
///   this fix targets reappears one layer up.
pub fn check_discovery_outcomes(
    at: &str,
    plan_ids: &BTreeSet<SemanticId>,
    discovered: &[SemanticId],
    missing: &[SemanticId],
    unreadable: &[SemanticId],
) -> Vec<Diagnostic> {
    let mut counts: BTreeMap<&SemanticId, u32> = BTreeMap::new();
    let mut bump = |id: &SemanticId| {
        if let Some(canonical) = plan_ids.get(id) {
            *counts.entry(canonical).or_insert(0) += 1;
        }
    };
    for id in discovered {
        bump(id);
    }
    for id in missing {
        bump(id);
    }
    for id in unreadable {
        bump(id);
    }

    let mut problems = Vec::new();
    for id in plan_ids {
        let count = counts.get(id).copied().unwrap_or(0);
        if count == 0 {
            problems.push(fail(format!(
                "{at} discovery_plan slot \"{id}\" has no outcome in discovered_sources/missing_sources/unreadable_sources; every declared slot requires exactly one"
            )));
        } else if count > 1 {
            problems.push(fail(format!(
                "{at} discovery_plan slot \"{id}\" has {count} outcomes across discovered_sources/missing_sources/unreadable_sources; exactly one is required per slot, whether the extras overlap two different arrays or repeat within one"
            )));
        }
    }
    problems
}

/// Property 7: a change is never silently absorbed — a discovered source
/// found `changed`, a `previously_known` missing source, and an unreadable
/// source each need a matching finding.
pub fn check_changes_have_findings(
    at: &str,
    discovered_changed_ids: &BTreeSet<String>,
    missing: &[MissingSource],
    unreadable: &[UnreadableSource],
    findings: &[Finding],
) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    let finding_key = |kind: FindingKind, plan_id: Option<&str>| {
        format!("{}::{}", kind.as_str(), plan_id.unwrap_or(""))
    };
    let finding_keys: BTreeSet<String> = findings
        .iter()
        .map(|f| finding_key(f.kind(), f.plan_id().map(SemanticId::as_str)))
        .collect();

    for id in discovered_changed_ids {
        if !finding_keys.contains(&finding_key(FindingKind::SourceChanged, Some(id))) {
            problems.push(fail(format!(
                "{at} discovered source \"{id}\" changed with no matching \"source-changed\" finding for plan_id \"{id}\"; a change is never silently absorbed into a rewritten snapshot"
            )));
        }
    }
    for m in missing {
        if matches!(m.knowledge(), PreviousKnowledge::PreviouslyKnown { .. })
            && !finding_keys.contains(&finding_key(
                FindingKind::SourceMissing,
                Some(m.plan_id().as_str()),
            ))
        {
            problems.push(fail(format!(
                "{at} missing_sources plan_id \"{}\" is previously_known with no matching \"source-missing\" finding",
                m.plan_id()
            )));
        }
    }
    for u in unreadable {
        if !finding_keys.contains(&finding_key(
            FindingKind::SourceUnreadable,
            Some(u.plan_id().as_str()),
        )) {
            problems.push(fail(format!(
                "{at} unreadable_sources plan_id \"{}\" has no matching \"source-unreadable\" finding",
                u.plan_id()
            )));
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::super::types::DiscoveryPlanSlot;
    use super::*;
    use crate::instruction_source::{Location, OpaqueRef, RelativePath};
    use crate::types::WorkspaceId;

    fn sid(s: &str) -> SemanticId {
        SemanticId::new(s).unwrap()
    }
    fn wid(s: &str) -> WorkspaceId {
        WorkspaceId::new(s).unwrap()
    }
    fn location(name: &str) -> Location {
        Location::file(
            RelativePath::new(format!("{name}.md")).unwrap(),
            OpaqueRef::new("container-1").unwrap(),
        )
    }

    #[test]
    fn compute_next_step_follows_the_closed_priority_regardless_of_order() {
        let conflict = Finding::new(FindingKind::Conflict, None, true);
        let ambiguous = Finding::new(FindingKind::AmbiguousScope, None, true);
        let other = Finding::new(FindingKind::Other, None, true);
        assert_eq!(
            compute_next_step(&[other.clone(), ambiguous.clone(), conflict.clone()]),
            super::super::NextStep::ResolveConflict
        );
        assert_eq!(
            compute_next_step(&[other.clone(), ambiguous]),
            super::super::NextStep::ResolveAmbiguity
        );
        assert_eq!(
            compute_next_step(&[other]),
            super::super::NextStep::AwaitOwnerDecision
        );
        assert_eq!(
            compute_next_step(&[]),
            super::super::NextStep::ContinueCompatibilityMode
        );
    }

    #[test]
    fn check_discovery_outcomes_flags_a_slot_with_no_outcome_and_a_slot_with_two() {
        let plan_ids: BTreeSet<SemanticId> = [sid("slot-a"), sid("slot-b")].into_iter().collect();
        // slot-b occurs TWICE among discovered ids — the exact multiplicity
        // scenario the corrective-round fix restores.
        let discovered = vec![sid("slot-b"), sid("slot-b")];
        let problems = check_discovery_outcomes("at", &plan_ids, &discovered, &[], &[]);
        assert!(problems
            .iter()
            .any(|d| d.message().contains("slot-a") && d.message().contains("no outcome")));
        assert!(problems
            .iter()
            .any(|d| d.message().contains("slot-b") && d.message().contains("2 outcomes")));
    }

    /// The multiplicity fix also applies when the two occurrences are
    /// spread across DIFFERENT arrays (one discovered, one missing) — same
    /// slot, two outcomes, still flagged.
    #[test]
    fn check_discovery_outcomes_counts_occurrences_across_different_arrays() {
        let plan_ids: BTreeSet<SemanticId> = [sid("slot-a")].into_iter().collect();
        let discovered = vec![sid("slot-a")];
        let missing = vec![sid("slot-a")];
        let problems = check_discovery_outcomes("at", &plan_ids, &discovered, &missing, &[]);
        assert!(problems
            .iter()
            .any(|d| d.message().contains("slot-a") && d.message().contains("2 outcomes")));
    }

    #[test]
    fn check_discovery_outcomes_accepts_exactly_one_outcome_per_slot() {
        let plan_ids: BTreeSet<SemanticId> = [sid("slot-a"), sid("slot-b")].into_iter().collect();
        let discovered = vec![sid("slot-a")];
        let missing = vec![sid("slot-b")];
        let problems = check_discovery_outcomes("at", &plan_ids, &discovered, &missing, &[]);
        assert!(problems.is_empty(), "{problems:?}");
    }

    #[test]
    fn check_changes_have_findings_requires_a_matching_finding_for_a_change() {
        let mut discovered_changed = BTreeSet::new();
        discovered_changed.insert("slot-a".to_string());
        let problems = check_changes_have_findings("at", &discovered_changed, &[], &[], &[]);
        assert!(problems.iter().any(|d| d
            .message()
            .contains("no matching \"source-changed\" finding")));

        let findings = vec![Finding::new(
            FindingKind::SourceChanged,
            Some(sid("slot-a")),
            true,
        )];
        let problems = check_changes_have_findings("at", &discovered_changed, &[], &[], &findings);
        assert!(problems.is_empty(), "{problems:?}");
    }

    #[test]
    fn check_repository_scope_requires_exact_match_for_repository_scope() {
        let scope = Scope::repository_scope(sid("sample-repository"), wid("sample-workspace"));
        let repo_ok = RepositoryRef::new(sid("sample-repository"), wid("sample-workspace"));
        assert!(check_repository_scope("at", &scope, &repo_ok).is_empty());
        let repo_bad = RepositoryRef::new(sid("other-repository"), wid("sample-workspace"));
        assert!(!check_repository_scope("at", &scope, &repo_bad).is_empty());
    }

    #[test]
    fn check_managed_mode_requires_a_decision() {
        let scope = Scope::project_workspace(sid("sample-project"), None);
        let problems = check_managed_mode("at", ConnectionMode::Managed, None, &scope);
        assert!(problems
            .iter()
            .any(|d| d.message().contains("never activates automatically")));
    }

    /// `DiscoveryPlanSlot`/`location` stay usable from this test module
    /// (structural sanity — this crate's own type, not `serde_json::Value`).
    #[test]
    fn discovery_plan_slot_is_constructible_with_a_typed_location() {
        let slot = DiscoveryPlanSlot::new(sid("slot-a"), location("a"));
        assert_eq!(slot.id().as_str(), "slot-a");
    }
}
