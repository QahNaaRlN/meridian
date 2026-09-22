//! Converts a schema-validated, transport-parsed [`super::dto`] tree into
//! [`meridian_core::instruction_source`] domain types (levels 3-4 of
//! `standards/workspace/rust-migration-quality.md` §4).
//!
//! Every function here accumulates ALL of an entry's independent defects
//! before deciding success or failure, mirroring
//! `controlled_rule_intake::convert`'s own discipline: a source whose
//! `location` AND `recorded_state.currency` are both malformed reports
//! both, not just the first one reached.

use meridian_core::instruction_source::{
    Currency, Divergence, DivergenceStatus, InstructionSource, InstructionSourcePayload, Location,
    MeridianVisibility, ObservedState, OpaqueRef, ReadChannel, ReadChannelKind, RecordedState,
    RelativePath, SourceFormat, NORMATIVE_STATUS, RECORD_TYPE,
};
use meridian_core::types::{ContentDigest, Diagnostic, Revision, SemanticId};

use super::dto::{
    DivergenceDto, EntryDto, LocationDto, ObservedStateDto, PayloadDto, ReadChannelDto,
    RecordedStateDto,
};
use super::{fail, try_field};

fn build_location(at: &str, medium: &str, dto: &LocationDto) -> Result<Location, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    if dto.missing_behavior != "fail-closed" {
        problems.push(fail(format!(
            "{at} payload.location.missing_behavior is \"{}\"; the only declared behaviour for an unresolvable source is \"fail-closed\" — no hidden fallback resolution",
            dto.missing_behavior
        )));
    }

    match medium {
        "file" => {
            for (present, field) in [
                (dto.service_ref.is_some(), "service_ref"),
                (dto.resource_ref.is_some(), "resource_ref"),
            ] {
                if present {
                    problems.push(fail(format!(
                        "{at} payload.location.{field} is not allowed for medium \"file\""
                    )));
                }
            }
            let path = match dto.path.as_deref() {
                Some(p) => match RelativePath::new(p) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        problems.push(fail(format!("{at} payload.location.path \"{p}\": {e}")));
                        None
                    }
                },
                None => {
                    problems.push(fail(format!(
                        "{at} payload.location.path is required for medium \"file\""
                    )));
                    None
                }
            };
            let container_ref = match dto.container_ref.as_deref() {
                Some(r) => match OpaqueRef::new(r) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        problems.push(fail(format!("{at} payload.location.container_ref: {e}")));
                        None
                    }
                },
                None => {
                    problems.push(fail(format!(
                        "{at} payload.location.container_ref is required for medium \"file\""
                    )));
                    None
                }
            };
            match (path, container_ref) {
                (Some(path), Some(container_ref)) if problems.is_empty() => {
                    Ok(Location::file(path, container_ref))
                }
                _ => Err(problems),
            }
        }
        "external-service" => {
            for (present, field) in [
                (dto.path.is_some(), "path"),
                (dto.container_ref.is_some(), "container_ref"),
            ] {
                if present {
                    problems.push(fail(format!(
                        "{at} payload.location.{field} is not allowed for medium \"external-service\""
                    )));
                }
            }
            let service_ref = match dto.service_ref.as_deref() {
                Some(r) => match OpaqueRef::new(r) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        problems.push(fail(format!("{at} payload.location.service_ref: {e}")));
                        None
                    }
                },
                None => {
                    problems.push(fail(format!(
                        "{at} payload.location.service_ref is required for medium \"external-service\""
                    )));
                    None
                }
            };
            let resource_ref = match dto.resource_ref.as_deref() {
                Some(r) => match OpaqueRef::new(r) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        problems.push(fail(format!("{at} payload.location.resource_ref: {e}")));
                        None
                    }
                },
                None => {
                    problems.push(fail(format!(
                        "{at} payload.location.resource_ref is required for medium \"external-service\""
                    )));
                    None
                }
            };
            match (service_ref, resource_ref) {
                (Some(service_ref), Some(resource_ref)) if problems.is_empty() => {
                    Ok(Location::external_service(service_ref, resource_ref))
                }
                _ => Err(problems),
            }
        }
        other => Err(vec![fail(format!(
            "{at} payload.medium \"{other}\" is not one of the two closed media"
        ))]),
    }
}

