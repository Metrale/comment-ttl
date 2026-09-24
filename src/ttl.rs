// 2026-09-24: The `comment_ttl` / `warn_within` duration grammar.
//
// Accepted forms (case-insensitive):
//   * a list of `<int><unit>` terms separated by spaces or commas, where
//     unit is d|day|days|w|wk|week|weeks|mo|mon|month|months|y|yr|year|years;
//   * an ISO-8601 date duration `P[nY][nM][nW][nD]` with no time part.
// Rejected: bare `m`, hours, minutes, seconds, zero, negatives, empty input.

use std::fmt;

use serde::{Deserialize, Serialize};

/// 2026-09-24: A calendar duration. Every field is non-negative and at least
/// one of them is non-zero once the value has been produced by [`Ttl::parse`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub struct Ttl {
    pub years: u32,
    pub months: u32,
    pub weeks: u32,
    pub days: u32,
}

/// 2026-09-24: The list of accepted forms, shown verbatim in every parse error
/// so a misconfigured workflow explains itself.
pub const VALID_FORMS: &str = "valid forms: `<int><unit>` terms separated by spaces or commas with \
unit d|day(s)|w|wk|week(s)|mo|mon|month(s)|y|yr|year(s) (for example `1 month`, `1mo`, `30d`, \
`2w`, `1y 2mo`), or an ISO-8601 date duration such as `P1M` or `P1Y2M3W4D` (no time part). \
Bare `m`, hours, minutes, seconds, zero, negative and empty values are rejected";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtlError {
    pub input: String,
    pub problem: String,
}

impl fmt::Display for TtlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid duration {:?}: {}; {}",
            self.input, self.problem, VALID_FORMS
        )
    }
}

impl std::error::Error for TtlError {}

impl Ttl {
    pub fn parse(input: &str) -> Result<Ttl, TtlError> {
        let err = |problem: &str| TtlError {
            input: input.to_string(),
            problem: problem.to_string(),
        };
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err(err("empty value"));
        }
        let lower = trimmed.to_ascii_lowercase();
        let ttl = if let Some(iso) = lower.strip_prefix('p') {
            parse_iso(iso).map_err(|p| err(&p))?
        } else {
            parse_terms(&lower).map_err(|p| err(&p))?
        };
        if ttl.is_zero() {
            return Err(err("duration is zero"));
        }
        Ok(ttl)
    }

    pub fn is_zero(&self) -> bool {
        self.years == 0 && self.months == 0 && self.weeks == 0 && self.days == 0
    }

    /// 2026-09-24: Canonical ISO-8601 rendering, used in reports so that
    /// `1 month` and `P1M` print identically.
    pub fn to_iso(&self) -> String {
        let mut out = String::from("P");
        if self.years > 0 {
            out.push_str(&format!("{}Y", self.years));
        }
        if self.months > 0 {
            out.push_str(&format!("{}M", self.months));
        }
        if self.weeks > 0 {
            out.push_str(&format!("{}W", self.weeks));
        }
        if self.days > 0 {
            out.push_str(&format!("{}D", self.days));
        }
        out
    }
}

impl fmt::Display for Ttl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_iso())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Unit {
    Day,
    Week,
    Month,
    Year,
}

fn unit_from_word(word: &str) -> Result<Unit, String> {
    match word {
        "d" | "day" | "days" => Ok(Unit::Day),
        "w" | "wk" | "wks" | "week" | "weeks" => Ok(Unit::Week),
        "mo" | "mon" | "month" | "months" => Ok(Unit::Month),
        "y" | "yr" | "yrs" | "year" | "years" => Ok(Unit::Year),
        "m" => Err("unit `m` is ambiguous; write `mo` for months".to_string()),
        "h" | "hr" | "hrs" | "hour" | "hours" | "min" | "mins" | "minute" | "minutes" | "s"
        | "sec" | "secs" | "second" | "seconds" | "ms" => {
            Err(format!("unit `{word}` is finer than a day and not allowed"))
        }
        other => Err(format!("unknown unit `{other}`")),
    }
}

fn add_unit(ttl: &mut Ttl, unit: Unit, n: u32) -> Result<(), String> {
    let slot = match unit {
        Unit::Day => &mut ttl.days,
        Unit::Week => &mut ttl.weeks,
        Unit::Month => &mut ttl.months,
        Unit::Year => &mut ttl.years,
    };
    *slot = slot
        .checked_add(n)
        .ok_or_else(|| "duration is too large".to_string())?;
    Ok(())
}

fn parse_count(digits: &str) -> Result<u32, String> {
    if digits.is_empty() {
        return Err("a term must start with a whole number".to_string());
    }
    let n: u32 = digits
        .parse()
        .map_err(|_| format!("`{digits}` is not a whole number in range"))?;
    if n == 0 {
        return Err("a term of zero is not allowed".to_string());
    }
    Ok(n)
}

