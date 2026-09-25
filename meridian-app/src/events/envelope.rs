//! [`ObservedEvent`] — one versioned, closed-kind observation.

use core::fmt;

use meridian_core::types::NonEmptyString;

/// The version of the [`ObservedEvent`] envelope shape itself — closed, so
/// a future second generation is added as a new variant, never a raw
/// integer an unrecognised value could silently satisfy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventEnvelopeVersion {
    V1,
}

impl EventEnvelopeVersion {
    pub const CURRENT: EventEnvelopeVersion = EventEnvelopeVersion::V1;

    pub fn as_u32(self) -> u32 {
        match self {
            EventEnvelopeVersion::V1 => 1,
        }
    }
}

impl fmt::Display for EventEnvelopeVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_u32())
    }
}

/// The closed set of things one [`ObservedEvent`] can describe
/// (`meridian-rust-migration-program-plan.md` §5.4, item 5). There is no
/// tenth variant for a model's internal chain of reasoning — it is not a
/// kind of observed event and is never accepted as one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    /// The input context a command started from.
    InputContext,
    /// A read of an external source.
    SourceAccess,
    /// A reference the command found while running.
    ReferenceFound,
    /// A tool or subprocess the command invoked.
    ToolInvocation,
    /// A change the command made.
    Change,
    /// A check the command ran.
    Check,
    /// An error the command encountered.
    Error,
    /// A correction the command applied after an error.
    Correction,
    /// The command's final outcome.
    Outcome,
}

impl EventKind {
    /// Every kind this closed set carries, in the order this module
    /// documents them — used by tests to prove the set is exactly this
    /// size, not "at least" this size.
    pub const ALL: [EventKind; 9] = [
        EventKind::InputContext,
        EventKind::SourceAccess,
        EventKind::ReferenceFound,
        EventKind::ToolInvocation,
        EventKind::Change,
        EventKind::Check,
        EventKind::Error,
        EventKind::Correction,
        EventKind::Outcome,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            EventKind::InputContext => "input-context",
            EventKind::SourceAccess => "source-access",
            EventKind::ReferenceFound => "reference-found",
            EventKind::ToolInvocation => "tool-invocation",
            EventKind::Change => "change",
            EventKind::Check => "check",
            EventKind::Error => "error",
            EventKind::Correction => "correction",
            EventKind::Outcome => "outcome",
        }
    }
}

impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One observed event: a version, a closed [`EventKind`], and a plain-text
/// summary — nothing else.
///
/// This corrective package (`knowledge-agent-foundation`) deliberately
/// carries no structured "detail" field. An earlier revision attached a
/// free-form JSON object (`meridian_app::storage::Payload`) here as
/// per-event detail; that was itself an unbounded channel a future caller
/// could have used to smuggle an internal reasoning trace through, however
/// unintentionally — an arbitrary JSON object cannot be shown by
/// construction to exclude a `"reasoning"` or `"chain_of_thought"` key.
/// Removing that field is what actually closes the *structured* channel:
/// [`ObservedEvent::new`] takes exactly `(kind, summary)`, so there is no
/// parameter through which an arbitrary nested object — reasoning or
/// otherwise — could be attached, and this crate's own test
/// `the_public_shape_is_exactly_version_kind_and_summary` proves this by
/// exhaustively destructuring every field this type has. This does
/// **not** close every channel: `summary` is a plain, unbounded string, and
/// nothing here stops a caller from writing an extended reasoning trace
/// into that string itself — that is a matter of what a caller chooses to
/// pass, not a guarantee this type provides. A closed, per-kind detail
/// shape may be introduced by a later package if a concrete need for one is
/// demonstrated; until then, "no detail field" bounds the risk more
/// strongly than any detail shape this package could invent without a real
/// consumer to validate it against.
#[derive(Debug, Clone, PartialEq)]
pub struct ObservedEvent {
    envelope_version: EventEnvelopeVersion,
    kind: EventKind,
    summary: NonEmptyString,
}

impl ObservedEvent {
    /// Builds an event at the current envelope version. Takes exactly a
    /// kind and a summary — no third parameter exists for arbitrary
    /// structured detail (see the type's own documentation above).
    pub fn new(kind: EventKind, summary: NonEmptyString) -> Self {
        Self {
            envelope_version: EventEnvelopeVersion::CURRENT,
            kind,
            summary,
        }
    }

    pub fn envelope_version(&self) -> EventEnvelopeVersion {
        self.envelope_version
    }

    pub fn kind(&self) -> EventKind {
        self.kind
    }

    pub fn summary(&self) -> &str {
        self.summary.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nes(s: &str) -> NonEmptyString {
        NonEmptyString::new(s).unwrap()
    }

    #[test]
    fn carries_the_current_envelope_version() {
        let event = ObservedEvent::new(EventKind::Check, nes("ran cargo test"));
        assert_eq!(event.envelope_version(), EventEnvelopeVersion::CURRENT);
        assert_eq!(event.envelope_version().as_u32(), 1);
    }

    #[test]
    fn round_trips_kind_and_summary() {
        let event = ObservedEvent::new(EventKind::Outcome, nes("succeeded"));
        assert_eq!(event.kind(), EventKind::Outcome);
        assert_eq!(event.summary(), "succeeded");
    }

    /// Type-level proof, not a runtime assertion: this exhaustive struct
    /// destructure only compiles if [`ObservedEvent`] has exactly these
    /// three fields and no others. Adding a fourth field — a detail
    /// payload, a "reasoning" string, anything — would fail this pattern
    /// at compile time until it is named here explicitly, which is exactly
    /// the guarantee this test exists to pin down.
    #[test]
    fn the_public_shape_is_exactly_version_kind_and_summary() {
        let event = ObservedEvent::new(EventKind::Change, nes("edited schema.rs"));
        let ObservedEvent {
            envelope_version,
            kind,
            summary,
        } = event;
        assert_eq!(envelope_version, EventEnvelopeVersion::CURRENT);
        assert_eq!(kind, EventKind::Change);
        assert_eq!(summary.as_str(), "edited schema.rs");
    }

    /// Proves the closed set is exactly nine kinds — not "at least nine" —
    /// and, by exhaustive match, that there is no tenth "reasoning" or
    /// "chain of thought" kind a future edit could add without this test
    /// being updated to name it explicitly.
    #[test]
    fn closed_event_kind_set_has_exactly_the_nine_documented_kinds() {
        assert_eq!(EventKind::ALL.len(), 9);
        for kind in EventKind::ALL {
            let described = match kind {
                EventKind::InputContext
                | EventKind::SourceAccess
                | EventKind::ReferenceFound
                | EventKind::ToolInvocation
                | EventKind::Change
                | EventKind::Check
                | EventKind::Error
                | EventKind::Correction
                | EventKind::Outcome => true,
            };
            assert!(described);
        }
    }
}
