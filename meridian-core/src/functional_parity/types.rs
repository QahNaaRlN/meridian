//! Closed domain types for one `functional-parity` evidence record
//! (`verification/functional-parity/functional-parity-evidence.schema.json`):
//! the four facets, the six neutral evidence kinds, the dotted-kebab id
//! shapes this contract's own JSON Schema declares, and [`EvidenceText`] —
//! each its own type, never a raw `String` compared against a string
//! literal and never a type this crate already carries for a DIFFERENT
//! contract's differently-shaped closed set
//! (`rust-architecture-conformance-4`, `governance/plans/meridian-rust-migration-program-plan.md`
//! §5.18.3 point 2). Mutually exclusive states — a facet's declared
//! assertions vs. its `not_applicable_justification`, `same` vs.
//! `explicitly-comparable` post-change conditions, a record- vs.
//! assertion-scoped gap, and a `VERIFIED`/`UNVERIFIED` verdict with its
//! corresponding (absent/required) reason — are enum forms carrying their
//! own data in [`super::construction`], not a flat marker enum plus a
//! separately-optional field a caller could construct inconsistently; see
//! that module's own doc comments.

use core::fmt;

/// The four facets of the observable contract a record's
/// `preserved_contract` declares, in the fixed order the schema declares
/// them and [`crate::functional_parity::checks`] walks them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Facet {
    PublicApi,
    ObservableIo,
    SideEffectsAndInteractions,
    UserVisibleBehavior,
}

impl Facet {
    /// The four facets, in the one declaration order every diagnostic walk
    /// in this contract uses.
    pub const ALL: [Facet; 4] = [
        Facet::PublicApi,
        Facet::ObservableIo,
        Facet::SideEffectsAndInteractions,
        Facet::UserVisibleBehavior,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Facet::PublicApi => "public_api",
            Facet::ObservableIo => "observable_io",
            Facet::SideEffectsAndInteractions => "side_effects_and_interactions",
            Facet::UserVisibleBehavior => "user_visible_behavior",
        }
    }

    pub fn parse(value: &str) -> Option<Facet> {
        match value {
            "public_api" => Some(Facet::PublicApi),
            "observable_io" => Some(Facet::ObservableIo),
            "side_effects_and_interactions" => Some(Facet::SideEffectsAndInteractions),
            "user_visible_behavior" => Some(Facet::UserVisibleBehavior),
            _ => None,
        }
    }
}

impl fmt::Display for Facet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The unordered set of six neutral evidence kinds
/// (`definitions.evidence_entry.kind`). No kind is mandatory, primary or
/// default; `PublicContractSnapshot` is the one kind the schema requires an
/// `applicability_justification` for, checked at the schema level, not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvidenceKind {
    BehavioralAssertion,
    ObservationLog,
    InterfaceEnumeration,
    InteractionTrace,
    PublicContractSnapshot,
    DifferentialExecution,
}

impl EvidenceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EvidenceKind::BehavioralAssertion => "behavioral-assertion",
            EvidenceKind::ObservationLog => "observation-log",
            EvidenceKind::InterfaceEnumeration => "interface-enumeration",
            EvidenceKind::InteractionTrace => "interaction-trace",
            EvidenceKind::PublicContractSnapshot => "public-contract-snapshot",
            EvidenceKind::DifferentialExecution => "differential-execution",
        }
    }

    pub fn parse(value: &str) -> Option<EvidenceKind> {
        match value {
            "behavioral-assertion" => Some(EvidenceKind::BehavioralAssertion),
            "observation-log" => Some(EvidenceKind::ObservationLog),
            "interface-enumeration" => Some(EvidenceKind::InterfaceEnumeration),
            "interaction-trace" => Some(EvidenceKind::InteractionTrace),
            "public-contract-snapshot" => Some(EvidenceKind::PublicContractSnapshot),
            "differential-execution" => Some(EvidenceKind::DifferentialExecution),
            _ => None,
        }
    }
}

impl fmt::Display for EvidenceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A value rejected by [`AssertionId::new`] or [`ConditionId::new`] — both
/// share this one error shape since both validate the exact same pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DottedKebabIdError {
    Empty,
    InvalidFormat { value: String },
}

