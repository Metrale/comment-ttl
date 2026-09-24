// 2026-09-24: Every git invocation. Output parsing is separated from process
// spawning so the parsers are testable on captured text.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use time::Date;

use crate::calendar::date_of_unix;

fn git(repo: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    String::from_utf8(out.stdout).map_err(|_| "git produced non-UTF-8 output".to_string())
}

pub fn ls_files(repo: &Path) -> Result<Vec<String>, String> {
    let out = git(repo, &["ls-files", "-z", "--cached"])?;
    Ok(out
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect())
}

/// 2026-09-24: The head commit's committer time as a UTC date.
pub fn head_commit_date(repo: &Path) -> Result<Date, String> {
    let out = git(repo, &["log", "-1", "--format=%ct", "HEAD"])?;
    let ts: i64 = out
        .trim()
        .parse()
        .map_err(|_| format!("unexpected commit timestamp {:?}", out.trim()))?;
    date_of_unix(ts).map_err(|e| e.to_string())
}

pub fn head_sha(repo: &Path) -> Result<String, String> {
    git(repo, &["rev-parse", "HEAD"]).map(|s| s.trim().to_string())
}

/// 2026-09-24: Added or modified line ranges per file between `base` and
/// HEAD, using the merge base (`base...HEAD`).
pub fn changed_ranges(
    repo: &Path,
    base: &str,
) -> Result<HashMap<String, Vec<(usize, usize)>>, String> {
    let range = format!("{base}...HEAD");
    let out = git(
        repo,
        &[
            "diff",
            "--unified=0",
            "--no-color",
            "--no-ext-diff",
            "--diff-filter=AM",
            &range,
        ],
    )?;
    Ok(parse_hunks(&out))
}

pub fn parse_hunks(diff: &str) -> HashMap<String, Vec<(usize, usize)>> {
    let mut map: HashMap<String, Vec<(usize, usize)>> = HashMap::new();
    let mut current: Option<String> = None;
    for line in diff.lines() {
        if let Some(path) = line.strip_prefix("+++ b/") {
            current = Some(path.to_string());
        } else if line.starts_with("+++ ") {
            current = None;
        } else if let Some(rest) = line.strip_prefix("@@ ") {
            let Some(plus) = rest.split(' ').find_map(|f| f.strip_prefix('+')) else {
                continue;
            };
            let (start, count) = match plus.split_once(',') {
                Some((s, c)) => (s.parse().unwrap_or(0), c.parse().unwrap_or(0)),
                None => (plus.parse().unwrap_or(0), 1usize),
            };
            if count == 0 {
                continue;
            }
            if let Some(path) = &current {
                map.entry(path.clone())
                    .or_default()
                    .push((start, start + count - 1));
            }
        }
    }
    map
}

/// 2026-09-24: For each requested inclusive range, the newest committer date
/// among its lines according to `git blame`.
pub fn blame_dates(
    repo: &Path,
    path: &str,
    ranges: &[(usize, usize)],
) -> Result<Vec<Option<Date>>, String> {
    let mut args = vec!["blame".to_string(), "--line-porcelain".to_string()];
    for (s, e) in ranges {
        args.push("-L".to_string());
        args.push(format!("{s},{e}"));
    }
    args.push("HEAD".to_string());
    args.push("--".to_string());
    args.push(path.to_string());
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = git(repo, &arg_refs)?;
    let times = parse_blame_times(&out);
    Ok(ranges
        .iter()
        .map(|&(s, e)| {
            (s..=e)
                .filter_map(|l| times.get(&l).copied())
                .max()
                .and_then(|ts| date_of_unix(ts).ok())
        })
        .collect())
}

/// 2026-09-24: Maps final line number to committer time from
/// `git blame --line-porcelain` output.
pub fn parse_blame_times(porcelain: &str) -> HashMap<usize, i64> {
    let mut map = HashMap::new();
    let mut current_line: Option<usize> = None;
    for line in porcelain.lines() {
        if let Some(ts) = line.strip_prefix("committer-time ") {
            if let (Some(l), Ok(t)) = (current_line, ts.trim().parse::<i64>()) {
                map.insert(l, t);
            }
        } else if line.len() > 41
            && line.as_bytes()[40] == b' '
            && line[..40].bytes().all(|b| b.is_ascii_hexdigit())
        {
            current_line = line.split(' ').nth(2).and_then(|n| n.parse().ok());
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hunks_are_parsed_per_file_with_new_side_ranges() {
        let diff = "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1,2 +1,3 @@\n+x\n@@ -10 +12,0 @@\n@@ -20,0 +23 @@\n+y\ndiff --git a/new.py b/new.py\n--- /dev/null\n+++ b/new.py\n@@ -0,0 +1,5 @@\n+z\n";
        let hunks = parse_hunks(diff);
        assert_eq!(hunks["a.rs"], vec![(1, 3), (23, 23)]);
        assert_eq!(hunks["new.py"], vec![(1, 5)]);
        assert_eq!(hunks.len(), 2);
    }

    #[test]
    fn blame_times_follow_the_final_line_number() {
        let porcelain = "0123456789012345678901234567890123456789 1 7 1\nauthor A\ncommitter-time 1790000000\n\tline\n0123456789012345678901234567890123456789 2 8\ncommitter-time 1700000000\n\tline\n";
        let times = parse_blame_times(porcelain);
        assert_eq!(times[&7], 1_790_000_000);
        assert_eq!(times[&8], 1_700_000_000);
        assert_eq!(times.len(), 2);
    }
}
