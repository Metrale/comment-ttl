// 2026-09-24: Command-line surface. Every action input has a matching flag;
// in GitHub Actions the same values arrive as `INPUT_<NAME>` environment
// variables, which are read here at the I/O boundary.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use serde_json::Value;

use crate::config::Layer;
use crate::run::{GithubFiles, Invocation, Mode};

#[derive(Parser, Debug)]
#[command(
    name = "comment-ttl",
    version,
    about = "Fail CI when a comment has not been re-verified within its TTL"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    #[command(about = "Scan the repository and report comments past their TTL")]
    Check(ScanArgs),
    #[command(about = "Write a baseline file grandfathering every currently flagged comment")]
    Baseline(ScanArgs),
}

#[derive(Args, Debug, Clone, Default)]
pub struct ScanArgs {
    #[arg(
        help = "Repository directory (defaults to the current directory)",
        long,
        value_name = "DIR"
    )]
    pub repo: Option<PathBuf>,
    #[arg(
        help = "Config file (defaults to .comment-ttl.toml in the repository, if present)",
        long,
        value_name = "FILE"
    )]
    pub config: Option<PathBuf>,
    #[arg(
        help = "Where to write the report or baseline (stdout when omitted)",
        long,
        value_name = "FILE"
    )]
    pub out: Option<PathBuf>,
    #[arg(
        help = "Base ref for `--scope changed` (the action derives it from the pull request)",
        long,
        value_name = "REF"
    )]
    pub diff_base: Option<String>,

    #[arg(
        help = "Time-to-live, e.g. `1 month`, `30d`, `2w`, `1y 2mo`, `P1M`",
        long,
        value_name = "DURATION"
    )]
    pub comment_ttl: Option<String>,
    #[arg(
        help = "Regex over directories to scan (matched against `dir/`, root files see `./`)",
        long,
        value_name = "REGEX"
    )]
    pub scan_dirs: Option<String>,
    #[arg(help = "Regex over file names to include", long, value_name = "REGEX")]
    pub file_types: Option<String>,
    #[arg(help = "Regex over paths to skip entirely", long, value_name = "REGEX")]
    pub exempt_paths: Option<String>,
    #[arg(
        help = "YAML/JSON mapping of file-name regex to comment syntax, merged over the defaults",
        long,
        value_name = "YAML"
    )]
    pub comment_formats: Option<String>,
    #[arg(
        help = "YAML/JSON list of regexes extending the default exemptions, or `{replace: true, patterns: [...]}`",
        long,
        value_name = "YAML"
    )]
    pub exemptions: Option<String>,
    #[arg(help = "json | csv | sarif | markdown", long, value_name = "TYPE")]
    pub output_type: Option<String>,
    #[arg(
        help = "Exit 1 when any comment is flagged (default true)",
        long,
        value_name = "BOOL"
    )]
    pub fail_on_flag: Option<String>,
    #[arg(
        help = "now | commit | YYYY-MM-DD (default commit)",
        long,
        value_name = "WHEN"
    )]
    pub reference_date: Option<String>,
    #[arg(
        help = "Report comments expiring within this duration without failing",
        long,
        value_name = "DURATION"
    )]
    pub warn_within: Option<String>,
    #[arg(
        help = "Baseline file of grandfathered comments",
        long,
        value_name = "FILE"
    )]
    pub baseline: Option<String>,
    #[arg(help = "all | changed (default all)", long, value_name = "SCOPE")]
    pub scope: Option<String>,
    #[arg(help = "fail | blame (default fail)", long, value_name = "MODE")]
    pub undated: Option<String>,
    #[arg(
        help = "Emit GitHub annotations (default true)",
        long,
        value_name = "BOOL"
    )]
    pub annotations: Option<String>,
}

