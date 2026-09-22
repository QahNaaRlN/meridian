//! Business-contract migration of `scripts/lib/evidence-and-handoff.mjs`: the
//! composite-consistency algorithm behind the `evidence-and-handoff-contract`
//! check (package 7, subpackage 7c). `registries/operating-model/evidence-and-handoff.schema.json`
//! is the COMPLETE schema for the portable handoff of the STATE and RESULT of
//! ONE execution run (`record_type: evidence-and-handoff`). Handoff DATA is
//! Instance data; the Kernel ships the schema, the product-neutral fixtures
//! and this module.
//!
//! The handoff relates to EXACTLY ONE run, and the link is DETERMINISTIC, not
//! a substring heuristic:
//!   - `execution_run_ref`, `task_specification_ref`, `human_control_ref` and
//!     `context_manifest_ref` are CLOSED structured pinned references —
//!     `{ record_type, id, reference, run_id?, revision?, sha256? }` — never
//!     a plain string, never an embedded body;
//!   - `scope.id` equals `execution_run_ref.id` by exact string comparison;
//!   - `human_control_ref.run_id` and `context_manifest_ref.run_id` equal
//!     `execution_run_ref.id`; `task_specification_ref` carries no `run_id`;
//!   - the four pinned references are RESOLVED through a boundary the
//!     handoff does not control (a resolver the caller supplies), and the
//!     handoff's own axes — the task specification it names, the next step
//!     and gate, the blocker id set, the passed-check set — are checked
//!     against the state the RESOLVED execution-run record carries. With no
//!     resolver the handoff fails closed.
//!
//! The CLOSED, fail-closed exact-revision rule ([`classify_revision`]) and
//! the `$schema`/rooted-path rules are REUSED from
//! [`super::bounded_context_manifest`] and [`super::task_specification`] —
//! this module carries no divergent second copy of either. The same rule is
//! applied to every pinned reference and to the per-repository
//! `source_state` / `result_state` revisions.
//!
//! Boundaries this module enforces that the JSON Schema subset cannot — see
//! `evidence-and-handoff-contract.md` and this module's per-section comments
//! below for the full list: claimed results / verifiable assertions /
//! evidence are three separate id-linked lists; an assertion is verified
//! only when a resolved, pin-checked evidence entry externally confirms it;
//! a claimed result's established/not_established status is derived, never
//! asserted; passed/failed/unable mandatory checks are backed by resolved
//! result evidence bound to THIS check's own `check_ref`; every acceptance
//! criterion is addressed or explicitly uncovered, and the closed set is
//! checked against the resolved task specification; source/result state are
//! pinned per repository and cover the same set; the worktree disposition
//! uses the closed `not_created`/`removed`/`retained` set
//! (`version-control-flow.md` §5.4); `outcome.status` axis rules are
//! checked, including against the resolved run's own state.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::bounded_context_manifest::{
    classify_revision, RecordResolver, REQUIRED_RESOLVED_STATE_FIELDS, RESOLVED_ENTRY_COMMON_KEYS,
};
use super::execution_state::{LIFECYCLE_STAGES, TERMINAL_STATUSES, WORK_STATUSES};
use super::task_specification::{non_portable_reason, resolve_schema_ref};
use crate::source_format::json_schema;

pub use super::bounded_context_manifest::make_record_resolver as make_evidence_resolver;

pub const RECORD_TYPE: &str = "evidence-and-handoff";

pub const CANONICAL_RECORD_BASE: &str = "records/evidence-and-handoff";
pub const SCHEMA_NAMESPACE_DIR: &str = "registries/operating-model";
pub const EXPECTED_SCHEMA_BASENAME: &str = "evidence-and-handoff.schema.json";
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
        "built-in-methodology" => "built-in-methodology is Kernel methodology, not a place for one run's result handoff",
        "user-profile" => "user-profile holds a user's rules and settings, not a run's result handoff",
        "organization-profile" => "organization-profile holds an organisation's rules and settings, not a run's result handoff",
        "project-workspace" => "project-workspace holds the project's goals and decisions; a run's result handoff is scoped to run-state",
        "repository-scope" => "repository-scope holds facts true for one repository; a run may span repositories and its handoff is scoped to run-state",
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

fn pinned_ref_slot(field: &str) -> Option<PinnedRefSlot> {
    match field {
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
        "context_manifest_ref" => Some(PinnedRefSlot {
            record_type: "context-manifest",
            run_id: RunIdRule::Required,
        }),
        _ => None,
    }
}

/// What the external boundary confirms an evidence artefact actually shows.
pub const OBSERVED_RESULTS: [&str; 3] = ["confirmed", "contradicted", "inconclusive"];

/// Fields that belong to a LATER package, to a specialised evidence
/// contract, or that would turn the bounded handoff into an unbounded
/// archive. Named here so a leak produces a pointed message, not just
/// "additional property not allowed".
pub const FORBIDDEN_PAYLOAD_FIELDS: [&str; 24] = [
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
    "task_specification",
    "execution_run",
    "human_control",
    "context_manifest",
    "run_state_checkpoint",
    "transition_history",
    "switch_history",
    "role_assignments",
    "supervision_mode",
];
// Node's array continues: 'human_authority', 'authoritative_sources',
// 'applicable_norms', 'functional_parity_record', 'functional_parity_evidence',
// 'parity_comparison', 'baseline', 'post_change_result', 'smoke_record',
// 'smoke_evidence', 'regression_record', 'regression_evidence',
// 'preserved_contract', 'field_metrics', 'evaluation_metrics',
// 'observation_log' — a second array so the combined check list matches
// exactly without one unreadable literal.
pub const FORBIDDEN_PAYLOAD_FIELDS_2: [&str; 16] = [
    "human_authority",
    "authoritative_sources",
    "applicable_norms",
    "functional_parity_record",
    "functional_parity_evidence",
    "parity_comparison",
    "baseline",
    "post_change_result",
    "smoke_record",
    "smoke_evidence",
    "regression_record",
    "regression_evidence",
    "preserved_contract",
    "field_metrics",
    "evaluation_metrics",
    "observation_log",
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
static SHA256_RE: std::sync::LazyLock<fancy_regex::Regex> = std::sync::LazyLock::new(|| {
    fancy_regex::Regex::new(r"^[0-9a-fA-F]{64}$").expect("sha256 pattern compiles")
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
/// `(a ?? null) === (b ?? null)` over two optional step values. Port of
/// `sameStepValue`.
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

fn json_stringify(v: Option<&Value>) -> String {
    match v {
        None => "undefined".to_string(),
        Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "undefined".to_string()),
    }
}

fn json_quote(s: Option<&str>) -> String {
    match s {
        None => "null".to_string(),
        Some(s) => serde_json::to_string(s).unwrap_or_default(),
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

/// Validate one `pinned_ref` slot, structurally only (no resolver). Port of
/// `checkPinnedRef`.
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
            "evidence and handoff \"{id}\" {field} is not a structured pinned reference; it is a closed {{ record_type, id, reference, revision?/sha256? }} object, never a plain string and never an embedded body"
        ));
        return;
    }
    let record_type = ref_val.get("record_type");
    if record_type.and_then(Value::as_str) != Some(slot.record_type) {
        problems.push(format!(
            "evidence and handoff \"{id}\" {field} names record_type \"{}\", not \"{}\"",
            js_str(record_type),
            slot.record_type
        ));
    }
    let ref_id = ref_val.get("id").and_then(Value::as_str);
    if !ref_id.is_some_and(is_semantic_id) {
        problems.push(format!(
            "evidence and handoff \"{id}\" {field} has no stable semantic id"
        ));
    }
    let reference = ref_val.get("reference");
    if reference.map(blank).unwrap_or(true) {
        problems.push(format!(
            "evidence and handoff \"{id}\" {field} has no portable reference"
        ));
    } else if let Some(r) = non_portable_reason(reference.and_then(Value::as_str)) {
        problems.push(format!(
            "evidence and handoff \"{id}\" {field} reference contains {r}; the reference is portable and is not an absolute machine path"
        ));
    }
    let revision = ref_val.get("revision").and_then(Value::as_str);
    let sha256 = ref_val.get("sha256").and_then(Value::as_str);
    if let Some(defect) = pin_defect(revision, sha256) {
        problems.push(format!(
            "evidence and handoff \"{id}\" {field} is not pinned to an exact edition: {defect}"
        ));
    }
    if let Some(rev) = revision.filter(|s| !s.is_empty()) {
        if let Some(rr) = non_portable_reason(Some(rev)) {
            problems.push(format!(
                "evidence and handoff \"{id}\" {field} revision contains {rr}"
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
                    "evidence and handoff \"{id}\" {field} carries run_id \"{}\"; a task specification is not run-scoped and names no run_id",
                    ref_run_id.unwrap_or_default()
                ));
            }
        }
        RunIdRule::Required => {
            if !has_run_id {
                problems.push(format!(
                    "evidence and handoff \"{id}\" {field} carries no run_id; a {} record explicitly names the run it belongs to",
                    slot.record_type
                ));
            }
            if has_run_id {
                if let Some(run_id) = run_id {
                    if ref_run_id != Some(run_id) {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" {field} run_id \"{}\" is not the referenced run \"{run_id}\"; the {} record must belong to the same run",
                            ref_run_id.unwrap_or_default(),
                            slot.record_type
                        ));
                    }
                }
            }
        }
        RunIdRule::SelfRun => {
            if has_run_id && ref_run_id != ref_id {
                problems.push(format!(
                    "evidence and handoff \"{id}\" {field} run_id \"{}\" is not the run's own id \"{}\"",
                    ref_run_id.unwrap_or_default(),
                    ref_id.unwrap_or_default()
                ));
            }
        }
    }
}

/// Shared: a list of `{ id, ... }` items — every id present, a semantic id,
/// unique within the handoff, every listed prose string portable. Port of
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
                "evidence and handoff \"{id}\" {label} entry {at} has no stable semantic id"
            ));
        } else {
            let iid = iid.unwrap();
            if !seen.insert(iid.to_string()) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" {label} id \"{iid}\" is used more than once"
                ));
            }
        }
        for f in fields {
            let val = it.get(*f);
            if val.map(blank).unwrap_or(true) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" {label} entry {at} has no {f}"
                ));
            } else if let Some(r) = non_portable_reason(val.and_then(Value::as_str)) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" {label} entry {at} {f} contains {r}"
                ));
            }
        }
    }
    seen
}

// ---------------------------------------------------------------------------
// The external record-resolution boundary — the SAME closed contract as the
// bounded context manifest, extended with the `task-specification` and
// `evidence-result` completeness checks this record composes with. Port of
// `resolvePinnedReference`, `resolveEvidenceEntry` and their supporting
// functions.
// ---------------------------------------------------------------------------

struct ConfirmedDigest {
    digest: Option<String>,
    inconsistent: Option<String>,
}

/// The SHA-256 the transformer confirms for a resolved edition. Port of
/// `confirmedDigest` (evidence-and-handoff.mjs's own local copy).
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

