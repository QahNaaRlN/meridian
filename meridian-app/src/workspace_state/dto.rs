//! Closed transport shapes of the typed payloads and their conversion into
//! strict `meridian_core::workspace_state` values. Every struct denies
//! unknown fields; a field this crate does not read is still declared (as
//! [`IgnoredAny`]) so the shape stays closed without carrying a `Value`
//! past this boundary.

use serde::de::IgnoredAny;
use serde::Deserialize;

use meridian_core::types::{
    ContentDigest, NonEmptyString, RepositoryId, Scope, ScopeType, SemanticId,
    WorkspaceRelativePath,
};
use meridian_core::workspace_state::dependencies::{DependencyState, ExternalDependency};
use meridian_core::workspace_state::intake::{
    Delivery, IntakeFields, IntakeRecord, IntakeTopic, IntakeVerdict,
};
use meridian_core::workspace_state::inventory::{RecordedVcs, RepositoryIdentity, WorkingTree};
use meridian_core::workspace_state::observation::{GateRunObservation, RunCounts};
use meridian_core::workspace_state::parentage::{
    CommitRevision, ParentReference, ParentRepository,
};
use meridian_core::workspace_state::product::{ForbiddenLiteral, ForbiddenPattern, ProductPurity};
use meridian_core::workspace_state::timestamp::Timestamp;

pub(crate) type Converted<T> = Result<T, String>;

fn text(field: &str, value: String) -> Converted<NonEmptyString> {
    NonEmptyString::new(value).map_err(|e| format!("{field}: {e}"))
}

/// The content container every product payload is.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContainerDto {
    pub media_type: String,
    pub encoding: String,
    pub content: String,
    pub digest: DigestDto,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DigestDto {
    pub algorithm: String,
    pub value: String,
}

