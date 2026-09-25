//! The single production `WorkspaceReader`
//! (`meridian_app::workspace::WorkspaceReader`): binds one verified root —
//! the `--kernel` path, already checked `is_dir()` by
//! `commands::validate::run` — to the real filesystem. `std::fs` for the
//! five `validate-mechanical-integrity` (7a) families lives only here
//! (`meridian-rust-migration-program-plan.md` §5.16.3, point 3).

use std::fs;
use std::path::{Path, PathBuf};

use meridian_app::workspace::{DirEntry, EntryKind, ReadError, WorkspaceReader};
use meridian_core::types::{EntryName, WorkspaceRelativePath};

pub struct FsWorkspaceReader {
    root: PathBuf,
}

impl FsWorkspaceReader {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn resolve(&self, path: &WorkspaceRelativePath) -> PathBuf {
        self.root.join(path.as_str())
    }
}

fn map_io_error(error: std::io::Error) -> ReadError {
    match error.kind() {
        std::io::ErrorKind::NotFound => ReadError::NotFound,
        _ => ReadError::Io(error.to_string()),
    }
}

impl WorkspaceReader for FsWorkspaceReader {
    fn read_text(&self, path: &WorkspaceRelativePath) -> Result<String, ReadError> {
        fs::read_to_string(self.resolve(path)).map_err(map_io_error)
    }

    fn read_bytes(&self, path: &WorkspaceRelativePath) -> Result<Vec<u8>, ReadError> {
        fs::read(self.resolve(path)).map_err(map_io_error)
    }

    fn list_dir(&self, path: &WorkspaceRelativePath) -> Result<Vec<DirEntry>, ReadError> {
        let dir = self.resolve(path);
        let read_dir = fs::read_dir(&dir).map_err(map_io_error)?;
        let mut entries = Vec::new();
        for entry in read_dir {
            let entry = entry.map_err(map_io_error)?;
            // `file_type()` reports the entry's own type as the directory
            // lists it, without following a symlink — unlike
            // `entry.path().is_dir()`, which stats through the link.
            let file_type = entry.file_type().map_err(map_io_error)?;
            let kind = if file_type.is_dir() {
                EntryKind::Dir
            } else if file_type.is_file() {
                EntryKind::File
            } else {
                EntryKind::Other
            };
            // Checked `OsString -> String` conversion, never
            // `to_string_lossy()`: a non-UTF-8 entry name is a real
            // encoding failure of this listing, reported as `Io`, never
            // silently mangled into a "close enough" lossy string that a
            // downstream check (e.g. `sha-provenance`'s skill-name lookup)
            // could then fail to find, misreporting an encoding failure as
            // an ordinary "missing" diagnostic.
            let raw_name = entry
                .file_name()
                .into_string()
                .map_err(|os| ReadError::Io(format!("entry name is not valid UTF-8: {os:?}")))?;
            let name = EntryName::new(raw_name)
                .map_err(|error| ReadError::Io(format!("invalid directory entry name: {error}")))?;
            entries.push(DirEntry { name, kind });
        }
        Ok(entries)
    }
}

/// Builds the Kernel-relative [`WorkspaceRelativePath`] used to address a
/// real file already found under `kernel_root` by an existing (out-of-scope)
/// walk, such as `crate::kernel::list_git_tracked_files`. A path this
/// function receives is always a strict descendant of `kernel_root`,
/// stripped and rejoined with `/` by `relative_slash` — the walk itself
/// never emits `.`/`..` and a real filesystem entry name can never embed a
/// path separator, so construction here cannot fail for genuine walk
/// output; `Err` is still handled (never `.expect()`ed) rather than assumed
/// away, matching this package's production-panic-audit requirement.
pub fn workspace_relative(
    kernel_root: &Path,
    absolute: &Path,
) -> Result<WorkspaceRelativePath, meridian_core::types::WorkspaceRelativePathError> {
    WorkspaceRelativePath::new(relative_slash(kernel_root, absolute))
}

