//! rpldoc — generate Markdown documentation from .rpl pattern files.
//!
//! See `docs/superpowers/specs/2026-05-26-rpldoc-design.md` for the design.

pub mod error;
pub use error::RpldocError;

/// Crate version, exposed for sanity tests.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

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
