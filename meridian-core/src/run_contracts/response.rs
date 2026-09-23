//! The typed shape of a transformer response — the one representation every
//! family that resolves pinned records through the external boundary
//! (`bounded-context-manifest`, `evidence-and-handoff-contract`,
//! `meridian-field-evaluation`) reads.
//!
//! A resolver response is NOT schema-checked before it reaches this crate:
//! its closed contract IS the families' domain rule. So a response field is
//! a [`ResponseField`] — absent, a value of the expected shape, or a
//! [`ForeignValue`] of some other JSON kind that is kept (kind plus its
//! rendered text) only so a diagnostic can name it. Nothing here is a
//! general JSON value: every field has its own expected shape, and unknown
//! keys are carried only by name.
//!
//! The same field shapes also carry a record's own measurement into the ONE
//! measurement rule of `meridian-field-evaluation`
//! ([`crate::field_evaluation::measurement`]), so a record's measurement and
//! a resolved observation's measurement are checked by one rule, not two
//! copies (`scripts/lib/field-evaluation.mjs`'s `measurementIssues`).

use super::json_quote;

/// The JSON kind of a [`ForeignValue`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResponseValueKind {
    Null,
    Boolean,
    Number,
    String,
    Array,
    Object,
}

impl ResponseValueKind {
    /// JavaScript `typeof` for this kind.
    pub(crate) fn type_of(self) -> &'static str {
        match self {
            ResponseValueKind::Null | ResponseValueKind::Array | ResponseValueKind::Object => {
                "object"
            }
            ResponseValueKind::Boolean => "boolean",
            ResponseValueKind::Number => "number",
            ResponseValueKind::String => "string",
        }
    }

    /// `null`, else `typeof` — how a non-string response value is named.
    pub(crate) fn null_or_type_of(self) -> &'static str {
        match self {
            ResponseValueKind::Null => "null",
            other => other.type_of(),
        }
    }

    pub(crate) fn with_article(self) -> &'static str {
        match self {
            ResponseValueKind::Null => "null",
            ResponseValueKind::Boolean => "a boolean",
            ResponseValueKind::Number => "a number",
            ResponseValueKind::String => "a string",
            ResponseValueKind::Array => "an array",
            ResponseValueKind::Object => "an object",
        }
    }
}

/// A response value of an unexpected JSON kind, kept only for its
/// diagnostic: its kind, its JSON text (`JSON.stringify`) and its string
/// coercion (`String(x)` / template interpolation).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ForeignValue {
    kind: ResponseValueKind,
    json_text: String,
    string_coercion: String,
}

impl ForeignValue {
    pub fn new(
        kind: ResponseValueKind,
        json_text: impl Into<String>,
        string_coercion: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            json_text: json_text.into(),
            string_coercion: string_coercion.into(),
        }
    }

    pub fn null() -> Self {
        Self::new(ResponseValueKind::Null, "null", "null")
    }

    pub fn kind(&self) -> ResponseValueKind {
        self.kind
    }
}

/// One field of a resolver response (or of a measurement).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ResponseField<T> {
    #[default]
    Absent,
    Present(T),
    Foreign(ForeignValue),
}

impl<T> ResponseField<T> {
    pub(crate) fn is_present(&self) -> bool {
        !matches!(self, ResponseField::Absent)
    }

    pub(crate) fn value(&self) -> Option<&T> {
        match self {
            ResponseField::Present(v) => Some(v),
            _ => None,
        }
    }
}

/// How a value renders in a diagnostic.
pub(crate) trait ResponseText {
    /// `JSON.stringify(x)`.
    fn json(&self) -> String;
    /// `String(x)`.
    fn coerced(&self) -> String;
}

impl ResponseText for String {
    fn json(&self) -> String {
        json_quote(self)
    }
    fn coerced(&self) -> String {
        self.clone()
    }
}

impl ResponseText for u64 {
    fn json(&self) -> String {
        self.to_string()
    }
    fn coerced(&self) -> String {
        self.to_string()
    }
}

impl ResponseText for JsonNumber {
    fn json(&self) -> String {
        self.text.clone()
    }
    fn coerced(&self) -> String {
        self.text.clone()
    }
}

impl<T: ResponseText> ResponseText for Option<T> {
    fn json(&self) -> String {
        self.as_ref().map_or_else(|| "null".to_string(), T::json)
    }
    fn coerced(&self) -> String {
        self.as_ref().map_or_else(|| "null".to_string(), T::coerced)
    }
}

