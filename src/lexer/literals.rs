// 2026-09-24: Skipping the things that look like comments but are not:
// string literals, template literals, Rust raw strings and chars, and
// JavaScript regex literals.

use super::scan::{Lexer, Stop};
use super::syntax::{StringRule, Syntax};

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// 2026-09-24: Consumes a string literal starting at the current position.
/// Templates re-enter code on `${`; an unterminated string runs to EOF, which
/// is the safe direction (nothing after it can be misread as a comment).
pub fn skip_string(lx: &mut Lexer<'_>, syn: &Syntax, rule: &StringRule) {
    let escape = rule.escape;
    lx.bump_n(rule.open.len());
    while !lx.at_end() {
        let b = lx.bytes[lx.pos];
        if Some(b) == escape {
            lx.bump_n(2);
            continue;
        }
        if rule.template && lx.starts_with("${") {
            lx.bump_n(2);
            lx.lex_code(syn, Stop::CloseBrace);
            if !lx.at_end() {
                lx.bump();
            }
            continue;
        }
        if lx.starts_with(&rule.close) {
            lx.bump_n(rule.close.len());
            if rule.doubled && lx.starts_with(&rule.close) {
                lx.bump_n(rule.close.len());
                continue;
            }
            return;
        }
        lx.bump();
    }
}

/// 2026-09-24: Rust raw strings (`r"…"`, `br#"…"#`, `cr##"…"##`) and the
/// `'` rule: `'\…'` and `'x'` are chars, anything else is a lifetime.
pub fn try_rust_literal(lx: &mut Lexer<'_>) -> bool {
    let b = lx.bytes[lx.pos];
    if b == b'\'' {
        if lx.peek(1) == Some(b'\\') {
            lx.bump_n(2);
            lx.skip_past("'");
        } else {
            let ch_len = lx.src[lx.pos + 1..]
                .chars()
                .next()
                .map_or(0, char::len_utf8);
            if ch_len > 0 && lx.peek(1 + ch_len) == Some(b'\'') {
                lx.bump_n(2 + ch_len);
            } else {
                lx.bump();
            }
        }
        return true;
    }
    let prefix_ok = !lx.prev().is_some_and(is_ident);
    if !prefix_ok || !matches!(b, b'r' | b'b' | b'c') {
        return false;
    }
    let mut i = lx.pos;
    if b != b'r' {
        i += 1;
        if lx.bytes.get(i) != Some(&b'r') {
            return false;
        }
    }
    i += 1;
    let hashes = lx.bytes[i..].iter().take_while(|&&c| c == b'#').count();
    if lx.bytes.get(i + hashes) != Some(&b'"') {
        return false;
    }
    lx.bump_n(i + hashes + 1 - lx.pos);
    let mut close = String::from("\"");
    close.push_str(&"#".repeat(hashes));
    lx.skip_past(&close);
    true
}

const REGEX_KEYWORDS: &[&str] = &[
    "return",
    "typeof",
    "instanceof",
    "in",
    "of",
    "new",
    "delete",
    "void",
    "throw",
    "case",
    "do",
    "else",
    "yield",
    "await",
];

/// 2026-09-24: A `/` that is not `//` or `/*` starts a regex literal when the
/// previous significant token cannot end an expression. If no closing `/`
/// appears on the line it was a division and nothing is consumed.
pub fn try_regex_literal(lx: &mut Lexer<'_>) -> bool {
    if lx.bytes[lx.pos] != b'/' || matches!(lx.peek(1), Some(b'/') | Some(b'*')) {
        return false;
    }
    if !regex_allowed(lx) {
        return false;
    }
    let mut i = lx.pos + 1;
    let mut in_class = false;
    while i < lx.bytes.len() {
        match lx.bytes[i] {
            b'\n' => return false,
            b'\\' => i += 1,
            b'[' => in_class = true,
            b']' => in_class = false,
            b'/' if !in_class => {
                i += 1;
                while i < lx.bytes.len() && lx.bytes[i].is_ascii_alphabetic() {
                    i += 1;
                }
                lx.bump_n(i - lx.pos);
                return true;
            }
            _ => {}
        }
        i += 1;
    }
    false
}

fn regex_allowed(lx: &Lexer<'_>) -> bool {
    let mut i = lx.pos;
    while i > 0 && lx.bytes[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    if i == 0 {
        return true;
    }
    let last = lx.bytes[i - 1];
    if is_ident(last) {
        let mut start = i;
        while start > 0 && is_ident(lx.bytes[start - 1]) {
            start -= 1;
        }
        return REGEX_KEYWORDS.contains(&&lx.src[start..i]);
    }
    !matches!(last, b')' | b']')
}
