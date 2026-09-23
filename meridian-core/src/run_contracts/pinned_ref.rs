//! Closed structured pinned references
//! (`context-manifest.schema.json` `definitions.pinned_ref`): a record kind,
//! a stable id, a portable reference, an optional run id and a pin
//! (revision and/or SHA-256). A manifest names exactly three of them, in
//! three [`PinnedSlot`]s, each with its own expected kind and run-id rule.

use crate::task_contracts::non_portable_reason;
use crate::types::{Diagnostic, SemanticId};

use super::fail;
use super::identity::{PortableRef, RecordText};
use super::revision::{pin_defect, PinSha256};

/// The record kinds a pinned reference may name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PinnedRecordKind {
    ExecutionRun,
    TaskSpecification,
    RunHumanControl,
}

impl PinnedRecordKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            PinnedRecordKind::ExecutionRun => "execution-run",
            PinnedRecordKind::TaskSpecification => "task-specification",
            PinnedRecordKind::RunHumanControl => "run-human-control",
        }
    }

    pub fn parse(value: &str) -> Option<PinnedRecordKind> {
        match value {
            "execution-run" => Some(PinnedRecordKind::ExecutionRun),
            "task-specification" => Some(PinnedRecordKind::TaskSpecification),
            "run-human-control" => Some(PinnedRecordKind::RunHumanControl),
            _ => None,
        }
    }
}

/// How a slot's `run_id` relates to the manifest's run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunIdRule {
    /// Optional; when present it is the run's own id.
    SelfRun,
    /// A task specification is not run-scoped.
    Forbidden,
    /// A human-control record names the run it belongs to.
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

/// Port of `checkPinnedRef` for one slot. `manifest_run` is the
/// manifest's own `execution_run_ref.id`.
pub(crate) fn check_pinned_ref(
    id: &str,
    site: PinSite,
    slot: PinnedSlot,
    pin: &PinnedRef,
    manifest_run: &SemanticId,
    problems: &mut Vec<Diagnostic>,
) {
    let field = site_label(site, slot);
    if pin.record_type != slot.kind() {
        problems.push(fail(format!(
            "context manifest \"{id}\" {field} names record_type \"{}\", not \"{}\"",
            pin.record_type.as_str(),
            slot.kind().as_str()
        )));
    }
    if pin.reference.is_blank() {
        problems.push(fail(format!(
            "context manifest \"{id}\" {field} has no portable reference"
        )));
    } else if let Some(r) = non_portable_reason(Some(pin.reference.as_str())) {
        problems.push(fail(format!(
            "context manifest \"{id}\" {field} reference contains {r}; the reference is portable and is not an absolute machine path"
        )));
    }
    let revision = pin.revision.as_ref().map(RecordText::as_str);
    if let Some(defect) = pin_defect(revision, pin.sha256.is_some()) {
        problems.push(fail(format!(
            "context manifest \"{id}\" {field} is not pinned to an exact edition: {defect}"
        )));
    }
    if let Some(r) = revision.and_then(|rev| non_portable_reason(Some(rev))) {
        problems.push(fail(format!(
            "context manifest \"{id}\" {field} revision contains {r}"
        )));
    }
    match (slot.run_id_rule(), &pin.run_id) {
        (RunIdRule::Forbidden, Some(run)) => problems.push(fail(format!(
            "context manifest \"{id}\" {field} carries run_id \"{}\"; a task specification is not run-scoped and names no run_id",
            run.as_str()
        ))),
        (RunIdRule::Required, None) => problems.push(fail(format!(
            "context manifest \"{id}\" {field} carries no run_id; a run-human-control record explicitly names the run it belongs to"
        ))),
        (RunIdRule::Required, Some(run)) if run != manifest_run => problems.push(fail(format!(
            "context manifest \"{id}\" {field} run_id \"{}\" is not the referenced run \"{}\"; the human-control record must belong to the same run",
            run.as_str(),
            manifest_run.as_str()
        ))),
        (RunIdRule::SelfRun, Some(run)) if *run != pin.id => problems.push(fail(format!(
            "context manifest \"{id}\" {field} run_id \"{}\" is not the run's own id \"{}\"",
            run.as_str(),
            pin.id.as_str()
        ))),
        _ => {}
    }
}
