use rustc_lint_defs::declare_lint;

declare_lint! {
    /// The `lengthless_buffer_passed_to_extern_function` lint detects a buffer
    /// pointer passed to an extern function without specifying its length.
    ///
    /// ### Example
    ///
    /// ```rust,compile_fail
    /// #![deny(lengthless_buffer_passed_to_extern_function)]
    /// use libc::{c_char, c_int};
    /// extern unsafe fn gets(c: *const c_char) -> c_int {
    ///     0
    /// }
    ///
    /// fn main() {
    ///     let mut p = [8; u8];
    ///     unsafe {
    ///         gets(p as *const c_char)
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
