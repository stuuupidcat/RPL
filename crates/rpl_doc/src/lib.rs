//! rpldoc — generate Markdown documentation from .rpl pattern files.
//!
//! See `docs/superpowers/specs/2026-05-26-rpldoc-design.md` for the design.

#![feature(rustc_private)]

pub mod error;
pub use error::RpldocError;

pub mod model;
pub use model::{DocDiag, DocExample, DocFile, DocItem};

mod examples;
mod extract;
mod render;

use std::path::Path;

/// Parse an `.rpl` source string through the rpl_parser pipeline.
///
/// Returns `Ok(())` if parsing succeeds. The typed AST is dropped — callers
/// that want it should use `rpl_parser::parse_main` directly. This is the
/// minimal entry point for the corpus-sweep backward-compatibility gate.
pub fn parse_only(path: &Path, source: &str) -> Result<(), RpldocError> {
    rpl_parser::parse_main(source, path)
        .map(|_| ())
        .map_err(|e| RpldocError::Parse {
            path: path.to_path_buf(),
            message: format!("{e}"),
        })
}

#[cfg(test)]
mod model_tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn doc_file_constructs() {
        let f = DocFile {
            path: PathBuf::from("/x.rpl"),
            header_name: "X".into(),
            file_doc: vec!["hello".into()],
            patterns: vec![],
            utilities: vec![],
            diagnostics: vec![],
            examples: vec![],
        };
        assert_eq!(f.header_name, "X");
    }
}

/// Options controlling output behavior.
#[derive(Debug, Clone, Default)]
pub struct GenerateOpts {
    /// Optional output base directory. If `None`, write the .md next to each .rpl.
    /// If `Some`, write outputs under this directory, mirroring the input tree
    /// rooted at the input PATH.
    pub output_root: Option<std::path::PathBuf>,
    /// Suppress per-file "Generated foo.md" status lines.
    pub quiet: bool,
}

/// Build the documentation Markdown for a single `.rpl` file and return it.
///
/// This is the in-memory entry point — the CLI calls this and writes the
/// returned string to disk.
pub fn render_markdown(rpl_path: &std::path::Path) -> Result<String, RpldocError> {
    let source = std::fs::read_to_string(rpl_path).map_err(|e| RpldocError::Io {
        path: rpl_path.to_path_buf(),
        source: e,
    })?;
    let main = rpl_parser::parse_main(&source, rpl_path).map_err(|e| RpldocError::Parse {
        path: rpl_path.to_path_buf(),
        message: format!("{e}"),
    })?;
    let mut doc = extract::build_doc_file(rpl_path, &main);
    if doc.header_name.is_empty() {
        return Err(RpldocError::MissingPatternHeader {
            path: rpl_path.to_path_buf(),
        });
    }
    let mut warnings: Vec<RpldocError> = Vec::new();
    doc.examples = examples::load_examples(rpl_path, |w| warnings.push(w));
    for w in warnings {
        eprintln!("warning: {w}");
    }
    Ok(render::render(&doc))
}

/// Drive the CLI: takes a `PATH` (file or directory) and writes Markdown
/// outputs. Returns `Ok(())` if everything succeeded, or a `Vec` of all
/// per-file errors (and exits 1 in the caller).
pub fn run_cli(path: &std::path::Path, opts: GenerateOpts) -> Result<(), Vec<RpldocError>> {
    let mut errors = Vec::new();
    let files: Vec<std::path::PathBuf> = if path.is_dir() {
        walkdir::WalkDir::new(path)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("rpl"))
            .map(|e| e.into_path())
            .collect()
    } else {
        vec![path.to_path_buf()]
    };

    for rpl in &files {
        let md = match render_markdown(rpl) {
            Ok(md) => md,
            Err(e) => {
                errors.push(e);
                continue;
            },
        };
        let out_path = compute_output_path(path, rpl, &opts);
        if let Some(parent) = out_path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                errors.push(RpldocError::OutputWrite {
                    path: parent.to_path_buf(),
                    source: e,
                });
                continue;
            }
        }
        if let Err(e) = std::fs::write(&out_path, &md) {
            errors.push(RpldocError::OutputWrite {
                path: out_path.clone(),
                source: e,
            });
            continue;
        }
        if !opts.quiet {
            eprintln!("generated {}", out_path.display());
        }
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

fn compute_output_path(input_root: &std::path::Path, rpl: &std::path::Path, opts: &GenerateOpts) -> std::path::PathBuf {
    match &opts.output_root {
        None => rpl.with_extension("md"),
        Some(out_root) => {
            // Mirror the rpl's path relative to input_root under out_root.
            let rel = if input_root.is_file() {
                std::path::PathBuf::from(rpl.file_name().unwrap_or_default())
            } else {
                rpl.strip_prefix(input_root)
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|_| std::path::PathBuf::from(rpl.file_name().unwrap_or_default()))
            };
            out_root.join(rel).with_extension("md")
        },
    }
}

/// Crate version, exposed for sanity tests.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod parse_only_tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn parse_only_accepts_simple_pattern_file() {
        let src = "pattern Foo\n";
        // parse_only does not need the path to exist on disk; only used in errors.
        let result = parse_only(Path::new("/synthetic/foo.rpl"), src);
        assert!(result.is_ok(), "expected Ok, got {result:?}");
    }

    #[test]
    fn parse_only_reports_parse_error() {
        // `paten` is a misspelling of `pattern` — parse failure expected.
        let src = "paten Foo\n";
        let result = parse_only(Path::new("/synthetic/foo.rpl"), src);
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn missing_pattern_header_error_renders() {
        let err = RpldocError::MissingPatternHeader {
            path: PathBuf::from("/tmp/foo.rpl"),
        };
        let msg = format!("{err}");
        assert!(msg.contains("foo.rpl"));
        assert!(msg.contains("pattern"));
    }

    #[test]
    fn io_error_renders() {
        let err = RpldocError::Io {
            path: PathBuf::from("/tmp/foo.rpl"),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
        };
        let msg = format!("{err}");
        assert!(msg.contains("foo.rpl"));
        assert!(msg.contains("not found"));
    }
}

#[cfg(test)]
mod render_markdown_tests {
    use super::*;

    fn write(td: &tempfile::TempDir, name: &str, content: &str) -> std::path::PathBuf {
        let p = td.path().join(name);
        std::fs::write(&p, content).unwrap();
        p
    }

    #[test]
    fn renders_minimal_pattern() {
        let td = tempfile::TempDir::new().unwrap();
        let p = write(&td, "Foo.rpl", "pattern Foo\n");
        let md = render_markdown(&p).unwrap();
        assert_eq!(md, "# Foo\n\n");
    }

    #[test]
    fn parse_error_propagates_as_rpldoc_error() {
        let td = tempfile::TempDir::new().unwrap();
        let p = write(&td, "Bad.rpl", "not a pattern at all\n");
        let err = render_markdown(&p).unwrap_err();
        assert!(matches!(err, RpldocError::Parse { .. }));
    }
}
