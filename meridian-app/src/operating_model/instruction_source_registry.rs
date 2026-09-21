//! Verbatim port of `scripts/lib/instruction-source-registry.mjs`: the
//! composite-consistency algorithm behind the `instruction-source-registry`
//! check. `registries/operating-model/instruction-source-registry.schema.json`
//! is the specialised payload schema for a registered instruction source.
//! Registry DATA is Instance, like the intake register — the Kernel ships
//! the schema, the product-neutral fixtures and this module.
//!
//! Nothing here reads process state, touches the filesystem or exits: it
//! takes a parsed document plus the two schemas and returns a flat list of
//! problem strings — empty means valid. Location confinement is a pure
//! string check: a registered source may be external or simply absent from
//! this checkout, so its existence is never required.
//!
//! `read_channel` describes only who observes the SOURCE and how — the
//! agent's own tool ingests the file (agent-native), Meridian's registry
//! reads it to snapshot it (meridian-observed), or a person reports it
//! (manual). It is not a norm-delivery channel: registering or reading a
//! source grants its text no authority and does not replace
//! controlled-rule-intake.
//!
//! Temporal model
//! (`standards/workspace/instruction-source-registry.md` §2.6–§2.7):
//!   - `recorded_state` — the snapshot the registry currently holds.
//!   - `divergence.previous_state` — the snapshot held before the last
//!     check (historical; may differ from `recorded_state`, EXCEPT on
//!     source-missing / source-unreadable, where the last-known
//!     `recorded_state` == `previous_state`).
//!   - `divergence.current_state` — the fresh observation that check
//!     produced. It and `recorded_state` describe one present observation
//!     and must agree on revision, digest AND verification state.
//!
//! Two verified states are `unchanged` only when BOTH revision and SHA-256
//! digest match; a difference in either is `changed`. `unknown` is only for
//! genuinely insufficient evidence. `source-missing` / `source-unreadable`
//! carry no `current_state`. Currency binds to `revision_verified` and to
//! `divergence.status`: `stale` is a verified last-known state, never a
//! stand-in for an unverified one.

use serde_json::Value;

use crate::source_format::json_schema;

/// The closed pools the schema also carries. Re-stated here only so the
/// standalone Rust set can assert the two halves have not drifted.
pub const MEDIA: [&str; 2] = ["file", "external-service"];
pub const FORMATS: [&str; 6] = [
    "agents-md",
    "claude-md",
    "cursor-rule",
    "kernel-doc",
    "markdown-section",
    "plain-text",
];
pub const READ_CHANNEL_KINDS: [&str; 3] = ["meridian-observed", "agent-native", "manual"];
pub const MERIDIAN_VISIBILITY: [&str; 3] = ["full", "partial", "none"];
pub const CURRENCY: [&str; 3] = ["current", "stale", "unverified"];
pub const DIVERGENCE_STATES: [&str; 5] = [
    "unknown",
    "unchanged",
    "changed",
    "source-missing",
    "source-unreadable",
];

fn source_gone(status: &str) -> bool {
    status == "source-missing" || status == "source-unreadable"
}

fn is_object(v: &Value) -> bool {
    v.is_object()
}

fn obj(v: &Value) -> &Value {
    static EMPTY: Value = Value::Null;
    if is_object(v) {
        v
    } else {
        &EMPTY
    }
}

/// A location path is confined to its declared medium: relative, already
/// normalised, no absolute form and no escape through "..". The source
/// file itself is never required to exist. Port of `checkLocationPath`.
pub fn check_location_path(p: Option<&str>) -> Result<(), String> {
    let Some(p) = p.filter(|s| !s.is_empty()) else {
        return Err("the location path is empty".to_string());
    };
    if p.contains('\\') {
        return Err(
            "the location path uses a backslash as a separator; a source path is POSIX-relative"
                .to_string(),
        );
    }
    if is_windows_drive(p) || p.starts_with('/') {
        return Err(
            "the location path is absolute; a registered source is located by a relative path inside its declared medium"
                .to_string(),
        );
    }
    if p.split('/').any(|s| s.is_empty() || s == "." || s == "..") {
        return Err(
            "the location path carries an empty, \".\" or \"..\" segment; it must already be normalised and must not escape the declared medium"
                .to_string(),
        );
    }
    Ok(())
}

