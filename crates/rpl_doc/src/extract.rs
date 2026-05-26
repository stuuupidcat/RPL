//! Build `DocFile` from the typed pest AST.

use crate::model::{DocDiag, DocFile, DocItem};
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

    let mut patterns = Vec::new();
    let mut utilities = Vec::new();
    let mut diagnostics = Vec::new();

    // `RPLPattern.Block()` returns a `Vec<&Block>` of zero-or-more blocks.
    // Each `Block` is a `Choice3` of `pattBlock | utilBlock | diagBlock`; the
    // generated typed-pair API exposes one `Option<&_>` accessor per variant.
    let rpl_pattern = main.RPLPattern();
    for block in rpl_pattern.Block() {
        if let Some(patt) = block.pattBlock() {
            for item in patt.RPLPatternItem() {
                patterns.push(build_doc_item(item));
            }
        } else if let Some(util) = block.utilBlock() {
            for item in util.RPLPatternItem() {
                utilities.push(build_doc_item(item));
            }
        } else if let Some(d) = block.diagBlock() {
            for item in d.diagBlockItem() {
                diagnostics.push(build_doc_diag(item));
            }
        }
    }

    DocFile {
        path,
        header_name,
        file_doc,
        patterns,
        utilities,
        diagnostics,
        examples: Vec::new(), // filled by examples::load_examples
    }
}

/// Convert one `RPLPatternItem` into a `DocItem`.
///
/// Grammar:
///   RPLPatternItem = OuterDocComment* ~ Attr* ~ Identifier
///                  ~ MetaVariableDeclList? ~ Assign ~ RustItemsOrPatternOperation
fn build_doc_item<'i>(item: &pairs::RPLPatternItem<'i>) -> DocItem {
    let outer_docs = item.OuterDocComment();
    let doc = collect_runs(outer_docs.iter().map(|p| p.span.as_str()));

    let diag_attr = item
        .Attr()
        .into_iter()
        .find_map(extract_diag_attr_value);

    let name = item.Identifier().span.as_str().to_string();
    let meta_vars = item
        .MetaVariableDeclList()
        .map(|m| m.span.as_str().to_string());

    let (signature, body_raw) = split_signature_and_body(item.RustItemsOrPatternOperation());
    let body_source = dedent(&body_raw);

    DocItem {
        name,
        meta_vars,
        doc,
        diag_attr,
        signature,
        body_source,
    }
}

/// Convert one `diagBlockItem` into a `DocDiag`.
///
/// Grammar:
///   diagBlockItem = OuterDocComment* ~ Identifier ~ Assign ~ LeftBrace
///                 ~ diagItems ~ MetaVariableWithDiagMessageSeparatedByComma? ~ RightBrace
///   diagItems     = diagItem ~ (Comma ~ diagItem)* ~ Comma?
///   diagItem      = Identifier ~ (LeftParen ~ diagAttrs ~ RightParen)? ~ Assign ~ diagMessage
fn build_doc_diag<'i>(item: &pairs::diagBlockItem<'i>) -> DocDiag {
    let name = item.Identifier().span.as_str().to_string();
    let doc = collect_runs(item.OuterDocComment().iter().map(|p| p.span.as_str()));

    let mut primary = None;
    let mut label = None;
    let mut help = None;
    let mut note = None;
    let mut level = None;
    let mut lint_name = None;

    // `diagItems().diagItem()` returns a head-tail tuple
    // `(&diagItem, Vec<&diagItem>)` — same shape as `diagAttrs.diagAttr()`.
    let (first, rest) = item.diagItems().diagItem();
    for di in std::iter::once(first).chain(rest.into_iter()) {
        let key = di.Identifier().span.as_str();
        let msg_text = di.diagMessage().diagMessageInner().span.as_str().to_string();
        match key {
            "primary" => primary = Some(msg_text),
            "label" => label = Some(msg_text),
            "help" => help = Some(msg_text),
            "note" => note = Some(msg_text),
            "level" => level = Some(msg_text),
            "name" => lint_name = Some(msg_text),
            _ => { /* unknown key — ignore */ }
        }
    }

    DocDiag {
        name,
        doc,
        primary,
        label,
        help,
        note,
        level,
        lint_name,
    }
}

/// If the given `Attr` is `#[diag = "value"]`, return `Some(value)` (the inner
/// string without surrounding quotes). Otherwise return `None`.
///
/// Grammar:
///   Attr      = Hash ~ LeftBracket ~ diagAttrs ~ RightBracket
///   diagAttrs = diagAttr ~ (Comma ~ diagAttr)* ~ Comma?
///   diagAttr  = Word ~ ((LeftParen ~ diagAttrs? ~ RightParen) | (Assign ~ diagMessage))?
///   diagMessage = "\"" ~ diagMessageInner ~ "\""
///
/// The `Word` of the first `diagAttr` whose key is `diag` and which has an
/// `Assign ~ diagMessage` form yields the value. `diagMessageInner` has its
/// own span (Both-mode atomic), so its `span.as_str()` already excludes the
/// surrounding quotes.
fn extract_diag_attr_value<'i>(attr: &pairs::Attr<'i>) -> Option<String> {
    let diag_attrs = attr.diagAttrs();
    let (first, rest) = diag_attrs.diagAttr();
    std::iter::once(first)
        .chain(rest.into_iter())
        .find_map(|da| {
            if da.Word().span.as_str() == "diag" {
                da.diagMessage()
                    .map(|m| m.diagMessageInner().span.as_str().to_string())
            } else {
                None
            }
        })
}

