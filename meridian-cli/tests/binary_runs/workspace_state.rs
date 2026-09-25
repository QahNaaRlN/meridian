//! Real-binary tests of `rust-workspace-state-validation`
//! (`meridian-rust-migration-program-plan.md` §5.23.11): `meridian validate
//! --workspace-db [--log-metrics]` over a workspace database filled by the
//! production `import --kind canonical-records`, against this Kernel and a
//! real local Git repository standing in for a product repository.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{json, Value};

use super::{exe, json_result, kernel_root, run, stderr_of, stdout_of, TempDir};

const FIXED_ENV: [(&str, &str); 6] = [
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
        .args(["-c", "core.autocrlf=false", "-c", "commit.gpgsign=false"])
        .args(args)
        .envs(FIXED_ENV)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        stderr_of(&output)
    );
    String::from_utf8(output.stdout).unwrap()
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

/// A product name the Kernel never carries — built from fragments, since
/// this source file is itself Kernel text the product scan reads.
fn product_name() -> String {
    ["Zorb", "laxian"].concat()
}

const AGENTS: &str = "<!-- meridian:begin instruction-section id=\"style\" owner=\"team\" -->\nUse tabs.\n<!-- meridian:end instruction-section id=\"style\" -->\n";

/// A product repository with one container region, one rule and a
/// manifest that supports the `vue-spa` profile.
fn product_repository(base: &Path, extra: &[(&str, &str)]) -> (PathBuf, String) {
    let repo = base.join("web");
    std::fs::create_dir_all(repo.join(".cursor/rules")).unwrap();
    std::fs::write(repo.join("AGENTS.md"), AGENTS).unwrap();
    std::fs::write(
        repo.join(".cursor/rules/a.mdc"),
        "Prefer small functions.\n",
    )
    .unwrap();
    std::fs::write(
        repo.join("package.json"),
        r#"{"dependencies":{"vue":"3"},"devDependencies":{"vite":"5"}}"#,
    )
    .unwrap();
    for (path, text) in extra {
        let file = repo.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
    }
    git(&repo, &["init", "-q", "-b", "dev"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "product fixture"]);
    let revision = git(&repo, &["rev-parse", "HEAD"]).trim().to_string();
    (repo, revision)
}

fn container(media: &str, content: &str) -> Value {
    json!({
        "media_type": media,
        "encoding": "utf-8",
        "content": content,
        "digest": {"algorithm": "sha-256", "value": sha256_hex(content.as_bytes())},
    })
}

fn record(record_type: &str, id: &str, scope: Value, title: &str, payload: Value) -> Value {
    json!({
        "$schema": "registries/operating-model/scoped-record.schema.json",
        "schema_version": 1,
        "id": id,
        "title": title,
        "record_type": record_type,
        "scope": scope,
        "origin": {"kind": "declared", "source_ref": "owner-decision:workspace-state-fixture"},
        "authority": {"kind": "project-owner", "authority_ref": "sample-owner"},
        "payload": payload,
    })
}

fn workspace() -> Value {
    json!({"type": "project-workspace", "id": "sample"})
}

fn repository_scope(id: &str) -> Value {
    json!({"type": "repository-scope", "id": id, "workspace_id": "sample"})
}

fn intake(artifact: &str, region: Option<&str>) -> Value {
    let mut content = json!({
        "artifact": artifact,
        "delivery": if region.is_some() { "agents-md-section" } else { "cursor-rule" },
        "digest": "b".repeat(64),
        "genre": "standard",
        "observed_activation": "always",
        "recorded_at": "2026-08-21",
        "scope": "repository",
        "topic": "unclassified",
        "unclassified_reason": "several subjects",
        "verdict": "deferred",
        "verdict_basis": "recorded as observed",
        "resume_condition": "owner decides",
    });
    if let Some(region) = region {
        content["region"] = json!(region);
    }
    container("application/json", &content.to_string())
}

/// The records of a workspace every check accepts; `edit` may change them
/// before they are imported.
struct Spec {
    product: String,
    dependencies: String,
    profile: String,
    recorded_revision: String,
    context: bool,
    extra: Vec<Value>,
}

impl Spec {
    fn consistent(revision: &str) -> Spec {
        Spec {
            product: format!("product:\n  name: {}\n", product_name()),
            dependencies: "dependencies:\n  - id: vendored-skill\n    state: vendored\n"
                .to_string(),
            profile: "vue-spa".to_string(),
            recorded_revision: revision.to_string(),
            context: true,
            extra: Vec::new(),
        }
    }

