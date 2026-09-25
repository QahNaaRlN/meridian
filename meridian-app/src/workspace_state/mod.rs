//! `rust-workspace-state-validation` (`meridian-rust-migration-program-plan.md`
//! §5.23.11): the read-only check of a workspace database's imported product
//! state that closes the category-3 rows of the GAP-09 mapping — M-02/M-03
//! (product purity), M-05b (payload contracts), M-06 (Instance context of
//! Kernel skills), M-07 (external dependencies), M-08 (inventory against the
//! repositories), M-09 (stack-profile declarations), M-12/M-15 (intake
//! references and completeness, with the topic, packaging and parentage
//! checks of decision D3) — and, as its one opt-in effect, M-18 (a
//! gate-run observation, [`observation_request`]).
//!
//! One operation, [`validate`], over ports only: [`RecordRepository`] for the
//! stored records, [`WorkspaceReader`] for the Kernel, [`RepositoryAccess`]
//! for the local product repositories and [`Clock`] for the instant. It
//! never writes and never reads `MERIDIAN_INSTANCE`.

mod dto;
mod intake;
mod payload;
mod ports;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::{json, Value};

use meridian_core::canonical::CanonicalJson;
use meridian_core::mechanical_integrity::instruction_topics::TopicPool;
use meridian_core::mechanical_integrity::stack_profiles::StackProfileName;
use meridian_core::types::{
    Authority, AuthorityKind, ContentDigest, Diagnostic, DiagnosticLevel, NonEmptyString, Origin,
    Scope, ScopeType, SemanticId, WorkspaceRelativePath,
};
use meridian_core::workspace_state::dependencies::{check_dependencies, ExternalDependency};
use meridian_core::workspace_state::intake::{summarize as intake_summary, IntakeRecord};
use meridian_core::workspace_state::inventory::{self, RepositoryIdentity};
use meridian_core::workspace_state::observation::GateRunObservation;
use meridian_core::workspace_state::product::{self, ProductPurity, PurityFinding};
use meridian_core::workspace_state::profile::{self, Declaration, Manifest, StackProfileSpec};
use meridian_core::workspace_state::record_types::TypedWorkspaceFile;
use meridian_core::workspace_state::timestamp::Timestamp;

use crate::operating_model::migration_boundary::canonical;
use crate::source_format::parse_yaml;
use crate::storage::{
    IdempotencyKey, ManagedRecord, Payload, PortError, PutRecordRequest, RecordKey,
    RecordRepository, RecordSchemaVersion, SchemaRef,
};
use crate::workspace::{EntryKind, ReadError, WorkspaceReader};

pub use payload::{
    PayloadRegistry, PayloadRegistryError, PayloadRejection, PayloadSubject, TypedPayload,
};
pub use ports::{Clock, RepositoryAccess, RepositoryUnavailable, RevisionSelector};

use intake::IntakeContext;

/// Why the operation produced no result at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceStateError {
    /// The workspace database could not be read.
    Storage(String),
    /// A Kernel file the operation needs could not be read.
    Kernel { path: String, message: String },
}

impl std::fmt::Display for WorkspaceStateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkspaceStateError::Storage(message) => {
                write!(f, "the workspace database cannot be read: {message}")
            }
            WorkspaceStateError::Kernel { path, message } => {
                write!(f, "Kernel file {path} cannot be read: {message}")
            }
        }
    }
}

impl std::error::Error for WorkspaceStateError {}

impl From<PortError> for WorkspaceStateError {
    fn from(error: PortError) -> Self {
        WorkspaceStateError::Storage(error.to_string())
    }
}

/// What the Kernel contributes to the check.
pub struct KernelSide<'a> {
    pub reader: &'a dyn WorkspaceReader,
    /// The Kernel's text files `kernel-purity` scans (the same universe as
    /// its personal-path half).
    pub files: &'a [WorkspaceRelativePath],
    /// Whether the stack-profile pool loaded and agreed; declarations are
    /// judged only against an accepted pool.
    pub stack_profile_pool_loaded: bool,
    /// The agreed topic pool; intake topics are judged only against an
    /// accepted pool.
    pub topic_pool: Option<&'a TopicPool>,
    /// The Kernel's root as [`RepositoryAccess`] addresses it — the parent
    /// repository `"kernel"` of an edition.
    pub repository_root: &'a str,
}

/// Every port the operation reads through.
pub struct Ports<'a> {
    pub records: &'a dyn RecordRepository,
    pub repositories: &'a dyn RepositoryAccess,
    pub clock: &'a dyn Clock,
}

