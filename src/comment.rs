// 2026-09-24: The comment record produced by the lexer and consumed by the
// classifier. Line numbers are 1-based and inclusive.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommentKind {
    Line,
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Comment {
    pub line: usize,
    pub end_line: usize,
    pub kind: CommentKind,
    /// 2026-09-24: The marker run that opened the comment, e.g. `//`, `///`,
    /// `//!`, `#`, `/*`, `<!--`. Line comments only group with an identical
    /// marker so a `//` note never merges into the `///` doc block below it.
    pub marker: String,
    /// 2026-09-24: True when nothing but whitespace precedes the marker on its
    /// line. Trailing comments (`x = 1; // note`) never group.
    pub own_line: bool,
    /// 2026-09-24: The exact whitespace before the marker (empty for trailing
    /// comments). Grouping requires an identical indent.
    pub indent: String,
    /// 2026-09-24: The comment as written, markers included, one entry per
    /// source line with leading and trailing whitespace removed. Exemption
    /// regexes run against these lines joined by `\n`.
    pub raw: String,
    /// 2026-09-24: The comment with markers stripped and each line trimmed,
    /// joined by `\n`. The date must be the first token of this text.
    pub text: String,
}

impl Comment {
    pub fn first_token(&self) -> Option<&str> {
        self.text.split_whitespace().next()
    }

    /// 2026-09-24: A one-line excerpt for reports and annotations.
    pub fn excerpt(&self, max_chars: usize) -> String {
        let first = self
            .text
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("");
        let mut out: String = first.chars().take(max_chars).collect();
        if first.chars().count() > max_chars || self.text.lines().count() > 1 {
            out.push('…');
        }
        out
    }
}
