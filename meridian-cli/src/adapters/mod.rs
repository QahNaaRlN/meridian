//! Concrete adapters that bind `meridian-app`-owned ports to the real
//! world. `meridian-cli` is the only crate that knows these implementations
//! exist (`meridian-cli-rfc.md`, `AGENTS.md` composition-root remit).

pub mod frozen_source;
pub mod git_inspector;
pub mod link_target;
pub mod workspace_reader;
