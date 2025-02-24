#![warn(rust_2018_idioms, unused_lifetimes)]
#![allow(unused_extern_crates)]

use ui_test::spanned::Spanned;
use ui_test::{status_emitter, Args, Config, Mode, OutputConflictHandling};

use std::collections::BTreeMap;
use std::env::{self, var_os};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

// Test dependencies may need an `extern crate` here to ensure that they show up
// in the depinfo file (otherwise cargo thinks they are unused)
// extern crate clippy_utils;

extern crate byte_slice_cast;
extern crate bytes;
// extern crate cassandra_cpp_sys;
extern crate futures;
extern crate if_chain;
extern crate itertools;
extern crate libc;
extern crate memmap;
extern crate num_derive;
extern crate num_rational;
extern crate num_traits;
extern crate parking_lot;
extern crate pin_project;
extern crate quote;
extern crate scoped_threadpool;
extern crate serde;
extern crate serde_json;
extern crate syn;
extern crate tokio;
extern crate tokio_util;

mod test_utils;

/// All crates used in UI tests are listed here
static TEST_DEPENDENCIES: &[&str] = &[
    "byte_slice_cast",
    "bytes",
    "futures",
    "if_chain",
    "itertools",
    "libc",
    "log",
    "memmap",
    "num_derive",
    "num_rational",
    "num_traits",
    "parking_lot",
    "pin_project",
    "quote",
    "regex",
    "serde",
    "serde_derive",
    "serde_json",
    "syn",
    "scoped_threadpool",
    "thiserror",
    "tracing",
    "tokio",
    "tokio_util",
    // "cassandra_cpp_sys", for cve_2024_27284
];

/// Produces a string with an `--extern` flag for all UI test crate
/// dependencies.
///
/// The dependency files are located by parsing the depinfo file for this test
/// module. This assumes the `-Z binary-dep-depinfo` flag is enabled. All test
/// dependencies must be added to Cargo.toml at the project root. Test
/// dependencies that are not *directly* used by this test module require an
/// `extern crate` declaration.
static EXTERN_FLAGS: LazyLock<Vec<String>> = LazyLock::new(|| {
    let current_exe_depinfo = {
        let mut path = env::current_exe().unwrap();
        path.set_extension("d");
        fs::read_to_string(path).unwrap()
    };
    let mut crates = BTreeMap::<&str, &str>::new();
    for line in current_exe_depinfo.lines() {
        // each dependency is expected to have a Makefile rule like `/path/to/crate-hash.rlib:`
        let parse_name_path = || {
            if line.starts_with(char::is_whitespace) {
                return None;
            }
            let path_str = line.strip_suffix(':')?;
            let path = Path::new(path_str);
            if !matches!(path.extension()?.to_str()?, "rlib" | "so" | "dylib" | "dll") {
                return None;
            }
            let (name, _hash) = path.file_stem()?.to_str()?.rsplit_once('-')?;
            // the "lib" prefix is not present for dll files
            let name = name.strip_prefix("lib").unwrap_or(name);
            Some((name, path_str))
        };
        if let Some((name, path)) = parse_name_path() {
            if TEST_DEPENDENCIES.contains(&name) {
                // A dependency may be listed twice if it is available in sysroot,
                // and the sysroot dependencies are listed first. As of the writing,
                // this only seems to apply to if_chain.
                crates.insert(name, path);
            }
        }
    }
    let not_found: Vec<&str> = TEST_DEPENDENCIES
        .iter()
        .copied()
        .filter(|n| !crates.contains_key(n))
        .collect();
    assert!(
        not_found.is_empty(),
        "dependencies not found in depinfo: {not_found:?}\n\
        help: Make sure the `-Z binary-dep-depinfo` rust flag is enabled\n\
        help: Try adding to dev-dependencies in Cargo.toml\n\
        help: Be sure to also add `extern crate ...;` to tests/compile-test.rs",
    );
    crates
        .into_iter()
        .map(|(name, path)| format!("--extern={name}={path}"))
        .collect()
});

fn base_config(test_dir: &str) -> (Config, Args) {
    let mut args = Args::test().unwrap();
    args.bless |= var_os("RUSTC_BLESS").is_some_and(|v| v != "0");

    let target_dir = PathBuf::from(var_os("CARGO_TARGET_DIR").unwrap_or_else(|| "target".into()));
    let mut config = Config {
        output_conflict_handling: OutputConflictHandling::Error,
        filter_files: env::var("TESTNAME")
            .map(|filters| filters.split(',').map(str::to_string).collect())
            .unwrap_or_default(),
        target: None,
        bless_command: Some("cargo uibless".into()),
        out_dir: target_dir.join("ui_test"),
        ..Config::rustc(Path::new("tests").join(test_dir))
    };
    config.comment_defaults.base().mode = Some(Spanned::dummy(Mode::Fail {
        require_patterns: false,
        rustfix: ui_test::RustfixMode::Everything,
    }))
    .into();
    config.comment_defaults.base().diagnostic_code_prefix = Some(Spanned::dummy("rpl::".into())).into();
    config.with_args(&args);
    let current_exe_path = env::current_exe().unwrap();
    let deps_path = current_exe_path.parent().unwrap();
    let profile_path = deps_path.parent().unwrap();

    config.program.args.extend(
        [
            "--emit=metadata",
            "-Aunused",
            "-Ainternal_features",
            "-Zui-testing",
            "-Zdeduplicate-diagnostics=no",
            "-Dwarnings",
            "-Adeprecated",
            &format!("-Ldependency={}", deps_path.display()),
        ]
        .map(OsString::from),
    );

    config.program.args.extend(EXTERN_FLAGS.iter().map(OsString::from));

    if let Some(host_libs) = option_env!("HOST_LIBS") {
        let dep = format!("-Ldependency={}", Path::new(host_libs).join("deps").display());
        config.program.args.push(dep.into());
    }

    config.program.program = profile_path.join(if cfg!(windows) { "rpl-driver.exe" } else { "rpl-driver" });
    (config, args)
}

fn run_ui() {
    let (config, args) = base_config("ui");

    ui_test::run_tests_generic(
        vec![config],
        ui_test::default_file_filter,
        ui_test::default_per_file_config,
        status_emitter::Text::from(args.format),
    )
    .unwrap();
}

fn main() {
    // Support being run by cargo nextest - https://nexte.st/book/custom-test-harnesses.html
    if env::args().any(|arg| arg == "--list") {
        if !env::args().any(|arg| arg == "--ignored") {
            println!("compile_test: test");
        }

        return;
    }

    // The SPEEDTEST_* env variables can be used to check RPL's performance on your PR. It runs the
    // affected test 1000 times and gets the average.
    if let Ok(speedtest) = std::env::var("SPEEDTEST") {
        println!("----------- STARTING SPEEDTEST -----------");
        let f = match speedtest.as_str() {
            "ui" => run_ui as fn(),
            _ => panic!("unknown speedtest: {speedtest} || accepted speedtests are: [ui]"),
        };

        let iterations;
        if let Ok(iterations_str) = std::env::var("SPEEDTEST_ITERATIONS") {
            iterations = iterations_str
                .parse::<u64>()
                .unwrap_or_else(|_| panic!("Couldn't parse `{iterations_str}`, please use a valid u64"));
        } else {
            iterations = 1000;
        }

        let mut sum = 0;
        for _ in 0..iterations {
            let start = std::time::Instant::now();
            f();
            sum += start.elapsed().as_millis();
        }
        println!(
            "average {} time: {} millis.",
            speedtest.to_uppercase(),
            sum / u128::from(iterations)
        );
    } else {
        run_ui();
    }
}
