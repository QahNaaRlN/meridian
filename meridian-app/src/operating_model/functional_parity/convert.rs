//! Converts a schema-validated, transport-parsed [`super::dto`] tree into
//! [`meridian_core::functional_parity`] typed input (levels 3-4 of
//! `standards/workspace/rust-migration-quality.md` §4). Called ONLY for a
//! document that already passed the whole-document JSON Schema pass — every
//! closed-set/pattern/mutual-exclusivity field here is schema-guaranteed
//! valid, so a conversion failure here is drift between this DTO and the
//! schema, not an ordinary rejection
//! (`governance/plans/meridian-rust-migration-program-plan.md` §5.18.3
//! point 6, corrective round item 3): [`build_record`] fails the WHOLE
//! record closed rather than silently continuing with a partial value, and
//! [`super::build_document`] fails the WHOLE document closed rather than a
//! partial record set.
//!
//! Every business-meaningful field [`super::dto`] carries is read here into
//! a typed `meridian_core::functional_parity` value — corrective round item
//! 1 — and every mutually exclusive schema shape (`facet.oneOf`,
//! `gap.allOf`/`if`/`then`, `inputs_and_conditions.allOf`/`if`/`then`,
//! `assertion_verdict`/`verdict.allOf`/`if`/`then`) is resolved into the
//! matching [`meridian_core::functional_parity`] enum variant, never left
//! as a flat value plus an ignored companion field.

use meridian_core::functional_parity::{
    Assertion, AssertionId, AssertionVerdictInput, BaselineInput, Condition, ConditionId,
    ContractLinkInput, EvidenceEntryInput, EvidenceKind, EvidenceText, Facet, FacetContent,
    FacetInput, GapInput, IdentifiedCondition, ObservedResult, PostChangeConditions,
    PostChangeInput, Provenance, RecordInput, SourceStateRef, VerdictInput, VerdictOutcome,
};
use meridian_core::types::{Diagnostic, DiagnosticLevel};

use super::dto::{
    BaselineDto, ContractLinkDto, EvidenceEntryDto, FacetDto, GapDto, IdentifiedConditionDto,
    InputsAndConditionsDto, ObservedResultDto, PostChangeEvidenceDto, ProvenanceDto, RecordDto,
    StateRefDto, VerdictDto,
};

fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message).expect("message is non-empty")
}

fn try_field<T, E: core::fmt::Display>(
    problems: &mut Vec<Diagnostic>,
    at: &str,
    result: Result<T, E>,
) -> Option<T> {
    match result {
        Ok(v) => Some(v),
        Err(e) => {
            problems.push(fail(format!("{at} {e}")));
            None
        }
    }
}

fn build_text(
    problems: &mut Vec<Diagnostic>,
    at: &str,
    field: &str,
    raw: &str,
) -> Option<EvidenceText> {
    try_field(
        problems,
        at,
        EvidenceText::new(raw).map_err(|e| format!("{field}: {e}")),
    )
}

fn build_optional_text(
    problems: &mut Vec<Diagnostic>,
    at: &str,
    field: &str,
    raw: &Option<String>,
) -> Option<Option<EvidenceText>> {
    match raw {
        None => Some(None),
        Some(s) => build_text(problems, at, field, s).map(Some),
    }
}

fn build_assertion_ids(
    at: &str,
    field: &str,
    ids: &[String],
) -> Result<Vec<AssertionId>, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        match AssertionId::new(id) {
            Ok(v) => out.push(v),
            Err(e) => problems.push(fail(format!(
                "{at} {field} carries an invalid id \"{id}\": {e}"
            ))),
        }
    }
    if problems.is_empty() {
        Ok(out)
    } else {
        Err(problems)
    }
}

fn build_condition_ids(
    at: &str,
    field: &str,
    ids: &[String],
) -> Result<Vec<ConditionId>, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        match ConditionId::new(id) {
            Ok(v) => out.push(v),
            Err(e) => problems.push(fail(format!(
                "{at} {field} carries an invalid id \"{id}\": {e}"
            ))),
        }
    }
    if problems.is_empty() {
        Ok(out)
    } else {
        Err(problems)
    }
}

fn build_texts(
    at: &str,
    field: &str,
    raw: &[String],
) -> Result<Vec<EvidenceText>, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let mut out = Vec::with_capacity(raw.len());
    for (i, s) in raw.iter().enumerate() {
        match EvidenceText::new(s) {
            Ok(v) => out.push(v),
            Err(e) => problems.push(fail(format!("{at} {field}[{i}]: {e}"))),
        }
    }
    if problems.is_empty() {
        Ok(out)
    } else {
        Err(problems)
    }
}