fn relative_slash(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "fs-workspace-reader-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write(dir: &Path, rel: &str, content: &str) {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn reads_text_and_bytes_of_a_real_file() {
        let temp = TempDir::new("read");
        write(&temp.0, "a/b.txt", "hello\n");
        let reader = FsWorkspaceReader::new(&temp.0);
        let path = WorkspaceRelativePath::new("a/b.txt").unwrap();
        assert_eq!(reader.read_text(&path).unwrap(), "hello\n");
        assert_eq!(reader.read_bytes(&path).unwrap(), b"hello\n");
    }

    #[test]
    fn a_missing_file_is_not_found_distinct_from_an_io_error() {
        let temp = TempDir::new("missing");
        let reader = FsWorkspaceReader::new(&temp.0);
        let path = WorkspaceRelativePath::new("nope.txt").unwrap();
        assert_eq!(reader.read_text(&path).unwrap_err(), ReadError::NotFound);
    }

    #[test]
    fn list_dir_reports_file_and_dir_kinds_without_recursing() {
        let temp = TempDir::new("list");
        write(&temp.0, "skills/demo/PIN.yaml", "x");
        write(&temp.0, "skills/loose.txt", "x");
        let reader = FsWorkspaceReader::new(&temp.0);
        let path = WorkspaceRelativePath::new("skills").unwrap();
        let mut entries = reader.list_dir(&path).unwrap();
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name.as_str(), "demo");
        assert_eq!(entries[0].kind, EntryKind::Dir);
        assert_eq!(entries[1].name.as_str(), "loose.txt");
        assert_eq!(entries[1].kind, EntryKind::File);
    }

    #[test]
    #[cfg(unix)]
    fn list_dir_never_follows_a_symlink_to_decide_its_kind() {
        use std::os::unix::fs::symlink;
        let temp = TempDir::new("symlink");
        write(&temp.0, "skills/real/SKILL.md", "x");
        symlink(temp.0.join("skills/real"), temp.0.join("skills/linked")).unwrap();
        let reader = FsWorkspaceReader::new(&temp.0);
        let path = WorkspaceRelativePath::new("skills").unwrap();
        let entries = reader.list_dir(&path).unwrap();
        let linked = entries
            .iter()
            .find(|e| e.name.as_str() == "linked")
            .unwrap();
        assert_eq!(linked.kind, EntryKind::Other);
    }

    /// Real-filesystem, genuinely non-UTF-8 entry name (corrective round,
    /// `meridian-cli-foundation-architecture-remediation`, item 3): built
    /// with `OsStr::from_bytes` on an invalid UTF-8 byte sequence — this
    /// must propagate as `ReadError::Io`, never a lossily-mangled
    /// [`meridian_core::types::EntryName`] that some other check could
    /// then silently fail to find.
    #[test]
    #[cfg(unix)]
    fn list_dir_rejects_a_non_utf8_entry_name_instead_of_lossily_converting_it() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        let temp = TempDir::new("non-utf8");
        std::fs::create_dir_all(temp.0.join("skills")).unwrap();
        let invalid_name = OsStr::from_bytes(&[0x66, 0x6f, 0x80, 0x6f]); // "fo<invalid>o"
        std::fs::write(temp.0.join("skills").join(invalid_name), "x").unwrap();
        let reader = FsWorkspaceReader::new(&temp.0);
        let path = WorkspaceRelativePath::new("skills").unwrap();
        let error = reader.list_dir(&path).unwrap_err();
        assert!(matches!(error, ReadError::Io(_)));
    }

    #[test]
    fn workspace_relative_builds_a_slash_joined_path_from_a_real_walk_result() {
        let temp = TempDir::new("relative");
        let absolute = temp.0.join("standards").join("workspace").join("doc.md");
        let rel = workspace_relative(&temp.0, &absolute).unwrap();
        assert_eq!(rel.as_str(), "standards/workspace/doc.md");
    }
}
