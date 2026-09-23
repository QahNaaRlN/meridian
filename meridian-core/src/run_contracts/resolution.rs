//! The external record-resolution boundary of the context manifest, typed.
//!
//! A pinned reference is resolved OUTSIDE the manifest: a
//! [`ResolutionCatalogue`] maps a portable reference to the
//! [`ResolvedEntry`] a transformer returned for it. Unlike a record, a
//! resolver response is NOT schema-checked before it reaches this crate —
//! the closed transformer-response contract IS this module's domain rule —
//! so a response field is a [`ResponseField`]: absent, a value of the
//! expected shape, or a [`ForeignValue`] of some other JSON kind that is
//! kept (kind plus its rendered text) only so the diagnostic can name it.
//! Nothing here is a general JSON value: every field has its own expected
//! shape, and unknown keys are carried only by name.
//!
//! `check_resolved_entry` is `resolvePinnedReference`; [`ResolvedStateField`]
//! is the closed vocabulary of the checkpoint axes an execution-run
//! response carries (`resolved_state`), and `check_resolved_state` its
//! closed contract. `same_resolved_edition` compares two resolved
//! editions of one slot.

use std::collections::{BTreeMap, HashSet};

use crate::task_contracts::non_portable_reason;
use crate::types::{ContentDigest, Diagnostic, SemanticId};

use super::identity::RecordText;
use super::pinned_ref::{site_label, PinSite, PinnedRecordKind, PinnedRef, PinnedSlot};
use super::revision::{is_sha256_text, RevisionClass};
use super::vocabulary::{LifecycleStage, WorkStatus};
use super::{fail, json_quote};

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
    fn type_of(self) -> &'static str {
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
    fn null_or_type_of(self) -> &'static str {
        match self {
            ResponseValueKind::Null => "null",
            other => other.type_of(),
        }
    }

    fn with_article(self) -> &'static str {
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

/// One field of a resolver response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseField<T> {
    Absent,
    Present(T),
    Foreign(ForeignValue),
}

impl<T> ResponseField<T> {
    fn is_present(&self) -> bool {
        !matches!(self, ResponseField::Absent)
    }

    fn value(&self) -> Option<&T> {
        match self {
            ResponseField::Present(v) => Some(v),
            _ => None,
        }
    }
}

impl ResponseField<String> {
    fn text(&self) -> Option<&str> {
        self.value().map(String::as_str)
    }

    fn non_blank(&self) -> Option<&str> {
        self.text().filter(|s| !s.trim().is_empty())
    }

    /// `JSON.stringify(x ?? null)`.
    fn json_or_null(&self) -> String {
        match self {
            ResponseField::Absent => "null".to_string(),
            ResponseField::Present(s) => json_quote(s),
            ResponseField::Foreign(f) => f.json_text.clone(),
        }
    }

    /// `` `${x ?? null}` ``.
    fn string_or_null(&self) -> String {
        match self {
            ResponseField::Absent => "null".to_string(),
            ResponseField::Present(s) => s.clone(),
            ResponseField::Foreign(f) => f.string_coercion.clone(),
        }
    }
}

/// One element of a response array.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseItem {
    Text(String),
    Foreign(ForeignValue),
}

/// The closed vocabulary of the axes an execution-run response's
/// `resolved_state` carries — every one required, nothing else allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResolvedStateField {
    TaskSpecificationRef,
    ScopeRevision,
    LifecycleStage,
    WorkStatus,
    NextAction,
    NextGate,
    BlockerIds,
    ResolvedNorms,
    CompletedChecks,
}

impl ResolvedStateField {
    pub const ALL: [ResolvedStateField; 9] = [
        ResolvedStateField::TaskSpecificationRef,
        ResolvedStateField::ScopeRevision,
        ResolvedStateField::LifecycleStage,
        ResolvedStateField::WorkStatus,
        ResolvedStateField::NextAction,
        ResolvedStateField::NextGate,
        ResolvedStateField::BlockerIds,
        ResolvedStateField::ResolvedNorms,
        ResolvedStateField::CompletedChecks,
    ];

