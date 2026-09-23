//! A migration plan as the schema admits it
//! (`registries/operating-model/instance-data-migration.schema.json`) — the
//! typed, NOT yet domain-checked input of [`super::check_migration_plans`].
//!
//! Every closed `allOf` of the schema (a disposition's required/forbidden
//! fields, the two mutually exclusive rollback variants, a verified
//! sub-verdict's evidence, `reproducible` ⇒ a clean working tree) is a
//! variant here, so the app's DTO converts total over a schema-clean
//! document. Everything the schema cannot state — the nine contract
//! properties — is a check, never a constructor failure: a schema-valid
//! plan with a bad reference, a wrong authority or a stale fingerprint is a
//! reportable input, not a conversion error.

use crate::resolver::IsoDate;
use crate::run_contracts::RecordText;
use crate::types::{AuthorityKind, ContentDigest, Scope, SemanticId, Verdict};

use super::super::content::{Encoding, EnvelopeFields};
use super::super::record::RecordHead;

/// One entry of `migration_plans`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanInput {
    pub head: RecordHead,
    pub payload: PlanPayloadInput,
}

/// `definitions.payload`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanPayloadInput {
    pub source: SourceInput,
    pub record_units: Vec<RecordUnitInput>,
    pub mappings: Vec<MappingInput>,
    pub rollback: RollbackInput,
    pub verification: VerificationInput,
    pub plan_fingerprint: ContentDigest,
    pub idempotency_key: ContentDigest,
    pub supersedes: Option<RecordText>,
}

/// Whether the pinned source qualifies as a reproducible base. The schema's
/// `if reproducible then working_tree_clean: true and no reason, else a
/// reason` is this enum: only `NotReproducible` carries a reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Qualification {
    Reproducible,
    NotReproducible { reason: RecordText },
}

/// `definitions.source_state`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceInput {
    pub revision: RecordText,
    pub digest: ContentDigest,
    pub repository_ref: RecordText,
    pub working_tree_clean: bool,
    pub qualification: Qualification,
}

/// `definitions.record_unit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordUnitInput {
    pub id: SemanticId,
    pub unit_ref: RecordText,
    pub classification_basis: RecordText,
}

/// `preserved` or `assigned` (`definitions.field_basis`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldBasisValue {
    Preserved,
    Assigned,
}

impl FieldBasisValue {
    pub fn as_str(self) -> &'static str {
        match self {
            FieldBasisValue::Preserved => "preserved",
            FieldBasisValue::Assigned => "assigned",
        }
    }

    pub fn parse(value: &str) -> Option<FieldBasisValue> {
        match value {
            "preserved" => Some(FieldBasisValue::Preserved),
            "assigned" => Some(FieldBasisValue::Assigned),
            _ => None,
        }
    }
}

/// The per-field basis of all nine target fields. `origin` is declared
/// like every other field; that it is always `assigned` is a check
/// (property 4), not a constructor rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldBasisInput {
    pub schema: FieldBasisValue,
    pub id: FieldBasisValue,
    pub title: FieldBasisValue,
    pub record_type: FieldBasisValue,
    pub scope: FieldBasisValue,
    pub schema_version: FieldBasisValue,
    pub origin: FieldBasisValue,
    pub authority: FieldBasisValue,
    pub payload: FieldBasisValue,
}

/// `definitions.target_authority`: `decision_ref` is required.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetAuthorityInput {
    pub kind: AuthorityKind,
    pub authority_ref: RecordText,
    pub decision_ref: RecordText,
}

/// `definitions.content_envelope` as declared — the content-preservation
/// rule is a check over it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredContent {
    pub media_type: RecordText,
    pub encoding: Encoding,
    pub content: RecordText,
    pub digest: ContentDigest,
}

impl DeclaredContent {
    pub(crate) fn fields(&self) -> EnvelopeFields {
        EnvelopeFields::declared(
            self.media_type.as_str(),
            self.encoding,
            self.content.as_str(),
            &self.digest,
        )
    }
}

/// `definitions.target_record` (`origin.kind` is the schema constant
/// `migrated`, `schema_version` the constant 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetInput {
    pub schema_ref: RecordText,
    pub id: SemanticId,
    pub title: RecordText,
    pub record_type: SemanticId,
    pub scope: Scope,
    pub field_basis: FieldBasisInput,
    pub origin_source_ref: RecordText,
    pub authority: TargetAuthorityInput,
    pub payload: DeclaredContent,
}

/// `definitions.owner_decision`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnerDecisionInput {
    pub decision_ref: RecordText,
    pub decided_at: IsoDate,
    pub reason: RecordText,
}

/// A mapping's disposition with exactly the fields the schema requires for
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispositionInput {
    Migrated {
        target: TargetInput,
        owner_decision: OwnerDecisionInput,
    },
    Merged {
        target: TargetInput,
        owner_decision: OwnerDecisionInput,
        merge_rule_ref: RecordText,
    },
    RetainedTransitional {
        retained_reason: RecordText,
    },
}

impl DispositionInput {
    pub fn as_str(&self) -> &'static str {
        match self {
            DispositionInput::Migrated { .. } => "migrated",
            DispositionInput::Merged { .. } => "merged",
            DispositionInput::RetainedTransitional { .. } => "retained-transitional",
        }
    }
}

/// `definitions.mapping`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingInput {
    pub unit_id: SemanticId,
    pub disposition: DispositionInput,
}

impl MappingInput {
    /// The target a `migrated`/`merged` mapping mints.
    pub fn target(&self) -> Option<&TargetInput> {
        match &self.disposition {
            DispositionInput::Migrated { target, .. } | DispositionInput::Merged { target, .. } => {
                Some(target)
            }
            DispositionInput::RetainedTransitional { .. } => None,
        }
    }

    pub fn owner_decision(&self) -> Option<&OwnerDecisionInput> {
        match &self.disposition {
            DispositionInput::Migrated { owner_decision, .. }
            | DispositionInput::Merged { owner_decision, .. } => Some(owner_decision),
            DispositionInput::RetainedTransitional { .. } => None,
        }
    }

    pub fn merge_rule_ref(&self) -> Option<&RecordText> {
        match &self.disposition {
            DispositionInput::Merged { merge_rule_ref, .. } => Some(merge_rule_ref),
            _ => None,
        }
    }
}

/// The two mutually exclusive rollback variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RollbackPlanInput {
    DeterministicReconstruction {
        deterministic_plan_ref: RecordText,
    },
    VerifiedRestoration {
        restoration_evidence_ref: RecordText,
    },
}

/// `definitions.rollback` (`rewrites_published_history` is the schema
/// constant `false`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackInput {
    pub source_snapshot_ref: RecordText,
    pub plan: RollbackPlanInput,
}

/// One sub-verdict: only a `verified` one carries (and must carry) its
/// evidence reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubVerdictInput {
    Verified { evidence_ref: RecordText },
    Unverified,
}

/// `definitions.verification`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationInput {
    pub coverage: SubVerdictInput,
    pub applicability_preservation: SubVerdictInput,
    pub overall_status: Verdict,
}
