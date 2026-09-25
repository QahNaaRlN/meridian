//! `bounded-context-manifest`: the bounded, resumable context of ONE run
//! (`registries/operating-model/context-manifest.schema.json`).
//!
//! The manifest relates to exactly one run, deterministically: three closed
//! [`PinnedRef`]s, a `run-state` scope whose id is the pinned run's id, and
//! a [`RunStateCheckpoint`] that is resolved through a
//! [`ResolutionCatalogue`] the manifest does not control and checked
//! against the state the RESOLVED execution-run response carries. With no
//! catalogue the manifest fails closed. [`check_context_manifest`] is the
//! ONLY way to obtain an accepted [`ContextManifest`]; it reports every
//! rule in the Node reference's diagnostic order.

use std::collections::HashSet;

use crate::task_contracts::non_portable_reason;
use crate::types::{Diagnostic, SemanticId};

use super::envelope::{check_envelope, RecordEnvelope, RecordFamily};
use super::execution_state::{check_blocker_list, check_unique_refs, Blocker};
use super::identity::{PortableRef, RecordText, RunStateScope, ScopeRevision};
use super::pinned_ref::{check_pinned_ref, site_label, PinSite, PinnedRef, PinnedSlot};
use super::resolution::{
    check_resolved_entry, response_scope_revision_json, response_text, response_text_json,
    response_text_set, same_resolved_edition, same_step, step_json, ResolutionCatalogue,
    ResolvedEntry,
};
use super::response::ResponseField;
use super::revision::{pin_defect, PinSha256, RevisionClass};
use super::vocabulary::{LifecycleStage, WorkStatus};
use super::{fail, json_quote, NextStep};

/// The three pinned references, one per [`PinnedSlot`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedRefs {
    pub execution_run: PinnedRef,
    pub task_specification: PinnedRef,
    pub human_control: PinnedRef,
}

impl PinnedRefs {
    pub fn get(&self, slot: PinnedSlot) -> &PinnedRef {
        match slot {
            PinnedSlot::ExecutionRun => &self.execution_run,
            PinnedSlot::TaskSpecification => &self.task_specification,
            PinnedSlot::HumanControl => &self.human_control,
        }
    }
}

/// One input the run needs to continue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthoritativeSource {
    pub id: SemanticId,
    pub reference: PortableRef,
    pub purpose: RecordText,
    pub mutable: bool,
    pub revision: Option<RecordText>,
    pub sha256: Option<PinSha256>,
}

/// One norm that applies to the run, with its rationale and pin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicableNorm {
    pub id: SemanticId,
    pub reference: PortableRef,
    pub origin: RecordText,
    pub revision: Option<RecordText>,
    pub sha256: Option<PinSha256>,
    pub applicability_rationale: RecordText,
}

/// A decision, open question or known gap: a stable id and its statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentifiedItem {
    pub id: SemanticId,
    pub text: RecordText,
}

/// The pinned authoritative snapshot of the run's state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunStateCheckpoint {
    pub pins: PinnedRefs,
    pub scope_revision: ScopeRevision,
    pub lifecycle_stage: LifecycleStage,
    pub work_status: WorkStatus,
    pub next_action: NextStep,
    pub next_gate: NextStep,
    pub blocker_ids: Vec<SemanticId>,
    pub resolved_norms: Vec<PortableRef>,
    pub completed_checks: Vec<PortableRef>,
}

/// The manifest body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestBody {
    pub pins: PinnedRefs,
    pub scope_revision: ScopeRevision,
    pub next_action: NextStep,
    pub next_gate: NextStep,
    pub authoritative_sources: Vec<AuthoritativeSource>,
    pub applicable_norms: Vec<ApplicableNorm>,
    pub decisions: Vec<IdentifiedItem>,
    pub open_questions: Vec<IdentifiedItem>,
    pub completed_actions: Vec<PortableRef>,
    pub completed_checks: Vec<PortableRef>,
    pub known_gaps: Vec<IdentifiedItem>,
    pub blockers: Vec<Blocker>,
    pub checkpoint: RunStateCheckpoint,
}

/// One schema-clean context-manifest record, not yet domain-checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextManifestInput {
    pub envelope: RecordEnvelope,
    pub scope: RunStateScope,
    pub body: ManifestBody,
}

