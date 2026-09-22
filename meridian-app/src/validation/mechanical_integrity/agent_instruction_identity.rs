//! App-owned orchestration for `agent-instruction-identity` (package 7,
//! subpackage 7a): owns candidate selection (which Markdown paths this
//! check reads at all — `carries_own_front_matter`, sort, dedup; moved here
//! from the CLI shim by the corrective round,
//! `meridian-cli-foundation-architecture-remediation`, item 3), reads each
//! candidate through the [`WorkspaceReader`] port, extracts its Front
//! Matter facts with regexes (this module owns them — `meridian-core` takes
//! no `fancy-regex` dependency), and hands each document's facts to
//! `meridian_core::mechanical_integrity::agent_instruction_identity` for §7
//! itself. A read failure for a candidate — `NotFound` included — has no
//! designed domain meaning for THIS check (unlike, say, `sha-provenance`'s
//! absent `PIN.yaml`): every `candidate_paths` entry was already found to
//! exist by the caller's own file walk, so any failure to read it back is a
//! walk/environment inconsistency, not a legitimate "this document has no
//! Front Matter" outcome, and propagates as [`OperationError`].

use fancy_regex::Regex;
use meridian_core::mechanical_integrity::agent_instruction_identity::{
    self as checks, Classification, NormField, NORM_FIELDS,
};
use meridian_core::mechanical_integrity::instruction_topics::TopicPool;
use meridian_core::types::{Diagnostic, WorkspaceRelativePath};

use super::{regex_capture1, regex_is_match, OperationError};
use crate::source_format::carries_own_front_matter;
use crate::workspace::{ReadError, WorkspaceReader};

