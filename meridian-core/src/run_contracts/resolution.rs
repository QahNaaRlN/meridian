//! The external record-resolution boundary, typed.
//!
//! A pinned reference is resolved OUTSIDE the record that names it: a
//! [`ResolutionCatalogue`] maps a portable reference to the
//! [`ResolvedEntry`] a transformer returned for it. The response shapes live
//! in [`super::response`]; this module owns the ONE closed
//! transformer-response contract (`resolvePinnedReference`) the three
//! resolving families share — `bounded-context-manifest`,
//! `evidence-and-handoff-contract` and `meridian-field-evaluation` — with the
//! family-specific wording, closed key sets and completeness tails selected
//! by [`RecordFamily`], never copied.
//!
//! [`ResolvedStateField`] is the closed vocabulary of the checkpoint axes an
//! execution-run response carries (`resolved_state`), and
//! `check_resolved_state` its closed contract. `same_resolved_edition`
//! compares two resolved editions of one manifest slot.

use std::collections::{BTreeMap, HashSet};

use crate::task_contracts::non_portable_reason;
use crate::types::{ContentDigest, Diagnostic, SemanticId};

use super::envelope::RecordFamily;
use super::identity::RecordText;
use super::pinned_ref::{PinnedRecordKind, PinnedRef, PinnedSlot};
use super::response::{
    EvidenceResultResponse, ExtensionKey, ObservationResponse, ResponseField, ResponseItem,
    SpecificationResponse,
};
use super::revision::{is_sha256_text, RevisionClass};
use super::vocabulary::{LifecycleStage, WorkStatus};
use super::{fail, json_quote};

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

/// One transformer response. The common fields, then the extension each
/// record kind may carry; `other_fields` are the names of keys outside
/// every known one, in sorted order. Which extension keys a response may
/// carry is its family's closed key set (`allowed_keys`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedEntry {
    pub record_type: ResponseField<String>,
    pub id: ResponseField<String>,
    pub reference: ResponseField<String>,
    pub revision: ResponseField<String>,
    pub content_digest: ResponseField<String>,
    pub source_bytes: ResponseField<String>,
    pub resolved_state: ResponseField<ResolvedStateResponse>,
    pub linked_run_ref: ResponseField<String>,
    pub specification: SpecificationResponse,
    pub evidence_result: EvidenceResultResponse,
    pub observation: ObservationResponse,
    pub other_fields: Vec<String>,
}

