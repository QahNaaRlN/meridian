//! Business-contract migration of `scripts/lib/context-manifest.mjs`: the
//! composite-consistency algorithm behind the `bounded-context-manifest`
//! check. `registries/operating-model/context-manifest.schema.json` is the
//! COMPLETE schema for the bounded context manifest of ONE execution run
//! (`record_type: context-manifest`). Manifest DATA is Instance data; the
//! Kernel ships the schema, the product-neutral fixtures and this module.
//!
//! The manifest relates to EXACTLY ONE run, and the link is DETERMINISTIC,
//! not a substring heuristic:
//!   - `execution_run_ref`, `task_specification_ref` and
//!     `human_control_ref` are CLOSED structured pinned references —
//!     `{ record_type, id, reference, run_id?, revision?, sha256? }` —
//!     never a plain string, never an embedded body;
//!   - `scope.id` equals `execution_run_ref.id` by exact string
//!     comparison;
//!   - `human_control_ref.run_id` equals `execution_run_ref.id`;
//!     `task_specification_ref` carries no `run_id`;
//!   - `run_state_checkpoint` is the PINNED authoritative snapshot,
//!     resolved through a boundary the manifest does not control (a
//!     resolver the caller supplies), and checked against the state the
//!     RESOLVED execution-run record carries. With no resolver the
//!     manifest fails closed.
//!
//! An exact revision is a CLOSED, fail-closed set ([`classify_revision`]):
//! a full Git SHA (40 or 64 hex), a strict `v?X.Y.Z` SemVer tag, or a
//! SHA-256 digest. A branch/channel reference is FLOATING whatever digits
//! it carries. Any other provider-specific or unknown revision format is
//! WEAK and needs a `sha256`.
//!
//! The `$schema` resolution and rooted-path rules are REUSED from
//! [`super::task_specification`], and the lifecycle/work-status pools from
//! [`super::execution_state`] — this module carries no divergent second
//! copy of either.

use std::collections::HashSet;

use serde_json::Value;

use super::execution_state::{LIFECYCLE_STAGES, TERMINAL_STATUSES, WORK_STATUSES};
use super::task_specification::{non_portable_reason, resolve_schema_ref};
use crate::source_format::json_schema;

pub const RECORD_TYPE: &str = "context-manifest";

pub const CANONICAL_RECORD_BASE: &str = "records/context-manifest";
pub const SCHEMA_NAMESPACE_DIR: &str = "registries/operating-model";
pub const EXPECTED_SCHEMA_BASENAME: &str = "context-manifest.schema.json";
pub const ENVELOPE_SCHEMA_BASENAME: &str = "scoped-record.schema.json";

fn expected_schema_ref() -> String {
    format!("{SCHEMA_NAMESPACE_DIR}/{EXPECTED_SCHEMA_BASENAME}")
}
fn envelope_schema_ref() -> String {
    format!("{SCHEMA_NAMESPACE_DIR}/{ENVELOPE_SCHEMA_BASENAME}")
}

pub const ALLOWED_SCOPE_TYPE: &str = "run-state";

fn scope_rejection_reason(scope_type: &str) -> &'static str {
    match scope_type {
        "built-in-methodology" => "built-in-methodology is Kernel methodology, not a place for one run's context manifest",
        "user-profile" => "user-profile holds a user's rules and settings, not a run's context manifest",
        "organization-profile" => "organization-profile holds an organisation's rules and settings, not a run's context manifest",
        "project-workspace" => "project-workspace holds the project's goals and decisions; a run's context manifest is scoped to run-state",
        "repository-scope" => "repository-scope holds facts true for one repository; a run may span repositories and its manifest is scoped to run-state",
        _ => "it is not run-state",
    }
}

enum RunIdRule {
    SelfRun,
    Forbidden,
    Required,
}

struct PinnedRefSlot {
    record_type: &'static str,
    run_id: RunIdRule,
}

fn strip_checkpoint_prefix(field: &str) -> &str {
    field.strip_prefix("run_state_checkpoint.").unwrap_or(field)
}

fn pinned_ref_slot(field: &str) -> Option<PinnedRefSlot> {
    match strip_checkpoint_prefix(field) {
        "execution_run_ref" => Some(PinnedRefSlot {
            record_type: "execution-run",
            run_id: RunIdRule::SelfRun,
        }),
        "task_specification_ref" => Some(PinnedRefSlot {
            record_type: "task-specification",
            run_id: RunIdRule::Forbidden,
        }),
        "human_control_ref" => Some(PinnedRefSlot {
            record_type: "run-human-control",
            run_id: RunIdRule::Required,
        }),
        _ => None,
    }
}

/// Known moving tokens that are never a pin. A branch or channel name
/// identifies a moving target, not a fixed edition.
pub const FLOATING_REVISION_TOKENS: [&str; 16] = [
    "latest", "current", "head", "tip", "stable", "unstable", "newest", "rolling", "edge",
    "nightly", "trunk", "main", "master", "develop", "dev", "default",
];
// Node's set also carries "release" as its 17th member.
const FLOATING_REVISION_TOKEN_RELEASE: &str = "release";

fn is_floating_token(t: &str) -> bool {
    let lower = t.to_lowercase();
    FLOATING_REVISION_TOKENS.contains(&lower.as_str()) || lower == FLOATING_REVISION_TOKEN_RELEASE
}

/// Fields that belong to a LATER package, or that would turn the bounded
/// manifest into an unbounded archive.
pub const FORBIDDEN_PAYLOAD_FIELDS: [&str; 33] = [
    "file_contents",
    "full_text",
    "full_texts",
    "raw_context",
    "context_dump",
    "command_log",
    "commands",
    "transcript",
    "messages",
    "chat_history",
    "conversation",
    "directory_dump",
    "dir_listing",
    "tree_dump",
    "attachments",
    "evidence",
    "evidence_contract",
    "claims",
    "claim_evidence_map",
    "handoff",
    "handoff_contract",
    "external_effects",
    "worktree_state",
    "worktree_status",
    "verification_verdict",
    "check_classification",
    "field_metrics",
    "evaluation_metrics",
    "observation_log",
    "task_specification",
    "execution_run",
    "human_control",
    "goal",
];
// Node's array continues: 'initial_state', 'target_model', 'task_pattern',
// 'constraints', 'acceptance_criteria', 'transition_history',
// 'switch_history', 'role_assignments', 'supervision_mode',
// 'human_authority', 'hitl', 'hotl', 'communication_mode' — a second array
// so the combined check list matches exactly without one unreadable literal.
pub const FORBIDDEN_PAYLOAD_FIELDS_2: [&str; 13] = [
    "initial_state",
    "target_model",
    "task_pattern",
    "constraints",
    "acceptance_criteria",
    "transition_history",
    "switch_history",
    "role_assignments",
    "supervision_mode",
    "human_authority",
    "hitl",
    "hotl",
    "communication_mode",
];

fn all_forbidden_fields() -> impl Iterator<Item = &'static str> {
    FORBIDDEN_PAYLOAD_FIELDS
        .into_iter()
        .chain(FORBIDDEN_PAYLOAD_FIELDS_2)
}

static SEMANTIC_ID_RE: std::sync::LazyLock<fancy_regex::Regex> = std::sync::LazyLock::new(|| {
    fancy_regex::Regex::new(r"^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$")
        .expect("semantic id pattern compiles")
});
static FULL_GIT_SHA_RE: std::sync::LazyLock<fancy_regex::Regex> = std::sync::LazyLock::new(|| {
    fancy_regex::Regex::new(r"^[0-9a-f]{40}$").expect("full git sha pattern compiles")
});
static SHA256_RE: std::sync::LazyLock<fancy_regex::Regex> = std::sync::LazyLock::new(|| {
    fancy_regex::Regex::new(r"^[0-9a-fA-F]{64}$").expect("sha256 pattern compiles")
});
static STRICT_SEMVER_RE: std::sync::LazyLock<fancy_regex::Regex> = std::sync::LazyLock::new(|| {
    fancy_regex::Regex::new(r"^v?(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)$")
        .expect("semver pattern compiles")
});
static HEX_ONLY_RE: std::sync::LazyLock<fancy_regex::Regex> = std::sync::LazyLock::new(|| {
    fancy_regex::Regex::new(r"^[0-9a-f]+$").expect("hex only pattern compiles")
});
static BARE_WORD_RE: std::sync::LazyLock<fancy_regex::Regex> = std::sync::LazyLock::new(|| {
    fancy_regex::Regex::new(r"^[A-Za-z][A-Za-z0-9]*$").expect("bare word pattern compiles")
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
fn blank_opt(v: Option<&Value>) -> bool {
    v.map(blank).unwrap_or(true)
}
fn has_cyrillic(s: &str) -> bool {
    s.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c))
}
fn is_positive_int(v: &Value) -> bool {
    match v {
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i > 0
            } else {
                n.as_u64().is_some_and(|u| u > 0)
            }
        }
        _ => false,
    }
}
/// `(a ?? null) === (b ?? null)` over two optional step values.
fn same_step_value(a: Option<&Value>, b: Option<&Value>) -> bool {
    let norm = |v: Option<&Value>| -> Value {
        match v {
            None => Value::Null,
            Some(v) => v.clone(),
        }
    };
    norm(a) == norm(b)
}
fn is_valid_sha256(s: Option<&str>) -> bool {
    s.is_some_and(|s| SHA256_RE.is_match(s.trim()).unwrap_or(false))
}

