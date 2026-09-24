//! `meridian doctor` — read-only health check of the Kernel (its edition and
//! that it is under Git, as `scripts/preflight.mjs` checks), the workspace
//! directory and both databases (`meridian-cli-rfc.md`, command `doctor`).
//!
//! Never writes: it neither creates a missing database
//! ([`super::open_existing_db`] refuses to) nor touches the workspace
//! directory or any Kernel file.

use std::io::Write;
use std::path::Path;

use meridian_app::events::EventSink;
use meridian_app::storage::{DatabaseMetadata, DatabaseRole};
use meridian_core::types::NonEmptyString;
use serde_json::{json, Value};

use crate::cli::{OutputFormat, ParsedArgs};
use crate::exit_code;
use crate::kernel;

const COMMAND: &str = "doctor";
pub const ALLOWED_FLAGS: &[&str] = &["kernel", "workspace", "tool-db", "workspace-db", "format"];

pub fn run(
    args: &ParsedArgs,
    out: &mut dyn Write,
    err: &mut dyn Write,
    sink: &dyn EventSink,
) -> i32 {
    let kernel_path = match args.require(COMMAND, "kernel") {
        Ok(value) => value,
        Err(error) => return crate::report_usage_error(err, &error),
    };
    let workspace_path = match args.require(COMMAND, "workspace") {
        Ok(value) => value,
        Err(error) => return crate::report_usage_error(err, &error),
    };
    let tool_db_path = match args.require(COMMAND, "tool-db") {
        Ok(value) => value,
        Err(error) => return crate::report_usage_error(err, &error),
    };
    let workspace_db_path = match args.require(COMMAND, "workspace-db") {
        Ok(value) => value,
        Err(error) => return crate::report_usage_error(err, &error),
    };

    sink.record(&meridian_app::events::ObservedEvent::new(
        meridian_app::events::EventKind::InputContext,
        NonEmptyString::new(format!("doctor: kernel={kernel_path}")).unwrap(),
    ));

    let kernel_root = Path::new(kernel_path);
    let mut healthy = true;

    let read_edition = kernel::read_kernel_edition(kernel_root);
    let kernel_check = match &read_edition {
        Ok(edition) => json!({"path": kernel_path, "ok": true, "edition": edition.as_str()}),
        Err(error) => {
            healthy = false;
            json!({"path": kernel_path, "ok": false, "error": error.to_string()})
        }
    };
    let kernel_edition = read_edition.ok();

    let kernel_git_check = match kernel::check_kernel_under_git(kernel_root) {
        Ok(()) => json!({"path": kernel_path, "ok": true}),
        Err(error) => {
            healthy = false;
            json!({"path": kernel_path, "ok": false, "error": error.to_string()})
        }
    };

    let workspace_root = Path::new(workspace_path);
    let workspace_check = if workspace_root.is_dir() {
        json!({"path": workspace_path, "ok": true, "exists": true})
    } else {
        healthy = false;
        json!({"path": workspace_path, "ok": false, "exists": false})
    };

    let (tool_check, tool_ok) =
        check_database(tool_db_path, DatabaseRole::Tool, kernel_edition.clone());
    let (workspace_check_db, workspace_db_ok) =
        check_database(workspace_db_path, DatabaseRole::Workspace, kernel_edition);
    healthy = healthy && tool_ok && workspace_db_ok;

    let result = json!({
        "kernel": kernel_check,
        "kernel_git": kernel_git_check,
        "workspace": workspace_check,
        "tool_db": tool_check,
        "workspace_db": workspace_check_db,
        "healthy": healthy,
    });

    sink.record(&meridian_app::events::ObservedEvent::new(
        meridian_app::events::EventKind::Outcome,
        NonEmptyString::new(if healthy {
            "doctor: healthy"
        } else {
            "doctor: unhealthy"
        })
        .unwrap(),
    ));

    match args.format {
        OutputFormat::Json => {
            crate::write_json_result(out, COMMAND, healthy, &result);
        }
        OutputFormat::Human => {
            print_human(out, &result);
        }
    }

    if healthy {
        exit_code::OK
    } else {
        exit_code::DOMAIN_NEGATIVE
    }
}

