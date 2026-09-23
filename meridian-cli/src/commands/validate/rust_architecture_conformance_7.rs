#![cfg(test)]
//! Structural and route gates of `rust-architecture-conformance-7`
//! (`governance/plans/meridian-rust-migration-program-plan.md` §5.21): the
//! four migration/qualification command modules are thin
//! composition/presentation shims; the migration and qualification core
//! carries no transport or I/O; the app never names the concrete adapter;
//! the retired `Value` entrypoints and the blocker mechanism are gone; each
//! family's prefix is applied exactly once; `NotFound` and `Io` stay
//! distinct through the real adapter; and the real Kernel is clean.

use std::path::{Path, PathBuf};

const MODULES: [(&str, &str); 4] = [
    ("instance_data_migration.rs", "instance-data-migration"),
    ("instance_canonical_export.rs", "instance-canonical-export"),
    (
        "workspace_compatibility_qualification.rs",
        "workspace-compatibility-qualification",
    ),
    (
        "upgrade_integration_qualification.rs",
        "upgrade-integration-qualification",
    ),
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("meridian-cli lives inside the Kernel")
        .to_path_buf()
}

/// The production part of a Rust file: nothing for an explicit test-only
/// module file, else everything before its first `#[cfg(test)]`, minus
/// comment lines.
fn production_text(path: &Path) -> String {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{} is readable: {e}", path.display()));
    let first_code_line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("//"));
    if first_code_line == Some("#![cfg(test)]") {
        return String::new();
    }
    let end = text.find("#[cfg(test)]").unwrap_or(text.len());
    text[..end]
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn sources(dir: &str) -> Vec<(String, String)> {
    let root = workspace_root();
    let mut files = Vec::new();
    rust_files(&root.join(dir), &mut files);
    files
        .into_iter()
        .map(|p| {
            let rel = p
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            (rel, production_text(&p))
        })
        .collect()
}

#[test]
fn migration_and_qualification_command_modules_are_thin_shims() {
    for (file, _) in MODULES {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/commands/validate")
            .join(file);
        let production = production_text(&path);
        for forbidden in [
            "std::fs",
            "std::process",
            "serde_json",
            "Value",
            "json_schema",
            "fixtures",
            "resolution",
            "evaluate_",
            "check_",
            "unwrap",
            "expect(",
        ] {
            assert!(
                !production.contains(forbidden),
                "{file} contains \"{forbidden}\" — it must stay a thin composition/presentation shim"
            );
        }
        for required in [
            "app::evaluate(",
            "FsWorkspaceReader::new(",
            "with_family_prefix(",
        ] {
            assert!(
                production.contains(required),
                "{file} must call `{required}`"
            );
        }
        // Only the upgrade qualification names a core type: the task
        // pattern catalogue it is handed, never a domain rule.
        let core_uses: Vec<&str> = production
            .lines()
            .filter(|l| l.contains("meridian_core"))
            .collect();
        assert!(
            core_uses.is_empty()
                || core_uses == ["use meridian_core::task_contracts::TaskPatternCatalog;"],
            "{file}: {core_uses:?}"
        );
    }
}

/// The migration and qualification core owners carry no serde, no JSON
/// transport value and no I/O.
#[test]
fn migration_and_qualification_core_is_pure() {
    for (rel, text) in sources("meridian-core/src").into_iter().filter(|(rel, _)| {
        rel.contains("/migration/")
            || rel.contains("/qualification/")
            || rel.ends_with("/canonical.rs")
    }) {
        for forbidden in [
            "serde",
            "&Value",
            "<Value>",
            ": Value",
            "std::fs",
            "std::io",
            "std::env",
            "std::process",
            "println!",
            "HashMap",
            "HashSet",
        ] {
            assert!(!text.contains(forbidden), "{rel} contains \"{forbidden}\"");
        }
    }
}

/// The four app routes and their shared boundary never name the concrete
/// adapter or spawn processes, and never iterate a hash collection.
#[test]
fn migration_and_qualification_app_routes_are_adapter_free() {
    for (rel, text) in sources("meridian-app/src").into_iter().filter(|(rel, _)| {
        rel.contains("/instance_data_migration/")
            || rel.contains("/instance_canonical_export/")
            || rel.contains("/workspace_compatibility_qualification/")
            || rel.contains("/upgrade_integration_qualification/")
            || rel.contains("/migration_boundary/")
    }) {
        for forbidden in [
            "FsWorkspaceReader",
            "std::fs",
            "std::process",
            "HashMap",
            "HashSet",
        ] {
            assert!(!text.contains(forbidden), "{rel} contains \"{forbidden}\"");
        }
    }
}

/// The retired migration API, the pre-package plan types and the blocker
/// mechanism exist nowhere in production code.
#[test]
fn migration_and_qualification_legacy_entrypoints_and_blocker_are_gone() {
    let mut all = sources("meridian-app/src");
    all.extend(sources("meridian-cli/src"));
    all.extend(sources("meridian-core/src"));
    for (rel, text) in all {
        for legacy in [
            "BLOCKED_CHECKS",
            "migration::checks",
            "migration::types",
            "MigrationTypeError",
            "check_idempotency_uniqueness",
            "evaluate_instance_data_migration",
            "evaluate_instance_canonical_export",
            "evaluate_workspace_compatibility_qualification",
            "evaluate_upgrade_integration_qualification",
            "\"blocked\": blocked",
        ] {
            assert!(!text.contains(legacy), "{rel} still names `{legacy}`");
        }
    }
}

