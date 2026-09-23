//! Canonical exports (`instance-canonical-export.schema.json`,
//! `scripts/lib/instance-data-migration.mjs`'s
//! `evaluateInstanceCanonicalExport`): the checkable proof that one
//! migration plan's minted and retained units are fully and faithfully
//! accounted for.
//!
//! An export is checked against exactly one plan basis ([`PlanBoundary`]):
//! either the contract's own resolver catalogue (the standalone export
//! contract), or a [`PinnedPlan`] — the plan a qualification already
//! resolved and pinned by its recomputed fingerprint. The second boundary
//! resolves ONLY that plan's id to ONLY that plan's content, so no
//! independently configured plan with the same id can reach the check.

mod check;

use std::borrow::Cow;

use crate::run_contracts::{RecordText, ResponseField};
use crate::types::{ContentDigest, Diagnostic, SemanticId};

use super::content::EnvelopeFields;
use super::plan::{DispositionInput, PinnedPlan};
use super::projection;
use super::record::RecordHead;
use super::resolved::{
    DigestResponse, MappingResponse, MigrationPlanResponse, OpenObject, RecordUnitResponse,
    ResponseCatalogue, ScopeResponse, SourceContentResponse, SourceIdentityResponse,
    TargetResponse,
};
use crate::canonical::CanonicalJson;

/// `definitions.pinned_source`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSourceInput {
    pub repository_ref: RecordText,
    pub revision: RecordText,
    pub digest: ContentDigest,
}

/// `definitions.retained_unit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedInput {
    pub unit_id: SemanticId,
    pub reason: RecordText,
}

/// An exported record's `payload`. The envelope schema admits any object
/// here, so the payload is kept as the open object it is, together with the
/// transport's typed reading of its content-envelope fields.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportedPayload {
    object: OpenObject,
    envelope: EnvelopeFields,
}

impl ExportedPayload {
    /// `object` and `envelope` are two readings of the same JSON object:
    /// its members, and its `media_type`/`encoding`/`content`/`digest`.
    pub fn new(object: OpenObject, envelope: EnvelopeFields) -> Self {
        Self { object, envelope }
    }

    pub fn envelope(&self) -> &EnvelopeFields {
        &self.envelope
    }

    pub(crate) fn canonical(&self) -> CanonicalJson {
        self.object.whole()
    }
}

/// One element of `payload.records`: a full scoped record (its envelope
/// already checked against `scoped-record.schema.json`).
#[derive(Debug, Clone, PartialEq)]
pub struct ExportedRecordInput {
    pub head: RecordHead,
    pub payload: ExportedPayload,
}

/// `definitions.payload`.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportPayloadInput {
    pub plan_ref: RecordText,
    pub plan_fingerprint: ContentDigest,
    pub source: ExportSourceInput,
    pub records: Vec<ExportedRecordInput>,
    pub retained: Vec<RetainedInput>,
    pub idempotency_key: ContentDigest,
    pub digest: ContentDigest,
}

/// One entry of `exports`.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportInput {
    pub head: RecordHead,
    pub payload: ExportPayloadInput,
}

/// `computeExportDigest(payload)`.
pub fn compute_export_digest(payload: &ExportPayloadInput) -> ContentDigest {
    projection::export_digest(payload)
}

/// `computeExportIdempotencyKey(plan_ref, plan_fingerprint)`.
pub fn compute_export_idempotency_key(plan_ref: &str, plan_fingerprint: &str) -> ContentDigest {
    projection::export_idempotency_key(plan_ref, plan_fingerprint)
}

/// `computeSourceContentKey`: the pinned source identity, the sorted
/// (code-unit order), de-duplicated contributing `unit_ref`s and, for a
/// merge, its own `merge_rule_ref`.
pub fn compute_source_content_key(
    repository_ref: &str,
    revision: &str,
    unit_refs: &[String],
    merge_rule_ref: Option<&str>,
) -> String {
    let mut sorted: Vec<&str> = Vec::new();
    for r in unit_refs {
        if !sorted.contains(&r.as_str()) {
            sorted.push(r);
        }
    }
    sorted.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    let base = format!("{repository_ref}@{revision}::{}", sorted.join(","));
    match merge_rule_ref.filter(|r| !r.is_empty()) {
        Some(rule) => format!("{base}::{rule}"),
        None => base,
    }
}

/// The one plan an export is checked against.
#[derive(Debug, Clone, Copy)]
pub enum PlanBoundary<'a> {
    /// The standalone contract's `resolveMigrationPlan`.
    Resolver(&'a ResponseCatalogue<MigrationPlanResponse>),
    /// A plan already resolved and pinned by the composing record: only
    /// its own id resolves, and only to its own content.
    Pinned(&'a PinnedPlan),
}

impl<'a> PlanBoundary<'a> {
    fn resolve(self, plan_ref: &str) -> Option<Cow<'a, MigrationPlanResponse>> {
        match self {
            PlanBoundary::Resolver(catalogue) => catalogue.resolve(plan_ref).map(Cow::Borrowed),
            PlanBoundary::Pinned(plan) => (plan.input().head.id.as_str() == plan_ref)
                .then(|| Cow::Owned(pinned_plan_response(plan))),
        }
    }
}

fn present(value: &str) -> ResponseField<String> {
    ResponseField::Present(value.to_string())
}

fn digest_response(digest: &ContentDigest) -> ResponseField<DigestResponse> {
    ResponseField::Present(DigestResponse {
        algorithm: present(digest.algorithm().as_str()),
        value: present(digest.value()),
        unknown_keys: Vec::new(),
    })
}

