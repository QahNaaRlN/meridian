//! Real-binary integration tests for package `meridian-cli-foundation`
//! (`meridian-rust-migration-program-plan.md` §4, row 7). Every test spawns
//! the compiled `meridian` binary via `std::process::Command` — no test here
//! calls into `meridian_cli` in-process (that suite lives in
//! `meridian-cli/src/lib.rs`'s own `#[cfg(test)] mod tests`).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

fn exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_meridian"))
}

/// This crate's own Kernel checkout — `meridian-cli/`'s parent directory,
/// which carries `VERSION`, `instance-template/` and `registries/`. Using
/// the real repository (rather than a synthetic fixture) is what proves
/// `init`'s template copy and `validate`'s file walk against real, current
/// Kernel content, not a hand-picked stand-in.
fn kernel_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("meridian-cli has a parent directory")
        .to_path_buf()
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// An RAII-owned, unique temporary directory: removed on `Drop`, including
/// while unwinding a panicking assertion — so a failing test does not leave
/// its scratch tree behind, and no test here ever runs a broad `/tmp`
/// cleanup of anything it did not itself create.
struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let n = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "meridian-cli-test-{}-{label}-{n}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create fresh temp dir");
        TempDir(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        // Best effort: a test that made a subdirectory unreadable (chmod 0)
        // for a fail-closed-walk case restores permissions first so removal
        // can actually succeed, but a residual failure here still must not
        // panic during unwinding.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = restore_permissions_recursive(&self.0);
            fn restore_permissions_recursive(dir: &Path) -> std::io::Result<()> {
                if dir.is_dir() {
                    let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755));
                    for entry in std::fs::read_dir(dir)?.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            let _ = restore_permissions_recursive(&path);
                        }
                    }
                }
                Ok(())
            }
        }
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&str]) -> Output {
    Command::new(exe())
        .args(args)
        .output()
        .expect("run meridian binary")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}
fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn json_result(output: &Output) -> serde_json::Value {
    serde_json::from_str(stdout_of(output).trim()).expect("stdout is one JSON document")
}

// ---------------------------------------------------------------------
// Usage surface: --help, unknown command/flag, missing required argument.
// ---------------------------------------------------------------------

#[test]
fn help_prints_usage_to_stdout_and_exits_ok() {
    for flag in ["--help", "-h", "help"] {
        let output = run(&[flag]);
        assert!(output.status.success(), "{flag} must exit 0");
        assert!(stdout_of(&output).contains("USAGE"));
        assert!(
            stderr_of(&output).is_empty(),
            "{flag} must not write to stderr"
        );
    }
}

