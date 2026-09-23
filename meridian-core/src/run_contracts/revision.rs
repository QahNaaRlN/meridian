//! The CLOSED, fail-closed classification of a revision string and the
//! "is this a sufficient pin?" rule built on it. This is the one production
//! owner of `classifyRevision`/`pinDefect`
//! (`scripts/lib/context-manifest.mjs`); the two 7c families still reach it
//! through the temporary `bounded_context_manifest::compat_7c` facade in
//! `meridian-app`.
//!
//! Classification follows the Node reference's patterns exactly, including
//! their case rules: a full Git SHA (40 hex, ANY case — `/i`), a SHA-256
//! digest (64 hex, any case), or a strict `v?X.Y.Z` tag with ASCII digits
//! is EXACT; a known moving token or anything containing `/` is FLOATING;
//! an all-hex string (any case — `/i`) is an ABBREVIATED SHA; any other
//! bare ASCII word is FLOATING; everything else is WEAK.

use core::fmt;

/// The five classes of [`RevisionClass::classify`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RevisionClass {
    Absent,
    Exact,
    Floating,
    AbbreviatedSha,
    Weak,
}

/// Known moving tokens that are never a pin, compared case-insensitively.
const FLOATING_REVISION_TOKENS: [&str; 17] = [
    "latest", "current", "head", "tip", "stable", "unstable", "newest", "rolling", "edge",
    "nightly", "trunk", "main", "master", "develop", "dev", "default", "release",
];

fn is_hex_len(s: &str, len: usize) -> bool {
    s.len() == len && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_semver_number(part: &str) -> bool {
    !part.is_empty()
        && part.bytes().all(|b| b.is_ascii_digit())
        && (part == "0" || !part.starts_with('0'))
}

fn is_strict_semver_tag(s: &str) -> bool {
    let body = s.strip_prefix('v').unwrap_or(s);
    let parts: Vec<&str> = body.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| is_semver_number(p))
}

fn is_bare_word(s: &str) -> bool {
    let mut bytes = s.bytes();
    bytes.next().is_some_and(|b| b.is_ascii_alphabetic())
        && bytes.all(|b| b.is_ascii_alphanumeric())
}

impl RevisionClass {
    /// Port of `classifyRevision`.
    pub fn classify(revision: Option<&str>) -> RevisionClass {
        let Some(t) = revision.map(str::trim).filter(|t| !t.is_empty()) else {
            return RevisionClass::Absent;
        };
        if is_hex_len(t, 40) || is_hex_len(t, 64) || is_strict_semver_tag(t) {
            return RevisionClass::Exact;
        }
        if FLOATING_REVISION_TOKENS.contains(&t.to_lowercase().as_str()) || t.contains('/') {
            return RevisionClass::Floating;
        }
        if t.bytes().all(|b| b.is_ascii_hexdigit()) {
            return RevisionClass::AbbreviatedSha;
        }
        if is_bare_word(t) {
            return RevisionClass::Floating;
        }
        RevisionClass::Weak
    }

    /// The Node reference's class label.
    pub fn as_str(self) -> &'static str {
        match self {
            RevisionClass::Absent => "absent",
            RevisionClass::Exact => "exact",
            RevisionClass::Floating => "floating",
            RevisionClass::AbbreviatedSha => "abbrev-sha",
            RevisionClass::Weak => "weak",
        }
    }
}

