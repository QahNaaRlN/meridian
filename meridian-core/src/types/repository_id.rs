//! [`RepositoryId`] — the identifier of a local repository scope.
//!
//! Pattern (`registries/rule-resolution/applicability.schema.json`
//! `definitions.record.properties.repository` — "Repository identifier from
//! the Instance inventory"):
//!
//! ```text
//! ^[a-z0-9][a-z0-9-]*$
//! ```
//!
//! This is deliberately a different, slightly looser pattern than
//! [`super::SemanticId`]'s: it allows a leading digit and does not forbid a
//! trailing or doubled hyphen. It is reused exactly as the applicability
//! contract defines it, not tightened to match `SemanticId`.

use core::fmt;

/// A value rejected by [`RepositoryId::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepositoryIdError {
    /// The input was the empty string.
    Empty,
    /// The input does not match `^[a-z0-9][a-z0-9-]*$`.
    InvalidFormat {
        /// The rejected value, carried for a precise diagnostic.
        value: String,
    },
}

impl fmt::Display for RepositoryIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RepositoryIdError::Empty => write!(f, "repository id is empty"),
            RepositoryIdError::InvalidFormat { value } => write!(
                f,
                "\"{value}\" is not a valid repository id (expected ^[a-z0-9][a-z0-9-]*$)"
            ),
        }
    }
}

impl std::error::Error for RepositoryIdError {}

/// Identifier of a local repository scope, from the Instance inventory.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepositoryId(String);

impl RepositoryId {
    /// Validates and wraps `value` against `^[a-z0-9][a-z0-9-]*$`.
    pub fn new(value: impl Into<String>) -> Result<Self, RepositoryIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(RepositoryIdError::Empty);
        }
        if !is_valid(&value) {
            return Err(RepositoryIdError::InvalidFormat { value });
        }
        Ok(Self(value))
    }

    /// Borrows the underlying string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_valid(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

impl fmt::Display for RepositoryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for RepositoryId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for RepositoryId {
    type Error = RepositoryIdError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for RepositoryId {
    type Error = RepositoryIdError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_leading_digit() {
        assert!(RepositoryId::new("1repo").is_ok());
    }

    #[test]
    fn accepts_hyphens() {
        assert!(RepositoryId::new("cbs-core-frontend").is_ok());
    }

    #[test]
    fn rejects_empty_string() {
        assert_eq!(RepositoryId::new("").unwrap_err(), RepositoryIdError::Empty);
    }

    #[test]
    fn rejects_uppercase() {
        assert!(RepositoryId::new("CbsCore").is_err());
    }

    #[test]
    fn rejects_underscore() {
        assert!(RepositoryId::new("cbs_core").is_err());
    }

    #[test]
    fn rejects_leading_hyphen() {
        assert!(RepositoryId::new("-cbs").is_err());
    }
}
