//! [`Timestamp`] — one instant, as milliseconds since the Unix epoch (UTC).
//!
//! Parsed from the ISO 8601 forms the inventory and the gate-run log carry
//! (`2026-08-30T20:26:52Z`, `2026-08-17T13:02:27.358Z`, a numeric offset, or
//! a bare `2026-08-30`, which is UTC midnight — the forms `Date.parse`
//! reads), and formatted the way `Date.prototype.toISOString` writes. The
//! current instant itself is never read here: it arrives through the app's
//! `Clock` port.

use core::fmt;

const MS_PER_DAY: i64 = 86_400_000;

/// Milliseconds since `1970-01-01T00:00:00.000Z`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(i64);

impl Timestamp {
    pub fn from_unix_millis(millis: i64) -> Self {
        Self(millis)
    }

    pub fn unix_millis(self) -> i64 {
        self.0
    }

    /// Parses an ISO 8601 date or date-time; `None` for anything else,
    /// including an out-of-range field.
    pub fn parse_iso8601(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        let number = |from: usize, len: usize| -> Option<i64> {
            let part = bytes.get(from..from + len)?;
            if !part.iter().all(u8::is_ascii_digit) {
                return None;
            }
            core::str::from_utf8(part).ok()?.parse().ok()
        };
        let year = number(0, 4)?;
        if bytes.get(4) != Some(&b'-') || bytes.get(7) != Some(&b'-') {
            return None;
        }
        let month = number(5, 2)?;
        let day = number(8, 2)?;
        if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
            return None;
        }
        let date = days_from_civil(year, month, day) * MS_PER_DAY;
        if bytes.len() == 10 {
            return Some(Self(date));
        }
        if bytes.get(10) != Some(&b'T') || bytes.get(13) != Some(&b':') {
            return None;
        }
        let hour = number(11, 2)?;
        let minute = number(14, 2)?;
        let (second, mut at) = if bytes.get(16) == Some(&b':') {
            (number(17, 2)?, 19)
        } else {
            (0, 16)
        };
        if hour > 23 || minute > 59 || second > 59 {
            return None;
        }
        let mut millis = 0;
        if bytes.get(at) == Some(&b'.') {
            let start = at + 1;
            let mut end = start;
            while bytes.get(end).is_some_and(u8::is_ascii_digit) {
                end += 1;
            }
            if end == start {
                return None;
            }
            let digits = &text[start..end.min(start + 3)];
            millis = digits.parse::<i64>().ok()? * 10_i64.pow(3 - digits.len() as u32);
            at = end;
        }
        let offset_minutes = match &text[at..] {
            "Z" | "z" => 0,
            zone if zone.len() == 6
                && (zone.starts_with('+') || zone.starts_with('-'))
                && zone.as_bytes()[3] == b':'
                && [1, 2, 4, 5]
                    .iter()
                    .all(|&i| zone.as_bytes()[i].is_ascii_digit()) =>
            {
                let sign = if zone.starts_with('-') { -1 } else { 1 };
                let hours: i64 = zone[1..3].parse().ok()?;
                let minutes: i64 = zone[4..6].parse().ok()?;
                if hours > 23 || minutes > 59 {
                    return None;
                }
                sign * (hours * 60 + minutes)
            }
            _ => return None,
        };
        let local = date + ((hour * 60 + minute) * 60 + second) * 1000 + millis;
        Some(Self(local - offset_minutes * 60_000))
    }

    /// Whole-and-fractional days from `self` to `now` (negative when `self`
    /// lies in the future).
    pub fn days_until(self, now: Timestamp) -> f64 {
        (now.0 - self.0) as f64 / MS_PER_DAY as f64
    }
}

impl fmt::Display for Timestamp {
    /// `YYYY-MM-DDTHH:MM:SS.mmmZ`, as `Date.prototype.toISOString`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let days = self.0.div_euclid(MS_PER_DAY);
        let in_day = self.0.rem_euclid(MS_PER_DAY);
        let (year, month, day) = civil_from_days(days);
        write!(
            f,
            "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
            in_day / 3_600_000,
            in_day / 60_000 % 60,
            in_day / 1000 % 60,
            in_day % 1000
        )
    }
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if is_leap(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since the epoch of a proleptic Gregorian date (H. Hinnant's
/// `days_from_civil`).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_state_timestamp_parses_the_inventory_and_log_forms() {
        let z = Timestamp::parse_iso8601("2026-08-30T20:26:52Z").unwrap();
        assert_eq!(z.unix_millis(), 1_788_121_612_000);
        let ms = Timestamp::parse_iso8601("2026-08-17T13:02:27.358Z").unwrap();
        assert_eq!(ms.to_string(), "2026-08-17T13:02:27.358Z");
        let offset = Timestamp::parse_iso8601("2026-08-30T22:26:52+02:00").unwrap();
        assert_eq!(offset, z);
        let date = Timestamp::parse_iso8601("2026-08-30").unwrap();
        assert_eq!(date.to_string(), "2026-08-30T00:00:00.000Z");
        assert_eq!(z.to_string(), "2026-08-30T20:26:52.000Z");
    }

    #[test]
    fn workspace_state_timestamp_rejects_what_is_not_an_iso_instant() {
        for bad in [
            "",
            "yesterday",
            "2026-13-01",
            "2026-02-30",
            "2026-08-30T25:00:00Z",
            "2026-08-30T20:26:52",
            "2026-08-30 20:26:52Z",
            "2026-08-30T20:26:52.Z",
        ] {
            assert_eq!(Timestamp::parse_iso8601(bad), None, "{bad}");
        }
    }

    #[test]
    fn workspace_state_timestamp_measures_age_in_days() {
        let then = Timestamp::parse_iso8601("2026-09-01T00:00:00Z").unwrap();
        let now = Timestamp::parse_iso8601("2026-09-04T12:00:00Z").unwrap();
        assert_eq!(then.days_until(now), 3.5);
    }
}
