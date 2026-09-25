//! Real-binary tests of package `meridian-cli-migration`
//! (`meridian-rust-migration-program-plan.md` §5.22): `import` and
//! `migration plan|apply|verify|rollback` over a REAL Git source built from
//! the deterministic synthetic fixture `tests/fixtures/frozen-instance`
//! (see its `generate.mjs`), plus — when `MERIDIAN_INSTANCE` names the
//! local frozen source — the same routes over the accepted real bundle
//! (`#[ignore]`d by default; the production binary never reads that
//! variable, only this test harness does, to locate its input).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

use super::{exe, json_result, kernel_root, run, stderr_of, stdout_of, TempDir};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/frozen-instance")
}

fn expected() -> Value {
    serde_json::from_str(&std::fs::read_to_string(fixture().join("expected.json")).unwrap())
        .unwrap()
}

const COMMIT_ENV: [(&str, &str); 6] = [
    ("GIT_AUTHOR_NAME", "Meridian Fixture"),
    ("GIT_AUTHOR_EMAIL", "fixture@meridian.invalid"),
    ("GIT_AUTHOR_DATE", "2026-01-01T00:00:00+00:00"),
    ("GIT_COMMITTER_NAME", "Meridian Fixture"),
    ("GIT_COMMITTER_EMAIL", "fixture@meridian.invalid"),
    ("GIT_COMMITTER_DATE", "2026-01-01T00:00:00+00:00"),
];

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "core.autocrlf=false",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .envs(COMMIT_ENV)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn copy_tree(from: &Path, to: &Path) {
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            std::fs::create_dir_all(&target).unwrap();
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// A Git source repository: the pinned fixture commit, then one commit
/// carrying the bundle (`variant` of it, then `edit`ed).
struct Source {
    _guard: TempDir,
    repo: PathBuf,
}

impl Source {
    fn new(label: &str, variant: Option<&str>, edit: impl FnOnce(&Path)) -> Source {
        let guard = TempDir::new(label);
        let repo = guard.path().join("source");
        std::fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q"]);
        copy_tree(&fixture().join("source"), &repo);
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "frozen instance fixture"]);
        let head = git(&repo, &["rev-parse", "HEAD"]);
        assert_eq!(
            head.trim(),
            expected()["revision"].as_str().unwrap(),
            "the fixture commit must be reproducible"
        );
        let bundle = repo.join("migration/instance-data");
        std::fs::create_dir_all(&bundle).unwrap();
        let from = match variant {
            None => fixture().join("bundle"),
            Some(v) => fixture().join("variants").join(v),
        };
        copy_tree(&from, &bundle);
        edit(&bundle);
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "accepted bundle"]);
        Source {
            _guard: guard,
            repo,
        }
    }

    fn path(&self) -> &str {
        self.repo.to_str().unwrap()
    }
}

/// A freshly initialised pair of databases.
struct Dbs {
    _guard: TempDir,
    tool: PathBuf,
    workspace: PathBuf,
}

impl Dbs {
    fn new(label: &str) -> Dbs {
        let guard = TempDir::new(label);
        let base = guard.path().to_path_buf();
        let dbs = Dbs {
            _guard: guard,
            tool: base.join("tool.sqlite3"),
            workspace: base.join("workspace.sqlite3"),
        };
        let kernel = kernel_root();
        let output = run(&[
            "init",
            "--kernel",
            kernel.to_str().unwrap(),
            "--workspace",
            base.join("workspace").to_str().unwrap(),
            "--tool-db",
            dbs.tool.to_str().unwrap(),
            "--workspace-db",
            dbs.workspace.to_str().unwrap(),
            "--format",
            "json",
        ]);
        assert_eq!(output.status.code(), Some(0), "{}", stderr_of(&output));
        dbs
    }

    /// A byte-for-byte copy of `other`'s two database files (SQLite files
    /// carry creation timestamps, so two separately initialised databases
    /// are never byte-identical).
    fn copy_of(other: &Dbs, label: &str) -> Dbs {
        let guard = TempDir::new(label);
        let base = guard.path().to_path_buf();
        let dbs = Dbs {
            _guard: guard,
            tool: base.join("tool.sqlite3"),
            workspace: base.join("workspace.sqlite3"),
        };
        std::fs::copy(&other.tool, &dbs.tool).unwrap();
        std::fs::copy(&other.workspace, &dbs.workspace).unwrap();
        dbs
    }

    fn tool(&self) -> &str {
        self.tool.to_str().unwrap()
    }

    fn ws(&self) -> &str {
        self.workspace.to_str().unwrap()
    }

    fn checkpoints(&self) -> Vec<String> {
        let dir = self
            .workspace
            .with_file_name("workspace.sqlite3.checkpoints");
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }
}

/// Everything a migration can change in one database, read raw.
#[derive(Debug, PartialEq, Eq)]
struct DbState {
    records: Vec<(String, String)>,
    revisions: i64,
    runs: Vec<(String, String)>,
    schema_version: i64,
    metadata: (String, String),
}

