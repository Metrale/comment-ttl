// 2026-09-24: Calendar arithmetic in UTC. Months clamp to the end of the
// target month (2026-01-31 + 1 month = 2026-02-28); weeks and days are exact.

use std::fmt;

use time::{Date, Month, OffsetDateTime, UtcOffset};

use crate::ttl::Ttl;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateError(pub String);

impl fmt::Display for DateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DateError {}

/// 2026-09-24: Parses a strict `YYYY-MM-DD`; `2026-02-30` and `2026-9-4` are
/// both rejected so a comment date is either exact or reported as malformed.
pub fn parse_ymd(s: &str) -> Result<Date, DateError> {
    let bytes = s.as_bytes();
    let shape_ok = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit());
    if !shape_ok {
        return Err(DateError(format!("{s:?} is not of the form YYYY-MM-DD")));
    }
    let year: i32 = s[0..4]
        .parse()
        .map_err(|_| DateError(format!("bad year in {s:?}")))?;
    let month: u8 = s[5..7]
        .parse()
        .map_err(|_| DateError(format!("bad month in {s:?}")))?;
    let day: u8 = s[8..10]
        .parse()
        .map_err(|_| DateError(format!("bad day in {s:?}")))?;
    let month =
        Month::try_from(month).map_err(|_| DateError(format!("{s:?}: month out of range")))?;
    Date::from_calendar_date(year, month, day)
        .map_err(|_| DateError(format!("{s:?}: day out of range for that month")))
}

pub fn format_ymd(d: Date) -> String {
    format!("{:04}-{:02}-{:02}", d.year(), u8::from(d.month()), d.day())
}

/// 2026-09-24: Adds whole months with end-of-month clamping.
pub fn add_months(d: Date, months: u32) -> Option<Date> {
    let total = i64::from(u8::from(d.month())) - 1 + i64::from(months);
    let year = i64::from(d.year()) + total.div_euclid(12);
    let month = Month::try_from(u8::try_from(total.rem_euclid(12) + 1).ok()?).ok()?;
    let year = i32::try_from(year).ok()?;
    let last = time::util::days_in_month(month, year);
    Date::from_calendar_date(year, month, d.day().min(last)).ok()
}

pub fn add_days(d: Date, days: i64) -> Option<Date> {
    d.checked_add(time::Duration::days(days))
}

/// 2026-09-24: `date + ttl`: years and months first (one clamped step), then
/// weeks and days exactly. Returns `None` only if the result leaves the
/// supported year range.
pub fn add_ttl(d: Date, ttl: Ttl) -> Option<Date> {
    let months = ttl.years.checked_mul(12)?.checked_add(ttl.months)?;
    let d = add_months(d, months)?;
    let days = i64::from(ttl.weeks) * 7 + i64::from(ttl.days);
    add_days(d, days)
}

/// 2026-09-24: Whole days from `from` to `to` (negative when `to` is earlier).
pub fn days_between(from: Date, to: Date) -> i64 {
    i64::from(to.to_julian_day()) - i64::from(from.to_julian_day())
}

pub fn today_utc() -> Date {
    OffsetDateTime::now_utc().date()
}

