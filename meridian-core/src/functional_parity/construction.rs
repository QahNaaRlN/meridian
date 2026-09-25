//! Typed input assembled by the caller (`meridian-app`'s DTO/schema
//! boundary) for one `functional-parity` evidence document, and the
//! accepted [`FunctionalParityEvidence`] output
//! [`crate::functional_parity::checks::check_document`] alone produces.
//! Nothing here performs I/O or sees `serde_json::Value` — every field is
//! already a closed [`super::types`] value; the caller has already run
//! JSON Schema validation and closed-DTO transport parsing before building
//! any of these.
//!
//! Mutually exclusive schema states are ENUM forms carrying their own data,
//! not a flat marker plus a separately-optional field a caller could
//! construct inconsistently (corrective round item 1): [`FacetContent`]
//! (declared assertions XOR a not-applicable justification), [`GapInput`]
//! (record-scoped XOR assertion-scoped, each with its own required fields),
//! [`PostChangeConditions`] (`same` XOR `explicitly-comparable`), and
//! [`VerdictOutcome`] (`VERIFIED` XOR `UNVERIFIED` with its reason). All are
//! total, exhaustive Rust representations of a mutually exclusive JSON
//! Schema `oneOf`/`allOf`-`if`/`then` shape: there is no way to construct,
//! for example, a record-scoped gap that also carries `assertion_ids`.

use super::types::{AssertionId, ConditionId, EvidenceKind, EvidenceText, Facet, SourceStateRef};

/// One declared preserved-contract assertion: its id and the human-readable
/// statement of what stays observably identical
/// (`definitions.assertion.statement`).
#[derive(Debug, Clone)]
pub struct Assertion {
    pub id: AssertionId,
    pub statement: EvidenceText,
}

/// One `preserved_contract` facet's content: either it declares at least
/// one [`Assertion`], or it carries a justification for why the facet does
/// not apply — the schema's own `facet.oneOf` made an exhaustive Rust enum
/// (corrective round item 1), never an empty `assertions` list standing in
/// for "not applicable".
#[derive(Debug, Clone)]
pub enum FacetContent {
    Declared(Vec<Assertion>),
    NotApplicable(EvidenceText),
}

/// One `preserved_contract` facet: its identity and its
/// [`FacetContent`].
#[derive(Debug, Clone)]
pub struct FacetInput {
    pub facet: Facet,
    pub content: FacetContent,
}

/// A `VERIFIED`/`UNVERIFIED` outcome carrying its own reason exactly where
/// the schema requires one: `Unverified` always carries a reason (schema:
/// `unverified_reason`/`overall_unverified_reason` required when the state
/// is `UNVERIFIED`), `Verified` never does (schema: the same field
/// forbidden when the state is `VERIFIED`) — used for both a per-assertion
/// verdict and the record's overall verdict (corrective round item 1).
#[derive(Debug, Clone)]
pub enum VerdictOutcome {
    Verified,
    Unverified { reason: EvidenceText },
}

impl VerdictOutcome {
    pub fn is_verified(&self) -> bool {
        matches!(self, VerdictOutcome::Verified)
    }

    pub fn is_unverified(&self) -> bool {
        matches!(self, VerdictOutcome::Unverified { .. })
    }
}

/// One `verdict.per_assertion` entry.
#[derive(Debug, Clone)]
pub struct AssertionVerdictInput {
    pub assertion_id: AssertionId,
    pub state: VerdictOutcome,
}

/// One record's whole `verdict` block.
#[derive(Debug, Clone)]
pub struct VerdictInput {
    pub per_assertion: Vec<AssertionVerdictInput>,
    pub overall: VerdictOutcome,
}

/// One `evidence` entry: `kind` (closed-type discipline,
/// `meridian-rust-migration-program-plan.md` §5.18.3 point 2 — no evidence
/// kind is mandatory or privileged, so no rule branches on it, but a
/// schema-valid document's kind is always parsed into this closed type, not
/// dropped), the scope of what it actually observed, the declared
/// assertions it covers, its required, non-empty limitations, and — only
/// for `public-contract-snapshot` — its applicability justification.
#[derive(Debug, Clone)]
pub struct EvidenceEntryInput {
    pub kind: EvidenceKind,
    pub observed_scope: EvidenceText,
    pub covers: Vec<AssertionId>,
    pub limitations: Vec<EvidenceText>,
    pub applicability_justification: Option<EvidenceText>,
}

/// One `gaps` entry: record-scoped (reaches every declared assertion) or
/// assertion-scoped (reaches only the assertions it names) — the schema's
/// own `gap.allOf`/`if`/`then` made an exhaustive Rust enum, so a
/// record-scoped gap can never also carry `assertion_ids` and an
/// assertion-scoped gap can never be missing them (corrective round item 1).
#[derive(Debug, Clone)]
pub enum GapInput {
    Record {
        description: EvidenceText,
    },
    Assertion {
        description: EvidenceText,
        assertion_ids: Vec<AssertionId>,
    },
}

/// One baseline `inputs_and_conditions` entry: its stable id, description,
/// and optional kind.
#[derive(Debug, Clone)]
pub struct IdentifiedCondition {
    pub id: ConditionId,
    pub description: EvidenceText,
    pub kind: Option<EvidenceText>,
}

/// How the baseline observation was obtained: `method` and whether that was
/// actually `established` — `established: false` is a gap, not a pass
/// (record rule 9).
#[derive(Debug, Clone)]
pub struct Provenance {
    pub method: EvidenceText,
    pub established: bool,
}

