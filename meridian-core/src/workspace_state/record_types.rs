//! M-05b — the closed set of product record types a workspace database may
//! hold, and the payload contract each one carries.
//!
//! Every stored payload is a content container (`media_type`, `encoding`,
//! `content`, `digest`). What the container must hold is decided here, by
//! record type, and nowhere else: a record type outside this set has no
//! contract and is refused, never silently accepted. The app maps each
//! [`PayloadContract::RegistryItem`] to the Kernel registry schema that
//! describes it and runs the full transport/schema/domain pass.

use core::fmt;

/// Every product record type the migration of a workspace produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProductRecordType {
    // Registry items with a Kernel schema.
    RepositoryIdentity,
    RepositoryReference,
    RepositoryRelationship,
    InstructionIntakeRecord,
    Command,
    AccessApplication,
    AccessReference,
    ActionProfile,
    Environment,
    EnvironmentApplication,
    TestDataReference,
    RuleApplicabilityRecord,
    // Structured JSON the Kernel declares no schema for.
    VersioningConformanceCase,
    SmokeAccess,
    SmokeCapability,
    SmokeEnvironment,
    SmokeTestData,
    // The gate's own run log.
    GateRunObservation,
    // Documents.
    Adr,
    Analysis,
    Concept,
    Contract,
    HowTo,
    Plan,
    Problem,
    Protocol,
    Reference,
    Report,
    Rfc,
    Standard,
    TechnicalSpecification,
    // A workspace file kept byte for byte.
    WorkspaceFile,
}

/// The Kernel registry an item record belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RegistryItem {
    InventoryRepository,
    InventoryReference,
    InventoryRelationship,
    IntakeRecord,
    RepositoryCommand,
    AccessApplication,
    AccessReference,
    ActionProfile,
    Environment,
    EnvironmentApplication,
    TestDataReference,
    ApplicabilityRecord,
}

/// What a record type's container must hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadContract {
    /// `application/json` content that is one item of a Kernel registry and
    /// must satisfy that item's schema.
    RegistryItem(RegistryItem),
    /// `application/json` content that must be one JSON object.
    JsonObject,
    /// `application/json` content that is one gate-run observation.
    GateRunObservation,
    /// `text/markdown` content.
    Markdown,
    /// Any `text/*` content of a workspace file; two workspace paths carry
    /// typed product configuration (see [`TypedWorkspaceFile`]).
    WorkspaceText,
}

const ALL: [(&str, ProductRecordType); 32] = [
    ("repository-identity", ProductRecordType::RepositoryIdentity),
    (
        "repository-reference",
        ProductRecordType::RepositoryReference,
    ),
    (
        "repository-relationship",
        ProductRecordType::RepositoryRelationship,
    ),
    (
        "instruction-intake-record",
        ProductRecordType::InstructionIntakeRecord,
    ),
    ("command", ProductRecordType::Command),
    ("access-application", ProductRecordType::AccessApplication),
    ("access-reference", ProductRecordType::AccessReference),
    ("action-profile", ProductRecordType::ActionProfile),
    ("environment", ProductRecordType::Environment),
    (
        "environment-application",
        ProductRecordType::EnvironmentApplication,
    ),
    ("test-data-reference", ProductRecordType::TestDataReference),
    (
        "rule-applicability-record",
        ProductRecordType::RuleApplicabilityRecord,
    ),
    (
        "versioning-conformance-case",
        ProductRecordType::VersioningConformanceCase,
    ),
    ("smoke-access", ProductRecordType::SmokeAccess),
    ("smoke-capability", ProductRecordType::SmokeCapability),
    ("smoke-environment", ProductRecordType::SmokeEnvironment),
    ("smoke-test-data", ProductRecordType::SmokeTestData),
    (
        "gate-run-observation",
        ProductRecordType::GateRunObservation,
    ),
    ("adr", ProductRecordType::Adr),
    ("analysis", ProductRecordType::Analysis),
    ("concept", ProductRecordType::Concept),
    ("contract", ProductRecordType::Contract),
    ("how-to", ProductRecordType::HowTo),
    ("plan", ProductRecordType::Plan),
    ("problem", ProductRecordType::Problem),
    ("protocol", ProductRecordType::Protocol),
    ("reference", ProductRecordType::Reference),
    ("report", ProductRecordType::Report),
    ("rfc", ProductRecordType::Rfc),
    ("standard", ProductRecordType::Standard),
    (
        "technical-specification",
        ProductRecordType::TechnicalSpecification,
    ),
    ("workspace-file", ProductRecordType::WorkspaceFile),
];

impl ProductRecordType {
    /// Every product record type, in registry order.
    pub fn all() -> impl Iterator<Item = ProductRecordType> {
        ALL.iter().map(|(_, t)| *t)
    }

