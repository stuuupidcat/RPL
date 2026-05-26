//! Render `DocFile` to Markdown.

use crate::model::{DocDiag, DocExample, DocFile, DocItem};

/// For a given block of text that will be wrapped in a backtick fence,
/// return the fence length to use. Always >= 3, and strictly longer than
/// any run of backticks inside `body`.
pub(crate) fn fence_len_for(body: &str) -> usize {
    let mut max_run = 0usize;
    let mut cur = 0usize;
    for c in body.chars() {
        if c == '`' {
            cur += 1;
            if cur > max_run {
                max_run = cur;
            }
        } else {
            cur = 0;
        }
    }
    std::cmp::max(3, max_run + 1)
}

/// Wrap `value` in a backtick code span, escalating the fence length if
/// `value` contains backticks. Returns a string like `` `value` `` or
/// `` `` `value with `inner` backticks` `` `` with proper escalation.
pub(crate) fn inline_code(value: &str) -> String {
    // Count the longest run of backticks inside the value.
    let mut max_run = 0usize;
    let mut cur = 0usize;
    for c in value.chars() {
        if c == '`' {
            cur += 1;
            if cur > max_run {
                max_run = cur;
            }
        } else {
            cur = 0;
        }
    }
    // Inline spans need at least 1 backtick (not the 3-tick floor for fences).
    let n = max_run + 1;
    let ticks: String = "`".repeat(n);
    // Add a space whenever the fence is longer than 1 tick (CommonMark rule:
    // avoids the content being mis-parsed as part of the delimiter), and also
    // when the value starts or ends with a backtick.
    let needs_pad = n > 1 || value.starts_with('`') || value.ends_with('`');
    if needs_pad {
        format!("{ticks} {value} {ticks}")
    } else {
        format!("{ticks}{value}{ticks}")
    }
}

/// Helper: write a fenced code block to `out`, escalating the fence length
/// as needed to safely embed `body`.
pub(crate) fn write_fence(out: &mut String, lang: &str, body: &str) {
    let n = fence_len_for(body);
    for _ in 0..n {
        out.push('`');
    }
    out.push_str(lang);
    out.push('\n');
    out.push_str(body);
    if !body.ends_with('\n') {
        out.push('\n');
    }
    for _ in 0..n {
        out.push('`');
    }
    out.push('\n');
}

/// Render a `DocFile` to a Markdown `String`.
pub(crate) fn render(doc: &DocFile) -> String {
    let mut out = String::new();

    // Title
    out.push_str(&format!("# {}\n\n", doc.header_name));

    // File-level prose
    for run in &doc.file_doc {
        out.push_str(run);
        out.push_str("\n\n");
    }

    // Patterns
    if !doc.patterns.is_empty() {
        out.push_str("## Patterns\n\n");
        for item in &doc.patterns {
            render_item(&mut out, item);
        }
    }

    // Utilities
    if !doc.utilities.is_empty() {
        out.push_str("## Utilities\n\n");
        for item in &doc.utilities {
            render_item(&mut out, item);
        }
    }

    // Diagnostics
    if !doc.diagnostics.is_empty() {
        out.push_str("## Diagnostics\n\n");
        for d in &doc.diagnostics {
            render_diag(&mut out, d);
        }
    }

    // Examples
    if !doc.examples.is_empty() {
        out.push_str("## Examples\n\n");
        for ex in &doc.examples {
            render_example(&mut out, ex);
        }
    }

    out
}

fn render_item(out: &mut String, item: &DocItem) {
    let mv = item.meta_vars.as_deref().unwrap_or("");
    out.push_str(&format!("### `{}{mv}`\n\n", item.name));
    for run in &item.doc {
        out.push_str(run);
        out.push_str("\n\n");
    }
    if let Some(diag) = &item.diag_attr {
        out.push_str(&format!(
            "**Diagnostic:** [`{diag}`](#diagnostic-{diag})\n\n",
        ));
    }
    out.push_str(&format!("**Signature:** `{}`\n\n", item.signature));
    out.push_str("<details><summary>Pattern body</summary>\n\n");
    write_fence(out, "rpl", &item.body_source);
    out.push_str("</details>\n\n");
}

fn render_diag(out: &mut String, d: &DocDiag) {
    out.push_str(&format!("<a id=\"diagnostic-{}\"></a>\n", d.name));
    out.push_str(&format!("### Diagnostic: `{}`\n\n", d.name));
    for run in &d.doc {
        out.push_str(run);
        out.push_str("\n\n");
    }
    if let Some(s) = &d.primary {
        out.push_str(&format!("- **Primary:** {}\n", inline_code(s)));
    }
    if let Some(s) = &d.label {
        out.push_str(&format!("- **Label:** {}\n", inline_code(s)));
    }
    if let Some(s) = &d.help {
        out.push_str(&format!("- **Help:** {}\n", inline_code(s)));
    }
    if let Some(s) = &d.note {
        out.push_str(&format!("- **Note:** {}\n", inline_code(s)));
    }
    if let Some(s) = &d.level {
        out.push_str(&format!("- **Level:** {}\n", inline_code(s)));
    }
    if let Some(s) = &d.lint_name {
        out.push_str(&format!("- **Lint name:** {}\n", inline_code(s)));
    }
    out.push_str("\n");
}