fn is_windows_drive(p: &str) -> bool {
    let bytes = p.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

/// An opaque identifier (`container_ref`, `service_ref`, `resource_ref`)
/// must not be a disguised filesystem location. Port of `opaqueRefProblem`.
fn opaque_ref_problem(reference: Option<&str>, label: &str) -> Option<String> {
    let r = reference.filter(|s| !s.is_empty())?;
    if r.contains('\\') || is_windows_drive(r) || r.starts_with('/') {
        return Some(format!(
            "{label} \"{r}\" looks like an absolute machine path; it must be an opaque identifier, not a filesystem location"
        ));
    }
    if r.split('/').any(|s| s == "..") {
        return Some(format!(
            "{label} \"{r}\" carries a \"..\" segment; it must be an opaque identifier"
        ));
    }
    None
}

fn as_str<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

fn is_verified_snapshot(s: &Value) -> bool {
    is_object(s)
        && s.get("verified") == Some(&Value::Bool(true))
        && as_str(s, "revision").is_some_and(|r| !r.is_empty())
        && is_object(s.get("digest").unwrap_or(&Value::Null))
        && as_str(s.get("digest").unwrap_or(&Value::Null), "algorithm") == Some("sha-256")
        && s.get("digest")
            .and_then(|d| d.get("value"))
            .and_then(Value::as_str)
            .is_some()
}

/// The canonical divergence status computed from two states. Two VERIFIED
/// states are "unchanged" only when both the revision AND the SHA-256
/// digest match; a difference in either is "changed". Anything short of two
/// verified states is "unknown" — the comparison cannot be made. Port of
/// `computeDivergence`.
pub fn compute_divergence(previous: &Value, current: &Value) -> &'static str {
    if !is_verified_snapshot(previous) || !is_verified_snapshot(current) {
        return "unknown";
    }
    let same_revision = as_str(previous, "revision") == as_str(current, "revision");
    let same_digest = previous.get("digest").and_then(|d| d.get("value"))
        == current.get("digest").and_then(|d| d.get("value"));
    if same_revision && same_digest {
        "unchanged"
    } else {
        "changed"
    }
}

/// A declared `divergence.status` must be consistent with its evidence.
/// Port of `checkDivergenceClaim`.
pub fn check_divergence_claim(divergence: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    let d = obj(divergence);
    let status = as_str(d, "status").unwrap_or("");
    let prev = d.get("previous_state").unwrap_or(&Value::Null);
    let cur = d.get("current_state").unwrap_or(&Value::Null);
    let both_verified = is_verified_snapshot(prev) && is_verified_snapshot(cur);

    if status == "unchanged" || status == "changed" {
        if !both_verified {
            problems.push(format!(
                "divergence status \"{status}\" needs a verifiable previous and current state (each with a revision, a sha-256 digest and verified: true); a changed, missing, unreadable or unverified source is never treated as matching"
            ));
        } else {
            let same_revision = as_str(prev, "revision") == as_str(cur, "revision");
            let same_digest = prev.get("digest").and_then(|d| d.get("value"))
                == cur.get("digest").and_then(|d| d.get("value"));
            let computed = if same_revision && same_digest {
                "unchanged"
            } else {
                "changed"
            };
            if status != computed {
                let detail = if same_revision && !same_digest {
                    "the SHA-256 digest differs while the revision is unchanged"
                } else if !same_revision && same_digest {
                    "the revision differs while the SHA-256 digest is unchanged"
                } else if !same_revision && !same_digest {
                    "both the revision and the SHA-256 digest differ"
                } else {
                    "the revision and the SHA-256 digest both match"
                };
                problems.push(format!(
                    "divergence status \"{status}\" contradicts the two verified states: {detail}, which computes \"{computed}\""
                ));
            }
        }
    } else if status == "unknown" {
        if both_verified {
            problems.push("divergence status \"unknown\" is declared with two complete verified states; when both states are present and verified the divergence is computable and the declared status must equal the computed one".to_string());
        }
    } else if source_gone(status) && is_object(cur) {
        problems.push(format!(
            "divergence status \"{status}\" carries a current_state; a source that is missing or unreadable has no current state to observe"
        ));
    }
    problems
}

