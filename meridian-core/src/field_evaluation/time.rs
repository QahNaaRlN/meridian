//! Calendar dates, timestamps and the windows built from them — validated
//! types, never a digit pattern `Date.parse` would silently normalise.
//!
//! - [`CalendarDate`] is `YYYY-MM-DD` naming a date that exists
//!   (proleptic Gregorian leap years), ordered chronologically.
//! - [`Timestamp`] is `YYYY-MM-DDTHH:MM:SS[.fraction](Z|±HH:MM)` naming a
//!   date and time that exist, with an offset `Date.parse` accepts
//!   (hours ≤ 23, minutes ≤ 59). It orders by its UTC instant at
//!   millisecond resolution: a fraction is read to its first three digits,
//!   right-padded and truncated — `.1` is 100 ms, `.1239` is 123 ms — so a
//!   sub-second interval is never collapsed to zero.
//! - [`DateWindow`] is a pair of dates whose end is not before its start;
//!   [`TimeInterval`] a pair of timestamps whose end is strictly after its
//!   start. An impossible window or interval is unrepresentable.

use core::fmt;

fn digits(s: &str) -> Option<u32> {
    (!s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        .then(|| s.parse().ok())
        .flatten()
}

const fn is_leap_year(year: u32) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

const fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        2 if is_leap_year(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// A date that exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CalendarDate {
    year: u32,
    month: u32,
    day: u32,
}

impl CalendarDate {
    /// `isValidDate`: `^\d{4}-\d{2}-\d{2}$` naming an existing date.
    pub fn parse(value: &str) -> Option<CalendarDate> {
        let bytes = value.as_bytes();
        if bytes.len() != 10 || bytes.get(4) != Some(&b'-') || bytes.get(7) != Some(&b'-') {
            return None;
        }
        let year = digits(value.get(0..4)?)?;
        let month = digits(value.get(5..7)?)?;
        let day = digits(value.get(8..10)?)?;
        CalendarDate::new(year, month, day)
    }

    fn new(year: u32, month: u32, day: u32) -> Option<CalendarDate> {
        ((1..=12).contains(&month) && (1..=days_in_month(year, month)).contains(&day))
            .then_some(CalendarDate { year, month, day })
    }

    /// Days since 1970-01-01 (Howard Hinnant's `days_from_civil`).
    fn days_from_epoch(self) -> i64 {
        let (y, m, d) = (
            i64::from(self.year),
            i64::from(self.month),
            i64::from(self.day),
        );
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let mp = (m + 9) % 12;
        let doy = (153 * mp + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }
}

impl fmt::Display for CalendarDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// A timestamp that exists, ordered by its UTC instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    utc_millis: i64,
}

impl Timestamp {
    /// `isValidDateTime` (format, calendar, clock, an offset `Date.parse`
    /// accepts) and the instant `Date.parse` reads.
    pub fn parse(value: &str) -> Option<Timestamp> {
        let date = CalendarDate::parse(value.get(0..10)?)?;
        let rest = value.get(10..)?;
        let rest = rest.strip_prefix(['T', 't'])?;
        let clock = rest.get(0..8)?;
        let cb = clock.as_bytes();
        if cb.get(2) != Some(&b':') || cb.get(5) != Some(&b':') {
            return None;
        }
        let hour = digits(clock.get(0..2)?)?;
        let minute = digits(clock.get(3..5)?)?;
        let second = digits(clock.get(6..8)?)?;
        if hour > 23 || minute > 59 || second > 59 {
            return None;
        }
        let mut rest = rest.get(8..)?;
        let mut millis = 0i64;
        if let Some(after_dot) = rest.strip_prefix('.') {
            let len = after_dot.bytes().take_while(u8::is_ascii_digit).count();
            if len == 0 {
                return None;
            }
            let fraction = after_dot.get(..len)?;
            let mut first_three = [b'0'; 3];
            for (slot, b) in first_three.iter_mut().zip(fraction.bytes()) {
                *slot = b;
            }
            millis = first_three
                .iter()
                .fold(0i64, |acc, b| acc * 10 + i64::from(b - b'0'));
            rest = after_dot.get(len..)?;
        }
        let offset_minutes: i64 = match rest {
            "Z" | "z" => 0,
            _ => {
                let sign = match rest.as_bytes().first() {
                    Some(b'+') => 1,
                    Some(b'-') => -1,
                    _ => return None,
                };
                let offset = rest.get(1..)?;
                if offset.len() != 5 || offset.as_bytes().get(2) != Some(&b':') {
                    return None;
                }
                let oh = digits(offset.get(0..2)?)?;
                let om = digits(offset.get(3..5)?)?;
                if oh > 23 || om > 59 {
                    return None;
                }
                sign * i64::from(oh * 60 + om)
            }
        };
        let local = date.days_from_epoch() * 86_400_000
            + i64::from(hour * 3600 + minute * 60 + second) * 1000
            + millis;
        Some(Timestamp {
            utc_millis: local - offset_minutes * 60_000,
        })
    }
}

/// A window of dates: the end is not before the start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DateWindow {
    start: CalendarDate,
    end: CalendarDate,
}

