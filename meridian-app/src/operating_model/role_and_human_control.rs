//! Business-contract migration of `scripts/lib/role-and-human-control.mjs`: the
//! composite-consistency algorithms behind the `role-and-human-control`
//! check. Two product-neutral Kernel contracts, one module:
//!
//!   1. `registries/operating-model/role-registry.schema.json` — the
//!      COMPLETE schema for the built-in, product-neutral catalogue of
//!      universal roles (`record_type: role-registry`). One scoped record;
//!      `standards/workspace/role-registry.yaml` is the catalogue.
//!   2. `registries/operating-model/human-control.schema.json` — the
//!      COMPLETE schema for the human-control state of ONE execution run
//!      (`record_type: run-human-control`): role assignments, the acting
//!      participant, the permanent human-in-command posture and its
//!      holder, the switchable supervision mode, the communication mode,
//!      an optional independent-review requirement and the ordered
//!      history of changes to every mutable control axis. Concrete
//!      control records are Instance data; the Kernel ships the schema,
//!      the product-neutral fixtures and this module.
//!
//! Boundaries this module enforces that the JSON Schema subset cannot:
//!   - the record's `$schema` declaration RESOLVES, within the Meridian
//!     namespace, to the right specialised schema — reusing
//!     [`super::task_specification`]'s `resolve_schema_ref`/
//!     `non_portable_reason`, exactly as `execution_state` does;
//!   - the built-in role catalogue lives only in `built-in-methodology`,
//!     carries every one of the seven universal roles exactly once, and
//!     names no programme, AI model, vendor or person;
//!   - a concrete control record lives only in `run-state`, carries
//!     `workspace_id` and a portable reference to EXACTLY ONE execution
//!     run, and rejects `origin.kind` `built-in`;
//!   - human-in-command is the permanent basis: not a supervision mode,
//!     cannot be disabled, its holder is a participant assigned the owner
//!     role; human-in-the-loop needs an `hitl` block and FORBIDS an
//!     `hotl` block; human-on-the-loop needs an `hotl` block and FORBIDS
//!     an `hitl` block;
//!   - role, supervision mode, human authority, the acting actor and the
//!     communication mode are independent axes;
//!   - the acting actor, and every actor a switch record moves to, is
//!     assigned in the matching role-assignment set;
//!   - the switch history is ordered by a strictly increasing sequence,
//!     with no break on any mutable axis, and the last record matches the
//!     whole current control state;
//!   - every reference and prose string is portable; the human-readable
//!     name is stated in Russian.
//!
//! `sameAssignmentSet`'s Node reference compares two role-assignment sets
//! by `JSON.stringify` equality — sensitive to source key order, not only
//! content. This port compares the already-parsed [`Value`]s directly
//! (`==`): `serde_json::Value::Object` is a `BTreeMap` in this workspace
//! (no `preserve_order` feature enabled anywhere), so both operands are
//! already key-order-canonical before this function ever sees them — the
//! same boundary already documented on
//! `meridian-cli/examples/resolve_cli_producer.rs`'s own `result` field,
//! not a new one introduced here.

use serde_json::Value;

use super::task_specification::{non_portable_reason, resolve_schema_ref};
use crate::source_format::json_schema;

pub const ROLE_REGISTRY_RECORD_TYPE: &str = "role-registry";
pub const HUMAN_CONTROL_RECORD_TYPE: &str = "run-human-control";

/// The closed pool of universal roles. Named from the operating glossary
/// (`role`): a set of powers and responsibilities, never a participant, a
/// programme, an AI model or a vendor. One participant may hold several.
pub const ROLES: [&str; 7] = [
    "owner",
    "operator",
    "executor",
    "reviewer",
    "verifier",
    "git_integrator",
    "deployer",
];

/// The closed, switchable supervision-mode pool. human-in-command is NOT a
/// member: it is the permanent basis on its own axis (`human_authority`).
pub const SUPERVISION_MODES: [&str; 2] = ["human-in-the-loop", "human-on-the-loop"];

/// The permanent human command posture. It is a constant, not a choice.
pub const HUMAN_AUTHORITY_POSTURE: &str = "human-in-command";

pub const COMMUNICATION_MODES: [&str; 2] = ["owner_relayed", "direct"];

/// The role that holds human-in-command.
pub const HUMAN_AUTHORITY_ROLE: &str = "owner";

/// Roles that make an actor a suitable INDEPENDENT reviewer of a result.
pub const INDEPENDENT_REVIEW_ROLES: [&str; 2] = ["reviewer", "verifier"];

pub const CONTROL_AXES: [&str; 4] = [
    "supervision_mode",
    "communication_mode",
    "acting_actor",
    "role_assignments",
];

pub const CANONICAL_REGISTRY_BASE: &str = "records/role-registry";
pub const CANONICAL_CONTROL_BASE: &str = "records/run-human-control";
pub const SCHEMA_NAMESPACE_DIR: &str = "registries/operating-model";
pub const REGISTRY_SCHEMA_BASENAME: &str = "role-registry.schema.json";
pub const CONTROL_SCHEMA_BASENAME: &str = "human-control.schema.json";
pub const ENVELOPE_SCHEMA_BASENAME: &str = "scoped-record.schema.json";

fn registry_schema_ref() -> String {
    format!("{SCHEMA_NAMESPACE_DIR}/{REGISTRY_SCHEMA_BASENAME}")
}
fn control_schema_ref() -> String {
    format!("{SCHEMA_NAMESPACE_DIR}/{CONTROL_SCHEMA_BASENAME}")
}
fn envelope_schema_ref() -> String {
    format!("{SCHEMA_NAMESPACE_DIR}/{ENVELOPE_SCHEMA_BASENAME}")
}

