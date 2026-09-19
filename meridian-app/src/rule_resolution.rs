//! In-memory application adapter for deterministic rule resolution.
//!
//! The pure algorithm lives in `meridian-core`. This module owns the
//! transport boundary: it validates already-loaded JSON values, converts
//! them into strict domain types, invokes the core, and serializes the
//! result. It performs no file, Git, database, network, environment, CLI or
//! process I/O.

use std::{collections::HashMap, fmt};

use meridian_core::{
    resolver::{
        resolve_rules, Activation, ApplicabilityRecord, ApplicabilityScope, ApplicabilitySource,
        ApplicabilityStatus, ChangeClass, DeclaredProfiles, Decomposition, Delivery, IntakePointer,
        IntakeRecordEntry, IntakeRegister, IntakeVerdict, IsoDate, NormRef, PreviousResolution,
        PriorState, ProtocolRoute, ProtocolRouteScope, RefactorFinding, RepositoryInventoryEntry,
        ResolverOutput, ResolverSources, RouteField, RouteKey, RouteProvenance, RouteSource,
        SupersedesPointer, TaskClassSelector, VerificationRoute, VerificationTarget, WorkItem,
        WorkItemKind, WorkKind,
    },
    types::ContentDigest,
};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::source_format::json_schema;

/// The stage at which the strict application boundary refused a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleResolutionError {
    InvalidInput(String),
    UnsupportedSchema(String),
    Core(String),
    InvalidOutput(String),
}

impl fmt::Display for RuleResolutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidInput(message)
            | Self::UnsupportedSchema(message)
            | Self::Core(message)
            | Self::InvalidOutput(message) => message,
        };
        f.write_str(message)
    }
}

impl std::error::Error for RuleResolutionError {}

fn invalid(message: impl Into<String>) -> RuleResolutionError {
    RuleResolutionError::InvalidInput(message.into())
}

