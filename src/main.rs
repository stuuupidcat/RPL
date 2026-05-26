// warn on lints, that are included in `rust-lang/rust`s bootstrap
#![warn(rust_2018_idioms, unused_lifetimes)]
#![feature(rustc_private)]

use std::env;
use std::path::PathBuf;
use std::process::{self, Command};

use anstream::println;

#[allow(clippy::ignored_unit_patterns)]
fn show_help() {
    println!("{}", help_message());
}

#[allow(clippy::ignored_unit_patterns)]
fn show_version() {
    let version_info = rustc_tools_util::get_version_info!();
    println!("{version_info}");
}

pub fn main() {
    // Check for version and help flags even when invoked as 'cargo-rpl'
    if env::args().any(|a| a == "--help" || a == "-h") {
        show_help();
        return;
    }

    if env::args().any(|a| a == "--version" || a == "-V") {
        show_version();
        return;
    }

    if let Some(pos) = env::args().position(|a| a == "--explain") {
        if let Some(mut lint) = env::args().nth(pos + 1) {
            lint.make_ascii_lowercase();
        } else {
            show_help();
        }
        return;
    }

    // rpldoc subcommand: `cargo rpl doc <PATH> [--output <DIR>] [--quiet]`
    if env::args().nth(2).as_deref() == Some("doc") {
        let doc_args: Vec<String> = env::args().skip(3).collect();
        match run_doc(doc_args) {
            Ok(()) => return,
            Err(code) => process::exit(code),
        }
    }

    if let Err(code) = process(env::args().skip(2)) {
        process::exit(code);
    }
}

#[derive(Debug)]
struct RplCmd {
    cargo_subcommand: &'static str,
    args: Vec<String>,
    rpl_args: Vec<String>,
    pattern_groups: Vec<String>,
    manifest_path: Option<PathBuf>,
}

impl RplCmd {
    fn new<I>(mut old_args: I) -> Result<Self, String>
    where
        I: Iterator<Item = String>,
    {
        let mut cargo_subcommand = "check";
        let mut args = Vec::new();
        let mut rpl_args = Vec::new();
        let mut pattern_groups = Vec::new();
        let mut manifest_path = None;
        let mut after_dashdash = false;

        let iter = old_args.by_ref();
        while let Some(arg) = iter.next() {
            if arg == "--" {
                after_dashdash = true;
                continue;
            }

            if after_dashdash {
                rpl_args.push(arg);
                continue;
            }

            if let Some(value) = arg.strip_prefix("--patterns=") {
                if value.is_empty() {
                    return Err("`--patterns` requires a non-empty value".to_string());
                }
                pattern_groups.push(value.to_string());
                continue;
            }
            if arg == "--patterns" {
                match iter.next() {
                    Some(value) if !value.is_empty() => pattern_groups.push(value),
                    _ => return Err("`--patterns` requires a value".to_string()),
                }
                continue;
            }

            match arg.as_str() {
                "--fix" => {
                    cargo_subcommand = "fix";
                    continue;
                },
                "--no-deps" => {
                    rpl_args.push("--no-deps".into());
                    continue;
                },
                _ => {},
            }

            if let Some(value) = arg.strip_prefix("--manifest-path=") {
                manifest_path = Some(PathBuf::from(value));
                args.push(arg);
                continue;
            }

            if arg == "--manifest-path" {
                if let Some(value) = iter.next() {
                    manifest_path = Some(PathBuf::from(&value));
                    args.push(arg);
                    args.push(value);
                } else {
                    args.push(arg);
                }
                continue;
            }

            args.push(arg);
        }
        if cargo_subcommand == "fix" && !rpl_args.iter().any(|arg| arg == "--no-deps") {
            rpl_args.push("--no-deps".into());
        }

        Ok(Self {
            cargo_subcommand,
            args,
            rpl_args,
            pattern_groups,
            manifest_path,
        })
    }

    fn path() -> PathBuf {
        let mut path = env::current_exe()
            .expect("current executable path invalid")
            .with_file_name("rpl-driver");

        if cfg!(windows) {
            path.set_extension("exe");
        }

        path
    }

