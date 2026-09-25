//! Converts a schema-validated, transport-parsed [`super::dto`] tree into
//! [`meridian_core::controlled_rule_intake`] domain types (levels 3-4 of
//! `standards/workspace/rust-migration-quality.md` §4).
//!
//! Every function here accumulates ALL of a candidate's independent
//! defects before deciding success or failure — corrective round item 9:
//! a candidate whose `boundary` AND `authority.kind` are both malformed
//! reports both, not just the first one reached. This module's own defects
//! (missing/malformed fields, unknown closed-string values) are collected
//! here; [`RuleCandidate::try_new`] then separately validates the
//! CROSS-field consistency (origin/source link, owner authority) that only
//! it can check, and its own diagnostics are folded into the same
//! accumulator.

use meridian_core::controlled_rule_intake::{
    Applicability, Boundary, NotApplicableReason, PinnedSourceRef, RuleCandidate,
    RuleCandidatePayload, SemanticKey, RECORD_TYPE, SOURCE_RECORD_TYPE,
};
use meridian_core::migration::OwnerDecision;
use meridian_core::resolver::IsoDate;
use meridian_core::types::{
    Authority, AuthorityKind, ContentDigest, Diagnostic, NonEmptyString, Origin, OriginError,
    Revision, Scope, SemanticId, WorkspaceId,
};

use super::dto::{
    AuthorityDto, BoundaryDto, CandidateDto, OriginDto, OwnerDecisionDto, PayloadDto, ScopeDto,
    SourceRefDto,
};
use super::{fail, try_field};

fn build_pinned_source_ref(
    at: &str,
    dto: &SourceRefDto,
) -> Result<PinnedSourceRef, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    if dto.record_type != SOURCE_RECORD_TYPE {
        problems.push(fail(format!(
            "{at} payload.source_ref.record_type \"{}\" is not \"{SOURCE_RECORD_TYPE}\"",
            dto.record_type
        )));
    }
    let id = try_field(&mut problems, at, SemanticId::new(&dto.id));
    let reference = try_field(&mut problems, at, NonEmptyString::new(&dto.reference));
    let revision = try_field(&mut problems, at, Revision::new(&dto.revision));
    let sha256 = try_field(&mut problems, at, ContentDigest::from_hex(&dto.sha256));
    match (id, reference, revision, sha256) {
        (Some(id), Some(reference), Some(revision), Some(sha256)) if problems.is_empty() => {
            Ok(PinnedSourceRef::new(id, reference, revision, sha256))
        }
        _ => Err(problems),
    }
}

fn build_ranged_boundary(
    at: &str,
    dto: &BoundaryDto,
    ctor: impl Fn(u64, u64) -> Result<Boundary, meridian_core::controlled_rule_intake::BoundaryError>,
) -> Result<Boundary, Vec<Diagnostic>> {
    let (Some(start), Some(end)) = (dto.start, dto.end) else {
        return Err(vec![fail(format!(
            "{at} payload.boundary requires both start and end for unit \"{}\"",
            dto.unit
        ))]);
    };
    ctor(start, end).map_err(|e| vec![fail(format!("{at} payload.boundary: {e}"))])
}

fn build_boundary(at: &str, dto: &BoundaryDto) -> Result<Boundary, Vec<Diagnostic>> {
    match dto.unit.as_str() {
        "agents-md-section" => {
            let Some(region) = dto.region.as_deref() else {
                return Err(vec![fail(format!(
                    "{at} payload.boundary.region is required for unit \"agents-md-section\""
                ))]);
            };
            NonEmptyString::new(region)
                .map(Boundary::agents_md_section)
                .map_err(|e| vec![fail(format!("{at} payload.boundary.region: {e}"))])
        }
        "line-range" => build_ranged_boundary(at, dto, Boundary::line_range),
        "byte-range" => build_ranged_boundary(at, dto, Boundary::byte_range),
        "whole-source" => Ok(Boundary::whole_source()),
        other => Err(vec![fail(format!(
            "{at} payload.boundary.unit \"{other}\" is not one of the four closed units"
        ))]),
    }
}

