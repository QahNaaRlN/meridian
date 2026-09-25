#![cfg(test)]
//! The workspace-state operation over fake ports: the real Kernel's
//! registries, skills and stack-profile pool (read from this checkout),
//! in-memory stored records, in-memory product repositories and a fixed
//! clock. Each category-3 row of the GAP-09 mapping has a positive case in
//! [`workspace_state_a_consistent_workspace_is_clean`] and its own negative
//! case below.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{json, Value};

use meridian_core::types::{
    Authority, AuthorityKind, ContentDigest, DiagnosticLevel, EntryName, NonEmptyString, Origin,
    Scope, SemanticId, WorkspaceId, WorkspaceRelativePath,
};
use meridian_core::workspace_state::inventory::ObservedVcs;
use meridian_core::workspace_state::observation::GateRunObservation;
use meridian_core::workspace_state::parentage::CommitRevision;
use meridian_core::workspace_state::product::PurityFinding;
use meridian_core::workspace_state::timestamp::Timestamp;

use super::*;
use crate::storage::{
    ManagedRecord, Payload, PortError, PutRecordOutcome, PutRecordRequest, RecordKey,
    RecordRevision, RecordSchemaVersion, RevisionNumber, SchemaRef,
};
use crate::workspace::{DirEntry, EntryKind, ReadError, WorkspaceReader};

const REV: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
/// The Kernel revision an edition was taken from, and the Kernel's head.
const TAKEN: &str = "cccccccccccccccccccccccccccccccccccccccc";
const KERNEL_HEAD: &str = "dddddddddddddddddddddddddddddddddddddddd";
const PARENT: &str = "editions/layers.md";
const PARENT_TEXT: &str = "Layers depend inwards.\n";
const SKILL: &str = "---\nname: review\ndescription: reviews a change\n---\nReview it.\n";

// ------------------------------------------------------------------ readers

/// This Kernel checkout, with extra in-memory files laid over it.
struct KernelReader {
    root: PathBuf,
    overlay: BTreeMap<String, String>,
}

impl KernelReader {
    fn new(overlay: &[(&str, &str)]) -> Self {
        Self {
            root: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".."),
            overlay: overlay
                .iter()
                .map(|(p, t)| (p.to_string(), t.to_string()))
                .collect(),
        }
    }
}

impl WorkspaceReader for KernelReader {
    fn read_text(&self, path: &WorkspaceRelativePath) -> Result<String, ReadError> {
        if let Some(text) = self.overlay.get(path.as_str()) {
            return Ok(text.clone());
        }
        std::fs::read_to_string(self.root.join(path.as_str())).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => ReadError::NotFound,
            _ => ReadError::Io(e.to_string()),
        })
    }
    fn read_bytes(&self, path: &WorkspaceRelativePath) -> Result<Vec<u8>, ReadError> {
        self.read_text(path).map(String::into_bytes)
    }
    fn list_dir(&self, path: &WorkspaceRelativePath) -> Result<Vec<DirEntry>, ReadError> {
        let entries = std::fs::read_dir(self.root.join(path.as_str()))
            .map_err(|e| ReadError::Io(e.to_string()))?;
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let kind = match entry.file_type() {
                Ok(t) if t.is_dir() => EntryKind::Dir,
                Ok(t) if t.is_file() => EntryKind::File,
                _ => EntryKind::Other,
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            out.push(DirEntry {
                name: EntryName::new(name).map_err(|e| ReadError::Io(e.to_string()))?,
                kind,
            });
        }
        Ok(out)
    }
}

/// One in-memory product repository.
struct MapReader(BTreeMap<String, String>);

impl WorkspaceReader for MapReader {
    fn read_text(&self, path: &WorkspaceRelativePath) -> Result<String, ReadError> {
        self.0
            .get(path.as_str())
            .cloned()
            .ok_or(ReadError::NotFound)
    }
    fn read_bytes(&self, path: &WorkspaceRelativePath) -> Result<Vec<u8>, ReadError> {
        self.read_text(path).map(String::into_bytes)
    }
    fn list_dir(&self, _: &WorkspaceRelativePath) -> Result<Vec<DirEntry>, ReadError> {
        Err(ReadError::NotFound)
    }
}

// ------------------------------------------------------------------- ports

/// A port of [`RepositoryAccess`] a test makes fail.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Port {
    Untracked,
    Ignored,
    Revision,
    FileAt,
}

#[derive(Clone, Default)]
struct Repo {
    vcs: Option<ObservedVcs>,
    tracked: Option<Vec<String>>,
    untracked: Vec<String>,
    ignored: Vec<String>,
    files: BTreeMap<String, String>,
    /// Committed files by revision; `vcs.revision` is the head.
    commits: BTreeMap<String, BTreeMap<String, String>>,
    failing: Vec<Port>,
}

#[derive(Default)]
struct Repos(BTreeMap<String, Repo>);

fn unavailable(root: &str) -> RepositoryUnavailable {
    RepositoryUnavailable(format!("{root} is not a repository"))
}

impl Repos {
    fn failing(&self, root: &str, port: Port) -> Result<&Repo, RepositoryUnavailable> {
        let repo = self.0.get(root).ok_or_else(|| unavailable(root))?;
        if repo.failing.contains(&port) {
            return Err(RepositoryUnavailable(format!("{root}: the port failed")));
        }
        Ok(repo)
    }
}

