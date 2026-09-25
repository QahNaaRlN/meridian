//! [`OwnerDecision`] — the `controlled-rule-intake` contract's owner
//! decision (`controlled-rule-intake.schema.json`, `definitions.owner_decision`),
//! whose schema shares the migration plan's shape but whose contract
//! requires non-blank text. A migration plan's own `owner_decision` is the
//! schema-shaped [`super::plan::OwnerDecisionInput`].

use crate::resolver::IsoDate;
use crate::types::NonEmptyString;

/// An owner's recorded decision: a reference, a date and a reason.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OwnerDecision {
    decision_ref: NonEmptyString,
    decided_at: IsoDate,
    reason: NonEmptyString,
}

impl OwnerDecision {
    pub fn new(decision_ref: NonEmptyString, decided_at: IsoDate, reason: NonEmptyString) -> Self {
        Self {
            decision_ref,
            decided_at,
            reason,
        }
    }

    pub fn decision_ref(&self) -> &str {
        self.decision_ref.as_str()
    }
    pub fn decided_at(&self) -> &IsoDate {
        &self.decided_at
    }
    pub fn reason(&self) -> &str {
        self.reason.as_str()
    }
}
