//! Port of `scripts/kernel-validate.mjs`'s `operating-foundation` check
//! (`standards/workspace/operating-foundation.yaml` +
//! `standards/workspace/operating-glossary.md` +
//! `standards/workspace/operating-principles.md`). The data supplies stable
//! identities and bilingual names. The documents supply definitions and
//! normative consequences. Neither half is accepted on its own: a
//! machine-only term is not understandable to the owner, while a
//! prose-only term cannot be referenced deterministically by a schema or
//! run.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use fancy_regex::Regex;
use meridian_app::source_format::{parse_yaml, regions::marked_region};
use serde_json::Value;

pub struct Outcome {
    pub failures: Vec<String>,
}

struct Row {
    id: String,
    ru: String,
    en: String,
    body: String,
}

pub fn run(kernel_root: &Path) -> Outcome {
    let mut failures = Vec::new();

    let yaml_raw =
        fs::read_to_string(kernel_root.join("standards/workspace/operating-foundation.yaml")).ok();
    let glossary_raw =
        fs::read_to_string(kernel_root.join("standards/workspace/operating-glossary.md")).ok();
    let principles_raw =
        fs::read_to_string(kernel_root.join("standards/workspace/operating-principles.md")).ok();

    if yaml_raw.is_none() || glossary_raw.is_none() || principles_raw.is_none() {
        let mut missing = Vec::new();
        if yaml_raw.is_none() {
            missing.push("standards/workspace/operating-foundation.yaml");
        }
        if glossary_raw.is_none() {
            missing.push("standards/workspace/operating-glossary.md");
        }
        if principles_raw.is_none() {
            missing.push("standards/workspace/operating-principles.md");
        }
        failures.push(format!(
            "operating-foundation: the canonical pool is incomplete ({}); machine identities and human signatures are one contract",
            missing.join(", ")
        ));
        return Outcome { failures };
    }
    let yaml_raw = yaml_raw.unwrap();
    let glossary_raw = glossary_raw.unwrap();
    let principles_raw = principles_raw.unwrap();

    let foundation = match parse_yaml(&yaml_raw) {
        Ok(value) => Some(value),
        Err(error) => {
            failures.push(format!("operating-foundation: {error}"));
            None
        }
    };

    let read_rows = |raw: &str,
                     region_id: &str,
                     label: &str,
                     body_label: &str,
                     failures: &mut Vec<String>|
     -> Vec<Row> {
        let region = match marked_region(raw, region_id) {
            Ok(region) => region,
            Err(error) => {
                failures.push(format!(
                    "operating-foundation: the {label} region is not readable — {error}"
                ));
                return Vec::new();
            }
        };
        let row_re = Regex::new(
            r"(?m)^\|\s*`([a-z][a-z0-9]*(?:-[a-z0-9]+)*)`\s*\|\s*([^|]+?)\s*\|\s*([^|]+?)\s*\|\s*([^|]*?)\s*\|",
        )
        .expect("row pattern compiles");
        let rows: Vec<Row> = row_re
            .captures_iter(&region.text)
            .filter_map(|c| c.ok())
            .map(|c| Row {
                id: c.get(1).map(|g| g.as_str().to_string()).unwrap_or_default(),
                ru: c
                    .get(2)
                    .map(|g| g.as_str().trim().to_string())
                    .unwrap_or_default(),
                en: c
                    .get(3)
                    .map(|g| g.as_str().trim().to_string())
                    .unwrap_or_default(),
                body: c
                    .get(4)
                    .map(|g| g.as_str().trim().to_string())
                    .unwrap_or_default(),
            })
            .collect();
        let empty_bodies: Vec<&str> = rows
            .iter()
            .filter(|r| r.body.is_empty())
            .map(|r| r.id.as_str())
            .collect();
        if !empty_bodies.is_empty() {
            failures.push(format!(
                "operating-foundation: {label} rows without {body_label}: {}",
                empty_bodies.join(", ")
            ));
        }
        rows
    };

    let compare_pool = |entries: Option<&Vec<Value>>,
                        rows: &[Row],
                        label: &str,
                        ru_field: &str,
                        en_field: &str,
                        machine_names_field: Option<&str>,
                        failures: &mut Vec<String>| {
        let Some(entries) = entries else {
            failures.push(format!("operating-foundation: {label} is not an array"));
            return;
        };

        // Node computes duplicates and disagreement diagnostics by reading
        // arrays and `Map`/`Set` objects, both of which iterate in
        // first-insertion order. Reading the equivalent lists here out of a
        // `HashMap`'s own iteration order — never sorted, and randomized
        // per process by Rust's `RandomState` — would make the *order* of
        // these diagnostic messages non-deterministic from run to run, not
        // merely different from Node's order. Every list below is built and
        // read as an ordered `Vec`; a `HashMap`/`HashSet` is used only for
        // O(1) membership tests, never iterated for its own order.
        let entry_ids: Vec<String> = entries
            .iter()
            .map(|e| {
                e.get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string()
            })
            .collect();
        let row_ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();

        let duplicate_data = dedupe_preserve_order(duplicates_after_first(&entry_ids, true));
        let duplicate_docs = dedupe_preserve_order(duplicates_after_first(&row_ids, false));
        if !duplicate_data.is_empty() {
            failures.push(format!(
                "operating-foundation: duplicate {label} id in data: {}",
                duplicate_data.join(", ")
            ));
        }
        if !duplicate_docs.is_empty() {
            failures.push(format!(
                "operating-foundation: duplicate {label} id in documentation: {}",
                duplicate_docs.join(", ")
            ));
        }

        if let Some(machine_names_field) = machine_names_field {
            let mut owners: HashMap<String, &str> = HashMap::new();
            let mut collisions_seen = std::collections::HashSet::new();
            let mut collisions_ordered = Vec::new();
            for entry in entries {
                let id = entry.get("id").and_then(Value::as_str).unwrap_or("");
                let names = entry
                    .get(machine_names_field)
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                for name in &names {
                    let Some(name) = name.as_str() else { continue };
                    if let Some(&prior) = owners.get(name) {
                        if prior != id && collisions_seen.insert(name.to_string()) {
                            collisions_ordered.push(name.to_string());
                        }
                    } else {
                        owners.insert(name.to_string(), id);
                    }
                }
            }
            if !collisions_ordered.is_empty() {
                failures.push(format!(
                    "operating-foundation: machine names assigned to more than one {label}: {}",
                    collisions_ordered.join(", ")
                ));
            }
        }

        // First-seen order for both id lists, then a lookup map per id
        // (last entry wins on a duplicate id, same as `new Map(pairs)`
        // re-setting an existing key updates its value without moving its
        // position).
        let data_order = dedupe_preserve_order(entry_ids.iter().cloned());
        let mut data_map: HashMap<&str, &Value> = HashMap::new();
        for (id, entry) in entry_ids.iter().zip(entries.iter()) {
            data_map.insert(id.as_str(), entry);
        }
        let docs_order = dedupe_preserve_order(row_ids.iter().cloned());
        let mut docs_map: HashMap<&str, &Row> = HashMap::new();
        for row in rows {
            docs_map.insert(row.id.as_str(), row);
        }

        let undocumented: Vec<&String> = data_order
            .iter()
            .filter(|id| !id.is_empty() && !docs_map.contains_key(id.as_str()))
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
            failures.push(format!(
                "operating-foundation: {label} halves disagree — {}",
                parts.join("; ")
            ));
        }

        for id in &data_order {
            let Some(entry) = data_map.get(id.as_str()) else {
                continue;
            };
            let Some(row) = docs_map.get(id.as_str()) else {
                continue;
            };
            let expected_ru = entry.get(ru_field).and_then(Value::as_str).unwrap_or("");
            let expected_en = entry.get(en_field).and_then(Value::as_str).unwrap_or("");
            if row.ru != expected_ru || row.en != expected_en {
                failures.push(format!(
                    "operating-foundation: {label} \"{id}\" has different bilingual names in data and documentation"
                ));
            }
        }
    };

    if let Some(foundation) = foundation {
        let term_rows = read_rows(
            &glossary_raw,
            "operating-term-pool",
            "term-pool",
            "a definition",
            &mut failures,
        );
        let principle_rows = read_rows(
            &principles_raw,
            "operating-principle-pool",
            "principle-pool",
            "a mandatory consequence",
            &mut failures,
        );
        let terms = foundation.get("terms").and_then(Value::as_array);
        let principles = foundation.get("principles").and_then(Value::as_array);
        compare_pool(
            terms,
            &term_rows,
            "term",
            "canonical_ru",
            "canonical_en",
            Some("machine_names"),
            &mut failures,
        );
        compare_pool(
            principles,
            &principle_rows,
            "principle",
            "title_ru",
            "title_en",
            None,
            &mut failures,
        );
    }

    Outcome { failures }
}