impl RepositoryAccess for Repos {
    fn vcs_state(&self, root: &str) -> Result<ObservedVcs, RepositoryUnavailable> {
        self.0
            .get(root)
            .and_then(|r| r.vcs.clone())
            .ok_or_else(|| unavailable(root))
    }
    fn tracked_files(&self, root: &str) -> Result<Vec<String>, RepositoryUnavailable> {
        self.0
            .get(root)
            .and_then(|r| r.tracked.clone())
            .ok_or_else(|| unavailable(root))
    }
    fn untracked_files(&self, root: &str) -> Result<Vec<String>, RepositoryUnavailable> {
        self.failing(root, Port::Untracked)
            .map(|r| r.untracked.clone())
    }
    fn ignored_among(
        &self,
        root: &str,
        candidates: &[String],
    ) -> Result<Vec<String>, RepositoryUnavailable> {
        self.failing(root, Port::Ignored).map(|r| {
            r.ignored
                .iter()
                .filter(|i| candidates.contains(i))
                .cloned()
                .collect()
        })
    }
    fn has_revision(
        &self,
        root: &str,
        revision: &CommitRevision,
    ) -> Result<bool, RepositoryUnavailable> {
        self.failing(root, Port::Revision)
            .map(|r| r.commits.contains_key(revision.as_str()))
    }
    fn file_at(
        &self,
        root: &str,
        at: RevisionSelector<'_>,
        path: &WorkspaceRelativePath,
    ) -> Result<Option<String>, RepositoryUnavailable> {
        let repo = self.failing(root, Port::FileAt)?;
        let revision = match at {
            RevisionSelector::Head => repo.vcs.as_ref().map(|v| v.revision.clone()),
            RevisionSelector::Commit(commit) => Some(commit.as_str().to_string()),
        };
        Ok(revision
            .and_then(|r| repo.commits.get(&r))
            .and_then(|files| files.get(path.as_str()))
            .cloned())
    }
    fn reader(&self, root: &str) -> Box<dyn WorkspaceReader + '_> {
        Box::new(MapReader(
            self.0
                .get(root)
                .map(|r| r.files.clone())
                .unwrap_or_default(),
        ))
    }
}

struct FixedClock(Timestamp);

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        self.0
    }
}

/// Stored records; every write is counted and refused.
struct Records {
    records: Vec<ManagedRecord>,
    writes: Cell<usize>,
}

impl RecordRepository for Records {
    fn put(&self, _: PutRecordRequest) -> Result<PutRecordOutcome, PortError> {
        self.writes.set(self.writes.get() + 1);
        Err(PortError::Storage("read-only fake".to_string()))
    }
    fn put_batch(&self, _: Vec<PutRecordRequest>) -> Result<Vec<PutRecordOutcome>, PortError> {
        self.writes.set(self.writes.get() + 1);
        Err(PortError::Storage("read-only fake".to_string()))
    }
    fn get(&self, _: &RecordKey) -> Result<Option<ManagedRecord>, PortError> {
        Ok(None)
    }
    fn get_revision(
        &self,
        _: &RecordKey,
        _: RevisionNumber,
    ) -> Result<Option<RecordRevision>, PortError> {
        Ok(None)
    }
    fn list_revisions(&self, _: &RecordKey) -> Result<Vec<RecordRevision>, PortError> {
        Ok(Vec::new())
    }
    fn export_all(&self) -> Result<Vec<ManagedRecord>, PortError> {
        Ok(self.records.clone())
    }
}

// ------------------------------------------------------------------ records

fn workspace() -> Scope {
    Scope::project_workspace(SemanticId::new("sample").unwrap(), None)
}

fn repository_scope(id: &str) -> Scope {
    Scope::repository_scope(
        SemanticId::new(id).unwrap(),
        WorkspaceId::new("sample").unwrap(),
    )
}

fn stored(
    record_type: &str,
    id: &str,
    scope: Scope,
    title: &str,
    media: &str,
    content: &str,
) -> ManagedRecord {
    let payload = json!({
        "media_type": media,
        "encoding": "utf-8",
        "content": content,
        "digest": {"algorithm": "sha-256", "value": ContentDigest::of_str(content).value()},
    });
    ManagedRecord::from_parts(
        RecordKey::new(scope, SemanticId::new(id).unwrap()),
        SchemaRef::new("registries/operating-model/scoped-record.schema.json").unwrap(),
        RecordSchemaVersion::CURRENT,
        NonEmptyString::new(title).unwrap(),
        SemanticId::new(record_type).unwrap(),
        Origin::migrated("record-unit:sample").unwrap(),
        Authority::new(AuthorityKind::ProjectOwner, "sample-owner", None).unwrap(),
        Payload::new(payload).unwrap(),
        RevisionNumber::FIRST,
        ContentDigest::of_str(id),
    )
}

fn identity_content(overrides: &[(&str, Value)]) -> String {
    let mut identity = json!({
        "id": "web",
        "path": "/work/web",
        "role": "web client",
        "profile": "vue-spa",
        "ownership": "own",
        "sources": {"rules": ["/work/web/AGENTS.md"], "manifests": ["/work/web/package.json"]},
        "vcs": {"type": "git", "revision": REV, "ref": "dev", "working_tree": "clean", "evidence_basis": "revision"},
        "last_verified": "2026-09-01T00:00:00Z",
    });
    for (key, value) in overrides {
        identity[*key] = value.clone();
    }
    identity.to_string()
}

fn intake_content(artifact: &str, region: Option<&str>) -> String {
    intake_with(artifact, region, &[])
}