/// 2026-09-24: `1y 2mo`, `30d`, `2 weeks`, `1w,2d`. A term may also be split
/// as `<int> <unit>` so `1 month` reads naturally.
fn parse_terms(input: &str) -> Result<Ttl, String> {
    let mut ttl = Ttl::default();
    let mut pending: Option<u32> = None;
    for word in input.split([' ', ',', '\t']).filter(|w| !w.is_empty()) {
        if word.starts_with('-') || word.starts_with('+') {
            return Err("signed numbers are not allowed".to_string());
        }
        let split = word
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(word.len());
        let (digits, unit) = word.split_at(split);
        match (digits.is_empty(), unit.is_empty(), pending) {
            (false, true, None) => pending = Some(parse_count(digits)?),
            (false, true, Some(_)) => {
                return Err(format!("number `{digits}` is missing its unit"));
            }
            (false, false, None) => {
                add_unit(&mut ttl, unit_from_word(unit)?, parse_count(digits)?)?
            }
            (false, false, Some(_)) => {
                return Err("number is missing its unit".to_string());
            }
            (true, false, Some(n)) => {
                add_unit(&mut ttl, unit_from_word(unit)?, n)?;
                pending = None;
            }
            (true, false, None) => return Err(format!("unit `{unit}` has no number before it")),
            (true, true, _) => unreachable!("empty words are filtered out"),
        }
    }
    if pending.is_some() {
        return Err("trailing number is missing its unit".to_string());
    }
    Ok(ttl)
}

/// 2026-09-24: The part after the leading `P`. Designators must appear in
/// Y, M, W, D order at most once each; `T` (a time part) is rejected.
fn parse_iso(body: &str) -> Result<Ttl, String> {
    if body.is_empty() {
        return Err("ISO duration has no designators".to_string());
    }
    if body.contains('t') {
        return Err("ISO durations with a time part are not allowed".to_string());
    }
    let mut ttl = Ttl::default();
    let mut digits = String::new();
    let mut last_rank = 0u8;
    for c in body.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
            continue;
        }
        let (rank, unit) = match c {
            'y' => (1, Unit::Year),
            'm' => (2, Unit::Month),
            'w' => (3, Unit::Week),
            'd' => (4, Unit::Day),
            other => return Err(format!("unexpected character `{other}` in ISO duration")),
        };
        if rank <= last_rank {
            return Err("ISO designators must be in Y, M, W, D order without repeats".to_string());
        }
        last_rank = rank;
        add_unit(&mut ttl, unit, parse_count(&digits)?)?;
        digits.clear();
    }
    if !digits.is_empty() {
        return Err("ISO duration ends with a number that has no designator".to_string());
    }
    Ok(ttl)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ttl(y: u32, mo: u32, w: u32, d: u32) -> Ttl {
        Ttl {
            years: y,
            months: mo,
            weeks: w,
            days: d,
        }
    }

    #[test]
    fn accepts_every_documented_example() {
        assert_eq!(Ttl::parse("1 month").unwrap(), ttl(0, 1, 0, 0));
        assert_eq!(Ttl::parse("1mo").unwrap(), ttl(0, 1, 0, 0));
        assert_eq!(Ttl::parse("30d").unwrap(), ttl(0, 0, 0, 30));
        assert_eq!(Ttl::parse("2w").unwrap(), ttl(0, 0, 2, 0));
        assert_eq!(Ttl::parse("1y 2mo").unwrap(), ttl(1, 2, 0, 0));
        assert_eq!(Ttl::parse("P1M").unwrap(), ttl(0, 1, 0, 0));
        assert_eq!(Ttl::parse("P1Y2M3W4D").unwrap(), ttl(1, 2, 3, 4));
    }

    #[test]
    fn separators_case_and_long_units() {
        assert_eq!(Ttl::parse("1W,2D").unwrap(), ttl(0, 0, 1, 2));
        assert_eq!(Ttl::parse(" 3 Weeks , 4 Days ").unwrap(), ttl(0, 0, 3, 4));
        assert_eq!(Ttl::parse("2yr 1mon").unwrap(), ttl(2, 1, 0, 0));
        assert_eq!(Ttl::parse("p2w").unwrap(), ttl(0, 0, 2, 0));
        assert_eq!(Ttl::parse("1d 1d").unwrap(), ttl(0, 0, 0, 2));
    }

    #[test]
    fn rejects_each_forbidden_form_with_the_valid_forms_listed() {
        for bad in [
            "",
            "   ",
            "m",
            "1m",
            "P1DT1H",
            "2h",
            "30min",
            "0d",
            "P0D",
            "-1d",
            "1 -1d",
            "1",
            "1 2d",
            "d",
            "1 month 2",
            "P",
            "PT",
            "P1D1W",
            "P1M1M",
            "1x",
            "1 fortnight",
            "1.5d",
            "+1d",
            "P1.5D",
        ] {
            let err = Ttl::parse(bad).unwrap_err();
            assert!(
                err.to_string().contains("valid forms:"),
                "{bad:?} error did not list valid forms: {err}"
            );
        }
        assert!(Ttl::parse("1m").unwrap_err().problem.contains("ambiguous"));
        assert!(
            Ttl::parse("2h")
                .unwrap_err()
                .problem
                .contains("finer than a day")
        );
    }

    #[test]
    fn iso_rendering_round_trips() {
        for s in ["P1M", "P1Y2M3W4D", "P30D", "P2W"] {
            assert_eq!(Ttl::parse(s).unwrap().to_iso(), s);
        }
        assert_eq!(Ttl::parse("1y 2mo").unwrap().to_iso(), "P1Y2M");
    }

    #[test]
    fn overflow_is_an_error_not_a_panic() {
        assert!(Ttl::parse("4294967295d 1d").is_err());
        assert!(Ttl::parse("99999999999d").is_err());
    }
}
