// 2026-09-24: SARIF 2.1.0 rendering. One rule per classification; flagged
// findings are errors, expiring ones warnings, escapes notes.

use serde_json::{Value, json};

use super::{Report, TOOL_NAME, TOOL_VERSION};
use crate::classify::Classification;

const SARIF_SCHEMA: &str = "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json";

fn rule(c: Classification) -> Value {
    let (name, description) = match c {
        Classification::Stale => (
            "Stale comment",
            "The comment's verification date is older than the configured TTL.",
        ),
        Classification::Undated => (
            "Undated comment",
            "The comment does not start with a YYYY-MM-DD verification date.",
        ),
        Classification::Malformed => (
            "Malformed date or escape",
            "The comment starts with an invalid date, or uses `no-rot` without a reason.",
        ),
        Classification::Future => (
            "Future-dated comment",
            "The comment is dated more than one day after the reference date.",
        ),
        Classification::Expiring => (
            "Expiring comment",
            "The comment will pass its TTL within the warn_within window.",
        ),
        Classification::Escape => (
            "Escaped comment",
            "The comment opted out of dating with a `no-rot:` reason.",
        ),
        Classification::Exempt => ("Exempt comment", "The comment matched an exemption rule."),
        Classification::Fresh => ("Fresh comment", "The comment is within its TTL."),
    };
    json!({
        "id": c.as_str(),
        "name": name,
        "shortDescription": { "text": description },
        "defaultConfiguration": { "level": level(c) }
    })
}

fn level(c: Classification) -> &'static str {
    if c.is_flagged() {
        "error"
    } else if c == Classification::Expiring {
        "warning"
    } else {
        "note"
    }
}

pub fn to_sarif(report: &Report) -> String {
    let rules: Vec<Value> = [
        Classification::Stale,
        Classification::Undated,
        Classification::Malformed,
        Classification::Future,
        Classification::Expiring,
        Classification::Escape,
    ]
    .into_iter()
    .map(rule)
    .collect();
    let results: Vec<Value> = report
        .findings
        .iter()
        .map(|f| {
            json!({
                "ruleId": f.classification.as_str(),
                "level": level(f.classification),
                "message": { "text": format!("{}: {}", f.classification.as_str(), f.reason) },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": f.path, "uriBaseId": "%SRCROOT%" },
                        "region": { "startLine": f.line, "endLine": f.end_line }
                    }
                }],
                "partialFingerprints": { "commentTextHash/v1": f.fingerprint }
            })
        })
        .collect();
    let doc = json!({
        "$schema": SARIF_SCHEMA,
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": TOOL_NAME,
                    "version": TOOL_VERSION,
                    "informationUri": "https://github.com/Metrale/comment-ttl",
                    "rules": rules
                }
            },
            "originalUriBaseIds": { "%SRCROOT%": { "uri": "file:///" } },
            "results": results
        }]
    });
    serde_json::to_string_pretty(&doc).expect("sarif serialises") + "\n"
}
