//! `workspace-compatibility-qualification`
//! (`registries/operating-model/workspace-compatibility-qualification.schema.json`,
//! `scripts/lib/workspace-compatibility-qualification.mjs`): the closed
//! verdict over one workspace's connection scans, an optional migration
//! plan and an optional canonical export.
//!
//! Composition: every connection is checked by the app-owned
//! `existing-project-compatibility-mode` route in ONE composition (its
//! diagnostics and per-connection accepted facts are handed in); the plan
//! and the export are checked here by the real [`crate::migration`]
//! checks. The export is checked against the [`PinnedPlan`] this
//! qualification itself resolved and pinned — [`PlanBoundary::Pinned`] —
//! so an independently resolved plan sharing its id can never reach it.

use crate::existing_project_compatibility_mode::NextStep;
use crate::migration::export::{
    check_canonical_exports, CanonicalExport, ExportInput, PinnedExport, PlanBoundary,
};
use crate::migration::plan::{check_migration_plans, MigrationPlan, PinnedPlan, PlanInput};
use crate::migration::record::RecordHead;
use crate::migration::resolved::{PlanResolution, ResponseCatalogue, SourceContentResponse};
use crate::run_contracts::{PinnedRecordKind, RecordText};
use crate::types::{AuthorityKind, Diagnostic, OriginKind, Scope, ScopeType, SemanticId, Verdict};

use super::pin::{check_resolution, QualificationPin, ResolvedRecord};
use super::{fail, prefixed, QualificationState};

const RECORD_TYPE: &str = "workspace-compatibility-qualification";
const REPOSITORY_SCOPE_MAX_CONNECTION_REFS: usize = 1;

/// `definitions.payload`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspacePayloadInput {
    pub workspace_connection_refs: Vec<QualificationPin>,
    /// Required exactly for a `project-workspace` scope (schema `allOf`).
    pub workspace_repository_ids: Option<Vec<SemanticId>>,
    pub migration_plan_ref: Option<QualificationPin>,
    pub canonical_export_ref: Option<QualificationPin>,
    pub qualification_state: QualificationState,
    pub qualification_reason: RecordText,
    pub blockers: Vec<RecordText>,
    pub open_questions: Vec<RecordText>,
}

/// One entry of `qualifications`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceQualificationInput {
    pub head: RecordHead,
    pub payload: WorkspacePayloadInput,
}

/// What a qualification reads from one ACCEPTED connection scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionFacts {
    pub scope: Scope,
    pub repository_id: SemanticId,
    pub next_step: NextStep,
    /// Any rule candidate still `applicability_state: "candidate"`.
    pub has_undecided_candidates: bool,
}

/// One declared connection reference after resolution and composition.
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionSlot {
    Unresolved,
    Resolved {
        record: Box<ResolvedRecord>,
        /// `Some` only for a pin-confirmed connection the composition
        /// accepted with no problem of its own.
        accepted: Option<ConnectionFacts>,
    },
}

/// A resolved composed record and the typed input its own contract's
/// schema route produced from it (`Err`: that route's own rejection,
/// prefix-free, before any typed input existed).
#[derive(Debug, Clone, PartialEq)]
pub struct TypedRecord<T> {
    pub record: ResolvedRecord,
    pub typed: Result<T, Vec<Diagnostic>>,
}

/// A nullable pinned reference after resolution.
#[derive(Debug, Clone, PartialEq)]
pub enum RefSlot<T> {
    /// The reference is `null`.
    NotDeclared,
    Unresolved,
    Resolved(TypedRecord<T>),
}

/// Everything a qualification entry composes.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceComposition {
    /// Parallel to `workspace_connection_refs`.
    pub connections: Vec<ConnectionSlot>,
    /// The one connection composition's own diagnostics, prefix-free.
    pub connection_diagnostics: Vec<Diagnostic>,
    pub plan: RefSlot<PlanInput>,
    pub export: RefSlot<ExportInput>,
}

