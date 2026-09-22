//! Pure half of the `agent-instruction-identity` check (package 7,
//! subpackage 7a): `standards/workspace/agent-instruction-identity.md` §7
//! applied to the Kernel's own documents. Front-Matter field presence is
//! extracted by the transport layer
//! (`meridian_app::validation::mechanical_integrity::agent_instruction_identity`,
//! which owns the regexes over raw Markdown text — `meridian-core` takes no
//! `fancy-regex` dependency); this module owns the closed `delivery`/
//! `activation` vocabularies (as real enums, not a string membership test),
//! the closed [`NormField`]/[`Declaration`] model, the [`build_identity`]
//! construction report, and the rest of §7's predicate over one document's
//! already-extracted facts.
//!
//! # Construction report, not an "invalid" domain variant
//!
//! [`CompleteIdentity`] is the ONLY valid domain value this module produces
//! for a document's declared identity: private fields, and it can exist
//! only when all four §7 fields are present AND `delivery`/`activation` are
//! both coherent (the presence flag [`build_identity`] was given agrees
//! with whether a captured value was ALSO supplied) closed, known values.
//! A half declaration, an unknown `delivery`/`activation` string, or an
//! incoherent presence/value pair are never stored inside `CompleteIdentity`
//! — they are [`IdentityConstructionIssue`]s, collected independently (so a
//! document with two unrelated defects still reports both), and
//! `CompleteIdentity` is `None` whenever the issue list is non-empty.
//! [`build_identity`] is the one pure construction function that turns raw,
//! already-extracted scalars into this report; core business checks that
//! need a validated `delivery`/`activation` read them from `CompleteIdentity`
//! through [`CompleteIdentity::delivery`]/[`CompleteIdentity::activation`],
//! never from a raw string or presence vector directly.

use crate::mechanical_integrity::fail;
use crate::mechanical_integrity::instruction_topics::TopicPool;
use crate::types::Diagnostic;

pub const PRESCRIPTIVE_TYPES: &[&str] = &["standard", "contract", "protocol"];

/// §7's four independent answers — a closed enum, not a free string, so the
/// "declares X but not Y" diagnostic can never name a field that is not one
/// of the four.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormField {
    Topic,
    Profile,
    Delivery,
    Activation,
}

impl NormField {
    pub fn as_str(self) -> &'static str {
        match self {
            NormField::Topic => "topic",
            NormField::Profile => "profile",
            NormField::Delivery => "delivery",
            NormField::Activation => "activation",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "topic" => Some(NormField::Topic),
            "profile" => Some(NormField::Profile),
            "delivery" => Some(NormField::Delivery),
            "activation" => Some(NormField::Activation),
            _ => None,
        }
    }
}

/// The canonical order of the four fields, in the order the "half-declared"
/// diagnostic lists a missing one.
pub const NORM_FIELD_ORDER: [NormField; 4] = [
    NormField::Topic,
    NormField::Profile,
    NormField::Delivery,
    NormField::Activation,
];

/// The literal field names, in [`NORM_FIELD_ORDER`] — used by the transport
/// layer to build its per-field regexes; kept here so the two lists can
/// never drift apart.
pub const NORM_FIELDS: &[&str] = &["topic", "profile", "delivery", "activation"];

/// §5's closed `delivery` vocabulary — a real enum, not a string checked
/// against a slice, so an unmatched value can only ever be "not in the
/// pool", never silently accepted through a typo shared by both sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    KernelDoc,
    SkillPackage,
    CursorRule,
    AgentsMdSection,
}

impl Delivery {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "kernel-doc" => Some(Self::KernelDoc),
            "skill-package" => Some(Self::SkillPackage),
            "cursor-rule" => Some(Self::CursorRule),
            "agents-md-section" => Some(Self::AgentsMdSection),
            _ => None,
        }
    }
}

/// §5's closed `activation` vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activation {
    Always,
    PathGlob,
    TaskClass,
    Explicit,
}

impl Activation {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "always" => Some(Self::Always),
            "path-glob" => Some(Self::PathGlob),
            "task-class" => Some(Self::TaskClass),
            "explicit" => Some(Self::Explicit),
            _ => None,
        }
    }
}

/// A document's §7 declaration state, classified from its raw field
/// presence — the type itself makes "some but not all of the four fields"
/// an explicit, named state (`Partial`) rather than something every reader
/// of a raw `Vec<NormField>` has to re-derive by checking its length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Declaration {
    /// None of the four fields, and no `derived_from`, is present.
    Undeclared,
    /// At least one, but not all four, of the fields is present.
    Partial {
        present: Vec<NormField>,
        absent: Vec<NormField>,
    },
    /// All four fields are present.
    Complete,
}

