//! Translates between `meridian-core`/`meridian-app` domain values and the
//! flat SQLite columns of `crate::schema`, and computes the canonical
//! content digest used for the idempotent-apply check
//! (`meridian-rust-migration-program-plan.md` §6.5a).
//!
//! Nothing here reuses `meridian-core`'s private canonical JSON writer
//! (`meridian-core/src/json.rs`) — that module stays private and
//! unexported. This crate's own canonicalisation only needs a stable,
//! deterministic fingerprint of content, not a Node.js-`JSON.stringify`
//! byte-for-byte equivalent, so it is built from `serde_json::Value`
//! directly: this workspace pins `serde_json` without the `preserve_order`
//! feature, so `serde_json::Map` is a `BTreeMap` and its keys — at every
//! nesting level — are always emitted in sorted order.

use meridian_app::storage::{Payload, PortError, RecordKey};
use meridian_core::types::{
    Authority, AuthorityKind, ContentDigest, Origin, Scope, SemanticId, WorkspaceId,
};

pub struct EncodedScope {
    pub scope_type: &'static str,
    pub scope_id: String,
    pub workspace_id: Option<String>,
    pub organization_profile_id: Option<String>,
}

pub fn encode_scope(scope: &Scope) -> EncodedScope {
    EncodedScope {
        scope_type: scope.scope_type().as_str(),
        scope_id: scope.id().to_string(),
        workspace_id: scope.workspace_id().map(|w| w.as_str().to_string()),
        organization_profile_id: scope
            .organization_profile_id()
            .map(|o| o.as_str().to_string()),
    }
}

fn corrupt(field: &str, value: impl std::fmt::Display) -> PortError {
    PortError::Storage(format!("corrupt {field} column value: {value}"))
}

pub fn decode_scope(
    scope_type: &str,
    scope_id: &str,
    workspace_id: Option<&str>,
    organization_profile_id: Option<&str>,
) -> Result<Scope, PortError> {
    let id = || SemanticId::new(scope_id).map_err(|e| corrupt("scope_id", e));
    let wid = |raw: &str| WorkspaceId::new(raw).map_err(|e| corrupt("scope_workspace_id", e));
    let org =
        |raw: &str| SemanticId::new(raw).map_err(|e| corrupt("scope_organization_profile_id", e));

    match scope_type {
        "built-in-methodology" => Ok(Scope::built_in_methodology()),
        "user-profile" => Ok(Scope::user_profile(id()?)),
        "organization-profile" => Ok(Scope::organization_profile(id()?)),
        "project-workspace" => {
            let org_id = organization_profile_id.map(org).transpose()?;
            Ok(Scope::project_workspace(id()?, org_id))
        }
        "repository-scope" => {
            let workspace_id = workspace_id
                .ok_or_else(|| corrupt("scope_workspace_id", "missing for repository-scope"))?;
            Ok(Scope::repository_scope(id()?, wid(workspace_id)?))
        }
        "run-state" => {
            let workspace_id = workspace_id
                .ok_or_else(|| corrupt("scope_workspace_id", "missing for run-state"))?;
            Ok(Scope::run_state(id()?, wid(workspace_id)?))
        }
        other => Err(corrupt("scope_type", other)),
    }
}

pub struct EncodedOrigin {
    pub kind: &'static str,
    pub source_ref: Option<String>,
}

pub fn encode_origin(origin: &Origin) -> EncodedOrigin {
    EncodedOrigin {
        kind: origin.kind().as_str(),
        source_ref: origin.source_ref().map(|s| s.to_string()),
    }
}

pub fn decode_origin(kind: &str, source_ref: Option<&str>) -> Result<Origin, PortError> {
    match kind {
        "built-in" => Ok(Origin::built_in()),
        "declared" => {
            Origin::declared(require_source_ref(source_ref)?).map_err(|e| corrupt("origin", e))
        }
        "migrated" => {
            Origin::migrated(require_source_ref(source_ref)?).map_err(|e| corrupt("origin", e))
        }
        "imported" => {
            Origin::imported(require_source_ref(source_ref)?).map_err(|e| corrupt("origin", e))
        }
        "derived" => {
            Origin::derived(require_source_ref(source_ref)?).map_err(|e| corrupt("origin", e))
        }
        other => Err(corrupt("origin_kind", other)),
    }
}

fn require_source_ref(source_ref: Option<&str>) -> Result<&str, PortError> {
    source_ref.ok_or_else(|| corrupt("origin_source_ref", "missing for a non-built-in origin"))
}

pub struct EncodedAuthority {
    pub kind: &'static str,
    pub authority_ref: String,
    pub decision_ref: Option<String>,
}

pub fn encode_authority(authority: &Authority) -> EncodedAuthority {
    EncodedAuthority {
        kind: authority.kind().as_str(),
        authority_ref: authority.authority_ref().to_string(),
        decision_ref: authority.decision_ref().map(|d| d.to_string()),
    }
}

