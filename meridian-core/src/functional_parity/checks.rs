//! Pure `functional-parity` evidence checks: the eleven inference rules a
//! JSON Schema subset cannot express because they relate one part of a
//! record to another (`verification/functional-parity/functional-parity-evidence.schema.json`'s
//! own top-level `description`,
//! `verification/functional-parity/functional-parity-evidence-contract.md`
//! §7 and §9). [`check_document`] is the ONE public constructor path to
//! [`FunctionalParityEvidence`]: a caller never obtains one that skipped any
//! of these checks.
//!
//! Direct, line-for-line port of `functionalParityConsistency`
//! (`scripts/kernel-validate.mjs`): every diagnostic text and the `${at}`
//! record-index suffix (` (record N)`, only when the document carries more
//! than one record) are reproduced EXACTLY, at the exact position Node
//! splices it in, and every loop's iteration order matches Node's own
//! `Map`/`Set` insertion order (corrective round item 6) — over
//! [`super::construction`]'s typed input instead of `serde_json::Value`.

use std::collections::{HashMap, HashSet};

use crate::types::{Diagnostic, DiagnosticLevel};

use super::construction::{
    BaselineInput, FacetContent, FacetInput, FunctionalParityEvidence, GapInput,
    PostChangeConditions, PostChangeInput, RecordEvidence, RecordInput, VerdictInput,
    VerdictOutcome,
};
use super::types::{AssertionId, ConditionId, Facet};

fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message).expect("message is non-empty")
}

/// The declared assertion catalog for one record: every assertion actually
/// declared, in declaration order (the four facets, in
/// [`Facet::ALL`](super::types::Facet::ALL) order, each walked top-to-bottom),
/// paired with its one owning facet. A [`PreservedContractCatalog`] never
/// carries two entries for the same [`AssertionId`] — [`build_catalog`] is
/// the only way to obtain one, and it reports a repeated id as a diagnostic
/// instead of inserting it twice (matches Node's own `catalog.has(id)`
/// guard, which never overwrites the first-seen owner either).
struct PreservedContractCatalog {
    order: Vec<AssertionId>,
    owner: HashMap<AssertionId, Facet>,
}

impl PreservedContractCatalog {
    fn declared_order(&self) -> &[AssertionId] {
        &self.order
    }

    fn owning_facet(&self, id: &AssertionId) -> Option<Facet> {
        self.owner.get(id).copied()
    }

    fn declared(&self) -> HashSet<&AssertionId> {
        self.order.iter().collect()
    }
}

fn build_catalog(
    at: &str,
    facets: &[FacetInput; 4],
) -> (PreservedContractCatalog, Vec<Diagnostic>) {
    let mut problems = Vec::new();
    let mut order = Vec::new();
    let mut owner: HashMap<AssertionId, Facet> = HashMap::new();

    for facet_input in facets {
        let FacetContent::Declared(assertions) = &facet_input.content else {
            continue;
        };
        for assertion in assertions {
            let id = &assertion.id;
            if let Some(existing_owner) = owner.get(id) {
                problems.push(fail(format!(
                    "assertion id \"{id}\"{at} is declared more than once (facets \"{existing_owner}\" and \"{}\"); ids are unique across the whole record",
                    facet_input.facet
                )));
            } else {
                owner.insert(id.clone(), facet_input.facet);
                order.push(id.clone());
            }
        }
    }

    (PreservedContractCatalog { order, owner }, problems)
}

