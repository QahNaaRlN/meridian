//! Closed transport DTOs for the scoped-record envelope parts every
//! run-contract record shares, and the defensive DTO -> core conversions
//! they and the family DTOs use. A conversion error is drift between the
//! DTO and the schema the document already passed: it is reported as a
//! `String` that the boundary turns into [`super::CaseOutcome::ConversionDrift`].

use meridian_core::run_contracts::{
    ActorRef, PinSha256, PinnedRecordKind, PinnedRef, PortableRef, RecordAuthority, RecordEnvelope,
    RecordOrigin, RecordText, RunId, RunStateScope, ScopeRevision, SourcedOriginKind,
    TransitionSequence,
};
use meridian_core::types::{AuthorityKind, OriginKind, SemanticId, WorkspaceId};
use serde::Deserialize;

pub(crate) type Converted<T> = Result<T, String>;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunStateScopeDto {
    #[serde(rename = "type")]
    pub scope_type: String,
    pub id: String,
    pub workspace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OriginDto {
    pub kind: String,
    pub source_ref: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AuthorityDto {
    pub kind: String,
    pub authority_ref: String,
    pub decision_ref: Option<String>,
}

/// A closed structured pinned reference
/// (`definitions.pinned_ref` of the context-manifest, evidence-and-handoff
/// and field-evaluation schemas; the field-evaluation schema admits no
/// `run_id`, which its schema gate rejects before this DTO).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PinnedRefDto {
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

impl PinnedRefDto {
    pub(crate) fn into_pin(self, field: &str) -> Converted<PinnedRef> {
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

/// A schema `sha256` (64 hexadecimal characters).
pub(crate) fn sha256(field: &str, value: Option<String>) -> Converted<Option<PinSha256>> {
    value
        .map(|v| PinSha256::new(v).map_err(|e| format!("{field}: {e}")))
        .transpose()
}

/// The envelope header fields of one record DTO, borrowed for conversion.
pub(crate) struct EnvelopeParts {
    pub declared_schema: String,
    pub schema_version: u64,
    pub id: String,
    pub title: String,
    pub record_type: String,
    pub origin: OriginDto,
    pub authority: AuthorityDto,
}

const ORIGIN_KINDS: [OriginKind; 5] = [
    OriginKind::BuiltIn,
    OriginKind::Declared,
    OriginKind::Migrated,
    OriginKind::Imported,
    OriginKind::Derived,
];

const AUTHORITY_KINDS: [AuthorityKind; 6] = [
    AuthorityKind::MethodologyOwner,
    AuthorityKind::User,
    AuthorityKind::Organization,
    AuthorityKind::ProjectOwner,
    AuthorityKind::RepositoryMaintainer,
    AuthorityKind::DelegatedRun,
];

pub(crate) fn text(field: &str, value: String) -> Converted<RecordText> {
    RecordText::new(value).map_err(|e| format!("{field}: {e}"))
}

pub(crate) fn optional_text(field: &str, value: Option<String>) -> Converted<Option<RecordText>> {
    value.map(|v| text(field, v)).transpose()
}

pub(crate) fn actor(field: &str, value: String) -> Converted<ActorRef> {
    ActorRef::new(value).map_err(|e| format!("{field}: {e}"))
}

pub(crate) fn portable(field: &str, value: String) -> Converted<PortableRef> {
    PortableRef::new(value).map_err(|e| format!("{field}: {e}"))
}

pub(crate) fn portable_list(field: &str, values: Vec<String>) -> Converted<Vec<PortableRef>> {
    values.into_iter().map(|v| portable(field, v)).collect()
}

pub(crate) fn semantic_id(field: &str, value: String) -> Converted<SemanticId> {
    SemanticId::new(value).map_err(|e| format!("{field}: {e}"))
}

pub(crate) fn scope_revision(field: &str, value: u64) -> Converted<ScopeRevision> {
    ScopeRevision::new(value).map_err(|e| format!("{field}: {e}"))
}

pub(crate) fn sequence(field: &str, value: u64) -> Converted<TransitionSequence> {
    TransitionSequence::new(value).map_err(|e| format!("{field}: {e}"))
}

/// A member of one of `meridian-core`'s closed pools, parsed by the pool's
/// own `parse` — this crate carries no second copy of any pool.
pub(crate) fn closed<T>(field: &str, value: &str, parse: fn(&str) -> Option<T>) -> Converted<T> {
    parse(value).ok_or_else(|| format!("{field}: \"{value}\" is outside the closed pool"))
}

pub(crate) fn optional_closed<T>(
    field: &str,
    value: Option<String>,
    parse: fn(&str) -> Option<T>,
) -> Converted<Option<T>> {
    value.map(|v| closed(field, &v, parse)).transpose()
}

pub(crate) fn run_state_scope(dto: RunStateScopeDto) -> Converted<RunStateScope> {
    if dto.scope_type != "run-state" {
        return Err(format!(
            "scope.type: \"{}\" is not run-state",
            dto.scope_type
        ));
    }
    let run = RunId::new(dto.id).map_err(|e| format!("scope.id: {e}"))?;
    let workspace =
        WorkspaceId::new(dto.workspace_id).map_err(|e| format!("scope.workspace_id: {e}"))?;
    Ok(RunStateScope::new(run, workspace))
}

/// The built-in role catalogue's fixed scope (`built-in-methodology`).
pub(crate) fn built_in_methodology_scope(scope_type: &str, id: &str) -> Converted<()> {
    let built_in = meridian_core::types::BUILT_IN_METHODOLOGY_ID;
    if scope_type == built_in && id == built_in {
        Ok(())
    } else {
        Err(format!(
            "scope: \"{scope_type}\"/\"{id}\" is not the built-in methodology scope"
        ))
    }
}

fn origin(dto: OriginDto) -> Converted<RecordOrigin> {
    let kind = ORIGIN_KINDS
        .into_iter()
        .find(|k| k.as_str() == dto.kind)
        .ok_or_else(|| format!("origin.kind: \"{}\" is outside the closed pool", dto.kind))?;
    match (SourcedOriginKind::new(kind), dto.source_ref) {
        (None, None) => Ok(RecordOrigin::BuiltIn),
        (Some(kind), Some(source_ref)) => Ok(RecordOrigin::Sourced {
            kind,
            source_ref: portable("origin.source_ref", source_ref)?,
        }),
        (None, Some(_)) => Err("origin: a built-in origin carries a source_ref".to_string()),
        (Some(_), None) => Err("origin: a sourced origin carries no source_ref".to_string()),
    }
}

fn authority(dto: AuthorityDto) -> Converted<RecordAuthority> {
    let kind = AUTHORITY_KINDS
        .into_iter()
        .find(|k| k.as_str() == dto.kind)
        .ok_or_else(|| {
            format!(
                "authority.kind: \"{}\" is outside the closed pool",
                dto.kind
            )
        })?;
    Ok(RecordAuthority {
        kind,
        authority_ref: portable("authority.authority_ref", dto.authority_ref)?,
        decision_ref: dto
            .decision_ref
            .map(|v| portable("authority.decision_ref", v))
            .transpose()?,
    })
}

/// The shared envelope: `schema_version` must be 1 and `record_type` the
/// family's own (both schema constants; anything else is drift).
pub(crate) fn record_envelope(
    parts: EnvelopeParts,
    record_type: &str,
) -> Converted<RecordEnvelope> {
    if parts.schema_version != 1 {
        return Err(format!("schema_version: {} is not 1", parts.schema_version));
    }
    if parts.record_type != record_type {
        return Err(format!(
            "record_type: \"{}\" is not \"{record_type}\"",
            parts.record_type
        ));
    }
    Ok(RecordEnvelope {
        declared_schema: text("$schema", parts.declared_schema)?,
        id: semantic_id("id", parts.id)?,
        title: text("title", parts.title)?,
        origin: origin(parts.origin)?,
        authority: authority(parts.authority)?,
    })
}
