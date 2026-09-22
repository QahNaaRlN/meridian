//! Business-contract migration of `scripts/lib/execution-state.mjs`: the
//! composite-consistency algorithm behind the `execution-state-model`
//! check. `registries/operating-model/execution-state.schema.json` is the
//! COMPLETE schema for the state of ONE execution run (`record_type:
//! execution-run`). Run-state DATA is Instance, like the task
//! specification and the instruction source registry — the Kernel ships
//! the schema, the product-neutral fixtures and this module.
//!
//! The managed entity is the RUN, not the conversational "task". One task
//! specification may drive several runs; each run carries its own stable
//! id and its own human-readable Russian title, and references EXACTLY ONE
//! task specification by a portable reference — it never absorbs or
//! duplicates the specification body inside its payload.
//!
//! Boundaries this module enforces that the JSON Schema subset cannot:
//!   - the record's `$schema` declaration is a portable relative reference
//!     that RESOLVES, within the Meridian namespace, to
//!     `execution-state.schema.json`. The resolution and the rooted-path
//!     rules are REUSED from [`super::task_specification`]
//!     (`resolve_schema_ref`, `non_portable_reason`): both record bases sit
//!     at namespace depth two, so a portable `"../../<dir>/<file>"`
//!     reference resolves to the same logical address, and this module
//!     adds no divergent copy of that address math or of the
//!     absolute-path rules;
//!   - the canonical scoped-record envelope is validated SEPARATELY;
//!   - a run record lives only in `scope.type` `run-state`; `scope.id`
//!     identifies the run and `scope.workspace_id` is mandatory;
//!   - the independent axes are all present and mutually consistent;
//!   - the transition history is ordered by an explicit, strictly
//!     increasing sequence, with no silent backward move or skipped stage;
//!   - run state does not leak a role, supervision mode, context manifest
//!     or evidence/handoff contract — those belong to later packages;
//!   - the reference strings are portable;
//!   - the human-readable name is stated in Russian for the reader.

use serde_json::Value;

use super::task_specification::{non_portable_reason, resolve_schema_ref};
use crate::source_format::json_schema;

pub const RECORD_TYPE: &str = "execution-run";

/// The closed, ordered lifecycle
/// (`meridian-operating-upgrade-plan.md` §7.1).
pub const LIFECYCLE_STAGES: [&str; 11] = [
    "intake",
    "classification",
    "norm_resolution",
    "planning",
    "execution",
    "verification",
    "acceptance",
    "integration",
    "deployment",
    "observation",
    "completion",
];

pub const WORK_STATUSES: [&str; 8] = [
    "planned",
    "ready",
    "active",
    "waiting_human",
    "blocked",
    "failed",
    "completed",
    "cancelled",
];

/// `completed` and `cancelled` are terminal: a transition that follows one
/// of them needs an explicitly allowed reopen basis.
pub const TERMINAL_STATUSES: [&str; 2] = ["completed", "cancelled"];

pub const CANONICAL_RECORD_BASE: &str = "records/execution-run";
pub const SCHEMA_NAMESPACE_DIR: &str = "registries/operating-model";
pub const EXPECTED_SCHEMA_BASENAME: &str = "execution-state.schema.json";
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
        "built-in-methodology" => "built-in-methodology is Kernel methodology, not a place for one run's episodic state",
        "user-profile" => "user-profile holds a user's rules and settings, not run state",
        "organization-profile" => "organization-profile holds an organisation's rules and settings, not run state",
        "project-workspace" => "project-workspace holds the project's goals and decisions; a run's episodic state is scoped to run-state",
        "repository-scope" => "repository-scope holds facts true for one repository; a run may span repositories and is scoped to run-state",
        _ => "it is not run-state",
    }
}

/// The independent axes of the minimal current state. One universal
/// "status" field standing in for several of them is forbidden.
pub const REQUIRED_AXES: [&str; 10] = [
    "lifecycle_stage",
    "work_status",
    "scope_revision",
    "current_actor",
    "resolved_norms",
    "completed_checks",
    "blockers",
    "next_action",
    "next_gate",
    "transition_history",
];

