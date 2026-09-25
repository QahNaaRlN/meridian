//! Closed transport DTOs of `upgrade-integration-qualification.schema.json`
//! and their total conversion into the typed core input.

use meridian_core::qualification::upgrade::{
    ScenarioId, ScenarioInput, UpgradePayloadInput, UpgradeQualificationInput,
};
use serde::Deserialize;

use super::super::migration_boundary::head::{
    head, qualification_state, texts, HeadParts, PinDto, ScopeDto,
};
use super::super::run_contract_boundary::envelope::{text, AuthorityDto, Converted, OriginDto};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ContainerDto {
    #[serde(rename = "$schema", default)]
    _schema: Option<String>,
    #[serde(rename = "schema_version")]
    _schema_version: u64,
    #[serde(rename = "registry_id")]
    _registry_id: String,
    #[serde(rename = "title")]
    _title: String,
    qualifications: Vec<EntryDto>,
}

impl ContainerDto {
    pub(super) fn into_inputs(self) -> Converted<Vec<UpgradeQualificationInput>> {
        self.qualifications
            .into_iter()
            .enumerate()
            .map(|(i, e)| {
                e.into_input()
                    .map_err(|m| format!("qualifications[{i}]: {m}"))
            })
            .collect()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryDto {
    #[serde(rename = "$schema")]
    declared_schema: String,
    schema_version: u64,
    id: String,
    title: String,
    record_type: String,
    scope: ScopeDto,
    origin: OriginDto,
    authority: AuthorityDto,
    payload: PayloadDto,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScenarioDto {
    scenario_id: String,
    task_specification_ref: PinDto,
    execution_state_ref: PinDto,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransitionDto {
    status: String,
    notes: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PayloadDto {
    task_journey_ref: PinDto,
    field_evaluation_report_ref: Option<PinDto>,
    scenario_classifications: Vec<ScenarioDto>,
    workspace_transition_compatibility: TransitionDto,
    qualification_state: String,
    qualification_reason: String,
    blockers: Vec<String>,
    open_questions: Vec<String>,
}

impl EntryDto {
    fn into_input(self) -> Converted<UpgradeQualificationInput> {
        let p = self.payload;
        if p.workspace_transition_compatibility.status != "kernel-template-only" {
            return Err(format!(
                "workspace_transition_compatibility.status: \"{}\" is not kernel-template-only",
                p.workspace_transition_compatibility.status
            ));
        }
        Ok(UpgradeQualificationInput {
            head: head(HeadParts {
                declared_schema: self.declared_schema,
                schema_version: self.schema_version,
                id: self.id,
                title: self.title,
                record_type: self.record_type,
                scope: self.scope,
                origin: self.origin,
                authority: self.authority,
            })?,
            payload: UpgradePayloadInput {
                task_journey_ref: p.task_journey_ref.into_pin("task_journey_ref")?,
                field_evaluation_report_ref: p
                    .field_evaluation_report_ref
                    .map(|pin| pin.into_pin("field_evaluation_report_ref"))
                    .transpose()?,
                scenario_classifications: p
                    .scenario_classifications
                    .into_iter()
                    .enumerate()
                    .map(|(i, s)| {
                        let field = format!("scenario_classifications[{i}]");
                        Ok(ScenarioInput {
                            scenario_id: ScenarioId::parse(&s.scenario_id).ok_or_else(|| {
                                format!(
                                    "{field}.scenario_id: \"{}\" is outside the closed pool",
                                    s.scenario_id
                                )
                            })?,
                            task_specification_ref: s
                                .task_specification_ref
                                .into_pin(&format!("{field}.task_specification_ref"))?,
                            execution_state_ref: s
                                .execution_state_ref
                                .into_pin(&format!("{field}.execution_state_ref"))?,
                        })
                    })
                    .collect::<Converted<_>>()?,
                transition_notes: text(
                    "workspace_transition_compatibility.notes",
                    p.workspace_transition_compatibility.notes,
                )?,
                qualification_state: qualification_state(
                    "qualification_state",
                    &p.qualification_state,
                )?,
                qualification_reason: text("qualification_reason", p.qualification_reason)?,
                blockers: texts("blockers", p.blockers)?,
                open_questions: texts("open_questions", p.open_questions)?,
            },
        })
    }
}
