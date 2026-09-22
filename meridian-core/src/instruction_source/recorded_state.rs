//! [`RecordedState`] — the snapshot an instruction-source-registry entry
//! currently holds for its source (`instruction-source-registry.md`).
//!
//! Moved here from `controlled_rule_intake::resolved` by
//! `rust-architecture-conformance-2` (§1): `recorded_state` is the
//! registry's own concept, reused both by the registry entry itself
//! ([`super::InstructionSourcePayload`]) and by a resolved snapshot
//! ([`super::ResolvedInstructionSource`]) a `controlled-rule-intake`
//! candidate compares its pin against. `controlled_rule_intake` re-exports
//! this type unchanged.

use super::Currency;
use crate::types::{ContentDigest, Revision};

/// The snapshot an instruction-source-registry entry currently holds for
/// its source: an explicit revision and a SHA-256 digest. The ONLY way to
/// construct a value of this type is [`RecordedState::new`] — its fields
/// are private and there is no other public constructor, so a caller
/// outside this module cannot assemble one field by field.
///
/// [`RecordedState::new`] is infallible on purpose: `revision`, `digest`,
/// `revision_verified` and `currency` are four independent facts a resolver
/// or the registry itself reports about a snapshot, and every combination
/// of them (already-valid components each) is itself a representable
/// domain state — including, say, `revision_verified: false` together with
/// `currency: Current`. Whether a GIVEN combination is acceptable is a
/// business rule that also depends on context this type does not carry on
/// its own (a candidate's `applicability_state` for
/// [`crate::controlled_rule_intake::checks::check_source_ref`]; the
/// entry's own `divergence` for [`super::InstructionSource::try_new`]) —
/// this is a deliberate choice, not an oversight: adding validation here
/// with nothing of its own to validate against would be fictitious.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecordedState {
    revision: Revision,
    digest: ContentDigest,
    revision_verified: bool,
    currency: Currency,
}

impl RecordedState {
    pub fn new(
        revision: Revision,
        digest: ContentDigest,
        revision_verified: bool,
        currency: Currency,
    ) -> Self {
        Self {
            revision,
            digest,
            revision_verified,
            currency,
        }
    }

    pub fn revision(&self) -> &Revision {
        &self.revision
    }

    pub fn digest(&self) -> &ContentDigest {
        &self.digest
    }

    pub fn revision_verified(&self) -> bool {
        self.revision_verified
    }

    pub fn currency(&self) -> Currency {
        self.currency
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_and_its_getters_round_trip_every_field() {
        let rs = RecordedState::new(
            Revision::new("a".repeat(40)).unwrap(),
            ContentDigest::from_hex("b".repeat(64)).unwrap(),
            true,
            Currency::Current,
        );
        assert_eq!(rs.revision().as_str(), "a".repeat(40));
        assert_eq!(rs.digest().value(), "b".repeat(64));
        assert!(rs.revision_verified());
        assert_eq!(rs.currency(), Currency::Current);
    }
}
