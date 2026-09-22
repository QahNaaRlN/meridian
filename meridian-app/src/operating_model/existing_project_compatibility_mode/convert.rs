//! Converts this module's OWN schema-validated, transport-parsed
//! [`super::dto`] fields into [`super::domain`] types (levels 3-4 of
//! `standards/workspace/rust-migration-quality.md` §4). `discovered_sources`
//! and `rule_candidates[].candidate` are converted separately, in
//! [`super`], through the REAL `instruction_source_registry`/
//! `controlled_rule_intake` pipelines — not here.

use meridian_core::instruction_source::{Location, OpaqueRef, RelativePath};
use meridian_core::resolver::IsoDate;
use meridian_core::types::{
    Authority, AuthorityKind, Diagnostic, DiagnosticLevel, NonEmptyString, SemanticId, WorkspaceId,
};

use super::domain::{
    DiscoveryPlanSlot, Finding, FindingKind, ManagedModeDecision, MissingSource, PreviousKnowledge,
    RepositoryRef, UnreadableSource,
};
use super::dto::{
    FindingDto, ManagedModeDecisionDto, MissingSourceDto, PlanSlotDto, RepositoryRefDto,
    UnreadableSourceDto,
};

fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message).expect("message is non-empty")
}

fn try_field<T, E: core::fmt::Display>(
    problems: &mut Vec<Diagnostic>,
    at: &str,
    result: Result<T, E>,
) -> Option<T> {
    match result {
        Ok(v) => Some(v),
        Err(e) => {
            problems.push(fail(format!("{at} {e}")));
            None
        }
    }
}

/// One `discovery_plan` slot's location — mirrors
/// `instruction_source_registry::convert::build_location`'s medium/location
/// coherence rule (the two contracts share the same shape by design, see
/// `existing-project-compatibility-mode.schema.json`'s own `plan_slot`
/// description), but is not the same function: a plan slot has no
/// `missing_behavior` field and no payload wrapper to unwrap.
pub(crate) fn build_plan_slot(
    at: &str,
    dto: &PlanSlotDto,
) -> Result<DiscoveryPlanSlot, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let id = try_field(&mut problems, at, SemanticId::new(&dto.id));

    let location = match dto.medium.as_str() {
        "file" => {
            for (present, field) in [
                (dto.service_ref.is_some(), "service_ref"),
                (dto.resource_ref.is_some(), "resource_ref"),
            ] {
                if present {
                    problems.push(fail(format!("{at} discovery_plan slot \"{}\": {field} is not allowed for medium \"file\"", dto.id)));
                }
            }
            let path = match dto.path.as_deref() {
                Some(p) => match RelativePath::new(p) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        problems.push(fail(format!(
                            "{at} discovery_plan slot \"{}\": path {e}",
                            dto.id
                        )));
                        None
                    }
                },
                None => {
                    problems.push(fail(format!(
                        "{at} discovery_plan slot \"{}\": path is required for medium \"file\"",
                        dto.id
                    )));
                    None
                }
            };
            let container_ref = match dto.container_ref.as_deref() {
                Some(r) => match OpaqueRef::new(r) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        problems.push(fail(format!(
                            "{at} discovery_plan slot \"{}\": container_ref {e}",
                            dto.id
                        )));
                        None
                    }
                },
                None => {
                    problems.push(fail(format!("{at} discovery_plan slot \"{}\": container_ref is required for medium \"file\"", dto.id)));
                    None
                }
            };
            match (path, container_ref) {
                (Some(path), Some(container_ref)) => Some(Location::file(path, container_ref)),
                _ => None,
            }
        }
        "external-service" => {
            for (present, field) in [
                (dto.path.is_some(), "path"),
                (dto.container_ref.is_some(), "container_ref"),
            ] {
                if present {
                    problems.push(fail(format!("{at} discovery_plan slot \"{}\": {field} is not allowed for medium \"external-service\"", dto.id)));
                }
            }
            let service_ref = match dto.service_ref.as_deref() {
                Some(r) => match OpaqueRef::new(r) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        problems.push(fail(format!(
                            "{at} discovery_plan slot \"{}\": service_ref {e}",
                            dto.id
                        )));
                        None
                    }
                },
                None => {
                    problems.push(fail(format!("{at} discovery_plan slot \"{}\": service_ref is required for medium \"external-service\"", dto.id)));
                    None
                }
            };
            let resource_ref = match dto.resource_ref.as_deref() {
                Some(r) => match OpaqueRef::new(r) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        problems.push(fail(format!(
                            "{at} discovery_plan slot \"{}\": resource_ref {e}",
                            dto.id
                        )));
                        None
                    }
                },
                None => {
                    problems.push(fail(format!("{at} discovery_plan slot \"{}\": resource_ref is required for medium \"external-service\"", dto.id)));
                    None
                }
            };
            match (service_ref, resource_ref) {
                (Some(service_ref), Some(resource_ref)) => {
                    Some(Location::external_service(service_ref, resource_ref))
                }
                _ => None,
            }
        }
        other => {
            problems.push(fail(format!(
                "{at} discovery_plan slot \"{}\": medium \"{other}\" is not one of the two closed media", dto.id
            )));
            None
        }
    };

    match (id, location) {
        (Some(id), Some(location)) if problems.is_empty() => {
            Ok(DiscoveryPlanSlot::new(id, location))
        }
        _ => Err(problems),
    }
}