/// Exported so a consumer resolving a source snapshot from OUTSIDE this
/// registry (`controlled-rule-intake`, property 10) can apply the exact
/// same agent-native/meridian-observed/manual coherence rules to a resolved
/// `read_channel` projection, instead of carrying a second, divergent copy.
/// Port of `checkReadChannel`.
pub fn check_read_channel(id: &str, rc: &Value, problems: &mut Vec<String>) {
    let at = format!("instruction source \"{id}\"");
    let kind = as_str(rc, "kind").unwrap_or("");
    let visibility = as_str(rc, "meridian_visibility").unwrap_or("");
    let auto = rc.get("agent_auto_read") == Some(&Value::Bool(true));

    if auto && kind != "agent-native" {
        problems.push(format!(
            "{at}: read_channel agent_auto_read is true but kind is \"{kind}\"; a source the agent's own tooling ingests by itself is the agent-native channel"
        ));
    }
    if auto && visibility == "full" {
        problems.push(format!(
            "{at}: read_channel agent_auto_read is true with meridian_visibility \"full\"; Meridian cannot fully observe a source the agent's own tool ingests"
        ));
    }
    if kind == "agent-native" && !auto {
        problems.push(format!(
            "{at}: read_channel kind is \"agent-native\" but agent_auto_read is not true; the agent-native channel is defined by the agent ingesting the source itself"
        ));
    }
    if kind == "meridian-observed" && auto {
        problems.push(format!(
            "{at}: read_channel kind is \"meridian-observed\" but agent_auto_read is true; the meridian-observed channel is Meridian's own read of the source, not the agent's"
        ));
    }
    if visibility == "full" && kind != "meridian-observed" {
        problems.push(format!(
            "{at}: read_channel meridian_visibility is \"full\" but kind is \"{kind}\"; full visibility of the source is possible only on the meridian-observed channel"
        ));
    }
}