impl fmt::Display for RevisionClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A value rejected by [`PinSha256::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinSha256Error {
    pub value: String,
}

impl fmt::Display for PinSha256Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\"{}\" is not 64 hexadecimal characters", self.value)
    }
}

impl std::error::Error for PinSha256Error {}

/// A pin's SHA-256 digest: the schemas' `^[0-9a-fA-F]{64}$`. Unlike
/// [`crate::types::ContentDigest`] (lowercase only), either case is
/// admitted, exactly as the schema admits it; comparisons are
/// case-insensitive.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PinSha256(String);

impl PinSha256 {
    pub fn new(value: impl Into<String>) -> Result<Self, PinSha256Error> {
        let value = value.into();
        if is_hex_len(&value, 64) {
            Ok(Self(value))
        } else {
            Err(PinSha256Error { value })
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PinSha256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// `true` when `value` is a well-formed SHA-256 digest string with no
/// surrounding whitespace (the resolver-response rule).
pub(crate) fn is_sha256_text(value: &str) -> bool {
    is_hex_len(value, 64)
}

/// Port of `pinDefect`: `None` when a `{ revision, sha256 }` pair is a
/// sufficient pin, otherwise the reason.
pub(crate) fn pin_defect(revision: Option<&str>, has_sha256: bool) -> Option<String> {
    if has_sha256 {
        return None;
    }
    let quoted = || revision.map_or_else(|| "null".to_string(), super::json_quote);
    match RevisionClass::classify(revision) {
        RevisionClass::Exact => None,
        RevisionClass::Floating => Some(format!(
            "revision {} is a branch or channel reference; a branch or channel is never a pin, whatever digits it carries",
            quoted()
        )),
        RevisionClass::AbbreviatedSha => Some(format!(
            "revision {} looks like an abbreviated or ambiguous Git SHA (not a full 40- or 64-hex object name); pin the full SHA or add a sha256 digest",
            quoted()
        )),
        RevisionClass::Weak => Some(format!(
            "revision {} is not a recognised exact revision (a full Git SHA, a strict v?X.Y.Z tag or a SHA-256 digest); a provider-specific or unknown revision format requires a sha256 digest",
            quoted()
        )),
        RevisionClass::Absent => {
            Some("it is pinned by neither an exact revision nor a SHA-256 digest".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class(s: &str) -> &'static str {
        RevisionClass::classify(Some(s)).as_str()
    }

    #[test]
    fn classification_matches_the_closed_set() {
        assert_eq!(class(&"a".repeat(40)), "exact");
        assert_eq!(class("v1.2.3"), "exact");
        assert_eq!(class("0.10.0"), "exact");
        assert_eq!(class(&"a".repeat(64)), "exact");
        assert_eq!(class("main"), "floating");
        assert_eq!(class("Release"), "floating");
        assert_eq!(class("feature/x"), "floating");
        assert_eq!(class("abc123"), "abbrev-sha");
        assert_eq!(class("branch-2026"), "weak");
        assert_eq!(class("v01.2.3"), "weak");
        assert_eq!(class("  "), "absent");
        assert_eq!(RevisionClass::classify(None), RevisionClass::Absent);
    }

    /// The Node reference's `FULL_GIT_SHA_RE` and `HEX_ONLY_RE` carry the
    /// `/i` flag and its `\d` is ASCII; the previous Rust port used
    /// lowercase-only hex and a Unicode `\d`. These are Node-contract
    /// regressions, not new behaviour.
    #[test]
    fn classification_follows_the_node_case_and_digit_rules() {
        assert_eq!(class(&"ABCDEF0123".repeat(4)), "exact");
        assert_eq!(class("ABC123"), "abbrev-sha");
        assert_eq!(class("v1.\u{0662}.3"), "weak");
    }

    #[test]
    fn a_sha256_always_pins_and_otherwise_the_class_decides() {
        assert!(pin_defect(Some("main"), true).is_none());
        assert!(pin_defect(Some("v1.0.0"), false).is_none());
        assert!(pin_defect(Some("main"), false)
            .unwrap()
            .starts_with("revision \"main\" is a branch or channel reference"));
        assert_eq!(
            pin_defect(None, false).unwrap(),
            "it is pinned by neither an exact revision nor a SHA-256 digest"
        );
    }

    #[test]
    fn pin_sha256_admits_either_case_but_only_64_hex() {
        assert!(PinSha256::new("A".repeat(64)).is_ok());
        assert!(PinSha256::new("a".repeat(63)).is_err());
        assert!(PinSha256::new(format!(" {}", "a".repeat(64))).is_err());
    }
}
