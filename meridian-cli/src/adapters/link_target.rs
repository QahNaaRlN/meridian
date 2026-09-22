//! The single production `LinkTargetPort`
//! (`meridian_app::workspace::LinkTargetPort`): resolves a canonical link's
//! `path` against the real filesystem — `std::fs::canonicalize`/
//! `std::fs::metadata` for this port live only here.

use std::path::{Path, PathBuf};

use meridian_app::workspace::{LinkTargetError, LinkTargetPort};
use meridian_core::types::WorkspaceRelativePath;

pub struct FsLinkTarget {
    root: PathBuf,
}

impl FsLinkTarget {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl LinkTargetPort for FsLinkTarget {
    fn check(&self, path: &WorkspaceRelativePath) -> Result<(), LinkTargetError> {
        check_link_target(&self.root, path)
    }
}

fn check_link_target(root: &Path, path: &WorkspaceRelativePath) -> Result<(), LinkTargetError> {
    let root = root
        .canonicalize()
        .map_err(|error| LinkTargetError::Io(error.to_string()))?;
    let lexical = root.join(path.as_str());
    let real_target = match std::fs::canonicalize(&lexical) {
        Ok(target) => target,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(LinkTargetError::Missing)
        }
        Err(error) => return Err(LinkTargetError::Io(error.to_string())),
    };
    if !real_target.starts_with(&root) {
        return Err(LinkTargetError::SymlinkEscape);
    }
    let metadata =
        std::fs::metadata(&real_target).map_err(|error| LinkTargetError::Io(error.to_string()))?;
    if !metadata.is_file() {
        return Err(LinkTargetError::NotRegularFile);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "fs-link-target-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn accepts_a_real_regular_file() {
        let dir = TempDir::new("ok");
        std::fs::write(dir.0.join("a.md"), "x").unwrap();
        let path = WorkspaceRelativePath::new("a.md").unwrap();
        assert!(check_link_target(&dir.0, &path).is_ok());
    }

    #[test]
    fn reports_a_missing_target() {
        let dir = TempDir::new("missing");
        let path = WorkspaceRelativePath::new("nope.md").unwrap();
        assert_eq!(
            check_link_target(&dir.0, &path).unwrap_err(),
            LinkTargetError::Missing
        );
    }

    #[test]
    fn reports_a_directory_target_as_not_a_regular_file() {
        let dir = TempDir::new("dir");
        std::fs::create_dir_all(dir.0.join("sub")).unwrap();
        let path = WorkspaceRelativePath::new("sub").unwrap();
        assert_eq!(
            check_link_target(&dir.0, &path).unwrap_err(),
            LinkTargetError::NotRegularFile
        );
    }

    #[test]
    #[cfg(unix)]
    fn reports_a_symlink_escape() {
        let dir = TempDir::new("escape-inside");
        let outside = TempDir::new("escape-outside");
        std::fs::write(outside.0.join("secret.md"), "x").unwrap();
        std::os::unix::fs::symlink(outside.0.join("secret.md"), dir.0.join("link.md")).unwrap();
        let path = WorkspaceRelativePath::new("link.md").unwrap();
        assert_eq!(
            check_link_target(&dir.0, &path).unwrap_err(),
            LinkTargetError::SymlinkEscape
        );
    }
}
