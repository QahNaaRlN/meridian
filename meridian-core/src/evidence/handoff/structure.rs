//! Sections 7–19 of `evaluateEvidenceAndHandoff`: every rule that reads
//! only the record itself. The facts the external resolution boundary
//! (section 20, [`super::resolved`]) needs are returned as [`Structure`],
//! indexed in declaration order.

use std::collections::HashSet;

use crate::ordered::{OrderedMap, OrderedSet};
use crate::run_contracts::envelope::RecordFamily;
use crate::run_contracts::pinned_ref::{check_pin_shape, check_run_id, RunIdRule};
use crate::run_contracts::revision::pin_defect;
use crate::run_contracts::RecordText;
use crate::task_contracts::non_portable_reason;
use crate::types::{Diagnostic, SemanticId};

use super::input::{
    AssertionInput, CheckStatus, CoverageStatus, CriterionInput, DeviationSeverity, HandoffBody,
    HandoffInput, HandoffSlot, OutcomeStatus, RepositoryStateInput, WorktreeState,
};
use crate::evidence::linkage::{claimed_result_status, AssertionStatus, ClaimedResultStatus};
use crate::evidence::resolve::check_pinned_evidence;
use crate::evidence::types::{EvidenceKind, ObservedResult};
use crate::evidence::{fail, record_subject};

const FAMILY: RecordFamily = RecordFamily::EvidenceAndHandoff;

/// A passed or failed check's expected result evidence.
pub(super) struct CheckExpectation<'a> {
    pub at: &'a SemanticId,
    pub evidence_id: &'a SemanticId,
    pub expect: ObservedResult,
    pub status: CheckStatus,
    /// The check's own `check_ref`, trimmed.
    pub check_ref: &'a str,
}

/// The record's cross-reference indexes, in declaration order.
pub(super) struct Structure<'a> {
    pub assertion_ids: HashSet<&'a SemanticId>,
    pub assertions: OrderedMap<&'a SemanticId, &'a AssertionInput>,
    pub check_expectations: Vec<CheckExpectation<'a>>,
    pub executed_check_refs: OrderedSet<&'a str>,
    pub not_executed_check_refs: OrderedSet<&'a str>,
    pub coverage: OrderedMap<&'a SemanticId, &'a CriterionInput>,
    pub blocker_ids: OrderedSet<&'a SemanticId>,
    /// The computed status of every claimed result, in declaration order.
    pub claimed_statuses: Vec<ClaimedResultStatus>,
}

/// `blank(x)` of the Node reference for an optional text.
fn blank(value: Option<&RecordText>) -> bool {
    value.is_none_or(RecordText::is_blank)
}

/// "has no {name}" when blank, else "{name} contains …" when not portable.
fn required_text(
    subject: &str,
    at: &str,
    name: &str,
    value: Option<&RecordText>,
    missing: impl FnOnce() -> String,
    problems: &mut Vec<Diagnostic>,
) {
    match value.filter(|v| !v.is_blank()) {
        None => problems.push(fail(missing())),
        Some(v) => {
            if let Some(r) = non_portable_reason(Some(v.as_str())) {
                problems.push(fail(format!("{subject} {at} {name} contains {r}")));
            }
        }
    }
}

fn run_id_rule(slot: HandoffSlot) -> RunIdRule {
    match slot {
        HandoffSlot::ExecutionRun => RunIdRule::SelfRun,
        HandoffSlot::TaskSpecification => RunIdRule::Forbidden,
        HandoffSlot::HumanControl | HandoffSlot::ContextManifest => RunIdRule::Required,
    }
}