    /// [`Self::ALL`]'s names, in order.
    pub const NAMES: [&'static str; 9] = [
        ResolvedStateField::TaskSpecificationRef.as_str(),
        ResolvedStateField::ScopeRevision.as_str(),
        ResolvedStateField::LifecycleStage.as_str(),
        ResolvedStateField::WorkStatus.as_str(),
        ResolvedStateField::NextAction.as_str(),
        ResolvedStateField::NextGate.as_str(),
        ResolvedStateField::BlockerIds.as_str(),
        ResolvedStateField::ResolvedNorms.as_str(),
        ResolvedStateField::CompletedChecks.as_str(),
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            ResolvedStateField::TaskSpecificationRef => "task_specification_ref",
            ResolvedStateField::ScopeRevision => "scope_revision",
            ResolvedStateField::LifecycleStage => "lifecycle_stage",
            ResolvedStateField::WorkStatus => "work_status",
            ResolvedStateField::NextAction => "next_action",
            ResolvedStateField::NextGate => "next_gate",
            ResolvedStateField::BlockerIds => "blocker_ids",
            ResolvedStateField::ResolvedNorms => "resolved_norms",
            ResolvedStateField::CompletedChecks => "completed_checks",
        }
    }
}

/// The key set common to every resolved entry, in the contract's order.
pub const RESOLVED_ENTRY_COMMON_KEYS: [&str; 6] = [
    "record_type",
    "id",
    "reference",
    "revision",
    "content_digest",
    "source_bytes",
];

/// An execution-run response's `resolved_state`. `scope_revision` is
/// `Present` for a JSON integer representable as `u64` (a zero is still a
/// domain rejection); `next_action`/`next_gate` are `Present(None)` for an
/// explicit `null`. `unknown_fields` are the names of any other keys, in
/// sorted order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedStateResponse {
    pub task_specification_ref: ResponseField<String>,
    pub scope_revision: ResponseField<u64>,
    pub lifecycle_stage: ResponseField<String>,
    pub work_status: ResponseField<String>,
    pub next_action: ResponseField<Option<String>>,
    pub next_gate: ResponseField<Option<String>>,
    pub blocker_ids: ResponseField<Vec<ResponseItem>>,
    pub resolved_norms: ResponseField<Vec<ResponseItem>>,
    pub completed_checks: ResponseField<Vec<ResponseItem>>,
    pub unknown_fields: Vec<String>,
}

/// One transformer response. `other_fields` are the names of keys outside
/// the eight this contract knows, in sorted order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedEntry {
    pub record_type: ResponseField<String>,
    pub id: ResponseField<String>,
    pub reference: ResponseField<String>,
    pub revision: ResponseField<String>,
    pub content_digest: ResponseField<String>,
    pub source_bytes: ResponseField<String>,
    pub resolved_state: ResponseField<ResolvedStateResponse>,
    pub linked_run_ref: ResponseField<String>,
    pub other_fields: Vec<String>,
}

/// The typed resolution input: portable reference -> the one response a
/// transformer returned for it. A reference with no entry does not resolve.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolutionCatalogue {
    entries: BTreeMap<String, ResolvedEntry>,
}

impl ResolutionCatalogue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, reference: impl Into<String>, entry: ResolvedEntry) {
        self.entries.insert(reference.into(), entry);
    }

    pub fn resolve(&self, reference: &str) -> Option<&ResolvedEntry> {
        self.entries.get(reference)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The SHA-256 a response confirms for its edition (`confirmedDigest`):
/// computed from `source_bytes` when present — cross-checked against a
/// stated well-formed `content_digest` — else that stated digest.
struct ConfirmedDigest {
    digest: Option<String>,
    inconsistent: Option<String>,
}

fn confirmed_digest(entry: &ResolvedEntry) -> ConfirmedDigest {
    let stated = entry.content_digest.text().filter(|s| is_sha256_text(s));
    if let Some(bytes) = entry.source_bytes.text() {
        let computed = ContentDigest::of_str(bytes).value().to_string();
        let inconsistent = stated
            .filter(|s| s.to_lowercase() != computed.to_lowercase())
            .map(str::to_string);
        return ConfirmedDigest {
            digest: Some(computed),
            inconsistent,
        };
    }
    ConfirmedDigest {
        digest: stated.map(str::to_string),
        inconsistent: None,
    }
}

fn allowed_keys(wanted: PinnedRecordKind) -> Vec<&'static str> {
    let mut keys = RESOLVED_ENTRY_COMMON_KEYS.to_vec();
    match wanted {
        PinnedRecordKind::ExecutionRun => keys.push("resolved_state"),
        PinnedRecordKind::RunHumanControl => keys.push("linked_run_ref"),
        PinnedRecordKind::TaskSpecification => {}
    }
    keys
}

