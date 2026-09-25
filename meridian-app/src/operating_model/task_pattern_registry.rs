//! `task-pattern-registry` — app-owned orchestration for the
//! `rust-architecture-conformance-3` production route:
//!
//! ```text
//! WorkspaceReader + GitInspector
//!   -> private transport DTO (this module)
//!   -> domain constructors in meridian-core (meridian_core::task_contracts::catalog)
//!   -> TaskPatternCatalog
//!   -> Vec<Diagnostic>
//!   -> existing CLI presentation
//! ```
//!
//! `standards/workspace/task-pattern-registry.yaml` (the built-in universal
//! task-type catalog) is a mandatory part of every Kernel. Its
//! container/payload shape is checked against
//! `registries/operating-model/task-pattern-registry.schema.json`, each
//! record's envelope against `registries/operating-model/scoped-record.schema.json`,
//! and the cross-record rules the JSON Schema subset cannot state — unique
//! pattern ids, the seven mandatory classification pairs each declared
//! exactly once, canonical-link path confinement and classification, and
//! the REFACTOR/BUGFIX-specific wiring rules — are enforced by
//! [`meridian_core::task_contracts::catalog::TaskPatternCatalog`]. A text
//! guard also keeps `standards/workspace/rule-resolution.md` from
//! re-asserting that `BUGFIX` routes to a Kernel *protocol*
//! (`bugfix-protocol` is a skill, owner decision).
//!
//! This module owns the file reads (through [`WorkspaceReader`]), the YAML
//! and JSON Schema parsing, the closed transport DTOs and their conversion
//! into domain types, and the external per-link check (a `present`
//! canonical link must resolve to a real, tracked, regular file inside the
//! Kernel) against [`LinkTargetPort`] (filesystem shape) and
//! [`GitInspector`] (tracked-file membership) — the ONLY two places this
//! family's production route reaches outside `meridian-app`'s own pure
//! boundary. `meridian-core` performs no filesystem, Git, process, env or
//! `serde_json::Value` I/O; the two CLI command modules
//! (`meridian-cli/src/commands/validate/task_pattern_registry.rs`) supply
//! only the concrete adapters and turn the returned diagnostics into
//! human/json presentation.
//!
//! `standards/workspace/task-pattern-registry.fixtures.json`'s valid/invalid
//! bundle and the real Kernel document run through this exact same pipeline
//! (`evaluate_document`/`check_external_links`) — there is no separate,
//! looser fixture-only code path.

use std::collections::HashSet;

use serde::Deserialize;
use serde_json::Value;

use meridian_core::resolver::{ChangeClass, WorkItemKind, WorkKind};
use meridian_core::task_contracts::catalog::{
    CanonicalLink as DomainCanonicalLink, CanonicalLinkField, TaskPatternCatalog,
};
use meridian_core::task_contracts::TaskPatternEntry;
use meridian_core::types::{
    Diagnostic, DiagnosticLevel, NonEmptyString, SemanticId, WorkspaceRelativePath,
};

use crate::source_format::{json_schema, parse_yaml};
use crate::validation::mechanical_integrity::OperationError;
use crate::workspace::{
    GitInspector, GitInspectorError, LinkTargetError, LinkTargetPort, ReadError, WorkspaceReader,
};

const YAML_PATH: &str = "standards/workspace/task-pattern-registry.yaml";
const REG_SCHEMA_PATH: &str = "registries/operating-model/task-pattern-registry.schema.json";
const ENV_SCHEMA_PATH: &str = "registries/operating-model/scoped-record.schema.json";
const FIXTURES_PATH: &str =
    "registries/operating-model/fixtures/task-pattern-registry.fixtures.json";
const RULE_RESOLUTION_PATH: &str = "standards/workspace/rule-resolution.md";

/// The existing closed pools (`standards/workspace/rule-resolution.md`
/// §2-§3), in the exact order the JSON Schema's own `enum` arrays declare
/// them — used only for the schema-pool cross-check below, which compares
/// JSON array VALUES (order-sensitive) rather than the closed Rust type.
const WORK_KINDS: [&str; 4] = ["assessment", "operation", "initiative", "change"];
const CHANGE_CLASSES: [&str; 4] = ["BUGFIX", "FEATURE", "BEHAVIOR_CHANGE", "REFACTOR"];

fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message).expect("message is non-empty")
}

fn warn(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Warn, message).expect("message is non-empty")
}

/// Every literal path this module reads is a compile-time constant declared
/// a few lines above — its validity is provable by inspection, the same
/// internal invariant `meridian_app::validation::mechanical_integrity::sha_provenance`'s
/// own `WorkspaceRelativePath::new("skills").expect(...)` already relies on
/// in this crate, never a value derived from workspace bytes, decoded DTOs,
/// Git output or catalogue data.
fn wp(literal: &str) -> WorkspaceRelativePath {
    WorkspaceRelativePath::new(literal).expect("literal path constant is a valid workspace path")
}

fn read_optional(
    reader: &dyn WorkspaceReader,
    path: &WorkspaceRelativePath,
) -> Result<Option<String>, OperationError> {
    match reader.read_text(path) {
        Ok(text) => Ok(Some(text)),
        Err(ReadError::NotFound) => Ok(None),
        Err(ReadError::Io(message)) => Err(OperationError {
            path: path.as_str().to_string(),
            message,
        }),
    }
}

fn load_schema_text(
    reader: &dyn WorkspaceReader,
    rel: &str,
    id: &str,
) -> Result<Result<Value, Diagnostic>, OperationError> {
    let Some(raw) = read_optional(reader, &wp(rel))? else {
        return Ok(Err(fail(format!(
            "{rel} is missing; the mandatory catalog cannot be checked without its {id} schema"
        ))));
    };
    let parsed: Value = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(error) => return Ok(Err(fail(format!("{rel} is not valid JSON: {error}")))),
    };
    if let Err(error) = json_schema::assert_supported_deep(&parsed, id) {
        return Ok(Err(fail(format!(
            "the {id} schema uses a construct this validator cannot check: {error}"
        ))));
    }
    Ok(Ok(parsed))
}

