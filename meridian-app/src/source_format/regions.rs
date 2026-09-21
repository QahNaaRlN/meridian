//! Marked regions. When something mechanical has to read a part of a
//! Markdown file rather than the whole of it, the part declares its own
//! boundaries. The alternative — locating it by a heading, or by the shape
//! of the tables inside it — makes the parser depend on prose that any
//! author may legitimately rewrite, and a parser that silently reads the
//! wrong region reports agreement it never checked. The vocabulary is one
//! pair of HTML comments:
//!   `<!-- meridian:begin <name> [key=value ...] -->`
//!   `<!-- meridian:end <name> -->`
//! Comments render as nothing in every Markdown tool, so the marker costs
//! the reader nothing and costs the parser no guessing. Anything other than
//! exactly one well-ordered pair is an error, never a fallback to the whole
//! file.
//!
//! Ported verbatim (parsing rules and every error string unchanged) from
//! `scripts/lib/regions.mjs`, file-independent: this module takes raw text
//! in and returns a typed result, with no file, Git, environment, process or
//! output I/O of its own. Every check that needs a marked region —
//! `instruction-topics`, `operating-foundation`, `stack-profiles` today —
//! reads it through this one implementation, not a copy embedded in each
//! check.

use fancy_regex::Regex;

/// The result of blanking every fenced code block in `raw`: an example is
/// never mistaken for a declaration.
pub struct BlankedFence {
    pub text: String,
    pub error: Option<String>,
}

/// Fenced code blocks are examples, not declarations: a document that
/// explains this very syntax would otherwise declare a region by quoting
/// one. Blanked, not removed, so every offset computed afterwards still
/// points where it did. A fence that never closes is an error rather than a
/// licence to blank the rest of the file — otherwise a Markdown typo hides
/// everything after it. A closing fence must be at least as long as the
/// opening one (CommonMark), or a shorter fence inside a longer one ends the
/// example early.
pub fn blank_fenced_blocks(raw: &str) -> BlankedFence {
    let fence_open = Regex::new(r"^ {0,3}(`{3,}|~{3,})").expect("fence-open pattern compiles");

    let original: Vec<&str> = raw.split('\n').collect();
    let mut lines: Vec<String> = original.iter().map(|s| s.to_string()).collect();

    let mut fence_char: Option<char> = None;
    let mut fence_len: usize = 0;
    let mut opened_at: i64 = -1;

    for i in 0..lines.len() {
        let m = fence_open
            .captures(&original[i])
            .ok()
            .flatten()
            .and_then(|c| c.get(1))
            .map(|g| g.as_str().to_string());

        let mut blank_this_line = false;
        match fence_char {
            None => {
                if let Some(marker) = &m {
                    fence_char = marker.chars().next();
                    fence_len = marker.chars().count();
                    opened_at = i as i64;
                    blank_this_line = true;
                }
                // else: outside any fence, no marker on this line — leave
                // this line untouched.
            }
            Some(open_char) => {
                if let Some(marker) = &m {
                    let marker_char = marker.chars().next();
                    if marker_char == Some(open_char) && marker.chars().count() >= fence_len {
                        fence_char = None;
                    }
                }
                blank_this_line = true;
            }
        }

        if blank_this_line {
            lines[i] = blank_line(original[i]);
        }
    }

    if fence_char.is_some() {
        let opened_at = opened_at as usize;
        for (i, line) in lines.iter_mut().enumerate().skip(opened_at) {
            *line = original[i].to_string();
        }
        return BlankedFence {
            text: lines.join("\n"),
            error: Some(format!(
                "a code fence opens at line {} and is never closed; until it is, everything after it can be read either as an example or as a declaration, and neither reading can be trusted",
                opened_at + 1
            )),
        };
    }

    BlankedFence {
        text: lines.join("\n"),
        error: None,
    }
}

/// Every character of `line` replaced with a space, except `\r` — preserved
/// so a CRLF line's length and trailing carriage return survive the blank,
/// exactly like the Node reference's `replace(/[^\r]/g, ' ')`. The Node
/// reference operates on UTF-16 code units, one space per unit; this port
/// operates on UTF-8 bytes, so a multi-byte character is replaced with as
/// many spaces as it has bytes — never with one space regardless of width.
/// Anything less would shift every byte offset computed after this point,
/// and both `marked_region` and `instruction_regions` slice the *original*
/// raw text by offsets taken from the blanked one.
fn blank_line(line: &str) -> String {
    line.chars()
        .map(|c| {
            if c == '\r' {
                c.to_string()
            } else {
                " ".repeat(c.len_utf8())
            }
        })
        .collect()
}

