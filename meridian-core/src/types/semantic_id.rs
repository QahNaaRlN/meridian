//! [`SemanticId`] — a stable `kebab-case` identifier of a governed entity.
//!
//! Pattern (`registries/operating-model/scoped-record.schema.json`
//! `definitions.semantic_id`, `operating-principles.md` §2):
//!
//! ```text
//! ^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$
//! ```

use core::fmt;

/// A value rejected by [`SemanticId::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticIdError {
    /// The input was the empty string.
    Empty,
    /// The input does not match `^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$`.
    InvalidFormat {
        /// The rejected value, carried for a precise diagnostic.
        value: String,
    },
}

impl fmt::Display for SemanticIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SemanticIdError::Empty => write!(f, "semantic id is empty"),
            SemanticIdError::InvalidFormat { value } => write!(
                f,
                "\"{value}\" is not a valid semantic id (expected lowercase kebab-case matching ^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$)"
            ),
        }
    }
}

impl std::error::Error for SemanticIdError {}

/// A stable, `kebab-case` identifier of a governed entity.
///
/// The closed set of legal values is every string matching
/// `^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$`: it starts with a lowercase letter,
/// contains only lowercase letters and digits within each hyphen-separated
/// segment, and never has an empty, leading, trailing or doubled hyphen
/// segment. No normalisation (case-folding, trimming) is performed —
/// anything outside the pattern is rejected, never silently coerced into it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SemanticId(String);

impl SemanticId {
    /// Validates and wraps `value` against
    /// `^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$`.
    pub fn new(value: impl Into<String>) -> Result<Self, SemanticIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(SemanticIdError::Empty);
        }
        if !is_valid_semantic_id(&value) {
            return Err(SemanticIdError::InvalidFormat { value });
        }
        Ok(Self(value))
    }

    /// Borrows the underlying string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes `self`, returning the underlying `String`.
    pub fn into_string(self) -> String {
        self.0
    }
}

/// The fixed identifier the built-in-methodology scope always carries
/// (`workspace-scope-model.md` §1, `scoped-record.schema.json`
/// `scope_ref.allOf`).
pub const BUILT_IN_METHODOLOGY_ID: &str = "built-in-methodology";

fn is_valid_semantic_id(value: &str) -> bool {
    let mut segments = value.split('-');
    let Some(first) = segments.next() else {
        return false;
    };
    if !is_valid_first_segment(first) {
        return false;
    }
    for segment in segments {
        if !is_valid_tail_segment(segment) {
            return false;
        }
    }
    true
}

fn is_valid_first_segment(segment: &str) -> bool {
    let mut chars = segment.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

fn is_valid_tail_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

impl fmt::Display for SemanticId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for SemanticId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SemanticId {
    type Error = SemanticIdError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for SemanticId {
    type Error = SemanticIdError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_simple_id() {
        assert_eq!(
            SemanticId::new("branch-naming").unwrap().as_str(),
            "branch-naming"
        );
    }

    #[test]
    fn accepts_a_single_letter() {
        assert!(SemanticId::new("a").is_ok());
    }

    #[test]
    fn accepts_digits_after_the_first_letter() {
        assert!(SemanticId::new("a1b2c3").is_ok());
        assert!(SemanticId::new("rust-domain-core-3").is_ok());
    }

    #[test]
    fn rejects_empty_string() {
        assert_eq!(SemanticId::new("").unwrap_err(), SemanticIdError::Empty);
    }

    #[test]
    fn rejects_leading_digit() {
        assert!(SemanticId::new("1abc").is_err());
    }

    #[test]
    fn rejects_uppercase() {
        assert!(SemanticId::new("Branch-Naming").is_err());
        assert!(SemanticId::new("branchNaming").is_err());
    }

    #[test]
    fn rejects_leading_trailing_and_double_hyphen() {
        assert!(SemanticId::new("-branch").is_err());
        assert!(SemanticId::new("branch-").is_err());
        assert!(SemanticId::new("branch--naming").is_err());
    }

    #[test]
    fn rejects_underscore_and_other_punctuation() {
        assert!(SemanticId::new("branch_naming").is_err());
        assert!(SemanticId::new("branch.naming").is_err());
        assert!(SemanticId::new("branch naming").is_err());
    }

    #[test]
    fn rejects_whitespace_only() {
        assert!(SemanticId::new("   ").is_err());
    }

    #[test]
    fn does_not_normalize_case_silently() {
        // A value outside the closed set is rejected, not coerced into it.
        let err = SemanticId::new("Branch").unwrap_err();
        match err {
            SemanticIdError::InvalidFormat { value } => assert_eq!(value, "Branch"),
            other => panic!("expected InvalidFormat, got {other:?}"),
        }
    }
}
