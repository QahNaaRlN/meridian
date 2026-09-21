//! `meridian init` — lays out a new workspace from `instance-template/` and
//! creates the two SQLite databases (`meridian-cli-rfc.md`, command `init`).
//!
//! Idempotent on repeat: an existing destination file is left untouched
//! (`crate::kernel::copy_instance_template`), and an existing database is
//! opened and its metadata checked, not recreated
//! (`meridian_storage_sqlite::SqliteStorage::open_path`).
//!
//! **Fail-clean.** This whole command is one atomic unit: on any failure
//! after work has begun — a copy failure partway through
//! `instance-template/`, a tool-database open failure, or a workspace-database
//! open failure after the template already copied cleanly — every path this
//! specific invocation created (copied template files, directories created
//! only to hold them, the workspace root itself if it did not exist before,
//! either database file) is removed, restoring exactly the state that
//! existed before this call. A path that already existed before this call —
//! a directory, a file the user had already placed, an already-open database
//! — is never deleted or overwritten by a rollback, whatever else in the same
//! call failed.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use meridian_app::events::EventSink;
use meridian_app::storage::{DatabaseMetadata, DatabaseRole};
use meridian_core::types::NonEmptyString;
use meridian_storage_sqlite::SqliteStorage;
use serde_json::json;

use crate::cli::{OutputFormat, ParsedArgs};
use crate::exit_code;
use crate::kernel::{self, TemplateCopyResult};

const COMMAND: &str = "init";
pub const ALLOWED_FLAGS: &[&str] = &["kernel", "workspace", "tool-db", "workspace-db", "format"];

/// Everything this specific `init` invocation has created so far — the
/// journal a failure rolls back. Nothing in here is ever a path that existed
/// before this call: each field is populated only at the moment this call
/// itself creates the thing it names.
#[derive(Default)]
struct Journal {
    workspace_created_dirs: Vec<PathBuf>,
    template: TemplateCopyResult,
    tool_db_created: bool,
    workspace_db_created: bool,
}

