//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/sync_variance.rpl
//@check-pass

// Adapted from Rudra's pinned `tests/send_sync/okay_channel.rs` known-FP fixture.
struct Channel<P, Q>(P, Q);

unsafe impl<P: Send, Q: Send> Sync for Channel<P, Q> {}

impl<P, Q> Channel<P, Q> {
    fn send_p<M>(&self, _message: M)
    where
        M: Into<P>,
    {
    }

    fn send_q(&self, _message: Box<Q>) {}
}

fn main() {}