/// Port of `checkRuleResolutionBugfixConsistency`: `rule-resolution.md` must
/// not, at the same time the catalog treats `bugfix-protocol` as a skill,
/// still call `BUGFIX → bugfix-protocol` a route to a Kernel *protocol*.
fn check_rule_resolution_bugfix_consistency(text: &str) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    let arrow_re = fancy_regex::Regex::new(r"BUGFIX\s*(?:\u{2192}|-&gt;|->|-->)\s*bugfix-protocol")
        .expect("arrow pattern compiles");
    if arrow_re.is_match(text).unwrap_or(false) {
        problems.push(fail("rule-resolution.md still routes \"BUGFIX → bugfix-protocol\" as a protocol route; bugfix-protocol is a skill (owner decision), and no separate BUGFIX protocol exists"));
    }
    let bugfix_re = fancy_regex::Regex::new(r"bugfix-protocol").expect("bugfix pattern compiles");
    let kernel_protocol_re =
        fancy_regex::Regex::new(r"(?i)протокол\s+ядра").expect("kernel-protocol pattern compiles");
    for line in text.split('\n') {
        if bugfix_re.is_match(line).unwrap_or(false)
            && kernel_protocol_re.is_match(line).unwrap_or(false)
        {
            problems.push(fail("rule-resolution.md names bugfix-protocol on the same line as \"протокол ядра\"; bugfix-protocol is a skill, not a Kernel protocol"));
            break;
        }
    }
    problems
}

#[derive(Debug, Clone, Deserialize)]
struct RegistryDto {
    #[serde(default)]
    task_patterns: Vec<EntryDto>,
}

#[derive(Debug, Clone, Deserialize)]
struct EntryDto {
    id: String,
    record_type: String,
    payload: PayloadDto,
}

#[derive(Debug, Clone, Deserialize)]
struct PayloadDto {
    work_kind: String,
    #[serde(default)]
    change_class: Option<String>,
    #[serde(default)]
    applicable_protocols: Vec<LinkDto>,
    #[serde(default)]
    applicable_skills: Vec<LinkDto>,
    #[serde(default)]
    applicable_evidence_contracts: Vec<LinkDto>,
}

/// The one DTO in this family whose shape genuinely matters beyond what the
/// mandatory JSON Schema gate already governs (`status`-conditional
/// presence of `id`/`path` vs. `absence_reason`), so it alone is closed
/// (`#[serde(deny_unknown_fields)]`) — every other DTO here is a plain
/// projection of fields this module actually reads, with the rest of the
/// document's shape already the schema gate's job.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct LinkDto {
    status: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    absence_reason: Option<String>,
}

fn parse_work_item_kind(
    work_kind: &str,
    change_class: Option<&str>,
) -> Result<WorkItemKind, Diagnostic> {
    let wk = match work_kind {
        "assessment" => WorkKind::Assessment,
        "operation" => WorkKind::Operation,
        "initiative" => WorkKind::Initiative,
        "change" => WorkKind::Change,
        other => return Err(fail(format!("unknown work_kind \"{other}\""))),
    };
    match wk {
        WorkKind::Change => {
            let cc = change_class
                .ok_or_else(|| fail("a \"change\" work_kind requires a change_class"))?;
            let cc = match cc {
                "BUGFIX" => ChangeClass::Bugfix,
                "FEATURE" => ChangeClass::Feature,
                "BEHAVIOR_CHANGE" => ChangeClass::BehaviorChange,
                "REFACTOR" => ChangeClass::Refactor,
                other => return Err(fail(format!("unknown change_class \"{other}\""))),
            };
            Ok(WorkItemKind::Change(cc))
        }
        WorkKind::Assessment => Ok(WorkItemKind::Assessment),
        WorkKind::Operation => Ok(WorkItemKind::Operation),
        WorkKind::Initiative => Ok(WorkItemKind::Initiative),
    }
}

fn convert_link(
    dto: &LinkDto,
    field: &str,
    pattern_id: &str,
) -> Result<DomainCanonicalLink, Diagnostic> {
    match dto.status.as_str() {
        "present" => {
            let id_str = dto.id.as_deref().ok_or_else(|| {
                fail(format!(
                    "pattern \"{pattern_id}\" {field} link is present but carries no id"
                ))
            })?;
            let path_str = dto.path.as_deref().ok_or_else(|| {
                fail(format!(
                    "pattern \"{pattern_id}\" {field} link is present but carries no path"
                ))
            })?;
            let id = SemanticId::new(id_str).map_err(|error| {
                fail(format!(
                    "pattern \"{pattern_id}\" {field} link id \"{id_str}\": {error}"
                ))
            })?;
            let path = WorkspaceRelativePath::new(path_str).map_err(|error| {
                fail(format!(
                    "pattern \"{pattern_id}\" {field} link \"{path_str}\": {error}"
                ))
            })?;
            Ok(DomainCanonicalLink::Present { id, path })
        }
        "absent" => {
            let reason = dto.absence_reason.as_deref().unwrap_or("");
            let reason = NonEmptyString::new(reason).map_err(|_| {
                fail(format!(
                    "pattern \"{pattern_id}\" {field} link is absent but carries no reason"
                ))
            })?;
            Ok(DomainCanonicalLink::Absent { reason })
        }
        other => Err(fail(format!(
            "pattern \"{pattern_id}\" {field} link has unknown status \"{other}\""
        ))),
    }
}

fn convert_entry(dto: &EntryDto) -> Result<TaskPatternEntry, Vec<Diagnostic>> {
    let mut problems = Vec::new();

    let id = match SemanticId::new(dto.id.clone()) {
        Ok(id) => id,
        Err(error) => {
            problems.push(fail(format!("pattern id \"{}\": {error}", dto.id)));
            return Err(problems);
        }
    };
    if dto.record_type != "task-pattern" {
        problems.push(fail(format!(
            "pattern \"{}\" declares record_type \"{}\", not \"task-pattern\"",
            dto.id, dto.record_type
        )));
        return Err(problems);
    }
    let kind =
        match parse_work_item_kind(&dto.payload.work_kind, dto.payload.change_class.as_deref()) {
            Ok(kind) => kind,
            Err(diagnostic) => {
                problems.push(diagnostic);
                return Err(problems);
            }
        };

    let mut protocols = Vec::new();
    let mut skills = Vec::new();
    let mut evidence_contracts = Vec::new();
    let mut links_ok = true;
    for (link_dtos, field, out) in [
        (
            &dto.payload.applicable_protocols,
            "applicable_protocols",
            &mut protocols,
        ),
        (
            &dto.payload.applicable_skills,
            "applicable_skills",
            &mut skills,
        ),
        (
            &dto.payload.applicable_evidence_contracts,
            "applicable_evidence_contracts",
            &mut evidence_contracts,
        ),
    ] {
        for link_dto in link_dtos {
            match convert_link(link_dto, field, &dto.id) {
                Ok(link) => out.push(link),
                Err(diagnostic) => {
                    problems.push(diagnostic);
                    links_ok = false;
                }
            }
        }
    }
    if !links_ok {
        return Err(problems);
    }

    TaskPatternEntry::try_new(id, kind, protocols, skills, evidence_contracts).map_err(|mut d| {
        problems.append(&mut d);
        problems
    })
}

