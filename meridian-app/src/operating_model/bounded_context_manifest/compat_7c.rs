//! TEMPORARY compatibility facade for the two not-yet-typed 7c families,
//! `evidence-and-handoff-contract` and `meridian-field-evaluation`
//! (`governance/plans/meridian-rust-migration-program-plan.md` §5.19.3
//! point 7). Removing it belongs to the package that types 7c.
//!
//! It exposes, for EXACTLY four consumers —
//! `meridian-app/src/operating_model/evidence_and_handoff.rs`,
//! `meridian-app/src/operating_model/field_evaluation.rs`,
//! `meridian-cli/src/commands/validate/evidence_and_handoff.rs` and
//! `meridian-cli/src/commands/validate/field_evaluation.rs` — the string
//! views and the `Value`-level resolver those modules were written against.
//! Every item is a re-export of, or a one-expression adapter over,
//! `meridian_core::run_contracts`: the vocabularies, revision
//! classification and resolved-entry key sets have ONE owner, and this
//! file holds no rule of its own. The allowlist is pinned by the
//! `rust_architecture_conformance_5` structural test in `meridian-cli`.

use meridian_core::run_contracts::{LifecycleStage, ResolvedStateField, RevisionClass, WorkStatus};
use serde_json::Value;

pub use meridian_core::run_contracts::RESOLVED_ENTRY_COMMON_KEYS;

/// [`LifecycleStage`] as strings, in lifecycle order.
pub const LIFECYCLE_STAGES: [&str; 11] = [
    LifecycleStage::Intake.as_str(),
    LifecycleStage::Classification.as_str(),
    LifecycleStage::NormResolution.as_str(),
    LifecycleStage::Planning.as_str(),
    LifecycleStage::Execution.as_str(),
    LifecycleStage::Verification.as_str(),
    LifecycleStage::Acceptance.as_str(),
    LifecycleStage::Integration.as_str(),
    LifecycleStage::Deployment.as_str(),
    LifecycleStage::Observation.as_str(),
    LifecycleStage::Completion.as_str(),
];

/// [`WorkStatus`] as strings.
pub const WORK_STATUSES: [&str; 8] = [
    WorkStatus::Planned.as_str(),
    WorkStatus::Ready.as_str(),
    WorkStatus::Active.as_str(),
    WorkStatus::WaitingHuman.as_str(),
    WorkStatus::Blocked.as_str(),
    WorkStatus::Failed.as_str(),
    WorkStatus::Completed.as_str(),
    WorkStatus::Cancelled.as_str(),
];

/// The terminal [`WorkStatus`] members as strings.
pub const TERMINAL_STATUSES: [&str; 2] = [
    WorkStatus::Completed.as_str(),
    WorkStatus::Cancelled.as_str(),
];

/// [`ResolvedStateField::NAMES`].
pub const REQUIRED_RESOLVED_STATE_FIELDS: [&str; 9] = ResolvedStateField::NAMES;

/// [`RevisionClass::classify`] as its Node label.
pub fn classify_revision(revision: Option<&str>) -> &'static str {
    RevisionClass::classify(revision).as_str()
}

/// The `Value`-level resolver the two 7c families still take.
pub type RecordResolver<'a> = dyn Fn(&Value) -> Option<Value> + 'a;

/// `makeRecordResolver` for the 7c families' own `resolution` map: a pinned
/// reference's `reference` string looked up in the map; a non-object entry
/// does not resolve. Transport lookup only — the three 7b families use the
/// typed `meridian_core::run_contracts::ResolutionCatalogue` instead.
pub fn make_record_resolver(resolution_map: &Value) -> impl Fn(&Value) -> Option<Value> + '_ {
    move |pin: &Value| {
        let reference = pin.get("reference").and_then(Value::as_str)?;
        resolution_map
            .get(reference)
            .filter(|e| e.is_object())
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bounded_context_manifest_compat_7c_views_come_from_the_core_owner() {
        assert_eq!(LIFECYCLE_STAGES[2], "norm_resolution");
        assert_eq!(TERMINAL_STATUSES, ["completed", "cancelled"]);
        assert_eq!(REQUIRED_RESOLVED_STATE_FIELDS[0], "task_specification_ref");
        assert_eq!(classify_revision(Some("main")), "floating");
        let map = json!({ "records/x": { "id": "x" }, "records/y": "no" });
        let resolve = make_record_resolver(&map);
        assert!(resolve(&json!({ "reference": "records/x" })).is_some());
        assert!(resolve(&json!({ "reference": "records/y" })).is_none());
        assert!(resolve(&json!("records/x")).is_none());
    }
}
