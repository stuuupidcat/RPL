//! rpldoc — generate Markdown documentation from .rpl pattern files.
//!
//! See `docs/superpowers/specs/2026-05-26-rpldoc-design.md` for the design.

/// Crate version, exposed for sanity tests.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    #[test]
    fn crate_loads() {
        assert!(!super::VERSION.is_empty());
    }
}
