//! Structural regressions and real-filesystem boundary tests for the five
//! `validate-mechanical-integrity` (package 7, subpackage 7a) checks —
//! corrective round, `meridian-cli-foundation-architecture-remediation`,
//! items 4 and 5. No production code lives in this file: it exists only to
//! make `cargo test -p meridian-cli mechanical_integrity` (§5.16.6's own
//! targeted gate) exercise a non-empty, meaningful set of real-filesystem
//! and structural tests, rather than matching zero tests by an accident of
//! naming.

#[cfg(test)]
mod structural_tests {
    use std::path::{Path, PathBuf};

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("{} is readable: {e}", path.display()))
    }

    /// The production part of one Rust source text, by one exact rule
    /// (the same the `meridian-app` crate-level scanner applies):
    ///
    /// - blank lines and inner-doc `//!` lines at the head are skipped;
    /// - when the first remaining line is exactly `#![cfg(test)]`, the whole
    ///   file is a test-only module and its production part is empty;
    /// - otherwise production ends before the first OUTER `#[cfg(test)]`
    ///   attribute line.
    ///
    /// A file is never exempted by its name (`tests.rs`), by an
    /// `#![cfg(test)]` anywhere but its head, or by a comment or string
    /// literal that merely mentions either attribute: only a line that IS
    /// the attribute counts.
    fn production_part(text: &str) -> String {
        let first_content = text
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty() && !line.starts_with("//!"));
        if first_content == Some("#![cfg(test)]") {
            return String::new();
        }
        text.lines()
            .take_while(|line| line.trim() != "#[cfg(test)]")
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn production_text(path: &Path) -> String {
        production_part(&read(path))
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn only_an_explicit_test_only_module_file_is_exempt_from_the_production_scan() {
        // An explicit head `#![cfg(test)]`, after blank and `//!` lines.
        let test_only = "//! Tests.\n\n#![cfg(test)]\n\nimpl WorkspaceReader for Fake {}\n";
        assert_eq!(production_part(test_only), "");

        // An ordinary production file is entirely production.
        let production = "use std::fs;\n\nimpl WorkspaceReader for Real {}\n";
        assert!(production_part(production).contains("impl WorkspaceReader for Real"));

        // A late or quoted mention of `#![cfg(test)]` exempts nothing.
        let late = "impl WorkspaceReader for Late {}\n#![cfg(test)]\n";
        assert!(production_part(late).contains("impl WorkspaceReader for Late"));
        let quoted = "const MARK: &str = \"#![cfg(test)]\";\nimpl WorkspaceReader for Quoted {}\n";
        assert!(production_part(quoted).contains("impl WorkspaceReader for Quoted"));
        let ordinary_comment_first =
            "// #![cfg(test)]\n#![cfg(test)]\nimpl WorkspaceReader for Commented {}\n";
        assert!(
            production_part(ordinary_comment_first).contains("impl WorkspaceReader for Commented")
        );

        // Production code before an outer `#[cfg(test)]` stays production;
        // what follows the attribute line does not. A comment or literal
        // mentioning the outer attribute does not end production early.
        let mixed = "/// Mentions #[cfg(test)] in a doc comment.\nimpl WorkspaceReader for Before {}\nconst S: &str = \"#[cfg(test)]\";\n#[cfg(test)]\nmod tests { impl WorkspaceReader for After {} }\n";
        let part = production_part(mixed);
        assert!(part.contains("impl WorkspaceReader for Before"), "{part}");
        assert!(part.contains("const S"), "{part}");
        assert!(!part.contains("impl WorkspaceReader for After"), "{part}");
    }

    fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
        let entries =
            std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{} is readable: {e}", dir.display()));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|e| panic!("dir entry is readable: {e}"))
                .path();
            if path.is_dir() {
                collect_rs_files(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }

    /// The five CLI shim modules for `validate-mechanical-integrity`
    /// contain none of: `std::fs`, a regex engine, `serde_json::Value`, or
    /// a locally-defined `fn check_*`/`fn evaluate_*` predicate — every one
    /// of those belongs to `meridian-app`/`meridian-core` now. A shim is
    /// transport-only: build the real adapter, call the real app function,
    /// split its `Diagnostic`s into the legacy `(failures, warnings)`
    /// shape.
    #[test]
    fn the_five_cli_shims_contain_no_domain_logic_fs_regex_or_raw_json_value() {
        let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands/validate");
        let shims = [
            "sha_provenance.rs",
            "instruction_topics.rs",
            "operating_foundation.rs",
            "stack_profiles.rs",
            "agent_instruction_identity.rs",
        ];
        for shim in shims {
            let path = base.join(shim);
            let production = production_text(&path);
            for forbidden in [
                "std::fs",
                "fancy_regex",
                "Regex::new",
                "serde_json::Value",
                "fn check_",
                "fn evaluate_",
                "parse_yaml(",
                "marked_region(",
            ] {
                assert!(
                    !production.contains(forbidden),
                    "{} contains \"{forbidden}\" — a CLI shim must stay transport-only (concrete filesystem adaptation, presentation, exit behaviour), never domain logic, regex or raw serde_json::Value",
                    path.display()
                );
            }
            // Positive half of the same claim: it actually delegates.
            assert!(
                production.contains("app::run("),
                "{} must call the real app::run — this test would not catch a shim that does nothing at all otherwise",
                path.display()
            );
        }
    }

    /// Production (across all three crates) has exactly one concrete
    /// `WorkspaceReader` implementation:
    /// `meridian_cli::adapters::workspace_reader::FsWorkspaceReader`.
    /// `meridian-core` never sees the port at all (no `serde`/I/O
    /// dependency); `meridian-app`'s own equivalent structural test
    /// (`meridian_app::validation::mechanical_integrity::tests` and
    /// `meridian_app`'s crate-level test) already proves it defines none in
    /// ITS production code — this test re-scans that crate too, so the
    /// total counted here is authoritative across the whole workspace, not
    /// merely "this crate's own count is zero".
    #[test]
    fn production_has_exactly_one_concrete_workspace_reader_implementation() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("meridian-cli has a parent directory");
        let mut count = 0usize;
        let mut where_found = Vec::new();
        for crate_dir in ["meridian-core/src", "meridian-app/src", "meridian-cli/src"] {
            let src = workspace_root.join(crate_dir);
            let mut files = Vec::new();
            collect_rs_files(&src, &mut files);
            for file in files {
                let production = production_text(&file);
                let occurrences = production.matches("impl WorkspaceReader for").count();
                if occurrences > 0 {
                    count += occurrences;
                    where_found.push(file.display().to_string());
                }
            }
        }
        assert_eq!(
            count, 1,
            "expected exactly one production `impl WorkspaceReader for` across the workspace, found {count} in: {where_found:?}"
        );
        assert!(
            where_found
                .iter()
                .any(|f| f.ends_with("adapters/workspace_reader.rs")),
            "the one production implementation must be meridian-cli's own FsWorkspaceReader, found in: {where_found:?}"
        );
    }
}

