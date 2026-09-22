//! [`InstructionSource`] — one valid, constructed instruction-source-registry
//! entry (`instruction-source-registry.md`). The ONLY way to construct a
//! value of this type is [`InstructionSource::try_new`]
//! (`rust-architecture-conformance-2`, §1): it runs the temporal
//! consistency rule between `recorded_state` and `divergence` (the Node
//! reference's `checkStateCoherence`) before allowing construction — an
//! incoherent combination is unrepresentable, not merely flagged after the
//! fact, mirroring [`crate::controlled_rule_intake::RuleCandidate::try_new`].

use crate::types::{Diagnostic, SemanticId};

use super::{fail, Currency, Divergence, Location, ReadChannel, RecordedState, SourceFormat};

/// The fixed `record_type` of an instruction-source-registry entry.
pub const RECORD_TYPE: &str = "instruction-source";

/// The only accepted `payload.normative_status`: registration grants the
/// source's text no norm authority, accepts no rule and resolves no
/// conflict.
pub const NORMATIVE_STATUS: &str = "not-a-norm";

/// `payload` — everything about a registered instruction source beyond its
/// scoped-record envelope. Every field is independently valid by the time a
/// value of this type exists; the cross-field temporal check that also
/// needs `recorded_state` AND `divergence` together lives on
/// [`InstructionSource::try_new`], not here — mirroring
/// `RuleCandidatePayload`'s own split.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstructionSourcePayload {
    location: Location,
    format: SourceFormat,
    recorded_state: RecordedState,
    read_channel: ReadChannel,
    divergence: Divergence,
}

impl InstructionSourcePayload {
    pub fn new(
        location: Location,
        format: SourceFormat,
        recorded_state: RecordedState,
        read_channel: ReadChannel,
        divergence: Divergence,
    ) -> Self {
        Self {
            location,
            format,
            recorded_state,
            read_channel,
            divergence,
        }
    }

    pub fn location(&self) -> &Location {
        &self.location
    }

    pub fn format(&self) -> SourceFormat {
        self.format
    }

    pub fn recorded_state(&self) -> &RecordedState {
        &self.recorded_state
    }

    pub fn read_channel(&self) -> &ReadChannel {
        &self.read_channel
    }

    pub fn divergence(&self) -> &Divergence {
        &self.divergence
    }
}

/// One registered instruction source: its stable id plus its payload.
///
/// The ONLY way to construct a value of this type is
/// [`InstructionSource::try_new`]: there is no public constructor that
/// skips its cross-field temporal check, so a `recorded_state` inconsistent
/// with its own `divergence` (the entry's `currency` claiming `"current"`
/// while `divergence.status` says the source is missing, for instance)
/// cannot exist as an `InstructionSource` at all, not even transiently.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstructionSource {
    id: SemanticId,
    payload: InstructionSourcePayload,
}

impl InstructionSource {
    pub fn try_new(
        id: SemanticId,
        payload: InstructionSourcePayload,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut problems = Vec::new();
        check_state_coherence(id.as_str(), &payload, &mut problems);
        if problems.is_empty() {
            Ok(Self { id, payload })
        } else {
            Err(problems)
        }
    }

    pub fn id(&self) -> &SemanticId {
        &self.id
    }

    pub fn payload(&self) -> &InstructionSourcePayload {
        &self.payload
    }
}

