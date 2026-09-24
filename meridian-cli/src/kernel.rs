//! Filesystem helpers shared by the commands that read a named Kernel
//! checkout (`meridian-cli-rfc.md` §"Расположение `instance-template`").
//!
//! Nothing here embeds `instance-template/` or any registry schema into the
//! binary: every path is read from the `--kernel` argument at run time, so a
//! `meridian` build always reflects whichever Kernel checkout it is pointed
//! at, never a copy frozen at build time.
//!
//! Every directory walk here is fail-closed: a `read_dir`, `DirEntry` or
//! `file_type` error stops the walk and is returned as a typed
//! [`WalkError`], never silently skipped (`filter_map(Result::ok)`) and
//! never absorbed into a quiet early `return` of a partial, already-built
//! result. A caller that cannot tell "the tree is small" from "part of the
//! tree could not be read" cannot honestly report a positive result over
//! it — `validate` (`crate::commands::validate`) turns any [`WalkError`]
//! into [`crate::exit_code::INPUT_OR_ENVIRONMENT`], never a clean or a
//! domain-negative result.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use meridian_core::types::{Revision, RevisionError};

/// A directory walk could not be completed. Each variant names the exact
/// filesystem call that failed and the path it failed on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WalkError {
    ReadDir { path: PathBuf, message: String },
    DirEntry { path: PathBuf, message: String },
    FileType { path: PathBuf, message: String },
}

impl std::fmt::Display for WalkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WalkError::ReadDir { path, message } => {
                write!(f, "cannot list directory {}: {message}", path.display())
            }
            WalkError::DirEntry { path, message } => write!(
                f,
                "cannot read a directory entry under {}: {message}",
                path.display()
            ),
            WalkError::FileType { path, message } => {
                write!(
                    f,
                    "cannot determine the file type of {}: {message}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for WalkError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelError {
    VersionUnreadable { path: PathBuf, message: String },
    InvalidVersion(RevisionError),
    TemplateMissing { path: PathBuf },
    Walk(WalkError),
    Io { path: PathBuf, message: String },
}

impl std::fmt::Display for KernelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KernelError::VersionUnreadable { path, message } => {
                write!(
                    f,
                    "cannot read Kernel VERSION at {}: {message}",
                    path.display()
                )
            }
            KernelError::InvalidVersion(error) => write!(f, "Kernel VERSION is invalid: {error}"),
            KernelError::TemplateMissing { path } => {
                write!(f, "instance-template not found at {}", path.display())
            }
            KernelError::Walk(error) => write!(f, "{error}"),
            KernelError::Io { path, message } => {
                write!(f, "cannot write {}: {message}", path.display())
            }
        }
    }
}

impl std::error::Error for KernelError {}

/// Reads and trims `<kernel>/VERSION` — the sole source of the Kernel
/// edition every database's metadata is checked against
/// (`meridian-cli-rfc.md`, "Получение редакции Kernel из канонического
/// `VERSION`").
pub fn read_kernel_edition(kernel_root: &Path) -> Result<Revision, KernelError> {
    let path = kernel_root.join("VERSION");
    let raw = fs::read_to_string(&path).map_err(|error| KernelError::VersionUnreadable {
        path: path.clone(),
        message: error.to_string(),
    })?;
    Revision::new(raw.trim()).map_err(KernelError::InvalidVersion)
}

/// Why a Kernel root is not under Git — the `meridian doctor` form of the
/// `preflight` "kernel is under Git" check (`meridian-cli-rfc.md`, command
/// `doctor`). Read-only: nothing is created, and no `git` process runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelGitError {
    Missing { path: PathBuf },
    Unreadable { path: PathBuf, message: String },
}

impl std::fmt::Display for KernelGitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KernelGitError::Missing { path } => {
                write!(f, "Kernel is not under Git: no {} entry", path.display())
            }
            KernelGitError::Unreadable { path, message } => write!(
                f,
                "Kernel Git state is unknown: cannot inspect {}: {message}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for KernelGitError {}

/// The Kernel root is under Git when it carries a `.git` entry — a
/// directory in a primary checkout, a `gitdir:` file in a linked worktree —
/// exactly the `preflight` criterion (`fs.existsSync(path.join(KERNEL,
/// '.git'))`) and `validate`'s own `git-provenance`. An entry that cannot
/// be inspected is not confirmed either.
pub fn check_kernel_under_git(kernel_root: &Path) -> Result<(), KernelGitError> {
    let path = kernel_root.join(".git");
    match fs::metadata(&path) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Err(KernelGitError::Missing { path })
        }
        Err(error) => Err(KernelGitError::Unreadable {
            path,
            message: error.to_string(),
        }),
    }
}

