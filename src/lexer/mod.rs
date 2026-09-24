// 2026-09-24: Public lexer API: `lex(source, syntax)` returns the grouped
// comments of one file.

pub mod group;
mod literals;
pub mod presets;
mod scan;
pub mod syntax;

pub use presets::{PRESET_NAMES, preset};
pub use syntax::{InlineSyntax, Syntax};

use crate::comment::Comment;

pub fn lex(source: &str, syntax: &Syntax) -> Vec<Comment> {
    let mut lx = scan::Lexer::new(source);
    lx.lex_code(syntax, scan::Stop::Eof);
    group::group(lx.out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(src: &str, name: &str) -> Vec<String> {
        lex(src, &preset(name).unwrap())
            .into_iter()
            .map(|c| c.text)
            .collect()
    }

    #[test]
    fn strings_hide_comment_markers() {
        assert_eq!(texts(r#"let u = "http://x"; // real"#, "js"), ["real"]);
        assert_eq!(
            texts(r#"let s = 'a // b'; let t = "c \" // d";"#, "js"),
            Vec::<String>::new()
        );
        assert_eq!(texts("x = \"#not\" # yes", "python"), ["yes"]);
        assert_eq!(texts("s = '''\n# inside\n''' # after", "python"), ["after"]);
        assert_eq!(texts("key: 'it''s # not' # yes", "yaml"), ["yes"]);
        assert_eq!(texts("s := `raw // not`; // yes", "go"), ["yes"]);
        assert_eq!(texts("SELECT '--x' -- yes", "sql"), ["yes"]);
        assert_eq!(texts("echo '# not' \"# not\" # yes", "shell"), ["yes"]);
    }

    #[test]
    fn hash_needs_whitespace_before_it() {
        assert_eq!(texts("url: http://x#frag # yes", "yaml"), ["yes"]);
        assert_eq!(texts("n=$# ; echo ${#a} # yes", "shell"), ["yes"]);
        assert_eq!(texts("#!/bin/sh\n# next", "shell"), ["/bin/sh", "next"]);
    }

    #[test]
    fn template_literals_nest() {
        let src = "const s = `a ${ `b ${ c } // not` } d`; // yes";
        assert_eq!(texts(src, "js"), ["yes"]);
        let src = "const s = `${ x /* real: code inside the expression */ } // not`;";
        assert_eq!(texts(src, "js"), ["real: code inside the expression"]);
        let src = "const s = `line1\n// not a comment\n${x}`;\n// yes";
        assert_eq!(texts(src, "js"), ["yes"]);
    }

    #[test]
    fn regex_literals_are_skipped_but_division_is_not() {
        assert_eq!(texts("const r = /https?:\\/\\//; // yes", "js"), ["yes"]);
        assert_eq!(texts("const q = a / b / c; // yes", "js"), ["yes"]);
        assert_eq!(texts("return /[/]x/.test(s) // yes", "js"), ["yes"]);
        assert_eq!(texts("x = y / 2\n// yes", "js"), ["yes"]);
    }

    #[test]
    fn rust_literals() {
        assert_eq!(texts(r##"let s = r#"// not"#; // yes"##, "rust"), ["yes"]);
        assert_eq!(texts("let c = '/'; let d = '\\''; // yes", "rust"), ["yes"]);
        assert_eq!(texts("fn f<'a>(x: &'a str) {} // yes", "rust"), ["yes"]);
        assert_eq!(texts("let c = 'é'; // yes", "rust"), ["yes"]);
        assert_eq!(
            texts("/* outer /* inner */ still */ // yes", "rust"),
            ["outer /* inner */ still", "yes"]
        );
        assert_eq!(texts(r#"let b = br"x // y"; // yes"#, "rust"), ["yes"]);
    }

    #[test]
    fn mixed_files_switch_lexers_per_region() {
        let src = "<!-- html -->\n<script lang=\"ts\">\n  // js\n  const s = \"<!-- not -->\";\n</script>\n<style>\n  /* css */\n</style>\n<p>// not a comment</p>";
        assert_eq!(texts(src, "html"), ["html", "js", "css"]);
        let src = "<script src=\"x.js\"/>\n<!-- after -->";
        assert_eq!(texts(src, "html"), ["after"]);
    }

    #[test]
    fn line_and_block_markers_carry_position_data() {
        let src = "code(); // trailing\n  // own\n  // own2\n/* block\n   more */";
        let cs = lex(src, &preset("c").unwrap());
        assert_eq!(cs.len(), 3);
        assert!(!cs[0].own_line);
        assert_eq!(
            (cs[1].line, cs[1].end_line, cs[1].indent.as_str()),
            (2, 3, "  ")
        );
        assert_eq!(cs[2].text, "block\nmore");
        assert_eq!(cs[2].raw, "/* block\nmore */");
        assert_eq!((cs[2].line, cs[2].end_line), (4, 5));
    }

    #[test]
    fn tex_escaped_percent_and_ruby_begin_end() {
        assert_eq!(texts("50\\% off % yes", "tex"), ["yes"]);
        assert_eq!(
            texts("x = 1\n=begin\nblock\n=end\n# yes", "ruby"),
            ["block", "yes"]
        );
        assert_eq!(texts("y = 2 =begin not\n# yes", "ruby"), ["yes"]);
    }

    #[test]
    fn crlf_sources_do_not_leak_carriage_returns() {
        let cs = lex("// a\r\n// b\r\nx\r\n", &preset("c").unwrap());
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].text, "a\nb");
        assert_eq!(cs[0].raw, "// a\n// b");
    }

    #[test]
    fn unterminated_literals_run_to_eof_without_panicking() {
        assert!(texts("x = \"open // never", "js").is_empty());
        assert!(texts("/* open", "c").len() == 1);
        assert!(texts("s = `open ${ x } // never", "js").is_empty());
    }
}
