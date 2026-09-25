//! Closed transport DTOs for one evidence-and-handoff record
//! (`registries/operating-model/evidence-and-handoff.schema.json`) and
//! their conversion into [`meridian_core::evidence::handoff::input::HandoffInput`].
//! Every object is `#[serde(deny_unknown_fields)]`; a conversion error is
//! drift between this DTO and the schema the record already passed.

use meridian_core::evidence::handoff::input::{
    AssertionInput, BlockerInput, ChangeKind, ChangedPathInput, CheckStatus, ClaimedResultInput,
    CoverageStatus, CriterionInput, DeviationInput, DeviationSeverity, ExternalEffectInput,
    HandoffBody, HandoffEvidenceInput, HandoffInput, HandoffPins, MandatoryCheckInput,
    NextStepInput, OpenGapInput, OutcomeInput, OutcomeStatus, OwnerDecisionInput,
    RepositoryStateInput, WorktreeInput, WorktreeState,
};
use meridian_core::evidence::{AssertionStatus, ClaimedResultStatus, EvidenceKind, PinnedEvidence};
use serde::Deserialize;

use crate::operating_model::run_contract_boundary::envelope::{
    closed, optional_text, portable, record_envelope, run_state_scope, semantic_id, sha256, text,
    AuthorityDto, Converted, EnvelopeParts, OriginDto, PinnedRefDto, RunStateScopeDto,
};