/// `recorded_state` and `divergence` together describe one present
/// observation. This is where the temporal model is enforced. Port of
/// `checkStateCoherence`.
fn check_state_coherence(id: &str, payload: &Value, problems: &mut Vec<String>) {
    let at = format!("instruction source \"{id}\"");
    let rs = obj(payload.get("recorded_state").unwrap_or(&Value::Null));
    let d = obj(payload.get("divergence").unwrap_or(&Value::Null));
    let status = as_str(d, "status").unwrap_or("");
    let verified = rs.get("revision_verified") == Some(&Value::Bool(true));
    let currency = as_str(rs, "currency").unwrap_or("");

    if verified && currency == "unverified" {
        problems.push(format!(
            "{at}: recorded_state.revision_verified is true but currency is \"unverified\"; a verified snapshot is \"current\" or \"stale\", never \"unverified\""
        ));
    }
    if !verified && currency != "unverified" {
        problems.push(format!(
            "{at}: recorded_state.revision_verified is not true, so currency must be \"unverified\""
        ));
    }

    if currency == "current" {
        if !verified {
            problems.push(format!(
                "{at}: recorded_state.currency is \"current\" but revision_verified is not true; an unverified revision is not declared current"
            ));
        }
        if source_gone(status) {
            problems.push(format!(
                "{at}: recorded_state.currency is \"current\" but divergence.status is \"{status}\"; a source that is missing or unreadable cannot have a current snapshot — record the verified last-known state as \"stale\""
            ));
        }
    }

    if currency == "stale" {
        if !verified {
            problems.push(format!(
                "{at}: recorded_state.currency is \"stale\" but revision_verified is not true; \"stale\" is a verified last-known state — an unverified snapshot is \"unverified\""
            ));
        }
        if !source_gone(status) {
            problems.push(format!(
                "{at}: recorded_state.currency is \"stale\" but divergence.status is \"{status}\"; \"stale\" needs positive evidence (source-missing or source-unreadable) that the source no longer matches the held snapshot"
            ));
        }
    }

    let cur = d.get("current_state").unwrap_or(&Value::Null);
    if is_object(cur) {
        let cur_revision = as_str(cur, "revision");
        let rs_revision = as_str(rs, "revision");
        if let (Some(cr), Some(rr)) = (cur_revision, rs_revision) {
            if cr != rr {
                problems.push(format!(
                    "{at}: divergence.current_state.revision \"{cr}\" contradicts recorded_state.revision \"{rr}\"; both describe the source as it is now and must agree"
                ));
            }
        }
        let cur_val = cur
            .get("digest")
            .and_then(|v| v.get("value"))
            .and_then(Value::as_str);
        let rs_val = rs
            .get("digest")
            .and_then(|v| v.get("value"))
            .and_then(Value::as_str);
        if let (Some(cv), Some(rv)) = (cur_val, rs_val) {
            if cv != rv {
                problems.push(format!(
                    "{at}: divergence.current_state digest contradicts recorded_state digest; both describe the source as it is now and must agree"
                ));
            }
        }
        let cur_verified = cur.get("verified").and_then(Value::as_bool);
        let rs_verified = rs.get("revision_verified").and_then(Value::as_bool);
        if let (Some(cv), Some(rv)) = (cur_verified, rs_verified) {
            if cv != rv {
                problems.push(format!(
                    "{at}: divergence.current_state.verified is {cv} but recorded_state.revision_verified is {rv}; one present observation cannot be both verified and not verified"
                ));
            }
        }
    }

    let prev = d.get("previous_state").unwrap_or(&Value::Null);
    if source_gone(status) && is_object(prev) {
        let prev_revision = as_str(prev, "revision");
        let rs_revision = as_str(rs, "revision");
        if let (Some(pr), Some(rr)) = (prev_revision, rs_revision) {
            if pr != rr {
                problems.push(format!(
                    "{at}: divergence.status is \"{status}\" with a previous_state whose revision \"{pr}\" contradicts recorded_state.revision \"{rr}\"; the last-known state must match the snapshot the registry holds"
                ));
            }
        }
        let prev_val = prev
            .get("digest")
            .and_then(|v| v.get("value"))
            .and_then(Value::as_str);
        let rs_val2 = rs
            .get("digest")
            .and_then(|v| v.get("value"))
            .and_then(Value::as_str);
        if let (Some(pv), Some(rv)) = (prev_val, rs_val2) {
            if pv != rv {
                problems.push(format!(
                    "{at}: divergence.status is \"{status}\" with a previous_state whose digest contradicts recorded_state digest; the last-known state must match the snapshot the registry holds"
                ));
            }
        }
    }
}

pub struct EvalSchemas<'a> {
    pub registry_schema: &'a Value,
    pub envelope_schema: &'a Value,
}