    fn into_std_cmd(self) -> Command {
        let mut cmd = Command::new(env::var("CARGO").unwrap_or("cargo".into()));
        let rpl_args: String = self
            .rpl_args
            .iter()
            .fold(String::new(), |s, arg| s + arg + "__RPL_HACKERY__");

        cmd.env("RUSTC_WORKSPACE_WRAPPER", Self::path())
            .env("RPL_ARGS", rpl_args)
            .arg(self.cargo_subcommand)
            .args(&self.args);

        cmd
    }
}

fn process<I>(old_args: I) -> Result<(), i32>
where
    I: Iterator<Item = String>,
{
    let cmd = match RplCmd::new(old_args) {
        Ok(cmd) => cmd,
        Err(err) => {
            eprintln!("{err}");
            return Err(1);
        },
    };
    let config = match rpl_config::load_config(cmd.manifest_path.as_deref(), &cmd.pattern_groups) {
        Ok(value) => value,
        Err(err) => {
            eprintln!("{err}");
            return Err(1);
        },
    };

    let mut cmd = cmd.into_std_cmd();
    if let Some(patterns_env) = config.patterns_env {
        cmd.env("RPL_PATS", patterns_env);
    }
    if let Some(inline_mir) = config.inline_mir {
        apply_inline_mir(&mut cmd, inline_mir);
    }

    let exit_status = cmd
        .spawn()
        .expect("could not run cargo")
        .wait()
        .expect("failed to wait for cargo?");

    if exit_status.success() {
        Ok(())
    } else {
        Err(exit_status.code().unwrap_or(-1))
    }
}

fn apply_inline_mir(cmd: &mut Command, inline_mir: bool) {
    let flag = format!("-Zinline-mir={inline_mir}");
    match env::var("RUSTFLAGS") {
        Ok(existing) if existing.contains("inline-mir") => {},
        Ok(existing) if existing.trim().is_empty() => {
            cmd.env("RUSTFLAGS", flag);
        },
        Ok(existing) => {
            cmd.env("RUSTFLAGS", format!("{existing} {flag}"));
        },
        Err(_) => {
            cmd.env("RUSTFLAGS", flag);
        },
    }
}

fn run_doc(args: Vec<String>) -> Result<(), i32> {
    let mut path: Option<std::path::PathBuf> = None;
    let mut output_root: Option<std::path::PathBuf> = None;
    let mut quiet = false;

    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--quiet" => quiet = true,
            "--output" => {
                let v = it.next().ok_or_else(|| {
                    eprintln!("error: --output requires a value");
                    2
                })?;
                output_root = Some(std::path::PathBuf::from(v));
            }
            s if s.starts_with("--output=") => {
                output_root = Some(std::path::PathBuf::from(&s["--output=".len()..]));
            }
            _ => {
                if path.is_some() {
                    eprintln!("error: only one PATH argument supported");
                    return Err(2);
                }
                path = Some(std::path::PathBuf::from(arg));
            }
        }
    }

    let path = path.ok_or_else(|| {
        eprintln!("usage: cargo rpl doc <PATH> [--output <DIR>] [--quiet]");
        2
    })?;

    let opts = rpl_doc::GenerateOpts { output_root, quiet };
    rpl_doc::run_cli(&path, opts).map_err(|errors| {
        for e in errors {
            eprintln!("error: {e}");
        }
        1
    })
}

