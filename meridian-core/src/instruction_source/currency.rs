//! [`Currency`] — the closed `recorded_state.currency` pool
//! (`instruction-source-registry.md`).
//!
//! Moved here from `controlled_rule_intake::resolved` by
//! `rust-architecture-conformance-2` (§1): currency is a fact about a
//! registered instruction source's held snapshot, not specific to the
//! `controlled-rule-intake` contract that happens to compare a pin against
//! one. `controlled_rule_intake` re-exports this type unchanged.

use core::fmt;

/// The closed `recorded_state.currency` pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Currency {
    Current,
    Stale,
    Unverified,
}

impl Currency {
    pub fn as_str(self) -> &'static str {
        match self {
            Currency::Current => "current",
            Currency::Stale => "stale",
            Currency::Unverified => "unverified",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "current" => Some(Currency::Current),
            "stale" => Some(Currency::Stale),
            "unverified" => Some(Currency::Unverified),
            _ => None,
        }
    }
}

impl fmt::Display for Currency {
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
            ("current", Currency::Current),
            ("stale", Currency::Stale),
            ("unverified", Currency::Unverified),
        ] {
            assert_eq!(Currency::parse(text), Some(variant));
            assert_eq!(variant.as_str(), text);
        }
    }

    #[test]
    fn parse_rejects_values_outside_the_closed_pool() {
        assert_eq!(Currency::parse("invented"), None);
    }
}
