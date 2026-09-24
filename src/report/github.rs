// 2026-09-24: GitHub Actions wiring: workflow-command annotations (capped at
// ten per step by the runner, so the full list lives in the summary and the
// report) and the `GITHUB_OUTPUT` key/value lines.

use super::Report;
use crate::classify::Classification;

pub const ANNOTATION_CAP: usize = 10;

fn escape_data(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

fn escape_property(s: &str) -> String {
    escape_data(s).replace(':', "%3A").replace(',', "%2C")
}

/// 2026-09-24: Annotation lines for stdout. Errors (flagged) come before
/// warnings (expiring) so the cap always favours the failures.
pub fn annotations(report: &Report) -> Vec<String> {
    let mut lines = Vec::new();
    let ordered = report
        .findings
        .iter()
        .filter(|f| f.classification.is_flagged())
        .chain(
            report
                .findings
                .iter()
                .filter(|f| f.classification == Classification::Expiring),
        );
    let mut total = 0usize;
    for f in ordered {
        total += 1;
        if lines.len() >= ANNOTATION_CAP {
            continue;
        }
        let kind = if f.classification.is_flagged() {
            "error"
        } else {
            "warning"
        };
        lines.push(format!(
            "::{kind} file={},line={},endLine={},title={}::{}",
            escape_property(&f.path),
            f.line,
            f.end_line,
            escape_property(&format!("comment-ttl: {}", f.classification.as_str())),
            escape_data(&format!("{} — {}", f.reason, f.text)),
        ));
    }
    if total > ANNOTATION_CAP {
        lines.push(format!(
            "::notice title=comment-ttl::{} of {} findings annotated; the full list is in the job summary and the report artifact.",
            ANNOTATION_CAP, total
        ));
    }
    lines
}

pub fn outputs(report: &Report, report_path: &str) -> Vec<(String, String)> {
    let c = &report.counts;
    vec![
        ("flagged".into(), c.flagged.to_string()),
        ("stale".into(), c.stale.to_string()),
        ("undated".into(), c.undated.to_string()),
        ("malformed".into(), c.malformed.to_string()),
        ("future".into(), c.future.to_string()),
        ("exempt".into(), c.exempt.to_string()),
        ("escape".into(), c.escape.to_string()),
        ("expiring".into(), c.expiring.to_string()),
        ("fresh".into(), c.fresh.to_string()),
        ("comments".into(), c.comments.to_string()),
        ("files_scanned".into(), c.files_scanned.to_string()),
        (
            "files_skipped_binary".into(),
            c.files_skipped_binary.to_string(),
        ),
        ("report_path".into(), report_path.to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaping_follows_the_workflow_command_rules() {
        assert_eq!(escape_data("a%b\nc\rd"), "a%25b%0Ac%0Dd");
        assert_eq!(escape_property("x:y,z"), "x%3Ay%2Cz");
    }
}
