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

#[cfg(test)]
mod tests {
    use super::*;

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