fn intake_with(artifact: &str, region: Option<&str>, overrides: &[(&str, Value)]) -> String {
    let mut record = json!({
        "artifact": artifact,
        "delivery": if region.is_some() { "agents-md-section" } else { "cursor-rule" },
        "digest": "b".repeat(64),
        "genre": "standard",
        "observed_activation": "always",
        "recorded_at": "2026-08-21",
        "scope": "repository",
        "topic": "unclassified",
        "unclassified_reason": "several subjects",
        "verdict": "deferred",
        "verdict_basis": "recorded as observed",
        "resume_condition": "owner decides",
    });
    if let Some(region) = region {
        record["region"] = json!(region);
    }
    for (key, value) in overrides {
        if value.is_null() {
            record.as_object_mut().unwrap().remove(*key);
        } else {
            record[*key] = value.clone();
        }
    }
    record.to_string()
}

/// A skill package, named from the pool's topics.
fn skill_intake(artifact: &str) -> String {
    intake_with(
        artifact,
        None,
        &[
            ("delivery", json!("skill-package")),
            ("topic", json!("code-review")),
            ("unclassified_reason", Value::Null),
            ("verdict", json!("keep-local")),
            ("resume_condition", Value::Null),
        ],
    )
}

/// An edition of a Kernel text.
fn edition_intake(artifact: &str, parent: Value, digest: &str) -> String {
    intake_with(
        artifact,
        None,
        &[
            ("topic", json!("layer-architecture")),
            ("unclassified_reason", Value::Null),
            ("verdict", json!("adopt-edition")),
            ("resume_condition", Value::Null),
            ("derived_from", parent),
            ("derived_from_digest", json!(digest)),
            ("narrowing", json!(["only the web layer"])),
        ],
    )
}

fn kernel_parent() -> Value {
    json!({"repository": "kernel", "path": PARENT, "revision": TAKEN})
}

const PRODUCT: &str = "product:\n  name: Acme\nforbidden_patterns:\n  - '\\bAcmeHub\\b'\n";
const DEPENDENCIES: &str =
    "schema_version: 1\ndependencies:\n  - id: vendored-skill\n    state: vendored\n";
const AGENTS: &str = "<!-- meridian:begin instruction-section id=\"style\" owner=\"team\" -->\nUse tabs.\n<!-- meridian:end instruction-section id=\"style\" -->\n";

/// A workspace every check accepts.
fn consistent_records() -> Vec<ManagedRecord> {
    vec![
        stored(
            "workspace-file",
            "product",
            workspace(),
            "product.yaml",
            "text/yaml",
            PRODUCT,
        ),
        stored(
            "workspace-file",
            "external-dependencies",
            workspace(),
            "external-dependencies.yaml",
            "text/yaml",
            DEPENDENCIES,
        ),
        stored(
            "workspace-file",
            "skills-bugfix-protocol-context",
            workspace(),
            "skills/bugfix-protocol/context.md",
            "text/markdown",
            "# context\n",
        ),
        stored(
            "repository-identity",
            "inventory-web",
            workspace(),
            "Repository web",
            "application/json",
            &identity_content(&[]),
        ),
        stored(
            "instruction-intake-record",
            "intake-web-agents",
            repository_scope("web"),
            "Intake web AGENTS.md#style",
            "application/json",
            &intake_content("AGENTS.md", Some("style")),
        ),
        stored(
            "instruction-intake-record",
            "intake-web-rule",
            repository_scope("web"),
            "Intake web a.mdc",
            "application/json",
            &intake_content(".cursor/rules/a.mdc", None),
        ),
        stored(
            "instruction-intake-record",
            "intake-web-skill",
            repository_scope("web"),
            "Intake web review skill",
            "application/json",
            &skill_intake("skills/review/SKILL.md"),
        ),
        stored(
            "instruction-intake-record",
            "intake-web-edition",
            repository_scope("web"),
            "Intake web layers edition",
            "application/json",
            &edition_intake(
                ".cursor/rules/layers.mdc",
                kernel_parent(),
                ContentDigest::of_str(PARENT_TEXT).value(),
            ),
        ),
        stored(
            "report",
            "report-one",
            workspace(),
            "Report one",
            "text/markdown",
            "# r\n",
        ),
    ]
}

fn web_repo() -> Repo {
    Repo {
        vcs: Some(ObservedVcs {
            revision: REV.to_string(),
            git_ref: "dev".to_string(),
            dirty: false,
        }),
        tracked: Some(vec![
            ".cursor/rules/a.mdc".to_string(),
            ".cursor/rules/layers.mdc".to_string(),
            "AGENTS.md".to_string(),
            "package.json".to_string(),
            "skills/review/SKILL.md".to_string(),
        ]),
        files: [
            ("AGENTS.md".to_string(), AGENTS.to_string()),
            ("skills/review/SKILL.md".to_string(), SKILL.to_string()),
            (
                "package.json".to_string(),
                r#"{"dependencies":{"vue":"3"},"devDependencies":{"vite":"5"}}"#.to_string(),
            ),
        ]
        .into(),
        ..Repo::default()
    }
}

/// The Kernel as a repository: the edition's parent at the revision it was
/// taken from and, unchanged, at the head.
fn kernel_repo() -> Repo {
    let parent: BTreeMap<String, String> = [(PARENT.to_string(), PARENT_TEXT.to_string())].into();
    Repo {
        vcs: Some(ObservedVcs {
            revision: KERNEL_HEAD.to_string(),
            git_ref: "dev".to_string(),
            dirty: false,
        }),
        commits: [
            (TAKEN.to_string(), parent.clone()),
            (KERNEL_HEAD.to_string(), parent),
        ]
        .into(),
        ..Repo::default()
    }
}

