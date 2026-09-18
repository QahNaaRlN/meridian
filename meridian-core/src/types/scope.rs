//! [`Scope`] — the area a record belongs to
//! (`workspace-scope-model.md` §1, `scoped-record.schema.json`
//! `definitions.scope_ref`).
//!
//! Exactly six areas exist, and each variant of [`Scope`] carries exactly
//! the fields that area's own record envelope permits — an invalid
//! combination (for example a `user-profile` scope carrying a
//! `workspace_id`, or a `repository-scope` scope missing one) cannot be
//! constructed at all, rather than being rejected at validation time.

use core::fmt;

use super::semantic_id::{SemanticId, BUILT_IN_METHODOLOGY_ID};
use super::workspace_id::WorkspaceId;

/// The plain discriminant of a [`Scope`] value, matching
/// `scope_ref.properties.type`'s six-value enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScopeType {
    BuiltInMethodology,
    UserProfile,
    OrganizationProfile,
    ProjectWorkspace,
    RepositoryScope,
    RunState,
}

impl ScopeType {
    /// The literal string the contract uses for this scope type.
    pub fn as_str(self) -> &'static str {
        match self {
            ScopeType::BuiltInMethodology => "built-in-methodology",
            ScopeType::UserProfile => "user-profile",
            ScopeType::OrganizationProfile => "organization-profile",
            ScopeType::ProjectWorkspace => "project-workspace",
            ScopeType::RepositoryScope => "repository-scope",
            ScopeType::RunState => "run-state",
        }
    }
}

impl fmt::Display for ScopeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The area a record belongs to — exactly one of the six areas
/// `workspace-scope-model.md` §1 defines.
///
/// Invariants enforced by construction (never by a separate validation
/// pass):
///
/// - `built-in-methodology` always carries the fixed id
///   `"built-in-methodology"` — there is no constructor parameter for it,
///   so a different id cannot be supplied;
/// - `repository-scope` and `run-state` always carry a `workspace_id`
///   (a required constructor parameter);
/// - every other area never carries a `workspace_id` (no such field exists
///   on those variants);
/// - `organization_profile_id` may be carried only by `project-workspace`
///   (only that variant has the field, and there only as an `Option`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Scope {
    BuiltInMethodology,
    UserProfile {
        id: SemanticId,
    },
    OrganizationProfile {
        id: SemanticId,
    },
    ProjectWorkspace {
        id: SemanticId,
        organization_profile_id: Option<SemanticId>,
    },
    RepositoryScope {
        id: SemanticId,
        workspace_id: WorkspaceId,
    },
    RunState {
        id: SemanticId,
        workspace_id: WorkspaceId,
    },
}

impl Scope {
    /// The built-in-methodology scope. Its id is always
    /// `"built-in-methodology"` — fixed by the contract, not a parameter.
    pub fn built_in_methodology() -> Self {
        Scope::BuiltInMethodology
    }

    /// A user-profile scope with the given record id.
    pub fn user_profile(id: SemanticId) -> Self {
        Scope::UserProfile { id }
    }

    /// An organization-profile scope with the given record id.
    pub fn organization_profile(id: SemanticId) -> Self {
        Scope::OrganizationProfile { id }
    }

    /// A project-workspace scope, optionally naming the organization
    /// profile it is linked to (`workspace-scope-model.md` §1: a workspace
    /// may reference an organization profile, but is not required to).
    pub fn project_workspace(id: SemanticId, organization_profile_id: Option<SemanticId>) -> Self {
        Scope::ProjectWorkspace {
            id,
            organization_profile_id,
        }
    }

    /// A repository-scope area, tied to its owning workspace.
    pub fn repository_scope(id: SemanticId, workspace_id: WorkspaceId) -> Self {
        Scope::RepositoryScope { id, workspace_id }
    }

    /// A run-state area, tied to its owning workspace.
    pub fn run_state(id: SemanticId, workspace_id: WorkspaceId) -> Self {
        Scope::RunState { id, workspace_id }
    }

    /// The plain discriminant of this scope.
    pub fn scope_type(&self) -> ScopeType {
        match self {
            Scope::BuiltInMethodology => ScopeType::BuiltInMethodology,
            Scope::UserProfile { .. } => ScopeType::UserProfile,
            Scope::OrganizationProfile { .. } => ScopeType::OrganizationProfile,
            Scope::ProjectWorkspace { .. } => ScopeType::ProjectWorkspace,
            Scope::RepositoryScope { .. } => ScopeType::RepositoryScope,
            Scope::RunState { .. } => ScopeType::RunState,
        }
    }

