//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/sync_variance.rpl

use std::sync::Mutex;

struct Wrapper<T> {
    value: Mutex<T>,
}

impl<T> Wrapper<T> {
    fn replace(&self, value: T) -> T {
        std::mem::replace(&mut *self.value.lock().unwrap(), value)
    }
}

unsafe impl<T> Sync for Wrapper<T> {}
//~^ ERROR: A pattern instance found in this span

fn main() {}
