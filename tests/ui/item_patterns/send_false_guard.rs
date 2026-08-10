//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/false_guard.rpl
//@check-pass

struct Wrapper<T> {
    value: T,
}

unsafe impl<T> Send for Wrapper<T> {}

fn main() {}
