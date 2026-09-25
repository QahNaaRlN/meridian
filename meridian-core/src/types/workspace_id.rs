//! [`WorkspaceId`] — the identifier of a project workspace.
//!
//! `scoped-record.schema.json` `scope_ref.workspace_id` is `$ref:
//! semantic_id`, so a workspace id is validated by the same closed pattern
//! as [`super::SemanticId`] (`^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$`). It is kept
//! as its own type rather than a bare `SemanticId` so a workspace id and an
//! unrelated record id are never accidentally interchangeable at a call
//! site.

use core::fmt;

use super::semantic_id::{SemanticId, SemanticIdError};

/// A value rejected by [`WorkspaceId::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceIdError(SemanticIdError);

impl fmt::Display for WorkspaceIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid workspace id: {}", self.0)
    }
}

impl std::error::Error for WorkspaceIdError {}

/// Identifier of a project workspace (`workspace-scope-model.md` §1).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkspaceId(SemanticId);

impl WorkspaceId {
    /// Validates and wraps `value` against the semantic-id pattern.
    pub fn new(value: impl Into<String>) -> Result<Self, WorkspaceIdError> {
        SemanticId::new(value).map(Self).map_err(WorkspaceIdError)
    }

    /// Borrows the underlying string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for WorkspaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl AsRef<str> for WorkspaceId {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl TryFrom<String> for WorkspaceId {
    type Error = WorkspaceIdError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<&str> for WorkspaceId {
    type Error = WorkspaceIdError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_valid_workspace_id() {
        assert_eq!(
            WorkspaceId::new("sample-project").unwrap().as_str(),
            "sample-project"
        );
    }

    #[test]
    fn rejects_empty_string() {
        assert!(WorkspaceId::new("").is_err());
    }

    #[test]
    fn rejects_uppercase() {
        assert!(WorkspaceId::new("Sample-Project").is_err());
    }

    #[test]
    fn rejects_whitespace_only() {
        assert!(WorkspaceId::new("   ").is_err());
    }
}