/// Every key of `entry` outside `wanted`'s closed key set, sorted — the
/// deterministic order the Rust port has always reported
/// (`COMPATIBILITY.md`: Node names them in source-text order).
fn unknown_entry_keys(entry: &ResolvedEntry, wanted: PinnedRecordKind) -> Vec<String> {
    let mut unknown = entry.other_fields.clone();
    if entry.resolved_state.is_present() && wanted != PinnedRecordKind::ExecutionRun {
        unknown.push("resolved_state".to_string());
    }
    if entry.linked_run_ref.is_present() && wanted != PinnedRecordKind::RunHumanControl {
        unknown.push("linked_run_ref".to_string());
    }
    unknown.sort();
    unknown.dedup();
    unknown
}

/// Port of `resolvePinnedReference`: resolves one pinned reference through
/// `catalogue` and checks the response against the closed
/// transformer-response contract. Returns the response even when it has
/// problems — the edition and link comparisons still read it.
pub(crate) fn check_resolved_entry<'c>(
    id: &str,
    site: PinSite,
    slot: PinnedSlot,
    pin: &PinnedRef,
    catalogue: &'c ResolutionCatalogue,
    problems: &mut Vec<Diagnostic>,
) -> Option<&'c ResolvedEntry> {
    let field = site_label(site, slot);
    let wanted = slot.kind();
    let want = wanted.as_str();
    let Some(entry) = catalogue.resolve(pin.reference.as_str()) else {
        problems.push(fail(format!(
            "context manifest \"{id}\" {field} does not resolve to an actual {want} through the external resolver; a pinned reference that resolves to nothing is not a verified pin and the manifest fails closed"
        )));
        return None;
    };

    let allowed = allowed_keys(wanted).join(", ");
    for key in unknown_entry_keys(entry, wanted) {
        problems.push(fail(format!(
            "context manifest \"{id}\" {field}: the resolver returned a record with an unknown field \"{key}\"; the transformer response is closed to {{ {allowed} }}"
        )));
    }

    match entry.record_type.text().filter(|s| !s.is_empty()) {
        None => problems.push(fail(format!(
            "context manifest \"{id}\" {field}: the resolver returned a record with no record_type; the transformer response is a closed contract"
        ))),
        Some(rt) if rt != want => problems.push(fail(format!(
            "context manifest \"{id}\" {field} resolves to a \"{rt}\" record, not \"{want}\""
        ))),
        Some(_) => {}
    }

    match entry.id.non_blank() {
        None => problems.push(fail(format!(
            "context manifest \"{id}\" {field}: the resolver returned a {want} with no id; a resolved record without an identity cannot be checked against the pin"
        ))),
        Some(eid) if SemanticId::new(eid).is_err() => problems.push(fail(format!(
            "context manifest \"{id}\" {field}: the resolver returned a {want} whose id \"{eid}\" is not a stable semantic identifier"
        ))),
        Some(eid) if eid != pin.id.as_str() => problems.push(fail(format!(
            "context manifest \"{id}\" {field} pins id \"{}\" but the reference resolves to record id \"{eid}\"",
            pin.id.as_str()
        ))),
        Some(_) => {}
    }

    if entry.reference.is_present() {
        match entry.reference.non_blank() {
            None => problems.push(fail(format!(
                "context manifest \"{id}\" {field}: the resolver's stated reference is present but not a non-empty string"
            ))),
            Some(er) => {
                if let Some(r) = non_portable_reason(Some(er)) {
                    problems.push(fail(format!(
                        "context manifest \"{id}\" {field}: the resolver's stated reference contains {r}"
                    )));
                } else if er != pin.reference.as_str() {
                    problems.push(fail(format!(
                        "context manifest \"{id}\" {field}: the resolver's stated reference \"{er}\" is not the resolved reference \"{}\"",
                        pin.reference
                    )));
                }
            }
        }
    }

    if entry.content_digest.is_present() && !entry.content_digest.text().is_some_and(is_sha256_text)
    {
        problems.push(fail(format!(
            "context manifest \"{id}\" {field}: the resolver's content_digest {} is not exactly 64 hexadecimal characters",
            entry.content_digest.json_or_null()
        )));
    }
    if let ResponseField::Foreign(f) = &entry.source_bytes {
        problems.push(fail(format!(
            "context manifest \"{id}\" {field}: the resolver's source_bytes is {}, not a string",
            f.kind.null_or_type_of()
        )));
    }

    let revision_exact = check_response_revision(id, &field, &entry.revision, problems);
    let confirmed = confirmed_digest(entry);
    if !revision_exact && confirmed.digest.is_none() {
        problems.push(fail(format!(
            "context manifest \"{id}\" {field}: the resolver confirmed neither an exact revision nor a content digest for this edition; the pinned edition is unverified"
        )));
    }
    if let Some(inconsistent) = &confirmed.inconsistent {
        problems.push(fail(format!(
            "context manifest \"{id}\" {field}: the resolver's content_digest \"{inconsistent}\" does not match the SHA-256 of the resolved source bytes \"{}\"",
            confirmed.digest.as_deref().unwrap_or_default()
        )));
    }
    if let Some(pin_rev) = pin.revision.as_ref().filter(|r| !r.is_blank()) {
        match entry.revision.non_blank() {
            None => problems.push(fail(format!(
                "context manifest \"{id}\" {field} pins revision \"{pin_rev}\" but the resolver confirmed no exact edition for this reference"
            ))),
            Some(entry_rev) if entry_rev != pin_rev.as_str() => problems.push(fail(format!(
                "context manifest \"{id}\" {field} pins revision \"{pin_rev}\" but the resolver confirmed edition \"{entry_rev}\""
            ))),
            Some(_) => {}
        }
    }
    if let Some(pin_sha) = &pin.sha256 {
        match &confirmed.digest {
            None => problems.push(fail(format!(
                "context manifest \"{id}\" {field} pins sha256 \"{pin_sha}\" but the resolver confirmed no content digest for this edition; a digest is verified against resolved source, never against a second copy of itself"
            ))),
            Some(d) if d.to_lowercase() != pin_sha.as_str().to_lowercase() => problems.push(fail(format!(
                "context manifest \"{id}\" {field} pins sha256 \"{pin_sha}\" but the SHA-256 of the resolved source is \"{d}\""
            ))),
            Some(_) => {}
        }
    }

    match wanted {
        PinnedRecordKind::ExecutionRun => {
            check_resolved_state(id, &field, &entry.resolved_state, problems)
        }
        PinnedRecordKind::RunHumanControl => match entry.linked_run_ref.non_blank() {
            None => problems.push(fail(format!(
                "context manifest \"{id}\" {field}: the resolver returned a run-human-control record with no linked_run_ref; the run it belongs to is unconfirmed"
            ))),
            Some(linked) => {
                if let Some(r) = non_portable_reason(Some(linked)) {
                    problems.push(fail(format!(
                        "context manifest \"{id}\" {field}: the resolved run-human-control record's linked_run_ref contains {r}"
                    )));
                }
            }
        },
        PinnedRecordKind::TaskSpecification => {}
    }

    Some(entry)
}

