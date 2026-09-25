//! Argument grammar shared by every command (`meridian-cli-rfc.md`, "Точный
//! контракт CLI").
//!
//! Grammar: `meridian <command> [--flag value]...`, and for the nested
//! `migration` command `meridian migration <plan|apply|verify|rollback>
//! [--flag value]...`. Every flag takes exactly
//! one value, except a command's declared switches (`validate
//! --log-metrics`), which take none.
//! `--help`/`-h`, alone or as the first token, and the bare command `help`
//! print usage and exit [`crate::exit_code::OK`] without touching stdin,
//! a Kernel path or a database. Anything else that cannot be parsed against
//! a command's own allowed flags is a usage error
//! ([`crate::exit_code::USAGE`]): an unknown command, an unknown flag, a
//! flag missing its value, or a flag repeated (the last occurrence is not
//! silently preferred — a repeat is rejected outright, so two conflicting
//! values can never be resolved by silent precedence).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Json,
}

impl OutputFormat {
    pub const DEFAULT: OutputFormat = OutputFormat::Human;

    fn parse(value: &str) -> Result<Self, CliError> {
        match value {
            "human" => Ok(OutputFormat::Human),
            "json" => Ok(OutputFormat::Json),
            other => Err(CliError::InvalidFormat(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    NoCommand,
    UnknownCommand(String),
    UnknownFlag {
        command: &'static str,
        flag: String,
    },
    FlagMissingValue {
        command: &'static str,
        flag: String,
    },
    FlagRepeated {
        command: &'static str,
        flag: String,
    },
    MissingRequiredFlag {
        command: &'static str,
        flag: &'static str,
    },
    InvalidFormat(String),
    /// A flag whose value belongs to a closed set (`--kind`, `--dry-run`,
    /// `--run`) got a value outside it — never a truthy string or fallback.
    InvalidFlagValue {
        command: &'static str,
        flag: &'static str,
        value: String,
        expected: &'static str,
    },
    /// A flag the chosen form of a command does not accept (for example
    /// `--input` with `--kind frozen-instance`).
    FlagNotAllowed {
        command: &'static str,
        flag: &'static str,
        reason: &'static str,
    },
    /// `migration` without a subcommand, or with an unknown one.
    UnknownSubcommand {
        command: &'static str,
        subcommand: Option<String>,
    },
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::NoCommand => write!(f, "no command given (try --help)"),
            CliError::UnknownCommand(name) => write!(f, "unknown command: \"{name}\""),
            CliError::UnknownFlag { command, flag } => {
                write!(f, "unknown flag for \"{command}\": \"{flag}\"")
            }
            CliError::FlagMissingValue { command, flag } => {
                write!(f, "flag \"{flag}\" for \"{command}\" requires a value")
            }
            CliError::FlagRepeated { command, flag } => {
                write!(
                    f,
                    "flag \"{flag}\" for \"{command}\" was given more than once"
                )
            }
            CliError::MissingRequiredFlag { command, flag } => {
                write!(f, "\"{command}\" requires --{flag}")
            }
            CliError::InvalidFormat(value) => write!(
                f,
                "invalid --format value \"{value}\" (expected \"human\" or \"json\")"
            ),
            CliError::InvalidFlagValue {
                command,
                flag,
                value,
                expected,
            } => write!(
                f,
                "invalid --{flag} value \"{value}\" for \"{command}\" (expected {expected})"
            ),
            CliError::FlagNotAllowed {
                command,
                flag,
                reason,
            } => write!(f, "\"{command}\" does not accept --{flag} {reason}"),
            CliError::UnknownSubcommand {
                command,
                subcommand: None,
            } => write!(
                f,
                "\"{command}\" requires a subcommand (plan, apply, verify or rollback)"
            ),
            CliError::UnknownSubcommand {
                command,
                subcommand: Some(name),
            } => write!(
                f,
                "unknown subcommand \"{command} {name}\" (expected plan, apply, verify or rollback)"
            ),
        }
    }
}

/// Parsed `--flag value` pairs for one command invocation, plus the shared
/// `--format`.
pub struct ParsedArgs {
    pub format: OutputFormat,
    flags: BTreeMap<String, String>,
    switches: BTreeSet<String>,
}

impl ParsedArgs {
    /// Whether the value-less switch `--<name>` was given.
    pub fn switch(&self, name: &str) -> bool {
        self.switches.contains(name)
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.flags.get(name).map(String::as_str)
    }

    pub fn require<'a>(
        &'a self,
        command: &'static str,
        name: &'static str,
    ) -> Result<&'a str, CliError> {
        self.get(name).ok_or(CliError::MissingRequiredFlag {
            command,
            flag: name,
        })
    }
}

/// The closed kind of an `import` input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportKind {
    FrozenInstance,
    CanonicalRecords,
}

impl ImportKind {
    pub fn parse(command: &'static str, value: &str) -> Result<ImportKind, CliError> {
        match value {
            "frozen-instance" => Ok(ImportKind::FrozenInstance),
            "canonical-records" => Ok(ImportKind::CanonicalRecords),
            other => Err(CliError::InvalidFlagValue {
                command,
                flag: "kind",
                value: other.to_string(),
                expected: "\"frozen-instance\" or \"canonical-records\"",
            }),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ImportKind::FrozenInstance => "frozen-instance",
            ImportKind::CanonicalRecords => "canonical-records",
        }
    }
}

