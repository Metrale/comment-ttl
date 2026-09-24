// 2026-09-24: CLI report tests: every output type, the job summary,
// annotations and outputs, reference-date handling and exit codes.

mod common;

use std::fs;

use common::*;
use serde_json::Value;

fn settings() -> insta::Settings {
    let mut s = insta::Settings::clone_current();
    s.set_snapshot_path("snapshots/cli");
    s.set_prepend_module_to_snapshot(false);
    s.add_filter(
        r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z",
        "[timestamp]",
    );
    s.add_filter(r#""version": "[^"]+""#, r#""version": "[version]""#);
    s.add_filter(r"comment-ttl \d+\.\d+\.\d+", "comment-ttl [version]");
    s.add_filter(r#"[A-Za-z]:\\[^\s"]+"#, "[tmp]");
    s.add_filter(r#"/[^\s"]*/\.tmp[^\s"/]*"#, "[tmp]");
    s
}

#[test]
fn json_report_matches_expected_counts_and_snapshot() {
    let r = repo();
    let a = check(
        r.path(),
        &["--comment-ttl", "1 month", "--warn-within", "7d"],
    )
    .code(1);
    let report = report_of(&a);
    assert_eq!(report["counts"], expected()["counts"]);
    assert_eq!(report["reference_source"], "fixed");
    assert_eq!(report["comment_ttl_iso"], "P1M");
    settings().bind(|| {
        insta::assert_snapshot!(
            "report_json",
            String::from_utf8_lossy(&a.get_output().stdout)
        )
    });
}

#[test]
fn reference_date_commit_uses_the_head_committer_date_in_utc() {
    let r = repo();
    let a = cmd(r.path())
        .args([
            "check",
            "--comment-ttl",
            "1 month",
            "--annotations",
            "false",
        ])
        .assert()
        .code(1);
    let report = report_of(&a);
    assert_eq!(
        report["reference_date"], "2026-09-24",
        "01:00+03:00 on the 25th is the 24th in UTC"
    );
    assert_eq!(report["reference_source"], "commit");
}

#[test]
fn fail_on_flag_false_still_reports_but_exits_zero() {
    let r = repo();
    let a = check(
        r.path(),
        &["--comment-ttl", "1 month", "--fail-on-flag", "false"],
    )
    .code(0);
    assert_eq!(counts_of(&a)["flagged"], 8);
}

#[test]
fn every_output_type_renders_the_same_findings() {
    let r = repo();
    for kind in ["csv", "sarif", "markdown"] {
        let out = r.path().join(format!("report.{kind}"));
        check(
            r.path(),
            &[
                "--comment-ttl",
                "1 month",
                "--warn-within",
                "7d",
                "--output-type",
                kind,
                "--out",
                out.to_str().unwrap(),
            ],
        )
        .code(1);
        let text = fs::read_to_string(&out).unwrap();
        settings().bind(|| insta::assert_snapshot!(format!("report_{kind}"), text));
    }
    let sarif: Value =
        serde_json::from_str(&fs::read_to_string(r.path().join("report.sarif")).unwrap()).unwrap();
    assert_eq!(
        sarif["runs"][0]["results"].as_array().unwrap().len(),
        10,
        "8 flagged + 1 expiring + 1 escape"
    );
}

#[test]
fn github_files_receive_annotations_summary_and_outputs() {
    let r = repo();
    let output = r.path().join("gh_output.txt");
    let summary = r.path().join("gh_summary.md");
    let report = r.path().join("report.json");
    let a = cmd(r.path())
        .args([
            "check",
            "--comment-ttl",
            "1 month",
            "--warn-within",
            "7d",
            "--reference-date",
            "2026-09-24",
        ])
        .args([
            "--exempt-paths",
            expected()["exempt_paths"].as_str().unwrap(),
        ])
        .args(["--out", report.to_str().unwrap()])
        .env("GITHUB_OUTPUT", &output)
        .env("GITHUB_STEP_SUMMARY", &summary)
        .assert()
        .code(1);
    let stdout = String::from_utf8_lossy(&a.get_output().stdout).into_owned();
    assert_eq!(stdout.matches("::error ").count(), 8);
    assert_eq!(stdout.matches("::warning ").count(), 1);
    settings().bind(|| {
        insta::assert_snapshot!("annotations", stdout);
        insta::assert_snapshot!("job_summary", fs::read_to_string(&summary).unwrap());
    });
    let outputs = fs::read_to_string(&output).unwrap();
    for (key, value) in [
        ("flagged", "8"),
        ("stale", "2"),
        ("undated", "3"),
        ("malformed", "2"),
        ("future", "1"),
        ("exempt", "4"),
        ("expiring", "1"),
    ] {
        assert!(
            outputs.contains(&format!("{key}={value}\n")),
            "{key}: {outputs}"
        );
    }
    assert!(outputs.contains(&format!("report_path={}\n", report.display())));
    assert!(report.exists());
}

#[test]
fn annotations_are_capped_at_ten_with_a_notice() {
    let r = repo();
    let many: String = (1..=12)
        .map(|i| format!("// undated number {i}\nfn f{i}() {{}}\n"))
        .collect();
    fs::write(r.path().join("src/many.rs"), many).unwrap();
    commit_all(r.path(), "many");
    let a = cmd(r.path())
        .args([
            "check",
            "--comment-ttl",
            "1 month",
            "--reference-date",
            "2026-09-24",
            "--out",
            "r.json",
        ])
        .args(["--scan-dirs", "^src/", "--file-types", "many"])
        .assert()
        .code(1);
    let stdout = String::from_utf8_lossy(&a.get_output().stdout).into_owned();
    assert_eq!(stdout.matches("::error ").count(), 10);
    assert!(stdout.contains("::notice title=comment-ttl::10 of 12 findings annotated"));
}