const RECORD_TYPE: &str = "evidence-and-handoff";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HandoffDto {
    #[serde(rename = "$schema")]
    declared_schema: String,
    schema_version: u64,
    id: String,
    title: String,
    record_type: String,
    scope: RunStateScopeDto,
    origin: OriginDto,
    authority: AuthorityDto,
    payload: PayloadDto,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PayloadDto {
    execution_run_ref: PinnedRefDto,
    task_specification_ref: PinnedRefDto,
    human_control_ref: PinnedRefDto,
    context_manifest_ref: PinnedRefDto,
    outcome: OutcomeDto,
    claimed_results: Vec<ClaimedResultDto>,
    verifiable_assertions: Vec<AssertionDto>,
    evidence: Vec<EvidenceDto>,
    mandatory_checks: Vec<MandatoryCheckDto>,
    acceptance_criteria: Vec<CriterionDto>,
    source_state: Vec<RepositoryStateDto>,
    result_state: Vec<RepositoryStateDto>,
    changed_paths: Vec<ChangedPathDto>,
    external_effects: Vec<ExternalEffectDto>,
    deviations: Vec<DeviationDto>,
    open_gaps: Vec<OpenGapDto>,
    required_owner_decisions: Vec<OwnerDecisionDto>,
    blockers: Vec<BlockerDto>,
    worktree_disposition: WorktreeDto,
    next_step: NextStepDto,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OutcomeDto {
    statement: String,
    status: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimedResultDto {
    id: String,
    statement: String,
    status: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AssertionDto {
    id: String,
    statement: String,
    claimed_result_id: String,
    status: String,
    #[serde(default)]
    unverified_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceDto {
    id: String,
    kind: String,
    reference: String,
    #[serde(default)]
    revision: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
    summary: String,
    covers: Vec<String>,
    limitations: Vec<String>,
    #[serde(default)]
    specialised_contract: Option<String>,
    #[serde(default)]
    recorded_verdict: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MandatoryCheckDto {
    id: String,
    name: String,
    check_ref: String,
    status: String,
    #[serde(default)]
    result_evidence_id: Option<String>,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CriterionDto {
    id: String,
    status: String,
    addressed_by: Vec<String>,
    #[serde(default)]
    uncovered_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepositoryStateDto {
    repository_ref: String,
    #[serde(default)]
    revision: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChangedPathDto {
    id: String,
    repository_ref: String,
    path: String,
    change_kind: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExternalEffectDto {
    id: String,
    description: String,
    reversible: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeviationDto {
    id: String,
    description: String,
    severity: String,
    disposition: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenGapDto {
    id: String,
    description: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerDecisionDto {
    id: String,
    question: String,
    blocking: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BlockerDto {
    id: String,
    description: String,
    resumption_condition: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorktreeDto {
    state: String,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    responsible: Option<String>,
    #[serde(default)]
    cleanup_condition: Option<String>,
}

/// Each axis is required by the schema and may be `null`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NextStepDto {
    action: Option<String>,
    gate: Option<String>,
    actor_ref: Option<String>,
}

fn list<D, T>(items: Vec<D>, convert: impl Fn(D) -> Converted<T>) -> Converted<Vec<T>> {
    items.into_iter().map(convert).collect()
}

fn ids(field: &str, values: Vec<String>) -> Converted<Vec<meridian_core::types::SemanticId>> {
    values.into_iter().map(|v| semantic_id(field, v)).collect()
}

impl HandoffDto {
    pub(super) fn into_input(self) -> Converted<HandoffInput> {
        let envelope = record_envelope(
            EnvelopeParts {
                declared_schema: self.declared_schema,
                schema_version: self.schema_version,
                id: self.id,
                title: self.title,
                record_type: self.record_type,
                origin: self.origin,
                authority: self.authority,
            },
            RECORD_TYPE,
        )?;
        Ok(HandoffInput {
            envelope,
            scope: run_state_scope(self.scope)?,
            body: self.payload.into_body()?,
        })
    }
}

impl PayloadDto {
    fn into_body(self) -> Converted<HandoffBody> {
        Ok(HandoffBody {
            pins: HandoffPins {
                execution_run: self.execution_run_ref.into_pin("execution_run_ref")?,
                task_specification: self
                    .task_specification_ref
                    .into_pin("task_specification_ref")?,
                human_control: self.human_control_ref.into_pin("human_control_ref")?,
                context_manifest: self.context_manifest_ref.into_pin("context_manifest_ref")?,
            },
            outcome: OutcomeInput {
                statement: text("outcome.statement", self.outcome.statement)?,
                status: closed("outcome.status", &self.outcome.status, OutcomeStatus::parse)?,
            },
            claimed_results: list(self.claimed_results, |c| {
                Ok(ClaimedResultInput {
                    id: semantic_id("claimed_results.id", c.id)?,
                    statement: text("claimed_results.statement", c.statement)?,
                    status: closed(
                        "claimed_results.status",
                        &c.status,
                        ClaimedResultStatus::parse,
                    )?,
                })
            })?,
            assertions: list(self.verifiable_assertions, |a| {
                Ok(AssertionInput {
                    id: semantic_id("verifiable_assertions.id", a.id)?,
                    statement: text("verifiable_assertions.statement", a.statement)?,
                    claimed_result_id: semantic_id(
                        "verifiable_assertions.claimed_result_id",
                        a.claimed_result_id,
                    )?,
                    status: closed(
                        "verifiable_assertions.status",
                        &a.status,
                        AssertionStatus::parse,
                    )?,
                    unverified_reason: optional_text(
                        "verifiable_assertions.unverified_reason",
                        a.unverified_reason,
                    )?,
                })
            })?,
            evidence: list(self.evidence, |e| {
                Ok(HandoffEvidenceInput {
                    evidence: PinnedEvidence {
                        id: semantic_id("evidence.id", e.id)?,
                        kind: closed("evidence.kind", &e.kind, EvidenceKind::parse)?,
                        reference: portable("evidence.reference", e.reference)?,
                        revision: optional_text("evidence.revision", e.revision)?,
                        sha256: sha256("evidence.sha256", e.sha256)?,
                        summary: text("evidence.summary", e.summary)?,
                    },
                    covers: ids("evidence.covers", e.covers)?,
                    limitations: list(e.limitations, |l| text("evidence.limitations", l))?,
                    specialised_contract: optional_text(
                        "evidence.specialised_contract",
                        e.specialised_contract,
                    )?,
                    recorded_verdict: optional_text(
                        "evidence.recorded_verdict",
                        e.recorded_verdict,
                    )?,
                })
            })?,
            mandatory_checks: list(self.mandatory_checks, |m| {
                Ok(MandatoryCheckInput {
                    id: semantic_id("mandatory_checks.id", m.id)?,
                    name: text("mandatory_checks.name", m.name)?,
                    check_ref: portable("mandatory_checks.check_ref", m.check_ref)?,
                    status: closed("mandatory_checks.status", &m.status, CheckStatus::parse)?,
                    result_evidence_id: m
                        .result_evidence_id
                        .map(|v| semantic_id("mandatory_checks.result_evidence_id", v))
                        .transpose()?,
                    reason: optional_text("mandatory_checks.reason", m.reason)?,
                })
            })?,
            acceptance_criteria: list(self.acceptance_criteria, |c| {
                Ok(CriterionInput {
                    id: semantic_id("acceptance_criteria.id", c.id)?,
                    status: closed(
                        "acceptance_criteria.status",
                        &c.status,
                        CoverageStatus::parse,
                    )?,
                    addressed_by: ids("acceptance_criteria.addressed_by", c.addressed_by)?,
                    uncovered_reason: optional_text(
                        "acceptance_criteria.uncovered_reason",
                        c.uncovered_reason,
                    )?,
                })
            })?,
            source_state: list(self.source_state, |s| repository_state("source_state", s))?,
            result_state: list(self.result_state, |s| repository_state("result_state", s))?,
            changed_paths: list(self.changed_paths, |c| {
                Ok(ChangedPathInput {
                    id: semantic_id("changed_paths.id", c.id)?,
                    repository_ref: portable("changed_paths.repository_ref", c.repository_ref)?,
                    path: portable("changed_paths.path", c.path)?,
                    change_kind: closed(
                        "changed_paths.change_kind",
                        &c.change_kind,
                        ChangeKind::parse,
                    )?,
                })
            })?,
            external_effects: list(self.external_effects, |e| {
                Ok(ExternalEffectInput {
                    id: semantic_id("external_effects.id", e.id)?,
                    description: text("external_effects.description", e.description)?,
                    reversible: e.reversible,
                })
            })?,
            deviations: list(self.deviations, |d| {
                Ok(DeviationInput {
                    id: semantic_id("deviations.id", d.id)?,
                    description: text("deviations.description", d.description)?,
                    severity: closed("deviations.severity", &d.severity, DeviationSeverity::parse)?,
                    disposition: text("deviations.disposition", d.disposition)?,
                })
            })?,
            open_gaps: list(self.open_gaps, |g| {
                Ok(OpenGapInput {
                    id: semantic_id("open_gaps.id", g.id)?,
                    description: text("open_gaps.description", g.description)?,
                })
            })?,
            owner_decisions: list(self.required_owner_decisions, |o| {
                Ok(OwnerDecisionInput {
                    id: semantic_id("required_owner_decisions.id", o.id)?,
                    question: text("required_owner_decisions.question", o.question)?,
                    blocking: o.blocking,
                })
            })?,
            blockers: list(self.blockers, |b| {
                Ok(BlockerInput {
                    id: semantic_id("blockers.id", b.id)?,
                    description: text("blockers.description", b.description)?,
                    resumption_condition: text(
                        "blockers.resumption_condition",
                        b.resumption_condition,
                    )?,
                })
            })?,
            worktree: WorktreeInput {
                state: closed(
                    "worktree_disposition.state",
                    &self.worktree_disposition.state,
                    WorktreeState::parse,
                )?,
                reason: optional_text(
                    "worktree_disposition.reason",
                    self.worktree_disposition.reason,
                )?,
                responsible: optional_text(
                    "worktree_disposition.responsible",
                    self.worktree_disposition.responsible,
                )?,
                cleanup_condition: optional_text(
                    "worktree_disposition.cleanup_condition",
                    self.worktree_disposition.cleanup_condition,
                )?,
            },
            next_step: NextStepInput {
                action: optional_text("next_step.action", self.next_step.action)?,
                gate: optional_text("next_step.gate", self.next_step.gate)?,
                actor_ref: optional_text("next_step.actor_ref", self.next_step.actor_ref)?,
            },
        })
    }
}

fn repository_state(field: &str, s: RepositoryStateDto) -> Converted<RepositoryStateInput> {
    Ok(RepositoryStateInput {
        repository_ref: portable(field, s.repository_ref)?,
        revision: optional_text(field, s.revision)?,
        sha256: sha256(field, s.sha256)?,
    })
}
