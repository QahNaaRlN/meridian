//! Port of `scripts/kernel-validate.mjs`'s `stack-profiles` check — the pool
//! half only (`stack-profiles/stack-profiles.{yaml,md}`): names and
//! signatures agree, and `"universal"` (the declared absence of a profile)
//! never appears in the pool itself. The per-repository declaration half
//! (a repository's manifest checked against a declared profile's
//! `requires`/`forbids`) is a separate, still-blocked family
//! (`BLOCKED_CHECKS`'s `stack-profile: no Instance root, declarations were
//! not checked` advisory, unchanged by this module — no `--instance` flag
//! is wired by this package).

use std::fs;
use std::path::Path;

use fancy_regex::Regex;
use meridian_app::source_format::{parse_yaml, regions::marked_region};
use serde_json::Value;

pub struct Outcome {
    pub failures: Vec<String>,
    /// Mirrors the Node reference's `STACK_PROFILES` truthiness: `true` only
    /// when the pool file pair parsed and its two halves agreed, so the
    /// caller can gate the "no Instance root, declarations were not
    /// checked" advisory the same way the Node reference does (only reached
    /// when `STACK_PROFILES` is non-null — a pool that failed to load is
    /// not silently promoted to "declarations were not checked").
    pub pool_loaded: bool,
}