/// The CLOSED key set of a resolved entry, extended per record type. Port of
/// `RESOLVED_ENTRY_KEYS_BY_TYPE`.
fn allowed_entry_keys(wanted: &str) -> Vec<&'static str> {
    let mut keys = RESOLVED_ENTRY_COMMON_KEYS.to_vec();
    match wanted {
        "execution-run" => keys.push("resolved_state"),
        "run-human-control" | "context-manifest" => keys.push("linked_run_ref"),
        "task-specification" => {
            keys.push("acceptance_criteria");
            keys.push("mandatory_checks");
        }
        "evidence-result" => {
            keys.push("observed_result");
            keys.push("covers");
            keys.push("check_ref");
            keys.push("specialised_contract");
            keys.push("recorded_verdict");
        }
        _ => {}
    }
    keys
}

/// One `resolved_state` axis that must be an ARRAY OF UNIQUE STRINGS. Port of
/// `checkResolvedStateArray` (evidence-and-handoff.mjs's own local copy).
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
            "evidence and handoff \"{id}\" {at} is {shown}, not an array of {kind_label} strings"
        ));
        return;
    };
    let mut seen = HashSet::new();
    for (i, el) in arr.iter().enumerate() {
        let Some(el_str) = el.as_str() else {
            problems.push(format!(
                "evidence and handoff \"{id}\" {at}[{i}] is not a non-empty string"
            ));
            continue;
        };
        if el_str.trim().is_empty() {
            problems.push(format!(
                "evidence and handoff \"{id}\" {at}[{i}] is not a non-empty string"
            ));
            continue;
        }
        let key = el_str.trim().to_string();
        if !seen.insert(key) {
            problems.push(format!(
                "evidence and handoff \"{id}\" {at} repeats \"{el_str}\""
            ));
        }
        if kind == "id" {
            if !is_semantic_id(el_str) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" {at}[{i}] \"{el_str}\" is not a stable semantic id"
                ));
            }
        } else if let Some(r) = non_portable_reason(Some(el_str)) {
            problems.push(format!(
                "evidence and handoff \"{id}\" {at}[{i}] contains {r}"
            ));
        }
    }
}

/// The CLOSED contract for an execution-run's `resolved_state`. Port of
/// `checkResolvedState` (evidence-and-handoff.mjs's own local copy).
fn check_resolved_state(problems: &mut Vec<String>, id: &str, field: &str, rs: Option<&Value>) {
    let Some(rs) = rs.filter(|v| is_object(v)) else {
        problems.push(format!(
            "evidence and handoff \"{id}\" {field}: the resolver returned an execution-run record with no resolved_state; the handoff axes cannot be checked against the run's own state"
        ));
        return;
    };
    for f in REQUIRED_RESOLVED_STATE_FIELDS {
        if rs.get(f).is_none() {
            problems.push(format!(
                "evidence and handoff \"{id}\" {field}: the resolved execution-run record's resolved_state is missing required field \"{f}\""
            ));
        }
    }
    // Named, accepted boundary (`bounded_context_manifest::check_resolved_state`
    // carries the same note): `Object.keys` iterates source key order while
    // `serde_json::Map` here is a `BTreeMap` and iterates alphabetically —
    // when more than one unknown field is present at once the two languages
    // can name a different one first, but each still names every one.
    if let Some(obj) = rs.as_object() {
        for k in obj.keys() {
            if !REQUIRED_RESOLVED_STATE_FIELDS.contains(&k.as_str()) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" {field}: the resolved execution-run record's resolved_state carries an unknown field \"{k}\"; resolved_state is closed to its {} declared axes",
                    REQUIRED_RESOLVED_STATE_FIELDS.len()
                ));
            }
        }
    }
    if let Some(v) = rs.get("task_specification_ref") {
        if blank(v) {
            problems.push(format!(
                "evidence and handoff \"{id}\" {field}: the resolved execution-run record's resolved_state.task_specification_ref is not a non-empty string"
            ));
        } else if let Some(r) = non_portable_reason(v.as_str()) {
            problems.push(format!(
                "evidence and handoff \"{id}\" {field}: the resolved execution-run record's resolved_state.task_specification_ref contains {r}"
            ));
        }
    }
    if let Some(sr) = rs.get("scope_revision") {
        if !is_positive_int(sr) {
            problems.push(format!(
                "evidence and handoff \"{id}\" {field}: the resolved execution-run record's resolved_state.scope_revision {} is not a positive integer",
                json_stringify(Some(sr))
            ));
        }
    }
    if let Some(ls) = rs.get("lifecycle_stage") {
        if !ls.as_str().is_some_and(|s| LIFECYCLE_STAGES.contains(&s)) {
            problems.push(format!(
                "evidence and handoff \"{id}\" {field}: the resolved execution-run record's resolved_state.lifecycle_stage {} is not one of the closed lifecycle pool",
                json_stringify(Some(ls))
            ));
        }
    }
    if let Some(wsv) = rs.get("work_status") {
        if !wsv.as_str().is_some_and(|s| WORK_STATUSES.contains(&s)) {
            problems.push(format!(
                "evidence and handoff \"{id}\" {field}: the resolved execution-run record's resolved_state.work_status {} is not one of the closed work-status pool",
                json_stringify(Some(wsv))
            ));
        }
    }
    for f in ["next_action", "next_gate"] {
        if let Some(v) = rs.get(f) {
            if !v.is_null() {
                if blank(v) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" {field}: the resolved execution-run record's resolved_state.{f} is present but neither null nor a non-empty string"
                    ));
                } else if let Some(r) = non_portable_reason(v.as_str()) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" {field}: the resolved execution-run record's resolved_state.{f} contains {r}"
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

/// The CLOSED contract for a resolved task-specification's
/// `acceptance_criteria`. Port of `checkResolvedAcceptanceCriteria`.
fn check_resolved_acceptance_criteria(
    problems: &mut Vec<String>,
    id: &str,
    field: &str,
    entry: &Value,
) {
    let Some(ac) = entry.get("acceptance_criteria").and_then(Value::as_array) else {
        problems.push(format!(
            "evidence and handoff \"{id}\" {field}: the resolver returned a task-specification record with no acceptance_criteria array; the handoff closes the loop with the specification's acceptance criteria and the transformer must confirm their identifiers"
        ));
        return;
    };
    if ac.is_empty() {
        problems.push(format!(
            "evidence and handoff \"{id}\" {field}: the resolved task-specification record's acceptance_criteria is empty; a canonical task specification requires a non-empty acceptance-criteria set, so a handoff that closes its coverage loop against an empty set is rejected"
        ));
    }
    let mut seen = HashSet::new();
    for (i, c) in ac.iter().enumerate() {
        let Some(c) = c.as_str().filter(|s| is_semantic_id(s)) else {
            problems.push(format!(
                "evidence and handoff \"{id}\" {field}: the resolved task-specification record's acceptance_criteria[{i}] is not a stable semantic id"
            ));
            continue;
        };
        if !seen.insert(c.to_string()) {
            problems.push(format!(
                "evidence and handoff \"{id}\" {field}: the resolved task-specification record's acceptance_criteria repeats \"{c}\""
            ));
        }
    }
}

/// The CLOSED contract for a resolved task-specification's
/// `mandatory_checks`. Port of `checkResolvedMandatoryChecks`.
fn check_resolved_mandatory_checks(
    problems: &mut Vec<String>,
    id: &str,
    field: &str,
    entry: &Value,
) {
    let Some(mc) = entry.get("mandatory_checks").and_then(Value::as_array) else {
        problems.push(format!(
            "evidence and handoff \"{id}\" {field}: the resolver returned a task-specification record with no mandatory_checks array; the handoff closes the FULL set of mandatory checks against the specification and the transformer must confirm their references"
        ));
        return;
    };
    let mut seen = HashSet::new();
    for (i, c) in mc.iter().enumerate() {
        let Some(c) = c.as_str().filter(|s| !s.trim().is_empty()) else {
            problems.push(format!(
                "evidence and handoff \"{id}\" {field}: the resolved task-specification record's mandatory_checks[{i}] is not a non-empty portable reference"
            ));
            continue;
        };
        if let Some(r) = non_portable_reason(Some(c)) {
            problems.push(format!(
                "evidence and handoff \"{id}\" {field}: the resolved task-specification record's mandatory_checks[{i}] contains {r}"
            ));
        }
        let key = c.trim().to_string();
        if !seen.insert(key) {
            problems.push(format!(
                "evidence and handoff \"{id}\" {field}: the resolved task-specification record's mandatory_checks repeats \"{c}\""
            ));
        }
    }
}

