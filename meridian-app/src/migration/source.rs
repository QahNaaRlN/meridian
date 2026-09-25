//! [`FrozenSource`] — the one port through which a frozen-Instance import
//! reads its source (`meridian-rust-migration-program-plan.md` §5.22.4,
//! items 6–7).
//!
//! One adapter instance is ONE source view: the accepted bundle is read
//! from a single commit resolved once ([`FrozenSource::bundle_revision`]),
//! and the pinned source content only through the tree listing of the
//! pinned revision itself. Nothing is ever read from a working tree, and
//! no two states are mixed. The tree digest is the adapter's own
//! recomputation over that same listing.

use core::fmt;

use meridian_core::types::ContentDigest;

/// The closed set of files an accepted `migration/instance-data` bundle
/// consists of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleFile {
    Registry,
    CanonicalExport,
    SourceSnapshot,
    RollbackSnapshot,
    DeterministicPlan,
    CoverageEvidence,
    ApplicabilityEvidence,
}

impl BundleFile {
    pub const ALL: [BundleFile; 7] = [
        BundleFile::Registry,
        BundleFile::CanonicalExport,
        BundleFile::SourceSnapshot,
        BundleFile::RollbackSnapshot,
        BundleFile::DeterministicPlan,
        BundleFile::CoverageEvidence,
        BundleFile::ApplicabilityEvidence,
    ];

    /// The file's path inside the source repository.
    pub fn path(self) -> &'static str {
        match self {
            BundleFile::Registry => "migration/instance-data/registry.json",
            BundleFile::CanonicalExport => "migration/instance-data/canonical-export.json",
            BundleFile::SourceSnapshot => "migration/instance-data/source-snapshot.json",
            BundleFile::RollbackSnapshot => "migration/instance-data/rollback-snapshot.json",
            BundleFile::DeterministicPlan => {
                "migration/instance-data/deterministic-reconstruction-plan.json"
            }
            BundleFile::CoverageEvidence => "migration/instance-data/evidence/coverage.json",
            BundleFile::ApplicabilityEvidence => {
                "migration/instance-data/evidence/applicability-preservation.json"
            }
        }
    }
}

/// The pinned revision's tree as the adapter listed it once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedTree {
    /// The recomputed tree digest (`source-snapshot.json`'s algorithm).
    pub digest: ContentDigest,
    /// Every tracked path, sorted.
    pub paths: Vec<String>,
}

/// Why the source could not be read. Each variant is an input/environment
/// failure, never a domain verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceError {
    /// No usable `git` executable.
    GitUnavailable(String),
    /// The named path is not a Git repository.
    NotARepository(String),
    /// The revision does not exist in the repository.
    MissingRevision { revision: String },
    /// The file does not exist at that revision.
    MissingFile { revision: String, path: String },
    /// The listing or a file could not be read or decoded.
    Unreadable(String),
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourceError::GitUnavailable(message) => write!(f, "git is not available: {message}"),
            SourceError::NotARepository(path) => {
                write!(f, "{path} is not a Git repository")
            }
            SourceError::MissingRevision { revision } => {
                write!(
                    f,
                    "revision {revision} does not exist in the source repository"
                )
            }
            SourceError::MissingFile { revision, path } => {
                write!(f, "{path} does not exist at revision {revision}")
            }
            SourceError::Unreadable(message) => {
                write!(f, "the source could not be read: {message}")
            }
        }
    }
}

impl std::error::Error for SourceError {}

/// One frozen source view.
pub trait FrozenSource {
    /// The single commit every bundle file is read from.
    fn bundle_revision(&self) -> &str;

    /// One bundle file's exact bytes at [`Self::bundle_revision`].
    fn bundle_file(&self, file: BundleFile) -> Result<Vec<u8>, SourceError>;

    /// The tree listing and recomputed digest of `revision`.
    fn pinned_tree(&self, revision: &str) -> Result<PinnedTree, SourceError>;

    /// One tracked file's exact bytes at `revision`, from the same listing
    /// [`Self::pinned_tree`] returned.
    fn file_at(&self, revision: &str, path: &str) -> Result<Vec<u8>, SourceError>;
}
