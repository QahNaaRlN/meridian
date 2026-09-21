//! Verbatim port of `scripts/lib/task-specification.mjs`: the
//! composite-consistency algorithm behind the `task-specification-contract`
//! check. `registries/operating-model/task-specification.schema.json` is
//! the COMPLETE schema for one task specification (`record_type:
//! task-specification`): a record declares it in its own `$schema`, so a
//! consumer that follows the declaration validates the whole contract — the
//! reused scoped-record envelope AND the specialised body — in one pass.
//! Specification DATA is Instance, like the intake register and the
//! instruction source registry — the Kernel ships the schema, the
//! product-neutral fixtures and this module.
//!
//! Nothing here reads process state, touches the filesystem or exits: it
//! takes a parsed record, the two schemas and the built-in task-pattern
//! list (`{ id, work_kind, change_class }`), and returns a flat list of
//! problem strings — empty means valid.
//!
//! Boundaries this module enforces that the JSON Schema subset cannot:
//!   - the record's `$schema` declaration is a portable relative reference
//!     that RESOLVES, within the Meridian namespace from the canonical
//!     logical base, to `task-specification.schema.json` — not merely a
//!     matching basename, and not the bare record envelope. The base is a
//!     logical address, not a filesystem directory, a repository or a
//!     storage mechanism;
//!   - the canonical scoped-record envelope is validated SEPARATELY and is
//!     never dropped, even though the specialised schema re-states its
//!     shape;
//!   - a project task specification lives only in project-workspace or
//!     repository-scope; user-profile, organization-profile, run-state and
//!     built-in-methodology are rejected with the rationale;
//!   - the envelope reference strings (`origin.source_ref`,
//!     `authority.authority_ref` and, when present,
//!     `authority.decision_ref`) are portable too: no rooted machine path
//!     and no `file://` URL;
//!   - the `task_pattern` reference resolves against the existing catalogue
//!     and fails closed on an unknown, ambiguous or inconsistent
//!     identifier;
//!   - acceptance criteria are non-empty, stably identified, non-duplicate
//!     and carry a verifiable condition (a method plus an expected
//!     result), not a free phrase;
//!   - run state (lifecycle stage, work status, actor, transition history,
//!     next step) does not leak into the statement of the work;
//!   - the specification is portable: no rooted machine path anywhere in
//!     its text — whatever ordinary separator precedes it and whatever the
//!     first path segment contains;
//!   - the human-readable name is stated in Russian for the reader.

use fancy_regex::Regex;
use serde_json::Value;

use crate::source_format::json_schema;

pub const RECORD_TYPE: &str = "task-specification";
pub const WORK_KINDS: [&str; 4] = ["assessment", "operation", "initiative", "change"];
pub const CHANGE_CLASSES: [&str; 4] = ["BUGFIX", "FEATURE", "BEHAVIOR_CHANGE", "REFACTOR"];

pub const CANONICAL_RECORD_BASE: &str = "records/task-specification";
pub const SCHEMA_NAMESPACE_DIR: &str = "registries/operating-model";
pub const EXPECTED_SCHEMA_BASENAME: &str = "task-specification.schema.json";
pub const ENVELOPE_SCHEMA_BASENAME: &str = "scoped-record.schema.json";

fn expected_schema_ref() -> String {
    format!("{SCHEMA_NAMESPACE_DIR}/{EXPECTED_SCHEMA_BASENAME}")
}
fn envelope_schema_ref() -> String {
    format!("{SCHEMA_NAMESPACE_DIR}/{ENVELOPE_SCHEMA_BASENAME}")
}

/// A synthetic namespace root the reference is resolved beneath, so that a
/// reference climbing past the Meridian namespace root is detectable rather
/// than silently clamped. It is a lexical anchor for address
/// normalisation, not a filesystem path.
const NAMESPACE_ROOT: &str = "/__meridian_namespace__";

