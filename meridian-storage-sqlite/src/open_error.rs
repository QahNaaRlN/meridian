//! [`OpenError`] — the closed set of ways opening a SQLite-backed
//! [`crate::SqliteStorage`] can fail.
//!
//! `meridian-rust-target-architecture.md` §4.2 requires that a corrupt file,
//! an unrecognised schema version, or a missing schema-version record each
//! give an explicit, typed error — never a silent empty state and never an
//! automatic recreation of the file. `meridian-rust-migration-program-plan.md`
//! §5.4, item 3 extends the same discipline to an unrecognised database
//! role, a metadata mismatch on reopen, and an existing record whose scope
//! is incompatible with the role a v1→v2 migration would assign.

use core::fmt;

use meridian_app::storage::{DatabaseMetadata, DatabaseRole};

/// A value returned when opening or preparing a SQLite database fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenError {
    /// The file exists but is not a valid SQLite database (for example: a
    /// truncated, foreign-format, or otherwise corrupted file). The database
    /// is left untouched — never silently recreated.
    CorruptDatabase(String),
    /// `schema_migrations` carries a version this build does not recognise
    /// — newer, or otherwise unsupported (older-but-not-in the migration
    /// ladder). Opening stops rather than guessing how to interpret an
    /// unfamiliar schema.
    UnsupportedSchemaVersion { found: i64 },
    /// `schema_migrations` exists but carries no version at all (an empty
    /// table) — a defect in how the database was prepared, not a legal
    /// "no version yet" state.
    MissingSchemaVersion,
    /// The database is at the current schema version, but its
    /// `database_metadata` table carries no row — a defect in how the
    /// database was prepared or migrated, not a legal "no metadata yet"
    /// state (a fresh bootstrap and a v1→v2 migration both write this row
    /// in the same transaction that establishes the version).
    MissingDatabaseMetadata,
    /// `database_metadata.role` carries a value this build does not
    /// recognise as `tool` or `workspace`. Never silently treated as
    /// either — a database whose role cannot be read is a database this
    /// build cannot safely write to at all.
    UnknownDatabaseRole { found: String },
    /// The caller asserted [`DatabaseMetadata`] — role AND Kernel edition —
    /// that does not match what is already recorded inside the database.
    /// Recorded metadata is authoritative and never silently overwritten or
    /// silently accepted as a mismatch on either field: until a separate
    /// edition-compatibility rule is defined, an exact match is required for
    /// both `role` and `kernel_edition`, for either role.
    DatabaseMetadataMismatch {
        expected: DatabaseMetadata,
        found: DatabaseMetadata,
    },
    /// A v1→v2 migration would assign a role to this database, but at least
    /// one existing `records.scope_type` value carries a well-formed scope
    /// type that role does not accept (for example a `tool` role over a
    /// database already holding `project-workspace` records). The migration
    /// stops before writing `database_metadata` or advancing the schema
    /// version — it never assigns a role a database's own existing content
    /// already contradicts.
    MigrationRoleIncompatibleWithExistingRecords {
        role: DatabaseRole,
        scope_type: String,
    },
    /// A v1→v2 migration found a `records.scope_type` value that is not one
    /// of the six known areas — unrecognised, not merely incompatible with
    /// the assigned role. The migration stops rather than guessing whether
    /// an unfamiliar area would have been allowed.
    UnrecognizedScopeTypeInExistingRecords { found: String },
    /// An unexpected failure from the SQLite driver, not covered by the more
    /// specific variants above. Carries a message only, never a driver type.
    Sqlite(String),
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenError::CorruptDatabase(message) => {
                write!(
                    f,
                    "the database file is corrupt or not a SQLite database: {message}"
                )
            }
            OpenError::UnsupportedSchemaVersion { found } => write!(
                f,
                "schema_migrations reports version {found}, which this build does not support"
            ),
            OpenError::MissingSchemaVersion => {
                write!(f, "schema_migrations exists but records no version")
            }
            OpenError::MissingDatabaseMetadata => {
                write!(f, "database_metadata exists but records no row")
            }
            OpenError::UnknownDatabaseRole { found } => write!(
                f,
                "database_metadata.role is \"{found}\", which this build does not recognise as tool or workspace"
            ),
            OpenError::DatabaseMetadataMismatch { expected, found } => write!(
                f,
                "expected role \"{}\" at Kernel edition \"{}\", but this database's own metadata records role \"{}\" at Kernel edition \"{}\"",
                expected.role(),
                expected.kernel_edition(),
                found.role(),
                found.kernel_edition(),
            ),
            OpenError::MigrationRoleIncompatibleWithExistingRecords { role, scope_type } => {
                write!(
                    f,
                    "migrating to role \"{role}\" is refused: an existing record already carries scope type \"{scope_type}\", which that role does not accept"
                )
            }
            OpenError::UnrecognizedScopeTypeInExistingRecords { found } => write!(
                f,
                "an existing record carries scope_type \"{found}\", which this build does not recognise as one of the six known areas"
            ),
            OpenError::Sqlite(message) => write!(f, "sqlite error: {message}"),
        }
    }
}

impl std::error::Error for OpenError {}
