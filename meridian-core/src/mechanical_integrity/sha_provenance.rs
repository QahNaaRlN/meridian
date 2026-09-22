//! Pure diagnostic-construction half of the `sha-provenance` check (package
//! 7, subpackage 7a): every vendored skill under `skills/` carries a
//! `PIN.yaml` next to it, verified by recomputing the digest of the artifact
//! it pins. File discovery and reading are the app/adapter's job
//! (`meridian_app::validation::mechanical_integrity::sha_provenance`,
//! `meridian-cli`'s `WorkspaceReader` adapter); this module only turns an
//! already-gathered outcome — did the pin parse, does the artifact/archive
//! digest match — into the exact [`crate::types::Diagnostic`] text the
//! Node.js reference (`scripts/kernel-validate.mjs`) produces, so every
//! branch is testable without a filesystem.
//!
//! # Construction reports, on the real production path
//!
//! [`SkillPin`] and [`SourceArchive`] are valid domain values with private
//! fields: neither can exist with a missing, empty, or malformed
//! artifact/path or `sha256`. [`build_pin_report`]/[`build_archive_report`]
//! are the ONE core entry points app orchestration calls — each returns a
//! [`PinConstructionReport`]/[`ArchiveConstructionReport`] carrying BOTH
//! independently typed field results (a pinned path becomes a real
//! [`crate::types::WorkspaceRelativePath`], a pinned digest a real
//! [`ContentDigest`] — never a raw `String` that could silently carry a
//! `..` escape or a malformed value) AND the aggregate valid object
//! (`complete()`), built through the real [`SkillPin::new`]/
//! [`SourceArchive::new`] — never a second, parallel assembly of the same
//! parts (corrective round,
//! `meridian-cli-foundation-architecture-remediation`, fourth round, item
//! 1: "app calls that builder, not the two validators independently").
//! [`validate_pinned_path`]/[`validate_pinned_digest`] remain the two
//! reusable primitives the report builders are made from, never called
//! independently by production app code. A MISSING or INVALID field is an
//! [`ArtifactPathError`]/[`PinnedDigestError`] — never a variant stored
//! inside the valid type — and the app orchestration decides,
//! independently for each of the (at most two) pinned values a check
//! reads, whether to still attempt a read using the raw text (so an
//! escaping or malformed value is reported with the SAME observable
//! diagnostic the prior, already-accepted behavior produces — see each
//! function's own doc comment and `COMPATIBILITY.md`), never by putting an
//! `Option<String>` into [`SkillPin`] or [`SourceArchive`] themselves.

use core::fmt;

use crate::mechanical_integrity::{fail, warn};
use crate::types::{
    ContentDigest, ContentDigestError, Diagnostic, WorkspaceRelativePath,
    WorkspaceRelativePathError,
};

/// A value rejected by [`validate_pinned_path`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactPathError {
    /// The field was absent, or present but the empty string — the Node
    /// reference's own JS-falsy guard (`pin?.artifact`, `arch?.path`)
    /// treats both the same way.
    Missing,
    /// The field was a non-empty string that is not a valid
    /// workspace-relative path (for example one containing a `..`
    /// component that would walk back out of the skill's own directory).
    Invalid(WorkspaceRelativePathError),
}

impl fmt::Display for ArtifactPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArtifactPathError::Missing => write!(f, "no path was declared"),
            ArtifactPathError::Invalid(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ArtifactPathError {}

/// Validates a pinned relative path (`PIN.yaml`'s `artifact`, or its
/// `source_archive.path`): non-empty, and syntactically a valid
/// [`WorkspaceRelativePath`] (no `..`, no absolute/drive-prefixed form, no
/// backslash). Confinement against the skill's own directory is a SEPARATE
/// concern the app/adapter still enforces itself by joining this value onto
/// the skill's `WorkspaceRelativePath` — this function only proves the
/// pinned text is a syntactically plausible relative path at all.
pub fn validate_pinned_path(
    value: Option<&str>,
) -> Result<WorkspaceRelativePath, ArtifactPathError> {
    let value = value
        .filter(|s| !s.is_empty())
        .ok_or(ArtifactPathError::Missing)?;
    WorkspaceRelativePath::new(value).map_err(ArtifactPathError::Invalid)
}

/// A value rejected by [`validate_pinned_digest`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinnedDigestError {
    /// The field was absent, or present but the empty string.
    Missing,
    /// The field was a non-empty string that is not a well-formed 64-hex
    /// SHA-256 value.
    Invalid(ContentDigestError),
}

