//! The per-plan checks of `evaluateInstanceDataMigration`, in the Node
//! reference's order and wording. Each function appends to the plan's own
//! problem list; [`check_plan`] runs them for one plan.

use std::collections::{BTreeMap, BTreeSet};

use crate::run_contracts::{RecordText, ResponseField};
use crate::types::{AuthorityKind, ContentDigest, Diagnostic, OriginKind, ScopeType, Verdict};

use super::super::content::check_content_envelope;
use super::super::fail;
use super::super::opaque::opaque_ref_problem;
use super::super::resolved::{
    DigestResponse, PlanResolution, ScopeResponse, SupersededPlanResponse,
};
use super::input::{
    FieldBasisValue, MappingInput, PlanInput, Qualification, RollbackInput, RollbackPlanInput,
    SourceInput, SubVerdictInput,
};

pub(crate) const RECORD_TYPE: &str = "instance-migration-plan";
const SOURCE_SNAPSHOT_RECORD_TYPE: &str = "instance-source-snapshot";
const EVIDENCE_RECORD_TYPE: &str = "instance-migration-evidence";
const ROLLBACK_SNAPSHOT_RECORD_TYPE: &str = "instance-rollback-snapshot";
const DETERMINISTIC_PLAN_RECORD_TYPE: &str = "instance-deterministic-reconstruction-plan";
const RESTORATION_EVIDENCE_RECORD_TYPE: &str = "instance-restoration-evidence";
const SUPERSEDED_PLAN_RECORD_TYPE: &str = "instance-superseded-plan";

fn text_is(field: &ResponseField<String>, expected: &str) -> bool {
    field.text() == Some(expected)
}

fn unknown_problems(
    unknown: &[String],
    subject: &str,
    what: &str,
    closed: &[&str],
    problems: &mut Vec<Diagnostic>,
) {
    for key in unknown {
        problems.push(fail(format!(
            "{subject}: the resolver returned {what} with unknown field \"{key}\"; the resolved response is closed to {{ {} }}",
            closed.join(", ")
        )));
    }
}

/// A resolved digest object read as `isObject(d) ? d : {}`.
fn digest_object(field: &ResponseField<DigestResponse>) -> Option<&DigestResponse> {
    match field {
        ResponseField::Present(d) => Some(d),
        _ => None,
    }
}

fn digest_matches(field: &ResponseField<DigestResponse>, declared: &ContentDigest) -> bool {
    digest_object(field).is_some_and(|d| {
        text_is(&d.algorithm, declared.algorithm().as_str()) && text_is(&d.value, declared.value())
    })
}

/// Property 1 / 8: every portable ref this plan declares.
fn check_refs(at: &str, input: &PlanInput, problems: &mut Vec<Diagnostic>) {
    let payload = &input.payload;
    let mut refs: Vec<(String, &RecordText)> = vec![(
        format!("{at} source.repository_ref"),
        &payload.source.repository_ref,
    )];
    refs.push((
        format!("{at} rollback.source_snapshot_ref"),
        &payload.rollback.source_snapshot_ref,
    ));
    match &payload.rollback.plan {
        RollbackPlanInput::DeterministicReconstruction {
            deterministic_plan_ref,
        } => refs.push((
            format!("{at} rollback.deterministic_plan_ref"),
            deterministic_plan_ref,
        )),
        RollbackPlanInput::VerifiedRestoration {
            restoration_evidence_ref,
        } => refs.push((
            format!("{at} rollback.restoration_evidence_ref"),
            restoration_evidence_ref,
        )),
    }
    for (sub, label) in [
        (
            &payload.verification.coverage,
            "verification.coverage.evidence_ref",
        ),
        (
            &payload.verification.applicability_preservation,
            "verification.applicability_preservation.evidence_ref",
        ),
    ] {
        if let SubVerdictInput::Verified { evidence_ref } = sub {
            refs.push((format!("{at} {label}"), evidence_ref));
        }
    }
    for m in &payload.mappings {
        if let Some(rule) = m.merge_rule_ref() {
            refs.push((
                format!("{at} mapping \"{}\" merge_rule_ref", m.unit_id),
                rule,
            ));
        }
    }
    for (label, value) in refs {
        if let Some(problem) = opaque_ref_problem(value.as_str(), &label) {
            problems.push(fail(problem));
        }
    }
}

