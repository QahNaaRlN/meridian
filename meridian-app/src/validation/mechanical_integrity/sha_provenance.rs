//! App-owned orchestration for `sha-provenance` (package 7, subpackage 7a):
//! reads `skills/` through the [`WorkspaceReader`] port, parses each pin
//! with the strict YAML adapter, and hands the parsed facts to
//! `meridian_core::mechanical_integrity::sha_provenance` for validation and
//! the actual diagnostic text. `std::fs` never appears here — only the
//! port. Every fallible read distinguishes the path's domain-meaningful
//! absence from an access/encoding/walk failure, which propagates as
//! [`OperationError`] rather than becoming a false "missing" diagnostic.
//!
//! # Sequencing, read off the report in Node-compatible order
//!
//! [`checks::build_pin_report`]/[`checks::build_archive_report`] (core) are
//! the ONE production construction entry points this module calls —
//! [`checks::validate_pinned_path`]/[`checks::validate_pinned_digest`] are
//! never called independently here any more (corrective round,
//! `meridian-cli-foundation-architecture-remediation`, fourth round, item
//! 1). Each report carries BOTH fields' independently typed results AND
//! the aggregate valid object (`complete()`, built through the real
//! `SkillPin::new`/`SourceArchive::new` — no longer test-only). This module
//! reads `artifact()` before ever looking at `sha256()`, matching the Node
//! reference's own read-artifact-first, sha256-second sequencing. For
//! `source_archive`, this module gates ONLY on each field being present
//! (`Err(…Error::Missing)` on the report) — that, and only that, is the
//! accepted Rust-native `source_archive_incomplete` case
//! (`COMPATIBILITY.md`); it is NOT claimed to reproduce Node's own
//! `if (arch?.path && arch?.sha256)` truthy check by the same mechanism,
//! only the same missing-vs-present split. A field that is present but
//! malformed is never `Missing`, so it never makes the block "incomplete"
//! — it still reaches the read/compare step below, in the same
//! Node-compatible order, using the raw declared text. A single
//! short-circuiting `Result` (`complete()` alone) cannot express any of
//! this — "the artifact read must still be attempted even though `sha256`
//! is what turned out to be missing/malformed" — so this module inspects
//! the report's two typed fields directly rather than only its
//! `complete()`.
//!
//! # Intentional difference from the Node.js/prior Rust reference
//!
//! The pinned `artifact` name and `source_archive.path` are resolved
//! through [`WorkspaceRelativePath::join`], which rejects a `..` component
//! or any other escape out of the skill's own directory — the Node
//! reference joins these attacker/author-controlled strings with
//! `path.join`, which performs no such check. An invalid or escaping name
//! is reported with the SAME "which does not exist" / "source archive ...
//! is missing" diagnostic text an ordinary missing file already produces,
//! so no new diagnostic TEXT appears; the only behavioural difference is
//! that a pin naming an escaping path can no longer be satisfied by a file
//! that happens to exist outside the skill's own directory. Documented in
//! `COMPATIBILITY.md`'s "Намеренные Rust-native усиления" table, with a
//! matched real Node/Rust test on the same mutated Kernel tree
//! (`test/conformance-harness.test.mjs`).

use meridian_core::mechanical_integrity::sha_provenance::{
    self as checks, ArtifactPathError, PinnedDigestError,
};
use meridian_core::types::{ContentDigest, Diagnostic, WorkspaceRelativePath};
use serde_json::Value;

use super::{list_dir, read_bytes, read_text, OperationError, ReadOutcome};
use crate::source_format::parse_yaml;
use crate::workspace::{EntryKind, WorkspaceReader};

#[derive(Debug)]
pub struct Outcome {
    pub diagnostics: Vec<Diagnostic>,
}

