//! [`Divergence`] — the result of comparing the snapshot held before the
//! last check ([`Divergence::previous_state`]) with the fresh observation
//! that check produced ([`Divergence::current_state`])
//! (`instruction-source-registry.md` §2.6-§2.7). Two verified
//! [`ObservedState`]s are `unchanged` only when BOTH revision and SHA-256
//! digest match; a difference in either is `changed`. `unknown` is only for
//! genuinely insufficient evidence. `source-missing`/`source-unreadable`
//! carry no `current_state` at all.
//!
//! [`Divergence::try_new`] is the ONLY constructor (`rust-architecture-conformance-2`,
//! §1): it runs the same declared-status-vs-evidence rule the Node
//! reference's `checkDivergenceClaim` checks against an already-built
//! value, so a `Divergence` whose declared status contradicts its own
//! `previous_state`/`current_state` is unrepresentable, not merely flagged
//! after the fact.

use crate::types::{ContentDigest, Diagnostic, Revision};

use super::fail;

/// One previous-or-current observation of a source, used to compute
/// [`DivergenceStatus`]. `verified` is whether this exact state was
/// actually read back from the source; an unverified state cannot support
/// an `unchanged` or `changed` claim.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObservedState {
    revision: Revision,
    digest: ContentDigest,
    verified: bool,
}

impl ObservedState {
    /// Infallible: `revision`, `digest` and `verified` are three
    /// independent, already-valid facts with no cross-field invariant of
    /// their own — whether an UNVERIFIED state is acceptable at all is
    /// [`Divergence::try_new`]'s business, not this type's.
    pub fn new(revision: Revision, digest: ContentDigest, verified: bool) -> Self {
        Self {
            revision,
            digest,
            verified,
        }
    }

    pub fn revision(&self) -> &Revision {
        &self.revision
    }

    pub fn digest(&self) -> &ContentDigest {
        &self.digest
    }

    pub fn verified(&self) -> bool {
        self.verified
    }
}

/// The closed `divergence.status` pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DivergenceStatus {
    Unknown,
    Unchanged,
    Changed,
    SourceMissing,
    SourceUnreadable,
}

impl DivergenceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            DivergenceStatus::Unknown => "unknown",
            DivergenceStatus::Unchanged => "unchanged",
            DivergenceStatus::Changed => "changed",
            DivergenceStatus::SourceMissing => "source-missing",
            DivergenceStatus::SourceUnreadable => "source-unreadable",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "unknown" => Some(DivergenceStatus::Unknown),
            "unchanged" => Some(DivergenceStatus::Unchanged),
            "changed" => Some(DivergenceStatus::Changed),
            "source-missing" => Some(DivergenceStatus::SourceMissing),
            "source-unreadable" => Some(DivergenceStatus::SourceUnreadable),
            _ => None,
        }
    }

    /// Whether this status means the source itself could not be observed
    /// at all this check (`source-missing`/`source-unreadable`) — the two
    /// statuses that carry no `current_state`.
    pub fn is_source_gone(self) -> bool {
        matches!(
            self,
            DivergenceStatus::SourceMissing | DivergenceStatus::SourceUnreadable
        )
    }
}

impl core::fmt::Display for DivergenceStatus {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

fn both_verified(previous: Option<&ObservedState>, current: Option<&ObservedState>) -> bool {
    previous.is_some_and(ObservedState::verified) && current.is_some_and(ObservedState::verified)
}

/// The result of comparing the snapshot held before the last check with the
/// fresh observation that check produced. The ONLY way to construct a value
/// of this type is [`Divergence::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Divergence {
    status: DivergenceStatus,
    previous_state: Option<ObservedState>,
    current_state: Option<ObservedState>,
}

impl Divergence {
    /// The canonical divergence status computed from two states, regardless
    /// of what a caller declares. Two VERIFIED states are `Unchanged` only
    /// when both the revision AND the SHA-256 digest match; a difference in
    /// either is `Changed`. Anything short of two verified states is
    /// `Unknown` — the comparison cannot be made.
    pub fn compute(
        previous: Option<&ObservedState>,
        current: Option<&ObservedState>,
    ) -> DivergenceStatus {
        let (Some(previous), Some(current)) = (previous, current) else {
            return DivergenceStatus::Unknown;
        };
        if !previous.verified || !current.verified {
            return DivergenceStatus::Unknown;
        }
        if previous.revision == current.revision && previous.digest == current.digest {
            DivergenceStatus::Unchanged
        } else {
            DivergenceStatus::Changed
        }
    }

