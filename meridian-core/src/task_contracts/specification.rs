//! Domain checks for `task-specification-contract`, run by
//! [`check_task_specification`] — the ONE public constructor of
//! [`TaskSpecification`]: acceptance-criteria integrity, constraint-set
//! integrity, the run-state leak guard, scope/origin closed-set membership,
//! the human-readable title's Cyrillic requirement, `task_pattern`
//! resolution against an already-built [`super::catalog::TaskPatternCatalog`]
//! — AND free-text PORTABILITY scanning
//! ([`crate::task_contracts::portability::non_portable_reason`]: rooted
//! paths, drive letters, `~/` references, `file://` URLs) over every
//! domain text field (`rust-architecture-conformance-3` corrective round
//! item 2: a [`TaskSpecification`] this function returns has ALREADY passed
//! portability, never only structure — there is no second, optional pass a
//! caller must remember to also run before treating the value as valid).
//! Every function here takes already-extracted primitive values and never
//! sees `serde_json::Value`; the `$schema`-envelope-specific portability
//! check (the declared schema reference itself, not a domain field) stays a
//! separate step in `meridian-app`'s orchestration, since `$schema` is not
//! one of this type's own fields.

use std::collections::HashSet;

use crate::resolver::{ChangeClass, WorkItemKind, WorkKind};
use crate::task_contracts::catalog::{PatternRefError, TaskPatternCatalog};
use crate::task_contracts::portability::non_portable_reason;
use crate::types::{Diagnostic, DiagnosticLevel, SemanticId};

fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message).expect("message is non-empty")
}

fn blank(s: Option<&str>) -> bool {
    s.map(|s| s.trim().is_empty()).unwrap_or(true)
}

fn is_semantic_id(s: &str) -> bool {
    SemanticId::new(s).is_ok()
}

pub fn has_cyrillic(s: &str) -> bool {
    s.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c))
}

pub const RECORD_TYPE: &str = "task-specification";

/// The only two workspace-scope-model areas a project task specification
/// may occupy, and the reason each other area is excluded.
pub const ALLOWED_SCOPE_TYPES: [&str; 2] = ["project-workspace", "repository-scope"];

pub fn scope_rejection_reason(scope_type: &str) -> &'static str {
    match scope_type {
        "built-in-methodology" => "built-in-methodology is Kernel methodology, not a place for a concrete work statement",
        "user-profile" => "user-profile holds a user's rules and settings, not a work statement",
        "organization-profile" => "organization-profile holds an organisation's rules and settings, not a work statement",
        "run-state" => "run-state holds one execution run's episodic state, and a specification exists before any run and may drive several",
        _ => "it is not one of the two areas a project task specification may occupy",
    }
}

/// Fields that describe the RUN, not the statement of the work.
/// `execution-state-model` owns them
/// (`standards/workspace/task-specification.md` §5).
pub const RUN_STATE_FIELDS: [&str; 14] = [
    "lifecycle_stage",
    "work_status",
    "scope_revision",
    "current_actor",
    "actor",
    "supervision_mode",
    "resolved_norms",
    "completed_checks",
    "blockers",
    "next_action",
    "next_gate",
    "transition_history",
    "workflow",
    "run_state",
];

/// One acceptance criterion's already-extracted primitive fields.
#[derive(Debug, Clone, Copy)]
pub struct AcceptanceCriterionInput<'a> {
    pub id: Option<&'a str>,
    pub statement: Option<&'a str>,
    pub verification_method: Option<&'a str>,
    pub verification_expected_result: Option<&'a str>,
}

