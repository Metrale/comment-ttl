// 2026-09-24: The scanning loop. It walks the source byte by byte (every
// marker is ASCII, so byte positions found by marker matches are always char
// boundaries), skips string and template literals, and emits raw comments.

use super::literals;
use super::syntax::{BlockMarker, LineMarker, Syntax};
use crate::comment::{Comment, CommentKind};

pub struct Lexer<'a> {
    pub src: &'a str,
    pub bytes: &'a [u8],
    pub pos: usize,
    pub line: usize,
    pub line_start: usize,
    pub out: Vec<Comment>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Stop<'s> {
    Eof,
    /// 2026-09-24: Stop at the `}` that closes a template `${` expression.
    CloseBrace,
    /// 2026-09-24: Stop before `</tag` (case-insensitive).
    Tag(&'s str),
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Lexer {
            src,
            bytes: src.as_bytes(),
            pos: 0,
            line: 1,
            line_start: 0,
            out: Vec::new(),
        }
    }

    pub fn at_end(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    pub fn starts_with(&self, s: &str) -> bool {
        self.bytes[self.pos..].starts_with(s.as_bytes())
    }

    pub fn peek(&self, offset: usize) -> Option<u8> {
        self.bytes.get(self.pos + offset).copied()
    }

    pub fn prev(&self) -> Option<u8> {
        self.pos.checked_sub(1).map(|i| self.bytes[i])
    }

    /// 2026-09-24: Advances one byte, tracking line boundaries.
    pub fn bump(&mut self) {
        if self.bytes[self.pos] == b'\n' {
            self.line += 1;
            self.line_start = self.pos + 1;
        }
        self.pos += 1;
    }

    pub fn bump_n(&mut self, n: usize) {
        for _ in 0..n {
            if self.at_end() {
                break;
            }
            self.bump();
        }
    }

    /// 2026-09-24: Advances until `needle` has been consumed or EOF.
    pub fn skip_past(&mut self, needle: &str) {
        while !self.at_end() {
            if self.starts_with(needle) {
                self.bump_n(needle.len());
                return;
            }
            self.bump();
        }
    }

    pub fn lex_code(&mut self, syn: &Syntax, stop: Stop<'_>) {
        let mut depth = 0usize;
        while !self.at_end() {
            match stop {
                Stop::CloseBrace => match self.bytes[self.pos] {
                    b'{' => depth += 1,
                    b'}' if depth == 0 => return,
                    b'}' => depth -= 1,
                    _ => {}
                },
                Stop::Tag(tag) => {
                    if self.at_close_tag(tag) {
                        return;
                    }
                }
                Stop::Eof => {}
            }
            if self.try_region(syn)
                || self.try_block_comment(syn)
                || self.try_line_comment(syn)
                || self.try_literal(syn)
            {
                continue;
            }
            self.bump();
        }
    }

    fn at_close_tag(&self, tag: &str) -> bool {
        if !self.starts_with("</") {
            return false;
        }
        let after = self.pos + 2;
        let end = after + tag.len();
        end <= self.bytes.len()
            && self.bytes[after..end].eq_ignore_ascii_case(tag.as_bytes())
            && matches!(
                self.bytes.get(end),
                Some(b'>') | Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r')
            )
    }

    /// 2026-09-24: `<script …>` / `<style …>` in mixed files: consume the open
    /// tag, lex the body with the embedded syntax, consume the close tag.
    fn try_region(&mut self, syn: &Syntax) -> bool {
        for region in &syn.regions {
            if self.bytes[self.pos] != b'<' {
                return false;
            }
            let name_end = self.pos + 1 + region.tag.len();
            let name_matches = name_end <= self.bytes.len()
                && self.bytes[self.pos + 1..name_end].eq_ignore_ascii_case(region.tag.as_bytes())
                && matches!(
                    self.bytes.get(name_end),
                    Some(b'>') | Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'\r') | Some(b'/')
                );
            if !name_matches {
                continue;
            }
            self.bump_n(1 + region.tag.len());
            let self_closing = self.consume_open_tag();
            if self_closing {
                return true;
            }
            self.lex_code(&region.syntax, Stop::Tag(&region.tag));
            if !self.at_end() {
                self.skip_past(">");
            }
            return true;
        }
        false
    }

    /// 2026-09-24: Consumes the rest of an open tag (attribute quotes respected)
    /// and reports whether it was self-closing.
    fn consume_open_tag(&mut self) -> bool {
        let mut quote: Option<u8> = None;
        let mut last_significant = b' ';
        while !self.at_end() {
            let b = self.bytes[self.pos];
            match quote {
                Some(q) if b == q => quote = None,
                Some(_) => {}
                None if b == b'"' || b == b'\'' => quote = Some(b),
                None if b == b'>' => {
                    self.bump();
                    return last_significant == b'/';
                }
                None => {}
            }
            if !b.is_ascii_whitespace() {
                last_significant = b;
            }
            self.bump();
        }
        true
    }

    fn try_block_comment(&mut self, syn: &Syntax) -> bool {
        for marker in &syn.block_markers {
            if !self.starts_with(&marker.open) {
                continue;
            }
            if marker.line_start_only && self.pos != self.line_start {
                continue;
            }
            self.emit_block(marker);
            return true;
        }
        false
    }

    fn emit_block(&mut self, marker: &BlockMarker) {
        let start = self.pos;
        let start_line = self.line;
        let (own_line, indent) = self.line_prefix();
        self.bump_n(marker.open.len());
        let mut depth = 1usize;
        let mut inner_end = self.bytes.len();
        while !self.at_end() {
            if self.starts_with(&marker.close) {
                depth -= 1;
                if depth == 0 {
                    inner_end = self.pos;
                    self.bump_n(marker.close.len());
                    break;
                }
                self.bump_n(marker.close.len());
            } else if marker.nested && self.starts_with(&marker.open) {
                depth += 1;
                self.bump_n(marker.open.len());
            } else {
                self.bump();
            }
        }
        let inner = &self.src[start + marker.open.len()..inner_end];
        let raw = &self.src[start..self.pos];
        self.out.push(Comment {
            line: start_line,
            end_line: self.line,
            kind: CommentKind::Block,
            marker: marker.open.clone(),
            own_line,
            indent,
            raw: super::group::normalise_raw(raw),
            text: super::group::block_text(inner, marker.open.ends_with('*')),
        });
    }

    fn try_line_comment(&mut self, syn: &Syntax) -> bool {
        for marker in &syn.line_markers {
            if !self.starts_with(&marker.text) {
                continue;
            }
            if marker.needs_ws_before && !self.prev().is_none_or(|b| b.is_ascii_whitespace()) {
                continue;
            }
            if marker.escape.is_some() && marker.escape == self.prev() {
                continue;
            }
            self.emit_line(marker);
            return true;
        }
        false
    }

    fn emit_line(&mut self, marker: &LineMarker) {
        let start = self.pos;
        let start_line = self.line;
        let (own_line, indent) = self.line_prefix();
        let eol = self.bytes[self.pos..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(self.bytes.len(), |i| self.pos + i);
        let raw_line = self.src[start..eol].trim_end();
        let run = super::group::marker_run(raw_line, &marker.text);
        self.out.push(Comment {
            line: start_line,
            end_line: start_line,
            kind: CommentKind::Line,
            marker: run.to_string(),
            own_line,
            indent,
            raw: raw_line.to_string(),
            text: raw_line[run.len()..].trim().to_string(),
        });
        self.bump_n(eol - start);
    }

    fn line_prefix(&self) -> (bool, String) {
        let prefix = &self.src[self.line_start..self.pos];
        if prefix.trim().is_empty() {
            (true, prefix.to_string())
        } else {
            (false, String::new())
        }
    }

    fn try_literal(&mut self, syn: &Syntax) -> bool {
        if syn.rust_literals && literals::try_rust_literal(self) {
            return true;
        }
        if syn.regex_literals && literals::try_regex_literal(self) {
            return true;
        }
        for rule in &syn.strings {
            if self.starts_with(&rule.open) {
                literals::skip_string(self, syn, rule);
                return true;
            }
        }
        false
    }
}