/// The external boundaries of a composed plan and export.
#[derive(Debug, Clone, Copy)]
pub struct MigrationBoundaries<'a> {
    pub plan: &'a PlanResolution,
    pub source_content: &'a ResponseCatalogue<SourceContentResponse>,
}

/// An accepted qualification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceQualification {
    pub id: SemanticId,
    pub scope: Scope,
    pub state: QualificationState,
}

/// The inputs of the closed decision matrix
/// (`workspace-compatibility-qualification.md` §4.1), read only from
/// accepted records. `next_steps` has one element per declared connection:
/// `None` for one that is not accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionInputs {
    pub next_steps: Vec<Option<NextStep>>,
    pub has_undecided_candidates: bool,
    pub plan: PlanSignal,
    pub has_accepted_export: bool,
}

/// What the decision matrix reads from the composed migration plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanSignal {
    /// No plan was composed (`null`, unresolved or not pinned).
    Absent,
    /// A pinned plan its own check rejected: an explicit unknown.
    Rejected,
    Accepted {
        overall: Verdict,
        mints_records: bool,
    },
}

/// `computeQualificationState`: nine rows over eight logical steps.
pub fn compute_qualification_state(inputs: &DecisionInputs) -> QualificationState {
    let steps = &inputs.next_steps;
    if steps.iter().any(|s| {
        matches!(
            s,
            Some(NextStep::ResolveConflict) | Some(NextStep::ResolveAmbiguity)
        )
    }) {
        return QualificationState::Blocked;
    }
    if steps.is_empty()
        || steps
            .iter()
            .any(|s| *s != Some(NextStep::ContinueCompatibilityMode))
    {
        return QualificationState::Unverified;
    }
    if inputs.has_undecided_candidates {
        return QualificationState::Unverified;
    }
    match inputs.plan {
        PlanSignal::Absent => QualificationState::Qualified,
        PlanSignal::Rejected => QualificationState::Unverified,
        PlanSignal::Accepted {
            overall: Verdict::Blocked,
            ..
        } => QualificationState::Blocked,
        PlanSignal::Accepted { overall, .. } if overall != Verdict::Verified => {
            QualificationState::Unverified
        }
        PlanSignal::Accepted {
            mints_records: true,
            ..
        } if !inputs.has_accepted_export => QualificationState::Unverified,
        PlanSignal::Accepted { .. } => QualificationState::Qualified,
    }
}

/// `canonicalConnectionSourceRef`: the declared ids, sorted by code unit.
fn connection_source_ref(ids: &[&str]) -> String {
    let mut sorted: Vec<&str> = ids.to_vec();
    sorted.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    format!("workspace-connection-scan:{}", sorted.join("+"))
}

