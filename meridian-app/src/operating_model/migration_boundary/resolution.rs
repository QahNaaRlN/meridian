//! Transport-to-resolution conversion of the migration contracts' resolver
//! maps: each named map of a fixture bundle is parsed ONCE into the typed
//! catalogue of its own slot (`meridian_core::migration::resolved`), with
//! the field helpers every resolving family shares
//! (`super::super::record_resolution`). A map that is absent or not an
//! object is an empty catalogue (`isObject(map) ? map : {}`); an entry that
//! is not an object never enters it. Unknown keys are kept by name, sorted.

use meridian_core::migration::resolved::{
    DeterministicPlanResponse, DigestResponse, EvidenceResponse, MappingResponse,
    MigrationPlanResponse, PlanResolution, RecordUnitResponse, ResponseCatalogue,
    RestorationEvidenceResponse, RollbackSnapshotResponse, ScopeResponse, SourceContentResponse,
    SourceIdentityResponse, SourceSnapshotResponse, SupersededPlanResponse, TargetResponse,
};
use meridian_core::run_contracts::ResponseField;
use serde_json::{Map, Value};

use super::super::record_resolution::{field, items_field, nullable, text_field, unknown_keys};
use super::{object_member, open_object};

fn boolean(value: Option<&Value>) -> ResponseField<bool> {
    field(value, Value::as_bool)
}

fn digest(value: Option<&Value>) -> ResponseField<DigestResponse> {
    field(value, |v| {
        v.as_object().map(|o| DigestResponse {
            algorithm: text_field(o.get("algorithm")),
            value: text_field(o.get("value")),
            unknown_keys: unknown_keys(o, &["algorithm", "value"]),
        })
    })
}

fn scope(value: Option<&Value>) -> ResponseField<ScopeResponse> {
    field(value, |v| {
        v.as_object().map(|o| ScopeResponse {
            scope_type: text_field(o.get("type")),
            id: text_field(o.get("id")),
            workspace_id: text_field(o.get("workspace_id")),
            organization_profile_id: text_field(o.get("organization_profile_id")),
            unknown_keys: unknown_keys(
                o,
                &["type", "id", "workspace_id", "organization_profile_id"],
            ),
        })
    })
}

fn catalogue<T>(
    owner: &Map<String, Value>,
    key: &str,
    convert: impl Fn(&Map<String, Value>) -> T,
) -> ResponseCatalogue<T> {
    let mut catalogue = ResponseCatalogue::new();
    if let Some(map) = object_member(owner, key) {
        for (reference, entry) in map {
            if let Some(entry) = entry.as_object() {
                catalogue.insert(reference.clone(), convert(entry));
            }
        }
    }
    catalogue
}

fn source_snapshot(o: &Map<String, Value>) -> SourceSnapshotResponse {
    SourceSnapshotResponse {
        record_type: text_field(o.get("record_type")),
        repository_ref: text_field(o.get("repository_ref")),
        revision: text_field(o.get("revision")),
        digest: digest(o.get("digest")),
        working_tree_clean: boolean(o.get("working_tree_clean")),
        unknown_keys: unknown_keys(
            o,
            &[
                "record_type",
                "repository_ref",
                "revision",
                "digest",
                "working_tree_clean",
            ],
        ),
    }
}

fn evidence(o: &Map<String, Value>) -> EvidenceResponse {
    EvidenceResponse {
        record_type: text_field(o.get("record_type")),
        evidence_ref: text_field(o.get("evidence_ref")),
        kind: text_field(o.get("kind")),
        plan_ref: text_field(o.get("plan_ref")),
        plan_fingerprint: text_field(o.get("plan_fingerprint")),
        confirms: boolean(o.get("confirms")),
        unknown_keys: unknown_keys(
            o,
            &[
                "record_type",
                "evidence_ref",
                "kind",
                "plan_ref",
                "plan_fingerprint",
                "confirms",
            ],
        ),
    }
}