impl ResolvedEntry {
    /// Every extension key this response carries.
    fn present_extension_keys(&self) -> Vec<ExtensionKey> {
        let spec = &self.specification;
        let ev = &self.evidence_result;
        let ob = &self.observation;
        [
            (
                ExtensionKey::ResolvedState,
                self.resolved_state.is_present(),
            ),
            (ExtensionKey::LinkedRunRef, self.linked_run_ref.is_present()),
            (
                ExtensionKey::AcceptanceCriteria,
                spec.acceptance_criteria.is_present(),
            ),
            (
                ExtensionKey::MandatoryChecks,
                spec.mandatory_checks.is_present(),
            ),
            (
                ExtensionKey::ObservedResult,
                ev.observed_result.is_present(),
            ),
            (ExtensionKey::Covers, ev.covers.is_present()),
            (ExtensionKey::CheckRef, ev.check_ref.is_present()),
            (
                ExtensionKey::SpecialisedContract,
                ev.specialised_contract.is_present(),
            ),
            (
                ExtensionKey::RecordedVerdict,
                ev.recorded_verdict.is_present(),
            ),
            (ExtensionKey::MetricRef, ev.metric_ref.is_present()),
            (ExtensionKey::MetricId, ob.metric_id.is_present()),
            (ExtensionKey::Status, ob.status.is_present()),
            (ExtensionKey::Measurement, ob.measurement.is_present()),
            (ExtensionKey::WorkspaceId, ob.workspace_id.is_present()),
            (ExtensionKey::ObservedAt, ob.observed_at.is_present()),
            (ExtensionKey::SupersedesId, ob.supersedes_id.is_present()),
            (
                ExtensionKey::ObservationPeriod,
                ob.observation_period.is_present(),
            ),
            (ExtensionKey::Coverage, ob.coverage.is_present()),
        ]
        .into_iter()
        .filter_map(|(key, present)| present.then_some(key))
        .collect()
    }
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

/// The closed extension key set of a response for `wanted`, in `family`'s
/// contract order (`RESOLVED_ENTRY_KEYS_BY_TYPE` of each Node module). The
/// field evaluation knows no execution-run extension: a `resolved_state`
/// there is an unknown field.
fn allowed_extensions(family: RecordFamily, wanted: PinnedRecordKind) -> &'static [ExtensionKey] {
    use ExtensionKey as K;
    match (family, wanted) {
        (
            RecordFamily::ContextManifest | RecordFamily::EvidenceAndHandoff,
            PinnedRecordKind::ExecutionRun,
        ) => &[K::ResolvedState],
        (RecordFamily::ContextManifest, PinnedRecordKind::RunHumanControl)
        | (
            RecordFamily::EvidenceAndHandoff,
            PinnedRecordKind::RunHumanControl | PinnedRecordKind::ContextManifest,
        ) => &[K::LinkedRunRef],
        (RecordFamily::EvidenceAndHandoff, PinnedRecordKind::TaskSpecification) => {
            &[K::AcceptanceCriteria, K::MandatoryChecks]
        }
        (RecordFamily::EvidenceAndHandoff, PinnedRecordKind::EvidenceResult) => &[
            K::ObservedResult,
            K::Covers,
            K::CheckRef,
            K::SpecialisedContract,
            K::RecordedVerdict,
        ],
        (
            RecordFamily::FieldEvaluationObservation | RecordFamily::FieldEvaluationReport,
            PinnedRecordKind::FieldEvaluationObservation,
        ) => &[
            K::MetricId,
            K::Status,
            K::Measurement,
            K::WorkspaceId,
            K::ObservedAt,
            K::SupersedesId,
            K::ObservationPeriod,
            K::Coverage,
        ],
        (
            RecordFamily::FieldEvaluationObservation | RecordFamily::FieldEvaluationReport,
            PinnedRecordKind::EvidenceResult,
        ) => &[K::ObservedResult, K::MetricRef],
        _ => &[],
    }
}

/// The closed key set's names, common keys first.
fn allowed_keys(family: RecordFamily, wanted: PinnedRecordKind) -> Vec<&'static str> {
    let mut keys = RESOLVED_ENTRY_COMMON_KEYS.to_vec();
    keys.extend(
        allowed_extensions(family, wanted)
            .iter()
            .map(|k| k.as_str()),
    );
    keys
}

/// Every key of `entry` outside the closed key set, sorted — the
/// deterministic order the Rust port has always reported
/// (`COMPATIBILITY.md`: Node names them in source-text order).
fn unknown_entry_keys(
    entry: &ResolvedEntry,
    family: RecordFamily,
    wanted: PinnedRecordKind,
) -> Vec<String> {
    let allowed = allowed_extensions(family, wanted);
    let mut unknown = entry.other_fields.clone();
    unknown.extend(
        entry
            .present_extension_keys()
            .into_iter()
            .filter(|k| !allowed.contains(k))
            .map(|k| k.as_str().to_string()),
    );
    unknown.sort();
    unknown.dedup();
    unknown
}

/// How the family closes an unresolved reference: "… and the manifest fails
/// closed".
fn fails_closed_subject(family: RecordFamily) -> &'static str {
    match family {
        RecordFamily::ContextManifest => "the manifest",
        RecordFamily::EvidenceAndHandoff => "the handoff",
        _ => "the record",
    }
}

