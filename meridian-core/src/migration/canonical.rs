//! Byte-compatible canonicalization for
//! [`plan_fingerprint`](super::checks::compute_plan_fingerprint) and
//! [`idempotency_key`](super::checks::compute_idempotency_key)
//! (`instance-data-migration.md` §7).
//!
//! This builds EXACTLY the same JSON projection, field names, key
//! ordering and presence rules as `computePlanFingerprint`/
//! `computeIdempotencyKey` (`scripts/lib/instance-data-migration.mjs`) —
//! using `crate::json`, a minimal private canonical-JSON writer (see that
//! module's documentation for why this is not the forbidden "transport
//! JSON structures and parsers"), not a separate Rust-only format. The
//! resulting canonical text, SHA-256-hashed as UTF-8 bytes exactly as
//! `createHash('sha256').update(canonical)` hashes a JS string, is
//! byte-for-byte what the Node reference would compute for the same
//! logical plan content — proven by the fixed reference hashes in
//! `meridian-core/tests/migration_fingerprint_reference.rs`, obtained by
//! calling the real Node.js functions.
//!
//! Field-by-field mapping to `instance-data-migration.schema.json`
//! (every key not applicable to the given variant is OMITTED, not written
//! as `null`, exactly as an absent JSON key would be for a value Node never
//! set):
//!
//! - `source_state`: `revision`, `digest{algorithm,value}`,
//!   `repository_ref`, `working_tree_clean`, `qualification`,
//!   `qualification_reason` (present only for `NotReproducible`);
//! - `record_unit`: `id`, `unit_ref`, `classification_basis`;
//! - `mapping`: `unit_id`, `disposition`, plus exactly the fields the
//!   schema requires per disposition (`target`, `owner_decision`,
//!   `merge_rule_ref`, `retained_reason`);
//! - `target_record`: `$schema`, `id`, `title`, `record_type`, `scope`,
//!   `schema_version` (always the literal `1`), `field_basis`, `origin`,
//!   `authority`, `payload`;
//! - `rollback`: `source_snapshot_ref`, `plan`,
//!   `deterministic_plan_ref`/`restoration_evidence_ref` (per variant),
//!   `rewrites_published_history`.
//!
//! Every one of these nested objects is written with
//! [`Json::Object`](crate::json::Json::Object) (sorted keys), matching
//! `canonicalize()`. Only the two OUTER wrapper objects
//! `computePlanFingerprint`/`computeIdempotencyKey` build as JavaScript
//! object literals — never passed through `canonicalize()` — use
//! [`Json::Literal`](crate::json::Json::Literal) (declared key order):
//! `{source, record_units, mappings, rollback}` and
//! `{scope: {type, id, workspace_id}, repository_ref, revision, digest:
//! {algorithm, value}}` (`computeIdempotencyKey`'s `scope`/`digest`
//! sub-objects are themselves literals too).

use crate::types::{Authority, ContentDigest, Scope};

use super::types::{
    ContentEnvelope, Disposition, FieldBasis, FieldBasisValue, Mapping, OwnerDecision,
    Qualification, RecordUnit, Rollback, RollbackPlan, SourceState, TargetOrigin, TargetRecord,
};
use crate::json::Json;

