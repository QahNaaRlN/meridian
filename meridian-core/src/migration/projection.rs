//! Byte-compatible canonical projections of migration-owned records — the
//! ONE owner of `computePlanFingerprint`, `computeIdempotencyKey`,
//! `computeExportDigest` and `computeExportIdempotencyKey`
//! (`scripts/lib/instance-data-migration.mjs`) and of the canonical shape
//! of a target record.
//!
//! Each projection is built from the typed, schema-shaped input, so it
//! exists for every schema-clean record — including one whose domain
//! checks fail (a stale fingerprint is reported against the RECOMPUTED
//! one). Nested objects are [`Json::Object`] (sorted keys, `canonicalize()`);
//! only the outer wrappers the Node reference builds as plain object
//! literals are [`Json::Literal`] (declared key order). Arrays the reference
//! sorts before canonicalizing are sorted with [`locale_order`] — its
//! `localeCompare`, not code-unit order. Every key not applicable to a
//! variant is omitted, exactly as an absent JavaScript property is.

use crate::canonical::locale_order;
use crate::json::Json;
use crate::types::{ContentDigest, Scope};

use super::export::{ExportPayloadInput, ExportedRecordInput};
use super::plan::{
    DeclaredContent, DispositionInput, FieldBasisInput, MappingInput, OwnerDecisionInput,
    PlanPayloadInput, Qualification, RecordUnitInput, RollbackInput, RollbackPlanInput,
    SourceInput, TargetAuthorityInput, TargetInput,
};

pub(crate) fn scope(scope: &Scope) -> Json {
    let mut entries = vec![
        (Json::key("type"), Json::str(scope.scope_type().as_str())),
        (Json::key("id"), Json::str(scope.id())),
    ];
    if let Some(w) = scope.workspace_id() {
        entries.push((Json::key("workspace_id"), Json::str(w.as_str())));
    }
    if let Some(o) = scope.organization_profile_id() {
        entries.push((Json::key("organization_profile_id"), Json::str(o.as_str())));
    }
    Json::Object(entries)
}

pub(crate) fn digest(d: &ContentDigest) -> Json {
    Json::Object(vec![
        (Json::key("algorithm"), Json::str(d.algorithm().as_str())),
        (Json::key("value"), Json::str(d.value())),
    ])
}

pub(crate) fn content(c: &DeclaredContent) -> Json {
    Json::Object(vec![
        (Json::key("media_type"), Json::str(c.media_type.as_str())),
        (Json::key("encoding"), Json::str(c.encoding.as_str())),
        (Json::key("content"), Json::str(c.content.as_str())),
        (Json::key("digest"), digest(&c.digest)),
    ])
}

fn field_basis(fb: &FieldBasisInput) -> Json {
    Json::Object(vec![
        (Json::key("$schema"), Json::str(fb.schema.as_str())),
        (Json::key("id"), Json::str(fb.id.as_str())),
        (Json::key("title"), Json::str(fb.title.as_str())),
        (Json::key("record_type"), Json::str(fb.record_type.as_str())),
        (Json::key("scope"), Json::str(fb.scope.as_str())),
        (
            Json::key("schema_version"),
            Json::str(fb.schema_version.as_str()),
        ),
        (Json::key("origin"), Json::str(fb.origin.as_str())),
        (Json::key("authority"), Json::str(fb.authority.as_str())),
        (Json::key("payload"), Json::str(fb.payload.as_str())),
    ])
}

fn target_authority(a: &TargetAuthorityInput) -> Json {
    Json::Object(vec![
        (Json::key("kind"), Json::str(a.kind.as_str())),
        (
            Json::key("authority_ref"),
            Json::str(a.authority_ref.as_str()),
        ),
        (
            Json::key("decision_ref"),
            Json::str(a.decision_ref.as_str()),
        ),
    ])
}

/// One member of a target record, by its schema field name.
pub(crate) fn target_member(t: &TargetInput, name: &str) -> Option<Json> {
    Some(match name {
        "$schema" => Json::str(t.schema_ref.as_str()),
        "id" => Json::str(t.id.as_str()),
        "title" => Json::str(t.title.as_str()),
        "record_type" => Json::str(t.record_type.as_str()),
        "scope" => scope(&t.scope),
        "schema_version" => Json::Number(1.0),
        "field_basis" => field_basis(&t.field_basis),
        "origin" => Json::Object(vec![
            (Json::key("kind"), Json::str("migrated")),
            (
                Json::key("source_ref"),
                Json::str(t.origin_source_ref.as_str()),
            ),
        ]),
        "authority" => target_authority(&t.authority),
        "payload" => content(&t.payload),
        _ => return None,
    })
}