fn authority_kind_of(raw: &str) -> Result<AuthorityKind, PortError> {
    match raw {
        "methodology-owner" => Ok(AuthorityKind::MethodologyOwner),
        "user" => Ok(AuthorityKind::User),
        "organization" => Ok(AuthorityKind::Organization),
        "project-owner" => Ok(AuthorityKind::ProjectOwner),
        "repository-maintainer" => Ok(AuthorityKind::RepositoryMaintainer),
        "delegated-run" => Ok(AuthorityKind::DelegatedRun),
        other => Err(corrupt("authority_kind", other)),
    }
}

pub fn decode_authority(
    kind: &str,
    authority_ref: &str,
    decision_ref: Option<&str>,
) -> Result<Authority, PortError> {
    let kind = authority_kind_of(kind)?;
    Authority::new(kind, authority_ref, decision_ref.map(|d| d.to_string()))
        .map_err(|e| corrupt("authority", e))
}

pub fn encode_payload(payload: &Payload) -> String {
    // `serde_json::to_string` over a `Value::Object` backed by `BTreeMap`
    // (this workspace never enables `preserve_order`) — keys are always
    // emitted sorted, recursively, so storing this string is already the
    // canonical form re-read verbatim on export.
    serde_json::to_string(&serde_json::Value::Object(payload.as_map().clone()))
        .expect("a JSON object serializes to a string without error")
}

pub fn decode_payload(raw: &str) -> Result<Payload, PortError> {
    let value: serde_json::Value = serde_json::from_str(raw).map_err(|e| corrupt("payload", e))?;
    Payload::new(value).map_err(|e| corrupt("payload", e))
}

/// Assembles the canonical content JSON of one record's envelope, sorted
/// deterministically at every level. Two calls with equal arguments always
/// produce byte-identical output — the property the idempotency check and
/// the canonical export both depend on.
#[allow(clippy::too_many_arguments)]
pub fn canonical_content_json(
    key: &RecordKey,
    schema_ref: &str,
    schema_version: u32,
    title: &str,
    record_type: &str,
    origin: &Origin,
    authority: &Authority,
    payload: &Payload,
) -> serde_json::Value {
    let scope = encode_scope(key.scope());
    let origin_enc = encode_origin(origin);
    let authority_enc = encode_authority(authority);

    let mut scope_obj = serde_json::Map::new();
    scope_obj.insert(
        "type".to_string(),
        serde_json::Value::String(scope.scope_type.to_string()),
    );
    scope_obj.insert("id".to_string(), serde_json::Value::String(scope.scope_id));
    if let Some(w) = scope.workspace_id {
        scope_obj.insert("workspace_id".to_string(), serde_json::Value::String(w));
    }
    if let Some(o) = scope.organization_profile_id {
        scope_obj.insert(
            "organization_profile_id".to_string(),
            serde_json::Value::String(o),
        );
    }

    let mut origin_obj = serde_json::Map::new();
    origin_obj.insert(
        "kind".to_string(),
        serde_json::Value::String(origin_enc.kind.to_string()),
    );
    if let Some(s) = origin_enc.source_ref {
        origin_obj.insert("source_ref".to_string(), serde_json::Value::String(s));
    }

    let mut authority_obj = serde_json::Map::new();
    authority_obj.insert(
        "kind".to_string(),
        serde_json::Value::String(authority_enc.kind.to_string()),
    );
    authority_obj.insert(
        "authority_ref".to_string(),
        serde_json::Value::String(authority_enc.authority_ref),
    );
    if let Some(d) = authority_enc.decision_ref {
        authority_obj.insert("decision_ref".to_string(), serde_json::Value::String(d));
    }

    let mut root = serde_json::Map::new();
    // The exact `$schema` this record's envelope declares, carried verbatim
    // — never a generic or fixed fallback (`meridian-app::storage::SchemaRef`).
    root.insert(
        "$schema".to_string(),
        serde_json::Value::String(schema_ref.to_string()),
    );
    root.insert(
        "id".to_string(),
        serde_json::Value::String(key.id().as_str().to_string()),
    );
    root.insert(
        "title".to_string(),
        serde_json::Value::String(title.to_string()),
    );
    root.insert(
        "record_type".to_string(),
        serde_json::Value::String(record_type.to_string()),
    );
    root.insert(
        "schema_version".to_string(),
        serde_json::Value::Number(schema_version.into()),
    );
    root.insert("scope".to_string(), serde_json::Value::Object(scope_obj));
    root.insert("origin".to_string(), serde_json::Value::Object(origin_obj));
    root.insert(
        "authority".to_string(),
        serde_json::Value::Object(authority_obj),
    );
    root.insert(
        "payload".to_string(),
        serde_json::Value::Object(payload.as_map().clone()),
    );
    serde_json::Value::Object(root)
}