/// Rules 3-4-5-6: per-assertion verdict naming/completeness, evidence
/// coverage, post-change contract links, and VERIFIED requiring both.
fn check_verdict_and_coverage(
    at: &str,
    catalog: &PreservedContractCatalog,
    verdict: &VerdictInput,
    evidence: &[super::construction::EvidenceEntryInput],
    post_change: &PostChangeInput,
) -> (Vec<Diagnostic>, HashMap<AssertionId, bool>) {
    let mut problems = Vec::new();
    let declared = catalog.declared();

    // Rule 3: every per-assertion verdict names a declared assertion, and
    // every declared assertion carries exactly one per-assertion verdict.
    let mut seen: HashMap<&AssertionId, usize> = HashMap::new();
    for v in &verdict.per_assertion {
        *seen.entry(&v.assertion_id).or_insert(0) += 1;
        if !declared.contains(&v.assertion_id) {
            problems.push(fail(format!(
                "the verdict names assertion \"{}\"{at}, which no preserved-contract facet declares",
                v.assertion_id
            )));
        }
    }
    for id in catalog.declared_order() {
        let n = seen.get(id).copied().unwrap_or(0);
        if n == 0 {
            problems.push(fail(format!(
                "assertion \"{id}\"{at} carries no per-assertion verdict"
            )));
        } else if n > 1 {
            problems.push(fail(format!(
                "assertion \"{id}\"{at} carries more than one per-assertion verdict"
            )));
        }
    }
    // `state_of` maps an id to whether its LAST per-assertion entry is
    // VERIFIED — last-write-wins over a duplicate id, matching Node's own
    // `new Map(perAssertion.map(...))` construction exactly.
    let mut state_of: HashMap<AssertionId, bool> = HashMap::new();
    for v in &verdict.per_assertion {
        state_of.insert(v.assertion_id.clone(), v.state.is_verified());
    }

    // Rule 4 (evidence half): every evidence `covers` id resolves to a
    // declared assertion.
    let mut covered: HashSet<AssertionId> = HashSet::new();
    for entry in evidence {
        for id in &entry.covers {
            if !declared.contains(id) {
                problems.push(fail(format!(
                    "an evidence entry covers assertion \"{id}\"{at}, which no preserved-contract facet declares"
                )));
            } else {
                covered.insert(id.clone());
            }
        }
    }

    // Rule 4 (contract-link half): every post-change `contract_links` entry
    // resolves to the exact {facet, assertion_id} pair of a declared
    // assertion.
    let mut linked: HashSet<AssertionId> = HashSet::new();
    for link in &post_change.contract_links {
        if !declared.contains(&link.assertion_id) {
            problems.push(fail(format!(
                "a post-change contract link names assertion \"{}\"{at}, which no preserved-contract facet declares",
                link.assertion_id
            )));
        } else if catalog.owning_facet(&link.assertion_id) != Some(link.facet) {
            let owner = catalog
                .owning_facet(&link.assertion_id)
                .map(|f| f.to_string())
                .unwrap_or_default();
            problems.push(fail(format!(
                "a post-change contract link names assertion \"{}\"{at} under facet \"{}\", but it is declared under facet \"{owner}\"",
                link.assertion_id, link.facet
            )));
        } else {
            linked.insert(link.assertion_id.clone());
        }
    }

    // Rule 6: a per-assertion VERIFIED needs BOTH a covering evidence entry
    // AND a post-change contract link that resolves to it.
    for id in catalog.declared_order() {
        if state_of.get(id).copied() != Some(true) {
            continue;
        }
        if !covered.contains(id) {
            problems.push(fail(format!(
                "assertion \"{id}\"{at} is VERIFIED but no evidence entry covers it"
            )));
        }
        if !linked.contains(id) {
            problems.push(fail(format!(
                "assertion \"{id}\"{at} is VERIFIED but no post-change contract link resolves to it; evidence coverage alone is not sufficient"
            )));
        }
    }

    (problems, state_of)
}

/// Rule 7: any UNVERIFIED per-assertion state forces an UNVERIFIED overall.
fn check_any_unverified_forces_overall(
    at: &str,
    state_of: &HashMap<AssertionId, bool>,
    overall: &VerdictOutcome,
) -> Vec<Diagnostic> {
    let any_unverified = state_of.values().any(|verified| !verified);
    if any_unverified && overall.is_verified() {
        vec![fail(format!(
            "a per-assertion verdict is UNVERIFIED{at} but the overall verdict is not"
        ))]
    } else {
        Vec::new()
    }
}

