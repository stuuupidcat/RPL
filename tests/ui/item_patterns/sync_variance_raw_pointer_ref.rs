//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/sync_variance.rpl
//@check-pass

struct Wrapper<T> {
    pointer: *const T,
}

impl<T> Wrapper<T> {
    fn pointer(&self) -> &*const T {
        &self.pointer
    }
}

unsafe impl<T> Sync for Wrapper<T> {}

fn main() {}