/// Property 2: `resolveAndCheckSourceSnapshot`.
fn check_source_snapshot(
    at: &str,
    source: &SourceInput,
    resolution: &PlanResolution,
    problems: &mut Vec<Diagnostic>,
) {
    if source.qualification != Qualification::Reproducible {
        return;
    }
    let key = format!("{}@{}", source.repository_ref, source.revision);
    let Some(r) = resolution.source_snapshots.resolve(&key) else {
        problems.push(fail(format!(
            "{at} source.qualification is \"reproducible\" but its revision does not resolve to a known source snapshot through the external resolver; a claimed value with no resolved backing never confirms reproducibility (property 2)"
        )));
        return;
    };
    for key in &r.unknown_keys {
        problems.push(fail(format!(
            "{at} source: the resolver returned a snapshot with unknown field \"{key}\"; the resolved response is closed to {{ record_type, repository_ref, revision, digest, working_tree_clean }}"
        )));
    }
    if !text_is(&r.record_type, SOURCE_SNAPSHOT_RECORD_TYPE) {
        problems.push(fail(format!(
            "{at} source: resolved snapshot record_type is \"{}\", not \"{SOURCE_SNAPSHOT_RECORD_TYPE}\"",
            r.record_type.string_or_undefined()
        )));
    }
    if !text_is(&r.repository_ref, source.repository_ref.as_str()) {
        problems.push(fail(format!(
            "{at} source.repository_ref \"{}\" does not match the resolved snapshot's repository_ref \"{}\"; a snapshot resolved for a different repository never confirms reproducibility (property 2)",
            source.repository_ref,
            r.repository_ref.string_or_undefined()
        )));
    }
    if !text_is(&r.revision, source.revision.as_str()) {
        problems.push(fail(format!(
            "{at} source.revision \"{}\" does not match the resolved snapshot's revision \"{}\"; a claimed revision with no matching resolved snapshot never confirms reproducibility (property 2)",
            source.revision,
            r.revision.string_or_undefined()
        )));
    }
    if let Some(d) = digest_object(&r.digest) {
        for key in &d.unknown_keys {
            problems.push(fail(format!(
                "{at} source: resolved snapshot digest has unknown field \"{key}\"; the resolved digest is closed to {{ algorithm, value }}"
            )));
        }
    }
    if !digest_matches(&r.digest, &source.digest) {
        problems.push(fail(format!(
            "{at} source.digest does not match the resolved snapshot's digest (extra, missing or differing field); a claimed digest with no matching resolved snapshot never confirms reproducibility (property 2)"
        )));
    }
    if r.working_tree_clean != ResponseField::Present(true) {
        problems.push(fail(format!(
            "{at} source.qualification is \"reproducible\" but the resolved snapshot's working_tree_clean is not true"
        )));
    }
}