/// Classifies a document's declaration state from its raw field presence.
pub fn classify_declaration(present: &[NormField], declares_parent: bool) -> Declaration {
    if present.is_empty() && !declares_parent {
        return Declaration::Undeclared;
    }
    let absent: Vec<NormField> = NORM_FIELD_ORDER
        .into_iter()
        .filter(|f| !present.contains(f))
        .collect();
    if absent.is_empty() {
        Declaration::Complete
    } else {
        Declaration::Partial {
            present: present.to_vec(),
            absent,
        }
    }
}

/// What one document contributed to the overall counts, alongside its own
/// diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Classification {
    /// At least one of the four fields (or `derived_from`) is declared —
    /// this document is an agent-instruction norm, whether or not it
    /// declared all four fields cleanly.
    DeclaredNorm,
    /// None of the four fields nor `derived_from` is present, and
    /// `document_type` is one of [`PRESCRIPTIVE_TYPES`].
    UndeclaredPrescriptive,
    /// None of the four fields nor `derived_from` is present, and
    /// `document_type` is anything else (including absent).
    UndeclaredOther,
}

/// Every independent reason [`build_identity`] could not produce a
/// [`CompleteIdentity`] for a declared (not [`Declaration::Undeclared`])
/// document. Collected exhaustively — [`build_identity`] never stops at the
/// first issue it finds — so a document with two unrelated defects (for
/// example a half declaration AND an unknown `delivery`) still reports
/// both, exactly as the Node.js reference's own independent per-field
/// checks do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityConstructionIssue {
    /// At least one, but not all four, of the §7 fields is present.
    HalfDeclared {
        present: Vec<NormField>,
        absent: Vec<NormField>,
    },
    /// `field` (`Delivery`, `Activation`, or `Topic`) was reported present
    /// by the caller's own presence flags, but no captured value was ALSO
    /// supplied for it — or the reverse: a captured value was supplied but
    /// the presence flags say the field is absent. A correct extraction
    /// over real Front Matter text can only reach the second direction (a
    /// multi-line YAML value the presence check's same-line pattern misses
    /// but the capture pattern's `\s*` still crosses into) — the first
    /// direction is proven unreachable for `Delivery`/`Activation`/`Topic`
    /// given how their presence and capture patterns relate, but
    /// [`build_identity`] still defends its own API boundary rather than
    /// assuming a caller-side invariant it cannot itself verify.
    IncoherentField(NormField),
    /// `delivery` was captured but is not one of §5's closed values.
    UnknownDelivery(String),
    /// `activation` was captured but is not one of §5's closed values.
    UnknownActivation(String),
}

/// One document's §7 declared identity, successfully constructed — a valid
/// domain value: private fields, constructible only through
/// [`build_identity`], and only when the issue list it also returns is
/// empty. `delivery`/`activation` are always the closed
/// [`Delivery`]/[`Activation`] enums, never a raw string or an
/// `Absent`/`Unknown` placeholder variant — those states can never reach
/// this type at all; they are [`IdentityConstructionIssue`]s instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompleteIdentity {
    delivery: Delivery,
    activation: Activation,
}

impl CompleteIdentity {
    pub fn delivery(&self) -> Delivery {
        self.delivery
    }

    pub fn activation(&self) -> Activation {
        self.activation
    }
}