fn build_assertion(at: &str, dto: &super::dto::AssertionDto) -> Result<Assertion, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let id = try_field(&mut problems, at, AssertionId::new(&dto.id));
    let statement = build_text(
        &mut problems,
        at,
        "preserved_contract assertion.statement",
        &dto.statement,
    );
    match (id, statement) {
        (Some(id), Some(statement)) if problems.is_empty() => Ok(Assertion { id, statement }),
        _ => Err(problems),
    }
}

/// Resolves the schema's own `facet.oneOf` — declared assertions XOR a
/// not-applicable justification — into the matching [`FacetContent`]
/// variant. Either shape being simultaneously present/absent on a
/// schema-valid document is drift, not an ordinary rejection.
fn build_facet(at: &str, facet: Facet, dto: &FacetDto) -> Result<FacetInput, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let field = format!("preserved_contract.{facet}");
    let content = match (&dto.assertions, &dto.not_applicable_justification) {
        (Some(assertions), None) => {
            let mut built = Vec::with_capacity(assertions.len());
            for a in assertions {
                match build_assertion(at, a) {
                    Ok(v) => built.push(v),
                    Err(errs) => problems.extend(errs),
                }
            }
            if problems.is_empty() {
                Some(FacetContent::Declared(built))
            } else {
                None
            }
        }
        (None, Some(justification)) => build_text(
            &mut problems,
            at,
            &format!("{field}.not_applicable_justification"),
            justification,
        )
        .map(FacetContent::NotApplicable),
        _ => {
            problems.push(fail(format!(
                "{at} {field} carries neither exactly one declared-assertions nor exactly one not-applicable-justification shape (schema/DTO drift)"
            )));
            None
        }
    };
    match content {
        Some(content) if problems.is_empty() => Ok(FacetInput { facet, content }),
        _ => Err(problems),
    }
}

fn build_source_state(
    at: &str,
    field: &str,
    dto: &StateRefDto,
) -> Result<SourceStateRef, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let identifier = build_text(
        &mut problems,
        at,
        &format!("{field}.identifier"),
        &dto.identifier,
    );
    let identifier_kind = build_text(
        &mut problems,
        at,
        &format!("{field}.identifier_kind"),
        &dto.identifier_kind,
    );
    match (identifier, identifier_kind) {
        (Some(identifier), Some(identifier_kind)) if problems.is_empty() => Ok(SourceStateRef {
            identifier,
            identifier_kind,
        }),
        _ => Err(problems),
    }
}

fn build_identified_condition(
    at: &str,
    dto: &IdentifiedConditionDto,
) -> Result<IdentifiedCondition, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let id = try_field(&mut problems, at, ConditionId::new(&dto.id));
    let description = build_text(
        &mut problems,
        at,
        "baseline.inputs_and_conditions.description",
        &dto.description,
    );
    let kind = build_optional_text(
        &mut problems,
        at,
        "baseline.inputs_and_conditions.kind",
        &dto.kind,
    );
    match (id, description, kind) {
        (Some(id), Some(description), Some(kind)) if problems.is_empty() => {
            Ok(IdentifiedCondition {
                id,
                description,
                kind,
            })
        }
        _ => Err(problems),
    }
}

fn build_provenance(at: &str, dto: &ProvenanceDto) -> Result<Provenance, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let method = build_text(&mut problems, at, "baseline.provenance.method", &dto.method);
    match method {
        Some(method) if problems.is_empty() => Ok(Provenance {
            method,
            established: dto.established,
        }),
        _ => Err(problems),
    }
}

fn build_observed_result(
    at: &str,
    field: &str,
    dto: &ObservedResultDto,
) -> Result<ObservedResult, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let summary = build_text(&mut problems, at, &format!("{field}.summary"), &dto.summary);
    let artifacts_raw = dto.artifacts.clone().unwrap_or_default();
    let artifacts = match build_texts(at, &format!("{field}.artifacts"), &artifacts_raw) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    match (summary, artifacts) {
        (Some(summary), Some(artifacts)) if problems.is_empty() => {
            Ok(ObservedResult { summary, artifacts })
        }
        _ => Err(problems),
    }
}