pub const REGISTRY_SCOPE_TYPE: &str = "built-in-methodology";
pub const CONTROL_SCOPE_TYPE: &str = "run-state";

fn control_scope_rejection_reason(scope_type: &str) -> &'static str {
    match scope_type {
        "built-in-methodology" => "built-in-methodology is Kernel methodology, not a place for one run's control state",
        "user-profile" => "user-profile holds a user's rules and settings, not run control state",
        "organization-profile" => "organization-profile holds an organisation's rules and settings, not run control state",
        "project-workspace" => "project-workspace holds the project's goals and decisions; a run's control state is scoped to run-state",
        "repository-scope" => "repository-scope holds facts true for one repository; a run may span repositories and is scoped to run-state",
        _ => "it is not run-state",
    }
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
/// Port of `sameAssignmentSet` — see the module doc comment for the
/// key-order boundary already accepted elsewhere in this workspace.
fn same_assignment_set(a: Option<&Value>, b: Option<&Value>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

fn display_opt(v: Option<&Value>) -> String {
    match v {
        None => "null".to_string(),
        Some(Value::Null) => "null".to_string(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

struct SchemaLabels<'a> {
    expected_ref: String,
    expected_basename: &'a str,
    canonical_base: &'a str,
    label: &'a str,
}

/// Shared: apply the reused envelope schema separately, then the
/// specialised schema. Port of `applySchemas`.
fn apply_schemas(
    doc: &Value,
    envelope_schema: &Value,
    record_schema: &Value,
    label: &str,
) -> Result<Vec<String>, Vec<String>> {
    let mut problems = Vec::new();
    match json_schema::validate(doc, envelope_schema) {
        Ok(errors) => problems.extend(errors.into_iter().map(|m| format!("envelope {m}"))),
        Err(error) => {
            return Err(vec![format!(
                "record envelope schema could not be applied: {error}"
            )])
        }
    }
    match json_schema::validate(doc, record_schema) {
        Ok(errors) => problems.extend(errors),
        Err(error) => {
            return Err(vec![format!(
                "{label} schema could not be applied: {error}"
            )])
        }
    }
    Ok(problems)
}

/// Shared: the record's `$schema` declaration must be a portable relative
/// reference that RESOLVES, within the Meridian namespace, to
/// `labels.expected_ref`. Port of `checkSchemaDeclaration`.
fn check_schema_declaration(
    problems: &mut Vec<String>,
    doc: &Value,
    id: &str,
    labels: &SchemaLabels,
) {
    let declared = doc.get("$schema").and_then(Value::as_str);
    let Some(declared) = declared.filter(|s| !s.is_empty()) else {
        let label = labels.label;
        let expected_basename = labels.expected_basename;
        problems.push(format!(
            "{label} \"{id}\" declares no $schema; a record names {expected_basename} so a consumer validates the whole contract — id, scope, origin, authority and body — not only the envelope"
        ));
        return;
    };
    if let Some(portability) = non_portable_reason(Some(declared)) {
        let label = labels.label;
        let canonical_base = labels.canonical_base;
        problems.push(format!(
            "{label} \"{id}\" $schema \"{declared}\" is not portable ({portability}); the schema is named by a relative reference resolved inside the Meridian namespace (canonical logical base {canonical_base})"
        ));
        return;
    }
    let resolved = resolve_schema_ref(Some(declared));
    if resolved.as_deref() == Some(envelope_schema_ref().as_str()) {
        let label = labels.label;
        let expected_basename = labels.expected_basename;
        problems.push(format!(
            "{label} \"{id}\" $schema \"{declared}\" resolves to the record envelope ({ENVELOPE_SCHEMA_BASENAME}); it must name {expected_basename}, which composes the envelope with the body"
        ));
    } else if resolved.as_deref() != Some(labels.expected_ref.as_str()) {
        let label = labels.label;
        let expected_ref = &labels.expected_ref;
        problems.push(format!(
            "{label} \"{id}\" $schema \"{declared}\" does not resolve to the logical address {expected_ref} within the Meridian namespace; the specialised schema is named by a portable relative reference (a bare basename, a missing namespace segment, a wrong segment and a reference climbing out of the namespace all resolve elsewhere)"
        ));
    }
}

/// Shared: the envelope reference strings travel inside the portable
/// record too. Port of `checkEnvelopeRefs`.
fn check_envelope_refs(problems: &mut Vec<String>, doc: &Value, id: &str, label: &str) {
    let empty_obj = Value::Object(Default::default());
    let origin = doc
        .get("origin")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);
    let authority = doc
        .get("authority")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);
    for (obj, field, name) in [
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
                    "{label} \"{id}\" {name} contains {r}; the record is portable and carries no rooted machine path"
                ));
            }
        }
    }
}

/// The result of validating one role-assignment set: which actors appear,
/// and which roles each holds.
pub struct AssignmentSet {
    pub actors: std::collections::HashSet<String>,
    pub actor_roles: std::collections::HashMap<String, std::collections::HashSet<String>>,
}