/// Split a `RustItemsOrPatternOperation` into (signature, body), driven by
/// the typed AST rather than byte-level brace counting.
///
/// Layout per variant:
/// - `PatternOperation`: pure expression (e.g. `divergent[$T = $T]`). The
///   entire text is the signature; there is no body.
/// - `RustItemsWithConstraint` (`{ item+ }`): a wrapping brace block whose
///   contents are several items. The signature is empty; the body is the
///   text strictly between the outer `LeftBrace` and `RightBrace`.
/// - `RustItemWithConstraint` (`Attr* ~ RustItem ~ WhereBlock?`): unwraps to
///   one `RustItem` (`Fn`/`Struct`/`Enum`/`Impl`). The signature runs from
///   the start of the node up to the item's opening brace, and the body is
///   the text between that brace and its matching closer:
///   - `Fn` with `LeftBrace ~ MirBody ~ RightBrace` body — use the `Fn`'s
///     `FnBody` braces; for a `SemiColon`-only `FnBody`, the body is empty.
///   - `Struct` / `Enum` / `Impl` — use their own `LeftBrace`/`RightBrace`
///     accessors.
///
/// All brace positions come from named pest accessors, so a `}` (or `{`)
/// inside a string literal (`const "..."`) never confuses the split — the
/// previous byte-scan would have truncated the body at the first `}` inside
/// a string. Body trimming uses `trim_start_matches('\n').trim_end_matches('\n')`
/// to drop a leading and trailing newline without collapsing blank lines on
/// either side.
fn split_signature_and_body<'i>(
    node: &pairs::RustItemsOrPatternOperation<'i>,
) -> (String, String) {
    // `PatternOperation` is an expression with no brace body.
    if node.PatternOperation().is_some() {
        return (node.span.as_str().trim().to_string(), String::new());
    }

    // `RustItemsWithConstraint` is `LeftBrace ~ RustItemWithConstraint+ ~
    // RightBrace`. There is no signature; the body is the inside-of-braces.
    if let Some(items) = node.RustItemsWithConstraint() {
        let lb = items.LeftBrace().span;
        let rb = items.RightBrace().span;
        let input = lb.get_input();
        let body = &input[lb.end()..rb.start()];
        return (
            String::new(),
            trim_one_newline(body).to_string(),
        );
    }

    // Otherwise: `RustItemWithConstraint` = `Attr* ~ RustItem ~ WhereBlock?`.
    // Grammar guarantees one Choice3 arm matches; this `expect` only fires on
    // a parser/grammar mismatch.
    let item = node
        .RustItemWithConstraint()
        .expect("RustItemsOrPatternOperation must match one Choice3 variant");
    let node_span = node.span;
    let input = node_span.get_input();
    let rust_item = item.RustItem();

    // For each Rust item variant, locate the brace pair delimiting its body.
    // `Fn` has the only variant whose body is optional (`SemiColon` form).
    let braces = if let Some(f) = rust_item.Fn() {
        let fn_body = f.FnBody();
        match (fn_body.LeftBrace(), fn_body.RightBrace()) {
            (Some(lb), Some(rb)) => Some((lb.span, rb.span)),
            _ => None, // `SemiColon` form
        }
    } else if let Some(s) = rust_item.Struct() {
        Some((s.LeftBrace().span, s.RightBrace().span))
    } else if let Some(e) = rust_item.Enum() {
        Some((e.LeftBrace().span, e.RightBrace().span))
    } else if let Some(i) = rust_item.Impl() {
        Some((i.LeftBrace().span, i.RightBrace().span))
    } else {
        // Grammar guarantees one RustItem variant matches; fall back safely.
        None
    };

    match braces {
        Some((lb, rb)) => {
            let sig = &input[node_span.start()..lb.start()];
            let body = &input[lb.end()..rb.start()];
            (
                sig.trim().to_string(),
                trim_one_newline(body).to_string(),
            )
        }
        None => (node_span.as_str().trim().to_string(), String::new()),
    }
}

