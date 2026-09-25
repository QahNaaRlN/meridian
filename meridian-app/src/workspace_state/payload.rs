//! M-05b — the one payload validator of product records.
//!
//! [`PayloadRegistry::check`] runs the whole transport/schema/domain pass of
//! one record's payload: the closed content container, the closed record
//! type (`meridian_core::workspace_state::record_types`), the media type and
//! digest of the container, the Kernel registry item schema where the type
//! has one, and the closed DTO of every type this crate reads. Both import
//! kinds call it before anything is written
//! (`migration::bundle`/`migration::operations`), and `validate
//! --workspace-db` calls it on every stored current record; there is no
//! second, simplified copy for either.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use meridian_core::types::{ContentDigest, Scope, SemanticId, WorkspaceRelativePath};
use meridian_core::workspace_state::dependencies::ExternalDependency;
use meridian_core::workspace_state::intake::IntakeRecord;
use meridian_core::workspace_state::inventory::RepositoryIdentity;
use meridian_core::workspace_state::observation::GateRunObservation;
use meridian_core::workspace_state::product::ProductPurity;
use meridian_core::workspace_state::record_types::{
    PayloadContract, ProductRecordType, RegistryItem, TypedWorkspaceFile,
};

use super::dto::{
    ContainerDto, ExternalDependenciesDto, IntakeRecordDto, ObservationDto, ProductDto,
    RepositoryIdentityDto,
};
use crate::source_format::{json_schema, parse_yaml};
use crate::workspace::WorkspaceReader;

/// Where each registry item's schema lives in the Kernel.
const ITEM_SCHEMAS: [(RegistryItem, &str, &str); 12] = [
    (
        RegistryItem::InventoryRepository,
        "registries/inventory/repositories.schema.json",
        "/properties/repositories/items",
    ),
    (
        RegistryItem::InventoryReference,
        "registries/inventory/repository-references.schema.json",
        "/properties/references/items",
    ),
    (
        RegistryItem::InventoryRelationship,
        "registries/inventory/relationships.schema.json",
        "/properties/relationships/items",
    ),
    (
        RegistryItem::IntakeRecord,
        "registries/instruction-intake/intake.schema.json",
        "/definitions/record",
    ),
    (
        RegistryItem::RepositoryCommand,
        "registries/commands/repositories.schema.json",
        "/properties/repositories/items/properties/commands/items",
    ),
    (
        RegistryItem::AccessApplication,
        "registries/environments/access.schema.json",
        "/properties/applications/items",
    ),
    (
        RegistryItem::AccessReference,
        "registries/environments/access.schema.json",
        "/properties/access_references/items",
    ),
    (
        RegistryItem::ActionProfile,
        "registries/environments/access.schema.json",
        "/properties/action_profiles/items",
    ),
    (
        RegistryItem::Environment,
        "registries/environments/environments.schema.json",
        "/properties/environments/items",
    ),
    (
        RegistryItem::EnvironmentApplication,
        "registries/environments/environments.schema.json",
        "/properties/applications/items",
    ),
    (
        RegistryItem::TestDataReference,
        "registries/environments/test-data.schema.json",
        "/properties/references/items",
    ),
    (
        RegistryItem::ApplicabilityRecord,
        "registries/rule-resolution/applicability.schema.json",
        "/properties/records/items",
    ),
];

/// A Kernel schema the registry needs could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadRegistryError(pub String);

impl std::fmt::Display for PayloadRegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PayloadRegistryError {}

/// The identity of the record whose payload is checked.
#[derive(Debug, Clone, Copy)]
pub struct PayloadSubject<'a> {
    pub id: &'a SemanticId,
    pub scope: &'a Scope,
    pub title: &'a str,
    pub record_type: &'a str,
}

/// What an accepted payload is, typed. Types no check reads further are
/// [`TypedPayload::Conforming`]: accepted, and carrying nothing onwards.
#[derive(Debug, Clone)]
pub enum TypedPayload {
    RepositoryIdentity(RepositoryIdentity),
    IntakeRecord(IntakeRecord),
    Observation(GateRunObservation),
    Product(ProductPurity),
    ExternalDependencies(Vec<ExternalDependency>),
    WorkspaceFile(WorkspaceRelativePath),
    Conforming,
}

/// Why a payload was refused: each problem, in the order found. A
/// typed-product workspace file names which one it is, so the caller can
/// report it under its own family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadRejection {
    pub typed_file: Option<TypedWorkspaceFile>,
    pub problems: Vec<String>,
}

/// The closed payload contracts, with their Kernel schemas loaded once.
#[derive(Debug, Clone)]
pub struct PayloadRegistry {
    item_schemas: BTreeMap<RegistryItem, Value>,
}

impl PayloadRegistry {
    /// Loads every registry item schema from the Kernel and rejects one the
    /// validator cannot fully check.
    pub fn load(kernel: &dyn WorkspaceReader) -> Result<Self, PayloadRegistryError> {
        let mut roots: BTreeMap<&str, Value> = BTreeMap::new();
        let mut item_schemas = BTreeMap::new();
        for (item, path, pointer) in ITEM_SCHEMAS {
            if !roots.contains_key(path) {
                let rel = WorkspaceRelativePath::new(path)
                    .map_err(|e| PayloadRegistryError(format!("{path}: {e}")))?;
                let text = kernel
                    .read_text(&rel)
                    .map_err(|e| PayloadRegistryError(format!("{path} cannot be read: {e}")))?;
                let root: Value = serde_json::from_str(&text)
                    .map_err(|e| PayloadRegistryError(format!("{path} is not valid JSON: {e}")))?;
                roots.insert(path, root);
            }
            let root = &roots[path];
            if root.pointer(pointer).is_none() {
                return Err(PayloadRegistryError(format!(
                    "{path} has no item schema at #{pointer}"
                )));
            }
            let mut definitions = root
                .get("definitions")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            definitions.insert("__registry".to_string(), root.clone());
            let wrapper = json!({
                "definitions": definitions,
                "$ref": format!("#/definitions/__registry{pointer}"),
            });
            json_schema::assert_supported_deep(&wrapper, path)
                .map_err(|e| PayloadRegistryError(format!("{path}: {e}")))?;
            item_schemas.insert(item, wrapper);
        }
        Ok(Self { item_schemas })
    }

