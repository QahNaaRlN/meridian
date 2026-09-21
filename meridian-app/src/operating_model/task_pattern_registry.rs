//! Verbatim port of `scripts/lib/task-pattern-registry.mjs`: the
//! composite-consistency algorithm behind the `task-pattern-registry`
//! check. `standards/workspace/task-pattern-registry.yaml` (the built-in
//! universal task-type catalog) is a mandatory part of every Kernel. Its
//! container/payload shape is checked against
//! `registries/operating-model/task-pattern-registry.schema.json`, each
//! record's envelope against the existing
//! `registries/operating-model/scoped-record.schema.json` (composition, not
//! a second envelope schema), and the cross-record rules the Draft 7 subset
//! cannot state — unique pattern ids, the seven mandatory classification
//! pairs each declared exactly once, canonical-link path confinement and
//! classification, and the specific REFACTOR/BUGFIX/initiative wiring rules
//! — are ported verbatim from `evaluateTaskPatternRegistry`. A text guard
//! also keeps `standards/workspace/rule-resolution.md` from re-asserting
//! that `BUGFIX` routes to a Kernel *protocol* (`bugfix-protocol` is a
//! skill, owner decision).
//!
//! Everything here is a pure function over already-parsed values, with one
//! narrow exception the Node reference itself cannot avoid either: a
//! `present` canonical link must resolve to a real, tracked file inside the
//! Kernel, which is inescapably a filesystem question. That one check is
//! expressed as a caller-supplied callback (`EvalContext::check_kernel_link`)
//! rather than an `fs` call made from this crate: `meridian-cli` implements
//! it with real `std::fs::canonicalize` against the real tracked-file set
//! and passes it in, exactly the same separation
//! `meridian_app::operating_model::bounded_context_manifest` uses for its
//! own external record-resolution boundary. File I/O, YAML/JSON parsing and
//! the tracked-file set itself all stay in `meridian-cli`'s own module.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::source_format::json_schema;

/// The existing closed pools (`standards/workspace/rule-resolution.md`
/// §2–§3). The catalog reuses them; it must not grow a second vocabulary.
pub const WORK_KINDS: [&str; 4] = ["assessment", "operation", "initiative", "change"];
pub const CHANGE_CLASSES: [&str; 4] = ["BUGFIX", "FEATURE", "BEHAVIOR_CHANGE", "REFACTOR"];

/// `(work_kind, change_class)` — `change_class` is `None` for the three
/// non-`change` work kinds, `Some` for each of the four `change` classes.
pub const REQUIRED_PAIRS: [(&str, Option<&str>); 7] = [
    ("assessment", None),
    ("operation", None),
    ("initiative", None),
    ("change", Some("BUGFIX")),
    ("change", Some("FEATURE")),
    ("change", Some("BEHAVIOR_CHANGE")),
    ("change", Some("REFACTOR")),
];

pub const LINK_FIELDS: [&str; 3] = [
    "applicable_protocols",
    "applicable_skills",
    "applicable_evidence_contracts",
];

pub const REFACTOR_PROTOCOL: &str = "verification/functional-parity/refactor-protocol.md";
pub const REFACTOR_EVIDENCE: &str =
    "verification/functional-parity/functional-parity-evidence-contract.md";
pub const BUGFIX_SKILL: &str = "skills/bugfix-protocol/SKILL.md";

fn is_skill_artifact_path(p: &str) -> bool {
    let Some(rest) = p.strip_prefix("skills/") else {
        return false;
    };
    // Exactly one non-empty segment, then "/SKILL.md" — mirrors
    // `^skills\/[^/]+\/SKILL\.md$`.
    match rest.strip_suffix("/SKILL.md") {
        Some(middle) => !middle.is_empty() && !middle.contains('/'),
        None => false,
    }
}

fn is_under_skills(p: &str) -> bool {
    p == "skills" || p.starts_with("skills/")
}

fn present_links(links: &Value) -> Vec<&Value> {
    links
        .as_array()
        .into_iter()
        .flatten()
        .filter(|l| l.get("status").and_then(Value::as_str) == Some("present"))
        .collect()
}

