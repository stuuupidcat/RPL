//! Build `DocFile` from the typed pest AST.

/// Strip the `///` or `//!` prefix and an optional single trailing space.
///
/// `/// foo`  → `foo`
/// `///foo`   → `foo`
/// `///  foo` → ` foo`  (extra leading space preserved)
/// `//!`      → ``
pub(crate) fn strip_doc_prefix(line: &str) -> &str {
    let after_prefix = if let Some(rest) = line.strip_prefix("///") {
        rest
    } else if let Some(rest) = line.strip_prefix("//!") {
        rest
    } else {
        line
    };
    after_prefix.strip_prefix(' ').unwrap_or(after_prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_outer_with_space() {
        assert_eq!(strip_doc_prefix("/// foo"), "foo");
    }
    #[test]
    fn strips_outer_without_space() {
        assert_eq!(strip_doc_prefix("///foo"), "foo");
    }
    #[test]
    fn preserves_extra_indentation() {
        assert_eq!(strip_doc_prefix("///  foo"), " foo");
    }
    #[test]
    fn strips_inner_with_space() {
        assert_eq!(strip_doc_prefix("//! foo"), "foo");
    }
    #[test]
    fn handles_empty_doc_comment() {
        assert_eq!(strip_doc_prefix("///"), "");
        assert_eq!(strip_doc_prefix("//!"), "");
    }
}
