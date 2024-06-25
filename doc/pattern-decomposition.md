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

## 设计

问题分解：

- 函数
- 满足 trait bound 的范型参数

可能产生的问题：

1. `Debug` or `std::fmt::Debug`? 如何处理？
2. 像refinement type一样的约束在哪儿写？
3. pat的类型是否有意义？
4. `where $T:Debug` 能否匹配 `where T1:Debug + Clone`？能否匹配`where T1:Debug, T2:Clone`？

```rust
// func_trait_bound.pat

T: type
G: GArgs // type | lifetime
fn_name: ident

p1: pat(fn_name, T) =
    pub fn $fn_name <$T:Debug> (...) {...}

p2: pat(fn_name, T) =
    pub fn $fn_name <..., $T, ...> (...)
    where $T:Debug {
        ...
    }

p2_: pat(fn_name, T) =
    pub fn $fn_name <$G> (...) 
    where $T:only!(Debug)
    {
        ...
    }

p: pat(fn_name, T) = p1 | p2
```

```rust
use p in func_trait_bound.pat

T: {type | p(_, self)}
fn_name: {ident | p(self, _)}
t: ident

p3: pat(fn_name, T, t) = 
    pub fn $fn_name <..., $T:Debug, ...> (..., $t: $T, ...) {
        println!("{:?}", $t);
    }
```
