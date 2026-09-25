//! Closed transport DTOs for one execution-run record
//! (`registries/operating-model/execution-state.schema.json`) and their
//! conversion into [`meridian_core::run_contracts::ExecutionRunInput`].
//! Parsed only after the record passed the schema gate; every object is
//! `#[serde(deny_unknown_fields)]` and every closed string is parsed by
//! the core pool that owns it.

use meridian_core::run_contracts::{
    Blocker, ExecutionRunInput, ExecutionRunState, LifecycleStage, StageSkip, Transition,
    WorkStatus,
};
use serde::Deserialize;

use crate::operating_model::run_contract_boundary::envelope::{
    actor, closed, optional_closed, optional_text, portable, portable_list, record_envelope,
    run_state_scope, scope_revision, semantic_id, sequence, text, AuthorityDto, Converted,
    EnvelopeParts, OriginDto, RunStateScopeDto,
};

pub(crate) const RECORD_TYPE: &str = "execution-run";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ExecutionRunDto {
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
    task_specification_ref: String,
    lifecycle_stage: String,
    work_status: String,
    scope_revision: u64,
    current_actor: String,
    resolved_norms: Vec<String>,
    completed_checks: Vec<String>,
    blockers: Vec<BlockerDto>,
    next_action: Option<String>,
    next_gate: Option<String>,
    transition_history: Vec<TransitionDto>,
}

/// One structured blocker — shared with the context-manifest DTO.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BlockerDto {
    id: String,
    description: String,
    resumption_condition: String,
}

impl BlockerDto {
    pub(crate) fn into_blocker(self) -> Converted<Blocker> {
        Ok(Blocker {
            id: semantic_id("blockers[].id", self.id)?,
            description: text("blockers[].description", self.description)?,
            resumption_condition: text(
                "blockers[].resumption_condition",
                self.resumption_condition,
            )?,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StageSkipDto {
    stage: String,
    rationale: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransitionDto {
    sequence: u64,
    #[serde(default)]
    from_stage: Option<String>,
    to_stage: String,
    #[serde(default)]
    from_status: Option<String>,
    to_status: String,
    scope_revision: u64,
    reason: String,
    #[serde(default)]
    skipped_stages: Vec<StageSkipDto>,
    #[serde(default)]
    backward_rationale: Option<String>,
    #[serde(default)]
    reopen_rationale: Option<String>,
}

impl TransitionDto {
    fn into_transition(self) -> Converted<Transition> {
        Ok(Transition {
            sequence: sequence("transition.sequence", self.sequence)?,
            from_stage: optional_closed(
                "transition.from_stage",
                self.from_stage,
                LifecycleStage::parse,
            )?,
            to_stage: closed("transition.to_stage", &self.to_stage, LifecycleStage::parse)?,
            from_status: optional_closed(
                "transition.from_status",
                self.from_status,
                WorkStatus::parse,
            )?,
            to_status: closed("transition.to_status", &self.to_status, WorkStatus::parse)?,
            scope_revision: scope_revision("transition.scope_revision", self.scope_revision)?,
            reason: text("transition.reason", self.reason)?,
            skipped_stages: self
                .skipped_stages
                .into_iter()
                .map(|s| {
                    Ok(StageSkip {
                        stage: closed("skipped_stages[].stage", &s.stage, LifecycleStage::parse)?,
                        rationale: text("skipped_stages[].rationale", s.rationale)?,
                    })
                })
                .collect::<Converted<_>>()?,
            backward_rationale: optional_text(
                "transition.backward_rationale",
                self.backward_rationale,
            )?,
            reopen_rationale: optional_text("transition.reopen_rationale", self.reopen_rationale)?,
        })
    }
}

impl ExecutionRunDto {
    pub(super) fn into_input(self) -> Converted<ExecutionRunInput> {
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
        let state = ExecutionRunState {
            task_specification_ref: portable("task_specification_ref", p.task_specification_ref)?,
            lifecycle_stage: closed("lifecycle_stage", &p.lifecycle_stage, LifecycleStage::parse)?,
            work_status: closed("work_status", &p.work_status, WorkStatus::parse)?,
            scope_revision: scope_revision("scope_revision", p.scope_revision)?,
            current_actor: actor("current_actor", p.current_actor)?,
            resolved_norms: portable_list("resolved_norms", p.resolved_norms)?,
            completed_checks: portable_list("completed_checks", p.completed_checks)?,
            blockers: p
                .blockers
                .into_iter()
                .map(BlockerDto::into_blocker)
                .collect::<Converted<_>>()?,
            next_action: optional_text("next_action", p.next_action)?,
            next_gate: optional_text("next_gate", p.next_gate)?,
            transition_history: p
                .transition_history
                .into_iter()
                .map(TransitionDto::into_transition)
                .collect::<Converted<_>>()?,
        };
        Ok(ExecutionRunInput {
            envelope,
            scope: run_state_scope(self.scope)?,
            state,
        })
    }
}
