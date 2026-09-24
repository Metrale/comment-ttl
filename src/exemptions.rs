// 2026-09-24: Comments that never need a date. Each default is a regex over
// the comment's raw lines (markers kept, whitespace-trimmed, `\n`-joined).

use regex::Regex;
use serde::Deserialize;

/// 2026-09-24: Marker prefix shared by the defaults: any run of comment
/// markers and whitespace, so one pattern covers `//`, `#`, `/**`, `<!--`…
const MARKERS: &str = r"(?:(?://+!?|/\*+|#+!?|--+|;+|%+|<!--|\*+|<#|=begin)\s*)*";

pub struct DefaultExemption {
    pub name: &'static str,
    pub pattern: String,
}

pub fn default_exemptions() -> Vec<DefaultExemption> {
    let m = MARKERS;
    vec![
        DefaultExemption {
            name: "license-header",
            pattern: format!(
                r"(?is)^{m}(?:SPDX-License-Identifier:|SPDX-FileCopyrightText:|Copyright\b|\(c\)\s*\d{{4}}|©|Licensed under\b|Licensed to the\b|Permission is hereby granted|This Source Code Form|This program is free software|Use of this source code is governed|All rights reserved)"
            ),
        },
        DefaultExemption {
            name: "shebang",
            pattern: r"^#!".to_string(),
        },
        DefaultExemption {
            name: "tool-directive",
            pattern: format!(
                r"(?i)^{m}(?:eslint-disable|@ts-expect-error|prettier-ignore|svelte-ignore|cspell:|markdownlint-|noqa\b|type:\s*ignore\b|#!?\[allow|clippy::|rustfmt::skip|yaml-language-server:)"
            ),
        },
        DefaultExemption {
            name: "doc-block-tags-only",
            pattern: r"(?s)\A(?:[ \t]*(?://[/!]?|/\*+|\*/|\*)?[ \t]*(?:@\w[^\n]*)?[ \t]*\n?)+\z"
                .to_string(),
        },
        DefaultExemption {
            name: "pinned-action-version",
            pattern: r"^#\s*v\d+(?:\.\d+)*\s*$".to_string(),
        },
    ]
}

/// 2026-09-24: The user-facing `exemptions` value: a list extends the
/// defaults; `{ replace: true, patterns: [...] }` discards them.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum ExemptionsSpec {
    List(Vec<String>),
    Object {
        #[serde(default)]
        replace: bool,
        patterns: Vec<String>,
    },
}

impl Default for ExemptionsSpec {
    fn default() -> Self {
        ExemptionsSpec::List(Vec::new())
    }
}

pub struct ExemptionRule {
    pub name: String,
    pub regex: Regex,
}

pub struct Exemptions {
    rules: Vec<ExemptionRule>,
}

impl Exemptions {
    pub fn new(spec: &ExemptionsSpec) -> Result<Self, String> {
        let (replace, patterns) = match spec {
            ExemptionsSpec::List(p) => (false, p),
            ExemptionsSpec::Object { replace, patterns } => (*replace, patterns),
        };
        let mut rules = Vec::new();
        if !replace {
            for d in default_exemptions() {
                rules.push(ExemptionRule {
                    name: d.name.to_string(),
                    regex: Regex::new(&d.pattern).expect("default exemption regexes are valid"),
                });
            }
        }
        for (i, p) in patterns.iter().enumerate() {
            let regex =
                Regex::new(p).map_err(|e| format!("exemptions: invalid regex {p:?}: {e}"))?;
            rules.push(ExemptionRule {
                name: format!("user-{}", i + 1),
                regex,
            });
        }
        Ok(Exemptions { rules })
    }

    /// 2026-09-24: The name of the first matching rule, if any.
    pub fn matches(&self, raw: &str) -> Option<&str> {
        self.rules
            .iter()
            .find(|r| r.regex.is_match(raw))
            .map(|r| r.name.as_str())
    }

    pub fn rules(&self) -> &[ExemptionRule] {
        &self.rules
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Exemptions {
        Exemptions::new(&ExemptionsSpec::default()).unwrap()
    }

    #[test]
    fn each_default_matches_its_target_and_not_prose() {
        let ex = defaults();
        let cases = [
            ("// SPDX-License-Identifier: MIT", "license-header"),
            ("/*\n* Copyright 2024 Acme\n*/", "license-header"),
            (
                "# Licensed under the Apache License, Version 2.0",
                "license-header",
            ),
            ("#!/usr/bin/env bash", "shebang"),
            ("// eslint-disable-next-line no-console", "tool-directive"),
            ("/* eslint-disable */", "tool-directive"),
            ("// @ts-expect-error until v5", "tool-directive"),
            ("<!-- prettier-ignore -->", "tool-directive"),
            (
                "<!-- svelte-ignore a11y-no-static-element-interactions -->",
                "tool-directive",
            ),
            ("// cspell:ignore metrale", "tool-directive"),
            ("<!-- markdownlint-disable MD033 -->", "tool-directive"),
            ("# noqa: E501", "tool-directive"),
            ("# type: ignore[attr-defined]", "tool-directive"),
            ("// #[allow(dead_code)]", "tool-directive"),
            (
                "// clippy::too_many_arguments is fine here",
                "tool-directive",
            ),
            ("// rustfmt::skip", "tool-directive"),
            ("# yaml-language-server: $schema=x", "tool-directive"),
            (
                "/**\n* @param {string} a\n* @returns {void}\n*/",
                "doc-block-tags-only",
            ),
            ("/// @deprecated", "doc-block-tags-only"),
            ("//", "doc-block-tags-only"),
            ("# v4", "pinned-action-version"),
            ("# v4.2.2", "pinned-action-version"),
        ];
        for (raw, expected) in cases {
            assert_eq!(ex.matches(raw), Some(expected), "{raw:?}");
        }
        for raw in [
            "// 2026-01-01: real note",
            "/// Parses the input and returns the count.",
            "/**\n* Does a thing.\n* @param a\n*/",
            "// the copyright field is optional here",
            "# v4 is faster than v3 in our tests",
            "// TODO: clippy is unhappy",
            "<!-- svelte component notes -->",
            "# noqaX",
        ] {
            assert_eq!(ex.matches(raw), None, "{raw:?} must not be exempt");
        }
    }

    #[test]
    fn user_patterns_extend_or_replace() {
        let extended = Exemptions::new(&ExemptionsSpec::List(vec!["^// ---".into()])).unwrap();
        assert_eq!(extended.matches("// -----"), Some("user-1"));
        assert_eq!(extended.matches("#!/bin/sh"), Some("shebang"));

        let replaced = Exemptions::new(&ExemptionsSpec::Object {
            replace: true,
            patterns: vec!["^// ---".into()],
        })
        .unwrap();
        assert_eq!(replaced.matches("#!/bin/sh"), None);
        assert_eq!(replaced.matches("// ---"), Some("user-1"));
        assert_eq!(replaced.rules().len(), 1);

        let bad = Exemptions::new(&ExemptionsSpec::List(vec!["(".into()]));
        assert!(bad.err().unwrap().contains("invalid regex"));
    }
}
