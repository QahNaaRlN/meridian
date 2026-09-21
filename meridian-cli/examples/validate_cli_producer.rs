//! Verification-only producer for `validate` cross-language conformance.
//!
//! Spawns the *actual compiled* `meridian` binary (`std::process::Command`,
//! never `meridian_cli::commands::validate::collect_diagnostics` or any
//! other library call) as `meridian validate --kernel <argv[1]> --format
//! json`, parses its own real stdout JSON, and re-emits its `failures` and
//! `warnings` arrays as `FAIL  <message>` / `WARN  <message>` lines — the
//! same two-space diagnostic convention `scripts/kernel-validate.mjs`'s own
//! `fail`/`warn` helpers use and the convention
//! `verification/conformance-harness/conformance-harness.mjs` parses on
//! both sides. This re-emission is *reformatting of the real subprocess's
//! own real output*, not a second implementation: it never reads a Kernel
//! file, never runs a check, and would break (not silently substitute) if
//! the real binary's JSON shape ever changed underneath it.
//!
//! `--format json`, not `--format human`, is used here specifically because
//! `validate`'s human-format trailer line is a bare `OK`/`FAIL` verdict
//! marker (`meridian-cli/src/commands/validate/mod.rs`) — indistinguishable
//! from an unparseable diagnostic line under this harness's own strict
//! `FAIL  <message>` convention. The JSON envelope's `failures`/`warnings`
//! arrays carry the exact same content without that trailer.
//!
//! Exit code matches the real process's own exit status exactly — never
//! recomputed from the parsed content.

use std::path::PathBuf;
use std::process::Command;

use serde_json::Value;

fn main() {
    let kernel_root = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .expect("usage: validate_cli_producer <kernel-root>");

    // This example's own executable path is
    // `<profile-dir>/examples/validate_cli_producer`; the real `meridian`
    // binary Cargo built alongside it is the sibling `<profile-dir>/meridian`
    // (`CARGO_BIN_EXE_meridian` is not available to examples, only to
    // integration tests and benchmarks).
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

    let output = Command::new(&meridian_bin)
        .args(["validate", "--kernel"])
        .arg(&kernel_root)
        .args(["--format", "json"])
        .output()
        .expect("spawn real meridian binary");

    let exit_code = output.status.code().expect("meridian exited normally");
    let stdout_text = String::from_utf8(output.stdout).expect("validate stdout is UTF-8");

    if !stdout_text.trim().is_empty() {
        let envelope: Value = serde_json::from_str(stdout_text.trim())
            .expect("validate --format json stdout is one JSON document");
        let result = &envelope["result"];
        for failure in result["failures"].as_array().expect("failures array") {
            println!("FAIL  {}", failure.as_str().expect("failure is a string"));
        }
        for warning in result["warnings"].as_array().expect("warnings array") {
            println!("WARN  {}", warning.as_str().expect("warning is a string"));
        }
    }

    std::process::exit(exit_code);
}
