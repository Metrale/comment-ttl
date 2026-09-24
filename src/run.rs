// 2026-09-24: Orchestration: reads the config file, resolves the reference
// date, lists files through git, feeds file text to the pure scan, and
// writes the report, annotations, summary and outputs.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::baseline::{self, Baseline, BaselineEntry};
use crate::calendar::{format_ymd, today_utc};
use crate::classify::Rules;
use crate::config::{
    CONFIG_FILE_NAME, Config, Layer, OutputType, ReferenceDate, Scope, UndatedMode,
};
use crate::git;
use crate::report::{self, Report, github, markdown, sarif};
use crate::scan::{self, Classified, Counts, Finding};

pub const EXIT_OK: i32 = 0;
pub const EXIT_FLAGGED: i32 = 1;
pub const EXIT_CONFIG: i32 = 2;

pub enum Mode {
    Check,
    /// 2026-09-24: Write the currently flagged comments as a baseline file.
    WriteBaseline,
}

/// 2026-09-24: Paths of the GitHub runner files, when running in Actions.
#[derive(Default)]
pub struct GithubFiles {
    pub output: Option<PathBuf>,
    pub step_summary: Option<PathBuf>,
}

pub struct Invocation {
    pub mode: Mode,
    pub repo: PathBuf,
    pub config_path: Option<PathBuf>,
    pub inputs: Layer,
    pub out: Option<PathBuf>,
    pub diff_base: Option<String>,
    pub github: GithubFiles,
}

pub struct Outcome {
    pub exit_code: i32,
    pub report: Report,
}

fn load_file_layer(inv: &Invocation) -> Result<Layer, String> {
    let path = match &inv.config_path {
        Some(p) => p.clone(),
        None => {
            let default = inv.repo.join(CONFIG_FILE_NAME);
            if !default.exists() {
                return Ok(Layer::default());
            }
            default
        }
    };
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Layer::from_toml(&text)
}

fn resolve_reference(cfg: &Config, repo: &Path) -> Result<(time::Date, &'static str), String> {
    Ok(match &cfg.reference_date {
        ReferenceDate::Now => (today_utc(), "now"),
        ReferenceDate::Commit => (git::head_commit_date(repo)?, "commit"),
        ReferenceDate::Fixed(d) => (*d, "fixed"),
    })
}

fn load_baseline(cfg: &Config, repo: &Path) -> Result<Option<Baseline>, String> {
    let Some(rel) = &cfg.baseline else {
        return Ok(None);
    };
    let path = repo.join(rel);
    let text =
        fs::read_to_string(&path).map_err(|e| format!("baseline {}: {e}", path.display()))?;
    Baseline::parse(&text).map(Some)
}

fn changed_ranges(inv: &Invocation) -> Result<HashMap<String, Vec<(usize, usize)>>, String> {
    let base = inv.diff_base.clone().ok_or(
        "scope is `changed` but no diff base is known: pass --diff-base <ref> (the action derives \
         it from the pull request base; the base commit must be fetched, e.g. `fetch-depth: 0`)",
    )?;
    git::changed_ranges(&inv.repo, &base)
}

/// 2026-09-24: Classified comments grouped by file path, in `git ls-files` order.
type PerFile = Vec<(String, Vec<Classified>)>;

fn scan_repo(
    inv: &Invocation,
    cfg: &Config,
    rules: &Rules<'_>,
) -> Result<(Counts, PerFile), String> {
    let ranges = match cfg.scope {
        Scope::All => None,
        Scope::Changed => Some(changed_ranges(inv)?),
    };
    let mut counts = Counts::default();
    let mut per_file = Vec::new();
    for path in git::ls_files(&inv.repo)? {
        if !cfg.path_selected(&path) || cfg.formats.for_path(&path).is_none() {
            continue;
        }
        let file_ranges = match &ranges {
            None => None,
            Some(map) => match map.get(&path) {
                Some(r) => Some(r.as_slice()),
                None => continue,
            },
        };
        let Ok(bytes) = fs::read(inv.repo.join(&path)) else {
            continue;
        };
        if scan::is_binary(&bytes) {
            counts.files_skipped_binary += 1;
            continue;
        }
        let content = String::from_utf8_lossy(&bytes);
        let Some(mut items) = scan::analyze_file(&path, &content, cfg, rules, file_ranges) else {
            continue;
        };
        counts.files_scanned += 1;
        if cfg.undated == UndatedMode::Blame {
            apply_blame(&inv.repo, &path, &mut items, rules)?;
        }
        per_file.push((path, items));
    }
    Ok((counts, per_file))
}

fn apply_blame(
    repo: &Path,
    path: &str,
    items: &mut [Classified],
    rules: &Rules<'_>,
) -> Result<(), String> {
    let undated: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, i)| i.verdict.classification == crate::classify::Classification::Undated)
        .map(|(idx, _)| idx)
        .collect();
    if undated.is_empty() {
        return Ok(());
    }
    let ranges: Vec<(usize, usize)> = undated
        .iter()
        .map(|&i| (items[i].comment.line, items[i].comment.end_line))
        .collect();
    let dates = git::blame_dates(repo, path, &ranges)?;
    for (idx, date) in undated.into_iter().zip(dates) {
        if let Some(d) = date {
            scan::reclassify_with_date(path, &mut items[idx], rules, d);
        }
    }
    Ok(())
}

