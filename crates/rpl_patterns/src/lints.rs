use rustc_lint_defs::declare_tool_lint;

declare_tool_lint! {
    /// The `rpl::lengthless_buffer_passed_to_extern_function` lint detects a buffer
    /// pointer passed to an extern function without specifying its length.
    ///
    /// ### Example
    ///
    /// ```rust
    /// #![deny(rpl::lengthless_buffer_passed_to_extern_function)]
    /// use libc::c_char;
    /// extern fn gets(c: *const c_char) -> i32 {
    ///     0
    /// }
    ///
    /// fn main() {
    ///     let mut p = [8u8; 64];
    ///     unsafe {
    ///         gets(&p as *const u8 as *const c_char);
    ///     }
    /// }
    /// ```
    ///
    /// {{produces}}
    ///
    /// ### Explanation
    ///
    /// When you pass a lengthless buffer to an extern function, the most probable
    /// situation is that you are using some old style C API which fills the buffer
    /// as much as it has, and it's never safe to use.
    ///
    /// However, in some cases, the size of the buffer may be fixed, and this lint
    /// can be suppressed then.
    pub rpl::LENGTHLESS_BUFFER_PASSED_TO_EXTERN_FUNCTION,
    Warn,
    "detects a lengthless buffer passed to extern function"
}

declare_tool_lint! {
    /// The `rpl::rust_string_pointer_as_c_string_pointer` lint detects a Rust string pointer
    /// used as a C string pointer directly, for example, using `as` or `std::mem::transmute`
    ///
    /// ### Example
    ///
    /// ```rust
    /// #![deny(rpl::rust_string_pointer_as_c_string_pointer)]
    /// use libc::c_char;
    /// extern fn gets(c: *const c_char) -> i32 {
    ///     0
    /// }
    ///
    /// fn main() {
    ///     let mut p = String::from("hello");
    ///     let p = p.as_bytes().as_ptr();
    ///     unsafe {
    ///         gets(p as *const c_char);
    ///     }
    /// }
    /// ```
    ///
    /// {{produces}}
    ///
    /// ### Explanation
    ///
    /// C strings normally end with a `\0`, while Rust strings do not. And
    /// Rust strings must contain valid UTF-8.
    pub rpl::RUST_STRING_POINTER_AS_C_STRING_POINTER,
    Deny,
    "detects a Rust string pointer used as a C string pointer directly"
}

declare_tool_lint! {
    /// The `rpl::unchecked_pointer_offset` lint detects a pointer that is offset using an unchecked integer.
    /// This is a common source of undefined behavior.
    ///
    /// ### Example
    ///
    /// ```rust
    /// #![deny(rpl::unchecked_pointer_offset)]
    ///
    /// fn index(p: *const u8, index: usize) -> *const u8 {
    ///     unsafe {
    ///         p.add(index)
    ///     }
    /// }
    /// ```
    ///
    /// {{produces}}
    ///
    /// ### Explanation
    ///
    /// The `add` method is used to offset a pointer by a given number of elements.
    /// However, if the index is not checked, it can lead to undefined behavior.
    /// To avoid this, you should always check the index before offsetting the pointer,
    /// unless both the pointer and the index are guaranteed to be valid, for example,
    /// when the index is calculated from the length of the slice, or both are constants.
    pub rpl::UNCHECKED_POINTER_OFFSET,
    Deny,
    "detects a pointer that is offset using an unchecked integer"
}