fn is_drive_letter_prefix(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// A minimal `path.posix.normalize` for the one shape this module needs:
/// an always-absolute input (it always starts from `NAMESPACE_ROOT`),
/// collapsing `.`/empty segments and popping a real segment on `..`
/// (clamped at the root, exactly as Node's POSIX `normalize` does — never
/// erroring past it).
fn posix_normalize_absolute(p: &str) -> String {
    let mut stack: Vec<&str> = Vec::new();
    for seg in p.split('/') {
        match seg {
            "" | "." => continue,
            ".." => {
                stack.pop();
            }
            other => stack.push(other),
        }
    }
    format!("/{}", stack.join("/"))
}

/// Resolve a record's declared `$schema` as a logical address within the
/// Meridian namespace, from the canonical logical base. Returns the
/// namespace-root-relative address it resolves to, or `None` when it is
/// not a resolvable in-namespace relative reference (an absolute path, a
/// drive-letter path, a backslash path, or a reference that climbs out of
/// the namespace root). Port of `resolveSchemaRef`.
pub fn resolve_schema_ref(declared: Option<&str>) -> Option<String> {
    let declared = declared.filter(|s| !s.is_empty())?;
    if declared.contains('\\') {
        return None;
    }
    if declared.starts_with('/') || is_drive_letter_prefix(declared) {
        return None;
    }
    let from = format!("{NAMESPACE_ROOT}/{CANONICAL_RECORD_BASE}");
    let resolved = posix_normalize_absolute(&format!("{from}/{declared}"));
    let prefix = format!("{NAMESPACE_ROOT}/");
    if resolved != NAMESPACE_ROOT && !resolved.starts_with(&prefix) {
        return None;
    }
    if resolved == NAMESPACE_ROOT {
        Some(String::new())
    } else {
        Some(resolved[NAMESPACE_ROOT.len() + 1..].to_string())
    }
}

/// The only two workspace-scope-model areas a project task specification
/// may occupy, and the reason each other area is excluded.
pub const ALLOWED_SCOPE_TYPES: [&str; 2] = ["project-workspace", "repository-scope"];

fn scope_rejection_reason(scope_type: &str) -> &'static str {
    match scope_type {
        "built-in-methodology" => "built-in-methodology is Kernel methodology, not a place for a concrete work statement",
        "user-profile" => "user-profile holds a user's rules and settings, not a work statement",
        "organization-profile" => "organization-profile holds an organisation's rules and settings, not a work statement",
        "run-state" => "run-state holds one execution run's episodic state, and a specification exists before any run and may drive several",
        _ => "it is not one of the two areas a project task specification may occupy",
    }
}

/// Fields that describe the RUN, not the statement of the work.
/// `execution-state-model` owns them
/// (`standards/workspace/task-specification.md` §5).
pub const RUN_STATE_FIELDS: [&str; 14] = [
    "lifecycle_stage",
    "work_status",
    "scope_revision",
    "current_actor",
    "actor",
    "supervision_mode",
    "resolved_norms",
    "completed_checks",
    "blockers",
    "next_action",
    "next_gate",
    "transition_history",
    "workflow",
    "run_state",
];

static SEMANTIC_ID_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$").expect("semantic id pattern compiles")
});

fn is_semantic_id(s: &str) -> bool {
    SEMANTIC_ID_RE.is_match(s).unwrap_or(false)
}

fn is_object(v: &Value) -> bool {
    v.is_object()
}

fn blank(v: &Value) -> bool {
    match v.as_str() {
        Some(s) => s.trim().is_empty(),
        None => true,
    }
}

fn has_cyrillic(s: &str) -> bool {
    s.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c))
}

