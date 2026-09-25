//! The per-export checks of `evaluateInstanceCanonicalExport`, in the Node
//! reference's order and wording.

use crate::run_contracts::{ResponseField, ResponseItem};
use crate::types::{AuthorityKind, ContentDigest, Diagnostic, OriginKind, ScopeType};

use super::super::content::{check_content_envelope, EnvelopeFields};
use super::super::fail;
use super::super::opaque::opaque_ref_problem;
use super::super::plan::{same_resolved_scope, RECORD_TYPE as PLAN_RECORD_TYPE};
use super::super::resolved::{
    DigestResponse, MappingResponse, MigrationPlanResponse, OpenObject, ResponseCatalogue,
    SourceContentResponse,
};
use super::{
    compute_export_idempotency_key, compute_source_content_key, ExportInput, ExportSourceInput,
};

pub(super) const RECORD_TYPE: &str = "instance-canonical-export";
const SOURCE_CONTENT_RECORD_TYPE: &str = "instance-source-content";

/// The nine fields an exported record and the plan's target are compared
/// on (`pick`), `$schema` included.
const PICKED: [&str; 9] = [
    "$schema",
    "id",
    "title",
    "record_type",
    "scope",
    "origin",
    "authority",
    "payload",
    "schema_version",
];

fn text_is(field: &ResponseField<String>, expected: &str) -> bool {
    field.text() == Some(expected)
}

fn digest_object(field: &ResponseField<DigestResponse>) -> Option<&DigestResponse> {
    match field {
        ResponseField::Present(d) => Some(d),
        _ => None,
    }
}

fn is_minting(m: &MappingResponse) -> bool {
    text_is(&m.disposition, "migrated") || text_is(&m.disposition, "merged")
}

/// `resolveAndCheckMigrationPlan`, after resolution.
fn check_resolved_plan(
    at: &str,
    entry: &ExportInput,
    resolved: &MigrationPlanResponse,
    problems: &mut Vec<Diagnostic>,
) {
    let reference = entry.payload.plan_ref.as_str();
    for key in &resolved.unknown_keys {
        problems.push(fail(format!(
            "{at} plan_ref \"{reference}\": the resolver returned a plan with unknown field \"{key}\"; the resolved response is closed to {{ record_type, plan_ref, plan_fingerprint, scope, source, record_units, mappings }}"
        )));
    }
    if !text_is(&resolved.record_type, PLAN_RECORD_TYPE) {
        problems.push(fail(format!(
            "{at} plan_ref \"{reference}\": resolved record_type is \"{}\", not \"{PLAN_RECORD_TYPE}\"",
            resolved.record_type.string_or_undefined()
        )));
    }
    if !text_is(&resolved.plan_ref, reference) {
        problems.push(fail(format!(
            "{at} plan_ref \"{reference}\": resolved plan_ref \"{}\" does not echo the queried ref",
            resolved.plan_ref.string_or_undefined()
        )));
    }
    let declared = entry.payload.plan_fingerprint.value();
    if !text_is(&resolved.plan_fingerprint, declared) {
        problems.push(fail(format!(
            "{at} plan_fingerprint \"{declared}\" does not match the resolved plan's own recomputed plan_fingerprint \"{}\"; an export pinned to a stale or different version of the plan's content never confirms the current one",
            resolved.plan_fingerprint.string_or_undefined()
        )));
    }
    let source = &entry.payload.source;
    let same_source = match &resolved.source {
        ResponseField::Present(rs) => {
            text_is(&rs.repository_ref, source.repository_ref.as_str())
                && text_is(&rs.revision, source.revision.as_str())
                && digest_object(&rs.digest).is_some_and(|d| {
                    text_is(&d.algorithm, source.digest.algorithm().as_str())
                        && text_is(&d.value, source.digest.value())
                })
        }
        _ => false,
    };
    if !same_source {
        problems.push(fail(format!(
            "{at} source does not match the resolved plan's own pinned source (repository_ref/revision/digest); an export must carry the SAME source identity as the plan it proves"
        )));
    }
}

