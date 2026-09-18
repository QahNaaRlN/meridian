//! [`WorkItem`] — the resolver's request shape
//! (`rule-resolution.md` §3.1, `rule-resolver.mjs`'s `resolveRules` input).

use core::fmt;

/// The closed pool of work kinds (`rule-resolution.md` §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkKind {
    Change,
    Assessment,
    Operation,
    Initiative,
}

impl WorkKind {
    pub fn as_str(self) -> &'static str {
        match self {
            WorkKind::Change => "change",
            WorkKind::Assessment => "assessment",
            WorkKind::Operation => "operation",
            WorkKind::Initiative => "initiative",
        }
    }
}

impl fmt::Display for WorkKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The closed pool of change classes (`rule-resolution.md` §3). Meaningful
/// only inside [`WorkKind::Change`] — a plain field on a flat struct would
/// let a caller pair a `change_class` with `assessment`; [`WorkItemKind`]
/// makes that combination unrepresentable instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChangeClass {
    Bugfix,
    Feature,
    BehaviorChange,
    Refactor,
}

impl ChangeClass {
    pub fn as_str(self) -> &'static str {
        match self {
            ChangeClass::Bugfix => "BUGFIX",
            ChangeClass::Feature => "FEATURE",
            ChangeClass::BehaviorChange => "BEHAVIOR_CHANGE",
            ChangeClass::Refactor => "REFACTOR",
        }
    }
}

impl fmt::Display for ChangeClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `work_kind`, together with `change_class` exactly where it is meaningful
/// (`rule-resolution.md` §2: "`change_class` is meaningful only inside
/// `work_kind: change`").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkItemKind {
    Change(ChangeClass),
    Assessment,
    Operation,
    Initiative,
}

impl WorkItemKind {
    pub fn work_kind(self) -> WorkKind {
        match self {
            WorkItemKind::Change(_) => WorkKind::Change,
            WorkItemKind::Assessment => WorkKind::Assessment,
            WorkItemKind::Operation => WorkKind::Operation,
            WorkItemKind::Initiative => WorkKind::Initiative,
        }
    }

    pub fn change_class(self) -> Option<ChangeClass> {
        match self {
            WorkItemKind::Change(c) => Some(c),
            _ => None,
        }
    }
}

/// Declared stack/architecture profile hints
/// (`rule-resolution.md` §5, §5.1–§5.2).
///
/// `technology_profile` overrides the repository inventory's own `profile`
/// when present (`resolveRules`: `wi.declared_profiles?.technology_profile
/// ?? repo.profile ?? null`). `architecture_profile` is accepted (carried,
/// type-checked) but is not an axis of the MVP applicability schema and has
/// no effect on resolution (`rule-resolution.md` §5.2) — `rule-resolver.mjs`
/// reads no such field from `ctx` either; kept here only so a caller can
/// supply it without the constructor rejecting the input, matching the
/// Node reference's actual (permissive) acceptance.
///
/// Node's own check for both fields is only "a string when present" — no
/// non-empty requirement — so these are plain `Option<String>`, not the
/// stricter [`crate::types::NonEmptyString`], to avoid rejecting an input
/// the frozen Node reference accepts (an intentional fidelity choice, not
/// an oversight).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct DeclaredProfiles {
    pub technology_profile: Option<String>,
    pub architecture_profile: Option<String>,
}

/// A value rejected by [`WorkItem::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkItemError {
    /// `repository_id` was the empty string. Node's own check is exactly
    /// `=== ''`, not a whitespace-blank check, so a whitespace-only value is
    /// accepted here too — the same intentional fidelity choice as
    /// [`DeclaredProfiles`]'s fields.
    EmptyRepositoryId,
}

impl fmt::Display for WorkItemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorkItemError::EmptyRepositoryId => {
                write!(f, "work item: repository_id must be a non-empty string")
            }
        }
    }
}

impl std::error::Error for WorkItemError {}

/// The resolver's request: exactly the shape of `rule-resolution.md` §3.1.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkItem {
    repository_id: String,
    kind: WorkItemKind,
    candidate_paths: Vec<String>,
    changed_paths: Vec<String>,
    declared_profiles: Option<DeclaredProfiles>,
}

impl WorkItem {
    /// Builds a work item. `candidate_paths` and `changed_paths` are always
    /// supplied (never "missing" as opposed to "empty") because the
    /// constructor requires them, matching `rule-resolver.mjs`'s own
    /// distinction between an absent field (fail-closed) and an empty list.
    pub fn new(
        repository_id: impl Into<String>,
        kind: WorkItemKind,
        candidate_paths: Vec<String>,
        changed_paths: Vec<String>,
        declared_profiles: Option<DeclaredProfiles>,
    ) -> Result<Self, WorkItemError> {
        let repository_id = repository_id.into();
        if repository_id.is_empty() {
            return Err(WorkItemError::EmptyRepositoryId);
        }
        Ok(Self {
            repository_id,
            kind,
            candidate_paths,
            changed_paths,
            declared_profiles,
        })
    }

    pub fn repository_id(&self) -> &str {
        &self.repository_id
    }

    pub fn kind(&self) -> WorkItemKind {
        self.kind
    }

    pub fn candidate_paths(&self) -> &[String] {
        &self.candidate_paths
    }

    pub fn changed_paths(&self) -> &[String] {
        &self.changed_paths
    }

    pub fn declared_profiles(&self) -> Option<&DeclaredProfiles> {
        self.declared_profiles.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_change_work_item() {
        let wi = WorkItem::new(
            "cbs-core-frontend",
            WorkItemKind::Change(ChangeClass::Bugfix),
            vec!["src/a.rs".to_string()],
            vec!["src/a.rs".to_string()],
            None,
        )
        .unwrap();
        assert_eq!(wi.kind().work_kind(), WorkKind::Change);
        assert_eq!(wi.kind().change_class(), Some(ChangeClass::Bugfix));
    }

    #[test]
    fn non_change_work_items_carry_no_change_class() {
        let wi = WorkItem::new("repo", WorkItemKind::Assessment, vec![], vec![], None).unwrap();
        assert_eq!(wi.kind().change_class(), None);
    }

    #[test]
    fn rejects_empty_repository_id() {
        assert_eq!(
            WorkItem::new("", WorkItemKind::Operation, vec![], vec![], None).unwrap_err(),
            WorkItemError::EmptyRepositoryId
        );
    }

    #[test]
    fn accepts_whitespace_only_repository_id_matching_node_laxity() {
        // Node's check is `=== ''`, so whitespace-only survives; this is a
        // deliberate fidelity choice, not an oversight (see WorkItemError
        // doc comment).
        assert!(WorkItem::new("   ", WorkItemKind::Operation, vec![], vec![], None).is_ok());
    }
}
