//! Closed structured pinned references
//! (`definitions.pinned_ref` of the context-manifest, evidence-and-handoff
//! and field-evaluation schemas): a record kind, a stable id, a portable
//! reference, an optional run id and a pin (revision and/or SHA-256).
//!
//! `check_pin_shape` is the one shape rule every family applies to a
//! pinned reference; `check_run_id` is the run-id rule of the two families
//! that bind their references to one run (the manifest's three
//! [`PinnedSlot`]s, the handoff's four slots in
//! `crate::evidence::handoff`).

use crate::task_contracts::non_portable_reason;
use crate::types::{Diagnostic, SemanticId};

use super::envelope::RecordFamily;
use super::fail;
use super::identity::{PortableRef, RecordText};
use super::revision::{pin_defect, PinSha256};

/// The record kinds a pinned reference, or a resolver response, may name.
/// The last five are the composed records the two qualification contracts
/// pin by content digest (`crate::qualification`); no run-contract schema's
/// closed `record_type` enum admits them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PinnedRecordKind {
    ExecutionRun,
    TaskSpecification,
    RunHumanControl,
    ContextManifest,
    FieldEvaluationObservation,
    EvidenceResult,
    WorkspaceConnectionScan,
    InstanceMigrationPlan,
    InstanceCanonicalExport,
    EvidenceAndHandoff,
    FieldEvaluationReport,
}

impl PinnedRecordKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            PinnedRecordKind::ExecutionRun => "execution-run",
            PinnedRecordKind::TaskSpecification => "task-specification",
            PinnedRecordKind::RunHumanControl => "run-human-control",
            PinnedRecordKind::ContextManifest => "context-manifest",
            PinnedRecordKind::FieldEvaluationObservation => "field-evaluation-observation",
            PinnedRecordKind::EvidenceResult => "evidence-result",
            PinnedRecordKind::WorkspaceConnectionScan => "workspace-connection-scan",
            PinnedRecordKind::InstanceMigrationPlan => "instance-migration-plan",
            PinnedRecordKind::InstanceCanonicalExport => "instance-canonical-export",
            PinnedRecordKind::EvidenceAndHandoff => "evidence-and-handoff",
            PinnedRecordKind::FieldEvaluationReport => "field-evaluation-report",
        }
    }

    pub fn parse(value: &str) -> Option<PinnedRecordKind> {
        match value {
            "execution-run" => Some(PinnedRecordKind::ExecutionRun),
            "task-specification" => Some(PinnedRecordKind::TaskSpecification),
            "run-human-control" => Some(PinnedRecordKind::RunHumanControl),
            "context-manifest" => Some(PinnedRecordKind::ContextManifest),
            "field-evaluation-observation" => Some(PinnedRecordKind::FieldEvaluationObservation),
            "evidence-result" => Some(PinnedRecordKind::EvidenceResult),
            "workspace-connection-scan" => Some(PinnedRecordKind::WorkspaceConnectionScan),
            "instance-migration-plan" => Some(PinnedRecordKind::InstanceMigrationPlan),
            "instance-canonical-export" => Some(PinnedRecordKind::InstanceCanonicalExport),
            "evidence-and-handoff" => Some(PinnedRecordKind::EvidenceAndHandoff),
            "field-evaluation-report" => Some(PinnedRecordKind::FieldEvaluationReport),
            _ => None,
        }
    }
}

/// How a run-bound slot's `run_id` relates to the record's run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunIdRule {
    /// Optional; when present it is the run's own id.
    SelfRun,
    /// A task specification is not run-scoped.
    Forbidden,
    /// A run-scoped record names the run it belongs to.
    Required,
}

/// The three pinned-reference slots of a manifest (and of its checkpoint),
/// in the fixed order every walk uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PinnedSlot {
    ExecutionRun,
    TaskSpecification,
    HumanControl,
}

impl PinnedSlot {
    pub const ALL: [PinnedSlot; 3] = [
        PinnedSlot::ExecutionRun,
        PinnedSlot::TaskSpecification,
        PinnedSlot::HumanControl,
    ];

    /// The payload field name of this slot.
    pub const fn field(self) -> &'static str {
        match self {
            PinnedSlot::ExecutionRun => "execution_run_ref",
            PinnedSlot::TaskSpecification => "task_specification_ref",
            PinnedSlot::HumanControl => "human_control_ref",
        }
    }

    /// The record kind this slot must name.
    pub const fn kind(self) -> PinnedRecordKind {
        match self {
            PinnedSlot::ExecutionRun => PinnedRecordKind::ExecutionRun,
            PinnedSlot::TaskSpecification => PinnedRecordKind::TaskSpecification,
            PinnedSlot::HumanControl => PinnedRecordKind::RunHumanControl,
        }
    }

    fn run_id_rule(self) -> RunIdRule {
        match self {
            PinnedSlot::ExecutionRun => RunIdRule::SelfRun,
            PinnedSlot::TaskSpecification => RunIdRule::Forbidden,
            PinnedSlot::HumanControl => RunIdRule::Required,
        }
    }
}