/// The result actually observed (`definitions.observed_result`): a summary
/// and optional supporting artifact references.
#[derive(Debug, Clone)]
pub struct ObservedResult {
    pub summary: EvidenceText,
    pub artifacts: Vec<EvidenceText>,
}

/// The baseline observation's already-extracted fields.
#[derive(Debug, Clone)]
pub struct BaselineInput {
    pub source_state: SourceStateRef,
    pub conditions: Vec<IdentifiedCondition>,
    pub provenance: Provenance,
    pub observed_result: ObservedResult,
}

/// One post-change, explicitly-restated input or observation condition
/// (`definitions.condition`) — used only when the relationship is
/// `explicitly-comparable`.
#[derive(Debug, Clone)]
pub struct Condition {
    pub description: EvidenceText,
    pub kind: Option<EvidenceText>,
}

/// How the post-change observation's inputs and conditions relate to the
/// baseline's — the schema's own
/// `post_change_evidence.inputs_and_conditions.allOf`/`if`/`then` made an
/// exhaustive Rust enum (corrective round item 1): `Same` carries the
/// baseline condition ids it claims to reproduce and nothing else; `Same`
/// can never also carry restated `items`, and `ExplicitlyComparable` can
/// never carry `baseline_condition_ids`.
#[derive(Debug, Clone)]
pub enum PostChangeConditions {
    Same {
        baseline_condition_ids: Vec<ConditionId>,
    },
    ExplicitlyComparable {
        items: Vec<Condition>,
        comparability_justification: EvidenceText,
    },
}

/// One `post_change_evidence.contract_links` entry.
#[derive(Debug, Clone)]
pub struct ContractLinkInput {
    pub facet: Facet,
    pub assertion_id: AssertionId,
}

/// The post-change observation's already-extracted fields.
#[derive(Debug, Clone)]
pub struct PostChangeInput {
    pub source_state: SourceStateRef,
    pub conditions: PostChangeConditions,
    pub observed_result: ObservedResult,
    pub contract_links: Vec<ContractLinkInput>,
}

/// One `records` entry's whole already-extracted, already-typed shape — the
/// ONLY input [`crate::functional_parity::checks::check_document`] accepts.
#[derive(Debug, Clone)]
pub struct RecordInput {
    pub work_item: EvidenceText,
    pub facets: [FacetInput; 4],
    pub baseline: BaselineInput,
    pub post_change: PostChangeInput,
    pub evidence: Vec<EvidenceEntryInput>,
    pub gaps: Vec<GapInput>,
    pub verdict: VerdictInput,
    pub recorded_at: EvidenceText,
}

/// One record's accepted evidence: the FULL checked [`RecordInput`] — every
/// business field the schema carries, not a summary projection — held in a
/// private validated wrapper. It exists only when that record's own eleven
/// inference rules produced zero diagnostics
/// (`meridian-rust-migration-program-plan.md` §5.18.3 point 3): the ONLY
/// constructor is `pub(super)` and is called solely by
/// [`super::checks::check_document`], so a held value can never carry a
/// duplicated id, a dangling reference, a false `VERIFIED`, or an
/// indistinguishable before/after pair. Every accessor is read-only; the
/// wrapped record cannot be mutated after acceptance.
#[derive(Debug, Clone)]
pub struct RecordEvidence {
    record: RecordInput,
    verified_assertions: Vec<AssertionId>,
}

impl RecordEvidence {
    pub(super) fn new(record: RecordInput, verified_assertions: Vec<AssertionId>) -> Self {
        Self {
            record,
            verified_assertions,
        }
    }

    /// The whole checked record, read-only.
    pub fn record(&self) -> &RecordInput {
        &self.record
    }

    pub fn work_item(&self) -> &EvidenceText {
        &self.record.work_item
    }

    pub fn recorded_at(&self) -> &EvidenceText {
        &self.record.recorded_at
    }

    /// The four facets, in [`Facet::ALL`] order.
    pub fn facets(&self) -> &[FacetInput; 4] {
        &self.record.facets
    }

    pub fn baseline(&self) -> &BaselineInput {
        &self.record.baseline
    }

    pub fn post_change(&self) -> &PostChangeInput {
        &self.record.post_change
    }

    pub fn evidence(&self) -> &[EvidenceEntryInput] {
        &self.record.evidence
    }

    pub fn gaps(&self) -> &[GapInput] {
        &self.record.gaps
    }

    pub fn verdict(&self) -> &VerdictInput {
        &self.record.verdict
    }

    pub fn overall(&self) -> &VerdictOutcome {
        &self.record.verdict.overall
    }

    /// The declared assertions whose per-assertion verdict is `VERIFIED`,
    /// sorted by id — derived once at acceptance time.
    pub fn verified_assertions(&self) -> &[AssertionId] {
        &self.verified_assertions
    }
}

/// A whole `functional-parity` evidence document that passed every one of
/// its records' eleven inference rules with zero diagnostics. The ONE public
/// path to this type is [`super::checks::check_document`]; there is no
/// other public constructor, so a caller can never hold one that was not
/// actually checked clean.
#[derive(Debug, Clone)]
pub struct FunctionalParityEvidence {
    records: Vec<RecordEvidence>,
}

impl FunctionalParityEvidence {
    pub(super) fn new(records: Vec<RecordEvidence>) -> Self {
        Self { records }
    }

    pub fn records(&self) -> &[RecordEvidence] {
        &self.records
    }
}
