//! Parentage of an `adopt-edition` intake record (D3 of the
//! `rust-workspace-state-validation` handoff).
//!
//! An edition narrows a parent text. Its reference is qualified —
//! repository, path, revision — and carries the digest of the parent at that
//! revision; [`ParentReference`] makes a bare or partial reference
//! unrepresentable. [`judge`] turns what the ports observed about the parent
//! into the verdict, keeping two findings apart:
//!
//! - the reference points at a different text (no such file at the
//!   revision, or another digest) — a defect of the record, `FAIL`;
//! - the parent has moved on since it was taken — an edition that lags,
//!   `WARN`.
//!
//! A repository or revision that cannot be consulted from here is
//! `UNVERIFIED`, never confirmed; a repository no inventory entry names is an
//! unresolvable reference, `FAIL`.

use core::fmt;

use crate::types::{
    ContentDigest, Diagnostic, DiagnosticLevel, RepositoryId, WorkspaceRelativePath,
};

use super::diagnostic;

/// A value rejected by [`CommitRevision::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitRevisionError(String);

impl fmt::Display for CommitRevisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\"{}\" is not a commit revision (7 to 40 lowercase hex digits)",
            self.0
        )
    }
}

impl std::error::Error for CommitRevisionError {}

/// The revision an edition was taken from: 7 to 40 lowercase hex digits,
/// the shape `intake.schema.json` declares.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommitRevision(String);

impl CommitRevision {
    pub fn new(value: impl Into<String>) -> Result<Self, CommitRevisionError> {
        let value = value.into();
        let hex = value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if hex && (7..=40).contains(&value.len()) {
            Ok(Self(value))
        } else {
            Err(CommitRevisionError(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The seven-digit form every message names.
    pub fn short(&self) -> &str {
        &self.0[..7]
    }
}

/// The repository a parent lives in: the Kernel itself, or one the
/// workspace inventory names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParentRepository {
    Kernel,
    Inventory(RepositoryId),
}

impl ParentRepository {
    /// `"kernel"` names the Kernel; any other identifier an inventory entry.
    pub fn new(value: &str) -> Result<Self, String> {
        if value == "kernel" {
            return Ok(Self::Kernel);
        }
        RepositoryId::new(value)
            .map(Self::Inventory)
            .map_err(|e| e.to_string())
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Kernel => "kernel",
            Self::Inventory(id) => id.as_str(),
        }
    }
}

/// The qualified reference of an edition to its parent, with the digest of
/// the parent at the referenced revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParentReference {
    pub repository: ParentRepository,
    pub path: WorkspaceRelativePath,
    pub revision: CommitRevision,
    pub digest: ContentDigest,
}

/// What the parent file is at the head of its repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParentAtHead {
    Present(ContentDigest),
    Absent,
    Unreadable(String),
}

/// What the ports observed about one parent reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParentObservation {
    /// The inventory names no repository with this identifier.
    UnknownRepository,
    /// The repository cannot be consulted from here.
    Unreachable,
    /// The repository does not carry the referenced revision (a shallow or
    /// partial clone, for example).
    RevisionAbsent,
    /// The revision exists but reading the file at it failed.
    Unreadable(String),
    /// The revision holds no such file.
    AbsentAtRevision,
    /// The file at the revision, and at the repository's head.
    Present {
        at_revision: ContentDigest,
        at_head: ParentAtHead,
    },
}