fn build_previous_knowledge(
    at: &str,
    previously_known: bool,
    id: &Option<String>,
) -> Result<PreviousKnowledge, Vec<Diagnostic>> {
    match (previously_known, id) {
        (true, Some(id_text)) => {
            let mut problems = Vec::new();
            match SemanticId::new(id_text) {
                Ok(id) => Ok(PreviousKnowledge::PreviouslyKnown { id }),
                Err(e) => {
                    problems.push(fail(format!("{at} id: {e}")));
                    Err(problems)
                }
            }
        }
        (true, None) => Err(vec![fail(format!(
            "{at} previously_known is true but id is absent; a previously known entry must name the record it was previously known as"
        ))]),
        (false, None) => Ok(PreviousKnowledge::New),
        (false, Some(_)) => Err(vec![fail(format!(
            "{at} previously_known is false but id is present; an entry never previously known has nothing to name"
        ))]),
    }
}

pub(crate) fn build_missing_source(
    at: &str,
    dto: &MissingSourceDto,
) -> Result<MissingSource, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let plan_id = try_field(&mut problems, at, SemanticId::new(&dto.plan_id));
    let knowledge = match build_previous_knowledge(at, dto.previously_known, &dto.id) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    match (plan_id, knowledge) {
        (Some(plan_id), Some(knowledge)) if problems.is_empty() => {
            Ok(MissingSource::new(plan_id, knowledge))
        }
        _ => Err(problems),
    }
}

pub(crate) fn build_unreadable_source(
    at: &str,
    dto: &UnreadableSourceDto,
) -> Result<UnreadableSource, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let plan_id = try_field(&mut problems, at, SemanticId::new(&dto.plan_id));
    let reason = try_field(&mut problems, at, NonEmptyString::new(&dto.reason));
    let knowledge = match build_previous_knowledge(at, dto.previously_known, &dto.id) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    match (plan_id, reason, knowledge) {
        (Some(plan_id), Some(reason), Some(knowledge)) if problems.is_empty() => {
            Ok(UnreadableSource::new(plan_id, reason, knowledge))
        }
        _ => Err(problems),
    }
}

pub(crate) fn build_finding(at: &str, dto: &FindingDto) -> Result<Finding, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let kind = match FindingKind::parse(&dto.kind) {
        Some(k) => Some(k),
        None => {
            problems.push(fail(format!(
                "{at} finding \"{}\": kind \"{}\" is not one of the seven closed kinds",
                dto.id, dto.kind
            )));
            None
        }
    };
    let plan_id = match &dto.plan_id {
        Some(p) => match SemanticId::new(p) {
            Ok(v) => Some(Some(v)),
            Err(e) => {
                problems.push(fail(format!("{at} finding \"{}\": plan_id {e}", dto.id)));
                None
            }
        },
        None => Some(None),
    };
    match (kind, plan_id) {
        (Some(kind), Some(plan_id)) if problems.is_empty() => {
            Ok(Finding::new(kind, plan_id, dto.blocking))
        }
        _ => Err(problems),
    }
}