/// One entry found while walking a directory tree: its full path and
/// whether it is a directory. Fail-closed: produced only by
/// [`walk_all_entries`], which stops and returns [`WalkError`] rather than
/// omitting an entry it could not fully inspect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalkedEntry {
    pub path: PathBuf,
    pub is_dir: bool,
}

/// Recursively lists every entry under `root`, skipping any directory whose
/// bare name is in `skip_dir_names`, in a stable (lexicographic per
/// directory, depth-first) order. Never silently drops an entry: a
/// `read_dir`, `DirEntry::file_type` or entry-iteration failure aborts the
/// whole walk and returns the specific [`WalkError`] instead of the entries
/// collected so far.
pub fn walk_all_entries(
    root: &Path,
    skip_dir_names: &[&str],
) -> Result<Vec<WalkedEntry>, WalkError> {
    fn walk(
        dir: &Path,
        skip_dir_names: &[&str],
        out: &mut Vec<WalkedEntry>,
    ) -> Result<(), WalkError> {
        let read = fs::read_dir(dir).map_err(|error| WalkError::ReadDir {
            path: dir.to_path_buf(),
            message: error.to_string(),
        })?;
        let mut entries = Vec::new();
        for entry in read {
            let entry = entry.map_err(|error| WalkError::DirEntry {
                path: dir.to_path_buf(),
                message: error.to_string(),
            })?;
            entries.push(entry);
        }
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let file_type = entry.file_type().map_err(|error| WalkError::FileType {
                path: path.clone(),
                message: error.to_string(),
            })?;
            let is_dir = file_type.is_dir();
            if is_dir {
                let name = entry.file_name();
                if skip_dir_names.contains(&name.to_string_lossy().as_ref()) {
                    continue;
                }
                out.push(WalkedEntry {
                    path: path.clone(),
                    is_dir: true,
                });
                walk(&path, skip_dir_names, out)?;
            } else {
                out.push(WalkedEntry {
                    path,
                    is_dir: false,
                });
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(root, skip_dir_names, &mut out)?;
    Ok(out)
}

/// Every regular file under `root`, skipping [`SKIPPED_DIR_NAMES`], fail-closed
/// (see [`walk_all_entries`]).
pub fn walk_all_files(root: &Path) -> Result<Vec<PathBuf>, WalkError> {
    Ok(walk_all_entries(root, SKIPPED_DIR_NAMES)?
        .into_iter()
        .filter(|entry| !entry.is_dir)
        .map(|entry| entry.path)
        .collect())
}

/// Every regular file under `root` whose extension is one of `extensions`,
/// skipping [`SKIPPED_DIR_NAMES`], fail-closed (see [`walk_all_entries`]).
pub fn walk_files_with_extensions(
    root: &Path,
    extensions: &[&str],
) -> Result<Vec<PathBuf>, WalkError> {
    Ok(walk_all_files(root)?
        .into_iter()
        .filter(|path| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| {
                    extensions
                        .iter()
                        .any(|candidate| candidate.eq_ignore_ascii_case(ext))
                })
        })
        .collect())
}

/// Directory names never descended into while walking a Kernel checkout —
/// build output and version-control metadata, neither of which is normative
/// Kernel content. `.github` is skipped alongside them: CI workflow YAML is
/// infrastructure configuration, not normative Kernel content subject to the
/// strict YAML subset (`standards/workspace/kernel-boundary.md` lists
/// `.github/` only as a release-unit artifact — GitHub Actions YAML
/// legitimately uses anchors/aliases the strict subset does not accept).
pub const SKIPPED_DIR_NAMES: &[&str] = &[".git", "target", "node_modules", ".github", "_to_delete"];

/// One file copied, or left alone, by [`copy_instance_template`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateCopyOutcome {
    /// Relative to both the template root and the destination workspace
    /// root, using `/` as the separator regardless of host platform.
    pub relative_path: String,
    pub copied: bool,
}

/// The full effect of one [`copy_instance_template`] call: which files it
/// copied or left alone, and exactly which destination directories did not
/// exist before the call and were created to hold a copied file. A caller
/// that must roll back a failed `init` ([`crate::commands::init`]) removes
/// only `created_dirs` (each, after its files are gone, if and only if it is
/// now empty) — a directory that already existed before this call, even
/// empty, is never in this list and is never touched by a rollback.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TemplateCopyResult {
    pub outcomes: Vec<TemplateCopyOutcome>,
    pub created_dirs: Vec<PathBuf>,
}