fn db_state(path: &Path) -> DbState {
    let conn =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let pairs = |sql: &str| -> Vec<(String, String)> {
        let mut stmt = conn.prepare(sql).unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    DbState {
        records: pairs("SELECT record_key, content_digest FROM records ORDER BY record_key"),
        revisions: conn
            .query_row("SELECT COUNT(*) FROM record_revisions", [], |r| r.get(0))
            .unwrap(),
        runs: pairs(
            "SELECT r.migration_run_id,
                    CASE WHEN b.migration_run_id IS NULL THEN 'applied' ELSE 'rolled-back' END
             FROM migration_runs r LEFT JOIN migration_rollbacks b USING (migration_run_id)
             ORDER BY r.run_sequence",
        ),
        schema_version: conn
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| {
                r.get(0)
            })
            .unwrap(),
        metadata: conn
            .query_row(
                "SELECT role, kernel_edition FROM database_metadata WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap(),
    }
}

/// The raw journal of one database: every `migration_runs` row as
/// `(id, sequence, post-state digest, recorded_at)` and every
/// `migration_rollbacks` row as `(id, restored digest, recorded_at)`.
#[allow(clippy::type_complexity)]
fn journal(
    path: &Path,
) -> (
    Vec<(String, i64, String, String)>,
    Vec<(String, String, String)>,
) {
    let conn =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let mut stmt = conn
        .prepare(
            "SELECT migration_run_id, run_sequence, post_state_digest, recorded_at
             FROM migration_runs ORDER BY migration_run_id",
        )
        .unwrap();
    let runs = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let mut stmt = conn
        .prepare(
            "SELECT migration_run_id, restored_state_digest, recorded_at
             FROM migration_rollbacks ORDER BY migration_run_id",
        )
        .unwrap();
    let rollbacks = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    (runs, rollbacks)
}

fn kernel() -> String {
    kernel_root().to_str().unwrap().to_string()
}

fn fingerprint() -> String {
    expected()["plan_fingerprint"].as_str().unwrap().to_string()
}

fn plan(source: &Source, format: &str) -> Output {
    run(&[
        "migration",
        "plan",
        "--kernel",
        &kernel(),
        "--source",
        source.path(),
        "--format",
        format,
    ])
}

fn apply(source: &Source, dbs: &Dbs, extra: &[&str]) -> Output {
    let k = kernel();
    let mut args = vec![
        "migration",
        "apply",
        "--kernel",
        &k,
        "--source",
        source.path(),
        "--workspace-db",
        dbs.ws(),
        "--format",
        "json",
    ];
    args.extend_from_slice(extra);
    run(&args)
}

fn apply_confirmed(source: &Source, dbs: &Dbs) -> Output {
    let fp = fingerprint();
    apply(source, dbs, &["--dry-run", "false", "--confirm", &fp])
}

fn verify(source: &Source, dbs: &Dbs) -> Output {
    run(&[
        "migration",
        "verify",
        "--kernel",
        &kernel(),
        "--source",
        source.path(),
        "--workspace-db",
        dbs.ws(),
        "--format",
        "json",
    ])
}

fn rollback(source: &Source, dbs: &Dbs, run_id: &str, confirm: &str) -> Output {
    run(&[
        "migration",
        "rollback",
        "--kernel",
        &kernel(),
        "--source",
        source.path(),
        "--workspace-db",
        dbs.ws(),
        "--run",
        run_id,
        "--confirm",
        confirm,
        "--format",
        "json",
    ])
}

fn import_frozen(source: &Source, dbs: &Dbs, confirm: &str) -> Output {
    run(&[
        "import",
        "--kernel",
        &kernel(),
        "--kind",
        "frozen-instance",
        "--source",
        source.path(),
        "--tool-db",
        dbs.tool(),
        "--workspace-db",
        dbs.ws(),
        "--confirm",
        confirm,
        "--format",
        "json",
    ])
}

fn export(dbs: &Dbs) -> Output {
    let output = run(&[
        "export",
        "--kernel",
        &kernel(),
        "--tool-db",
        dbs.tool(),
        "--workspace-db",
        dbs.ws(),
        "--format",
        "json",
    ]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr_of(&output));
    output
}

fn import_canonical(input: &Path, dbs: &Dbs, confirm: &str) -> Output {
    run(&[
        "import",
        "--kernel",
        &kernel(),
        "--kind",
        "canonical-records",
        "--input",
        input.to_str().unwrap(),
        "--tool-db",
        dbs.tool(),
        "--workspace-db",
        dbs.ws(),
        "--confirm",
        confirm,
        "--format",
        "json",
    ])
}

fn sha256_hex(bytes: &[u8]) -> String {
    meridian_core::types::ContentDigest::of_bytes(bytes)
        .value()
        .to_string()
}

fn assert_code(output: &Output, code: i32) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "stdout: {}\nstderr: {}",
        stdout_of(output),
        stderr_of(output)
    );
}

fn assert_usage(args: &[&str]) {
    let output = run(args);
    assert_code(&output, 2);
    assert!(stdout_of(&output).is_empty(), "{args:?}");
    assert!(!stderr_of(&output).is_empty(), "{args:?}");
}

// ---------------------------------------------------------------------------
// Grammar, usage and exit codes
// ---------------------------------------------------------------------------

#[test]
fn migration_help_lists_the_five_new_commands() {
    let output = run(&["--help"]);
    assert_code(&output, 0);
    let usage = stdout_of(&output);
    for needle in [
        "import    --kernel <path> --kind frozen-instance",
        "import    --kernel <path> --kind canonical-records",
        "migration plan",
        "migration apply",
        "migration verify",
        "migration rollback",
    ] {
        assert!(usage.contains(needle), "{needle}");
    }
}

#[test]
fn migration_usage_errors_are_closed_and_write_no_stdout() {
    let k = kernel();
    assert_usage(&["migration"]);
    assert_usage(&["migration", "bogus", "--kernel", &k]);
    assert_usage(&["migration", "plan", "--kernel", &k]);
    assert_usage(&[
        "migration",
        "plan",
        "--kernel",
        &k,
        "--source",
        "x",
        "--workspace-db",
        "y",
    ]);
    for value in ["yes", "1", "TRUE", ""] {
        assert_usage(&[
            "migration",
            "apply",
            "--kernel",
            &k,
            "--source",
            "x",
            "--workspace-db",
            "y",
            "--dry-run",
            value,
        ]);
    }
    assert_usage(&[
        "migration",
        "apply",
        "--kernel",
        &k,
        "--source",
        "x",
        "--workspace-db",
        "y",
        "--dry-run",
        "false",
    ]);
    assert_usage(&[
        "migration",
        "rollback",
        "--kernel",
        &k,
        "--source",
        "x",
        "--workspace-db",
        "y",
        "--run",
        "not-a-run",
        "--confirm",
        "not-a-run",
    ]);
    for kind in ["frozen", "canonical", "auto", ""] {
        assert_usage(&[
            "import",
            "--kernel",
            &k,
            "--kind",
            kind,
            "--source",
            "x",
            "--tool-db",
            "t",
            "--workspace-db",
            "w",
            "--confirm",
            "c",
        ]);
    }
    assert_usage(&[
        "import",
        "--kernel",
        &k,
        "--kind",
        "frozen-instance",
        "--source",
        "x",
        "--input",
        "i",
        "--tool-db",
        "t",
        "--workspace-db",
        "w",
        "--confirm",
        "c",
    ]);
    assert_usage(&[
        "import",
        "--kernel",
        &k,
        "--kind",
        "canonical-records",
        "--input",
        "i",
        "--source",
        "x",
        "--tool-db",
        "t",
        "--workspace-db",
        "w",
        "--confirm",
        "c",
    ]);
    assert_usage(&[
        "import",
        "--kernel",
        &k,
        "--kind",
        "frozen-instance",
        "--source",
        "x",
        "--tool-db",
        "t",
        "--workspace-db",
        "w",
    ]);
}

// ---------------------------------------------------------------------------
// plan
// ---------------------------------------------------------------------------