/// One named marked region's body, with two views (see [`marked_region`]).
#[derive(Debug)]
pub struct MarkedRegion {
    pub text: String,
    pub attrs: String,
}

/// Reads the single `name`d marked region out of `raw`. An error is returned
/// for anything other than exactly one well-ordered `begin`/`end` pair, or
/// for an unclosed fenced block anywhere in `raw` (a fence that cannot be
/// blanked cannot be trusted not to contain a quoted example marker).
pub fn marked_region(raw: &str, name: &str) -> Result<MarkedRegion, String> {
    let fenced = blank_fenced_blocks(raw);
    if let Some(error) = fenced.error {
        return Err(error);
    }
    let text = fenced.text;

    let begin_pattern = format!(r"<!--\s*meridian:begin\s+{name}\b([^>]*?)-->");
    let end_pattern = format!(r"<!--\s*meridian:end\s+{name}\s*-->");
    let begin_re = Regex::new(&begin_pattern).map_err(|e| e.to_string())?;
    let end_re = Regex::new(&end_pattern).map_err(|e| e.to_string())?;

    let begins: Vec<_> = begin_re
        .captures_iter(&text)
        .filter_map(|c| c.ok())
        .collect();
    let ends: Vec<_> = end_re.captures_iter(&text).filter_map(|c| c.ok()).collect();

    if begins.len() != 1 || ends.len() != 1 {
        return Err(format!(
            "expected exactly one \"meridian:begin {name}\" marker and one \"meridian:end {name}\" marker, found {} and {}",
            begins.len(),
            ends.len()
        ));
    }

    let begin_match = begins[0].get(0).expect("whole-match group 0 exists");
    let from = begin_match.start() + begin_match.as_str().len();
    let to = ends[0].get(0).expect("whole-match group 0 exists").start();

    if to < from {
        return Err(format!(
            "\"meridian:end {name}\" comes before \"meridian:begin {name}\""
        ));
    }

    let attrs = begins[0]
        .get(1)
        .map(|g| g.as_str().trim().to_string())
        .unwrap_or_default();

    Ok(MarkedRegion {
        text: text[from..to].to_string(),
        attrs,
    })
}

/// One `instruction-section` region out of a container file — see
/// [`instruction_regions`].
pub struct InstructionRegion {
    pub id: String,
    pub owner: String,
    pub generated: bool,
    /// The parser view: the slice of the fenced-blanked buffer, so a marker
    /// quoted inside a fenced example never counts as a declaration.
    pub text: String,
    /// The verbatim slice of the ORIGINAL raw input between the two
    /// markers, every character intact, fenced code included — the text to
    /// hash for a container norm's digest.
    pub source_text: String,
}

/// The result of parsing every `instruction-section` region out of a
/// container file such as `AGENTS.md`.
pub struct InstructionRegionsResult {
    pub errors: Vec<String>,
    pub regions: Vec<InstructionRegion>,
    pub uncovered_lines: usize,
}

