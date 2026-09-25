use meridian_core::types::WorkspaceRelativePath;

/// Why a canonical link's filesystem target failed
/// [`LinkTargetPort::check`] — the four states `checkKernelLinkTarget`'s
/// filesystem half distinguishes, and only those: whether the path is
/// tracked by Git is a SEPARATE question ([`super::GitInspector`]), not
/// part of this port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTargetError {
    /// The target does not exist.
    Missing,
    /// The target exists but could not be inspected (permission denied, a
    /// non-UTF-8 symlink target on some platforms, and so on).
    Io(String),
    /// The target exists but is not a regular file (a directory, a FIFO,
    /// ...).
    NotRegularFile,
    /// The target resolves outside the workspace root once symbolic links
    /// are followed.
    SymlinkEscape,
}

impl std::fmt::Display for LinkTargetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkTargetError::Missing => write!(f, "the target does not exist"),
            LinkTargetError::Io(message) => write!(f, "the target cannot be inspected: {message}"),
            LinkTargetError::NotRegularFile => write!(f, "the target is not a regular file"),
            LinkTargetError::SymlinkEscape => write!(
                f,
                "the target resolves outside the workspace once symbolic links are followed"
            ),
        }
    }
}

impl std::error::Error for LinkTargetError {}

/// App-owned companion port to [`super::WorkspaceReader`]: resolves a
/// canonical link's `path` against the real filesystem, answering exactly
/// the question a text-only portability check cannot — does this path,
/// once symbolic links are followed, land on a real regular file that
/// stays inside the workspace root. The one production implementation
/// canonicalizes against the real filesystem
/// (`meridian_cli::adapters::link_target::FsLinkTarget`); a fake
/// implementation drives this crate's own orchestration tests.
pub trait LinkTargetPort {
    fn check(&self, path: &WorkspaceRelativePath) -> Result<(), LinkTargetError>;
}