fn build_owner_decision(
    at: &str,
    dto: &OwnerDecisionDto,
) -> Result<OwnerDecision, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let decision_ref = try_field(&mut problems, at, NonEmptyString::new(&dto.decision_ref));
    let decided_at = try_field(&mut problems, at, IsoDate::new(&dto.decided_at));
    let reason = try_field(&mut problems, at, NonEmptyString::new(&dto.reason));
    match (decision_ref, decided_at, reason) {
        (Some(decision_ref), Some(decided_at), Some(reason)) if problems.is_empty() => {
            Ok(OwnerDecision::new(decision_ref, decided_at, reason))
        }
        _ => Err(problems),
    }
}

fn build_applicability(at: &str, dto: &PayloadDto) -> Result<Applicability, Vec<Diagnostic>> {
    match dto.applicability_state.as_str() {
        "candidate" => Ok(Applicability::Candidate),
        "accepted" => {
            let Some(od_dto) = &dto.owner_decision else {
                return Err(vec![fail(format!("{at} payload.owner_decision is required when applicability_state is \"accepted\""))]);
            };
            build_owner_decision(at, od_dto)
                .map(|owner_decision| Applicability::Accepted { owner_decision })
        }
        "not-applicable" => {
            let mut problems = Vec::new();
            let owner_decision = match &dto.owner_decision {
                Some(od_dto) => match build_owner_decision(at, od_dto) {
                    Ok(v) => Some(v),
                    Err(errs) => {
                        problems.extend(errs);
                        None
                    }
                },
                None => {
                    problems.push(fail(format!("{at} payload.owner_decision is required when applicability_state is \"not-applicable\"")));
                    None
                }
            };
            let reason = match dto.not_applicable_reason.as_deref() {
                Some(r) => match NotApplicableReason::parse(r) {
                    Some(v) => Some(v),
                    None => {
                        problems.push(fail(format!("{at} payload.not_applicable_reason \"{r}\" is not one of the two closed reasons")));
                        None
                    }
                },
                None => {
                    problems.push(fail(format!("{at} payload.not_applicable_reason is required when applicability_state is \"not-applicable\"")));
                    None
                }
            };
            match (owner_decision, reason) {
                (Some(owner_decision), Some(reason)) => Ok(Applicability::NotApplicable {
                    owner_decision,
                    reason,
                }),
                _ => Err(problems),
            }
        }
        other => Err(vec![fail(format!(
            "{at} payload.applicability_state \"{other}\" is not one of the three closed states"
        ))]),
    }
}

fn build_payload(at: &str, dto: &PayloadDto) -> Result<RuleCandidatePayload, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let source_ref = match build_pinned_source_ref(at, &dto.source_ref) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    let boundary = match build_boundary(at, &dto.boundary) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    let raw_excerpt = try_field(&mut problems, at, NonEmptyString::new(&dto.raw_excerpt));
    let normalized_text = try_field(&mut problems, at, NonEmptyString::new(&dto.normalized_text));
    let semantic_key = try_field(&mut problems, at, SemanticKey::new(&dto.semantic_key));
    let classification_basis = try_field(
        &mut problems,
        at,
        NonEmptyString::new(&dto.classification_basis),
    );
    let mut conflicts_with = Vec::new();
    for c in &dto.conflicts_with {
        match SemanticKey::new(c) {
            Ok(k) => conflicts_with.push(k),
            Err(e) => problems.push(fail(format!("{at} payload.conflicts_with: {e}"))),
        }
    }
    let applicability = match build_applicability(at, dto) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };

    match (
        source_ref,
        boundary,
        raw_excerpt,
        normalized_text,
        semantic_key,
        classification_basis,
        applicability,
    ) {
        (
            Some(source_ref),
            Some(boundary),
            Some(raw_excerpt),
            Some(normalized_text),
            Some(semantic_key),
            Some(classification_basis),
            Some(applicability),
        ) if problems.is_empty() => Ok(RuleCandidatePayload::new(
            source_ref,
            boundary,
            raw_excerpt,
            normalized_text,
            semantic_key,
            classification_basis,
            conflicts_with,
            applicability,
        )),
        _ => Err(problems),
    }
}

fn required_workspace_id(
    problems: &mut Vec<Diagnostic>,
    at: &str,
    value: &Option<String>,
    field: &str,
) -> Option<WorkspaceId> {
    match value {
        None => {
            problems.push(fail(format!("{at} {field} is required")));
            None
        }
        Some(s) => try_field(problems, at, WorkspaceId::new(s)),
    }
}

