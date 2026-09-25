//! M-02/M-03 — the product-specific half of `kernel-purity`: the literals
//! and patterns a workspace's product record declares off-limits for the
//! Kernel, and the scan of one Kernel text against them.
//!
//! The literal set is derived exactly as the reference derives it from
//! `product.yaml` (product name, wiki space and URL, task-tracker URL,
//! repository host URL and namespace, plus `forbidden_literals`); the app
//! builds it from the typed product record. A literal matches as a
//! case-insensitive whole word, or — when it is a URL — as a
//! case-insensitive substring. A declared pattern that does not compile is a
//! typed error of the product record, never a skipped pattern.

use core::fmt;

use fancy_regex::Regex;

use crate::types::NonEmptyString;

/// Why a product record's purity declaration cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProductPurityError {
    /// A declared forbidden literal is empty or whitespace-only.
    EmptyLiteral,
    /// A declared forbidden pattern is not a valid regular expression.
    PatternDoesNotCompile { pattern: String, reason: String },
}

impl fmt::Display for ProductPurityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProductPurityError::EmptyLiteral => f.write_str("a forbidden literal is empty"),
            ProductPurityError::PatternDoesNotCompile { pattern, reason } => {
                write!(
                    f,
                    "forbidden pattern /{pattern}/ does not compile: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for ProductPurityError {}

/// How a literal is matched.
#[derive(Debug, Clone)]
enum LiteralMatcher {
    /// An ASCII literal: a substring search in the ASCII-lowercased text,
    /// with the reference's `\b` (ASCII word boundary) checked at both ends
    /// unless the literal is a URL. Exactly the reference's
    /// `/\b<literal>\b/i`, without a regex pass over the whole Kernel per
    /// literal.
    Ascii { needle: String, whole_word: bool },
    /// Any other literal: a case-insensitive regex for the literal alone,
    /// with the same ASCII `\b` checked around each match.
    Regex { regex: Regex, whole_word: bool },
}

/// One literal the Kernel must never carry.
#[derive(Debug, Clone)]
pub struct ForbiddenLiteral {
    text: NonEmptyString,
    matcher: LiteralMatcher,
}

fn is_word(byte: Option<u8>) -> bool {
    byte.is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_')
}

impl ForbiddenLiteral {
    pub fn new(text: impl Into<String>) -> Result<Self, ProductPurityError> {
        let text = NonEmptyString::new(text).map_err(|_| ProductPurityError::EmptyLiteral)?;
        let lower = text.as_str().to_ascii_lowercase();
        let whole_word = !(lower.starts_with("http://") || lower.starts_with("https://"));
        if text.as_str().is_ascii() {
            return Ok(Self {
                text,
                matcher: LiteralMatcher::Ascii {
                    needle: lower,
                    whole_word,
                },
            });
        }
        let source = format!("(?i){}", fancy_regex::escape(text.as_str()));
        let regex = Regex::new(&source).map_err(|e| ProductPurityError::PatternDoesNotCompile {
            pattern: source.clone(),
            reason: e.to_string(),
        })?;
        Ok(Self {
            text,
            matcher: LiteralMatcher::Regex { regex, whole_word },
        })
    }

    pub fn as_str(&self) -> &str {
        self.text.as_str()
    }

    /// `lowered` is `text` ASCII-lowercased (same byte offsets).
    fn is_match(&self, text: &str, lowered: &str) -> Result<bool, String> {
        match &self.matcher {
            LiteralMatcher::Ascii { needle, whole_word } => {
                let bytes = lowered.as_bytes();
                let first = needle.as_bytes().first().copied();
                let last = needle.as_bytes().last().copied();
                let mut from = 0;
                while let Some(offset) = lowered[from..].find(needle.as_str()) {
                    let start = from + offset;
                    let end = start + needle.len();
                    let boundary_before = start.checked_sub(1).map(|i| bytes[i]);
                    let boundary_after = bytes.get(end).copied();
                    if !whole_word
                        || (is_word(boundary_before) != is_word(first)
                            && is_word(last) != is_word(boundary_after))
                    {
                        return Ok(true);
                    }
                    // The needle is ASCII, so `start + 1` is inside it and
                    // the next char boundary is at most 3 bytes further.
                    from = (start + 1..=lowered.len())
                        .find(|i| lowered.is_char_boundary(*i))
                        .unwrap_or(lowered.len());
                }
                Ok(false)
            }
            LiteralMatcher::Regex { regex, whole_word } => {
                let bytes = text.as_bytes();
                for found in regex.find_iter(text) {
                    let found = found.map_err(|e| e.to_string())?;
                    let matched = found.as_str().as_bytes();
                    let before = found.start().checked_sub(1).map(|i| bytes[i]);
                    let after = bytes.get(found.end()).copied();
                    if !whole_word
                        || (is_word(before) != is_word(matched.first().copied())
                            && is_word(matched.last().copied()) != is_word(after))
                    {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }
}

/// One case-sensitive pattern the Kernel must never match.
#[derive(Debug, Clone)]
pub struct ForbiddenPattern {
    source: String,
    matcher: Regex,
}

impl ForbiddenPattern {
    pub fn new(source: impl Into<String>) -> Result<Self, ProductPurityError> {
        let source = source.into();
        let matcher =
            Regex::new(&source).map_err(|e| ProductPurityError::PatternDoesNotCompile {
                pattern: source.clone(),
                reason: e.to_string(),
            })?;
        Ok(Self { source, matcher })
    }

    /// The pattern as the reference prints a `RegExp`: `/<source>/`.
    pub fn display(&self) -> String {
        format!("/{}/", self.source)
    }
}

/// The whole purity declaration of one product.
#[derive(Debug, Clone)]
pub struct ProductPurity {
    literals: Vec<ForbiddenLiteral>,
    patterns: Vec<ForbiddenPattern>,
}

impl ProductPurity {
    pub fn new(literals: Vec<ForbiddenLiteral>, patterns: Vec<ForbiddenPattern>) -> Self {
        Self { literals, patterns }
    }

    pub fn literal_count(&self) -> usize {
        self.literals.len()
    }

    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }
}

/// What one scan of one Kernel text found.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PurityFinding {
    Literal {
        literal: String,
    },
    Pattern {
        pattern: String,
        matched: String,
    },
    /// The regex engine gave up on this text (backtracking limit); the text
    /// is not confirmed clean.
    Unevaluable {
        pattern: String,
        reason: String,
    },
}

impl PurityFinding {
    /// The reference's message for this finding in `file` (the file as the
    /// caller displays it).
    pub fn message(&self, file: &str) -> String {
        match self {
            PurityFinding::Literal { literal } => {
                format!("kernel-purity: product literal \"{literal}\" found in {file}")
            }
            PurityFinding::Pattern { pattern, matched } => {
                format!("kernel-purity: forbidden pattern {pattern} matched \"{matched}\" in {file}")
            }
            PurityFinding::Unevaluable { pattern, reason } => format!(
                "kernel-purity: {pattern} could not be evaluated against {file} ({reason}); the file is not confirmed clean"
            ),
        }
    }
}

/// Every literal (in declaration order) found in `text`, then the first
/// match of every pattern — the reference's order within one file.
pub fn scan(text: &str, purity: &ProductPurity) -> Vec<PurityFinding> {
    let mut findings = Vec::new();
    let lowered = text.to_ascii_lowercase();
    for literal in &purity.literals {
        match literal.is_match(text, &lowered) {
            Ok(true) => findings.push(PurityFinding::Literal {
                literal: literal.as_str().to_string(),
            }),
            Ok(false) => {}
            Err(reason) => findings.push(PurityFinding::Unevaluable {
                pattern: format!("product literal \"{}\"", literal.as_str()),
                reason,
            }),
        }
    }
    for pattern in &purity.patterns {
        match pattern.matcher.find(text) {
            Ok(Some(found)) => findings.push(PurityFinding::Pattern {
                pattern: pattern.display(),
                matched: found.as_str().to_string(),
            }),
            Ok(None) => {}
            Err(error) => findings.push(PurityFinding::Unevaluable {
                pattern: format!("forbidden pattern {}", pattern.display()),
                reason: error.to_string(),
            }),
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn purity(literals: &[&str], patterns: &[&str]) -> ProductPurity {
        ProductPurity::new(
            literals
                .iter()
                .map(|l| ForbiddenLiteral::new(*l).unwrap())
                .collect(),
            patterns
                .iter()
                .map(|p| ForbiddenPattern::new(*p).unwrap())
                .collect(),
        )
    }

    #[test]
    fn workspace_state_product_literal_matches_a_whole_word_case_insensitively() {
        let p = purity(&["Acme"], &[]);
        assert_eq!(
            scan("built for ACME customers", &p),
            [PurityFinding::Literal {
                literal: "Acme".to_string()
            }]
        );
        assert!(scan("acmeish is not the word", &p).is_empty());
    }

    /// The reference's `\b` is an ASCII word boundary on both sides of
    /// the literal, whatever the literal's own first and last characters.
    #[test]
    fn workspace_state_product_literal_boundaries_follow_the_reference() {
        let word = purity(&["Acme"], &[]);
        for (text, found) in [
            ("Acme", true),
            ("(acme)", true),
            ("acme_x", false),
            ("xacme", false),
            ("acme9", false),
            ("é acme é", true),
            ("acmeé", true),
            ("acmeacme acme", true),
        ] {
            assert_eq!(!scan(text, &word).is_empty(), found, "{text}");
        }
        let dotted = purity(&[".acme"], &[]);
        assert!(!scan("x.acme", &dotted).is_empty());
        assert!(scan(" .acme", &dotted).is_empty());
        // Between non-ASCII letters there is no ASCII word boundary, so the
        // reference never matches there; next to ASCII word characters it
        // does, case-insensitively.
        let cyrillic = purity(&["Продукт"], &[]);
        assert!(scan("это продукт", &cyrillic).is_empty());
        assert!(!scan("aпродуктb", &cyrillic).is_empty());
    }

    #[test]
    fn workspace_state_product_url_literal_matches_as_a_substring() {
        let p = purity(&["https://wiki.example/x"], &[]);
        assert_eq!(scan("see HTTPS://WIKI.EXAMPLE/x/page", &p).len(), 1);
        let messages: Vec<String> = scan("see https://wiki.example/x", &p)
            .iter()
            .map(|f| f.message("/k/a.md"))
            .collect();
        assert_eq!(
            messages,
            ["kernel-purity: product literal \"https://wiki.example/x\" found in /k/a.md"]
        );
    }

    #[test]
    fn workspace_state_product_pattern_is_case_sensitive_and_reports_its_match() {
        let p = purity(&[], &[r"\bWidgetd\b"]);
        let found = scan("the Widgetd service", &p);
        assert_eq!(
            found[0].message("/k/b.rs"),
            "kernel-purity: forbidden pattern /\\bWidgetd\\b/ matched \"Widgetd\" in /k/b.rs"
        );
        assert!(scan("the widgetd service", &p).is_empty());
    }

    #[test]
    fn workspace_state_product_uncompilable_pattern_is_a_typed_error() {
        let error = ForbiddenPattern::new("(unclosed").unwrap_err();
        assert!(matches!(
            error,
            ProductPurityError::PatternDoesNotCompile { ref pattern, .. } if pattern == "(unclosed"
        ));
        assert!(error
            .to_string()
            .starts_with("forbidden pattern /(unclosed/ does not compile: "));
        assert_eq!(
            ForbiddenLiteral::new("  ").unwrap_err(),
            ProductPurityError::EmptyLiteral
        );
    }
}