#[cfg(test)]
mod real_filesystem_boundary_tests {
    use std::path::{Path, PathBuf};

    use meridian_app::validation::mechanical_integrity::{instruction_topics, sha_provenance};
    use meridian_core::types::ContentDigest;

    use crate::adapters::workspace_reader::FsWorkspaceReader;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "meridian-cli-mechanical-integrity-boundary-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            // A permission-denied test below strips write/execute from a
            // file or directory it created; restore before removal so
            // cleanup itself never fails.
            restore_permissions(&self.0);
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(unix)]
    fn restore_permissions(root: &Path) {
        use std::os::unix::fs::PermissionsExt;
        fn walk(dir: &Path) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755));
                if path.is_dir() {
                    walk(&path);
                }
            }
        }
        let _ = std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o755));
        walk(root);
    }
    #[cfg(not(unix))]
    fn restore_permissions(_root: &Path) {}

    fn write(dir: &Path, rel: &str, content: &str) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    /// Real-filesystem success: a genuine `WorkspaceReader` over real files
    /// on disk, not the in-memory fake `meridian-app`'s own tests use.
    #[test]
    fn real_fs_success_instruction_topics() {
        let temp = TempDir::new("topics-success");
        write(
            temp.path(),
            "standards/workspace/instruction-topics.yaml",
            "topics:\n  - agent-conduct\n",
        );
        write(
            temp.path(),
            "standards/workspace/instruction-topics.md",
            "<!-- meridian:begin topic-pool -->\n| `agent-conduct` | x |\n<!-- meridian:end topic-pool -->\n",
        );
        let reader = FsWorkspaceReader::new(temp.path());
        let outcome = instruction_topics::run(&reader).unwrap();
        assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
        assert!(outcome.topic_pool.is_some());
    }

    /// Real-filesystem, domain-meaningful absence: no pool files at all —
    /// a real `ReadError::NotFound` from a real `fs::read_to_string`,
    /// reported as the ordinary "missing pool" diagnostic, not an error.
    #[test]
    fn real_fs_missing_instruction_topics_pool() {
        let temp = TempDir::new("topics-missing");
        let reader = FsWorkspaceReader::new(temp.path());
        let outcome = instruction_topics::run(&reader).unwrap();
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(outcome.diagnostics[0]
            .message()
            .contains("is missing from the Kernel"));
    }

    /// Real-filesystem success for `sha-provenance`, including its
    /// `source_archive` half, over real files with a real recomputed
    /// SHA-256.
    #[test]
    fn real_fs_success_sha_provenance() {
        let temp = TempDir::new("sha-success");
        let digest = ContentDigest::of_str("hello world\n");
        write(temp.path(), "skills/demo/SKILL.md", "hello world\n");
        write(
            temp.path(),
            "skills/demo/PIN.yaml",
            &format!("artifact: SKILL.md\nsha256: {}\n", digest.value()),
        );
        let reader = FsWorkspaceReader::new(temp.path());
        let outcome = sha_provenance::run(&reader).unwrap();
        assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    }

    /// Real-filesystem, domain-meaningful absence for `sha-provenance`: no
    /// `skills/` directory at all is a warning, not an operation error —
    /// `ReadError::NotFound` from a real `fs::read_dir`.
    #[test]
    fn real_fs_missing_skills_directory_is_a_warning() {
        let temp = TempDir::new("sha-no-skills");
        let reader = FsWorkspaceReader::new(temp.path());
        let outcome = sha_provenance::run(&reader).unwrap();
        assert_eq!(outcome.diagnostics.len(), 1);
        assert_eq!(
            outcome.diagnostics[0].level(),
            meridian_core::types::DiagnosticLevel::Warn
        );
    }

    /// Real-filesystem walk failure: `skills` exists but is a PLAIN FILE,
    /// not a directory — a real `fs::read_dir` `Io` failure (not
    /// `NotFound`), which must propagate as an operation error and must
    /// NOT be silently folded into "no vendored skills found" (the exact
    /// defect this corrective round's item 1 closes: the prior
    /// `if let Ok(entries) = fs::read_dir(...)` swallowed this case
    /// identically to a genuinely absent directory).
    #[test]
    fn real_fs_skills_as_a_plain_file_is_an_operation_error_not_a_warning() {
        let temp = TempDir::new("sha-skills-is-a-file");
        std::fs::write(temp.path().join("skills"), "not a directory").unwrap();
        let reader = FsWorkspaceReader::new(temp.path());
        let error = sha_provenance::run(&reader).unwrap_err();
        assert_eq!(error.path, "skills");
    }

    /// Real-filesystem symlink boundary: a symlink inside `skills/`
    /// pointing at a real directory is never enrolled as a skill (its own
    /// `PIN.yaml` does not exist under the symlink's own name) — the real
    /// non-following `file_type()` semantics, not the fake reader's.
    #[test]
    #[cfg(unix)]
    fn real_fs_symlinked_entry_under_skills_is_never_treated_as_a_skill_directory() {
        use std::os::unix::fs::symlink;
        let temp = TempDir::new("sha-symlink");
        let digest = ContentDigest::of_str("hello world\n");
        write(temp.path(), "real-skill/SKILL.md", "hello world\n");
        write(
            temp.path(),
            "real-skill/PIN.yaml",
            &format!("artifact: SKILL.md\nsha256: {}\n", digest.value()),
        );
        std::fs::create_dir_all(temp.path().join("skills")).unwrap();
        symlink(
            temp.path().join("real-skill"),
            temp.path().join("skills/linked-skill"),
        )
        .unwrap();
        let reader = FsWorkspaceReader::new(temp.path());
        let outcome = sha_provenance::run(&reader).unwrap();
        // The symlink is not a directory entry -> no skill dirs at all ->
        // the "no vendored skills found" warning, not a failure about a
        // missing PIN.yaml for "linked-skill".
        assert_eq!(outcome.diagnostics.len(), 1);
        assert_eq!(
            outcome.diagnostics[0].level(),
            meridian_core::types::DiagnosticLevel::Warn
        );
    }

    /// Real-filesystem, genuinely unreadable file: a real permission-denied
    /// `PIN.yaml` (chmod `000`) must propagate as an operation error, never
    /// be reported as the ordinary "has no PIN.yaml" domain diagnostic —
    /// the two are different real causes and must stay distinguishable.
    /// Not run as root (root ignores file-mode permission bits there, so
    /// the read would succeed regardless and this assertion would not be
    /// meaningful) — checked the same way this codebase's own git hooks
    /// skip such cases, via the `$USER`/`whoami` environment this crate's
    /// own tests otherwise never depend on: skipped only when unreadable.
    #[test]
    #[cfg(unix)]
    fn real_fs_permission_denied_pin_propagates_as_an_operation_error() {
        use std::os::unix::fs::PermissionsExt;
        let temp = TempDir::new("sha-permission-denied");
        write(
            temp.path(),
            "skills/demo/PIN.yaml",
            "artifact: SKILL.md\nsha256: abc\n",
        );
        let pin_path = temp.path().join("skills/demo/PIN.yaml");
        std::fs::set_permissions(&pin_path, std::fs::Permissions::from_mode(0o000)).unwrap();
        if std::fs::read_to_string(&pin_path).is_ok() {
            eprintln!(
                "skipping: {} is still readable despite mode 0o000 (likely running as root); permission bits are not enforced here",
                pin_path.display()
            );
            return;
        }
        let reader = FsWorkspaceReader::new(temp.path());
        let error = sha_provenance::run(&reader).unwrap_err();
        assert_eq!(error.path, "skills/demo/PIN.yaml");
    }
}