#[test]
fn migration_plan_accepts_the_bundle_through_the_accepted_operations() {
    let source = Source::new("plan", None, |_| {});
    let output = plan(&source, "json");
    assert_code(&output, 0);
    assert!(stderr_of(&output).is_empty(), "{}", stderr_of(&output));
    let doc = json_result(&output);
    assert_eq!(doc["status"], "ok");
    assert_eq!(doc["command"], "migration plan");
    let r = &doc["result"];
    let e = expected();
    assert_eq!(r["verdict"], "accepted");
    assert_eq!(r["source"]["revision"], e["revision"]);
    assert_eq!(r["source"]["digest"]["value"], e["tree_digest"]);
    assert_eq!(
        r["plan"]["plan_fingerprint"]["value"],
        e["plan_fingerprint"]
    );
    assert_eq!(r["plan"]["idempotency_key"]["value"], e["idempotency_key"]);
    assert_eq!(r["plan"]["counts"]["record_units"], 6);
    assert_eq!(r["plan"]["counts"]["migrated"], 5);
    assert_eq!(r["plan"]["counts"]["retained"], 1);
    assert_eq!(r["plan"]["counts"]["merged"], 0);
    assert_eq!(r["canonical_export"]["digest"]["value"], e["export_digest"]);
    // Deterministic, human and JSON agree on the verdict.
    assert_eq!(plan(&source, "json").stdout, output.stdout);
    let human = plan(&source, "human");
    assert_code(&human, 0);
    assert!(stdout_of(&human).contains("Bundle ACCEPTED"));
}

/// M-05b (`rust-workspace-state-validation`): a bundle every Node check
/// accepts, whose one target declares a record type no payload contract
/// names, is refused by `migration plan` and by `import --kind
/// frozen-instance` alike — before any database is written.
#[test]
fn workspace_state_frozen_import_refuses_a_target_no_payload_contract_accepts() {
    let source = Source::new("plan-unknown-type", Some("unknown-record-type"), |_| {});
    let output = plan(&source, "json");
    assert_code(&output, 1);
    let r = json_result(&output)["result"].clone();
    assert_eq!(r["verdict"], "rejected");
    assert_eq!(
        r["diagnostics"],
        serde_json::json!([
            "payload-contract: target \"inventory-items-alpha\" (inventory-item): record_type \"inventory-item\" has no payload contract; a product record type is added to the registry by decision, never accepted unchecked"
        ])
    );

    let dbs = Dbs::new("import-unknown-type");
    let before = export(&dbs).stdout;
    let output = import_frozen(&source, &dbs, &fingerprint());
    assert_code(&output, 1);
    assert!(stdout_of(&output).contains("has no payload contract"));
    assert_eq!(export(&dbs).stdout, before, "nothing may be written");
}

#[test]
fn migration_plan_fails_closed_on_every_foreign_source_state() {
    // Wrong source digest: internally consistent, but not the pinned tree.
    let source = Source::new("plan-digest", Some("wrong-digest"), |_| {});
    let output = plan(&source, "json");
    assert_code(&output, 1);
    let r = json_result(&output)["result"].clone();
    assert_eq!(r["verdict"], "rejected");
    assert!(r["diagnostics"][0]
        .as_str()
        .unwrap()
        .contains("tree of revision"));

    // Source-content mismatch: a declared payload the pinned revision does
    // not carry byte for byte.
    let source = Source::new("plan-content", Some("content-mismatch"), |_| {});
    let output = plan(&source, "json");
    assert_code(&output, 1);
    let text = stdout_of(&output);
    assert!(text.contains("byte-for-byte"), "{text}");

    // A built-in-methodology target never reaches a write.
    let source = Source::new("plan-built-in", Some("built-in-target"), |_| {});
    assert_code(&plan(&source, "json"), 1);

    // Plan substitution: one target changed under the same plan id.
    let source = Source::new("plan-substitution", None, |bundle| {
        let path = bundle.join("registry.json");
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, text.replacen("Item alpha", "Item substituted", 1)).unwrap();
    });
    let output = plan(&source, "json");
    assert_code(&output, 1);
    let r = json_result(&output)["result"].clone();
    assert_ne!(
        r["plan"]["plan_fingerprint"]["value"],
        expected()["plan_fingerprint"]
    );
    assert!(stdout_of(&output).contains("stale or different version"));

    // A stale export pin.
    let source = Source::new("plan-stale-export", None, |bundle| {
        let path = bundle.join("canonical-export.json");
        let text = std::fs::read_to_string(&path).unwrap();
        let fp = fingerprint();
        std::fs::write(&path, text.replacen(&fp, &"0".repeat(64), 1)).unwrap();
    });
    let output = plan(&source, "json");
    assert_code(&output, 1);
    assert!(stdout_of(&output).contains("plan_fingerprint"));

    // A missing Git revision: the bundle pins a commit this repository
    // does not have.
    let guard = TempDir::new("plan-missing-revision");
    let repo = guard.path().join("source");
    std::fs::create_dir_all(repo.join("migration/instance-data")).unwrap();
    git(&repo, &["init", "-q"]);
    copy_tree(
        &fixture().join("bundle"),
        &repo.join("migration/instance-data"),
    );
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "bundle without its source"]);
    let output = run(&[
        "migration",
        "plan",
        "--kernel",
        &kernel(),
        "--source",
        repo.to_str().unwrap(),
    ]);
    assert_code(&output, 3);
    assert!(stdout_of(&output).is_empty());
    assert!(
        stderr_of(&output).contains("does not exist"),
        "{}",
        stderr_of(&output)
    );

    // Not a repository at all.
    let output = run(&[
        "migration",
        "plan",
        "--kernel",
        &kernel(),
        "--source",
        guard.path().to_str().unwrap(),
    ]);
    assert_code(&output, 3);
    assert!(stdout_of(&output).is_empty());
}

// ---------------------------------------------------------------------------
// apply / verify / idempotency / dry run / confirmation
// ---------------------------------------------------------------------------

#[test]
fn migration_dry_run_and_wrong_or_missing_confirmation_change_nothing() {
    let source = Source::new("dry-run", None, |_| {});
    let dbs = Dbs::new("dry-run");
    let before = std::fs::read(&dbs.workspace).unwrap();

    for extra in [&[][..], &["--dry-run", "true"][..]] {
        let output = apply(&source, &dbs, extra);
        assert_code(&output, 0);
        let r = json_result(&output)["result"].clone();
        assert_eq!(r["dry_run"], true);
        assert_eq!(r["status"], "dry-run");
        assert_eq!(r["effects"]["created"], 5);
        assert_eq!(r["run_id"], Value::Null);
    }
    let output = apply(&source, &dbs, &["--confirm", "0000"]);
    assert_code(&output, 1);
    assert_eq!(
        json_result(&output)["result"]["status"],
        "confirmation-mismatch"
    );
    let output = apply(&source, &dbs, &["--dry-run", "false", "--confirm", "0000"]);
    assert_code(&output, 1);
    assert_eq!(
        json_result(&output)["result"]["status"],
        "confirmation-mismatch"
    );
    assert_code(&import_frozen(&source, &dbs, "0000"), 1);

    assert_eq!(std::fs::read(&dbs.workspace).unwrap(), before);
    assert!(dbs.checkpoints().is_empty());

    // A dry run never brings a database into existence.
    let guard = TempDir::new("dry-run-missing");
    let missing = guard.path().join("absent.sqlite3");
    let output = run(&[
        "migration",
        "apply",
        "--kernel",
        &kernel(),
        "--source",
        source.path(),
        "--workspace-db",
        missing.to_str().unwrap(),
    ]);
    assert_code(&output, 3);
    assert!(!missing.exists());
}

