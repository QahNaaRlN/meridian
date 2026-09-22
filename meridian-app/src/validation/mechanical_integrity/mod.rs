//! App-owned orchestration for the five historical `validate-mechanical-integrity`
//! (package 7, subpackage 7a) checks: `sha-provenance`, `instruction-topics`,
//! `operating-foundation`, `stack-profiles` and `agent-instruction-identity`.
//! Each module here reads the Kernel workspace through the
//! [`crate::workspace::WorkspaceReader`] port, parses transport formats with
//! this crate's own strict adapters ([`crate::source_format`]), and hands
//! already-typed facts to the matching
//! `meridian_core::mechanical_integrity` module for the actual predicate
//! and diagnostic text.

pub mod agent_instruction_identity;
pub mod instruction_topics;
pub mod operating_foundation;
pub mod sha_provenance;
pub mod stack_profiles;

use std::fmt;

use fancy_regex::{Captures, Regex};
use meridian_core::types::WorkspaceRelativePath;

use crate::workspace::{DirEntry, ReadError, WorkspaceReader};

/// A `WorkspaceReader` operation failed for a reason that is NOT part of
/// any of the five families' observable domain contract — permission
/// denied, an encoding error, a path that turned out not to be a directory,
/// a directory-entry or `file_type()` failure mid-walk, and so on. Every
/// such failure propagates as this typed error rather than being folded
/// into a domain "missing"/warning/empty result: a Kernel checkout that
/// cannot be READ is an environment problem for `meridian validate` to
/// report as such (`crate::exit_code::INPUT_OR_ENVIRONMENT` at the CLI
/// boundary), never a silent, possibly wrong, "everything is fine" or
/// "nothing here" domain verdict. Contrast [`ReadError::NotFound`] on a
/// path this package's own checks give a designed domain meaning to (a
/// skill's absent `PIN.yaml`, an absent pool file, an absent `skills/`
/// directory) — those stay ordinary [`meridian_core::types::Diagnostic`]s,
/// constructed by the matching `meridian_core::mechanical_integrity`
/// module, never this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationError {
    pub path: String,
    pub message: String,
}

impl fmt::Display for OperationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot read {}: {}", self.path, self.message)
    }
}

impl std::error::Error for OperationError {}

fn operation_error(path: &WorkspaceRelativePath, message: String) -> OperationError {
    OperationError {
        path: path.as_str().to_string(),
        message,
    }
}

/// The three-way outcome every fallible port read collapses to: a value, the
/// path's domain-meaningful absence, or (returned as `Err`, never folded in
/// here) an operational failure. Every family's orchestration matches on
/// this explicitly — there is no fourth, catch-all `.ok()` branch anywhere
/// in this package's production code.
pub(crate) enum ReadOutcome<T> {
    Present(T),
    Absent,
}

pub(crate) fn read_text(
    reader: &dyn WorkspaceReader,
    path: &WorkspaceRelativePath,
) -> Result<ReadOutcome<String>, OperationError> {
    match reader.read_text(path) {
        Ok(text) => Ok(ReadOutcome::Present(text)),
        Err(ReadError::NotFound) => Ok(ReadOutcome::Absent),
        Err(ReadError::Io(message)) => Err(operation_error(path, message)),
    }
}

pub(crate) fn read_bytes(
    reader: &dyn WorkspaceReader,
    path: &WorkspaceRelativePath,
) -> Result<ReadOutcome<Vec<u8>>, OperationError> {
    match reader.read_bytes(path) {
        Ok(bytes) => Ok(ReadOutcome::Present(bytes)),
        Err(ReadError::NotFound) => Ok(ReadOutcome::Absent),
        Err(ReadError::Io(message)) => Err(operation_error(path, message)),
    }
}

pub(crate) fn list_dir(
    reader: &dyn WorkspaceReader,
    path: &WorkspaceRelativePath,
) -> Result<ReadOutcome<Vec<DirEntry>>, OperationError> {
    match reader.list_dir(path) {
        Ok(entries) => Ok(ReadOutcome::Present(entries)),
        Err(ReadError::NotFound) => Ok(ReadOutcome::Absent),
        Err(ReadError::Io(message)) => Err(operation_error(path, message)),
    }
}

