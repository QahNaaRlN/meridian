//! App-owned orchestration for `operating-foundation` (package 7,
//! subpackage 7a): reads the YAML pool and the two Markdown documentation
//! files through the [`WorkspaceReader`] port, extracts entries/rows with
//! this crate's own strict adapters, and hands them to
//! `meridian_core::mechanical_integrity::operating_foundation` for the
//! predicate and diagnostic text.

use fancy_regex::Regex;
use meridian_core::mechanical_integrity::operating_foundation::{
    self as checks, FoundationEntry, FoundationEntryError, FoundationRow,
};
use meridian_core::types::{Diagnostic, DiagnosticLevel, WorkspaceRelativePath};
use serde_json::Value;

/// The raw, not-yet-validated shape of one `terms[]`/`principles[]` entry,
/// exactly as extracted from parsed YAML — an empty (or absent) `id` is
/// representable here, unlike the validated
/// [`meridian_core::mechanical_integrity::operating_foundation::FoundationEntry`]
/// domain type it is resolved into by [`resolve_entries`] (corrective
/// round, `meridian-cli-foundation-architecture-remediation`, item 1: "Keep
/// raw field presence and raw strings in an app-owned transport
/// projection").
struct RawFoundationEntry {
    id: String,
    ru: String,
    en: String,
    machine_names: Vec<String>,
}

use super::{read_text, regex_collect_captures, OperationError, ReadOutcome};
use crate::source_format::{parse_yaml, regions::marked_region};
use crate::workspace::WorkspaceReader;

#[derive(Debug)]
pub struct Outcome {
    pub diagnostics: Vec<Diagnostic>,
}

fn fail(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticLevel::Fail, message).expect("message is non-empty")
}

const YAML_PATH: &str = "standards/workspace/operating-foundation.yaml";
const GLOSSARY_PATH: &str = "standards/workspace/operating-glossary.md";
const PRINCIPLES_PATH: &str = "standards/workspace/operating-principles.md";

pub fn run(reader: &dyn WorkspaceReader) -> Result<Outcome, OperationError> {
    let yaml_path = WorkspaceRelativePath::new(YAML_PATH).expect("literal path is valid");
    let glossary_path = WorkspaceRelativePath::new(GLOSSARY_PATH).expect("literal path is valid");
    let principles_path =
        WorkspaceRelativePath::new(PRINCIPLES_PATH).expect("literal path is valid");

    let yaml_raw = match read_text(reader, &yaml_path)? {
        ReadOutcome::Present(text) => Some(text),
        ReadOutcome::Absent => None,
    };
    let glossary_raw = match read_text(reader, &glossary_path)? {
        ReadOutcome::Present(text) => Some(text),
        ReadOutcome::Absent => None,
    };
    let principles_raw = match read_text(reader, &principles_path)? {
        ReadOutcome::Present(text) => Some(text),
        ReadOutcome::Absent => None,
    };

    if yaml_raw.is_none() || glossary_raw.is_none() || principles_raw.is_none() {
        let mut missing = Vec::new();
        if yaml_raw.is_none() {
            missing.push(YAML_PATH);
        }
        if glossary_raw.is_none() {
            missing.push(GLOSSARY_PATH);
        }
        if principles_raw.is_none() {
            missing.push(PRINCIPLES_PATH);
        }
        return Ok(Outcome {
            diagnostics: vec![checks::missing_pool_files(&missing)],
        });
    }
    let yaml_raw = yaml_raw.unwrap();
    let glossary_raw = glossary_raw.unwrap();
    let principles_raw = principles_raw.unwrap();

    let mut diagnostics = Vec::new();

    let foundation = match parse_yaml(&yaml_raw) {
        Ok(value) => Some(value),
        Err(error) => {
            diagnostics.push(fail(format!("operating-foundation: {error}")));
            None
        }
    };

    let Some(foundation) = foundation else {
        return Ok(Outcome { diagnostics });
    };

    let term_rows = extract_rows(
        &glossary_raw,
        GLOSSARY_PATH,
        "operating-term-pool",
        "term-pool",
        "a definition",
        &mut diagnostics,
    )?;
    let principle_rows = extract_rows(
        &principles_raw,
        PRINCIPLES_PATH,
        "operating-principle-pool",
        "principle-pool",
        "a mandatory consequence",
        &mut diagnostics,
    )?;

    let raw_terms = extract_entries(foundation.get("terms"), true);
    let raw_principles = extract_entries(foundation.get("principles"), false);

    let terms = raw_terms.map(|raw| resolve_entries(raw, "term", &mut diagnostics));
    let principles = raw_principles.map(|raw| resolve_entries(raw, "principle", &mut diagnostics));

    diagnostics.extend(checks::check_pool(
        terms.as_deref(),
        &term_rows,
        "term",
        true,
    ));
    diagnostics.extend(checks::check_pool(
        principles.as_deref(),
        &principle_rows,
        "principle",
        false,
    ));

    Ok(Outcome { diagnostics })
}

