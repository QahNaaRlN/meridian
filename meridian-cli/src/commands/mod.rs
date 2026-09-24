//! The five commands of package `meridian-cli-foundation` and the `import`
//! and nested `migration plan|apply|verify|rollback` commands of package
//! `meridian-cli-migration`. Every
//! command function has the same shape: it takes already-parsed flags, a
//! writer for stdout, a writer for stderr and an `EventSink`, and returns
//! a stable [`crate::exit_code`]. Nothing here calls `std::process::exit` or
//! reads `std::env` directly — that belongs to [`crate::run`] alone, so a
//! test can call any command function in-process with in-memory buffers.

pub mod doctor;
pub mod export;
pub mod import;
pub mod init;
pub mod migration;
pub mod resolve;
pub mod validate;

use std::path::Path;

use meridian_app::storage::DatabaseMetadata;
use meridian_storage_sqlite::{OpenError, SqliteStorage};

/// A database this command tried to open for reading or writing, before any
/// [`OpenError`] this crate's dependency can report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbOpenDiagnostic {
    /// No file exists at the named path. Reported explicitly rather than
    /// letting `SqliteStorage::open_path` create one — a read-only command
    /// (`doctor`, `export`) never brings a database into existence as a side
    /// effect of merely checking or reading it.
    Missing,
    Open(OpenError),
}

impl std::fmt::Display for DbOpenDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DbOpenDiagnostic::Missing => write!(f, "no database file exists at this path"),
            DbOpenDiagnostic::Open(error) => write!(f, "{error}"),
        }
    }
}

/// Opens an already-existing database file at `path`, asserting `expected`
/// metadata. Never creates a file: a missing path is reported as
/// [`DbOpenDiagnostic::Missing`], not bootstrapped — the read-only commands
/// that call this (`doctor`, `export`) must never change state.
pub fn open_existing_db(
    path: &Path,
    expected: DatabaseMetadata,
) -> Result<SqliteStorage, DbOpenDiagnostic> {
    if !path.exists() {
        return Err(DbOpenDiagnostic::Missing);
    }
    SqliteStorage::open_path(path, expected).map_err(DbOpenDiagnostic::Open)
}