impl fmt::Display for PinnedDigestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PinnedDigestError::Missing => write!(f, "no sha256 was declared"),
            PinnedDigestError::Invalid(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for PinnedDigestError {}

/// Validates a pinned SHA-256 value (`PIN.yaml`'s `sha256`, or its
/// `source_archive.sha256`): non-empty, and a well-formed 64-character
/// lowercase hex digest.
pub fn validate_pinned_digest(value: Option<&str>) -> Result<ContentDigest, PinnedDigestError> {
    let value = value
        .filter(|s| !s.is_empty())
        .ok_or(PinnedDigestError::Missing)?;
    ContentDigest::from_hex(value).map_err(PinnedDigestError::Invalid)
}

/// A value rejected by [`SkillPin::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillPinError {
    Artifact(ArtifactPathError),
    Sha256(PinnedDigestError),
}

/// One skill's parsed `PIN.yaml` — a valid domain value: private fields,
/// constructible only through [`SkillPin::new`], which rejects a missing,
/// empty, or malformed `artifact`/`sha256` outright. There is no state in
/// which a `SkillPin` exists with an absent or invalid field. The app
/// orchestration does not call `SkillPin::new` directly on its hot path,
/// and never calls [`validate_pinned_path`]/[`validate_pinned_digest`]
/// independently either — corrective round,
/// `meridian-cli-foundation-architecture-remediation`, fourth round item
/// one. It calls [`build_pin_report`] instead, the ONE production
/// construction entry point, which invokes `SkillPin::new` itself to
/// populate [`PinConstructionReport::complete`] and ALSO exposes the two
/// fields' independently typed results. Reading the artifact must be
/// attempted (and, if it fails, reported) even when only `sha256` is the
/// field that is missing or malformed, which `complete()` alone cannot
/// express — the app reads `report.artifact()`/`report.sha256()` in that
/// order, exactly preserving the Node reference's own
/// read-artifact-first, sha256-second sequencing. `SkillPin::new` is real
/// production code, reached on every call to [`build_pin_report`] — not a
/// test-only API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillPin {
    artifact: WorkspaceRelativePath,
    sha256: ContentDigest,
}

impl SkillPin {
    pub fn new(artifact: Option<&str>, sha256: Option<&str>) -> Result<Self, SkillPinError> {
        let artifact = validate_pinned_path(artifact).map_err(SkillPinError::Artifact)?;
        let sha256 = validate_pinned_digest(sha256).map_err(SkillPinError::Sha256)?;
        Ok(Self { artifact, sha256 })
    }

    pub fn artifact(&self) -> &WorkspaceRelativePath {
        &self.artifact
    }

    pub fn sha256(&self) -> &ContentDigest {
        &self.sha256
    }
}

/// A value rejected by [`SourceArchive::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceArchiveError {
    Path(ArtifactPathError),
    Sha256(PinnedDigestError),
}

/// A skill's declared, COMPLETE `source_archive` — a valid domain value:
/// private fields, constructible only through [`SourceArchive::new`], which
/// rejects a missing, empty, or malformed `path`/`sha256` outright. An
/// incomplete OR invalid declaration can never be represented by this type
/// at all. As with [`SkillPin`], the app orchestration does not call
/// `SourceArchive::new` directly on its hot path, and never calls
/// [`validate_pinned_path`]/[`validate_pinned_digest`] independently either
/// (corrective round, `meridian-cli-foundation-architecture-remediation`,
/// fourth round, item 1) — it calls [`build_archive_report`], the ONE
/// production construction entry point, which invokes `SourceArchive::new`
/// itself to populate [`ArchiveConstructionReport::complete`] and ALSO
/// exposes the two fields' independently typed results (needed to
/// distinguish "missing" from "present but invalid shape" and preserve the
/// already-accepted path-confinement and digest-mismatch diagnostics
/// exactly). `SourceArchive::new` is real production code, reached on
/// every call to [`build_archive_report`] — not a test-only API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceArchive {
    path: WorkspaceRelativePath,
    sha256: ContentDigest,
}