    fn records(&self, repo: &Path) -> Vec<Value> {
        let root = repo.to_str().unwrap();
        let identity = json!({
            "id": "web",
            "path": root,
            "role": "web client",
            "profile": self.profile,
            "ownership": "own",
            "sources": {"rules": [format!("{root}/AGENTS.md")], "manifests": [format!("{root}/package.json")]},
            "vcs": {"type": "git", "revision": self.recorded_revision, "ref": "dev", "working_tree": "clean", "evidence_basis": "revision"},
            "last_verified": "2026-01-01T00:00:00Z",
        });
        let mut records = vec![
            record(
                "workspace-file",
                "product",
                workspace(),
                "product.yaml",
                container("text/yaml", &self.product),
            ),
            record(
                "workspace-file",
                "external-dependencies",
                workspace(),
                "external-dependencies.yaml",
                container("text/yaml", &self.dependencies),
            ),
            record(
                "repository-identity",
                "inventory-web",
                workspace(),
                "Repository web",
                container("application/json", &identity.to_string()),
            ),
            record(
                "instruction-intake-record",
                "intake-web-agents",
                repository_scope("web"),
                "Intake web AGENTS.md#style",
                intake("AGENTS.md", Some("style")),
            ),
            record(
                "instruction-intake-record",
                "intake-web-rule",
                repository_scope("web"),
                "Intake web a.mdc",
                intake(".cursor/rules/a.mdc", None),
            ),
            record(
                "report",
                "report-one",
                workspace(),
                "Report one",
                container("text/markdown", "# report\n"),
            ),
        ];
        if self.context {
            records.push(record(
                "workspace-file",
                "skills-bugfix-protocol-context",
                workspace(),
                "skills/bugfix-protocol/context.md",
                container("text/markdown", "# context\n"),
            ));
        }
        records.extend(self.extra.iter().cloned());
        records
    }
}

/// A workspace database filled through the production canonical import.
struct Db {
    guard: TempDir,
    tool: PathBuf,
    ws: PathBuf,
}

impl Db {
    fn new(label: &str) -> Db {
        let guard = TempDir::new(label);
        let base = guard.path().to_path_buf();
        let db = Db {
            tool: base.join("tool.sqlite3"),
            ws: base.join("workspace.sqlite3"),
            guard,
        };
        let k = kernel_root();
        assert_code(
            &run(&[
                "init",
                "--kernel",
                k.to_str().unwrap(),
                "--workspace",
                base.join("workspace").to_str().unwrap(),
                "--tool-db",
                db.tool.to_str().unwrap(),
                "--workspace-db",
                db.ws.to_str().unwrap(),
            ]),
            0,
        );
        db
    }

    fn base(&self) -> &Path {
        self.guard.path()
    }

    fn import(&self, records: &[Value]) -> Output {
        let envelope = json!({"status": "ok", "command": "export", "result": records});
        let input = self.base().join(format!("export-{}.json", records.len()));
        let bytes = serde_json::to_vec(&envelope).unwrap();
        std::fs::write(&input, &bytes).unwrap();
        let k = kernel_root();
        run(&[
            "import",
            "--kernel",
            k.to_str().unwrap(),
            "--kind",
            "canonical-records",
            "--input",
            input.to_str().unwrap(),
            "--tool-db",
            self.tool.to_str().unwrap(),
            "--workspace-db",
            self.ws.to_str().unwrap(),
            "--confirm",
            &sha256_hex(&bytes),
            "--format",
            "json",
        ])
    }

    fn ws(&self) -> &str {
        self.ws.to_str().unwrap()
    }

    fn export(&self) -> String {
        let k = kernel_root();
        let output = run(&[
            "export",
            "--kernel",
            k.to_str().unwrap(),
            "--tool-db",
            self.tool.to_str().unwrap(),
            "--workspace-db",
            self.ws(),
            "--format",
            "json",
        ]);
        assert_code(&output, 0);
        stdout_of(&output)
    }

    fn validate(&self, extra: &[&str]) -> Output {
        let k = kernel_root();
        let mut args = vec![
            "validate",
            "--kernel",
            k.to_str().unwrap(),
            "--workspace-db",
            self.ws(),
        ];
        args.extend(extra);
        let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        Command::new(exe())
            .args(&owned)
            .env_remove("MERIDIAN_INSTANCE")
            .output()
            .unwrap()
    }
}

fn filled(label: &str, spec: impl FnOnce(&str) -> Spec, extra_files: &[(&str, &str)]) -> Db {
    let db = Db::new(label);
    let (repo, revision) = product_repository(db.base(), extra_files);
    let records = spec(&revision).records(&repo);
    assert_code(&db.import(&records), 0);
    db
}

