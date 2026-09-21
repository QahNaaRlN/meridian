//! Port of `scripts/kernel-validate.mjs`'s `instruction-topics` check
//! (`standards/workspace/instruction-topics.{yaml,md}`). The topic pool
//! lives in two files on purpose: names where the gate can read them,
//! signatures where a person can compare them. Two files holding one pool
//! are two pools unless something checks they agree, so that is checked
//! first and the pool is refused outright when they do not.
//!
//! Reuses the file-independent marked-region adapter
//! (`meridian_app::source_format::regions::marked_region`) and the strict
//! YAML adapter (`meridian_app::source_format::parse_yaml`); does not
//! reimplement either.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use fancy_regex::Regex;
use meridian_app::source_format::{parse_yaml, regions::marked_region};
use serde_json::Value;

pub struct Outcome {
    pub failures: Vec<String>,
    /// The declared topic pool, present only when both halves parsed and
    /// agreed — consumed by `agent-instruction-identity`, which must not
    /// reject a topic against a pool this check itself already rejected.
    pub topic_pool: Option<BTreeSet<String>>,
}

pub fn run(kernel_root: &Path) -> Outcome {
    let mut failures = Vec::new();

    let yaml_path = kernel_root.join("standards/workspace/instruction-topics.yaml");
    let md_path = kernel_root.join("standards/workspace/instruction-topics.md");
    let yaml_raw = fs::read_to_string(&yaml_path).ok();
    let md_raw = fs::read_to_string(&md_path).ok();

    if yaml_raw.is_none() || md_raw.is_none() {
        let mut missing = Vec::new();
        if yaml_raw.is_none() {
            missing.push("standards/workspace/instruction-topics.yaml");
        }
        if md_raw.is_none() {
            missing.push("standards/workspace/instruction-topics.md");
        }
        failures.push(format!(
            "instruction-topics: the topic pool is missing from the Kernel ({}); no topic in any register could be checked against it, and an unreadable pool is a defect of the Kernel, not a reason to skip the check",
            missing.join(", ")
        ));
        return Outcome {
            failures,
            topic_pool: None,
        };
    }
    let yaml_raw = yaml_raw.unwrap();
    let md_raw = md_raw.unwrap();

    let names: Vec<String> = match parse_yaml(&yaml_raw) {
        Ok(doc) => match doc.get("topics") {
            // Absent, or a JS-falsy scalar in the Node reference's own
            // `(yamlParse(...)?.topics || []).map(String)` — both fall back
            // to an empty, legal pool there.
            None | Some(Value::Null) | Some(Value::Bool(false)) => Vec::new(),
            Some(Value::Number(n)) if n.as_f64() == Some(0.0) => Vec::new(),
            Some(Value::String(s)) if s.is_empty() => Vec::new(),
            Some(Value::Array(arr)) => arr
                .iter()
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .collect(),
            // Any other type is JS-truthy and not an array: the Node
            // reference's `.map(String)` then throws on it (no `.map`
            // method on a string/number/object/`true`), which is caught and
            // reported as a failure, never silently treated as an empty —
            // and therefore legal — pool.
            Some(other) => {
                failures.push(format!(
                    "instruction-topics: \"topics\" must be a list, found {}",
                    type_name(other)
                ));
                Vec::new()
            }
        },
        Err(error) => {
            failures.push(format!("instruction-topics: {error}"));
            Vec::new()
        }
    };

    let region = match marked_region(&md_raw, "topic-pool") {
        Ok(region) => region,
        Err(error) => {
            failures.push(format!(
                "instruction-topics: the pool region of instruction-topics.md is not readable — {error}; the signatures the gate compares against are the ones inside the markers, and nothing else"
            ));
            return Outcome {
                failures,
                topic_pool: None,
            };
        }
    };

    let row_re = Regex::new(r"(?m)^\|\s*`([a-z0-9-]+)`\s*\|").expect("row pattern compiles");
    // Node reads both halves into a `Set`, which iterates in first-insertion
    // order — the document's row order and the YAML's declaration order,
    // respectively — never alphabetically. The diagnostic text below must
    // read in that same order; a `BTreeSet` would silently re-sort it.
    let documented_ordered: Vec<String> = dedupe_preserve_order(
        row_re
            .captures_iter(&region.text)
            .filter_map(|c| c.ok())
            .filter_map(|c| c.get(1).map(|g| g.as_str().to_string())),
    );
    let declared_ordered: Vec<String> = dedupe_preserve_order(names);
    let documented_set: std::collections::HashSet<&str> =
        documented_ordered.iter().map(|s| s.as_str()).collect();
    let declared_set: std::collections::HashSet<&str> =
        declared_ordered.iter().map(|s| s.as_str()).collect();

    let undocumented: Vec<&String> = declared_ordered
        .iter()
        .filter(|n| !documented_set.contains(n.as_str()))
        .collect();
    let unlisted: Vec<&String> = documented_ordered
        .iter()
        .filter(|n| !declared_set.contains(n.as_str()))
        .collect();

    if !undocumented.is_empty() || !unlisted.is_empty() {
        let mut parts = Vec::new();
        if !undocumented.is_empty() {
            let names: Vec<&str> = undocumented.iter().map(|s| s.as_str()).collect();
            parts.push(format!(
                "named in the data but carrying no signature: {}",
                names.join(", ")
            ));
        }
        if !unlisted.is_empty() {
            let names: Vec<&str> = unlisted.iter().map(|s| s.as_str()).collect();
            parts.push(format!(
                "carrying a signature but absent from the data: {}",
                names.join(", ")
            ));
        }
        failures.push(format!(
            "instruction-topics: the two halves of the pool disagree — {}",
            parts.join("; ")
        ));
        return Outcome {
            failures,
            topic_pool: None,
        };
    }

    Outcome {
        failures,
        topic_pool: Some(declared_ordered.into_iter().collect()),
    }
}

