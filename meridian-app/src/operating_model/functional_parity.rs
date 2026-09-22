//! Business-contract migration of `functionalParityConsistency` (`scripts/kernel-validate.mjs`,
//! the `functional-parity` check, subpackage 7b). `verification/functional-parity/`
//! carries a portable functional-parity evidence contract, its JSON Schema
//! and product-neutral fixtures. The inference rules the Draft 7 subset
//! cannot express because they relate one part of a record to another are
//! enforced here:
//!    1. the document carries at least one record;
//!    2. every preserved-contract assertion id is unique across the whole
//!       record, the four facets included, and maps to exactly one owning
//!       facet;
//!    3. every per-assertion verdict names a declared assertion, and every
//!       declared assertion carries exactly one per-assertion verdict;
//!    4. every evidence `covers` id resolves to a declared assertion;
//!    5. every post-change `contract_links` entry resolves to the exact
//!       {facet, assertion_id} pair of a declared assertion; a right id
//!       under the wrong facet is rejected;
//!    6. a per-assertion VERIFIED needs BOTH a covering evidence entry AND
//!       a post-change contract link that resolves to it; `covers` alone
//!       is not enough (an empty `contract_links` array is legal only when
//!       nothing is VERIFIED);
//!    7. any UNVERIFIED per-assertion state forces an UNVERIFIED overall;
//!    8. a record-scoped gap forces every declared per-assertion verdict
//!       UNVERIFIED and the overall UNVERIFIED; an assertion-scoped gap
//!       forces only the assertions it names, leaving the rest free to
//!       stand;
//!    9. `relationship: same` means the post-change observation referenced
//!       every baseline condition id and no others, with no duplicate
//!       baseline condition id; the word in a description is not evidence
//!       of it;
//!   10. a baseline whose provenance is not established forces every
//!       declared per-assertion verdict UNVERIFIED and the overall
//!       UNVERIFIED, whatever gaps are recorded;
//!   11. the baseline and post-change source states are a distinguishable
//!       pair; the same {identifier_kind, identifier} for both is not a
//!       before/after comparison.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

const FACETS: [&str; 4] = [
    "public_api",
    "observable_io",
    "side_effects_and_interactions",
    "user_visible_behavior",
];

fn s(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

fn arr(v: Option<&Value>) -> &[Value] {
    static EMPTY: Vec<Value> = Vec::new();
    v.and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&EMPTY)
}