/// A manifest with zero domain and resolution problems: every pin is
/// resolved, pinned to an exact edition and agrees with its checkpoint,
/// and the checkpoint reproduces the resolved run's own state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextManifest(ContextManifestInput);

impl ContextManifest {
    pub fn envelope(&self) -> &RecordEnvelope {
        &self.0.envelope
    }

    pub fn scope(&self) -> &RunStateScope {
        &self.0.scope
    }

    pub fn body(&self) -> &ManifestBody {
        &self.0.body
    }

    pub fn checkpoint(&self) -> &RunStateCheckpoint {
        &self.0.body.checkpoint
    }
}

/// Every manifest rule. `resolution` is the external boundary; `None`
/// fails closed.
pub fn check_context_manifest(
    input: ContextManifestInput,
    resolution: Option<&ResolutionCatalogue>,
) -> (Option<ContextManifest>, Vec<Diagnostic>) {
    let mut problems = Vec::new();
    check_envelope(
        RecordFamily::ContextManifest,
        &input.envelope,
        &mut problems,
    );
    let id = input.envelope.id.as_str();
    let subject = format!("context manifest \"{id}\"");
    let body = &input.body;
    let run_id = &body.pins.execution_run.id;

    for slot in PinnedSlot::ALL {
        check_pinned_ref(
            id,
            PinSite::Manifest,
            slot,
            body.pins.get(slot),
            run_id,
            &mut problems,
        );
    }
    let scope_run = input.scope.run_id().as_str();
    if scope_run != run_id.as_str() {
        problems.push(fail(format!(
            "{subject} scope identifies run \"{scope_run}\" but execution_run_ref.id is \"{}\"; the two must be the exact same string, not one a substring of the other",
            run_id.as_str()
        )));
    }

    check_sources(&subject, &body.authoritative_sources, &mut problems);
    let norm_refs = check_norms(&subject, &body.applicable_norms, &mut problems);
    for (label, items, field) in [
        ("decisions", &body.decisions, "statement"),
        ("open_questions", &body.open_questions, "question"),
        ("known_gaps", &body.known_gaps, "description"),
    ] {
        check_identified_items(&subject, label, items, field, &mut problems);
    }
    check_unique_refs(
        &subject,
        "completed_actions",
        &body.completed_actions,
        &mut problems,
    );
    check_unique_refs(
        &subject,
        "completed_checks",
        &body.completed_checks,
        &mut problems,
    );
    let blocker_ids = check_blocker_list(&subject, &body.blockers, &mut problems);

    check_checkpoint(id, &subject, body, &blocker_ids, &norm_refs, &mut problems);

    match resolution {
        None => problems.push(fail(format!(
            "{subject} cannot be verified: no external record resolver was supplied; the pinned execution-run, task-specification and run-human-control records are resolved OUTSIDE the manifest and checked against it — without that boundary the run_state_checkpoint is only a second unanchored copy"
        ))),
        Some(catalogue) => check_resolution(id, &subject, body, catalogue, &mut problems),
    }

    for (name, step) in [
        ("next_action", &body.next_action),
        ("next_gate", &body.next_gate),
    ] {
        if let Some(text) = step {
            if text.is_blank() {
                problems.push(fail(format!(
                    "{subject} {name} is present but empty; state an action, or null where there is no continuation"
                )));
            }
            if let Some(r) = non_portable_reason(Some(text.as_str())) {
                problems.push(fail(format!("{subject} {name} contains {r}")));
            }
        }
    }

    let accepted = problems.is_empty().then_some(ContextManifest(input));
    (accepted, problems)
}

