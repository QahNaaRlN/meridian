//! [`ApplicabilityRecord`] — one entry of the applicability register
//! (`registries/rule-resolution/applicability.schema.json`), and the types
//! it is built from.
//!
//! Every conditionally-required field of the schema's `allOf` is expressed
//! as data carried only by the enum variant it is required under
//! ([`ApplicabilityScope`], [`Activation`], [`ApplicabilitySource`],
//! [`ApplicabilityStatus`]), so a record that violates one of those
//! conditions cannot be constructed — the schema's cross-field validation
//! becomes a compile-time impossibility instead of a runtime check repeated
//! at every call site.

use core::fmt;

use crate::types::ContentDigest;

use super::date::IsoDate;
use super::work_item::{ChangeClass, WorkKind};

/// A value rejected while building an applicability type in this module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicabilityError {
    /// A required string field was empty.
    EmptyField {
        /// The field name, for a precise diagnostic.
        field: &'static str,
    },
    /// `norm.region` does not match `^[a-z0-9]+([._-][a-z0-9]+)*$`.
    InvalidRegion {
        /// The rejected value.
        value: String,
    },
    /// `activation: path-glob` was declared with an empty `globs` list.
    EmptyGlobs,
    /// A `task_class.change_class` was declared but `task_class.work_kind`
    /// does not contain `change` (`applicability.schema.json`
    /// `definitions.record.properties.task_class.allOf`).
    ChangeClassWithoutChangeWorkKind,
    /// `task_class.work_kind` was declared empty.
    EmptyWorkKindList,
}

impl fmt::Display for ApplicabilityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApplicabilityError::EmptyField { field } => write!(f, "{field} must not be empty"),
            ApplicabilityError::InvalidRegion { value } => write!(
                f,
                "\"{value}\" is not a valid region id (expected ^[a-z0-9]+([._-][a-z0-9]+)*$)"
            ),
            ApplicabilityError::EmptyGlobs => {
                write!(f, "activation \"path-glob\" requires at least one glob")
            }
            ApplicabilityError::ChangeClassWithoutChangeWorkKind => write!(
                f,
                "task_class.change_class is declared but task_class.work_kind does not contain \"change\""
            ),
            ApplicabilityError::EmptyWorkKindList => write!(f, "task_class.work_kind must not be empty"),
        }
    }
}

impl std::error::Error for ApplicabilityError {}

fn require_non_empty(
    field: &'static str,
    value: impl Into<String>,
) -> Result<String, ApplicabilityError> {
    let value = value.into();
    if value.is_empty() {
        return Err(ApplicabilityError::EmptyField { field });
    }
    Ok(value)
}

fn is_valid_region(value: &str) -> bool {
    value.split(['.', '_', '-']).all(|seg| {
        !seg.is_empty()
            && seg
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    }) && !value.is_empty()
}

/// Qualified reference to where a norm's text is recorded
/// (`applicability.schema.json` `definitions.record.properties.norm`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NormRef {
    repository: String,
    path: String,
    region: Option<String>,
}

impl NormRef {
    pub fn new(
        repository: impl Into<String>,
        path: impl Into<String>,
        region: Option<String>,
    ) -> Result<Self, ApplicabilityError> {
        let repository = require_non_empty("norm.repository", repository)?;
        let path = require_non_empty("norm.path", path)?;
        let region = match region {
            Some(r) if is_valid_region(&r) => Some(r),
            Some(r) => return Err(ApplicabilityError::InvalidRegion { value: r }),
            None => None,
        };
        Ok(Self {
            repository,
            path,
            region,
        })
    }

    pub fn repository(&self) -> &str {
        &self.repository
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn region(&self) -> Option<&str> {
        self.region.as_deref()
    }
}

/// The closed pool of instruction-intake verdicts
/// (`registries/instruction-intake/intake.schema.json`, reused as a pointer
/// component by `intake_record`/`supersedes`, never as a precedence rank).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntakeVerdict {
    AdoptCore,
    AdoptEdition,
    KeepLocal,
    MergeInto,
    Rename,
    Retire,
    Deferred,
}

impl IntakeVerdict {
    pub fn as_str(self) -> &'static str {
        match self {
            IntakeVerdict::AdoptCore => "adopt-core",
            IntakeVerdict::AdoptEdition => "adopt-edition",
            IntakeVerdict::KeepLocal => "keep-local",
            IntakeVerdict::MergeInto => "merge-into",
            IntakeVerdict::Rename => "rename",
            IntakeVerdict::Retire => "retire",
            IntakeVerdict::Deferred => "deferred",
        }
    }
}

impl fmt::Display for IntakeVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Unambiguous pointer to one append-only instruction-intake record
/// (`applicability.schema.json` `definitions.record.properties.intake_record`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IntakePointer {
    register: String,
    recorded_at: IsoDate,
    verdict: IntakeVerdict,
    revision: Option<String>,
}

