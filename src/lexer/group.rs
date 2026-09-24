// 2026-09-24: Text normalisation and the grouping rule: consecutive own-line
// line comments with the same marker and the same indent are one comment.

use crate::comment::{Comment, CommentKind};

/// 2026-09-24: The marker run at the start of a raw line: the marker followed
/// by repeats of its last character (`///`, `####`, `----`) plus a `!` for
/// `//!` inner docs and `#!` shebangs. A shebang therefore never groups with
/// the comment on the line below it, so the shebang exemption cannot swallow
/// that comment.
pub fn marker_run<'a>(raw_line: &'a str, marker: &str) -> &'a str {
    let last = marker.as_bytes()[marker.len() - 1];
    let mut end = marker.len();
    let bytes = raw_line.as_bytes();
    while end < bytes.len() && bytes[end] == last {
        end += 1;
    }
    if (marker == "//" || marker == "#") && bytes.get(end) == Some(&b'!') {
        end += 1;
    }
    &raw_line[..end]
}

/// 2026-09-24: Every source line of a block comment with leading and trailing
/// whitespace removed, joined by `\n`.
pub fn normalise_raw(raw: &str) -> String {
    raw.lines().map(str::trim).collect::<Vec<_>>().join("\n")
}

/// 2026-09-24: The text inside a block comment. For `/* … */` blocks the
/// leading `*` of each line (and the extra `*` of `/**`) is stripped, so the
/// date of a JSDoc block is its first token.
pub fn block_text(inner: &str, strip_stars: bool) -> String {
    let mut lines: Vec<&str> = inner
        .lines()
        .enumerate()
        .map(|(i, l)| {
            let mut l = l.trim();
            if strip_stars {
                if i == 0 {
                    l = l.trim_start_matches('*').trim_start();
                } else if let Some(rest) = l.strip_prefix('*') {
                    l = rest.trim_start();
                }
            }
            l
        })
        .collect();
    while lines.first().is_some_and(|l| l.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

pub fn group(comments: Vec<Comment>) -> Vec<Comment> {
    let mut out: Vec<Comment> = Vec::with_capacity(comments.len());
    for c in comments {
        let joins = out.last().is_some_and(|prev| {
            prev.kind == CommentKind::Line
                && c.kind == CommentKind::Line
                && prev.own_line
                && c.own_line
                && prev.marker == c.marker
                && prev.indent == c.indent
                && c.line == prev.end_line + 1
        });
        if joins {
            let prev = out.last_mut().expect("checked above");
            prev.end_line = c.line;
            prev.raw.push('\n');
            prev.raw.push_str(&c.raw);
            prev.text.push('\n');
            prev.text.push_str(&c.text);
        } else {
            out.push(c);
        }
    }
    for c in &mut out {
        c.text = trim_blank_edges(&c.text);
    }
    out
}

fn trim_blank_edges(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let first = lines.iter().position(|l| !l.trim().is_empty());
    let last = lines.iter().rposition(|l| !l.trim().is_empty());
    match (first, last) {
        (Some(f), Some(l)) => lines[f..=l].join("\n"),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(n: usize, marker: &str, indent: &str, own: bool, text: &str) -> Comment {
        Comment {
            line: n,
            end_line: n,
            kind: CommentKind::Line,
            marker: marker.into(),
            own_line: own,
            indent: indent.into(),
            raw: format!("{marker} {text}"),
            text: text.into(),
        }
    }

    #[test]
    fn marker_runs() {
        assert_eq!(marker_run("/// doc", "//"), "///");
        assert_eq!(marker_run("//! inner", "//"), "//!");
        assert_eq!(marker_run("//////", "//"), "//////");
        assert_eq!(marker_run("// x", "//"), "//");
        assert_eq!(marker_run("#!/bin/sh", "#"), "#!");
        assert_eq!(marker_run("### h", "#"), "###");
        assert_eq!(marker_run("-- x", "--"), "--");
        assert_eq!(marker_run("---- x", "--"), "----");
    }

    #[test]
    fn block_text_strips_stars_only_for_star_blocks() {
        assert_eq!(
            block_text("* 2026-01-01: a\n * b\n ", true),
            "2026-01-01: a\nb"
        );
        assert_eq!(
            block_text(" 2026-01-01: a\n   b ", true),
            "2026-01-01: a\nb"
        );
        assert_eq!(block_text(" * kept\n * star ", false), "* kept\n* star");
        assert_eq!(block_text("\n\n", true), "");
    }

    #[test]
    fn grouping_requires_adjacency_indent_marker_and_own_line() {
        let grouped = group(vec![
            line(1, "//", "", true, "a"),
            line(2, "//", "", true, "b"),
            line(3, "///", "", true, "doc"),
            line(4, "///", "", true, "doc2"),
            line(6, "///", "", true, "gap"),
            line(7, "//", "  ", true, "indented"),
            line(8, "//", "", false, "trailing"),
            line(9, "//", "", false, "trailing2"),
        ]);
        let texts: Vec<&str> = grouped.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(
            texts,
            [
                "a\nb",
                "doc\ndoc2",
                "gap",
                "indented",
                "trailing",
                "trailing2"
            ]
        );
        assert_eq!((grouped[0].line, grouped[0].end_line), (1, 2));
        assert_eq!(grouped[0].raw, "// a\n// b");
    }

    #[test]
    fn blank_comment_lines_inside_a_group_are_kept_but_edges_are_trimmed() {
        let grouped = group(vec![
            line(1, "//", "", true, ""),
            line(2, "//", "", true, "2026-01-01: x"),
            line(3, "//", "", true, ""),
            line(4, "//", "", true, "y"),
            line(5, "//", "", true, ""),
        ]);
        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped[0].text, "2026-01-01: x\n\ny");
    }
}