/// Rule 8: a record-scoped gap forces every declared per-assertion verdict,
/// and the overall verdict, UNVERIFIED; an assertion-scoped gap forces only
/// the assertions it names.
fn check_gaps(
    at: &str,
    catalog: &PreservedContractCatalog,
    gaps: &[GapInput],
    state_of: &HashMap<AssertionId, bool>,
    overall: &VerdictOutcome,
) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    let declared = catalog.declared();

    for gap in gaps {
        match gap {
            GapInput::Record { .. } => {
                for id in catalog.declared_order() {
                    if state_of.get(id).copied() == Some(true) {
                        problems.push(fail(format!(
                            "a record-scoped gap is recorded{at} but assertion \"{id}\" is VERIFIED; a record-scoped gap leaves every assertion UNVERIFIED"
                        )));
                    }
                }
                if overall.is_verified() {
                    problems.push(fail(format!(
                        "a record-scoped gap is recorded{at} but the overall verdict is not UNVERIFIED"
                    )));
                }
            }
            GapInput::Assertion { assertion_ids, .. } => {
                for id in assertion_ids {
                    if !declared.contains(id) {
                        problems.push(fail(format!(
                            "a gap names assertion \"{id}\"{at}, which no preserved-contract facet declares"
                        )));
                    } else if state_of.get(id).copied() != Some(false) {
                        problems.push(fail(format!(
                            "a gap leaves assertion \"{id}\"{at} unclosed but its verdict is not UNVERIFIED"
                        )));
                    }
                }
            }
        }
    }

    problems
}

/// Rule 2 (baseline-condition half) and rule 9: baseline condition id
/// uniqueness, and `relationship: same` reproducing exactly the baseline's
/// identified conditions.
fn check_baseline_conditions(
    at: &str,
    baseline: &BaselineInput,
    post_change: &PostChangeInput,
) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    let mut base_cond_ids: HashSet<ConditionId> = HashSet::new();
    let mut base_cond_order: Vec<ConditionId> = Vec::new();
    for c in &baseline.conditions {
        let id = &c.id;
        if base_cond_ids.contains(id) {
            problems.push(fail(format!(
                "baseline condition id \"{id}\"{at} is declared more than once"
            )));
        } else {
            base_cond_ids.insert(id.clone());
            base_cond_order.push(id.clone());
        }
    }

    if let PostChangeConditions::Same {
        baseline_condition_ids,
    } = &post_change.conditions
    {
        let ref_set: HashSet<&ConditionId> = baseline_condition_ids.iter().collect();
        for r in baseline_condition_ids {
            if !base_cond_ids.contains(r) {
                problems.push(fail(format!(
                    "post-change conditions are declared \"same\"{at} but reference baseline condition \"{r}\", which the baseline does not define"
                )));
            }
        }
        for id in &base_cond_order {
            if !ref_set.contains(id) {
                problems.push(fail(format!(
                    "post-change conditions are declared \"same\"{at} but do not reproduce baseline condition \"{id}\""
                )));
            }
        }
    }

    problems
}

/// Rule 10: a baseline whose provenance is not established forces every
/// declared per-assertion verdict, and the overall verdict, UNVERIFIED.
fn check_baseline_provenance(
    at: &str,
    catalog: &PreservedContractCatalog,
    baseline: &BaselineInput,
    state_of: &HashMap<AssertionId, bool>,
    overall: &VerdictOutcome,
) -> Vec<Diagnostic> {
    if baseline.provenance.established {
        return Vec::new();
    }
    let mut problems = Vec::new();
    for id in catalog.declared_order() {
        if state_of.get(id).copied() == Some(true) {
            problems.push(fail(format!(
                "the baseline provenance was not established{at} but assertion \"{id}\" is VERIFIED; an unestablished baseline leaves every assertion UNVERIFIED"
            )));
        }
    }
    if overall.is_verified() {
        problems.push(fail(format!(
            "the baseline provenance was not established{at} but the overall verdict is not UNVERIFIED"
        )));
    }
    problems
}

/// Rule 11: the baseline and post-change source states are a distinguishable
/// pair.
fn check_source_states_distinguishable(
    at: &str,
    baseline: &BaselineInput,
    post_change: &PostChangeInput,
) -> Vec<Diagnostic> {
    let b_id = baseline.source_state.identifier.as_str();
    if !b_id.is_empty()
        && b_id == post_change.source_state.identifier.as_str()
        && baseline.source_state.identifier_kind.as_str()
            == post_change.source_state.identifier_kind.as_str()
    {
        vec![fail(format!(
            "the baseline and post-change source states are the identical pair {{identifier_kind: \"{}\", identifier: \"{}\"}}{at}; a before/after comparison needs two distinguishable states",
            baseline.source_state.identifier_kind, b_id
        ))]
    } else {
        Vec::new()
    }
}

