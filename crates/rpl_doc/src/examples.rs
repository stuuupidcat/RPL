//! Sibling examples folder discovery and rendering.
//!
//! For `<dir>/<stem>.rpl`, looks at `<dir>/<stem>/` for `.rs` files.

use crate::error::RpldocError;
use crate::model::DocExample;
use std::path::Path;

/// Load `.rs` example files from the sibling folder of `rpl_path`.
///
/// Returns an empty `Vec` if the folder doesn't exist. Per-file read errors
/// are reported to `warn` (caller's responsibility to surface), and the
/// offending file is skipped.
pub(crate) fn load_examples(
    rpl_path: &Path,
    mut warn: impl FnMut(RpldocError),
) -> Vec<DocExample> {
    let stem = match rpl_path.file_stem().and_then(|s| s.to_str()) {
        Some(s) => s,
        None => return Vec::new(),
    };
    let dir = match rpl_path.parent() {
        Some(p) => p.join(stem),
        None => return Vec::new(),
    };
    if !dir.is_dir() {
        return Vec::new();
    }

    let mut entries: Vec<_> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd.filter_map(Result::ok).collect(),
        Err(e) => {
            warn(RpldocError::Io { path: dir.clone(), source: e });
            return Vec::new();
        }
    };
    entries.sort_by_key(|e| e.file_name());

    let mut out = Vec::new();
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let filename = match path.file_name().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        let source = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                warn(RpldocError::Io { path: path.clone(), source: e });
                continue;
            }
        };
        out.push(promote_leading_inner_doc(filename, source));
    }
    out
}

/// Split off any leading `//!` block (the contiguous run of `//!` lines at
/// the very top of the file, before any other non-blank token).
fn promote_leading_inner_doc(filename: String, source: String) -> DocExample {
    let mut leading_doc = Vec::new();
    let mut consumed_bytes = 0usize;
    for line in source.split_inclusive('\n') {
        let trimmed = line.trim_end_matches('\n').trim_end_matches('\r');
        if trimmed.trim_start().starts_with("//!") {
            // Capture, stripping prefix and one optional space.
            let after = trimmed
                .trim_start()
                .strip_prefix("//!")
                .unwrap_or("");
            let stripped = after.strip_prefix(' ').unwrap_or(after);
            leading_doc.push(stripped.to_string());
            consumed_bytes += line.len();
        } else if trimmed.is_empty() && !leading_doc.is_empty() {
            // A blank line after a //! run terminates the leading block.
            consumed_bytes += line.len();
            break;
        } else {
            break;
        }
    }

    let rest = if consumed_bytes == 0 {
        source
    } else {
        source[consumed_bytes..].to_string()
    };

    let leading_runs = if leading_doc.is_empty() {
        Vec::new()
    } else {
        vec![leading_doc.join("\n")]
    };

    DocExample {
        filename,
        leading_doc: leading_runs,
        source: rest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_temp_with_layout(stem: &str, files: &[(&str, &str)]) -> tempfile::TempDir {
        let td = tempfile::TempDir::new().unwrap();
        let rpl = td.path().join(format!("{stem}.rpl"));
        fs::write(&rpl, "pattern X\n").unwrap();
        let ex_dir = td.path().join(stem);
        fs::create_dir(&ex_dir).unwrap();
        for (name, content) in files {
            fs::write(ex_dir.join(name), content).unwrap();
        }
        td
    }

    #[test]
    fn empty_when_no_folder() {
        let td = tempfile::TempDir::new().unwrap();
        let rpl = td.path().join("Foo.rpl");
        fs::write(&rpl, "pattern X").unwrap();
        let mut warnings = Vec::new();
        let examples = load_examples(&rpl, |w| warnings.push(w));
        assert!(examples.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn loads_rs_files_in_lex_order() {
        let td = make_temp_with_layout(
            "Foo",
            &[
                ("z.rs", "fn z() {}\n"),
                ("a.rs", "fn a() {}\n"),
                ("m.rs", "fn m() {}\n"),
            ],
        );
        let rpl = td.path().join("Foo.rpl");
        let examples = load_examples(&rpl, |_| {});
        assert_eq!(
            examples.iter().map(|e| &e.filename).collect::<Vec<_>>(),
            vec!["a.rs", "m.rs", "z.rs"],
        );
    }

    #[test]
    fn ignores_non_rs_files() {
        let td = make_temp_with_layout(
            "Foo",
            &[("a.rs", "fn a() {}"), ("README.md", "# readme"), ("data.txt", "x")],
        );
        let rpl = td.path().join("Foo.rpl");
        let examples = load_examples(&rpl, |_| {});
        assert_eq!(examples.len(), 1);
        assert_eq!(examples[0].filename, "a.rs");
    }

    #[test]
    fn promotes_leading_inner_doc_block() {
        let td = make_temp_with_layout(
            "Foo",
            &[(
                "a.rs",
                "//! Demonstrates the bug.\n//! Second line.\n\nfn a() {}\n",
            )],
        );
        let rpl = td.path().join("Foo.rpl");
        let examples = load_examples(&rpl, |_| {});
        assert_eq!(examples.len(), 1);
        assert_eq!(
            examples[0].leading_doc,
            vec!["Demonstrates the bug.\nSecond line."]
        );
        assert_eq!(examples[0].source, "fn a() {}\n");
    }

    #[test]
    fn no_promotion_when_no_leading_inner_doc() {
        let td = make_temp_with_layout("Foo", &[("a.rs", "fn a() {}\n")]);
        let rpl = td.path().join("Foo.rpl");
        let examples = load_examples(&rpl, |_| {});
        assert_eq!(examples.len(), 1);
        assert!(examples[0].leading_doc.is_empty());
        assert_eq!(examples[0].source, "fn a() {}\n");
    }
}