/// Structural (non-portability) acceptance-criteria checks: non-empty set,
/// each with a stable, unique semantic id, a stated statement, and a
/// verifiable condition (a method AND an expected_result, not a free
/// phrase) — plus whole-criterion (statement + verification) deduplication.
fn check_acceptance_criteria(
    spec_id: &str,
    criteria: &[AcceptanceCriterionInput],
) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    if criteria.is_empty() {
        problems.push(fail(format!(
            "task specification \"{spec_id}\" carries no acceptance criteria; the set must be non-empty"
        )));
        return problems;
    }
    let mut seen_id = HashSet::new();
    let mut seen_body = HashSet::new();
    for (i, c) in criteria.iter().enumerate() {
        let at = c.id.map(str::to_string).unwrap_or_else(|| format!("#{i}"));
        match c.id {
            Some(cid) if is_semantic_id(cid) => {
                if !seen_id.insert(cid.to_string()) {
                    problems.push(fail(format!(
                        "task specification \"{spec_id}\" acceptance criterion id \"{cid}\" is used more than once"
                    )));
                }
            }
            _ => problems.push(fail(format!(
                "task specification \"{spec_id}\" acceptance criterion {at} has no stable identifier (a semantic id)"
            ))),
        }
        if blank(c.statement) {
            problems.push(fail(format!(
                "task specification \"{spec_id}\" acceptance criterion {at} has no statement"
            )));
        }
        let verifiable = !blank(c.verification_method) && !blank(c.verification_expected_result);
        if !verifiable {
            problems.push(fail(format!(
                "task specification \"{spec_id}\" acceptance criterion {at} has no verifiable condition; a criterion states a method and an observable expected_result, not a free phrase"
            )));
        }
        let body_key = format!(
            "{}|{}|{}",
            c.statement.unwrap_or("").trim(),
            c.verification_method.unwrap_or("").trim(),
            c.verification_expected_result.unwrap_or("").trim(),
        );
        if !seen_body.insert(body_key) {
            problems.push(fail(format!(
                "task specification \"{spec_id}\" acceptance criterion {at} duplicates another criterion's statement and verification"
            )));
        }
    }
    problems
}

/// Structural (non-portability) constraint-set checks: non-empty,
/// non-blank, no duplicate.
fn check_constraints(spec_id: &str, constraints: &[String]) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    if constraints.is_empty() {
        problems.push(fail(format!(
            "task specification \"{spec_id}\" carries no constraints; the set must be non-empty"
        )));
        return problems;
    }
    let mut seen = HashSet::new();
    for (i, c) in constraints.iter().enumerate() {
        if c.trim().is_empty() {
            problems.push(fail(format!(
                "task specification \"{spec_id}\" constraint #{i} is empty or whitespace-only"
            )));
            continue;
        }
        if !seen.insert(c.trim().to_string()) {
            problems.push(fail(format!(
                "task specification \"{spec_id}\" repeats constraint \"{c}\""
            )));
        }
    }
    problems
}

fn check_run_state_leak(spec_id: &str, payload_keys: &[String]) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    for f in RUN_STATE_FIELDS {
        if payload_keys.iter().any(|k| k == f) {
            problems.push(fail(format!(
                "task specification \"{spec_id}\" payload carries \"{f}\"; run state (stage, status, actor, transition history, next step) belongs to execution-state-model, not to the specification"
            )));
        }
    }
    if payload_keys.iter().any(|k| k == "record_type") {
        problems.push(fail(format!(
            "task specification \"{spec_id}\" repeats record_type inside the payload; the record type is declared once, on the envelope"
        )));
    }
    problems
}

fn check_scope_type(spec_id: &str, scope_type: Option<&str>) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    if let Some(scope_type) = scope_type {
        if !ALLOWED_SCOPE_TYPES.contains(&scope_type) {
            let why = scope_rejection_reason(scope_type);
            problems.push(fail(format!(
                "task specification \"{spec_id}\" is scoped to \"{scope_type}\"; a project task specification lives in project-workspace or repository-scope only — {why}"
            )));
        }
    }
    problems
}