fn check_sources(subject: &str, sources: &[AuthoritativeSource], problems: &mut Vec<Diagnostic>) {
    if sources.is_empty() {
        problems.push(fail(format!(
            "{subject} lists no authoritative sources; a resumable run names at least the inputs it needs to continue"
        )));
        return;
    }
    let mut seen_id = HashSet::new();
    let mut seen_ref = HashSet::new();
    for source in sources {
        let at = source.id.as_str();
        if !seen_id.insert(at) {
            problems.push(fail(format!(
                "{subject} authoritative source id \"{at}\" is used more than once"
            )));
        }
        let reference = source.reference.as_str();
        if source.reference.is_blank() {
            problems.push(fail(format!(
                "{subject} authoritative source {at} has no reference"
            )));
        } else {
            if let Some(r) = non_portable_reason(Some(reference)) {
                problems.push(fail(format!(
                    "{subject} authoritative source {at} reference contains {r}"
                )));
            }
            if !seen_ref.insert(reference.trim()) {
                problems.push(fail(format!(
                    "{subject} authoritative source reference \"{reference}\" is listed more than once"
                )));
            }
        }
        if source.purpose.is_blank() {
            problems.push(fail(format!(
                "{subject} authoritative source {at} states no purpose; a source not needed to continue is not listed"
            )));
        } else if let Some(r) = non_portable_reason(Some(source.purpose.as_str())) {
            problems.push(fail(format!(
                "{subject} authoritative source {at} purpose contains {r}"
            )));
        }
        let revision = source.revision.as_ref().map(RecordText::as_str);
        if let Some(r) = revision.and_then(|rev| non_portable_reason(Some(rev))) {
            problems.push(fail(format!(
                "{subject} authoritative source {at} revision contains {r}"
            )));
        }
        if source.mutable {
            if let Some(defect) = pin_defect(revision, source.sha256.is_some()) {
                problems.push(fail(format!(
                    "{subject} authoritative source {at} is mutable but not pinned: {defect}"
                )));
            }
        } else if RevisionClass::classify(revision) == RevisionClass::Floating {
            problems.push(fail(format!(
                "{subject} authoritative source {at} is declared immutable but its revision {} is a branch or channel reference",
                revision.map_or_else(|| "null".to_string(), json_quote)
            )));
        }
    }
}

/// Returns the trimmed, non-blank norm references (the manifest's view of
/// the run's resolved norm set).
fn check_norms(
    subject: &str,
    norms: &[ApplicableNorm],
    problems: &mut Vec<Diagnostic>,
) -> HashSet<String> {
    let mut seen_id = HashSet::new();
    let mut refs = HashSet::new();
    for norm in norms {
        let at = norm.id.as_str();
        if !seen_id.insert(at) {
            problems.push(fail(format!(
                "{subject} applicable norm id \"{at}\" is used more than once"
            )));
        }
        for (field, human, text) in [
            ("reference", "reference", norm.reference.as_str()),
            ("origin", "origin", norm.origin.as_str()),
            (
                "applicability_rationale",
                "applicability rationale",
                norm.applicability_rationale.as_str(),
            ),
        ] {
            if text.trim().is_empty() {
                problems.push(fail(format!(
                    "{subject} applicable norm {at} has no {human}"
                )));
            } else if let Some(r) = non_portable_reason(Some(text)) {
                problems.push(fail(format!(
                    "{subject} applicable norm {at} {field} contains {r}"
                )));
            }
        }
        let reference = norm.reference.as_str();
        if !norm.reference.is_blank() && !refs.insert(reference.trim().to_string()) {
            problems.push(fail(format!(
                "{subject} applicable norm reference \"{reference}\" is listed more than once"
            )));
        }
        let revision = norm.revision.as_ref().map(RecordText::as_str);
        if let Some(r) = revision.and_then(|rev| non_portable_reason(Some(rev))) {
            problems.push(fail(format!(
                "{subject} applicable norm {at} revision contains {r}"
            )));
        }
        if let Some(defect) = pin_defect(revision, norm.sha256.is_some()) {
            problems.push(fail(format!(
                "{subject} applicable norm {at} is not pinned: {defect}"
            )));
        }
    }
    refs
}

fn check_identified_items(
    subject: &str,
    label: &str,
    items: &[IdentifiedItem],
    field: &str,
    problems: &mut Vec<Diagnostic>,
) {
    let mut seen = HashSet::new();
    for item in items {
        let at = item.id.as_str();
        if !seen.insert(at) {
            problems.push(fail(format!(
                "{subject} {label} id \"{at}\" is used more than once"
            )));
        }
        if item.text.is_blank() {
            problems.push(fail(format!("{subject} {label} entry {at} has no {field}")));
        } else if let Some(r) = non_portable_reason(Some(item.text.as_str())) {
            problems.push(fail(format!(
                "{subject} {label} entry {at} {field} contains {r}"
            )));
        }
    }
}

