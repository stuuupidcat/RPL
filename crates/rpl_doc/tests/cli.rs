//! End-to-end tests driving the `cargo-rpl doc` binary.

#![feature(rustc_private)]

use std::path::Path;
use std::process::Command;

fn cargo_rpl_bin() -> std::path::PathBuf {
    // The `CARGO_BIN_EXE_cargo-rpl` env var is only set when the binary lives
    // in the same package as the test. Since `cargo-rpl` is defined in the
    // workspace root package while this test crate is `rpl_doc`, we locate
    // the binary from the workspace `target/` tree at runtime.
    //
    // Strategy (in priority order):
    // 1. `CARGO_BIN_EXE_cargo-rpl` — set when Cargo does supply it (e.g. if
    //    the project is restructured in the future).
    // 2. Walk up from `CARGO_MANIFEST_DIR` to find the workspace root, then
    //    locate `target/{profile}/cargo-rpl`.
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_cargo-rpl") {
        return std::path::PathBuf::from(p);
    }

    // CARGO_MANIFEST_DIR points to crates/rpl_doc; go up two levels to
    // reach the workspace root.
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent() // crates/
        .and_then(|p| p.parent()) // workspace root
        .expect("could not locate workspace root from CARGO_MANIFEST_DIR");

    // Prefer the same profile the test was built with.  We approximate
    // this by checking PROFILE env var (set by some CI setups) or falling
    // back to "debug".
    let profile = std::env::var("CARGO_PROFILE").unwrap_or_else(|_| "debug".to_string());
    let mut bin = workspace_root.join("target").join(&profile).join("cargo-rpl");

    if cfg!(windows) {
        bin.set_extension("exe");
    }

    // If the profiled path doesn't exist, fall back to debug.
    if !bin.exists() && profile != "debug" {
        let mut fallback = workspace_root.join("target").join("debug").join("cargo-rpl");
        if cfg!(windows) {
            fallback.set_extension("exe");
        }
        if fallback.exists() {
            return fallback;
        }
    }

    bin
}

#[test]
fn single_file_mode_writes_md_next_to_rpl() {
    let td = tempfile::TempDir::new().unwrap();
    let rpl = td.path().join("Foo.rpl");
    std::fs::write(&rpl, "pattern Foo\n").unwrap();

    let status = Command::new(cargo_rpl_bin())
        .arg("rpl")
        .arg("doc")
        .arg(&rpl)
        .arg("--quiet")
        .status()
        .expect("spawn");
    assert!(status.success());

    let out = td.path().join("Foo.md");
    let md = std::fs::read_to_string(&out).expect("expected output file");
    assert!(md.starts_with("# Foo"));
}

#[test]
fn directory_mode_walks_recursively() {
    let td = tempfile::TempDir::new().unwrap();
    std::fs::write(td.path().join("A.rpl"), "pattern A\n").unwrap();
    let sub = td.path().join("nested");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(sub.join("B.rpl"), "pattern B\n").unwrap();

    let status = Command::new(cargo_rpl_bin())
        .arg("rpl")
        .arg("doc")
        .arg(td.path())
        .arg("--quiet")
        .status()
        .expect("spawn");
    assert!(status.success());

    assert!(td.path().join("A.md").exists());
    assert!(sub.join("B.md").exists());
}

#[test]
fn parse_error_exits_nonzero() {
    let td = tempfile::TempDir::new().unwrap();
    let rpl = td.path().join("Bad.rpl");
    std::fs::write(&rpl, "garbage not a pattern\n").unwrap();

    let status = Command::new(cargo_rpl_bin())
        .arg("rpl")
        .arg("doc")
        .arg(&rpl)
        .status()
        .expect("spawn");
    assert!(!status.success());
}

#[test]
fn output_flag_mirrors_input_tree() {
    let td_in = tempfile::TempDir::new().unwrap();
    let td_out = tempfile::TempDir::new().unwrap();
    std::fs::write(td_in.path().join("A.rpl"), "pattern A\n").unwrap();
    let sub = td_in.path().join("nested");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(sub.join("B.rpl"), "pattern B\n").unwrap();

    let status = Command::new(cargo_rpl_bin())
        .arg("rpl")
        .arg("doc")
        .arg(td_in.path())
        .arg("--output")
        .arg(td_out.path())
        .arg("--quiet")
        .status()
        .expect("spawn");
    assert!(status.success());

    assert!(td_out.path().join("A.md").exists());
    assert!(td_out.path().join("nested/B.md").exists());
    // Input tree was not modified.
    assert!(!td_in.path().join("A.md").exists());
}
