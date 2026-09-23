//! Closed transport DTOs of `instance-canonical-export.schema.json` and
//! their total conversion into `meridian_core::migration::export::ExportInput`.
//! An exported record's `payload` is an open object (the envelope schema says
//! only `"type": "object"`), so it is kept as its members plus the typed
//! reading of its content-envelope fields — never re-read later.

use meridian_core::migration::export::{
    ExportInput, ExportPayloadInput, ExportSourceInput, ExportedPayload, ExportedRecordInput,
    RetainedInput,
};
use serde::Deserialize;
use serde_json::{Map, Value};

use super::super::migration_boundary::head::{
    digest, head, sha256_hex, DigestDto, HeadParts, ScopeDto,
};
use super::super::migration_boundary::open_object;
use super::super::migration_boundary::resolution::envelope_fields;
use super::super::run_contract_boundary::envelope::{
    semantic_id, text, AuthorityDto, Converted, OriginDto,
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ContainerDto {
    #[serde(rename = "$schema", default)]
    _schema: Option<String>,
    #[serde(rename = "schema_version")]
    _schema_version: u64,
    #[serde(rename = "registry_id")]
    _registry_id: String,
    #[serde(rename = "title")]
    _title: String,
    exports: Vec<ExportEntryDto>,
}

impl ContainerDto {
    pub(super) fn into_inputs(self) -> Converted<Vec<ExportInput>> {
        self.exports
            .into_iter()
            .enumerate()
            .map(|(i, e)| e.into_input().map_err(|m| format!("exports[{i}]: {m}")))
            .collect()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportEntryDto {
    #[serde(rename = "$schema")]
    declared_schema: String,
    schema_version: u64,
    id: String,
    title: String,
    record_type: String,
    scope: ScopeDto,
    origin: OriginDto,
    authority: AuthorityDto,
    payload: PayloadDto,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceDto {
    repository_ref: String,
    revision: String,
    digest: DigestDto,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetainedDto {
    unit_id: String,
    reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordDto {
    #[serde(rename = "$schema")]
    declared_schema: String,
    schema_version: u64,
    id: String,
    title: String,
    record_type: String,
    scope: ScopeDto,
    origin: OriginDto,
    authority: AuthorityDto,
    payload: Map<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PayloadDto {
    plan_ref: String,
    plan_fingerprint: String,
    source: SourceDto,
    records: Vec<RecordDto>,
    retained: Vec<RetainedDto>,
    idempotency_key: String,
    digest: String,
}

impl RecordDto {
    fn into_input(self) -> Converted<ExportedRecordInput> {
        let payload =
            ExportedPayload::new(open_object(&self.payload), envelope_fields(&self.payload));
        Ok(ExportedRecordInput {
            head: head(HeadParts {
                declared_schema: self.declared_schema,
                schema_version: self.schema_version,
                id: self.id,
                title: self.title,
                record_type: self.record_type,
                scope: self.scope,
                origin: self.origin,
                authority: self.authority,
            })?,
            payload,
        })
    }
}

impl ExportEntryDto {
    fn into_input(self) -> Converted<ExportInput> {
        let p = self.payload;
        Ok(ExportInput {
            head: head(HeadParts {
                declared_schema: self.declared_schema,
                schema_version: self.schema_version,
                id: self.id,
                title: self.title,
                record_type: self.record_type,
                scope: self.scope,
                origin: self.origin,
                authority: self.authority,
            })?,
            payload: ExportPayloadInput {
                plan_ref: text("plan_ref", p.plan_ref)?,
                plan_fingerprint: sha256_hex("plan_fingerprint", p.plan_fingerprint)?,
                source: ExportSourceInput {
                    repository_ref: text("source.repository_ref", p.source.repository_ref)?,
                    revision: text("source.revision", p.source.revision)?,
                    digest: digest("source.digest", p.source.digest)?,
                },
                records: p
                    .records
                    .into_iter()
                    .enumerate()
                    .map(|(j, r)| r.into_input().map_err(|m| format!("records[{j}]: {m}")))
                    .collect::<Converted<_>>()?,
                retained: p
                    .retained
                    .into_iter()
                    .map(|r| {
                        Ok(RetainedInput {
                            unit_id: semantic_id("retained.unit_id", r.unit_id)?,
                            reason: text("retained.reason", r.reason)?,
                        })
                    })
                    .collect::<Converted<_>>()?,
                idempotency_key: sha256_hex("idempotency_key", p.idempotency_key)?,
                digest: sha256_hex("digest", p.digest)?,
            },
        })
    }
}
