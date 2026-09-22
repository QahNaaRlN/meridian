//! Domain types and pure checks for `task-pattern-registry` and
//! `task-specification-contract`, moved into `meridian-core` by
//! `rust-architecture-conformance-3`
//! (`governance/plans/meridian-rust-migration-program-plan.md` §5.17). Both
//! families reuse [`crate::resolver::{WorkKind, ChangeClass, WorkItemKind}`]
//! as their one classification pool and
//! [`crate::types::WorkspaceRelativePath`] as their one portable-path type
//! — neither family defines a second pool or a second path-portability
//! rule.
//!
//! [`catalog`] owns [`catalog::TaskPatternCatalog`], built once by the
//! `task-pattern-registry` route and consumed — never re-parsed — by the
//! `task-specification-contract` route ([`specification`]). Free-text
//! portability scanning and `$schema` logical-address resolution
//! ([`portability`]) are pure pattern matching (`fancy-regex`, no
//! filesystem/Git/process I/O) and live here too, so
//! [`specification::check_task_specification`] can gate its own
//! [`TaskSpecification`] construction on portability itself
//! (`rust-architecture-conformance-3` corrective round item 2).

pub mod catalog;
pub mod portability;
pub mod specification;

pub use catalog::{
    CanonicalLink, CanonicalLinkField, PatternRefError, TaskPatternCatalog, TaskPatternEntry,
    BUGFIX_SKILL, REFACTOR_EVIDENCE, REFACTOR_PROTOCOL, REQUIRED_KINDS,
};
pub use portability::{non_portable_reason, resolve_schema_ref, CANONICAL_RECORD_BASE};
pub use specification::{
    check_task_specification, spec_id_label, AcceptanceCriterionInput, DeclaredPattern,
    SpecificationFields, TaskSpecification, ALLOWED_SCOPE_TYPES, RECORD_TYPE, RUN_STATE_FIELDS,
};