/// The one pure construction function this module exposes: turns one
/// document's raw, already-extracted §7 facts into a construction report —
/// its [`Classification`], every independent
/// [`IdentityConstructionIssue`] found, and, only when that list is empty,
/// a [`CompleteIdentity`]. `document_type` is read only to classify an
/// [`Declaration::Undeclared`] document (`UndeclaredPrescriptive` vs
/// `UndeclaredOther`) — no identity is even attempted for it.
pub fn build_identity(
    present_fields: &[NormField],
    declares_parent: bool,
    document_type: Option<&str>,
    delivery: Option<&str>,
    activation: Option<&str>,
    topic: Option<&str>,
) -> (
    Classification,
    Vec<IdentityConstructionIssue>,
    Option<CompleteIdentity>,
) {
    let declaration = classify_declaration(present_fields, declares_parent);
    if declaration == Declaration::Undeclared {
        let classification = match document_type {
            Some(t) if PRESCRIPTIVE_TYPES.contains(&t) => Classification::UndeclaredPrescriptive,
            _ => Classification::UndeclaredOther,
        };
        return (classification, Vec::new(), None);
    }

    let mut issues = Vec::new();

    if let Declaration::Partial { present, absent } = &declaration {
        issues.push(IdentityConstructionIssue::HalfDeclared {
            present: present.clone(),
            absent: absent.clone(),
        });
    }

    if present_fields.contains(&NormField::Topic) != topic.is_some() {
        issues.push(IdentityConstructionIssue::IncoherentField(NormField::Topic));
    }
    if present_fields.contains(&NormField::Delivery) != delivery.is_some() {
        issues.push(IdentityConstructionIssue::IncoherentField(
            NormField::Delivery,
        ));
    }
    if present_fields.contains(&NormField::Activation) != activation.is_some() {
        issues.push(IdentityConstructionIssue::IncoherentField(
            NormField::Activation,
        ));
    }

    let delivery_value = delivery.and_then(Delivery::parse);
    if let Some(value) = delivery {
        if delivery_value.is_none() {
            issues.push(IdentityConstructionIssue::UnknownDelivery(
                value.to_string(),
            ));
        }
    }
    let activation_value = activation.and_then(Activation::parse);
    if let Some(value) = activation {
        if activation_value.is_none() {
            issues.push(IdentityConstructionIssue::UnknownActivation(
                value.to_string(),
            ));
        }
    }

    let complete = if issues.is_empty() && declaration == Declaration::Complete {
        match (delivery_value, activation_value) {
            (Some(delivery), Some(activation)) => Some(CompleteIdentity {
                delivery,
                activation,
            }),
            _ => None,
        }
    } else {
        None
    };

    (Classification::DeclaredNorm, issues, complete)
}

/// Maps one construction issue to its observable diagnostic — except
/// [`IdentityConstructionIssue::IncoherentField`], which stays internal:
/// it still prevents [`CompleteIdentity`] construction, but it is never
/// mapped to an additional user-visible `Diagnostic` of its own (corrective
/// round, `meridian-cli-foundation-architecture-remediation`, fourth
/// round, item 3). The one direction real Front Matter extraction can
/// reach for it (a captured value whose presence flag is absent) only ever
/// occurs alongside a [`Declaration::Partial`], which already produces
/// `HalfDeclared`'s own diagnostic — the SAME observable defect Node
/// reports, not a second, Rust-only one. The other direction (a presence
/// flag with no captured value) is proven unreachable by real extraction
/// (this module's own doc comment); `build_identity` still refuses to
/// construct a `CompleteIdentity` for it, silently, rather than assume the
/// caller-side invariant.
fn issue_diagnostic(rel: &str, issue: &IdentityConstructionIssue) -> Option<Diagnostic> {
    match issue {
        IdentityConstructionIssue::HalfDeclared { present, absent } => Some(fail(format!(
            "agent-instruction-identity: {rel} declares {} but not {}; §7 is four independent answers, and a half-declared norm is exactly the drift this standard was written against",
            present.iter().map(|f| f.as_str()).collect::<Vec<_>>().join(", "),
            absent.iter().map(|f| f.as_str()).collect::<Vec<_>>().join(", ")
        ))),
        IdentityConstructionIssue::UnknownDelivery(value) => Some(fail(format!(
            "agent-instruction-identity: {rel} declares delivery \"{value}\", which is not in the pool of §5"
        ))),
        IdentityConstructionIssue::UnknownActivation(value) => Some(fail(format!(
            "agent-instruction-identity: {rel} declares activation \"{value}\", which is not in the pool of §5"
        ))),
        IdentityConstructionIssue::IncoherentField(_) => None,
    }
}