/// JS `${x}` template-literal string coercion for a JSON value (or an
/// absent property, coerced to `"undefined"` exactly as an actual
/// JavaScript `undefined` would be).
fn js_str(v: Option<&Value>) -> String {
    match v {
        None => "undefined".to_string(),
        Some(Value::Null) => "null".to_string(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Array(a)) => a
            .iter()
            .map(|x| js_str(Some(x)))
            .collect::<Vec<_>>()
            .join(","),
        Some(Value::Object(_)) => "[object Object]".to_string(),
    }
}

/// JS `JSON.stringify(x)` interpolated into a template literal: `undefined`
/// (absent) still renders as the literal string `"undefined"`, exactly as
/// `${JSON.stringify(undefined)}` does (inserting the JS `undefined` value
/// into a template converts it to that string) — every other value is its
/// real JSON text.
fn json_stringify(v: Option<&Value>) -> String {
    match v {
        None => "undefined".to_string(),
        Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "undefined".to_string()),
    }
}

fn js_typeof(v: &Value) -> &'static str {
    match v {
        Value::Null => "object",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "object",
        Value::Object(_) => "object",
    }
}

/// The CLOSED, fail-closed classification of a revision string. Port of
/// `classifyRevision`.
pub fn classify_revision(rev: Option<&str>) -> &'static str {
    let Some(rev) = rev else { return "absent" };
    let t = rev.trim();
    if t.is_empty() {
        return "absent";
    }
    if FULL_GIT_SHA_RE.is_match(t).unwrap_or(false)
        || SHA256_RE.is_match(t).unwrap_or(false)
        || STRICT_SEMVER_RE.is_match(t).unwrap_or(false)
    {
        return "exact";
    }
    if is_floating_token(t) {
        return "floating";
    }
    if t.contains('/') {
        return "floating";
    }
    if HEX_ONLY_RE.is_match(t).unwrap_or(false) {
        return "abbrev-sha";
    }
    if BARE_WORD_RE.is_match(t).unwrap_or(false) {
        return "floating";
    }
    "weak"
}

/// Port of `isFloatingRevision`.
pub fn is_floating_revision(rev: Option<&str>) -> bool {
    classify_revision(rev) == "floating"
}

/// Is this `{ revision, sha256 }` pair a sufficient pin? Port of
/// `pinDefect`.
fn pin_defect(revision: Option<&str>, sha256: Option<&str>) -> Option<String> {
    if is_valid_sha256(sha256) {
        return None;
    }
    match classify_revision(revision) {
        "exact" => None,
        "floating" => Some(format!(
            "revision {} is a branch or channel reference; a branch or channel is never a pin, whatever digits it carries",
            json_quote(revision)
        )),
        "abbrev-sha" => Some(format!(
            "revision {} looks like an abbreviated or ambiguous Git SHA (not a full 40- or 64-hex object name); pin the full SHA or add a sha256 digest",
            json_quote(revision)
        )),
        "weak" => Some(format!(
            "revision {} is not a recognised exact revision (a full Git SHA, a strict v?X.Y.Z tag or a SHA-256 digest); a provider-specific or unknown revision format requires a sha256 digest",
            json_quote(revision)
        )),
        _ => Some("it is pinned by neither an exact revision nor a SHA-256 digest".to_string()),
    }
}

fn json_quote(s: Option<&str>) -> String {
    match s {
        None => "null".to_string(),
        Some(s) => serde_json::to_string(s).unwrap_or_default(),
    }
}

/// Validate one `pinned_ref` slot. Port of `checkPinnedRef`.
fn check_pinned_ref(
    problems: &mut Vec<String>,
    id: &str,
    field: &str,
    ref_val: &Value,
    run_id: Option<&str>,
) {
    let Some(slot) = pinned_ref_slot(field) else {
        return;
    };
    if !is_object(ref_val) {
        problems.push(format!(
            "context manifest \"{id}\" {field} is not a structured pinned reference; it is a closed {{ record_type, id, reference, revision?/sha256? }} object, never a plain string and never an embedded body"
        ));
        return;
    }
    let record_type = ref_val.get("record_type");
    if record_type.and_then(Value::as_str) != Some(slot.record_type) {
        problems.push(format!(
            "context manifest \"{id}\" {field} names record_type \"{}\", not \"{}\"",
            js_str(record_type),
            slot.record_type
        ));
    }
    let ref_id = ref_val.get("id").and_then(Value::as_str);
    if !ref_id.is_some_and(is_semantic_id) {
        problems.push(format!(
            "context manifest \"{id}\" {field} has no stable semantic id"
        ));
    }
    let reference = ref_val.get("reference");
    if reference.map(blank).unwrap_or(true) {
        problems.push(format!(
            "context manifest \"{id}\" {field} has no portable reference"
        ));
    } else if let Some(r) = non_portable_reason(reference.and_then(Value::as_str)) {
        problems.push(format!(
            "context manifest \"{id}\" {field} reference contains {r}; the reference is portable and is not an absolute machine path"
        ));
    }
    let revision = ref_val.get("revision").and_then(Value::as_str);
    let sha256 = ref_val.get("sha256").and_then(Value::as_str);
    if let Some(defect) = pin_defect(revision, sha256) {
        problems.push(format!(
            "context manifest \"{id}\" {field} is not pinned to an exact edition: {defect}"
        ));
    }
    if let Some(rev) = revision.filter(|s| !s.is_empty()) {
        if let Some(rr) = non_portable_reason(Some(rev)) {
            problems.push(format!(
                "context manifest \"{id}\" {field} revision contains {rr}"
            ));
        }
    }
    let has_run_id = ref_val
        .get("run_id")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.is_empty());
    let ref_run_id = ref_val.get("run_id").and_then(Value::as_str);
    match slot.run_id {
        RunIdRule::Forbidden => {
            if has_run_id {
                problems.push(format!(
                    "context manifest \"{id}\" {field} carries run_id \"{}\"; a task specification is not run-scoped and names no run_id",
                    ref_run_id.unwrap_or_default()
                ));
            }
        }
        RunIdRule::Required => {
            if !has_run_id {
                problems.push(format!(
                    "context manifest \"{id}\" {field} carries no run_id; a run-human-control record explicitly names the run it belongs to"
                ));
            }
            if has_run_id {
                if let Some(run_id) = run_id {
                    if ref_run_id != Some(run_id) {
                        problems.push(format!(
                            "context manifest \"{id}\" {field} run_id \"{}\" is not the referenced run \"{run_id}\"; the human-control record must belong to the same run",
                            ref_run_id.unwrap_or_default()
                        ));
                    }
                }
            }
        }
        RunIdRule::SelfRun => {
            if has_run_id && ref_run_id != ref_id {
                problems.push(format!(
                    "context manifest \"{id}\" {field} run_id \"{}\" is not the run's own id \"{}\"",
                    ref_run_id.unwrap_or_default(),
                    ref_id.unwrap_or_default()
                ));
            }
        }
    }
}

/// A list of `{ id, ... }` items — every id present, a semantic id, unique
/// within the manifest, every listed prose string portable. Port of
/// `checkIdentifiedItems`.
fn check_identified_items(
    problems: &mut Vec<String>,
    id: &str,
    label: &str,
    list: Option<&Value>,
    fields: &[&str],
) -> HashSet<String> {
    let mut seen = HashSet::new();
    let empty = Vec::new();
    let arr = list.and_then(Value::as_array).unwrap_or(&empty);
    for (i, item) in arr.iter().enumerate() {
        let it = if is_object(item) {
            item.clone()
        } else {
            Value::Null
        };
        let iid = it.get("id").and_then(Value::as_str);
        let at = iid.map(str::to_string).unwrap_or_else(|| format!("#{i}"));
        if !iid.is_some_and(is_semantic_id) {
            problems.push(format!(
                "context manifest \"{id}\" {label} entry {at} has no stable semantic id"
            ));
        } else {
            let iid = iid.unwrap();
            if !seen.insert(iid.to_string()) {
                problems.push(format!(
                    "context manifest \"{id}\" {label} id \"{iid}\" is used more than once"
                ));
            }
        }
        for f in fields {
            let val = it.get(*f);
            if val.map(blank).unwrap_or(true) {
                problems.push(format!(
                    "context manifest \"{id}\" {label} entry {at} has no {f}"
                ));
            } else if let Some(r) = non_portable_reason(val.and_then(Value::as_str)) {
                problems.push(format!(
                    "context manifest \"{id}\" {label} entry {at} {f} contains {r}"
                ));
            }
        }
    }
    seen
}