declare_tool_lint! {
    /// The `rpl::cassandra_iter_next_ptr_passed_to_cass_iter_get` lint detects a pointer returned by
    /// `cassandra_iterator_next` that is utilized in `cass_iterator_get_*`.
    ///
    /// ### Example
    ///
    /// ```rust
    /// /* extern crate cassandra_cpp_sys;
    /// use cassandra_cpp_sys::CassIterator as _CassIterator;
    /// use cassandra_cpp_sys::{
    ///     cass_false, cass_iterator_get_aggregate_meta, cass_iterator_next, cass_true,
    ///     CassAggregateMeta as _CassAggregateMeta,
    /// };
    /// use std::iter::Iterator;
    /// pub struct AggregateMeta(*const _CassAggregateMeta);
    /// impl AggregateMeta {
    ///     fn build(inner: *const _CassAggregateMeta) -> Self {
    ///         if inner.is_null() {
    ///             panic!("Unexpected null pointer")
    ///         };
    ///         AggregateMeta(inner)
    ///     }
    /// }
    /// #[derive(Debug)]
    /// pub struct AggregateIterator(*mut _CassIterator);
    /// impl Iterator for AggregateIterator {
    ///     type Item = AggregateMeta;
    ///     #![deny(cassandra_iter_next_ptr_passed_to_cass_iter_get)]
    ///     fn next(&mut self) -> Option<<Self as Iterator>::Item> {
    ///         unsafe {
    ///             match cass_iterator_next(self.0) {
    ///                 cass_false => None,
    ///                 cass_true => {
    ///                     let field_value = cass_iterator_get_aggregate_meta(self.0);
    ///                     Some(AggregateMeta::build(field_value))
    ///                 }
    ///             }
    ///         }
    ///     }
    /// } */
    /// ```
    ///
    /// {{produces}}
    ///
    /// ### Explanation
    ///
    /// Code that attempts to **use an item returned by an iterator
    /// after the iterator has advanced to the next item** will be accessing freed memory,
    /// which caused by the underlying Cassandra driver which invalidates the current item when `next()` is called,
    /// leading to a **use-after-free** vulnerability.
    pub rpl::CASSANDRA_ITER_NEXT_PTR_PASSED_TO_CASS_ITER_GET,
    Deny,
    "detects a pointer returned by `cassandra_iterator_next` that is utilized in `cass_iterator_get_*`"
}

declare_tool_lint! {
    /// The `rpl::set_len_to_extend` lint detects using `Vec::set_len` to extend the length of a `Vec`
    /// without initializing the new elements.
    ///
    /// ### Example
    ///
    /// ```rust
    /// #![deny(rpl::set_len_to_extend)]
    /// let v = vec![1, 2, 3];
    /// unsafe {
    ///    v.set_len(5);
    /// }
    /// v[4]; // undefined behavior
    /// ```
    ///
    /// {{produces}}
    ///
    /// ### Explanation
    ///
    /// The `set_len` method is used to change the length of a `Vec` without changing its capacity.
    /// However, it does not initialize the new elements, which can lead to undefined behavior,
    /// even if the `Vec` is not used after the `set_len` call,
    /// as the `Vec` may be dropped, and the destructor may access the uninitialized memory.
    /// To avoid this, you should always use the `resize` method to extend the length of a `Vec`.
    pub rpl::SET_LEN_TO_EXTEND,
    Deny,
    "detects using `Vec::set_len` to extend the length of a `Vec` without initializing the new elements"
}

declare_tool_lint! {
    /// The `rpl::set_len_to_truncate` lint detects using `Vec::set_len` to truncate the length of a `Vec`.
    ///
    /// ### Example
    ///
    /// ```rust
    /// #![deny(rpl::set_len_to_truncate)]
    /// let mut v = vec![Box::new(1), Box::new(2), Box::new(3)];
    /// unsafe {
    ///   v.set_len(2); // memory leak
    /// }
    /// ```
    ///
    /// {{produces}}
    ///
    /// ### Explanation
    ///
    /// The `set_len` method is used to change the length of a `Vec` without changing its capacity.
    /// However, it does not drop the elements that are removed, which can lead to memory leaks.
    /// To avoid this, you should always use the `truncate` method to truncate the length of a `Vec`.
    pub rpl::SET_LEN_TO_TRUNCATE,
    Warn,
    "detects using `Vec::set_len` to truncate the length of a `Vec` without dropping the elements"
}

declare_tool_lint! {
    /// The `rpl::set_len_to_uninitialized` lint detects using `Vec::set_len` to truncate the length of a `Vec`.
    ///
    /// ### Example
    ///
    /// ```rust
    /// #![deny(rpl::set_len_to_uninitialized)]
    /// let mut v = Vec::with_capacity(3);
    /// unsafe {
    ///   v.set_len(3);
    /// }
    /// ```
    ///
    /// {{produces}}
    ///
    /// ### Explanation
    ///
    /// The `set_len` method is used to change the length of a `Vec` without changing its capacity.
    /// However, it does not initialize the new elements, which can lead to undefined behavior.
    /// To avoid this, you should always use the `resize` method to initialize the new elements.
    pub rpl::SET_LEN_UNINITIALIZED,
    Warn,
    "detects calling `Vec::set_len` without initializing the new elements in advance"
}
