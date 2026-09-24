//! `carries_own_front_matter` — which Markdown documents are expected to
//! carry their own Front Matter at all, shared verbatim by two checks that
//! must never disagree about it: `document-identity` (adapted in
//! `meridian-cli::commands::validate::document_identity`, which still owns
//! everything else about that check — file/directory naming, the six
//! required fields) and `agent-instruction-identity`
//! (`crate::validation::mechanical_integrity::agent_instruction_identity`,
//! which also owns candidate selection: sort, dedup and classification).
//! Moved here (corrective round,
//! `meridian-cli-foundation-architecture-remediation`, item 3) so a CLI
//! shim never has to re-derive or duplicate this predicate — pure text
//! logic, no file, Git, environment or process I/O.

use std::path::Path;

pub fn carries_own_front_matter(rel: &str) -> bool {
    !rel.ends_with("-template.md")
        && !rel.ends_with("-body.md")
        && !rel.starts_with("instance-template/")
        && !rel.starts_with("test/")
        && Path::new(rel).file_name().and_then(|n| n.to_str()) != Some("SKILL.md")
}

/// The leading Front Matter block both checks read their fields from — the
/// same text the Node reference takes as
/// `text.slice(4, text.indexOf('\n---', 3))` (`''` when there is no
/// closing marker or the text does not open with `---`). Total over every
/// input: an empty block (`---\n---`) or a multi-byte character right after
/// the opening marker yields the reference's own result instead of a byte
/// range that `str` indexing would reject
/// (`rust-business-contract-qualification`, production panic audit).
///
/// Node's index 4 is the UTF-16 position after the one code unit at index
/// 3; for a character outside the BMP that position splits a surrogate
/// pair, leaving a non-newline code unit at the start of the block, so the
/// block here keeps that character instead — a line-anchored `^field:`
/// match sees the same first line either way.
pub fn front_matter_block(text: &str) -> &str {
    let Some(after_open) = text.strip_prefix("---") else {
        return "";
    };
    let Some(end) = after_open.find("\n---").map(|i| i + 3) else {
        return "";
    };
    let start = match after_open.chars().next() {
        Some(c) if c.len_utf16() == 1 => 3 + c.len_utf8(),
        _ => 3,
    };
    text.get(start..end).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_front_matter_block_is_the_text_between_the_two_markers() {
        assert_eq!(
            front_matter_block("---\ntitle: x\nstatus: y\n---\nbody\n"),
            "title: x\nstatus: y"
        );
        assert_eq!(
            front_matter_block("---\r\ntitle: x\r\n---\r\n"),
            "\ntitle: x\r"
        );
    }

    #[test]
    fn a_missing_opening_or_closing_marker_yields_an_empty_block() {
        assert_eq!(front_matter_block("# no front matter\n"), "");
        assert_eq!(front_matter_block("---\ntitle: never closed\n"), "");
        assert_eq!(front_matter_block(""), "");
    }

    /// Regression (`rust-business-contract-qualification`): `---\n---` made
    /// the former `&text[4..end]` a reversed range (4..3) and panicked,
    /// ending `meridian validate` with exit code 101 where the reference's
    /// `slice(4, 3)` is `''`.
    #[test]
    fn an_empty_block_is_empty_not_a_reversed_range() {
        assert_eq!(front_matter_block("---\n---\n# probe\n"), "");
        assert_eq!(front_matter_block("---\n---"), "");
    }

    /// Regression: a multi-byte character right after `---` put byte 4
    /// inside that character.
    #[test]
    fn a_multi_byte_character_after_the_opening_marker_is_not_split() {
        assert_eq!(front_matter_block("---é\n---\n"), "");
        assert_eq!(front_matter_block("---ж\ntitle: x\n---\n"), "\ntitle: x");
        assert_eq!(front_matter_block("---😀title: x\n---\n"), "😀title: x");
    }

    #[test]
    fn an_ordinary_document_carries_its_own_front_matter() {
        assert!(carries_own_front_matter("standards/workspace/x.md"));
    }

    #[test]
    fn a_template_body_or_skill_file_does_not() {
        assert!(!carries_own_front_matter("foo-template.md"));
        assert!(!carries_own_front_matter("foo-body.md"));
        assert!(!carries_own_front_matter("instance-template/x.md"));
        assert!(!carries_own_front_matter("test/fixture.md"));
        assert!(!carries_own_front_matter("skills/demo/SKILL.md"));
    }
}