fn json_scope(scope: &Scope) -> Json {
    let mut entries: Vec<(Vec<u16>, Json)> = vec![
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

fn json_authority(authority: &Authority) -> Json {
    let mut entries = vec![
        (Json::key("kind"), Json::str(authority.kind().as_str())),
        (
            Json::key("authority_ref"),
            Json::str(authority.authority_ref()),
        ),
    ];
    if let Some(d) = authority.decision_ref() {
        entries.push((Json::key("decision_ref"), Json::str(d)));
    }
    Json::Object(entries)
}

fn json_field_basis(fb: &FieldBasis) -> Json {
    let v = |x: FieldBasisValue| {
        if matches!(x, FieldBasisValue::Preserved) {
            "preserved"
        } else {
            "assigned"
        }
    };
    Json::Object(vec![
        (Json::key("$schema"), Json::str(v(fb.schema()))),
        (Json::key("id"), Json::str(v(fb.id()))),
        (Json::key("title"), Json::str(v(fb.title()))),
        (Json::key("record_type"), Json::str(v(fb.record_type()))),
        (Json::key("scope"), Json::str(v(fb.scope()))),
        (
            Json::key("schema_version"),
            Json::str(v(fb.schema_version())),
        ),
        (Json::key("origin"), Json::str(v(fb.origin()))),
        (Json::key("authority"), Json::str(v(fb.authority()))),
        (Json::key("payload"), Json::str(v(fb.payload()))),
    ])
}

fn json_content_envelope(c: &ContentEnvelope) -> Json {
    Json::Object(vec![
        (Json::key("media_type"), Json::str(c.media_type())),
        (Json::key("encoding"), Json::str(c.encoding().as_str())),
        (Json::key("content"), Json::str(c.content())),
        (Json::key("digest"), json_digest(c.digest())),
    ])
}

fn json_digest(d: &ContentDigest) -> Json {
    Json::Object(vec![
        (Json::key("algorithm"), Json::str(d.algorithm().as_str())),
        (Json::key("value"), Json::str(d.value())),
    ])
}

fn json_target_origin(o: &TargetOrigin) -> Json {
    Json::Object(vec![
        (Json::key("kind"), Json::str("migrated")),
        (Json::key("source_ref"), Json::str(o.source_ref())),
    ])
}

fn json_target(t: &TargetRecord) -> Json {
    Json::Object(vec![
        (Json::key("$schema"), Json::str(t.schema_ref())),
        (Json::key("id"), Json::str(t.id().as_str())),
        (Json::key("title"), Json::str(t.title())),
        (
            Json::key("record_type"),
            Json::str(t.record_type().as_str()),
        ),
        (Json::key("scope"), json_scope(t.scope())),
        (Json::key("schema_version"), Json::Number(1.0)),
        (Json::key("field_basis"), json_field_basis(t.field_basis())),
        (Json::key("origin"), json_target_origin(t.origin())),
        (Json::key("authority"), json_authority(t.authority())),
        (Json::key("payload"), json_content_envelope(t.payload())),
    ])
}

fn json_owner_decision(o: &OwnerDecision) -> Json {
    Json::Object(vec![
        (Json::key("decision_ref"), Json::str(o.decision_ref())),
        (Json::key("decided_at"), Json::str(o.decided_at().as_str())),
        (Json::key("reason"), Json::str(o.reason())),
    ])
}

fn json_mapping(m: &Mapping) -> Json {
    let mut entries = vec![(Json::key("unit_id"), Json::str(m.unit_id().as_str()))];
    match m.disposition() {
        Disposition::Migrated {
            target,
            owner_decision,
        } => {
            entries.push((Json::key("disposition"), Json::str("migrated")));
            entries.push((Json::key("target"), json_target(target)));
            entries.push((
                Json::key("owner_decision"),
                json_owner_decision(owner_decision),
            ));
        }
        Disposition::Merged {
            target,
            owner_decision,
            merge_rule_ref,
        } => {
            entries.push((Json::key("disposition"), Json::str("merged")));
            entries.push((Json::key("target"), json_target(target)));
            entries.push((
                Json::key("owner_decision"),
                json_owner_decision(owner_decision),
            ));
            entries.push((
                Json::key("merge_rule_ref"),
                Json::str(merge_rule_ref.as_str()),
            ));
        }
        Disposition::RetainedTransitional { retained_reason } => {
            entries.push((Json::key("disposition"), Json::str("retained-transitional")));
            entries.push((
                Json::key("retained_reason"),
                Json::str(retained_reason.as_str()),
            ));
        }
    }
    Json::Object(entries)
}

fn json_record_unit(u: &RecordUnit) -> Json {
    Json::Object(vec![
        (Json::key("id"), Json::str(u.id().as_str())),
        (Json::key("unit_ref"), Json::str(u.unit_ref())),
        (
            Json::key("classification_basis"),
            Json::str(u.classification_basis()),
        ),
    ])
}

fn json_source(s: &SourceState) -> Json {
    let mut entries = vec![
        (Json::key("revision"), Json::str(s.revision().as_str())),
        (Json::key("digest"), json_digest(s.digest())),
        (
            Json::key("repository_ref"),
            Json::str(s.repository_ref().as_str()),
        ),
        (
            Json::key("working_tree_clean"),
            Json::Bool(s.working_tree_clean()),
        ),
    ];
    match s.qualification() {
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

fn json_rollback(r: &Rollback) -> Json {
    let mut entries = vec![(
        Json::key("source_snapshot_ref"),
        Json::str(r.source_snapshot_ref().as_str()),
    )];
    match r.plan() {
        RollbackPlan::DeterministicReconstruction {
            deterministic_plan_ref,
        } => {
            entries.push((Json::key("plan"), Json::str("deterministic-reconstruction")));
            entries.push((
                Json::key("deterministic_plan_ref"),
                Json::str(deterministic_plan_ref.as_str()),
            ));
        }
        RollbackPlan::VerifiedRestoration {
            restoration_evidence_ref,
        } => {
            entries.push((Json::key("plan"), Json::str("verified-restoration")));
            entries.push((
                Json::key("restoration_evidence_ref"),
                Json::str(restoration_evidence_ref.as_str()),
            ));
        }
    }
    entries.push((
        Json::key("rewrites_published_history"),
        Json::Bool(r.rewrites_published_history()),
    ));
    Json::Object(entries)
}

/// `record_units` sorted by `id`, matching `computePlanFingerprint`'s
/// `.sort((a, b) => String(a.id ?? '').localeCompare(String(b.id ?? '')))`
/// — for the plain-ASCII semantic ids this crate's [`RecordUnit::id`]
/// admits, `String::localeCompare` and Rust's byte-lexicographic `Ord`
/// agree.
fn sorted_record_units(record_units: &[RecordUnit]) -> Vec<&RecordUnit> {
    let mut units: Vec<&RecordUnit> = record_units.iter().collect();
    units.sort_by_key(|u| u.id().as_str().to_string());
    units
}

/// `mappings` sorted by `` `${unit_id}::${target?.id ?? ''}` ``, matching
/// `computePlanFingerprint`'s own stable sort key.
fn sorted_mappings(mappings: &[Mapping]) -> Vec<&Mapping> {
    let mut sorted: Vec<&Mapping> = mappings.iter().collect();
    sorted.sort_by_key(|m| {
        let target_id = m.target().map(|t| t.id().as_str()).unwrap_or("");
        format!("{}::{}", m.unit_id().as_str(), target_id)
    });
    sorted
}

/// The canonical projection `plan_fingerprint` is computed from: `source`
/// (including digest), `record_units` (sorted by `id`), `mappings` (sorted
/// by `unit_id::target.id`) and `rollback` — `verification`,
/// `plan_fingerprint`, `idempotency_key` and `supersedes` are deliberately
/// excluded, exactly as `computePlanFingerprint` excludes them.
pub(super) fn plan_fingerprint_preimage(
    source: &SourceState,
    record_units: &[RecordUnit],
    mappings: &[Mapping],
    rollback: &Rollback,
) -> String {
    let units_json = Json::Array(
        sorted_record_units(record_units)
            .into_iter()
            .map(json_record_unit)
            .collect(),
    );
    let mappings_json = Json::Array(
        sorted_mappings(mappings)
            .into_iter()
            .map(json_mapping)
            .collect(),
    );
    let top = Json::Literal(vec![
        ("source", json_source(source)),
        ("record_units", units_json),
        ("mappings", mappings_json),
        ("rollback", json_rollback(rollback)),
    ]);
    top.to_canonical_string()
}

/// `idempotency_key` is a deterministic function of the plan's own scope
/// (`type`/`id`/`workspace_id` — `workspace_id` always present as a key,
/// defaulting to `""` when the scope carries none, exactly as `s.workspace_id
/// ?? ''` does; `organization_profile_id` is never part of this
/// projection) and the canonical identity of its pinned source
/// (`repository_ref`, `revision` AND the full digest) —
/// `qualification`/`working_tree_clean` are deliberately excluded, exactly
/// as `computeIdempotencyKey` excludes them.
pub(super) fn idempotency_key_preimage(scope: &Scope, source: &SourceState) -> String {
    let scope_literal = Json::Literal(vec![
        ("type", Json::str(scope.scope_type().as_str())),
        ("id", Json::str(scope.id())),
        (
            "workspace_id",
            Json::str(scope.workspace_id().map(|w| w.as_str()).unwrap_or("")),
        ),
    ]);
    let digest_literal = Json::Literal(vec![
        ("algorithm", Json::str(source.digest().algorithm().as_str())),
        ("value", Json::str(source.digest().value())),
    ]);
    let top = Json::Literal(vec![
        ("scope", scope_literal),
        (
            "repository_ref",
            Json::str(source.repository_ref().as_str()),
        ),
        ("revision", Json::str(source.revision().as_str())),
        ("digest", digest_literal),
    ]);
    top.to_canonical_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idempotency_key_preimage_matches_the_documented_literal_shape() {
        let scope = Scope::project_workspace(
            crate::types::SemanticId::new("sample-project").unwrap(),
            None,
        );
        let source = SourceState::new(
            crate::types::Revision::new("rev-1").unwrap(),
            ContentDigest::of_str("d"),
            crate::types::EvidenceRef::new("instance:sample").unwrap(),
            true,
            Qualification::Reproducible,
        )
        .unwrap();
        let preimage = idempotency_key_preimage(&scope, &source);
        assert_eq!(
            preimage,
            format!(
                r#"{{"scope":{{"type":"project-workspace","id":"sample-project","workspace_id":""}},"repository_ref":"instance:sample","revision":"rev-1","digest":{{"algorithm":"sha-256","value":"{}"}}}}"#,
                ContentDigest::of_str("d").value()
            )
        );
    }
}
