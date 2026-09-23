//! The scoped-record envelope every run-contract record carries, and the
//! ONE envelope check every record family of the run contracts, the
//! evidence-and-handoff contract and the field evaluation shares: logical
//! `$schema` resolution, the Russian title, the `built-in` origin rejection
//! and the portability of the envelope's reference strings. Before
//! `rust-architecture-conformance-5` the same check existed as drifting
//! copies; [`RecordFamily`] carries the only family-specific wording, so the
//! algorithm has one owner.

use crate::task_contracts::{non_portable_reason, resolve_schema_ref};
use crate::types::{AuthorityKind, Diagnostic, OriginKind, SemanticId};

use super::identity::{PortableRef, RecordText};
use super::{fail, has_cyrillic};

const SCHEMA_NAMESPACE_DIR: &str = "registries/operating-model";
const ENVELOPE_SCHEMA_BASENAME: &str = "scoped-record.schema.json";

/// The record families that share the envelope check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RecordFamily {
    ExecutionRun,
    RoleRegistry,
    HumanControl,
    ContextManifest,
    EvidenceAndHandoff,
    FieldEvaluationObservation,
    FieldEvaluationReport,
}

impl RecordFamily {
    /// How a diagnostic names a record of this family.
    pub fn label(self) -> &'static str {
        match self {
            RecordFamily::ExecutionRun => "execution run",
            RecordFamily::RoleRegistry => "role registry",
            RecordFamily::HumanControl => "human-control record",
            RecordFamily::ContextManifest => "context manifest",
            RecordFamily::EvidenceAndHandoff => "evidence and handoff",
            RecordFamily::FieldEvaluationObservation => "field evaluation observation",
            RecordFamily::FieldEvaluationReport => "field evaluation report",
        }
    }

    /// The specialised schema a record of this family must declare.
    pub fn schema_basename(self) -> &'static str {
        match self {
            RecordFamily::ExecutionRun => "execution-state.schema.json",
            RecordFamily::RoleRegistry => "role-registry.schema.json",
            RecordFamily::HumanControl => "human-control.schema.json",
            RecordFamily::ContextManifest => "context-manifest.schema.json",
            RecordFamily::EvidenceAndHandoff => "evidence-and-handoff.schema.json",
            RecordFamily::FieldEvaluationObservation | RecordFamily::FieldEvaluationReport => {
                "field-evaluation.schema.json"
            }
        }
    }

    fn canonical_base(self) -> &'static str {
        match self {
            RecordFamily::ExecutionRun => "records/execution-run",
            RecordFamily::RoleRegistry => "records/role-registry",
            RecordFamily::HumanControl => "records/run-human-control",
            RecordFamily::ContextManifest => "records/context-manifest",
            RecordFamily::EvidenceAndHandoff => "records/evidence-and-handoff",
            RecordFamily::FieldEvaluationObservation | RecordFamily::FieldEvaluationReport => {
                "records/field-evaluation"
            }
        }
    }

    fn title_noun(self) -> &'static str {
        match self {
            RecordFamily::ExecutionRun => "the run name",
            RecordFamily::RoleRegistry => "the catalogue name",
            RecordFamily::HumanControl => "the record name",
            RecordFamily::ContextManifest => "the manifest name",
            RecordFamily::EvidenceAndHandoff => "the handoff name",
            RecordFamily::FieldEvaluationObservation | RecordFamily::FieldEvaluationReport => {
                "the record name"
            }
        }
    }

    /// Why `origin.kind: built-in` is rejected, or `None` for the one family
    /// (the built-in role catalogue) whose schema REQUIRES it.
    fn built_in_origin_rejection(self) -> Option<&'static str> {
        match self {
            RecordFamily::ExecutionRun => {
                Some("a run is started in a workspace, not shipped with the methodology")
            }
            RecordFamily::RoleRegistry => None,
            RecordFamily::HumanControl => Some(
                "a run's control state is established in a workspace, not shipped with the methodology",
            ),
            RecordFamily::ContextManifest => {
                Some("a manifest is written in a workspace, not shipped with the methodology")
            }
            RecordFamily::EvidenceAndHandoff => {
                Some("a handoff is written in a workspace, not shipped with the methodology")
            }
            RecordFamily::FieldEvaluationObservation | RecordFamily::FieldEvaluationReport => {
                Some("evaluation data is written in a workspace, not shipped with the methodology")
            }
        }
    }

    fn portable_record_phrase(self) -> &'static str {
        match self {
            RecordFamily::ExecutionRun => "a run record is portable",
            RecordFamily::RoleRegistry | RecordFamily::HumanControl => "the record is portable",
            RecordFamily::ContextManifest => "a manifest is portable",
            RecordFamily::EvidenceAndHandoff => "a handoff is portable",
            RecordFamily::FieldEvaluationObservation | RecordFamily::FieldEvaluationReport => {
                "a record is portable"
            }
        }
    }
}

