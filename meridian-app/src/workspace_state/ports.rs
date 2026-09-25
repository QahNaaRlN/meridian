//! The two ports the workspace-state operation needs beyond
//! [`RecordRepository`](crate::storage::RecordRepository) and
//! [`WorkspaceReader`]: the local product repositories the inventory names,
//! and the current instant. Their production implementations live in
//! `meridian-cli` (`adapters::repository_access`, `adapters::clock`); this
//! crate only defines them.

use meridian_core::types::WorkspaceRelativePath;
use meridian_core::workspace_state::inventory::ObservedVcs;
use meridian_core::workspace_state::parentage::CommitRevision;
use meridian_core::workspace_state::timestamp::Timestamp;

use crate::workspace::WorkspaceReader;

/// A product repository could not be consulted (no such directory, not a
/// Git work tree, `git` failed). Never a domain verdict by itself: the
/// operation turns it into an explicit `UNVERIFIED`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryUnavailable(pub String);

impl std::fmt::Display for RepositoryUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for RepositoryUnavailable {}

/// Which committed state of a repository a file is read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevisionSelector<'a> {
    Head,
    Commit(&'a CommitRevision),
}

/// The local repositories of the inventory, addressed by the path the
/// inventory records for each (`RepositoryIdentity::path`), and the Kernel
/// itself, addressed by its root. One adapter answers every question about
/// every repository.
pub trait RepositoryAccess {
    /// The repository's current revision, ref and working-tree state.
    fn vcs_state(&self, root: &str) -> Result<ObservedVcs, RepositoryUnavailable>;

    /// Paths Git tracks, repository-relative with `/`, in Git's order.
    fn tracked_files(&self, root: &str) -> Result<Vec<String>, RepositoryUnavailable>;

    /// Untracked, not ignored paths, repository-relative with `/`.
    fn untracked_files(&self, root: &str) -> Result<Vec<String>, RepositoryUnavailable>;

    /// Those of `candidates` that exist in the tree and Git ignores.
    fn ignored_among(
        &self,
        root: &str,
        candidates: &[String],
    ) -> Result<Vec<String>, RepositoryUnavailable>;

    /// Whether the repository carries `revision` as a commit. `Ok(false)`
    /// is an answer (a shallow or partial clone lacks it), not a failure.
    fn has_revision(
        &self,
        root: &str,
        revision: &CommitRevision,
    ) -> Result<bool, RepositoryUnavailable>;

    /// The committed text of `path` at `at`; `Ok(None)` when that revision
    /// holds no such file.
    fn file_at(
        &self,
        root: &str,
        at: RevisionSelector<'_>,
        path: &WorkspaceRelativePath,
    ) -> Result<Option<String>, RepositoryUnavailable>;

    /// A reader bound to the repository root.
    fn reader(&self, root: &str) -> Box<dyn WorkspaceReader + '_>;
}

/// The current instant; read once per operation.
pub trait Clock {
    fn now(&self) -> Timestamp;
}