/// A compiled `fancy_regex::Regex`'s own evaluation (`is_match`/`captures`/
/// `captures_iter`) is itself fallible — unlike the `regex` crate, a
/// backtracking engine can exhaust its step budget on pathological input.
/// The three helpers below are this package's ONLY call sites for regex
/// evaluation (corrective round,
/// `meridian-cli-foundation-architecture-remediation`, item 2): every one
/// propagates a genuine evaluation failure as an [`OperationError`] — an
/// environment/operational problem, exactly like a `WorkspaceReader` read
/// failure — rather than folding it into `false`/`None`/a silently shorter
/// result list. `path` is the workspace-relative document or region this
/// evaluation was attempting to read, used verbatim in the propagated
/// error.
pub(crate) fn regex_is_match(re: &Regex, text: &str, path: &str) -> Result<bool, OperationError> {
    re.is_match(text).map_err(|error| OperationError {
        path: path.to_string(),
        message: format!("regex evaluation failed: {error}"),
    })
}

pub(crate) fn regex_capture1(
    re: &Regex,
    text: &str,
    path: &str,
) -> Result<Option<String>, OperationError> {
    let captures = re.captures(text).map_err(|error| OperationError {
        path: path.to_string(),
        message: format!("regex evaluation failed: {error}"),
    })?;
    Ok(captures
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string()))
}

pub(crate) fn regex_collect_captures<'t>(
    re: &Regex,
    text: &'t str,
    path: &str,
) -> Result<Vec<Captures<'t, str>>, OperationError> {
    let mut out = Vec::new();
    for captures in re.captures_iter(text) {
        out.push(captures.map_err(|error| OperationError {
            path: path.to_string(),
            message: format!("regex evaluation failed: {error}"),
        })?);
    }
    Ok(out)
}

/// Injected parser-failure tests (corrective round,
/// `meridian-cli-foundation-architecture-remediation`, item 2): a genuine
/// `fancy_regex` evaluation failure — not merely a non-match — propagates
/// through all three helpers above as an [`OperationError`], never as
/// `false`/`None`/a shortened result list. A plain (non-lookaround) pattern
/// is delegated by `fancy_regex` to the non-backtracking `regex` crate
/// internally and can never itself exhaust a backtrack budget, so each test
/// here deliberately builds a pattern that forces the backtracking engine
/// (a lookahead) together with an artificially tiny `backtrack_limit`, and
/// an input shaped to exhaust it deterministically.
#[cfg(test)]
mod regex_helper_tests {
    use fancy_regex::RegexBuilder;

    use super::{regex_capture1, regex_collect_captures, regex_is_match};

    fn failing_regex() -> Regex {
        // `(?=a)` forces the backtracking engine; `(a+)+b` against an input
        // with no trailing `b` is the classic catastrophic-backtracking
        // shape, so even a tiny backtrack_limit is exhausted deterministically.
        RegexBuilder::new(r"(?=a)(a+)+b")
            .backtrack_limit(5)
            .build()
            .expect("pattern compiles")
    }

    use super::Regex;

    #[test]
    fn regex_is_match_propagates_a_real_evaluation_failure() {
        let re = failing_regex();
        let error = regex_is_match(&re, &"a".repeat(40), "doc.md").unwrap_err();
        assert_eq!(error.path, "doc.md");
    }

    #[test]
    fn regex_capture1_propagates_a_real_evaluation_failure() {
        let re = failing_regex();
        let error = regex_capture1(&re, &"a".repeat(40), "doc.md").unwrap_err();
        assert_eq!(error.path, "doc.md");
    }

    #[test]
    fn regex_collect_captures_propagates_a_real_evaluation_failure() {
        let re = failing_regex();
        let error = regex_collect_captures(&re, &"a".repeat(40), "region.md").unwrap_err();
        assert_eq!(error.path, "region.md");
    }

    #[test]
    fn regex_is_match_still_returns_a_plain_bool_on_ordinary_input() {
        let re = Regex::new(r"^ok$").unwrap();
        assert!(regex_is_match(&re, "ok", "doc.md").unwrap());
        assert!(!regex_is_match(&re, "no", "doc.md").unwrap());
    }
}