/// Where a record came from. A `built-in` origin carries no source
/// reference; every other origin kind carries exactly one — the schema's
/// `if/then/else`, made unrepresentable to violate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordOrigin {
    BuiltIn,
    Sourced {
        kind: SourcedOriginKind,
        source_ref: PortableRef,
    },
}

/// An [`OriginKind`] other than [`OriginKind::BuiltIn`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcedOriginKind(OriginKind);

impl SourcedOriginKind {
    /// `None` for [`OriginKind::BuiltIn`], which never carries a source.
    pub fn new(kind: OriginKind) -> Option<Self> {
        (kind != OriginKind::BuiltIn).then_some(Self(kind))
    }

    pub fn kind(self) -> OriginKind {
        self.0
    }
}

/// The envelope's authority block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordAuthority {
    pub kind: AuthorityKind,
    pub authority_ref: PortableRef,
    pub decision_ref: Option<PortableRef>,
}

/// The envelope fields every run-contract record carries. `schema_version`,
/// `record_type` and the scope are fixed by each family's schema and typed
/// in the family's own input (a run-state scope, or the built-in
/// methodology scope of the role catalogue).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordEnvelope {
    pub declared_schema: RecordText,
    pub id: SemanticId,
    pub title: RecordText,
    pub origin: RecordOrigin,
    pub authority: RecordAuthority,
}

/// The shared envelope check, in the fixed order every family reports it:
/// `$schema`, title, `built-in` origin, envelope reference portability.
pub(crate) fn check_envelope(
    family: RecordFamily,
    envelope: &RecordEnvelope,
    problems: &mut Vec<Diagnostic>,
) {
    check_envelope_around(family, envelope, problems, |_| {});
}