/// One product-purity finding in one Kernel file; presentation joins the
/// path to the Kernel root it displays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelPurityFinding {
    pub path: WorkspaceRelativePath,
    pub finding: PurityFinding,
}

/// Counts the presentation reports.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkspaceStateCounts {
    pub records: usize,
    pub rejected_payloads: usize,
    pub repositories: usize,
    pub repositories_confirmed: usize,
    pub intake_records: usize,
    pub intake_repositories_complete: usize,
    pub observations: usize,
}

/// The typed result of one run.
#[derive(Debug, Clone)]
pub struct WorkspaceStateReport {
    /// Every diagnostic, deterministically sorted.
    pub diagnostics: Vec<Diagnostic>,
    /// Product-purity findings, sorted by path.
    pub purity: Vec<KernelPurityFinding>,
    pub counts: WorkspaceStateCounts,
    /// The instant the run judged ages against.
    pub now: Timestamp,
    /// The one project workspace the records belong to, when there is one.
    pub workspace: Result<Scope, String>,
}

fn fail(message: String) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message).expect("message has a family prefix")
}

fn warn(message: String) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Warn, message).expect("message has a family prefix")
}

fn info(message: String) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Info, message).expect("message has a family prefix")
}

/// Everything the stored records say, typed.
#[derive(Default)]
struct Typed {
    identities: Vec<RepositoryIdentity>,
    intake: Vec<IntakeRecord>,
    products: Vec<ProductPurity>,
    dependencies: Vec<Vec<ExternalDependency>>,
    workspace_files: BTreeSet<String>,
    observations: usize,
    product_rejected: bool,
    dependencies_rejected: bool,
}

/// The check. Read-only: it calls no write method of any port.
pub fn validate(
    kernel: &KernelSide<'_>,
    ports: &Ports<'_>,
    payloads: &PayloadRegistry,
) -> Result<WorkspaceStateReport, WorkspaceStateError> {
    let now = ports.clock.now();
    let records = ports.records.export_all()?;
    let mut diagnostics = Vec::new();
    let mut counts = WorkspaceStateCounts {
        records: records.len(),
        ..WorkspaceStateCounts::default()
    };
    let typed = type_records(&records, payloads, &mut diagnostics, &mut counts);
    counts.observations = typed.observations;

    let purity = check_product(kernel, &typed, &mut diagnostics)?;
    check_instance_context(kernel.reader, &typed, &mut diagnostics)?;
    check_external_dependencies(&typed, &mut diagnostics);

    let mut identities: Vec<&RepositoryIdentity> = typed.identities.iter().collect();
    identities.sort_by(|a, b| a.id().as_str().cmp(b.id().as_str()));
    counts.repositories = identities.len();
    let mut confirmed = 0;
    for identity in &identities {
        let observed = ports.repositories.vcs_state(identity.path()).ok();
        let check = inventory::check_entry(identity, observed.as_ref(), now);
        confirmed += usize::from(check.confirmed);
        diagnostics.extend(check.diagnostics);
    }
    counts.repositories_confirmed = confirmed;
    diagnostics.push(inventory::summarize(identities.len(), confirmed));

    if kernel.stack_profile_pool_loaded {
        let pool = load_stack_profiles(kernel.reader, &mut diagnostics)?;
        check_profiles(&identities, &pool, ports.repositories, &mut diagnostics);
    }

    counts.intake_records = typed.intake.len();
    let outcome = intake::check(
        &typed.intake,
        &IntakeContext {
            identities: &identities,
            repositories: ports.repositories,
            kernel_root: kernel.repository_root,
            topic_pool: kernel.topic_pool,
        },
    );
    diagnostics.extend(outcome.diagnostics);
    counts.intake_repositories_complete = outcome.complete;
    diagnostics.push(intake_summary(
        typed.intake.len(),
        outcome.repositories,
        outcome.complete,
    ));

    Diagnostic::sort(&mut diagnostics);
    Ok(WorkspaceStateReport {
        diagnostics,
        purity,
        counts,
        now,
        workspace: workspace_scope(&records),
    })
}

