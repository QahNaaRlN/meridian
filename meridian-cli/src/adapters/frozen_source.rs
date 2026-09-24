//! The single production [`FrozenSource`]: one Git repository named by an
//! explicit `--source` path, read ONLY from Git objects
//! (`meridian-rust-migration-program-plan.md` §5.22.4, item 7).
//!
//! The bundle commit is resolved once (`HEAD` at open time) and every
//! bundle file is read from that commit; the pinned source revision is
//! listed once (`git ls-tree -r`) and every file is read by the blob id of
//! that same listing, so no working-tree file, second revision or third
//! state can enter an operation. The tree digest is recomputed here over
//! the same listing: every `<mode> <type> <object id> <path>` line, sorted,
//! newline-joined with a trailing newline, SHA-256 — the algorithm the
//! accepted `source-snapshot.json` declares. Every `git` call passes its
//! arguments separately (`std::process::Command`, never a shell string) and
//! pins `core.quotePath` so the listing does not depend on user config.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use meridian_app::migration::{BundleFile, FrozenSource, PinnedTree, SourceError};
use meridian_core::types::ContentDigest;

/// One listed blob of the pinned tree.
#[derive(Debug, Clone)]
struct Blob {
    object: String,
}

#[derive(Debug, Clone)]
struct Listing {
    tree: PinnedTree,
    blobs: BTreeMap<String, Blob>,
}

pub struct GitFrozenSource {
    root: PathBuf,
    bundle_commit: String,
    listings: RefCell<BTreeMap<String, Listing>>,
}

fn git(root: &Path, args: &[&str]) -> Result<Output, SourceError> {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["-c", "core.quotePath=true"])
        .args(args)
        .output()
        .map_err(|e| SourceError::GitUnavailable(e.to_string()))
}

fn stdout_text(output: Output, what: &str) -> Result<String, SourceError> {
    String::from_utf8(output.stdout)
        .map_err(|_| SourceError::Unreadable(format!("{what} is not UTF-8")))
}

/// A full commit id: 40 (SHA-1) or 64 (SHA-256) lowercase hex digits. Any
/// other text is never handed to `git` as a revision.
fn is_commit_id(revision: &str) -> bool {
    matches!(revision.len(), 40 | 64)
        && revision
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

impl GitFrozenSource {
    /// Opens the repository at `root` and pins its current commit as the
    /// bundle commit.
    pub fn open(root: &Path) -> Result<GitFrozenSource, SourceError> {
        if !root.is_dir() {
            return Err(SourceError::NotARepository(root.display().to_string()));
        }
        let probe = git(root, &["rev-parse", "--git-dir"])?;
        if !probe.status.success() {
            return Err(SourceError::NotARepository(root.display().to_string()));
        }
        let head = git(root, &["rev-parse", "--verify", "--quiet", "HEAD^{commit}"])?;
        if !head.status.success() {
            return Err(SourceError::MissingRevision {
                revision: "HEAD".to_string(),
            });
        }
        let commit = stdout_text(head, "HEAD")?.trim().to_string();
        if !is_commit_id(&commit) {
            return Err(SourceError::Unreadable(format!(
                "HEAD resolved to \"{commit}\", not a commit id"
            )));
        }
        Ok(GitFrozenSource {
            root: root.to_path_buf(),
            bundle_commit: commit,
            listings: RefCell::new(BTreeMap::new()),
        })
    }

    fn blob_bytes(&self, object: &str) -> Result<Vec<u8>, SourceError> {
        let output = git(&self.root, &["cat-file", "blob", object])?;
        if !output.status.success() {
            return Err(SourceError::Unreadable(format!(
                "blob {object} cannot be read"
            )));
        }
        Ok(output.stdout)
    }

    fn listing(&self, revision: &str) -> Result<Listing, SourceError> {
        if let Some(listing) = self.listings.borrow().get(revision) {
            return Ok(listing.clone());
        }
        if !is_commit_id(revision) {
            return Err(SourceError::MissingRevision {
                revision: revision.to_string(),
            });
        }
        let exists = git(
            &self.root,
            &["cat-file", "-e", &format!("{revision}^{{commit}}")],
        )?;
        if !exists.status.success() {
            return Err(SourceError::MissingRevision {
                revision: revision.to_string(),
            });
        }
        let output = git(
            &self.root,
            &[
                "ls-tree",
                "-r",
                "--format=%(objectmode) %(objecttype) %(objectname) %(path)",
                revision,
            ],
        )?;
        if !output.status.success() {
            return Err(SourceError::Unreadable(format!(
                "git ls-tree of {revision} failed"
            )));
        }
        let text = stdout_text(output, "the tree listing")?;
        let mut lines: Vec<&str> = text.trim().split('\n').filter(|l| !l.is_empty()).collect();
        let mut blobs = BTreeMap::new();
        for line in &lines {
            let mut parts = line.splitn(4, ' ');
            let (Some(_mode), Some(kind), Some(object), Some(path)) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                return Err(SourceError::Unreadable(format!(
                    "unexpected tree listing line \"{line}\""
                )));
            };
            if path.starts_with('"') {
                return Err(SourceError::Unreadable(format!(
                    "tracked path {path} needs quoting; such paths are not supported"
                )));
            }
            if kind == "blob" {
                blobs.insert(
                    path.to_string(),
                    Blob {
                        object: object.to_string(),
                    },
                );
            }
        }
        lines.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        let mut manifest = lines.join("\n");
        manifest.push('\n');
        let listing = Listing {
            tree: PinnedTree {
                digest: ContentDigest::of_str(&manifest),
                paths: blobs.keys().cloned().collect(),
            },
            blobs,
        };
        self.listings
            .borrow_mut()
            .insert(revision.to_string(), listing.clone());
        Ok(listing)
    }
}

impl FrozenSource for GitFrozenSource {
    fn bundle_revision(&self) -> &str {
        &self.bundle_commit
    }

    fn bundle_file(&self, file: BundleFile) -> Result<Vec<u8>, SourceError> {
        let listing = self.listing(&self.bundle_commit.clone())?;
        let blob = listing
            .blobs
            .get(file.path())
            .ok_or_else(|| SourceError::MissingFile {
                revision: self.bundle_commit.clone(),
                path: file.path().to_string(),
            })?;
        self.blob_bytes(&blob.object)
    }

    fn pinned_tree(&self, revision: &str) -> Result<PinnedTree, SourceError> {
        Ok(self.listing(revision)?.tree)
    }

    fn file_at(&self, revision: &str, path: &str) -> Result<Vec<u8>, SourceError> {
        let listing = self.listing(revision)?;
        let blob = listing
            .blobs
            .get(path)
            .ok_or_else(|| SourceError::MissingFile {
                revision: revision.to_string(),
                path: path.to_string(),
            })?;
        self.blob_bytes(&blob.object)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_source_only_full_commit_ids_reach_git() {
        assert!(is_commit_id(&"a".repeat(40)));
        assert!(is_commit_id(&"0".repeat(64)));
        assert!(!is_commit_id("HEAD"));
        assert!(!is_commit_id("--output=/tmp/x"));
        assert!(!is_commit_id(&"A".repeat(40)));
    }

    #[test]
    fn migration_source_a_plain_directory_is_not_a_repository() {
        let dir = std::env::temp_dir().join(format!(
            "meridian-frozen-source-not-a-repo-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        struct Guard(PathBuf);
        impl Drop for Guard {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let guard = Guard(dir);
        assert!(matches!(
            GitFrozenSource::open(&guard.0),
            Err(SourceError::NotARepository(_))
        ));
    }
}
