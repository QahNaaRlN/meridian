//! [`WorkspaceRelativePath`] — a validated, portable relative path inside a
//! workspace root: non-empty, POSIX-relative, normalized, carrying no `.` or
//! `..` component, no backslash, and no absolute or drive-prefixed form.
//!
//! A port that only accepts this type can never be asked to escape its own
//! root through a string convention the adapter alone is trusted to check —
//! the "does this path stay inside the root" question is answered once, at
//! construction, not re-derived by every adapter that later joins the value
//! onto a real filesystem root.

use core::fmt;

/// A value rejected by [`WorkspaceRelativePath::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceRelativePathError {
    /// The input was the empty string.
    Empty,
    /// The input started with `/` or a drive letter (`C:`), so it names an
    /// absolute location rather than one relative to a workspace root.
    Absolute { value: String },
    /// The input contained a backslash — never accepted, even on a platform
    /// whose own path separator is one, so a validated value has exactly
    /// one portable spelling.
    Backslash { value: String },
    /// One path component was `.`.
    CurrentDirComponent { value: String },
    /// One path component was `..` — the one shape that could otherwise
    /// walk a joined path back out of its root.
    ParentDirComponent { value: String },
    /// Two consecutive `/` (or a leading/trailing `/`) produced an empty
    /// component.
    EmptyComponent { value: String },
}

impl fmt::Display for WorkspaceRelativePathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkspaceRelativePathError::Empty => write!(f, "workspace path must not be empty"),
            WorkspaceRelativePathError::Absolute { value } => write!(
                f,
                "workspace path \"{value}\" must be relative, not absolute or drive-prefixed"
            ),
            WorkspaceRelativePathError::Backslash { value } => {
                write!(f, "workspace path \"{value}\" must not contain a backslash")
            }
            WorkspaceRelativePathError::CurrentDirComponent { value } => write!(
                f,
                "workspace path \"{value}\" must not contain a \".\" component"
            ),
            WorkspaceRelativePathError::ParentDirComponent { value } => write!(
                f,
                "workspace path \"{value}\" must not contain a \"..\" component"
            ),
            WorkspaceRelativePathError::EmptyComponent { value } => write!(
                f,
                "workspace path \"{value}\" must not contain an empty component"
            ),
        }
    }
}

impl std::error::Error for WorkspaceRelativePathError {}

/// A validated, portable relative path inside a workspace root. The only
/// public constructors are [`WorkspaceRelativePath::new`] and
/// [`WorkspaceRelativePath::join`], both of which reject anything that would
/// let the value escape a root it is later joined onto.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkspaceRelativePath(String);

impl WorkspaceRelativePath {
    /// Validates and wraps `value`: non-empty, relative, `/`-separated,
    /// carrying no `.`/`..`/empty component and no backslash.
    pub fn new(value: impl Into<String>) -> Result<Self, WorkspaceRelativePathError> {
        let value = value.into();
        if value.is_empty() {
            return Err(WorkspaceRelativePathError::Empty);
        }
        if value.contains('\\') {
            return Err(WorkspaceRelativePathError::Backslash { value });
        }
        if value.starts_with('/') || is_drive_prefixed(&value) {
            return Err(WorkspaceRelativePathError::Absolute { value });
        }
        for segment in value.split('/') {
            if segment.is_empty() {
                return Err(WorkspaceRelativePathError::EmptyComponent { value });
            }
            if segment == "." {
                return Err(WorkspaceRelativePathError::CurrentDirComponent { value });
            }
            if segment == ".." {
                return Err(WorkspaceRelativePathError::ParentDirComponent { value });
            }
        }
        Ok(Self(value))
    }

    /// Appends one more `/`-joined segment, validating the combined result
    /// exactly as [`WorkspaceRelativePath::new`] would.
    pub fn join(&self, segment: &str) -> Result<Self, WorkspaceRelativePathError> {
        Self::new(format!("{}/{segment}", self.0))
    }

    /// Borrows the validated, `/`-separated path text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_drive_prefixed(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

impl fmt::Display for WorkspaceRelativePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for WorkspaceRelativePath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_simple_relative_path() {
        let p = WorkspaceRelativePath::new("skills/demo/PIN.yaml").unwrap();
        assert_eq!(p.as_str(), "skills/demo/PIN.yaml");
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(
            WorkspaceRelativePath::new("").unwrap_err(),
            WorkspaceRelativePathError::Empty
        );
    }

    #[test]
    fn rejects_leading_slash() {
        assert!(matches!(
            WorkspaceRelativePath::new("/etc/passwd").unwrap_err(),
            WorkspaceRelativePathError::Absolute { .. }
        ));
    }

    #[test]
    fn rejects_drive_prefix() {
        assert!(matches!(
            WorkspaceRelativePath::new("C:/Windows").unwrap_err(),
            WorkspaceRelativePathError::Absolute { .. }
        ));
    }

    #[test]
    fn rejects_backslash() {
        assert!(matches!(
            WorkspaceRelativePath::new("skills\\demo").unwrap_err(),
            WorkspaceRelativePathError::Backslash { .. }
        ));
    }

    #[test]
    fn rejects_dot_component() {
        assert!(matches!(
            WorkspaceRelativePath::new("skills/./demo").unwrap_err(),
            WorkspaceRelativePathError::CurrentDirComponent { .. }
        ));
    }

    #[test]
    fn rejects_parent_component() {
        assert!(matches!(
            WorkspaceRelativePath::new("skills/../secret").unwrap_err(),
            WorkspaceRelativePathError::ParentDirComponent { .. }
        ));
    }

    #[test]
    fn rejects_empty_component_from_double_slash() {
        assert!(matches!(
            WorkspaceRelativePath::new("skills//demo").unwrap_err(),
            WorkspaceRelativePathError::EmptyComponent { .. }
        ));
    }

    #[test]
    fn rejects_trailing_slash_as_empty_component() {
        assert!(matches!(
            WorkspaceRelativePath::new("skills/demo/").unwrap_err(),
            WorkspaceRelativePathError::EmptyComponent { .. }
        ));
    }

    #[test]
    fn join_validates_the_combined_path() {
        let base = WorkspaceRelativePath::new("skills/demo").unwrap();
        assert_eq!(
            base.join("PIN.yaml").unwrap().as_str(),
            "skills/demo/PIN.yaml"
        );
        assert!(base.join("..").is_err());
        assert!(base.join("").is_err());
    }
}
