//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/sync_variance.rpl
//@check-pass

use std::marker::PhantomData;

struct Wrapper<T> {
    marker: PhantomData<T>,
}

impl<T> Wrapper<T> {
    fn marker(&self) -> &PhantomData<T> {
        &self.marker
    }
}

unsafe impl<T> Sync for Wrapper<T> {}

fn main() {}