struct World {
    records: Records,
    repos: Repos,
    kernel: KernelReader,
    files: Vec<WorkspaceRelativePath>,
}

impl World {
    fn new(records: Vec<ManagedRecord>) -> Self {
        World {
            records: Records {
                records,
                writes: Cell::new(0),
            },
            repos: Repos(
                [
                    ("/work/web".to_string(), web_repo()),
                    ("/kernel".to_string(), kernel_repo()),
                ]
                .into(),
            ),
            kernel: KernelReader::new(&[("probe/clean.md", "nothing product-specific\n")]),
            files: vec![WorkspaceRelativePath::new("probe/clean.md").unwrap()],
        }
    }

    fn run(&self) -> WorkspaceStateReport {
        let payloads = PayloadRegistry::load(&self.kernel).unwrap();
        let topics = crate::validation::mechanical_integrity::instruction_topics::run(&self.kernel)
            .unwrap()
            .topic_pool
            .expect("the Kernel's topic pool agrees");
        let report = validate(
            &KernelSide {
                reader: &self.kernel,
                files: &self.files,
                stack_profile_pool_loaded: true,
                topic_pool: Some(&topics),
                repository_root: "/kernel",
            },
            &Ports {
                records: &self.records,
                repositories: &self.repos,
                clock: &FixedClock(Timestamp::parse_iso8601("2026-09-02T00:00:00Z").unwrap()),
            },
            &payloads,
        )
        .unwrap();
        assert_eq!(self.records.writes.get(), 0, "validate must never write");
        report
    }
}

fn at(report: &WorkspaceStateReport, level: DiagnosticLevel) -> Vec<String> {
    report
        .diagnostics
        .iter()
        .filter(|d| d.level() == level)
        .map(|d| d.message().to_string())
        .collect()
}

fn fails(report: &WorkspaceStateReport) -> Vec<String> {
    at(report, DiagnosticLevel::Fail)
}

fn replace(records: &mut [ManagedRecord], id: &str, with: ManagedRecord) {
    let slot = records
        .iter_mut()
        .find(|r| r.key().id().as_str() == id)
        .unwrap();
    *slot = with;
}

// ------------------------------------------------------------------- tests

#[test]
fn workspace_state_a_consistent_workspace_is_clean() {
    let report = World::new(consistent_records()).run();
    assert!(fails(&report).is_empty(), "{:?}", fails(&report));
    assert!(
        at(&report, DiagnosticLevel::Warn).is_empty(),
        "{:?}",
        at(&report, DiagnosticLevel::Warn)
    );
    assert!(report.purity.is_empty());
    assert_eq!(report.counts.records, 9);
    assert_eq!(report.counts.rejected_payloads, 0);
    assert_eq!(report.counts.repositories_confirmed, 1);
    assert_eq!(report.counts.intake_repositories_complete, 1);
    assert_eq!(report.workspace, Ok(workspace()));
    let infos = at(&report, DiagnosticLevel::Info);
    assert!(
        infos.contains(
            &"stack-profile: 1/1 declarations confirmed against the repository's own manifest"
                .to_string()
        ),
        "{infos:?}"
    );
    assert!(infos.contains(
        &"inventory-git: 1/1 entries confirmed against actual repository state".to_string()
    ));
}

#[test]
fn workspace_state_m02_product_literal_and_pattern_are_found_in_the_kernel() {
    let mut world = World::new(consistent_records());
    world.kernel = KernelReader::new(&[("probe/leak.md", "built for ACME via AcmeHub\n")]);
    world.files = vec![WorkspaceRelativePath::new("probe/leak.md").unwrap()];
    let report = world.run();
    let findings: Vec<String> = report
        .purity
        .iter()
        .map(|f| f.finding.message(&format!("/k/{}", f.path.as_str())))
        .collect();
    assert_eq!(
        findings,
        [
            "kernel-purity: product literal \"Acme\" found in /k/probe/leak.md",
            "kernel-purity: forbidden pattern /\\bAcmeHub\\b/ matched \"AcmeHub\" in /k/probe/leak.md",
        ]
    );
    assert!(matches!(
        report.purity[0].finding,
        PurityFinding::Literal { .. }
    ));
}

#[test]
fn workspace_state_m03_missing_or_uncompilable_product_record_fails() {
    let mut records = consistent_records();
    records.retain(|r| r.key().id().as_str() != "product");
    let report = World::new(records).run();
    assert!(fails(&report).contains(&"product record not found in the workspace database (workspace file \"product.yaml\"); kernel-purity cannot be verified".to_string()), "{:?}", fails(&report));

    let mut records = consistent_records();
    replace(
        &mut records,
        "product",
        stored(
            "workspace-file",
            "product",
            workspace(),
            "product.yaml",
            "text/yaml",
            "forbidden_patterns:\n  - '(unclosed'\n",
        ),
    );
    let report = World::new(records).run();
    let fails = fails(&report);
    assert_eq!(fails.len(), 1, "{fails:?}");
    assert!(
        fails[0].starts_with("product record: forbidden pattern /(unclosed/ does not compile: "),
        "{fails:?}"
    );
}