/// A list of unique portable reference strings. Port of `checkRefList`.
fn check_ref_list(
    problems: &mut Vec<String>,
    id: &str,
    label: &str,
    list: Option<&Value>,
) -> HashSet<String> {
    let mut seen = HashSet::new();
    let empty = Vec::new();
    let arr = list.and_then(Value::as_array).unwrap_or(&empty);
    for (i, r) in arr.iter().enumerate() {
        if blank(r) {
            problems.push(format!(
                "context manifest \"{id}\" {label}[{i}] is empty or whitespace-only"
            ));
            continue;
        }
        let r_str = r.as_str().unwrap_or("");
        if let Some(reason) = non_portable_reason(Some(r_str)) {
            problems.push(format!(
                "context manifest \"{id}\" {label}[{i}] contains {reason}"
            ));
        }
        let key = r_str.trim().to_string();
        if !seen.insert(key) {
            problems.push(format!(
                "context manifest \"{id}\" {label} repeats reference \"{r_str}\""
            ));
        }
    }
    seen
}

/// The external record-resolution boundary: `(pinnedRef) => resolvedEntry
/// | null`. Port of `makeRecordResolver`, built from a fixtures-style
/// resolution map keyed by `reference` string.
pub fn make_record_resolver(resolution_map: &Value) -> impl Fn(&Value) -> Option<Value> + '_ {
    move |ref_val: &Value| {
        if !is_object(ref_val) {
            return None;
        }
        let reference = ref_val.get("reference").and_then(Value::as_str)?;
        let entry = resolution_map.get(reference)?;
        if is_object(entry) {
            Some(entry.clone())
        } else {
            None
        }
    }
}

struct ConfirmedDigest {
    digest: Option<String>,
    inconsistent: Option<String>,
}

/// The SHA-256 the transformer confirms for a resolved edition. Port of
/// `confirmedDigest`.
fn confirmed_digest(entry: &Value) -> ConfirmedDigest {
    let cd = entry
        .get("content_digest")
        .and_then(Value::as_str)
        .filter(|s| SHA256_RE.is_match(s).unwrap_or(false));
    if let Some(source_bytes) = entry.get("source_bytes").and_then(Value::as_str) {
        let h = meridian_core::types::ContentDigest::of_str(source_bytes)
            .value()
            .to_string();
        if let Some(cd) = cd {
            if cd.to_lowercase() != h.to_lowercase() {
                return ConfirmedDigest {
                    digest: Some(h),
                    inconsistent: Some(cd.to_string()),
                };
            }
        }
        return ConfirmedDigest {
            digest: Some(h),
            inconsistent: None,
        };
    }
    if let Some(cd) = cd {
        return ConfirmedDigest {
            digest: Some(cd.to_string()),
            inconsistent: None,
        };
    }
    ConfirmedDigest {
        digest: None,
        inconsistent: None,
    }
}

/// Field-for-field equality of two `pinned_ref` objects. Port of
/// `samePinnedRef`.
fn same_pinned_ref(a: &Value, b: &Value) -> bool {
    if !is_object(a) || !is_object(b) {
        return false;
    }
    let norm = |v: &Value, k: &str| v.get(k).cloned().unwrap_or(Value::Null);
    a.get("record_type") == b.get("record_type")
        && a.get("id") == b.get("id")
        && norm(a, "run_id") == norm(b, "run_id")
        && a.get("reference") == b.get("reference")
        && norm(a, "revision") == norm(b, "revision")
        && norm(a, "sha256") == norm(b, "sha256")
}

/// Every field a `resolved_state` (execution-run) MUST carry, and NOTHING
/// else.
pub const REQUIRED_RESOLVED_STATE_FIELDS: [&str; 9] = [
    "task_specification_ref",
    "scope_revision",
    "lifecycle_stage",
    "work_status",
    "next_action",
    "next_gate",
    "blocker_ids",
    "resolved_norms",
    "completed_checks",
];

/// The CLOSED key set common to every resolved entry.
pub const RESOLVED_ENTRY_COMMON_KEYS: [&str; 6] = [
    "record_type",
    "id",
    "reference",
    "revision",
    "content_digest",
    "source_bytes",
];

fn allowed_entry_keys(wanted: Option<&str>) -> Vec<&'static str> {
    let mut keys = RESOLVED_ENTRY_COMMON_KEYS.to_vec();
    match wanted {
        Some("execution-run") => keys.push("resolved_state"),
        Some("run-human-control") => keys.push("linked_run_ref"),
        _ => {}
    }
    keys
}

/// One `resolved_state` axis that must be an ARRAY OF UNIQUE STRINGS. Port
/// of `checkResolvedStateArray`.
fn check_resolved_state_array(
    problems: &mut Vec<String>,
    id: &str,
    field: &str,
    axis: &str,
    value: Option<&Value>,
    kind: &str,
) {
    let at = format!("{field}: the resolved execution-run record's resolved_state.{axis}");
    let kind_label = if kind == "id" {
        "semantic-id"
    } else {
        "portable-reference"
    };
    let Some(arr) = value.and_then(Value::as_array) else {
        let shown = match value {
            None => "undefined".to_string(),
            Some(Value::Null) => "null".to_string(),
            Some(other) => js_typeof(other).to_string(),
        };
        problems.push(format!(
            "context manifest \"{id}\" {at} is {shown}, not an array of {kind_label} strings"
        ));
        return;
    };
    let mut seen = HashSet::new();
    for (i, el) in arr.iter().enumerate() {
        let Some(el_str) = el.as_str() else {
            problems.push(format!(
                "context manifest \"{id}\" {at}[{i}] is not a non-empty string"
            ));
            continue;
        };
        if el_str.trim().is_empty() {
            problems.push(format!(
                "context manifest \"{id}\" {at}[{i}] is not a non-empty string"
            ));
            continue;
        }
        let key = el_str.trim().to_string();
        if !seen.insert(key) {
            problems.push(format!(
                "context manifest \"{id}\" {at} repeats \"{el_str}\""
            ));
        }
        if kind == "id" {
            if !is_semantic_id(el_str) {
                problems.push(format!(
                    "context manifest \"{id}\" {at}[{i}] \"{el_str}\" is not a stable semantic id"
                ));
            }
        } else if let Some(r) = non_portable_reason(Some(el_str)) {
            problems.push(format!("context manifest \"{id}\" {at}[{i}] contains {r}"));
        }
    }
}

/// The CLOSED contract for an execution-run's `resolved_state`. Port of
/// `checkResolvedState`.
fn check_resolved_state(problems: &mut Vec<String>, id: &str, field: &str, rs: Option<&Value>) {
    let Some(rs) = rs.filter(|v| is_object(v)) else {
        problems.push(format!(
            "context manifest \"{id}\" {field}: the resolver returned an execution-run record with no resolved_state; the checkpoint axes cannot be checked against the run's own state"
        ));
        return;
    };
    for f in REQUIRED_RESOLVED_STATE_FIELDS {
        if rs.get(f).is_none() {
            problems.push(format!(
                "context manifest \"{id}\" {field}: the resolved execution-run record's resolved_state is missing required field \"{f}\""
            ));
        }
    }
    // Named, accepted boundary (the same one already documented on
    // `meridian-cli/examples/resolve_cli_producer.rs`'s own `result`
    // field): the Node reference's `Object.keys(rs)` iterates in the
    // object's actual source key order, while `serde_json::Map` is a
    // `BTreeMap` here (no `preserve_order` feature enabled anywhere) and
    // iterates alphabetically. When a resolver-supplied `resolved_state`
    // carries more than one unknown field at once, the two languages can
    // name a different one first; each still names every one of them, and
    // this ordering is never observed by any consumer other than this
    // diagnostic's own text.
    if let Some(obj) = rs.as_object() {
        for k in obj.keys() {
            if !REQUIRED_RESOLVED_STATE_FIELDS.contains(&k.as_str()) {
                problems.push(format!(
                    "context manifest \"{id}\" {field}: the resolved execution-run record's resolved_state carries an unknown field \"{k}\"; resolved_state is closed to its {} declared axes",
                    REQUIRED_RESOLVED_STATE_FIELDS.len()
                ));
            }
        }
    }
    if let Some(v) = rs.get("task_specification_ref") {
        if blank(v) {
            problems.push(format!(
                "context manifest \"{id}\" {field}: the resolved execution-run record's resolved_state.task_specification_ref is not a non-empty string"
            ));
        } else if let Some(r) = non_portable_reason(v.as_str()) {
            problems.push(format!(
                "context manifest \"{id}\" {field}: the resolved execution-run record's resolved_state.task_specification_ref contains {r}"
            ));
        }
    }
    if let Some(sr) = rs.get("scope_revision") {
        if !is_positive_int(sr) {
            problems.push(format!(
                "context manifest \"{id}\" {field}: the resolved execution-run record's resolved_state.scope_revision {} is not a positive integer",
                json_stringify(Some(sr))
            ));
        }
    }
    if let Some(ls) = rs.get("lifecycle_stage") {
        if !ls.as_str().is_some_and(|s| LIFECYCLE_STAGES.contains(&s)) {
            problems.push(format!(
                "context manifest \"{id}\" {field}: the resolved execution-run record's resolved_state.lifecycle_stage {} is not one of the closed lifecycle pool",
                json_stringify(Some(ls))
            ));
        }
    }
    if let Some(wsv) = rs.get("work_status") {
        if !wsv.as_str().is_some_and(|s| WORK_STATUSES.contains(&s)) {
            problems.push(format!(
                "context manifest \"{id}\" {field}: the resolved execution-run record's resolved_state.work_status {} is not one of the closed work-status pool",
                json_stringify(Some(wsv))
            ));
        }
    }
    for f in ["next_action", "next_gate"] {
        if let Some(v) = rs.get(f) {
            if !v.is_null() {
                if blank(v) {
                    problems.push(format!(
                        "context manifest \"{id}\" {field}: the resolved execution-run record's resolved_state.{f} is present but neither null nor a non-empty string"
                    ));
                } else if let Some(r) = non_portable_reason(v.as_str()) {
                    problems.push(format!(
                        "context manifest \"{id}\" {field}: the resolved execution-run record's resolved_state.{f} contains {r}"
                    ));
                }
            }
        }
    }
    if rs.get("blocker_ids").is_some() {
        check_resolved_state_array(
            problems,
            id,
            field,
            "blocker_ids",
            rs.get("blocker_ids"),
            "id",
        );
    }
    if rs.get("resolved_norms").is_some() {
        check_resolved_state_array(
            problems,
            id,
            field,
            "resolved_norms",
            rs.get("resolved_norms"),
            "ref",
        );
    }
    if rs.get("completed_checks").is_some() {
        check_resolved_state_array(
            problems,
            id,
            field,
            "completed_checks",
            rs.get("completed_checks"),
            "ref",
        );
    }
}

