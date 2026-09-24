// 2026-09-24: Golden lexer fixtures: every file under tests/fixtures/lexer is
// lexed with the default format registry and its comments are snapshotted.
// Reviewing a snapshot diff is reviewing the lexer's behaviour for that
// language, including which strings and template literals it skipped.

use std::fs;
use std::path::Path;

use comment_ttl::formats::FormatRegistry;
use comment_ttl::lexer::lex;

#[test]
fn every_fixture_has_a_format_and_a_stable_comment_list() {
    let registry = FormatRegistry::new(&[]).unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/lexer");
    let mut names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert!(
        names.len() >= 17,
        "fixture directory looks incomplete: {names:?}"
    );
    for name in names {
        let content = fs::read_to_string(dir.join(&name)).unwrap();
        let syntax = registry
            .for_path(&name)
            .unwrap_or_else(|| panic!("no default format for fixture {name}"));
        let comments = lex(&content, syntax);
        assert!(!comments.is_empty(), "{name}: lexer found no comments");
        assert!(
            comments.iter().all(|c| !c.raw.contains("DECOY")),
            "{name}: a DECOY string leaked into the comment list: {comments:#?}"
        );
        assert!(
            content.contains("DECOY"),
            "{name}: every fixture must contain at least one DECOY string"
        );
        insta::with_settings!({ snapshot_path => "snapshots/lexer", prepend_module_to_snapshot => false }, {
            insta::assert_yaml_snapshot!(name.replace('.', "_"), comments);
        });
    }
}
