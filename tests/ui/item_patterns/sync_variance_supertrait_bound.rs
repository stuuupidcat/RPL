//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/sync_variance.rpl
//@check-pass

trait SafeToShare: Sync {}

impl<T: Sync> SafeToShare for T {}

struct Wrapper<T> {
    value: T,
}

impl<T: SafeToShare> Wrapper<T> {
    fn get(&self) -> &T {
        &self.value
    }
}

unsafe impl<T> Sync for Wrapper<T> {}

fn main() {}