/// Resolve ONE pinned reference through the boundary and check it against
/// the CLOSED transformer-response contract. Port of
/// `resolvePinnedReference`.
fn resolve_pinned_reference(
    problems: &mut Vec<String>,
    id: &str,
    field: &str,
    ref_val: &Value,
    resolve: &RecordResolver,
) -> Option<Value> {
    if !is_object(ref_val) {
        return None;
    }
    let slot = pinned_ref_slot(field);
    let wanted = slot.map(|s| s.record_type);
    let entry = resolve(ref_val);
    let Some(entry) = entry.filter(is_object) else {
        problems.push(format!(
            "context manifest \"{id}\" {field} does not resolve to an actual {} through the external resolver; a pinned reference that resolves to nothing is not a verified pin and the manifest fails closed",
            wanted.unwrap_or("record")
        ));
        return None;
    };

    // Same named, accepted key-order boundary as `check_resolved_state`
    // above.
    let allowed_keys = allowed_entry_keys(wanted);
    if let Some(obj) = entry.as_object() {
        for k in obj.keys() {
            if !allowed_keys.contains(&k.as_str()) {
                problems.push(format!(
                    "context manifest \"{id}\" {field}: the resolver returned a record with an unknown field \"{k}\"; the transformer response is closed to {{ {} }}",
                    allowed_keys.join(", ")
                ));
            }
        }
    }

    let record_type = entry.get("record_type").and_then(Value::as_str);
    match record_type.filter(|s| !s.is_empty()) {
        None => problems.push(format!(
            "context manifest \"{id}\" {field}: the resolver returned a record with no record_type; the transformer response is a closed contract"
        )),
        Some(rt) => {
            if let Some(w) = wanted {
                if rt != w {
                    problems.push(format!(
                        "context manifest \"{id}\" {field} resolves to a \"{rt}\" record, not \"{w}\""
                    ));
                }
            }
        }
    }

    let entry_id = entry.get("id").and_then(Value::as_str);
    match entry_id.filter(|s| !s.trim().is_empty()) {
        None => problems.push(format!(
            "context manifest \"{id}\" {field}: the resolver returned a {} with no id; a resolved record without an identity cannot be checked against the pin",
            wanted.unwrap_or("record")
        )),
        Some(eid) if !is_semantic_id(eid) => problems.push(format!(
            "context manifest \"{id}\" {field}: the resolver returned a {} whose id \"{eid}\" is not a stable semantic identifier",
            wanted.unwrap_or("record")
        )),
        Some(eid) => {
            if let Some(pin_id) = ref_val.get("id").and_then(Value::as_str) {
                if eid != pin_id {
                    problems.push(format!(
                        "context manifest \"{id}\" {field} pins id \"{pin_id}\" but the reference resolves to record id \"{eid}\""
                    ));
                }
            }
        }
    }

    if let Some(entry_ref) = entry.get("reference") {
        match entry_ref.as_str().filter(|s| !s.trim().is_empty()) {
            None => problems.push(format!(
                "context manifest \"{id}\" {field}: the resolver's stated reference is present but not a non-empty string"
            )),
            Some(er) => {
                if let Some(r) = non_portable_reason(Some(er)) {
                    problems.push(format!(
                        "context manifest \"{id}\" {field}: the resolver's stated reference contains {r}"
                    ));
                } else if let Some(pin_ref) = ref_val.get("reference").and_then(Value::as_str) {
                    if er != pin_ref {
                        problems.push(format!(
                            "context manifest \"{id}\" {field}: the resolver's stated reference \"{er}\" is not the resolved reference \"{pin_ref}\""
                        ));
                    }
                }
            }
        }
    }

    if let Some(cd) = entry.get("content_digest") {
        let valid = cd
            .as_str()
            .is_some_and(|s| SHA256_RE.is_match(s).unwrap_or(false));
        if !valid {
            problems.push(format!(
                "context manifest \"{id}\" {field}: the resolver's content_digest {} is not exactly 64 hexadecimal characters",
                json_stringify(Some(cd))
            ));
        }
    }
    if let Some(sb) = entry.get("source_bytes") {
        if sb.as_str().is_none() {
            let shown = if sb.is_null() {
                "null".to_string()
            } else {
                js_typeof(sb).to_string()
            };
            problems.push(format!(
                "context manifest \"{id}\" {field}: the resolver's source_bytes is {shown}, not a string"
            ));
        }
    }

    let has_revision_string = entry
        .get("revision")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.trim().is_empty());
    let mut revision_exact = false;
    if let Some(rev) = entry.get("revision") {
        match rev.as_str() {
            None => {
                let shown = match rev {
                    Value::Null => "null".to_string(),
                    Value::Array(_) => "an array".to_string(),
                    Value::Number(_) => "a number".to_string(),
                    Value::Bool(_) => "a boolean".to_string(),
                    Value::Object(_) => "an object".to_string(),
                    Value::String(_) => unreachable!(),
                };
                problems.push(format!(
                    "context manifest \"{id}\" {field}: the resolver's revision is {shown}, not a string"
                ));
            }
            Some(s) if s.trim().is_empty() => {
                problems.push(format!(
                    "context manifest \"{id}\" {field}: the resolver's revision is present but empty"
                ));
            }
            Some(s) => {
                if classify_revision(Some(s)) != "exact" {
                    problems.push(format!(
                        "context manifest \"{id}\" {field}: the resolver confirmed revision {}, which is not an exact edition (a full Git SHA, a strict v?X.Y.Z tag or a SHA-256 digest); the transformer confirms a fixed edition, not a moving reference",
                        json_stringify(Some(rev))
                    ));
                } else {
                    revision_exact = true;
                }
            }
        }
    }
    let confirmed = confirmed_digest(&entry);
    if !revision_exact && confirmed.digest.is_none() {
        problems.push(format!(
            "context manifest \"{id}\" {field}: the resolver confirmed neither an exact revision nor a content digest for this edition; the pinned edition is unverified"
        ));
    }
    if let Some(inconsistent) = &confirmed.inconsistent {
        problems.push(format!(
            "context manifest \"{id}\" {field}: the resolver's content_digest \"{inconsistent}\" does not match the SHA-256 of the resolved source bytes \"{}\"",
            confirmed.digest.as_deref().unwrap_or("")
        ));
    }
    if let Some(pin_rev) = ref_val
        .get("revision")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
    {
        if !has_revision_string {
            problems.push(format!(
                "context manifest \"{id}\" {field} pins revision \"{pin_rev}\" but the resolver confirmed no exact edition for this reference"
            ));
        } else {
            let entry_rev = entry.get("revision").and_then(Value::as_str).unwrap_or("");
            if entry_rev != pin_rev {
                problems.push(format!(
                    "context manifest \"{id}\" {field} pins revision \"{pin_rev}\" but the resolver confirmed edition \"{entry_rev}\""
                ));
            }
        }
    }
    if let Some(pin_sha) = ref_val
        .get("sha256")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
    {
        match &confirmed.digest {
            None => problems.push(format!(
                "context manifest \"{id}\" {field} pins sha256 \"{pin_sha}\" but the resolver confirmed no content digest for this edition; a digest is verified against resolved source, never against a second copy of itself"
            )),
            Some(d) => {
                if d.to_lowercase() != pin_sha.to_lowercase() {
                    problems.push(format!(
                        "context manifest \"{id}\" {field} pins sha256 \"{pin_sha}\" but the SHA-256 of the resolved source is \"{d}\""
                    ));
                }
            }
        }
    }

    if wanted == Some("execution-run") {
        check_resolved_state(problems, id, field, entry.get("resolved_state"));
    }
    if wanted == Some("run-human-control") {
        let lr = entry
            .get("linked_run_ref")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty());
        match lr {
            None => problems.push(format!(
                "context manifest \"{id}\" {field}: the resolver returned a run-human-control record with no linked_run_ref; the run it belongs to is unconfirmed"
            )),
            Some(lr) => {
                if let Some(r) = non_portable_reason(Some(lr)) {
                    problems.push(format!(
                        "context manifest \"{id}\" {field}: the resolved run-human-control record's linked_run_ref contains {r}"
                    ));
                }
            }
        }
    }

    Some(entry)
}