#[test]
fn migration_import_frozen_reports_a_wrong_confirmation_before_touching_any_database() {
    let source = Source::new("confirm-first", None, |_| {});
    let guard = TempDir::new("confirm-first-dbs");
    let missing_tool = guard.path().join("missing-tool.sqlite3");
    let missing_ws = guard.path().join("missing-workspace.sqlite3");
    let corrupt_tool = guard.path().join("corrupt-tool.sqlite3");
    let corrupt_bytes = b"this is not a sqlite database at all";
    std::fs::write(&corrupt_tool, corrupt_bytes).unwrap();
    let dbs = Dbs::new("confirm-first-valid");
    let ws_before = std::fs::read(&dbs.workspace).unwrap();

    let import = |tool: &Path, ws: &Path, confirm: &str| {
        run(&[
            "import",
            "--kernel",
            &kernel(),
            "--kind",
            "frozen-instance",
            "--source",
            source.path(),
            "--tool-db",
            tool.to_str().unwrap(),
            "--workspace-db",
            ws.to_str().unwrap(),
            "--confirm",
            confirm,
            "--format",
            "json",
        ])
    };
    let checkpoint_dir = |db: &Path| {
        let mut name = db.file_name().unwrap().to_os_string();
        name.push(".checkpoints");
        db.with_file_name(name)
    };

    // Wrong confirmation + missing tool DB (+ missing workspace DB), and
    // wrong confirmation + corrupt tool DB (+ valid workspace DB): both are
    // a confirmation mismatch, exit 1, and create no database or checkpoint.
    for (tool, ws) in [
        (&missing_tool, &missing_ws),
        (&corrupt_tool, &dbs.workspace),
    ] {
        let output = import(tool, ws, "0000");
        assert_code(&output, 1);
        assert!(stderr_of(&output).is_empty(), "{}", stderr_of(&output));
        let r = json_result(&output)["result"].clone();
        assert_eq!(r["apply"]["status"], "confirmation-mismatch", "{r}");
        assert_eq!(r["status"], "confirmation-mismatch", "{r}");
        assert_eq!(r["verify"], Value::Null);
        assert!(!checkpoint_dir(tool).exists());
        assert!(!checkpoint_dir(ws).exists());
    }
    assert!(!missing_tool.exists());
    assert!(!missing_ws.exists());
    assert_eq!(std::fs::read(&corrupt_tool).unwrap(), corrupt_bytes);
    assert_eq!(std::fs::read(&dbs.workspace).unwrap(), ws_before);
    assert!(dbs.checkpoints().is_empty());

    // With the matching confirmation the tool database IS verified: a
    // missing or corrupt one is an environment error (exit 3) that leaves
    // the workspace database untouched.
    for tool in [&missing_tool, &corrupt_tool] {
        let output = import(tool, &dbs.workspace, &fingerprint());
        assert_code(&output, 3);
        assert!(stdout_of(&output).is_empty());
        assert!(
            stderr_of(&output).contains("tool database"),
            "{}",
            stderr_of(&output)
        );
    }
    assert!(!missing_tool.exists());
    assert_eq!(std::fs::read(&dbs.workspace).unwrap(), ws_before);
    assert!(dbs.checkpoints().is_empty());
}

#[test]
fn migration_apply_verify_and_every_repeat_are_idempotent() {
    let source = Source::new("apply", None, |_| {});
    let dbs = Dbs::new("apply");

    let output = apply_confirmed(&source, &dbs);
    assert_code(&output, 0);
    let r = json_result(&output)["result"].clone();
    assert_eq!(r["status"], "applied");
    assert_eq!(r["effects"]["created"], 5);
    assert_eq!(r["retained"], 1);
    let run_id = r["run_id"].as_str().unwrap().to_string();
    assert!(r["checkpoint_digest"]["value"].is_string());

    let output = verify(&source, &dbs);
    assert_code(&output, 0);
    let v = json_result(&output)["result"].clone();
    assert_eq!(v["status"], "verified");
    for (field, value) in [
        ("expected", 5),
        ("imported", 5),
        ("missing", 0),
        ("extra", 0),
        ("changed", 0),
        ("duplicate", 0),
        ("retained", 1),
    ] {
        assert_eq!(v[field], value, "{field}");
    }
    assert_eq!(v["applicability"]["verdict"], "equivalent");
    assert_eq!(v["applicability"]["controlled"], 2);
    assert_eq!(v["applicability"]["imported"], 2);

    let state = db_state(&dbs.workspace);
    let exported = export(&dbs).stdout;
    let checkpoints = dbs.checkpoints();
    assert_eq!(checkpoints, vec![format!("{run_id}.sqlite3")]);

    let output = apply_confirmed(&source, &dbs);
    assert_code(&output, 0);
    let r = json_result(&output)["result"].clone();
    assert_eq!(r["status"], "already-applied");
    assert_eq!(r["already_applied"], true);
    assert_eq!(r["run_id"], run_id.as_str());

    let output = import_frozen(&source, &dbs, &fingerprint());
    assert_code(&output, 0);
    let i = json_result(&output)["result"].clone();
    assert_eq!(i["kind"], "frozen-instance");
    assert_eq!(i["apply"]["status"], "already-applied");
    assert_eq!(i["verify"]["status"], "verified");
    assert_eq!(i["input_digest"]["value"], fingerprint().as_str());

    let output = apply(&source, &dbs, &[]);
    assert_code(&output, 0);
    assert_eq!(
        json_result(&output)["result"]["status"],
        "dry-run-already-applied"
    );

    assert_eq!(db_state(&dbs.workspace), state);
    assert_eq!(export(&dbs).stdout, exported);
    assert_eq!(dbs.checkpoints(), checkpoints);
}

#[test]
fn migration_import_frozen_is_plan_apply_verify_in_one_route() {
    let source = Source::new("import-frozen", None, |_| {});
    let dbs = Dbs::new("import-frozen");
    let tool_before = std::fs::read(&dbs.tool).unwrap();
    let output = import_frozen(&source, &dbs, &fingerprint());
    assert_code(&output, 0);
    let doc = json_result(&output);
    assert_eq!(doc["command"], "import");
    let i = &doc["result"];
    assert_eq!(i["status"], "applied");
    assert_eq!(i["apply"]["effects"]["created"], 5);
    assert_eq!(i["verify"]["status"], "verified");
    assert_eq!(i["source"]["revision"], expected()["revision"]);
    // The tool database is verified but never written by a frozen import.
    assert_eq!(std::fs::read(&dbs.tool).unwrap(), tool_before);
}