impl fmt::Display for DottedKebabIdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DottedKebabIdError::Empty => write!(f, "id is empty"),
            DottedKebabIdError::InvalidFormat { value } => write!(
                f,
                "\"{value}\" is not a valid id (expected lowercase alphanumeric segments separated by \"-\" or \".\", matching ^[a-z0-9]+([-.][a-z0-9]+)*$)"
            ),
        }
    }
}

impl std::error::Error for DottedKebabIdError {}

/// `^[a-z0-9]+([-.][a-z0-9]+)*$` (`functional-parity-evidence.schema.json`
/// `definitions.assertion.id`): one or more non-empty segments of lowercase
/// letters and digits, separated by a single `-` or `.`. A digit MAY lead a
/// segment and `.` IS a legal separator — both outside
/// [`crate::types::SemanticId`]'s own `^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$`
/// pattern, whose legal set does not coincide with this one; that
/// divergence is exactly why this contract owns its own id type rather than
/// reusing `SemanticId`.
fn is_valid_dotted_kebab(value: &str) -> bool {
    if value.is_empty() {
        return false;
    }
    value.split(['-', '.']).all(|segment| {
        !segment.is_empty()
            && segment
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    })
}

fn new_dotted_kebab_id(value: impl Into<String>) -> Result<String, DottedKebabIdError> {
    let value = value.into();
    if value.is_empty() {
        return Err(DottedKebabIdError::Empty);
    }
    if !is_valid_dotted_kebab(&value) {
        return Err(DottedKebabIdError::InvalidFormat { value });
    }
    Ok(value)
}

/// One preserved-contract assertion's, evidence `covers` entry's,
/// post-change contract link's, per-assertion verdict's, or gap
/// `assertion_ids` entry's identifier — the ONE id namespace all five of
/// those fields share and cross-reference against each other
/// (`functional-parity-evidence-contract.md` §7 rules 2-6).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssertionId(String);