fn build_scope(at: &str, dto: &ScopeDto) -> Result<Scope, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    match dto.scope_type.as_str() {
        "built-in-methodology" => Ok(Scope::built_in_methodology()),
        "user-profile" => match SemanticId::new(&dto.id) {
            Ok(id) => Ok(Scope::user_profile(id)),
            Err(e) => Err(vec![fail(format!("{at} scope.id: {e}"))]),
        },
        "organization-profile" => match SemanticId::new(&dto.id) {
            Ok(id) => Ok(Scope::organization_profile(id)),
            Err(e) => Err(vec![fail(format!("{at} scope.id: {e}"))]),
        },
        "project-workspace" => {
            let id = try_field(&mut problems, at, SemanticId::new(&dto.id));
            let organization_profile_id = match &dto.organization_profile_id {
                None => Some(None),
                Some(s) => match SemanticId::new(s) {
                    Ok(v) => Some(Some(v)),
                    Err(e) => {
                        problems.push(fail(format!("{at} scope.organization_profile_id: {e}")));
                        None
                    }
                },
            };
            match (id, organization_profile_id) {
                (Some(id), Some(organization_profile_id)) if problems.is_empty() => {
                    Ok(Scope::project_workspace(id, organization_profile_id))
                }
                _ => Err(problems),
            }
        }
        "repository-scope" => {
            let id = try_field(&mut problems, at, SemanticId::new(&dto.id));
            let workspace_id =
                required_workspace_id(&mut problems, at, &dto.workspace_id, "scope.workspace_id");
            match (id, workspace_id) {
                (Some(id), Some(workspace_id)) if problems.is_empty() => {
                    Ok(Scope::repository_scope(id, workspace_id))
                }
                _ => Err(problems),
            }
        }
        "run-state" => {
            let id = try_field(&mut problems, at, SemanticId::new(&dto.id));
            let workspace_id =
                required_workspace_id(&mut problems, at, &dto.workspace_id, "scope.workspace_id");
            match (id, workspace_id) {
                (Some(id), Some(workspace_id)) if problems.is_empty() => {
                    Ok(Scope::run_state(id, workspace_id))
                }
                _ => Err(problems),
            }
        }
        other => Err(vec![fail(format!(
            "{at} scope.type \"{other}\" is not one of the six closed areas"
        ))]),
    }
}

fn build_origin_with_source_ref(
    at: &str,
    dto: &OriginDto,
    ctor: impl Fn(&str) -> Result<Origin, OriginError>,
) -> Result<Origin, Vec<Diagnostic>> {
    let Some(source_ref) = dto.source_ref.as_deref() else {
        return Err(vec![fail(format!(
            "{at} origin.source_ref is required for origin.kind \"{}\"",
            dto.kind
        ))]);
    };
    ctor(source_ref).map_err(|e| vec![fail(format!("{at} origin.source_ref: {e}"))])
}

fn build_origin(at: &str, dto: &OriginDto) -> Result<Origin, Vec<Diagnostic>> {
    match dto.kind.as_str() {
        "built-in" => Ok(Origin::built_in()),
        "declared" => build_origin_with_source_ref(at, dto, |s| Origin::declared(s)),
        "migrated" => build_origin_with_source_ref(at, dto, |s| Origin::migrated(s)),
        "imported" => build_origin_with_source_ref(at, dto, |s| Origin::imported(s)),
        "derived" => build_origin_with_source_ref(at, dto, |s| Origin::derived(s)),
        other => Err(vec![fail(format!(
            "{at} origin.kind \"{other}\" is not one of the five closed kinds"
        ))]),
    }
}

fn parse_authority_kind(value: &str) -> Option<AuthorityKind> {
    match value {
        "methodology-owner" => Some(AuthorityKind::MethodologyOwner),
        "user" => Some(AuthorityKind::User),
        "organization" => Some(AuthorityKind::Organization),
        "project-owner" => Some(AuthorityKind::ProjectOwner),
        "repository-maintainer" => Some(AuthorityKind::RepositoryMaintainer),
        "delegated-run" => Some(AuthorityKind::DelegatedRun),
        _ => None,
    }
}

