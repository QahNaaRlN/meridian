//! Evidence structures and the rules linking a claimed result to a
//! verifiable, checkable result (`standards/workspace/evidence-and-handoff-contract.md`
//! §5, scoped to the pure claim/assertion/evidence linkage package
//! `rust-domain-core` needs — not the full handoff record, which belongs to
//! a later package once `meridian-app` has the ports this crate does not
//! know about).

mod aggregate;
mod types;

pub use aggregate::{
    claimed_result_status, evaluate, verify_assertion, AssertionVerdict, ClaimedResultStatus,
    EvidenceEvaluation,
};
pub use types::{
    ClaimedResult, EvidenceEntry, ObservedResult, Pin, ResolvedEvidenceResult, VerifiableAssertion,
};