/// Sections 7–19, in the Node reference's order.
pub(super) fn check_structure<'a>(
    input: &'a HandoffInput,
    problems: &mut Vec<Diagnostic>,
) -> Structure<'a> {
    let id = input.envelope.id.as_str();
    let subject = record_subject(FAMILY, id);
    let body = &input.body;

    check_pins(id, &subject, input, problems);
    let claimed_ids = check_claimed_results(&subject, body, problems);
    let (assertion_ids, assertions, by_result) =
        check_assertions(&subject, body, &claimed_ids, problems);
    let evidence_ids = check_evidence(id, &subject, body, &assertion_ids, problems);
    let claimed_statuses =
        check_established_inference(&subject, body, &assertions, &by_result, problems);
    let checks = check_mandatory_checks(&subject, body, &evidence_ids, problems);
    let coverage = check_acceptance_criteria(&subject, body, &assertion_ids, problems);
    check_repository_states_and_paths(&subject, body, problems);
    let flags = check_effects_deviations_gaps_decisions(&subject, body, problems);
    let blocker_ids = check_blockers(&subject, body, problems);
    check_worktree(&subject, body, problems);
    check_next_step(&subject, body, problems);
    check_outcome(&subject, body, &checks, &flags, &blocker_ids, problems);

    Structure {
        assertion_ids,
        assertions,
        check_expectations: checks.expectations,
        executed_check_refs: checks.executed,
        not_executed_check_refs: checks.not_executed,
        coverage,
        blocker_ids,
        claimed_statuses,
    }
}

/// Section 7: the four structured pinned references and the DETERMINISTIC
/// single-run link.
fn check_pins(id: &str, subject: &str, input: &HandoffInput, problems: &mut Vec<Diagnostic>) {
    let pins = &input.body.pins;
    let run_id = &pins.execution_run.id;
    for slot in HandoffSlot::ALL {
        let pin = pins.get(slot);
        let field = slot.field();
        check_pin_shape(FAMILY, id, field, slot.kind(), pin, problems);
        check_run_id(
            FAMILY,
            id,
            field,
            run_id_rule(slot),
            slot.kind(),
            pin,
            run_id,
            problems,
        );
    }
    let scope_run = input.scope.run_id().as_str();
    if scope_run != run_id.as_str() {
        problems.push(fail(format!(
            "{subject} scope identifies run \"{scope_run}\" but execution_run_ref.id is \"{}\"; the two must be the exact same string, not one a substring of the other",
            run_id.as_str()
        )));
    }
}

/// Section 8: claimed results.
fn check_claimed_results<'a>(
    subject: &str,
    body: &'a HandoffBody,
    problems: &mut Vec<Diagnostic>,
) -> HashSet<&'a SemanticId> {
    let mut claimed = HashSet::new();
    for c in &body.claimed_results {
        let at = c.id.as_str();
        if !claimed.insert(&c.id) {
            problems.push(fail(format!(
                "{subject} claimed result id \"{at}\" is used more than once"
            )));
        }
        required_text(
            subject,
            &format!("claimed result {at}"),
            "statement",
            Some(&c.statement),
            || format!("{subject} claimed result {at} has no statement"),
            problems,
        );
    }
    claimed
}

type AssertionIndex<'a> = (
    HashSet<&'a SemanticId>,
    OrderedMap<&'a SemanticId, &'a AssertionInput>,
    OrderedMap<&'a SemanticId, Vec<&'a SemanticId>>,
);

/// Section 9: verifiable assertions — each names its one claimed result.
fn check_assertions<'a>(
    subject: &str,
    body: &'a HandoffBody,
    claimed: &HashSet<&'a SemanticId>,
    problems: &mut Vec<Diagnostic>,
) -> AssertionIndex<'a> {
    let mut ids = HashSet::new();
    let mut by_id = OrderedMap::new();
    let mut by_result: OrderedMap<&SemanticId, Vec<&SemanticId>> = OrderedMap::new();
    for a in &body.assertions {
        let at = a.id.as_str();
        if !ids.insert(&a.id) {
            problems.push(fail(format!(
                "{subject} verifiable assertion id \"{at}\" is used more than once"
            )));
        }
        by_id.insert(&a.id, a);
        required_text(
            subject,
            &format!("verifiable assertion {at}"),
            "statement",
            Some(&a.statement),
            || format!("{subject} verifiable assertion {at} has no statement"),
            problems,
        );
        if claimed.contains(&a.claimed_result_id) {
            let mut linked = by_result
                .get(&&a.claimed_result_id)
                .cloned()
                .unwrap_or_default();
            linked.push(&a.id);
            by_result.insert(&a.claimed_result_id, linked);
        } else {
            problems.push(fail(format!(
                "{subject} verifiable assertion {at} names claimed_result_id \"{}\", which is not a declared claimed result",
                a.claimed_result_id.as_str()
            )));
        }
        match a.status {
            AssertionStatus::Unverified => required_text(
                subject,
                &format!("verifiable assertion {at}"),
                "unverified_reason",
                a.unverified_reason.as_ref(),
                || {
                    format!("{subject} verifiable assertion {at} is unverified but states no unverified_reason; the unproven stays explicitly unproven")
                },
                problems,
            ),
            AssertionStatus::Verified if a.unverified_reason.is_some() => {
                problems.push(fail(format!(
                "{subject} verifiable assertion {at} is verified but carries an unverified_reason"
            )))
            }
            AssertionStatus::Verified => {}
        }
    }
    (ids, by_id, by_result)
}

