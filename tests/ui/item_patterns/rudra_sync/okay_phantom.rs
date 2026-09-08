//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/sync_variance.rpl
//@check-pass

// Adapted from Rudra's pinned `tests/send_sync/okay_phantom.rs` fixture. Rudra
// reports this at its naive tier; RPL finds no resource accessible through `&Atom1`.
use std::marker::PhantomData;

struct Atom1<'a, P, Q, R> {
    _marker0: PhantomData<P>,
    _marker1: PhantomData<Option<*mut P>>,
    _marker2: PhantomData<Box<(&'a mut Q, Box<Result<R, i32>>)>>,
}

unsafe impl<'a, A: Send, B, C> Send for Atom1<'a, A, B, C> {}
unsafe impl<'a, A: Sync, B, C> Sync for Atom1<'a, A, B, C> {}

fn main() {}
