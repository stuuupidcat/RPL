//! Tests for the doc-comment productions added in the rpldoc work.
//!
//! Covers each of the three attachment points (file head, patt item,
//! diag item), plus negative cases for stray placements.

#![feature(rustc_private)]

use rpl_parser::parse_main;
use std::path::Path;

fn assert_parses(label: &str, src: &str) {
    let result = parse_main(src, Path::new("/synthetic/test.rpl"));
    assert!(result.is_ok(), "{label}: expected Ok, got: {:?}", result.err());
}

fn assert_parse_error(label: &str, src: &str) {
    let result = parse_main(src, Path::new("/synthetic/test.rpl"));
    assert!(result.is_err(), "{label}: expected parse error, got Ok");
}

#[test]
fn inner_doc_at_file_head_parses() {
    let src = "\
//! This pattern detects something.
//! Continuation line.
pattern Foo
";
    assert_parses("inner_doc_at_file_head", src);
}

#[test]
fn outer_doc_on_patt_item_parses() {
    let src = "\
pattern Foo
patt {
    /// docs for the item
    /// continuation
    p_foo = fn _ (..) -> _ { _ = const 0_usize; }
}
";
    assert_parses("outer_doc_on_patt_item", src);
}

#[test]
fn outer_doc_on_diag_item_parses() {
    let src = r#"
pattern Foo
diag {
    /// description of the diagnostic
    p_foo = {
        primary(span) = "primary message",
        level = "deny",
        name = "foo_lint",
    }
}
"#;
    assert_parses("outer_doc_on_diag_item", src);
}

#[test]
fn four_slash_is_not_doc_comment_parses() {
    let src = "\
//// ─── section divider ───
pattern Foo
";
    // Should parse; the //// is a regular comment, NOT an InnerDocComment.
    assert_parses("four_slash_is_not_doc_comment", src);
}

#[test]
fn block_comment_at_file_head_parses() {
    let src = "\
/* block comment */
pattern Foo
";
    assert_parses("block_comment_at_file_head", src);
}

#[test]
fn doc_comment_in_pattern_body_is_parse_error() {
    // `///` inside a function body has no attachment site in the grammar;
    // the body grammar does not include OuterDocComment. This fails because
    // of doc placement, not because of body syntax — verified with a valid
    // surrounding body using `_ = const 0_usize;`.
    let src = "\
pattern Foo
patt {
    p_foo = fn _ (..) -> _ {
        let $x: u8 = _;
        /// stray doc inside body
        let $y: u8 = copy $x;
    }
}
";
    assert_parse_error("doc_comment_in_pattern_body", src);
}

#[test]
fn doc_after_attribute_is_parse_error() {
    // Per grammar: OuterDocComment* comes BEFORE Attr*, so /// after #[..]
    // does not have an attachment site. The body uses the valid form
    // `_ = const 0_usize;` so the error is solely due to doc placement.
    let src = r#"
pattern Foo
patt {
    #[diag = "p_foo"]
    /// surprise — wrong order
    p_foo = fn _ (..) -> _ { _ = const 0_usize; }
}
"#;
    assert_parse_error("doc_after_attribute", src);
}

#[test]
fn inner_doc_inside_block_is_parse_error() {
    // //! only legal at file head, not inside a patt/diag block.
    // The body uses the valid form `_ = const 0_usize;` so the error is
    // solely due to the misplaced //! token.
    let src = "\
pattern Foo
patt {
    //! not a valid placement
    p_foo = fn _ (..) -> _ { _ = const 0_usize; }
}
";
    assert_parse_error("inner_doc_inside_block", src);
}

#[test]
fn doc_separated_by_blank_line_still_attaches() {
    // The pest grammar does NOT enforce rustdoc-style "blank line breaks
    // the run" semantics. `OuterDocComment*` greedily matches consecutive
    // /// lines because WHITESPACE (which skips blank lines) is consumed
    // between them.
    //
    // The extraction layer in rpl_doc treats all attached /// lines as
    // one doc block. Authors who want to convey "two separate ideas"
    // should put the second below the next item, not before the same one.
    let src = "\
pattern Foo
patt {
    /// first run

    /// second run
    p_foo = fn _ (..) -> _ { _ = const 0_usize; }
}
";
    assert_parses("doc_separated_by_blank_line_still_attaches", src);
}