/// The raw, not-yet-validated shape of one parsed `PIN.yaml` document —
/// this crate's own closed transport DTO/projection for it. Every field is
/// exactly what `serde_json::Value::get` finds, with no interpretation of
/// absence/validity yet — that is [`checks::build_pin_report`]'s (for
/// `artifact`/`sha256`) and [`checks::build_archive_report`]'s (for
/// `source_archive_path`/`source_archive_sha256`) job: this module never
/// calls `checks::validate_pinned_path`/`checks::validate_pinned_digest`
/// independently, only through those two report builders. Nothing
/// downstream of [`parse_pin`] ever inspects `pin: &Value` directly again.
struct RawPin {
    artifact: Option<String>,
    sha256: Option<String>,
    source_archive_present: bool,
    source_archive_path: Option<String>,
    source_archive_sha256: Option<String>,
}

fn parse_pin(pin: &Value) -> RawPin {
    let source_archive = pin.get("source_archive");
    RawPin {
        artifact: pin
            .get("artifact")
            .and_then(Value::as_str)
            .map(String::from),
        sha256: pin.get("sha256").and_then(Value::as_str).map(String::from),
        source_archive_present: source_archive.is_some_and(|v| !v.is_null()),
        source_archive_path: source_archive
            .and_then(|v| v.get("path"))
            .and_then(Value::as_str)
            .map(String::from),
        source_archive_sha256: source_archive
            .and_then(|v| v.get("sha256"))
            .and_then(Value::as_str)
            .map(String::from),
    }
}