fn type_records(
    records: &[ManagedRecord],
    payloads: &PayloadRegistry,
    diagnostics: &mut Vec<Diagnostic>,
    counts: &mut WorkspaceStateCounts,
) -> Typed {
    let mut typed = Typed::default();
    for record in records {
        let subject = PayloadSubject {
            id: record.key().id(),
            scope: record.key().scope(),
            title: record.title(),
            record_type: record.record_type().as_str(),
        };
        match payloads.check(subject, record.payload().as_map()) {
            Ok(TypedPayload::RepositoryIdentity(identity)) => typed.identities.push(identity),
            Ok(TypedPayload::IntakeRecord(record)) => typed.intake.push(record),
            Ok(TypedPayload::Observation(_)) => typed.observations += 1,
            Ok(TypedPayload::Product(purity)) => typed.products.push(purity),
            Ok(TypedPayload::ExternalDependencies(deps)) => typed.dependencies.push(deps),
            Ok(TypedPayload::WorkspaceFile(path)) => {
                typed.workspace_files.insert(path.as_str().to_string());
            }
            Ok(TypedPayload::Conforming) => {}
            Err(rejection) => {
                counts.rejected_payloads += 1;
                match rejection.typed_file {
                    Some(TypedWorkspaceFile::Product) => {
                        typed.product_rejected = true;
                        for problem in rejection.problems {
                            diagnostics.push(fail(format!("product record: {problem}")));
                        }
                    }
                    Some(TypedWorkspaceFile::ExternalDependencies) => {
                        typed.dependencies_rejected = true;
                        for problem in rejection.problems {
                            diagnostics.push(fail(format!("ext-dependencies: {problem}")));
                        }
                    }
                    None => {
                        for problem in rejection.problems {
                            diagnostics.push(fail(format!(
                                "payload-contract: {} record \"{}\" in {} \"{}\": {problem}",
                                record.record_type().as_str(),
                                record.key().id().as_str(),
                                record.key().scope().scope_type().as_str(),
                                record.key().scope().id()
                            )));
                        }
                    }
                }
            }
        }
    }
    typed
}

