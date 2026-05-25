//! Plain-data types describing an rpldoc-parsed pattern file.
//!
//! These types are constructed by `extract.rs` from the typed pest AST and
//! consumed by `render.rs` to emit Markdown.

use std::path::PathBuf;

/// The top-level documentation extracted from a single `.rpl` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocFile {
    /// Source path. Used in errors and to derive output path / examples folder.
    pub path: PathBuf,
    /// Name from `pattern <Name>`.
    pub header_name: String,
    /// File-level `//!` content, split into runs. Each `Vec<String>` element
    /// is one run; lines within a run are joined with `\n` at render time.
    pub file_doc: Vec<String>,
    /// Items from any `patt` block(s).
    pub patterns: Vec<DocItem>,
    /// Items from any `util` block(s).
    pub utilities: Vec<DocItem>,
    /// Items from any `diag` block(s).
    pub diagnostics: Vec<DocDiag>,
    /// Sibling-folder `.rs` files, lex-ordered.
    pub examples: Vec<DocExample>,
}

/// A pattern or util item: `[/// docs]* [#[attr]]* name [meta_vars] = body`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocItem {
    /// Identifier — e.g. `p_u8_to_t_misordered_para_inline`.
    pub name: String,
    /// Bracketed meta-var list source text — e.g. `[$T: type]`. `None` if absent.
    pub meta_vars: Option<String>,
    /// Attached `///` content, prefix-stripped. Each element is one source
    /// line; the renderer joins them with `\n`. Empty `Vec` when no doc is
    /// attached. Stored as `Vec<String>` (not `Option<String>`) so future
    /// blank-line-broken multi-run support can land without an API change.
    pub doc: Vec<String>,
    /// Value of an `#[diag = "..."]` attribute, if present.
    pub diag_attr: Option<String>,
    /// Item signature — text from `=` up to the body's opening brace.
    /// Example: `unsafe? fn _ (..) -> _`.
    pub signature: String,
    /// Body text between matching `{` and `}`, uniformly dedented.
    pub body_source: String,
}

/// A diagnostic group: `[/// docs]* name = { fields... }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocDiag {
    /// Diagnostic identifier — the LHS of `<name> = { ... }` in the diag block.
    pub name: String,
    /// Attached `///` content, prefix-stripped. Each element is one source
    /// line; the renderer joins them with `\n`. Empty `Vec` when no doc is
    /// attached. Stored as `Vec<String>` (not `Option<String>`) so future
    /// blank-line-broken multi-run support can land without an API change.
    pub doc: Vec<String>,
    /// Primary message text from `primary(span) = "..."`.
    pub primary: Option<String>,
    /// Span label text from `label(span) = "..."`.
    pub label: Option<String>,
    /// Help text from `help(span) = "..."`.
    pub help: Option<String>,
    /// Supplemental note text from `note(span) = "..."`.
    pub note: Option<String>,
    /// Severity from `level = "..."` — typically `"deny"`, `"warn"`, or `"allow"`.
    pub level: Option<String>,
    /// User-visible lint name from `name = "..."` — appears in compiler output.
    pub lint_name: Option<String>,
}

/// An example .rs file from the sibling folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocExample {
    /// Bare filename (e.g. `"basic.rs"`) — no directory component.
    pub filename: String,
    /// `//!` block at the top of the file, prefix-stripped, if any.
    /// Each element is one source line; renderer joins with `\n`.
    pub leading_doc: Vec<String>,
    /// Source text with the leading `//!` block removed.
    pub source: String,
}
