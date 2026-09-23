//! Section 20 of `evaluateEvidenceAndHandoff`: THE EXTERNAL RESOLUTION
//! BOUNDARY. The four pinned references and every evidence entry are
//! resolved through the typed [`ResolutionCatalogue`], and the handoff's own
//! axes are checked against the state the RESOLVED execution-run record
//! carries.

use std::collections::HashSet;

use crate::ordered::{OrderedMap, OrderedSet};
use crate::run_contracts::envelope::RecordFamily;
use crate::run_contracts::resolution::{
    check_resolved_entry, response_text, response_text_set, same_step, step_json,
};
use crate::run_contracts::{
    RecordText, ResolutionCatalogue, ResolvedEntry, ResolvedStateResponse, ResponseField,
    WorkStatus,
};
use crate::task_contracts::non_portable_reason;
use crate::types::{Diagnostic, SemanticId};

use super::input::{CoverageStatus, HandoffEvidenceInput, HandoffInput, HandoffSlot};
use super::structure::Structure;
use crate::evidence::linkage::{supports, AssertionStatus, ResolvedEvidenceResult};
use crate::evidence::resolve::resolve_evidence_result;
use crate::evidence::types::{EvidenceKind, ObservedResult};
use crate::evidence::{fail, record_subject};

const FAMILY: RecordFamily = RecordFamily::EvidenceAndHandoff;

/// One evidence entry's resolution (`resolveEvidenceEntry`'s result).
struct EvidenceOutcome<'c> {
    entry: Option<&'c ResolvedEntry>,
    clean: bool,
    observed: Option<ObservedResult>,
}

impl EvidenceOutcome<'_> {
    /// The clean resolution as the accepted handoff records it.
    fn accepted(&self) -> Option<ResolvedEvidenceResult> {
        let observed_result = self.observed.filter(|_| self.clean)?;
        let covers = match self.entry.map(|e| &e.evidence_result.covers) {
            Some(ResponseField::Present(items)) => items
                .iter()
                .filter_map(|item| item.text().and_then(|t| SemanticId::new(t).ok()))
                .collect(),
            _ => Vec::new(),
        };
        Some(ResolvedEvidenceResult {
            observed_result,
            covers,
        })
    }
}

/// `JSON.stringify` of an optional record text (`x ?? null`).
fn text_json(value: Option<&RecordText>) -> String {
    value.map_or_else(
        || "null".to_string(),
        |v| crate::run_contracts::json_quote(v.as_str()),
    )
}

