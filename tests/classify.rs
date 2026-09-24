// 2026-09-24: Classification through the public API: source text is lexed
// with the real presets and judged with real rules, so every boundary
// (tomorrow, month clamp, warn window) is tested end to end.

use comment_ttl::baseline::{Baseline, BaselineEntry, fingerprint, render};
use comment_ttl::calendar::parse_ymd;
use comment_ttl::classify::{Classification, External, Rules, classify};
use comment_ttl::exemptions::{Exemptions, ExemptionsSpec};
use comment_ttl::lexer::{lex, preset};
use comment_ttl::ttl::Ttl;

fn verdicts(
    src: &str,
    lang: &str,
    reference: &str,
    ttl: &str,
    warn: Option<&str>,
) -> Vec<(Classification, String)> {
    let exemptions = Exemptions::new(&ExemptionsSpec::default()).unwrap();
    let rules = Rules {
        reference: parse_ymd(reference).unwrap(),
        ttl: Ttl::parse(ttl).unwrap(),
        warn_within: warn.map(|w| Ttl::parse(w).unwrap()),
        exemptions: &exemptions,
        baseline: None,
    };
    lex(src, &preset(lang).unwrap())
        .iter()
        .map(|c| {
            let v = classify("f", c, &rules, None);
            (v.classification, v.reason)
        })
        .collect()
}

fn kinds(
    src: &str,
    lang: &str,
    reference: &str,
    ttl: &str,
    warn: Option<&str>,
) -> Vec<Classification> {
    verdicts(src, lang, reference, ttl, warn)
        .into_iter()
        .map(|(c, _)| c)
        .collect()
}

#[test]
fn every_classification_is_reachable() {
    use Classification::*;
    let src = "\
// SPDX-License-Identifier: MIT
x();
// no-rot: fixed by the wire format
x();
// undated prose
x();
// 2026-02-30: malformed
x();
// 2026-10-01: future
x();
// 2026-07-01: stale
x();
// 2026-08-27: expiring
x();
// 2026-09-20: fresh
";
    assert_eq!(
        kinds(src, "c", "2026-09-24", "1 month", Some("7d")),
        [
            Exempt, Escape, Undated, Malformed, Future, Stale, Expiring, Fresh
        ]
    );
}

#[test]
fn the_date_must_be_the_first_token_and_may_carry_punctuation() {
    use Classification::*;
    let ok = [
        "// 2026-09-20: x",
        "// 2026-09-20 x",
        "//2026-09-20, x",
        "/* 2026-09-20 */",
        "/**\n * 2026-09-20: doc\n */",
    ];
    for src in ok {
        assert_eq!(
            kinds(src, "c", "2026-09-24", "1 month", None),
            [Fresh],
            "{src:?}"
        );
    }
    let undated = [
        "// see 2026-09-20",
        "// v2026-09-20",
        "// (2026-09-20)",
        "// 20260920",
        "// 2026",
    ];
    for src in undated {
        assert_eq!(
            kinds(src, "c", "2026-09-24", "1 month", None),
            [Undated],
            "{src:?}"
        );
    }
    let malformed = [
        "// 2026-9-4: x",
        "// 2026/09/04 x",
        "// 2026-13-01",
        "// 2023-02-29 leap",
    ];
    for src in malformed {
        assert_eq!(
            kinds(src, "c", "2026-09-24", "1 month", None),
            [Malformed],
            "{src:?}"
        );
    }
}

#[test]
fn escape_needs_a_reason_and_is_case_insensitive() {
    use Classification::*;
    assert_eq!(
        kinds("// No-Rot: because", "c", "2026-09-24", "1d", None),
        [Escape]
    );
    assert_eq!(
        verdicts(
            "// no-rot: multi\n// line reason",
            "c",
            "2026-09-24",
            "1d",
            None
        )[0]
        .1,
        "multi line reason"
    );
    assert_eq!(
        kinds("// no-rot", "c", "2026-09-24", "1d", None),
        [Malformed]
    );
    assert_eq!(
        kinds("// no-rot:", "c", "2026-09-24", "1d", None),
        [Malformed]
    );
    assert_eq!(
        kinds("// no-rot because", "c", "2026-09-24", "1d", None),
        [Malformed]
    );
    assert_eq!(
        kinds("// no-rotation handling", "c", "2026-09-24", "1d", None),
        [Undated]
    );
}