fn step_text_json(step: &NextStep) -> String {
    step.as_ref()
        .map_or_else(|| "null".to_string(), |s| json_quote(s.as_str()))
}

/// The checkpoint's own pins, its agreement with the manifest, and its
/// internal axis rules.
fn check_checkpoint(
    id: &str,
    subject: &str,
    body: &ManifestBody,
    blocker_ids: &HashSet<String>,
    norm_refs: &HashSet<String>,
    problems: &mut Vec<Diagnostic>,
) {
    let cp = &body.checkpoint;
    let run_id = &body.pins.execution_run.id;
    for slot in PinnedSlot::ALL {
        let pin = cp.pins.get(slot);
        check_pinned_ref(id, PinSite::Checkpoint, slot, pin, run_id, problems);
        if pin != body.pins.get(slot) {
            let field = slot.field();
            problems.push(fail(format!(
                "{subject} run_state_checkpoint.{field} does not equal the manifest's {field} field for field; the checkpoint pins the SAME record edition as the manifest, not a parallel one"
            )));
        }
    }

    for (name, step) in [
        ("next_action", &cp.next_action),
        ("next_gate", &cp.next_gate),
    ] {
        if let Some(text) = step {
            if text.is_blank() {
                problems.push(fail(format!(
                    "{subject} run_state_checkpoint.{name} is present but empty; state an action, or null where there is no continuation"
                )));
            }
            if let Some(r) = non_portable_reason(Some(text.as_str())) {
                problems.push(fail(format!(
                    "{subject} run_state_checkpoint.{name} contains {r}"
                )));
            }
        }
    }
    let cp_checks = check_unique_refs(
        subject,
        "run_state_checkpoint.completed_checks",
        &cp.completed_checks,
        problems,
    );
    let cp_norms = check_unique_refs(
        subject,
        "run_state_checkpoint.resolved_norms",
        &cp.resolved_norms,
        problems,
    );
    let cp_blockers: HashSet<&str> = cp.blocker_ids.iter().map(SemanticId::as_str).collect();

    if body.scope_revision != cp.scope_revision {
        problems.push(fail(format!(
            "{subject} scope_revision {} contradicts the pinned run state (run_state_checkpoint.scope_revision {})",
            body.scope_revision, cp.scope_revision
        )));
    }
    for (name, manifest_step, cp_step) in [
        ("next_action", &body.next_action, &cp.next_action),
        ("next_gate", &body.next_gate, &cp.next_gate),
    ] {
        if manifest_step != cp_step {
            problems.push(fail(format!(
                "{subject} {name} {} contradicts the pinned run state (run_state_checkpoint.{name} {})",
                step_text_json(manifest_step),
                step_text_json(cp_step)
            )));
        }
    }
    let manifest_blockers: HashSet<&str> = blocker_ids.iter().map(String::as_str).collect();
    if manifest_blockers != cp_blockers {
        problems.push(fail(format!(
            "{subject} blockers do not match the pinned run state (run_state_checkpoint.blocker_ids); the two sets of blocker ids must be the same"
        )));
    }
    let manifest_checks: HashSet<&str> = body
        .completed_checks
        .iter()
        .map(|c| c.as_str().trim())
        .collect();
    let cp_checks: HashSet<&str> = cp_checks.iter().map(String::as_str).collect();
    if manifest_checks != cp_checks {
        problems.push(fail(format!(
            "{subject} completed_checks do not match the pinned run state (run_state_checkpoint.completed_checks); the two sets must be the same"
        )));
    }
    let cp_norms: HashSet<&str> = cp_norms.iter().map(String::as_str).collect();
    let norm_refs: HashSet<&str> = norm_refs.iter().map(String::as_str).collect();
    if norm_refs != cp_norms {
        problems.push(fail(format!(
            "{subject} applicable_norms references do not match the pinned run state (run_state_checkpoint.resolved_norms); the manifest adds an applicability rationale but cannot add to or drop from the run's resolved norm set"
        )));
    }

    let ws = cp.work_status;
    if ws == WorkStatus::Blocked && cp_blockers.is_empty() {
        problems.push(fail(format!(
            "{subject} run_state_checkpoint work_status is \"blocked\" with no blocker id; \"blocked\" means continuation is impossible until a stated external condition changes"
        )));
    }
    if ws != WorkStatus::Blocked && !cp_blockers.is_empty() {
        problems.push(fail(format!(
            "{subject} run_state_checkpoint carries {} blocker id(s) but work_status is \"{ws}\"; a non-empty blocker set agrees with work_status \"blocked\"",
            cp_blockers.len()
        )));
    }
    if ws.is_terminal() {
        for (name, step) in [
            ("next_action", &cp.next_action),
            ("next_gate", &cp.next_gate),
        ] {
            if step.is_some() {
                problems.push(fail(format!(
                    "{subject} run_state_checkpoint work_status is \"{ws}\" but {name} carries an executable value; a completed or cancelled run does not silently carry a next executable step"
                )));
            }
        }
    }
    if ws == WorkStatus::WaitingHuman && cp.next_action.as_ref().is_none_or(RecordText::is_blank) {
        problems.push(fail(format!(
            "{subject} run_state_checkpoint work_status is \"waiting_human\" but next_action names no concrete human action; \"waiting_human\" means a specific required human action is known"
        )));
    }
}

