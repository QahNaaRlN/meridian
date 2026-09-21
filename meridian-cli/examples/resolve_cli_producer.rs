//! Verification-only producer for `resolve` cross-language conformance — at
//! the `meridian-cli` boundary specifically, not only `meridian-app`
//! (`verification/conformance-harness/README.md`, "Real-producer reuse").
//!
//! Unlike `meridian-app/examples/rule_resolution_producer.rs` (which calls
//! `meridian_app::rule_resolution::resolve` directly), this spawns the
//! *actual compiled* `meridian` binary once per corpus case
//! (`std::process::Command`, the request piped on its stdin — the same
//! `--kernel`/`--format json` invocation `meridian resolve` documents), and
//! reads back its real stdout and real process exit status. Nothing here
//! calls `meridian_cli::commands::resolve::run` or any other library
//! function directly: nothing about the resolve request ever gets a result
//! from a function call, only from a separate operating-system process this
//! program did not write the resolver logic of (`AGENTS.md`
//! `meridian-rust-migration-program-plan.md`, item 2 of the second
//! `CHANGES_REQUESTED` round on package `meridian-cli-foundation` —
//! "verification-only adapter may normalize observations after process
//! invocation, but must not replace the process with a library call").

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::Value;

fn main() {
    // An optional first argument overrides the embedded shared corpus with a
    // corpus file of the caller's own — used only by
    // `meridian-cli/tests/resolve_cli_producer_runs.rs` to prove the
    // `rejected:exit=...` branch below actually runs end to end. The shared
    // corpus embedded by default is the one the Node/Rust conformance
    // fixture (`real-node-rust-cli-resolve`) compares byte-for-byte against
    // `rule-resolution-node-producer.mjs`'s own bare "rejected" text, so it
    // cannot itself carry a rejected case without breaking that comparison;
    // an override corpus is how this producer's rejected-branch formatting
    // is exercised without touching that shared, cross-language fixture.
    let corpus: Value = match std::env::args().nth(1) {
        Some(path) => {
            let raw = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("read corpus override {path}: {e}"));
            serde_json::from_str(&raw)
                .unwrap_or_else(|e| panic!("parse corpus override {path}: {e}"))
        }
        None => serde_json::from_str(include_str!(
            "../../verification/conformance-harness/fixtures/rule-resolution-corpus.json"
        ))
        .expect("parse embedded rule-resolution corpus"),
    };

    let kernel_root: PathBuf = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("meridian-cli has a parent directory")
        .to_path_buf();
    let kernel_str = kernel_root
        .to_str()
        .expect("kernel path is UTF-8")
        .to_string();
    // No `CARGO_BIN_EXE_meridian` here: Cargo only injects that variable for
    // integration tests and benchmarks, not for examples. This example's own
    // executable path is `<profile-dir>/examples/resolve_cli_producer`, so
    // the real `meridian` binary Cargo built alongside it is the sibling
    // `<profile-dir>/meridian` — two directories up, not one.
    let meridian_bin = std::env::current_exe()
        .expect("this example's own executable path")
        .parent()
        .expect("examples/ directory")
        .parent()
        .expect("profile directory (e.g. target/debug)")
        .join(if cfg!(windows) {
            "meridian.exe"
        } else {
            "meridian"
        });
    assert!(
        meridian_bin.is_file(),
        "expected the real meridian binary built alongside this example at {}",
        meridian_bin.display()
    );

    for case in corpus["cases"].as_array().expect("rule-resolution cases") {
        let name = case["name"].as_str().expect("case name");
        let request_text = serde_json::to_string(&case["request"]).expect("serialize request");

        let mut child = Command::new(&meridian_bin)
            .args(["resolve", "--kernel", &kernel_str, "--format", "json"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn real meridian binary");
        child
            .stdin
            .take()
            .expect("child stdin")
            .write_all(request_text.as_bytes())
            .expect("write request to child stdin");
        let output = child.wait_with_output().expect("wait for meridian resolve");

        let observation = if output.status.success() {
            let stdout_text = String::from_utf8(output.stdout).expect("resolve stdout is UTF-8");
            let envelope: Value = serde_json::from_str(stdout_text.trim())
                .expect("resolve stdout is one JSON document");
            // `serde_json::Value::Object` is a `BTreeMap` in this workspace
            // (no `preserve_order` feature enabled anywhere), so this is
            // already recursively sorted-key JSON — the same canonical form
            // `rule-resolution-node-producer.mjs`'s own `canonical()`
            // produces by explicitly sorting `Object.keys`.
            format!(
                "accepted:{}",
                serde_json::to_string(&envelope["result"]).expect("serialize result")
            )
        } else {
            // Package meridian-cli-foundation, subpackage
            // validate-mechanical-integrity, item 5: a rejected request
            // keeps its real exit code and its real stderr diagnostic —
            // collapsing every nonzero exit into one bare "rejected" string
            // (the prior shape of this producer) would make a future
            // regression that changed exit 3 (INPUT_OR_ENVIRONMENT) into
            // exit 2 (USAGE), or silenced the diagnostic entirely,
            // invisible to this producer's own output.
            let exit_code = output
                .status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "signal".to_string());
            let stderr_text = String::from_utf8_lossy(&output.stderr)
                .trim()
                .replace('\n', " / ");
            format!("rejected:exit={exit_code}:{stderr_text}")
        };
        println!("WARN  resolver/{name}: {observation}");
    }
}