/// Runs every rule (2 through 11 — rule 1, "at least one record", is
/// [`check_document`]'s own document-level gate) over one already-typed
/// record, returning its diagnostics and, only when they are empty, its
/// accepted [`RecordEvidence`]. `at` is the exact Node-matching record-index
/// suffix (` (record N)` or `""`), spliced into each message at the exact
/// position the Node reference does.
fn check_record(at: &str, record: &RecordInput) -> (Vec<Diagnostic>, Option<RecordEvidence>) {
    let mut problems = Vec::new();

    let (catalog, catalog_problems) = build_catalog(at, &record.facets);
    problems.extend(catalog_problems);
    if catalog.declared_order().is_empty() {
        problems.push(fail(format!(
            "no preserved-contract assertion is declared{at}; a parity record with no assertion proves nothing"
        )));
    }

    let (verdict_problems, state_of) = check_verdict_and_coverage(
        at,
        &catalog,
        &record.verdict,
        &record.evidence,
        &record.post_change,
    );
    problems.extend(verdict_problems);

    problems.extend(check_any_unverified_forces_overall(
        at,
        &state_of,
        &record.verdict.overall,
    ));
    problems.extend(check_gaps(
        at,
        &catalog,
        &record.gaps,
        &state_of,
        &record.verdict.overall,
    ));
    problems.extend(check_baseline_conditions(
        at,
        &record.baseline,
        &record.post_change,
    ));
    problems.extend(check_baseline_provenance(
        at,
        &catalog,
        &record.baseline,
        &state_of,
        &record.verdict.overall,
    ));
    problems.extend(check_source_states_distinguishable(
        at,
        &record.baseline,
        &record.post_change,
    ));

    if problems.is_empty() {
        let mut verified_assertions: Vec<AssertionId> = catalog
            .declared_order()
            .iter()
            .filter(|id| state_of.get(*id).copied() == Some(true))
            .cloned()
            .collect();
        verified_assertions.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        let evidence = RecordEvidence::new(record.clone(), verified_assertions);
        (problems, Some(evidence))
    } else {
        (problems, None)
    }
}