/// The resolved plan's minted targets, by id, in first-seen order; a
/// later mapping for the same id replaces the target but keeps its place
/// (`Map.set`).
fn expected_targets(resolved: &MigrationPlanResponse) -> Vec<(&str, &OpenObject)> {
    let mut targets: Vec<(&str, &OpenObject)> = Vec::new();
    for m in resolved.mappings.iter().filter(|m| is_minting(m)) {
        let Some(target) = &m.target else { continue };
        let Some(id) = target.id.text() else { continue };
        match targets.iter_mut().find(|(t, _)| *t == id) {
            Some(slot) => slot.1 = &target.record,
            None => targets.push((id, &target.record)),
        }
    }
    targets
}

fn expected_retained(resolved: &MigrationPlanResponse) -> Vec<(&str, &ResponseField<String>)> {
    let mut retained: Vec<(&str, &ResponseField<String>)> = Vec::new();
    for m in &resolved.mappings {
        if !text_is(&m.disposition, "retained-transitional") {
            continue;
        }
        let Some(unit_id) = m.unit_id.text() else {
            continue;
        };
        match retained.iter_mut().find(|(u, _)| *u == unit_id) {
            Some(slot) => slot.1 = &m.retained_reason,
            None => retained.push((unit_id, &m.retained_reason)),
        }
    }
    retained
}

fn count_in_order<'a>(ids: impl Iterator<Item = &'a str>) -> Vec<(&'a str, usize)> {
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for id in ids {
        match counts.iter_mut().find(|(i, _)| *i == id) {
            Some(slot) => slot.1 += 1,
            None => counts.push((id, 1)),
        }
    }
    counts
}

/// Property: completeness (`checkExportCompleteness`).
fn check_completeness(
    at: &str,
    entry: &ExportInput,
    resolved: &MigrationPlanResponse,
    problems: &mut Vec<Diagnostic>,
) {
    let targets = expected_targets(resolved);
    let records = &entry.payload.records;
    for record in records {
        let id = record.head.id.as_str();
        let Some((_, expected)) = targets.iter().find(|(t, _)| *t == id) else {
            problems.push(fail(format!(
                "{at} exported record \"{id}\" does not correspond to any migrated or merged target the resolved plan minted; an extra record is never legal (property: completeness)"
            )));
            continue;
        };
        // The envelope schema closes an exported record to exactly the nine
        // picked fields, so its pick is the whole record.
        let exported = crate::canonical::CanonicalJson::from_json(
            record
                .head
                .canonical_record(record.payload.canonical().into_json()),
        );
        if expected.pick(&PICKED).canonical_text() != exported.canonical_text() {
            problems.push(fail(format!(
                "{at} exported record \"{id}\" diverges from the plan's own target for the same group; the exported record and the plan's target must be structurally identical, including $schema (property: completeness)"
            )));
        }
        problems.extend(check_content_envelope(
            &format!("{at} exported record \"{id}\" payload"),
            record.payload.envelope(),
        ));
    }
    for (id, count) in count_in_order(records.iter().map(|r| r.head.id.as_str())) {
        if count > 1 {
            problems.push(fail(format!(
                "{at} exported record \"{id}\" appears {count} times; exactly one record is required per migrated/merged target (property: completeness)"
            )));
        }
    }
    for (target_id, _) in &targets {
        if !records.iter().any(|r| r.head.id.as_str() == *target_id) {
            problems.push(fail(format!(
                "{at} the plan's target \"{target_id}\" has no corresponding exported record; every migrated unit and every merged group requires exactly one exported record (property: completeness)"
            )));
        }
    }

    let retained = expected_retained(resolved);
    for entry_retained in &entry.payload.retained {
        let unit_id = entry_retained.unit_id.as_str();
        let Some((_, reason)) = retained.iter().find(|(u, _)| *u == unit_id) else {
            problems.push(fail(format!(
                "{at} retained unit_id \"{unit_id}\" is not a retained-transitional unit of the resolved plan; an unknown or misclassified unit is never a legal retained entry (property: completeness)"
            )));
            continue;
        };
        if !text_is(reason, entry_retained.reason.as_str()) {
            problems.push(fail(format!(
                "{at} retained unit_id \"{unit_id}\" reason does not match the plan's own retained_reason for that unit; a retained reason is a documented fact of the plan, not a freely restated one"
            )));
        }
    }
    for (unit_id, count) in
        count_in_order(entry.payload.retained.iter().map(|r| r.unit_id.as_str()))
    {
        if count > 1 {
            problems.push(fail(format!(
                "{at} retained unit_id \"{unit_id}\" appears {count} times; exactly one retained entry is required per retained-transitional unit (property: completeness)"
            )));
        }
    }
    for (unit_id, _) in &retained {
        if !entry
            .payload
            .retained
            .iter()
            .any(|r| r.unit_id.as_str() == *unit_id)
        {
            problems.push(fail(format!(
                "{at} the plan's retained-transitional unit \"{unit_id}\" has no corresponding retained entry; every retained-transitional unit must be listed exactly once (property: completeness)"
            )));
        }
    }
}

