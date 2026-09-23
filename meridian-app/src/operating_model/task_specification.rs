//! `task-specification-contract` — app-owned orchestration for the
//! `rust-architecture-conformance-3` production route:
//!
//! ```text
//! WorkspaceReader
//!   -> private transport boundary (this module, over an already-parsed serde_json::Value)
//!   -> domain checks in meridian-core (meridian_core::task_contracts::specification)
//!   -> TaskSpecification, resolved against the ACCEPTED TaskPatternCatalog
//!   -> Vec<Diagnostic>
//!   -> existing CLI presentation
//! ```
//!
//! `registries/operating-model/task-specification.schema.json` is the
//! COMPLETE schema for one task specification (`record_type:
//! task-specification`): a record declares it in its own `$schema`, so a
//! consumer that follows the declaration validates the whole contract — the
//! reused scoped-record envelope AND the specialised body — in one pass.
//! Specification DATA is Instance, like the intake register and the
//! instruction source registry — the Kernel ships the schema, the
//! product-neutral fixtures and this module.
//!
//! This module's transport boundary is a set of small `&Value ->
//! Option<&str>`/`Vec<...>` PROJECTIONS (never a closed `serde` DTO struct,
//! because a task specification's shape is deeply nested and this
//! projection needs to tolerate a schema-invalid document without ever
//! panicking, exactly like the pure `Value`-walking the prior port used) —
//! `serde_json::Value` never crosses into [`meridian_core`]. Every
//! structural AND free-text-portability domain check runs in
//! [`meridian_core::task_contracts::specification::check_task_specification`]
//! (`rust-architecture-conformance-3` corrective round item 2): this module
//! only extracts the primitive fields and hands them to that ONE
//! constructor. The one portability check that stays here is the
//! `$schema`-envelope declaration itself
//! (`crate::operating_model::reference_portability::non_portable_reason`/
//! `resolve_schema_ref`, both thin re-exports of
//! `meridian_core::task_contracts::portability` now) — `$schema` is not a
//! [`meridian_core::task_contracts::SpecificationFields`] domain field, so
//! it is checked at this transport/envelope boundary instead.
//!
//! **Critically**, this module does NOT read or parse
//! `standards/workspace/task-pattern-registry.yaml` itself: the
//! `task_pattern` reference is resolved against the
//! [`meridian_core::task_contracts::TaskPatternCatalog`] the caller
//! (`meridian-cli`'s composition, via [`super::task_pattern_registry`]'s own
//! production route) already built and passes in.

use serde_json::Value;

use meridian_core::resolver::{ChangeClass, WorkKind};
use meridian_core::task_contracts::catalog::TaskPatternCatalog;
use meridian_core::task_contracts::specification::{
    check_task_specification, spec_id_label, AcceptanceCriterionInput, DeclaredPattern,
    SpecificationFields, TaskSpecification,
};
use meridian_core::types::{Diagnostic, DiagnosticLevel};

use crate::operating_model::reference_portability;
use crate::source_format::json_schema;
use crate::validation::mechanical_integrity::OperationError;
use crate::workspace::{ReadError, WorkspaceReader};

// Re-exported facade (`rust-architecture-conformance-3` §5.17.3, point 6):
// `non_portable_reason`/`resolve_schema_ref` used to be DEFINED in this
// module; they moved to the neutral
// `crate::operating_model::reference_portability` owner, which this module
// itself now also imports (above). Two neighbouring modules —
// `super::evidence_and_handoff` and `super::field_evaluation` — still import
// both functions from `super::task_specification` (`use
// super::task_specification::{non_portable_reason, resolve_schema_ref};`);
// this re-export keeps those `use` lines working unchanged
// (`rust-architecture-conformance-5` moved the other three former
// consumers into `meridian_core::run_contracts`). No NEW consumer may be
// added through this facade — the package that types those two families
// should switch their import and delete this re-export.
pub use reference_portability::{non_portable_reason, resolve_schema_ref};

const SCHEMA_NAMESPACE_DIR: &str = "registries/operating-model";
const EXPECTED_SCHEMA_BASENAME: &str = "task-specification.schema.json";
const ENVELOPE_SCHEMA_BASENAME: &str = "scoped-record.schema.json";

