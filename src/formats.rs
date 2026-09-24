// 2026-09-24: Maps file names to comment syntaxes. User-supplied
// `comment_formats` entries are consulted before the defaults, so a project
// can override any extension.

use regex::Regex;
use serde::Deserialize;

use crate::lexer::{InlineSyntax, Syntax, preset};

/// 2026-09-24: The shipped map, in evaluation order. Keys are regexes matched
/// against the file name (not the full path).
pub const DEFAULT_FORMATS: &[(&str, &str)] = &[
    (
        r"\.(js|mjs|cjs|jsx|ts|mts|cts|tsx|jsonc)$|^(tsconfig|jsconfig)(\..*)?\.json$",
        "js",
    ),
    (r"\.rs$", "rust"),
    (r"\.go$", "go"),
    (
        r"\.(java|kt|kts|scala|c|h|cc|cpp|cxx|hpp|hh|hxx|cu|cuh|cs|swift|dart|proto|groovy|gradle|zig|metal|glsl|hlsl|wgsl|scss|less|php)$",
        "c",
    ),
    (r"\.css$", "css"),
    (r"\.(py|pyi|pyw)$", "python"),
    (
        r"\.(sh|bash|zsh|ksh|fish)$|^\.?(bashrc|zshrc|profile|bash_profile)$",
        "shell",
    ),
    (
        r"^(Dockerfile|Containerfile|Makefile|GNUmakefile|Justfile|\.gitignore|\.dockerignore|\.gitattributes|\.editorconfig|\.env(\..*)?)$|\.(mk|cfg|conf|properties|dockerfile)$|^requirements.*\.txt$",
        "hash",
    ),
    (r"\.(ya?ml)$", "yaml"),
    (r"\.toml$", "toml"),
    (r"\.(rb|rake|gemspec)$|^(Gemfile|Rakefile)$", "ruby"),
    (r"\.(sql|psql)$", "sql"),
    (r"\.lua$", "lua"),
    (r"\.hs$", "haskell"),
    (
        r"\.(lisp|el|clj|cljs|cljc|edn|scm|ss|rkt|ini|asm)$",
        "semicolon",
    ),
    (r"\.(tex|sty|cls|bib)$", "tex"),
    (r"\.(erl|hrl)$", "erlang"),
    (
        r"\.(md|markdown|mdx|xml|svg|xsl|xslt|xsd|plist|csproj|fsproj|vbproj|props|targets|nuspec|xhtml|rss|atom|wsdl|resx)$",
        "xml",
    ),
    (r"\.(html|htm|svelte|vue|astro)$", "html"),
    (r"\.(ps1|psm1|psd1)$", "powershell"),
];

/// 2026-09-24: One `comment_formats` value: a preset name or an inline syntax.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum FormatSpec {
    Preset(String),
    Inline(InlineSyntax),
}

pub struct FormatEntry {
    pub pattern: Regex,
    pub syntax: Syntax,
}

pub struct FormatRegistry {
    entries: Vec<FormatEntry>,
}

impl FormatRegistry {
    /// 2026-09-24: Builds the registry from user entries (first) plus the
    /// defaults. A bad regex, unknown preset or empty inline syntax is a
    /// configuration error.
    pub fn new(user: &[(String, FormatSpec)]) -> Result<Self, String> {
        let mut entries = Vec::with_capacity(user.len() + DEFAULT_FORMATS.len());
        for (pattern, spec) in user {
            let regex = Regex::new(pattern)
                .map_err(|e| format!("comment_formats: invalid regex {pattern:?}: {e}"))?;
            let syntax = match spec {
                FormatSpec::Preset(name) => preset(name).ok_or_else(|| {
                    format!(
                        "comment_formats: unknown preset {name:?} for {pattern:?}; known presets: {}",
                        crate::lexer::PRESET_NAMES.join(", ")
                    )
                })?,
                FormatSpec::Inline(inline) => inline.clone().into_syntax(pattern)?,
            };
            entries.push(FormatEntry {
                pattern: regex,
                syntax,
            });
        }
        for (pattern, name) in DEFAULT_FORMATS {
            entries.push(FormatEntry {
                pattern: Regex::new(pattern).expect("default format regexes are valid"),
                syntax: preset(name).expect("default presets exist"),
            });
        }
        Ok(FormatRegistry { entries })
    }

    pub fn for_path(&self, path: &str) -> Option<&Syntax> {
        let name = path.rsplit('/').next().unwrap_or(path);
        self.entries
            .iter()
            .find(|e| e.pattern.is_match(name))
            .map(|e| &e.syntax)
    }

    pub fn entries(&self) -> &[FormatEntry] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name_for(reg: &FormatRegistry, path: &str) -> Option<String> {
        reg.for_path(path).map(|s| s.name.clone())
    }

    #[test]
    fn defaults_cover_the_documented_families() {
        let reg = FormatRegistry::new(&[]).unwrap();
        for (path, expected) in [
            ("a/b.rs", "rust"),
            ("x.tsx", "js"),
            ("tsconfig.build.json", "js"),
            ("k.cu", "c"),
            ("style.scss", "c"),
            ("style.css", "css"),
            ("Dockerfile", "hash"),
            (".gitignore", "hash"),
            ("requirements-dev.txt", "hash"),
            ("ci.yml", "yaml"),
            ("Cargo.toml", "toml"),
            ("Gemfile", "ruby"),
            ("q.sql", "sql"),
            ("README.md", "xml"),
            ("icon.svg", "xml"),
            ("App.svelte", "html"),
            ("page.vue", "html"),
            ("paper.tex", "tex"),
            ("mod.erl", "erlang"),
            ("deploy.ps1", "powershell"),
        ] {
            assert_eq!(name_for(&reg, path).as_deref(), Some(expected), "{path}");
        }
        for path in [
            "notes.txt",
            "data.json",
            "img.png",
            "Cargo.lock",
            "bin",
            "a.s",
        ] {
            assert_eq!(name_for(&reg, path), None, "{path} must not be scanned");
        }
    }

    #[test]
    fn user_entries_win_and_are_validated() {
        let user = vec![
            (r"\.md$".to_string(), FormatSpec::Preset("hash".into())),
            (
                r"\.zz$".to_string(),
                FormatSpec::Inline(InlineSyntax {
                    line: vec!["!!".into()],
                    ..InlineSyntax::default()
                }),
            ),
        ];
        let reg = FormatRegistry::new(&user).unwrap();
        assert_eq!(name_for(&reg, "README.md").as_deref(), Some("hash"));
        assert_eq!(name_for(&reg, "f.zz").as_deref(), Some(r"\.zz$"));
        assert_eq!(name_for(&reg, "f.rs").as_deref(), Some("rust"));

        let bad_regex = vec![("(".to_string(), FormatSpec::Preset("c".into()))];
        assert!(
            FormatRegistry::new(&bad_regex)
                .err()
                .unwrap()
                .contains("invalid regex")
        );
        let bad_preset = vec![(r"\.x$".to_string(), FormatSpec::Preset("cobol".into()))];
        assert!(
            FormatRegistry::new(&bad_preset)
                .err()
                .unwrap()
                .contains("unknown preset")
        );
        let empty = vec![(
            r"\.x$".to_string(),
            FormatSpec::Inline(InlineSyntax::default()),
        )];
        assert!(
            FormatRegistry::new(&empty)
                .err()
                .unwrap()
                .contains("neither")
        );
    }
}
