//! Conversions between accepted domain values and the storage port
//! (`meridian-rust-migration-program-plan.md` §5.22.4, item 2): an accepted
//! plan target or canonical record becomes a strict [`PutRecordRequest`];
//! a stored [`ManagedRecord`] becomes the core's [`ReadBackRecord`].
//! Nothing here reads or writes anything.

use meridian_core::canonical::CanonicalJson;
use meridian_core::migration::record::RecordHead;
use meridian_core::migration::write_set::{ReadBackRecord, WriteSetEntry};
use meridian_core::run_contracts::{RecordAuthority, RecordOrigin};
use meridian_core::types::{Authority, ContentDigest, NonEmptyString, Origin, OriginKind};
use serde_json::{json, Map, Value};

use crate::operating_model::migration_boundary::canonical;
use crate::storage::{
    IdempotencyKey, ManagedRecord, Payload, PutRecordRequest, RecordKey, RecordSchemaVersion,
    SchemaRef,
};

fn key_digest(parts: &[&str]) -> Result<IdempotencyKey, String> {
    IdempotencyKey::new(ContentDigest::of_str(&parts.join("\n")).value())
        .map_err(|e| format!("idempotency key: {e}"))
}

/// The stored payload of one accepted plan target: its content container.
pub(crate) fn target_payload(entry: &WriteSetEntry) -> Value {
    let target = entry.target();
    json!({
        "media_type": target.payload.media_type.as_str(),
        "encoding": target.payload.encoding.as_str(),
        "content": target.payload.content.as_str(),
        "digest": {
            "algorithm": target.payload.digest.algorithm().as_str(),
            "value": target.payload.digest.value(),
        },
    })
}

/// The strict write of one accepted plan target. Its idempotency key is a
/// deterministic function of the plan's own idempotency key and the
/// record's identity.
pub(crate) fn target_request(
    entry: &WriteSetEntry,
    plan_key: &ContentDigest,
) -> Result<PutRecordRequest, String> {
    let target = entry.target();
    let at = |field: &str| format!("target \"{}\" {field}", target.id.as_str());
    let key = RecordKey::new(target.scope.clone(), target.id.clone());
    let payload = target_payload(entry);
    let idempotency = key_digest(&["frozen-instance", plan_key.value(), &key.storage_key()])?;
    PutRecordRequest::new(
        key,
        SchemaRef::new(target.schema_ref.as_str())
            .map_err(|e| format!("{}: {e}", at("$schema")))?,
        RecordSchemaVersion::CURRENT,
        NonEmptyString::new(target.title.as_str()).map_err(|e| format!("{}: {e}", at("title")))?,
        target.record_type.clone(),
        Origin::migrated(target.origin_source_ref.as_str())
            .map_err(|e| format!("{}: {e}", at("origin")))?,
        Authority::new(
            target.authority.kind,
            target.authority.authority_ref.as_str(),
            Some(target.authority.decision_ref.as_str().to_string()),
        )
        .map_err(|e| format!("{}: {e}", at("authority")))?,
        Payload::new(payload).map_err(|e| format!("{}: {e}", at("payload")))?,
        idempotency,
    )
    .map_err(|e| format!("{}: {e}", at("envelope")))
}

/// What storage reported back, in the core's comparison shape.
pub(crate) fn read_back(record: &ManagedRecord) -> ReadBackRecord {
    ReadBackRecord {
        scope: record.key().scope().clone(),
        id: record.key().id().clone(),
        schema: record.schema().to_string(),
        schema_version: record.schema_version().as_u32(),
        title: record.title().to_string(),
        record_type: record.record_type().clone(),
        origin: record.origin().clone(),
        authority: record.authority().clone(),
        payload: canonical(&Value::Object(record.payload().as_map().clone())),
    }
}

fn origin(origin: &RecordOrigin) -> Result<Origin, String> {
    let (kind, source_ref) = match origin {
        RecordOrigin::BuiltIn => return Ok(Origin::built_in()),
        RecordOrigin::Sourced { kind, source_ref } => (kind.kind(), source_ref.as_str()),
    };
    let built = match kind {
        OriginKind::Declared => Origin::declared(source_ref),
        OriginKind::Migrated => Origin::migrated(source_ref),
        OriginKind::Imported => Origin::imported(source_ref),
        OriginKind::Derived => Origin::derived(source_ref),
        OriginKind::BuiltIn => {
            return Err("origin: a sourced origin cannot be built-in".to_string())
        }
    };
    built.map_err(|e| format!("origin: {e}"))
}

fn authority(authority: &RecordAuthority) -> Result<Authority, String> {
    Authority::new(
        authority.kind,
        authority.authority_ref.as_str(),
        authority
            .decision_ref
            .as_ref()
            .map(|d| d.as_str().to_string()),
    )
    .map_err(|e| format!("authority: {e}"))
}

/// The strict write of one accepted canonical record. Its idempotency key
/// is a deterministic function of the record's whole canonical content, so
/// importing the same content again is recognised, never duplicated.
pub(crate) fn canonical_request(
    head: &RecordHead,
    payload: &Map<String, Value>,
    whole: &Value,
) -> Result<PutRecordRequest, String> {
    let key = RecordKey::new(head.scope.clone(), head.id.clone());
    let content: CanonicalJson = canonical(whole);
    let idempotency = key_digest(&["canonical-record", content.digest().value()])?;
    PutRecordRequest::new(
        key,
        SchemaRef::new(head.declared_schema.as_str()).map_err(|e| format!("$schema: {e}"))?,
        RecordSchemaVersion::CURRENT,
        NonEmptyString::new(head.title.as_str()).map_err(|e| format!("title: {e}"))?,
        head.record_type.clone(),
        origin(&head.origin)?,
        authority(&head.authority)?,
        Payload::new(Value::Object(payload.clone())).map_err(|e| format!("payload: {e}"))?,
        idempotency,
    )
    .map_err(|e| format!("envelope: {e}"))
}