fn build_authority(at: &str, dto: &AuthorityDto) -> Result<Authority, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let Some(kind) = parse_authority_kind(&dto.kind) else {
        return Err(vec![fail(format!(
            "{at} authority.kind \"{}\" is not one of the six closed kinds",
            dto.kind
        ))]);
    };
    match Authority::new(kind, dto.authority_ref.clone(), dto.decision_ref.clone()) {
        Ok(a) => {
            if problems.is_empty() {
                Ok(a)
            } else {
                Err(problems)
            }
        }
        Err(e) => {
            problems.push(fail(format!("{at} authority: {e}")));
            Err(problems)
        }
    }
}

/// Builds one [`RuleCandidate`] from its transport DTO, accumulating every
/// independent defect (corrective round item 9) before deciding success —
/// a candidate whose `scope`/`origin`/`authority`/`payload` all have their
/// own separate defects reports all of them. Only when every sub-part
/// parses does this call [`RuleCandidate::try_new`], which then separately
/// validates cross-field consistency and may itself add diagnostics.
pub(crate) fn build_candidate(dto: &CandidateDto) -> Result<RuleCandidate, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let at = format!("rule candidate \"{}\"", dto.id);

    if dto.record_type != RECORD_TYPE {
        problems.push(fail(format!(
            "{at} declares record_type \"{}\", not \"{RECORD_TYPE}\"",
            dto.record_type
        )));
    }
    let id = try_field(&mut problems, &at, SemanticId::new(&dto.id));
    let scope = match build_scope(&at, &dto.scope) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    let origin = match build_origin(&at, &dto.origin) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    let authority = match build_authority(&at, &dto.authority) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    let payload = match build_payload(&at, &dto.payload) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };

    let (Some(id), Some(scope), Some(origin), Some(authority), Some(payload)) =
        (id, scope, origin, authority, payload)
    else {
        return Err(problems);
    };

    match RuleCandidate::try_new(id, scope, origin, authority, payload) {
        Ok(candidate) if problems.is_empty() => Ok(candidate),
        Ok(_) => Err(problems),
        Err(cross_field_problems) => {
            problems.extend(cross_field_problems);
            Err(problems)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operating_model::controlled_rule_intake::dto::RegistryDto;

    fn candidate_dto() -> CandidateDto {
        let doc = serde_json::json!({
            "id": "sample-candidate",
            "record_type": "rule-candidate",
            "scope": {"type": "repository-scope", "id": "sample-repository", "workspace_id": "sample-workspace"},
            "origin": {"kind": "derived", "source_ref": "instruction-source:src-1"},
            "authority": {"kind": "delegated-run", "authority_ref": "controlled-rule-intake-run:sample-pass-1"},
            "payload": {
                "source_ref": {"record_type": "instruction-source", "id": "src-1", "reference": "sources/src-1", "revision": "a".repeat(40), "sha256": "b".repeat(64)},
                "boundary": {"unit": "whole-source"},
                "raw_excerpt": "Branches follow feature/<slug>.",
                "normalized_text": "branches follow feature slug",
                "semantic_key": "branch-naming-feature-slug",
                "classification_basis": "the repository owns this rule",
                "applicability_state": "candidate",
            },
        });
        serde_json::from_value(doc).unwrap()
    }

    #[test]
    fn a_well_formed_candidate_dto_builds_cleanly() {
        assert!(build_candidate(&candidate_dto()).is_ok());
    }

    /// Independent defects in `boundary` AND `authority.kind` are BOTH
    /// reported, not just the first (corrective round item 9).
    #[test]
    fn multiple_independent_field_defects_are_all_accumulated() {
        let mut dto = candidate_dto();
        dto.payload.boundary.unit = "line-range".to_string();
        dto.payload.boundary.start = Some(10);
        dto.payload.boundary.end = Some(5);
        dto.authority.kind = "invented-kind".to_string();
        let errors = build_candidate(&dto).unwrap_err();
        assert!(
            errors.iter().any(|d| d.message().contains("start")),
            "{errors:?}"
        );
        assert!(
            errors.iter().any(|d| d.message().contains("invented-kind")),
            "{errors:?}"
        );
    }

    #[test]
    fn deserializing_the_whole_registry_dto_round_trips() {
        let doc = serde_json::json!({
            "schema_version": 1,
            "registry_id": "controlled-rule-intake",
            "title": "sample registry",
            "rule_candidates": [],
        });
        let parsed: RegistryDto = serde_json::from_value(doc).unwrap();
        assert_eq!(parsed.registry_id, "controlled-rule-intake");
    }
}