/// The whole in-memory composition pipeline for one registry document —
/// container/payload schema, per-entry envelope schema, closed transport
/// DTO parse, per-entry domain construction, and the catalogue-level
/// cross-record rules. Never touches a file, Git or a port: the external
/// per-link check is a caller's separate step
/// ([`check_external_links`]), because it alone needs I/O.
/// **Fail-closed** (`rust-architecture-conformance-3` corrective round item
/// 1): returns a `Some` catalogue only when EVERY step below — container
/// schema, per-entry envelope schema, closed transport DTO parse, per-entry
/// domain construction, AND the catalogue-level cross-record rules
/// ([`TaskPatternCatalog::build`]'s own fail-closed gate) — succeeds with no
/// diagnostics. A schema, DTO or domain-construction failure never leaves a
/// catalogue built only from the entries that happened to parse; the
/// catalogue is either the complete, checked result of the whole document,
/// or it does not exist.
fn evaluate_document(
    doc: &Value,
    registry_schema: &Value,
    envelope_schema: &Value,
) -> (Vec<Diagnostic>, Option<TaskPatternCatalog>) {
    let mut diagnostics = Vec::new();
    let mut ok = true;

    match json_schema::validate(doc, registry_schema) {
        Ok(errors) => {
            if !errors.is_empty() {
                ok = false;
            }
            diagnostics.extend(errors.into_iter().map(fail));
        }
        Err(error) => {
            diagnostics.push(fail(format!(
                "container/payload schema could not be applied: {error}"
            )));
            return (diagnostics, None);
        }
    }

    let empty_vec = Vec::new();
    let entries_value: &Vec<Value> = doc
        .get("task_patterns")
        .and_then(Value::as_array)
        .unwrap_or(&empty_vec);
    for (i, entry) in entries_value.iter().enumerate() {
        match json_schema::validate(entry, envelope_schema) {
            Ok(errors) => {
                if !errors.is_empty() {
                    ok = false;
                }
                diagnostics.extend(
                    errors
                        .into_iter()
                        .map(|m| fail(format!("entry {i} envelope {m}"))),
                );
            }
            Err(error) => {
                diagnostics.push(fail(format!(
                    "entry {i} envelope could not be applied: {error}"
                )));
                ok = false;
            }
        }
    }

    let registry_dto: RegistryDto = match serde_json::from_value(doc.clone()) {
        Ok(dto) => dto,
        Err(_) => {
            diagnostics.push(fail(
                "the registry document could not be parsed into the closed transport shape",
            ));
            return (diagnostics, None);
        }
    };

    let mut entries = Vec::new();
    for entry_dto in &registry_dto.task_patterns {
        match convert_entry(entry_dto) {
            Ok(entry) => entries.push(entry),
            Err(mut d) => {
                diagnostics.append(&mut d);
                ok = false;
            }
        }
    }

    // `entries` still runs through `TaskPatternCatalog::build` even when an
    // earlier schema/domain-construction step already set `ok = false` —
    // exactly as before this corrective round — so a document with BOTH a
    // container-schema defect AND a catalogue-level defect (for example, a
    // duplicate id that also trips the schema's own `uniqueItems`) still
    // surfaces both diagnostics, not just the first one reached. The only
    // change is the FINAL `catalog` binding below: `TaskPatternCatalog::build`
    // is itself already fail-closed for catalogue-level defects, and any
    // EARLIER diagnostic (schema, DTO parse already handled above, or
    // domain construction) now additionally forces the returned catalog to
    // `None` even if the entries that did convert happened to build a
    // superficially clean catalogue.
    let (catalog, catalog_problems) = TaskPatternCatalog::build(entries);
    if !catalog_problems.is_empty() {
        ok = false;
    }
    diagnostics.extend(catalog_problems);

    let catalog = if ok { catalog } else { None };
    (diagnostics, catalog)
}

fn link_target_diagnostic(
    entry_id: &SemanticId,
    field: CanonicalLinkField,
    path: &WorkspaceRelativePath,
    error: LinkTargetError,
) -> Diagnostic {
    let reason = match error {
        LinkTargetError::Missing => "the target does not exist".to_string(),
        LinkTargetError::Io(message) => format!("the target cannot be inspected: {message}"),
        LinkTargetError::NotRegularFile => "the target is not a regular file".to_string(),
        LinkTargetError::SymlinkEscape => {
            "the target resolves outside the Kernel once symbolic links are followed".to_string()
        }
    };
    fail(format!(
        "pattern \"{entry_id}\" {field} link \"{path}\": {reason}"
    ))
}

/// The external boundary a pure `evaluate_document` cannot cross: every
/// `present` canonical link resolves to a real, regular file inside the
/// Kernel ([`LinkTargetPort`]), and — when the tracked-file set is known —
/// that file is genuinely tracked by Git. `tracked: None` means Git
/// enumeration was unavailable; the caller has already emitted the one
/// required warning ([`resolve_tracked_set`]), and this function accepts a
/// target's mere filesystem existence instead of also requiring
/// trackedness, exactly as `meridian validate`'s existing filesystem
/// fallback does elsewhere.
fn check_external_links(
    catalog: &TaskPatternCatalog,
    tracked: Option<&HashSet<String>>,
    link_target: &dyn LinkTargetPort,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for entry in catalog.entries() {
        for (field, _id, path) in entry.present_links() {
            match link_target.check(path) {
                Ok(()) => {
                    if let Some(tracked) = tracked {
                        if !tracked.contains(path.as_str()) {
                            diagnostics.push(fail(format!(
                                "pattern \"{}\" {field} link \"{path}\": the target is not in the Kernel's tracked file set",
                                entry.id()
                            )));
                        }
                    }
                }
                Err(error) => {
                    diagnostics.push(link_target_diagnostic(entry.id(), field, path, error))
                }
            }
        }
    }
    diagnostics
}

