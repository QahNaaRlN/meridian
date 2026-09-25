//! `WorkspaceReader` — the app-owned port through which the five
//! `validate-mechanical-integrity` (7a) checks read the Kernel workspace
//! (`meridian-rust-migration-program-plan.md` §5.16). The only production
//! implementation binds one verified root to the real filesystem
//! (`meridian_cli::adapters::workspace_reader::FsWorkspaceReader`); a fake
//! implementation drives this crate's own orchestration tests
//! (`crate::validation::mechanical_integrity`) without touching disk.

mod git_inspector;
mod link_target;
mod reader;

pub use git_inspector::{GitInspector, GitInspectorError};
pub use link_target::{LinkTargetError, LinkTargetPort};
pub use reader::{DirEntry, EntryKind, ReadError, WorkspaceReader};