#[test]
fn workspace_state_m05b_a_stored_payload_no_contract_accepts_fails() {
    let mut records = consistent_records();
    // One mandatory field of a schema-backed family removed.
    let broken: Value = {
        let mut v: Value = serde_json::from_str(&identity_content(&[])).unwrap();
        v.as_object_mut().unwrap().remove("role");
        v
    };
    replace(
        &mut records,
        "inventory-web",
        stored(
            "repository-identity",
            "inventory-web",
            workspace(),
            "Repository web",
            "application/json",
            &broken.to_string(),
        ),
    );
    records.push(stored(
        "note",
        "note-one",
        workspace(),
        "Note",
        "text/markdown",
        "x",
    ));
    let report = World::new(records).run();
    let fails = fails(&report);
    assert!(fails.iter().any(|f| f.starts_with("payload-contract: repository-identity record \"inventory-web\" in project-workspace \"sample\": content ") && f.contains("role")), "{fails:?}");
    assert!(fails.contains(&"payload-contract: note record \"note-one\" in project-workspace \"sample\": record_type \"note\" has no payload contract; a product record type is added to the registry by decision, never accepted unchecked".to_string()), "{fails:?}");
    assert_eq!(report.counts.rejected_payloads, 2);
}

#[test]
fn workspace_state_m06_missing_instance_context_fails() {
    let mut records = consistent_records();
    records.retain(|r| r.key().id().as_str() != "skills-bugfix-protocol-context");
    let report = World::new(records).run();
    assert_eq!(
        fails(&report),
        ["instance-context: bugfix-protocol requires \"skills/bugfix-protocol/context.md\", which is missing from the workspace database; the skill declares this a blocker, not a licence to guess"]
    );
}

#[test]
fn workspace_state_m07_malformed_or_unpinned_dependencies() {
    let mut records = consistent_records();
    replace(
        &mut records,
        "external-dependencies",
        stored(
            "workspace-file",
            "external-dependencies",
            workspace(),
            "external-dependencies.yaml",
            "text/yaml",
            "dependencies:\n  - id: x\n",
        ),
    );
    let report = World::new(records).run();
    let fails = fails(&report);
    assert!(
        fails.len() == 1
            && fails[0].starts_with(
                "ext-dependencies: content is not the closed shape: missing field `state`"
            ),
        "{fails:?}"
    );

    let mut records = consistent_records();
    replace(
        &mut records,
        "external-dependencies",
        stored(
            "workspace-file",
            "external-dependencies",
            workspace(),
            "external-dependencies.yaml",
            "text/yaml",
            "dependencies:\n  - id: mirror\n    state: external\n",
        ),
    );
    let report = World::new(records).run();
    assert_eq!(
        at(&report, DiagnosticLevel::Warn),
        ["ext-dependency: mirror is \"external\" with no pinned SHA — unverifiable on another machine"]
    );
}

#[test]
fn workspace_state_m08_drift_fails_and_an_unreachable_repository_is_unverified() {
    let mut world = World::new(consistent_records());
    world.repos.0.get_mut("/work/web").unwrap().vcs = Some(ObservedVcs {
        revision: REV.to_string(),
        git_ref: "main".to_string(),
        dirty: false,
    });
    let report = world.run();
    assert!(fails(&report).contains(&"inventory-git: web — ref recorded \"dev\" but HEAD is on \"main\"; revalidate the entry, do not merely re-date it".to_string()));

    let mut world = World::new(consistent_records());
    world.repos.0.clear();
    let report = world.run();
    assert!(at(&report, DiagnosticLevel::Warn)
        .contains(&"inventory-git: 0/1 entries could be confirmed".to_string()));
    assert!(at(&report, DiagnosticLevel::Info)
        .iter()
        .any(|m| m.contains("recorded revision is UNVERIFIED, not confirmed")));
    assert!(at(&report, DiagnosticLevel::Info)
        .iter()
        .any(|m| m.contains("completeness is UNVERIFIED, not confirmed")));
    assert!(fails(&report).is_empty(), "{:?}", fails(&report));
}

#[test]
fn workspace_state_m09_undeclared_pool_profile_or_unreadable_manifest() {
    let mut records = consistent_records();
    replace(
        &mut records,
        "inventory-web",
        stored(
            "repository-identity",
            "inventory-web",
            workspace(),
            "Repository web",
            "application/json",
            &identity_content(&[("profile", json!("vue-monorepo"))]),
        ),
    );
    let report = World::new(records).run();
    assert!(fails(&report).contains(&"stack-profile: web declares \"vue-monorepo\", which is not in the pool; a profile is named from the pool or added to it by decision, never invented at the point of use".to_string()), "{:?}", fails(&report));

    let mut world = World::new(consistent_records());
    world
        .repos
        .0
        .get_mut("/work/web")
        .unwrap()
        .files
        .remove("package.json");
    let report = world.run();
    assert!(at(&report, DiagnosticLevel::Warn).contains(&"stack-profile: 1/1 declarations could not be confirmed; the manifest was not reachable from here".to_string()));
}

#[test]
fn workspace_state_m12_intake_of_an_unknown_repository_fails() {
    let mut records = consistent_records();
    records.push(stored(
        "instruction-intake-record",
        "intake-ghost",
        repository_scope("ghost"),
        "Intake ghost",
        "application/json",
        &intake_content("AGENTS.md", None),
    ));
    let report = World::new(records).run();
    assert_eq!(
        fails(&report),
        ["instruction-intake: 1 record(s) of repository \"ghost\" name a repository the workspace inventory does not name; this is an unresolvable reference, not an unreachable repository"]
    );
}

#[test]
fn workspace_state_m15_an_uncovered_norm_fails_completeness() {
    let mut world = World::new(consistent_records());
    world
        .repos
        .0
        .get_mut("/work/web")
        .unwrap()
        .tracked
        .as_mut()
        .unwrap()
        .push("skills/new/SKILL.md".to_string());
    let report = world.run();
    assert_eq!(
        fails(&report),
        ["instruction-intake: web — 1 norm(s) in the tree have no record (skills/new/SKILL.md); the register is complete or it proves nothing"]
    );
    assert_eq!(report.counts.intake_repositories_complete, 0);
}