/// The manifest's and the checkpoint's reference for the same slot,
/// resolved SEPARATELY, must land on one canonical identity, edition and
/// digest. Port of `sameResolvedEdition`.
fn same_resolved_edition(
    problems: &mut Vec<String>,
    id: &str,
    slot_field: &str,
    manifest_entry: Option<&Value>,
    checkpoint_entry: Option<&Value>,
) {
    let Some(m) = manifest_entry.filter(|v| is_object(v)) else {
        return;
    };
    let Some(c) = checkpoint_entry.filter(|v| is_object(v)) else {
        return;
    };
    if let (Some(mi), Some(ci)) = (
        m.get("id").and_then(Value::as_str),
        c.get("id").and_then(Value::as_str),
    ) {
        if mi != ci {
            problems.push(format!(
                "context manifest \"{id}\" run_state_checkpoint.{slot_field} resolves to record \"{ci}\", not the manifest's \"{mi}\"; the checkpoint pins the SAME record, not a parallel one that also exists"
            ));
        }
    }
    let m_rt = m.get("record_type").cloned().unwrap_or(Value::Null);
    let c_rt = c.get("record_type").cloned().unwrap_or(Value::Null);
    if m_rt != c_rt {
        problems.push(format!(
            "context manifest \"{id}\" run_state_checkpoint.{slot_field} resolves to a \"{}\" record, not the manifest's \"{}\"",
            js_str(Some(&c_rt)),
            js_str(Some(&m_rt))
        ));
    }
    let m_rev = m.get("revision").cloned().unwrap_or(Value::Null);
    let c_rev = c.get("revision").cloned().unwrap_or(Value::Null);
    if m_rev != c_rev {
        problems.push(format!(
            "context manifest \"{id}\" run_state_checkpoint.{slot_field} resolves to edition {}, not the manifest's {}; both must pin the same edition",
            json_stringify(Some(&c_rev)),
            json_stringify(Some(&m_rev))
        ));
    }
    let dm = confirmed_digest(m).digest;
    let dc = confirmed_digest(c).digest;
    if let (Some(dm), Some(dc)) = (&dm, &dc) {
        if dm.to_lowercase() != dc.to_lowercase() {
            problems.push(format!(
                "context manifest \"{id}\" run_state_checkpoint.{slot_field} resolves to a source whose digest differs from the manifest's {slot_field}"
            ));
        }
    }
}

/// Set equality over the string members of two arrays. Port of
/// `sameRefSet`.
fn same_ref_set(a: Option<&Value>, b: Option<&Value>) -> bool {
    let (Some(a), Some(b)) = (a.and_then(Value::as_array), b.and_then(Value::as_array)) else {
        return true;
    };
    let sa: HashSet<String> = a
        .iter()
        .filter_map(Value::as_str)
        .map(|s| s.trim().to_string())
        .collect();
    let sb: HashSet<String> = b
        .iter()
        .filter_map(Value::as_str)
        .map(|s| s.trim().to_string())
        .collect();
    sa.len() == sb.len() && sa.iter().all(|x| sb.contains(x))
}

pub struct EvalSchemas<'a> {
    pub record_schema: &'a Value,
    pub envelope_schema: &'a Value,
}

/// The external record-resolution boundary's function type: `(pinnedRef) =>
/// resolvedEntry | null`.
pub type RecordResolver<'a> = dyn Fn(&Value) -> Option<Value> + 'a;