/// Section 20. Returns the clean resolution of every evidence entry, in
/// declaration order, for the accepted handoff.
pub(super) fn check_resolution(
    input: &HandoffInput,
    structure: &Structure<'_>,
    catalogue: &ResolutionCatalogue,
    problems: &mut Vec<Diagnostic>,
) -> Vec<Option<ResolvedEvidenceResult>> {
    let id = input.envelope.id.as_str();
    let subject = record_subject(FAMILY, id);
    let body = &input.body;
    let pins = &body.pins;

    let [run_entry, spec_entry, hc_entry, cm_entry] = HandoffSlot::ALL.map(|slot| {
        check_resolved_entry(
            FAMILY,
            id,
            slot.field(),
            slot.kind(),
            pins.get(slot),
            catalogue,
            problems,
        )
    });

    let run_state: Option<&ResolvedStateResponse> =
        run_entry.and_then(|e| e.resolved_state.value());
    let resolved_criteria: Option<OrderedSet<&str>> = spec_entry
        .and_then(|e| e.specification.acceptance_criteria.value())
        .filter(|items| !items.is_empty())
        .map(|items| items.iter().filter_map(|i| i.text()).collect());
    let resolved_checks: Option<OrderedSet<&str>> = spec_entry
        .and_then(|e| e.specification.mandatory_checks.value())
        .map(|items| {
            items
                .iter()
                .filter_map(|i| i.text().map(str::trim))
                .collect()
        });

    // 20a. resolve every evidence entry.
    let mut resolutions: OrderedMap<&SemanticId, usize> = OrderedMap::new();
    let mut outcomes = Vec::with_capacity(body.evidence.len());
    let mut supported: HashSet<&SemanticId> = HashSet::new();
    for (index, e) in body.evidence.iter().enumerate() {
        let outcome = resolve_handoff_evidence(id, &subject, e, catalogue, problems);
        resolutions.insert(&e.evidence.id, index);
        if let Some(resolved) = outcome.accepted() {
            for cv in &e.covers {
                if structure.assertion_ids.contains(cv) && supports(&e.covers, &resolved, cv) {
                    supported.insert(cv);
                }
            }
        }
        outcomes.push(outcome);
    }

    // 20b. a verified assertion needs supporting resolved evidence.
    for (aid, assertion) in structure.assertions.iter() {
        if assertion.status == AssertionStatus::Verified && !supported.contains(aid) {
            problems.push(fail(format!(
                "{subject} verifiable assertion \"{aid}\" is \"verified\" but no resolved, pin-checked evidence entry confirms it (a covering evidence entry that resolves and whose observed_result is \"confirmed\"); a plain reference is not proof"
            )));
        }
    }

    // 20c. each passed/failed mandatory check's result evidence.
    for cre in &structure.check_expectations {
        let Some(res) = resolutions
            .get(&cre.evidence_id)
            .and_then(|&i| outcomes.get(i))
        else {
            continue;
        };
        let at = cre.at.as_str();
        let status = cre.status.as_str();
        let evidence_id = cre.evidence_id.as_str();
        if !res.clean {
            problems.push(fail(format!(
                "{subject} mandatory check {at} is \"{status}\" but its result evidence \"{evidence_id}\" did not resolve cleanly through the external boundary"
            )));
        } else if res.observed != Some(cre.expect) {
            problems.push(fail(format!(
                "{subject} mandatory check {at} is \"{status}\" but its result evidence \"{evidence_id}\" resolves with observed_result {}, not \"{}\"",
                res.observed.map_or_else(|| "null".to_string(), |o| format!("\"{o}\"")),
                cre.expect
            )));
        }
        if let Some(entry) = res.entry {
            match entry
                .evidence_result
                .check_ref
                .text()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                None => problems.push(fail(format!(
                    "{subject} mandatory check {at} names result evidence \"{evidence_id}\", but the resolved evidence-result records no check_ref; the transformer confirms which check a recorded result is the result of, and summary + covers from the handoff alone are not enough"
                ))),
                Some(confirmed) if confirmed != cre.check_ref => problems.push(fail(format!(
                    "{subject} mandatory check {at} names result evidence \"{evidence_id}\", but the resolved evidence-result records check_ref \"{confirmed}\", not this check's check_ref \"{}\"; a check's result is not another check's result",
                    cre.check_ref
                ))),
                Some(_) => {}
            }
        }
    }

    // 20d. acceptance-criteria coverage closed against the resolved spec.
    if let Some(resolved) = &resolved_criteria {
        let handoff: Vec<&str> = structure.coverage.keys().map(|c| c.as_str()).collect();
        for c in &handoff {
            if !resolved.contains(c) {
                problems.push(fail(format!(
                    "{subject} acceptance criterion \"{c}\" is not among the resolved task-specification record's acceptance criteria"
                )));
            }
        }
        for c in resolved.iter() {
            if !handoff.contains(c) {
                problems.push(fail(format!(
                    "{subject} does not address acceptance criterion \"{c}\" declared by the resolved task-specification record; every criterion is covered or explicitly uncovered"
                )));
            }
        }
        if !resolved.is_empty() && body.mandatory_checks.is_empty() {
            problems.push(fail(format!(
                "{subject} lists no mandatory_checks, but the resolved task-specification record declares {} acceptance criteria; a self-selected empty check list does not discharge the specification's mandatory checks",
                resolved.len()
            )));
        }
    }

    // 20d'. the FULL set of mandatory_checks closed against the spec.
    if let Some(resolved) = &resolved_checks {
        let handoff: OrderedSet<&str> = body
            .mandatory_checks
            .iter()
            .map(|m| m.check_ref.as_str().trim())
            .filter(|r| !r.is_empty())
            .collect();
        for c in handoff.iter() {
            if !resolved.contains(c) {
                problems.push(fail(format!(
                    "{subject} mandatory check \"{c}\" is not among the resolved task-specification record's mandatory checks; the full set of mandatory checks is closed against the specification, and \"list is non-empty\" is not enough"
                )));
            }
        }
        for c in resolved.iter() {
            if !handoff.contains(c) {
                problems.push(fail(format!(
                    "{subject} omits mandatory check \"{c}\" declared by the resolved task-specification record; removing a mandatory check from the handoff is rejected"
                )));
            }
        }
    }

    let step = &body.next_step;
    let outcome = body.outcome.status;
    // 20e. outcome.status "complete" is the WHOLE run finished.
    if outcome == super::input::OutcomeStatus::Complete {
        if let Some(state) = run_state {
            if state.work_status.text() != Some(WorkStatus::Completed.as_str()) {
                problems.push(fail(format!(
                    "{subject} outcome.status is \"complete\" but the resolved execution-run record's work_status is \"{}\", not \"completed\"; a complete outcome is the whole run finished",
                    state.work_status.string_or_undefined()
                )));
            }
        }
        if step.action.is_some() || step.gate.is_some() {
            problems.push(fail(format!(
                "{subject} outcome.status is \"complete\" but next_step still carries an executable {}; a completed run has no next step",
                if step.action.is_some() { "action" } else { "gate" }
            )));
        }
        for (cid, criterion) in structure.coverage.iter() {
            if criterion.status != CoverageStatus::Covered {
                problems.push(fail(format!(
                    "{subject} outcome.status is \"complete\" but acceptance criterion \"{cid}\" is not covered"
                )));
                continue;
            }
            let unverified: Vec<&str> = criterion
                .addressed_by
                .iter()
                .filter(|aid| {
                    structure
                        .assertions
                        .get(aid)
                        .is_none_or(|a| a.status != AssertionStatus::Verified)
                })
                .map(SemanticId::as_str)
                .collect();
            if !unverified.is_empty() {
                problems.push(fail(format!(
                    "{subject} outcome.status is \"complete\" but acceptance criterion \"{cid}\" is addressed only through unverified assertion(s) {}",
                    unverified.join(", ")
                )));
            }
        }
    }

    if let Some(state) = run_state {
        let pinned_spec = pins.task_specification.reference.as_str();
        if let Some(run_spec) = response_text(&state.task_specification_ref) {
            if run_spec != pinned_spec {
                problems.push(fail(format!(
                    "{subject} task_specification_ref names \"{pinned_spec}\", but the resolved execution-run record names task specification \"{run_spec}\"; the handoff carries the SAME specification the run resolved, not an independent one"
                )));
            }
        }
        check_against_run_state(&subject, input, structure, state, problems);
    }

    let pinned_run = pins.execution_run.reference.as_str();
    for (slot, entry) in [
        (HandoffSlot::HumanControl, hc_entry),
        (HandoffSlot::ContextManifest, cm_entry),
    ] {
        let Some(linked) = entry.and_then(|e| response_text(&e.linked_run_ref)) else {
            continue;
        };
        if linked != pinned_run {
            problems.push(fail(format!(
                "{subject} {} resolves to a {} record whose linked_run_ref is \"{linked}\", not the handoff's pinned run \"{pinned_run}\"",
                slot.field(),
                slot.kind().as_str()
            )));
        }
    }

    outcomes.iter().map(EvidenceOutcome::accepted).collect()
}

