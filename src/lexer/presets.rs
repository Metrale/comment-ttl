// 2026-09-24: Named comment syntaxes. `comment_formats` values may refer to
// any of these by name.

use super::syntax::{
    BlockMarker, Region, StringRule, Syntax, block, line, line_ws, quote, quote_no_escape,
};

pub const PRESET_NAMES: &[&str] = &[
    "c",
    "js",
    "rust",
    "go",
    "css",
    "python",
    "shell",
    "hash",
    "yaml",
    "toml",
    "ruby",
    "sql",
    "lua",
    "haskell",
    "semicolon",
    "tex",
    "erlang",
    "xml",
    "html",
    "powershell",
];

pub fn preset(name: &str) -> Option<Syntax> {
    let base = Syntax {
        name: name.to_string(),
        ..Syntax::default()
    };
    let c_like = |base: Syntax| Syntax {
        line_markers: vec![line("//")],
        block_markers: vec![block("/*", "*/")],
        strings: vec![quote("\""), quote("'")],
        ..base
    };
    let hash_like = |base: Syntax, strings: Vec<StringRule>| Syntax {
        line_markers: vec![line_ws("#")],
        strings,
        ..base
    };
    Some(match name {
        "c" => c_like(base),
        "js" => Syntax {
            strings: vec![
                quote("\""),
                quote("'"),
                StringRule {
                    template: true,
                    ..quote("`")
                },
            ],
            regex_literals: true,
            ..c_like(base)
        },
        "rust" => Syntax {
            line_markers: vec![line("//")],
            block_markers: vec![BlockMarker {
                nested: true,
                ..block("/*", "*/")
            }],
            strings: vec![quote("\"")],
            rust_literals: true,
            ..base
        },
        "go" => Syntax {
            strings: vec![quote("\""), quote("'"), quote_no_escape("`")],
            ..c_like(base)
        },
        "css" => Syntax {
            block_markers: vec![block("/*", "*/")],
            strings: vec![quote("\""), quote("'")],
            ..base
        },
        "python" => hash_like(
            base,
            vec![quote("\"\"\""), quote("'''"), quote("\""), quote("'")],
        ),
        "shell" => hash_like(base, vec![quote("\""), quote_no_escape("'")]),
        "hash" => hash_like(base, vec![]),
        "yaml" => hash_like(
            base,
            vec![
                quote("\""),
                StringRule {
                    doubled: true,
                    ..quote_no_escape("'")
                },
            ],
        ),
        "toml" => hash_like(
            base,
            vec![
                quote("\"\"\""),
                quote_no_escape("'''"),
                quote("\""),
                quote_no_escape("'"),
            ],
        ),
        "ruby" => Syntax {
            block_markers: vec![BlockMarker {
                line_start_only: true,
                ..block("=begin", "=end")
            }],
            ..hash_like(base, vec![quote("\""), quote("'")])
        },
        "sql" => Syntax {
            line_markers: vec![line("--")],
            block_markers: vec![block("/*", "*/")],
            strings: vec![quote_no_escape("'"), quote("\"")],
            ..base
        },
        "lua" => Syntax {
            line_markers: vec![line("--")],
            block_markers: vec![block("--[[", "]]")],
            strings: vec![quote("\""), quote("'"), quote_no_escape("[[")]
                .into_iter()
                .map(|mut r| {
                    if r.open == "[[" {
                        r.close = "]]".to_string();
                    }
                    r
                })
                .collect(),
            ..base
        },
        "haskell" => Syntax {
            line_markers: vec![line("--")],
            block_markers: vec![BlockMarker {
                nested: true,
                ..block("{-", "-}")
            }],
            strings: vec![quote("\"")],
            ..base
        },
        "semicolon" => Syntax {
            line_markers: vec![line(";"), line_ws("#")],
            strings: vec![quote("\"")],
            ..base
        },
        "tex" => Syntax {
            line_markers: vec![super::syntax::LineMarker {
                escape: Some(b'\\'),
                ..line("%")
            }],
            ..base
        },
        "erlang" => Syntax {
            line_markers: vec![line("%")],
            strings: vec![quote("\"")],
            ..base
        },
        "xml" => Syntax {
            block_markers: vec![block("<!--", "-->")],
            ..base
        },
        "html" => Syntax {
            block_markers: vec![block("<!--", "-->")],
            regions: vec![
                Region {
                    tag: "script".to_string(),
                    syntax: Box::new(preset("js")?),
                },
                Region {
                    tag: "style".to_string(),
                    syntax: Box::new(preset("css")?),
                },
            ],
            ..base
        },
        "powershell" => Syntax {
            block_markers: vec![block("<#", "#>")],
            ..hash_like(base, vec![quote("\""), quote("'")])
        },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_preset_resolves_and_nothing_else_does() {
        for name in PRESET_NAMES {
            let s = preset(name).unwrap_or_else(|| panic!("preset {name} missing"));
            assert_eq!(s.name, *name);
            assert!(
                !s.line_markers.is_empty() || !s.block_markers.is_empty(),
                "{name} has no markers"
            );
        }
        assert!(preset("cobol").is_none());
        assert!(preset("").is_none());
    }

    #[test]
    fn lua_long_strings_close_with_double_bracket() {
        let lua = preset("lua").unwrap();
        let long = lua.strings.iter().find(|r| r.open == "[[").unwrap();
        assert_eq!(long.close, "]]");
    }
}
