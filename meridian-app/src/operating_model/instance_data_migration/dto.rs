//! Closed transport DTOs of `instance-data-migration.schema.json` and their
//! total conversion into `meridian_core::migration::plan::PlanInput`. Every
//! `Err` is drift: the document already passed the schema.

use meridian_core::migration::plan::{
    DeclaredContent, DispositionInput, FieldBasisInput, FieldBasisValue, MappingInput,
    OwnerDecisionInput, PlanInput, PlanPayloadInput, Qualification, RecordUnitInput, RollbackInput,
    RollbackPlanInput, SourceInput, SubVerdictInput, TargetAuthorityInput, TargetInput,
    VerificationInput,
};
use meridian_core::migration::Encoding;
use meridian_core::resolver::IsoDate;
use serde::Deserialize;

use super::super::migration_boundary::head::{
    authority_kind, digest, head, optional_record_text, scope, sha256_hex, verdict, DigestDto,
    HeadParts, ScopeDto,
};
use super::super::run_contract_boundary::envelope::{
    semantic_id, text, AuthorityDto, Converted, OriginDto,
};

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
    migration_plans: Vec<PlanEntryDto>,
}

impl ContainerDto {
    pub(super) fn into_inputs(self) -> Converted<Vec<PlanInput>> {
        self.migration_plans
            .into_iter()
            .enumerate()
            .map(|(i, e)| {
                e.into_input()
                    .map_err(|m| format!("migration_plans[{i}]: {m}"))
            })
            .collect()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlanEntryDto {
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

impl PlanEntryDto {
    fn into_input(self) -> Converted<PlanInput> {
        Ok(PlanInput {
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
            payload: self.payload.into_input()?,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceDto {
    revision: String,
    digest: DigestDto,
    repository_ref: String,
    working_tree_clean: bool,
    qualification: String,
    #[serde(default)]
    qualification_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordUnitDto {
    id: String,
    unit_ref: String,
    classification_basis: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FieldBasisDto {
    #[serde(rename = "$schema")]
    schema: String,
    id: String,
    title: String,
    record_type: String,
    scope: String,
    schema_version: String,
    origin: String,
    authority: String,
    payload: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetOriginDto {
    kind: String,
    source_ref: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetAuthorityDto {
    kind: String,
    authority_ref: String,
    decision_ref: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContentDto {
    media_type: String,
    encoding: String,
    content: String,
    digest: DigestDto,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TargetDto {
    #[serde(rename = "$schema")]
    schema_ref: String,
    id: String,
    title: String,
    record_type: String,
    scope: ScopeDto,
    schema_version: u64,
    field_basis: FieldBasisDto,
    origin: TargetOriginDto,
    authority: TargetAuthorityDto,
    payload: ContentDto,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerDecisionDto {
    decision_ref: String,
    decided_at: String,
    reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MappingDto {
    unit_id: String,
    disposition: String,
    #[serde(default)]
    target: Option<TargetDto>,
    #[serde(default)]
    merge_rule_ref: Option<String>,
    #[serde(default)]
    retained_reason: Option<String>,
    #[serde(default)]
    owner_decision: Option<OwnerDecisionDto>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RollbackDto {
    source_snapshot_ref: String,
    plan: String,
    #[serde(default)]
    deterministic_plan_ref: Option<String>,
    #[serde(default)]
    restoration_evidence_ref: Option<String>,
    rewrites_published_history: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SubVerdictDto {
    status: String,
    #[serde(default)]
    evidence_ref: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerificationDto {
    coverage: SubVerdictDto,
    applicability_preservation: SubVerdictDto,
    overall_status: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PayloadDto {
    source: SourceDto,
    record_units: Vec<RecordUnitDto>,
    mappings: Vec<MappingDto>,
    rollback: RollbackDto,
    verification: VerificationDto,
    plan_fingerprint: String,
    idempotency_key: String,
    #[serde(default)]
    supersedes: Option<String>,
}

fn basis(field: &str, value: &str) -> Converted<FieldBasisValue> {
    FieldBasisValue::parse(value)
        .ok_or_else(|| format!("{field}: \"{value}\" is outside the closed pool"))
}

impl FieldBasisDto {
    fn into_input(self) -> Converted<FieldBasisInput> {
        Ok(FieldBasisInput {
            schema: basis("field_basis.$schema", &self.schema)?,
            id: basis("field_basis.id", &self.id)?,
            title: basis("field_basis.title", &self.title)?,
            record_type: basis("field_basis.record_type", &self.record_type)?,
            scope: basis("field_basis.scope", &self.scope)?,
            schema_version: basis("field_basis.schema_version", &self.schema_version)?,
            origin: basis("field_basis.origin", &self.origin)?,
            authority: basis("field_basis.authority", &self.authority)?,
            payload: basis("field_basis.payload", &self.payload)?,
        })
    }
}

fn content(dto: ContentDto) -> Converted<DeclaredContent> {
    Ok(DeclaredContent {
        media_type: text("payload.media_type", dto.media_type)?,
        encoding: Encoding::parse(&dto.encoding).ok_or_else(|| {
            format!(
                "payload.encoding: \"{}\" is outside the closed pool",
                dto.encoding
            )
        })?,
        content: text("payload.content", dto.content)?,
        digest: digest("payload.digest", dto.digest)?,
    })
}

impl TargetDto {
    fn into_input(self) -> Converted<TargetInput> {
        if self.schema_version != 1 {
            return Err(format!(
                "target.schema_version: {} is not 1",
                self.schema_version
            ));
        }
        if self.origin.kind != "migrated" {
            return Err(format!(
                "target.origin.kind: \"{}\" is not migrated",
                self.origin.kind
            ));
        }
        Ok(TargetInput {
            schema_ref: text("target.$schema", self.schema_ref)?,
            id: semantic_id("target.id", self.id)?,
            title: text("target.title", self.title)?,
            record_type: semantic_id("target.record_type", self.record_type)?,
            scope: scope("target.scope", self.scope)?,
            field_basis: self.field_basis.into_input()?,
            origin_source_ref: text("target.origin.source_ref", self.origin.source_ref)?,
            authority: TargetAuthorityInput {
                kind: authority_kind("target.authority.kind", &self.authority.kind)?,
                authority_ref: text(
                    "target.authority.authority_ref",
                    self.authority.authority_ref,
                )?,
                decision_ref: text("target.authority.decision_ref", self.authority.decision_ref)?,
            },
            payload: content(self.payload)?,
        })
    }
}

impl OwnerDecisionDto {
    fn into_input(self) -> Converted<OwnerDecisionInput> {
        Ok(OwnerDecisionInput {
            decision_ref: text("owner_decision.decision_ref", self.decision_ref)?,
            decided_at: IsoDate::new(self.decided_at)
                .map_err(|e| format!("owner_decision.decided_at: {e}"))?,
            reason: text("owner_decision.reason", self.reason)?,
        })
    }
}

impl MappingDto {
    fn into_input(self) -> Converted<MappingInput> {
        let unit_id = semantic_id("mapping.unit_id", self.unit_id)?;
        let minted =
            |target: Option<TargetDto>, decision: Option<OwnerDecisionDto>| -> Converted<_> {
                let target = target.ok_or("mapping.target: absent for a minting disposition")?;
                let decision =
                    decision.ok_or("mapping.owner_decision: absent for a minting disposition")?;
                Ok((target.into_input()?, decision.into_input()?))
            };
        let disposition = match self.disposition.as_str() {
            "migrated" => {
                if self.merge_rule_ref.is_some() || self.retained_reason.is_some() {
                    return Err("mapping: a migrated disposition carries a forbidden field".into());
                }
                let (target, owner_decision) = minted(self.target, self.owner_decision)?;
                DispositionInput::Migrated {
                    target,
                    owner_decision,
                }
            }
            "merged" => {
                if self.retained_reason.is_some() {
                    return Err("mapping: a merged disposition carries retained_reason".into());
                }
                let rule = self
                    .merge_rule_ref
                    .ok_or("mapping.merge_rule_ref: absent for a merged disposition")?;
                let (target, owner_decision) = minted(self.target, self.owner_decision)?;
                DispositionInput::Merged {
                    target,
                    owner_decision,
                    merge_rule_ref: text("mapping.merge_rule_ref", rule)?,
                }
            }
            "retained-transitional" => {
                if self.target.is_some()
                    || self.merge_rule_ref.is_some()
                    || self.owner_decision.is_some()
                {
                    return Err(
                        "mapping: a retained-transitional disposition carries a forbidden field"
                            .into(),
                    );
                }
                let reason = self
                    .retained_reason
                    .ok_or("mapping.retained_reason: absent for a retained disposition")?;
                DispositionInput::RetainedTransitional {
                    retained_reason: text("mapping.retained_reason", reason)?,
                }
            }
            other => {
                return Err(format!(
                    "mapping.disposition: \"{other}\" is outside the closed pool"
                ))
            }
        };
        Ok(MappingInput {
            unit_id,
            disposition,
        })
    }
}

impl RollbackDto {
    fn into_input(self) -> Converted<RollbackInput> {
        if self.rewrites_published_history {
            return Err("rollback.rewrites_published_history: true".into());
        }
        let plan = match (
            self.plan.as_str(),
            self.deterministic_plan_ref,
            self.restoration_evidence_ref,
        ) {
            ("deterministic-reconstruction", Some(r), None) => {
                RollbackPlanInput::DeterministicReconstruction {
                    deterministic_plan_ref: text("rollback.deterministic_plan_ref", r)?,
                }
            }
            ("verified-restoration", None, Some(r)) => RollbackPlanInput::VerifiedRestoration {
                restoration_evidence_ref: text("rollback.restoration_evidence_ref", r)?,
            },
            (plan, _, _) => {
                return Err(format!(
                    "rollback: plan \"{plan}\" does not carry exactly its own reference"
                ))
            }
        };
        Ok(RollbackInput {
            source_snapshot_ref: text("rollback.source_snapshot_ref", self.source_snapshot_ref)?,
            plan,
        })
    }
}

fn sub_verdict(field: &str, dto: SubVerdictDto) -> Converted<SubVerdictInput> {
    match (dto.status.as_str(), dto.evidence_ref) {
        ("verified", Some(r)) => Ok(SubVerdictInput::Verified {
            evidence_ref: text(&format!("{field}.evidence_ref"), r)?,
        }),
        ("unverified", None) => Ok(SubVerdictInput::Unverified),
        (status, _) => Err(format!(
            "{field}: status \"{status}\" and evidence_ref disagree"
        )),
    }
}

impl PayloadDto {
    fn into_input(self) -> Converted<PlanPayloadInput> {
        let s = self.source;
        let qualification = match (s.qualification.as_str(), s.qualification_reason) {
            ("reproducible", None) if s.working_tree_clean => Qualification::Reproducible,
            ("not-reproducible", Some(reason)) => Qualification::NotReproducible {
                reason: text("source.qualification_reason", reason)?,
            },
            (q, _) => {
                return Err(format!(
                    "source.qualification: \"{q}\" disagrees with its fields"
                ))
            }
        };
        Ok(PlanPayloadInput {
            source: SourceInput {
                revision: text("source.revision", s.revision)?,
                digest: digest("source.digest", s.digest)?,
                repository_ref: text("source.repository_ref", s.repository_ref)?,
                working_tree_clean: s.working_tree_clean,
                qualification,
            },
            record_units: self
                .record_units
                .into_iter()
                .map(|u| {
                    Ok(RecordUnitInput {
                        id: semantic_id("record_unit.id", u.id)?,
                        unit_ref: text("record_unit.unit_ref", u.unit_ref)?,
                        classification_basis: text(
                            "record_unit.classification_basis",
                            u.classification_basis,
                        )?,
                    })
                })
                .collect::<Converted<_>>()?,
            mappings: self
                .mappings
                .into_iter()
                .map(MappingDto::into_input)
                .collect::<Converted<_>>()?,
            rollback: self.rollback.into_input()?,
            verification: VerificationInput {
                coverage: sub_verdict("verification.coverage", self.verification.coverage)?,
                applicability_preservation: sub_verdict(
                    "verification.applicability_preservation",
                    self.verification.applicability_preservation,
                )?,
                overall_status: verdict(
                    "verification.overall_status",
                    &self.verification.overall_status,
                )?,
            },
            plan_fingerprint: sha256_hex("plan_fingerprint", self.plan_fingerprint)?,
            idempotency_key: sha256_hex("idempotency_key", self.idempotency_key)?,
            supersedes: optional_record_text("supersedes", self.supersedes)?,
        })
    }
}
