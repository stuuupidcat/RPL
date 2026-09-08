//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/sync_variance.rpl
//@check-pass

// Adapted from Rudra's pinned `tests/send_sync/wild_phantom.rs` fixture. The
// first backend deliberately does not infer safe access from inert pointer fields.
use std::marker::PhantomData;
use std::ptr::NonNull;

struct Atom1<'a, T> {
    ptr: NonNull<T>,
    _marker: PhantomData<&'a mut T>,
}

unsafe impl<'a, A> Send for Atom1<'a, A> {}
unsafe impl<'a, A> Sync for Atom1<'a, A> {}

fn main() {}