pub(crate) fn build_repository_ref(
    at: &str,
    dto: &RepositoryRefDto,
) -> Result<RepositoryRef, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let id = try_field(&mut problems, at, SemanticId::new(&dto.id));
    let workspace_id = try_field(&mut problems, at, WorkspaceId::new(&dto.workspace_id));
    match (id, workspace_id) {
        (Some(id), Some(workspace_id)) if problems.is_empty() => {
            Ok(RepositoryRef::new(id, workspace_id))
        }
        _ => Err(problems),
    }
}

fn parse_managed_authority_kind(value: &str) -> Option<AuthorityKind> {
    match value {
        "project-owner" => Some(AuthorityKind::ProjectOwner),
        "repository-maintainer" => Some(AuthorityKind::RepositoryMaintainer),
        _ => None,
    }
}

pub(crate) fn build_managed_mode_decision(
    at: &str,
    dto: &ManagedModeDecisionDto,
) -> Result<ManagedModeDecision, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let decided_at = try_field(&mut problems, at, IsoDate::new(&dto.decided_at));
    let kind = match parse_managed_authority_kind(&dto.authority.kind) {
        Some(k) => Some(k),
        None => {
            problems.push(fail(format!(
                "{at} managed_mode_decision.authority.kind \"{}\" is not one of the two closed kinds",
                dto.authority.kind
            )));
            None
        }
    };
    let authority = match kind {
        Some(kind) => match Authority::new(kind, dto.authority.authority_ref.clone(), None) {
            Ok(a) => Some(a),
            Err(e) => {
                problems.push(fail(format!("{at} managed_mode_decision.authority: {e}")));
                None
            }
        },
        None => None,
    };
    match (decided_at, authority) {
        (Some(decided_at), Some(authority)) if problems.is_empty() => {
            Ok(ManagedModeDecision::new(decided_at, authority))
        }
        _ => Err(problems),
    }
}

#[cfg(test)]
mod tests {
    use super::super::dto::{ManagedModeAuthorityDto, PlanSlotDto};
    use super::*;

    #[test]
    fn a_well_formed_plan_slot_builds_cleanly() {
        let dto = PlanSlotDto {
            id: "slot-1".to_string(),
            medium: "file".to_string(),
            path: Some("a.md".to_string()),
            container_ref: Some("container-1".to_string()),
            service_ref: None,
            resource_ref: None,
        };
        assert!(build_plan_slot("at", &dto).is_ok());
    }

    #[test]
    fn previously_known_true_requires_id() {
        let dto = MissingSourceDto {
            plan_id: "slot-1".to_string(),
            previously_known: true,
            id: None,
        };
        let err = build_missing_source("at", &dto).unwrap_err();
        assert!(err.iter().any(|d| d.message().contains("id is absent")));
    }

    #[test]
    fn previously_known_false_forbids_id() {
        let dto = MissingSourceDto {
            plan_id: "slot-1".to_string(),
            previously_known: false,
            id: Some("previous-id".to_string()),
        };
        let err = build_missing_source("at", &dto).unwrap_err();
        assert!(err.iter().any(|d| d.message().contains("id is present")));
    }

    #[test]
    fn managed_mode_decision_builds_cleanly() {
        let dto = ManagedModeDecisionDto {
            decision_ref: "x".to_string(),
            decided_at: "2026-09-21".to_string(),
            reason: "y".to_string(),
            authority: ManagedModeAuthorityDto {
                kind: "project-owner".to_string(),
                authority_ref: "owner:1".to_string(),
            },
        };
        assert!(build_managed_mode_decision("at", &dto).is_ok());
    }
}