fn expected_schema_ref() -> String {
    format!("{SCHEMA_NAMESPACE_DIR}/{EXPECTED_SCHEMA_BASENAME}")
}
fn envelope_schema_ref() -> String {
    format!("{SCHEMA_NAMESPACE_DIR}/{ENVELOPE_SCHEMA_BASENAME}")
}

fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message).expect("message is non-empty")
}

fn parse_work_kind(s: &str) -> Option<WorkKind> {
    match s {
        "assessment" => Some(WorkKind::Assessment),
        "operation" => Some(WorkKind::Operation),
        "initiative" => Some(WorkKind::Initiative),
        "change" => Some(WorkKind::Change),
        _ => None,
    }
}

fn parse_change_class(s: &str) -> Option<ChangeClass> {
    match s {
        "BUGFIX" => Some(ChangeClass::Bugfix),
        "FEATURE" => Some(ChangeClass::Feature),
        "BEHAVIOR_CHANGE" => Some(ChangeClass::BehaviorChange),
        "REFACTOR" => Some(ChangeClass::Refactor),
        _ => None,
    }
}

/// A closed, minimal projection of `&Value -> Option<&str>`: `container` is
/// itself optional (a missing/non-object parent collapses cleanly to
/// `None`, never a panic or a borrowed temporary — the reason this is a
/// free function taking `Option<&'a Value>` rather than falling back to a
/// locally-constructed empty object, which could not outlive this
/// function).
fn field<'a>(container: Option<&'a Value>, key: &str) -> Option<&'a str> {
    container?.get(key)?.as_str()
}

fn obj<'a>(container: Option<&'a Value>, key: &str) -> Option<&'a Value> {
    container?.get(key).filter(|v| v.is_object())
}

/// One task specification's transport-boundary projection: every field the
/// domain checks and the portability scan need, extracted once from the
/// already-schema-validated (or not — extraction never assumes validity)
/// `Value`.
struct Extracted<'a> {
    id: Option<&'a str>,
    schema_declared: Option<&'a str>,
    record_type: &'a str,
    title: Option<&'a str>,
    scope_type: Option<&'a str>,
    origin_kind: Option<&'a str>,
    origin_source_ref: Option<&'a str>,
    authority_ref: Option<&'a str>,
    decision_ref: Option<&'a str>,
    goal: Option<&'a str>,
    initial_state: Option<&'a str>,
    target_model: Option<&'a str>,
    constraints: Vec<String>,
    acceptance_criteria: Vec<AcceptanceCriterionInput<'a>>,
    payload_keys: Vec<String>,
    declared_pattern: Option<DeclaredPattern<'a>>,
}

