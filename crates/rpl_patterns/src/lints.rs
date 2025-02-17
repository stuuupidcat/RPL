use rustc_lint_defs::declare_lint;

declare_lint! {
    /// The `lengthless_buffer_passed_to_extern_function` lint detects a buffer
    /// pointer passed to an extern function without specifying its length.
    ///
    /// ### Example
    ///
    /// ```rust
    /// #![deny(lengthless_buffer_passed_to_extern_function)]
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
    pub LENGTHLESS_BUFFER_PASSED_TO_EXTERN_FUNCTION,
    Warn,
    "detects a lengthless buffer passed to extern function"
}

declare_lint! {
    /// The `rust_string_pointer_as_c_string_pointer` lint detects a Rust string pointer
    /// used as a C string pointer directly, for example, using `as` or `std::mem::transmute`
    ///
    /// ### Example
    ///
    /// ```rust
    /// #![deny(rust_string_pointer_as_c_string_pointer)]
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
    pub RUST_STRING_POINTER_AS_C_STRING_POINTER,
    Deny,
    "detects a Rust string pointer used as a C string pointer directly"
}

declare_lint! {
    /// The `unchecked_pointer_offset` lint detects a pointer that is offset using an unchecked integer.
    /// This is a common source of undefined behavior.
    ///
    /// ### Example
    ///
    /// ```rust
    /// #![deny(unchecked_pointer_offset)]
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
    pub UNCHECKED_POINTER_OFFSET,
    Deny,
    "detects a pointer that is offset using an unchecked integer"
}