/// Free-text portability over every domain field a task specification
/// carries — the same scan [`check_task_specification`] used to leave to a
/// caller's separate pass, now folded into its own gate
/// (`rust-architecture-conformance-3` corrective round item 2).
fn check_portability(spec_id: &str, fields: &SpecificationFields) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    let scan = |problems: &mut Vec<Diagnostic>, value: Option<&str>, label: &str| {
        if let Some(reason) = non_portable_reason(value) {
            problems.push(fail(format!(
                "task specification \"{spec_id}\" {label} contains {reason}; a specification is portable and carries no rooted machine path"
            )));
        }
    };

    scan(&mut problems, fields.title, "title");
    scan(&mut problems, fields.goal, "goal");
    scan(&mut problems, fields.initial_state, "initial state");
    scan(&mut problems, fields.target_model, "target model");
    scan(&mut problems, fields.origin_source_ref, "origin.source_ref");
    scan(
        &mut problems,
        fields.authority_ref,
        "authority.authority_ref",
    );
    scan(&mut problems, fields.decision_ref, "authority.decision_ref");

    for (i, c) in fields.constraints.iter().enumerate() {
        if let Some(reason) = non_portable_reason(Some(c)) {
            problems.push(fail(format!(
                "task specification \"{spec_id}\" constraint #{i} contains {reason}"
            )));
        }
    }

    for (i, c) in fields.acceptance_criteria.iter().enumerate() {
        let at = c.id.map(str::to_string).unwrap_or_else(|| format!("#{i}"));
        for (value, field_label) in [
            (c.statement, "statement"),
            (c.verification_method, "method"),
            (c.verification_expected_result, "expected_result"),
        ] {
            if let Some(reason) = non_portable_reason(value) {
                problems.push(fail(format!(
                    "task specification \"{spec_id}\" acceptance criterion {at} contains {reason} ({field_label})"
                )));
            }
        }
    }

    problems
}

fn check_origin_kind(spec_id: &str, origin_kind: Option<&str>) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    if origin_kind == Some("built-in") {
        problems.push(fail(format!(
            "task specification \"{spec_id}\" declares origin.kind \"built-in\"; a concrete specification is authored in a workspace, not shipped with the methodology"
        )));
    }
    problems
}

/// `payload.task_pattern`'s already-extracted primitive fields.
#[derive(Debug, Clone, Copy)]
pub struct DeclaredPattern<'a> {
    pub id: Option<&'a str>,
    pub work_kind: Option<WorkKind>,
    pub change_class_present: bool,
    pub change_class: Option<ChangeClass>,
}

/// Resolves `payload.task_pattern` against `catalog` and cross-checks the
/// declared `work_kind`/`change_class` (when present) against the resolved
/// pattern's own [`WorkItemKind`]. A caller never receives a resolved kind
/// from an unknown, ambiguous, or cross-inconsistent reference.
fn resolve_task_pattern(
    spec_id: &str,
    declared: Option<DeclaredPattern>,
    catalog: &TaskPatternCatalog,
) -> (Vec<Diagnostic>, Option<WorkItemKind>) {
    let mut problems = Vec::new();
    let no_pattern_diagnostic = || {
        fail(format!(
            "task specification \"{spec_id}\" names no task pattern; the specification selects exactly one pattern from task-pattern-registry"
        ))
    };
    let Some(declared) = declared else {
        problems.push(no_pattern_diagnostic());
        return (problems, None);
    };
    let Some(tp_id) = declared.id.filter(|s| !s.is_empty()) else {
        problems.push(no_pattern_diagnostic());
        return (problems, None);
    };
    match catalog.resolve(tp_id) {
        Err(PatternRefError::Unknown) => {
            problems.push(fail(format!(
                "task specification \"{spec_id}\" references task pattern \"{tp_id}\", which is not in task-pattern-registry"
            )));
            (problems, None)
        }
        Err(PatternRefError::Ambiguous(n)) => {
            problems.push(fail(format!(
                "task specification \"{spec_id}\" references task pattern \"{tp_id}\", which is declared {n} times in task-pattern-registry — the reference is ambiguous"
            )));
            (problems, None)
        }
        Ok(pattern) => {
            let resolved_kind = pattern.kind();
            if let Some(declared_wk) = declared.work_kind {
                if declared_wk != resolved_kind.work_kind() {
                    problems.push(fail(format!(
                        "task specification \"{spec_id}\" declares work_kind \"{declared_wk}\" but task pattern \"{tp_id}\" is work_kind \"{}\"",
                        resolved_kind.work_kind()
                    )));
                }
            }
            if declared.change_class_present {
                let dcc = declared.change_class;
                let pcc = resolved_kind.change_class();
                if dcc != pcc {
                    let dcc_display = dcc
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "none".to_string());
                    let pcc_display = pcc
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "none".to_string());
                    problems.push(fail(format!(
                        "task specification \"{spec_id}\" declares change_class \"{dcc_display}\" but task pattern \"{tp_id}\" is change_class \"{pcc_display}\""
                    )));
                }
            }
            (problems, Some(resolved_kind))
        }
    }
}