/// Section 10: evidence — pinned, and (in section 20) resolved through the external
/// boundary; the structural covers link is checked here.
fn check_evidence<'a>(
    id: &str,
    subject: &str,
    body: &'a HandoffBody,
    assertion_ids: &HashSet<&'a SemanticId>,
    problems: &mut Vec<Diagnostic>,
) -> HashSet<&'a SemanticId> {
    let mut seen = HashSet::new();
    for e in &body.evidence {
        check_pinned_evidence(FAMILY, id, &e.evidence, &mut seen, problems);
        let at = e.evidence.id.as_str();
        let mut seen_cover = HashSet::new();
        for cv in &e.covers {
            if !seen_cover.insert(cv) {
                problems.push(fail(format!(
                    "{subject} evidence entry {at} covers \"{cv}\" more than once"
                )));
            }
            if !assertion_ids.contains(cv) {
                problems.push(fail(format!(
                    "{subject} evidence entry {at} covers \"{cv}\", which is not a declared verifiable assertion; a narrow evidence entry is not widened to an undeclared assertion"
                )));
            }
        }
        for (j, lm) in e.limitations.iter().enumerate() {
            if lm.is_blank() {
                problems.push(fail(format!(
                    "{subject} evidence entry {at} limitations[{j}] is empty or whitespace-only"
                )));
            } else if let Some(r) = non_portable_reason(Some(lm.as_str())) {
                problems.push(fail(format!(
                    "{subject} evidence entry {at} limitations[{j}] contains {r}"
                )));
            }
        }
        let specialised = [
            ("specialised_contract", e.specialised_contract.as_ref()),
            ("recorded_verdict", e.recorded_verdict.as_ref()),
        ];
        if e.evidence.kind == EvidenceKind::SpecialisedEvidenceRecord {
            for (f, value) in specialised {
                required_text(
                    subject,
                    &format!("evidence entry {at}"),
                    f,
                    value,
                    || {
                        format!("{subject} evidence entry {at} is a specialised-evidence-record but states no {f}; the specialised contract keeps authority over its own verdict, which this handoff records rather than re-derives")
                    },
                    problems,
                );
            }
        } else {
            for (f, value) in specialised {
                if value.is_some() {
                    problems.push(fail(format!(
                        "{subject} evidence entry {at} carries \"{f}\" but its kind is \"{}\", not \"specialised-evidence-record\"",
                        e.evidence.kind
                    )));
                }
            }
        }
    }
    seen
}

/// Section 11: the established / not_established inference, checked on BOTH sides
/// against the declared statuses of the linked assertions. Returns the
/// computed status of every claimed result.
fn check_established_inference(
    subject: &str,
    body: &HandoffBody,
    assertions: &OrderedMap<&SemanticId, &AssertionInput>,
    by_result: &OrderedMap<&SemanticId, Vec<&SemanticId>>,
    problems: &mut Vec<Diagnostic>,
) -> Vec<ClaimedResultStatus> {
    let mut statuses = Vec::with_capacity(body.claimed_results.len());
    for c in &body.claimed_results {
        let cid = c.id.as_str();
        let linked: &[&SemanticId] = by_result.get(&&c.id).map_or(&[], Vec::as_slice);
        let declared: Vec<AssertionStatus> = linked
            .iter()
            .map(|aid| {
                assertions
                    .get(aid)
                    .map_or(AssertionStatus::Unverified, |a| a.status)
            })
            .collect();
        let unverified: Vec<&str> = linked
            .iter()
            .zip(&declared)
            .filter(|(_, s)| **s != AssertionStatus::Verified)
            .map(|(aid, _)| aid.as_str())
            .collect();
        let computed = claimed_result_status(&declared);
        match c.status {
            ClaimedResultStatus::Established if linked.is_empty() => problems.push(fail(format!(
                "{subject} claimed result \"{cid}\" is established but no verifiable assertion links to it; an established result carries at least one verified assertion"
            ))),
            ClaimedResultStatus::Established if !unverified.is_empty() => problems.push(fail(format!(
                "{subject} claimed result \"{cid}\" is established but assertion(s) {} linked to it are not verified; the unproven cannot be established",
                unverified.join(", ")
            ))),
            ClaimedResultStatus::NotEstablished
                if computed == ClaimedResultStatus::Established =>
            {
                problems.push(fail(format!(
                    "{subject} claimed result \"{cid}\" is \"not_established\" but it has at least one linked verifiable assertion and every one of them is verified; the status is derived from the evidence, not asserted — this result is established"
                )))
            }
            _ => {}
        }
        statuses.push(computed);
    }
    statuses
}

