//! Closed transport DTOs shared by the four container families: the
//! scoped-record envelope head, a scope, a `{ algorithm, value }` digest and
//! a qualification's pinned reference — and their total conversions into
//! the typed `meridian-core` shapes. A conversion error is drift between the
//! DTO and the schema the document already passed.

use meridian_core::migration::record::RecordHead;
use meridian_core::qualification::pin::QualificationPin;
use meridian_core::qualification::QualificationState;
use meridian_core::run_contracts::{PinSha256, RecordText};
use meridian_core::types::{
    AuthorityKind, ContentDigest, Scope, SemanticId, Verdict, WorkspaceId, BUILT_IN_METHODOLOGY_ID,
};
use serde::Deserialize;

use super::super::run_contract_boundary::envelope::{
    authority, origin, semantic_id, text, AuthorityDto, Converted, OriginDto,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ScopeDto {
    #[serde(rename = "type")]
    pub scope_type: String,
    pub id: String,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub organization_profile_id: Option<String>,
}

fn workspace(field: &str, value: Option<String>) -> Converted<WorkspaceId> {
    let value = value.ok_or_else(|| format!("{field}.workspace_id: absent"))?;
    WorkspaceId::new(value).map_err(|e| format!("{field}.workspace_id: {e}"))
}

/// The closed scope the envelope's (and a target's) `allOf` shapes.
pub(crate) fn scope(field: &str, dto: ScopeDto) -> Converted<Scope> {
    let id = |v: String| semantic_id(&format!("{field}.id"), v);
    let no_org = |dto_org: &Option<String>| match dto_org {
        None => Ok(()),
        Some(_) => Err(format!(
            "{field}.organization_profile_id: only a project-workspace carries one"
        )),
    };
    let no_workspace = |w: &Option<String>| match w {
        None => Ok(()),
        Some(_) => Err(format!(
            "{field}.workspace_id: not carried by this scope type"
        )),
    };
    match dto.scope_type.as_str() {
        "built-in-methodology" => {
            no_org(&dto.organization_profile_id)?;
            no_workspace(&dto.workspace_id)?;
            if dto.id != BUILT_IN_METHODOLOGY_ID {
                return Err(format!("{field}.id: not the built-in methodology id"));
            }
            Ok(Scope::built_in_methodology())
        }
        "user-profile" => {
            no_org(&dto.organization_profile_id)?;
            no_workspace(&dto.workspace_id)?;
            Ok(Scope::user_profile(id(dto.id)?))
        }
        "organization-profile" => {
            no_org(&dto.organization_profile_id)?;
            no_workspace(&dto.workspace_id)?;
            Ok(Scope::organization_profile(id(dto.id)?))
        }
        "project-workspace" => {
            no_workspace(&dto.workspace_id)?;
            let org = dto
                .organization_profile_id
                .map(|o| semantic_id(&format!("{field}.organization_profile_id"), o))
                .transpose()?;
            Ok(Scope::project_workspace(id(dto.id)?, org))
        }
        "repository-scope" => {
            no_org(&dto.organization_profile_id)?;
            let w = workspace(field, dto.workspace_id)?;
            Ok(Scope::repository_scope(id(dto.id)?, w))
        }
        "run-state" => {
            no_org(&dto.organization_profile_id)?;
            let w = workspace(field, dto.workspace_id)?;
            Ok(Scope::run_state(id(dto.id)?, w))
        }
        other => Err(format!(
            "{field}.type: \"{other}\" is outside the closed pool"
        )),
    }
}

/// The scoped-record envelope fields of one entry.
pub(crate) struct HeadParts {
    pub declared_schema: String,
    pub schema_version: u64,
    pub id: String,
    pub title: String,
    pub record_type: String,
    pub scope: ScopeDto,
    pub origin: OriginDto,
    pub authority: AuthorityDto,
}

pub(crate) fn head(parts: HeadParts) -> Converted<RecordHead> {
    if parts.schema_version != 1 {
        return Err(format!("schema_version: {} is not 1", parts.schema_version));
    }
    Ok(RecordHead {
        declared_schema: parts.declared_schema,
        id: semantic_id("id", parts.id)?,
        title: text("title", parts.title)?,
        record_type: semantic_id("record_type", parts.record_type)?,
        scope: scope("scope", parts.scope)?,
        origin: origin(parts.origin)?,
        authority: authority(parts.authority)?,
    })
}

/// `definitions.sha256_digest`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DigestDto {
    pub algorithm: String,
    pub value: String,
}

pub(crate) fn digest(field: &str, dto: DigestDto) -> Converted<ContentDigest> {
    if dto.algorithm != "sha-256" {
        return Err(format!(
            "{field}.algorithm: \"{}\" is not sha-256",
            dto.algorithm
        ));
    }
    sha256_hex(field, dto.value)
}

/// `definitions.sha256_hex`.
pub(crate) fn sha256_hex(field: &str, value: String) -> Converted<ContentDigest> {
    ContentDigest::from_hex(value).map_err(|e| format!("{field}: {e}"))
}

pub(crate) fn texts(field: &str, values: Vec<String>) -> Converted<Vec<RecordText>> {
    values.into_iter().map(|v| text(field, v)).collect()
}

pub(crate) fn optional_record_text(
    field: &str,
    value: Option<String>,
) -> Converted<Option<RecordText>> {
    value.map(|v| text(field, v)).transpose()
}

pub(crate) fn authority_kind(field: &str, value: &str) -> Converted<AuthorityKind> {
    [
        AuthorityKind::MethodologyOwner,
        AuthorityKind::User,
        AuthorityKind::Organization,
        AuthorityKind::ProjectOwner,
        AuthorityKind::RepositoryMaintainer,
        AuthorityKind::DelegatedRun,
    ]
    .into_iter()
    .find(|k| k.as_str() == value)
    .ok_or_else(|| format!("{field}: \"{value}\" is outside the closed pool"))
}

pub(crate) fn verdict(field: &str, value: &str) -> Converted<Verdict> {
    [Verdict::Verified, Verdict::Unverified, Verdict::Blocked]
        .into_iter()
        .find(|v| v.as_str() == value)
        .ok_or_else(|| format!("{field}: \"{value}\" is outside the closed pool"))
}

pub(crate) fn qualification_state(field: &str, value: &str) -> Converted<QualificationState> {
    QualificationState::parse(value)
        .ok_or_else(|| format!("{field}: \"{value}\" is outside the closed pool"))
}

pub(crate) fn semantic_ids(field: &str, values: Vec<String>) -> Converted<Vec<SemanticId>> {
    values.into_iter().map(|v| semantic_id(field, v)).collect()
}

/// `definitions.pinned_ref` of both qualification schemas.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PinDto {
    record_type: String,
    id: String,
    reference: String,
    sha256: String,
}

impl PinDto {
    pub(crate) fn into_pin(self, field: &str) -> Converted<QualificationPin> {
        Ok(QualificationPin {
            declared_kind: text(&format!("{field}.record_type"), self.record_type)?,
            id: semantic_id(&format!("{field}.id"), self.id)?,
            reference: text(&format!("{field}.reference"), self.reference)?,
            sha256: PinSha256::new(self.sha256).map_err(|e| format!("{field}.sha256: {e}"))?,
        })
    }
}
