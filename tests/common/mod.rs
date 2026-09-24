// 2026-09-24: Shared helpers for the CLI tests: the e2e fixture tree is
// committed into a temporary git repository with a fixed committer date and
// driven through the real binary. Counts are asserted against
// tests/fixtures/e2e/expected.json (the file the e2e workflow checks too).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as Std;

use assert_cmd::Command;
use assert_cmd::assert::Assert;
use serde_json::Value;
use tempfile::TempDir;

pub const FIXED_COMMIT_DATE: &str = "2026-09-25T01:00:00+03:00";

pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/e2e")
}

pub fn expected() -> Value {
    serde_json::from_str(&fs::read_to_string(fixtures().join("expected.json")).unwrap()).unwrap()
}

pub fn copy_dir(from: &Path, to: &Path) {
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            fs::create_dir_all(&target).unwrap();
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

pub fn git(repo: &Path, args: &[&str]) {
    let status = Std::new("git")
        .args(args)
        .current_dir(repo)
        .env("GIT_AUTHOR_DATE", FIXED_COMMIT_DATE)
        .env("GIT_COMMITTER_DATE", FIXED_COMMIT_DATE)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

pub fn commit_all(repo: &Path, message: &str) {
    git(repo, &["add", "-A"]);
    git(
        repo,
        &["-c", "commit.gpgsign=false", "commit", "-q", "-m", message],
    );
}

pub fn repo() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(&fixtures(), dir.path());
    git(dir.path(), &["init", "-q", "-b", "main"]);
    commit_all(dir.path(), "fixtures");
    dir
}

pub fn cmd(repo: &Path) -> Command {
    let mut c = Command::cargo_bin("comment-ttl").unwrap();
    c.current_dir(repo);
    for (k, _) in std::env::vars() {
        if k.starts_with("INPUT_") || k.starts_with("GITHUB_") {
            c.env_remove(k);
        }
    }
    c
}

pub fn check(repo: &Path, args: &[&str]) -> Assert {
    cmd(repo)
        .arg("check")
        .args(["--reference-date", "2026-09-24", "--annotations", "false"])
        .arg("--exempt-paths")
        .arg(expected()["exempt_paths"].as_str().unwrap())
        .args(args)
        .assert()
}

pub fn report_of(assert: &Assert) -> Value {
    serde_json::from_slice(&assert.get_output().stdout).unwrap()
}

pub fn counts_of(assert: &Assert) -> Value {
    report_of(assert)["counts"].clone()
}
