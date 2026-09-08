//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/sync_variance.rpl
//@check-pass

struct Wrapper<T> {
    value: T,
}

impl<T> Wrapper<T> {
    unsafe fn get_unchecked(&self) -> &T {
        &self.value
    }
}

unsafe impl<T> Sync for Wrapper<T> {}

fn main() {}