/// The derived plan response of a pinned plan — the reference's
/// `derivedPlanResolver`, with the RECOMPUTED fingerprint.
fn pinned_plan_response(plan: &PinnedPlan) -> MigrationPlanResponse {
    let input = plan.input();
    let scope = &input.head.scope;
    let source = &input.payload.source;
    MigrationPlanResponse {
        record_type: present(super::plan::RECORD_TYPE),
        plan_ref: present(input.head.id.as_str()),
        plan_fingerprint: present(plan.fingerprint().value()),
        scope: ResponseField::Present(ScopeResponse {
            scope_type: present(scope.scope_type().as_str()),
            id: present(scope.id()),
            workspace_id: scope
                .workspace_id()
                .map_or(ResponseField::Absent, |w| present(w.as_str())),
            organization_profile_id: scope
                .organization_profile_id()
                .map_or(ResponseField::Absent, |o| present(o.as_str())),
            unknown_keys: Vec::new(),
        }),
        source: ResponseField::Present(SourceIdentityResponse {
            repository_ref: present(source.repository_ref.as_str()),
            revision: present(source.revision.as_str()),
            digest: digest_response(&source.digest),
        }),
        record_units: input
            .payload
            .record_units
            .iter()
            .map(|u| RecordUnitResponse {
                id: present(u.id.as_str()),
                unit_ref: present(u.unit_ref.as_str()),
            })
            .collect(),
        mappings: input
            .payload
            .mappings
            .iter()
            .map(|m| MappingResponse {
                unit_id: present(m.unit_id.as_str()),
                disposition: present(m.disposition.as_str()),
                target: m.target().map(|t| TargetResponse {
                    id: present(t.id.as_str()),
                    record: OpenObject::new(
                        projection::TARGET_FIELDS
                            .iter()
                            .filter_map(|name| {
                                projection::target_member(t, name)
                                    .map(|v| ((*name).to_string(), CanonicalJson::from_json(v)))
                            })
                            .collect(),
                    ),
                }),
                merge_rule_ref: m
                    .merge_rule_ref()
                    .map_or(ResponseField::Absent, |r| present(r.as_str())),
                retained_reason: match &m.disposition {
                    DispositionInput::RetainedTransitional { retained_reason } => {
                        present(retained_reason.as_str())
                    }
                    _ => ResponseField::Absent,
                },
            })
            .collect(),
        unknown_keys: Vec::new(),
    }
}

/// A canonical export with zero problems of its own; its digest is the
/// recomputed one.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonicalExport {
    input: ExportInput,
    digest: ContentDigest,
}

impl CanonicalExport {
    pub fn input(&self) -> &ExportInput {
        &self.input
    }
    pub fn digest(&self) -> &ContentDigest {
        &self.digest
    }
}

/// A schema-shaped export whose recomputed digest equals a composing
/// record's pin.
#[derive(Debug, Clone, PartialEq)]
pub struct PinnedExport {
    input: ExportInput,
    digest: ContentDigest,
}

impl PinnedExport {
    /// `Ok` when `pinned_sha256` is this export's recomputed digest;
    /// otherwise the recomputed digest the pin failed to name.
    pub fn confirm(input: ExportInput, pinned_sha256: &str) -> Result<PinnedExport, ContentDigest> {
        let digest = compute_export_digest(&input.payload);
        if digest.value() == pinned_sha256 {
            Ok(PinnedExport { input, digest })
        } else {
            Err(digest)
        }
    }

    pub fn input(&self) -> &ExportInput {
        &self.input
    }
    pub fn digest(&self) -> &ContentDigest {
        &self.digest
    }
}

/// The result of one batch of exports.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportBatchOutcome {
    pub diagnostics: Vec<Diagnostic>,
    pub accepted: Vec<Option<CanonicalExport>>,
}

/// `evaluateInstanceCanonicalExport` over one schema-clean container's
/// `exports`, checked against `plan` and the resolved source content.
pub fn check_canonical_exports(
    entries: &[ExportInput],
    plan: PlanBoundary<'_>,
    content: &ResponseCatalogue<SourceContentResponse>,
) -> ExportBatchOutcome {
    let mut owned: Vec<(usize, Diagnostic)> = Vec::new();
    let mut seen_ids: Vec<&str> = Vec::new();
    let mut digests = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let mut problems = Vec::new();
        let id = entry.head.id.as_str();
        if seen_ids.contains(&id) {
            problems.push(super::fail(format!(
                "canonical export \"{id}\" is declared more than once"
            )));
        } else {
            seen_ids.push(id);
        }
        let digest = compute_export_digest(&entry.payload);
        let resolved = plan.resolve(entry.payload.plan_ref.as_str());
        check::check_export(entry, resolved.as_deref(), content, &digest, &mut problems);
        owned.extend(problems.into_iter().map(|p| (index, p)));
        digests.push(digest);
    }

    let mut first_by_key: Vec<(&str, &str)> = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        let key = entry.payload.idempotency_key.value();
        let id = entry.head.id.as_str();
        match first_by_key.iter().find(|(k, _)| *k == key) {
            Some((_, first)) => owned.push((
                index,
                super::fail(format!(
                    "export idempotency_key \"{key}\" is declared by more than one canonical export (\"{first}\" and \"{id}\"); a rerun over the same pinned plan state must recognise the existing export, not mint a duplicate"
                )),
            )),
            None => first_by_key.push((key, id)),
        }
    }

    let accepted = entries
        .iter()
        .zip(digests)
        .enumerate()
        .map(|(index, (entry, digest))| {
            (!owned.iter().any(|(i, _)| *i == index)).then(|| CanonicalExport {
                input: entry.clone(),
                digest,
            })
        })
        .collect();
    ExportBatchOutcome {
        diagnostics: owned.into_iter().map(|(_, d)| d).collect(),
        accepted,
    }
}

#[cfg(test)]
mod tests;