impl IntakePointer {
    pub fn new(
        register: impl Into<String>,
        recorded_at: IsoDate,
        verdict: IntakeVerdict,
        revision: Option<String>,
    ) -> Result<Self, ApplicabilityError> {
        let register = require_non_empty("intake_record.register", register)?;
        Ok(Self {
            register,
            recorded_at,
            verdict,
            revision,
        })
    }

    pub fn register(&self) -> &str {
        &self.register
    }
    pub fn recorded_at(&self) -> &IsoDate {
        &self.recorded_at
    }
    pub fn verdict(&self) -> IntakeVerdict {
        self.verdict
    }
    pub fn revision(&self) -> Option<&str> {
        self.revision.as_deref()
    }
}

/// Pointer identical in shape to [`IntakePointer`] plus the superseded
/// record's own `norm.path`/`norm.region` (`applicability.schema.json`
/// `definitions.record.properties.supersedes`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SupersedesPointer {
    register: String,
    path: String,
    region: Option<String>,
    recorded_at: IsoDate,
    verdict: IntakeVerdict,
    revision: Option<String>,
}

impl SupersedesPointer {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        register: impl Into<String>,
        path: impl Into<String>,
        region: Option<String>,
        recorded_at: IsoDate,
        verdict: IntakeVerdict,
        revision: Option<String>,
    ) -> Result<Self, ApplicabilityError> {
        let register = require_non_empty("supersedes.register", register)?;
        let path = require_non_empty("supersedes.path", path)?;
        Ok(Self {
            register,
            path,
            region,
            recorded_at,
            verdict,
            revision,
        })
    }

    pub fn register(&self) -> &str {
        &self.register
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn region(&self) -> Option<&str> {
        self.region.as_deref()
    }
    pub fn recorded_at(&self) -> &IsoDate {
        &self.recorded_at
    }
    pub fn verdict(&self) -> IntakeVerdict {
        self.verdict
    }
    pub fn revision(&self) -> Option<&str> {
        self.revision.as_deref()
    }
}

/// Narrows `activation: task-class` to specific `work_kind`/`change_class`
/// values (`applicability.schema.json`
/// `definitions.record.properties.task_class`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskClassSelector {
    work_kind: Vec<WorkKind>,
    change_class: Option<Vec<ChangeClass>>,
}

impl TaskClassSelector {
    /// `change_class`, when present, requires `work_kind` to contain
    /// `WorkKind::Change` — an incompatible pairing (for example
    /// `work_kind: [assessment]` with a `change_class`) is rejected here,
    /// exactly as the schema's `allOf` requires.
    pub fn new(
        work_kind: Vec<WorkKind>,
        change_class: Option<Vec<ChangeClass>>,
    ) -> Result<Self, ApplicabilityError> {
        if work_kind.is_empty() {
            return Err(ApplicabilityError::EmptyWorkKindList);
        }
        if let Some(cc) = &change_class {
            if cc.is_empty() || !work_kind.contains(&WorkKind::Change) {
                return Err(ApplicabilityError::ChangeClassWithoutChangeWorkKind);
            }
        }
        Ok(Self {
            work_kind,
            change_class,
        })
    }

    pub fn work_kind(&self) -> &[WorkKind] {
        &self.work_kind
    }
    pub fn change_class(&self) -> Option<&[ChangeClass]> {
        self.change_class.as_deref()
    }
}

/// The area an applicability record applies to
/// (`rule-resolution.md` §5), conjoined with the field each area requires.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ApplicabilityScope {
    Universal,
    Profile { technology_profile: String },
    Repository { repository: String },
    ProductDomain { product_domain: String },
}

/// The activation axis (`rule-resolution.md` §5), conjoined with the field
/// each activation kind requires.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Activation {
    Always,
    PathGlob { globs: Vec<String> },
    TaskClass { task_class: TaskClassSelector },
    Explicit,
    Undetermined,
}

impl Activation {
    pub fn path_glob(globs: Vec<String>) -> Result<Self, ApplicabilityError> {
        if globs.is_empty() {
            return Err(ApplicabilityError::EmptyGlobs);
        }
        Ok(Activation::PathGlob { globs })
    }

    /// The plain five-value discriminant, reused verbatim by
    /// `applicable_norm.activation_reason` in the resolver output.
    pub fn kind(&self) -> ActivationKind {
        match self {
            Activation::Always => ActivationKind::Always,
            Activation::PathGlob { .. } => ActivationKind::PathGlob,
            Activation::TaskClass { .. } => ActivationKind::TaskClass,
            Activation::Explicit => ActivationKind::Explicit,
            Activation::Undetermined => ActivationKind::Undetermined,
        }
    }
}