fn check_database(
    path_str: &str,
    role: DatabaseRole,
    kernel_edition: Option<meridian_core::types::Revision>,
) -> (Value, bool) {
    let path = Path::new(path_str);
    let Some(edition) = kernel_edition else {
        return (
            json!({"path": path_str, "role": role.as_str(), "ok": false, "error": "Kernel edition is unknown; see the \"kernel\" check"}),
            false,
        );
    };
    let expected = DatabaseMetadata::new(role, edition);
    match super::open_existing_db(path, expected) {
        Ok(storage) => match storage.schema_version() {
            Ok(schema_version) => (
                json!({
                    "path": path_str,
                    "role": storage.database_metadata().role().as_str(),
                    "kernel_edition": storage.database_metadata().kernel_edition().as_str(),
                    "schema_version": schema_version,
                    "ok": true,
                }),
                true,
            ),
            // A database that opens and passes its role/edition check but
            // cannot report its own schema version is not healthy — this
            // must never be reported as `ok: true` with a silently absent
            // `schema_version` (`null`), which previously happened because
            // the error here was dropped with `.ok()`.
            Err(error) => (
                json!({
                    "path": path_str,
                    "role": storage.database_metadata().role().as_str(),
                    "kernel_edition": storage.database_metadata().kernel_edition().as_str(),
                    "ok": false,
                    "error": format!("database opened but its schema_version could not be read: {error}"),
                }),
                false,
            ),
        },
        Err(diagnostic) => (
            json!({"path": path_str, "role": role.as_str(), "ok": false, "error": diagnostic.to_string()}),
            false,
        ),
    }
}

fn print_human(out: &mut dyn Write, result: &Value) {
    let line = |ok: bool| if ok { "OK" } else { "FAIL" };
    let _ = writeln!(
        out,
        "Kernel: {} ({})",
        line(result["kernel"]["ok"].as_bool().unwrap_or(false)),
        result["kernel"]
            .get("edition")
            .and_then(Value::as_str)
            .map(|e| format!("edition {e}"))
            .unwrap_or_else(|| result["kernel"]["error"].as_str().unwrap_or("").to_string())
    );
    let kernel_git = &result["kernel_git"];
    if kernel_git["ok"].as_bool().unwrap_or(false) {
        let _ = writeln!(out, "Kernel Git: OK");
    } else {
        let _ = writeln!(
            out,
            "Kernel Git: FAIL ({})",
            kernel_git["error"].as_str().unwrap_or("unknown error")
        );
    }
    let _ = writeln!(
        out,
        "Workspace: {}",
        line(result["workspace"]["ok"].as_bool().unwrap_or(false))
    );
    for (label, key) in [
        ("Tool database", "tool_db"),
        ("Workspace database", "workspace_db"),
    ] {
        let entry = &result[key];
        let ok = entry["ok"].as_bool().unwrap_or(false);
        if ok {
            let _ = writeln!(
                out,
                "{label}: OK (role={}, edition={}, schema_version={})",
                entry["role"].as_str().unwrap_or("?"),
                entry["kernel_edition"].as_str().unwrap_or("?"),
                entry["schema_version"]
                    .as_i64()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "?".to_string()),
            );
        } else {
            let _ = writeln!(
                out,
                "{label}: FAIL ({})",
                entry["error"].as_str().unwrap_or("unknown error")
            );
        }
    }
    let _ = writeln!(
        out,
        "Overall: {}",
        if result["healthy"].as_bool().unwrap_or(false) {
            "HEALTHY"
        } else {
            "UNHEALTHY"
        }
    );
}