/// The whole composition pipeline for one context-manifest record. Port of
/// `evaluateContextManifest`.
pub fn evaluate_context_manifest(
    doc: &Value,
    schemas: &EvalSchemas,
    resolve_records: Option<&RecordResolver>,
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
                "context-manifest schema could not be applied: {error}"
            )]
        }
    }

    if !is_object(doc) {
        if problems.is_empty() {
            return vec!["the context manifest record is not an object".to_string()];
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
        None => problems.push(format!(
            "context manifest \"{id}\" declares no $schema; a record names {EXPECTED_SCHEMA_BASENAME} so a consumer validates the whole contract — id, scope, origin, authority and body — not only the envelope"
        )),
        Some(declared) => {
            if let Some(portability) = non_portable_reason(Some(declared)) {
                problems.push(format!(
                    "context manifest \"{id}\" $schema \"{declared}\" is not portable ({portability}); the schema is named by a relative reference resolved inside the Meridian namespace (canonical logical base {CANONICAL_RECORD_BASE})"
                ));
            } else {
                let resolved = resolve_schema_ref(Some(declared));
                if resolved.as_deref() == Some(envelope_schema_ref().as_str()) {
                    problems.push(format!(
                        "context manifest \"{id}\" $schema \"{declared}\" resolves to the record envelope ({ENVELOPE_SCHEMA_BASENAME}); it must name {EXPECTED_SCHEMA_BASENAME}, which composes the envelope with the body"
                    ));
                } else if resolved.as_deref() != Some(expected_schema_ref().as_str()) {
                    let expected = expected_schema_ref();
                    problems.push(format!(
                        "context manifest \"{id}\" $schema \"{declared}\" does not resolve to the logical address {expected} within the Meridian namespace; the specialised schema is named by a portable relative reference (a bare basename, a missing namespace segment, a wrong segment and a reference climbing out of the namespace all resolve elsewhere)"
                    ));
                }
            }
        }
    }

    let record_type = doc.get("record_type").and_then(Value::as_str).unwrap_or("");
    if record_type != RECORD_TYPE {
        problems.push(format!(
            "context manifest \"{id}\" declares record_type \"{record_type}\", not \"{RECORD_TYPE}\"; the record type names the manifest and does not open a second envelope"
        ));
    }
    if !doc
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(is_semantic_id)
    {
        problems.push(format!(
            "context manifest \"{id}\" has no stable semantic id on the record envelope"
        ));
    }
    let title = doc.get("title").and_then(Value::as_str);
    match title {
        None => problems.push(format!("context manifest \"{id}\" has no human-readable title")),
        Some(t) if t.trim().is_empty() => {
            problems.push(format!("context manifest \"{id}\" has no human-readable title"))
        }
        Some(t) if !has_cyrillic(t) => problems.push(format!(
            "context manifest \"{id}\" title \"{t}\" carries no Russian (Cyrillic) text; the manifest name is stated in Russian for the human reader"
        )),
        _ => {}
    }
    if let Some(r) = non_portable_reason(title) {
        problems.push(format!("context manifest \"{id}\" title contains {r}"));
    }

    let empty_obj = Value::Object(Default::default());
    let scope = doc
        .get("scope")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);
    let scope_type = scope.get("type").and_then(Value::as_str);
    if let Some(scope_type_str) = scope_type {
        if scope_type_str != ALLOWED_SCOPE_TYPE {
            let why = scope_rejection_reason(scope_type_str);
            problems.push(format!(
                "context manifest \"{id}\" is scoped to \"{scope_type_str}\"; a context manifest lives in run-state only — {why}"
            ));
        }
    }
    if scope_type == Some(ALLOWED_SCOPE_TYPE) {
        if !scope
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(is_semantic_id)
        {
            problems.push(format!(
                "context manifest \"{id}\" run-state scope carries no stable scope.id identifying the run"
            ));
        }
        if !scope
            .get("workspace_id")
            .and_then(Value::as_str)
            .is_some_and(is_semantic_id)
        {
            problems.push(format!(
                "context manifest \"{id}\" run-state scope carries no workspace_id; a run belongs to a project workspace (workspace-scope-model.md §1)"
            ));
        }
    }

    let origin = doc
        .get("origin")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);
    if origin.get("kind").and_then(Value::as_str) == Some("built-in") {
        problems.push(format!(
            "context manifest \"{id}\" declares origin.kind \"built-in\"; a manifest is written in a workspace, not shipped with the methodology"
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
                    "context manifest \"{id}\" {label} contains {r}; a manifest is portable and carries no rooted machine path"
                ));
            }
        }
    }

    let payload = doc
        .get("payload")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);

    for f in all_forbidden_fields() {
        if payload.get(f).is_some() {
            problems.push(format!(
                "context manifest \"{id}\" payload carries \"{f}\"; the manifest is a bounded list of sources and state — an unbounded material dump, a full evidence / handoff contract or the body of a referenced record belongs elsewhere, not here"
            ));
        }
    }
    if payload.get("record_type").is_some() {
        problems.push(format!(
            "context manifest \"{id}\" repeats record_type inside the payload; the record type is declared once, on the envelope"
        ));
    }

    let run_ref = payload.get("execution_run_ref");
    let run_id: Option<String> = run_ref
        .filter(|v| is_object(v))
        .and_then(|v| v.get("id"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    for field in [
        "execution_run_ref",
        "task_specification_ref",
        "human_control_ref",
    ] {
        match payload.get(field) {
            None => problems.push(format!(
                "context manifest \"{id}\" names no {field}; a manifest carries a structured pinned reference to exactly one such record"
            )),
            Some(v) => check_pinned_ref(&mut problems, &id, field, v, run_id.as_deref()),
        }
    }
    if scope_type == Some(ALLOWED_SCOPE_TYPE) {
        if let Some(scope_id) = scope
            .get("id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            if let Some(run_id) = &run_id {
                if scope_id != run_id {
                    problems.push(format!(
                        "context manifest \"{id}\" scope identifies run \"{scope_id}\" but execution_run_ref.id is \"{run_id}\"; the two must be the exact same string, not one a substring of the other"
                    ));
                }
            }
        }
    }

    let sources = payload
        .get("authoritative_sources")
        .and_then(Value::as_array);
    match sources.filter(|s| !s.is_empty()) {
        None => problems.push(format!(
            "context manifest \"{id}\" lists no authoritative sources; a resumable run names at least the inputs it needs to continue"
        )),
        Some(sources) => {
            let mut seen_id = HashSet::new();
            let mut seen_ref = HashSet::new();
            for (i, s) in sources.iter().enumerate() {
                let so = if is_object(s) { s.clone() } else { Value::Null };
                let sid = so.get("id").and_then(Value::as_str);
                let at = sid.map(str::to_string).unwrap_or_else(|| format!("#{i}"));
                if !sid.is_some_and(is_semantic_id) {
                    problems.push(format!(
                        "context manifest \"{id}\" authoritative source {at} has no stable semantic id"
                    ));
                } else {
                    let sid = sid.unwrap();
                    if !seen_id.insert(sid.to_string()) {
                        problems.push(format!(
                            "context manifest \"{id}\" authoritative source id \"{sid}\" is used more than once"
                        ));
                    }
                }
                let reference = so.get("reference");
                if reference.map(blank).unwrap_or(true) {
                    problems.push(format!(
                        "context manifest \"{id}\" authoritative source {at} has no reference"
                    ));
                } else {
                    let ref_str = reference.and_then(Value::as_str).unwrap_or("");
                    if let Some(r) = non_portable_reason(Some(ref_str)) {
                        problems.push(format!(
                            "context manifest \"{id}\" authoritative source {at} reference contains {r}"
                        ));
                    }
                    let key = ref_str.trim().to_string();
                    if !seen_ref.insert(key) {
                        problems.push(format!(
                            "context manifest \"{id}\" authoritative source reference \"{ref_str}\" is listed more than once"
                        ));
                    }
                }
                let purpose = so.get("purpose");
                if purpose.map(blank).unwrap_or(true) {
                    problems.push(format!(
                        "context manifest \"{id}\" authoritative source {at} states no purpose; a source not needed to continue is not listed"
                    ));
                } else if let Some(r) = non_portable_reason(purpose.and_then(Value::as_str)) {
                    problems.push(format!(
                        "context manifest \"{id}\" authoritative source {at} purpose contains {r}"
                    ));
                }
                let revision = so.get("revision").and_then(Value::as_str);
                if let Some(rev) = revision.filter(|s| !s.is_empty()) {
                    if let Some(r) = non_portable_reason(Some(rev)) {
                        problems.push(format!(
                            "context manifest \"{id}\" authoritative source {at} revision contains {r}"
                        ));
                    }
                }
                if so.get("mutable") == Some(&Value::Bool(true)) {
                    let sha256 = so.get("sha256").and_then(Value::as_str);
                    if let Some(defect) = pin_defect(revision, sha256) {
                        problems.push(format!(
                            "context manifest \"{id}\" authoritative source {at} is mutable but not pinned: {defect}"
                        ));
                    }
                } else if classify_revision(revision) == "floating" {
                    problems.push(format!(
                        "context manifest \"{id}\" authoritative source {at} is declared immutable but its revision {} is a branch or channel reference",
                        json_quote(revision)
                    ));
                }
            }
        }
    }

    let norms = payload
        .get("applicable_norms")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut norm_refs: HashSet<String> = HashSet::new();
    {
        let mut seen_id = HashSet::new();
        for (i, n) in norms.iter().enumerate() {
            let no = if is_object(n) { n.clone() } else { Value::Null };
            let nid = no.get("id").and_then(Value::as_str);
            let at = nid.map(str::to_string).unwrap_or_else(|| format!("#{i}"));
            if !nid.is_some_and(is_semantic_id) {
                problems.push(format!(
                    "context manifest \"{id}\" applicable norm {at} has no stable semantic id"
                ));
            } else {
                let nid = nid.unwrap();
                if !seen_id.insert(nid.to_string()) {
                    problems.push(format!(
                        "context manifest \"{id}\" applicable norm id \"{nid}\" is used more than once"
                    ));
                }
            }
            for (f, human) in [
                ("reference", "reference"),
                ("origin", "origin"),
                ("applicability_rationale", "applicability rationale"),
            ] {
                let val = no.get(f);
                if val.map(blank).unwrap_or(true) {
                    problems.push(format!(
                        "context manifest \"{id}\" applicable norm {at} has no {human}"
                    ));
                } else if let Some(r) = non_portable_reason(val.and_then(Value::as_str)) {
                    problems.push(format!(
                        "context manifest \"{id}\" applicable norm {at} {f} contains {r}"
                    ));
                }
            }
            if let Some(reference) = no
                .get("reference")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
            {
                let key = reference.trim().to_string();
                if !norm_refs.insert(key) {
                    problems.push(format!(
                        "context manifest \"{id}\" applicable norm reference \"{reference}\" is listed more than once"
                    ));
                }
            }
            let revision = no.get("revision").and_then(Value::as_str);
            if let Some(rev) = revision.filter(|s| !s.is_empty()) {
                if let Some(r) = non_portable_reason(Some(rev)) {
                    problems.push(format!(
                        "context manifest \"{id}\" applicable norm {at} revision contains {r}"
                    ));
                }
            }
            let sha256 = no.get("sha256").and_then(Value::as_str);
            if let Some(defect) = pin_defect(revision, sha256) {
                problems.push(format!(
                    "context manifest \"{id}\" applicable norm {at} is not pinned: {defect}"
                ));
            }
        }
    }

    check_identified_items(
        &mut problems,
        &id,
        "decisions",
        payload.get("decisions"),
        &["statement"],
    );
    check_identified_items(
        &mut problems,
        &id,
        "open_questions",
        payload.get("open_questions"),
        &["question"],
    );
    check_identified_items(
        &mut problems,
        &id,
        "known_gaps",
        payload.get("known_gaps"),
        &["description"],
    );

    check_ref_list(
        &mut problems,
        &id,
        "completed_actions",
        payload.get("completed_actions"),
    );
    let manifest_completed_checks_seen = check_ref_list(
        &mut problems,
        &id,
        "completed_checks",
        payload.get("completed_checks"),
    );
    let _ = &manifest_completed_checks_seen;

    let blockers = payload
        .get("blockers")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut blocker_ids: HashSet<String> = HashSet::new();
    for (i, b) in blockers.iter().enumerate() {
        let bo = if is_object(b) { b.clone() } else { Value::Null };
        let bid = bo.get("id").and_then(Value::as_str);
        let at = bid.map(str::to_string).unwrap_or_else(|| format!("#{i}"));
        if !bid.is_some_and(is_semantic_id) {
            problems.push(format!(
                "context manifest \"{id}\" blocker {at} has no stable id"
            ));
        } else {
            let bid = bid.unwrap();
            if !blocker_ids.insert(bid.to_string()) {
                problems.push(format!(
                    "context manifest \"{id}\" blocker id \"{bid}\" is used more than once"
                ));
            }
        }
        if blank_opt(bo.get("description")) {
            problems.push(format!(
                "context manifest \"{id}\" blocker {at} has no description"
            ));
        }
        if blank_opt(bo.get("resumption_condition")) {
            problems.push(format!(
                "context manifest \"{id}\" blocker {at} has no verifiable resumption condition"
            ));
        }
        for s in [
            bo.get("description").and_then(Value::as_str),
            bo.get("resumption_condition").and_then(Value::as_str),
        ] {
            if let Some(r) = non_portable_reason(s) {
                problems.push(format!(
                    "context manifest \"{id}\" blocker {at} contains {r}"
                ));
            }
        }
    }

    let cp = payload.get("run_state_checkpoint").filter(|v| is_object(v));
    if cp.is_none() && payload.get("run_state_checkpoint").is_some() {
        problems.push(format!(
            "context manifest \"{id}\" run_state_checkpoint is not an object"
        ));
    }
    let mut cp_norms: HashSet<String> = HashSet::new();
    if let Some(cp) = cp {
        for field in [
            "execution_run_ref",
            "task_specification_ref",
            "human_control_ref",
        ] {
            match cp.get(field) {
                None => problems.push(format!(
                    "context manifest \"{id}\" run_state_checkpoint names no {field}; the pinned snapshot carries its own pinned references"
                )),
                Some(v) => {
                    check_pinned_ref(
                        &mut problems,
                        &id,
                        &format!("run_state_checkpoint.{field}"),
                        v,
                        run_id.as_deref(),
                    );
                    if let Some(payload_field) = payload.get(field) {
                        if !same_pinned_ref(v, payload_field) {
                            problems.push(format!(
                                "context manifest \"{id}\" run_state_checkpoint.{field} does not equal the manifest's {field} field for field; the checkpoint pins the SAME record edition as the manifest, not a parallel one"
                            ));
                        }
                    }
                }
            }
        }

        if let Some(ls) = cp.get("lifecycle_stage").and_then(Value::as_str) {
            if !LIFECYCLE_STAGES.contains(&ls) {
                problems.push(format!(
                    "context manifest \"{id}\" run_state_checkpoint.lifecycle_stage \"{ls}\" is not one of the closed lifecycle pool"
                ));
            }
        }
        if let Some(ws) = cp.get("work_status").and_then(Value::as_str) {
            if !WORK_STATUSES.contains(&ws) {
                problems.push(format!(
                    "context manifest \"{id}\" run_state_checkpoint.work_status \"{ws}\" is not one of the closed work-status pool"
                ));
            }
        }
        if let Some(sr) = cp.get("scope_revision") {
            if !is_positive_int(sr) {
                problems.push(format!(
                    "context manifest \"{id}\" run_state_checkpoint.scope_revision \"{}\" is not a positive integer",
                    js_str(Some(sr))
                ));
            }
        }
        for f in ["next_action", "next_gate"] {
            if let Some(v) = cp.get(f) {
                if !v.is_null() && blank(v) {
                    problems.push(format!(
                        "context manifest \"{id}\" run_state_checkpoint.{f} is present but empty; state an action, or null where there is no continuation"
                    ));
                }
            }
            if let Some(r) = non_portable_reason(cp.get(f).and_then(Value::as_str)) {
                problems.push(format!(
                    "context manifest \"{id}\" run_state_checkpoint.{f} contains {r}"
                ));
            }
        }
        let cp_checks = check_ref_list(
            &mut problems,
            &id,
            "run_state_checkpoint.completed_checks",
            cp.get("completed_checks"),
        );
        cp_norms = check_ref_list(
            &mut problems,
            &id,
            "run_state_checkpoint.resolved_norms",
            cp.get("resolved_norms"),
        );
        let cp_blocker_ids: HashSet<String> = cp
            .get("blocker_ids")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();

        if let (Some(psr), Some(csr)) = (payload.get("scope_revision"), cp.get("scope_revision")) {
            if is_positive_int(psr) && is_positive_int(csr) && psr != csr {
                problems.push(format!(
                    "context manifest \"{id}\" scope_revision {} contradicts the pinned run state (run_state_checkpoint.scope_revision {})",
                    js_str(Some(psr)),
                    js_str(Some(csr))
                ));
            }
        }
        for f in ["next_action", "next_gate"] {
            if payload.get(f).is_some()
                && cp.get(f).is_some()
                && !same_step_value(payload.get(f), cp.get(f))
            {
                problems.push(format!(
                    "context manifest \"{id}\" {f} {} contradicts the pinned run state (run_state_checkpoint.{f} {})",
                    json_stringify(payload.get(f)),
                    json_stringify(cp.get(f))
                ));
            }
        }
        if cp.get("blocker_ids").and_then(Value::as_array).is_some() {
            let extra_in_manifest: Vec<&String> = blocker_ids
                .iter()
                .filter(|x| !cp_blocker_ids.contains(*x))
                .collect();
            let missing_in_manifest: Vec<&String> = cp_blocker_ids
                .iter()
                .filter(|x| !blocker_ids.contains(*x))
                .collect();
            if !extra_in_manifest.is_empty() || !missing_in_manifest.is_empty() {
                problems.push(format!(
                    "context manifest \"{id}\" blockers do not match the pinned run state (run_state_checkpoint.blocker_ids); the two sets of blocker ids must be the same"
                ));
            }
        }
        if cp
            .get("completed_checks")
            .and_then(Value::as_array)
            .is_some()
            && payload
                .get("completed_checks")
                .and_then(Value::as_array)
                .is_some()
        {
            let manifest_checks: HashSet<String> = payload
                .get("completed_checks")
                .and_then(Value::as_array)
                .unwrap()
                .iter()
                .filter_map(Value::as_str)
                .map(|s| s.trim().to_string())
                .collect();
            let mismatch = manifest_checks.len() != cp_checks.len()
                || manifest_checks.iter().any(|x| !cp_checks.contains(x));
            if mismatch {
                problems.push(format!(
                    "context manifest \"{id}\" completed_checks do not match the pinned run state (run_state_checkpoint.completed_checks); the two sets must be the same"
                ));
            }
        }
        if cp.get("resolved_norms").and_then(Value::as_array).is_some() {
            let extra: Vec<&String> = norm_refs
                .iter()
                .filter(|x| !cp_norms.contains(*x))
                .collect();
            let missing: Vec<&String> = cp_norms
                .iter()
                .filter(|x| !norm_refs.contains(*x))
                .collect();
            if !extra.is_empty() || !missing.is_empty() {
                problems.push(format!(
                    "context manifest \"{id}\" applicable_norms references do not match the pinned run state (run_state_checkpoint.resolved_norms); the manifest adds an applicability rationale but cannot add to or drop from the run's resolved norm set"
                ));
            }
        }

        let ws = cp.get("work_status").and_then(Value::as_str);
        if ws == Some("blocked") && cp_blocker_ids.is_empty() {
            problems.push(format!(
                "context manifest \"{id}\" run_state_checkpoint work_status is \"blocked\" with no blocker id; \"blocked\" means continuation is impossible until a stated external condition changes"
            ));
        }
        if ws.is_some() && ws != Some("blocked") && !cp_blocker_ids.is_empty() {
            problems.push(format!(
                "context manifest \"{id}\" run_state_checkpoint carries {} blocker id(s) but work_status is \"{}\"; a non-empty blocker set agrees with work_status \"blocked\"",
                cp_blocker_ids.len(),
                ws.unwrap_or("")
            ));
        }
        if ws.is_some_and(|w| TERMINAL_STATUSES.contains(&w)) {
            for f in ["next_action", "next_gate"] {
                if let Some(v) = cp.get(f) {
                    if !v.is_null() {
                        problems.push(format!(
                            "context manifest \"{id}\" run_state_checkpoint work_status is \"{}\" but {f} carries an executable value; a completed or cancelled run does not silently carry a next executable step",
                            ws.unwrap_or("")
                        ));
                    }
                }
            }
        }
        if ws == Some("waiting_human") && blank_opt(cp.get("next_action")) {
            problems.push(format!(
                "context manifest \"{id}\" run_state_checkpoint work_status is \"waiting_human\" but next_action names no concrete human action; \"waiting_human\" means a specific required human action is known"
            ));
        }
    }

    // 14d. THE EXTERNAL RESOLUTION BOUNDARY.
    match resolve_records {
        None => problems.push(format!(
            "context manifest \"{id}\" cannot be verified: no external record resolver was supplied; the pinned execution-run, task-specification and run-human-control records are resolved OUTSIDE the manifest and checked against it — without that boundary the run_state_checkpoint is only a second unanchored copy"
        )),
        Some(resolve) => {
            let run_top = payload
                .get("execution_run_ref")
                .and_then(|r| resolve_pinned_reference(&mut problems, &id, "execution_run_ref", r, resolve));
            let spec_top = payload
                .get("task_specification_ref")
                .and_then(|r| resolve_pinned_reference(&mut problems, &id, "task_specification_ref", r, resolve));
            let hc_top = payload
                .get("human_control_ref")
                .and_then(|r| resolve_pinned_reference(&mut problems, &id, "human_control_ref", r, resolve));

            let mut run_cp: Option<Value> = None;
            let mut hc_cp: Option<Value> = None;
            if let Some(cp) = cp {
                run_cp = cp.get("execution_run_ref").and_then(|r| {
                    resolve_pinned_reference(&mut problems, &id, "run_state_checkpoint.execution_run_ref", r, resolve)
                });
                let spec_cp = cp.get("task_specification_ref").and_then(|r| {
                    resolve_pinned_reference(&mut problems, &id, "run_state_checkpoint.task_specification_ref", r, resolve)
                });
                hc_cp = cp.get("human_control_ref").and_then(|r| {
                    resolve_pinned_reference(&mut problems, &id, "run_state_checkpoint.human_control_ref", r, resolve)
                });

                same_resolved_edition(&mut problems, &id, "execution_run_ref", run_top.as_ref(), run_cp.as_ref());
                same_resolved_edition(&mut problems, &id, "task_specification_ref", spec_top.as_ref(), spec_cp.as_ref());
                same_resolved_edition(&mut problems, &id, "human_control_ref", hc_top.as_ref(), hc_cp.as_ref());
            }

            let run_for_axes = if cp.is_some() { run_cp.as_ref() } else { None };
            let run_state = run_for_axes
                .and_then(|r| r.get("resolved_state"))
                .filter(|v| is_object(v));

            if let (Some(run_state), Some(tsr)) = (run_state, payload.get("task_specification_ref").filter(|v| is_object(v))) {
                if let (Some(rs_ref), Some(payload_ref)) = (
                    run_state.get("task_specification_ref").and_then(Value::as_str),
                    tsr.get("reference").and_then(Value::as_str),
                ) {
                    if rs_ref != payload_ref {
                        problems.push(format!(
                            "context manifest \"{id}\" task_specification_ref names \"{payload_ref}\", but the resolved execution-run record names task specification \"{rs_ref}\"; the manifest carries the SAME specification the run resolved, not an independent one"
                        ));
                    }
                }
            }

            let hc_for_link = if cp.is_some() { hc_cp.as_ref() } else { None };
            if let (Some(hc_for_link), Some(run_ref)) = (hc_for_link, payload.get("execution_run_ref").filter(|v| is_object(v))) {
                if let (Some(linked), Some(payload_ref)) = (
                    hc_for_link.get("linked_run_ref").and_then(Value::as_str),
                    run_ref.get("reference").and_then(Value::as_str),
                ) {
                    if linked != payload_ref {
                        problems.push(format!(
                            "context manifest \"{id}\" human_control_ref resolves to a run-human-control record whose linked_run_ref is \"{linked}\", not the manifest's pinned run \"{payload_ref}\""
                        ));
                    }
                }
            }

            if let (Some(cp), Some(run_state)) = (cp, run_state) {
                for (axis, want) in [
                    ("scope_revision", run_state.get("scope_revision")),
                    ("lifecycle_stage", run_state.get("lifecycle_stage")),
                    ("work_status", run_state.get("work_status")),
                ] {
                    if let (Some(want), Some(have)) = (want, cp.get(axis)) {
                        if have != want {
                            problems.push(format!(
                                "context manifest \"{id}\" run_state_checkpoint.{axis} {} does not match the resolved execution-run record ({}); the pinned snapshot reproduces the run's own resolved state, not an unanchored copy",
                                json_stringify(Some(have)),
                                json_stringify(Some(want))
                            ));
                        }
                    }
                }
                for (axis, want) in [
                    ("next_action", run_state.get("next_action")),
                    ("next_gate", run_state.get("next_gate")),
                ] {
                    if let (Some(want), Some(have)) = (want, cp.get(axis)) {
                        if !same_step_value(Some(have), Some(want)) {
                            problems.push(format!(
                                "context manifest \"{id}\" run_state_checkpoint.{axis} {} does not match the resolved execution-run record ({}); the pinned snapshot reproduces the run's own resolved state, not an unanchored copy",
                                json_stringify(Some(have)),
                                json_stringify(Some(want))
                            ));
                        }
                    }
                }
                for axis in ["blocker_ids", "resolved_norms", "completed_checks"] {
                    let want = run_state.get(axis);
                    let have = cp.get(axis);
                    if want.is_some_and(|w| w.is_array())
                        && have.is_some_and(|h| h.is_array())
                        && !same_ref_set(have, want)
                    {
                        problems.push(format!(
                            "context manifest \"{id}\" run_state_checkpoint.{axis} does not match the resolved execution-run record's {axis}; the pinned snapshot reproduces the run's own resolved set, not an unanchored copy"
                        ));
                    }
                }
            }
        }
    }
    let _ = &cp_norms;

    for f in ["next_action", "next_gate"] {
        if let Some(v) = payload.get(f) {
            if !v.is_null() && blank(v) {
                problems.push(format!(
                    "context manifest \"{id}\" {f} is present but empty; state an action, or null where there is no continuation"
                ));
            }
        }
        if let Some(r) = non_portable_reason(payload.get(f).and_then(Value::as_str)) {
            problems.push(format!("context manifest \"{id}\" {f} contains {r}"));
        }
    }

    problems
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn classify_revision_matches_the_closed_set() {
        assert_eq!(classify_revision(Some("a".repeat(40).as_str())), "exact");
        assert_eq!(classify_revision(Some("v1.2.3")), "exact");
        assert_eq!(classify_revision(Some(&"a".repeat(64))), "exact");
        assert_eq!(classify_revision(Some("main")), "floating");
        assert_eq!(classify_revision(Some("feature/x")), "floating");
        assert_eq!(classify_revision(Some("abc123")), "abbrev-sha");
        assert_eq!(classify_revision(Some("branch-2026")), "weak");
        assert_eq!(classify_revision(Some("")), "absent");
        assert_eq!(classify_revision(None), "absent");
    }

    #[test]
    fn pin_defect_accepts_a_valid_sha256_regardless_of_revision() {
        let sha = "a".repeat(64);
        assert!(pin_defect(Some("main"), Some(&sha)).is_none());
    }

    #[test]
    fn make_record_resolver_looks_up_by_reference() {
        let map = json!({ "records/example-run.md": { "record_type": "execution-run", "id": "example-run" } });
        let resolver = make_record_resolver(&map);
        let found = resolver(&json!({ "reference": "records/example-run.md" }));
        assert!(found.is_some());
        assert!(resolver(&json!({ "reference": "missing" })).is_none());
    }

    /// Named, accepted boundary (see the two `for k in obj.keys()` sites in
    /// `resolve_pinned_reference`): a resolved entry carrying several
    /// unknown fields at once is reported in `serde_json::Map`'s (a
    /// `BTreeMap` here) alphabetical order, not the Node reference's
    /// source-text order. This is a genuine Rust-native improvement —
    /// deterministic and reproducible across processes and platforms,
    /// unlike Node's own `Object.keys()` order, which JSON.parse derives
    /// from incidental source text — kept, not reverted. This test pins
    /// that the order is not just *an* arbitrary order but the SAME order
    /// every time: five independent calls over an entry whose two unknown
    /// fields are named to sort opposite to their construction order here
    /// (`aaaa_extra_field` inserted last, `zzzz_extra_field` inserted
    /// first) must all report `aaaa_extra_field` before `zzzz_extra_field`
    /// and must all produce byte-identical `problems`, not merely the same
    /// set.
    #[test]
    fn unknown_fields_on_a_resolved_entry_are_reported_in_stable_alphabetical_order_across_repeated_calls(
    ) {
        let mut entry = serde_json::Map::new();
        entry.insert("zzzz_extra_field".to_string(), json!("z"));
        entry.insert("record_type".to_string(), json!("execution-run"));
        entry.insert("id".to_string(), json!("example-run"));
        entry.insert(
            "reference".to_string(),
            json!("records/execution-run/example-run"),
        );
        entry.insert("revision".to_string(), json!("v1.0.0"));
        entry.insert(
            "resolved_state".to_string(),
            json!({
                "task_specification_ref": "records/task-specification/example",
                "scope_revision": 1,
                "lifecycle_stage": "execution",
                "work_status": "active",
                "next_action": null,
                "next_gate": null,
                "blocker_ids": [],
                "resolved_norms": [],
                "completed_checks": [],
            }),
        );
        entry.insert("aaaa_extra_field".to_string(), json!("a"));
        let entry = Value::Object(entry);

        let resolver = move |_: &Value| Some(entry.clone());
        let ref_val = json!({
            "record_type": "execution-run",
            "id": "example-run",
            "reference": "records/execution-run/example-run",
            "revision": "v1.0.0",
        });

        let mut first_run: Option<Vec<String>> = None;
        for _ in 0..5 {
            let mut problems = Vec::new();
            resolve_pinned_reference(
                &mut problems,
                "manifest-id",
                "execution_run_ref",
                &ref_val,
                &resolver,
            );
            let unknown: Vec<&String> = problems
                .iter()
                .filter(|p| p.contains("unknown field"))
                .collect();
            assert_eq!(unknown.len(), 2, "{problems:?}");
            assert!(
                unknown[0].contains("\"aaaa_extra_field\""),
                "expected alphabetical order (aaaa_extra_field first): {unknown:?}"
            );
            assert!(
                unknown[1].contains("\"zzzz_extra_field\""),
                "expected alphabetical order (zzzz_extra_field second): {unknown:?}"
            );
            match &first_run {
                None => first_run = Some(problems),
                Some(first) => assert_eq!(
                    first, &problems,
                    "repeated calls over the same input must produce byte-identical output, not just the same set"
                ),
            }
        }
    }

    #[test]
    fn the_real_kernel_schema_and_fixtures_agree() {
        let kernel_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let record_schema: Value = serde_json::from_str(
            &std::fs::read_to_string(
                kernel_root.join("registries/operating-model/context-manifest.schema.json"),
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
                    .join("registries/operating-model/fixtures/context-manifest.fixtures.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let schemas = EvalSchemas {
            record_schema: &record_schema,
            envelope_schema: &envelope_schema,
        };
        let resolver = make_record_resolver(&fixtures["resolution"]);
        for case in fixtures["valid"].as_array().unwrap() {
            let problems = evaluate_context_manifest(&case["spec"], &schemas, Some(&resolver));
            assert!(
                problems.is_empty(),
                "note={:?} problems={:?}",
                case.get("note"),
                problems
            );
        }
        for case in fixtures["invalid"].as_array().unwrap() {
            let problems = evaluate_context_manifest(&case["spec"], &schemas, Some(&resolver));
            assert!(!problems.is_empty(), "note={:?}", case.get("note"));
        }
    }
}
