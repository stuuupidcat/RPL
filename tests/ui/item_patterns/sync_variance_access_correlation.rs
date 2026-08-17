//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/sync_variance.rpl

struct Wrapper<T> {
    value: T,
}

impl<T> Wrapper<T> {
    fn guarded(&self) -> &T
    where
        T: Sync,
    {
        &self.value
    }

    fn unguarded(&self) -> &T {
        &self.value
    }
}

unsafe impl<T> Sync for Wrapper<T> {}
//~^ ERROR: A pattern instance found in this span

fn main() {}