/// Strip a single leading `\n` from `s` and strip all trailing whitespace
/// (spaces, tabs, and newlines) from `s`.
///
/// The trailing-whitespace strip (rather than a single `\n`) is needed for
/// multi-line bodies: the text between the opening `{` and closing `}` of
/// `fn _ (..) -> _ {\n    body;\n}` ends with `\n    ` (the indentation
/// before `}`), not a bare `\n`.  Stripping only `\n` leaves the trailing
/// spaces intact, which then survive `dedent` as a whitespace-only line and
/// end up as trailing spaces inside the rendered code block.
///
/// `dedent` preserves blank lines verbatim, so removing all trailing
/// whitespace here does not affect intentional blank lines inside the body.
fn trim_one_newline(s: &str) -> &str {
    let s = s.strip_prefix('\n').unwrap_or(s);
    s.trim_end_matches(|c: char| c.is_ascii_whitespace())
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

    #[test]
    fn pattern_item_extracted() {
        let src = "\
pattern Foo
patt {
    /// docs for p_foo
    /// continuation
    p_foo = fn _ (..) -> _ { _ = const 0_usize; }
}
";
        let main = parse(src);
        let doc = build_doc_file(Path::new("/x/Foo.rpl"), &main);
        assert_eq!(doc.patterns.len(), 1);
        let item = &doc.patterns[0];
        assert_eq!(item.name, "p_foo");
        assert_eq!(item.doc, vec!["docs for p_foo\ncontinuation"]);
        assert!(item.body_source.contains("_ = const 0_usize;"));
        assert!(item.signature.contains("fn _"));
        assert!(item.meta_vars.is_none());
    }

    #[test]
    fn pattern_item_with_meta_vars() {
        let src = "\
pattern Foo
patt {
    p_foo[$T: type] = fn _ (..) -> _ { _ = const 0_usize; }
}
";
        let main = parse(src);
        let doc = build_doc_file(Path::new("/x/Foo.rpl"), &main);
        assert_eq!(doc.patterns[0].meta_vars.as_deref(), Some("[$T: type]"));
    }

    #[test]
    fn pattern_item_with_diag_attr() {
        let src = r#"
pattern Foo
patt {
    #[diag = "p_misordered"]
    p_foo = fn _ (..) -> _ { _ = const 0_usize; }
}
"#;
        let main = parse(src);
        let doc = build_doc_file(Path::new("/x/Foo.rpl"), &main);
        assert_eq!(doc.patterns[0].diag_attr.as_deref(), Some("p_misordered"));
    }

    #[test]
    fn util_block_items_extracted() {
        let src = "\
pattern Foo
util {
    u_foo = fn _ (..) -> _ { _ = const 0_usize; }
}
";
        let main = parse(src);
        let doc = build_doc_file(Path::new("/x/Foo.rpl"), &main);
        assert_eq!(doc.utilities.len(), 1);
        assert_eq!(doc.utilities[0].name, "u_foo");
        assert!(doc.patterns.is_empty());
    }

    #[test]
    fn pattern_body_with_brace_in_string_literal_extracts_correctly() {
        // Regression guard: `}` (or `{`) inside a `const "..."` string literal
        // must not fool the signature/body split. A byte-level brace counter
        // truncates the body at the inner `}`; the AST-driven split uses the
        // typed span of the function's body braces and so sees through it.
        let src = r#"
pattern Foo
patt {
    p_foo = fn _ (..) -> _ { _ = const "look: }"; }
}
"#;
        let main = parse(src);
        let doc = build_doc_file(Path::new("/x/Foo.rpl"), &main);
        assert_eq!(doc.patterns.len(), 1);
        let body = &doc.patterns[0].body_source;
        assert!(
            body.contains("look: }"),
            "body lost the string content; got: {body:?}"
        );
        // The trailing `;` must also survive — i.e. we did not stop at the
        // `}` inside the literal.
        assert!(
            body.contains(';'),
            "body truncated before the terminating `;`; got: {body:?}"
        );
    }

    #[test]
    fn diag_item_extracted() {
        let src = r#"
pattern Foo
diag {
    /// detection notes
    p_foo = {
        primary(span) = "primary text",
        label(span) = "label text",
        help(span) = "help text",
        level = "deny",
        name = "foo_lint",
    }
}
"#;
        let main = parse(src);
        let doc = build_doc_file(Path::new("/x/Foo.rpl"), &main);
        assert_eq!(doc.diagnostics.len(), 1);
        let d = &doc.diagnostics[0];
        assert_eq!(d.name, "p_foo");
        assert_eq!(d.doc, vec!["detection notes"]);
        assert_eq!(d.primary.as_deref(), Some("primary text"));
        assert_eq!(d.label.as_deref(), Some("label text"));
        assert_eq!(d.help.as_deref(), Some("help text"));
        assert_eq!(d.level.as_deref(), Some("deny"));
        assert_eq!(d.lint_name.as_deref(), Some("foo_lint"));
        assert!(d.note.is_none());
    }

    #[test]
    fn non_diag_attr_is_silently_skipped() {
        // The grammar accepts any `Word` as the attr key; we only consume
        // `diag`. A non-`diag` key (e.g. `lint_level`) must not error or be
        // misread as a diagnostic message.
        let src = r#"
pattern Foo
patt {
    #[lint_level = "deny"]
    p_foo = fn _ (..) -> _ { _ = const 0_usize; }
}
"#;
        let main = parse(src);
        let doc = build_doc_file(Path::new("/x/Foo.rpl"), &main);
        assert_eq!(doc.patterns.len(), 1);
        assert!(doc.patterns[0].diag_attr.is_none());
    }
}
