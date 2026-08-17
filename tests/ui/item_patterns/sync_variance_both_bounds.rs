//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/sync_variance.rpl
//@check-pass

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

unsafe impl<T: Send + Sync> Sync for Wrapper<T> {}

fn main() {}