/// Shared: validate one role-assignment set. Rejects a blank actor, an
/// empty or unknown roles list, a duplicate role within an actor, a
/// duplicate actor. Port of `checkAssignmentSet`.
fn check_assignment_set(
    problems: &mut Vec<String>,
    id: &str,
    label: &str,
    list: Option<&Value>,
) -> AssignmentSet {
    let mut actors = std::collections::HashSet::new();
    let mut actor_roles: std::collections::HashMap<String, std::collections::HashSet<String>> =
        std::collections::HashMap::new();

    let arr = list.and_then(Value::as_array);
    let Some(arr) = arr.filter(|a| !a.is_empty()) else {
        problems.push(format!(
            "human-control record \"{id}\" {label} carries no role assignments; a run has at least one assigned participant"
        ));
        return AssignmentSet {
            actors,
            actor_roles,
        };
    };

    for (i, a) in arr.iter().enumerate() {
        let ao = if is_object(a) { a.clone() } else { Value::Null };
        let actor = ao.get("actor").and_then(Value::as_str).map(str::to_string);
        let at = actor
            .as_deref()
            .filter(|a| !blank(&Value::String(a.to_string())))
            .map(|a| format!("\"{a}\""))
            .unwrap_or_else(|| format!("#{i}"));
        let mut current_actor: Option<String> = None;
        match &actor {
            Some(actor_str) if !actor_str.trim().is_empty() => {
                if let Some(r) = non_portable_reason(Some(actor_str)) {
                    problems.push(format!(
                        "human-control record \"{id}\" {label} assignment {at} actor contains {r}"
                    ));
                }
                if actors.contains(actor_str) {
                    problems.push(format!(
                        "human-control record \"{id}\" {label} assigns actor \"{actor_str}\" more than once; combine an actor's roles in one assignment"
                    ));
                }
                actors.insert(actor_str.clone());
                actor_roles.entry(actor_str.clone()).or_default();
                current_actor = Some(actor_str.clone());
            }
            _ => {
                problems.push(format!(
                    "human-control record \"{id}\" {label} assignment {at} names no actor; an empty or ambiguous assignment is rejected"
                ));
            }
        }

        let roles = ao.get("roles").and_then(Value::as_array);
        let Some(roles) = roles.filter(|r| !r.is_empty()) else {
            problems.push(format!(
                "human-control record \"{id}\" {label} assignment {at} lists no role; an empty or ambiguous assignment is rejected"
            ));
            continue;
        };
        let mut here = current_actor
            .as_ref()
            .and_then(|a| actor_roles.get(a).cloned())
            .unwrap_or_default();
        for role in roles {
            let Some(role_str) = role.as_str() else {
                problems.push(format!(
                    "human-control record \"{id}\" {label} assignment {at} names role {}, which is not in the closed pool {}",
                    display_opt(Some(role)),
                    ROLES.join(", ")
                ));
                continue;
            };
            if !ROLES.contains(&role_str) {
                problems.push(format!(
                    "human-control record \"{id}\" {label} assignment {at} names role \"{role_str}\", which is not in the closed pool {}",
                    ROLES.join(", ")
                ));
                continue;
            }
            if here.contains(role_str) {
                problems.push(format!(
                    "human-control record \"{id}\" {label} assignment {at} assigns role \"{role_str}\" more than once"
                ));
            }
            here.insert(role_str.to_string());
        }
        if let Some(actor_str) = &current_actor {
            actor_roles.insert(actor_str.clone(), here);
        }
    }

    AssignmentSet {
        actors,
        actor_roles,
    }
}

pub struct RoleRegistrySchemas<'a> {
    pub registry_schema: &'a Value,
    pub envelope_schema: &'a Value,
}

