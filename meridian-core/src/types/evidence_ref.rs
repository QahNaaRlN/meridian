//! [`EvidenceRef`] — an opaque reference to evidence, resolved elsewhere
//! through an `EvidenceRepository` port (not by `meridian-core`, which does
//! not know that port exists).
//!
//! The validation rule is `checkOpaqueRef`
//! (`scripts/lib/instance-data-migration.mjs`): a portable ref is an opaque
//! Meridian identifier, never a disguised filesystem location.

use core::fmt;

/// A value rejected by [`EvidenceRef::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceRefError {
    /// The input was the empty string.
    Empty,
    /// The input consisted only of whitespace characters.
    Blank,
    /// The input is a `file://` URL.
    FileUrl {
        /// The rejected value.
        value: String,
    },
    /// The input looks like an absolute machine path (a POSIX root path, a
    /// Windows drive path, or contains a backslash).
    AbsoluteMachinePath {
        /// The rejected value.
        value: String,
    },
    /// The input carries a `..` path-escape segment.
    ParentEscape {
        /// The rejected value.
        value: String,
    },
}

impl fmt::Display for EvidenceRefError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvidenceRefError::Empty => write!(f, "evidence ref is empty"),
            EvidenceRefError::Blank => write!(f, "evidence ref is whitespace-only"),
            EvidenceRefError::FileUrl { value } => write!(
                f,
                "\"{value}\" is a file:// URL; an evidence ref must be an opaque identifier, not a filesystem location"
            ),
            EvidenceRefError::AbsoluteMachinePath { value } => write!(
                f,
                "\"{value}\" looks like an absolute machine path; it must be an opaque identifier, not a filesystem location"
            ),
            EvidenceRefError::ParentEscape { value } => {
                write!(f, "\"{value}\" carries a \"..\" segment; it must be an opaque identifier")
            }
        }
    }
}

impl std::error::Error for EvidenceRefError {}

/// An opaque, storage-neutral reference to a piece of evidence.
///
/// `meridian-core` never resolves this reference itself — resolution
/// through an `EvidenceRepository` port belongs to `meridian-app`. This type
/// only guarantees the reference is well-formed as an opaque identifier: not
/// empty, not blank, and not a disguised filesystem location (root POSIX
/// path, Windows drive path, backslash separator, `file://` URL, or a `..`
/// escape segment) — `checkOpaqueRef`'s rule, reused verbatim.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceRef(String);

impl EvidenceRef {
    /// Validates and wraps `value` against the opaque-ref rule.
    pub fn new(value: impl Into<String>) -> Result<Self, EvidenceRefError> {
        let value = value.into();
        validate_opaque_ref(&value)?;
        Ok(Self(value))
    }

    /// Borrows the underlying string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The `checkOpaqueRef` rule, reusable by any other portable-ref field this
/// crate validates (for example a migration plan's `merge_rule_ref` or
/// rollback refs) without a second, diverging copy of the check.
pub(crate) fn validate_opaque_ref(value: &str) -> Result<(), EvidenceRefError> {
    if value.is_empty() {
        return Err(EvidenceRefError::Empty);
    }
    if value.trim().is_empty() {
        return Err(EvidenceRefError::Blank);
    }
    if value.len() >= 7 && value[..7].eq_ignore_ascii_case("file://") {
        return Err(EvidenceRefError::FileUrl {
            value: value.to_string(),
        });
    }
    let looks_like_windows_drive = {
        let bytes = value.as_bytes();
        bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
    };
    if value.contains('\\') || looks_like_windows_drive || value.starts_with('/') {
        return Err(EvidenceRefError::AbsoluteMachinePath {
            value: value.to_string(),
        });
    }
    if value.split('/').any(|segment| segment == "..") {
        return Err(EvidenceRefError::ParentEscape {
            value: value.to_string(),
        });
    }
    Ok(())
}

impl fmt::Display for EvidenceRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for EvidenceRef {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for EvidenceRef {
    type Error = EvidenceRefError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for EvidenceRef {
    type Error = EvidenceRefError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_an_opaque_identifier() {
        assert!(EvidenceRef::new("evidence:coverage-2026-09-18").is_ok());
    }

    #[test]
    fn accepts_a_relative_path_like_reference() {
        assert!(EvidenceRef::new("reports/coverage.json").is_ok());
    }

    #[test]
    fn rejects_empty_and_blank() {
        assert!(EvidenceRef::new("").is_err());
        assert!(EvidenceRef::new("   ").is_err());
    }

    #[test]
    fn rejects_file_url() {
        assert!(matches!(
            EvidenceRef::new("file:///tmp/x").unwrap_err(),
            EvidenceRefError::FileUrl { .. }
        ));
    }

    #[test]
    fn rejects_root_posix_path() {
        assert!(matches!(
            EvidenceRef::new("/etc/passwd").unwrap_err(),
            EvidenceRefError::AbsoluteMachinePath { .. }
        ));
    }

    #[test]
    fn rejects_windows_drive_path() {
        assert!(matches!(
            EvidenceRef::new("C:\\Users\\x").unwrap_err(),
            EvidenceRefError::AbsoluteMachinePath { .. }
        ));
    }

    #[test]
    fn rejects_backslash_without_drive_letter() {
        assert!(matches!(
            EvidenceRef::new("a\\b").unwrap_err(),
            EvidenceRefError::AbsoluteMachinePath { .. }
        ));
    }

    #[test]
    fn rejects_parent_escape_segment() {
        assert!(matches!(
            EvidenceRef::new("a/../b").unwrap_err(),
            EvidenceRefError::ParentEscape { .. }
        ));
    }
}