impl DateWindow {
    /// `None` when `end` is before `start`.
    pub fn new(start: CalendarDate, end: CalendarDate) -> Option<DateWindow> {
        (end >= start).then_some(DateWindow { start, end })
    }

    pub fn start(self) -> CalendarDate {
        self.start
    }

    pub fn end(self) -> CalendarDate {
        self.end
    }

    pub fn contains(self, date: CalendarDate) -> bool {
        self.start <= date && date <= self.end
    }
}

/// A time interval: the end is strictly after the start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimeInterval {
    start: Timestamp,
    end: Timestamp,
}

impl TimeInterval {
    /// `None` when `end` is not after `start`.
    pub fn new(start: Timestamp, end: Timestamp) -> Option<TimeInterval> {
        (end > start).then_some(TimeInterval { start, end })
    }

    pub fn start(self) -> Timestamp {
        self.start
    }

    pub fn end(self) -> Timestamp {
        self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(s: &str) -> Timestamp {
        Timestamp::parse(s).unwrap()
    }

    #[test]
    fn field_evaluation_a_date_must_exist() {
        assert!(CalendarDate::parse("2026-01-15").is_some());
        assert!(CalendarDate::parse("2026-02-31").is_none());
        assert!(CalendarDate::parse("2026-02-29").is_none());
        assert!(CalendarDate::parse("2024-02-29").is_some());
        assert!(CalendarDate::parse("2026-1-15").is_none());
        assert!(CalendarDate::parse("２026-01-15").is_none());
        assert_eq!(
            CalendarDate::parse("0999-03-01").unwrap().to_string(),
            "0999-03-01"
        );
    }

    #[test]
    fn field_evaluation_a_timestamp_keeps_sub_second_order_and_its_offset() {
        let base = ts("2026-01-01T00:00:00Z").utc_millis;
        assert_eq!(ts("2026-01-01T00:00:00.1Z").utc_millis, base + 100);
        assert_eq!(ts("2026-01-01T00:00:00.12Z").utc_millis, base + 120);
        assert_eq!(ts("2026-01-01T00:00:00.123456Z").utc_millis, base + 123);
        assert!(ts("2026-01-01T00:00:00.100Z") < ts("2026-01-01T00:00:00.200Z"));
        assert_eq!(
            ts("2026-01-01T02:00:00.100+02:00"),
            ts("2026-01-01T00:00:00.100z")
        );
        assert!(TimeInterval::new(
            ts("2026-01-01T02:00:00.100+02:00"),
            ts("2026-01-01T00:00:00.200Z")
        )
        .is_some());
        assert!(
            TimeInterval::new(ts("2026-01-01T00:00:00.5Z"), ts("2026-01-01T00:00:00.5Z")).is_none()
        );
    }

    #[test]
    fn field_evaluation_a_timestamp_rejects_what_date_parse_would_not_order() {
        for bad in [
            "2026-02-30T00:00:00Z",
            "2026-01-01T24:00:00Z",
            "2026-01-01T00:60:00Z",
            "2026-01-01T00:00:60Z",
            "2026-01-01T00:00:00+24:00",
            "2026-01-01T00:00:00-00:60",
            "2026-01-01T00:00:00.Z",
            "2026-01-01 00:00:00Z",
            "2026-01-01T00:00:00",
        ] {
            assert!(Timestamp::parse(bad).is_none(), "{bad}");
        }
        assert!(Timestamp::parse("2026-01-01T00:00:00+23:59").is_some());
    }

    #[test]
    fn field_evaluation_a_window_never_ends_before_it_starts() {
        let a = CalendarDate::parse("2026-01-01").unwrap();
        let b = CalendarDate::parse("2026-01-31").unwrap();
        assert!(DateWindow::new(a, a).is_some());
        assert!(DateWindow::new(b, a).is_none());
        assert!(DateWindow::new(a, b).unwrap().contains(b));
    }
}