/// The JS "typeof"-adjacent label the Node reference's uncaught `TypeError`
/// implicitly names for a non-array `topics` value — used only for this
/// port's own explicit failure message, since Rust does not throw here.
fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// A `Set`'s iteration order in the Node reference: each item kept once, in
/// the order it was first seen. Membership can be (and is) checked through a
/// separate `HashSet` built from this — that structure's own iteration
/// order is never used for anything a person or a diagnostic reads.
fn dedupe_preserve_order(items: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for item in items {
        if seen.insert(item.clone()) {
            out.push(item);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, rel: &str, content: &str) {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    /// RAII guard: removes its exact temp directory on `Drop`, including
    /// while unwinding a panicking assertion, so a failing test never
    /// leaves its scratch tree behind. The name mixes the PID with a
    /// nanosecond timestamp — the PID alone is a shared, predictable
    /// directory that a second run (or a parallel `cargo test` thread) can
    /// collide on; the timestamp makes each call's directory unique.
    struct TestDir(std::path::PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "instruction-topics-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            TestDir(dir)
        }
    }

    impl std::ops::Deref for TestDir {
        type Target = Path;
        fn deref(&self) -> &Path {
            &self.0
        }
    }

    impl AsRef<Path> for TestDir {
        fn as_ref(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn temp(name: &str) -> TestDir {
        TestDir::new(name)
    }

    #[test]
    fn agreeing_halves_produce_the_declared_pool_and_no_failures() {
        let dir = temp("agree");
        write(
            &dir,
            "standards/workspace/instruction-topics.yaml",
            "topics:\n  - agent-conduct\n  - test-planning\n",
        );
        write(
            &dir,
            "standards/workspace/instruction-topics.md",
            "before\n<!-- meridian:begin topic-pool -->\n| `agent-conduct` | x |\n| `test-planning` | y |\n<!-- meridian:end topic-pool -->\nafter\n",
        );
        let outcome = run(&dir);
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
        let pool = outcome.topic_pool.expect("pool present");
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn a_topic_named_only_in_data_is_a_failure() {
        let dir = temp("undocumented");
        write(
            &dir,
            "standards/workspace/instruction-topics.yaml",
            "topics:\n  - agent-conduct\n  - ghost-topic\n",
        );
        write(
            &dir,
            "standards/workspace/instruction-topics.md",
            "<!-- meridian:begin topic-pool -->\n| `agent-conduct` | x |\n<!-- meridian:end topic-pool -->\n",
        );
        let outcome = run(&dir);
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].contains("carrying no signature"));
        assert!(outcome.topic_pool.is_none());
    }

    #[test]
    fn a_topic_documented_but_absent_from_data_is_a_failure() {
        let dir = temp("unlisted");
        write(
            &dir,
            "standards/workspace/instruction-topics.yaml",
            "topics:\n  - agent-conduct\n",
        );
        write(
            &dir,
            "standards/workspace/instruction-topics.md",
            "<!-- meridian:begin topic-pool -->\n| `agent-conduct` | x |\n| `ghost-topic` | y |\n<!-- meridian:end topic-pool -->\n",
        );
        let outcome = run(&dir);
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].contains("absent from the data"));
    }

    #[test]
    fn a_topics_key_of_the_wrong_type_is_a_failure_not_an_empty_legal_pool() {
        let dir = temp("wrong-type");
        write(
            &dir,
            "standards/workspace/instruction-topics.yaml",
            "topics: not-a-list\n",
        );
        write(
            &dir,
            "standards/workspace/instruction-topics.md",
            "<!-- meridian:begin topic-pool -->\n<!-- meridian:end topic-pool -->\n",
        );
        let outcome = run(&dir);
        // The Node reference's `.map(String)` throws on this value (no
        // `.map` on a non-array), which is caught and reported as a failure
        // — the gate is red regardless of what TOPIC_POOL ends up holding,
        // exactly as it is here.
        assert!(
            outcome
                .failures
                .iter()
                .any(|f| f.contains("must be a list")),
            "{:?}",
            outcome.failures
        );
    }

    #[test]
    fn a_topics_key_that_is_an_empty_string_is_a_legal_empty_pool() {
        let dir = temp("falsy-empty-string");
        write(
            &dir,
            "standards/workspace/instruction-topics.yaml",
            "topics: \"\"\n",
        );
        write(
            &dir,
            "standards/workspace/instruction-topics.md",
            "<!-- meridian:begin topic-pool -->\n<!-- meridian:end topic-pool -->\n",
        );
        let outcome = run(&dir);
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
        assert_eq!(outcome.topic_pool.expect("pool present").len(), 0);
    }

    #[test]
    fn disagreement_diagnostics_list_names_in_declaration_order_not_alphabetically() {
        let dir = temp("insertion-order");
        write(
            &dir,
            "standards/workspace/instruction-topics.yaml",
            "topics:\n  - zeta-topic\n  - alpha-topic\n",
        );
        write(
            &dir,
            "standards/workspace/instruction-topics.md",
            "<!-- meridian:begin topic-pool -->\n<!-- meridian:end topic-pool -->\n",
        );
        let outcome = run(&dir);
        assert_eq!(outcome.failures.len(), 1);
        let zeta_pos = outcome.failures[0].find("zeta-topic").unwrap();
        let alpha_pos = outcome.failures[0].find("alpha-topic").unwrap();
        assert!(
            zeta_pos < alpha_pos,
            "expected declaration order (zeta before alpha), got: {}",
            outcome.failures[0]
        );
    }

    #[test]
    fn a_missing_pool_file_fails_closed() {
        let dir = temp("missing");
        fs::create_dir_all(&dir).unwrap();
        let outcome = run(&dir);
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].contains("is missing from the Kernel"));
        assert!(outcome.topic_pool.is_none());
    }

    #[test]
    fn an_unreadable_region_is_a_failure() {
        let dir = temp("no-region");
        write(
            &dir,
            "standards/workspace/instruction-topics.yaml",
            "topics:\n  - agent-conduct\n",
        );
        write(
            &dir,
            "standards/workspace/instruction-topics.md",
            "no markers here\n",
        );
        let outcome = run(&dir);
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].contains("is not readable"));
    }

    #[test]
    fn the_real_kernel_pool_agrees_with_itself() {
        let kernel_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let outcome = run(kernel_root);
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
        assert!(outcome.topic_pool.is_some());
    }
}