/// Port of `resolvePinnedReference`: resolves one pinned reference through
/// `catalogue` and checks the response against `family`'s closed
/// transformer-response contract for `wanted`, including the family's
/// completeness tail for that record kind. `field` is how the diagnostic
/// names the reference. Returns the response even when it has problems —
/// the edition and link comparisons, and a caller's own completeness check,
/// still read it.
pub(crate) fn check_resolved_entry<'c>(
    family: RecordFamily,
    id: &str,
    field: &str,
    wanted: PinnedRecordKind,
    pin: &PinnedRef,
    catalogue: &'c ResolutionCatalogue,
    problems: &mut Vec<Diagnostic>,
) -> Option<&'c ResolvedEntry> {
    let label = family.label();
    let subject = format!("{label} \"{id}\"");
    let want = wanted.as_str();
    let Some(entry) = catalogue.resolve(pin.reference.as_str()) else {
        problems.push(fail(format!(
            "{subject} {field} does not resolve to an actual {want} through the external resolver; a pinned reference that resolves to nothing is not a verified pin and {} fails closed",
            fails_closed_subject(family)
        )));
        return None;
    };

    let allowed = allowed_keys(family, wanted).join(", ");
    for key in unknown_entry_keys(entry, family, wanted) {
        problems.push(fail(format!(
            "{subject} {field}: the resolver returned a record with an unknown field \"{key}\"; the transformer response is closed to {{ {allowed} }}"
        )));
    }

    match entry.record_type.text().filter(|s| !s.is_empty()) {
        None => problems.push(fail(format!(
            "{subject} {field}: the resolver returned a record with no record_type; the transformer response is a closed contract"
        ))),
        Some(rt) if rt != want => problems.push(fail(format!(
            "{subject} {field} resolves to a \"{rt}\" record, not \"{want}\""
        ))),
        Some(_) => {}
    }

    let id_mismatch_tail = match family {
        RecordFamily::FieldEvaluationObservation | RecordFamily::FieldEvaluationReport => {
            "; a pinned reference that resolves to a DIFFERENT record is not the same record that was pinned"
        }
        _ => "",
    };
    match entry.id.non_blank() {
        None => problems.push(fail(format!(
            "{subject} {field}: the resolver returned a {want} with no id; a resolved record without an identity cannot be checked against the pin"
        ))),
        Some(eid) if SemanticId::new(eid).is_err() => problems.push(fail(format!(
            "{subject} {field}: the resolver returned a {want} whose id \"{eid}\" is not a stable semantic identifier"
        ))),
        Some(eid) if eid != pin.id.as_str() => problems.push(fail(format!(
            "{subject} {field} pins id \"{}\" but the reference resolves to record id \"{eid}\"{id_mismatch_tail}",
            pin.id.as_str()
        ))),
        Some(_) => {}
    }

    if entry.reference.is_present() {
        match entry.reference.non_blank() {
            None => problems.push(fail(format!(
                "{subject} {field}: the resolver's stated reference is present but not a non-empty string"
            ))),
            Some(er) => {
                if let Some(r) = non_portable_reason(Some(er)) {
                    problems.push(fail(format!(
                        "{subject} {field}: the resolver's stated reference contains {r}"
                    )));
                } else if er != pin.reference.as_str() {
                    problems.push(fail(format!(
                        "{subject} {field}: the resolver's stated reference \"{er}\" is not the resolved reference \"{}\"",
                        pin.reference
                    )));
                }
            }
        }
    }

    if entry.content_digest.is_present() && !entry.content_digest.text().is_some_and(is_sha256_text)
    {
        problems.push(fail(format!(
            "{subject} {field}: the resolver's content_digest {} is not exactly 64 hexadecimal characters",
            entry.content_digest.json_or_null()
        )));
    }
    if let ResponseField::Foreign(f) = &entry.source_bytes {
        problems.push(fail(format!(
            "{subject} {field}: the resolver's source_bytes is {}, not a string",
            f.kind().null_or_type_of()
        )));
    }

    let revision_exact = check_response_revision(&subject, field, &entry.revision, problems);
    let confirmed = confirmed_digest(entry);
    if !revision_exact && confirmed.digest.is_none() {
        problems.push(fail(format!(
            "{subject} {field}: the resolver confirmed neither an exact revision nor a content digest for this edition; the pinned edition is unverified"
        )));
    }
    if let Some(inconsistent) = &confirmed.inconsistent {
        problems.push(fail(format!(
            "{subject} {field}: the resolver's content_digest \"{inconsistent}\" does not match the SHA-256 of the resolved source bytes \"{}\"",
            confirmed.digest.as_deref().unwrap_or_default()
        )));
    }
    if let Some(pin_rev) = pin.revision.as_ref().filter(|r| !r.is_blank()) {
        match entry.revision.non_blank() {
            None => problems.push(fail(format!(
                "{subject} {field} pins revision \"{pin_rev}\" but the resolver confirmed no exact edition for this reference"
            ))),
            Some(entry_rev) if entry_rev != pin_rev.as_str() => problems.push(fail(format!(
                "{subject} {field} pins revision \"{pin_rev}\" but the resolver confirmed edition \"{entry_rev}\""
            ))),
            Some(_) => {}
        }
    }
    if let Some(pin_sha) = &pin.sha256 {
        match &confirmed.digest {
            None => problems.push(fail(format!(
                "{subject} {field} pins sha256 \"{pin_sha}\" but the resolver confirmed no content digest for this edition; a digest is verified against resolved source, never against a second copy of itself"
            ))),
            Some(d) if d.to_lowercase() != pin_sha.as_str().to_lowercase() => problems.push(fail(format!(
                "{subject} {field} pins sha256 \"{pin_sha}\" but the SHA-256 of the resolved source is \"{d}\""
            ))),
            Some(_) => {}
        }
    }

    check_completeness(family, &subject, field, wanted, entry, problems);
    Some(entry)
}