/// The mandatory-check facts later sections read.
struct Checks<'a> {
    expectations: Vec<CheckExpectation<'a>>,
    executed: OrderedSet<&'a str>,
    not_executed: OrderedSet<&'a str>,
    failed_or_unable: bool,
    every_passed: bool,
}

/// Section 12: mandatory checks — three distinct statuses, each backed by evidence
/// of its result.
fn check_mandatory_checks<'a>(
    subject: &str,
    body: &'a HandoffBody,
    evidence_ids: &HashSet<&'a SemanticId>,
    problems: &mut Vec<Diagnostic>,
) -> Checks<'a> {
    let mut checks = Checks {
        expectations: Vec::new(),
        executed: OrderedSet::new(),
        not_executed: OrderedSet::new(),
        failed_or_unable: false,
        every_passed: body
            .mandatory_checks
            .iter()
            .all(|m| m.status == CheckStatus::Passed),
    };
    let mut seen_id = HashSet::new();
    let mut seen_ref = HashSet::new();
    for m in &body.mandatory_checks {
        let at = m.id.as_str();
        if !seen_id.insert(&m.id) {
            problems.push(fail(format!(
                "{subject} mandatory check id \"{at}\" is used more than once"
            )));
        }
        required_text(
            subject,
            &format!("mandatory check {at}"),
            "name",
            Some(&m.name),
            || format!("{subject} mandatory check {at} has no name"),
            problems,
        );
        let raw_ref = m.check_ref.as_str();
        let check_ref = raw_ref.trim();
        if m.check_ref.is_blank() {
            problems.push(fail(format!(
                "{subject} mandatory check {at} has no check_ref"
            )));
        } else {
            if let Some(r) = non_portable_reason(Some(raw_ref)) {
                problems.push(fail(format!(
                    "{subject} mandatory check {at} check_ref contains {r}"
                )));
            }
            if !seen_ref.insert(check_ref) {
                problems.push(fail(format!(
                    "{subject} mandatory check check_ref \"{raw_ref}\" is listed more than once"
                )));
            }
        }
        let status = m.status.as_str();
        if m.status.ran() {
            let (expect, verb) = match m.status {
                CheckStatus::Passed => (ObservedResult::Confirmed, "successful"),
                _ => (ObservedResult::Contradicted, "failing"),
            };
            match &m.result_evidence_id {
                None => problems.push(fail(format!(
                    "{subject} mandatory check {at} is \"{status}\" but names no result_evidence_id; a {status} status is backed by an evidence entry that shows the {verb} result, not by the completed_checks list"
                ))),
                Some(rev) if !evidence_ids.contains(rev) => problems.push(fail(format!(
                    "{subject} mandatory check {at} result_evidence_id \"{rev}\" is not a declared evidence entry"
                ))),
                Some(rev) => checks.expectations.push(CheckExpectation {
                    at: &m.id,
                    evidence_id: rev,
                    expect,
                    status: m.status,
                    check_ref,
                }),
            }
        }
        match m.status {
            CheckStatus::Passed => {
                if m.reason.is_some() {
                    problems.push(fail(format!(
                        "{subject} mandatory check {at} passed but carries a reason; a reason is stated for a failed or unable check"
                    )));
                }
            }
            CheckStatus::Failed => {
                checks.failed_or_unable = true;
                required_text(
                    subject,
                    &format!("mandatory check {at}"),
                    "reason",
                    m.reason.as_ref(),
                    || format!("{subject} mandatory check {at} is \"failed\" but states no reason"),
                    problems,
                );
            }
            CheckStatus::Unable => {
                checks.failed_or_unable = true;
                required_text(
                    subject,
                    &format!("mandatory check {at}"),
                    "reason",
                    m.reason.as_ref(),
                    || {
                        format!("{subject} mandatory check {at} is \"unable\" but states no verifiable reason; an unable check is not a pass and does not become one silently")
                    },
                    problems,
                );
                if m.result_evidence_id.is_some() {
                    problems.push(fail(format!(
                        "{subject} mandatory check {at} is \"unable\" but names a result_evidence_id; a check that could not run has no result to evidence"
                    )));
                }
            }
        }
        if !check_ref.is_empty() {
            if m.status.ran() {
                checks.executed.insert(check_ref);
            } else {
                checks.not_executed.insert(check_ref);
            }
        }
    }
    checks
}

