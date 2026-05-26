//! Backward-compatibility gate: every .rpl file under docs/patterns-pest/
//! must parse successfully under the rpl_parser grammar.
//!
//! Today (pre-grammar-change) this test should pass against the unchanged
//! grammar. After the grammar change in Task B3, it must STILL pass —
//! that is its purpose: catch any regression in pattern-file acceptance.

#![feature(rustc_private)]

use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR points at crates/rpl_doc/; go up two levels.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

fn all_rpl_files() -> Vec<PathBuf> {
    let patterns_dir = workspace_root().join("docs/patterns-pest");
    walkdir::WalkDir::new(&patterns_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rpl"))
        .map(|e| e.into_path())
        .collect()
}

#[test]
fn existing_patterns_still_parse_after_grammar_change() {
    let files = all_rpl_files();
    assert!(
        !files.is_empty(),
        "expected at least one .rpl under docs/patterns-pest/"
    );

    let mut failures = Vec::new();
    for path in &files {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("read {}: {e}", path.display()));
                continue;
            },
        };
        if let Err(e) = rpl_doc::parse_only(path, &source) {
            failures.push(format!("parse {}: {e}", path.display()));
        }
    }

    if !failures.is_empty() {
        panic!(
            "{} of {} pattern files failed to parse:\n{}",
            failures.len(),
            files.len(),
            failures.join("\n")
        );
    }
}
