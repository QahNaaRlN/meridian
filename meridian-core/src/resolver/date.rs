//! [`IsoDate`] — a `YYYY-MM-DD` calendar date, as `recorded_at` fields carry
//! (`registries/rule-resolution/applicability.schema.json`, `format: date`).
//!
//! Stored as its canonical text form: two `IsoDate` values compare
//! chronologically under ordinary string/derived ordering because the
//! `YYYY-MM-DD` form sorts lexicographically the same as it sorts in time —
//! the same property `rule-resolver.mjs`'s `pickAuthoritative` relies on
//! (`dates.sort()` over `recorded_at` strings).

use core::fmt;

/// A value rejected by [`IsoDate::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsoDateError {
    /// The rejected value.
    pub value: String,
}

impl fmt::Display for IsoDateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\"{}\" is not a well-formed YYYY-MM-DD calendar date",
            self.value
        )
    }
}

impl std::error::Error for IsoDateError {}

/// A `YYYY-MM-DD` calendar date.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IsoDate(String);

impl IsoDate {
    /// Validates and wraps `value`: exactly `YYYY-MM-DD`, four digit year,
    /// month `01`–`12`, day `01`–`31` (not adjusted for the month's actual
    /// length — the applicability contract needs deterministic ordering and
    /// well-formedness, not a full calendar validator).
    pub fn new(value: impl Into<String>) -> Result<Self, IsoDateError> {
        let value = value.into();
        if !is_well_formed(&value) {
            return Err(IsoDateError { value });
        }
        Ok(Self(value))
    }

    /// Borrows the underlying `YYYY-MM-DD` string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_well_formed(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 {
        return false;
    }
    if bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let is_digit = |b: u8| b.is_ascii_digit();
    if !(bytes[0..4].iter().all(|&b| is_digit(b))) {
        return false;
    }
    if !(bytes[5..7].iter().all(|&b| is_digit(b))) {
        return false;
    }
    if !(bytes[8..10].iter().all(|&b| is_digit(b))) {
        return false;
    }
    let month: u32 = value[5..7].parse().unwrap_or(0);
    let day: u32 = value[8..10].parse().unwrap_or(0);
    (1..=12).contains(&month) && (1..=31).contains(&day)
}

impl fmt::Display for IsoDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for IsoDate {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_well_formed_date() {
        assert!(IsoDate::new("2026-09-18").is_ok());
    }

    #[test]
    fn rejects_wrong_length() {
        assert!(IsoDate::new("2026-9-18").is_err());
        assert!(IsoDate::new("2026-09-8").is_err());
    }

    #[test]
    fn rejects_bad_separators() {
        assert!(IsoDate::new("2026/09/18").is_err());
    }

    #[test]
    fn rejects_out_of_range_month_or_day() {
        assert!(IsoDate::new("2026-13-01").is_err());
        assert!(IsoDate::new("2026-01-32").is_err());
        assert!(IsoDate::new("2026-00-01").is_err());
        assert!(IsoDate::new("2026-01-00").is_err());
    }

    #[test]
    fn orders_chronologically_by_string_order() {
        let a = IsoDate::new("2026-09-08").unwrap();
        let b = IsoDate::new("2026-09-18").unwrap();
        let c = IsoDate::new("2026-10-01").unwrap();
        assert!(a < b);
        assert!(b < c);
    }
}
