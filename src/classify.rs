// 2026-09-24: Classifies one comment against the reference date, the TTL,
// the exemptions and the baseline. Pure: no I/O.

use serde::{Deserialize, Serialize};
use time::Date;

use crate::baseline::Baseline;
use crate::calendar::{add_days, add_ttl, days_between, format_ymd, parse_ymd};
use crate::comment::Comment;
use crate::exemptions::Exemptions;
use crate::ttl::Ttl;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    Exempt,
    Escape,
    Undated,
    Malformed,
    Future,
    Stale,
    Expiring,
    Fresh,
}

impl Classification {
    pub fn is_flagged(self) -> bool {
        matches!(
            self,
            Classification::Undated
                | Classification::Malformed
                | Classification::Future
                | Classification::Stale
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Classification::Exempt => "exempt",
            Classification::Escape => "escape",
            Classification::Undated => "undated",
            Classification::Malformed => "malformed",
            Classification::Future => "future",
            Classification::Stale => "stale",
            Classification::Expiring => "expiring",
            Classification::Fresh => "fresh",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verdict {
    pub classification: Classification,
    /// 2026-09-24: Human explanation: the exemption rule name, the escape
    /// reason, the malformed detail, or the age/expiry summary.
    pub reason: String,
    pub date: Option<String>,
    pub expires: Option<String>,
    pub age_days: Option<i64>,
}

pub struct Rules<'a> {
    pub reference: Date,
    pub ttl: Ttl,
    pub warn_within: Option<Ttl>,
    pub exemptions: &'a Exemptions,
    pub baseline: Option<&'a Baseline>,
}

/// 2026-09-24: A date supplied from outside the comment (git blame) for the
/// `undated: blame` mode.
pub struct External {
    pub date: Date,
}

pub fn classify(
    path: &str,
    comment: &Comment,
    rules: &Rules<'_>,
    external: Option<&External>,
) -> Verdict {
    if let Some(rule) = rules.exemptions.matches(&comment.raw) {
        return simple(Classification::Exempt, format!("exemption rule `{rule}`"));
    }
    let verdict = escape_verdict(comment).unwrap_or_else(|| date_verdict(comment, rules, external));
    if verdict.classification.is_flagged()
        && rules
            .baseline
            .is_some_and(|b| b.contains(path, &comment.text))
    {
        return simple(
            Classification::Exempt,
            "grandfathered by baseline".to_string(),
        );
    }
    verdict
}

fn simple(classification: Classification, reason: String) -> Verdict {
    Verdict {
        classification,
        reason,
        date: None,
        expires: None,
        age_days: None,
    }
}

fn escape_verdict(comment: &Comment) -> Option<Verdict> {
    let text = comment.text.trim_start();
    let lower = text.to_ascii_lowercase();
    if !lower.starts_with("no-rot") {
        return None;
    }
    let rest = &text["no-rot".len()..];
    if rest.starts_with(|c: char| c.is_alphanumeric() || c == '-' || c == '_') {
        return None;
    }
    let reason = rest
        .trim_start()
        .strip_prefix(':')
        .map(str::trim)
        .filter(|r| !r.is_empty());
    Some(match reason {
        Some(r) => simple(Classification::Escape, r.replace('\n', " ")),
        None => simple(
            Classification::Malformed,
            "`no-rot` escape without a reason; write `no-rot: <reason>`".to_string(),
        ),
    })
}

fn date_verdict(comment: &Comment, rules: &Rules<'_>, external: Option<&External>) -> Verdict {
    let token = comment.first_token().unwrap_or("");
    let candidate = token.trim_end_matches([':', ',', '.', ';', '-']);
    let (date, source) = if looks_like_date(candidate) {
        match parse_ymd(candidate) {
            Ok(d) => (d, "comment"),
            Err(e) => return simple(Classification::Malformed, e.to_string()),
        }
    } else if let Some(ext) = external {
        (ext.date, "blame")
    } else {
        return simple(
            Classification::Undated,
            "no date at the start of the comment".to_string(),
        );
    };
    let Some(expires) = add_ttl(date, rules.ttl) else {
        return simple(
            Classification::Malformed,
            "date plus TTL is out of range".to_string(),
        );
    };
    let age = days_between(date, rules.reference);
    let with = |c: Classification, reason: String| Verdict {
        classification: c,
        reason,
        date: Some(format_ymd(date)),
        expires: Some(format_ymd(expires)),
        age_days: Some(age),
    };
    let tomorrow = add_days(rules.reference, 1).unwrap_or(rules.reference);
    if date > tomorrow {
        return with(
            Classification::Future,
            format!("dated {} days after the reference date", -age),
        );
    }
    if rules.reference > expires {
        let overdue = days_between(expires, rules.reference);
        return with(
            Classification::Stale,
            format!(
                "expired {} ({overdue} days ago; date from {source})",
                format_ymd(expires)
            ),
        );
    }
    if let Some(warn) = rules.warn_within {
        let horizon = add_ttl(rules.reference, warn).unwrap_or(expires);
        if horizon > expires {
            let left = days_between(rules.reference, expires);
            return with(
                Classification::Expiring,
                format!(
                    "expires {} (in {left} days; date from {source})",
                    format_ymd(expires)
                ),
            );
        }
    }
    with(
        Classification::Fresh,
        format!("expires {} (date from {source})", format_ymd(expires)),
    )
}

/// 2026-09-24: A token that is clearly meant to be a date (four digits, a
/// separator, one or two digits, a separator, one or two digits). Anything
/// looser is `undated`, anything in this shape but invalid is `malformed`.
fn looks_like_date(token: &str) -> bool {
    let digits = |p: Option<&str>, min: usize, max: usize| {
        p.is_some_and(|p| (min..=max).contains(&p.len()) && p.bytes().all(|c| c.is_ascii_digit()))
    };
    let mut parts = token.split(['-', '/', '.']);
    digits(parts.next(), 4, 4)
        && digits(parts.next(), 1, 2)
        && digits(parts.next(), 1, 2)
        && parts.next().is_none()
}