#[test]
fn no_command_is_a_usage_error_on_stderr_with_empty_stdout() {
    let output = run(&[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stdout_of(&output).is_empty());
    assert!(!stderr_of(&output).is_empty());
}

#[test]
fn unknown_command_is_a_usage_error() {
    let output = run(&["nonsense-command"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stdout_of(&output).is_empty());
    assert!(stderr_of(&output).contains("nonsense-command"));
}

#[test]
fn unknown_flag_is_a_usage_error() {
    let output = run(&["validate", "--kernel", ".", "--totally-unknown", "x"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stdout_of(&output).is_empty());
    assert!(stderr_of(&output).contains("totally-unknown"));
}

#[test]
fn missing_required_argument_is_a_usage_error() {
    for args in [
        vec!["validate"],
        vec!["init"],
        vec!["doctor"],
        vec!["export"],
        vec!["resolve"],
    ] {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(2), "args={args:?}");
        assert!(stdout_of(&output).is_empty());
        assert!(stderr_of(&output).contains("kernel"));
    }
}

/// Package `meridian-cli-foundation`, subpackage `validate-mechanical-integrity`
/// item 5: `resolve`'s two distinct negative exit codes,
/// side by side in one test, so a future regression that collapsed them
/// back into one code would fail here rather than only in a producer's own
/// discarded detail. A bad flag never reaches request content at all
/// (`USAGE`, exit 2); a well-formed command line with a malformed request
/// body is read, and only its content is rejected (`INPUT_OR_ENVIRONMENT`,
/// exit 3) — two different reasons to refuse, two different codes.
#[test]
fn resolve_distinguishes_a_usage_error_from_a_rejected_request_by_exit_code() {
    let kernel = kernel_root();

    let usage = run(&[
        "resolve",
        "--kernel",
        kernel.to_str().unwrap(),
        "--totally-unknown",
        "x",
    ]);
    assert_eq!(usage.status.code(), Some(2));
    assert!(stdout_of(&usage).is_empty());
    assert!(!stderr_of(&usage).is_empty());

    use std::io::Write;
    let mut child = Command::new(exe())
        .args(["resolve", "--kernel", kernel.to_str().unwrap()])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn meridian resolve");
    child
        .stdin
        .take()
        .expect("child stdin")
        .write_all(b"not json")
        .expect("write malformed request to child stdin");
    let rejected = child.wait_with_output().expect("wait for meridian resolve");

    assert_eq!(rejected.status.code(), Some(3));
    assert!(stdout_of(&rejected).is_empty());
    assert!(!stderr_of(&rejected).is_empty());

    assert_ne!(usage.status.code(), rejected.status.code());
}

// ---------------------------------------------------------------------
// validate — positive, against the real Kernel checkout.
// ---------------------------------------------------------------------

/// This Kernel checkout has zero *real* `validate` failures, but `validate`
/// still correctly reports `exit_code::DOMAIN_NEGATIVE` / `status: "fail"` /
/// `result.ok: false`, never `0`/`"ok"`/`true` — `BLOCKED_CHECKS`
/// (`meridian-cli/src/commands/validate/mod.rs`) is non-empty, and an unrun
/// mandatory gate family is not a passed one
/// (`meridian-rust-migration-program-plan.md`, item 1 of the second
/// `CHANGES_REQUESTED` round on package `meridian-cli-foundation`).
#[test]
fn validate_reports_blocked_not_ok_on_this_kernel_with_zero_real_failures() {
    let kernel = kernel_root();
    let kernel = kernel.to_str().unwrap();

    let human = run(&["validate", "--kernel", kernel]);
    assert_eq!(
        human.status.code(),
        Some(1),
        "stderr: {}",
        stderr_of(&human)
    );
    let human_out = stdout_of(&human);
    assert!(human_out.contains("0 failure(s)"), "stdout: {human_out}");
    assert!(human_out.contains("BLOCKED"), "stdout: {human_out}");
    assert!(!human_out.contains("\nOK\n") && !human_out.ends_with("OK\n"));

    let json_output = run(&["validate", "--kernel", kernel, "--format", "json"]);
    assert_eq!(json_output.status.code(), Some(1));
    let value = json_result(&json_output);
    assert_eq!(value["status"], "fail");
    assert_eq!(value["command"], "validate");
    assert_eq!(value["result"]["ok"], false);
    assert_eq!(value["result"]["failures"].as_array().unwrap().len(), 0);
    // Ground-truth cross-check against the real Node reference's own output
    // on this exact tree (`node scripts/kernel-validate.mjs`): 333 tracked
    // names (was 302 before the 7a merge added tracked files to `dev`), 12/12
    // registries, 20 satisfied / 27 rejected rule-resolution fixtures.
    assert_eq!(value["result"]["stats"]["document_identity_checked"], 333);
    assert_eq!(value["result"]["stats"]["schema_validated"], 12);
    assert_eq!(value["result"]["stats"]["schema_attempted"], 12);
    assert_eq!(value["result"]["stats"]["rule_resolution_satisfied"], 20);
    assert_eq!(value["result"]["stats"]["rule_resolution_rejected"], 27);
    assert_eq!(
        value["result"]["stats"]["agent_instruction_identity_declared_norms"],
        28
    );
    assert_eq!(
        value["result"]["stats"]["agent_instruction_identity_undeclared_prescriptive"],
        22
    );
    assert_eq!(
        value["result"]["stats"]["agent_instruction_identity_undeclared_other"],
        30
    );
    // 20 before subpackage 7a, 15 after 7a removed its own five families;
    // subpackage 7b then removed its own seven families from
    // `BLOCKED_CHECKS` (`meridian-cli/src/commands/validate/mod.rs`) — 8
    // remain, all belonging to 7c/7d.
    assert_eq!(value["result"]["blocked"].as_array().unwrap().len(), 8);
    assert!(stderr_of(&json_output).is_empty());
}

// ---------------------------------------------------------------------
// validate — real negative cases, one per ported check family.
// ---------------------------------------------------------------------

#[test]
fn validate_detects_a_personal_path_leak() {
    let temp = TempDir::new("validate-kernel-purity");
    // Built from fragments, like the pattern it is meant to trip: this
    // source file is itself scanned by `meridian validate` (real Kernel
    // content), so it must never contain the literal sequence it is testing
    // for — the same reason `kernel_purity::personal_path_pattern` builds
    // its own regex the same way.
    let personal_path = ["C:", r"\", "Users", r"\", "alice"].join("");
    let content = format!(
        "---\ntitle: Note\ndocument_type: unclassified\nunclassified_reason: test fixture\nstatus: draft\nscope: workspace\nowner: workspace-owner\ncreated: 2026-01-01\nupdated: 2026-01-01\n---\n\nleaked path: {personal_path}\\secret\n"
    );
    std::fs::write(temp.path().join("note.md"), content).unwrap();

    let output = run(&[
        "validate",
        "--kernel",
        temp.path().to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let value = json_result(&output);
    let failures: Vec<String> = value["result"]["failures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(
        failures
            .iter()
            .any(|f| f.starts_with("kernel-purity: personal home path")),
        "failures: {failures:?}"
    );
}

#[test]
fn validate_detects_a_non_kebab_case_file_name() {
    let temp = TempDir::new("validate-document-identity");
    std::fs::write(temp.path().join("NotKebabCase.txt"), "content").unwrap();

    let output = run(&[
        "validate",
        "--kernel",
        temp.path().to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let value = json_result(&output);
    let failures: Vec<String> = value["result"]["failures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(
        failures
            .iter()
            .any(|f| f.contains("document-identity") && f.contains("not lower kebab-case")),
        "failures: {failures:?}"
    );
}

#[test]
fn validate_detects_an_orphaned_trailing_front_matter_block() {
    let temp = TempDir::new("validate-duplicate-fm");
    std::fs::write(
        temp.path().join("doc.md"),
        "---\ntitle: Doc\nstatus: draft\nscope: workspace\nowner: workspace-owner\ncreated: 2026-01-01\nupdated: 2026-01-01\ndocument_type: unclassified\nunclassified_reason: test fixture\n---\n\nBody text.\n\n---\ntitle: leaked\n---\n",
    )
    .unwrap();

    let output = run(&[
        "validate",
        "--kernel",
        temp.path().to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let value = json_result(&output);
    let failures: Vec<String> = value["result"]["failures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(
        failures.iter().any(|f| f.starts_with("duplicate-fm:")),
        "failures: {failures:?}"
    );
}

#[test]
fn validate_detects_a_dangling_markdown_link() {
    let temp = TempDir::new("validate-link-check");
    std::fs::write(
        temp.path().join("doc.md"),
        "See [missing](./does-not-exist.md) for details.\n",
    )
    .unwrap();

    let output = run(&[
        "validate",
        "--kernel",
        temp.path().to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let value = json_result(&output);
    let failures: Vec<String> = value["result"]["failures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(
        failures
            .iter()
            .any(|f| f.starts_with("link:") && f.contains("does not resolve")),
        "failures: {failures:?}"
    );
}

#[test]
fn validate_detects_a_registry_document_that_violates_its_declared_schema() {
    let temp = TempDir::new("validate-registry-schema");
    std::fs::write(
        temp.path().join("schema.json"),
        r#"{"$schema":"http://json-schema.org/draft-07/schema#","type":"object","required":["name"],"properties":{"name":{"type":"string"}}}"#,
    )
    .unwrap();
    std::fs::write(
        temp.path().join("data.yaml"),
        "$schema: ./schema.json\nage: 5\n",
    )
    .unwrap();

    let output = run(&[
        "validate",
        "--kernel",
        temp.path().to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let value = json_result(&output);
    let failures: Vec<String> = value["result"]["failures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(
        failures.iter().any(|f| f.starts_with("schema:")),
        "failures: {failures:?}"
    );
}

// ---------------------------------------------------------------------
// validate — fail-closed file walk (item 4): an unreadable subtree is a
// hard environment error (exit 3), never a clean or domain-negative result.
// ---------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn validate_exits_with_environment_error_when_a_subdirectory_is_unreadable() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TempDir::new("validate-unreadable");
    let locked = temp.path().join("locked");
    std::fs::create_dir_all(&locked).unwrap();
    std::fs::write(locked.join("inside.md"), "content").unwrap();
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();

    // Skip under a root-equivalent test runner (or a filesystem, such as
    // some container/CI overlays, that does not enforce these bits at all):
    // an empirical check, not a euid lookup, so this test genuinely does not
    // apply to any environment where the permission bits do not block a
    // read here — the assertion below would otherwise be a false failure of
    // the test, not of the command under test.
    let mode_is_enforced = std::fs::read_dir(&locked).is_err();
    if !mode_is_enforced {
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }

    let output = run(&[
        "validate",
        "--kernel",
        temp.path().to_str().unwrap(),
        "--format",
        "json",
    ]);

    // Restore permissions immediately so `TempDir::drop` can clean up even
    // if an assertion below panics.
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();

    assert_eq!(
        output.status.code(),
        Some(3),
        "stderr: {}",
        stderr_of(&output)
    );
    assert!(stdout_of(&output).is_empty());
    assert!(!stderr_of(&output).is_empty());
}

// ---------------------------------------------------------------------
// resolve
// ---------------------------------------------------------------------

fn write_resolve_request(dir: &Path) -> PathBuf {
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
    let path = dir.join("request.json");
    std::fs::write(&path, serde_json::to_string_pretty(&request).unwrap()).unwrap();
    path
}

#[test]
fn resolve_reads_a_request_file_and_prints_json_to_stdout_only() {
    let temp = TempDir::new("resolve-file");
    let request_path = write_resolve_request(temp.path());
    let kernel = kernel_root();

    let output = run(&[
        "resolve",
        "--kernel",
        kernel.to_str().unwrap(),
        "--request",
        request_path.to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        stderr_of(&output)
    );
    assert!(stderr_of(&output).is_empty());
    let value = json_result(&output);
    assert_eq!(value["status"], "ok");
    assert_eq!(value["result"]["applicable_norms"], serde_json::json!([]));
}

#[test]
fn resolve_reads_from_stdin_when_no_request_flag_is_given() {
    use std::io::Write;
    let kernel = kernel_root();
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

    let mut child = Command::new(exe())
        .args([
            "resolve",
            "--kernel",
            kernel.to_str().unwrap(),
            "--format",
            "json",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(request.to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        stderr_of(&output)
    );
    let value = json_result(&output);
    assert_eq!(value["status"], "ok");
}

#[test]
fn resolve_rejects_malformed_json_with_environment_exit_code_and_empty_stdout() {
    let temp = TempDir::new("resolve-malformed");
    let path = temp.path().join("bad.json");
    std::fs::write(&path, "{ not json").unwrap();
    let kernel = kernel_root();

    let output = run(&[
        "resolve",
        "--kernel",
        kernel.to_str().unwrap(),
        "--request",
        path.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(3));
    assert!(stdout_of(&output).is_empty());
    assert!(!stderr_of(&output).is_empty());
}

#[test]
fn resolve_rejects_unknown_transport_fields() {
    let temp = TempDir::new("resolve-unknown-field");
    let mut request = serde_json::json!({
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
    request["reviewer"] = serde_json::json!("must-not-affect-resolution");
    let path = temp.path().join("request.json");
    std::fs::write(&path, request.to_string()).unwrap();
    let kernel = kernel_root();

    let output = run(&[
        "resolve",
        "--kernel",
        kernel.to_str().unwrap(),
        "--request",
        path.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(3));
    assert!(stdout_of(&output).is_empty());
    assert!(stderr_of(&output).contains("unknown field"));
}

// ---------------------------------------------------------------------
// init / doctor / export over a real, freshly initialised workspace.
// ---------------------------------------------------------------------

struct InitPaths {
    _guard: TempDir,
    workspace: PathBuf,
    tool_db: PathBuf,
    workspace_db: PathBuf,
}

impl InitPaths {
    fn new(label: &str) -> Self {
        let guard = TempDir::new(label);
        let base = guard.path().to_path_buf();
        InitPaths {
            _guard: guard,
            workspace: base.join("workspace"),
            tool_db: base.join("tool.sqlite3"),
            workspace_db: base.join("workspace.sqlite3"),
        }
    }
}

fn run_init(paths: &InitPaths, format: &str) -> Output {
    let kernel = kernel_root();
    run(&[
        "init",
        "--kernel",
        kernel.to_str().unwrap(),
        "--workspace",
        paths.workspace.to_str().unwrap(),
        "--tool-db",
        paths.tool_db.to_str().unwrap(),
        "--workspace-db",
        paths.workspace_db.to_str().unwrap(),
        "--format",
        format,
    ])
}

/// A full, deterministic snapshot of everything `init` could have touched:
/// whether the workspace directory exists, the sorted relative-path/content
/// list of every file under it, and whether each database file exists (byte
/// content of a SQLite file is not compared directly — timestamps inside it
/// are not deterministic — existence plus, when it exists, its own
/// role/edition/schema-version metadata is what a rollback promises to
/// restore, and those are compared separately where relevant).
#[derive(Debug, PartialEq, Eq)]
struct WorkspaceSnapshot {
    workspace_exists: bool,
    files: Vec<(String, Vec<u8>)>,
    tool_db_exists: bool,
    workspace_db_exists: bool,
}

fn snapshot(paths: &InitPaths) -> WorkspaceSnapshot {
    let mut files = Vec::new();
    if paths.workspace.is_dir() {
        let mut stack = vec![paths.workspace.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    let rel = path
                        .strip_prefix(&paths.workspace)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned();
                    files.push((rel, std::fs::read(&path).unwrap()));
                }
            }
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    WorkspaceSnapshot {
        workspace_exists: paths.workspace.is_dir(),
        files,
        tool_db_exists: paths.tool_db.exists(),
        workspace_db_exists: paths.workspace_db.exists(),
    }
}

#[test]
fn init_on_an_empty_directory_copies_the_template_file_for_file_and_creates_two_databases() {
    let paths = InitPaths::new("init-empty");
    let output = run_init(&paths, "json");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        stderr_of(&output)
    );
    assert!(stderr_of(&output).is_empty());

    let value = json_result(&output);
    assert_eq!(value["status"], "ok");
    assert_eq!(value["result"]["tool_db_created"], true);
    assert_eq!(value["result"]["workspace_db_created"], true);
    assert!(value["result"]["template_files_skipped"]
        .as_array()
        .unwrap()
        .is_empty());

    assert!(paths.tool_db.is_file());
    assert!(paths.workspace_db.is_file());

    // Exact file-for-file copy: every file under instance-template/ exists,
    // byte-identical, at the same relative path under the workspace.
    let kernel = kernel_root();
    let template_root = kernel.join("instance-template");
    let mut stack = vec![template_root.clone()];
    let mut checked = 0;
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let relative = path.strip_prefix(&template_root).unwrap();
                let destination = paths.workspace.join(relative);
                assert!(destination.is_file(), "missing copied file: {relative:?}");
                assert_eq!(
                    std::fs::read(&path).unwrap(),
                    std::fs::read(&destination).unwrap(),
                    "copied file differs: {relative:?}"
                );
                checked += 1;
            }
        }
    }
    assert!(
        checked > 0,
        "instance-template must contain at least one file"
    );
}

#[test]
fn repeat_init_is_idempotent_and_never_overwrites_an_existing_file() {
    let paths = InitPaths::new("init-repeat");
    let first = run_init(&paths, "json");
    assert_eq!(first.status.code(), Some(0));

    // A user edits one of the copied files.
    let edited_relative = "product.yaml";
    let edited_path = paths.workspace.join(edited_relative);
    assert!(edited_path.is_file());
    std::fs::write(&edited_path, b"# user edit\n").unwrap();

    let second = run_init(&paths, "json");
    assert_eq!(
        second.status.code(),
        Some(0),
        "stderr: {}",
        stderr_of(&second)
    );
    let value = json_result(&second);
    assert_eq!(value["result"]["tool_db_created"], false);
    assert_eq!(value["result"]["workspace_db_created"], false);
    assert!(value["result"]["template_files_skipped"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p.as_str() == Some(edited_relative)));

    // The user's edit survived the second init untouched.
    assert_eq!(std::fs::read(&edited_path).unwrap(), b"# user edit\n");
}

// -----------------------------------------------------------------
// init — fail-clean (item 3): a failure at each of the three distinct
// points restores exactly the pre-call state. Each test snapshots a
// pre-existing, unrelated user file/directory placed in the destination
// *before* calling `init`, to prove a rollback never touches something it
// did not itself create — not only that "some new stuff" is removed.
// -----------------------------------------------------------------

#[test]
fn init_rolls_back_completely_when_the_template_copy_fails_partway_through() {
    let paths = InitPaths::new("init-fail-mid-copy");
    std::fs::create_dir_all(&paths.workspace).unwrap();
    // A pre-existing, unrelated user file that must survive untouched.
    std::fs::write(paths.workspace.join("user-file.txt"), b"pre-existing").unwrap();
    // Force the copy to fail partway: `commands/repositories.yaml` is one of
    // instance-template's files, sorted after several others
    // (`.agent/...`), so earlier files are copied before this one is
    // reached. Pre-creating `commands` as a plain *file* (not a directory)
    // makes the later `fs::copy` into `commands/repositories.yaml` fail,
    // since its parent cannot be a directory.
    std::fs::write(paths.workspace.join("commands"), b"blocking file").unwrap();

    let before = snapshot(&paths);
    let output = run_init(&paths, "json");
    assert_eq!(
        output.status.code(),
        Some(3),
        "stdout: {}",
        stdout_of(&output)
    );
    assert!(stdout_of(&output).is_empty());
    assert!(!stderr_of(&output).is_empty());

    // Remove the directory we deliberately planted so the snapshot
    // comparison is meaningful (it is not something `init` created, so it
    // must still be exactly there — comparing snapshots that both include
    // it is the actual proof).
    let after = snapshot(&paths);
    assert_eq!(
        before, after,
        "init must restore exactly the pre-call state on a mid-copy failure"
    );
    assert_eq!(
        std::fs::read(paths.workspace.join("user-file.txt")).unwrap(),
        b"pre-existing"
    );
}

#[test]
fn init_rolls_back_completely_when_the_tool_database_cannot_be_created() {
    let paths = InitPaths::new("init-fail-tool-db");
    // Tool-db path is itself a pre-existing directory, so opening it as a
    // SQLite file necessarily fails, before any template file is copied.
    std::fs::create_dir_all(&paths.tool_db).unwrap();

    let output = run_init(&paths, "json");
    assert_eq!(output.status.code(), Some(3));
    assert!(stdout_of(&output).is_empty());
    assert!(!stderr_of(&output).is_empty());

    // Nothing this call would have created is left behind: no workspace
    // directory, no copied files, no workspace database. The pre-existing
    // tool-db directory (not created by this call) is untouched.
    assert!(
        !paths.workspace.exists(),
        "workspace directory must not survive a rolled-back init"
    );
    assert!(!paths.workspace_db.exists());
    assert!(
        paths.tool_db.is_dir(),
        "the pre-existing tool-db path must be left exactly as it was"
    );
}

#[test]
fn init_rolls_back_completely_when_the_workspace_database_fails_after_a_successful_template_copy() {
    let paths = InitPaths::new("init-fail-workspace-db");
    // Workspace-db path is itself a pre-existing directory, so the template
    // copy and the tool database both succeed before this fails.
    std::fs::create_dir_all(&paths.workspace_db).unwrap();

    let output = run_init(&paths, "json");
    assert_eq!(output.status.code(), Some(3));
    assert!(stdout_of(&output).is_empty());
    assert!(!stderr_of(&output).is_empty());

    assert!(
        !paths.workspace.exists(),
        "the freshly created workspace directory and its copied template files must be rolled back"
    );
    assert!(
        !paths.tool_db.exists(),
        "the freshly created tool database must be removed when the workspace database fails to open"
    );
    assert!(
        paths.workspace_db.is_dir(),
        "the pre-existing workspace-db path must be left exactly as it was"
    );
}

#[test]
fn init_fails_without_partial_state_when_the_workspace_database_cannot_be_created() {
    let paths = InitPaths::new("init-partial-failure");
    // Make the workspace-db path itself a directory, so opening it as a
    // SQLite file necessarily fails.
    std::fs::create_dir_all(&paths.workspace_db).unwrap();

    let output = run_init(&paths, "json");
    assert_eq!(output.status.code(), Some(3));
    assert!(stdout_of(&output).is_empty());
    assert!(!stderr_of(&output).is_empty());

    // The tool database this run created must have been rolled back: no
    // half-finished pair is left behind.
    assert!(
        !paths.tool_db.exists(),
        "a freshly created tool database must be removed when the workspace database fails to open"
    );
}

/// A real, OS-level fault injection: caps the child process's maximum file
/// size below the size of one of the template files it is about to copy,
/// via `sh`'s own `ulimit -f` and `trap '' XFSZ` builtins — never
/// `unsafe` Rust (the workspace forbids `unsafe_code`, `Cargo.toml`
/// `[workspace.lints.rust]`). `trap '' XFSZ` sets `SIGXFSZ`'s disposition to
/// ignored in the shell itself; POSIX preserves an explicitly ignored
/// disposition across `exec`, so once the shell `exec`s the real `meridian`
/// binary in its own place, an oversized write there reports back as a
/// normal `EFBIG` `io::Error` (`crate::kernel::copy_instance_template`'s
/// write path) instead of killing the process by signal — the same failure
/// shape a full disk or a filesystem quota produces. This exercises the
/// exact defect item 3 of the second `CHANGES_REQUESTED` round named: a
/// destination file left partially written on disk by a failed
/// `fs::copy`/`io::copy` must still be found and removed, both by
/// `copy_instance_template`'s own immediate cleanup and by the caller's
/// journal-driven rollback, byte-for-byte restoring the pre-call tree —
/// never approximated with a simulated error return.
#[cfg(unix)]
#[test]
fn init_rolls_back_a_partial_file_left_by_a_real_write_failure_mid_copy() {
    // A synthetic Kernel, not the real repository: this test needs exact
    // control over file sizes and copy order so the induced write failure
    // lands mid-copy of a specific file, after an earlier file in the same
    // `instance-template/` has already copied through in full.
    let kernel_guard = TempDir::new("init-fault-kernel");
    let kernel = kernel_guard.path().to_path_buf();
    std::fs::write(kernel.join("VERSION"), b"0.0.0-fault-injection\n").unwrap();
    let template = kernel.join("instance-template");
    std::fs::create_dir_all(&template).unwrap();
    // Sorts before "z-big.bin" (`copy_instance_template` copies in sorted
    // order), so it copies through completely before the induced limit is
    // ever hit.
    std::fs::write(template.join("a-small.txt"), b"ok\n").unwrap();
    // Exceeds the 1024-byte process file-size limit set below, so its write
    // fails partway through — not at `create_new`, not on the first byte.
    std::fs::write(template.join("z-big.bin"), vec![b'A'; 4096]).unwrap();

    let paths = InitPaths::new("init-fault-injection");
    std::fs::create_dir_all(&paths.workspace).unwrap();
    std::fs::write(paths.workspace.join("user-file.txt"), b"pre-existing").unwrap();
    let before = snapshot(&paths);

    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(r#"trap '' XFSZ; ulimit -f 2; exec "$0" "$@""#)
        .arg(exe())
        .args([
            "init",
            "--kernel",
            kernel.to_str().unwrap(),
            "--workspace",
            paths.workspace.to_str().unwrap(),
            "--tool-db",
            paths.tool_db.to_str().unwrap(),
            "--workspace-db",
            paths.workspace_db.to_str().unwrap(),
            "--format",
            "json",
        ]);
    let output = command
        .output()
        .expect("run meridian binary under sh -c ulimit -f");

    assert_eq!(
        output.status.code(),
        Some(3),
        "stdout: {} stderr: {}",
        stdout_of(&output),
        stderr_of(&output)
    );
    assert!(stdout_of(&output).is_empty());
    assert!(!stderr_of(&output).is_empty());

    let after = snapshot(&paths);
    assert_eq!(
        before, after,
        "a real write failure partway through one file must still roll back byte-for-byte \
         to the pre-call state, including the sibling file that had already copied through \
         completely before the failure"
    );
    assert_eq!(
        std::fs::read(paths.workspace.join("user-file.txt")).unwrap(),
        b"pre-existing"
    );
    assert!(!paths.tool_db.exists());
    assert!(!paths.workspace_db.exists());
}

#[test]
fn doctor_reports_healthy_after_a_successful_init() {
    let paths = InitPaths::new("doctor-healthy");
    assert_eq!(run_init(&paths, "json").status.code(), Some(0));

    let kernel = kernel_root();
    let output = run(&[
        "doctor",
        "--kernel",
        kernel.to_str().unwrap(),
        "--workspace",
        paths.workspace.to_str().unwrap(),
        "--tool-db",
        paths.tool_db.to_str().unwrap(),
        "--workspace-db",
        paths.workspace_db.to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        stderr_of(&output)
    );
    let value = json_result(&output);
    assert_eq!(value["result"]["healthy"], true);
    assert_eq!(value["result"]["tool_db"]["role"], "tool");
    assert_eq!(value["result"]["workspace_db"]["role"], "workspace");
}

#[test]
fn doctor_rejects_swapped_database_roles_instead_of_guessing_from_the_path() {
    let paths = InitPaths::new("doctor-swapped");
    assert_eq!(run_init(&paths, "json").status.code(), Some(0));

    let kernel = kernel_root();
    // Point --tool-db at the actual workspace database and vice versa.
    let output = run(&[
        "doctor",
        "--kernel",
        kernel.to_str().unwrap(),
        "--workspace",
        paths.workspace.to_str().unwrap(),
        "--tool-db",
        paths.workspace_db.to_str().unwrap(),
        "--workspace-db",
        paths.tool_db.to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let value = json_result(&output);
    assert_eq!(value["result"]["healthy"], false);
    assert_eq!(value["result"]["tool_db"]["ok"], false);
    assert_eq!(value["result"]["workspace_db"]["ok"], false);
}

#[test]
fn doctor_reports_a_corrupt_database_file_explicitly() {
    let paths = InitPaths::new("doctor-corrupt");
    std::fs::create_dir_all(&paths.workspace).unwrap();
    std::fs::write(&paths.tool_db, b"this is not a sqlite file").unwrap();
    std::fs::write(&paths.workspace_db, b"this is not a sqlite file either").unwrap();

    let kernel = kernel_root();
    let output = run(&[
        "doctor",
        "--kernel",
        kernel.to_str().unwrap(),
        "--workspace",
        paths.workspace.to_str().unwrap(),
        "--tool-db",
        paths.tool_db.to_str().unwrap(),
        "--workspace-db",
        paths.workspace_db.to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let value = json_result(&output);
    assert_eq!(value["result"]["tool_db"]["ok"], false);
}

#[test]
fn doctor_reports_a_missing_workspace_directory() {
    let paths = InitPaths::new("doctor-missing-workspace");
    assert_eq!(run_init(&paths, "json").status.code(), Some(0));
    std::fs::remove_dir_all(&paths.workspace).unwrap();

    let kernel = kernel_root();
    let output = run(&[
        "doctor",
        "--kernel",
        kernel.to_str().unwrap(),
        "--workspace",
        paths.workspace.to_str().unwrap(),
        "--tool-db",
        paths.tool_db.to_str().unwrap(),
        "--workspace-db",
        paths.workspace_db.to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let value = json_result(&output);
    assert_eq!(value["result"]["workspace"]["ok"], false);
    assert_eq!(value["result"]["healthy"], false);
}

/// Item 6: a database that opens and passes its role/edition check but
/// cannot report its own `schema_version` must be reported unhealthy with an
/// explicit diagnostic — never `ok: true` with a silently absent
/// `schema_version`. Corrupts the tool database's `schema_migrations` table
/// directly (via a raw SQLite connection, `rusqlite` — already a workspace
/// dependency of `meridian-storage-sqlite`), after a normal `init`, so the
/// database still opens (its role/edition metadata table is intact) but
/// `schema_version()`'s own query fails.
#[test]
fn doctor_reports_unhealthy_when_schema_version_cannot_be_read() {
    let paths = InitPaths::new("doctor-schema-version-error");
    assert_eq!(run_init(&paths, "json").status.code(), Some(0));

    {
        let conn = rusqlite::Connection::open(&paths.tool_db).unwrap();
        conn.execute("DROP TABLE schema_migrations", []).unwrap();
    }

    let kernel = kernel_root();
    let output = run(&[
        "doctor",
        "--kernel",
        kernel.to_str().unwrap(),
        "--workspace",
        paths.workspace.to_str().unwrap(),
        "--tool-db",
        paths.tool_db.to_str().unwrap(),
        "--workspace-db",
        paths.workspace_db.to_str().unwrap(),
        "--format",
        "json",
    ]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout: {}",
        stdout_of(&output)
    );
    let value = json_result(&output);
    assert_eq!(value["result"]["healthy"], false);
    assert_eq!(value["result"]["tool_db"]["ok"], false);
    assert!(value["result"]["tool_db"]["schema_version"].is_null());
    assert!(
        value["result"]["tool_db"]["error"]
            .as_str()
            .unwrap()
            .contains("schema_version"),
        "{:?}",
        value["result"]["tool_db"]
    );
    // Human output must also carry an explicit diagnostic, not a silent gap.
    let human = run(&[
        "doctor",
        "--kernel",
        kernel.to_str().unwrap(),
        "--workspace",
        paths.workspace.to_str().unwrap(),
        "--tool-db",
        paths.tool_db.to_str().unwrap(),
        "--workspace-db",
        paths.workspace_db.to_str().unwrap(),
    ]);
    assert!(stdout_of(&human).contains("FAIL"));
}

#[test]
fn export_produces_an_empty_deterministic_array_right_after_init() {
    let paths = InitPaths::new("export-empty");
    assert_eq!(run_init(&paths, "json").status.code(), Some(0));

    let kernel = kernel_root();
    let run_once = || {
        let output = run(&[
            "export",
            "--kernel",
            kernel.to_str().unwrap(),
            "--tool-db",
            paths.tool_db.to_str().unwrap(),
            "--workspace-db",
            paths.workspace_db.to_str().unwrap(),
            "--format",
            "json",
        ]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "stderr: {}",
            stderr_of(&output)
        );
        stdout_of(&output)
    };
    let first = run_once();
    let second = run_once();
    assert_eq!(
        first, second,
        "repeated export of unchanged state must be byte-identical"
    );
    let value: serde_json::Value = serde_json::from_str(first.trim()).unwrap();
    assert_eq!(value["result"], serde_json::json!([]));
}

#[test]
fn export_reports_a_missing_database_as_an_environment_error() {
    let paths = InitPaths::new("export-missing-db");
    std::fs::create_dir_all(paths.tool_db.parent().unwrap()).unwrap();
    let kernel = kernel_root();
    let output = run(&[
        "export",
        "--kernel",
        kernel.to_str().unwrap(),
        "--tool-db",
        paths.tool_db.to_str().unwrap(),
        "--workspace-db",
        paths.workspace_db.to_str().unwrap(),
    ]);
    assert_eq!(output.status.code(), Some(3));
    assert!(stdout_of(&output).is_empty());
    assert!(!stderr_of(&output).is_empty());
}
