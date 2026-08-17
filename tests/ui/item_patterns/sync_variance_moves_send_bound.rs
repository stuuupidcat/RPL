//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/sync_variance.rpl
//@check-pass

use std::sync::Mutex;

struct Wrapper<T> {
    value: Mutex<T>,
}

impl<T> Wrapper<T> {
    fn replace(&self, value: T) -> T {
        std::mem::replace(&mut *self.value.lock().unwrap(), value)
    }
}

unsafe impl<T: Send> Sync for Wrapper<T> {}

fn main() {}