/// The response's own revision; `true` when it is an exact edition.
fn check_response_revision(
    id: &str,
    field: &str,
    revision: &ResponseField<String>,
    problems: &mut Vec<Diagnostic>,
) -> bool {
    match revision {
        ResponseField::Absent => false,
        ResponseField::Foreign(f) => {
            problems.push(fail(format!(
                "context manifest \"{id}\" {field}: the resolver's revision is {}, not a string",
                f.kind.with_article()
            )));
            false
        }
        ResponseField::Present(s) if s.trim().is_empty() => {
            problems.push(fail(format!(
                "context manifest \"{id}\" {field}: the resolver's revision is present but empty"
            )));
            false
        }
        ResponseField::Present(s) => {
            if RevisionClass::classify(Some(s)) == RevisionClass::Exact {
                true
            } else {
                problems.push(fail(format!(
                    "context manifest \"{id}\" {field}: the resolver confirmed revision {}, which is not an exact edition (a full Git SHA, a strict v?X.Y.Z tag or a SHA-256 digest); the transformer confirms a fixed edition, not a moving reference",
                    json_quote(s)
                )));
                false
            }
        }
    }
}

/// The closed contract of an execution-run response's `resolved_state`
/// (`checkResolvedState`).
fn check_resolved_state(
    id: &str,
    field: &str,
    state: &ResponseField<ResolvedStateResponse>,
    problems: &mut Vec<Diagnostic>,
) {
    let ResponseField::Present(rs) = state else {
        problems.push(fail(format!(
            "context manifest \"{id}\" {field}: the resolver returned an execution-run record with no resolved_state; the checkpoint axes cannot be checked against the run's own state"
        )));
        return;
    };
    let subject = format!(
        "context manifest \"{id}\" {field}: the resolved execution-run record's resolved_state"
    );
    let present = [
        rs.task_specification_ref.is_present(),
        rs.scope_revision.is_present(),
        rs.lifecycle_stage.is_present(),
        rs.work_status.is_present(),
        rs.next_action.is_present(),
        rs.next_gate.is_present(),
        rs.blocker_ids.is_present(),
        rs.resolved_norms.is_present(),
        rs.completed_checks.is_present(),
    ];
    for (axis, is_present) in ResolvedStateField::ALL.iter().zip(present) {
        if !is_present {
            problems.push(fail(format!(
                "{subject} is missing required field \"{}\"",
                axis.as_str()
            )));
        }
    }
    for key in &rs.unknown_fields {
        problems.push(fail(format!(
            "{subject} carries an unknown field \"{key}\"; resolved_state is closed to its {} declared axes",
            ResolvedStateField::ALL.len()
        )));
    }

    if rs.task_specification_ref.is_present() {
        match rs.task_specification_ref.non_blank() {
            None => problems.push(fail(format!(
                "{subject}.task_specification_ref is not a non-empty string"
            ))),
            Some(r) => {
                if let Some(reason) = non_portable_reason(Some(r)) {
                    problems.push(fail(format!(
                        "{subject}.task_specification_ref contains {reason}"
                    )));
                }
            }
        }
    }
    match &rs.scope_revision {
        ResponseField::Present(n) if *n > 0 => {}
        ResponseField::Absent => {}
        other => problems.push(fail(format!(
            "{subject}.scope_revision {} is not a positive integer",
            scope_revision_json(other)
        ))),
    }
    if rs.lifecycle_stage.is_present()
        && rs
            .lifecycle_stage
            .text()
            .and_then(LifecycleStage::parse)
            .is_none()
    {
        problems.push(fail(format!(
            "{subject}.lifecycle_stage {} is not one of the closed lifecycle pool",
            rs.lifecycle_stage.json_or_null()
        )));
    }
    if rs.work_status.is_present() && rs.work_status.text().and_then(WorkStatus::parse).is_none() {
        problems.push(fail(format!(
            "{subject}.work_status {} is not one of the closed work-status pool",
            rs.work_status.json_or_null()
        )));
    }
    for (name, step) in [
        ("next_action", &rs.next_action),
        ("next_gate", &rs.next_gate),
    ] {
        match step {
            ResponseField::Absent | ResponseField::Present(None) => {}
            ResponseField::Present(Some(s)) if !s.trim().is_empty() => {
                if let Some(r) = non_portable_reason(Some(s)) {
                    problems.push(fail(format!("{subject}.{name} contains {r}")));
                }
            }
            _ => problems.push(fail(format!(
                "{subject}.{name} is present but neither null nor a non-empty string"
            ))),
        }
    }
    for (name, items, ids) in [
        ("blocker_ids", &rs.blocker_ids, true),
        ("resolved_norms", &rs.resolved_norms, false),
        ("completed_checks", &rs.completed_checks, false),
    ] {
        check_resolved_state_array(&subject, name, items, ids, problems);
    }
}