/// Validate and resolve one already-loaded request.
///
/// The request is a closed object with `work_item`, `repository_inventory`,
/// `applicability`, and optional injected sources. The two schemas are also
/// injected explicitly; this function never opens a Kernel file.
pub fn resolve(
    request: &Value,
    applicability_schema: &Value,
    output_schema: &Value,
) -> Result<Value, RuleResolutionError> {
    let transport: ResolutionRequest = serde_json::from_value(request.clone())
        .map_err(|error| invalid(format!("invalid rule-resolution request: {error}")))?;

    let schema_errors = json_schema::validate(&transport.applicability, applicability_schema)
        .map_err(|error| {
            RuleResolutionError::UnsupportedSchema(format!(
                "unsupported applicability schema: {error}"
            ))
        })?;
    if let Some(error) = schema_errors.first() {
        return Err(invalid(format!(
            "applicability document does not satisfy its schema: {error}"
        )));
    }

    let work_item = convert_work_item(&transport.work_item)?;
    let sources = convert_sources(transport)?;
    let value =
        output_value(resolve_rules(&work_item, &sources).map_err(|error| {
            RuleResolutionError::Core(format!("rule resolution failed: {error}"))
        })?);

    let output_errors = json_schema::validate(&value, output_schema).map_err(|error| {
        RuleResolutionError::UnsupportedSchema(format!(
            "unsupported resolver-output schema: {error}"
        ))
    })?;
    if let Some(error) = output_errors.first() {
        return Err(RuleResolutionError::InvalidOutput(format!(
            "internal: resolver output does not satisfy its schema: {error}"
        )));
    }
    Ok(value)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolutionRequest {
    work_item: WorkItemDto,
    repository_inventory: Vec<RepositoryDto>,
    applicability: Value,
    #[serde(default)]
    intake_registers: Vec<IntakeRegisterDto>,
    #[serde(default)]
    protocol_routes: Vec<Value>,
    #[serde(default)]
    verification_routes: Vec<VerificationRouteDto>,
    #[serde(default)]
    norm_texts: HashMap<String, String>,
    #[serde(default)]
    prior_state: PriorStateDto,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkItemDto {
    repository_id: String,
    work_kind: String,
    #[serde(default)]
    change_class: Option<String>,
    candidate_paths: Vec<String>,
    changed_paths: Vec<String>,
    #[serde(default)]
    declared_profiles: Option<DeclaredProfilesDto>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeclaredProfilesDto {
    #[serde(default)]
    technology_profile: Option<String>,
    #[serde(default)]
    architecture_profile: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepositoryDto {
    id: String,
    #[serde(default)]
    profile: Option<String>,
    #[serde(default)]
    semantic_areas: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplicabilityEnvelopeDto {
    #[serde(rename = "$schema")]
    _schema: String,
    schema_version: u64,
    records: Vec<ApplicabilityRecordDto>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplicabilityRecordDto {
    norm: NormDto,
    digest: String,
    scope: String,
    #[serde(default)]
    repository: Option<String>,
    #[serde(default)]
    technology_profile: Option<String>,
    #[serde(default)]
    product_domain: Option<String>,
    activation: String,
    #[serde(default)]
    globs: Option<Vec<String>>,
    #[serde(default)]
    task_class: Option<TaskClassDto>,
    source: String,
    #[serde(default)]
    intake_record: Option<IntakePointerDto>,
    status: String,
    #[serde(default)]
    resume_condition: Option<String>,
    recorded_at: String,
    #[serde(default)]
    supersedes: Option<SupersedesDto>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct NormDto {
    repository: String,
    path: String,
    #[serde(default)]
    region: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskClassDto {
    work_kind: Vec<String>,
    #[serde(default)]
    change_class: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IntakePointerDto {
    register: String,
    recorded_at: String,
    verdict: String,
    #[serde(default)]
    revision: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SupersedesDto {
    register: String,
    path: String,
    #[serde(default)]
    region: Option<String>,
    recorded_at: String,
    verdict: String,
    #[serde(default)]
    revision: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IntakeRegisterDto {
    register: String,
    records: Vec<IntakeRecordDto>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IntakeRecordDto {
    artifact: String,
    #[serde(default)]
    region: Option<String>,
    recorded_at: String,
    verdict: String,
    delivery: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerificationRouteDto {
    applies_to: String,
    path: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PriorStateDto {
    #[serde(default)]
    previous_resolution: Option<PreviousResolutionDto>,
    #[serde(default)]
    decomposition: Option<DecompositionDto>,
    #[serde(default)]
    initiative_protocol: Option<String>,
    #[serde(default)]
    pre_decomposition_norms: Vec<String>,
    #[serde(default)]
    source_repository_ids: Option<Vec<String>>,
    #[serde(default)]
    refactor_findings: Vec<RefactorFindingDto>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PreviousResolutionDto {
    candidate_paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DecompositionDto {
    child_work_items: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RefactorFindingDto {
    #[serde(rename = "type")]
    finding_type: String,
}

fn convert_work_item(dto: &WorkItemDto) -> Result<WorkItem, RuleResolutionError> {
    let kind = match (dto.work_kind.as_str(), dto.change_class.as_deref()) {
        ("change", Some(value)) => WorkItemKind::Change(change_class(value)?),
        ("change", None) => return Err(invalid("work item: change_class is required for change")),
        ("assessment", None) => WorkItemKind::Assessment,
        ("operation", None) => WorkItemKind::Operation,
        ("initiative", None) => WorkItemKind::Initiative,
        (kind, Some(_)) if kind != "change" => {
            return Err(invalid(format!(
                "work item: change_class is forbidden for work_kind {kind:?}"
            )))
        }
        (value, None) => return Err(invalid(format!("unknown work_kind {value:?}"))),
        _ => return Err(invalid("invalid work item kind")),
    };
    let profiles = dto
        .declared_profiles
        .as_ref()
        .map(|value| DeclaredProfiles {
            technology_profile: value.technology_profile.clone(),
            architecture_profile: value.architecture_profile.clone(),
        });
    WorkItem::new(
        dto.repository_id.clone(),
        kind,
        dto.candidate_paths.clone(),
        dto.changed_paths.clone(),
        profiles,
    )
    .map_err(|error| invalid(error.to_string()))
}

fn convert_sources(dto: ResolutionRequest) -> Result<ResolverSources, RuleResolutionError> {
    let envelope: ApplicabilityEnvelopeDto = serde_json::from_value(dto.applicability)
        .map_err(|error| invalid(format!("invalid applicability document: {error}")))?;
    if envelope.schema_version != 1 {
        return Err(invalid("applicability schema_version must be 1"));
    }

    Ok(ResolverSources {
        repository_inventory: dto
            .repository_inventory
            .into_iter()
            .map(|value| RepositoryInventoryEntry {
                id: value.id,
                profile: value.profile,
                semantic_areas: value.semantic_areas,
            })
            .collect(),
        applicability_records: envelope
            .records
            .into_iter()
            .map(convert_applicability_record)
            .collect::<Result<_, _>>()?,
        intake_registers: dto
            .intake_registers
            .into_iter()
            .map(convert_intake_register)
            .collect::<Result<_, _>>()?,
        protocol_routes: dto
            .protocol_routes
            .into_iter()
            .map(convert_protocol_route)
            .collect::<Result<_, _>>()?,
        verification_routes: dto
            .verification_routes
            .into_iter()
            .map(|value| VerificationRoute {
                applies_to: if value.applies_to == "*" {
                    VerificationTarget::All
                } else {
                    VerificationTarget::Norm(value.applies_to)
                },
                path: value.path,
            })
            .collect(),
        norm_texts: dto.norm_texts,
        prior_state: PriorState {
            previous_resolution: dto.prior_state.previous_resolution.map(|value| {
                PreviousResolution {
                    candidate_paths: value.candidate_paths,
                }
            }),
            decomposition: dto.prior_state.decomposition.map(|value| Decomposition {
                child_work_items: value.child_work_items,
            }),
            initiative_protocol: dto.prior_state.initiative_protocol,
            pre_decomposition_norms: dto.prior_state.pre_decomposition_norms,
            source_repository_ids: dto.prior_state.source_repository_ids,
            refactor_findings: dto
                .prior_state
                .refactor_findings
                .into_iter()
                .map(|value| RefactorFinding {
                    finding_type: value.finding_type,
                })
                .collect(),
        },
    })
}

fn convert_applicability_record(
    dto: ApplicabilityRecordDto,
) -> Result<ApplicabilityRecord, RuleResolutionError> {
    let scope = match dto.scope.as_str() {
        "universal" => ApplicabilityScope::Universal,
        "profile" => ApplicabilityScope::Profile {
            technology_profile: dto
                .technology_profile
                .ok_or_else(|| invalid("profile scope requires technology_profile"))?,
        },
        "repository" => ApplicabilityScope::Repository {
            repository: dto
                .repository
                .ok_or_else(|| invalid("repository scope requires repository"))?,
        },
        "product-domain" => ApplicabilityScope::ProductDomain {
            product_domain: dto
                .product_domain
                .ok_or_else(|| invalid("product-domain scope requires product_domain"))?,
        },
        value => return Err(invalid(format!("unknown applicability scope {value:?}"))),
    };
    let activation = match dto.activation.as_str() {
        "always" => Activation::Always,
        "path-glob" => Activation::path_glob(
            dto.globs
                .ok_or_else(|| invalid("path-glob activation requires globs"))?,
        )
        .map_err(|error| invalid(error.to_string()))?,
        "task-class" => {
            let task = dto
                .task_class
                .ok_or_else(|| invalid("task-class activation requires task_class"))?;
            Activation::TaskClass {
                task_class: TaskClassSelector::new(
                    task.work_kind
                        .into_iter()
                        .map(|value| work_kind(&value))
                        .collect::<Result<_, _>>()?,
                    task.change_class
                        .map(|values| {
                            values
                                .into_iter()
                                .map(|value| change_class(&value))
                                .collect::<Result<Vec<_>, _>>()
                        })
                        .transpose()?,
                )
                .map_err(|error| invalid(error.to_string()))?,
            }
        }
        "explicit" => Activation::Explicit,
        "undetermined" => Activation::Undetermined,
        value => return Err(invalid(format!("unknown activation {value:?}"))),
    };
    let intake = dto.intake_record.map(convert_intake_pointer).transpose()?;
    let supersedes = dto.supersedes.map(convert_supersedes).transpose()?;
    let source = match dto.source.as_str() {
        "kernel" => ApplicabilitySource::Kernel,
        "repository" => ApplicabilitySource::Repository {
            intake_record: intake
                .ok_or_else(|| invalid("repository source requires intake_record"))?,
            supersedes,
        },
        "delivery-adapter" => ApplicabilitySource::DeliveryAdapter {
            intake_record: intake
                .ok_or_else(|| invalid("delivery-adapter source requires intake_record"))?,
            supersedes,
        },
        value => return Err(invalid(format!("unknown applicability source {value:?}"))),
    };
    let status = match dto.status.as_str() {
        "resolved" => ApplicabilityStatus::Resolved,
        "unresolved" => ApplicabilityStatus::Unresolved {
            resume_condition: dto
                .resume_condition
                .ok_or_else(|| invalid("unresolved status requires resume_condition"))?,
        },
        value => return Err(invalid(format!("unknown applicability status {value:?}"))),
    };
    Ok(ApplicabilityRecord::new(
        NormRef::new(dto.norm.repository, dto.norm.path, dto.norm.region)
            .map_err(|error| invalid(error.to_string()))?,
        ContentDigest::from_hex(dto.digest).map_err(|error| invalid(error.to_string()))?,
        scope,
        activation,
        source,
        status,
        date(&dto.recorded_at)?,
    ))
}

fn convert_intake_pointer(dto: IntakePointerDto) -> Result<IntakePointer, RuleResolutionError> {
    IntakePointer::new(
        dto.register,
        date(&dto.recorded_at)?,
        intake_verdict(&dto.verdict)?,
        dto.revision,
    )
    .map_err(|error| invalid(error.to_string()))
}

fn convert_supersedes(dto: SupersedesDto) -> Result<SupersedesPointer, RuleResolutionError> {
    SupersedesPointer::new(
        dto.register,
        dto.path,
        dto.region,
        date(&dto.recorded_at)?,
        intake_verdict(&dto.verdict)?,
        dto.revision,
    )
    .map_err(|error| invalid(error.to_string()))
}

fn convert_intake_register(dto: IntakeRegisterDto) -> Result<IntakeRegister, RuleResolutionError> {
    Ok(IntakeRegister {
        register: dto.register,
        records: dto
            .records
            .into_iter()
            .map(|value| {
                Ok(IntakeRecordEntry {
                    artifact: value.artifact,
                    region: value.region,
                    recorded_at: date(&value.recorded_at)?,
                    verdict: intake_verdict(&value.verdict)?,
                    delivery: delivery(&value.delivery)?,
                })
            })
            .collect::<Result<_, RuleResolutionError>>()?,
    })
}

fn convert_protocol_route(value: Value) -> Result<ProtocolRoute, RuleResolutionError> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("protocol route must be an object"))?;
    for key in object.keys() {
        route_field(key)?;
    }
    // `mandatory` is always named in `order`, even when the source document
    // omitted it: `ProtocolRoute::new` requires `field_order` to name EXACTLY
    // `{Protocol, RoutedFrom, Source, Scope, ..., Mandatory}` (`mandatory` is
    // unconditional, unlike `repository`/`product_domain`/`digest`/`revision`,
    // which are conditional on this route's own scope/provenance). Absence in
    // the transport therefore normalizes to `false`, never to a missing field.
    let order = [
        "protocol",
        "routed_from",
        "source",
        "scope",
        "repository",
        "product_domain",
        "digest",
        "revision",
        "mandatory",
    ]
    .into_iter()
    .filter(|key| *key == "mandatory" || object.contains_key(*key))
    .map(route_field)
    .collect::<Result<Vec<_>, _>>()?;
    let string = |name: &str| {
        object
            .get(name)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| invalid(format!("protocol route requires string {name}")))
    };
    let routed_from = route_key(&string("routed_from")?)?;
    let source = match string("source")?.as_str() {
        "kernel" => RouteSource::Kernel,
        "instance" => RouteSource::Instance,
        "repository" => RouteSource::Repository,
        value => return Err(invalid(format!("unknown protocol route source {value:?}"))),
    };
    let scope = match string("scope")?.as_str() {
        "universal" => ProtocolRouteScope::Universal,
        "repository" => ProtocolRouteScope::repository(string("repository")?)
            .map_err(|error| invalid(error.to_string()))?,
        "product-domain" => ProtocolRouteScope::product_domain(string("product_domain")?)
            .map_err(|error| invalid(error.to_string()))?,
        value => return Err(invalid(format!("unknown protocol route scope {value:?}"))),
    };
    let provenance = match (object.get("digest"), object.get("revision")) {
        (Some(Value::String(value)), None) => RouteProvenance::Digest(
            ContentDigest::from_hex(value.clone()).map_err(|error| invalid(error.to_string()))?,
        ),
        (None, Some(Value::String(value))) => RouteProvenance::Revision(value.clone()),
        _ => {
            return Err(invalid(
                "protocol route requires exactly one of digest or revision",
            ))
        }
    };
    // Absent `mandatory` means `false`, matching the Node.js reference
    // (`rule-resolver.mjs`'s injected route shape treats a missing field as
    // falsy). A present value is required to be strictly boolean: `null`, a
    // string, a number or any other JSON type is rejected rather than
    // coerced.
    let mandatory = match object.get("mandatory") {
        None => false,
        Some(Value::Bool(value)) => *value,
        Some(_) => return Err(invalid("protocol route mandatory must be a boolean")),
    };
    ProtocolRoute::new(
        string("protocol")?,
        routed_from,
        source,
        scope,
        provenance,
        mandatory,
        order,
    )
    .map_err(|error| invalid(error.to_string()))
}

fn route_field(value: &str) -> Result<RouteField, RuleResolutionError> {
    match value {
        "protocol" => Ok(RouteField::Protocol),
        "routed_from" => Ok(RouteField::RoutedFrom),
        "source" => Ok(RouteField::Source),
        "scope" => Ok(RouteField::Scope),
        "repository" => Ok(RouteField::Repository),
        "product_domain" => Ok(RouteField::ProductDomain),
        "digest" => Ok(RouteField::Digest),
        "revision" => Ok(RouteField::Revision),
        "mandatory" => Ok(RouteField::Mandatory),
        value => Err(invalid(format!("unknown protocol route field {value:?}"))),
    }
}

fn work_kind(value: &str) -> Result<WorkKind, RuleResolutionError> {
    match value {
        "change" => Ok(WorkKind::Change),
        "assessment" => Ok(WorkKind::Assessment),
        "operation" => Ok(WorkKind::Operation),
        "initiative" => Ok(WorkKind::Initiative),
        value => Err(invalid(format!("unknown work_kind {value:?}"))),
    }
}

fn change_class(value: &str) -> Result<ChangeClass, RuleResolutionError> {
    match value {
        "BUGFIX" => Ok(ChangeClass::Bugfix),
        "FEATURE" => Ok(ChangeClass::Feature),
        "BEHAVIOR_CHANGE" => Ok(ChangeClass::BehaviorChange),
        "REFACTOR" => Ok(ChangeClass::Refactor),
        value => Err(invalid(format!("unknown change_class {value:?}"))),
    }
}

fn route_key(value: &str) -> Result<RouteKey, RuleResolutionError> {
    change_class(value)
        .map(RouteKey::ChangeClass)
        .or_else(|_| work_kind(value).map(RouteKey::WorkKind))
}

fn intake_verdict(value: &str) -> Result<IntakeVerdict, RuleResolutionError> {
    match value {
        "adopt-core" => Ok(IntakeVerdict::AdoptCore),
        "adopt-edition" => Ok(IntakeVerdict::AdoptEdition),
        "keep-local" => Ok(IntakeVerdict::KeepLocal),
        "merge-into" => Ok(IntakeVerdict::MergeInto),
        "rename" => Ok(IntakeVerdict::Rename),
        "retire" => Ok(IntakeVerdict::Retire),
        "deferred" => Ok(IntakeVerdict::Deferred),
        value => Err(invalid(format!("unknown intake verdict {value:?}"))),
    }
}

fn delivery(value: &str) -> Result<Delivery, RuleResolutionError> {
    match value {
        "kernel-doc" => Ok(Delivery::KernelDoc),
        "skill-package" => Ok(Delivery::SkillPackage),
        "cursor-rule" => Ok(Delivery::CursorRule),
        "agents-md-section" => Ok(Delivery::AgentsMdSection),
        value => Err(invalid(format!("unknown delivery {value:?}"))),
    }
}

fn date(value: &str) -> Result<IsoDate, RuleResolutionError> {
    IsoDate::new(value).map_err(|error| invalid(error.to_string()))
}

fn output_value(output: ResolverOutput) -> Value {
    let applicable_norms: Vec<Value> = output
        .applicable_norms
        .into_iter()
        .map(|value| {
            json!({
                "norm": value.norm,
                "activation_reason": value.activation_reason.as_str(),
                "source": value.source.as_str(),
                "digest": value.digest.value(),
                "delivery": value.delivery.as_str(),
            })
        })
        .collect();
    let applicable_protocols: Vec<Value> = output
        .applicable_protocols
        .into_iter()
        .map(|value| {
            let mut object = Map::new();
            object.insert("protocol".into(), Value::String(value.protocol));
            object.insert(
                "routed_from".into(),
                Value::String(value.routed_from.as_str().into()),
            );
            object.insert("source".into(), Value::String(value.source.as_str().into()));
            object.insert("scope".into(), Value::String(value.scope.as_str().into()));
            match value.provenance {
                RouteProvenance::Digest(digest) => {
                    object.insert("digest".into(), Value::String(digest.value().into()));
                }
                RouteProvenance::Revision(revision) => {
                    object.insert("revision".into(), Value::String(revision));
                }
            }
            Value::Object(object)
        })
        .collect();
    json!({
        "applicable_norms": applicable_norms,
        "applicable_protocols": applicable_protocols,
        "required_verification": output.required_verification,
        "conflicts": output.conflicts.into_iter().map(|value| json!({"norms": value.norms, "reason": value.reason})).collect::<Vec<_>>(),
        "unresolved_applicability": output.unresolved_applicability.into_iter().map(|value| json!({"subject": value.subject, "reason": value.reason})).collect::<Vec<_>>(),
        "unresolved_items": output.unresolved_items.into_iter().map(|value| json!({"item": value.item, "reason": value.reason})).collect::<Vec<_>>(),
        "requires_reresolution": output.requires_reresolution,
        "decomposition_required": output.decomposition_required,
        "applicable_initiative_protocol": output.applicable_initiative_protocol,
        "pre_decomposition_norms": output.pre_decomposition_norms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const APPLICABILITY_SCHEMA: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../registries/rule-resolution/applicability.schema.json"
    ));
    const OUTPUT_SCHEMA: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../registries/rule-resolution/resolver-output.schema.json"
    ));

    fn schemas() -> (Value, Value) {
        (
            serde_json::from_str(APPLICABILITY_SCHEMA).unwrap(),
            serde_json::from_str(OUTPUT_SCHEMA).unwrap(),
        )
    }

    fn base_request() -> Value {
        json!({
            "work_item": {
                "repository_id": "sample-repo",
                "work_kind": "operation",
                "candidate_paths": [],
                "changed_paths": []
            },
            "repository_inventory": [{"id": "sample-repo"}],
            "applicability": {
                "$schema": "../../registries/rule-resolution/applicability.schema.json",
                "schema_version": 1,
                "records": []
            }
        })
    }

    #[test]
    fn resolves_an_empty_but_valid_source_set() {
        let (applicability, output) = schemas();
        let value = resolve(&base_request(), &applicability, &output).unwrap();
        assert_eq!(value["applicable_norms"], json!([]));
        assert_eq!(value["requires_reresolution"], false);
    }

    #[test]
    fn rejects_unknown_transport_fields() {
        let (applicability, output) = schemas();
        let mut request = base_request();
        request["reviewer"] = json!("must-not-affect-resolution");
        assert!(resolve(&request, &applicability, &output)
            .unwrap_err()
            .to_string()
            .contains("unknown field"));
    }

    #[test]
    fn malformed_applicability_fails_before_core_resolution() {
        let (applicability, output) = schemas();
        let mut request = base_request();
        request["applicability"]
            .as_object_mut()
            .unwrap()
            .remove("records");
        assert!(resolve(&request, &applicability, &output)
            .unwrap_err()
            .to_string()
            .contains("does not satisfy its schema"));
    }

    #[test]
    fn composes_transport_sources_and_returns_core_output() {
        let (applicability, output) = schemas();
        let text = "always apply";
        let digest = ContentDigest::of_str(text).value().to_owned();
        let mut request = base_request();
        request["work_item"] = json!({
            "repository_id": "sample-repo",
            "work_kind": "change",
            "change_class": "FEATURE",
            "candidate_paths": ["src/lib.rs"],
            "changed_paths": ["src/lib.rs", "src/new.rs"]
        });
        request["applicability"]["records"] = json!([{
            "norm": {"repository": "kernel", "path": "standards/sample.md"},
            "digest": digest,
            "scope": "universal",
            "activation": "always",
            "source": "kernel",
            "status": "resolved",
            "recorded_at": "2026-09-19"
        }]);
        request["protocol_routes"] = json!([{
            "protocol": "feature-protocol",
            "routed_from": "FEATURE",
            "source": "kernel",
            "scope": "universal",
            "revision": "abcdef0",
            "mandatory": true
        }]);
        request["verification_routes"] = json!([{
            "applies_to": "*",
            "path": "verification/regression-testing/README.md"
        }]);
        request["norm_texts"] = json!({"kernel::standards/sample.md": text});
        request["prior_state"] = json!({
            "previous_resolution": {"candidate_paths": ["src/lib.rs"]}
        });

        let value = resolve(&request, &applicability, &output).unwrap();
        assert_eq!(value["applicable_norms"][0]["norm"], "standards/sample.md");
        assert_eq!(
            value["applicable_protocols"][0]["protocol"],
            "feature-protocol"
        );
        assert_eq!(
            value["required_verification"],
            json!(["verification/regression-testing/README.md"])
        );
        assert_eq!(value["requires_reresolution"], true);
    }

    #[test]
    fn initiative_blockers_stay_separate_from_norm_applicability() {
        let (applicability, output) = schemas();
        let mut request = base_request();
        request["work_item"] = json!({
            "repository_id": "sample-repo",
            "work_kind": "initiative",
            "candidate_paths": [],
            "changed_paths": []
        });
        request["prior_state"] = json!({
            "initiative_protocol": "decompose-initiative",
            "pre_decomposition_norms": ["product-wide"]
        });

        let value = resolve(&request, &applicability, &output).unwrap();
        assert_eq!(value["decomposition_required"], true);
        assert_eq!(value["unresolved_items"].as_array().unwrap().len(), 1);
        assert_eq!(value["unresolved_applicability"], json!([]));
    }

    fn change_work_item() -> Value {
        json!({
            "repository_id": "sample-repo",
            "work_kind": "change",
            "change_class": "FEATURE",
            "candidate_paths": [],
            "changed_paths": []
        })
    }

    #[test]
    fn protocol_route_without_mandatory_defaults_to_false_and_is_accepted() {
        let (applicability, output) = schemas();

        let mut without_mandatory = base_request();
        without_mandatory["work_item"] = change_work_item();
        without_mandatory["protocol_routes"] = json!([{
            "protocol": "feature-protocol",
            "routed_from": "FEATURE",
            "source": "kernel",
            "scope": "universal",
            "revision": "abcdef0"
        }]);

        let mut with_explicit_false = without_mandatory.clone();
        with_explicit_false["protocol_routes"][0]["mandatory"] = json!(false);

        let value_without = resolve(&without_mandatory, &applicability, &output).unwrap();
        let value_explicit = resolve(&with_explicit_false, &applicability, &output).unwrap();

        assert_eq!(
            value_without, value_explicit,
            "an omitted mandatory must resolve identically to an explicit mandatory: false"
        );
        assert_eq!(
            value_without["applicable_protocols"][0]["protocol"],
            "feature-protocol"
        );
    }

    #[test]
    fn protocol_route_mandatory_of_the_wrong_type_is_rejected() {
        let (applicability, output) = schemas();
        for bad_mandatory in [json!(null), json!("true"), json!(1), json!([]), json!({})] {
            let mut request = base_request();
            request["work_item"] = change_work_item();
            request["protocol_routes"] = json!([{
                "protocol": "feature-protocol",
                "routed_from": "FEATURE",
                "source": "kernel",
                "scope": "universal",
                "revision": "abcdef0",
                "mandatory": bad_mandatory
            }]);

            let error = resolve(&request, &applicability, &output).unwrap_err();
            assert!(
                error.to_string().contains("mandatory"),
                "expected a mandatory-typed rejection, got: {error}"
            );
        }
    }

    #[test]
    fn protocol_route_field_order_does_not_affect_resolution() {
        let (applicability, output) = schemas();

        let declared_order = r#"{
            "work_item": {"repository_id":"sample-repo","work_kind":"change","change_class":"FEATURE","candidate_paths":[],"changed_paths":[]},
            "repository_inventory": [{"id":"sample-repo"}],
            "applicability": {"$schema":"../../registries/rule-resolution/applicability.schema.json","schema_version":1,"records":[]},
            "protocol_routes": [{"protocol":"feature-protocol","routed_from":"FEATURE","source":"kernel","scope":"universal","revision":"abcdef0","mandatory":true}]
        }"#;
        let permuted_order = r#"{
            "protocol_routes": [{"mandatory":true,"scope":"universal","revision":"abcdef0","routed_from":"FEATURE","source":"kernel","protocol":"feature-protocol"}],
            "applicability": {"records":[],"schema_version":1,"$schema":"../../registries/rule-resolution/applicability.schema.json"},
            "repository_inventory": [{"id":"sample-repo"}],
            "work_item": {"changed_paths":[],"candidate_paths":[],"change_class":"FEATURE","work_kind":"change","repository_id":"sample-repo"}
        }"#;

        let request_declared: Value = serde_json::from_str(declared_order).unwrap();
        let request_permuted: Value = serde_json::from_str(permuted_order).unwrap();

        let value_declared = resolve(&request_declared, &applicability, &output).unwrap();
        let value_permuted = resolve(&request_permuted, &applicability, &output).unwrap();

        assert_eq!(
            value_declared, value_permuted,
            "permuting a logically identical route's transport key order must not change the resolved result"
        );
    }

    #[test]
    fn tied_protocol_routes_same_source_rank_pick_a_deterministic_winner_by_provenance() {
        let (applicability, output) = schemas();
        let digest = ContentDigest::of_str("route-with-digest-provenance")
            .value()
            .to_owned();

        let route_with_digest = json!({
            "protocol": "feature-protocol",
            "routed_from": "FEATURE",
            "source": "kernel",
            "scope": "universal",
            "digest": digest,
            "mandatory": false
        });
        let route_with_revision = json!({
            "protocol": "feature-protocol",
            "routed_from": "FEATURE",
            "source": "kernel",
            "scope": "universal",
            "revision": "rev-b",
            "mandatory": false
        });

        let mut digest_first = base_request();
        digest_first["work_item"] = change_work_item();
        digest_first["protocol_routes"] =
            json!([route_with_digest.clone(), route_with_revision.clone()]);

        let mut revision_first = base_request();
        revision_first["work_item"] = change_work_item();
        revision_first["protocol_routes"] = json!([route_with_revision, route_with_digest]);

        let value_digest_first = resolve(&digest_first, &applicability, &output).unwrap();
        let value_revision_first = resolve(&revision_first, &applicability, &output).unwrap();

        assert_eq!(
            value_digest_first, value_revision_first,
            "the winner of a real tie among same-protocol, same-source-rank routes must not depend on source-array order"
        );

        let protocols = value_digest_first["applicable_protocols"]
            .as_array()
            .unwrap();
        assert_eq!(protocols.len(), 1, "the two routes tie on one protocol key");
        assert!(
            protocols[0].get("digest").is_some(),
            "expected the digest-provenance route to win the deterministic tie-break, got: {:?}",
            protocols[0]
        );
    }
}
