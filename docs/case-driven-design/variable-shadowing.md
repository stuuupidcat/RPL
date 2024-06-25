# 变量遮蔽 (variable-shadowing)

## 任务 From @Shigma

解决在模式匹配中的变量遮蔽问题。

```Rust
fn main() {
    let shadowed_binding = 1;

    {
        println!("before being shadowed: {}", shadowed_binding);

        // This binding *shadows* the outer one
        let shadowed_binding = "abc";

        println!("shadowed in inner block: {}", shadowed_binding);
    }
    println!("outside inner block: {}", shadowed_binding);

    // This binding *shadows* the previous binding
    let shadowed_binding = 2;
    println!("shadowed in outer block: {}", shadowed_binding);
}
```

使设计的模式不要匹配到`(let shadowed_binding = "abc"; , println!("shadowed in outer block: {}", shadowed_binding);)`
