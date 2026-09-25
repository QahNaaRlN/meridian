#![forbid(unsafe_code)]

//! Meridian CLI — composition root binary. Thin wrapper around
//! [`meridian_cli::run`]: this is the only place that reads real
//! `std::env::args`, touches real stdio, or calls `std::process::exit`. The
//! production binary always composes with [`meridian_app::events::NoOpEventSink`]
//! — this package introduces no consumer of observed events and no flag to
//! name a different one (`meridian_cli::events`).

use std::io;

use meridian_app::events::NoOpEventSink;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut stderr = io::stderr();
    let code = meridian_cli::run(&args, &mut stdin, &mut stdout, &mut stderr, &NoOpEventSink);
    std::process::exit(code);
}