    /// The type named by `value`, or `None` when no contract exists for it.
    pub fn parse(value: &str) -> Option<Self> {
        ALL.iter().find(|(name, _)| *name == value).map(|(_, t)| *t)
    }

    pub fn as_str(self) -> &'static str {
        ALL.iter()
            .find(|(_, t)| *t == self)
            .map(|(name, _)| *name)
            .unwrap_or("workspace-file")
    }

    pub fn contract(self) -> PayloadContract {
        use ProductRecordType as T;
        use RegistryItem as R;
        match self {
            T::RepositoryIdentity => PayloadContract::RegistryItem(R::InventoryRepository),
            T::RepositoryReference => PayloadContract::RegistryItem(R::InventoryReference),
            T::RepositoryRelationship => PayloadContract::RegistryItem(R::InventoryRelationship),
            T::InstructionIntakeRecord => PayloadContract::RegistryItem(R::IntakeRecord),
            T::Command => PayloadContract::RegistryItem(R::RepositoryCommand),
            T::AccessApplication => PayloadContract::RegistryItem(R::AccessApplication),
            T::AccessReference => PayloadContract::RegistryItem(R::AccessReference),
            T::ActionProfile => PayloadContract::RegistryItem(R::ActionProfile),
            T::Environment => PayloadContract::RegistryItem(R::Environment),
            T::EnvironmentApplication => PayloadContract::RegistryItem(R::EnvironmentApplication),
            T::TestDataReference => PayloadContract::RegistryItem(R::TestDataReference),
            T::RuleApplicabilityRecord => PayloadContract::RegistryItem(R::ApplicabilityRecord),
            T::VersioningConformanceCase
            | T::SmokeAccess
            | T::SmokeCapability
            | T::SmokeEnvironment
            | T::SmokeTestData => PayloadContract::JsonObject,
            T::GateRunObservation => PayloadContract::GateRunObservation,
            T::Adr
            | T::Analysis
            | T::Concept
            | T::Contract
            | T::HowTo
            | T::Plan
            | T::Problem
            | T::Protocol
            | T::Reference
            | T::Report
            | T::Rfc
            | T::Standard
            | T::TechnicalSpecification => PayloadContract::Markdown,
            T::WorkspaceFile => PayloadContract::WorkspaceText,
        }
    }

    /// Whether `media_type` is the one this type's contract carries.
    pub fn accepts_media_type(self, media_type: &str) -> bool {
        match self.contract() {
            PayloadContract::RegistryItem(_)
            | PayloadContract::JsonObject
            | PayloadContract::GateRunObservation => media_type == "application/json",
            PayloadContract::Markdown => media_type == "text/markdown",
            PayloadContract::WorkspaceText => media_type.starts_with("text/"),
        }
    }
}

impl fmt::Display for ProductRecordType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The two workspace files whose content is typed product configuration,
/// named by their workspace-relative path — the contract key, not a file
/// the validator reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TypedWorkspaceFile {
    /// `product.yaml` (M-02/M-03).
    Product,
    /// `external-dependencies.yaml` (M-07).
    ExternalDependencies,
}

impl TypedWorkspaceFile {
    pub fn for_path(path: &str) -> Option<Self> {
        match path {
            "product.yaml" => Some(TypedWorkspaceFile::Product),
            "external-dependencies.yaml" => Some(TypedWorkspaceFile::ExternalDependencies),
            _ => None,
        }
    }

    pub fn path(self) -> &'static str {
        match self {
            TypedWorkspaceFile::Product => "product.yaml",
            TypedWorkspaceFile::ExternalDependencies => "external-dependencies.yaml",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_state_record_types_round_trip_and_refuse_an_unknown_name() {
        for (name, t) in ALL {
            assert_eq!(ProductRecordType::parse(name), Some(t));
            assert_eq!(t.as_str(), name);
        }
        assert_eq!(ProductRecordType::parse("inventory-item"), None);
        assert_eq!(ProductRecordType::parse(""), None);
    }

    #[test]
    fn workspace_state_record_types_bind_each_contract_to_one_media_type() {
        assert!(ProductRecordType::RepositoryIdentity.accepts_media_type("application/json"));
        assert!(!ProductRecordType::RepositoryIdentity.accepts_media_type("text/yaml"));
        assert!(ProductRecordType::Plan.accepts_media_type("text/markdown"));
        assert!(!ProductRecordType::Plan.accepts_media_type("text/plain"));
        assert!(ProductRecordType::WorkspaceFile.accepts_media_type("text/yaml"));
        assert!(!ProductRecordType::WorkspaceFile.accepts_media_type("application/json"));
        assert_eq!(
            TypedWorkspaceFile::for_path("product.yaml"),
            Some(TypedWorkspaceFile::Product)
        );
        assert_eq!(TypedWorkspaceFile::for_path("notes/product.yaml"), None);
    }
}