#[test]
fn migration_round_trip_import_export_import_export_is_byte_identical() {
    let source = Source::new("round-trip", None, |_| {});
    let first = Dbs::new("round-trip-first");
    assert_code(&import_frozen(&source, &first, &fingerprint()), 0);
    let e1 = export(&first).stdout;

    let guard = TempDir::new("round-trip-input");
    let input = guard.path().join("export.json");
    std::fs::write(&input, &e1).unwrap();
    let second = Dbs::new("round-trip-second");
    let before = db_state(&second.workspace);
    assert_code(&import_canonical(&input, &second, "not-the-digest"), 1);
    assert_eq!(db_state(&second.workspace), before);

    let digest = sha256_hex(&e1);
    let output = import_canonical(&input, &second, &digest);
    assert_code(&output, 0);
    let r = json_result(&output)["result"].clone();
    assert_eq!(r["status"], "imported");
    assert_eq!(r["input_digest"]["value"], digest.as_str());
    assert_eq!(r["workspace"]["created"], 5);
    assert_eq!(r["tool"]["records"], 0);
    let e2 = export(&second).stdout;
    assert_eq!(
        e2, e1,
        "the second export must be byte-identical to the first"
    );

    let state = db_state(&second.workspace);
    let output = import_canonical(&input, &second, &digest);
    assert_code(&output, 0);
    assert_eq!(json_result(&output)["result"]["workspace"]["unchanged"], 5);
    assert_eq!(db_state(&second.workspace), state);
    assert_eq!(export(&second).stdout, e1);
}

#[test]
fn migration_canonical_import_accepts_only_a_complete_export_envelope() {
    let dbs = Dbs::new("canonical-closed");
    let guard = TempDir::new("canonical-closed-input");
    let before = db_state(&dbs.workspace);
    let cases: [(&str, &str); 5] = [
        ("not-json", "{"),
        (
            "wrong-command",
            r#"{"status":"ok","command":"validate","result":[]}"#,
        ),
        (
            "fail-status",
            r#"{"status":"fail","command":"export","result":[]}"#,
        ),
        (
            "extra-field",
            r#"{"status":"ok","command":"export","result":[],"x":1}"#,
        ),
        ("bare-array", "[]"),
    ];
    for (name, text) in cases {
        let input = guard.path().join(format!("{name}.json"));
        std::fs::write(&input, text).unwrap();
        let output = import_canonical(&input, &dbs, &sha256_hex(text.as_bytes()));
        assert_code(&output, 3);
        assert!(stdout_of(&output).is_empty(), "{name}");
    }
    // Duplicate identities are refused before any write.
    let source = Source::new("canonical-dup", None, |_| {});
    let seeded = Dbs::new("canonical-dup-seed");
    assert_code(&import_frozen(&source, &seeded, &fingerprint()), 0);
    let mut doc: Value = serde_json::from_slice(&export(&seeded).stdout).unwrap();
    let first = doc["result"][0].clone();
    doc["result"].as_array_mut().unwrap().push(first);
    let text = serde_json::to_string(&doc).unwrap();
    let input = guard.path().join("duplicate.json");
    std::fs::write(&input, &text).unwrap();
    let output = import_canonical(&input, &dbs, &sha256_hex(text.as_bytes()));
    assert_code(&output, 3);
    assert!(stderr_of(&output).contains("share an identity"));
    assert_eq!(db_state(&dbs.workspace), before);
}

// ---------------------------------------------------------------------------
// rollback
// ---------------------------------------------------------------------------

#[test]
fn migration_apply_then_rollback_restores_the_exact_pre_state() {
    let source = Source::new("rollback", None, |_| {});
    let dbs = Dbs::new("rollback");
    let pre_export = export(&dbs).stdout;
    let pre = db_state(&dbs.workspace);

    let output = apply_confirmed(&source, &dbs);
    assert_code(&output, 0);
    let run_id = json_result(&output)["result"]["run_id"]
        .as_str()
        .unwrap()
        .to_string();

    let output = rollback(&source, &dbs, &run_id, "mr-9-000000000000");
    assert_code(&output, 1);
    assert_eq!(
        json_result(&output)["result"]["status"],
        "confirmation-mismatch"
    );

    let (applied_runs, no_rollbacks) = journal(&dbs.workspace);
    assert_eq!(applied_runs.len(), 1);
    assert!(no_rollbacks.is_empty());

    let output = rollback(&source, &dbs, &run_id, &run_id);
    assert_code(&output, 0);
    let r = json_result(&output)["result"].clone();
    assert_eq!(r["status"], "rolled-back");
    assert_eq!(r["restored_revision_count"], 0);
    // The rollback never replaces history: after the restore the journal
    // holds the original applied fact unchanged (the same row, byte for
    // byte, including its recorded_at) AND a separate rollback fact.
    let (runs, rollbacks) = journal(&dbs.workspace);
    assert_eq!(runs, applied_runs);
    assert_eq!(rollbacks.len(), 1);
    assert_eq!(rollbacks[0].0, run_id);
    assert_eq!(
        rollbacks[0].1,
        r["restored_record_state_digest"]["value"].as_str().unwrap()
    );
    assert_eq!(export(&dbs).stdout, pre_export);
    let after = db_state(&dbs.workspace);
    assert_eq!(after.records, pre.records);
    assert_eq!(after.revisions, pre.revisions);
    assert_eq!(after.schema_version, pre.schema_version);
    assert_eq!(after.metadata, pre.metadata);
    assert_eq!(
        after.runs,
        vec![(run_id.clone(), "rolled-back".to_string())]
    );

    // A second rollback is refused without change.
    let output = rollback(&source, &dbs, &run_id, &run_id);
    assert_code(&output, 1);
    assert_eq!(
        json_result(&output)["result"]["refusal"],
        "already-rolled-back"
    );
    assert_eq!(db_state(&dbs.workspace), after);

    assert_eq!(journal(&dbs.workspace), (runs, rollbacks));

    // The rolled-back plan is never re-applied implicitly.
    let output = apply_confirmed(&source, &dbs);
    assert_code(&output, 1);
    assert_eq!(
        json_result(&output)["result"]["status"],
        "previously-rolled-back"
    );
    assert_eq!(db_state(&dbs.workspace), after);
}

