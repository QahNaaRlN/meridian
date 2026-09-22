//! The `SourceResolver` app-port
//! (`governance/specifications/meridian-rust-target-architecture.md` §3):
//! resolves a candidate's `payload.source_ref` against the external
//! instruction-source-registry boundary.
//!
//! `SourceResolver` is a trait, owned by `meridian-app` (third corrective
//! round, item 1). `meridian-app` itself contains ZERO
//! `impl SourceResolver for ...` blocks (item 2) — the only implementation
//! is `meridian-cli`'s `FixtureSourceResolver`. Composition that already
//! holds fully-typed resolved data in memory (item 3,
//! `operating_model::existing_project_compatibility_mode`'s own scan) does
//! not implement this trait at all — it uses
//! [`super::SourceResolution::Prefetched`] instead, a plain typed lookup
//! table the orchestration itself reads, never a hidden callback adapter.
//!
//! This module is the app-side half of the "raw response -> closed DTO ->
//! validated `ResolvedInstructionSource` -> pure core check" pipeline
//! (corrective round item 8): [`SourceResolver::resolve`] returns the
//! closed [`ResolvedInstructionSourceDto`] (produced by the adapter, which
//! owns the raw fetch); `validate_resolved_response` here turns that DTO
//! into a [`meridian_core::controlled_rule_intake::ResolvedInstructionSource`],
//! accumulating every shape defect rather than stopping at the first
//! (corrective round item 9); the actual pin-vs-resolved comparison is
//! [`meridian_core::controlled_rule_intake::checks::check_source_ref`], a
//! pure core function that never sees a `Value` or a DTO.
//!
//! `read_channel` is converted into the typed
//! [`meridian_core::instruction_source::ReadChannel`] through
//! [`meridian_core::instruction_source::ReadChannel::try_new`] — the ONE
//! canonical, shared, typed constructor, which itself enforces the
//! coherence rule (third corrective round, item 4: an incoherent
//! `ReadChannel` is unrepresentable). This function never reconstructs a
//! `serde_json::Value` from the DTO to call a Value-based checker;
//! `read_channel` is still checked-and-discarded (nothing downstream of
//! this function reads it again), so it is not part of the returned
//! [`ResolvedInstructionSource`].

use core::fmt;

use meridian_core::controlled_rule_intake::{
    Currency, PinnedSourceRef, RecordedState, ResolvedInstructionSource, SOURCE_RECORD_TYPE,
};
use meridian_core::instruction_source::{MeridianVisibility, ReadChannel, ReadChannelKind};
use meridian_core::types::{ContentDigest, Diagnostic, NonEmptyString, Revision, SemanticId};
use serde::Deserialize;

use super::{fail, try_field};

/// A value rejected while resolving a candidate's `source_ref`. Distinct
/// from a shape defect in the RESPONSE itself (reported as `Diagnostic`s by
/// `validate_resolved_response`): this error is about the RESOLUTION CALL
/// not producing a response to validate at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceResolverError {
    /// No source is registered under the pinned reference. Distinct from
    /// [`SourceResolverError::Failed`] (corrective round item 12): an
    /// unknown source and a resolver that could not complete its own
    /// lookup are different failures with different remediation — a wrong
    /// pin versus a broken adapter.
    NotFound,
    /// The resolver itself failed (a malformed response it could not even
    /// parse into the closed transport shape, an adapter-side I/O error in
    /// a future real adapter) — carries a human-readable reason.
    Failed(String),
}

impl fmt::Display for SourceResolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourceResolverError::NotFound => write!(
                f,
                "no registered instruction source resolves for this reference"
            ),
            SourceResolverError::Failed(reason) => write!(f, "the resolver failed: {reason}"),
        }
    }
}

/// Resolves a candidate's pinned `source_ref` against the external
/// instruction-source-registry boundary. Implemented by a concrete adapter
/// in `meridian-cli` — never by `meridian-app` itself (target architecture
/// §3.1; third corrective round items 1-2). Called exactly once per
/// candidate by [`super::evaluate_controlled_rule_intake`], outside any
/// check — a check only ever validates an already-obtained response, it
/// never triggers resolution itself.
pub trait SourceResolver {
    fn resolve(
        &self,
        pinned: &PinnedSourceRef,
    ) -> Result<ResolvedInstructionSourceDto, SourceResolverError>;
}