/// Section 12b: acceptance-criteria coverage.
fn check_acceptance_criteria<'a>(
    subject: &str,
    body: &'a HandoffBody,
    assertion_ids: &HashSet<&'a SemanticId>,
    problems: &mut Vec<Diagnostic>,
) -> OrderedMap<&'a SemanticId, &'a CriterionInput> {
    let mut coverage = OrderedMap::new();
    let mut seen_id = HashSet::new();
    for c in &body.acceptance_criteria {
        let at = c.id.as_str();
        if !seen_id.insert(&c.id) {
            problems.push(fail(format!(
                "{subject} acceptance criterion id \"{at}\" is used more than once"
            )));
        }
        let mut seen_a = HashSet::new();
        for aid in &c.addressed_by {
            if !seen_a.insert(aid) {
                problems.push(fail(format!(
                    "{subject} acceptance criterion {at} addresses \"{aid}\" more than once"
                )));
            }
            if !assertion_ids.contains(aid) {
                problems.push(fail(format!(
                    "{subject} acceptance criterion {at} is addressed_by \"{aid}\", which is not a declared verifiable assertion"
                )));
            }
        }
        match c.status {
            CoverageStatus::Covered => {
                if c.addressed_by.is_empty() {
                    problems.push(fail(format!(
                        "{subject} acceptance criterion {at} is \"covered\" but names no addressed_by verifiable assertion"
                    )));
                }
                if c.uncovered_reason.is_some() {
                    problems.push(fail(format!(
                        "{subject} acceptance criterion {at} is \"covered\" but carries an uncovered_reason"
                    )));
                }
            }
            CoverageStatus::Uncovered => {
                if !c.addressed_by.is_empty() {
                    problems.push(fail(format!(
                        "{subject} acceptance criterion {at} is \"uncovered\" but names addressed_by assertions"
                    )));
                }
                required_text(
                    subject,
                    &format!("acceptance criterion {at}"),
                    "uncovered_reason",
                    c.uncovered_reason.as_ref(),
                    || {
                        format!("{subject} acceptance criterion {at} is \"uncovered\" but states no uncovered_reason")
                    },
                    problems,
                );
            }
        }
        coverage.insert(&c.id, c);
    }
    coverage
}

/// One repository-state list (`checkRepositoryStates`): the first entry of
/// a repeated repository wins, in declaration order.
fn check_repository_states<'a>(
    subject: &str,
    label: &str,
    list: &'a [RepositoryStateInput],
    problems: &mut Vec<Diagnostic>,
) -> OrderedMap<&'a str, &'a RepositoryStateInput> {
    let mut by_repo = OrderedMap::new();
    if list.is_empty() {
        problems.push(fail(format!(
            "{subject} {label} lists no repository state; source and result states are pinned per repository"
        )));
    }
    for s in list {
        let raw = s.repository_ref.as_str();
        if s.repository_ref.is_blank() {
            problems.push(fail(format!(
                "{subject} {label} entry {raw} has no repository_ref"
            )));
        } else {
            if let Some(r) = non_portable_reason(Some(raw)) {
                problems.push(fail(format!(
                    "{subject} {label} entry {raw} repository_ref contains {r}"
                )));
            }
            let key = raw.trim();
            if by_repo.contains(&key) {
                problems.push(fail(format!(
                    "{subject} {label} repository_ref \"{raw}\" is listed more than once"
                )));
            } else {
                by_repo.insert(key, s);
            }
        }
        let revision = s.revision.as_ref().map(RecordText::as_str);
        if let Some(r) = revision.and_then(|rev| non_portable_reason(Some(rev))) {
            problems.push(fail(format!(
                "{subject} {label} entry {raw} revision contains {r}"
            )));
        }
        if let Some(defect) = pin_defect(revision, s.sha256.is_some()) {
            problems.push(fail(format!(
                "{subject} {label} entry {raw} is not pinned to an exact revision: {defect}"
            )));
        }
    }
    by_repo
}

