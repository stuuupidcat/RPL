# Sample-Documented

Reference pattern demonstrating rpldoc conventions.

This file is documented end-to-end so authors can copy-paste the
structure. It does not detect any real bug — `p_sample` is a no-op
pattern that matches a trivial assignment.

See `docs/development/rpldoc-authoring.md` for the authoring guide.

## Patterns

### `p_sample`

Matches a function whose body contains a trivial usize assignment.

**What to document here:** explain what code shape this pattern detects,
under what conditions it fires, and any caveats (false positives, edge
cases) authors should know. The first line should be a short summary
that stands alone; subsequent paragraphs (separated by an empty `///`
line) can expand on it.

The `#[diag = "p_sample"]` attribute below cross-references the
diagnostic group of the same name — the generated docs render this as
a clickable link to the `## Diagnostics` section.

**Diagnostic:** [`p_sample`](#diagnostic-p_sample)

**Signature:** `fn _ (..) -> _`

<details><summary>Pattern body</summary>

```rpl
_ = const 0_usize;
```
</details>

## Diagnostics

<a id="diagnostic-p_sample"></a>
### Diagnostic: `p_sample`

Diagnostic emitted whenever `p_sample` matches.

The `///` block in front of a diagnostic group lets you document
the lint name and the rationale separately from the matching shape.

- **Primary:** `trivial usize assignment found`
- **Help:** `this is a demo lint; remove the assignment`
- **Level:** `warn`
- **Lint name:** `sample_lint`

## Examples

### Example: `triggering.rs`

Minimal example that would trigger `sample_lint`.

```rust
fn main() {
    let _: usize = 0;
}
```