/// Property 5: `resolveAndCheckRollbackSnapshot` and the variant's own
/// deterministic-plan or restoration-evidence check.
fn check_rollback(
    at: &str,
    plan_id: &str,
    fingerprint: &ContentDigest,
    source: &SourceInput,
    rollback: &RollbackInput,
    resolution: &PlanResolution,
    problems: &mut Vec<Diagnostic>,
) {
    let snapshot_ref = rollback.source_snapshot_ref.as_str();
    let subject = format!("{at} rollback.source_snapshot_ref \"{snapshot_ref}\"");
    match resolution.rollback_snapshots.resolve(snapshot_ref) {
        None => problems.push(fail(format!(
            "{subject} does not resolve to a known source snapshot through the external resolver; a bare ref string never confirms reversibility (property 5)"
        ))),
        Some(r) => {
            unknown_problems(
                &r.unknown_keys,
                &subject,
                "a record",
                &[
                    "record_type",
                    "source_snapshot_ref",
                    "repository_ref",
                    "revision",
                    "digest",
                ],
                problems,
            );
            if !text_is(&r.record_type, ROLLBACK_SNAPSHOT_RECORD_TYPE) {
                problems.push(fail(format!(
                    "{subject}: resolved record_type is \"{}\", not \"{ROLLBACK_SNAPSHOT_RECORD_TYPE}\"",
                    r.record_type.string_or_undefined()
                )));
            }
            if !text_is(&r.source_snapshot_ref, snapshot_ref) {
                problems.push(fail(format!(
                    "{subject}: resolved source_snapshot_ref \"{}\" does not echo the queried ref",
                    r.source_snapshot_ref.string_or_undefined()
                )));
            }
            if !text_is(&r.repository_ref, source.repository_ref.as_str())
                || !text_is(&r.revision, source.revision.as_str())
            {
                problems.push(fail(format!(
                    "{subject} resolves to repository_ref/revision \"{}\"/\"{}\", which does not match this plan's own source \"{}\"/\"{}\"; a rollback snapshot must uniquely link to the SAME confirmed source snapshot this plan pins (property 5)",
                    r.repository_ref.string_or_undefined(),
                    r.revision.string_or_undefined(),
                    source.repository_ref,
                    source.revision
                )));
            }
            if let Some(d) = digest_object(&r.digest) {
                for key in &d.unknown_keys {
                    problems.push(fail(format!(
                        "{subject}: resolved digest has unknown field \"{key}\"; the resolved digest is closed to {{ algorithm, value }}"
                    )));
                }
            }
            if !digest_matches(&r.digest, &source.digest) {
                problems.push(fail(format!(
                    "{subject} resolves to a digest that does not match this plan's own source.digest (extra, missing or differing field); a rollback snapshot must uniquely link to the SAME confirmed source snapshot (property 5)"
                )));
            }
        }
    }

    match &rollback.plan {
        RollbackPlanInput::DeterministicReconstruction {
            deterministic_plan_ref,
        } => {
            let reference = deterministic_plan_ref.as_str();
            let subject = format!("{at} rollback.deterministic_plan_ref \"{reference}\"");
            let Some(r) = resolution.deterministic_plans.resolve(reference) else {
                problems.push(fail(format!(
                    "{subject} does not resolve to a known deterministic-reconstruction plan through the external resolver; a bare ref string never confirms reversibility (property 5)"
                )));
                return;
            };
            unknown_problems(
                &r.unknown_keys,
                &subject,
                "a record",
                &[
                    "record_type",
                    "deterministic_plan_ref",
                    "source_snapshot_ref",
                    "plan_ref",
                    "plan_fingerprint",
                    "applicable",
                ],
                problems,
            );
            if !text_is(&r.record_type, DETERMINISTIC_PLAN_RECORD_TYPE) {
                problems.push(fail(format!(
                    "{subject}: resolved record_type is \"{}\", not \"{DETERMINISTIC_PLAN_RECORD_TYPE}\"",
                    r.record_type.string_or_undefined()
                )));
            }
            if !text_is(&r.deterministic_plan_ref, reference) {
                problems.push(fail(format!(
                    "{subject}: resolved deterministic_plan_ref \"{}\" does not echo the queried ref",
                    r.deterministic_plan_ref.string_or_undefined()
                )));
            }
            if !text_is(&r.source_snapshot_ref, snapshot_ref) {
                problems.push(fail(format!(
                    "{subject}: resolved source_snapshot_ref \"{}\" does not name this plan's rollback.source_snapshot_ref \"{snapshot_ref}\"; a reconstruction plan for a different snapshot cannot back this rollback (property 5)",
                    r.source_snapshot_ref.string_or_undefined()
                )));
            }
            if !text_is(&r.plan_ref, plan_id) {
                problems.push(fail(format!(
                    "{subject}: resolved plan_ref \"{}\" does not name this plan \"{plan_id}\"; a reconstruction plan bound to a different migration plan cannot back this one",
                    r.plan_ref.string_or_undefined()
                )));
            }
            if !text_is(&r.plan_fingerprint, fingerprint.value()) {
                problems.push(fail(format!(
                    "{subject}: resolved plan_fingerprint \"{}\" does not match this plan's own recomputed fingerprint \"{}\"; a reconstruction plan pinned to a stale or different version of this plan's content never confirms the current one (property 5)",
                    r.plan_fingerprint.string_or_undefined(),
                    fingerprint.value()
                )));
            }
            if r.applicable != ResponseField::Present(true) {
                problems.push(fail(format!(
                    "{subject}: resolved plan is not marked applicable"
                )));
            }
        }
        RollbackPlanInput::VerifiedRestoration {
            restoration_evidence_ref,
        } => {
            let reference = restoration_evidence_ref.as_str();
            let subject = format!("{at} rollback.restoration_evidence_ref \"{reference}\"");
            let Some(r) = resolution.restoration_evidence.resolve(reference) else {
                problems.push(fail(format!(
                    "{subject} does not resolve to a known restoration-evidence record through the external resolver; a bare ref string never confirms reversibility (property 5)"
                )));
                return;
            };
            unknown_problems(
                &r.unknown_keys,
                &subject,
                "a record",
                &[
                    "record_type",
                    "evidence_ref",
                    "plan_ref",
                    "plan_fingerprint",
                    "source_snapshot_ref",
                    "confirms",
                ],
                problems,
            );
            if !text_is(&r.record_type, RESTORATION_EVIDENCE_RECORD_TYPE) {
                problems.push(fail(format!(
                    "{subject}: resolved record_type is \"{}\", not \"{RESTORATION_EVIDENCE_RECORD_TYPE}\"",
                    r.record_type.string_or_undefined()
                )));
            }
            if !text_is(&r.evidence_ref, reference) {
                problems.push(fail(format!(
                    "{subject}: resolved evidence_ref \"{}\" does not echo the queried ref",
                    r.evidence_ref.string_or_undefined()
                )));
            }
            if !text_is(&r.plan_ref, plan_id) {
                problems.push(fail(format!(
                    "{subject}: resolved plan_ref \"{}\" does not name this plan \"{plan_id}\"; restoration evidence from a different plan cannot back this one (property 5)",
                    r.plan_ref.string_or_undefined()
                )));
            }
            if !text_is(&r.plan_fingerprint, fingerprint.value()) {
                problems.push(fail(format!(
                    "{subject}: resolved plan_fingerprint \"{}\" does not match this plan's own recomputed fingerprint \"{}\"; restoration evidence pinned to a stale or different version of this plan's content never confirms the current one (property 5)",
                    r.plan_fingerprint.string_or_undefined(),
                    fingerprint.value()
                )));
            }
            if !text_is(&r.source_snapshot_ref, snapshot_ref) {
                problems.push(fail(format!(
                    "{subject}: resolved source_snapshot_ref \"{}\" does not name this plan's rollback.source_snapshot_ref \"{snapshot_ref}\"; restoration evidence for a different snapshot cannot confirm this rollback (property 5)",
                    r.source_snapshot_ref.string_or_undefined()
                )));
            }
            if r.confirms != ResponseField::Present(true) {
                problems.push(fail(format!(
                    "{subject}: resolved evidence does not confirm restoration (confirms is not true)"
                )));
            }
        }
    }
}

