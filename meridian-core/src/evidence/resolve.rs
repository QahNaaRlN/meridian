//! One pinned piece of evidence, checked and resolved the same way in every
//! evidence-bearing contract (`checkEvidence…`/`resolveEvidenceEntry` of
//! `scripts/lib/evidence-and-handoff.mjs` and
//! `scripts/lib/field-evaluation.mjs`): its own shape and pin first, then
//! its `evidence-result` transformer response through the shared
//! resolution boundary and the closed observed-result pool. What subject the
//! result is about — assertions and a check (handoff) or a metric (field
//! evaluation) — is completed by the calling contract.

use std::collections::HashSet;

use crate::run_contracts::envelope::RecordFamily;
use crate::run_contracts::resolution::check_resolved_entry;
use crate::run_contracts::revision::pin_defect;
use crate::run_contracts::{PinnedRecordKind, ResolutionCatalogue, ResolvedEntry, ResponseField};
use crate::task_contracts::non_portable_reason;
use crate::types::{Diagnostic, SemanticId};

use super::types::{ObservedResult, PinnedEvidence};
use super::{fail, record_subject};

/// The shape and pin of one piece of evidence. `seen` collects the ids
/// already declared by the record.
pub(crate) fn check_pinned_evidence<'e>(
    family: RecordFamily,
    id: &str,
    evidence: &'e PinnedEvidence,
    seen: &mut HashSet<&'e SemanticId>,
    problems: &mut Vec<Diagnostic>,
) {
    let subject = record_subject(family, id);
    let at = evidence.id.as_str();
    if !seen.insert(&evidence.id) {
        problems.push(fail(format!(
            "{subject} evidence id \"{at}\" is used more than once"
        )));
    }
    for (name, value) in [
        ("reference", evidence.reference.as_str()),
        ("summary", evidence.summary.as_str()),
    ] {
        if value.trim().is_empty() {
            problems.push(fail(format!("{subject} evidence entry {at} has no {name}")));
        } else if let Some(r) = non_portable_reason(Some(value)) {
            problems.push(fail(format!(
                "{subject} evidence entry {at} {name} contains {r}"
            )));
        }
    }
    let revision = evidence.revision.as_ref().map(|r| r.as_str());
    if let Some(r) = revision.and_then(|rev| non_portable_reason(Some(rev))) {
        problems.push(fail(format!(
            "{subject} evidence entry {at} revision contains {r}"
        )));
    }
    if let Some(defect) = pin_defect(revision, evidence.sha256.is_some()) {
        let why = match family {
            RecordFamily::EvidenceAndHandoff => "a plain reference is not verifiable evidence",
            _ => "an unpinned reference is not verifiable evidence",
        };
        problems.push(fail(format!(
            "{subject} evidence entry {at} is not pinned to an exact edition: {defect}; {why}"
        )));
    }
}

/// What resolving one piece of evidence yielded: the transformer response
/// (when the reference resolved at all) and its observed result when that
/// is a member of the closed pool.
pub(crate) struct EvidenceResolution<'c> {
    pub entry: Option<&'c ResolvedEntry>,
    pub observed: Option<ObservedResult>,
}

/// Resolves one piece of evidence as an `evidence-result` and checks the
/// closed observed-result pool.
pub(crate) fn resolve_evidence_result<'c>(
    family: RecordFamily,
    id: &str,
    evidence: &PinnedEvidence,
    catalogue: &'c ResolutionCatalogue,
    problems: &mut Vec<Diagnostic>,
) -> EvidenceResolution<'c> {
    let at = evidence.id.as_str();
    let field = format!("evidence entry {at}");
    let entry = check_resolved_entry(
        family,
        id,
        &field,
        PinnedRecordKind::EvidenceResult,
        &evidence.result_pin(),
        catalogue,
        problems,
    );
    let mut observed = None;
    if let Some(e) = entry {
        let subject = record_subject(family, id);
        let result = &e.evidence_result.observed_result;
        match result {
            ResponseField::Absent => problems.push(fail(format!(
                "{subject} evidence entry {at}: the resolved evidence-result confirms no observed_result; the transformer says whether the artefact confirmed, contradicted or was inconclusive about its subject"
            ))),
            _ => match result.text().and_then(ObservedResult::parse) {
                Some(r) => observed = Some(r),
                None => problems.push(fail(format!(
                    "{subject} evidence entry {at}: the resolved evidence-result's observed_result {} is not one of {{ {} }}",
                    result.json_or_null(),
                    ObservedResult::ALL.map(ObservedResult::as_str).join(", ")
                ))),
            },
        }
    }
    EvidenceResolution { entry, observed }
}
