#![cfg(test)]
//! Test-only helpers shared by the four migration/qualification families'
//! route tests: the real Kernel files, a fake reader seeded from them, and
//! the real task-pattern catalogue.

use meridian_core::task_contracts::TaskPatternCatalog;
use meridian_core::types::{Diagnostic, WorkspaceRelativePath};
use serde_json::Value;

use crate::validation::mechanical_integrity::tests::{real_fs_reader, FakeReader};
use crate::workspace::{
    GitInspector, GitInspectorError, LinkTargetError, LinkTargetPort, ReadError,
};

pub(crate) const DIR: &str = "registries/operating-model";

pub(crate) fn kernel_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("meridian-app lives inside the Kernel")
        .to_path_buf()
}

pub(crate) fn real(path: &str) -> String {
    std::fs::read_to_string(kernel_root().join(path))
        .unwrap_or_else(|e| panic!("{path} is readable: {e}"))
}

pub(crate) fn real_json(path: &str) -> Value {
    serde_json::from_str(&real(path)).unwrap_or_else(|e| panic!("{path} is JSON: {e}"))
}

pub(crate) fn schema(name: &str) -> String {
    format!("{DIR}/{name}")
}

pub(crate) fn fixtures(name: &str) -> String {
    format!("{DIR}/fixtures/{name}")
}

/// A fake reader holding the real content of every named path.
pub(crate) fn real_reader(paths: &[String]) -> FakeReader {
    paths
        .iter()
        .fold(FakeReader::new(), |r, p| r.with_file(p, &real(p)))
}

pub(crate) fn texts(diagnostics: &[Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|d| d.message().to_string())
        .collect()
}

pub(crate) fn io(message: &str) -> ReadError {
    ReadError::Io(message.to_string())
}

/// The real Kernel reader.
pub(crate) fn fs_reader() -> impl crate::workspace::WorkspaceReader {
    real_fs_reader(kernel_root())
}

/// The Kernel checkout's own tracked files (`git ls-files -z`).
struct RealGit;
impl GitInspector for RealGit {
    fn tracked_files(&self) -> Result<Vec<WorkspaceRelativePath>, GitInspectorError> {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(kernel_root())
            .args(["ls-files", "-z"])
            .output()
            .map_err(|_| GitInspectorError::Unavailable)?;
        if !output.status.success() {
            return Err(GitInspectorError::Unavailable);
        }
        let text = String::from_utf8(output.stdout).map_err(|_| GitInspectorError::Unavailable)?;
        text.split('\0')
            .filter(|s| !s.is_empty())
            .map(|raw| {
                WorkspaceRelativePath::new(raw).map_err(|e| GitInspectorError::InvalidPath {
                    raw: raw.to_string(),
                    reason: e.to_string(),
                })
            })
            .collect()
    }
}

struct EveryLinkResolves;
impl LinkTargetPort for EveryLinkResolves {
    fn check(&self, _path: &WorkspaceRelativePath) -> Result<(), LinkTargetError> {
        Ok(())
    }
}

/// The real task-pattern catalogue, built by the real
/// `task-pattern-registry` route (link targets are irrelevant to
/// which patterns it publishes).
pub(crate) fn real_task_pattern_catalog() -> TaskPatternCatalog {
    let reader = fs_reader();
    let ports = super::super::task_pattern_registry::Ports {
        reader: &reader,
        git: &RealGit,
        link_target: &EveryLinkResolves,
    };
    super::super::task_pattern_registry::evaluate(&ports)
        .expect("the real registry is readable")
        .catalog
        .expect("the real registry publishes its catalogue")
}

/// The Rust half of one matched Node/Rust container/envelope schema
/// short-circuit case (`COMPATIBILITY.md`; the Node half is
/// `test/conformance-harness.test.mjs`, block `rust-architecture-conformance-7`
/// schema short-circuit). `doc` is a real valid fixture container carrying
/// ONE independent, schema-clean domain defect, which `check` — the
/// family's real production container route — reports as exactly
/// `domain_line`. Then the same document with ONE added schema-only defect
/// the closed DTOs would accept (an empty `title`: `minLength: 1`, not a
/// domain rule) — once on the container, once on the first entry's
/// envelope: the route stops at the schema gate with exactly that schema
/// diagnostic and never reaches the domain. Node reports the schema
/// diagnostic first AND `domain_line`.
pub(crate) fn assert_schema_short_circuit<T>(
    doc: &Value,
    entries_key: &str,
    domain_line: &str,
    check: impl Fn(&Value) -> super::super::run_contract_boundary::CaseOutcome<T>,
) {
    use super::super::run_contract_boundary::CaseOutcome;
    let run = |doc: &Value| {
        let outcome = check(doc);
        let schema = matches!(outcome, CaseOutcome::SchemaRejected(_));
        let domain = matches!(outcome, CaseOutcome::DomainRejected(_));
        (schema, domain, texts(&super::outcome_diagnostics(&outcome)))
    };
    assert_eq!(run(doc), (false, true, vec![domain_line.to_string()]));

    let mut container = doc.clone();
    container["title"] = Value::from("");
    assert_eq!(
        run(&container),
        (true, false, vec!["/title: shorter than 1".to_string()])
    );

    let mut envelope = doc.clone();
    envelope[entries_key][0]["title"] = Value::from("");
    assert_eq!(
        run(&envelope),
        (
            true,
            false,
            vec!["entry 0 envelope /title: shorter than 1".to_string()]
        )
    );
}