/// Every schema field of a target record, in schema order.
pub(crate) const TARGET_FIELDS: [&str; 10] = [
    "$schema",
    "id",
    "title",
    "record_type",
    "scope",
    "schema_version",
    "field_basis",
    "origin",
    "authority",
    "payload",
];

pub(crate) fn target(t: &TargetInput) -> Json {
    Json::Object(
        TARGET_FIELDS
            .iter()
            .filter_map(|name| target_member(t, name).map(|v| (Json::key(name), v)))
            .collect(),
    )
}

fn owner_decision(o: &OwnerDecisionInput) -> Json {
    Json::Object(vec![
        (
            Json::key("decision_ref"),
            Json::str(o.decision_ref.as_str()),
        ),
        (Json::key("decided_at"), Json::str(o.decided_at.as_str())),
        (Json::key("reason"), Json::str(o.reason.as_str())),
    ])
}

fn mapping(m: &MappingInput) -> Json {
    let mut entries = vec![
        (Json::key("unit_id"), Json::str(m.unit_id.as_str())),
        (Json::key("disposition"), Json::str(m.disposition.as_str())),
    ];
    match &m.disposition {
        DispositionInput::Migrated {
            target: t,
            owner_decision: o,
        } => {
            entries.push((Json::key("target"), target(t)));
            entries.push((Json::key("owner_decision"), owner_decision(o)));
        }
        DispositionInput::Merged {
            target: t,
            owner_decision: o,
            merge_rule_ref,
        } => {
            entries.push((Json::key("target"), target(t)));
            entries.push((Json::key("owner_decision"), owner_decision(o)));
            entries.push((
                Json::key("merge_rule_ref"),
                Json::str(merge_rule_ref.as_str()),
            ));
        }
        DispositionInput::RetainedTransitional { retained_reason } => {
            entries.push((
                Json::key("retained_reason"),
                Json::str(retained_reason.as_str()),
            ));
        }
    }
    Json::Object(entries)
}

fn record_unit(u: &RecordUnitInput) -> Json {
    Json::Object(vec![
        (Json::key("id"), Json::str(u.id.as_str())),
        (Json::key("unit_ref"), Json::str(u.unit_ref.as_str())),
        (
            Json::key("classification_basis"),
            Json::str(u.classification_basis.as_str()),
        ),
    ])
}

fn source(s: &SourceInput) -> Json {
    let mut entries = vec![
        (Json::key("revision"), Json::str(s.revision.as_str())),
        (Json::key("digest"), digest(&s.digest)),
        (
            Json::key("repository_ref"),
            Json::str(s.repository_ref.as_str()),
        ),
        (
            Json::key("working_tree_clean"),
            Json::Bool(s.working_tree_clean),
        ),
    ];
    match &s.qualification {
        Qualification::Reproducible => {
            entries.push((Json::key("qualification"), Json::str("reproducible")));
        }
        Qualification::NotReproducible { reason } => {
            entries.push((Json::key("qualification"), Json::str("not-reproducible")));
            entries.push((
                Json::key("qualification_reason"),
                Json::str(reason.as_str()),
            ));
        }
    }
    Json::Object(entries)
}

fn rollback(r: &RollbackInput) -> Json {
    let mut entries = vec![(
        Json::key("source_snapshot_ref"),
        Json::str(r.source_snapshot_ref.as_str()),
    )];
    match &r.plan {
        RollbackPlanInput::DeterministicReconstruction {
            deterministic_plan_ref,
        } => {
            entries.push((Json::key("plan"), Json::str("deterministic-reconstruction")));
            entries.push((
                Json::key("deterministic_plan_ref"),
                Json::str(deterministic_plan_ref.as_str()),
            ));
        }
        RollbackPlanInput::VerifiedRestoration {
            restoration_evidence_ref,
        } => {
            entries.push((Json::key("plan"), Json::str("verified-restoration")));
            entries.push((
                Json::key("restoration_evidence_ref"),
                Json::str(restoration_evidence_ref.as_str()),
            ));
        }
    }
    entries.push((Json::key("rewrites_published_history"), Json::Bool(false)));
    Json::Object(entries)
}