/// 2026-09-24: The UTC calendar date of a unix timestamp, used for the
/// `reference_date: commit` mode.
pub fn date_of_unix(ts: i64) -> Result<Date, DateError> {
    OffsetDateTime::from_unix_timestamp(ts)
        .map(|t| t.to_offset(UtcOffset::UTC).date())
        .map_err(|_| DateError(format!("unix timestamp {ts} is out of range")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn d(s: &str) -> Date {
        parse_ymd(s).unwrap()
    }

    fn ttl(y: u32, mo: u32, w: u32, d: u32) -> Ttl {
        Ttl {
            years: y,
            months: mo,
            weeks: w,
            days: d,
        }
    }

    #[test]
    fn month_clamp_matches_the_spec_example() {
        assert_eq!(add_months(d("2026-01-31"), 1).unwrap(), d("2026-02-28"));
        assert_eq!(add_months(d("2024-01-31"), 1).unwrap(), d("2024-02-29"));
        assert_eq!(add_months(d("2026-01-31"), 2).unwrap(), d("2026-03-31"));
        assert_eq!(add_months(d("2026-03-31"), 1).unwrap(), d("2026-04-30"));
        assert_eq!(add_months(d("2025-12-15"), 1).unwrap(), d("2026-01-15"));
        assert_eq!(add_months(d("2025-11-30"), 15).unwrap(), d("2027-02-28"));
    }

    #[test]
    fn add_ttl_applies_months_then_days() {
        assert_eq!(
            add_ttl(d("2026-01-31"), ttl(0, 1, 0, 1)).unwrap(),
            d("2026-03-01")
        );
        assert_eq!(
            add_ttl(d("2026-01-01"), ttl(1, 2, 3, 4)).unwrap(),
            d("2027-03-26")
        );
        assert_eq!(
            add_ttl(d("2026-09-24"), ttl(0, 0, 0, 30)).unwrap(),
            d("2026-10-24")
        );
        assert_eq!(
            add_ttl(d("2024-02-29"), ttl(1, 0, 0, 0)).unwrap(),
            d("2025-02-28")
        );
    }

    #[test]
    fn strict_ymd_parsing() {
        assert!(parse_ymd("2026-02-30").is_err());
        assert!(parse_ymd("2026-13-01").is_err());
        assert!(parse_ymd("2026-00-10").is_err());
        assert!(parse_ymd("2026-9-4").is_err());
        assert!(parse_ymd("2026/09/04").is_err());
        assert!(parse_ymd("20260904").is_err());
        assert!(parse_ymd("2026-09-04x").is_err());
        assert_eq!(format_ymd(parse_ymd("2024-02-29").unwrap()), "2024-02-29");
        assert!(parse_ymd("2023-02-29").is_err());
    }

    #[test]
    fn days_between_and_unix() {
        assert_eq!(days_between(d("2026-01-31"), d("2026-02-01")), 1);
        assert_eq!(days_between(d("2026-02-01"), d("2026-01-31")), -1);
        assert_eq!(date_of_unix(1_790_000_000).unwrap(), d("2026-09-21"));
        assert_eq!(date_of_unix(0).unwrap(), d("1970-01-01"));
        assert_eq!(date_of_unix(86_399).unwrap(), d("1970-01-01"));
        assert_eq!(date_of_unix(86_400).unwrap(), d("1970-01-02"));
    }

    fn arb_date() -> impl Strategy<Value = Date> {
        (1970i32..2400, 1u8..=12, 1u8..=31).prop_filter_map("valid date", |(y, m, day)| {
            Date::from_calendar_date(y, Month::try_from(m).unwrap(), day).ok()
        })
    }

    proptest! {
        #[test]
        fn month_addition_never_overshoots_and_keeps_day_when_possible(base in arb_date(), n in 0u32..600) {
            let out = add_months(base, n).unwrap();
            let expected_index = i64::from(u8::from(base.month())) - 1 + i64::from(n);
            prop_assert_eq!(i64::from(out.year()), i64::from(base.year()) + expected_index.div_euclid(12));
            prop_assert_eq!(i64::from(u8::from(out.month())), expected_index.rem_euclid(12) + 1);
            let last = time::util::days_in_month(out.month(), out.year());
            prop_assert_eq!(out.day(), base.day().min(last));
            prop_assert!(out >= base);
        }

        #[test]
        fn month_addition_is_monotone_in_the_base_date(a in arb_date(), b in arb_date(), n in 0u32..120) {
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            prop_assert!(add_months(lo, n).unwrap() <= add_months(hi, n).unwrap());
        }

        #[test]
        fn twelve_months_equals_one_year(base in arb_date(), years in 0u32..30) {
            let by_months = add_months(base, years * 12).unwrap();
            let by_years = add_ttl(base, ttl(years, 0, 0, 0)).unwrap();
            prop_assert_eq!(by_months, by_years);
            prop_assert_eq!(by_years.month(), base.month());
        }

        #[test]
        fn weeks_and_days_are_exact(base in arb_date(), w in 0u32..200, dd in 0u32..2000) {
            let out = add_ttl(base, ttl(0, 0, w, dd)).unwrap();
            prop_assert_eq!(days_between(base, out), i64::from(w) * 7 + i64::from(dd));
        }

        #[test]
        fn ymd_round_trips(base in arb_date()) {
            prop_assert_eq!(parse_ymd(&format_ymd(base)).unwrap(), base);
        }

        #[test]
        fn freshness_is_monotone_in_the_reference(base in arb_date(), n in 1u32..48, k in 0i64..2000) {
            let expires = add_ttl(base, ttl(0, n, 0, 0)).unwrap();
            let reference = add_days(base, k).unwrap();
            let fresh = reference <= expires;
            let later = add_days(reference, 1).unwrap();
            if !fresh {
                prop_assert!(later > expires);
            }
        }
    }
}