/// Mirrors the Node reference's
/// `ids.filter((id, index, all) => (require_truthy ? id : true) && all.indexOf(id) !== index)`:
/// every occurrence of an id beyond its first, in original order. When
/// `require_truthy` is set, an empty id is never reported as a duplicate of
/// itself (matching the data-side check, which guards on `id &&`); the
/// documentation-side check carries no such guard because a captured row id
/// is never empty.
fn duplicates_after_first(ids: &[String], require_truthy: bool) -> Vec<String> {
    let mut first_index: HashMap<&str, usize> = HashMap::new();
    for (i, id) in ids.iter().enumerate() {
        first_index.entry(id.as_str()).or_insert(i);
    }
    ids.iter()
        .enumerate()
        .filter(|(i, id)| {
            if require_truthy && id.is_empty() {
                return false;
            }
            first_index[id.as_str()] != *i
        })
        .map(|(_, id)| id.clone())
        .collect()
}

/// A `Set`'s iteration order in the Node reference: each item kept once, in
/// the order it was first seen.
fn dedupe_preserve_order(items: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for item in items {
        if seen.insert(item.clone()) {
            out.push(item);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, rel: &str, content: &str) {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    /// RAII guard: removes its exact temp directory on `Drop`, including
    /// while unwinding a panicking assertion, so a failing test never
    /// leaves its scratch tree behind. The name mixes the PID with a
    /// nanosecond timestamp — the PID alone is a shared, predictable
    /// directory that a second run (or a parallel `cargo test` thread) can
    /// collide on; the timestamp makes each call's directory unique.
    struct TestDir(std::path::PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "operating-foundation-{label}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            TestDir(dir)
        }
    }

    impl std::ops::Deref for TestDir {
        type Target = Path;
        fn deref(&self) -> &Path {
            &self.0
        }
    }

    impl AsRef<Path> for TestDir {
        fn as_ref(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn temp(name: &str) -> TestDir {
        TestDir::new(name)
    }

    fn write_agreeing_pool(dir: &Path) {
        write(
            dir,
            "standards/workspace/operating-foundation.yaml",
            "terms:\n  - id: widget\n    canonical_ru: Виджет\n    canonical_en: Widget\n    machine_names: [widget]\nprinciples:\n  - id: safety-first\n    title_ru: Сначала безопасность\n    title_en: Safety first\n",
        );
        write(
            dir,
            "standards/workspace/operating-glossary.md",
            "<!-- meridian:begin operating-term-pool -->\n| Id | RU | EN | Def |\n|---|---|---|---|\n| `widget` | Виджет | Widget | A thing |\n<!-- meridian:end operating-term-pool -->\n",
        );
        write(
            dir,
            "standards/workspace/operating-principles.md",
            "<!-- meridian:begin operating-principle-pool -->\n| Id | RU | EN | Consequence |\n|---|---|---|---|\n| `safety-first` | Сначала безопасность | Safety first | Do not break things |\n<!-- meridian:end operating-principle-pool -->\n",
        );
    }

    #[test]
    fn agreeing_pools_produce_no_failures() {
        let dir = temp("agree");
        write_agreeing_pool(&dir);
        let outcome = run(&dir);
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    }

    #[test]
    fn a_term_documented_with_a_different_bilingual_name_is_a_failure() {
        let dir = temp("bilingual-mismatch");
        write_agreeing_pool(&dir);
        write(
            &dir,
            "standards/workspace/operating-glossary.md",
            "<!-- meridian:begin operating-term-pool -->\n| Id | RU | EN | Def |\n|---|---|---|---|\n| `widget` | Виджет | Different English | A thing |\n<!-- meridian:end operating-term-pool -->\n",
        );
        let outcome = run(&dir);
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].contains("different bilingual names"));
    }

    #[test]
    fn a_term_named_only_in_data_is_a_failure() {
        let dir = temp("undocumented-term");
        write_agreeing_pool(&dir);
        write(
            &dir,
            "standards/workspace/operating-foundation.yaml",
            "terms:\n  - id: widget\n    canonical_ru: Виджет\n    canonical_en: Widget\n  - id: ghost\n    canonical_ru: Призрак\n    canonical_en: Ghost\nprinciples:\n  - id: safety-first\n    title_ru: Сначала безопасность\n    title_en: Safety first\n",
        );
        let outcome = run(&dir);
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].contains("absent from documentation"));
    }

    #[test]
    fn disagreement_diagnostics_list_ids_in_declaration_order_not_alphabetically() {
        let dir = temp("insertion-order");
        write(
            &dir,
            "standards/workspace/operating-foundation.yaml",
            "terms:\n  - id: zeta-term\n    canonical_ru: З\n    canonical_en: Z\n  - id: alpha-term\n    canonical_ru: А\n    canonical_en: A\nprinciples: []\n",
        );
        write(
            &dir,
            "standards/workspace/operating-glossary.md",
            "<!-- meridian:begin operating-term-pool -->\n| Id | RU | EN | Def |\n|---|---|---|---|\n<!-- meridian:end operating-term-pool -->\n",
        );
        write(
            &dir,
            "standards/workspace/operating-principles.md",
            "<!-- meridian:begin operating-principle-pool -->\n<!-- meridian:end operating-principle-pool -->\n",
        );
        let outcome = run(&dir);
        assert_eq!(outcome.failures.len(), 1);
        let zeta_pos = outcome.failures[0].find("zeta-term").unwrap();
        let alpha_pos = outcome.failures[0].find("alpha-term").unwrap();
        assert!(
            zeta_pos < alpha_pos,
            "expected declaration order (zeta before alpha), got: {}",
            outcome.failures[0]
        );
    }

    #[test]
    fn a_duplicated_id_in_data_is_reported_once_in_first_seen_order() {
        let dir = temp("duplicate-data");
        write(
            &dir,
            "standards/workspace/operating-foundation.yaml",
            "terms:\n  - id: zeta-term\n    canonical_ru: З\n    canonical_en: Z\n  - id: alpha-term\n    canonical_ru: А\n    canonical_en: A\n  - id: zeta-term\n    canonical_ru: З\n    canonical_en: Z\nprinciples: []\n",
        );
        write(
            &dir,
            "standards/workspace/operating-glossary.md",
            "<!-- meridian:begin operating-term-pool -->\n| Id | RU | EN | Def |\n|---|---|---|---|\n| `zeta-term` | З | Z | d |\n| `alpha-term` | А | A | d |\n<!-- meridian:end operating-term-pool -->\n",
        );
        write(
            &dir,
            "standards/workspace/operating-principles.md",
            "<!-- meridian:begin operating-principle-pool -->\n<!-- meridian:end operating-principle-pool -->\n",
        );
        let outcome = run(&dir);
        assert_eq!(
            outcome
                .failures
                .iter()
                .filter(|f| f.contains("duplicate term id in data"))
                .count(),
            1
        );
        assert!(
            outcome
                .failures
                .iter()
                .any(|f| f.contains("duplicate term id in data: zeta-term")),
            "{:?}",
            outcome.failures
        );
    }

    #[test]
    fn bilingual_mismatch_reports_are_deterministic_across_many_runs() {
        // Regression for the family of bugs this package's CHANGES_REQUESTED
        // round named "operating-foundation must not depend on random
        // HashMap order": run the same disagreeing pool enough times that a
        // genuinely-random iteration order would show up as a flaky ordering
        // of the two failure messages sooner or later.
        let dir = temp("deterministic-bilingual");
        write(
            &dir,
            "standards/workspace/operating-foundation.yaml",
            "terms:\n  - id: alpha-term\n    canonical_ru: А\n    canonical_en: A\n  - id: beta-term\n    canonical_ru: Б\n    canonical_en: B\nprinciples: []\n",
        );
        write(
            &dir,
            "standards/workspace/operating-glossary.md",
            "<!-- meridian:begin operating-term-pool -->\n| Id | RU | EN | Def |\n|---|---|---|---|\n| `alpha-term` | А | Wrong A | d |\n| `beta-term` | Б | Wrong B | d |\n<!-- meridian:end operating-term-pool -->\n",
        );
        write(
            &dir,
            "standards/workspace/operating-principles.md",
            "<!-- meridian:begin operating-principle-pool -->\n<!-- meridian:end operating-principle-pool -->\n",
        );
        let first = run(&dir).failures;
        for _ in 0..20 {
            assert_eq!(
                run(&dir).failures,
                first,
                "diagnostic order changed run to run"
            );
        }
    }

    #[test]
    fn a_missing_pool_file_fails_closed() {
        let dir = temp("missing");
        fs::create_dir_all(&dir).unwrap();
        let outcome = run(&dir);
        assert_eq!(outcome.failures.len(), 1);
        assert!(outcome.failures[0].contains("is incomplete"));
    }

    #[test]
    fn a_machine_name_assigned_to_two_terms_is_a_failure() {
        let dir = temp("machine-name-collision");
        write(
            &dir,
            "standards/workspace/operating-foundation.yaml",
            "terms:\n  - id: widget\n    canonical_ru: Виджет\n    canonical_en: Widget\n    machine_names: [shared]\n  - id: gadget\n    canonical_ru: Гаджет\n    canonical_en: Gadget\n    machine_names: [shared]\nprinciples: []\n",
        );
        write(
            &dir,
            "standards/workspace/operating-glossary.md",
            "<!-- meridian:begin operating-term-pool -->\n| Id | RU | EN | Def |\n|---|---|---|---|\n| `widget` | Виджет | Widget | A thing |\n| `gadget` | Гаджет | Gadget | Another thing |\n<!-- meridian:end operating-term-pool -->\n",
        );
        write(
            &dir,
            "standards/workspace/operating-principles.md",
            "<!-- meridian:begin operating-principle-pool -->\n<!-- meridian:end operating-principle-pool -->\n",
        );
        let outcome = run(&dir);
        assert!(
            outcome
                .failures
                .iter()
                .any(|f| f.contains("machine names assigned to more than one")),
            "{:?}",
            outcome.failures
        );
    }

    #[test]
    fn the_real_kernel_pool_agrees_with_itself() {
        let kernel_root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let outcome = run(kernel_root);
        assert!(outcome.failures.is_empty(), "{:?}", outcome.failures);
    }
}