#[test]
fn migration_rollback_over_a_newer_revision_or_a_damaged_checkpoint_changes_nothing() {
    let source = Source::new("rollback-refused", None, |_| {});
    let dbs = Dbs::new("rollback-refused");
    let output = apply_confirmed(&source, &dbs);
    assert_code(&output, 0);
    let run_id = json_result(&output)["result"]["run_id"]
        .as_str()
        .unwrap()
        .to_string();

    // Damaged checkpoint: exit 3, nothing restored.
    let checkpoint = dbs
        .workspace
        .with_file_name("workspace.sqlite3.checkpoints")
        .join(format!("{run_id}.sqlite3"));
    let original = std::fs::read(&checkpoint).unwrap();
    let mut damaged = original.clone();
    damaged[100] ^= 0x5a;
    std::fs::write(&checkpoint, &damaged).unwrap();
    let state = db_state(&dbs.workspace);
    let output = rollback(&source, &dbs, &run_id, &run_id);
    assert_code(&output, 3);
    assert!(stdout_of(&output).is_empty());
    assert!(stderr_of(&output).contains("damaged"));
    assert_eq!(db_state(&dbs.workspace), state);
    std::fs::write(&checkpoint, &original).unwrap();

    // A newer revision of an affected record: refused, unchanged.
    let mut doc: Value = serde_json::from_slice(&export(&dbs).stdout).unwrap();
    doc["result"][0]["title"] = Value::from("A later edit");
    let text = serde_json::to_string(&doc).unwrap();
    let guard = TempDir::new("rollback-refused-input");
    let input = guard.path().join("edit.json");
    std::fs::write(&input, &text).unwrap();
    assert_code(
        &import_canonical(&input, &dbs, &sha256_hex(text.as_bytes())),
        0,
    );
    let state = db_state(&dbs.workspace);
    let output = rollback(&source, &dbs, &run_id, &run_id);
    assert_code(&output, 1);
    assert_eq!(json_result(&output)["result"]["refusal"], "state-changed");
    assert_eq!(db_state(&dbs.workspace), state);

    // An unknown run.
    let output = rollback(&source, &dbs, "mr-7-0123456789ab", "mr-7-0123456789ab");
    assert_code(&output, 1);
    assert_eq!(json_result(&output)["result"]["refusal"], "unknown-run");
    assert_eq!(db_state(&dbs.workspace), state);
}

// ---------------------------------------------------------------------------
// Databases: missing, corrupt, wrong role, wrong Kernel edition
// ---------------------------------------------------------------------------

#[test]
fn migration_databases_fail_closed_on_missing_corrupt_wrong_role_or_edition() {
    let source = Source::new("db-fail", None, |_| {});
    let dbs = Dbs::new("db-fail");
    let fp = fingerprint();
    let guard = TempDir::new("db-fail-files");

    let missing = guard.path().join("missing.sqlite3");
    let corrupt = guard.path().join("corrupt.sqlite3");
    std::fs::write(&corrupt, b"this is not a sqlite database at all").unwrap();
    for db in [&missing, &corrupt, &dbs.tool] {
        for extra in [&[][..], &["--dry-run", "false", "--confirm", &fp][..]] {
            let k = kernel();
            let mut args = vec![
                "migration",
                "apply",
                "--kernel",
                &k,
                "--source",
                source.path(),
                "--workspace-db",
                db.to_str().unwrap(),
            ];
            args.extend_from_slice(extra);
            let output = run(&args);
            assert_code(&output, 3);
            assert!(stdout_of(&output).is_empty());
        }
    }
    assert!(!missing.exists());
    assert_eq!(
        std::fs::read(&corrupt).unwrap(),
        b"this is not a sqlite database at all"
    );

    // Wrong Kernel edition: a Kernel whose VERSION differs from the one the
    // databases record.
    let other_kernel = guard.path().join("kernel");
    // The whole registry tree: the migration schemas and the payload
    // registry's item schemas (`rust-workspace-state-validation`).
    std::fs::create_dir_all(other_kernel.join("registries")).unwrap();
    copy_tree(
        &kernel_root().join("registries"),
        &other_kernel.join("registries"),
    );
    std::fs::write(other_kernel.join("VERSION"), "9.9.9\n").unwrap();
    let before = std::fs::read(&dbs.workspace).unwrap();
    let output = run(&[
        "migration",
        "verify",
        "--kernel",
        other_kernel.to_str().unwrap(),
        "--source",
        source.path(),
        "--workspace-db",
        dbs.ws(),
    ]);
    assert_code(&output, 3);
    assert!(
        stderr_of(&output).contains("9.9.9"),
        "{}",
        stderr_of(&output)
    );
    assert_eq!(std::fs::read(&dbs.workspace).unwrap(), before);
}

// ---------------------------------------------------------------------------
// Event sinks, network, structure
// ---------------------------------------------------------------------------

fn in_process(
    args: &[String],
    sink: &dyn meridian_cli::events::EventSink,
) -> (i32, Vec<u8>, Vec<u8>) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut stdin = std::io::empty();
    let code = meridian_cli::run(args, &mut stdin, &mut stdout, &mut stderr, sink);
    (code, stdout, stderr)
}

#[test]
fn migration_noop_and_recording_event_sinks_give_identical_results_and_state() {
    use meridian_cli::events::{NoOpEventSink, RecordingEventSink};
    let source = Source::new("sinks", None, |_| {});
    let fp = fingerprint();
    let noop_dbs = Dbs::new("sinks-noop");
    let rec_dbs = Dbs::copy_of(&noop_dbs, "sinks-recording");
    let recording = RecordingEventSink::new();
    let first_run = format!("mr-1-{}", &fp[..12]);
    let command_lines = |dbs: &Dbs| -> Vec<Vec<String>> {
        let k = kernel();
        let own = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        vec![
            own(&[
                "migration",
                "plan",
                "--kernel",
                &k,
                "--source",
                source.path(),
                "--format",
                "json",
            ]),
            own(&[
                "migration",
                "apply",
                "--kernel",
                &k,
                "--source",
                source.path(),
                "--workspace-db",
                dbs.ws(),
                "--format",
                "json",
            ]),
            own(&[
                "import",
                "--kernel",
                &k,
                "--kind",
                "frozen-instance",
                "--source",
                source.path(),
                "--tool-db",
                dbs.tool(),
                "--workspace-db",
                dbs.ws(),
                "--confirm",
                &fp,
                "--format",
                "json",
            ]),
            own(&[
                "migration",
                "apply",
                "--kernel",
                &k,
                "--source",
                source.path(),
                "--workspace-db",
                dbs.ws(),
                "--dry-run",
                "false",
                "--confirm",
                &fp,
                "--format",
                "json",
            ]),
            own(&[
                "migration",
                "verify",
                "--kernel",
                &k,
                "--source",
                source.path(),
                "--workspace-db",
                dbs.ws(),
                "--format",
                "human",
            ]),
            own(&[
                "migration",
                "rollback",
                "--kernel",
                &k,
                "--source",
                source.path(),
                "--workspace-db",
                dbs.ws(),
                "--run",
                &first_run,
                "--confirm",
                &first_run,
                "--format",
                "json",
            ]),
            own(&[
                "migration",
                "rollback",
                "--kernel",
                &k,
                "--source",
                source.path(),
                "--workspace-db",
                dbs.ws(),
                "--run",
                &first_run,
                "--confirm",
                &first_run,
                "--format",
                "json",
            ]),
        ]
    };
    let noop_runs: Vec<_> = command_lines(&noop_dbs)
        .iter()
        .map(|args| in_process(args, &NoOpEventSink))
        .collect();
    let rec_runs: Vec<_> = command_lines(&rec_dbs)
        .iter()
        .map(|args| in_process(args, &recording))
        .collect();
    let normalise = |bytes: &[u8], dbs: &Dbs| {
        String::from_utf8_lossy(bytes)
            .replace(dbs.ws(), "<ws>")
            .replace(dbs.tool(), "<tool>")
    };
    for (i, (a, b)) in noop_runs.iter().zip(&rec_runs).enumerate() {
        assert_eq!(a.0, b.0, "exit code of command {i}");
        assert_eq!(
            normalise(&a.1, &noop_dbs),
            normalise(&b.1, &rec_dbs),
            "stdout of command {i}"
        );
        assert_eq!(
            normalise(&a.2, &noop_dbs),
            normalise(&b.2, &rec_dbs),
            "stderr of command {i}"
        );
    }
    assert_eq!(noop_runs[5].0, 0, "the rollback itself succeeded");
    assert_eq!(noop_runs[6].0, 1, "the repeated rollback is refused");
    assert_eq!(db_state(&noop_dbs.workspace), db_state(&rec_dbs.workspace));
    assert!(!recording.recorded().is_empty());
}

