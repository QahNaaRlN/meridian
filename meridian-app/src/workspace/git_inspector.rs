use meridian_core::types::WorkspaceRelativePath;

/// Why a [`GitInspector::tracked_files`] call did not return a tracked-path
/// list. Distinguishes "Git itself could not be consulted" (no repository,
/// `git` missing from `PATH`, a non-zero exit) from "Git answered, but one
/// of its own reported paths is not a valid [`WorkspaceRelativePath`]" — the
/// two are never folded into the same outcome: a caller that gets
/// `Unavailable` falls back to an explicit filesystem walk with an explicit
/// warning (the existing `meridian validate` contract), while
/// `InvalidPath` names a genuine data problem in Git's own output that a
/// silent filesystem fallback would hide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitInspectorError {
    Unavailable,
    InvalidPath { raw: String, reason: String },
}

impl std::fmt::Display for GitInspectorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitInspectorError::Unavailable => write!(f, "git is unavailable"),
            GitInspectorError::InvalidPath { raw, reason } => {
                write!(
                    f,
                    "git reported an invalid tracked path \"{raw}\": {reason}"
                )
            }
        }
    }
}

impl std::error::Error for GitInspectorError {}

/// App-owned port: the set of paths Git considers tracked inside one
/// workspace root, as validated [`WorkspaceRelativePath`] values. The one
/// production implementation spawns real `git` plumbing
/// (`meridian_cli::adapters::git_inspector::RealGitInspector`); a fake
/// implementation drives this crate's own orchestration tests.
pub trait GitInspector {
    fn tracked_files(&self) -> Result<Vec<WorkspaceRelativePath>, GitInspectorError>;
}
