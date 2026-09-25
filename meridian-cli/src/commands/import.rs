//! `meridian import --kind frozen-instance|canonical-records`
//! (`meridian-rust-migration-program-plan.md` §5.22.2–§5.22.3).
//!
//! The kind is always explicit and closed: no format is guessed and no
//! kind falls back to the other. `frozen-instance` is the SAME
//! `plan → apply → verify` route as `migration plan|apply|verify` over one
//! bundle load; `canonical-records` accepts only the complete envelope
//! `meridian export --format json` writes.

use std::io::Write;
use std::path::Path;

use meridian_app::events::{EventKind, EventSink};
use meridian_app::migration::{self, CanonicalOutcome, RoleEffects};
use meridian_app::storage::DatabaseRole;
use serde_json::{json, Value};

use super::migration::{
    apply_json, digest_json, kernel_context, observe, open_source, open_store, rejected_json,
    report_error, verify_json, write_apply_human, write_rejection_human, write_verify_human,
};
use crate::cli::{CliError, ImportKind, OutputFormat, ParsedArgs};
use crate::exit_code;

const COMMAND: &str = "import";
pub const ALLOWED_FLAGS: &[&str] = &[
    "kernel",
    "kind",
    "source",
    "input",
    "tool-db",
    "workspace-db",
    "confirm",
    "format",
];

struct Parsed<'a> {
    kernel: &'a str,
    kind: ImportKind,
    location: &'a str,
    tool_db: &'a str,
    workspace_db: &'a str,
    confirm: &'a str,
}

fn parse(args: &ParsedArgs) -> Result<Parsed<'_>, CliError> {
    let kernel = args.require(COMMAND, "kernel")?;
    let kind = ImportKind::parse(COMMAND, args.require(COMMAND, "kind")?)?;
    let (location, forbidden, reason) = match kind {
        ImportKind::FrozenInstance => (
            args.require(COMMAND, "source")?,
            "input",
            "with --kind frozen-instance",
        ),
        ImportKind::CanonicalRecords => (
            args.require(COMMAND, "input")?,
            "source",
            "with --kind canonical-records",
        ),
    };
    if args.get(forbidden).is_some() {
        return Err(CliError::FlagNotAllowed {
            command: COMMAND,
            flag: forbidden,
            reason,
        });
    }
    Ok(Parsed {
        kernel,
        kind,
        location,
        tool_db: args.require(COMMAND, "tool-db")?,
        workspace_db: args.require(COMMAND, "workspace-db")?,
        confirm: args.require(COMMAND, "confirm")?,
    })
}

pub fn run(
    args: &ParsedArgs,
    out: &mut dyn Write,
    err: &mut dyn Write,
    sink: &dyn EventSink,
) -> i32 {
    let parsed = match parse(args) {
        Ok(parsed) => parsed,
        Err(error) => return crate::report_usage_error(err, &error),
    };
    let context = match kernel_context(parsed.kernel) {
        Ok(context) => context,
        Err(message) => return report_error(err, &message),
    };
    observe(
        sink,
        EventKind::InputContext,
        format!(
            "{COMMAND}: kind={} kernel={}",
            parsed.kind.as_str(),
            parsed.kernel
        ),
    );
    match parsed.kind {
        ImportKind::FrozenInstance => frozen(&parsed, &context, args.format, out, err, sink),
        ImportKind::CanonicalRecords => canonical(&parsed, &context, args.format, out, err, sink),
    }
}

fn finish(
    format: OutputFormat,
    ok: bool,
    value: &Value,
    human: impl FnOnce(&mut dyn Write),
    out: &mut dyn Write,
    sink: &dyn EventSink,
) -> i32 {
    observe(
        sink,
        EventKind::Outcome,
        format!(
            "{COMMAND}: {}",
            value["status"].as_str().unwrap_or("rejected")
        ),
    );
    match format {
        OutputFormat::Json => crate::write_json_result(out, COMMAND, ok, value),
        OutputFormat::Human => human(out),
    }
    if ok {
        exit_code::OK
    } else {
        exit_code::DOMAIN_NEGATIVE
    }
}