fn scope_revision_json(value: &ResponseField<u64>) -> String {
    match value {
        ResponseField::Absent => "null".to_string(),
        ResponseField::Present(n) => n.to_string(),
        ResponseField::Foreign(f) => f.json_text.clone(),
    }
}

/// One `resolved_state` axis that must be an array of unique strings —
/// semantic ids (`ids`) or portable references.
fn check_resolved_state_array(
    subject: &str,
    name: &str,
    items: &ResponseField<Vec<ResponseItem>>,
    ids: bool,
    problems: &mut Vec<Diagnostic>,
) {
    let kind_label = if ids {
        "semantic-id"
    } else {
        "portable-reference"
    };
    let at = format!("{subject}.{name}");
    let items = match items {
        ResponseField::Absent => return,
        ResponseField::Foreign(f) => {
            problems.push(fail(format!(
                "{at} is {}, not an array of {kind_label} strings",
                f.kind.null_or_type_of()
            )));
            return;
        }
        ResponseField::Present(items) => items,
    };
    let mut seen = HashSet::new();
    for (i, item) in items.iter().enumerate() {
        let text = match item {
            ResponseItem::Text(s) if !s.trim().is_empty() => s,
            _ => {
                problems.push(fail(format!("{at}[{i}] is not a non-empty string")));
                continue;
            }
        };
        if !seen.insert(text.trim()) {
            problems.push(fail(format!("{at} repeats \"{text}\"")));
        }
        if ids {
            if SemanticId::new(text.as_str()).is_err() {
                problems.push(fail(format!(
                    "{at}[{i}] \"{text}\" is not a stable semantic id"
                )));
            }
        } else if let Some(r) = non_portable_reason(Some(text)) {
            problems.push(fail(format!("{at}[{i}] contains {r}")));
        }
    }
}