impl SourceArchive {
    pub fn new(path: Option<&str>, sha256: Option<&str>) -> Result<Self, SourceArchiveError> {
        let path = validate_pinned_path(path).map_err(SourceArchiveError::Path)?;
        let sha256 = validate_pinned_digest(sha256).map_err(SourceArchiveError::Sha256)?;
        Ok(Self { path, sha256 })
    }

    pub fn path(&self) -> &WorkspaceRelativePath {
        &self.path
    }

    pub fn sha256(&self) -> &ContentDigest {
        &self.sha256
    }
}

/// The one production construction report for a skill's parsed `PIN.yaml`:
/// both independently typed field results, AND the aggregate [`SkillPin`]
/// when both succeed. [`build_pin_report`] is the ONE core function that
/// owns this validation; app orchestration calls it — never
/// [`validate_pinned_path`]/[`validate_pinned_digest`] independently
/// (corrective round, `meridian-cli-foundation-architecture-remediation`,
/// fourth round, item 1) — and reads `artifact`/`sha256` off the report in
/// the Node reference's own order (artifact read first, digest comparison
/// second) to decide which diagnostic, if any, to emit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinConstructionReport {
    artifact: Result<WorkspaceRelativePath, ArtifactPathError>,
    sha256: Result<ContentDigest, PinnedDigestError>,
    complete: Option<SkillPin>,
}

impl PinConstructionReport {
    pub fn artifact(&self) -> Result<&WorkspaceRelativePath, &ArtifactPathError> {
        self.artifact.as_ref()
    }

    pub fn sha256(&self) -> Result<&ContentDigest, &PinnedDigestError> {
        self.sha256.as_ref()
    }

    /// The aggregate, valid domain object — `Some` only when BOTH
    /// `artifact` and `sha256` are `Ok`, built through the real
    /// [`SkillPin::new`], never a second, parallel assembly of the same
    /// two already-validated parts.
    pub fn complete(&self) -> Option<&SkillPin> {
        self.complete.as_ref()
    }
}

/// Builds the one production [`PinConstructionReport`] for a pin's raw,
/// already-extracted `artifact`/`sha256` scalars.
pub fn build_pin_report(artifact: Option<&str>, sha256: Option<&str>) -> PinConstructionReport {
    PinConstructionReport {
        artifact: validate_pinned_path(artifact),
        sha256: validate_pinned_digest(sha256),
        complete: SkillPin::new(artifact, sha256).ok(),
    }
}

/// The one production construction report for a skill's declared
/// `source_archive`. [`build_archive_report`] is called for ANY declared,
/// non-null `source_archive` key — it does NOT presuppose that `path` and
/// `sha256` are already known to be present; that presence check is not a
/// precondition of calling the builder, it is read OFF the report the
/// builder returns. `path`/`sha256` are typed independently (`Result<...,
/// ArtifactPathError>` / `Result<..., PinnedDigestError>`), each `Missing`
/// when the corresponding raw field was absent or empty. `complete` exists
/// only when BOTH fields are simultaneously valid — built through the real
/// [`SourceArchive::new`]. The app
/// (`meridian_app::validation::mechanical_integrity::sha_provenance`) reads
/// `path()`/`sha256()` for `Missing` to decide the accepted Rust-native
/// `source_archive_incomplete` diagnostic (`COMPATIBILITY.md`) — a
/// present-but-malformed field is NOT `Missing` and does not trigger it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveConstructionReport {
    path: Result<WorkspaceRelativePath, ArtifactPathError>,
    sha256: Result<ContentDigest, PinnedDigestError>,
    complete: Option<SourceArchive>,
}

impl ArchiveConstructionReport {
    pub fn path(&self) -> Result<&WorkspaceRelativePath, &ArtifactPathError> {
        self.path.as_ref()
    }

    pub fn sha256(&self) -> Result<&ContentDigest, &PinnedDigestError> {
        self.sha256.as_ref()
    }

    /// The aggregate, valid domain object — `Some` only when BOTH `path`
    /// and `sha256` are `Ok`, built through the real [`SourceArchive::new`].
    pub fn complete(&self) -> Option<&SourceArchive> {
        self.complete.as_ref()
    }
}