    /// Validates the declared `status` against `previous_state`/
    /// `current_state` and only then builds `Self`. `id` names the
    /// instruction source this divergence belongs to, for the diagnostic
    /// text only.
    pub fn try_new(
        id: &str,
        status: DivergenceStatus,
        previous_state: Option<ObservedState>,
        current_state: Option<ObservedState>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let at = format!("instruction source \"{id}\"");
        let verified = both_verified(previous_state.as_ref(), current_state.as_ref());

        match status {
            DivergenceStatus::Unchanged | DivergenceStatus::Changed => {
                if !verified {
                    return Err(vec![fail(format!(
                        "{at} divergence status \"{status}\" needs a verifiable previous and current state (each with a revision, a sha-256 digest and verified: true); a changed, missing, unreadable or unverified source is never treated as matching"
                    ))]);
                }
                let computed = Self::compute(previous_state.as_ref(), current_state.as_ref());
                if status != computed {
                    let previous = previous_state.as_ref().expect("verified implies present");
                    let current = current_state.as_ref().expect("verified implies present");
                    let same_revision = previous.revision == current.revision;
                    let same_digest = previous.digest == current.digest;
                    let detail = if same_revision && !same_digest {
                        "the SHA-256 digest differs while the revision is unchanged"
                    } else if !same_revision && same_digest {
                        "the revision differs while the SHA-256 digest is unchanged"
                    } else if !same_revision && !same_digest {
                        "both the revision and the SHA-256 digest differ"
                    } else {
                        "the revision and the SHA-256 digest both match"
                    };
                    return Err(vec![fail(format!(
                        "{at} divergence status \"{status}\" contradicts the two verified states: {detail}, which computes \"{computed}\""
                    ))]);
                }
            }
            DivergenceStatus::Unknown => {
                if verified {
                    return Err(vec![fail(format!(
                        "{at} divergence status \"unknown\" is declared with two complete verified states; when both states are present and verified the divergence is computable and the declared status must equal the computed one"
                    ))]);
                }
            }
            DivergenceStatus::SourceMissing | DivergenceStatus::SourceUnreadable => {
                if current_state.is_some() {
                    return Err(vec![fail(format!(
                        "{at} divergence status \"{status}\" carries a current_state; a source that is missing or unreadable has no current state to observe"
                    ))]);
                }
            }
        }

        Ok(Self {
            status,
            previous_state,
            current_state,
        })
    }

    pub fn status(&self) -> DivergenceStatus {
        self.status
    }

    pub fn previous_state(&self) -> Option<&ObservedState> {
        self.previous_state.as_ref()
    }

    pub fn current_state(&self) -> Option<&ObservedState> {
        self.current_state.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verified(revision: &str, digest: &str) -> ObservedState {
        ObservedState::new(
            Revision::new(revision).unwrap(),
            ContentDigest::from_hex(digest.to_string()).unwrap(),
            true,
        )
    }

    #[test]
    fn compute_is_unknown_without_two_verified_snapshots() {
        assert_eq!(Divergence::compute(None, None), DivergenceStatus::Unknown);
    }

    #[test]
    fn compute_is_unchanged_when_revision_and_digest_match() {
        let a = verified(&"a".repeat(40), &"b".repeat(64));
        let b = verified(&"a".repeat(40), &"b".repeat(64));
        assert_eq!(
            Divergence::compute(Some(&a), Some(&b)),
            DivergenceStatus::Unchanged
        );
    }

    #[test]
    fn compute_is_changed_when_digest_differs() {
        let a = verified(&"a".repeat(40), &"b".repeat(64));
        let b = verified(&"a".repeat(40), &"c".repeat(64));
        assert_eq!(
            Divergence::compute(Some(&a), Some(&b)),
            DivergenceStatus::Changed
        );
    }

    #[test]
    fn try_new_rejects_unchanged_without_two_verified_states() {
        let err = Divergence::try_new("s1", DivergenceStatus::Unchanged, None, None).unwrap_err();
        assert!(err[0]
            .message()
            .contains("needs a verifiable previous and current state"));
    }

    #[test]
    fn try_new_rejects_unchanged_that_contradicts_the_states() {
        let previous = verified(&"a".repeat(40), &"b".repeat(64));
        let current = verified(&"a".repeat(40), &"c".repeat(64));
        let err = Divergence::try_new(
            "s1",
            DivergenceStatus::Unchanged,
            Some(previous),
            Some(current),
        )
        .unwrap_err();
        assert!(err[0].message().contains("contradicts"));
    }

    #[test]
    fn try_new_rejects_unknown_with_two_complete_verified_states() {
        let previous = verified(&"a".repeat(40), &"b".repeat(64));
        let current = verified(&"a".repeat(40), &"b".repeat(64));
        let err = Divergence::try_new(
            "s1",
            DivergenceStatus::Unknown,
            Some(previous),
            Some(current),
        )
        .unwrap_err();
        assert!(err[0].message().contains("computable"));
    }

    #[test]
    fn try_new_rejects_source_missing_carrying_a_current_state() {
        let current = verified(&"a".repeat(40), &"b".repeat(64));
        let err = Divergence::try_new("s1", DivergenceStatus::SourceMissing, None, Some(current))
            .unwrap_err();
        assert!(err[0].message().contains("no current state to observe"));
    }

    #[test]
    fn try_new_accepts_a_coherent_unknown_divergence() {
        assert!(Divergence::try_new("s1", DivergenceStatus::Unknown, None, None).is_ok());
    }

    #[test]
    fn try_new_accepts_source_missing_with_a_matching_previous_state() {
        let previous = verified(&"a".repeat(40), &"b".repeat(64));
        assert!(
            Divergence::try_new("s1", DivergenceStatus::SourceMissing, Some(previous), None)
                .is_ok()
        );
    }
}
