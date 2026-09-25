//! The single production `GitInspector`
//! (`meridian_app::workspace::GitInspector`): spawns real `git` plumbing
//! (`std::process::Command`, never a shell string) against one workspace
//! root and validates every path it reports as a
//! [`WorkspaceRelativePath`]. `std::process` for this port lives only here
//! (`rust-architecture-conformance-3`,
//! `governance/plans/meridian-rust-migration-program-plan.md` §5.17.3,
//! point 5).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use meridian_app::workspace::{GitInspector, GitInspectorError, WorkspaceReader};
use meridian_app::workspace_state::{RepositoryAccess, RepositoryUnavailable, RevisionSelector};
use meridian_core::workspace_state::inventory::ObservedVcs;
use meridian_core::workspace_state::parentage::CommitRevision;

use super::workspace_reader::FsWorkspaceReader;
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

/// The single production [`RepositoryAccess`]: the same real `git`
/// plumbing, against each product repository the inventory names. The
/// repository roots arrive as explicit arguments from the app operation,
/// never from the environment or the current directory.
pub struct RealRepositoryAccess;

fn git_output(
    root: &str,
    args: &[&str],
    stdin: Option<&[u8]>,
) -> Result<Output, RepositoryUnavailable> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(root)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    command.stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    let mut child = command
        .spawn()
        .map_err(|e| RepositoryUnavailable(format!("git cannot be started: {e}")))?;
    if let (Some(bytes), Some(mut pipe)) = (stdin, child.stdin.take()) {
        pipe.write_all(bytes)
            .map_err(|e| RepositoryUnavailable(format!("git input cannot be written: {e}")))?;
    }
    child
        .wait_with_output()
        .map_err(|e| RepositoryUnavailable(format!("git did not finish: {e}")))
}

fn git_text(root: &str, args: &[&str]) -> Result<String, RepositoryUnavailable> {
    let output = git_output(root, args, None)?;
    if !output.status.success() {
        return Err(RepositoryUnavailable(format!(
            "git {} failed in {root}",
            args.join(" ")
        )));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| RepositoryUnavailable(format!("git {} output is not UTF-8", args.join(" "))))
}

fn nul_separated(text: &str) -> Vec<String> {
    text.split('\0')
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

impl RepositoryAccess for RealRepositoryAccess {
    fn vcs_state(&self, root: &str) -> Result<ObservedVcs, RepositoryUnavailable> {
        Ok(ObservedVcs {
            revision: git_text(root, &["rev-parse", "HEAD"])?.trim().to_string(),
            git_ref: git_text(root, &["rev-parse", "--abbrev-ref", "HEAD"])?
                .trim()
                .to_string(),
            dirty: !git_text(root, &["status", "--porcelain"])?
                .trim()
                .is_empty(),
        })
    }

    fn tracked_files(&self, root: &str) -> Result<Vec<String>, RepositoryUnavailable> {
        git_text(root, &["ls-files", "-z"]).map(|t| nul_separated(&t))
    }

    fn untracked_files(&self, root: &str) -> Result<Vec<String>, RepositoryUnavailable> {
        git_text(root, &["ls-files", "-z", "--others", "--exclude-standard"])
            .map(|t| nul_separated(&t))
    }

    fn ignored_among(
        &self,
        root: &str,
        candidates: &[String],
    ) -> Result<Vec<String>, RepositoryUnavailable> {
        let existing: Vec<&String> = candidates
            .iter()
            .filter(|c| Path::new(root).join(c.as_str()).exists())
            .collect();
        if existing.is_empty() {
            return Ok(Vec::new());
        }
        let input: String = existing.iter().map(|c| format!("{c}\0")).collect();
        let output = git_output(
            root,
            &["check-ignore", "--stdin", "-z"],
            Some(input.as_bytes()),
        )?;
        // Exit 1 is git's own "none of them is ignored".
        match output.status.code() {
            Some(0) | Some(1) => String::from_utf8(output.stdout)
                .map(|t| nul_separated(&t))
                .map_err(|_| {
                    RepositoryUnavailable("git check-ignore output is not UTF-8".to_string())
                }),
            _ => Err(RepositoryUnavailable(format!(
                "git check-ignore failed in {root}"
            ))),
        }
    }

    fn has_revision(
        &self,
        root: &str,
        revision: &CommitRevision,
    ) -> Result<bool, RepositoryUnavailable> {
        let object = format!("{}^{{commit}}", revision.as_str());
        let output = git_output(root, &["rev-parse", "--verify", "--quiet", &object], None)?;
        revision_presence(root, revision, output.status.code())
    }

    fn file_at(
        &self,
        root: &str,
        at: RevisionSelector<'_>,
        path: &WorkspaceRelativePath,
    ) -> Result<Option<String>, RepositoryUnavailable> {
        let revision = match at {
            RevisionSelector::Head => "HEAD",
            RevisionSelector::Commit(commit) => commit.as_str(),
        };
        // `ls-tree` answers "no such file" with an empty listing and exit
        // 0, so absence is never confused with a failing `git`.
        let listing = git_text(
            root,
            &[
                "ls-tree",
                "-z",
                "--full-tree",
                revision,
                "--",
                path.as_str(),
            ],
        )?;
        let is_blob = nul_separated(&listing).iter().any(|entry| {
            entry
                .split_once('\t')
                .is_some_and(|(meta, name)| name == path.as_str() && meta.contains(" blob "))
        });
        if !is_blob {
            return Ok(None);
        }
        let object = format!("{revision}:{}", path.as_str());
        let output = git_output(root, &["cat-file", "blob", &object], None)?;
        if !output.status.success() {
            return Err(RepositoryUnavailable(format!(
                "git cat-file blob {object} failed in {root}"
            )));
        }
        // The reference hashes the text as `git show` decodes it.
        Ok(Some(String::from_utf8_lossy(&output.stdout).into_owned()))
    }

    fn reader(&self, root: &str) -> Box<dyn WorkspaceReader + '_> {
        Box::new(FsWorkspaceReader::new(Path::new(root)))
    }
}

