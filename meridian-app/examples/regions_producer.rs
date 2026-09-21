//! Verification-only producer for the cross-language conformance harness:
//! calls the REAL compiled `meridian_app::source_format::regions` functions
//! directly — no second, test-only copy of
//! `blank_fenced_blocks`/`marked_region`/`instruction_regions` logic. Its
//! Node counterpart (`verification/conformance-harness/regions-node-producer.mjs`)
//! calls the real exported `scripts/lib/regions.mjs` functions on the same
//! embedded corpus
//! (`verification/conformance-harness/fixtures/regions-corpus.json`), and
//! both are compared byte-for-byte by the conformance harness
//! (`real-node-rust-regions-adapters` fixture).
//!
//! `serde_json::Value::Object` is a `BTreeMap` in this workspace (no
//! `preserve_order` feature enabled anywhere), so building each observation
//! with `serde_json::json!` already yields the same recursively
//! sorted-key JSON the Node producer's own `canonical()` produces by
//! explicitly sorting `Object.keys` — the same reasoning
//! `meridian-cli/examples/resolve_cli_producer.rs` documents for its own
//! `result` field.

use meridian_app::source_format::regions::{
    blank_fenced_blocks, instruction_regions, marked_region,
};
use serde_json::{json, Value};

/// Named, intentional boundary (documented on the Node side too, in
/// `regions-node-producer.mjs`): the Node reference blanks one space per
/// UTF-16 code unit, while this port blanks one space per UTF-8 *byte* of
/// the original character — required here so every later byte offset used
/// to slice the original raw text stays valid, which UTF-16-width blanking
/// cannot guarantee for a multi-byte character
/// (`meridian-app/src/source_format/regions.rs::blank_line`'s own doc
/// comment). Nothing downstream of either language's own blanked buffer
/// ever inspects the *width* of a blanked run, so a run of blanked spaces is
/// collapsed to one fixed placeholder before comparison here — but ONLY on a
/// line that was actually fully blanked by `blank_fenced_blocks` (a line
/// whose text differs from the corresponding line of `raw`). A line
/// `blank_fenced_blocks` left untouched is compared byte-for-byte, including
/// any run of ordinary spaces it contains — collapsing spaces on every line,
/// blanked or not, would have hidden a real mismatch in untouched text
/// behind the same placeholder both languages happen to agree on.
fn normalize_blanked_runs(raw: &str, text: &str) -> String {
    let raw_lines: Vec<&str> = raw.split('\n').collect();
    text.split('\n')
        .enumerate()
        .map(|(i, line)| {
            if raw_lines.get(i) == Some(&line) {
                line.to_string()
            } else {
                collapse_blanked_spaces(line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn collapse_blanked_spaces(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == ' ' {
            while chars.peek() == Some(&' ') {
                chars.next();
            }
            out.push_str("\u{b7}BLANKED\u{b7}");
        } else {
            out.push(c);
        }
    }
    out
}

fn main() {
    let corpus: Value = serde_json::from_str(include_str!(
        "../../verification/conformance-harness/fixtures/regions-corpus.json"
    ))
    .expect("parse embedded regions corpus");

    for case in corpus["blank_fenced_blocks"]
        .as_array()
        .expect("blank_fenced_blocks cases")
    {
        let name = case["name"].as_str().expect("case name");
        let raw = case["raw"].as_str().expect("case raw");
        let result = blank_fenced_blocks(raw);
        let observation =
            json!({ "text": normalize_blanked_runs(raw, &result.text), "error": result.error });
        println!(
            "WARN  blank_fenced_blocks/{name}: {}",
            serde_json::to_string(&observation).expect("serialize observation")
        );
    }

    for case in corpus["marked_region"]
        .as_array()
        .expect("marked_region cases")
    {
        let name = case["name"].as_str().expect("case name");
        let raw = case["raw"].as_str().expect("case raw");
        let region_name = case["region_name"].as_str().expect("case region_name");
        let observation = match marked_region(raw, region_name) {
            Ok(region) => json!({ "text": region.text, "attrs": region.attrs }),
            Err(error) => json!({ "error": error }),
        };
        println!(
            "WARN  marked_region/{name}: {}",
            serde_json::to_string(&observation).expect("serialize observation")
        );
    }

    for case in corpus["instruction_regions"]
        .as_array()
        .expect("instruction_regions cases")
    {
        let name = case["name"].as_str().expect("case name");
        let raw = case["raw"].as_str().expect("case raw");
        let result = instruction_regions(raw);
        let regions: Vec<Value> = result
            .regions
            .iter()
            .map(|r| {
                json!({
                    "id": r.id,
                    "owner": r.owner,
                    "generated": r.generated,
                    "text": r.text,
                    "sourceText": r.source_text,
                })
            })
            .collect();
        let observation = json!({
            "errors": result.errors,
            "uncoveredLines": result.uncovered_lines,
            "regions": regions,
        });
        println!(
            "WARN  instruction_regions/{name}: {}",
            serde_json::to_string(&observation).expect("serialize observation")
        );
    }
}
