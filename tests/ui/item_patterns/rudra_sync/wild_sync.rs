//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/sync_variance.rpl
//@check-pass

// Adapted from Rudra's pinned `tests/send_sync/wild_sync.rs` fixture. Rudra's
// naive tier reports the missing `Q: Sync`; RPL requires an accessible resource.
struct Atom<P, Q>(P, Q);

unsafe impl<P: Send, Q> Sync for Atom<P, Q>
where
    Q: Copy,
    P: Sync,
{
}

fn main() {}
