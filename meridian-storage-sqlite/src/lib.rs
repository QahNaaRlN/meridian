#![forbid(unsafe_code)]

//! Meridian SQLite storage adapter.
//!
//! Package `rust-workspace-foundation` only proves, in `#[cfg(test)]`, that
//! this crate builds, links against SQLite through `rusqlite`, and can open
//! a working connection. It carries no production adapter API, no schema and
//! no `CREATE TABLE`. The schema, transactional writes, the
//! `RecordRepository`/`EvidenceRepository` port implementations and the
//! invariants of `meridian-rust-target-architecture.md` §4 are added by
//! package `sqlite-storage-adapter`.

/// Identifies this crate in composition-root diagnostics until the real
/// storage adapter exists.
pub const CRATE_NAME: &str = "meridian-storage-sqlite";

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name_is_stable() {
        assert_eq!(super::CRATE_NAME, "meridian-storage-sqlite");
    }

    /// Proves the chosen SQLite driver (`rusqlite`, `bundled`) compiles,
    /// links, and opens and queries a working connection. No schema, no
    /// `CREATE TABLE`, no adapter: those belong to package
    /// `sqlite-storage-adapter`.
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
