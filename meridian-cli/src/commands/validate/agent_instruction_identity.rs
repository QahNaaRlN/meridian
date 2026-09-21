//! Port of `scripts/kernel-validate.mjs`'s `agent-instruction-identity`
//! check (`standards/workspace/agent-instruction-identity.md`): §7 applied
//! to the Kernel's own documents. Which Kernel documents are agent
//! instruction norms is declared, not derived: a document says so by
//! carrying `delivery`.
//!
//! Reuses `document_identity::carries_own_front_matter` (the same rule
//! decides which Markdown files are read for Front Matter in both checks)
//! and, when it parsed clean, `instruction_topics`'s declared topic pool —
//! a topic this check rejects must never be one `instruction-topics`
//! itself already rejected the pool over.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use fancy_regex::Regex;

use super::document_identity::carries_own_front_matter;

const NORM_DELIVERY: &[&str] = &[
    "kernel-doc",
    "skill-package",
    "cursor-rule",
    "agents-md-section",
];
const NORM_ACTIVATION: &[&str] = &["always", "path-glob", "task-class", "explicit"];
const NORM_FIELDS: &[&str] = &["topic", "profile", "delivery", "activation"];
const PRESCRIPTIVE_TYPES: &[&str] = &["standard", "contract", "protocol"];

pub struct Outcome {
    pub failures: Vec<String>,
    pub declared_norms: usize,
    pub undeclared_prescriptive: usize,
    pub undeclared_other: usize,
}

pub fn run(
    kernel_root: &Path,
    markdown_files: &[PathBuf],
    topic_pool: Option<&BTreeSet<String>>,
) -> Outcome {
    let mut failures = Vec::new();
    let mut declared_norms = 0usize;
    let mut undeclared_prescriptive = 0usize;
    let mut undeclared_other = 0usize;

    let field_re: Vec<(&str, Regex)> = NORM_FIELDS
        .iter()
        .map(|f| {
            (
                *f,
                Regex::new(&format!(r"(?m)^{f}:[^\S\r\n]*\S")).expect("field pattern compiles"),
            )
        })
        .collect();
    let declares_parent_re =
        Regex::new(r"(?m)^derived_from:").expect("declares-parent pattern compiles");
    let document_type_re =
        Regex::new(r"(?m)^document_type:\s*(\S+)").expect("document-type pattern compiles");
    let delivery_re = Regex::new(r"(?m)^delivery:\s*(\S+)").expect("delivery pattern compiles");
    let activation_re =
        Regex::new(r"(?m)^activation:\s*(\S+)").expect("activation pattern compiles");
    let topic_re = Regex::new(r"(?m)^topic:\s*(\S+)").expect("topic pattern compiles");
    let unclassified_reason_re = Regex::new(r"(?m)^unclassified_reason:[^\S\r\n]*\S")
        .expect("unclassified-reason pattern compiles");
    let derived_from_scalar_re = Regex::new(r"(?m)^derived_from:[^\S\r\n]*\S")
        .expect("derived-from-scalar pattern compiles");
    let narrowing_re = Regex::new(r"(?m)^narrowing:").expect("narrowing pattern compiles");

    let mut rels: Vec<String> = markdown_files
        .iter()
        .map(|f| relative_slash(kernel_root, f))
        .filter(|rel| carries_own_front_matter(rel))
        .collect();
    rels.sort();
    rels.dedup();

    for rel in &rels {
        let Ok(text) = fs::read_to_string(kernel_root.join(rel)) else {
            continue;
        };
        if !text.starts_with("---") {
            continue;
        }
        let end = text[3..].find("\n---").map(|i| i + 3);
        let fm = match end {
            Some(end) => &text[4..end],
            None => "",
        };

        let present: Vec<&str> = field_re
            .iter()
            .filter(|(_, re)| matches(re, fm))
            .map(|(f, _)| *f)
            .collect();
        let declares_parent = matches(&declares_parent_re, fm);

        if present.is_empty() && !declares_parent {
            let doc_type = document_type_re
                .captures(fm)
                .ok()
                .flatten()
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_string());
            match doc_type {
                Some(t) if PRESCRIPTIVE_TYPES.contains(&t.as_str()) => undeclared_prescriptive += 1,
                _ => undeclared_other += 1,
            }
            continue;
        }

        declared_norms += 1;
        let absent: Vec<&str> = NORM_FIELDS
            .iter()
            .filter(|f| !present.contains(f))
            .cloned()
            .collect();
        if !absent.is_empty() {
            failures.push(format!(
                "agent-instruction-identity: {rel} declares {} but not {}; §7 is four independent answers, and a half-declared norm is exactly the drift this standard was written against",
                present.join(", "),
                absent.join(", ")
            ));
        }

        if let Some(delivery) = capture1(&delivery_re, fm) {
            if !NORM_DELIVERY.contains(&delivery.as_str()) {
                failures.push(format!(
                    "agent-instruction-identity: {rel} declares delivery \"{delivery}\", which is not in the pool of §5"
                ));
            }
        }
        if let Some(activation) = capture1(&activation_re, fm) {
            if !NORM_ACTIVATION.contains(&activation.as_str()) {
                failures.push(format!(
                    "agent-instruction-identity: {rel} declares activation \"{activation}\", which is not in the pool of §5"
                ));
            }
        }
        if let Some(topic) = capture1(&topic_re, fm) {
            if topic == "unclassified" {
                if !matches(&unclassified_reason_re, fm) {
                    failures.push(format!(
                        "agent-instruction-identity: {rel} carries topic \"unclassified\" with no unclassified_reason; the state is legal, an unexplained one is not"
                    ));
                }
            } else if let Some(pool) = topic_pool {
                if !pool.contains(&topic) {
                    failures.push(format!(
                        "agent-instruction-identity: {rel} names topic \"{topic}\", which is not in the pool; a new topic is a change to the registry, not to one document"
                    ));
                }
            }
        }

        if matches(&derived_from_scalar_re, fm) {
            failures.push(format!(
                "agent-instruction-identity: {rel} records derived_from as a scalar; the parent reference is a mapping of repository, path and revision"
            ));
        } else if declares_parent && !matches(&narrowing_re, fm) {
            failures.push(format!(
                "agent-instruction-identity: {rel} declares derived_from with no narrowing list; an edition that does not say what it removed is not distinguishable from a text written independently"
            ));
        }
    }

    Outcome {
        failures,
        declared_norms,
        undeclared_prescriptive,
        undeclared_other,
    }
}