/// Any ROOTED machine path, a drive-letter path, a backslash-separated
/// path, a `~/` home reference or a `file://` URL anywhere in a
/// specification's text breaks portability. Returns a short reason string
/// or `None`. Port of `nonPortableReason`.
static FILE_URL_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"(?i)\bfile://").expect("file url pattern compiles"));
static SCRUB_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r#"(?i)\b(?:https?|ftp|ftps|ssh|git|mailto):(?://)?[^\s"'()<>\[\]]+"#)
        .expect("scrub pattern compiles")
});
static HOME_REF_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r#"(?:^|[^\p{L}\p{N}._~-])~/"#).expect("home ref pattern compiles")
});
static DRIVE_LETTER_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r#"(?:^|[^A-Za-z0-9])[A-Za-z]:[\\/][^\s"')>\]]+"#)
        .expect("drive letter pattern compiles")
});
static ROOTED_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r#"(?:^|[^\p{L}\p{N}._~-])/[^\s/"'()<>\[\]][^\s"')>\]]*"#)
        .expect("rooted path pattern compiles")
});
static BACKSLASH_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r#"[\p{L}\p{N}_.-]+\\[\p{L}\p{N}_.-]+"#).expect("backslash pattern compiles")
});

pub fn non_portable_reason(s: Option<&str>) -> Option<String> {
    let s = s.filter(|s| !s.is_empty())?;

    if FILE_URL_RE.is_match(s).unwrap_or(false) {
        return Some("a file:// URL".to_string());
    }

    let scrubbed = SCRUB_RE.replace_all(s, " ");

    if HOME_REF_RE.is_match(scrubbed.as_ref()).unwrap_or(false) {
        return Some("a \"~/\" home-directory reference".to_string());
    }

    if DRIVE_LETTER_RE.is_match(scrubbed.as_ref()).unwrap_or(false) {
        return Some("a drive-letter path".to_string());
    }

    if ROOTED_RE.is_match(scrubbed.as_ref()).unwrap_or(false) {
        return Some("a rooted (absolute) POSIX path".to_string());
    }

    if BACKSLASH_RE.is_match(scrubbed.as_ref()).unwrap_or(false) {
        return Some("a backslash-separated path".to_string());
    }

    None
}

/// A minimal representative of a `task-pattern-registry` entry, as seen by
/// this check: identity plus the classification pair.
pub struct TaskPatternRef<'a> {
    pub id: &'a str,
    pub work_kind: Option<&'a str>,
    pub change_class: Option<&'a str>,
}

pub struct EvalSchemas<'a> {
    pub record_schema: &'a Value,
    pub envelope_schema: &'a Value,
}

