//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/sync_variance.rpl

use std::sync::Mutex;

struct Wrapper<T> {
    shared: T,
    exclusive: Mutex<T>,
}

impl<T> Wrapper<T> {
    fn get(&self) -> &T {
        &self.shared
    }

    fn replace(&self, value: T) -> T {
        std::mem::replace(&mut *self.exclusive.lock().unwrap(), value)
    }
}

unsafe impl<T> Sync for Wrapper<T> {}
//~^ ERROR: A pattern instance found in this span
//~| ERROR: A pattern instance found in this span

fn main() {}
