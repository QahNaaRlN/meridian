#![forbid(unsafe_code)]

//! Meridian CLI — composition root (package `meridian-cli-foundation`,
//! `meridian-rust-migration-program-plan.md` §4, row 7).
//!
//! [`run`] is the entire command surface, expressed as a pure function of
//! its arguments, stdin, and an [`EventSink`], writing only to the two
//! buffers it is given — `main` (the binary crate) is the only place that touches
//! real stdio, real `std::env::args`, or calls `std::process::exit`. This is
//! what lets the test suite exercise every command in-process, with a
//! [`crate::events::RecordingEventSink`] in place of
//! [`meridian_app::events::NoOpEventSink`], without spawning the real
//! binary — the real-binary integration tests in `tests/` exist
//! specifically to prove the compiled binary itself behaves the same way.
//!
//! Composition root responsibilities named in `meridian-cli-rfc.md`: this
//! crate is the only one that parses CLI arguments, reads Kernel and
//! workspace files, opens the two [`meridian_storage_sqlite::SqliteStorage`]
//! instances, and produces stdout/stderr/exit-code output. `meridian-core`
//! stays a pure domain library; `meridian-app` stays free of file, Git,
//! database, environment and CLI I/O; `meridian-storage-sqlite` stays the
//! only crate that knows SQLite. No command here re-implements a domain
//! algorithm `meridian-app`/`meridian-core` already provide.

pub mod adapters;
pub mod cli;
pub mod commands;
pub mod events;
pub mod exit_code;
pub mod kernel;

use std::io::{Read, Write};

use cli::{parse_flags, CliError};
use events::EventSink;
use serde_json::{json, Value};

/// Runs one invocation. `args` is every token after the binary name (i.e.
/// `std::env::args().skip(1)`), never including it.
pub fn run(
    args: &[String],
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    sink: &dyn EventSink,
) -> i32 {
    let Some(command) = args.first() else {
        let _ = writeln!(stderr, "error: {}", CliError::NoCommand);
        return exit_code::USAGE;
    };

    if command == "--help" || command == "-h" || command == "help" {
        let _ = write!(stdout, "{}", cli::USAGE);
        return exit_code::OK;
    }

    let rest = &args[1..];
    match command.as_str() {
        "init" => {
            let parsed = match parse_flags("init", rest, commands::init::ALLOWED_FLAGS) {
                Ok(parsed) => parsed,
                Err(error) => return report_usage_error(stderr, &error),
            };
            commands::init::run(&parsed, stdout, stderr, sink)
        }
        "doctor" => {
            let parsed = match parse_flags("doctor", rest, commands::doctor::ALLOWED_FLAGS) {
                Ok(parsed) => parsed,
                Err(error) => return report_usage_error(stderr, &error),
            };
            commands::doctor::run(&parsed, stdout, stderr, sink)
        }
        "validate" => {
            let parsed = match parse_flags("validate", rest, commands::validate::ALLOWED_FLAGS) {
                Ok(parsed) => parsed,
                Err(error) => return report_usage_error(stderr, &error),
            };
            commands::validate::run(&parsed, stdout, stderr, sink)
        }
        "resolve" => {
            let parsed = match parse_flags("resolve", rest, commands::resolve::ALLOWED_FLAGS) {
                Ok(parsed) => parsed,
                Err(error) => return report_usage_error(stderr, &error),
            };
            commands::resolve::run(&parsed, stdin, stdout, stderr, sink)
        }
        "export" => {
            let parsed = match parse_flags("export", rest, commands::export::ALLOWED_FLAGS) {
                Ok(parsed) => parsed,
                Err(error) => return report_usage_error(stderr, &error),
            };
            commands::export::run(&parsed, stdout, stderr, sink)
        }
        other => report_usage_error(stderr, &CliError::UnknownCommand(other.to_string())),
    }
}

fn report_usage_error(stderr: &mut dyn Write, message: &dyn std::fmt::Display) -> i32 {
    let _ = writeln!(stderr, "error: {message}");
    exit_code::USAGE
}

fn report_environment_error(stderr: &mut dyn Write, message: &dyn std::fmt::Display) -> i32 {
    let _ = writeln!(stderr, "error: {message}");
    exit_code::INPUT_OR_ENVIRONMENT
}