/// Resolve ONE pinned reference through the boundary and check it against
/// the CLOSED transformer-response contract, including the type-specific
/// completeness check. Port of `resolvePinnedReference`. `wanted` is always
/// explicit here (never slot-derived only) so the same function serves the
/// four named slots AND the synthetic `evidence-result` reference built by
/// [`resolve_evidence_entry`], which does its OWN extra completeness check
/// afterwards using the returned entry (mirroring the Node closure's
/// `opts.completeness` override).
fn resolve_pinned_reference(
    problems: &mut Vec<String>,
    id: &str,
    field: &str,
    ref_val: &Value,
    resolve: &RecordResolver,
    wanted: &str,
) -> Option<Value> {
    if !is_object(ref_val) {
        return None;
    }
    let entry = resolve(ref_val);
    let Some(entry) = entry.filter(is_object) else {
        problems.push(format!(
            "evidence and handoff \"{id}\" {field} does not resolve to an actual {wanted} through the external resolver; a pinned reference that resolves to nothing is not a verified pin and the handoff fails closed"
        ));
        return None;
    };

    let allowed_keys = allowed_entry_keys(wanted);
    if let Some(obj) = entry.as_object() {
        for k in obj.keys() {
            if !allowed_keys.contains(&k.as_str()) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" {field}: the resolver returned a record with an unknown field \"{k}\"; the transformer response is closed to {{ {} }}",
                    allowed_keys.join(", ")
                ));
            }
        }
    }

    let record_type = entry.get("record_type").and_then(Value::as_str);
    match record_type.filter(|s| !s.is_empty()) {
        None => problems.push(format!(
            "evidence and handoff \"{id}\" {field}: the resolver returned a record with no record_type; the transformer response is a closed contract"
        )),
        Some(rt) => {
            if rt != wanted {
                problems.push(format!(
                    "evidence and handoff \"{id}\" {field} resolves to a \"{rt}\" record, not \"{wanted}\""
                ));
            }
        }
    }

    let entry_id = entry.get("id").and_then(Value::as_str);
    match entry_id.filter(|s| !s.trim().is_empty()) {
        None => problems.push(format!(
            "evidence and handoff \"{id}\" {field}: the resolver returned a {wanted} with no id; a resolved record without an identity cannot be checked against the pin"
        )),
        Some(eid) if !is_semantic_id(eid) => problems.push(format!(
            "evidence and handoff \"{id}\" {field}: the resolver returned a {wanted} whose id \"{eid}\" is not a stable semantic identifier"
        )),
        Some(eid) => {
            if let Some(pin_id) = ref_val.get("id").and_then(Value::as_str) {
                if eid != pin_id {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" {field} pins id \"{pin_id}\" but the reference resolves to record id \"{eid}\""
                    ));
                }
            }
        }
    }

    if let Some(entry_ref) = entry.get("reference") {
        match entry_ref.as_str().filter(|s| !s.trim().is_empty()) {
            None => problems.push(format!(
                "evidence and handoff \"{id}\" {field}: the resolver's stated reference is present but not a non-empty string"
            )),
            Some(er) => {
                if let Some(r) = non_portable_reason(Some(er)) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" {field}: the resolver's stated reference contains {r}"
                    ));
                } else if let Some(pin_ref) = ref_val.get("reference").and_then(Value::as_str) {
                    if er != pin_ref {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" {field}: the resolver's stated reference \"{er}\" is not the resolved reference \"{pin_ref}\""
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
                "evidence and handoff \"{id}\" {field}: the resolver's content_digest {} is not exactly 64 hexadecimal characters",
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
                "evidence and handoff \"{id}\" {field}: the resolver's source_bytes is {shown}, not a string"
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
                    "evidence and handoff \"{id}\" {field}: the resolver's revision is {shown}, not a string"
                ));
            }
            Some(s) if s.trim().is_empty() => {
                problems.push(format!(
                    "evidence and handoff \"{id}\" {field}: the resolver's revision is present but empty"
                ));
            }
            Some(s) => {
                if classify_revision(Some(s)) != "exact" {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" {field}: the resolver confirmed revision {}, which is not an exact edition (a full Git SHA, a strict v?X.Y.Z tag or a SHA-256 digest); the transformer confirms a fixed edition, not a moving reference",
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
            "evidence and handoff \"{id}\" {field}: the resolver confirmed neither an exact revision nor a content digest for this edition; the pinned edition is unverified"
        ));
    }
    if let Some(inconsistent) = &confirmed.inconsistent {
        problems.push(format!(
            "evidence and handoff \"{id}\" {field}: the resolver's content_digest \"{inconsistent}\" does not match the SHA-256 of the resolved source bytes \"{}\"",
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
                "evidence and handoff \"{id}\" {field} pins revision \"{pin_rev}\" but the resolver confirmed no exact edition for this reference"
            ));
        } else {
            let entry_rev = entry.get("revision").and_then(Value::as_str).unwrap_or("");
            if entry_rev != pin_rev {
                problems.push(format!(
                    "evidence and handoff \"{id}\" {field} pins revision \"{pin_rev}\" but the resolver confirmed edition \"{entry_rev}\""
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
                "evidence and handoff \"{id}\" {field} pins sha256 \"{pin_sha}\" but the resolver confirmed no content digest for this edition; a digest is verified against resolved source, never against a second copy of itself"
            )),
            Some(d) => {
                if d.to_lowercase() != pin_sha.to_lowercase() {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" {field} pins sha256 \"{pin_sha}\" but the SHA-256 of the resolved source is \"{d}\""
                    ));
                }
            }
        }
    }

    match wanted {
        "execution-run" => check_resolved_state(problems, id, field, entry.get("resolved_state")),
        "run-human-control" | "context-manifest" => {
            let lr = entry
                .get("linked_run_ref")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty());
            match lr {
                None => problems.push(format!(
                    "evidence and handoff \"{id}\" {field}: the resolver returned a {wanted} record with no linked_run_ref; the run it belongs to is unconfirmed"
                )),
                Some(lr) => {
                    if let Some(r) = non_portable_reason(Some(lr)) {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" {field}: the resolved {wanted} record's linked_run_ref contains {r}"
                        ));
                    }
                }
            }
        }
        "task-specification" => {
            check_resolved_acceptance_criteria(problems, id, field, &entry);
            check_resolved_mandatory_checks(problems, id, field, &entry);
        }
        _ => {}
    }

    Some(entry)
}

/// The result of resolving one evidence entry: `clean` is true only when the
/// entry produced no problem, and an entry only makes an assertion verified
/// or confirms a check when `clean` and `observed_result` are right. Port of
/// the object `resolveEvidenceEntry` returns.
struct EvidenceResolution {
    clean: bool,
    observed_result: Option<String>,
    entry: Option<Value>,
}

/// Resolve ONE evidence entry through the boundary as an `evidence-result`
/// transformer response. Port of `resolveEvidenceEntry`.
fn resolve_evidence_entry(
    problems: &mut Vec<String>,
    id: &str,
    at: &str,
    eo: &Value,
    assertion_ids: &HashSet<String>,
    resolve: &RecordResolver,
) -> EvidenceResolution {
    let mut synth_ref = serde_json::json!({
        "record_type": "evidence-result",
        "id": eo.get("id").cloned().unwrap_or(Value::Null),
        "reference": eo.get("reference").cloned().unwrap_or(Value::Null),
    });
    if let Some(rev) = eo
        .get("revision")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        synth_ref["revision"] = Value::String(rev.to_string());
    }
    if let Some(sha) = eo
        .get("sha256")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        synth_ref["sha256"] = Value::String(sha.to_string());
    }

    let before = problems.len();
    let field = format!("evidence entry {at}");
    let entry =
        resolve_pinned_reference(problems, id, &field, &synth_ref, resolve, "evidence-result");

    if let Some(e) = &entry {
        if e.get("observed_result").is_none() {
            problems.push(format!(
                "evidence and handoff \"{id}\" evidence entry {at}: the resolved evidence-result confirms no observed_result; the transformer says whether the artefact confirmed, contradicted or was inconclusive about its subject"
            ));
        } else if !e
            .get("observed_result")
            .and_then(Value::as_str)
            .is_some_and(|s| OBSERVED_RESULTS.contains(&s))
        {
            problems.push(format!(
                "evidence and handoff \"{id}\" evidence entry {at}: the resolved evidence-result's observed_result {} is not one of {{ {} }}",
                json_stringify(e.get("observed_result")),
                OBSERVED_RESULTS.join(", ")
            ));
        }

        let mut confirmed_covers: Option<HashSet<String>> = None;
        if let Some(covers) = e.get("covers") {
            match covers.as_array() {
                None => problems.push(format!(
                    "evidence and handoff \"{id}\" evidence entry {at}: the resolved evidence-result's covers is {}, not an array of assertion ids",
                    if covers.is_null() { "null".to_string() } else { js_typeof(covers).to_string() }
                )),
                Some(arr) => {
                    let mut set = HashSet::new();
                    for (k, cv) in arr.iter().enumerate() {
                        let Some(cv_str) = cv.as_str().filter(|s| is_semantic_id(s)) else {
                            problems.push(format!(
                                "evidence and handoff \"{id}\" evidence entry {at}: the resolved evidence-result's covers[{k}] is not a stable semantic id"
                            ));
                            continue;
                        };
                        if !set.insert(cv_str.to_string()) {
                            problems.push(format!(
                                "evidence and handoff \"{id}\" evidence entry {at}: the resolved evidence-result's covers repeats \"{cv_str}\""
                            ));
                        }
                    }
                    confirmed_covers = Some(set);
                }
            }
        }

        if let Some(check_ref) = e.get("check_ref") {
            if blank(check_ref) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" evidence entry {at}: the resolved evidence-result's check_ref is present but not a non-empty reference"
                ));
            } else if let Some(r) = non_portable_reason(check_ref.as_str()) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" evidence entry {at}: the resolved evidence-result's check_ref contains {r}"
                ));
            }
        }

        let handoff_covers: Vec<String> = eo
            .get("covers")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .filter(|s| is_semantic_id(s))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        if !handoff_covers.is_empty() {
            match &confirmed_covers {
                None => problems.push(format!(
                    "evidence and handoff \"{id}\" evidence entry {at}: the resolved evidence-result confirms no covered subject; which assertion(s) an evidence entry bears on is confirmed by the transformer, not asserted by the handoff, and summary + covers from the handoff alone are not enough"
                )),
                Some(cc) => {
                    for cv in &handoff_covers {
                        if !cc.contains(cv) {
                            problems.push(format!(
                                "evidence and handoff \"{id}\" evidence entry {at}: the resolved evidence-result does not confirm that this evidence bears on assertion \"{cv}\"; a resolved evidence entry's covers is not transferred to a foreign assertion"
                            ));
                        }
                    }
                }
            }
        }
        let _ = assertion_ids; // handoffCovers already filtered by is_semantic_id, matching Node

        let kind = eo.get("kind").and_then(Value::as_str);
        if kind == Some("specialised-evidence-record") {
            for f in ["specialised_contract", "recorded_verdict"] {
                let ev = e.get(f);
                if ev.map(blank).unwrap_or(true) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" evidence entry {at}: the resolved specialised evidence record confirms no {f}; a specialised verdict is confirmed by the resolved record, not by the handoff's own words"
                    ));
                } else if ev != eo.get(f) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" evidence entry {at}: the resolved specialised evidence record's {f} \"{}\" does not match the handoff's stated {f} {}",
                        ev.and_then(Value::as_str).unwrap_or(""),
                        json_stringify(eo.get(f))
                    ));
                }
            }
        } else {
            for f in ["specialised_contract", "recorded_verdict"] {
                if e.get(f).is_some() {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" evidence entry {at}: the resolved evidence-result carries \"{f}\" but the evidence entry kind is \"{}\", not \"specialised-evidence-record\"",
                        kind.unwrap_or("undefined")
                    ));
                }
            }
        }
    }

    let clean = problems.len() == before;
    let observed_result = entry
        .as_ref()
        .and_then(|e| e.get("observed_result"))
        .and_then(Value::as_str)
        .filter(|s| OBSERVED_RESULTS.contains(s))
        .map(str::to_string);
    EvidenceResolution {
        clean,
        observed_result,
        entry,
    }
}

/// Set equality over the string members of two arrays. Port of
/// `sameRefSet`.
fn same_ref_set(a: &HashSet<String>, b: Option<&Value>) -> bool {
    let Some(arr) = b.and_then(Value::as_array) else {
        return true;
    };
    let sb: HashSet<String> = arr
        .iter()
        .filter_map(Value::as_str)
        .map(|s| s.trim().to_string())
        .collect();
    a.len() == sb.len() && a.iter().all(|x| sb.contains(x))
}

/// A repository's pin as a comparable tuple. Port of `repoPinKey`.
fn repo_pin_key(entry: &Value) -> String {
    let rev = entry
        .get("revision")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let sha = entry
        .get("sha256")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_lowercase();
    format!("{rev} {sha}")
}

/// One repository-state list. Port of `checkRepositoryStates`.
fn check_repository_states(
    problems: &mut Vec<String>,
    id: &str,
    label: &str,
    list: Option<&Value>,
) -> HashMap<String, Value> {
    let mut by_repo = HashMap::new();
    let empty = Vec::new();
    let arr = list.and_then(Value::as_array).unwrap_or(&empty);
    if arr.is_empty() {
        problems.push(format!(
            "evidence and handoff \"{id}\" {label} lists no repository state; source and result states are pinned per repository"
        ));
    }
    for (i, s) in arr.iter().enumerate() {
        let so = if is_object(s) { s.clone() } else { Value::Null };
        let repo = so.get("repository_ref").and_then(Value::as_str);
        let at = repo.map(str::to_string).unwrap_or_else(|| format!("#{i}"));
        if so.get("repository_ref").map(blank).unwrap_or(true) {
            problems.push(format!(
                "evidence and handoff \"{id}\" {label} entry {at} has no repository_ref"
            ));
        } else {
            let repo = repo.unwrap();
            if let Some(r) = non_portable_reason(Some(repo)) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" {label} entry {at} repository_ref contains {r}"
                ));
            }
            let key = repo.trim().to_string();
            match by_repo.entry(key) {
                std::collections::hash_map::Entry::Occupied(_) => {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" {label} repository_ref \"{repo}\" is listed more than once"
                    ));
                }
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(so.clone());
                }
            }
        }
        if let Some(rev) = so
            .get("revision")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            if let Some(r) = non_portable_reason(Some(rev)) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" {label} entry {at} revision contains {r}"
                ));
            }
        }
        let defect = pin_defect(
            so.get("revision").and_then(Value::as_str),
            so.get("sha256").and_then(Value::as_str),
        );
        if let Some(defect) = defect {
            problems.push(format!(
                "evidence and handoff \"{id}\" {label} entry {at} is not pinned to an exact revision: {defect}"
            ));
        }
    }
    by_repo
}