/// Builds the one production [`ArchiveConstructionReport`] for a declared
/// `source_archive`'s raw, already-extracted `path`/`sha256` scalars.
pub fn build_archive_report(path: Option<&str>, sha256: Option<&str>) -> ArchiveConstructionReport {
    ArchiveConstructionReport {
        path: validate_pinned_path(path),
        sha256: validate_pinned_digest(sha256),
        complete: SourceArchive::new(path, sha256).ok(),
    }
}

/// No `skills/` entries at all — legal, reported as a warning, not a
/// failure.
pub fn no_skills_found() -> Diagnostic {
    warn("sha-provenance: no vendored skills found")
}

/// `skills/<name>/PIN.yaml` does not exist or could not be read.
pub fn missing_pin(skill_name: &str) -> Diagnostic {
    fail(format!(
        "sha-provenance: {skill_name} has no PIN.yaml; its provenance is unverifiable"
    ))
}

/// `PIN.yaml` exists but did not parse as the strict YAML subset.
pub fn pin_parse_error(skill_name: &str, error: &str) -> Diagnostic {
    fail(format!("sha-provenance: {skill_name} PIN.yaml: {error}"))
}

/// The pinned artifact (defaulting to the empty string when absent —
/// matching the Node reference's own `pin?.artifact ?? ''` fallback) could
/// not be read — including a pinned name that is not a valid
/// workspace-relative path at all (for example one that would walk back out
/// of the skill's own directory): reported identically to a merely-absent
/// artifact, since either way there is nothing at the declared name to
/// verify a digest against. `artifact_name` is always the RAW declared
/// text, never [`SkillPin::artifact`] — the app orchestration calls this
/// precisely when no valid, readable `SkillPin` could be produced, so the
/// typed value does not exist to pass here.
pub fn artifact_missing(skill_name: &str, artifact_name: &str) -> Diagnostic {
    fail(format!(
        "sha-provenance: {skill_name} pins \"{artifact_name}\", which does not exist"
    ))
}

/// The pin records no `sha256` for its artifact at all.
pub fn missing_sha256(skill_name: &str) -> Diagnostic {
    fail(format!(
        "sha-provenance: {skill_name} PIN.yaml records no sha256"
    ))
}

/// The recomputed artifact digest does not match the pinned one. `pinned`
/// is always the RAW declared text (which may be malformed — a malformed
/// value can never equal a real computed digest, so it is reported as an
/// ordinary mismatch, matching the Node reference's own string comparison,
/// which never validates the pinned value's shape at all).
pub fn artifact_digest_mismatch(
    skill_name: &str,
    actual: &ContentDigest,
    pinned: &str,
) -> Diagnostic {
    fail(format!(
        "sha-provenance: {skill_name} artifact {} != pinned {}",
        short(actual.value()),
        short(pinned)
    ))
}

/// A pin declares `source_archive` (with both `path` and `sha256`) but the
/// archive file itself could not be read. `archive_path` is always the RAW
/// declared text.
pub fn archive_missing(skill_name: &str, archive_path: &str) -> Diagnostic {
    fail(format!(
        "sha-provenance: {skill_name} source archive {archive_path} is missing"
    ))
}

/// The recomputed source-archive digest does not match the pinned one.
/// `pinned` is always the RAW declared text (same reasoning as
/// [`artifact_digest_mismatch`]).
pub fn archive_digest_mismatch(
    skill_name: &str,
    actual: &ContentDigest,
    pinned: &str,
) -> Diagnostic {
    fail(format!(
        "sha-provenance: {skill_name} source archive {} != pinned {}",
        short(actual.value()),
        short(pinned)
    ))
}

/// A pin declares a `source_archive` key (present and non-null) but its
/// `path`, `sha256`, or both is missing (JS-falsy: absent or empty) —
/// previously a silently-ignored case entirely; a contradictory/incomplete
/// pin is never permitted to pass through unreported (corrective round,
/// `meridian-cli-foundation-architecture-remediation`, first round, item 2).
/// A `path`/`sha256` that is present but syntactically invalid is NOT this
/// case — the app orchestration still attempts to satisfy it (and reports
/// [`archive_missing`]/[`archive_digest_mismatch`] instead), matching the
/// Node reference's own truthy-only gate (`if (arch?.path && arch?.sha256)`
/// never validates either field's shape).
pub fn source_archive_incomplete(skill_name: &str) -> Diagnostic {
    fail(format!(
        "sha-provenance: {skill_name} declares source_archive but is missing path, sha256, or both; a half-declared source archive is checked against nothing and verifies nothing"
    ))
}