#[test]
fn workspace_state_m18_observation_request_is_checked_and_idempotent() {
    let kernel = KernelReader::new(&[]);
    let payloads = PayloadRegistry::load(&kernel).unwrap();
    let observation = GateRunObservation::of_run(
        Timestamp::parse_iso8601("2026-09-02T00:00:00Z").unwrap(),
        "0.5.0".to_string(),
        Some(REV.to_string()),
        1,
        &["b".to_string(), "a".to_string()],
        &["w".to_string()],
        4,
    )
    .unwrap();
    let first = observation_request(&observation, &workspace(), &payloads).unwrap();
    let again = observation_request(&observation, &workspace(), &payloads).unwrap();
    assert_eq!(first, again, "the same logical run is the same write");
    assert_eq!(first.record_type().as_str(), "gate-run-observation");
    assert!(first
        .key()
        .id()
        .as_str()
        .starts_with("gate-run-observation-"));
    // The stored form reads back through the same payload validator.
    let subject = PayloadSubject {
        id: first.key().id(),
        scope: first.key().scope(),
        title: first.title(),
        record_type: first.record_type().as_str(),
    };
    let Ok(TypedPayload::Observation(read_back)) =
        payloads.check(subject, first.payload().as_map())
    else {
        panic!("the observation must satisfy its own contract");
    };
    assert_eq!(read_back, observation);
    assert_eq!(read_back.fail_messages(), ["a", "b"]);
}

#[test]
fn workspace_state_workspace_scope_must_be_unique() {
    let mut records = consistent_records();
    records.push(stored(
        "report",
        "other-report",
        Scope::project_workspace(SemanticId::new("other").unwrap(), None),
        "Other",
        "text/markdown",
        "x",
    ));
    let report = World::new(records).run();
    assert_eq!(
        report.workspace,
        Err("the workspace database holds records of several project workspaces".to_string())
    );
}

// ------------------------------------------ fail-closed tree ports (M-15)

fn web(world: &mut World) -> &mut Repo {
    world.repos.0.get_mut("/work/web").unwrap()
}

#[test]
fn workspace_state_m15_a_failing_untracked_port_is_unverified_never_complete() {
    let mut world = World::new(consistent_records());
    web(&mut world).failing.push(Port::Untracked);
    let report = world.run();
    assert_eq!(
        at(&report, DiagnosticLevel::Warn),
        ["instruction-intake: web — the untracked files of the tree cannot be listed (/work/web: the port failed); norms outside version control are UNVERIFIED, and this tree is NOT counted as confirmed complete"]
    );
    assert!(fails(&report).is_empty(), "{:?}", fails(&report));
    assert_eq!(report.counts.intake_repositories_complete, 0);
}

/// A recorded artifact outside the tracked files is the one the ignored
/// port is asked about.
fn with_untracked_record() -> Vec<ManagedRecord> {
    let mut records = consistent_records();
    records.push(stored(
        "instruction-intake-record",
        "intake-web-local",
        repository_scope("web"),
        "Intake web local.mdc",
        "application/json",
        &intake_content("local.mdc", None),
    ));
    records
}

#[test]
fn workspace_state_m15_a_failing_ignored_port_is_unverified_never_complete() {
    // Control: the same tree with a working port is confirmed complete.
    let report = World::new(with_untracked_record()).run();
    assert_eq!(report.counts.intake_repositories_complete, 1);

    let mut world = World::new(with_untracked_record());
    web(&mut world).failing.push(Port::Ignored);
    let report = world.run();
    assert_eq!(
        at(&report, DiagnosticLevel::Warn),
        ["instruction-intake: web — whether the recorded norms outside version control are ignored cannot be established (/work/web: the port failed); such a norm is UNVERIFIED, and this tree is NOT counted as confirmed complete"]
    );
    assert!(fails(&report).is_empty(), "{:?}", fails(&report));
    assert_eq!(report.counts.intake_repositories_complete, 0);
}

#[test]
fn workspace_state_m15_an_unreadable_container_is_never_counted_complete() {
    let mut world = World::new(consistent_records());
    web(&mut world).files.remove("AGENTS.md");
    let report = world.run();
    assert!(at(&report, DiagnosticLevel::Info).contains(
        &"instruction-intake: web/AGENTS.md could not be read; its regions are UNVERIFIED, not confirmed".to_string()
    ));
    assert_eq!(report.counts.intake_repositories_complete, 0);
}

// ------------------------------------------------ D3: topic existence

#[test]
fn workspace_state_d3_a_topic_outside_the_kernel_pool_fails() {
    let mut records = consistent_records();
    replace(
        &mut records,
        "intake-web-rule",
        stored(
            "instruction-intake-record",
            "intake-web-rule",
            repository_scope("web"),
            "Intake web a.mdc",
            "application/json",
            &intake_with(
                ".cursor/rules/a.mdc",
                None,
                &[
                    ("topic", json!("invented-topic")),
                    ("unclassified_reason", Value::Null),
                ],
            ),
        ),
    );
    let report = World::new(records).run();
    assert_eq!(
        fails(&report),
        ["instruction-intake: the register of repository \"web\" names 1 topic(s) outside the pool (invented-topic); a new topic is a change to the registry, not to one record"]
    );
}

// ------------------------------------------------ D3: skill packaging