fn build_baseline(at: &str, dto: &BaselineDto) -> Result<BaselineInput, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let source_state = match build_source_state(at, "baseline.source_state", &dto.source_state) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    let mut conditions = Vec::with_capacity(dto.inputs_and_conditions.len());
    for c in &dto.inputs_and_conditions {
        match build_identified_condition(at, c) {
            Ok(v) => conditions.push(v),
            Err(errs) => problems.extend(errs),
        }
    }
    let provenance = match build_provenance(at, &dto.provenance) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    let observed_result =
        match build_observed_result(at, "baseline.observed_result", &dto.observed_result) {
            Ok(v) => Some(v),
            Err(errs) => {
                problems.extend(errs);
                None
            }
        };
    match (source_state, provenance, observed_result) {
        (Some(source_state), Some(provenance), Some(observed_result)) if problems.is_empty() => {
            Ok(BaselineInput {
                source_state,
                conditions,
                provenance,
                observed_result,
            })
        }
        _ => Err(problems),
    }
}

fn build_condition(at: &str, dto: &super::dto::ConditionDto) -> Result<Condition, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let description = build_text(
        &mut problems,
        at,
        "post_change_evidence.inputs_and_conditions.items.description",
        &dto.description,
    );
    let kind = build_optional_text(
        &mut problems,
        at,
        "post_change_evidence.inputs_and_conditions.items.kind",
        &dto.kind,
    );
    match (description, kind) {
        (Some(description), Some(kind)) if problems.is_empty() => {
            Ok(Condition { description, kind })
        }
        _ => Err(problems),
    }
}

/// Resolves the schema's own
/// `post_change_evidence.inputs_and_conditions.allOf`/`if`/`then` — `same`
/// XOR `explicitly-comparable` — into the matching [`PostChangeConditions`]
/// variant.
fn build_post_change_conditions(
    at: &str,
    dto: &InputsAndConditionsDto,
) -> Result<PostChangeConditions, Vec<Diagnostic>> {
    let field = "post_change_evidence.inputs_and_conditions";
    match dto.relationship.as_str() {
        "same" => {
            let ids = dto.baseline_condition_ids.clone().unwrap_or_default();
            let baseline_condition_ids =
                build_condition_ids(at, &format!("{field}.baseline_condition_ids"), &ids)?;
            Ok(PostChangeConditions::Same {
                baseline_condition_ids,
            })
        }
        "explicitly-comparable" => {
            let mut problems = Vec::new();
            let items_raw = dto.items.clone().unwrap_or_default();
            let mut items = Vec::with_capacity(items_raw.len());
            for c in &items_raw {
                match build_condition(at, c) {
                    Ok(v) => items.push(v),
                    Err(errs) => problems.extend(errs),
                }
            }
            let comparability_justification = match &dto.comparability_justification {
                Some(s) => build_text(
                    &mut problems,
                    at,
                    &format!("{field}.comparability_justification"),
                    s,
                ),
                None => {
                    problems.push(fail(format!(
                        "{at} {field}.comparability_justification is required when relationship is \"explicitly-comparable\" (schema/DTO drift)"
                    )));
                    None
                }
            };
            match comparability_justification {
                Some(comparability_justification) if problems.is_empty() => {
                    Ok(PostChangeConditions::ExplicitlyComparable {
                        items,
                        comparability_justification,
                    })
                }
                _ => Err(problems),
            }
        }
        other => Err(vec![fail(format!(
            "{at} {field}.relationship \"{other}\" is not one of the two closed values"
        ))]),
    }
}

fn build_contract_link(
    at: &str,
    dto: &ContractLinkDto,
) -> Result<ContractLinkInput, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let facet = match Facet::parse(&dto.facet) {
        Some(f) => Some(f),
        None => {
            problems.push(fail(format!(
                "{at} post_change_evidence.contract_links facet \"{}\" is not one of the four closed facets",
                dto.facet
            )));
            None
        }
    };
    let assertion_id = try_field(&mut problems, at, AssertionId::new(&dto.assertion_id));
    match (facet, assertion_id) {
        (Some(facet), Some(assertion_id)) if problems.is_empty() => Ok(ContractLinkInput {
            facet,
            assertion_id,
        }),
        _ => Err(problems),
    }
}

