// 2026-09-24: Configuration layering: action inputs (or CLI flags) win over
// `.comment-ttl.toml`, which wins over the documented defaults. Every value
// is validated here so the scan never sees a malformed setting.

use std::path::PathBuf;

use regex::Regex;
use serde::Deserialize;
use serde_json::Value;

use crate::calendar::parse_ymd;
use crate::exemptions::{Exemptions, ExemptionsSpec};
use crate::formats::{FormatRegistry, FormatSpec};
use crate::ttl::Ttl;

pub const CONFIG_FILE_NAME: &str = ".comment-ttl.toml";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputType {
    Json,
    Csv,
    Sarif,
    Markdown,
}

impl OutputType {
    pub fn extension(self) -> &'static str {
        match self {
            OutputType::Json => "json",
            OutputType::Csv => "csv",
            OutputType::Sarif => "sarif",
            OutputType::Markdown => "md",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceDate {
    Now,
    Commit,
    Fixed(time::Date),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    All,
    Changed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndatedMode {
    Fail,
    Blame,
}

/// 2026-09-24: One layer of settings before validation. `None` means "not
/// set at this layer". Strings are accepted for every field so the same
/// struct serves action inputs, CLI flags and the TOML file.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    pub comment_ttl: Option<String>,
    pub scan_dirs: Option<String>,
    pub file_types: Option<String>,
    pub exempt_paths: Option<String>,
    pub comment_formats: Option<Value>,
    pub exemptions: Option<Value>,
    pub output_type: Option<String>,
    pub fail_on_flag: Option<Value>,
    pub reference_date: Option<String>,
    pub warn_within: Option<String>,
    pub baseline: Option<String>,
    pub scope: Option<String>,
    pub undated: Option<String>,
    pub annotations: Option<Value>,
}

impl Layer {
    /// 2026-09-24: `over` takes precedence field by field.
    pub fn merged_over(self, under: Layer) -> Layer {
        Layer {
            comment_ttl: self.comment_ttl.or(under.comment_ttl),
            scan_dirs: self.scan_dirs.or(under.scan_dirs),
            file_types: self.file_types.or(under.file_types),
            exempt_paths: self.exempt_paths.or(under.exempt_paths),
            comment_formats: self.comment_formats.or(under.comment_formats),
            exemptions: self.exemptions.or(under.exemptions),
            output_type: self.output_type.or(under.output_type),
            fail_on_flag: self.fail_on_flag.or(under.fail_on_flag),
            reference_date: self.reference_date.or(under.reference_date),
            warn_within: self.warn_within.or(under.warn_within),
            baseline: self.baseline.or(under.baseline),
            scope: self.scope.or(under.scope),
            undated: self.undated.or(under.undated),
            annotations: self.annotations.or(under.annotations),
        }
    }

    pub fn from_toml(text: &str) -> Result<Layer, String> {
        let table: toml::Table =
            toml::from_str(text).map_err(|e| format!("{CONFIG_FILE_NAME}: {e}"))?;
        let value = serde_json::to_value(table).map_err(|e| format!("{CONFIG_FILE_NAME}: {e}"))?;
        serde_json::from_value(value).map_err(|e| format!("{CONFIG_FILE_NAME}: {e}"))
    }
}

pub struct Config {
    pub ttl: Ttl,
    pub ttl_text: String,
    pub scan_dirs: Regex,
    pub file_types: Option<Regex>,
    pub exempt_paths: Option<Regex>,
    pub formats: FormatRegistry,
    pub exemptions: Exemptions,
    pub output_type: OutputType,
    pub fail_on_flag: bool,
    pub reference_date: ReferenceDate,
    pub warn_within: Option<Ttl>,
    pub warn_within_text: Option<String>,
    pub baseline: Option<PathBuf>,
    pub scope: Scope,
    pub undated: UndatedMode,
    pub annotations: bool,
}

fn regex(name: &str, pattern: &str) -> Result<Regex, String> {
    Regex::new(pattern).map_err(|e| format!("{name}: invalid regex {pattern:?}: {e}"))
}

fn optional_regex(name: &str, value: Option<String>) -> Result<Option<Regex>, String> {
    value
        .filter(|v| !v.trim().is_empty())
        .map(|v| regex(name, &v))
        .transpose()
}

fn boolean(name: &str, value: Option<Value>, default: bool) -> Result<bool, String> {
    match value {
        None => Ok(default),
        Some(Value::Bool(b)) => Ok(b),
        Some(Value::String(s)) => match s.trim().to_ascii_lowercase().as_str() {
            "true" | "yes" | "on" | "1" => Ok(true),
            "false" | "no" | "off" | "0" => Ok(false),
            "" => Ok(default),
            other => Err(format!("{name}: expected true or false, got {other:?}")),
        },
        Some(other) => Err(format!("{name}: expected true or false, got {other}")),
    }
}

/// 2026-09-24: Inputs arrive as text (YAML or JSON) from the action, or as a
/// structured value from the TOML file. Both are normalised to JSON here.
fn structured(name: &str, value: Option<Value>) -> Result<Option<Value>, String> {
    match value {
        None => Ok(None),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Value::String(s)) => serde_yaml_ng::from_str::<Value>(&s)
            .map(Some)
            .map_err(|e| format!("{name}: not valid YAML/JSON: {e}")),
        Some(v) => Ok(Some(v)),
    }
}

fn formats(value: Option<Value>) -> Result<Vec<(String, FormatSpec)>, String> {
    let Some(value) = structured("comment_formats", value)? else {
        return Ok(Vec::new());
    };
    let Value::Object(map) = value else {
        return Err(
            "comment_formats: expected a mapping of extension regex to comment syntax".into(),
        );
    };
    map.into_iter()
        .map(|(k, v)| {
            let spec: FormatSpec = serde_json::from_value(v)
                .map_err(|e| format!("comment_formats: entry {k:?}: {e}"))?;
            Ok((k, spec))
        })
        .collect()
}

fn exemptions(value: Option<Value>) -> Result<ExemptionsSpec, String> {
    match structured("exemptions", value)? {
        None => Ok(ExemptionsSpec::default()),
        Some(v) => serde_json::from_value(v).map_err(|e| {
            format!("exemptions: expected a list of regexes or {{replace, patterns}}: {e}")
        }),
    }
}

fn choice<T>(
    name: &str,
    value: Option<String>,
    default: T,
    options: &[(&str, T)],
) -> Result<T, String>
where
    T: Copy,
{
    let Some(v) = value
        .map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
    else {
        return Ok(default);
    };
    options
        .iter()
        .find(|(k, _)| *k == v)
        .map(|(_, t)| *t)
        .ok_or_else(|| {
            let names: Vec<&str> = options.iter().map(|(k, _)| *k).collect();
            format!("{name}: expected one of {}, got {v:?}", names.join("|"))
        })
}

impl Config {
    pub fn from_layer(layer: Layer) -> Result<Config, String> {
        let ttl_text = layer
            .comment_ttl
            .filter(|v| !v.trim().is_empty())
            .ok_or("comment_ttl is required (for example `1 month`, `30d` or `P1M`)")?;
        let ttl = Ttl::parse(&ttl_text).map_err(|e| format!("comment_ttl: {e}"))?;
        let warn_within_text = layer.warn_within.filter(|v| !v.trim().is_empty());
        let warn_within = warn_within_text
            .as_deref()
            .map(|v| Ttl::parse(v).map_err(|e| format!("warn_within: {e}")))
            .transpose()?;
        let reference_date = match layer.reference_date.as_deref().map(str::trim) {
            None | Some("") | Some("commit") => ReferenceDate::Commit,
            Some("now") => ReferenceDate::Now,
            Some(other) => ReferenceDate::Fixed(
                parse_ymd(other)
                    .map_err(|e| format!("reference_date: expected now|commit|YYYY-MM-DD: {e}"))?,
            ),
        };
        Ok(Config {
            ttl,
            ttl_text: ttl_text.trim().to_string(),
            scan_dirs: regex(
                "scan_dirs",
                layer
                    .scan_dirs
                    .as_deref()
                    .filter(|v| !v.trim().is_empty())
                    .unwrap_or("."),
            )?,
            file_types: optional_regex("file_types", layer.file_types)?,
            exempt_paths: optional_regex("exempt_paths", layer.exempt_paths)?,
            formats: FormatRegistry::new(&formats(layer.comment_formats)?)?,
            exemptions: Exemptions::new(&exemptions(layer.exemptions)?)?,
            output_type: choice(
                "output_type",
                layer.output_type,
                OutputType::Json,
                &[
                    ("json", OutputType::Json),
                    ("csv", OutputType::Csv),
                    ("sarif", OutputType::Sarif),
                    ("markdown", OutputType::Markdown),
                ],
            )?,
            fail_on_flag: boolean("fail_on_flag", layer.fail_on_flag, true)?,
            reference_date,
            warn_within,
            warn_within_text,
            baseline: layer
                .baseline
                .filter(|v| !v.trim().is_empty())
                .map(PathBuf::from),
            scope: choice(
                "scope",
                layer.scope,
                Scope::All,
                &[("all", Scope::All), ("changed", Scope::Changed)],
            )?,
            undated: choice(
                "undated",
                layer.undated,
                UndatedMode::Fail,
                &[("fail", UndatedMode::Fail), ("blame", UndatedMode::Blame)],
            )?,
            annotations: boolean("annotations", layer.annotations, true)?,
        })
    }

    /// 2026-09-24: The `scan_dirs` regex runs against the directory part of a
    /// path with a trailing slash; files at the repository root see `./`.
    pub fn path_selected(&self, path: &str) -> bool {
        let dir = match path.rfind('/') {
            Some(i) => format!("{}/", &path[..i]),
            None => "./".to_string(),
        };
        let name = path.rsplit('/').next().unwrap_or(path);
        self.scan_dirs.is_match(&dir)
            && self.file_types.as_ref().is_none_or(|r| r.is_match(name))
            && !self.exempt_paths.as_ref().is_some_and(|r| r.is_match(path))
    }
}