// Every method of this block is crate-private; the bound only selects
// which field shapes render, it is not a public contract.
#[allow(private_bounds)]
impl<T: ResponseText> ResponseField<T> {
    /// `JSON.stringify(x ?? null)`.
    pub(crate) fn json_or_null(&self) -> String {
        match self {
            ResponseField::Absent => "null".to_string(),
            ResponseField::Present(v) => v.json(),
            ResponseField::Foreign(f) => f.json_text.clone(),
        }
    }

    /// `JSON.stringify(x)` — the literal `undefined` for an absent field.
    pub(crate) fn json_or_undefined(&self) -> String {
        match self {
            ResponseField::Absent => "undefined".to_string(),
            other => other.json_or_null(),
        }
    }

    /// `` `${x ?? null}` ``.
    pub(crate) fn string_or_null(&self) -> String {
        match self {
            ResponseField::Absent => "null".to_string(),
            ResponseField::Present(v) => v.coerced(),
            ResponseField::Foreign(f) => f.string_coercion.clone(),
        }
    }

    /// `` `${x}` `` — the literal `undefined` for an absent field.
    pub(crate) fn string_or_undefined(&self) -> String {
        match self {
            ResponseField::Absent => "undefined".to_string(),
            other => other.string_or_null(),
        }
    }
}

impl ResponseField<String> {
    pub(crate) fn text(&self) -> Option<&str> {
        self.value().map(String::as_str)
    }

    pub(crate) fn non_blank(&self) -> Option<&str> {
        self.text().filter(|s| !s.trim().is_empty())
    }
}

impl<T> ResponseField<Option<T>> {
    /// `x != null` — present and not the explicit JSON `null`.
    pub(crate) fn is_non_null(&self) -> bool {
        matches!(
            self,
            ResponseField::Present(Some(_)) | ResponseField::Foreign(_)
        )
    }
}

/// One element of a response array.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseItem {
    Text(String),
    Foreign(ForeignValue),
}

impl ResponseItem {
    pub(crate) fn text(&self) -> Option<&str> {
        match self {
            ResponseItem::Text(s) => Some(s),
            ResponseItem::Foreign(_) => None,
        }
    }
}

/// A JSON number as the transport read it: its value and its JSON text.
/// JSON has no `NaN`/infinity, so `value` is always finite.
#[derive(Debug, Clone, PartialEq)]
pub struct JsonNumber {
    value: f64,
    text: String,
}

impl JsonNumber {
    /// `None` for a non-finite value — never a JSON number.
    pub fn new(value: f64, text: impl Into<String>) -> Option<Self> {
        value.is_finite().then(|| Self {
            value,
            text: text.into(),
        })
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    /// `Number.isInteger(x)`.
    pub(crate) fn is_integer(&self) -> bool {
        self.value.fract() == 0.0
    }
}

impl Eq for JsonNumber {}

/// A measurement's fields — a record's own `payload.measurement` (every
/// field `Present`, the schema already applied) or a resolved observation's
/// `measurement` (any field may be absent or foreign). The one measurement
/// rule reads this shape (`crate::field_evaluation::measurement`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MeasurementFields {
    pub kind: ResponseField<String>,
    pub outcome: ResponseField<String>,
    pub basis: ResponseField<String>,
    pub duration_kind: ResponseField<String>,
    pub unit: ResponseField<String>,
    pub seconds: ResponseField<JsonNumber>,
    pub paused_seconds: ResponseField<JsonNumber>,
    pub start_ts: ResponseField<String>,
    pub end_ts: ResponseField<String>,
    pub value: ResponseField<JsonNumber>,
}

/// An observation window's two dates.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WindowFields {
    pub start_date: ResponseField<String>,
    pub end_date: ResponseField<String>,
}

/// The `task-specification` extension of a response: the minimal machine
/// lists a handoff closes its coverage against.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SpecificationResponse {
    pub acceptance_criteria: ResponseField<Vec<ResponseItem>>,
    pub mandatory_checks: ResponseField<Vec<ResponseItem>>,
}

/// The `evidence-result` extension of a response: what the transformer
/// confirms an evidence artefact shows, and about which subject.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EvidenceResultResponse {
    pub observed_result: ResponseField<String>,
    pub covers: ResponseField<Vec<ResponseItem>>,
    pub check_ref: ResponseField<String>,
    pub specialised_contract: ResponseField<String>,
    pub recorded_verdict: ResponseField<String>,
    pub metric_ref: ResponseField<String>,
}

