//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/sync_variance.rpl

// Adapted from Rudra's pinned `tests/send_sync/wild_channel.rs` fixture.
struct Container<P, Q>(P, Q);

unsafe impl<P: Sync, Q: Send> Sync for Container<P, Q> {}
//~^ ERROR: A pattern instance found in this span

impl<P, Q> Container<P, Q> {
    fn append_to_queue(&self, _message: Q) {}

    fn peek_queue_end(&self) -> Result<&Q, ()> {
        Ok(&self.1)
    }
}

fn main() {}