/// Property 3: `checkCoverage`.
fn check_coverage(at: &str, input: &PlanInput, problems: &mut Vec<Diagnostic>) {
    let mut unit_ids: Vec<&str> = Vec::new();
    for u in &input.payload.record_units {
        let id = u.id.as_str();
        if unit_ids.contains(&id) {
            problems.push(fail(format!(
                "{at} record_units id \"{id}\" is declared more than once"
            )));
        } else {
            unit_ids.push(id);
        }
        if u.unit_ref.as_str() == u.classification_basis.as_str() {
            problems.push(fail(format!(
                "{at} record unit \"{id}\" classification_basis equals unit_ref verbatim; classification must not be derived from the path or reference alone"
            )));
        }
    }
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for m in &input.payload.mappings {
        let id = m.unit_id.as_str();
        if !unit_ids.contains(&id) {
            problems.push(fail(format!(
                "{at} mapping references unit_id \"{id}\", which is not a declared record unit; a mapping cannot correspond to more than was declared"
            )));
            continue;
        }
        *counts.entry(id).or_default() += 1;
    }
    for id in unit_ids {
        match counts.get(id).copied().unwrap_or(0) {
            0 => problems.push(fail(format!(
                "{at} record unit \"{id}\" has no mapping; every declared unit requires exactly one correspondence decision (migrated, retained-transitional or merged)"
            ))),
            1 => {}
            n => problems.push(fail(format!(
                "{at} record unit \"{id}\" has {n} mappings; exactly one is required per unit — a duplicate mapping is never a valid correspondence"
            ))),
        }
    }
}

/// `deriveOriginSourceRef`: the sorted (code-unit order), de-duplicated set
/// of contributing unit ids.
pub(crate) fn derive_origin_source_ref(unit_ids: &[&str]) -> String {
    let sorted: BTreeSet<&str> = unit_ids.iter().copied().collect();
    let sorted: Vec<&str> = sorted.into_iter().collect();
    if sorted.len() == 1 {
        format!("record-unit:{}", sorted[0])
    } else {
        format!("record-units:{}", sorted.join(","))
    }
}