fn json_list(items: &[&str]) -> String {
    format!(
        "[{}]",
        items
            .iter()
            .map(|s| crate::run_contracts::json_quote(s))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn check_head(at: &str, head: &RecordHead, problems: &mut Vec<Diagnostic>) {
    if head.record_type.as_str() != RECORD_TYPE {
        problems.push(fail(format!(
            "{at} declares record_type \"{}\", not \"{RECORD_TYPE}\"",
            head.record_type
        )));
    }
    let scope_type = head.scope.scope_type();
    if !matches!(
        scope_type,
        ScopeType::ProjectWorkspace | ScopeType::RepositoryScope
    ) {
        problems.push(fail(format!(
            "{at} scope.type is \"{scope_type}\"; a qualification is scoped to project-workspace or repository-scope only"
        )));
    }
    let origin = head.origin_kind();
    if origin != OriginKind::Derived {
        problems.push(fail(format!(
            "{at} origin.kind is \"{origin}\", not \"derived\"; a qualification is computed from the composed records it pins, never declared on its own"
        )));
    }
    let authority = head.authority.kind;
    if authority != AuthorityKind::DelegatedRun {
        problems.push(fail(format!(
            "{at} authority.kind is \"{authority}\", not \"delegated-run\"; computing a qualification is carried out by a run and never itself mints an owner decision"
        )));
    }
}

/// `checkNoDuplicateRefs` over the declared connection references.
fn check_duplicate_refs(at: &str, refs: &[QualificationPin], problems: &mut Vec<Diagnostic>) {
    let field = "payload.workspace_connection_refs";
    let mut ids: Vec<(&str, usize)> = Vec::new();
    let mut references: Vec<(&str, usize)> = Vec::new();
    for (i, pin) in refs.iter().enumerate() {
        let id = pin.id.as_str();
        match ids.iter().find(|(v, _)| *v == id) {
            Some((_, first)) => problems.push(fail(format!(
                "{at} {field}[{i}].id \"{id}\" repeats {field}[{first}].id; each declared reference in this array must name a distinct composed record"
            ))),
            None => ids.push((id, i)),
        }
        let reference = pin.reference.as_str();
        match references.iter().find(|(v, _)| *v == reference) {
            Some((_, first)) => problems.push(fail(format!(
                "{at} {field}[{i}].reference \"{reference}\" repeats {field}[{first}].reference; each declared reference in this array must name a distinct composed record"
            ))),
            None => references.push((reference, i)),
        }
    }
}

/// The connection references: resolution echo, pin, scope agreement (on
/// accepted connections), the envelope's origin, the composition's own
/// problems and the exact repository set.
fn check_connections(
    at: &str,
    entry: &WorkspaceQualificationInput,
    composition: &WorkspaceComposition,
    problems: &mut Vec<Diagnostic>,
) {
    let refs = &entry.payload.workspace_connection_refs;
    let scope = &entry.head.scope;
    if refs.is_empty() {
        problems.push(fail(format!(
            "{at} payload.workspace_connection_refs is missing or empty; a qualification without at least one pinned, resolvable workspace connection scan does not exist"
        )));
    }
    if scope.scope_type() == ScopeType::RepositoryScope
        && refs.len() > REPOSITORY_SCOPE_MAX_CONNECTION_REFS
    {
        problems.push(fail(format!(
            "{at} scope.type \"repository-scope\" names exactly one repository through scope.id, so payload.workspace_connection_refs may carry at most {REPOSITORY_SCOPE_MAX_CONNECTION_REFS} entry (found {}); a multi-repository workspace is expressed only through one \"project-workspace\" record's workspace_connection_refs, never through several \"repository-scope\" records",
            refs.len()
        )));
    }
    check_duplicate_refs(at, refs, problems);

    for (i, pin) in refs.iter().enumerate() {
        let field = format!("payload.workspace_connection_refs[{i}]");
        let (record, accepted) = match composition.connections.get(i) {
            Some(ConnectionSlot::Resolved { record, accepted }) => {
                (Some(record.as_ref()), accepted.as_ref())
            }
            _ => (None, None),
        };
        if !check_resolution(
            at,
            &field,
            pin,
            record,
            PinnedRecordKind::WorkspaceConnectionScan,
            problems,
        ) {
            continue;
        }
        let Some(record) = record else { continue };
        let digest = record.content_digest();
        if !pin.confirms(&digest) {
            problems.push(fail(format!(
                "{at} {field}.sha256 \"{}\" does not equal the resolved connection's own recomputed content digest \"{}\"; a pinned reference names the exact composed version, never a bare id/reference an independent resolver could satisfy with different content under the same label",
                pin.sha256,
                digest.value()
            )));
            continue;
        }
        if let Some(facts) = accepted {
            if !scope.same_scope(&facts.scope) {
                problems.push(fail(format!(
                    "{at} scope does not match the resolved {field} record's scope; a qualification cannot claim a different scope than any connection scan it pins"
                )));
            }
        }
    }

    let declared_ids: Vec<&str> = refs.iter().map(|p| p.id.as_str()).collect();
    let expected = connection_source_ref(&declared_ids);
    if entry.head.origin_source_ref() != Some(expected.as_str()) {
        problems.push(fail(format!(
            "{at} origin.source_ref \"{}\" does not equal \"{expected}\"; the envelope and payload must pin the SAME set of composed workspace connections",
            entry.head.origin_source_ref().unwrap_or("undefined")
        )));
    }
    problems.extend(prefixed(
        &format!("{at} workspace_connection_refs: "),
        &composition.connection_diagnostics,
    ));

    let accepted: Vec<&ConnectionFacts> = composition
        .connections
        .iter()
        .filter_map(|slot| match slot {
            ConnectionSlot::Resolved {
                accepted: Some(facts),
                ..
            } => Some(facts),
            _ => None,
        })
        .collect();
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for facts in &accepted {
        let id = facts.repository_id.as_str();
        match counts.iter_mut().find(|(r, _)| *r == id) {
            Some(slot) => slot.1 += 1,
            None => counts.push((id, 1)),
        }
    }
    for (repository, count) in &counts {
        if *count > 1 {
            problems.push(fail(format!(
                "{at} payload.workspace_connection_refs carries {count} resolved scans all naming the same payload.repository.id \"{repository}\"; each declared repository is scanned at most once"
            )));
        }
    }
    if scope.scope_type() == ScopeType::ProjectWorkspace {
        let declared: Vec<&str> = entry
            .payload
            .workspace_repository_ids
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(SemanticId::as_str)
            .collect();
        let missing: Vec<&str> = counts
            .iter()
            .map(|(r, _)| *r)
            .filter(|r| !declared.contains(r))
            .collect();
        if !missing.is_empty() {
            problems.push(fail(format!(
                "{at} payload.workspace_repository_ids is missing {}, actually scanned by a resolved connection but not declared",
                json_list(&missing)
            )));
        }
        // Which repositories were scanned is known only when EVERY declared
        // connection is accepted; otherwise the gap is not reported as a
        // repository that was never scanned.
        if accepted.len() == refs.len() {
            let mut unscanned: Vec<&str> = Vec::new();
            for r in &declared {
                if !counts.iter().any(|(c, _)| c == r) && !unscanned.contains(r) {
                    unscanned.push(r);
                }
            }
            if !unscanned.is_empty() {
                problems.push(fail(format!(
                    "{at} payload.workspace_repository_ids declares {}, which no resolved connection's payload.repository.id names",
                    json_list(&unscanned)
                )));
            }
        }
    }
}

/// The plan reference: resolution echo, typed route, pin, scope, the real
/// plan check. Returns the pinned plan and its composed result.
fn check_plan(
    at: &str,
    entry: &WorkspaceQualificationInput,
    slot: &RefSlot<PlanInput>,
    boundaries: MigrationBoundaries<'_>,
    problems: &mut Vec<Diagnostic>,
) -> Option<(PinnedPlan, Option<MigrationPlan>)> {
    let pin = entry.payload.migration_plan_ref.as_ref()?;
    let field = "payload.migration_plan_ref";
    let resolved = match slot {
        RefSlot::Resolved(typed) => Some(typed),
        _ => None,
    };
    if !check_resolution(
        at,
        field,
        pin,
        resolved.map(|t| &t.record),
        PinnedRecordKind::InstanceMigrationPlan,
        problems,
    ) {
        return None;
    }
    let typed = resolved?;
    let input = match &typed.typed {
        Err(route) => {
            problems.extend(prefixed(&format!("{at} migration_plan_ref: "), route));
            return None;
        }
        Ok(input) => input.clone(),
    };
    let pinned = match PinnedPlan::confirm(input, pin.sha256.as_str()) {
        Ok(pinned) => pinned,
        Err(recomputed) => {
            problems.push(fail(format!(
                "{at} {field}.sha256 \"{}\" does not equal the resolved plan's own recomputed plan_fingerprint \"{}\"; a pinned reference names the exact composed version, never a bare id an independent resolver could satisfy with a differently-fingerprinted plan of the same id",
                pin.sha256,
                recomputed.value()
            )));
            return None;
        }
    };
    if !entry.head.scope.same_scope(&pinned.input().head.scope) {
        problems.push(fail(format!(
            "{at} scope does not match the resolved {field} record's scope; a composed migration plan must describe the same workspace or repository as the connections it is qualified alongside"
        )));
    }
    let outcome = check_migration_plans(std::slice::from_ref(pinned.input()), boundaries.plan);
    problems.extend(prefixed(
        &format!("{at} migration_plan_ref: "),
        &outcome.diagnostics,
    ));
    let accepted = outcome.accepted.into_iter().next().flatten();
    Some((pinned, accepted))
}

/// The export reference, anchored to the already-pinned plan.
fn check_export(
    at: &str,
    entry: &WorkspaceQualificationInput,
    slot: &RefSlot<ExportInput>,
    plan: Option<&PinnedPlan>,
    boundaries: MigrationBoundaries<'_>,
    problems: &mut Vec<Diagnostic>,
) -> Option<CanonicalExport> {
    let pin = entry.payload.canonical_export_ref.as_ref()?;
    let field = "payload.canonical_export_ref";
    let Some(plan) = plan else {
        problems.push(fail(format!(
            "{at} {field} is present without a resolvable payload.migration_plan_ref; an export proves a plan's own targets and cannot be composed without one"
        )));
        return None;
    };
    let resolved = match slot {
        RefSlot::Resolved(typed) => Some(typed),
        _ => None,
    };
    if !check_resolution(
        at,
        field,
        pin,
        resolved.map(|t| &t.record),
        PinnedRecordKind::InstanceCanonicalExport,
        problems,
    ) {
        return None;
    }
    let typed = resolved?;
    let input = match &typed.typed {
        Err(route) => {
            problems.extend(prefixed(&format!("{at} canonical_export_ref: "), route));
            return None;
        }
        Ok(input) => input.clone(),
    };
    let pinned = match PinnedExport::confirm(input, pin.sha256.as_str()) {
        Ok(pinned) => pinned,
        Err(recomputed) => {
            problems.push(fail(format!(
                "{at} {field}.sha256 \"{}\" does not equal the resolved export's own recomputed digest \"{}\"; a pinned reference names the exact composed version, never a bare id",
                pin.sha256,
                recomputed.value()
            )));
            return None;
        }
    };
    let export_plan_ref = pinned.input().payload.plan_ref.as_str();
    let plan_id = plan.input().head.id.as_str();
    if export_plan_ref != plan_id {
        problems.push(fail(format!(
            "{at} {field}'s resolved record names plan_ref \"{export_plan_ref}\", not \"{plan_id}\"; the composed export must prove the SAME pinned plan, not a different one"
        )));
    }
    if !entry.head.scope.same_scope(&pinned.input().head.scope) {
        problems.push(fail(format!(
            "{at} scope does not match the resolved {field} record's scope; a composed export must describe the same workspace or repository as the plan it proves"
        )));
    }
    let outcome = check_canonical_exports(
        std::slice::from_ref(pinned.input()),
        PlanBoundary::Pinned(plan),
        boundaries.source_content,
    );
    problems.extend(prefixed(
        &format!("{at} canonical_export_ref: "),
        &outcome.diagnostics,
    ));
    outcome.accepted.into_iter().next().flatten()
}

/// One entry: every problem, and the recomputed state.
fn check_entry(
    entry: &WorkspaceQualificationInput,
    composition: &WorkspaceComposition,
    boundaries: MigrationBoundaries<'_>,
    problems: &mut Vec<Diagnostic>,
) -> QualificationState {
    let at = format!("qualification \"{}\"", entry.head.id);
    check_head(&at, &entry.head, problems);
    check_connections(&at, entry, composition, problems);
    let plan = check_plan(&at, entry, &composition.plan, boundaries, problems);
    let export = check_export(
        &at,
        entry,
        &composition.export,
        plan.as_ref().map(|(pinned, _)| pinned),
        boundaries,
        problems,
    );

    let next_steps = (0..entry.payload.workspace_connection_refs.len())
        .map(|i| match composition.connections.get(i) {
            Some(ConnectionSlot::Resolved {
                accepted: Some(facts),
                ..
            }) => Some(facts.next_step),
            _ => None,
        })
        .collect();
    let has_undecided_candidates = composition.connections.iter().any(|slot| {
        matches!(
            slot,
            ConnectionSlot::Resolved {
                accepted: Some(ConnectionFacts {
                    has_undecided_candidates: true,
                    ..
                }),
                ..
            }
        )
    });
    let plan_signal = match &plan {
        None => PlanSignal::Absent,
        Some((_, None)) => PlanSignal::Rejected,
        Some((_, Some(accepted))) => PlanSignal::Accepted {
            overall: accepted.overall_status(),
            mints_records: accepted.mints_records(),
        },
    };
    let expected = compute_qualification_state(&DecisionInputs {
        next_steps,
        has_undecided_candidates,
        plan: plan_signal,
        has_accepted_export: export.is_some(),
    });
    let declared = entry.payload.qualification_state;
    if declared != expected {
        problems.push(fail(format!(
            "{at} qualification_state is \"{declared}\", but the closed decision matrix over the composed records' own next_steps/pending rule-candidate decisions/verification.overall_status/canonical export coverage computes \"{expected}\""
        )));
    }
    let blockers = &entry.payload.blockers;
    let questions = &entry.payload.open_questions;
    let non_empty = |v: &[RecordText]| if v.is_empty() { "empty" } else { "non-empty" };
    if (declared == QualificationState::Blocked) != !blockers.is_empty() {
        problems.push(fail(format!(
            "{at} blockers is {} but qualification_state is \"{declared}\"; blockers is non-empty exactly when qualification_state is \"BLOCKED\"",
            non_empty(blockers)
        )));
    }
    if (declared == QualificationState::Unverified) != !questions.is_empty() {
        problems.push(fail(format!(
            "{at} open_questions is {} but qualification_state is \"{declared}\"; open_questions is non-empty exactly when qualification_state is \"UNVERIFIED\" — a verifiable, unresolved reason must be named, never a bare label",
            non_empty(questions)
        )));
    }
    expected
}

/// The result of one batch of qualifications.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceBatchOutcome {
    pub diagnostics: Vec<Diagnostic>,
    pub accepted: Vec<Option<WorkspaceQualification>>,
}

/// `evaluateWorkspaceCompatibilityQualification` over one schema-clean
/// container's `qualifications`, each with its own composition.
pub fn check_workspace_qualifications(
    entries: &[(WorkspaceQualificationInput, WorkspaceComposition)],
    boundaries: MigrationBoundaries<'_>,
) -> WorkspaceBatchOutcome {
    let mut diagnostics = Vec::new();
    let mut accepted = Vec::with_capacity(entries.len());
    let mut seen: Vec<&str> = Vec::new();
    for (entry, composition) in entries {
        let mut problems = Vec::new();
        let id = entry.head.id.as_str();
        if seen.contains(&id) {
            problems.push(fail(format!(
                "qualification \"{id}\" is declared more than once"
            )));
        } else {
            seen.push(id);
        }
        let state = check_entry(entry, composition, boundaries, &mut problems);
        accepted.push(problems.is_empty().then(|| WorkspaceQualification {
            id: entry.head.id.clone(),
            scope: entry.head.scope.clone(),
            state,
        }));
        diagnostics.extend(problems);
    }
    WorkspaceBatchOutcome {
        diagnostics,
        accepted,
    }
}
