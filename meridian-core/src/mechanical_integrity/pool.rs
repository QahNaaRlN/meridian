//! Shared ordered-set utilities used by every 7a family that compares two
//! hand-authored halves of a pool (a data list and a documentation table):
//! `instruction-topics`, `operating-foundation` and `stack-profiles`. Moved
//! here, unchanged, from three separate per-file copies in
//! `meridian-cli/src/commands/validate/{instruction_topics,
//! operating_foundation,stack_profiles}.rs`.

use std::collections::{HashMap, HashSet};

use crate::mechanical_integrity::fail;
use crate::types::Diagnostic;

/// A `Set`'s iteration order in the Node reference this package ports: each
/// item kept once, in the order it was first seen. Membership can be (and
/// is) checked through a separate `HashSet` built from this — that
/// structure's own iteration order is never used for anything a person or a
/// diagnostic reads.
pub fn dedupe_preserve_order(items: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in items {
        if seen.insert(item.clone()) {
            out.push(item);
        }
    }
    out
}

/// Mirrors the Node reference's
/// `ids.filter((id, index, all) => (require_truthy ? id : true) && all.indexOf(id) !== index)`:
/// every occurrence of an id beyond its first, in original order. When
/// `require_truthy` is set, an empty id is never reported as a duplicate of
/// itself.
pub fn duplicates_after_first(ids: &[String], require_truthy: bool) -> Vec<String> {
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

/// The two-halves-of-a-pool agreement check shared verbatim (message text
/// included) by `instruction-topics` and `stack-profiles`: `declared_ordered`
/// (the data half) and `documented_ordered` (the table-row half) must name
/// exactly the same set, in first-seen order for the diagnostic text.
/// Returns `None` when they agree.
pub fn ordered_pool_agreement(
    check_name: &str,
    declared_ordered: &[String],
    documented_ordered: &[String],
) -> Option<Diagnostic> {
    let documented_set: HashSet<&str> = documented_ordered.iter().map(|s| s.as_str()).collect();
    let declared_set: HashSet<&str> = declared_ordered.iter().map(|s| s.as_str()).collect();

    let undocumented: Vec<&String> = declared_ordered
        .iter()
        .filter(|n| !documented_set.contains(n.as_str()))
        .collect();
    let unlisted: Vec<&String> = documented_ordered
        .iter()
        .filter(|n| !declared_set.contains(n.as_str()))
        .collect();

    if undocumented.is_empty() && unlisted.is_empty() {
        return None;
    }

    let mut parts = Vec::new();
    if !undocumented.is_empty() {
        let names: Vec<&str> = undocumented.iter().map(|s| s.as_str()).collect();
        parts.push(format!(
            "named in the data but carrying no signature: {}",
            names.join(", ")
        ));
    }
    if !unlisted.is_empty() {
        let names: Vec<&str> = unlisted.iter().map(|s| s.as_str()).collect();
        parts.push(format!(
            "carrying a signature but absent from the data: {}",
            names.join(", ")
        ));
    }
    Some(fail(format!(
        "{check_name}: the two halves of the pool disagree — {}",
        parts.join("; ")
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedupe_preserve_order_keeps_first_seen_order() {
        let out = dedupe_preserve_order(["b", "a", "b", "c", "a"].into_iter().map(String::from));
        assert_eq!(out, vec!["b", "a", "c"]);
    }

    #[test]
    fn duplicates_after_first_reports_every_occurrence_past_the_first() {
        let ids = vec![
            "a".to_string(),
            "b".to_string(),
            "a".to_string(),
            "a".to_string(),
        ];
        assert_eq!(duplicates_after_first(&ids, false), vec!["a", "a"]);
    }

    #[test]
    fn duplicates_after_first_never_reports_an_empty_id_when_truthy_required() {
        let ids = vec!["".to_string(), "".to_string()];
        assert!(duplicates_after_first(&ids, true).is_empty());
    }

    #[test]
    fn ordered_pool_agreement_none_when_sets_match() {
        let a = vec!["x".to_string()];
        let b = vec!["x".to_string()];
        assert!(ordered_pool_agreement("check", &a, &b).is_none());
    }

    #[test]
    fn ordered_pool_agreement_reports_both_directions_in_declaration_order() {
        let declared = vec!["zeta".to_string(), "alpha".to_string()];
        let documented: Vec<String> = Vec::new();
        let d = ordered_pool_agreement("check", &declared, &documented).unwrap();
        let zeta_pos = d.message().find("zeta").unwrap();
        let alpha_pos = d.message().find("alpha").unwrap();
        assert!(zeta_pos < alpha_pos);
    }
}
