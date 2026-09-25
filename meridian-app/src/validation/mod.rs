//! Orchestration for `meridian validate` checks that read the Kernel
//! workspace through the [`crate::workspace::WorkspaceReader`] port rather
//! than composing already-parsed in-memory values (contrast
//! [`crate::operating_model`], whose composite algorithms take a `Value`
//! document and never touch a port).

pub mod mechanical_integrity;