fn short(digest: &str) -> String {
    digest.chars().take(12).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    #[test]
    fn artifact_digest_mismatch_truncates_both_digests_to_twelve_characters() {
        let actual = ContentDigest::of_str("hello world\n");
        let d = artifact_digest_mismatch(
            "demo",
            &actual,
            "dead000000000000000000000000000000000000000000000000000000000000",
        );
        assert!(d.message().contains("!= pinned dead00000000"));
        assert!(!d.message().contains("dead0000000000000000"));
    }

    #[test]
    fn no_skills_found_is_a_warning() {
        assert_eq!(
            no_skills_found().level(),
            crate::types::DiagnosticLevel::Warn
        );
    }

    #[test]
    fn missing_pin_is_a_failure_naming_the_skill() {
        let d = missing_pin("demo");
        assert_eq!(d.level(), crate::types::DiagnosticLevel::Fail);
        assert!(d.message().contains("demo has no PIN.yaml"));
    }

    #[test]
    fn validate_pinned_path_rejects_absent_and_empty() {
        assert_eq!(validate_pinned_path(None), Err(ArtifactPathError::Missing));
        assert_eq!(
            validate_pinned_path(Some("")),
            Err(ArtifactPathError::Missing)
        );
    }

    #[test]
    fn validate_pinned_path_rejects_an_escaping_value() {
        assert!(matches!(
            validate_pinned_path(Some("../../secret.txt")),
            Err(ArtifactPathError::Invalid(_))
        ));
    }

    #[test]
    fn validate_pinned_path_accepts_a_confined_relative_path() {
        let path = validate_pinned_path(Some("docs/artifact.txt")).unwrap();
        assert_eq!(path.as_str(), "docs/artifact.txt");
    }

    #[test]
    fn validate_pinned_digest_rejects_absent_empty_and_malformed() {
        assert_eq!(
            validate_pinned_digest(None),
            Err(PinnedDigestError::Missing)
        );
        assert_eq!(
            validate_pinned_digest(Some("")),
            Err(PinnedDigestError::Missing)
        );
        assert!(matches!(
            validate_pinned_digest(Some("not-hex")),
            Err(PinnedDigestError::Invalid(_))
        ));
    }

    #[test]
    fn validate_pinned_digest_accepts_a_well_formed_value() {
        let digest = validate_pinned_digest(Some(VALID_SHA256)).unwrap();
        assert_eq!(digest.value(), VALID_SHA256);
    }

    // --- SkillPin: no incomplete/empty/invalid value can be constructed ---

    #[test]
    fn skill_pin_cannot_be_constructed_with_a_missing_artifact() {
        assert_eq!(
            SkillPin::new(None, Some(VALID_SHA256)).unwrap_err(),
            SkillPinError::Artifact(ArtifactPathError::Missing)
        );
    }

    #[test]
    fn skill_pin_cannot_be_constructed_with_an_empty_artifact() {
        assert_eq!(
            SkillPin::new(Some(""), Some(VALID_SHA256)).unwrap_err(),
            SkillPinError::Artifact(ArtifactPathError::Missing)
        );
    }

    #[test]
    fn skill_pin_cannot_be_constructed_with_an_escaping_artifact() {
        assert!(matches!(
            SkillPin::new(Some("../escape.txt"), Some(VALID_SHA256)).unwrap_err(),
            SkillPinError::Artifact(ArtifactPathError::Invalid(_))
        ));
    }

    #[test]
    fn skill_pin_cannot_be_constructed_with_a_missing_sha256() {
        assert_eq!(
            SkillPin::new(Some("SKILL.md"), None).unwrap_err(),
            SkillPinError::Sha256(PinnedDigestError::Missing)
        );
    }

    #[test]
    fn skill_pin_cannot_be_constructed_with_an_invalid_sha256() {
        assert!(matches!(
            SkillPin::new(Some("SKILL.md"), Some("not-hex")).unwrap_err(),
            SkillPinError::Sha256(PinnedDigestError::Invalid(_))
        ));
    }

    #[test]
    fn skill_pin_constructs_successfully_with_a_valid_artifact_and_sha256() {
        let pin = SkillPin::new(Some("SKILL.md"), Some(VALID_SHA256)).unwrap();
        assert_eq!(pin.artifact().as_str(), "SKILL.md");
        assert_eq!(pin.sha256().value(), VALID_SHA256);
    }

    // --- SourceArchive: no incomplete/empty/invalid value can be constructed ---

    #[test]
    fn source_archive_cannot_be_constructed_with_a_missing_path() {
        assert_eq!(
            SourceArchive::new(None, Some(VALID_SHA256)).unwrap_err(),
            SourceArchiveError::Path(ArtifactPathError::Missing)
        );
    }

    #[test]
    fn source_archive_cannot_be_constructed_with_an_escaping_path() {
        assert!(matches!(
            SourceArchive::new(Some("../../escaped.zip"), Some(VALID_SHA256)).unwrap_err(),
            SourceArchiveError::Path(ArtifactPathError::Invalid(_))
        ));
    }

    #[test]
    fn source_archive_cannot_be_constructed_with_a_missing_or_invalid_sha256() {
        assert_eq!(
            SourceArchive::new(Some("source/archive.zip"), None).unwrap_err(),
            SourceArchiveError::Sha256(PinnedDigestError::Missing)
        );
        assert!(matches!(
            SourceArchive::new(Some("source/archive.zip"), Some("not-hex")).unwrap_err(),
            SourceArchiveError::Sha256(PinnedDigestError::Invalid(_))
        ));
    }

    #[test]
    fn source_archive_constructs_successfully_with_a_valid_path_and_sha256() {
        let archive = SourceArchive::new(Some("source/archive.zip"), Some(VALID_SHA256)).unwrap();
        assert_eq!(archive.path().as_str(), "source/archive.zip");
        assert_eq!(archive.sha256().value(), VALID_SHA256);
    }

    // --- PinConstructionReport / ArchiveConstructionReport: the one
    // production entry point, carrying both independent field results AND
    // the aggregate valid object ---

    #[test]
    fn pin_report_complete_is_none_when_artifact_is_missing_even_though_sha256_is_valid() {
        let report = build_pin_report(None, Some(VALID_SHA256));
        assert_eq!(report.artifact(), Err(&ArtifactPathError::Missing));
        assert_eq!(
            report.sha256(),
            Ok(&ContentDigest::from_hex(VALID_SHA256).unwrap())
        );
        assert!(report.complete().is_none());
    }

    #[test]
    fn pin_report_complete_is_none_when_sha256_is_missing_even_though_artifact_is_valid() {
        let report = build_pin_report(Some("SKILL.md"), None);
        assert!(report.artifact().is_ok());
        assert_eq!(report.sha256(), Err(&PinnedDigestError::Missing));
        assert!(report.complete().is_none());
    }

    #[test]
    fn pin_report_complete_is_some_only_when_both_fields_are_valid() {
        let report = build_pin_report(Some("SKILL.md"), Some(VALID_SHA256));
        let pin = report.complete().expect("both fields are valid");
        assert_eq!(pin.artifact().as_str(), "SKILL.md");
        assert_eq!(pin.sha256().value(), VALID_SHA256);
    }

    #[test]
    fn archive_report_complete_is_none_when_path_is_escaping_even_though_sha256_is_valid() {
        let report = build_archive_report(Some("../escape.zip"), Some(VALID_SHA256));
        assert!(matches!(report.path(), Err(ArtifactPathError::Invalid(_))));
        assert!(report.sha256().is_ok());
        assert!(report.complete().is_none());
    }

    #[test]
    fn archive_report_complete_is_some_only_when_both_fields_are_valid() {
        let report = build_archive_report(Some("source/archive.zip"), Some(VALID_SHA256));
        let archive = report.complete().expect("both fields are valid");
        assert_eq!(archive.path().as_str(), "source/archive.zip");
        assert_eq!(archive.sha256().value(), VALID_SHA256);
    }
}
