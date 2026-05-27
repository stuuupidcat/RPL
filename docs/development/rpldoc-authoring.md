# Authoring rpldoc Documentation

This guide explains how to document `.rpl` pattern files so they render
nicely with `cargo rpl doc`. For the full design spec, see
[`docs/superpowers/specs/2026-05-26-rpldoc-design.md`](../superpowers/specs/2026-05-26-rpldoc-design.md).

## The two doc-comment forms

| Form | Where | What it documents |
|------|-------|-------------------|
| `//!` | At the very top of the `.rpl` file, before `pattern <Name>`. | The file as a whole. |
| `///` | Immediately before a `patt`/`util` item, or before a `diag` item. | That single item. |

A run of consecutive `///` (or `//!`) lines is treated as one paragraph.
You can use either an empty `///` line OR a true blank line (no `///`
prefix) to start a new paragraph. Both forms render as Markdown paragraph
breaks. Doc-comment content is rendered as raw Markdown; use inline code,
lists, and `####` or deeper headings freely.

Example — two paragraphs above the same item:

```rpl
/// First paragraph: what this pattern detects.
///
/// Second paragraph: caveats or false positives.
p_foo = fn _ (..) -> _ { … }
```

Equivalent (blank line instead of empty `///`):

```rpl
/// First paragraph: what this pattern detects.

/// Second paragraph: caveats or false positives.
p_foo = fn _ (..) -> _ { … }
```

## Strict placement

Doc comments are only legal at the three attachment points above. A
`///` placed anywhere else — inside a pattern body, between an
attribute and the item name, before a `use` path — is a parse error.
This is by design: the same way Rust rejects `#[doc = "..."]` in an
illegal position.

If you see an error like
```
expected one of OuterDocComment, Attr, Identifier
```
move the doc comment to immediately before the item you meant to
document.

## Worked example

See [`examples/Sample-Documented.rpl`](examples/Sample-Documented.rpl)
for a complete file. The companion folder
[`examples/Sample-Documented/`](examples/Sample-Documented/) holds the
`.rs` examples it points to, and
[`examples/Sample-Documented.md`](examples/Sample-Documented.md) is the
output rpldoc produces from this source — read it side-by-side with the
`.rpl` to see what each `//!` and `///` block becomes.

To regenerate it locally, run:

```sh
cargo rpl doc docs/development/examples/Sample-Documented.rpl
```

## Generated layout

For a file `<dir>/<stem>.rpl`, rpldoc emits `<dir>/<stem>.md` with
sections in this order, omitting any empty section:

1. `# <PatternHeaderName>` — the title.
2. File-level prose, rendered verbatim from the `//!` block.
3. `## Patterns` — one `### `<name>`` subsection per `patt` item.
4. `## Utilities` — one subsection per `util` item.
5. `## Diagnostics` — one anchored subsection per diagnostic group.
6. `## Examples` — one fenced code block per `.rs` file in the
   sibling `<stem>/` folder.

Inside each pattern subsection:
- The `///` prose, if any.
- A `**Diagnostic:**` link if the item carries `#[diag = "..."]`.
- A `**Signature:**` line showing the item head.
- A `<details>`-collapsed code block with the pattern body.

## The examples folder

For `<dir>/<stem>.rpl`, rpldoc looks for a sibling folder
`<dir>/<stem>/`. If it exists, every `.rs` file in it is embedded as
an `## Example: <filename.rs>` subsection in lexicographic order.

You can prepend a `//!` block to an example `.rs` file; rpldoc strips
it from the embedded source and renders it as prose above the code
block. This is the recommended way to caption examples.

Non-`.rs` files in the folder are ignored. Subdirectories are not
recursed.

## Running the tool

```sh
# Single file
cargo rpl doc path/to/Foo.rpl

# Recursive over a directory
cargo rpl doc docs/patterns-pest/cve/

# Mirror outputs into a separate tree
cargo rpl doc docs/patterns-pest/ --output /tmp/rpl-docs/

# Quiet mode (suppress per-file status lines)
cargo rpl doc docs/patterns-pest/ --quiet
```

## Best practices

- **Lead with one short summary sentence.** Authors and tools alike
  benefit from a first line that stands alone.
- **Separate "what it detects" from "how it detects."** Use a
  paragraph break (an empty `///` line) between intent and mechanism.
- **Link to the CVE/issue.** Use inline Markdown links — they render
  in any Markdown viewer.
- **Reference the diagnostic by name from the pattern's `///`.** The
  `[`name`](#diagnostic-name)` link works out of the box; don't
  hand-craft slugs.
- **Use code spans (`backticks`) liberally.** Type names, function
  paths, MIR fragments — anything that would be code in prose.