fn build_digest(
    at: &str,
    field: &str,
    dto: &super::dto::DigestDto,
) -> Result<ContentDigest, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let digest = try_field(&mut problems, at, ContentDigest::from_hex(&dto.value));
    if dto.algorithm != "sha-256" {
        problems.push(fail(format!(
            "{at} {field}.digest.algorithm is not \"sha-256\""
        )));
    }
    match digest {
        Some(digest) if problems.is_empty() => Ok(digest),
        _ => Err(problems),
    }
}

fn build_recorded_state(
    at: &str,
    dto: &RecordedStateDto,
) -> Result<RecordedState, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let revision = try_field(&mut problems, at, Revision::new(&dto.revision));
    let digest = match build_digest(at, "payload.recorded_state", &dto.digest) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    let currency = match Currency::parse(&dto.currency) {
        Some(c) => Some(c),
        None => {
            problems.push(fail(format!(
                "{at} payload.recorded_state.currency \"{}\" is not one of the three closed values",
                dto.currency
            )));
            None
        }
    };
    match (revision, digest, currency) {
        (Some(revision), Some(digest), Some(currency)) if problems.is_empty() => Ok(
            RecordedState::new(revision, digest, dto.revision_verified, currency),
        ),
        _ => Err(problems),
    }
}

fn build_observed_state(
    at: &str,
    label: &str,
    dto: &ObservedStateDto,
) -> Result<ObservedState, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let revision = try_field(&mut problems, at, Revision::new(&dto.revision));
    let digest = match build_digest(at, &format!("payload.divergence.{label}"), &dto.digest) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    match (revision, digest) {
        (Some(revision), Some(digest)) if problems.is_empty() => {
            Ok(ObservedState::new(revision, digest, dto.verified))
        }
        _ => Err(problems),
    }
}

fn build_divergence(
    at: &str,
    id: &str,
    dto: &DivergenceDto,
) -> Result<Divergence, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let status = match DivergenceStatus::parse(&dto.status) {
        Some(s) => Some(s),
        None => {
            problems.push(fail(format!(
                "{at} payload.divergence.status \"{}\" is not one of the five closed statuses",
                dto.status
            )));
            None
        }
    };
    let previous_state = match &dto.previous_state {
        Some(v) => match build_observed_state(at, "previous_state", v) {
            Ok(s) => Some(Some(s)),
            Err(errs) => {
                problems.extend(errs);
                None
            }
        },
        None => Some(None),
    };
    let current_state = match &dto.current_state {
        Some(v) => match build_observed_state(at, "current_state", v) {
            Ok(s) => Some(Some(s)),
            Err(errs) => {
                problems.extend(errs);
                None
            }
        },
        None => Some(None),
    };
    match (status, previous_state, current_state) {
        (Some(status), Some(previous_state), Some(current_state)) if problems.is_empty() => {
            Divergence::try_new(id, status, previous_state, current_state)
        }
        _ => Err(problems),
    }
}

fn build_read_channel(
    at: &str,
    id: &str,
    dto: &ReadChannelDto,
) -> Result<ReadChannel, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let kind = match ReadChannelKind::parse(&dto.kind) {
        Some(k) => Some(k),
        None => {
            problems.push(fail(format!(
                "{at} payload.read_channel.kind \"{}\" is not one of the three closed kinds",
                dto.kind
            )));
            None
        }
    };
    let visibility = match MeridianVisibility::parse(&dto.meridian_visibility) {
        Some(v) => Some(v),
        None => {
            problems.push(fail(format!(
                "{at} payload.read_channel.meridian_visibility \"{}\" is not one of the three closed values",
                dto.meridian_visibility
            )));
            None
        }
    };
    match (kind, visibility) {
        (Some(kind), Some(visibility)) if problems.is_empty() => {
            ReadChannel::try_new(id, kind, visibility, dto.agent_auto_read)
        }
        _ => Err(problems),
    }
}

