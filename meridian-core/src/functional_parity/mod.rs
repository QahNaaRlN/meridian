//! Domain types and pure checks for `functional-parity` evidence records
//! (package 7, subpackage 7b's last remaining family; typed by
//! `rust-architecture-conformance-4`,
//! `governance/plans/meridian-rust-migration-program-plan.md` §5.18). A
//! self-contained evidence contract: it reads only its own
//! `verification/functional-parity/` schema/fixtures and never consumes
//! `execution-state-model`, `role-and-human-control` or
//! `bounded-context-manifest`.
//!
//! # Architecture
//!
//! ```text
//! FsWorkspaceReader (CLI adapter)
//!   -> functional_parity app operation (meridian-app)
//!   -> JSON Schema + private fixture/record DTO boundary (meridian-app)
//!   -> typed functional-parity input (this module's [`construction::RecordInput`])
//!   -> cross-reference/inference checks ([`checks::check_document`])
//!   -> Option<[`construction::FunctionalParityEvidence`]> + Vec<Diagnostic>
//!   -> CLI family-prefix/presentation
//! ```
//!
//! [`types`] owns the closed enums and dotted-kebab id/text types the
//! schema's own vocabulary requires; [`construction`] owns the typed input
//! shape a caller assembles — every business-meaningful schema field, with
//! mutually exclusive states as enum forms carrying their own data, never a
//! flat marker plus a silently-dropped optional field — and the accepted
//! evidence output type; [`checks`] owns the eleven pure inference rules
//! and is the ONE path to a [`construction::FunctionalParityEvidence`]. No
//! filesystem, Git, process, env or `serde_json::Value` here — only
//! already-validated domain values in, [`crate::types::Diagnostic`]s and
//! typed domain values out.

pub mod checks;
pub mod construction;
pub mod types;

pub use checks::check_document;
pub use construction::{
    Assertion, AssertionVerdictInput, BaselineInput, Condition, ContractLinkInput,
    EvidenceEntryInput, FacetContent, FacetInput, FunctionalParityEvidence, GapInput,
    IdentifiedCondition, ObservedResult, PostChangeConditions, PostChangeInput, Provenance,
    RecordEvidence, RecordInput, VerdictInput, VerdictOutcome,
};
pub use types::{
    AssertionId, ConditionId, DottedKebabIdError, EvidenceKind, EvidenceText, EvidenceTextError,
    Facet, SourceStateRef,
};