// ---------------------------------------------------------------- inventory

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RepositoryIdentityDto {
    id: String,
    path: String,
    #[allow(dead_code)]
    role: IgnoredAny,
    profile: String,
    #[allow(dead_code)]
    #[serde(default)]
    ownership: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    products: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    semantic_areas: Option<IgnoredAny>,
    sources: SourcesDto,
    vcs: VcsDto,
    last_verified: String,
    #[allow(dead_code)]
    #[serde(default)]
    notes: Option<IgnoredAny>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourcesDto {
    #[allow(dead_code)]
    rules: IgnoredAny,
    manifests: Vec<String>,
    #[allow(dead_code)]
    #[serde(default)]
    other: Option<IgnoredAny>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VcsDto {
    #[serde(rename = "type")]
    vcs_type: String,
    revision: String,
    #[serde(rename = "ref")]
    git_ref: String,
    working_tree: String,
    #[allow(dead_code)]
    evidence_basis: IgnoredAny,
}

impl RepositoryIdentityDto {
    pub(crate) fn convert(self) -> Converted<RepositoryIdentity> {
        if self.vcs.vcs_type != "git" {
            return Err(format!(
                "vcs.type: \"{}\" is not \"git\"",
                self.vcs.vcs_type
            ));
        }
        let working_tree = WorkingTree::parse(&self.vcs.working_tree).ok_or_else(|| {
            format!(
                "vcs.working_tree: \"{}\" is neither \"clean\" nor \"dirty\"",
                self.vcs.working_tree
            )
        })?;
        let manifests = self
            .sources
            .manifests
            .into_iter()
            .map(|m| text("sources.manifests", m))
            .collect::<Converted<Vec<_>>>()?;
        Ok(RepositoryIdentity::new(
            RepositoryId::new(self.id).map_err(|e| format!("id: {e}"))?,
            text("path", self.path)?,
            self.profile,
            RecordedVcs {
                revision: text("vcs.revision", self.vcs.revision)?,
                git_ref: text("vcs.ref", self.vcs.git_ref)?,
                working_tree,
            },
            Timestamp::parse_iso8601(&self.last_verified),
            manifests,
        ))
    }
}

// ------------------------------------------------------------------- intake

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct IntakeRecordDto {
    artifact: String,
    #[serde(default)]
    region: Option<String>,
    #[allow(dead_code)]
    digest: IgnoredAny,
    delivery: String,
    topic: String,
    #[allow(dead_code)]
    #[serde(default)]
    unclassified_reason: Option<IgnoredAny>,
    #[allow(dead_code)]
    genre: IgnoredAny,
    #[allow(dead_code)]
    scope: IgnoredAny,
    #[allow(dead_code)]
    #[serde(default)]
    profile: Option<IgnoredAny>,
    #[allow(dead_code)]
    observed_activation: IgnoredAny,
    verdict: String,
    #[allow(dead_code)]
    verdict_basis: IgnoredAny,
    #[serde(default)]
    derived_from: Option<DerivedFromDto>,
    #[serde(default)]
    derived_from_digest: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    narrowing: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    merge_target: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    new_topic_name: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    successor: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    retire_reason: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    resume_condition: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    core_lines: Option<IgnoredAny>,
    recorded_at: String,
    #[allow(dead_code)]
    #[serde(default)]
    applied_at: Option<IgnoredAny>,
}

/// The qualified parent reference of an edition.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DerivedFromDto {
    repository: String,
    path: String,
    revision: String,
}

impl IntakeRecordDto {
    /// The record's repository is its own `repository-scope`. An edition's
    /// parent is part of its verdict; the parent of any other verdict is not
    /// read (the schema requires it of an edition alone).
    pub(crate) fn convert(self, id: &SemanticId, scope: &Scope) -> Converted<IntakeRecord> {
        if scope.scope_type() != ScopeType::RepositoryScope {
            return Err(format!(
                "scope: an intake record belongs to one repository (repository-scope), not to a {} scope",
                scope.scope_type().as_str()
            ));
        }
        let verdict = match self.verdict.as_str() {
            "adopt-core" => IntakeVerdict::AdoptCore,
            "adopt-edition" => {
                let parent = self.derived_from.ok_or_else(|| {
                    "derived_from: an adopt-edition record names its parent".to_string()
                })?;
                let digest = self.derived_from_digest.ok_or_else(|| {
                    "derived_from_digest: an adopt-edition record carries its parent's digest"
                        .to_string()
                })?;
                IntakeVerdict::AdoptEdition(ParentReference {
                    repository: ParentRepository::new(&parent.repository)
                        .map_err(|e| format!("derived_from.repository: {e}"))?,
                    path: WorkspaceRelativePath::new(parent.path)
                        .map_err(|e| format!("derived_from.path: {e}"))?,
                    revision: CommitRevision::new(parent.revision)
                        .map_err(|e| format!("derived_from.revision: {e}"))?,
                    digest: ContentDigest::from_hex(digest)
                        .map_err(|e| format!("derived_from_digest: {e}"))?,
                })
            }
            "keep-local" => IntakeVerdict::KeepLocal,
            "merge-into" => IntakeVerdict::MergeInto,
            "rename" => IntakeVerdict::Rename,
            "retire" => IntakeVerdict::Retire,
            "deferred" => IntakeVerdict::Deferred,
            other => return Err(format!("verdict: \"{other}\" is not an intake verdict")),
        };
        Ok(IntakeRecord::new(IntakeFields {
            id: id.clone(),
            repository: RepositoryId::new(scope.id()).map_err(|e| format!("scope.id: {e}"))?,
            artifact: text("artifact", self.artifact)?,
            region: self.region.map(|r| text("region", r)).transpose()?,
            topic: IntakeTopic::new(&self.topic).map_err(|e| format!("topic: {e}"))?,
            delivery: Delivery::parse(&self.delivery)
                .ok_or_else(|| format!("delivery: \"{}\" is not a delivery", self.delivery))?,
            verdict,
            recorded_at: text("recorded_at", self.recorded_at)?,
        }))
    }
}

// -------------------------------------------------------------- observation

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ObservationDto {
    ts: String,
    #[serde(default)]
    kernel_version: Option<String>,
    #[serde(default)]
    kernel_revision: Option<String>,
    #[serde(default)]
    instance_revision: Option<String>,
    exit_code: i32,
    #[serde(default)]
    failing: Option<u64>,
    #[serde(default)]
    warnings: Option<u64>,
    #[serde(default)]
    info_ok: Option<u64>,
    fail_messages: Vec<String>,
    warn_messages: Vec<String>,
}

impl ObservationDto {
    pub(crate) fn convert(self) -> Converted<GateRunObservation> {
        let ts = Timestamp::parse_iso8601(&self.ts)
            .ok_or_else(|| format!("ts: \"{}\" is not an ISO 8601 instant", self.ts))?;
        GateRunObservation::new(
            ts,
            self.kernel_version,
            self.kernel_revision,
            self.instance_revision,
            self.exit_code,
            RunCounts {
                failing: self.failing,
                warnings: self.warnings,
                info_ok: self.info_ok,
            },
            self.fail_messages,
            self.warn_messages,
        )
        .map_err(|e| e.to_string())
    }
}

// ------------------------------------------------------------------ product

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProductDto {
    #[allow(dead_code)]
    #[serde(default)]
    schema_version: Option<IgnoredAny>,
    #[serde(default)]
    product: Option<ProductNameDto>,
    #[serde(default)]
    canonical_wiki: Option<CanonicalWikiDto>,
    #[allow(dead_code)]
    #[serde(default)]
    naming_conventions: Option<IgnoredAny>,
    #[serde(default)]
    tooling: Option<ToolingDto>,
    #[serde(default)]
    forbidden_literals: Option<Vec<String>>,
    #[serde(default)]
    forbidden_patterns: Option<Vec<String>>,
    #[allow(dead_code)]
    #[serde(default)]
    owner: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    last_verified: Option<IgnoredAny>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductNameDto {
    #[serde(default)]
    name: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    description: Option<IgnoredAny>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CanonicalWikiDto {
    #[allow(dead_code)]
    #[serde(default)]
    tool: Option<IgnoredAny>,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    space_key: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    documentation_standard_url: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    documentation_standard_version: Option<IgnoredAny>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToolingDto {
    #[allow(dead_code)]
    #[serde(default)]
    documentation: Option<IgnoredAny>,
    #[serde(default)]
    task_tracker: Option<TaskTrackerDto>,
    #[serde(default)]
    repository: Option<RepositoryHostDto>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskTrackerDto {
    #[allow(dead_code)]
    #[serde(default)]
    tool: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    hosting: Option<IgnoredAny>,
    #[serde(default)]
    base_url: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    access_method: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    constraint: Option<IgnoredAny>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepositoryHostDto {
    #[allow(dead_code)]
    #[serde(default)]
    tool: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    hosting: Option<IgnoredAny>,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    namespace: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    instance_repository: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    default_branch: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    git_transport: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    api_access: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    constraint: Option<IgnoredAny>,
}

impl ProductDto {
    /// The reference's derivation of the literal set: the product name, the
    /// wiki space and URL, the task-tracker URL, the repository host URL and
    /// namespace, then `forbidden_literals`; empty values are skipped.
    pub(crate) fn convert(self) -> Converted<ProductPurity> {
        let tooling = self.tooling;
        let (tracker, host) = match tooling {
            Some(t) => (t.task_tracker, t.repository),
            None => (None, None),
        };
        let (host_url, namespace) = match host {
            Some(h) => (h.base_url, h.namespace),
            None => (None, None),
        };
        let (wiki_url, space_key) = match self.canonical_wiki {
            Some(w) => (w.base_url, w.space_key),
            None => (None, None),
        };
        let derived = [
            self.product.and_then(|p| p.name),
            space_key,
            wiki_url,
            tracker.and_then(|t| t.base_url),
            host_url,
            namespace,
        ];
        let literals = derived
            .into_iter()
            .flatten()
            .chain(self.forbidden_literals.unwrap_or_default())
            .filter(|l| !l.trim().is_empty())
            .map(|l| ForbiddenLiteral::new(l).map_err(|e| e.to_string()))
            .collect::<Converted<Vec<_>>>()?;
        let patterns = self
            .forbidden_patterns
            .unwrap_or_default()
            .into_iter()
            .map(|p| ForbiddenPattern::new(p).map_err(|e| e.to_string()))
            .collect::<Converted<Vec<_>>>()?;
        Ok(ProductPurity::new(literals, patterns))
    }
}

// ----------------------------------------------------- external dependencies

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExternalDependenciesDto {
    #[allow(dead_code)]
    #[serde(default)]
    schema_version: Option<IgnoredAny>,
    dependencies: Vec<DependencyDto>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DependencyDto {
    id: String,
    #[allow(dead_code)]
    #[serde(default)]
    description: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    used_by: Option<IgnoredAny>,
    state: String,
    #[allow(dead_code)]
    #[serde(default)]
    vendored_at: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    vendored_to: Option<IgnoredAny>,
    /// A dated local-path note of the reference data, kept only so the
    /// shape stays closed.
    #[allow(dead_code)]
    #[serde(default, rename = "local_path_2026-08-18")]
    local_path_note: Option<IgnoredAny>,
    #[serde(default)]
    expected_sha256: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    verified_sha256: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    kernel_artifact_sha256: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    kernel_artifact_note: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    sha_provenance: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    note: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    on_missing: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    archive_sha256: Option<IgnoredAny>,
    #[allow(dead_code)]
    #[serde(default)]
    entry_path: Option<IgnoredAny>,
    #[serde(default)]
    entry_sha256: Option<String>,
}

impl ExternalDependenciesDto {
    pub(crate) fn convert(self) -> Converted<Vec<ExternalDependency>> {
        self.dependencies
            .into_iter()
            .enumerate()
            .map(|(i, d)| {
                let pinned = d.expected_sha256.is_some_and(|s| !s.is_empty())
                    || d.entry_sha256.is_some_and(|s| !s.is_empty());
                let state = DependencyState::parse(d.state)
                    .map_err(|e| format!("dependencies[{i}].{e}"))?;
                ExternalDependency::new(d.id, state, pinned)
                    .map_err(|e| format!("dependencies[{i}].{e}"))
            })
            .collect()
    }
}
