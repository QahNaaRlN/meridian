//! Strict domain types (`meridian-rust-target-architecture.md` §5).
//!
//! Every type here validates at its public constructor: a syntactically
//! plausible but semantically invalid value (an empty string where a
//! non-empty [`SemanticId`] is required, a filesystem path where an opaque
//! reference is required) cannot reach the rest of `meridian-core` through
//! it. Closed sets are represented as enums, not free-form strings, and
//! conditionally-required fields are expressed by which enum variant
//! carries them rather than by a runtime "field X requires field Y" check
//! layered on top of a flat struct.

mod authority;
mod content_digest;
mod diagnostic;
mod entry_name;
mod evidence_ref;
mod nonempty;
mod origin;
mod repository_id;
mod revision;
mod scope;
mod semantic_id;
mod verdict;
mod workspace_id;
mod workspace_relative_path;

pub use authority::{Authority, AuthorityError, AuthorityKind};
pub use content_digest::{ContentDigest, ContentDigestError, DigestAlgorithm};
pub use diagnostic::{Diagnostic, DiagnosticError, DiagnosticLevel, DiagnosticReference};
pub use entry_name::{EntryName, EntryNameError};
pub(crate) use evidence_ref::opaque_ref_fault;
pub use evidence_ref::{EvidenceRef, EvidenceRefError};
pub use nonempty::{NonEmptyString, NonEmptyStringError};
pub use origin::{Origin, OriginError, OriginKind};
pub use repository_id::{RepositoryId, RepositoryIdError};
pub use revision::{Revision, RevisionError};
pub use scope::{Scope, ScopeType};
pub use semantic_id::{SemanticId, SemanticIdError, BUILT_IN_METHODOLOGY_ID};
pub use verdict::Verdict;
pub use workspace_id::{WorkspaceId, WorkspaceIdError};
pub use workspace_relative_path::{WorkspaceRelativePath, WorkspaceRelativePathError};
