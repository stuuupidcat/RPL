# 模式分解 (pattern-decomposition)

## 任务

设计语法，完成如下任务，并在其中使用模式分解。

匹配一个函数，其满足如下条件：
- 具有若干范型参数
- 其中一个范型参数实现了 `Debug` trait，不妨称之为 `$T`
- 函数具有一个类型为 `$T` 的参数，不妨称之为 `$t`
- 函数体中调用了 `println!` 宏，且其参数为 `$t`

## 案例

```rust
fn f<A, B:Debug>(a: A, b: B) {
    println!("{:?}", b);
}
```

## 方案 @stuuupidcat