fn present_paths(links: &Value) -> Vec<String> {
    present_links(links)
        .into_iter()
        .filter_map(|l| l.get("path").and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

fn pair_label(wk: &str, cc: Option<&str>) -> String {
    match cc {
        Some(cc) => format!("({wk}, {cc})"),
        None => format!("({wk})"),
    }
}

/// Port of `checkRuleResolutionBugfixConsistency`: `rule-resolution.md` must
/// not, at the same time the catalog treats `bugfix-protocol` as a skill,
/// still call `BUGFIX → bugfix-protocol` a route to a Kernel *protocol*.
pub fn check_rule_resolution_bugfix_consistency(text: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let arrow_re = fancy_regex::Regex::new(r"BUGFIX\s*(?:\u{2192}|-&gt;|->|-->)\s*bugfix-protocol")
        .expect("arrow pattern compiles");
    if arrow_re.is_match(text).unwrap_or(false) {
        problems.push("rule-resolution.md still routes \"BUGFIX → bugfix-protocol\" as a protocol route; bugfix-protocol is a skill (owner decision), and no separate BUGFIX protocol exists".to_string());
    }
    let bugfix_re = fancy_regex::Regex::new(r"bugfix-protocol").expect("bugfix pattern compiles");
    let kernel_protocol_re =
        fancy_regex::Regex::new(r"(?i)протокол\s+ядра").expect("kernel-protocol pattern compiles");
    for line in text.split('\n') {
        if bugfix_re.is_match(line).unwrap_or(false)
            && kernel_protocol_re.is_match(line).unwrap_or(false)
        {
            problems.push("rule-resolution.md names bugfix-protocol on the same line as \"протокол ядра\"; bugfix-protocol is a skill, not a Kernel protocol".to_string());
            break;
        }
    }
    problems
}

/// The portable-path shape a canonical link's `path` must have before its
/// filesystem TARGET is ever looked at: non-empty, POSIX-relative
/// (`/`-separated, never a backslash), not absolute (a leading `/` or a
/// drive letter), already normalised (no empty, `.` or `..` segment). None
/// of this touches the filesystem, so it stays here rather than folded
/// into the `check_kernel_link` callback `meridian-cli` supplies — only the
/// question "does this resolve to a real, tracked, regular file" needs
/// that boundary. Port of the non-filesystem half of `checkKernelLinkTarget`.
fn portable_relative_path_defect(rel_path: &str) -> Option<String> {
    if rel_path.is_empty() {
        return Some("the path is empty".to_string());
    }
    if rel_path.contains('\\') {
        return Some(
            "the path uses a backslash as a separator; a Kernel path is POSIX-relative".to_string(),
        );
    }
    if rel_path.starts_with('/') || is_windows_absolute(rel_path) {
        return Some(
            "the path is absolute; a canonical link is a relative path inside the Kernel"
                .to_string(),
        );
    }
    let segments: Vec<&str> = rel_path.split('/').collect();
    if segments
        .iter()
        .any(|s| s.is_empty() || *s == "." || *s == "..")
    {
        return Some(
            "the path carries an empty, \".\" or \"..\" segment; it must already be normalised"
                .to_string(),
        );
    }
    None
}

fn is_windows_absolute(p: &str) -> bool {
    let bytes = p.as_bytes();
    bytes.len() >= 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes.len() == 2 || bytes[2] == b'/' || bytes[2] == b'\\')
}

fn check_link_classification(pattern_id: &str, field: &str, link: &Value) -> Option<String> {
    let p = link.get("path").and_then(Value::as_str).unwrap_or("");
    if field == "applicable_skills" {
        if !is_skill_artifact_path(p) {
            return Some(format!(
                "pattern \"{pattern_id}\" applicable_skills link \"{p}\" is not a skill package (skills/<name>/SKILL.md); a protocol or contract is a different axis"
            ));
        }
        return None;
    }
    if is_under_skills(p) {
        return Some(format!(
            "pattern \"{pattern_id}\" {field} link \"{p}\" points at a skill package; a way of carrying work out belongs in applicable_skills, not among protocols or evidence contracts"
        ));
    }
    None
}

/// The external file-target boundary: a `present` canonical link's `path`
/// resolved to `Ok(())` when it is a real, tracked, regular file genuinely
/// inside the Kernel, or `Err(reason)` otherwise. `meridian-cli` supplies
/// the real implementation (`std::fs::canonicalize` against the real
/// tracked-file set); this crate never calls it directly.
pub type KernelLinkCheck<'a> = dyn Fn(&str) -> Result<(), String> + 'a;

pub struct EvalContext<'a> {
    pub registry_schema: &'a Value,
    pub envelope_schema: &'a Value,
    pub check_kernel_link: &'a KernelLinkCheck<'a>,
}