impl ScanArgs {
    pub fn layer(&self) -> Layer {
        Layer {
            comment_ttl: self.comment_ttl.clone(),
            scan_dirs: self.scan_dirs.clone(),
            file_types: self.file_types.clone(),
            exempt_paths: self.exempt_paths.clone(),
            comment_formats: self.comment_formats.clone().map(Value::String),
            exemptions: self.exemptions.clone().map(Value::String),
            output_type: self.output_type.clone(),
            fail_on_flag: self.fail_on_flag.clone().map(Value::String),
            reference_date: self.reference_date.clone(),
            warn_within: self.warn_within.clone(),
            baseline: self.baseline.clone(),
            scope: self.scope.clone(),
            undated: self.undated.clone(),
            annotations: self.annotations.clone().map(Value::String),
        }
    }
}

/// 2026-09-24: The `INPUT_*` layer. Empty values count as unset, which is
/// how a composite action passes an input the caller left out.
pub fn layer_from_env(get: &dyn Fn(&str) -> Option<String>) -> Layer {
    let s = |name: &str| {
        get(&format!("INPUT_{}", name.to_ascii_uppercase())).filter(|v| !v.trim().is_empty())
    };
    let v = |name: &str| s(name).map(Value::String);
    Layer {
        comment_ttl: s("comment_ttl"),
        scan_dirs: s("scan_dirs"),
        file_types: s("file_types"),
        exempt_paths: s("exempt_paths"),
        comment_formats: v("comment_formats"),
        exemptions: v("exemptions"),
        output_type: s("output_type"),
        fail_on_flag: v("fail_on_flag"),
        reference_date: s("reference_date"),
        warn_within: s("warn_within"),
        baseline: s("baseline"),
        scope: s("scope"),
        undated: s("undated"),
        annotations: v("annotations"),
    }
}

pub fn invocation(cli: Cli, env: &dyn Fn(&str) -> Option<String>) -> Invocation {
    let (mode, args) = match cli.command {
        Command::Check(a) => (Mode::Check, a),
        Command::Baseline(a) => (Mode::WriteBaseline, a),
    };
    let inputs = args.layer().merged_over(layer_from_env(env));
    let diff_base = args.diff_base.clone().or_else(|| {
        env("INPUT_DIFF_BASE")
            .filter(|v| !v.trim().is_empty())
            .or_else(|| {
                env("GITHUB_BASE_REF")
                    .filter(|v| !v.is_empty())
                    .map(|b| format!("origin/{b}"))
            })
    });
    Invocation {
        mode,
        repo: args.repo.clone().unwrap_or_else(|| PathBuf::from(".")),
        config_path: args.config.clone(),
        inputs,
        out: args.out.clone(),
        diff_base,
        github: GithubFiles {
            output: env("GITHUB_OUTPUT")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
            step_summary: env("GITHUB_STEP_SUMMARY")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_layer_ignores_empty_values_and_flags_win() {
        let env = |k: &str| match k {
            "INPUT_COMMENT_TTL" => Some("30d".to_string()),
            "INPUT_SCOPE" => Some("".to_string()),
            "INPUT_FAIL_ON_FLAG" => Some("false".to_string()),
            _ => None,
        };
        let layer = layer_from_env(&env);
        assert_eq!(layer.comment_ttl.as_deref(), Some("30d"));
        assert_eq!(layer.scope, None);
        assert_eq!(layer.fail_on_flag, Some(Value::String("false".into())));

        let cli = Cli::parse_from([
            "comment-ttl",
            "check",
            "--comment-ttl",
            "2w",
            "--scope",
            "changed",
        ]);
        let inv = invocation(cli, &env);
        assert_eq!(inv.inputs.comment_ttl.as_deref(), Some("2w"));
        assert_eq!(inv.inputs.scope.as_deref(), Some("changed"));
        assert_eq!(inv.inputs.fail_on_flag, Some(Value::String("false".into())));
        assert!(inv.diff_base.is_none());
    }

    #[test]
    fn diff_base_falls_back_to_the_pull_request_base_ref() {
        let env = |k: &str| (k == "GITHUB_BASE_REF").then(|| "main".to_string());
        let cli = Cli::parse_from(["comment-ttl", "check"]);
        assert_eq!(
            invocation(cli, &env).diff_base.as_deref(),
            Some("origin/main")
        );
        let cli = Cli::parse_from(["comment-ttl", "check", "--diff-base", "abc"]);
        assert_eq!(invocation(cli, &env).diff_base.as_deref(), Some("abc"));
    }
}
