//! [`NonEmptyString`] — a string that is never empty and never
//! whitespace-only, and is never silently trimmed or otherwise normalised on
//! construction.

use core::fmt;

/// A value rejected by [`NonEmptyString::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NonEmptyStringError {
    /// The input was the empty string.
    Empty,
    /// The input consisted only of whitespace characters.
    Blank,
}

impl fmt::Display for NonEmptyStringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NonEmptyStringError::Empty => write!(f, "value is empty"),
            NonEmptyStringError::Blank => write!(f, "value is whitespace-only"),
        }
    }
}

impl std::error::Error for NonEmptyStringError {}

/// A string guaranteed to contain at least one non-whitespace character.
///
/// The value is stored exactly as supplied: construction rejects an empty or
/// whitespace-only input rather than trimming it, so no silent normalisation
/// ever happens on the way in.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NonEmptyString(String);

impl NonEmptyString {
    /// Validates and wraps `value`. Rejects the empty string and a
    /// whitespace-only string; every other input is stored verbatim.
    pub fn new(value: impl Into<String>) -> Result<Self, NonEmptyStringError> {
        let value = value.into();
        if value.is_empty() {
            return Err(NonEmptyStringError::Empty);
        }
        if value.trim().is_empty() {
            return Err(NonEmptyStringError::Blank);
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

impl fmt::Display for NonEmptyString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for NonEmptyString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for NonEmptyString {
    type Error = NonEmptyStringError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for NonEmptyString {
    type Error = NonEmptyStringError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_normal_string() {
        assert_eq!(NonEmptyString::new("hello").unwrap().as_str(), "hello");
    }

    #[test]
    fn rejects_empty_string() {
        assert_eq!(
            NonEmptyString::new("").unwrap_err(),
            NonEmptyStringError::Empty
        );
    }

    #[test]
    fn rejects_whitespace_only_string() {
        assert_eq!(
            NonEmptyString::new("   ").unwrap_err(),
            NonEmptyStringError::Blank
        );
        assert_eq!(
            NonEmptyString::new("\t\n").unwrap_err(),
            NonEmptyStringError::Blank
        );
    }

    #[test]
    fn does_not_trim_a_valid_value() {
        // Surrounding whitespace around otherwise-meaningful content is
        // preserved verbatim, never silently normalised away.
        assert_eq!(
            NonEmptyString::new("  hello  ").unwrap().as_str(),
            "  hello  "
        );
    }
}