/// Fields that belong to LATER packages, or that would collapse the
/// independent axes into one.
pub const FORBIDDEN_PAYLOAD_FIELDS: [&str; 24] = [
    "status",
    "role",
    "roles",
    "assigned_role",
    "reviewer_independence",
    "human_authority",
    "supervision_mode",
    "hic",
    "hitl",
    "hotl",
    "communication_mode",
    "owner_relayed",
    "context_manifest",
    "bounded_context",
    "evidence",
    "evidence_contract",
    "handoff",
    "worktree_state",
    "field_metrics",
    "evaluation_metrics",
    "goal",
    "initial_state",
    "target_model",
    "task_pattern",
];
// Node's array also carries `constraints`, `acceptance_criteria` and
// `task_specification` after `task_pattern`, plus `command_log`,
// `transcript`, `messages`, `chat_history` — kept as a second array below
// so the combined check list matches exactly without a fixed-size literal
// growing unreadable.
pub const FORBIDDEN_PAYLOAD_FIELDS_2: [&str; 7] = [
    "constraints",
    "acceptance_criteria",
    "task_specification",
    "command_log",
    "transcript",
    "messages",
    "chat_history",
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
fn is_semantic_id(s: &str) -> bool {
    SEMANTIC_ID_RE.is_match(s).unwrap_or(false)
}
fn is_object(v: &Value) -> bool {
    v.is_object()
}
fn blank(v: &Value) -> bool {
    match v.as_str() {
        Some(s) => s.trim().is_empty(),
        None => v.is_null() || !v.is_string(),
    }
}
fn blank_opt(v: Option<&Value>) -> bool {
    match v {
        None => true,
        Some(v) => blank(v),
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

pub fn stage_index(stage: &str) -> Option<usize> {
    LIFECYCLE_STAGES.iter().position(|s| *s == stage)
}

pub struct EvalSchemas<'a> {
    pub record_schema: &'a Value,
    pub envelope_schema: &'a Value,
}

/// The whole composition pipeline for one execution-run record. Port of
/// `evaluateExecutionState`.
pub fn evaluate_execution_state(doc: &Value, schemas: &EvalSchemas) -> Vec<String> {
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
                "execution-state schema could not be applied: {error}"
            )]
        }
    }

    if !is_object(doc) {
        if problems.is_empty() {
            return vec!["the execution run record is not an object".to_string()];
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
            "execution run \"{id}\" declares no $schema; a record names {EXPECTED_SCHEMA_BASENAME} so a consumer validates the whole contract — id, scope, origin, authority and body — not only the envelope"
        )),
        Some(declared) => {
            if let Some(portability) = non_portable_reason(Some(declared)) {
                problems.push(format!(
                    "execution run \"{id}\" $schema \"{declared}\" is not portable ({portability}); the schema is named by a relative reference resolved inside the Meridian namespace (canonical logical base {CANONICAL_RECORD_BASE})"
                ));
            } else {
                let resolved = resolve_schema_ref(Some(declared));
                if resolved.as_deref() == Some(envelope_schema_ref().as_str()) {
                    problems.push(format!(
                        "execution run \"{id}\" $schema \"{declared}\" resolves to the record envelope ({ENVELOPE_SCHEMA_BASENAME}); it must name {EXPECTED_SCHEMA_BASENAME}, which composes the envelope with the body"
                    ));
                } else if resolved.as_deref() != Some(expected_schema_ref().as_str()) {
                    let expected = expected_schema_ref();
                    problems.push(format!(
                        "execution run \"{id}\" $schema \"{declared}\" does not resolve to the logical address {expected} within the Meridian namespace; the specialised schema is named by a portable relative reference (a bare basename, a missing namespace segment, a wrong segment and a reference climbing out of the namespace all resolve elsewhere)"
                    ));
                }
            }
        }
    }

    let record_type = doc.get("record_type").and_then(Value::as_str).unwrap_or("");
    if record_type != RECORD_TYPE {
        problems.push(format!(
            "execution run \"{id}\" declares record_type \"{record_type}\", not \"{RECORD_TYPE}\"; the record type names the run and does not open a second envelope"
        ));
    }
    if !doc
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(is_semantic_id)
    {
        problems.push(format!(
            "execution run \"{id}\" has no stable semantic id on the record envelope"
        ));
    }
    let title = doc.get("title").and_then(Value::as_str);
    match title {
        None => problems.push(format!("execution run \"{id}\" has no human-readable title")),
        Some(t) if t.trim().is_empty() => {
            problems.push(format!("execution run \"{id}\" has no human-readable title"))
        }
        Some(t) if !has_cyrillic(t) => problems.push(format!(
            "execution run \"{id}\" title \"{t}\" carries no Russian (Cyrillic) text; the run name is stated in Russian for the human reader"
        )),
        _ => {}
    }
    if let Some(r) = non_portable_reason(title) {
        problems.push(format!("execution run \"{id}\" title contains {r}"));
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
                "execution run \"{id}\" is scoped to \"{scope_type_str}\"; a run record lives in run-state only — {why}"
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
                "execution run \"{id}\" run-state scope carries no stable scope.id identifying the run"
            ));
        }
        if !scope
            .get("workspace_id")
            .and_then(Value::as_str)
            .is_some_and(is_semantic_id)
        {
            problems.push(format!(
                "execution run \"{id}\" run-state scope carries no workspace_id; a run belongs to a project workspace (workspace-scope-model.md §1)"
            ));
        }
    }

    let origin = doc
        .get("origin")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);
    if origin.get("kind").and_then(Value::as_str) == Some("built-in") {
        problems.push(format!(
            "execution run \"{id}\" declares origin.kind \"built-in\"; a run is started in a workspace, not shipped with the methodology"
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
                    "execution run \"{id}\" {label} contains {r}; a run record is portable and carries no rooted machine path"
                ));
            }
        }
    }

    let payload = doc
        .get("payload")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);

    for axis in REQUIRED_AXES {
        if payload.get(axis).is_none() {
            problems.push(format!(
                "execution run \"{id}\" states no {axis}; it is a required independent axis with no default — a missing axis is not an empty list, null or a default value"
            ));
        }
    }

    for f in all_forbidden_fields() {
        if payload.get(f).is_some() {
            let why = if f == "status" {
                "the model uses independent axes (lifecycle_stage, work_status, scope_revision, …), not one universal \"status\""
            } else {
                "a role, supervision mode, communication mode, context manifest, full evidence/handoff contract or the statement of the work belongs to a later package, not to run state"
            };
            problems.push(format!(
                "execution run \"{id}\" payload carries \"{f}\"; {why}"
            ));
        }
    }
    if payload.get("record_type").is_some() {
        problems.push(format!(
            "execution run \"{id}\" repeats record_type inside the payload; the record type is declared once, on the envelope"
        ));
    }

    let tsr = payload.get("task_specification_ref");
    match tsr {
        None => problems.push(format!(
            "execution run \"{id}\" names no task_specification_ref; a run references exactly one task specification"
        )),
        Some(v) if is_object(v) || v.is_array() => problems.push(format!(
            "execution run \"{id}\" task_specification_ref is not a plain reference; the run references the specification, it does not embed or duplicate it"
        )),
        Some(v) if blank(v) => problems.push(format!(
            "execution run \"{id}\" task_specification_ref is empty or whitespace-only"
        )),
        Some(v) => {
            if let Some(r) = non_portable_reason(v.as_str()) {
                problems.push(format!(
                    "execution run \"{id}\" task_specification_ref contains {r}; the reference is portable and is not an absolute machine path"
                ));
            }
        }
    }

    let work_status = payload
        .get("work_status")
        .and_then(Value::as_str)
        .unwrap_or("");
    let lifecycle_stage = payload
        .get("lifecycle_stage")
        .and_then(Value::as_str)
        .unwrap_or("");

    let blockers_opt = payload.get("blockers").and_then(Value::as_array);
    if let Some(blockers) = blockers_opt {
        let mut seen_blocker_id = std::collections::HashSet::new();
        for (i, b) in blockers.iter().enumerate() {
            let bo = if is_object(b) { b.clone() } else { Value::Null };
            let bid = bo.get("id").and_then(Value::as_str);
            let at = bid.map(str::to_string).unwrap_or_else(|| format!("#{i}"));
            match bid {
                Some(bid) if is_semantic_id(bid) => {
                    if !seen_blocker_id.insert(bid.to_string()) {
                        problems.push(format!(
                            "execution run \"{id}\" blocker id \"{bid}\" is used more than once"
                        ));
                    }
                }
                _ => problems.push(format!(
                    "execution run \"{id}\" blocker {at} has no stable id"
                )),
            }
            if blank_opt(bo.get("description")) {
                problems.push(format!(
                    "execution run \"{id}\" blocker {at} has no description"
                ));
            }
            if blank_opt(bo.get("resumption_condition")) {
                problems.push(format!(
                    "execution run \"{id}\" blocker {at} has no verifiable resumption condition"
                ));
            }
            for s in [
                bo.get("description").and_then(Value::as_str),
                bo.get("resumption_condition").and_then(Value::as_str),
            ] {
                if let Some(r) = non_portable_reason(s) {
                    problems.push(format!("execution run \"{id}\" blocker {at} contains {r}"));
                }
            }
        }
        if !blockers.is_empty() && work_status != "blocked" {
            problems.push(format!(
                "execution run \"{id}\" carries {} blocker(s) but work_status is \"{work_status}\"; a non-empty blocker agrees with work_status \"blocked\" (waiting_human and blocked are not interchangeable)",
                blockers.len()
            ));
        }
        if blockers.is_empty() && work_status == "blocked" {
            problems.push(format!(
                "execution run \"{id}\" work_status is \"blocked\" with no blocker; \"blocked\" means continuation is impossible until a stated external condition changes"
            ));
        }
    }

    for f in ["next_action", "next_gate"] {
        if let Some(v) = payload.get(f) {
            if !v.is_null() && blank(v) {
                problems.push(format!(
                    "execution run \"{id}\" {f} is present but empty; state an action, or null where there is no continuation"
                ));
            }
        }
    }
    if TERMINAL_STATUSES.contains(&work_status) {
        for f in ["next_action", "next_gate"] {
            if let Some(v) = payload.get(f) {
                if !v.is_null() {
                    problems.push(format!(
                        "execution run \"{id}\" work_status is \"{work_status}\" but {f} carries an executable value \"{}\"; a completed or cancelled run does not silently carry a next executable step",
                        display_value(v)
                    ));
                }
            }
        }
    }
    if work_status == "waiting_human" && blank_opt(payload.get("next_action")) {
        problems.push(format!(
            "execution run \"{id}\" work_status is \"waiting_human\" but next_action names no concrete human action; \"waiting_human\" means a specific required human action is known"
        ));
    }

    for f in ["resolved_norms", "completed_checks"] {
        let Some(arr) = payload.get(f).and_then(Value::as_array) else {
            continue;
        };
        let mut seen = std::collections::HashSet::new();
        for (i, r) in arr.iter().enumerate() {
            if blank(r) {
                problems.push(format!(
                    "execution run \"{id}\" {f}[{i}] is empty or whitespace-only"
                ));
                continue;
            }
            let r_str = r.as_str().unwrap_or("");
            if let Some(reason) = non_portable_reason(Some(r_str)) {
                problems.push(format!("execution run \"{id}\" {f}[{i}] contains {reason}"));
            }
            let key = r_str.trim().to_string();
            if !seen.insert(key) {
                problems.push(format!(
                    "execution run \"{id}\" {f} repeats reference \"{r_str}\""
                ));
            }
        }
    }

    if let Some(actor) = payload
        .get("current_actor")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        if let Some(r) = non_portable_reason(Some(actor)) {
            problems.push(format!(
                "execution run \"{id}\" current_actor contains {r}; the actor is a portable opaque reference and grants no role or authority"
            ));
        }
    }

    if let Some(rev) = payload.get("scope_revision") {
        if !is_positive_int(rev) {
            problems.push(format!(
                "execution run \"{id}\" scope_revision \"{}\" is not a positive integer revision of the run's resolved scope",
                display_value(rev)
            ));
        }
    }

    let history_val = payload.get("transition_history");
    let history = history_val.and_then(Value::as_array);
    match (history_val, history) {
        (Some(_), None) => problems.push(format!(
            "execution run \"{id}\" transition_history is not an ordered list"
        )),
        (_, Some(history)) if history.is_empty() => problems.push(format!(
            "execution run \"{id}\" transition_history is empty; the history is mandatory and its first record states the initial state"
        )),
        (_, Some(history)) => {
            let mut prev_seq: Option<i64> = None;
            let mut prev_revision: Option<i64> = None;
            for (i, tr) in history.iter().enumerate() {
                let t = if is_object(tr) { tr.clone() } else { Value::Null };
                let at = format!("transition #{i}");

                for (val, label) in [
                    (t.get("reason"), "reason"),
                    (t.get("backward_rationale"), "backward_rationale"),
                    (t.get("reopen_rationale"), "reopen_rationale"),
                ] {
                    let Some(val) = val else { continue };
                    if label == "reason" && blank(val) {
                        problems.push(format!(
                            "execution run \"{id}\" {at} states no reason; every transition states why it happened"
                        ));
                    }
                    if let Some(r) = non_portable_reason(val.as_str()) {
                        problems.push(format!("execution run \"{id}\" {at} {label} contains {r}"));
                    }
                }
                if t.get("reason").is_none() {
                    problems.push(format!(
                        "execution run \"{id}\" {at} states no reason; every transition states why it happened"
                    ));
                }

                let seq = t.get("sequence");
                if !seq.is_some_and(is_positive_int) {
                    problems.push(format!(
                        "execution run \"{id}\" {at} has no positive integer sequence ordinal; the history is ordered by an explicit sequence, not by object or file order"
                    ));
                } else if let Some(prev) = prev_seq {
                    let s = seq.unwrap().as_i64().unwrap();
                    if s == prev {
                        problems.push(format!(
                            "execution run \"{id}\" {at} repeats sequence ordinal {s}; ordinals do not repeat"
                        ));
                    } else if s < prev {
                        problems.push(format!(
                            "execution run \"{id}\" {at} sequence ordinal {s} is below the previous {prev}; ordinals do not decrease"
                        ));
                    }
                }
                if seq.is_some_and(is_positive_int) {
                    prev_seq = seq.and_then(Value::as_i64);
                }

                let rev = t.get("scope_revision");
                if !rev.is_some_and(is_positive_int) {
                    problems.push(format!(
                        "execution run \"{id}\" {at} has no positive integer scope_revision"
                    ));
                } else {
                    let r = rev.unwrap().as_i64().unwrap();
                    if let Some(prev) = prev_revision {
                        if r < prev {
                            problems.push(format!(
                                "execution run \"{id}\" {at} scope_revision {r} is below the previous {prev}; the scope revision does not decrease"
                            ));
                        }
                    }
                    prev_revision = Some(r);
                }

                if i == 0 {
                    if !t.get("from_stage").is_none_or(Value::is_null)
                        || !t.get("from_status").is_none_or(Value::is_null)
                    {
                        problems.push(format!(
                            "execution run \"{id}\" {at} is the first record but names a prior from_stage/from_status; the first record states the initial state (from_stage and from_status are null)"
                        ));
                    }
                } else {
                    let prev = history[i - 1].clone();
                    let prev = if is_object(&prev) { prev } else { Value::Null };
                    if t.get("from_stage") != prev.get("to_stage") {
                        problems.push(format!(
                            "execution run \"{id}\" {at} from_stage \"{}\" does not continue the previous to_stage \"{}\"; the history has no break",
                            display_opt(t.get("from_stage")),
                            display_opt(prev.get("to_stage"))
                        ));
                    }
                    if t.get("from_status") != prev.get("to_status") {
                        problems.push(format!(
                            "execution run \"{id}\" {at} from_status \"{}\" does not continue the previous to_status \"{}\"; the history has no break",
                            display_opt(t.get("from_status")),
                            display_opt(prev.get("to_status"))
                        ));
                    }
                }

                let from_idx = t.get("from_stage").and_then(Value::as_str).and_then(stage_index);
                let to_idx = t.get("to_stage").and_then(Value::as_str).and_then(stage_index);
                if i > 0 {
                    if let (Some(from_idx), Some(to_idx)) = (from_idx, to_idx) {
                        if to_idx < from_idx && blank_opt(t.get("backward_rationale")) {
                            problems.push(format!(
                                "execution run \"{id}\" {at} moves back from \"{}\" to \"{}\" with no backward_rationale; a backward lifecycle move is not accepted silently",
                                LIFECYCLE_STAGES[from_idx], LIFECYCLE_STAGES[to_idx]
                            ));
                        }
                        if to_idx > from_idx + 1 {
                            let covered: std::collections::HashSet<&str> = t
                                .get("skipped_stages")
                                .and_then(Value::as_array)
                                .into_iter()
                                .flatten()
                                .filter(|s| is_object(s) && !blank_opt(s.get("rationale")))
                                .filter_map(|s| s.get("stage").and_then(Value::as_str))
                                .collect();
                            for skipped in &LIFECYCLE_STAGES[(from_idx + 1)..to_idx] {
                                let skipped = *skipped;
                                if !covered.contains(skipped) {
                                    problems.push(format!(
                                        "execution run \"{id}\" {at} jumps over \"{skipped}\" with no explicit inapplicability basis; a skipped stage is stated, not silently treated as passed"
                                    ));
                                }
                            }
                        }
                    }
                }
                for skip in t
                    .get("skipped_stages")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if !is_object(skip) {
                        continue;
                    }
                    let stage = skip.get("stage").and_then(Value::as_str).unwrap_or("");
                    if stage_index(stage).is_none() {
                        problems.push(format!(
                            "execution run \"{id}\" {at} skipped_stages names unknown stage \"{stage}\""
                        ));
                    }
                    if blank_opt(skip.get("rationale")) {
                        problems.push(format!(
                            "execution run \"{id}\" {at} skipped_stages entry for \"{stage}\" has no rationale"
                        ));
                    }
                    if let Some(r) = non_portable_reason(skip.get("rationale").and_then(Value::as_str)) {
                        problems.push(format!(
                            "execution run \"{id}\" {at} skipped_stages rationale contains {r}"
                        ));
                    }
                }

                let prev_terminal = i > 0 && {
                    let prev = history[i - 1].clone();
                    let prev = if is_object(&prev) { prev } else { Value::Null };
                    prev.get("to_status")
                        .and_then(Value::as_str)
                        .is_some_and(|s| TERMINAL_STATUSES.contains(&s))
                };
                if prev_terminal && blank_opt(t.get("reopen_rationale")) {
                    problems.push(format!(
                        "execution run \"{id}\" {at} follows a completed or cancelled state with no reopen_rationale; a new transition after a terminal state is rejected without an explicitly allowed reopen basis"
                    ));
                }
            }

            let last = history.last().cloned().unwrap_or(Value::Null);
            let last = if is_object(&last) { last } else { Value::Null };
            let last_to_stage = last.get("to_stage").and_then(Value::as_str).unwrap_or("");
            if last_to_stage != lifecycle_stage {
                problems.push(format!(
                    "execution run \"{id}\" last transition to_stage \"{last_to_stage}\" does not match the current lifecycle_stage \"{lifecycle_stage}\""
                ));
            }
            let last_to_status = last.get("to_status").and_then(Value::as_str).unwrap_or("");
            if last_to_status != work_status {
                problems.push(format!(
                    "execution run \"{id}\" last transition to_status \"{last_to_status}\" does not match the current work_status \"{work_status}\""
                ));
            }
            if let (Some(last_rev), Some(cur_rev)) = (last.get("scope_revision"), payload.get("scope_revision")) {
                if is_positive_int(last_rev) && is_positive_int(cur_rev) && last_rev != cur_rev {
                    problems.push(format!(
                        "execution run \"{id}\" last transition scope_revision {} does not match the current scope_revision {}",
                        display_value(last_rev), display_value(cur_rev)
                    ));
                }
            }
        }
        (None, None) => {}
    }

    problems
}

