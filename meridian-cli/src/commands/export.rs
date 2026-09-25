//! `meridian export` — canonical, deterministic export of both databases'
//! current records into one JSON array (`meridian-cli-rfc.md`, command
//! `export`).
//!
//! Each database's own canonical export
//! (`meridian_storage_sqlite::SqliteStorage::canonical_export_json`) already
//! carries recursively sorted keys and its own records ordered by
//! `record_key`; this command only concatenates the two arrays and
//! re-sorts the combined array by the same
//! [`meridian_app::storage::RecordKey::storage_key`] ordering, so the merge
//! itself introduces no dependence on which database happened to be read
//! first. No record is copied between databases — each element in the
//! output was read from exactly the database whose role already held it.

use std::io::Write;
use std::path::Path;

use meridian_app::events::EventSink;
use meridian_app::storage::{DatabaseMetadata, DatabaseRole};
use meridian_core::types::NonEmptyString;
use serde_json::Value;

use crate::cli::{OutputFormat, ParsedArgs};
use crate::exit_code;
use crate::kernel;

const COMMAND: &str = "export";
pub const ALLOWED_FLAGS: &[&str] = &["kernel", "tool-db", "workspace-db", "format"];

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
    let tool_db_path = match args.require(COMMAND, "tool-db") {
        Ok(value) => value,
        Err(error) => return crate::report_usage_error(err, &error),
    };
    let workspace_db_path = match args.require(COMMAND, "workspace-db") {
        Ok(value) => value,
        Err(error) => return crate::report_usage_error(err, &error),
    };

    let edition = match kernel::read_kernel_edition(Path::new(kernel_path)) {
        Ok(edition) => edition,
        Err(error) => return crate::report_environment_error(err, &error),
    };

    sink.record(&meridian_app::events::ObservedEvent::new(
        meridian_app::events::EventKind::InputContext,
        NonEmptyString::new(format!("export: kernel={kernel_path}")).unwrap(),
    ));

    let tool_storage = match super::open_existing_db(
        Path::new(tool_db_path),
        DatabaseMetadata::new(DatabaseRole::Tool, edition.clone()),
    ) {
        Ok(storage) => storage,
        Err(diagnostic) => {
            return crate::report_environment_error(
                err,
                &format!("cannot open tool database {tool_db_path}: {diagnostic}"),
            )
        }
    };
    let workspace_storage = match super::open_existing_db(
        Path::new(workspace_db_path),
        DatabaseMetadata::new(DatabaseRole::Workspace, edition),
    ) {
        Ok(storage) => storage,
        Err(diagnostic) => {
            return crate::report_environment_error(
                err,
                &format!("cannot open workspace database {workspace_db_path}: {diagnostic}"),
            )
        }
    };

    let tool_json = match tool_storage.canonical_export_json() {
        Ok(text) => text,
        Err(error) => {
            return crate::report_environment_error(
                err,
                &format!("cannot export tool database: {error}"),
            )
        }
    };
    let workspace_json = match workspace_storage.canonical_export_json() {
        Ok(text) => text,
        Err(error) => {
            return crate::report_environment_error(
                err,
                &format!("cannot export workspace database: {error}"),
            )
        }
    };

    let merged = merge_canonical_exports(&tool_json, &workspace_json);

    sink.record(&meridian_app::events::ObservedEvent::new(
        meridian_app::events::EventKind::Outcome,
        NonEmptyString::new(format!(
            "export: {} record(s)",
            merged.as_array().map(Vec::len).unwrap_or(0)
        ))
        .unwrap(),
    ));

    match args.format {
        OutputFormat::Json => crate::write_json_result(out, COMMAND, true, &merged),
        OutputFormat::Human => {
            let records = merged.as_array().cloned().unwrap_or_default();
            let _ = writeln!(out, "Exported {} record(s)", records.len());
            for record in &records {
                let _ = writeln!(
                    out,
                    "{} [{}] {}",
                    record["id"].as_str().unwrap_or("?"),
                    record["scope"]["type"].as_str().unwrap_or("?"),
                    record["title"].as_str().unwrap_or("?"),
                );
            }
        }
    }

    exit_code::OK
}

/// Parses two already-canonical export documents, concatenates their
/// arrays, and re-sorts the combined array by the same segment ordering
/// [`meridian_app::storage::RecordKey::storage_key`] uses — so the array
/// order is a function of content alone, never of which database was read
/// first or which order this function happens to combine them in.
fn merge_canonical_exports(tool_json: &str, workspace_json: &str) -> Value {
    let mut records: Vec<Value> = Vec::new();
    for text in [tool_json, workspace_json] {
        if let Ok(Value::Array(items)) = serde_json::from_str::<Value>(text) {
            records.extend(items);
        }
    }
    records.sort_by_key(sort_key);
    Value::Array(records)
}

fn sort_key(record: &Value) -> String {
    let scope = &record["scope"];
    format!(
        "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
        scope["type"].as_str().unwrap_or(""),
        scope["id"].as_str().unwrap_or(""),
        scope["workspace_id"].as_str().unwrap_or(""),
        scope["organization_profile_id"].as_str().unwrap_or(""),
        record["id"].as_str().unwrap_or(""),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merge_orders_deterministically_regardless_of_input_order() {
        let a = json!([{"id": "b", "scope": {"type": "project-workspace", "id": "p"}}]).to_string();
        let b = json!([{"id": "a", "scope": {"type": "project-workspace", "id": "p"}}]).to_string();
        let merged_ab = merge_canonical_exports(&a, &b);
        let merged_ba = merge_canonical_exports(&b, &a);
        assert_eq!(merged_ab, merged_ba);
    }
}