/// Evaluates §7 for one document: which [`Classification`] it falls into,
/// plus every diagnostic its own facts produce. `rel` is the document's
/// Kernel-relative path, already computed by the caller, used verbatim in
/// every diagnostic. `topic_pool` is the typed, agreed `instruction-topics`
/// pool — a topic this check rejects must never be one `instruction-topics`
/// itself already rejected.
///
/// The `topic`/`derived_from` business checks below run whenever the
/// document is declared at all (`classification == DeclaredNorm`) —
/// independently of whether a [`CompleteIdentity`] was produced, exactly
/// matching the Node.js reference, which checks every one of §7's
/// independent facts unconditionally once a document has declared ANY of
/// them. Only `delivery`/`activation` validity moved into
/// [`build_identity`]'s construction-report issues; `topic` and
/// `derived_from` were never part of "is there a complete, coherent
/// identity" to begin with.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_document(
    rel: &str,
    present_fields: &[NormField],
    declares_parent: bool,
    document_type: Option<&str>,
    delivery: Option<&str>,
    activation: Option<&str>,
    topic: Option<&str>,
    has_unclassified_reason: bool,
    derived_from_is_scalar: bool,
    has_narrowing: bool,
    topic_pool: Option<&TopicPool>,
) -> (Classification, Vec<Diagnostic>) {
    let (classification, issues, _complete) = build_identity(
        present_fields,
        declares_parent,
        document_type,
        delivery,
        activation,
        topic,
    );

    if classification != Classification::DeclaredNorm {
        return (classification, Vec::new());
    }

    let mut diagnostics: Vec<Diagnostic> = issues
        .iter()
        .filter_map(|issue| issue_diagnostic(rel, issue))
        .collect();

    if let Some(topic) = topic {
        if topic == "unclassified" {
            if !has_unclassified_reason {
                diagnostics.push(fail(format!(
                    "agent-instruction-identity: {rel} carries topic \"unclassified\" with no unclassified_reason; the state is legal, an unexplained one is not"
                )));
            }
        } else if let Some(pool) = topic_pool {
            if !pool.contains(topic) {
                diagnostics.push(fail(format!(
                    "agent-instruction-identity: {rel} names topic \"{topic}\", which is not in the pool; a new topic is a change to the registry, not to one document"
                )));
            }
        }
    }

    if derived_from_is_scalar {
        diagnostics.push(fail(format!(
            "agent-instruction-identity: {rel} records derived_from as a scalar; the parent reference is a mapping of repository, path and revision"
        )));
    } else if declares_parent && !has_narrowing {
        diagnostics.push(fail(format!(
            "agent-instruction-identity: {rel} declares derived_from with no narrowing list; an edition that does not say what it removed is not distinguishable from a text written independently"
        )));
    }

    (classification, diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mechanical_integrity::instruction_topics::TopicId;

    fn pool(topics: &[&str]) -> TopicPool {
        topics.iter().map(|t| TopicId::new(*t).unwrap()).collect()
    }

    /// Test-only stand-in for the raw Front Matter facts the app transport
    /// layer extracts — `meridian-core` itself owns no such raw/unvalidated
    /// struct.
    #[derive(Default)]
    struct Facts {
        present_fields: Vec<NormField>,
        declares_parent: bool,
        document_type: Option<String>,
        delivery: Option<String>,
        activation: Option<String>,
        topic: Option<String>,
        has_unclassified_reason: bool,
        derived_from_is_scalar: bool,
        has_narrowing: bool,
    }

    fn facts() -> Facts {
        Facts {
            present_fields: vec![
                NormField::Topic,
                NormField::Profile,
                NormField::Delivery,
                NormField::Activation,
            ],
            delivery: Some("kernel-doc".to_string()),
            activation: Some("always".to_string()),
            topic: Some("agent-conduct".to_string()),
            ..Default::default()
        }
    }

    fn call(
        rel: &str,
        f: &Facts,
        topic_pool: Option<&TopicPool>,
    ) -> (Classification, Vec<Diagnostic>) {
        evaluate_document(
            rel,
            &f.present_fields,
            f.declares_parent,
            f.document_type.as_deref(),
            f.delivery.as_deref(),
            f.activation.as_deref(),
            f.topic.as_deref(),
            f.has_unclassified_reason,
            f.derived_from_is_scalar,
            f.has_narrowing,
            topic_pool,
        )
    }

    fn build(
        f: &Facts,
    ) -> (
        Classification,
        Vec<IdentityConstructionIssue>,
        Option<CompleteIdentity>,
    ) {
        build_identity(
            &f.present_fields,
            f.declares_parent,
            f.document_type.as_deref(),
            f.delivery.as_deref(),
            f.activation.as_deref(),
            f.topic.as_deref(),
        )
    }

    #[test]
    fn a_fully_declared_norm_is_clean() {
        let p = pool(&["agent-conduct"]);
        let (classification, diagnostics) = call("doc.md", &facts(), Some(&p));
        assert_eq!(classification, Classification::DeclaredNorm);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn a_half_declared_norm_is_a_failure() {
        let mut f = facts();
        f.present_fields = vec![NormField::Topic];
        f.delivery = None;
        f.activation = None;
        let (classification, diagnostics) = call("doc.md", &f, None);
        assert_eq!(classification, Classification::DeclaredNorm);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message().contains("but not"));
    }

    #[test]
    fn classify_declaration_names_the_three_states_explicitly() {
        assert_eq!(classify_declaration(&[], false), Declaration::Undeclared);
        assert!(matches!(
            classify_declaration(&[NormField::Topic], false),
            Declaration::Partial { .. }
        ));
        assert_eq!(
            classify_declaration(&NORM_FIELD_ORDER, false),
            Declaration::Complete
        );
    }

    #[test]
    fn undeclared_prescriptive_vs_other() {
        let f = Facts {
            document_type: Some("standard".to_string()),
            ..Default::default()
        };
        let (classification, _) = call("doc.md", &f, None);
        assert_eq!(classification, Classification::UndeclaredPrescriptive);

        let f2 = Facts::default();
        let (classification2, _) = call("doc.md", &f2, None);
        assert_eq!(classification2, Classification::UndeclaredOther);
    }

    #[test]
    fn unknown_delivery_is_rejected_even_when_every_field_present() {
        let mut f = facts();
        f.delivery = Some("made-up".to_string());
        let (_, diagnostics) = call("doc.md", &f, None);
        assert!(diagnostics
            .iter()
            .any(|d| d.message().contains("not in the pool of §5")));
    }

    #[test]
    fn unclassified_without_reason_is_a_failure() {
        let mut f = facts();
        f.topic = Some("unclassified".to_string());
        f.has_unclassified_reason = false;
        let (_, diagnostics) = call("doc.md", &f, None);
        assert!(diagnostics
            .iter()
            .any(|d| d.message().contains("unclassified_reason")));
    }

    #[test]
    fn a_topic_absent_from_the_pool_is_a_failure() {
        let f = facts();
        let p = pool(&[]);
        let (_, diagnostics) = call("doc.md", &f, Some(&p));
        assert!(diagnostics
            .iter()
            .any(|d| d.message().contains("not in the pool; a new topic")));
    }

    #[test]
    fn scalar_derived_from_is_a_failure() {
        let mut f = facts();
        f.derived_from_is_scalar = true;
        let p = pool(&["agent-conduct"]);
        let (_, diagnostics) = call("doc.md", &f, Some(&p));
        assert!(diagnostics
            .iter()
            .any(|d| d.message().contains("as a scalar")));
    }

    #[test]
    fn declared_parent_with_no_narrowing_is_a_failure() {
        let mut f = facts();
        f.declares_parent = true;
        f.has_narrowing = false;
        let p = pool(&["agent-conduct"]);
        let (_, diagnostics) = call("doc.md", &f, Some(&p));
        assert!(diagnostics
            .iter()
            .any(|d| d.message().contains("no narrowing list")));
    }

    #[test]
    fn a_document_with_two_independent_problems_gets_both_diagnostics() {
        let mut f = facts();
        f.present_fields = vec![NormField::Topic, NormField::Delivery];
        f.delivery = Some("made-up".to_string());
        f.activation = None;
        let (classification, diagnostics) = call("doc.md", &f, None);
        assert_eq!(classification, Classification::DeclaredNorm);
        assert!(
            diagnostics.iter().any(|d| d.message().contains("but not")),
            "{diagnostics:?}"
        );
        assert!(
            diagnostics
                .iter()
                .any(|d| d.message().contains("not in the pool of §5")),
            "{diagnostics:?}"
        );
    }

    // --- build_identity: the construction report itself ---

    #[test]
    fn partial_declaration_means_complete_identity_is_none() {
        let mut f = facts();
        f.present_fields = vec![NormField::Topic];
        f.delivery = None;
        f.activation = None;
        let (classification, issues, complete) = build(&f);
        assert_eq!(classification, Classification::DeclaredNorm);
        assert!(matches!(
            issues.as_slice(),
            [IdentityConstructionIssue::HalfDeclared { .. }]
        ));
        assert!(complete.is_none());
    }

    #[test]
    fn unknown_delivery_means_complete_identity_is_none() {
        let mut f = facts();
        f.delivery = Some("made-up".to_string());
        let (_, issues, complete) = build(&f);
        assert!(issues
            .iter()
            .any(|i| matches!(i, IdentityConstructionIssue::UnknownDelivery(v) if v == "made-up")));
        assert!(complete.is_none());
    }

    #[test]
    fn unknown_activation_means_complete_identity_is_none() {
        let mut f = facts();
        f.activation = Some("made-up".to_string());
        let (_, issues, complete) = build(&f);
        assert!(issues.iter().any(
            |i| matches!(i, IdentityConstructionIssue::UnknownActivation(v) if v == "made-up")
        ));
        assert!(complete.is_none());
    }

    #[test]
    fn incoherent_field_marked_present_but_value_absent_means_complete_identity_is_none() {
        // Direct constructor call with a self-contradictory input: the
        // caller's presence flags say Delivery IS present, but no captured
        // value was supplied for it. A real regex-based extraction can
        // never produce this combination (proven in this module's own doc
        // comment), but `build_identity` still defends its own API
        // boundary rather than assuming that invariant.
        let (classification, issues, complete) = build_identity(
            &[
                NormField::Topic,
                NormField::Profile,
                NormField::Delivery,
                NormField::Activation,
            ],
            false,
            None,
            None, // delivery: marked present above, but no value supplied
            Some("always"),
            Some("agent-conduct"),
        );
        assert_eq!(classification, Classification::DeclaredNorm);
        assert!(issues.contains(&IdentityConstructionIssue::IncoherentField(
            NormField::Delivery
        )));
        assert!(complete.is_none());
    }

    #[test]
    fn incoherent_field_value_present_but_marked_absent_means_complete_identity_is_none() {
        // The reachable-in-practice direction: a value was captured (e.g. a
        // multi-line YAML value the presence check's same-line pattern
        // misses) but the presence flags do not list the field.
        let (classification, issues, complete) = build_identity(
            &[NormField::Topic, NormField::Profile, NormField::Activation],
            false,
            None,
            Some("kernel-doc"), // delivery: a value was captured, but the field is not in present_fields
            Some("always"),
            Some("agent-conduct"),
        );
        assert_eq!(classification, Classification::DeclaredNorm);
        assert!(issues.contains(&IdentityConstructionIssue::IncoherentField(
            NormField::Delivery
        )));
        // Also half-declared, since present_fields is missing Delivery —
        // both issues are independently, simultaneously true and BOTH
        // reported (never silently collapsed to one).
        assert!(issues
            .iter()
            .any(|i| matches!(i, IdentityConstructionIssue::HalfDeclared { .. })));
        assert!(complete.is_none());
    }

    #[test]
    fn two_independent_construction_defects_both_remain_reported() {
        let mut f = facts();
        f.present_fields = vec![NormField::Topic, NormField::Delivery];
        f.delivery = Some("made-up".to_string());
        f.activation = None;
        let (_, issues, complete) = build(&f);
        assert!(issues
            .iter()
            .any(|i| matches!(i, IdentityConstructionIssue::HalfDeclared { .. })));
        assert!(issues
            .iter()
            .any(|i| matches!(i, IdentityConstructionIssue::UnknownDelivery(_))));
        assert_eq!(issues.len(), 2, "{issues:?}");
        assert!(complete.is_none());
    }

    #[test]
    fn complete_known_values_construct_successfully() {
        let f = facts();
        let (classification, issues, complete) = build(&f);
        assert_eq!(classification, Classification::DeclaredNorm);
        assert!(issues.is_empty(), "{issues:?}");
        let identity = complete.expect("fully declared, coherent, known values must construct");
        assert_eq!(identity.delivery(), Delivery::KernelDoc);
        assert_eq!(identity.activation(), Activation::Always);
    }

    #[test]
    fn incoherent_field_stays_internal_evaluate_document_reports_exactly_half_declared() {
        // Item 3 of the fourth corrective round: the reachable-in-practice
        // IncoherentField direction (a captured value whose presence flag
        // is absent — the `delivery:\n  kernel-doc` shape) must produce
        // EXACTLY the same single diagnostic Node produces (HalfDeclared),
        // never an additional Rust-only "presence/value pair" message.
        let (classification, diagnostics) = evaluate_document(
            "doc.md",
            &[NormField::Topic, NormField::Profile, NormField::Activation],
            false,
            None,
            Some("kernel-doc"), // captured despite Delivery being absent from present_fields
            Some("always"),
            Some("agent-conduct"),
            false,
            false,
            false,
            None,
        );
        assert_eq!(classification, Classification::DeclaredNorm);
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert!(diagnostics[0].message().contains("but not"));
        assert!(!diagnostics[0].message().contains("presence/value"));
    }
}
