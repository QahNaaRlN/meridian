#![forbid(unsafe_code)]

//! Meridian SQLite storage adapter.
//!
//! The only crate that knows SQLite exists (`meridian-rust-sqlite-architecture.md`
//! §"Принятое решение" 4): it implements
//! [`meridian_app::storage::RecordRepository`] and
//! [`meridian_app::storage::EvidenceRepository`] over ten tables
//! (`meridian-rust-target-architecture.md` §4.1), owning the connection,
//! schema, schema-migration ladder, database-role guard, transactions,
//! backup and row⇄domain-type translation. It carries no CLI, no workspace
//! file traversal, no Git logic, no environment reads and no product data
//! in the `tool`-role database.
//!
//! Package `sqlite-storage-adapter`
//! (`meridian-rust-migration-program-plan.md` §4) established this crate's
//! original content; corrective package `knowledge-agent-foundation` (§5.4)
//! adds database roles, schema migration beyond version 1, and the
//! role-boundary guard on every write. Package `meridian-cli-migration` adds
//! schema version 3 (the migration-run journal: separate append-only
//! `migration_runs` and `migration_rollbacks` facts) and the
//! `MigrationRepository` implementation (`migration`): one transaction per
//! apply, checkpoints through SQLite's online backup API, and a verified
//! restore on rollback.

mod codec;
mod migration;
mod open_error;
mod schema;
mod storage;

pub use open_error::OpenError;
pub use storage::SqliteStorage;

/// Names of the ten tables a fully-migrated database carries
/// (`meridian-rust-target-architecture.md` §4.1; `database_metadata` added
/// by `meridian-rust-migration-program-plan.md` §5.4, `migration_rollbacks`
/// by §5.22.5) — re-exported for
/// tests and diagnostics that want to check schema completeness without
/// duplicating the list.
pub use schema::TABLE_NAMES;

/// Identifies this crate in composition-root diagnostics.
pub const CRATE_NAME: &str = "meridian-storage-sqlite";

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_is_stable() {
        assert_eq!(super::CRATE_NAME, "meridian-storage-sqlite");
    }

    /// Proves the chosen SQLite driver (`rusqlite`, `bundled`) still
    /// compiles, links, and opens and queries a working connection —
    /// carried over from package `rust-workspace-foundation`, now
    /// alongside the real adapter it originally reserved room for.
    #[test]
    fn sqlite_driver_compiles_links_and_opens_a_connection() {
        let conn =
            rusqlite::Connection::open_in_memory().expect("open in-memory sqlite connection");
        let answer: i64 = conn
            .query_row("SELECT 1", [], |row| row.get(0))
            .expect("query through the sqlite driver");
        assert_eq!(answer, 1);
    }
}
