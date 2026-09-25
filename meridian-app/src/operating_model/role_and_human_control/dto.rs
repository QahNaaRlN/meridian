//! Closed transport DTOs for the two role-and-human-control records —
//! the built-in role catalogue (`role-registry.schema.json`) and one run's
//! human-control record (`human-control.schema.json`) — and their
//! conversion into `meridian_core::run_contracts` inputs.

use meridian_core::run_contracts::{
    CommunicationMode, ControlSwitch, HumanControlInput, HumanControlState, ReviewIndependence,
    RoleAssignment, RoleDefinition, RoleRegistryInput, Supervision, SupervisionMode, UniversalRole,
    HUMAN_AUTHORITY_POSTURE,
};
use serde::Deserialize;

use crate::operating_model::run_contract_boundary::envelope::{
    actor, built_in_methodology_scope, closed, optional_closed, portable, record_envelope,
    run_state_scope, sequence, text, AuthorityDto, Converted, EnvelopeParts, OriginDto,
    RunStateScopeDto,
};

const REGISTRY_RECORD_TYPE: &str = "role-registry";
const CONTROL_RECORD_TYPE: &str = "run-human-control";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BuiltInScopeDto {
    #[serde(rename = "type")]
    scope_type: String,
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RoleRegistryDto {
    #[serde(rename = "$schema")]
    declared_schema: String,
    schema_version: u64,
    id: String,
    title: String,
    record_type: String,
    scope: BuiltInScopeDto,
    origin: OriginDto,
    authority: AuthorityDto,
    payload: RegistryPayloadDto,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegistryPayloadDto {
    roles: Vec<RoleDto>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RoleDto {
    id: String,
    title: String,
    summary: String,
    responsibilities: Vec<String>,
}

impl RoleRegistryDto {
    pub(super) fn into_input(self) -> Converted<RoleRegistryInput> {
        built_in_methodology_scope(&self.scope.scope_type, &self.scope.id)?;
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
            REGISTRY_RECORD_TYPE,
        )?;
        let roles = self
            .payload
            .roles
            .into_iter()
            .map(|r| {
                Ok(RoleDefinition {
                    id: closed("roles[].id", &r.id, UniversalRole::parse)?,
                    title: text("roles[].title", r.title)?,
                    summary: text("roles[].summary", r.summary)?,
                    responsibilities: r
                        .responsibilities
                        .into_iter()
                        .map(|d| text("roles[].responsibilities[]", d))
                        .collect::<Converted<_>>()?,
                })
            })
            .collect::<Converted<_>>()?;
        Ok(RoleRegistryInput { envelope, roles })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HumanControlDto {
    #[serde(rename = "$schema")]
    declared_schema: String,
    schema_version: u64,
    id: String,
    title: String,
    record_type: String,
    scope: RunStateScopeDto,
    origin: OriginDto,
    authority: AuthorityDto,
    payload: ControlPayloadDto,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HumanAuthorityDto {
    posture: String,
    holder: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HitlDto {
    required_human_action: String,
    gate: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HotlDto {
    autonomy_bounds: Vec<String>,
    intervention: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RoleAssignmentDto {
    actor: String,
    roles: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewIndependenceDto {
    required: bool,
    #[serde(default)]
    reviewer_actor: Option<String>,
    #[serde(default)]
    reviewed_actor: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlSwitchDto {
    sequence: u64,
    from_supervision_mode: Option<String>,
    to_supervision_mode: String,
    from_communication_mode: Option<String>,
    to_communication_mode: String,
    from_acting_actor: Option<String>,
    to_acting_actor: String,
    from_role_assignments: Option<Vec<RoleAssignmentDto>>,
    to_role_assignments: Vec<RoleAssignmentDto>,
    checkpoint_ref: String,
    reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlPayloadDto {
    execution_run_ref: String,
    human_authority: HumanAuthorityDto,
    supervision_mode: String,
    #[serde(default)]
    hitl: Option<HitlDto>,
    #[serde(default)]
    hotl: Option<HotlDto>,
    acting_actor: String,
    role_assignments: Vec<RoleAssignmentDto>,
    #[serde(default)]
    review_independence: Option<ReviewIndependenceDto>,
    communication_mode: String,
    switch_history: Vec<ControlSwitchDto>,
}

fn assignments(field: &str, dtos: Vec<RoleAssignmentDto>) -> Converted<Vec<RoleAssignment>> {
    dtos.into_iter()
        .map(|a| {
            let roles = a
                .roles
                .iter()
                .map(|r| closed(field, r, UniversalRole::parse))
                .collect::<Converted<Vec<_>>>()?;
            RoleAssignment::new(actor(field, a.actor)?, roles).map_err(|e| format!("{field}: {e}"))
        })
        .collect()
}

fn supervision(mode: &str, hitl: Option<HitlDto>, hotl: Option<HotlDto>) -> Converted<Supervision> {
    match (
        closed("supervision_mode", mode, SupervisionMode::parse)?,
        hitl,
        hotl,
    ) {
        (SupervisionMode::HumanInTheLoop, Some(h), None) => Ok(Supervision::HumanInTheLoop {
            required_human_action: text("hitl.required_human_action", h.required_human_action)?,
            gate: text("hitl.gate", h.gate)?,
        }),
        (SupervisionMode::HumanOnTheLoop, None, Some(h)) => Ok(Supervision::HumanOnTheLoop {
            autonomy_bounds: h
                .autonomy_bounds
                .into_iter()
                .map(|b| text("hotl.autonomy_bounds[]", b))
                .collect::<Converted<_>>()?,
            intervention: text("hotl.intervention", h.intervention)?,
        }),
        (mode, _, _) => Err(format!(
            "supervision_mode \"{mode}\" does not carry exactly its own hitl/hotl block"
        )),
    }
}

fn review(dto: ReviewIndependenceDto) -> Converted<ReviewIndependence> {
    match (dto.required, dto.reviewer_actor, dto.reviewed_actor) {
        (true, Some(reviewer), Some(reviewed)) => Ok(ReviewIndependence::Required {
            reviewer_actor: actor("review_independence.reviewer_actor", reviewer)?,
            reviewed_actor: actor("review_independence.reviewed_actor", reviewed)?,
        }),
        (false, None, None) => Ok(ReviewIndependence::NotRequired),
        _ => Err("review_independence: the actors do not match `required`".to_string()),
    }
}

impl ControlSwitchDto {
    fn into_switch(self) -> Converted<ControlSwitch> {
        Ok(ControlSwitch {
            sequence: sequence("switch.sequence", self.sequence)?,
            from_supervision_mode: optional_closed(
                "switch.from_supervision_mode",
                self.from_supervision_mode,
                SupervisionMode::parse,
            )?,
            to_supervision_mode: closed(
                "switch.to_supervision_mode",
                &self.to_supervision_mode,
                SupervisionMode::parse,
            )?,
            from_communication_mode: optional_closed(
                "switch.from_communication_mode",
                self.from_communication_mode,
                CommunicationMode::parse,
            )?,
            to_communication_mode: closed(
                "switch.to_communication_mode",
                &self.to_communication_mode,
                CommunicationMode::parse,
            )?,
            from_acting_actor: self
                .from_acting_actor
                .map(|a| actor("switch.from_acting_actor", a))
                .transpose()?,
            to_acting_actor: actor("switch.to_acting_actor", self.to_acting_actor)?,
            from_role_assignments: self
                .from_role_assignments
                .map(|a| assignments("switch.from_role_assignments", a))
                .transpose()?,
            to_role_assignments: assignments(
                "switch.to_role_assignments",
                self.to_role_assignments,
            )?,
            checkpoint_ref: text("switch.checkpoint_ref", self.checkpoint_ref)?,
            reason: text("switch.reason", self.reason)?,
        })
    }
}

impl HumanControlDto {
    pub(super) fn into_input(self) -> Converted<HumanControlInput> {
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
            CONTROL_RECORD_TYPE,
        )?;
        let p = self.payload;
        if p.human_authority.posture != HUMAN_AUTHORITY_POSTURE {
            return Err(format!(
                "human_authority.posture: \"{}\" is not \"{HUMAN_AUTHORITY_POSTURE}\"",
                p.human_authority.posture
            ));
        }
        let control = HumanControlState {
            execution_run_ref: portable("execution_run_ref", p.execution_run_ref)?,
            authority_holder: actor("human_authority.holder", p.human_authority.holder)?,
            supervision: supervision(&p.supervision_mode, p.hitl, p.hotl)?,
            acting_actor: actor("acting_actor", p.acting_actor)?,
            role_assignments: assignments("role_assignments", p.role_assignments)?,
            review_independence: p.review_independence.map(review).transpose()?,
            communication_mode: closed(
                "communication_mode",
                &p.communication_mode,
                CommunicationMode::parse,
            )?,
            switch_history: p
                .switch_history
                .into_iter()
                .map(ControlSwitchDto::into_switch)
                .collect::<Converted<_>>()?,
        };
        Ok(HumanControlInput {
            envelope,
            scope: run_state_scope(self.scope)?,
            control,
        })
    }
}
