//! [`PortError`] — the one error type both storage ports return.
//!
//! Carries no dependency on any adapter's driver (no `rusqlite::Error`, no
//! SQL error code): an adapter maps its own failures into this closed,
//! adapter-neutral shape at the port boundary, the same discipline
//! `meridian-core`'s own types apply to their constructors.

use core::fmt;

use meridian_core::types::ScopeType;

use super::database_role::DatabaseRole;

/// A value returned by [`super::RecordRepository`] or
/// [`super::EvidenceRepository`] when an operation does not succeed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortError {
    /// The same [`super::IdempotencyKey`] was already applied for content
    /// whose digest differs from the one requested now — a genuine conflict,
    /// never silently treated as a successful repeat.
    IdempotencyConflict {
        idempotency_key: String,
        existing_content_digest: String,
        requested_content_digest: String,
    },
    /// A value the request referenced (for example the subject record of a
    /// piece of evidence) does not exist in storage.
    ReferenceNotFound { reference: String },
    /// The evidence reference was already recorded. Evidence is write-once:
    /// no caller is trusted to change or replace an already-stored entry,
    /// so a second `put` for the same [`meridian_core::types::EvidenceRef`]
    /// is rejected unconditionally, whether or not its content matches the
    /// first.
    EvidenceAlreadyExists { evidence_ref: String },
    /// A record's [`meridian_core::types::Scope`] is not one this
    /// database's [`DatabaseRole`] accepts — a `tool` database rejects
    /// every scope but `built-in-methodology`, and a `workspace` database
    /// rejects `built-in-methodology`
    /// (`meridian-rust-migration-program-plan.md` §5.4, item 1). Rejected
    /// at the storage boundary itself, not only by whichever composition
    /// chose which adapter to call — a direct call against the wrong-role
    /// adapter is refused just the same.
    ScopeNotAllowedForDatabaseRole {
        role: DatabaseRole,
        scope_type: ScopeType,
    },
    /// A batch mixed a `built-in-methodology` request with a non-`built-in-methodology`
    /// request. A batch is one atomic unit inside one database transaction
    /// (`super::RecordRepository::put_batch`); splitting it silently across
    /// the `tool` and `workspace` databases would fake atomicity that does
    /// not exist, so a mixed-role batch is refused instead of split.
    MixedDatabaseRolesInBatch,
    /// The underlying storage failed for a reason not covered by the more
    /// specific variants above (I/O, a corrupt file, an unexpected driver
    /// error). Carries a human-readable message only — never a driver type.
    Storage(String),
}

impl fmt::Display for PortError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PortError::IdempotencyConflict {
                idempotency_key,
                existing_content_digest,
                requested_content_digest,
            } => write!(
                f,
                "idempotency key \"{idempotency_key}\" was already applied for different content (existing digest {existing_content_digest}, requested digest {requested_content_digest})"
            ),
            PortError::ReferenceNotFound { reference } => {
                write!(f, "referenced entity \"{reference}\" does not exist")
            }
            PortError::EvidenceAlreadyExists { evidence_ref } => write!(
                f,
                "evidence \"{evidence_ref}\" is already recorded and cannot be replaced"
            ),
            PortError::ScopeNotAllowedForDatabaseRole { role, scope_type } => write!(
                f,
                "scope type \"{scope_type}\" is not allowed in a \"{role}\" database"
            ),
            PortError::MixedDatabaseRolesInBatch => write!(
                f,
                "a batch cannot mix a built-in-methodology request with a non-built-in-methodology request"
            ),
            PortError::Storage(message) => write!(f, "storage error: {message}"),
        }
    }
}

impl std::error::Error for PortError {}
