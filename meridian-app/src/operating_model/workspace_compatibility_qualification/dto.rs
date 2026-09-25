//! Closed transport DTOs of `workspace-compatibility-qualification.schema.json`
//! and their total conversion into the typed core input.

use meridian_core::qualification::workspace::{WorkspacePayloadInput, WorkspaceQualificationInput};
use serde::Deserialize;

use super::super::migration_boundary::head::{
    head, qualification_state, semantic_ids, texts, HeadParts, PinDto, ScopeDto,
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
    pub(super) fn into_inputs(self) -> Converted<Vec<WorkspaceQualificationInput>> {
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
struct PayloadDto {
    workspace_connection_refs: Vec<PinDto>,
    #[serde(default)]
    workspace_repository_ids: Option<Vec<String>>,
    migration_plan_ref: Option<PinDto>,
    canonical_export_ref: Option<PinDto>,
    qualification_state: String,
    qualification_reason: String,
    blockers: Vec<String>,
    open_questions: Vec<String>,
}

impl EntryDto {
    fn into_input(self) -> Converted<WorkspaceQualificationInput> {
        let p = self.payload;
        Ok(WorkspaceQualificationInput {
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
            payload: WorkspacePayloadInput {
                workspace_connection_refs: p
                    .workspace_connection_refs
                    .into_iter()
                    .enumerate()
                    .map(|(i, pin)| pin.into_pin(&format!("workspace_connection_refs[{i}]")))
                    .collect::<Converted<_>>()?,
                workspace_repository_ids: p
                    .workspace_repository_ids
                    .map(|ids| semantic_ids("workspace_repository_ids", ids))
                    .transpose()?,
                migration_plan_ref: p
                    .migration_plan_ref
                    .map(|pin| pin.into_pin("migration_plan_ref"))
                    .transpose()?,
                canonical_export_ref: p
                    .canonical_export_ref
                    .map(|pin| pin.into_pin("canonical_export_ref"))
                    .transpose()?,
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