/// `recorded_state` and `divergence` together describe one present
/// observation. This is where the temporal model is enforced — port of the
/// Node reference's `checkStateCoherence`, operating on already-typed
/// values instead of a `Value` tree.
fn check_state_coherence(
    id: &str,
    payload: &InstructionSourcePayload,
    problems: &mut Vec<Diagnostic>,
) {
    let at = format!("instruction source \"{id}\"");
    let rs = payload.recorded_state();
    let d = payload.divergence();
    let status = d.status();
    let verified = rs.revision_verified();
    let currency = rs.currency();

    if verified && currency == Currency::Unverified {
        problems.push(fail(format!(
            "{at}: recorded_state.revision_verified is true but currency is \"unverified\"; a verified snapshot is \"current\" or \"stale\", never \"unverified\""
        )));
    }
    if !verified && currency != Currency::Unverified {
        problems.push(fail(format!(
            "{at}: recorded_state.revision_verified is not true, so currency must be \"unverified\""
        )));
    }

    if currency == Currency::Current {
        if !verified {
            problems.push(fail(format!(
                "{at}: recorded_state.currency is \"current\" but revision_verified is not true; an unverified revision is not declared current"
            )));
        }
        if status.is_source_gone() {
            problems.push(fail(format!(
                "{at}: recorded_state.currency is \"current\" but divergence.status is \"{status}\"; a source that is missing or unreadable cannot have a current snapshot — record the verified last-known state as \"stale\""
            )));
        }
    }

    if currency == Currency::Stale {
        if !verified {
            problems.push(fail(format!(
                "{at}: recorded_state.currency is \"stale\" but revision_verified is not true; \"stale\" is a verified last-known state — an unverified snapshot is \"unverified\""
            )));
        }
        if !status.is_source_gone() {
            problems.push(fail(format!(
                "{at}: recorded_state.currency is \"stale\" but divergence.status is \"{status}\"; \"stale\" needs positive evidence (source-missing or source-unreadable) that the source no longer matches the held snapshot"
            )));
        }
    }

    if let Some(cur) = d.current_state() {
        if cur.revision() != rs.revision() {
            problems.push(fail(format!(
                "{at}: divergence.current_state.revision \"{}\" contradicts recorded_state.revision \"{}\"; both describe the source as it is now and must agree",
                cur.revision(),
                rs.revision()
            )));
        }
        if cur.digest() != rs.digest() {
            problems.push(fail(format!(
                "{at}: divergence.current_state digest contradicts recorded_state digest; both describe the source as it is now and must agree"
            )));
        }
        if cur.verified() != verified {
            problems.push(fail(format!(
                "{at}: divergence.current_state.verified is {} but recorded_state.revision_verified is {}; one present observation cannot be both verified and not verified",
                cur.verified(),
                verified
            )));
        }
    }

    if status.is_source_gone() {
        if let Some(prev) = d.previous_state() {
            if prev.revision() != rs.revision() {
                problems.push(fail(format!(
                    "{at}: divergence.status is \"{status}\" with a previous_state whose revision \"{}\" contradicts recorded_state.revision \"{}\"; the last-known state must match the snapshot the registry holds",
                    prev.revision(),
                    rs.revision()
                )));
            }
            if prev.digest() != rs.digest() {
                problems.push(fail(format!(
                    "{at}: divergence.status is \"{status}\" with a previous_state whose digest contradicts recorded_state digest; the last-known state must match the snapshot the registry holds"
                )));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruction_source::{
        DivergenceStatus, MeridianVisibility, ObservedState, OpaqueRef, ReadChannelKind,
        RelativePath,
    };
    use crate::types::{ContentDigest, Revision};

    fn read_channel() -> ReadChannel {
        ReadChannel::try_new(
            "s1",
            ReadChannelKind::MeridianObserved,
            MeridianVisibility::Full,
            false,
        )
        .unwrap()
    }

    fn location() -> Location {
        Location::file(
            RelativePath::new("a.md").unwrap(),
            OpaqueRef::new("container-1").unwrap(),
        )
    }

    fn recorded_state(verified: bool, currency: Currency) -> RecordedState {
        RecordedState::new(
            Revision::new("a".repeat(40)).unwrap(),
            ContentDigest::from_hex("b".repeat(64)).unwrap(),
            verified,
            currency,
        )
    }

    #[test]
    fn try_new_accepts_a_coherent_current_source() {
        let payload = InstructionSourcePayload::new(
            location(),
            SourceFormat::MarkdownSection,
            recorded_state(true, Currency::Current),
            read_channel(),
            Divergence::try_new("s1", DivergenceStatus::Unknown, None, None).unwrap(),
        );
        let source = InstructionSource::try_new(SemanticId::new("s1").unwrap(), payload);
        assert!(source.is_ok(), "{source:?}");
    }

    /// A currency that claims "current" while divergence says the source is
    /// gone is unrepresentable — `try_new` rejects it, there is no
    /// infallible constructor to bypass the rule.
    #[test]
    fn try_new_rejects_current_currency_with_a_source_gone_divergence() {
        let payload = InstructionSourcePayload::new(
            location(),
            SourceFormat::MarkdownSection,
            recorded_state(true, Currency::Current),
            read_channel(),
            Divergence::try_new("s1", DivergenceStatus::SourceMissing, None, None).unwrap(),
        );
        let err = InstructionSource::try_new(SemanticId::new("s1").unwrap(), payload).unwrap_err();
        assert!(
            err.iter()
                .any(|d| d.message().contains("cannot have a current snapshot")),
            "{err:?}"
        );
    }

    /// Multiple independent defects (revision_verified/currency mismatch
    /// AND current_state disagreement) are accumulated together.
    #[test]
    fn try_new_accumulates_multiple_simultaneous_defects() {
        let current_state = ObservedState::new(
            Revision::new("different-revision").unwrap(),
            ContentDigest::from_hex("c".repeat(64)).unwrap(),
            true,
        );
        let payload = InstructionSourcePayload::new(
            location(),
            SourceFormat::MarkdownSection,
            recorded_state(false, Currency::Current),
            read_channel(),
            Divergence::try_new("s1", DivergenceStatus::Unknown, None, Some(current_state))
                .unwrap(),
        );
        let err = InstructionSource::try_new(SemanticId::new("s1").unwrap(), payload).unwrap_err();
        assert!(
            err.iter().any(|d| d
                .message()
                .contains("unverified revision is not declared current")),
            "{err:?}"
        );
        assert!(
            err.iter()
                .any(|d| d.message().contains("contradicts recorded_state.revision")),
            "{err:?}"
        );
        assert!(err.len() >= 2, "{err:?}");
    }
}