/// One document's raw, not-yet-validated Front Matter facts — this crate's
/// own transport projection (corrective round,
/// `meridian-cli-foundation-architecture-remediation`, item 1: "Keep raw
/// field presence and raw strings in an app-owned transport projection").
/// `meridian_core::mechanical_integrity::agent_instruction_identity` owns no
/// struct like this any more; its `CompleteIdentity` domain type (and the
/// `IdentityConstructionIssue`s a document can produce instead) is built
/// straight from these already-extracted scalars by
/// [`checks::build_identity`]/[`checks::evaluate_document`].
struct RawFrontMatter {
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

#[derive(Debug)]
pub struct Outcome {
    pub diagnostics: Vec<Diagnostic>,
    pub declared_norms: usize,
    pub undeclared_prescriptive: usize,
    pub undeclared_other: usize,
}

/// Selects, sorts and deduplicates `agent-instruction-identity`'s candidate
/// set out of every Markdown path the caller's own (out-of-scope) file walk
/// found — the same `carries_own_front_matter` rule `document-identity`
/// applies, shared through `crate::source_format`, never re-derived here.
fn select_candidates(all_markdown_paths: &[WorkspaceRelativePath]) -> Vec<WorkspaceRelativePath> {
    let mut candidates: Vec<WorkspaceRelativePath> = all_markdown_paths
        .iter()
        .filter(|p| carries_own_front_matter(p.as_str()))
        .cloned()
        .collect();
    candidates.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    candidates.dedup_by(|a, b| a.as_str() == b.as_str());
    candidates
}

pub fn run(
    reader: &dyn WorkspaceReader,
    all_markdown_paths: &[WorkspaceRelativePath],
    topic_pool: Option<&TopicPool>,
) -> Result<Outcome, OperationError> {
    let mut diagnostics = Vec::new();
    let mut declared_norms = 0usize;
    let mut undeclared_prescriptive = 0usize;
    let mut undeclared_other = 0usize;

    let field_re: Vec<(NormField, Regex)> = NORM_FIELDS
        .iter()
        .map(|f| {
            let field = NormField::parse(f).expect("NORM_FIELDS and NormField::parse agree");
            (
                field,
                Regex::new(&format!(r"(?m)^{f}:[^\S\r\n]*\S")).expect("field pattern compiles"),
            )
        })
        .collect();
    let declares_parent_re =
        Regex::new(r"(?m)^derived_from:").expect("declares-parent pattern compiles");
    let document_type_re =
        Regex::new(r"(?m)^document_type:\s*(\S+)").expect("document-type pattern compiles");
    let delivery_re = Regex::new(r"(?m)^delivery:\s*(\S+)").expect("delivery pattern compiles");
    let activation_re =
        Regex::new(r"(?m)^activation:\s*(\S+)").expect("activation pattern compiles");
    let topic_re = Regex::new(r"(?m)^topic:\s*(\S+)").expect("topic pattern compiles");
    let unclassified_reason_re = Regex::new(r"(?m)^unclassified_reason:[^\S\r\n]*\S")
        .expect("unclassified-reason pattern compiles");
    let derived_from_scalar_re = Regex::new(r"(?m)^derived_from:[^\S\r\n]*\S")
        .expect("derived-from-scalar pattern compiles");
    let narrowing_re = Regex::new(r"(?m)^narrowing:").expect("narrowing pattern compiles");

    for path in select_candidates(all_markdown_paths) {
        let text = match reader.read_text(&path) {
            Ok(text) => text,
            Err(ReadError::NotFound) => {
                return Err(OperationError {
                    path: path.as_str().to_string(),
                    message: "candidate document was found by the file walk but no longer exists; a walk/read inconsistency, not a designed \"no Front Matter\" outcome".to_string(),
                });
            }
            Err(ReadError::Io(message)) => {
                return Err(OperationError {
                    path: path.as_str().to_string(),
                    message,
                });
            }
        };
        if !text.starts_with("---") {
            continue;
        }
        let end = text[3..].find("\n---").map(|i| i + 3);
        let fm = match end {
            Some(end) => &text[4..end],
            None => "",
        };

        let mut present_fields = Vec::new();
        for (field, re) in &field_re {
            if regex_is_match(re, fm, path.as_str())? {
                present_fields.push(*field);
            }
        }

        let facts = RawFrontMatter {
            present_fields,
            declares_parent: regex_is_match(&declares_parent_re, fm, path.as_str())?,
            document_type: regex_capture1(&document_type_re, fm, path.as_str())?,
            delivery: regex_capture1(&delivery_re, fm, path.as_str())?,
            activation: regex_capture1(&activation_re, fm, path.as_str())?,
            topic: regex_capture1(&topic_re, fm, path.as_str())?,
            has_unclassified_reason: regex_is_match(&unclassified_reason_re, fm, path.as_str())?,
            derived_from_is_scalar: regex_is_match(&derived_from_scalar_re, fm, path.as_str())?,
            has_narrowing: regex_is_match(&narrowing_re, fm, path.as_str())?,
        };

        let (classification, doc_diagnostics) = checks::evaluate_document(
            path.as_str(),
            &facts.present_fields,
            facts.declares_parent,
            facts.document_type.as_deref(),
            facts.delivery.as_deref(),
            facts.activation.as_deref(),
            facts.topic.as_deref(),
            facts.has_unclassified_reason,
            facts.derived_from_is_scalar,
            facts.has_narrowing,
            topic_pool,
        );
        diagnostics.extend(doc_diagnostics);
        match classification {
            Classification::DeclaredNorm => declared_norms += 1,
            Classification::UndeclaredPrescriptive => undeclared_prescriptive += 1,
            Classification::UndeclaredOther => undeclared_other += 1,
        }
    }

    Ok(Outcome {
        diagnostics,
        declared_norms,
        undeclared_prescriptive,
        undeclared_other,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation::mechanical_integrity::tests::FakeReader;
    use meridian_core::mechanical_integrity::instruction_topics::TopicId;

    fn path(s: &str) -> WorkspaceRelativePath {
        WorkspaceRelativePath::new(s).unwrap()
    }

    fn pool(topics: &[&str]) -> TopicPool {
        topics.iter().map(|t| TopicId::new(*t).unwrap()).collect()
    }

    #[test]
    fn a_fully_declared_norm_is_clean() {
        let reader = FakeReader::new().with_file(
            "doc.md",
            "---\ntopic: agent-conduct\nprofile: universal\ndelivery: kernel-doc\nactivation: always\n---\nbody\n",
        );
        let p = pool(&["agent-conduct"]);
        let outcome = run(&reader, &[path("doc.md")], Some(&p)).unwrap();
        assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
        assert_eq!(outcome.declared_norms, 1);
    }

    #[test]
    fn a_document_with_no_front_matter_marker_is_skipped_entirely() {
        let reader = FakeReader::new().with_file("doc.md", "no front matter here\n");
        let outcome = run(&reader, &[path("doc.md")], None).unwrap();
        assert!(outcome.diagnostics.is_empty());
        assert_eq!(outcome.declared_norms, 0);
        assert_eq!(outcome.undeclared_other, 0);
        assert_eq!(outcome.undeclared_prescriptive, 0);
    }

    #[test]
    fn a_candidate_excluded_by_carries_own_front_matter_is_never_read() {
        // "SKILL.md" is one of the excluded names; if it were read despite
        // that, its Front Matter (declaring an unknown delivery) would
        // produce a diagnostic. select_candidates() must have dropped it
        // before any read is attempted.
        let reader = FakeReader::new().with_file(
            "skills/demo/SKILL.md",
            "---\ntopic: a\nprofile: b\ndelivery: made-up\nactivation: always\n---\n",
        );
        let outcome = run(&reader, &[path("skills/demo/SKILL.md")], None).unwrap();
        assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
        assert_eq!(outcome.declared_norms, 0);
    }

    #[test]
    fn duplicate_candidate_paths_are_evaluated_once() {
        let reader = FakeReader::new().with_file("doc.md", "---\ndocument_type: standard\n---\n");
        let outcome = run(&reader, &[path("doc.md"), path("doc.md")], None).unwrap();
        assert_eq!(outcome.undeclared_prescriptive, 1);
    }

    #[test]
    fn a_missing_candidate_propagates_as_an_operation_error_not_a_silent_skip() {
        let reader = FakeReader::new();
        let error = run(&reader, &[path("missing.md")], None).unwrap_err();
        assert_eq!(error.path, "missing.md");
    }

    #[test]
    fn an_io_failure_reading_a_candidate_propagates_as_an_operation_error() {
        let reader = FakeReader::new().with_error(
            "doc.md",
            crate::workspace::ReadError::Io("permission denied".to_string()),
        );
        let error = run(&reader, &[path("doc.md")], None).unwrap_err();
        assert_eq!(error.path, "doc.md");
        assert!(error.message.contains("permission denied"));
    }

    #[test]
    fn a_half_declared_norm_is_a_failure() {
        let reader =
            FakeReader::new().with_file("doc.md", "---\ntopic: agent-conduct\n---\nbody\n");
        let outcome = run(&reader, &[path("doc.md")], None).unwrap();
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(outcome.diagnostics[0].message().contains("but not"));
        assert_eq!(outcome.declared_norms, 1);
    }

    #[test]
    fn a_multiline_delivery_value_is_reported_as_exactly_half_declared() {
        // Item 3 of the fourth corrective round: reproduces the one
        // reachable-in-practice IncoherentField direction through the REAL
        // regex extraction this module owns — `delivery:` with its value on
        // the NEXT line. The presence check (`^delivery:[^\S\r\n]*\S`,
        // same-line only) misses it, so Delivery is absent from
        // `present_fields`; the capture (`^delivery:\s*(\S+)`, crosses
        // newlines) still reads "kernel-doc". `meridian_core` must report
        // EXACTLY the ordinary half-declared diagnostic Node would produce
        // for a document missing `delivery` — never an additional
        // Rust-only "presence/value pair" message.
        let reader = FakeReader::new().with_file(
            "doc.md",
            "---\ntopic: agent-conduct\nprofile: universal\ndelivery:\n  kernel-doc\nactivation: always\n---\nbody\n",
        );
        let outcome = run(&reader, &[path("doc.md")], None).unwrap();
        assert_eq!(outcome.diagnostics.len(), 1, "{:?}", outcome.diagnostics);
        assert!(outcome.diagnostics[0]
            .message()
            .contains("but not delivery"));
        assert!(!outcome.diagnostics[0].message().contains("presence/value"));
        assert_eq!(outcome.declared_norms, 1);
    }

    #[test]
    fn an_undeclared_prescriptive_document_is_counted_not_failed() {
        let reader =
            FakeReader::new().with_file("doc.md", "---\ndocument_type: standard\n---\nbody\n");
        let outcome = run(&reader, &[path("doc.md")], None).unwrap();
        assert!(outcome.diagnostics.is_empty());
        assert_eq!(outcome.undeclared_prescriptive, 1);
    }
}