fn display_value(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn display_opt(v: Option<&Value>) -> String {
    match v {
        None => "null".to_string(),
        Some(v) => display_value(v),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_index_finds_known_stages_in_order() {
        assert_eq!(stage_index("intake"), Some(0));
        assert_eq!(stage_index("completion"), Some(10));
        assert_eq!(stage_index("bogus"), None);
    }

    #[test]
    fn the_real_kernel_schema_and_fixtures_agree() {
        let kernel_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let record_schema: Value = serde_json::from_str(
            &std::fs::read_to_string(
                kernel_root.join("registries/operating-model/execution-state.schema.json"),
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
                    .join("registries/operating-model/fixtures/execution-state.fixtures.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let schemas = EvalSchemas {
            record_schema: &record_schema,
            envelope_schema: &envelope_schema,
        };
        for case in fixtures["valid"].as_array().unwrap() {
            let problems = evaluate_execution_state(&case["spec"], &schemas);
            assert!(
                problems.is_empty(),
                "note={:?} problems={:?}",
                case.get("note"),
                problems
            );
        }
        for case in fixtures["invalid"].as_array().unwrap() {
            let problems = evaluate_execution_state(&case["spec"], &schemas);
            assert!(!problems.is_empty(), "note={:?}", case.get("note"));
        }
    }
}