/// `--dry-run`: exactly `true` or `false`; absent means `true`.
pub fn parse_dry_run(command: &'static str, value: Option<&str>) -> Result<bool, CliError> {
    match value {
        None | Some("true") => Ok(true),
        Some("false") => Ok(false),
        Some(other) => Err(CliError::InvalidFlagValue {
            command,
            flag: "dry-run",
            value: other.to_string(),
            expected: "\"true\" or \"false\"",
        }),
    }
}

/// Parses `rest` (every token after the command name) against `allowed_flags`
/// (which must already include `"format"` if the command accepts it — every
/// implemented command does). `command` names the command for diagnostics.
pub fn parse_flags(
    command: &'static str,
    rest: &[String],
    allowed_flags: &[&str],
) -> Result<ParsedArgs, CliError> {
    parse_flags_with_switches(command, rest, allowed_flags, &[])
}

/// [`parse_flags`] for a command that also declares value-less switches;
/// a repeated switch is rejected like a repeated flag.
pub fn parse_flags_with_switches(
    command: &'static str,
    rest: &[String],
    allowed_flags: &[&str],
    allowed_switches: &[&str],
) -> Result<ParsedArgs, CliError> {
    let mut flags = BTreeMap::new();
    let mut switches = BTreeSet::new();
    let mut iter = rest.iter();
    while let Some(token) = iter.next() {
        let Some(name) = token.strip_prefix("--") else {
            return Err(CliError::UnknownFlag {
                command,
                flag: token.clone(),
            });
        };
        if allowed_switches.contains(&name) {
            if !switches.insert(name.to_string()) {
                return Err(CliError::FlagRepeated {
                    command,
                    flag: name.to_string(),
                });
            }
            continue;
        }
        if !allowed_flags.contains(&name) {
            return Err(CliError::UnknownFlag {
                command,
                flag: name.to_string(),
            });
        }
        let value = iter.next().ok_or_else(|| CliError::FlagMissingValue {
            command,
            flag: name.to_string(),
        })?;
        if flags.insert(name.to_string(), value.clone()).is_some() {
            return Err(CliError::FlagRepeated {
                command,
                flag: name.to_string(),
            });
        }
    }
    let format = match flags.remove("format") {
        Some(value) => OutputFormat::parse(&value)?,
        None => OutputFormat::DEFAULT,
    };
    Ok(ParsedArgs {
        format,
        flags,
        switches,
    })
}

pub const USAGE: &str = "\
meridian — Meridian Rust CLI (packages meridian-cli-foundation, meridian-cli-migration)

USAGE:
    meridian <command> [--flag value]...

COMMANDS:
    init      --kernel <path> --workspace <path> --tool-db <path> --workspace-db <path> [--format human|json]
    doctor    --kernel <path> --workspace <path> --tool-db <path> --workspace-db <path> [--format human|json]
    validate  --kernel <path> [--workspace-db <path>] [--log-metrics] [--format human|json]
    resolve   --kernel <path> [--request <path>] [--format human|json]
    export    --kernel <path> --tool-db <path> --workspace-db <path> [--format human|json]
    import    --kernel <path> --kind frozen-instance --source <instance-repository-path> --tool-db <path> --workspace-db <path> --confirm <plan-fingerprint> [--format human|json]
    import    --kernel <path> --kind canonical-records --input <json-path> --tool-db <path> --workspace-db <path> --confirm <sha256-of-input> [--format human|json]
    migration plan     --kernel <path> --source <instance-repository-path> [--format human|json]
    migration apply    --kernel <path> --source <instance-repository-path> --workspace-db <path> [--dry-run true|false] [--confirm <plan-fingerprint>] [--format human|json]
    migration verify   --kernel <path> --source <instance-repository-path> --workspace-db <path> [--format human|json]
    migration rollback --kernel <path> --source <instance-repository-path> --workspace-db <path> --run <migration-run-id> --confirm <migration-run-id> [--format human|json]

    --format defaults to \"human\" for every command.
    --request for \"resolve\" defaults to reading the request document from stdin.
    --dry-run defaults to \"true\"; only \"--dry-run false --confirm <recomputed plan
    fingerprint>\" changes state. A present confirmation must match even for a dry run.
    import changes state only when --confirm matches: the recomputed plan fingerprint
    (frozen-instance) or the SHA-256 of the exact input bytes (canonical-records).
    rollback requires --confirm to repeat --run exactly.
    The migration source is named only by --source; no command reads MERIDIAN_INSTANCE.

    -h, --help    print this message and exit 0

EXIT CODES:
    0   success — the command's result is entirely positive
    1   the command ran and produced a result, but that result is negative
        (validate found a FAIL, doctor found an unhealthy component, a
        migration bundle was rejected, a confirmation did not match, a
        rollback was refused, verification did not pass)
    2   usage error — the command line itself could not be understood
    3   the named input or environment made it impossible to produce a
        result (missing/incompatible Kernel, workspace or database; a
        malformed resolve request; an unreadable migration source; a
        malformed canonical-records input; a missing or damaged checkpoint)

Result is always written to stdout only; diagnostics, warnings and errors are
always written to stderr only, regardless of --format.
";