/// The ONE Git snapshot the whole registry route uses
/// (`rust-architecture-conformance-3` corrective round item 3):
/// [`GitInspector::tracked_files`] is called exactly once, from
/// [`evaluate`], and its single outcome governs every canonical-link check
/// below it.
enum TrackedSetResolution {
    /// A known, trustworthy tracked-file set.
    Tracked(HashSet<String>),
    /// Git itself could not be consulted (no repository, `git` missing,
    /// non-zero exit) — the existing explicit filesystem-existence fallback
    /// applies. The accompanying [`Diagnostic`] is NOT folded into
    /// [`evaluate`]'s generic `diagnostics` list; it is carried on
    /// [`Outcome::git_unavailable`] instead, typed, all the way to whatever
    /// boundary presents it — a composed caller that already reports
    /// Git-tracked-set availability once for a whole run
    /// (`meridian_cli::commands::validate`) can then simply never read this
    /// field, with no text comparison required to avoid a duplicate
    /// (`rust-architecture-conformance-3` corrective round, "one normalized
    /// Git snapshot, no text-based dedup").
    UnavailableFallback(Diagnostic),
    /// Git answered, but one of its own reported paths is not a valid
    /// workspace-relative path — a data-integrity problem in Git's own
    /// output, not "Git is merely unavailable". Falling back to accepting
    /// filesystem existence would silently trust an already-untrustworthy
    /// snapshot, so this fails closed instead: a `Fail` diagnostic, no
    /// fallback, and (`evaluate`) no catalog.
    Invalid(Diagnostic),
}

fn resolve_tracked_set(git: &dyn GitInspector) -> TrackedSetResolution {
    match git.tracked_files() {
        Ok(files) => TrackedSetResolution::Tracked(
            files.iter().map(|p| p.as_str().to_string()).collect(),
        ),
        Err(error @ GitInspectorError::Unavailable) => {
            // Prefix-free (`rust-architecture-conformance-3` corrective
            // round, "presentation ownership"): the `task-pattern-registry:
            // ` CLI presentation prefix is applied exactly once, by the CLI
            // command module, never baked in here — an app-owned diagnostic
            // never pre-bakes its caller's family prefix.
            TrackedSetResolution::UnavailableFallback(warn(format!(
                "Git enumeration unavailable ({error}); canonical-link tracked-file checks are skipped and a target's filesystem existence is accepted instead"
            )))
        }
        Err(GitInspectorError::InvalidPath { raw, reason }) => TrackedSetResolution::Invalid(fail(format!(
            "Git reported a workspace-invalid tracked path \"{raw}\" ({reason}); the tracked-file snapshot cannot be trusted, so canonical-link checks cannot fall back to accepting filesystem existence"
        ))),
    }
}

fn run_fixture_bundle(
    bundle: &Value,
    registry_schema: &Value,
    envelope_schema: &Value,
    tracked: Option<&HashSet<String>>,
    link_target: &dyn LinkTargetPort,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for key in ["valid", "invalid"] {
        let nonempty = bundle
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty());
        if !nonempty {
            diagnostics.push(fail(format!(
                "the fixtures file has no non-empty \"{key}\" array"
            )));
        }
    }
    if !diagnostics.is_empty() {
        return diagnostics;
    }

    let evaluate_case = |registry: &Value| -> Vec<Diagnostic> {
        let (mut problems, catalog) = evaluate_document(registry, registry_schema, envelope_schema);
        if let Some(catalog) = &catalog {
            problems.extend(check_external_links(catalog, tracked, link_target));
        }
        problems
    };

    let empty: Vec<Value> = Vec::new();
    for case in bundle
        .get("valid")
        .and_then(Value::as_array)
        .unwrap_or(&empty)
    {
        let registry = case.get("registry").cloned().unwrap_or(Value::Null);
        let problems = evaluate_case(&registry);
        if !problems.is_empty() {
            diagnostics.push(fail(format!(
                "a fixture that must be a valid catalog was rejected ({}): {}",
                case.get("note").and_then(Value::as_str).unwrap_or(""),
                problems[0].message()
            )));
        }
    }
    for case in bundle
        .get("invalid")
        .and_then(Value::as_array)
        .unwrap_or(&empty)
    {
        let registry = case.get("registry").cloned().unwrap_or(Value::Null);
        let problems = evaluate_case(&registry);
        if problems.is_empty() {
            diagnostics.push(fail(format!(
                "a fixture that must be rejected validated clean ({})",
                case.get("note").and_then(Value::as_str).unwrap_or("")
            )));
        }
    }
    diagnostics
}

pub struct Ports<'a> {
    pub reader: &'a dyn WorkspaceReader,
    pub git: &'a dyn GitInspector,
    pub link_target: &'a dyn LinkTargetPort,
}

pub struct Outcome {
    pub diagnostics: Vec<Diagnostic>,
    pub catalog: Option<TaskPatternCatalog>,
    /// Set exactly when Git tracked-file enumeration was unavailable and
    /// this route fell back to accepting a target's mere filesystem
    /// existence — a typed carrier for that ONE fact, kept OUT of
    /// `diagnostics` rather than folded in and later filtered back out by a
    /// caller scanning message text. `None` covers both "Git answered" and
    /// "the route never reached Git resolution" (an earlier gate, such as a
    /// missing registry YAML, already returned).
    pub git_unavailable: Option<Diagnostic>,
}

