//! Closed transport DTOs for an `instruction-source-registry` document
//! (`registries/operating-model/instruction-source-registry.schema.json`,
//! `registries/operating-model/scoped-record.schema.json`). Every
//! object-shaped DTO here is `#[serde(deny_unknown_fields)]`: an unknown
//! field is a transport-level rejection, not silently ignored.
//!
//! This is level 1 of the boundary sequence
//! (`standards/workspace/rust-migration-quality.md` §4): these types know
//! nothing about the CONTENT rules the JSON Schema's `allOf`/`if`/`then`
//! branches or this crate's own domain conversion enforce (a `medium` of
//! `"file"` requiring `location.path`/`location.container_ref` and
//! forbidding `service_ref`/`resource_ref`, a closed `format`/`currency`/
//! `divergence.status` pool) — most fields are therefore `String`/
//! `Option<T>`, not yet the closed enums [`meridian_core::instruction_source`]
//! equivalents would be, matched against and converted in
//! [`super::convert`].

use serde::Deserialize;

// `schema_ref`/`schema_version`/`title`/`scope`/`origin`/`authority` are
// read by nothing past this struct: they close the shape to exactly the
// fields `scoped-record.schema.json` declares (their own content rules are
// either already checked by the JSON Schema pass that always runs before
// this DTO is even parsed, or — for scope/origin/authority — simply not a
// business rule this contract has today; nothing in
// `instruction-source-registry.md` conditions on them). Keeping them here,
// instead of leaving them out and letting `deny_unknown_fields` reject a
// real fixture, is what makes the closed shape honest about what a real
// envelope actually carries.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct RegistryDto {
    #[serde(rename = "$schema", default)]
    pub schema_ref: Option<String>,
    pub schema_version: i64,
    pub registry_id: String,
    pub title: String,
    pub instruction_sources: Vec<EntryDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntryDto {
    #[serde(rename = "$schema", default)]
    #[allow(dead_code)]
    pub schema_ref: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub schema_version: Option<i64>,
    pub id: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub title: Option<String>,
    pub record_type: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub scope: Option<ScopeDto>,
    #[serde(default)]
    #[allow(dead_code)]
    pub origin: Option<OriginDto>,
    #[serde(default)]
    #[allow(dead_code)]
    pub authority: Option<AuthorityDto>,
    pub payload: PayloadDto,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct ScopeDto {
    #[serde(rename = "type")]
    pub scope_type: String,
    pub id: String,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub organization_profile_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct OriginDto {
    pub kind: String,
    #[serde(default)]
    pub source_ref: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct AuthorityDto {
    pub kind: String,
    pub authority_ref: String,
    #[serde(default)]
    pub decision_ref: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PayloadDto {
    pub normative_status: String,
    pub medium: String,
    pub location: LocationDto,
    pub recorded_state: RecordedStateDto,
    pub format: String,
    pub read_channel: ReadChannelDto,
    pub divergence: DivergenceDto,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LocationDto {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub container_ref: Option<String>,
    #[serde(default)]
    pub service_ref: Option<String>,
    #[serde(default)]
    pub resource_ref: Option<String>,
    pub missing_behavior: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordedStateDto {
    pub revision: String,
    pub digest: DigestDto,
    pub revision_verified: bool,
    pub currency: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DigestDto {
    pub algorithm: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadChannelDto {
    pub kind: String,
    pub meridian_visibility: String,
    pub agent_auto_read: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DivergenceDto {
    pub status: String,
    #[serde(default)]
    pub previous_state: Option<ObservedStateDto>,
    #[serde(default)]
    pub current_state: Option<ObservedStateDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedStateDto {
    pub revision: String,
    pub digest: DigestDto,
    pub verified: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest_json() -> serde_json::Value {
        serde_json::json!({"algorithm": "sha-256", "value": "b".repeat(64)})
    }
    fn recorded_state_json() -> serde_json::Value {
        serde_json::json!({"revision": "a".repeat(40), "digest": digest_json(), "revision_verified": true, "currency": "current"})
    }
    fn read_channel_json() -> serde_json::Value {
        serde_json::json!({"kind": "meridian-observed", "meridian_visibility": "full", "agent_auto_read": false})
    }
    fn location_json() -> serde_json::Value {
        serde_json::json!({"path": "a.md", "container_ref": "container-1", "missing_behavior": "fail-closed"})
    }
    fn divergence_json() -> serde_json::Value {
        serde_json::json!({"status": "unknown"})
    }
    fn payload_json() -> serde_json::Value {
        serde_json::json!({
            "normative_status": "not-a-norm",
            "medium": "file",
            "location": location_json(),
            "recorded_state": recorded_state_json(),
            "format": "markdown-section",
            "read_channel": read_channel_json(),
            "divergence": divergence_json(),
        })
    }
    fn scope_json() -> serde_json::Value {
        serde_json::json!({"type": "repository-scope", "id": "sample-repository", "workspace_id": "sample-workspace"})
    }
    fn origin_json() -> serde_json::Value {
        serde_json::json!({"kind": "declared", "source_ref": "hand-entered"})
    }
    fn authority_json() -> serde_json::Value {
        serde_json::json!({"kind": "repository-maintainer", "authority_ref": "maintainer:1"})
    }
    fn entry_json() -> serde_json::Value {
        serde_json::json!({
            "id": "src-1",
            "record_type": "instruction-source",
            "scope": scope_json(),
            "origin": origin_json(),
            "authority": authority_json(),
            "payload": payload_json(),
        })
    }
    fn registry_json() -> serde_json::Value {
        serde_json::json!({"schema_version": 1, "registry_id": "instruction-source-registry", "title": "x", "instruction_sources": [entry_json()]})
    }

    fn with_extra_field(mut v: serde_json::Value) -> serde_json::Value {
        v.as_object_mut()
            .expect("fixture must be an object")
            .insert("extra_unknown_field".to_string(), serde_json::json!(1));
        v
    }

    /// Every object-shaped DTO this module declares is exercised
    /// individually with its own well-formed fixture plus one added unknown
    /// field.
    #[test]
    fn deny_unknown_fields_rejects_an_extra_field_on_every_object_dto() {
        assert!(
            serde_json::from_value::<RegistryDto>(with_extra_field(registry_json())).is_err(),
            "RegistryDto"
        );
        assert!(
            serde_json::from_value::<EntryDto>(with_extra_field(entry_json())).is_err(),
            "EntryDto"
        );
        assert!(
            serde_json::from_value::<ScopeDto>(with_extra_field(scope_json())).is_err(),
            "ScopeDto"
        );
        assert!(
            serde_json::from_value::<OriginDto>(with_extra_field(origin_json())).is_err(),
            "OriginDto"
        );
        assert!(
            serde_json::from_value::<AuthorityDto>(with_extra_field(authority_json())).is_err(),
            "AuthorityDto"
        );
        assert!(
            serde_json::from_value::<PayloadDto>(with_extra_field(payload_json())).is_err(),
            "PayloadDto"
        );
        assert!(
            serde_json::from_value::<LocationDto>(with_extra_field(location_json())).is_err(),
            "LocationDto"
        );
        assert!(
            serde_json::from_value::<RecordedStateDto>(with_extra_field(recorded_state_json()))
                .is_err(),
            "RecordedStateDto"
        );
        assert!(
            serde_json::from_value::<DigestDto>(with_extra_field(digest_json())).is_err(),
            "DigestDto"
        );
        assert!(
            serde_json::from_value::<ReadChannelDto>(with_extra_field(read_channel_json()))
                .is_err(),
            "ReadChannelDto"
        );
        assert!(
            serde_json::from_value::<DivergenceDto>(with_extra_field(divergence_json())).is_err(),
            "DivergenceDto"
        );
        assert!(
            serde_json::from_value::<ObservedStateDto>(with_extra_field(serde_json::json!({
                "revision": "a".repeat(40), "digest": digest_json(), "verified": true,
            })))
            .is_err(),
            "ObservedStateDto"
        );
    }

    /// Companion positive check: every one of the same fixtures, WITHOUT the
    /// extra field, deserializes cleanly.
    #[test]
    fn every_object_dto_accepts_its_own_well_formed_fixture() {
        assert!(serde_json::from_value::<RegistryDto>(registry_json()).is_ok());
        assert!(serde_json::from_value::<EntryDto>(entry_json()).is_ok());
        assert!(serde_json::from_value::<ScopeDto>(scope_json()).is_ok());
        assert!(serde_json::from_value::<OriginDto>(origin_json()).is_ok());
        assert!(serde_json::from_value::<AuthorityDto>(authority_json()).is_ok());
        assert!(serde_json::from_value::<PayloadDto>(payload_json()).is_ok());
        assert!(serde_json::from_value::<LocationDto>(location_json()).is_ok());
        assert!(serde_json::from_value::<RecordedStateDto>(recorded_state_json()).is_ok());
        assert!(serde_json::from_value::<DigestDto>(digest_json()).is_ok());
        assert!(serde_json::from_value::<ReadChannelDto>(read_channel_json()).is_ok());
        assert!(serde_json::from_value::<DivergenceDto>(divergence_json()).is_ok());
    }
}