    /// The record id this scope names — `"built-in-methodology"` for that
    /// fixed variant.
    pub fn id(&self) -> &str {
        match self {
            Scope::BuiltInMethodology => BUILT_IN_METHODOLOGY_ID,
            Scope::UserProfile { id }
            | Scope::OrganizationProfile { id }
            | Scope::ProjectWorkspace { id, .. }
            | Scope::RepositoryScope { id, .. }
            | Scope::RunState { id, .. } => id.as_str(),
        }
    }

    /// The workspace this scope belongs to, when the area carries one
    /// (`repository-scope` and `run-state` only).
    pub fn workspace_id(&self) -> Option<&WorkspaceId> {
        match self {
            Scope::RepositoryScope { workspace_id, .. } | Scope::RunState { workspace_id, .. } => {
                Some(workspace_id)
            }
            _ => None,
        }
    }

    /// The linked organization profile, when this is a `project-workspace`
    /// scope that names one.
    pub fn organization_profile_id(&self) -> Option<&SemanticId> {
        match self {
            Scope::ProjectWorkspace {
                organization_profile_id,
                ..
            } => organization_profile_id.as_ref(),
            _ => None,
        }
    }

    /// Whether this scope and `other` name the same full identity — type,
    /// id, `workspace_id` and `organization_profile_id` all equal
    /// (`instance-data-migration.md` §7, `checkSupersedes`'s `sameScope`).
    pub fn same_scope(&self, other: &Scope) -> bool {
        self == other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sid(s: &str) -> SemanticId {
        SemanticId::new(s).unwrap()
    }
    fn wid(s: &str) -> WorkspaceId {
        WorkspaceId::new(s).unwrap()
    }

    #[test]
    fn built_in_methodology_always_uses_the_fixed_id() {
        let s = Scope::built_in_methodology();
        assert_eq!(s.scope_type(), ScopeType::BuiltInMethodology);
        assert_eq!(s.id(), "built-in-methodology");
        assert_eq!(s.workspace_id(), None);
    }

    #[test]
    fn repository_scope_requires_and_carries_workspace_id() {
        let s = Scope::repository_scope(sid("branch-naming"), wid("sample-project"));
        assert_eq!(s.scope_type(), ScopeType::RepositoryScope);
        assert_eq!(s.workspace_id().unwrap().as_str(), "sample-project");
    }

    #[test]
    fn run_state_requires_and_carries_workspace_id() {
        let s = Scope::run_state(sid("run-42"), wid("sample-project"));
        assert_eq!(s.scope_type(), ScopeType::RunState);
        assert_eq!(s.workspace_id().unwrap().as_str(), "sample-project");
    }

    #[test]
    fn other_areas_never_carry_a_workspace_id() {
        assert_eq!(Scope::user_profile(sid("alice")).workspace_id(), None);
        assert_eq!(
            Scope::organization_profile(sid("acme")).workspace_id(),
            None
        );
        assert_eq!(
            Scope::project_workspace(sid("sample-project"), None).workspace_id(),
            None
        );
    }

    #[test]
    fn only_project_workspace_carries_organization_profile_id() {
        let with_org = Scope::project_workspace(sid("sample-project"), Some(sid("acme")));
        assert_eq!(with_org.organization_profile_id().unwrap().as_str(), "acme");

        let without_org = Scope::project_workspace(sid("sample-project"), None);
        assert_eq!(without_org.organization_profile_id(), None);

        // No other variant even has a constructor parameter for it — the
        // type system makes the invalid combination unrepresentable, so
        // there is nothing further to assert for repository-scope/run-state
        // beyond calling their constructors without such a parameter.
        assert_eq!(
            Scope::repository_scope(sid("x"), wid("sample-project")).organization_profile_id(),
            None
        );
    }

    #[test]
    fn same_scope_requires_full_identity_equality() {
        let a = Scope::project_workspace(sid("p"), Some(sid("acme")));
        let b = Scope::project_workspace(sid("p"), Some(sid("acme")));
        let c = Scope::project_workspace(sid("p"), None);
        assert!(a.same_scope(&b));
        assert!(!a.same_scope(&c));
    }

    #[test]
    fn different_scope_types_with_the_same_id_are_not_equal() {
        let a = Scope::user_profile(sid("x"));
        let b = Scope::organization_profile(sid("x"));
        assert_ne!(a, b);
    }
}