#[test]
fn migration_all_five_commands_run_without_network() {
    let source = Source::new("offline", None, |_| {});
    let dbs = Dbs::new("offline");
    let fp = fingerprint();
    let guard = TempDir::new("offline-input");
    let k = kernel();
    let isolated = Command::new("unshare")
        .args(["-rn", "true"])
        .output()
        .is_ok_and(|o| o.status.success());
    let offline = |args: &[&str]| -> Output {
        let mut command = if isolated {
            let mut c = Command::new("unshare");
            c.arg("-rn").arg(exe());
            c
        } else {
            Command::new(exe())
        };
        command
            .args(args)
            .env("http_proxy", "http://127.0.0.1:9")
            .env("https_proxy", "http://127.0.0.1:9")
            .env("all_proxy", "http://127.0.0.1:9")
            .env("GIT_ALLOW_PROTOCOL", "none")
            .env_remove("MERIDIAN_INSTANCE")
            .output()
            .unwrap()
    };
    if !isolated {
        eprintln!("note: no network namespace is available here; network access is blocked through proxy/protocol environment only");
    }
    assert_code(
        &offline(&[
            "migration",
            "plan",
            "--kernel",
            &k,
            "--source",
            source.path(),
        ]),
        0,
    );
    let apply = offline(&[
        "migration",
        "apply",
        "--kernel",
        &k,
        "--source",
        source.path(),
        "--workspace-db",
        dbs.ws(),
        "--dry-run",
        "false",
        "--confirm",
        &fp,
        "--format",
        "json",
    ]);
    assert_code(&apply, 0);
    let run_id = json_result(&apply)["result"]["run_id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_code(
        &offline(&[
            "migration",
            "verify",
            "--kernel",
            &k,
            "--source",
            source.path(),
            "--workspace-db",
            dbs.ws(),
        ]),
        0,
    );
    assert_code(
        &offline(&[
            "import",
            "--kernel",
            &k,
            "--kind",
            "frozen-instance",
            "--source",
            source.path(),
            "--tool-db",
            dbs.tool(),
            "--workspace-db",
            dbs.ws(),
            "--confirm",
            &fp,
        ]),
        0,
    );
    let e1 = export(&dbs).stdout;
    let input = guard.path().join("export.json");
    std::fs::write(&input, &e1).unwrap();
    let fresh = Dbs::new("offline-fresh");
    assert_code(
        &offline(&[
            "import",
            "--kernel",
            &k,
            "--kind",
            "canonical-records",
            "--input",
            input.to_str().unwrap(),
            "--tool-db",
            fresh.tool(),
            "--workspace-db",
            fresh.ws(),
            "--confirm",
            &sha256_hex(&e1),
        ]),
        0,
    );
    assert_code(
        &offline(&[
            "migration",
            "rollback",
            "--kernel",
            &k,
            "--source",
            source.path(),
            "--workspace-db",
            dbs.ws(),
            "--run",
            &run_id,
            "--confirm",
            &run_id,
        ]),
        0,
    );
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn migration_production_code_respects_crate_boundaries_and_never_reads_meridian_instance() {
    let root = kernel_root();
    let mut files = Vec::new();
    for krate in [
        "meridian-core",
        "meridian-app",
        "meridian-storage-sqlite",
        "meridian-cli",
    ] {
        rust_files(&root.join(krate).join("src"), &mut files);
    }
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap();
        for line in text.lines() {
            let code = line.split("//").next().unwrap_or("");
            assert!(
                !(code.contains("MERIDIAN_INSTANCE") && code.contains("env")),
                "{} reads MERIDIAN_INSTANCE: {line}",
                file.display()
            );
        }
    }
    let forbidden = |path: &str, needles: &[&str]| {
        let mut files = Vec::new();
        let full = root.join(path);
        if full.is_dir() {
            rust_files(&full, &mut files);
        } else {
            files.push(full);
        }
        for file in files {
            let text = std::fs::read_to_string(&file).unwrap();
            for needle in needles {
                assert!(
                    !text.contains(needle),
                    "{} contains {needle}",
                    file.display()
                );
            }
        }
    };
    let io = [
        "std::fs",
        "std::process",
        "std::env",
        "rusqlite",
        "println!",
        "eprintln!",
        "std::net",
    ];
    for core in [
        "meridian-core/src/migration/write_set.rs",
        "meridian-core/src/migration/run.rs",
        "meridian-core/src/migration/applicability.rs",
    ] {
        forbidden(core, &io);
    }
    forbidden(
        "meridian-app/src/migration",
        &[
            "std::fs",
            "std::process",
            "std::env",
            "rusqlite",
            "meridian_storage_sqlite",
            "std::net",
        ],
    );
    forbidden(
        "meridian-cli/src/commands/migration.rs",
        &[
            "SELECT ",
            "INSERT ",
            "compute_plan_fingerprint",
            "verify_read_back",
            "compare_applicability",
        ],
    );
    forbidden(
        "meridian-cli/src/commands/import.rs",
        &[
            "SELECT ",
            "INSERT ",
            "compute_plan_fingerprint",
            "verify_read_back",
            "serde_json::from_slice",
        ],
    );
}

// ---------------------------------------------------------------------------
// The accepted real bundle (needs the local frozen source)
// ---------------------------------------------------------------------------

fn real_source() -> String {
    std::env::var("MERIDIAN_INSTANCE")
        .expect("these tests need MERIDIAN_INSTANCE to name the local frozen source")
}

const REAL_REVISION: &str = "59f2fa220d015cde8eed873629750aa74f72dbbe";
const REAL_FINGERPRINT: &str = "6e426393b62c8d918def095ef34d06e871f44bfa3c6d524868103fa46e6def76";

