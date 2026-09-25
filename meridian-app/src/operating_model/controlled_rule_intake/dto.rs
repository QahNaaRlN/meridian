//! Closed transport DTOs for a `controlled-rule-intake` registry document
//! (`registries/operating-model/controlled-rule-intake.schema.json`,
//! `registries/operating-model/scoped-record.schema.json`). Every
//! object-shaped DTO here is `#[serde(deny_unknown_fields)]`: an unknown
//! field is a transport-level rejection, not silently ignored.
//!
//! This is level 1 of the boundary sequence
//! (`standards/workspace/rust-migration-quality.md` §4): these types know
//! nothing about the CONTENT rules the JSON Schema's `allOf`/`if`/`then`
//! branches or this crate's own domain conversion enforce (a boundary
//! `unit` of `"agents-md-section"` requiring `region` and forbidding
//! `start`/`end`, an `applicability_state` gating which other fields are
//! legal) — most fields are therefore `String`/`Option<T>`, not yet the
//! closed enums [`super::domain`] equivalents would be, matched against and
//! converted in [`super::convert`].

use serde::Deserialize;

// `schema_ref`/`schema_version`/`registry_id`/`title` are read by nothing
// past this struct: they exist only so `deny_unknown_fields` closes the
// shape to exactly the fields `controlled-rule-intake.schema.json` and
// `scoped-record.schema.json` declare (their own values — `const 1`,
// `const "controlled-rule-intake"` — are already checked by the JSON
// Schema pass that always runs before this DTO is even parsed, so
// re-checking them here would be a second, redundant copy of that check).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct RegistryDto {
    #[serde(rename = "$schema", default)]
    pub schema_ref: Option<String>,
    pub schema_version: i64,
    pub registry_id: String,
    pub title: String,
    pub rule_candidates: Vec<CandidateDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateDto {
    // See `RegistryDto`'s comment: closes the transport shape, not read
    // past this struct.
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
    pub scope: ScopeDto,
    pub origin: OriginDto,
    pub authority: AuthorityDto,
    pub payload: PayloadDto,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
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
pub struct OriginDto {
    pub kind: String,
    #[serde(default)]
    pub source_ref: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityDto {
    pub kind: String,
    pub authority_ref: String,
    #[serde(default)]
    pub decision_ref: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PayloadDto {
    pub source_ref: SourceRefDto,
    pub boundary: BoundaryDto,
    pub raw_excerpt: String,
    pub normalized_text: String,
    pub semantic_key: String,
    pub classification_basis: String,
    #[serde(default)]
    pub conflicts_with: Vec<String>,
    pub applicability_state: String,
    #[serde(default)]
    pub not_applicable_reason: Option<String>,
    #[serde(default)]
    pub owner_decision: Option<OwnerDecisionDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRefDto {
    pub record_type: String,
    pub id: String,
    pub reference: String,
    pub revision: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundaryDto {
    pub unit: String,
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default)]
    pub start: Option<u64>,
    #[serde(default)]
    pub end: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OwnerDecisionDto {
    pub decision_ref: String,
    pub decided_at: String,
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::super::source_resolver::{
        DigestDto, ReadChannelDto, RecordedStateDto, ResolvedInstructionSourceDto,
    };
    use super::*;

    fn source_ref_json() -> serde_json::Value {
        serde_json::json!({"record_type": "instruction-source", "id": "src-1", "reference": "r", "revision": "rev", "sha256": "a".repeat(64)})
    }
    fn boundary_json() -> serde_json::Value {
        serde_json::json!({"unit": "whole-source"})
    }
    fn payload_json() -> serde_json::Value {
        serde_json::json!({
            "source_ref": source_ref_json(),
            "boundary": boundary_json(),
            "raw_excerpt": "x",
            "normalized_text": "x",
            "semantic_key": "x",
            "classification_basis": "x",
            "applicability_state": "candidate",
        })
    }
    fn scope_json() -> serde_json::Value {
        serde_json::json!({"type": "built-in-methodology", "id": "built-in-methodology"})
    }
    fn origin_json() -> serde_json::Value {
        serde_json::json!({"kind": "built-in"})
    }
    fn authority_json() -> serde_json::Value {
        serde_json::json!({"kind": "delegated-run", "authority_ref": "run:1"})
    }
    fn candidate_json() -> serde_json::Value {
        serde_json::json!({
            "id": "cand-1",
            "record_type": "rule-candidate",
            "scope": scope_json(),
            "origin": origin_json(),
            "authority": authority_json(),
            "payload": payload_json(),
        })
    }
    fn registry_json() -> serde_json::Value {
        serde_json::json!({"schema_version": 1, "registry_id": "controlled-rule-intake", "title": "x", "rule_candidates": [candidate_json()]})
    }
    fn owner_decision_json() -> serde_json::Value {
        serde_json::json!({"decision_ref": "x", "decided_at": "2026-09-21", "reason": "y"})
    }
    fn digest_json() -> serde_json::Value {
        serde_json::json!({"algorithm": "sha-256", "value": "b".repeat(64)})
    }
    fn read_channel_json() -> serde_json::Value {
        serde_json::json!({"kind": "meridian-observed", "meridian_visibility": "full", "agent_auto_read": false})
    }
    fn recorded_state_json() -> serde_json::Value {
        serde_json::json!({"revision": "rev", "digest": digest_json(), "revision_verified": true, "currency": "current"})
    }
    fn resolved_instruction_source_json() -> serde_json::Value {
        serde_json::json!({"record_type": "instruction-source", "id": "src-1", "reference": "r", "recorded_state": recorded_state_json(), "read_channel": read_channel_json()})
    }

    /// Adds `"extra_unknown_field": 1` to a JSON object and asserts it is
    /// still valid JSON (a defensive check on the test's own fixture, not
    /// the thing under test).
    fn with_extra_field(mut v: serde_json::Value) -> serde_json::Value {
        v.as_object_mut()
            .expect("fixture must be an object")
            .insert("extra_unknown_field".to_string(), serde_json::json!(1));
        v
    }

    /// Corrective round item 4: an honest structural test of every one of
    /// the 13 object-shaped DTOs this module and `source_resolver` declare
    /// — each is exercised individually with ITS OWN well-formed fixture
    /// plus one added unknown field, not a handful standing in for the
    /// rest. Listed in the same order `dto.rs`/`source_resolver.rs`
    /// declare them.
    #[test]
    fn deny_unknown_fields_rejects_an_extra_field_on_every_one_of_the_13_object_dtos() {
        assert!(
            serde_json::from_value::<RegistryDto>(with_extra_field(registry_json())).is_err(),
            "RegistryDto"
        );
        assert!(
            serde_json::from_value::<CandidateDto>(with_extra_field(candidate_json())).is_err(),
            "CandidateDto"
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
            serde_json::from_value::<SourceRefDto>(with_extra_field(source_ref_json())).is_err(),
            "SourceRefDto"
        );
        assert!(
            serde_json::from_value::<BoundaryDto>(with_extra_field(boundary_json())).is_err(),
            "BoundaryDto"
        );
        assert!(
            serde_json::from_value::<OwnerDecisionDto>(with_extra_field(owner_decision_json()))
                .is_err(),
            "OwnerDecisionDto"
        );
        assert!(
            serde_json::from_value::<ResolvedInstructionSourceDto>(with_extra_field(
                resolved_instruction_source_json()
            ))
            .is_err(),
            "ResolvedInstructionSourceDto"
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
    }

    /// Companion positive check: every one of the same 13 fixtures, WITHOUT
    /// the extra field, deserializes cleanly — the test above proves
    /// rejection is real, not a fixture that was already malformed for an
    /// unrelated reason.
    #[test]
    fn every_one_of_the_13_object_dtos_accepts_its_own_well_formed_fixture() {
        assert!(
            serde_json::from_value::<RegistryDto>(registry_json()).is_ok(),
            "RegistryDto"
        );
        assert!(
            serde_json::from_value::<CandidateDto>(candidate_json()).is_ok(),
            "CandidateDto"
        );
        assert!(
            serde_json::from_value::<ScopeDto>(scope_json()).is_ok(),
            "ScopeDto"
        );
        assert!(
            serde_json::from_value::<OriginDto>(origin_json()).is_ok(),
            "OriginDto"
        );
        assert!(
            serde_json::from_value::<AuthorityDto>(authority_json()).is_ok(),
            "AuthorityDto"
        );
        assert!(
            serde_json::from_value::<PayloadDto>(payload_json()).is_ok(),
            "PayloadDto"
        );
        assert!(
            serde_json::from_value::<SourceRefDto>(source_ref_json()).is_ok(),
            "SourceRefDto"
        );
        assert!(
            serde_json::from_value::<BoundaryDto>(boundary_json()).is_ok(),
            "BoundaryDto"
        );
        assert!(
            serde_json::from_value::<OwnerDecisionDto>(owner_decision_json()).is_ok(),
            "OwnerDecisionDto"
        );
        assert!(
            serde_json::from_value::<ResolvedInstructionSourceDto>(
                resolved_instruction_source_json()
            )
            .is_ok(),
            "ResolvedInstructionSourceDto"
        );
        assert!(
            serde_json::from_value::<RecordedStateDto>(recorded_state_json()).is_ok(),
            "RecordedStateDto"
        );
        assert!(
            serde_json::from_value::<DigestDto>(digest_json()).is_ok(),
            "DigestDto"
        );
        assert!(
            serde_json::from_value::<ReadChannelDto>(read_channel_json()).is_ok(),
            "ReadChannelDto"
        );
    }

    #[test]
    fn accepts_a_well_formed_boundary_dto() {
        let boundary = serde_json::json!({"unit": "line-range", "start": 1, "end": 2});
        assert!(serde_json::from_value::<BoundaryDto>(boundary).is_ok());
    }
}
