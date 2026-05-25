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

/// Uniformly remove the smallest leading-whitespace prefix common to all
/// non-blank lines. Blank lines are preserved as-is, and a trailing newline
/// in the input is preserved in the output.
///
/// Used to "outdent" a pattern body so it doesn't carry the surrounding
/// indentation into the rendered code block.
pub(crate) fn dedent(text: &str) -> String {
    let common = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);

    let mut out = String::with_capacity(text.len());
    for (i, line) in text.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        if line.trim().is_empty() {
            out.push_str(line);
        } else {
            out.push_str(&line[common..]);
        }
    }
    // Preserve a trailing newline if the input had one — `str::lines()`
    // does not yield a trailing empty element for `\n`-terminated input.
    if text.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod dedent_tests {
    use super::*;

    #[test]
    fn dedents_uniform_block() {
        let input = "    let a = 1;\n    let b = 2;";
        assert_eq!(dedent(input), "let a = 1;\nlet b = 2;");
    }

    #[test]
    fn preserves_relative_indentation() {
        let input = "    if x {\n        y\n    }";
        assert_eq!(dedent(input), "if x {\n    y\n}");
    }

    #[test]
    fn ignores_blank_lines_when_computing_prefix() {
        let input = "    a\n\n    b";
        assert_eq!(dedent(input), "a\n\nb");
    }

    #[test]
    fn returns_empty_for_empty_input() {
        assert_eq!(dedent(""), "");
    }

    #[test]
    fn preserves_trailing_newline() {
        let input = "    a\n    b\n";
        assert_eq!(dedent(input), "a\nb\n");
    }

    #[test]
    fn no_trailing_newline_in_no_trailing_newline_out() {
        let input = "    a\n    b";
        assert_eq!(dedent(input), "a\nb");
    }
}