fn frozen(
    parsed: &Parsed<'_>,
    context: &super::migration::Context,
    format: OutputFormat,
    out: &mut dyn Write,
    err: &mut dyn Write,
    sink: &dyn EventSink,
) -> i32 {
    let source = match open_source(parsed.location) {
        Ok(source) => source,
        Err(message) => return report_error(err, &message),
    };
    // The tool database is named by the contract and verified (existing
    // file, role, Kernel edition) but never written by a frozen import. Like
    // the workspace database, it is opened only after the confirmation
    // matched: a wrong `--confirm` is reported as such whatever state the
    // databases are in, and opens neither.
    let tool_db = Path::new(parsed.tool_db);
    let open_tool = |access| open_store(tool_db, DatabaseRole::Tool, &context.edition, access);
    let workspace_db = Path::new(parsed.workspace_db);
    let open_workspace = |access| {
        open_store(
            workspace_db,
            DatabaseRole::Workspace,
            &context.edition,
            access,
        )
    };
    let result = match migration::import_frozen(
        &context.schemas,
        &source,
        &open_tool,
        &open_workspace,
        parsed.confirm,
        sink,
    ) {
        Ok(result) => result,
        Err(error) => return report_error(err, &error),
    };
    let import = match result {
        Err(rejection) => {
            let value = json!({
                "kind": ImportKind::FrozenInstance.as_str(),
                "status": "rejected",
                "bundle": rejected_json(&rejection),
            });
            return finish(
                format,
                false,
                &value,
                |o| write_rejection_human(o, &rejection),
                out,
                sink,
            );
        }
        Ok(import) => import,
    };
    let apply = apply_json(&import.bundle, false, &import.apply);
    let verify = import
        .verification
        .as_ref()
        .map(|v| verify_json(&import.bundle, v));
    let verified = import.verification.as_ref().is_some_and(|v| v.verified);
    let ok = import.apply.is_positive() && verified;
    let status = if ok {
        apply["status"].as_str().unwrap_or("").to_string()
    } else if import.apply.is_positive() {
        "not-verified".to_string()
    } else {
        apply["status"].as_str().unwrap_or("").to_string()
    };
    let value = json!({
        "kind": ImportKind::FrozenInstance.as_str(),
        "status": status,
        "input_digest": digest_json(&import.bundle.facts.fingerprint),
        "bundle_revision": import.bundle.bundle_revision,
        "source": {
            "repository_ref": import.bundle.source.repository_ref,
            "revision": import.bundle.source.revision,
            "digest": digest_json(&import.bundle.source.digest),
        },
        "apply": apply,
        "verify": verify,
    });
    let human = |o: &mut dyn Write| {
        let _ = writeln!(o, "Import frozen-instance: {status}");
        write_apply_human(o, &value["apply"]);
        if !value["verify"].is_null() {
            write_verify_human(o, &value["verify"]);
        }
    };
    finish(format, ok, &value, human, out, sink)
}

fn role_json(effects: &RoleEffects) -> Value {
    json!({
        "records": effects.records,
        "created": effects.effects.created,
        "updated": effects.effects.updated,
        "unchanged": effects.effects.unchanged,
    })
}

fn canonical(
    parsed: &Parsed<'_>,
    context: &super::migration::Context,
    format: OutputFormat,
    out: &mut dyn Write,
    err: &mut dyn Write,
    sink: &dyn EventSink,
) -> i32 {
    let bytes = match std::fs::read(parsed.location) {
        Ok(bytes) => bytes,
        Err(e) => {
            return report_error(
                err,
                &format!("cannot read the input {}: {e}", parsed.location),
            )
        }
    };
    let tool_db = Path::new(parsed.tool_db);
    let workspace_db = Path::new(parsed.workspace_db);
    let open_tool = |access| open_store(tool_db, DatabaseRole::Tool, &context.edition, access);
    let open_workspace = |access| {
        open_store(
            workspace_db,
            DatabaseRole::Workspace,
            &context.edition,
            access,
        )
    };
    let outcome = match migration::import_canonical(
        &context.schemas,
        &bytes,
        parsed.confirm,
        &open_tool,
        &open_workspace,
        sink,
    ) {
        Ok(outcome) => outcome,
        Err(error) => return report_error(err, &error),
    };
    let (ok, value) = match &outcome {
        CanonicalOutcome::Imported {
            input_digest,
            tool,
            workspace,
        } => (
            true,
            json!({
                "kind": ImportKind::CanonicalRecords.as_str(),
                "status": "imported",
                "input_digest": digest_json(input_digest),
                "tool": role_json(tool),
                "workspace": role_json(workspace),
                "reason": Value::Null,
            }),
        ),
        CanonicalOutcome::ConfirmationMismatch { expected, given } => (
            false,
            json!({
                "kind": ImportKind::CanonicalRecords.as_str(),
                "status": "confirmation-mismatch",
                "input_digest": digest_json(expected),
                "tool": Value::Null,
                "workspace": Value::Null,
                "reason": format!("--confirm \"{given}\" is not the SHA-256 of the input bytes; nothing was changed"),
            }),
        ),
        CanonicalOutcome::WriteFailed {
            input_digest,
            role,
            reason,
            restored_tool_state,
        } => (
            false,
            json!({
                "kind": ImportKind::CanonicalRecords.as_str(),
                "status": "write-failed",
                "input_digest": digest_json(input_digest),
                "failed_role": role.as_str(),
                "restored_tool_state_digest": restored_tool_state.as_ref().map(|s| digest_json(&s.digest)),
                "tool": Value::Null,
                "workspace": Value::Null,
                "reason": reason,
            }),
        ),
    };
    let human = |o: &mut dyn Write| {
        let _ = writeln!(
            o,
            "Import canonical-records: {}",
            value["status"].as_str().unwrap_or("")
        );
        for role in ["tool", "workspace"] {
            let r = &value[role];
            if !r.is_null() {
                let _ = writeln!(
                    o,
                    "{role}: {} record(s) — {} created, {} updated, {} unchanged",
                    r["records"], r["created"], r["updated"], r["unchanged"]
                );
            }
        }
        if let Some(reason) = value["reason"].as_str() {
            let _ = writeln!(o, "Reason: {reason}");
        }
    };
    finish(format, ok, &value, human, out, sink)
}