/// Container files hold several subjects at once and are partly written by
/// a generator. The unit of intake for them is a declared region, not the
/// file: one topic assigned to a whole `AGENTS.md` would be false about
/// most of it. Regions declare themselves with the same marker vocabulary
/// the topic pool uses, plus the two attributes a container needs — who
/// owns the region, and whether a generator writes it.
pub fn instruction_regions(raw: &str) -> InstructionRegionsResult {
    // A leading Front Matter block is the file's own metadata, not a
    // subject of intake. It is blanked rather than removed so that every
    // offset below still points where it did.
    let mut text = raw.to_string();
    if text.starts_with("---") {
        if let Some(rel_close) = text[3..].find("\n---") {
            let close = rel_close + 3;
            // `close` is the index of the '\n' that opens "\n---"; the
            // closing marker line itself starts right after it, so the
            // search for its own terminating newline must start past that
            // point — searching from `close` would immediately find the
            // very same '\n' and leave the closing "---" line unblanked.
            let after = text[close + 1..].find('\n').map(|i| close + 1 + i);
            let cut = after.map(|a| a + 1).unwrap_or(text.len());
            let blanked: String = text[..cut]
                .chars()
                .map(|c| {
                    if c == '\n' {
                        c.to_string()
                    } else {
                        " ".repeat(c.len_utf8())
                    }
                })
                .collect();
            text = format!("{blanked}{}", &text[cut..]);
        }
    }

    // The same treatment of fenced examples the topic pool gets, from the
    // same helper: one rule, one implementation, one way to be wrong.
    let fenced = blank_fenced_blocks(&text);
    text = fenced.text;
    let mut errors = Vec::new();
    if let Some(error) = fenced.error {
        errors.push(error);
    }

    let token = Regex::new(r"<!--\s*meridian:(begin|end)\s+instruction-section\b([^>]*?)-->")
        .expect("region-token pattern compiles");

    struct Open {
        id: String,
        owner: String,
        generated: bool,
        start: usize,
    }

    let mut regions = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut open: Option<Open> = None;
    let mut cursor = 0usize;
    let mut outside = String::new();

    for m in token.captures_iter(&text) {
        let Ok(m) = m else { continue };
        let whole = m.get(0).expect("whole match exists");
        let kind = m.get(1).map(|g| g.as_str()).unwrap_or_default();
        let attrs_raw = m.get(2).map(|g| g.as_str()).unwrap_or_default();
        let attrs = parse_marker_attrs(attrs_raw);

        if kind == "begin" {
            if let Some(o) = &open {
                errors.push(format!(
                    "region \"{}\" is still open where region \"{}\" begins; regions sit side by side, they do not nest",
                    o.id,
                    attrs.get("id").cloned().unwrap_or_else(|| "?".to_string())
                ));
                return InstructionRegionsResult {
                    errors,
                    regions: Vec::new(),
                    uncovered_lines: 0,
                };
            }
            outside.push_str(&text[cursor..whole.start()]);
            let id = attrs.get("id").cloned().unwrap_or_default();
            if id.is_empty() {
                errors.push(
                    "a region begins without an id; a region that cannot be named cannot be recorded"
                        .to_string(),
                );
            } else if seen.contains(&id) {
                errors.push(format!(
                    "region id \"{id}\" is declared twice; two regions with one name are one name for two subjects"
                ));
            } else {
                seen.insert(id.clone());
            }
            let generated_str = attrs
                .get("generated")
                .cloned()
                .unwrap_or_else(|| "no".to_string());
            if generated_str != "yes" && generated_str != "no" {
                errors.push(format!(
                    "region \"{id}\" declares generated=\"{generated_str}\"; the answer is yes or no"
                ));
            }
            open = Some(Open {
                id,
                owner: attrs.get("owner").cloned().unwrap_or_default(),
                generated: generated_str == "yes",
                start: whole.start() + whole.as_str().len(),
            });
        } else {
            let Some(o) = open.take() else {
                errors.push("a region ends where none is open".to_string());
                return InstructionRegionsResult {
                    errors,
                    regions: Vec::new(),
                    uncovered_lines: 0,
                };
            };
            if let Some(closing_id) = attrs.get("id") {
                if closing_id != &o.id {
                    errors.push(format!(
                        "region \"{}\" is closed by a marker naming \"{closing_id}\"",
                        o.id
                    ));
                }
            }
            regions.push(InstructionRegion {
                id: o.id,
                owner: o.owner,
                generated: o.generated,
                text: text[o.start..whole.start()].to_string(),
                source_text: raw[o.start..whole.start()].to_string(),
            });
        }
        cursor = whole.start() + whole.as_str().len();
    }

    if let Some(o) = &open {
        errors.push(format!(
            "region \"{}\" is opened and never closed; everything after it would silently belong to it",
            o.id
        ));
    }
    outside.push_str(&text[cursor..]);

    let heading = Regex::new(r"^#{1,6}\s").expect("heading pattern compiles");
    let uncovered_lines = outside
        .split('\n')
        .filter(|l| {
            let trimmed = l.trim();
            !trimmed.is_empty() && !heading.is_match(trimmed).unwrap_or(false)
        })
        .count();

    InstructionRegionsResult {
        errors,
        regions,
        uncovered_lines,
    }
}

