# WithExamples

## Patterns

### `p_foo`

**Signature:** `fn _ (..) -> _`

<details><summary>Pattern body</summary>

```rpl
_ = const 0_usize;
```
</details>

## Examples

### Example: `positive.rs`

This file demonstrates the triggering case.

```rust
fn main() {
    let _ = 0usize;
}
```