/// The verdict on one edition's parent.
pub fn judge(
    artifact: &str,
    parent: &ParentReference,
    observed: &ParentObservation,
) -> Option<Diagnostic> {
    let repository = parent.repository.as_str();
    let path = parent.path.as_str();
    let short = parent.revision.short();
    let (level, message) = match observed {
        ParentObservation::UnknownRepository => (
            DiagnosticLevel::Fail,
            format!("instruction-intake: {artifact} derives from repository \"{repository}\", which the workspace inventory does not name; the reference cannot be resolved by anyone, here or elsewhere"),
        ),
        ParentObservation::Unreachable => (
            DiagnosticLevel::Info,
            format!("instruction-intake: repository \"{repository}\" is not reachable from here; the parent of {artifact} is UNVERIFIED, not confirmed"),
        ),
        ParentObservation::RevisionAbsent => (
            DiagnosticLevel::Info,
            format!("instruction-intake: revision {short} of {repository} is not present in this checkout; the parent of {artifact} is UNVERIFIED, not confirmed"),
        ),
        ParentObservation::Unreadable(reason) => (
            DiagnosticLevel::Info,
            format!("instruction-intake: \"{path}\" at {short} in {repository} cannot be read ({reason}); the parent of {artifact} is UNVERIFIED, not confirmed"),
        ),
        ParentObservation::AbsentAtRevision => (
            DiagnosticLevel::Fail,
            format!("instruction-intake: {artifact} derives from \"{path}\" at {short} in {repository}, and that revision holds no such file; the reference points at nothing"),
        ),
        ParentObservation::Present { at_revision, .. } if *at_revision != parent.digest => (
            DiagnosticLevel::Fail,
            format!(
                "instruction-intake: {artifact} records parent digest {} but \"{path}\" at {short} hashes to {}; the reference names a different text, which no re-dating can fix",
                &parent.digest.value()[..12],
                &at_revision.value()[..12]
            ),
        ),
        ParentObservation::Present { at_revision, at_head } => match at_head {
            ParentAtHead::Present(head) if head == at_revision => return None,
            ParentAtHead::Present(_) => (
                DiagnosticLevel::Warn,
                format!("instruction-intake: {artifact} — the edition is behind its parent: \"{path}\" has changed in {repository} since {short}. This is not a defective record: re-take the edition against the current parent when the difference matters"),
            ),
            ParentAtHead::Absent => (
                DiagnosticLevel::Warn,
                format!("instruction-intake: {artifact} — its parent \"{path}\" no longer exists at HEAD of {repository}; the edition stays anchored to {short}, but the text it narrows has been removed or moved"),
            ),
            ParentAtHead::Unreadable(reason) => (
                DiagnosticLevel::Info,
                format!("instruction-intake: {artifact} — \"{path}\" at HEAD of {repository} cannot be read ({reason}); whether the edition is behind its parent is UNVERIFIED, not confirmed"),
            ),
        },
    };
    Some(diagnostic(level, message))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference(repository: &str) -> ParentReference {
        ParentReference {
            repository: ParentRepository::new(repository).unwrap(),
            path: WorkspaceRelativePath::new("editions/layers.md").unwrap(),
            revision: CommitRevision::new("c5142b5bb549caf49a892d0747aa14020e6ca6e9").unwrap(),
            digest: ContentDigest::of_str("parent"),
        }
    }

    fn verdict(observed: ParentObservation) -> Option<(DiagnosticLevel, String)> {
        judge("skills/x/SKILL.md", &reference("kernel"), &observed)
            .map(|d| (d.level(), d.message().to_string()))
    }

    #[test]
    fn workspace_state_parentage_types_refuse_an_unqualified_reference() {
        assert!(CommitRevision::new("c5142b5").is_ok());
        for bad in ["c5142b", "C5142B5", "not-a-sha", &"a".repeat(41)] {
            assert!(CommitRevision::new(bad).is_err(), "{bad}");
        }
        assert_eq!(
            ParentRepository::new("kernel").unwrap(),
            ParentRepository::Kernel
        );
        assert!(ParentRepository::new("Not An Id").is_err());
        assert_eq!(
            CommitRevision::new("c5142b5bb549").unwrap().short(),
            "c5142b5"
        );
    }

    #[test]
    fn workspace_state_parentage_a_matching_current_parent_is_confirmed() {
        let digest = ContentDigest::of_str("parent");
        assert_eq!(
            verdict(ParentObservation::Present {
                at_revision: digest.clone(),
                at_head: ParentAtHead::Present(digest),
            }),
            None
        );
    }

    #[test]
    fn workspace_state_parentage_a_different_text_or_no_file_fails() {
        let (level, message) = verdict(ParentObservation::Present {
            at_revision: ContentDigest::of_str("other"),
            at_head: ParentAtHead::Absent,
        })
        .unwrap();
        assert_eq!(level, DiagnosticLevel::Fail);
        assert_eq!(
            message,
            format!(
                "instruction-intake: skills/x/SKILL.md records parent digest {} but \"editions/layers.md\" at c5142b5 hashes to {}; the reference names a different text, which no re-dating can fix",
                &ContentDigest::of_str("parent").value()[..12],
                &ContentDigest::of_str("other").value()[..12]
            )
        );
        assert_eq!(
            verdict(ParentObservation::AbsentAtRevision).unwrap(),
            (
                DiagnosticLevel::Fail,
                "instruction-intake: skills/x/SKILL.md derives from \"editions/layers.md\" at c5142b5 in kernel, and that revision holds no such file; the reference points at nothing".to_string()
            )
        );
        let unknown = judge(
            "a.mdc",
            &reference("ghost"),
            &ParentObservation::UnknownRepository,
        )
        .unwrap();
        assert_eq!(unknown.level(), DiagnosticLevel::Fail);
        assert_eq!(
            unknown.message(),
            "instruction-intake: a.mdc derives from repository \"ghost\", which the workspace inventory does not name; the reference cannot be resolved by anyone, here or elsewhere"
        );
    }

    #[test]
    fn workspace_state_parentage_a_moved_parent_warns_and_an_unreachable_one_is_unverified() {
        let digest = ContentDigest::of_str("parent");
        let behind = verdict(ParentObservation::Present {
            at_revision: digest.clone(),
            at_head: ParentAtHead::Present(ContentDigest::of_str("newer")),
        })
        .unwrap();
        assert_eq!(behind.0, DiagnosticLevel::Warn);
        assert!(behind.1.contains("the edition is behind its parent"));
        let gone = verdict(ParentObservation::Present {
            at_revision: digest,
            at_head: ParentAtHead::Absent,
        })
        .unwrap();
        assert_eq!(gone.0, DiagnosticLevel::Warn);
        assert!(gone.1.contains("no longer exists at HEAD of kernel"));
        for observed in [
            ParentObservation::Unreachable,
            ParentObservation::RevisionAbsent,
            ParentObservation::Unreadable("git failed".into()),
        ] {
            let (level, message) = verdict(observed).unwrap();
            assert_eq!(level, DiagnosticLevel::Info);
            assert!(message.ends_with("UNVERIFIED, not confirmed"), "{message}");
        }
    }
}