/// Creates `dir` and every missing ancestor of it, recording each one this
/// call actually created (shallowest first) into `created_dirs`. An ancestor
/// that already existed is never recorded and never touched. Exposed so
/// [`crate::commands::init`] can use the exact same tracked-creation
/// discipline for the workspace root itself, not only for template files.
pub fn ensure_dir_tracked(dir: &Path, created_dirs: &mut Vec<PathBuf>) -> Result<(), KernelError> {
    let mut missing = Vec::new();
    let mut current = dir;
    loop {
        if current.exists() {
            break;
        }
        missing.push(current.to_path_buf());
        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }
    for path in missing.into_iter().rev() {
        fs::create_dir(&path).map_err(|error| KernelError::Io {
            path: path.clone(),
            message: error.to_string(),
        })?;
        created_dirs.push(path);
    }
    Ok(())
}

/// Creates `path` atomically, never following or clobbering anything already
/// there (`O_CREAT|O_EXCL` via [`fs::OpenOptions::create_new`]): if a file,
/// directory, or symlink (dangling or not) already occupies `path`, this
/// fails with [`io::ErrorKind::AlreadyExists`] without touching it — the same
/// outcome whether the collision was there before this process started or
/// appeared in a race right before this call. There is no separate
/// existence check to race against: creation and the exclusivity check are
/// one syscall.
fn create_new_file(path: &Path) -> io::Result<fs::File> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
}