/// Structural regressions (corrective round,
/// `meridian-cli-foundation-architecture-remediation`, item 4): each
/// family's orchestration calls the REAL, corresponding typed
/// `meridian_core::mechanical_integrity` check by name — not a local
/// reimplementation of the predicate it exists to delegate to. Scoped to
/// each file's own source text, the same discipline
/// `existing_project_compatibility_mode`'s own
/// `orchestration_calls_the_real_core_checks_by_name` test already uses in
/// this crate.
#[cfg(test)]
mod structural_tests {
    fn read(rel: &str) -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} is readable: {e}", path.display()))
    }

    #[test]
    fn each_orchestration_module_calls_its_real_core_checks_by_name() {
        let expectations: &[(&str, &[&str])] = &[
            (
                "src/validation/mechanical_integrity/sha_provenance.rs",
                &[
                    "checks::no_skills_found(",
                    "checks::missing_pin(",
                    "checks::pin_parse_error(",
                    "checks::artifact_missing(",
                    "checks::missing_sha256(",
                    "checks::artifact_digest_mismatch(",
                    "checks::build_pin_report(",
                    "checks::build_archive_report(",
                    "checks::source_archive_incomplete(",
                    "checks::archive_missing(",
                    "checks::archive_digest_mismatch(",
                ],
            ),
            (
                "src/validation/mechanical_integrity/instruction_topics.rs",
                &[
                    "checks::missing_pool_files(",
                    "checks::check_pool_agreement(",
                ],
            ),
            (
                "src/validation/mechanical_integrity/operating_foundation.rs",
                &[
                    "checks::missing_pool_files(",
                    "checks::region_unreadable(",
                    "checks::rows_without_body(",
                    "checks::check_pool(",
                ],
            ),
            (
                "src/validation/mechanical_integrity/stack_profiles.rs",
                &[
                    "checks::missing_pool_files(",
                    "checks::region_unreadable(",
                    "checks::check_pool_agreement(",
                ],
            ),
            (
                "src/validation/mechanical_integrity/agent_instruction_identity.rs",
                &["checks::evaluate_document("],
            ),
        ];
        for (rel, calls) in expectations {
            let text = read(rel);
            for call in *calls {
                assert!(
                    text.contains(call),
                    "{rel} must call `{call}` — the real meridian_core::mechanical_integrity check, not a local reimplementation"
                );
            }
        }
    }

    /// Item 1 of the fourth corrective round: production `sha-provenance`
    /// orchestration calls the report builders
    /// (`checks::build_pin_report`/`checks::build_archive_report`) — the
    /// ONE core entry points that own validation — and never calls
    /// `checks::validate_pinned_path`/`checks::validate_pinned_digest`
    /// independently. Scoped to the file's PRODUCTION text (before its own
    /// `#[cfg(test)]`) so the core crate's own unit tests of the low-level
    /// validators (which legitimately call them directly to prove their
    /// own behaviour) are not mistaken for a second, duplicate validation
    /// path in application code.
    #[test]
    fn sha_provenance_production_code_never_calls_the_low_level_validators_independently() {
        let production = production_text("src/validation/mechanical_integrity/sha_provenance.rs");
        assert!(
            production.contains("checks::build_pin_report("),
            "sha_provenance.rs must call checks::build_pin_report"
        );
        assert!(
            production.contains("checks::build_archive_report("),
            "sha_provenance.rs must call checks::build_archive_report"
        );
        assert!(
            !production.contains("checks::validate_pinned_path("),
            "sha_provenance.rs must not call checks::validate_pinned_path independently — it must read the report's own artifact()/path() instead"
        );
        assert!(
            !production.contains("checks::validate_pinned_digest("),
            "sha_provenance.rs must not call checks::validate_pinned_digest independently — it must read the report's own sha256() instead"
        );
    }

    fn production_text(rel: &str) -> String {
        let text = read(rel);
        let production_end = text.find("#[cfg(test)]").unwrap_or(text.len());
        text[..production_end]
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Item 2's own claim, made mechanical: no production code path in any
    /// of the five families' orchestration (nor this module's own regex
    /// helpers) silently discards a `fancy_regex` `Result` — every
    /// `is_match`/`captures`/`captures_iter` call site goes through
    /// `regex_is_match`/`regex_capture1`/`regex_collect_captures`, and
    /// nowhere else in this crate's `.rs` files does `.ok()`,
    /// `unwrap_or(false)`, or `filter_map(|` immediately follow a regex
    /// evaluation.
    #[test]
    fn no_production_parser_path_silently_discards_a_result() {
        let files = [
            "src/validation/mechanical_integrity/mod.rs",
            "src/validation/mechanical_integrity/agent_instruction_identity.rs",
            "src/validation/mechanical_integrity/sha_provenance.rs",
            "src/validation/mechanical_integrity/operating_foundation.rs",
            "src/validation/mechanical_integrity/instruction_topics.rs",
            "src/validation/mechanical_integrity/stack_profiles.rs",
        ];
        for rel in files {
            let production = production_text(rel);
            for forbidden in [
                ".is_match(text).unwrap_or(false)",
                ".captures(text).ok()",
                ".filter_map(|c| c.ok())",
                ".filter_map(|result| result.ok())",
            ] {
                assert!(
                    !production.contains(forbidden),
                    "{rel} contains \"{forbidden}\" — a fancy_regex evaluation failure must propagate as an OperationError, never fold into false/None/a silently shortened result"
                );
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;

    use meridian_core::types::{EntryName, WorkspaceRelativePath};

    use crate::workspace::{DirEntry, EntryKind, ReadError, WorkspaceReader};

    /// A real-filesystem [`WorkspaceReader`] used only by this crate's own
    /// "the real Kernel content agrees with itself" tests — not a second
    /// production adapter; the one production `WorkspaceReader` is
    /// `meridian_cli::adapters::workspace_reader::FsWorkspaceReader`.
    pub struct RealFsReader {
        root: PathBuf,
    }

    pub fn real_fs_reader(root: PathBuf) -> RealFsReader {
        RealFsReader { root }
    }

    impl WorkspaceReader for RealFsReader {
        fn read_text(&self, path: &WorkspaceRelativePath) -> Result<String, ReadError> {
            fs::read_to_string(self.root.join(path.as_str())).map_err(map_error)
        }
        fn read_bytes(&self, path: &WorkspaceRelativePath) -> Result<Vec<u8>, ReadError> {
            fs::read(self.root.join(path.as_str())).map_err(map_error)
        }
        fn list_dir(&self, path: &WorkspaceRelativePath) -> Result<Vec<DirEntry>, ReadError> {
            let read_dir = fs::read_dir(self.root.join(path.as_str())).map_err(map_error)?;
            let mut entries = Vec::new();
            for entry in read_dir {
                let entry = entry.map_err(map_error)?;
                let file_type = entry.file_type().map_err(map_error)?;
                let kind = if file_type.is_dir() {
                    EntryKind::Dir
                } else if file_type.is_file() {
                    EntryKind::File
                } else {
                    EntryKind::Other
                };
                // Same checked conversion as the one production adapter
                // (`meridian_cli::adapters::workspace_reader::FsWorkspaceReader`)
                // — this test-only reader exists to prove the real Kernel
                // content agrees with itself, and a lossily-mangled name
                // would defeat that.
                let raw_name = entry.file_name().into_string().map_err(|os| {
                    ReadError::Io(format!("entry name is not valid UTF-8: {os:?}"))
                })?;
                let name = EntryName::new(raw_name).map_err(|error| {
                    ReadError::Io(format!("invalid directory entry name: {error}"))
                })?;
                entries.push(DirEntry { name, kind });
            }
            Ok(entries)
        }
    }

    fn map_error(error: std::io::Error) -> ReadError {
        match error.kind() {
            std::io::ErrorKind::NotFound => ReadError::NotFound,
            _ => ReadError::Io(error.to_string()),
        }
    }

    /// A minimal in-memory [`WorkspaceReader`] shared by every family's own
    /// orchestration tests. `errors` lets a test inject an arbitrary
    /// [`ReadError`] (in particular `Io`) for one exact path, taking
    /// priority over `files`/`dirs` — the one place this fake can simulate
    /// an access/encoding/walk failure distinct from a plain absence.
    pub struct FakeReader {
        pub files: HashMap<String, Vec<u8>>,
        pub dirs: HashMap<String, Vec<DirEntry>>,
        pub errors: HashMap<String, ReadError>,
    }

    impl FakeReader {
        pub fn new() -> Self {
            Self {
                files: HashMap::new(),
                dirs: HashMap::new(),
                errors: HashMap::new(),
            }
        }
        pub fn with_file(mut self, path: &str, content: &str) -> Self {
            self.files
                .insert(path.to_string(), content.as_bytes().to_vec());
            self
        }
        pub fn with_dir(mut self, path: &str, entries: Vec<DirEntry>) -> Self {
            self.dirs.insert(path.to_string(), entries);
            self
        }
        pub fn with_error(mut self, path: &str, error: ReadError) -> Self {
            self.errors.insert(path.to_string(), error);
            self
        }
    }

    impl WorkspaceReader for FakeReader {
        fn read_text(&self, path: &WorkspaceRelativePath) -> Result<String, ReadError> {
            if let Some(error) = self.errors.get(path.as_str()) {
                return Err(error.clone());
            }
            self.files
                .get(path.as_str())
                .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
                .ok_or(ReadError::NotFound)
        }
        fn read_bytes(&self, path: &WorkspaceRelativePath) -> Result<Vec<u8>, ReadError> {
            if let Some(error) = self.errors.get(path.as_str()) {
                return Err(error.clone());
            }
            self.files
                .get(path.as_str())
                .cloned()
                .ok_or(ReadError::NotFound)
        }
        fn list_dir(&self, path: &WorkspaceRelativePath) -> Result<Vec<DirEntry>, ReadError> {
            if let Some(error) = self.errors.get(path.as_str()) {
                return Err(error.clone());
            }
            self.dirs
                .get(path.as_str())
                .cloned()
                .ok_or(ReadError::NotFound)
        }
    }
}