/// Port of `functionalParityConsistency`.
pub fn functional_parity_consistency(rec: &Value) -> Vec<String> {
    let mut problems = Vec::new();
    let empty = Vec::new();
    let records = rec
        .get("records")
        .and_then(Value::as_array)
        .unwrap_or(&empty);

    if records.is_empty() {
        problems.push("the evidence document carries no records; a functional-parity document with no record proves nothing".to_string());
    }

    for (ri, r) in records.iter().enumerate() {
        let at = if records.len() > 1 {
            format!(" (record {ri})")
        } else {
            String::new()
        };
        let empty_obj = Value::Object(Default::default());
        let pc = r.get("preserved_contract").unwrap_or(&empty_obj);

        // `catalog`/`declared` mirror the Node reference's `Map`/`Set`,
        // which iterate in first-insertion order — the order assertions
        // are actually declared across the four facets, not an incidental
        // one. A plain `HashMap`/`HashSet` would iterate in an arbitrary,
        // hash-dependent order instead, and when several declared
        // assertions simultaneously trigger a diagnostic below, that would
        // report them in a different order than the Node reference.
        // `declared_order` reproduces the insertion order for every loop
        // that iterates `declared` below; `declared` itself remains for
        // O(1) membership checks.
        let mut catalog: HashMap<String, String> = HashMap::new();
        let mut declared_order: Vec<String> = Vec::new();
        for fn_name in FACETS {
            let assertions = arr(pc.get(fn_name).and_then(|v| v.get("assertions")));
            for a in assertions {
                let id = s(a.get("id"));
                if id.is_empty() {
                    continue;
                }
                if let Some(owner) = catalog.get(&id) {
                    problems.push(format!(
                        "assertion id \"{id}\"{at} is declared more than once (facets \"{owner}\" and \"{fn_name}\"); ids are unique across the whole record"
                    ));
                } else {
                    catalog.insert(id.clone(), fn_name.to_string());
                    declared_order.push(id);
                }
            }
        }
        let declared: HashSet<String> = declared_order.iter().cloned().collect();
        if declared.is_empty() {
            problems.push(format!(
                "no preserved-contract assertion is declared{at}; a parity record with no assertion proves nothing"
            ));
        }

        let verdict = r.get("verdict").unwrap_or(&empty_obj);
        let per_assertion = arr(verdict.get("per_assertion"));
        let mut seen: HashMap<String, usize> = HashMap::new();
        for v in per_assertion {
            let id = s(v.get("assertion_id"));
            *seen.entry(id.clone()).or_insert(0) += 1;
            if !declared.contains(&id) {
                problems.push(format!(
                    "the verdict names assertion \"{id}\"{at}, which no preserved-contract facet declares"
                ));
            }
        }
        for id in &declared_order {
            let n = seen.get(id).copied().unwrap_or(0);
            if n == 0 {
                problems.push(format!(
                    "assertion \"{id}\"{at} carries no per-assertion verdict"
                ));
            } else if n > 1 {
                problems.push(format!(
                    "assertion \"{id}\"{at} carries more than one per-assertion verdict"
                ));
            }
        }
        let state_of: HashMap<String, String> = per_assertion
            .iter()
            .map(|v| (s(v.get("assertion_id")), s(v.get("state"))))
            .collect();

        let mut covered: HashSet<String> = HashSet::new();
        for ev in arr(r.get("evidence")) {
            for id in arr(ev.get("covers")) {
                let id_s = s(Some(id));
                if !declared.contains(&id_s) {
                    problems.push(format!(
                        "an evidence entry covers assertion \"{id_s}\"{at}, which no preserved-contract facet declares"
                    ));
                } else {
                    covered.insert(id_s);
                }
            }
        }

        let mut linked: HashSet<String> = HashSet::new();
        let post_change_evidence = r.get("post_change_evidence").unwrap_or(&empty_obj);
        for link in arr(post_change_evidence.get("contract_links")) {
            let link_id = s(link.get("assertion_id"));
            let facet = s(link.get("facet"));
            if !declared.contains(&link_id) {
                problems.push(format!(
                    "a post-change contract link names assertion \"{link_id}\"{at}, which no preserved-contract facet declares"
                ));
            } else if catalog.get(&link_id) != Some(&facet) {
                let owner = catalog.get(&link_id).cloned().unwrap_or_default();
                problems.push(format!(
                    "a post-change contract link names assertion \"{link_id}\"{at} under facet \"{facet}\", but it is declared under facet \"{owner}\""
                ));
            } else {
                linked.insert(link_id);
            }
        }

        for id in &declared_order {
            if state_of.get(id).map(String::as_str) != Some("VERIFIED") {
                continue;
            }
            if !covered.contains(id) {
                problems.push(format!(
                    "assertion \"{id}\"{at} is VERIFIED but no evidence entry covers it"
                ));
            }
            if !linked.contains(id) {
                problems.push(format!(
                    "assertion \"{id}\"{at} is VERIFIED but no post-change contract link resolves to it; evidence coverage alone is not sufficient"
                ));
            }
        }

        let any_unverified = state_of.values().any(|v| v == "UNVERIFIED");
        let overall = s(verdict.get("overall"));
        if any_unverified && overall != "UNVERIFIED" {
            problems.push(format!(
                "a per-assertion verdict is UNVERIFIED{at} but the overall verdict is not"
            ));
        }

        for g in arr(r.get("gaps")) {
            let scope = s(g.get("scope"));
            if scope == "record" {
                for id in &declared_order {
                    if state_of.get(id).map(String::as_str) == Some("VERIFIED") {
                        problems.push(format!(
                            "a record-scoped gap is recorded{at} but assertion \"{id}\" is VERIFIED; a record-scoped gap leaves every assertion UNVERIFIED"
                        ));
                    }
                }
                if overall != "UNVERIFIED" {
                    problems.push(format!(
                        "a record-scoped gap is recorded{at} but the overall verdict is not UNVERIFIED"
                    ));
                }
            } else if scope == "assertion" {
                for id in arr(g.get("assertion_ids")) {
                    let id_s = s(Some(id));
                    if !declared.contains(&id_s) {
                        problems.push(format!(
                            "a gap names assertion \"{id_s}\"{at}, which no preserved-contract facet declares"
                        ));
                    } else if state_of.get(&id_s).map(String::as_str) != Some("UNVERIFIED") {
                        problems.push(format!(
                            "a gap leaves assertion \"{id_s}\"{at} unclosed but its verdict is not UNVERIFIED"
                        ));
                    }
                }
            }
        }

        let baseline = r.get("baseline").unwrap_or(&empty_obj);
        // Same insertion-order concern as `declared` above: Node's
        // `baseCondIds` is also a `Set`, iterated below in declaration
        // order.
        let mut base_cond_ids: HashSet<String> = HashSet::new();
        let mut base_cond_order: Vec<String> = Vec::new();
        for c in arr(baseline.get("inputs_and_conditions")) {
            let id = s(c.get("id"));
            if id.is_empty() {
                continue;
            }
            if base_cond_ids.contains(&id) {
                problems.push(format!(
                    "baseline condition id \"{id}\"{at} is declared more than once"
                ));
            } else {
                base_cond_ids.insert(id.clone());
                base_cond_order.push(id);
            }
        }
        let pc_cond = post_change_evidence
            .get("inputs_and_conditions")
            .unwrap_or(&empty_obj);
        if s(pc_cond.get("relationship")) == "same" {
            let refs: Vec<String> = arr(pc_cond.get("baseline_condition_ids"))
                .iter()
                .map(|v| s(Some(v)))
                .collect();
            let ref_set: HashSet<&String> = refs.iter().collect();
            for r in &refs {
                if !base_cond_ids.contains(r) {
                    problems.push(format!(
                        "post-change conditions are declared \"same\"{at} but reference baseline condition \"{r}\", which the baseline does not define"
                    ));
                }
            }
            for id in &base_cond_order {
                if !ref_set.contains(id) {
                    problems.push(format!(
                        "post-change conditions are declared \"same\"{at} but do not reproduce baseline condition \"{id}\""
                    ));
                }
            }
        }

        if baseline
            .get("provenance")
            .and_then(|p| p.get("established"))
            == Some(&Value::Bool(false))
        {
            for id in &declared_order {
                if state_of.get(id).map(String::as_str) == Some("VERIFIED") {
                    problems.push(format!(
                        "the baseline provenance was not established{at} but assertion \"{id}\" is VERIFIED; an unestablished baseline leaves every assertion UNVERIFIED"
                    ));
                }
            }
            if overall != "UNVERIFIED" {
                problems.push(format!(
                    "the baseline provenance was not established{at} but the overall verdict is not UNVERIFIED"
                ));
            }
        }

        let bss = baseline.get("source_state").unwrap_or(&empty_obj);
        let pss = post_change_evidence
            .get("source_state")
            .unwrap_or(&empty_obj);
        let b_id = s(bss.get("identifier"));
        let b_kind = s(bss.get("identifier_kind"));
        if !b_id.is_empty()
            && b_id == s(pss.get("identifier"))
            && b_kind == s(pss.get("identifier_kind"))
        {
            problems.push(format!(
                "the baseline and post-change source states are the identical pair {{identifier_kind: \"{b_kind}\", identifier: \"{b_id}\"}}{at}; a before/after comparison needs two distinguishable states"
            ));
        }
    }

    problems
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn no_records_is_a_problem() {
        let problems = functional_parity_consistency(&json!({}));
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("no records"));
    }

    #[test]
    fn the_real_kernel_schema_and_fixtures_agree() {
        let kernel_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let fp_dir = kernel_root.join("verification/functional-parity");
        let schema_raw =
            std::fs::read_to_string(fp_dir.join("functional-parity-evidence.schema.json")).unwrap();
        let schema: Value = serde_json::from_str(&schema_raw).unwrap();
        let fixtures: Value = serde_json::from_str(
            &std::fs::read_to_string(
                fp_dir.join("fixtures/functional-parity-evidence.fixtures.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let groups = fixtures.as_array().unwrap();
        let group = groups
            .iter()
            .find(|g| {
                g.get("schema").and_then(Value::as_str)
                    == Some("functional-parity-evidence.schema.json")
            })
            .unwrap();
        for case in group["valid"].as_array().unwrap() {
            let doc = &case["doc"];
            let schema_errors = crate::source_format::json_schema::validate(doc, &schema).unwrap();
            let semantic = functional_parity_consistency(doc);
            assert!(
                schema_errors.is_empty() && semantic.is_empty(),
                "note={:?} schema_errors={:?} semantic={:?}",
                case.get("note"),
                schema_errors,
                semantic
            );
        }
        for case in group["invalid"].as_array().unwrap() {
            let doc = &case["doc"];
            let schema_errors =
                crate::source_format::json_schema::validate(doc, &schema).unwrap_or_default();
            let semantic = functional_parity_consistency(doc);
            assert!(
                !schema_errors.is_empty() || !semantic.is_empty(),
                "note={:?}",
                case.get("note")
            );
        }
    }
}