/// 1. the built-in role catalogue. Port of `evaluateRoleRegistry`.
pub fn evaluate_role_registry(doc: &Value, schemas: &RoleRegistrySchemas) -> Vec<String> {
    let mut problems = match apply_schemas(
        doc,
        schemas.envelope_schema,
        schemas.registry_schema,
        "role-registry",
    ) {
        Ok(problems) => problems,
        Err(problems) => return problems,
    };

    if !is_object(doc) {
        if problems.is_empty() {
            return vec!["the role registry record is not an object".to_string()];
        }
        return problems;
    }
    let id = doc
        .get("id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or("(no id)")
        .to_string();

    check_schema_declaration(
        &mut problems,
        doc,
        &id,
        &SchemaLabels {
            expected_ref: registry_schema_ref(),
            expected_basename: REGISTRY_SCHEMA_BASENAME,
            canonical_base: CANONICAL_REGISTRY_BASE,
            label: "role registry",
        },
    );

    let record_type = doc.get("record_type").and_then(Value::as_str).unwrap_or("");
    if record_type != ROLE_REGISTRY_RECORD_TYPE {
        problems.push(format!(
            "role registry \"{id}\" declares record_type \"{record_type}\", not \"{ROLE_REGISTRY_RECORD_TYPE}\"; the record type names the catalogue and does not open a second envelope"
        ));
    }
    if !doc
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(is_semantic_id)
    {
        problems.push(format!(
            "role registry \"{id}\" has no stable semantic id on the record envelope"
        ));
    }
    let title = doc.get("title").and_then(Value::as_str);
    match title {
        None => problems.push(format!("role registry \"{id}\" has no human-readable title")),
        Some(t) if t.trim().is_empty() => {
            problems.push(format!("role registry \"{id}\" has no human-readable title"))
        }
        Some(t) if !has_cyrillic(t) => problems.push(format!(
            "role registry \"{id}\" title \"{t}\" carries no Russian (Cyrillic) text; the catalogue name is stated in Russian for the human reader"
        )),
        _ => {}
    }
    if let Some(r) = non_portable_reason(title) {
        problems.push(format!("role registry \"{id}\" title contains {r}"));
    }

    let empty_obj = Value::Object(Default::default());
    let scope = doc
        .get("scope")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);
    if let Some(scope_type) = scope.get("type").and_then(Value::as_str) {
        if scope_type != REGISTRY_SCOPE_TYPE {
            problems.push(format!(
                "role registry \"{id}\" is scoped to \"{scope_type}\"; the built-in role catalogue is Kernel methodology and lives in built-in-methodology only"
            ));
        }
    }
    let origin = doc
        .get("origin")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);
    if let Some(kind) = origin.get("kind").and_then(Value::as_str) {
        if kind != "built-in" {
            problems.push(format!(
                "role registry \"{id}\" declares origin.kind \"{kind}\"; the built-in role catalogue is shipped with the methodology (origin.kind built-in)"
            ));
        }
    }
    let authority = doc
        .get("authority")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);
    if let Some(kind) = authority.get("kind").and_then(Value::as_str) {
        if kind != "methodology-owner" {
            problems.push(format!(
                "role registry \"{id}\" declares authority.kind \"{kind}\"; the built-in role catalogue is methodology-owner authority"
            ));
        }
    }
    check_envelope_refs(&mut problems, doc, &id, "role registry");

    let payload = doc
        .get("payload")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);
    let roles_val = payload.get("roles");
    let roles = roles_val.and_then(Value::as_array);
    match (roles_val, roles) {
        (Some(_), None) => problems.push(format!("role registry \"{id}\" payload.roles is not a list")),
        (None, _) => problems.push(format!(
            "role registry \"{id}\" states no roles; the catalogue body carries the closed pool of universal roles"
        )),
        (_, Some(roles)) => {
            let mut seen = std::collections::HashSet::new();
            for (i, r) in roles.iter().enumerate() {
                let ro = if is_object(r) { r.clone() } else { Value::Null };
                let rid = ro.get("id").and_then(Value::as_str);
                let at = rid.map(str::to_string).unwrap_or_else(|| format!("#{i}"));
                match rid {
                    Some(rid) if ROLES.contains(&rid) => {
                        if !seen.insert(rid.to_string()) {
                            problems.push(format!(
                                "role registry \"{id}\" role \"{rid}\" is declared more than once"
                            ));
                        }
                    }
                    _ => problems.push(format!(
                        "role registry \"{id}\" role {at} is not one of the closed pool {}",
                        ROLES.join(", ")
                    )),
                }
                let title = ro.get("title");
                if title.map(blank).unwrap_or(true) {
                    problems.push(format!("role registry \"{id}\" role {at} has no Russian title"));
                } else if !title.and_then(Value::as_str).is_some_and(has_cyrillic) {
                    problems.push(format!(
                        "role registry \"{id}\" role {at} title \"{}\" carries no Russian (Cyrillic) text",
                        title.and_then(Value::as_str).unwrap_or("")
                    ));
                }
                if ro.get("summary").map(blank).unwrap_or(true) {
                    problems.push(format!("role registry \"{id}\" role {at} has no summary"));
                }
                let resp = ro.get("responsibilities").and_then(Value::as_array);
                match resp.filter(|r| !r.is_empty()) {
                    None => problems.push(format!(
                        "role registry \"{id}\" role {at} states no responsibilities; a universal role names its powers and duties"
                    )),
                    Some(resp) => {
                        for (j, s) in resp.iter().enumerate() {
                            if blank(s) {
                                problems.push(format!(
                                    "role registry \"{id}\" role {at} responsibility #{j} is empty or whitespace-only"
                                ));
                            }
                            if let Some(pr) = non_portable_reason(s.as_str()) {
                                problems.push(format!(
                                    "role registry \"{id}\" role {at} responsibility #{j} contains {pr}"
                                ));
                            }
                        }
                    }
                }
                for s in [ro.get("title").and_then(Value::as_str), ro.get("summary").and_then(Value::as_str)] {
                    if let Some(pr) = non_portable_reason(s) {
                        problems.push(format!("role registry \"{id}\" role {at} contains {pr}"));
                    }
                }
            }
            let missing: Vec<&str> = ROLES.iter().filter(|r| !seen.contains(**r)).copied().collect();
            if !missing.is_empty() {
                problems.push(format!(
                    "role registry \"{id}\" is missing universal role(s) {}; the pool is closed and fully covered",
                    missing.join(", ")
                ));
            }
        }
    }

    problems
}

pub struct HumanControlSchemas<'a> {
    pub record_schema: &'a Value,
    pub envelope_schema: &'a Value,
}

