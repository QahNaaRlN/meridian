//! [`Verdict`] — the outcome of a check whose closed set is exactly
//! `VERIFIED` / `UNVERIFIED` / `BLOCKED`
//! (`instance-data-migration.schema.json` `definitions.verification`).
//!
//! Other contracts define their own, differently-shaped closed sets over
//! similar-looking words (for example a migration sub-verdict's
//! `verified`/`unverified`, or an evidence result's
//! `confirmed`/`contradicted`/`inconclusive`) — those are deliberately kept
//! as their own, separate enums rather than folded into this one, per the
//! package instruction not to merge closed sets whose meaning differs.

use core::fmt;

/// The outcome of a check: `VERIFIED`, `UNVERIFIED`, or `BLOCKED`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Verdict {
    /// The claim is checked against actual evidence and holds.
    Verified,
    /// The claim is not (yet) checked and confirmed.
    Unverified,
    /// The claim cannot be checked at all — for example because its
    /// prerequisite (a reproducible source) itself failed.
    Blocked,
}

impl Verdict {
    /// The literal string the contract uses for this verdict.
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Verified => "VERIFIED",
            Verdict::Unverified => "UNVERIFIED",
            Verdict::Blocked => "BLOCKED",
        }
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_the_three_closed_values() {
        assert_eq!(Verdict::Verified.as_str(), "VERIFIED");
        assert_eq!(Verdict::Unverified.as_str(), "UNVERIFIED");
        assert_eq!(Verdict::Blocked.as_str(), "BLOCKED");
    }

    #[test]
    fn is_ordered_deterministically_for_stable_sorting() {
        let mut v = vec![Verdict::Blocked, Verdict::Verified, Verdict::Unverified];
        v.sort();
        assert_eq!(
            v,
            vec![Verdict::Verified, Verdict::Unverified, Verdict::Blocked]
        );
    }
}