fn build_post_change(
    at: &str,
    dto: &PostChangeEvidenceDto,
) -> Result<PostChangeInput, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let source_state =
        match build_source_state(at, "post_change_evidence.source_state", &dto.source_state) {
            Ok(v) => Some(v),
            Err(errs) => {
                problems.extend(errs);
                None
            }
        };
    let conditions = match build_post_change_conditions(at, &dto.inputs_and_conditions) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    let observed_result = match build_observed_result(
        at,
        "post_change_evidence.observed_result",
        &dto.observed_result,
    ) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    let mut contract_links = Vec::with_capacity(dto.contract_links.len());
    for link in &dto.contract_links {
        match build_contract_link(at, link) {
            Ok(v) => contract_links.push(v),
            Err(errs) => problems.extend(errs),
        }
    }
    match (source_state, conditions, observed_result) {
        (Some(source_state), Some(conditions), Some(observed_result)) if problems.is_empty() => {
            Ok(PostChangeInput {
                source_state,
                conditions,
                observed_result,
                contract_links,
            })
        }
        _ => Err(problems),
    }
}

fn build_evidence_entry(
    at: &str,
    dto: &EvidenceEntryDto,
) -> Result<EvidenceEntryInput, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let kind = match EvidenceKind::parse(&dto.kind) {
        Some(k) => Some(k),
        None => {
            problems.push(fail(format!(
                "{at} evidence kind \"{}\" is not one of the six closed kinds",
                dto.kind
            )));
            None
        }
    };
    let observed_scope = build_text(
        &mut problems,
        at,
        "evidence.observed_scope",
        &dto.observed_scope,
    );
    let covers = match build_assertion_ids(at, "evidence.covers", &dto.covers) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    let limitations = match build_texts(at, "evidence.limitations", &dto.limitations) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    let applicability_justification = build_optional_text(
        &mut problems,
        at,
        "evidence.applicability_justification",
        &dto.applicability_justification,
    );
    match (
        kind,
        observed_scope,
        covers,
        limitations,
        applicability_justification,
    ) {
        (
            Some(kind),
            Some(observed_scope),
            Some(covers),
            Some(limitations),
            Some(applicability_justification),
        ) if problems.is_empty() => Ok(EvidenceEntryInput {
            kind,
            observed_scope,
            covers,
            limitations,
            applicability_justification,
        }),
        _ => Err(problems),
    }
}

/// Resolves the schema's own `gap.allOf`/`if`/`then` — `record`-scoped XOR
/// `assertion`-scoped — into the matching [`GapInput`] variant.
fn build_gap(at: &str, dto: &GapDto) -> Result<GapInput, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let description = build_text(&mut problems, at, "gaps.description", &dto.description);
    match dto.scope.as_str() {
        "record" => {
            if dto.assertion_ids.is_some() {
                problems.push(fail(format!(
                    "{at} a record-scoped gap must not carry assertion_ids (schema/DTO drift)"
                )));
            }
            match description {
                Some(description) if problems.is_empty() => Ok(GapInput::Record { description }),
                _ => Err(problems),
            }
        }
        "assertion" => {
            let ids = dto.assertion_ids.clone().unwrap_or_default();
            let assertion_ids = match build_assertion_ids(at, "gaps.assertion_ids", &ids) {
                Ok(v) => Some(v),
                Err(errs) => {
                    problems.extend(errs);
                    None
                }
            };
            match (description, assertion_ids) {
                (Some(description), Some(assertion_ids)) if problems.is_empty() => {
                    Ok(GapInput::Assertion {
                        description,
                        assertion_ids,
                    })
                }
                _ => Err(problems),
            }
        }
        other => Err(vec![fail(format!(
            "{at} gap scope \"{other}\" is not one of the two closed values"
        ))]),
    }
}

/// Resolves the schema's own `assertion_verdict.allOf`/`if`/`then` —
/// `VERIFIED` XOR `UNVERIFIED` with its required `unverified_reason` — into
/// the matching [`VerdictOutcome`] variant.
fn build_verdict_outcome(
    at: &str,
    field: &str,
    state: &str,
    reason: &Option<String>,
) -> Result<VerdictOutcome, Vec<Diagnostic>> {
    match state {
        "VERIFIED" => {
            if reason.is_some() {
                return Err(vec![fail(format!(
                    "{at} {field} is VERIFIED but also carries a reason meant only for UNVERIFIED (schema/DTO drift)"
                ))]);
            }
            Ok(VerdictOutcome::Verified)
        }
        "UNVERIFIED" => {
            let mut problems = Vec::new();
            let reason = match reason {
                Some(s) => build_text(&mut problems, at, &format!("{field} reason"), s),
                None => {
                    problems.push(fail(format!(
                        "{at} {field} is UNVERIFIED but carries no reason (schema/DTO drift)"
                    )));
                    None
                }
            };
            match reason {
                Some(reason) if problems.is_empty() => Ok(VerdictOutcome::Unverified { reason }),
                _ => Err(problems),
            }
        }
        other => Err(vec![fail(format!(
            "{at} {field} \"{other}\" is not one of the two closed values"
        ))]),
    }
}