/// Where a pinned reference sits: the manifest's own field or the
/// checkpoint's copy of it. Only the diagnostic label differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PinSite {
    Manifest,
    Checkpoint,
}

pub(crate) fn site_label(site: PinSite, slot: PinnedSlot) -> String {
    match site {
        PinSite::Manifest => slot.field().to_string(),
        PinSite::Checkpoint => format!("run_state_checkpoint.{}", slot.field()),
    }
}

/// One schema-clean pinned reference. Field-for-field equality (`==`) is
/// `samePinnedRef`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedRef {
    pub record_type: PinnedRecordKind,
    pub id: SemanticId,
    pub run_id: Option<SemanticId>,
    pub reference: PortableRef,
    pub revision: Option<RecordText>,
    pub sha256: Option<PinSha256>,
}

/// The shape of one pinned reference, identical in every family
/// (`checkPinnedRef`): the expected record kind, a portable non-blank
/// reference, an exact pin and a portable revision. `field` is how the
/// diagnostic names the reference.
pub(crate) fn check_pin_shape(
    family: RecordFamily,
    id: &str,
    field: &str,
    expected: PinnedRecordKind,
    pin: &PinnedRef,
    problems: &mut Vec<Diagnostic>,
) {
    let label = family.label();
    if pin.record_type != expected {
        problems.push(fail(format!(
            "{label} \"{id}\" {field} names record_type \"{}\", not \"{}\"",
            pin.record_type.as_str(),
            expected.as_str()
        )));
    }
    if pin.reference.is_blank() {
        problems.push(fail(format!(
            "{label} \"{id}\" {field} has no portable reference"
        )));
    } else if let Some(r) = non_portable_reason(Some(pin.reference.as_str())) {
        problems.push(fail(format!(
            "{label} \"{id}\" {field} reference contains {r}; the reference is portable and is not an absolute machine path"
        )));
    }
    let revision = pin.revision.as_ref().map(RecordText::as_str);
    if let Some(defect) = pin_defect(revision, pin.sha256.is_some()) {
        problems.push(fail(format!(
            "{label} \"{id}\" {field} is not pinned to an exact edition: {defect}"
        )));
    }
    if let Some(r) = revision.and_then(|rev| non_portable_reason(Some(rev))) {
        problems.push(fail(format!(
            "{label} \"{id}\" {field} revision contains {r}"
        )));
    }
}

/// The run-id rule of one run-bound slot whose record kind is `kind`.
/// `run` is the record's own `execution_run_ref.id`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn check_run_id(
    family: RecordFamily,
    id: &str,
    field: &str,
    rule: RunIdRule,
    kind: PinnedRecordKind,
    pin: &PinnedRef,
    run: &SemanticId,
    problems: &mut Vec<Diagnostic>,
) {
    let label = family.label();
    let kind = kind.as_str();
    match (rule, &pin.run_id) {
        (RunIdRule::Forbidden, Some(run_id)) => problems.push(fail(format!(
            "{label} \"{id}\" {field} carries run_id \"{}\"; a task specification is not run-scoped and names no run_id",
            run_id.as_str()
        ))),
        (RunIdRule::Required, None) => problems.push(fail(format!(
            "{label} \"{id}\" {field} carries no run_id; a {kind} record explicitly names the run it belongs to"
        ))),
        (RunIdRule::Required, Some(run_id)) if run_id != run => {
            // The manifest names its run-human-control record by its role.
            let owner = match family {
                RecordFamily::ContextManifest => "human-control",
                _ => kind,
            };
            problems.push(fail(format!(
                "{label} \"{id}\" {field} run_id \"{}\" is not the referenced run \"{}\"; the {owner} record must belong to the same run",
                run_id.as_str(),
                run.as_str()
            )))
        }
        (RunIdRule::SelfRun, Some(run_id)) if *run_id != pin.id => problems.push(fail(format!(
            "{label} \"{id}\" {field} run_id \"{}\" is not the run's own id \"{}\"",
            run_id.as_str(),
            pin.id.as_str()
        ))),
        _ => {}
    }
}

/// `checkPinnedRef` of the context manifest: shape, then run id.
pub(crate) fn check_pinned_ref(
    id: &str,
    site: PinSite,
    slot: PinnedSlot,
    pin: &PinnedRef,
    manifest_run: &SemanticId,
    problems: &mut Vec<Diagnostic>,
) {
    let family = RecordFamily::ContextManifest;
    let field = site_label(site, slot);
    check_pin_shape(family, id, &field, slot.kind(), pin, problems);
    check_run_id(
        family,
        id,
        &field,
        slot.run_id_rule(),
        slot.kind(),
        pin,
        manifest_run,
        problems,
    );
}