/// [`check_envelope`] with a family-specific check between the title and
/// the origin — where the field evaluation reports its record-type-bound
/// scope (`scripts/lib/field-evaluation.mjs`'s order).
pub(crate) fn check_envelope_around(
    family: RecordFamily,
    envelope: &RecordEnvelope,
    problems: &mut Vec<Diagnostic>,
    between: impl FnOnce(&mut Vec<Diagnostic>),
) {
    let label = family.label();
    let id = envelope.id.as_str();
    let basename = family.schema_basename();

    let declared = envelope.declared_schema.as_str();
    if let Some(portability) = non_portable_reason(Some(declared)) {
        problems.push(fail(format!(
            "{label} \"{id}\" $schema \"{declared}\" is not portable ({portability}); the schema is named by a relative reference resolved inside the Meridian namespace (canonical logical base {})",
            family.canonical_base()
        )));
    } else {
        let resolved = resolve_schema_ref(Some(declared));
        let envelope_ref = format!("{SCHEMA_NAMESPACE_DIR}/{ENVELOPE_SCHEMA_BASENAME}");
        let expected_ref = format!("{SCHEMA_NAMESPACE_DIR}/{basename}");
        if resolved.as_deref() == Some(envelope_ref.as_str()) {
            problems.push(fail(format!(
                "{label} \"{id}\" $schema \"{declared}\" resolves to the record envelope ({ENVELOPE_SCHEMA_BASENAME}); it must name {basename}, which composes the envelope with the body"
            )));
        } else if resolved.as_deref() != Some(expected_ref.as_str()) {
            problems.push(fail(format!(
                "{label} \"{id}\" $schema \"{declared}\" does not resolve to the logical address {expected_ref} within the Meridian namespace; the specialised schema is named by a portable relative reference (a bare basename, a missing namespace segment, a wrong segment and a reference climbing out of the namespace all resolve elsewhere)"
            )));
        }
    }

    let title = envelope.title.as_str();
    if envelope.title.is_blank() {
        problems.push(fail(format!(
            "{label} \"{id}\" has no human-readable title"
        )));
    } else if !has_cyrillic(title) {
        problems.push(fail(format!(
            "{label} \"{id}\" title \"{title}\" carries no Russian (Cyrillic) text; {} is stated in Russian for the human reader",
            family.title_noun()
        )));
    }
    if let Some(r) = non_portable_reason(Some(title)) {
        problems.push(fail(format!("{label} \"{id}\" title contains {r}")));
    }

    between(problems);

    if let (RecordOrigin::BuiltIn, Some(why)) =
        (&envelope.origin, family.built_in_origin_rejection())
    {
        problems.push(fail(format!(
            "{label} \"{id}\" declares origin.kind \"built-in\"; {why}"
        )));
    }

    let source_ref = match &envelope.origin {
        RecordOrigin::BuiltIn => None,
        RecordOrigin::Sourced { source_ref, .. } => Some(source_ref),
    };
    for (value, name) in [
        (source_ref, "origin.source_ref"),
        (
            Some(&envelope.authority.authority_ref),
            "authority.authority_ref",
        ),
        (
            envelope.authority.decision_ref.as_ref(),
            "authority.decision_ref",
        ),
    ] {
        if let Some(r) = value.and_then(|v| non_portable_reason(Some(v.as_str()))) {
            problems.push(fail(format!(
                "{label} \"{id}\" {name} contains {r}; {} and carries no rooted machine path",
                family.portable_record_phrase()
            )));
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn envelope(family: RecordFamily) -> RecordEnvelope {
        RecordEnvelope {
            declared_schema: RecordText::new(format!(
                "../../registries/operating-model/{}",
                family.schema_basename()
            ))
            .unwrap(),
            id: SemanticId::new("example-record").unwrap(),
            title: RecordText::new("Пример записи").unwrap(),
            origin: RecordOrigin::Sourced {
                kind: SourcedOriginKind::new(OriginKind::Declared).unwrap(),
                source_ref: PortableRef::new("owner-decision:example").unwrap(),
            },
            authority: RecordAuthority {
                kind: AuthorityKind::DelegatedRun,
                authority_ref: PortableRef::new("example-owner").unwrap(),
                decision_ref: None,
            },
        }
    }

    fn messages(family: RecordFamily, envelope: &RecordEnvelope) -> Vec<String> {
        let mut problems = Vec::new();
        check_envelope(family, envelope, &mut problems);
        problems.iter().map(|d| d.message().to_string()).collect()
    }

    #[test]
    fn a_clean_envelope_has_no_problems_for_every_family() {
        for family in [
            RecordFamily::ExecutionRun,
            RecordFamily::HumanControl,
            RecordFamily::ContextManifest,
        ] {
            assert!(messages(family, &envelope(family)).is_empty());
        }
    }

    #[test]
    fn schema_declaration_title_origin_and_refs_are_reported_in_order() {
        let family = RecordFamily::ExecutionRun;
        let mut e = envelope(family);
        e.declared_schema =
            RecordText::new("../../registries/operating-model/scoped-record.schema.json").unwrap();
        e.title = RecordText::new("English only").unwrap();
        e.origin = RecordOrigin::BuiltIn;
        e.authority.decision_ref = Some(PortableRef::new("/etc/passwd").unwrap());
        let m = messages(family, &e);
        assert_eq!(m.len(), 4, "{m:?}");
        assert!(m[0].contains("resolves to the record envelope (scoped-record.schema.json)"));
        assert!(m[1].contains("carries no Russian (Cyrillic) text; the run name is stated"));
        assert!(m[2].contains("declares origin.kind \"built-in\"; a run is started"));
        assert!(m[3].starts_with(
            "execution run \"example-record\" authority.decision_ref contains a rooted"
        ));
        assert!(m[3].ends_with("a run record is portable and carries no rooted machine path"));
    }

    #[test]
    fn the_role_catalogue_accepts_a_built_in_origin() {
        let family = RecordFamily::RoleRegistry;
        let mut e = envelope(family);
        e.origin = RecordOrigin::BuiltIn;
        assert!(messages(family, &e).is_empty());
    }

    #[test]
    fn a_wrong_and_a_non_portable_schema_reference_are_distinct() {
        let family = RecordFamily::ContextManifest;
        let mut e = envelope(family);
        e.declared_schema = RecordText::new("context-manifest.schema.json").unwrap();
        assert!(messages(family, &e)[0].contains("does not resolve to the logical address registries/operating-model/context-manifest.schema.json"));
        e.declared_schema = RecordText::new("/abs/context-manifest.schema.json").unwrap();
        assert!(
            messages(family, &e)[0].contains("is not portable (a rooted (absolute) POSIX path)")
        );
        assert!(
            messages(family, &e)[0].contains("(canonical logical base records/context-manifest)")
        );
    }

    #[test]
    fn blank_title_is_not_reported_as_missing_cyrillic() {
        let family = RecordFamily::HumanControl;
        let mut e = envelope(family);
        e.title = RecordText::new("  ").unwrap();
        assert_eq!(
            messages(family, &e),
            ["human-control record \"example-record\" has no human-readable title"]
        );
    }
}
