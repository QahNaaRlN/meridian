//! Typed run contracts — the domain of the three last historical-7b
//! families, `execution-state-model`, `role-and-human-control` and
//! `bounded-context-manifest`
//! (`rust-architecture-conformance-5`,
//! `governance/plans/meridian-rust-migration-program-plan.md` §5.19).
//!
//! # Architecture
//!
//! ```text
//! FsWorkspaceReader (CLI adapter)
//!   -> three app operations over one WorkspaceReader (meridian-app)
//!   -> JSON Schema + private closed DTOs + fixture resolution transport (meridian-app)
//!   -> typed inputs (this module)
//!   -> pure constructors and checks (this module)
//!   -> Option<accepted value> + Vec<Diagnostic>
//!   -> one CLI prefix/presentation path per family
//! ```
//!
//! Responsibilities, one module each:
//!
//! - [`vocabulary`] — the closed pools: lifecycle stage, work status,
//!   universal role, supervision and communication mode;
//! - [`identity`] — run identity, scope revision, transition sequence,
//!   actor/reference text, the `run-state` scope;
//! - [`envelope`] — the one shared scoped-record envelope check;
//! - [`execution_state`] — one run's state and transition history;
//! - [`roles`] — the built-in universal-role catalogue;
//! - [`human_control`] — role assignments, supervision and the switch
//!   history of one run;
//! - [`revision`] — revision classification and the pin rule;
//! - [`pinned_ref`] — pinned-record kinds and slots;
//! - [`resolution`] — the typed resolution catalogue and the resolved
//!   checkpoint vocabulary;
//! - [`context_manifest`] — the bounded context manifest.
//!
//! Every public `check_*` returns its accepted type only when it found
//! zero problems. Diagnostics are prefix-free; the family prefix is
//! presentation, added once by the CLI. No serde, `serde_json::Value`,
//! JSON Schema, file, process, env or output here.

pub mod context_manifest;
pub mod envelope;
pub mod execution_state;
pub mod human_control;
pub mod identity;
pub mod pinned_ref;
pub mod resolution;
pub mod revision;
pub mod roles;
pub mod vocabulary;

use crate::types::{Diagnostic, DiagnosticLevel};

pub use context_manifest::{
    check_context_manifest, ApplicableNorm, AuthoritativeSource, ContextManifest,
    ContextManifestInput, IdentifiedItem, ManifestBody, PinnedRefs, RunStateCheckpoint,
};
pub use envelope::{
    RecordAuthority, RecordEnvelope, RecordFamily, RecordOrigin, SourcedOriginKind,
};
pub use execution_state::{
    check_execution_run, Blocker, ExecutionRun, ExecutionRunInput, ExecutionRunState, StageSkip,
    Transition,
};
pub use human_control::{
    check_human_control, ControlSwitch, HumanControl, HumanControlInput, HumanControlState,
    ReviewIndependence, RoleAssignment, RoleAssignmentError, Supervision,
};
pub use identity::{
    ActorRef, PortableRef, RecordText, RunId, RunStateScope, RunValueError, ScopeRevision,
    TransitionSequence,
};
pub use pinned_ref::{PinnedRecordKind, PinnedRef, PinnedSlot};
pub use resolution::{
    ForeignValue, ResolutionCatalogue, ResolvedEntry, ResolvedStateField, ResolvedStateResponse,
    ResponseField, ResponseItem, ResponseValueKind, RESOLVED_ENTRY_COMMON_KEYS,
};
pub use revision::{PinSha256, PinSha256Error, RevisionClass};
pub use roles::{check_role_registry, RoleCatalogue, RoleDefinition, RoleRegistryInput};
pub use vocabulary::{
    CommunicationMode, LifecycleStage, SupervisionMode, UniversalRole, WorkStatus,
    HUMAN_AUTHORITY_POSTURE,
};

/// A run's next action or next gate: an executable value, or `None` for the
/// schema's explicit `null` (no continuation).
pub type NextStep = Option<RecordText>;

/// Every message this module builds starts with a fixed, non-empty record
/// label (`execution run "…"`, `context manifest "…"`, …), so the
/// non-blank [`Diagnostic`] invariant holds by construction — an internal
/// invariant, not a property of external data.
fn fail(message: String) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message)
        .expect("every run-contract diagnostic starts with a non-empty record label")
}

/// Russian text for the human reader: at least one Cyrillic letter.
fn has_cyrillic(s: &str) -> bool {
    s.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c))
}

/// `JSON.stringify` of a string, as the diagnostics quote it.
fn json_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A nullable axis as the Node reference interpolates it: the value, or
/// `null`.
fn option_text<T: core::fmt::Display>(value: Option<T>) -> String {
    value.map_or_else(|| "null".to_string(), |v| v.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_quote_escapes_like_json_stringify() {
        assert_eq!(json_quote("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(json_quote("line\nnext\u{1}"), "\"line\\nnext\\u0001\"");
        assert_eq!(json_quote("кириллица"), "\"кириллица\"");
    }

    #[test]
    fn option_text_renders_null_for_none() {
        assert_eq!(option_text(None::<&str>), "null");
        assert_eq!(option_text(Some(LifecycleStage::Intake)), "intake");
    }
}