/// Rolls back exactly what this `init` invocation's [`Journal`] says it
/// created, and returns every cleanup failure encountered along the way
/// instead of discarding it — a caller that swallowed these could report a
/// clean rollback that did not, in fact, fully happen.
fn rollback(
    journal: &Journal,
    workspace_root: &Path,
    tool_db_path: &Path,
    workspace_db_path: &Path,
) -> Vec<String> {
    let mut diagnostics = Vec::new();
    if journal.workspace_db_created {
        if let Err(error) = fs::remove_file(workspace_db_path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                diagnostics.push(format!(
                    "rollback: cannot remove workspace database {}: {error}",
                    workspace_db_path.display()
                ));
            }
        }
    }
    if journal.tool_db_created {
        if let Err(error) = fs::remove_file(tool_db_path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                diagnostics.push(format!(
                    "rollback: cannot remove tool database {}: {error}",
                    tool_db_path.display()
                ));
            }
        }
    }
    kernel::rollback_template_copy(&journal.template, workspace_root, &mut diagnostics);
    let mut dirs = journal.workspace_created_dirs.clone();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    for dir in dirs {
        match fs::read_dir(&dir) {
            Ok(mut entries) => {
                if entries.next().is_none() {
                    if let Err(error) = fs::remove_dir(&dir) {
                        diagnostics.push(format!(
                            "rollback: cannot remove directory {}: {error}",
                            dir.display()
                        ));
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => diagnostics.push(format!(
                "rollback: cannot inspect directory {}: {error}",
                dir.display()
            )),
        }
    }
    diagnostics
}

/// Writes each rollback diagnostic as its own `warning:` line on `err`
/// before the primary error is reported — additional detail, never a
/// replacement for the primary failure that triggered the rollback.
fn report_rollback_diagnostics(err: &mut dyn Write, diagnostics: &[String]) {
    for diagnostic in diagnostics {
        let _ = writeln!(err, "warning: {diagnostic}");
    }
}

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
        NonEmptyString::new(format!(
            "init: kernel={kernel_path} workspace={workspace_path}"
        ))
        .unwrap(),
    ));

    let kernel_root = Path::new(kernel_path);
    let edition = match kernel::read_kernel_edition(kernel_root) {
        Ok(edition) => edition,
        Err(error) => return crate::report_environment_error(err, &error),
    };

    let workspace_root = Path::new(workspace_path);
    let tool_db_path_p = Path::new(tool_db_path);
    let workspace_db_path_p = Path::new(workspace_db_path);
    let tool_db_pre_existed = tool_db_path_p.exists();
    let workspace_db_pre_existed = workspace_db_path_p.exists();

    let mut journal = Journal::default();

    if let Err(error) =
        kernel::ensure_dir_tracked(workspace_root, &mut journal.workspace_created_dirs)
    {
        let diagnostics = rollback(
            &journal,
            workspace_root,
            tool_db_path_p,
            workspace_db_path_p,
        );
        report_rollback_diagnostics(err, &diagnostics);
        return crate::report_environment_error(
            err,
            &format!("cannot create workspace directory {workspace_path}: {error}"),
        );
    }

    match kernel::copy_instance_template(kernel_root, workspace_root) {
        Ok(result) => journal.template = result,
        Err((partial, error)) => {
            journal.template = partial;
            let diagnostics = rollback(
                &journal,
                workspace_root,
                tool_db_path_p,
                workspace_db_path_p,
            );
            report_rollback_diagnostics(err, &diagnostics);
            return crate::report_environment_error(err, &error);
        }
    }

    let tool_storage = match SqliteStorage::open_path(
        tool_db_path_p,
        DatabaseMetadata::new(DatabaseRole::Tool, edition.clone()),
    ) {
        Ok(storage) => {
            journal.tool_db_created = !tool_db_pre_existed;
            storage
        }
        Err(error) => {
            // SQLite creates the file as soon as it is opened, even when
            // schema preparation fails afterward — a freshly created (but
            // now unusable) file must still be rolled back, never a
            // pre-existing one.
            journal.tool_db_created = !tool_db_pre_existed;
            let diagnostics = rollback(
                &journal,
                workspace_root,
                tool_db_path_p,
                workspace_db_path_p,
            );
            report_rollback_diagnostics(err, &diagnostics);
            return crate::report_environment_error(
                err,
                &format!("cannot open tool database {tool_db_path}: {error}"),
            );
        }
    };

    let workspace_storage = match SqliteStorage::open_path(
        workspace_db_path_p,
        DatabaseMetadata::new(DatabaseRole::Workspace, edition.clone()),
    ) {
        Ok(storage) => {
            journal.workspace_db_created = !workspace_db_pre_existed;
            storage
        }
        Err(error) => {
            journal.workspace_db_created = !workspace_db_pre_existed;
            drop(tool_storage);
            let diagnostics = rollback(
                &journal,
                workspace_root,
                tool_db_path_p,
                workspace_db_path_p,
            );
            report_rollback_diagnostics(err, &diagnostics);
            return crate::report_environment_error(
                err,
                &format!("cannot open workspace database {workspace_db_path}: {error}"),
            );
        }
    };

    let (copied_files, skipped_files): (Vec<_>, Vec<_>) =
        journal.template.outcomes.iter().partition(|o| o.copied);

    let result = json!({
        "kernel": kernel_path,
        "kernel_edition": edition.as_str(),
        "workspace": workspace_path,
        "tool_db": tool_db_path,
        "workspace_db": workspace_db_path,
        "tool_db_created": journal.tool_db_created,
        "workspace_db_created": journal.workspace_db_created,
        "template_files_copied": copied_files.iter().map(|o| o.relative_path.clone()).collect::<Vec<_>>(),
        "template_files_skipped": skipped_files.iter().map(|o| o.relative_path.clone()).collect::<Vec<_>>(),
    });

    let _ = tool_storage.database_metadata();
    let _ = workspace_storage.database_metadata();

    sink.record(&meridian_app::events::ObservedEvent::new(
        meridian_app::events::EventKind::Outcome,
        NonEmptyString::new("init: completed").unwrap(),
    ));

    match args.format {
        OutputFormat::Json => crate::write_json_result(out, COMMAND, true, &result),
        OutputFormat::Human => {
            let _ = writeln!(out, "Kernel: {kernel_path} (edition {})", edition.as_str());
            let _ = writeln!(out, "Workspace: {workspace_path}");
            let _ = writeln!(
                out,
                "Tool database: {tool_db_path} ({})",
                if journal.tool_db_created {
                    "created"
                } else {
                    "already present"
                }
            );
            let _ = writeln!(
                out,
                "Workspace database: {workspace_db_path} ({})",
                if journal.workspace_db_created {
                    "created"
                } else {
                    "already present"
                }
            );
            let _ = writeln!(out, "Copied {} template file(s)", copied_files.len());
            let _ = writeln!(out, "Skipped {} existing file(s)", skipped_files.len());
        }
    }

    exit_code::OK
}