/// Properties 3/4: `checkTargetGroups`. Groups are visited in the order
/// their target id first appears, never in hash order.
fn check_target_groups(at: &str, mappings: &[MappingInput], problems: &mut Vec<Diagnostic>) {
    let mut groups: Vec<(&str, Vec<&MappingInput>)> = Vec::new();
    for m in mappings {
        let Some(target) = m.target() else { continue };
        let id = target.id.as_str();
        match groups.iter_mut().find(|(g, _)| *g == id) {
            Some((_, members)) => members.push(m),
            None => groups.push((id, vec![m])),
        }
    }
    for (target_id, group) in groups {
        let migrated = group.iter().any(|m| m.disposition.as_str() == "migrated");
        let merged = group.iter().any(|m| m.disposition.as_str() == "merged");
        if migrated && merged {
            problems.push(fail(format!(
                "{at} target \"{target_id}\" is claimed by mappings with mixed dispositions (migrated and merged); a shared target is consistently one or the other"
            )));
            continue;
        }
        if migrated {
            if group.len() > 1 {
                problems.push(fail(format!(
                    "{at} target \"{target_id}\" is claimed by {} \"migrated\" mappings; a target claimed by more than one unit requires the explicit \"merged\" disposition, not an implicit multi-unit \"migrated\"",
                    group.len()
                )));
                continue;
            }
        } else if group.len() < 2 {
            problems.push(fail(format!(
                "{at} merge target \"{target_id}\" is claimed by only {} mapping(s); a \"merged\" disposition requires at least two source units combining into one target — a single unit is \"migrated\", not \"merged\"",
                group.len()
            )));
            continue;
        } else {
            let mut rules: Vec<&str> = Vec::new();
            for rule in group.iter().filter_map(|m| m.merge_rule_ref()) {
                if !rules.contains(&rule.as_str()) {
                    rules.push(rule.as_str());
                }
            }
            if rules.len() > 1 {
                problems.push(fail(format!(
                    "{at} merge target \"{target_id}\" is claimed by mappings citing different merge_rule_ref values ({}); every mapping merging into one target must cite the same explicit rule",
                    rules.join(", ")
                )));
            }
        }
        if !group.windows(2).all(|w| w[0].target() == w[1].target()) {
            problems.push(fail(format!(
                "{at} target \"{target_id}\" is described inconsistently across the mapping(s) that share it; every mapping contributing to the same target must declare an identical target record"
            )));
        }
        let unit_ids: Vec<&str> = group.iter().map(|m| m.unit_id.as_str()).collect();
        let expected = derive_origin_source_ref(&unit_ids);
        for m in &group {
            let Some(target) = m.target() else { continue };
            if target.origin_source_ref.as_str() != expected {
                problems.push(fail(format!(
                    "{at} target \"{target_id}\" origin.source_ref \"{}\" does not trace to the actual contributing unit(s); expected \"{expected}\" (property 4)",
                    target.origin_source_ref
                )));
            }
            if target.field_basis.origin != FieldBasisValue::Assigned {
                problems.push(fail(format!(
                    "{at} target \"{target_id}\" field_basis.origin is \"{}\", not \"assigned\"; a pre-Meridian record never carried this canonical origin shape, so it can never be claimed \"preserved\" (property 4)",
                    target.field_basis.origin.as_str()
                )));
            }
        }
    }
}

/// The closed owner authority a target's own scope requires.
fn required_authority(scope_type: ScopeType) -> Option<AuthorityKind> {
    match scope_type {
        ScopeType::BuiltInMethodology => Some(AuthorityKind::MethodologyOwner),
        ScopeType::UserProfile => Some(AuthorityKind::User),
        ScopeType::OrganizationProfile => Some(AuthorityKind::Organization),
        ScopeType::ProjectWorkspace => Some(AuthorityKind::ProjectOwner),
        ScopeType::RepositoryScope => Some(AuthorityKind::RepositoryMaintainer),
        ScopeType::RunState => None,
    }
}