fn build_report(
    cfg: &Config,
    reference: time::Date,
    source: &str,
    mut counts: Counts,
    per_file: &PerFile,
) -> Report {
    let mut findings: Vec<Finding> = Vec::new();
    for (path, items) in per_file {
        for item in items {
            counts.record(item.verdict.classification);
            if scan::is_reported(item.verdict.classification) {
                findings.push(scan::finding(path, item));
            }
        }
    }
    Report {
        tool: report::TOOL_NAME.to_string(),
        version: report::TOOL_VERSION.to_string(),
        generated_at: OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_default(),
        reference_date: format_ymd(reference),
        reference_source: source.to_string(),
        comment_ttl: cfg.ttl_text.clone(),
        comment_ttl_iso: cfg.ttl.to_iso(),
        warn_within: cfg.warn_within_text.clone(),
        scope: match cfg.scope {
            Scope::All => "all".into(),
            Scope::Changed => "changed".into(),
        },
        counts,
        findings,
    }
}

fn render(report: &Report, output_type: OutputType) -> String {
    match output_type {
        OutputType::Json => report.to_json(),
        OutputType::Csv => report.to_csv(),
        OutputType::Sarif => sarif::to_sarif(report),
        OutputType::Markdown => markdown::to_markdown(report),
    }
}

fn write_or_print(path: Option<&Path>, content: &str) -> Result<String, String> {
    match path {
        Some(p) => {
            if let Some(parent) = p.parent().filter(|d| !d.as_os_str().is_empty()) {
                fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
            }
            fs::write(p, content).map_err(|e| format!("{}: {e}", p.display()))?;
            Ok(p.display().to_string())
        }
        None => {
            let mut stdout = std::io::stdout().lock();
            stdout
                .write_all(content.as_bytes())
                .map_err(|e| format!("stdout: {e}"))?;
            Ok(String::from("-"))
        }
    }
}

fn append(path: &Path, content: &str) -> Result<(), String> {
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    f.write_all(content.as_bytes())
        .map_err(|e| format!("{}: {e}", path.display()))
}

pub fn run(inv: Invocation) -> Result<Outcome, String> {
    let file_layer = load_file_layer(&inv)?;
    let cfg = Config::from_layer(inv.inputs.clone().merged_over(file_layer))?;
    let (reference, source) = resolve_reference(&cfg, &inv.repo)?;
    let baseline = load_baseline(&cfg, &inv.repo)?;
    let rules = Rules {
        reference,
        ttl: cfg.ttl,
        warn_within: cfg.warn_within,
        exemptions: &cfg.exemptions,
        baseline: baseline.as_ref(),
    };
    let (counts, per_file) = scan_repo(&inv, &cfg, &rules)?;
    let report = build_report(&cfg, reference, source, counts, &per_file);

    if matches!(inv.mode, Mode::WriteBaseline) {
        let entries: Vec<BaselineEntry> = report
            .findings
            .iter()
            .filter(|f| f.classification.is_flagged())
            .map(|f| BaselineEntry {
                path: f.path.clone(),
                fingerprint: f.fingerprint.clone(),
                line: Some(f.line),
                excerpt: Some(f.text.clone()),
            })
            .collect();
        let n = entries.len();
        let where_ = write_or_print(inv.out.as_deref(), &baseline::render(entries))?;
        eprintln!("comment-ttl: wrote baseline with {n} grandfathered comment(s) to {where_}");
        return Ok(Outcome {
            exit_code: EXIT_OK,
            report,
        });
    }

    let failed = report.counts.flagged > 0 && cfg.fail_on_flag;
    let report_path = write_or_print(inv.out.as_deref(), &render(&report, cfg.output_type))?;
    if cfg.annotations {
        let mut stdout = std::io::stdout().lock();
        for line in github::annotations(&report) {
            writeln!(stdout, "{line}").map_err(|e| format!("stdout: {e}"))?;
        }
    }
    if let Some(summary) = &inv.github.step_summary {
        append(summary, &markdown::to_summary(&report, failed))?;
    }
    if let Some(output) = &inv.github.output {
        let mut text = String::new();
        for (k, v) in github::outputs(&report, &report_path) {
            text.push_str(&format!("{k}={v}\n"));
        }
        append(output, &text)?;
    }
    let c = &report.counts;
    eprintln!(
        "comment-ttl: {} comment(s) in {} file(s): {} flagged ({} stale, {} undated, {} malformed, {} future), {} expiring, {} escaped, {} exempt, {} fresh",
        c.comments,
        c.files_scanned,
        c.flagged,
        c.stale,
        c.undated,
        c.malformed,
        c.future,
        c.expiring,
        c.escape,
        c.exempt,
        c.fresh
    );
    Ok(Outcome {
        exit_code: if failed { EXIT_FLAGGED } else { EXIT_OK },
        report,
    })
}