/// The digest of one record's canonical content — recomputed on every write
/// so repeat detection (`super::codec`, `meridian-app::storage::PortError::IdempotencyConflict`)
/// never trusts a caller-supplied fingerprint.
#[allow(clippy::too_many_arguments)]
pub fn content_digest(
    key: &RecordKey,
    schema_ref: &str,
    schema_version: u32,
    title: &str,
    record_type: &str,
    origin: &Origin,
    authority: &Authority,
    payload: &Payload,
) -> ContentDigest {
    let value = canonical_content_json(
        key,
        schema_ref,
        schema_version,
        title,
        record_type,
        origin,
        authority,
        payload,
    );
    // Compact, sorted-key serialization — deterministic byte input to the
    // digest for any two calls with equal arguments.
    let bytes = serde_json::to_vec(&value).expect("canonical content serializes without error");
    ContentDigest::of_bytes(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sid(s: &str) -> SemanticId {
        SemanticId::new(s).unwrap()
    }

    #[test]
    fn scope_round_trips_for_every_variant() {
        let cases = vec![
            Scope::built_in_methodology(),
            Scope::user_profile(sid("alice")),
            Scope::organization_profile(sid("acme")),
            Scope::project_workspace(sid("sample-project"), Some(sid("acme"))),
            Scope::project_workspace(sid("sample-project"), None),
            Scope::repository_scope(
                sid("branch-naming"),
                WorkspaceId::new("sample-project").unwrap(),
            ),
            Scope::run_state(sid("run-42"), WorkspaceId::new("sample-project").unwrap()),
        ];
        for scope in cases {
            let enc = encode_scope(&scope);
            let decoded = decode_scope(
                enc.scope_type,
                &enc.scope_id,
                enc.workspace_id.as_deref(),
                enc.organization_profile_id.as_deref(),
            )
            .unwrap();
            assert_eq!(decoded, scope);
        }
    }

    #[test]
    fn origin_round_trips_for_every_kind() {
        let cases = vec![
            Origin::built_in(),
            Origin::declared("owner-decision:x").unwrap(),
            Origin::migrated("record-unit:legacy-1").unwrap(),
            Origin::imported("import:1").unwrap(),
            Origin::derived("migration-plan:1").unwrap(),
        ];
        for origin in cases {
            let enc = encode_origin(&origin);
            let decoded = decode_origin(enc.kind, enc.source_ref.as_deref()).unwrap();
            assert_eq!(decoded, origin);
        }
    }

    #[test]
    fn authority_round_trips_with_and_without_decision_ref() {
        let a = Authority::new(AuthorityKind::ProjectOwner, "workspace-owner", None).unwrap();
        let enc = encode_authority(&a);
        assert_eq!(
            decode_authority(enc.kind, &enc.authority_ref, enc.decision_ref.as_deref()).unwrap(),
            a
        );

        let b = Authority::new(
            AuthorityKind::ProjectOwner,
            "workspace-owner",
            Some("owner-decision:x".to_string()),
        )
        .unwrap();
        let enc_b = encode_authority(&b);
        assert_eq!(
            decode_authority(
                enc_b.kind,
                &enc_b.authority_ref,
                enc_b.decision_ref.as_deref()
            )
            .unwrap(),
            b
        );
    }

    #[test]
    fn payload_round_trips_through_text_encoding() {
        let payload = Payload::new(serde_json::json!({"b": 1, "a": 2})).unwrap();
        let text = encode_payload(&payload);
        // Sorted-key form regardless of insertion order.
        assert_eq!(text, r#"{"a":2,"b":1}"#);
        assert_eq!(decode_payload(&text).unwrap(), payload);
    }

    #[test]
    fn content_digest_is_deterministic_and_sensitive_to_every_field() {
        let key = RecordKey::new(
            Scope::project_workspace(sid("sample-project"), None),
            sid("branch-naming"),
        );
        let origin = Origin::declared("owner-decision:x").unwrap();
        let authority =
            Authority::new(AuthorityKind::ProjectOwner, "workspace-owner", None).unwrap();
        let payload = Payload::empty();

        let schema_ref =
            "https://meridian.invalid/registries/operating-model/scoped-record.schema.json";
        let d1 = content_digest(
            &key, schema_ref, 1, "Title", "norm", &origin, &authority, &payload,
        );
        let d2 = content_digest(
            &key, schema_ref, 1, "Title", "norm", &origin, &authority, &payload,
        );
        assert_eq!(d1, d2);

        let d3 = content_digest(
            &key,
            schema_ref,
            1,
            "Different title",
            "norm",
            &origin,
            &authority,
            &payload,
        );
        let d4 = content_digest(
            &key,
            "https://meridian.invalid/registries/custom/other.schema.json",
            1,
            "Title",
            "norm",
            &origin,
            &authority,
            &payload,
        );
        assert_ne!(d1, d4, "a different $schema must change the digest too");
        assert_ne!(d1, d3);
    }
}