    /// The whole pass for one record's payload.
    pub fn check(
        &self,
        subject: PayloadSubject<'_>,
        payload: &Map<String, Value>,
    ) -> Result<TypedPayload, PayloadRejection> {
        let reject = |problems: Vec<String>| PayloadRejection {
            typed_file: None,
            problems,
        };
        let Some(record_type) = ProductRecordType::parse(subject.record_type) else {
            return Err(reject(vec![format!(
                "record_type \"{}\" has no payload contract; a product record type is added to the registry by decision, never accepted unchecked",
                subject.record_type
            )]));
        };
        let container: ContainerDto = serde_json::from_value(Value::Object(payload.clone()))
            .map_err(|e| reject(vec![format!("payload is not a content container: {e}")]))?;
        let mut problems = Vec::new();
        if !record_type.accepts_media_type(&container.media_type) {
            problems.push(format!(
                "payload.media_type \"{}\" is not the one a {record_type} record carries",
                container.media_type
            ));
        }
        if container.encoding != "utf-8" {
            problems.push(format!(
                "payload.encoding \"{}\" is not \"utf-8\"",
                container.encoding
            ));
        }
        let actual = ContentDigest::of_str(&container.content);
        if container.digest.algorithm != actual.algorithm().as_str()
            || container.digest.value != actual.value()
        {
            problems.push(format!(
                "payload.digest is not the {} of payload.content",
                actual.algorithm().as_str()
            ));
        }
        if !problems.is_empty() {
            return Err(reject(problems));
        }
        let content = container.content.as_str();
        match record_type.contract() {
            PayloadContract::RegistryItem(item) => {
                let value = parse_json(content).map_err(|p| reject(vec![p]))?;
                let Some(schema) = self.item_schemas.get(&item) else {
                    return Err(reject(vec![format!(
                        "no Kernel schema is loaded for {record_type} records"
                    )]));
                };
                let errors = json_schema::validate(&value, schema)
                    .map_err(|e| reject(vec![format!("content cannot be checked: {e}")]))?;
                if !errors.is_empty() {
                    return Err(reject(
                        errors.into_iter().map(|e| format!("content {e}")).collect(),
                    ));
                }
                match item {
                    RegistryItem::InventoryRepository => from_json::<RepositoryIdentityDto>(value)
                        .and_then(RepositoryIdentityDto::convert)
                        .map(TypedPayload::RepositoryIdentity),
                    RegistryItem::IntakeRecord => from_json::<IntakeRecordDto>(value)
                        .and_then(|dto| dto.convert(subject.id, subject.scope))
                        .map(TypedPayload::IntakeRecord),
                    _ => Ok(TypedPayload::Conforming),
                }
                .map_err(|p| reject(vec![p]))
            }
            PayloadContract::JsonObject => match parse_json(content) {
                Ok(Value::Object(_)) => Ok(TypedPayload::Conforming),
                Ok(_) => Err(reject(vec!["content is not a JSON object".to_string()])),
                Err(p) => Err(reject(vec![p])),
            },
            PayloadContract::GateRunObservation => parse_json(content)
                .and_then(from_json::<ObservationDto>)
                .and_then(ObservationDto::convert)
                .map(TypedPayload::Observation)
                .map_err(|p| reject(vec![p])),
            PayloadContract::Markdown => Ok(TypedPayload::Conforming),
            PayloadContract::WorkspaceText => {
                let path = WorkspaceRelativePath::new(subject.title).map_err(|e| {
                    reject(vec![format!(
                        "title \"{}\" is not the workspace-relative path of the file: {e}",
                        subject.title
                    )])
                })?;
                let Some(typed) = TypedWorkspaceFile::for_path(path.as_str()) else {
                    return Ok(TypedPayload::WorkspaceFile(path));
                };
                let typed_reject = |problem: String| PayloadRejection {
                    typed_file: Some(typed),
                    problems: vec![problem],
                };
                let document = parse_yaml(content)
                    .map_err(|e| typed_reject(format!("{} does not parse: {e}", typed.path())))?;
                match typed {
                    TypedWorkspaceFile::Product => from_json::<ProductDto>(document)
                        .and_then(ProductDto::convert)
                        .map(TypedPayload::Product),
                    TypedWorkspaceFile::ExternalDependencies => {
                        from_json::<ExternalDependenciesDto>(document)
                            .and_then(ExternalDependenciesDto::convert)
                            .map(TypedPayload::ExternalDependencies)
                    }
                }
                .map_err(typed_reject)
            }
        }
    }
}

fn parse_json(content: &str) -> Result<Value, String> {
    serde_json::from_str(content).map_err(|e| format!("content is not valid JSON: {e}"))
}

fn from_json<T: serde::de::DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value).map_err(|e| format!("content is not the closed shape: {e}"))
}