fn extract(doc: &Value) -> Extracted<'_> {
    let root = Some(doc);
    let scope = obj(root, "scope");
    let origin = obj(root, "origin");
    let authority = obj(root, "authority");
    let payload = obj(root, "payload");

    let constraints: Vec<String> = payload
        .and_then(|p| p.get("constraints"))
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default();

    let criteria_values: &[Value] = payload
        .and_then(|p| p.get("acceptance_criteria"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let acceptance_criteria: Vec<AcceptanceCriterionInput> = criteria_values
        .iter()
        .map(|c| {
            let verification = obj(Some(c), "verification");
            AcceptanceCriterionInput {
                id: field(Some(c), "id"),
                statement: field(Some(c), "statement"),
                verification_method: field(verification, "method"),
                verification_expected_result: field(verification, "expected_result"),
            }
        })
        .collect();

    let payload_keys: Vec<String> = payload
        .and_then(Value::as_object)
        .map(|o| o.keys().cloned().collect())
        .unwrap_or_default();

    let tp = obj(payload, "task_pattern");
    let declared_pattern = tp.map(|tp| DeclaredPattern {
        id: field(Some(tp), "id"),
        work_kind: field(Some(tp), "work_kind").and_then(parse_work_kind),
        change_class_present: tp.get("change_class").is_some(),
        change_class: field(Some(tp), "change_class").and_then(parse_change_class),
    });

    Extracted {
        id: field(root, "id"),
        schema_declared: field(root, "$schema"),
        record_type: field(root, "record_type").unwrap_or(""),
        title: field(root, "title"),
        scope_type: field(scope, "type"),
        origin_kind: field(origin, "kind"),
        origin_source_ref: field(origin, "source_ref"),
        authority_ref: field(authority, "authority_ref"),
        decision_ref: field(authority, "decision_ref"),
        goal: field(payload, "goal"),
        initial_state: field(payload, "initial_state"),
        target_model: field(payload, "target_model"),
        constraints,
        acceptance_criteria,
        payload_keys,
        declared_pattern,
    }
}

/// The `$schema` declaration must resolve, within the Meridian namespace
/// from the canonical logical base, to `task-specification.schema.json` —
/// not merely a matching basename, and not the bare record envelope. Port
/// of the `$schema`-specific half of `evaluateTaskSpecification`.
fn check_schema_declaration(id_label: &str, declared: Option<&str>) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    match declared {
        None => problems.push(fail(format!(
            "task specification \"{id_label}\" declares no $schema; a record names {EXPECTED_SCHEMA_BASENAME} so a consumer validates the whole contract, not only the envelope"
        ))),
        Some(declared) => {
            if let Some(portability) = reference_portability::non_portable_reason(Some(declared)) {
                problems.push(fail(format!(
                    "task specification \"{id_label}\" $schema \"{declared}\" is not portable ({portability}); the schema is named by a relative reference resolved from the canonical logical resolution base ({})",
                    reference_portability::CANONICAL_RECORD_BASE
                )));
            } else {
                let resolved = reference_portability::resolve_schema_ref(Some(declared));
                if resolved.as_deref() == Some(envelope_schema_ref().as_str()) {
                    problems.push(fail(format!(
                        "task specification \"{id_label}\" $schema \"{declared}\" resolves to the record envelope ({ENVELOPE_SCHEMA_BASENAME}); it must name {EXPECTED_SCHEMA_BASENAME}, which composes the envelope with the body"
                    )));
                } else if resolved.as_deref() != Some(expected_schema_ref().as_str()) {
                    let expected = expected_schema_ref();
                    problems.push(fail(format!(
                        "task specification \"{id_label}\" $schema \"{declared}\" does not resolve to the logical address {expected} from the canonical logical resolution base ({}); the specialised schema is named by a relative reference written from that base",
                        reference_portability::CANONICAL_RECORD_BASE
                    )));
                }
            }
        }
    }
    problems
}

