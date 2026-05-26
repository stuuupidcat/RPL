//! Round-trip integration tests: parse + extract + render for paired fixtures.

#![feature(rustc_private)]

use std::path::Path;

use pretty_assertions::assert_eq;

fn fixture_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn assert_fixture(name: &str) {
    let rpl = fixture_dir().join(format!("{name}.rpl"));
    let expected_path = fixture_dir().join(format!("{name}.expected.md"));
    let expected = std::fs::read_to_string(&expected_path)
        .unwrap_or_else(|_| panic!("missing fixture {}", expected_path.display()));
    let actual =
        rpl_doc::render_markdown(&rpl).unwrap_or_else(|e| panic!("render_markdown({}) failed: {e}", rpl.display()));
    assert_eq!(actual, expected, "fixture: {name}");
}

#[test]
fn minimal() {
    assert_fixture("minimal");
}
#[test]
fn patt_with_doc() {
    assert_fixture("patt_with_doc");
}
#[test]
fn diag_with_doc() {
    assert_fixture("diag_with_doc");
}
#[test]
fn with_examples() {
    assert_fixture("with_examples");
}
#[test]
fn body_with_backticks() {
    assert_fixture("body_with_backticks");
}
#[test]
fn multi_block() {
    assert_fixture("multi_block");
}