/// Port of `sameResolvedEdition`: the manifest's and the checkpoint's
/// reference for one slot, resolved separately, land on one identity,
/// record type, edition and digest.
pub(crate) fn same_resolved_edition(
    id: &str,
    slot: PinnedSlot,
    manifest: Option<&ResolvedEntry>,
    checkpoint: Option<&ResolvedEntry>,
    problems: &mut Vec<Diagnostic>,
) {
    let (Some(m), Some(c)) = (manifest, checkpoint) else {
        return;
    };
    let field = slot.field();
    if let (Some(mi), Some(ci)) = (m.id.text(), c.id.text()) {
        if mi != ci {
            problems.push(fail(format!(
                "context manifest \"{id}\" run_state_checkpoint.{field} resolves to record \"{ci}\", not the manifest's \"{mi}\"; the checkpoint pins the SAME record, not a parallel one that also exists"
            )));
        }
    }
    if m.record_type.json_or_null() != c.record_type.json_or_null() {
        problems.push(fail(format!(
            "context manifest \"{id}\" run_state_checkpoint.{field} resolves to a \"{}\" record, not the manifest's \"{}\"",
            c.record_type.string_or_null(),
            m.record_type.string_or_null()
        )));
    }
    let (m_rev, c_rev) = (m.revision.json_or_null(), c.revision.json_or_null());
    if m_rev != c_rev {
        problems.push(fail(format!(
            "context manifest \"{id}\" run_state_checkpoint.{field} resolves to edition {c_rev}, not the manifest's {m_rev}; both must pin the same edition"
        )));
    }
    if let (Some(dm), Some(dc)) = (confirmed_digest(m).digest, confirmed_digest(c).digest) {
        if dm.to_lowercase() != dc.to_lowercase() {
            problems.push(fail(format!(
                "context manifest \"{id}\" run_state_checkpoint.{field} resolves to a source whose digest differs from the manifest's {field}"
            )));
        }
    }
}

/// `(x ?? null) === (y ?? null)` for a next step: the checkpoint's value
/// against a response's.
pub(crate) fn same_step(have: Option<&RecordText>, want: &ResponseField<Option<String>>) -> bool {
    match want {
        ResponseField::Absent => have.is_none(),
        ResponseField::Present(w) => have.map(RecordText::as_str) == w.as_deref(),
        ResponseField::Foreign(_) => false,
    }
}

/// `JSON.stringify` of a response next step.
pub(crate) fn step_json(step: &ResponseField<Option<String>>) -> String {
    match step {
        ResponseField::Absent | ResponseField::Present(None) => "null".to_string(),
        ResponseField::Present(Some(s)) => json_quote(s),
        ResponseField::Foreign(f) => f.json_text.clone(),
    }
}

/// The string members of a response array, trimmed, as a set.
pub(crate) fn response_text_set(items: &[ResponseItem]) -> HashSet<&str> {
    items
        .iter()
        .filter_map(|item| match item {
            ResponseItem::Text(s) => Some(s.trim()),
            ResponseItem::Foreign(_) => None,
        })
        .collect()
}

/// `JSON.stringify` of a response scope revision.
pub(crate) fn response_scope_revision_json(value: &ResponseField<u64>) -> String {
    scope_revision_json(value)
}

/// `JSON.stringify` of a response string field (absent as `null`).
pub(crate) fn response_text_json(value: &ResponseField<String>) -> String {
    value.json_or_null()
}

/// The string value of a response field, if it is one.
pub(crate) fn response_text(value: &ResponseField<String>) -> Option<&str> {
    value.text()
}
