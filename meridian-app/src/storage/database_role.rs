//! [`DatabaseRole`] — the closed set of physical-storage roles a storage
//! adapter's backing store can hold (`meridian-rust-migration-program-plan.md`
//! §5.4, item 1).
//!
//! Lives in `meridian-app::storage`, not `meridian-core`: a "database role"
//! is a storage-adapter concept — `meridian-core` carries no notion of a
//! database, SQLite, or a storage port at all
//! (`meridian-rust-sqlite-architecture.md` §"Принятое решение" 3–4). This
//! type is adapter-neutral (it names no SQLite type and no `rusqlite`
//! item), so it belongs at the port boundary this crate defines, not inside
//! the one adapter that happens to implement that boundary today
//! (`meridian-storage-sqlite`).
//!
//! A role is metadata read from inside a database, never guessed from a
//! file path or file name: this type exists so that guess can never even be
//! expressed in code — nothing here accepts a `Path`.

use core::fmt;

use meridian_core::types::ScopeType;

/// A value that does not name one of the two known roles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseRoleError(pub String);

impl fmt::Display for DatabaseRoleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown database role: \"{}\"", self.0)
    }
}

impl std::error::Error for DatabaseRoleError {}

/// The physical role of one storage adapter's backing database
/// (`meridian-rust-target-architecture.md` §4, roles introduced by
/// `knowledge-agent-foundation`).
///
/// - `Tool` — the built-in-methodology database: exactly one Kernel edition,
///   no product data.
/// - `Workspace` — the rest of the six logical areas
///   (`workspace-scope-model.md` §1): everything except
///   `built-in-methodology`.
///
/// Closed by construction: there is no third variant and no "unknown" state
/// a caller can accidentally construct — an unrecognised string is rejected
/// by [`DatabaseRole::try_from`], never silently mapped to either known
/// role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DatabaseRole {
    Tool,
    Workspace,
}

impl DatabaseRole {
    /// The literal string this role is stored and read back as.
    pub fn as_str(self) -> &'static str {
        match self {
            DatabaseRole::Tool => "tool",
            DatabaseRole::Workspace => "workspace",
        }
    }

    /// Whether a record whose scope has this [`ScopeType`] is allowed to be
    /// written to a database holding this role
    /// (`meridian-rust-migration-program-plan.md` §5.4, item 1: "база tool
    /// принимает только built-in-methodology"; the five product areas —
    /// `user-profile`, `organization-profile`, `project-workspace`,
    /// `repository-scope`, `run-state` — are accepted only by `Workspace`).
    pub fn accepts_scope_type(self, scope_type: ScopeType) -> bool {
        match self {
            DatabaseRole::Tool => scope_type == ScopeType::BuiltInMethodology,
            DatabaseRole::Workspace => scope_type != ScopeType::BuiltInMethodology,
        }
    }
}

impl fmt::Display for DatabaseRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<&str> for DatabaseRole {
    type Error = DatabaseRoleError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "tool" => Ok(DatabaseRole::Tool),
            "workspace" => Ok(DatabaseRole::Workspace),
            other => Err(DatabaseRoleError(other.to_string())),
        }
    }
}

impl TryFrom<String> for DatabaseRole {
    type Error = DatabaseRoleError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_as_str() {
        assert_eq!(DatabaseRole::try_from("tool").unwrap(), DatabaseRole::Tool);
        assert_eq!(
            DatabaseRole::try_from("workspace").unwrap(),
            DatabaseRole::Workspace
        );
        assert_eq!(DatabaseRole::Tool.as_str(), "tool");
        assert_eq!(DatabaseRole::Workspace.as_str(), "workspace");
    }

    #[test]
    fn rejects_an_unknown_role_string_instead_of_defaulting() {
        let err = DatabaseRole::try_from("nonsense").unwrap_err();
        assert_eq!(err.0, "nonsense");
    }

    #[test]
    fn rejects_empty_string() {
        assert!(DatabaseRole::try_from("").is_err());
    }

    #[test]
    fn tool_accepts_only_built_in_methodology() {
        assert!(DatabaseRole::Tool.accepts_scope_type(ScopeType::BuiltInMethodology));
        assert!(!DatabaseRole::Tool.accepts_scope_type(ScopeType::UserProfile));
        assert!(!DatabaseRole::Tool.accepts_scope_type(ScopeType::OrganizationProfile));
        assert!(!DatabaseRole::Tool.accepts_scope_type(ScopeType::ProjectWorkspace));
        assert!(!DatabaseRole::Tool.accepts_scope_type(ScopeType::RepositoryScope));
        assert!(!DatabaseRole::Tool.accepts_scope_type(ScopeType::RunState));
    }

    #[test]
    fn workspace_accepts_every_area_except_built_in_methodology() {
        assert!(!DatabaseRole::Workspace.accepts_scope_type(ScopeType::BuiltInMethodology));
        assert!(DatabaseRole::Workspace.accepts_scope_type(ScopeType::UserProfile));
        assert!(DatabaseRole::Workspace.accepts_scope_type(ScopeType::OrganizationProfile));
        assert!(DatabaseRole::Workspace.accepts_scope_type(ScopeType::ProjectWorkspace));
        assert!(DatabaseRole::Workspace.accepts_scope_type(ScopeType::RepositoryScope));
        assert!(DatabaseRole::Workspace.accepts_scope_type(ScopeType::RunState));
    }
}