/// Every already-extracted primitive field [`check_task_specification`]
/// needs. Constructing this is the caller's (`meridian-app`'s) own DTO ->
/// primitive projection; nothing here is a `serde_json::Value`.
pub struct SpecificationFields<'a> {
    pub id: Option<&'a str>,
    pub record_type: &'a str,
    pub title: Option<&'a str>,
    pub scope_type: Option<&'a str>,
    pub origin_kind: Option<&'a str>,
    pub origin_source_ref: Option<&'a str>,
    pub authority_ref: Option<&'a str>,
    pub decision_ref: Option<&'a str>,
    pub goal: Option<&'a str>,
    pub initial_state: Option<&'a str>,
    pub target_model: Option<&'a str>,
    pub constraints: &'a [String],
    pub acceptance_criteria: &'a [AcceptanceCriterionInput<'a>],
    pub payload_keys: &'a [String],
    pub declared_pattern: Option<DeclaredPattern<'a>>,
}

/// The label every diagnostic below names the specification by — `id` when
/// it is a non-empty string, `"(no id)"` otherwise (a missing/invalid id is
/// itself reported separately by [`check_task_specification`]).
pub fn spec_id_label(id: Option<&str>) -> &str {
    id.filter(|s| !s.is_empty()).unwrap_or("(no id)")
}

/// A task specification that passed every structural AND portability check
/// and resolved cleanly against the task-pattern catalogue — the ONLY way
/// to obtain one is [`check_task_specification`] returning a `Some` here
/// alongside an empty diagnostic list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSpecification {
    id: SemanticId,
    kind: WorkItemKind,
}

impl TaskSpecification {
    pub fn id(&self) -> &SemanticId {
        &self.id
    }

    pub fn kind(&self) -> WorkItemKind {
        self.kind
    }
}