#[test]
fn workspace_state_d3_a_misnamed_or_doubly_activated_skill_package_fails() {
    let mut world = World::new(consistent_records());
    web(&mut world).files.insert(
        "skills/review/SKILL.md".to_string(),
        "---\nname: reviewer\nalwaysApply: true\n---\nReview it.\n".to_string(),
    );
    let report = world.run();
    assert_eq!(
        fails(&report),
        [
            "instruction-intake: skills/review/SKILL.md declares activation twice — alwaysApply belong to the rule vocabulary, not the package one; which one applies is then decided by the tool, not by the author",
            "instruction-intake: skills/review/SKILL.md is a skill named \"reviewer\" in a directory named \"review\"; a norm that cannot be found by its own name makes every reference to it dangling",
        ]
    );

    let mut world = World::new(consistent_records());
    web(&mut world).files.remove("skills/review/SKILL.md");
    let report = world.run();
    assert!(fails(&report).is_empty(), "{:?}", fails(&report));
    assert!(at(&report, DiagnosticLevel::Info).contains(
        &"instruction-intake: skills/review/SKILL.md not reachable; its packaging is UNVERIFIED, not confirmed".to_string()
    ));
}

// ------------------------------------------------ D3: edition parentage

fn with_edition(parent: Value, digest: &str) -> Vec<ManagedRecord> {
    let mut records = consistent_records();
    replace(
        &mut records,
        "intake-web-edition",
        stored(
            "instruction-intake-record",
            "intake-web-edition",
            repository_scope("web"),
            "Intake web layers edition",
            "application/json",
            &edition_intake(".cursor/rules/layers.mdc", parent, digest),
        ),
    );
    records
}

fn kernel(world: &mut World) -> &mut Repo {
    world.repos.0.get_mut("/kernel").unwrap()
}

#[test]
fn workspace_state_d3_an_unresolvable_or_different_parent_fails() {
    let right = ContentDigest::of_str(PARENT_TEXT);
    let wrong = ContentDigest::of_str("another text");
    let report = World::new(with_edition(kernel_parent(), wrong.value())).run();
    assert_eq!(
        fails(&report),
        [format!(
            "instruction-intake: .cursor/rules/layers.mdc records parent digest {} but \"editions/layers.md\" at ccccccc hashes to {}; the reference names a different text, which no re-dating can fix",
            &wrong.value()[..12],
            &right.value()[..12]
        )]
    );

    let missing = json!({"repository": "kernel", "path": "editions/gone.md", "revision": TAKEN});
    let report = World::new(with_edition(missing, right.value())).run();
    assert_eq!(
        fails(&report),
        ["instruction-intake: .cursor/rules/layers.mdc derives from \"editions/gone.md\" at ccccccc in kernel, and that revision holds no such file; the reference points at nothing"]
    );

    let ghost = json!({"repository": "ghost", "path": PARENT, "revision": TAKEN});
    let report = World::new(with_edition(ghost, right.value())).run();
    assert_eq!(
        fails(&report),
        ["instruction-intake: .cursor/rules/layers.mdc derives from repository \"ghost\", which the workspace inventory does not name; the reference cannot be resolved by anyone, here or elsewhere"]
    );

    // A bare path is not a reference: the payload contract refuses the
    // record, which then covers nothing.
    let bare = json!(PARENT);
    let report = World::new(with_edition(bare, right.value())).run();
    assert_eq!(
        fails(&report),
        [
            "instruction-intake: web — 1 norm(s) in the tree have no record (.cursor/rules/layers.mdc); the register is complete or it proves nothing",
            "payload-contract: instruction-intake-record record \"intake-web-edition\" in repository-scope \"web\": content /derived_from: expected object, got string",
        ]
    );
}

#[test]
fn workspace_state_d3_an_unavailable_parent_is_unverified_and_a_moved_one_warns() {
    let right = ContentDigest::of_str(PARENT_TEXT);
    // The repository is gone, the revision is absent, the file cannot be
    // read: UNVERIFIED, never a failure and never confirmed.
    for (label, edit) in [
        (
            "unreachable",
            (|repo: &mut Repo| repo.vcs = None) as fn(&mut Repo),
        ),
        ("revision", |repo: &mut Repo| {
            repo.commits.remove(TAKEN);
        }),
        ("revision port", |repo: &mut Repo| {
            repo.failing.push(Port::Revision)
        }),
        ("file port", |repo: &mut Repo| {
            repo.failing.push(Port::FileAt)
        }),
    ] {
        let mut world = World::new(with_edition(kernel_parent(), right.value()));
        edit(kernel(&mut world));
        let report = world.run();
        assert!(fails(&report).is_empty(), "{label}: {:?}", fails(&report));
        assert!(
            at(&report, DiagnosticLevel::Info).iter().any(|m| m
                .ends_with("the parent of .cursor/rules/layers.mdc is UNVERIFIED, not confirmed")),
            "{label}: {:?}",
            at(&report, DiagnosticLevel::Info)
        );
    }

    let mut world = World::new(with_edition(kernel_parent(), right.value()));
    kernel(&mut world)
        .commits
        .get_mut(KERNEL_HEAD)
        .unwrap()
        .insert(
            PARENT.to_string(),
            "Layers depend inwards, mostly.\n".to_string(),
        );
    let report = world.run();
    assert!(fails(&report).is_empty(), "{:?}", fails(&report));
    assert_eq!(
        at(&report, DiagnosticLevel::Warn),
        ["instruction-intake: .cursor/rules/layers.mdc — the edition is behind its parent: \"editions/layers.md\" has changed in kernel since ccccccc. This is not a defective record: re-take the edition against the current parent when the difference matters"]
    );
}

