//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/true_guard.rpl
//@check-pass

struct Wrapper<T> {
    value: T,
}

mod custom {
    pub unsafe trait Send {}
}

unsafe impl<T> custom::Send for Wrapper<T> {}

fn main() {}
