//! [`EntryName`] — a validated bare directory-entry name: exactly one path
//! component, never a separator, `.`, `..`, or the empty string. Used by the
//! `WorkspaceReader` port (`meridian-app/src/workspace`) so a non-recursive
//! directory listing can never hand its caller a name that would silently
//! become a path-traversal or empty-join hazard when later joined onto a
//! [`crate::types::WorkspaceRelativePath`].
//!
//! An adapter that reads a real directory encounters entry names as
//! platform `OsString`s, which are not guaranteed to be valid UTF-8 at all —
//! [`EntryName::new`] only ever accepts an already-decoded `String`; the
//! adapter's own checked `OsString -> String` conversion (never
//! `to_string_lossy()`) is a precondition this type does not perform itself,
//! documented at each adapter's own call site.

use core::fmt;

/// A value rejected by [`EntryName::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryNameError {
    /// The input was the empty string.
    Empty,
    /// The input contained a `/` or `\` — never a single path component.
    ContainsSeparator { value: String },
    /// The input was exactly `.`.
    CurrentDir,
    /// The input was exactly `..`.
    ParentDir,
}

impl fmt::Display for EntryNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EntryNameError::Empty => write!(f, "directory entry name must not be empty"),
            EntryNameError::ContainsSeparator { value } => write!(
                f,
                "directory entry name \"{value}\" must not contain a path separator"
            ),
            EntryNameError::CurrentDir => {
                write!(f, "directory entry name must not be \".\"")
            }
            EntryNameError::ParentDir => {
                write!(f, "directory entry name must not be \"..\"")
            }
        }
    }
}

impl std::error::Error for EntryNameError {}

/// A validated, single-component directory-entry name.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntryName(String);

impl EntryName {
    /// Validates and wraps `value`: non-empty, no `/` or `\`, and not `.`
    /// or `..`.
    pub fn new(value: impl Into<String>) -> Result<Self, EntryNameError> {
        let value = value.into();
        if value.is_empty() {
            return Err(EntryNameError::Empty);
        }
        if value.contains('/') || value.contains('\\') {
            return Err(EntryNameError::ContainsSeparator { value });
        }
        if value == "." {
            return Err(EntryNameError::CurrentDir);
        }
        if value == ".." {
            return Err(EntryNameError::ParentDir);
        }
        Ok(Self(value))
    }

    /// Borrows the validated name text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EntryName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for EntryName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_an_ordinary_name() {
        let n = EntryName::new("demo").unwrap();
        assert_eq!(n.as_str(), "demo");
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(EntryName::new("").unwrap_err(), EntryNameError::Empty);
    }

    #[test]
    fn rejects_forward_slash() {
        assert!(matches!(
            EntryName::new("a/b").unwrap_err(),
            EntryNameError::ContainsSeparator { .. }
        ));
    }

    #[test]
    fn rejects_backslash() {
        assert!(matches!(
            EntryName::new("a\\b").unwrap_err(),
            EntryNameError::ContainsSeparator { .. }
        ));
    }

    #[test]
    fn rejects_current_dir() {
        assert_eq!(EntryName::new(".").unwrap_err(), EntryNameError::CurrentDir);
    }

    #[test]
    fn rejects_parent_dir() {
        assert_eq!(EntryName::new("..").unwrap_err(), EntryNameError::ParentDir);
    }
}
