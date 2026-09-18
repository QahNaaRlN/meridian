//! [`Authority`] — who authorises a record
//! (`scoped-record.schema.json` `definitions.authority`).

use core::fmt;

use super::nonempty::{NonEmptyString, NonEmptyStringError};

/// The closed set of authority kinds
/// (`scoped-record.schema.json` `authority.properties.kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AuthorityKind {
    MethodologyOwner,
    User,
    Organization,
    ProjectOwner,
    RepositoryMaintainer,
    /// The authority of the run carrying out an act, never a human owner's
    /// permanent authority (`instance-data-migration.md` §4).
    DelegatedRun,
}

impl AuthorityKind {
    /// The literal string the contract uses for this authority kind.
    pub fn as_str(self) -> &'static str {
        match self {
            AuthorityKind::MethodologyOwner => "methodology-owner",
            AuthorityKind::User => "user",
            AuthorityKind::Organization => "organization",
            AuthorityKind::ProjectOwner => "project-owner",
            AuthorityKind::RepositoryMaintainer => "repository-maintainer",
            AuthorityKind::DelegatedRun => "delegated-run",
        }
    }
}

impl fmt::Display for AuthorityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A value rejected while building an [`Authority`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorityError {
    /// `authority_ref` was empty or whitespace-only.
    InvalidAuthorityRef(NonEmptyStringError),
    /// `decision_ref` was present but empty or whitespace-only.
    InvalidDecisionRef(NonEmptyStringError),
}

impl fmt::Display for AuthorityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthorityError::InvalidAuthorityRef(e) => write!(f, "invalid authority_ref: {e}"),
            AuthorityError::InvalidDecisionRef(e) => write!(f, "invalid decision_ref: {e}"),
        }
    }
}

impl std::error::Error for AuthorityError {}

/// Who authorises a record: a closed `kind`, a required non-empty
/// `authority_ref`, and an optional non-empty `decision_ref`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Authority {
    kind: AuthorityKind,
    authority_ref: NonEmptyString,
    decision_ref: Option<NonEmptyString>,
}

impl Authority {
    /// Validates and builds an `Authority`. `decision_ref` is optional —
    /// pass `None` when the record's authority carries none.
    pub fn new(
        kind: AuthorityKind,
        authority_ref: impl Into<String>,
        decision_ref: Option<String>,
    ) -> Result<Self, AuthorityError> {
        let authority_ref =
            NonEmptyString::new(authority_ref).map_err(AuthorityError::InvalidAuthorityRef)?;
        let decision_ref = decision_ref
            .map(NonEmptyString::new)
            .transpose()
            .map_err(AuthorityError::InvalidDecisionRef)?;
        Ok(Self {
            kind,
            authority_ref,
            decision_ref,
        })
    }

    /// The closed authority kind.
    pub fn kind(&self) -> AuthorityKind {
        self.kind
    }

    /// The authority reference.
    pub fn authority_ref(&self) -> &str {
        self.authority_ref.as_str()
    }

    /// The owner-decision reference, when present.
    pub fn decision_ref(&self) -> Option<&str> {
        self.decision_ref.as_ref().map(NonEmptyString::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_valid_authority_without_decision_ref() {
        let a = Authority::new(AuthorityKind::ProjectOwner, "workspace-owner", None).unwrap();
        assert_eq!(a.kind(), AuthorityKind::ProjectOwner);
        assert_eq!(a.authority_ref(), "workspace-owner");
        assert_eq!(a.decision_ref(), None);
    }

    #[test]
    fn accepts_a_valid_authority_with_decision_ref() {
        let a = Authority::new(
            AuthorityKind::ProjectOwner,
            "workspace-owner",
            Some("owner-decision:branch-naming".to_string()),
        )
        .unwrap();
        assert_eq!(a.decision_ref(), Some("owner-decision:branch-naming"));
    }

    #[test]
    fn rejects_empty_authority_ref() {
        assert!(matches!(
            Authority::new(AuthorityKind::User, "", None).unwrap_err(),
            AuthorityError::InvalidAuthorityRef(_)
        ));
    }

    #[test]
    fn rejects_blank_decision_ref() {
        assert!(matches!(
            Authority::new(AuthorityKind::User, "alice", Some("   ".to_string())).unwrap_err(),
            AuthorityError::InvalidDecisionRef(_)
        ));
    }

    #[test]
    fn delegated_run_is_a_distinct_closed_kind() {
        let a = Authority::new(AuthorityKind::DelegatedRun, "run:42", None).unwrap();
        assert_eq!(a.kind(), AuthorityKind::DelegatedRun);
    }
}