/// The whole in-memory composition pipeline for one task-specification
/// document: the canonical record envelope (always run), the complete
/// specialised schema, the `$schema` resolution check, and the domain
/// checks — structural, free-text portability, AND `task_pattern`
/// resolution against `catalog`, ALL of them now
/// [`check_task_specification`]'s own gate
/// (`rust-architecture-conformance-3` corrective round item 2).
///
/// **Fail-closed**: the returned [`TaskSpecification`] is `Some` if and
/// only if the COMBINED diagnostic list — envelope schema, specialised
/// schema, `$schema` resolution, AND `check_task_specification`'s own
/// (structural + portability + pattern-resolution) gate — is empty.
/// `check_task_specification`'s `Some` already means portability was
/// checked; this function only ADDS the envelope/schema-level diagnostics
/// that gate is not itself responsible for, and re-clears `resolved` to
/// `None` whenever any of those additional diagnostics exist.
///
/// Not `pub`: this function is an internal step of [`evaluate`], never
/// called across a crate or module boundary, so `&Value` never needs to
/// appear in a signature outside this module's own private transport
/// boundary.
fn evaluate_document(
    doc: &Value,
    record_schema: &Value,
    envelope_schema: &Value,
    catalog: &TaskPatternCatalog,
) -> (Vec<Diagnostic>, Option<TaskSpecification>) {
    let mut problems = Vec::new();

    match json_schema::validate(doc, envelope_schema) {
        Ok(errors) => problems.extend(errors.into_iter().map(|m| fail(format!("envelope {m}")))),
        Err(error) => {
            return (
                vec![fail(format!(
                    "record envelope schema could not be applied: {error}"
                ))],
                None,
            )
        }
    }
    match json_schema::validate(doc, record_schema) {
        Ok(errors) => problems.extend(errors.into_iter().map(fail)),
        Err(error) => {
            return (
                vec![fail(format!(
                    "task-specification schema could not be applied: {error}"
                ))],
                None,
            )
        }
    }
    if !doc.is_object() {
        if problems.is_empty() {
            return (vec![fail("the task specification is not an object")], None);
        }
        return (problems, None);
    }

    let extracted = extract(doc);
    let id_label = spec_id_label(extracted.id).to_string();

    problems.extend(check_schema_declaration(
        &id_label,
        extracted.schema_declared,
    ));

    let fields = SpecificationFields {
        id: extracted.id,
        record_type: extracted.record_type,
        title: extracted.title,
        scope_type: extracted.scope_type,
        origin_kind: extracted.origin_kind,
        origin_source_ref: extracted.origin_source_ref,
        authority_ref: extracted.authority_ref,
        decision_ref: extracted.decision_ref,
        goal: extracted.goal,
        initial_state: extracted.initial_state,
        target_model: extracted.target_model,
        constraints: &extracted.constraints,
        acceptance_criteria: &extracted.acceptance_criteria,
        payload_keys: &extracted.payload_keys,
        declared_pattern: extracted.declared_pattern,
    };
    // `check_task_specification` already covers structural rules, free-text
    // portability over every domain field above, AND `task_pattern`
    // resolution — its own `Some` already means all of that passed
    // (`rust-architecture-conformance-3` corrective round item 2).
    let (domain_problems, resolved) = check_task_specification(&fields, catalog);
    problems.extend(domain_problems);

    // `resolved` is `check_task_specification`'s own verdict, already
    // including portability; it is only handed on here once `problems`
    // (envelope + specialised schema + `$schema` resolution + everything
    // `check_task_specification` itself checked) is confirmed empty — an
    // unresolved `$schema` must still yield `None` even though the domain
    // gate by itself was satisfied.
    let resolved = if problems.is_empty() { resolved } else { None };

    (problems, resolved)
}

const SCHEMA_PATH: &str = "registries/operating-model/task-specification.schema.json";
const ENVELOPE_SCHEMA_PATH: &str = "registries/operating-model/scoped-record.schema.json";
const FIXTURES_PATH: &str = "registries/operating-model/fixtures/task-specification.fixtures.json";

fn wp(literal: &str) -> meridian_core::types::WorkspaceRelativePath {
    meridian_core::types::WorkspaceRelativePath::new(literal)
        .expect("literal path constant is a valid workspace path")
}