/// 2. one run's human-control record. Port of `evaluateHumanControl`.
pub fn evaluate_human_control(doc: &Value, schemas: &HumanControlSchemas) -> Vec<String> {
    let mut problems = match apply_schemas(
        doc,
        schemas.envelope_schema,
        schemas.record_schema,
        "human-control",
    ) {
        Ok(problems) => problems,
        Err(problems) => return problems,
    };

    if !is_object(doc) {
        if problems.is_empty() {
            return vec!["the human-control record is not an object".to_string()];
        }
        return problems;
    }
    let id = doc
        .get("id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .unwrap_or("(no id)")
        .to_string();

    check_schema_declaration(
        &mut problems,
        doc,
        &id,
        &SchemaLabels {
            expected_ref: control_schema_ref(),
            expected_basename: CONTROL_SCHEMA_BASENAME,
            canonical_base: CANONICAL_CONTROL_BASE,
            label: "human-control record",
        },
    );

    let record_type = doc.get("record_type").and_then(Value::as_str).unwrap_or("");
    if record_type != HUMAN_CONTROL_RECORD_TYPE {
        problems.push(format!(
            "human-control record \"{id}\" declares record_type \"{record_type}\", not \"{HUMAN_CONTROL_RECORD_TYPE}\""
        ));
    }
    if !doc
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(is_semantic_id)
    {
        problems.push(format!(
            "human-control record \"{id}\" has no stable semantic id on the record envelope"
        ));
    }
    let title = doc.get("title").and_then(Value::as_str);
    match title {
        None => problems.push(format!("human-control record \"{id}\" has no human-readable title")),
        Some(t) if t.trim().is_empty() => problems.push(format!(
            "human-control record \"{id}\" has no human-readable title"
        )),
        Some(t) if !has_cyrillic(t) => problems.push(format!(
            "human-control record \"{id}\" title \"{t}\" carries no Russian (Cyrillic) text; the record name is stated in Russian for the human reader"
        )),
        _ => {}
    }
    if let Some(r) = non_portable_reason(title) {
        problems.push(format!("human-control record \"{id}\" title contains {r}"));
    }

    let empty_obj = Value::Object(Default::default());
    let scope = doc
        .get("scope")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);
    let scope_type = scope.get("type").and_then(Value::as_str);
    if let Some(scope_type_str) = scope_type {
        if scope_type_str != CONTROL_SCOPE_TYPE {
            let why = control_scope_rejection_reason(scope_type_str);
            problems.push(format!(
                "human-control record \"{id}\" is scoped to \"{scope_type_str}\"; a run's control state lives in run-state only — {why}"
            ));
        }
    }
    if scope_type == Some(CONTROL_SCOPE_TYPE) {
        if !scope
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(is_semantic_id)
        {
            problems.push(format!(
                "human-control record \"{id}\" run-state scope carries no stable scope.id identifying the run"
            ));
        }
        if !scope
            .get("workspace_id")
            .and_then(Value::as_str)
            .is_some_and(is_semantic_id)
        {
            problems.push(format!(
                "human-control record \"{id}\" run-state scope carries no workspace_id; a run belongs to a project workspace (workspace-scope-model.md §1)"
            ));
        }
    }
    let origin = doc
        .get("origin")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);
    if origin.get("kind").and_then(Value::as_str) == Some("built-in") {
        problems.push(format!(
            "human-control record \"{id}\" declares origin.kind \"built-in\"; a run's control state is established in a workspace, not shipped with the methodology"
        ));
    }
    check_envelope_refs(&mut problems, doc, &id, "human-control record");

    let payload = doc
        .get("payload")
        .filter(|v| is_object(v))
        .unwrap_or(&empty_obj);

    let run_ref = payload.get("execution_run_ref");
    match run_ref {
        None => problems.push(format!(
            "human-control record \"{id}\" names no execution_run_ref; a control record references exactly one execution run"
        )),
        Some(v) if is_object(v) || v.is_array() => problems.push(format!(
            "human-control record \"{id}\" execution_run_ref is not a plain reference; the record references the run, it does not embed run state"
        )),
        Some(v) if blank(v) => problems.push(format!(
            "human-control record \"{id}\" execution_run_ref is empty or whitespace-only"
        )),
        Some(v) => {
            if let Some(r) = non_portable_reason(v.as_str()) {
                problems.push(format!(
                    "human-control record \"{id}\" execution_run_ref contains {r}; the reference is portable and is not an absolute machine path"
                ));
            }
        }
    }

    let current = check_assignment_set(
        &mut problems,
        &id,
        "current",
        payload.get("role_assignments"),
    );

    let authority_axis = payload.get("human_authority").filter(|v| is_object(v));
    let posture_ok = authority_axis
        .and_then(|a| a.get("posture"))
        .and_then(Value::as_str)
        == Some(HUMAN_AUTHORITY_POSTURE);
    if !posture_ok {
        problems.push(format!(
            "human-control record \"{id}\" does not state human_authority.posture \"{HUMAN_AUTHORITY_POSTURE}\"; human-in-command is the permanent basis of Meridian and cannot be replaced by a supervision mode or disabled"
        ));
    }
    if let Some(authority_axis) = authority_axis {
        let holder = authority_axis
            .get("holder")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty());
        match holder {
            None => problems.push(format!(
                "human-control record \"{id}\" human_authority names no holder; the record identifies the participant who holds human-in-command"
            )),
            Some(holder) => {
                if let Some(r) = non_portable_reason(Some(holder)) {
                    problems.push(format!(
                        "human-control record \"{id}\" human_authority.holder contains {r}"
                    ));
                }
                let holds_owner = current
                    .actor_roles
                    .get(holder)
                    .is_some_and(|roles| roles.contains(HUMAN_AUTHORITY_ROLE));
                if !holds_owner {
                    problems.push(format!(
                        "human-control record \"{id}\" human_authority.holder \"{holder}\" is not a participant assigned the {HUMAN_AUTHORITY_ROLE} role; human-in-command is held by an assigned owner"
                    ));
                }
            }
        }
    }

    let supervision = payload.get("supervision_mode").and_then(Value::as_str);
    if supervision == Some(HUMAN_AUTHORITY_POSTURE) {
        problems.push(format!(
            "human-control record \"{id}\" sets supervision_mode to \"{HUMAN_AUTHORITY_POSTURE}\"; human-in-command is not a switchable supervision mode — the switchable pool is {}",
            SUPERVISION_MODES.join(", ")
        ));
    } else if let Some(supervision_str) = supervision {
        if !SUPERVISION_MODES.contains(&supervision_str) {
            problems.push(format!(
                "human-control record \"{id}\" supervision_mode \"{supervision_str}\" is not in the closed pool {}",
                SUPERVISION_MODES.join(", ")
            ));
        }
    }
    let has_hitl = payload.get("hitl").is_some();
    let has_hotl = payload.get("hotl").is_some();
    if supervision == Some("human-in-the-loop") {
        let hitl = payload.get("hitl").filter(|v| is_object(v));
        let action_ok = hitl.is_some_and(|h| !blank_opt(h.get("required_human_action")));
        let gate_ok = hitl.is_some_and(|h| !blank_opt(h.get("gate")));
        if !(action_ok && gate_ok) {
            problems.push(format!(
                "human-control record \"{id}\" is human-in-the-loop but names no concrete required human action and declared gate; HITL means a specific human action on a stated gate is known"
            ));
        }
        if has_hotl {
            problems.push(format!(
                "human-control record \"{id}\" is human-in-the-loop but also carries an hotl block; the modes are mutually exclusive — human-in-the-loop forbids hotl"
            ));
        }
        for s in [
            hitl.and_then(|h| h.get("required_human_action"))
                .and_then(Value::as_str),
            hitl.and_then(|h| h.get("gate")).and_then(Value::as_str),
        ] {
            if let Some(r) = non_portable_reason(s) {
                problems.push(format!(
                    "human-control record \"{id}\" hitl block contains {r}"
                ));
            }
        }
    }
    if supervision == Some("human-on-the-loop") {
        let hotl = payload.get("hotl").filter(|v| is_object(v));
        let bounds = hotl
            .and_then(|h| h.get("autonomy_bounds"))
            .and_then(Value::as_array);
        let bounds_ok = bounds.is_some_and(|b| !b.is_empty());
        let intervention_ok = hotl.is_some_and(|h| !blank_opt(h.get("intervention")));
        if !(bounds_ok && intervention_ok) {
            problems.push(format!(
                "human-control record \"{id}\" is human-on-the-loop but states no autonomy bounds or intervention capability; HOTL means bounded autonomy inside pre-set bounds with a standing way to intervene"
            ));
        }
        if has_hitl {
            problems.push(format!(
                "human-control record \"{id}\" is human-on-the-loop but also carries an hitl block; the modes are mutually exclusive — human-on-the-loop forbids hitl"
            ));
        }
        let empty_bounds = Vec::new();
        for s in bounds
            .unwrap_or(&empty_bounds)
            .iter()
            .map(Value::as_str)
            .chain(std::iter::once(
                hotl.and_then(|h| h.get("intervention"))
                    .and_then(Value::as_str),
            ))
        {
            if let Some(r) = non_portable_reason(s) {
                problems.push(format!(
                    "human-control record \"{id}\" hotl block contains {r}"
                ));
            }
        }
    }

    if let Some(mode) = payload.get("communication_mode").and_then(Value::as_str) {
        if !COMMUNICATION_MODES.contains(&mode) {
            problems.push(format!(
                "human-control record \"{id}\" communication_mode \"{mode}\" is not in the closed pool {}",
                COMMUNICATION_MODES.join(", ")
            ));
        }
    }

    let acting_actor = payload.get("acting_actor");
    if blank_opt(acting_actor) {
        problems.push(format!(
            "human-control record \"{id}\" names no acting_actor"
        ));
    } else {
        let actor_str = acting_actor.and_then(Value::as_str).unwrap_or("");
        if let Some(r) = non_portable_reason(Some(actor_str)) {
            problems.push(format!(
                "human-control record \"{id}\" acting_actor contains {r}"
            ));
        }
        if !current.actors.contains(actor_str) {
            problems.push(format!(
                "human-control record \"{id}\" acting_actor \"{actor_str}\" is not assigned in this run; any participant in control must be present in role_assignments"
            ));
        }
    }

    let ri = payload.get("review_independence").filter(|v| is_object(v));
    if let Some(ri) = ri {
        let rev = ri.get("reviewer_actor").and_then(Value::as_str);
        let reviewed = ri.get("reviewed_actor").and_then(Value::as_str);
        for (v, name) in [(rev, "reviewer_actor"), (reviewed, "reviewed_actor")] {
            if let Some(v) = v.filter(|s| !s.is_empty()) {
                if let Some(r) = non_portable_reason(Some(v)) {
                    problems.push(format!(
                        "human-control record \"{id}\" review_independence.{name} contains {r}"
                    ));
                }
            }
        }
        let required = ri.get("required");
        if required == Some(&Value::Bool(true)) {
            let rev_ok = rev.is_some_and(|r| !r.trim().is_empty());
            let reviewed_ok = reviewed.is_some_and(|r| !r.trim().is_empty());
            if !rev_ok || !reviewed_ok {
                problems.push(format!(
                    "human-control record \"{id}\" requires review independence but does not name both a reviewer_actor and the reviewed_actor"
                ));
            } else {
                let rev = rev.unwrap();
                let reviewed = reviewed.unwrap();
                if rev == reviewed {
                    problems.push(format!(
                        "human-control record \"{id}\" requires review independence but names the same actor \"{rev}\" as reviewer and reviewed; an independent reviewer cannot be the participant whose result is independently reviewed"
                    ));
                }
                let suitable = current.actor_roles.get(rev).is_some_and(|roles| {
                    INDEPENDENT_REVIEW_ROLES.iter().any(|r| roles.contains(*r))
                });
                if !suitable {
                    problems.push(format!(
                        "human-control record \"{id}\" requires review independence but no separate actor is assigned a reviewing role ({}); the requirement needs a suitable independent participant",
                        INDEPENDENT_REVIEW_ROLES.join(" or ")
                    ));
                }
                if !current.actors.contains(reviewed) {
                    problems.push(format!(
                        "human-control record \"{id}\" review_independence.reviewed_actor \"{reviewed}\" is not assigned in this run"
                    ));
                }
            }
        } else if required == Some(&Value::Bool(false)) {
            let rev_present = rev.is_some();
            let reviewed_present = reviewed.is_some();
            if rev_present || reviewed_present {
                problems.push(format!(
                    "human-control record \"{id}\" review_independence.required is false but still names a reviewer_actor or reviewed_actor; drop both references, or drop the block, when independence is not required"
                ));
            }
        }
    }

    let history_val = payload.get("switch_history");
    let history = history_val.and_then(Value::as_array);
    match (history_val, history) {
        (Some(_), None) => problems.push(format!(
            "human-control record \"{id}\" switch_history is not an ordered list"
        )),
        (_, Some(history)) if history.is_empty() => problems.push(format!(
            "human-control record \"{id}\" switch_history is empty; the history is mandatory and its first record states the initial establishment"
        )),
        (_, Some(history)) => {
            let mut prev_seq: Option<i64> = None;
            for (i, sw) in history.iter().enumerate() {
                let s = if is_object(sw) { sw.clone() } else { Value::Null };
                let at = format!("switch #{i}");

                if blank_opt(s.get("reason")) {
                    problems.push(format!(
                        "human-control record \"{id}\" {at} states no reason; every change states its basis"
                    ));
                }
                if blank_opt(s.get("checkpoint_ref")) {
                    problems.push(format!(
                        "human-control record \"{id}\" {at} names no checkpoint_ref; a change preserves a verifiable checkpoint and never opens a new run"
                    ));
                }
                for (val, name) in [
                    (s.get("reason"), "reason"),
                    (s.get("checkpoint_ref"), "checkpoint_ref"),
                    (s.get("to_acting_actor"), "to_acting_actor"),
                ] {
                    if let Some(r) = non_portable_reason(val.and_then(Value::as_str)) {
                        problems.push(format!("human-control record \"{id}\" {at} {name} contains {r}"));
                    }
                }
                if blank_opt(s.get("to_acting_actor")) {
                    problems.push(format!(
                        "human-control record \"{id}\" {at} names no to_acting_actor"
                    ));
                }
                if let Some(mode) = s.get("to_supervision_mode").and_then(Value::as_str) {
                    if !SUPERVISION_MODES.contains(&mode) {
                        problems.push(format!(
                            "human-control record \"{id}\" {at} to_supervision_mode \"{mode}\" is not in the closed pool {}",
                            SUPERVISION_MODES.join(", ")
                        ));
                    }
                }
                if let Some(mode) = s.get("to_communication_mode").and_then(Value::as_str) {
                    if !COMMUNICATION_MODES.contains(&mode) {
                        problems.push(format!(
                            "human-control record \"{id}\" {at} to_communication_mode \"{mode}\" is not in the closed pool {}",
                            COMMUNICATION_MODES.join(", ")
                        ));
                    }
                }

                let to = check_assignment_set(
                    &mut problems,
                    &id,
                    &format!("{at} to_role_assignments"),
                    s.get("to_role_assignments"),
                );
                let to_acting_actor = s.get("to_acting_actor").and_then(Value::as_str);
                if let Some(to_actor) = to_acting_actor.filter(|a| !a.trim().is_empty()) {
                    if !to.actors.contains(to_actor) {
                        problems.push(format!(
                            "human-control record \"{id}\" {at} moves to acting actor \"{to_actor}\", who is not present in that transition's role assignments"
                        ));
                    }
                }

                let seq = s.get("sequence");
                if !seq.is_some_and(is_positive_int) {
                    problems.push(format!(
                        "human-control record \"{id}\" {at} has no positive integer sequence ordinal; the history is ordered by an explicit sequence, not by object or file order"
                    ));
                } else if let Some(prev) = prev_seq {
                    let n = seq.unwrap().as_i64().unwrap();
                    if n == prev {
                        problems.push(format!(
                            "human-control record \"{id}\" {at} repeats sequence ordinal {n}; ordinals do not repeat"
                        ));
                    } else if n < prev {
                        problems.push(format!(
                            "human-control record \"{id}\" {at} sequence ordinal {n} is below the previous {prev}; ordinals do not decrease"
                        ));
                    }
                }
                if seq.is_some_and(is_positive_int) {
                    prev_seq = seq.and_then(Value::as_i64);
                }

                if i == 0 {
                    let priors: Vec<&str> = [
                        ("from_supervision_mode", s.get("from_supervision_mode")),
                        ("from_communication_mode", s.get("from_communication_mode")),
                        ("from_acting_actor", s.get("from_acting_actor")),
                        ("from_role_assignments", s.get("from_role_assignments")),
                    ]
                    .into_iter()
                    .filter(|(_, v)| v.is_some_and(|v| !v.is_null()))
                    .map(|(k, _)| k)
                    .collect();
                    if !priors.is_empty() {
                        problems.push(format!(
                            "human-control record \"{id}\" {at} is the first record but names a prior {}; the first record states the initial establishment (every from_* axis is null)",
                            priors.join(", ")
                        ));
                    }
                } else {
                    let prev = history[i - 1].clone();
                    let prev = if is_object(&prev) { prev } else { Value::Null };
                    if s.get("from_supervision_mode") != prev.get("to_supervision_mode") {
                        problems.push(format!(
                            "human-control record \"{id}\" {at} from_supervision_mode \"{}\" does not continue the previous to_supervision_mode \"{}\"; the switch history has a break on the supervision-mode axis",
                            display_opt(s.get("from_supervision_mode")), display_opt(prev.get("to_supervision_mode"))
                        ));
                    }
                    if s.get("from_communication_mode") != prev.get("to_communication_mode") {
                        problems.push(format!(
                            "human-control record \"{id}\" {at} from_communication_mode \"{}\" does not continue the previous to_communication_mode \"{}\"; the switch history has a break on the communication-mode axis",
                            display_opt(s.get("from_communication_mode")), display_opt(prev.get("to_communication_mode"))
                        ));
                    }
                    if s.get("from_acting_actor") != prev.get("to_acting_actor") {
                        problems.push(format!(
                            "human-control record \"{id}\" {at} from_acting_actor \"{}\" does not continue the previous to_acting_actor \"{}\"; the switch history has a break on the acting-actor axis",
                            display_opt(s.get("from_acting_actor")), display_opt(prev.get("to_acting_actor"))
                        ));
                    }
                    if !same_assignment_set(s.get("from_role_assignments"), prev.get("to_role_assignments")) {
                        problems.push(format!(
                            "human-control record \"{id}\" {at} from_role_assignments does not continue the previous to_role_assignments; the switch history has a break on the role-assignment axis"
                        ));
                    }
                }
            }

            let last = history.last().cloned().unwrap_or(Value::Null);
            let last = if is_object(&last) { last } else { Value::Null };
            if last.get("to_supervision_mode").and_then(Value::as_str) != supervision {
                problems.push(format!(
                    "human-control record \"{id}\" last switch to_supervision_mode \"{}\" does not match the current supervision_mode \"{}\"",
                    display_opt(last.get("to_supervision_mode")), display_opt(supervision.map(|s| Value::String(s.to_string())).as_ref())
                ));
            }
            if last.get("to_communication_mode") != payload.get("communication_mode") {
                problems.push(format!(
                    "human-control record \"{id}\" last switch to_communication_mode \"{}\" does not match the current communication_mode \"{}\"",
                    display_opt(last.get("to_communication_mode")), display_opt(payload.get("communication_mode"))
                ));
            }
            if last.get("to_acting_actor") != acting_actor {
                problems.push(format!(
                    "human-control record \"{id}\" last switch to_acting_actor \"{}\" does not match the current acting_actor \"{}\"",
                    display_opt(last.get("to_acting_actor")), display_opt(acting_actor)
                ));
            }
            if !same_assignment_set(last.get("to_role_assignments"), payload.get("role_assignments")) {
                problems.push(format!(
                    "human-control record \"{id}\" last switch to_role_assignments does not match the current role_assignments"
                ));
            }
        }
        (None, None) => {}
    }

    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_real_kernel_role_registry_schema_and_fixtures_agree() {
        let kernel_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let registry_schema: Value = serde_json::from_str(
            &std::fs::read_to_string(
                kernel_root.join("registries/operating-model/role-registry.schema.json"),
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
        let fixtures: Value =
            serde_json::from_str(
                &std::fs::read_to_string(kernel_root.join(
                    "registries/operating-model/fixtures/role-and-human-control.fixtures.json",
                ))
                .unwrap(),
            )
            .unwrap();
        let schemas = RoleRegistrySchemas {
            registry_schema: &registry_schema,
            envelope_schema: &envelope_schema,
        };
        for case in fixtures["registry"]["valid"].as_array().unwrap() {
            let problems = evaluate_role_registry(&case["spec"], &schemas);
            assert!(
                problems.is_empty(),
                "note={:?} problems={:?}",
                case.get("note"),
                problems
            );
        }
        for case in fixtures["registry"]["invalid"].as_array().unwrap() {
            let problems = evaluate_role_registry(&case["spec"], &schemas);
            assert!(!problems.is_empty(), "note={:?}", case.get("note"));
        }

        // the shipped catalogue itself is the canonical valid role registry.
        let catalogue_yaml =
            std::fs::read_to_string(kernel_root.join("standards/workspace/role-registry.yaml"))
                .unwrap();
        let catalogue = crate::source_format::parse_yaml(&catalogue_yaml).unwrap();
        let problems = evaluate_role_registry(&catalogue, &schemas);
        assert!(problems.is_empty(), "{problems:?}");
    }

    #[test]
    fn the_real_kernel_human_control_schema_and_fixtures_agree() {
        let kernel_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let record_schema: Value = serde_json::from_str(
            &std::fs::read_to_string(
                kernel_root.join("registries/operating-model/human-control.schema.json"),
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
        let fixtures: Value =
            serde_json::from_str(
                &std::fs::read_to_string(kernel_root.join(
                    "registries/operating-model/fixtures/role-and-human-control.fixtures.json",
                ))
                .unwrap(),
            )
            .unwrap();
        let schemas = HumanControlSchemas {
            record_schema: &record_schema,
            envelope_schema: &envelope_schema,
        };
        for case in fixtures["control"]["valid"].as_array().unwrap() {
            let problems = evaluate_human_control(&case["spec"], &schemas);
            assert!(
                problems.is_empty(),
                "note={:?} problems={:?}",
                case.get("note"),
                problems
            );
        }
        for case in fixtures["control"]["invalid"].as_array().unwrap() {
            let problems = evaluate_human_control(&case["spec"], &schemas);
            assert!(!problems.is_empty(), "note={:?}", case.get("note"));
        }
    }
}