/// The whole `functional-parity` document check: rule 1 ("the document
/// carries at least one record"), then every record's own rules 2-11, in
/// declaration order. Returns every diagnostic across the whole document
/// and, only when the list is empty, the accepted [`FunctionalParityEvidence`]
/// — the ONE way to obtain one.
pub fn check_document(
    records: &[RecordInput],
) -> (Vec<Diagnostic>, Option<FunctionalParityEvidence>) {
    if records.is_empty() {
        return (
            vec![fail(
                "the evidence document carries no records; a functional-parity document with no record proves nothing",
            )],
            None,
        );
    }

    let multi = records.len() > 1;
    let mut problems = Vec::new();
    let mut evidences = Vec::with_capacity(records.len());

    for (index, record) in records.iter().enumerate() {
        let at = if multi {
            format!(" (record {index})")
        } else {
            String::new()
        };
        let (record_problems, record_evidence) = check_record(&at, record);
        problems.extend(record_problems);
        if let Some(evidence) = record_evidence {
            evidences.push(evidence);
        }
    }

    let resolved = if problems.is_empty() {
        Some(FunctionalParityEvidence::new(evidences))
    } else {
        None
    };
    (problems, resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::functional_parity::construction::{
        Assertion, AssertionVerdictInput, BaselineInput as Baseline, Condition, ContractLinkInput,
        EvidenceEntryInput, IdentifiedCondition, ObservedResult, PostChangeInput as PostChange,
        Provenance, RecordInput as Record, VerdictInput as Verdict,
    };
    use crate::functional_parity::types::{EvidenceKind, EvidenceText, SourceStateRef};

    fn aid(s: &str) -> AssertionId {
        AssertionId::new(s).unwrap()
    }
    fn cid(s: &str) -> ConditionId {
        ConditionId::new(s).unwrap()
    }
    fn text(s: &str) -> EvidenceText {
        EvidenceText::new(s).unwrap()
    }
    fn state_ref(id: &str) -> SourceStateRef {
        SourceStateRef {
            identifier: text(id),
            identifier_kind: text("described-source-revision"),
        }
    }
    fn declared_facet(facet: Facet, ids: &[&str]) -> FacetInput {
        FacetInput {
            facet,
            content: FacetContent::Declared(
                ids.iter()
                    .map(|id| Assertion {
                        id: aid(id),
                        statement: text("statement"),
                    })
                    .collect(),
            ),
        }
    }
    fn not_applicable_facet(facet: Facet) -> FacetInput {
        FacetInput {
            facet,
            content: FacetContent::NotApplicable(text("n/a")),
        }
    }
    fn observed_result() -> ObservedResult {
        ObservedResult {
            summary: text("summary"),
            artifacts: vec![],
        }
    }

    fn well_formed_record() -> Record {
        Record {
            work_item: text("refactor-example-001"),
            recorded_at: text("2026-09-23"),
            facets: [
                declared_facet(Facet::PublicApi, &["api.exports"]),
                declared_facet(Facet::ObservableIo, &["io.mapping"]),
                not_applicable_facet(Facet::SideEffectsAndInteractions),
                not_applicable_facet(Facet::UserVisibleBehavior),
            ],
            baseline: Baseline {
                source_state: state_ref("state-A"),
                conditions: vec![IdentifiedCondition {
                    id: cid("cond.inputs"),
                    description: text("description"),
                    kind: None,
                }],
                provenance: Provenance {
                    method: text("method"),
                    established: true,
                },
                observed_result: observed_result(),
            },
            post_change: PostChange {
                source_state: state_ref("state-B"),
                conditions: PostChangeConditions::Same {
                    baseline_condition_ids: vec![cid("cond.inputs")],
                },
                observed_result: observed_result(),
                contract_links: vec![
                    ContractLinkInput {
                        facet: Facet::PublicApi,
                        assertion_id: aid("api.exports"),
                    },
                    ContractLinkInput {
                        facet: Facet::ObservableIo,
                        assertion_id: aid("io.mapping"),
                    },
                ],
            },
            evidence: vec![
                EvidenceEntryInput {
                    kind: EvidenceKind::InterfaceEnumeration,
                    observed_scope: text("scope"),
                    covers: vec![aid("api.exports")],
                    limitations: vec![text("limit")],
                    applicability_justification: None,
                },
                EvidenceEntryInput {
                    kind: EvidenceKind::BehavioralAssertion,
                    observed_scope: text("scope"),
                    covers: vec![aid("io.mapping")],
                    limitations: vec![text("limit")],
                    applicability_justification: None,
                },
            ],
            gaps: vec![],
            verdict: Verdict {
                per_assertion: vec![
                    AssertionVerdictInput {
                        assertion_id: aid("api.exports"),
                        state: VerdictOutcome::Verified,
                    },
                    AssertionVerdictInput {
                        assertion_id: aid("io.mapping"),
                        state: VerdictOutcome::Verified,
                    },
                ],
                overall: VerdictOutcome::Verified,
            },
        }
    }

    #[test]
    fn a_well_formed_record_resolves_cleanly() {
        let (problems, resolved) = check_document(&[well_formed_record()]);
        assert!(problems.is_empty(), "{problems:?}");
        let resolved = resolved.expect("a clean document resolves");
        assert_eq!(resolved.records().len(), 1);
        assert!(resolved.records()[0].overall().is_verified());
        assert_eq!(resolved.records()[0].verified_assertions().len(), 2);
    }

    #[test]
    fn an_empty_document_is_rejected() {
        let (problems, resolved) = check_document(&[]);
        assert!(resolved.is_none());
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert_eq!(
            problems[0].message(),
            "the evidence document carries no records; a functional-parity document with no record proves nothing"
        );
    }

    #[test]
    fn a_duplicate_assertion_id_across_facets_is_rejected_with_the_exact_node_text() {
        let mut record = well_formed_record();
        record.facets[2] = declared_facet(Facet::SideEffectsAndInteractions, &["api.exports"]);
        let (problems, resolved) = check_document(&[record]);
        assert!(resolved.is_none());
        assert!(
            problems.iter().any(|p| p.message()
                == "assertion id \"api.exports\" is declared more than once (facets \"public_api\" and \"side_effects_and_interactions\"); ids are unique across the whole record"),
            "{problems:?}"
        );
    }

    #[test]
    fn a_verified_assertion_with_no_covering_evidence_is_rejected() {
        let mut record = well_formed_record();
        record
            .evidence
            .retain(|e| !e.covers.contains(&aid("io.mapping")));
        let (problems, resolved) = check_document(&[record]);
        assert!(resolved.is_none());
        assert!(
            problems.iter().any(|p| p.message()
                == "assertion \"io.mapping\" is VERIFIED but no evidence entry covers it"),
            "{problems:?}"
        );
    }

    #[test]
    fn a_verified_assertion_with_no_contract_link_is_rejected() {
        let mut record = well_formed_record();
        record
            .post_change
            .contract_links
            .retain(|l| l.assertion_id != aid("io.mapping"));
        let (problems, resolved) = check_document(&[record]);
        assert!(resolved.is_none());
        assert!(
            problems.iter().any(|p| p.message() == "assertion \"io.mapping\" is VERIFIED but no post-change contract link resolves to it; evidence coverage alone is not sufficient"),
            "{problems:?}"
        );
    }

    #[test]
    fn a_contract_link_under_the_wrong_facet_is_rejected_with_the_exact_node_text() {
        let mut record = well_formed_record();
        record.post_change.contract_links[1].facet = Facet::SideEffectsAndInteractions;
        let (problems, resolved) = check_document(&[record]);
        assert!(resolved.is_none());
        assert!(
            problems.iter().any(|p| p.message() == "a post-change contract link names assertion \"io.mapping\" under facet \"side_effects_and_interactions\", but it is declared under facet \"observable_io\""),
            "{problems:?}"
        );
    }

    #[test]
    fn a_record_scoped_gap_with_a_verified_assertion_is_rejected() {
        let mut record = well_formed_record();
        record.gaps.push(GapInput::Record {
            description: text("gap"),
        });
        let (problems, resolved) = check_document(&[record]);
        assert!(resolved.is_none());
        assert!(
            problems.iter().any(|p| p.message() == "a record-scoped gap is recorded but assertion \"api.exports\" is VERIFIED; a record-scoped gap leaves every assertion UNVERIFIED"),
            "{problems:?}"
        );
    }

    #[test]
    fn an_unestablished_baseline_with_a_verified_assertion_is_rejected() {
        let mut record = well_formed_record();
        record.baseline.provenance.established = false;
        record.gaps = vec![];
        let (problems, resolved) = check_document(&[record]);
        assert!(resolved.is_none());
        assert!(
            problems.iter().any(|p| p.message() == "the baseline provenance was not established but assertion \"api.exports\" is VERIFIED; an unestablished baseline leaves every assertion UNVERIFIED"),
            "{problems:?}"
        );
    }

    #[test]
    fn identical_baseline_and_post_change_source_states_are_rejected_with_the_exact_node_text() {
        let mut record = well_formed_record();
        record.post_change.source_state = record.baseline.source_state.clone();
        let (problems, resolved) = check_document(&[record]);
        assert!(resolved.is_none());
        assert!(
            problems.iter().any(|p| p.message() == "the baseline and post-change source states are the identical pair {identifier_kind: \"described-source-revision\", identifier: \"state-A\"}; a before/after comparison needs two distinguishable states"),
            "{problems:?}"
        );
    }

    #[test]
    fn relationship_same_missing_a_baseline_condition_reference_is_rejected() {
        let mut record = well_formed_record();
        record.baseline.conditions.push(IdentifiedCondition {
            id: cid("cond.extra"),
            description: text("description"),
            kind: None,
        });
        let (problems, resolved) = check_document(&[record]);
        assert!(resolved.is_none());
        assert!(
            problems.iter().any(|p| p.message() == "post-change conditions are declared \"same\" but do not reproduce baseline condition \"cond.extra\""),
            "{problems:?}"
        );
    }

    /// Corrective round item 6: an EXACT, ORDERED multi-error regression —
    /// not `.contains()` on any one message — over a record carrying THREE
    /// independent defects at once (a duplicate assertion id, a per-assertion
    /// verdict naming an undeclared assertion, and an unestablished baseline
    /// with a VERIFIED assertion), asserting the full diagnostic list is
    /// byte-for-byte equal, in order, to what the Node reference's own
    /// `functionalParityConsistency` produces for the same document shape.
    #[test]
    fn multi_error_diagnostics_are_exact_and_ordered() {
        let mut record = well_formed_record();
        // Defect 1: duplicate assertion id across facets.
        record.facets[2] = declared_facet(Facet::SideEffectsAndInteractions, &["api.exports"]);
        // Defect 2: an extra per-assertion verdict naming an undeclared id.
        record.verdict.per_assertion.push(AssertionVerdictInput {
            assertion_id: aid("no-such-assertion"),
            state: VerdictOutcome::Verified,
        });
        // Defect 3: unestablished baseline provenance, with api.exports
        // still VERIFIED.
        record.baseline.provenance.established = false;

        let (problems, resolved) = check_document(&[record]);
        assert!(resolved.is_none());
        let texts: Vec<&str> = problems.iter().map(Diagnostic::message).collect();
        assert_eq!(
            texts,
            vec![
                "assertion id \"api.exports\" is declared more than once (facets \"public_api\" and \"side_effects_and_interactions\"); ids are unique across the whole record",
                "the verdict names assertion \"no-such-assertion\", which no preserved-contract facet declares",
                "the baseline provenance was not established but assertion \"api.exports\" is VERIFIED; an unestablished baseline leaves every assertion UNVERIFIED",
                "the baseline provenance was not established but assertion \"io.mapping\" is VERIFIED; an unestablished baseline leaves every assertion UNVERIFIED",
                "the baseline provenance was not established but the overall verdict is not UNVERIFIED",
            ],
            "{texts:#?}"
        );
    }

    /// Corrective round item 6: the exact `(record N)` placement — spliced
    /// immediately after the referenced noun phrase, matching Node's own
    /// `${at}` interpolation point, not appended at the end of the message.
    #[test]
    fn two_records_and_a_second_record_defect_carries_the_exact_node_matching_at_suffix() {
        let good = well_formed_record();
        let mut bad = well_formed_record();
        bad.baseline.provenance.established = false;
        bad.gaps = vec![];
        let (problems, resolved) = check_document(&[good, bad]);
        assert!(resolved.is_none());
        let texts: Vec<&str> = problems.iter().map(Diagnostic::message).collect();
        assert_eq!(
            texts,
            vec![
                "the baseline provenance was not established (record 1) but assertion \"api.exports\" is VERIFIED; an unestablished baseline leaves every assertion UNVERIFIED",
                "the baseline provenance was not established (record 1) but assertion \"io.mapping\" is VERIFIED; an unestablished baseline leaves every assertion UNVERIFIED",
                "the baseline provenance was not established (record 1) but the overall verdict is not UNVERIFIED",
            ],
            "{texts:#?}"
        );
    }

    /// Corrective round (second): accepted evidence is NOT a summary
    /// projection. After a clean `check_document`, the held
    /// `RecordEvidence` still owns representative business data from every
    /// part of the record — assertion statement, not-applicable
    /// justification, baseline condition/provenance/observed result,
    /// post-change `explicitly-comparable` conditions, observed result and
    /// contract link, a full evidence entry, an assertion-scoped gap, and
    /// per-assertion/overall verdicts WITH their UNVERIFIED reasons.
    #[test]
    fn accepted_evidence_retains_the_full_checked_record_not_a_projection() {
        let mut record = well_formed_record();
        record.baseline.conditions[0].kind = Some(text("input-set"));
        record.baseline.observed_result.artifacts = vec![text("baseline-output-set")];
        record.post_change.conditions = PostChangeConditions::ExplicitlyComparable {
            items: vec![Condition {
                description: text("restated inputs"),
                kind: Some(text("environment")),
            }],
            comparability_justification: text("same inputs on a new host"),
        };
        record.post_change.observed_result.summary = text("post-change summary");
        record.evidence[0].kind = EvidenceKind::PublicContractSnapshot;
        record.evidence[0].applicability_justification = Some(text("the api is public"));
        record.verdict.per_assertion[1].state = VerdictOutcome::Unverified {
            reason: text("io not re-observed"),
        };
        record.verdict.overall = VerdictOutcome::Unverified {
            reason: text("one assertion open"),
        };
        record.gaps = vec![GapInput::Assertion {
            description: text("io mapping not re-run"),
            assertion_ids: vec![aid("io.mapping")],
        }];

        let (problems, resolved) = check_document(&[record]);
        assert!(problems.is_empty(), "{problems:?}");
        let resolved = resolved.expect("a clean document resolves");
        let r = &resolved.records()[0];

        assert_eq!(r.work_item().as_str(), "refactor-example-001");
        assert_eq!(r.recorded_at().as_str(), "2026-09-23");

        let FacetContent::Declared(assertions) = &r.facets()[0].content else {
            panic!("public_api must stay declared");
        };
        assert_eq!(assertions[0].id.as_str(), "api.exports");
        assert_eq!(assertions[0].statement.as_str(), "statement");
        let FacetContent::NotApplicable(justification) = &r.facets()[2].content else {
            panic!("side_effects_and_interactions must stay not-applicable");
        };
        assert_eq!(justification.as_str(), "n/a");

        let b = r.baseline();
        assert_eq!(b.source_state.identifier.as_str(), "state-A");
        assert_eq!(b.conditions[0].id.as_str(), "cond.inputs");
        assert_eq!(b.conditions[0].description.as_str(), "description");
        assert_eq!(
            b.conditions[0].kind.as_ref().map(EvidenceText::as_str),
            Some("input-set")
        );
        assert_eq!(b.provenance.method.as_str(), "method");
        assert!(b.provenance.established);
        assert_eq!(b.observed_result.summary.as_str(), "summary");
        assert_eq!(
            b.observed_result.artifacts[0].as_str(),
            "baseline-output-set"
        );

        let pc = r.post_change();
        assert_eq!(pc.source_state.identifier.as_str(), "state-B");
        let PostChangeConditions::ExplicitlyComparable {
            items,
            comparability_justification,
        } = &pc.conditions
        else {
            panic!("post-change conditions must stay explicitly-comparable");
        };
        assert_eq!(items[0].description.as_str(), "restated inputs");
        assert_eq!(
            items[0].kind.as_ref().map(EvidenceText::as_str),
            Some("environment")
        );
        assert_eq!(
            comparability_justification.as_str(),
            "same inputs on a new host"
        );
        assert_eq!(pc.observed_result.summary.as_str(), "post-change summary");
        assert_eq!(pc.contract_links[1].facet, Facet::ObservableIo);
        assert_eq!(pc.contract_links[1].assertion_id.as_str(), "io.mapping");

        let e = &r.evidence()[0];
        assert_eq!(e.kind, EvidenceKind::PublicContractSnapshot);
        assert_eq!(e.observed_scope.as_str(), "scope");
        assert_eq!(e.covers[0].as_str(), "api.exports");
        assert_eq!(e.limitations[0].as_str(), "limit");
        assert_eq!(
            e.applicability_justification
                .as_ref()
                .map(EvidenceText::as_str),
            Some("the api is public")
        );

        let GapInput::Assertion {
            description,
            assertion_ids,
        } = &r.gaps()[0]
        else {
            panic!("the gap must stay assertion-scoped");
        };
        assert_eq!(description.as_str(), "io mapping not re-run");
        assert_eq!(assertion_ids[0].as_str(), "io.mapping");

        let v = r.verdict();
        assert!(v.per_assertion[0].state.is_verified());
        let VerdictOutcome::Unverified { reason } = &v.per_assertion[1].state else {
            panic!("io.mapping must stay UNVERIFIED");
        };
        assert_eq!(reason.as_str(), "io not re-observed");
        let VerdictOutcome::Unverified { reason } = r.overall() else {
            panic!("overall must stay UNVERIFIED");
        };
        assert_eq!(reason.as_str(), "one assertion open");
        assert_eq!(
            r.verified_assertions()
                .iter()
                .map(AssertionId::as_str)
                .collect::<Vec<_>>(),
            vec!["api.exports"]
        );
    }
}