/// What `git rev-parse --verify --quiet <revision>^{commit}` exiting with
/// `code` says: 0 — the commit is present; 1 — git's own "no such commit"
/// (an absent object, or one that is not a commit); any other code, or an
/// end by signal (`None`), is a failure of `git`, never an answer.
fn revision_presence(
    root: &str,
    revision: &CommitRevision,
    code: Option<i32>,
) -> Result<bool, RepositoryUnavailable> {
    match code {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        Some(other) => Err(RepositoryUnavailable(format!(
            "git rev-parse --verify {} exited with {other} in {root}",
            revision.as_str()
        ))),
        None => Err(RepositoryUnavailable(format!(
            "git rev-parse --verify {} was ended by a signal in {root}",
            revision.as_str()
        ))),
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

    fn head_of(repo: &TempRepo) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(&repo.dir)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("git runs");
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    }

    #[test]
    fn workspace_state_has_revision_finds_a_present_commit() {
        let repo = TempRepo::new("revision-present");
        let head = CommitRevision::new(head_of(&repo)).unwrap();
        let root = repo.dir.display().to_string();
        assert_eq!(RealRepositoryAccess.has_revision(&root, &head), Ok(true));
        let short = CommitRevision::new(&head.as_str()[..7]).unwrap();
        assert_eq!(RealRepositoryAccess.has_revision(&root, &short), Ok(true));
    }

    #[test]
    fn workspace_state_has_revision_answers_false_only_for_an_absent_commit() {
        let repo = TempRepo::new("revision-absent");
        let root = repo.dir.display().to_string();
        let absent = CommitRevision::new("0123456789abcdef0123456789abcdef01234567").unwrap();
        assert_eq!(RealRepositoryAccess.has_revision(&root, &absent), Ok(false));
    }

    #[test]
    fn workspace_state_has_revision_reports_a_git_failure_as_unavailable() {
        struct Guard(PathBuf);
        impl Drop for Guard {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let dir = Guard(std::env::temp_dir().join(format!(
            "real-git-inspector-revision-error-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
        std::fs::create_dir_all(&dir.0).unwrap();
        let revision = CommitRevision::new("0123456789abcdef0123456789abcdef01234567").unwrap();
        // Not a repository: git exits 128, which is no answer about the
        // commit.
        let error = RealRepositoryAccess
            .has_revision(&dir.0.display().to_string(), &revision)
            .unwrap_err();
        assert!(error.0.contains("exited with 128"), "{error}");
        // An exit code other than 0/1, and an end by signal, are failures.
        assert!(revision_presence("/r", &revision, Some(2)).is_err());
        assert!(revision_presence("/r", &revision, None)
            .unwrap_err()
            .0
            .contains("signal"));
        assert_eq!(revision_presence("/r", &revision, Some(1)), Ok(false));
        assert_eq!(revision_presence("/r", &revision, Some(0)), Ok(true));
    }
}
