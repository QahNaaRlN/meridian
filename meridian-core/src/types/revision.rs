//! [`Revision`] — the exact pinned revision of a source (for example a Git
//! revision).

use core::fmt;

use super::nonempty::{NonEmptyString, NonEmptyStringError};

/// A value rejected by [`Revision::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionError(NonEmptyStringError);

impl fmt::Display for RevisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid revision: {}", self.0)
    }
}

impl std::error::Error for RevisionError {}

/// The exact revision of a source, opaque to `meridian-core` beyond "not
/// empty, not whitespace-only" (`instance-data-migration.md` §2,
/// `payload.source.revision`).
///
/// This type does not enforce the stricter "exact revision" closed rule
/// (full Git SHA, strict `vX.Y.Z` tag, or SHA-256 digest) that
/// `evidence-and-handoff-contract.md` §4 applies to its own `pinned_ref`
/// values — that contract, and the type carrying its rule, are out of
/// package `rust-domain-core`'s scope (§5 of the target architecture lists
/// only the general `Revision` type).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(NonEmptyString);

impl Revision {
    /// Validates and wraps `value`: rejects empty and whitespace-only input.
    pub fn new(value: impl Into<String>) -> Result<Self, RevisionError> {
        NonEmptyString::new(value).map(Self).map_err(RevisionError)
    }

    /// Borrows the underlying string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for Revision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl AsRef<str> for Revision {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl TryFrom<String> for Revision {
    type Error = RevisionError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for Revision {
    type Error = RevisionError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_git_sha() {
        assert!(Revision::new("37a1dd3").is_ok());
    }

    #[test]
    fn rejects_empty_string() {
        assert!(Revision::new("").is_err());
    }

    #[test]
    fn rejects_whitespace_only() {
        assert!(Revision::new("   ").is_err());
    }
}