/// The handoff's next step, blockers and checks against the resolved run.
fn check_against_run_state(
    subject: &str,
    input: &HandoffInput,
    structure: &Structure<'_>,
    state: &ResolvedStateResponse,
    problems: &mut Vec<Diagnostic>,
) {
    let step = &input.body.next_step;
    for (name, have, want) in [
        ("action", step.action.as_ref(), &state.next_action),
        ("gate", step.gate.as_ref(), &state.next_gate),
    ] {
        if want.is_present() && !same_step(have, want) {
            problems.push(fail(format!(
                "{subject} next_step.{name} {} does not match the resolved execution-run record's next_{name} {}",
                text_json(have),
                step_json(want)
            )));
        }
    }
    if let ResponseField::Present(items) = &state.blocker_ids {
        let run: HashSet<&str> = response_text_set(items);
        let own: HashSet<&str> = structure
            .blocker_ids
            .iter()
            .map(|b| b.as_str().trim())
            .collect();
        if run != own {
            problems.push(fail(format!(
                "{subject} blockers do not match the resolved execution-run record's blocker_ids; the two sets of blocker ids must be the same"
            )));
        }
    }
    if let ResponseField::Present(items) = &state.completed_checks {
        let completed = response_text_set(items);
        for r in structure.executed_check_refs.iter() {
            if !completed.contains(r) {
                problems.push(fail(format!(
                    "{subject} mandatory check \"{r}\" ran (its status is \"passed\" or \"failed\") but its check_ref is not among the resolved execution-run record's completed_checks; a check that ran — passed OR failed — is a completed check of the run, and completed_checks confirms the fact it ran, not the verdict"
                )));
            }
        }
        for r in structure.not_executed_check_refs.iter() {
            if completed.contains(r) {
                problems.push(fail(format!(
                    "{subject} mandatory check \"{r}\" is \"unable\" but appears among the resolved execution-run record's completed_checks; a check that could not run is not a completed check"
                )));
            }
        }
    }
    let work_status = state.work_status.text().and_then(WorkStatus::parse);
    if let Some(ws) = work_status.filter(|ws| ws.is_terminal()) {
        for (name, value) in [("action", &step.action), ("gate", &step.gate)] {
            if value.is_some() {
                problems.push(fail(format!(
                    "{subject} the resolved execution-run record's work_status is \"{}\" but next_step.{name} carries an executable value; a completed or cancelled run does not carry a next executable step",
                    ws.as_str()
                )));
            }
        }
    }
    let outcome = input.body.outcome.status;
    if work_status == Some(WorkStatus::Blocked) && outcome != super::input::OutcomeStatus::Blocked {
        problems.push(fail(format!(
            "{subject} the resolved execution-run record's work_status is \"blocked\" but outcome.status is \"{}\"",
            outcome.as_str()
        )));
    }
}