/// The whole `task-pattern-registry` production route. See this module's
/// own doc comment for the boundary sequence.
pub fn evaluate(ports: &Ports) -> Result<Outcome, OperationError> {
    let mut diagnostics = Vec::new();
    let mut git_unavailable: Option<Diagnostic> = None;

    let Some(yaml_raw) = read_optional(ports.reader, &wp(YAML_PATH))? else {
        diagnostics.push(fail(format!(
            "{YAML_PATH} is missing; the built-in task-type catalog is a mandatory part of this Kernel, not an optional add-on"
        )));
        return Ok(Outcome {
            diagnostics,
            catalog: None,
            git_unavailable,
        });
    };

    let mut ok = true;

    let registry_schema = match load_schema_text(ports.reader, REG_SCHEMA_PATH, "registry")? {
        Ok(schema) => Some(schema),
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            ok = false;
            None
        }
    };
    let envelope_schema = match load_schema_text(ports.reader, ENV_SCHEMA_PATH, "envelope")? {
        Ok(schema) => Some(schema),
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            ok = false;
            None
        }
    };

    if let (true, Some(registry_schema)) = (ok, &registry_schema) {
        let wk_enum = registry_schema
            .pointer("/definitions/payload/properties/work_kind/enum")
            .cloned();
        let cc_enum = registry_schema
            .pointer("/definitions/payload/properties/change_class/enum")
            .cloned();
        let expected_wk: Value = serde_json::json!(WORK_KINDS);
        let expected_cc: Value = serde_json::json!(CHANGE_CLASSES);
        if wk_enum.as_ref() != Some(&expected_wk) {
            diagnostics.push(fail(format!(
                "the schema's work_kind pool {} diverges from the canonical four (rule-resolution.md §2)",
                serde_json::to_string(&wk_enum).unwrap_or_default()
            )));
            ok = false;
        }
        if cc_enum.as_ref() != Some(&expected_cc) {
            diagnostics.push(fail(format!(
                "the schema's change_class pool {} diverges from the canonical four (rule-resolution.md §3)",
                serde_json::to_string(&cc_enum).unwrap_or_default()
            )));
            ok = false;
        }
    }

    let mut doc: Option<Value> = None;
    if ok {
        match parse_yaml(&yaml_raw) {
            Ok(parsed) => doc = Some(parsed),
            Err(error) => {
                diagnostics.push(fail(format!("cannot parse {YAML_PATH}: {error}")));
                ok = false;
            }
        }
    }

    // Exactly one Git snapshot for the whole registry route
    // (`rust-architecture-conformance-3` corrective round item 3):
    // `resolve_tracked_set` calls `GitInspector::tracked_files` here, once,
    // and nowhere else in this function or in `run_fixture_bundle`'s own use
    // of the same `tracked` value below.
    let tracked_snapshot = resolve_tracked_set(ports.git);
    let tracked: Option<HashSet<String>> = match &tracked_snapshot {
        TrackedSetResolution::Tracked(set) => Some(set.clone()),
        TrackedSetResolution::UnavailableFallback(warning) => {
            // Carried on `Outcome::git_unavailable`, not pushed into
            // `diagnostics` — see that field's own doc comment.
            git_unavailable = Some(warning.clone());
            None
        }
        TrackedSetResolution::Invalid(failure) => {
            diagnostics.push(failure.clone());
            ok = false;
            None
        }
    };

    // Only ever assigned `Some` once every gate below — external-link
    // checks, `rule-resolution.md` consistency, and the fixture bundle's own
    // self-consistency — has ALSO passed; see the single `catalog` binding
    // at the very end of this function.
    let mut built_catalog: Option<TaskPatternCatalog> = None;
    if let (true, Some(doc), Some(registry_schema), Some(envelope_schema)) =
        (ok, &doc, &registry_schema, &envelope_schema)
    {
        let (mut problems, catalog) = evaluate_document(doc, registry_schema, envelope_schema);
        if let Some(catalog) = &catalog {
            problems.extend(check_external_links(
                catalog,
                tracked.as_ref(),
                ports.link_target,
            ));
        }
        if problems.is_empty() {
            built_catalog = catalog;
        } else {
            ok = false;
        }
        if !problems.is_empty() {
            diagnostics.extend(problems.into_iter().take(10));
        }
    }
    // When `ok` was already `false` here (a schema/YAML-parse failure, or
    // Git's tracked-file snapshot being untrustworthy), the `if let` above
    // never matches: `evaluate_document` and `check_external_links` are not
    // run at all, so an untrustworthy Git snapshot never gets a filesystem-
    // existence fallback the way genuine Git unavailability does — it fails
    // the whole route closed instead (corrective round item 3).

    if let Some(rr_raw) = read_optional(ports.reader, &wp(RULE_RESOLUTION_PATH))? {
        let rr_problems = check_rule_resolution_bugfix_consistency(&rr_raw);
        if !rr_problems.is_empty() {
            ok = false;
        }
        diagnostics.extend(rr_problems);
    }

    let Some(fx_raw) = read_optional(ports.reader, &wp(FIXTURES_PATH))? else {
        diagnostics.push(fail(format!(
            "the mandatory catalog carries no fixtures ({FIXTURES_PATH}); a schema no run exercises is not one this gate has reached"
        )));
        return Ok(Outcome {
            diagnostics,
            catalog: None,
            git_unavailable,
        });
    };

    if !ok {
        return Ok(Outcome {
            diagnostics,
            catalog: None,
            git_unavailable,
        });
    }

    let bundle: Value = match serde_json::from_str(&fx_raw) {
        Ok(value) => value,
        Err(error) => {
            diagnostics.push(fail(format!(
                "the fixtures file is not valid JSON: {error}"
            )));
            return Ok(Outcome {
                diagnostics,
                catalog: None,
                git_unavailable,
            });
        }
    };

    let (Some(registry_schema), Some(envelope_schema)) = (&registry_schema, &envelope_schema)
    else {
        // `ok` is only kept `true` past the two schema-loading steps above
        // when both were successfully set to `Some`; this is unreachable in
        // practice, and returning a diagnostic rather than unwrapping keeps
        // it that way even if a future edit ever loosens that invariant.
        diagnostics.push(fail(
            "internal: schemas were unexpectedly unavailable after loading succeeded",
        ));
        return Ok(Outcome {
            diagnostics,
            catalog: None,
            git_unavailable,
        });
    };
    let fixture_problems = run_fixture_bundle(
        &bundle,
        registry_schema,
        envelope_schema,
        tracked.as_ref(),
        ports.link_target,
    );
    let fixtures_ok = fixture_problems.is_empty();
    diagnostics.extend(fixture_problems);

    // The one place `catalog` is ever `Some`: every gate above — schema,
    // DTO/domain construction, the catalogue-level cross-record rules,
    // external-link resolution, `rule-resolution.md` consistency, AND the
    // fixture bundle's own self-consistency — passed
    // (`rust-architecture-conformance-3` corrective round item 1).
    let catalog = if fixtures_ok { built_catalog } else { None };

    Ok(Outcome {
        diagnostics,
        catalog,
        git_unavailable,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation::mechanical_integrity::tests::FakeReader;
    use meridian_core::types::DiagnosticLevel;

    struct FakeGit {
        result: Result<Vec<WorkspaceRelativePath>, GitInspectorError>,
    }
    impl GitInspector for FakeGit {
        fn tracked_files(&self) -> Result<Vec<WorkspaceRelativePath>, GitInspectorError> {
            self.result.clone()
        }
    }

    struct FakeLinkTarget {
        results: std::collections::HashMap<String, Result<(), LinkTargetError>>,
    }
    impl LinkTargetPort for FakeLinkTarget {
        fn check(&self, path: &WorkspaceRelativePath) -> Result<(), LinkTargetError> {
            self.results.get(path.as_str()).cloned().unwrap_or(Ok(()))
        }
    }

    fn real_kernel_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }

    fn real_reader() -> crate::validation::mechanical_integrity::tests::RealFsReader {
        crate::validation::mechanical_integrity::tests::real_fs_reader(real_kernel_root())
    }

    struct RealGit {
        root: std::path::PathBuf,
    }
    impl GitInspector for RealGit {
        fn tracked_files(&self) -> Result<Vec<WorkspaceRelativePath>, GitInspectorError> {
            let output = std::process::Command::new("git")
                .arg("-C")
                .arg(&self.root)
                .args(["ls-files", "-z"])
                .output()
                .map_err(|_| GitInspectorError::Unavailable)?;
            if !output.status.success() {
                return Err(GitInspectorError::Unavailable);
            }
            let text =
                String::from_utf8(output.stdout).map_err(|_| GitInspectorError::Unavailable)?;
            let mut files = Vec::new();
            for raw in text.split('\0').filter(|s| !s.is_empty()) {
                let path = WorkspaceRelativePath::new(raw).map_err(|e| {
                    GitInspectorError::InvalidPath {
                        raw: raw.to_string(),
                        reason: e.to_string(),
                    }
                })?;
                files.push(path);
            }
            Ok(files)
        }
    }

    struct RealLinkTarget {
        root: std::path::PathBuf,
    }
    impl LinkTargetPort for RealLinkTarget {
        fn check(&self, path: &WorkspaceRelativePath) -> Result<(), LinkTargetError> {
            let root = self
                .root
                .canonicalize()
                .map_err(|e| LinkTargetError::Io(e.to_string()))?;
            let lexical = root.join(path.as_str());
            let real_target = match std::fs::canonicalize(&lexical) {
                Ok(p) => p,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Err(LinkTargetError::Missing)
                }
                Err(e) => return Err(LinkTargetError::Io(e.to_string())),
            };
            if !real_target.starts_with(&root) {
                return Err(LinkTargetError::SymlinkEscape);
            }
            let metadata =
                std::fs::metadata(&real_target).map_err(|e| LinkTargetError::Io(e.to_string()))?;
            if !metadata.is_file() {
                return Err(LinkTargetError::NotRegularFile);
            }
            Ok(())
        }
    }

    #[test]
    fn the_real_kernel_catalog_agrees_with_its_own_schema_and_fixtures() {
        let root = real_kernel_root();
        let reader = real_reader();
        let git = RealGit { root: root.clone() };
        let link_target = RealLinkTarget { root: root.clone() };
        let ports = Ports {
            reader: &reader,
            git: &git,
            link_target: &link_target,
        };
        let outcome = evaluate(&ports).unwrap();
        let failures: Vec<&str> = outcome
            .diagnostics
            .iter()
            .filter(|d| d.level() == DiagnosticLevel::Fail)
            .map(Diagnostic::message)
            .collect();
        assert!(failures.is_empty(), "{failures:?}");
        assert!(outcome.catalog.is_some());
        assert_eq!(outcome.catalog.unwrap().entries().len(), 7);
    }

    #[test]
    fn a_missing_yaml_file_fails_closed() {
        let reader = FakeReader::new();
        let git = FakeGit {
            result: Ok(Vec::new()),
        };
        let link_target = FakeLinkTarget {
            results: Default::default(),
        };
        let ports = Ports {
            reader: &reader,
            git: &git,
            link_target: &link_target,
        };
        let outcome = evaluate(&ports).unwrap();
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(outcome.diagnostics[0].message().contains("is missing"));
        assert!(outcome.catalog.is_none());
    }

    #[test]
    fn git_unavailable_falls_back_to_accepting_filesystem_existence_with_a_warning() {
        let root = real_kernel_root();
        let reader = real_reader();
        let git = FakeGit {
            result: Err(GitInspectorError::Unavailable),
        };
        let link_target = RealLinkTarget { root };
        let ports = Ports {
            reader: &reader,
            git: &git,
            link_target: &link_target,
        };
        let outcome = evaluate(&ports).unwrap();
        // The Git-unavailable fact is a typed field, not a `diagnostics`
        // entry a caller must scan for (`rust-architecture-conformance-3`
        // corrective round, "one normalized Git snapshot, no text-based
        // dedup") — a composed caller that already reports this once for a
        // whole run can simply not read it, with nothing to filter out of
        // `diagnostics`.
        let warning = outcome
            .git_unavailable
            .as_ref()
            .expect("git_unavailable must be set when GitInspector reports Unavailable");
        assert_eq!(warning.level(), DiagnosticLevel::Warn);
        assert!(
            warning.message().contains("Git enumeration unavailable"),
            "{}",
            warning.message()
        );
        assert!(
            !outcome
                .diagnostics
                .iter()
                .any(|d| d.message().contains("Git enumeration unavailable")),
            "the Git-unavailable diagnostic must not ALSO appear in the generic diagnostics list: {:?}",
            outcome
                .diagnostics
                .iter()
                .map(Diagnostic::message)
                .collect::<Vec<_>>()
        );
        let failures: Vec<&str> = outcome
            .diagnostics
            .iter()
            .filter(|d| d.level() == DiagnosticLevel::Fail)
            .map(Diagnostic::message)
            .collect();
        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn a_present_link_the_link_target_port_reports_missing_is_a_failure() {
        let mut results = std::collections::HashMap::new();
        results.insert(
            "workflows/task-lifecycle.md".to_string(),
            Err(LinkTargetError::Missing),
        );
        let root = real_kernel_root();
        let reader = real_reader();
        let git = RealGit { root: root.clone() };
        let link_target = FakeLinkTarget { results };
        let ports = Ports {
            reader: &reader,
            git: &git,
            link_target: &link_target,
        };
        let outcome = evaluate(&ports).unwrap();
        assert!(
            outcome
                .diagnostics
                .iter()
                .any(|d| d.message().contains("the target does not exist")),
            "{:?}",
            outcome
                .diagnostics
                .iter()
                .map(Diagnostic::message)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_present_link_reported_as_untracked_is_a_failure() {
        let reader = real_reader();
        let git = FakeGit {
            result: Ok(Vec::new()),
        };
        let link_target = FakeLinkTarget {
            results: Default::default(),
        };
        let ports = Ports {
            reader: &reader,
            git: &git,
            link_target: &link_target,
        };
        let outcome = evaluate(&ports).unwrap();
        assert!(
            outcome
                .diagnostics
                .iter()
                .any(|d| d.message().contains("not in the Kernel's tracked file set")),
            "{:?}",
            outcome
                .diagnostics
                .iter()
                .map(Diagnostic::message)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn a_directory_target_is_reported_as_not_a_regular_file() {
        let mut results = std::collections::HashMap::new();
        results.insert(
            "workflows/task-lifecycle.md".to_string(),
            Err(LinkTargetError::NotRegularFile),
        );
        let root = real_kernel_root();
        let reader = real_reader();
        let git = RealGit { root: root.clone() };
        let link_target = FakeLinkTarget { results };
        let ports = Ports {
            reader: &reader,
            git: &git,
            link_target: &link_target,
        };
        let outcome = evaluate(&ports).unwrap();
        assert!(
            outcome
                .diagnostics
                .iter()
                .any(|d| d.message().contains("is not a regular file")),
            "{:?}",
            outcome
                .diagnostics
                .iter()
                .map(Diagnostic::message)
                .collect::<Vec<_>>()
        );
        assert!(
            outcome.catalog.is_none(),
            "an external-link failure must leave the catalog None"
        );
    }

    #[test]
    fn a_symlink_escape_target_is_reported() {
        let mut results = std::collections::HashMap::new();
        results.insert(
            "workflows/task-lifecycle.md".to_string(),
            Err(LinkTargetError::SymlinkEscape),
        );
        let root = real_kernel_root();
        let reader = real_reader();
        let git = RealGit { root: root.clone() };
        let link_target = FakeLinkTarget { results };
        let ports = Ports {
            reader: &reader,
            git: &git,
            link_target: &link_target,
        };
        let outcome = evaluate(&ports).unwrap();
        assert!(
            outcome
                .diagnostics
                .iter()
                .any(|d| d.message().contains("outside the Kernel")),
            "{:?}",
            outcome
                .diagnostics
                .iter()
                .map(Diagnostic::message)
                .collect::<Vec<_>>()
        );
        assert!(
            outcome.catalog.is_none(),
            "an external-link failure must leave the catalog None"
        );
    }

    /// Corrective round item 3: Git reporting an invalid path is a
    /// data-integrity problem in Git's own output, not mere unavailability
    /// — it fails closed (`Fail`, not `Warn`), never falls back to
    /// accepting filesystem existence, and never yields a catalog, even
    /// though the real Kernel registry document itself is fully valid.
    #[test]
    fn an_invalid_path_from_git_output_fails_closed_without_fallback_or_catalog() {
        let reader = real_reader();
        let git = FakeGit {
            result: Err(GitInspectorError::InvalidPath {
                raw: "../escape".to_string(),
                reason: "workspace path must be relative, not absolute or drive-prefixed"
                    .to_string(),
            }),
        };
        let link_target = RealLinkTarget {
            root: real_kernel_root(),
        };
        let ports = Ports {
            reader: &reader,
            git: &git,
            link_target: &link_target,
        };
        let outcome = evaluate(&ports).unwrap();
        assert!(
            outcome
                .diagnostics
                .iter()
                .any(|d| d.level() == DiagnosticLevel::Fail
                    && d.message().contains("workspace-invalid tracked path")),
            "{:?}",
            outcome
                .diagnostics
                .iter()
                .map(Diagnostic::message)
                .collect::<Vec<_>>()
        );
        assert!(
            !outcome
                .diagnostics
                .iter()
                .any(|d| d.message().contains("Git enumeration unavailable")),
            "an invalid Git snapshot must not also claim Git was merely unavailable: {:?}",
            outcome
                .diagnostics
                .iter()
                .map(Diagnostic::message)
                .collect::<Vec<_>>()
        );
        assert!(
            outcome.catalog.is_none(),
            "an untrustworthy Git snapshot must never yield an accepted catalog"
        );
    }

    /// Corrective round item 3: `GitInspector::tracked_files` is called
    /// exactly once for the whole registry route, never once per canonical
    /// link and never a second time for the fixture bundle.
    #[test]
    fn the_whole_registry_route_calls_git_tracked_files_exactly_once() {
        struct CountingGit {
            calls: std::cell::Cell<u32>,
        }
        impl GitInspector for CountingGit {
            fn tracked_files(&self) -> Result<Vec<WorkspaceRelativePath>, GitInspectorError> {
                self.calls.set(self.calls.get() + 1);
                Ok(Vec::new())
            }
        }
        let root = real_kernel_root();
        let reader = real_reader();
        let git = CountingGit {
            calls: std::cell::Cell::new(0),
        };
        let link_target = RealLinkTarget { root };
        let ports = Ports {
            reader: &reader,
            git: &git,
            link_target: &link_target,
        };
        let _outcome = evaluate(&ports).unwrap();
        assert_eq!(git.calls.get(), 1);
    }

    /// A minimal [`WorkspaceReader`] delegating to a base reader for every
    /// path except a set of test-chosen overrides — used to isolate ONE
    /// failure class (a broken `rule-resolution.md`, or broken fixtures)
    /// against an otherwise fully valid, real Kernel registry, so a test can
    /// prove that failure class alone leaves the catalog `None`.
    struct OverrideReader<'a> {
        base: &'a dyn WorkspaceReader,
        overrides: std::collections::HashMap<&'static str, String>,
    }
    impl<'a> WorkspaceReader for OverrideReader<'a> {
        fn read_text(
            &self,
            path: &WorkspaceRelativePath,
        ) -> Result<String, crate::workspace::ReadError> {
            if let Some(text) = self.overrides.get(path.as_str()) {
                return Ok(text.clone());
            }
            self.base.read_text(path)
        }
        fn read_bytes(
            &self,
            path: &WorkspaceRelativePath,
        ) -> Result<Vec<u8>, crate::workspace::ReadError> {
            if let Some(text) = self.overrides.get(path.as_str()) {
                return Ok(text.clone().into_bytes());
            }
            self.base.read_bytes(path)
        }
        fn list_dir(
            &self,
            path: &WorkspaceRelativePath,
        ) -> Result<Vec<crate::workspace::DirEntry>, crate::workspace::ReadError> {
            self.base.list_dir(path)
        }
    }

    /// Corrective round item 1/4: a `rule-resolution.md` consistency
    /// failure leaves the catalog `None`, even though the registry document
    /// itself is fully valid.
    #[test]
    fn a_rule_resolution_failure_leaves_the_catalog_none() {
        let root = real_kernel_root();
        let real = real_reader();
        let mut overrides = std::collections::HashMap::new();
        overrides.insert(
            RULE_RESOLUTION_PATH,
            "BUGFIX -> bugfix-protocol is documented here as a route to a Kernel protocol."
                .to_string(),
        );
        let reader = OverrideReader {
            base: &real,
            overrides,
        };
        let git = RealGit { root: root.clone() };
        let link_target = RealLinkTarget { root };
        let ports = Ports {
            reader: &reader,
            git: &git,
            link_target: &link_target,
        };
        let outcome = evaluate(&ports).unwrap();
        assert!(
            outcome
                .diagnostics
                .iter()
                .any(|d| d.level() == DiagnosticLevel::Fail
                    && d.message().contains("rule-resolution.md")),
            "{:?}",
            outcome
                .diagnostics
                .iter()
                .map(Diagnostic::message)
                .collect::<Vec<_>>()
        );
        assert!(
            outcome.catalog.is_none(),
            "a rule-resolution.md consistency failure must leave the catalog None"
        );
    }

    /// Corrective round item 1/4: a fixture-bundle failure leaves the
    /// catalog `None`, even though the registry document itself is fully
    /// valid — a schema no run exercises is not one this gate has reached.
    #[test]
    fn a_fixture_bundle_failure_leaves_the_catalog_none() {
        let root = real_kernel_root();
        let real = real_reader();
        let mut overrides = std::collections::HashMap::new();
        overrides.insert(
            FIXTURES_PATH,
            r#"{"valid": [], "invalid": [{"registry": {}, "note": "x"}]}"#.to_string(),
        );
        let reader = OverrideReader {
            base: &real,
            overrides,
        };
        let git = RealGit { root: root.clone() };
        let link_target = RealLinkTarget { root };
        let ports = Ports {
            reader: &reader,
            git: &git,
            link_target: &link_target,
        };
        let outcome = evaluate(&ports).unwrap();
        assert!(
            outcome
                .diagnostics
                .iter()
                .any(|d| d.level() == DiagnosticLevel::Fail
                    && d.message().contains("no non-empty \"valid\" array")),
            "{:?}",
            outcome
                .diagnostics
                .iter()
                .map(Diagnostic::message)
                .collect::<Vec<_>>()
        );
        assert!(
            outcome.catalog.is_none(),
            "a fixture-bundle failure must leave the catalog None"
        );
    }

    /// Corrective round item 1/4: a container schema failure leaves the
    /// catalog `None` at the `evaluate_document` level.
    #[test]
    fn a_container_schema_failure_leaves_the_catalog_none() {
        let (registry_schema, envelope_schema, _fx_raw) = real_schemas_and_fixtures();
        let doc = serde_json::json!({ "task_patterns": "not-an-array" });
        let (problems, catalog) = evaluate_document(&doc, &registry_schema, &envelope_schema);
        assert!(!problems.is_empty(), "{problems:?}");
        assert!(catalog.is_none());
    }

    /// Corrective round item 1/4: a per-entry domain-construction failure
    /// (here, an unknown `work_kind`) leaves the catalog `None` even though
    /// the rest of the document parses.
    #[test]
    fn a_domain_construction_failure_leaves_the_catalog_none() {
        let (registry_schema, envelope_schema, fx_raw) = real_schemas_and_fixtures();
        let bundle: Value = serde_json::from_str(&fx_raw).unwrap();
        let mut doc = bundle["valid"][0]["registry"].clone();
        doc["task_patterns"][0]["payload"]["work_kind"] = Value::String("bogus".to_string());
        let (problems, catalog) = evaluate_document(&doc, &registry_schema, &envelope_schema);
        assert!(!problems.is_empty(), "{problems:?}");
        assert!(catalog.is_none());
    }

    #[test]
    fn duplicate_pattern_id_is_reported_through_the_full_pipeline() {
        let (registry_schema, envelope_schema, fx_raw) = real_schemas_and_fixtures();
        let bundle: Value = serde_json::from_str(&fx_raw).unwrap();
        let mut doc = bundle["valid"][0]["registry"].clone();
        let duplicate = doc["task_patterns"][0].clone();
        doc["task_patterns"].as_array_mut().unwrap().push(duplicate);
        let (problems, catalog) = evaluate_document(&doc, &registry_schema, &envelope_schema);
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("declared more than once")),
            "{problems:?}"
        );
        assert!(
            catalog.is_none(),
            "a catalogue-level defect (duplicate id) must leave the catalog None"
        );
    }

    fn real_schemas_and_fixtures() -> (Value, Value, String) {
        let root = real_kernel_root();
        let read = |rel: &str| std::fs::read_to_string(root.join(rel)).unwrap();
        let registry_schema: Value = serde_json::from_str(&read(REG_SCHEMA_PATH)).unwrap();
        let envelope_schema: Value = serde_json::from_str(&read(ENV_SCHEMA_PATH)).unwrap();
        let fx_raw = read(FIXTURES_PATH);
        (registry_schema, envelope_schema, fx_raw)
    }
}