fn unit_refs_text(items: &[String]) -> String {
    items.join(", ")
}

/// The resolved `unit_refs` as `[...new Set(array)].sort()`.
fn resolved_unit_refs(field: &ResponseField<Vec<ResponseItem>>) -> (Vec<String>, bool) {
    let items = match field {
        ResponseField::Present(items) => items.as_slice(),
        _ => &[],
    };
    let mut all_text = true;
    let mut seen: Vec<String> = Vec::new();
    for item in items {
        let text = match item {
            ResponseItem::Text(s) => s.clone(),
            ResponseItem::Foreign(f) => {
                all_text = false;
                f.coerced().to_string()
            }
        };
        if !seen.contains(&text) {
            seen.push(text);
        }
    }
    seen.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    (seen, all_text)
}

/// Property: content preservation (`resolveAndCheckSourceContent`).
fn check_source_content(
    at: &str,
    source: &ExportSourceInput,
    unit_refs: &[String],
    merge_rule_ref: Option<&str>,
    payload: &EnvelopeFields,
    content: &ResponseCatalogue<SourceContentResponse>,
    problems: &mut Vec<Diagnostic>,
) {
    let key = compute_source_content_key(
        source.repository_ref.as_str(),
        source.revision.as_str(),
        unit_refs,
        merge_rule_ref,
    );
    let Some(r) = content.resolve(&key) else {
        let rule = merge_rule_ref
            .filter(|r| !r.is_empty())
            .map(|r| format!(" and merge_rule_ref \"{r}\""))
            .unwrap_or_default();
        problems.push(fail(format!(
            "{at} does not resolve to known source content through the external resolver for unit_ref(s) [{}]{rule}; an unresolved content claim never confirms preservation (property: content preservation)",
            unit_refs_text(unit_refs)
        )));
        return;
    };
    for key in &r.unknown_keys {
        problems.push(fail(format!(
            "{at}: the resolver returned content with unknown field \"{key}\"; the resolved response is closed to {{ record_type, repository_ref, revision, unit_refs, merge_rule_ref, media_type, encoding, content, digest }}"
        )));
    }
    if !text_is(&r.record_type, SOURCE_CONTENT_RECORD_TYPE) {
        problems.push(fail(format!(
            "{at}: resolved record_type is \"{}\", not \"{SOURCE_CONTENT_RECORD_TYPE}\"",
            r.record_type.string_or_undefined()
        )));
    }
    if !text_is(&r.repository_ref, source.repository_ref.as_str())
        || !text_is(&r.revision, source.revision.as_str())
    {
        problems.push(fail(format!(
            "{at}: resolved content names repository_ref/revision \"{}\"/\"{}\", which does not match this export's own source \"{}\"/\"{}\"; content resolved for a different source never confirms preservation",
            r.repository_ref.string_or_undefined(),
            r.revision.string_or_undefined(),
            source.repository_ref,
            source.revision
        )));
    }
    let (resolved_refs, all_text) = resolved_unit_refs(&r.unit_refs);
    if !all_text || resolved_refs != unit_refs {
        problems.push(fail(format!(
            "{at}: resolved content names unit_ref(s) [{}], which does not match the actual contributing unit(s) [{}]; content resolved for different source unit(s) never confirms preservation",
            unit_refs_text(&resolved_refs),
            unit_refs_text(unit_refs)
        )));
    }
    let resolved_rule: Option<&str> = match &r.merge_rule_ref {
        ResponseField::Present(Some(rule)) => Some(rule),
        _ => None,
    };
    let rule_matches = match &r.merge_rule_ref {
        ResponseField::Foreign(_) => false,
        _ => resolved_rule == merge_rule_ref,
    };
    if !rule_matches {
        problems.push(fail(format!(
            "{at}: resolved content names merge_rule_ref \"{}\", which does not match this target's own merge_rule_ref \"{}\"; a merged record's content must be resolved together with its OWN confirmed merge rule",
            r.merge_rule_ref.string_or_undefined(),
            merge_rule_ref.unwrap_or("undefined")
        )));
    }
    let resolved_fields = EnvelopeFields {
        media_type: r.media_type.clone(),
        encoding: r.encoding.clone(),
        content: r.content.clone(),
        digest_algorithm: digest_object(&r.digest)
            .map_or(ResponseField::Absent, |d| d.algorithm.clone()),
        digest_value: digest_object(&r.digest).map_or(ResponseField::Absent, |d| d.value.clone()),
    };
    problems.extend(check_content_envelope(
        &format!("{at} resolved source content"),
        &resolved_fields,
    ));
    if !payload.same_bytes(&resolved_fields) {
        problems.push(fail(format!(
            "{at}: exported payload does not match the resolved actual content of its contributing source unit(s) byte-for-byte (media_type/encoding/content/digest); a changed value or byte is never accepted as preserved (property: content preservation)"
        )));
    }
}