fn capture1(re: &Regex, text: &str) -> Option<String> {
    re.captures(text)
        .ok()
        .flatten()
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn matches(re: &Regex, text: &str) -> bool {
    re.is_match(text).unwrap_or(false)
}

fn relative_slash(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, rel: &str, content: &str) -> PathBuf {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
        path
    }

    fn temp(name: &str) -> PathBuf {
        let temp = std::env::temp_dir().join(format!(
            "agent-instruction-identity-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&temp);
        temp
    }

    #[test]
    fn a_fully_declared_norm_is_clean() {
        let dir = temp("clean");
        let file = write(
            &dir,
            "doc.md",
            "---\ntopic: agent-conduct\nprofile: universal\ndelivery: kernel-doc\nactivation: always\n---\nbody\n",
        );
        let pool: BTreeSet<String> = ["agent-conduct".to_string()].into_iter().collect();
        let outcome = run(&dir, &[file], Some(&pool));
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
        assert_eq!(outcome.declared_norms, 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_half_declared_norm_is_a_failure() {
        let dir = temp("half");
        let file = write(&dir, "doc.md", "---\ntopic: agent-conduct\n---\nbody\n");
        let pool: BTreeSet<String> = ["agent-conduct".to_string()].into_iter().collect();
        let outcome = run(&dir, &[file], Some(&pool));
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].contains("but not"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_document_declaring_nothing_of_seven_is_counted_not_failed() {
        let dir = temp("undeclared");
        let file = write(&dir, "doc.md", "---\ndocument_type: standard\n---\nbody\n");
        let outcome = run(&dir, &[file], None);
        assert!(outcome.failures.is_empty());
        assert_eq!(outcome.undeclared_prescriptive, 1);
        assert_eq!(outcome.undeclared_other, 0);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unknown_delivery_value_is_a_failure() {
        let dir = temp("bad-delivery");
        let file = write(
            &dir,
            "doc.md",
            "---\ntopic: agent-conduct\nprofile: universal\ndelivery: made-up\nactivation: always\n---\nbody\n",
        );
        let pool: BTreeSet<String> = ["agent-conduct".to_string()].into_iter().collect();
        let outcome = run(&dir, &[file], Some(&pool));
        assert!(outcome
            .failures
            .iter()
            .any(|f| f.contains("not in the pool of §5")));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn unclassified_without_a_reason_is_a_failure() {
        let dir = temp("unclassified");
        let file = write(
            &dir,
            "doc.md",
            "---\ntopic: unclassified\nprofile: universal\ndelivery: kernel-doc\nactivation: always\n---\nbody\n",
        );
        let outcome = run(&dir, &[file], None);
        assert!(outcome
            .failures
            .iter()
            .any(|f| f.contains("unclassified_reason")));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_topic_absent_from_the_pool_is_a_failure() {
        let dir = temp("bad-topic");
        let file = write(
            &dir,
            "doc.md",
            "---\ntopic: made-up-topic\nprofile: universal\ndelivery: kernel-doc\nactivation: always\n---\nbody\n",
        );
        let pool: BTreeSet<String> = ["agent-conduct".to_string()].into_iter().collect();
        let outcome = run(&dir, &[file], Some(&pool));
        assert!(outcome
            .failures
            .iter()
            .any(|f| f.contains("not in the pool; a new topic")));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_scalar_derived_from_is_a_failure() {
        let dir = temp("scalar-parent");
        let file = write(
            &dir,
            "doc.md",
            "---\ntopic: agent-conduct\nprofile: universal\ndelivery: kernel-doc\nactivation: always\nderived_from: some-path.md\n---\nbody\n",
        );
        let pool: BTreeSet<String> = ["agent-conduct".to_string()].into_iter().collect();
        let outcome = run(&dir, &[file], Some(&pool));
        assert!(outcome.failures.iter().any(|f| f.contains("as a scalar")));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_declared_parent_with_no_narrowing_is_a_failure() {
        let dir = temp("no-narrowing");
        let file = write(
            &dir,
            "doc.md",
            "---\ntopic: agent-conduct\nprofile: universal\ndelivery: kernel-doc\nactivation: always\nderived_from:\n  repository: kernel\n  path: x.md\n  revision: abc\n---\nbody\n",
        );
        let pool: BTreeSet<String> = ["agent-conduct".to_string()].into_iter().collect();
        let outcome = run(&dir, &[file], Some(&pool));
        assert!(outcome
            .failures
            .iter()
            .any(|f| f.contains("no narrowing list")));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn files_the_document_identity_rule_excludes_are_skipped() {
        let dir = temp("excluded");
        let file = write(
            &dir,
            "instance-template/skipped.md",
            "---\ntopic: agent-conduct\n---\n",
        );
        let outcome = run(&dir, &[file], None);
        assert!(outcome.failures.is_empty());
        assert_eq!(outcome.declared_norms, 0);
        assert_eq!(outcome.undeclared_other, 0);
        let _ = fs::remove_dir_all(&dir);
    }
}
