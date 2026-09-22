//! Pure half of the `operating-foundation` check (package 7, subpackage 7a):
//! `standards/workspace/operating-foundation.yaml` (machine identities) must
//! agree with `operating-glossary.md`/`operating-principles.md` (human
//! signatures) — same id set, same bilingual names, no duplicate id, no
//! machine name owned by two terms. Transport parsing (YAML, marked-region
//! table-row extraction) stays in
//! `meridian_app::validation::mechanical_integrity::operating_foundation`;
//! this module owns the predicate algorithms over the already-extracted
//! rows/entries.

use std::collections::HashMap;
use std::fmt;

use crate::mechanical_integrity::{dedupe_preserve_order, duplicates_after_first, fail};
use crate::types::Diagnostic;

/// A value rejected by [`FoundationEntry::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoundationEntryError {
    EmptyId,
}

impl fmt::Display for FoundationEntryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FoundationEntryError::EmptyId => write!(f, "id must not be empty"),
        }
    }
}

impl std::error::Error for FoundationEntryError {}

/// A value rejected by [`FoundationRow::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoundationRowError {
    EmptyId,
}

impl fmt::Display for FoundationRowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FoundationRowError::EmptyId => write!(f, "id must not be empty"),
        }
    }
}

impl std::error::Error for FoundationRowError {}

/// One row of a documentation table (`operating-term-pool` /
/// `operating-principle-pool`) — a valid domain value: private fields,
/// constructible only through [`FoundationRow::new`], which rejects an
/// empty `id`. `ru`/`en`/`body` are deliberately NOT rejected here
/// (corrective round, `meridian-cli-foundation-architecture-remediation`,
/// fourth round, item 2): a row's id/ru/en SIGNATURE must stay available to
/// [`check_pool`]'s pool-agreement and bilingual-mismatch comparisons even
/// when its `body` is blank — rejecting construction for an empty body
/// would make the row (and its id) vanish from those comparisons entirely,
/// producing a FALSE "halves disagree" the Node reference never reports.
/// [`rows_without_body`]/[`empty_body_row_ids`] independently, and only
/// additionally, flag a blank `body` — the row itself is still fully
/// checked. Node's own table-row regex requires at least one character for
/// `id`/`ru`/`en` (only `body`'s own capture group allows zero), so an
/// empty `ru`/`en` is not reachable from real extraction either way; `id`
/// is the one field whose absence makes the row unusable for identity
/// comparison at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundationRow {
    id: String,
    ru: String,
    en: String,
    body: String,
}

