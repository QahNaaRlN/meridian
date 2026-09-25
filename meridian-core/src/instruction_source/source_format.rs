//! [`SourceFormat`] — the closed `payload.format` pool
//! (`instruction-source-registry.md`): the shape of a registered instruction
//! source's content, modelled separately from [`super::ReadChannel`] — one
//! is never derived from the other.

use core::fmt;

/// The shape of a registered instruction source's content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceFormat {
    AgentsMd,
    ClaudeMd,
    CursorRule,
    KernelDoc,
    MarkdownSection,
    PlainText,
}

impl SourceFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            SourceFormat::AgentsMd => "agents-md",
            SourceFormat::ClaudeMd => "claude-md",
            SourceFormat::CursorRule => "cursor-rule",
            SourceFormat::KernelDoc => "kernel-doc",
            SourceFormat::MarkdownSection => "markdown-section",
            SourceFormat::PlainText => "plain-text",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "agents-md" => Some(SourceFormat::AgentsMd),
            "claude-md" => Some(SourceFormat::ClaudeMd),
            "cursor-rule" => Some(SourceFormat::CursorRule),
            "kernel-doc" => Some(SourceFormat::KernelDoc),
            "markdown-section" => Some(SourceFormat::MarkdownSection),
            "plain-text" => Some(SourceFormat::PlainText),
            _ => None,
        }
    }
}

impl fmt::Display for SourceFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_closed_value() {
        for (text, variant) in [
            ("agents-md", SourceFormat::AgentsMd),
            ("claude-md", SourceFormat::ClaudeMd),
            ("cursor-rule", SourceFormat::CursorRule),
            ("kernel-doc", SourceFormat::KernelDoc),
            ("markdown-section", SourceFormat::MarkdownSection),
            ("plain-text", SourceFormat::PlainText),
        ] {
            assert_eq!(SourceFormat::parse(text), Some(variant));
            assert_eq!(variant.as_str(), text);
        }
    }

    #[test]
    fn parse_rejects_values_outside_the_closed_pool() {
        assert_eq!(SourceFormat::parse("invented"), None);
    }
}
