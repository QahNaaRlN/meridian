//! Closed transport DTOs for an `existing-project-compatibility-mode`
//! document
//! (`registries/operating-model/existing-project-compatibility-mode.schema.json`,
//! `registries/operating-model/scoped-record.schema.json`). Every
//! object-shaped DTO here is `#[serde(deny_unknown_fields)]`.
//!
//! `discovered_sources` and `rule_candidates[].candidate` are deliberately
//! left as raw [`serde_json::Value`] here (`rust-architecture-conformance-2`,
//! §4): they are never independently typed at THIS module's own boundary —
//! they compose with the REAL `instruction-source-registry` and
//! `controlled-rule-intake` transport/domain pipelines in
//! [`super::convert`]/[`super::mod`], which is the ONE place each is ever
//! parsed. Declaring a second, competing typed shape for either here would
//! be exactly the "повторно объявлять DTO другого контракта" this package
//! forbids.

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub struct RegistryDto {
    #[serde(rename = "$schema", default)]
    pub schema_ref: Option<String>,
    pub schema_version: i64,
    pub registry_id: String,
    pub title: String,
    pub workspace_connections: Vec<ConnectionEntryDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectionEntryDto {
    #[serde(rename = "$schema", default)]
    #[allow(dead_code)]
    pub schema_ref: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub schema_version: Option<i64>,
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
    pub connection_mode: String,
    #[serde(default)]
    pub managed_mode_decision: Option<ManagedModeDecisionDto>,
    pub repository: RepositoryRefDto,
    pub scan_kind: String,
    #[serde(default)]
    pub discovery_plan: Vec<PlanSlotDto>,
    #[serde(default)]
    pub discovered_sources: Vec<Value>,
    #[serde(default)]
    pub missing_sources: Vec<MissingSourceDto>,
    #[serde(default)]
    pub unreadable_sources: Vec<UnreadableSourceDto>,
    #[serde(default)]
    pub rule_candidates: Vec<RuleCandidateEntryDto>,
    #[serde(default)]
    pub findings: Vec<FindingDto>,
    pub next_step: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositoryRefDto {
    pub id: String,
    pub workspace_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedModeDecisionDto {
    #[allow(dead_code)]
    pub decision_ref: String,
    pub decided_at: String,
    #[allow(dead_code)]
    pub reason: String,
    pub authority: ManagedModeAuthorityDto,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedModeAuthorityDto {
    pub kind: String,
    pub authority_ref: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanSlotDto {
    pub id: String,
    pub medium: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub container_ref: Option<String>,
    #[serde(default)]
    pub service_ref: Option<String>,
    #[serde(default)]
    pub resource_ref: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MissingSourceDto {
    pub plan_id: String,
    pub previously_known: bool,
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnreadableSourceDto {
    pub plan_id: String,
    pub reason: String,
    pub previously_known: bool,
    #[serde(default)]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleCandidateEntryDto {
    pub discovery_status: String,
    pub candidate: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FindingDto {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub detail: Option<String>,
    #[serde(default)]
    pub plan_id: Option<String>,
    pub blocking: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot_json() -> Value {
        serde_json::json!({"id": "slot-1", "medium": "file", "path": "a.md", "container_ref": "container-1"})
    }
    fn missing_json() -> Value {
        serde_json::json!({"plan_id": "slot-1", "previously_known": false})
    }
    fn unreadable_json() -> Value {
        serde_json::json!({"plan_id": "slot-1", "reason": "permission denied", "previously_known": false})
    }
    fn finding_json() -> Value {
        serde_json::json!({"id": "finding-1", "kind": "other", "detail": "x", "blocking": false})
    }
    fn managed_decision_json() -> Value {
        serde_json::json!({"decision_ref": "x", "decided_at": "2026-09-21", "reason": "y", "authority": {"kind": "project-owner", "authority_ref": "owner:1"}})
    }
    fn repository_json() -> Value {
        serde_json::json!({"id": "sample-repository", "workspace_id": "sample-workspace"})
    }

    fn with_extra_field(mut v: Value) -> Value {
        v.as_object_mut()
            .expect("fixture must be an object")
            .insert("extra_unknown_field".to_string(), serde_json::json!(1));
        v
    }

    #[test]
    fn deny_unknown_fields_rejects_an_extra_field_on_every_own_object_dto() {
        assert!(serde_json::from_value::<PlanSlotDto>(with_extra_field(slot_json())).is_err());
        assert!(
            serde_json::from_value::<MissingSourceDto>(with_extra_field(missing_json())).is_err()
        );
        assert!(
            serde_json::from_value::<UnreadableSourceDto>(with_extra_field(unreadable_json()))
                .is_err()
        );
        assert!(serde_json::from_value::<FindingDto>(with_extra_field(finding_json())).is_err());
        assert!(
            serde_json::from_value::<ManagedModeDecisionDto>(with_extra_field(
                managed_decision_json()
            ))
            .is_err()
        );
        assert!(
            serde_json::from_value::<RepositoryRefDto>(with_extra_field(repository_json()))
                .is_err()
        );
    }

    #[test]
    fn every_own_object_dto_accepts_its_own_well_formed_fixture() {
        assert!(serde_json::from_value::<PlanSlotDto>(slot_json()).is_ok());
        assert!(serde_json::from_value::<MissingSourceDto>(missing_json()).is_ok());
        assert!(serde_json::from_value::<UnreadableSourceDto>(unreadable_json()).is_ok());
        assert!(serde_json::from_value::<FindingDto>(finding_json()).is_ok());
        assert!(serde_json::from_value::<ManagedModeDecisionDto>(managed_decision_json()).is_ok());
        assert!(serde_json::from_value::<RepositoryRefDto>(repository_json()).is_ok());
    }
}
