# 元变量 (meta-variable)

## 任务

设计一套描述 Rust Pattern 的元变量。

## 参考：[Rust 宏中的 fragment-specifier](https://veykril.github.io/tlborm/decl-macros/macros-methodical.html)

Captures are written as a dollar ($) followed by an identifier, a colon (:), and finally the kind of capture which is also called the fragment-specifier, which must be one of the following:

block: a block (i.e. a block of statements and/or an expression, surrounded by braces)
expr: an expression
ident: an identifier (this includes keywords)
item: an item, like a function, struct, module, impl, etc.
lifetime: a lifetime (e.g. 'foo, 'static, ...)
literal: a literal (e.g. "Hello World!", 3.14, '🦀', ...)
meta: a meta item; the things that go inside the #[...] and #![...] attributes
pat: a pattern
path: a path (e.g. foo, ::std::mem::replace, transmute::<\_, int>, …)
stmt: a statement
tt: a single token tree
ty: a type
vis: a possible empty visibility qualifier (e.g. pub, pub(in crate), ...)

## 设计

- 参考：[syn](https://github.com/dtolnay/syn/tree/master/src)?
- 目前可以先使用 Rust 宏的 fragment-specifier 作为参考。