/// Property 4: `checkTargetAuthority` and the content-preservation rule for
/// one `migrated`/`merged` mapping.
fn check_target(at: &str, mapping: &MappingInput, problems: &mut Vec<Diagnostic>) {
    let (Some(target), Some(decision)) = (mapping.target(), mapping.owner_decision()) else {
        return;
    };
    let id = target.id.as_str();
    let kind = target.authority.kind;
    let scope_type = target.scope.scope_type();
    if kind == AuthorityKind::DelegatedRun {
        problems.push(fail(format!(
            "{at} target \"{id}\" authority.kind is \"delegated-run\"; the run carrying out the migration cannot itself mint the record's permanent authority (property 4)"
        )));
    } else {
        match required_authority(scope_type) {
            None => problems.push(fail(format!(
                "{at} target \"{id}\" scope.type is \"{scope_type}\"; a migrated or merged record cannot be classified into a scope with no human owner authority"
            ))),
            Some(required) if required != kind => problems.push(fail(format!(
                "{at} target \"{id}\" authority.kind is \"{kind}\", but scope \"{scope_type}\" requires owner authority \"{required}\""
            ))),
            Some(_) => {}
        }
    }
    if target.authority.decision_ref != decision.decision_ref {
        problems.push(fail(format!(
            "{at} target \"{id}\" authority.decision_ref \"{}\" does not match mapping.owner_decision.decision_ref \"{}\"; both name the same owner decision",
            target.authority.decision_ref, decision.decision_ref
        )));
    }
    problems.extend(check_content_envelope(
        &format!("{at} target \"{id}\" payload"),
        &target.payload.fields(),
    ));
}

/// Property 6: `checkVerification`.
fn check_verification(at: &str, input: &PlanInput, problems: &mut Vec<Diagnostic>) {
    let v = &input.payload.verification;
    let reproducible = input.payload.source.qualification == Qualification::Reproducible;
    let overall = v.overall_status;
    if !reproducible && overall != Verdict::Blocked {
        problems.push(fail(format!(
            "{at} source.qualification is \"not-reproducible\" but verification.overall_status is \"{overall}\"; a dirty, incomplete, ambiguous or changed source is never accepted as a reproducible base, and the plan is BLOCKED, not merely unverified"
        )));
    }
    let coverage = matches!(v.coverage, SubVerdictInput::Verified { .. });
    let applicability = matches!(
        v.applicability_preservation,
        SubVerdictInput::Verified { .. }
    );
    if overall == Verdict::Verified && !(coverage && applicability && reproducible) {
        let mut reasons = Vec::new();
        if !coverage {
            reasons.push("coverage is not verified");
        }
        if !applicability {
            reasons.push("applicability preservation is not verified");
        }
        if !reproducible {
            reasons.push("the source is not reproducible");
        }
        problems.push(fail(format!(
            "{at} verification.overall_status is \"VERIFIED\" but {}; a claimed result is checked against actual evidence, never asserted on its own (property 6)",
            reasons.join("; ")
        )));
    }
}

/// Property 6: `resolveAndCheckEvidence` for each `verified` sub-verdict.
fn check_evidence(
    at: &str,
    plan_id: &str,
    fingerprint: &ContentDigest,
    input: &PlanInput,
    resolution: &PlanResolution,
    problems: &mut Vec<Diagnostic>,
) {
    let v = &input.payload.verification;
    for (kind, sub) in [
        ("coverage", &v.coverage),
        ("applicability_preservation", &v.applicability_preservation),
    ] {
        let SubVerdictInput::Verified { evidence_ref } = sub else {
            continue;
        };
        let reference = evidence_ref.as_str();
        let Some(r) = resolution.evidence.resolve(reference) else {
            problems.push(fail(format!(
                "{at} verification.{kind}.evidence_ref \"{reference}\" does not resolve to a known evidence record through the external resolver; an unresolved evidence_ref never confirms a verified status (property 6)"
            )));
            continue;
        };
        let subject = format!("{at} evidence \"{reference}\"");
        unknown_problems(
            &r.unknown_keys,
            &subject,
            "a record",
            &[
                "record_type",
                "evidence_ref",
                "kind",
                "plan_ref",
                "plan_fingerprint",
                "confirms",
            ],
            problems,
        );
        if !text_is(&r.record_type, EVIDENCE_RECORD_TYPE) {
            problems.push(fail(format!(
                "{subject}: resolved record_type is \"{}\", not \"{EVIDENCE_RECORD_TYPE}\"",
                r.record_type.string_or_undefined()
            )));
        }
        if !text_is(&r.evidence_ref, reference) {
            problems.push(fail(format!(
                "{subject}: resolved evidence_ref \"{}\" does not match the queried \"{reference}\"",
                r.evidence_ref.string_or_undefined()
            )));
        }
        if !text_is(&r.kind, kind) {
            problems.push(fail(format!(
                "{subject}: resolved kind \"{}\" does not match the expected \"{kind}\"; evidence for one sub-verdict cannot confirm another",
                r.kind.string_or_undefined()
            )));
        }
        if !text_is(&r.plan_ref, plan_id) {
            problems.push(fail(format!(
                "{subject}: resolved plan_ref \"{}\" does not name this plan \"{plan_id}\"; evidence from a different plan cannot back this one",
                r.plan_ref.string_or_undefined()
            )));
        }
        if !text_is(&r.plan_fingerprint, fingerprint.value()) {
            problems.push(fail(format!(
                "{subject}: resolved plan_fingerprint \"{}\" does not match this plan's own recomputed fingerprint \"{}\"; evidence pinned to a stale or different version of this plan's content never confirms the current one (property 6)",
                r.plan_fingerprint.string_or_undefined(),
                fingerprint.value()
            )));
        }
        if r.confirms != ResponseField::Present(true) {
            problems.push(fail(format!(
                "{subject}: resolved evidence does not confirm the claimed result (confirms is not true)"
            )));
        }
    }
}