impl FoundationRow {
    pub fn new(
        id: impl Into<String>,
        ru: impl Into<String>,
        en: impl Into<String>,
        body: impl Into<String>,
    ) -> Result<Self, FoundationRowError> {
        let id = id.into();
        if id.is_empty() {
            return Err(FoundationRowError::EmptyId);
        }
        Ok(Self {
            id,
            ru: ru.into(),
            en: en.into(),
            body: body.into(),
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn ru(&self) -> &str {
        &self.ru
    }
    pub fn en(&self) -> &str {
        &self.en
    }
    pub fn body(&self) -> &str {
        &self.body
    }
}

/// One machine-identity entry (`terms[]` / `principles[]`) — a valid domain
/// value: private fields, constructible only through
/// [`FoundationEntry::new`], which rejects an empty `id`. `ru`/`en` are
/// deliberately NOT rejected here (corrective round,
/// `meridian-cli-foundation-architecture-remediation`, fourth round, item
/// 2): an entry's valid `id` must stay available to [`check_pool`]'s
/// duplicate and pool-membership checks even when `ru`/`en` is missing or
/// empty — rejecting construction for it would make the entry (and its id)
/// vanish from those comparisons entirely, producing a FALSE "halves
/// disagree" the Node reference never reports. A blank/missing `ru`/`en`
/// is instead reported through the EXISTING, Node-compatible bilingual
/// mismatch diagnostic wherever a documented row with a different (or any)
/// value exists — comparing `""` against real text is exactly the string
/// inequality the Node reference's own `row.ru !== entry.ru` already
/// performs; no new diagnostic is introduced for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundationEntry {
    id: String,
    ru: String,
    en: String,
    /// Only ever checked for a term pool (`machine_names_checked` in
    /// [`check_pool`]); an empty vector for a principle entry.
    machine_names: Vec<String>,
}

impl FoundationEntry {
    pub fn new(
        id: impl Into<String>,
        ru: impl Into<String>,
        en: impl Into<String>,
        machine_names: Vec<String>,
    ) -> Result<Self, FoundationEntryError> {
        let id = id.into();
        if id.is_empty() {
            return Err(FoundationEntryError::EmptyId);
        }
        Ok(Self {
            id,
            ru: ru.into(),
            en: en.into(),
            machine_names,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn ru(&self) -> &str {
        &self.ru
    }
    pub fn en(&self) -> &str {
        &self.en
    }
    pub fn machine_names(&self) -> &[String] {
        &self.machine_names
    }
}

/// Neither `operating-foundation.yaml`, `operating-glossary.md` nor
/// `operating-principles.md` could be read.
pub fn missing_pool_files(missing: &[&str]) -> Diagnostic {
    fail(format!(
        "operating-foundation: the canonical pool is incomplete ({}); machine identities and human signatures are one contract",
        missing.join(", ")
    ))
}

/// A marked region (`operating-term-pool` / `operating-principle-pool`)
/// could not be read.
pub fn region_unreadable(label: &str, error: &str) -> Diagnostic {
    fail(format!(
        "operating-foundation: the {label} region is not readable — {error}"
    ))
}

/// One or more documented rows carry an empty body column.
pub fn rows_without_body(label: &str, body_label: &str, ids: &[String]) -> Diagnostic {
    fail(format!(
        "operating-foundation: {label} rows without {body_label}: {}",
        ids.join(", ")
    ))
}

/// The ids of every successfully-constructed row whose `body` is empty —
/// [`rows_without_body`]'s trigger, computed from real [`FoundationRow`]
/// values (item 2: body emptiness never blocks construction, so this scans
/// the SAME rows [`check_pool`] itself compares, never a separate raw
/// pre-construction list).
pub fn empty_body_row_ids(rows: &[FoundationRow]) -> Vec<String> {
    rows.iter()
        .filter(|r| r.body.is_empty())
        .map(|r| r.id.clone())
        .collect()
}

/// One or more raw data entries for `label` carried no id at all — computed
/// by the app transport layer from the RAW, not-yet-validated entries
/// (before [`FoundationEntry::new`] ever ran), since a domain
/// [`FoundationEntry`] cannot represent this state at all (corrective round,
/// `meridian-cli-foundation-architecture-remediation`, item 1: "Incomplete
/// must be a constructor issue, not a domain enum variant" — the same
/// discipline applied here to an empty id). Previously a silently-dropped
/// case entirely (Node parity keeps such an entry out of the "undocumented"
/// comparison, matching its own `id &&` truthy guard, but gave it no
/// diagnostic of its own at all); the first corrective round's item 2
/// closed that: an incomplete data row is never permitted to pass through
/// unreported.
pub fn entry_missing_id(label: &str, count: usize) -> Diagnostic {
    fail(format!(
        "operating-foundation: {count} {label} entry(ies) in data have no id; an entry with no id cannot be checked against documentation and is not silently ignored"
    ))
}

/// The full `compare_pool` predicate: duplicate ids, machine-name
/// collisions (when `check_machine_names` is set), pool-halves agreement,
/// and per-id bilingual-name agreement. `entries` is `None` when the parsed
/// YAML value for this pool was not an array at all. Every entry received
/// here is already a successfully-constructed [`FoundationEntry`] — an
/// empty-id raw entry never reaches this function at all; the caller
/// resolves the raw list and reports [`entry_missing_id`] itself before
/// calling this (corrective round, item 1).
pub fn check_pool(
    entries: Option<&[FoundationEntry]>,
    rows: &[FoundationRow],
    label: &str,
    check_machine_names: bool,
) -> Vec<Diagnostic> {
    let mut failures = Vec::new();

    let Some(entries) = entries else {
        failures.push(fail(format!(
            "operating-foundation: {label} is not an array"
        )));
        return failures;
    };

    let entry_ids: Vec<String> = entries.iter().map(|e| e.id.clone()).collect();
    let row_ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();

    let duplicate_data = dedupe_preserve_order(duplicates_after_first(&entry_ids, true));
    let duplicate_docs = dedupe_preserve_order(duplicates_after_first(&row_ids, false));
    if !duplicate_data.is_empty() {
        failures.push(fail(format!(
            "operating-foundation: duplicate {label} id in data: {}",
            duplicate_data.join(", ")
        )));
    }
    if !duplicate_docs.is_empty() {
        failures.push(fail(format!(
            "operating-foundation: duplicate {label} id in documentation: {}",
            duplicate_docs.join(", ")
        )));
    }

    if check_machine_names {
        let mut owners: HashMap<&str, &str> = HashMap::new();
        let mut collisions_seen = std::collections::HashSet::new();
        let mut collisions_ordered = Vec::new();
        for entry in entries {
            for name in &entry.machine_names {
                if let Some(&prior) = owners.get(name.as_str()) {
                    if prior != entry.id && collisions_seen.insert(name.clone()) {
                        collisions_ordered.push(name.clone());
                    }
                } else {
                    owners.insert(name.as_str(), entry.id.as_str());
                }
            }
        }
        if !collisions_ordered.is_empty() {
            failures.push(fail(format!(
                "operating-foundation: machine names assigned to more than one {label}: {}",
                collisions_ordered.join(", ")
            )));
        }
    }

    let data_order = dedupe_preserve_order(entry_ids.iter().cloned());
    let mut data_map: HashMap<&str, &FoundationEntry> = HashMap::new();
    for (id, entry) in entry_ids.iter().zip(entries.iter()) {
        data_map.insert(id.as_str(), entry);
    }
    let docs_order = dedupe_preserve_order(row_ids.iter().cloned());
    let mut docs_map: HashMap<&str, &FoundationRow> = HashMap::new();
    for row in rows {
        docs_map.insert(row.id.as_str(), row);
    }

    let undocumented: Vec<&String> = data_order
        .iter()
        .filter(|id| !docs_map.contains_key(id.as_str()))
        .collect();
    let unlisted: Vec<&String> = docs_order
        .iter()
        .filter(|id| !data_map.contains_key(id.as_str()))
        .collect();

    if !undocumented.is_empty() || !unlisted.is_empty() {
        let mut parts = Vec::new();
        if !undocumented.is_empty() {
            parts.push(format!(
                "named in data but absent from documentation: {}",
                undocumented
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !unlisted.is_empty() {
            parts.push(format!(
                "documented but absent from data: {}",
                unlisted
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        failures.push(fail(format!(
            "operating-foundation: {label} halves disagree — {}",
            parts.join("; ")
        )));
    }

    for id in &data_order {
        let Some(entry) = data_map.get(id.as_str()) else {
            continue;
        };
        let Some(row) = docs_map.get(id.as_str()) else {
            continue;
        };
        if row.ru != entry.ru || row.en != entry.en {
            failures.push(fail(format!(
                "operating-foundation: {label} \"{id}\" has different bilingual names in data and documentation"
            )));
        }
    }

    failures
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, ru: &str, en: &str) -> FoundationEntry {
        FoundationEntry::new(id, ru, en, Vec::new()).unwrap()
    }

    fn row(id: &str, ru: &str, en: &str, body: &str) -> FoundationRow {
        FoundationRow::new(id, ru, en, body).unwrap()
    }

    #[test]
    fn agreeing_pool_produces_no_failures() {
        let entries = vec![entry("widget", "Виджет", "Widget")];
        let rows = vec![row("widget", "Виджет", "Widget", "def")];
        assert!(check_pool(Some(&entries), &rows, "term", false).is_empty());
    }

    #[test]
    fn a_non_array_pool_is_a_single_failure() {
        let rows: Vec<FoundationRow> = Vec::new();
        let failures = check_pool(None, &rows, "term", false);
        assert_eq!(failures.len(), 1);
        assert!(failures[0].message().contains("is not an array"));
    }

    #[test]
    fn foundation_entry_rejects_empty_id() {
        assert_eq!(
            FoundationEntry::new("", "ru", "en", Vec::new()).unwrap_err(),
            FoundationEntryError::EmptyId
        );
        assert!(FoundationEntry::new("id", "ru", "en", Vec::new()).is_ok());
    }

    #[test]
    fn foundation_entry_accepts_an_empty_ru_or_en_the_id_signature_must_survive() {
        // Item 2 of the fourth corrective round: only `id` is rejected —
        // `ru`/`en` stay usable (even blank) so the entry's id remains
        // available to duplicate/pool-membership checks; a blank/mismatched
        // bilingual name surfaces through the EXISTING mismatch check
        // instead (see `an_empty_entry_bilingual_name_surfaces_as_the_existing_mismatch_not_a_new_diagnostic`).
        assert!(FoundationEntry::new("id", "", "en", Vec::new()).is_ok());
        assert!(FoundationEntry::new("id", "ru", "", Vec::new()).is_ok());
    }

    #[test]
    fn foundation_row_rejects_empty_id_only() {
        assert_eq!(
            FoundationRow::new("", "ru", "en", "body").unwrap_err(),
            FoundationRowError::EmptyId
        );
        assert!(FoundationRow::new("id", "ru", "en", "body").is_ok());
    }

    #[test]
    fn foundation_row_accepts_an_empty_ru_en_or_body_the_id_signature_must_survive() {
        // Item 2: a blank `body` (or, defensively, `ru`/`en`) must not make
        // the row vanish from `check_pool`'s comparisons — only `id` gates
        // construction.
        assert!(FoundationRow::new("id", "", "en", "body").is_ok());
        assert!(FoundationRow::new("id", "ru", "", "body").is_ok());
        assert!(FoundationRow::new("id", "ru", "en", "").is_ok());
    }

    #[test]
    fn entry_missing_id_names_the_label_and_count() {
        let d = entry_missing_id("term", 2);
        assert!(d.message().contains("2 term entry(ies)"));
        assert!(d.message().contains("have no id"));
    }

    #[test]
    fn bilingual_mismatch_is_reported() {
        let entries = vec![entry("widget", "Виджет", "Widget")];
        let rows = vec![row("widget", "Виджет", "Different", "def")];
        let failures = check_pool(Some(&entries), &rows, "term", false);
        assert!(failures
            .iter()
            .any(|f| f.message().contains("different bilingual names")));
    }

    #[test]
    fn an_empty_entry_bilingual_name_surfaces_as_the_existing_mismatch_not_a_new_diagnostic() {
        // Item 2, preferred resolution: a missing/empty RU with a matching
        // documented id must not make the id appear absent, and must not
        // introduce a new diagnostic — it must map to the SAME
        // Node-compatible "different bilingual names" mismatch, since ""
        // != the row's real text.
        let entries = vec![entry("widget", "", "Widget")];
        let rows = vec![row("widget", "Виджет", "Widget", "def")];
        let failures = check_pool(Some(&entries), &rows, "term", false);
        assert_eq!(failures.len(), 1, "{failures:?}");
        assert!(failures[0].message().contains("different bilingual names"));
        assert!(!failures[0].message().contains("disagree"));
    }

    #[test]
    fn a_documented_row_with_a_blank_body_still_participates_in_pool_agreement() {
        // Item 2, observable parity: a blank documented body must produce
        // ONLY `rows_without_body` — never a false "halves disagree" —
        // because the row's id/ru/en signature stays available.
        let entries = vec![entry("widget", "Виджет", "Widget")];
        let rows = vec![row("widget", "Виджет", "Widget", "")];
        let failures = check_pool(Some(&entries), &rows, "term", false);
        assert!(
            failures.is_empty(),
            "a blank body alone must not fail check_pool: {failures:?}"
        );
        assert_eq!(empty_body_row_ids(&rows), vec!["widget".to_string()]);
    }

    #[test]
    fn machine_name_collision_is_detected_only_when_checked() {
        let entries = vec![
            FoundationEntry::new("a", "А", "A", vec!["shared".into()]).unwrap(),
            FoundationEntry::new("b", "Б", "B", vec!["shared".into()]).unwrap(),
        ];
        let rows = vec![row("a", "А", "A", "d"), row("b", "Б", "B", "d")];
        let failures = check_pool(Some(&entries), &rows, "term", true);
        assert!(failures
            .iter()
            .any(|f| f.message().contains("machine names assigned")));
        let failures_unchecked = check_pool(Some(&entries), &rows, "term", false);
        assert!(!failures_unchecked
            .iter()
            .any(|f| f.message().contains("machine names assigned")));
    }

    #[test]
    fn empty_body_rows_are_reported() {
        // A row with an empty body constructs successfully (item 2) — its
        // id is still collected by `empty_body_row_ids` for the
        // `rows_without_body` diagnostic.
        let rows = vec![row("a", "А", "A", "")];
        let ids = empty_body_row_ids(&rows);
        assert_eq!(ids, vec!["a".to_string()]);
        let d = rows_without_body("term-pool", "a definition", &ids);
        assert!(d.message().contains("rows without a definition"));
        assert!(d.message().contains("a"));
    }

    #[test]
    fn disagreement_lists_ids_in_declaration_order() {
        let entries = vec![entry("zeta", "З", "Z"), entry("alpha", "А", "A")];
        let rows: Vec<FoundationRow> = Vec::new();
        let failures = check_pool(Some(&entries), &rows, "term", false);
        let d = failures
            .iter()
            .find(|f| f.message().contains("disagree"))
            .unwrap();
        let zeta_pos = d.message().find("zeta").unwrap();
        let alpha_pos = d.message().find("alpha").unwrap();
        assert!(zeta_pos < alpha_pos);
    }
}