// ------------------------------------------- M-05b: the payload matrix

/// One family of `meridian-cli/tests/fixtures/payload-matrix/families.json`.
struct Family {
    record_type: String,
    contract: String,
    scope: Scope,
    title: String,
    media_type: String,
    valid: String,
    mutated: Option<String>,
}

fn content_of(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn families() -> Vec<Family> {
    let doc: Value = serde_json::from_str(include_str!(
        "../../../meridian-cli/tests/fixtures/payload-matrix/families.json"
    ))
    .unwrap();
    doc["families"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            let mutated = match &f["mutation"] {
                Value::Null => None,
                m => Some(match (m.get("remove"), m.get("text")) {
                    (Some(Value::String(field)), None) => {
                        let mut value = f["valid"].clone();
                        assert!(value.as_object_mut().unwrap().remove(field).is_some());
                        value.to_string()
                    }
                    (None, Some(Value::String(text))) => text.clone(),
                    _ => panic!("a mutation removes a field or replaces the text: {m}"),
                }),
            };
            Family {
                record_type: f["record_type"].as_str().unwrap().to_string(),
                contract: f["contract"].as_str().unwrap().to_string(),
                scope: match f["scope"].as_str().unwrap() {
                    "workspace" => workspace(),
                    "repository" => repository_scope("web"),
                    other => panic!("scope {other}"),
                },
                title: f["title"].as_str().unwrap().to_string(),
                media_type: f["media_type"].as_str().unwrap().to_string(),
                valid: content_of(&f["valid"]),
                mutated,
            }
        })
        .collect()
}

fn matrix_record(family: &Family, id: &str, content: &str) -> ManagedRecord {
    stored(
        &family.record_type,
        id,
        family.scope.clone(),
        &family.title,
        &family.media_type,
        content,
    )
}

/// The table covers every product record type, and every schema-backed
/// family — each registry item, the gate-run observation and both typed
/// workspace files — carries a mandatory mutation.
#[test]
fn workspace_state_payload_matrix_covers_every_family() {
    use meridian_core::workspace_state::record_types::{PayloadContract, ProductRecordType};
    let families = families();
    for record_type in ProductRecordType::all() {
        assert!(
            families
                .iter()
                .any(|f| f.record_type == record_type.as_str()),
            "{record_type} has no row"
        );
    }
    let schema_backed = |f: &Family| {
        matches!(
            f.contract.as_str(),
            "registry-item" | "gate-run-observation" | "typed-workspace-file"
        )
    };
    for family in &families {
        assert_eq!(
            schema_backed(family),
            family.mutated.is_some(),
            "{}: a schema-backed family carries exactly one mutation",
            family.title
        );
        let record_type = ProductRecordType::parse(&family.record_type).unwrap();
        let declared = match record_type.contract() {
            PayloadContract::RegistryItem(_) => "registry-item",
            PayloadContract::JsonObject => "json-object",
            PayloadContract::GateRunObservation => "gate-run-observation",
            PayloadContract::Markdown => "markdown",
            PayloadContract::WorkspaceText if family.contract == "typed-workspace-file" => {
                "typed-workspace-file"
            }
            PayloadContract::WorkspaceText => "workspace-text",
        };
        assert_eq!(family.contract, declared, "{}", family.title);
    }
    assert_eq!(families.iter().filter(|f| f.mutated.is_some()).count(), 15);
}

/// Every valid row is accepted and every mutation refused by the one
/// payload validator.
#[test]
fn workspace_state_payload_matrix_every_family_accepts_and_refuses() {
    let payloads = PayloadRegistry::load(&KernelReader::new(&[])).unwrap();
    for (i, family) in families().iter().enumerate() {
        let id = SemanticId::new(format!("matrix-{i}")).unwrap();
        let check = |content: &str| {
            let record = matrix_record(family, id.as_str(), content);
            payloads.check(
                PayloadSubject {
                    id: record.key().id(),
                    scope: record.key().scope(),
                    title: record.title(),
                    record_type: record.record_type().as_str(),
                },
                record.payload().as_map(),
            )
        };
        if let Err(rejection) = check(&family.valid) {
            panic!(
                "{} valid row refused: {:?}",
                family.title, rejection.problems
            );
        }
        if let Some(mutated) = &family.mutated {
            assert!(
                check(mutated).is_err(),
                "{}: the mutation was accepted",
                family.title
            );
        }
    }
}

/// The same rows through the stored-state route: a database holding every
/// valid row rejects nothing; one holding every mutation names each.
#[test]
fn workspace_state_payload_matrix_is_judged_in_the_stored_state() {
    let families = families();
    let valid: Vec<ManagedRecord> = families
        .iter()
        .enumerate()
        .map(|(i, f)| matrix_record(f, &format!("matrix-{i}"), &f.valid))
        .collect();
    let report = World::new(valid).run();
    assert_eq!(report.counts.rejected_payloads, 0, "{:?}", fails(&report));

    let mutated: Vec<ManagedRecord> = families
        .iter()
        .enumerate()
        .filter_map(|(i, f)| {
            f.mutated
                .as_ref()
                .map(|m| matrix_record(f, &format!("matrix-{i}"), m))
        })
        .collect();
    let count = mutated.len();
    let report = World::new(mutated).run();
    assert_eq!(
        report.counts.rejected_payloads,
        count,
        "{:?}",
        fails(&report)
    );
}