/// A repository's pin as a comparable tuple (`repoPinKey`).
fn repo_pin_key(state: &RepositoryStateInput) -> (String, String) {
    (
        state
            .revision
            .as_ref()
            .map_or(String::new(), |r| r.as_str().trim().to_string()),
        state
            .sha256
            .as_ref()
            .map_or(String::new(), |s| s.as_str().trim().to_lowercase()),
    )
}

/// Sections 13 and 14: source and result states, and changed paths.
fn check_repository_states_and_paths(
    subject: &str,
    body: &HandoffBody,
    problems: &mut Vec<Diagnostic>,
) {
    let src = check_repository_states(subject, "source_state", &body.source_state, problems);
    let res = check_repository_states(subject, "result_state", &body.result_state, problems);
    if !src.is_empty() && !res.is_empty() {
        let source_only: Vec<&str> = src.keys().filter(|k| !res.contains(k)).copied().collect();
        let result_only: Vec<&str> = res.keys().filter(|k| !src.contains(k)).copied().collect();
        if !source_only.is_empty() || !result_only.is_empty() {
            let mut parts = Vec::new();
            if !source_only.is_empty() {
                parts.push(format!("source-only: {}", source_only.join(", ")));
            }
            if !result_only.is_empty() {
                parts.push(format!("result-only: {}", result_only.join(", ")));
            }
            problems.push(fail(format!(
                "{subject} source_state and result_state pin different sets of repositories ({}); a run's before and after states cover the same repositories",
                parts.join("; ")
            )));
        }
    }

    let mut seen_id = HashSet::new();
    let mut seen_pair = HashSet::new();
    for c in &body.changed_paths {
        let at = c.id.as_str();
        if !seen_id.insert(&c.id) {
            problems.push(fail(format!(
                "{subject} changed path id \"{at}\" is used more than once"
            )));
        }
        for (f, value) in [
            ("repository_ref", c.repository_ref.as_str()),
            ("path", c.path.as_str()),
        ] {
            if value.trim().is_empty() {
                problems.push(fail(format!("{subject} changed path {at} has no {f}")));
            } else if let Some(r) = non_portable_reason(Some(value)) {
                problems.push(fail(format!(
                    "{subject} changed path {at} {f} contains {r}"
                )));
            }
        }
        let raw_repo = c.repository_ref.as_str();
        let repo = raw_repo.trim();
        let path = c.path.as_str().trim();
        if !repo.is_empty()
            && !path.is_empty()
            && !seen_pair.insert((repo.to_string(), path.to_string()))
        {
            problems.push(fail(format!(
                "{subject} changed path repeats \"{}\" in repository \"{raw_repo}\"",
                c.path.as_str()
            )));
        }
        if !repo.is_empty() {
            match (src.get(&repo), res.get(&repo)) {
                (Some(before), Some(after)) => {
                    if repo_pin_key(before) == repo_pin_key(after) {
                        problems.push(fail(format!(
                            "{subject} changed path {at} names repository \"{raw_repo}\", but its source_state and result_state pins are identical; a change produced a new revision"
                        )));
                    }
                }
                _ => problems.push(fail(format!(
                    "{subject} changed path {at} names repository \"{raw_repo}\", which is not pinned in both source_state and result_state; a change has a before and an after revision"
                ))),
            }
        }
    }
}