/// The whole composition pipeline for one registry document: container +
/// payload schema, per-entry envelope schema, then the cross-cutting rules
/// the JSON Schema subset cannot state. Port of
/// `evaluateInstructionSourceRegistry`.
pub fn evaluate_instruction_source_registry(doc: &Value, schemas: &EvalSchemas) -> Vec<String> {
    let mut problems = Vec::new();

    match json_schema::validate(doc, schemas.registry_schema) {
        Ok(errors) => problems.extend(errors),
        Err(error) => {
            return vec![format!(
                "container/payload schema could not be applied: {error}"
            )]
        }
    }

    let empty = Vec::new();
    let entries: &Vec<Value> = doc
        .get("instruction_sources")
        .and_then(Value::as_array)
        .unwrap_or(&empty);

    for (i, entry) in entries.iter().enumerate() {
        match json_schema::validate(entry, schemas.envelope_schema) {
            Ok(errors) => problems.extend(
                errors
                    .into_iter()
                    .map(|m| format!("entry {i} envelope {m}")),
            ),
            Err(error) => {
                problems.push(format!("entry {i} envelope could not be applied: {error}"))
            }
        }
    }

    let mut seen_ids = std::collections::HashSet::new();
    for (index, e) in entries.iter().enumerate() {
        if !is_object(e) {
            continue;
        }
        let declared_id = as_str(e, "id");
        let id = declared_id
            .map(str::to_string)
            .unwrap_or_else(|| format!("#{index}"));
        if let Some(declared_id) = declared_id {
            if !seen_ids.insert(declared_id.to_string()) {
                problems.push(format!(
                    "instruction source id \"{declared_id}\" is declared more than once"
                ));
            }
        }
        let record_type = as_str(e, "record_type").unwrap_or("");
        if record_type != "instruction-source" {
            problems.push(format!(
                "instruction source \"{id}\" declares record_type \"{record_type}\", not \"instruction-source\"; registering a source never turns it into a norm"
            ));
        }

        let payload = e
            .get("payload")
            .filter(|p| is_object(p))
            .cloned()
            .unwrap_or(Value::Null);
        let payload = obj(&payload);

        let normative_status = as_str(payload, "normative_status").unwrap_or("");
        if normative_status != "not-a-norm" {
            problems.push(format!(
                "instruction source \"{id}\" payload.normative_status is \"{normative_status}\", not \"not-a-norm\"; registration grants the source's text no norm authority, accepts no rule and resolves no conflict"
            ));
        }

        let location = payload
            .get("location")
            .filter(|l| is_object(l))
            .cloned()
            .unwrap_or(Value::Null);
        let location = obj(&location);
        let medium = as_str(payload, "medium").unwrap_or("");
        if medium == "file" {
            if let Err(reason) = check_location_path(as_str(location, "path")) {
                let path_display = as_str(location, "path").unwrap_or("");
                problems.push(format!(
                    "instruction source \"{id}\" location.path \"{path_display}\": {reason}"
                ));
            }
        }
        for (field, label) in [
            ("container_ref", "location.container_ref"),
            ("service_ref", "location.service_ref"),
            ("resource_ref", "location.resource_ref"),
        ] {
            if let Some(problem) = opaque_ref_problem(
                as_str(location, field),
                &format!("instruction source \"{id}\" {label}"),
            ) {
                problems.push(problem);
            }
        }
        if let Some(missing_behavior) = location.get("missing_behavior") {
            if missing_behavior.as_str() != Some("fail-closed") {
                problems.push(format!(
                    "instruction source \"{id}\" location.missing_behavior is {}; the only declared behaviour for an unresolvable source is \"fail-closed\" — no hidden fallback resolution",
                    display_value(missing_behavior)
                ));
            }
        }

        check_state_coherence(&id, payload, &mut problems);
        if let Some(rc) = payload.get("read_channel").filter(|v| is_object(v)) {
            check_read_channel(&id, rc, &mut problems);
        }

        for p in check_divergence_claim(payload.get("divergence").unwrap_or(&Value::Null)) {
            problems.push(format!("instruction source \"{id}\" {p}"));
        }
    }

    problems
}

