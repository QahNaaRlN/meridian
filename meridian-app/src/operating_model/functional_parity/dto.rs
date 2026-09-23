//! Closed transport DTOs for one `functional-parity` evidence document
//! (`verification/functional-parity/functional-parity-evidence.schema.json`).
//! Every object-shaped DTO here is `#[serde(deny_unknown_fields)]`: an
//! unknown field is a transport-level rejection, not silently ignored. This
//! is level 1 of the boundary sequence
//! (`standards/workspace/rust-migration-quality.md` §4) — parsed only for a
//! document that has ALREADY passed the whole-document JSON Schema pass, so
//! every string field here is schema-guaranteed to already satisfy its
//! closed set or pattern; [`super::convert`] still validates defensively on
//! the way into [`meridian_core::functional_parity`]'s typed values rather
//! than trusting that guarantee blindly.
//!
//! Every field below is read by [`super::convert`] into a
//! `meridian_core::functional_parity` typed value (corrective round item
//! 1): `#[allow(dead_code)]` marks ONLY the two envelope/transport fields
//! that identify the CONTRACT itself rather than carry evidence business
//! content (`$schema`, `schema_version` — the same line
//! `task_specification.rs` already draws around its own `$schema` handling)
//! — it is never used as a way to silently drop a business-meaningful
//! field.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentDto {
    #[serde(rename = "$schema")]
    #[allow(dead_code)]
    pub schema_ref: String,
    #[allow(dead_code)]
    pub schema_version: i64,
    pub records: Vec<RecordDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordDto {
    pub work_item: String,
    pub preserved_contract: PreservedContractDto,
    pub baseline: BaselineDto,
    pub post_change_evidence: PostChangeEvidenceDto,
    pub evidence: Vec<EvidenceEntryDto>,
    pub gaps: Vec<GapDto>,
    pub verdict: VerdictDto,
    pub recorded_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreservedContractDto {
    pub public_api: FacetDto,
    pub observable_io: FacetDto,
    pub side_effects_and_interactions: FacetDto,
    pub user_visible_behavior: FacetDto,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FacetDto {
    #[serde(default)]
    pub assertions: Option<Vec<AssertionDto>>,
    #[serde(default)]
    pub not_applicable_justification: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssertionDto {
    pub id: String,
    pub statement: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateRefDto {
    pub identifier: String,
    pub identifier_kind: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentifiedConditionDto {
    pub id: String,
    pub description: String,
    #[serde(default)]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvenanceDto {
    pub method: String,
    pub established: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservedResultDto {
    pub summary: String,
    #[serde(default)]
    pub artifacts: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaselineDto {
    pub source_state: StateRefDto,
    pub inputs_and_conditions: Vec<IdentifiedConditionDto>,
    pub provenance: ProvenanceDto,
    pub observed_result: ObservedResultDto,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConditionDto {
    pub description: String,
    #[serde(default)]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputsAndConditionsDto {
    pub relationship: String,
    #[serde(default)]
    pub baseline_condition_ids: Option<Vec<String>>,
    #[serde(default)]
    pub comparability_justification: Option<String>,
    #[serde(default)]
    pub items: Option<Vec<ConditionDto>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractLinkDto {
    pub facet: String,
    pub assertion_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostChangeEvidenceDto {
    pub source_state: StateRefDto,
    pub inputs_and_conditions: InputsAndConditionsDto,
    pub observed_result: ObservedResultDto,
    pub contract_links: Vec<ContractLinkDto>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceEntryDto {
    pub kind: String,
    pub observed_scope: String,
    pub covers: Vec<String>,
    pub limitations: Vec<String>,
    #[serde(default)]
    pub applicability_justification: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GapDto {
    pub description: String,
    pub scope: String,
    #[serde(default)]
    pub assertion_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssertionVerdictDto {
    pub assertion_id: String,
    pub state: String,
    #[serde(default)]
    pub unverified_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerdictDto {
    pub per_assertion: Vec<AssertionVerdictDto>,
    pub overall: String,
    #[serde(default)]
    pub overall_unverified_reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_ref_json() -> serde_json::Value {
        serde_json::json!({"identifier": "state-A", "identifier_kind": "described-source-revision"})
    }
    fn provenance_json() -> serde_json::Value {
        serde_json::json!({"method": "m", "established": true})
    }
    fn observed_result_json() -> serde_json::Value {
        serde_json::json!({"summary": "s"})
    }
    fn baseline_json() -> serde_json::Value {
        serde_json::json!({
            "source_state": state_ref_json(),
            "inputs_and_conditions": [{"id": "cond.a", "description": "d"}],
            "provenance": provenance_json(),
            "observed_result": observed_result_json(),
        })
    }
    fn post_change_json() -> serde_json::Value {
        serde_json::json!({
            "source_state": {"identifier": "state-B", "identifier_kind": "described-source-revision"},
            "inputs_and_conditions": {"relationship": "same", "baseline_condition_ids": ["cond.a"]},
            "observed_result": observed_result_json(),
            "contract_links": [{"facet": "public_api", "assertion_id": "api.a"}],
        })
    }
    fn evidence_json() -> serde_json::Value {
        serde_json::json!({
            "kind": "interface-enumeration",
            "observed_scope": "scope",
            "covers": ["api.a"],
            "limitations": ["limit"],
        })
    }
    fn verdict_json() -> serde_json::Value {
        serde_json::json!({
            "per_assertion": [{"assertion_id": "api.a", "state": "VERIFIED"}],
            "overall": "VERIFIED",
        })
    }
    fn facet_json() -> serde_json::Value {
        serde_json::json!({"assertions": [{"id": "api.a", "statement": "s"}]})
    }
    fn record_json() -> serde_json::Value {
        serde_json::json!({
            "work_item": "wi-1",
            "preserved_contract": {
                "public_api": facet_json(),
                "observable_io": {"not_applicable_justification": "n/a"},
                "side_effects_and_interactions": {"not_applicable_justification": "n/a"},
                "user_visible_behavior": {"not_applicable_justification": "n/a"},
            },
            "baseline": baseline_json(),
            "post_change_evidence": post_change_json(),
            "evidence": [evidence_json()],
            "gaps": [],
            "verdict": verdict_json(),
            "recorded_at": "2026-09-23",
        })
    }
    fn document_json() -> serde_json::Value {
        serde_json::json!({
            "$schema": "./functional-parity-evidence.schema.json",
            "schema_version": 1,
            "records": [record_json()],
        })
    }

    fn with_extra_field(mut v: serde_json::Value) -> serde_json::Value {
        v.as_object_mut()
            .expect("fixture must be an object")
            .insert("extra_unknown_field".to_string(), serde_json::json!(1));
        v
    }

    #[test]
    fn every_object_dto_accepts_its_own_well_formed_fixture() {
        assert!(serde_json::from_value::<DocumentDto>(document_json()).is_ok());
        assert!(serde_json::from_value::<RecordDto>(record_json()).is_ok());
        assert!(serde_json::from_value::<PreservedContractDto>(
            document_json()["records"][0]["preserved_contract"].clone()
        )
        .is_ok());
        assert!(serde_json::from_value::<FacetDto>(facet_json()).is_ok());
        assert!(serde_json::from_value::<StateRefDto>(state_ref_json()).is_ok());
        assert!(serde_json::from_value::<ProvenanceDto>(provenance_json()).is_ok());
        assert!(serde_json::from_value::<ObservedResultDto>(observed_result_json()).is_ok());
        assert!(serde_json::from_value::<BaselineDto>(baseline_json()).is_ok());
        assert!(serde_json::from_value::<PostChangeEvidenceDto>(post_change_json()).is_ok());
        assert!(serde_json::from_value::<EvidenceEntryDto>(evidence_json()).is_ok());
        assert!(serde_json::from_value::<VerdictDto>(verdict_json()).is_ok());
    }

    /// Every object-shaped DTO this module declares rejects one added
    /// unknown field on its own well-formed fixture.
    #[test]
    fn deny_unknown_fields_rejects_an_extra_field_on_every_object_dto() {
        assert!(
            serde_json::from_value::<DocumentDto>(with_extra_field(document_json())).is_err(),
            "DocumentDto"
        );
        assert!(
            serde_json::from_value::<RecordDto>(with_extra_field(record_json())).is_err(),
            "RecordDto"
        );
        assert!(
            serde_json::from_value::<FacetDto>(with_extra_field(facet_json())).is_err(),
            "FacetDto"
        );
        assert!(
            serde_json::from_value::<StateRefDto>(with_extra_field(state_ref_json())).is_err(),
            "StateRefDto"
        );
        assert!(
            serde_json::from_value::<ProvenanceDto>(with_extra_field(provenance_json())).is_err(),
            "ProvenanceDto"
        );
        assert!(
            serde_json::from_value::<ObservedResultDto>(with_extra_field(observed_result_json()))
                .is_err(),
            "ObservedResultDto"
        );
        assert!(
            serde_json::from_value::<BaselineDto>(with_extra_field(baseline_json())).is_err(),
            "BaselineDto"
        );
        assert!(
            serde_json::from_value::<PostChangeEvidenceDto>(with_extra_field(post_change_json()))
                .is_err(),
            "PostChangeEvidenceDto"
        );
        assert!(
            serde_json::from_value::<EvidenceEntryDto>(with_extra_field(evidence_json())).is_err(),
            "EvidenceEntryDto"
        );
        assert!(
            serde_json::from_value::<VerdictDto>(with_extra_field(verdict_json())).is_err(),
            "VerdictDto"
        );
        assert!(
            serde_json::from_value::<AssertionDto>(with_extra_field(
                serde_json::json!({"id": "a", "statement": "s"})
            ))
            .is_err(),
            "AssertionDto"
        );
        assert!(
            serde_json::from_value::<IdentifiedConditionDto>(with_extra_field(
                serde_json::json!({"id": "a", "description": "d"})
            ))
            .is_err(),
            "IdentifiedConditionDto"
        );
        assert!(
            serde_json::from_value::<InputsAndConditionsDto>(with_extra_field(
                serde_json::json!({"relationship": "same", "baseline_condition_ids": ["a"]})
            ))
            .is_err(),
            "InputsAndConditionsDto"
        );
        assert!(
            serde_json::from_value::<ContractLinkDto>(with_extra_field(
                serde_json::json!({"facet": "public_api", "assertion_id": "a"})
            ))
            .is_err(),
            "ContractLinkDto"
        );
        assert!(
            serde_json::from_value::<GapDto>(with_extra_field(
                serde_json::json!({"description": "d", "scope": "record"})
            ))
            .is_err(),
            "GapDto"
        );
        assert!(
            serde_json::from_value::<AssertionVerdictDto>(with_extra_field(
                serde_json::json!({"assertion_id": "a", "state": "VERIFIED"})
            ))
            .is_err(),
            "AssertionVerdictDto"
        );
        assert!(
            serde_json::from_value::<ConditionDto>(with_extra_field(
                serde_json::json!({"description": "d"})
            ))
            .is_err(),
            "ConditionDto"
        );
    }
}