fn rollback_snapshot(o: &Map<String, Value>) -> RollbackSnapshotResponse {
    RollbackSnapshotResponse {
        record_type: text_field(o.get("record_type")),
        source_snapshot_ref: text_field(o.get("source_snapshot_ref")),
        repository_ref: text_field(o.get("repository_ref")),
        revision: text_field(o.get("revision")),
        digest: digest(o.get("digest")),
        unknown_keys: unknown_keys(
            o,
            &[
                "record_type",
                "source_snapshot_ref",
                "repository_ref",
                "revision",
                "digest",
            ],
        ),
    }
}

fn deterministic_plan(o: &Map<String, Value>) -> DeterministicPlanResponse {
    DeterministicPlanResponse {
        record_type: text_field(o.get("record_type")),
        deterministic_plan_ref: text_field(o.get("deterministic_plan_ref")),
        source_snapshot_ref: text_field(o.get("source_snapshot_ref")),
        plan_ref: text_field(o.get("plan_ref")),
        plan_fingerprint: text_field(o.get("plan_fingerprint")),
        applicable: boolean(o.get("applicable")),
        unknown_keys: unknown_keys(
            o,
            &[
                "record_type",
                "deterministic_plan_ref",
                "source_snapshot_ref",
                "plan_ref",
                "plan_fingerprint",
                "applicable",
            ],
        ),
    }
}

fn restoration_evidence(o: &Map<String, Value>) -> RestorationEvidenceResponse {
    RestorationEvidenceResponse {
        record_type: text_field(o.get("record_type")),
        evidence_ref: text_field(o.get("evidence_ref")),
        plan_ref: text_field(o.get("plan_ref")),
        plan_fingerprint: text_field(o.get("plan_fingerprint")),
        source_snapshot_ref: text_field(o.get("source_snapshot_ref")),
        confirms: boolean(o.get("confirms")),
        unknown_keys: unknown_keys(
            o,
            &[
                "record_type",
                "evidence_ref",
                "plan_ref",
                "plan_fingerprint",
                "source_snapshot_ref",
                "confirms",
            ],
        ),
    }
}

fn superseded_plan(o: &Map<String, Value>) -> SupersededPlanResponse {
    SupersededPlanResponse {
        record_type: text_field(o.get("record_type")),
        plan_ref: text_field(o.get("plan_ref")),
        scope: scope(o.get("scope")),
        repository_ref: text_field(o.get("repository_ref")),
        revision: text_field(o.get("revision")),
        unknown_keys: unknown_keys(
            o,
            &[
                "record_type",
                "plan_ref",
                "scope",
                "repository_ref",
                "revision",
            ],
        ),
    }
}

/// The six maps of an `instance-data-migration` bundle (or of a
/// workspace qualification's `migration_resolution`).
pub(crate) fn plan_resolution(owner: &Map<String, Value>) -> PlanResolution {
    PlanResolution {
        source_snapshots: catalogue(owner, "source_snapshot_resolution", source_snapshot),
        evidence: catalogue(owner, "evidence_resolution", evidence),
        rollback_snapshots: catalogue(owner, "rollback_snapshot_resolution", rollback_snapshot),
        deterministic_plans: catalogue(owner, "deterministic_plan_resolution", deterministic_plan),
        restoration_evidence: catalogue(
            owner,
            "restoration_evidence_resolution",
            restoration_evidence,
        ),
        superseded_plans: catalogue(owner, "superseded_plan_resolution", superseded_plan),
    }
}

fn objects(value: Option<&Value>) -> impl Iterator<Item = &Map<String, Value>> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
}

