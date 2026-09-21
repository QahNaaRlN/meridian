//! Stable process exit codes (`meridian-cli-rfc.md`, "Точный контракт CLI").
//!
//! Every command returns exactly one of these four codes. A command that
//! ran to completion and produced a well-formed result — even a negative
//! one, such as `validate` finding a `FAIL` or `doctor` finding an unhealthy
//! database — exits [`DOMAIN_NEGATIVE`], never [`USAGE`] or
//! [`INPUT_OR_ENVIRONMENT`]: those two are reserved for cases where no
//! command-specific result exists at all.

/// The command ran and its result is entirely positive (`validate` found no
/// `FAIL`, `doctor` found every checked component healthy, `resolve`
/// produced a resolver output, `export`/`init` completed).
pub const OK: i32 = 0;

/// The command ran to completion and produced a well-formed result, but that
/// result is negative: `validate` found at least one `FAIL`, or `doctor`
/// found at least one unhealthy component. This is a domain-level answer,
/// not a crash — the command's stdout still carries the full result.
pub const DOMAIN_NEGATIVE: i32 = 1;

/// The command line itself could not be understood: no subcommand, an
/// unknown subcommand, an unknown flag, a missing required flag, or an
/// unrecognised `--format` value. No command-specific result exists; stdout
/// is empty and the reason is on stderr.
pub const USAGE: i32 = 2;

/// The command line was understood, but the named input or environment made
/// it impossible to produce a result: a missing or unreadable Kernel or
/// workspace path, a missing, corrupt or role/edition-incompatible SQLite
/// database, or (for `resolve`) a request that is not well-formed JSON or
/// does not satisfy the rule-resolution transport contract. No
/// command-specific result exists; stdout is empty and the reason is on
/// stderr.
pub const INPUT_OR_ENVIRONMENT: i32 = 3;
