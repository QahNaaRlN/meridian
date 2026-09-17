//! Proves the minimal composition-root binary actually runs to completion,
//! per `rust-workspace-foundation`'s acceptance criterion "запуска
//! минимального бинарника без реализации будущего CLI".

use std::process::Command;

#[test]
fn binary_runs_and_exits_successfully() {
    let exe = env!("CARGO_BIN_EXE_meridian");
    let output = Command::new(exe).output().expect("run meridian binary");
    assert!(output.status.success(), "meridian binary must exit 0");
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    assert!(stdout.contains("meridian-app"));
    assert!(stdout.contains("meridian-storage-sqlite"));
}