fn migration_plan(o: &Map<String, Value>) -> MigrationPlanResponse {
    MigrationPlanResponse {
        record_type: text_field(o.get("record_type")),
        plan_ref: text_field(o.get("plan_ref")),
        plan_fingerprint: text_field(o.get("plan_fingerprint")),
        scope: scope(o.get("scope")),
        source: field(o.get("source"), |v| {
            v.as_object().map(|s| SourceIdentityResponse {
                repository_ref: text_field(s.get("repository_ref")),
                revision: text_field(s.get("revision")),
                digest: digest(s.get("digest")),
            })
        }),
        record_units: objects(o.get("record_units"))
            .map(|u| RecordUnitResponse {
                id: text_field(u.get("id")),
                unit_ref: text_field(u.get("unit_ref")),
            })
            .collect(),
        mappings: objects(o.get("mappings"))
            .map(|m| MappingResponse {
                unit_id: text_field(m.get("unit_id")),
                disposition: text_field(m.get("disposition")),
                target: m
                    .get("target")
                    .and_then(Value::as_object)
                    .map(|t| TargetResponse {
                        id: text_field(t.get("id")),
                        record: open_object(t),
                    }),
                merge_rule_ref: text_field(m.get("merge_rule_ref")),
                retained_reason: text_field(m.get("retained_reason")),
            })
            .collect(),
        unknown_keys: unknown_keys(
            o,
            &[
                "record_type",
                "plan_ref",
                "plan_fingerprint",
                "scope",
                "source",
                "record_units",
                "mappings",
            ],
        ),
    }
}

/// An `instance-canonical-export` bundle's `plan_resolution`.
pub(crate) fn plan_responses(
    owner: &Map<String, Value>,
) -> ResponseCatalogue<MigrationPlanResponse> {
    catalogue(owner, "plan_resolution", migration_plan)
}

fn source_content(o: &Map<String, Value>) -> SourceContentResponse {
    SourceContentResponse {
        record_type: text_field(o.get("record_type")),
        repository_ref: text_field(o.get("repository_ref")),
        revision: text_field(o.get("revision")),
        unit_refs: items_field(o.get("unit_refs")),
        merge_rule_ref: nullable(o.get("merge_rule_ref"), |v| v.as_str().map(str::to_string)),
        media_type: text_field(o.get("media_type")),
        encoding: text_field(o.get("encoding")),
        content: text_field(o.get("content")),
        digest: digest(o.get("digest")),
        unknown_keys: unknown_keys(
            o,
            &[
                "record_type",
                "repository_ref",
                "revision",
                "unit_refs",
                "merge_rule_ref",
                "media_type",
                "encoding",
                "content",
                "digest",
            ],
        ),
    }
}

/// A bundle's `source_content_resolution`.
pub(crate) fn source_contents(
    owner: &Map<String, Value>,
) -> ResponseCatalogue<SourceContentResponse> {
    catalogue(owner, "source_content_resolution", source_content)
}

/// The content-envelope fields of an open payload object.
pub(crate) fn envelope_fields(o: &Map<String, Value>) -> meridian_core::migration::EnvelopeFields {
    let digest = o.get("digest").and_then(Value::as_object);
    meridian_core::migration::EnvelopeFields {
        media_type: text_field(o.get("media_type")),
        encoding: text_field(o.get("encoding")),
        content: text_field(o.get("content")),
        digest_algorithm: digest.map_or(ResponseField::Absent, |d| text_field(d.get("algorithm"))),
        digest_value: digest.map_or(ResponseField::Absent, |d| text_field(d.get("value"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_non_object_map_or_entry_never_resolves_and_unknown_keys_are_sorted() {
        let owner = json!({
            "evidence_resolution": { "a": "not a record", "b": { "zeta": 1, "alpha": 2, "confirms": "yes" } },
            "source_snapshot_resolution": ["not", "a", "map"]
        });
        let resolution = plan_resolution(owner.as_object().unwrap());
        assert!(resolution.evidence.resolve("a").is_none());
        let b = resolution.evidence.resolve("b").unwrap();
        assert_eq!(b.unknown_keys, ["alpha", "zeta"]);
        assert!(matches!(b.confirms, ResponseField::Foreign(_)));
        assert!(resolution.source_snapshots.is_empty());
    }

    #[test]
    fn a_resolved_plan_keeps_only_object_units_and_mappings() {
        let owner = json!({ "plan_resolution": { "p": {
            "record_units": [1, { "id": "u", "unit_ref": "r" }],
            "mappings": ["x", { "unit_id": "u", "disposition": "migrated", "target": { "id": "t" } }]
        } } });
        let plans = plan_responses(owner.as_object().unwrap());
        let p = plans.resolve("p").unwrap();
        assert_eq!(p.record_units.len(), 1);
        assert_eq!(p.mappings.len(), 1);
        assert!(p.mappings[0].target.is_some());
    }
}
