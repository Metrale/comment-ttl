// 2026-09-24: The table-driven description of one comment syntax. Presets for
// the shipped languages live in `presets.rs`; users can also supply an inline
// syntax through `comment_formats`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineMarker {
    pub text: String,
    /// 2026-09-24: The marker only opens a comment at line start or after
    /// whitespace. `#` needs this so `url#frag`, `$#` and `${#x}` are code.
    pub needs_ws_before: bool,
    /// 2026-09-24: A character that neutralises the marker, e.g. `\%` in TeX.
    pub escape: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockMarker {
    pub open: String,
    pub close: String,
    /// 2026-09-24: Rust and Haskell nest block comments.
    pub nested: bool,
    /// 2026-09-24: Only valid at the start of a line (Ruby `=begin`).
    pub line_start_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringRule {
    pub open: String,
    pub close: String,
    pub escape: Option<u8>,
    /// 2026-09-24: A doubled close quote is an escaped quote (`'it''s'`).
    pub doubled: bool,
    /// 2026-09-24: `${ ... }` inside the string re-enters code (JS templates).
    pub template: bool,
}

/// 2026-09-24: An embedded language region for mixed files: everything
/// between `<tag ...>` and `</tag>` is lexed with `syntax`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    pub tag: String,
    pub syntax: Box<Syntax>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Syntax {
    pub name: String,
    pub line_markers: Vec<LineMarker>,
    pub block_markers: Vec<BlockMarker>,
    pub strings: Vec<StringRule>,
    pub regions: Vec<Region>,
    /// 2026-09-24: JavaScript regex literals (`/a\/\/b/`) are skipped using
    /// the previous-token heuristic.
    pub regex_literals: bool,
    /// 2026-09-24: Rust raw strings (`r#"…"#`) and the char-vs-lifetime rule.
    pub rust_literals: bool,
}

/// 2026-09-24: The user-facing shape of an inline syntax in `comment_formats`.
/// Every field is optional so `{ line: ["//"] }` is a complete definition.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InlineSyntax {
    #[serde(default)]
    pub line: Vec<String>,
    #[serde(default)]
    pub block: Vec<[String; 2]>,
    #[serde(default)]
    pub strings: Vec<String>,
    #[serde(default)]
    pub line_needs_whitespace: bool,
    #[serde(default)]
    pub nested_blocks: bool,
}

impl InlineSyntax {
    pub fn into_syntax(self, name: &str) -> Result<Syntax, String> {
        if self.line.is_empty() && self.block.is_empty() {
            return Err(format!(
                "comment format {name:?} defines neither `line` nor `block` markers"
            ));
        }
        for m in self
            .line
            .iter()
            .chain(self.block.iter().flatten())
            .chain(&self.strings)
        {
            if m.is_empty() || !m.is_ascii() {
                return Err(format!(
                    "comment format {name:?}: marker {m:?} must be non-empty ASCII"
                ));
            }
        }
        Ok(Syntax {
            name: name.to_string(),
            line_markers: self
                .line
                .into_iter()
                .map(|text| LineMarker {
                    text,
                    needs_ws_before: self.line_needs_whitespace,
                    escape: None,
                })
                .collect(),
            block_markers: self
                .block
                .into_iter()
                .map(|[open, close]| BlockMarker {
                    open,
                    close,
                    nested: self.nested_blocks,
                    line_start_only: false,
                })
                .collect(),
            strings: self
                .strings
                .into_iter()
                .map(|q| StringRule {
                    close: q.clone(),
                    open: q,
                    escape: Some(b'\\'),
                    doubled: false,
                    template: false,
                })
                .collect(),
            ..Syntax::default()
        })
    }
}

pub fn line(text: &str) -> LineMarker {
    LineMarker {
        text: text.to_string(),
        needs_ws_before: false,
        escape: None,
    }
}

pub fn line_ws(text: &str) -> LineMarker {
    LineMarker {
        needs_ws_before: true,
        ..line(text)
    }
}

pub fn block(open: &str, close: &str) -> BlockMarker {
    BlockMarker {
        open: open.to_string(),
        close: close.to_string(),
        nested: false,
        line_start_only: false,
    }
}

pub fn quote(q: &str) -> StringRule {
    StringRule {
        open: q.to_string(),
        close: q.to_string(),
        escape: Some(b'\\'),
        doubled: false,
        template: false,
    }
}

pub fn quote_no_escape(q: &str) -> StringRule {
    StringRule {
        escape: None,
        ..quote(q)
    }
}
