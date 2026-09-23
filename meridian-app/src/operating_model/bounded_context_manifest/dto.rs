//! Closed transport DTOs for one context-manifest record
//! (`registries/operating-model/context-manifest.schema.json`) and their
//! conversion into [`meridian_core::run_contracts::ContextManifestInput`].

use meridian_core::run_contracts::{
    ApplicableNorm, AuthoritativeSource, ContextManifestInput, IdentifiedItem, LifecycleStage,
    ManifestBody, PinSha256, PinnedRecordKind, PinnedRef, PinnedRefs, RunStateCheckpoint,
    WorkStatus,
};
use serde::Deserialize;

use crate::operating_model::execution_state::BlockerDto;
use crate::operating_model::run_contract_boundary::envelope::{
    closed, optional_text, portable, portable_list, record_envelope, run_state_scope,
    scope_revision, semantic_id, text, AuthorityDto, Converted, EnvelopeParts, OriginDto,
    RunStateScopeDto,
};

const RECORD_TYPE: &str = "context-manifest";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ContextManifestDto {
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
struct PinnedRefDto {
    record_type: String,
    id: String,
    #[serde(default)]
    run_id: Option<String>,
    reference: String,
    #[serde(default)]
    revision: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoritativeSourceDto {
    id: String,
    reference: String,
    purpose: String,
    mutable: bool,
    #[serde(default)]
    revision: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplicableNormDto {
    id: String,
    reference: String,
    origin: String,
    #[serde(default)]
    revision: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
    applicability_rationale: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DecisionDto {
    id: String,
    statement: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenQuestionDto {
    id: String,
    question: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GapDto {
    id: String,
    description: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckpointDto {
    execution_run_ref: PinnedRefDto,
    task_specification_ref: PinnedRefDto,
    human_control_ref: PinnedRefDto,
    scope_revision: u64,
    lifecycle_stage: String,
    work_status: String,
    next_action: Option<String>,
    next_gate: Option<String>,
    blocker_ids: Vec<String>,
    resolved_norms: Vec<String>,
    completed_checks: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PayloadDto {
    execution_run_ref: PinnedRefDto,
    task_specification_ref: PinnedRefDto,
    human_control_ref: PinnedRefDto,
    scope_revision: u64,
    next_action: Option<String>,
    next_gate: Option<String>,
    authoritative_sources: Vec<AuthoritativeSourceDto>,
    applicable_norms: Vec<ApplicableNormDto>,
    decisions: Vec<DecisionDto>,
    open_questions: Vec<OpenQuestionDto>,
    completed_actions: Vec<String>,
    completed_checks: Vec<String>,
    known_gaps: Vec<GapDto>,
    blockers: Vec<BlockerDto>,
    run_state_checkpoint: CheckpointDto,
}

fn sha256(field: &str, value: Option<String>) -> Converted<Option<PinSha256>> {
    value
        .map(|v| PinSha256::new(v).map_err(|e| format!("{field}: {e}")))
        .transpose()
}

impl PinnedRefDto {
    fn into_pin(self, field: &str) -> Converted<PinnedRef> {
        Ok(PinnedRef {
            record_type: closed(field, &self.record_type, PinnedRecordKind::parse)?,
            id: semantic_id(field, self.id)?,
            run_id: self.run_id.map(|v| semantic_id(field, v)).transpose()?,
            reference: portable(field, self.reference)?,
            revision: optional_text(field, self.revision)?,
            sha256: sha256(field, self.sha256)?,
        })
    }
}

fn pins(
    execution_run: PinnedRefDto,
    task_specification: PinnedRefDto,
    human_control: PinnedRefDto,
) -> Converted<PinnedRefs> {
    Ok(PinnedRefs {
        execution_run: execution_run.into_pin("execution_run_ref")?,
        task_specification: task_specification.into_pin("task_specification_ref")?,
        human_control: human_control.into_pin("human_control_ref")?,
    })
}

fn items(label: &str, items: Vec<(String, String)>) -> Converted<Vec<IdentifiedItem>> {
    items
        .into_iter()
        .map(|(id, statement)| {
            Ok(IdentifiedItem {
                id: semantic_id(label, id)?,
                text: text(label, statement)?,
            })
        })
        .collect()
}

impl CheckpointDto {
    fn into_checkpoint(self) -> Converted<RunStateCheckpoint> {
        Ok(RunStateCheckpoint {
            pins: pins(
                self.execution_run_ref,
                self.task_specification_ref,
                self.human_control_ref,
            )?,
            scope_revision: scope_revision(
                "run_state_checkpoint.scope_revision",
                self.scope_revision,
            )?,
            lifecycle_stage: closed(
                "run_state_checkpoint.lifecycle_stage",
                &self.lifecycle_stage,
                LifecycleStage::parse,
            )?,
            work_status: closed(
                "run_state_checkpoint.work_status",
                &self.work_status,
                WorkStatus::parse,
            )?,
            next_action: optional_text("run_state_checkpoint.next_action", self.next_action)?,
            next_gate: optional_text("run_state_checkpoint.next_gate", self.next_gate)?,
            blocker_ids: self
                .blocker_ids
                .into_iter()
                .map(|v| semantic_id("run_state_checkpoint.blocker_ids", v))
                .collect::<Converted<_>>()?,
            resolved_norms: portable_list(
                "run_state_checkpoint.resolved_norms",
                self.resolved_norms,
            )?,
            completed_checks: portable_list(
                "run_state_checkpoint.completed_checks",
                self.completed_checks,
            )?,
        })
    }
}

impl ContextManifestDto {
    pub(super) fn into_input(self) -> Converted<ContextManifestInput> {
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
        let p = self.payload;
        let body = ManifestBody {
            pins: pins(
                p.execution_run_ref,
                p.task_specification_ref,
                p.human_control_ref,
            )?,
            scope_revision: scope_revision("scope_revision", p.scope_revision)?,
            next_action: optional_text("next_action", p.next_action)?,
            next_gate: optional_text("next_gate", p.next_gate)?,
            authoritative_sources: p
                .authoritative_sources
                .into_iter()
                .map(|s| {
                    Ok(AuthoritativeSource {
                        id: semantic_id("authoritative_sources[].id", s.id)?,
                        reference: portable("authoritative_sources[].reference", s.reference)?,
                        purpose: text("authoritative_sources[].purpose", s.purpose)?,
                        mutable: s.mutable,
                        revision: optional_text("authoritative_sources[].revision", s.revision)?,
                        sha256: sha256("authoritative_sources[].sha256", s.sha256)?,
                    })
                })
                .collect::<Converted<_>>()?,
            applicable_norms: p
                .applicable_norms
                .into_iter()
                .map(|n| {
                    Ok(ApplicableNorm {
                        id: semantic_id("applicable_norms[].id", n.id)?,
                        reference: portable("applicable_norms[].reference", n.reference)?,
                        origin: text("applicable_norms[].origin", n.origin)?,
                        revision: optional_text("applicable_norms[].revision", n.revision)?,
                        sha256: sha256("applicable_norms[].sha256", n.sha256)?,
                        applicability_rationale: text(
                            "applicable_norms[].applicability_rationale",
                            n.applicability_rationale,
                        )?,
                    })
                })
                .collect::<Converted<_>>()?,
            decisions: items(
                "decisions",
                p.decisions
                    .into_iter()
                    .map(|d| (d.id, d.statement))
                    .collect(),
            )?,
            open_questions: items(
                "open_questions",
                p.open_questions
                    .into_iter()
                    .map(|q| (q.id, q.question))
                    .collect(),
            )?,
            completed_actions: portable_list("completed_actions", p.completed_actions)?,
            completed_checks: portable_list("completed_checks", p.completed_checks)?,
            known_gaps: items(
                "known_gaps",
                p.known_gaps
                    .into_iter()
                    .map(|g| (g.id, g.description))
                    .collect(),
            )?,
            blockers: p
                .blockers
                .into_iter()
                .map(BlockerDto::into_blocker)
                .collect::<Converted<_>>()?,
            checkpoint: p.run_state_checkpoint.into_checkpoint()?,
        };
        Ok(ContextManifestInput {
            envelope,
            scope: run_state_scope(self.scope)?,
            body,
        })
    }
}
