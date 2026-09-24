// 2026-09-24: The pure scan pipeline: file text in, classified comments out.
// Reading files and talking to git happens in `run.rs`.

use serde::{Deserialize, Serialize};

use crate::baseline::fingerprint;
use crate::classify::{Classification, External, Rules, Verdict, classify};
use crate::comment::Comment;
use crate::config::Config;
use crate::lexer;

pub const EXCERPT_CHARS: usize = 120;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub path: String,
    pub line: usize,
    pub end_line: usize,
    pub classification: Classification,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age_days: Option<i64>,
    pub text: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
    pub files_scanned: usize,
    pub files_skipped_binary: usize,
    pub comments: usize,
    pub flagged: usize,
    pub stale: usize,
    pub undated: usize,
    pub malformed: usize,
    pub future: usize,
    pub exempt: usize,
    pub escape: usize,
    pub expiring: usize,
    pub fresh: usize,
}

impl Counts {
    pub fn record(&mut self, c: Classification) {
        self.comments += 1;
        if c.is_flagged() {
            self.flagged += 1;
        }
        let slot = match c {
            Classification::Exempt => &mut self.exempt,
            Classification::Escape => &mut self.escape,
            Classification::Undated => &mut self.undated,
            Classification::Malformed => &mut self.malformed,
            Classification::Future => &mut self.future,
            Classification::Stale => &mut self.stale,
            Classification::Expiring => &mut self.expiring,
            Classification::Fresh => &mut self.fresh,
        };
        *slot += 1;
    }
}

/// 2026-09-24: A comment with its verdict, before it is folded into counts.
pub struct Classified {
    pub comment: Comment,
    pub verdict: Verdict,
}

/// 2026-09-24: True when the file should be skipped as binary: a NUL byte in
/// the first 8 KiB.
pub fn is_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8192).any(|&b| b == 0)
}

/// 2026-09-24: Lexes and classifies one file. `ranges` restricts the result
/// to comments overlapping the given inclusive line ranges (`scope: changed`).
pub fn analyze_file(
    path: &str,
    content: &str,
    cfg: &Config,
    rules: &Rules<'_>,
    ranges: Option<&[(usize, usize)]>,
) -> Option<Vec<Classified>> {
    let syntax = cfg.formats.for_path(path)?;
    let comments = lexer::lex(content, syntax);
    Some(
        comments
            .into_iter()
            .filter(|c| {
                ranges.is_none_or(|rs| rs.iter().any(|&(s, e)| c.line <= e && c.end_line >= s))
            })
            .map(|comment| {
                let verdict = classify(path, &comment, rules, None);
                Classified { comment, verdict }
            })
            .collect(),
    )
}

/// 2026-09-24: Re-runs classification for one comment with a blame-derived
/// date (the `undated: blame` mode).
pub fn reclassify_with_date(
    path: &str,
    item: &mut Classified,
    rules: &Rules<'_>,
    date: time::Date,
) {
    item.verdict = classify(path, &item.comment, rules, Some(&External { date }));
}

pub fn finding(path: &str, item: &Classified) -> Finding {
    Finding {
        path: path.to_string(),
        line: item.comment.line,
        end_line: item.comment.end_line,
        classification: item.verdict.classification,
        reason: item.verdict.reason.clone(),
        date: item.verdict.date.clone(),
        expires: item.verdict.expires.clone(),
        age_days: item.verdict.age_days,
        text: item.comment.excerpt(EXCERPT_CHARS),
        fingerprint: fingerprint(path, &item.comment.text),
    }
}

/// 2026-09-24: Which classifications appear in the findings list. Fresh and
/// exempt comments are counted but not listed, keeping reports proportional
/// to the work they ask for.
pub fn is_reported(c: Classification) -> bool {
    !matches!(c, Classification::Fresh | Classification::Exempt)
}