/// `resolveEvidenceEntry` of the handoff: the shared evidence-result
/// resolution, then the handoff's subject binding — covers, check_ref and
/// the specialised verdict.
fn resolve_handoff_evidence<'c>(
    id: &str,
    subject: &str,
    e: &HandoffEvidenceInput,
    catalogue: &'c ResolutionCatalogue,
    problems: &mut Vec<Diagnostic>,
) -> EvidenceOutcome<'c> {
    let before = problems.len();
    let at = e.evidence.id.as_str();
    let resolution = resolve_evidence_result(FAMILY, id, &e.evidence, catalogue, problems);
    if let Some(entry) = resolution.entry {
        let result = &entry.evidence_result;
        let prefix = format!("{subject} evidence entry {at}: the resolved");
        let mut confirmed: Option<HashSet<&str>> = None;
        match &result.covers {
            ResponseField::Absent => {}
            ResponseField::Foreign(f) => problems.push(fail(format!(
                "{prefix} evidence-result's covers is {}, not an array of assertion ids",
                f.kind().null_or_type_of()
            ))),
            ResponseField::Present(items) => {
                let mut set = HashSet::new();
                for (k, item) in items.iter().enumerate() {
                    match item.text().filter(|t| SemanticId::new(*t).is_ok()) {
                        None => problems.push(fail(format!(
                            "{prefix} evidence-result's covers[{k}] is not a stable semantic id"
                        ))),
                        Some(cv) if !set.insert(cv) => problems.push(fail(format!(
                            "{prefix} evidence-result's covers repeats \"{cv}\""
                        ))),
                        Some(_) => {}
                    }
                }
                confirmed = Some(set);
            }
        }
        if result.check_ref.is_present() {
            match result.check_ref.non_blank() {
                None => problems.push(fail(format!(
                    "{prefix} evidence-result's check_ref is present but not a non-empty reference"
                ))),
                Some(r) => {
                    if let Some(reason) = non_portable_reason(Some(r)) {
                        problems.push(fail(format!(
                            "{prefix} evidence-result's check_ref contains {reason}"
                        )));
                    }
                }
            }
        }
        if !e.covers.is_empty() {
            match &confirmed {
                None => problems.push(fail(format!(
                    "{prefix} evidence-result confirms no covered subject; which assertion(s) an evidence entry bears on is confirmed by the transformer, not asserted by the handoff, and summary + covers from the handoff alone are not enough"
                ))),
                Some(set) => {
                    for cv in &e.covers {
                        if !set.contains(cv.as_str()) {
                            problems.push(fail(format!(
                                "{prefix} evidence-result does not confirm that this evidence bears on assertion \"{cv}\"; a resolved evidence entry's covers is not transferred to a foreign assertion"
                            )));
                        }
                    }
                }
            }
        }
        let specialised = [
            (
                "specialised_contract",
                &result.specialised_contract,
                e.specialised_contract.as_ref(),
            ),
            (
                "recorded_verdict",
                &result.recorded_verdict,
                e.recorded_verdict.as_ref(),
            ),
        ];
        if e.evidence.kind == EvidenceKind::SpecialisedEvidenceRecord {
            for (f, resolved, stated) in specialised {
                match resolved.non_blank() {
                    None => problems.push(fail(format!(
                        "{prefix} specialised evidence record confirms no {f}; a specialised verdict is confirmed by the resolved record, not by the handoff's own words"
                    ))),
                    Some(r) if Some(r) != stated.map(RecordText::as_str) => {
                        problems.push(fail(format!(
                            "{prefix} specialised evidence record's {f} \"{r}\" does not match the handoff's stated {f} {}",
                            text_json(stated)
                        )))
                    }
                    Some(_) => {}
                }
            }
        } else {
            for (f, resolved, _) in specialised {
                if resolved.is_present() {
                    problems.push(fail(format!(
                        "{prefix} evidence-result carries \"{f}\" but the evidence entry kind is \"{}\", not \"specialised-evidence-record\"",
                        e.evidence.kind
                    )));
                }
            }
        }
    }
    EvidenceOutcome {
        entry: resolution.entry,
        clean: problems.len() == before,
        observed: resolution.observed,
    }
}