/// The plain discriminant of [`Activation`]
/// (`resolver-output.schema.json` `applicable_norm.activation_reason`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActivationKind {
    Always,
    PathGlob,
    TaskClass,
    Explicit,
    Undetermined,
}

impl ActivationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ActivationKind::Always => "always",
            ActivationKind::PathGlob => "path-glob",
            ActivationKind::TaskClass => "task-class",
            ActivationKind::Explicit => "explicit",
            ActivationKind::Undetermined => "undetermined",
        }
    }
}

impl fmt::Display for ActivationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where an applicability record's norm comes from
/// (`applicability.schema.json` `definitions.record.properties.source`),
/// carrying the intake pointer every non-kernel record requires and the
/// `kernel` variant's structural inability to carry one or a `supersedes`
/// pointer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ApplicabilitySource {
    Kernel,
    Repository {
        intake_record: IntakePointer,
        supersedes: Option<SupersedesPointer>,
    },
    DeliveryAdapter {
        intake_record: IntakePointer,
        supersedes: Option<SupersedesPointer>,
    },
}

impl ApplicabilitySource {
    pub fn intake_record(&self) -> Option<&IntakePointer> {
        match self {
            ApplicabilitySource::Kernel => None,
            ApplicabilitySource::Repository { intake_record, .. }
            | ApplicabilitySource::DeliveryAdapter { intake_record, .. } => Some(intake_record),
        }
    }

    pub fn supersedes(&self) -> Option<&SupersedesPointer> {
        match self {
            ApplicabilitySource::Kernel => None,
            ApplicabilitySource::Repository { supersedes, .. }
            | ApplicabilitySource::DeliveryAdapter { supersedes, .. } => supersedes.as_ref(),
        }
    }

    pub fn as_str(&self) -> &'static str {
        self.kind().as_str()
    }

    /// The plain three-value discriminant, reused verbatim by
    /// `applicable_norm.source` in the resolver output
    /// (`resolver-output.schema.json`: "Same three values as
    /// `applicability.schema.json` `record.source`").
    pub fn kind(&self) -> ApplicabilitySourceKind {
        match self {
            ApplicabilitySource::Kernel => ApplicabilitySourceKind::Kernel,
            ApplicabilitySource::Repository { .. } => ApplicabilitySourceKind::Repository,
            ApplicabilitySource::DeliveryAdapter { .. } => ApplicabilitySourceKind::DeliveryAdapter,
        }
    }
}

/// The plain discriminant of [`ApplicabilitySource`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ApplicabilitySourceKind {
    Kernel,
    Repository,
    DeliveryAdapter,
}

impl ApplicabilitySourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ApplicabilitySourceKind::Kernel => "kernel",
            ApplicabilitySourceKind::Repository => "repository",
            ApplicabilitySourceKind::DeliveryAdapter => "delivery-adapter",
        }
    }
}

impl fmt::Display for ApplicabilitySourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// This record's own runtime-applicability state
/// (`rule-resolution.md` §4): exactly two states, and `resume_condition` is
/// carried only by `Unresolved`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ApplicabilityStatus {
    Resolved,
    Unresolved { resume_condition: String },
}

/// The closed pool of delivery adapters
/// (`registries/rule-resolution/resolver-output.schema.json`
/// `definitions.applicable_norm.properties.delivery`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Delivery {
    KernelDoc,
    SkillPackage,
    CursorRule,
    AgentsMdSection,
}

impl Delivery {
    pub fn as_str(self) -> &'static str {
        match self {
            Delivery::KernelDoc => "kernel-doc",
            Delivery::SkillPackage => "skill-package",
            Delivery::CursorRule => "cursor-rule",
            Delivery::AgentsMdSection => "agents-md-section",
        }
    }
}

impl fmt::Display for Delivery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One record of the applicability register
/// (`registries/rule-resolution/applicability.schema.json`
/// `definitions.record`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ApplicabilityRecord {
    norm: NormRef,
    digest: ContentDigest,
    scope: ApplicabilityScope,
    activation: Activation,
    source: ApplicabilitySource,
    status: ApplicabilityStatus,
    recorded_at: IsoDate,
}

impl ApplicabilityRecord {
    pub fn new(
        norm: NormRef,
        digest: ContentDigest,
        scope: ApplicabilityScope,
        activation: Activation,
        source: ApplicabilitySource,
        status: ApplicabilityStatus,
        recorded_at: IsoDate,
    ) -> Self {
        Self {
            norm,
            digest,
            scope,
            activation,
            source,
            status,
            recorded_at,
        }
    }

    pub fn norm(&self) -> &NormRef {
        &self.norm
    }
    pub fn digest(&self) -> &ContentDigest {
        &self.digest
    }
    pub fn scope(&self) -> &ApplicabilityScope {
        &self.scope
    }
    pub fn activation(&self) -> &Activation {
        &self.activation
    }
    pub fn source(&self) -> &ApplicabilitySource {
        &self.source
    }
    pub fn status(&self) -> &ApplicabilityStatus {
        &self.status
    }
    pub fn recorded_at(&self) -> &IsoDate {
        &self.recorded_at
    }