/// The per-plan half of `evaluateInstanceDataMigration`, from the
/// envelope-kind checks to the recomputed fingerprint and key. `fingerprint`
/// and `idempotency_key` are this plan's own recomputed values.
pub(super) fn check_plan(
    input: &PlanInput,
    fingerprint: &ContentDigest,
    idempotency_key: &ContentDigest,
    resolution: &PlanResolution,
    problems: &mut Vec<Diagnostic>,
) {
    let id = input.head.id.as_str();
    let at = format!("migration plan \"{id}\"");
    if input.head.record_type.as_str() != RECORD_TYPE {
        problems.push(fail(format!(
            "{at} declares record_type \"{}\", not \"{RECORD_TYPE}\"",
            input.head.record_type
        )));
    }
    let scope_type = input.head.scope.scope_type();
    if !matches!(
        scope_type,
        ScopeType::ProjectWorkspace | ScopeType::RepositoryScope
    ) {
        problems.push(fail(format!(
            "{at} scope.type is \"{scope_type}\"; a migration plan is scoped to project-workspace or repository-scope only"
        )));
    }
    let origin = input.head.origin_kind();
    if origin != OriginKind::Declared {
        problems.push(fail(format!(
            "{at} origin.kind is \"{origin}\", not \"declared\"; a migration plan is an explicit declared act, never text derived from another source"
        )));
    }
    let authority = input.head.authority.kind;
    if authority != AuthorityKind::DelegatedRun {
        problems.push(fail(format!(
            "{at} authority.kind is \"{authority}\", not \"delegated-run\"; the plan itself is carried out by a run's delegated authority and never mints an owner decision on its own"
        )));
    }

    let payload = &input.payload;
    check_refs(&at, input, problems);
    check_source_snapshot(&at, &payload.source, resolution, problems);
    check_rollback(
        &at,
        id,
        fingerprint,
        &payload.source,
        &payload.rollback,
        resolution,
        problems,
    );
    check_coverage(&at, input, problems);
    check_target_groups(&at, &payload.mappings, problems);
    for m in &payload.mappings {
        check_target(&at, m, problems);
    }
    check_verification(&at, input, problems);
    check_evidence(&at, id, fingerprint, input, resolution, problems);

    if &payload.plan_fingerprint != fingerprint {
        problems.push(fail(format!(
            "{at} plan_fingerprint \"{}\" does not match the recomputed fingerprint \"{}\" of its own documented canonical projection (source, record_units, mappings, rollback); the same pinned input must always compute the same fingerprint",
            payload.plan_fingerprint.value(),
            fingerprint.value()
        )));
    }
    if &payload.idempotency_key != idempotency_key {
        problems.push(fail(format!(
            "{at} idempotency_key \"{}\" does not match the recomputed key \"{}\" derived from this plan's own scope and source (repository_ref, revision, digest); idempotency_key is never an arbitrary free-form string",
            payload.idempotency_key.value(),
            idempotency_key.value()
        )));
    }
}

/// `sameScope` of a typed scope and a resolved one: each of the four
/// fields strictly equal, an absent one equal only to an absent one.
pub(crate) fn same_resolved_scope(
    scope: &crate::types::Scope,
    resolved: &ResponseField<ScopeResponse>,
) -> bool {
    let empty = ScopeResponse::default();
    let r = match resolved {
        ResponseField::Present(r) => r,
        _ => &empty,
    };
    let eq = |field: &ResponseField<String>, value: Option<&str>| match (field, value) {
        (ResponseField::Absent, None) => true,
        (ResponseField::Present(s), Some(v)) => s == v,
        _ => false,
    };
    eq(&r.scope_type, Some(scope.scope_type().as_str()))
        && eq(&r.id, Some(scope.id()))
        && eq(&r.workspace_id, scope.workspace_id().map(|w| w.as_str()))
        && eq(
            &r.organization_profile_id,
            scope.organization_profile_id().map(|o| o.as_str()),
        )
}