/// The `field-evaluation-observation` extension of a response: the subject
/// fields a report validates before it counts anything. A `null`
/// `measurement`/`supersedes_id`/`observation_period`/`coverage` is
/// `Present(None)`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObservationResponse {
    pub metric_id: ResponseField<String>,
    pub status: ResponseField<String>,
    pub measurement: ResponseField<Option<MeasurementFields>>,
    pub workspace_id: ResponseField<String>,
    pub observed_at: ResponseField<String>,
    pub supersedes_id: ResponseField<Option<String>>,
    pub observation_period: ResponseField<Option<WindowFields>>,
    pub coverage: ResponseField<Option<String>>,
}

/// Every key a response may carry beyond [`super::RESOLVED_ENTRY_COMMON_KEYS`],
/// by the record kinds that declare it. Which of them a given response may
/// carry is the family's closed key set (`super::resolution::allowed_keys`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ExtensionKey {
    ResolvedState,
    LinkedRunRef,
    AcceptanceCriteria,
    MandatoryChecks,
    ObservedResult,
    Covers,
    CheckRef,
    SpecialisedContract,
    RecordedVerdict,
    MetricRef,
    MetricId,
    Status,
    Measurement,
    WorkspaceId,
    ObservedAt,
    SupersedesId,
    ObservationPeriod,
    Coverage,
}

impl ExtensionKey {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            ExtensionKey::ResolvedState => "resolved_state",
            ExtensionKey::LinkedRunRef => "linked_run_ref",
            ExtensionKey::AcceptanceCriteria => "acceptance_criteria",
            ExtensionKey::MandatoryChecks => "mandatory_checks",
            ExtensionKey::ObservedResult => "observed_result",
            ExtensionKey::Covers => "covers",
            ExtensionKey::CheckRef => "check_ref",
            ExtensionKey::SpecialisedContract => "specialised_contract",
            ExtensionKey::RecordedVerdict => "recorded_verdict",
            ExtensionKey::MetricRef => "metric_ref",
            ExtensionKey::MetricId => "metric_id",
            ExtensionKey::Status => "status",
            ExtensionKey::Measurement => "measurement",
            ExtensionKey::WorkspaceId => "workspace_id",
            ExtensionKey::ObservedAt => "observed_at",
            ExtensionKey::SupersedesId => "supersedes_id",
            ExtensionKey::ObservationPeriod => "observation_period",
            ExtensionKey::Coverage => "coverage",
        }
    }
}

/// Every key name the typed response knows, so the transport can keep
/// everything else by name only.
pub const KNOWN_RESPONSE_KEYS: [&str; 24] = [
    "record_type",
    "id",
    "reference",
    "revision",
    "content_digest",
    "source_bytes",
    ExtensionKey::ResolvedState.as_str(),
    ExtensionKey::LinkedRunRef.as_str(),
    ExtensionKey::AcceptanceCriteria.as_str(),
    ExtensionKey::MandatoryChecks.as_str(),
    ExtensionKey::ObservedResult.as_str(),
    ExtensionKey::Covers.as_str(),
    ExtensionKey::CheckRef.as_str(),
    ExtensionKey::SpecialisedContract.as_str(),
    ExtensionKey::RecordedVerdict.as_str(),
    ExtensionKey::MetricRef.as_str(),
    ExtensionKey::MetricId.as_str(),
    ExtensionKey::Status.as_str(),
    ExtensionKey::Measurement.as_str(),
    ExtensionKey::WorkspaceId.as_str(),
    ExtensionKey::ObservedAt.as_str(),
    ExtensionKey::SupersedesId.as_str(),
    ExtensionKey::ObservationPeriod.as_str(),
    ExtensionKey::Coverage.as_str(),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_field_renders_like_the_node_reference_interpolates_it() {
        let absent: ResponseField<String> = ResponseField::Absent;
        assert_eq!(absent.json_or_null(), "null");
        assert_eq!(absent.json_or_undefined(), "undefined");
        assert_eq!(absent.string_or_undefined(), "undefined");
        let text = ResponseField::Present("a\"b".to_string());
        assert_eq!(text.json_or_undefined(), "\"a\\\"b\"");
        assert_eq!(text.string_or_null(), "a\"b");
        let foreign: ResponseField<String> =
            ResponseField::Foreign(ForeignValue::new(ResponseValueKind::Number, "7", "7"));
        assert_eq!(foreign.json_or_undefined(), "7");
        let null: ResponseField<Option<String>> = ResponseField::Present(None);
        assert!(!null.is_non_null());
        assert_eq!(null.json_or_null(), "null");
    }

    #[test]
    fn a_json_number_is_finite_and_knows_whether_it_is_an_integer() {
        assert!(JsonNumber::new(f64::NAN, "NaN").is_none());
        assert!(JsonNumber::new(2.0, "2.0").unwrap().is_integer());
        assert!(!JsonNumber::new(2.5, "2.5").unwrap().is_integer());
    }
}
