// 2026-09-24: CLI configuration tests: the config file, configuration
// errors, baselines, changed scope, blame, path filters, formats/exemptions
// and action-style environment inputs.

mod common;

use std::fs;

use common::*;
use predicates::prelude::*;
use serde_json::Value;

#[test]
fn config_file_supplies_defaults_and_flags_win() {
    let r = repo();
    fs::write(
        r.path().join(".comment-ttl.toml"),
        "comment_ttl = \"10 years\"\nfail_on_flag = false\nannotations = false\nexempt_paths = \"(^|/)generated/\"\n",
    )
    .unwrap();
    let a = cmd(r.path())
        .args(["check", "--reference-date", "2026-09-24"])
        .assert()
        .code(0);
    let c = counts_of(&a);
    assert_eq!(c["stale"], 0, "a ten-year TTL makes nothing stale");
    assert_eq!(c["flagged"], 6);
    let a = cmd(r.path())
        .args([
            "check",
            "--reference-date",
            "2026-09-24",
            "--comment-ttl",
            "1 month",
            "--fail-on-flag",
            "true",
        ])
        .assert()
        .code(1);
    assert_eq!(counts_of(&a)["stale"], 2);
}

#[test]
fn configuration_errors_exit_two_with_a_helpful_message() {
    let r = repo();
    cmd(r.path())
        .args(["check"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("comment_ttl is required"));
    cmd(r.path())
        .args(["check", "--comment-ttl", "2h"])
        .assert()
        .code(2)
        .stderr(
            predicate::str::contains("finer than a day")
                .and(predicate::str::contains("valid forms:")),
        );
    cmd(r.path())
        .args(["check", "--comment-ttl", "1mo", "--exempt-paths", "("])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("exempt_paths: invalid regex"));
    cmd(r.path())
        .args(["check", "--comment-ttl", "1mo", "--output-type", "xml"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "output_type: expected one of json|csv|sarif|markdown",
        ));
    cmd(r.path())
        .args([
            "check",
            "--comment-ttl",
            "1mo",
            "--reference-date",
            "yesterday",
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "reference_date: expected now|commit|YYYY-MM-DD",
        ));
    cmd(r.path())
        .args(["check", "--comment-ttl", "1mo", "--scope", "changed"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("no diff base is known"));
    fs::write(
        r.path().join(".comment-ttl.toml"),
        "comment_ttl = \"1mo\"\nmax_age = \"3mo\"\n",
    )
    .unwrap();
    cmd(r.path())
        .args(["check"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("unknown field `max_age`"));
    cmd(r.path())
        .env("GITHUB_ACTIONS", "true")
        .args(["check"])
        .assert()
        .code(2)
        .stdout(predicate::str::contains(
            "::error title=comment-ttl configuration error::",
        ));
}

#[test]
fn baseline_grandfathers_existing_flags_but_not_new_ones() {
    let r = repo();
    let baseline = r.path().join("baseline.json");
    cmd(r.path())
        .args([
            "baseline",
            "--comment-ttl",
            "1 month",
            "--reference-date",
            "2026-09-24",
        ])
        .args([
            "--exempt-paths",
            expected()["exempt_paths"].as_str().unwrap(),
            "--out",
            baseline.to_str().unwrap(),
        ])
        .assert()
        .code(0)
        .stderr(predicate::str::contains("8 grandfathered"));
    let a = check(
        r.path(),
        &["--comment-ttl", "1 month", "--baseline", "baseline.json"],
    )
    .code(0);
    let c = counts_of(&a);
    assert_eq!(c["flagged"], 0);
    assert_eq!(c["exempt"], 12);
    fs::write(r.path().join("src/new.rs"), "// brand new and undated\n").unwrap();
    commit_all(r.path(), "new");
    let a = check(
        r.path(),
        &["--comment-ttl", "1 month", "--baseline", "baseline.json"],
    )
    .code(1);
    let report = report_of(&a);
    assert_eq!(report["counts"]["flagged"], 1);
    let flagged: Vec<&Value> = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["classification"] == "undated")
        .collect();
    assert_eq!(flagged.len(), 1);
    assert_eq!(flagged[0]["path"], "src/new.rs");
    check(
        r.path(),
        &["--comment-ttl", "1 month", "--baseline", "missing.json"],
    )
    .code(2)
    .stderr(predicate::str::contains("baseline"));
}

#[test]
fn scope_changed_only_reports_comments_in_the_diff() {
    let r = repo();
    fs::write(
        r.path().join("src/new.rs"),
        "// added in the PR\nfn a() {}\n",
    )
    .unwrap();
    let lib = r.path().join("src/lib.rs");
    let mut text = fs::read_to_string(&lib).unwrap();
    text.push_str("\n// 2026-09-24: appended in the PR\npub fn appended() {}\n");
    fs::write(&lib, text).unwrap();
    commit_all(r.path(), "pr");
    let a = check(
        r.path(),
        &[
            "--comment-ttl",
            "1 month",
            "--scope",
            "changed",
            "--diff-base",
            "HEAD~1",
        ],
    )
    .code(1);
    let report = report_of(&a);
    assert_eq!(report["scope"], "changed");
    assert_eq!(
        report["counts"]["comments"], 2,
        "one new file comment plus one appended comment"
    );
    assert_eq!(report["counts"]["undated"], 1);
    assert_eq!(report["counts"]["fresh"], 1);
    assert_eq!(report["findings"][0]["path"], "src/new.rs");
    let a = cmd(r.path())
        .args([
            "check",
            "--comment-ttl",
            "1 month",
            "--reference-date",
            "2026-09-24",
            "--annotations",
            "false",
            "--scope",
            "changed",
        ])
        .env("GITHUB_BASE_REF", "main")
        .assert()
        .code(2);
    assert!(
        String::from_utf8_lossy(&a.get_output().stderr).contains("origin/main"),
        "the pull request base ref is used when no --diff-base is given"
    );
}

#[test]
fn undated_blame_dates_comments_from_git_history() {
    let r = repo();
    let a = check(
        r.path(),
        &[
            "--comment-ttl",
            "1 month",
            "--warn-within",
            "7d",
            "--undated",
            "blame",
        ],
    )
    .code(1);
    let c = counts_of(&a);
    assert_eq!(c["undated"], 0);
    assert_eq!(
        c["fresh"], 6,
        "three formerly undated comments are dated 2026-09-24 by blame"
    );
    assert_eq!(c["expiring"], 1);
    assert_eq!(c["flagged"], 5);
    let report = report_of(&a);
    assert!(
        report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["classification"] != "undated")
    );
}

