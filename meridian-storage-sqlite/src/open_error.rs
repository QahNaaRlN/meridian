//! [`OpenError`] — the closed set of ways opening a SQLite-backed
//! [`crate::SqliteStorage`] can fail.
//!
//! `meridian-rust-target-architecture.md` §4.2 requires that a corrupt file,
//! an unrecognised schema version, or a missing schema-version record each
//! give an explicit, typed error — never a silent empty state and never an
//! automatic recreation of the file.

use core::fmt;

/// A value returned when opening or preparing a SQLite database fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenError {
    /// The file exists but is not a valid SQLite database (for example: a
    /// truncated, foreign-format, or otherwise corrupted file). The database
    /// is left untouched — never silently recreated.
    CorruptDatabase(String),
    /// `schema_migrations` carries a version this build does not recognise
    /// — newer, older-but-skipped, or otherwise unsupported. Opening stops
    /// rather than guessing how to interpret an unfamiliar schema.
    UnsupportedSchemaVersion { found: i64 },
    /// `schema_migrations` exists but carries no version at all (an empty
    /// table) — a defect in how the database was prepared, not a legal
    /// "no version yet" state.
    MissingSchemaVersion,
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
            OpenError::Sqlite(message) => write!(f, "sqlite error: {message}"),
        }
    }
}

impl std::error::Error for OpenError {}