/// Port of `evaluateTaskPatternRegistry`: the whole composition pipeline for
/// one registry document — container + payload schema, per-record envelope
/// schema, then the cross-record rules the JSON Schema subset cannot state.
pub fn evaluate_task_pattern_registry(doc: &Value, ctx: &EvalContext) -> Vec<String> {
    let mut problems = Vec::new();

    match json_schema::validate(doc, ctx.registry_schema) {
        Ok(errors) => problems.extend(errors),
        Err(error) => {
            return vec![format!(
                "container/payload schema could not be applied: {error}"
            )]
        }
    }

    let empty_vec = Vec::new();
    let entries: &Vec<Value> = doc
        .get("task_patterns")
        .and_then(Value::as_array)
        .unwrap_or(&empty_vec);

    for (i, entry) in entries.iter().enumerate() {
        match json_schema::validate(entry, ctx.envelope_schema) {
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

    let mut seen_ids = HashSet::new();
    for p in entries {
        let Some(id) = p.get("id").and_then(Value::as_str) else {
            continue;
        };
        if !seen_ids.insert(id.to_string()) {
            problems.push(format!("pattern id \"{id}\" is declared more than once"));
        }
        let record_type = p.get("record_type").and_then(Value::as_str).unwrap_or("");
        if record_type != "task-pattern" {
            problems.push(format!(
                "pattern \"{id}\" declares record_type \"{record_type}\", not \"task-pattern\""
            ));
        }
    }

    // A `HashMap` iterates in an arbitrary, hash-dependent order; the Node
    // reference's `pairCount` is a `Map`, which iterates in first-insertion
    // order — the order pairs are actually declared in the document, not an
    // incidental one. `pair_order` reproduces that so that, when several
    // pairs are simultaneously over-declared, the diagnostics below name
    // them in the same order both languages agree on, not whichever this
    // process's hasher happens to visit first.
    let mut pair_count: HashMap<(String, Option<String>), usize> = HashMap::new();
    let mut pair_order: Vec<(String, Option<String>)> = Vec::new();
    for p in entries {
        let payload = p.get("payload");
        let wk = payload
            .and_then(|v| v.get("work_kind"))
            .and_then(Value::as_str);
        let cc = payload
            .and_then(|v| v.get("change_class"))
            .and_then(Value::as_str);
        let id = p.get("id").and_then(Value::as_str).unwrap_or("");
        if let Some(wk) = wk {
            let key = (wk.to_string(), cc.map(str::to_string));
            if !pair_count.contains_key(&key) {
                pair_order.push(key.clone());
            }
            *pair_count.entry(key).or_insert(0) += 1;
            if !REQUIRED_PAIRS.iter().any(|(w, c)| *w == wk && *c == cc) {
                problems.push(format!(
                    "pattern \"{id}\" carries classification pair {}, which is not one of the seven active pairs",
                    pair_label(wk, cc)
                ));
            }
        }
    }
    for (wk, cc) in &pair_order {
        let n = pair_count[&(wk.clone(), cc.clone())];
        if n <= 1 {
            continue;
        }
        problems.push(format!(
            "classification pair {} is declared {n} times; each active pair appears exactly once",
            pair_label(wk, cc.as_deref())
        ));
    }
    for (wk, cc) in REQUIRED_PAIRS {
        let key = (wk.to_string(), cc.map(str::to_string));
        if !pair_count.contains_key(&key) {
            problems.push(format!(
                "no pattern for classification pair {}; all seven are mandatory",
                pair_label(wk, cc)
            ));
        }
    }

    for p in entries {
        let id = p.get("id").and_then(Value::as_str).unwrap_or("");
        let Some(payload) = p.get("payload") else {
            continue;
        };
        for field in LINK_FIELDS {
            let Some(links) = payload.get(field) else {
                continue;
            };
            for link in links.as_array().into_iter().flatten() {
                if link.get("status").and_then(Value::as_str) != Some("present") {
                    continue;
                }
                if let Some(problem) = check_link_classification(id, field, link) {
                    problems.push(problem);
                    continue;
                }
                let path = link.get("path").and_then(Value::as_str).unwrap_or("");
                if let Some(reason) = portable_relative_path_defect(path) {
                    problems.push(format!(
                        "pattern \"{id}\" {field} link \"{path}\": {reason}"
                    ));
                } else if let Err(reason) = (ctx.check_kernel_link)(path) {
                    problems.push(format!(
                        "pattern \"{id}\" {field} link \"{path}\": {reason}"
                    ));
                }
            }
        }
    }

    let find = |wk: &str, cc: Option<&str>| -> Option<&Value> {
        entries.iter().find(|p| {
            let payload = p.get("payload");
            let pwk = payload
                .and_then(|v| v.get("work_kind"))
                .and_then(Value::as_str);
            let pcc = payload
                .and_then(|v| v.get("change_class"))
                .and_then(Value::as_str);
            pwk == Some(wk) && pcc == cc
        })
    };

    if let Some(refactor) = find("change", Some("REFACTOR")) {
        let payload = refactor.get("payload").cloned().unwrap_or(Value::Null);
        let protocols = payload
            .get("applicable_protocols")
            .cloned()
            .unwrap_or(Value::Null);
        let evidence = payload
            .get("applicable_evidence_contracts")
            .cloned()
            .unwrap_or(Value::Null);
        if !present_paths(&protocols)
            .iter()
            .any(|p| p == REFACTOR_PROTOCOL)
        {
            problems.push(format!(
                "the REFACTOR pattern does not route to the functional-parity protocol \"{REFACTOR_PROTOCOL}\""
            ));
        }
        if !present_paths(&evidence)
            .iter()
            .any(|p| p == REFACTOR_EVIDENCE)
        {
            problems.push(format!(
                "the REFACTOR pattern does not reference the functional-parity evidence contract \"{REFACTOR_EVIDENCE}\""
            ));
        }
    }

    if let Some(bugfix) = find("change", Some("BUGFIX")) {
        let payload = bugfix.get("payload").cloned().unwrap_or(Value::Null);
        let skills = payload
            .get("applicable_skills")
            .cloned()
            .unwrap_or(Value::Null);
        let bugfix_skills: HashSet<String> = present_paths(&skills).into_iter().collect();
        if !bugfix_skills.contains(BUGFIX_SKILL) {
            problems.push(format!(
                "the BUGFIX pattern does not reference the bugfix way of carrying work out \"{BUGFIX_SKILL}\" in applicable_skills"
            ));
        }
        for (label, other) in [
            ("FEATURE", find("change", Some("FEATURE"))),
            ("BEHAVIOR_CHANGE", find("change", Some("BEHAVIOR_CHANGE"))),
        ] {
            let Some(other) = other else { continue };
            let other_payload = other.get("payload").cloned().unwrap_or(Value::Null);
            let other_skills = other_payload
                .get("applicable_skills")
                .cloned()
                .unwrap_or(Value::Null);
            let other_protocols = other_payload
                .get("applicable_protocols")
                .cloned()
                .unwrap_or(Value::Null);
            let shared = present_paths(&other_skills)
                .into_iter()
                .chain(present_paths(&other_protocols))
                .find(|p| bugfix_skills.contains(p));
            if let Some(shared) = shared {
                problems.push(format!(
                    "the BUGFIX pattern shares \"{shared}\" with the {label} pattern; BUGFIX must not be conflated with FEATURE or BEHAVIOR_CHANGE"
                ));
            }
        }
    }

    if let Some(initiative) = find("initiative", None) {
        let payload = initiative.get("payload").cloned().unwrap_or(Value::Null);
        if payload.get("decomposition_required") != Some(&Value::Bool(true)) {
            problems.push("the initiative pattern does not require decomposition".to_string());
        }
        if payload.get("change_class").is_some() {
            problems.push("the initiative pattern carries a change_class".to_string());
        }
    }

    problems
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::Path;

    use super::*;
    use serde_json::json;

    #[test]
    fn portable_relative_path_defect_rejects_an_absolute_path() {
        let err = portable_relative_path_defect("/etc/passwd").unwrap();
        assert!(err.contains("absolute"));
    }

    #[test]
    fn portable_relative_path_defect_rejects_a_windows_drive_path() {
        let err = portable_relative_path_defect("C:/Users/name").unwrap();
        assert!(err.contains("absolute"));
    }

    #[test]
    fn portable_relative_path_defect_rejects_a_backslash_path() {
        let err = portable_relative_path_defect("a\\b").unwrap();
        assert!(err.contains("POSIX-relative"));
    }

    #[test]
    fn portable_relative_path_defect_rejects_a_dot_dot_segment() {
        let err = portable_relative_path_defect("a/../b").unwrap();
        assert!(err.contains("normalised"));
    }

    #[test]
    fn portable_relative_path_defect_rejects_an_empty_path() {
        let err = portable_relative_path_defect("").unwrap();
        assert!(err.contains("empty"));
    }

    #[test]
    fn portable_relative_path_defect_accepts_a_normalised_relative_path() {
        assert!(portable_relative_path_defect("skills/bugfix-protocol/SKILL.md").is_none());
    }

    #[test]
    fn is_skill_artifact_path_accepts_exactly_one_segment() {
        assert!(is_skill_artifact_path("skills/bugfix-protocol/SKILL.md"));
        assert!(!is_skill_artifact_path("skills/a/b/SKILL.md"));
        assert!(!is_skill_artifact_path("skills/SKILL.md"));
        assert!(!is_skill_artifact_path("not-skills/a/SKILL.md"));
    }

    #[test]
    fn check_rule_resolution_bugfix_consistency_flags_the_arrow_route() {
        let problems =
            check_rule_resolution_bugfix_consistency("BUGFIX → bugfix-protocol is the route");
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("still routes"));
    }

    #[test]
    fn check_rule_resolution_bugfix_consistency_flags_kernel_protocol_wording() {
        let problems = check_rule_resolution_bugfix_consistency(
            "bugfix-protocol is called протокол ядра on this line",
        );
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("same line"));
    }

    #[test]
    fn check_rule_resolution_bugfix_consistency_is_silent_on_consistent_text() {
        let problems = check_rule_resolution_bugfix_consistency(
            "BUGFIX routes to the skill bugfix-protocol, not a protocol",
        );
        assert!(problems.is_empty(), "{problems:?}");
    }

    /// This crate owns no filesystem in its production code — the whole
    /// point of `EvalContext::check_kernel_link` — but its own test code is
    /// not bound by that, exactly as `instruction_source_registry`'s and
    /// `task_specification`'s own `the_real_kernel_schema_and_fixtures_agree`
    /// tests already read real Kernel files directly. This test proves the
    /// composite algorithm end to end against the SAME real fixtures
    /// `meridian-cli`'s own `the_real_kernel_catalog_agrees_with_its_own_schema_and_fixtures`
    /// exercises, backed by a real (test-local) filesystem callback over
    /// `git ls-files`, not a stub that could never observe a fixture whose
    /// invalidity is specifically about the link's filesystem target
    /// (missing, a directory, untracked, escaping the Kernel via a
    /// symlink).
    fn real_check_kernel_link(kernel_root: &Path) -> impl Fn(&str) -> Result<(), String> + '_ {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(kernel_root)
            .args(["ls-files", "-z"])
            .output()
            .expect("git ls-files runs");
        assert!(output.status.success(), "git ls-files must succeed here");
        let text = String::from_utf8(output.stdout).expect("git ls-files output is UTF-8");
        let tracked: HashSet<String> = text
            .split('\0')
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        move |rel_path: &str| {
            let root = kernel_root
                .canonicalize()
                .map_err(|_| "the Kernel root itself cannot be resolved".to_string())?;
            let lexical = root.join(rel_path);
            let real_target = std::fs::canonicalize(&lexical)
                .map_err(|_| "the target does not exist".to_string())?;
            if !real_target.starts_with(&root) {
                return Err(
                    "the target resolves outside the Kernel once symbolic links are followed"
                        .to_string(),
                );
            }
            let metadata = std::fs::metadata(&real_target)
                .map_err(|_| "the target cannot be inspected".to_string())?;
            if !metadata.is_file() {
                return Err("the target is not a regular file".to_string());
            }
            if !tracked.contains(rel_path) {
                return Err("the target is not in the Kernel's tracked file set".to_string());
            }
            Ok(())
        }
    }

    #[test]
    fn the_real_kernel_schema_and_fixtures_agree() {
        let kernel_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let registry_schema: Value = serde_json::from_str(
            &std::fs::read_to_string(
                kernel_root.join("registries/operating-model/task-pattern-registry.schema.json"),
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
        let fx_raw = std::fs::read_to_string(
            kernel_root
                .join("registries/operating-model/fixtures/task-pattern-registry.fixtures.json"),
        )
        .unwrap();
        let bundle: Value = serde_json::from_str(&fx_raw).unwrap();
        let check_kernel_link = real_check_kernel_link(kernel_root);
        let ctx = EvalContext {
            registry_schema: &registry_schema,
            envelope_schema: &envelope_schema,
            check_kernel_link: &check_kernel_link,
        };
        for case in bundle["valid"].as_array().unwrap() {
            let problems = evaluate_task_pattern_registry(&case["registry"], &ctx);
            assert!(
                problems.is_empty(),
                "note={:?} problems={:?}",
                case.get("note"),
                problems
            );
        }
        for case in bundle["invalid"].as_array().unwrap() {
            let problems = evaluate_task_pattern_registry(&case["registry"], &ctx);
            assert!(!problems.is_empty(), "note={:?}", case.get("note"));
        }
    }

    #[test]
    fn a_duplicated_pattern_id_is_reported() {
        let kernel_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let registry_schema: Value = serde_json::from_str(
            &std::fs::read_to_string(
                kernel_root.join("registries/operating-model/task-pattern-registry.schema.json"),
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
        let fx_raw = std::fs::read_to_string(
            kernel_root
                .join("registries/operating-model/fixtures/task-pattern-registry.fixtures.json"),
        )
        .unwrap();
        let bundle: Value = serde_json::from_str(&fx_raw).unwrap();
        let mut doc = bundle["valid"][0]["registry"].clone();
        let duplicate = doc["task_patterns"][0].clone();
        doc["task_patterns"].as_array_mut().unwrap().push(duplicate);
        let check_kernel_link = |_: &str| Ok(());
        let ctx = EvalContext {
            registry_schema: &registry_schema,
            envelope_schema: &envelope_schema,
            check_kernel_link: &check_kernel_link,
        };
        let problems = evaluate_task_pattern_registry(&doc, &ctx);
        assert!(
            problems
                .iter()
                .any(|p| p.contains("declared more than once")),
            "{problems:?}"
        );
    }

    #[test]
    fn a_link_target_the_callback_rejects_is_reported_with_its_reason() {
        let registry_schema = json!({});
        let envelope_schema = json!({});
        let check_kernel_link = |_: &str| Err("the target does not exist".to_string());
        let ctx = EvalContext {
            registry_schema: &registry_schema,
            envelope_schema: &envelope_schema,
            check_kernel_link: &check_kernel_link,
        };
        let doc = json!({
            "task_patterns": [{
                "id": "example-pattern",
                "record_type": "task-pattern",
                "payload": {
                    "work_kind": "assessment",
                    "applicable_protocols": [
                        { "status": "present", "path": "some/protocol.md" }
                    ],
                },
            }],
        });
        let problems = evaluate_task_pattern_registry(&doc, &ctx);
        assert!(
            problems
                .iter()
                .any(|p| p.contains("some/protocol.md") && p.contains("does not exist")),
            "{problems:?}"
        );
    }
}
