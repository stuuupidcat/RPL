//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/send_variance.rpl
//@check-pass

struct Wrapper<T> {
    value: T,
}

unsafe impl<T> Send for Wrapper<T> where T: Send {}

fn main() {}
