//@rustc-env: RPL_PATS=tests/ui/item_patterns/patterns/sync_variance.rpl
//@check-pass

// Adapted from Rudra's pinned `tests/send_sync/okay_where.rs` fixture.
struct Atom1<P, Q>(P, Q);
unsafe impl<P, Q> Send for Atom1<P, Q>
where
    Q: Send,
    P: Copy + Send,
{
}

struct Atom2<P>(P);
unsafe impl<P> Sync for Atom2<P> where P: Sync {}

fn main() {}
