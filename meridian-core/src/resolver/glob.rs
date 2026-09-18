//! The exact path-glob grammar `rule-resolution.md` §5/`rule-resolver.mjs`
//! define, ported without a general-purpose regex engine (a candidate
//! dependency was deliberately not added: the grammar is small, closed, and
//! directly expressible as a segment matcher).
//!
//! Grammar:
//!
//! - `**` — zero or more WHOLE path segments; only as a complete segment
//!   (`a/**/b`, `**/x`, `pkg/**`), never a fragment (`a**b`). Adjacent `**`
//!   segments collapse to one.
//! - `*` — any run of characters except `/`.
//! - `?` — exactly one character except `/`.
//! - every other character matches itself.
//!
//! Refused with [`GlobError`], never approximated: brace expansion `{a,b}`,
//! character classes `[...]`, extglob `!(...)`/`+(...)`/`@(...)`, a leading
//! `!` negation, and a `**` that is not a whole segment. Matching is
//! anchored against the full candidate path.

use core::fmt;

/// A value rejected while compiling a path-glob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlobError {
    /// The glob used brace expansion, a character class, extglob or a
    /// leading `!` negation — syntax outside the documented grammar, never
    /// interpreted approximately.
    UnsupportedSyntax {
        /// The rejected glob.
        glob: String,
    },
    /// The glob contained `**` that is not a whole path segment (for
    /// example `a**b`).
    FragmentedDoubleStar {
        /// The rejected glob.
        glob: String,
    },
}

impl fmt::Display for GlobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GlobError::UnsupportedSyntax { glob } => write!(
                f,
                "path-glob \"{glob}\" uses unsupported syntax (brace expansion, character class, extglob or \"!\" negation); the resolver does not interpret it approximately"
            ),
            GlobError::FragmentedDoubleStar { glob } => write!(
                f,
                "path-glob \"{glob}\" contains \"**\" that is not a whole path segment; write it as its own segment (\"a/**/b\") or use \"*\""
            ),
        }
    }
}

impl std::error::Error for GlobError {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum GlobSegment {
    /// A whole-segment `**`.
    DoubleStar,
    /// A literal segment, possibly containing `*`/`?` wildcards.
    Literal(String),
}

/// A compiled path-glob, ready to test candidate paths against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledGlob(Vec<GlobSegment>);

impl CompiledGlob {
    /// Compiles `glob`, rejecting unsupported syntax fail-closed.
    pub fn compile(glob: &str) -> Result<Self, GlobError> {
        if glob.chars().any(|c| "{}[]()!".contains(c)) {
            return Err(GlobError::UnsupportedSyntax {
                glob: glob.to_string(),
            });
        }
        let raw: Vec<&str> = glob.split('/').collect();
        for seg in &raw {
            if *seg != "**" && seg.contains("**") {
                return Err(GlobError::FragmentedDoubleStar {
                    glob: glob.to_string(),
                });
            }
        }
        // Adjacent "**" segments mean the same as one.
        let mut segments = Vec::with_capacity(raw.len());
        for seg in raw {
            if seg == "**" {
                if matches!(segments.last(), Some(GlobSegment::DoubleStar)) {
                    continue;
                }
                segments.push(GlobSegment::DoubleStar);
            } else {
                segments.push(GlobSegment::Literal(seg.to_string()));
            }
        }
        Ok(Self(segments))
    }