impl AssertionId {
    pub fn new(value: impl Into<String>) -> Result<Self, DottedKebabIdError> {
        Ok(Self(new_dotted_kebab_id(value)?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AssertionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One baseline `inputs_and_conditions` entry's identifier, or a
/// post-change `baseline_condition_ids` reference to one — a DIFFERENT id
/// namespace from [`AssertionId`] even though both happen to validate the
/// same pattern: a baseline condition id and an assertion id are never
/// cross-referenced against each other by any of this contract's rules, so
/// keeping them as two distinct Rust types prevents one from ever being
/// passed where the other is expected.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConditionId(String);

impl ConditionId {
    pub fn new(value: impl Into<String>) -> Result<Self, DottedKebabIdError> {
        Ok(Self(new_dotted_kebab_id(value)?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConditionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A value rejected by [`EvidenceText::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceTextError {
    Empty,
}

impl fmt::Display for EvidenceTextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvidenceTextError::Empty => write!(f, "value is empty"),
        }
    }
}

impl std::error::Error for EvidenceTextError {}

/// Free text governed ONLY by this schema's own
/// `{"type": "string", "minLength": 1}` constraint —
/// `definitions.assertion.statement`, `definitions.state_ref.identifier`/
/// `identifier_kind`, `definitions.identified_condition.description`/`kind`,
/// `definitions.condition.description`/`kind`,
/// `definitions.observed_result.summary`/`artifacts[]`,
/// `definitions.evidence_entry.observed_scope`/`limitations[]`/
/// `applicability_justification`, `definitions.gap.description`,
/// `definitions.assertion_verdict.unverified_reason`,
/// `definitions.record.verdict.overall_unverified_reason`,
/// `definitions.record.work_item`/`recorded_at`, and
/// `baseline.provenance.method`/
/// `post_change_evidence.inputs_and_conditions.comparability_justification`.
///
/// Deliberately NOT [`crate::types::NonEmptyString`]: JSON Schema's
/// `minLength` counts UTF-16 code units and does not exclude a
/// whitespace-only value, while `NonEmptyString::new` rejects one
/// (`NonEmptyStringError::Blank`) — reusing it here would reject a value
/// BOTH the schema and the Node.js reference (a plain, unvalidated property
/// read, `String(x ?? '')`) accept, an unauthorized behavioural difference
/// this package's own scope does not license
/// (`governance/plans/meridian-rust-migration-program-plan.md` §5.18.3
/// point 2). `EvidenceText::new` therefore checks ONLY non-emptiness,
/// matching `minLength: 1` exactly — see
/// `tests::whitespace_only_value_is_accepted_matching_minlength_one_not_nonemptystring`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceText(String);

impl EvidenceText {
    pub fn new(value: impl Into<String>) -> Result<Self, EvidenceTextError> {
        let value = value.into();
        if value.is_empty() {
            Err(EvidenceTextError::Empty)
        } else {
            Ok(Self(value))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EvidenceText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// An identifiable state of the source (`definitions.state_ref`):
/// `identifier_kind` names what kind of identifier `identifier` is, both in
/// neutral terms. The baseline's and the post-change's own `SourceStateRef`
/// must be a distinguishable pair (record rule 11,
/// [`crate::functional_parity::checks`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceStateRef {
    pub identifier: EvidenceText,
    pub identifier_kind: EvidenceText,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assertion_id_accepts_a_dotted_and_a_hyphenated_id() {
        assert!(AssertionId::new("api.exports").is_ok());
        assert!(AssertionId::new("fx-outbound-calls").is_ok());
        assert!(AssertionId::new("a1.b2-c3").is_ok());
    }

    #[test]
    fn assertion_id_accepts_a_leading_digit_unlike_semantic_id() {
        // `SemanticId` rejects a leading digit; this contract's own pattern
        // does not — proof the two types' legal sets genuinely diverge.
        assert!(AssertionId::new("1st-check").is_ok());
        assert!(crate::types::SemanticId::new("1st-check").is_err());
    }

    #[test]
    fn assertion_id_rejects_empty_and_malformed_values() {
        assert_eq!(AssertionId::new("").unwrap_err(), DottedKebabIdError::Empty);
        assert!(AssertionId::new(".leading-dot").is_err());
        assert!(AssertionId::new("trailing-dot.").is_err());
        assert!(AssertionId::new("double--hyphen").is_err());
        assert!(AssertionId::new("Uppercase").is_err());
        assert!(AssertionId::new("has space").is_err());
    }

    #[test]
    fn condition_id_and_assertion_id_are_distinct_types() {
        // Compile-time proof only: this test exists so the module keeps
        // exercising both constructors, not to assert anything a type
        // checker cannot already guarantee.
        let a = AssertionId::new("api.exports").unwrap();
        let c = ConditionId::new("cond.inputs").unwrap();
        assert_eq!(a.as_str(), "api.exports");
        assert_eq!(c.as_str(), "cond.inputs");
    }

    #[test]
    fn source_state_ref_equality_matches_identifier_and_kind() {
        let a = SourceStateRef {
            identifier: EvidenceText::new("state-A").unwrap(),
            identifier_kind: EvidenceText::new("described-source-revision").unwrap(),
        };
        let b = SourceStateRef {
            identifier: EvidenceText::new("state-A").unwrap(),
            identifier_kind: EvidenceText::new("described-source-revision").unwrap(),
        };
        let c = SourceStateRef {
            identifier: EvidenceText::new("state-B").unwrap(),
            identifier_kind: EvidenceText::new("described-source-revision").unwrap(),
        };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    /// Regression for corrective round item 2: this schema's own
    /// `minLength: 1` — and the Node.js reference's plain, unvalidated
    /// property read — accept a whitespace-only value; `NonEmptyString`
    /// would incorrectly reject it, which is exactly the unauthorized
    /// divergence this type exists to avoid.
    #[test]
    fn whitespace_only_value_is_accepted_matching_minlength_one_not_nonemptystring() {
        assert!(EvidenceText::new("   ").is_ok());
        assert!(crate::types::NonEmptyString::new("   ").is_err());
        assert_eq!(EvidenceText::new("   ").unwrap().as_str(), "   ");
    }

    #[test]
    fn empty_value_is_rejected() {
        assert_eq!(EvidenceText::new("").unwrap_err(), EvidenceTextError::Empty);
    }
}