fn render_example(out: &mut String, ex: &DocExample) {
    out.push_str(&format!("### Example: `{}`\n\n", ex.filename));
    for run in &ex.leading_doc {
        out.push_str(run);
        out.push_str("\n\n");
    }
    write_fence(out, "rust", &ex.source);
    out.push_str("\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_fence_is_three() {
        assert_eq!(fence_len_for("plain text\nwithout backticks"), 3);
    }

    #[test]
    fn fence_escalates_past_inner_run() {
        assert_eq!(fence_len_for("contains ``` triple"), 4);
        assert_eq!(fence_len_for("contains ```` quad"), 5);
    }

    #[test]
    fn inline_code_escalates_for_inner_backticks() {
        assert_eq!(inline_code("plain"), "`plain`");
        assert_eq!(inline_code("has `inner` backticks"), "`` has `inner` backticks ``");
    }

    #[test]
    fn inline_code_pads_when_value_starts_or_ends_with_backtick() {
        assert_eq!(inline_code("`x"), "`` `x ``");
        assert_eq!(inline_code("x`"), "`` x` ``");
    }

    #[test]
    fn write_fence_uses_correct_length() {
        let mut s = String::new();
        write_fence(&mut s, "rpl", "let x = 1;\n");
        assert_eq!(s, "```rpl\nlet x = 1;\n```\n");

        let mut s = String::new();
        write_fence(&mut s, "rpl", "look: ```\n");
        assert_eq!(s, "````rpl\nlook: ```\n````\n");
    }
}

#[cfg(test)]
mod render_doc_file_tests {
    use super::*;
    use std::path::PathBuf;

    fn empty_doc() -> DocFile {
        DocFile {
            path: PathBuf::from("/x/Foo.rpl"),
            header_name: "Foo".into(),
            file_doc: vec![],
            patterns: vec![],
            utilities: vec![],
            diagnostics: vec![],
            examples: vec![],
        }
    }

    #[test]
    fn header_only_renders_just_title() {
        let out = render(&empty_doc());
        assert_eq!(out, "# Foo\n\n");
    }

    #[test]
    fn file_doc_appears_after_title() {
        let mut doc = empty_doc();
        doc.file_doc = vec!["intro line 1\nintro line 2".into()];
        let out = render(&doc);
        assert!(out.starts_with("# Foo\n\nintro line 1\nintro line 2\n\n"));
    }

    #[test]
    fn empty_sections_are_omitted() {
        let out = render(&empty_doc());
        assert!(!out.contains("## Patterns"));
        assert!(!out.contains("## Utilities"));
        assert!(!out.contains("## Diagnostics"));
        assert!(!out.contains("## Examples"));
    }

    #[test]
    fn pattern_section_emitted_when_nonempty() {
        let mut doc = empty_doc();
        doc.patterns.push(DocItem {
            name: "p_foo".into(),
            meta_vars: None,
            doc: vec!["docs".into()],
            diag_attr: Some("p_diag".into()),
            signature: "fn _ (..) -> _".into(),
            body_source: "let x = 1;".into(),
        });
        let out = render(&doc);
        assert!(out.contains("## Patterns"));
        assert!(out.contains("### `p_foo`"));
        assert!(out.contains("docs"));
        assert!(out.contains("[`p_diag`](#diagnostic-p_diag)"));
        assert!(out.contains("**Signature:** `fn _ (..) -> _`"));
        assert!(out.contains("```rpl\nlet x = 1;\n```"));
    }

    #[test]
    fn diag_section_has_anchor_and_fields() {
        let mut doc = empty_doc();
        doc.diagnostics.push(DocDiag {
            name: "p_diag".into(),
            doc: vec![],
            primary: Some("primary msg".into()),
            label: None,
            help: Some("help msg".into()),
            note: None,
            level: Some("deny".into()),
            lint_name: Some("foo_lint".into()),
        });
        let out = render(&doc);
        assert!(out.contains("<a id=\"diagnostic-p_diag\"></a>"));
        assert!(out.contains("### Diagnostic: `p_diag`"));
        assert!(out.contains("- **Primary:** `primary msg`"));
        assert!(!out.contains("- **Label:**"));   // omitted
        assert!(out.contains("- **Help:** `help msg`"));
        assert!(!out.contains("- **Note:**"));   // omitted
        assert!(out.contains("- **Level:** `deny`"));
        assert!(out.contains("- **Lint name:** `foo_lint`"));
    }

    #[test]
    fn example_with_leading_doc_renders_prose_before_fence() {
        let mut doc = empty_doc();
        doc.examples.push(DocExample {
            filename: "basic.rs".into(),
            leading_doc: vec!["intro".into()],
            source: "fn main() {}".into(),
        });
        let out = render(&doc);
        let i_intro = out.find("intro").unwrap();
        let i_fence = out.find("```rust").unwrap();
        assert!(i_intro < i_fence);
    }

    #[test]
    fn pattern_with_meta_vars_renders_brackets() {
        let mut doc = empty_doc();
        doc.patterns.push(DocItem {
            name: "p_foo".into(),
            meta_vars: Some("[$T: type]".into()),
            doc: vec![],
            diag_attr: None,
            signature: "fn _ (..) -> _".into(),
            body_source: "let x = 1;".into(),
        });
        let out = render(&doc);
        assert!(out.contains("### `p_foo[$T: type]`"));
    }
}