fn parse_marker_attrs(s: &str) -> std::collections::HashMap<String, String> {
    let re = Regex::new(r#"([a-z][a-z0-9-]*)=("([^"]*)"|[^\s"]+)"#)
        .expect("marker-attrs pattern compiles");
    let mut out = std::collections::HashMap::new();
    for m in re.captures_iter(s) {
        let Ok(m) = m else { continue };
        let key = m.get(1).map(|g| g.as_str().to_string());
        let value = m
            .get(3)
            .or_else(|| m.get(2))
            .map(|g| g.as_str().to_string());
        if let (Some(key), Some(value)) = (key, value) {
            out.insert(key, value);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_fenced_blocks_blanks_content_and_preserves_offsets() {
        let raw = "before\n```\ncode <!-- meridian:begin x -->\n```\nafter\n";
        let out = blank_fenced_blocks(raw);
        assert!(out.error.is_none());
        assert!(out.text.contains("before"));
        assert!(out.text.contains("after"));
        assert!(!out.text.contains("meridian:begin"));
        assert_eq!(out.text.split('\n').count(), raw.split('\n').count());
    }

    #[test]
    fn blank_fenced_blocks_reports_an_unclosed_fence() {
        let raw = "before\n```\nnever closes\n";
        let out = blank_fenced_blocks(raw);
        assert!(out.error.is_some());
        assert!(out.text.contains("never closes"));
    }

    #[test]
    fn blank_fenced_blocks_a_shorter_nested_fence_does_not_close_the_outer_one() {
        let raw = "````\n```\n````\nafter\n";
        let out = blank_fenced_blocks(raw);
        assert!(out.error.is_none());
        assert!(out.text.contains("after"));
    }

    #[test]
    fn marked_region_reads_exactly_the_body_between_its_markers() {
        let raw = "prefix\n<!-- meridian:begin topic-pool owner=x -->\nbody line\n<!-- meridian:end topic-pool -->\nsuffix\n";
        let region = marked_region(raw, "topic-pool").expect("region reads");
        assert!(region.text.contains("body line"));
        assert!(!region.text.contains("prefix"));
        assert!(!region.text.contains("suffix"));
        assert_eq!(region.attrs, "owner=x");
    }

    #[test]
    fn marked_region_rejects_a_missing_marker() {
        let err = marked_region("no markers here\n", "topic-pool").unwrap_err();
        assert!(err.contains("expected exactly one"));
    }

    #[test]
    fn marked_region_rejects_a_duplicated_begin_marker() {
        let raw = "<!-- meridian:begin topic-pool -->\na\n<!-- meridian:end topic-pool -->\n<!-- meridian:begin topic-pool -->\nb\n<!-- meridian:end topic-pool -->\n";
        let err = marked_region(raw, "topic-pool").unwrap_err();
        assert!(err.contains("found 2"));
    }

    #[test]
    fn marked_region_ignores_a_marker_quoted_inside_a_fenced_example() {
        let raw = "```\n<!-- meridian:begin topic-pool -->\nquoted\n<!-- meridian:end topic-pool -->\n```\n<!-- meridian:begin topic-pool -->\nreal\n<!-- meridian:end topic-pool -->\n";
        let region = marked_region(raw, "topic-pool").expect("region reads");
        assert!(region.text.contains("real"));
        assert!(!region.text.contains("quoted"));
    }

    #[test]
    fn instruction_regions_reads_side_by_side_regions_and_ignores_headings_outside() {
        let raw = "# Title\n<!-- meridian:begin instruction-section id=\"a\" owner=\"team-a\" generated=\"no\" -->\nbody a\n<!-- meridian:end instruction-section -->\n<!-- meridian:begin instruction-section id=\"b\" owner=\"team-b\" generated=\"yes\" -->\nbody b\n<!-- meridian:end instruction-section -->\n";
        let result = instruction_regions(raw);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.regions.len(), 2);
        assert_eq!(result.regions[0].id, "a");
        assert!(!result.regions[0].generated);
        assert_eq!(result.regions[1].id, "b");
        assert!(result.regions[1].generated);
        assert_eq!(result.uncovered_lines, 0);
    }

    #[test]
    fn instruction_regions_rejects_nesting() {
        let raw = "<!-- meridian:begin instruction-section id=\"a\" -->\n<!-- meridian:begin instruction-section id=\"b\" -->\n<!-- meridian:end instruction-section -->\n<!-- meridian:end instruction-section -->\n";
        let result = instruction_regions(raw);
        assert!(!result.errors.is_empty());
        assert!(result.errors[0].contains("still open"));
    }

    #[test]
    fn instruction_regions_rejects_a_duplicated_region_id() {
        let raw = "<!-- meridian:begin instruction-section id=\"a\" -->\nbody a\n<!-- meridian:end instruction-section -->\n<!-- meridian:begin instruction-section id=\"a\" -->\nbody a again\n<!-- meridian:end instruction-section -->\n";
        let result = instruction_regions(raw);
        assert!(
            result.errors.iter().any(|e| e.contains("declared twice")),
            "{:?}",
            result.errors
        );
    }

    #[test]
    fn instruction_regions_blanks_a_leading_russian_front_matter_through_its_closing_line() {
        let raw = "---\nзаголовок: Значение с русским текстом\nвладелец: команда\n---\n<!-- meridian:begin instruction-section id=\"a\" -->\nbody\n<!-- meridian:end instruction-section -->\n";
        let result = instruction_regions(raw);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.regions.len(), 1);
        assert_eq!(result.regions[0].id, "a");
        assert_eq!(result.regions[0].source_text, "\nbody\n");
        assert_eq!(result.uncovered_lines, 0);
    }

    #[test]
    fn front_matter_blanking_preserves_byte_offsets_with_multibyte_characters() {
        // Every character in the Front Matter is multi-byte in UTF-8; a
        // naive one-space-per-char replacement would shift every offset
        // after it and misalign the region slice taken from `raw`.
        let raw = "---\nключ: 日本語テスト\n---\n<!-- meridian:begin instruction-section id=\"a\" -->\nbody\n<!-- meridian:end instruction-section -->\n";
        let result = instruction_regions(raw);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.regions[0].source_text, "\nbody\n");
    }

    #[test]
    fn blank_fenced_blocks_preserves_byte_offsets_with_unicode_before_a_later_region() {
        let raw = "```\nこんにちは 世界\n```\n<!-- meridian:begin topic-pool -->\nreal body\n<!-- meridian:end topic-pool -->\n";
        let region = marked_region(raw, "topic-pool").expect("region reads");
        assert_eq!(region.text.trim(), "real body");
    }

    #[test]
    fn marked_region_handles_crlf_line_endings() {
        let raw = "prefix\r\n<!-- meridian:begin topic-pool -->\r\nbody line\r\n<!-- meridian:end topic-pool -->\r\nsuffix\r\n";
        let region = marked_region(raw, "topic-pool").expect("region reads");
        assert!(region.text.contains("body line"));
        assert!(!region.text.contains("prefix"));
        assert!(!region.text.contains("suffix"));
    }

    #[test]
    fn blank_fenced_blocks_with_crlf_preserves_carriage_returns_and_offsets() {
        let raw = "before\r\n```\r\ncode <!-- meridian:begin x -->\r\n```\r\nafter\r\n";
        let out = blank_fenced_blocks(raw);
        assert!(out.error.is_none());
        assert!(out.text.contains("before"));
        assert!(out.text.contains("after"));
        assert!(!out.text.contains("meridian:begin"));
        assert_eq!(out.text.len(), raw.len());
        for line in out.text.split('\n') {
            if line.trim_start().is_empty() {
                // A blanked line still ends with its own '\r', preserved.
                assert!(line.is_empty() || line.ends_with('\r'));
            }
        }
    }

    #[test]
    fn marked_region_rejects_an_unclosed_fence_anywhere_in_the_document() {
        let raw = "<!-- meridian:begin topic-pool -->\nbody\n<!-- meridian:end topic-pool -->\n```\nnever closes\n";
        let err = marked_region(raw, "topic-pool").unwrap_err();
        assert!(err.contains("never closed"));
    }

    #[test]
    fn instruction_regions_rejects_an_unclosed_fence_anywhere_in_the_document() {
        let raw = "<!-- meridian:begin instruction-section id=\"a\" -->\nbody\n<!-- meridian:end instruction-section -->\n```\nnever closes\n";
        let result = instruction_regions(raw);
        assert!(result.errors.iter().any(|e| e.contains("never closed")));
    }

    #[test]
    fn instruction_regions_counts_uncovered_non_heading_lines() {
        let raw = "# Title\nstray text outside every region\n<!-- meridian:begin instruction-section id=\"a\" -->\nbody\n<!-- meridian:end instruction-section -->\n";
        let result = instruction_regions(raw);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.uncovered_lines, 1);
    }
}