fn lines(output: &Output, key: &str) -> Vec<String> {
    json_result(output)["result"][key]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

const KERNEL_ONLY_ADVISORIES: [&str; 4] = [
    "ext-dependencies: no Instance root, not checked",
    "inventory-git: no Instance root, not checked",
    "stack-profile: no Instance root, declarations were not checked",
    "instruction-intake: no Instance root, not checked",
];

#[test]
fn workspace_state_a_consistent_database_is_clean_and_left_untouched() {
    let db = filled("ws-clean", Spec::consistent, &[]);
    let file_before = std::fs::read(&db.ws).unwrap();
    let export_before = db.export();

    let output = db.validate(&["--format", "json"]);
    assert_code(&output, 0);
    assert!(stderr_of(&output).is_empty(), "{}", stderr_of(&output));
    let failures = lines(&output, "failures");
    assert!(failures.is_empty(), "{failures:?}");
    let warnings = lines(&output, "warnings");
    for advisory in KERNEL_ONLY_ADVISORIES {
        assert!(!warnings.iter().any(|w| w == advisory), "{warnings:?}");
    }
    assert!(!warnings
        .iter()
        .any(|w| w.starts_with("kernel-purity: MERIDIAN_INSTANCE is not set")));
    assert!(!warnings.iter().any(|w| w.starts_with("instance-context:")));
    let state = &json_result(&output)["result"]["workspace_state"];
    assert_eq!(state["records"], 7);
    assert_eq!(state["rejected_payloads"], 0);
    assert_eq!(state["repositories_confirmed"], 1);
    assert_eq!(state["intake_repositories_complete"], 1);
    assert!(json_result(&output)["result"].get("metrics").is_none());

    // Read-only: no revision, record or byte of the database changed.
    assert_eq!(std::fs::read(&db.ws).unwrap(), file_before);
    assert_eq!(db.export(), export_before);

    let human = db.validate(&[]);
    assert_code(&human, 0);
    assert!(stdout_of(&human)
        .contains("Workspace database: 7 record(s), 1 repository(ies), 2 intake record(s)"));
}

#[test]
fn workspace_state_kernel_only_validate_keeps_its_advisories_and_shape() {
    let k = kernel_root();
    let output = Command::new(exe())
        .args([
            "validate",
            "--kernel",
            k.to_str().unwrap(),
            "--format",
            "json",
        ])
        .env_remove("MERIDIAN_INSTANCE")
        .output()
        .unwrap();
    assert_code(&output, 0);
    let warnings = lines(&output, "warnings");
    for advisory in KERNEL_ONLY_ADVISORIES {
        assert!(
            warnings.iter().any(|w| w == advisory),
            "{advisory}: {warnings:?}"
        );
    }
    assert!(warnings.iter().any(|w| w.starts_with("kernel-purity: MERIDIAN_INSTANCE is not set; product literals were NOT checked (UNVERIFIED).")));
    assert!(warnings
        .iter()
        .any(|w| w.starts_with("instance-context: bugfix-protocol requires")));
    let result = &json_result(&output)["result"];
    assert!(result.get("workspace_state").is_none());
    assert!(result.get("metrics").is_none());
}

#[test]
fn workspace_state_every_category_3_row_fails_through_the_binary() {
    let db = filled(
        "ws-negative",
        |revision| {
            let mut spec = Spec::consistent(revision);
            // M-02: a declared pattern the real Kernel matches.
            spec.product = format!(
                "product:\n  name: {}\nforbidden_patterns:\n  - '\\bmeridian-cli-rfc\\b'\n",
                product_name()
            );
            // M-06: the context the vendored skill requires is absent.
            spec.context = false;
            // M-07: an unpinned external dependency (a warning).
            spec.dependencies = "dependencies:\n  - id: mirror\n    state: external\n".to_string();
            // M-08: the recorded revision is not the repository's.
            spec.recorded_revision = "0".repeat(40);
            // M-12: intake of a repository the inventory does not name.
            spec.extra.push(record(
                "instruction-intake-record",
                "intake-ghost",
                repository_scope("ghost"),
                "Intake ghost",
                intake("AGENTS.md", None),
            ));
            spec
        },
        // M-15: a tracked norm no record covers.
        &[("skills/new/SKILL.md", "---\nname: new\n---\n")],
    );
    let output = db.validate(&["--format", "json"]);
    assert_code(&output, 1);
    assert!(stderr_of(&output).is_empty());
    let failures = lines(&output, "failures");
    let has = |prefix: &str| failures.iter().any(|f| f.starts_with(prefix));
    assert!(has("kernel-purity: forbidden pattern /\\bmeridian-cli-rfc\\b/ matched \"meridian-cli-rfc\" in "), "{failures:?}");
    assert!(failures.contains(&"instance-context: bugfix-protocol requires \"skills/bugfix-protocol/context.md\", which is missing from the workspace database; the skill declares this a blocker, not a licence to guess".to_string()));
    assert!(
        has("inventory-git: web — revision recorded 0000000 but HEAD is "),
        "{failures:?}"
    );
    assert!(failures.contains(&"instruction-intake: 1 record(s) of repository \"ghost\" name a repository the workspace inventory does not name; this is an unresolvable reference, not an unreachable repository".to_string()));
    assert!(failures.contains(&"instruction-intake: web — 1 norm(s) in the tree have no record (skills/new/SKILL.md); the register is complete or it proves nothing".to_string()), "{failures:?}");
    let warnings = lines(&output, "warnings");
    assert!(warnings.contains(&"ext-dependency: mirror is \"external\" with no pinned SHA — unverifiable on another machine".to_string()));
}

#[test]
fn workspace_state_undeclared_profile_and_unreachable_repository() {
    // M-09: a profile outside the pool.
    let db = filled(
        "ws-profile",
        |revision| Spec {
            profile: "vue-monorepo".to_string(),
            ..Spec::consistent(revision)
        },
        &[],
    );
    let output = db.validate(&["--format", "json"]);
    assert_code(&output, 1);
    assert_eq!(
        lines(&output, "failures"),
        ["stack-profile: web declares \"vue-monorepo\", which is not in the pool; a profile is named from the pool or added to it by decision, never invented at the point of use"]
    );

    // M-08/M-09/M-15: the repository is gone — UNVERIFIED, never confirmed.
    let db = filled("ws-unreachable", Spec::consistent, &[]);
    std::fs::remove_dir_all(db.base().join("web")).unwrap();
    let output = db.validate(&["--format", "json"]);
    assert_code(&output, 0);
    let warnings = lines(&output, "warnings");
    assert!(warnings.contains(&"inventory-git: 0/1 entries could be confirmed".to_string()));
    assert!(warnings.contains(&"stack-profile: 1/1 declarations could not be confirmed; the manifest was not reachable from here".to_string()));
    let info = &json_result(&output)["result"]["workspace_state"]["info"];
    assert!(info.as_array().unwrap().iter().any(|i| i
        .as_str()
        .unwrap()
        .contains("completeness is UNVERIFIED, not confirmed")));
    assert_eq!(
        json_result(&output)["result"]["workspace_state"]["repositories_confirmed"],
        0
    );
}

/// M-05b: the canonical import refuses a payload no contract accepts
/// before any database changes, and a payload that reached the database by
/// another route is found by `validate`.
#[test]
fn workspace_state_payload_contracts_guard_import_and_stored_state() {
    let db = filled("ws-payload", Spec::consistent, &[]);
    let before = db.export();
    let repo = db.base().join("web");
    let mut broken_identity: Value = serde_json::from_str(
        Spec::consistent(&"a".repeat(40)).records(&repo)[2]["payload"]["content"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    broken_identity.as_object_mut().unwrap().remove("role");
    for (bad, needle) in [
        (
            record(
                "note",
                "note-one",
                workspace(),
                "Note",
                container("text/markdown", "x"),
            ),
            "record_type \"note\" has no payload contract",
        ),
        (
            record(
                "repository-identity",
                "inventory-broken",
                workspace(),
                "Broken",
                container("application/json", &broken_identity.to_string()),
            ),
            "role",
        ),
    ] {
        let output = db.import(&[bad]);
        assert_code(&output, 3);
        assert!(stdout_of(&output).is_empty());
        assert!(
            stderr_of(&output).contains("payload-contract: ")
                && stderr_of(&output).contains(needle),
            "{}",
            stderr_of(&output)
        );
        assert_eq!(db.export(), before, "a refused import writes nothing");
    }

    // Plant an unknown record type directly in SQLite, bypassing import.
    {
        let conn = rusqlite::Connection::open(&db.ws).unwrap();
        conn.execute_batch(
            "CREATE TEMP TABLE r AS SELECT * FROM records WHERE record_id = 'report-one';
             UPDATE r SET record_key = record_key || '-planted', record_id = 'planted-note', record_type = 'note';
             INSERT INTO records SELECT * FROM r;
             CREATE TEMP TABLE v AS SELECT * FROM record_revisions WHERE record_key = (SELECT record_key FROM records WHERE record_id = 'report-one');
             UPDATE v SET record_key = record_key || '-planted', record_type = 'note', idempotency_key = idempotency_key || '-planted';
             INSERT INTO record_revisions SELECT * FROM v;",
        )
        .unwrap();
    }
    let output = db.validate(&["--format", "json"]);
    assert_code(&output, 1);
    let failures = lines(&output, "failures");
    assert!(
        failures.iter().any(|f| f.starts_with("payload-contract: note record \"planted-note\" in project-workspace \"sample\": record_type \"note\" has no payload contract")),
        "{failures:?}"
    );
    assert_eq!(
        json_result(&output)["result"]["workspace_state"]["rejected_payloads"],
        1
    );
}

#[test]
fn workspace_state_database_that_cannot_be_opened_is_an_environment_error() {
    let db = Db::new("ws-open");
    let k = kernel_root();
    let k = k.to_str().unwrap();
    let corrupt = db.base().join("corrupt.sqlite3");
    std::fs::write(&corrupt, b"not a database").unwrap();
    let other_kernel = TempDir::new("ws-open-kernel");
    std::fs::write(other_kernel.path().join("VERSION"), "9.9.9\n").unwrap();
    let missing = db.base().join("missing.sqlite3");
    let tool = db.tool.to_str().unwrap().to_string();
    for (kernel, path) in [
        (k, missing.to_str().unwrap()),
        (k, corrupt.to_str().unwrap()),
        (k, tool.as_str()),
        (other_kernel.path().to_str().unwrap(), db.ws()),
    ] {
        let output = run(&[
            "validate",
            "--kernel",
            kernel,
            "--workspace-db",
            path,
            "--format",
            "json",
        ]);
        assert_code(&output, 3);
        assert!(stdout_of(&output).is_empty(), "{path}");
        assert!(
            stderr_of(&output).contains(&format!("workspace database {path}: ")),
            "{}",
            stderr_of(&output)
        );
    }
    assert!(!missing.exists(), "validate must never create a database");
}

#[test]
fn workspace_state_log_metrics_records_one_observation_without_changing_the_verdict() {
    let k = kernel_root();
    let usage = run(&["validate", "--kernel", k.to_str().unwrap(), "--log-metrics"]);
    assert_code(&usage, 2);
    assert!(stdout_of(&usage).is_empty());
    assert!(stderr_of(&usage).contains("log-metrics"));

    for (label, spec) in [
        ("ws-metrics-ok", Spec::consistent as fn(&str) -> Spec),
        ("ws-metrics-fail", |r: &str| Spec {
            profile: "vue-monorepo".to_string(),
            ..Spec::consistent(r)
        }),
    ] {
        let db = filled(label, spec, &[]);
        let plain = db.validate(&["--format", "json"]);
        let count = |export: &str| {
            serde_json::from_str::<Value>(export).unwrap()["result"]
                .as_array()
                .unwrap()
                .len()
        };
        let before = count(&db.export());
        let logged = db.validate(&["--log-metrics", "--format", "json"]);
        assert_eq!(logged.status.code(), plain.status.code(), "{label}");
        assert_eq!(
            lines(&logged, "failures"),
            lines(&plain, "failures"),
            "{label}"
        );
        assert!(stderr_of(&logged).is_empty(), "{}", stderr_of(&logged));
        let metrics = &json_result(&logged)["result"]["metrics"];
        assert_eq!(metrics["outcome"], "recorded", "{label}: {metrics}");
        assert!(metrics["record_id"]
            .as_str()
            .unwrap()
            .starts_with("gate-run-observation-"));
        assert_eq!(
            count(&db.export()),
            before + 1,
            "{label}: exactly one observation"
        );
        let after_plain = db.validate(&["--format", "json"]);
        assert_eq!(
            json_result(&after_plain)["result"]["workspace_state"]["observations"],
            1
        );
        assert_eq!(
            count(&db.export()),
            before + 1,
            "{label}: a plain run writes nothing"
        );
        let human = db.validate(&["--log-metrics"]);
        assert!(stdout_of(&human).contains("Metrics: recorded"));
    }

    // A write that cannot happen is reported, never a changed verdict.
    let db = filled(
        "ws-metrics-ambiguous",
        |r| Spec {
            extra: vec![record(
                "report",
                "other-report",
                json!({"type": "project-workspace", "id": "other"}),
                "Other",
                container("text/markdown", "x"),
            )],
            ..Spec::consistent(r)
        },
        &[],
    );
    let plain = db.validate(&["--format", "json"]);
    let logged = db.validate(&["--log-metrics", "--format", "json"]);
    assert_eq!(logged.status.code(), plain.status.code());
    let metrics = &json_result(&logged)["result"]["metrics"];
    assert_eq!(metrics["outcome"], "not-recorded");
    assert_eq!(
        metrics["error"],
        "the workspace database holds records of several project workspaces"
    );
    assert!(stderr_of(&logged).contains("validate: the gate-run observation was not recorded: "));
}

#[test]
fn workspace_state_db_backed_validate_runs_without_network() {
    let db = filled("ws-offline", Spec::consistent, &[]);
    let k = kernel_root();
    let output = Command::new(exe())
        .args([
            "validate",
            "--kernel",
            k.to_str().unwrap(),
            "--workspace-db",
            db.ws(),
            "--log-metrics",
            "--format",
            "json",
        ])
        .env("http_proxy", "http://127.0.0.1:9")
        .env("https_proxy", "http://127.0.0.1:9")
        .env("all_proxy", "http://127.0.0.1:9")
        .env("GIT_ALLOW_PROTOCOL", "none")
        .env_remove("MERIDIAN_INSTANCE")
        .output()
        .unwrap();
    assert_code(&output, 0);
    assert_eq!(
        json_result(&output)["result"]["metrics"]["outcome"],
        "recorded"
    );
}

// ------------------------------------------------------------ decision D3

/// An intake record whose fields other than the artifact are `fields`.
fn intake_of(artifact: &str, fields: Value) -> Value {
    let mut content = json!({
        "artifact": artifact,
        "delivery": "cursor-rule",
        "digest": "b".repeat(64),
        "genre": "standard",
        "observed_activation": "always",
        "recorded_at": "2026-08-21",
        "scope": "repository",
        "verdict_basis": "recorded as observed",
    });
    for (key, value) in fields.as_object().unwrap() {
        content[key] = value.clone();
    }
    container("application/json", &content.to_string())
}

fn skill_record(topic: &str) -> Value {
    record(
        "instruction-intake-record",
        "intake-web-skill",
        repository_scope("web"),
        "Intake web review skill",
        intake_of(
            "skills/review/SKILL.md",
            json!({"delivery": "skill-package", "topic": topic, "verdict": "keep-local"}),
        ),
    )
}

fn edition_record(id: &str, artifact: &str, parent: Value, digest: &str) -> Value {
    record(
        "instruction-intake-record",
        id,
        repository_scope("web"),
        &format!("Intake web {artifact}"),
        intake_of(
            artifact,
            json!({
                "topic": "layer-architecture",
                "verdict": "adopt-edition",
                "derived_from": parent,
                "derived_from_digest": digest,
                "narrowing": ["only the web layer"],
            }),
        ),
    )
}

const PARENT_TEXT: &str = "Layers depend inwards.\n";

/// The files the D3 records name: a skill package, the parent text inside
/// the product repository and the rules that carry the editions.
fn d3_files(skill: &'static str) -> Vec<(&'static str, &'static str)> {
    vec![
        ("skills/review/SKILL.md", skill),
        ("docs/parent.md", PARENT_TEXT),
        (".cursor/rules/layers.mdc", "Layers, narrowed.\n"),
        (".cursor/rules/version.mdc", "Version, narrowed.\n"),
    ]
}

/// The Kernel checkout's own head and the digest of a file committed there
/// — the parent repository `"kernel"` an edition may name.
fn kernel_head_file(path: &str) -> (String, String) {
    let k = kernel_root();
    let head = git(&k, &["rev-parse", "HEAD"]).trim().to_string();
    let text = git(&k, &["show", &format!("HEAD:{path}")]);
    (head, sha256_hex(text.as_bytes()))
}

/// Topic existence, skill packaging and edition parentage of decision D3
/// are judged by the DB-backed production route; a consistent workspace
/// passes all three.
#[test]
fn workspace_state_d3_topic_packaging_and_parentage_are_confirmed_through_the_binary() {
    let (kernel_head, version_digest) = kernel_head_file("VERSION");
    let db = filled(
        "ws-d3-clean",
        |revision| {
            let mut spec = Spec::consistent(revision);
            spec.extra = vec![
                skill_record("code-review"),
                edition_record(
                    "intake-web-layers",
                    ".cursor/rules/layers.mdc",
                    json!({"repository": "web", "path": "docs/parent.md", "revision": revision}),
                    &sha256_hex(PARENT_TEXT.as_bytes()),
                ),
                edition_record(
                    "intake-web-version",
                    ".cursor/rules/version.mdc",
                    json!({"repository": "kernel", "path": "VERSION", "revision": kernel_head}),
                    &version_digest,
                ),
            ];
            spec
        },
        &d3_files("---\nname: review\ndescription: reviews a change\n---\nReview it.\n"),
    );
    let output = db.validate(&["--format", "json"]);
    assert_code(&output, 0);
    assert!(
        lines(&output, "failures").is_empty(),
        "{:?}",
        lines(&output, "failures")
    );
    let family: Vec<String> = lines(&output, "warnings")
        .into_iter()
        .filter(|w| w.starts_with("instruction-intake"))
        .collect();
    assert!(family.is_empty(), "{family:?}");
    let state = &json_result(&output)["result"]["workspace_state"];
    assert_eq!(state["rejected_payloads"], 0);
    assert_eq!(state["intake_records"], 5);
    assert_eq!(state["intake_repositories_complete"], 1);
    let info: Vec<&str> = state["info"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(!info.iter().any(|i| i.contains("UNVERIFIED")), "{info:?}");
}

#[test]
fn workspace_state_d3_every_broken_intake_contract_fails_through_the_binary() {
    let absent_revision = "0123456789abcdef0123456789abcdef01234567";
    let wrong_digest = sha256_hex(b"another text");
    let db = filled(
        "ws-d3-broken",
        |revision| {
            let mut spec = Spec::consistent(revision);
            spec.extra = vec![
                // A topic outside the Kernel's pool.
                skill_record("invented-topic"),
                // The parent at the revision is another text.
                edition_record(
                    "intake-web-layers",
                    ".cursor/rules/layers.mdc",
                    json!({"repository": "web", "path": "docs/parent.md", "revision": revision}),
                    &wrong_digest,
                ),
                // The revision holds no such file.
                edition_record(
                    "intake-web-version",
                    ".cursor/rules/version.mdc",
                    json!({"repository": "web", "path": "docs/gone.md", "revision": revision}),
                    &wrong_digest,
                ),
                // A repository no inventory entry names.
                edition_record(
                    "intake-web-ghost",
                    ".cursor/rules/ghost.mdc",
                    json!({"repository": "ghost", "path": "a.md", "revision": revision}),
                    &wrong_digest,
                ),
                // A revision this checkout does not carry: UNVERIFIED.
                edition_record(
                    "intake-web-shallow",
                    ".cursor/rules/shallow.mdc",
                    json!({"repository": "web", "path": "docs/parent.md", "revision": absent_revision}),
                    &wrong_digest,
                ),
            ];
            spec
        },
        // A misnamed skill package that also speaks the rule vocabulary.
        &[
            d3_files("---\nname: reviewer\nglobs: '*.ts'\n---\nReview it.\n"),
            vec![
                (".cursor/rules/ghost.mdc", "Ghost.\n"),
                (".cursor/rules/shallow.mdc", "Shallow.\n"),
            ],
        ]
        .concat(),
    );
    let output = db.validate(&["--format", "json"]);
    assert_code(&output, 1);
    let taken = sha256_hex(PARENT_TEXT.as_bytes());
    let revision = git(&db.base().join("web"), &["rev-parse", "HEAD"]);
    let short = &revision[..7];
    let expected = [
        "instruction-intake: .cursor/rules/ghost.mdc derives from repository \"ghost\", which the workspace inventory does not name; the reference cannot be resolved by anyone, here or elsewhere".to_string(),
        format!("instruction-intake: .cursor/rules/layers.mdc records parent digest {} but \"docs/parent.md\" at {short} hashes to {}; the reference names a different text, which no re-dating can fix", &wrong_digest[..12], &taken[..12]),
        format!("instruction-intake: .cursor/rules/version.mdc derives from \"docs/gone.md\" at {short} in web, and that revision holds no such file; the reference points at nothing"),
        "instruction-intake: skills/review/SKILL.md declares activation twice — globs belong to the rule vocabulary, not the package one; which one applies is then decided by the tool, not by the author".to_string(),
        "instruction-intake: skills/review/SKILL.md is a skill named \"reviewer\" in a directory named \"review\"; a norm that cannot be found by its own name makes every reference to it dangling".to_string(),
        "instruction-intake: the register of repository \"web\" names 1 topic(s) outside the pool (invented-topic); a new topic is a change to the registry, not to one record".to_string(),
    ];
    assert_eq!(lines(&output, "failures"), expected);
    let info = &json_result(&output)["result"]["workspace_state"]["info"];
    assert!(info.as_array().unwrap().iter().any(|i| i.as_str().unwrap()
        == "instruction-intake: revision 0123456 of web is not present in this checkout; the parent of .cursor/rules/shallow.mdc is UNVERIFIED, not confirmed"), "{info}");
}

/// An edition whose parent repository is gone, and a skill package in a
/// repository that is gone, are UNVERIFIED — never a failure, never
/// confirmed.
#[test]
fn workspace_state_d3_an_unreachable_parent_or_package_is_unverified() {
    let db = filled(
        "ws-d3-unreachable",
        |revision| {
            let mut spec = Spec::consistent(revision);
            spec.extra = vec![
                skill_record("code-review"),
                edition_record(
                    "intake-web-layers",
                    ".cursor/rules/layers.mdc",
                    json!({"repository": "web", "path": "docs/parent.md", "revision": revision}),
                    &sha256_hex(PARENT_TEXT.as_bytes()),
                ),
            ];
            spec
        },
        &d3_files("---\nname: review\n---\n"),
    );
    std::fs::remove_dir_all(db.base().join("web")).unwrap();
    let output = db.validate(&["--format", "json"]);
    assert_code(&output, 0);
    assert!(lines(&output, "failures").is_empty());
    let info: Vec<String> = json_result(&output)["result"]["workspace_state"]["info"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    for expected in [
        "instruction-intake: repository \"web\" is not reachable from here; the parent of .cursor/rules/layers.mdc is UNVERIFIED, not confirmed",
        "instruction-intake: skills/review/SKILL.md not reachable; its packaging is UNVERIFIED, not confirmed",
    ] {
        assert!(info.iter().any(|i| i == expected), "{expected}: {info:?}");
    }
}

// ------------------------------------------- M-05b: the payload matrix

fn matrix_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// One row of `tests/fixtures/payload-matrix/families.json`: the record
/// with its valid payload, and with its mutated one when the family is
/// schema-backed.
struct MatrixRow {
    valid: Value,
    mutated: Option<Value>,
}

fn matrix_rows() -> Vec<MatrixRow> {
    let doc: Value = serde_json::from_str(
        &std::fs::read_to_string(matrix_fixture().join("payload-matrix/families.json")).unwrap(),
    )
    .unwrap();
    let text = |v: &Value| match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    doc["families"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let scope = match f["scope"].as_str().unwrap() {
                "repository" => repository_scope("web"),
                _ => workspace(),
            };
            let media = f["media_type"].as_str().unwrap();
            let build = |id: String, content: String| {
                record(
                    f["record_type"].as_str().unwrap(),
                    &id,
                    scope.clone(),
                    f["title"].as_str().unwrap(),
                    container(media, &content),
                )
            };
            let mutated = match &f["mutation"] {
                Value::Null => None,
                m => Some(match (m.get("remove"), m.get("text")) {
                    (Some(Value::String(field)), _) => {
                        let mut value = f["valid"].clone();
                        value.as_object_mut().unwrap().remove(field);
                        value.to_string()
                    }
                    (_, Some(Value::String(t))) => t.clone(),
                    _ => panic!("{m}"),
                }),
            };
            MatrixRow {
                valid: build(format!("matrix-{i}"), text(&f["valid"])),
                mutated: mutated.map(|m| build(format!("matrix-{i}-mutated"), m)),
            }
        })
        .collect()
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

/// The pinned payload-matrix source with one of its two bundles; returns the
/// source path and the bundle's plan fingerprint.
fn matrix_source(base: &Path, bundle: &str) -> (PathBuf, String) {
    let fixture = matrix_fixture().join("frozen-instance/payload-matrix");
    let expected: Value =
        serde_json::from_str(&std::fs::read_to_string(fixture.join("expected.json")).unwrap())
            .unwrap();
    let repo = base.join(format!("matrix-{bundle}"));
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    copy_tree(&fixture.join("source"), &repo);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "frozen instance fixture"]);
    assert_eq!(
        git(&repo, &["rev-parse", "HEAD"]).trim(),
        expected["revision"].as_str().unwrap(),
        "the payload-matrix commit must be reproducible"
    );
    let target = repo.join("migration/instance-data");
    std::fs::create_dir_all(&target).unwrap();
    copy_tree(&fixture.join(bundle), &target);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "accepted bundle"]);
    let fingerprint = expected[bundle]["plan_fingerprint"]
        .as_str()
        .unwrap()
        .to_string();
    (repo, fingerprint)
}

impl Db {
    fn import_frozen(&self, source: &Path, fingerprint: &str) -> Output {
        let k = kernel_root();
        run(&[
            "import",
            "--kernel",
            k.to_str().unwrap(),
            "--kind",
            "frozen-instance",
            "--source",
            source.to_str().unwrap(),
            "--tool-db",
            self.tool.to_str().unwrap(),
            "--workspace-db",
            self.ws(),
            "--confirm",
            fingerprint,
            "--format",
            "json",
        ])
    }
}

fn strings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(s) => out.push(s.clone()),
        Value::Array(items) => items.iter().for_each(|v| strings(v, out)),
        Value::Object(map) => map.values().for_each(|v| strings(v, out)),
        _ => {}
    }
}