/// The flags section 19 reads from section 15.
struct Flags {
    blocking_deviation: bool,
    blocking_owner_decision: bool,
}

/// Section 15: external effects, deviations, open gaps, required owner decisions.
fn check_effects_deviations_gaps_decisions(
    subject: &str,
    body: &HandoffBody,
    problems: &mut Vec<Diagnostic>,
) -> Flags {
    let mut seen = HashSet::new();
    for e in &body.external_effects {
        let at = e.id.as_str();
        if !seen.insert(&e.id) {
            problems.push(fail(format!(
                "{subject} external effect id \"{at}\" is used more than once"
            )));
        }
        required_text(
            subject,
            &format!("external effect {at}"),
            "description",
            Some(&e.description),
            || format!("{subject} external effect {at} has no description"),
            problems,
        );
    }

    let mut blocking_deviation = false;
    let mut seen = HashSet::new();
    for d in &body.deviations {
        let at = d.id.as_str();
        if !seen.insert(&d.id) {
            problems.push(fail(format!(
                "{subject} deviation id \"{at}\" is used more than once"
            )));
        }
        for (f, value) in [
            ("description", &d.description),
            ("disposition", &d.disposition),
        ] {
            required_text(
                subject,
                &format!("deviation {at}"),
                f,
                Some(value),
                || format!("{subject} deviation {at} has no {f}"),
                problems,
            );
        }
        blocking_deviation |= d.severity == DeviationSeverity::Blocking;
    }

    let mut seen = HashSet::new();
    for g in &body.open_gaps {
        let at = g.id.as_str();
        if !seen.insert(&g.id) {
            problems.push(fail(format!(
                "{subject} open_gaps id \"{at}\" is used more than once"
            )));
        }
        required_text(
            subject,
            &format!("open_gaps entry {at}"),
            "description",
            Some(&g.description),
            || format!("{subject} open_gaps entry {at} has no description"),
            problems,
        );
    }

    let mut blocking_owner_decision = false;
    let mut seen = HashSet::new();
    for o in &body.owner_decisions {
        let at = o.id.as_str();
        if !seen.insert(&o.id) {
            problems.push(fail(format!(
                "{subject} required owner decision id \"{at}\" is used more than once"
            )));
        }
        required_text(
            subject,
            &format!("required owner decision {at}"),
            "question",
            Some(&o.question),
            || format!("{subject} required owner decision {at} has no question"),
            problems,
        );
        blocking_owner_decision |= o.blocking;
    }
    Flags {
        blocking_deviation,
        blocking_owner_decision,
    }
}

/// Section 16: blockers.
fn check_blockers<'a>(
    subject: &str,
    body: &'a HandoffBody,
    problems: &mut Vec<Diagnostic>,
) -> OrderedSet<&'a SemanticId> {
    let mut ids = OrderedSet::new();
    for b in &body.blockers {
        let at = b.id.as_str();
        if !ids.insert(&b.id) {
            problems.push(fail(format!(
                "{subject} blocker id \"{at}\" is used more than once"
            )));
        }
        if b.description.is_blank() {
            problems.push(fail(format!("{subject} blocker {at} has no description")));
        }
        if b.resumption_condition.is_blank() {
            problems.push(fail(format!(
                "{subject} blocker {at} has no verifiable resumption condition"
            )));
        }
        for text in [&b.description, &b.resumption_condition] {
            if let Some(r) = non_portable_reason(Some(text.as_str())) {
                problems.push(fail(format!("{subject} blocker {at} contains {r}")));
            }
        }
    }
    ids
}

/// Section 17: the worktree disposition — the closed set of
/// `version-control-flow.md` §5.4.
fn check_worktree(subject: &str, body: &HandoffBody, problems: &mut Vec<Diagnostic>) {
    let wt = &body.worktree;
    let fields = [
        ("reason", wt.reason.as_ref()),
        ("responsible", wt.responsible.as_ref()),
        ("cleanup_condition", wt.cleanup_condition.as_ref()),
    ];
    match wt.state {
        WorktreeState::Retained => {
            for (f, value) in fields {
                if blank(value) {
                    problems.push(fail(format!(
                        "{subject} worktree_disposition is \"retained\" but states no {f}; a retained worktree carries a reason, a responsible party and a verifiable cleanup condition"
                    )));
                } else if let Some(r) = value.and_then(|v| non_portable_reason(Some(v.as_str()))) {
                    problems.push(fail(format!(
                        "{subject} worktree_disposition {f} contains {r}"
                    )));
                }
            }
        }
        WorktreeState::NotCreated | WorktreeState::Removed => {
            for (f, value) in fields {
                if value.is_some() {
                    problems.push(fail(format!(
                        "{subject} worktree_disposition is \"{}\" but carries \"{f}\"; reason, responsible and cleanup_condition belong to the \"retained\" state only",
                        wt.state.as_str()
                    )));
                }
            }
        }
    }
}