/// The whole composition pipeline for one task specification: the
/// canonical record envelope (reused, always run), the complete
/// specialised schema (which a `$schema`-follower would also use), then
/// the cross-cutting rules the JSON Schema subset cannot state. Port of
/// `evaluateTaskSpecification`.
pub fn evaluate_task_specification(
    doc: &Value,
    schemas: &EvalSchemas,
    task_patterns: &[TaskPatternRef],
) -> Vec<String> {
    let mut problems = Vec::new();

    match json_schema::validate(doc, schemas.envelope_schema) {
        Ok(errors) => problems.extend(errors.into_iter().map(|m| format!("envelope {m}"))),
        Err(error) => {
            return vec![format!(
                "record envelope schema could not be applied: {error}"
            )]
        }
    }

    match json_schema::validate(doc, schemas.record_schema) {
        Ok(errors) => problems.extend(errors),
        Err(error) => {
            return vec![format!(
                "task-specification schema could not be applied: {error}"
            )]
        }
    }

    if !is_object(doc) {
        if problems.is_empty() {
            return vec!["the task specification is not an object".to_string()];
        }
        return problems;
    }

    let id = doc
        .get("id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or("(no id)")
        .to_string();

    let declared = doc.get("$schema").and_then(Value::as_str);
    match declared {
        None => {
            problems.push(format!(
                "task specification \"{id}\" declares no $schema; a record names {EXPECTED_SCHEMA_BASENAME} so a consumer validates the whole contract, not only the envelope"
            ));
        }
        Some(declared) => {
            if let Some(portability) = non_portable_reason(Some(declared)) {
                problems.push(format!(
                    "task specification \"{id}\" $schema \"{declared}\" is not portable ({portability}); the schema is named by a relative reference resolved from the canonical logical resolution base ({CANONICAL_RECORD_BASE})"
                ));
            } else {
                let resolved = resolve_schema_ref(Some(declared));
                if resolved.as_deref() == Some(envelope_schema_ref().as_str()) {
                    problems.push(format!(
                        "task specification \"{id}\" $schema \"{declared}\" resolves to the record envelope ({ENVELOPE_SCHEMA_BASENAME}); it must name {EXPECTED_SCHEMA_BASENAME}, which composes the envelope with the body"
                    ));
                } else if resolved.as_deref() != Some(expected_schema_ref().as_str()) {
                    let expected = expected_schema_ref();
                    problems.push(format!(
                        "task specification \"{id}\" $schema \"{declared}\" does not resolve to the logical address {expected} from the canonical logical resolution base ({CANONICAL_RECORD_BASE}); the specialised schema is named by a relative reference written from that base"
                    ));
                }
            }
        }
    }

    let record_type = doc.get("record_type").and_then(Value::as_str).unwrap_or("");
    if record_type != RECORD_TYPE {
        problems.push(format!(
            "task specification \"{id}\" declares record_type \"{record_type}\", not \"{RECORD_TYPE}\""
        ));
    }
    let doc_id = doc.get("id").and_then(Value::as_str);
    if !doc_id.is_some_and(is_semantic_id) {
        problems.push(format!(
            "task specification \"{id}\" has no stable semantic id on the record envelope"
        ));
    }
    let title = doc.get("title").and_then(Value::as_str);
    match title {
        None => problems.push(format!(
            "task specification \"{id}\" has no human-readable title"
        )),
        Some(t) if t.trim().is_empty() => problems.push(format!(
            "task specification \"{id}\" has no human-readable title"
        )),
        Some(t) if !has_cyrillic(t) => problems.push(format!(
            "task specification \"{id}\" title \"{t}\" carries no Russian (Cyrillic) text; the specification name is stated in Russian for the human reader"
        )),
        _ => {}
    }

    let empty_obj = Value::Object(Default::default());
    let scope = doc
        .get("scope")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);
    if let Some(scope_type) = scope.get("type").and_then(Value::as_str) {
        if !ALLOWED_SCOPE_TYPES.contains(&scope_type) {
            let why = scope_rejection_reason(scope_type);
            problems.push(format!(
                "task specification \"{id}\" is scoped to \"{scope_type}\"; a project task specification lives in project-workspace or repository-scope only — {why}"
            ));
        }
    }
    let origin = doc
        .get("origin")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);
    if origin.get("kind").and_then(Value::as_str) == Some("built-in") {
        problems.push(format!(
            "task specification \"{id}\" declares origin.kind \"built-in\"; a concrete specification is authored in a workspace, not shipped with the methodology"
        ));
    }
    let authority = doc
        .get("authority")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);

    for (obj, field, label) in [
        (origin, "source_ref", "origin.source_ref"),
        (authority, "authority_ref", "authority.authority_ref"),
        (authority, "decision_ref", "authority.decision_ref"),
    ] {
        if let Some(val) = obj
            .get(field)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            if let Some(r) = non_portable_reason(Some(val)) {
                problems.push(format!(
                    "task specification \"{id}\" {label} contains {r}; a specification is portable and carries no rooted machine path"
                ));
            }
        }
    }

    let payload = doc
        .get("payload")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);

    for f in RUN_STATE_FIELDS {
        if payload.get(f).is_some() {
            problems.push(format!(
                "task specification \"{id}\" payload carries \"{f}\"; run state (stage, status, actor, transition history, next step) belongs to execution-state-model, not to the specification"
            ));
        }
    }
    if payload.get("record_type").is_some() {
        problems.push(format!(
            "task specification \"{id}\" repeats record_type inside the payload; the record type is declared once, on the envelope"
        ));
    }

    for (f, label) in [
        ("goal", "goal"),
        ("initial_state", "initial state"),
        ("target_model", "target model"),
    ] {
        match payload.get(f) {
            None => problems.push(format!(
                "task specification \"{id}\" states no {label}; it is a required input with no default"
            )),
            Some(v) if blank(v) => problems.push(format!(
                "task specification \"{id}\" {label} is empty or whitespace-only"
            )),
            Some(v) => {
                if let Some(r) = non_portable_reason(v.as_str()) {
                    problems.push(format!(
                        "task specification \"{id}\" {label} contains {r}; a specification is portable and carries no rooted machine path"
                    ));
                }
            }
        }
    }
    if let Some(r) = non_portable_reason(title) {
        problems.push(format!("task specification \"{id}\" title contains {r}"));
    }

    let empty_arr = Vec::new();
    let constraints = payload
        .get("constraints")
        .and_then(Value::as_array)
        .unwrap_or(&empty_arr);
    if constraints.is_empty() {
        problems.push(format!(
            "task specification \"{id}\" carries no constraints; the set must be non-empty"
        ));
    } else {
        let mut seen = std::collections::HashSet::new();
        for (i, c) in constraints.iter().enumerate() {
            if blank(c) {
                problems.push(format!(
                    "task specification \"{id}\" constraint #{i} is empty or whitespace-only"
                ));
                continue;
            }
            let c_str = c.as_str().unwrap_or("");
            if let Some(r) = non_portable_reason(Some(c_str)) {
                problems.push(format!(
                    "task specification \"{id}\" constraint #{i} contains {r}"
                ));
            }
            let key = c_str.trim().to_string();
            if !seen.insert(key) {
                problems.push(format!(
                    "task specification \"{id}\" repeats constraint \"{c_str}\""
                ));
            }
        }
    }

    let crits = payload
        .get("acceptance_criteria")
        .and_then(Value::as_array)
        .unwrap_or(&empty_arr);
    if crits.is_empty() {
        problems.push(format!(
            "task specification \"{id}\" carries no acceptance criteria; the set must be non-empty"
        ));
    } else {
        let mut seen_id = std::collections::HashSet::new();
        let mut seen_body = std::collections::HashSet::new();
        for (i, c) in crits.iter().enumerate() {
            let co = c.clone();
            let co = if is_object(&co) { co } else { Value::Null };
            let cid = co.get("id").and_then(Value::as_str);
            let at = cid.map(str::to_string).unwrap_or_else(|| format!("#{i}"));
            match cid {
                Some(cid) if is_semantic_id(cid) => {
                    if !seen_id.insert(cid.to_string()) {
                        problems.push(format!(
                            "task specification \"{id}\" acceptance criterion id \"{cid}\" is used more than once"
                        ));
                    }
                }
                _ => problems.push(format!(
                    "task specification \"{id}\" acceptance criterion {at} has no stable identifier (a semantic id)"
                )),
            }
            let statement = co.get("statement");
            if statement.map(blank).unwrap_or(true) {
                problems.push(format!(
                    "task specification \"{id}\" acceptance criterion {at} has no statement"
                ));
            }
            let verification = co.get("verification").filter(|v| is_object(v));
            let method = verification.and_then(|v| v.get("method"));
            let expected_result = verification.and_then(|v| v.get("expected_result"));
            let verifiable = verification.is_some()
                && !method.map(blank).unwrap_or(true)
                && !expected_result.map(blank).unwrap_or(true);
            if !verifiable {
                problems.push(format!(
                    "task specification \"{id}\" acceptance criterion {at} has no verifiable condition; a criterion states a method and an observable expected_result, not a free phrase"
                ));
            }
            for s in [
                statement.and_then(Value::as_str),
                method.and_then(Value::as_str),
                expected_result.and_then(Value::as_str),
            ] {
                if let Some(r) = non_portable_reason(s) {
                    problems.push(format!(
                        "task specification \"{id}\" acceptance criterion {at} contains {r}"
                    ));
                }
            }
            let body_key = format!(
                "{}|{}|{}",
                statement.and_then(Value::as_str).unwrap_or("").trim(),
                method.and_then(Value::as_str).unwrap_or("").trim(),
                expected_result.and_then(Value::as_str).unwrap_or("").trim(),
            );
            if !seen_body.insert(body_key) {
                problems.push(format!(
                    "task specification \"{id}\" acceptance criterion {at} duplicates another criterion's statement and verification"
                ));
            }
        }
    }

    let tp = payload.get("task_pattern").filter(|v| is_object(v));
    let tp_id = tp.and_then(|v| v.get("id")).and_then(Value::as_str);
    match tp_id.filter(|s| !s.is_empty()) {
        None => problems.push(format!(
            "task specification \"{id}\" names no task pattern; the specification selects exactly one pattern from task-pattern-registry"
        )),
        Some(tp_id) => {
            let matches: Vec<&TaskPatternRef> =
                task_patterns.iter().filter(|p| p.id == tp_id).collect();
            if matches.is_empty() {
                problems.push(format!(
                    "task specification \"{id}\" references task pattern \"{tp_id}\", which is not in task-pattern-registry"
                ));
            } else if matches.len() > 1 {
                let n = matches.len();
                problems.push(format!(
                    "task specification \"{id}\" references task pattern \"{tp_id}\", which is declared {n} times in task-pattern-registry — the reference is ambiguous"
                ));
            } else {
                let p = matches[0];
                let tp_work_kind = tp.unwrap().get("work_kind").and_then(Value::as_str);
                if let Some(twk) = tp_work_kind {
                    if Some(twk) != p.work_kind {
                        let pwk = p.work_kind.unwrap_or("");
                        problems.push(format!(
                            "task specification \"{id}\" declares work_kind \"{twk}\" but task pattern \"{tp_id}\" is work_kind \"{pwk}\""
                        ));
                    }
                }
                if tp.unwrap().get("change_class").is_some() {
                    let tcc = tp.unwrap().get("change_class").and_then(Value::as_str);
                    if tcc != p.change_class {
                        let tcc_display = tcc.unwrap_or("none");
                        let pcc_display = p.change_class.unwrap_or("none");
                        problems.push(format!(
                            "task specification \"{id}\" declares change_class \"{tcc_display}\" but task pattern \"{tp_id}\" is change_class \"{pcc_display}\""
                        ));
                    }
                }
            }
        }
    }

    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_schema_ref_accepts_the_canonical_relative_reference() {
        let resolved = resolve_schema_ref(Some(
            "../../registries/operating-model/task-specification.schema.json",
        ));
        assert_eq!(resolved.as_deref(), Some(expected_schema_ref().as_str()));
    }

    #[test]
    fn resolve_schema_ref_rejects_an_absolute_path() {
        assert!(resolve_schema_ref(Some(
            "/registries/operating-model/task-specification.schema.json"
        ))
        .is_none());
    }

    #[test]
    fn resolve_schema_ref_rejects_climbing_out_of_the_namespace() {
        assert!(resolve_schema_ref(Some("../../../../../../etc/passwd")).is_none());
    }

    #[test]
    fn resolve_schema_ref_resolves_the_envelope_schema_distinctly() {
        let resolved = resolve_schema_ref(Some(
            "../../registries/operating-model/scoped-record.schema.json",
        ));
        assert_eq!(resolved.as_deref(), Some(envelope_schema_ref().as_str()));
    }

    #[test]
    fn non_portable_reason_detects_file_url() {
        assert!(non_portable_reason(Some("file:///etc/passwd"))
            .unwrap()
            .contains("file://"));
    }

    #[test]
    fn non_portable_reason_detects_home_reference() {
        assert!(non_portable_reason(Some("go look at ~/secrets")).is_some());
    }

    #[test]
    fn non_portable_reason_detects_drive_letter_path() {
        assert!(non_portable_reason(Some("C:\\Users\\name"))
            .unwrap()
            .contains("drive-letter"));
    }

    #[test]
    fn non_portable_reason_detects_rooted_posix_path() {
        assert!(non_portable_reason(Some("see /etc/passwd for details"))
            .unwrap()
            .contains("rooted"));
    }

    #[test]
    fn non_portable_reason_is_silent_on_relative_paths() {
        assert!(non_portable_reason(Some("src/config/parser")).is_none());
        assert!(non_portable_reason(Some("n/a")).is_none());
        assert!(non_portable_reason(Some("24/7")).is_none());
        assert!(non_portable_reason(Some("owner-decision:2026-09-09:example")).is_none());
    }

    #[test]
    fn non_portable_reason_is_silent_on_ordinary_urls() {
        assert!(non_portable_reason(Some("see https://example.invalid/api for details")).is_none());
    }

    #[test]
    fn non_portable_reason_flags_unicode_rooted_paths_with_no_allow_list() {
        assert!(non_portable_reason(Some("см. /Проект/файл")).is_some());
        assert!(non_portable_reason(Some("see /数据/文件")).is_some());
    }

    #[test]
    fn run_state_fields_matches_the_node_reference_exactly() {
        assert_eq!(
            RUN_STATE_FIELDS,
            [
                "lifecycle_stage",
                "work_status",
                "scope_revision",
                "current_actor",
                "actor",
                "supervision_mode",
                "resolved_norms",
                "completed_checks",
                "blockers",
                "next_action",
                "next_gate",
                "transition_history",
                "workflow",
                "run_state",
            ]
        );
    }

    #[test]
    fn has_cyrillic_detects_cyrillic_text() {
        assert!(has_cyrillic("Русский текст"));
        assert!(!has_cyrillic("English text"));
    }

    #[test]
    fn the_real_kernel_schema_and_fixtures_agree() {
        let kernel_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let record_schema: Value = serde_json::from_str(
            &std::fs::read_to_string(
                kernel_root.join("registries/operating-model/task-specification.schema.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let envelope_schema: Value = serde_json::from_str(
            &std::fs::read_to_string(
                kernel_root.join("registries/operating-model/scoped-record.schema.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let fixtures: Value = serde_json::from_str(
            &std::fs::read_to_string(
                kernel_root
                    .join("registries/operating-model/fixtures/task-specification.fixtures.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let schemas = EvalSchemas {
            record_schema: &record_schema,
            envelope_schema: &envelope_schema,
        };
        let tpr_yaml = std::fs::read_to_string(
            kernel_root.join("standards/workspace/task-pattern-registry.yaml"),
        )
        .unwrap();
        let tpr_doc = crate::source_format::parse_yaml(&tpr_yaml).unwrap();
        let empty = Vec::new();
        let entries = tpr_doc
            .get("task_patterns")
            .and_then(Value::as_array)
            .unwrap_or(&empty);
        let task_patterns: Vec<TaskPatternRef> = entries
            .iter()
            .map(|p| TaskPatternRef {
                id: p.get("id").and_then(Value::as_str).unwrap_or(""),
                work_kind: p
                    .get("payload")
                    .and_then(|v| v.get("work_kind"))
                    .and_then(Value::as_str),
                change_class: p
                    .get("payload")
                    .and_then(|v| v.get("change_class"))
                    .and_then(Value::as_str),
            })
            .collect();
        for case in fixtures["valid"].as_array().unwrap() {
            let problems = evaluate_task_specification(&case["spec"], &schemas, &task_patterns);
            assert!(
                problems.is_empty(),
                "note={:?} problems={:?}",
                case.get("note"),
                problems
            );
        }
        for case in fixtures["invalid"].as_array().unwrap() {
            let problems = evaluate_task_specification(&case["spec"], &schemas, &task_patterns);
            assert!(!problems.is_empty(), "note={:?}", case.get("note"));
        }
    }
}