fn record_count(export: &str) -> usize {
    serde_json::from_str::<Value>(export).unwrap()["result"]
        .as_array()
        .unwrap()
        .len()
}

/// Acceptance criterion 3 over the frozen-instance import: every valid
/// family of the matrix is accepted and stored, every mutated
/// schema-backed family is refused before anything is written, and the
/// stored database is judged clean.
#[test]
fn workspace_state_payload_matrix_through_the_frozen_import() {
    let db = Db::new("ws-matrix-frozen");
    let empty = db.export();
    let (mutated, fingerprint) = matrix_source(db.base(), "mutated");
    let output = db.import_frozen(&mutated, &fingerprint);
    assert_code(&output, 1);
    let mut refusals = Vec::new();
    strings(&json_result(&output), &mut refusals);
    let expected: Value = serde_json::from_str(
        &std::fs::read_to_string(
            matrix_fixture().join("frozen-instance/payload-matrix/expected.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let refused = expected["mutated"]["refused_targets"].as_array().unwrap();
    assert_eq!(refused.len(), 15);
    for id in refused {
        let prefix = format!("payload-contract: target \"{}\" (", id.as_str().unwrap());
        assert!(
            refusals.iter().any(|l| l.starts_with(&prefix)),
            "{prefix}: {refusals:?}"
        );
    }
    assert_eq!(db.export(), empty, "a refused import writes nothing");

    let (accepted, fingerprint) = matrix_source(db.base(), "accepted");
    assert_code(&db.import_frozen(&accepted, &fingerprint), 0);
    assert_eq!(
        record_count(&db.export()),
        record_count(&empty) + expected["accepted"]["migrated"].as_u64().unwrap() as usize
    );
    let output = db.validate(&["--format", "json"]);
    let state = &json_result(&output)["result"]["workspace_state"];
    assert_eq!(state["rejected_payloads"], 0, "{}", stdout_of(&output));
    assert!(!lines(&output, "failures")
        .iter()
        .any(|f| f.starts_with("payload-contract:")));
}

/// Acceptance criterion 3 over the canonical-records import and the stored
/// state: every valid family is accepted; each mutated schema-backed family
/// is refused before anything is written; the same mutations planted
/// directly in SQLite are each found by `validate`.
#[test]
fn workspace_state_payload_matrix_through_the_canonical_import_and_the_stored_state() {
    let rows = matrix_rows();
    let db = Db::new("ws-matrix-canonical");
    let valid: Vec<Value> = rows.iter().map(|r| r.valid.clone()).collect();
    assert_code(&db.import(&valid), 0);
    let before = db.export();
    let output = db.validate(&["--format", "json"]);
    assert_eq!(
        json_result(&output)["result"]["workspace_state"]["rejected_payloads"],
        0,
        "{}",
        stdout_of(&output)
    );

    let mutated: Vec<&Value> = rows.iter().filter_map(|r| r.mutated.as_ref()).collect();
    assert_eq!(mutated.len(), 15);
    for record in &mutated {
        let output = db.import(&[(*record).clone()]);
        assert_code(&output, 3);
        assert!(stdout_of(&output).is_empty());
        assert!(
            stderr_of(&output).contains("payload-contract: "),
            "{}: {}",
            record["id"],
            stderr_of(&output)
        );
        assert_eq!(
            db.export(),
            before,
            "{}: a refused import writes nothing",
            record["id"]
        );
    }

    // Plant each mutation as a copy of its valid row, bypassing import.
    {
        let conn = rusqlite::Connection::open(&db.ws).unwrap();
        for (i, row) in rows.iter().enumerate() {
            let Some(bad) = &row.mutated else { continue };
            let source_id = format!("matrix-{i}");
            let payload = bad["payload"].to_string();
            conn.execute(
                "INSERT INTO records SELECT record_key || '-planted', scope_type, scope_id, scope_workspace_id, scope_organization_profile_id, record_id || '-planted', schema_ref, schema_version, title, record_type, origin_kind, origin_source_ref, authority_kind, authority_ref, authority_decision_ref, ?2, content_digest, current_revision FROM records WHERE record_id = ?1",
                rusqlite::params![source_id, payload],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO record_revisions SELECT v.record_key || '-planted', v.revision_number, v.schema_ref, v.schema_version, v.title, v.record_type, v.origin_kind, v.origin_source_ref, v.authority_kind, v.authority_ref, v.authority_decision_ref, ?2, v.content_digest, v.idempotency_key || '-planted', v.recorded_at FROM record_revisions v JOIN records r ON r.record_key = v.record_key WHERE r.record_id = ?1",
                rusqlite::params![source_id, payload],
            )
            .unwrap();
        }
    }
    let output = db.validate(&["--format", "json"]);
    assert_code(&output, 1);
    assert_eq!(
        json_result(&output)["result"]["workspace_state"]["rejected_payloads"],
        15,
        "{}",
        stdout_of(&output)
    );
    let failures = lines(&output, "failures");
    for (i, row) in rows.iter().enumerate() {
        if row.mutated.is_none() {
            continue;
        }
        let title = row.valid["title"].as_str().unwrap();
        let planted = format!("\"matrix-{i}-planted\"");
        let named = match title {
            // The typed workspace files report under their own family.
            "product.yaml" => failures.iter().any(|f| f.starts_with("product record: ")),
            "external-dependencies.yaml" => {
                failures.iter().any(|f| f.starts_with("ext-dependencies: "))
            }
            _ => failures
                .iter()
                .any(|f| f.starts_with("payload-contract: ") && f.contains(&planted)),
        };
        assert!(named, "{title}: {failures:?}");
    }
}