/// The family's completeness tail for one record kind. The manifest and the
/// handoff check the run's `resolved_state` and a run-scoped record's
/// `linked_run_ref`; the handoff also closes a task specification's
/// machine lists. An evidence result's subject and the field evaluation's
/// resolved records are completed by their own callers.
fn check_completeness(
    family: RecordFamily,
    subject: &str,
    field: &str,
    wanted: PinnedRecordKind,
    entry: &ResolvedEntry,
    problems: &mut Vec<Diagnostic>,
) {
    let want = wanted.as_str();
    let axes_owner = match family {
        RecordFamily::ContextManifest => "the checkpoint axes",
        _ => "the handoff axes",
    };
    match (family, wanted) {
        (
            RecordFamily::ContextManifest | RecordFamily::EvidenceAndHandoff,
            PinnedRecordKind::ExecutionRun,
        ) => check_resolved_state(subject, field, axes_owner, &entry.resolved_state, problems),
        (RecordFamily::ContextManifest, PinnedRecordKind::RunHumanControl)
        | (
            RecordFamily::EvidenceAndHandoff,
            PinnedRecordKind::RunHumanControl | PinnedRecordKind::ContextManifest,
        ) => match entry.linked_run_ref.non_blank() {
            None => problems.push(fail(format!(
                "{subject} {field}: the resolver returned a {want} record with no linked_run_ref; the run it belongs to is unconfirmed"
            ))),
            Some(linked) => {
                if let Some(r) = non_portable_reason(Some(linked)) {
                    problems.push(fail(format!(
                        "{subject} {field}: the resolved {want} record's linked_run_ref contains {r}"
                    )));
                }
            }
        },
        (RecordFamily::EvidenceAndHandoff, PinnedRecordKind::TaskSpecification) => {
            check_resolved_acceptance_criteria(subject, field, &entry.specification, problems);
            check_resolved_mandatory_checks(subject, field, &entry.specification, problems);
        }
        _ => {}
    }
}

