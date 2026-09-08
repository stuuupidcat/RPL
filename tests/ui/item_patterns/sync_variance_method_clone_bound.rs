//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/sync_variance.rpl

struct Wrapper<T> {
    value: T,
}

impl<T> Wrapper<T> {
    fn get(&self) -> &T
    where
        T: Clone,
    {
        &self.value
    }
}

unsafe impl<T> Sync for Wrapper<T> {}
//~^ ERROR: A pattern instance found in this span

fn main() {}
