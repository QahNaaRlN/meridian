//! Argument grammar shared by every command (`meridian-cli-rfc.md`, "Точный
//! контракт CLI").
//!
//! Grammar: `meridian <command> [--flag value]...`. Every flag takes exactly
//! one value — there are no boolean switches in this package's surface.
//! `--help`/`-h`, alone or as the first token, and the bare command `help`
//! print usage and exit [`crate::exit_code::OK`] without touching stdin,
//! a Kernel path or a database. Anything else that cannot be parsed against
//! a command's own allowed flags is a usage error
//! ([`crate::exit_code::USAGE`]): an unknown command, an unknown flag, a
//! flag missing its value, or a flag repeated (the last occurrence is not
//! silently preferred — a repeat is rejected outright, so two conflicting
//! values can never be resolved by silent precedence).

use std::collections::BTreeMap;
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
        }
    }
}

/// Parsed `--flag value` pairs for one command invocation, plus the shared
/// `--format`.
pub struct ParsedArgs {
    pub format: OutputFormat,
    flags: BTreeMap<String, String>,
}

impl ParsedArgs {
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

/// Parses `rest` (every token after the command name) against `allowed_flags`
/// (which must already include `"format"` if the command accepts it — every
/// implemented command does). `command` names the command for diagnostics.
pub fn parse_flags(
    command: &'static str,
    rest: &[String],
    allowed_flags: &[&str],
) -> Result<ParsedArgs, CliError> {
    let mut flags = BTreeMap::new();
    let mut iter = rest.iter();
    while let Some(token) = iter.next() {
        let Some(name) = token.strip_prefix("--") else {
            return Err(CliError::UnknownFlag {
                command,
                flag: token.clone(),
            });
        };
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
    Ok(ParsedArgs { format, flags })
}

pub const USAGE: &str = "\
meridian — Meridian Rust CLI (package meridian-cli-foundation)

USAGE:
    meridian <command> [--flag value]...

COMMANDS:
    init      --kernel <path> --workspace <path> --tool-db <path> --workspace-db <path> [--format human|json]
    doctor    --kernel <path> --workspace <path> --tool-db <path> --workspace-db <path> [--format human|json]
    validate  --kernel <path> [--format human|json]
    resolve   --kernel <path> [--request <path>] [--format human|json]
    export    --kernel <path> --tool-db <path> --workspace-db <path> [--format human|json]

    --format defaults to \"human\" for every command.
    --request for \"resolve\" defaults to reading the request document from stdin.

    -h, --help    print this message and exit 0

EXIT CODES:
    0   success — the command's result is entirely positive
    1   the command ran and produced a result, but that result is negative
        (validate found a FAIL, doctor found an unhealthy component)
    2   usage error — the command line itself could not be understood
    3   the named input or environment made it impossible to produce a
        result (missing/incompatible Kernel, workspace or database; a
        malformed resolve request)

Result is always written to stdout only; diagnostics, warnings and errors are
always written to stderr only, regardless of --format.
";
