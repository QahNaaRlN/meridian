//! `meridian resolve` — a thin file/stdin/stdout adapter around
//! `meridian_app::rule_resolution::resolve` (`meridian-cli-rfc.md`, command
//! `resolve`).
//!
//! The two registry schemas are read from the named `--kernel` checkout on
//! every invocation, never embedded in the binary (same principle as
//! `instance-template` for `init` — a build must always reflect whichever
//! Kernel it is pointed at). The request document is read from `--request
//! <path>` when given, or from stdin otherwise; exactly one JSON document is
//! expected, and it is never mixed with diagnostics on stdout.
//!
//! A schema violation, an unknown transport field or a malformed core
//! resolution is reported as [`crate::exit_code::INPUT_OR_ENVIRONMENT`] —
//! this is a rejected *request*, not a resolver *result* — but a *resolved*
//! output that itself carries `unresolved_applicability`,
//! `unresolved_items` or `conflicts` is exit 0: that is the resolver's own,
//! well-formed, deterministic answer, not a CLI failure
//! (`meridian-owner-intent-contract.md` §10, "resolver обязан вернуть
//! `unresolved_applicability` и остановиться, а не подставить правдоподобную
//! догадку" — returning that answer correctly is success for this command).

use std::io::{Read, Write};
use std::path::Path;

use meridian_app::events::EventSink;
use meridian_app::rule_resolution::{self, RuleResolutionError};
use meridian_core::types::NonEmptyString;
use serde_json::Value;

use crate::cli::{OutputFormat, ParsedArgs};
use crate::exit_code;

const COMMAND: &str = "resolve";
pub const ALLOWED_FLAGS: &[&str] = &["kernel", "request", "format"];

pub fn run(
    args: &ParsedArgs,
    stdin: &mut dyn Read,
    out: &mut dyn Write,
    err: &mut dyn Write,
    sink: &dyn EventSink,
) -> i32 {
    let kernel_path = match args.require(COMMAND, "kernel") {
        Ok(value) => value,
        Err(error) => return crate::report_usage_error(err, &error),
    };
    let kernel_root = Path::new(kernel_path);

    let applicability_schema_path =
        kernel_root.join("registries/rule-resolution/applicability.schema.json");
    let output_schema_path =
        kernel_root.join("registries/rule-resolution/resolver-output.schema.json");

    let applicability_schema = match read_json_file(&applicability_schema_path) {
        Ok(value) => value,
        Err(message) => return crate::report_environment_error(err, &message),
    };
    let output_schema = match read_json_file(&output_schema_path) {
        Ok(value) => value,
        Err(message) => return crate::report_environment_error(err, &message),
    };

    let request_text = match args.get("request") {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                return crate::report_environment_error(
                    err,
                    &format!("cannot read request file {path}: {error}"),
                )
            }
        },
        None => {
            let mut buffer = String::new();
            if let Err(error) = stdin.read_to_string(&mut buffer) {
                return crate::report_environment_error(
                    err,
                    &format!("cannot read request from stdin: {error}"),
                );
            }
            buffer
        }
    };

    sink.record(&meridian_app::events::ObservedEvent::new(
        meridian_app::events::EventKind::InputContext,
        NonEmptyString::new(format!("resolve: kernel={kernel_path}")).unwrap(),
    ));

    let request: Value = match serde_json::from_str(&request_text) {
        Ok(value) => value,
        Err(error) => {
            return crate::report_environment_error(
                err,
                &format!("request is not well-formed JSON: {error}"),
            )
        }
    };

    let outcome = rule_resolution::resolve(&request, &applicability_schema, &output_schema);
    match outcome {
        Ok(output) => {
            sink.record(&meridian_app::events::ObservedEvent::new(
                meridian_app::events::EventKind::Outcome,
                NonEmptyString::new("resolve: resolved").unwrap(),
            ));
            match args.format {
                OutputFormat::Json => crate::write_json_result(out, COMMAND, true, &output),
                OutputFormat::Human => print_human(out, &output),
            }
            exit_code::OK
        }
        Err(error) => {
            sink.record(&meridian_app::events::ObservedEvent::new(
                meridian_app::events::EventKind::Error,
                NonEmptyString::new(format!("resolve: rejected ({})", error_stage(&error)))
                    .unwrap(),
            ));
            crate::report_environment_error(err, &format!("request rejected: {error}"))
        }
    }
}

fn error_stage(error: &RuleResolutionError) -> &'static str {
    match error {
        RuleResolutionError::InvalidInput(_) => "invalid-input",
        RuleResolutionError::UnsupportedSchema(_) => "unsupported-schema",
        RuleResolutionError::Core(_) => "core",
        RuleResolutionError::InvalidOutput(_) => "invalid-output",
    }
}

fn read_json_file(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("malformed JSON in {}: {error}", path.display()))
}

fn print_human(out: &mut dyn Write, output: &Value) {
    let norms = output["applicable_norms"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);
    let protocols = output["applicable_protocols"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);
    let unresolved_applicability = output["unresolved_applicability"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);
    let unresolved_items = output["unresolved_items"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);
    let conflicts = output["conflicts"].as_array().map(Vec::len).unwrap_or(0);
    let _ = writeln!(out, "Applicable norms: {norms}");
    let _ = writeln!(out, "Applicable protocols: {protocols}");
    let _ = writeln!(
        out,
        "Required verification: {}",
        output["required_verification"]
            .as_array()
            .map(Vec::len)
            .unwrap_or(0)
    );
    let _ = writeln!(out, "Conflicts: {conflicts}");
    let _ = writeln!(out, "Unresolved applicability: {unresolved_applicability}");
    let _ = writeln!(out, "Unresolved items: {unresolved_items}");
    let _ = writeln!(
        out,
        "Requires re-resolution: {}",
        output["requires_reresolution"].as_bool().unwrap_or(false)
    );
    let _ = writeln!(
        out,
        "Decomposition required: {}",
        output["decomposition_required"].as_bool().unwrap_or(false)
    );
}