#[test]
#[ignore = "needs the local frozen source: MERIDIAN_INSTANCE=<path> cargo test -p meridian-cli --test binary_runs migration -- --ignored"]
fn migration_real_bundle_imports_336_records_verifies_61_norms_and_rolls_back() {
    let source = real_source();
    let k = kernel();
    let output = run(&[
        "migration",
        "plan",
        "--kernel",
        &k,
        "--source",
        &source,
        "--format",
        "json",
    ]);
    assert_code(&output, 0);
    let p = json_result(&output)["result"].clone();
    assert_eq!(p["source"]["revision"], REAL_REVISION);
    assert_eq!(p["plan"]["plan_fingerprint"]["value"], REAL_FINGERPRINT);
    assert_eq!(p["plan"]["counts"]["record_units"], 346);
    assert_eq!(p["plan"]["counts"]["migrated"], 336);
    assert_eq!(p["plan"]["counts"]["retained"], 10);
    assert_eq!(p["plan"]["counts"]["merged"], 0);

    let dbs = Dbs::new("real");
    let pre_export = export(&dbs).stdout;
    let output = run(&[
        "import",
        "--kernel",
        &k,
        "--kind",
        "frozen-instance",
        "--source",
        &source,
        "--tool-db",
        dbs.tool(),
        "--workspace-db",
        dbs.ws(),
        "--confirm",
        REAL_FINGERPRINT,
        "--format",
        "json",
    ]);
    assert_code(&output, 0);
    let i = json_result(&output)["result"].clone();
    assert_eq!(i["apply"]["effects"]["created"], 336);
    assert_eq!(i["apply"]["retained"], 10);
    let v = &i["verify"];
    assert_eq!(v["status"], "verified");
    for (field, value) in [
        ("expected", 336),
        ("imported", 336),
        ("missing", 0),
        ("extra", 0),
        ("changed", 0),
        ("duplicate", 0),
        ("retained", 10),
    ] {
        assert_eq!(v[field], value, "{field}");
    }
    let a = &v["applicability"];
    assert_eq!(a["verdict"], "equivalent");
    for (field, value) in [
        ("controlled", 61),
        ("imported", 61),
        ("lost", 0),
        ("added", 0),
        ("changed", 0),
        ("duplicate", 0),
    ] {
        assert_eq!(a[field], value, "{field}");
    }
    assert_eq!(db_state(&dbs.workspace).records.len(), 336);
    let run_id = i["apply"]["run_id"].as_str().unwrap().to_string();

    // Idempotent repeat of import and apply.
    let state = db_state(&dbs.workspace);
    let e1 = export(&dbs).stdout;
    let output = run(&[
        "import",
        "--kernel",
        &k,
        "--kind",
        "frozen-instance",
        "--source",
        &source,
        "--tool-db",
        dbs.tool(),
        "--workspace-db",
        dbs.ws(),
        "--confirm",
        REAL_FINGERPRINT,
        "--format",
        "json",
    ]);
    assert_code(&output, 0);
    assert_eq!(
        json_result(&output)["result"]["apply"]["status"],
        "already-applied"
    );
    let output = run(&[
        "migration",
        "apply",
        "--kernel",
        &k,
        "--source",
        &source,
        "--workspace-db",
        dbs.ws(),
        "--dry-run",
        "false",
        "--confirm",
        REAL_FINGERPRINT,
        "--format",
        "json",
    ]);
    assert_code(&output, 0);
    assert_eq!(json_result(&output)["result"]["status"], "already-applied");
    assert_eq!(db_state(&dbs.workspace), state);
    assert_eq!(export(&dbs).stdout, e1);

    // Real round trip.
    let guard = TempDir::new("real-round-trip");
    let input = guard.path().join("export.json");
    std::fs::write(&input, &e1).unwrap();
    let second = Dbs::new("real-second");
    assert_code(&import_canonical(&input, &second, &sha256_hex(&e1)), 0);
    assert_eq!(export(&second).stdout, e1);

    // Real rollback, then a refused repeat. The journal keeps the applied
    // fact unchanged and gains one separate rollback fact.
    let (applied_runs, no_rollbacks) = journal(&dbs.workspace);
    assert_eq!(applied_runs.len(), 1);
    assert!(no_rollbacks.is_empty());
    let output = run(&[
        "migration",
        "rollback",
        "--kernel",
        &k,
        "--source",
        &source,
        "--workspace-db",
        dbs.ws(),
        "--run",
        &run_id,
        "--confirm",
        &run_id,
        "--format",
        "json",
    ]);
    assert_code(&output, 0);
    assert_eq!(export(&dbs).stdout, pre_export);
    let (runs, rollbacks) = journal(&dbs.workspace);
    assert_eq!(runs, applied_runs);
    assert_eq!(rollbacks.len(), 1);
    assert_eq!(rollbacks[0].0, run_id);
    let output = run(&[
        "migration",
        "rollback",
        "--kernel",
        &k,
        "--source",
        &source,
        "--workspace-db",
        dbs.ws(),
        "--run",
        &run_id,
        "--confirm",
        &run_id,
        "--format",
        "json",
    ]);
    assert_code(&output, 1);
    assert_eq!(export(&dbs).stdout, pre_export);
    assert_eq!(journal(&dbs.workspace), (runs, rollbacks));
}

#[test]
#[ignore = "needs the local frozen source: MERIDIAN_INSTANCE=<path> cargo test -p meridian-cli --test binary_runs migration -- --ignored"]
fn migration_real_bundle_every_read_back_record_equals_its_accepted_target() {
    let source = real_source();
    let dbs = Dbs::new("real-fields");
    assert_code(
        &run(&[
            "import",
            "--kernel",
            &kernel(),
            "--kind",
            "frozen-instance",
            "--source",
            &source,
            "--tool-db",
            dbs.tool(),
            "--workspace-db",
            dbs.ws(),
            "--confirm",
            REAL_FINGERPRINT,
        ]),
        0,
    );
    // Field by field against the accepted canonical export of the bundle
    // itself: schema, id, title, record type, scope, origin, authority,
    // payload, schema version.
    let bundle: Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(&source).join("migration/instance-data/canonical-export.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let mut accepted: Vec<Value> = bundle["exports"][0]["payload"]["records"]
        .as_array()
        .unwrap()
        .clone();
    let exported: Value = serde_json::from_slice(&export(&dbs).stdout).unwrap();
    let mut stored: Vec<Value> = exported["result"].as_array().unwrap().clone();
    let key = |r: &Value| format!("{}/{}/{}", r["scope"]["type"], r["scope"]["id"], r["id"]);
    accepted.sort_by_key(key);
    stored.sort_by_key(key);
    assert_eq!(stored.len(), 336);
    assert_eq!(accepted.len(), 336);
    for (a, s) in accepted.iter().zip(&stored) {
        for field in [
            "$schema",
            "id",
            "title",
            "record_type",
            "scope",
            "origin",
            "authority",
            "payload",
            "schema_version",
        ] {
            assert_eq!(a[field], s[field], "{} {field}", a["id"]);
        }
    }
}