/// `checkSupersedes` for one plan whose predecessor is NOT in the same
/// batch: resolved through the external boundary, checked closed.
pub(super) fn check_resolved_predecessor(
    input: &PlanInput,
    predecessor: &str,
    resolved: Option<&SupersededPlanResponse>,
    problems: &mut Vec<Diagnostic>,
) {
    let id = input.head.id.as_str();
    let subject = format!("migration plan \"{id}\" supersedes \"{predecessor}\"");
    let Some(r) = resolved else {
        problems.push(fail(format!(
            "{subject}, which is not present in this document and does not resolve to a known predecessor through the external resolver; an unknown or unresolved predecessor is never accepted automatically"
        )));
        return;
    };
    unknown_problems(
        &r.unknown_keys,
        &subject,
        "a record",
        &[
            "record_type",
            "plan_ref",
            "scope",
            "repository_ref",
            "revision",
        ],
        problems,
    );
    if !text_is(&r.record_type, SUPERSEDED_PLAN_RECORD_TYPE) {
        problems.push(fail(format!(
            "{subject}: resolved record_type is \"{}\", not \"{SUPERSEDED_PLAN_RECORD_TYPE}\"",
            r.record_type.string_or_undefined()
        )));
    }
    if !text_is(&r.plan_ref, predecessor) {
        problems.push(fail(format!(
            "{subject}: resolved plan_ref \"{}\" does not echo the queried predecessor",
            r.plan_ref.string_or_undefined()
        )));
    }
    if let ResponseField::Present(scope) = &r.scope {
        for key in &scope.unknown_keys {
            problems.push(fail(format!(
                "{subject}: resolved scope has unknown field \"{key}\"; the resolved scope is closed to {{ type, id, workspace_id, organization_profile_id }}"
            )));
        }
    }
    if !same_resolved_scope(&input.head.scope, &r.scope) {
        problems.push(fail(format!(
            "{subject} but the resolved predecessor is scoped differently; a plan supersedes only a predecessor for the SAME full scope (type, id, workspace_id and organization_profile_id)"
        )));
    }
    let source = &input.payload.source;
    if !text_is(&r.repository_ref, source.repository_ref.as_str()) {
        problems.push(fail(format!(
            "{subject} but the resolved predecessor pins a different repository_ref (\"{}\"); a plan supersedes only a predecessor of the SAME source repository",
            r.repository_ref.string_or_undefined()
        )));
    }
    if text_is(&r.revision, source.revision.as_str()) {
        problems.push(fail(format!(
            "{subject} but the resolved predecessor pins the identical source.revision \"{}\"; a superseding plan requires a changed source revision, not a restatement of the same one",
            source.revision
        )));
    }
}

/// `checkSupersedes` for one plan whose predecessor IS in the same batch.
pub(super) fn check_present_predecessor(
    input: &PlanInput,
    predecessor: &PlanInput,
    problems: &mut Vec<Diagnostic>,
) {
    let id = input.head.id.as_str();
    let sup = predecessor.head.id.as_str();
    let subject = format!("migration plan \"{id}\" supersedes \"{sup}\"");
    if !input.head.scope.same_scope(&predecessor.head.scope) {
        problems.push(fail(format!(
            "{subject} but they are scoped differently; a plan supersedes only a predecessor for the SAME full scope (type, id, workspace_id and organization_profile_id)"
        )));
    }
    let (e, t) = (&input.payload.source, &predecessor.payload.source);
    if e.repository_ref != t.repository_ref {
        problems.push(fail(format!(
            "{subject} but they pin different source.repository_ref (\"{}\" vs \"{}\"); a plan supersedes only a predecessor of the SAME source repository",
            e.repository_ref, t.repository_ref
        )));
    }
    if e.revision == t.revision {
        problems.push(fail(format!(
            "{subject} but both pin the identical source.revision \"{}\"; a superseding plan requires a changed source revision, not a restatement of the same one",
            e.revision
        )));
    }
}