/// `checkPayloadPreservation`: for every minted target with a record, the
/// group's actual contributing `unit_ref`s (through the resolved plan's own
/// units) and, for a merge, its own `merge_rule_ref`.
fn check_preservation(
    at: &str,
    entry: &ExportInput,
    resolved: &MigrationPlanResponse,
    content: &ResponseCatalogue<SourceContentResponse>,
    problems: &mut Vec<Diagnostic>,
) {
    let unit_ref_of = |unit_id: &str| -> Option<&str> {
        resolved
            .record_units
            .iter()
            .rev()
            .find(|u| u.id.text() == Some(unit_id))
            .and_then(|u| u.unit_ref.text())
    };
    for (target_id, _) in expected_targets(resolved) {
        let Some(record) = entry
            .payload
            .records
            .iter()
            .rev()
            .find(|r| r.head.id.as_str() == target_id)
        else {
            continue;
        };
        let group: Vec<&MappingResponse> = resolved
            .mappings
            .iter()
            .filter(|m| {
                is_minting(m) && m.target.as_ref().and_then(|t| t.id.text()) == Some(target_id)
            })
            .collect();
        let mut unit_refs: Vec<String> = Vec::new();
        for m in &group {
            let Some(unit_ref) = m.unit_id.text().and_then(unit_ref_of) else {
                continue;
            };
            if !unit_refs.iter().any(|r| r == unit_ref) {
                unit_refs.push(unit_ref.to_string());
            }
        }
        unit_refs.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        let merge_rule_ref = group.iter().find_map(|m| m.merge_rule_ref.text());
        check_source_content(
            &format!("{at} target \"{target_id}\""),
            &entry.payload.source,
            &unit_refs,
            merge_rule_ref,
            record.payload.envelope(),
            content,
            problems,
        );
    }
}