/// `checkResolvedAcceptanceCriteria`: a non-empty set of unique semantic ids.
fn check_resolved_acceptance_criteria(
    subject: &str,
    field: &str,
    spec: &SpecificationResponse,
    problems: &mut Vec<Diagnostic>,
) {
    let ResponseField::Present(items) = &spec.acceptance_criteria else {
        problems.push(fail(format!(
            "{subject} {field}: the resolver returned a task-specification record with no acceptance_criteria array; the handoff closes the loop with the specification's acceptance criteria and the transformer must confirm their identifiers"
        )));
        return;
    };
    let at =
        format!("{subject} {field}: the resolved task-specification record's acceptance_criteria");
    if items.is_empty() {
        problems.push(fail(format!(
            "{at} is empty; a canonical task specification requires a non-empty acceptance-criteria set, so a handoff that closes its coverage loop against an empty set is rejected"
        )));
    }
    let mut seen = HashSet::new();
    for (i, item) in items.iter().enumerate() {
        let Some(c) = item.text().filter(|c| SemanticId::new(*c).is_ok()) else {
            problems.push(fail(format!("{at}[{i}] is not a stable semantic id")));
            continue;
        };
        if !seen.insert(c) {
            problems.push(fail(format!("{at} repeats \"{c}\"")));
        }
    }
}

/// `checkResolvedMandatoryChecks`: a list of unique portable references.
fn check_resolved_mandatory_checks(
    subject: &str,
    field: &str,
    spec: &SpecificationResponse,
    problems: &mut Vec<Diagnostic>,
) {
    let ResponseField::Present(items) = &spec.mandatory_checks else {
        problems.push(fail(format!(
            "{subject} {field}: the resolver returned a task-specification record with no mandatory_checks array; the handoff closes the FULL set of mandatory checks against the specification and the transformer must confirm their references"
        )));
        return;
    };
    let at =
        format!("{subject} {field}: the resolved task-specification record's mandatory_checks");
    let mut seen = HashSet::new();
    for (i, item) in items.iter().enumerate() {
        let Some(c) = item.text().filter(|c| !c.trim().is_empty()) else {
            problems.push(fail(format!(
                "{at}[{i}] is not a non-empty portable reference"
            )));
            continue;
        };
        if let Some(r) = non_portable_reason(Some(c)) {
            problems.push(fail(format!("{at}[{i}] contains {r}")));
        }
        if !seen.insert(c.trim()) {
            problems.push(fail(format!("{at} repeats \"{c}\"")));
        }
    }
}

/// The response's own revision; `true` when it is an exact edition.
fn check_response_revision(
    subject: &str,
    field: &str,
    revision: &ResponseField<String>,
    problems: &mut Vec<Diagnostic>,
) -> bool {
    match revision {
        ResponseField::Absent => false,
        ResponseField::Foreign(f) => {
            problems.push(fail(format!(
                "{subject} {field}: the resolver's revision is {}, not a string",
                f.kind().with_article()
            )));
            false
        }
        ResponseField::Present(s) if s.trim().is_empty() => {
            problems.push(fail(format!(
                "{subject} {field}: the resolver's revision is present but empty"
            )));
            false
        }
        ResponseField::Present(s) => {
            if RevisionClass::classify(Some(s)) == RevisionClass::Exact {
                true
            } else {
                problems.push(fail(format!(
                    "{subject} {field}: the resolver confirmed revision {}, which is not an exact edition (a full Git SHA, a strict v?X.Y.Z tag or a SHA-256 digest); the transformer confirms a fixed edition, not a moving reference",
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
    record: &str,
    field: &str,
    axes_owner: &str,
    state: &ResponseField<ResolvedStateResponse>,
    problems: &mut Vec<Diagnostic>,
) {
    let ResponseField::Present(rs) = state else {
        problems.push(fail(format!(
            "{record} {field}: the resolver returned an execution-run record with no resolved_state; {axes_owner} cannot be checked against the run's own state"
        )));
        return;
    };
    let subject = format!("{record} {field}: the resolved execution-run record's resolved_state");
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
    value.json_or_null()
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
                f.kind().null_or_type_of()
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
    step.json_or_null()
}

/// The string members of a response array, trimmed, as a set.
pub(crate) fn response_text_set(items: &[ResponseItem]) -> HashSet<&str> {
    items
        .iter()
        .filter_map(|item| item.text().map(str::trim))
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