/// Section 18: the next step, stated explicitly.
fn check_next_step(subject: &str, body: &HandoffBody, problems: &mut Vec<Diagnostic>) {
    let step = &body.next_step;
    for (f, value) in [
        ("action", step.action.as_ref()),
        ("gate", step.gate.as_ref()),
        ("actor_ref", step.actor_ref.as_ref()),
    ] {
        let Some(value) = value else { continue };
        if value.is_blank() {
            problems.push(fail(format!(
                "{subject} next_step.{f} is present but empty; state a value, or null where there is none"
            )));
        }
        if let Some(r) = non_portable_reason(Some(value.as_str())) {
            problems.push(fail(format!("{subject} next_step.{f} contains {r}")));
        }
    }
}

/// Section 19: the overall outcome and its axis rules.
fn check_outcome(
    subject: &str,
    body: &HandoffBody,
    checks: &Checks<'_>,
    flags: &Flags,
    blocker_ids: &OrderedSet<&SemanticId>,
    problems: &mut Vec<Diagnostic>,
) {
    let outcome = &body.outcome;
    required_text(
        subject,
        "outcome",
        "statement",
        Some(&outcome.statement),
        || format!("{subject} outcome states no statement"),
        problems,
    );
    let every_result_established = !body.claimed_results.is_empty()
        && body
            .claimed_results
            .iter()
            .all(|c| c.status == ClaimedResultStatus::Established);
    let open_gaps = body.open_gaps.len();
    let blocked_trigger =
        !blocker_ids.is_empty() || checks.failed_or_unable || flags.blocking_deviation;
    match outcome.status {
        OutcomeStatus::Complete => {
            if !every_result_established {
                problems.push(fail(format!(
                    "{subject} outcome.status is \"complete\" but not every claimed result is established"
                )));
            }
            if !checks.every_passed {
                problems.push(fail(format!(
                    "{subject} outcome.status is \"complete\" but a mandatory check is not \"passed\"; a failed or unable check is not a completion"
                )));
            }
            if !blocker_ids.is_empty() || flags.blocking_deviation {
                problems.push(fail(format!(
                    "{subject} outcome.status is \"complete\" but a blocker or a blocking deviation remains"
                )));
            }
            if flags.blocking_owner_decision {
                problems.push(fail(format!(
                    "{subject} outcome.status is \"complete\" but a required owner decision is blocking; the next step cannot proceed, so the run is handed off, not complete"
                )));
            }
            if open_gaps > 0 {
                problems.push(fail(format!(
                    "{subject} outcome.status is \"complete\" but {open_gaps} open gap(s) remain; a completed run carries no acknowledged incompleteness — record it as a deviation or leave the run handed_off_incomplete"
                )));
            }
        }
        OutcomeStatus::Blocked if !blocked_trigger => problems.push(fail(format!(
            "{subject} outcome.status is \"blocked\" but no blocker, failed or unable mandatory check, or blocking deviation is recorded"
        ))),
        OutcomeStatus::HandedOffIncomplete if blocked_trigger => problems.push(fail(format!(
            "{subject} outcome.status is \"handed_off_incomplete\" but a blocker, a failed or unable mandatory check, or a blocking deviation is recorded; that is a \"blocked\" outcome"
        ))),
        _ => {}
    }
    if !blocker_ids.is_empty() && outcome.status != OutcomeStatus::Blocked {
        problems.push(fail(format!(
            "{subject} carries {} blocker(s) but outcome.status is \"{}\"; a non-empty blocker set is a \"blocked\" outcome",
            blocker_ids.len(),
            outcome.status.as_str()
        )));
    }
}