#[test]
fn boundaries_follow_reference_le_date_plus_ttl() {
    use Classification::*;
    let day = |reference: &str| kinds("// 2026-01-31: x", "c", reference, "1 month", None)[0];
    assert_eq!(
        day("2026-02-28"),
        Fresh,
        "clamped expiry day is still fresh"
    );
    assert_eq!(
        day("2026-03-01"),
        Stale,
        "the day after the clamped expiry is stale"
    );
    assert_eq!(day("2026-01-31"), Fresh);
    assert_eq!(day("2026-02-01"), Fresh, "dated yesterday");
    assert_eq!(day("2026-01-30"), Fresh, "dated tomorrow is allowed");
    assert_eq!(day("2026-01-29"), Future, "two days ahead is future");

    let warn =
        |reference: &str| kinds("// 2026-08-27: x", "c", reference, "1 month", Some("7d"))[0];
    assert_eq!(
        warn("2026-09-19"),
        Fresh,
        "expires 09-27; horizon 09-26 is not past it"
    );
    assert_eq!(
        warn("2026-09-20"),
        Fresh,
        "horizon 09-27 equals expiry, not past it"
    );
    assert_eq!(warn("2026-09-21"), Expiring, "horizon 09-28 is past expiry");
    assert_eq!(
        warn("2026-09-27"),
        Expiring,
        "last fresh day, inside the window"
    );
    assert_eq!(warn("2026-09-28"), Stale);
}

#[test]
fn reasons_carry_dates_and_ages() {
    let v = verdicts("// 2026-07-01: stale", "c", "2026-09-24", "1 month", None);
    assert_eq!(
        v[0].1,
        "expired 2026-08-01 (54 days ago; date from comment)"
    );
    let v = verdicts(
        "// 2026-08-27: soon",
        "c",
        "2026-09-24",
        "1 month",
        Some("7d"),
    );
    assert_eq!(v[0].1, "expires 2026-09-27 (in 3 days; date from comment)");
}

#[test]
fn external_dates_stand_in_for_missing_ones_but_never_override_written_ones() {
    let exemptions = Exemptions::new(&ExemptionsSpec::default()).unwrap();
    let rules = Rules {
        reference: parse_ymd("2026-09-24").unwrap(),
        ttl: Ttl::parse("1 month").unwrap(),
        warn_within: None,
        exemptions: &exemptions,
        baseline: None,
    };
    let comments = lex(
        "// undated\n\n// 2026-01-01: written",
        &preset("c").unwrap(),
    );
    let blame = External {
        date: parse_ymd("2026-09-01").unwrap(),
    };
    let a = classify("f", &comments[0], &rules, Some(&blame));
    assert_eq!(a.classification, Classification::Fresh);
    assert_eq!(a.reason, "expires 2026-10-01 (date from blame)");
    let b = classify("f", &comments[1], &rules, Some(&blame));
    assert_eq!(b.classification, Classification::Stale);
    assert!(b.reason.ends_with("date from comment)"));
}

#[test]
fn baseline_grandfathers_flagged_comments_by_path_and_text() {
    let exemptions = Exemptions::new(&ExemptionsSpec::default()).unwrap();
    let json = render(vec![BaselineEntry {
        path: "a.rs".into(),
        fingerprint: fingerprint("a.rs", "old  note"),
        line: None,
        excerpt: None,
    }]);
    let baseline = Baseline::parse(&json).unwrap();
    let rules = Rules {
        reference: parse_ymd("2026-09-24").unwrap(),
        ttl: Ttl::parse("1 month").unwrap(),
        warn_within: None,
        exemptions: &exemptions,
        baseline: Some(&baseline),
    };
    let c = &lex("// old note", &preset("c").unwrap())[0];
    assert_eq!(
        classify("a.rs", c, &rules, None).classification,
        Classification::Exempt
    );
    assert_eq!(
        classify("a.rs", c, &rules, None).reason,
        "grandfathered by baseline"
    );
    assert_eq!(
        classify("b.rs", c, &rules, None).classification,
        Classification::Undated
    );
    let changed = &lex("// old note edited", &preset("c").unwrap())[0];
    assert_eq!(
        classify("a.rs", changed, &rules, None).classification,
        Classification::Undated
    );
    let bare_escape = &lex("// no-rot", &preset("c").unwrap())[0];
    assert_eq!(
        classify("a.rs", bare_escape, &rules, None).classification,
        Classification::Malformed
    );
    let json = render(vec![BaselineEntry {
        path: "a.rs".into(),
        fingerprint: fingerprint("a.rs", "no-rot"),
        line: None,
        excerpt: None,
    }]);
    let escapes = Baseline::parse(&json).unwrap();
    let rules_with_escape = Rules {
        baseline: Some(&escapes),
        ..rules
    };
    assert_eq!(
        classify("a.rs", bare_escape, &rules_with_escape, None).classification,
        Classification::Exempt,
        "a reasonless escape is flagged, so a baseline must be able to grandfather it"
    );
    let fresh = &lex("// 2026-09-20: old note", &preset("c").unwrap())[0];
    assert_eq!(
        classify("a.rs", fresh, &rules, None).classification,
        Classification::Fresh,
        "baseline never masks a real date"
    );
}