pub fn run(kernel_root: &Path) -> Outcome {
    let mut failures = Vec::new();

    let yaml_raw = fs::read_to_string(kernel_root.join("stack-profiles/stack-profiles.yaml")).ok();
    let md_raw = fs::read_to_string(kernel_root.join("stack-profiles/stack-profiles.md")).ok();

    if yaml_raw.is_none() || md_raw.is_none() {
        let mut missing = Vec::new();
        if yaml_raw.is_none() {
            missing.push("stack-profiles/stack-profiles.yaml");
        }
        if md_raw.is_none() {
            missing.push("stack-profiles/stack-profiles.md");
        }
        failures.push(format!(
            "stack-profiles: the profile pool is missing from the Kernel ({}); no declaration could be checked against it, and an unreadable pool is a defect of the Kernel, not a reason to skip the check",
            missing.join(", ")
        ));
        return Outcome {
            failures,
            pool_loaded: false,
        };
    }
    let yaml_raw = yaml_raw.unwrap();
    let md_raw = md_raw.unwrap();

    let (entries, parsed) = match parse_yaml(&yaml_raw) {
        Ok(doc) => match doc.get("profiles") {
            // Absent, or a JS-falsy scalar in the Node reference's own
            // `yamlParse(...)?.profiles || []` — both fall back to an
            // empty, legal pool there.
            None | Some(Value::Null) | Some(Value::Bool(false)) => (Vec::new(), true),
            Some(Value::Number(n)) if n.as_f64() == Some(0.0) => (Vec::new(), true),
            Some(Value::String(s)) if s.is_empty() => (Vec::new(), true),
            Some(Value::Array(arr)) => (arr.clone(), true),
            // Any other type is JS-truthy and not an array. The Node
            // reference's own assignment does not fail here (`|| []` never
            // rejects a truthy value), but the code that follows calls
            // `entries.map(...)` on it outside any try/catch, which throws
            // uncaught for a non-array. Never following that path into a
            // crash, this port reports the same defect as an explicit
            // failure instead of silently treating it as an empty, and
            // therefore legal, pool.
            Some(other) => {
                failures.push(format!(
                    "stack-profiles: \"profiles\" must be a list, found {}",
                    type_name(other)
                ));
                (Vec::new(), false)
            }
        },
        Err(error) => {
            failures.push(format!("stack-profiles: {error}"));
            (Vec::new(), false)
        }
    };

    let region = match marked_region(&md_raw, "stack-profile-pool") {
        Ok(region) => region,
        Err(error) => {
            failures.push(format!(
                "stack-profiles: the pool region of stack-profiles.md is not readable — {error}; the signatures the gate compares against are the ones inside the markers, and nothing else"
            ));
            return Outcome {
                failures,
                pool_loaded: false,
            };
        }
    };

    if !parsed {
        return Outcome {
            failures,
            pool_loaded: false,
        };
    }

    let row_re = Regex::new(r"(?m)^\|\s*`([a-z0-9-]+)`\s*\|").expect("row pattern compiles");
    // Node reads both halves into a `Set`, iterated in first-insertion order
    // (document row order / YAML declaration order) — never alphabetically,
    // which a `BTreeSet` would silently impose on the diagnostic text below.
    let documented_ordered: Vec<String> = dedupe_preserve_order(
        row_re
            .captures_iter(&region.text)
            .filter_map(|c| c.ok())
            .filter_map(|c| c.get(1).map(|g| g.as_str().to_string())),
    );
    let declared_ordered: Vec<String> = dedupe_preserve_order(entries.iter().map(|e| {
        e.get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    }));
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
            parts.push(format!(
                "named in the data but carrying no signature: {}",
                undocumented
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !unlisted.is_empty() {
            parts.push(format!(
                "carrying a signature but absent from the data: {}",
                unlisted
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        failures.push(format!(
            "stack-profiles: the two halves of the pool disagree — {}",
            parts.join("; ")
        ));
        return Outcome {
            failures,
            pool_loaded: false,
        };
    } else if declared_set.contains("universal") {
        failures.push(
            "stack-profiles: \"universal\" is not a stack profile and must not be listed in the pool; it is the declared absence of one"
                .to_string(),
        );
        return Outcome {
            failures,
            pool_loaded: false,
        };
    }

    Outcome {
        failures,
        pool_loaded: true,
    }
}

/// The JS "typeof"-adjacent label the Node reference's uncaught `TypeError`
/// (thrown by `entries.map(...)` on a non-array `profiles`) implicitly names
/// — used only for this port's own explicit failure message.
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
                "stack-profiles-{label}-{}-{}",
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
    fn agreeing_halves_produce_no_failures() {
        let dir = temp("agree");
        write(
            &dir,
            "stack-profiles/stack-profiles.yaml",
            "profiles:\n  - name: vue-spa\n",
        );
        write(
            &dir,
            "stack-profiles/stack-profiles.md",
            "<!-- meridian:begin stack-profile-pool -->\n| `vue-spa` | x | y | z |\n<!-- meridian:end stack-profile-pool -->\n",
        );
        let outcome = run(&dir);
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    }

    #[test]
    fn universal_in_the_pool_is_rejected() {
        let dir = temp("universal");
        write(
            &dir,
            "stack-profiles/stack-profiles.yaml",
            "profiles:\n  - name: universal\n",
        );
        write(
            &dir,
            "stack-profiles/stack-profiles.md",
            "<!-- meridian:begin stack-profile-pool -->\n| `universal` | x | y | z |\n<!-- meridian:end stack-profile-pool -->\n",
        );
        let outcome = run(&dir);
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].contains("is not a stack profile"));
    }

    #[test]
    fn a_disagreeing_pool_is_a_failure() {
        let dir = temp("disagree");
        write(
            &dir,
            "stack-profiles/stack-profiles.yaml",
            "profiles:\n  - name: vue-spa\n  - name: ghost-profile\n",
        );
        write(
            &dir,
            "stack-profiles/stack-profiles.md",
            "<!-- meridian:begin stack-profile-pool -->\n| `vue-spa` | x | y | z |\n<!-- meridian:end stack-profile-pool -->\n",
        );
        let outcome = run(&dir);
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].contains("disagree"));
    }

    #[test]
    fn a_missing_pool_file_fails_closed() {
        let dir = temp("missing");
        fs::create_dir_all(&dir).unwrap();
        let outcome = run(&dir);
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].contains("is missing from the Kernel"));
    }

    #[test]
    fn a_profiles_key_of_the_wrong_type_is_a_failure_not_an_empty_legal_pool() {
        let dir = temp("wrong-type");
        write(
            &dir,
            "stack-profiles/stack-profiles.yaml",
            "profiles: not-a-list\n",
        );
        write(
            &dir,
            "stack-profiles/stack-profiles.md",
            "<!-- meridian:begin stack-profile-pool -->\n<!-- meridian:end stack-profile-pool -->\n",
        );
        let outcome = run(&dir);
        assert!(
            outcome
                .failures
                .iter()
                .any(|f| f.contains("must be a list")),
            "{:?}",
            outcome.failures
        );
        assert!(!outcome.pool_loaded);
    }

    #[test]
    fn a_profiles_key_that_is_an_empty_string_is_a_legal_empty_pool() {
        let dir = temp("falsy-empty-string");
        write(
            &dir,
            "stack-profiles/stack-profiles.yaml",
            "profiles: \"\"\n",
        );
        write(
            &dir,
            "stack-profiles/stack-profiles.md",
            "<!-- meridian:begin stack-profile-pool -->\n<!-- meridian:end stack-profile-pool -->\n",
        );
        let outcome = run(&dir);
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
        assert!(outcome.pool_loaded);
    }

    #[test]
    fn disagreement_diagnostics_list_names_in_declaration_order_not_alphabetically() {
        let dir = temp("insertion-order");
        write(
            &dir,
            "stack-profiles/stack-profiles.yaml",
            "profiles:\n  - name: zeta-profile\n  - name: alpha-profile\n",
        );
        write(
            &dir,
            "stack-profiles/stack-profiles.md",
            "<!-- meridian:begin stack-profile-pool -->\n<!-- meridian:end stack-profile-pool -->\n",
        );
        let outcome = run(&dir);
        assert_eq!(outcome.failures.len(), 1);
        let zeta_pos = outcome.failures[0].find("zeta-profile").unwrap();
        let alpha_pos = outcome.failures[0].find("alpha-profile").unwrap();
        assert!(
            zeta_pos < alpha_pos,
            "expected declaration order (zeta before alpha), got: {}",
            outcome.failures[0]
        );
    }

    #[test]
    fn the_real_kernel_pool_agrees_with_itself() {
        let kernel_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let outcome = run(kernel_root);
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    }
}
