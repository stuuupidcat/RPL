//! Build `DocFile` from the typed pest AST.

use crate::model::DocFile;
use rpl_parser::pairs;
use std::path::Path;

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

/// Build a `DocFile` from already-parsed `pairs::main`.
///
/// Doesn't load examples (that's `examples::load_examples`'s job).
pub(crate) fn build_doc_file<'i>(
    path: &Path,
    main: &pairs::main<'i>,
) -> DocFile {
    let path = path.to_path_buf();
    let header_name = collect_header_name(main).to_string();
    let file_doc = collect_file_doc(main);

    DocFile {
        path,
        header_name,
        file_doc,
        patterns: Vec::new(),     // filled by later tasks
        utilities: Vec::new(),    // filled by later tasks
        diagnostics: Vec::new(),  // filled by later tasks
        examples: Vec::new(),     // filled by examples::load_examples
    }
}

fn collect_header_name<'i>(main: &pairs::main<'i>) -> &'i str {
    // Named accessors are immune to grammar shifts (e.g., a future
    // file-level attribute prepended to `RPLPattern`). The generator emits
    // one accessor per named child in the production:
    //   main         → r#RPLPattern()
    //   RPLPattern   → r#RPLHeader()
    //   RPLHeader    → r#Identifier()
    let rpl_pattern = main.RPLPattern();
    let rpl_header = rpl_pattern.RPLHeader();
    rpl_header.Identifier().span.as_str()
}

fn collect_file_doc<'i>(main: &pairs::main<'i>) -> Vec<String> {
    // `RPLPattern.InnerDocComment()` returns a `Vec<&InnerDocComment>` of
    // the zero-or-more inner doc lines that precede `RPLHeader`.
    let rpl_pattern = main.RPLPattern();
    let inner_docs = rpl_pattern.InnerDocComment();
    collect_runs(inner_docs.iter().map(|p| p.span.as_str()))
}

/// Join doc-comment lines (with their `///` / `//!` prefix already stripped)
/// into a single run separated by `\n`. Returns `Vec<String>` containing
/// either zero or one run.
///
/// Why one run only: the strict-mode grammar forbids two doc-comment runs at
/// the same attachment site (a blank line between them is a parse error), so
/// the pest pairs we iterate are always one contiguous block.
fn collect_runs<'a>(lines: impl Iterator<Item = &'a str>) -> Vec<String> {
    let stripped: Vec<&str> = lines.map(strip_doc_prefix).collect();
    if stripped.is_empty() {
        Vec::new()
    } else {
        vec![stripped.join("\n")]
    }
}

#[cfg(test)]
mod build_tests {
    use super::*;
    use rpl_parser::parse_main;

    fn parse(src: &str) -> pairs::main<'_> {
        parse_main(src, Path::new("/synthetic/test.rpl")).expect("parse")
    }

    #[test]
    fn header_name_extracted() {
        let src = "pattern Foo\n";
        let main = parse(src);
        let doc = build_doc_file(Path::new("/x/Foo.rpl"), &main);
        assert_eq!(doc.header_name, "Foo");
    }

    #[test]
    fn file_doc_empty_when_no_inner_doc() {
        let src = "pattern Foo\n";
        let main = parse(src);
        let doc = build_doc_file(Path::new("/x/Foo.rpl"), &main);
        assert!(doc.file_doc.is_empty());
    }

    #[test]
    fn file_doc_captured_when_inner_doc_present() {
        let src = "\
//! first line
//! second line
pattern Foo
";
        let main = parse(src);
        let doc = build_doc_file(Path::new("/x/Foo.rpl"), &main);
        assert_eq!(doc.file_doc, vec!["first line\nsecond line".to_string()]);
    }
}