fn extract_rows(
    raw: &str,
    path: &str,
    region_id: &str,
    label: &str,
    body_label: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Vec<FoundationRow>, OperationError> {
    let region = match marked_region(raw, region_id) {
        Ok(region) => region,
        Err(error) => {
            diagnostics.push(checks::region_unreadable(label, &error));
            return Ok(Vec::new());
        }
    };
    let row_re = Regex::new(
        r"(?m)^\|\s*`([a-z][a-z0-9]*(?:-[a-z0-9]+)*)`\s*\|\s*([^|]+?)\s*\|\s*([^|]+?)\s*\|\s*([^|]*?)\s*\|",
    )
    .expect("row pattern compiles");
    let mut rows = Vec::new();
    for c in regex_collect_captures(&row_re, &region.text, path)? {
        let id = c.get(1).map(|g| g.as_str().to_string()).unwrap_or_default();
        let ru = c
            .get(2)
            .map(|g| g.as_str().trim().to_string())
            .unwrap_or_default();
        let en = c
            .get(3)
            .map(|g| g.as_str().trim().to_string())
            .unwrap_or_default();
        let body = c
            .get(4)
            .map(|g| g.as_str().trim().to_string())
            .unwrap_or_default();
        // Only `id` gates construction (item 2, fourth corrective round): a
        // row's id/ru/en signature must stay usable for `check_pool` even
        // with a blank `body` — `empty_body_row_ids` below reports that
        // separately, from the SAME constructed rows, never by excluding
        // the row from `rows` (which would make its id vanish from pool
        // agreement and produce a false "halves disagree").
        match FoundationRow::new(id, ru, en, body) {
            Ok(row) => rows.push(row),
            // The `id` capturing group requires at least one character
            // (`[a-z][a-z0-9]*(?:-[a-z0-9]+)*`) for the row pattern to have
            // matched at all — `FoundationRow::new` rejecting it can never
            // actually trigger here; propagated as an `OperationError` (an
            // invariant violation, not a domain outcome) rather than
            // silently dropped.
            Err(error) => {
                return Err(OperationError {
                    path: path.to_string(),
                    message: format!(
                        "a documentation table row matched with an empty id, which the row pattern should make impossible: {error}"
                    ),
                });
            }
        }
    }
    let empty_body_ids = checks::empty_body_row_ids(&rows);
    if !empty_body_ids.is_empty() {
        diagnostics.push(checks::rows_without_body(
            label,
            body_label,
            &empty_body_ids,
        ));
    }
    Ok(rows)
}

/// Resolves the raw, not-yet-validated `entries` into successfully
/// constructed domain [`FoundationEntry`] values, reporting
/// [`checks::entry_missing_id`] once for every raw entry whose `id` was
/// missing or empty rather than silently dropping it (corrective round,
/// `meridian-cli-foundation-architecture-remediation`) — the resolved list
/// handed to [`checks::check_pool`] can therefore never contain an entry
/// construction failed to build. A missing/empty `ru`/`en` does NOT stop
/// construction (item 2, fourth corrective round): the entry's `id` stays
/// available to duplicate/pool-membership checks, and a blank bilingual
/// name surfaces through `check_pool`'s own EXISTING mismatch comparison
/// wherever a documented row exists — never a new diagnostic.
fn resolve_entries(
    raw: Vec<RawFoundationEntry>,
    label: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<FoundationEntry> {
    let mut resolved = Vec::with_capacity(raw.len());
    let mut missing_id = 0usize;
    for entry in raw {
        match FoundationEntry::new(entry.id, entry.ru, entry.en, entry.machine_names) {
            Ok(entry) => resolved.push(entry),
            Err(FoundationEntryError::EmptyId) => missing_id += 1,
        }
    }
    if missing_id > 0 {
        diagnostics.push(checks::entry_missing_id(label, missing_id));
    }
    resolved
}

/// `entries` is `None` only when the YAML key was absent entirely or held a
/// non-array value — `meridian_core`'s [`checks::check_pool`] treats that as
/// "not an array", exactly matching the Node reference's own
/// `foundation.terms`/`.principles` (`undefined` there for an absent key)
/// being rejected by `Array.isArray`.
fn extract_entries(value: Option<&Value>, machine_names: bool) -> Option<Vec<RawFoundationEntry>> {
    let array = value.and_then(Value::as_array)?;
    Some(
        array
            .iter()
            .map(|entry| RawFoundationEntry {
                id: entry
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                ru: field(
                    entry,
                    if machine_names {
                        "canonical_ru"
                    } else {
                        "title_ru"
                    },
                ),
                en: field(
                    entry,
                    if machine_names {
                        "canonical_en"
                    } else {
                        "title_en"
                    },
                ),
                machine_names: entry
                    .get("machine_names")
                    .and_then(Value::as_array)
                    .map(|names| {
                        names
                            .iter()
                            .filter_map(Value::as_str)
                            .map(String::from)
                            .collect()
                    })
                    .unwrap_or_default(),
            })
            .collect(),
    )
}

fn field(entry: &Value, name: &str) -> String {
    entry
        .get(name)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validation::mechanical_integrity::tests::FakeReader;
    use crate::workspace::ReadError;

    fn write_agreeing_pool() -> FakeReader {
        FakeReader::new()
            .with_file(
                YAML_PATH,
                "terms:\n  - id: widget\n    canonical_ru: Виджет\n    canonical_en: Widget\n    machine_names: [widget]\nprinciples:\n  - id: safety-first\n    title_ru: Сначала безопасность\n    title_en: Safety first\n",
            )
            .with_file(
                GLOSSARY_PATH,
                "<!-- meridian:begin operating-term-pool -->\n| Id | RU | EN | Def |\n|---|---|---|---|\n| `widget` | Виджет | Widget | A thing |\n<!-- meridian:end operating-term-pool -->\n",
            )
            .with_file(
                PRINCIPLES_PATH,
                "<!-- meridian:begin operating-principle-pool -->\n| Id | RU | EN | Consequence |\n|---|---|---|---|\n| `safety-first` | Сначала безопасность | Safety first | Do not break things |\n<!-- meridian:end operating-principle-pool -->\n",
            )
    }

    #[test]
    fn agreeing_pools_produce_no_diagnostics() {
        let outcome = run(&write_agreeing_pool()).unwrap();
        assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    }

    #[test]
    fn a_missing_pool_file_fails_closed() {
        let outcome = run(&FakeReader::new()).unwrap();
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(outcome.diagnostics[0].message().contains("is incomplete"));
    }

    #[test]
    fn an_io_failure_reading_any_of_the_three_files_propagates_as_an_operation_error() {
        let reader = FakeReader::new()
            .with_file(GLOSSARY_PATH, "")
            .with_file(PRINCIPLES_PATH, "")
            .with_error(YAML_PATH, ReadError::Io("permission denied".to_string()));
        let error = run(&reader).unwrap_err();
        assert_eq!(error.path, YAML_PATH);
        assert!(error.message.contains("permission denied"));
    }

    #[test]
    fn an_entry_with_no_id_is_its_own_reported_defect_not_silently_dropped() {
        // End-to-end regression for the corrective round
        // (`meridian-cli-foundation-architecture-remediation`, item 1): the
        // raw entry with an empty id can no longer be constructed as a
        // domain `FoundationEntry` at all — `resolve_entries` must still
        // surface `entry_missing_id` for it, and the other, well-formed
        // entry must still be checked normally.
        let reader = FakeReader::new()
            .with_file(
                YAML_PATH,
                "terms:\n  - id: ''\n    canonical_ru: ''\n    canonical_en: ''\n  - id: widget\n    canonical_ru: Виджет\n    canonical_en: Widget\nprinciples: []\n",
            )
            .with_file(
                GLOSSARY_PATH,
                "<!-- meridian:begin operating-term-pool -->\n| Id | RU | EN | Def |\n|---|---|---|---|\n| `widget` | Виджет | Widget | A thing |\n<!-- meridian:end operating-term-pool -->\n",
            )
            .with_file(
                PRINCIPLES_PATH,
                "<!-- meridian:begin operating-principle-pool -->\n<!-- meridian:end operating-principle-pool -->\n",
            );
        let outcome = run(&reader).unwrap();
        assert!(
            outcome
                .diagnostics
                .iter()
                .any(|d| d.message().contains("have no id")),
            "{:?}",
            outcome.diagnostics
        );
        assert!(
            !outcome
                .diagnostics
                .iter()
                .any(|d| d.message().contains("disagree")),
            "the well-formed \"widget\" entry must still be checked normally: {:?}",
            outcome.diagnostics
        );
    }

    #[test]
    fn an_entry_with_an_empty_bilingual_name_surfaces_as_the_existing_mismatch_diagnostic() {
        // Item 2, fourth corrective round, preferred resolution: no
        // `entry_missing_bilingual_name` diagnostic — a blank `canonical_ru`
        // with a matching documented row (which carries a REAL value) must
        // map to the SAME "different bilingual names" mismatch Node itself
        // would report via plain string inequality (`"" !== "Виджет"`), and
        // must NOT make the entry's id look "absent" from documentation.
        let reader = FakeReader::new()
            .with_file(
                YAML_PATH,
                "terms:\n  - id: widget\n    canonical_ru: ''\n    canonical_en: Widget\nprinciples: []\n",
            )
            .with_file(
                GLOSSARY_PATH,
                "<!-- meridian:begin operating-term-pool -->\n| Id | RU | EN | Def |\n|---|---|---|---|\n| `widget` | Виджет | Widget | A thing |\n<!-- meridian:end operating-term-pool -->\n",
            )
            .with_file(
                PRINCIPLES_PATH,
                "<!-- meridian:begin operating-principle-pool -->\n<!-- meridian:end operating-principle-pool -->\n",
            );
        let outcome = run(&reader).unwrap();
        let messages: Vec<String> = outcome
            .diagnostics
            .iter()
            .map(|d| d.message().to_string())
            .collect();
        assert_eq!(
            messages,
            vec![
                "operating-foundation: term \"widget\" has different bilingual names in data and documentation".to_string(),
            ],
            "{messages:?}"
        );
    }

    #[test]
    fn a_documented_row_with_a_blank_body_cell_is_still_reported_by_rows_without_body() {
        let reader = FakeReader::new()
            .with_file(
                YAML_PATH,
                "terms:\n  - id: widget\n    canonical_ru: Виджет\n    canonical_en: Widget\nprinciples: []\n",
            )
            .with_file(
                GLOSSARY_PATH,
                "<!-- meridian:begin operating-term-pool -->\n| Id | RU | EN | Def |\n|---|---|---|---|\n| `widget` | Виджет | Widget | |\n<!-- meridian:end operating-term-pool -->\n",
            )
            .with_file(
                PRINCIPLES_PATH,
                "<!-- meridian:begin operating-principle-pool -->\n<!-- meridian:end operating-principle-pool -->\n",
            );
        let outcome = run(&reader).unwrap();
        let messages: Vec<String> = outcome
            .diagnostics
            .iter()
            .map(|d| d.message().to_string())
            .collect();
        assert_eq!(
            messages,
            vec!["operating-foundation: term-pool rows without a definition: widget".to_string()],
            "a blank body must not ALSO produce a false halves-disagree: {messages:?}"
        );
    }

    #[test]
    fn the_exact_full_diagnostic_list_for_two_simultaneous_independent_defects() {
        // Item 2's explicit testing requirement: assert the EXACT full
        // diagnostic list (not merely "contains"), for a fixture combining
        // TWO simultaneous, INDEPENDENT defects — a blank documented body
        // (on "widget") and a blank data bilingual name (on "gadget",
        // matched against a row carrying a real value) — proving both are
        // reported, neither suppresses the other or a well-formed sibling,
        // and no spurious "halves disagree" appears for either id.
        let reader = FakeReader::new()
            .with_file(
                YAML_PATH,
                "terms:\n  - id: widget\n    canonical_ru: Виджет\n    canonical_en: Widget\n  - id: gadget\n    canonical_ru: ''\n    canonical_en: Gadget\nprinciples: []\n",
            )
            .with_file(
                GLOSSARY_PATH,
                "<!-- meridian:begin operating-term-pool -->\n| Id | RU | EN | Def |\n|---|---|---|---|\n| `widget` | Виджет | Widget | |\n| `gadget` | Гаджет | Gadget | def |\n<!-- meridian:end operating-term-pool -->\n",
            )
            .with_file(
                PRINCIPLES_PATH,
                "<!-- meridian:begin operating-principle-pool -->\n<!-- meridian:end operating-principle-pool -->\n",
            );
        let outcome = run(&reader).unwrap();
        let messages: Vec<String> = outcome
            .diagnostics
            .iter()
            .map(|d| d.message().to_string())
            .collect();
        assert_eq!(
            messages,
            vec![
                "operating-foundation: term-pool rows without a definition: widget".to_string(),
                "operating-foundation: term \"gadget\" has different bilingual names in data and documentation".to_string(),
            ],
            "{messages:?}"
        );
    }

    #[test]
    fn a_machine_name_collision_is_a_failure() {
        let reader = FakeReader::new()
            .with_file(
                YAML_PATH,
                "terms:\n  - id: widget\n    canonical_ru: Виджет\n    canonical_en: Widget\n    machine_names: [shared]\n  - id: gadget\n    canonical_ru: Гаджет\n    canonical_en: Gadget\n    machine_names: [shared]\nprinciples: []\n",
            )
            .with_file(
                GLOSSARY_PATH,
                "<!-- meridian:begin operating-term-pool -->\n| Id | RU | EN | Def |\n|---|---|---|---|\n| `widget` | Виджет | Widget | A thing |\n| `gadget` | Гаджет | Gadget | Another thing |\n<!-- meridian:end operating-term-pool -->\n",
            )
            .with_file(
                PRINCIPLES_PATH,
                "<!-- meridian:begin operating-principle-pool -->\n<!-- meridian:end operating-principle-pool -->\n",
            );
        let outcome = run(&reader).unwrap();
        assert!(outcome.diagnostics.iter().any(|d| d
            .message()
            .contains("machine names assigned to more than one")));
    }

    #[test]
    fn the_real_kernel_pool_agrees_with_itself() {
        let kernel_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf();
        let reader = crate::validation::mechanical_integrity::tests::real_fs_reader(kernel_root);
        let outcome = run(&reader).unwrap();
        assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    }
}
