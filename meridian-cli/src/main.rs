#![forbid(unsafe_code)]

//! Meridian CLI — composition root binary.
//!
//! Package `rust-workspace-foundation` proves only that the binary builds,
//! links against `meridian-app` and `meridian-storage-sqlite`, and runs to
//! completion. It carries no subcommands, argument parsing, or domain
//! behaviour: those belong to package `meridian-cli-foundation` and later.

fn main() {
    println!(
        "meridian-cli workspace foundation: {} + {} (no commands implemented yet)",
        meridian_app::CRATE_NAME,
        meridian_storage_sqlite::CRATE_NAME
    );
}