fn build_payload(
    at: &str,
    id: &str,
    dto: &PayloadDto,
) -> Result<InstructionSourcePayload, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    if dto.normative_status != NORMATIVE_STATUS {
        problems.push(fail(format!(
            "{at} payload.normative_status is \"{}\", not \"{NORMATIVE_STATUS}\"; registration grants the source's text no norm authority, accepts no rule and resolves no conflict",
            dto.normative_status
        )));
    }
    let location = match build_location(at, &dto.medium, &dto.location) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    let format = match SourceFormat::parse(&dto.format) {
        Some(f) => Some(f),
        None => {
            problems.push(fail(format!(
                "{at} payload.format \"{}\" is not one of the six closed formats",
                dto.format
            )));
            None
        }
    };
    let recorded_state = match build_recorded_state(at, &dto.recorded_state) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    let read_channel = match build_read_channel(at, id, &dto.read_channel) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };
    let divergence = match build_divergence(at, id, &dto.divergence) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };

    match (location, format, recorded_state, read_channel, divergence) {
        (
            Some(location),
            Some(format),
            Some(recorded_state),
            Some(read_channel),
            Some(divergence),
        ) if problems.is_empty() => Ok(InstructionSourcePayload::new(
            location,
            format,
            recorded_state,
            read_channel,
            divergence,
        )),
        _ => Err(problems),
    }
}

/// Builds one [`InstructionSource`] from its transport DTO, accumulating
/// every independent defect before deciding success. Only when every
/// sub-part parses does this call [`InstructionSource::try_new`], which
/// then separately validates the recorded-state/divergence temporal
/// coherence and may itself add diagnostics.
pub(crate) fn build_source(dto: &EntryDto) -> Result<InstructionSource, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let at = format!("instruction source \"{}\"", dto.id);

    if dto.record_type != RECORD_TYPE {
        problems.push(fail(format!(
            "{at} declares record_type \"{}\", not \"{RECORD_TYPE}\"; registering a source never turns it into a norm",
            dto.record_type
        )));
    }
    let id = try_field(&mut problems, &at, SemanticId::new(&dto.id));
    let payload = match build_payload(&at, &dto.id, &dto.payload) {
        Ok(v) => Some(v),
        Err(errs) => {
            problems.extend(errs);
            None
        }
    };

    let (Some(id), Some(payload)) = (id, payload) else {
        return Err(problems);
    };

    match InstructionSource::try_new(id, payload) {
        Ok(source) if problems.is_empty() => Ok(source),
        Ok(_) => Err(problems),
        Err(cross_field_problems) => {
            problems.extend(cross_field_problems);
            Err(problems)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::dto::EntryDto;
    use super::*;

    fn entry_dto() -> EntryDto {
        let doc = serde_json::json!({
            "id": "src-1",
            "record_type": "instruction-source",
            "payload": {
                "normative_status": "not-a-norm",
                "medium": "file",
                "location": {"path": "a.md", "container_ref": "container-1", "missing_behavior": "fail-closed"},
                "recorded_state": {"revision": "a".repeat(40), "digest": {"algorithm": "sha-256", "value": "b".repeat(64)}, "revision_verified": true, "currency": "current"},
                "format": "markdown-section",
                "read_channel": {"kind": "meridian-observed", "meridian_visibility": "full", "agent_auto_read": false},
                "divergence": {"status": "unknown"},
            },
        });
        serde_json::from_value(doc).unwrap()
    }

    #[test]
    fn a_well_formed_entry_dto_builds_cleanly() {
        assert!(build_source(&entry_dto()).is_ok());
    }

    /// Independent defects in `location` AND `recorded_state.currency` are
    /// BOTH reported, not just the first.
    #[test]
    fn multiple_independent_field_defects_are_all_accumulated() {
        let mut dto = entry_dto();
        dto.payload.location.path = None;
        dto.payload.recorded_state.currency = "invented".to_string();
        let errors = build_source(&dto).unwrap_err();
        assert!(
            errors.iter().any(|d| d.message().contains("location.path")),
            "{errors:?}"
        );
        assert!(
            errors.iter().any(|d| d.message().contains("invented")),
            "{errors:?}"
        );
    }

    #[test]
    fn an_external_service_medium_rejects_a_file_only_field() {
        let mut dto = entry_dto();
        dto.payload.medium = "external-service".to_string();
        dto.payload.location.service_ref = Some("service-1".to_string());
        dto.payload.location.resource_ref = Some("resource-1".to_string());
        let errors = build_source(&dto).unwrap_err();
        assert!(
            errors.iter().any(|d| d
                .message()
                .contains("path is not allowed for medium \"external-service\"")),
            "{errors:?}"
        );
    }
}
