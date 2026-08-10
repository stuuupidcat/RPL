//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/send_variance.rpl
//@check-pass

use std::marker::PhantomData;

struct Wrapper<T> {
    marker: PhantomData<T>,
}

unsafe impl<T> Send for Wrapper<T> {}

fn main() {}
