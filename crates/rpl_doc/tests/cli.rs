//! End-to-end tests driving the `cargo-rpl doc` binary.

#![feature(rustc_private)]

use std::path::Path;
use std::process::Command;

fn cargo_rpl_bin() -> std::path::PathBuf {
    // If Cargo set CARGO_BIN_EXE_cargo-rpl (it does for tests living in the
    // same package as the binary), use it directly.
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_cargo-rpl") {
        return std::path::PathBuf::from(path);
    }

    // Otherwise: this integration-test binary lives at
    //   target/<profile>/deps/cli-<hash>(.exe)
    // The cargo-rpl binary is a sibling of `deps/`:
    //   target/<profile>/cargo-rpl(.exe)
    let test_exe = std::env::current_exe().expect("current_exe");
    let target_profile = test_exe
        .parent() // .../target/<profile>/deps
        .and_then(Path::parent) // .../target/<profile>
        .expect("test binary should live under target/<profile>/deps/");
    let exe_name = if cfg!(windows) { "cargo-rpl.exe" } else { "cargo-rpl" };
    target_profile.join(exe_name)
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

#[test]
fn unknown_flag_exits_nonzero() {
    let td = tempfile::TempDir::new().unwrap();
    let rpl = td.path().join("Foo.rpl");
    std::fs::write(&rpl, "pattern Foo\n").unwrap();

    let status = Command::new(cargo_rpl_bin())
        .arg("rpl")
        .arg("doc")
        .arg(&rpl)
        .arg("--verbose") // unknown flag
        .status()
        .expect("spawn");
    assert!(!status.success(), "should reject --verbose");
}
