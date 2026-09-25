//! Pure domain of `rust-workspace-state-validation`
//! (`meridian-rust-migration-program-plan.md` §5.23.11): the strict types and
//! checks that judge the imported product state of a workspace database.
//!
//! Every value here is already parsed and typed: this module knows no JSON,
//! YAML, SQLite, path of a database, Git, file, environment, clock or CLI.
//! `meridian-app` owns the transport/schema/domain boundary that builds these
//! values from stored records and the ports that observe repositories, and
//! `meridian-cli` owns presentation.
//!
//! One submodule per contract of the GAP-09 mapping (qualification report
//! §9.3): [`product`] (M-02/M-03), [`record_types`] (M-05b),
//! [`dependencies`] (M-07), [`inventory`] (M-08), [`profile`] (M-09),
//! [`intake`] (M-12/M-15 and topic existence), [`packaging`] and
//! [`parentage`] (the two further intake checks of decision D3) and
//! [`observation`] (M-18); [`timestamp`] is the shared instant type the TTL
//! check and the observation use.

pub mod dependencies;
pub mod intake;
pub mod inventory;
pub mod observation;
pub mod packaging;
pub mod parentage;
pub mod product;
pub mod profile;
pub mod record_types;
pub mod timestamp;

use crate::types::{Diagnostic, DiagnosticLevel};

/// Every message this module builds starts with a fixed, non-empty family
/// prefix, so construction cannot fail.
pub(crate) fn diagnostic(level: DiagnosticLevel, message: String) -> Diagnostic {
    Diagnostic::new(level, message).expect("message starts with a non-empty family prefix")
}