fn read_optional(
    reader: &dyn WorkspaceReader,
    path: &meridian_core::types::WorkspaceRelativePath,
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

pub struct Outcome {
    pub diagnostics: Vec<Diagnostic>,
}

/// The whole `task-specification-contract` production route. `catalog` is
/// the [`TaskPatternCatalog`] the caller already built through
/// [`super::task_pattern_registry`]'s own production route — this function
/// never reads or parses `task-pattern-registry.yaml` again.
pub fn evaluate(
    reader: &dyn WorkspaceReader,
    catalog: &TaskPatternCatalog,
) -> Result<Outcome, OperationError> {
    let mut diagnostics = Vec::new();

    let Some(schema_raw) = read_optional(reader, &wp(SCHEMA_PATH))? else {
        diagnostics.push(fail(format!(
            "{SCHEMA_PATH} is missing; the task specification contract is a mandatory part of this Kernel, not an optional add-on"
        )));
        return Ok(Outcome { diagnostics });
    };

    let mut ok = true;
    let mut schema: Option<Value> = None;
    match serde_json::from_str::<Value>(&schema_raw) {
        Ok(parsed) => schema = Some(parsed),
        Err(error) => {
            diagnostics.push(fail(format!("{SCHEMA_PATH} is not valid JSON: {error}")));
            ok = false;
        }
    }

    let mut envelope: Option<Value> = None;
    match read_optional(reader, &wp(ENVELOPE_SCHEMA_PATH))? {
        Some(raw) => match serde_json::from_str::<Value>(&raw) {
            Ok(parsed) => envelope = Some(parsed),
            Err(error) => {
                diagnostics.push(fail(format!(
                    "{ENVELOPE_SCHEMA_PATH} is not valid JSON: {error}"
                )));
                ok = false;
            }
        },
        None => {
            diagnostics.push(fail(format!(
                "{ENVELOPE_SCHEMA_PATH} is missing; the specification composes with the record envelope and cannot be checked without it"
            )));
            ok = false;
        }
    }

    if let Some(schema) = &schema {
        if let Err(error) = json_schema::assert_supported_deep(schema, SCHEMA_PATH) {
            diagnostics.push(fail(format!(
                "the schema uses a construct this validator cannot check: {error}"
            )));
            ok = false;
        }
    }

    let Some(fx_raw) = read_optional(reader, &wp(FIXTURES_PATH))? else {
        diagnostics.push(fail(format!(
            "the schema carries no fixtures ({FIXTURES_PATH}); a schema no run exercises is not one this gate has reached"
        )));
        return Ok(Outcome { diagnostics });
    };
    if !ok {
        return Ok(Outcome { diagnostics });
    }

    let bundle: Value = match serde_json::from_str(&fx_raw) {
        Ok(value) => value,
        Err(error) => {
            diagnostics.push(fail(format!(
                "the fixtures file is not valid JSON: {error}"
            )));
            return Ok(Outcome { diagnostics });
        }
    };
    if !bundle.is_object() {
        diagnostics.push(fail(
            "the fixtures file must be an object with non-empty \"valid\" and \"invalid\" arrays",
        ));
        return Ok(Outcome { diagnostics });
    }
    let mut bundle_ok = true;
    for key in ["valid", "invalid"] {
        let is_nonempty = bundle
            .get(key)
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty());
        if !is_nonempty {
            bundle_ok = false;
            diagnostics.push(fail(format!(
                "the fixtures file has no non-empty \"{key}\" array"
            )));
        }
    }
    if !bundle_ok {
        return Ok(Outcome { diagnostics });
    }

    let (Some(schema), Some(envelope)) = (&schema, &envelope) else {
        // `ok` is only kept `true` past the two schema-loading steps above
        // when both were successfully set to `Some`; unreachable in
        // practice, kept as an explicit diagnostic rather than an unwrap so
        // a future edit that loosens that invariant fails closed here
        // instead of panicking.
        diagnostics.push(fail(
            "internal: schemas were unexpectedly unavailable after loading succeeded",
        ));
        return Ok(Outcome { diagnostics });
    };

    let empty: Vec<Value> = Vec::new();
    for case in bundle
        .get("valid")
        .and_then(Value::as_array)
        .unwrap_or(&empty)
    {
        let spec = case.get("spec").cloned().unwrap_or(Value::Null);
        let (problems, resolved) = evaluate_document(&spec, schema, envelope, catalog);
        if !problems.is_empty() {
            diagnostics.push(fail(format!(
                "a fixture that must be a valid specification was rejected ({}): {}",
                case.get("note").and_then(Value::as_str).unwrap_or(""),
                problems[0].message()
            )));
        } else if resolved.is_none() {
            // A valid fixture is not merely "no diagnostics" — it must also
            // resolve to the typed `TaskSpecification`
            // (`rust-architecture-conformance-3` corrective round item 2):
            // an empty problem list with no resolved value would mean this
            // fixture never actually exercised the typed construction path.
            diagnostics.push(fail(format!(
                "a fixture that must be a valid specification produced no diagnostics but did not resolve to a typed TaskSpecification ({})",
                case.get("note").and_then(Value::as_str).unwrap_or("")
            )));
        }
    }
    for case in bundle
        .get("invalid")
        .and_then(Value::as_array)
        .unwrap_or(&empty)
    {
        let spec = case.get("spec").cloned().unwrap_or(Value::Null);
        let (problems, resolved) = evaluate_document(&spec, schema, envelope, catalog);
        if problems.is_empty() {
            diagnostics.push(fail(format!(
                "a fixture that must be rejected validated clean ({})",
                case.get("note").and_then(Value::as_str).unwrap_or("")
            )));
        } else if resolved.is_some() {
            // A rejected fixture must never carry a resolved typed value
            // alongside its diagnostics — `check_task_specification`'s own
            // gate already guarantees this, but the fixture loop asserts it
            // too, so any future change that weakens that guarantee is
            // caught here, over real fixture documents.
            diagnostics.push(fail(format!(
                "a fixture that must be rejected produced diagnostics but still resolved to a typed TaskSpecification ({})",
                case.get("note").and_then(Value::as_str).unwrap_or("")
            )));
        }
    }

    Ok(Outcome { diagnostics })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation::mechanical_integrity::tests::{real_fs_reader, FakeReader};
    use meridian_core::resolver::WorkItemKind;
    use meridian_core::task_contracts::catalog::{
        CanonicalLink, TaskPatternEntry, BUGFIX_SKILL, REFACTOR_EVIDENCE, REFACTOR_PROTOCOL,
    };
    use meridian_core::types::{NonEmptyString, SemanticId, WorkspaceRelativePath};

    fn real_kernel_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }

    fn real_task_pattern_catalog() -> TaskPatternCatalog {
        let root = real_kernel_root();
        let reader = real_fs_reader(root.clone());
        let git = RealGitForTests { root: root.clone() };
        let link_target = RealLinkTargetForTests { root };
        let ports = super::super::task_pattern_registry::Ports {
            reader: &reader,
            git: &git,
            link_target: &link_target,
        };
        super::super::task_pattern_registry::evaluate(&ports)
            .unwrap()
            .catalog
            .expect("real catalog builds")
    }

    struct RealGitForTests {
        root: std::path::PathBuf,
    }
    impl crate::workspace::GitInspector for RealGitForTests {
        fn tracked_files(
            &self,
        ) -> Result<
            Vec<meridian_core::types::WorkspaceRelativePath>,
            crate::workspace::GitInspectorError,
        > {
            let output = std::process::Command::new("git")
                .arg("-C")
                .arg(&self.root)
                .args(["ls-files", "-z"])
                .output()
                .map_err(|_| crate::workspace::GitInspectorError::Unavailable)?;
            if !output.status.success() {
                return Err(crate::workspace::GitInspectorError::Unavailable);
            }
            let text = String::from_utf8(output.stdout)
                .map_err(|_| crate::workspace::GitInspectorError::Unavailable)?;
            let mut files = Vec::new();
            for raw in text.split('\0').filter(|s| !s.is_empty()) {
                let path = meridian_core::types::WorkspaceRelativePath::new(raw).map_err(|e| {
                    crate::workspace::GitInspectorError::InvalidPath {
                        raw: raw.to_string(),
                        reason: e.to_string(),
                    }
                })?;
                files.push(path);
            }
            Ok(files)
        }
    }

    struct RealLinkTargetForTests {
        root: std::path::PathBuf,
    }
    impl crate::workspace::LinkTargetPort for RealLinkTargetForTests {
        fn check(
            &self,
            path: &meridian_core::types::WorkspaceRelativePath,
        ) -> Result<(), crate::workspace::LinkTargetError> {
            use crate::workspace::LinkTargetError;
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
    fn the_real_kernel_schema_and_fixtures_agree() {
        let root = real_kernel_root();
        let reader = real_fs_reader(root);
        let catalog = real_task_pattern_catalog();
        let outcome = evaluate(&reader, &catalog).unwrap();
        let failures: Vec<&str> = outcome
            .diagnostics
            .iter()
            .filter(|d| d.level() == meridian_core::types::DiagnosticLevel::Fail)
            .map(Diagnostic::message)
            .collect();
        assert!(failures.is_empty(), "{failures:?}");
    }

    #[test]
    fn a_missing_schema_file_fails_closed() {
        let reader = FakeReader::new();
        let catalog = assessment_catalog();
        let outcome = evaluate(&reader, &catalog).unwrap();
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(outcome.diagnostics[0].message().contains("is missing"));
    }

    fn present(id: &str, path: &str) -> CanonicalLink {
        CanonicalLink::Present {
            id: SemanticId::new(id).unwrap(),
            path: WorkspaceRelativePath::new(path).unwrap(),
        }
    }

    fn absent(reason: &str) -> CanonicalLink {
        CanonicalLink::Absent {
            reason: NonEmptyString::new(reason).unwrap(),
        }
    }

    fn simple_entry(id: &str, kind: WorkItemKind) -> TaskPatternEntry {
        TaskPatternEntry::try_new(
            SemanticId::new(id).unwrap(),
            kind,
            vec![present(
                "standard-task-lifecycle",
                "workflows/task-lifecycle.md",
            )],
            vec![absent("no vendored way of carrying this out")],
            vec![absent("no canonical evidence contract yet")],
        )
        .unwrap()
    }

    /// A full, defect-free seven-pattern catalogue, built through the real
    /// (now fail-closed — `rust-architecture-conformance-3` corrective
    /// round item 1) [`TaskPatternCatalog::build`]. This module's own tests
    /// only ever resolve `task_pattern.id: "assess-existing-state"` against
    /// it; the other six entries exist purely because `build` refuses to
    /// construct a catalogue missing any of the seven mandatory pairs —
    /// there is no crate-local test-only shortcut available here the way
    /// `meridian-core`'s own `task_contracts::specification` tests have,
    /// since this is a different crate.
    fn assessment_catalog() -> TaskPatternCatalog {
        let entries = vec![
            simple_entry("assess-existing-state", WorkItemKind::Assessment),
            simple_entry("operate-environment-action", WorkItemKind::Operation),
            simple_entry("decompose-initiative", WorkItemKind::Initiative),
            TaskPatternEntry::try_new(
                SemanticId::new("fix-defect").unwrap(),
                WorkItemKind::Change(ChangeClass::Bugfix),
                vec![absent("no separate protocol document")],
                vec![present("bugfix-protocol", BUGFIX_SKILL)],
                vec![present(
                    "regression-evidence-record",
                    "verification/regression-testing/README.md",
                )],
            )
            .unwrap(),
            simple_entry("add-capability", WorkItemKind::Change(ChangeClass::Feature)),
            simple_entry(
                "change-behavior",
                WorkItemKind::Change(ChangeClass::BehaviorChange),
            ),
            TaskPatternEntry::try_new(
                SemanticId::new("refactor-preserving-behavior").unwrap(),
                WorkItemKind::Change(ChangeClass::Refactor),
                vec![present("refactor-execution-protocol", REFACTOR_PROTOCOL)],
                vec![absent("no separate vendored way of carrying this out")],
                vec![present(
                    "functional-parity-evidence-contract",
                    REFACTOR_EVIDENCE,
                )],
            )
            .unwrap(),
        ];
        let (catalog, problems) = TaskPatternCatalog::build(entries);
        assert!(problems.is_empty(), "{problems:?}");
        catalog.expect("all seven canonical patterns build cleanly")
    }

    fn well_formed_spec() -> Value {
        serde_json::json!({
            "$schema": "../../registries/operating-model/task-specification.schema.json",
            "schema_version": 1,
            "id": "sample-spec",
            "title": "Пример спецификации",
            "record_type": "task-specification",
            "scope": {"type": "repository-scope", "id": "sample-repository", "workspace_id": "sample-workspace"},
            "origin": {"kind": "declared", "source_ref": "sources/example"},
            "authority": {"kind": "repository-maintainer", "authority_ref": "repository-maintainer"},
            "payload": {
                "goal": "goal text",
                "initial_state": "initial state text",
                "target_model": "target model text",
                "task_pattern": {"id": "assess-existing-state", "work_kind": "assessment"},
                "constraints": ["one constraint"],
                "acceptance_criteria": [
                    {"id": "criterion-one", "statement": "statement", "verification": {"method": "method", "expected_result": "expected"}}
                ],
            },
        })
    }

    fn permissive_schema() -> Value {
        serde_json::json!({"type": "object"})
    }

    #[test]
    fn a_well_formed_specification_evaluates_cleanly_against_a_real_catalog() {
        let catalog = assessment_catalog();
        let spec = well_formed_spec();
        let (problems, _resolved) =
            evaluate_document(&spec, &permissive_schema(), &permissive_schema(), &catalog);
        assert!(problems.is_empty(), "{problems:?}");
    }

    /// Corrective round item 2: a fully valid specification resolves to the
    /// typed [`TaskSpecification`] value, not merely an empty diagnostic
    /// list — the typed value is the thing the plan requires to actually
    /// exist, not be discarded.
    #[test]
    fn a_well_formed_specification_resolves_to_the_typed_value() {
        let catalog = assessment_catalog();
        let spec = well_formed_spec();
        let (problems, resolved) =
            evaluate_document(&spec, &permissive_schema(), &permissive_schema(), &catalog);
        assert!(problems.is_empty(), "{problems:?}");
        let resolved = resolved.expect("a fully valid specification resolves to a typed value");
        assert_eq!(resolved.id().as_str(), "sample-spec");
        assert_eq!(resolved.kind(), WorkItemKind::Assessment);
    }

    #[test]
    fn a_rooted_path_in_the_goal_is_rejected() {
        let catalog = assessment_catalog();
        let mut spec = well_formed_spec();
        spec["payload"]["goal"] = Value::String("see /etc/passwd for details".to_string());
        let (problems, resolved) =
            evaluate_document(&spec, &permissive_schema(), &permissive_schema(), &catalog);
        assert!(
            problems.iter().any(|p| p.message().contains("rooted")),
            "{problems:?}"
        );
        assert!(
            resolved.is_none(),
            "a portability violation must prevent the typed specification from being returned, even though the structural gate alone was satisfied"
        );
    }

    #[test]
    fn schema_absent_is_rejected() {
        let catalog = assessment_catalog();
        let mut spec = well_formed_spec();
        spec.as_object_mut().unwrap().remove("$schema");
        let (problems, resolved) =
            evaluate_document(&spec, &permissive_schema(), &permissive_schema(), &catalog);
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("declares no $schema")),
            "{problems:?}"
        );
        assert!(
            resolved.is_none(),
            "a missing $schema declaration must prevent the typed specification from being returned"
        );
    }

    #[test]
    fn schema_pointing_only_at_the_envelope_is_rejected() {
        let catalog = assessment_catalog();
        let mut spec = well_formed_spec();
        spec["$schema"] =
            Value::String("../../registries/operating-model/scoped-record.schema.json".to_string());
        let (problems, resolved) =
            evaluate_document(&spec, &permissive_schema(), &permissive_schema(), &catalog);
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("resolves to the record envelope")),
            "{problems:?}"
        );
        assert!(resolved.is_none());
    }

    #[test]
    fn task_specification_never_reparses_the_registry_yaml() {
        let production = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/operating_model/task_specification.rs"
        ))
        .unwrap();
        let production_end = production.find("#[cfg(test)]").unwrap_or(production.len());
        let code_only: String = production[..production_end]
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !code_only.contains("task-pattern-registry.yaml"),
            "task_specification.rs must never read task-pattern-registry.yaml itself"
        );
    }

    /// Structural gate (`rust-architecture-conformance-3` §5.17.3, point 6):
    /// the temporary `non_portable_reason`/`resolve_schema_ref` re-export
    /// facade this module carries has EXACTLY the documented consumers
    /// (this module's own doc comment) — no new one may be added
    /// through it. A future package that switches any of these five
    /// imports to `crate::operating_model::reference_portability` directly
    /// should shrink this list, and once it is empty the facade
    /// (`pub use reference_portability::{non_portable_reason,
    /// resolve_schema_ref};`) should be deleted.
    #[test]
    fn the_facade_re_export_has_no_new_consumers() {
        let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/operating_model");
        // `rust-architecture-conformance-5` removed the three run-contract
        // families from this list: their domain now imports
        // `meridian_core::task_contracts` directly.
        let known_consumers = ["evidence_and_handoff.rs", "field_evaluation.rs"];
        let entries = std::fs::read_dir(&src_dir).unwrap();
        for entry in entries {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let name = path.file_name().unwrap().to_str().unwrap().to_string();
            if name == "task_specification.rs" {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            let imports_facade = text.contains("task_specification::{non_portable_reason")
                || text.contains("task_specification::{resolve_schema_ref")
                || text.contains("task_specification::non_portable_reason")
                || text.contains("task_specification::resolve_schema_ref");
            if imports_facade {
                assert!(
                    known_consumers.contains(&name.as_str()),
                    "{name} imports the non_portable_reason/resolve_schema_ref facade but is not one of the documented consumers — either it is a genuinely new facade consumer (not allowed by this package) or the documented list is stale"
                );
            }
        }
    }
}