/// The per-export half of `evaluateInstanceCanonicalExport`.
pub(super) fn check_export(
    entry: &ExportInput,
    resolved: Option<&MigrationPlanResponse>,
    content: &ResponseCatalogue<SourceContentResponse>,
    digest: &ContentDigest,
    problems: &mut Vec<Diagnostic>,
) {
    let id = entry.head.id.as_str();
    let at = format!("canonical export \"{id}\"");
    if entry.head.record_type.as_str() != RECORD_TYPE {
        problems.push(fail(format!(
            "{at} declares record_type \"{}\", not \"{RECORD_TYPE}\"",
            entry.head.record_type
        )));
    }
    let scope_type = entry.head.scope.scope_type();
    if !matches!(
        scope_type,
        ScopeType::ProjectWorkspace | ScopeType::RepositoryScope
    ) {
        problems.push(fail(format!(
            "{at} scope.type is \"{scope_type}\"; a canonical export is scoped to project-workspace or repository-scope only"
        )));
    }
    let origin = entry.head.origin_kind();
    if origin != OriginKind::Derived {
        problems.push(fail(format!(
            "{at} origin.kind is \"{origin}\", not \"derived\"; a canonical export is mechanically derived from its plan, never a fresh declared act"
        )));
    }
    let authority = entry.head.authority.kind;
    if authority != AuthorityKind::DelegatedRun {
        problems.push(fail(format!(
            "{at} authority.kind is \"{authority}\", not \"delegated-run\"; the export is produced by a run's delegated authority and never mints an owner decision on its own"
        )));
    }
    let payload = &entry.payload;
    for (value, label) in [
        (
            payload.source.repository_ref.as_str(),
            format!("{at} source.repository_ref"),
        ),
        (payload.plan_ref.as_str(), format!("{at} plan_ref")),
    ] {
        if let Some(problem) = opaque_ref_problem(value, &label) {
            problems.push(fail(problem));
        }
    }
    let expected_origin = format!("migration-plan:{}", payload.plan_ref);
    if entry.head.origin_source_ref() != Some(expected_origin.as_str()) {
        problems.push(fail(format!(
            "{at} origin.source_ref \"{}\" does not trace to this export's own plan_ref \"{}\"; expected \"{expected_origin}\"",
            entry.head.origin_source_ref().unwrap_or("undefined"),
            payload.plan_ref
        )));
    }

    match resolved {
        None => problems.push(fail(format!(
            "{at} plan_ref \"{}\" does not resolve to a known migration plan through the external resolver; a claimed plan_ref with no resolved backing never confirms an export",
            payload.plan_ref
        ))),
        Some(plan) => {
            check_resolved_plan(&at, entry, plan, problems);
            if !same_resolved_scope(&entry.head.scope, &plan.scope) {
                problems.push(fail(format!(
                    "{at} scope does not match the resolved plan's own scope; an export must be scoped exactly like the plan it proves"
                )));
            }
            check_completeness(&at, entry, plan, problems);
            check_preservation(&at, entry, plan, content, problems);
        }
    }

    let key =
        compute_export_idempotency_key(payload.plan_ref.as_str(), payload.plan_fingerprint.value());
    if payload.idempotency_key != key {
        problems.push(fail(format!(
            "{at} idempotency_key \"{}\" does not match the recomputed key \"{}\" derived from this export's own plan_ref and plan_fingerprint; idempotency_key is never an arbitrary free-form string",
            payload.idempotency_key.value(),
            key.value()
        )));
    }
    if &payload.digest != digest {
        problems.push(fail(format!(
            "{at} digest \"{}\" does not match the recomputed digest \"{}\" of its own documented canonical projection (plan_ref, plan_fingerprint, source, records, retained); the same pinned content must always compute the same digest",
            payload.digest.value(),
            digest.value()
        )));
    }
}