pub fn run(reader: &dyn WorkspaceReader) -> Result<Outcome, OperationError> {
    let mut diagnostics = Vec::new();

    let skills_dir =
        WorkspaceRelativePath::new("skills").expect("\"skills\" is a valid workspace path");
    let mut names: Vec<String> = Vec::new();
    match list_dir(reader, &skills_dir)? {
        ReadOutcome::Present(entries) => {
            for entry in entries {
                if entry.kind == EntryKind::Dir {
                    names.push(entry.name.as_str().to_string());
                }
            }
        }
        ReadOutcome::Absent => {}
    }
    names.sort();

    if names.is_empty() {
        diagnostics.push(checks::no_skills_found());
        return Ok(Outcome { diagnostics });
    }

    for name in names {
        let Ok(dir) = skills_dir.join(&name) else {
            diagnostics.push(checks::missing_pin(&name));
            continue;
        };
        let Ok(pin_path) = dir.join("PIN.yaml") else {
            diagnostics.push(checks::missing_pin(&name));
            continue;
        };
        let pin_raw = match read_text(reader, &pin_path)? {
            ReadOutcome::Present(text) => text,
            ReadOutcome::Absent => {
                diagnostics.push(checks::missing_pin(&name));
                continue;
            }
        };
        let pin = match parse_yaml(&pin_raw) {
            Ok(value) => value,
            Err(error) => {
                diagnostics.push(checks::pin_parse_error(&name, &error.to_string()));
                continue;
            }
        };

        let raw = parse_pin(&pin);

        // Artifact first — the Node reference reads it before ever looking
        // at `sha256`, and a read failure here skips `sha256` entirely.
        // Read off ONE production construction report, never the two
        // validators independently.
        let raw_artifact = raw.artifact.as_deref().unwrap_or("");
        let pin_report = checks::build_pin_report(raw.artifact.as_deref(), raw.sha256.as_deref());
        let artifact_present = match pin_report.artifact() {
            Ok(valid_path) => match dir.join(valid_path.as_str()) {
                Ok(path) => match read_text(reader, &path)? {
                    ReadOutcome::Present(text) => Some(text),
                    ReadOutcome::Absent => None,
                },
                Err(_) => None,
            },
            Err(_) => None,
        };
        let Some(artifact_text) = artifact_present else {
            diagnostics.push(checks::artifact_missing(&name, raw_artifact));
            continue;
        };
        let actual = ContentDigest::of_str(&artifact_text);
        match pin_report.sha256() {
            Err(PinnedDigestError::Missing) => diagnostics.push(checks::missing_sha256(&name)),
            Err(PinnedDigestError::Invalid(_)) => {
                // A malformed pinned value can never equal a real computed
                // digest — the Node reference never validates its shape
                // either, it just compares strings — so this is reported as
                // an ordinary mismatch, using the RAW declared text.
                let raw_sha = raw.sha256.as_deref().unwrap_or("");
                diagnostics.push(checks::artifact_digest_mismatch(&name, &actual, raw_sha));
            }
            Ok(pinned) if pinned.value() != actual.value() => {
                diagnostics.push(checks::artifact_digest_mismatch(
                    &name,
                    &actual,
                    pinned.value(),
                ));
            }
            Ok(_) => {}
        }

        // `source_archive` — the report's `path`/`sha256` are read off ONE
        // production construction report, never the two validators
        // independently. Only an ABSENT/EMPTY field (`Err(…Error::Missing)`
        // on the report) makes the whole block "incomplete" — this is the
        // accepted Rust-native `source_archive_incomplete` case
        // (`COMPATIBILITY.md`), not a claim that this reproduces Node's own
        // `if (arch?.path && arch?.sha256)` truthy check by the same
        // mechanism. A field that is present but malformed is never
        // `Missing`, so it is NOT "incomplete" — it still reaches the
        // read/compare step below, using the raw declared text, in the
        // same Node-compatible order.
        if raw.source_archive_present {
            let raw_archive_path = raw.source_archive_path.as_deref().unwrap_or("");
            let archive_report = checks::build_archive_report(
                raw.source_archive_path.as_deref(),
                raw.source_archive_sha256.as_deref(),
            );
            let path_missing = matches!(archive_report.path(), Err(ArtifactPathError::Missing));
            let sha256_missing = matches!(archive_report.sha256(), Err(PinnedDigestError::Missing));
            if path_missing || sha256_missing {
                diagnostics.push(checks::source_archive_incomplete(&name));
            } else {
                let archive_bytes = match archive_report.path() {
                    Ok(valid_path) => match dir.join(valid_path.as_str()) {
                        Ok(p) => match read_bytes(reader, &p)? {
                            ReadOutcome::Present(bytes) => Some(bytes),
                            ReadOutcome::Absent => None,
                        },
                        Err(_) => None,
                    },
                    Err(_) => None,
                };
                match archive_bytes {
                    None => diagnostics.push(checks::archive_missing(&name, raw_archive_path)),
                    Some(bytes) => {
                        let arch_actual = ContentDigest::of_bytes(&bytes);
                        let matched = matches!(archive_report.sha256(), Ok(d) if d.value() == arch_actual.value());
                        if !matched {
                            let raw_archive_sha =
                                raw.source_archive_sha256.as_deref().unwrap_or("");
                            diagnostics.push(checks::archive_digest_mismatch(
                                &name,
                                &arch_actual,
                                raw_archive_sha,
                            ));
                        }
                    }
                }
            }
        }
    }

    Ok(Outcome { diagnostics })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation::mechanical_integrity::tests::FakeReader;
    use crate::workspace::{DirEntry, ReadError};

    const VALID_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    fn dir_entry(name: &str, kind: EntryKind) -> DirEntry {
        DirEntry {
            name: meridian_core::types::EntryName::new(name).unwrap(),
            kind,
        }
    }

    #[test]
    fn no_skills_directory_is_a_warning_not_a_failure() {
        let reader = FakeReader::new();
        let outcome = run(&reader).unwrap();
        assert_eq!(outcome.diagnostics.len(), 1);
        assert_eq!(
            outcome.diagnostics[0].level(),
            meridian_core::types::DiagnosticLevel::Warn
        );
    }

    #[test]
    fn a_clean_pin_produces_no_diagnostics() {
        let digest = ContentDigest::of_str("hello world\n");
        let reader = FakeReader::new()
            .with_dir("skills", vec![dir_entry("demo", EntryKind::Dir)])
            .with_file("skills/demo/SKILL.md", "hello world\n")
            .with_file(
                "skills/demo/PIN.yaml",
                &format!("artifact: SKILL.md\nsha256: {}\n", digest.value()),
            );
        let outcome = run(&reader).unwrap();
        assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    }

    #[test]
    fn a_clean_pin_with_a_nested_artifact_path_verifies_positively() {
        // Positive counterpart of the escaping-path tests: an ordinary
        // confined relative path, including a subdirectory, still resolves
        // and verifies through `WorkspaceRelativePath` exactly as before.
        let digest = ContentDigest::of_str("nested body\n");
        let reader = FakeReader::new()
            .with_dir("skills", vec![dir_entry("demo", EntryKind::Dir)])
            .with_file("skills/demo/docs/ARTIFACT.md", "nested body\n")
            .with_file(
                "skills/demo/PIN.yaml",
                &format!("artifact: docs/ARTIFACT.md\nsha256: {}\n", digest.value()),
            );
        let outcome = run(&reader).unwrap();
        assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    }

    #[test]
    fn a_missing_pin_is_reported() {
        let reader = FakeReader::new().with_dir("skills", vec![dir_entry("demo", EntryKind::Dir)]);
        let outcome = run(&reader).unwrap();
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(outcome.diagnostics[0].message().contains("has no PIN.yaml"));
    }

    #[test]
    fn a_symlink_entry_is_never_treated_as_a_skill_directory() {
        let reader =
            FakeReader::new().with_dir("skills", vec![dir_entry("linked-skill", EntryKind::Other)]);
        let outcome = run(&reader).unwrap();
        // No dir-kind entries at all -> falls back to the "no skills" warning.
        assert_eq!(outcome.diagnostics.len(), 1);
        assert_eq!(
            outcome.diagnostics[0].level(),
            meridian_core::types::DiagnosticLevel::Warn
        );
    }

    #[test]
    fn a_missing_artifact_field_is_reported_as_missing_with_an_empty_declared_name() {
        let reader = FakeReader::new()
            .with_dir("skills", vec![dir_entry("demo", EntryKind::Dir)])
            .with_file("skills/demo/PIN.yaml", &format!("sha256: {VALID_SHA256}\n"));
        let outcome = run(&reader).unwrap();
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(outcome.diagnostics[0].message().contains("pins \"\""));
    }

    #[test]
    fn an_escaping_artifact_name_is_reported_as_missing_not_read_from_outside_the_skill() {
        let reader = FakeReader::new()
            .with_dir("skills", vec![dir_entry("demo", EntryKind::Dir)])
            .with_file(
                "skills/demo/PIN.yaml",
                &format!("artifact: ../secret.txt\nsha256: {VALID_SHA256}\n"),
            )
            .with_file("secret.txt", "leaked");
        let outcome = run(&reader).unwrap();
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(outcome.diagnostics[0]
            .message()
            .contains("which does not exist"));
    }

    #[test]
    fn a_malformed_pinned_sha256_is_reported_as_an_ordinary_mismatch() {
        let digest = ContentDigest::of_str("ok\n");
        let reader = FakeReader::new()
            .with_dir("skills", vec![dir_entry("demo", EntryKind::Dir)])
            .with_file("skills/demo/SKILL.md", "ok\n")
            .with_file(
                "skills/demo/PIN.yaml",
                "artifact: SKILL.md\nsha256: not-hex\n",
            );
        let outcome = run(&reader).unwrap();
        let _ = digest;
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(outcome.diagnostics[0].message().contains("!= pinned"));
    }

    #[test]
    fn an_escaping_source_archive_path_is_reported_as_missing_not_read_from_outside_the_skill() {
        let digest = ContentDigest::of_str("ok\n");
        let reader = FakeReader::new()
            .with_dir("skills", vec![dir_entry("demo", EntryKind::Dir)])
            .with_file("skills/demo/SKILL.md", "ok\n")
            .with_file(
                "skills/demo/PIN.yaml",
                &format!(
                    "artifact: SKILL.md\nsha256: {}\nsource_archive:\n  path: ../../escaped.zip\n  sha256: {VALID_SHA256}\n",
                    digest.value()
                ),
            )
            .with_file("escaped.zip", "not really a zip");
        let outcome = run(&reader).unwrap();
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(outcome.diagnostics[0].message().contains("is missing"));
    }

    #[test]
    fn a_source_archive_missing_path_or_sha256_is_a_new_incomplete_diagnostic() {
        let digest = ContentDigest::of_str("ok\n");
        let reader = FakeReader::new()
            .with_dir("skills", vec![dir_entry("demo", EntryKind::Dir)])
            .with_file("skills/demo/SKILL.md", "ok\n")
            .with_file(
                "skills/demo/PIN.yaml",
                &format!(
                    "artifact: SKILL.md\nsha256: {}\nsource_archive:\n  path: source/archive.zip\n",
                    digest.value()
                ),
            );
        let outcome = run(&reader).unwrap();
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(outcome.diagnostics[0]
            .message()
            .contains("missing path, sha256, or both"));
    }

    #[test]
    fn a_source_archive_with_a_malformed_but_present_sha256_still_attempts_the_read() {
        // Present-but-invalid is never "incomplete" — the Node reference's
        // own truthy-only gate would still attempt the read, and any
        // malformed pinned value can never match the real computed digest.
        let digest = ContentDigest::of_str("ok\n");
        let reader = FakeReader::new()
            .with_dir("skills", vec![dir_entry("demo", EntryKind::Dir)])
            .with_file("skills/demo/SKILL.md", "ok\n")
            .with_file("skills/demo/archive.zip", "archive bytes")
            .with_file(
                "skills/demo/PIN.yaml",
                &format!(
                    "artifact: SKILL.md\nsha256: {}\nsource_archive:\n  path: archive.zip\n  sha256: not-hex\n",
                    digest.value()
                ),
            );
        let outcome = run(&reader).unwrap();
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(outcome.diagnostics[0].message().contains("source archive"));
        assert!(outcome.diagnostics[0].message().contains("!= pinned"));
    }

    #[test]
    fn an_io_failure_listing_skills_propagates_as_an_operation_error() {
        let reader =
            FakeReader::new().with_error("skills", ReadError::Io("permission denied".to_string()));
        let error = run(&reader).unwrap_err();
        assert_eq!(error.path, "skills");
        assert!(error.message.contains("permission denied"));
    }

    #[test]
    fn an_io_failure_reading_a_pin_propagates_as_an_operation_error_not_a_missing_pin_diagnostic() {
        let reader = FakeReader::new()
            .with_dir("skills", vec![dir_entry("demo", EntryKind::Dir)])
            .with_error(
                "skills/demo/PIN.yaml",
                ReadError::Io("permission denied".to_string()),
            );
        let error = run(&reader).unwrap_err();
        assert_eq!(error.path, "skills/demo/PIN.yaml");
    }

    #[test]
    fn an_io_failure_reading_the_artifact_propagates_as_an_operation_error() {
        let reader = FakeReader::new()
            .with_dir("skills", vec![dir_entry("demo", EntryKind::Dir)])
            .with_file(
                "skills/demo/PIN.yaml",
                &format!("artifact: SKILL.md\nsha256: {VALID_SHA256}\n"),
            )
            .with_error(
                "skills/demo/SKILL.md",
                ReadError::Io("is a directory".to_string()),
            );
        let error = run(&reader).unwrap_err();
        assert_eq!(error.path, "skills/demo/SKILL.md");
    }

    #[test]
    fn an_io_failure_reading_the_source_archive_propagates_as_an_operation_error() {
        let digest = ContentDigest::of_str("ok\n");
        let reader = FakeReader::new()
            .with_dir("skills", vec![dir_entry("demo", EntryKind::Dir)])
            .with_file("skills/demo/SKILL.md", "ok\n")
            .with_file(
                "skills/demo/PIN.yaml",
                &format!(
                    "artifact: SKILL.md\nsha256: {}\nsource_archive:\n  path: archive.zip\n  sha256: {VALID_SHA256}\n",
                    digest.value()
                ),
            )
            .with_error(
                "skills/demo/archive.zip",
                ReadError::Io("permission denied".to_string()),
            );
        let error = run(&reader).unwrap_err();
        assert_eq!(error.path, "skills/demo/archive.zip");
    }

    #[test]
    fn the_real_kernel_vendored_skills_verify_cleanly() {
        let kernel_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let reader = crate::validation::mechanical_integrity::tests::real_fs_reader(kernel_root);
        let outcome = run(&reader).unwrap();
        assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    }
}
