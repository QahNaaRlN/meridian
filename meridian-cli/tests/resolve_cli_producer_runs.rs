//! Proves `meridian-cli/examples/resolve_cli_producer.rs`'s own rejected-branch
//! formatting (`format!("rejected:exit={exit_code}:{stderr_text}")`) actually
//! runs, not only that the compiled `meridian` binary itself distinguishes
//! exit 2 from exit 3 (`meridian-cli/tests/binary_runs.rs`'s
//! `resolve_distinguishes_a_usage_error_from_a_rejected_request_by_exit_code`
//! proves that half already). A test of the direct binary alone cannot prove
//! the producer's own translation of a child process's exit code and stderr
//! into one `WARN` line is correct — that line is a separate piece of code,
//! and it is untested by every other suite in this crate.
//!
//! The producer's default, embedded corpus
//! (`verification/conformance-harness/fixtures/rule-resolution-corpus.json`)
//! is shared with `rule-resolution-node-producer.mjs` inside the
//! `real-node-rust-cli-resolve` conformance fixture, which compares the two
//! producers' output byte-for-byte and expects `conformant`. The Node
//! producer collapses every thrown rejection into the bare literal
//! `"rejected"`, so a rejected case in that shared corpus would make the two
//! producers' output diverge by construction, regardless of any Rust defect.
//! This test therefore runs the producer against its own, separate corpus
//! file (via the optional override argument `resolve_cli_producer.rs`
//! accepts for exactly this purpose) instead of touching the shared one.

use std::path::PathBuf;
use std::process::Command;

fn producer_exe() -> PathBuf {
    // Mirrors resolve_cli_producer.rs's own reasoning for locating its
    // sibling `meridian` binary, in reverse: CARGO_BIN_EXE_meridian's parent
    // is the profile directory (e.g. target/debug), and every example lives
    // in that directory's own `examples/` subdirectory.
    let meridian_bin = PathBuf::from(env!("CARGO_BIN_EXE_meridian"));
    let profile_dir = meridian_bin
        .parent()
        .expect("meridian binary has a parent directory");
    profile_dir.join("examples").join(if cfg!(windows) {
        "resolve_cli_producer.exe"
    } else {
        "resolve_cli_producer"
    })
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "resolve-cli-producer-test-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        TempDir(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn the_rejected_branch_prints_the_real_exit_code_and_a_non_empty_stderr_detail() {
    let producer = producer_exe();
    assert!(
        producer.is_file(),
        "expected the compiled resolve_cli_producer example at {}; run `cargo build --workspace --examples` first",
        producer.display()
    );

    let temp = TempDir::new("rejected");
    // An unknown transport field is content the well-formed CLI invocation
    // reads and then refuses (exit_code::INPUT_OR_ENVIRONMENT, exit 3) — the
    // same shape `binary_runs.rs`'s `resolve_rejects_unknown_transport_fields`
    // exercises directly against the binary.
    let rejected_request = serde_json::json!({
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
        },
        "reviewer": "must-not-affect-resolution"
    });
    let accepted_request = serde_json::json!({
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
    let corpus = serde_json::json!({
        "cases": [
            {"name": "accepted-empty-operation", "request": accepted_request},
            {"name": "rejected-unknown-transport-field", "request": rejected_request},
        ]
    });
    let corpus_path = temp.0.join("corpus.json");
    std::fs::write(&corpus_path, serde_json::to_string_pretty(&corpus).unwrap())
        .expect("write override corpus");

    let output = Command::new(&producer)
        .arg(&corpus_path)
        .output()
        .expect("run resolve_cli_producer");
    assert!(
        output.status.success(),
        "resolve_cli_producer itself must exit 0 even when a corpus case is rejected — stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("producer stdout is UTF-8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines.len(), 2, "stdout: {stdout:?}");

    let accepted_line = lines
        .iter()
        .find(|l| l.contains("accepted-empty-operation"))
        .unwrap_or_else(|| panic!("no line for the accepted case in: {stdout:?}"));
    assert!(
        accepted_line.contains(": accepted:"),
        "accepted case must go through the accepted branch: {accepted_line}"
    );

    let rejected_line = lines
        .iter()
        .find(|l| l.contains("rejected-unknown-transport-field"))
        .unwrap_or_else(|| panic!("no line for the rejected case in: {stdout:?}"));
    assert!(
        rejected_line.contains(": rejected:exit=3:"),
        "rejected case must carry the real exit code (3, INPUT_OR_ENVIRONMENT): {rejected_line}"
    );
    let detail = rejected_line
        .split_once("rejected:exit=3:")
        .map(|(_, detail)| detail)
        .expect("rejected line has a detail suffix");
    assert!(
        !detail.trim().is_empty(),
        "a rejected observation without any stderr detail hides the actual reason: {rejected_line}"
    );
}
