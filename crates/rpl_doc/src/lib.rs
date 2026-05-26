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

use std::path::Path;

/// Parse an `.rpl` source string through the rpl_parser pipeline.
///
/// Returns `Ok(())` if parsing succeeds. The typed AST is dropped — callers
/// that want it should use `rpl_parser::parse_main` directly. This is the
/// minimal entry point for the corpus-sweep backward-compatibility gate.
pub fn parse_only(path: &Path, source: &str) -> Result<(), RpldocError> {
    rpl_parser::parse_main(source, path).map(|_| ()).map_err(|e| {
        RpldocError::Parse {
            path: path.to_path_buf(),
            message: format!("{e}"),
        }
    })
}

#[cfg(test)]
mod model_tests {
    use super::*;
    use std::path::PathBuf;

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

/// Crate version, exposed for sanity tests.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod parse_only_tests {
    use super::*;
    use std::path::Path;

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
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn crate_loads() {
        assert!(!VERSION.is_empty());
    }

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