/// Runs every `task-specification-contract` domain check — structural
/// rules, free-text portability, and `task_pattern` resolution against
/// `catalog` — and returns the full diagnostic list plus, only when it is
/// empty AND `id` is itself a valid [`SemanticId`], the resolved
/// [`TaskSpecification`]. This is the ONE public path to a
/// [`TaskSpecification`]: a rooted path or other non-portable text in any
/// domain field is exactly as disqualifying as a missing acceptance
/// criterion, checked by this same function
/// (`rust-architecture-conformance-3` corrective round item 2) — a caller
/// can never obtain the typed value structurally clean but portability-
/// unchecked. The caller (`meridian-app`'s orchestration) still runs its
/// OWN separate `$schema`-envelope check and merges that diagnostic list
/// with this one before deciding pass/fail, since `$schema` is not a field
/// of [`SpecificationFields`].
pub fn check_task_specification(
    fields: &SpecificationFields,
    catalog: &TaskPatternCatalog,
) -> (Vec<Diagnostic>, Option<TaskSpecification>) {
    let mut problems = Vec::new();
    let id_label = spec_id_label(fields.id);

    if fields.record_type != RECORD_TYPE {
        problems.push(fail(format!(
            "task specification \"{id_label}\" declares record_type \"{}\", not \"{RECORD_TYPE}\"",
            fields.record_type
        )));
    }
    if !fields.id.is_some_and(is_semantic_id) {
        problems.push(fail(format!(
            "task specification \"{id_label}\" has no stable semantic id on the record envelope"
        )));
    }
    match fields.title {
        None => problems.push(fail(format!(
            "task specification \"{id_label}\" has no human-readable title"
        ))),
        Some(t) if t.trim().is_empty() => problems.push(fail(format!(
            "task specification \"{id_label}\" has no human-readable title"
        ))),
        Some(t) if !has_cyrillic(t) => problems.push(fail(format!(
            "task specification \"{id_label}\" title \"{t}\" carries no Russian (Cyrillic) text; the specification name is stated in Russian for the human reader"
        ))),
        _ => {}
    }

    problems.extend(check_scope_type(id_label, fields.scope_type));
    problems.extend(check_origin_kind(id_label, fields.origin_kind));

    for (value, label) in [
        (fields.goal, "goal"),
        (fields.initial_state, "initial state"),
        (fields.target_model, "target model"),
    ] {
        match value {
            None => problems.push(fail(format!(
                "task specification \"{id_label}\" states no {label}; it is a required input with no default"
            ))),
            Some(v) if v.trim().is_empty() => problems.push(fail(format!(
                "task specification \"{id_label}\" {label} is empty or whitespace-only"
            ))),
            _ => {}
        }
    }

    problems.extend(check_constraints(id_label, fields.constraints));
    problems.extend(check_acceptance_criteria(
        id_label,
        fields.acceptance_criteria,
    ));
    problems.extend(check_run_state_leak(id_label, fields.payload_keys));
    problems.extend(check_portability(id_label, fields));

    let (pattern_problems, resolved_kind) =
        resolve_task_pattern(id_label, fields.declared_pattern, catalog);
    problems.extend(pattern_problems);

    let resolved = if problems.is_empty() {
        match (
            fields.id.and_then(|s| SemanticId::new(s).ok()),
            resolved_kind,
        ) {
            (Some(id), Some(kind)) => Some(TaskSpecification { id, kind }),
            _ => None,
        }
    } else {
        None
    };

    (problems, resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_contracts::catalog::{CanonicalLink, TaskPatternCatalog, TaskPatternEntry};
    use crate::types::NonEmptyString;

    fn catalog_with_assessment() -> TaskPatternCatalog {
        let entry = TaskPatternEntry::try_new(
            SemanticId::new("assess-existing-state").unwrap(),
            WorkItemKind::Assessment,
            vec![CanonicalLink::Absent {
                reason: NonEmptyString::new("x").unwrap(),
            }],
            vec![CanonicalLink::Absent {
                reason: NonEmptyString::new("x").unwrap(),
            }],
            vec![CanonicalLink::Absent {
                reason: NonEmptyString::new("x").unwrap(),
            }],
        )
        .unwrap();
        // A single-entry catalogue is missing six of the seven mandatory
        // pairs — a catalogue-level defect the fail-closed
        // `TaskPatternCatalog::build` (`rust-architecture-conformance-3`
        // corrective round item 1) now refuses to build at all. This
        // module's own tests only need a minimal catalogue to exercise
        // `task_pattern` RESOLUTION, not registry completeness, so they use
        // the crate-local test-only escape hatch instead.
        TaskPatternCatalog::test_only_unchecked(vec![entry])
    }

    fn valid_fields<'a>(
        constraints: &'a [String],
        criteria: &'a [AcceptanceCriterionInput<'a>],
        payload_keys: &'a [String],
    ) -> SpecificationFields<'a> {
        SpecificationFields {
            id: Some("sample-spec"),
            record_type: RECORD_TYPE,
            title: Some("Пример спецификации"),
            scope_type: Some("repository-scope"),
            origin_kind: Some("declared"),
            origin_source_ref: Some("sources/example"),
            authority_ref: Some("repository-maintainer"),
            decision_ref: None,
            goal: Some("goal text"),
            initial_state: Some("initial state text"),
            target_model: Some("target model text"),
            constraints,
            acceptance_criteria: criteria,
            payload_keys,
            declared_pattern: Some(DeclaredPattern {
                id: Some("assess-existing-state"),
                work_kind: Some(WorkKind::Assessment),
                change_class_present: false,
                change_class: None,
            }),
        }
    }

    #[test]
    fn a_well_formed_specification_resolves_cleanly() {
        let catalog = catalog_with_assessment();
        let constraints = vec!["one constraint".to_string()];
        let criteria = vec![AcceptanceCriterionInput {
            id: Some("criterion-one"),
            statement: Some("statement"),
            verification_method: Some("method"),
            verification_expected_result: Some("expected"),
        }];
        let payload_keys = vec!["goal".to_string()];
        let fields = valid_fields(&constraints, &criteria, &payload_keys);
        let (problems, resolved) = check_task_specification(&fields, &catalog);
        assert!(problems.is_empty(), "{problems:?}");
        let resolved = resolved.expect("must resolve");
        assert_eq!(resolved.id().as_str(), "sample-spec");
        assert_eq!(resolved.kind(), WorkItemKind::Assessment);
    }

    #[test]
    fn an_unknown_task_pattern_reference_is_rejected() {
        let catalog = catalog_with_assessment();
        let constraints = vec!["one constraint".to_string()];
        let criteria = vec![AcceptanceCriterionInput {
            id: Some("criterion-one"),
            statement: Some("statement"),
            verification_method: Some("method"),
            verification_expected_result: Some("expected"),
        }];
        let payload_keys = vec![];
        let mut fields = valid_fields(&constraints, &criteria, &payload_keys);
        fields.declared_pattern = Some(DeclaredPattern {
            id: Some("no-such-pattern"),
            work_kind: None,
            change_class_present: false,
            change_class: None,
        });
        let (problems, resolved) = check_task_specification(&fields, &catalog);
        assert!(resolved.is_none());
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("not in task-pattern-registry")),
            "{problems:?}"
        );
    }

    #[test]
    fn work_kind_disagreement_with_the_resolved_pattern_is_rejected() {
        let catalog = catalog_with_assessment();
        let constraints = vec!["one constraint".to_string()];
        let criteria = vec![AcceptanceCriterionInput {
            id: Some("criterion-one"),
            statement: Some("statement"),
            verification_method: Some("method"),
            verification_expected_result: Some("expected"),
        }];
        let payload_keys = vec![];
        let mut fields = valid_fields(&constraints, &criteria, &payload_keys);
        fields.declared_pattern = Some(DeclaredPattern {
            id: Some("assess-existing-state"),
            work_kind: Some(WorkKind::Operation),
            change_class_present: false,
            change_class: None,
        });
        let (problems, resolved) = check_task_specification(&fields, &catalog);
        assert!(resolved.is_none());
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("declares work_kind")),
            "{problems:?}"
        );
    }

    #[test]
    fn empty_acceptance_criteria_is_rejected() {
        let catalog = catalog_with_assessment();
        let constraints = vec!["one constraint".to_string()];
        let criteria: Vec<AcceptanceCriterionInput> = vec![];
        let payload_keys = vec![];
        let fields = valid_fields(&constraints, &criteria, &payload_keys);
        let (problems, resolved) = check_task_specification(&fields, &catalog);
        assert!(resolved.is_none());
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("no acceptance criteria")),
            "{problems:?}"
        );
    }

    #[test]
    fn a_run_state_field_in_the_payload_is_rejected() {
        let catalog = catalog_with_assessment();
        let constraints = vec!["one constraint".to_string()];
        let criteria = vec![AcceptanceCriterionInput {
            id: Some("criterion-one"),
            statement: Some("statement"),
            verification_method: Some("method"),
            verification_expected_result: Some("expected"),
        }];
        let payload_keys = vec!["next_action".to_string()];
        let fields = valid_fields(&constraints, &criteria, &payload_keys);
        let (problems, resolved) = check_task_specification(&fields, &catalog);
        assert!(resolved.is_none());
        assert!(
            problems.iter().any(|p| p.message().contains("next_action")),
            "{problems:?}"
        );
    }

    #[test]
    fn a_title_without_cyrillic_text_is_rejected() {
        let catalog = catalog_with_assessment();
        let constraints = vec!["one constraint".to_string()];
        let criteria = vec![AcceptanceCriterionInput {
            id: Some("criterion-one"),
            statement: Some("statement"),
            verification_method: Some("method"),
            verification_expected_result: Some("expected"),
        }];
        let payload_keys = vec![];
        let mut fields = valid_fields(&constraints, &criteria, &payload_keys);
        fields.title = Some("English title only");
        let (problems, resolved) = check_task_specification(&fields, &catalog);
        assert!(resolved.is_none());
        assert!(
            problems.iter().any(|p| p.message().contains("Russian")),
            "{problems:?}"
        );
    }

    #[test]
    fn a_forbidden_scope_is_rejected_with_its_reason() {
        let catalog = catalog_with_assessment();
        let constraints = vec!["one constraint".to_string()];
        let criteria = vec![AcceptanceCriterionInput {
            id: Some("criterion-one"),
            statement: Some("statement"),
            verification_method: Some("method"),
            verification_expected_result: Some("expected"),
        }];
        let payload_keys = vec![];
        let mut fields = valid_fields(&constraints, &criteria, &payload_keys);
        fields.scope_type = Some("user-profile");
        let (problems, resolved) = check_task_specification(&fields, &catalog);
        assert!(resolved.is_none());
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("user's rules and settings")),
            "{problems:?}"
        );
    }

    /// Corrective round item 2's core-level regression: `TaskSpecification`
    /// has exactly one public constructor — [`check_task_specification`] —
    /// and a rooted path anywhere in a domain field can never come out the
    /// other end as a typed, "valid" value, even though every OTHER check
    /// (structural, run-state leak, pattern resolution) is satisfied. This
    /// is proven directly against the core API, with no `meridian-app`
    /// orchestration involved — portability is this function's own gate,
    /// not something a caller must additionally run.
    #[test]
    fn a_rooted_path_in_any_domain_field_can_never_produce_a_task_specification() {
        let catalog = catalog_with_assessment();
        let criteria = vec![AcceptanceCriterionInput {
            id: Some("criterion-one"),
            statement: Some("statement"),
            verification_method: Some("method"),
            verification_expected_result: Some("expected"),
        }];
        let payload_keys: Vec<String> = vec![];

        // goal, initial_state, target_model, origin_source_ref,
        // authority_ref, decision_ref, a constraint, and an acceptance
        // criterion field: every domain text field this type carries, each
        // tried in isolation with every OTHER field left fully valid.
        const ROOTED: &str = "/etc/passwd";
        #[derive(Clone, Copy)]
        enum Field {
            Goal,
            InitialState,
            TargetModel,
            OriginSourceRef,
            AuthorityRef,
            DecisionRef,
        }
        let cases = [
            ("goal", Field::Goal),
            ("initial_state", Field::InitialState),
            ("target_model", Field::TargetModel),
            ("origin_source_ref", Field::OriginSourceRef),
            ("authority_ref", Field::AuthorityRef),
            ("decision_ref", Field::DecisionRef),
        ];

        for (label, field) in cases {
            let constraints = vec!["one constraint".to_string()];
            let payload_keys = payload_keys.clone();
            let mut fields = valid_fields(&constraints, &criteria, &payload_keys);
            match field {
                Field::Goal => fields.goal = Some(ROOTED),
                Field::InitialState => fields.initial_state = Some(ROOTED),
                Field::TargetModel => fields.target_model = Some(ROOTED),
                Field::OriginSourceRef => fields.origin_source_ref = Some(ROOTED),
                Field::AuthorityRef => fields.authority_ref = Some(ROOTED),
                Field::DecisionRef => fields.decision_ref = Some(ROOTED),
            }
            let (problems, resolved) = check_task_specification(&fields, &catalog);
            assert!(
                resolved.is_none(),
                "a rooted path in {label} must never produce a TaskSpecification"
            );
            assert!(
                problems.iter().any(|p| p.message().contains("rooted")),
                "{label}: {problems:?}"
            );
        }

        // A rooted path inside a constraint or an acceptance-criterion
        // field, rather than a top-level scalar field.
        let constraints = vec![ROOTED.to_string()];
        let fields = valid_fields(&constraints, &criteria, &payload_keys);
        let (problems, resolved) = check_task_specification(&fields, &catalog);
        assert!(resolved.is_none());
        assert!(
            problems.iter().any(|p| p.message().contains("constraint")),
            "{problems:?}"
        );

        let constraints = vec!["one constraint".to_string()];
        let rooted_criteria = vec![AcceptanceCriterionInput {
            id: Some("criterion-one"),
            statement: Some(ROOTED),
            verification_method: Some("method"),
            verification_expected_result: Some("expected"),
        }];
        let fields = valid_fields(&constraints, &rooted_criteria, &payload_keys);
        let (problems, resolved) = check_task_specification(&fields, &catalog);
        assert!(resolved.is_none());
        assert!(
            problems
                .iter()
                .any(|p| p.message().contains("acceptance criterion")),
            "{problems:?}"
        );
    }
}
