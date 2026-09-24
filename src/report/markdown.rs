// 2026-09-24: Markdown rendering, used both for `output_type: markdown` and
// for the job summary (which caps the table).

use super::Report;
use crate::scan::Finding;

pub const SUMMARY_ROW_CAP: usize = 500;

fn escape_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn counts_table(report: &Report) -> String {
    let c = &report.counts;
    let mut s = String::from("| classification | count |\n|---|---:|\n");
    for (name, n) in [
        ("flagged", c.flagged),
        ("stale", c.stale),
        ("undated", c.undated),
        ("malformed", c.malformed),
        ("future", c.future),
        ("expiring", c.expiring),
        ("escape", c.escape),
        ("exempt", c.exempt),
        ("fresh", c.fresh),
        ("comments", c.comments),
        ("files scanned", c.files_scanned),
        ("binary files skipped", c.files_skipped_binary),
    ] {
        s.push_str(&format!("| {name} | {n} |\n"));
    }
    s
}

fn findings_table(findings: &[Finding], cap: Option<usize>) -> String {
    if findings.is_empty() {
        return String::from("No findings.\n");
    }
    let mut s = String::from(
        "| location | classification | date | expires | note | comment |\n|---|---|---|---|---|---|\n",
    );
    let shown = cap.map_or(findings.len(), |c| c.min(findings.len()));
    for f in &findings[..shown] {
        let location = if f.end_line > f.line {
            format!("`{}:{}-{}`", f.path, f.line, f.end_line)
        } else {
            format!("`{}:{}`", f.path, f.line)
        };
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            location,
            f.classification.as_str(),
            f.date.as_deref().unwrap_or(""),
            f.expires.as_deref().unwrap_or(""),
            escape_cell(&f.reason),
            escape_cell(&f.text),
        ));
    }
    if shown < findings.len() {
        s.push_str(&format!(
            "\n{} more findings are in the report artifact.\n",
            findings.len() - shown
        ));
    }
    s
}

fn header(report: &Report) -> String {
    let warn = report
        .warn_within
        .as_deref()
        .map(|w| format!(", warn within `{w}`"))
        .unwrap_or_default();
    format!(
        "Reference date **{}** ({}), TTL `{}` (`{}`){}, scope `{}`.\n\n",
        report.reference_date,
        report.reference_source,
        report.comment_ttl,
        report.comment_ttl_iso,
        warn,
        report.scope
    )
}

pub fn to_markdown(report: &Report) -> String {
    format!(
        "# comment-ttl report\n\n{}{}\n## Findings ({})\n\n{}",
        header(report),
        counts_table(report),
        report.findings.len(),
        findings_table(&report.findings, None)
    )
}

/// 2026-09-24: The job summary: same content, findings capped, with a verdict
/// line first so the outcome is visible without scrolling.
pub fn to_summary(report: &Report, failed: bool) -> String {
    let verdict = if report.counts.flagged == 0 {
        "No stale, undated, malformed or future-dated comments.".to_string()
    } else if failed {
        format!(
            "**{} flagged comment(s)** — this check failed.",
            report.counts.flagged
        )
    } else {
        format!(
            "**{} flagged comment(s)** (advisory: `fail_on_flag` is false).",
            report.counts.flagged
        )
    };
    format!(
        "## comment-ttl\n\n{verdict}\n\n{}{}\n### Findings ({})\n\n{}",
        header(report),
        counts_table(report),
        report.findings.len(),
        findings_table(&report.findings, Some(SUMMARY_ROW_CAP))
    )
}
