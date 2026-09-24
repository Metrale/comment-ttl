// 2026-09-24: The report document and its JSON/CSV renderings. SARIF and
// Markdown live in sibling modules; GitHub-specific output in `github.rs`.

pub mod github;
pub mod markdown;
pub mod sarif;

use serde::{Deserialize, Serialize};

use crate::scan::{Counts, Finding};

pub const TOOL_NAME: &str = "comment-ttl";
pub const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    pub tool: String,
    pub version: String,
    pub generated_at: String,
    pub reference_date: String,
    pub reference_source: String,
    pub comment_ttl: String,
    pub comment_ttl_iso: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warn_within: Option<String>,
    pub scope: String,
    pub counts: Counts,
    pub findings: Vec<Finding>,
}

impl Report {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("report serialises") + "\n"
    }

    pub fn to_csv(&self) -> String {
        let mut out =
            String::from("path,line,end_line,classification,date,expires,age_days,reason,text\n");
        for f in &self.findings {
            let row = [
                f.path.clone(),
                f.line.to_string(),
                f.end_line.to_string(),
                f.classification.as_str().to_string(),
                f.date.clone().unwrap_or_default(),
                f.expires.clone().unwrap_or_default(),
                f.age_days.map(|a| a.to_string()).unwrap_or_default(),
                f.reason.clone(),
                f.text.clone(),
            ];
            let cells: Vec<String> = row.iter().map(|c| csv_cell(c)).collect();
            out.push_str(&cells.join(","));
            out.push('\n');
        }
        out
    }
}

fn csv_cell(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_cells_are_quoted_only_when_needed() {
        assert_eq!(csv_cell("plain"), "plain");
        assert_eq!(csv_cell("a,b"), "\"a,b\"");
        assert_eq!(csv_cell("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_cell("two\nlines"), "\"two\nlines\"");
    }
}