fn display_value(v: &Value) -> String {
    match v {
        Value::String(s) => format!("\"{s}\""),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn check_location_path_rejects_backslash() {
        assert!(check_location_path(Some("a\\b")).is_err());
    }

    #[test]
    fn check_location_path_rejects_dot_dot() {
        assert!(check_location_path(Some("a/../b")).is_err());
    }

    #[test]
    fn check_location_path_accepts_a_normalised_relative_path() {
        assert!(check_location_path(Some("a/b/c.md")).is_ok());
    }

    #[test]
    fn opaque_ref_problem_flags_a_disguised_absolute_path() {
        let problem = opaque_ref_problem(Some("/etc/passwd"), "location.container_ref").unwrap();
        assert!(problem.contains("absolute machine path"));
    }

    #[test]
    fn opaque_ref_problem_is_silent_on_a_plain_opaque_id() {
        assert!(opaque_ref_problem(Some("container-abc123"), "location.container_ref").is_none());
    }

    #[test]
    fn compute_divergence_is_unknown_without_two_verified_snapshots() {
        assert_eq!(compute_divergence(&Value::Null, &Value::Null), "unknown");
    }

    fn verified_snapshot(revision: &str, digest: &str) -> Value {
        json!({ "revision": revision, "digest": { "algorithm": "sha-256", "value": digest }, "verified": true })
    }

    #[test]
    fn compute_divergence_is_unchanged_when_revision_and_digest_match() {
        let a = verified_snapshot("r1", "d1");
        let b = verified_snapshot("r1", "d1");
        assert_eq!(compute_divergence(&a, &b), "unchanged");
    }

    #[test]
    fn compute_divergence_is_changed_when_digest_differs() {
        let a = verified_snapshot("r1", "d1");
        let b = verified_snapshot("r1", "d2");
        assert_eq!(compute_divergence(&a, &b), "changed");
    }

    #[test]
    fn check_divergence_claim_rejects_unchanged_without_two_verified_states() {
        let d = json!({ "status": "unchanged" });
        let problems = check_divergence_claim(&d);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("needs a verifiable previous and current state"));
    }

    #[test]
    fn check_divergence_claim_rejects_unchanged_that_contradicts_the_states() {
        let d = json!({
            "status": "unchanged",
            "previous_state": verified_snapshot("r1", "d1"),
            "current_state": verified_snapshot("r1", "d2"),
        });
        let problems = check_divergence_claim(&d);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("contradicts"));
    }

    #[test]
    fn check_divergence_claim_rejects_unknown_with_two_complete_verified_states() {
        let d = json!({
            "status": "unknown",
            "previous_state": verified_snapshot("r1", "d1"),
            "current_state": verified_snapshot("r1", "d1"),
        });
        let problems = check_divergence_claim(&d);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("computable"));
    }

    #[test]
    fn check_divergence_claim_rejects_source_missing_carrying_a_current_state() {
        let d = json!({
            "status": "source-missing",
            "current_state": verified_snapshot("r1", "d1"),
        });
        let problems = check_divergence_claim(&d);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("no current state to observe"));
    }

    #[test]
    fn check_read_channel_rejects_agent_auto_read_with_full_visibility() {
        let mut problems = Vec::new();
        check_read_channel(
            "s1",
            &json!({ "kind": "agent-native", "agent_auto_read": true, "meridian_visibility": "full" }),
            &mut problems,
        );
        assert!(problems.iter().any(|p| p.contains("cannot fully observe")));
    }

    #[test]
    fn check_read_channel_requires_agent_native_kind_to_set_agent_auto_read() {
        let mut problems = Vec::new();
        check_read_channel(
            "s1",
            &json!({ "kind": "agent-native", "agent_auto_read": false }),
            &mut problems,
        );
        assert!(problems.iter().any(|p| p.contains("agent-native")));
    }

    #[test]
    fn the_real_kernel_schema_and_fixtures_agree() {
        let kernel_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let registry_schema: Value = serde_json::from_str(
            &std::fs::read_to_string(
                kernel_root
                    .join("registries/operating-model/instruction-source-registry.schema.json"),
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
            &std::fs::read_to_string(kernel_root.join(
                "registries/operating-model/fixtures/instruction-source-registry.fixtures.json",
            ))
            .unwrap(),
        )
        .unwrap();
        let schemas = EvalSchemas {
            registry_schema: &registry_schema,
            envelope_schema: &envelope_schema,
        };
        for case in fixtures["valid"].as_array().unwrap() {
            let problems = evaluate_instruction_source_registry(&case["registry"], &schemas);
            assert!(
                problems.is_empty(),
                "note={:?} problems={:?}",
                case.get("note"),
                problems
            );
        }
        for case in fixtures["invalid"].as_array().unwrap() {
            let problems = evaluate_instruction_source_registry(&case["registry"], &schemas);
            assert!(!problems.is_empty(), "note={:?}", case.get("note"));
        }
    }
}