/// Writes the one stable JSON envelope every command's `--format json`
/// output uses: `{"status": "ok"|"fail", "command": <name>, "result": …}`.
/// `ok` names the domain verdict (`false` only for a well-formed negative
/// result such as `validate` finding a `FAIL`), never a usage or
/// environment error — those never reach this function at all, because they
/// have no command-specific result to wrap (`crate::exit_code`).
fn write_json_result(stdout: &mut dyn Write, command: &str, ok: bool, result: &Value) {
    let envelope = json!({
        "status": if ok { "ok" } else { "fail" },
        "command": command,
        "result": result,
    });
    let _ = writeln!(
        stdout,
        "{}",
        serde_json::to_string(&envelope).expect("envelope serializes without error")
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use events::{NoOpEventSink, RecordingEventSink};

    /// This crate's own Kernel checkout — `meridian-cli/`'s parent
    /// directory. `cargo test` runs a lib test binary with its working
    /// directory set to the *package* root (`meridian-cli/`, confirmed by a
    /// throwaway probe test), never the workspace root, so tests that need a
    /// real `VERSION`/`instance-template/` must not rely on
    /// `std::env::current_dir()`.
    fn kernel_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("meridian-cli has a parent directory")
            .to_path_buf()
    }

    fn run_args(args: &[&str]) -> (i32, String, String) {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut stdin = std::io::empty();
        let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let code = run(&owned, &mut stdin, &mut stdout, &mut stderr, &NoOpEventSink);
        (
            code,
            String::from_utf8(stdout).unwrap(),
            String::from_utf8(stderr).unwrap(),
        )
    }

    #[test]
    fn no_command_is_a_usage_error() {
        let (code, stdout, stderr) = run_args(&[]);
        assert_eq!(code, exit_code::USAGE);
        assert!(stdout.is_empty());
        assert!(!stderr.is_empty());
    }

    #[test]
    fn unknown_command_is_a_usage_error() {
        let (code, stdout, stderr) = run_args(&["bogus"]);
        assert_eq!(code, exit_code::USAGE);
        assert!(stdout.is_empty());
        assert!(stderr.contains("bogus"));
    }

    #[test]
    fn help_prints_usage_and_exits_ok() {
        let (code, stdout, stderr) = run_args(&["--help"]);
        assert_eq!(code, exit_code::OK);
        assert!(stdout.contains("USAGE"));
        assert!(stderr.is_empty());

        let (code, stdout, _) = run_args(&["-h"]);
        assert_eq!(code, exit_code::OK);
        assert!(stdout.contains("USAGE"));

        let (code, stdout, _) = run_args(&["help"]);
        assert_eq!(code, exit_code::OK);
        assert!(stdout.contains("USAGE"));
    }

    #[test]
    fn unknown_flag_is_a_usage_error() {
        let (code, stdout, stderr) = run_args(&["validate", "--bogus", "x"]);
        assert_eq!(code, exit_code::USAGE);
        assert!(stdout.is_empty());
        assert!(stderr.contains("bogus"));
    }

    #[test]
    fn missing_required_argument_is_a_usage_error() {
        let (code, stdout, stderr) = run_args(&["validate"]);
        assert_eq!(code, exit_code::USAGE);
        assert!(stdout.is_empty());
        assert!(stderr.contains("kernel"));
    }

    #[test]
    fn missing_flag_value_is_a_usage_error() {
        let (code, _, stderr) = run_args(&["validate", "--kernel"]);
        assert_eq!(code, exit_code::USAGE);
        assert!(stderr.contains("kernel"));
    }

    #[test]
    fn repeated_flag_is_a_usage_error() {
        let (code, _, stderr) = run_args(&["validate", "--kernel", "a", "--kernel", "b"]);
        assert_eq!(code, exit_code::USAGE);
        assert!(stderr.contains("kernel"));
    }

    #[test]
    fn invalid_format_value_is_a_usage_error() {
        let kernel = kernel_root();
        let (code, _, stderr) = run_args(&[
            "validate",
            "--kernel",
            kernel.to_str().unwrap(),
            "--format",
            "xml",
        ]);
        assert_eq!(code, exit_code::USAGE);
        assert!(stderr.contains("format"));
    }

    /// Package-7 gate (`meridian-rust-migration-program-plan.md` §6.4b):
    /// swapping the event sink changes nothing observable at the process
    /// boundary. Run against `validate` on this very Kernel checkout —
    /// exercising a real, non-trivial command, not a stub.
    #[test]
    fn swapping_the_event_sink_does_not_change_stdout_stderr_or_exit_code() {
        let kernel = kernel_root();
        let kernel_str = kernel.to_str().unwrap();
        let args: Vec<String> = ["validate", "--kernel", kernel_str, "--format", "json"]
            .iter()
            .map(|s| s.to_string())
            .collect();

        let mut stdout_noop = Vec::new();
        let mut stderr_noop = Vec::new();
        let code_noop = run(
            &args,
            &mut std::io::empty(),
            &mut stdout_noop,
            &mut stderr_noop,
            &NoOpEventSink,
        );

        let recording = RecordingEventSink::new();
        let mut stdout_recording = Vec::new();
        let mut stderr_recording = Vec::new();
        let code_recording = run(
            &args,
            &mut std::io::empty(),
            &mut stdout_recording,
            &mut stderr_recording,
            &recording,
        );

        // What must hold for this event-sink comparison to be meaningful
        // is that this checkout has no real `validate` failures of its own.
        let envelope: serde_json::Value =
            serde_json::from_slice(&stdout_noop).expect("validate --format json is one document");
        assert!(
            envelope["result"]["failures"]
                .as_array()
                .expect("failures array")
                .is_empty(),
            "this Kernel checkout must have zero real validate failures for the comparison to be meaningful: {envelope}"
        );
        assert_eq!(code_noop, code_recording);
        assert_eq!(stdout_noop, stdout_recording);
        assert_eq!(stderr_noop, stderr_recording);
        assert!(
            !recording.recorded().is_empty(),
            "the recording sink must actually have been invoked for this comparison to be meaningful"
        );
    }

    // -----------------------------------------------------------------
    // Package-7 gate (§6.4b), extended to every command: `doctor`,
    // `resolve`, `export` and `init` (init separately, below, with two
    // isolated destinations, since it is the one command whose result
    // depends on filesystem/database state as well as stdout).
    // -----------------------------------------------------------------

    struct TempDir(std::path::PathBuf);
    impl TempDir {
        fn new(label: &str) -> Self {
            use std::sync::atomic::{AtomicU64, Ordering};
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!(
                "meridian-cli-libtest-{}-{label}-{n}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&dir).expect("create temp dir");
            TempDir(dir)
        }
        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn run_twice_and_compare_process_output(args: &[String]) -> (i32, RecordingEventSink) {
        let mut stdout_noop = Vec::new();
        let mut stderr_noop = Vec::new();
        let code_noop = run(
            args,
            &mut std::io::empty(),
            &mut stdout_noop,
            &mut stderr_noop,
            &NoOpEventSink,
        );

        let recording = RecordingEventSink::new();
        let mut stdout_recording = Vec::new();
        let mut stderr_recording = Vec::new();
        let code_recording = run(
            args,
            &mut std::io::empty(),
            &mut stdout_recording,
            &mut stderr_recording,
            &recording,
        );

        assert_eq!(
            code_noop, code_recording,
            "exit code must not depend on the event sink"
        );
        assert_eq!(
            stdout_noop, stdout_recording,
            "stdout must not depend on the event sink"
        );
        assert_eq!(
            stderr_noop, stderr_recording,
            "stderr must not depend on the event sink"
        );
        (code_noop, recording)
    }

    #[test]
    fn event_sink_has_no_effect_on_doctor() {
        let temp = TempDir::new("doctor-sink");
        let kernel = kernel_root();
        let workspace = temp.path().join("workspace");
        let tool_db = temp.path().join("tool.sqlite3");
        let workspace_db = temp.path().join("workspace.sqlite3");
        let init_args: Vec<String> = [
            "init",
            "--kernel",
            kernel.to_str().unwrap(),
            "--workspace",
            workspace.to_str().unwrap(),
            "--tool-db",
            tool_db.to_str().unwrap(),
            "--workspace-db",
            workspace_db.to_str().unwrap(),
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(
            run(
                &init_args,
                &mut std::io::empty(),
                &mut Vec::new(),
                &mut Vec::new(),
                &NoOpEventSink
            ),
            exit_code::OK
        );
        let args: Vec<String> = [
            "doctor",
            "--kernel",
            kernel.to_str().unwrap(),
            "--workspace",
            workspace.to_str().unwrap(),
            "--tool-db",
            tool_db.to_str().unwrap(),
            "--workspace-db",
            workspace_db.to_str().unwrap(),
            "--format",
            "json",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let (code, recording) = run_twice_and_compare_process_output(&args);
        assert_eq!(
            code,
            exit_code::OK,
            "this scenario must actually be healthy for the comparison to be meaningful"
        );
        assert!(!recording.recorded().is_empty());
    }

    #[test]
    fn event_sink_has_no_effect_on_resolve() {
        let request = serde_json::json!({
            "work_item": {
                "repository_id": "sample-repo",
                "work_kind": "operation",
                "candidate_paths": [],
                "changed_paths": []
            },
            "repository_inventory": [{"id": "sample-repo"}],
            "applicability": {
                "$schema": "../../registries/rule-resolution/applicability.schema.json",
                "schema_version": 1,
                "records": []
            }
        });
        let temp = TempDir::new("resolve-sink");
        let request_path = temp.path().join("request.json");
        std::fs::write(&request_path, request.to_string()).unwrap();
        let kernel = kernel_root();
        let args: Vec<String> = [
            "resolve",
            "--kernel",
            kernel.to_str().unwrap(),
            "--request",
            request_path.to_str().unwrap(),
            "--format",
            "json",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let (code, recording) = run_twice_and_compare_process_output(&args);
        assert_eq!(
            code,
            exit_code::OK,
            "this scenario must actually be healthy for the comparison to be meaningful"
        );
        assert!(!recording.recorded().is_empty());
    }

    #[test]
    fn event_sink_has_no_effect_on_export() {
        let temp = TempDir::new("export-sink");
        let kernel = kernel_root();
        let workspace = temp.path().join("workspace");
        let tool_db = temp.path().join("tool.sqlite3");
        let workspace_db = temp.path().join("workspace.sqlite3");
        let init_args: Vec<String> = [
            "init",
            "--kernel",
            kernel.to_str().unwrap(),
            "--workspace",
            workspace.to_str().unwrap(),
            "--tool-db",
            tool_db.to_str().unwrap(),
            "--workspace-db",
            workspace_db.to_str().unwrap(),
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(
            run(
                &init_args,
                &mut std::io::empty(),
                &mut Vec::new(),
                &mut Vec::new(),
                &NoOpEventSink
            ),
            exit_code::OK
        );

        let args: Vec<String> = [
            "export",
            "--kernel",
            kernel.to_str().unwrap(),
            "--tool-db",
            tool_db.to_str().unwrap(),
            "--workspace-db",
            workspace_db.to_str().unwrap(),
            "--format",
            "json",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let (code, recording) = run_twice_and_compare_process_output(&args);
        assert_eq!(
            code,
            exit_code::OK,
            "this scenario must actually be healthy for the comparison to be meaningful"
        );
        assert!(!recording.recorded().is_empty());
    }

    /// `init` is the one command whose real effect is filesystem/database
    /// state, not only stdout — so this gate runs it into **two separate,
    /// isolated destinations** (one per sink) rather than the same
    /// destination twice, and compares the resulting file tree and each
    /// database's own metadata, not just process output.
    #[test]
    fn event_sink_has_no_effect_on_init_result_tree_or_database_metadata() {
        let kernel = kernel_root();

        let run_init =
            |label: &str, sink: &dyn EventSink| -> (std::path::PathBuf, TempDir, Vec<u8>, i32) {
                let temp = TempDir::new(label);
                let workspace = temp.path().join("workspace");
                let tool_db = temp.path().join("tool.sqlite3");
                let workspace_db = temp.path().join("workspace.sqlite3");
                let args: Vec<String> = [
                    "init",
                    "--kernel",
                    kernel.to_str().unwrap(),
                    "--workspace",
                    workspace.to_str().unwrap(),
                    "--tool-db",
                    tool_db.to_str().unwrap(),
                    "--workspace-db",
                    workspace_db.to_str().unwrap(),
                    "--format",
                    "json",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect();
                let mut stdout = Vec::new();
                let code = run(
                    &args,
                    &mut std::io::empty(),
                    &mut stdout,
                    &mut Vec::new(),
                    sink,
                );
                (workspace, temp, stdout, code)
            };

        let (workspace_noop, temp_noop, stdout_noop, code_noop) =
            run_init("init-sink-noop", &NoOpEventSink);
        let recording = RecordingEventSink::new();
        let (workspace_recording, temp_recording, stdout_recording, code_recording) =
            run_init("init-sink-recording", &recording);

        assert_eq!(code_noop, exit_code::OK);
        assert_eq!(code_noop, code_recording);
        assert!(!recording.recorded().is_empty());

        // The JSON result differs only in the two destinations' own literal
        // paths — parse both and compare everything else.
        let mut value_noop: serde_json::Value =
            serde_json::from_str(std::str::from_utf8(&stdout_noop).unwrap().trim()).unwrap();
        let mut value_recording: serde_json::Value =
            serde_json::from_str(std::str::from_utf8(&stdout_recording).unwrap().trim()).unwrap();
        for value in [&mut value_noop, &mut value_recording] {
            let result = value.get_mut("result").unwrap().as_object_mut().unwrap();
            result.remove("workspace");
            result.remove("tool_db");
            result.remove("workspace_db");
        }
        assert_eq!(value_noop, value_recording, "init's own reported outcome (edition, created flags, copied/skipped file lists) must be identical across two isolated destinations regardless of event sink");

        // The copied file trees themselves must be byte-identical.
        let files_of = |root: &std::path::Path| -> Vec<(String, Vec<u8>)> {
            let mut out = Vec::new();
            let mut stack = vec![root.to_path_buf()];
            while let Some(dir) = stack.pop() {
                for entry in std::fs::read_dir(&dir).unwrap() {
                    let entry = entry.unwrap();
                    let path = entry.path();
                    if path.is_dir() {
                        stack.push(path);
                    } else {
                        let rel = path
                            .strip_prefix(root)
                            .unwrap()
                            .to_string_lossy()
                            .into_owned();
                        out.push((rel, std::fs::read(&path).unwrap()));
                    }
                }
            }
            out.sort_by(|a, b| a.0.cmp(&b.0));
            out
        };
        assert_eq!(files_of(&workspace_noop), files_of(&workspace_recording));

        // Both databases' own canonicalised metadata (role, Kernel edition,
        // schema version) must agree too — not only the process output that
        // claims it does.
        let edition = kernel::read_kernel_edition(&kernel).unwrap();
        let db_metadata_of = |dir: &std::path::Path,
                              file: &str,
                              role: meridian_app::storage::DatabaseRole|
         -> (String, String, i64) {
            let storage = meridian_storage_sqlite::SqliteStorage::open_path(
                dir.join(file),
                meridian_app::storage::DatabaseMetadata::new(role, edition.clone()),
            )
            .unwrap();
            (
                storage.database_metadata().role().as_str().to_string(),
                storage
                    .database_metadata()
                    .kernel_edition()
                    .as_str()
                    .to_string(),
                storage.schema_version().unwrap(),
            )
        };
        assert_eq!(
            db_metadata_of(
                temp_noop.path(),
                "tool.sqlite3",
                meridian_app::storage::DatabaseRole::Tool
            ),
            db_metadata_of(
                temp_recording.path(),
                "tool.sqlite3",
                meridian_app::storage::DatabaseRole::Tool
            )
        );
        assert_eq!(
            db_metadata_of(
                temp_noop.path(),
                "workspace.sqlite3",
                meridian_app::storage::DatabaseRole::Workspace
            ),
            db_metadata_of(
                temp_recording.path(),
                "workspace.sqlite3",
                meridian_app::storage::DatabaseRole::Workspace
            )
        );
    }
}