/// The closed transport shape of one resolved instruction-source entry —
/// the "closed transformer response" every resolver (fixture-backed today,
/// a real registry-backed one later) must produce.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedInstructionSourceDto {
    pub record_type: String,
    pub id: String,
    pub reference: String,
    pub recorded_state: RecordedStateDto,
    pub read_channel: ReadChannelDto,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordedStateDto {
    pub revision: String,
    pub digest: DigestDto,
    pub revision_verified: bool,
    pub currency: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DigestDto {
    pub algorithm: String,
    pub value: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadChannelDto {
    pub kind: String,
    pub meridian_visibility: String,
    pub agent_auto_read: bool,
}

impl ResolvedInstructionSourceDto {
    /// Builds this closed transport shape directly from an already-typed
    /// snapshot, with no `serde_json::Value` at any point
    /// (`rust-architecture-conformance-2`, §4). For composition that
    /// already holds a real domain
    /// [`meridian_core::instruction_source::RecordedState`]/[`ReadChannel`]
    /// in memory (`operating_model::existing_project_compatibility_mode`'s
    /// own discovered sources, converted through the REAL
    /// `instruction_source_registry` boundary) — never a synthetic `Value`
    /// assembled just to round-trip back into this type, and never a
    /// hidden resolver callback standing in for it.
    ///
    /// This does not duplicate or bypass `validate_resolved_response`: it
    /// goes the OTHER direction (typed domain data -> this DTO), and the
    /// caller still has to pass the result through
    /// `evaluate_controlled_rule_intake`'s ordinary resolution path, which
    /// calls `validate_resolved_response` on it exactly as it would on any
    /// other resolver's response. `RecordedStateDto`/`DigestDto`/
    /// `ReadChannelDto` are private outside this module (transport shapes,
    /// not the ones `rust-migration-quality.md` wants named at a boundary
    /// a caller composes against), so an inherent method on the one
    /// already-public type is this module's own way of offering typed
    /// construction without widening that surface.
    pub fn from_typed(
        id: &meridian_core::types::SemanticId,
        reference: &str,
        recorded_state: &RecordedState,
        read_channel: &ReadChannel,
    ) -> Self {
        Self {
            record_type: SOURCE_RECORD_TYPE.to_string(),
            id: id.as_str().to_string(),
            reference: reference.to_string(),
            recorded_state: RecordedStateDto {
                revision: recorded_state.revision().as_str().to_string(),
                digest: DigestDto {
                    algorithm: recorded_state.digest().algorithm().to_string(),
                    value: recorded_state.digest().value().to_string(),
                },
                revision_verified: recorded_state.revision_verified(),
                currency: recorded_state.currency().as_str().to_string(),
            },
            read_channel: ReadChannelDto {
                kind: read_channel.kind().as_str().to_string(),
                meridian_visibility: read_channel.meridian_visibility().as_str().to_string(),
                agent_auto_read: read_channel.agent_auto_read(),
            },
        }
    }
}

/// Converts the DTO's `read_channel` fields into the typed
/// [`ReadChannel`] and checks the coherence rule directly — no
/// intermediate `Value` at any point.
fn build_and_check_read_channel(
    at: &str,
    source_id: &str,
    dto: &ReadChannelDto,
    problems: &mut Vec<Diagnostic>,
) {
    let kind = match ReadChannelKind::parse(&dto.kind) {
        Some(k) => Some(k),
        None => {
            problems.push(fail(format!(
                "{at} source_ref: the resolved instruction source's read_channel.kind \"{}\" is not one of meridian-observed, agent-native, manual",
                dto.kind
            )));
            None
        }
    };
    let visibility = match MeridianVisibility::parse(&dto.meridian_visibility) {
        Some(v) => Some(v),
        None => {
            problems.push(fail(format!(
                "{at} source_ref: the resolved instruction source's read_channel.meridian_visibility \"{}\" is not one of full, partial, none",
                dto.meridian_visibility
            )));
            None
        }
    };
    let (Some(kind), Some(visibility)) = (kind, visibility) else {
        return;
    };
    if let Err(coherence_problems) =
        ReadChannel::try_new(source_id, kind, visibility, dto.agent_auto_read)
    {
        for d in coherence_problems {
            problems.push(fail(format!("{at} source_ref: resolved {}", d.message())));
        }
    }
}

/// Validates the resolver's closed DTO into a [`ResolvedInstructionSource`]
/// — every shape defect is accumulated, not just the first (corrective
/// round item 9); `Ok` is returned only when every field parsed. This is
/// still a transport/syntax-adjacent function (it inspects the DTO's own
/// string fields, e.g. checking `sha256` is well-formed hex) — the actual
/// BUSINESS comparison against the pin happens only in
/// [`meridian_core::controlled_rule_intake::checks::check_source_ref`],
/// which this function's caller invokes separately, never here.
pub(crate) fn validate_resolved_response(
    candidate_id: &str,
    source_id: &str,
    dto: ResolvedInstructionSourceDto,
) -> Result<ResolvedInstructionSource, Vec<Diagnostic>> {
    let mut problems = Vec::new();
    let at = format!("rule candidate \"{candidate_id}\"");

    if dto.record_type != SOURCE_RECORD_TYPE {
        problems.push(fail(format!(
            "{at} source_ref resolves to a \"{}\" record, not \"{SOURCE_RECORD_TYPE}\"",
            dto.record_type
        )));
    }
    let id = try_field(&mut problems, &at, SemanticId::new(&dto.id));
    let reference = try_field(&mut problems, &at, NonEmptyString::new(&dto.reference));
    let revision = try_field(
        &mut problems,
        &at,
        Revision::new(&dto.recorded_state.revision),
    );
    let digest = try_field(
        &mut problems,
        &at,
        ContentDigest::from_hex(&dto.recorded_state.digest.value),
    );
    if dto.recorded_state.digest.algorithm != "sha-256" {
        problems.push(fail(format!("{at} source_ref: the resolved instruction source's recorded_state.digest.algorithm is not \"sha-256\"")));
    }
    let currency = match Currency::parse(&dto.recorded_state.currency) {
        Some(c) => Some(c),
        None => {
            problems.push(fail(format!(
                "{at} source_ref: the resolved instruction source's recorded_state.currency \"{}\" is not one of current, stale, unverified",
                dto.recorded_state.currency
            )));
            None
        }
    };

    build_and_check_read_channel(&at, source_id, &dto.read_channel, &mut problems);

    match (id, reference, revision, digest, currency) {
        (Some(id), Some(reference), Some(revision), Some(digest), Some(currency))
            if problems.is_empty() =>
        {
            Ok(ResolvedInstructionSource::new(
                id,
                reference,
                RecordedState::new(
                    revision,
                    digest,
                    dto.recorded_state.revision_verified,
                    currency,
                ),
            ))
        }
        _ => Err(problems),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok_dto() -> ResolvedInstructionSourceDto {
        ResolvedInstructionSourceDto {
            record_type: SOURCE_RECORD_TYPE.to_string(),
            id: "src-1".to_string(),
            reference: "sources/src-1".to_string(),
            recorded_state: RecordedStateDto {
                revision: "a".repeat(40),
                digest: DigestDto {
                    algorithm: "sha-256".to_string(),
                    value: "b".repeat(64),
                },
                revision_verified: true,
                currency: "current".to_string(),
            },
            read_channel: ReadChannelDto {
                kind: "meridian-observed".to_string(),
                meridian_visibility: "full".to_string(),
                agent_auto_read: false,
            },
        }
    }

    #[test]
    fn a_well_formed_response_validates_into_a_resolved_instruction_source() {
        let resolved = validate_resolved_response("cand-1", "src-1", ok_dto());
        assert!(resolved.is_ok(), "{resolved:?}");
    }

    /// Every simultaneous shape defect is reported, not just the first
    /// (corrective round item 9).
    #[test]
    fn multiple_simultaneous_shape_defects_are_all_reported() {
        let mut dto = ok_dto();
        dto.recorded_state.digest.algorithm = "md5".to_string();
        dto.recorded_state.currency = "invented".to_string();
        let errors = validate_resolved_response("cand-1", "src-1", dto).unwrap_err();
        assert!(
            errors.iter().any(|d| d.message().contains("sha-256")),
            "{errors:?}"
        );
        assert!(
            errors.iter().any(|d| d.message().contains("invented")),
            "{errors:?}"
        );
    }

    /// The read_channel coherence rule (corrective round item 2) is
    /// reached through the typed conversion, not a `Value` round-trip —
    /// this asserts the RULE still actually fires end to end.
    #[test]
    fn an_incoherent_read_channel_is_rejected() {
        let mut dto = ok_dto();
        dto.read_channel = ReadChannelDto {
            kind: "agent-native".to_string(),
            meridian_visibility: "full".to_string(),
            agent_auto_read: true,
        };
        let errors = validate_resolved_response("cand-1", "src-1", dto).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|d| d.message().contains("Meridian cannot fully observe")),
            "{errors:?}"
        );
    }
}
