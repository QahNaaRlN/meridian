//! The scoped-record envelope every migration-owned record carries — a
//! migration plan, a canonical export, a record inside an export — as a
//! typed, schema-shaped head (`registries/operating-model/scoped-record.schema.json`).
//!
//! Every field admits exactly what the envelope schema admits, so the app's
//! closed transport DTO converts total over a schema-clean document:
//! `$schema` is any string, `title` and the references are `minLength: 1`
//! ([`RecordText`]/[`PortableRef`](crate::run_contracts::PortableRef), a whitespace-only value included), the
//! scope is the closed [`Scope`] the envelope's own `allOf` shapes. Which
//! scope, origin or authority a family allows is a DOMAIN rule its check
//! reports, never a conversion failure.

use crate::json::Json;
use crate::run_contracts::{RecordAuthority, RecordOrigin, RecordText};
use crate::types::{OriginKind, Scope, SemanticId};

use super::projection;

/// The envelope fields of one scoped record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordHead {
    pub declared_schema: String,
    pub id: SemanticId,
    pub title: RecordText,
    pub record_type: SemanticId,
    pub scope: Scope,
    pub origin: RecordOrigin,
    pub authority: RecordAuthority,
}

impl RecordHead {
    pub fn origin_kind(&self) -> OriginKind {
        match &self.origin {
            RecordOrigin::BuiltIn => OriginKind::BuiltIn,
            RecordOrigin::Sourced { kind, .. } => kind.kind(),
        }
    }

    pub fn origin_source_ref(&self) -> Option<&str> {
        match &self.origin {
            RecordOrigin::BuiltIn => None,
            RecordOrigin::Sourced { source_ref, .. } => Some(source_ref.as_str()),
        }
    }

    /// The canonical members of the envelope (everything but `payload`):
    /// the record `canonicalize()` writes, with the payload supplied by the
    /// caller.
    pub(crate) fn canonical_record(&self, payload: Json) -> Json {
        let mut origin = vec![(Json::key("kind"), Json::str(self.origin_kind().as_str()))];
        if let Some(source_ref) = self.origin_source_ref() {
            origin.push((Json::key("source_ref"), Json::str(source_ref)));
        }
        let mut authority = vec![
            (Json::key("kind"), Json::str(self.authority.kind.as_str())),
            (
                Json::key("authority_ref"),
                Json::str(self.authority.authority_ref.as_str()),
            ),
        ];
        if let Some(decision_ref) = &self.authority.decision_ref {
            authority.push((Json::key("decision_ref"), Json::str(decision_ref.as_str())));
        }
        Json::Object(vec![
            (Json::key("$schema"), Json::str(&self.declared_schema)),
            (Json::key("schema_version"), Json::Number(1.0)),
            (Json::key("id"), Json::str(self.id.as_str())),
            (Json::key("title"), Json::str(self.title.as_str())),
            (
                Json::key("record_type"),
                Json::str(self.record_type.as_str()),
            ),
            (Json::key("scope"), projection::scope(&self.scope)),
            (Json::key("origin"), Json::Object(origin)),
            (Json::key("authority"), Json::Object(authority)),
            (Json::key("payload"), payload),
        ])
    }
}