#[test]
fn path_filters_select_files() {
    let r = repo();
    let a = check(r.path(), &["--comment-ttl", "1mo", "--scan-dirs", "^web/"]).code(1);
    assert_eq!(counts_of(&a)["files_scanned"], 1);
    let a = check(
        r.path(),
        &["--comment-ttl", "1mo", "--file-types", r"\.rs$"],
    )
    .code(1);
    assert_eq!(counts_of(&a)["files_scanned"], 1);
    let a = cmd(r.path())
        .args([
            "check",
            "--comment-ttl",
            "1mo",
            "--reference-date",
            "2026-09-24",
            "--annotations",
            "false",
            "--exempt-paths",
            ".",
        ])
        .assert()
        .code(0);
    assert_eq!(counts_of(&a)["files_scanned"], 0);
    let a = cmd(r.path())
        .args([
            "check",
            "--comment-ttl",
            "1mo",
            "--reference-date",
            "2026-09-24",
            "--annotations",
            "false",
        ])
        .assert()
        .code(1);
    assert_eq!(
        counts_of(&a)["files_scanned"],
        4,
        "without exempt_paths the generated file is scanned too"
    );
}

#[test]
fn comment_formats_and_exemptions_inputs_are_merged_over_the_defaults() {
    let r = repo();
    let a = check(
        r.path(),
        &[
            "--comment-ttl",
            "1mo",
            "--comment-formats",
            r#"{"\\.txt$": "hash"}"#,
        ],
    )
    .code(1);
    assert_eq!(
        counts_of(&a)["files_scanned"],
        4,
        "notes.txt is now scanned"
    );
    let a = check(
        r.path(),
        &[
            "--comment-ttl",
            "1mo",
            "--comment-formats",
            r#"{"\\.txt$": "cobol"}"#,
        ],
    )
    .code(2);
    assert!(String::from_utf8_lossy(&a.get_output().stderr).contains("unknown preset"));
    let a = check(
        r.path(),
        &["--comment-ttl", "1mo", "--exemptions", r#"["^// no-rot$"]"#],
    )
    .code(1);
    let c = counts_of(&a);
    assert_eq!(c["malformed"], 1, "the bare no-rot is now exempt");
    assert_eq!(c["exempt"], 5);
    let a = check(
        r.path(),
        &[
            "--comment-ttl",
            "1mo",
            "--exemptions",
            r#"{"replace": true, "patterns": []}"#,
        ],
    )
    .code(1);
    let c = counts_of(&a);
    assert_eq!(c["exempt"], 0);
    assert_eq!(
        c["undated"], 7,
        "SPDX, @deprecated, svelte-ignore and the shebang lose their exemptions"
    );
}

#[test]
fn action_inputs_arrive_through_the_environment() {
    let r = repo();
    let a = cmd(r.path())
        .arg("check")
        .env("INPUT_COMMENT_TTL", "1 month")
        .env("INPUT_REFERENCE_DATE", "2026-09-24")
        .env(
            "INPUT_EXEMPT_PATHS",
            expected()["exempt_paths"].as_str().unwrap(),
        )
        .env("INPUT_ANNOTATIONS", "false")
        .env("INPUT_WARN_WITHIN", "7d")
        .env("INPUT_SCOPE", "")
        .env("INPUT_FAIL_ON_FLAG", "")
        .assert()
        .code(1);
    assert_eq!(counts_of(&a), expected()["counts"]);
}
