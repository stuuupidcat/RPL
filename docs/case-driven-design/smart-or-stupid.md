# smart or stupid

## 任务

设计语法，完成如下任务

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
2. 像 refinement type 一样的约束在哪儿写？
3. pat 的类型是否有意义？
4. `where $T:Debug` 能否匹配 `where T1:Debug + Clone`？能否匹配`where T1:Debug, T2:Clone`？

> @shigma
> 1. debug 可以先直接匹配后缀，后续再研究如何 resolve to qualified ident
> 2. refinement 可以写在外面
> 3. pattern 应该是有类型的，比如一个 item 类型的 pattern 就不太能插在表达式里；但是这种类型上的意义其实可以不用着急做检测，因为错误的本来就匹配不到结果；结论是可以不做类型
> 4. where 的结构类似 struct 中的元组（即：未写出的可能存在，写出的不保证顺序），所以你问的两个问题的答案应该都是肯定的；如果要强制否定的话需要引入特别的语法，例如 where T!: Debug 表示 T 就只有 Debug，不能有别的，这种情况下第一个匹配不成立，第二个匹配依然成立（! 的写法可以讨论，only!()?)


### 方案一：

原则：提供内置函数描述有关Rust trait的模式，以提高其特殊性：

````Rust
t: ident
T: type
func: ident

p = ```rust
    pub fn $func<$$(_:GArgs)>($t:$T) {
        assert_impl!($T, Debug)
    	println!("{:?}", $t);    
	}
```
````

### 方案二：

一个更具通用性的方案：

````rust
t: ident
T: type
func: ident

p1 = ```rust
	pub fn $func<$T:Debug>($t:$T) {
    	println!("{:?}", $t);    
	}
```

p2 = ```rust
	pub fn $func<$T>($t:$T) 
	where
		$T:Debug
	{
		println!("{:?}", $t); 
	}
```

p = p1 | p2

````