fn check_product(
    kernel: &KernelSide<'_>,
    typed: &Typed,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Vec<KernelPurityFinding>, WorkspaceStateError> {
    let purity = match typed.products.as_slice() {
        [one] => one,
        [] => {
            if !typed.product_rejected {
                diagnostics.push(fail(
                    "product record not found in the workspace database (workspace file \"product.yaml\"); kernel-purity cannot be verified".to_string(),
                ));
            }
            return Ok(Vec::new());
        }
        several => {
            diagnostics.push(fail(format!(
                "product record: the workspace database holds {} product records (workspace file \"product.yaml\"); the product is ambiguous and kernel-purity cannot be verified",
                several.len()
            )));
            return Ok(Vec::new());
        }
    };
    let mut findings = Vec::new();
    for path in kernel.files {
        let text = match kernel.reader.read_text(path) {
            Ok(text) => text,
            // The same universe and the same posture as the personal-path
            // half: a file that is gone or not text is not scanned.
            Err(_) => continue,
        };
        for finding in product::scan(&text, purity) {
            findings.push(KernelPurityFinding {
                path: path.clone(),
                finding,
            });
        }
    }
    findings.sort_by(|a, b| {
        a.path
            .as_str()
            .cmp(b.path.as_str())
            .then_with(|| a.finding.cmp(&b.finding))
    });
    Ok(findings)
}

#[derive(Deserialize)]
struct PinContextDto {
    #[serde(default)]
    requires_instance_context: Option<Value>,
}

fn check_instance_context(
    kernel: &dyn WorkspaceReader,
    typed: &Typed,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(), WorkspaceStateError> {
    let skills = WorkspaceRelativePath::new("skills").expect("literal path is valid");
    let entries = match kernel.list_dir(&skills) {
        Ok(entries) => entries,
        Err(ReadError::NotFound) => return Ok(()),
        Err(ReadError::Io(message)) => {
            return Err(WorkspaceStateError::Kernel {
                path: "skills".to_string(),
                message,
            })
        }
    };
    let mut names: Vec<&str> = entries
        .iter()
        .filter(|e| e.kind == EntryKind::Dir)
        .map(|e| e.name.as_str())
        .collect();
    names.sort_unstable();
    for name in names {
        let Ok(pin_path) = skills.join(name).and_then(|dir| dir.join("PIN.yaml")) else {
            continue;
        };
        // An unreadable or malformed PIN is `sha-provenance`'s finding.
        let Ok(raw) = kernel.read_text(&pin_path) else {
            continue;
        };
        let Some(Value::String(context)) = parse_yaml(&raw)
            .ok()
            .and_then(|doc| serde_json::from_value::<PinContextDto>(doc).ok())
            .and_then(|pin| pin.requires_instance_context)
        else {
            continue;
        };
        if !typed.workspace_files.contains(&context) {
            diagnostics.push(fail(format!(
                "instance-context: {name} requires \"{context}\", which is missing from the workspace database; the skill declares this a blocker, not a licence to guess"
            )));
        }
    }
    Ok(())
}

fn check_external_dependencies(typed: &Typed, diagnostics: &mut Vec<Diagnostic>) {
    match typed.dependencies.as_slice() {
        [] if typed.dependencies_rejected => {}
        [] => diagnostics.push(warn("ext-dependencies: record not found".to_string())),
        [one] => {
            diagnostics.extend(check_dependencies(one));
            diagnostics.push(info(format!("ext-dependencies: {} declared", one.len())));
        }
        several => diagnostics.push(fail(format!(
            "ext-dependencies: the workspace database holds {} external-dependency records (workspace file \"external-dependencies.yaml\"); the record is ambiguous",
            several.len()
        ))),
    }
}

// ------------------------------------------------------------- stack profiles

#[derive(Deserialize)]
struct PoolDto {
    #[serde(default)]
    profiles: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileSpecDto {
    name: String,
    manifest: String,
    #[serde(default)]
    requires: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    forbids: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    forbids_fields: Vec<String>,
}

fn load_stack_profiles(
    kernel: &dyn WorkspaceReader,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Vec<StackProfileSpec>, WorkspaceStateError> {
    const POOL: &str = "stack-profiles/stack-profiles.yaml";
    let path = WorkspaceRelativePath::new(POOL).expect("literal path is valid");
    let raw = kernel
        .read_text(&path)
        .map_err(|e| WorkspaceStateError::Kernel {
            path: POOL.to_string(),
            message: e.to_string(),
        })?;
    let entries = parse_yaml(&raw)
        .ok()
        .and_then(|doc| serde_json::from_value::<PoolDto>(doc).ok())
        .map(|pool| pool.profiles)
        .unwrap_or_default();
    let mut specs = Vec::new();
    for (i, entry) in entries.into_iter().enumerate() {
        let converted = serde_json::from_value::<ProfileSpecDto>(entry)
            .map_err(|e| e.to_string())
            .and_then(|dto| {
                Ok(StackProfileSpec::new(
                    StackProfileName::new(dto.name).map_err(|e| format!("name: {e}"))?,
                    NonEmptyString::new(dto.manifest).map_err(|e| format!("manifest: {e}"))?,
                    dto.requires.into_iter().collect(),
                    dto.forbids.into_iter().collect(),
                    dto.forbids_fields,
                ))
            });
        match converted {
            Ok(spec) => specs.push(spec),
            Err(e) => diagnostics.push(fail(format!(
                "stack-profile: pool entry {i} of {POOL} cannot be read as a profile ({e}); no declaration naming it can be judged"
            ))),
        }
    }
    Ok(specs)
}

/// The manifest's path relative to the repository root, when it lies inside
/// it.
fn relative_to(root: &str, path: &str) -> Option<WorkspaceRelativePath> {
    let root = root.replace('\\', "/");
    let path = path.replace('\\', "/");
    let rest = path
        .strip_prefix(root.trim_end_matches('/'))?
        .strip_prefix('/')?;
    WorkspaceRelativePath::new(rest).ok()
}

fn manifest_of(value: &Value) -> Manifest {
    let Some(object) = value.as_object() else {
        return Manifest::default();
    };
    let fields = object.keys().cloned().collect();
    let sections = object
        .iter()
        .filter_map(|(key, value)| {
            value
                .as_object()
                .map(|section| (key.clone(), section.keys().cloned().collect()))
        })
        .collect();
    Manifest::new(fields, sections)
}

fn check_profiles(
    identities: &[&RepositoryIdentity],
    pool: &[StackProfileSpec],
    repositories: &dyn RepositoryAccess,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut confirmed = 0;
    let mut unconfirmed = 0;
    for identity in identities {
        let (spec, manifest_path) = match profile::declaration(identity, pool) {
            Declaration::Refused(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
            Declaration::Evidence {
                spec,
                manifest_path,
            } => (spec, manifest_path),
        };
        let text = relative_to(identity.path(), manifest_path)
            .and_then(|rel| repositories.reader(identity.path()).read_text(&rel).ok());
        let Some(text) = text else {
            diagnostics.push(profile::unreadable_manifest(identity, spec, manifest_path));
            unconfirmed += 1;
            continue;
        };
        let value: Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(e) => {
                diagnostics.push(profile::invalid_manifest(
                    identity,
                    manifest_path,
                    &e.to_string(),
                ));
                continue;
            }
        };
        match profile::judge_manifest(identity, spec, &manifest_of(&value)) {
            Some(mismatch) => diagnostics.push(mismatch),
            None => confirmed += 1,
        }
    }
    diagnostics.extend(profile::summarize(identities.len(), confirmed, unconfirmed));
}

// ----------------------------------------------------------- observation

/// The one project workspace every product record of the database belongs
/// to (directly, or through its repository or run scope).
fn workspace_scope(records: &[ManagedRecord]) -> Result<Scope, String> {
    let mut workspaces: BTreeSet<String> = BTreeSet::new();
    for record in records {
        let scope = record.key().scope();
        match scope.scope_type() {
            ScopeType::ProjectWorkspace => {
                workspaces.insert(scope.id().to_string());
            }
            _ => {
                if let Some(workspace) = scope.workspace_id() {
                    workspaces.insert(workspace.as_str().to_string());
                }
            }
        }
    }
    let mut workspaces = workspaces.into_iter();
    match (workspaces.next(), workspaces.next()) {
        (Some(one), None) => SemanticId::new(one)
            .map(|id| Scope::project_workspace(id, None))
            .map_err(|e| format!("the workspace id is not a semantic id: {e}")),
        (None, _) => Err("the workspace database holds no project workspace record".to_string()),
        (Some(_), Some(_)) => {
            Err("the workspace database holds records of several project workspaces".to_string())
        }
    }
}

/// The canonical JSON content of an observation — the same closed shape
/// the payload registry reads back.
fn observation_content(observation: &GateRunObservation) -> Value {
    let counts = observation.counts();
    json!({
        "ts": observation.ts().to_string(),
        "kernel_version": observation.kernel_version(),
        "kernel_revision": observation.kernel_revision(),
        "instance_revision": observation.instance_revision(),
        "exit_code": observation.exit_code(),
        "failing": counts.failing,
        "warnings": counts.warnings,
        "info_ok": counts.info_ok,
        "fail_messages": observation.fail_messages(),
        "warn_messages": observation.warn_messages(),
    })
}

/// M-18: the write of one gate-run observation into `workspace`. Its
/// idempotency key is the digest of the observation's canonical content, so
/// repeating the same logical run writes nothing. The request is checked by
/// the same [`PayloadRegistry`] the imports and `validate` use before it is
/// returned.
pub fn observation_request(
    observation: &GateRunObservation,
    workspace: &Scope,
    payloads: &PayloadRegistry,
) -> Result<PutRecordRequest, String> {
    let content: CanonicalJson = canonical(&observation_content(observation));
    let text = content.canonical_text();
    let digest = ContentDigest::of_str(&text);
    let key_digest = content.digest();
    let id = SemanticId::new(format!(
        "gate-run-observation-{}",
        &key_digest.value()[..16]
    ))
    .map_err(|e| format!("id: {e}"))?;
    let record_type = SemanticId::new("gate-run-observation").map_err(|e| e.to_string())?;
    let title = format!("Gate run {}", observation.ts());
    let payload = json!({
        "media_type": "application/json",
        "encoding": "utf-8",
        "content": text,
        "digest": {"algorithm": digest.algorithm().as_str(), "value": digest.value()},
    });
    let Value::Object(payload_map) = &payload else {
        return Err("internal: the payload is not an object".to_string());
    };
    payloads
        .check(
            PayloadSubject {
                id: &id,
                scope: workspace,
                title: &title,
                record_type: record_type.as_str(),
            },
            payload_map,
        )
        .map_err(|r| {
            format!(
                "the observation does not satisfy its own contract: {}",
                r.problems.join("; ")
            )
        })?;
    PutRecordRequest::new(
        RecordKey::new(workspace.clone(), id),
        SchemaRef::new("registries/operating-model/scoped-record.schema.json")
            .map_err(|e| e.to_string())?,
        RecordSchemaVersion::CURRENT,
        NonEmptyString::new(title).map_err(|e| e.to_string())?,
        record_type,
        Origin::derived("meridian-validate").map_err(|e| e.to_string())?,
        Authority::new(AuthorityKind::DelegatedRun, "meridian-validate", None)
            .map_err(|e| e.to_string())?,
        Payload::new(payload).map_err(|e| e.to_string())?,
        IdempotencyKey::new(key_digest.value()).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