    /// `normId` (`rule-resolver.mjs`): the Kernel path when this record's
    /// source is `kernel`, otherwise the full intake pointer.
    pub fn norm_id(&self) -> String {
        match self.source.intake_record() {
            None => self.norm.path.clone(),
            Some(p) => {
                let region = self
                    .norm
                    .region
                    .as_deref()
                    .map(|r| format!(":{r}"))
                    .unwrap_or_default();
                format!(
                    "{}#{}{}@{}/{}",
                    p.register(),
                    self.norm.path,
                    region,
                    p.recorded_at(),
                    p.verdict()
                )
            }
        }
    }

    /// `normTextKey` (`rule-resolver.mjs`): the key `norm_texts` is looked
    /// up by.
    pub fn norm_text_key(&self) -> String {
        let region = self
            .norm
            .region
            .as_deref()
            .map(|r| format!("::{r}"))
            .unwrap_or_default();
        format!("{}::{}{}", self.norm.repository, self.norm.path, region)
    }

    /// `k` in `resolveNorms`'s grouping (`rule-resolver.mjs`): the identity
    /// a norm's applicability records are grouped by.
    pub fn identity_key(&self) -> String {
        format!(
            "{}::{}::{}",
            self.norm.repository,
            self.norm.path,
            self.norm.region.as_deref().unwrap_or("")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest() -> ContentDigest {
        ContentDigest::of_str("norm text")
    }

    #[test]
    fn task_class_requires_change_work_kind_alongside_change_class() {
        assert!(
            TaskClassSelector::new(vec![WorkKind::Change], Some(vec![ChangeClass::Bugfix])).is_ok()
        );
        assert_eq!(
            TaskClassSelector::new(vec![WorkKind::Assessment], Some(vec![ChangeClass::Bugfix]))
                .unwrap_err(),
            ApplicabilityError::ChangeClassWithoutChangeWorkKind
        );
    }

    #[test]
    fn task_class_rejects_empty_work_kind() {
        assert_eq!(
            TaskClassSelector::new(vec![], None).unwrap_err(),
            ApplicabilityError::EmptyWorkKindList
        );
    }

    #[test]
    fn path_glob_rejects_empty_globs() {
        assert_eq!(
            Activation::path_glob(vec![]).unwrap_err(),
            ApplicabilityError::EmptyGlobs
        );
        assert!(Activation::path_glob(vec!["src/**".to_string()]).is_ok());
    }

    #[test]
    fn kernel_source_carries_no_intake_record_or_supersedes() {
        let source = ApplicabilitySource::Kernel;
        assert_eq!(source.intake_record(), None);
        assert_eq!(source.supersedes(), None);
        assert_eq!(source.as_str(), "kernel");
    }

    #[test]
    fn norm_ref_validates_region_pattern() {
        assert!(NormRef::new("kernel", "standards/x.md", Some("scope-1".to_string())).is_ok());
        assert!(matches!(
            NormRef::new("kernel", "standards/x.md", Some("Scope 1".to_string())).unwrap_err(),
            ApplicabilityError::InvalidRegion { .. }
        ));
    }

    #[test]
    fn norm_id_is_the_kernel_path_for_kernel_source() {
        let rec = ApplicabilityRecord::new(
            NormRef::new("kernel", "standards/x.md", None).unwrap(),
            digest(),
            ApplicabilityScope::Universal,
            Activation::Always,
            ApplicabilitySource::Kernel,
            ApplicabilityStatus::Resolved,
            IsoDate::new("2026-09-08").unwrap(),
        );
        assert_eq!(rec.norm_id(), "standards/x.md");
    }

    #[test]
    fn norm_id_is_the_full_intake_pointer_for_non_kernel_source() {
        let intake = IntakePointer::new(
            "instruction-intake/sample.yaml",
            IsoDate::new("2026-09-01").unwrap(),
            IntakeVerdict::AdoptCore,
            None,
        )
        .unwrap();
        let rec = ApplicabilityRecord::new(
            NormRef::new(
                "sample-core-frontend",
                "AGENTS.md",
                Some("scope-1".to_string()),
            )
            .unwrap(),
            digest(),
            ApplicabilityScope::Universal,
            Activation::Always,
            ApplicabilitySource::Repository {
                intake_record: intake,
                supersedes: None,
            },
            ApplicabilityStatus::Resolved,
            IsoDate::new("2026-09-08").unwrap(),
        );
        assert_eq!(
            rec.norm_id(),
            "instruction-intake/sample.yaml#AGENTS.md:scope-1@2026-09-01/adopt-core"
        );
    }
}