/// Copies `<kernel>/instance-template/` into `workspace_root`, file by file,
/// fail-closed: the whole source tree is enumerated first
/// ([`walk_all_entries`], no `skip_dir_names` — a template must be copied
/// whole), and any read failure aborts before a single file is copied. A
/// destination that already exists — file, directory, or symlink — is left
/// completely untouched: `create_new_file` never opens through it, so this
/// is what makes a repeat `init` never silently overwrite a user's own edits
/// (`meridian-cli-rfc.md`, command `init`).
///
/// Each destination is registered in the returned [`TemplateCopyResult`] as
/// `copied: true` **the moment `create_new_file` succeeds**, before a
/// single byte of content is written — so a write failure partway through
/// one file still leaves the caller's rollback able to find and remove that
/// exact partial file, never just the files that copied all the way through.
/// This function also removes that partial file itself on a write failure,
/// as a second, independent line of defense against leaving partial content
/// on disk (`meridian-cli/tests/binary_runs.rs`,
/// `init_rolls_back_a_partial_file_left_by_a_write_failure_mid_copy`).
///
/// On success, `Ok` carries the full [`TemplateCopyResult`]. On a copy
/// failure partway through, `Err` carries **both** the [`TemplateCopyResult`]
/// already accumulated (so the caller, `crate::commands::init`, can roll it
/// back with [`rollback_template_copy`]) **and** the [`KernelError`] that
/// stopped the copy — never just the error alone, which would leave the
/// caller unable to tell what this call had already done before it failed.
pub fn copy_instance_template(
    kernel_root: &Path,
    workspace_root: &Path,
) -> Result<TemplateCopyResult, (TemplateCopyResult, KernelError)> {
    let template_root = kernel_root.join("instance-template");
    if !template_root.is_dir() {
        return Err((
            TemplateCopyResult::default(),
            KernelError::TemplateMissing {
                path: template_root,
            },
        ));
    }
    let mut files: Vec<PathBuf> = match walk_all_entries(&template_root, &[]) {
        Ok(entries) => entries
            .into_iter()
            .filter(|entry| !entry.is_dir)
            .map(|entry| entry.path)
            .collect(),
        Err(error) => return Err((TemplateCopyResult::default(), KernelError::Walk(error))),
    };
    files.sort();

    let mut result = TemplateCopyResult::default();
    for source in files {
        let relative = source
            .strip_prefix(&template_root)
            .expect("source was built under template_root")
            .to_path_buf();
        let relative_display = relative
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        let destination = workspace_root.join(&relative);

        if let Some(parent) = destination.parent() {
            if let Err(error) = ensure_dir_tracked(parent, &mut result.created_dirs) {
                return Err((result, error));
            }
        }

        match create_new_file(&destination) {
            Ok(mut dest_file) => {
                // Recorded before the copy body runs: from this point on,
                // `destination` is a path this specific call created, and
                // both this function's own cleanup below and the caller's
                // rollback are entitled to remove it on failure.
                result.outcomes.push(TemplateCopyOutcome {
                    relative_path: relative_display,
                    copied: true,
                });
                let copy_result = fs::File::open(&source)
                    .and_then(|mut src_file| io::copy(&mut src_file, &mut dest_file));
                if let Err(error) = copy_result {
                    drop(dest_file);
                    let _ = fs::remove_file(&destination);
                    return Err((
                        result,
                        KernelError::Io {
                            path: destination,
                            message: error.to_string(),
                        },
                    ));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                result.outcomes.push(TemplateCopyOutcome {
                    relative_path: relative_display,
                    copied: false,
                });
            }
            Err(error) => {
                return Err((
                    result,
                    KernelError::Io {
                        path: destination,
                        message: error.to_string(),
                    },
                ));
            }
        }
    }
    Ok(result)
}

/// Removes a file this call itself created. `NotFound` is not reported: it
/// means the file is already gone, either because
/// [`copy_instance_template`] already cleaned up its own partial write or
/// because this is a second rollback pass — both fine outcomes, not cleanup
/// failures. Any other error — most commonly a permission problem — is
/// pushed onto `diagnostics` rather than swallowed, so a caller can surface
/// it instead of silently claiming a clean rollback that did not fully
/// happen.
fn remove_file_reporting(path: &Path, diagnostics: &mut Vec<String>) {
    if let Err(error) = fs::remove_file(path) {
        if error.kind() != io::ErrorKind::NotFound {
            diagnostics.push(format!(
                "rollback: cannot remove file {}: {error}",
                path.display()
            ));
        }
    }
}

/// Removes `dir` only if this call finds it empty, and only reports a
/// diagnostic for an actual failure to inspect or remove it — finding it
/// non-empty is the expected, silent outcome documented on
/// [`rollback_template_copy`] (something this call did not create still
/// lives there), not a cleanup error.
fn remove_dir_if_empty_reporting(dir: &Path, diagnostics: &mut Vec<String>) {
    match fs::read_dir(dir) {
        Ok(mut entries) => {
            if entries.next().is_none() {
                if let Err(error) = fs::remove_dir(dir) {
                    diagnostics.push(format!(
                        "rollback: cannot remove directory {}: {error}",
                        dir.display()
                    ));
                }
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => diagnostics.push(format!(
            "rollback: cannot inspect directory {}: {error}",
            dir.display()
        )),
    }
}

/// Removes exactly the effect of one [`copy_instance_template`] call:
/// files it reports as `copied: true`, then the directories it reports in
/// `created_dirs` (deepest first, only if now empty — a directory this call
/// did not create, or one that still holds something else, is left alone).
/// Used by `init`'s fail-clean rollback ([`crate::commands::init`]) when a
/// later step of the same invocation fails; never called for a directory or
/// file this call did not itself create. Any cleanup failure is appended to
/// `diagnostics` instead of being discarded, so the caller can report it
/// rather than claim a silently complete rollback.
pub fn rollback_template_copy(
    result: &TemplateCopyResult,
    workspace_root: &Path,
    diagnostics: &mut Vec<String>,
) {
    for outcome in &result.outcomes {
        if outcome.copied {
            remove_file_reporting(&workspace_root.join(&outcome.relative_path), diagnostics);
        }
    }
    let mut dirs = result.created_dirs.clone();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    for dir in dirs {
        remove_dir_if_empty_reporting(&dir, diagnostics);
    }
}

/// Lists every file `git` considers tracked under `root`, or `None` when
/// `git` is unavailable or `root` is not inside a Git working tree — the
/// caller then falls back to [`walk_all_files`] and reports that fallback
/// explicitly, the same discipline `scripts/kernel-validate.mjs`'s own
/// `gitTrackedFiles` applies (`meridian-cli/src/commands/validate/kernel_purity.rs`).
/// Spawns `git -C <root> ls-files -z` directly (`std::process::Command`,
/// never a shell string) — a local, no-network Git plumbing command, within
/// `meridian-cli`'s own remit as the crate that owns Git adapters.
pub fn list_git_tracked_files(root: &Path) -> Option<Vec<PathBuf>> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let files: Vec<PathBuf> = text
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(|s| root.join(s))
        .collect();
    if files.is_empty() {
        None
    } else {
        Some(files)
    }
}

/// Lists every file `git` considers untracked-but-not-ignored under `root`
/// (`git ls-files --others --exclude-standard -z`) — the complement
/// [`list_git_tracked_files`] does not cover, named explicitly so a later
/// "N tracked files clean" claim never silently reads as "the whole tree".
/// `None` on any failure to run `git` — callers only call this after
/// [`list_git_tracked_files`] already succeeded, so a failure here is
/// reported as "nothing to warn about" rather than aborting the caller.
pub fn list_git_untracked_files(root: &Path) -> Option<Vec<PathBuf>> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "--others", "--exclude-standard", "-z"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    Some(
        text.split('\0')
            .filter(|s| !s.is_empty())
            .map(|s| root.join(s))
            .collect(),
    )
}