/// `computePlanFingerprint`: `source`, `record_units` sorted by
/// `id.localeCompare`, `mappings` sorted by
/// `` `${unit_id}::${target?.id ?? ''}`.localeCompare `` and `rollback` —
/// `verification`, `plan_fingerprint`, `idempotency_key` and `supersedes`
/// are excluded.
pub(crate) fn plan_fingerprint(payload: &PlanPayloadInput) -> ContentDigest {
    let mut units: Vec<&RecordUnitInput> = payload.record_units.iter().collect();
    units.sort_by(|a, b| locale_order(a.id.as_str(), b.id.as_str()));
    let key = |m: &MappingInput| {
        format!(
            "{}::{}",
            m.unit_id.as_str(),
            m.target().map(|t| t.id.as_str()).unwrap_or("")
        )
    };
    let mut mappings: Vec<&MappingInput> = payload.mappings.iter().collect();
    mappings.sort_by(|a, b| locale_order(&key(a), &key(b)));
    let top = Json::Literal(vec![
        ("source", source(&payload.source)),
        (
            "record_units",
            Json::Array(units.into_iter().map(record_unit).collect()),
        ),
        (
            "mappings",
            Json::Array(mappings.into_iter().map(mapping).collect()),
        ),
        ("rollback", rollback(&payload.rollback)),
    ]);
    ContentDigest::of_str(&top.to_canonical_string())
}

/// `computeIdempotencyKey(scope, source)`: the scope's `type`/`id`/
/// `workspace_id` (`""` when absent; `organization_profile_id` is not part
/// of it) and the full source identity.
pub(crate) fn plan_idempotency_key(scope: &Scope, source: &SourceInput) -> ContentDigest {
    let top = Json::Literal(vec![
        (
            "scope",
            Json::Literal(vec![
                ("type", Json::str(scope.scope_type().as_str())),
                ("id", Json::str(scope.id())),
                (
                    "workspace_id",
                    Json::str(scope.workspace_id().map(|w| w.as_str()).unwrap_or("")),
                ),
            ]),
        ),
        ("repository_ref", Json::str(source.repository_ref.as_str())),
        ("revision", Json::str(source.revision.as_str())),
        (
            "digest",
            Json::Literal(vec![
                ("algorithm", Json::str(source.digest.algorithm().as_str())),
                ("value", Json::str(source.digest.value())),
            ]),
        ),
    ]);
    ContentDigest::of_str(&top.to_canonical_string())
}

fn exported_record(r: &ExportedRecordInput) -> Json {
    r.head
        .canonical_record(r.payload.canonical().clone().into_json())
}

/// `computeExportDigest`: `plan_ref`, `plan_fingerprint`, `source`,
/// `records` sorted by `id.localeCompare` and `retained` sorted by
/// `unit_id.localeCompare`.
pub(crate) fn export_digest(payload: &ExportPayloadInput) -> ContentDigest {
    let source = Json::Object(vec![
        (
            Json::key("repository_ref"),
            Json::str(payload.source.repository_ref.as_str()),
        ),
        (
            Json::key("revision"),
            Json::str(payload.source.revision.as_str()),
        ),
        (Json::key("digest"), digest(&payload.source.digest)),
    ]);
    let mut records: Vec<&ExportedRecordInput> = payload.records.iter().collect();
    records.sort_by(|a, b| locale_order(a.head.id.as_str(), b.head.id.as_str()));
    let mut retained: Vec<_> = payload.retained.iter().collect();
    retained.sort_by(|a, b| locale_order(a.unit_id.as_str(), b.unit_id.as_str()));
    let top = Json::Literal(vec![
        ("plan_ref", Json::str(payload.plan_ref.as_str())),
        (
            "plan_fingerprint",
            Json::str(payload.plan_fingerprint.value()),
        ),
        ("source", source),
        (
            "records",
            Json::Array(records.into_iter().map(exported_record).collect()),
        ),
        (
            "retained",
            Json::Array(
                retained
                    .into_iter()
                    .map(|r| {
                        Json::Object(vec![
                            (Json::key("unit_id"), Json::str(r.unit_id.as_str())),
                            (Json::key("reason"), Json::str(r.reason.as_str())),
                        ])
                    })
                    .collect(),
            ),
        ),
    ]);
    ContentDigest::of_str(&top.to_canonical_string())
}

/// `computeExportIdempotencyKey(plan_ref, plan_fingerprint)`.
pub(crate) fn export_idempotency_key(plan_ref: &str, plan_fingerprint: &str) -> ContentDigest {
    let top = Json::Literal(vec![
        ("plan_ref", Json::str(plan_ref)),
        ("plan_fingerprint", Json::str(plan_fingerprint)),
    ]);
    ContentDigest::of_str(&top.to_canonical_string())
}