#[must_use]
pub fn help_message() -> &'static str {
    color_print::cstr!(
"Checks a package to catch common mistakes and improve your Rust code.

<green,bold>Usage</>:
    <cyan,bold>cargo rpl</> <cyan>[OPTIONS] [--] [<<ARGS>>...]</>

<green,bold>Common options:</>
    <cyan,bold>--no-deps</>                Run RPL only on the given crate, without linting the dependencies
    <cyan,bold>--fix</>                    Automatically apply lint suggestions. This flag implies <cyan>--no-deps</> and <cyan>--all-targets</>
    <cyan,bold>--patterns</> <cyan><<GROUP>></>     Run RPL with selected pattern groups (repeatable)
    <cyan,bold>-h</>, <cyan,bold>--help</>               Print this message
    <cyan,bold>-V</>, <cyan,bold>--version</>            Print version info and exit
    <cyan,bold>--explain [LINT]</>         Print the documentation for a given lint

See all options with <cyan,bold>cargo check --help</>.

<green,bold>Allowing / Denying lints</>

To allow or deny a lint from the command line you can use <cyan,bold>cargo rpl --</> with:

    <cyan,bold>-W</> / <cyan,bold>--warn</> <cyan>[LINT]</>       Set lint warnings
    <cyan,bold>-A</> / <cyan,bold>--allow</> <cyan>[LINT]</>      Set lint allowed
    <cyan,bold>-D</> / <cyan,bold>--deny</> <cyan>[LINT]</>       Set lint denied
    <cyan,bold>-F</> / <cyan,bold>--forbid</> <cyan>[LINT]</>     Set lint forbidden

<green,bold>Manifest Options:</>
    <cyan,bold>--manifest-path</> <cyan><<PATH>></>  Path to Cargo.toml
    <cyan,bold>--frozen</>                Require Cargo.lock and cache are up to date
    <cyan,bold>--locked</>                Require Cargo.lock is up to date
    <cyan,bold>--offline</>               Run without accessing the network
")
}
#[cfg(test)]
mod tests {
    use super::RplCmd;

    #[test]
    fn fix() {
        let args = "cargo rpl --fix".split_whitespace().map(ToString::to_string);
        let cmd = RplCmd::new(args).expect("parse args");
        assert_eq!("fix", cmd.cargo_subcommand);
        assert!(!cmd.args.iter().any(|arg| arg.ends_with("unstable-options")));
    }

    #[test]
    fn fix_implies_no_deps() {
        let args = "cargo rpl --fix".split_whitespace().map(ToString::to_string);
        let cmd = RplCmd::new(args).expect("parse args");
        assert!(cmd.rpl_args.iter().any(|arg| arg == "--no-deps"));
    }

    #[test]
    fn no_deps_not_duplicated_with_fix() {
        let args = "cargo rpl --fix -- --no-deps"
            .split_whitespace()
            .map(ToString::to_string);
        let cmd = RplCmd::new(args).expect("parse args");
        assert_eq!(cmd.rpl_args.iter().filter(|arg| *arg == "--no-deps").count(), 1);
    }

    #[test]
    fn check() {
        let args = "cargo rpl".split_whitespace().map(ToString::to_string);
        let cmd = RplCmd::new(args).expect("parse args");
        assert_eq!("check", cmd.cargo_subcommand);
    }

    #[test]
    fn patterns_equals() {
        let args = "cargo rpl --patterns=core".split_whitespace().map(ToString::to_string);
        let cmd = RplCmd::new(args).expect("parse args");
        assert_eq!(cmd.pattern_groups, vec!["core".to_string()]);
    }

    #[test]
    fn patterns_space() {
        let args = "cargo rpl --patterns core".split_whitespace().map(ToString::to_string);
        let cmd = RplCmd::new(args).expect("parse args");
        assert_eq!(cmd.pattern_groups, vec!["core".to_string()]);
    }

    #[test]
    fn patterns_multiple() {
        let args = "cargo rpl --patterns=core --patterns extra"
            .split_whitespace()
            .map(ToString::to_string);
        let cmd = RplCmd::new(args).expect("parse args");
        assert_eq!(cmd.pattern_groups, vec!["core".to_string(), "extra".to_string()]);
    }

    #[test]
    fn patterns_missing_value() {
        let args = "cargo rpl --patterns".split_whitespace().map(ToString::to_string);
        let err = RplCmd::new(args).expect_err("missing value should error");
        assert!(err.contains("--patterns"));
    }

    #[test]
    fn patterns_empty_value() {
        let args = "cargo rpl --patterns=".split_whitespace().map(ToString::to_string);
        let err = RplCmd::new(args).expect_err("empty value should error");
        assert!(err.contains("--patterns"));
    }

    #[test]
    fn patterns_after_dashdash() {
        let args = "cargo rpl -- --patterns=core"
            .split_whitespace()
            .map(ToString::to_string);
        let cmd = RplCmd::new(args).expect("parse args");
        assert!(cmd.pattern_groups.is_empty());
        assert!(cmd.rpl_args.iter().any(|arg| arg == "--patterns=core"));
    }
}