fn build_assertion_verdict(
    at: &str,
    dto: &super::dto::AssertionVerdictDto,
) -> Result<AssertionVerdictInput, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let assertion_id = try_field(&mut problems, at, AssertionId::new(&dto.assertion_id));
    let state = match build_verdict_outcome(
        at,
        "verdict.per_assertion.state",
        &dto.state,
        &dto.unverified_reason,
    ) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    match (assertion_id, state) {
        (Some(assertion_id), Some(state)) if problems.is_empty() => Ok(AssertionVerdictInput {
            assertion_id,
            state,
        }),
        _ => Err(problems),
    }
}

fn build_verdict(at: &str, dto: &VerdictDto) -> Result<VerdictInput, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let mut per_assertion = Vec::with_capacity(dto.per_assertion.len());
    for entry in &dto.per_assertion {
        match build_assertion_verdict(at, entry) {
            Ok(v) => per_assertion.push(v),
            Err(errs) => problems.extend(errs),
        }
    }
    let overall = match build_verdict_outcome(
        at,
        "verdict.overall",
        &dto.overall,
        &dto.overall_unverified_reason,
    ) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    match overall {
        Some(overall) if problems.is_empty() => Ok(VerdictInput {
            per_assertion,
            overall,
        }),
        _ => Err(problems),
    }
}

/// Builds one [`RecordInput`] from its transport DTO, accumulating every
/// independent defect before deciding success.
pub(crate) fn build_record(dto: &RecordDto) -> Result<RecordInput, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let at = format!("functional-parity record \"{}\"", dto.work_item);

    let work_item = build_text(&mut problems, &at, "work_item", &dto.work_item);
    let recorded_at = build_text(&mut problems, &at, "recorded_at", &dto.recorded_at);

    let pc = &dto.preserved_contract;
    let public_api = match build_facet(&at, Facet::PublicApi, &pc.public_api) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    let observable_io = match build_facet(&at, Facet::ObservableIo, &pc.observable_io) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    let side_effects_and_interactions = match build_facet(
        &at,
        Facet::SideEffectsAndInteractions,
        &pc.side_effects_and_interactions,
    ) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    let user_visible_behavior =
        match build_facet(&at, Facet::UserVisibleBehavior, &pc.user_visible_behavior) {
            Ok(v) => Some(v),
            Err(errs) => {
                problems.extend(errs);
                None
            }
        };

    let baseline = match build_baseline(&at, &dto.baseline) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    let post_change = match build_post_change(&at, &dto.post_change_evidence) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };

    let mut evidence = Vec::with_capacity(dto.evidence.len());
    for entry in &dto.evidence {
        match build_evidence_entry(&at, entry) {
            Ok(v) => evidence.push(v),
            Err(errs) => problems.extend(errs),
        }
    }

    let mut gaps = Vec::with_capacity(dto.gaps.len());
    for gap in &dto.gaps {
        match build_gap(&at, gap) {
            Ok(v) => gaps.push(v),
            Err(errs) => problems.extend(errs),
        }
    }

    let verdict = match build_verdict(&at, &dto.verdict) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };

    let (
        Some(work_item),
        Some(recorded_at),
        Some(public_api),
        Some(observable_io),
        Some(side_effects_and_interactions),
        Some(user_visible_behavior),
        Some(baseline),
        Some(post_change),
        Some(verdict),
    ) = (
        work_item,
        recorded_at,
        public_api,
        observable_io,
        side_effects_and_interactions,
        user_visible_behavior,
        baseline,
        post_change,
        verdict,
    )
    else {
        return Err(problems);
    };
    if !problems.is_empty() {
        return Err(problems);
    }

    Ok(RecordInput {
        work_item,
        facets: [
            public_api,
            observable_io,
            side_effects_and_interactions,
            user_visible_behavior,
        ],
        baseline,
        post_change,
        evidence,
        gaps,
        verdict,
        recorded_at,
    })
}
