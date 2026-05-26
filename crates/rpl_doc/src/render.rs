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
    fn write_fence_uses_correct_length() {
        let mut s = String::new();
        write_fence(&mut s, "rpl", "let x = 1;\n");
        assert_eq!(s, "```rpl\nlet x = 1;\n```\n");

        let mut s = String::new();
        write_fence(&mut s, "rpl", "look: ```\n");
        assert_eq!(s, "````rpl\nlook: ```\n````\n");
    }
}