pub struct EvalSchemas<'a> {
    pub record_schema: &'a Value,
    pub envelope_schema: &'a Value,
}

/// The whole composition pipeline for one evidence-and-handoff record. Port
/// of `evaluateEvidenceAndHandoff`.
pub fn evaluate_evidence_and_handoff(
    doc: &Value,
    schemas: &EvalSchemas,
    resolve_records: Option<&RecordResolver>,
) -> Vec<String> {
    let mut problems = Vec::new();

    // 1. the canonical scoped-record envelope, validated separately and always.
    match json_schema::validate(doc, schemas.envelope_schema) {
        Ok(errors) => problems.extend(errors.into_iter().map(|m| format!("envelope {m}"))),
        Err(error) => {
            return vec![format!(
                "record envelope schema could not be applied: {error}"
            )]
        }
    }

    // 2. the complete specialised schema.
    match json_schema::validate(doc, schemas.record_schema) {
        Ok(errors) => problems.extend(errors),
        Err(error) => {
            return vec![format!(
                "evidence-and-handoff schema could not be applied: {error}"
            )]
        }
    }

    if !is_object(doc) {
        if problems.is_empty() {
            return vec!["the evidence and handoff record is not an object".to_string()];
        }
        return problems;
    }

    let id = doc
        .get("id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or("(no id)")
        .to_string();

    // 3. the record's $schema declaration.
    let declared = doc.get("$schema").and_then(Value::as_str);
    match declared {
        None => problems.push(format!(
            "evidence and handoff \"{id}\" declares no $schema; a record names {EXPECTED_SCHEMA_BASENAME} so a consumer validates the whole contract — id, scope, origin, authority and body — not only the envelope"
        )),
        Some(declared) => {
            if let Some(portability) = non_portable_reason(Some(declared)) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" $schema \"{declared}\" is not portable ({portability}); the schema is named by a relative reference resolved inside the Meridian namespace (canonical logical base {CANONICAL_RECORD_BASE})"
                ));
            } else {
                let resolved = resolve_schema_ref(Some(declared));
                if resolved.as_deref() == Some(envelope_schema_ref().as_str()) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" $schema \"{declared}\" resolves to the record envelope ({ENVELOPE_SCHEMA_BASENAME}); it must name {EXPECTED_SCHEMA_BASENAME}, which composes the envelope with the body"
                    ));
                } else if resolved.as_deref() != Some(expected_schema_ref().as_str()) {
                    let expected = expected_schema_ref();
                    problems.push(format!(
                        "evidence and handoff \"{id}\" $schema \"{declared}\" does not resolve to the logical address {expected} within the Meridian namespace; the specialised schema is named by a portable relative reference (a bare basename, a missing namespace segment, a wrong segment and a reference climbing out of the namespace all resolve elsewhere)"
                    ));
                }
            }
        }
    }

    // 4. identity and the human-readable Russian name.
    let record_type = doc.get("record_type").and_then(Value::as_str).unwrap_or("");
    if record_type != RECORD_TYPE {
        problems.push(format!(
            "evidence and handoff \"{id}\" declares record_type \"{record_type}\", not \"{RECORD_TYPE}\"; the record type names the handoff and does not open a second envelope"
        ));
    }
    if !doc
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(is_semantic_id)
    {
        problems.push(format!(
            "evidence and handoff \"{id}\" has no stable semantic id on the record envelope"
        ));
    }
    let title = doc.get("title").and_then(Value::as_str);
    match title {
        None => problems.push(format!("evidence and handoff \"{id}\" has no human-readable title")),
        Some(t) if t.trim().is_empty() => {
            problems.push(format!("evidence and handoff \"{id}\" has no human-readable title"))
        }
        Some(t) if !has_cyrillic(t) => problems.push(format!(
            "evidence and handoff \"{id}\" title \"{t}\" carries no Russian (Cyrillic) text; the handoff name is stated in Russian for the human reader"
        )),
        _ => {}
    }
    if let Some(r) = non_portable_reason(title) {
        problems.push(format!("evidence and handoff \"{id}\" title contains {r}"));
    }

    // 5. the handoff lives only in run-state.
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
                "evidence and handoff \"{id}\" is scoped to \"{scope_type_str}\"; an evidence-and-handoff record lives in run-state only — {why}"
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
                "evidence and handoff \"{id}\" run-state scope carries no stable scope.id identifying the run"
            ));
        }
        if !scope
            .get("workspace_id")
            .and_then(Value::as_str)
            .is_some_and(is_semantic_id)
        {
            problems.push(format!(
                "evidence and handoff \"{id}\" run-state scope carries no workspace_id; a run belongs to a project workspace (workspace-scope-model.md §1)"
            ));
        }
    }

    let origin = doc
        .get("origin")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);
    if origin.get("kind").and_then(Value::as_str) == Some("built-in") {
        problems.push(format!(
            "evidence and handoff \"{id}\" declares origin.kind \"built-in\"; a handoff is written in a workspace, not shipped with the methodology"
        ));
    }
    let authority = doc
        .get("authority")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);

    // 5a. the envelope reference strings travel inside the portable record too.
    for (obj, _field, label) in [
        (origin, "source_ref", "origin.source_ref"),
        (authority, "authority_ref", "authority.authority_ref"),
        (authority, "decision_ref", "authority.decision_ref"),
    ] {
        if let Some(val) = obj
            .get(_field)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            if let Some(r) = non_portable_reason(Some(val)) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" {label} contains {r}; a handoff is portable and carries no rooted machine path"
                ));
            }
        }
    }

    let payload = doc
        .get("payload")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);

    // 6. an unbounded material dump, a specialised-evidence body, or a field
    //    of a later package.
    for f in all_forbidden_fields() {
        if payload.get(f).is_some() {
            problems.push(format!(
                "evidence and handoff \"{id}\" payload carries \"{f}\"; the handoff is a bounded record of what happened and what to do next — an unbounded material dump, a full specialised-evidence body or the body of a referenced record belongs elsewhere, not here"
            ));
        }
    }
    if payload.get("record_type").is_some() {
        problems.push(format!(
            "evidence and handoff \"{id}\" repeats record_type inside the payload; the record type is declared once, on the envelope"
        ));
    }

    // 7. the four structured pinned references and the DETERMINISTIC single-run link.
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
        "context_manifest_ref",
    ] {
        match payload.get(field) {
            None => problems.push(format!(
                "evidence and handoff \"{id}\" names no {field}; a handoff carries a structured pinned reference to exactly one such record"
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
                        "evidence and handoff \"{id}\" scope identifies run \"{scope_id}\" but execution_run_ref.id is \"{run_id}\"; the two must be the exact same string, not one a substring of the other"
                    ));
                }
            }
        }
    }

    // 8. claimed results.
    let mut claimed_ids: HashSet<String> = HashSet::new();
    {
        let empty = Vec::new();
        let arr = payload
            .get("claimed_results")
            .and_then(Value::as_array)
            .unwrap_or(&empty);
        for (i, c) in arr.iter().enumerate() {
            let co = if is_object(c) { c.clone() } else { Value::Null };
            let cid = co.get("id").and_then(Value::as_str);
            let at = cid.map(str::to_string).unwrap_or_else(|| format!("#{i}"));
            if !cid.is_some_and(is_semantic_id) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" claimed result {at} has no stable semantic id"
                ));
            } else {
                let cid = cid.unwrap();
                if !claimed_ids.insert(cid.to_string()) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" claimed result id \"{cid}\" is used more than once"
                    ));
                }
            }
            if co.get("statement").map(blank).unwrap_or(true) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" claimed result {at} has no statement"
                ));
            } else if let Some(r) = non_portable_reason(co.get("statement").and_then(Value::as_str))
            {
                problems.push(format!(
                    "evidence and handoff \"{id}\" claimed result {at} statement contains {r}"
                ));
            }
        }
    }

    // 9. verifiable assertions.
    let mut assertion_ids: HashSet<String> = HashSet::new();
    let mut assertion_by_id: HashMap<String, Value> = HashMap::new();
    let mut assertions_by_result: HashMap<String, Vec<String>> = HashMap::new();
    {
        let empty = Vec::new();
        let arr = payload
            .get("verifiable_assertions")
            .and_then(Value::as_array)
            .unwrap_or(&empty);
        for (i, a) in arr.iter().enumerate() {
            let ao = if is_object(a) { a.clone() } else { Value::Null };
            let aid = ao.get("id").and_then(Value::as_str);
            let at = aid.map(str::to_string).unwrap_or_else(|| format!("#{i}"));
            if !aid.is_some_and(is_semantic_id) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" verifiable assertion {at} has no stable semantic id"
                ));
            } else {
                let aid = aid.unwrap().to_string();
                if !assertion_ids.insert(aid.clone()) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" verifiable assertion id \"{aid}\" is used more than once"
                    ));
                }
                assertion_by_id.insert(aid, ao.clone());
            }
            if ao.get("statement").map(blank).unwrap_or(true) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" verifiable assertion {at} has no statement"
                ));
            } else if let Some(r) = non_portable_reason(ao.get("statement").and_then(Value::as_str))
            {
                problems.push(format!(
                    "evidence and handoff \"{id}\" verifiable assertion {at} statement contains {r}"
                ));
            }
            let crid = ao.get("claimed_result_id").and_then(Value::as_str);
            if !crid.is_some_and(is_semantic_id) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" verifiable assertion {at} names no claimed_result_id"
                ));
            } else {
                let crid = crid.unwrap();
                if !claimed_ids.contains(crid) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" verifiable assertion {at} names claimed_result_id \"{crid}\", which is not a declared claimed result"
                    ));
                } else if let Some(aid) = aid.filter(|s| is_semantic_id(s)) {
                    assertions_by_result
                        .entry(crid.to_string())
                        .or_default()
                        .push(aid.to_string());
                }
            }
            let status = ao.get("status").and_then(Value::as_str);
            if status == Some("unverified") {
                if ao.get("unverified_reason").map(blank).unwrap_or(true) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" verifiable assertion {at} is unverified but states no unverified_reason; the unproven stays explicitly unproven"
                    ));
                } else if let Some(r) =
                    non_portable_reason(ao.get("unverified_reason").and_then(Value::as_str))
                {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" verifiable assertion {at} unverified_reason contains {r}"
                    ));
                }
            } else if status == Some("verified") && ao.get("unverified_reason").is_some() {
                problems.push(format!(
                    "evidence and handoff \"{id}\" verifiable assertion {at} is verified but carries an unverified_reason"
                ));
            }
        }
    }

    // 10. evidence — pinned, and (in §20) resolved through the external
    //     boundary; the structural covers link is checked here.
    let mut evidence_ids: HashSet<String> = HashSet::new();
    {
        let empty = Vec::new();
        let arr = payload
            .get("evidence")
            .and_then(Value::as_array)
            .unwrap_or(&empty);
        for (i, e) in arr.iter().enumerate() {
            let eo = if is_object(e) { e.clone() } else { Value::Null };
            let eid = eo.get("id").and_then(Value::as_str);
            let at = eid.map(str::to_string).unwrap_or_else(|| format!("#{i}"));
            if !eid.is_some_and(is_semantic_id) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" evidence entry {at} has no stable semantic id"
                ));
            } else {
                let eid = eid.unwrap();
                if !evidence_ids.insert(eid.to_string()) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" evidence id \"{eid}\" is used more than once"
                    ));
                }
            }
            for (f, human) in [("reference", "reference"), ("summary", "summary")] {
                if eo.get(f).map(blank).unwrap_or(true) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" evidence entry {at} has no {human}"
                    ));
                } else if let Some(r) = non_portable_reason(eo.get(f).and_then(Value::as_str)) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" evidence entry {at} {f} contains {r}"
                    ));
                }
            }
            if let Some(rev) = eo
                .get("revision")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
            {
                if let Some(rr) = non_portable_reason(Some(rev)) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" evidence entry {at} revision contains {rr}"
                    ));
                }
            }
            let pin_reason = pin_defect(
                eo.get("revision").and_then(Value::as_str),
                eo.get("sha256").and_then(Value::as_str),
            );
            if let Some(pin_reason) = pin_reason {
                problems.push(format!(
                    "evidence and handoff \"{id}\" evidence entry {at} is not pinned to an exact edition: {pin_reason}; a plain reference is not verifiable evidence"
                ));
            }
            let mut seen_cover = HashSet::new();
            if let Some(covers) = eo.get("covers").and_then(Value::as_array) {
                for (j, cv) in covers.iter().enumerate() {
                    let Some(cv) = cv.as_str().filter(|s| is_semantic_id(s)) else {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" evidence entry {at} covers[{j}] is not a stable semantic id"
                        ));
                        continue;
                    };
                    if !seen_cover.insert(cv.to_string()) {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" evidence entry {at} covers \"{cv}\" more than once"
                        ));
                    }
                    if !assertion_ids.contains(cv) {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" evidence entry {at} covers \"{cv}\", which is not a declared verifiable assertion; a narrow evidence entry is not widened to an undeclared assertion"
                        ));
                    }
                }
            }
            if let Some(limitations) = eo.get("limitations").and_then(Value::as_array) {
                for (j, lm) in limitations.iter().enumerate() {
                    if blank(lm) {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" evidence entry {at} limitations[{j}] is empty or whitespace-only"
                        ));
                    } else if let Some(r) = non_portable_reason(lm.as_str()) {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" evidence entry {at} limitations[{j}] contains {r}"
                        ));
                    }
                }
            }
            let kind = eo.get("kind").and_then(Value::as_str);
            if kind == Some("specialised-evidence-record") {
                for (f, human) in [
                    ("specialised_contract", "specialised_contract"),
                    ("recorded_verdict", "recorded_verdict"),
                ] {
                    if eo.get(f).map(blank).unwrap_or(true) {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" evidence entry {at} is a specialised-evidence-record but states no {human}; the specialised contract keeps authority over its own verdict, which this handoff records rather than re-derives"
                        ));
                    } else if let Some(r) = non_portable_reason(eo.get(f).and_then(Value::as_str)) {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" evidence entry {at} {f} contains {r}"
                        ));
                    }
                }
            } else {
                for f in ["specialised_contract", "recorded_verdict"] {
                    if eo.get(f).is_some() {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" evidence entry {at} carries \"{f}\" but its kind is \"{}\", not \"specialised-evidence-record\"",
                            kind.unwrap_or("undefined")
                        ));
                    }
                }
            }
        }
    }

    // 11. the established / not_established inference, checked on BOTH sides.
    {
        let empty = Vec::new();
        let arr = payload
            .get("claimed_results")
            .and_then(Value::as_array)
            .unwrap_or(&empty);
        for c in arr {
            let co = if is_object(c) { c.clone() } else { Value::Null };
            let cid = co.get("id").and_then(Value::as_str);
            let linked = cid
                .and_then(|cid| assertions_by_result.get(cid))
                .cloned()
                .unwrap_or_default();
            let unverified: Vec<&String> = linked
                .iter()
                .filter(|aid| {
                    assertion_by_id
                        .get(*aid)
                        .and_then(|ao| ao.get("status"))
                        .and_then(Value::as_str)
                        != Some("verified")
                })
                .collect();
            let all_verified = !linked.is_empty() && unverified.is_empty();
            let status = co.get("status").and_then(Value::as_str);
            let cid_str = cid.unwrap_or("");
            if status == Some("established") {
                if linked.is_empty() {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" claimed result \"{cid_str}\" is established but no verifiable assertion links to it; an established result carries at least one verified assertion"
                    ));
                } else if !unverified.is_empty() {
                    let names: Vec<&str> = unverified.iter().map(|s| s.as_str()).collect();
                    problems.push(format!(
                        "evidence and handoff \"{id}\" claimed result \"{cid_str}\" is established but assertion(s) {} linked to it are not verified; the unproven cannot be established",
                        names.join(", ")
                    ));
                }
            } else if status == Some("not_established") && all_verified {
                problems.push(format!(
                    "evidence and handoff \"{id}\" claimed result \"{cid_str}\" is \"not_established\" but it has at least one linked verifiable assertion and every one of them is verified; the status is derived from the evidence, not asserted — this result is established"
                ));
            }
        }
    }

    // 12. mandatory checks.
    struct CheckResultExpect {
        at: String,
        evidence_id: String,
        expect: &'static str,
        status: String,
        check_ref: Option<String>,
    }
    let mut executed_check_refs: HashSet<String> = HashSet::new();
    let mut not_executed_check_refs: HashSet<String> = HashSet::new();
    let mut check_result_expect: Vec<CheckResultExpect> = Vec::new();
    let mut has_failed_or_unable_check = false;
    {
        let mut seen_id = HashSet::new();
        let mut seen_ref = HashSet::new();
        let empty = Vec::new();
        let arr = payload
            .get("mandatory_checks")
            .and_then(Value::as_array)
            .unwrap_or(&empty);
        for (i, m) in arr.iter().enumerate() {
            let mo = if is_object(m) { m.clone() } else { Value::Null };
            let mid = mo.get("id").and_then(Value::as_str);
            let at = mid.map(str::to_string).unwrap_or_else(|| format!("#{i}"));
            if !mid.is_some_and(is_semantic_id) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" mandatory check {at} has no stable semantic id"
                ));
            } else {
                let mid = mid.unwrap();
                if !seen_id.insert(mid.to_string()) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" mandatory check id \"{mid}\" is used more than once"
                    ));
                }
            }
            if mo.get("name").map(blank).unwrap_or(true) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" mandatory check {at} has no name"
                ));
            } else if let Some(r) = non_portable_reason(mo.get("name").and_then(Value::as_str)) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" mandatory check {at} name contains {r}"
                ));
            }
            let mo_check_ref = mo.get("check_ref").and_then(Value::as_str).map(str::trim);
            if mo.get("check_ref").map(blank).unwrap_or(true) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" mandatory check {at} has no check_ref"
                ));
            } else {
                let raw = mo.get("check_ref").and_then(Value::as_str).unwrap_or("");
                if let Some(r) = non_portable_reason(Some(raw)) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" mandatory check {at} check_ref contains {r}"
                    ));
                }
                if let Some(ref_key) = mo_check_ref {
                    if !seen_ref.insert(ref_key.to_string()) {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" mandatory check check_ref \"{raw}\" is listed more than once"
                        ));
                    }
                }
            }
            let rev = mo.get("result_evidence_id").and_then(Value::as_str);
            let rev_valid = rev.is_some_and(is_semantic_id);
            let status = mo.get("status").and_then(Value::as_str);
            if status == Some("passed") || status == Some("failed") {
                let want = if status == Some("passed") {
                    "confirmed"
                } else {
                    "contradicted"
                };
                let status_str = status.unwrap();
                if !rev_valid {
                    let verb = if status_str == "passed" {
                        "successful"
                    } else {
                        "failing"
                    };
                    problems.push(format!(
                        "evidence and handoff \"{id}\" mandatory check {at} is \"{status_str}\" but names no result_evidence_id; a {status_str} status is backed by an evidence entry that shows the {verb} result, not by the completed_checks list"
                    ));
                } else {
                    let rev = rev.unwrap();
                    if !evidence_ids.contains(rev) {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" mandatory check {at} result_evidence_id \"{rev}\" is not a declared evidence entry"
                        ));
                    } else {
                        check_result_expect.push(CheckResultExpect {
                            at: at.clone(),
                            evidence_id: rev.to_string(),
                            expect: want,
                            status: status_str.to_string(),
                            check_ref: mo_check_ref.map(str::to_string),
                        });
                    }
                }
            }
            if status == Some("passed") {
                if mo.get("reason").is_some() {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" mandatory check {at} passed but carries a reason; a reason is stated for a failed or unable check"
                    ));
                }
                if let Some(r) = mo_check_ref {
                    executed_check_refs.insert(r.to_string());
                }
            } else if status == Some("failed") {
                has_failed_or_unable_check = true;
                if mo.get("reason").map(blank).unwrap_or(true) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" mandatory check {at} is \"failed\" but states no reason"
                    ));
                } else if let Some(r) =
                    non_portable_reason(mo.get("reason").and_then(Value::as_str))
                {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" mandatory check {at} reason contains {r}"
                    ));
                }
                if let Some(r) = mo_check_ref {
                    executed_check_refs.insert(r.to_string());
                }
            } else if status == Some("unable") {
                has_failed_or_unable_check = true;
                if mo.get("reason").map(blank).unwrap_or(true) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" mandatory check {at} is \"unable\" but states no verifiable reason; an unable check is not a pass and does not become one silently"
                    ));
                } else if let Some(r) =
                    non_portable_reason(mo.get("reason").and_then(Value::as_str))
                {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" mandatory check {at} reason contains {r}"
                    ));
                }
                if mo.get("result_evidence_id").is_some() {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" mandatory check {at} is \"unable\" but names a result_evidence_id; a check that could not run has no result to evidence"
                    ));
                }
                if let Some(r) = mo_check_ref {
                    not_executed_check_refs.insert(r.to_string());
                }
            }
        }
    }

    // 12b. acceptance-criteria coverage.
    let mut ac_coverage: HashMap<String, (Option<String>, Vec<String>)> = HashMap::new();
    {
        let mut seen_id = HashSet::new();
        let empty = Vec::new();
        let arr = payload
            .get("acceptance_criteria")
            .and_then(Value::as_array)
            .unwrap_or(&empty);
        for (i, c) in arr.iter().enumerate() {
            let co = if is_object(c) { c.clone() } else { Value::Null };
            let cid = co.get("id").and_then(Value::as_str);
            let at = cid.map(str::to_string).unwrap_or_else(|| format!("#{i}"));
            if !cid.is_some_and(is_semantic_id) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" acceptance criterion {at} has no stable semantic id"
                ));
            } else {
                let cid = cid.unwrap();
                if !seen_id.insert(cid.to_string()) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" acceptance criterion id \"{cid}\" is used more than once"
                    ));
                }
            }
            let mut addressed_by: Vec<String> = Vec::new();
            let mut seen_a = HashSet::new();
            if let Some(ab) = co.get("addressed_by").and_then(Value::as_array) {
                for (j, aid) in ab.iter().enumerate() {
                    let Some(aid) = aid.as_str().filter(|s| is_semantic_id(s)) else {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" acceptance criterion {at} addressed_by[{j}] is not a stable semantic id"
                        ));
                        continue;
                    };
                    if !seen_a.insert(aid.to_string()) {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" acceptance criterion {at} addresses \"{aid}\" more than once"
                        ));
                    }
                    if !assertion_ids.contains(aid) {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" acceptance criterion {at} is addressed_by \"{aid}\", which is not a declared verifiable assertion"
                        ));
                    }
                    addressed_by.push(aid.to_string());
                }
            }
            let status = co.get("status").and_then(Value::as_str);
            if status == Some("covered") {
                if addressed_by.is_empty() {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" acceptance criterion {at} is \"covered\" but names no addressed_by verifiable assertion"
                    ));
                }
                if co.get("uncovered_reason").is_some() {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" acceptance criterion {at} is \"covered\" but carries an uncovered_reason"
                    ));
                }
            } else if status == Some("uncovered") {
                if !addressed_by.is_empty() {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" acceptance criterion {at} is \"uncovered\" but names addressed_by assertions"
                    ));
                }
                if co.get("uncovered_reason").map(blank).unwrap_or(true) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" acceptance criterion {at} is \"uncovered\" but states no uncovered_reason"
                    ));
                } else if let Some(r) =
                    non_portable_reason(co.get("uncovered_reason").and_then(Value::as_str))
                {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" acceptance criterion {at} uncovered_reason contains {r}"
                    ));
                }
            }
            if let Some(cid) = cid {
                ac_coverage.insert(cid.to_string(), (status.map(str::to_string), addressed_by));
            }
        }
    }

    // 13. source and result states.
    let src_by_repo = check_repository_states(
        &mut problems,
        &id,
        "source_state",
        payload.get("source_state"),
    );
    let res_by_repo = check_repository_states(
        &mut problems,
        &id,
        "result_state",
        payload.get("result_state"),
    );
    if !src_by_repo.is_empty() && !res_by_repo.is_empty() {
        let mut extra_in_src: Vec<&str> = src_by_repo
            .keys()
            .filter(|k| !res_by_repo.contains_key(*k))
            .map(String::as_str)
            .collect();
        let mut extra_in_res: Vec<&str> = res_by_repo
            .keys()
            .filter(|k| !src_by_repo.contains_key(*k))
            .map(String::as_str)
            .collect();
        extra_in_src.sort_unstable();
        extra_in_res.sort_unstable();
        if !extra_in_src.is_empty() || !extra_in_res.is_empty() {
            let mut parts = String::new();
            if !extra_in_src.is_empty() {
                parts.push_str(&format!("source-only: {}", extra_in_src.join(", ")));
            }
            if !extra_in_src.is_empty() && !extra_in_res.is_empty() {
                parts.push_str("; ");
            }
            if !extra_in_res.is_empty() {
                parts.push_str(&format!("result-only: {}", extra_in_res.join(", ")));
            }
            problems.push(format!(
                "evidence and handoff \"{id}\" source_state and result_state pin different sets of repositories ({parts}); a run's before and after states cover the same repositories"
            ));
        }
    }

    // 14. changed paths.
    {
        let mut seen_id = HashSet::new();
        let mut seen_pair = HashSet::new();
        let empty = Vec::new();
        let arr = payload
            .get("changed_paths")
            .and_then(Value::as_array)
            .unwrap_or(&empty);
        for (i, c) in arr.iter().enumerate() {
            let co = if is_object(c) { c.clone() } else { Value::Null };
            let cid = co.get("id").and_then(Value::as_str);
            let at = cid.map(str::to_string).unwrap_or_else(|| format!("#{i}"));
            if !cid.is_some_and(is_semantic_id) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" changed path {at} has no stable semantic id"
                ));
            } else {
                let cid = cid.unwrap();
                if !seen_id.insert(cid.to_string()) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" changed path id \"{cid}\" is used more than once"
                    ));
                }
            }
            for (f, human) in [("repository_ref", "repository_ref"), ("path", "path")] {
                if co.get(f).map(blank).unwrap_or(true) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" changed path {at} has no {human}"
                    ));
                } else if let Some(r) = non_portable_reason(co.get(f).and_then(Value::as_str)) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" changed path {at} {f} contains {r}"
                    ));
                }
            }
            let repo = co
                .get("repository_ref")
                .and_then(Value::as_str)
                .map(str::trim);
            let p = co.get("path").and_then(Value::as_str).map(str::trim);
            if let (Some(repo), Some(p)) = (repo, p) {
                let pair = format!("{repo} {p}");
                if !seen_pair.insert(pair) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" changed path repeats \"{}\" in repository \"{repo}\"",
                        co.get("path").and_then(Value::as_str).unwrap_or("")
                    ));
                }
            }
            if let Some(repo) = repo {
                let in_src = src_by_repo.get(repo);
                let in_res = res_by_repo.get(repo);
                match (in_src, in_res) {
                    (Some(in_src), Some(in_res)) => {
                        if repo_pin_key(in_src) == repo_pin_key(in_res) {
                            problems.push(format!(
                                "evidence and handoff \"{id}\" changed path {at} names repository \"{}\", but its source_state and result_state pins are identical; a change produced a new revision",
                                co.get("repository_ref").and_then(Value::as_str).unwrap_or("")
                            ));
                        }
                    }
                    _ => {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" changed path {at} names repository \"{}\", which is not pinned in both source_state and result_state; a change has a before and an after revision",
                            co.get("repository_ref").and_then(Value::as_str).unwrap_or("")
                        ));
                    }
                }
            }
        }
    }

    // 15. external effects, deviations, open gaps, required owner decisions.
    {
        let mut seen_id = HashSet::new();
        let empty = Vec::new();
        let arr = payload
            .get("external_effects")
            .and_then(Value::as_array)
            .unwrap_or(&empty);
        for (i, e) in arr.iter().enumerate() {
            let eo = if is_object(e) { e.clone() } else { Value::Null };
            let eid = eo.get("id").and_then(Value::as_str);
            let at = eid.map(str::to_string).unwrap_or_else(|| format!("#{i}"));
            if !eid.is_some_and(is_semantic_id) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" external effect {at} has no stable semantic id"
                ));
            } else {
                let eid = eid.unwrap();
                if !seen_id.insert(eid.to_string()) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" external effect id \"{eid}\" is used more than once"
                    ));
                }
            }
            if eo.get("description").map(blank).unwrap_or(true) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" external effect {at} has no description"
                ));
            } else if let Some(r) =
                non_portable_reason(eo.get("description").and_then(Value::as_str))
            {
                problems.push(format!(
                    "evidence and handoff \"{id}\" external effect {at} description contains {r}"
                ));
            }
            if !matches!(eo.get("reversible"), Some(Value::Bool(_))) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" external effect {at} does not state whether it is reversible"
                ));
            }
        }
    }
    let mut has_blocking_deviation = false;
    {
        let mut seen_id = HashSet::new();
        let empty = Vec::new();
        let arr = payload
            .get("deviations")
            .and_then(Value::as_array)
            .unwrap_or(&empty);
        for (i, d) in arr.iter().enumerate() {
            let dobj = if is_object(d) { d.clone() } else { Value::Null };
            let did = dobj.get("id").and_then(Value::as_str);
            let at = did.map(str::to_string).unwrap_or_else(|| format!("#{i}"));
            if !did.is_some_and(is_semantic_id) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" deviation {at} has no stable semantic id"
                ));
            } else {
                let did = did.unwrap();
                if !seen_id.insert(did.to_string()) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" deviation id \"{did}\" is used more than once"
                    ));
                }
            }
            for (f, human) in [
                ("description", "description"),
                ("disposition", "disposition"),
            ] {
                if dobj.get(f).map(blank).unwrap_or(true) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" deviation {at} has no {human}"
                    ));
                } else if let Some(r) = non_portable_reason(dobj.get(f).and_then(Value::as_str)) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" deviation {at} {f} contains {r}"
                    ));
                }
            }
            if dobj.get("severity").and_then(Value::as_str) == Some("blocking") {
                has_blocking_deviation = true;
            }
        }
    }
    check_identified_items(
        &mut problems,
        &id,
        "open_gaps",
        payload.get("open_gaps"),
        &["description"],
    );
    let mut has_blocking_owner_decision = false;
    {
        let mut seen_id = HashSet::new();
        let empty = Vec::new();
        let arr = payload
            .get("required_owner_decisions")
            .and_then(Value::as_array)
            .unwrap_or(&empty);
        for (i, o) in arr.iter().enumerate() {
            let oo = if is_object(o) { o.clone() } else { Value::Null };
            let oid = oo.get("id").and_then(Value::as_str);
            let at = oid.map(str::to_string).unwrap_or_else(|| format!("#{i}"));
            if !oid.is_some_and(is_semantic_id) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" required owner decision {at} has no stable semantic id"
                ));
            } else {
                let oid = oid.unwrap();
                if !seen_id.insert(oid.to_string()) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" required owner decision id \"{oid}\" is used more than once"
                    ));
                }
            }
            if oo.get("question").map(blank).unwrap_or(true) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" required owner decision {at} has no question"
                ));
            } else if let Some(r) = non_portable_reason(oo.get("question").and_then(Value::as_str))
            {
                problems.push(format!(
                    "evidence and handoff \"{id}\" required owner decision {at} question contains {r}"
                ));
            }
            match oo.get("blocking") {
                Some(Value::Bool(true)) => has_blocking_owner_decision = true,
                Some(Value::Bool(false)) => {}
                _ => problems.push(format!(
                    "evidence and handoff \"{id}\" required owner decision {at} does not state whether it is blocking"
                )),
            }
        }
    }

    // 16. blockers.
    let mut blocker_ids: HashSet<String> = HashSet::new();
    {
        let empty = Vec::new();
        let arr = payload
            .get("blockers")
            .and_then(Value::as_array)
            .unwrap_or(&empty);
        for (i, b) in arr.iter().enumerate() {
            let bo = if is_object(b) { b.clone() } else { Value::Null };
            let bid = bo.get("id").and_then(Value::as_str);
            let at = bid.map(str::to_string).unwrap_or_else(|| format!("#{i}"));
            if !bid.is_some_and(is_semantic_id) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" blocker {at} has no stable id"
                ));
            } else {
                let bid = bid.unwrap();
                if !blocker_ids.insert(bid.to_string()) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" blocker id \"{bid}\" is used more than once"
                    ));
                }
            }
            if bo.get("description").map(blank).unwrap_or(true) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" blocker {at} has no description"
                ));
            }
            if bo.get("resumption_condition").map(blank).unwrap_or(true) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" blocker {at} has no verifiable resumption condition"
                ));
            }
            for f in ["description", "resumption_condition"] {
                if let Some(r) = non_portable_reason(bo.get(f).and_then(Value::as_str)) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" blocker {at} contains {r}"
                    ));
                }
            }
        }
    }

    // 17. the worktree disposition.
    {
        if let Some(wt) = payload.get("worktree_disposition").filter(|v| is_object(v)) {
            let state = wt.get("state").and_then(Value::as_str);
            if state == Some("retained") {
                for f in ["reason", "responsible", "cleanup_condition"] {
                    if wt.get(f).map(blank).unwrap_or(true) {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" worktree_disposition is \"retained\" but states no {f}; a retained worktree carries a reason, a responsible party and a verifiable cleanup condition"
                        ));
                    } else if let Some(r) = non_portable_reason(wt.get(f).and_then(Value::as_str)) {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" worktree_disposition {f} contains {r}"
                        ));
                    }
                }
            } else if state == Some("not_created") || state == Some("removed") {
                for f in ["reason", "responsible", "cleanup_condition"] {
                    if wt.get(f).is_some() {
                        let state_str = state.unwrap();
                        problems.push(format!(
                            "evidence and handoff \"{id}\" worktree_disposition is \"{state_str}\" but carries \"{f}\"; reason, responsible and cleanup_condition belong to the \"retained\" state only"
                        ));
                    }
                }
            }
        }
    }

    // 18. the next step, stated explicitly.
    let next_step = doc
        .get("payload")
        .and_then(|p| p.get("next_step"))
        .filter(|v| is_object(v))
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));
    for f in ["action", "gate", "actor_ref"] {
        if let Some(v) = next_step.get(f) {
            if !v.is_null() && blank(v) {
                problems.push(format!(
                    "evidence and handoff \"{id}\" next_step.{f} is present but empty; state a value, or null where there is none"
                ));
            }
        }
        if let Some(r) = non_portable_reason(next_step.get(f).and_then(Value::as_str)) {
            problems.push(format!(
                "evidence and handoff \"{id}\" next_step.{f} contains {r}"
            ));
        }
    }

    // 19. the overall outcome and its axis rules.
    let outcome = payload
        .get("outcome")
        .filter(|v| is_object(v))
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));
    if outcome.get("statement").map(blank).unwrap_or(true) {
        problems.push(format!(
            "evidence and handoff \"{id}\" outcome states no statement"
        ));
    } else if let Some(r) = non_portable_reason(outcome.get("statement").and_then(Value::as_str)) {
        problems.push(format!(
            "evidence and handoff \"{id}\" outcome statement contains {r}"
        ));
    }
    let results_arr = payload
        .get("claimed_results")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let every_result_established = !results_arr.is_empty()
        && results_arr.iter().all(|c| {
            is_object(c) && c.get("status").and_then(Value::as_str) == Some("established")
        });
    let every_check_passed = payload
        .get("mandatory_checks")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .all(|m| is_object(m) && m.get("status").and_then(Value::as_str) == Some("passed"))
        })
        .unwrap_or(true);
    let open_gap_count = payload
        .get("open_gaps")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let blocked_trigger =
        !blocker_ids.is_empty() || has_failed_or_unable_check || has_blocking_deviation;
    let outcome_status = outcome.get("status").and_then(Value::as_str);
    if outcome_status == Some("complete") {
        if !every_result_established {
            problems.push(format!(
                "evidence and handoff \"{id}\" outcome.status is \"complete\" but not every claimed result is established"
            ));
        }
        if !every_check_passed {
            problems.push(format!(
                "evidence and handoff \"{id}\" outcome.status is \"complete\" but a mandatory check is not \"passed\"; a failed or unable check is not a completion"
            ));
        }
        if !blocker_ids.is_empty() || has_blocking_deviation {
            problems.push(format!(
                "evidence and handoff \"{id}\" outcome.status is \"complete\" but a blocker or a blocking deviation remains"
            ));
        }
        if has_blocking_owner_decision {
            problems.push(format!(
                "evidence and handoff \"{id}\" outcome.status is \"complete\" but a required owner decision is blocking; the next step cannot proceed, so the run is handed off, not complete"
            ));
        }
        if open_gap_count > 0 {
            problems.push(format!(
                "evidence and handoff \"{id}\" outcome.status is \"complete\" but {open_gap_count} open gap(s) remain; a completed run carries no acknowledged incompleteness — record it as a deviation or leave the run handed_off_incomplete"
            ));
        }
    } else if outcome_status == Some("blocked") {
        if !blocked_trigger {
            problems.push(format!(
                "evidence and handoff \"{id}\" outcome.status is \"blocked\" but no blocker, failed or unable mandatory check, or blocking deviation is recorded"
            ));
        }
    } else if outcome_status == Some("handed_off_incomplete") && blocked_trigger {
        problems.push(format!(
            "evidence and handoff \"{id}\" outcome.status is \"handed_off_incomplete\" but a blocker, a failed or unable mandatory check, or a blocking deviation is recorded; that is a \"blocked\" outcome"
        ));
    }
    if !blocker_ids.is_empty()
        && outcome.get("status").is_some()
        && outcome_status != Some("blocked")
    {
        problems.push(format!(
            "evidence and handoff \"{id}\" carries {} blocker(s) but outcome.status is \"{}\"; a non-empty blocker set is a \"blocked\" outcome",
            blocker_ids.len(),
            outcome_status.unwrap_or("")
        ));
    }

    // 20. THE EXTERNAL RESOLUTION BOUNDARY.
    match resolve_records {
        None => problems.push(format!(
            "evidence and handoff \"{id}\" cannot be verified: no external record resolver was supplied; the pinned execution-run, task-specification, run-human-control and context-manifest records, and every evidence entry, are resolved OUTSIDE the handoff and checked against it — without that boundary the handoff is only an unanchored self-report"
        )),
        Some(resolve) => {
            let run_entry = payload
                .get("execution_run_ref")
                .and_then(|r| resolve_pinned_reference(&mut problems, &id, "execution_run_ref", r, resolve, "execution-run"));
            let spec_entry = payload
                .get("task_specification_ref")
                .and_then(|r| resolve_pinned_reference(&mut problems, &id, "task_specification_ref", r, resolve, "task-specification"));
            let hc_entry = payload
                .get("human_control_ref")
                .and_then(|r| resolve_pinned_reference(&mut problems, &id, "human_control_ref", r, resolve, "run-human-control"));
            let cm_entry = payload
                .get("context_manifest_ref")
                .and_then(|r| resolve_pinned_reference(&mut problems, &id, "context_manifest_ref", r, resolve, "context-manifest"));

            let run_state = run_entry.as_ref().and_then(|e| e.get("resolved_state")).filter(|v| is_object(v));
            let resolved_criteria: Option<HashSet<String>> = spec_entry
                .as_ref()
                .and_then(|e| e.get("acceptance_criteria"))
                .and_then(Value::as_array)
                .filter(|a| !a.is_empty())
                .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect());
            let resolved_mandatory_checks: Option<HashSet<String>> = spec_entry
                .as_ref()
                .and_then(|e| e.get("mandatory_checks"))
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_str).map(|s| s.trim().to_string()).collect());

            // 20a. resolve every evidence entry.
            let mut evidence_resolution: HashMap<String, EvidenceResolution> = HashMap::new();
            let mut supported_assertion_ids: HashSet<String> = HashSet::new();
            {
                let empty = Vec::new();
                let arr = payload.get("evidence").and_then(Value::as_array).unwrap_or(&empty);
                for (i, e) in arr.iter().enumerate() {
                    let eo = if is_object(e) { e.clone() } else { Value::Null };
                    let at = eo.get("id").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| format!("#{i}"));
                    let res = resolve_evidence_entry(&mut problems, &id, &at, &eo, &assertion_ids, resolve);
                    if res.clean && res.observed_result.as_deref() == Some("confirmed") {
                        if let Some(covers) = eo.get("covers").and_then(Value::as_array) {
                            for cv in covers {
                                if let Some(cv) = cv.as_str() {
                                    if assertion_ids.contains(cv) {
                                        supported_assertion_ids.insert(cv.to_string());
                                    }
                                }
                            }
                        }
                    }
                    if let Some(eid) = eo.get("id").and_then(Value::as_str) {
                        evidence_resolution.insert(eid.to_string(), res);
                    }
                }
            }

            // 20b. a verified assertion needs supporting resolved evidence.
            for (aid, ao) in &assertion_by_id {
                if ao.get("status").and_then(Value::as_str) == Some("verified") && !supported_assertion_ids.contains(aid) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" verifiable assertion \"{aid}\" is \"verified\" but no resolved, pin-checked evidence entry confirms it (a covering evidence entry that resolves and whose observed_result is \"confirmed\"); a plain reference is not proof"
                    ));
                }
            }

            // 20c. each passed/failed mandatory check's result evidence.
            for cre in &check_result_expect {
                let Some(res) = evidence_resolution.get(&cre.evidence_id) else {
                    continue;
                };
                if !res.clean {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" mandatory check {} is \"{}\" but its result evidence \"{}\" did not resolve cleanly through the external boundary",
                        cre.at, cre.status, cre.evidence_id
                    ));
                } else if res.observed_result.as_deref() != Some(cre.expect) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" mandatory check {} is \"{}\" but its result evidence \"{}\" resolves with observed_result {}, not \"{}\"",
                        cre.at,
                        cre.status,
                        cre.evidence_id,
                        serde_json::to_string(&res.observed_result).unwrap_or_else(|_| "null".to_string()),
                        cre.expect
                    ));
                }
                let confirmed_check_ref = res
                    .entry
                    .as_ref()
                    .and_then(|e| e.get("check_ref"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                if res.entry.is_some() {
                    match confirmed_check_ref {
                        None => problems.push(format!(
                            "evidence and handoff \"{id}\" mandatory check {} names result evidence \"{}\", but the resolved evidence-result records no check_ref; the transformer confirms which check a recorded result is the result of, and summary + covers from the handoff alone are not enough",
                            cre.at, cre.evidence_id
                        )),
                        Some(ccr) => {
                            if let Some(want) = &cre.check_ref {
                                if ccr != want {
                                    problems.push(format!(
                                        "evidence and handoff \"{id}\" mandatory check {} names result evidence \"{}\", but the resolved evidence-result records check_ref \"{ccr}\", not this check's check_ref \"{want}\"; a check's result is not another check's result",
                                        cre.at, cre.evidence_id
                                    ));
                                }
                            }
                        }
                    }
                }
            }

            // 20d. acceptance-criteria coverage closed against the resolved spec.
            if let Some(resolved_criteria) = &resolved_criteria {
                let handoff_criteria: HashSet<String> = ac_coverage.keys().cloned().collect();
                for c in &handoff_criteria {
                    if !resolved_criteria.contains(c) {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" acceptance criterion \"{c}\" is not among the resolved task-specification record's acceptance criteria"
                        ));
                    }
                }
                for c in resolved_criteria {
                    if !handoff_criteria.contains(c) {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" does not address acceptance criterion \"{c}\" declared by the resolved task-specification record; every criterion is covered or explicitly uncovered"
                        ));
                    }
                }
                let mc_len = payload.get("mandatory_checks").and_then(Value::as_array).map(Vec::len).unwrap_or(0);
                if !resolved_criteria.is_empty() && mc_len == 0 {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" lists no mandatory_checks, but the resolved task-specification record declares {} acceptance criteria; a self-selected empty check list does not discharge the specification's mandatory checks",
                        resolved_criteria.len()
                    ));
                }
            }

            // 20d'. the FULL set of mandatory_checks closed against the spec.
            if let Some(resolved_mandatory_checks) = &resolved_mandatory_checks {
                let mut handoff_checks: HashSet<String> = HashSet::new();
                if let Some(arr) = payload.get("mandatory_checks").and_then(Value::as_array) {
                    for m in arr {
                        if let Some(cr) = m.get("check_ref").and_then(Value::as_str).filter(|s| !s.trim().is_empty()) {
                            handoff_checks.insert(cr.trim().to_string());
                        }
                    }
                }
                for c in &handoff_checks {
                    if !resolved_mandatory_checks.contains(c) {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" mandatory check \"{c}\" is not among the resolved task-specification record's mandatory checks; the full set of mandatory checks is closed against the specification, and \"list is non-empty\" is not enough"
                        ));
                    }
                }
                for c in resolved_mandatory_checks {
                    if !handoff_checks.contains(c) {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" omits mandatory check \"{c}\" declared by the resolved task-specification record; removing a mandatory check from the handoff is rejected"
                        ));
                    }
                }
            }

            // 20e. outcome.status "complete" is the WHOLE run finished.
            if outcome_status == Some("complete") {
                if let Some(run_state) = run_state {
                    if run_state.get("work_status").and_then(Value::as_str) != Some("completed") {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" outcome.status is \"complete\" but the resolved execution-run record's work_status is {}, not \"completed\"; a complete outcome is the whole run finished",
                            json_stringify(run_state.get("work_status"))
                        ));
                    }
                }
                let action_present = next_step.get("action").is_some_and(|v| !v.is_null());
                let gate_present = next_step.get("gate").is_some_and(|v| !v.is_null());
                if action_present || gate_present {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" outcome.status is \"complete\" but next_step still carries an executable {}; a completed run has no next step",
                        if action_present { "action" } else { "gate" }
                    ));
                }
                for (cid, (status, addressed_by)) in &ac_coverage {
                    if status.as_deref() != Some("covered") {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" outcome.status is \"complete\" but acceptance criterion \"{cid}\" is not covered"
                        ));
                        continue;
                    }
                    let unver: Vec<&str> = addressed_by
                        .iter()
                        .filter(|aid| {
                            assertion_by_id
                                .get(*aid)
                                .and_then(|a| a.get("status"))
                                .and_then(Value::as_str)
                                != Some("verified")
                        })
                        .map(String::as_str)
                        .collect();
                    if !unver.is_empty() {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" outcome.status is \"complete\" but acceptance criterion \"{cid}\" is addressed only through unverified assertion(s) {}",
                            unver.join(", ")
                        ));
                    }
                }
            }

            if let (Some(run_state), Some(tsr)) = (run_state, payload.get("task_specification_ref").filter(|v| is_object(v))) {
                if let (Some(rst), Some(prr)) = (
                    run_state.get("task_specification_ref").and_then(Value::as_str),
                    tsr.get("reference").and_then(Value::as_str),
                ) {
                    if rst != prr {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" task_specification_ref names \"{prr}\", but the resolved execution-run record names task specification \"{rst}\"; the handoff carries the SAME specification the run resolved, not an independent one"
                        ));
                    }
                }
            }

            if let Some(run_state) = run_state {
                if run_state.get("next_action").is_some() && !same_step_value(next_step.get("action"), run_state.get("next_action")) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" next_step.action {} does not match the resolved execution-run record's next_action {}",
                        json_stringify(Some(next_step.get("action").cloned().unwrap_or(Value::Null)).as_ref()),
                        json_stringify(Some(run_state.get("next_action").cloned().unwrap_or(Value::Null)).as_ref())
                    ));
                }
                if run_state.get("next_gate").is_some() && !same_step_value(next_step.get("gate"), run_state.get("next_gate")) {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" next_step.gate {} does not match the resolved execution-run record's next_gate {}",
                        json_stringify(Some(next_step.get("gate").cloned().unwrap_or(Value::Null)).as_ref()),
                        json_stringify(Some(run_state.get("next_gate").cloned().unwrap_or(Value::Null)).as_ref())
                    ));
                }
                if run_state.get("blocker_ids").is_some_and(|v| v.is_array())
                    && !same_ref_set(&blocker_ids, run_state.get("blocker_ids"))
                {
                    problems.push(format!(
                        "evidence and handoff \"{id}\" blockers do not match the resolved execution-run record's blocker_ids; the two sets of blocker ids must be the same"
                    ));
                }
                if let Some(completed) = run_state.get("completed_checks").and_then(Value::as_array) {
                    let completed: HashSet<String> = completed
                        .iter()
                        .filter_map(Value::as_str)
                        .map(|s| s.trim().to_string())
                        .collect();
                    for r in &executed_check_refs {
                        if !completed.contains(r) {
                            problems.push(format!(
                                "evidence and handoff \"{id}\" mandatory check \"{r}\" ran (its status is \"passed\" or \"failed\") but its check_ref is not among the resolved execution-run record's completed_checks; a check that ran — passed OR failed — is a completed check of the run, and completed_checks confirms the fact it ran, not the verdict"
                            ));
                        }
                    }
                    for r in &not_executed_check_refs {
                        if completed.contains(r) {
                            problems.push(format!(
                                "evidence and handoff \"{id}\" mandatory check \"{r}\" is \"unable\" but appears among the resolved execution-run record's completed_checks; a check that could not run is not a completed check"
                            ));
                        }
                    }
                }
                if let Some(ws) = run_state.get("work_status").and_then(Value::as_str) {
                    if TERMINAL_STATUSES.contains(&ws) {
                        for f in ["action", "gate"] {
                            if next_step.get(f).is_some_and(|v| !v.is_null()) {
                                problems.push(format!(
                                    "evidence and handoff \"{id}\" the resolved execution-run record's work_status is \"{ws}\" but next_step.{f} carries an executable value; a completed or cancelled run does not carry a next executable step"
                                ));
                            }
                        }
                    }
                    if ws == "blocked" && outcome.get("status").is_some() && outcome_status != Some("blocked") {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" the resolved execution-run record's work_status is \"blocked\" but outcome.status is \"{}\"",
                            outcome_status.unwrap_or("")
                        ));
                    }
                }
            }

            if let (Some(hc_entry), Some(err)) = (&hc_entry, payload.get("execution_run_ref").filter(|v| is_object(v))) {
                if let (Some(lrr), Some(prr)) = (
                    hc_entry.get("linked_run_ref").and_then(Value::as_str),
                    err.get("reference").and_then(Value::as_str),
                ) {
                    if lrr != prr {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" human_control_ref resolves to a run-human-control record whose linked_run_ref is \"{lrr}\", not the handoff's pinned run \"{prr}\""
                        ));
                    }
                }
            }
            if let (Some(cm_entry), Some(err)) = (&cm_entry, payload.get("execution_run_ref").filter(|v| is_object(v))) {
                if let (Some(lrr), Some(prr)) = (
                    cm_entry.get("linked_run_ref").and_then(Value::as_str),
                    err.get("reference").and_then(Value::as_str),
                ) {
                    if lrr != prr {
                        problems.push(format!(
                            "evidence and handoff \"{id}\" context_manifest_ref resolves to a context-manifest record whose linked_run_ref is \"{lrr}\", not the handoff's pinned run \"{prr}\""
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
    fn pin_defect_matches_reference_pool() {
        assert!(pin_defect(Some("main"), None).is_some());
        assert!(pin_defect(Some(&"a".repeat(40)), None).is_none());
        assert!(pin_defect(None, Some(&"a".repeat(64))).is_none());
    }

    /// Audit: an unknown-field check over a resolved entry iterates
    /// `serde_json::Map` — a `BTreeMap` in this workspace (no
    /// `preserve_order` feature enabled anywhere) — never Node's
    /// `Object.keys()` source-text order. Two unknown fields named so their
    /// source-text order is the OPPOSITE of alphabetical order must both
    /// still be reported, deterministically, across repeated calls.
    #[test]
    fn resolve_pinned_reference_reports_every_unknown_field_deterministically_and_never_panics() {
        let resolver = |_r: &Value| {
            Some(serde_json::json!({
                "record_type": "execution-run",
                "id": "run-1",
                "reference": "runs/run-1",
                "revision": "a".repeat(40),
                "resolved_state": {},
                "zzzz_extra_field": "one",
                "aaaa_extra_field": "two",
            }))
        };
        let ref_val = serde_json::json!({
            "record_type": "execution-run", "id": "run-1", "reference": "runs/run-1",
        });
        for _ in 0..3 {
            let mut problems = Vec::new();
            resolve_pinned_reference(
                &mut problems,
                "h1",
                "execution_run_ref",
                &ref_val,
                &resolver,
                "execution-run",
            );
            assert!(
                problems.iter().any(|p| p.contains("zzzz_extra_field")),
                "{problems:?}"
            );
            assert!(
                problems.iter().any(|p| p.contains("aaaa_extra_field")),
                "{problems:?}"
            );
        }
    }

    #[test]
    fn resolve_pinned_reference_on_a_wrong_type_field_and_a_missing_field_fails_closed_without_panicking(
    ) {
        let resolver = |_r: &Value| {
            Some(serde_json::json!({
                "record_type": "execution-run",
                "id": "run-1",
                "reference": "runs/run-1",
                "revision": "a".repeat(40),
                "resolved_state": Value::Null,
                "source_bytes": 12345,
            }))
        };
        let ref_val = serde_json::json!({
            "record_type": "execution-run", "id": "run-1", "reference": "runs/run-1",
        });
        let mut problems = Vec::new();
        let entry = resolve_pinned_reference(
            &mut problems,
            "h1",
            "execution_run_ref",
            &ref_val,
            &resolver,
            "execution-run",
        );
        assert!(entry.is_some());
        assert!(
            problems.iter().any(|p| p.contains("no resolved_state")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.contains("source_bytes")),
            "{problems:?}"
        );
    }

    #[test]
    fn resolve_pinned_reference_on_a_resolver_that_finds_nothing_fails_closed_without_panicking() {
        let resolver = |_r: &Value| None;
        let ref_val = serde_json::json!({
            "record_type": "execution-run", "id": "run-1", "reference": "runs/run-1",
        });
        let mut problems = Vec::new();
        let entry = resolve_pinned_reference(
            &mut problems,
            "h1",
            "execution_run_ref",
            &ref_val,
            &resolver,
            "execution-run",
        );
        assert!(entry.is_none());
        assert!(!problems.is_empty());
    }

    #[test]
    fn evaluate_evidence_and_handoff_on_a_null_document_fails_closed_without_panicking() {
        let record_schema = serde_json::json!({"type": "object"});
        let envelope_schema = serde_json::json!({"type": "object"});
        let schemas = EvalSchemas {
            record_schema: &record_schema,
            envelope_schema: &envelope_schema,
        };
        let problems = evaluate_evidence_and_handoff(&Value::Null, &schemas, None);
        assert!(!problems.is_empty());
    }

    #[test]
    fn evaluate_evidence_and_handoff_with_no_resolver_fails_closed_without_panicking() {
        let record_schema = serde_json::json!({"type": "object"});
        let envelope_schema = serde_json::json!({"type": "object"});
        let schemas = EvalSchemas {
            record_schema: &record_schema,
            envelope_schema: &envelope_schema,
        };
        let doc = serde_json::json!({
            "record_type": "evidence-and-handoff",
            "id": "h1",
            "payload": {"execution_run_ref": {"record_type": "execution-run", "id": "run-1", "reference": "runs/run-1"}},
        });
        let problems = evaluate_evidence_and_handoff(&doc, &schemas, None);
        assert!(
            problems
                .iter()
                .any(|p| p.contains("no external record resolver")),
            "{problems:?}"
        );
    }
}