    /// Whether `path` matches this glob, anchored against the whole path.
    pub fn is_match(&self, path: &str) -> bool {
        let path_segments: Vec<&str> = path.split('/').collect();
        match_segments(&self.0, &path_segments)
    }
}

fn match_segments(glob: &[GlobSegment], path: &[&str]) -> bool {
    match glob.split_first() {
        None => path.is_empty(),
        Some((GlobSegment::DoubleStar, rest)) => {
            if match_segments(rest, path) {
                return true;
            }
            match path.split_first() {
                Some((_, path_rest)) => match_segments(glob, path_rest),
                None => false,
            }
        }
        Some((GlobSegment::Literal(literal), rest)) => match path.split_first() {
            Some((segment, path_rest)) => {
                segment_matches(literal, segment) && match_segments(rest, path_rest)
            }
            None => false,
        },
    }
}

/// Classic DP wildcard match of one path segment against a `*`/`?` pattern
/// (no `/` can occur in either, since both come from an already-split
/// segment).
fn segment_matches(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (pl, tl) = (p.len(), t.len());
    let mut dp = vec![vec![false; tl + 1]; pl + 1];
    dp[0][0] = true;
    for i in 1..=pl {
        if p[i - 1] == '*' {
            dp[i][0] = dp[i - 1][0];
        }
    }
    for i in 1..=pl {
        for j in 1..=tl {
            dp[i][j] = match p[i - 1] {
                '*' => dp[i - 1][j] || dp[i][j - 1],
                '?' => dp[i - 1][j - 1],
                c => dp[i - 1][j - 1] && c == t[j - 1],
            };
        }
    }
    dp[pl][tl]
}

/// Compiles `glob` and tests it against `path` in one call — the shape the
/// resolver needs for each `(glob, candidate_path)` pair.
pub fn glob_matches(glob: &str, path: &str) -> Result<bool, GlobError> {
    Ok(CompiledGlob::compile(glob)?.is_match(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(glob: &str, path: &str) -> bool {
        glob_matches(glob, path).unwrap()
    }

    #[test]
    fn literal_segment_matches_exactly() {
        assert!(matches("src/lib.rs", "src/lib.rs"));
        assert!(!matches("src/lib.rs", "src/main.rs"));
    }

    #[test]
    fn star_matches_any_run_within_a_segment() {
        assert!(matches("src/*.rs", "src/lib.rs"));
        assert!(matches("src/*.rs", "src/.rs"));
        assert!(!matches("src/*.rs", "src/sub/lib.rs"));
    }

    #[test]
    fn question_mark_matches_exactly_one_character() {
        assert!(matches("src/li?.rs", "src/lib.rs"));
        assert!(!matches("src/li?.rs", "src/libb.rs"));
    }

    #[test]
    fn double_star_matches_zero_or_more_whole_segments() {
        assert!(matches("**/x", "x"));
        assert!(matches("**/x", "a/x"));
        assert!(matches("**/x", "a/b/x"));
        assert!(matches("a/**/b", "a/b"));
        assert!(matches("a/**/b", "a/x/b"));
        assert!(matches("a/**/b", "a/x/y/b"));
        assert!(matches("pkg/**", "pkg/x"));
        assert!(matches("pkg/**", "pkg"));
    }

    #[test]
    fn whole_glob_double_star_matches_any_path() {
        assert!(matches("**", "anything/at/all"));
        assert!(matches("**", ""));
    }

    #[test]
    fn adjacent_double_star_segments_collapse() {
        assert!(matches("**/**/x", "x"));
        assert!(matches("**/**/x", "a/x"));
        assert!(matches("a/**/**/b", "a/b"));
        assert!(matches("a/**/**/b", "a/x/b"));
        assert!(matches("a/**/**/b", "a/x/y/b"));
    }

    #[test]
    fn rejects_brace_expansion() {
        assert!(matches!(
            CompiledGlob::compile("src/{a,b}.rs").unwrap_err(),
            GlobError::UnsupportedSyntax { .. }
        ));
    }

    #[test]
    fn rejects_character_class() {
        assert!(matches!(
            CompiledGlob::compile("src/[ab].rs").unwrap_err(),
            GlobError::UnsupportedSyntax { .. }
        ));
    }

    #[test]
    fn rejects_extglob_and_leading_negation() {
        assert!(matches!(
            CompiledGlob::compile("src/!(a).rs").unwrap_err(),
            GlobError::UnsupportedSyntax { .. }
        ));
        assert!(matches!(
            CompiledGlob::compile("!src/a.rs").unwrap_err(),
            GlobError::UnsupportedSyntax { .. }
        ));
    }

    #[test]
    fn rejects_fragmented_double_star() {
        assert!(matches!(
            CompiledGlob::compile("a**b").unwrap_err(),
            GlobError::FragmentedDoubleStar { .. }
        ));
    }

    #[test]
    fn matching_is_anchored_to_the_whole_path() {
        assert!(!matches("src/lib.rs", "other/src/lib.rs"));
        assert!(!matches("src/lib.rs", "src/lib.rs.bak"));
    }
}
