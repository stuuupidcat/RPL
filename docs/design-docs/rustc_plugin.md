## 一、以`rustc`插件的形式开发`RPL`后端

- `rustc`插件泛指开启了`#![feature(rustc_private)]`特性的 crate，即把`rustc`当成一个库来使用

  ````rust
  // clippy中的一个例子
  
  use rustc_hir::{Body, ExprKind, Impl, ImplItemKind, Item, ItemKind, Node};
  
  declare_clippy_lint! {
      /// ### What it does
      /// Checks for empty `Drop` implementations.
      ///
      /// ### Why restrict this?
      /// Empty `Drop` implementations have no effect when dropping an instance of the type. They are
      /// most likely useless. However, an empty `Drop` implementation prevents a type from being
      /// destructured, which might be the intention behind adding the implementation as a marker.
      ///
      /// ### Example
      /// ```no_run
      /// struct S;
      ///
      /// impl Drop for S {
      ///     fn drop(&mut self) {}
      /// }
      /// ```
      /// Use instead:
      /// ```no_run
      /// struct S;
      /// ```
      #[clippy::version = "1.62.0"]
      pub EMPTY_DROP,
      restriction,
      "empty `Drop` implementations"
  }
  
  impl LateLintPass<'_> for EmptyDrop {
      fn check_item(&mut self, cx: &LateContext<'_>, item: &Item<'_>) {
          if let ItemKind::Impl(Impl {
              of_trait: Some(ref trait_ref),
              items: [child],
              ..
          }) = item.kind
              && trait_ref.trait_def_id() == cx.tcx.lang_items().drop_trait()
              && let impl_item_hir = child.id.hir_id()
              && let Node::ImplItem(impl_item) = cx.tcx.hir_node(impl_item_hir)
              && let ImplItemKind::Fn(_, b) = &impl_item.kind
              && let Body { value: func_expr, .. } = cx.tcx.hir().body(*b)
              && let func_expr = peel_blocks(func_expr)
              && let ExprKind::Block(block, _) = func_expr.kind
              && block.stmts.is_empty()
              && block.expr.is_none()
          {
              ....
          }
      }
  }
  
  ````
  
- 在`AST`层级上能做的事情的事情比较有限；最好可以使用`HIR`及其之后层级的信息；调用`rustc`不与轻量级的设计相冲突

    - 例一：
    
    ````rust
    pattern CVE-2018-21000
    
    meta {
        T1: ty
        T2: ty 
        // ty = HIR::Ty | MIR::Ty
        func: ident
        vec: ident
        ptr: ident
        cap: ident
        len: ident
        op: Operator
        
        T1: HIR::Ty
        T2: MIR::Ty::TyKind::Item
    }
    
    util {
        p1 = ```
            pub fn $func<$$(_:GArgs)> (mut $vec: Vec<$T1>) -> Vec<$T2> {
                #unordered! {
                    let $size = size_of::<$T2>();
                    let $cap = $vec.capacity() $op $size;
                    let $len = $vec.len() $op $size;
                    let $ptr = $vec.as_mut_ptr();
                }
                forget($vec);
                Vec::from_raw_parts($ptr as *mut $T2, $cap, $len)
            }
        ```
    }
    
    patt {
        p =
            | p1 {
                T2 = `u8`
                op = `*`
            }
            | p1 {
                T1 = `u8`
                op = `/`
            }
    }
    ````
    
    如果在THIR或MIR的层级上实现，上述模式中的18、19行可以匹配到如下两种代码实例：
    
    ```rust
    // 代码实例1
    let size = size_of::<$T2>();
    let cap = vec.capacity() * size;
    let len = vec.len() * size;
    
    // 代码实例2
    let cap = vec.capacity() / size_of::<$T2>();
    let len = vec.len() / size_of::<$T2>();
    ```
    
    - 例二：
    
    ```Rust
    patt {
        pat_transfrom_without_static = ```
            fn $func<$W>($(_:ident): Arc<$W>) -> $Waker
            where
                $W: #without!('static)
            {
                $Waker::#new!(...)
            }
    }
    ```
    
- 代码实现最大的工作量在解析“类`rust`"代码上

- 在不考虑性能的情况下，`SafeDrop`的工程实现不需要修改编译器

## 二、其他

- 使用`use`代码块来进行若干符号的说明：

```rust
pattern CVE-2018-21000

util {
    p1 = ```
        pub fn $func<$$(_:GArgs)> (mut $vec: Vec<$T1>) -> Vec<$T2> {
            #unordered! {
                let $size = size_of::<$T2>();
                let $cap = $vec.capacity() $op $size;
                let $len = $vec.len() $op $size;
                let $ptr = $vec.as_mut_ptr();
            }
            forget($vec);
            Vec::from_raw_parts($ptr as *mut $T2, $cap, $len)
        }
    ```
}

use {
    std::vec::Vec;
	std::mem::size_of;
	std::mem::forget;
}
```
