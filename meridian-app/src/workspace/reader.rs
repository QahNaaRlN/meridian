use meridian_core::types::{EntryName, WorkspaceRelativePath};

/// One directory entry's own type, as the directory lists it — never
/// resolved through a symlink. A symlink (or anything else the platform
/// does not classify as a plain file or directory) is [`EntryKind::Other`],
/// never silently promoted to [`EntryKind::File`] or [`EntryKind::Dir`]: a
/// caller that wants to treat a symlinked skill directory as a directory
/// has to say so explicitly, and none of the five 7a checks does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Dir,
    Other,
}

/// One entry of a directory listing: its validated bare name (no path,
/// never a separator/`.`/`..`/empty string — [`EntryName`]) and its own
/// [`EntryKind`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: EntryName,
    pub kind: EntryKind,
}

/// Why a [`WorkspaceReader`] operation did not return a value. Distinguishes
/// "nothing is there" from "something is there but could not be read" —
/// the two are never the same diagnostic in any of the five families this
/// port serves (`meridian-rust-migration-program-plan.md` §5.16.3, point 2).
/// A directory entry whose name is not valid UTF-8, or that otherwise fails
/// [`EntryName::new`], is an `Io` failure of the listing itself — never
/// silently lossy-converted and never folded into a domain-meaningful
/// "missing" outcome (corrective round,
/// `meridian-cli-foundation-architecture-remediation`, item 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    NotFound,
    Io(String),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::NotFound => write!(f, "not found"),
            ReadError::Io(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for ReadError {}

/// App-owned port: reads one verified workspace root, one validated
/// relative path at a time. The minimal, actually-used surface for the five
/// `validate-mechanical-integrity` checks — text, bytes, and a non-recursive
/// directory listing that never follows a symlink to decide an entry's own
/// kind.
pub trait WorkspaceReader {
    fn read_text(&self, path: &WorkspaceRelativePath) -> Result<String, ReadError>;
    fn read_bytes(&self, path: &WorkspaceRelativePath) -> Result<Vec<u8>, ReadError>;
    fn list_dir(&self, path: &WorkspaceRelativePath) -> Result<Vec<DirEntry>, ReadError>;
}