#[test]
fn migration_and_qualification_real_adapter_route_is_clean() {
    let root = workspace_root();
    assert_eq!(
        super::instance_data_migration::run(&root).failures,
        Vec::<String>::new()
    );
    assert_eq!(
        super::instance_canonical_export::run(&root).failures,
        Vec::<String>::new()
    );
    assert_eq!(
        super::workspace_compatibility_qualification::run(&root).failures,
        Vec::<String>::new()
    );
    let git = crate::adapters::git_inspector::RealGitInspector::new(&root);
    let catalog = super::task_pattern_registry::run(&root, &git)
        .expect("the real registry is readable")
        .catalog;
    assert!(catalog.is_some());
    assert_eq!(
        super::upgrade_integration_qualification::run(&root, catalog.as_ref()).failures,
        Vec::<String>::new()
    );
}

struct TempRoot(PathBuf);
impl TempRoot {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "rust-architecture-conformance-7-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run_family(family: &str, root: &Path) -> Vec<String> {
    match family {
        "instance-data-migration" => super::instance_data_migration::run(root).failures,
        "instance-canonical-export" => super::instance_canonical_export::run(root).failures,
        "workspace-compatibility-qualification" => {
            super::workspace_compatibility_qualification::run(root).failures
        }
        _ => {
            let git = crate::adapters::git_inspector::RealGitInspector::new(workspace_root());
            let catalog = super::task_pattern_registry::run(&workspace_root(), &git)
                .expect("the real registry is readable")
                .catalog;
            super::upgrade_integration_qualification::run(root, catalog.as_ref()).failures
        }
    }
}

/// An app-generated failure (every mandatory file absent) carries EXACTLY
/// ONE family prefix.
#[test]
fn migration_and_qualification_failures_carry_exactly_one_prefix() {
    let empty = TempRoot::new("empty");
    for (_, family) in MODULES {
        let failures = run_family(family, &empty.0);
        assert_eq!(failures.len(), 1, "{failures:?}");
        let prefix = format!("{family}: ");
        assert!(failures[0].starts_with(&prefix), "{}", failures[0]);
        assert_eq!(failures[0].matches(&prefix).count(), 1, "{}", failures[0]);
        assert!(failures[0].contains(" is missing; "), "{}", failures[0]);
    }
}

/// Without a published task-pattern catalogue the upgrade qualification
/// fails closed without reading the workspace.
#[test]
fn upgrade_integration_qualification_without_a_catalog_fails_closed() {
    let failures = super::upgrade_integration_qualification::run(
        Path::new("/nonexistent-kernel-root-used-only-to-prove-no-io-happens"),
        None,
    )
    .failures;
    assert_eq!(
        failures,
        ["upgrade-integration-qualification: no task-pattern catalog is available; task-pattern-registry must be checked first"]
    );
}

/// Through the REAL `FsWorkspaceReader`, a directory standing where a
/// mandatory file is expected is `ReadError::Io`, never "missing".
#[test]
fn migration_and_qualification_unreadable_is_not_missing() {
    let cases = [
        (
            "instance-data-migration",
            "registries/operating-model/instance-data-migration.schema.json",
        ),
        (
            "instance-canonical-export",
            "registries/operating-model/fixtures/instance-canonical-export.fixtures.json",
        ),
        (
            "workspace-compatibility-qualification",
            "registries/operating-model/existing-project-compatibility-mode.schema.json",
        ),
        (
            "upgrade-integration-qualification",
            "registries/operating-model/upgrade-integration-qualification.schema.json",
        ),
    ];
    let real = workspace_root();
    for (family, unreadable) in cases {
        let tree = TempRoot::new("directory");
        let dir = real.join("registries/operating-model");
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_file() {
                let rel = path.strip_prefix(&real).unwrap();
                let target = tree.0.join(rel);
                std::fs::create_dir_all(target.parent().unwrap()).unwrap();
                std::fs::copy(&path, &target).unwrap();
            }
        }
        for entry in std::fs::read_dir(dir.join("fixtures")).unwrap() {
            let path = entry.unwrap().path();
            let rel = path.strip_prefix(&real).unwrap();
            let target = tree.0.join(rel);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::copy(&path, &target).unwrap();
        }
        let target = tree.0.join(unreadable);
        std::fs::remove_file(&target).unwrap();
        std::fs::create_dir_all(&target).unwrap();
        let failures = run_family(family, &tree.0);
        assert_eq!(failures.len(), 1, "{family}: {failures:?}");
        assert!(
            failures[0].starts_with(&format!("{family}: {unreadable} could not be read: ")),
            "{}",
            failures[0]
        );
        assert!(!failures[0].contains("is missing"), "{}", failures[0]);
    }
}