/// The external resolution boundary: every pin resolved and checked, the
/// checkpoint's editions compared with the manifest's, and the checkpoint
/// axes compared with the resolved run's own state.
fn check_resolution(
    id: &str,
    subject: &str,
    body: &ManifestBody,
    catalogue: &ResolutionCatalogue,
    problems: &mut Vec<Diagnostic>,
) {
    let cp = &body.checkpoint;
    // One resolution per slot and site, in `PinnedSlot::ALL` order (the
    // manifest's three, then the checkpoint's three).
    let manifest: [Option<&ResolvedEntry>; 3] = PinnedSlot::ALL.map(|slot| {
        check_resolved_entry(
            RecordFamily::ContextManifest,
            id,
            &site_label(PinSite::Manifest, slot),
            slot.kind(),
            body.pins.get(slot),
            catalogue,
            problems,
        )
    });
    let checkpoint: [Option<&ResolvedEntry>; 3] = PinnedSlot::ALL.map(|slot| {
        check_resolved_entry(
            RecordFamily::ContextManifest,
            id,
            &site_label(PinSite::Checkpoint, slot),
            slot.kind(),
            cp.pins.get(slot),
            catalogue,
            problems,
        )
    });
    for ((slot, m), c) in PinnedSlot::ALL.into_iter().zip(manifest).zip(checkpoint) {
        same_resolved_edition(id, slot, m, c, problems);
    }
    let [run_cp, _, hc_cp] = checkpoint;

    let run_state = match run_cp.map(|e| &e.resolved_state) {
        Some(ResponseField::Present(state)) => Some(state),
        _ => None,
    };

    if let Some(state) = run_state {
        if let Some(rs_ref) = response_text(&state.task_specification_ref) {
            let payload_ref = body.pins.task_specification.reference.as_str();
            if rs_ref != payload_ref {
                problems.push(fail(format!(
                    "{subject} task_specification_ref names \"{payload_ref}\", but the resolved execution-run record names task specification \"{rs_ref}\"; the manifest carries the SAME specification the run resolved, not an independent one"
                )));
            }
        }
    }
    if let Some(hc) = hc_cp {
        if let Some(linked) = response_text(&hc.linked_run_ref) {
            let payload_ref = body.pins.execution_run.reference.as_str();
            if linked != payload_ref {
                problems.push(fail(format!(
                    "{subject} human_control_ref resolves to a run-human-control record whose linked_run_ref is \"{linked}\", not the manifest's pinned run \"{payload_ref}\""
                )));
            }
        }
    }

    let Some(state) = run_state else {
        return;
    };
    let mismatch = |axis: &str, have: String, want: String| {
        fail(format!(
            "{subject} run_state_checkpoint.{axis} {have} does not match the resolved execution-run record ({want}); the pinned snapshot reproduces the run's own resolved state, not an unanchored copy"
        ))
    };
    match &state.scope_revision {
        ResponseField::Absent => {}
        ResponseField::Present(n) if *n == cp.scope_revision.get() => {}
        want => problems.push(mismatch(
            "scope_revision",
            cp.scope_revision.to_string(),
            response_scope_revision_json(want),
        )),
    }
    for (axis, have, want) in [
        (
            "lifecycle_stage",
            cp.lifecycle_stage.as_str(),
            &state.lifecycle_stage,
        ),
        ("work_status", cp.work_status.as_str(), &state.work_status),
    ] {
        match want {
            ResponseField::Absent => {}
            ResponseField::Present(w) if w == have => {}
            want => problems.push(mismatch(axis, json_quote(have), response_text_json(want))),
        }
    }
    for (axis, have, want) in [
        ("next_action", &cp.next_action, &state.next_action),
        ("next_gate", &cp.next_gate, &state.next_gate),
    ] {
        if !matches!(want, ResponseField::Absent) && !same_step(have.as_ref(), want) {
            problems.push(mismatch(axis, step_text_json(have), step_json(want)));
        }
    }
    let cp_blocker_ids: Vec<&str> = cp.blocker_ids.iter().map(SemanticId::as_str).collect();
    let cp_norms: Vec<&str> = cp.resolved_norms.iter().map(PortableRef::as_str).collect();
    let cp_checks: Vec<&str> = cp
        .completed_checks
        .iter()
        .map(PortableRef::as_str)
        .collect();
    for (axis, have, want) in [
        ("blocker_ids", cp_blocker_ids, &state.blocker_ids),
        ("resolved_norms", cp_norms, &state.resolved_norms),
        ("completed_checks", cp_checks, &state.completed_checks),
    ] {
        let ResponseField::Present(items) = want else {
            continue;
        };
        let have: HashSet<&str> = have.iter().map(|s| s.trim()).collect();
        if have != response_text_set(items) {
            problems.push(fail(format!(
                "{subject} run_state_checkpoint.{axis} does not match the resolved execution-run record's {axis}; the pinned snapshot reproduces the run's own resolved set, not an unanchored copy"
            )));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::run_contracts::envelope::tests::envelope;
    use crate::run_contracts::identity::RunId;
    use crate::run_contracts::pinned_ref::PinnedRecordKind;
    use crate::run_contracts::resolution::ResolvedStateResponse;
    use crate::run_contracts::response::{ForeignValue, ResponseValueKind};
    use crate::types::WorkspaceId;

    fn text(s: &str) -> RecordText {
        RecordText::new(s).unwrap()
    }

    fn id(s: &str) -> SemanticId {
        SemanticId::new(s).unwrap()
    }

    fn pins() -> PinnedRefs {
        PinnedRefs {
            execution_run: PinnedRef {
                record_type: PinnedRecordKind::ExecutionRun,
                id: id("run-1"),
                run_id: None,
                reference: PortableRef::new("records/execution-run/run-1").unwrap(),
                revision: Some(text("v1.0.0")),
                sha256: None,
            },
            task_specification: PinnedRef {
                record_type: PinnedRecordKind::TaskSpecification,
                id: id("spec-1"),
                run_id: None,
                reference: PortableRef::new("records/task-specification/spec-1").unwrap(),
                revision: None,
                sha256: Some(PinSha256::new("a".repeat(64)).unwrap()),
            },
            human_control: PinnedRef {
                record_type: PinnedRecordKind::RunHumanControl,
                id: id("hc-1"),
                run_id: Some(id("run-1")),
                reference: PortableRef::new("records/run-human-control/run-1").unwrap(),
                revision: Some(text("v1.0.0")),
                sha256: None,
            },
        }
    }

    fn input() -> ContextManifestInput {
        ContextManifestInput {
            envelope: envelope(RecordFamily::ContextManifest),
            scope: RunStateScope::new(
                RunId::new("run-1").unwrap(),
                WorkspaceId::new("example-workspace").unwrap(),
            ),
            body: ManifestBody {
                pins: pins(),
                scope_revision: ScopeRevision::new(2).unwrap(),
                next_action: Some(text("do")),
                next_gate: None,
                authoritative_sources: vec![AuthoritativeSource {
                    id: id("owner-decision"),
                    reference: PortableRef::new("owner-decision:start").unwrap(),
                    purpose: text("Решение владельца."),
                    mutable: false,
                    revision: None,
                    sha256: None,
                }],
                applicable_norms: Vec::new(),
                decisions: Vec::new(),
                open_questions: Vec::new(),
                completed_actions: Vec::new(),
                completed_checks: Vec::new(),
                known_gaps: Vec::new(),
                blockers: Vec::new(),
                checkpoint: RunStateCheckpoint {
                    pins: pins(),
                    scope_revision: ScopeRevision::new(2).unwrap(),
                    lifecycle_stage: LifecycleStage::Execution,
                    work_status: WorkStatus::Active,
                    next_action: Some(text("do")),
                    next_gate: None,
                    blocker_ids: Vec::new(),
                    resolved_norms: Vec::new(),
                    completed_checks: Vec::new(),
                },
            },
        }
    }

    fn present(s: &str) -> ResponseField<String> {
        ResponseField::Present(s.to_string())
    }

    fn entry(record_type: &str, entry_id: &str, reference: &str) -> ResolvedEntry {
        ResolvedEntry {
            record_type: present(record_type),
            id: present(entry_id),
            reference: present(reference),
            revision: present("v1.0.0"),
            content_digest: ResponseField::Absent,
            source_bytes: ResponseField::Absent,
            ..ResolvedEntry::default()
        }
    }

    fn run_state() -> ResolvedStateResponse {
        ResolvedStateResponse {
            task_specification_ref: present("records/task-specification/spec-1"),
            scope_revision: ResponseField::Present(2),
            lifecycle_stage: present("execution"),
            work_status: present("active"),
            next_action: ResponseField::Present(Some("do".to_string())),
            next_gate: ResponseField::Present(None),
            blocker_ids: ResponseField::Present(Vec::new()),
            resolved_norms: ResponseField::Present(Vec::new()),
            completed_checks: ResponseField::Present(Vec::new()),
            unknown_fields: Vec::new(),
        }
    }

    fn catalogue_with(state: ResolvedStateResponse) -> ResolutionCatalogue {
        let mut run = entry("execution-run", "run-1", "records/execution-run/run-1");
        run.resolved_state = ResponseField::Present(state);
        let mut spec = entry(
            "task-specification",
            "spec-1",
            "records/task-specification/spec-1",
        );
        spec.revision = ResponseField::Absent;
        spec.content_digest = present(&"a".repeat(64));
        let mut hc = entry(
            "run-human-control",
            "hc-1",
            "records/run-human-control/run-1",
        );
        hc.linked_run_ref = present("records/execution-run/run-1");
        let mut catalogue = ResolutionCatalogue::new();
        catalogue.insert("records/execution-run/run-1", run);
        catalogue.insert("records/task-specification/spec-1", spec);
        catalogue.insert("records/run-human-control/run-1", hc);
        catalogue
    }

    fn messages(
        input: ContextManifestInput,
        catalogue: Option<&ResolutionCatalogue>,
    ) -> (bool, Vec<String>) {
        let (accepted, problems) = check_context_manifest(input, catalogue);
        (
            accepted.is_some(),
            problems.iter().map(|d| d.message().to_string()).collect(),
        )
    }

    #[test]
    fn a_resolved_consistent_manifest_is_accepted() {
        let catalogue = catalogue_with(run_state());
        let (accepted, problems) = check_context_manifest(input(), Some(&catalogue));
        assert!(problems.is_empty(), "{problems:?}");
        let manifest = accepted.unwrap();
        assert_eq!(manifest.checkpoint().work_status, WorkStatus::Active);
        assert_eq!(
            manifest
                .body()
                .pins
                .get(PinnedSlot::HumanControl)
                .id
                .as_str(),
            "hc-1"
        );
    }

    #[test]
    fn without_a_catalogue_the_manifest_fails_closed() {
        let (accepted, m) = messages(input(), None);
        assert!(!accepted);
        assert_eq!(m.len(), 1);
        assert!(m[0].contains("cannot be verified: no external record resolver was supplied"));
    }

    #[test]
    fn an_unresolved_pin_fails_closed_at_both_sites() {
        let mut catalogue = catalogue_with(run_state());
        catalogue = {
            let mut c = ResolutionCatalogue::new();
            for reference in [
                "records/execution-run/run-1",
                "records/task-specification/spec-1",
            ] {
                if let Some(e) = catalogue.resolve(reference) {
                    c.insert(reference, e.clone());
                }
            }
            c
        };
        let (accepted, m) = messages(input(), Some(&catalogue));
        assert!(!accepted);
        assert_eq!(
            m,
            [
                "context manifest \"example-record\" human_control_ref does not resolve to an actual run-human-control through the external resolver; a pinned reference that resolves to nothing is not a verified pin and the manifest fails closed",
                "context manifest \"example-record\" run_state_checkpoint.human_control_ref does not resolve to an actual run-human-control through the external resolver; a pinned reference that resolves to nothing is not a verified pin and the manifest fails closed",
            ]
        );
    }

    #[test]
    fn the_checkpoint_must_reproduce_the_resolved_run_state() {
        let mut state = run_state();
        state.scope_revision = ResponseField::Present(3);
        state.work_status = present("blocked");
        let catalogue = catalogue_with(state);
        let (_, m) = messages(input(), Some(&catalogue));
        assert_eq!(
            m,
            [
                "context manifest \"example-record\" run_state_checkpoint.scope_revision 2 does not match the resolved execution-run record (3); the pinned snapshot reproduces the run's own resolved state, not an unanchored copy",
                "context manifest \"example-record\" run_state_checkpoint.work_status \"active\" does not match the resolved execution-run record (\"blocked\"); the pinned snapshot reproduces the run's own resolved state, not an unanchored copy",
            ]
        );
    }

    /// A malformed resolver response is typed, not a `Value`: its foreign
    /// kind and rendered text still reach the diagnostic.
    #[test]
    fn a_foreign_resolved_state_value_is_named_by_its_json_text() {
        let mut state = run_state();
        state.scope_revision =
            ResponseField::Foreign(ForeignValue::new(ResponseValueKind::String, "\"2\"", "2"));
        state.unknown_fields = vec!["extra".to_string()];
        let catalogue = catalogue_with(state);
        let (_, m) = messages(input(), Some(&catalogue));
        assert_eq!(
            m,
            [
                "context manifest \"example-record\" execution_run_ref: the resolved execution-run record's resolved_state carries an unknown field \"extra\"; resolved_state is closed to its 9 declared axes",
                "context manifest \"example-record\" execution_run_ref: the resolved execution-run record's resolved_state.scope_revision \"2\" is not a positive integer",
                "context manifest \"example-record\" run_state_checkpoint.execution_run_ref: the resolved execution-run record's resolved_state carries an unknown field \"extra\"; resolved_state is closed to its 9 declared axes",
                "context manifest \"example-record\" run_state_checkpoint.execution_run_ref: the resolved execution-run record's resolved_state.scope_revision \"2\" is not a positive integer",
                "context manifest \"example-record\" run_state_checkpoint.scope_revision 2 does not match the resolved execution-run record (\"2\"); the pinned snapshot reproduces the run's own resolved state, not an unanchored copy",
            ]
        );
    }

    #[test]
    fn a_mutable_source_needs_a_pin_and_the_scope_must_name_the_pinned_run() {
        let mut input = input();
        input.body.authoritative_sources[0].mutable = true;
        input.body.authoritative_sources[0].revision = Some(text("main"));
        input.scope = RunStateScope::new(
            RunId::new("run-10").unwrap(),
            WorkspaceId::new("example-workspace").unwrap(),
        );
        let catalogue = catalogue_with(run_state());
        let (_, m) = messages(input, Some(&catalogue));
        assert_eq!(
            m,
            [
                "context manifest \"example-record\" scope identifies run \"run-10\" but execution_run_ref.id is \"run-1\"; the two must be the exact same string, not one a substring of the other",
                "context manifest \"example-record\" authoritative source owner-decision is mutable but not pinned: revision \"main\" is a branch or channel reference; a branch or channel is never a pin, whatever digits it carries",
            ]
        );
    }
}
