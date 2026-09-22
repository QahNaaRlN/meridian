//! The single production `GitInspector`
//! (`meridian_app::workspace::GitInspector`): spawns real `git` plumbing
//! (`std::process::Command`, never a shell string) against one workspace
//! root and validates every path it reports as a
//! [`WorkspaceRelativePath`]. `std::process` for this port lives only here
//! (`rust-architecture-conformance-3`,
//! `governance/plans/meridian-rust-migration-program-plan.md` §5.17.3,
//! point 5).

use std::path::{Path, PathBuf};
use std::process::Command;

use meridian_app::workspace::{GitInspector, GitInspectorError};
use meridian_core::types::WorkspaceRelativePath;

pub struct RealGitInspector {
    root: PathBuf,
}

impl RealGitInspector {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl GitInspector for RealGitInspector {
    fn tracked_files(&self) -> Result<Vec<WorkspaceRelativePath>, GitInspectorError> {
        list_tracked(&self.root)
    }
}

/// A [`GitInspector`] over an already-resolved snapshot, never spawning
/// `git` itself. `meridian validate` calls [`RealGitInspector::tracked_files`]
/// exactly once per invocation (in `commands::validate::collect`) and wraps
/// the single `Result` in one of these so every consumer within that run —
/// `kernel-purity`, `document-identity` and `task-pattern-registry` — reads
/// the same snapshot through the same app-owned port, instead of each
/// spawning its own real `git ls-files` (`rust-architecture-conformance-3`
/// corrective round, "one Git snapshot for the whole `validate`").
pub struct CachedGitInspector {
    snapshot: Result<Vec<WorkspaceRelativePath>, GitInspectorError>,
}

impl CachedGitInspector {
    pub fn new(snapshot: Result<Vec<WorkspaceRelativePath>, GitInspectorError>) -> Self {
        Self { snapshot }
    }
}

impl GitInspector for CachedGitInspector {
    fn tracked_files(&self) -> Result<Vec<WorkspaceRelativePath>, GitInspectorError> {
        self.snapshot.clone()
    }
}

fn list_tracked(root: &Path) -> Result<Vec<WorkspaceRelativePath>, GitInspectorError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z"])
        .output()
        .map_err(|_| GitInspectorError::Unavailable)?;
    if !output.status.success() {
        return Err(GitInspectorError::Unavailable);
    }
    let text = String::from_utf8(output.stdout).map_err(|_| GitInspectorError::Unavailable)?;
    let mut files = Vec::new();
    for raw in text.split('\0').filter(|s| !s.is_empty()) {
        let path =
            WorkspaceRelativePath::new(raw).map_err(|error| GitInspectorError::InvalidPath {
                raw: raw.to_string(),
                reason: error.to_string(),
            })?;
        files.push(path);
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempRepo {
        dir: PathBuf,
    }

    impl TempRepo {
        /// Constructs the RAII guard immediately after the one directory
        /// creation this needs, BEFORE any of the several subsequent
        /// fallible/panicking `git`/filesystem operations below
        /// (`rust-architecture-conformance-3` corrective round item 5): if
        /// `git init`/`config`/`commit` or a write panics partway through
        /// setup, `repo` already exists and its `Drop` still cleans up the
        /// directory during unwinding, instead of leaking it the way
        /// returning a bare `TempRepo { dir }` only at the end would if any
        /// earlier step never got there.
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "real-git-inspector-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let repo = TempRepo { dir };
            let run = |args: &[&str]| {
                let status = Command::new("git")
                    .arg("-C")
                    .arg(&repo.dir)
                    .args(args)
                    .status()
                    .expect("git runs");
                assert!(status.success(), "git {args:?} must succeed");
            };
            run(&["init", "-q"]);
            run(&["config", "user.email", "test@example.invalid"]);
            run(&["config", "user.name", "Test"]);
            std::fs::write(repo.dir.join("tracked.md"), "x").unwrap();
            run(&["add", "tracked.md"]);
            run(&["commit", "-q", "-m", "init"]);
            std::fs::write(repo.dir.join("untracked.md"), "x").unwrap();
            repo
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn lists_exactly_the_tracked_files_of_a_real_repository() {
        let repo = TempRepo::new("list");
        let files = list_tracked(&repo.dir).unwrap();
        let names: Vec<&str> = files.iter().map(WorkspaceRelativePath::as_str).collect();
        assert_eq!(names, vec!["tracked.md"]);
    }

    #[test]
    fn reports_unavailable_for_a_directory_that_is_not_a_git_repository() {
        // Same RAII-first ordering as `TempRepo::new` above: the guard
        // exists before the one subsequent fallible call
        // (`list_tracked`/`unwrap_err`), so a panic there still cleans up
        // rather than leaking the directory via a manual, unreachable-on-
        // panic `remove_dir_all` at the end.
        struct Guard(PathBuf);
        impl Drop for Guard {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let dir = std::env::temp_dir().join(format!(
            "real-git-inspector-not-a-repo-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let guard = Guard(dir);
        let result = list_tracked(&guard.0);
        assert_eq!(result.unwrap_err(), GitInspectorError::Unavailable);
    }

    #[test]
    fn cached_git_inspector_returns_the_wrapped_snapshot_without_spawning_git() {
        let path = WorkspaceRelativePath::new("tracked.md").unwrap();
        let cached = CachedGitInspector::new(Ok(vec![path.clone()]));
        assert_eq!(cached.tracked_files().unwrap(), vec![path.clone()]);
        // Repeated reads return the same snapshot — no re-invocation of any
        // underlying process, since `CachedGitInspector` holds no `Command`.
        assert_eq!(cached.tracked_files().unwrap(), vec![path]);
    }

    #[test]
    fn cached_git_inspector_preserves_an_error_snapshot() {
        let cached = CachedGitInspector::new(Err(GitInspectorError::Unavailable));
        assert_eq!(
            cached.tracked_files().unwrap_err(),
            GitInspectorError::Unavailable
        );
    }
}
